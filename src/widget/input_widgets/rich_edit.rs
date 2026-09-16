// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rich text editor widget.
use crate::core::HorizontalAlignment;
use crate::core::Rect;
use crate::impl_widget_property_hooks;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;

/// Rich text/code editor baseline widget contract.
///
/// Holds a plain `String` plus a byte-offset selection and a set of change
/// signals. Despite the name it stores no styling or markup: it is a text
/// buffer with editor signalling, and "rich" refers to the role it fills (the
/// editing surface that richer tooling builds on) rather than to its contents.
///
/// Offsets in the selection and cursor signals are **byte** indices into the
/// UTF-8 text, not character indices, so they are not guaranteed to land on
/// character boundaries unless the writer keeps them there.
///
pub struct RichEdit {
    base: BaseWidget,
    text: String,
    selection: Option<(usize, usize)>,
    read_only: bool,
    /// Emitted with the full new text on every accepted change. Carries the
    /// whole document, not a delta.
    pub text_changed: Signal1<String>,
    /// Emitted with the new selection as byte offsets `(start, end)`, or `None`
    /// when the selection is cleared.
    pub selection_changed: Signal1<Option<(usize, usize)>>,
    /// Emitted with the new flag from [`RichEdit::set_read_only`].
    pub read_only_changed: Signal1<bool>,
    /// Emitted with the new cursor byte offset whenever the text is replaced.
    /// Note the name does not match the payload: it reports an offset, not a
    /// position struct.
    pub cursor_position_changed: Signal1<usize>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}
impl RichEdit {
    /// Creates an empty rich editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "RichEdit"),
            text: String::new(),
            selection: None,
            read_only: false,
            text_changed: Signal1::new(),
            selection_changed: Signal1::new(),
            read_only_changed: Signal1::new(),
            cursor_position_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }
    /// Returns current editor text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Replaces editor text and resets selection/cursor to end.
    ///
    /// Ignored entirely — no change, no signals — when the editor is read-only
    /// or when `text` equals the current content. Otherwise the previous content
    /// is pushed onto the undo stack (unless an undo/redo is being replayed),
    /// the selection is cleared, and `text_changed` then `cursor_position_changed`
    /// fire, followed by a redraw request.
    ///
    /// The cursor is reported as `text.len()`, a byte offset at the end of the
    /// document.
    pub fn set_text(&mut self, text: String) {
        if self.read_only || self.text == text {
            return;
        }
        let before = self.text.clone();
        self.text = text;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "rich_edit_text",
            )));
        }
        self.selection = None;
        self.text_changed.emit(self.text.clone());
        self.cursor_position_changed.emit(self.text.len());
        self.base.request_redraw();
    }

    /// Steps back one text change and returns `true`, or `false` when there is
    /// nothing to undo.
    ///
    /// Replaying history emits `text_changed` but does not consult the read-only
    /// flag, so an undo can alter the text of a read-only editor. No undo entry
    /// is created for the replay.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Steps forward one undone change and returns `true`, or `false` when there
    /// is nothing to redo. Signal behaviour matches [`RichEdit::undo`], including
    /// the ignored read-only flag.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns `true` if [`RichEdit::undo`] would change the text.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`RichEdit::redo`] would change the text.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        self.restoring_history = true;
        self.text = text;
        self.selection = Some((self.text.len(), self.text.len()));
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.cursor_position_changed.emit(self.text.len());
        self.base.request_redraw();
    }
    /// Returns current selection range.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }
    /// Sets selection range.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        if self.read_only {
            return;
        }
        let start = start.min(self.text.len());
        let end = end.min(self.text.len());
        if self.selection == Some((start, end)) {
            return;
        }
        self.selection = Some((start, end));
        self.selection_changed.emit(self.selection);
        self.base.request_redraw();
    }
    /// Clears selection.
    pub fn clear_selection(&mut self) {
        if self.selection.is_none() {
            return;
        }
        self.selection = None;
        self.selection_changed.emit(None);
    }
    /// Returns read-only state.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        if self.read_only == read_only {
            return;
        }
        self.read_only = read_only;
        self.read_only_changed.emit(read_only);
        self.base.request_redraw();
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.selection.map_or(0, |(start, _)| start)
    }

    /// Converts a byte offset into (line_index, col_index).
    /// Returns `None` when the offset is at end of text.
    fn byte_offset_to_line_col(&self, offset: usize) -> Option<(usize, usize)> {
        if self.text.is_empty() {
            return Some((0, 0));
        }
        let offset = offset.min(self.text.len());
        let mut line = 0usize;
        let mut col = 0usize;
        for (i, ch) in self.text.char_indices() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        Some((line, col))
    }

    /// Converts (line_index, col_index) back to a byte offset.
    fn line_col_to_byte_offset(&self, line: usize, col: usize) -> usize {
        let mut current_line = 0usize;
        let mut current_col = 0usize;
        for (i, ch) in self.text.char_indices() {
            if current_line == line && current_col == col {
                return i;
            }
            if ch == '\n' {
                current_line += 1;
                current_col = 0;
            } else {
                current_col += 1;
            }
        }
        // If we reached the end, return the text length if we're on the right line
        if current_line == line {
            self.text.len()
        } else {
            0
        }
    }
    /// Sets cursor position.
    pub fn set_cursor_position(&mut self, position: usize) {
        if self.read_only {
            return;
        }
        let position = position.min(self.text.len());
        self.selection = Some((position, position));
        self.cursor_position_changed.emit(position);
        self.base.request_redraw();
    }
}
impl Widget for RichEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `RichEdit`'s property contract.
///
/// `WidgetKind::RichEdit` is the kind the capability layer pairs with the
/// [`CodeEditor`](crate::widget::CodeEditor) control
/// (`capability::properties::code_editor_capability`), so the `RichEdit` arm of
/// the old centralised dispatch was answered by `CodeEditor`, not by this widget.
/// The trait impl therefore lives beside `CodeEditor` in its own file; this widget
/// declares that it has no properties of its own rather than claiming another
/// control's, which is the honest answer the property layer asks for.
impl WidgetProperties for RichEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        base_property_get(self, name)
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        base_property_set(self, name, value)
    }

    fn property_names(&self) -> &'static [&'static str] {
        crate::widget::capability::properties_trait::BASE_PROPERTY_NAMES
    }
}
impl Draw for RichEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw background
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        // Draw border
        context.draw_rect(
            rect,
            if self.read_only { Color::rgb(220, 220, 220) } else { Color::rgb(180, 180, 180) },
        );
        // Draw text content — all lines
        let font = crate::core::Font::default();
        let line_height = 16i32;
        let padding = 2i32;
        let mut line_y = rect.y + padding + line_height;
        let cursor_before = self.selection.map_or(0, |(start, _)| start);
        // Compute (line, col) for cursor
        let cursor_coord = self.byte_offset_to_line_col(cursor_before);
        for (line_idx, line) in self.text.lines().enumerate() {
            if line_y > rect.y + rect.height as i32 {
                break;
            }
            // Draw the line text
            context.draw_text(
                crate::core::Point::new(rect.x + padding, line_y),
                line,
                &font,
                Color::rgb(0, 0, 0),
                HorizontalAlignment::Left,
            );
            // Draw cursor on this line if not read-only
            if !self.read_only && Some(line_idx) == cursor_coord.map(|(l, _)| l) {
                if let Some((_, col)) = cursor_coord {
                    // Estimate cursor x position (rough char width)
                    let cursor_x = rect.x + padding + (col as i32) * 7;
                    context.draw_line(
                        crate::core::Point::new(cursor_x, line_y - line_height + 2),
                        crate::core::Point::new(cursor_x, line_y + 2),
                        Color::rgb(0, 0, 0),
                    );
                }
            }
            line_y += line_height;
        }
    }
}

impl crate::event::EventHandler for RichEdit {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let crate::event::Event::KeyPress { key: 90, modifiers: 2 } = event {
            let _ = self.undo();
            return;
        }
        if let crate::event::Event::KeyPress { key: 89, modifiers: 2 } = event {
            let _ = self.redo();
            return;
        }
        if self.read_only {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            crate::event::Event::KeyPress { key, modifiers } => {
                let cursor = self.selection.map_or(0, |(start, _)| start);
                match *key {
                    8 if cursor > 0 => {
                        // Backspace — delete char before cursor
                        let boundary = floor_char_boundary(&self.text, cursor - 1);
                        let mut next = self.text.clone();
                        next.drain(boundary..cursor);
                        let new_cursor = boundary;
                        self.set_text(next);
                        self.selection = Some((new_cursor, new_cursor));
                        self.cursor_position_changed.emit(new_cursor);
                    }
                    127 if cursor < self.text.len() => {
                        // Delete — delete char after cursor
                        let end = floor_char_boundary(&self.text, cursor + 1);
                        // Ensure we advance at least one char
                        let end = if end == cursor { cursor + 1 } else { end };
                        let mut next = self.text.clone();
                        next.drain(cursor..end.min(self.text.len()));
                        self.set_text(next);
                        self.selection = Some((cursor, cursor));
                        self.cursor_position_changed.emit(cursor);
                    }
                    13 => {
                        // Enter — insert newline at cursor
                        let mut next = self.text.clone();
                        next.insert(cursor, '\n');
                        self.set_text(next);
                        let new_cursor = cursor + 1;
                        self.selection = Some((new_cursor, new_cursor));
                        self.text_changed.emit(self.text.clone());
                        self.cursor_position_changed.emit(new_cursor);
                    }
                    37 if *modifiers == 0 && cursor > 0 => {
                        // Left arrow — move cursor left by one char
                        let boundary = floor_char_boundary(&self.text, cursor - 1);
                        self.selection = Some((boundary, boundary));
                        self.cursor_position_changed.emit(boundary);
                    }
                    39 if *modifiers == 0 && cursor < self.text.len() => {
                        // Right arrow — move cursor right by one char
                        let next = floor_char_boundary(&self.text, cursor + 1);
                        self.selection = Some((next, next));
                        self.cursor_position_changed.emit(next);
                    }
                    36 if *modifiers == 0 => {
                        // Home — move to beginning of current line
                        let coord = self.byte_offset_to_line_col(cursor);
                        if let Some((line, _)) = coord {
                            let new_cursor = self.line_col_to_byte_offset(line, 0);
                            self.selection = Some((new_cursor, new_cursor));
                            self.cursor_position_changed.emit(new_cursor);
                        }
                    }
                    35 if *modifiers == 0 => {
                        // End — move to end of current line
                        let coord = self.byte_offset_to_line_col(cursor);
                        if let Some((line, _)) = coord {
                            // Find the line end
                            let mut current_line = 0usize;
                            let mut line_end = self.text.len();
                            for (i, ch) in self.text.char_indices() {
                                if current_line == line && ch == '\n' {
                                    line_end = i;
                                    break;
                                }
                                if ch == '\n' {
                                    current_line += 1;
                                }
                            }
                            // If we didn't find newline on this line, it's the last line
                            if current_line == line && line_end == self.text.len() {
                                // line_end already = text.len()
                            }
                            self.selection = Some((line_end, line_end));
                            self.cursor_position_changed.emit(line_end);
                        }
                    }
                    38 if *modifiers == 0 => {
                        // Up arrow — move cursor up one line if possible
                        let coord = self.byte_offset_to_line_col(cursor);
                        if let Some((line, col)) = coord {
                            if line > 0 {
                                let new_cursor = self.line_col_to_byte_offset(line - 1, col);
                                self.selection = Some((new_cursor, new_cursor));
                                self.cursor_position_changed.emit(new_cursor);
                            }
                        }
                    }
                    40 if *modifiers == 0 => {
                        // Down arrow — move cursor down one line if possible
                        let coord = self.byte_offset_to_line_col(cursor);
                        if let Some((line, col)) = coord {
                            let new_cursor = self.line_col_to_byte_offset(line + 1, col);
                            if new_cursor != cursor {
                                self.selection = Some((new_cursor, new_cursor));
                                self.cursor_position_changed.emit(new_cursor);
                            }
                        }
                    }
                    _ if *key >= 32 && *key <= 126 => {
                        // Printable ASCII — insert at cursor position
                        let c = char::from_u32(*key).unwrap_or(' ');
                        let c = if *modifiers & 0x02 != 0 { c.to_ascii_uppercase() } else { c };
                        self.text.insert(cursor, c);
                        let new_cursor = cursor + c.len_utf8();
                        self.selection = Some((new_cursor, new_cursor));
                        self.text_changed.emit(self.text.clone());
                        self.cursor_position_changed.emit(new_cursor);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::event::EventHandler;

    #[test]
    fn richedit_creation_defaults() {
        let re = RichEdit::new(Rect::new(0, 0, 400, 300));
        assert!(re.text().is_empty());
        assert!(re.selection().is_none());
        assert!(!re.is_read_only());
        assert_eq!(re.cursor_position(), 0);
    }

    #[test]
    fn richedit_set_text() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_text("Hello RichEdit".to_string());
        assert_eq!(re.text(), "Hello RichEdit");
    }

    #[test]
    fn richedit_undo_redo_restores_text() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_text("one".to_string());
        re.set_text("two".to_string());
        assert!(re.can_undo());
        assert!(re.undo());
        assert_eq!(re.text(), "one");
        assert!(re.can_redo());
        assert!(re.redo());
        assert_eq!(re.text(), "two");
    }

    #[test]
    fn richedit_control_z_and_control_y_drive_history() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_text("before".to_string());
        re.set_text("after".to_string());
        re.handle_event(&crate::event::Event::key_press(90, 2));
        assert_eq!(re.text(), "before");
        re.handle_event(&crate::event::Event::key_press(89, 2));
        assert_eq!(re.text(), "after");
    }

    #[test]
    fn richedit_set_text_read_only() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_read_only(true);
        re.set_text("Should not change".to_string());
        assert!(re.text().is_empty());
    }

    #[test]
    fn richedit_read_only() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        assert!(!re.is_read_only());
        re.set_read_only(true);
        assert!(re.is_read_only());
        re.set_read_only(false);
        assert!(!re.is_read_only());
    }

    #[test]
    fn richedit_set_selection() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_text("Hello World".to_string());
        re.set_selection(0, 5);
        assert_eq!(re.selection(), Some((0, 5)));
    }

    #[test]
    fn richedit_clear_selection() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_text("Hello World".to_string());
        re.set_selection(0, 5);
        re.clear_selection();
        assert!(re.selection().is_none());
    }

    #[test]
    fn richedit_set_cursor_position() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_text("Hello".to_string());
        re.set_cursor_position(3);
        assert_eq!(re.cursor_position(), 3);
        re.set_cursor_position(100); // clamps
        assert_eq!(re.cursor_position(), 5);
    }

    #[test]
    fn richedit_geometry_delegation() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        re.set_geometry(Rect::new(10, 10, 500, 400));
        assert_eq!(re.geometry(), Rect::new(10, 10, 500, 400));
    }

    #[test]
    fn richedit_visibility() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        assert!(re.is_visible());
        re.hide();
        assert!(!re.is_visible());
        re.show();
        assert!(re.is_visible());
    }

    #[test]
    fn richedit_enabled() {
        let mut re = RichEdit::new(Rect::new(0, 0, 400, 300));
        assert!(re.is_enabled());
        re.set_enabled(false);
        assert!(!re.is_enabled());
        re.set_enabled(true);
        assert!(re.is_enabled());
    }

    #[test]
    fn richedit_id_kind() {
        let re_a = RichEdit::new(Rect::new(0, 0, 100, 100));
        let re_b = RichEdit::new(Rect::new(0, 0, 100, 100));
        assert_ne!(re_a.id(), re_b.id());
        assert_eq!(re_a.kind(), WidgetKind::RichEdit);
        assert_eq!(re_b.kind(), WidgetKind::RichEdit);
    }

    #[test]
    fn richedit_signal_accessors() {
        let re = RichEdit::new(Rect::new(0, 0, 100, 100));
        let _ = &re.text_changed;
        let _ = &re.selection_changed;
        let _ = &re.read_only_changed;
        let _ = &re.cursor_position_changed;
    }
}
