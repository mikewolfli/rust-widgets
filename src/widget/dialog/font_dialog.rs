//! Font dialog widget.
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Font selection dialog.
pub struct FontDialog {
    base: BaseWidget,
    current_font: Font,
    pub font_selected: Signal1<Font>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl FontDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dialog, geometry, "FontDialog"),
            current_font: Font::default(),
            font_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    pub fn set_current_font(&mut self, font: Font) {
        self.current_font = font.clone();
        self.font_selected.emit(font);
    }
    pub fn accept(&mut self) {
        self.font_selected.emit(self.current_font.clone());
        self.accepted.emit();
        self.hide();
    }
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
    pub fn get_font(&self) -> Font {
        self.current_font.clone()
    }
}
impl Widget for FontDialog {
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
impl EventHandler for FontDialog {
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
impl Draw for FontDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(160, 160, 160),
        );
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            "Select Font",
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        let col_w = (rect.width / 3).saturating_sub(6);
        let list_y = rect.y + 38;
        let list_h = rect.height.saturating_sub(120);
        // Family, Style, Size columns
        for (i, label) in ["Font Family", "Style", "Size"].iter().enumerate() {
            let col_x = rect.x as f32 + 4.0 + i as f32 * (col_w as f32 + 4.0);
            context.draw_text(
                Point::new(col_x as i32, list_y - 10),
                label,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
            );
            context.fill_rect(Rect::new(col_x as i32, list_y, col_w, list_h), Color::from_rgb(255, 255, 255));
            context.draw_rect(Rect::new(col_x as i32, list_y, col_w, list_h), Color::from_rgb(150, 150, 150));
        }
        // Preview area
        let prev_y = list_y + list_h as i32 + 8;
        let bw = rect.width.saturating_sub(8);
        context.fill_rect(
            Rect::new(rect.x + 4, prev_y, bw, 36),
            Color::from_rgb(255, 255, 255),
        );
        context.draw_rect(
            Rect::new(rect.x + 4, prev_y, bw, 36),
            Color::from_rgb(150, 150, 150),
        );
        context.draw_text(
            Point::new(rect.x + 10, prev_y + 18),
            "AaBbYyZz 0123",
            &self.current_font,
            Color::from_rgb(0, 0, 0),
        );
        // OK/Cancel
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, 80, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            "OK",
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, 80, 28),
            Color::from_rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, 80, 28),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 48, (btn_y + 14.0) as i32),
            "Cancel",
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
