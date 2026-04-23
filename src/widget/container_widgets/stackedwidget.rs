//! Stacked widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
/// Stacked widget.
pub struct StackedWidget {
    base: BaseWidget,
    widgets: Vec<ObjectId>,
    current_index: usize,
    pub current_changed: Signal1<usize>,
}
impl StackedWidget {
    /// Creates a stacked widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StackedWidget, geometry, "StackedWidget"),
            widgets: Vec::new(),
            current_index: 0,
            current_changed: Signal1::new(),
        }
    }
    /// Adds a widget.
    pub fn add_widget(&mut self, widget: ObjectId) -> usize {
        self.base.add_child(widget);
        self.widgets.push(widget);
        self.widgets.len() - 1
    }
    /// Inserts a widget at position.
    pub fn insert_widget(&mut self, index: usize, widget: ObjectId) {
        self.base.add_child(widget);
        self.widgets.insert(index, widget);
        if self.current_index >= index {
            self.current_index += 1;
        }
    }
    /// Removes a widget.
    pub fn remove_widget(&mut self, widget: ObjectId) {
        if let Some(index) = self.widgets.iter().position(|&id| id == widget) {
            self.base.remove_child(widget);
            self.widgets.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.widgets.is_empty() {
                self.current_index = 0;
            }
        }
    }
    /// Returns number of widgets.
    pub fn count(&self) -> usize {
        self.widgets.len()
    }
    /// Returns current widget index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current widget index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.widgets.len() && self.current_index != index {
            self.current_index = index;
            self.current_changed.emit(index);
        }
    }
    /// Returns current widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.widgets.get(self.current_index).copied()
    }
    /// Returns widget at index.
    pub fn widget(&self, index: usize) -> Option<ObjectId> {
        self.widgets.get(index).copied()
    }
    /// Returns index of widget.
    pub fn index_of(&self, widget: ObjectId) -> Option<usize> {
        self.widgets.iter().position(|&id| id == widget)
    }
}
// Implement Widget trait
impl Widget for StackedWidget {
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
impl EventHandler for StackedWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Forward events to current widget
        if let Some(widget_id) = self.current_widget() {
            // TODO: Forward event to current widget
        }
    }
}
impl Draw for StackedWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        // Draw background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw current widget
        if let Some(widget_id) = self.current_widget() {
            // TODO: Draw current widget
        }
    }
}
