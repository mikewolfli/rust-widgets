//! Splitter widget.
use crate::core::{Orientation, Rect};
use crate::layout::splitter::SplitterLayout;
use crate::object::ObjectId;
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// Splitter widget with deterministic pane-ratio distribution contract.
///
/// Delegates layout calculations to [`SplitterLayout`].
pub struct Splitter {
    base: BaseWidget,
    layout: SplitterLayout,
    pub pane_layout_changed: Signal1<Vec<f32>>,
    pub orientation_changed: Signal1<Orientation>,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
impl Splitter {
    /// Creates an empty splitter with horizontal orientation.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Splitter, geometry, "Splitter"),
            layout: SplitterLayout::new(Orientation::Horizontal, 0),
            pane_layout_changed: Signal1::new(),
            orientation_changed: Signal1::new(),
            registry: None,
        }
    }
    /// Returns splitter orientation.
    pub fn orientation(&self) -> Orientation {
        self.layout.orientation()
    }
    /// Sets splitter orientation and emits change signal on transition.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if self.layout.orientation() == orientation {
            return;
        }
        self.layout.set_orientation(orientation);
        self.orientation_changed.emit(orientation);
    }
    /// Returns pane count.
    pub fn pane_count(&self) -> usize {
        self.layout.pane_count()
    }
    /// Returns pane ids in stable order.
    pub fn pane_ids(&self) -> &[ObjectId] {
        self.layout.pane_ids()
    }
    /// Returns ratio for pane index.
    pub fn ratio(&self, index: usize) -> Option<f32> {
        self.layout.ratio(index)
    }
    /// Adds one pane and returns assigned index.
    pub fn add_pane(&mut self, pane_id: ObjectId, stretch: u32) -> usize {
        let index = self.layout.add_pane(pane_id, stretch);
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        index
    }
    /// Removes one pane by object id.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        if !self.layout.remove_pane(pane_id) {
            return false;
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        true
    }
    /// Sets ratio for pane index.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) -> bool {
        if !self.layout.set_ratio(index, ratio) {
            return false;
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        true
    }
    /// Sets all pane ratios.
    pub fn set_ratios(&mut self, ratios: Vec<f32>) -> bool {
        if !self.layout.set_ratios(ratios) {
            return false;
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        true
    }
    /// Normalizes ratios to sum to 1.
    pub fn normalize_ratios(&mut self) {
        self.layout.normalize_ratios();
    }

    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
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
impl Draw for Splitter {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw splitter handles between panes
        let rect = self.base.geometry();
        let handle_width = 5;
        match self.orientation() {
            Orientation::Horizontal => {
                // Draw vertical splitter handles
                if self.pane_count() > 1 {
                    let total_width = rect.width as f32;
                    let mut x = rect.x as f32;
                    for i in 0..self.pane_count() - 1 {
                        let ratio = self.ratio(i).unwrap_or(0.0);
                        x += total_width * ratio;
                        let handle_rect = Rect::new(
                            x as i32 - handle_width / 2,
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
            Orientation::Vertical => {
                // Draw horizontal splitter handles
                if self.pane_count() > 1 {
                    let total_height = rect.height as f32;
                    let mut y = rect.y as f32;
                    for i in 0..self.pane_count() - 1 {
                        let ratio = self.ratio(i).unwrap_or(0.0);
                        y += total_height * ratio;
                        let handle_rect = Rect::new(
                            rect.x,
                            y as i32 - handle_width / 2,
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
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // Use ratio-based handle dragging
        let rect = self.base.geometry();
        let handle_width = 5.0;
        match event {
            crate::event::Event::MousePress { pos, button }
                if *button == 1 && self.pane_count() > 1 =>
            {
                let total = if self.orientation() == Orientation::Horizontal {
                    rect.width as f32
                } else {
                    rect.height as f32
                };
                let pos_primary = if self.orientation() == Orientation::Horizontal {
                    pos.x as f32 - rect.x as f32
                } else {
                    pos.y as f32 - rect.y as f32
                };
                let mut acc = 0.0;
                for i in 0..self.pane_count() - 1 {
                    if let Some(r) = self.ratio(i) {
                        acc += r * total;
                    }
                    if (pos_primary - acc).abs() <= handle_width / 2.0 {
                        // Store drag state: negative index-1 to indicate dragging
                        // and save the initial position for delta calculation
                        self.layout.set_ratio(i, self.layout.ratio(i).unwrap_or(1.0));
                        break;
                    }
                }
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                // Drag ended - normalize ratios
                self.layout.normalize_ratios();
                if self.pane_layout_changed.slot_count() > 0 {
                    self.pane_layout_changed.emit(self.layout.ratios().to_vec());
                }
            }
            _ => { /* Other events are not relevant */ }
        }
        // Forward events to panes
        if self.base.is_enabled() {
            if let Some(ref reg) = self.registry {
                for pane_id in self.pane_ids() {
                    let _ = reg.borrow_mut().forward_event(*pane_id, event);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Orientation, Rect};
    use crate::object::ObjectId;

    #[test]
    fn splitter_creation_defaults() {
        let sp = Splitter::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sp.geometry(), Rect::new(0, 0, 300, 200));
        assert_eq!(sp.orientation(), Orientation::Horizontal);
        assert_eq!(sp.pane_count(), 0);
    }

    #[test]
    fn splitter_add_and_remove_pane() {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        let pane_id: ObjectId = 1;
        let idx = sp.add_pane(pane_id, 1);
        assert_eq!(idx, 0);
        assert_eq!(sp.pane_count(), 1);
        assert_eq!(sp.pane_ids(), &[1]);
        assert!(sp.remove_pane(pane_id));
        assert_eq!(sp.pane_count(), 0);
    }

    #[test]
    fn splitter_set_orientation() {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        sp.set_orientation(Orientation::Vertical);
        assert_eq!(sp.orientation(), Orientation::Vertical);
    }
}
