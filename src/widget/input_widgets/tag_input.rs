// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TagInput widget — a text input that creates tags/chips on Enter or comma, with removable tags.
//!
//! The TagInput presents a text field where the user types tag text, pressing Enter or comma
//! to create a tag chip. Each tag is displayed as a rounded chip with an X button for removal.
//! It emits `tags_changed` with the current list of tags on every change.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::Signal1;
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_string, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// Horizontal padding between tag chip and container edge, or between chips.
const TAG_PADDING: i32 = 6;
/// Gap between tag chips.
const TAG_GAP: i32 = 4;
/// Line height for the tag row.
const TAG_HEIGHT: i32 = 24;
/// Close button radius inside each tag chip.
const TAG_CLOSE_RADIUS: i32 = 7;
/// Corner radius of each tag chip.
const TAG_CHIP_RADIUS: u32 = 12;
/// Minimum input area width.
const MIN_INPUT_WIDTH: i32 = 60;

/// The undo payload: the tag list plus the in-progress input buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TagInputState {
    tags: Vec<String>,
    input_buffer: String,
}

/// One chip's geometry inside the painted field.
#[derive(Clone, Copy, Debug)]
struct ChipBox {
    index: usize,
    x: i32,
    y: i32,
    width: i32,
}

/// The laid-out chip row(s), shared by the painter and the two hit tests (I-P58).
#[derive(Debug)]
struct ChipLayout {
    chips: Vec<ChipBox>,
    /// Chips that did not fit, rendered as a trailing "+N" chip (I-114).
    hidden: usize,
}

struct ChipLayoutCache {
    revision: u64,
    width: u32,
    multiline: bool,
    font: Font,
    layout: Rc<ChipLayout>,
}

/// TagInput widget — a text input that creates tags/chips on Enter or comma.
pub struct TagInput {
    base: BaseWidget,
    tags: Vec<String>,
    input_buffer: String,
    // The input buffer's caret, in bytes, and the other end of a selection.
    input_cursor: usize,
    input_anchor: Option<usize>,
    /// Optional upper bound on the number of tags.
    max_tags: Option<usize>,
    /// Candidate completions for the current input.
    suggestions: Vec<String>,
    /// The tag being dragged, for reordering.
    dragging_tag: Option<usize>,
    placeholder: String,
    focused: bool,
    /// How the field's placeholder and typed text are aligned within the input area.
    ///
    /// Horizontal only: the text is centred vertically in the input band, so a `top`/`bottom`
    /// value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// The input caret's blink state, advanced by [`TagInput::tick`].
    cursor_blink: crate::style::CursorBlink,
    /// Bumped whenever the tag list changes, so the chip layout can be cached (I-P58).
    tags_revision: u64,
    /// The chip geometry the painter and both hit tests share (I-P58).
    chip_cache: RefCell<Option<ChipLayoutCache>>,
    /// Whether chips wrap onto multiple rows (I-114).
    multiline: bool,
    /// First multi-row chip row the field paints (I-114).
    chip_scroll: usize,
    /// The x of the drag insertion indicator while a chip is being dragged (I-115).
    drag_indicator: Option<i32>,
    /// The candidate row the suggestion layer highlights (I-113).
    suggestion_highlight: usize,
    /// Whether the suggestion layer is open (I-113).
    suggestions_open: bool,
    /// The IME pre-edit string rendered inline before it is committed (I-115).
    preedit: String,
    /// Emitted when the tags list changes, providing the full list of tags.
    pub tags_changed: Signal1<Vec<String>>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<TagInputState>>,
}

impl TagInput {
    /// Creates a new TagInput widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TagInput, geometry, "TagInput"),
            tags: Vec::new(),
            input_buffer: String::new(),
            input_cursor: 0,
            input_anchor: None,
            max_tags: None,
            suggestions: Vec::new(),
            dragging_tag: None,
            placeholder: "Type and press Enter\u{2026}".to_string(),
            focused: false,
            alignment: crate::core::Alignment::Left,
            cursor_blink: crate::style::CursorBlink::new(),
            tags_revision: 0,
            chip_cache: RefCell::new(None),
            multiline: false,
            chip_scroll: 0,
            drag_indicator: None,
            suggestion_highlight: 0,
            suggestions_open: false,
            preedit: String::new(),
            tags_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(TagInputState {
                tags: Vec::new(),
                input_buffer: String::new(),
            })),
        }
    }

    /// Adds a tag to the list. Trims the input and ignores empty tags.
    /// Emits `tags_changed` if a tag was actually added.
    pub fn add_tag(&mut self, tag: &str) {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.max_tags.is_some_and(|limit| self.tags.len() >= limit) {
            return;
        }
        // Avoid duplicate tags
        if self.tags.iter().any(|t| t == trimmed) {
            return;
        }
        let before = self.snapshot_state();
        self.tags.push(trimmed.to_string());
        self.bump_tags();
        self.record_state_change(before);
        self.tags_changed.emit(self.tags.clone());
        self.base.request_redraw();
    }

    /// Removes a tag at the given index.
    /// Emits `tags_changed` if a tag was actually removed.
    pub fn remove_tag(&mut self, index: usize) {
        if index < self.tags.len() {
            let before = self.snapshot_state();
            self.tags.remove(index);
            self.bump_tags();
            self.record_state_change(before);
            self.tags_changed.emit(self.tags.clone());
            self.base.request_redraw();
        }
    }

    /// Returns a slice of all current tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// The optional upper bound on the number of tags.
    pub fn max_tags(&self) -> Option<usize> {
        self.max_tags
    }

    /// Sets an upper bound on the number of tags.
    ///
    /// A limit that is already exceeded does not remove tags; it only refuses new ones, which
    /// keeps a limit write from destroying data the user can see.
    pub fn set_max_tags(&mut self, max_tags: Option<usize>) {
        if self.max_tags == max_tags {
            return;
        }
        self.max_tags = max_tags;
    }

    /// The input buffer's caret, in bytes.
    pub fn input_cursor_position(&self) -> usize {
        self.input_cursor
    }

    /// Moves the input caret.
    pub fn set_input_cursor_position(&mut self, position: usize) {
        let position = crate::widget::text_utils::floor_char_boundary(
            &self.input_buffer,
            position.min(self.input_buffer.len()),
        );
        if self.input_cursor == position && self.input_anchor.is_none() {
            return;
        }
        self.input_cursor = position;
        self.input_anchor = None;
        self.base.request_redraw();
    }

    /// The input buffer's selected byte range, if any.
    pub fn input_selection(&self) -> Option<(usize, usize)> {
        let anchor = self.input_anchor?;
        let (start, end) = if anchor <= self.input_cursor {
            (anchor, self.input_cursor)
        } else {
            (self.input_cursor, anchor)
        };
        (start != end).then_some((start, end))
    }

    /// Candidate completions for the current input.
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    /// Replaces the candidate completions.
    pub fn set_suggestions(&mut self, suggestions: Vec<String>) {
        if self.suggestions == suggestions {
            return;
        }
        self.suggestions = suggestions;
        self.base.request_redraw();
    }

    /// Completes the input buffer from the first suggestion that starts with it.
    ///
    /// Returns whether a completion was applied. The completed text is selected in full, so
    /// the next keystroke replaces it rather than appending to it.
    pub fn complete_input(&mut self) -> bool {
        let prefix = self.input_buffer.trim().to_lowercase();
        if prefix.is_empty() {
            return false;
        }
        let Some(completion) = self
            .suggestions
            .iter()
            .find(|candidate| candidate.to_lowercase().starts_with(&prefix))
            .cloned()
        else {
            return false;
        };
        let before = self.snapshot_state();
        self.input_buffer = completion;
        self.input_cursor = self.input_buffer.len();
        self.input_anchor = Some(0);
        self.record_state_change(before);
        self.base.request_redraw();
        true
    }

    /// Pastes text, creating one tag per comma- or newline-separated field.
    ///
    /// This is the batch-entry path: a user copying a list from a spreadsheet expects the
    /// paste to become tags, not one long tag containing the separators.
    pub fn paste_text(&mut self, text: &str) {
        // I-P59: one snapshot and one undo entry for the whole paste, not one per tag. The old
        // loop recorded a step for each field it added, so undoing a pasted list took as many
        // presses as the list had rows.
        let before = self.snapshot_state();
        let mut added = false;
        for part in text.split([',', '\n', ';']) {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            if self.max_tags.is_some_and(|limit| self.tags.len() >= limit) {
                break;
            }
            if self.tags.iter().any(|tag| tag == trimmed) {
                continue;
            }
            self.tags.push(trimmed.to_string());
            added = true;
        }
        if added {
            self.input_buffer.clear();
            self.input_cursor = 0;
            self.input_anchor = None;
            self.bump_tags();
            self.record_state_change(before);
            self.tags_changed.emit(self.tags.clone());
            self.base.request_redraw();
        }
    }

    /// Pastes the platform clipboard, creating one tag per separated field.
    ///
    /// This is what the primary+V chord calls. It shares [`Self::paste_text`]'s parsing and
    /// limit handling, so a clipboard paste and a programmatic paste cannot disagree.
    #[cfg(not(alloc_frugal))]
    pub fn paste_from_clipboard(&mut self) {
        let text = crate::get_clipboard_text();
        if !text.is_empty() {
            self.paste_text(&text);
        }
    }

    /// The allocation-frugal build has no clipboard integration.
    #[cfg(alloc_frugal)]
    pub fn paste_from_clipboard(&mut self) {}

    /// Copies the input selection, or the whole input buffer when nothing is selected.
    ///
    /// Returns the copied text, or `None` when the buffer is empty. Tags themselves are not
    /// copied: the input buffer is the field's editable value, and copying a chip would need a
    /// way to decide which chips a partial selection covers.
    #[cfg(not(alloc_frugal))]
    pub fn copy_input_to_clipboard(&self) -> Option<String> {
        let text = match self.input_selection() {
            Some((start, end)) => self.input_buffer.get(start..end)?.to_string(),
            None if !self.input_buffer.is_empty() => self.input_buffer.clone(),
            None => return None,
        };
        crate::set_clipboard_text(&text);
        Some(text)
    }

    /// The allocation-frugal build has no clipboard integration.
    #[cfg(alloc_frugal)]
    pub fn copy_input_to_clipboard(&self) -> Option<String> {
        None
    }

    /// Moves the tag at `from` to `to`, keeping the chips in the order the user dropped them.
    ///
    /// Returns whether the order changed.
    pub fn move_tag(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tags.len() || to >= self.tags.len() || from == to {
            return false;
        }
        let before = self.snapshot_state();
        let tag = self.tags.remove(from);
        self.tags.insert(to, tag);
        self.bump_tags();
        self.record_state_change(before);
        self.tags_changed.emit(self.tags.clone());
        self.base.request_redraw();
        true
    }

    /// Begins editing the tag at `index` by moving it into the input buffer.
    ///
    /// Returns whether the index named a tag. This is the double-click affordance: editing an
    /// existing chip is the only way to correct a tag without deleting and retyping it.
    pub fn begin_edit_tag(&mut self, index: usize) -> bool {
        if index >= self.tags.len() {
            return false;
        }
        let before = self.snapshot_state();
        let tag = self.tags.remove(index);
        self.input_buffer = tag;
        self.input_cursor = self.input_buffer.len();
        self.input_anchor = None;
        self.bump_tags();
        self.record_state_change(before);
        self.tags_changed.emit(self.tags.clone());
        self.base.request_redraw();
        true
    }

    /// Returns the placeholder shown while the input area is empty.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// How the field's placeholder and typed text are aligned within the input area.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's placeholder and typed text are aligned within the input area.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the text is centred
    /// vertically in the input band by the field's own layout — the property route refuses it
    /// through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this
    /// setter matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Sets the placeholder shown while the input area is empty.
    pub fn set_placeholder(&mut self, placeholder: &str) {
        if self.placeholder == placeholder {
            return;
        }
        self.placeholder = placeholder.to_string();
        self.base.request_redraw();
    }

    /// Clears all tags. Emits `tags_changed` if the list was non-empty.
    pub fn clear_tags(&mut self) {
        if !self.tags.is_empty() {
            let before = self.snapshot_state();
            self.tags.clear();
            self.input_buffer.clear();
            self.bump_tags();
            self.record_state_change(before);
            self.tags_changed.emit(self.tags.clone());
            self.base.request_redraw();
        }
    }

    /// Returns whether this tag input currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets the focused state.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            // The caret only blinks in a focused field, so the blink follows the same flag the
            // draw pass reads rather than a second notion of "active" that could disagree.
            if focused {
                self.cursor_blink.start();
            } else {
                self.cursor_blink.stop();
            }
            self.base.request_redraw();
            if focused {
                self.base.focus_gained.emit();
            } else {
                self.base.focus_lost.emit();
            }
        }
    }

    /// Advances the input caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention. Before it existed this control drew a solid
    /// caret and advertised a `Cursor blink interval` constant that was not defined anywhere.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.focused {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }

    /// Commits the current input buffer as a tag and clears the buffer.
    fn commit_input(&mut self) {
        let before = self.snapshot_state();
        let old_tags = self.tags.clone();
        let text = self.input_buffer.trim().to_string();
        if !text.is_empty()
            && !self.tags.iter().any(|tag| tag == &text)
            && !self.max_tags.is_some_and(|limit| self.tags.len() >= limit)
        {
            self.tags.push(text);
        }
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.input_anchor = None;
        self.bump_tags();
        self.record_state_change(before);
        if self.tags != old_tags {
            self.tags_changed.emit(self.tags.clone());
        }
        self.base.request_redraw();
    }

    /// Reverts the most recent tag/input-buffer change.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise emits `tags_changed` if the tag list actually differs.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_state();
        true
    }

    /// Reapplies the most recently undone tag/input-buffer change.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise emits `tags_changed` if the tag list actually differs.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_state();
        true
    }

    /// Returns whether there is a tag/input-buffer change to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone tag/input-buffer change to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    // ── Private helpers ──

    fn snapshot_state(&self) -> TagInputState {
        TagInputState { tags: self.tags.clone(), input_buffer: self.input_buffer.clone() }
    }

    fn record_state_change(&mut self, before: TagInputState) {
        let after = self.snapshot_state();
        if before == after {
            return;
        }
        *self.history_target.borrow_mut() = after.clone();
        self.undo_stack.push(Box::new(ValueCommand::new(
            self.history_target.clone(),
            before,
            after,
            "Edit tags",
            "tag_input_state",
        )));
    }

    fn restore_history_state(&mut self) {
        let previous_tags = self.tags.clone();
        let state = self.history_target.borrow().clone();
        self.tags = state.tags;
        self.input_buffer = state.input_buffer;
        self.input_cursor = self.input_buffer.len();
        self.input_anchor = None;
        self.bump_tags();
        if self.tags != previous_tags {
            self.tags_changed.emit(self.tags.clone());
        }
        self.base.request_redraw();
    }

    /// Moves the input caret by `delta` characters, extending the selection when asked.
    fn move_input_caret(&mut self, delta: i32, extend: bool) {
        let mut target = self.input_cursor;
        if delta < 0 {
            for _ in 0..delta.unsigned_abs() {
                target =
                    crate::widget::text_utils::previous_char_boundary(&self.input_buffer, target);
            }
        } else {
            for _ in 0..delta.unsigned_abs() {
                target = crate::widget::text_utils::next_char_boundary(&self.input_buffer, target);
            }
        }
        self.move_input_caret_to(target, extend);
    }

    /// Moves the input caret to `position`, extending the selection when asked.
    fn move_input_caret_to(&mut self, position: usize, extend: bool) {
        let position = crate::widget::text_utils::floor_char_boundary(
            &self.input_buffer,
            position.min(self.input_buffer.len()),
        );
        if extend {
            if self.input_anchor.is_none() {
                self.input_anchor = Some(self.input_cursor);
            }
        } else {
            self.input_anchor = None;
        }
        if self.input_cursor != position {
            self.input_cursor = position;
            self.base.request_redraw();
        }
    }

    fn push_input_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let before = self.snapshot_state();
        // Insert at the caret, replacing a selection. Appending unconditionally made the
        // buffer look editable while every key landed at its end.
        let (start, end) = self.input_selection().unwrap_or((self.input_cursor, self.input_cursor));
        self.input_buffer.replace_range(start..end, text);
        self.input_cursor = start + text.len();
        self.input_anchor = None;
        self.record_state_change(before);
        self.base.request_redraw();
    }

    /// The field the control actually paints.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A tag input is a text field that happens to hold chips: it is
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] tall, full width, centred in the area the
    /// caller offers, exactly as `lineedit` and the other input controls are. Painting
    /// `geometry()` made a 240x120 census cell a 240x120 box — a panel rather than a field
    /// — and, more visibly, it made the chips and their close buttons sit on a different
    /// row from the text the field shares a form with.
    ///
    /// Everything the control paints **and everything it hit-tests** is placed from this
    /// one box. The close button's hit test used to be derived from the control's
    /// rectangle while its ink was drawn from the chip row, and the two differed by the
    /// band's offset — so a click on the drawn X missed it by the height difference.
    /// Deriving both from `field_rect` is what makes hit and ink the same fact.
    fn field_rect(&self) -> Rect {
        let height = if self.multiline {
            self.multiline_field_height()
        } else {
            dimensions::TEXT_FIELD_MIN_HEIGHT
        };
        ControlMetrics::full_width_band(self.geometry(), height)
    }

    /// The height a multi-line chip layout needs, capped by the offered rectangle (I-114).
    fn multiline_field_height(&self) -> u32 {
        let rows = self.multiline_row_count();
        let needed = TAG_PADDING * 2 + rows as i32 * TAG_HEIGHT + (rows as i32 - 1) * TAG_GAP;
        (needed.max(dimensions::TEXT_FIELD_MIN_HEIGHT as i32) as u32)
            .min(self.geometry().height)
            .max(dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The number of chip rows the current tags need at the offered width (I-114).
    fn multiline_row_count(&self) -> usize {
        let available = (self.geometry().width as i32 - TAG_PADDING * 2).max(1);
        let mut x = 0i32;
        let mut rows = 1usize;
        for tag in &self.tags {
            let width = self.compute_chip_width(tag);
            if x > 0 && x + width > available {
                rows += 1;
                x = 0;
            }
            x += width + TAG_GAP;
        }
        rows
    }

    /// Whether chips wrap onto multiple rows (I-114).
    pub fn multiline(&self) -> bool {
        self.multiline
    }

    /// Enables or disables multi-row chip layout (I-114).
    ///
    /// Off by default, which keeps the single-row layout and its pixels unchanged.
    pub fn set_multiline(&mut self, multiline: bool) {
        if self.multiline == multiline {
            return;
        }
        self.multiline = multiline;
        if !multiline {
            self.chip_scroll = 0;
        }
        self.chip_cache.borrow_mut().take();
        self.base.request_redraw();
    }

    /// The trailing "+N" chip's rectangle and hidden count, when chips overflow (I-114).
    fn overflow_rect(&self) -> Option<(Rect, usize)> {
        let layout = self.chip_layout();
        if layout.hidden == 0 {
            return None;
        }
        let (x, y) = layout
            .chips
            .last()
            .map(|chip| (chip.x + chip.width + TAG_GAP, chip.y))
            .unwrap_or((self.field_rect().x + TAG_PADDING, self.chip_row_y()));
        let label = format!("+{}", layout.hidden);
        let width =
            estimate_text_width(&label, effective_font(self.style()), 1.0) as i32 + TAG_PADDING * 2;
        Some((Rect::new(x, y, width as u32, TAG_HEIGHT as u32), layout.hidden))
    }

    /// The y of the chip row inside the painted field: the row is centred on the field.
    fn chip_row_y(&self) -> i32 {
        let field = self.field_rect();
        field.y + (field.height as i32 - TAG_HEIGHT) / 2
    }

    /// Returns the close button center for a tag chip at the given pixel position.
    fn tag_close_center(&self, chip_x: i32, chip_width: i32, chip_y: i32) -> Point {
        Point::new(chip_x + chip_width - TAG_PADDING - TAG_CLOSE_RADIUS, chip_y + TAG_HEIGHT / 2)
    }

    /// Runs `f` against a throwaway measurement surface.
    fn with_measurement<R>(f: impl FnOnce(&RenderContext) -> R) -> R {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        f(&RenderContext::new(&mut measurement))
    }

    /// The chip geometry the painter, the close hit test and the chip hit test all read (I-P58).
    ///
    /// One renderer measurement per chip, cached by the tag revision and the field width, so the
    /// painter and the two hit tests cannot disagree about where a chip is or how wide it is.
    fn chip_layout(&self) -> Rc<ChipLayout> {
        let rect = self.field_rect();
        let width = rect.width;
        let font = effective_font(self.style()).clone();
        if let Some(cache) = self.chip_cache.borrow().as_ref() {
            if cache.revision == self.tags_revision
                && cache.width == width
                && cache.multiline == self.multiline
                && cache.font == font
            {
                return Rc::clone(&cache.layout);
            }
        }
        let max_x = rect.x + rect.width as i32 - TAG_PADDING;
        let mut x = rect.x + TAG_PADDING;
        let mut y = if self.multiline {
            rect.y + TAG_PADDING - (self.chip_scroll * (TAG_HEIGHT + TAG_GAP) as usize) as i32
        } else {
            self.chip_row_y()
        };
        let mut chips = Vec::new();
        let mut hidden = 0usize;
        Self::with_measurement(|context| {
            for (index, tag) in self.tags.iter().enumerate() {
                let text_width = context.measure_text(tag, &font).width as i32;
                let chip_width = text_width + TAG_PADDING * 2 + TAG_CLOSE_RADIUS * 2 + 4;
                if x + chip_width > max_x {
                    if !self.multiline {
                        hidden = self.tags.len() - index;
                        break;
                    }
                    x = rect.x + TAG_PADDING;
                    y += TAG_HEIGHT + TAG_GAP;
                    if y + TAG_HEIGHT > rect.y + rect.height as i32 - TAG_PADDING {
                        hidden = self.tags.len() - index;
                        break;
                    }
                }
                // I-114: rows scrolled above the field are neither painted nor hit-tested.
                if y + TAG_HEIGHT > rect.y + TAG_PADDING {
                    chips.push(ChipBox { index, x, y, width: chip_width });
                }
                x += chip_width + TAG_GAP;
            }
        });
        let layout = Rc::new(ChipLayout { chips, hidden });
        *self.chip_cache.borrow_mut() = Some(ChipLayoutCache {
            revision: self.tags_revision,
            width,
            multiline: self.multiline,
            font,
            layout: Rc::clone(&layout),
        });
        layout
    }

    /// Hit-tests whether a point is within the close button of any tag chip.
    /// Returns the tag index if a close button was hit, or `None`.
    fn hit_tag_close(&self, pos: Point) -> Option<usize> {
        for chip in &self.chip_layout().chips {
            let close_center = self.tag_close_center(chip.x, chip.width, chip.y);
            let dx = (pos.x - close_center.x) as i64;
            let dy = (pos.y - close_center.y) as i64;
            if dx * dx + dy * dy <= (TAG_CLOSE_RADIUS as i64 + 2) * (TAG_CLOSE_RADIUS as i64 + 2) {
                return Some(chip.index);
            }
        }
        None
    }

    /// Hit-tests whether a point is on a tag chip (not only its close button).
    fn hit_tag_index(&self, pos: Point) -> Option<usize> {
        self.chip_layout()
            .chips
            .iter()
            .find(|chip| {
                pos.x >= chip.x
                    && pos.x < chip.x + chip.width
                    && pos.y >= chip.y
                    && pos.y < chip.y + TAG_HEIGHT
            })
            .map(|chip| chip.index)
    }

    /// Computes the width of a tag chip based on its text content.
    ///
    /// Measured, not estimated from the byte count: `tag.len() * 7` under-measured CJK and wide
    /// Latin glyphs, so the label ran under the close disc, and it counted UTF-8 bytes rather than
    /// characters (`"日本"` is 6 bytes and 2 glyphs). The same ruler the painter uses keeps the
    /// chip and its label in agreement.
    fn compute_chip_width(&self, tag: &str) -> i32 {
        let text_width = estimate_text_width(tag, effective_font(self.style()), 1.0) as i32;
        text_width + TAG_PADDING * 2 + TAG_CLOSE_RADIUS * 2 + 4
    }

    /// Marks the tag list as changed so the shared chip layout is rebuilt (I-P58).
    fn bump_tags(&mut self) {
        self.tags_revision = self.tags_revision.wrapping_add(1);
        self.chip_cache.borrow_mut().take();
    }

    /// The suggestions that match the current input as a case-insensitive prefix (I-113).
    fn visible_suggestions(&self) -> Vec<usize> {
        if self.input_buffer.trim().is_empty() {
            return Vec::new();
        }
        let prefix = self.input_buffer.trim().to_lowercase();
        self.suggestions
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.to_lowercase().starts_with(&prefix))
            .map(|(index, _)| index)
            .collect()
    }

    /// Puts the highlighted suggestion into the input buffer (I-113).
    fn accept_highlighted(&mut self) -> bool {
        let matches = self.visible_suggestions();
        let Some(&index) = matches.get(self.suggestion_highlight) else {
            return false;
        };
        self.input_buffer = self.suggestions[index].clone();
        self.input_cursor = self.input_buffer.len();
        // The completed text is selected in full (I-113), matching `complete_input`, so the
        // next keystroke replaces it rather than appending to it.
        self.input_anchor = Some(0);
        self.suggestions_open = false;
        self.suggestion_highlight = 0;
        self.base.request_redraw();
        true
    }

    /// Opens or closes the suggestion layer from the current filter (I-113).
    fn refresh_suggestions(&mut self) {
        let count = self.visible_suggestions().len();
        self.suggestions_open = count > 0 && self.focused;
        if count == 0 {
            self.suggestion_highlight = 0;
        } else if self.suggestion_highlight >= count {
            self.suggestion_highlight = count - 1;
        }
    }
}

impl Widget for TagInput {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        let height = if self.multiline { self.multiline_field_height().max(28) } else { 28 };
        crate::core::Size::new(200, height)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    // The caret blink is driven through the trait so the animation bus reaches it.
    fn tick(&mut self, delta_ms: u32) -> bool {
        TagInput::tick(self, delta_ms)
    }

    /// Whether the caret is cycling, i.e. whether a frame is owed.
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host consults it to
    /// decide whether a frame is needed **before** paying for a sweep. A focused tag field whose caret
    /// is mid-blink answered `false`, so such a host would stop scheduling frames between two blinks
    /// and the caret would stop cycling — the same unreachable animation the trait `tick` above exists
    /// to end, one layer up. The condition mirrors [`TagInput::tick`]: the caret runs exactly while the
    /// field holds focus.
    fn is_animating(&self) -> bool {
        self.focused
    }
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "tags_changed" => {
                Some(EventSignalRef::mapped("tags_changed", &self.tags_changed, |v| {
                    // The declared shape is `ListScalar` of `String`, so the payload arrives as the
                    // tag list rather than its `Debug` spelling — a subscriber can read each tag
                    // as its own string.
                    CapabilityValue::Tuple(
                        v.iter().map(|tag| CapabilityValue::String(tag.clone())).collect(),
                    )
                }))
            }
            _ => None,
        }
    }
}

/// `TagInput`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The tag list is published as
/// the comma-joined string the schema declares and is read-only: there is no
/// writer for a collection through a single string, and the legacy write side had
/// no `tags` arm either, so `set` refuses the name rather than inventing a parse.
impl WidgetProperties for TagInput {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tags" => Ok(CapabilityValue::String(self.tags().join(","))),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            // I-114: the multi-row layout switch.
            "multiline" => Ok(CapabilityValue::Bool(self.multiline())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "tags" => Err(CapabilityAccessError::ReadOnlyProperty),
            "placeholder" => {
                self.set_placeholder(&expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "multiline" => {
                self.set_multiline(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["tags", "placeholder", "alignment", "multiline", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `tag_input` publishes.
    ///
    /// `add_tag` and `remove_tag` name a tag, so a bare command has nothing to act on:
    /// there is no "current tag" for either to fall back to, and inventing one would
    /// mean the command silently added or deleted a tag the caller never named. They are
    /// therefore answered through the property route. `set_placeholder` is the same
    /// shape — it assigns the placeholder string, `set` already carries the arm, and the
    /// `placeholder` property is published. A caller reaches all three through `set`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_tag" | "remove_tag" | "set_placeholder" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for TagInput {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **field**, not the control's rectangle: see `field_rect`. Every measurement
        // below — the fill, the border, the chip row and the input area — is taken from this
        // one box, so none of them can drift away from the others.
        let rect = self.field_rect();
        let is_enabled = self.base.is_enabled();

        // ── Background ──
        // Resolved from the style so a theme switch reaches this control; the three-state
        // ladder is *derived* from the resolved colour rather than being three fixed greys, so
        // the states stay distinguishable and the control follows the theme.
        let style = self.style();
        let themed = crate::style::resolved_theme_style_shared("tag_input");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        let accent = style.border_color.or(themed_border);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(235, 235, 235, 200));
        // The page this field sits in, read once: a disabled fill recedes toward it.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let bg_color = if !is_enabled {
            // A disabled **field** recedes toward the page. It read `base_bg.blend(&Color::WHITE,
            // 0.35)`, and white is not a direction: on a dark field that moves the disabled fill
            // *away* from the window, so the disabled state was the more prominent of the two.
            self.base.disabled_surface_near(base_bg, window_fill)
        } else if self.focused {
            base_bg.blend(&accent.unwrap_or(Color::rgba(60, 140, 255, 200)), 0.18)
        } else {
            base_bg
        };
        // The field's own stadium: the radius is half its height. A literal `6` was written
        // for a 28 px field and becomes an almost-square corner on the 48 px field every
        // other input in the crate shares, so the fill and the border below could not even
        // agree with each other — the two edges crossed at the corners.
        let field_radius = rect.height / 2;
        context.fill_rounded_rect(rect, field_radius, bg_color);

        // — Focus border —
        if self.focused && is_enabled {
            context.draw_rounded_rect_stroke(
                rect,
                field_radius,
                accent.unwrap_or(Color::rgba(60, 140, 255, 200)),
                2,
            );
        } else {
            context.draw_rounded_rect_stroke(
                rect,
                field_radius,
                style.border_color.unwrap_or(Color::rgba(200, 200, 200, 160)),
                1,
            );
        }

        // I-P58: one cached layout, shared with both hit tests, instead of a second estimate in
        // the painter and a third measurement of the same label below.
        let layout = self.chip_layout();
        let chip_y = self.chip_row_y();
        let font = effective_font(style);
        let mut last_x = rect.x + TAG_PADDING;
        let mut last_y = chip_y;

        // ── Draw tag chips ──
        for chip in &layout.chips {
            let tag = &self.tags[chip.index];
            let chip_width = chip.width;
            let chip_rect = Rect::new(chip.x, chip.y, chip_width as u32, TAG_HEIGHT as u32);

            // Chip background
            // The chip is this control's own chrome — a filled pill holding a tag — so its
            // fill follows the resolved accent rather than a fixed Material blue. The close
            // button is then the chip's *inverse* (white circle, accent X), which is what a
            // "remove this" affordance looks like on a coloured pill; deriving it keeps the
            // pair legible whatever the theme's accent is.
            // I-115: the dragged chip is highlighted so the gesture has a visible source.
            let chip_bg = if self.dragging_tag == Some(chip.index) {
                accent.unwrap_or(Color::rgb(25, 118, 210)).with_alpha(150)
            } else if is_enabled {
                accent.unwrap_or(Color::rgb(25, 118, 210))
            } else {
                Color::rgba(180, 180, 180, 160)
            };
            context.fill_rounded_rect(chip_rect, TAG_CHIP_RADIUS, chip_bg);

            // Chip text: the contrasting ink for the fill, so it stays legible when the
            // theme's accent is light.
            let text_color = chip_bg.contrast_color();
            let text_x = chip.x + TAG_PADDING;
            // The origin is the glyph box's top edge, so the row's centre is half the
            // difference of the line boxes — `chip_y + TAG_HEIGHT/2` began the box half a
            // line below the chip's middle.
            let tag_metrics = context.measure_text(tag, font);
            let text_origin =
                Point::new(text_x, chip.y + (TAG_HEIGHT - tag_metrics.height as i32) / 2);
            context.draw_text(text_origin, tag, font, text_color, HorizontalAlignment::Left);

            // Close button circle. It is the chip's *contrast* colour rather than white: on a
            // light-accent theme a white circle on the chip is a low-contrast disc, and the
            // "remove this" affordance has to read on whatever accent the theme supplies.
            let close_center = self.tag_close_center(chip.x, chip_width, chip.y);
            let close_bg = text_color;
            context.fill_circle(close_center, TAG_CLOSE_RADIUS as u32, close_bg);

            // X mark (two diagonal lines)
            let x_offset = (TAG_CLOSE_RADIUS as f32 * 0.45) as i32;
            let close_fg = chip_bg;
            context.draw_line(
                Point::new(close_center.x - x_offset, close_center.y - x_offset),
                Point::new(close_center.x + x_offset, close_center.y + x_offset),
                close_fg,
            );
            context.draw_line(
                Point::new(close_center.x + x_offset, close_center.y - x_offset),
                Point::new(close_center.x - x_offset, close_center.y + x_offset),
                close_fg,
            );

            last_x = chip.x + chip_width + TAG_GAP;
            last_y = chip.y;
        }

        // I-115: the insertion line marks where the dragged chip will land.
        if self.dragging_tag.is_some() {
            let x = self.drag_indicator.unwrap_or(last_x);
            context.draw_line(
                Point::new(x, last_y),
                Point::new(x, last_y + TAG_HEIGHT),
                accent.unwrap_or(Color::rgb(25, 118, 210)),
            );
        }

        // I-114: the chips that do not fit are summarised in a trailing "+N" chip instead of
        // silently disappearing. Clicking it expands the field into its multi-row layout.
        if let Some((overflow, hidden)) = self.overflow_rect() {
            let chip_bg = accent.unwrap_or(Color::rgb(120, 120, 120));
            context.fill_rounded_rect(overflow, TAG_CHIP_RADIUS, chip_bg);
            context.draw_text(
                Point::new(overflow.x + TAG_PADDING, overflow.y + 4),
                &format!("+{hidden}"),
                font,
                chip_bg.contrast_color(),
                HorizontalAlignment::Left,
            );
            last_x = overflow.x + overflow.width as i32 + TAG_GAP;
            last_y = overflow.y;
        }

        // ── Input area ──
        let input_x = last_x.min(rect.x + rect.width as i32 - MIN_INPUT_WIDTH);
        let input_width = (rect.x + rect.width as i32 - TAG_PADDING - input_x).max(0);
        let input_rect = Rect::new(input_x, last_y, input_width as u32, TAG_HEIGHT as u32);

        // Input background (slightly inset).
        //
        // The field is a band *of* the tag box, so it is one step from the resolved background
        // rather than a white wash: the literal `rgba(255,255,255,180)` was a light-theme
        // assumption written as a colour, and on the dark appearance it bleached the box's own
        // panel. The same mistake on the inks made the placeholder — which is the only thing
        // the field shows when it is empty — measure 1.25:1 against the field it sits on.
        let base_ink = self
            .style()
            .text_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
            .unwrap_or_else(|| bg_color.contrast_color());
        let input_bg = if !is_enabled {
            bg_color.blend(&base_ink, 0.04)
        } else {
            bg_color.blend(&base_ink, 0.08)
        };
        context.fill_rounded_rect(input_rect, 4, input_bg);

        // Input text. The typed text is the field's ink; the placeholder is that same ink
        // dimmed, and both are pushed clear of the field so neither can disappear into it.
        let typed_color = base_ink.legible_on(input_bg, 4.5);
        let hint_color = typed_color.blend(&input_bg, 0.35).legible_on(input_bg, 4.5);
        let input_text_color = if !is_enabled {
            // `legible_on` is applied **last**, once the colour is final. It used to be applied to
            // `typed_color` and then blended away again by 0.6 toward the field, which undid the
            // guarantee: the blend moved the ink most of the way back to the surface the
            // legibility check had just stepped it away from. Applying it after the recede means
            // the disabled placeholder is muted *and* legible, on both appearances.
            typed_color
                .blend(&input_bg, crate::widget::BaseWidget::disabled_ink_weight())
                .legible_on(input_bg, 4.5)
        } else if !self.input_buffer.is_empty() {
            typed_color
        } else {
            hint_color
        };
        let display_text = if self.input_buffer.is_empty() && self.tags.is_empty() {
            self.placeholder.as_str()
        } else if self.input_buffer.is_empty() {
            ""
        } else {
            &self.input_buffer
        };
        // The placeholder and the typed text are fitted to the input area, and the caret is
        // positioned from the **fitted** text's measured width. `input_buffer.len() * 7`
        // was wrong twice: `len()` counts bytes, so a CJK tag moved the caret four times too
        // far, and the advance the renderer uses is the font size, not 7. A caret that
        // disagrees with the glyphs beside it is a visible defect on its own.
        let input_band =
            Rect::new(input_x + 4, last_y, input_rect.width.saturating_sub(8), TAG_HEIGHT as u32);
        // The placeholder/typed text goes in the input band's own line box, not at the band's
        // top edge. `draw_text_fitted` aligns horizontally only — its origin is `bounds.y`
        // unchanged — so handing it the chip-height band pinned the text to the top of the
        // field: `tag_input.svg` put a 14 px line at `y = 4` inside a band running `4..28`.
        // The chip labels in this same file already centred themselves correctly; the field
        // was the one place that had not been brought in line.
        let input_line = context.text_line(input_band, font);
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        // Selection band behind the buffer's selected range.
        if let Some((start, end)) = self.input_selection() {
            let advance = context.measure_text(display_text, font).width as i32;
            let origin = crate::render::fitted_origin(input_line, advance, value_align);
            let start_x =
                origin.x + context.measure_text(&self.input_buffer[..start], font).width as i32;
            let end_x =
                origin.x + context.measure_text(&self.input_buffer[..end], font).width as i32;
            context.fill_rect(
                Rect::new(
                    start_x.min(end_x),
                    input_line.y,
                    (end_x - start_x).unsigned_abs(),
                    input_line.height.max(1),
                ),
                Color::rgba(60, 140, 255, 70),
            );
        }
        let drawn = context.draw_text_fitted_cow(
            input_line,
            display_text,
            font,
            input_text_color,
            value_align,
        );

        // ── Cursor (when focused and input is active) ──
        if self.focused && is_enabled && self.cursor_blink.is_visible() {
            // The caret follows the buffer's own caret, measured from the fitted origin so a
            // centred buffer keeps its caret on the glyphs it belongs to.
            let advance = context.measure_text(display_text, font).width as i32;
            let origin = crate::render::fitted_origin(input_line, advance, value_align);
            let prefix = if self.input_buffer.is_empty() {
                &drawn[..]
            } else {
                let caret = crate::widget::text_utils::floor_char_boundary(
                    &self.input_buffer,
                    self.input_cursor,
                );
                &self.input_buffer[..caret]
            };
            let cursor_x = origin.x + context.measure_text(prefix, font).width as i32;
            let cursor_y1 = input_line.y;
            let cursor_y2 = input_line.y + input_line.height as i32;
            // Clamped to the input box: a caret at the far right of a full buffer belongs on
            // the last pixel of the field, not past its edge.
            let cursor_x = cursor_x.min(input_rect.x + input_rect.width as i32 - 1);
            context.draw_line(
                Point::new(cursor_x, cursor_y1),
                Point::new(cursor_x, cursor_y2),
                Color::rgba(60, 140, 255, 200),
            );
            // I-115: the IME composition string is painted inline with an underline until it is
            // committed, so an East-Asian user can see what is being composed.
            if !self.preedit.is_empty() {
                let preedit_x = cursor_x + 2;
                context.draw_text(
                    Point::new(preedit_x, cursor_y1),
                    &self.preedit,
                    font,
                    input_text_color,
                    HorizontalAlignment::Left,
                );
                context.draw_line(
                    Point::new(preedit_x, cursor_y2 - 1),
                    Point::new(
                        preedit_x + context.measure_text(&self.preedit, font).width as i32,
                        cursor_y2 - 1,
                    ),
                    input_text_color,
                );
            }
        }

        // ── Suggestion layer (I-113) ──
        if self.focused && self.suggestions_open {
            let matches = self.visible_suggestions();
            if !matches.is_empty() {
                let row_height = 20i32;
                let max_visible = 6usize;
                let highlight = self.suggestion_highlight.min(matches.len() - 1);
                let start = highlight.saturating_sub(max_visible - 1);
                let visible = matches.len().min(max_visible);
                let layer = Rect::new(
                    rect.x,
                    rect.y + rect.height as i32 + 2,
                    rect.width,
                    (visible as i32 * row_height) as u32,
                );
                context.fill_rect(layer, base_bg);
                context.draw_rect(layer, accent.unwrap_or(Color::rgb(180, 180, 180)));
                for (row, index) in matches.iter().skip(start).take(max_visible).enumerate() {
                    let y = layer.y + row as i32 * row_height;
                    let selected = start + row == highlight;
                    if selected {
                        context.fill_rect(
                            Rect::new(
                                layer.x + 1,
                                y,
                                layer.width.saturating_sub(2),
                                row_height as u32,
                            ),
                            input_bg.blend(&accent.unwrap_or(Color::rgb(60, 140, 255)), 0.25),
                        );
                    }
                    context.draw_text(
                        Point::new(layer.x + 6, y + 2),
                        &self.suggestions[*index],
                        font,
                        base_ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

impl EventHandler for TagInput {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                // I-114: the "+N" summary expands the field into its multi-row layout.
                if let Some((overflow, _)) = self.overflow_rect() {
                    if overflow.contains_point(*pos) {
                        self.set_multiline(true);
                        return;
                    }
                }
                // Check if the click is on a tag's close button
                if let Some(tag_index) = self.hit_tag_close(*pos) {
                    self.remove_tag(tag_index);
                    return;
                }
                // A press on a chip begins a reorder drag; releasing on another chip moves it.
                if let Some(tag_index) = self.hit_tag_index(*pos) {
                    self.dragging_tag = Some(tag_index);
                }
                // Focus is gained only from a press on the **painted field**. Testing the
                // control's rectangle instead would let a click on the empty space below a
                // 48 px field in a 120 px cell focus the control, and the field the user
                // was told they had focused is the one they can see.
                if self.field_rect().contains_point(*pos) {
                    self.set_focused(true);
                }
            }
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                // Double-clicking a chip edits it: its text returns to the input buffer.
                if let Some(index) = self.hit_tag_index(*pos) {
                    self.begin_edit_tag(index);
                }
            }
            Event::MouseMove { pos } => {
                // I-115: while a chip is dragged, the insertion line follows the pointer.
                if self.dragging_tag.is_some() {
                    self.drag_indicator = Some(pos.x);
                    self.base.request_redraw();
                }
            }
            Event::Wheel { delta, modifiers, .. } => {
                // I-114: a multi-row chip area scrolls by whole rows.
                if self.multiline {
                    let scroll =
                        crate::event::wheel::WheelScroll::from_event(*delta, *modifiers, 1);
                    let rows = self.multiline_row_count();
                    let visible = ((self.field_rect().height - TAG_PADDING as u32 * 2
                        + TAG_GAP as u32)
                        / (TAG_HEIGHT + TAG_GAP) as u32)
                        .max(1) as usize;
                    let max_scroll = rows.saturating_sub(visible);
                    let next = (self.chip_scroll as i64 - scroll.lines_y as i64)
                        .clamp(0, max_scroll as i64) as usize;
                    if next != self.chip_scroll {
                        self.chip_scroll = next;
                        self.chip_cache.borrow_mut().take();
                        self.base.request_redraw();
                    }
                }
            }
            Event::MouseRelease { pos, button } if *button == 1 => {
                self.drag_indicator = None;
                if let Some(from) = self.dragging_tag.take() {
                    if let Some(to) = self.hit_tag_index(*pos) {
                        self.move_tag(from, to);
                    }
                }
                self.base.request_redraw();
            }
            Event::MouseLeave { .. } => {
                self.dragging_tag = None;
                self.drag_indicator = None;
            }
            Event::FocusGained { .. } => {
                self.set_focused(true);
            }
            Event::FocusLost => {
                // Commit any pending input on focus loss
                if !self.input_buffer.is_empty() {
                    self.commit_input();
                }
                self.set_focused(false);
            }
            Event::KeyPress { key, modifiers } => {
                if !self.focused {
                    return;
                }
                // Undo and redo are the platform primary chord: Control+Z/Y on
                // Windows and Linux, Command+Z/Y on macOS. A shifted chord is a
                // different binding and stays unbound here.
                if primary_shortcut_pressed(*key, *modifiers, Key::Z) {
                    let _ = self.undo();
                    return;
                }
                if primary_shortcut_pressed(*key, *modifiers, Key::Y) {
                    let _ = self.redo();
                    return;
                }
                // Clipboard: paste turns a separated list into tags in one gesture, and copy
                // puts the input buffer (or its selection) back on the clipboard.
                if primary_shortcut_pressed(*key, *modifiers, Key::V) {
                    self.paste_from_clipboard();
                    return;
                }
                if primary_shortcut_pressed(*key, *modifiers, Key::C) {
                    let _ = self.copy_input_to_clipboard();
                    return;
                }
                // I-115: Primary+A selects the input buffer (the chips are not text).
                if primary_shortcut_pressed(*key, *modifiers, Key::A) {
                    self.input_anchor = Some(0);
                    self.input_cursor = self.input_buffer.len();
                    self.base.request_redraw();
                    return;
                }
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                // I-113: the open suggestion layer owns Up/Down/Enter/Tab/Escape.
                if self.suggestions_open {
                    let count = self.visible_suggestions().len();
                    match *key {
                        38 => {
                            if self.suggestion_highlight > 0 {
                                self.suggestion_highlight -= 1;
                                self.base.request_redraw();
                            }
                            return;
                        }
                        40 => {
                            if count > 0 && self.suggestion_highlight + 1 < count {
                                self.suggestion_highlight += 1;
                                self.base.request_redraw();
                            }
                            return;
                        }
                        13 => {
                            if self.accept_highlighted() {
                                self.commit_input();
                            }
                            return;
                        }
                        9 => {
                            let _ = self.accept_highlighted();
                            return;
                        }
                        27 => {
                            self.suggestions_open = false;
                            self.base.request_redraw();
                            return;
                        }
                        _ => {}
                    }
                }
                match *key {
                    8 => {
                        // Backspace removes the selection or the character before the caret;
                        // at the start of the buffer it removes the last tag.
                        let range = self.input_selection().or_else(|| {
                            (self.input_cursor > 0).then(|| {
                                (
                                    crate::widget::text_utils::previous_char_boundary(
                                        &self.input_buffer,
                                        self.input_cursor,
                                    ),
                                    self.input_cursor,
                                )
                            })
                        });
                        if let Some((start, end)) = range {
                            let before = self.snapshot_state();
                            self.input_buffer.replace_range(start..end, "");
                            self.input_cursor = start;
                            self.input_anchor = None;
                            self.record_state_change(before);
                            self.base.request_redraw();
                        } else if !self.tags.is_empty() {
                            let before = self.snapshot_state();
                            self.tags.pop();
                            self.record_state_change(before);
                            self.tags_changed.emit(self.tags.clone());
                            self.base.request_redraw();
                        }
                    }
                    127 => {
                        // Delete removes the selection or the character after the caret.
                        if let Some((start, end)) = self.input_selection() {
                            let before = self.snapshot_state();
                            self.input_buffer.replace_range(start..end, "");
                            self.input_cursor = start;
                            self.input_anchor = None;
                            self.record_state_change(before);
                            self.base.request_redraw();
                        } else if self.input_cursor < self.input_buffer.len() {
                            let before = self.snapshot_state();
                            let end = crate::widget::text_utils::next_char_boundary(
                                &self.input_buffer,
                                self.input_cursor,
                            );
                            self.input_buffer.replace_range(self.input_cursor..end, "");
                            self.record_state_change(before);
                            self.base.request_redraw();
                        }
                    }
                    37 => self.move_input_caret(-1, shift),
                    39 => self.move_input_caret(1, shift),
                    36 => self.move_input_caret_to(0, shift),
                    35 => self.move_input_caret_to(self.input_buffer.len(), shift),
                    13 => {
                        // Enter — commit the current input as a tag
                        self.commit_input();
                    }
                    44 => {
                        // Comma — commit the current input as a tag
                        self.commit_input();
                    }
                    9 => {
                        // Tab accepts the first matching suggestion.
                        self.complete_input();
                        self.refresh_suggestions();
                    }
                    38 | 40 => {
                        // Vertical movement belongs to the suggestion layer; with no layer open
                        // these keys are not text and must not become `&`/`(`.
                    }
                    27 => {
                        // Escape — lose focus
                        self.set_focused(false);
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.push_input_text(&ch.to_string());
                                self.refresh_suggestions();
                            }
                        }
                    }
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if self.focused {
                    self.preedit.clear();
                    self.push_input_text(text);
                    self.refresh_suggestions();
                }
            }
            Event::ImePreedit { text, .. } => {
                // I-115: composition text is shown but not committed to the buffer.
                self.preedit = text.clone();
                self.base.request_redraw();
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::input_widgets::shortcut_editor::primary_modifier_event_bits;
    use std::sync::{Arc, Mutex};

    #[test]
    fn tag_input_default_creation() {
        let ti = TagInput::new(Rect::new(0, 0, 300, 36));
        assert!(ti.tags().is_empty());
        assert!(!ti.is_focused());
        assert_eq!(ti.kind(), WidgetKind::TagInput);
        assert!(ti.is_enabled());
    }

    /// `is_animating` agrees with `tick`: a tag field owes a frame exactly while it holds focus.
    ///
    /// # The defect this pins
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host consults the
    /// answer to decide whether a frame is needed **before** paying for a sweep. A focused tag field
    /// whose caret was mid-blink answered `false`, so such a host would stop scheduling frames between
    /// two blinks and the caret would stop cycling — the same unreachable animation the trait `tick`
    /// was lifted for, one layer up.
    ///
    /// # Why it is asserted against `tick`
    ///
    /// The two answers describe one animation, so a version that disagreed with `tick` in either
    /// direction would be wrong whichever value it picked: `false` while ticking freezes the caret,
    /// `true` while settled pins the host at frame rate. Comparing them makes a drift impossible.
    #[test]
    fn is_animating_agrees_with_tick() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        assert!(!ti.is_animating(), "an unfocused field owes no frames");

        ti.set_focused(true);
        assert!(ti.is_animating(), "a focused field animates its caret");

        ti.set_focused(false);
        assert!(!ti.is_animating(), "blurring settles the caret");
        assert_eq!(ti.is_animating(), ti.tick(0));
    }

    #[test]
    fn tag_input_add_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("rust");
        assert_eq!(ti.tags(), &["rust"]);
    }

    #[test]
    fn tag_input_add_tag_trims_whitespace() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("  hello  ");
        assert_eq!(ti.tags(), &["hello"]);
    }

    #[test]
    fn tag_input_ignores_empty_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("");
        ti.add_tag("  ");
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_add_tag_emits_signal() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        let captured = Arc::new(Mutex::new(None::<Vec<String>>));
        ti.tags_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<Vec<String>>| {
                *captured.lock().unwrap() = Some(val.to_vec());
            }
        });

        ti.add_tag("rust");
        assert_eq!(*captured.lock().unwrap(), Some(vec!["rust".to_string()]));
    }

    #[test]
    fn tag_input_remove_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");
        ti.add_tag("gamma");
        assert_eq!(ti.tags().len(), 3);

        ti.remove_tag(1); // remove "beta"
        assert_eq!(ti.tags(), &["alpha", "gamma"]);
    }

    #[test]
    fn tag_input_remove_tag_emits_signal() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");

        let count = Arc::new(Mutex::new(0));
        ti.tags_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<Vec<String>>| {
                *count.lock().unwrap() += 1;
            }
        });

        ti.remove_tag(0);
        assert_eq!(*count.lock().unwrap(), 1);
        assert_eq!(ti.tags(), &["beta"]);
    }

    #[test]
    fn tag_input_remove_tag_out_of_bounds() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("only");
        ti.remove_tag(5); // out of bounds — no-op
        assert_eq!(ti.tags().len(), 1);
    }

    #[test]
    fn tag_input_clear_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("a");
        ti.add_tag("b");
        ti.clear_tags();
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_clear_tags_emits_signal() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("data");

        let count = Arc::new(Mutex::new(0));
        ti.tags_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<Vec<String>>| {
                *count.lock().unwrap() += 1;
            }
        });

        ti.clear_tags();
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn tag_input_clear_empty_does_not_emit() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        let count = Arc::new(Mutex::new(0));
        ti.tags_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<Vec<String>>| {
                *count.lock().unwrap() += 1;
            }
        });

        ti.clear_tags();
        assert_eq!(*count.lock().unwrap(), 0);
    }

    #[test]
    fn tag_input_keyboard_typing() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 104, modifiers: 0 }); // 'h'
        ti.handle_event(&Event::KeyPress { key: 105, modifiers: 0 }); // 'i'
        assert_eq!(ti.input_buffer, "hi");
    }

    #[test]
    fn tag_input_enter_creates_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 114, modifiers: 0 }); // 'r'
        ti.handle_event(&Event::KeyPress { key: 117, modifiers: 0 }); // 'u'
        ti.handle_event(&Event::KeyPress { key: 115, modifiers: 0 }); // 's'
        ti.handle_event(&Event::KeyPress { key: 116, modifiers: 0 }); // 't'
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter

        assert_eq!(ti.tags(), &["rust"]);
        assert!(ti.input_buffer.is_empty());
    }

    #[test]
    fn tag_input_comma_creates_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 103, modifiers: 0 }); // 'g'
        ti.handle_event(&Event::KeyPress { key: 111, modifiers: 0 }); // 'o'
        ti.handle_event(&Event::KeyPress { key: 44, modifiers: 0 }); // Comma

        assert_eq!(ti.tags(), &["go"]);
        assert!(ti.input_buffer.is_empty());
    }

    #[test]
    fn tag_input_backspace_removes_last_char_or_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        // Type "ab" then backspace removes 'b'
        ti.handle_event(&Event::KeyPress { key: 97, modifiers: 0 }); // 'a'
        ti.handle_event(&Event::KeyPress { key: 98, modifiers: 0 }); // 'b'
        ti.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert_eq!(ti.input_buffer, "a");

        // Commit and test backspace removes last tag
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter
        assert_eq!(ti.tags(), &["a"]);

        // Backspace when buffer empty — removes last tag
        ti.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_undo_redo_restores_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");

        assert!(ti.can_undo());
        assert!(ti.undo());
        assert_eq!(ti.tags(), &["alpha"]);
        assert!(ti.can_redo());
        assert!(ti.redo());
        assert_eq!(ti.tags(), &["alpha", "beta"]);
    }

    #[test]
    fn tag_input_undo_redo_restores_input_buffer() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);
        ti.handle_event(&Event::text_input("你好"));
        assert_eq!(ti.input_buffer, "你好");

        let primary = primary_modifier_event_bits();
        ti.handle_event(&Event::KeyPress { key: 90, modifiers: primary });
        assert_eq!(ti.input_buffer, "");
        ti.handle_event(&Event::KeyPress { key: 89, modifiers: primary });
        assert_eq!(ti.input_buffer, "你好");
    }

    #[test]
    fn tag_input_commit_history_restores_complete_state() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);
        ti.handle_event(&Event::text_input("rust"));
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(ti.tags(), &["rust"]);
        assert_eq!(ti.input_buffer, "");

        assert!(ti.undo());
        assert!(ti.tags().is_empty());
        assert_eq!(ti.input_buffer, "rust");
    }

    #[test]
    fn tag_input_focus_on_click() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        assert!(!ti.is_focused());

        ti.handle_event(&Event::MousePress { pos: Point::new(50, 18), button: 1, modifiers: 0 });
        assert!(ti.is_focused());
    }

    #[test]
    fn tag_input_focus_lost_on_escape() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);
        assert!(ti.is_focused());

        ti.handle_event(&Event::KeyPress { key: 27, modifiers: 0 }); // Escape
        assert!(!ti.is_focused());
    }

    #[test]
    fn tag_input_disabled_blocks_events() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_enabled(false);

        ti.handle_event(&Event::MousePress { pos: Point::new(50, 18), button: 1, modifiers: 0 });
        assert!(!ti.is_focused());
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_avoid_duplicate_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("rust");
        ti.add_tag("rust"); // duplicate — should be ignored
        assert_eq!(ti.tags().len(), 1);
    }

    #[test]
    fn tag_input_multiple_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");
        ti.add_tag("gamma");
        assert_eq!(ti.tags(), &["alpha", "beta", "gamma"]);
    }

    #[test]
    fn tag_input_svg_output() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("rust");
        ti.add_tag("gui");

        let svg = crate::widget::svg::render_to_svg(&mut ti);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"36\""));
    }

    #[test]
    fn tag_input_svg_empty() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        let svg = crate::widget::svg::render_to_svg(&mut ti);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn tag_input_close_click_removes_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("removable");

        // The expected close-button position is derived from the **painted field**, which is
        // where the ink is. This used to be computed from `ti.geometry()` plus a second
        // vertical padding literal, so the test agreed with the old hit test by construction
        // — and both were wrong together: the circle was drawn on the chip row while the hit
        // test guessed at the control's top edge, so a click on the drawn X could miss it.
        // Pinning the painted geometry is what makes this test able to catch that.
        let field = ti.field_rect();
        let chip_x = field.x + TAG_PADDING;
        let chip_y = ti.chip_row_y();
        let chip_width =
            ti.compute_chip_width("removable").min(field.width as i32 - TAG_PADDING * 2);
        let close_center = ti.tag_close_center(chip_x, chip_width, chip_y);

        // Click on the close button
        ti.handle_event(&Event::MousePress { pos: close_center, button: 1, modifiers: 0 });
        assert!(ti.tags().is_empty());
    }

    /// The chip row and its close buttons are placed from the **painted field**.
    ///
    /// The close button's hit test was derived from the control's rectangle while its ink
    /// came from the chip row, so the two sat 36 px apart in a 120 px cell: the circle a user
    /// saw was not the one that answered. Both now come from `chip_row_y()`, and this pins
    /// the drawn circle against the same derivation the hit test uses.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_close_circle_is_hit_where_it_is_drawn() {
        let mut ti = TagInput::new(Rect::new(0, 0, 240, 120));
        ti.add_tag("removable");

        let field = ti.field_rect();
        let chip_x = field.x + TAG_PADDING;
        let chip_width =
            ti.compute_chip_width("removable").min(field.width as i32 - TAG_PADDING * 2);
        let close_center = ti.tag_close_center(chip_x, chip_width, ti.chip_row_y());

        // The centre of the drawn circle is inside the field, not above or below it.
        assert!(
            field.contains_point(close_center),
            "the close circle is painted inside the field: {close_center:?}, field={field:?}"
        );
        // And clicking exactly there removes the tag.
        ti.handle_event(&Event::MousePress { pos: close_center, button: 1, modifiers: 0 });
        assert!(ti.tags().is_empty(), "the drawn close circle answers a click");
    }

    /// The field is a full-width band one text-field height tall, centred in the control.
    ///
    /// The control painted its whole rectangle, so a 240x120 census cell drew a 240x120 box
    /// — a panel rather than a field — and its chips sat on a row unrelated to the fields it
    /// shares a form with.
    #[test]
    fn the_field_is_a_text_field_height_in_any_rectangle() {
        for height in [48u32, 120, 300] {
            let ti = TagInput::new(Rect::new(0, 0, 240, height));
            let field = ti.field_rect();
            assert_eq!(
                field.height,
                dimensions::TEXT_FIELD_MIN_HEIGHT,
                "at control height {height}"
            );
            assert_eq!(field.width, 240, "the field spans the control's width");
            assert_eq!(field.y, (height - field.height) as i32 / 2, "at control height {height}");
        }

        let short = TagInput::new(Rect::new(0, 0, 240, 20));
        assert_eq!(short.field_rect().height, 20, "a short control clamps the field");
    }

    #[test]
    fn tag_input_keyboard_input_emits_signal_on_enter() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        let captured = Arc::new(Mutex::new(None::<Vec<String>>));
        ti.tags_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<Vec<String>>| {
                *captured.lock().unwrap() = Some(val.to_vec());
            }
        });

        ti.handle_event(&Event::KeyPress { key: 99, modifiers: 0 }); // 'c'
        ti.handle_event(&Event::KeyPress { key: 111, modifiers: 0 }); // 'o'
        ti.handle_event(&Event::KeyPress { key: 100, modifiers: 0 }); // 'd'
        ti.handle_event(&Event::KeyPress { key: 101, modifiers: 0 }); // 'e'
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter

        assert_eq!(*captured.lock().unwrap(), Some(vec!["code".to_string()]));
    }

    #[test]
    fn tag_input_focus_lost_commits_input() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 112, modifiers: 0 }); // 'p'
        ti.handle_event(&Event::KeyPress { key: 101, modifiers: 0 }); // 'e'
        ti.handle_event(&Event::KeyPress { key: 110, modifiers: 0 }); // 'n'

        // Focus lost — should commit pending input
        ti.handle_event(&Event::FocusLost);
        assert_eq!(ti.tags(), &["pen"]);
        assert!(!ti.is_focused());
    }

    /// The input buffer has a caret, so text can be inserted in the middle.
    #[test]
    fn the_input_caret_edits_inside_the_buffer() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        for key in [114u32, 115] {
            ti.handle_event(&Event::KeyPress { key, modifiers: 0 });
        }
        assert_eq!(ti.input_cursor_position(), 2);
        ti.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        ti.handle_event(&Event::KeyPress { key: 117, modifiers: 0 }); // 'u'
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(ti.tags(), &["rus"]);
    }

    /// Pasting a separated list creates one tag per field, bounded by `max_tags`.
    #[test]
    fn paste_creates_multiple_tags_and_respects_the_limit() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_max_tags(Some(3));
        assert_eq!(ti.max_tags(), Some(3));
        ti.paste_text("red, green; blue\nyellow");
        assert_eq!(ti.tags(), &["red", "green", "blue"]);
        ti.add_tag("black");
        assert_eq!(ti.tags().len(), 3, "the limit refuses a fourth tag");
    }

    /// Suggestions complete the buffer, and a suggestion is selected in full.
    #[test]
    fn tab_accepts_the_first_matching_suggestion() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_suggestions(vec!["rust".to_string(), "ruby".to_string()]);
        ti.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        ti.handle_event(&Event::KeyPress { key: 114, modifiers: 0 });
        ti.handle_event(&Event::KeyPress { key: 9, modifiers: 0 });
        assert_eq!(ti.suggestions().len(), 2);
        assert_eq!(ti.input_selection(), Some((0, 4)));
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(ti.tags(), &["rust"]);
    }

    /// Dragging a chip reorders the list, and double-clicking edits it.
    #[test]
    fn tags_can_be_reordered_and_edited() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("one");
        ti.add_tag("two");
        ti.add_tag("three");
        assert!(ti.move_tag(0, 2));
        assert_eq!(ti.tags(), &["two", "three", "one"]);

        assert!(ti.begin_edit_tag(1));
        assert_eq!(ti.tags(), &["two", "one"]);
        assert_eq!(ti.input_buffer, "three");
        ti.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(ti.tags(), &["two", "one", "three"]);
    }

    /// Primary+V turns a comma-separated clipboard into tags, and Primary+C copies the buffer.
    #[test]
    fn clipboard_chords_paste_a_list_and_copy_the_buffer() {
        let _clipboard = crate::clipboard::clipboard_test_guard();
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        crate::set_clipboard_text("alpha, beta; gamma");
        // Primary is the host's accelerator chord, not a hard-coded Control bit.
        ti.handle_event(&Event::KeyPress { key: 86, modifiers: primary_modifier_event_bits() });
        assert_eq!(ti.tags(), &["alpha", "beta", "gamma"]);

        ti.handle_event(&Event::KeyPress { key: 120, modifiers: 0 });
        assert_eq!(ti.input_buffer, "x");
        ti.handle_event(&Event::KeyPress { key: 67, modifiers: primary_modifier_event_bits() });
        assert_eq!(crate::get_clipboard_text(), "x");
    }
}
