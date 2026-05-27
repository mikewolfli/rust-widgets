//! Frame widget.
use crate::core::{Color, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;

use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// Frame widget.
pub struct Frame {
    base: BaseWidget,
    frame_shape: FrameShape,
    frame_shadow: FrameShadow,
    line_width: f32,
    mid_line_width: f32,
    widget: Option<ObjectId>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
/// Frame shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameShape {
    /// No frame
    NoFrame,
    /// Box frame
    #[default]
    Box,
    /// Panel frame
    Panel,
    /// Styled panel frame
    StyledPanel,
    /// HLine frame
    HLine,
    /// VLine frame
    VLine,
    /// WinPanel frame
    WinPanel,
}
/// Frame shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameShadow {
    /// Plain shadow
    #[default]
    Plain,
    /// Raised shadow
    Raised,
    /// Sunken shadow
    Sunken,
}
impl Frame {
    /// Creates a frame.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "Frame"),
            frame_shape: FrameShape::Box,
            frame_shadow: FrameShadow::Plain,
            line_width: 1.0,
            mid_line_width: 0.0,
            widget: None,
            registry: None,
        }
    }
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Returns frame shape.
    pub fn frame_shape(&self) -> FrameShape {
        self.frame_shape
    }
    /// Sets frame shape.
    pub fn set_frame_shape(&mut self, shape: FrameShape) {
        self.frame_shape = shape;
    }
    /// Returns frame shadow.
    pub fn frame_shadow(&self) -> FrameShadow {
        self.frame_shadow
    }
    /// Sets frame shadow.
    pub fn set_frame_shadow(&mut self, shadow: FrameShadow) {
        self.frame_shadow = shadow;
    }
    /// Returns line width.
    pub fn line_width(&self) -> f32 {
        self.line_width
    }
    /// Sets line width.
    pub fn set_line_width(&mut self, width: f32) {
        self.line_width = width.max(0.0);
    }
    /// Returns mid line width.
    pub fn mid_line_width(&self) -> f32 {
        self.mid_line_width
    }
    /// Sets mid line width.
    pub fn set_mid_line_width(&mut self, width: f32) {
        self.mid_line_width = width.max(0.0);
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Draws frame border.
    fn draw_frame(&self, context: &mut RenderContext) {
        let rect = self.geometry();
        match self.frame_shape {
            FrameShape::NoFrame => {}
            FrameShape::Box => self.draw_box_frame(context, rect),
            FrameShape::Panel => self.draw_panel_frame(context, rect),
            FrameShape::StyledPanel => self.draw_styled_panel_frame(context, rect),
            FrameShape::HLine => self.draw_hline_frame(context, rect),
            FrameShape::VLine => self.draw_vline_frame(context, rect),
            FrameShape::WinPanel => self.draw_win_panel_frame(context, rect),
        }
    }
    /// Draws box frame.
    fn draw_box_frame(&self, context: &mut RenderContext, rect: Rect) {
        let line_width = self.line_width;
        let mid_line_width = self.mid_line_width;
        match self.frame_shadow {
            FrameShadow::Plain => {
                // Draw single border
                context.draw_rect(rect, Color::from_rgb(0, 0, 0));
            }
            FrameShadow::Raised => {
                // Draw raised border
                let light_color = Color::from_rgb(255, 255, 255);
                let dark_color = Color::from_rgb(128, 128, 128);
                // Top and left (light)
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    light_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
                    light_color,
                );
                // Bottom and right (dark)
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32,
                        rect.y as f32 + rect.height as f32,
                    ),
                    dark_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32,
                        rect.y as f32 + rect.height as f32,
                    ),
                    dark_color,
                );
                // Draw mid line if needed
                if mid_line_width > 0.0 {
                    let mid_light = Color::from_rgb(192, 192, 192);
                    let mid_dark = Color::from_rgb(64, 64, 64);
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + line_width,
                        ),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(
                            rect.x as f32 + line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(
                            rect.x as f32 + line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        mid_dark,
                    );
                    context.draw_line(
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + line_width,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        mid_dark,
                    );
                }
            }
            FrameShadow::Sunken => {
                // Draw sunken border
                let light_color = Color::from_rgb(128, 128, 128);
                let dark_color = Color::from_rgb(255, 255, 255);
                // Top and left (dark)
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    light_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
                    light_color,
                );
                // Bottom and right (light)
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32,
                        rect.y as f32 + rect.height as f32,
                    ),
                    dark_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32,
                        rect.y as f32 + rect.height as f32,
                    ),
                    dark_color,
                );
                // Draw mid line if needed
                if mid_line_width > 0.0 {
                    let mid_light = Color::from_rgb(64, 64, 64);
                    let mid_dark = Color::from_rgb(192, 192, 192);
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + line_width,
                        ),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(
                            rect.x as f32 + line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(
                            rect.x as f32 + line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        mid_dark,
                    );
                    context.draw_line(
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + line_width,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - line_width,
                            rect.y as f32 + rect.height as f32 - line_width,
                        ),
                        mid_dark,
                    );
                }
            }
        }
    }
    /// Draws panel frame.
    fn draw_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        // Similar to box but with different colors
        self.draw_box_frame(context, rect);
    }
    /// Draws styled panel frame.
    fn draw_styled_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        // More sophisticated panel with gradient
        let bg_color = Color::from_rgb(240, 240, 240);
        context.fill_rect(rect, bg_color);
        self.draw_box_frame(context, rect);
    }
    /// Draws horizontal line frame.
    fn draw_hline_frame(&self, context: &mut RenderContext, rect: Rect) {
        let y = rect.y + (rect.height as i32) / 2;
        context.draw_line(
            Point::new(rect.x, y),
            Point::new(rect.x + rect.width as i32, y),
            Color::from_rgb(0, 0, 0),
        );
    }
    /// Draws vertical line frame.
    fn draw_vline_frame(&self, context: &mut RenderContext, rect: Rect) {
        let x = rect.x + (rect.width as i32) / 2;
        context.draw_line(
            Point::new(x, rect.y),
            Point::new(x, rect.y + rect.height as i32),
            Color::from_rgb(0, 0, 0),
        );
    }
    /// Draws Windows panel frame.
    fn draw_win_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        // Windows-style panel
        let bg_color = Color::from_rgb(240, 240, 240);
        context.fill_rect(rect, bg_color);
        // Draw 3D border
        let light_color = Color::from_rgb(255, 255, 255);
        let dark_color = Color::from_rgb(128, 128, 128);
        // Outer border (sunken)
        context.draw_line(
            Point::from_f32(rect.x as f32, rect.y as f32),
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
            dark_color,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32, rect.y as f32),
            Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
            dark_color,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
            Point::from_f32(
                rect.x as f32 + rect.width as f32,
                rect.y as f32 + rect.height as f32,
            ),
            light_color,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
            Point::from_f32(
                rect.x as f32 + rect.width as f32,
                rect.y as f32 + rect.height as f32,
            ),
            light_color,
        );
        // Inner border (raised)
        let inner_rect = Rect::new(
            rect.x + 1,
            rect.y + 1,
            rect.width.saturating_sub(2),
            rect.height.saturating_sub(2),
        );
        context.draw_line(
            Point::new(inner_rect.x, inner_rect.y),
            Point::new(inner_rect.x + inner_rect.width as i32, inner_rect.y),
            light_color,
        );
        context.draw_line(
            Point::new(inner_rect.x, inner_rect.y),
            Point::new(inner_rect.x, inner_rect.y + inner_rect.height as i32),
            light_color,
        );
        context.draw_line(
            Point::new(inner_rect.x, inner_rect.y + inner_rect.height as i32),
            Point::new(
                inner_rect.x + inner_rect.width as i32,
                inner_rect.y + inner_rect.height as i32,
            ),
            dark_color,
        );
        context.draw_line(
            Point::new(inner_rect.x + inner_rect.width as i32, inner_rect.y),
            Point::new(
                inner_rect.x + inner_rect.width as i32,
                inner_rect.y + inner_rect.height as i32,
            ),
            dark_color,
        );
    }
}
// Implement Widget trait
impl Widget for Frame {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for Frame {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Forward events to widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for Frame {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw frame
        self.draw_frame(context);
        // Draw widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, ObjectId, Rect};
    use crate::style::WidgetStyle;

    #[test]
    fn frame_creation_defaults() {
        let f = Frame::new(Rect::new(0, 0, 200, 100));
        assert_eq!(f.frame_shape(), FrameShape::Box);
        assert_eq!(f.frame_shadow(), FrameShadow::Plain);
        assert!((f.line_width() - 1.0).abs() < f32::EPSILON);
        assert!((f.mid_line_width() - 0.0).abs() < f32::EPSILON);
        assert!(f.widget().is_none());
        assert!(f.registry().is_none());
    }

    #[test]
    fn frame_shape_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_frame_shape(FrameShape::Panel);
        assert_eq!(f.frame_shape(), FrameShape::Panel);
        f.set_frame_shape(FrameShape::NoFrame);
        assert_eq!(f.frame_shape(), FrameShape::NoFrame);
        f.set_frame_shape(FrameShape::StyledPanel);
        assert_eq!(f.frame_shape(), FrameShape::StyledPanel);
        f.set_frame_shape(FrameShape::HLine);
        assert_eq!(f.frame_shape(), FrameShape::HLine);
        f.set_frame_shape(FrameShape::VLine);
        assert_eq!(f.frame_shape(), FrameShape::VLine);
        f.set_frame_shape(FrameShape::WinPanel);
        assert_eq!(f.frame_shape(), FrameShape::WinPanel);
        f.set_frame_shape(FrameShape::Box);
        assert_eq!(f.frame_shape(), FrameShape::Box);
    }

    #[test]
    fn frame_shadow_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_frame_shadow(FrameShadow::Raised);
        assert_eq!(f.frame_shadow(), FrameShadow::Raised);
        f.set_frame_shadow(FrameShadow::Sunken);
        assert_eq!(f.frame_shadow(), FrameShadow::Sunken);
        f.set_frame_shadow(FrameShadow::Plain);
        assert_eq!(f.frame_shadow(), FrameShadow::Plain);
    }

    #[test]
    fn frame_line_width_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_line_width(3.0);
        assert!((f.line_width() - 3.0).abs() < f32::EPSILON);
        f.set_line_width(0.5);
        assert!((f.line_width() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_mid_line_width_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_mid_line_width(2.0);
        assert!((f.mid_line_width() - 2.0).abs() < f32::EPSILON);
        f.set_mid_line_width(0.0);
        assert!((f.mid_line_width() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_widget_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.widget().is_none());
        let wid: ObjectId = 42;
        f.set_widget(Some(wid));
        assert_eq!(f.widget(), Some(wid));
        f.set_widget(None);
        assert!(f.widget().is_none());
    }

    #[test]
    fn frame_geometry_delegation() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_geometry(Rect::new(10, 20, 300, 150));
        assert_eq!(f.geometry(), Rect::new(10, 20, 300, 150));
    }

    #[test]
    fn frame_visibility() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.is_visible());
        f.hide();
        assert!(!f.is_visible());
        f.show();
        assert!(f.is_visible());
    }

    #[test]
    fn frame_enabled() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.is_enabled());
        f.set_enabled(false);
        assert!(!f.is_enabled());
        f.set_enabled(true);
        assert!(f.is_enabled());
    }

    #[test]
    fn frame_parent_children() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.parent().is_none());
        let pid: ObjectId = 10;
        f.set_parent(Some(pid));
        assert_eq!(f.parent(), Some(pid));
        f.set_parent(None);
        assert!(f.parent().is_none());

        let cid: ObjectId = 20;
        f.add_child(cid);
        assert_eq!(f.children().len(), 1);
        assert_eq!(f.children()[0], cid);
        f.remove_child(cid);
        assert!(f.children().is_empty());
    }

    #[test]
    fn frame_tooltip_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.tooltip().is_empty());
        f.set_tooltip("Frame info".to_string());
        assert_eq!(f.tooltip(), "Frame info");
        f.set_tooltip(String::new());
        assert!(f.tooltip().is_empty());
    }

    #[test]
    fn frame_style_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert_eq!(*f.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::from_rgb(240, 240, 240));
        f.set_style(custom.clone());
        assert_eq!(*f.style(), custom);
    }

    #[test]
    fn frame_id_kind() {
        let f_a = Frame::new(Rect::new(0, 0, 100, 50));
        let f_b = Frame::new(Rect::new(0, 0, 100, 50));
        assert_ne!(f_a.id(), f_b.id());
        assert_eq!(f_a.kind(), WidgetKind::Panel);
        assert_eq!(f_b.kind(), WidgetKind::Panel);
    }

    #[test]
    fn frame_signal_accessors() {
        let f = Frame::new(Rect::new(0, 0, 100, 50));
        let _hover = f.base().hover_signal();
        let _mouse_down = f.base().mouse_down_signal();
        let _mouse_up = f.base().mouse_up_signal();
    }
}
