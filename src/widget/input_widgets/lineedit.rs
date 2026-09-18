// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Single-line text edit widget.
use crate::compat::{Box, Rc, RefCell, String, ToString};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{TextSnapshotCommand, UndoStack};

use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::text_utils::floor_char_boundary;
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
        self.base.request_redraw();
    }
    /// Sets text and emits text_changed signal if different.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
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
}

/// `LineEdit`'s property contract.
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
            Event::FocusGained => {
                self.set_focused(true);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for LineEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let style = self.style();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y as f32 + rect.height as f32 / 2.0;
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
        if !display_text.is_empty() {
            let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
            let default_font = crate::core::Font::default();
            let font = style.font.as_ref().unwrap_or(&default_font);
            context.draw_text(
                Point::new(text_x, text_y as i32),
                display_text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
        // Draw the caret for whichever field owns keyboard focus.
        if self.focused && !self.read_only {
            // The caret sits after the text drawn so far — measured with the same font
            // the text was drawn with, so it tracks the character count rather than
            // the pixel count. Before this, nothing was drawn at all: the field had no
            // focus state to consult.
            let default_font = crate::core::Font::default();
            let font = style.font.as_ref().unwrap_or(&default_font);
            let caret_x = text_x + context.measure_text(display_text, font).width as i32;
            // Inset so the caret does not touch the border.
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

    /// The field must learn about focus from the events the runtime sends, since
    /// that is what gates the caret. Before this the field had no focus state, so
    /// `FocusGained` was ignored and the caret was never drawn.
    #[test]
    fn lineedit_tracks_focus_from_events() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

        le.handle_event(&Event::FocusGained);
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
    fn lineedit_clipboard_copy_paste_cut() {
        use crate::event::Event::KeyPress;

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
