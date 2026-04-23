//! Combo box widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
/// Combo box widget.
pub struct ComboBox {
    base: BaseWidget,
    items: Vec<String>,
    current_index: Option<usize>,
    editable: bool,
    max_visible_items: usize,
    pub current_index_changed: Signal1<Option<usize>>,
    pub current_text_changed: Signal1<String>,
    pub activated: Signal1<usize>,
}
impl ComboBox {
    /// Creates an empty combo box with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ComboBox, geometry, "ComboBox"),
            items: Vec::new(),
            current_index: None,
            editable: false,
            max_visible_items: 10,
            current_index_changed: Signal1::new(),
            current_text_changed: Signal1::new(),
            activated: Signal1::new(),
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns whether the combo box is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Returns item at specified index.
    pub fn item(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|s| s.as_str())
    }
    /// Adds an item.
    pub fn add_item(&mut self, text: String) {
        self.items.push(text);
    }
    /// Adds multiple items.
    pub fn add_items(&mut self, items: Vec<String>) {
        self.items.extend(items);
    }
    /// Inserts an item at specified position.
    pub fn insert_item(&mut self, index: usize, text: String) {
        if index <= self.items.len() {
            self.items.insert(index, text);
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
                if index <= *current {
                    *current += 1;
                }
            }
        }
    }
    /// Removes item at specified index.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
                if index == *current {
                    self.current_index = None;
                    self.current_text_changed.emit(String::new());
                    self.current_index_changed.emit(None);
                } else if index < *current {
                    *current -= 1;
                }
            }
        }
    }
    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        self.current_text_changed.emit(String::new());
        self.current_index_changed.emit(None);
    }
    /// Returns current index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    /// Sets current index.
    pub fn set_current_index(&mut self, index: Option<usize>) {
        if index == self.current_index {
            return;
        }
        if let Some(idx) = index {
            if idx < self.items.len() {
                self.current_index = Some(idx);
                self.current_text_changed.emit(self.items[idx].clone());
                self.current_index_changed.emit(Some(idx));
            }
        } else {
            self.current_index = None;
            self.current_text_changed.emit(String::new());
            self.current_index_changed.emit(None);
        }
    }
    /// Returns current text.
    pub fn current_text(&self) -> String {
        self.current_index
            .and_then(|idx| self.items.get(idx))
            .cloned()
            .unwrap_or_default()
    }
    /// Sets current text (for editable combo boxes).
    pub fn set_current_text(&mut self, text: String) {
        if !self.editable {
            return;
        }
        // Find matching item
        let index = self.items.iter().position(|item| item == &text);
        self.set_current_index(index);
        // For editable combo boxes, we might want to add the text if not found
        if index.is_none() && !text.is_empty() {
            // In a real implementation, we might add it or keep it as custom text
        }
    }
    /// Returns whether the combo box is editable.
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    /// Sets editable state.
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
    }
    /// Returns maximum number of visible items in dropdown.
    pub fn max_visible_items(&self) -> usize {
        self.max_visible_items
    }
    /// Sets maximum number of visible items in dropdown.
    pub fn set_max_visible_items(&mut self, max: usize) {
        self.max_visible_items = max.max(1);
    }
    /// Finds index of item with specified text.
    pub fn find_text(&self, text: &str) -> Option<usize> {
        self.items.iter().position(|item| item == text)
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }
}
// Implement Widget trait
impl Widget for ComboBox {
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
impl EventHandler for ComboBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    // Toggle dropdown (in real implementation)
                    self.base.clicked.emit();
                    // Simulate selection for demo
                    if !self.items.is_empty() {
                        let new_index = if let Some(current) = self.current_index {
                            (current + 1) % self.items.len()
                        } else {
                            0
                        };
                        self.set_current_index(Some(new_index));
                        self.activated.emit(new_index);
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    38 => {
                        // Up arrow - previous item
                        if let Some(current) = self.current_index {
                            if current > 0 {
                                self.set_current_index(Some(current - 1));
                                self.activated.emit(current - 1);
                            }
                        } else if !self.items.is_empty() {
                            self.set_current_index(Some(self.items.len() - 1));
                            self.activated.emit(self.items.len() - 1);
                        }
                    }
                    40 => {
                        // Down arrow - next item
                        if let Some(current) = self.current_index {
                            if current < self.items.len() - 1 {
                                self.set_current_index(Some(current + 1));
                                self.activated.emit(current + 1);
                            }
                        } else if !self.items.is_empty() {
                            self.set_current_index(Some(0));
                            self.activated.emit(0);
                        }
                    }
                    13 => {
                        // Enter - activate current item
                        if let Some(current) = self.current_index {
                            self.activated.emit(current);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl Draw for ComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y + rect.height as i32 / 2;
        // Draw background
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(255, 255, 255),
        );
        // Draw border
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );
        // Draw dropdown arrow
        let arrow_size = 8;
        let arrow_x = rect.x + rect.width as i32 - padding - arrow_size;
        let arrow_y = rect.y + rect.height as i32 / 2;
        // Draw arrow (triangle)
        context.draw_line(Point::new(arrow_x, arrow_y - arrow_size / 2), Point::new(arrow_x + arrow_size, arrow_y - arrow_size / 2), Color::from_rgb(100, 100, 100),
        );
        context.draw_line(Point::new(arrow_x + arrow_size, arrow_y - arrow_size / 2), Point::new(arrow_x + arrow_size / 2, arrow_y + arrow_size / 2), Color::from_rgb(100, 100, 100),
        );
        context.draw_line(Point::new(arrow_x + arrow_size / 2, arrow_y + arrow_size / 2), Point::new(arrow_x, arrow_y - arrow_size / 2), Color::from_rgb(100, 100, 100),
        );
        // Draw current text
        let current_text = self.current_text();
        if !current_text.is_empty() {
            context.draw_text(
                text_x,
                text_y,
                &current_text,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
                Alignment::Left,
            );
        } else if self.items.is_empty() {
            // Draw placeholder
            context.draw_text(
                text_x,
                text_y,
                "(Empty)",
                &Font::default(),
                Color::from_rgb(150, 150, 150),
                Alignment::Left,
            );
        }
    }
}
