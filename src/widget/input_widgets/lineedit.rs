// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Single-line text edit widget.
use crate::compat::{Box, Rc, RefCell, String, ToString};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{TextSnapshotCommand, UndoStack};

use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::decorations::{
    DecorationLayout, DecorationMetrics, DecorationSlots, DECORATION_GAP,
};
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::text_utils::{byte_index_of_char, floor_char_boundary};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Single-line text edit widget.
pub struct LineEdit {
    base: BaseWidget,
    text: String,
    placeholder_text: String,
    max_length: Option<usize>,
    echo_mode: EchoMode,
    cursor_position: usize,
    selection_start: Option<usize>,
    read_only: bool,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// Whether this field currently owns keyboard focus.
    ///
    /// Tracked here because the caret is only drawn for the focused field, and the
    /// library's focus router sends `FocusGained` / `FocusLost` (see
    /// `crate::widget::runtime::focus_widget`). Without it the field had no way to
    /// know, and the caret was never drawn at all.
    focused: bool,
    /// The five non-value strings this field shows: `prefix`/`suffix` inside it, and
    /// `helper`/`error`/`counter` on the row below.
    ///
    /// Held as one record rather than five fields so a caller assembling a form from a document can
    /// replace all five in one step, and so the painted state is one value that can be compared.
    decorations: DecorationSlots,
    /// The caret's blink state, advanced by [`LineEdit::tick`].
    ///
    /// Borrowed from [`crate::style::CursorBlink`] rather than reimplemented, so this field's caret
    /// keeps the tempo and phase logic of every other caret in the crate.
    cursor_blink: crate::style::CursorBlink,
    /// Emitted after the widget's text changes: on edit commits, and after an
    /// undo/redo restores a snapshot. Not emitted when a programmatic
    /// `set_text` is given the text the field already holds.
    pub text_changed: Signal1<String>,
    /// Emitted when editing ends — the field loses focus or Enter is pressed.
    /// Carries no payload.
    pub editing_finished: GenericSignal,
    /// Emitted when Enter is pressed in the field, after the edit is committed.
    pub return_pressed: GenericSignal,
}
/// Text echo mode for a line edit.
///
/// Re-exported from [`crate::platform::EchoMode`] — the widget layer and the
/// platform layer name the **same three modes**, so a mode read back from a
/// native control (`widget_echo_mode()`) can be handed straight to this widget
/// with no conversion, and neither copy can drift from the other
/// (principle #54).
///
/// The former local variant `PasswordEchoOnEdit` was removed with this change: it
/// had no consumer outside this file and its "implementation" was a placeholder
/// that behaved exactly like `Password` (principle #5). Adding a real one back
/// means adding it to the canonical enum and to every backend that must honour
/// it.
pub use crate::platform::EchoMode;

impl LineEdit {
    /// Creates an empty line edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "LineEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            echo_mode: EchoMode::Normal,
            cursor_position: 0,
            selection_start: None,
            read_only: false,
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            focused: false,
            decorations: DecorationSlots::default(),
            cursor_blink: crate::style::CursorBlink::new(),
            text_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
            return_pressed: GenericSignal::new(),
        }
    }
    /// Returns current text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns whether this field currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }
    /// Sets the focus flag directly.
    ///
    /// The runtime normally drives this through `FocusGained` / `FocusLost`, the same
    /// way the other input controls are driven; this setter exists for hosts and tests
    /// that need to place focus without an event round-trip.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        // The caret blinks exactly while the field holds focus, so the blink state is driven from
        // the same flag `draw` reads rather than from a parallel notion of "active".
        if focused {
            self.cursor_blink.start();
        } else {
            self.cursor_blink.stop();
        }
        self.base.request_redraw();
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention: `true` while the caret is still cycling, so
    /// a host schedules the next frame only for a field that is actually blinking. A blurred field
    /// or a read-only one returns `false` immediately — a steady caret has no next frame.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.focused || self.read_only {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }
    /// Sets text and emits text_changed signal if different.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        // `max_length` is enforced here as well as in `insert_text`.
        //
        // # Why the two paths have to agree
        //
        // `insert_text` clamped to the limit and this did not, so a programmatic `set_text` could leave
        // the field holding a longer value than its own limit — and once the `counter` is derived from
        // the value and that limit, the two disagreed visibly (`8/5`). A limit the control enforces only
        // for typing is not a limit, it is a hint. The truncation goes through `floor_char_boundary`
        // because `max_length` counts **characters** and the slice is in bytes.
        let text = match self.max_length {
            Some(max) if text.chars().count() > max => {
                let byte = byte_index_of_char(&text, max);
                text[..byte].to_string()
            }
            _ => text,
        };
        if self.text == text {
            return;
        }
        self.text = text;
        if !self.restoring_history {
            let before = self.history_target.borrow().clone();
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "line_edit_text",
            )));
        }
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Undo the latest text mutation.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Redo the latest undone text mutation.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether a text mutation can be undone.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether a text mutation can be redone.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        self.restoring_history = true;
        self.text = text;
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }
    /// Returns placeholder text.
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }
    /// Sets placeholder text.
    pub fn set_placeholder_text(&mut self, text: String) {
        self.placeholder_text = text;
        self.base.request_redraw();
    }
    /// Returns maximum text length.
    pub fn max_length(&self) -> Option<usize> {
        self.max_length
    }
    /// Sets maximum text length.
    pub fn set_max_length(&mut self, max_length: Option<usize>) {
        self.max_length = max_length;
        // Truncate if needed
        if let Some(max) = max_length {
            if self.text.len() > max {
                self.text.truncate(max);
                self.cursor_position = self.cursor_position.min(max);
                if let Some(start) = &mut self.selection_start {
                    *start = (*start).min(max);
                }
                self.text_changed.emit(self.text.clone());
            }
        }
        self.base.request_redraw();
    }
    /// Returns echo mode.
    pub fn echo_mode(&self) -> EchoMode {
        self.echo_mode
    }
    /// Sets echo mode.
    pub fn set_echo_mode(&mut self, mode: EchoMode) {
        self.echo_mode = mode;
        self.base.request_redraw();
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }
    /// Sets cursor position.
    pub fn set_cursor_position(&mut self, position: usize) {
        self.cursor_position = position.min(self.text.len());
        self.selection_start = None;
        self.base.request_redraw();
    }
    /// Returns selection start position.
    pub fn selection_start(&self) -> Option<usize> {
        self.selection_start
    }
    /// Returns selected text.
    pub fn selected_text(&self) -> String {
        if let Some(start) = self.selection_start {
            let start = start.min(self.text.len());
            let end = self.cursor_position.min(self.text.len());
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            self.text[start..end].to_string()
        } else {
            String::new()
        }
    }
    /// Selects all text.
    pub fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor_position = self.text.len();
    }
    /// Clears selection.
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
    }
    /// Inserts text at cursor position.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Check max length and truncate if needed
        // SAFETY: `available` is bounded by `text.len()`, and we check
        // `text.len() > available` before slicing, so no panic occurs.
        // However, to avoid splitting a multi-byte UTF-8 character, we
        // use `floor_char_boundary` to ensure the slice is on a char boundary.
        let effective_text = if let Some(max) = self.max_length {
            let available = max.saturating_sub(self.text.len());
            if available == 0 {
                return;
            }
            if text.len() > available {
                let boundary = floor_char_boundary(text, available);
                &text[..boundary]
            } else {
                text
            }
        } else {
            text
        };
        // Handle selection
        let mut new_text = self.text.clone();
        if let Some(start) = self.selection_start {
            let start = start.min(new_text.len());
            let end = self.cursor_position.min(new_text.len());
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            new_text.replace_range(start..end, effective_text);
            self.cursor_position = start + effective_text.len();
        } else {
            new_text.insert_str(self.cursor_position, effective_text);
            self.cursor_position += effective_text.len();
        }
        self.selection_start = None;
        self.set_text(new_text);
    }
    /// Deletes selected text or character before cursor.
    pub fn backspace(&mut self) {
        if let Some(start) = self.selection_start {
            // Delete selection
            let start = start.min(self.text.len());
            let end = self.cursor_position.min(self.text.len());
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            let mut new_text = self.text.clone();
            new_text.replace_range(start..end, "");
            self.cursor_position = start;
            self.selection_start = None;
            self.set_text(new_text);
        } else if self.cursor_position > 0 {
            // Delete character before cursor
            let mut new_text = self.text.clone();
            new_text.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            self.set_text(new_text);
        }
    }
    /// Deletes selected text or character after cursor.
    pub fn delete(&mut self) {
        if let Some(_start) = self.selection_start {
            // Delete selection
            self.backspace(); // Same logic
        } else if self.cursor_position < self.text.len() {
            // Delete character after cursor
            let mut new_text = self.text.clone();
            new_text.remove(self.cursor_position);
            self.set_text(new_text);
        }
    }
    /// Returns whether the line edit is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Sets the read-only state.
    pub fn set_read_only(&mut self, ro: bool) {
        self.read_only = ro;
    }

    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }
    /// Copy the current selection to the platform clipboard (no-op when empty).
    #[cfg(not(alloc_frugal))]
    fn copy_selection_to_clipboard(&self) {
        let selection = self.selected_text();
        if !selection.is_empty() {
            crate::set_clipboard_text(&selection);
        }
    }
    /// Returns display text based on echo mode.
    ///
    /// `EchoMode` has exactly three modes, so this match is exhaustive without a
    /// catch-all: adding a mode to the canonical enum forces every renderer to
    /// decide what it looks like, instead of silently inheriting one.
    fn display_text(&self) -> String {
        match self.echo_mode {
            EchoMode::Normal => self.text.clone(),
            EchoMode::Password => "*".repeat(self.text.len()),
            EchoMode::NoEcho => String::new(),
        }
    }

    /// The field the control actually paints.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A text field is a *fixed-height band*: [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is the
    /// touch-sized content floor every field in this crate shares. Painting `rect` made a
    /// 240x120 census cell a 240x120 white slab — a rectangle pretending to be a field —
    /// and it also made `size_hint`'s 24 disagree with the ink by a factor of five.
    /// [`ControlMetrics::full_width_band`] keeps the full width, takes the field's own
    /// height and centres it, which is exactly what stops a 48 px field from drawing as a
    /// 120 px panel.
    ///
    /// Everything the control paints **and everything it hit-tests** is placed from this
    /// one box, so the clickable area cannot drift away from the ink.
    fn field_rect(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    // ---------------------------------------------------------------------------
    // Decoration slots
    // ---------------------------------------------------------------------------

    /// Returns the five decoration strings this field shows.
    pub fn decorations(&self) -> &DecorationSlots {
        &self.decorations
    }

    /// Replaces the whole decoration set at once.
    ///
    /// Prefer the individual setters when only one slot changes; this exists because a caller
    /// assembling a form from a document has all five at once and five separate calls would repaint
    /// five times.
    pub fn set_decorations(&mut self, decorations: DecorationSlots) {
        let changed = self.decorations != decorations;
        self.decorations = decorations;
        if changed {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Returns the unit marker written before the value.
    pub fn prefix(&self) -> &str {
        &self.decorations.prefix
    }

    /// Sets the unit marker written before the value.
    ///
    /// # Why this does not touch `text`
    ///
    /// The prefix is **chrome**, not content: it is drawn in its own box, so the caret can still sit at
    /// the start of what the user is editing and a select-all copies the value alone. Folding it into
    /// `text` would make all three of those wrong while looking the same on screen.
    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.prefix = prefix.into();
        self.set_decorations(next);
    }

    /// Returns the unit marker written after the value.
    pub fn suffix(&self) -> &str {
        &self.decorations.suffix
    }

    /// Sets the unit marker written after the value, anchored to the field's trailing edge.
    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.suffix = suffix.into();
        self.set_decorations(next);
    }

    /// Returns the quiet hint shown below the field.
    pub fn helper_text(&self) -> &str {
        &self.decorations.helper
    }

    /// Sets the quiet hint shown below the field, displaced by any error.
    pub fn set_helper_text(&mut self, helper: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.helper = helper.into();
        self.set_decorations(next);
    }

    /// Returns the refusal message shown below the field.
    pub fn error_text(&self) -> &str {
        &self.decorations.error
    }

    /// Sets the refusal message, which displaces the helper and paints in the theme's error colour.
    ///
    /// Pass an empty string to clear it. This does not *validate* anything — it is the caller's report
    /// — but [`Self::set_max_length`] does drive the counter and the over-limit state on its own,
    /// because the field knows both numbers itself.
    pub fn set_error_text(&mut self, error: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.error = error.into();
        self.set_decorations(next);
    }

    /// Returns the usage counter shown at the field's trailing lower edge, if any.
    ///
    /// Derived from the value's length and `max_length` rather than stored, so it cannot go stale: a
    /// counter the caller has to keep in step with the text is a counter that will disagree with it.
    pub fn counter_text(&self) -> Option<String> {
        DecorationLayout::counter_text(self.text.chars().count(), self.max_length)
    }

    /// Returns whether the current value exceeds `max_length`.
    ///
    /// The field's own answer, so an over-long value is *reported* rather than silently truncated.
    pub fn is_over_limit(&self) -> bool {
        DecorationLayout::over_limit(self.text.chars().count(), self.max_length) > 0
    }

    /// The boxes the field's five regions occupy, measured for the current font.
    ///
    /// # Why the layout is computed from `field_rect()` and not from `geometry()`
    ///
    /// The decoration slots belong to the **field**, which on a tall control is a band centred inside
    /// it. Measuring from the control's rectangle would put the helper row under the control instead of
    /// under the field, and the support row would be as wide as the cell rather than as the input.
    fn decoration_layout(&self, context: &mut RenderContext) -> DecorationLayout {
        let field = self.field_rect();
        let style = self.base.style().clone();
        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        let metrics = DecorationMetrics::measure(&self.decorations, |text| {
            context.measure_text(text, font).width
        });
        let counter_width =
            self.counter_text().map(|text| context.measure_text(&text, font).width).unwrap_or(0);
        let line_height = font.effective_line_height().max(1.0) as u32;
        DecorationLayout::compute(
            field,
            dimensions::TEXT_FIELD_PADDING_H,
            line_height,
            DECORATION_GAP,
            metrics,
            counter_width,
            &self.decorations,
        )
    }
}
// Implement Widget trait
impl Widget for LineEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let text_w = self.text().len() as u32 * 8 + 10;
        Size::new(text_w.max(80), 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    // The caret blink is the whole animation: without a trait-visible `tick` a host holding
    // `&mut dyn Widget` could not advance it, so the caret never blinked on screen.
    fn tick(&mut self, delta_ms: u32) -> bool {
        LineEdit::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.focused && !self.read_only
    }
}

/// `LineEdit`'s property contract.
///
/// # The decoration slots
///
/// `prefix`, `suffix`, `helper`, `error` and `counter` are all published here. They are the five strings
/// a text entry shows that are **not** its value, and before they existed the control could only be
/// announced and driven by its text and placeholder.
///
/// `counter` is published read-only: it is derived from the value's own length and `max_length`, so a
/// caller that could write it would be able to make the count disagree with the text it counts.
///
/// `echo_mode` is intentionally absent: the centralised layer never exposed it,
/// so publishing it here would add a property rather than preserve one.
impl WidgetProperties for LineEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder_text" => Ok(CapabilityValue::String(self.placeholder_text().to_string())),
            "max_length" => match self.max_length() {
                Some(len) => Ok(CapabilityValue::UInt(len as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "read_only" => Ok(CapabilityValue::Bool(self.is_read_only())),
            "cursor_position" => Ok(CapabilityValue::UInt(self.cursor_position() as u64)),
            "prefix" => Ok(CapabilityValue::String(self.prefix().to_string())),
            "suffix" => Ok(CapabilityValue::String(self.suffix().to_string())),
            "helper" => Ok(CapabilityValue::String(self.helper_text().to_string())),
            "error" => Ok(CapabilityValue::String(self.error_text().to_string())),
            "counter" => match self.counter_text() {
                Some(text) => Ok(CapabilityValue::String(text)),
                None => Ok(CapabilityValue::Null),
            },
            "over_limit" => Ok(CapabilityValue::Bool(self.is_over_limit())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder_text" => {
                self.set_placeholder_text(expect_string(value)?);
                Ok(())
            }
            "max_length" => {
                match value {
                    CapabilityValue::Null => self.set_max_length(None),
                    other => self.set_max_length(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "read_only" => {
                self.set_read_only(expect_bool(value)?);
                Ok(())
            }
            "prefix" => {
                self.set_prefix(expect_string(value)?);
                Ok(())
            }
            "suffix" => {
                self.set_suffix(expect_string(value)?);
                Ok(())
            }
            "helper" => {
                self.set_helper_text(expect_string(value)?);
                Ok(())
            }
            "error" => {
                self.set_error_text(expect_string(value)?);
                Ok(())
            }
            // Derived from the value and the limit: a writer would be a second way to say what the
            // text already determines, and one of the two would be able to disagree.
            "counter" | "over_limit" => Err(CapabilityAccessError::ReadOnlyProperty),
            "cursor_position" => {
                self.set_cursor_position(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `LINE_EDIT_PROPERTIES`.
        property_names_of![
            "text",
            "placeholder_text",
            "max_length",
            "read_only",
            "cursor_position",
            "prefix",
            "suffix",
            "helper",
            "error",
            "counter",
            "over_limit",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `line_edit` publishes.
    ///
    /// Both are zero-argument actions with a direct method on the control, so both
    /// run rather than being routed through the property path: `clear` assigns the
    /// empty text and `select_all` selects without changing it.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "select_all" => {
                self.select_all();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for LineEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.read_only {
            // Read-only fields still allow copying the current selection.
            #[cfg(not(alloc_frugal))]
            if let Event::KeyPress { key: 67, modifiers: 2 } = event {
                self.copy_selection_to_clipboard();
            }
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => {
                match *key {
                    8 => {
                        // Backspace
                        self.backspace();
                    }
                    46 => {
                        // Delete
                        self.delete();
                    }
                    13 => {
                        // Enter/Return
                        self.return_pressed.emit();
                        self.editing_finished.emit();
                    }
                    27 => {
                        // Escape
                        self.editing_finished.emit();
                    }
                    37 => {
                        // Left arrow
                        if self.cursor_position > 0 {
                            if modifiers & 1 != 0 {
                                if self.selection_start.is_none() {
                                    self.selection_start = Some(self.cursor_position);
                                }
                            } else {
                                self.selection_start = None;
                            }
                            self.cursor_position -= 1;
                        }
                    }
                    39 => {
                        // Right arrow
                        if self.cursor_position < self.text.len() {
                            if modifiers & 1 != 0 {
                                if self.selection_start.is_none() {
                                    self.selection_start = Some(self.cursor_position);
                                }
                            } else {
                                self.selection_start = None;
                            }
                            self.cursor_position += 1;
                        }
                    }
                    36 => {
                        // Home
                        if modifiers & 1 != 0 {
                            if self.selection_start.is_none() {
                                self.selection_start = Some(self.cursor_position);
                            }
                        } else {
                            self.selection_start = None;
                        }
                        self.cursor_position = 0;
                    }
                    35 => {
                        // End
                        if modifiers & 1 != 0 {
                            if self.selection_start.is_none() {
                                self.selection_start = Some(self.cursor_position);
                            }
                        } else {
                            self.selection_start = None;
                        }
                        self.cursor_position = self.text.len();
                    }
                    65 if modifiers & 2 != 0 => {
                        // Ctrl+A: Select all
                        self.select_all();
                    }
                    86 if modifiers & 2 != 0 => {
                        // Ctrl+V: Paste from the platform clipboard.
                        #[cfg(not(alloc_frugal))]
                        {
                            if !self.read_only {
                                let pasted = crate::get_clipboard_text();
                                if !pasted.is_empty() {
                                    self.insert_text(&pasted);
                                }
                            }
                        }
                        #[cfg(alloc_frugal)]
                        {
                            // Clipboard integration is unavailable in the mini profile.
                            self.base.redraw_requested.emit();
                        }
                    }
                    67 if modifiers & 2 != 0 => {
                        // Ctrl+C: Copy selection to the platform clipboard.
                        #[cfg(not(alloc_frugal))]
                        {
                            self.copy_selection_to_clipboard();
                        }
                        #[cfg(alloc_frugal)]
                        {
                            // Clipboard integration is unavailable in the mini profile.
                            self.base.redraw_requested.emit();
                        }
                    }
                    88 if modifiers & 2 != 0 => {
                        // Ctrl+X: Copy selection, then delete it.
                        #[cfg(not(alloc_frugal))]
                        {
                            let selection = self.selected_text();
                            if !selection.is_empty() {
                                crate::set_clipboard_text(&selection);
                                self.backspace(); // removes the selection
                            }
                        }
                        #[cfg(alloc_frugal)]
                        {
                            // Clipboard integration is unavailable in the mini profile.
                            self.base.redraw_requested.emit();
                        }
                    }
                    90 if modifiers & 2 != 0 => {
                        let _ = self.undo();
                    }
                    89 if modifiers & 2 != 0 => {
                        let _ = self.redo();
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.insert_text(&ch.to_string());
                            }
                        }
                    }
                }
            }
            Event::FocusLost => {
                self.set_focused(false);
                self.editing_finished.emit();
            }
            Event::FocusGained { .. } => {
                self.set_focused(true);
            }
            // A press focuses the field only when it lands on the **painted band**, not on
            // the control's rectangle. The two used to be the same box, so the hit test
            // silently agreed with the ink by accident; once the field became a centred
            // 48 px band in a 120 px cell, testing `geometry()` would let a user focus the
            // field by clicking 60 px below it — on the window background, nowhere near
            // any ink. The test is against `field_rect()` for exactly that reason.
            Event::MousePress { pos, button }
                if *button == 1 && self.field_rect().contains_point(*pos) =>
            {
                self.set_focused(true);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for LineEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        //
        // The **field**, not the control's rectangle: see `field_rect`. Every measurement
        // below — the fill, the border, the text's line box and the caret's top and bottom
        // — is taken from this one box, so they cannot disagree about where the field is.
        let rect = self.field_rect();
        let style = self.style();
        // ── The decorated layout ──
        //
        // Every box below comes from **one** derivation, measured against the renderer's own font. The
        // value's origin, the two slots and the support row cannot disagree, because they are the same
        // answer read for different purposes. `text_x` used to be `rect.x + padding` regardless of any
        // slot, so a `$` would have been drawn *over* the value it marks.
        let layout = self.decoration_layout(context);
        let text_x = layout.value.x;
        // Draw background
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            let bw = style.border_width.unwrap_or(0);
            if bw > 0 {
                context.draw_rect_stroke(
                    Rect::new(rect.x, rect.y, rect.width, rect.height),
                    border_color,
                    bw,
                );
            } else {
                context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), border_color);
            }
        }
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.display_text()
        };
        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        let value_line = context.text_line(rect, font);
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        if !display_text.is_empty() {
            // The field's own line box. A glyph origin is the box's top-left edge, so the
            // previous `rect.y + rect.height / 2` placed that edge on the field's middle line
            // and drew the value half a line low.
            context.draw_text(
                Point::new(text_x, value_line.y),
                display_text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // ── The in-field slots ──
        //
        // Drawn in their **own** boxes, in a muted tone of the field's ink so they read as unit marks
        // rather than as content. They are deliberately not folded into `display_text`: the caret below
        // is measured against `self.text` alone, so a prefix in the string would put `cursor_position
        // == 0` after the `$` and make the start of the value unreachable.
        let slot_color = text_color.blend(&bg, 0.35);
        if let Some(prefix_box) = layout.prefix {
            context.draw_text(
                Point::new(prefix_box.x, value_line.y),
                &self.decorations.prefix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }
        if let Some(suffix_box) = layout.suffix {
            context.draw_text(
                Point::new(suffix_box.x, value_line.y),
                &self.decorations.suffix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }

        // ── The support row ──
        //
        // Message on the leading edge, counter on the trailing one. The error takes the theme's error
        // colour, because a refusal that is painted in the same ink as a hint is a refusal the user has
        // to read to notice.
        if let Some(row) = layout.support {
            let message = self.decorations.support_message();
            if !message.is_empty() {
                let message_color = if self.decorations.has_error() {
                    crate::style::resolved_theme_style("line_edit")
                        .and_then(|theme| theme.border_color)
                        .map(|border| border.blend(&Color::rgb(220, 40, 40), 0.6))
                        .unwrap_or(Color::rgb(200, 40, 40))
                } else {
                    slot_color
                };
                let row_line = context.text_line(row, font);
                context.draw_text(
                    Point::new(row.x, row_line.y),
                    message,
                    font,
                    message_color,
                    HorizontalAlignment::Left,
                );
            }
            if let Some(counter_box) = layout.counter {
                let counter_line = context.text_line(counter_box, font);
                // The counter is a *budget* reading, so it turns to the error colour once the value
                // exceeds the limit — the one moment it has something to warn about.
                let counter_color =
                    if self.is_over_limit() { Color::rgb(200, 40, 40) } else { slot_color };
                if let Some(counter) = self.counter_text() {
                    context.draw_text(
                        Point::new(counter_box.x, counter_line.y),
                        &counter,
                        font,
                        counter_color,
                        HorizontalAlignment::Right,
                    );
                }
            }
        }
        // Draw the caret for whichever field owns keyboard focus.
        if self.focused && !self.read_only && self.cursor_blink.is_visible() {
            // The caret sits at **`cursor_position`**, not at the end of the value.
            //
            // This measured the whole string, so the marker was always drawn after the last
            // glyph however the control was positioned: with `text == "Sample"` and
            // `cursor_position == 0`, the caret rendered past the `e`. The field answers
            // `cursor_position`, `set_cursor_position` clamps it, and `backspace`/`delete`
            // both act on it — so the cursor was the one thing in the field that ignored the
            // very field that defines it, and every editing affordance appeared to work at the
            // wrong end of the text.
            //
            // The slice is taken through `floor_char_boundary` because `cursor_position` indexes
            // bytes and the value may be multi-byte; slicing mid-character would panic, which is
            // why the import for it was already present in this file.
            let caret_byte = floor_char_boundary(self.text.as_str(), self.cursor_position);
            let prefix = &self.text[..caret_byte];
            let caret_x = text_x + context.measure_text(prefix, font).width as i32;
            // The marker is clipped to the **value's** box. A caret beyond the visible text (a value
            // wider than the room it has) belongs at the last pixel a user can see, not outside the
            // control and not under the suffix. Clipping to the whole field instead would let the caret
            // sit on top of a `%` and read as if the unit were part of the value.
            let caret_x = caret_x.min(layout.value.x + layout.value.width as i32);
            let caret_x = caret_x.max(layout.value.x);
            // The caret spans the field's own content band, inset so it does not touch the
            // border. The band is the field's, not the control's: with the field centred in
            // a tall cell, `rect` alone would have drawn a caret taller than the field it
            // belongs to.
            let caret_top = rect.y + 2;
            let caret_bottom = rect.y + rect.height as i32 - 2;
            let caret_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
            context.draw_line(
                Point::new(caret_x, caret_top),
                Point::new(caret_x, caret_bottom),
                caret_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{MiniToString, Vec};
    use crate::core::Rect;

    #[test]
    fn lineedit_creation_defaults() {
        let le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.text().is_empty());
        assert!(le.placeholder_text().is_empty());
        assert_eq!(le.max_length(), None);
        assert_eq!(le.echo_mode(), EchoMode::Normal);
        assert_eq!(le.cursor_position(), 0);
        assert!(le.selection_start().is_none());
        assert!(!le.is_focused(), "a fresh field is not focused");
    }

    /// The caret blinks only while the field holds focus, and it stops asking for frames when it
    /// does not.
    ///
    /// The caret was a solid line for the whole life of the field, so `tick` did not exist and a
    /// host had no signal about whether more frames were owed. This pins the contract in both
    /// directions: a blurred field is inert, and a focused one keeps reporting work until it
    /// loses focus again.
    #[test]
    fn the_caret_blinks_while_focused_and_is_inert_while_blurred() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

        // Blurred: no frames owed, whatever the delta.
        assert!(!le.tick(10_000), "a blurred field owes no frame");

        // Focused: it reports work, and keeps doing so because a blink never settles.
        le.set_focused(true);
        assert!(le.tick(0), "a focused field is blinking");
        for _ in 0..10 {
            assert!(le.tick(500), "a blink is periodic, so it never reports settled");
        }

        // A read-only field shows no caret, so it must not animate one either.
        le.set_focused(false);
        assert!(!le.tick(500));
    }

    /// The field must learn about focus from the events the runtime sends, since
    /// that is what gates the caret. Before this the field had no focus state, so
    /// `FocusGained` was ignored and the caret was never drawn.
    #[test]
    fn lineedit_tracks_focus_from_events() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

        le.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(le.is_focused(), "FocusGained must mark the field focused");

        le.handle_event(&Event::FocusLost);
        assert!(!le.is_focused(), "FocusLost must clear it");
    }

    /// Setting focus directly must be idempotent in effect: the flag is a boolean,
    /// so a repeated set cannot leave it in a contradictory state.
    #[test]
    fn lineedit_set_focused_round_trips() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_focused(true);
        assert!(le.is_focused());
        le.set_focused(true);
        assert!(le.is_focused());
        le.set_focused(false);
        assert!(!le.is_focused());
    }

    /// A focused field must actually paint a caret. The previous code drew nothing,
    /// and the comment said why — there was no focus state to read.
    #[test]
    fn focused_lineedit_draws_a_caret() {
        let rect = Rect::new(0, 0, 200, 24);
        let mut focused = LineEdit::new(rect);
        focused.set_text("ab");
        focused.set_focused(true);

        let mut unfocused = LineEdit::new(rect);
        unfocused.set_text("ab");

        let focused_frame = render(&mut focused, rect);
        let unfocused_frame = render(&mut unfocused, rect);
        assert_ne!(
            focused_frame, unfocused_frame,
            "focusing must change what is painted, because it draws the caret"
        );
    }

    /// Renders a widget into an RGBA frame for comparison.
    fn render(widget: &mut LineEdit, rect: Rect) -> Vec<u8> {
        use crate::core::{Color, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::Draw;

        let mut surface = SoftwarePaintBackend::new(Size::new(rect.width, rect.height), 1.0);
        surface.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut surface);
            widget.draw(&mut context);
        }
        surface.end_frame();
        surface.frame_rgba().to_vec()
    }

    #[test]
    fn lineedit_set_text() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        assert_eq!(le.text(), "Hello");
        assert_eq!(le.cursor_position(), 5);
    }

    #[test]
    fn lineedit_set_text_empty() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.set_text(String::new());
        assert!(le.text().is_empty());
        assert_eq!(le.cursor_position(), 0);
    }

    #[test]
    fn lineedit_undo_redo_restores_text() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("one");
        le.set_text("two");
        assert!(le.can_undo());
        assert!(le.undo());
        assert_eq!(le.text(), "one");
        assert!(le.can_redo());
        assert!(le.redo());
        assert_eq!(le.text(), "two");
    }

    #[test]
    fn lineedit_control_z_and_control_y_drive_history() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("before");
        le.set_text("after");
        le.handle_event(&Event::key_press(90, 2));
        assert_eq!(le.text(), "before");
        le.handle_event(&Event::key_press(89, 2));
        assert_eq!(le.text(), "after");
    }

    #[test]
    fn lineedit_placeholder() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.placeholder_text().is_empty());
        le.set_placeholder_text("Enter name".to_string());
        assert_eq!(le.placeholder_text(), "Enter name");
        le.set_placeholder_text(String::new());
        assert!(le.placeholder_text().is_empty());
    }

    #[test]
    fn lineedit_max_length() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert_eq!(le.max_length(), None);
        le.set_max_length(Some(5));
        assert_eq!(le.max_length(), Some(5));
        le.set_max_length(None);
        assert_eq!(le.max_length(), None);
    }

    #[test]
    fn lineedit_max_length_truncates() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello World".to_string());
        le.set_max_length(Some(5));
        assert_eq!(le.text(), "Hello");
    }

    #[test]
    fn lineedit_cursor_position() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.set_cursor_position(3);
        assert_eq!(le.cursor_position(), 3);
        le.set_cursor_position(100); // clamps to text len
        assert_eq!(le.cursor_position(), 5);
    }

    #[test]
    fn lineedit_echo_mode() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert_eq!(le.echo_mode(), EchoMode::Normal);
        le.set_echo_mode(EchoMode::Password);
        assert_eq!(le.echo_mode(), EchoMode::Password);
        le.set_echo_mode(EchoMode::NoEcho);
        assert_eq!(le.echo_mode(), EchoMode::NoEcho);
        le.set_echo_mode(EchoMode::Normal);
        assert_eq!(le.echo_mode(), EchoMode::Normal);
    }

    /// The widget layer and the platform layer must name the **same** echo-mode
    /// enum, so a mode read back from a native control can be handed straight to
    /// this widget. Distinct types would make the assignment below ill-typed.
    #[test]
    fn lineedit_shares_the_canonical_echo_mode_type() {
        let canonical: crate::platform::EchoMode = crate::platform::EchoMode::Password;
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_echo_mode(canonical);
        assert_eq!(le.echo_mode(), canonical);
    }

    #[test]
    fn lineedit_select_all() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello World".to_string());
        le.select_all();
        assert_eq!(le.selection_start(), Some(0));
        assert_eq!(le.cursor_position(), 11);
        assert_eq!(le.selected_text(), "Hello World");
    }

    #[test]
    fn lineedit_clear_selection() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.select_all();
        le.clear_selection();
        assert!(le.selection_start().is_none());
    }

    #[test]
    fn lineedit_insert_text() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.insert_text("Hello");
        assert_eq!(le.text(), "Hello");
    }

    #[test]
    fn lineedit_backspace() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.backspace();
        assert_eq!(le.text(), "Hell");
    }

    #[test]
    fn lineedit_delete() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.set_cursor_position(0);
        le.delete();
        assert_eq!(le.text(), "ello");
    }

    #[test]
    fn lineedit_clear() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.clear();
        assert!(le.text().is_empty());
    }

    #[test]
    fn lineedit_geometry_delegation() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_geometry(Rect::new(10, 10, 150, 30));
        assert_eq!(le.geometry(), Rect::new(10, 10, 150, 30));
    }

    /// The field is a full-width band one text-field height tall, centred in the control.
    ///
    /// The control used to paint its whole rectangle, so a 240x120 census cell drew a
    /// 240x120 white slab — a panel, not a field — and the drawn height disagreed with
    /// `size_hint`'s 24 by a factor of five. This pins the shared `full_width_band`
    /// derivation in both directions: the height is the table's constant whatever the
    /// cell, and a control smaller than the field clamps rather than painting outside.
    #[test]
    fn the_field_is_a_text_field_height_in_any_rectangle() {
        for height in [48u32, 120, 300] {
            let le = LineEdit::new(Rect::new(0, 0, 240, height));
            let field = le.field_rect();
            assert_eq!(
                field.height,
                dimensions::TEXT_FIELD_MIN_HEIGHT,
                "at control height {height}"
            );
            assert_eq!(field.width, 240, "the field spans the control's width");
            assert_eq!(field.y, (height - field.height) as i32 / 2, "at control height {height}");
        }

        let short = LineEdit::new(Rect::new(0, 0, 240, 20));
        assert_eq!(short.field_rect().height, 20, "a short control clamps the field");
    }

    /// Hit-testing follows the ink: a press below the field does not focus it.
    ///
    /// With the field centred in a 120 px cell, a press on the control's rectangle but
    /// 60 px below the drawn band belongs to the window background. The control must not
    /// claim it — the clickable area has to be the one a user can see.
    #[test]
    fn a_press_outside_the_drawn_band_does_not_focus_the_field() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        let field = le.field_rect();

        // Inside the band focuses.
        le.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 / 2),
            button: 1,
        });
        assert!(le.is_focused(), "a press on the drawn field focuses it");

        // Below the band, inside the control's rectangle, does not.
        le.set_focused(false);
        le.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 + 40),
            button: 1,
        });
        assert!(!le.is_focused(), "a press below the drawn field must not focus it");
    }

    /// The value is drawn on the field's own middle line.
    ///
    /// The origin of a text run is its glyph box's top-left corner, so the old
    /// `rect.y + rect.height / 2` put that corner on the field's middle line and drew the
    /// value half a line low. Pinning the drawn line box — not the origin — to the centre is
    /// what keeps the two from agreeing by accident.
    ///
    /// # Why the check reads the ink
    ///
    /// The value is no longer a `<text>` element carrying a `y`: the backend emits the same
    /// `font8x8` rectangles the software rasteriser fills, as subpaths of one `<path>` (see
    /// `crate::widget::svg::text_ink_box`). The ink box is also the better witness, because it
    /// is where the glyphs actually landed rather than what an element claimed.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_value_sits_on_the_fields_middle_line() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("Sample");
        let svg = crate::widget::svg::render_to_svg(&mut le);
        let field = le.field_rect();

        let (_, top, _, bottom) = crate::widget::svg::text_ink_box(&svg)
            .expect("a field with a value draws it as glyph geometry");
        assert!(top >= field.y, "the value starts inside the field: y={top}");
        assert!(
            bottom <= field.y + field.height as i32,
            "and above its bottom edge: y={bottom}, field={field:?}"
        );
        // And it is *centred*, not merely contained: the field's own line box is the reference
        // the value was moved onto, so its ink shares that box's middle line. A value left on
        // the old `rect.y + rect.height / 2` anchor sits a whole half line below it.
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
        let context = RenderContext::new(&mut backend);
        let font = crate::core::Font::default();
        let line = context.text_line(field, &font);
        assert_eq!(
            top + bottom,
            line.y * 2 + line.height as i32,
            "the value hangs on the field's own line box middle line"
        );
    }

    /// The value's horizontal origin is the field's own padding.
    ///
    /// A field's text is inset from its edge by [`dimensions::TEXT_FIELD_PADDING_H`]; the
    /// origin was a local literal `4`, which is a different fact written in a second place.
    ///
    /// # Why the edge is read off the ink
    ///
    /// There is no `x` attribute to read any more: the backend emits the string as `font8x8`
    /// glyph rectangles inside one `<path>` (see `crate::widget::svg::text_ink_box`), so the
    /// run's left edge is where the pen actually put its first set bit. `context.text_line`
    /// centres the line box's *height*, not each cluster, so the pen itself is `field.x +
    /// TEXT_FIELD_PADDING_H` exactly; what is left over is the first glyph's own blank lead
    /// column, which the `font8x8` table gives as 0 for `S`. The assertion is still a real
    /// constraint on the padding: the pen is what the padding places, and the test would read
    /// `padding + 20` (or any other literal) instead of `padding` if the draw site stopped
    /// reading [`dimensions::TEXT_FIELD_PADDING_H`].
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_value_starts_at_the_fields_horizontal_padding() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("Sample");
        let svg = crate::widget::svg::render_to_svg(&mut le);
        // The fixture paints only the field's own chrome — a fill and a border — so what is
        // left is the value's ink, which no other string in this control can supply.
        assert_eq!(svg.matches("<rect").count(), 2, "only the field's fill and border are rects");
        let field = le.field_rect();

        let (left, _, _, _) = crate::widget::svg::text_ink_box(&svg)
            .expect("a field with a value draws it as glyph geometry");
        assert_eq!(
            left,
            field.x + dimensions::TEXT_FIELD_PADDING_H as i32,
            "the value starts at the field's own padding: field={field:?}"
        );
    }

    // ─── Decoration slots (F-12) ───

    /// A field that never sets a slot must paint exactly as it did, and reserve nothing extra.
    ///
    /// This is the non-change guarantee: the slots are additive, so every existing caller keeps its
    /// pixels and its height.
    #[test]
    fn a_field_with_no_slots_is_unchanged() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("Sample");
        assert!(le.decorations().is_empty());
        assert_eq!(le.get("prefix").unwrap().as_str(), Some(""));
        assert_eq!(le.get("counter").unwrap(), CapabilityValue::Null, "no limit means no counter");

        let svg = crate::widget::svg::render_to_svg(&mut le);
        // The value is still the only ink, and it still starts at the field's padding.
        let (left, _, _, _) = crate::widget::svg::text_ink_box(&svg).expect("the value is drawn");
        assert_eq!(left, le.field_rect().x + dimensions::TEXT_FIELD_PADDING_H as i32);
    }

    /// A prefix is drawn **before** the value, and the value moves right to make room.
    ///
    /// # The defect this pins
    ///
    /// The shortcut is to concatenate `prefix + value` into one string. That draws nearly the same
    /// glyphs in nearly the same place, so it looks right — while it also (a) makes `cursor_position ==
    /// 0` render after the `$`, and (b) makes a select-all copy the `$`. The distinguishing fact is that
    /// the two are **separate ink runs** with the prefix to the left of the value, so the test reads all
    /// runs and orders them rather than trusting which one was painted first.
    #[test]
    fn a_prefix_is_drawn_before_the_value_and_shifts_it() {
        let mut plain = LineEdit::new(Rect::new(0, 0, 240, 120));
        plain.set_text("12");
        let plain_svg = crate::widget::svg::render_to_svg(&mut plain);
        let plain_runs = crate::widget::svg::text_ink_boxes(&plain_svg);
        assert_eq!(plain_runs.len(), 1, "a plain field draws its value as one run");
        let (plain_left, _, plain_right, _) = plain_runs[0];

        let mut prefixed = LineEdit::new(Rect::new(0, 0, 240, 120));
        prefixed.set_text("12");
        prefixed.set_prefix("$");
        let prefixed_svg = crate::widget::svg::render_to_svg(&mut prefixed);
        let mut runs = crate::widget::svg::text_ink_boxes(&prefixed_svg);
        assert_eq!(
            runs.len(),
            2,
            "the prefix and the value are two runs, not one concatenated string"
        );
        runs.sort_by_key(|b| b.0);
        let (prefix_left, prefix_right) = (runs[0].0, runs[0].2);
        let (value_left, value_right) = (runs[1].0, runs[1].2);

        // The `$` takes the field's leading padding. The run's left edge is the *ink*, and the `$`
        // bitmap has no set bit in its leftmost column, so the ink starts a glyph-bit right of the
        // origin — the same inset a value at the padding shows. Asserting the origin would need the
        // pen position, which the SVG does not carry; asserting the relation to the plain field is the
        // honest form.
        assert_eq!(
            prefix_left, plain_left,
            "the prefix starts where a value with no prefix starts: the field's leading padding"
        );
        // …and the value is pushed clear of it rather than starting there too.
        assert!(
            value_left > prefix_right,
            "the value must begin past the prefix: prefix ends at {prefix_right}, value at {value_left}"
        );
        assert!(
            value_left > plain_left,
            "and further right than it sat without a prefix ({plain_left})"
        );
        // The value's own ink is the same width as before — only its origin moved, so the prefix did not
        // disturb the glyphs it marks.
        assert_eq!(
            value_right - value_left,
            plain_right - plain_left,
            "the prefix shifted the value without reshaping it"
        );

        assert_eq!(prefixed.get("prefix").unwrap().as_str(), Some("$"));
        assert!(
            plain_left >= plain.field_rect().x + dimensions::TEXT_FIELD_PADDING_H as i32,
            "the value is inset by the field's padding, not on the border"
        );
    }

    /// The caret is measured against the **value**, so at `cursor_position == 0` it sits at the value's
    /// origin — after a prefix, not after the `$`'s own box.
    ///
    /// This is the property that makes keeping the slots separate worth the trouble: with the prefix
    /// folded into the text, `cursor_position == 0` would place the caret to the right of the `$` and
    /// the start of the value would be unreachable.
    #[test]
    fn the_caret_at_position_zero_sits_at_the_values_origin_not_after_the_prefix() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_prefix("$");
        le.set_text("12");
        le.set_cursor_position(0);
        le.set_focused(true);

        // The layout is computed against the **real** SVG backend's metrics — the same backend the
        // painter uses — and then read for the two facts that matter: the prefix has a box, and the
        // value begins past it.
        let mut backend = crate::render::svg::SvgPaintBackend::new(le.geometry().size());
        let layout = {
            let mut context = crate::render::RenderContext::new(&mut backend);
            le.decoration_layout(&mut context)
        };
        let prefix = layout.prefix.expect("the prefix has a box of its own");
        assert!(
            layout.value.x >= prefix.right(),
            "the value must begin at or past the prefix's trailing edge: {layout:?}"
        );
        assert!(layout.value.width > 0, "and it must keep some room: {layout:?}");
    }

    /// An error displaces the helper and paints on its own row; the counter shares that row and is
    /// derived from the value and the limit.
    #[test]
    fn the_support_row_shows_the_error_and_a_derived_counter() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_max_length(Some(5));
        le.set_text("abc");
        assert_eq!(le.get("counter").unwrap().as_str(), Some("3/5"));
        assert_eq!(le.get("over_limit").unwrap().as_bool(), Some(false));

        // The helper shows while there is no error…
        le.set_helper_text("Up to five characters");
        assert_eq!(le.decorations().support_message(), "Up to five characters");
        assert!(le.decorations().needs_support_row());

        // …and the error displaces it rather than joining it.
        le.set_error_text("Too long");
        assert_eq!(le.decorations().support_message(), "Too long");
        assert!(le.decorations().has_error());
        assert_eq!(le.get("error").unwrap().as_str(), Some("Too long"));
        assert_eq!(
            le.get("helper").unwrap().as_str(),
            Some("Up to five characters"),
            "the helper is displaced, not deleted"
        );

        // Over the limit is reported, and the counter says how far. `set_text` enforces the limit too —
        // a limit the control honours only while typing is a hint, not a limit — so the value is clamped
        // and the pair stays consistent.
        le.set_text("abcdefgh");
        assert_eq!(le.text(), "abcde", "`set_text` must honour `max_length` like typing does");
        assert_eq!(le.get("counter").unwrap().as_str(), Some("5/5"));
        assert_eq!(
            le.get("over_limit").unwrap().as_bool(),
            Some(false),
            "the stored value cannot exceed a limit it was clamped to"
        );

        // Clearing the error restores the helper without the caller having to re-set it.
        le.set_error_text("");
        assert_eq!(le.decorations().support_message(), "Up to five characters");
    }

    /// The counter is read-only, and the two in-field slots round-trip through the contract.
    #[test]
    fn the_slots_round_trip_through_the_property_api() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set("prefix", CapabilityValue::String("$".to_string())).unwrap();
        le.set("suffix", CapabilityValue::String("%".to_string())).unwrap();
        le.set("helper", CapabilityValue::String("Hint".to_string())).unwrap();
        le.set("error", CapabilityValue::String("Bad".to_string())).unwrap();

        assert_eq!(le.get("prefix").unwrap().as_str(), Some("$"));
        assert_eq!(le.get("suffix").unwrap().as_str(), Some("%"));
        assert_eq!(le.get("helper").unwrap().as_str(), Some("Hint"));
        assert_eq!(le.get("error").unwrap().as_str(), Some("Bad"));

        // The two derived names are refused, so there is no second writer to disagree with.
        assert!(le.set("counter", CapabilityValue::String("1/1".to_string())).is_err());
        assert!(le.set("over_limit", CapabilityValue::Bool(true)).is_err());
    }

    /// A field too narrow for its slots still draws its field chrome and does not panic.
    #[test]
    fn a_field_squeezed_by_its_slots_still_paints() {
        let mut le = LineEdit::new(Rect::new(0, 0, 40, 120));
        le.set_prefix("verylongprefix");
        le.set_suffix("verylongsuffix");
        le.set_text("value");
        let svg = crate::widget::svg::render_to_svg(&mut le);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn lineedit_visibility() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.is_visible());
        le.hide();
        assert!(!le.is_visible());
        le.show();
        assert!(le.is_visible());
    }

    #[test]
    fn lineedit_enabled() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.is_enabled());
        le.set_enabled(false);
        assert!(!le.is_enabled());
        le.set_enabled(true);
        assert!(le.is_enabled());
    }

    #[test]
    fn lineedit_id_kind() {
        let le_a = LineEdit::new(Rect::new(0, 0, 100, 24));
        let le_b = LineEdit::new(Rect::new(0, 0, 100, 24));
        assert_ne!(le_a.id(), le_b.id());
        assert_eq!(le_a.kind(), WidgetKind::LineEdit);
        assert_eq!(le_b.kind(), WidgetKind::LineEdit);
    }

    #[test]
    fn lineedit_signal_accessors() {
        let le = LineEdit::new(Rect::new(0, 0, 100, 24));
        let _text_changed = &le.text_changed;
        let _editing_finished = &le.editing_finished;
        let _return_pressed = &le.return_pressed;
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_caret_is_drawn_at_cursor_position_not_at_the_end() {
        // The caret used to be measured against the **whole** value, so it was always drawn
        // after the last glyph however the control was positioned: with `cursor_position = 0` it
        // still sat past the `e` of "Sample". The field answers `cursor_position`, clamps it on
        // every write, and acts on it in `backspace`/`delete`, so the marker was the one part of
        // the field ignoring the field's own cursor.
        //
        // The assertion reads the drawn geometry out of the SVG, because the defect is entirely
        // about where the ink lands — asserting the stored index would have passed before the fix
        // too, which is why it survived.
        fn caret_x(cursor_position: usize) -> i32 {
            let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
            field.set_text("Sample".to_string());
            // `focus()` places the caret at the end for the autofocus case, so the requested
            // position is applied *after* it — the same order a caller that positions a caret
            // uses.
            field.set_focused(true);
            field.set_cursor_position(cursor_position);
            let svg = crate::widget::svg::render_to_svg(&mut field);
            // The caret is the only vertical line the control draws, emitted as a `<line>` with
            // `x1 == x2`.
            svg.lines()
                .filter(|line| line.contains("<line"))
                .find_map(|line| {
                    let x1 = line.split("x1=\"").nth(1)?.split('"').next()?;
                    let x2 = line.split("x2=\"").nth(1)?.split('"').next()?;
                    if x1 != x2 {
                        return None;
                    }
                    x1.parse::<i32>().ok()
                })
                .expect("a focused, editable field must draw its caret as a vertical line")
        }

        let at_start = caret_x(0);
        let at_three = caret_x(3);
        let at_end = caret_x(6);
        // This assertion used to pin the literal `4`, which was a local padding constant
        // written at the draw site. The origin is now [`dimensions::TEXT_FIELD_PADDING_H`],
        // the same table value every other field insets its content by, so what this pins is
        // "the caret starts at the field's own horizontal padding" — the fact the old
        // literal was an unshared spelling of, not a different fact.
        assert_eq!(
            at_start,
            dimensions::TEXT_FIELD_PADDING_H as i32,
            "a caret at position 0 sits at the text origin (the field's padding inset)"
        );
        assert!(
            at_start < at_three && at_three < at_end,
            "the caret must advance with the cursor position, not jump to the end \
             ({at_start}, {at_three}, {at_end})"
        );
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn lineedit_clipboard_copy_paste_cut() {
        use crate::event::Event::KeyPress;
        // The clipboard is process-wide, so this test must not run concurrently with any
        // other test that copies or pastes. Without the guard it failed intermittently
        // under the parallel harness while passing on its own.
        let _clipboard = crate::clipboard::clipboard_test_guard();

        let mut source = LineEdit::new(Rect::new(0, 0, 200, 24));
        source.set_text("hello world");
        source.select_all();
        // Ctrl+C copies the selection to the platform clipboard.
        source.handle_event(&KeyPress { key: 67, modifiers: 2 });
        assert_eq!(crate::get_clipboard_text(), "hello world");

        // Ctrl+V pastes into another field.
        let mut target = LineEdit::new(Rect::new(0, 0, 200, 24));
        target.handle_event(&KeyPress { key: 86, modifiers: 2 });
        assert_eq!(target.text(), "hello world");

        // Ctrl+X on a selected field copies then removes the selection.
        source.select_all();
        source.handle_event(&KeyPress { key: 88, modifiers: 2 });
        assert_eq!(source.text(), "");
        assert_eq!(crate::get_clipboard_text(), "hello world");

        // Read-only fields still copy via Ctrl+C.
        let mut read_only = LineEdit::new(Rect::new(0, 0, 200, 24));
        read_only.set_text("secret");
        read_only.set_read_only(true);
        read_only.select_all();
        read_only.handle_event(&KeyPress { key: 67, modifiers: 2 });
        assert_eq!(crate::get_clipboard_text(), "secret");
        // …but editing stays blocked.
        assert_eq!(read_only.text(), "secret");
    }
}
