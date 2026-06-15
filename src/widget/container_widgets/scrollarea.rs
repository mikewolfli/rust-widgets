//! Scroll area widget.
use crate::core::{Alignment, Color, ObjectId, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// Scroll area widget.
pub struct ScrollArea {
    base: BaseWidget,
    widget_resizable: bool,
    alignment: Alignment,
    horizontal_scroll_bar_policy: ScrollBarPolicy,
    vertical_scroll_bar_policy: ScrollBarPolicy,
    viewport: Rect,
    widget: Option<ObjectId>,
    /// The total size of the content (for AsNeeded scroll bar calculation).
    content_size: Size,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Current scroll position (x, y) in content coordinates.
    scroll_position: (i32, i32),
    /// Emitted whenever scroll position changes.
    pub scroll_position_changed: Signal1<(i32, i32)>,
}
/// Scroll bar policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBarPolicy {
    /// Scroll bar is always shown
    AlwaysOn,
    /// Scroll bar is always hidden
    AlwaysOff,
    /// Scroll bar is shown when needed
    #[default]
    AsNeeded,
}
impl ScrollArea {
    /// Creates a scroll area.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScrollArea, geometry, "ScrollArea"),
            widget_resizable: true,
            alignment: Alignment::Center,
            horizontal_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            vertical_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            viewport: Rect::new(0, 0, 0, 0),
            widget: None,
            content_size: Size::new(0, 0),
            registry: None,
            scroll_position: (0, 0),
            scroll_position_changed: Signal1::new(),
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
    /// Returns the current scroll position.
    pub fn scroll_position(&self) -> (i32, i32) {
        self.scroll_position
    }

    /// Sets the scroll position, clamped within the content extent.
    pub fn set_scroll_position(&mut self, x: i32, y: i32) {
        let view_w = self.viewport.width as i32;
        let view_h = self.viewport.height as i32;
        let content_w = self.content_size.width as i32;
        let content_h = self.content_size.height as i32;
        let max_x = (content_w - view_w).max(0);
        let max_y = (content_h - view_h).max(0);
        let clamped = (x.clamp(0, max_x), y.clamp(0, max_y));
        if self.scroll_position == clamped {
            return;
        }
        self.scroll_position = clamped;
        self.scroll_position_changed.emit(self.scroll_position);
        self.base.request_redraw();
    }

    /// Sets the total content size (used for AsNeeded scroll bar calculation).
    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
    }

    /// Returns the total content size.
    pub fn content_size(&self) -> Size {
        self.content_size
    }
    /// Ensures rectangle is visible.
    pub fn ensure_visible(&mut self, rect: Rect) {
        // Adjust viewport to make rect visible
        let mut new_viewport = self.viewport;
        if rect.x < new_viewport.x {
            new_viewport.x = rect.x;
        } else if rect.x as f32 + rect.width as f32
            > new_viewport.x as f32 + new_viewport.width as f32
        {
            new_viewport.x = (rect.x as f32 + rect.width as f32 - new_viewport.width as f32) as i32;
        }
        if rect.y < new_viewport.y {
            new_viewport.y = rect.y;
        } else if rect.y as f32 + rect.height as f32
            > new_viewport.y as f32 + new_viewport.height as f32
        {
            new_viewport.y =
                (rect.y as f32 + rect.height as f32 - new_viewport.height as f32) as i32;
        }
        self.viewport = new_viewport;
    }
    /// Ensures widget is visible.
    pub fn ensure_widget_visible(&mut self, widget_id: ObjectId) {
        // If the widget is the child widget, center the viewport on the content.
        if Some(widget_id) == self.widget {
            let half_w = self.viewport.width / 2;
            let half_h = self.viewport.height / 2;
            let cx = self.viewport.x + self.viewport.width as i32 / 2;
            let cy = self.viewport.y + self.viewport.height as i32 / 2;
            self.viewport.x = (cx - half_w as i32).max(0);
            self.viewport.y = (cy - half_h as i32).max(0);
        }
    }

    /// Scrolls to the top of the content.
    pub fn scroll_to_top(&mut self) {
        self.set_scroll_position(self.scroll_position.0, 0);
    }

    /// Scrolls to the bottom of the content.
    pub fn scroll_to_bottom(&mut self) {
        if self.content_size.height > self.viewport.height {
            self.set_scroll_position(
                self.scroll_position.0,
                (self.content_size.height - self.viewport.height) as i32,
            );
        } else {
            self.set_scroll_position(self.scroll_position.0, 0);
        }
    }

    /// Scrolls to the left edge of the content.
    pub fn scroll_to_left(&mut self) {
        self.set_scroll_position(0, self.scroll_position.1);
    }

    /// Scrolls to the right edge of the content.
    pub fn scroll_to_right(&mut self) {
        if self.content_size.width > self.viewport.width {
            self.set_scroll_position(
                (self.content_size.width - self.viewport.width) as i32,
                self.scroll_position.1,
            );
        } else {
            self.set_scroll_position(0, self.scroll_position.1);
        }
    }
    /// Returns whether horizontal scroll bar is visible.
    fn horizontal_scroll_bar_visible(&self) -> bool {
        match self.horizontal_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => self.content_size.width > self.viewport.width,
        }
    }
    /// Returns whether vertical scroll bar is visible.
    fn vertical_scroll_bar_visible(&self) -> bool {
        match self.vertical_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => self.content_size.height > self.viewport.height,
        }
    }
    /// Updates scroll bars — redraws so scroll bar visibility changes take effect.
    /// Note: Full scroll bar widget creation would require access to a widget factory
    /// and is not yet implemented. Visual scroll bar drawing is handled in `Draw`.
    fn update_scroll_bars(&mut self) {
        self.base.request_redraw();
    }
}
// Implement Widget trait
impl Widget for ScrollArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.viewport.width = geometry.width;
        self.viewport.height = geometry.height;
        self.update_scroll_bars();
    }
}
impl EventHandler for ScrollArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // Handle scroll events (mouse wheel + touch swipe)
        match event {
            Event::Wheel { delta, modifiers: _ } => {
                // Scroll the content using clamped content coordinates.
                self.set_scroll_position(
                    self.scroll_position.0 + delta.x * 20,
                    self.scroll_position.1 + delta.y * 20,
                );
            }
            #[cfg(feature = "touch")]
            Event::Swipe { start, end, velocity: _ } => {
                // Map swipe to scroll
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                self.set_scroll_position(self.scroll_position.0 + dx, self.scroll_position.1 + dy);
            }
            #[cfg(feature = "touch")]
            Event::Drag { delta, .. } => {
                // Map finger drag to scroll
                self.set_scroll_position(
                    self.scroll_position.0 + delta.x,
                    self.scroll_position.1 + delta.y,
                );
            }
            _ => { /* Other events are not relevant */ }
        }
        // Forward events to widget via registry (with viewport offset)
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for ScrollArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let style = self.style();
        // Draw background
        context.fill_rect(rect, style.background_color.unwrap_or(Color::rgb(255, 255, 255)));
        // Draw border
        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(200, 200, 200)));
        // Set viewport for clipping
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        // Draw widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
        context.pop_clip();
        // Draw scroll bars if visible
        let h_scroll_visible = self.horizontal_scroll_bar_visible();
        let v_scroll_visible = self.vertical_scroll_bar_visible();
        if h_scroll_visible {
            // Draw horizontal scroll bar
            let scroll_bar_height = 16;
            let scroll_bar_y = rect.y as f32 + rect.height as f32 - scroll_bar_height as f32;
            context.fill_rect(
                Rect::new(rect.x, scroll_bar_y as i32, rect.width, scroll_bar_height as u32),
                Color::rgb(240, 240, 240),
            );
            context.draw_rect(
                Rect::new(rect.x, scroll_bar_y as i32, rect.width, scroll_bar_height as u32),
                Color::rgb(200, 200, 200),
            );
            // Draw scroll bar thumb
            let thumb_width = rect.width * 3 / 10;
            let thumb_x = rect.x
                + ((rect.width - thumb_width) as i32)
                    * (self.viewport.x / self.viewport.width.max(1) as i32);
            context.fill_rect(
                Rect::new(thumb_x, scroll_bar_y as i32, thumb_width, scroll_bar_height as u32),
                Color::rgb(180, 180, 180),
            );
        }
        if v_scroll_visible {
            // Draw vertical scroll bar
            let scroll_bar_width = 16;
            let scroll_bar_x = rect.x as f32 + rect.width as f32 - scroll_bar_width as f32;
            context.fill_rect(
                Rect::new(scroll_bar_x as i32, rect.y, scroll_bar_width as u32, rect.height),
                Color::rgb(240, 240, 240),
            );
            context.draw_rect(
                Rect::new(scroll_bar_x as i32, rect.y, scroll_bar_width as u32, rect.height),
                Color::rgb(200, 200, 200),
            );
            // Draw scroll bar thumb
            let thumb_height = rect.height * 3 / 10;
            let thumb_y = rect.y
                + ((rect.height - thumb_height) as i32)
                    * (self.viewport.y / self.viewport.height.max(1) as i32);
            context.fill_rect(
                Rect::new(scroll_bar_x as i32, thumb_y, scroll_bar_width as u32, thumb_height),
                Color::rgb(180, 180, 180),
            );
        }
        // Draw corner between scroll bars
        if h_scroll_visible && v_scroll_visible {
            let corner_size = 16;
            let corner_x = rect.x as f32 + rect.width as f32 - corner_size as f32;
            let corner_y = rect.y as f32 + rect.height as f32 - corner_size as f32;
            context.fill_rect(
                Rect::new(corner_x as i32, corner_y as i32, corner_size as u32, corner_size as u32),
                Color::rgb(240, 240, 240),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn scrollarea_creation_defaults() {
        let sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        assert_eq!(sa.geometry(), Rect::new(0, 0, 200, 200));
        assert_eq!(sa.scroll_position(), (0, 0));
        assert_eq!(sa.horizontal_scroll_bar_policy(), ScrollBarPolicy::AsNeeded);
        assert_eq!(sa.vertical_scroll_bar_policy(), ScrollBarPolicy::AsNeeded);
    }

    #[test]
    fn scrollarea_set_scroll_position_clamps_content_space() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(300, 260));
        sa.set_viewport(Rect::new(0, 0, 100, 100));

        sa.set_scroll_position(500, 400);
        assert_eq!(sa.scroll_position(), (200, 160));

        sa.set_scroll_position(-10, -8);
        assert_eq!(sa.scroll_position(), (0, 0));
    }
}
