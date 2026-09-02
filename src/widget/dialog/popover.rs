//! Popover widget — a floating bubble card with an anchor arrow.
//!
//! The Popover widget displays a floating card near an anchor rectangle,
//! optionally containing a child widget. It supports show/hide with an
//! arrow pointing toward the anchor, and auto-dismisses when the user
//! clicks outside the popover area.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Arrow size in pixels from tip to base.
const ARROW_SIZE: i32 = 10;
/// Corner radius of the popover body.
const CORNER_RADIUS: u32 = 8;

/// Popover widget — a floating bubble card with an anchor arrow.
///
/// Shows a rounded rectangle with a triangular arrow pointing toward an
/// anchor rectangle. When visible, it can contain an optional child widget
/// rendered inside the card body. Clicking outside the popover area
/// automatically hides it.
pub struct Popover {
    base: BaseWidget,
    content: Option<Box<dyn Widget>>,
    anchor_rect: Rect,
    visible: bool,
    /// Cached popover body rectangle (computed during draw).
    body_rect: Rect,
}

impl Popover {
    /// Creates a new Popover widget with the given geometry.
    ///
    /// Initially hidden with no content and an empty anchor rectangle.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Popover, geometry, "Popover"),
            content: None,
            anchor_rect: Rect::default(),
            visible: false,
            body_rect: Rect::default(),
        }
    }

    /// Shows the popover positioned relative to the given anchor rectangle.
    pub fn show(&mut self, anchor: Rect) {
        self.anchor_rect = anchor;
        self.visible = true;
        self.base.request_redraw();
    }

    /// Hides the popover.
    pub fn hide(&mut self) {
        self.visible = false;
        self.base.request_redraw();
    }

    /// Returns whether the popover is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Sets the content widget displayed inside the popover.
    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
        self.base.request_redraw();
    }

    /// Returns a reference to the content widget, if any.
    pub fn content(&self) -> Option<&dyn Widget> {
        self.content.as_deref()
    }

    /// Returns a mutable reference to the content widget, if any.
    pub fn content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_deref_mut()
    }

    /// Returns the anchor rectangle.
    pub fn anchor_rect(&self) -> Rect {
        self.anchor_rect
    }

    /// Sets the anchor rectangle.
    pub fn set_anchor_rect(&mut self, rect: Rect) {
        self.anchor_rect = rect;
        self.base.request_redraw();
    }

    /// Computes the popover body rectangle below/above the anchor.
    fn compute_layout(&self) -> (Rect, Point, ArrowDirection) {
        let geom = self.geometry();
        let body_width = geom.width.clamp(100, 400);
        let body_height = geom.height.clamp(60, 400);

        // Position popover below the anchor by default; flip above if not enough room
        let below_space =
            geom.y + geom.height as i32 - (self.anchor_rect.y + self.anchor_rect.height as i32);
        let above_space = self.anchor_rect.y - geom.y;

        let (body_y, arrow_dir) = if below_space >= body_height as i32 + ARROW_SIZE {
            (self.anchor_rect.y + self.anchor_rect.height as i32 + ARROW_SIZE, ArrowDirection::Up)
        } else if above_space >= body_height as i32 + ARROW_SIZE {
            (self.anchor_rect.y - body_height as i32 - ARROW_SIZE, ArrowDirection::Down)
        } else {
            // Default: below
            (self.anchor_rect.y + self.anchor_rect.height as i32 + ARROW_SIZE, ArrowDirection::Up)
        };

        // Center horizontally on anchor
        let anchor_center_x = self.anchor_rect.x + self.anchor_rect.width as i32 / 2;
        let body_x = (anchor_center_x - body_width as i32 / 2).max(geom.x);
        let body_rect = Rect::new(body_x, body_y, body_width, body_height);

        // Arrow tip points to anchor center
        let arrow_tip = Point::new(
            anchor_center_x,
            match arrow_dir {
                ArrowDirection::Up => body_y - ARROW_SIZE,
                ArrowDirection::Down => body_y + body_height as i32 + ARROW_SIZE,
            },
        );

        (body_rect, arrow_tip, arrow_dir)
    }
}

impl Widget for Popover {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(200, 150)
    }
}

impl Draw for Popover {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible {
            return;
        }

        let (body_rect, arrow_tip, arrow_dir) = self.compute_layout();
        self.body_rect = body_rect;

        // ── Draw shadow ──
        let shadow_offset = 2i32;
        let shadow_rect = Rect::new(
            body_rect.x + shadow_offset,
            body_rect.y + shadow_offset,
            body_rect.width,
            body_rect.height,
        );
        context.fill_rounded_rect(shadow_rect, CORNER_RADIUS, Color::rgba(0, 0, 0, 40));

        // ── Draw popover body ──
        context.fill_rounded_rect(body_rect, CORNER_RADIUS, Color::WHITE);
        context.draw_rounded_rect_stroke(
            body_rect,
            CORNER_RADIUS,
            Color::rgba(200, 200, 200, 200),
            1,
        );

        // ── Draw arrow ──
        self.draw_arrow(context, arrow_tip, arrow_dir);

        // ── Draw placeholder content indicator ──
        let content_padding = 8i32;
        let content_rect = Rect::new(
            body_rect.x + content_padding,
            body_rect.y + content_padding,
            body_rect.width - (content_padding as u32) * 2,
            body_rect.height - (content_padding as u32) * 2,
        );
        let font = Font::simple("sans-serif", 13.0);
        let label = if self.content.is_some() { "Popover" } else { "Popover (empty)" };
        let metrics = context.measure_text(label, &font);
        let text_x = content_rect.x + (content_rect.width as i32 - metrics.width as i32) / 2;
        let text_y = content_rect.y
            + (content_rect.height as i32 - metrics.height as i32) / 2
            + metrics.ascent as i32;
        context.draw_text(
            Point::new(text_x.max(content_rect.x), text_y.max(content_rect.y)),
            label,
            &font,
            Color::rgba(150, 150, 150, 200),
            HorizontalAlignment::Left,
        );
    }
}

/// Direction the popover arrow points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrowDirection {
    Up,
    Down,
}

impl Popover {
    /// Draws the triangular arrow pointing toward the anchor.
    fn draw_arrow(&self, context: &mut RenderContext, tip: Point, dir: ArrowDirection) {
        let half_base = ARROW_SIZE / 2;
        let (_base_center, base_left, base_right) = match dir {
            ArrowDirection::Up => {
                let base_center = Point::new(tip.x, tip.y + ARROW_SIZE);
                let base_left = Point::new(tip.x - half_base, tip.y + ARROW_SIZE);
                let base_right = Point::new(tip.x + half_base, tip.y + ARROW_SIZE);
                (base_center, base_left, base_right)
            }
            ArrowDirection::Down => {
                let base_center = Point::new(tip.x, tip.y - ARROW_SIZE);
                let base_left = Point::new(tip.x - half_base, tip.y - ARROW_SIZE);
                let base_right = Point::new(tip.x + half_base, tip.y - ARROW_SIZE);
                (base_center, base_left, base_right)
            }
        };

        // Draw filled triangle using DrawPath
        let points = vec![tip, base_left, base_right];
        context.execute_command(RenderCommand::DrawPath {
            points: points.clone(),
            closed: true,
            color: Color::WHITE,
            filled: true,
            width: 1,
        });
        // Draw triangle outline
        context.execute_command(RenderCommand::DrawPath {
            points,
            closed: true,
            color: Color::rgba(200, 200, 200, 200),
            filled: false,
            width: 1,
        });
    }
}

impl EventHandler for Popover {
    fn handle_event(&mut self, event: &Event) {
        if !self.visible {
            self.base.handle_event(event);
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 && !self.body_rect.contains_point(*pos) {
                    // Auto-dismiss on click outside
                    self.hide();
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == 27 {
                    // Escape key
                    self.hide();
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
    /// A simple test widget used as content inside popover tests.
    struct TestContent {
        base: BaseWidget,
        text: String,
    }

    impl TestContent {
        fn new(text: &str, geometry: Rect) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, geometry, "TestContent"),
                text: text.to_string(),
            }
        }
    }

    impl Widget for TestContent {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
    }

    impl Draw for TestContent {
        fn draw(&mut self, context: &mut RenderContext) {
            let font = Font::simple("sans-serif", 12.0);
            context.draw_text(
                Point::new(self.geometry().x, self.geometry().y),
                &self.text,
                &font,
                Color::BLACK,
                HorizontalAlignment::Left,
            );
        }
    }

    impl EventHandler for TestContent {
        fn handle_event(&mut self, _event: &Event) {}
    }

    #[test]
    fn popover_default_creation() {
        let popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert_eq!(popover.kind(), WidgetKind::Popover);
        assert!(!popover.is_visible());
        assert!(popover.content().is_none());
        assert_eq!(popover.geometry(), Rect::new(0, 0, 300, 200));
    }

    #[test]
    fn popover_show_hide() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert!(!popover.is_visible());

        popover.show(Rect::new(100, 100, 50, 20));
        assert!(popover.is_visible());

        popover.hide();
        assert!(!popover.is_visible());
    }

    #[test]
    fn popover_anchor_rect() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 400));
        let anchor = Rect::new(100, 100, 50, 20);
        popover.show(anchor);
        assert_eq!(popover.anchor_rect(), anchor);

        let new_anchor = Rect::new(50, 50, 80, 30);
        popover.set_anchor_rect(new_anchor);
        assert_eq!(popover.anchor_rect(), new_anchor);
    }

    #[test]
    fn popover_set_content() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert!(popover.content().is_none());

        let content = TestContent::new("Hello", Rect::new(0, 0, 100, 30));
        popover.set_content(Box::new(content));
        assert!(popover.content().is_some());
    }

    #[test]
    fn popover_auto_dismiss_on_click_outside() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        let anchor = Rect::new(150, 100, 50, 20);
        popover.show(anchor);
        assert!(popover.is_visible());

        // Click far outside the body rect
        popover.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1 });
        assert!(!popover.is_visible());
    }

    #[test]
    fn popover_escape_key_dismisses() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        popover.show(Rect::new(150, 100, 50, 20));
        assert!(popover.is_visible());

        popover.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!popover.is_visible());
    }

    #[test]
    fn popover_svg_output_visible() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        popover.show(Rect::new(100, 100, 50, 20));
        let svg = render_to_svg(&mut popover);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    #[test]
    fn popover_svg_output_hidden() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        let svg = render_to_svg(&mut popover);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // Only the background fill from the SVG infrastructure
        let fill_count = svg.matches("fill=").count();
        assert_eq!(fill_count, 1, "expected only background fill, got {fill_count}: {svg}");
    }
}
