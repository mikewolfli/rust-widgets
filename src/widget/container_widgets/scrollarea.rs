//! Scroll area widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
/// Scroll area widget.
pub struct ScrollArea {
    base: BaseWidget,
    widget_resizable: bool,
    alignment: Alignment,
    horizontal_scroll_bar_policy: ScrollBarPolicy,
    vertical_scroll_bar_policy: ScrollBarPolicy,
    horizontal_scroll_bar: Option<ObjectId>,
    vertical_scroll_bar: Option<ObjectId>,
    viewport: Rect,
    widget: Option<ObjectId>,
}
/// Scroll bar policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBarPolicy {
    /// Scroll bar is always shown
    AlwaysOn,
    /// Scroll bar is always hidden
    AlwaysOff,
    /// Scroll bar is shown when needed
    AsNeeded,
}
impl Default for ScrollBarPolicy {
    fn default() -> Self {
        Self::AsNeeded
    }
}
impl ScrollArea {
    /// Creates a scroll area.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScrollArea, geometry, "ScrollArea"),
            widget_resizable: false,
            alignment: Alignment::Center,
            horizontal_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            vertical_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            horizontal_scroll_bar: None,
            vertical_scroll_bar: None,
            viewport: Rect::new(0, 0, geometry.width, geometry.height),
            widget: None,
        }
    }
    /// Returns whether the widget is resizable.
    pub fn widget_resizable(&self) -> bool {
        self.widget_resizable
    }
    /// Sets whether the widget is resizable.
    pub fn set_widget_resizable(&mut self, resizable: bool) {
        self.widget_resizable = resizable;
    }
    /// Returns alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
    /// Sets alignment.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
    }
    /// Returns horizontal scroll bar policy.
    pub fn horizontal_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.horizontal_scroll_bar_policy
    }
    /// Sets horizontal scroll bar policy.
    pub fn set_horizontal_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        self.horizontal_scroll_bar_policy = policy;
    }
    /// Returns vertical scroll bar policy.
    pub fn vertical_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.vertical_scroll_bar_policy
    }
    /// Sets vertical scroll bar policy.
    pub fn set_vertical_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        self.vertical_scroll_bar_policy = policy;
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
    /// Returns viewport rectangle.
    pub fn viewport(&self) -> Rect {
        self.viewport
    }
    /// Sets viewport rectangle.
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
    }
    /// Ensures rectangle is visible.
    pub fn ensure_visible(&mut self, rect: Rect) {
        // Adjust viewport to make rect visible
        let mut new_viewport = self.viewport;
        if rect.x < new_viewport.x {
            new_viewport.x = rect.x;
        } else if rect.x + rect.width as i32 > new_viewport.x + new_viewport.width {
            new_viewport.x = rect.x + rect.width as i32 - new_viewport.width;
        }
        if rect.y < new_viewport.y {
            new_viewport.y = rect.y;
        } else if rect.y + rect.height as i32 > new_viewport.y + new_viewport.height {
            new_viewport.y = rect.y + rect.height as i32 - new_viewport.height;
        }
        self.viewport = new_viewport;
    }
    /// Ensures widget is visible.
    pub fn ensure_widget_visible(&mut self, widget_id: ObjectId) {
        // This would need access to widget geometry
        // For now, just center the viewport
        if let Some(_) = self.widget {
            // TODO: Get widget geometry and ensure visible
        }
    }
    /// Returns whether horizontal scroll bar is visible.
    fn horizontal_scroll_bar_visible(&self) -> bool {
        match self.horizontal_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => {
                if let Some(widget_id) = self.widget {
                    // TODO: Check if widget width > viewport width
                    false
                } else {
                    false
                }
            }
        }
    }
    /// Returns whether vertical scroll bar is visible.
    fn vertical_scroll_bar_visible(&self) -> bool {
        match self.vertical_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => {
                if let Some(widget_id) = self.widget {
                    // TODO: Check if widget height > viewport height
                    false
                } else {
                    false
                }
            }
        }
    }
    /// Updates scroll bars.
    fn update_scroll_bars(&mut self) {
        // TODO: Create or update scroll bars based on policy and widget size
    }
}
// Implement Widget trait
impl Widget for ScrollArea {
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
        self.viewport.width = geometry.width;
        self.viewport.height = geometry.height;
        self.update_scroll_bars();
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
impl EventHandler for ScrollArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Handle scroll events
        match event {
            Event::Wheel {
                delta,
                modifiers: _,
            } => {
                // Scroll the viewport
                self.viewport.x += delta.x * 20;
                self.viewport.y += delta.y * 20;
            }
            _ => {}
        }
        // Forward events to widget (with viewport offset)
        if let Some(widget_id) = self.widget {
            // TODO: Forward event to widget with adjusted coordinates
        }
    }
}
impl Draw for ScrollArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        // Draw background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
        // Set viewport for clipping
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        // Draw widget with viewport offset
        if let Some(widget_id) = self.widget {
            // TODO: Draw widget with translation based on viewport
            // context.translate(-self.viewport.x, -self.viewport.y);
            // widget.draw(context);
            // context.translate(self.viewport.x, self.viewport.y);
        }
        context.pop_clip();
        // Draw scroll bars if visible
        let h_scroll_visible = self.horizontal_scroll_bar_visible();
        let v_scroll_visible = self.vertical_scroll_bar_visible();
        if h_scroll_visible {
            // Draw horizontal scroll bar
            let scroll_bar_height = 16;
            let scroll_bar_y = rect.y + rect.height as i32 - scroll_bar_height;
            context.fill_rect(
                Rect::new(rect.x, scroll_bar_y, rect.width, scroll_bar_height as u32),
                Color::from_rgb(240, 240, 240),
            );
            context.draw_rect(
                Rect::new(rect.x, scroll_bar_y, rect.width, scroll_bar_height as u32),
                Color::from_rgb(200, 200, 200),
            );
            // Draw scroll bar thumb
            let thumb_width = rect.width * 3 / 10;
            let thumb_x = rect.x
                + (rect.width - thumb_width) * (self.viewport.x / self.viewport.width.max(1));
            context.fill_rect(
                Rect::new(
                    thumb_x as i32,
                    scroll_bar_y,
                    thumb_width as u32,
                    scroll_bar_height as u32,
                ),
                Color::from_rgb(180, 180, 180),
            );
        }
        if v_scroll_visible {
            // Draw vertical scroll bar
            let scroll_bar_width = 16;
            let scroll_bar_x = rect.x + rect.width as i32 - scroll_bar_width;
            context.fill_rect(
                Rect::new(scroll_bar_x, rect.y, scroll_bar_width as u32, rect.height),
                Color::from_rgb(240, 240, 240),
            );
            context.draw_rect(
                Rect::new(scroll_bar_x, rect.y, scroll_bar_width as u32, rect.height),
                Color::from_rgb(200, 200, 200),
            );
            // Draw scroll bar thumb
            let thumb_height = rect.height * 3 / 10;
            let thumb_y = rect.y
                + (rect.height - thumb_height) * (self.viewport.y / self.viewport.height.max(1));
            context.fill_rect(
                Rect::new(
                    scroll_bar_x,
                    thumb_y as i32,
                    scroll_bar_width as u32,
                    thumb_height as u32,
                ),
                Color::from_rgb(180, 180, 180),
            );
        }
        // Draw corner between scroll bars
        if h_scroll_visible && v_scroll_visible {
            let corner_size = 16;
            let corner_x = rect.x + rect.width as i32 - corner_size;
            let corner_y = rect.y + rect.height as i32 - corner_size;
            context.fill_rect(
                Rect::new(corner_x, corner_y, corner_size as u32, corner_size as u32),
                Color::from_rgb(240, 240, 240),
            );
        }
    }
}
