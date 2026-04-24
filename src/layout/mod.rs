//! Layout managers.
pub mod absolute;
pub mod box_layout;
pub mod flow;
pub mod form;
pub mod grid;
pub mod inspector;
pub mod splitter;
pub mod stack;
pub use crate::core::Orientation;
use crate::core::{ObjectId, Point, Rect, Size};
pub use absolute::*;
pub use box_layout::*;
pub use flow::*;
pub use form::*;
pub use grid::*;
pub use splitter::*;
pub use stack::*;
/// Space allocation preference used by layout items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizePolicy {
    /// Use fixed size defined by constraints.
    Fixed,
    /// Prefer natural size while allowing negotiation.
    Preferred,
    /// Expand to consume remaining space.
    Expanding,
}
/// Min/max limits applied during layout calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConstraints {
    /// Minimum major-axis size.
    pub min: u32,
    /// Optional maximum major-axis size.
    pub max: Option<u32>,
}
impl LayoutConstraints {
    /// Creates new layout constraints.
    pub fn new(min: u32, max: Option<u32>) -> Self {
        Self { min, max }
    }
}
/// Common interface implemented by all layout managers.
pub trait Layout {
    /// Add widget into layout with optional stretch factor.
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    /// Remove widget from layout.
    fn remove_widget(&mut self, widget_id: ObjectId);
    /// Recompute child geometries within given rect.
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
    /// Recompute child geometries from explicit position/size primitives.
    fn update_from_position_size(
        &self,
        position: Point,
        size: Size,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(Rect::from_position_size(position, size), widgets);
    }
    /// Enables downcasting from `dyn Layout` to concrete types.
    /// Required by the layout inspector for introspection.
    fn as_any(&self) -> &dyn std::any::Any;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn box_layout_applies_constraints() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_constraints(1, LayoutConstraints::new(80, Some(80)));
        layout.set_size_policy(1, SizePolicy::Fixed);
        let mut rects = std::collections::HashMap::new();
        layout.update(Rect::new(0, 0, 200, 40), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.get(&1).map(|rect| rect.width), Some(80));
    }
    #[test]
    fn splitter_layout_distributes_space() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 0);
        splitter.add_widget(1, 1);
        splitter.add_widget(2, 3);
        let mut rects = std::collections::HashMap::new();
        splitter.update(Rect::new(0, 0, 400, 40), &mut |id, rect| {
            rects.insert(id, rect);
        });
        let left = rects.get(&1).map(|rect| rect.width).unwrap_or(0);
        let right = rects.get(&2).map(|rect| rect.width).unwrap_or(0);
        assert!(right > left);
    }
    #[test]
    fn layout_update_from_position_size_routes_through_rect_conversion() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(42, 1);
        let mut out = None;
        layout.update_from_position_size(Point::new(9, 11), Size::new(30, 12), &mut |id, rect| {
            if id == 42 {
                out = Some(rect);
            }
        });
        assert_eq!(out, Some(Rect::new(9, 11, 30, 12)));
    }
    #[test]
    fn hbox_and_vbox_named_types_delegate_to_box_layout_contract() {
        let mut hbox = HBoxLayout::new(3, 2);
        hbox.add_widget(1, 1);
        hbox.add_spacer(1);
        hbox.add_widget(2, 2);
        assert_eq!(hbox.spacing(), 3);
        assert_eq!(hbox.margin(), 2);
        assert_eq!(hbox.item_count(), 3);
        let mut rects = std::collections::HashMap::new();
        hbox.update(Rect::new(0, 0, 120, 20), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.len(), 2);
        let mut vbox = VBoxLayout::new(1, 0);
        vbox.add_widget(10, 1);
        vbox.add_widget(11, 1);
        let mut out = std::collections::HashMap::new();
        vbox.update(Rect::new(0, 0, 20, 40), &mut |id, rect| {
            out.insert(id, rect);
        });
        assert_eq!(out.len(), 2);
        assert!(
            out.get(&11).map(|r| r.y).unwrap_or_default()
                > out.get(&10).map(|r| r.y).unwrap_or_default()
        );
    }
    #[test]
    fn box_layout_distribution_consumes_available_major_axis() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.add_widget(3, 1);
        let mut widths = std::collections::HashMap::new();
        layout.update(Rect::new(0, 0, 100, 10), &mut |id, rect| {
            widths.insert(id, rect.width);
        });
        let total: u32 = widths.values().copied().sum();
        assert_eq!(total, 100);
    }
    #[test]
    fn grid_and_stack_layouts_have_deterministic_placement() {
        let mut grid = GridLayout::new(2, 2, 0, 0);
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 1, 2);
        let mut grid_rects = std::collections::HashMap::new();
        grid.update(Rect::new(0, 0, 40, 20), &mut |id, rect| {
            grid_rects.insert(id, rect);
        });
        assert_eq!(grid_rects.get(&1), Some(&Rect::new(0, 0, 20, 10)));
        assert_eq!(grid_rects.get(&2), Some(&Rect::new(20, 10, 20, 10)));
        let mut stack = StackLayout::new();
        stack.add_widget(7, 0);
        stack.add_widget(8, 0);
        stack.set_current_index(1);
        let mut shown = None;
        stack.update(Rect::new(1, 2, 30, 40), &mut |id, rect| {
            shown = Some((id, rect));
        });
        assert_eq!(shown, Some((8, Rect::new(1, 2, 30, 40))));
    }
}
