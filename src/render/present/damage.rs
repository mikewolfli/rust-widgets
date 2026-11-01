// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Which rows of the window a frame actually changed.
//!
//! The presentation path re-uploads only the rows that changed and presents only when something
//! did (plan B29.09/B29.12). Reported damage arrives as arbitrary rectangles from the paint
//! backend ([`crate::render::PaintBackend::damage_regions`]); this module turns that into
//! **full-width row bands**, because a band is what a texture upload copies and what a surface
//! present can be scoped to without a scissor.
//!
//! # Why bands and not rectangles
//!
//! A texture upload copies a rectangular sub-resource, so a rectangle would also work; the row
//! band is chosen because the compositor takes the whole row range anyway and because it keeps
//! the number of copies low: five scattered rectangles in one horizontal band become one copy.
//! The cost is uploading the columns between them, which is small compared with a per-copy submit.
//!
//! # What is not decided here
//!
//! Nothing in this module touches a GPU. It answers "which rows" and "is there anything to
//! present"; the presenter decides how to copy them.

use crate::compat::Vec;

/// The largest number of disjoint row bands worth copying in one frame.
///
/// Above it the gaps between bands are cheaper to copy than the extra submits, so the frame
/// collapses to a whole-frame upload. Eight is chosen to match the maximum number of damage
/// regions the incremental software path already keeps before it merges them (see
/// `crate::performance::region`), so the presenter never sees more bands than the frame's own
/// damage model produced.
pub const MAX_PRESENT_BANDS: usize = 8;

/// A run of consecutive rows that changed, in physical pixels from the top of the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageBand {
    /// First damaged row.
    pub y: u32,
    /// Number of rows in the band; never zero.
    pub height: u32,
}

impl DamageBand {
    /// One past the last damaged row.
    pub const fn end(self) -> u32 {
        self.y + self.height
    }
}

/// Accumulates the damage of one frame and coalesces it into row bands.
///
/// The caller reports damage as often as it likes; [`PresentDamage::finish`] is what makes the
/// report readable. Calling the read accessors before `finish` answers about uncoalesced input,
/// which is why the presenter always finishes first.
#[derive(Debug, Default, Clone)]
pub struct PresentDamage {
    bands: Vec<DamageBand>,
    full: bool,
}

impl PresentDamage {
    /// An empty report: nothing changed, so nothing needs presenting.
    pub fn new() -> Self {
        Self::default()
    }

    /// Drops every recorded band. Called once a frame has been presented.
    pub fn clear(&mut self) {
        self.bands.clear();
        self.full = false;
    }

    /// Whether the frame has nothing to present.
    pub fn is_empty(&self) -> bool {
        !self.full && self.bands.is_empty()
    }

    /// Whether the whole target is dirty, so the frame must be uploaded in one piece.
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Marks the whole target dirty (first frame, resize, a restored window, a lost surface).
    pub fn mark_full(&mut self) {
        self.full = true;
        self.bands.clear();
    }

    /// Records a damaged rectangle, clipped to a target `surface_height` rows tall.
    ///
    /// A rectangle that is entirely outside the target, or has a zero dimension, records nothing:
    /// a caller that reports a 0×0 damage did not damage anything, and a rect above the target is
    /// not an error to propagate.
    pub fn mark_rect(&mut self, x: i32, y: i32, width: u32, height: u32, surface_height: u32) {
        if self.full || width == 0 || height == 0 || surface_height == 0 {
            return;
        }
        let _ = x;
        let top = i64::from(y);
        let bottom = top + i64::from(height);
        let top = top.max(0);
        let bottom = bottom.min(i64::from(surface_height));
        if bottom <= top {
            return;
        }
        self.bands.push(DamageBand { y: top as u32, height: (bottom - top) as u32 });
    }

    /// Records each `(x, y, width, height)` rectangle reported by the paint backend.
    ///
    /// The tuple layout is the one [`crate::render::PaintBackend::damage_regions`] returns, so a
    /// caller forwards that slice without converting it.
    pub fn mark_regions(&mut self, regions: &[(i32, i32, u32, u32)], surface_height: u32) {
        for &(x, y, width, height) in regions {
            self.mark_rect(x, y, width, height, surface_height);
        }
    }

    /// Sorts and merges the recorded bands, and collapses to a whole-frame upload when the result
    /// would be more than [`MAX_PRESENT_BANDS`] disjoint runs.
    pub fn finish(&mut self) {
        if self.full {
            self.bands.clear();
            return;
        }
        if self.bands.len() > 1 {
            self.bands.sort_unstable_by_key(|band| band.y);
            let mut merged: Vec<DamageBand> = Vec::with_capacity(self.bands.len());
            for band in self.bands.drain(..) {
                match merged.last_mut() {
                    Some(current) if band.y <= current.end() => {
                        let end = current.end().max(band.end());
                        current.height = end - current.y;
                    }
                    _ => merged.push(band),
                }
            }
            self.bands = merged;
        }
        if self.bands.len() > MAX_PRESENT_BANDS {
            self.mark_full();
        }
    }

    /// The coalesced bands, in top-to-bottom order. Empty when the whole frame is dirty.
    pub fn bands(&self) -> &[DamageBand] {
        &self.bands
    }

    /// Total number of rows the frame must upload, for a target `surface_height` rows tall.
    ///
    /// A whole-frame upload reports the target height, so a caller can size its copy without a
    /// second query; when nothing is damaged it reports `0`.
    pub fn damaged_rows(&self, surface_height: u32) -> u32 {
        if self.full {
            return surface_height;
        }
        self.bands.iter().map(|band| band.height).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::{DamageBand, PresentDamage, MAX_PRESENT_BANDS};

    #[test]
    fn a_frame_without_damage_needs_no_present() {
        let mut damage = PresentDamage::new();
        damage.finish();
        assert!(damage.is_empty());
        assert!(damage.bands().is_empty());
        assert_eq!(damage.damaged_rows(100), 0);
    }

    #[test]
    fn a_rect_inside_the_target_becomes_one_band() {
        let mut damage = PresentDamage::new();
        damage.mark_rect(4, 10, 20, 5, 100);
        damage.finish();
        assert!(!damage.is_empty());
        assert_eq!(damage.bands(), &[DamageBand { y: 10, height: 5 }]);
        assert_eq!(damage.damaged_rows(100), 5);
    }

    #[test]
    fn overlapping_and_adjacent_bands_coalesce() {
        let mut damage = PresentDamage::new();
        damage.mark_rect(0, 10, 4, 5, 100);
        damage.mark_rect(0, 12, 4, 4, 100);
        damage.mark_rect(0, 16, 4, 2, 100);
        damage.finish();
        assert_eq!(damage.bands(), &[DamageBand { y: 10, height: 8 }]);
    }

    #[test]
    fn separated_bands_stay_separate_and_sorted() {
        let mut damage = PresentDamage::new();
        damage.mark_rect(0, 40, 4, 2, 100);
        damage.mark_rect(0, 5, 4, 2, 100);
        damage.finish();
        assert_eq!(
            damage.bands(),
            &[DamageBand { y: 5, height: 2 }, DamageBand { y: 40, height: 2 }]
        );
        assert_eq!(damage.damaged_rows(100), 4);
    }

    #[test]
    fn a_rect_outside_the_target_is_ignored() {
        let mut damage = PresentDamage::new();
        damage.mark_rect(0, 100, 10, 4, 100);
        damage.mark_rect(0, -10, 10, 4, 100);
        damage.mark_rect(0, 10, 0, 4, 100);
        damage.mark_rect(0, 10, 4, 0, 100);
        damage.finish();
        assert!(damage.is_empty());
    }

    #[test]
    fn a_rect_crossing_the_top_or_bottom_edge_is_clipped() {
        let mut damage = PresentDamage::new();
        damage.mark_rect(0, -4, 10, 10, 100);
        damage.finish();
        assert_eq!(damage.bands(), &[DamageBand { y: 0, height: 6 }]);

        let mut damage = PresentDamage::new();
        damage.mark_rect(0, 96, 10, 10, 100);
        damage.finish();
        assert_eq!(damage.bands(), &[DamageBand { y: 96, height: 4 }]);
    }

    #[test]
    fn a_full_mark_wins_over_reported_bands() {
        let mut damage = PresentDamage::new();
        damage.mark_rect(0, 10, 4, 5, 100);
        damage.mark_full();
        damage.mark_rect(0, 20, 4, 5, 100);
        damage.finish();
        assert!(damage.is_full());
        assert!(damage.bands().is_empty());
        assert!(!damage.is_empty());
        assert_eq!(damage.damaged_rows(100), 100);
    }

    #[test]
    fn too_many_disjoint_bands_collapse_to_the_whole_frame() {
        let mut damage = PresentDamage::new();
        for index in 0..=MAX_PRESENT_BANDS {
            let y = (index as i32) * 10;
            damage.mark_rect(0, y, 4, 2, 1_000);
        }
        damage.finish();
        assert!(damage.is_full(), "more than {MAX_PRESENT_BANDS} bands is cheaper as one upload");
    }

    #[test]
    fn clearing_after_a_present_resets_both_full_and_bands() {
        let mut damage = PresentDamage::new();
        damage.mark_full();
        damage.finish();
        damage.clear();
        assert!(damage.is_empty());
        assert!(!damage.is_full());
    }

    #[test]
    fn the_backend_region_tuple_is_forwarded_without_conversion() {
        let mut damage = PresentDamage::new();
        damage.mark_regions(&[(0, 8, 4, 4), (0, 40, 4, 4)], 100);
        damage.finish();
        assert_eq!(damage.bands().len(), 2);
    }
}
