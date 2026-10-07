// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Performance monitoring and optimization utilities, including dirty region tracking, update batching, and profiling.
//!
//! # Reachability
//!
//! **State:** Reserved. Two distinct findings, because the earlier statement here
//! ("superseded by `gpu::performance`") was wrong and is corrected rather than
//! deleted:
//!
//! * `frame_timer` / `profiler` / `batcher` — a frame-timing and batching toolkit
//!   with no production caller. These genuinely overlap `gpu::performance`
//!   (`AdaptivePerformanceMonitor`, `PerformanceStats`), which *is* consumed by
//!   `src/gpu/adapter.rs`. Removal condition for this group: once
//!   `gpu::performance` is confirmed to cover frame timing too.
//! * `dirty` / `region` / `render_dirty` — **not duplicated anywhere**: this is the
//!   only damage-region implementation in the crate, and `README.md` advertises
//!   "Partial Refresh" on the strength of it. **Now wired** (BLUE16 phase F):
//!   `widget::runtime` holds a per-widget `DirtyRegionTracker`, `mark_dirty_rect`
//!   records damage, and `render_frame_incremental` consumes it through
//!   `render_dirty_regions`. The wire is opt-in via `widget::runtime::RepaintMode`,
//!   so the default remains a full paint and no existing caller's behaviour changed.
//!
//!   The earlier note here said the loop "never consults a damage tracker", which
//!   was true and is why the README's claim was once softened. The claim can now
//!   stand again, with the caveat that damage tracking is a *choice*: a caller that
//!   leaves the mode at `Full` gets exactly the old behaviour.
//!
//! So this module is not dead weight: `dirty` / `region` / `render_dirty` are a
//! live, wired feature — `render_dirty_regions` is called from
//! `src/widget/runtime.rs` — and the frame-timing group is the only part still
//! reserved. Deleting the module without deciding that question would silently
//! drop the capability the README promises.
//!
//! The real boundaries, stated once so they cannot drift: the default repaint
//! mode is `Full`; `Dirty` is opt-in; and within `Dirty`, damage is grouped by
//! covered *area* (see `render_dirty::FULL_REPAINT_AREA_RATIO`) rather than by
//! region count.
/// Coalesces repaint requests so a burst of invalidations costs one frame.
pub mod batcher;
/// Tracks the union of regions that need repainting.
pub mod dirty;
/// Frame timing and frame-rate statistics.
pub mod frame_timer;
mod profiler;
/// Rect-region primitives used by the dirty-region tracker.
pub mod region;
/// Turns widget-level repaint requests into renderer-level dirty regions.
pub mod render_dirty;
pub use batcher::*;
pub use dirty::*;
pub use frame_timer::*;
pub use profiler::*;
pub use region::*;
pub use render_dirty::render_dirty_regions;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::PaintBackend;
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
    fn test_render_dirty_regions_empty() {
        // Empty tracker should do nothing and not panic
        let mut tracker = DirtyRegionTracker::new();
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        let mut called = false;
        render_dirty_regions(&mut tracker, &mut ctx, |_ctx| {
            called = true;
        });
        assert!(!called, "render_all should not be called for empty tracker");
        assert!(tracker.is_empty());
        backend.end_frame();
    }

    #[test]
    fn test_render_dirty_regions_single() {
        // Single region should render once with correct clip
        let mut tracker = DirtyRegionTracker::new();
        tracker.add(Rect::new(10, 10, 50, 50));
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        let mut call_count = 0;
        render_dirty_regions(&mut tracker, &mut ctx, |_ctx| {
            call_count += 1;
        });
        assert_eq!(call_count, 1, "render_all should be called once for a single region");
        assert!(tracker.is_empty());
        backend.end_frame();
    }

    #[test]
    fn test_render_dirty_regions_merges() {
        // Overlapping regions should trigger merge, single render call with bounding rect
        let mut tracker = DirtyRegionTracker::new();
        tracker.add(Rect::new(0, 0, 100, 100));
        tracker.add(Rect::new(50, 50, 100, 100));
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        tracker.merge();
        assert_eq!(tracker.len(), 1, "overlapping regions should merge into one");
        let mut call_count = 0;
        render_dirty_regions(&mut tracker, &mut ctx, |_ctx| {
            call_count += 1;
        });
        assert_eq!(call_count, 1, "render_all should be called once after merge");
        assert!(tracker.is_empty());
        backend.end_frame();
    }

    #[test]
    fn test_render_dirty_regions_too_many() {
        // Twenty tiny regions covering 1.25% of the frame must each get their own
        // pass. The rule used to be "more than 16 regions falls back", which made
        // this input collapse to a single full-frame clip — repainting 40,000 pixels
        // to show 500.
        let mut tracker = DirtyRegionTracker::new();
        for i in 0..20 {
            tracker.add(Rect::new(i * 10, 0, 5, 5));
        }
        assert_eq!(tracker.len(), 20);
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        let mut call_count = 0;
        render_dirty_regions(&mut tracker, &mut ctx, |_ctx| {
            call_count += 1;
        });
        assert_eq!(
            call_count, 20,
            "many small regions stay separate; only covered area decides the fallback"
        );
        assert!(tracker.is_empty());
        backend.end_frame();
    }

    /// The replacement for the count rule must still fall back when the damage is
    /// genuinely large, or the optimisation would repaint pixel-by-pixel region
    /// clips over a frame that changed entirely.
    #[test]
    fn test_render_dirty_regions_large_damage_falls_back() {
        let mut tracker = DirtyRegionTracker::new();
        // Two regions whose union is most of the frame, but which do not overlap.
        tracker.add(Rect::new(0, 0, 200, 100));
        tracker.add(Rect::new(0, 100, 200, 100));
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        let mut call_count = 0;
        render_dirty_regions(&mut tracker, &mut ctx, |_ctx| {
            call_count += 1;
        });
        assert_eq!(call_count, 1, "frame-sized damage collapses to one pass");
        assert!(tracker.is_empty());
        backend.end_frame();
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

    /// A single rect added to a fresh batcher must still be delivered once its timeout
    /// elapses, even though the count threshold is never reached.
    ///
    /// The timer used to start only after the first flush, so a fresh batcher receiving one
    /// rect (and nothing else) had no elapsed baseline and `should_flush` stayed `false`
    /// forever — a small first batch could wait indefinitely.
    #[test]
    fn a_small_first_batch_flushes_within_the_timeout() {
        let mut batcher = UpdateBatcher::new(100);
        batcher.add(Rect::new(0, 0, 10, 10));

        // A single rect is under the count threshold, and the budget has not elapsed yet.
        assert!(!batcher.should_flush(), "a single fresh rect must not flush immediately");

        // Once the budget elapses the same single rect must be delivered.
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert!(
            batcher.should_flush(),
            "the first small batch must flush within its timeout budget"
        );

        // Flushing drains the batch and leaves nothing pending.
        assert!(!batcher.flush().is_empty());
        assert!(batcher.is_empty());
        assert!(!batcher.should_flush());
    }
}
