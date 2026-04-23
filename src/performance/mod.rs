mod profiler;
pub use profiler::*;
use crate::core::Rect;
use std::collections::{HashMap, HashSet};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(u64);
impl RegionId {
    pub fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}
impl Default for RegionId {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone)]
pub struct DirtyRegion {
    pub id: RegionId,
    pub rect: Rect,
    pub priority: u8,
    pub layer: u32,
}
impl DirtyRegion {
    pub fn new(rect: Rect) -> Self {
        Self {
            id: RegionId::new(),
            rect,
            priority: 0,
            layer: 0,
        }
    }
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
    pub fn with_layer(mut self, layer: u32) -> Self {
        self.layer = layer;
        self
    }
    pub fn intersects(&self, other: &Rect) -> bool {
        self.rect.intersects(other)
    }
    pub fn contains(&self, other: &Rect) -> bool {
        self.rect.contains_rect(other)
    }
}
pub struct DirtyRegionTracker {
    regions: Vec<DirtyRegion>,
    merged: bool,
    max_regions: usize,
}
impl DirtyRegionTracker {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            merged: false,
            max_regions: 100,
        }
    }
    pub fn with_max_regions(max_regions: usize) -> Self {
        Self {
            regions: Vec::new(),
            merged: false,
            max_regions,
        }
    }
    pub fn add(&mut self, rect: Rect) -> RegionId {
        let region = DirtyRegion::new(rect);
        let id = region.id;
        self.regions.push(region);
        self.merged = false;
        id
    }
    pub fn add_with_priority(&mut self, rect: Rect, priority: u8) -> RegionId {
        let region = DirtyRegion::new(rect).with_priority(priority);
        let id = region.id;
        self.regions.push(region);
        self.merged = false;
        id
    }
    pub fn add_with_layer(&mut self, rect: Rect, layer: u32) -> RegionId {
        let region = DirtyRegion::new(rect).with_layer(layer);
        let id = region.id;
        self.regions.push(region);
        self.merged = false;
        id
    }
    pub fn remove(&mut self, id: RegionId) -> bool {
        let len = self.regions.len();
        self.regions.retain(|r| r.id != id);
        self.regions.len() < len
    }
    pub fn clear(&mut self) {
        self.regions.clear();
        self.merged = false;
    }
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
    pub fn len(&self) -> usize {
        self.regions.len()
    }
    pub fn regions(&self) -> &[DirtyRegion] {
        &self.regions
    }
    pub fn merge(&mut self) {
        if self.merged || self.regions.len() <= 1 {
            return;
        }
        let mut merged = Vec::new();
        let mut used = vec![false; self.regions.len()];
        for (i, region) in self.regions.iter().enumerate() {
            if used[i] {
                continue;
            }
            let mut current_rect = region.rect;
            used[i] = true;
            for (j, other) in self.regions.iter().enumerate() {
                if used[j] {
                    continue;
                }
                if current_rect.intersects(&other.rect) {
                    current_rect = current_rect.union(&other.rect);
                    used[j] = true;
                }
            }
            merged.push(DirtyRegion::new(current_rect));
        }
        self.regions = merged;
        self.merged = true;
    }
    pub fn get_bounding_rect(&self) -> Option<Rect> {
        if self.regions.is_empty() {
            return None;
        }
        let mut result = self.regions[0].rect;
        for region in &self.regions[1..] {
            result = result.union(&region.rect);
        }
        Some(result)
    }
    pub fn get_regions_for_rect(&self, rect: &Rect) -> Vec<&DirtyRegion> {
        self.regions.iter().filter(|r| r.intersects(rect)).collect()
    }
    pub fn clip_to(&mut self, clip_rect: &Rect) {
        self.regions.retain(|r| r.intersects(clip_rect));
        for region in &mut self.regions {
            region.rect = region.rect.intersection(clip_rect).unwrap_or(region.rect);
        }
    }
    pub fn optimize(&mut self) {
        if self.regions.len() > self.max_regions {
            self.merge();
            if self.regions.len() > self.max_regions {
                self.regions.sort_by(|a, b| b.priority.cmp(&a.priority));
                self.regions.truncate(self.max_regions);
            }
        }
    }
}
impl Default for DirtyRegionTracker {
    fn default() -> Self {
        Self::new()
    }
}
pub struct UpdateBatcher {
    pending_updates: Vec<Rect>,
    batch_timeout_ms: u64,
    last_batch: Option<std::time::Instant>,
}
impl UpdateBatcher {
    pub fn new(batch_timeout_ms: u64) -> Self {
        Self {
            pending_updates: Vec::new(),
            batch_timeout_ms,
            last_batch: None,
        }
    }
    pub fn add(&mut self, rect: Rect) {
        self.pending_updates.push(rect);
    }
    pub fn should_flush(&self) -> bool {
        if self.pending_updates.is_empty() {
            return false;
        }
        if let Some(last) = self.last_batch {
            let elapsed = last.elapsed().as_millis() as u64;
            if elapsed >= self.batch_timeout_ms {
                return true;
            }
        }
        self.pending_updates.len() >= 10
    }
    pub fn flush(&mut self) -> Vec<Rect> {
        if self.pending_updates.is_empty() {
            return Vec::new();
        }
        let mut tracker = DirtyRegionTracker::new();
        for rect in self.pending_updates.drain(..) {
            tracker.add(rect);
        }
        tracker.merge();
        self.last_batch = Some(std::time::Instant::now());
        tracker.regions.into_iter().map(|r| r.rect).collect()
    }
    pub fn clear(&mut self) {
        self.pending_updates.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.pending_updates.is_empty()
    }
    pub fn len(&self) -> usize {
        self.pending_updates.len()
    }
}
impl Default for UpdateBatcher {
    fn default() -> Self {
        Self::new(16)
    }
}
pub struct WidgetDirtyState {
    dirty_widgets: HashSet<crate::core::ObjectId>,
    dirty_rects: HashMap<crate::core::ObjectId, Rect>,
}
impl WidgetDirtyState {
    pub fn new() -> Self {
        Self {
            dirty_widgets: HashSet::new(),
            dirty_rects: HashMap::new(),
        }
    }
    pub fn mark_dirty(&mut self, id: crate::core::ObjectId, rect: Rect) {
        self.dirty_widgets.insert(id);
        self.dirty_rects.insert(id, rect);
    }
    pub fn mark_clean(&mut self, id: crate::core::ObjectId) {
        self.dirty_widgets.remove(&id);
        self.dirty_rects.remove(&id);
    }
    pub fn is_dirty(&self, id: crate::core::ObjectId) -> bool {
        self.dirty_widgets.contains(&id)
    }
    pub fn get_dirty_rect(&self, id: crate::core::ObjectId) -> Option<&Rect> {
        self.dirty_rects.get(&id)
    }
    pub fn dirty_widgets(&self) -> &HashSet<crate::core::ObjectId> {
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    #[test]
    fn test_dirty_region_tracker() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.add(Rect::new(0, 0, 100, 100));
        tracker.add(Rect::new(50, 50, 100, 100));
        assert_eq!(tracker.len(), 2);
        tracker.merge();
        assert_eq!(tracker.len(), 1);
        let bounding = tracker.get_bounding_rect().unwrap();
        assert_eq!(bounding.x, 0);
        assert_eq!(bounding.y, 0);
        assert_eq!(bounding.width, 150);
        assert_eq!(bounding.height, 150);
    }
    #[test]
    fn test_update_batcher() {
        let mut batcher = UpdateBatcher::new(100);
        batcher.add(Rect::new(0, 0, 10, 10));
        batcher.add(Rect::new(20, 20, 10, 10));
        assert_eq!(batcher.len(), 2);
        let rects = batcher.flush();
        assert!(!rects.is_empty());
        assert!(batcher.is_empty());
    }
}
