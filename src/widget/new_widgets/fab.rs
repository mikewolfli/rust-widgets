//! FAB (Floating Action Button) widget — a circular floating button for
//! primary actions.
//!
//! The FAB is a Material Design-style circular button that floats above the UI.
//! It displays an icon as a text character (e.g., "+") and supports press
//! animation, shadow, and click signal emission.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Floating Action Button widget.
///
/// A circular button with a background color (typically accent), an icon
/// rendered as centered text, a drop shadow, and a press animation that
/// briefly shrinks the button. Emits `clicked` on press-release.
pub struct FAB {
    base: BaseWidget,
    /// The icon text character displayed in the center (e.g. "+", "✕", "↓").
    icon_text: String,
    /// The fill color of the circular button.
    accent_color: Color,
    /// Whether this is a mini FAB (smaller size).
    mini: bool,
    /// Whether the button is currently pressed (for press animation).
    pressed: bool,
}

impl FAB {
    /// Creates a new FAB widget with the given geometry.
    ///
    /// Defaults: icon text "+", accent color `Color::PRIMARY`, normal size.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FAB, geometry, "FAB"),
            icon_text: String::from("+"),
            accent_color: Color::PRIMARY,
            mini: false,
            pressed: false,
        }
    }

    /// Sets the icon text displayed in the center of the button.
    ///
    /// Common values: `"+"`, `"✕"`, `"↓"`, `"↑"`, `"✓"`, `"✎"`.
    pub fn set_icon_text(&mut self, text: &str) {
        self.icon_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current icon text.
    pub fn icon_text(&self) -> &str {
        &self.icon_text
    }

    /// Sets the accent (fill) color of the circular button.
    pub fn set_accent_color(&mut self, color: Color) {
        self.accent_color = color;
        self.base.request_redraw();
    }

    /// Returns the current accent color.
    pub fn accent_color(&self) -> Color {
        self.accent_color
    }

    /// Sets whether this FAB renders in mini (smaller) mode.
    ///
    /// Mini FABs are typically 40×40 instead of the standard 56×56.
    /// The geometry rect should be adjusted separately; this flag controls
    /// visual proportions like shadow offset and press scale factor.
    pub fn set_mini(&mut self, mini: bool) {
        self.mini = mini;
        self.base.request_redraw();
    }

    /// Returns whether this FAB is in mini mode.
    pub fn is_mini(&self) -> bool {
        self.mini
    }

    /// Programmatically trigger a click cycle (press then release).
    pub fn click(&mut self) {
        self.base.clicked.emit();
    }
}

impl Widget for FAB {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for FAB {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let dpi = self.base.dpi_scale();
        let is_enabled = self.base.is_enabled();

        // Compute button center and radius
        let cx = rect.x + (rect.width as i32) / 2;
        let cy = rect.y + (rect.height as i32) / 2;
        let base_radius = rect.width.min(rect.height) / 2;

        // When pressed, shrink by ~10% for press animation
        let (radius, shadow_offset) = if self.pressed && is_enabled {
            let shrunk = (base_radius as f32 * 0.9) as u32;
            (shrunk, (2.0 * dpi) as u32)
        } else {
            (base_radius, (3.0 * dpi) as u32)
        };

        let center = Point::new(cx, cy);

        // Draw shadow (offset darker circle underneath)
        let shadow_center =
            Point::new(cx + shadow_offset as i32 / 2, cy + shadow_offset as i32 / 2);
        let shadow_color = Color::rgba(0, 0, 0, 60);
        context.fill_circle_aa(shadow_center, radius, shadow_color);

        // Determine button fill color
        let fill_color = if !is_enabled {
            Color::rgba(self.accent_color.r, self.accent_color.g, self.accent_color.b, 120)
        } else {
            self.accent_color
        };

        // Draw filled circle
        context.fill_circle_aa(center, radius, fill_color);

        // Draw icon text centered in the button
        if !self.icon_text.is_empty() {
            // Use a default monospace-like font at a size proportional to button
            let font_size = if self.mini {
                (radius as f32 * 0.7).max(12.0)
            } else {
                (radius as f32 * 0.7).max(16.0)
            };
            use crate::core::Font;
            let font = Font::new("sans-serif", font_size, false, false);

            let metrics = context.measure_text(&self.icon_text, &font);
            let text_x = cx - (metrics.width as i32) / 2;
            let text_y = cy - (metrics.height as i32) / 2 + (metrics.ascent as i32);

            let text_color =
                if !is_enabled { Color::rgba(255, 255, 255, 120) } else { Color::WHITE };
            context.draw_text(Point::new(text_x, text_y), &self.icon_text, &font, text_color);
        }
    }
}

impl EventHandler for FAB {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos: _, button } => {
                if *button == 1 {
                    self.pressed = true;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 && self.pressed {
                    self.pressed = false;
                    self.base.clicked.emit();
                    self.base.request_redraw();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } => {
                self.pressed = true;
                self.base.request_redraw();
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                if self.pressed {
                    self.pressed = false;
                    self.base.clicked.emit();
                    self.base.request_redraw();
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } => {
                self.base.clicked.emit();
                self.base.request_redraw();
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn make_fab() -> FAB {
        FAB::new(Rect::new(0, 0, 56, 56))
    }

    #[test]
    fn fab_default_creation() {
        let fab = make_fab();
        assert_eq!(fab.kind(), WidgetKind::FAB);
        assert_eq!(fab.icon_text(), "+");
        assert_eq!(fab.accent_color(), Color::PRIMARY);
        assert!(!fab.is_mini());
        assert_eq!(fab.geometry(), Rect::new(0, 0, 56, 56));
        assert!(fab.is_visible());
        assert!(fab.is_enabled());
    }

    #[test]
    fn fab_icon_text() {
        let mut fab = make_fab();
        assert_eq!(fab.icon_text(), "+");

        fab.set_icon_text("✕");
        assert_eq!(fab.icon_text(), "✕");

        fab.set_icon_text("↓");
        assert_eq!(fab.icon_text(), "↓");
    }

    #[test]
    fn fab_accent_color() {
        let mut fab = make_fab();
        assert_eq!(fab.accent_color(), Color::PRIMARY);

        fab.set_accent_color(Color::ERROR);
        assert_eq!(fab.accent_color(), Color::ERROR);

        fab.set_accent_color(Color::SUCCESS);
        assert_eq!(fab.accent_color(), Color::SUCCESS);
    }

    #[test]
    fn fab_mini_mode() {
        let mut fab = make_fab();
        assert!(!fab.is_mini());

        fab.set_mini(true);
        assert!(fab.is_mini());

        fab.set_mini(false);
        assert!(!fab.is_mini());
    }

    #[test]
    fn fab_click_signal_emits() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.click();
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_mouse_press_release_emits_clicked() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 1 });
        assert!(fab.pressed);
        assert!(!clicked.load(Ordering::SeqCst));

        fab.handle_event(&Event::MouseRelease { pos: Point::new(28, 28), button: 1 });
        assert!(!fab.pressed);
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_mouse_press_release_other_button_noop() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 2 });
        assert!(!fab.pressed);
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_disabled_blocks_events() {
        let mut fab = make_fab();
        fab.set_enabled(false);

        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 1 });
        assert!(!fab.pressed);

        fab.handle_event(&Event::MouseRelease { pos: Point::new(28, 28), button: 1 });
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn fab_tap_emits_clicked() {
        let mut fab = make_fab();
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        fab.clicked_signal().connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        fab.handle_event(&Event::Tap { pos: Point::new(28, 28) });
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn fab_svg_output() {
        let mut fab = make_fab();
        let svg = render_to_svg(&mut fab);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"56\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn fab_svg_output_mini() {
        let mut fab = FAB::new(Rect::new(0, 0, 40, 40));
        fab.set_mini(true);
        let svg = render_to_svg(&mut fab);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"40\""));
        assert!(svg.contains("height=\"40\""));
    }

    #[test]
    fn fab_press_animation_sets_pressed_state() {
        let mut fab = make_fab();
        assert!(!fab.pressed);

        fab.handle_event(&Event::MousePress { pos: Point::new(28, 28), button: 1 });
        assert!(fab.pressed);

        fab.handle_event(&Event::MouseRelease { pos: Point::new(28, 28), button: 1 });
        assert!(!fab.pressed);
    }

    #[test]
    fn fab_clicked_signal_accessor() {
        let fab = make_fab();
        let signal = fab.clicked_signal();
        // Signal should be valid and connectable
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }
}
