// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Writing direction, for controls whose geometry means different things in a
//! right-to-left script.
//!
//! # Why this type exists
//!
//! A horizontal slider, a progress bar and a range selector all map a value onto
//! a line, and the line they map it onto runs *from where the user starts reading
//! to where they finish*. In an Arabic or Hebrew interface that is right to left,
//! so "minimum" sits at the right edge and dragging toward the minimum means
//! dragging left. Every one of those controls in this crate assumed left-to-right
//! and had no way to say otherwise, which made them not merely mistranslated but
//! **wrong**: the handle moved the opposite way from the value.
//!
//! # What this type deliberately is not
//!
//! It is not a layout engine and not a mirroring transform. A full RTL
//! implementation mirrors padding, borders, icon placement, text run order and
//! scrollbar sides, and doing that globally is a separate project with its own
//! risk of breaking the layouts that work today. This type answers exactly one
//! question — *which end of a line is the beginning* — so a control can honour
//! direction without the rest of the framework changing underneath it.
//!
//! That scoping is the point: a control that reads the direction is correct for
//! the axis it manages, and a control that does not is no worse off than before
//! because the default is [`TextDirection::LeftToRight`].

/// The direction in which a line of content is laid out and read.
///
/// The default is [`TextDirection::LeftToRight`], which is what every control in
/// this crate assumed before the type existed — so adding it changes no existing
/// behaviour and a control only becomes direction-aware by asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDirection {
    /// Left-to-right: the beginning of a line is its left edge.
    #[default]
    LeftToRight,
    /// Right-to-left: the beginning of a line is its right edge.
    RightToLeft,
}

impl TextDirection {
    /// Whether this direction begins at the right edge.
    pub const fn is_right_to_left(self) -> bool {
        matches!(self, Self::RightToLeft)
    }

    /// Maps a fraction along the line where `0.0` is the **beginning** to the
    /// fraction measured from the **left** edge.
    ///
    /// A control computes its layout in reading order — `0.0` at the minimum,
    /// `1.0` at the maximum — and needs a left-edge offset to draw with. This is
    /// the one conversion between those two frames, so a control cannot implement
    /// it slightly differently from its neighbour. It is an involution: applying
    /// it twice returns the input, which is what makes it safe to use for both
    /// directions rather than needing a branch per direction.
    pub const fn begin_fraction_to_left_fraction(self, begin_fraction: f32) -> f32 {
        match self {
            Self::LeftToRight => begin_fraction,
            Self::RightToLeft => 1.0 - begin_fraction,
        }
    }

    /// Maps a fraction measured from the **left** edge back to a
    /// fraction along the line in reading order (the inverse of
    /// [`Self::begin_fraction_to_left_fraction`]).
    pub const fn left_fraction_to_begin_fraction(self, left_fraction: f32) -> f32 {
        // The mapping is its own inverse because the line is symmetric: swapping
        // the ends twice returns the original frame.
        self.begin_fraction_to_left_fraction(left_fraction)
    }

    /// Applies the direction to a signed step along the line.
    ///
    /// A step of `+1` means "one increment in reading order" — toward the
    /// maximum — and a caller passes the increment it wants in those terms. This
    /// returns the step in the frame where positive is rightward, which is what a
    /// key code or a drag delta is expressed in. Without it, an arrow key would
    /// have to be inverted at every call site, which is how the wrong branch gets
    /// taken once and stays.
    pub const fn begin_step_to_left_step(self, begin_step: i32) -> i32 {
        match self {
            Self::LeftToRight => begin_step,
            Self::RightToLeft => -begin_step,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TextDirection;

    /// The default is left-to-right, so adding the type changed nothing.
    ///
    /// This is the assertion that makes the change safe: every control that does
    /// not ask for a direction must keep the behaviour it had, and the only way
    /// that holds is if the default is the assumption those controls were written
    /// under.
    #[test]
    fn the_default_direction_is_left_to_right() {
        assert_eq!(TextDirection::default(), TextDirection::LeftToRight);
        assert!(!TextDirection::default().is_right_to_left());
    }

    /// The two directions put the beginning of the line at opposite ends.
    ///
    /// The numbers are the contract: in LTR the beginning is the left edge and
    /// the end is the right, in RTL the reverse. A control that reads this gets
    /// the value's geometry right; the defect being fixed is precisely that
    /// nothing made this distinction.
    #[test]
    fn the_beginning_of_the_line_is_at_opposite_ends() {
        let ltr = TextDirection::LeftToRight;
        let rtl = TextDirection::RightToLeft;

        // Beginning of the line.
        assert_eq!(ltr.begin_fraction_to_left_fraction(0.0), 0.0);
        assert_eq!(rtl.begin_fraction_to_left_fraction(0.0), 1.0);

        // End of the line.
        assert_eq!(ltr.begin_fraction_to_left_fraction(1.0), 1.0);
        assert_eq!(rtl.begin_fraction_to_left_fraction(1.0), 0.0);

        // The middle is the middle either way, which is why a centred handle
        // looked correct in both directions and the defect survived review.
        assert_eq!(ltr.begin_fraction_to_left_fraction(0.5), 0.5);
        assert_eq!(rtl.begin_fraction_to_left_fraction(0.5), 0.5);
    }

    /// Converting to the left-edge frame and back returns the original fraction.
    ///
    /// A control reads a mouse position in the left-edge frame and needs a value
    /// in reading order, then draws the handle back in the left-edge frame. If the
    /// two conversions were not inverses, dragging the handle would make it drift
    /// away from the pointer — a defect that only shows up under RTL and would be
    /// very hard to attribute later.
    #[test]
    fn the_two_conversions_are_inverses() {
        for direction in [TextDirection::LeftToRight, TextDirection::RightToLeft] {
            for step in 0..=10 {
                let begin_fraction = step as f32 / 10.0;
                let left = direction.begin_fraction_to_left_fraction(begin_fraction);
                let back = direction.left_fraction_to_begin_fraction(left);
                assert!(
                    (back - begin_fraction).abs() < f32::EPSILON,
                    "{direction:?}: {begin_fraction} round-tripped to {back}"
                );
            }
        }
    }

    /// A step toward the maximum moves right in LTR and left in RTL.
    #[test]
    fn a_step_toward_the_maximum_follows_the_direction() {
        assert_eq!(TextDirection::LeftToRight.begin_step_to_left_step(1), 1);
        assert_eq!(TextDirection::RightToLeft.begin_step_to_left_step(1), -1);
        // Decrementing is the mirror, so no call site needs its own negation.
        assert_eq!(TextDirection::LeftToRight.begin_step_to_left_step(-1), -1);
        assert_eq!(TextDirection::RightToLeft.begin_step_to_left_step(-1), 1);
    }
}
