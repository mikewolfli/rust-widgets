// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dirty region tracking for incremental rendering.
use crate::compat::Vec;
use crate::core::rect_merge::bounding_rect;
use crate::core::Rect;
use core::cmp::Reverse;
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
    /// Records `rect` as dirty, coalescing to one covering region once the tracker is over its
    /// `max_regions` limit, and returns the id of the region the damage landed in.
    ///
    /// # Why this is not `optimize` (D09-RENDER-01)
    ///
    /// `optimize` sorts by layer/priority and **truncates**, which drops regions — the pixels
    /// they covered are then never repainted and stay stale. A renderer cannot do that. Instead,
    /// once the count would exceed the limit, every currently tracked region is replaced by their
    /// single bounding rectangle and the new one is unioned into it. The count collapses to one,
    /// which bounds both the later `merge` (whose cost grows with the count) and the per-region
    /// repaint loop, while **no damaged pixel is lost**: a bounding rectangle contains every
    /// region it was formed from, and the new rect is contained by construction.
    ///
    /// Returns the id of the region now holding the damage — the freshly added region when it was
    /// appended, or the single coalesced region's new id otherwise.
    pub fn add_coalescing_beyond_capacity(&mut self, rect: Rect) -> RegionId {
        if self.regions.len() < self.max_regions {
            return self.add(rect);
        }
        // Over (or exactly at) capacity: fold everything, including the new rect, into one
        // covering region so the damage is bounded without being dropped.
        let mut bounding = rect;
        let mut layer = 0u32;
        let mut priority = 0u8;
        for region in self.regions.drain(..) {
            bounding = bounding.union(&region.rect);
            layer = layer.max(region.layer);
            priority = priority.max(region.priority);
        }
        let coalesced = DirtyRegion::new(bounding).with_layer(layer).with_priority(priority);
        let id = coalesced.id;
        self.regions.push(coalesced);
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
    /// Returns the configured [`DirtyRegionTracker::optimize`] limit.
    pub fn max_regions(&self) -> usize {
        self.max_regions
    }
    /// Returns `true` when more regions are tracked than the configured limit.
    ///
    /// # Why this is a query and not an automatic action (D09-RENDER-01)
    ///
    /// `optimize` *drops* regions once it is over capacity, which is correct for a compositor
    /// that only wants a hint but wrong for a renderer: a dropped region is a band of the surface
    /// that is never repainted and therefore shows stale pixels. A renderer that learns it is over
    /// capacity must instead cover **all** the damage with one repaint, so it needs to *ask*
    /// whether the tracker is over the limit without the tracker having already thrown damage
    /// away. This method answers that question; the decision about what to do with it belongs to
    /// the caller that knows how it paints.
    pub fn over_capacity(&self) -> bool {
        self.regions.len() > self.max_regions
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
    /// Every surviving region gets a fresh [`RegionId`] (merge invalidates ids). Unlike the
    /// first version, a merged union **keeps** the highest [`DirtyRegion::layer`] and
    /// [`DirtyRegion::priority`] of its constituents rather than resetting them to `0`; a
    /// region that merged with nothing keeps its own values. The geometry merge itself is
    /// performed here on [`DirtyRegion`] rather than by handing a bare `Vec<Rect>` to
    /// `merge_intersecting_rects`, because that helper cannot carry the metadata the union
    /// must preserve.
    ///
    /// # Cost (D09-RENDER-01)
    ///
    /// Unioning bounding boxes is transitive: a union can grow to reach a region the scan already
    /// passed, so the scan restarts from the beginning after each union. That is what makes the
    /// work grow with the square of the region count in the worst case, and it is why the caller
    /// must bound the count *before* calling this: [`DirtyRegionTracker::over_capacity`] lets a
    /// renderer switch to a single covering repaint instead of merging an unbounded bag. The
    /// regions are sorted by left edge first so that the restart is cheap for the clustered
    /// damage a frame actually produces, and the ordering never changes the result because a
    /// union is commutative in geometry and the metadata merge is a `max`.
    pub fn merge(&mut self) {
        if self.merged || self.regions.len() <= 1 {
            return;
        }
        // Sorting by left edge does not change which rectangles end up in which union; it only
        // makes the restarts below walk regions that are near each other in `x`.
        self.regions.sort_by_key(|region| region.rect.x);
        let mut merged: Vec<DirtyRegion> = Vec::with_capacity(self.regions.len());
        for mut current in self.regions.drain(..) {
            let mut i = 0;
            while i < merged.len() {
                if current.rect.intersects(&merged[i].rect) {
                    current.rect = current.rect.union(&merged[i].rect);
                    // A union sits on the highest layer and keeps the highest retention
                    // priority of the regions it absorbed, so `optimize` can still order it.
                    current.layer = current.layer.max(merged[i].layer);
                    current.priority = current.priority.max(merged[i].priority);
                    merged.swap_remove(i);
                    i = 0;
                } else {
                    i += 1;
                }
            }
            current.id = RegionId::new();
            merged.push(current);
        }
        self.regions = merged;
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
    /// sorted by descending [`DirtyRegion::layer`], then descending
    /// [`DirtyRegion::priority`], and the tail is truncated, so the regions that sit **above**
    /// others survive first, and within a layer the higher-priority (and, on ties,
    /// earlier-inserted) regions are kept.
    ///
    /// # Why `layer` leads the sort
    ///
    /// The field documents "higher layers sit above lower ones and are retained first". Sorting
    /// on `priority` alone made that false: a low-priority region on a top layer could be dropped
    /// while a high-priority one on a background layer survived, which is the opposite of what a
    /// compositor wants. `layer` is the outer key because an obscured upper layer is the one whose
    /// loss is visible.
    ///
    /// Calling this when the tracker is within its limit does nothing.
    pub fn optimize(&mut self) {
        if self.regions.len() > self.max_regions {
            self.merge();
            if self.regions.len() > self.max_regions {
                // `Reverse` on both keys: descending layer, then descending priority. `sort_by_key`
                // is stable, so equal (layer, priority) pairs keep their insertion order.
                self.regions
                    .sort_by_key(|region| (Reverse(region.layer), Reverse(region.priority)));
                self.regions.truncate(self.max_regions);
            }
        }
    }
}
crate::impl_default_via_new!(DirtyRegionTracker);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    fn region_rects(tracker: &DirtyRegionTracker) -> Vec<(i32, i32)> {
        tracker.regions.iter().map(|region| (region.rect.x, region.rect.y)).collect()
    }

    /// `optimize` must retain the regions that sit **above** others first, then by priority.
    ///
    /// Pins the defect: the doc said `layer` is "used by `optimize`", but `optimize` sorted on
    /// `priority` alone — so a low-priority region on a top layer could be dropped while a
    /// high-priority background one survived, the opposite of what a compositor wants.
    #[test]
    fn optimize_keeps_upper_layers_first() {
        let mut tracker = DirtyRegionTracker::with_max_regions(2);
        // Distinct, non-overlapping rects so `merge` cannot collapse them.
        tracker.add_with_layer(Rect::new(0, 0, 10, 10), 0); // background, high priority
        tracker.regions.last_mut().expect("just pushed").priority = 200;
        tracker.add_with_layer(Rect::new(50, 0, 10, 10), 5); // top layer, low priority
        tracker.add_with_layer(Rect::new(100, 0, 10, 10), 1); // middle

        tracker.optimize();

        let kept = region_rects(&tracker);
        assert_eq!(kept.len(), 2, "the tracker is trimmed to its limit");
        assert!(
            kept.contains(&(50, 0)),
            "the top-layer region must survive even at low priority: {kept:?}"
        );
        assert!(!kept.contains(&(0, 0)), "the background region is the one dropped: {kept:?}");
    }

    /// Within one layer, priority still decides.
    #[test]
    fn optimize_orders_by_priority_within_a_layer() {
        let mut tracker = DirtyRegionTracker::with_max_regions(1);
        tracker.add_with_layer(Rect::new(0, 0, 10, 10), 2);
        tracker.regions.last_mut().expect("just pushed").priority = 1;
        tracker.add_with_layer(Rect::new(50, 0, 10, 10), 2);
        tracker.regions.last_mut().expect("just pushed").priority = 9;

        tracker.optimize();

        let kept = region_rects(&tracker);
        assert_eq!(kept, vec![(50, 0)], "the higher-priority same-layer region is kept");
    }

    /// `optimize` must retain priority across `merge`, and in both insertion orders, so a
    /// high-priority region is never dropped because `merge` reset it to zero.
    #[test]
    fn optimize_preserves_priority_in_either_insertion_order() {
        for reverse in [false, true] {
            let mut tracker = DirtyRegionTracker::with_max_regions(1);
            if !reverse {
                tracker.add_with_priority(Rect::new(0, 0, 10, 10), 200);
                tracker.add_with_priority(Rect::new(50, 0, 10, 10), 1);
            } else {
                tracker.add_with_priority(Rect::new(50, 0, 10, 10), 1);
                tracker.add_with_priority(Rect::new(0, 0, 10, 10), 200);
            }

            tracker.optimize();

            assert_eq!(
                region_rects(&tracker),
                vec![(0, 0)],
                "the high-priority region survives regardless of insertion order: {reverse}"
            );
        }
    }

    /// A merged union carries the highest layer and priority of its constituents, so `optimize`
    /// can still order it after the merge.
    #[test]
    fn merge_carries_priority_and_layer_into_the_union() {
        let mut tracker = DirtyRegionTracker::with_max_regions(1);
        tracker.add_with_priority(Rect::new(0, 0, 10, 10), 50);
        tracker.add_with_layer(Rect::new(5, 5, 10, 10), 3);

        tracker.merge();

        assert_eq!(tracker.len(), 1, "the overlapping rects collapse into one region");
        let region = &tracker.regions[0];
        assert_eq!(region.rect, Rect::new(0, 0, 15, 15));
        assert_eq!(region.layer, 3, "the union sits on the top layer");
        assert_eq!(region.priority, 50, "and keeps the highest priority");
    }

    /// Adding past the capacity coalesces to one covering region and keeps every pixel covered,
    /// unlike `optimize`, which would truncate and drop damage. (D09-RENDER-01)
    #[test]
    fn add_coalescing_beyond_capacity_covers_all_damage() {
        let mut tracker = DirtyRegionTracker::with_max_regions(4);
        // Three disjoint regions: still under the limit, so they stay separate.
        for i in 0..3 {
            tracker.add(Rect::new(i * 10, 0, 4, 4));
        }
        assert_eq!(tracker.len(), 3, "under capacity nothing is coalesced");

        // The fourth fills the limit exactly.
        tracker.add(Rect::new(100, 100, 4, 4));
        assert_eq!(tracker.len(), 4);

        // The fifth pushes past it: everything folds into one covering region.
        tracker.add_coalescing_beyond_capacity(Rect::new(200, 200, 4, 4));
        assert_eq!(tracker.len(), 1, "over capacity the damage collapses to one region");
        let region = tracker.regions[0].rect;
        assert!(region.contains_rect(&Rect::new(0, 0, 4, 4)));
        assert!(region.contains_rect(&Rect::new(100, 100, 4, 4)));
        assert!(region.contains_rect(&Rect::new(200, 200, 4, 4)));
        assert!(!tracker.over_capacity());
    }

    /// `over_capacity` reports the limit without dropping anything: it is a query, so the regions
    /// are untouched. (D09-RENDER-01)
    #[test]
    fn over_capacity_reports_without_dropping() {
        let mut tracker = DirtyRegionTracker::with_max_regions(2);
        assert!(!tracker.over_capacity());
        tracker.add(Rect::new(0, 0, 1, 1));
        tracker.add(Rect::new(2, 0, 1, 1));
        assert!(!tracker.over_capacity(), "exactly at the limit is not over");
        tracker.add(Rect::new(4, 0, 1, 1));
        assert!(tracker.over_capacity(), "one past the limit is over");
        assert_eq!(tracker.len(), 3, "the query must not have dropped the excess");
    }
}
