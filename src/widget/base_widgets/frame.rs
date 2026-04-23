//! Frame widget.
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Frame widget.
pub struct Frame {
    base: BaseWidget,
    frame_shape: FrameShape,
    frame_shadow: FrameShadow,
    line_width: f32,
    mid_line_width: f32,
    widget: Option<ObjectId>,
}
/// Frame shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameShape {
    /// No frame
    NoFrame,
    /// Box frame
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
impl Default for FrameShape {
    fn default() -> Self {
        Self::Box
    }
}
/// Frame shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameShadow {
    /// Plain shadow
    Plain,
    /// Raised shadow
    Raised,
    /// Sunken shadow
    Sunken,
}
impl Default for FrameShadow {
    fn default() -> Self {
        Self::Plain
    }
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
        }
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
    /// Returns content rectangle.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let padding = self.frame_padding();
        Rect::new(
            rect.x + padding.left as i32,
            rect.y + padding.top as i32,
            rect.width.saturating_sub(padding.left + padding.right),
            rect.height.saturating_sub(padding.top + padding.bottom),
        )
    }
    /// Returns frame padding based on shape and shadow.
    fn frame_padding(&self) -> Padding {
        match self.frame_shape {
            FrameShape::NoFrame => Padding::all(0),
            FrameShape::Box
            | FrameShape::Panel
            | FrameShape::StyledPanel
            | FrameShape::WinPanel => {
                let line_width = self.line_width as u32;
                let mid_line_width = self.mid_line_width as u32;
                let total_width = line_width + mid_line_width;
                match self.frame_shadow {
                    FrameShadow::Plain => Padding::all(total_width),
                    FrameShadow::Raised | FrameShadow::Sunken => Padding::all(total_width * 2),
                }
            }
            FrameShape::HLine => {
                let line_width = self.line_width as u32;
                Padding::new(0, line_width, 0, line_width)
            }
            FrameShape::VLine => {
                let line_width = self.line_width as u32;
                Padding::new(line_width, 0, line_width, 0)
            }
        }
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
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
                    dark_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
                    dark_color,
                );
                // Draw mid line if needed
                if mid_line_width > 0.0 {
                    let mid_light = Color::from_rgb(192, 192, 192);
                    let mid_dark = Color::from_rgb(64, 64, 64);
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + line_width),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + rect.height as f32 - line_width),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + rect.height as f32 - line_width),
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + rect.height as f32 - line_width),
                        mid_dark,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + line_width),
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + rect.height as f32 - line_width),
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
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
                    dark_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
                    dark_color,
                );
                // Draw mid line if needed
                if mid_line_width > 0.0 {
                    let mid_light = Color::from_rgb(64, 64, 64);
                    let mid_dark = Color::from_rgb(192, 192, 192);
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + line_width),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + rect.height as f32 - line_width),
                        mid_light,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + rect.height as f32 - line_width),
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + rect.height as f32 - line_width),
                        mid_dark,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + line_width),
                        Point::from_f32(rect.x as f32 + rect.width as f32 - line_width, rect.y as f32 + rect.height as f32 - line_width),
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
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
            light_color,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
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
            Point::new(inner_rect.x + inner_rect.width as i32, inner_rect.y + inner_rect.height as i32),
            dark_color,
        );
        context.draw_line(
            Point::new(inner_rect.x + inner_rect.width as i32, inner_rect.y),
            Point::new(inner_rect.x + inner_rect.width as i32, inner_rect.y + inner_rect.height as i32),
            dark_color,
        );
    }
}
// Implement Widget trait
impl Widget for Frame {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}
impl EventHandler for Frame {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Forward events to widget
        if self.widget.is_some() {
            // TODO: Forward event to widget
        }
    }
}
impl Draw for Frame {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw frame
        self.draw_frame(context);
        // Draw widget
        if self.widget.is_some() {
            // TODO: Draw widget in content area
            let _content_rect = self.content_rect();
            // widget.draw(context);
        }
    }
}
