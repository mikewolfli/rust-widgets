// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Splitter layout manager — distributes space by pane ratios.
use super::{Layout, Orientation};
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect};

/// Splitter-like layout distributing space by pane ratios.
pub struct SplitterLayout {
    orientation: Orientation,
    spacing: u32,
    panes: Vec<ObjectId>,
    ratios: Vec<f32>,
}

impl SplitterLayout {
    /// Creates a splitter layout with orientation and pane spacing.
    pub fn new(orientation: Orientation, spacing: u32) -> Self {
        Self { orientation, spacing, panes: Vec::new(), ratios: Vec::new() }
    }

    /// Returns layout orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Sets layout orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }

    /// Returns pane count.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Returns pane ids in stable order.
    pub fn pane_ids(&self) -> &[ObjectId] {
        &self.panes
    }

    /// Returns current pane ratios slice.
    ///
    /// # What the entries are
    ///
    /// **Relative weights**, not fractions: the slice need not sum to `1.0`. Each pane's
    /// share of the splitter is `weights[i] / sum(weights)`, which is what `update`
    /// computes. The weights are preserved during a drag (so only the dragged pair moves)
    /// and normalised on release by [`Self::normalize_ratios`]. Reading the slice as a
    /// fraction is only valid after a release or a call to `normalize_ratios`.
    pub fn ratios(&self) -> &[f32] {
        &self.ratios
    }

    /// Returns the relative weight for pane index.
    pub fn ratio(&self, index: usize) -> Option<f32> {
        self.ratios.get(index).copied()
    }

    /// Adds one pane and returns assigned index.
    ///
    /// `stretch` is a relative weight, matching [`Self::add_widget`]. The weight is stored
    /// as given (floored at `0.01` so a pane is never allocated exactly nothing); it is
    /// normalised by [`Self::normalize_ratios`], not here, so that adding a pane does not
    /// rescale the panes already present during an in-flight drag.
    pub fn add_pane(&mut self, pane_id: ObjectId, stretch: u32) -> usize {
        self.panes.push(pane_id);
        self.ratios.push((stretch.max(1) as f32).max(0.01));
        self.panes.len().saturating_sub(1)
    }

    /// Removes one pane by object id. Returns false if not found.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        let Some(index) = self.panes.iter().position(|id| *id == pane_id) else {
            return false;
        };
        self.panes.remove(index);
        self.ratios.remove(index);
        true
    }

    /// Sets ratio for pane index. Returns false if index out of range.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) -> bool {
        if index >= self.ratios.len() {
            return false;
        }
        self.ratios[index] = if ratio.is_finite() { ratio.max(0.0) } else { 0.0 };
        true
    }

    /// Sets all pane ratios. Returns false if length mismatch.
    pub fn set_ratios(&mut self, ratios: Vec<f32>) -> bool {
        if ratios.len() != self.ratios.len() {
            return false;
        }
        self.ratios =
            ratios.into_iter().map(|r| if r.is_finite() { r.max(0.0) } else { 0.0 }).collect();
        true
    }

    /// Normalizes ratios to sum to 1.
    ///
    /// The stored values are relative weights (see [`Self::ratios`]), and this turns them
    /// into fractions. A total of zero — every pane collapsed — is left as it is rather
    /// than divided by zero; `update` falls back to its own `max(0.01)` divisor, so the
    /// panes stay addressable.
    ///
    /// The sum is accumulated in `f64`: an `f32` accumulator overflows to infinity for
    /// finite-but-huge weights such as `[f32::MAX, f32::MAX]`, which then collapsed both
    /// panes to a zero share (S-48).
    pub fn normalize_ratios(&mut self) {
        let sum: f64 =
            self.ratios.iter().filter(|value| value.is_finite()).map(|r| *r as f64).sum();
        if sum > 0.0 {
            for ratio in &mut self.ratios {
                *ratio = if ratio.is_finite() { (*ratio as f64 / sum) as f32 } else { 0.0 };
            }
        }
    }
}

impl Layout for SplitterLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        // One body, one meaning: the trait method and `add_pane` must not be able to drift
        // apart. The module's own regression test pins that they agree, and the cheapest way
        // to keep that true forever is to have only one of them push the panes.
        self.add_pane(widget_id, stretch);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.remove_pane(widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.panes.clone()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.panes.contains(&id)
    }

    fn clear(&mut self) {
        self.panes.clear();
        self.ratios.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.panes.is_empty() {
            return;
        }

        let count = self.panes.len();
        let gaps = (count.saturating_sub(1)) as u32;
        let budget = match self.orientation {
            Orientation::Horizontal => rect.width,
            Orientation::Vertical => rect.height,
        };

        // S-49: the spacing is honoured only as far as the budget can pay for it. Reducing it
        // before deriving the pane budget keeps the cursor step and the pane allocation on one
        // number, so a tiny box can no longer step past its own far edge.
        let effective_spacing = budget.checked_div(gaps).map_or(0, |q| self.spacing.min(q));
        let pane_budget = budget.saturating_sub(gaps * effective_spacing);

        // S-48: sum the weights in f64 so finite-but-huge f32 weights cannot overflow to
        // infinity and collapse every pane to a zero share.
        let total_ratio = self.ratios.iter().map(|r| *r as f64).sum::<f64>().max(0.01);

        // Distribute `pane_budget` proportionally, sharing the integer rounding remainder to the
        // largest fractional parts. The panes sum to exactly `pane_budget` — never more — which is
        // what keeps the run inside the parent without a per-pane `max(1.0)` floor that could
        // push the last panes past the edge.
        let mut major: Vec<u32> = Vec::with_capacity(count);
        let mut assigned: u32 = 0;
        let mut fractional: Vec<(usize, f64)> = Vec::with_capacity(count);
        for (index, _pane) in self.panes.iter().enumerate() {
            let ratio = self.ratios.get(index).copied().unwrap_or(1.0) as f64 / total_ratio;
            let exact = pane_budget as f64 * ratio;
            let whole = exact.floor() as u32;
            major.push(whole);
            assigned += whole;
            fractional.push((index, exact - exact.floor()));
        }
        fractional.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        let mut remainder = pane_budget.saturating_sub(assigned);
        for (index, _) in fractional {
            if remainder == 0 {
                break;
            }
            major[index] += 1;
            remainder -= 1;
        }

        let mut cursor_x = rect.x;
        let mut cursor_y = rect.y;

        for (index, pane) in self.panes.iter().enumerate() {
            let major = major[index];
            let pane_rect = match self.orientation {
                Orientation::Horizontal => Rect::new(cursor_x, rect.y, major, rect.height),
                Orientation::Vertical => Rect::new(rect.x, cursor_y, rect.width, major),
            };

            widgets(*pane, pane_rect);

            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + effective_spacing) as i32,
                Orientation::Vertical => cursor_y += (major + effective_spacing) as i32,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;

    /// S-48: finite-but-huge weights must normalise to real fractions rather than overflow
    /// the f32 sum to infinity and collapse every pane to zero.
    #[test]
    fn huge_finite_ratios_normalize_stably() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 0);
        splitter.add_pane(1, 1);
        splitter.add_pane(2, 1);
        assert!(splitter.set_ratios(vec![f32::MAX, f32::MAX]));

        splitter.normalize_ratios();
        assert!((splitter.ratio(0).unwrap() - 0.5).abs() < 1e-6);
        assert!((splitter.ratio(1).unwrap() - 0.5).abs() < 1e-6);

        let mut rects = HashMap::new();
        splitter.update(Rect::new(0, 0, 100, 40), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.get(&1).map(|r| r.width), Some(50));
        assert_eq!(rects.get(&2).map(|r| r.width), Some(50));
    }

    /// S-48: a zero sum is left as-is rather than divided by zero.
    #[test]
    fn zero_sum_ratios_stay_zero_sum_after_normalizing() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 0);
        splitter.add_pane(1, 1);
        splitter.add_pane(2, 1);
        assert!(splitter.set_ratios(vec![0.0, 0.0]));
        splitter.normalize_ratios();
        assert_eq!(splitter.ratio(0).unwrap(), 0.0);
        assert_eq!(splitter.ratio(1).unwrap(), 0.0);
    }

    /// S-49: a tiny box reduces the spacing so the panes never step past the parent.
    #[test]
    fn tiny_box_keeps_panes_inside_with_spacing() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 10);
        splitter.add_pane(1, 1);
        splitter.add_pane(2, 1);
        splitter.add_pane(3, 1);

        let mut rects = HashMap::new();
        splitter.update(Rect::new(0, 0, 5, 40), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.len(), 3);
        for (id, rect) in &rects {
            assert!(
                rect.x >= 0 && rect.x + rect.width as i32 <= 5,
                "pane {id} escaped the 5px box: {rect:?}"
            );
        }
    }

    /// S-49 (vertical): the same one-budget policy holds for the vertical orientation.
    #[test]
    fn tiny_box_keeps_panes_inside_vertically() {
        let mut splitter = SplitterLayout::new(Orientation::Vertical, 10);
        splitter.add_pane(1, 1);
        splitter.add_pane(2, 1);
        splitter.add_pane(3, 1);

        let mut rects = HashMap::new();
        splitter.update(Rect::new(0, 0, 40, 5), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.len(), 3);
        for (id, rect) in &rects {
            assert!(
                rect.y >= 0 && rect.y + rect.height as i32 <= 5,
                "pane {id} escaped the 5px box: {rect:?}"
            );
        }
    }
}
