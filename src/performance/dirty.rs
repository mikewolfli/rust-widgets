//! Per-widget dirty state tracking.
use crate::core::{ObjectId, Rect};
use std::collections::{HashMap, HashSet};
/// Tracks dirty/clean state for individual widgets.
pub struct WidgetDirtyState {
    dirty_widgets: HashSet<ObjectId>,
    dirty_rects: HashMap<ObjectId, Rect>,
}
impl WidgetDirtyState {
    pub fn new() -> Self {
        Self {
            dirty_widgets: HashSet::new(),
            dirty_rects: HashMap::new(),
        }
    }
    pub fn mark_dirty(&mut self, id: ObjectId, rect: Rect) {
        self.dirty_widgets.insert(id);
        self.dirty_rects.insert(id, rect);
    }
    pub fn mark_clean(&mut self, id: ObjectId) {
        self.dirty_widgets.remove(&id);
        self.dirty_rects.remove(&id);
    }
    pub fn is_dirty(&self, id: ObjectId) -> bool {
        self.dirty_widgets.contains(&id)
    }
    pub fn get_dirty_rect(&self, id: ObjectId) -> Option<&Rect> {
        self.dirty_rects.get(&id)
    }
    pub fn dirty_widgets(&self) -> &HashSet<ObjectId> {
        &self.dirty_widgets
    }
    pub fn clear(&mut self) {
        self.dirty_widgets.clear();
        self.dirty_rects.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.dirty_widgets.is_empty()
    }
    pub fn len(&self) -> usize {
        self.dirty_widgets.len()
    }
    pub fn get_all_rects(&self) -> Vec<Rect> {
        self.dirty_rects.values().copied().collect()
    }
}
impl Default for WidgetDirtyState {
    fn default() -> Self {
        Self::new()
    }
}
