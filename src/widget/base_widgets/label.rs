//! Label widget implementation.
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Label widget for displaying text.
pub struct Label {
    base: BaseWidget,
    text: String,
    alignment: crate::core::Alignment,
}
impl Label {
    /// Creates a label with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, geometry, "Label"),
            text,
            alignment: crate::core::Alignment::Left,
        }
    }
    /// Returns label text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets label text.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.base.request_redraw();
    }
    /// Returns text alignment.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }
    /// Sets text alignment.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        self.alignment = alignment;
        self.base.request_redraw();
    }
}
impl Widget for Label {
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
impl EventHandler for Label {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}
impl Draw for Label {
    fn draw(&mut self, context: &mut RenderContext) {
        // Label rendering logic
        let rect = self.geometry();
        // Draw background if specified
        if let Some(bg_color) = self.style().background_color {
            context.fill_rect(rect, bg_color);
        }
        // Draw text
        if !self.text.is_empty() {
            let text_color = self.style().text_color.unwrap_or(Color::from_rgb(0, 0, 0));
            context.draw_text(
                Point::new(rect.x, rect.y),
                &self.text,
                &self.font().cloned().unwrap_or_default(),
                text_color,
            );
        }
        // Draw border if specified
        if let Some(border_color) = self.style().border_color {
            context.draw_rect(rect, border_color);
        }
    }
}
