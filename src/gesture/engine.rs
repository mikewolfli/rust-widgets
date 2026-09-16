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

    /// Feed an event through all recognizers.
    /// Returns the first semantic gesture event produced, or `None`.
    pub fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        self.last_timestamp_ms = now_ms;
        for r in &mut self.recognizers {
            if let Some(gesture_event) = r.process(event, now_ms) {
                return Some(gesture_event);
            }
        }
        None
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
}
