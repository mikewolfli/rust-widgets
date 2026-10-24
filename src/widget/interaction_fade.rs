// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The shared hover/press fade used by pointer-driven controls.
//!
//! # Why this exists
//!
//! [`PropertyDriver`] already owns the interpolation, the theme-priced duration and the
//! target, and [`Widget::tick`](crate::widget::Widget::tick) is the bus that advances a
//! control. What every control still spelled out by hand was the same three-step
//! choreography: keep two drivers, retarget them from the pointer state on every
//! interaction event, and report "am I moving?" from both. That boilerplate was the
//! reason the long-tail controls had no transition at all.
//!
//! `InteractionFade` states that choreography once. A control owns one of these, calls
//! [`sync`](Self::sync) from its `MouseEnter`/`MouseLeave`/`MousePress`/`MouseRelease`
//! arms, and forwards [`tick`](Self::tick)/[`is_animating`](Self::is_animating) from its
//! `Widget` implementation.
//!
//! # Rest is exactly zero
//!
//! Both drivers are built with `PropertyDriver::at(0.0, MotionSlot::Fast)`, so a freshly
//! constructed control paints its resting appearance byte-for-byte. A draw path multiplies
//! its tint by [`intensity`](Self::intensity) and can therefore branch on `> 0.0`: until a
//! real interaction moves a driver, the tint is not applied at all.
//!
//! # Reduced motion
//!
//! No branch lives here. The drivers are priced by [`MotionSlot::Fast`], whose duration is
//! read through the existing timing path ([`TransitionTempo::duration_ms`], which goes
//! through `motion_tokens` and applies the same reduced-motion collapse as
//! `effective_duration`). With reduced motion in force that duration is zero, so the first
//! [`tick`](Self::tick) reaches the target and reports that no further frame is needed.
//!
//! [`TransitionTempo::duration_ms`]: crate::style::animation::TransitionTempo::duration_ms

use crate::style::{MotionSlot, PropertyDriver};

/// Two interaction drivers — hover and press — shared by every pointer-driven control.
///
/// The fields are public on purpose: a draw path reads them directly
/// (`fade.hover.value()`, `fade.press.value()`) rather than going through a second
/// accessor that could drift from what the drivers actually hold.
pub struct InteractionFade {
    /// The hover wash: `0.0` at rest, `1.0` while the pointer is over the control.
    pub hover: PropertyDriver,
    /// The press wash: `0.0` at rest, `1.0` while a gesture holds the control.
    ///
    /// Kept separate from [`Self::hover`] so a press that arrives without a preceding
    /// `MouseEnter` still tints, and so a draw path can treat the gesture as its own channel
    /// instead of inferring it from a single combined fraction.
    pub press: PropertyDriver,
}

impl InteractionFade {
    /// A fade resting at exactly `0.0` in both drivers, priced by [`MotionSlot::Fast`].
    ///
    /// `Fast` is the tempo the crate's pointer feedback already uses: a hover is a direct
    /// reaction to the pointer, not a control's own state change.
    pub fn new() -> Self {
        Self {
            hover: PropertyDriver::at(0.0, MotionSlot::Fast),
            press: PropertyDriver::at(0.0, MotionSlot::Fast),
        }
    }

    /// Retargets both drivers from the control's current pointer state.
    ///
    /// Returns whether either driver still owes frames, which is the value an event arm
    /// can hand back as "please schedule the next frame". Calling it with the same state
    /// as the previous call is a no-op in the drivers: `PropertyDriver::set_target`
    /// re-aims rather than restarts, so repeated events cannot make the fade stutter.
    pub fn sync(&mut self, hovered: bool, pressed: bool) -> bool {
        self.hover.set_target(if hovered { 1.0 } else { 0.0 });
        self.press.set_target(if pressed { 1.0 } else { 0.0 });
        self.is_animating()
    }

    /// Advances both drivers by `delta_ms`.
    ///
    /// Both are advanced unconditionally: short-circuiting on the first moving driver
    /// would leave the other one stuck for the frames in which the first was settling.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let hover_moving = self.hover.tick(delta_ms);
        let press_moving = self.press.tick(delta_ms);
        hover_moving || press_moving
    }

    /// Whether either driver is between two values.
    ///
    /// This is the answer [`Widget::is_animating`](crate::widget::Widget::is_animating)
    /// needs: the query never advances a driver, so a frame loop can ask every control
    /// whether it owes frames without moving any of them.
    pub fn is_animating(&self) -> bool {
        self.hover.is_moving() || self.press.is_moving()
    }

    /// The tint strength a draw path should apply, in `0.0..=1.0`.
    ///
    /// This is the **stronger** of the two drivers: a press that arrives without a preceding
    /// hover still tints, while a press on top of a hover is one wash rather than two. At rest
    /// both values are exactly `0.0`, so this is exactly `0.0` and a caller can test `> 0.0`
    /// to skip the tint entirely.
    pub fn intensity(&self) -> f32 {
        self.hover.value().max(self.press.value()).clamp(0.0, 1.0)
    }
}

impl Default for InteractionFade {
    /// The resting fade, matching [`InteractionFade::new`].
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::InteractionFade;

    /// A new fade is still at the rest end, and that value is exactly zero.
    ///
    /// The exactness matters: draw paths skip their tint when the value is `0.0`, so a
    /// freshly built control must produce the same pixels it did before the fade existed.
    #[test]
    fn a_new_fade_rests_at_exactly_zero() {
        let fade = InteractionFade::new();
        assert_eq!(fade.hover.value(), 0.0);
        assert_eq!(fade.press.value(), 0.0);
        assert_eq!(fade.intensity(), 0.0);
        assert!(!fade.is_animating(), "a resting control owes no frames");
    }

    /// Hovering retargets only the hover driver and immediately owes frames.
    #[test]
    fn sync_retargets_the_driver_that_changed() {
        let mut fade = InteractionFade::new();
        assert!(fade.sync(true, false), "a hover arrival owes frames at once");
        assert_eq!(fade.hover.target(), 1.0);
        assert_eq!(fade.press.target(), 0.0);
        assert!(fade.is_animating());
    }

    /// Repeating the same state does not restart the movement.
    #[test]
    fn syncing_the_same_state_does_not_reset_progress() {
        let mut fade = InteractionFade::new();
        let _ = fade.sync(true, false);
        let _ = fade.tick(16);
        let partway = fade.hover.value();
        assert!(partway > 0.0 && partway < 1.0, "the hover is in flight: {partway}");

        let _ = fade.sync(true, false);
        assert_eq!(
            fade.hover.value(),
            partway,
            "a repeated MouseEnter must re-aim the same movement, not restart it"
        );
    }

    /// Ticking to the target settles it exactly and then reports no more frames.
    #[test]
    fn a_fade_settles_exactly_at_the_target() {
        let mut fade = InteractionFade::new();
        let _ = fade.sync(true, false);
        let mut frames = 0;
        while fade.tick(16) {
            frames += 1;
            assert!(frames < 500, "the fade must terminate rather than tick forever");
        }
        assert_eq!(fade.hover.value(), 1.0, "the hover arrives exactly at its target");
        assert_eq!(fade.press.value(), 0.0, "an untouched driver does not move");
        assert!(!fade.is_animating());
    }

    /// Reduced motion needs no branch in this type: the zero-duration token settles the
    /// fade on the first tick.
    #[test]
    fn reduced_motion_settles_on_the_first_tick() {
        use crate::compat::Box;
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Reduced;
        impl EnvironmentProvider for Reduced {
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::ReduceMotion
            }
        }

        let _guard = crate::style::theme_test_guard();
        let previous = install_environment(Box::new(Reduced));

        let mut fade = InteractionFade::new();
        let _ = fade.sync(true, false);
        let moving = fade.tick(16);
        assert!(!moving, "a zero-duration fade settles on the first tick");
        assert_eq!(fade.hover.value(), 1.0, "and it reaches the target, not half way");
        assert!(!fade.is_animating());

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }
    }
}
