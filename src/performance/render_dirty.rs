//! Helper for rendering only dirty regions using clip rects.

use crate::core::Rect;
use crate::performance::DirtyRegionTracker;

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
    use crate::core::Rect;
    #[test]
    fn test_dirty_region_tracker() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.add(Rect::new(0, 0, 100, 100));
        tracker.add(Rect::new(50, 50, 100, 100));
        tracker.merge();
        // The two overlapping rects merge into a single dirty region.
        assert_eq!(tracker.len(), 1);
        assert!(!tracker.is_empty());
    }
}
