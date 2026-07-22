//! InplaceEditor widget — an in-place text editing control for table/cell editing.
//!
//! Displays text normally, and when activated (double-click), switches to an
//! edit mode with a blinking cursor. Enter/Tab accepts changes, Escape cancels.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A text editor that switches between display mode and edit mode in-place.
///
/// In display mode, the text is drawn as plain text with a background.
/// In edit mode, it draws a text input area with a blinking cursor.
pub struct InplaceEditor {
    base: BaseWidget,
    text: String,
    is_editing: bool,
    original_text: String,
    font_size: f32,
    padding: i32,
    cursor_position: usize,
    /// Emitted when the edit is accepted (Enter/Tab). Carries the final text.
    pub edit_accepted: Signal1<String>,
    /// Emitted when the edit is cancelled (Escape).
    pub edit_cancelled: GenericSignal,
}

impl InplaceEditor {
    /// Creates a new InplaceEditor widget with the given text and geometry.
    pub fn new(text: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::InplaceEditor, geometry, "InplaceEditor"),
            text: text.to_string(),
            is_editing: false,
            original_text: text.to_string(),
            font_size: 14.0,
            padding: 4,
            cursor_position: text.len(),
            edit_accepted: Signal1::new(),
            edit_cancelled: GenericSignal::new(),
        }
    }

    /// Starts editing mode.
    pub fn start_edit(&mut self) {
        if !self.is_editing {
            self.is_editing = true;
            self.original_text = self.text.clone();
            self.cursor_position = self.text.len();
            self.base.request_redraw();
        }
    }

    /// Finishes editing mode. If `accept` is true, the current text is kept;
    /// otherwise it reverts to the original text.
    pub fn finish_edit(&mut self, accept: bool) {
        if !self.is_editing {
            return;
        }
        self.is_editing = false;
        if accept {
            self.edit_accepted.emit(self.text.clone());
        } else {
            self.text = self.original_text.clone();
            self.edit_cancelled.emit();
        }
        self.base.request_redraw();
    }

    /// Returns whether the editor is in edit mode.
    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor_position = self.text.len();
        self.base.request_redraw();
    }

    /// Sets the font size.
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.max(4.0);
        self.base.request_redraw();
    }

    /// Returns the font size.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Sets the padding around the text.
    pub fn set_padding(&mut self, padding: i32) {
        self.padding = padding.max(0);
        self.base.request_redraw();
    }

    /// Returns the padding.
    pub fn padding(&self) -> i32 {
        self.padding
    }

    /// Inserts a character at the cursor position.
    fn insert_char(&mut self, c: char) {
        if c == '\u{7f}' {
            // Delete (backward)
            if self.cursor_position > 0 {
                let mut chars: Vec<char> = self.text.chars().collect();
                chars.remove(self.cursor_position - 1);
                self.text = chars.into_iter().collect();
                self.cursor_position = self.cursor_position.saturating_sub(1);
                self.base.request_redraw();
            }
        } else if c == '\u{ffff}' {
            // Forward delete
            if self.cursor_position < self.text.chars().count() {
                let mut chars: Vec<char> = self.text.chars().collect();
                chars.remove(self.cursor_position);
                self.text = chars.into_iter().collect();
                self.base.request_redraw();
            }
        } else {
            let mut chars: Vec<char> = self.text.chars().collect();
            chars.insert(self.cursor_position, c);
            self.text = chars.into_iter().collect();
            self.cursor_position += 1;
            self.base.request_redraw();
        }
    }

    /// Moves the cursor left by one character.
    fn cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.base.request_redraw();
        }
    }

    /// Moves the cursor right by one character.
    fn cursor_right(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor_position < char_count {
            self.cursor_position += 1;
            self.base.request_redraw();
        }
    }
}

impl Widget for InplaceEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
}

impl Draw for InplaceEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let font = Font::new("sans-serif", self.font_size, false, false);

        if self.is_editing {
            // Draw editing mode
            context.fill_rect(rect, Color::rgba(255, 255, 255, 255));
            context.draw_rect_stroke(rect, Color::rgba(0, 120, 255, 255), 2);

            // Draw text
            let text_x = rect.x + self.padding;
            let text_y = rect.y + self.padding + self.font_size as i32;
            context.draw_text(Point::new(text_x, text_y), &self.text, &font, Color::BLACK, HorizontalAlignment::Left);

            // Draw cursor (blinking vertical line)
            let cursor_x = text_x + self.cursor_position as i32 * 8;
            context.draw_line(
                Point::new(cursor_x, rect.y + self.padding),
                Point::new(cursor_x, rect.y + rect.height as i32 - self.padding),
                Color::rgba(0, 0, 0, 200),
            );
        } else {
            // Draw display mode
            context.fill_rect(rect, Color::rgba(245, 245, 245, 255));
            context.draw_rect_stroke(rect, Color::rgba(200, 200, 200, 255), 1);

            let text_x = rect.x + self.padding;
            let text_y = rect.y + self.padding + self.font_size as i32;
            context.draw_text(
                Point::new(text_x, text_y),
                &self.text,
                &font,
                Color::rgba(50, 50, 50, 255),
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for InplaceEditor {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseDoubleClick { pos: _, button } if *button == 1 => {
                self.start_edit();
            }
            Event::KeyPress { key, modifiers: _ } => {
                if !self.is_editing {
                    self.base.handle_event(event);
                    return;
                }
                match *key {
                    0x1B => {
                        // Escape - cancel
                        self.finish_edit(false);
                    }
                    0x0D | 0x09 => {
                        // Enter or Tab - accept
                        self.finish_edit(true);
                    }
                    0x08 => {
                        // Backspace
                        self.insert_char('\u{7f}');
                    }
                    0x2E => {
                        // Delete (forward)
                        self.insert_char('\u{ffff}');
                    }
                    0x25 => {
                        // Left arrow
                        self.cursor_left();
                    }
                    0x27 => {
                        // Right arrow
                        self.cursor_right();
                    }
                    _ => {
                        // Printable characters
                        if let Some(c) = char::from_u32(*key) {
                            if c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation()
                            {
                                self.insert_char(c);
                            }
                        }
                    }
                }
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
    use crate::core::Point;

    #[test]
    fn inplace_editor_initial_state() {
        let ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        assert_eq!(ie.text(), "Hello");
        assert!(!ie.is_editing());
        assert!((ie.font_size() - 14.0).abs() < 0.01);
        assert_eq!(ie.padding(), 4);
        assert_eq!(ie.kind(), WidgetKind::InplaceEditor);
    }

    #[test]
    fn inplace_editor_set_text() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.set_text("World");
        assert_eq!(ie.text(), "World");
    }

    #[test]
    fn inplace_editor_start_and_finish_edit_accept() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));

        let accepted = std::sync::Arc::new(std::sync::Mutex::new(None));
        let accepted_clone = accepted.clone();
        ie.edit_accepted.connect(move |text| {
            *accepted_clone.lock().unwrap() = Some((*text).clone());
        });

        ie.start_edit();
        assert!(ie.is_editing());

        ie.set_text("Hello World");
        ie.finish_edit(true);

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Hello World");
        assert_eq!(*accepted.lock().unwrap(), Some("Hello World".to_string()));
    }

    #[test]
    fn inplace_editor_start_and_finish_edit_cancel() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));

        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancelled_clone = cancelled.clone();
        ie.edit_cancelled.connect(move || {
            cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        ie.start_edit();
        assert!(ie.is_editing());

        ie.set_text("Modified Text");
        ie.finish_edit(false);

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Hello"); // Should revert to original
        assert!(cancelled.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn inplace_editor_double_click_starts_edit() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.handle_event(&Event::MouseDoubleClick { pos: Point::new(50, 15), button: 1 });
        assert!(ie.is_editing());
    }

    #[test]
    fn inplace_editor_escape_cancels_edit() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.set_text("Changed");

        ie.handle_event(&Event::KeyPress {
            key: 0x1B, // Escape
            modifiers: 0,
        });

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Hello");
    }

    #[test]
    fn inplace_editor_enter_accepts_edit() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.set_text("Accepted");

        ie.handle_event(&Event::KeyPress {
            key: 0x0D, // Enter
            modifiers: 0,
        });

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Accepted");
    }

    #[test]
    fn inplace_editor_set_font_size_and_padding() {
        let mut ie = InplaceEditor::new("Test", Rect::new(0, 0, 200, 30));

        ie.set_font_size(18.0);
        assert!((ie.font_size() - 18.0).abs() < 0.01);

        ie.set_font_size(0.0); // Should clamp
        assert!((ie.font_size() - 4.0).abs() < 0.01);

        ie.set_padding(8);
        assert_eq!(ie.padding(), 8);

        ie.set_padding(-5); // Should clamp to 0
        assert_eq!(ie.padding(), 0);
    }

    #[test]
    fn inplace_editor_insert_characters() {
        let mut ie = InplaceEditor::new("", Rect::new(0, 0, 200, 30));
        ie.start_edit();

        // Simulate typing
        ie.insert_char('A');
        ie.insert_char('B');
        ie.insert_char('C');
        assert_eq!(ie.text(), "ABC");
        assert_eq!(ie.cursor_position, 3);

        // Backspace
        ie.insert_char('\u{7f}');
        assert_eq!(ie.text(), "AB");
        assert_eq!(ie.cursor_position, 2);
    }
}
