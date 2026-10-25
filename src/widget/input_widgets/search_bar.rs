// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SearchBar widget — iOS-style search bar with search icon, clear button,
//! and optional cancel button when active.
//!
//! Unlike the existing SearchBox, this widget is designed for mobile-style
//! search UX with active state management and a cancel button.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{
    expect_horizontal_alignment, expect_string, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// The number of recent searches the bar remembers.
const MAX_RECENT_SEARCHES: usize = 5;
/// Height of one recent-search row in the menu below the field.
const RECENT_ROW_HEIGHT: u32 = 24;

/// iOS-style search bar widget.
///
/// Displays a rounded search field with a search icon on the left, optional
/// clear button when text is present, and a cancel button on the right when
/// the search bar is active.
pub struct SearchBar {
    base: BaseWidget,
    text: String,
    placeholder: String,
    is_active: bool,
    cancel_button_visible: bool,
    /// How the typed value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field band as a matter of the
    /// field's own layout, so a `top`/`bottom` value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    /// Emitted when the text content changes.
    pub text_changed: Signal1<String>,
    /// Emitted when the user submits a search (Enter key).
    pub search_submitted: Signal1<String>,
    /// Emitted when the user taps the cancel button.
    pub canceled: GenericSignal,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// Whether the field owns keyboard focus. Key and text events only edit the value while
    /// this is true; before it existed, any keystroke anywhere in the window rewrote the query.
    focused: bool,
    /// The insertion point, in bytes from the start of the value.
    cursor: usize,
    /// The other end of a selection, in bytes. `None` means no selection.
    selection_anchor: Option<usize>,
    /// The caret's blink state, shared with the crate's other carets.
    cursor_blink: crate::style::CursorBlink,
    /// Most recent submitted queries, newest first.
    recent_searches: Vec<String>,
    /// Whether the recent-search menu is open.
    show_recent: bool,
    /// When true, the value is drawn as one chip per whitespace-separated term.
    tokenizing: bool,
}

impl SearchBar {
    /// Creates a new SearchBar widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SearchBar, geometry, "SearchBar"),
            text: String::new(),
            placeholder: "Search".to_string(),
            is_active: false,
            cancel_button_visible: true,
            alignment: crate::core::Alignment::Left,
            text_changed: Signal1::new(),
            search_submitted: Signal1::new(),
            canceled: GenericSignal::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            focused: false,
            cursor: 0,
            selection_anchor: None,
            cursor_blink: crate::style::CursorBlink::new(),
            recent_searches: Vec::new(),
            show_recent: false,
            tokenizing: false,
        }
    }

    /// Returns the current search text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Compatibility spelling of [`Self::canceled`] (W-19): the same emitted signal.
    pub fn cancelled(&self) -> &GenericSignal {
        &self.canceled
    }

    /// How the typed value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the typed value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the value is centred
    /// vertically in the field band by the field's own layout — the property route refuses it
    /// through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Sets the search text. Emits `text_changed` signal.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let new_text = text.into();
        if self.text != new_text {
            let before = self.text.clone();
            self.text = new_text.clone();
            if !self.restoring_history {
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.push(Box::new(ValueCommand::new(
                    self.history_target.clone(),
                    before,
                    self.text.clone(),
                    "Edit text",
                    "search_bar_text",
                )));
            }
            self.cursor = self.text.len();
            self.selection_anchor = None;
            self.text_changed.emit(new_text);
            self.base.request_redraw();
        }
    }

    /// Returns the placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text.
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        let placeholder = placeholder.into();
        if self.placeholder == placeholder {
            return;
        }
        self.placeholder = placeholder;
        self.base.request_redraw();
    }

    /// Sets whether the search bar is active (showing cancel button).
    pub fn set_active(&mut self, active: bool) {
        if self.is_active != active {
            self.is_active = active;
            self.base.request_redraw();
        }
    }

    /// Returns whether the search bar is active.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Whether the field owns keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets the focused state. Losing focus closes the recent-search menu.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        if focused {
            self.cursor_blink.start();
        } else {
            self.cursor_blink.stop();
            self.selection_anchor = None;
            self.show_recent = false;
        }
        self.base.request_redraw();
    }

    /// The caret's byte offset in the value.
    pub fn cursor_position(&self) -> usize {
        self.cursor
    }

    /// Moves the caret, clearing the selection.
    pub fn set_cursor_position(&mut self, position: usize) {
        let position = crate::widget::text_utils::floor_char_boundary(
            &self.text,
            position.min(self.text.len()),
        );
        if self.cursor == position && self.selection_anchor.is_none() {
            return;
        }
        self.cursor = position;
        self.selection_anchor = None;
        self.base.request_redraw();
    }

    /// The selected byte range, or `None` when nothing is selected.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let (start, end) =
            if anchor <= self.cursor { (anchor, self.cursor) } else { (self.cursor, anchor) };
        (start != end).then_some((start, end))
    }

    /// The selected text, or an empty string when nothing is selected.
    pub fn selected_text(&self) -> String {
        self.selection_range()
            .map_or_else(String::new, |(start, end)| self.text[start..end].to_string())
    }

    /// Selects the whole value.
    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection_anchor = Some(0);
            self.cursor = self.text.len();
            self.base.request_redraw();
        }
    }

    /// Advances the caret blink. Returns whether it owes another frame.
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

    /// The remembered recent searches, newest first.
    pub fn recent_searches(&self) -> &[String] {
        &self.recent_searches
    }

    /// Replaces the recent-search list. The list is capped at five entries.
    pub fn set_recent_searches(&mut self, searches: Vec<String>) {
        let mut next: Vec<String> = Vec::new();
        for search in searches {
            let search = search.trim().to_string();
            if search.is_empty() || next.contains(&search) {
                continue;
            }
            next.push(search);
            if next.len() == MAX_RECENT_SEARCHES {
                break;
            }
        }
        if self.recent_searches == next {
            return;
        }
        self.recent_searches = next;
        self.base.request_redraw();
    }

    /// Remembers `search` as the newest recent search.
    pub fn remember_search(&mut self, search: &str) {
        let search = search.trim();
        if search.is_empty() {
            return;
        }
        self.recent_searches.retain(|existing| existing != search);
        self.recent_searches.insert(0, search.to_string());
        self.recent_searches.truncate(MAX_RECENT_SEARCHES);
        self.base.request_redraw();
    }

    /// Clears the recent-search list and closes its menu.
    pub fn clear_recent_searches(&mut self) {
        if self.recent_searches.is_empty() && !self.show_recent {
            return;
        }
        self.recent_searches.clear();
        self.show_recent = false;
        self.base.request_redraw();
    }

    /// Whether the recent-search menu is open.
    pub fn is_recent_menu_open(&self) -> bool {
        self.show_recent
    }

    /// Opens or closes the recent-search menu.
    pub fn set_recent_menu_open(&mut self, open: bool) {
        let open = open && !self.recent_searches.is_empty();
        if self.show_recent == open {
            return;
        }
        self.show_recent = open;
        self.base.request_redraw();
    }

    /// Whether the value is drawn as one chip per term.
    pub fn is_tokenizing(&self) -> bool {
        self.tokenizing
    }

    /// Sets whether the value is drawn as chips.
    ///
    /// Tokenizing is presentation only: the stored value and the submitted string stay the
    /// space-separated text the user typed, so a host never has to rejoin chips.
    pub fn set_tokenizing(&mut self, tokenizing: bool) {
        if self.tokenizing == tokenizing {
            return;
        }
        self.tokenizing = tokenizing;
        self.base.request_redraw();
    }

    /// The value split into its terms, or an empty list when the value is blank.
    pub fn tokens(&self) -> Vec<String> {
        self.text.split_whitespace().map(|term| term.to_string()).collect()
    }

    /// The width of the cancel button, measured from its label rather than assumed.
    ///
    /// The field reserves this much room when active, so a translated or larger-font label
    /// cannot be clipped by a literal 60 px reservation.
    fn cancel_button_width(&self) -> u32 {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        let font = effective_font(self.style());
        let text = "Cancel";
        let measured = context.measure_text(text, font).width;
        (measured + 16).max(44)
    }

    /// Sets whether the cancel button is visible when active.
    pub fn set_cancel_button_visible(&mut self, visible: bool) {
        if self.cancel_button_visible == visible {
            return;
        }
        self.cancel_button_visible = visible;
        self.base.request_redraw();
    }

    /// Returns whether the cancel button is visible when active.
    pub fn cancel_button_visible(&self) -> bool {
        self.cancel_button_visible
    }

    /// Clears the search text. Emits `text_changed` signal.
    pub fn clear(&mut self) {
        if !self.text.is_empty() {
            self.set_text(String::new());
        }
    }

    /// Reverts the most recent edit in this search bar.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise emits `text_changed` with the restored text.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the most recently undone edit.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise emits `text_changed` with the restored text.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone edit to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.restoring_history = false;
        self.cursor = self.text.len();
        self.selection_anchor = None;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Matches a printable or IME-committed string at the caret, replacing any selection.
    fn insert_text(&mut self, text: &str) {
        let filtered: String = text.chars().filter(|ch| !ch.is_control()).collect();
        if filtered.is_empty() {
            return;
        }
        let (start, end) = self.selection_range().unwrap_or((self.cursor, self.cursor));
        let mut next = self.text.clone();
        next.replace_range(start..end, &filtered);
        self.set_text(next);
        self.cursor = start + filtered.len();
        self.selection_anchor = None;
        self.base.request_redraw();
    }

    /// Removes the selection, or the character before the caret when there is none.
    fn backspace(&mut self) {
        let (start, end) = self.selection_range().unwrap_or_else(|| {
            let start = crate::widget::text_utils::previous_char_boundary(&self.text, self.cursor);
            (start, self.cursor)
        });
        if start == end {
            return;
        }
        let mut next = self.text.clone();
        next.replace_range(start..end, "");
        self.set_text(next);
        self.cursor = start;
        self.selection_anchor = None;
    }

    /// Moves the caret by `delta` characters, extending the selection when `extend` is set.
    fn move_caret(&mut self, delta: i32, extend: bool) {
        let mut target = self.cursor;
        if delta < 0 {
            for _ in 0..delta.unsigned_abs() {
                target = crate::widget::text_utils::previous_char_boundary(&self.text, target);
            }
        } else {
            for _ in 0..delta.unsigned_abs() {
                target = crate::widget::text_utils::next_char_boundary(&self.text, target);
            }
        }
        let target = crate::widget::text_utils::floor_char_boundary(&self.text, target);
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
        } else {
            self.selection_anchor = None;
        }
        if self.cursor != target {
            self.cursor = target;
            self.base.request_redraw();
        }
    }

    /// The recent-search row a point falls on, if the menu is open.
    fn recent_row_at(&self, pos: Point) -> Option<usize> {
        if !self.show_recent {
            return None;
        }
        let rect = self.geometry();
        let menu_top = rect.y + rect.height as i32;
        if pos.x < rect.x || pos.x >= rect.x + rect.width as i32 || pos.y < menu_top {
            return None;
        }
        let row = ((pos.y - menu_top) / RECENT_ROW_HEIGHT as i32) as usize;
        (row < self.recent_searches.len()).then_some(row)
    }

    /// Submits the current search text.
    fn submit(&mut self) {
        let text = self.text.clone();
        if !text.is_empty() {
            self.remember_search(&text);
            self.search_submitted.emit(text);
        }
    }
}

impl Widget for SearchBar {
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
        crate::core::Size::new(200, 36)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "search_submitted" => {
                Some(EventSignalRef::mapped("search_submitted", &self.search_submitted, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "canceled" => Some(EventSignalRef::unit("canceled", &self.canceled)),
            _ => None,
        }
    }
}

/// `SearchBar`'s property contract.
impl WidgetProperties for SearchBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SEARCH_BAR_PROPERTIES`.
        property_names_of!["text", "placeholder", "alignment", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `search_bar` publishes.
    ///
    /// Both names assign state and need a payload, so both are answered through the
    /// property route. The capability publishes no zero-argument action for this
    /// control, which is why there is no `Ok(())` arm here.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_placeholder" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for SearchBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        // Resolved once for the whole draw: the field, the icon and the cancel label all read
        // it, and re-resolving would take the theme lock once per read. Precedence is the
        // crate-wide one — an explicit style wins, then the theme, then the literal.
        let style = self.style();
        let themed = crate::style::resolved_theme_style_shared("search_bar");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let accent = style
            .border_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.border_color));
        let cancel_width: u32 = if self.is_active && self.cancel_button_visible {
            self.cancel_button_width()
        } else {
            0
        };
        let field_width = rect.width.saturating_sub(cancel_width + 4);

        // Cancel button area
        let cancel_x = rect.x + rect.width as i32 - cancel_width as i32;
        // The **effective font** — the resolved theme/caller font — for the cancel label, the
        // placeholder and the value (D09-STYLE-01).
        let font = effective_font(style);
        if self.is_active && self.cancel_button_visible {
            let _cancel_rect = Rect::new(cancel_x, rect.y, cancel_width, rect.height);
            let cancel_font = font;
            let cancel_text = "Cancel";
            let metrics = context.measure_text(cancel_text, cancel_font);
            let text_x = cancel_x + (cancel_width as i32 - metrics.width as i32) / 2;
            // Centre the label on the cancel area. The origin is the glyph box's top edge, so
            // half the *line box* is the correct offset; the `+ ascent` began the glyph box
            // half a line below the middle.
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            context.draw_text(
                Point::new(text_x, text_y),
                cancel_text,
                cancel_font,
                // The cancel label is the control's accent — an actionable affordance — so it
                // follows the resolved border colour rather than a fixed blue.
                accent.unwrap_or(Color::rgba(52, 120, 246, 255)),
                HorizontalAlignment::Left,
            );
        }

        // Search field background
        //
        // Resolved from the style so a theme switch reaches this control; the two states are
        // *derived* from the resolved colour, so disabled reads as the same field with the
        // energy taken out and the control follows the theme.
        let field_rect = Rect::new(rect.x, rect.y, field_width, rect.height);
        let base_field =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(200, 200, 205, 200));
        // The page this field sits in, read once: a disabled fill recedes toward it.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        // # Why the field acknowledges the pointer
        //
        // Disabled is handled (faded toward white); hovered was not, so a pointer over the
        // search field painted the resting fill and the field read as static. The weight comes
        // from `StateOverlay::fill_blend`, which is the same source the preset theme's
        // `"<kind>:hover"` key derives from — a private constant here would drift from it.
        let hover = if is_enabled {
            let overlay = crate::style::StateOverlay::from_base(
                self.base.is_hovered(),
                self.base.is_pressed(),
                false,
            );
            overlay.fill_blend()
        } else {
            0.0
        };
        let field_color = if !is_enabled {
            // Stepped toward the field's own contrast colour, exactly as the hover arm below
            // does. It read `base_field.blend(&Color::WHITE, 0.35)`, and white is not a
            // direction: against a light window it pushed the disabled field *further* from the
            // window than the enabled one (measured 11.90:1 disabled vs 9.04:1 enabled), so the
            // disabled state was the more prominent of the two.
            self.base.disabled_surface_near(base_field, window_fill)
        } else if hover > 0.0 {
            base_field.blend(&base_field.contrast_color(), hover)
        } else {
            base_field
        };
        // The field's ink: the typed text and the placeholder both read on `field_color`, so
        // they are derived from it rather than being two literals written for a light field.
        // The previous pair (`40,40,40` and `160,160,160`) rendered at 1.5:1 and 2.6:1 once the
        // field resolved to a dark surface.
        let field_ink = field_color.contrast_color();
        let corner_radius = rect.height / 2;
        context.fill_rounded_rect(field_rect, corner_radius, field_color);

        // Search icon (magnifying glass)
        let icon_size: u32 = 14;
        let icon_left = rect.x + 10;
        let icon_top = rect.y + (rect.height as i32 - icon_size as i32) / 2;
        // The magnifier is a glyph, so it reads the control's text colour.
        let icon_color = style.text_color.unwrap_or(Color::rgba(140, 140, 140, 255));
        context.draw_circle_stroke(
            Point::new(icon_left + (icon_size / 2) as i32, icon_top + (icon_size / 2) as i32),
            icon_size / 2,
            icon_color,
            2,
        );
        // Search icon handle (diagonal line from bottom-right of circle)
        let handle_offset = icon_size as i32;
        context.draw_line(
            Point::new(icon_left + handle_offset - 2, icon_top + handle_offset - 2),
            Point::new(icon_left + handle_offset + 3, icon_top + handle_offset + 3),
            icon_color,
        );

        // Text or placeholder
        let text_left = icon_left + icon_size as i32 + 6;
        let text_width = if self.text.is_empty() {
            field_width.saturating_sub((text_left - rect.x) as u32)
        } else {
            field_width.saturating_sub((text_left - rect.x) as u32 + 24)
        };
        // The alignment this control's own value is drawn with. The placeholder shares it, so the
        // field reads as one column whichever state it is in.
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);

        // The value's own origin and advance, shared by the selection band and the caret. The
        // placeholder branch below keeps its own centring; only real text has a caret.
        let value_advance = if self.text.is_empty() {
            0
        } else {
            context.measure_text(&self.text, font).width as i32
        };
        let value_box = Rect::new(
            text_left,
            rect.y
                + (rect.height as i32
                    - context.text_line(Rect::new(0, 0, 1, rect.height), font).height as i32)
                    / 2,
            text_width.max(1),
            rect.height,
        );
        let value_origin = crate::render::fitted_origin(value_box, value_advance, value_align);

        // — Selection band —
        if !self.text.is_empty() {
            if let Some((start, end)) = self.selection_range() {
                let start_x =
                    value_origin.x + context.measure_text(&self.text[..start], font).width as i32;
                let end_x =
                    value_origin.x + context.measure_text(&self.text[..end], font).width as i32;
                let line_height = context.text_line(Rect::new(0, 0, 1, rect.height), font).height;
                context.fill_rect(
                    Rect::new(
                        start_x.min(end_x),
                        value_box.y,
                        (end_x - start_x).unsigned_abs(),
                        line_height.max(1),
                    ),
                    accent.unwrap_or(Color::rgba(52, 120, 246, 255)).with_alpha(90),
                );
            }
        }

        if self.text.is_empty() {
            // Draw placeholder text. Same centring fix as the typed branch below: `ascent` is
            // inside the line box, so adding it to an already-centred y put the hint half a line
            // low, and the fixed grey could not read on a themed field.
            let metrics = context.measure_text(&self.placeholder, font);
            if text_width >= metrics.width {
                let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
                context.draw_text_fitted_cow(
                    Rect::new(text_left, text_y, text_width.max(1), metrics.height.max(1)),
                    &self.placeholder,
                    font,
                    field_ink.blend(&field_color, 0.45),
                    value_align,
                );
            }
        } else {
            // Draw text
            let metrics = context.measure_text(&self.text, font);
            if text_width >= metrics.width {
                // Two defects in one origin. It added `ascent` on top of an already-centred y —
                // the renderer's origin is the glyph's **top** edge, so the label sat half a
                // line low — and it used a fixed `40,40,40`, which is a light field's ink and
                // rendered at 1.5:1 once the field's background resolved to a dark surface.
                let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
                if self.tokenizing {
                    // Tokenized display: one rounded chip per term. The value itself stays the
                    // plain space-separated text, so submitting and querying are unchanged.
                    let mut chip_x = text_left;
                    let chip_fill = accent.unwrap_or(Color::rgba(52, 120, 246, 255)).with_alpha(40);
                    for token in self.tokens() {
                        let advance = context.measure_text(&token, font).width as i32;
                        let chip_width = (advance + 12).max(1) as u32;
                        if chip_x + chip_width as i32 > text_left + text_width as i32 {
                            break;
                        }
                        context.fill_rounded_rect(
                            Rect::new(chip_x, text_y - 2, chip_width, metrics.height + 4),
                            (metrics.height + 4) / 2,
                            chip_fill,
                        );
                        context.draw_text(
                            Point::new(chip_x + 6, text_y),
                            &token,
                            font,
                            field_ink,
                            HorizontalAlignment::Left,
                        );
                        chip_x += chip_width as i32 + 4;
                    }
                } else {
                    context.draw_text_fitted_cow(
                        Rect::new(
                            text_left,
                            text_y,
                            (rect.width as i32 - (text_left - rect.x) - 12).max(1) as u32,
                            metrics.height.max(1),
                        ),
                        &self.text,
                        font,
                        field_ink,
                        value_align,
                    );
                }
            }

            // — Caret —
            if self.focused && self.cursor_blink.is_visible() {
                let caret_byte =
                    crate::widget::text_utils::floor_char_boundary(&self.text, self.cursor);
                let caret_x = value_origin.x
                    + context.measure_text(&self.text[..caret_byte], font).width as i32;
                let caret_x = caret_x.min(text_left + text_width as i32 - 1).max(text_left);
                context.draw_line(
                    Point::new(caret_x, value_box.y + 2),
                    Point::new(caret_x, value_box.y + value_box.height as i32 - 4),
                    field_ink,
                );
            }

            // Draw clear button (X)
            let clear_x = rect.x + field_width as i32 - 22;
            let clear_y = rect.y + (rect.height as i32 - 16) / 2;
            let clear_center = Point::new(clear_x + 8, clear_y + 8);
            context.fill_circle(clear_center, 8, Color::rgba(180, 180, 180, 200));
            // X lines
            context.draw_line(
                Point::new(clear_center.x - 3, clear_center.y - 3),
                Point::new(clear_center.x + 3, clear_center.y + 3),
                Color::WHITE,
            );
            context.draw_line(
                Point::new(clear_center.x + 3, clear_center.y - 3),
                Point::new(clear_center.x - 3, clear_center.y + 3),
                Color::WHITE,
            );
        }

        // — Recent searches —
        // The menu hangs below the field, one row per remembered query. It is painted from the
        // same state the hit test reads, so a row cannot be clickable somewhere it is not drawn.
        if self.show_recent {
            let menu_top = rect.y + rect.height as i32;
            for (index, search) in self.recent_searches.iter().enumerate() {
                let row = Rect::new(
                    rect.x,
                    menu_top + (index as i32) * RECENT_ROW_HEIGHT as i32,
                    rect.width,
                    RECENT_ROW_HEIGHT,
                );
                context.fill_rect(row, field_color);
                context.draw_rect(
                    row,
                    style.border_color.or(themed_bg).unwrap_or(Color::rgba(200, 200, 205, 200)),
                );
                context.draw_text(
                    Point::new(row.x + 10, row.y + 4),
                    search,
                    font,
                    field_ink,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl SearchBar {
    /// The byte offset a horizontal pixel points at, using the same origin the paint uses.
    fn byte_index_at_x(&self, x: i32) -> usize {
        if self.text.is_empty() {
            return 0;
        }
        let rect = self.geometry();
        let cancel_width = if self.is_active && self.cancel_button_visible {
            self.cancel_button_width()
        } else {
            0
        };
        let field_width = rect.width.saturating_sub(cancel_width + 4);
        let text_left = rect.x + 10 + 14 + 6;
        let text_width = field_width.saturating_sub((text_left - rect.x) as u32);
        let font = effective_font(self.style());
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        let advance = context.measure_text(&self.text, font).width as i32;
        let value_box = Rect::new(text_left, rect.y, text_width.max(1), rect.height);
        let origin = crate::render::fitted_origin(
            value_box,
            advance,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );
        let dx = (x - origin.x).clamp(0, advance.max(1));
        let chars = self.text.chars().count().max(1) as f32;
        let boundary =
            ((dx as f32 * chars / advance.max(1) as f32).round() as usize).min(chars as usize);
        self.text.char_indices().nth(boundary).map(|(index, _)| index).unwrap_or(self.text.len())
    }
}

impl EventHandler for SearchBar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    // A click on the recent-search menu picks that query; the menu hangs below
                    // the field, so this must be tested before the field hit test.
                    if let Some(row) = self.recent_row_at(*pos) {
                        let chosen = self.recent_searches[row].clone();
                        self.show_recent = false;
                        self.set_text(chosen);
                        self.submit();
                        return;
                    }
                    let rect = self.geometry();
                    let cancel_width = if self.is_active && self.cancel_button_visible {
                        self.cancel_button_width()
                    } else {
                        0
                    };
                    let cancel_x = rect.x + rect.width as i32 - cancel_width as i32;

                    // Cancel button click
                    if self.is_active && self.cancel_button_visible && pos.x >= cancel_x {
                        self.is_active = false;
                        self.set_focused(false);
                        self.canceled.emit();
                        self.base.request_redraw();
                        return;
                    }

                    // Clear button click (when text is non-empty)
                    let field_width = rect.width.saturating_sub(cancel_width + 4);
                    let clear_x = rect.x + field_width as i32 - 22;
                    if !self.text.is_empty()
                        && pos.x >= clear_x
                        && pos.x < clear_x + 16
                        && pos.y >= rect.y
                        && pos.y < rect.y + rect.height as i32
                    {
                        self.clear();
                        return;
                    }

                    // Focus the search bar and place the caret where the press landed.
                    if !self.is_active {
                        self.set_active(true);
                    }
                    self.set_focused(true);
                    self.cursor = self.byte_index_at_x(pos.x);
                    self.selection_anchor = None;
                    // An empty field shows its history; a field with a query keeps it closed so
                    // the menu never covers the user's typing.
                    self.show_recent = self.text.is_empty() && !self.recent_searches.is_empty();
                    self.base.request_redraw();
                }
            }
            Event::FocusGained { .. } => {
                self.set_active(true);
                self.set_focused(true);
            }
            Event::FocusLost => self.set_focused(false),
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
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                if *key == key_codes::ENTER || *key == key_codes::LINE_FEED {
                    // Enter submits; if the recent menu is open on the keyboard, Enter picks its
                    // first row instead of submitting an empty query.
                    if self.show_recent {
                        if let Some(first) = self.recent_searches.first().cloned() {
                            self.show_recent = false;
                            self.set_text(first);
                        }
                    }
                    self.submit();
                } else if *key == key_codes::ESCAPE {
                    // Escape cancels: the field leaves its active state, drops the query and
                    // tells the host, so a search that was opened by mistake can be dismissed
                    // without a pointer.
                    self.clear();
                    self.is_active = false;
                    self.show_recent = false;
                    self.set_focused(false);
                    self.canceled.emit();
                    self.base.request_redraw();
                } else if *key == key_codes::BACKSPACE {
                    self.backspace();
                } else if *key == 127 {
                    // Delete removes the character after the caret.
                    let (start, end) = self.selection_range().unwrap_or_else(|| {
                        (
                            self.cursor,
                            crate::widget::text_utils::next_char_boundary(&self.text, self.cursor),
                        )
                    });
                    if start != end {
                        let mut next = self.text.clone();
                        next.replace_range(start..end, "");
                        self.set_text(next);
                        self.cursor = start;
                        self.selection_anchor = None;
                    }
                } else if *key == 37 {
                    self.move_caret(-1, shift);
                } else if *key == 39 {
                    self.move_caret(1, shift);
                } else if *key == 36 {
                    self.move_caret(-(self.text.chars().count() as i32), shift);
                } else if *key == 35 {
                    self.move_caret(self.text.chars().count() as i32, shift);
                } else if *key == 40 && self.text.is_empty() {
                    self.show_recent = !self.recent_searches.is_empty();
                    self.base.request_redraw();
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII.
                    if let Some(c) = char::from_u32(*key) {
                        self.insert_text(&c.to_string());
                    }
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if self.focused && !text.is_empty() {
                    self.insert_text(text);
                }
            }
            Event::ImePreedit { .. } => {}
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::input_widgets::shortcut_editor::primary_modifier_event_bits;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    fn make_search_bar() -> SearchBar {
        SearchBar::new(Rect::new(0, 0, 300, 40))
    }

    #[test]
    fn search_bar_default_state() {
        let sb = make_search_bar();
        assert_eq!(sb.text(), "");
        assert_eq!(sb.placeholder(), "Search");
        assert!(!sb.is_active());
        assert!(sb.cancel_button_visible());
        assert_eq!(sb.kind(), WidgetKind::SearchBar);
    }

    #[test]
    fn search_bar_set_text() {
        let mut sb = make_search_bar();
        sb.set_text("hello");
        assert_eq!(sb.text(), "hello");
    }

    #[test]
    fn search_bar_text_changed_signal() {
        let mut sb = make_search_bar();
        let captured = Arc::new(Mutex::new(None));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some((*val).clone());
            }
        });

        sb.set_text("test");
        assert_eq!(captured.lock().unwrap().as_deref(), Some("test"));
    }

    #[test]
    fn search_bar_submit_signal() {
        let mut sb = make_search_bar();
        sb.set_text("query");
        let captured = Arc::new(Mutex::new(None));
        sb.search_submitted.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some((*val).clone());
            }
        });

        // Simulate Enter key press
        sb.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        sb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(captured.lock().unwrap().as_deref(), Some("query"));
    }

    #[test]
    fn search_bar_clear_button_and_signal() {
        let mut sb = make_search_bar();
        sb.set_text("hello");
        let captured = Arc::new(Mutex::new(None));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some((*val).clone());
            }
        });

        sb.clear();
        assert_eq!(sb.text(), "");
        assert_eq!(captured.lock().unwrap().as_deref(), Some(""));
    }

    #[test]
    fn search_bar_cancel_signal() {
        let mut sb = make_search_bar();
        sb.set_active(true);
        let captured = Arc::new(AtomicBool::new(false));
        sb.canceled.connect({
            let captured = Arc::clone(&captured);
            move || {
                captured.store(true, Ordering::SeqCst);
            }
        });

        // Click cancel button (at x=240+, since width=300, cancel starts at 300-60=240)
        sb.handle_event(&Event::MousePress { pos: Point::new(260, 20), button: 1, modifiers: 0 });
        assert!(!sb.is_active());
        assert!(captured.load(Ordering::SeqCst));
    }

    #[test]
    fn search_bar_set_placeholder() {
        let mut sb = make_search_bar();
        sb.set_placeholder("Find...");
        assert_eq!(sb.placeholder(), "Find...");
    }

    #[test]
    fn search_bar_svg_output() {
        let mut sb = make_search_bar();
        sb.set_text("foo");
        let svg = render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn search_bar_undo_redo_restores_text() {
        let mut sb = make_search_bar();
        sb.set_text("alpha");
        sb.set_text("beta");

        assert!(sb.can_undo());
        assert!(sb.undo());
        assert_eq!(sb.text(), "alpha");
        assert!(sb.can_redo());
        assert!(sb.redo());
        assert_eq!(sb.text(), "beta");
    }

    #[test]
    fn search_bar_key_shortcuts_drive_history() {
        let mut sb = make_search_bar();
        sb.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 });
        sb.handle_event(&Event::KeyPress { key: 98, modifiers: 0 });
        assert_eq!(sb.text(), "ab");

        let primary = primary_modifier_event_bits();
        sb.handle_event(&Event::KeyPress { key: 90, modifiers: primary });
        assert_eq!(sb.text(), "a");
        sb.handle_event(&Event::KeyPress { key: 89, modifiers: primary });
        assert_eq!(sb.text(), "ab");
    }

    #[test]
    fn search_bar_accepts_unicode_text_input_events() {
        let mut sb = make_search_bar();
        sb.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        sb.handle_event(&Event::text_input("你好"));
        sb.handle_event(&Event::ime_commit("世界"));

        assert_eq!(sb.text(), "你好世界");
        assert!(sb.undo());
        assert_eq!(sb.text(), "你好");
    }

    /// Keystrokes only edit the bar that owns focus.
    #[test]
    fn keys_are_gated_on_focus() {
        let mut sb = make_search_bar();
        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 });
        assert_eq!(sb.text(), "", "an unfocused bar must ignore the keyboard");

        sb.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 });
        assert_eq!(sb.text(), "a");
        sb.handle_event(&Event::FocusLost);
        sb.handle_event(&Event::KeyPress { key: 98, modifiers: 0 });
        assert_eq!(sb.text(), "a", "losing focus stops editing");
    }

    /// Escape is a full cancel: clear, deactivate and notify.
    #[test]
    fn escape_cancels_the_search() {
        let mut sb = make_search_bar();
        sb.set_active(true);
        sb.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 });
        let canceled = Arc::new(AtomicBool::new(false));
        sb.canceled.connect({
            let canceled = Arc::clone(&canceled);
            move || canceled.store(true, std::sync::atomic::Ordering::SeqCst)
        });

        sb.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(sb.text(), "");
        assert!(!sb.is_active());
        assert!(!sb.is_focused());
        assert!(canceled.load(std::sync::atomic::Ordering::SeqCst));
    }

    /// Submitted queries become recent searches, which can be chosen from the menu.
    #[test]
    fn submitted_queries_fill_the_recent_menu() {
        let mut sb = make_search_bar();
        sb.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        sb.set_text("rust");
        sb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        sb.set_text("widgets");
        sb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });

        assert_eq!(
            sb.recent_searches(),
            vec!["widgets".to_string(), "rust".to_string()].as_slice()
        );
        assert_eq!(sb.recent_searches().len(), 2);

        // Clicking the first menu row selects that query.
        let menu_row = Point::new(20, sb.geometry().y + sb.geometry().height as i32 + 4);
        sb.set_recent_menu_open(true);
        sb.handle_event(&Event::MousePress { pos: menu_row, button: 1, modifiers: 0 });
        assert_eq!(sb.text(), "widgets");
        assert!(!sb.is_recent_menu_open());
        assert!(sb.recent_searches().len() <= MAX_RECENT_SEARCHES);
    }

    /// The caret edits inside the query, and tokenizing only changes the paint.
    #[test]
    fn the_caret_edits_inside_the_query_and_tokens_split_it() {
        let mut sb = make_search_bar();
        sb.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        sb.set_text("helo");
        sb.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        sb.handle_event(&Event::KeyPress { key: 108, modifiers: 0 });
        assert_eq!(sb.text(), "hello");

        sb.set_text("alpha beta");
        sb.set_tokenizing(true);
        assert_eq!(sb.tokens(), vec!["alpha".to_string(), "beta".to_string()]);
        assert!(sb.tokens().len() == 2, "tokenizing splits the value into its terms");
        assert_eq!(sb.text(), "alpha beta", "tokenizing does not alter the value");
    }
}
