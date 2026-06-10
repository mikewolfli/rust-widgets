//! Single-line text edit widget.
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
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
    pub text_changed: Signal1<String>,
    pub editing_finished: GenericSignal,
    pub return_pressed: GenericSignal,
}
/// Text echo mode for password fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EchoMode {
    /// Display characters as entered (default)
    #[default]
    Normal,
    /// Display asterisks for password fields
    Password,
    /// Display nothing (for sensitive data)
    NoEcho,
    /// Display asterisks only when editing
    PasswordEchoOnEdit,
}
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
            text_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
            return_pressed: GenericSignal::new(),
        }
    }
    /// Returns current text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets text and emits text_changed signal if different.
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text;
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.text_changed.emit(self.text.clone());
    }
    /// Returns placeholder text.
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }
    /// Sets placeholder text.
    pub fn set_placeholder_text(&mut self, text: String) {
        self.placeholder_text = text;
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
    }
    /// Returns echo mode.
    pub fn echo_mode(&self) -> EchoMode {
        self.echo_mode
    }
    /// Sets echo mode.
    pub fn set_echo_mode(&mut self, mode: EchoMode) {
        self.echo_mode = mode;
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }
    /// Sets cursor position.
    pub fn set_cursor_position(&mut self, position: usize) {
        self.cursor_position = position.min(self.text.len());
        self.selection_start = None;
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
        // Check max length
        if let Some(max) = self.max_length {
            let available = max.saturating_sub(self.text.len());
            if available == 0 {
                return;
            }
            let _text = if text.len() > available { &text[..available] } else { text };
        }
        // Handle selection
        let mut new_text = self.text.clone();
        if let Some(start) = self.selection_start {
            let start = start.min(new_text.len());
            let end = self.cursor_position.min(new_text.len());
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            new_text.replace_range(start..end, text);
            self.cursor_position = start + text.len();
        } else {
            new_text.insert_str(self.cursor_position, text);
            self.cursor_position += text.len();
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
    /// Returns display text based on echo mode.
    fn display_text(&self) -> String {
        match self.echo_mode {
            EchoMode::Normal => self.text.clone(),
            EchoMode::Password => "*".repeat(self.text.len()),
            EchoMode::NoEcho => String::new(),
            EchoMode::PasswordEchoOnEdit => {
                // In real implementation, would track edit state
                "*".repeat(self.text.len())
            }
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
}
impl EventHandler for LineEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.read_only {
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
                        // Ctrl+V: Paste (would need clipboard integration)
                        // For now, just emit signal
                        self.base.redraw_requested.emit();
                    }
                    67 if modifiers & 2 != 0 => {
                        // Ctrl+C: Copy (would need clipboard integration)
                        // For now, just emit signal
                    }
                    88 if modifiers & 2 != 0 => {
                        // Ctrl+X: Cut (would need clipboard integration)
                        // For now, just emit signal
                        self.base.redraw_requested.emit();
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
                self.editing_finished.emit();
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
        let bg = style.background_color.unwrap_or(Color::from_rgb(255, 255, 255));
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            if style.border_width > 0 {
                context.draw_rect_stroke(
                    Rect::new(rect.x, rect.y, rect.width, rect.height),
                    border_color,
                    style.border_width,
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
            let text_color = style.text_color.unwrap_or(Color::from_rgb(0, 0, 0));
            let font = style.font.clone().unwrap_or_default();
            context.draw_text(Point::new(text_x, text_y as i32), display_text, &font, text_color);
        }
        // Draw cursor if focused
        // Note: Would need focus state tracking
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        le.set_echo_mode(EchoMode::PasswordEchoOnEdit);
        assert_eq!(le.echo_mode(), EchoMode::PasswordEchoOnEdit);
        le.set_echo_mode(EchoMode::Normal);
        assert_eq!(le.echo_mode(), EchoMode::Normal);
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
}
