// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The event layer's single mouse double-click judgement (W-203).
//!
//! # The three tracks this collapses
//!
//! Double-click semantics existed in three independent places: the touch recogniser
//! (`gesture::DoubleTapGesture`, using [`crate::gesture::tuning`]), the host's
//! [`Event::MouseDoubleClick`](crate::event::Event::MouseDoubleClick) (each backend uses its own
//! OS interval), and the JSON compatibility key. The first two could disagree about the same
//! physical gesture: a touch double-tap and a mouse double-click had different thresholds and no
//! shared definition.
//!
//! This tracker makes the **gesture tuning the one rule**: it calls
//! `crate::gesture::is_double_tap_pair`, the same predicate `DoubleTapGesture` uses for its
//! second tap, so the interval and the movement bound cannot drift between the two input
//! families. The platform's own detection is then normalised to that rule rather than accepted
//! blindly.
//!
//! The JSON side (`src/json/event_route.rs`) is outside this module's write scope; it still
//! rejects `on_double_click` and needs to route that key to the published `double_clicked` name.

#[cfg(full_widgets)]
use crate::compat::{lock, Mutex, Vec};
use crate::core::ObjectId;
use crate::core::Point;
#[cfg(full_widgets)]
use alloc::sync::{Arc, Weak};

/// Process-wide weak handles to the tracker of every live [`DoubleClickTracker`] owner (EV-X03).
///
/// Widget destruction happens in `widget::runtime`, which has no route to the loop's tracker:
/// the tracker lives on the host thread (and moves onto the loop thread while it runs), while
/// `unregister` is a plain registry operation. The tracker is shared through an `Arc`, so a weak
/// handle registered here lets the destroy chain drop a dead target's last press without owning
/// the loop. Weak handles let the registry prune itself when a loop is dropped.
///
/// `full_widgets` only: that is the only profile with a widget destroy chain.
#[cfg(full_widgets)]
static DOUBLE_CLICK_PURGE_ARMS: crate::compat::OnceLock<
    Mutex<Vec<Weak<Mutex<DoubleClickTracker>>>>,
> = crate::compat::OnceLock::new();

/// Publishes a tracker so [`forget_double_click_for_target_everywhere`] can reach it.
#[cfg(full_widgets)]
pub(crate) fn register_double_click_purge_arm(tracker: &Arc<Mutex<DoubleClickTracker>>) {
    let registry = DOUBLE_CLICK_PURGE_ARMS.get_or_init(|| Mutex::new(Vec::new()));
    let mut arms = lock(registry);
    arms.retain(|weak| weak.strong_count() > 0);
    arms.push(Arc::downgrade(tracker));
}

/// Drops the recorded press for `target` in every live double-click tracker (EV-X03).
///
/// This is the destroy-chain entry point. [`DoubleClickTracker::forget`] cannot be called from
/// `widget::runtime` — the loop is not reachable from there — so the same cleanup is applied
/// through the shared tracker handle instead. Without it, a new control that reuses an id could
/// inherit the dead control's last press and report a phantom double-click within the interval.
#[cfg(full_widgets)]
pub(crate) fn forget_double_click_for_target_everywhere(target: ObjectId) {
    let Some(registry) = DOUBLE_CLICK_PURGE_ARMS.get() else {
        return;
    };
    let mut arms = lock(registry);
    arms.retain(|weak| {
        let Some(tracker) = weak.upgrade() else {
            return false;
        };
        lock(&tracker).forget(target);
        true
    });
}

/// The interval and movement bound used when the gesture module is not compiled in.
///
/// `gesture` is gated behind the `touch` feature because it exists to recognise touch input.
/// A build without it has no tuning tokens to consult, so the event layer's own judge uses the
/// same defaults [`crate::gesture::GestureTuning`] documents. The *rule* below is shared either
/// way; only the values differ, and only in a build where the touch subsystem does not exist.
#[cfg(not(feature = "touch"))]
const DEFAULT_DOUBLE_CLICK_TIMEOUT_MS: u64 = 400;
#[cfg(not(feature = "touch"))]
const DEFAULT_MAX_STATIONARY_DISTANCE: f32 = 15.0;

/// How far apart the two taps of a double-tap may land, as a multiple of the stationary bound
/// (W-03).
///
/// # Why the two thresholds differ, stated once
///
/// A **single** tap's own travel must stay below `GestureTuning::max_stationary_distance`
/// (`gesture/tap.rs`), because moving farther than that is a drag, not a tap. A double-tap is two
/// *separate* taps, and each may wander within that own bound, so the two down points together can
/// legitimately be up to twice the single-tap bound apart.
///
/// The factor is named here rather than written as `* 2.0` at the comparison, because the 2× gap
/// between `TapGesture`'s bound and `is_double_click_pair`'s was previously undocumented and read
/// like a copy/paste drift. Any future change to the ratio has exactly one place to change, and
/// both the single-tap and the double-tap recognisers read it.
pub const DOUBLE_TAP_SEPARATION_FACTOR: f32 = 2.0;

/// Whether two `(position, monotonic_ms)` contacts form a double-click pair.
///
/// This is the single spelling of the rule. `gesture::is_double_tap_pair` delegates here with its
/// runtime tuning, and this module's tracker delegates here with the same tuning, so a change to
/// the interval or movement bound cannot move one input family without the other (W-203).
///
/// The movement bound is `max_stationary_distance * DOUBLE_TAP_SEPARATION_FACTOR`; see that
/// constant for why a two-tap pair is allowed twice the single tap's bound (W-03).
pub(crate) fn is_double_click_pair(
    first: (Point, u64),
    second: (Point, u64),
    timeout_ms: u64,
    max_stationary_distance: f32,
) -> bool {
    let elapsed = second.1.saturating_sub(first.1);
    let dx = (second.0.x as i64 - first.0.x as i64) as f32;
    let dy = (second.0.y as i64 - first.0.y as i64) as f32;
    elapsed <= timeout_ms
        && (dx * dx + dy * dy).sqrt() < max_stationary_distance * DOUBLE_TAP_SEPARATION_FACTOR
}

/// The tuning in force for mouse double-click judging.
fn pair_tuning() -> (u64, f32) {
    #[cfg(feature = "touch")]
    {
        let tuning = crate::gesture::tuning();
        (tuning.double_tap_timeout_ms, tuning.max_stationary_distance)
    }
    #[cfg(not(feature = "touch"))]
    {
        (DEFAULT_DOUBLE_CLICK_TIMEOUT_MS, DEFAULT_MAX_STATIONARY_DISTANCE)
    }
}

/// Applies the shared rule with the tuning this build has available.
fn pair_matches(first: (Point, u64), second: (Point, u64)) -> bool {
    let (timeout_ms, max_move) = pair_tuning();
    is_double_click_pair(first, second, timeout_ms, max_move)
}

/// The most recent primary press, kept so the next click can be judged against it.
#[derive(Debug, Clone, Copy)]
struct LastPress {
    target: ObjectId,
    pos: Point,
    /// Monotonic milliseconds since the loop's origin.
    at_ms: u64,
    button: u32,
    modifiers: u32,
}

/// Judges mouse double-clicks with the gesture engine's timing.
///
/// # Why one record, not a map
///
/// A double-click is two clicks on the **same** target: a press on a different target starts a
/// new run, so keeping only the latest press is sufficient and cannot grow with the number of
/// controls the user has ever clicked.
#[derive(Debug, Default)]
pub struct DoubleClickTracker {
    last: Option<LastPress>,
}

impl DoubleClickTracker {
    /// Creates a tracker that has not seen a press.
    pub fn new() -> Self {
        Self { last: None }
    }

    /// Records a primary press and reports whether it completes a double click.
    ///
    /// Used for hosts whose pointer path delivers presses (and for a loop that sees the second
    /// press of a double click). A non-primary button never arms the tracker: double-click is a
    /// primary-button gesture in the event contract, and treating every button alike would make
    /// two rapid right-clicks activate a control that only listens for a primary double-click.
    pub fn observe_press(
        &mut self,
        target: ObjectId,
        pos: Point,
        button: u32,
        modifiers: u32,
        now_ms: u64,
    ) -> bool {
        const PRIMARY: u32 = crate::event::mouse_button::PRIMARY;
        if button != PRIMARY {
            self.last = Some(LastPress { target, pos, at_ms: now_ms, button, modifiers });
            return false;
        }
        let double = matches!(
            self.last,
            Some(last) if last.target == target
                && last.button == button
                && pair_matches((last.pos, last.at_ms), (pos, now_ms))
        );
        // A recognised pair consumes the record, so a third click starts a fresh run instead of
        // chaining into a second double-click (the classic triple-click bug).
        self.last = if double {
            None
        } else {
            Some(LastPress { target, pos, at_ms: now_ms, button, modifiers })
        };
        double
    }

    /// Decides whether a host-produced `MouseDoubleClick` agrees with the unified interval.
    ///
    /// `true` means deliver it as a double-click. `false` means the host's detector fired outside
    /// the gesture engine's interval; the caller replays it as a single `MousePress` so the click
    /// is not lost.
    ///
    /// With no recorded press the host is the only evidence available, so the answer is `true`.
    /// A loop whose host does not deliver presses (every desktop backend sends the press, but a
    /// synthetic host need not) must not lose a double-click it cannot contradict.
    pub fn accept_platform_double_click(
        &mut self,
        target: ObjectId,
        pos: Point,
        button: u32,
        now_ms: u64,
    ) -> bool {
        const PRIMARY: u32 = crate::event::mouse_button::PRIMARY;
        if button != PRIMARY {
            return true;
        }
        match self.last {
            None => true,
            Some(last) => {
                let agrees = last.target == target
                    && last.button == button
                    && pair_matches((last.pos, last.at_ms), (pos, now_ms));
                if agrees {
                    // Consume the pair, so the next click starts a new run.
                    self.last = None;
                }
                agrees
            }
        }
    }

    /// The modifier bits carried by the most recent press, or `0` when none is recorded.
    ///
    /// Used when a host double-click is replayed as a single press: the platform's double-click
    /// event does not carry modifiers, and the press that preceded it does, so the replay keeps
    /// them instead of silently dropping a Shift/Ctrl click.
    pub fn last_press_modifiers(&self) -> u32 {
        self.last.map_or(0, |last| last.modifiers)
    }

    /// Drops the recorded press for `target` (the control is being released).
    ///
    /// A record survives until it is superseded, which is correct for a control that is merely
    /// idle between clicks; this entry point is for teardown, where keeping state for an id that
    /// no longer exists would be a leak in everything but name.
    pub fn forget(&mut self, target: ObjectId) {
        if matches!(self.last, Some(last) if last.target == target) {
            self.last = None;
        }
    }

    /// Clears the record unconditionally.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "touch")]
    use crate::gesture::{set_tuning, GestureTuning};

    /// Runs `f` with the gesture tuning pinned to `timeout_ms`.
    ///
    /// `gesture` is gated behind `touch`, so a build without it has no runtime tuning to set:
    /// the event layer then uses `DEFAULT_DOUBLE_CLICK_TIMEOUT_MS`. Every case below uses that
    /// default's 400 ms, so the no-`touch` branch calls `f` directly rather than asserting a
    /// threshold the build does not have.
    fn with_timeout<T>(timeout_ms: u64, f: impl FnOnce() -> T) -> T {
        #[cfg(feature = "touch")]
        {
            let previous = set_tuning(Some(GestureTuning {
                double_tap_timeout_ms: timeout_ms,
                ..GestureTuning::default()
            }));
            let result = f();
            set_tuning(previous);
            result
        }
        #[cfg(not(feature = "touch"))]
        {
            let _ = timeout_ms;
            f()
        }
    }

    #[test]
    fn a_second_press_inside_the_gesture_interval_is_a_double_click() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            let p = Point::new(10, 10);
            assert!(!tracker.observe_press(1, p, 1, 0, 0));
            assert!(tracker.observe_press(1, p, 1, 0, 200));
        });
    }

    #[test]
    fn a_second_press_after_the_interval_is_a_new_single_click() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            let p = Point::new(10, 10);
            assert!(!tracker.observe_press(1, p, 1, 0, 0));
            assert!(!tracker.observe_press(1, p, 1, 0, 401));
            // The late press re-arms the run, so a prompt follow-up is a double-click.
            assert!(tracker.observe_press(1, p, 1, 0, 500));
        });
    }

    #[test]
    fn a_distant_second_press_is_not_a_double_click() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            assert!(!tracker.observe_press(1, Point::new(0, 0), 1, 0, 0));
            assert!(!tracker.observe_press(1, Point::new(500, 500), 1, 0, 100));
        });
    }

    /// W-03: the two-tap separation bound is one named factor, and it is *not* the single-tap
    /// bound the tap recogniser uses — the ratio is documented on the constant.
    #[test]
    fn the_double_tap_separation_is_the_documented_multiple_of_the_stationary_bound() {
        const BOUND: f32 = 10.0;
        // Just under twice the bound is a pair; just over it is not.
        assert!(is_double_click_pair(
            (Point::new(0, 0), 0),
            (Point::new((BOUND * DOUBLE_TAP_SEPARATION_FACTOR - 0.5) as i32, 0), 100),
            400,
            BOUND
        ));
        assert!(!is_double_click_pair(
            (Point::new(0, 0), 0),
            (Point::new((BOUND * DOUBLE_TAP_SEPARATION_FACTOR + 0.5) as i32, 0), 100),
            400,
            BOUND
        ));
        // A single tap's own travel (`TapGesture`) uses the bound itself, so the pair bound is
        // wider by exactly the factor: a distance between the two is *not* a pair under the tap
        // bound but is under the pair bound. Checked at compile time so the assumption cannot
        // drift with the constants.
        const { assert!(BOUND < BOUND * DOUBLE_TAP_SEPARATION_FACTOR) };
        assert!(is_double_click_pair(
            (Point::new(0, 0), 0),
            (Point::new((BOUND + 1.0) as i32, 0), 100),
            400,
            BOUND
        ));
    }

    #[test]
    fn a_press_on_another_target_restarts_the_run() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            let p = Point::new(10, 10);
            assert!(!tracker.observe_press(1, p, 1, 0, 0));
            assert!(!tracker.observe_press(2, p, 1, 0, 100));
            assert!(!tracker.observe_press(1, p, 1, 0, 200));
        });
    }

    #[test]
    fn a_non_primary_button_never_forms_a_double_click() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            let p = Point::new(10, 10);
            assert!(!tracker.observe_press(1, p, 2, 0, 0));
            assert!(!tracker.observe_press(1, p, 2, 0, 100));
        });
    }

    #[test]
    fn a_platform_double_click_inside_the_interval_is_accepted() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            let p = Point::new(10, 10);
            assert!(!tracker.observe_press(1, p, 1, 0, 0));
            assert!(tracker.accept_platform_double_click(1, p, 1, 200));
        });
    }

    #[test]
    fn a_platform_double_click_outside_the_interval_is_rejected() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            let p = Point::new(10, 10);
            assert!(!tracker.observe_press(1, p, 1, 0, 0));
            assert!(!tracker.accept_platform_double_click(1, p, 1, 900));
        });
    }

    #[test]
    fn an_uncontradicted_platform_double_click_is_accepted() {
        let mut tracker = DoubleClickTracker::new();
        assert!(tracker.accept_platform_double_click(1, Point::new(0, 0), 1, 0));
    }

    #[test]
    fn a_third_click_starts_a_fresh_run() {
        with_timeout(400, || {
            let mut tracker = DoubleClickTracker::new();
            let p = Point::new(10, 10);
            assert!(!tracker.observe_press(1, p, 1, 0, 0));
            assert!(tracker.observe_press(1, p, 1, 0, 100));
            assert!(!tracker.observe_press(1, p, 1, 0, 200));
        });
    }
}
