//! Splitter widget.
use crate::core::Rect;
use crate::object::ObjectId;
use crate::render::RenderContext;
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
impl Widget for Splitter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl crate::widget::base::Draw for Splitter {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw splitter handles between panes
        let rect = self.base.geometry();
        let handle_width = 5;
        match self.orientation {
            SplitterOrientation::Horizontal => {
                // Draw vertical splitter handles
                if self.pane_count() > 1 {
                    let total_width = rect.width as f32;
                    let mut x = rect.x as f32;
                    for i in 0..self.pane_count() - 1 {
                        let ratio = self.ratio(i).unwrap_or(0.0);
                        x += total_width * ratio;
                        let handle_rect = Rect::new(
                            x as i32 - handle_width as i32 / 2,
                            rect.y,
                            handle_width as u32,
                            rect.height,
                        );
                        // Draw splitter handle
                        context.fill_rect(handle_rect, crate::core::Color::from_rgb(200, 200, 200));
                        context.draw_rect(handle_rect, crate::core::Color::from_rgb(150, 150, 150));
                    }
                }
            }
            SplitterOrientation::Vertical => {
                // Draw horizontal splitter handles
                if self.pane_count() > 1 {
                    let total_height = rect.height as f32;
                    let mut y = rect.y as f32;
                    for i in 0..self.pane_count() - 1 {
                        let ratio = self.ratio(i).unwrap_or(0.0);
                        y += total_height * ratio;
                        let handle_rect = Rect::new(
                            rect.x,
                            y as i32 - handle_width as i32 / 2,
                            rect.width,
                            handle_width as u32,
                        );
                        // Draw splitter handle
                        context.fill_rect(handle_rect, crate::core::Color::from_rgb(200, 200, 200));
                        context.draw_rect(handle_rect, crate::core::Color::from_rgb(150, 150, 150));
                    }
                }
            }
        }
    }
}
impl crate::event::EventHandler for Splitter {
    fn handle_event(&mut self, event: &crate::event::Event) {
        // Default event handling
        let _ = event;
    }
}
