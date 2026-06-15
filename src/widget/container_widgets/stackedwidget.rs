//! Stacked widget.
use crate::core::{Color, ObjectId, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// Stacked widget.
pub struct StackedWidget {
    base: BaseWidget,
    widgets: Vec<ObjectId>,
    current_index: usize,
    pub current_changed: Signal1<usize>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
impl StackedWidget {
    /// Creates a stacked widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StackedWidget, geometry, "StackedWidget"),
            widgets: Vec::new(),
            current_index: 0,
            current_changed: Signal1::new(),
            registry: None,
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
    /// Adds a widget.
    pub fn add_widget(&mut self, widget: ObjectId) -> usize {
        self.base.add_child(widget);
        self.widgets.push(widget);
        self.widgets.len().saturating_sub(1)
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

    /// Returns the number of widgets (alias for count).
    pub fn widget_count(&self) -> usize {
        self.widgets.len()
    }

    /// Sets the current widget by its ObjectId.
    pub fn set_current_widget(&mut self, id: ObjectId) {
        if let Some(index) = self.widgets.iter().position(|&wid| wid == id) {
            self.set_current_index(index);
        }
    }
    /// Returns current widget index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current widget index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.widgets.len() && self.current_index != index {
            self.current_index = index;
            self.base.request_redraw();
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
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for StackedWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Forward events to current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for StackedWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        // Draw background
        context.fill_rect(rect, Color::rgb(255, 255, 255));
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
    use crate::core::{Point, Rect, Size};

    /// Helper to create unique test ObjectIds by wrapping simple constants.
    fn widget_id_1() -> ObjectId {
        1001
    }
    fn widget_id_2() -> ObjectId {
        1002
    }
    fn widget_id_3() -> ObjectId {
        1003
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────

    #[test]
    fn stacked_widget_creation_defaults() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.widget_count(), 0);
        assert_eq!(sw.current_index(), 0);
        assert_eq!(sw.current_widget(), None);
        assert!(sw.is_visible());
        assert!(sw.is_enabled());
        assert_eq!(sw.geometry(), Rect::new(0, 0, 300, 200));
        assert!(sw.children().is_empty());
        assert!(sw.registry().is_none());
    }

    #[test]
    fn stacked_widget_empty_current_widget_is_none() {
        let sw = StackedWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(sw.current_widget(), None);
        // Even though current_index is 0, an empty widgets vec means no current widget.
        assert_eq!(sw.current_index(), 0);
        assert_eq!(sw.widget(0), None);
        assert_eq!(sw.widget(1), None);
    }

    #[test]
    fn stacked_widget_empty_index_of_returns_none() {
        let sw = StackedWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(sw.index_of(widget_id_1()), None);
    }

    // ── 2. Adding pages (add_widget) ──────────────────────────────────────

    #[test]
    fn stacked_widget_add_widget_returns_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        let idx = sw.add_widget(widget_id_1());
        assert_eq!(idx, 0);
        assert_eq!(sw.count(), 1);
    }

    #[test]
    fn stacked_widget_add_multiple_widgets() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        assert_eq!(sw.count(), 3);
        assert_eq!(sw.widget(0), Some(widget_id_1()));
        assert_eq!(sw.widget(1), Some(widget_id_2()));
        assert_eq!(sw.widget(2), Some(widget_id_3()));
    }

    #[test]
    fn stacked_widget_add_widget_updates_children() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        let children = sw.children();
        assert_eq!(children.len(), 2);
        assert!(children.contains(&widget_id_1()));
        assert!(children.contains(&widget_id_2()));
    }

    #[test]
    fn stacked_widget_current_index_is_zero_after_first_add() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        // current_index stays 0 after add
        assert_eq!(sw.current_index(), 0);
        assert_eq!(sw.current_widget(), Some(widget_id_1()));
    }

    // ── 3. Setting current index ──────────────────────────────────────────

    #[test]
    fn stacked_widget_set_current_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(1);
        assert_eq!(sw.current_index(), 1);
        assert_eq!(sw.current_widget(), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_set_current_index_out_of_bounds_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.set_current_index(5); // out of bounds
        assert_eq!(sw.current_index(), 0);
    }

    #[test]
    fn stacked_widget_set_current_index_same_value_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(0); // already 0
        assert_eq!(sw.current_index(), 0);
    }

    #[test]
    fn stacked_widget_set_current_index_first_tab() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_index(2);
        sw.set_current_index(0);
        assert_eq!(sw.current_index(), 0);
        assert_eq!(sw.current_widget(), Some(widget_id_1()));
    }

    #[test]
    fn stacked_widget_set_current_widget_by_id() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_widget(widget_id_3());
        assert_eq!(sw.current_index(), 2);
        assert_eq!(sw.current_widget(), Some(widget_id_3()));
    }

    #[test]
    fn stacked_widget_set_current_widget_unknown_id_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.set_current_widget(9999); // unknown id
        assert_eq!(sw.current_index(), 0);
    }

    // ── 4. Page count ─────────────────────────────────────────────────────

    #[test]
    fn stacked_widget_count_after_add_and_remove() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.widget_count(), 0);
        sw.add_widget(widget_id_1());
        assert_eq!(sw.count(), 1);
        assert_eq!(sw.widget_count(), 1);
        sw.add_widget(widget_id_2());
        assert_eq!(sw.count(), 2);
        assert_eq!(sw.widget_count(), 2);
    }

    // ── 5. Removing pages ─────────────────────────────────────────────────

    #[test]
    fn stacked_widget_remove_widget() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());

        sw.remove_widget(widget_id_2());
        assert_eq!(sw.count(), 2);
        assert_eq!(sw.widget(0), Some(widget_id_1()));
        assert_eq!(sw.widget(1), Some(widget_id_3()));
    }

    #[test]
    fn stacked_widget_remove_widget_updates_children() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.remove_widget(widget_id_1());
        let children = sw.children();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], widget_id_2());
    }

    #[test]
    fn stacked_widget_remove_widget_nonexistent_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.remove_widget(9999); // not present
        assert_eq!(sw.count(), 1);
    }

    #[test]
    fn stacked_widget_remove_widget_adjusts_current_index_when_below() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_index(2); // last widget
                                 // Remove first widget (index 0 < current_index 2)
        sw.remove_widget(widget_id_1());
        // current_index should decrement from 2 to 1
        assert_eq!(sw.current_index(), 1);
        assert_eq!(sw.current_widget(), Some(widget_id_3()));
    }

    #[test]
    fn stacked_widget_remove_widget_above_current_index_does_not_change_it() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_index(0);
        // Remove last widget (index 2 > current_index 0)
        sw.remove_widget(widget_id_3());
        assert_eq!(sw.current_index(), 0);
    }

    #[test]
    fn stacked_widget_remove_all_widgets_resets_current_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(1);
        sw.remove_widget(widget_id_1());
        sw.remove_widget(widget_id_2());
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.current_index(), 0);
        assert_eq!(sw.current_widget(), None);
    }

    // ── 6. Insert widget ──────────────────────────────────────────────────

    #[test]
    fn stacked_widget_insert_widget_at_beginning() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.insert_widget(0, widget_id_3());
        assert_eq!(sw.count(), 3);
        assert_eq!(sw.widget(0), Some(widget_id_3()));
        assert_eq!(sw.widget(1), Some(widget_id_1()));
        assert_eq!(sw.widget(2), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_insert_widget_shifts_current_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(1); // currently at widget_id_2
                                 // Insert at index 0, current_index (1) >= 0, so it increments to 2
        sw.insert_widget(0, widget_id_3());
        assert_eq!(sw.current_index(), 2);
        assert_eq!(sw.current_widget(), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_insert_widget_after_current_index_no_change() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(0);
        // Insert at end (> current_index), current_index stays 0
        sw.insert_widget(2, widget_id_3());
        assert_eq!(sw.current_index(), 0);
    }

    #[test]
    fn stacked_widget_insert_widget_updates_children() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.insert_widget(0, widget_id_2());
        let children = sw.children();
        // Both widget_id_1 and widget_id_2 should be children
        assert_eq!(children.len(), 2);
        assert!(children.contains(&widget_id_1()));
        assert!(children.contains(&widget_id_2()));
    }

    // ── 7. Clear (removing all widgets) ───────────────────────────────────

    #[test]
    fn stacked_widget_index_of_after_operations() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        assert_eq!(sw.index_of(widget_id_1()), Some(0));
        assert_eq!(sw.index_of(widget_id_2()), Some(1));
        assert_eq!(sw.index_of(widget_id_3()), Some(2));
        assert_eq!(sw.index_of(9999), None);

        // After removal, indices shift
        sw.remove_widget(widget_id_2());
        assert_eq!(sw.index_of(widget_id_3()), Some(1));
    }

    // ── 8. Getting current widget / page ──────────────────────────────────

    #[test]
    fn stacked_widget_current_widget_after_add_and_set() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        // Initially no current widget
        assert_eq!(sw.current_widget(), None);

        sw.add_widget(widget_id_1());
        assert_eq!(sw.current_widget(), Some(widget_id_1()));

        sw.add_widget(widget_id_2());
        // Still on first widget
        assert_eq!(sw.current_widget(), Some(widget_id_1()));

        sw.set_current_index(1);
        assert_eq!(sw.current_widget(), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_widget_at_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        assert_eq!(sw.widget(0), Some(widget_id_1()));
        assert_eq!(sw.widget(1), Some(widget_id_2()));
        assert_eq!(sw.widget(2), None);
    }

    // ── 9. Geometry delegation ────────────────────────────────────────────

    #[test]
    fn stacked_widget_geometry_delegation() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.geometry(), Rect::new(0, 0, 300, 200));

        sw.set_geometry(Rect::new(10, 20, 400, 300));
        assert_eq!(sw.geometry(), Rect::new(10, 20, 400, 300));
    }

    #[test]
    fn stacked_widget_geometry_position_and_size() {
        let mut sw = StackedWidget::new(Rect::new(5, 10, 200, 150));
        assert_eq!(sw.position(), Point::new(5, 10));
        assert_eq!(sw.size(), Size::new(200, 150));

        sw.set_position(Point::new(20, 30));
        assert_eq!(sw.geometry(), Rect::new(20, 30, 200, 150));

        sw.set_size(Size::new(300, 200));
        assert_eq!(sw.geometry(), Rect::new(20, 30, 300, 200));
    }

    // ── 10. Visibility ────────────────────────────────────────────────────

    #[test]
    fn stacked_widget_visibility() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert!(sw.is_visible());

        sw.hide();
        assert!(!sw.is_visible());

        sw.show();
        assert!(sw.is_visible());

        sw.set_visible(false);
        assert!(!sw.is_visible());

        sw.set_visible(true);
        assert!(sw.is_visible());
    }

    #[test]
    fn stacked_widget_enabled_state() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert!(sw.is_enabled());

        sw.set_enabled(false);
        assert!(!sw.is_enabled());

        sw.set_enabled(true);
        assert!(sw.is_enabled());
    }

    // ── 11. ID / Kind ─────────────────────────────────────────────────────

    #[test]
    fn stacked_widget_kind() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.kind(), WidgetKind::StackedWidget);
    }

    #[test]
    fn stacked_widget_id_is_unique() {
        let sw1 = StackedWidget::new(Rect::new(0, 0, 100, 100));
        let sw2 = StackedWidget::new(Rect::new(0, 0, 100, 100));
        assert_ne!(sw1.id(), sw2.id(), "Each StackedWidget must have a unique id");
    }

    #[test]
    fn stacked_widget_class_name() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.accessible_name(), "StackedWidget");
    }

    // ── 12. SVG draw output ───────────────────────────────────────────────

    #[test]
    fn stacked_widget_draw_produces_svg_output() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        let svg = crate::widget::svg::render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"200\""));
        assert!(svg.contains("fill=")); // white background fill
        assert!(svg.len() > 50);
    }

    #[test]
    fn stacked_widget_draw_with_registry_produces_svg() {
        use crate::widget::SimpleRegistry;
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        sw.set_registry(reg.clone());

        let child_id = 7777;
        sw.add_widget(child_id);

        // Register a draw closure that draws a red rect
        reg.borrow_mut().register(
            child_id,
            |ctx| {
                ctx.fill_rect(Rect::new(10, 10, 50, 50), crate::core::Color::RED);
            },
            |_| {},
        );

        let svg = crate::widget::svg::render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"300\""));
        // Both the white background from StackedWidget and the red rect from the child
        assert!(svg.contains("fill="));
        assert!(svg.len() > 100);
    }

    // ── 13. Signal accessors ────────────────────────────────────────────────

    #[test]
    fn stacked_widget_current_changed_signal_emits_on_set() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());

        let emitted = Arc::new(AtomicUsize::new(0));
        let e = emitted.clone();
        sw.current_changed.connect(move |idx| {
            e.store(*idx, Ordering::SeqCst);
        });

        sw.set_current_index(1);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);

        sw.set_current_index(2);
        assert_eq!(emitted.load(Ordering::SeqCst), 2);

        // Setting to same value does NOT emit
        sw.set_current_index(2);
        assert_eq!(emitted.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn stacked_widget_current_changed_not_emitted_for_out_of_bounds() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());

        let emitted = Arc::new(AtomicUsize::new(0));
        let e = emitted.clone();
        sw.current_changed.connect(move |idx| {
            e.store(*idx, Ordering::SeqCst);
        });

        // Out of bounds - should not emit
        sw.set_current_index(10);
        assert_eq!(emitted.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn stacked_widget_base_signals_accessible() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        // These are all delegated through BaseWidget
        let _clicked = sw.clicked_signal();
        let _changed = sw.changed_signal();
        let _hover = sw.hover_signal();
        let _mouse_down = sw.mouse_down_signal();
        let _mouse_up = sw.mouse_up_signal();
        let _key_down = sw.key_down_signal();
        let _key_up = sw.key_up_signal();
        let _focus_gained = sw.focus_gained_signal();
        let _focus_lost = sw.focus_lost_signal();
        let _redraw = sw.redraw_requested_signal();
        let _layout = sw.layout_requested_signal();
        // Smoke test signals are properly connected
        _redraw.emit();
        _layout.emit();
        _changed.emit();
        _clicked.emit();
    }

    // ── 14. Registry ───────────────────────────────────────────────────────

    #[test]
    fn stacked_widget_registry_delegation() {
        use crate::widget::SimpleRegistry;
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert!(sw.registry().is_none());

        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        sw.set_registry(reg.clone());

        assert!(sw.registry().is_some());
        // Verify it's the same registry via pointer identity
        assert!(Rc::ptr_eq(sw.registry().unwrap(), &reg));
    }

    #[test]
    fn stacked_widget_event_forwards_to_current_widget() {
        use crate::event::Event;
        use crate::widget::SimpleRegistry;
        use std::cell::RefCell;
        use std::rc::Rc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        sw.set_registry(reg.clone());

        let child_id = 42;
        sw.add_widget(child_id);

        let received = Arc::new(AtomicBool::new(false));
        let r = received.clone();
        reg.borrow_mut().register(
            child_id,
            |_| {},
            move |_| {
                r.store(true, Ordering::SeqCst);
            },
        );

        sw.handle_event(&Event::timer(0));
        assert!(
            received.load(Ordering::SeqCst),
            "Event should be forwarded to current child widget"
        );
    }

    #[test]
    fn stacked_widget_event_noop_without_registry() {
        use crate::event::Event;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        // Should not panic
        sw.handle_event(&Event::timer(0));
    }

    #[test]
    fn stacked_widget_draw_noop_without_registry() {
        use crate::render::PaintBackend;
        use crate::render::RenderContext;
        use crate::render::SvgPaintBackend;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());

        // draw without registry should not panic
        let size = Size::new(300, 200);
        let mut backend = SvgPaintBackend::new(size);
        backend.begin_frame(crate::core::Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut backend);
            sw.draw(&mut ctx);
        }
        backend.end_frame();
        let svg = backend.finish();
        assert!(svg.starts_with("<svg"));
    }
}
