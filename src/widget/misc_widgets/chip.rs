//! Chip/Tag widget — a compact label/tag element with optional close button and selection state.
//!
//! The Chip widget represents a small rounded label used for tags, tokens, or filter pills,
//! similar to Android Chip or iOS Tag views. It supports a text label, optional close
//! button (X), optional selected/active state, and click/close signals.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Spacing between text and close button.
const CHIP_PADDING: i32 = 8;
/// Close button radius.
const CLOSE_RADIUS: i32 = 7;
/// Gap between text end and close button.
const CLOSE_GAP: i32 = 6;
/// Chip/Tag widget for compact label/token display.
pub struct Chip {
    base: BaseWidget,
    text: String,
    closable: bool,
    selected: bool,
    /// Emitted when the close button (X) is clicked.
    pub closed: GenericSignal,
    /// Emitted when the chip body is clicked (excluding close button).
    pub clicked: GenericSignal,
}

impl Chip {
    /// Creates a new Chip widget with the given geometry.
    /// Initial state: empty text, not closable, not selected.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chip, geometry, "Chip"),
            text: String::new(),
            closable: false,
            selected: false,
            closed: GenericSignal::new(),
            clicked: GenericSignal::new(),
        }
    }

    /// Returns the current label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the label text.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.base.request_redraw();
    }

    /// Sets whether the chip shows a close button (X).
    pub fn set_closable(&mut self, closable: bool) {
        if self.closable != closable {
            self.closable = closable;
            self.base.request_redraw();
        }
    }

    /// Returns whether the chip shows a close button.
    pub fn is_closable(&self) -> bool {
        self.closable
    }

    /// Sets the selected/active state. When selected, the chip uses a
    /// different background/text color to indicate activity.
    pub fn set_selected(&mut self, selected: bool) {
        if self.selected != selected {
            self.selected = selected;
            self.base.request_redraw();
        }
    }

    /// Returns whether the chip is in the selected/active state.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    // ── Private helpers ──

    /// Returns the close button center point, or `None` if the chip is not closable.
    fn close_button_center(&self) -> Option<Point> {
        if !self.closable {
            return None;
        }
        let rect = self.geometry();
        Some(Point::new(
            rect.x + rect.width as i32 - CHIP_PADDING - CLOSE_RADIUS,
            rect.y + rect.height as i32 / 2,
        ))
    }

    /// Returns whether the given point is inside the close button hit area.
    fn hit_close_button(&self, pos: Point) -> bool {
        self.close_button_center().is_some_and(|center| {
            let dx = (pos.x - center.x) as i64;
            let dy = (pos.y - center.y) as i64;
            dx * dx + dy * dy <= (CLOSE_RADIUS as i64 + 2) * (CLOSE_RADIUS as i64 + 2)
        })
    }
}

impl Widget for Chip {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Chip {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let corner_radius = (rect.height.min(32) as f32 / 2.0) as u32;

        // ── Background ──
        let bg_color = if !is_enabled {
            Color::rgba(200, 200, 200, 100)
        } else if self.selected {
            Color::from_rgb(25, 118, 210) // Material blue
        } else {
            Color::rgba(220, 220, 220, 200)
        };
        context.fill_rounded_rect(rect, corner_radius, bg_color);

        // ── Border ──
        let border_color = if !is_enabled {
            Color::rgba(180, 180, 180, 80)
        } else if self.selected {
            Color::from_rgb(21, 101, 192)
        } else {
            Color::rgba(190, 190, 190, 150)
        };
        context.draw_rounded_rect_stroke(rect, corner_radius, border_color, 1);

        // ── Text ──
        let text_color = if !is_enabled {
            Color::rgba(160, 160, 160, 180)
        } else if self.selected {
            Color::WHITE
        } else {
            Color::from_rgb(33, 33, 33)
        };

        // Text area: from left padding up to close button (if closable) or right padding
        let text_end = if self.closable {
            rect.x + rect.width as i32 - CHIP_PADDING - CLOSE_RADIUS * 2 - CLOSE_GAP
        } else {
            rect.x + rect.width as i32 - CHIP_PADDING
        };
        let text_x = rect.x + CHIP_PADDING;
        let text_origin = Point::new(text_x, rect.y + rect.height as i32 / 2);

        // Measure text to handle truncation
        let metrics = context.measure_text(&self.text, &Font::default());
        let text_width = metrics.width as i32;
        let max_text_width = (text_end - text_x).max(0);
        let display_text = if text_width > max_text_width && max_text_width > 0 {
            // Truncate with ellipsis
            let font = Font::default();
            let mut truncated = String::new();
            for c in self.text.chars() {
                let candidate = format!("{}{}", truncated, c);
                let w = context.measure_text(&candidate, &font).width as i32;
                if w + context.measure_text("…", &font).width as i32 > max_text_width {
                    break;
                }
                truncated.push(c);
            }
            truncated.push('…');
            truncated
        } else {
            self.text.clone()
        };

        if !display_text.is_empty() {
            context.draw_text(text_origin, &display_text, &Font::default(), text_color, HorizontalAlignment::Left);
        }

        // ── Close button ──
        if self.closable && is_enabled {
            if let Some(center) = self.close_button_center() {
                let close_bg = if self.selected {
                    Color::rgba(255, 255, 255, 200)
                } else {
                    Color::rgba(180, 180, 180, 200)
                };
                let close_fg = if self.selected {
                    Color::from_rgb(25, 118, 210)
                } else {
                    Color::rgba(80, 80, 80, 220)
                };

                // Draw close button circle
                context.fill_circle(center, CLOSE_RADIUS as u32, close_bg);
                context.draw_circle_stroke(
                    center,
                    CLOSE_RADIUS as u32,
                    Color::rgba(0, 0, 0, 40),
                    1,
                );

                // Draw X: two crossing lines
                let x_offset = (CLOSE_RADIUS as f32 * 0.45) as i32;
                context.draw_line(
                    Point::new(center.x - x_offset, center.y - x_offset),
                    Point::new(center.x + x_offset, center.y + x_offset),
                    close_fg,
                );
                context.draw_line(
                    Point::new(center.x + x_offset, center.y - x_offset),
                    Point::new(center.x - x_offset, center.y + x_offset),
                    close_fg,
                );
            }
        }
    }
}

impl EventHandler for Chip {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if self.hit_close_button(*pos) {
                        self.closed.emit();
                    } else if self.geometry().contains_point(*pos) {
                        self.clicked.emit();
                    }
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    // No special handling needed
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
    fn chip_default_creation() {
        let chip = Chip::new(Rect::new(0, 0, 100, 28));
        assert_eq!(chip.text(), "");
        assert!(!chip.is_closable());
        assert!(!chip.is_selected());
        assert_eq!(chip.kind(), WidgetKind::Chip);
    }

    #[test]
    fn chip_text_accessor() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        assert_eq!(chip.text(), "");
        chip.set_text("Rust");
        assert_eq!(chip.text(), "Rust");
        chip.set_text("Tag");
        assert_eq!(chip.text(), "Tag");
    }

    #[test]
    fn chip_closeable_click_emits_closed_signal() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        chip.set_text("Chip");
        chip.set_closable(true);
        assert!(chip.is_closable());

        let closed_fired = Arc::new(Mutex::new(false));
        chip.closed.connect({
            let closed_fired = Arc::clone(&closed_fired);
            move || {
                *closed_fired.lock().unwrap() = true;
            }
        });

        // Click on the close button area (right side of chip)
        // close_button_center = (100 - 8 - 7, 14) = (85, 14)
        chip.handle_event(&Event::mouse_press(85, 14, 1));
        assert!(*closed_fired.lock().unwrap());
    }

    #[test]
    fn chip_close_click_inside_close_button_does_not_emit_clicked() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        chip.set_text("Test");
        chip.set_closable(true);

        let clicked_fired = Arc::new(Mutex::new(false));
        chip.clicked.connect({
            let clicked_fired = Arc::clone(&clicked_fired);
            move || {
                *clicked_fired.lock().unwrap() = true;
            }
        });

        // Click on close button
        chip.handle_event(&Event::mouse_press(85, 14, 1));
        assert!(!*clicked_fired.lock().unwrap());
    }

    #[test]
    fn chip_clicked_signal() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        chip.set_text("Click Me");

        let clicked_fired = Arc::new(Mutex::new(false));
        chip.clicked.connect({
            let clicked_fired = Arc::clone(&clicked_fired);
            move || {
                *clicked_fired.lock().unwrap() = true;
            }
        });

        // Click on chip body (not on close button)
        chip.handle_event(&Event::mouse_press(20, 14, 1));
        assert!(*clicked_fired.lock().unwrap());
    }

    #[test]
    fn chip_selection_state() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        assert!(!chip.is_selected());

        chip.set_selected(true);
        assert!(chip.is_selected());

        chip.set_selected(false);
        assert!(!chip.is_selected());
    }

    #[test]
    fn chip_set_selected_does_not_emit_signals() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        let clicked_fired = Arc::new(Mutex::new(false));
        let closed_fired = Arc::new(Mutex::new(false));

        chip.clicked.connect({
            let clicked_fired = Arc::clone(&clicked_fired);
            move || {
                *clicked_fired.lock().unwrap() = true;
            }
        });
        chip.closed.connect({
            let closed_fired = Arc::clone(&closed_fired);
            move || {
                *closed_fired.lock().unwrap() = true;
            }
        });

        chip.set_selected(true);
        assert!(!*clicked_fired.lock().unwrap());
        assert!(!*closed_fired.lock().unwrap());
    }

    #[test]
    fn chip_disabled_blocks_events() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        chip.set_text("Disabled");
        chip.set_enabled(false);

        let clicked_fired = Arc::new(Mutex::new(false));
        chip.clicked.connect({
            let clicked_fired = Arc::clone(&clicked_fired);
            move || {
                *clicked_fired.lock().unwrap() = true;
            }
        });

        chip.handle_event(&Event::mouse_press(20, 14, 1));
        assert!(!*clicked_fired.lock().unwrap());
    }

    #[test]
    fn chip_closable_property() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        assert!(!chip.is_closable());

        chip.set_closable(true);
        assert!(chip.is_closable());

        chip.set_closable(false);
        assert!(!chip.is_closable());
    }

    #[test]
    fn chip_svg_output() {
        let mut chip = Chip::new(Rect::new(0, 0, 120, 30));
        chip.set_text("SVG Test");
        chip.set_closable(true);
        chip.set_selected(true);
        let svg = crate::widget::svg::render_to_svg(&mut chip);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"120\""));
        assert!(svg.contains("height=\"30\""));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn chip_close_button_non_closable_no_emit() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        chip.set_text("No Close");
        // closable is false by default

        let closed_fired = Arc::new(Mutex::new(false));
        chip.closed.connect({
            let closed_fired = Arc::clone(&closed_fired);
            move || {
                *closed_fired.lock().unwrap() = true;
            }
        });

        // Click where close button would be
        chip.handle_event(&Event::mouse_press(85, 14, 1));
        assert!(!*closed_fired.lock().unwrap());
    }

    #[test]
    fn chip_mouse_press_outside_does_not_emit() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        chip.set_text("Outside");

        let clicked_fired = Arc::new(Mutex::new(false));
        chip.clicked.connect({
            let clicked_fired = Arc::clone(&clicked_fired);
            move || {
                *clicked_fired.lock().unwrap() = true;
            }
        });

        // Click far outside chip bounds
        chip.handle_event(&Event::mouse_press(200, 200, 1));
        assert!(!*clicked_fired.lock().unwrap());
    }

    #[test]
    fn chip_right_mouse_button_ignored() {
        let mut chip = Chip::new(Rect::new(0, 0, 100, 28));
        chip.set_text("Right Click");

        let clicked_fired = Arc::new(Mutex::new(false));
        chip.clicked.connect({
            let clicked_fired = Arc::clone(&clicked_fired);
            move || {
                *clicked_fired.lock().unwrap() = true;
            }
        });

        // Right click (button 2) should be ignored
        chip.handle_event(&Event::mouse_press(20, 14, 2));
        assert!(!*clicked_fired.lock().unwrap());
    }
}
