//! CupertinoNavigationBar — iOS-style large title navigation bar.
//!
//! An iOS-style navigation bar with optional large title (similar to the
//! iOS 13+ large title nav bar), back button with arrow, and translucent
//! background effect.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// iOS-style large title navigation bar.
///
/// Renders a translucent background with an optional large title and a
/// back button. Emits `back_pressed` when the back button is clicked.
pub struct CupertinoNavigationBar {
    base: BaseWidget,
    title: String,
    large_title: bool,
    back_button_visible: bool,
    back_button_text: String,
    /// Emitted when the back button is pressed.
    pub back_pressed: Signal1<()>,
}

impl CupertinoNavigationBar {
    /// Creates a new CupertinoNavigationBar with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let base =
            BaseWidget::new(WidgetKind::CupertinoNavigationBar, geometry, "CupertinoNavigationBar");
        Self {
            base,
            title: String::new(),
            large_title: true,
            back_button_visible: false,
            back_button_text: "Back".to_string(),
            back_pressed: Signal1::new(),
        }
    }

    /// Sets whether the back button is visible.
    pub fn show_back_button(&mut self, visible: bool) {
        self.back_button_visible = visible;
        self.base.request_redraw();
    }

    /// Sets the title text.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns whether large title mode is enabled.
    pub fn is_large_title(&self) -> bool {
        self.large_title
    }

    /// Enables or disables large title mode.
    pub fn set_large_title(&mut self, enabled: bool) {
        self.large_title = enabled;
        self.base.request_redraw();
    }

    /// Returns whether the back button is visible.
    pub fn is_back_button_visible(&self) -> bool {
        self.back_button_visible
    }

    /// Sets the text for the back button.
    pub fn set_back_button_text(&mut self, text: &str) {
        self.back_button_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the back button text.
    pub fn back_button_text(&self) -> &str {
        &self.back_button_text
    }
}

impl Widget for CupertinoNavigationBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::CupertinoNavigationBar
    }
}

impl Draw for CupertinoNavigationBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // ── Translucent background (frosted glass effect via semi-transparent white) ──
        context.fill_rect(rect, Color::rgba(255, 255, 255, 230));

        // ── Bottom border line ──
        let border_y = rect.y + rect.height as i32 - 1;
        context.draw_line(
            Point::new(rect.x, border_y),
            Point::new(rect.x + rect.width as i32, border_y),
            Color::rgba(200, 200, 200, 200),
        );

        if self.large_title {
            // ── Large title ──
            let title_font = Font::new("sans-serif", 34.0, true, false);
            if !self.title.is_empty() {
                let metrics = context.measure_text(&self.title, &title_font);
                let title_x = rect.x + 16;
                let title_y = rect.y + (rect.height as i32 / 2) + (metrics.ascent as i32 / 2);
                context.draw_text(
                    Point::new(title_x, title_y),
                    &self.title,
                    &title_font,
                    Color::BLACK,
                );
            }
        } else {
            // ── Compact title (centered in navigation bar area) ──
            let title_font = Font::new("sans-serif", 18.0, false, false);
            if !self.title.is_empty() {
                let metrics = context.measure_text(&self.title, &title_font);
                let title_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
                let title_y =
                    rect.y + 22 + (metrics.ascent as i32 / 2) - (metrics.descent as i32 / 2);
                context.draw_text(
                    Point::new(title_x, title_y),
                    &self.title,
                    &title_font,
                    Color::BLACK,
                );
            }
        }

        // ── Back button (left side) ──
        if self.back_button_visible {
            let arrow_font = Font::new("sans-serif", 20.0, false, false);
            let label_font = Font::new("sans-serif", 17.0, false, false);
            let arrow_symbol = "\u{2190}"; // ←

            let arrow_metrics = context.measure_text(arrow_symbol, &arrow_font);
            let arrow_x = rect.x + 8;
            let arrow_y = rect.y + 22 + (arrow_metrics.ascent as i32 / 2);

            // Draw arrow
            context.draw_text(
                Point::new(arrow_x, arrow_y),
                arrow_symbol,
                &arrow_font,
                Color::rgba(0, 122, 255, 255), // iOS blue
            );

            // Draw text label next to arrow
            if !self.back_button_text.is_empty() {
                let label_metrics = context.measure_text(&self.back_button_text, &label_font);
                let label_x = arrow_x + arrow_metrics.width as i32 + 4;
                let label_y = rect.y + 22 + (label_metrics.ascent as i32 / 2);
                context.draw_text(
                    Point::new(label_x, label_y),
                    &self.back_button_text,
                    &label_font,
                    Color::rgba(0, 122, 255, 255),
                );
            }
        }
    }
}

impl EventHandler for CupertinoNavigationBar {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                // Only handle back button area clicks
                if !self.back_button_visible {
                    return;
                }

                let rect = self.geometry();
                // Back button area: left ~80px of the nav bar, top 44px
                let back_area = Rect::new(rect.x, rect.y, 80, 44);
                if back_area.contains_point(*pos) {
                    self.back_pressed.emit(());
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

    #[test]
    fn cupertino_nav_bar_creation() {
        let bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert_eq!(bar.kind(), WidgetKind::CupertinoNavigationBar);
        assert!(bar.title().is_empty());
        assert!(bar.is_large_title());
        assert!(!bar.is_back_button_visible());
    }

    #[test]
    fn cupertino_nav_bar_title_accessors() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        bar.set_title("Home");
        assert_eq!(bar.title(), "Home");
    }

    #[test]
    fn cupertino_nav_bar_back_button_visibility() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert!(!bar.is_back_button_visible());

        bar.show_back_button(true);
        assert!(bar.is_back_button_visible());

        bar.show_back_button(false);
        assert!(!bar.is_back_button_visible());
    }

    #[test]
    fn cupertino_nav_bar_large_title_toggle() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert!(bar.is_large_title());

        bar.set_large_title(false);
        assert!(!bar.is_large_title());

        bar.set_large_title(true);
        assert!(bar.is_large_title());
    }

    #[test]
    fn cupertino_nav_bar_back_button_text() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert_eq!(bar.back_button_text(), "Back");

        bar.set_back_button_text("Settings");
        assert_eq!(bar.back_button_text(), "Settings");
    }

    #[test]
    fn cupertino_nav_bar_back_pressed_signal() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        bar.show_back_button(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move |_: std::sync::Arc<()>| {
            f.store(true, Ordering::SeqCst);
        });

        // Click on back button area
        bar.handle_event(&Event::MouseRelease { pos: Point::new(20, 22), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn cupertino_nav_bar_back_pressed_not_fired_when_hidden() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        // back_button_visible is false by default

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move |_: std::sync::Arc<()>| {
            f.store(true, Ordering::SeqCst);
        });

        // Click where back button would be
        bar.handle_event(&Event::MouseRelease { pos: Point::new(20, 22), button: 1 });
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn cupertino_nav_bar_svg_output() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        bar.set_title("Settings");
        bar.show_back_button(true);
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
    }
}
