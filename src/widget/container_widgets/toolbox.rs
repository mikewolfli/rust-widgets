//! Tool box widget.
use crate::core::{HorizontalAlignment, Color, Font, ObjectId, Orientation, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

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
        Self { text, icon: None, tooltip: String::new(), enabled: true, widget: None }
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
            base: BaseWidget::new(WidgetKind::Toolbox, geometry, "ToolBox"),
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
        self.items.get(self.current_index).and_then(|item| item.widget)
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
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
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
        if let Event::MousePress { pos, button } = event {
            if *button == 1 {
                if let Some(index) = self.item_at_position(*pos) {
                    if self.items[index].enabled {
                        self.set_current_index(index);
                    }
                }
            }
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
            Rect::new(content_rect.x, content_rect.y, content_rect.width, content_rect.height),
            Color::from_rgb(255, 255, 255),
        );
        // Draw content border
        context.draw_rect(
            Rect::new(content_rect.x, content_rect.y, content_rect.width, content_rect.height),
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
                let text_x =
                    if item.icon.is_some() { item_rect.x + icon_size + 5 } else { item_rect.x + 5 };
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
                    HorizontalAlignment::Left,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect};
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // ── Helper constants ──────────────────────────────────────────────────

    fn widget_id_1() -> ObjectId {
        9101
    }
    fn widget_id_2() -> ObjectId {
        9102
    }
    fn widget_id_3() -> ObjectId {
        9103
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────

    #[test]
    fn toolbox_creation_defaults() {
        let tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.count(), 0, "should have no items");
        assert_eq!(tb.current_index(), 0, "current index should be 0");
        assert_eq!(tb.current_widget(), None, "no current widget");
        assert_eq!(tb.orientation(), Orientation::Vertical, "default orientation");
        assert!(tb.is_visible(), "should be visible");
        assert!(tb.is_enabled(), "should be enabled");
        assert_eq!(tb.geometry(), Rect::new(0, 0, 200, 160));
        assert_eq!(tb.kind(), WidgetKind::Toolbox);
        assert!(tb.registry().is_none(), "registry should be None by default");
    }

    // ── 2. Adding items ───────────────────────────────────────────────────

    #[test]
    fn toolbox_add_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        let idx = tb.add_item("First".to_string(), None);
        assert_eq!(idx, 0, "first item index should be 0");
        assert_eq!(tb.count(), 1);

        let idx2 = tb.add_item("Second".to_string(), Some(widget_id_1()));
        assert_eq!(idx2, 1, "second item index should be 1");
        assert_eq!(tb.count(), 2);

        // The widget associated with the second item becomes a child
        let children = tb.children();
        assert!(children.contains(&widget_id_1()), "child widget should be tracked");
    }

    #[test]
    fn toolbox_add_item_without_widget_does_not_add_child() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("NoWidget".to_string(), None);
        assert!(tb.children().is_empty(), "no children when no widget is given");
    }

    // ── 3. Setting/getting current index ──────────────────────────────────

    #[test]
    fn toolbox_get_set_current_index() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        assert_eq!(tb.current_index(), 0, "starts at 0");
        tb.set_current_index(1);
        assert_eq!(tb.current_index(), 1);
        tb.set_current_index(0);
        assert_eq!(tb.current_index(), 0);
    }

    #[test]
    fn toolbox_set_current_index_out_of_bounds_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("Only".to_string(), None);
        tb.set_current_index(5); // out of bounds
        assert_eq!(tb.current_index(), 0, "should remain 0");
    }

    #[test]
    fn toolbox_set_current_index_same_value_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.set_current_index(0); // already 0, should not emit signal
        assert_eq!(tb.current_index(), 0);
    }

    // ── 4. Item count ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_item_count() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.count(), 0);
        tb.add_item("A".to_string(), None);
        assert_eq!(tb.count(), 1);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        assert_eq!(tb.count(), 3);
    }

    // ── 5. Removing items ─────────────────────────────────────────────────

    #[test]
    fn toolbox_remove_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), Some(widget_id_1()));
        tb.add_item("C".to_string(), None);

        tb.remove_item(1);
        assert_eq!(tb.count(), 2);
        // The widget should be removed from children
        assert!(!tb.children().contains(&widget_id_1()), "child widget should be removed");
    }

    #[test]
    fn toolbox_remove_item_out_of_bounds_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.remove_item(5); // out of bounds
        assert_eq!(tb.count(), 1);
    }

    #[test]
    fn toolbox_remove_item_adjusts_current_index() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        tb.set_current_index(2);

        // Remove item at index 0 (< current_index), current decrements to 1
        tb.remove_item(0);
        assert_eq!(tb.current_index(), 1);

        // Removing last item leaves empty -> current_index resets to 0
        tb.remove_item(1);
        tb.remove_item(0);
        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), 0);
    }

    #[test]
    fn toolbox_remove_item_above_current_index_does_not_change_it() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(0);
        // Remove item at index 1 (> current_index 0), current stays 0
        tb.remove_item(1);
        assert_eq!(tb.current_index(), 0);
    }

    // ── 6. Clear all items ────────────────────────────────────────────────

    #[test]
    fn toolbox_clear_all_items() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), Some(widget_id_1()));
        tb.add_item("B".to_string(), Some(widget_id_2()));
        tb.add_item("C".to_string(), Some(widget_id_3()));

        // Remove all items one by one (no dedicated clear() method)
        tb.remove_item(0);
        tb.remove_item(0);
        tb.remove_item(0);

        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), 0);
        assert!(tb.children().is_empty(), "all child widgets should be removed");
        assert_eq!(tb.current_widget(), None);
    }

    // ── 7. Setting/getting item text/tooltip ──────────────────────────────

    #[test]
    fn toolbox_item_text_and_tooltip() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("Hello".to_string(), None);
        tb.add_item("World".to_string(), None);

        if let Some(item) = tb.item(0) {
            assert_eq!(item.text(), "Hello");
            assert_eq!(item.tooltip(), "");
        }

        // Modify item via item_mut
        if let Some(item) = tb.item_mut(1) {
            item.set_text("Modified".to_string());
            item.set_tooltip("Tooltip text".to_string());
        }

        if let Some(item) = tb.item(1) {
            assert_eq!(item.text(), "Modified");
            assert_eq!(item.tooltip(), "Tooltip text");
        }

        // Getting item at out-of-bounds index
        assert!(tb.item(5).is_none());
        assert!(tb.item_mut(5).is_none());
    }

    // ── 8. Geometry delegation ────────────────────────────────────────────

    #[test]
    fn toolbox_geometry_delegation() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.geometry(), Rect::new(0, 0, 200, 160));

        tb.set_geometry(Rect::new(10, 20, 300, 200));
        assert_eq!(tb.geometry(), Rect::new(10, 20, 300, 200));
        assert_eq!(tb.base().geometry(), tb.geometry());
    }

    #[test]
    fn toolbox_position_and_size() {
        let mut tb = ToolBox::new(Rect::new(5, 10, 200, 150));
        assert_eq!(tb.position(), Point::new(5, 10));
        assert_eq!(tb.size(), crate::core::Size::new(200, 150));

        tb.set_position(Point::new(20, 30));
        assert_eq!(tb.geometry(), Rect::new(20, 30, 200, 150));

        tb.set_size(crate::core::Size::new(300, 200));
        assert_eq!(tb.geometry(), Rect::new(20, 30, 300, 200));
    }

    // ── 9. Visibility ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_visibility() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert!(tb.is_visible());

        tb.hide();
        assert!(!tb.is_visible());

        tb.show();
        assert!(tb.is_visible());

        tb.set_visible(false);
        assert!(!tb.is_visible());

        tb.set_visible(true);
        assert!(tb.is_visible());
    }

    #[test]
    fn toolbox_enabled_state() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert!(tb.is_enabled());

        tb.set_enabled(false);
        assert!(!tb.is_enabled());

        tb.set_enabled(true);
        assert!(tb.is_enabled());
    }

    // ── 10. ID / Kind ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_kind() {
        let tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.kind(), WidgetKind::Toolbox);
    }

    #[test]
    fn toolbox_id_is_unique() {
        let tb1 = ToolBox::new(Rect::new(0, 0, 100, 100));
        let tb2 = ToolBox::new(Rect::new(0, 0, 100, 100));
        assert_ne!(tb1.id(), tb2.id(), "each ToolBox must have a unique id");
    }

    // ── 11. SVG draw output ───────────────────────────────────────────────

    #[test]
    fn toolbox_draw_produces_svg_output() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 300, 160));
        tb.add_item("Item1".to_string(), None);
        tb.add_item("Item2".to_string(), None);
        let svg = render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"), "SVG must start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG must end with </svg>");
        assert!(svg.contains("width=\"300\""), "SVG must contain correct width");
        assert!(svg.contains("height=\"160\""), "SVG must contain correct height");
        assert!(svg.contains("fill="), "SVG should contain fill attributes");
        assert!(svg.len() > 100, "SVG output should be substantial");
    }

    #[test]
    fn toolbox_draw_empty_produces_svg() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 100, 50));
        let svg = render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 50);
    }

    // ── 12. Signal accessors ──────────────────────────────────────────────

    #[test]
    fn toolbox_current_changed_signal_emits_on_set() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);

        let emitted = Arc::new(AtomicUsize::new(0));
        let e = emitted.clone();
        tb.current_changed.connect(move |idx: Arc<usize>| {
            e.store(*idx, Ordering::SeqCst);
        });

        tb.set_current_index(1);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);

        tb.set_current_index(2);
        assert_eq!(emitted.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn toolbox_current_changed_does_not_emit_for_same_value() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);

        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        tb.current_changed.connect(move |_: Arc<usize>| {
            h.fetch_add(1, Ordering::SeqCst);
        });

        // Set to same index (0) — should not emit
        tb.set_current_index(0);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    // ── 13. Mouse click selects item ──────────────────────────────────────

    #[test]
    fn toolbox_mouse_click_selects_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("First".to_string(), None);
        tb.add_item("Second".to_string(), None);

        assert_eq!(tb.current_index(), 0);

        // Default orientation is Vertical; item_rect(1) has y = 0 + 32*1 = 32
        // Click on item 1 at (10, 40) which is within vertical item 1: (0, 32, 200, 32)
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 1, "clicking item should select it");
    }

    #[test]
    fn toolbox_mouse_click_on_disabled_item_does_not_select() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        // Disable item 1
        if let Some(item) = tb.item_mut(1) {
            item.set_enabled(false);
        }

        assert_eq!(tb.current_index(), 0);
        // Click on item 1 (y=32), should not select because item is disabled
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 0, "clicking disabled item should not change current");
    }

    #[test]
    fn toolbox_mouse_click_on_empty_region_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        assert_eq!(tb.current_index(), 0);

        // Click far below items (y=200, but geometry only goes to 160)
        // Since item_at_position returns None for out-of-bounds, no change
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 200), button: 1 });
        assert_eq!(tb.current_index(), 0);
    }

    // ── 14. Disabled state blocks events ──────────────────────────────────

    #[test]
    fn toolbox_disabled_state_blocks_mouse_events() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(0);

        // Disable the entire toolbox
        tb.set_enabled(false);

        // Try clicking on item 1
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        // current_index should remain 0 because event handling returns early when disabled
        assert_eq!(tb.current_index(), 0, "disabled toolbox should not process mouse events");
    }

    #[test]
    fn toolbox_disabled_re_enable_processes_events() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        tb.set_enabled(false);
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 0);

        // Re-enable and click again
        tb.set_enabled(true);
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 1, "after re-enable, mouse clicks should work");
    }
}
