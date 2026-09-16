// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Per-widget dirty state tracking.
use crate::core::{ObjectId, Rect};
use std::collections::{HashMap, HashSet};
/// Tracks dirty/clean state for individual widgets.
///
/// Each dirty widget remembers the rectangle it was marked with, so a redraw
/// pass can repaint only the affected regions. The rect is whatever the caller
/// supplied — it is not intersected with the widget's previous or current
/// geometry — so a caller that moves a widget is responsible for dirtying the
/// union of the old and new areas.
///
#[derive(Debug)]
pub struct WidgetDirtyState {
    dirty_widgets: HashSet<ObjectId>,
    dirty_rects: HashMap<ObjectId, Rect>,
}
impl WidgetDirtyState {
    /// Creates a state with nothing marked dirty.
    pub fn new() -> Self {
        Self { dirty_widgets: HashSet::new(), dirty_rects: HashMap::new() }
    }
    /// Marks a widget dirty and records the region needing repaint.
    ///
    /// The `rect` is in the same coordinate space the renderer expects for
    /// damage regions. Re-marking an already-dirty widget **replaces** its rect
    /// rather than unioning with it, so calling this repeatedly with a shrinking
    /// rect can leave earlier damage unpainted.
    pub fn mark_dirty(&mut self, id: ObjectId, rect: Rect) {
        self.dirty_widgets.insert(id);
        self.dirty_rects.insert(id, rect);
    }
    /// Clears the dirty flag and drops the remembered rect. No-op for an id
    /// that is not dirty.
    pub fn mark_clean(&mut self, id: ObjectId) {
        self.dirty_widgets.remove(&id);
        self.dirty_rects.remove(&id);
    }
    /// Returns whether `id` is currently dirty.
    pub fn is_dirty(&self, id: ObjectId) -> bool {
        self.dirty_widgets.contains(&id)
    }
    /// Returns the region recorded for `id`, or `None` when it is not dirty.
    pub fn get_dirty_rect(&self, id: ObjectId) -> Option<&Rect> {
        self.dirty_rects.get(&id)
    }
    /// Returns every dirty widget id. Iteration order is unspecified.
    pub fn dirty_widgets(&self) -> &HashSet<ObjectId> {
        &self.dirty_widgets
    }
    /// Discards all dirty state, as if every widget had been marked clean.
    pub fn clear(&mut self) {
        self.dirty_widgets.clear();
        self.dirty_rects.clear();
    }
    /// Returns `true` when no widget is dirty.
    pub fn is_empty(&self) -> bool {
        self.dirty_widgets.is_empty()
    }
    /// Returns how many widgets are dirty. `mark_dirty` on an already-dirty
    /// widget does not increase this.
    pub fn len(&self) -> usize {
        self.dirty_widgets.len()
    }
    /// Returns the dirty regions, one per dirty widget, in unspecified order.
    ///
    /// The rectangles are independent and may overlap or be adjacent; a caller
    /// wanting a minimal repaint set must merge them itself.
    pub fn get_all_rects(&self) -> Vec<Rect> {
        self.dirty_rects.values().copied().collect()
    }
}
crate::impl_default_via_new!(WidgetDirtyState);
