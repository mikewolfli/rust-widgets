// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Splitter layout manager — distributes space by pane ratios.
use super::{Layout, Orientation};
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
    pub fn normalize_ratios(&mut self) {
        let sum: f32 = self.ratios.iter().filter(|value| value.is_finite()).sum();
        if sum > 0.0 {
            for ratio in &mut self.ratios {
                *ratio = if ratio.is_finite() { *ratio / sum } else { 0.0 };
            }
        }
    }
}

impl Layout for SplitterLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.panes.push(widget_id);
        self.ratios.push((stretch.max(1) as f32).max(0.01));
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        if let Some(index) = self.panes.iter().position(|id| *id == widget_id) {
            self.panes.remove(index);
            self.ratios.remove(index);
        }
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

        let total_ratio = self.ratios.iter().copied().sum::<f32>().max(0.01);
        let gaps = (self.panes.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width.saturating_sub(gaps * self.spacing),
            Orientation::Vertical => rect.height.saturating_sub(gaps * self.spacing),
        };

        let mut cursor_x = rect.x;
        let mut cursor_y = rect.y;

        for (index, pane) in self.panes.iter().enumerate() {
            let ratio = self.ratios.get(index).copied().unwrap_or(1.0) / total_ratio;
            let major = ((primary as f32) * ratio).max(1.0) as u32;

            let pane_rect = match self.orientation {
                Orientation::Horizontal => Rect::new(cursor_x, rect.y, major, rect.height),
                Orientation::Vertical => Rect::new(rect.x, cursor_y, rect.width, major),
            };

            widgets(*pane, pane_rect);

            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + self.spacing) as i32,
                Orientation::Vertical => cursor_y += (major + self.spacing) as i32,
            }
        }
    }
}
