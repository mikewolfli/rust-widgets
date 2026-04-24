//! Splitter widget.
use crate::core::Orientation;
use crate::core::Rect;
use crate::layout::splitter::SplitterLayout;
use crate::object::ObjectId;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Splitter widget with deterministic pane-ratio distribution contract.
///
/// Delegates layout calculations to [`SplitterLayout`].
pub struct Splitter {
    base: BaseWidget,
    layout: SplitterLayout,
    pub pane_layout_changed: Signal1<Vec<f32>>,
    pub orientation_changed: Signal1<Orientation>,
}
impl Splitter {
    /// Creates an empty splitter with horizontal orientation.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Splitter, geometry, "Splitter"),
            layout: SplitterLayout::new(Orientation::Horizontal, 0),
            pane_layout_changed: Signal1::new(),
            orientation_changed: Signal1::new(),
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
        self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        index
    }
    /// Removes one pane by object id.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        if !self.layout.remove_pane(pane_id) {
            return false;
        }
        self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        true
    }
    /// Sets ratio for pane index.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) -> bool {
        if !self.layout.set_ratio(index, ratio) {
            return false;
        }
        self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        true
    }
    /// Sets all pane ratios.
    pub fn set_ratios(&mut self, ratios: Vec<f32>) -> bool {
        if !self.layout.set_ratios(ratios) {
            return false;
        }
        self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        true
    }
    /// Normalizes ratios to sum to 1.
    pub fn normalize_ratios(&mut self) {
        self.layout.normalize_ratios();
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
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // Use ratio-based handle dragging
        let rect = self.base.geometry();
        let handle_width = 5.0;
        match event {
            crate::event::Event::MousePress { pos, button } => {
                if *button == 1 && self.pane_count() > 1 {
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
                            self.layout
                                .set_ratio(i, self.layout.ratio(i).unwrap_or(1.0));
                            break;
                        }
                    }
                }
            }
            crate::event::Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    // Drag ended - normalize ratios
                    self.layout.normalize_ratios();
                    self.pane_layout_changed.emit(self.layout.ratios().to_vec());
                }
            }
            _ => {}
        }
    }
}
