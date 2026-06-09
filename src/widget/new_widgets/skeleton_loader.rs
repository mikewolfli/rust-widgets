//! SkeletonLoader widget — animated placeholder content while data loads.
//!
//! The SkeletonLoader renders a gray rounded-rect placeholder with a shimmer
//! pulse effect that oscillates the fill opacity between ~0.1 and ~0.3,
//! providing clear visual feedback that data is still loading.
//!
//! Supported shapes:
//! - `Rect(w, h)` — rectangular block
//! - `Circle(r)` — circular avatar/image placeholder
//! - `TextLine(w)` — three stacked text line placeholders

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Shape variants for the skeleton placeholder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkeletonShape {
    /// Rectangular placeholder (width, height).
    Rect(u32, u32),
    /// Circular placeholder (radius).
    Circle(u32),
    /// Single text line placeholder (width).
    TextLine(u32),
}

/// Timer ID used to drive the shimmer animation.
const SKELETON_ANIMATION_TIMER_ID: u32 = 0x534B;

/// SkeletonLoader widget — renders a shimmering placeholder shape while data loads.
pub struct SkeletonLoader {
    base: BaseWidget,
    shape: SkeletonShape,
    animated: bool,
    /// Counter incremented on each timer tick to drive opacity oscillation.
    animation_counter: u32,
}

impl SkeletonLoader {
    /// Creates a new SkeletonLoader with the given geometry.
    ///
    /// The default shape is a 200×20 `Rect` and the shimmer animation is enabled
    /// by default.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SkeletonLoader, geometry, "SkeletonLoader"),
            shape: SkeletonShape::Rect(200, 20),
            animated: true,
            animation_counter: 0,
        }
    }

    /// Sets the skeleton placeholder shape.
    pub fn set_shape(&mut self, shape: SkeletonShape) {
        self.shape = shape;
    }

    /// Returns the current skeleton placeholder shape.
    pub fn shape(&self) -> SkeletonShape {
        self.shape
    }

    /// Enables or disables the shimmer animation.
    ///
    /// When disabled the placeholder draws at a static mid-opacity of 0.2.
    pub fn set_animated(&mut self, animated: bool) {
        self.animated = animated;
    }

    /// Returns whether the shimmer animation is enabled.
    pub fn is_animated(&self) -> bool {
        self.animated
    }

    /// Computes the current shimmer opacity based on the animation counter.
    ///
    /// Uses a triangle wave over 20 frames to oscillate between 0.1 and 0.3:
    /// - Frames 0..9  → increasing opacity (0.1 → 0.3)
    /// - Frames 10..19 → decreasing opacity (0.3 → 0.1)
    /// - Disabled animation returns a static 0.2.
    fn current_opacity(&self) -> f32 {
        if !self.animated {
            return 0.2;
        }
        let phase = self.animation_counter % 20;
        let t = if phase < 10 { phase as f32 / 9.0 } else { (19 - phase) as f32 / 9.0 };
        0.1 + 0.2 * t
    }
}

impl Widget for SkeletonLoader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for SkeletonLoader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;

        let opacity = self.current_opacity();
        let base_color = Color::rgba(200, 200, 200, (opacity * 255.0).round() as u8);

        match self.shape {
            SkeletonShape::Rect(w, h) => {
                let shape_rect = Rect::new(cx - w as i32 / 2, cy - h as i32 / 2, w, h);
                context.fill_rounded_rect(shape_rect, 4, base_color);
            }
            SkeletonShape::Circle(r) => {
                let center = Point::new(cx, cy);
                context.fill_circle_aa(center, r, base_color);
            }
            SkeletonShape::TextLine(w) => {
                // Draw three stacked text-like lines to simulate paragraph text
                let line_h: u32 = 12;
                let gap: u32 = 6;
                let total_h = 3 * line_h + 2 * gap;
                let start_y = cy - total_h as i32 / 2;
                for i in 0..3 {
                    let y = start_y + i as i32 * (line_h + gap) as i32;
                    let line_rect = Rect::new(cx - w as i32 / 2, y, w, line_h);
                    context.fill_rounded_rect(line_rect, 3, base_color);
                }
            }
        }
    }
}

impl EventHandler for SkeletonLoader {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::Timer { id } if *id == SKELETON_ANIMATION_TIMER_ID => {
                if self.animated {
                    self.animation_counter = self.animation_counter.wrapping_add(1);
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

    #[test]
    fn skeleton_loader_default_creation() {
        let sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        assert_eq!(sl.kind(), WidgetKind::SkeletonLoader);
        assert_eq!(sl.geometry(), Rect::new(0, 0, 200, 100));
        assert_eq!(sl.shape(), SkeletonShape::Rect(200, 20));
        assert!(sl.is_animated());
    }

    #[test]
    fn skeleton_loader_rect_shape() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        sl.set_shape(SkeletonShape::Rect(100, 50));
        assert_eq!(sl.shape(), SkeletonShape::Rect(100, 50));
    }

    #[test]
    fn skeleton_loader_circle_shape() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        sl.set_shape(SkeletonShape::Circle(30));
        assert_eq!(sl.shape(), SkeletonShape::Circle(30));
    }

    #[test]
    fn skeleton_loader_text_line_shape() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        sl.set_shape(SkeletonShape::TextLine(150));
        assert_eq!(sl.shape(), SkeletonShape::TextLine(150));
    }

    #[test]
    fn skeleton_loader_animated_timer_creates_oscillation() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        assert!(sl.is_animated());
        assert_eq!(sl.animation_counter, 0);

        // Single timer tick advances the counter
        sl.handle_event(&Event::Timer { id: SKELETON_ANIMATION_TIMER_ID });
        assert_eq!(sl.animation_counter, 1);

        // Multiple timer ticks advance the counter
        for _ in 0..10 {
            sl.handle_event(&Event::Timer { id: SKELETON_ANIMATION_TIMER_ID });
        }
        assert_eq!(sl.animation_counter, 11);

        // Collect opacities over a full cycle to verify oscillation
        let opacities: Vec<f32> = (0..20)
            .map(|_| {
                sl.handle_event(&Event::Timer { id: SKELETON_ANIMATION_TIMER_ID });
                sl.current_opacity()
            })
            .collect();

        // Should have both the low (~0.1) and high (~0.3) ends of the range
        let min_op = opacities.iter().cloned().fold(f32::MAX, f32::min);
        let max_op = opacities.iter().cloned().fold(f32::MIN, f32::max);
        assert!(min_op < 0.15, "minimum opacity should be near 0.1, got {}", min_op);
        assert!(max_op > 0.25, "maximum opacity should be near 0.3, got {}", max_op);

        // Disabling animation freezes opacity at mid-point (0.2)
        sl.set_animated(false);
        assert!(!sl.is_animated());
        let static_op = sl.current_opacity();
        assert!((static_op - 0.2).abs() < 0.01, "static opacity should be 0.2, got {}", static_op);
    }

    #[test]
    fn skeleton_loader_svg_output() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        let svg = render_to_svg(&mut sl);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"100\""));
    }

    #[test]
    fn skeleton_loader_set_animated_flag() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        assert!(sl.is_animated());
        sl.set_animated(false);
        assert!(!sl.is_animated());
        sl.set_animated(true);
        assert!(sl.is_animated());
    }
}
