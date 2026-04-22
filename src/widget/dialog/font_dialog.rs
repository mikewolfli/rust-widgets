//! Font dialog widget.

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
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

    pub fn current_font(&self) -> &Font { &self.current_font }

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

    pub fn get_font(&self) -> Font { self.current_font.clone() }
}

impl Widget for FontDialog {
    fn id(&self) -> ObjectId { self.base.id() }
    fn kind(&self) -> WidgetKind { self.base.kind() }
    fn geometry(&self) -> Rect { self.base.geometry() }
    fn set_geometry(&mut self, g: Rect) { self.base.set_geometry(g); }
    fn min_size(&self) -> Option<Size> { self.base.min_size() }
    fn max_size(&self) -> Option<Size> { self.base.max_size() }
    fn set_min_size(&mut self, s: Option<Size>) { self.base.set_min_size(s); }
    fn set_max_size(&mut self, s: Option<Size>) { self.base.set_max_size(s); }
    fn parent(&self) -> Option<ObjectId> { self.base.parent() }
    fn set_parent(&mut self, p: Option<ObjectId>) { self.base.set_parent(p); }
    fn add_child(&mut self, c: ObjectId) { self.base.add_child(c); }
    fn remove_child(&mut self, c: ObjectId) { self.base.remove_child(c); }
    fn children(&self) -> &[ObjectId] { self.base.children() }
    fn show(&mut self) { self.base.show(); }
    fn hide(&mut self) { self.base.hide(); }
    fn is_visible(&self) -> bool { self.base.is_visible() }
    fn set_enabled(&mut self, e: bool) { self.base.set_enabled(e); }
    fn is_enabled(&self) -> bool { self.base.is_enabled() }
    fn set_tooltip(&mut self, t: String) { self.base.set_tooltip(t); }
    fn tooltip(&self) -> &str { self.base.tooltip() }
    fn style(&self) -> &WidgetStyle { self.base.style() }
    fn set_style(&mut self, s: WidgetStyle) { self.base.set_style(s); }
    fn connection_scope(&self) -> &ConnectionScope { self.base.connection_scope() }
    fn hover_signal(&self) -> &Signal1<Point> { self.base.hover_signal() }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_down_signal() }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_up_signal() }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_down_signal() }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_up_signal() }
    fn focus_gained_signal(&self) -> &GenericSignal { self.base.focus_gained_signal() }
    fn focus_lost_signal(&self) -> &GenericSignal { self.base.focus_lost_signal() }
    fn redraw_requested_signal(&self) -> &GenericSignal { self.base.redraw_requested_signal() }
    fn layout_requested_signal(&self) -> &GenericSignal { self.base.layout_requested_signal() }
}

impl EventHandler for FontDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() { return; }
        match event {
            Event::KeyPress { key, .. } => {
                if *key == 13 { self.accept(); }
                else if *key == 27 { self.reject(); }
            }
            _ => {}
        }
    }
}

impl Draw for FontDialog {
    fn draw(&self, context: &mut RenderContext) {
        self.base.draw(context);
        let rect = self.geometry();

        context.fill_rect(rect.x, rect.y, rect.width, rect.height, Color::from_rgb(245, 245, 245));
        context.draw_rect(rect.x, rect.y, rect.width, rect.height, Color::from_rgb(160, 160, 160));
        context.fill_rect(rect.x, rect.y, rect.width, 28.0, Color::from_rgb(0, 120, 215));
        context.draw_text(rect.x + 8.0, rect.y + 14.0, "Select Font", &Font::default(), Color::from_rgb(255, 255, 255), Alignment::Left);

        let col_w = rect.width / 3.0 - 6.0;
        let list_y = rect.y + 38.0;
        let list_h = rect.height - 120.0;

        // Family, Style, Size columns
        for (i, label) in ["Font Family", "Style", "Size"].iter().enumerate() {
            let col_x = rect.x + 4.0 + i as f32 * (col_w + 4.0);
            context.draw_text(col_x, list_y - 10.0, label, &Font::default(), Color::from_rgb(0, 0, 0), Alignment::Left);
            context.fill_rect(col_x, list_y, col_w, list_h, Color::from_rgb(255, 255, 255));
            context.draw_rect(col_x, list_y, col_w, list_h, Color::from_rgb(150, 150, 150));
        }

        // Preview area
        let prev_y = list_y + list_h + 8.0;
        context.fill_rect(rect.x + 4.0, prev_y, rect.width - 8.0, 36.0, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect.x + 4.0, prev_y, rect.width - 8.0, 36.0, Color::from_rgb(150, 150, 150));
        context.draw_text(rect.x + 10.0, prev_y + 18.0, "AaBbYyZz 0123", &self.current_font, Color::from_rgb(0, 0, 0), Alignment::Left);

        // OK/Cancel
        let btn_y = rect.y + rect.height - 40.0;
        context.fill_rect(rect.x + rect.width - 176.0, btn_y, 80.0, 28.0, Color::from_rgb(0, 120, 215));
        context.draw_text(rect.x + rect.width - 136.0, btn_y + 14.0, "OK", &Font::default(), Color::from_rgb(255, 255, 255), Alignment::Center);
        context.fill_rect(rect.x + rect.width - 88.0, btn_y, 80.0, 28.0, Color::from_rgb(225, 225, 225));
        context.draw_rect(rect.x + rect.width - 88.0, btn_y, 80.0, 28.0, Color::from_rgb(100, 100, 100));
        context.draw_text(rect.x + rect.width - 48.0, btn_y + 14.0, "Cancel", &Font::default(), Color::from_rgb(0, 0, 0), Alignment::Center);
    }
}
