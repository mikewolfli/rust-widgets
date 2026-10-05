// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Center layout manager — centers a single child within the available area.
//!
//! The child is positioned at the visual center of the parent rect. Optional
//! width_factor and height_factor (0.0–1.0) control how much of the available
//! space the child consumes before centering.
use super::Layout;
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect};

/// The factor a non-finite input falls back to.
///
/// `f32::clamp` propagates `NaN` (every comparison against it is false), so a `NaN`
/// factor would survive the clamp and then cast to a zero-size child — a widget that
/// asked for a fraction of its parent would silently vanish. A non-finite factor has no
/// meaningful fraction to honour, so it is replaced by this documented default: full
/// size, matching [`CenterLayout::new`]. A finite out-of-range factor is still honest
/// and is simply clamped.
const NON_FINITE_FACTOR_DEFAULT: f32 = 1.0;

/// Normalises a factor into `0.0..=1.0`, mapping non-finite input to
/// [`NON_FINITE_FACTOR_DEFAULT`].
fn normalise_factor(factor: f32) -> f32 {
    if factor.is_finite() {
        factor.clamp(0.0, 1.0)
    } else {
        NON_FINITE_FACTOR_DEFAULT
    }
}

/// The visible extent along one axis for a parent length and a factor.
///
/// The product is rounded rather than truncated (principle #50): truncation turned any
/// positive sub-pixel result into `0`, so a `1`-pixel parent at factor `0.5` gave a
/// zero-sized — invisible — child. A positive result is then raised to at least one pixel
/// so it stays addressable, except when the factor is exactly `0`, which is an explicit
/// request for no size and may legitimately stay `0`. The result never exceeds the
/// parent.
fn visible_extent(parent: u32, factor: f32) -> u32 {
    if factor == 0.0 || parent == 0 {
        return 0;
    }
    let scaled = (parent as f32 * factor).round() as u32;
    scaled.min(parent).max(1)
}

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
    /// A finite factor is clamped to the range `0.0..=1.0`; a non-finite factor
    /// (`NaN` or `±Infinity`) has no meaningful fraction, so it falls back to the
    /// documented default of `1.0` rather than surviving the clamp as `NaN`.
    pub fn with_factors(width_factor: f32, height_factor: f32) -> Self {
        Self {
            child: None,
            width_factor: normalise_factor(width_factor),
            height_factor: normalise_factor(height_factor),
        }
    }

    /// Set the width factor (0.0–1.0) for how much of the parent width the child uses.
    ///
    /// A non-finite factor is rejected and the previous value is retained, so a caller
    /// cannot accidentally hide the child by setting `NaN`.
    pub fn set_width_factor(&mut self, factor: f32) {
        if factor.is_finite() {
            self.width_factor = factor.clamp(0.0, 1.0);
        }
    }

    /// Set the height factor (0.0–1.0) for how much of the parent height the child uses.
    ///
    /// A non-finite factor is rejected and the previous value is retained, so a caller
    /// cannot accidentally hide the child by setting `NaN`.
    pub fn set_height_factor(&mut self, factor: f32) {
        if factor.is_finite() {
            self.height_factor = factor.clamp(0.0, 1.0);
        }
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

crate::impl_default_via_new!(CenterLayout);

impl Layout for CenterLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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
            let child_width = visible_extent(rect.width, self.width_factor);
            let child_height = visible_extent(rect.height, self.height_factor);
            let x_offset = (rect.width.saturating_sub(child_width) / 2) as i32;
            let y_offset = (rect.height.saturating_sub(child_height) / 2) as i32;
            let child_rect =
                Rect::new(rect.x + x_offset, rect.y + y_offset, child_width, child_height);
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

    /// Collects the single child's rect from an `update` callback.
    fn child_rect(layout: &CenterLayout, rect: Rect) -> Option<Rect> {
        let mut out = None;
        layout.update(rect, &mut |_, r| out = Some(r));
        out
    }

    /// The defect this pins: a positive sub-pixel size was truncated to zero, so a
    /// visible child became zero-sized. A one-pixel parent at factor `0.5` must still
    /// produce a one-pixel child.
    #[test]
    fn a_positive_sub_pixel_factor_keeps_a_minimum_visible_pixel() {
        let mut layout = CenterLayout::with_factors(0.5, 0.5);
        layout.add_widget(1, 0);
        let rect = child_rect(&layout, Rect::new(0, 0, 1, 1)).expect("child positioned");
        assert_eq!(rect.width, 1, "0.5 of a 1px parent must round up to a visible pixel");
        assert_eq!(rect.height, 1, "0.5 of a 1px parent must round up to a visible pixel");
    }

    /// The rounding and clamping across the factors the contract names, on a 3x3 parent:
    /// `0.0` is an explicit zero, `0.5` rounds, `1.0` fills — and nothing exceeds the parent.
    #[test]
    fn factors_zero_half_and_one_round_and_stay_within_the_parent() {
        for (factor, expected) in [(0.0f32, 0u32), (0.5, 2), (1.0, 3)] {
            let mut layout = CenterLayout::with_factors(factor, factor);
            layout.add_widget(1, 0);
            let rect = child_rect(&layout, Rect::new(0, 0, 3, 3)).expect("child positioned");
            assert_eq!(rect.width, expected, "width for factor {factor}");
            assert_eq!(rect.height, expected, "height for factor {factor}");
            assert!(rect.width <= 3 && rect.height <= 3, "never exceeds the parent: {rect:?}");
        }
    }

    /// An empty parent leaves a positive-factor child at zero: there is no room to be
    /// visible, and the result must not exceed the (empty) parent.
    #[test]
    fn an_empty_parent_yields_a_zero_sized_child() {
        let mut layout = CenterLayout::with_factors(0.5, 0.5);
        layout.add_widget(1, 0);
        let rect = child_rect(&layout, Rect::new(5, 5, 0, 0)).expect("child positioned");
        assert_eq!(rect.width, 0);
        assert_eq!(rect.height, 0);
    }

    /// `NaN` must not survive the constructor's normalisation: it has no fraction, so it
    /// falls back to the documented default of full size instead of casting to zero.
    #[test]
    fn a_non_finite_constructor_factor_falls_back_to_the_documented_default() {
        let layout = CenterLayout::with_factors(f32::NAN, f32::INFINITY);
        assert_eq!(layout.width_factor(), 1.0, "NaN falls back to the documented default");
        assert_eq!(layout.height_factor(), 1.0, "+Infinity falls back to the default too");

        let layout = CenterLayout::with_factors(f32::NEG_INFINITY, f32::NAN);
        assert_eq!(layout.width_factor(), 1.0, "-Infinity falls back to the default");
    }

    /// A non-finite setter argument is rejected and the previous value is retained, so the
    /// caller cannot hide the child by accident. Finite out-of-range values still clamp.
    #[test]
    fn non_finite_setters_are_rejected_and_finite_values_clamp() {
        let mut layout = CenterLayout::with_factors(0.25, 0.75);

        layout.set_width_factor(f32::NAN);
        assert_eq!(layout.width_factor(), 0.25, "NaN must not overwrite the previous value");
        layout.set_height_factor(f32::NEG_INFINITY);
        assert_eq!(layout.height_factor(), 0.75, "-Infinity must not overwrite the previous value");

        layout.set_width_factor(5.0);
        assert_eq!(layout.width_factor(), 1.0, "a finite value above range clamps up");
        layout.set_height_factor(-2.0);
        assert_eq!(layout.height_factor(), 0.0, "a finite value below range clamps down");
        layout.set_width_factor(f32::INFINITY);
        assert_eq!(layout.width_factor(), 1.0, "+Infinity is still rejected");
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
