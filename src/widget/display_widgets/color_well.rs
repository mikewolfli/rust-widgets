//! ColorWell widget — a compact color swatch that displays the current color
//! and emits a click signal when pressed.
//!
//! The ColorWell shows a filled rectangle with the selected color. When the
//! color has alpha transparency, a checkerboard pattern is rendered behind it
//! to indicate the transparent regions. An optional border frames the swatch.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A compact color swatch widget that displays a color and emits a signal
/// when clicked.
pub struct ColorWell {
    base: BaseWidget,
    color: Color,
    show_border: bool,
    /// Emitted when the color well is clicked.
    pub clicked: GenericSignal,
}

impl ColorWell {
    /// Creates a new ColorWell widget with the given color and geometry.
    ///
    /// The border is enabled by default.
    pub fn new(color: Color, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorWell, geometry, "ColorWell"),
            color,
            show_border: true,
            clicked: GenericSignal::new(),
        }
    }

    /// Returns the current color.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Sets the current color and requests a redraw.
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.base.request_redraw();
    }

    /// Returns whether the border is visible.
    pub fn show_border(&self) -> bool {
        self.show_border
    }

    /// Sets whether the border is visible and requests a redraw.
    pub fn set_show_border(&mut self, show_border: bool) {
        if self.show_border != show_border {
            self.show_border = show_border;
            self.base.request_redraw();
        }
    }
}

impl Widget for ColorWell {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for ColorWell {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Draw checkerboard for transparency indication
        let checker_size = 4u32;
        let even = Color::rgba(200, 200, 200, 255);
        let odd = Color::rgba(255, 255, 255, 255);

        for y in (rect.y..(rect.y + rect.height as i32)).step_by(checker_size as usize) {
            for x in (rect.x..(rect.x + rect.width as i32)).step_by(checker_size as usize) {
                let tile_x = (x - rect.x) / checker_size as i32;
                let tile_y = (y - rect.y) / checker_size as i32;
                let tile_color = if (tile_x + tile_y) % 2 == 0 { even } else { odd };
                let tile_w = checker_size.min((rect.x + rect.width as i32 - x) as u32);
                let tile_h = checker_size.min((rect.y + rect.height as i32 - y) as u32);
                context.fill_rect(Rect::new(x, y, tile_w, tile_h), tile_color);
            }
        }

        // Draw the actual color (with alpha blending onto the checkerboard)
        context.fill_rect(rect, self.color);

        // Draw border if enabled
        if self.show_border {
            context.draw_rect_stroke(rect, Color::rgba(0, 0, 0, 80), 1);
        }
    }
}

impl EventHandler for ColorWell {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } | Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.clicked.emit();
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn color_well_default_color() {
        let cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        assert_eq!(cw.color(), Color::RED);
        assert!(cw.show_border());
        assert_eq!(cw.kind(), WidgetKind::ColorWell);
    }

    #[test]
    fn color_well_set_color() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        cw.set_color(Color::BLUE);
        assert_eq!(cw.color(), Color::BLUE);
    }

    #[test]
    fn color_well_signal_emitted_on_click() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let clicked = Arc::new(AtomicBool::new(false));
        cw.clicked.connect({
            let clicked = Arc::clone(&clicked);
            move || {
                clicked.store(true, Ordering::SeqCst);
            }
        });

        cw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn color_well_border_toggle() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        assert!(cw.show_border());

        cw.set_show_border(false);
        assert!(!cw.show_border());

        cw.set_show_border(true);
        assert!(cw.show_border());
    }

    #[test]
    fn color_well_disabled_blocks_events() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let clicked = Arc::new(AtomicBool::new(false));
        cw.clicked.connect({
            let clicked = Arc::clone(&clicked);
            move || {
                clicked.store(true, Ordering::SeqCst);
            }
        });

        cw.set_enabled(false);
        cw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn color_well_svg_output() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let svg = crate::widget::svg::render_to_svg(&mut cw);
        assert!(svg.starts_with("<svg"));
    }
}
