//! Splitter widget.

use crate::core::Rect;
use crate::object::ObjectId;
use crate::signal::Signal1;
use crate::widget::base::{BaseWidget, Widget, WidgetKind};

/// Splitter orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterOrientation {
    Horizontal,
    Vertical,
}

/// Splitter widget with deterministic pane-ratio distribution contract.
pub struct Splitter {
    base: BaseWidget,
    orientation: SplitterOrientation,
    panes: Vec<ObjectId>,
    ratios: Vec<f32>,
    pub pane_layout_changed: Signal1<Vec<f32>>,
    pub orientation_changed: Signal1<SplitterOrientation>,
}

impl Splitter {
    /// Creates an empty splitter with horizontal orientation.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Splitter, geometry, "Splitter"),
            orientation: SplitterOrientation::Horizontal,
            panes: Vec::new(),
            ratios: Vec::new(),
            pane_layout_changed: Signal1::new(),
            orientation_changed: Signal1::new(),
        }
    }

    /// Returns splitter orientation.
    pub fn orientation(&self) -> SplitterOrientation {
        self.orientation
    }

    /// Sets splitter orientation and emits change signal on transition.
    pub fn set_orientation(&mut self, orientation: SplitterOrientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        self.orientation_changed.emit(orientation);
    }

    /// Returns pane count.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Returns pane ids in stable order.
    pub fn pane_ids(&self) -> &[ObjectId] {
        &self.panes
    }

    /// Returns ratio for pane index.
    pub fn ratio(&self, index: usize) -> Option<f32> {
        self.ratios.get(index).copied()
    }

    /// Adds one pane and returns assigned index.
    pub fn add_pane(&mut self, pane_id: ObjectId, stretch: u32) -> usize {
        self.panes.push(pane_id);
        self.ratios.push((stretch.max(1)) as f32);
        self.pane_layout_changed.emit(self.ratios.clone());
        self.panes.len() - 1
    }

    /// Removes one pane by object id.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        let Some(index) = self.panes.iter().position(|id| *id == pane_id) else {
            return false;
        };
        self.panes.remove(index);
        self.ratios.remove(index);
        self.pane_layout_changed.emit(self.ratios.clone());
        true
    }

    /// Sets ratio for pane index.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) -> bool {
        if index >= self.ratios.len() {
            return false;
        }
        self.ratios[index] = ratio.max(0.0);
        self.pane_layout_changed.emit(self.ratios.clone());
        true
    }

    /// Sets all pane ratios.
    pub fn set_ratios(&mut self, ratios: Vec<f32>) -> bool {
        if ratios.len() != self.ratios.len() {
            return false;
        }
        self.ratios = ratios.into_iter().map(|r| r.max(0.0)).collect();
        self.pane_layout_changed.emit(self.ratios.clone());
        true
    }

    /// Normalizes ratios to sum to 1.0.
    pub fn normalize_ratios(&mut self) {
        let sum: f32 = self.ratios.iter().sum();
        if sum > 0.0 {
            for ratio in &mut self.ratios {
                *ratio /= sum;
            }
        }
    }
}

impl Widget for Splitter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl crate::widget::base::Draw for Splitter {
    fn draw(&self, canvas: &mut dyn crate::render::Canvas) {
        // Default drawing implementation
        // Splitter handles are drawn by the renderer
    }
}

impl crate::event::EventHandler for Splitter {
    fn handle_event(&mut self, event: &crate::event::Event) -> bool {
        // Default event handling
        false
    }
}