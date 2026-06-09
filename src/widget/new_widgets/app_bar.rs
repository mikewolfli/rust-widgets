//! AppBar (Top Bar) widget — a mobile-style top navigation bar with title,
//! optional back button, and optional action text.
//!
//! The AppBar is a Material Design-inspired top app bar that displays a title
//! centered in the bar, an optional back arrow (←) on the left, and optional
//! action text on the right. It emits `back_pressed` when the back area is
//! tapped and `action_pressed` when the action area is tapped.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// AppBar / Top Bar widget — mobile-style top navigation bar.
///
/// Displays a title centered in the bar, an optional back arrow (←) on the
/// left, and optional action text on the right. Emits signals when the back
/// or action areas are pressed.
pub struct AppBar {
    base: BaseWidget,
    /// The title text centered in the bar.
    title: String,
    /// Whether to show the back arrow (←) on the left side.
    show_back: bool,
    /// Optional action text displayed on the right side.
    action_text: String,
    /// Emitted when the back arrow area is pressed.
    pub back_pressed: GenericSignal,
    /// Emitted when the action text area is pressed.
    pub action_pressed: GenericSignal,
}

impl AppBar {
    /// Creates a new AppBar widget with the given title and geometry.
    ///
    /// By default, the back arrow is hidden and the action text is empty.
    pub fn new(title: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AppBar, geometry, "AppBar"),
            title: title.to_string(),
            show_back: false,
            action_text: String::new(),
            back_pressed: GenericSignal::new(),
            action_pressed: GenericSignal::new(),
        }
    }

    /// Sets the title text displayed centered in the bar.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the current title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets whether the back arrow (←) is shown on the left side.
    pub fn set_show_back(&mut self, show_back: bool) {
        self.show_back = show_back;
        self.base.request_redraw();
    }

    /// Returns whether the back arrow is currently shown.
    pub fn show_back(&self) -> bool {
        self.show_back
    }

    /// Sets the action text displayed on the right side.
    ///
    /// Pass an empty string to hide the action area.
    pub fn set_action_text(&mut self, text: &str) {
        self.action_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current action text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }
}

impl Widget for AppBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for AppBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let bar_height = rect.height;

        // Draw background
        let bg_color =
            if is_enabled { Color::rgba(248, 248, 250, 255) } else { Color::DISABLED_BACKGROUND };
        context.fill_rect(rect, bg_color);

        // Draw bottom border line
        let border_y = rect.y + bar_height as i32 - 1;
        context.draw_line_stroke(
            Point::new(rect.x, border_y),
            Point::new(rect.x + rect.width as i32, border_y),
            Color::DIVIDER,
            1,
        );

        // Determine font sizes based on bar height
        let title_font_size = (bar_height as f32 * 0.38).max(14.0).min(22.0);
        let action_font_size = (bar_height as f32 * 0.32).max(12.0).min(18.0);

        // ── Back arrow (left side) ──
        if self.show_back {
            let back_font = Font::new("sans-serif", action_font_size + 2.0, false, false);
            let back_text = "←";
            let metrics = context.measure_text(back_text, &back_font);
            // Left margin: ~12px, back arrow centered vertically
            let back_x = rect.x + 12;
            let back_y = rect.y + (bar_height as i32 / 2) + (metrics.ascent as i32 / 2)
                - (metrics.descent as i32 / 2);
            let back_color =
                if is_enabled { Color::FOREGROUND } else { Color::DISABLED_FOREGROUND };
            context.draw_text(Point::new(back_x, back_y), back_text, &back_font, back_color);
        }

        // ── Centered title ──
        if !self.title.is_empty() {
            let title_font = Font::new("sans-serif", title_font_size, false, false);
            let metrics = context.measure_text(&self.title, &title_font);

            // Compute available width: reserve space for back (40px) and action (80px)
            let left_reserve = if self.show_back { 40 } else { 16 };
            let right_reserve = if self.action_text.is_empty() { 16 } else { 80 };
            let available_width = rect.width as i32 - left_reserve - right_reserve;
            let title_width = metrics.width as i32;

            let title_x = if title_width > available_width {
                // Overflow: left-align with left margin
                rect.x + left_reserve
            } else {
                // Center
                rect.x + (rect.width as i32 / 2) - (title_width / 2)
            };
            let title_y = rect.y + (bar_height as i32 / 2) + (metrics.ascent as i32 / 2)
                - (metrics.descent as i32 / 2);

            let title_color =
                if is_enabled { Color::FOREGROUND } else { Color::DISABLED_FOREGROUND };
            context.draw_text(Point::new(title_x, title_y), &self.title, &title_font, title_color);
        }

        // ── Action text (right side) ──
        if !self.action_text.is_empty() {
            let action_font = Font::new("sans-serif", action_font_size, false, false);
            let metrics = context.measure_text(&self.action_text, &action_font);

            // Right margin: ~16px
            let action_x = rect.x + rect.width as i32 - metrics.width as i32 - 16;
            let action_y = rect.y + (bar_height as i32 / 2) + (metrics.ascent as i32 / 2)
                - (metrics.descent as i32 / 2);

            let action_color = if is_enabled { Color::PRIMARY } else { Color::DISABLED_FOREGROUND };
            context.draw_text(
                Point::new(action_x, action_y),
                &self.action_text,
                &action_font,
                action_color,
            );
        }
    }
}

impl EventHandler for AppBar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                let rect = self.geometry();

                // Determine tap zone
                // Left zone (back arrow): first 48px if show_back
                // Right zone (action): last 80px if action_text is non-empty
                // Title/general area: middle (handled below)

                if self.show_back && pos.x >= rect.x && pos.x <= rect.x + 48 {
                    self.back_pressed.emit();
                    self.base.request_redraw();
                    return;
                }

                if !self.action_text.is_empty()
                    && pos.x >= rect.x + rect.width as i32 - 80
                    && pos.x <= rect.x + rect.width as i32
                {
                    self.action_pressed.emit();
                    self.base.request_redraw();
                    return;
                }

                // If neither back nor action matched, treat as back when show_back,
                // otherwise as general click on the bar.
                if self.show_back {
                    self.back_pressed.emit();
                    self.base.request_redraw();
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn make_app_bar() -> AppBar {
        AppBar::new("Home", Rect::new(0, 0, 375, 56))
    }

    #[test]
    fn app_bar_default_creation() {
        let bar = make_app_bar();
        assert_eq!(bar.kind(), WidgetKind::AppBar);
        assert_eq!(bar.title(), "Home");
        assert!(!bar.show_back());
        assert_eq!(bar.action_text(), "");
        assert!(bar.is_visible());
        assert!(bar.is_enabled());
        assert_eq!(bar.geometry(), Rect::new(0, 0, 375, 56));
    }

    #[test]
    fn app_bar_title_accessors() {
        let mut bar = make_app_bar();
        assert_eq!(bar.title(), "Home");

        bar.set_title("Settings");
        assert_eq!(bar.title(), "Settings");

        bar.set_title("");
        assert_eq!(bar.title(), "");
    }

    #[test]
    fn app_bar_show_back_accessors() {
        let mut bar = make_app_bar();
        assert!(!bar.show_back());

        bar.set_show_back(true);
        assert!(bar.show_back());

        bar.set_show_back(false);
        assert!(!bar.show_back());
    }

    #[test]
    fn app_bar_action_text_accessors() {
        let mut bar = make_app_bar();
        assert_eq!(bar.action_text(), "");

        bar.set_action_text("Save");
        assert_eq!(bar.action_text(), "Save");

        bar.set_action_text("");
        assert_eq!(bar.action_text(), "");
    }

    #[test]
    fn app_bar_back_pressed_signal_emits_on_left_tap() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Tap in left zone (first 48px)
        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_back_pressed_emits_on_center_tap_when_back_shown() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Tap in center — falls through to back when show_back is true
        bar.handle_event(&Event::MousePress { pos: Point::new(188, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_action_pressed_signal_emits_on_right_tap() {
        let mut bar = make_app_bar();
        bar.set_action_text("Save");

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.action_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Tap in right action zone (last 80px)
        bar.handle_event(&Event::MousePress { pos: Point::new(340, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_action_pressed_not_emitted_on_center_tap() {
        let mut bar = make_app_bar();
        bar.set_action_text("Save");
        bar.set_show_back(true);

        let action_fired = Arc::new(AtomicBool::new(false));
        let a = action_fired.clone();
        bar.action_pressed.connect(move || {
            a.store(true, Ordering::SeqCst);
        });

        // Tap in center — action should NOT fire
        bar.handle_event(&Event::MousePress { pos: Point::new(188, 28), button: 1 });
        assert!(!action_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_disabled_blocks_events() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);
        bar.set_enabled(false);
        bar.set_action_text("Save");

        let back_fired = Arc::new(AtomicBool::new(false));
        let b = back_fired.clone();
        bar.back_pressed.connect(move || {
            b.store(true, Ordering::SeqCst);
        });

        let action_fired = Arc::new(AtomicBool::new(false));
        let a = action_fired.clone();
        bar.action_pressed.connect(move || {
            a.store(true, Ordering::SeqCst);
        });

        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 1 });
        assert!(!back_fired.load(Ordering::SeqCst));
        assert!(!action_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_svg_output() {
        let mut bar = make_app_bar();
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn app_bar_svg_with_back_and_action() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);
        bar.set_action_text("Cancel");
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn app_bar_back_pressed_signal_accessor() {
        let bar = make_app_bar();
        let signal = &bar.back_pressed;
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_action_pressed_signal_accessor() {
        let bar = make_app_bar();
        let signal = &bar.action_pressed;
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_other_button_noop() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Right-button click should be ignored
        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 2 });
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_release_also_emits() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        bar.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }
}
