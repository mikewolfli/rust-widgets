// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! FindReplaceDialog widget — a find/replace dialog (like VS Code/IDE find).
//!
//! Provides a two-row dialog with find and replace input fields, toggle
//! buttons for match case, whole word, regex, and highlight all, plus
//! action buttons for find next, find previous, replace, replace all,
//! and close. Emits typed signals when actions are triggered.

use crate::compat::format;
use crate::core::{Color, HorizontalAlignment, ObjectId, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Padding inside the dialog.
const PADDING: i32 = 8;
/// Row height (input field area).
const ROW_HEIGHT: u32 = 28;
/// Small button size for toggle/action buttons.
const BTN_SIZE: u32 = 24;
/// The width of the trailing arrow buttons on the find row, and of the two action
/// buttons on the replace row.
///
/// Named once because the draw path and the hit test both step by it: the two used to
/// disagree (`24` at the draw site, a `28` stride the hit test measured with), so the
/// clickable area of Replace All sat one inset to the left of the button it named.
const ARROW_BTN: u32 = 24;
/// The width of the two replacement buttons on the replace row.
///
/// Wider than the arrow buttons because their labels are words, not glyphs: at `24` the labels
/// could only be `R` and `RA`, which a reader has to decode and which were the most conspicuous
/// wrong labels in the control. The trailing group is derived from this constant in both the draw
/// path and the hit test, so widening it moves the clickable button with the painted one.
const REPLACE_BTN: u32 = 52;
/// The width of the "Find:" / "Rpl:" label column.
///
/// Both rows reserve the same column, so the two entry fields start on the same x. The
/// draw path's `40` and the hit test's `46` were two different answers to the same
/// question, which is how a click could land in the gap between a label and its field.
const LABEL_WIDTH: u32 = 46;
/// Gap between adjacent widgets on a row.
const GAP: i32 = 6;

/// The four search flags the dialog exposes, as one value.
///
/// Grouped because they are always read together: a host running a search needs every one of them or
/// nobody's search is reproducible, and four separate payload fields would let a caller drop one on
/// the floor without noticing. [`Copy`] so a handler can stow it in a closure without a clone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    /// Whether the match is case-sensitive.
    pub case_sensitive: bool,
    /// Whether a match must be a whole word.
    pub whole_word: bool,
    /// Whether the pattern is a regular expression.
    pub use_regex: bool,
    /// Whether reaching the end of the document continues from the top.
    ///
    /// Defaults to `true`, matching [`FindReplaceDialog::new`] -- a search that stops at the end
    /// without saying so is what "wrap around = off" already means, so the default is the permissive
    /// one and the host must opt into the stop.
    pub wrap_around: bool,
}

/// One find request: the text to look for plus the flags to look for it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    /// The text the user typed into the find field.
    pub text: String,
    /// The options in force when the button was pressed.
    pub options: SearchOptions,
}

/// FindReplaceDialog widget — a find/replace panel for text search and replace.
pub struct FindReplaceDialog {
    base: BaseWidget,
    /// Current find (search) text.
    find_text: String,
    /// Current replace text.
    replace_text: String,
    /// Match case toggle state.
    match_case: bool,
    /// Whole word toggle state.
    whole_word: bool,
    /// Use regex toggle state.
    use_regex: bool,
    /// Highlight all matches toggle state.
    highlight_all: bool,
    /// Whether a search that reaches the end of the document continues from the top.
    wrap_around: bool,
    /// Whether the dialog is visible.
    visible: bool,

    // -- Signals --
    /// Emitted when find next is requested, with the find text and the dialog's search options.
    ///
    /// The payload is [`SearchRequest`] rather than a bare `String` because the dialog **has no
    /// document to search**: it reports what the user asked for and the host runs it. That makes
    /// every option a piece of information the host needs and cannot derive -- and this dialog
    /// declares four of them (`case_sensitive`, `whole_word`, `use_regex`, `wrap_around`), of which
    /// the old payload carried none. A host could read them back off the getters, but only by
    /// reaching for the widget again from inside a signal handler that had just been handed
    /// everything else it needs.
    pub find_next_signal: Signal1<SearchRequest>,
    /// Emitted when find previous is requested, with the find text and the search options.
    pub find_previous_signal: Signal1<SearchRequest>,
    /// Emitted when replace is requested, with the replace text and the search options.
    pub replace_signal: Signal1<SearchRequest>,
    /// Emitted when replace all is requested, with (find_text, replace_text) and the options.
    pub replace_all_signal: Signal1<(String, String, SearchOptions)>,
    /// Emitted when the dialog is closed.
    pub close_signal: Signal1<()>,

    // Internal state tracking
    /// Which text field has focus: 0 = find, 1 = replace
    focus_field: u8,
    /// Emitted when Tab moves the focus between the find and replace fields; the payload is
    /// this dialog's own object id (its two fields are not separate objects).
    pub focus_changed: Signal1<ObjectId>,
    /// Caret position, in characters, for each field.
    caret: [usize; 2],
    /// Selection anchor, in characters, for each field. Equal to the caret when nothing is
    /// selected; the range between the two is the selected text.
    anchor: [usize; 2],
    /// Whether the host has delivered committed text. Once it has, raw key codes are no longer
    /// treated as characters, so a non-Latin layout is not double-inserted.
    text_input_seen: bool,
    /// The host's match report: `(current, total)`, or `None` for "no matches" (DLG-11).
    match_report: Option<(usize, usize)>,
}

impl FindReplaceDialog {
    /// Creates a new FindReplaceDialog with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FindReplaceDialog, geometry, "FindReplaceDialog"),
            find_text: String::new(),
            replace_text: String::new(),
            match_case: false,
            whole_word: false,
            use_regex: false,
            highlight_all: false,
            wrap_around: true,
            visible: false,
            find_next_signal: Signal1::new(),
            find_previous_signal: Signal1::new(),
            replace_signal: Signal1::new(),
            replace_all_signal: Signal1::new(),
            close_signal: Signal1::new(),
            focus_field: 0,
            focus_changed: Signal1::new(),
            caret: [0, 0],
            anchor: [0, 0],
            text_input_seen: false,
            match_report: None,
        }
    }

    // ── Find text accessors ──

    /// Returns the current find text.
    pub fn find_text(&self) -> &str {
        &self.find_text
    }

    /// Sets the find text.
    pub fn set_find_text(&mut self, text: &str) {
        if self.find_text == text {
            return;
        }
        self.find_text = text.to_string();
        let length = self.find_text.chars().count();
        self.caret[0] = self.caret[0].min(length);
        self.anchor[0] = self.anchor[0].min(length);
        self.base.request_redraw();
    }

    /// Returns the current replace text.
    pub fn replace_text(&self) -> &str {
        &self.replace_text
    }

    /// Sets the replace text.
    pub fn set_replace_text(&mut self, text: &str) {
        if self.replace_text == text {
            return;
        }
        self.replace_text = text.to_string();
        let length = self.replace_text.chars().count();
        self.caret[1] = self.caret[1].min(length);
        self.anchor[1] = self.anchor[1].min(length);
        self.base.request_redraw();
    }

    /// Records the host's match report and repaints.
    ///
    /// `(current, total)` is the ordinary "3 of 12" answer; `None` states that the search ran
    /// and found nothing, which is the one result a panel cannot derive from its own state
    /// (DLG-11).
    pub fn set_match_report(&mut self, report: Option<(usize, usize)>) {
        if self.match_report == report {
            return;
        }
        self.match_report = report;
        self.base.request_redraw();
    }

    /// The current match report, as recorded by [`Self::set_match_report`].
    pub fn match_report(&self) -> Option<(usize, usize)> {
        self.match_report
    }

    // ── Match case ──

    /// Returns whether match case is enabled.
    pub fn is_match_case(&self) -> bool {
        self.match_case
    }

    /// Sets the match case toggle.
    pub fn set_match_case(&mut self, value: bool) {
        if self.match_case == value {
            return;
        }
        self.match_case = value;
        self.base.request_redraw();
    }

    // ── Whole word ──

    /// Returns whether whole word matching is enabled.
    pub fn is_whole_word(&self) -> bool {
        self.whole_word
    }

    /// Sets the whole word toggle.
    pub fn set_whole_word(&mut self, value: bool) {
        if self.whole_word == value {
            return;
        }
        self.whole_word = value;
        self.base.request_redraw();
    }

    // ── Use regex ──

    /// Returns whether regex mode is enabled.
    pub fn is_use_regex(&self) -> bool {
        self.use_regex
    }

    /// Sets the regex toggle.
    pub fn set_use_regex(&mut self, value: bool) {
        if self.use_regex == value {
            return;
        }
        self.use_regex = value;
        self.base.request_redraw();
    }

    // ── Highlight all ──

    /// Returns whether highlight all matches is enabled.
    pub fn is_highlight_all(&self) -> bool {
        self.highlight_all
    }

    /// Sets the highlight all toggle.
    pub fn set_highlight_all(&mut self, value: bool) {
        if self.highlight_all == value {
            return;
        }
        self.highlight_all = value;
        self.base.request_redraw();
    }

    // ── Wrap around ──

    /// Returns whether a search wraps around to the start of the document.
    pub fn is_wrap_around(&self) -> bool {
        self.wrap_around
    }

    /// The four search flags as one value, which is what the signals carry.
    ///
    /// Built here rather than at each of the four emit sites, so a fifth flag added later cannot be
    /// added to one payload and forgotten in the other three.
    fn search_options(&self) -> SearchOptions {
        SearchOptions {
            case_sensitive: self.match_case,
            whole_word: self.whole_word,
            use_regex: self.use_regex,
            wrap_around: self.wrap_around,
        }
    }

    /// Sets the wrap-around toggle.
    pub fn set_wrap_around(&mut self, value: bool) {
        if self.wrap_around == value {
            return;
        }
        self.wrap_around = value;
        self.base.request_redraw();
    }

    // ── Visibility ──

    /// Shows the dialog.
    pub fn show(&mut self) {
        self.visible = true;
        self.base.request_redraw();
    }

    /// Hides the dialog.
    pub fn hide(&mut self) {
        self.visible = false;
        self.base.request_redraw();
    }

    /// Returns whether the dialog is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // ── Actions ──

    /// Emits `find_next_signal` with the find text and the options in force.
    ///
    /// The options travel with the text because the host is the one that searches -- see the signal's
    /// own documentation for why a bare `String` was not enough.
    pub fn find_next(&mut self) {
        if !self.find_text.is_empty() {
            let request =
                SearchRequest { text: self.find_text.clone(), options: self.search_options() };
            self.find_next_signal.emit(request);
        }
    }

    /// Emits `find_previous_signal` with the find text and the options in force.
    pub fn find_previous(&mut self) {
        if !self.find_text.is_empty() {
            let request =
                SearchRequest { text: self.find_text.clone(), options: self.search_options() };
            self.find_previous_signal.emit(request);
        }
    }

    /// Emits `replace_signal` with the replacement text and the options in force.
    pub fn replace(&mut self) {
        if !self.find_text.is_empty() {
            let request =
                SearchRequest { text: self.replace_text.clone(), options: self.search_options() };
            self.replace_signal.emit(request);
        }
    }

    /// Emits `replace_all_signal` with (find_text, replace_text) and the options in force.
    pub fn replace_all(&mut self) {
        if !self.find_text.is_empty() {
            let options = self.search_options();
            self.replace_all_signal.emit((
                self.find_text.clone(),
                self.replace_text.clone(),
                options,
            ));
        }
    }

    /// The focused field's index.
    fn field_index(&self) -> usize {
        usize::from(self.focus_field.min(1))
    }

    /// The text of field `index`.
    fn field_text(&self, index: usize) -> &str {
        if index == 0 {
            &self.find_text
        } else {
            &self.replace_text
        }
    }

    /// Replaces the text of field `index`.
    fn set_field_text(&mut self, index: usize, text: String) {
        if index == 0 {
            self.find_text = text;
        } else {
            self.replace_text = text;
        }
        let length = self.field_text(index).chars().count();
        self.caret[index] = self.caret[index].min(length);
        self.anchor[index] = self.anchor[index].min(length);
        self.base.request_redraw();
    }

    /// The selected character range of field `index`, if any.
    fn selected_range(&self, index: usize) -> Option<(usize, usize)> {
        let start = self.caret[index].min(self.anchor[index]);
        let end = self.caret[index].max(self.anchor[index]);
        (start != end).then_some((start, end))
    }

    /// The byte offset of character `index` in `text`.
    fn byte_index(text: &str, index: usize) -> usize {
        text.char_indices().nth(index).map(|(offset, _)| offset).unwrap_or(text.len())
    }

    /// Replaces the focused field's selection (or inserts at the caret) with `inserted`.
    fn insert_text(&mut self, inserted: &str) {
        let index = self.field_index();
        let current = self.field_text(index).to_string();
        let (start, end) =
            self.selected_range(index).unwrap_or((self.caret[index], self.caret[index]));
        let start_byte = Self::byte_index(&current, start);
        let end_byte = Self::byte_index(&current, end);
        let mut next = String::with_capacity(current.len() + inserted.len());
        next.push_str(&current[..start_byte]);
        next.push_str(inserted);
        next.push_str(&current[end_byte..]);
        let caret = start + inserted.chars().count();
        self.caret[index] = caret;
        self.anchor[index] = caret;
        self.set_field_text(index, next);
    }

    /// Deletes the focused field's selection, or one character before the caret.
    fn backspace_focused(&mut self) {
        let index = self.field_index();
        let current = self.field_text(index).to_string();
        if let Some((start, end)) = self.selected_range(index) {
            let start_byte = Self::byte_index(&current, start);
            let end_byte = Self::byte_index(&current, end);
            let mut next = String::with_capacity(current.len());
            next.push_str(&current[..start_byte]);
            next.push_str(&current[end_byte..]);
            self.caret[index] = start;
            self.anchor[index] = start;
            self.set_field_text(index, next);
        } else if self.caret[index] > 0 {
            let end_byte = Self::byte_index(&current, self.caret[index]);
            let start_byte = Self::byte_index(&current, self.caret[index] - 1);
            let mut next = String::with_capacity(current.len());
            next.push_str(&current[..start_byte]);
            next.push_str(&current[end_byte..]);
            self.caret[index] -= 1;
            self.anchor[index] = self.caret[index];
            self.set_field_text(index, next);
        }
    }

    /// Deletes the focused field's selection, or one character after the caret.
    fn delete_focused(&mut self) {
        let index = self.field_index();
        let current = self.field_text(index).to_string();
        if let Some((start, end)) = self.selected_range(index) {
            let start_byte = Self::byte_index(&current, start);
            let end_byte = Self::byte_index(&current, end);
            let mut next = String::with_capacity(current.len());
            next.push_str(&current[..start_byte]);
            next.push_str(&current[end_byte..]);
            self.caret[index] = start;
            self.anchor[index] = start;
            self.set_field_text(index, next);
        } else if self.caret[index] < current.chars().count() {
            let start_byte = Self::byte_index(&current, self.caret[index]);
            let end_byte = Self::byte_index(&current, self.caret[index] + 1);
            let mut next = String::with_capacity(current.len());
            next.push_str(&current[..start_byte]);
            next.push_str(&current[end_byte..]);
            self.set_field_text(index, next);
        }
    }

    /// Moves the focused field's caret, extending the selection when `extend` is set.
    fn move_caret(&mut self, to: usize, extend: bool) {
        let index = self.field_index();
        let length = self.field_text(index).chars().count();
        let to = to.min(length);
        if !extend {
            self.anchor[index] = to;
        } else if self.caret[index] == self.anchor[index] {
            // Start the selection at the old caret, so the first shift+arrow selects the
            // character the caret moves across.
            self.anchor[index] = self.caret[index];
        }
        self.caret[index] = to;
        self.base.request_redraw();
    }

    /// The caret index under a click at `x` inside a field whose text starts at `left`.
    fn caret_at_x(&self, x: i32, left: i32, advance: u32) -> usize {
        let index = self.field_index();
        let length = self.field_text(index).chars().count();
        let offset = (x - left).max(0) as u32;
        ((offset / advance.max(1)) as usize).min(length)
    }

    /// Computes layout rectangles for the two rows.
    ///
    /// Both rows are stored, so the draw path and the hit test step through the **same**
    /// boxes: they used to re-derive their own x offsets from four separate literals
    /// (`46` here, `40 + GAP` there), and the entry-field region the click test built was
    /// 6 px narrower than the field it was meant to name.
    fn compute_layout(&self) -> (Rect, Rect) {
        let geom = self.frame_rect();
        let row1 = Rect::new(
            geom.x + PADDING,
            geom.y + PADDING,
            geom.width.saturating_sub((PADDING as u32) * 2),
            ROW_HEIGHT,
        );
        let row2 = Rect::new(
            geom.x + PADDING,
            row1.y + row1.height as i32 + GAP,
            geom.width.saturating_sub((PADDING as u32) * 2),
            ROW_HEIGHT,
        );
        (row1, row2)
    }

    /// The bar this control actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the bar is not the caller's rectangle
    ///
    /// `rect` is the area the find bar is **offered**. Painting it verbatim made the
    /// 240x120 census cell a 240x120 panel, and — because the two rows were then anchored
    /// to that panel's top edge — left three quarters of the bar empty with both rows
    /// crammed into the top eighth. [`ControlMetrics::painted_box`] caps the width at
    /// [`dimensions::DIALOG_MIN_WIDTH`] and the height at the two rows the control
    /// actually has, then centres the result, so the chrome is sized by what it contains
    /// rather than by whatever box the caller handed it.
    fn frame_rect(&self) -> Rect {
        // Two rows plus the padding above, between and below them: the bar's intrinsic
        // height is derived from its own parts so a row-height change cannot leave the
        // frame a row short.
        let intrinsic_height = PADDING as u32 * 2 + ROW_HEIGHT * 2 + GAP as u32;
        ControlMetrics::painted_box(
            self.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, intrinsic_height),
        )
    }

    /// The x each trailing column starts at on a row of `row`, laid out from the row's
    /// **right edge** inwards.
    ///
    /// # Why the fixed columns are placed from the right
    ///
    /// The row's fixed budget on a 240 px control did not fit: the two arrow buttons were
    /// drawn at x 240 and x 270, entirely outside the frame, and at 400 px the Replace All
    /// button sat 30 px past the right edge. Anchoring every fixed-width control to the
    /// row's right edge makes the arrows — the affordances — the last thing to be placed
    /// and the one flexible column (the entry field) absorb the remainder, so no fixed
    /// column can leave the frame however narrow the caller's rectangle is.
    ///
    /// The arithmetic is floored at the row's own left edge, so even a row narrower than
    /// its trailing columns keeps every button inside the frame.
    fn trailing_x(row: Rect, trailing_width: i32) -> i32 {
        let right = row.x + row.width as i32 - trailing_width;
        right.max(row.x)
    }

    /// The line box, vertically centred in `cell`, that a label in `font` should occupy.
    ///
    /// Both rows of this panel draw each label from a hand-computed `y + height / 2 + 4`
    /// origin. The `+ 4` is not a centring term: the renderer's origin is the glyph's
    /// **top-left**, so the box painted downward from a point already below the cell's own
    /// middle and the bottom half of every label hung out of the row. Centring the measured
    /// line inside the cell is the correct placement, and naming it once keeps the entry
    /// fields, the toggles and the buttons aligned to the same rule.
    ///
    /// The arithmetic now lives in [`crate::render::text_line`], which this delegates to: the
    /// rule was correct here first and has since become the crate-wide primitive for it, so
    /// keeping a second copy would be the drift this file's own comment warns about.
    fn text_line(&self, cell: Rect, font: &crate::core::Font, context: &RenderContext) -> Rect {
        context.text_line(cell, font)
    }
}

impl Widget for FindReplaceDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::FIND_REPLACE_DIALOG_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::FIND_REPLACE_DIALOG_DEFAULT_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "find_next" => Some(EventSignalRef::mapped("find_next", &self.find_next_signal, |v| {
                CapabilityValue::String(format!("{v:?}"))
            })),
            "find_previous" => {
                Some(EventSignalRef::mapped("find_previous", &self.find_previous_signal, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "replace" => Some(EventSignalRef::mapped("replace", &self.replace_signal, |v| {
                CapabilityValue::String(format!("{v:?}"))
            })),
            "replace_all" => {
                Some(EventSignalRef::mapped("replace_all", &self.replace_all_signal, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::String(v.0.clone()),
                        CapabilityValue::String(v.1.clone()),
                        CapabilityValue::String(format!("{:?}", v.2)),
                    ])
                }))
            }
            "close" => {
                Some(EventSignalRef::mapped("close", &self.close_signal, |_| CapabilityValue::Null))
            }
            "focus_changed" => {
                Some(EventSignalRef::mapped("focus_changed", &self.focus_changed, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `FindReplaceDialog`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for FindReplaceDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "find_text" => Ok(CapabilityValue::String(self.find_text().to_string())),
            "replace_text" => Ok(CapabilityValue::String(self.replace_text().to_string())),
            "match_case" => Ok(CapabilityValue::Bool(self.is_match_case())),
            "whole_word" => Ok(CapabilityValue::Bool(self.is_whole_word())),
            "use_regex" => Ok(CapabilityValue::Bool(self.is_use_regex())),
            "highlight_all" => Ok(CapabilityValue::Bool(self.is_highlight_all())),
            "wrap_around" => Ok(CapabilityValue::Bool(self.is_wrap_around())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "find_text" => {
                self.set_find_text(&expect_string(value)?);
                Ok(())
            }
            "replace_text" => {
                self.set_replace_text(&expect_string(value)?);
                Ok(())
            }
            "match_case" => {
                self.set_match_case(expect_bool(value)?);
                Ok(())
            }
            "whole_word" => {
                self.set_whole_word(expect_bool(value)?);
                Ok(())
            }
            "use_regex" => {
                self.set_use_regex(expect_bool(value)?);
                Ok(())
            }
            "highlight_all" => {
                self.set_highlight_all(expect_bool(value)?);
                Ok(())
            }
            "wrap_around" => {
                self.set_wrap_around(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "find_text",
            "replace_text",
            "match_case",
            "whole_word",
            "use_regex",
            "highlight_all",
            "wrap_around",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for FindReplaceDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible {
            return;
        }

        // The **bar**, not the control's rectangle: see `frame_rect`. Every band below —
        // the two rows, their labels, their fields and their buttons — is derived from
        // this one rect, so the ink and the hit test cannot be anchored to different boxes.
        let geom = self.frame_rect();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Every colour below used
        // to be one of the `Color::BACKGROUND` / `FOREGROUND` / `PRIMARY` constants, which
        // do not move when the appearance switches — the rendering census reported the
        // control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("find_replace_dialog");
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        let ink = style.text_color.or(themed_ink).unwrap_or(Color::FOREGROUND);
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::BACKGROUND);
        let border = style
            .border_color
            .or(themed_border)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.25));
        // The entry fields and the accent come from the shared palette snapshot, so no manager
        // guard is held while the bar is drawn.
        let (window_fill, accent, muted) = {
            match crate::style::palette_shared() {
                Some(colors) => (colors.background, colors.primary, colors.secondary),
                None => (Color::BACKGROUND, Color::PRIMARY, Color::SECONDARY),
            }
        };
        // The two entry fields are editable interiors: one step *away* from the bar's own
        // surface, so on a light bar they are lighter and on a dark one darker, rather
        // than the forced white they used to be.
        let field = if surface.is_dark() {
            surface.blend(&Color::WHITE, 0.06)
        } else {
            surface.blend(&Color::BLACK, 0.04)
        };
        let accent_ink = accent.contrast_color();
        let muted_ink = muted.contrast_color();

        // ── Background ──
        // The bar floats over the surface it searches, so its fill is derived from its own
        // resolved surface and nudged away from the window fill.
        let bar = if surface == window_fill { surface.blend(&ink, 0.06) } else { surface };
        context.fill_rect(geom, bar);
        context.draw_rect_stroke(geom, border, 1);

        // The row rectangles are local, not mirrored onto `self`.
        //
        // They used to be written into two struct fields here and read by nobody: the event
        // path calls `compute_layout` for itself (see the press arm below), so the mirror could
        // only ever drift from the value the draw pass actually used — the "write-only field"
        // shape principle #99 forbids.
        let (find_row, replace_row) = self.compute_layout();

        // ── Find row: label + input + toggles + buttons ──
        //
        // The four toggles and the two arrows are a **fixed** trailing group; only the
        // entry field flexes. The group's width is named once so the field can take
        // exactly the remainder and no fixed column can be pushed past the row's right
        // edge — see `trailing_x` for the defect that caused.
        let trailing = GAP * 6 + BTN_SIZE as i32 * 4 + ARROW_BTN as i32 * 2;
        let label_col = LABEL_WIDTH.min(find_row.width);
        let input_width = (find_row.width as i32 - label_col as i32 - GAP - trailing).max(0) as u32;
        let mut x = find_row.x;

        // "Find:" label. Bounded by the label column, so it truncates there rather than
        // running into the input it names.
        let label_rect = Rect::new(x, find_row.y, label_col, find_row.height);
        // The **effective font** — the resolved theme/caller font — for every label and field in
        // the bar, rather than the fixed 12 px sans-serif this used to build (D09-STYLE-01).
        let font = effective_font(style);
        let label_line = self.text_line(label_rect, font, context);
        context.draw_text_fitted_cow(
            label_line,
            &super::localized_label("dialog.find_replace.find", "Find:"),
            font,
            ink,
            HorizontalAlignment::Left,
        );
        x += label_col as i32 + GAP;

        // Find text input background. The field is only drawn when the row genuinely
        // leaves room for it: on a bar narrower than its own trailing columns the
        // remainder floors to zero, and a zero-width field is an element the SVG backend
        // emits and the rasteriser skips — a silent divergence. Dropping it keeps the two
        // backends in agreement.
        if input_width > 0 {
            let input_rect = Rect::new(x, find_row.y, input_width, find_row.height);
            context.fill_rect(input_rect, field);
            context.draw_rect_stroke(input_rect, border, 1);
            // Guarded on the text being non-empty: an unguarded `draw_text_fitted` emits
            // `<text …></text>`, an empty element that the rasteriser never produces.
            let display_text = if self.find_text.is_empty() { "" } else { &self.find_text };
            if !display_text.is_empty() {
                let input_line = self.text_line(input_rect, font, context);
                context.draw_text_fitted_cow(
                    input_line,
                    display_text,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
            // The host's match report is painted inside the field's trailing edge, so the
            // user sees "3 / 12" (or "no matches") beside the query that produced it
            // (DLG-11). Only a non-empty query can have a report worth showing.
            if !self.find_text.is_empty() {
                let status = match self.match_report {
                    Some((current, total)) => format!("{current} / {total}"),
                    None => super::localized_label("dialog.find_replace.no_matches", "No matches"),
                };
                let status_band = Rect::new(
                    input_rect.x + input_rect.width as i32 / 3,
                    input_rect.y,
                    (input_rect.width - input_rect.width / 3).saturating_sub(6),
                    input_rect.height,
                );
                let status_line = self.text_line(status_band, font, context);
                context.draw_text_fitted_cow(
                    status_line,
                    &status,
                    font,
                    ink.blend(&field, 0.35),
                    HorizontalAlignment::Right,
                );
            }
        }
        x = Self::trailing_x(find_row, trailing);

        // Match Case toggle
        let mc_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let mc_color = if self.match_case { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(mc_rect, mc_color);
        let mc_line = self.text_line(mc_rect, font, context);
        context.draw_text_fitted_cow(
            mc_line,
            "Aa",
            font,
            if self.match_case { accent_ink } else { ink },
            HorizontalAlignment::Center,
        );
        x += BTN_SIZE as i32 + GAP;

        // Whole Word toggle
        let ww_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let ww_color = if self.whole_word { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(ww_rect, ww_color);
        let ww_line = self.text_line(ww_rect, font, context);
        context.draw_text_fitted_cow(
            ww_line,
            "W",
            font,
            if self.whole_word { accent_ink } else { ink },
            HorizontalAlignment::Center,
        );
        x += BTN_SIZE as i32 + GAP;

        // Regex toggle
        let rx_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let rx_color = if self.use_regex { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(rx_rect, rx_color);
        let rx_line = self.text_line(rx_rect, font, context);
        context.draw_text_fitted_cow(
            rx_line,
            ".*",
            font,
            if self.use_regex { accent_ink } else { ink },
            HorizontalAlignment::Center,
        );
        x += BTN_SIZE as i32 + GAP;

        // Highlight All toggle
        let ha_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let ha_color = if self.highlight_all { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(ha_rect, ha_color);
        let ha_line = self.text_line(ha_rect, font, context);
        context.draw_text_fitted_cow(
            ha_line,
            "H",
            font,
            if self.highlight_all { accent_ink } else { ink },
            HorizontalAlignment::Center,
        );
        x += BTN_SIZE as i32 + GAP;

        // Find Previous button. The glyph is an `IconName` outline rather than the `U+25B2`
        // character it used to be: no bundled face covers the geometric-shapes block, so the
        // character fell back to an 8x8 bitmap block and the SVG showed a coarse arrow where the
        // pixels showed nothing. An icon is geometry, and both backends already agree on it.
        let fp_rect = Rect::new(x, find_row.y, ARROW_BTN, find_row.height);
        context.fill_rect(fp_rect, muted);
        let arrow_side = find_row.height.min(ARROW_BTN);
        crate::widget::draw_icon_centered(
            context,
            fp_rect,
            arrow_side,
            muted_ink,
            IconName::ArrowUp,
        );
        x += ARROW_BTN as i32 + GAP;

        // Find Next button
        let fn_rect = Rect::new(x, find_row.y, ARROW_BTN, find_row.height);
        context.fill_rect(fn_rect, accent);
        crate::widget::draw_icon_centered(
            context,
            fn_rect,
            arrow_side,
            accent_ink,
            IconName::ArrowDown,
        );

        // ── Replace row: label + input + buttons ──
        //
        // The replace row's trailing group is the two action buttons and the two GAPs
        // around them, laid out from the right edge by the same helper as the find row, so
        // the Replace All button sits inside the frame instead of 30 px past it.
        let r_trailing = GAP * 2 + REPLACE_BTN as i32 * 2;
        let r_input_width =
            (replace_row.width as i32 - label_col as i32 - GAP - r_trailing).max(0) as u32;
        let mut x2 = replace_row.x;

        // "Replace:" label
        let rl_rect = Rect::new(x2, replace_row.y, label_col, replace_row.height);
        let rl_line = self.text_line(rl_rect, font, context);
        context.draw_text_fitted_cow(
            rl_line,
            &super::localized_label("dialog.find_replace.replace", "Rpl:"),
            font,
            ink,
            HorizontalAlignment::Left,
        );
        x2 += label_col as i32 + GAP;

        // Replace text input background. Drawn only when there is room, for the same
        // zero-width reason the find field is guarded above.
        if r_input_width > 0 {
            let r_input_rect = Rect::new(x2, replace_row.y, r_input_width, replace_row.height);
            context.fill_rect(r_input_rect, field);
            context.draw_rect_stroke(r_input_rect, border, 1);
            let r_text = if self.replace_text.is_empty() { "" } else { &self.replace_text };
            if !r_text.is_empty() {
                let r_input_line = self.text_line(r_input_rect, font, context);
                context.draw_text_fitted_cow(
                    r_input_line,
                    r_text,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
        x2 = Self::trailing_x(replace_row, r_trailing);

        // Replace button. The labels are words rather than `R`/`RA`: a one- or two-letter label
        // is a code the reader has to puzzle out, and at this width there is room for `Rpl` and
        // `Rpl All` when the two buttons take `REPLACE_BTN` each.
        let rep_rect = Rect::new(x2, replace_row.y, REPLACE_BTN, replace_row.height);
        context.fill_rect(rep_rect, muted);
        let rep_line = self.text_line(rep_rect, font, context);
        context.draw_text_fitted_cow(
            rep_line,
            &super::localized_label("dialog.find_replace.replace_short", "Rpl"),
            font,
            muted_ink,
            HorizontalAlignment::Center,
        );
        x2 += REPLACE_BTN as i32 + GAP;

        // Replace All button
        let ra_rect = Rect::new(x2, replace_row.y, REPLACE_BTN, replace_row.height);
        context.fill_rect(ra_rect, muted);
        let ra_line = self.text_line(ra_rect, font, context);
        context.draw_text_fitted_cow(
            ra_line,
            &super::localized_label("dialog.find_replace.replace_all", "Rpl All"),
            font,
            muted_ink,
            HorizontalAlignment::Center,
        );
    }
}

impl EventHandler for FindReplaceDialog {
    fn handle_event(&mut self, event: &Event) {
        if !self.visible {
            self.base.handle_event(event);
            return;
        }

        // A disabled dialog must not act on input: no focus changes, no find/replace
        // actions, no close. Only `visible` was checked here, so `set_enabled(false)`
        // left the whole dialog live.
        if !self.base.is_enabled() {
            self.base.handle_event(event);
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    // Check clicks on find row buttons. The regions come from the same
                    // `compute_layout` + trailing-column arithmetic the draw path uses, so
                    // a button's clickable area cannot drift from its ink. They used to be
                    // re-derived here from their own literals — `46` for the label column
                    // against the draw path's `40 + GAP`, and a `28` stride between the two
                    // replace buttons against the `24` they were drawn at — which put the
                    // Replace All hit region one inset to the left of the button it named.
                    let (find_row, replace_row) = self.compute_layout();
                    let label_col = LABEL_WIDTH.min(find_row.width);
                    let trailing = GAP * 6 + BTN_SIZE as i32 * 4 + ARROW_BTN as i32 * 2;

                    // Focus find field on click. The region is exactly the drawn field:
                    // when the row leaves no room for it (`input_width == 0`) there is no
                    // field to click, so it is not reported as one.
                    let find_input_w =
                        (find_row.width as i32 - label_col as i32 - GAP - trailing).max(0);
                    if find_input_w > 0 {
                        let find_input_region = Rect::new(
                            find_row.x + label_col as i32 + GAP,
                            find_row.y,
                            find_input_w as u32,
                            find_row.height,
                        );
                        if find_input_region.contains_point(*pos) {
                            self.focus_field = 0;
                            let advance =
                                effective_font(self.base.style()).size().ceil().max(1.0) as u32;
                            let caret = self.caret_at_x(pos.x, find_input_region.x + 4, advance);
                            self.caret[0] = caret;
                            self.anchor[0] = caret;
                            self.base.request_redraw();
                            return;
                        }
                    }

                    // Click on replace input
                    let r_trailing = GAP * 2 + REPLACE_BTN as i32 * 2;
                    let r_label_col = LABEL_WIDTH.min(replace_row.width);
                    let r_input_width =
                        (replace_row.width as i32 - r_label_col as i32 - GAP - r_trailing).max(0);
                    if r_input_width > 0 {
                        let r_input_region = Rect::new(
                            replace_row.x + r_label_col as i32 + GAP,
                            replace_row.y,
                            r_input_width as u32,
                            replace_row.height,
                        );
                        if r_input_region.contains_point(*pos) {
                            self.focus_field = 1;
                            let advance =
                                effective_font(self.base.style()).size().ceil().max(1.0) as u32;
                            let caret = self.caret_at_x(pos.x, r_input_region.x + 4, advance);
                            self.caret[1] = caret;
                            self.anchor[1] = caret;
                            self.base.request_redraw();
                            return;
                        }
                    }

                    // Toggle buttons on find row. Placed from the row's right edge by the
                    // same helper the draw path uses, so the two agree by construction.
                    let button_start_x = Self::trailing_x(find_row, trailing);

                    // Match Case
                    let mc_rect = Rect::new(button_start_x, find_row.y, BTN_SIZE, find_row.height);
                    if mc_rect.contains_point(*pos) {
                        self.match_case = !self.match_case;
                        self.base.request_redraw();
                        return;
                    }

                    // Whole Word
                    let ww_rect = Rect::new(
                        button_start_x + BTN_SIZE as i32 + GAP,
                        find_row.y,
                        BTN_SIZE,
                        find_row.height,
                    );
                    if ww_rect.contains_point(*pos) {
                        self.whole_word = !self.whole_word;
                        self.base.request_redraw();
                        return;
                    }

                    // Regex
                    let rx_rect = Rect::new(
                        button_start_x + (BTN_SIZE as i32 + GAP) * 2,
                        find_row.y,
                        BTN_SIZE,
                        find_row.height,
                    );
                    if rx_rect.contains_point(*pos) {
                        self.use_regex = !self.use_regex;
                        self.base.request_redraw();
                        return;
                    }

                    // Highlight All
                    let ha_rect = Rect::new(
                        button_start_x + (BTN_SIZE as i32 + GAP) * 3,
                        find_row.y,
                        BTN_SIZE,
                        find_row.height,
                    );
                    if ha_rect.contains_point(*pos) {
                        self.highlight_all = !self.highlight_all;
                        self.base.request_redraw();
                        return;
                    }

                    // Find Previous button
                    let fp_x = button_start_x + (BTN_SIZE as i32 + GAP) * 4;
                    let fp_rect = Rect::new(fp_x, find_row.y, ARROW_BTN, find_row.height);
                    if fp_rect.contains_point(*pos) {
                        self.find_previous();
                        return;
                    }

                    // Find Next button
                    let fn_rect = Rect::new(
                        fp_x + ARROW_BTN as i32 + GAP,
                        find_row.y,
                        ARROW_BTN,
                        find_row.height,
                    );
                    if fn_rect.contains_point(*pos) {
                        self.find_next();
                        return;
                    }

                    // Replace button. Anchored to the row's right edge by the same helper
                    // the draw path uses, so the hit region is the button.
                    let r_btn_x = Self::trailing_x(replace_row, r_trailing);
                    let rep_rect =
                        Rect::new(r_btn_x, replace_row.y, REPLACE_BTN, replace_row.height);
                    if rep_rect.contains_point(*pos) {
                        self.replace();
                        return;
                    }

                    // Replace All button
                    let ra_rect = Rect::new(
                        r_btn_x + REPLACE_BTN as i32 + GAP,
                        replace_row.y,
                        REPLACE_BTN,
                        replace_row.height,
                    );
                    if ra_rect.contains_point(*pos) {
                        self.replace_all();
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                let shift = *modifiers & 1 != 0;
                let primary = *modifiers & (2 | 8) != 0;
                match *key {
                    27 => {
                        // Escape
                        self.visible = false;
                        self.close_signal.emit(());
                        self.base.request_redraw();
                    }
                    13 => {
                        // Enter — trigger find next
                        self.find_next();
                    }
                    8 => {
                        // Backspace
                        self.backspace_focused();
                    }
                    46 => {
                        // Forward delete
                        self.delete_focused();
                    }
                    9 => {
                        // Tab — switch focus between find and replace
                        self.focus_field = 1 - self.focus_field;
                        let index = self.field_index();
                        self.caret[index] = self.field_text(index).chars().count();
                        self.anchor[index] = self.caret[index];
                        self.base.request_redraw();
                        self.focus_changed.emit(self.id());
                    }
                    37 | 39 | 36 | 35 => {
                        // Left / Right / Home / End
                        let index = self.field_index();
                        let length = self.field_text(index).chars().count();
                        let target = match *key {
                            37 => self.caret[index].saturating_sub(1),
                            39 => (self.caret[index] + 1).min(length),
                            36 => 0,
                            _ => length,
                        };
                        self.move_caret(target, shift);
                    }
                    65 | 97 if primary => {
                        // Primary+A — select all in the focused field
                        let index = self.field_index();
                        self.anchor[index] = 0;
                        self.caret[index] = self.field_text(index).chars().count();
                        self.base.request_redraw();
                    }
                    _ => {
                        // Printable ASCII is the fallback for a host that only reports key
                        // codes. Once committed text has arrived, the backend is speaking the
                        // text protocol and the raw key would either duplicate the character
                        // or insert a code a non-Latin layout never meant as a character.
                        if !self.text_input_seen && *key >= 32 && *key <= 126 {
                            if let Some(ch) = char::from_u32(*key) {
                                self.insert_text(&ch.to_string());
                            }
                        }
                    }
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                self.text_input_seen = true;
                let filtered: String =
                    text.chars().filter(|character| !character.is_control()).collect();
                if !filtered.is_empty() {
                    self.insert_text(&filtered);
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

// These tests drive the **theme**, which only exists in a build with a device profile
// (see `crate::lib`: `pub mod theme` is gated on `device_profile`). Without this gate the
// `mini` and `embedded` profiles fail to compile their test targets, because the test code
// names a module that those builds compile out — the production code is profile-clean and
// only the fixture was not.
#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::event::key_codes;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn find_replace_dialog_default_creation() {
        let dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        assert_eq!(dialog.kind(), WidgetKind::FindReplaceDialog);
        assert!(!dialog.is_visible());
        assert!(dialog.find_text().is_empty());
        assert!(dialog.replace_text().is_empty());
        assert!(!dialog.is_match_case());
        assert!(!dialog.is_whole_word());
        assert!(!dialog.is_use_regex());
        assert!(!dialog.is_highlight_all());
    }

    #[test]
    fn find_replace_dialog_set_get_text() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("hello");
        assert_eq!(dialog.find_text(), "hello");

        dialog.set_replace_text("world");
        assert_eq!(dialog.replace_text(), "world");

        dialog.set_find_text("");
        assert!(dialog.find_text().is_empty());
    }

    #[test]
    fn find_replace_dialog_toggle_options() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));

        assert!(!dialog.is_match_case());
        dialog.set_match_case(true);
        assert!(dialog.is_match_case());
        dialog.set_match_case(false);
        assert!(!dialog.is_match_case());

        assert!(!dialog.is_whole_word());
        dialog.set_whole_word(true);
        assert!(dialog.is_whole_word());

        assert!(!dialog.is_use_regex());
        dialog.set_use_regex(true);
        assert!(dialog.is_use_regex());

        assert!(!dialog.is_highlight_all());
        dialog.set_highlight_all(true);
        assert!(dialog.is_highlight_all());
    }

    #[test]
    fn find_replace_dialog_show_hide() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        assert!(!dialog.is_visible());

        dialog.show();
        assert!(dialog.is_visible());

        dialog.hide();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn find_replace_dialog_signals_find_next() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("search");
        dialog.show();

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.find_next_signal.connect(move |request: Arc<SearchRequest>| {
            if request.text == "search" {
                fired_clone.store(true, Ordering::SeqCst);
            }
        });

        dialog.find_next();
        assert!(fired.load(Ordering::SeqCst), "find_next_signal should fire with the find text");
    }

    /// The find signal carries the **options**, not only the text.
    ///
    /// # The defect this pins
    ///
    /// The dialog declares four search flags and its signals carried none of them, so a host could
    /// not run the search the user asked for: it received a bare string and had to reach back into
    /// the widget for the mode. `wrap_around` was the sharpest case -- it was stored, published, and
    /// **read by nothing at all**, not even by the dialog, because the dialog does not search.
    ///
    /// The assertion is on the payload's *contents*, and it flips a flag first, because a payload
    /// that always carried the defaults would satisfy "the options arrived" while being useless.
    #[test]
    fn a_find_request_carries_the_options_in_force() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("needle");
        dialog.set_wrap_around(false);
        dialog.show();

        let seen = Arc::new(std::sync::Mutex::new(Vec::<SearchOptions>::new()));
        let sink = Arc::clone(&seen);
        dialog.find_next_signal.connect(move |request: Arc<SearchRequest>| {
            sink.lock().expect("sink poisoned").push(request.options);
        });

        dialog.find_next();
        let captured = seen.lock().expect("lock").clone();
        assert_eq!(captured.len(), 1, "the signal fired once");
        let options = captured[0];
        assert!(
            !options.wrap_around,
            "the wrap-around toggle must travel with the request, or the host cannot honour it: \
             {options:?}"
        );
        // The other three travel with it too -- one grouped value is the point of `SearchOptions`.
        assert_eq!(
            options,
            SearchOptions {
                case_sensitive: dialog.is_match_case(),
                whole_word: dialog.is_whole_word(),
                use_regex: dialog.is_use_regex(),
                wrap_around: false,
            },
            "every flag must match the dialog's own state"
        );
    }

    #[test]
    fn find_replace_dialog_signals_replace_all() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("foo");
        dialog.set_replace_text("bar");
        dialog.show();

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.replace_all_signal.connect(move |payload: Arc<(String, String, SearchOptions)>| {
            if payload.0 == "foo" && payload.1 == "bar" {
                fired_clone.store(true, Ordering::SeqCst);
            }
        });

        dialog.replace_all();
        assert!(
            fired.load(Ordering::SeqCst),
            "replace_all_signal should fire with (find, replace)"
        );
    }

    /// A press on the painted Replace button replaces.
    ///
    /// The button was widened from 24 px to `REPLACE_BTN` so its label could be a word; the
    /// click region is derived from the same constant, so this pins the pair together. A
    /// press at the button's centre must reach `replace`, and the old 24 px position (the
    /// leading edge of the new button) must still be inside it.
    #[test]
    fn a_press_on_the_replace_button_replaces() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 240, 120));
        dialog.set_find_text("foo");
        dialog.set_replace_text("bar");
        dialog.show();

        let fired = Arc::new(AtomicBool::new(false));
        let sink = Arc::clone(&fired);
        dialog.replace_signal.connect(move |request: Arc<SearchRequest>| {
            if request.text == "bar" {
                sink.store(true, Ordering::SeqCst);
            }
        });

        let (_, replace_row) = dialog.compute_layout();
        let trailing = GAP * 2 + REPLACE_BTN as i32 * 2;
        let button_x = FindReplaceDialog::trailing_x(replace_row, trailing);
        dialog.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(
                button_x + REPLACE_BTN as i32 / 2,
                replace_row.y + replace_row.height as i32 / 2,
            ),
            button: 1,
            modifiers: 0,
        });
        assert!(fired.load(Ordering::SeqCst), "the painted Replace button must be clickable");
    }

    /// A **shown** find bar paints differently in the two appearances.
    ///
    /// The rendering census measures this control hidden, which is correct — a find bar
    /// is opened on demand, so `visible: false` is its designed initial state and the
    /// census therefore counts zero ink. That makes the census silent about whether the
    /// bar responds to a theme switch, so the property is asserted here, through the real
    /// `Draw` path, by rendering a shown instance in each appearance and comparing the
    /// output.
    ///
    /// # Why the guard covers the whole test, not just each render
    ///
    /// The appearance is process-wide, and `theme_test_guard` is the mutex that serialises
    /// tests against it. This test used to take the guard *inside* its `frame` helper, release
    /// it, and then restore the appearance after the assertion — outside any guard. The window
    /// between the helper returning and that final write was unguarded, so a test in another
    /// module that reads the global theme while rendering (for instance `meter`'s render
    /// assertions) could observe a half-switched state and fail intermittently. Holding one
    /// guard for the whole test is what makes "restore what I changed" atomic with respect to
    /// every other reader.
    #[test]
    fn a_shown_find_bar_renders_differently_in_light_and_dark() {
        let _guard = crate::style::theme_test_guard();
        {
            let mut manager = crate::style::theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.register_theme(crate::theme::Theme::dark());
        }

        fn frame(appearance: crate::style::AppearanceMode) -> String {
            crate::style::theme_manager().set_appearance(appearance);
            let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 240, 120));
            dialog.set_find_text("Sample");
            dialog.show();
            crate::theme::apply_active_theme(&mut dialog);
            crate::widget::svg::render_to_svg(&mut dialog)
        }

        let light = frame(crate::style::AppearanceMode::Light);
        let dark = frame(crate::style::AppearanceMode::Dark);
        assert_ne!(
            light, dark,
            "a theme switch must change what a shown find bar paints; identical output \
             means the chrome is hardcoded"
        );
        // Restored while the guard is still held, so no other reader can observe the light
        // appearance this test selected.
        crate::style::theme_manager().set_appearance(crate::style::AppearanceMode::Light);
    }

    #[test]
    fn find_replace_dialog_close_on_escape() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.show();
        assert!(dialog.is_visible());

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.close_signal.connect(move |_: Arc<()>| {
            fired_clone.store(true, Ordering::SeqCst);
        });

        dialog.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!dialog.is_visible());
        assert!(fired.load(Ordering::SeqCst), "close_signal should fire on Escape");
    }

    /// A disabled dialog must neither close nor act on typed input.
    ///
    /// `handle_event` gated on `visible` only, so `set_enabled(false)` left the dialog
    /// fully interactive — a suspended dialog could still be closed with Escape and
    /// still ran find/replace on Enter.
    #[test]
    fn find_replace_dialog_disabled_ignores_input() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.show();
        dialog.set_enabled(false);

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.close_signal.connect(move |_: Arc<()>| {
            fired_clone.store(true, Ordering::SeqCst);
        });

        dialog.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(dialog.is_visible(), "a disabled dialog must not close on Escape");
        assert!(!fired.load(Ordering::SeqCst), "a disabled dialog must not emit close_signal");

        // The find action must not run either.
        let find_fired = Arc::new(AtomicBool::new(false));
        let ff = Arc::clone(&find_fired);
        dialog.find_next_signal.connect(move |_: Arc<SearchRequest>| {
            ff.store(true, Ordering::SeqCst);
        });
        dialog.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(!find_fired.load(Ordering::SeqCst), "a disabled dialog must not run find");

        // Re-enabling restores the behaviour, so the gate suspends rather than locks.
        dialog.set_enabled(true);
        dialog.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!dialog.is_visible(), "re-enabling must restore Escape-to-close");
    }

    #[test]
    fn find_replace_dialog_svg_output() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("find_me");
        dialog.set_replace_text("replace_with");
        dialog.set_match_case(true);
        dialog.show();

        let svg = render_to_svg(&mut dialog);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    /// The census geometry must emit no zero-extent rect and no empty text element, and no
    /// element may leave the 240 px frame.
    ///
    /// This pins the three named defects of the old layout together, because they share one
    /// cause — the rows were laid out from the control's raw rectangle with four independent
    /// literals:
    ///
    /// * `find_replace_dialog.svg` carried `<rect x="52" y="29" width="0" height="28">`
    ///   (the find field collapsed to zero width). A zero-width rect is emitted by the SVG
    ///   backend and skipped by the rasteriser, so the two backends disagreed about the frame.
    /// * each entry field emitted `<text …></text>` when its value was empty — again an element
    ///   the rasteriser never produces.
    /// * the two arrow buttons were drawn at x 240 and x 270, past the 240 px frame.
    #[test]
    fn the_search_flags_round_trip_through_the_property_route() {
        use crate::widget::capability::WidgetProperties;

        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        for (name, setter) in [("whole_word", 0usize), ("use_regex", 1), ("highlight_all", 2)] {
            dialog
                .set(name, CapabilityValue::Bool(true))
                .unwrap_or_else(|error| panic!("{name} must be writable: {error:?}"));
            assert_eq!(dialog.get(name), Ok(CapabilityValue::Bool(true)));
            let _ = setter;
        }
        assert!(dialog.is_whole_word() && dialog.is_use_regex() && dialog.is_highlight_all());
    }

    #[test]
    fn committed_text_reaches_the_focused_field() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.show();
        dialog.handle_event(&Event::TextInput { text: "搜索".to_string() });
        assert_eq!(dialog.find_text(), "搜索");
        // The raw key fallback no longer applies once the backend speaks committed text.
        dialog.handle_event(&Event::KeyPress { key: 'x' as u32, modifiers: 0 });
        assert_eq!(dialog.find_text(), "搜索");
        dialog.handle_event(&Event::ImeCommit { text: "值".to_string() });
        assert_eq!(dialog.find_text(), "搜索值");
    }

    #[test]
    fn shift_arrows_select_and_typing_replaces_the_selection() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("abcdef");
        dialog.show();
        let index = dialog.field_index();
        dialog.caret[index] = 6;
        dialog.anchor[index] = 6;
        // Shift+Left twice selects "ef".
        dialog.handle_event(&Event::KeyPress { key: key_codes::LEFT, modifiers: 1 });
        dialog.handle_event(&Event::KeyPress { key: key_codes::LEFT, modifiers: 1 });
        assert_eq!(dialog.selected_range(index), Some((4, 6)));
        dialog.handle_event(&Event::KeyPress { key: 'X' as u32, modifiers: 0 });
        assert_eq!(dialog.find_text(), "abcdX");
        assert_eq!(dialog.caret[index], 5);
    }

    #[test]
    fn home_end_and_delete_edit_at_the_caret() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("abc");
        dialog.show();
        let index = dialog.field_index();
        dialog.handle_event(&Event::KeyPress { key: key_codes::HOME, modifiers: 0 });
        assert_eq!(dialog.caret[index], 0);
        dialog.handle_event(&Event::KeyPress { key: key_codes::DELETE, modifiers: 0 });
        assert_eq!(dialog.find_text(), "bc");
        dialog.handle_event(&Event::KeyPress { key: key_codes::END, modifiers: 0 });
        assert_eq!(dialog.caret[index], 2);
        dialog.handle_event(&Event::KeyPress { key: key_codes::BACKSPACE, modifiers: 0 });
        assert_eq!(dialog.find_text(), "b");
    }

    #[test]
    fn primary_a_selects_the_whole_field() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("needle");
        dialog.show();
        dialog.handle_event(&Event::KeyPress { key: 65, modifiers: 2 });
        let index = dialog.field_index();
        assert_eq!(dialog.selected_range(index), Some((0, 6)));
    }

    #[test]
    fn the_find_bar_emits_no_zero_extent_or_out_of_frame_ink_at_the_census_geometry() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 240, 120));
        dialog.show();
        let svg = render_to_svg(&mut dialog);

        // The defect was a **zero-extent shape**: `<rect … width="0" …>`, which the SVG backend
        // emits and the rasteriser skips. `stroke-width="0"` on a filled path is *not* that — a
        // stroke width is a paint property, and the element paints no stroke — so only the shape
        // elements are inspected, and only their own `width`/`height`.
        for element in ["<rect", "<image", "<circle", "<ellipse"] {
            for chunk in svg.split(element).skip(1) {
                let attrs = chunk.split('>').next().unwrap_or("");
                assert!(
                    !attrs.contains("width=\"0\"") && !attrs.contains("height=\"0\""),
                    "{element} has a zero-extent dimension, which the SVG backend emits and the \
                     rasteriser skips: {element}{attrs}"
                );
            }
        }
        assert!(
            !svg.contains("></text>"),
            "an empty <text> element is a silent divergence between the two backends: {svg}"
        );
        // Every x is a placement or a line endpoint; none may start or end past the frame.
        for token in
            svg.split(|c: char| !(c.is_ascii_digit() || c == '=' || c.is_ascii_alphabetic()))
        {
            if let Some(rest) = token.strip_prefix("x=") {
                if let Ok(value) = rest.parse::<i32>() {
                    assert!(value <= 240, "ink at x={value} is outside the 240 px frame: {svg}");
                }
            }
        }
    }

    #[test]
    fn tab_switches_fields_and_announces_the_focus_change() {
        use std::sync::{Arc, Mutex};

        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 320, 140));
        dialog.show();
        assert_eq!(dialog.field_index(), 0);
        let seen = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&seen);
        dialog.focus_changed.connect(move |_| {
            *sink.lock().expect("lock") += 1;
        });
        dialog.handle_event(&Event::key_press(9, 0));
        assert_eq!(dialog.field_index(), 1);
        assert_eq!(*seen.lock().expect("lock"), 1);
    }
}
