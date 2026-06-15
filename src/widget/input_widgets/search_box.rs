//! SearchBox widget — a text input with search icon, placeholder, and clear button.
//!
//! The SearchBox presents a text field with a magnifying glass icon on the left,
//! placeholder text when empty, and a clear (X) button on the right when the
//! text is non-empty. It emits `text_changed` with the current text value on
//! every edit.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// SearchBox widget with search icon, placeholder, and clear button.
pub struct SearchBox {
    base: BaseWidget,
    text: String,
    placeholder: String,
    focused: bool,
    /// Emitted when the text changes, providing the new text value.
    pub text_changed: Signal1<String>,
}

impl SearchBox {
    /// Creates a new SearchBox widget with the given geometry.
    /// Uses "Search…" as the default placeholder text.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SearchBox, geometry, "SearchBox"),
            text: String::new(),
            placeholder: "Search\u{2026}".to_string(),
            focused: false,
            text_changed: Signal1::new(),
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the current text content (alias for `text()`).
    pub fn get_text(&self) -> &str {
        &self.text
    }

    /// Sets the text content. Emits `text_changed` if the text actually changes.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text.clone();
            self.text_changed.emit(text);
            self.base.request_redraw();
        }
    }

    /// Returns the current placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text shown when the search box is empty.
    pub fn set_placeholder(&mut self, placeholder: &str) {
        self.placeholder = placeholder.to_string();
        self.base.request_redraw();
    }

    /// Returns whether this search box currently has keyboard focus.
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

    /// Clears the text content. Emits `text_changed` if the text was non-empty.
    pub fn clear(&mut self) {
        if !self.text.is_empty() {
            self.text.clear();
            self.text_changed.emit(self.text.clone());
            self.base.request_redraw();
        }
    }
}

impl Widget for SearchBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for SearchBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let icon_size = 14;
        let icon_margin = 10;
        let clear_btn_size = 16;
        let clear_margin = 8;
        let text_x = rect.x + icon_margin + icon_size + 6;
        let text_width = if self.text.is_empty() {
            rect.width as i32 - (icon_margin + icon_size + 6 + clear_margin)
        } else {
            rect.width as i32
                - (icon_margin + icon_size + 6 + clear_margin + clear_btn_size + clear_margin)
        };
        let text_rect = Rect::new(text_x, rect.y, text_width.max(1) as u32, rect.height);
        let center_y = rect.y + rect.height as i32 / 2;

        // — Background —
        let bg_color = if !is_enabled {
            Color::rgba(240, 240, 240, 160)
        } else if self.focused {
            Color::rgba(245, 245, 255, 220)
        } else {
            Color::rgba(235, 235, 235, 200)
        };
        context.fill_rounded_rect(rect, 6, bg_color);

        // — Focus border —
        if self.focused && is_enabled {
            context.draw_rounded_rect_stroke(rect, 6, Color::rgba(60, 140, 255, 200), 2);
        } else {
            context.draw_rounded_rect_stroke(rect, 6, Color::rgba(200, 200, 200, 160), 1);
        }

        // — Search icon (magnifying glass) —
        let icon_cx = rect.x + icon_margin + icon_size / 2;
        let icon_cy = center_y;
        let icon_color = if is_enabled {
            Color::rgba(140, 140, 140, 220)
        } else {
            Color::rgba(180, 180, 180, 120)
        };

        // Draw circle part of magnifying glass
        let circle_radius = icon_size as u32 * 5 / 14; // ~5 pixel radius
        context.draw_circle(Point::new(icon_cx - 1, icon_cy - 1), circle_radius, icon_color);
        // Draw handle (angled line)
        let handle_start_x = icon_cx - 1 + circle_radius as i32;
        let handle_start_y = icon_cy - 1 + circle_radius as i32;
        let handle_end_x = handle_start_x + 4;
        let handle_end_y = handle_start_y + 4;
        context.draw_line(
            Point::new(handle_start_x, handle_start_y),
            Point::new(handle_end_x, handle_end_y),
            icon_color,
        );

        // — Text / Placeholder —
        let default_font = crate::core::Font::default();
        let font = self.font().unwrap_or(&default_font);
        let text_color = if !is_enabled {
            Color::rgba(160, 160, 160, 180)
        } else if !self.text.is_empty() {
            Color::rgba(30, 30, 30, 230)
        } else {
            Color::rgba(160, 160, 160, 200)
        };
        let display_text = if self.text.is_empty() { &self.placeholder } else { &self.text };
        let text_origin =
            Point::new(text_rect.x + 2, text_rect.y + text_rect.height as i32 / 2 + 4);
        context.draw_text(text_origin, display_text, font, text_color, HorizontalAlignment::Left);

        // — Clear button (X circle) —
        if !self.text.is_empty() && is_enabled {
            let clear_cx = rect.x + rect.width as i32 - clear_margin - clear_btn_size / 2;
            let clear_cy = center_y;
            let clear_radius = clear_btn_size as u32 / 2;
            // Circle background
            context.fill_circle(
                Point::new(clear_cx, clear_cy),
                clear_radius,
                Color::rgba(180, 180, 180, 200),
            );
            // X mark (two diagonal lines)
            let offset = 3;
            context.draw_line(
                Point::new(clear_cx - offset, clear_cy - offset),
                Point::new(clear_cx + offset, clear_cy + offset),
                Color::WHITE,
            );
            context.draw_line(
                Point::new(clear_cx + offset, clear_cy - offset),
                Point::new(clear_cx - offset, clear_cy + offset),
                Color::WHITE,
            );
        }
    }
}

impl EventHandler for SearchBox {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                // Check if clear button was clicked
                if !self.text.is_empty() {
                    let rect = self.geometry();
                    let clear_margin = 8;
                    let clear_btn_size = 16;
                    let center_y = rect.y + rect.height as i32 / 2;
                    let clear_cx = rect.x + rect.width as i32 - clear_margin - clear_btn_size / 2;
                    let clear_cy = center_y;
                    let dx = pos.x - clear_cx;
                    let dy = pos.y - clear_cy;
                    let clear_radius = clear_btn_size as u32 / 2;
                    // Hit-test within the clear button circle
                    if dx * dx + dy * dy <= (clear_radius as i32 * clear_radius as i32) {
                        self.clear();
                        return;
                    }
                }
                // Click on the search box → gain focus
                self.set_focused(true);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                // No special release handling needed
            }
            Event::FocusGained => {
                self.set_focused(true);
            }
            Event::FocusLost => {
                self.set_focused(false);
            }
            Event::KeyPress { key, modifiers: _ } => {
                if !self.focused {
                    return;
                }
                match *key {
                    8 => {
                        // Backspace — remove last character
                        if !self.text.is_empty() {
                            self.text.pop();
                            self.text_changed.emit(self.text.clone());
                            self.base.request_redraw();
                        }
                    }
                    127 => {
                        // Delete — remove last character
                        if !self.text.is_empty() {
                            self.text.pop();
                            self.text_changed.emit(self.text.clone());
                            self.base.request_redraw();
                        }
                    }
                    13 | 27 => {
                        // Enter / Escape — lose focus
                        self.set_focused(false);
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.text.push(ch);
                                self.text_changed.emit(self.text.clone());
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
    fn search_box_default_creation() {
        let sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        assert_eq!(sb.text(), "");
        assert_eq!(sb.get_text(), "");
        assert_eq!(sb.placeholder(), "Search\u{2026}");
        assert!(!sb.is_focused());
        assert_eq!(sb.kind(), WidgetKind::SearchBox);
        assert!(sb.is_enabled());
    }

    #[test]
    fn search_box_set_text_get_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("hello");
        assert_eq!(sb.text(), "hello");
        assert_eq!(sb.get_text(), "hello");
    }

    #[test]
    fn search_box_set_text_emits_signal() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let captured = Arc::new(Mutex::new(None::<String>));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        sb.set_text("world");
        assert_eq!(*captured.lock().unwrap(), Some("world".to_string()));
    }

    #[test]
    fn search_box_text_changed_signal_on_type() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let last = Arc::new(Mutex::new(String::new()));
        sb.text_changed.connect({
            let last = Arc::clone(&last);
            move |val: Arc<String>| {
                *last.lock().unwrap() = val.to_string();
            }
        });

        // Simulate typing 'a', 'b', 'c'
        sb.set_focused(true);
        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 }); // 'a'
        sb.handle_event(&Event::KeyPress { key: 98, modifiers: 0 }); // 'b'
        sb.handle_event(&Event::KeyPress { key: 99, modifiers: 0 }); // 'c'
        assert_eq!(sb.text(), "abc");
        assert_eq!(*last.lock().unwrap(), "abc");
    }

    #[test]
    fn search_box_placeholder_display() {
        let sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        // Default placeholder
        assert_eq!(sb.placeholder(), "Search\u{2026}");

        // Custom placeholder
        let mut sb_custom = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb_custom.set_placeholder("Find...");
        assert_eq!(sb_custom.placeholder(), "Find...");
    }

    #[test]
    fn search_box_clear_button_click() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("test text");
        assert_eq!(sb.text(), "test text");

        // Calculate clear button center position
        let clear_margin = 8;
        let clear_btn_size = 16;
        let rect = sb.geometry();
        let center_y = rect.y + rect.height as i32 / 2;
        let clear_cx = rect.x + rect.width as i32 - clear_margin - clear_btn_size / 2;
        let clear_cy = center_y;

        // Click the clear button
        sb.handle_event(&Event::MousePress { pos: Point::new(clear_cx, clear_cy), button: 1 });

        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_clear_button_emits_signal() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let captured = Arc::new(Mutex::new(None::<String>));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        sb.set_text("hello");

        // Clear via method
        sb.clear();
        assert_eq!(sb.text(), "");
        assert_eq!(*captured.lock().unwrap(), Some("".to_string()));
    }

    #[test]
    fn search_box_backspace() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("hello");
        sb.set_focused(true);

        sb.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert_eq!(sb.text(), "hell");

        sb.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert_eq!(sb.text(), "hel");
    }

    #[test]
    fn search_box_set_placeholder_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_placeholder("Type to search...");
        assert_eq!(sb.placeholder(), "Type to search...");
    }

    #[test]
    fn search_box_focus_on_click() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        assert!(!sb.is_focused());

        sb.handle_event(&Event::MousePress { pos: Point::new(50, 16), button: 1 });
        assert!(sb.is_focused());
    }

    #[test]
    fn search_box_focus_lost_on_escape() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_focused(true);
        assert!(sb.is_focused());

        sb.handle_event(&Event::KeyPress { key: 27, modifiers: 0 }); // Escape
        assert!(!sb.is_focused());
    }

    #[test]
    fn search_box_disabled_blocks_events() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_enabled(false);

        sb.handle_event(&Event::MousePress { pos: Point::new(50, 16), button: 1 });
        assert!(!sb.is_focused());
        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_disabled_blocks_keyboard() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_enabled(false);
        sb.set_focused(true);

        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 }); // 'a'
        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_svg_output() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let svg = crate::widget::svg::render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"32\""));
    }

    #[test]
    fn search_box_svg_with_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("testing");
        let svg = crate::widget::svg::render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn search_box_clear_non_empty_only() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let captured = Arc::new(Mutex::new(0));
        let c = captured.clone();
        sb.text_changed.connect(move |_: Arc<String>| {
            *c.lock().unwrap() += 1;
        });

        // Clear when already empty — should not emit
        sb.clear();
        assert_eq!(*captured.lock().unwrap(), 0);

        // Set text and clear
        sb.set_text("data");
        assert_eq!(*captured.lock().unwrap(), 1);
        sb.clear();
        assert_eq!(*captured.lock().unwrap(), 2);
        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_no_duplicate_emit_on_set_same_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let count = Arc::new(Mutex::new(0));
        let c = count.clone();
        sb.text_changed.connect(move |_: Arc<String>| {
            *c.lock().unwrap() += 1;
        });

        sb.set_text("hello");
        sb.set_text("hello"); // same text — should not emit
        assert_eq!(*count.lock().unwrap(), 1);
    }
}
