//! TagInput widget — a text input that creates tags/chips on Enter or comma, with removable tags.
//!
//! The TagInput presents a text field where the user types tag text, pressing Enter or comma
//! to create a tag chip. Each tag is displayed as a rounded chip with an X button for removal.
//! It emits `tags_changed` with the current list of tags on every change.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Horizontal padding between tag chip and container edge, or between chips.
const TAG_PADDING: i32 = 6;
/// Vertical padding inside the tag area.
const TAG_VERTICAL_PADDING: i32 = 4;
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
/// Cursor blink interval in milliseconds.
/// TagInput widget — a text input that creates tags/chips on Enter or comma.
pub struct TagInput {
    base: BaseWidget,
    tags: Vec<String>,
    input_buffer: String,
    focused: bool,
    /// Emitted when the tags list changes, providing the full list of tags.
    pub tags_changed: Signal1<Vec<String>>,
}

impl TagInput {
    /// Creates a new TagInput widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TagInput, geometry, "TagInput"),
            tags: Vec::new(),
            input_buffer: String::new(),
            focused: false,
            tags_changed: Signal1::new(),
        }
    }

    /// Adds a tag to the list. Trims the input and ignores empty tags.
    /// Emits `tags_changed` if a tag was actually added.
    pub fn add_tag(&mut self, tag: &str) {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return;
        }
        // Avoid duplicate tags
        if self.tags.iter().any(|t| t == trimmed) {
            return;
        }
        self.tags.push(trimmed.to_string());
        self.tags_changed.emit(self.tags.clone());
        self.base.request_redraw();
    }

    /// Removes a tag at the given index.
    /// Emits `tags_changed` if a tag was actually removed.
    pub fn remove_tag(&mut self, index: usize) {
        if index < self.tags.len() {
            self.tags.remove(index);
            self.tags_changed.emit(self.tags.clone());
            self.base.request_redraw();
        }
    }

    /// Returns a slice of all current tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Clears all tags. Emits `tags_changed` if the list was non-empty.
    pub fn clear_tags(&mut self) {
        if !self.tags.is_empty() {
            self.tags.clear();
            self.input_buffer.clear();
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
            self.base.request_redraw();
            if focused {
                self.base.focus_gained.emit();
            } else {
                self.base.focus_lost.emit();
            }
        }
    }

    /// Commits the current input buffer as a tag and clears the buffer.
    fn commit_input(&mut self) {
        let text = self.input_buffer.trim().to_string();
        if !text.is_empty() {
            self.add_tag(&text);
        }
        self.input_buffer.clear();
        self.base.request_redraw();
    }

    // ── Private helpers ──

    /// Returns the close button center for a tag chip at the given pixel position.
    fn tag_close_center(&self, chip_x: i32, chip_width: i32, chip_y: i32) -> Point {
        Point::new(chip_x + chip_width - TAG_PADDING - TAG_CLOSE_RADIUS, chip_y + TAG_HEIGHT / 2)
    }

    /// Hit-tests whether a point is within the close button of any tag chip.
    /// Returns the tag index if a close button was hit, or `None`.
    fn hit_tag_close(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        let mut current_x = rect.x + TAG_PADDING;
        let chip_y = rect.y + TAG_VERTICAL_PADDING;
        let max_width = rect.width as i32 - TAG_PADDING * 2;

        for (index, tag) in self.tags.iter().enumerate() {
            let chip_width = self.compute_chip_width(tag).min(max_width);

            // Check if the close button within this chip was hit
            let close_center = self.tag_close_center(current_x, chip_width, chip_y);
            let dx = (pos.x - close_center.x) as i64;
            let dy = (pos.y - close_center.y) as i64;
            if dx * dx + dy * dy <= (TAG_CLOSE_RADIUS as i64 + 2) * (TAG_CLOSE_RADIUS as i64 + 2) {
                return Some(index);
            }

            current_x += chip_width + TAG_GAP;
        }
        None
    }

    /// Computes the width of a tag chip based on its text content.
    fn compute_chip_width(&self, tag: &str) -> i32 {
        let text_width = tag.len() as i32 * 7 + 12; // approximate pixel width
                                                    // Add space for close button + padding
        text_width + TAG_PADDING * 2 + TAG_CLOSE_RADIUS * 2 + 4
    }
}

impl Widget for TagInput {
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

impl Draw for TagInput {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // ── Background ──
        let bg_color = if !is_enabled {
            Color::rgba(240, 240, 240, 160)
        } else if self.focused {
            Color::rgba(245, 245, 255, 220)
        } else {
            Color::rgba(235, 235, 235, 200)
        };
        context.fill_rounded_rect(rect, 6, bg_color);

        // ── Focus border ──
        if self.focused && is_enabled {
            context.draw_rounded_rect_stroke(rect, 6, Color::rgba(60, 140, 255, 200), 2);
        } else {
            context.draw_rounded_rect_stroke(rect, 6, Color::rgba(200, 200, 200, 160), 1);
        }

        let mut current_x = rect.x + TAG_PADDING;
        let chip_y = rect.y + TAG_VERTICAL_PADDING;
        let max_width = rect.width as i32 - TAG_PADDING * 2;
        let default_font = crate::core::Font::default();

        // ── Draw tag chips ──
        for tag in &self.tags {
            let chip_width = self.compute_chip_width(tag).min(max_width);
            let chip_rect = Rect::new(current_x, chip_y, chip_width as u32, TAG_HEIGHT as u32);

            // Chip background
            let chip_bg = if is_enabled {
                Color::rgb(25, 118, 210) // Material blue
            } else {
                Color::rgba(180, 180, 180, 160)
            };
            context.fill_rounded_rect(chip_rect, TAG_CHIP_RADIUS, chip_bg);

            // Chip text
            let text_color = Color::WHITE;
            let text_x = current_x + TAG_PADDING;
            let text_origin = Point::new(text_x, chip_y + TAG_HEIGHT / 2);
            context.draw_text(
                text_origin,
                tag,
                &default_font,
                text_color,
                HorizontalAlignment::Left,
            );

            // Close button circle
            let close_center = self.tag_close_center(current_x, chip_width, chip_y);
            let close_bg = Color::rgba(255, 255, 255, 200);
            context.fill_circle(close_center, TAG_CLOSE_RADIUS as u32, close_bg);

            // X mark (two diagonal lines)
            let x_offset = (TAG_CLOSE_RADIUS as f32 * 0.45) as i32;
            let close_fg = Color::rgb(25, 118, 210);
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

            current_x += chip_width + TAG_GAP;
        }

        // ── Input area ──
        let input_x = current_x;
        let input_width = (rect.width as i32 - input_x - TAG_PADDING).max(MIN_INPUT_WIDTH);
        let input_rect = Rect::new(input_x, chip_y, input_width as u32, TAG_HEIGHT as u32);

        // Input background (slightly inset)
        let input_bg = if !is_enabled {
            Color::rgba(240, 240, 240, 100)
        } else {
            Color::rgba(255, 255, 255, 180)
        };
        context.fill_rounded_rect(input_rect, 4, input_bg);

        // Input text
        let input_text_color = if !is_enabled {
            Color::rgba(160, 160, 160, 180)
        } else if !self.input_buffer.is_empty() {
            Color::rgba(30, 30, 30, 230)
        } else {
            Color::rgba(160, 160, 160, 200)
        };
        let display_text = if self.input_buffer.is_empty() && self.tags.is_empty() {
            "Type and press Enter..."
        } else if self.input_buffer.is_empty() {
            ""
        } else {
            &self.input_buffer
        };
        let text_origin = Point::new(input_x + 4, chip_y + TAG_HEIGHT / 2);
        context.draw_text(
            text_origin,
            display_text,
            &default_font,
            input_text_color,
            HorizontalAlignment::Left,
        );

        // ── Cursor (when focused and input is active) ──
        if self.focused && is_enabled {
            let cursor_x = input_x + 4 + (self.input_buffer.len() as i32 * 7);
            let cursor_y1 = chip_y + 3;
            let cursor_y2 = chip_y + TAG_HEIGHT - 3;
            context.draw_line(
                Point::new(cursor_x, cursor_y1),
                Point::new(cursor_x, cursor_y2),
                Color::rgba(60, 140, 255, 200),
            );
        }
    }
}

impl EventHandler for TagInput {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                // Check if the click is on a tag's close button
                if let Some(tag_index) = self.hit_tag_close(*pos) {
                    self.remove_tag(tag_index);
                    return;
                }
                // Click anywhere else in the widget → gain focus
                self.set_focused(true);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                // No special release handling needed
            }
            Event::FocusGained => {
                self.set_focused(true);
            }
            Event::FocusLost => {
                // Commit any pending input on focus loss
                if !self.input_buffer.is_empty() {
                    self.commit_input();
                }
                self.set_focused(false);
            }
            Event::KeyPress { key, modifiers: _ } => {
                if !self.focused {
                    return;
                }
                match *key {
                    8 => {
                        // Backspace — remove last character from buffer
                        // If buffer is empty, remove the last tag
                        if !self.input_buffer.is_empty() {
                            self.input_buffer.pop();
                            self.base.request_redraw();
                        } else if !self.tags.is_empty() {
                            self.tags.pop();
                            self.tags_changed.emit(self.tags.clone());
                            self.base.request_redraw();
                        }
                    }
                    13 => {
                        // Enter — commit the current input as a tag
                        self.commit_input();
                    }
                    44 => {
                        // Comma — commit the current input as a tag
                        self.commit_input();
                    }
                    27 => {
                        // Escape — lose focus
                        self.set_focused(false);
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.input_buffer.push(ch);
                                self.base.request_redraw();
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn tag_input_default_creation() {
        let ti = TagInput::new(Rect::new(0, 0, 300, 36));
        assert!(ti.tags().is_empty());
        assert!(!ti.is_focused());
        assert_eq!(ti.kind(), WidgetKind::TagInput);
        assert!(ti.is_enabled());
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
    fn tag_input_focus_on_click() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        assert!(!ti.is_focused());

        ti.handle_event(&Event::MousePress { pos: Point::new(50, 18), button: 1 });
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

        ti.handle_event(&Event::MousePress { pos: Point::new(50, 18), button: 1 });
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

        // Compute expected close button position for the first (and only) tag chip
        let rect = ti.geometry();
        let chip_x = rect.x + TAG_PADDING;
        let chip_y = rect.y + TAG_VERTICAL_PADDING;
        let chip_width =
            ti.compute_chip_width("removable").min(rect.width as i32 - TAG_PADDING * 2);
        let close_center = ti.tag_close_center(chip_x, chip_width, chip_y);

        // Click on the close button
        ti.handle_event(&Event::MousePress { pos: close_center, button: 1 });
        assert!(ti.tags().is_empty());
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
}
