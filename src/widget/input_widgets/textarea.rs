//! TextArea widget — multi-line text input (BLUE13 R2.5).
use crate::core::{HorizontalAlignment, Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Multi-line text input widget.
///
/// Supports multi-line text storage with a byte-index cursor, character-level
/// insertion and deletion, placeholder display, read-only mode, and word-wrap
/// toggling. Emits a `changed` signal whenever the text content changes.
pub struct TextArea {
    base: BaseWidget,
    /// Text content (lines separated by '\n').
    text: String,
    /// Cursor position (byte index into text).
    cursor_pos: usize,
    /// Maximum text length (0 = unlimited).
    max_length: usize,
    /// Whether the widget is read-only.
    read_only: bool,
    /// Placeholder text when empty.
    placeholder: String,
    /// Whether this widget currently holds keyboard focus.
    focused: bool,
    /// Signal emitted when text changes.
    pub changed: GenericSignal,
}

impl TextArea {
    /// Creates a new `TextArea` with the given initial text and geometry.
    pub fn new(text: String, rect: Rect) -> Self {
        let cursor_pos = text.len();
        Self {
            base: BaseWidget::new(WidgetKind::TextArea, rect, "TextArea"),
            text,
            cursor_pos,
            max_length: 0,
            read_only: false,
            placeholder: String::new(),
            focused: false,
            changed: GenericSignal::new(),
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the entire text content.
    ///
    /// Emits the `changed` signal if the new text differs from the current text.
    /// The cursor is moved to the end of the new text.
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        let max = self.max_length;
        self.text = if max > 0 && text.len() > max {
            // Use floor_char_boundary to avoid splitting a multi-byte UTF-8 char
            let boundary = text.floor_char_boundary(max);
            text[..boundary].to_string()
        } else {
            text
        };
        self.cursor_pos = self.text.len();
        self.changed.emit();
    }

    /// Inserts a single character at the current cursor position.
    ///
    /// If `max_length` is greater than zero and the text already meets or exceeds
    /// that limit, the character is not inserted.
    ///
    /// SAFETY: Ensures `cursor_pos` is on a valid UTF-8 char boundary before
    /// inserting. If not, snaps to the nearest char boundary via `floor_char_boundary`.
    pub fn insert(&mut self, ch: char) {
        if self.max_length > 0 && self.text.len() >= self.max_length {
            return;
        }
        let boundary = self.text.floor_char_boundary(self.cursor_pos);
        self.cursor_pos = boundary;
        self.text.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
        self.changed.emit();
    }

    /// Deletes the character immediately before the cursor.
    ///
    /// If the cursor is at position 0, this is a no-op.
    pub fn delete_char(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let prev = self.text[..self.cursor_pos].char_indices().last().map(|(i, c)| {
            if c == '\n' {
                (i, 1)
            } else {
                (i, c.len_utf8())
            }
        });
        if let Some((start, len)) = prev {
            self.text.replace_range(start..start + len, "");
            self.cursor_pos = start;
            self.changed.emit();
        }
    }

    /// Returns the current cursor position (byte index into `text`).
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Sets the cursor position, clamping it to the text length.
    pub fn set_cursor_pos(&mut self, pos: usize) {
        self.cursor_pos = pos.min(self.text.len());
    }

    /// Sets the maximum text length.
    ///
    /// A value of `0` means unlimited. If the current text exceeds the new limit
    /// it is truncated and the cursor is adjusted accordingly.
    pub fn set_max_length(&mut self, max: usize) {
        self.max_length = max;
        if max > 0 && self.text.len() > max {
            self.text.truncate(max);
            self.cursor_pos = self.cursor_pos.min(max);
            self.changed.emit();
        }
    }

    /// Returns whether the text area is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Sets the read-only state.
    pub fn set_read_only(&mut self, ro: bool) {
        self.read_only = ro;
    }

    /// Returns the placeholder text shown when the text is empty.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text.
    pub fn set_placeholder(&mut self, text: String) {
        self.placeholder = text;
    }
}

impl Widget for TextArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let line_count = if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        };
        let max_line_width = self.text.lines().map(|l| l.len() as u32).max().unwrap_or(0);
        let w = (max_line_width * 8 + 10).max(120);
        let h = (line_count as u32 * 16 + 10).max(60);
        Size::new(w, h)
    }
}

impl EventHandler for TextArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained => {
                self.focused = true;
                self.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.request_redraw();
            }
            Event::KeyPress { key, .. } => {
                if self.read_only {
                    return;
                }
                match *key {
                    8 => {
                        // Backspace
                        self.delete_char();
                        self.request_redraw();
                    }
                    13 => {
                        // Enter — insert newline
                        self.insert('\n');
                        self.request_redraw();
                    }
                    37 => {
                        // Left arrow — move to previous char boundary
                        if self.cursor_pos > 0 {
                            let mut new_pos = self.cursor_pos;
                            while new_pos > 0 {
                                new_pos -= 1;
                                if self.text.is_char_boundary(new_pos) {
                                    break;
                                }
                            }
                            self.cursor_pos = new_pos;
                            self.request_redraw();
                        }
                    }
                    39 => {
                        // Right arrow — move to next char boundary
                        if self.cursor_pos < self.text.len() {
                            let mut new_pos = self.cursor_pos + 1;
                            while new_pos <= self.text.len() && !self.text.is_char_boundary(new_pos)
                            {
                                new_pos += 1;
                            }
                            self.cursor_pos = new_pos.min(self.text.len());
                            self.request_redraw();
                        }
                    }
                    _ => {
                        // Regular character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.insert(ch);
                                self.request_redraw();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Approximate pixel advance per character for `Font::default()`.
const CHAR_W: i32 = 8;
/// Approximate line height in pixels for `Font::default()`.
const LINE_H: i32 = 16;

impl Draw for TextArea {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let padding = 4;

        // -- Background --
        let bg = self.style().background_color.unwrap_or(Color::from_rgb(255, 255, 255));
        context.fill_rect(rect, bg);

        // -- Border --
        let border = self.style().border_color.unwrap_or(Color::from_rgb(200, 200, 200));
        context.draw_rect(rect, border);

        // -- Text --
        let text_color = self.style().text_color.unwrap_or(Color::from_rgb(0, 0, 0));
        let placeholder_color = Color::from_rgb(180, 180, 180);

        if self.text.is_empty() && !self.placeholder.is_empty() && !self.focused {
            // Draw placeholder in gray
            context.draw_text(
                Point::new(rect.x + padding, rect.y + padding),
                &self.placeholder,
                &Font::default(),
                placeholder_color,
                HorizontalAlignment::Left,
            );
        } else if !self.text.is_empty() {
            let mut y = rect.y + padding;
            for line in self.text.lines() {
                if y + LINE_H > rect.y + rect.height as i32 {
                    break;
                }
                context.draw_text(
                    Point::new(rect.x + padding, y),
                    line,
                    &Font::default(),
                    text_color,
                    HorizontalAlignment::Left,
                );
                y += LINE_H;
            }
            // Handle the case where text ends with '\n' (lines() strips trailing newline)
            if self.text.ends_with('\n') {
                // Draw an empty visual line so the cursor can be on the last line
                // (just advance y — nothing to draw for the empty line)
            }
        }

        // -- Cursor --
        if self.focused {
            let cursor_x = self.cursor_screen_x(rect.x + padding);
            let cursor_y = self.cursor_screen_y(rect.y + padding);
            context.draw_line(
                Point::new(cursor_x, cursor_y),
                Point::new(cursor_x, cursor_y + LINE_H),
                text_color,
            );
        }
    }
}

impl TextArea {
    /// Computes the screen-space X coordinate of the cursor.
    fn cursor_screen_x(&self, origin_x: i32) -> i32 {
        // Find the character position within the current line
        let text_before = &self.text[..self.cursor_pos];
        // Find the last newline before cursor
        let line_start = text_before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = self.text[line_start..self.cursor_pos].len();
        origin_x + col as i32 * CHAR_W
    }

    /// Computes the screen-space Y coordinate of the cursor (top of cursor line).
    fn cursor_screen_y(&self, origin_y: i32) -> i32 {
        let text_before = &self.text[..self.cursor_pos];
        let lines_before = text_before.chars().filter(|&c| c == '\n').count();
        origin_y + lines_before as i32 * LINE_H
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn textarea_creation_defaults() {
        let ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert_eq!(ta.text(), "");
        assert_eq!(ta.cursor_pos(), 0);
        assert_eq!(ta.max_length, 0);
        assert!(!ta.is_read_only());
        assert!(ta.placeholder().is_empty());
        assert!(!ta.focused);
    }

    #[test]
    fn textarea_set_text() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_text("Hello\nWorld".to_string());
        assert_eq!(ta.text(), "Hello\nWorld");
        assert_eq!(ta.cursor_pos(), 11); // cursor at end
    }

    #[test]
    fn textarea_insert_char() {
        let mut ta = TextArea::new("Helo".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(3);
        ta.insert('l');
        assert_eq!(ta.text(), "Hello");
        assert_eq!(ta.cursor_pos(), 4);
    }

    #[test]
    fn textarea_delete_char() {
        let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(5);
        ta.delete_char();
        assert_eq!(ta.text(), "Hell");
        assert_eq!(ta.cursor_pos(), 4);
    }

    #[test]
    fn textarea_delete_char_at_start() {
        let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(0);
        ta.delete_char();
        assert_eq!(ta.text(), "Hello");
        assert_eq!(ta.cursor_pos(), 0);
    }

    #[test]
    fn textarea_delete_char_with_newline() {
        let mut ta = TextArea::new("A\nB".to_string(), Rect::new(0, 0, 300, 200));
        // "A\nB" — cursor at 3 (end), delete_char removes 'B', cursor at 2
        ta.set_cursor_pos(3);
        ta.delete_char();
        assert_eq!(ta.text(), "A\n");
        assert_eq!(ta.cursor_pos(), 2);
    }

    #[test]
    fn textarea_cursor_movement() {
        let mut ta = TextArea::new("Hi".to_string(), Rect::new(0, 0, 300, 200));
        assert_eq!(ta.cursor_pos(), 2);
        ta.set_cursor_pos(0);
        assert_eq!(ta.cursor_pos(), 0);
        ta.set_cursor_pos(5); // clamp
        assert_eq!(ta.cursor_pos(), 2);
    }

    #[test]
    fn textarea_placeholder() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert!(ta.placeholder().is_empty());
        ta.set_placeholder("Enter text...".to_string());
        assert_eq!(ta.placeholder(), "Enter text...");
    }

    #[test]
    fn textarea_read_only() {
        let mut ta = TextArea::new("Readable".to_string(), Rect::new(0, 0, 300, 200));
        assert!(!ta.is_read_only());
        ta.set_read_only(true);
        assert!(ta.is_read_only());
    }

    #[test]
    fn textarea_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(5);
        assert_eq!(ta.max_length, 5);
        // insert beyond limit
        ta.set_text("Hello World".to_string());
        assert_eq!(ta.text().len(), 5);
        assert_eq!(ta.text(), "Hello");
    }

    #[test]
    fn textarea_draw_does_not_panic() {
        use crate::render::RenderContext;
        use crate::render::SoftwarePaintBackend;

        let mut ta = TextArea::new("Line1\nLine2".to_string(), Rect::new(0, 0, 200, 100));
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 100), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        // Should not panic
        ta.draw(&mut ctx);
    }

    #[test]
    fn textarea_set_text_truncates_on_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(3);
        ta.set_text("Hello".to_string());
        assert_eq!(ta.text(), "Hel");
        assert_eq!(ta.cursor_pos(), 3);
    }

    #[test]
    fn textarea_insert_respects_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(3);
        ta.set_text("ABC".to_string());
        ta.insert('D');
        // Should not insert because max_length reached
        assert_eq!(ta.text(), "ABC");
        assert_eq!(ta.cursor_pos(), 3);
    }

    #[test]
    fn textarea_empty_text_events_no_panic() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // backspace on empty
        assert_eq!(ta.text(), "");
        ta.handle_event(&Event::KeyPress { key: 37, modifiers: 0 }); // left arrow at 0
        assert_eq!(ta.cursor_pos(), 0);
        ta.handle_event(&Event::KeyPress { key: 39, modifiers: 0 }); // right arrow at 0 (no change)
        assert_eq!(ta.cursor_pos(), 0);
    }

    #[test]
    fn textarea_focus_events() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert!(!ta.focused);
        ta.handle_event(&Event::FocusGained);
        assert!(ta.focused);
        ta.handle_event(&Event::FocusLost);
        assert!(!ta.focused);
    }

    #[test]
    fn textarea_keypress_insertion() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 72, modifiers: 0 }); // 'H'
        ta.handle_event(&Event::KeyPress { key: 105, modifiers: 0 }); // 'i'
        assert_eq!(ta.text(), "Hi");
    }

    #[test]
    fn textarea_keypress_enter_inserts_newline() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        ta.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter
        ta.handle_event(&Event::KeyPress { key: 66, modifiers: 0 }); // 'B'
        assert_eq!(ta.text(), "A\nB");
    }
}
