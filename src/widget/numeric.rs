// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Numeric helpers for widget value clamping.
//!
//! # Why this module exists
//!
//! Widgets that carry a value next to a `min`/`max` pair — sliders, sliders' arcs,
//! meters, scroll bars, steppers, spin boxes, dials, LCD numbers, progress bars,
//! range sliders — wrote `value.clamp(self.min, self.max)` at the point of use.
//! That is safe exactly as long as `min <= max` and neither is `NaN`, and
//! `f32::clamp` / `f64::clamp` **panic** rather than clamp when that does not hold:
//!
//! ```text
//! thread 'main' panicked at core/src/num/f32.rs:1566:9:
//! min > max, or either was NaN. min = 3.5, max = 1.0
//! ```
//!
//! The bounds are not constants — they are public setters (`set_min`,
//! `set_maximum`, …) and writable capability properties, so an ordinary sequence of
//! two documented calls reaches the panic:
//!
//! ```text
//! cupertino_slider: create (min=0.0, max=1.0)
//! set_property("min", 3.5)   ->  abort
//! ```
//!
//! That sequence is reachable from Rust, from declarative JSON, and from every
//! language binding through the C ABI. A GUI library must not abort its process
//! because a caller set two numbers in the wrong order.
//!
//! # The rule this encodes
//!
//! Clamping is defined against the **ordered** pair, not against the pair as
//! declared: `min` and `max` are treated as two bounds, and the smaller of them is
//! the floor. A caller that swaps them gets a value inside the range they named,
//! instead of a crash. `NaN` bounds are ignored rather than propagated, because a
//! `NaN` bound is not a range at all; when both bounds are `NaN` the value is
//! returned unchanged, which is the only non-arbitrary answer.
//!
//! This is deliberately *not* a silent repair of the widget's own state — the
//! setters remain free to normalise `min`/`max` themselves (several already do,
//! see `slider.rs`'s "adjusts maximum when crossed" tests). This module only
//! guarantees that the act of clamping cannot panic.

/// Clamps `value` into the range spanned by `a` and `b`, in either order.
///
/// Either bound may be `NaN`; a `NaN` bound is treated as absent. If both are
/// `NaN` there is no range to clamp to, so `value` is returned unchanged.
///
/// Never panics, which is the entire point: it replaces `value.clamp(min, max)`
/// at call sites where `min` and `max` are mutable fields rather than constants.
///
/// ```
/// # use rust_widgets::widget::numeric::ordered_clamp;
/// // Declared order.
/// assert_eq!(ordered_clamp(5.0, 0.0, 10.0), 5.0);
/// // Crossed bounds clamp instead of panicking.
/// assert_eq!(ordered_clamp(5.0, 10.0, 0.0), 5.0);
/// assert_eq!(ordered_clamp(-3.0, 10.0, 0.0), 0.0);
/// // A NaN bound is ignored.
/// assert_eq!(ordered_clamp(5.0, f32::NAN, 10.0), 5.0);
/// // No usable bounds at all: the value is returned as-is.
/// assert_eq!(ordered_clamp(5.0, f32::NAN, f32::NAN), 5.0);
/// ```
pub fn ordered_clamp(value: f32, a: f32, b: f32) -> f32 {
    match (a.is_nan(), b.is_nan()) {
        (false, false) => {
            let low = a.min(b);
            let high = a.max(b);
            // `value` may itself be NaN; clamp would panic-free return NaN, and so
            // does this, because `min`/`max` propagate the non-NaN operand.
            value.min(high).max(low)
        }
        // One bound is usable, so treat the range as bounded on that side only.
        (false, true) => value.max(a),
        (true, false) => value.min(b),
        // Neither bound is a number: there is no range, so nothing to clamp.
        (true, true) => value,
    }
}

/// `f64` counterpart of [`ordered_clamp`], with identical semantics.
///
/// ```
/// # use rust_widgets::widget::numeric::ordered_clamp_f64;
/// assert_eq!(ordered_clamp_f64(5.0, 0.0, 10.0), 5.0);
/// assert_eq!(ordered_clamp_f64(5.0, 10.0, 0.0), 5.0);
/// assert_eq!(ordered_clamp_f64(5.0, f64::NAN, 10.0), 5.0);
/// assert_eq!(ordered_clamp_f64(5.0, f64::NAN, f64::NAN), 5.0);
/// ```
pub fn ordered_clamp_f64(value: f64, a: f64, b: f64) -> f64 {
    match (a.is_nan(), b.is_nan()) {
        (false, false) => {
            let low = a.min(b);
            let high = a.max(b);
            value.min(high).max(low)
        }
        (false, true) => value.max(a),
        (true, false) => value.min(b),
        (true, true) => value,
    }
}

/// Clamps an integer into the range spanned by `a` and `b`, in either order.
///
/// Integer `clamp` panics on `min > max` for the same reason the float one does, and
/// integer widgets (`scroll_bar`, `progress_bar`, `stepper`, `spin_box`, `dial`)
/// carry the same two mutable bounds. Reordering is the whole fix; there is no `NaN`
/// case to consider.
///
/// ```
/// # use rust_widgets::widget::numeric::ordered_clamp_i32;
/// assert_eq!(ordered_clamp_i32(5, 0, 10), 5);
/// assert_eq!(ordered_clamp_i32(5, 10, 0), 5);
/// assert_eq!(ordered_clamp_i32(-1, 10, 0), 0);
/// ```
pub fn ordered_clamp_i32(value: i32, a: i32, b: i32) -> i32 {
    let low = a.min(b);
    let high = a.max(b);
    value.min(high).max(low)
}

/// `i64` counterpart of [`ordered_clamp_i32`].
///
/// Needed because `InputDialog` carries its bounds as `i64` (so that
/// `i64::MIN`/`i64::MAX` remain usable as open bounds) and its `get_int`
/// constructor takes them as parameters — a caller-supplied pair that
/// `i64::clamp` panics on when crossed.
///
/// ```
/// # use rust_widgets::widget::numeric::ordered_clamp_i64;
/// assert_eq!(ordered_clamp_i64(5, 0, 10), 5);
/// assert_eq!(ordered_clamp_i64(5, 10, 0), 5);
/// assert_eq!(ordered_clamp_i64(-1, 10, 0), 0);
/// ```
pub fn ordered_clamp_i64(value: i64, a: i64, b: i64) -> i64 {
    let low = a.min(b);
    let high = a.max(b);
    value.min(high).max(low)
}

/// `u32` counterpart of [`ordered_clamp_i32`].
///
/// ```
/// # use rust_widgets::widget::numeric::ordered_clamp_u32;
/// assert_eq!(ordered_clamp_u32(5, 0, 10), 5);
/// assert_eq!(ordered_clamp_u32(5, 10, 0), 5);
/// assert_eq!(ordered_clamp_u32(50, 10, 0), 10);
/// ```
pub fn ordered_clamp_u32(value: u32, a: u32, b: u32) -> u32 {
    let low = a.min(b);
    let high = a.max(b);
    value.min(high).max(low)
}

/// `usize` counterpart of [`ordered_clamp_i32`], for index-valued bounds.
///
/// ```
/// # use rust_widgets::widget::numeric::ordered_clamp_usize;
/// assert_eq!(ordered_clamp_usize(5, 0, 10), 5);
/// assert_eq!(ordered_clamp_usize(5, 10, 0), 5);
/// ```
pub fn ordered_clamp_usize(value: usize, a: usize, b: usize) -> usize {
    let low = a.min(b);
    let high = a.max(b);
    value.min(high).max(low)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact regression: the sequence that used to abort the process.
    ///
    /// `cupertino_slider` is created with `min = 0.0, max = 1.0`, so writing
    /// `min = 3.5` crosses the bounds. `f32::clamp` panicked here; this must not.
    #[test]
    fn crossed_float_bounds_do_not_panic() {
        let clamped = ordered_clamp(0.0, 3.5, 1.0);
        assert_eq!(clamped, 1.0, "the value belongs inside the ordered range [1.0, 3.5]");
    }

    /// A `NaN` bound must be ignored, not propagated into the result.
    #[test]
    fn a_nan_bound_is_ignored() {
        assert_eq!(ordered_clamp(5.0, f32::NAN, 10.0), 5.0);
        assert_eq!(ordered_clamp(5.0, 0.0, f32::NAN), 5.0);
        assert_eq!(ordered_clamp(15.0, 0.0, f32::NAN), 15.0);
    }

    /// With no usable bound the value passes through, which is the only answer that
    /// does not invent a range.
    #[test]
    fn two_nan_bounds_return_the_value_unchanged() {
        assert_eq!(ordered_clamp(5.0, f32::NAN, f32::NAN), 5.0);
        assert_eq!(ordered_clamp_f64(5.0, f64::NAN, f64::NAN), 5.0);
    }

    /// Declared order must behave exactly like `clamp`, so this is a drop-in
    /// replacement and not a behaviour change for correct callers.
    #[test]
    fn declared_order_matches_std_clamp() {
        for value in [-5.0_f32, 0.0, 5.0, 10.0, 15.0] {
            assert_eq!(
                ordered_clamp(value, 0.0, 10.0),
                value.clamp(0.0, 10.0),
                "ordered_clamp must not diverge from clamp when the bounds are ordered"
            );
        }
        for value in [-5, 0, 5, 10, 15] {
            assert_eq!(ordered_clamp_i32(value, 0, 10), value.clamp(0, 10));
        }
    }

    /// The integer variants must survive swapped bounds too.
    #[test]
    fn crossed_integer_bounds_are_reordered() {
        assert_eq!(ordered_clamp_i32(5, 10, 0), 5);
        assert_eq!(ordered_clamp_i32(20, 10, 0), 10);
        assert_eq!(ordered_clamp_i64(5, 10, 0), 5);
        assert_eq!(ordered_clamp_i64(20, 10, 0), 10);
        assert_eq!(ordered_clamp_i64(-20, 10, 0), 0);
        assert_eq!(ordered_clamp_u32(20, 10, 0), 10);
        assert_eq!(ordered_clamp_usize(20, 10, 0), 10);
    }

    /// Infinity as a bound is a legitimate range end, not an error.
    #[test]
    fn infinite_bounds_are_respected() {
        assert_eq!(ordered_clamp(1e30, f32::NEG_INFINITY, f32::INFINITY), 1e30);
        assert_eq!(ordered_clamp(f32::INFINITY, 0.0, 10.0), 10.0);
    }
}
