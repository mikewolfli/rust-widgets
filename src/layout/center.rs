//! Center layout manager — centers a single child within the available area.
//!
//! The child is positioned at the visual center of the parent rect. Optional
//! width_factor and height_factor (0.0–1.0) control how much of the available
//! space the child consumes before centering.
use super::Layout;
use crate::core::{ObjectId, Rect};

/// A layout that centers a single child within the available rectangle.
///
/// The child receives a rect whose size is `width_factor × parent_width`
/// and `height_factor × parent_height`, centered within the parent.
#[derive(Debug)]
pub struct CenterLayout {
    child: Option<ObjectId>,
    /// Fraction of parent width used for the child (0.0–1.0).
    width_factor: f32,
    /// Fraction of parent height used for the child (0.0–1.0).
    height_factor: f32,
}

impl CenterLayout {
    /// Create a new center layout with default factors (1.0).
    pub fn new() -> Self {
        Self { child: None, width_factor: 1.0, height_factor: 1.0 }
    }

    /// Create a center layout with custom size factors.
    ///
    /// Both factors are clamped to the range `0.0..=1.0`.
    pub fn with_factors(width_factor: f32, height_factor: f32) -> Self {
        Self {
            child: None,
            width_factor: width_factor.clamp(0.0, 1.0),
            height_factor: height_factor.clamp(0.0, 1.0),
        }
    }

    /// Set the width factor (0.0–1.0) for how much of the parent width the child uses.
    pub fn set_width_factor(&mut self, factor: f32) {
        self.width_factor = factor.clamp(0.0, 1.0);
    }

    /// Set the height factor (0.0–1.0) for how much of the parent height the child uses.
    pub fn set_height_factor(&mut self, factor: f32) {
        self.height_factor = factor.clamp(0.0, 1.0);
    }

    /// Returns the current width factor.
    pub fn width_factor(&self) -> f32 {
        self.width_factor
    }

    /// Returns the current height factor.
    pub fn height_factor(&self) -> f32 {
        self.height_factor
    }

    /// Returns the child widget ID, if any.
    pub fn child(&self) -> Option<ObjectId> {
        self.child
    }
}

impl Default for CenterLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout for CenterLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        // CenterLayout manages only a single child; replace any existing child.
        self.child = Some(widget_id);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        if self.child == Some(widget_id) {
            self.child = None;
        }
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.child.into_iter().collect()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.child == Some(id)
    }

    fn clear(&mut self) {
        self.child = None;
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if let Some(child_id) = self.child {
            let child_width = (rect.width as f32 * self.width_factor) as u32;
            let child_height = (rect.height as f32 * self.height_factor) as u32;
            let x_offset = (rect.width.saturating_sub(child_width) / 2) as i32;
            let y_offset = (rect.height.saturating_sub(child_height) / 2) as i32;
            let child_rect = Rect::new(rect.x + x_offset, rect.y + y_offset, child_width, child_height);
            widgets(child_id, child_rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_full_size() {
        let mut layout = CenterLayout::new();
        layout.add_widget(42, 0);

        let mut out = None;
        layout.update(Rect::new(10, 20, 200, 100), &mut |id, rect| {
            if id == 42 {
                out = Some(rect);
            }
        });

        // width_factor=1.0, height_factor=1.0 -> child fills parent exactly
        let rect = out.expect("child should be positioned");
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 200);
        assert_eq!(rect.height, 100);
    }

    #[test]
    fn test_center_half_size() {
        let mut layout = CenterLayout::with_factors(0.5, 0.5);
        layout.add_widget(7, 0);

        let mut out = None;
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            if id == 7 {
                out = Some(rect);
            }
        });

        // child width = 200 * 0.5 = 100, height = 100 * 0.5 = 50
        // centered: x = (200-100)/2 = 50, y = (100-50)/2 = 25
        let rect = out.expect("child should be positioned");
        assert_eq!(rect.x, 50);
        assert_eq!(rect.y, 25);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 50);
    }

    #[test]
    fn test_center_replace_child() {
        let mut layout = CenterLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0); // replaces child 1

        assert!(!layout.has_child(1));
        assert!(layout.has_child(2));
    }

    #[test]
    fn test_center_remove_and_clear() {
        let mut layout = CenterLayout::new();
        layout.add_widget(10, 0);
        assert!(layout.has_child(10));

        layout.remove_widget(10);
        assert!(!layout.has_child(10));

        layout.add_widget(20, 0);
        layout.clear();
        assert!(!layout.has_child(20));
        assert!(layout.child_ids().is_empty());
    }
}
