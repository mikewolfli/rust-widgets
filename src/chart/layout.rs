//! Chart layout manager that positions a chart widget to fill available space.
//!
//! `ChartLayout` is a simple single-child layout that places a chart widget
//! at the given [`ObjectId`] so that it occupies the entire allocated rectangle.
//! It implements the [`Layout`] trait and supports the standard lifecycle:
//! `add_widget`, `remove_widget`, `child_ids`, `has_child`, and `clear`.

use crate::core::{ObjectId, Rect};
use crate::layout::Layout;

/// A layout that positions a single chart widget to fill all available space.
///
/// # Example
///
/// ```ignore
/// use crate::chart::layout::ChartLayout;
/// use crate::layout::Layout;
///
/// let mut layout = ChartLayout::new(42);
/// assert!(layout.has_child(42));
/// assert_eq!(layout.child_ids(), vec![42]);
/// ```
#[derive(Debug)]
pub struct ChartLayout {
    /// The widget id of the chart being managed.
    chart_id: Option<ObjectId>,
}

impl ChartLayout {
    /// Create a new `ChartLayout` that will manage the given chart widget.
    pub fn new(chart_id: ObjectId) -> Self {
        Self {
            chart_id: Some(chart_id),
        }
    }
}

impl Layout for ChartLayout {
    /// Add a widget to this layout.
    ///
    /// `ChartLayout` only tracks a single child; repeated calls replace
    /// the previously stored widget id.
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        self.chart_id = Some(widget_id);
    }

    /// Remove a widget from this layout.
    fn remove_widget(&mut self, widget_id: ObjectId) {
        if self.chart_id == Some(widget_id) {
            self.chart_id = None;
        }
    }

    /// Recompute child geometries within the given rectangle.
    ///
    /// The chart widget is positioned to occupy the entire `rect`.
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if let Some(id) = self.chart_id {
            widgets(id, rect);
        }
    }

    /// Return all child widget IDs managed by this layout.
    fn child_ids(&self) -> Vec<ObjectId> {
        self.chart_id.into_iter().collect()
    }

    /// Return `true` if the given widget ID is managed by this layout.
    fn has_child(&self, id: ObjectId) -> bool {
        self.chart_id == Some(id)
    }

    /// Remove all children from this layout.
    fn clear(&mut self) {
        self.chart_id = None;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn chart_layout_manages_single_child() {
        let mut layout = ChartLayout::new(1);
        assert!(layout.has_child(1));
        assert_eq!(layout.child_ids(), vec![1]);

        // Replace child
        layout.add_widget(2, 0);
        assert!(!layout.has_child(1));
        assert!(layout.has_child(2));
    }

    #[test]
    fn chart_layout_remove_widget() {
        let mut layout = ChartLayout::new(42);
        layout.remove_widget(42);
        assert!(!layout.has_child(42));
        assert!(layout.child_ids().is_empty());
    }

    #[test]
    fn chart_layout_update_fills_rect() {
        let layout = ChartLayout::new(99);
        let mut results = Vec::new();
        layout.update(Rect::new(10, 20, 300, 200), &mut |id, rect| {
            results.push((id, rect));
        });
        assert_eq!(results, vec![(99, Rect::new(10, 20, 300, 200))]);
    }

    #[test]
    fn chart_layout_clear() {
        let mut layout = ChartLayout::new(7);
        assert!(layout.has_child(7));
        layout.clear();
        assert!(!layout.has_child(7));
        assert!(layout.child_ids().is_empty());
    }

    #[test]
    fn chart_layout_update_empty_does_nothing() {
        let layout = ChartLayout { chart_id: None };
        let mut called = false;
        layout.update(Rect::new(0, 0, 100, 100), &mut |_id, _rect| {
            called = true;
        });
        assert!(!called);
    }
}
