//! Color dialog widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Color dialog for picking RGBA colors.
pub struct ColorDialog {
    base: BaseWidget,
    current_color: Color,
    options_alpha: bool,
    pub color_selected: Signal1<Color>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl ColorDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dialog, geometry, "ColorDialog"),
            current_color: Color::from_rgb(255, 255, 255),
            options_alpha: false,
            color_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn current_color(&self) -> Color {
        self.current_color
    }
    pub fn options_alpha(&self) -> bool {
        self.options_alpha
    }
    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
        self.color_selected.emit(color);
    }
    pub fn set_options_alpha(&mut self, enabled: bool) {
        self.options_alpha = enabled;
    }
    pub fn accept(&mut self) {
        self.accepted.emit();
        self.hide();
    }
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
    pub fn get_color(&self) -> Color {
        self.current_color
    }
}
impl Widget for ColorDialog {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
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
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
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
impl EventHandler for ColorDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => {
                if *key == 13 {
                    self.accept();
                } else if *key == 27 {
                    self.reject();
                }
            }
            _ => {}
        }
    }
}
impl Draw for ColorDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(245, 245, 245),
        );
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(160, 160, 160),
        );
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            28,
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            rect.x + 8,
            rect.y + 14,
            "Select Color",
            &Font::default(),
            Color::from_rgb(255, 255, 255),
            Alignment::Left,
        );
        // Color picker area (simplified)
        let picker_rect_x = rect.x + 10;
        let picker_rect_y = rect.y + 38;
        let picker_w = rect.width - 20;
        let picker_h = rect.height - 120;
        context.fill_rect(
            picker_rect_x,
            picker_rect_y,
            picker_w,
            picker_h,
            Color::from_rgb(200, 200, 200),
        );
        context.draw_rect(
            picker_rect_x,
            picker_rect_y,
            picker_w,
            picker_h,
            Color::from_rgb(100, 100, 100),
        );
        // Color preview
        let preview_y = rect.y + rect.height as f32 - 80;
        context.fill_rect(Rect::new(rect.x + 10, preview_y, 60, 30), self.current_color);
        context.draw_rect(
            rect.x + 10,
            preview_y,
            60,
            30,
            Color::from_rgb(0, 0, 0),
        );
        context.draw_text(
            rect.x + 80,
            preview_y + 15,
            "Current Color",
            &Font::default(),
            Color::from_rgb(0, 0, 0),
            Alignment::Left,
        );
        // OK/Cancel buttons
        let btn_y = rect.y + rect.height as f32 - 40;
        let btn_w = 80;
        context.fill_rect(
            rect.x + rect.width as f32 - 176,
            btn_y,
            btn_w,
            28,
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            rect.x + rect.width as f32 - 136,
            btn_y + 14,
            "OK",
            &Font::default(),
            Color::from_rgb(255, 255, 255),
            Alignment::Center,
        );
        context.fill_rect(
            rect.x + rect.width as f32 - 88,
            btn_y,
            btn_w,
            28,
            Color::from_rgb(225, 225, 225),
        );
        context.draw_rect(
            rect.x + rect.width as f32 - 88,
            btn_y,
            btn_w,
            28,
            Color::from_rgb(100, 100, 100),
        );
        context.draw_text(
            rect.x + rect.width as f32 - 48,
            btn_y + 14,
            "Cancel",
            &Font::default(),
            Color::from_rgb(0, 0, 0),
            Alignment::Center,
        );
    }
}
