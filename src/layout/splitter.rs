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
    pub fn ratios(&self) -> &[f32] {
        &self.ratios
    }

    /// Returns ratio for pane index.
    pub fn ratio(&self, index: usize) -> Option<f32> {
        self.ratios.get(index).copied()
    }

    /// Adds one pane and returns assigned index.
    pub fn add_pane(&mut self, pane_id: ObjectId, stretch: u32) -> usize {
        self.panes.push(pane_id);
        self.ratios.push((stretch.max(1)) as f32);
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
        self.ratios[index] = ratio.max(0.0);
        true
    }

    /// Sets all pane ratios. Returns false if length mismatch.
    pub fn set_ratios(&mut self, ratios: Vec<f32>) -> bool {
        if ratios.len() != self.ratios.len() {
            return false;
        }
        self.ratios = ratios.into_iter().map(|r| r.max(0.0)).collect();
        true
    }

    /// Normalizes ratios to sum to 1.
    pub fn normalize_ratios(&mut self) {
        let sum: f32 = self.ratios.iter().sum();
        if sum > 0.0 {
            for ratio in &mut self.ratios {
                *ratio /= sum;
            }
        }
    }
}

impl Layout for SplitterLayout {
    fn as_any(&self) -> &dyn std::any::Any {
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
