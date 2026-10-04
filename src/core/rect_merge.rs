// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rect merging algorithms — canonical implementations for combining overlapping
//! rectangles into minimal covering sets.
//!
//! # Duplication Elimination
//!
//! Previously, `crate::performance::region::DirtyRegionTracker` implemented
//! its own rect-merging loop with `intersects` + `union`, and
//! `crate::render::backend::batch::RenderBatch` had its own `merge_adjacent_rects`.
//! This module centralises the core algorithm so all consumers share a single
//! correct, tested implementation.

use crate::compat::Vec;
use crate::core::Rect;

/// Merge overlapping rectangles into a minimal covering set.
///
/// Two rectangles are considered "overlapping" when [`Rect::intersects`] returns
/// `true`. When an overlap is detected, the two rectangles are replaced by their
/// [`Rect::union`], and the process is repeated until no more overlaps exist.
///
/// The input order is **not** preserved in the output.
///
/// # Example
///
/// ```
/// use rust_widgets::core::{Rect, rect_merge::merge_intersecting_rects};
///
/// let rects = vec![
///     Rect::new(0, 0, 100, 100),
///     Rect::new(50, 50, 100, 100),
///     Rect::new(200, 200, 50, 50),   // disjoint
/// ];
/// let merged = merge_intersecting_rects(&rects);
/// assert_eq!(merged.len(), 2); // one merged bounding + one separate
/// ```
pub fn merge_intersecting_rects(rects: &[Rect]) -> Vec<Rect> {
    if rects.is_empty() {
        return Vec::new();
    }
    if rects.len() == 1 {
        return rects.to_vec();
    }

    // Build the output incrementally: for each input rect, merge it against every rect
    // *already emitted*. The previous loop only re-checked the un-consumed `remaining` tail,
    // so a union that grew late could overlap an earlier output and break the "until no more
    // overlaps exist" contract. Merging against the output itself — and restarting the scan
    // after each merge, because a union can bridge to a rect checked earlier in this pass —
    // keeps the emitted set non-overlapping.
    let mut merged = Vec::new();
    for &rect in rects {
        let mut current = rect;
        let mut i = 0;
        while i < merged.len() {
            if current.intersects(&merged[i]) {
                current = current.union(&merged[i]);
                merged.swap_remove(i);
                // Restart from the beginning: the newly expanded `current` may now
                // intersect a rect checked earlier in this pass.
                i = 0;
            } else {
                i += 1;
            }
        }
        merged.push(current);
    }

    merged
}

/// Compute the bounding rectangle of a set of rectangles.
///
/// Returns `None` when the input is empty.
pub fn bounding_rect(rects: &[Rect]) -> Option<Rect> {
    if rects.is_empty() {
        return None;
    }
    let mut result = rects[0];
    for r in &rects[1..] {
        result = result.union(r);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn test_merge_intersecting_empty() {
        assert!(merge_intersecting_rects(&[]).is_empty());
    }

    #[test]
    fn test_merge_intersecting_single() {
        let r = Rect::new(10, 10, 50, 50);
        assert_eq!(merge_intersecting_rects(&[r]), vec![r]);
    }

    #[test]
    fn test_merge_intersecting_overlap() {
        let rects = vec![Rect::new(0, 0, 100, 100), Rect::new(50, 50, 100, 100)];
        let merged = merge_intersecting_rects(&rects);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], Rect::new(0, 0, 150, 150));
    }

    #[test]
    fn test_merge_intersecting_disjoint() {
        let rects =
            vec![Rect::new(0, 0, 10, 10), Rect::new(100, 100, 10, 10), Rect::new(200, 200, 10, 10)];
        let merged = merge_intersecting_rects(&rects);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_bounding_rect_empty() {
        assert!(bounding_rect(&[]).is_none());
    }

    #[test]
    fn test_bounding_rect_single() {
        let r = Rect::new(5, 5, 20, 20);
        assert_eq!(bounding_rect(&[r]), Some(r));
    }

    #[test]
    fn test_bounding_rect_multiple() {
        let rects =
            vec![Rect::new(0, 0, 10, 10), Rect::new(20, 20, 10, 10), Rect::new(100, 100, 50, 50)];
        assert_eq!(bounding_rect(&rects), Some(Rect::new(0, 0, 150, 150)));
    }

    /// A union that grows late must not overlap a rect emitted earlier: the merge scans the
    /// already-emitted output, not just the un-consumed tail.
    #[test]
    fn a_later_union_never_overlaps_an_earlier_output() {
        // An L-shaped pair plus an interior rect. Naively, the pair merges first and the interior
        // rect is then emitted beside the pair even though it sits inside the union.
        let rects = vec![Rect::new(0, 0, 10, 2), Rect::new(0, 0, 2, 10), Rect::new(4, 4, 1, 1)];
        let merged = merge_intersecting_rects(&rects);
        for (i, a) in merged.iter().enumerate() {
            for b in &merged[i + 1..] {
                assert!(!a.intersects(b), "output rects must not overlap: {a:?} vs {b:?}");
            }
        }
        assert_eq!(merged, vec![Rect::new(0, 0, 10, 10)], "the L-shape absorbs the interior rect");
    }

    /// The merge is idempotent and every input rect is covered by some output rect.
    #[test]
    fn merging_is_idempotent_and_covers_the_input() {
        let rects = vec![
            Rect::new(0, 0, 10, 10),
            Rect::new(5, 5, 20, 20),
            Rect::new(100, 100, 10, 10),
            Rect::new(110, 100, 5, 5),
        ];
        let once = merge_intersecting_rects(&rects);
        let twice = merge_intersecting_rects(&once);
        assert_eq!(once.len(), twice.len(), "a second pass does not change the count");
        for out in &twice {
            assert!(once.contains(out), "the second pass is the same set");
        }
        for input in &rects {
            assert!(
                once.iter().any(|out| out.contains_rect(input)),
                "every input rect is covered by an output rect: {input:?} not in {once:?}"
            );
        }
    }
}
