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
//!   "Partial Refresh" on the strength of it. It is unreachable because the live
//!   render loop (`widget::runtime::render_frame`) always paints the whole frame
//!   and never consults a damage tracker. Removal condition: either the render
//!   loop adopts it, or the README stops claiming the feature.
//!
//! So this module is not simply dead weight: one third of it is an advertised,
//! implemented, unwired feature. Deleting it without deciding that question would
//! silently drop the capability the README promises.
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
        // More than 16 regions should fall back to bounding rect
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
        // Should use bounding rect fallback: 1 call
        assert_eq!(call_count, 1, "too many regions should fall back to bounding rect (1 call)");
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
}
