//! Dirty region tracking for incremental rendering.
use crate::core::rect_merge::{bounding_rect, merge_intersecting_rects};
use crate::core::Rect;
/// Unique identifier for a dirty region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(u64);
impl RegionId {
    pub fn new() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
    }
}
impl Default for RegionId {
    fn default() -> Self {
        Self::new()
    }
}
/// A rectangular region that needs to be re-rendered.
#[derive(Debug, Clone)]
pub struct DirtyRegion {
    pub id: RegionId,
    pub rect: Rect,
    pub priority: u8,
    pub layer: u32,
}
impl DirtyRegion {
    pub fn new(rect: Rect) -> Self {
        Self { id: RegionId::new(), rect, priority: 0, layer: 0 }
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
/// Tracks dirty regions, supports merging and optimization.
#[derive(Debug)]
pub struct DirtyRegionTracker {
    pub(crate) regions: Vec<DirtyRegion>,
    merged: bool,
    max_regions: usize,
}
impl DirtyRegionTracker {
    pub fn new() -> Self {
        Self { regions: Vec::new(), merged: false, max_regions: 100 }
    }
    pub fn with_max_regions(max_regions: usize) -> Self {
        Self { regions: Vec::new(), merged: false, max_regions }
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
        let rects: Vec<Rect> = self.regions.iter().map(|r| r.rect).collect();
        self.regions = merge_intersecting_rects(&rects).into_iter().map(DirtyRegion::new).collect();
        self.merged = true;
    }
    pub fn get_bounding_rect(&self) -> Option<Rect> {
        let rects: Vec<Rect> = self.regions.iter().map(|r| r.rect).collect();
        bounding_rect(&rects)
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
                self.regions.sort_by_key(|b| std::cmp::Reverse(b.priority));
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
