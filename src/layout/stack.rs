//! Stack layout manager — shows one child page at a time.
use crate::core::{ObjectId, Rect};
use super::Layout;
/// Stack layout that shows one child page at a time.
pub struct StackLayout {
    items: Vec<ObjectId>,
    current: usize,
}
impl StackLayout {
    /// Create stack layout with no pages.
    pub fn new() -> Self { Self { items: Vec::new(), current: 0 } }
    /// Select visible page by index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.items.len() { self.current = index; }
    }
}
impl Default for StackLayout { fn default() -> Self { Self::new() } }
impl Layout for StackLayout {
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) { self.items.push(widget_id); }
    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.items.retain(|id| *id != widget_id);
        if self.current >= self.items.len() { self.current = self.items.len().saturating_sub(1); }
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if let Some(widget_id) = self.items.get(self.current) { widgets(*widget_id, rect); }
    }
}
