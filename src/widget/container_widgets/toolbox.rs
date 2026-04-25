//! Tool box widget.
use crate::core::{Color, Font, ObjectId, Orientation, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Image, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// Tool box widget.
pub struct ToolBox {
    base: BaseWidget,
    items: Vec<ToolBoxItem>,
    current_index: usize,
    orientation: Orientation,
    pub current_changed: Signal1<usize>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
/// Tool box item.
pub struct ToolBoxItem {
    text: String,
    icon: Option<Image>,
    tooltip: String,
    enabled: bool,
    widget: Option<ObjectId>,
}
impl ToolBoxItem {
    /// Creates a new tool box item.
    pub fn new(text: String) -> Self {
        Self {
            text,
            icon: None,
            tooltip: String::new(),
            enabled: true,
            widget: None,
        }
    }
    /// Returns text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets text.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }
    /// Returns icon.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    /// Sets icon.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }
    /// Returns tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }
    /// Sets tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    /// Returns whether item is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Sets enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
    }
}
impl ToolBox {
    /// Creates a tool box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolBox, geometry, "ToolBox"),
            items: Vec::new(),
            current_index: 0,
            orientation: Orientation::Vertical,
            current_changed: Signal1::new(),
            registry: None,
        }
    }
    /// Adds an item.
    pub fn add_item(&mut self, text: String, widget: Option<ObjectId>) -> usize {
        let mut item = ToolBoxItem::new(text);
        item.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.items.push(item);
        self.items.len().saturating_sub(1)
    }
    /// Inserts an item at position.
    pub fn insert_item(&mut self, index: usize, text: String, widget: Option<ObjectId>) {
        let mut item = ToolBoxItem::new(text);
        item.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.items.insert(index, item);
        if self.current_index >= index {
            self.current_index += 1;
        }
    }
    /// Removes an item.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            if let Some(widget_id) = self.items[index].widget {
                self.base.remove_child(widget_id);
            }
            self.items.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.items.is_empty() {
                self.current_index = 0;
            }
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns current item index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current item index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.items.len() && self.current_index != index {
            self.current_index = index;
            self.current_changed.emit(index);
        }
    }
    /// Returns current item widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.items
            .get(self.current_index)
            .and_then(|item| item.widget)
    }
    /// Returns item at index.
    pub fn item(&self, index: usize) -> Option<&ToolBoxItem> {
        self.items.get(index)
    }
    /// Returns mutable item at index.
    pub fn item_mut(&mut self, index: usize) -> Option<&mut ToolBoxItem> {
        self.items.get_mut(index)
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }
    /// Returns item rectangle at index.
    fn item_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let rect = self.geometry();
        let item_height = 32;
        let item_width = 120;
        match self.orientation {
            Orientation::Horizontal => {
                let x = rect.x as f32 + item_width as f32 * index as f32;
                Some(Rect::new(x as i32, rect.y, item_width, rect.height))
            }
            Orientation::Vertical => {
                let y = rect.y as f32 + item_height as f32 * index as f32;
                Some(Rect::new(rect.x, y as i32, rect.width, item_height))
            }
        }
    }
    /// Returns content rectangle.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        match self.orientation {
            Orientation::Horizontal => {
                let item_width = 120;
                let content_width =
                    (rect.width as f32 - item_width as f32 * self.items.len() as f32).max(0.0);
                Rect::new(
                    (rect.x as f32 + item_width as f32 * self.items.len() as f32) as i32,
                    rect.y,
                    content_width as u32,
                    rect.height,
                )
            }
            Orientation::Vertical => {
                let item_height = 32;
                let content_height =
                    (rect.height as f32 - item_height as f32 * self.items.len() as f32).max(0.0);
                Rect::new(
                    rect.x,
                    (rect.y as f32 + item_height as f32 * self.items.len() as f32) as i32,
                    rect.width,
                    content_height as u32,
                )
            }
        }
    }
    /// Returns index of item at position.
    fn item_at_position(&self, pos: Point) -> Option<usize> {
        for i in 0..self.items.len() {
            if let Some(item_rect) = self.item_rect(i) {
                if item_rect.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }
}
// Implement Widget trait
impl Widget for ToolBox {
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
impl ToolBox {
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
}
impl EventHandler for ToolBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if let Some(index) = self.item_at_position(*pos) {
                        if self.items[index].enabled {
                            self.set_current_index(index);
                        }
                    }
                }
            }
            _ => {}
        }
        // Forward events to current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for ToolBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let _rect = self.geometry();
        let content_rect = self.content_rect();
        // Draw content background
        context.fill_rect(
            Rect::new(
                content_rect.x,
                content_rect.y,
                content_rect.width,
                content_rect.height,
            ),
            Color::from_rgb(255, 255, 255),
        );
        // Draw content border
        context.draw_rect(
            Rect::new(
                content_rect.x,
                content_rect.y,
                content_rect.width,
                content_rect.height,
            ),
            Color::from_rgb(200, 200, 200),
        );
        // Draw items
        for i in 0..self.items.len() {
            if let Some(item_rect) = self.item_rect(i) {
                let item = &self.items[i];
                let is_current = i == self.current_index;
                let is_enabled = item.enabled;
                // Draw item background
                let bg_color = if !is_enabled {
                    Color::from_rgb(240, 240, 240)
                } else if is_current {
                    Color::from_rgb(220, 220, 255)
                } else {
                    Color::from_rgb(240, 240, 240)
                };
                context.fill_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    bg_color,
                );
                // Draw item border
                let border_color = if !is_enabled {
                    Color::from_rgb(200, 200, 200)
                } else if is_current {
                    Color::from_rgb(100, 100, 200)
                } else {
                    Color::from_rgb(200, 200, 200)
                };
                context.draw_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    border_color,
                );
                // Draw icon if available
                let icon_size = 16;
                let text_x = if item.icon.is_some() {
                    item_rect.x + icon_size + 5
                } else {
                    item_rect.x + 5
                };
                if item.icon.is_some() {
                    // NOTE: Full icon rendering requires draw_image() on RenderContext
                    // For now, draw a placeholder gray square
                    context.fill_rect(
                        Rect::new(
                            item_rect.x + 5,
                            item_rect.y + (item_rect.height - icon_size as u32) as i32 / 2,
                            icon_size as u32,
                            icon_size as u32,
                        ),
                        Color::from_rgb(150, 150, 150),
                    );
                }
                // Draw item text
                let text_color = if !is_enabled {
                    Color::from_rgb(150, 150, 150)
                } else {
                    Color::from_rgb(0, 0, 0)
                };
                context.draw_text(
                    Point::new(text_x, item_rect.y + item_rect.height as i32 / 2),
                    &item.text,
                    &Font::default(),
                    text_color,
                );
            }
        }
        // Draw current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
    }
}
