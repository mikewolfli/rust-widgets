// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The **feel** of a scrollable: how far a drag must travel, how fast a flick counts, and how a
//! release settles.
//!
//! # Why this lives in `core` and not in `gesture`
//!
//! It was written in `src/gesture/`, which is gated on the `touch` feature — so a control that is
//! always built (`Carousel`) could not use it without pulling in the whole gesture recognizer system,
//! and the `check_profiles` gate caught exactly that (`android`'s feature set has no `touch`).
//! Nothing here touches a recognizer: it is arithmetic over four thresholds. Moving it beside
//! [`crate::core::MediaClock`] is the same correction — timing and feel are values, not subsystems.
//!
//! # The gap this closes (BLUE24 §12 U-3)
//!
//! Three controls each carried their own hand-tuned constants — a carousel's
//! `SWIPE_THRESHOLD_FRACTION`, `FLICK_VELOCITY_PX_PER_SEC` and `MIN_FLICK_FRACTION` — and each set of
//! numbers was a private `const` no caller could see or change. Two consequences:
//!
//! 1. **A host could not tune the feel.** A large touchscreen and a mouse want different thresholds;
//!    a design tool wants snapping to be visible; a kiosk wants it to be impossible to land between
//!    pages. With the numbers compiled in, the only way to change any of that was to patch the crate.
//! 2. **The numbers could not be compared.** "Is the carousel's flick threshold the same as the
//!    scroller's?" had no answer, because they were in two files in different units of intent.
//!
//! # What this deliberately is not
//!
//! It is **not** a simulation. A physics integrator (force, mass, damping, per-frame spring) would
//! need a `tick` and a state vector, and it would make "does a flick page?" depend on the frame rate
//! the host happened to run at. What a UI needs from "physics" is four decisions, and each is a
//! threshold on a value the gesture already produces:
//!
//! ```text
//! drag shorter than `drag_min_fraction`   -> never pages, however fast
//! drag past `page_fraction`               -> pages on distance
//! release faster than `flick_velocity`    -> pages on speed
//! otherwise                               -> snaps back
//! ```
//!
//! # Why the unit is pixels per second, and why that matters here
//!
//! The crate had a real defect in this area: two recognizers emitted velocity in different units —
//! one in logical pixels per millisecond and one per second, a factor of 1000 — and nothing could
//! detect it because both were `f32`. `src/gesture/swipe.rs` records the fix and now scales to
//! **pixels per second**, and `Event::Swipe::velocity` documents that unit. This type's fields are in
//! the same unit, stated in each field's name, so a caller cannot pass a per-millisecond number by
//! accident: the name says `px_per_sec`.

/// How a scrollable or pageable control responds to a drag and a release.
///
/// All distances are **fractions of the control's own extent**, not pixels, so one `ScrollPhysics`
/// reads the same on a phone and on a wall display. All speeds are **logical pixels per second**,
/// matching [`crate::event::Event::Swipe`]'s documented unit.
///
/// See the module docs for what this is (four thresholds) and what it is not (a simulation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollPhysics {
    /// A drag past this fraction of the extent pages on distance alone.
    ///
    /// Fraction rather than pixels because the gesture's meaning is proportional: "a fifth of the way
    /// across" is a swipe on any screen, while "80 pixels" is a swipe on a phone and a nudge on a
    /// desktop.
    pub page_fraction: f32,
    /// The release speed at or above which a *short* drag still pages.
    ///
    /// This is the rule that makes a flick work: without it, a quick flick that travels less than
    /// [`ScrollPhysics::page_fraction`] snaps back, which reads as the control ignoring the gesture.
    pub flick_velocity_px_per_sec: f32,
    /// A drag shorter than this fraction of the extent never pages, however fast it was.
    ///
    /// The floor that makes the velocity rule safe. A pointer that jitters a few pixels inside one
    /// frame computes an enormous instantaneous speed, and that is noise rather than intent — so a
    /// very short drag is refused before its speed is consulted.
    pub drag_min_fraction: f32,
    /// Whether a release settles onto a page rather than stopping where the drag let go.
    ///
    /// `true` for a pager (a carousel, a wizard step): landing between two pages shows half of each,
    /// which is a state the control has no layout for. `false` for a continuous scroller, where
    /// stopping under the finger is the expected behaviour.
    pub snaps_to_page: bool,
}

impl Default for ScrollPhysics {
    /// The crate's existing feel: the carousel's three constants, adopted as the default so nothing
    /// changes for a caller that does not ask for anything else.
    ///
    /// The values are not invented here — they are the ones three controls converged on
    /// independently, and this type's whole purpose is to be their **single named home** rather than
    /// a fourth opinion.
    fn default() -> Self {
        Self {
            page_fraction: 0.18,
            flick_velocity_px_per_sec: 400.0,
            drag_min_fraction: 0.02,
            snaps_to_page: true,
        }
    }
}

impl ScrollPhysics {
    /// The default pager feel: a drag past 18% pages, a flick at 400 px/s pages.
    pub const fn pager() -> Self {
        Self {
            page_fraction: 0.18,
            flick_velocity_px_per_sec: 400.0,
            drag_min_fraction: 0.02,
            snaps_to_page: true,
        }
    }

    /// A continuous scroller: no snapping, and paging thresholds that are not consulted.
    pub const fn scroller() -> Self {
        Self {
            page_fraction: 1.0,
            flick_velocity_px_per_sec: f32::INFINITY,
            drag_min_fraction: 0.0,
            snaps_to_page: false,
        }
    }

    /// A feel that makes paging **deliberate**: a drag must travel most of the way, and a flick must
    /// be fast.
    ///
    /// For a kiosk or a wizard, where landing on the wrong page is a mistake rather than a
    /// mis-tap.
    pub const fn deliberate() -> Self {
        Self {
            page_fraction: 0.5,
            flick_velocity_px_per_sec: 1200.0,
            drag_min_fraction: 0.1,
            snaps_to_page: true,
        }
    }

    /// Decides what a release does, given how far it travelled and how fast.
    ///
    /// `travel` and `extent` are in the same unit (logical pixels); `velocity_px_per_sec` is in the
    /// unit its name says. Returns `Some(direction)` where `-1` is toward the start and `1` toward
    /// the end, or `None` for "snap back / do not page".
    ///
    /// # Why one function and not four comparisons at the call site
    ///
    /// The four rules are **ordered** — the short-drag floor is checked before the speed, because a
    /// jittering pointer must not page — and a call site that reorders them changes the feel without
    /// changing any number. Stating the order once, here, is what makes "the carousel pages on a
    /// flick" a property of the physics rather than of the control's `if` chain.
    pub fn release_direction(
        &self,
        travel: f32,
        extent: f32,
        velocity_px_per_sec: f32,
    ) -> Option<i8> {
        if !self.snaps_to_page || !travel.is_finite() || !extent.is_finite() || extent <= 0.0 {
            return None;
        }
        // A non-finite speed is treated as "no speed": `atan2` on a zero-length gesture yields NaN,
        // and a NaN comparison is false, which would silently fall through to the distance rule and
        // page on a gesture that had no speed information at all.
        //
        // `INFINITY` is deliberately **not** collapsed with `NaN`: it is a legitimate "very fast"
        // reading (a test fixture, or a speed that saturated), and treating it as zero would refuse
        // the fastest possible flick.
        let speed = if velocity_px_per_sec.is_nan() { 0.0 } else { velocity_px_per_sec.abs() };

        let fraction = (travel / extent).abs();
        // The floor comes **first**: a pointer that jittered two pixels in one frame can compute a
        // huge instantaneous speed, and that is noise, not intent.
        if fraction < self.drag_min_fraction {
            return None;
        }
        // Distance, or a fast enough release.
        if fraction >= self.page_fraction || speed >= self.flick_velocity_px_per_sec {
            return Some(if travel > 0.0 { 1 } else { -1 });
        }
        None
    }

    /// Whether a drag of `travel` at `velocity` would page — the question a test asks.
    ///
    /// A convenience over [`ScrollPhysics::release_direction`] so a caller that only wants the
    /// yes/no does not have to compare against `None` and then discard the direction.
    pub fn would_page(&self, travel: f32, extent: f32, velocity_px_per_sec: f32) -> bool {
        self.release_direction(travel, extent, velocity_px_per_sec).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTENT: f32 = 400.0;

    /// The default feel is the one the crate already shipped, so adopting it changes nothing.
    #[test]
    fn the_default_is_the_crates_existing_feel() {
        let physics = ScrollPhysics::default();
        assert_eq!(physics.page_fraction, 0.18);
        assert_eq!(physics.flick_velocity_px_per_sec, 400.0);
        assert_eq!(physics.drag_min_fraction, 0.02);
        assert!(physics.snaps_to_page);
        assert_eq!(ScrollPhysics::pager(), ScrollPhysics::default());
    }

    /// A drag past the threshold pages on distance, whichever way it went.
    #[test]
    fn a_long_drag_pages_on_distance() {
        let physics = ScrollPhysics::default();
        // 0.4 x 400 = 160 px, well past the 18% threshold, with no speed at all.
        assert_eq!(physics.release_direction(160.0, EXTENT, 0.0), Some(1));
        assert_eq!(physics.release_direction(-160.0, EXTENT, 0.0), Some(-1));
    }

    /// A short **flick** pages on speed — the rule that stops a quick gesture snapping back.
    #[test]
    fn a_short_fast_flick_pages_on_speed() {
        let physics = ScrollPhysics::default();
        // 60 px is 15%: under the distance threshold, but at 900 px/s it is a flick.
        assert_eq!(physics.release_direction(60.0, EXTENT, 900.0), Some(1));
        // The same travel at 100 px/s is a slow nudge and snaps back.
        assert_eq!(physics.release_direction(60.0, EXTENT, 100.0), None);
    }

    /// The short-drag floor is checked **before** the speed, so a jittering pointer cannot page.
    #[test]
    fn a_jittering_pointer_does_not_page_however_fast() {
        let physics = ScrollPhysics::default();
        // 4 px is 1%, under the 2% floor. A pointer that moved 4 px inside one 8 ms frame computes
        // 500 px/s, which is above the flick threshold — and must still not page.
        assert_eq!(physics.release_direction(4.0, EXTENT, 500.0), None);
        assert!(!physics.would_page(4.0, EXTENT, 500.0));
    }

    /// A continuous scroller never pages, whatever the gesture.
    #[test]
    fn a_scroller_never_pages() {
        let physics = ScrollPhysics::scroller();
        assert!(!physics.would_page(1_000.0, EXTENT, 5_000.0), "a scroller stops where it is");
        assert!(!physics.snaps_to_page);
    }

    /// The deliberate feel refuses a gesture that the default would accept.
    #[test]
    fn the_deliberate_feel_is_harder_to_trigger() {
        let physics = ScrollPhysics::deliberate();
        // 100 px is 25%: past the default's 18%, but not past the deliberate 50%.
        assert!(!physics.would_page(100.0, EXTENT, 0.0));
        assert!(ScrollPhysics::default().would_page(100.0, EXTENT, 0.0));

        // And a 600 px/s flick is fast by the default rule and slow by this one.
        assert!(ScrollPhysics::default().would_page(80.0, EXTENT, 600.0));
        assert!(!physics.would_page(80.0, EXTENT, 600.0));
    }

    /// A degenerate extent is refused rather than dividing by zero.
    #[test]
    fn a_degenerate_extent_does_not_page_or_panic() {
        let physics = ScrollPhysics::default();
        assert_eq!(
            physics.release_direction(50.0, 0.0, 1_000.0),
            None,
            "a zero extent pages nothing"
        );
        assert_eq!(physics.release_direction(50.0, f32::NAN, 1_000.0), None);
        assert_eq!(physics.release_direction(50.0, -10.0, 1_000.0), None);
    }

    /// A frame rate that produced a NaN speed must not fall through to the distance rule.
    ///
    /// `INFINITY` is a different case and is *not* collapsed with it: it is a legitimate "very
    /// fast" value, and treating it as no speed would refuse the fastest possible flick.
    #[test]
    fn a_nan_speed_is_treated_as_no_speed() {
        let physics = ScrollPhysics::default();
        // 40 px is 10%: under the distance threshold, so only the speed could page it. A NaN speed
        // must not, and must not panic either.
        assert_eq!(physics.release_direction(40.0, EXTENT, f32::NAN), None);

        // An infinite speed is fast, not unusable.
        assert_eq!(physics.release_direction(40.0, EXTENT, f32::INFINITY), Some(1));
        // The sign of the travel decides the direction, not the sign of the speed: a flick back
        // toward the start while the finger was still drifting forward pages backwards.
        assert_eq!(physics.release_direction(-40.0, EXTENT, f32::INFINITY), Some(-1));
    }
}
