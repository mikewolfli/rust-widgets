//! Performance monitoring and optimization utilities, including dirty region tracking, update batching, and profiling.
pub mod batcher;
pub mod dirty;
mod profiler;
pub mod region;
use crate::core::Rect;
pub use batcher::*;
pub use dirty::*;
pub use profiler::*;
pub use region::*;

/// Render only the dirty regions by setting clip rects.
/// Uses existing DirtyRegionTracker + RenderContext push_clip/pop_clip.
/// This avoids redrawing the entire frame when only parts changed.
pub fn render_dirty_regions(
    tracker: &mut DirtyRegionTracker,
    ctx: &mut crate::render::RenderContext,
    mut render_all: impl FnMut(&mut crate::render::RenderContext),
) {
    // 1. Merge overlapping regions
    tracker.merge();

    // 2. If no dirty regions, skip
    if tracker.is_empty() {
        return;
    }

    // 3. If too many regions, fall back to full redraw using bounding rect
    if tracker.len() > 16 {
        if let Some(bounding) = tracker.get_bounding_rect() {
            ctx.push_clip(bounding.x, bounding.y, bounding.width, bounding.height);
            render_all(ctx);
            ctx.pop_clip();
        }
        tracker.clear();
        return;
    }

    // 4. Otherwise, redraw each dirty region separately
    let regions: Vec<Rect> = tracker.regions().iter().map(|r| r.rect).collect();
    for rect in regions {
        ctx.push_clip(rect.x, rect.y, rect.width, rect.height);
        render_all(ctx);
        ctx.pop_clip();
    }
    tracker.clear();
}

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
