// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Aspect ratio layout manager — constrains a child to a specific aspect ratio.
//!
//! The child is sized to fit within the parent while maintaining the given
//! aspect ratio (width / height). When `respect_parent` is true, the child's
//! size is bounded by the parent dimensions so it never exceeds them.
use super::Layout;
use crate::compat::{Any, Vec};
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
    /// Panics if `aspect_ratio` is not a finite positive number. A `NaN` or an
    /// infinity is not a ratio the child can have: an infinite ratio would make
    /// `parent_w / ratio` zero and collapse the child to nothing even in a
    /// non-empty parent (D08-L-02, Round 55).
    pub fn new(aspect_ratio: f32, respect_parent: bool) -> Self {
        assert!(
            aspect_ratio.is_finite() && aspect_ratio > 0.0,
            "AspectRatioLayout: aspect_ratio must be finite and positive"
        );
        Self { child: None, aspect_ratio, respect_parent }
    }

    /// Set the aspect ratio (width / height).
    ///
    /// # Panics
    ///
    /// Panics if `ratio` is not a finite positive number (see [`AspectRatioLayout::new`]).
    pub fn set_aspect_ratio(&mut self, ratio: f32) {
        assert!(
            ratio.is_finite() && ratio > 0.0,
            "AspectRatioLayout: aspect_ratio must be finite and positive"
        );
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
    ///
    /// A positive extent that rounds to a sub-pixel value keeps at least one pixel, so a
    /// child that has real width or height does not silently disappear (principle #50).
    /// A zero extent (an empty parent, or a degenerate axis) stays zero.
    fn compute_child_rect(&self, parent: Rect) -> Rect {
        let parent_w = parent.width as f32;
        let parent_h = parent.height as f32;
        let ratio = self.aspect_ratio;

        // Round a non-negative extent to pixels, keeping a positive-but-sub-pixel extent
        // visible as one pixel rather than collapsing it to zero.
        fn to_px(extent: f32) -> u32 {
            if extent <= 0.0 {
                return 0;
            }
            let rounded = extent.round();
            if rounded >= 1.0 {
                rounded as u32
            } else {
                // A positive extent below one pixel (including one that rounds down to 0)
                // keeps a single visible pixel.
                1
            }
        }

        let (child_w, child_h) = if self.respect_parent {
            // Fit within parent bounds.
            let by_width = (parent_w, parent_w / ratio);
            let by_height = (parent_h * ratio, parent_h);

            // Pick the one that fits entirely inside the parent.
            if by_width.1 <= parent_h {
                (to_px(by_width.0), to_px(by_width.1))
            } else {
                (to_px(by_height.0), to_px(by_height.1))
            }
        } else {
            // Allow child to exceed parent if necessary to maintain ratio.
            // Use parent width as the base, derive height.
            let w = parent_w;
            let h = w / ratio;
            (to_px(w), to_px(h))
        };

        let x_offset = (parent.width.saturating_sub(child_w) / 2) as i32;
        let y_offset = (parent.height.saturating_sub(child_h) / 2) as i32;

        Rect::new(parent.x + x_offset, parent.y + y_offset, child_w, child_h)
    }
}

impl Layout for AspectRatioLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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
#[allow(clippy::items_after_test_module)]
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

    /// D08-L-02: a positive sub-pixel extent must stay visible, not round to zero.
    #[test]
    fn a_positive_sub_pixel_extent_keeps_one_pixel() {
        // ratio 4.0, non-empty 1x1 parent: by_width is (1, 0.25), which fits, and the
        // 0.25 height must become 1 rather than 0.
        let layout = AspectRatioLayout::new_with_child(42, 4.0, true);
        let mut out = None;
        layout.update(Rect::new(0, 0, 1, 1), &mut |id, rect| {
            if id == 42 {
                out = Some(rect);
            }
        });
        let rect = out.expect("child should be positioned");
        assert_eq!(rect.width, 1);
        assert_eq!(rect.height, 1, "a positive extent must not collapse to zero");
    }

    /// An empty parent still yields a degenerate (zero) child rather than a 1x1 one.
    #[test]
    fn an_empty_parent_still_yields_a_zero_sized_child() {
        let layout = AspectRatioLayout::new_with_child(42, 2.0, true);
        let mut out = None;
        layout.update(Rect::new(0, 0, 0, 0), &mut |id, rect| {
            if id == 42 {
                out = Some(rect);
            }
        });
        let rect = out.expect("child should be positioned");
        assert_eq!((rect.width, rect.height), (0, 0));
    }

    /// D08-L-02: a non-finite ratio is rejected at every entry point, so it can never
    /// produce a zero-height child in a non-empty parent.
    #[test]
    fn non_finite_ratios_are_rejected() {
        assert!(std::panic::catch_unwind(|| AspectRatioLayout::new(f32::INFINITY, true)).is_err());
        assert!(std::panic::catch_unwind(|| AspectRatioLayout::new(f32::NAN, true)).is_err());
        assert!(std::panic::catch_unwind(|| AspectRatioLayout::new(0.0, true)).is_err());
        assert!(std::panic::catch_unwind(|| AspectRatioLayout::new(-1.0, true)).is_err());
        let mut layout = AspectRatioLayout::new(2.0, true);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            layout.set_aspect_ratio(f32::INFINITY)
        }))
        .is_err());
    }
}

/// Helper for constructing a layout pre-populated with a child.
#[cfg(test)]
impl AspectRatioLayout {
    /// Create a new aspect ratio layout with an initial child (test-only helper).
    ///
    /// # Panics
    ///
    /// Panics if `aspect_ratio` is not a finite positive number.
    pub fn new_with_child(child_id: ObjectId, aspect_ratio: f32, respect_parent: bool) -> Self {
        assert!(
            aspect_ratio.is_finite() && aspect_ratio > 0.0,
            "AspectRatioLayout: aspect_ratio must be finite and positive"
        );
        Self { child: Some(child_id), aspect_ratio, respect_parent }
    }
}
