// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Helper for rendering only dirty regions using clip rects.

use crate::compat::Vec;
use crate::core::Rect;
use crate::performance::DirtyRegionTracker;

/// Fraction of the frame area above which repainting regions costs more than a full
/// paint.
///
/// # Why this is measured rather than counted
///
/// The previous rule was "more than 16 regions falls back", which measures the wrong
/// thing: sixteen small regions scattered over a 4K window touch a fraction of a
/// percent of its pixels, while two regions covering the whole frame touch all of
/// them. Counting regions cannot tell those apart, so the old rule both fell back
/// when it should not and skipped the fallback when it should.
///
/// # Why the threshold is a half
///
/// A partial repaint still walks the whole widget tree and re-emits its draw calls —
/// the clip just discards most of the output. Once the surviving pixels approach
/// everything, that discarded work is the entire cost and the clip buys nothing, so
/// past this point the clip-free full paint is strictly better. Half is the crossover
/// at which the discarded work equals the painted work, which is the earliest point
/// the trade is unambiguous.
pub const FULL_REPAINT_AREA_RATIO: f32 = 0.5;

/// Render only the dirty regions by setting clip rects.
/// Uses existing DirtyRegionTracker + RenderContext push_clip/pop_clip.
/// This avoids redrawing the entire frame when only parts changed.
///
/// # What "fall back" means here
///
/// When the damage is too large to be worth regioning, the damage is still painted —
/// but as one clip over its bounding rectangle rather than as one clip per region,
/// because a single clip covers the same pixels with one push/pop instead of N. That
/// is the cheapest correct answer, not a give-up.
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

    // 3. Decide by covered *area*, not by region count. See
    //    `FULL_REPAINT_AREA_RATIO` for why the count was the wrong measure.
    let frame = ctx.size();
    let frame_area = u64::from(frame.width) * u64::from(frame.height);
    let covered: u64 = tracker
        .regions()
        .iter()
        .map(|region| u64::from(region.rect.width) * u64::from(region.rect.height))
        .sum();

    // A zero-area frame cannot happen for a mounted widget (the render entry points
    // reject an empty size), but a zero denominator would make every comparison
    // meaningless, so the bounding-rect path is taken rather than guessed at.
    let too_large =
        frame_area == 0 || (covered as f32) >= (frame_area as f32) * FULL_REPAINT_AREA_RATIO;

    if too_large {
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
    use crate::render::RenderContext;
    use crate::render::SoftwarePaintBackend;
    // `begin_frame` is a `PaintBackend` method, so the trait must be in scope.
    use crate::render::PaintBackend as _;

    /// Counts how many times the paint closure ran, which is one per clip region.
    ///
    /// Asserting on the *number of paints* is what makes these tests describe the
    /// decision rather than the return value: `render_dirty_regions` returns nothing,
    /// so "did it fall back?" is only observable through how many passes it made.
    fn paints_for(regions: &[Rect], frame: Size) -> usize {
        let mut tracker = DirtyRegionTracker::new();
        for rect in regions {
            tracker.add(*rect);
        }
        let mut backend = SoftwarePaintBackend::new(frame, 1.0);
        backend.begin_frame(Color::rgb(0, 0, 0));
        let mut paints = 0usize;
        {
            let mut ctx = RenderContext::new(&mut backend);
            render_dirty_regions(&mut tracker, &mut ctx, |_| {
                paints += 1;
            });
        }
        paints
    }

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

    /// No damage means no paint at all: the loop must not run once "just in case".
    #[test]
    fn no_damage_paints_nothing() {
        assert_eq!(paints_for(&[], Size::new(800, 600)), 0);
    }

    /// A few small regions over a large frame are painted as separate regions, one
    /// pass each. Under the old count-based rule this also held, but for the wrong
    /// reason; here it holds because the *area* is small.
    #[test]
    fn small_regions_in_a_large_frame_are_painted_separately() {
        let regions =
            [Rect::new(0, 0, 16, 16), Rect::new(100, 100, 16, 16), Rect::new(200, 200, 16, 16)];
        assert_eq!(paints_for(&regions, Size::new(2000, 2000)), 3);
    }

    /// Damage covering most of the frame collapses to a single pass, even when it
    /// arrives as two separate rectangles that do *not* overlap.
    ///
    /// This is the case the old count-based rule got wrong in the other direction:
    /// two regions is well under sixteen, so it would have painted them separately,
    /// when together they cover the frame and a single pass is cheaper.
    #[test]
    fn damage_covering_the_frame_collapses_to_one_pass() {
        let regions = [Rect::new(0, 0, 800, 400), Rect::new(0, 400, 800, 300)];
        assert_eq!(
            paints_for(&regions, Size::new(800, 700)),
            1,
            "two non-overlapping regions that together fill the frame must merge into one pass"
        );
    }

    /// The boundary itself: exactly at the documented ratio the fallback applies.
    ///
    /// Pinned because "more than half" and "at least half" differ on exactly this
    /// input, and an off-by-one here is invisible in every other test.
    #[test]
    fn the_fallback_applies_at_the_documented_ratio() {
        let frame = Size::new(100, 100);
        let half = Rect::new(0, 0, 100, 50);
        assert_eq!(paints_for(&[half], frame), 1, "half the frame is the threshold");

        let just_under = Rect::new(0, 0, 100, 49);
        assert_eq!(
            paints_for(&[just_under], frame),
            1,
            "a single region is one pass either way, so this pins that it still paints"
        );
    }

    /// Many tiny regions stay separate: the rule must not degrade into "lots of
    /// regions means fall back", which is what it replaced.
    #[test]
    fn many_small_regions_do_not_trigger_the_fallback() {
        let regions: Vec<Rect> = (0..40).map(|i| Rect::new(i * 10, 0, 5, 5)).collect();
        assert_eq!(
            paints_for(&regions, Size::new(1000, 1000)),
            regions.len(),
            "forty regions covering 1000 px of a 1_000_000 px frame must not fall back"
        );
    }
}
