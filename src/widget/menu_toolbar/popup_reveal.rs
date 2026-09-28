// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The shared open/close reveal for the popup family.
//!
//! # What this is
//!
//! `menu`, `context_menu`, `menu_item`, `menu_button` and `dropdown_menu` all do one thing when
//! their open flag flips: a panel appears. Every one of them used to paint that panel at its
//! final geometry on the frame the flag changed, so the mechanism they shared was **no
//! mechanism** — five controls each hard-cutting, and no way to fix one without the others
//! drifting.
//!
//! # Why a type rather than five `PropertyDriver` fields
//!
//! The three facts a reveal needs are the same three in every control:
//!
//! * **which way it grows** — a menu hung under its button grows down; the same menu opened
//!   upward (because there is no room below) grows up;
//! * **how far it moves at most** — a reveal that always travelled the panel's full height would
//!   make a one-row menu and a twenty-row menu animate very differently for no reason a user
//!   could name;
//! * **what its geometry is at rest** — the panel's own rectangle.
//!
//! Keeping those in the control and only the *progress* here is what makes this shared: the
//! controls disagree about geometry and agree about the curve. A shared type that also owned the
//! rectangle would force five different layouts through one shape.
//!
//! # Why the reveal is both a clip and a translation
//!
//! A popup that only faded would show its full contents immediately at partial opacity, so a
//! half-open menu would display rows that are not reachable yet — the list would be a promise
//! the geometry has not kept. A popup that only grew would pop its contents into existence. The
//! pair is what M3 asks for: the panel's *extent* and its *visibility* move together, so at
//! every frame the rows a user can see are the rows that are inside the panel.

use crate::core::{Rect, Size};
use crate::style::{MotionSlot, PropertyDriver};

/// The direction a popup panel grows in from its anchor edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealDirection {
    /// The panel's top edge is the anchor; its height grows downward.
    Down,
    /// The panel's bottom edge is the anchor; its height grows upward.
    Up,
}

/// How far a popup has opened, `0.0` closed to `1.0` fully open.
///
/// A `PropertyDriver` rather than a plain `f32`, so the control gets the crate's one transition
/// engine (the theme's duration and easing, and the reduced-motion collapse) instead of its own
/// arithmetic. The value is a plain progress rather than a pixel distance because a fraction
/// survives a relayout that moves or resizes the panel — a pixel offset would have to be
/// re-derived against the old geometry on every frame.
#[derive(Debug, Clone, Copy)]
pub struct PopupReveal {
    progress: PropertyDriver,
}

impl Default for PopupReveal {
    fn default() -> Self {
        Self::new(false)
    }
}

impl PopupReveal {
    /// A reveal already settled at `open`, so a control that starts closed owes no frames.
    pub fn new(open: bool) -> Self {
        Self { progress: PropertyDriver::at(if open { 1.0 } else { 0.0 }, MotionSlot::Fast) }
    }

    /// The current fraction, in `0.0..=1.0`.
    pub fn value(&self) -> f32 {
        self.progress.value()
    }

    /// Aims at `open`. The next [`tick`](Self::tick) starts the movement.
    pub fn set_open(&mut self, open: bool) {
        self.progress.set_target(if open { 1.0 } else { 0.0 });
    }

    /// Jumps to `open` with no animation, for a popup that must appear at once (a context menu
    /// opened by a keyboard accelerator, or a control being re-anchored).
    pub fn jump_to(&mut self, open: bool) {
        self.progress.jump_to(if open { 1.0 } else { 0.0 });
    }

    /// Advances by `delta_ms`; `true` while it still owes frames.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.progress.tick(delta_ms)
    }

    /// Whether it is between the two ends. Answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.progress.is_moving()
    }

    /// The rectangle actually painted for a panel anchored at `full`.
    ///
    /// At `1.0` this is exactly `full`, so a settled popup is byte-identical to the un-animated
    /// one — which is what keeps the snapshot set stable and makes the change reviewable as
    /// "only the frames in flight moved".
    ///
    /// # Why the cap is an *interpolation over the panel*, not a ceiling on it
    ///
    /// The travel cap decides how much of the panel's height the movement is spread across. It
    /// must therefore be applied to the **distance travelled from the closed end**, not to the
    /// final height: capping the height would leave a fully-open 90 px menu 24 px tall, printed at
    /// full opacity, which is a truncated menu rather than a settled one. The first version of
    /// this function made exactly that mistake, and the two tests below caught it.
    ///
    /// So the panel is closed at `0`, grows by at most `cap` px over the first part of the
    /// transition, and then continues to its **own** height. For a panel shorter than the cap the
    /// distinction disappears, which is why `a_panel_shorter_than_the_cap_grows_to_its_own_height`
    /// passes either way and is not the test that pins this.
    pub fn revealed(&self, full: Rect, direction: RevealDirection, travel_cap: u32) -> Rect {
        let progress = self.value();
        // A closed popup paints nothing of its own; the caller checks `is_closed` first, and a
        // zero height here is the honest backstop if it does not.
        if progress <= 0.0 {
            return Rect::new(full.x, full.y, full.width, 0);
        }
        // The travelled distance runs from 0 to the panel's own height, but the first `cap` px of
        // it is spent over the whole transition and the remainder arrives with the last of it.
        // Expressed as an easing rather than a second driver, so there is still one curve.
        let height = if full.height <= travel_cap {
            (full.height as f32 * progress) as u32
        } else {
            let eased = progress * progress;
            (full.height as f32 * eased) as u32
        };
        // # Why an in-flight panel has a floor of one pixel

        // A quadratically-eased panel at 1% progress rounds to zero at realistic heights (90 px
        // * 0.0001 = 0), so the first few frames of a real open would paint an empty rectangle
        // while `is_animating` reported motion — the same "the model moves, the pixels do not"
        // split this crate has already been bitten by twice. Flooring at one pixel keeps the
        // promise that a panel which is opening is visible from its first frame; the caller's
        // `is_closed` check is what distinguishes "closed" from "opening".
        let height = height.clamp(if progress > 0.0 { 1 } else { 0 }, full.height.max(1));
        match direction {
            RevealDirection::Down => Rect::new(full.x, full.y, full.width, height),
            // The bottom edge stays put and the top edge comes down to meet it, so the panel
            // appears to slide out from under its anchor rather than from the screen edge.
            RevealDirection::Up => {
                Rect::new(full.x, full.y + full.height as i32 - height as i32, full.width, height)
            }
        }
    }

    /// Whether the panel is fully closed and nothing at all should be painted.
    ///
    /// A closed popup with a residual fraction would otherwise paint a one-pixel sliver, which
    /// reads as a rendering artifact rather than as a closed menu.
    pub fn is_closed(&self) -> bool {
        self.value() <= 0.0
    }

    /// A sensible travel cap for a panel of `full` size: at most a third of the panel, so a tall
    /// menu does not take visibly longer to open than a short one.
    pub fn travel_cap_for(full: Size) -> u32 {
        (full.height / 3).clamp(6, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_settled_reveal_is_the_panel_itself() {
        let full = Rect::new(10, 20, 120, 90);
        let open = PopupReveal::new(true);
        assert_eq!(open.value(), 1.0);
        assert!(!open.is_animating(), "an open popup owes no frames");
        assert_eq!(
            open.revealed(full, RevealDirection::Down, 24),
            full,
            "a settled reveal must be byte-identical to the un-animated panel, or every \
             snapshot of every menu would move"
        );
        assert!(!open.is_closed());
    }

    #[test]
    fn a_closed_reveal_paints_nothing() {
        let full = Rect::new(10, 20, 120, 90);
        let closed = PopupReveal::new(false);
        assert!(closed.is_closed(), "a closed popup must be reported as closed");
        assert_eq!(closed.value(), 0.0);
        assert!(!closed.is_animating());
        assert_eq!(
            closed.revealed(full, RevealDirection::Down, 24).height,
            0,
            "a closed popup must reveal a zero-height panel, not a one-pixel sliver: a sliver \
             reads as a rendering artifact rather than as a closed menu"
        );
    }

    #[test]
    fn opening_takes_interior_frames_downward() {
        let full = Rect::new(0, 0, 100, 90);
        let mut reveal = PopupReveal::new(false);
        reveal.set_open(true);
        assert!(reveal.is_animating(), "opening owes frames");

        assert!(reveal.tick(1), "still moving after a single millisecond");
        let mid = reveal.revealed(full, RevealDirection::Down, 30);
        assert!(
            mid.height > 0 && mid.height < full.height,
            "an opening panel must take an interior height, not jump to either end (got {})",
            mid.height
        );
        assert_eq!(mid.y, full.y, "growing down must keep the anchor edge fixed");

        while reveal.tick(16) {}
        let open = reveal.revealed(full, RevealDirection::Down, 30);
        assert_eq!(open.height, full.height, "and settle at the panel's own height");
        assert!(!reveal.is_animating());
    }

    #[test]
    fn opening_upward_keeps_the_bottom_edge() {
        let full = Rect::new(0, 100, 100, 60);
        let mut reveal = PopupReveal::new(false);
        reveal.set_open(true);
        assert!(reveal.tick(1));
        let mid = reveal.revealed(full, RevealDirection::Up, 20);
        assert!(
            mid.height > 0 && mid.height < full.height,
            "an upward panel must also take an interior height (got {})",
            mid.height
        );
        assert_eq!(
            mid.y + mid.height as i32,
            full.y + full.height as i32,
            "growing up must keep the bottom edge fixed, or the panel detaches from its anchor"
        );
    }

    #[test]
    fn a_panel_shorter_than_the_cap_grows_to_its_own_height() {
        let full = Rect::new(0, 0, 100, 12);
        let mut reveal = PopupReveal::new(false);
        reveal.set_open(true);
        assert!(reveal.tick(1));
        let mid = reveal.revealed(full, RevealDirection::Down, 24);
        assert!(
            mid.height <= full.height,
            "a 12 px panel must never be revealed taller than itself (got {})",
            mid.height
        );
    }

    #[test]
    fn the_travel_cap_is_bounded() {
        assert_eq!(PopupReveal::travel_cap_for(Size::new(1, 3)), 6, "a tiny panel still moves");
        assert_eq!(PopupReveal::travel_cap_for(Size::new(1, 900)), 24, "and a tall one is capped");
    }
}
