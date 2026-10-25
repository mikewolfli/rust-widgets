// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Host-reported mobile lifecycle, orientation, and the animation-clock link (BLUE28 B28.24).
//!
//! # Why this is a process-wide module
//!
//! The lifecycle callback arrives from the OS on the platform thread, while the frame loop may
//! run on another thread, and the `mini`/`embedded` profiles have no thread-local frame ledger.
//! Suspend/resume is a fact about the application, so it lives in atomics rather than in any one
//! backend's state. Android, iOS, and HarmonyOS all report into this one place, which is what
//! makes [`Platform::suspend`](crate::platform::Platform) mean the same thing on each.
//!
//! # Unknown is not Active
//!
//! The initial phase is `Unknown`: `lifecycle_state` answers `None` until the host reports one,
//! and `orientation` answers `None` until a rotation is reported. A backend that has not heard
//! from its host must not claim the app is in the foreground or the device is portrait.
//!
//! # The animation clock
//!
//! `set_state` stops the animation clock for `Inactive` and `Background` and restarts it for
//! `Active`. The restart leaves a one-frame zero-delta request in
//! [`crate::widget::runtime`], so time spent in the background is not replayed as one huge step.

use crate::compat::atomic::{AtomicI32, AtomicU64, AtomicU8, Ordering};
use crate::platform::types::{MobileLifecycleState, ScreenOrientation};

/// No lifecycle phase has been reported.
const UNKNOWN: u8 = 0xFF;
/// `MobileLifecycleState::Active`.
const ACTIVE: u8 = 0;
/// `MobileLifecycleState::Inactive`.
const INACTIVE: u8 = 1;
/// `MobileLifecycleState::Background`.
const BACKGROUND: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(UNKNOWN);
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// `Display.getRotation()` value, or `-1` before the host reports one.
static ROTATION: AtomicI32 = AtomicI32::new(-1);
static ROTATION_GENERATION: AtomicU64 = AtomicU64::new(0);

fn encode(state: MobileLifecycleState) -> u8 {
    match state {
        MobileLifecycleState::Active => ACTIVE,
        MobileLifecycleState::Inactive => INACTIVE,
        MobileLifecycleState::Background => BACKGROUND,
    }
}

fn decode(value: u8) -> Option<MobileLifecycleState> {
    match value {
        ACTIVE => Some(MobileLifecycleState::Active),
        INACTIVE => Some(MobileLifecycleState::Inactive),
        BACKGROUND => Some(MobileLifecycleState::Background),
        _ => None,
    }
}

/// Records a lifecycle phase and links it to the animation clock.
///
/// Returns `true` on every accepted value. The generation changes only when the phase actually
/// changes, so a host that reports the same phase twice does not look like a transition.
pub fn set_state(state: MobileLifecycleState) -> bool {
    let previous = STATE.swap(encode(state), Ordering::SeqCst);
    let current = encode(state);
    if previous != current {
        GENERATION.fetch_add(1, Ordering::SeqCst);
        // `Inactive` (a sheet, a call, the app switcher) and `Background` both stop the clock;
        // only `Active` restarts it. A repeated report is not a transition, so it does not leave
        // a spurious zero-delta request for the next frame.
        crate::set_animation_suspended(current != ACTIVE);
    }
    true
}

/// The phase the host last reported, or `None` before it reports one.
pub fn lifecycle_state() -> Option<MobileLifecycleState> {
    decode(STATE.load(Ordering::SeqCst))
}

/// A revision that changes on each lifecycle transition.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::SeqCst)
}

/// Records `Display.getRotation()` (`0..=3`) or `-1` for unknown.
///
/// Returns `false` for a value outside the documented range, so a host passing a made-up number
/// does not turn into a fabricated orientation.
pub fn set_rotation(rotation: i32) -> bool {
    if !(-1..=3).contains(&rotation) {
        return false;
    }
    let previous = ROTATION.swap(rotation, Ordering::SeqCst);
    if previous != rotation {
        ROTATION_GENERATION.fetch_add(1, Ordering::SeqCst);
    }
    true
}

/// The orientation implied by the reported rotation, or `None` before one is reported.
///
/// The mapping is Android's own `Surface.ROTATION_*` set; iOS reports the same four phases
/// through [`set_orientation`], so both platforms answer from one place.
pub fn orientation() -> Option<ScreenOrientation> {
    match ROTATION.load(Ordering::SeqCst) {
        0 => Some(ScreenOrientation::Portrait),
        1 => Some(ScreenOrientation::LandscapeLeft),
        2 => Some(ScreenOrientation::PortraitUpsideDown),
        3 => Some(ScreenOrientation::LandscapeRight),
        _ => None,
    }
}

/// Records a `ScreenOrientation` directly (iOS' `UIInterfaceOrientation` mapping).
pub fn set_orientation(orientation: ScreenOrientation) -> bool {
    let rotation = match orientation {
        ScreenOrientation::Portrait => 0,
        ScreenOrientation::LandscapeLeft => 1,
        ScreenOrientation::PortraitUpsideDown => 2,
        ScreenOrientation::LandscapeRight => 3,
    };
    set_rotation(rotation)
}

/// A revision that changes when the reported rotation changes.
pub fn rotation_generation() -> u64 {
    ROTATION_GENERATION.load(Ordering::SeqCst)
}

/// Maps a `Configuration.orientation` constant (`1` portrait, `2` landscape) when a host has no
/// rotation value. `None` for any other value.
pub fn set_axis_orientation(axis: i32) -> bool {
    match axis {
        1 => set_orientation(ScreenOrientation::Portrait),
        2 => set_orientation(ScreenOrientation::LandscapeLeft),
        _ => false,
    }
}

#[cfg(test)]
pub(crate) static TEST_LOCK: crate::compat::Mutex<()> = crate::compat::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    /// The atomics are process-wide, so these tests serialize their own transitions rather than
    /// asserting an initial value another test may have changed.
    #[test]
    fn a_reported_phase_is_read_back_and_bumps_once() {
        let _guard = crate::compat::lock(&TEST_LOCK);
        assert!(set_state(MobileLifecycleState::Background));
        assert_eq!(lifecycle_state(), Some(MobileLifecycleState::Background));
        let before = generation();
        assert!(set_state(MobileLifecycleState::Background));
        assert_eq!(generation(), before, "a repeated phase is not a transition");
        assert!(set_state(MobileLifecycleState::Active));
        assert_eq!(generation(), before + 1);
        assert!(!crate::animation_suspended());
    }

    #[test]
    fn a_rotation_maps_to_the_four_orientations() {
        let _guard = crate::compat::lock(&TEST_LOCK);
        assert!(set_rotation(-1));
        assert_eq!(orientation(), None, "unknown before a rotation is reported");
        assert!(set_rotation(0));
        assert_eq!(orientation(), Some(ScreenOrientation::Portrait));
        assert!(set_rotation(1));
        assert_eq!(orientation(), Some(ScreenOrientation::LandscapeLeft));
        assert!(set_rotation(2));
        assert_eq!(orientation(), Some(ScreenOrientation::PortraitUpsideDown));
        assert!(set_rotation(3));
        assert_eq!(orientation(), Some(ScreenOrientation::LandscapeRight));
        assert!(!set_rotation(4), "an out-of-range rotation is refused");
    }

    #[test]
    fn an_axis_report_names_only_the_axis() {
        let _guard = crate::compat::lock(&TEST_LOCK);
        assert!(set_axis_orientation(2));
        assert_eq!(orientation(), Some(ScreenOrientation::LandscapeLeft));
        assert!(!set_axis_orientation(3));
    }

    #[test]
    fn suspension_follows_the_phase() {
        let _guard = crate::compat::lock(&TEST_LOCK);
        set_state(MobileLifecycleState::Inactive);
        // The animation clock only exists in a profile that has the animation bus. A sized
        // profile (`mini`/`embedded`) has no clock to stop, and its `animation_suspended`
        // documents that answer — so the phase is still recorded, but the query is `false`
        // there by contract rather than because the transition failed.
        #[cfg(not(alloc_frugal))]
        assert!(crate::animation_suspended());
        #[cfg(alloc_frugal)]
        assert!(!crate::animation_suspended(), "a sized profile has no animation clock to suspend");
        set_state(MobileLifecycleState::Active);
        assert!(!crate::animation_suspended());
    }
}
