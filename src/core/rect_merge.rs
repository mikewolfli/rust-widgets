//! Rect merging algorithms — canonical implementations for combining overlapping
//! rectangles into minimal covering sets.
//!
//! # Duplication Elimination
//!
//! Previously, [`crate::performance::region::DirtyRegionTracker`] implemented
//! its own rect-merging loop with `intersects` + `union`, and
//! [`crate::render::backend::batch::RenderBatch`] had its own `merge_adjacent_rects`.
//! This module centralises the core algorithm so all consumers share a single
//! correct, tested implementation.

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

    let working: Vec<Rect> = rects.to_vec();
    let mut consumed = vec![false; working.len()];
    let mut merged = Vec::new();

    for i in 0..working.len() {
        if consumed[i] {
            continue;
        }
        let mut current = working[i];
        consumed[i] = true;

        // Greedily absorb any other rect that intersects the current one.
        // Repeat passes because a newly-absorbed rect may itself intersect
        // others that the original `current` did not.
        loop {
            let mut progressed = false;
            for j in (i + 1)..working.len() {
                if consumed[j] {
                    continue;
                }
                if current.intersects(&working[j]) {
                    current = current.union(&working[j]);
                    consumed[j] = true;
                    progressed = true;
                }
            }
            if !progressed {
                break;
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
        let rects = vec![
            Rect::new(0, 0, 10, 10),
            Rect::new(100, 100, 10, 10),
            Rect::new(200, 200, 10, 10),
        ];
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
        let rects = vec![
            Rect::new(0, 0, 10, 10),
            Rect::new(20, 20, 10, 10),
            Rect::new(100, 100, 50, 50),
        ];
        assert_eq!(bounding_rect(&rects), Some(Rect::new(0, 0, 150, 150)));
    }
}
