//! Multi-line text edit widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Multi-line text edit widget.
pub struct TextEdit {
    base: BaseWidget,
    text: String,
    placeholder_text: String,
    max_length: Option<usize>,
    read_only: bool,
    line_wrap: bool,
    pub text_changed: Signal1<String>,
    pub cursor_position_changed: Signal1<usize>,
}
impl TextEdit {
    /// Creates an empty text edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TextEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            read_only: false,
            line_wrap: true,
            text_changed: Signal1::new(),
            cursor_position_changed: Signal1::new(),
        }
    }
    /// Returns current text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets text and emits text_changed signal if different.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        self.text = text;
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
        // Truncate if needed (using floor_char_boundary to avoid mid-char panic)
        if let Some(max) = max_length {
            if self.text.len() > max {
                let boundary = self.text.floor_char_boundary(max);
                self.text.truncate(boundary);
                self.text_changed.emit(self.text.clone());
            }
        }
    }
    /// Returns whether the widget is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }
    /// Returns whether line wrap is enabled.
    pub fn line_wrap(&self) -> bool {
        self.line_wrap
    }
    /// Sets line wrap state.
    pub fn set_line_wrap(&mut self, line_wrap: bool) {
        self.line_wrap = line_wrap;
    }
    /// Returns number of lines in the text.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        }
    }
    /// Returns text at specified line (0-indexed).
    pub fn line_text(&self, line: usize) -> Option<&str> {
        let mut start = 0;
        let mut current_line = 0;
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                if current_line == line {
                    return Some(&self.text[start..i]);
                }
                start = i + 1;
                current_line += 1;
            }
        }
        if current_line == line {
            Some(&self.text[start..])
        } else {
            None
        }
    }
    /// Appends text to the end.
    pub fn append(&mut self, text: &str) {
        self.text.push_str(text);
        self.text_changed.emit(self.text.clone());
    }
    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }
    /// Returns whether the text edit is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}
// Implement Widget trait
impl Widget for TextEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        Size::new(200, 24)
    }
}
impl EventHandler for TextEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || self.read_only {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
            match *key {
                8 => {
                    // Backspace
                    if !self.text.is_empty() {
                        self.text.pop();
                        self.text_changed.emit(self.text.clone());
                    }
                }
                13 => {
                    // Enter
                    self.text.push('\n');
                    self.text_changed.emit(self.text.clone());
                }
                _ => {
                    // Character input
                    if let Some(ch) = char::from_u32(*key) {
                        if ch.is_ascii_graphic() || ch == ' ' || ch == '\t' {
                            self.text.push(ch);
                            self.text_changed.emit(self.text.clone());
                        }
                    }
                }
            }
        }
    }
}

impl Draw for TextEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding;
        // Draw background
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::rgb(200, 200, 200));
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.text
        };
        if !display_text.is_empty() {
            // Simple text drawing - in real implementation would handle line wrapping
            context.draw_text(
                Point::new(text_x, text_y),
                display_text,
                &Font::default(),
                Color::rgb(0, 0, 0),
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn textedit_creation_defaults() {
        let te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.text().is_empty());
        assert!(te.placeholder_text().is_empty());
        assert_eq!(te.max_length(), None);
        assert!(!te.is_read_only());
        assert!(te.line_wrap());
        assert!(te.is_empty());
        assert_eq!(te.line_count(), 1);
    }

    #[test]
    fn textedit_set_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Hello World".to_string());
        assert_eq!(te.text(), "Hello World");
        assert!(!te.is_empty());
    }

    #[test]
    fn textedit_set_text_empty() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Some text".to_string());
        te.set_text(String::new());
        assert!(te.text().is_empty());
    }

    #[test]
    fn textedit_placeholder() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.placeholder_text().is_empty());
        te.set_placeholder_text("Enter text here".to_string());
        assert_eq!(te.placeholder_text(), "Enter text here");
        te.set_placeholder_text(String::new());
        assert!(te.placeholder_text().is_empty());
    }

    #[test]
    fn textedit_max_length() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert_eq!(te.max_length(), None);
        te.set_max_length(Some(10));
        assert_eq!(te.max_length(), Some(10));
        te.set_max_length(None);
        assert_eq!(te.max_length(), None);
    }

    #[test]
    fn textedit_max_length_truncates() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Hello World Too Long".to_string());
        te.set_max_length(Some(10));
        assert_eq!(te.text().len(), 10);
    }

    #[test]
    fn textedit_read_only() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(!te.is_read_only());
        te.set_read_only(true);
        assert!(te.is_read_only());
        te.set_read_only(false);
        assert!(!te.is_read_only());
    }

    #[test]
    fn textedit_line_wrap() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.line_wrap());
        te.set_line_wrap(false);
        assert!(!te.line_wrap());
        te.set_line_wrap(true);
        assert!(te.line_wrap());
    }

    #[test]
    fn textedit_line_count() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert_eq!(te.line_count(), 1);
        te.set_text("Line 1\nLine 2\nLine 3".to_string());
        assert_eq!(te.line_count(), 3);
    }

    #[test]
    fn textedit_line_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("First\nSecond\nThird".to_string());
        assert_eq!(te.line_text(0), Some("First"));
        assert_eq!(te.line_text(1), Some("Second"));
        assert_eq!(te.line_text(2), Some("Third"));
        assert_eq!(te.line_text(5), None);
    }

    #[test]
    fn textedit_append() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.append("Hello");
        assert_eq!(te.text(), "Hello");
        te.append(" World");
        assert_eq!(te.text(), "Hello World");
    }

    #[test]
    fn textedit_clear() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Some text".to_string());
        te.clear();
        assert!(te.is_empty());
    }

    #[test]
    fn textedit_geometry_delegation() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_geometry(Rect::new(10, 10, 400, 300));
        assert_eq!(te.geometry(), Rect::new(10, 10, 400, 300));
    }

    #[test]
    fn textedit_visibility() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.is_visible());
        te.hide();
        assert!(!te.is_visible());
        te.show();
        assert!(te.is_visible());
    }

    #[test]
    fn textedit_enabled() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.is_enabled());
        te.set_enabled(false);
        assert!(!te.is_enabled());
        te.set_enabled(true);
        assert!(te.is_enabled());
    }

    #[test]
    fn textedit_id_kind() {
        let te_a = TextEdit::new(Rect::new(0, 0, 100, 100));
        let te_b = TextEdit::new(Rect::new(0, 0, 100, 100));
        assert_ne!(te_a.id(), te_b.id());
        assert_eq!(te_a.kind(), WidgetKind::TextEdit);
        assert_eq!(te_b.kind(), WidgetKind::TextEdit);
    }

    #[test]
    fn textedit_signal_accessors() {
        let te = TextEdit::new(Rect::new(0, 0, 100, 100));
        let _ = &te.text_changed;
        let _ = &te.cursor_position_changed;
    }
}
