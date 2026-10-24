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
    /// Minimum extent each pane may be reduced to.
    min_sizes: Vec<u32>,
    /// Whether each pane may collapse to zero when its ratio reaches zero.
    collapsible: Vec<bool>,
}

impl SplitterLayout {
    /// Creates a splitter layout with orientation and pane spacing.
    pub fn new(orientation: Orientation, spacing: u32) -> Self {
        Self {
            orientation,
            spacing,
            panes: Vec::new(),
            ratios: Vec::new(),
            min_sizes: Vec::new(),
            collapsible: Vec::new(),
        }
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
        self.min_sizes.push(0);
        self.collapsible.push(true);
        self.panes.len().saturating_sub(1)
    }

    /// Removes one pane by object id. Returns false if not found.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        let Some(index) = self.panes.iter().position(|id| *id == pane_id) else {
            return false;
        };
        self.panes.remove(index);
        self.ratios.remove(index);
        self.min_sizes.remove(index);
        self.collapsible.remove(index);
        true
    }

    /// Sets the minimum extent of pane `index`.
    ///
    /// Returns `false` when the index is out of range. A minimum is honoured during
    /// distribution: when every minimum cannot be paid, the panes are scaled down
    /// proportionally rather than one pane absorbing the whole shortfall.
    pub fn set_min_size(&mut self, index: usize, minimum: u32) -> bool {
        match self.min_sizes.get_mut(index) {
            Some(slot) => {
                *slot = minimum;
                true
            }
            None => false,
        }
    }

    /// The minimum extent of pane `index`, or `0` when out of range.
    pub fn min_size(&self, index: usize) -> u32 {
        self.min_sizes.get(index).copied().unwrap_or(0)
    }

    /// Sets whether pane `index` may collapse to zero width/height.
    ///
    /// A non-collapsible pane is floored at its minimum extent even when its ratio is
    /// zero. Returns `false` when the index is out of range.
    pub fn set_collapsible(&mut self, index: usize, collapsible: bool) -> bool {
        match self.collapsible.get_mut(index) {
            Some(slot) => {
                *slot = collapsible;
                true
            }
            None => false,
        }
    }

    /// Whether pane `index` may collapse.
    pub fn collapsible(&self, index: usize) -> bool {
        self.collapsible.get(index).copied().unwrap_or(true)
    }

    /// Serialises the pane layout (ratios, minimums and collapse flags).
    ///
    /// The format is versioned, so a future field can be added without a restore misreading
    /// it. Pane ids are not stored: a restored layout applies to the panes the caller adds.
    pub fn save_state(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(9 + self.panes.len() * 9);
        out.extend_from_slice(b"SPL1");
        out.extend_from_slice(&(self.panes.len() as u32).to_le_bytes());
        for ratio in &self.ratios {
            out.extend_from_slice(&ratio.to_bits().to_le_bytes());
        }
        for minimum in &self.min_sizes {
            out.extend_from_slice(&minimum.to_le_bytes());
        }
        for collapsible in &self.collapsible {
            out.push(u8::from(*collapsible));
        }
        out
    }

    /// Restores a state produced by [`Self::save_state`].
    ///
    /// Returns `false` for a malformed blob or a pane count that does not match this
    /// layout's, leaving the current state untouched.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        const HEADER: usize = 8;
        if state.len() < HEADER || &state[..4] != b"SPL1" {
            return false;
        }
        let count = u32::from_le_bytes([state[4], state[5], state[6], state[7]]) as usize;
        if count != self.panes.len() {
            return false;
        }
        let expected = HEADER + count * (4 + 4 + 1);
        if state.len() != expected {
            return false;
        }
        let mut cursor = HEADER;
        let mut ratios = Vec::with_capacity(count);
        for _ in 0..count {
            let bytes = [state[cursor], state[cursor + 1], state[cursor + 2], state[cursor + 3]];
            ratios.push(f32::from_bits(u32::from_le_bytes(bytes)));
            cursor += 4;
        }
        let mut minimums = Vec::with_capacity(count);
        for _ in 0..count {
            let bytes = [state[cursor], state[cursor + 1], state[cursor + 2], state[cursor + 3]];
            minimums.push(u32::from_le_bytes(bytes));
            cursor += 4;
        }
        let mut collapsible = Vec::with_capacity(count);
        for _ in 0..count {
            collapsible.push(state[cursor] != 0);
            cursor += 1;
        }
        self.ratios = ratios;
        self.min_sizes = minimums;
        self.collapsible = collapsible;
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
        self.min_sizes.clear();
        self.collapsible.clear();
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

        // Floors first, then the remaining space by weight. Stating it in this order is
        // what lets a minimum coexist with a weight: the minimum is paid before any pane
        // is given its proportional share, and a zero-ratio non-collapsible pane still
        // occupies its floor.
        let floors: Vec<u32> = (0..count)
            .map(|index| {
                let minimum = self.min_sizes.get(index).copied().unwrap_or(0);
                if self.collapsible.get(index).copied().unwrap_or(true) {
                    minimum
                } else {
                    minimum.max(1)
                }
            })
            .collect();
        let total_floor: u64 = floors.iter().map(|floor| *floor as u64).sum();
        let mut major: Vec<u32> = Vec::with_capacity(count);
        if total_floor > pane_budget as u64 {
            // The floors themselves do not fit; scale them proportionally. Every pane then
            // falls short by the same fraction, which is order-independent.
            let mut consumed = 0u32;
            for (index, floor) in floors.iter().enumerate() {
                let share = if index + 1 == count {
                    pane_budget.saturating_sub(consumed)
                } else {
                    (pane_budget as u64 * *floor as u64 / total_floor) as u32
                };
                let share = share.min(pane_budget.saturating_sub(consumed));
                consumed = consumed.saturating_add(share);
                major.push(share);
            }
        } else {
            let remaining = pane_budget - total_floor as u32;
            let mut fractional: Vec<(usize, f64)> = Vec::with_capacity(count);
            let mut assigned = 0u32;
            for (index, floor) in floors.iter().enumerate() {
                let ratio = self.ratios.get(index).copied().unwrap_or(1.0) as f64 / total_ratio;
                let exact = remaining as f64 * ratio;
                let whole = exact.floor() as u32;
                major.push(floor.saturating_add(whole));
                assigned = assigned.saturating_add(whole);
                fractional.push((index, exact - exact.floor()));
            }
            fractional.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
            let mut leftover = remaining.saturating_sub(assigned);
            for (index, _) in fractional {
                if leftover == 0 {
                    break;
                }
                major[index] += 1;
                leftover -= 1;
            }
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

    /// Minimums, collapse flags and the saved state survive a round trip.
    #[test]
    fn minimums_collapse_and_state_round_trip() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 0);
        splitter.add_pane(1, 1);
        splitter.add_pane(2, 1);
        assert!(splitter.set_min_size(0, 40));
        assert!(splitter.set_collapsible(1, false));
        assert!(!splitter.set_min_size(9, 1));
        let mut rects = HashMap::new();
        splitter.update(Rect::new(0, 0, 100, 20), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.get(&1).map(|r| r.width), Some(70), "the minimum is paid first");
        assert_eq!(rects.get(&2).map(|r| r.width), Some(30), "the rest is split by weight");

        let state = splitter.save_state();
        let mut restored = SplitterLayout::new(Orientation::Horizontal, 0);
        restored.add_pane(10, 1);
        restored.add_pane(20, 1);
        assert!(restored.restore_state(&state));
        assert_eq!(restored.min_size(0), 40);
        assert!(!restored.collapsible(1));
        assert!(!restored.restore_state(b"nope"));

        splitter.set_min_size(1, 80);
        splitter.set_ratio(1, 0.0);
        splitter.set_ratio(0, 1.0);
        let mut rects = HashMap::new();
        splitter.update(Rect::new(0, 0, 100, 20), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.get(&1).map(|r| r.width), Some(33));
        assert_eq!(
            rects.get(&2).map(|r| r.width),
            Some(67),
            "floors that do not fit are scaled proportionally, never past the parent"
        );
    }
}
