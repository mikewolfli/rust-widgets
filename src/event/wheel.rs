// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Normalised interpretation of [`Event::Wheel`].
//!
//! # One unit rule
//!
//! `Event::Wheel` documents `delta` in wheel **notches**. Some long-lived input
//! paths in this crate predate that contract and deliver the platform's raw
//! 120-unit tick (`WHEEL_DELTA`: one notch is 120 units), which is why several
//! widgets each contained their own `/ 120`. Rather than let every control keep
//! its own rule, [`WheelScroll::from_event`] applies one normalisation for all
//! callers:
//!
//! - a component whose absolute value is at least `WHEEL_DELTA_UNITS` (120) is
//! in legacy 1/120 ticks and is divided by 120, truncating toward zero;
//! - a smaller component is already a notch count and is used as-is.
//!
//! The sign of the event delta is preserved: positive `y` means the content
//! moves down (the wheel turns toward the user) and positive `x` means the
//! content moves right.
//!
//! Callers that want one line per notch pass `lines_per_notch = 1`. A desktop
//! control that follows the conventional three lines per notch passes
//! `DEFAULT_LINES_PER_NOTCH`. The result is expressed in whole lines (or
//! steps for controls that select by row), never in pixels: the caller owns the
//! line-to-pixel conversion.

use crate::core::Point;
use crate::shortcut::Modifiers;

/// Platform wheel tick: one notch is 120 units in the legacy event convention.
pub const WHEEL_DELTA_UNITS: i32 = 120;

/// Logical points one conventional wheel notch covers on a precise-scrolling device.
///
/// A trackpad or a high-resolution wheel reports its motion in **points** rather than in
/// notches. Dividing a few points by this value and carrying the remainder is what keeps a
/// light two-finger swipe from being reported as dozens of notches; it is also the point at
/// which the remainder becomes one notch. The value matches the conventional three lines per
/// notch at a ~13 px line, which is why it reads as "one notch" to the controls that consume
/// [`WheelScroll`].
pub const POINTS_PER_WHEEL_NOTCH: f64 = 40.0;

/// Lines a desktop wheel notch conventionally scrolls (参考工具包 `abstract slider`).
pub const DEFAULT_LINES_PER_NOTCH: u32 = 3;

/// A wheel event converted to signed line/step counts.
///
/// `lines_y` and `columns_x` follow the sign of [`Event::Wheel`](crate::event::Event::Wheel):
/// positive means the content moves down or right. The struct does **not**
/// clamp, page, or otherwise reinterpret those counts; each control decides
/// what one line means for its own content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelScroll {
    /// Vertical movement in lines; positive means the content moves down.
    pub lines_y: i32,
    /// Horizontal movement in lines; positive means the content moves right.
    pub columns_x: i32,
    /// Keyboard modifiers carried by the wheel event.
    pub modifiers: Modifiers,
}

impl WheelScroll {
    /// Converts an event delta into line counts.
    ///
    /// `lines_per_notch` is the number of lines one notch scrolls. See the
    /// [module documentation](self) for the 120-unit normalization rule.
    pub fn from_event(delta: Point, modifiers: u32, lines_per_notch: u32) -> Self {
        Self {
            lines_y: lines_from_delta(delta.y, lines_per_notch),
            columns_x: lines_from_delta(delta.x, lines_per_notch),
            modifiers: Modifiers::from_event_bits(modifiers),
        }
    }

    /// Whether the event has a vertical component.
    ///
    /// A horizontal-only wheel event returns `false`, as does an event whose
    /// delta is smaller than one line.
    pub fn is_vertical(&self) -> bool {
        self.lines_y != 0
    }
}

/// Normalises one delta component and scales it to lines.
fn lines_from_delta(value: i32, lines_per_notch: u32) -> i32 {
    let value = value as i64;
    let notches = if value.unsigned_abs() >= WHEEL_DELTA_UNITS as u64 {
        value / WHEEL_DELTA_UNITS as i64
    } else {
        value
    };
    let lines = notches * lines_per_notch as i64;
    lines.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// Converts a precise (pixel/point) scroll stream into whole wheel notches.
///
/// # Why a sub-notch accumulator is necessary
///
/// AppKit's `hasPreciseScrollingDeltas` path reports a trackpad's motion in points: a gentle
/// two-finger swipe may deliver `0.8`, `1.7`, `2.4`, … per event. Rounding each sample on its
/// own turns those into `1` notch apiece, inflating the distance by an order of magnitude, and
/// discarding them loses the slow scroll entirely. Keeping the remainder means several small
/// samples add up to one notch and the direction of travel survives, while the total distance
/// stays proportional to what the user's fingers actually did.
///
/// Both axes are carried independently, so a diagonal gesture does not borrow from, or lose,
/// the other axis' remainder. A non-finite or non-positive `points_per_notch` is refused with
/// a zero delta rather than producing an infinite number of notches.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WheelNotchAccumulator {
    residual_x: f64,
    residual_y: f64,
}

impl WheelNotchAccumulator {
    /// A fresh accumulator with no carried remainder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one precise sample, using [`POINTS_PER_WHEEL_NOTCH`] as the conversion.
    pub fn add_points(&mut self, dx: f64, dy: f64) -> Point {
        self.add_points_per_notch(dx, dy, POINTS_PER_WHEEL_NOTCH)
    }

    /// Adds one precise sample with an explicit points-per-notch conversion.
    pub fn add_points_per_notch(&mut self, dx: f64, dy: f64, points_per_notch: f64) -> Point {
        if !points_per_notch.is_finite() || points_per_notch <= 0.0 {
            return Point::new(0, 0);
        }
        if dx.is_finite() {
            self.residual_x += dx;
        }
        if dy.is_finite() {
            self.residual_y += dy;
        }
        let notches_x = (self.residual_x / points_per_notch).trunc();
        let notches_y = (self.residual_y / points_per_notch).trunc();
        self.residual_x -= notches_x * points_per_notch;
        self.residual_y -= notches_y * points_per_notch;
        Point::new(saturating_i32(notches_x), saturating_i32(notches_y))
    }

    /// Drops any carried remainder, for a stream that ended or changed direction source.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Rounds a notch count to `i32`, saturating rather than wrapping on an absurd sample.
fn saturating_i32(value: f64) -> i32 {
    if value >= i32::MAX as f64 {
        i32::MAX
    } else if value <= i32::MIN as f64 {
        i32::MIN
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notch_moves_one_line_with_its_original_sign() {
        let down = WheelScroll::from_event(Point::new(0, 1), 0, 1);
        assert_eq!(down.lines_y, 1);
        assert_eq!(down.columns_x, 0);

        let up = WheelScroll::from_event(Point::new(0, -1), 0, 1);
        assert_eq!(up.lines_y, -1);
    }

    #[test]
    fn legacy_120_unit_ticks_become_one_notch() {
        let down = WheelScroll::from_event(Point::new(0, WHEEL_DELTA_UNITS), 0, 1);
        assert_eq!(down.lines_y, 1, "120 legacy units are one notch, one line");

        let up = WheelScroll::from_event(Point::new(0, -WHEEL_DELTA_UNITS), 0, 1);
        assert_eq!(up.lines_y, -1);

        let two = WheelScroll::from_event(Point::new(0, 2 * WHEEL_DELTA_UNITS), 0, 1);
        assert_eq!(two.lines_y, 2);
    }

    #[test]
    fn deltas_below_120_are_already_notches() {
        let event = WheelScroll::from_event(Point::new(3, -2), 0, 1);
        assert_eq!(event.columns_x, 3);
        assert_eq!(event.lines_y, -2);
    }

    #[test]
    fn lines_per_notch_scales_both_axes() {
        let event = WheelScroll::from_event(Point::new(-1, 2), 0, DEFAULT_LINES_PER_NOTCH);
        assert_eq!(event.columns_x, -3);
        assert_eq!(event.lines_y, 6);
    }

    #[test]
    fn is_vertical_depends_only_on_the_vertical_component() {
        assert!(WheelScroll::from_event(Point::new(0, 1), 0, 1).is_vertical());
        assert!(!WheelScroll::from_event(Point::new(0, -5), 0, 0).is_vertical());
        assert!(!WheelScroll::from_event(Point::new(4, 0), 0, 1).is_vertical());
        assert!(!WheelScroll::from_event(Point::new(0, 0), 0, 1).is_vertical());
    }

    #[test]
    fn modifiers_are_decoded_from_event_bits() {
        // Event bit 0 = Shift, bit 1 = Control, bit 2 = Alt.
        let event = WheelScroll::from_event(Point::new(0, 1), 0b0111, 1);
        assert!(event.modifiers.contains(Modifiers::SHIFT));
        assert!(event.modifiers.contains(Modifiers::CTRL));
        assert!(event.modifiers.contains(Modifiers::ALT));
        assert!(!event.modifiers.contains(Modifiers::PRIMARY));
    }

    #[test]
    fn the_command_bit_maps_to_primary() {
        // Event bit 3 = Command/Meta, which `from_event_bits` reports as PRIMARY.
        let event = WheelScroll::from_event(Point::new(0, 1), 0b1000, 1);
        assert!(event.modifiers.contains(Modifiers::PRIMARY));
    }

    /// Several small precise samples must add up to one notch instead of each becoming a notch.
    #[test]
    fn precise_samples_accumulate_into_whole_notches() {
        let mut accumulator = WheelNotchAccumulator::new();
        assert_eq!(accumulator.add_points(0.0, 10.0), Point::new(0, 0));
        assert_eq!(accumulator.add_points(0.0, 15.0), Point::new(0, 0));
        // 10 + 15 + 15 = 40 points: exactly one notch, with nothing left over.
        assert_eq!(accumulator.add_points(0.0, 15.0), Point::new(0, 1));
        assert_eq!(accumulator.add_points(0.0, 10.0), Point::new(0, 0));
    }

    /// The remainder survives across samples and does not leak between axes.
    #[test]
    fn precise_remainders_are_per_axis() {
        let mut accumulator = WheelNotchAccumulator::new();
        // 30 points on each axis: neither is a notch yet.
        assert_eq!(accumulator.add_points(30.0, 30.0), Point::new(0, 0));
        // Ten more on x crosses the bar; y only reaches 40 and crosses too.
        assert_eq!(accumulator.add_points(10.0, 10.0), Point::new(1, 1));
        // The remainders are both zero now.
        assert_eq!(accumulator.add_points(39.0, -39.0), Point::new(0, 0));
        assert_eq!(accumulator.add_points(1.0, -1.0), Point::new(1, -1));
    }

    /// A degenerate conversion must not turn a sample into an unbounded scroll.
    #[test]
    fn a_non_positive_points_per_notch_yields_no_movement() {
        let mut accumulator = WheelNotchAccumulator::new();
        assert_eq!(accumulator.add_points_per_notch(100.0, 100.0, 0.0), Point::new(0, 0));
        assert_eq!(accumulator.add_points_per_notch(100.0, 100.0, -1.0), Point::new(0, 0));
        assert_eq!(accumulator.add_points_per_notch(100.0, 100.0, f64::NAN), Point::new(0, 0));
    }

    /// Resetting drops the carried remainder, as a device change requires.
    #[test]
    fn reset_drops_the_remainder() {
        let mut accumulator = WheelNotchAccumulator::new();
        assert_eq!(accumulator.add_points(0.0, 30.0), Point::new(0, 0));
        accumulator.reset();
        assert_eq!(accumulator.add_points(0.0, 9.0), Point::new(0, 0));
        assert_eq!(accumulator.add_points(0.0, 31.0), Point::new(0, 1));
    }
}
