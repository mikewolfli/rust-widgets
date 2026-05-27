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
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
    }
}
