// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Gesture recognizer system — engine, trait, and constants.

use crate::core::Point;
use crate::event::Event;

use super::{
    DoubleTapGesture, FlingGesture, LongPressDragGesture, LongPressGesture, PanGesture,
    PinchGesture, RotateGesture, SwipeGesture, TapGesture, TwoFingerSwipeGesture,
    TwoFingerTapGesture,
};

// ────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────

/// Maximum time delta (ms) between two taps to register as a double-tap.
pub(crate) const DOUBLE_TAP_TIMEOUT_MS: u64 = 400;
/// Minimum hold duration (ms) for long-press detection.
pub(crate) const LONG_PRESS_MIN_MS: u64 = 500;
/// Minimum swipe velocity to activate a swipe gesture.
///
/// **The unit everywhere is logical pixels per second**, matching the public
/// contract documented on `Event::Swipe` / `Event::TwoFingerSwipe` / `Event::Fling`.
/// Timestamps arrive in milliseconds, so each recogniser converts its px/ms ratio
/// by 1000. That conversion is the one thing to keep in step: it was previously
/// applied by `FlingGesture` only, leaving the two swipe recognisers reporting
/// px/ms for a field documented as px/s.
pub(crate) const SWIPE_MIN_VELOCITY: f32 = 500.0;
/// Maximum finger movement (px) to still consider the touch "stationary".
pub(crate) const MAX_STATIONARY_DISTANCE: f32 = 15.0;
/// Maximum finger movement (px) during a long-press hold.
pub(crate) const LONG_PRESS_MAX_MOVE: f32 = 10.0;
/// Minimum distance (px) for a swipe to be recognised.
pub(crate) const SWIPE_MIN_DISTANCE: f32 = 30.0;
/// Minimum total travel (px) before `PanGesture` reports a drag.
///
/// Deliberately smaller than [`SWIPE_MIN_DISTANCE`]: a pan is the continuous,
/// lower-energy gesture and must start responding well before a swipe would fire,
/// while still being large enough that hand tremor during a tap cannot trigger it.
/// `MAX_STATIONARY_DISTANCE` (15 px) is the tap recogniser's own bound and sits
/// between the two, which is what keeps the three recognisers from claiming each
/// other's gestures.
pub(crate) const PAN_MIN_DISTANCE: f32 = 8.0;
/// Maximum time delta (ms) between touch-down and release for a single tap.
pub(crate) const TAP_TIMEOUT_MS: u64 = 300;

// ────────────────────────────────────────────
// GestureRecognizer trait
// ────────────────────────────────────────────

/// A single gesture recognizer that processes raw events and optionally
/// produces a semantic gesture event.
pub trait GestureRecognizer: std::fmt::Debug + Send {
    /// Feed a raw event into the recognizer.
    ///
    /// Returns `Some(Event)` if the recognizer has completed a gesture,
    /// or `None` if it is still collecting data.
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;

    /// Reset the recognizer to its initial idle state.
    fn reset(&mut self);
}

// ────────────────────────────────────────────
// GestureEngine — chain of recognizers
// ────────────────────────────────────────────

/// An ordered chain of gesture recognisers.
///
/// Events are fed to every recognizer in order. The first recognizer
/// that produces a semantic event wins (subsequent recognizers are
/// skipped for that round).
#[derive(Debug)]
pub struct GestureEngine {
    recognizers: Vec<Box<dyn GestureRecognizer>>,
    last_timestamp_ms: u64,
}

impl GestureEngine {
    /// Create an engine pre-populated with the standard recognizers.
    pub fn new() -> Self {
        let recognizers: Vec<Box<dyn GestureRecognizer>> = vec![
            Box::new(TapGesture::new()),
            Box::new(DoubleTapGesture::new()),
            Box::new(LongPressGesture::new()),
            Box::new(LongPressDragGesture::new()),
            Box::new(PanGesture::new()),
            Box::new(SwipeGesture::new()),
            Box::new(FlingGesture::new()),
            Box::new(TwoFingerTapGesture::new()),
            Box::new(TwoFingerSwipeGesture::new()),
            Box::new(PinchGesture::new()),
            Box::new(RotateGesture::new()),
        ];
        Self { recognizers, last_timestamp_ms: 0 }
    }

    /// Feeds an event through all recognizers and returns the most specific semantic
    /// gesture event produced, or `None`.
    ///
    /// # Why every recognizer gets a chance
    ///
    /// Returning on the first hit made several recognizers unreachable, because a
    /// recognizer's *input* is often another recognizer's *output*: `DoubleTapGesture`
    /// consumes `Event::Tap`, which `TapGesture` produces, and `TapGesture` sits
    /// earlier in the chain. Returning `Tap` immediately meant `DoubleTapGesture` was
    /// never asked, so `Event::DoubleTap` could not be produced at all.
    ///
    /// # Why "first hit wins" is the wrong rule
    ///
    /// Running every recognizer is necessary but not sufficient: the winner must be
    /// chosen by **specificity**, not by position. `TapGesture` is by construction the
    /// easiest recognizer to satisfy — every completed touch that did not move is a tap
    /// — so it fires on the second tap of a double-tap as well. If position decided the
    /// winner, the richer `DoubleTap` that a later recognizer computed would be
    /// discarded in favour of the coarser earlier `Tap`, and the recognizer chain would
    /// still be effectively unreachable even though every recognizer had run.
    ///
    /// See `GestureEngine::is_more_specific` for the ordering. It is a total order on
    /// the gesture events that can be produced within one call, so the result does not
    /// depend on recognizer registration order.
    ///
    /// # How the derived event reaches its consumers
    ///
    /// A recognizer is offered the raw event first, then (if the raw event produced
    /// nothing) the most recent semantic event. `TapGesture` consumes the raw `TouchEnd`
    /// and yields `Tap`; `DoubleTapGesture` is then handed that `Tap` and can arm itself
    /// or complete a double-tap. The derived event is never offered to recognizers
    /// *ahead* of its producer — feeding backwards would let `TapGesture` re-consume the
    /// `Tap` it just emitted. Each recognizer sees at most two events per call, so this
    /// cannot loop.
    pub fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        self.last_timestamp_ms = now_ms;
        let mut produced: Option<Event> = None;
        let mut derived: Option<Event> = None;
        for r in &mut self.recognizers {
            // The raw event first: recognizers that track the touch sequence directly
            // (tap, press, swipe, pinch, rotate) read this one.
            let mut hit = r.process(event, now_ms);
            // Then the most recent semantic event, for recognizers that refine another
            // recognizer's output rather than the raw stream.
            if hit.is_none() {
                if let Some(ref derived_event) = derived {
                    hit = r.process(derived_event, now_ms);
                }
            }
            if let Some(gesture_event) = hit {
                // Most specific wins, not first: see the note above.
                if produced.as_ref().is_none_or(|best| Self::is_more_specific(&gesture_event, best))
                {
                    produced = Some(gesture_event.clone());
                }
                derived = Some(gesture_event);
            }
        }
        produced
    }

    /// Returns whether `candidate` describes the user's gesture more precisely than
    /// `incumbent`.
    ///
    /// The order, least to most specific:
    ///
    /// 1. `Drag` — the continuous fallback; it fires for any sustained movement, so it
    ///    is the weakest claim on an event.
    /// 2. `Tap` / `LongPress` — a completed discrete gesture, but one that a more
    ///    deliberate gesture can be built on top of.
    /// 3. `Swipe` / `Pinch` / `Rotate` — directional or multi-touch gestures that
    ///    required more from the user than a tap did.
    /// 4. `Fling` — a swipe that also met the higher *velocity* bar, so it is a strictly
    ///    more specific claim on the same motion than `Swipe`. Ordering them equally
    ///    meant a hard flick was reported only as a `Swipe`, because `SwipeGesture` sits
    ///    earlier in the chain and therefore held the incumbent spot.
    /// 5. `DoubleTap` / `TwoFingerTap` / `TwoFingerSwipe` — recognitions that *include*
    ///    another gesture as a precondition, so they are strictly more informed than the
    ///    gesture they subsume.
    ///
    /// Two events of the same rank are equally specific and the incumbent is kept, which
    /// makes the outcome stable regardless of registration order.
    fn is_more_specific(candidate: &Event, incumbent: &Event) -> bool {
        Self::gesture_rank(candidate) > Self::gesture_rank(incumbent)
    }

    /// The specificity rank used by `GestureEngine::is_more_specific`.
    fn gesture_rank(event: &Event) -> u8 {
        match event {
            Event::Drag { .. } => 0,
            Event::Tap { .. } | Event::LongPress { .. } => 1,
            Event::Swipe { .. } | Event::Pinch { .. } | Event::Rotate { .. } => 2,
            Event::Fling { .. } => 3,
            Event::DoubleTap { .. } | Event::TwoFingerTap { .. } | Event::TwoFingerSwipe { .. } => {
                4
            }
            // Anything else the engine may forward (raw touch/mouse events) never
            // outranks a recognised gesture.
            _ => 0,
        }
    }

    /// Reset all recognizers (e.g., when a touch sequence is cancelled).
    pub fn reset_all(&mut self) {
        for r in &mut self.recognizers {
            r.reset();
        }
    }
}

crate::impl_default_via_new!(GestureEngine);

// ────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────

/// Euclidean distance between two points.
pub(crate) fn distance(a: Point, b: Point) -> f32 {
    let dx = (a.x - b.x) as f32;
    let dy = (a.y - b.y) as f32;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;

    #[test]
    fn gesture_engine_new() {
        let engine = GestureEngine::new();
        assert_eq!(engine.recognizers.len(), 11);
    }

    #[test]
    fn gesture_engine_process_none_for_unrelated_event() {
        let mut engine = GestureEngine::new();
        let result =
            engine.process(&Event::MousePress { pos: crate::core::Point::new(0, 0), button: 0 }, 0);
        assert!(result.is_none());
    }

    #[test]
    fn gesture_engine_reset_all_clears_state() {
        let mut engine = GestureEngine::new();
        engine.reset_all();
        assert_eq!(engine.recognizers.len(), 11);
    }

    /// Every recogniser must report velocity in the **same** unit: px/s.
    ///
    /// This is the regression guard for a real defect, found while documenting:
    /// `Event::Swipe::velocity` / `TwoFingerSwipe::velocity` / `Fling::velocity` are
    /// all documented as *logical pixels per second*, but `SwipeGesture` and
    /// `TwoFingerSwipeGesture` emitted px/ms while `FlingGesture` emitted px/s — a
    /// 1000x disagreement between two recognisers populating the same field. No test
    /// exercised those paths, so nothing caught it.
    ///
    /// Both recognisers are fed the same physical motion (100 px in 100 ms = 1000
    /// px/s) and must agree.
    #[test]
    fn every_recogniser_reports_velocity_in_pixels_per_second() {
        const START: i32 = 0;
        const END: i32 = 100; // 100 px
        const ELAPSED_MS: u64 = 100; // over 100 ms -> 1000 px/s
        const TOUCH: u64 = 1;
        // Generous band: the point is to catch a wrong *scale*, not to pin rounding.
        const EXPECTED: std::ops::RangeInclusive<f32> = 900.0..=1100.0;

        // Single-finger swipe.
        let mut swipe = SwipeGesture::new();
        assert!(swipe
            .process(&Event::TouchBegin { pos: Point::new(START, START), touch_id: TOUCH }, 0)
            .is_none());
        let swipe_event = swipe
            .process(&Event::TouchEnd { pos: Point::new(END, START), touch_id: TOUCH }, ELAPSED_MS);
        let swipe_velocity = match swipe_event {
            Some(Event::Swipe { velocity, .. }) => velocity,
            other => panic!("expected Event::Swipe over 100px in 100ms, got {other:?}"),
        };

        // Fling over the same physical motion.
        let mut fling = FlingGesture::new();
        assert!(fling
            .process(&Event::TouchBegin { pos: Point::new(START, START), touch_id: TOUCH }, 0)
            .is_none());
        for step in 1..=4u64 {
            fling.process(
                &Event::TouchMove {
                    pos: Point::new(START + (step as i32) * (END - START) / 4, START),
                    touch_id: TOUCH,
                },
                ELAPSED_MS / 4 * step,
            );
        }
        let fling_event = fling
            .process(&Event::TouchEnd { pos: Point::new(END, START), touch_id: TOUCH }, ELAPSED_MS);
        let fling_velocity = match fling_event {
            Some(Event::Fling { velocity, .. }) => velocity,
            other => panic!("expected Event::Fling over 100px in 100ms, got {other:?}"),
        };

        // 100 px in 100 ms is 1000 px/s. A value near 1 means the px/ms ratio was
        // emitted unscaled — the defect this test exists for.
        for (name, velocity) in [("swipe", swipe_velocity), ("fling", fling_velocity.x as f32)] {
            assert!(
                EXPECTED.contains(&velocity),
                "{name} velocity must be ~1000 px/s for 100px in 100ms, got {velocity}. \
                 A value near 1 means px/ms leaked through, which contradicts the \
                 unit documented on Event::Swipe / Event::Fling"
            );
        }
    }

    /// A double tap must reach the consumer as `DoubleTap`, not as two `Tap`s.
    ///
    /// Regression guard for a real defect: `DoubleTapGesture` consumes `Event::Tap`,
    /// which `TapGesture` produces and which sits **earlier** in the chain, so the
    /// engine's "first hit wins, then return" rule meant the double-tap recogniser was
    /// never consulted on the second tap. `Event::DoubleTap` was therefore impossible
    /// to observe through the engine even though the recogniser itself worked, and its
    /// unit tests passed because they drove it directly.
    ///
    /// The second tap must also still be reported as *something* on the first tap of a
    /// pair: a lone tap is a `Tap`.
    #[test]
    fn second_tap_is_reported_as_double_tap() {
        let mut engine = GestureEngine::new();
        let p = Point::new(10, 10);
        // Two taps inside `DOUBLE_TAP_TIMEOUT_MS` (400 ms), same spot.
        assert!(engine.process(&Event::TouchBegin { pos: p, touch_id: 1 }, 0).is_none());
        let first = engine.process(&Event::TouchEnd { pos: p, touch_id: 1 }, 50);
        assert!(
            matches!(first, Some(Event::Tap { .. })),
            "a lone tap must be reported as Tap, got {first:?}"
        );
        assert!(engine.process(&Event::TouchBegin { pos: p, touch_id: 1 }, 100).is_none());
        let second = engine.process(&Event::TouchEnd { pos: p, touch_id: 1 }, 150);
        assert!(
            matches!(second, Some(Event::DoubleTap { .. })),
            "the second tap must upgrade to DoubleTap, got {second:?}"
        );
    }

    /// A single finger must never be reported as a two-finger gesture.
    ///
    /// Regression guard: `TwoFingerTapGesture` accepted a one-finger tap because it
    /// tested `touches.is_empty()` — which is also true once the *only* finger lifts —
    /// and never counted the fingers it had seen. Every ordinary tap was therefore also
    /// reported as `TwoFingerTap`.
    #[test]
    fn one_finger_tap_is_never_two_finger_tap() {
        let mut engine = GestureEngine::new();
        let p = Point::new(10, 10);
        engine.process(&Event::TouchBegin { pos: p, touch_id: 1 }, 0);
        let released = engine.process(&Event::TouchEnd { pos: p, touch_id: 1 }, 50);
        assert!(
            !matches!(released, Some(Event::TwoFingerTap { .. })),
            "one finger cannot make a two-finger tap, got {released:?}"
        );
    }

    /// Two fingers landing and lifting together *are* a two-finger tap.
    ///
    /// The positive half of the pair above: narrowing the recogniser to reject one
    /// finger must not have made it reject two as well.
    #[test]
    fn two_fingers_together_are_two_finger_tap() {
        let mut engine = GestureEngine::new();
        engine.process(&Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 }, 0);
        engine.process(&Event::TouchBegin { pos: Point::new(30, 10), touch_id: 2 }, 10);
        engine.process(&Event::TouchEnd { pos: Point::new(10, 10), touch_id: 1 }, 40);
        let released =
            engine.process(&Event::TouchEnd { pos: Point::new(30, 10), touch_id: 2 }, 50);
        assert!(
            matches!(released, Some(Event::TwoFingerTap { .. })),
            "two fingers down and up together must be a TwoFingerTap, got {released:?}"
        );
    }

    /// `PanGesture` must not report a drag until the finger has actually travelled.
    ///
    /// Regression guard: `PanGesture` emitted `Drag` on the *first* `TouchMove` with no
    /// displacement threshold. `ScrollArea` scrolls by `Drag::delta`, so a finger that
    /// merely landed and jittered one pixel scrolled the content — the drag and the tap
    /// were indistinguishable to every consumer.
    #[test]
    fn pan_waits_for_the_distance_threshold() {
        let mut engine = GestureEngine::new();
        engine.process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0);

        // Below the threshold: jitter, not a drag.
        let jitter = engine.process(&Event::TouchMove { pos: Point::new(3, 0), touch_id: 1 }, 10);
        assert!(jitter.is_none(), "3 px is jitter, so no Drag yet, got {jitter:?}");

        // Past the threshold: a real drag, and it must be reported.
        let drag = engine.process(&Event::TouchMove { pos: Point::new(50, 0), touch_id: 1 }, 20);
        assert!(
            matches!(drag, Some(Event::Drag { .. })),
            "50 px of travel is a drag, got {drag:?}"
        );
    }

    /// Every emitted rotation must be the shortest signed turn, never a full twist.
    ///
    /// Regression guard for a wrap-around defect (principle #76): `angle_between`
    /// returns `atan2`, whose range is `(-pi, pi]`, and the recogniser subtracted two
    /// such angles directly. Crossing the boundary made a real rotation of ~0.05 rad
    /// report a delta near `-2*pi`, i.e. a full turn the user never made.
    #[test]
    fn rotation_never_reports_a_full_turn_for_a_tiny_change() {
        let mut engine = GestureEngine::new();
        // Place the pair so its angle sits just under +pi, then nudge one finger across
        // the boundary. The true change is a fraction of a radian.
        let a = Point::new(1000, 26); // atan2(26, 1000)  ~ +0.026
        let b = Point::new(-1000, -26); // atan2(-26, -1000) ~ -3.116
        engine.process(&Event::TouchBegin { pos: a, touch_id: 1 }, 0);
        engine.process(&Event::TouchBegin { pos: b, touch_id: 2 }, 0);
        let b2 = Point::new(-1000, 26); // atan2(26, -1000) ~ +3.116
        let produced = engine.process(&Event::TouchMove { pos: b2, touch_id: 2 }, 10);

        if let Some(Event::Rotate { angle }) = produced {
            assert!(
                angle.abs() <= core::f32::consts::PI,
                "a rotation must be the shortest signed turn (|angle| <= pi), got {angle}. \
                 A value near 2*pi means the atan2 wrap-around was not normalised"
            );
        }
    }

    // ────────────────────────────────────────
    // Behaviour tests for the recognisers that had none (BLUE16 B-5d).
    //
    // `src/gesture/` previously held four tests, all in `engine.rs`, and they covered
    // the engine's bookkeeping rather than any recogniser's behaviour. Each test below
    // asserts an observable outcome — the event produced and its payload — not merely
    // that a call returned.
    // ────────────────────────────────────────

    /// A hold longer than `LONG_PRESS_MIN_MS` must produce `LongPress` at the press point.
    #[test]
    fn long_hold_produces_long_press() {
        let mut engine = GestureEngine::new();
        let p = Point::new(40, 60);
        assert!(engine.process(&Event::TouchBegin { pos: p, touch_id: 1 }, 0).is_none());

        // The recogniser fires on the first event *after* the timeout, so a timer-like
        // tick drives it — the same way the event loop does.
        let produced = engine.process(&Event::Timer { id: 0 }, LONG_PRESS_MIN_MS + 1);
        match produced {
            Some(Event::LongPress { pos }) => assert_eq!(pos, p),
            other => panic!("a {LONG_PRESS_MIN_MS}ms hold must be a LongPress, got {other:?}"),
        }
    }

    /// A quick, fast flick must produce `Fling` with a px/s velocity on the moved axis.
    #[test]
    fn fast_flick_produces_fling() {
        let mut engine = GestureEngine::new();
        let start = Point::new(0, 0);
        let end = Point::new(200, 0);
        engine.process(&Event::TouchBegin { pos: start, touch_id: 1 }, 0);
        // 200 px in 50 ms = 4000 px/s.
        engine.process(&Event::TouchMove { pos: Point::new(100, 0), touch_id: 1 }, 25);
        let produced = engine.process(&Event::TouchEnd { pos: end, touch_id: 1 }, 50);

        match produced {
            Some(Event::Fling { velocity, .. }) => assert!(
                velocity.x > 0,
                "a flick to the right must have positive x velocity, got {velocity:?}"
            ),
            other => panic!("a 200px flick in 50ms must produce Fling, got {other:?}"),
        }
    }

    /// Moving two fingers apart must produce `Pinch` with a scale above 1.
    ///
    /// This is the multi-touch path, which needs two independent touches to be tracked
    /// at once — the case the single-pointer capture manager could not serve.
    #[test]
    fn fingers_moving_apart_produce_pinch_out() {
        let mut engine = GestureEngine::new();
        engine.process(&Event::TouchBegin { pos: Point::new(100, 100), touch_id: 1 }, 0);
        engine.process(&Event::TouchBegin { pos: Point::new(200, 100), touch_id: 2 }, 0);
        // Double the separation: 100 px -> 200 px.
        let produced =
            engine.process(&Event::TouchMove { pos: Point::new(300, 100), touch_id: 2 }, 10);

        match produced {
            Some(Event::Pinch { scale }) => {
                assert!(scale > 1.0, "spreading the fingers must zoom in (scale > 1), got {scale}")
            }
            other => panic!("two fingers moving apart must produce Pinch, got {other:?}"),
        }
    }

    /// Two fingers moving together must produce `TwoFingerSwipe` on release.
    #[test]
    fn two_fingers_moving_together_produce_two_finger_swipe() {
        let mut engine = GestureEngine::new();
        engine.process(&Event::TouchBegin { pos: Point::new(0, 100), touch_id: 1 }, 0);
        engine.process(&Event::TouchBegin { pos: Point::new(0, 120), touch_id: 2 }, 0);
        // Both fingers travel 150 px right in 50 ms = 3000 px/s.
        engine.process(&Event::TouchMove { pos: Point::new(150, 100), touch_id: 1 }, 25);
        engine.process(&Event::TouchMove { pos: Point::new(150, 120), touch_id: 2 }, 25);
        engine.process(&Event::TouchEnd { pos: Point::new(150, 100), touch_id: 1 }, 50);
        let produced =
            engine.process(&Event::TouchEnd { pos: Point::new(150, 120), touch_id: 2 }, 50);

        assert!(
            matches!(produced, Some(Event::TwoFingerSwipe { .. })),
            "two fingers swiping together must produce TwoFingerSwipe, got {produced:?}"
        );
    }

    /// Two independent contacts must reach `Pinch` — the B-5b acceptance test.
    ///
    /// # Why this is the test that matters for backend touch support
    ///
    /// Adding `WM_TOUCH` / `NSTouch` / GDK touch forwarding is only worth anything if
    /// two separately-identified fingers survive the whole path. Before it, the capture
    /// manager held a single pointer and no backend emitted `TouchBegin`, so
    /// `Pinch`/`Rotate` were unreachable no matter how correct the recognisers were.
    ///
    /// The two `touch_id`s here are what a backend now supplies (Win32 `dwID`, AppKit's
    /// `NSTouch.identity`, GDK's `GdkEventSequence`), so the recogniser sees the same
    /// shape of input the real backends produce.
    #[test]
    fn two_independent_contacts_reach_pinch() {
        let mut engine = GestureEngine::new();
        engine.process(&Event::TouchBegin { pos: Point::new(100, 100), touch_id: 1 }, 0);
        engine.process(&Event::TouchBegin { pos: Point::new(200, 100), touch_id: 2 }, 1);
        // Spread both fingers outward: separation grows from 100 px to 220 px.
        engine.process(&Event::TouchMove { pos: Point::new(40, 100), touch_id: 1 }, 2);
        let produced =
            engine.process(&Event::TouchMove { pos: Point::new(260, 100), touch_id: 2 }, 3);

        match produced {
            Some(Event::Pinch { scale }) => assert!(
                scale > 1.0,
                "spreading two identified fingers must zoom in, got scale {scale}"
            ),
            other => panic!(
                "two independent contacts must reach Pinch; got {other:?}. \
                 If this fails, a backend has stopped emitting TouchBegin/Move with \
                 distinct touch ids"
            ),
        }
    }

    /// Two independent contacts must reach `Rotate` with the quarter turn that happened.
    ///
    /// The companion to the pinch test: it is the other recogniser that needs two
    /// simultaneous fingers, and it is also the one whose angle had to be normalised.
    #[test]
    fn two_independent_contacts_reach_rotate() {
        let mut engine = GestureEngine::new();
        engine.process(&Event::TouchBegin { pos: Point::new(100, 100), touch_id: 1 }, 0);
        engine.process(&Event::TouchBegin { pos: Point::new(200, 100), touch_id: 2 }, 1);
        // Swing the second finger onto the first one's vertical: a quarter turn.
        let produced =
            engine.process(&Event::TouchMove { pos: Point::new(100, 200), touch_id: 2 }, 2);

        match produced {
            Some(Event::Rotate { angle }) => {
                let quarter_turn = core::f32::consts::FRAC_PI_2;
                assert!(
                    (angle - quarter_turn).abs() < 0.01,
                    "a quarter turn must report pi/2 (~1.5708), got {angle}"
                );
            }
            other => panic!("two independent contacts must reach Rotate, got {other:?}"),
        }
    }

    /// An ordinary rotation is reported with the sign the user's motion implies.
    ///
    /// The positive counterpart to the wrap-around test: narrowing the angle to the
    /// shortest turn must not have made ordinary rotations stop being reported, nor
    /// flipped their sign.
    #[test]
    fn ordinary_rotation_is_reported_with_the_correct_sign() {
        let mut engine = GestureEngine::new();
        // Two fingers on the x axis: the pair angle is 0.
        engine.process(&Event::TouchBegin { pos: Point::new(100, 100), touch_id: 1 }, 0);
        engine.process(&Event::TouchBegin { pos: Point::new(200, 100), touch_id: 2 }, 0);
        // Move the second finger to sit directly above the first. The pair angle runs
        // from 0 to atan2(dy, dx) with dy = -100, i.e. -pi/2: a quarter turn. The sign is
        // negative because screen y grows downward, so "up" is the negative direction.
        let produced =
            engine.process(&Event::TouchMove { pos: Point::new(100, 0), touch_id: 2 }, 10);

        match produced {
            Some(Event::Rotate { angle }) => {
                let quarter_turn = core::f32::consts::FRAC_PI_2;
                assert!(
                    (angle + quarter_turn).abs() < 0.01,
                    "a quarter turn to the screen-up direction must report -pi/2 (~-1.5708), \
                     got {angle}"
                );
            }
            other => panic!("a quarter-turn rotation must produce Rotate, got {other:?}"),
        }
    }
}
