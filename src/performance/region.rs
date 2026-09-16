// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dirty region tracking for incremental rendering.
use crate::core::rect_merge::{bounding_rect, merge_intersecting_rects};
use crate::core::Rect;
/// Unique identifier for a dirty region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(u64);
impl RegionId {
    /// Allocates a fresh identifier, unique for the lifetime of the process.
    ///
    /// Ids come from a relaxed atomic counter, so they are never reused and
    /// can be safely stored across frames; ordering between ids carries no
    /// meaning.
    pub fn new() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
    }
}
crate::impl_default_via_new!(RegionId);
/// A rectangular region that needs to be re-rendered.
///
/// Rectangles are in the render target's pixel coordinate space: `x`/`y` are
/// relative to the surface origin (top-left), and `width`/`height` extend
/// down-right, matching [`Rect`]'s usual convention.
///
/// `priority` and `layer` are hints used by [`DirtyRegionTracker::optimize`]
/// when the tracker is over capacity; higher values are treated as more
/// important. Both default to `0` in [`DirtyRegion::new`].
#[derive(Debug, Clone)]
pub struct DirtyRegion {
    /// Identity of this region, stable across merging only until
    /// [`DirtyRegionTracker::merge`] replaces the entries.
    pub id: RegionId,
    /// Area to re-render, in surface pixel coordinates.
    pub rect: Rect,
    /// Retention hint in `0..=255`; higher is more likely to survive
    /// [`DirtyRegionTracker::optimize`] truncation. Defaults to `0`.
    pub priority: u8,
    /// Compositing layer this region belongs to; higher layers are assumed to
    /// sit above lower ones and are retained first. Defaults to `0`.
    pub layer: u32,
}
impl DirtyRegion {
    /// Creates a zero-priority, layer-`0` region covering `rect` with a freshly
    /// allocated [`RegionId`].
    pub fn new(rect: Rect) -> Self {
        Self { id: RegionId::new(), rect, priority: 0, layer: 0 }
    }
    /// Builder-style setter for the `0..=255` retention priority.
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
    /// Builder-style setter for the compositing layer index.
    pub fn with_layer(mut self, layer: u32) -> Self {
        self.layer = layer;
        self
    }
    /// Returns `true` when `other` overlaps this region's rectangle.
    ///
    /// Touching edges share no area and therefore do not intersect. `other` is
    /// in the same pixel coordinate space as [`DirtyRegion::rect`].
    pub fn intersects(&self, other: &Rect) -> bool {
        self.rect.intersects(other)
    }
    /// Returns `true` when `other` is fully inside this region's rectangle.
    pub fn contains(&self, other: &Rect) -> bool {
        self.rect.contains_rect(other)
    }
}
/// Tracks dirty regions, supports merging and optimization.
///
/// A *dirty region* is the axis-aligned rectangle of a render target that must
/// be repainted because its content changed. Tracking them lets a renderer
/// redraw only the invalidated pixels instead of the whole surface.
///
/// Regions are stored as an unordered bag rather than a merged set: adding a
/// rectangle never modifies the rectangles already present, so overlaps persist
/// until [`DirtyRegionTracker::merge`] is called. Merging replaces every entry
/// with new ones, and therefore with new [`RegionId`]s, discarding the
/// priorities and layers that were set.
#[derive(Debug)]
pub struct DirtyRegionTracker {
    pub(crate) regions: Vec<DirtyRegion>,
    merged: bool,
    max_regions: usize,
}
impl DirtyRegionTracker {
    /// Creates an empty tracker with a capacity limit of `100` regions.
    ///
    /// The limit only takes effect when [`DirtyRegionTracker::optimize`] is
    /// called; adding regions never evicts anything by itself.
    pub fn new() -> Self {
        Self { regions: Vec::new(), merged: false, max_regions: 100 }
    }
    /// Creates an empty tracker whose [`DirtyRegionTracker::optimize`] limit is
    /// `max_regions`.
    pub fn with_max_regions(max_regions: usize) -> Self {
        Self { regions: Vec::new(), merged: false, max_regions }
    }
    /// Records `rect` as dirty with priority `0` and returns its new id.
    ///
    /// The rectangle is stored as-is, without merging or clipping against
    /// existing regions.
    pub fn add(&mut self, rect: Rect) -> RegionId {
        let region = DirtyRegion::new(rect);
        let id = region.id;
        self.regions.push(region);
        self.merged = false;
        id
    }
    /// Records `rect` as dirty with the given retention `priority` and returns
    /// its new id.
    ///
    /// See [`DirtyRegion::priority`] for the meaning of the value.
    pub fn add_with_priority(&mut self, rect: Rect, priority: u8) -> RegionId {
        let region = DirtyRegion::new(rect).with_priority(priority);
        let id = region.id;
        self.regions.push(region);
        self.merged = false;
        id
    }
    /// Records `rect` as dirty on compositing `layer` and returns its new id.
    ///
    /// See [`DirtyRegion::layer`] for the meaning of the value.
    pub fn add_with_layer(&mut self, rect: Rect, layer: u32) -> RegionId {
        let region = DirtyRegion::new(rect).with_layer(layer);
        let id = region.id;
        self.regions.push(region);
        self.merged = false;
        id
    }
    /// Removes the region with `id`.
    ///
    /// Returns `true` if a region was removed, `false` if the id was unknown
    /// (including ids invalidated by a previous [`DirtyRegionTracker::merge`]).
    pub fn remove(&mut self, id: RegionId) -> bool {
        let len = self.regions.len();
        self.regions.retain(|r| r.id != id);
        self.regions.len() < len
    }
    /// Discards every tracked region, leaving the tracker empty.
    ///
    /// The capacity limit is unaffected. Previously issued [`RegionId`]s become
    /// invalid.
    pub fn clear(&mut self) {
        self.regions.clear();
        self.merged = false;
    }
    /// Returns `true` when no dirty regions are tracked.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
    /// Returns the number of tracked regions.
    ///
    /// This is the count of stored rectangles, not of distinct dirty areas:
    /// overlapping rectangles are counted separately until
    /// [`DirtyRegionTracker::merge`] is called.
    pub fn len(&self) -> usize {
        self.regions.len()
    }
    /// Borrows all tracked regions, in insertion order until a merge reorders
    /// them.
    pub fn regions(&self) -> &[DirtyRegion] {
        &self.regions
    }
    /// Coalesces overlapping rectangles into fewer regions.
    ///
    /// Regions whose rectangles intersect are replaced by their union (see
    /// `merge_intersecting_rects`); disjoint rectangles are left alone, so areas
    /// that merely touch are not merged. The operation is idempotent while no
    /// region is added, and it is skipped entirely when fewer than two regions
    /// are tracked.
    ///
    /// # Side effects
    ///
    /// Existing [`RegionId`]s, priorities, and layers are discarded; every
    /// surviving region is a fresh [`DirtyRegion::new`] with priority `0` and
    /// layer `0`.
    pub fn merge(&mut self) {
        if self.merged || self.regions.len() <= 1 {
            return;
        }
        let rects: Vec<Rect> = self.regions.iter().map(|r| r.rect).collect();
        self.regions = merge_intersecting_rects(&rects).into_iter().map(DirtyRegion::new).collect();
        self.merged = true;
    }
    /// Returns the smallest rectangle enclosing every tracked region, or `None`
    /// when nothing is tracked.
    ///
    /// The bounding rectangle covers regions that are far apart, so it may
    /// include large areas that are not actually dirty. Use
    /// [`DirtyRegionTracker::regions`] when the full extent is not wanted.
    pub fn get_bounding_rect(&self) -> Option<Rect> {
        let rects: Vec<Rect> = self.regions.iter().map(|r| r.rect).collect();
        bounding_rect(&rects)
    }
    /// Returns every tracked region whose rectangle intersects `rect`.
    ///
    /// The result is a borrowed, unordered selection; regions are not clipped
    /// to `rect`.
    pub fn get_regions_for_rect(&self, rect: &Rect) -> Vec<&DirtyRegion> {
        self.regions.iter().filter(|r| r.intersects(rect)).collect()
    }
    /// Restricts all tracked regions to `clip_rect`.
    ///
    /// Regions that do not intersect `clip_rect` are dropped; the rest are
    /// shrunk to their intersection. `clip_rect` is expected to be in the same
    /// pixel coordinate space as the stored rectangles.
    ///
    /// Note that if the intersection is empty (rectangles that merely touch),
    /// the original rectangle is kept unchanged rather than being removed.
    pub fn clip_to(&mut self, clip_rect: &Rect) {
        self.regions.retain(|r| r.intersects(clip_rect));
        for region in &mut self.regions {
            region.rect = region.rect.intersection(clip_rect).unwrap_or(region.rect);
        }
    }
    /// Reduces the tracker to at most `max_regions` regions when it exceeds
    /// that limit.
    ///
    /// Merging is attempted first. If merging alone is not enough, regions are
    /// sorted by descending [`DirtyRegion::priority`] and the tail is truncated,
    /// so low-priority (and, on ties, earlier-inserted) regions are dropped
    /// without being rendered.
    ///
    /// Calling this when the tracker is within its limit does nothing.
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
crate::impl_default_via_new!(DirtyRegionTracker);
