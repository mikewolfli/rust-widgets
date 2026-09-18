// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Advanced widgets.
pub mod calendar;
pub mod date_edit;
pub mod date_time_edit;
pub mod dial;
pub mod key_sequence_edit;
pub mod pie_menu;
pub mod ribbon_bar;
pub mod tab_bar;
pub mod time_edit;

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

// Re-export advanced widget types
pub use calendar::Calendar;
pub use date_edit::DateEdit;
pub use date_time_edit::DateTimeEdit;
pub use dial::Dial;
pub use key_sequence_edit::KeySequenceEdit;
pub use pie_menu::{PieMenu, PieMenuItem};
pub use ribbon_bar::{RibbonBar, RibbonGroup, RibbonItem};
pub use tab_bar::{TabBar, TabBarTab};
pub use time_edit::TimeEdit;
