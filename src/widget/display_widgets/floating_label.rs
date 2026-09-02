//! FloatingLabel widget — a text input with a floating label (Material Design style).
//!
//! The FloatingLabel widget combines a text input field with a label that
//! animates from inside the field to above it when the field is focused or
//! contains text. It also supports placeholder text that is shown when the
//! field is empty and unfocused.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A text input with a floating label (Material Design TextInputLayout style).
///
/// The label floats above the text field when the field is focused or contains
/// text. When the field is empty and unfocused, the label appears inside the
/// field (or a separate placeholder text is shown). The transition between
/// states can be animated via `animation_progress` (0.0 = label inside field,
/// 1.0 = label fully above).
pub struct FloatingLabel {
    base: BaseWidget,
    text: String,
    label: String,
    placeholder: String,
    is_focused: bool,
    show_label_above: bool,
    animation_progress: f32,
    /// Emitted when the text content changes.
    pub text_changed: Signal1<String>,
}

impl FloatingLabel {
    /// Creates a new FloatingLabel widget with the given geometry.
    ///
    /// By default, no text, empty label, empty placeholder, unfocused.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FloatingLabel, geometry, "FloatingLabel"),
            text: String::new(),
            label: String::new(),
            placeholder: String::new(),
            is_focused: false,
            show_label_above: false,
            animation_progress: 0.0,
            text_changed: Signal1::new(),
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content and emits `text_changed` signal.
    /// Also updates the floating label state based on whether text is non-empty.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.update_label_state();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Returns the label text.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Sets the label text (shown inside field or floating above).
    pub fn set_label(&mut self, label: String) {
        self.label = label;
        self.update_label_state();
        self.base.request_redraw();
    }

    /// Returns the placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text (shown when empty and unfocused).
    pub fn set_placeholder(&mut self, placeholder: String) {
        self.placeholder = placeholder;
        self.base.request_redraw();
    }

    /// Returns whether the input field is currently focused.
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Returns whether the text content is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Sets the focused state. When focused, the label floats above.
    pub fn set_focused(&mut self, focused: bool) {
        if self.is_focused != focused {
            self.is_focused = focused;
            self.update_label_state();
            self.base.request_redraw();
        }
    }

    /// Updates whether the label should float above based on focus and content.
    fn update_label_state(&mut self) {
        let should_float = self.is_focused || !self.text.is_empty();
        if should_float != self.show_label_above {
            self.show_label_above = should_float;
            // In a real implementation, this would trigger an animation.
            // For simplicity, we set animation_progress to 0 or 1.
            self.animation_progress = if should_float { 1.0 } else { 0.0 };
        }
    }
}

impl Widget for FloatingLabel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 40)
    }
}

impl Draw for FloatingLabel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Draw the text field background
        let bg_color = if is_enabled {
            Color::rgba(255, 255, 255, 255)
        } else {
            Color::rgba(240, 240, 240, 255)
        };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Draw the underline/border
        let border_color = if self.is_focused {
            Color::rgba(52, 120, 246, 255)
        } else {
            Color::rgba(180, 180, 180, 255)
        };
        let underline_y = rect.y + rect.height as i32 - 2;
        let underline_rect = Rect::new(rect.x + 2, underline_y, rect.width - 4, 2);
        context.fill_rounded_rect(underline_rect, 1, border_color);

        // Fonts
        let input_font = Font::simple("sans-serif", 14.0);
        let label_font = Font::simple("sans-serif", 11.0);
        let padding = 8i32;
        let label_top_margin = 4i32;

        // Calculate positions
        let has_label = !self.label.is_empty();
        let text_field_top_offset = if has_label && self.show_label_above {
            16i32 // space for floating label
        } else {
            6i32
        };

        // Draw the label (floating above or inline)
        if has_label {
            let label_color = if self.is_focused {
                Color::rgba(52, 120, 246, 255)
            } else if is_enabled {
                Color::rgba(100, 100, 100, 255)
            } else {
                Color::rgba(180, 180, 180, 255)
            };

            if self.show_label_above {
                // Floating label above
                let label_x = rect.x + padding;
                let label_y = rect.y + label_top_margin + 10;
                context.draw_text(
                    Point::new(label_x, label_y),
                    &self.label,
                    &label_font,
                    label_color,
                    HorizontalAlignment::Left,
                );
            } else if self.text.is_empty() && !self.is_focused {
                // Label inline acts as placeholder
                let label_x = rect.x + padding;
                let label_y = rect.y + text_field_top_offset + 14;
                context.draw_text(
                    Point::new(label_x, label_y),
                    &self.label,
                    &input_font,
                    Color::rgba(160, 160, 160, 255),
                    HorizontalAlignment::Left,
                );
            }
        }

        // Show placeholder when empty, unfocused, and label is not shown inline
        let show_placeholder =
            self.text.is_empty() && !self.is_focused && (!has_label || self.show_label_above);

        if show_placeholder && !self.placeholder.is_empty() {
            let placeholder_x = rect.x + padding;
            let placeholder_y = rect.y + text_field_top_offset + 14;
            context.draw_text(
                Point::new(placeholder_x, placeholder_y),
                &self.placeholder,
                &input_font,
                Color::rgba(180, 180, 180, 255),
                HorizontalAlignment::Left,
            );
        }

        // Draw input text
        if !self.text.is_empty() {
            let text_x = rect.x + padding;
            let text_y = rect.y + text_field_top_offset + 14;
            let text_color = if is_enabled {
                Color::rgba(0, 0, 0, 255)
            } else {
                Color::rgba(160, 160, 160, 255)
            };
            context.draw_text(
                Point::new(text_x, text_y),
                &self.text,
                &input_font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

const KEYCODE_TAB: u32 = 9;
const KEYCODE_ENTER: u32 = 13;
const KEYCODE_BACKSPACE: u32 = 8;

impl EventHandler for FloatingLabel {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();
                if rect.contains_point(*pos) {
                    self.set_focused(true);
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == KEYCODE_TAB {
                    // Tab — lose focus
                    self.set_focused(false);
                } else if *key == KEYCODE_ENTER {
                    // Enter — lose focus
                    self.set_focused(false);
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII — append to text
                    let c = char::from_u32(*key).unwrap_or(' ');
                    if self.is_focused {
                        let mut new_text = self.text.clone();
                        new_text.push(c);
                        self.set_text(new_text);
                    }
                } else if *key == KEYCODE_BACKSPACE {
                    // Backspace
                    if self.is_focused && !self.text.is_empty() {
                        let mut new_text = self.text.clone();
                        new_text.pop();
                        self.set_text(new_text);
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
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    #[test]
    fn floating_label_default_creation() {
        let fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        assert_eq!(fl.kind(), WidgetKind::FloatingLabel);
        assert!(fl.text().is_empty());
        assert!(fl.label().is_empty());
        assert!(fl.placeholder().is_empty());
        assert!(!fl.is_focused());
        assert!(fl.is_empty());
    }

    #[test]
    fn floating_label_set_text_and_label() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Username".to_string());
        assert_eq!(fl.label(), "Username");

        fl.set_text("hello".to_string());
        assert_eq!(fl.text(), "hello");
        assert!(!fl.is_empty());

        // Label should float above since text is non-empty
        assert!(fl.show_label_above);
        assert!((fl.animation_progress - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn floating_label_focus_toggle() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Email".to_string());
        assert!(!fl.is_focused());
        assert!(!fl.show_label_above);

        fl.set_focused(true);
        assert!(fl.is_focused());
        assert!(fl.show_label_above);

        fl.set_focused(false);
        assert!(!fl.is_focused());
        // Should still float since text is empty? No, empty + not focused = not floating
        assert!(!fl.show_label_above);
    }

    #[test]
    fn floating_label_text_changed_signal() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        let captured = Arc::new(Mutex::new(None::<String>));
        fl.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        fl.set_text("World".to_string());
        assert_eq!(captured.lock().unwrap().as_deref(), Some("World"));
    }

    #[test]
    fn floating_label_placeholder() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_placeholder("Enter text here...".to_string());
        assert_eq!(fl.placeholder(), "Enter text here...");
    }

    #[test]
    fn floating_label_focus_on_click() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        assert!(!fl.is_focused());

        fl.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(fl.is_focused());
    }

    #[test]
    fn floating_label_keyboard_input() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_focused(true);

        // Type 'A'
        fl.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
        assert_eq!(fl.text(), "A");

        // Type 'B'
        fl.handle_event(&Event::KeyPress { key: 66, modifiers: 0 });
        assert_eq!(fl.text(), "AB");

        // Backspace
        fl.handle_event(&Event::KeyPress { key: 8, modifiers: 0 });
        assert_eq!(fl.text(), "A");
    }

    #[test]
    fn floating_label_svg_output() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Name".to_string());
        fl.set_placeholder("Enter name".to_string());
        fl.set_text("John".to_string());
        let svg = render_to_svg(&mut fl);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
