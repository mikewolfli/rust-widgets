//! Stack layout manager — shows one child page at a time.
use super::Layout;
use crate::core::{ObjectId, Rect};
/// Stack layout that shows one child page at a time.
pub struct StackLayout {
    items: Vec<ObjectId>,
    current: usize,
}
impl StackLayout {
    /// Create stack layout with no pages.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current: 0,
        }
    }
    /// Select visible page by index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.items.len() {
            self.current = index;
        }
    }
    /// Returns the number of pages (widgets) in the stack.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    /// Returns the index of the currently visible page.
    pub fn current_index(&self) -> usize {
        self.current
    }
    /// Returns the widget id at the given index, if in bounds.
    pub fn item_at(&self, index: usize) -> Option<ObjectId> {
        self.items.get(index).copied()
    }
}
impl Default for StackLayout {
    fn default() -> Self {
        Self::new()
    }
}
impl Layout for StackLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        self.items.push(widget_id);
    }
    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.items.retain(|id| *id != widget_id);
        if self.current >= self.items.len() {
            self.current = self.items.len().saturating_sub(1);
        }
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if let Some(widget_id) = self.items.get(self.current) {
            widgets(*widget_id, rect);
        }
    }
    fn child_ids(&self) -> Vec<ObjectId> {
        self.items.clone()
    }
    fn has_child(&self, id: ObjectId) -> bool {
        self.items.contains(&id)
    }
    fn clear(&mut self) {
        self.items.clear();
        self.current = 0;
    }
}
