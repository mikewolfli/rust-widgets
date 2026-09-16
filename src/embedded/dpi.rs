// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Fixed-DPI overrides for hosts that cannot report their panel density.
//!
//! # The fixed-DPI assumption
//!
//! Desktop backends ask the window system for the display's density. An embedded
//! target has no such query — the panel is soldered to the board and its
//! geometry is a build-time fact — so a caller states the density once through
//! [`set_fixed_dpi`] and every scaling helper below reads that pin instead.
//!
//! # Scale factor semantics
//!
//! A scale factor is a ratio, not a DPI: `1.0` means the 96 DPI baseline
//! (96 DPI, the `BASE_DPI` constant) and one logical pixel maps to one physical
//! pixel. It is
//! `fixed_dpi / 96`, so `144` gives `1.5` and `192` gives `2.0`. This matches the
//! crate-wide meaning of a scale factor (see
//! [`crate::widget::Widget::dpi_scale`]).
//!
//! # When scaling is the identity
//!
//! The module-level helpers scale by exactly `1.0` — [`scale`] and friends return
//! their argument unchanged — whenever no DPI is pinned, which includes both the
//! initial state and after [`clear_fixed_dpi`]. A pinned `0` is the same as no
//! pin, because `0` is the "unset" sentinel of the backing atomic. Per-instance
//! [`DpiScaler`] never has an absent DPI: it clamps `0` up to `1`, so its scale
//! factor is `1/96` rather than `1.0` unless the base is set to `0` too (see
//! [`DpiScaler::new`]).

use std::sync::atomic::{AtomicU32, Ordering};
static FIXED_DPI: AtomicU32 = AtomicU32::new(0);
static BASE_DPI: u32 = 96;
/// Set fixed DPI for embedded systems
pub fn set_fixed_dpi(dpi: u32) {
    FIXED_DPI.store(dpi, Ordering::Relaxed);
}
/// Get fixed DPI if set
pub fn get_fixed_dpi() -> Option<u32> {
    let dpi = FIXED_DPI.load(Ordering::Relaxed);
    if dpi > 0 {
        Some(dpi)
    } else {
        None
    }
}
/// Clear fixed DPI setting
pub fn clear_fixed_dpi() {
    FIXED_DPI.store(0, Ordering::Relaxed);
}
/// Check if fixed DPI mode is enabled
pub fn is_fixed_dpi() -> bool {
    FIXED_DPI.load(Ordering::Relaxed) > 0
}
/// Scale factor implied by the pinned DPI.
///
/// Returns `fixed_dpi / 96`, or `1.0` when no DPI is pinned (or the pin is `0`),
/// so a call is always safe and always returns a usable multiplier.
///
/// This is process-wide state: the same factor applies to every widget.
pub fn scale_factor() -> f32 {
    get_fixed_dpi().map(|dpi| dpi as f32 / BASE_DPI as f32).unwrap_or(1.0)
}
/// Scale a value by the current DPI factor
pub fn scale(value: i32) -> i32 {
    (value as f32 * scale_factor()) as i32
}
/// Scale a value by the current DPI factor (unsigned)
pub fn scale_u32(value: u32) -> u32 {
    (value as f32 * scale_factor()) as u32
}
/// Scale a value by the current DPI factor (float)
pub fn scale_f32(value: f32) -> f32 {
    value * scale_factor()
}
/// Convert pixels to points (1/72 inch)
///
/// A point is a physical unit, so this is the only helper here that does **not**
/// consult the pinned DPI — `dpi` is supplied per call. Returns `0.0` for a
/// non-finite `pixels` or a `dpi` of `0`, which is the honest "no meaningful
/// conversion" answer rather than an infinity.
///
/// Note the truncation is *not* applied: a fractional `pixels` gives a
/// fractional result.
pub fn pixels_to_points(pixels: f32, dpi: u32) -> f32 {
    if !pixels.is_finite() || dpi == 0 {
        0.0
    } else {
        pixels * 72.0 / dpi as f32
    }
}
/// Convert points to pixels
///
/// The inverse of [`pixels_to_points`] for a valid `dpi`. Unlike that function
/// this one has no guard, so `dpi == 0` yields a non-finite result and a
/// non-finite `points` propagates; validate the input first.
///
/// Units here are points (1/72 inch) and pixels, not logical units — the pinned
/// DPI is deliberately ignored.
pub fn points_to_pixels(points: f32, dpi: u32) -> f32 {
    points * dpi as f32 / 72.0
}
/// DPI-aware size calculator
///
/// The same `fixed_dpi / base_dpi` arithmetic as [`scale_factor`], but carried as
/// a value instead of read from the process-wide pin. That makes it usable for a
/// second display with a different density, and testable without touching global
/// state. It is immutable once built: `with_base_dpi` consumes and returns it.
#[derive(Debug, Clone, Copy)]
pub struct DpiScaler {
    dpi: u32,
    base_dpi: u32,
}
impl DpiScaler {
    /// Creates a scaler for a display at `dpi`, with the 96 DPI baseline.
    ///
    /// `dpi` is clamped up to `1`, because `0` would make every scaled value
    /// collapse to `0`; a `0` argument therefore produces a `1/96` scale factor
    /// rather than the `1.0` that [`scale_factor`] reports for an unset pin.
    pub fn new(dpi: u32) -> Self {
        Self { dpi: dpi.max(1), base_dpi: BASE_DPI }
    }
    /// Replaces the reference density that a factor of `1.0` maps to.
    ///
    /// Use this when the UI was authored against something other than 96 DPI.
    /// `base` is clamped up to `1` for the same reason as in
    /// [`DpiScaler::new`], so a factor is always finite.
    pub fn with_base_dpi(mut self, base: u32) -> Self {
        self.base_dpi = base.max(1);
        self
    }
    /// Returns `dpi / base_dpi`, i.e. `1.0` when the display density matches the
    /// reference density.
    pub fn scale_factor(&self) -> f32 {
        self.dpi as f32 / self.base_dpi.max(1) as f32
    }
    /// Multiplies `value` by [`DpiScaler::scale_factor`], rounding toward zero.
    ///
    /// `value` is in logical pixels. The truncation is toward zero, not to
    /// nearest, so scaling a small value down can lose a pixel.
    pub fn scale(&self, value: i32) -> i32 {
        (value as f32 * self.scale_factor()) as i32
    }
    /// [`DpiScaler::scale`] for a value that cannot be negative.
    ///
    /// Note the conversion traps at its extremes: `f32 as u32` saturates to `0`
    /// for negative results, so scaling cannot produce a wrapped-around value
    /// here, but a very large `value` at a large factor saturates at `u32::MAX`.
    pub fn scale_u32(&self, value: u32) -> u32 {
        (value as f32 * self.scale_factor()) as u32
    }
    /// Multiplies `value` by [`DpiScaler::scale_factor`] without truncation, so a
    /// fractional result survives.
    pub fn scale_f32(&self, value: f32) -> f32 {
        value * self.scale_factor()
    }
    /// Divides `value` by the scale factor, truncating toward zero.
    ///
    /// Converts physical pixels back to logical ones. Round-trips through
    /// [`DpiScaler::scale`] only when the product was exact, because both steps
    /// truncate; measure the resulting 1-pixel drift against `scale_factor`
    /// rather than expecting equality.
    pub fn unscale(&self, value: i32) -> i32 {
        (value as f32 / self.scale_factor()) as i32
    }
    /// [`DpiScaler::unscale`] for a value that cannot be negative.
    pub fn unscale_u32(&self, value: u32) -> u32 {
        (value as f32 / self.scale_factor()) as u32
    }
    /// Divides `value` by the scale factor without truncation.
    pub fn unscale_f32(&self, value: f32) -> f32 {
        value / self.scale_factor()
    }
}
impl Default for DpiScaler {
    /// A scaler whose density equals the baseline, i.e. a scale factor of `1.0`.
    fn default() -> Self {
        Self::new(BASE_DPI)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fixed_dpi_and_scale_functions() {
        // These tests are combined because they share global FIXED_DPI state
        // and would race when run in parallel.
        set_fixed_dpi(144);
        assert!(is_fixed_dpi());
        assert_eq!(get_fixed_dpi(), Some(144));
        assert_eq!(scale_factor(), 1.5);
        clear_fixed_dpi();
        assert!(!is_fixed_dpi());
        assert_eq!(get_fixed_dpi(), None);

        set_fixed_dpi(192);
        assert_eq!(scale(100), 200);
        assert_eq!(scale_u32(100), 200);
        assert!((scale_f32(100.0) - 200.0).abs() < 0.01);
        clear_fixed_dpi();
    }
    #[test]
    fn test_dpi_scaler() {
        let scaler = DpiScaler::new(144);
        assert!((scaler.scale_factor() - 1.5).abs() < 0.01);
        assert_eq!(scaler.scale(100), 150);
        assert_eq!(scaler.unscale(150), 100);
    }
    #[test]
    fn test_points_conversion() {
        let pixels = points_to_pixels(12.0, 96);
        assert!((pixels - 16.0).abs() < 0.01);
        let points = pixels_to_points(16.0, 96);
        assert!((points - 12.0).abs() < 0.01);
    }

    #[test]
    fn invalid_dpi_inputs_are_safe() {
        assert_eq!(pixels_to_points(96.0, 0), 0.0);
        let scaler = DpiScaler::new(0).with_base_dpi(0);
        assert_eq!(scaler.scale_factor(), 1.0);
        assert_eq!(scaler.unscale(120), 120);
    }
}
