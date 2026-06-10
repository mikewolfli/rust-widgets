//! Aspect ratio layout manager — constrains a child to a specific aspect ratio.
//!
//! The child is sized to fit within the parent while maintaining the given
//! aspect ratio (width / height). When `respect_parent` is true, the child's
//! size is bounded by the parent dimensions so it never exceeds them.
use super::Layout;
use crate::core::{ObjectId, Rect};

/// A layout that constrains a single child to a specific aspect ratio.
///
/// The aspect ratio is defined as `width / height`. The child is sized to
/// fit within the parent while maintaining this ratio. When `respect_parent`
/// is true, the child never exceeds the parent's dimensions.
#[derive(Debug)]
pub struct AspectRatioLayout {
    child: Option<ObjectId>,
    /// Desired aspect ratio (width / height).
    aspect_ratio: f32,
    /// If true, the child is bounded by the parent rect on both axes.
    respect_parent: bool,
}

impl AspectRatioLayout {
    /// Create a new aspect ratio layout.
    ///
    /// # Panics
    ///
    /// Panics if `aspect_ratio` is not positive.
    pub fn new(aspect_ratio: f32, respect_parent: bool) -> Self {
        assert!(aspect_ratio > 0.0, "AspectRatioLayout: aspect_ratio must be positive");
        Self { child: None, aspect_ratio, respect_parent }
    }

    /// Set the aspect ratio (width / height).
    ///
    /// # Panics
    ///
    /// Panics if `ratio` is not positive.
    pub fn set_aspect_ratio(&mut self, ratio: f32) {
        assert!(ratio > 0.0, "AspectRatioLayout: aspect_ratio must be positive");
        self.aspect_ratio = ratio;
    }

    /// Returns the current aspect ratio.
    pub fn aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    /// Set whether the child is bounded by parent dimensions.
    pub fn set_respect_parent(&mut self, respect: bool) {
        self.respect_parent = respect;
    }

    /// Returns whether the child is bounded by parent dimensions.
    pub fn respect_parent(&self) -> bool {
        self.respect_parent
    }

    /// Compute the child rect that fits within the given rect while maintaining the aspect ratio.
    fn compute_child_rect(&self, parent: Rect) -> Rect {
        let parent_w = parent.width as f32;
        let parent_h = parent.height as f32;
        let ratio = self.aspect_ratio;

        let (child_w, child_h) = if self.respect_parent {
            // Fit within parent bounds.
            let by_width = (parent_w, parent_w / ratio);
            let by_height = (parent_h * ratio, parent_h);

            // Pick the one that fits entirely inside the parent.
            if by_width.1 <= parent_h {
                (by_width.0 as u32, by_width.1 as u32)
            } else {
                (by_height.0 as u32, by_height.1 as u32)
            }
        } else {
            // Allow child to exceed parent if necessary to maintain ratio.
            // Use parent width as the base, derive height.
            let w = parent_w;
            let h = w / ratio;
            (w as u32, h as u32)
        };

        let x_offset = (parent.width.saturating_sub(child_w) / 2) as i32;
        let y_offset = (parent.height.saturating_sub(child_h) / 2) as i32;

        Rect::new(parent.x + x_offset, parent.y + y_offset, child_w, child_h)
    }
}

impl Layout for AspectRatioLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
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
            let child_rect = self.compute_child_rect(rect);
            widgets(child_id, child_rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aspect_ratio_fit_width() {
        // aspect_ratio = 2.0 (width/height), parent is 200x100.
        // with respect_parent=true.
        // By width: 200x100 -> 200/2=100 height, fits (100 <= 100).
        let layout = AspectRatioLayout::new_with_child(42, 2.0, true);

        let mut out = None;
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            if id == 42 {
                out = Some(rect);
            }
        });

        let rect = out.expect("child should be positioned");
        assert_eq!(rect.width, 200);
        assert_eq!(rect.height, 100);
    }

    #[test]
    fn test_aspect_ratio_fit_height() {
        // aspect_ratio = 1.0 (square), parent is 200x50 (wide).
        // with respect_parent=true.
        // By width: 200x200 -> 200 height exceeds 50, so use by_height: 50x50.
        let layout = AspectRatioLayout::new_with_child(42, 1.0, true);

        let mut out = None;
        layout.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            if id == 42 {
                out = Some(rect);
            }
        });

        // by height: height=50, width=50*1.0=50 -> 50x50
        let rect = out.expect("child should be positioned");
        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 50);
        // centering: x = (200-50)/2 = 75, y = 0
        assert_eq!(rect.x, 75);
        assert_eq!(rect.y, 0);
    }

    #[test]
    fn test_aspect_ratio_does_not_respect_parent() {
        // aspect_ratio = 1.0, parent is 100x100.
        // With respect_parent=false, child matches parent exactly.
        let layout = AspectRatioLayout::new_with_child(1, 1.0, false);

        let mut out = None;
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            if id == 1 {
                out = Some(rect);
            }
        });

        let rect = out.expect("child should be positioned");
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 100);
    }

    #[test]
    fn test_aspect_ratio_remove_and_clear() {
        let mut layout = AspectRatioLayout::new(2.0, true);
        layout.add_widget(10, 0);
        assert!(layout.has_child(10));

        layout.remove_widget(10);
        assert!(!layout.has_child(10));

        layout.add_widget(20, 0);
        layout.clear();
        assert!(layout.child_ids().is_empty());
    }
}

// Helper used only in tests — but visible for external callers too
impl AspectRatioLayout {
    /// Create a new aspect ratio layout with an initial child.
    ///
    /// # Panics
    ///
    /// Panics if `aspect_ratio` is not positive.
    pub fn new_with_child(child_id: ObjectId, aspect_ratio: f32, respect_parent: bool) -> Self {
        assert!(aspect_ratio > 0.0, "AspectRatioLayout: aspect_ratio must be positive");
        Self { child: Some(child_id), aspect_ratio, respect_parent }
    }
}
