// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Normalised interpretation of [`Event::Wheel`](crate::event::Event::Wheel).
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
//! - a component whose absolute value is at least [`WHEEL_DELTA_UNITS`] (120) is
//! in legacy 1/120 ticks and is divided by 120, truncating toward zero;
//! - a smaller component is already a notch count and is used as-is.
//!
//! The sign of the event delta is preserved: positive `y` means the content
//! moves down (the wheel turns toward the user) and positive `x` means the
//! content moves right.
//!
//! Callers that want one line per notch pass `lines_per_notch = 1`. A desktop
//! control that follows the conventional three lines per notch passes
//! [`DEFAULT_LINES_PER_NOTCH`]. The result is expressed in whole lines (or
//! steps for controls that select by row), never in pixels: the caller owns the
//! line-to-pixel conversion.

use crate::core::Point;
use crate::shortcut::Modifiers;

/// Platform wheel tick: one notch is 120 units in the legacy event convention.
pub const WHEEL_DELTA_UNITS: i32 = 120;

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
}
