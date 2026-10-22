// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Ordered-range clamping shared by the date/time edit family.
//!
//! Moved out of `mod.rs` by principle #8: a `mod.rs` declares its children, it does not
//! implement them. The behaviour is unchanged — the three date/time widgets still call
//! [`clamp_ordered_range`] through the re-export.

/// Moves `value` inside `minimum..=maximum`, returning the adjusted value.
///
/// Shared by the date/time edit family, whose three widgets each carried a
/// byte-identical private `clamp_to_range` that differed only in the field it
/// assigned. The inverted-range rule is the interesting part and is stated once
/// here: when `minimum > maximum` the minimum is authoritative, which keeps the
/// operation total (it always terminates with a value a bound setter would
/// accept) instead of wedging the control at a value no write can replace.
pub(crate) fn clamp_ordered_range<T: PartialOrd + Copy>(value: T, minimum: T, maximum: T) -> T {
    if value < minimum {
        minimum
    } else if value > maximum {
        // Only reachable when `minimum <= maximum`; an inverted range already
        // resolved through the branch above or the comparison failing.
        if minimum > maximum {
            minimum
        } else {
            maximum
        }
    } else {
        value
    }
}
