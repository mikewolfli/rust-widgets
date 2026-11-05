// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The "this notch actually moved me" claim a scroll-bearing control reports after handling
//! [`Event::Wheel`](crate::event::Event::Wheel).
//!
//! # Why the pair lives in one function
//!
//! Every control that scrolls on a wheel notch answers the same two-part question: *did the
//! position change, and if it did, does the router stop walking ancestors?* That answer was
//! previously written out at each scroll arm as
//! `if position != before { crate::widget::runtime::claim_wheel(); }`, which had two costs.
//!
//! * The claim lives in [`crate::widget::runtime`], which the stripped profiles compile out, but
//!   `ScrollArea` and `ScrollBar` are built in *every* profile. Their two copies therefore did not
//!   compile under `--features mini` (`E0433: cannot find runtime in widget`), which the
//!   generator-output gate caught.
//! * The comparison and the claim are two separately-maintained facts. Keeping them in a single
//!   function means a control cannot compare the right field and then claim unconditionally.
//!
//! A stripped build has neither a registry nor a router to chain to, so the claim has no meaning
//! there and this module reports "not claimed" without naming `runtime` at all. That keeps the
//! profile knowledge at one layer and lets the controls call the same expression in every build.

/// Claims the wheel event for the caller **iff** `before != after`, i.e. the notch actually moved
/// the control. Returns whether the claim was made.
///
/// A scroll container clamped at its own end passes an unchanged position, gets `false`, and so
/// leaves the claim unset — that is what lets the router offer the notch to the next ancestor
/// ("scroll chaining", C2-D0).
#[cfg(not(alloc_frugal))]
#[inline]
pub fn claim_wheel_if_moved<T: PartialEq>(before: &T, after: &T) -> bool {
    if before == after {
        return false;
    }
    crate::widget::runtime::claim_wheel();
    true
}

/// Stripped-profile form: this build has no [`crate::widget::runtime`] to claim through, so no
/// router routed the notch here in the first place. Returns `false` — the honest answer — rather
/// than pretending the event was consumed.
#[cfg(alloc_frugal)]
#[inline]
pub fn claim_wheel_if_moved<T: PartialEq>(_before: &T, _after: &T) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::claim_wheel_if_moved;

    /// The claim is conditional, not unconditional: a position that did not change must report
    /// `false` so an outer container still gets the notch. True in every profile — including the
    /// stripped ones, where the answer is `false` for both inputs.
    #[test]
    fn an_unmoved_position_does_not_claim() {
        assert!(!claim_wheel_if_moved(&7, &7));
        assert!(!claim_wheel_if_moved(&(3, 4), &(3, 4)));
    }

    /// A position that moved claims the notch **where a router exists**: the stripped profiles have
    /// no `widget::runtime`, so no notch was routed and the honest answer is `false`. Asserting the
    /// profile's answer is what keeps this test meaningful in both builds instead of compiling it
    /// out of one of them.
    #[test]
    fn a_moved_position_claims_where_there_is_a_router() {
        let claims = !cfg!(alloc_frugal);
        assert_eq!(claim_wheel_if_moved(&7, &8), claims);
        assert_eq!(claim_wheel_if_moved(&(3, 4), &(3, 9)), claims);
    }
}
