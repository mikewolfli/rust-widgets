//! SafeArea widget — insets content to avoid notches, status bars, home indicators (BLUE11 R10.14).
use crate::core::{Color, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Safe area insets for mobile devices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafeAreaInsets {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl Default for SafeAreaInsets {
    fn default() -> Self {
        Self { top: 44, bottom: 34, left: 0, right: 0 }
    }
}

/// SafeArea widget — wraps content with safe area insets (BLUE11 R10.14).
pub struct SafeArea {
    base: BaseWidget,
    insets: SafeAreaInsets,
    /// Background color for the safe area margins.
    margin_color: Color,
}

impl SafeArea {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SafeArea, geometry, "SafeArea"),
            insets: SafeAreaInsets::default(),
            margin_color: Color::WHITE,
        }
    }
    pub fn set_insets(&mut self, insets: SafeAreaInsets) {
        self.insets = insets;
        self.base.request_redraw();
    }
    pub fn insets(&self) -> SafeAreaInsets {
        self.insets
    }
    pub fn set_margin_color(&mut self, color: Color) {
        self.margin_color = color;
    }
    pub fn content_rect(&self) -> Rect {
        let g = self.geometry();
        Rect::new(
            g.x + self.insets.left as i32,
            g.y + self.insets.top as i32,
            g.width.saturating_sub(self.insets.left + self.insets.right),
            g.height.saturating_sub(self.insets.top + self.insets.bottom),
        )
    }
}

impl Widget for SafeArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }
}

impl Draw for SafeArea {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let g = self.geometry();
        let cr = self.content_rect();
        // Fill margin areas (top/bottom/left/right bars)
        if self.insets.top > 0 {
            ctx.fill_rect(Rect::new(g.x, g.y, g.width, self.insets.top), self.margin_color);
        }
        if self.insets.bottom > 0 {
            ctx.fill_rect(
                Rect::new(
                    g.x,
                    g.y + g.height as i32 - self.insets.bottom as i32,
                    g.width,
                    self.insets.bottom,
                ),
                self.margin_color,
            );
        }
        if self.insets.left > 0 {
            let left_bar_height = g.height.saturating_sub(self.insets.top + self.insets.bottom);
            ctx.fill_rect(
                Rect::new(g.x, g.y + self.insets.top as i32, self.insets.left, left_bar_height),
                self.margin_color,
            );
        }
        if self.insets.right > 0 {
            let right_bar_height = g.height.saturating_sub(self.insets.top + self.insets.bottom);
            ctx.fill_rect(
                Rect::new(
                    g.x + g.width as i32 - self.insets.right as i32,
                    g.y + self.insets.top as i32,
                    self.insets.right,
                    right_bar_height,
                ),
                self.margin_color,
            );
        }
        // Draw content area border (subtle)
        ctx.draw_rect(cr, Color::rgba(200, 200, 200, 100));
    }
}

impl EventHandler for SafeArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn safe_area_default_insets() {
        let sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let insets = sa.insets();
        assert_eq!(insets.top, 44);
        assert_eq!(insets.bottom, 34);
        assert_eq!(insets.left, 0);
        assert_eq!(insets.right, 0);
    }

    #[test]
    fn safe_area_content_rect_computes_correctly() {
        let sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let cr = sa.content_rect();
        assert_eq!(cr.x, 0);
        assert_eq!(cr.y, 44);
        assert_eq!(cr.width, 375);
        assert_eq!(cr.height, 812 - 44 - 34);
    }

    #[test]
    fn safe_area_content_rect_with_all_insets() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        sa.set_insets(SafeAreaInsets { top: 50, bottom: 40, left: 16, right: 16 });
        let cr = sa.content_rect();
        assert_eq!(cr.x, 16);
        assert_eq!(cr.y, 50);
        assert_eq!(cr.width, 375 - 32);
        assert_eq!(cr.height, 812 - 90);
    }

    #[test]
    fn safe_area_set_insets_updates_content_rect() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 400, 800));
        let custom = SafeAreaInsets { top: 60, bottom: 50, left: 10, right: 10 };
        sa.set_insets(custom);
        assert_eq!(sa.insets(), custom);
        let cr = sa.content_rect();
        assert_eq!(cr.x, 10);
        assert_eq!(cr.y, 60);
    }

    #[test]
    fn safe_area_set_margin_color() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        sa.set_margin_color(Color::BLACK);
        // No crash; color is used in draw.
        assert_eq!(sa.kind(), WidgetKind::SafeArea);
    }

    #[test]
    fn safe_area_event_delegation() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        // Events should not panic/crash.
        sa.handle_event(&Event::MousePress { pos: Point::new(10, 5), button: 1 });
        sa.handle_event(&Event::MouseRelease { pos: Point::new(10, 5), button: 1 });
        assert_eq!(sa.insets().top, 44);
    }

    #[test]
    fn safe_area_svg_output() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let svg = crate::widget::svg::render_to_svg(&mut sa);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn safe_area_content_rect_saturating_overflow() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 100, 100));
        let huge = SafeAreaInsets { top: 200, bottom: 200, left: 0, right: 0 };
        sa.set_insets(huge);
        // This should not panic; height saturates to 0.
        let cr = sa.content_rect();
        assert_eq!(cr.width, 100); // left/right are 0, so width unchanged
        assert_eq!(cr.height, 0);
    }
}
