//! Gesture recognizer system (BLUE8 P4-2).
//!
//! This module provides gesture recognition primitives that transform
//! raw touch events (`TouchBegin`/`TouchEnd`/`TouchMove`) into
//! semantic gesture events (`Tap`, `DoubleTap`, `LongPress`, `Swipe`,
//! `Pinch`, `Rotate`).
//!
//! # Architecture
//!
//! - `GestureRecognizer` trait: processes raw events, emits semantic events
//! - `GestureEngine`: holds a chain of recognizers, dispatches events
//! - Concrete recognizers:
//!   - `TapGesture` — quick touch-and-release
//!   - `DoubleTapGesture` — two taps within 400ms
//!   - `LongPressGesture` — hold ≥500ms
//!   - `SwipeGesture` — rapid directional motion
//!   - `PanGesture` — continuous drag tracking
//!   - `LongPressDragGesture` — long press then drag
//!   - `FlingGesture` — velocity-based fling/flick
//!   - `TwoFingerTapGesture` — two-finger tap (≈ right-click)
//!   - `TwoFingerSwipeGesture` — two-finger directional swipe
//!   - `PinchGesture` — two-finger distance change
//!   - `RotateGesture` — two-finger angle change

use crate::core::Point;
use crate::event::Event;

// ── Sub-modules for concrete gesture recognizers ──

mod pinch;
mod press;
mod rotate;
mod swipe;
mod tap;

// ── Re-exports ──

pub use pinch::{PinchGesture, PinchTouch};
pub use press::{LongPressDragGesture, LongPressGesture, PanGesture};
pub use rotate::RotateGesture;
pub use swipe::{FlingGesture, SwipeGesture, TwoFingerSwipeGesture};
pub use tap::{DoubleTapGesture, TapGesture, TwoFingerTapGesture};

// ────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────

/// Maximum time delta (ms) between two taps to register as a double-tap.
const DOUBLE_TAP_TIMEOUT_MS: u64 = 400;
/// Minimum hold duration (ms) for long-press detection.
const LONG_PRESS_MIN_MS: u64 = 500;
/// Minimum swipe velocity (px/ms) to activate swipe gesture.
const SWIPE_MIN_VELOCITY: f32 = 0.5;
/// Maximum finger movement (px) to still consider the touch "stationary".
const MAX_STATIONARY_DISTANCE: f32 = 15.0;
/// Maximum finger movement (px) during a long-press hold.
const LONG_PRESS_MAX_MOVE: f32 = 10.0;
/// Minimum distance (px) for a swipe to be recognised.
const SWIPE_MIN_DISTANCE: f32 = 30.0;
/// Maximum time delta (ms) between touch-down and release for a single tap.
const TAP_TIMEOUT_MS: u64 = 300;

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
            Box::new(SwipeGesture::new()),
            Box::new(PanGesture::new()),
            Box::new(LongPressDragGesture::new()),
            Box::new(FlingGesture::new()),
            Box::new(TwoFingerTapGesture::new()),
            Box::new(TwoFingerSwipeGesture::new()),
            Box::new(PinchGesture::new()),
            Box::new(RotateGesture::new()),
        ];
        Self { recognizers, last_timestamp_ms: 0 }
    }

    /// Create an engine with a custom recognizer list.
    pub fn with_recognizers(recognizers: Vec<Box<dyn GestureRecognizer>>) -> Self {
        Self { recognizers, last_timestamp_ms: 0 }
    }

    /// Feed an event through the recognizer chain.
    ///
    /// If a recognizer produces a semantic gesture event, that event is
    /// returned; otherwise `None`.
    pub fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        self.last_timestamp_ms = now_ms;
        for rec in self.recognizers.iter_mut() {
            if let Some(gesture) = rec.process(event, now_ms) {
                return Some(gesture);
            }
        }
        None
    }

    /// Reset all recognizers to their idle state.
    pub fn reset_all(&mut self) {
        for rec in self.recognizers.iter_mut() {
            rec.reset();
        }
    }

    /// Returns the last timestamp provided to `process()`.
    pub fn last_timestamp(&self) -> u64 {
        self.last_timestamp_ms
    }
}

impl Default for GestureEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────
// Helper: Euclidean distance between two Points
// ────────────────────────────────────────────

/// Compute the Euclidean distance between two points.
pub(crate) fn distance(a: Point, b: Point) -> f32 {
    let dx = (a.x - b.x) as f32;
    let dy = (a.y - b.y) as f32;
    (dx * dx + dy * dy).sqrt()
}

// ────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── TapGesture tests ──

    #[test]
    fn tap_recognises_quick_touch_end() {
        let mut rec = TapGesture::new();
        let t0 = 1000;
        let t1 = 1100; // 100 ms later

        assert!(rec.process(&Event::touch_begin(50, 50, 0), t0).is_none());
        let result = rec.process(&Event::touch_end(51, 51, 0), t1);
        assert!(matches!(result, Some(Event::Tap { pos }) if pos.x == 51 && pos.y == 51));
    }

    #[test]
    fn tap_ignores_long_hold() {
        let mut rec = TapGesture::new();
        rec.process(&Event::touch_begin(50, 50, 0), 0);
        // End after 400ms → exceeds 300ms tap threshold
        let result = rec.process(&Event::touch_end(50, 50, 0), 400);
        assert!(result.is_none());
    }

    #[test]
    fn tap_cancelled_by_excessive_movement() {
        let mut rec = TapGesture::new();
        rec.process(&Event::touch_begin(50, 50, 0), 0);
        // Move finger far away
        rec.process(&Event::touch_move(100, 100, 0), 100);
        let result = rec.process(&Event::touch_end(100, 100, 0), 150);
        assert!(result.is_none());
    }

    #[test]
    fn tap_requires_matching_touch_id() {
        let mut rec = TapGesture::new();
        rec.process(&Event::touch_begin(10, 10, 5), 0);
        // Different touch ID ends → ignored
        let result = rec.process(&Event::touch_end(10, 10, 7), 100);
        assert!(result.is_none());
    }

    // ── DoubleTapGesture tests ──

    #[test]
    fn double_tap_recognises_two_quick_taps() {
        let mut rec = DoubleTapGesture::new();
        let t0 = 1000;
        // First tap
        rec.process(&Event::tap(50, 50), t0);
        // Second tap within 400ms
        let result = rec.process(&Event::tap(52, 52), t0 + 200);
        assert!(matches!(result, Some(Event::DoubleTap { .. })));
    }

    #[test]
    fn double_tap_too_slow_second_tap() {
        let mut rec = DoubleTapGesture::new();
        rec.process(&Event::tap(50, 50), 0);
        // Second tap after 500ms → too slow
        let result = rec.process(&Event::tap(55, 55), 500);
        assert!(result.is_none());
        // But a third tap within the window should still work
        let result2 = rec.process(&Event::tap(55, 55), 700); // 200ms after second
        assert!(matches!(result2, Some(Event::DoubleTap { .. })));
    }

    // ── LongPressGesture tests ──

    #[test]
    fn long_press_fires_after_threshold() {
        let mut rec = LongPressGesture::new();
        rec.process(&Event::touch_begin(30, 30, 0), 0);
        // Simulate a timer tick after 500ms
        let result = rec.process(&Event::Timer { id: 0 }, 500);
        assert!(matches!(result, Some(Event::LongPress { pos }) if pos.x == 30 && pos.y == 30));
    }

    #[test]
    fn long_press_does_not_fire_before_threshold() {
        let mut rec = LongPressGesture::new();
        rec.process(&Event::touch_begin(30, 30, 0), 0);
        // Timer tick at 300ms → too early
        let result = rec.process(&Event::Timer { id: 0 }, 300);
        assert!(result.is_none());
    }

    #[test]
    fn long_press_cancelled_on_early_release() {
        let mut rec = LongPressGesture::new();
        rec.process(&Event::touch_begin(30, 30, 0), 0);
        rec.process(&Event::touch_end(30, 30, 0), 100);
        let result = rec.process(&Event::Timer { id: 0 }, 500);
        assert!(result.is_none());
    }

    #[test]
    fn long_press_cancelled_by_excessive_move() {
        let mut rec = LongPressGesture::new();
        rec.process(&Event::touch_begin(30, 30, 0), 0);
        rec.process(&Event::touch_move(50, 50, 0), 100); // >10px
        let result = rec.process(&Event::Timer { id: 0 }, 500);
        assert!(result.is_none());
    }

    // ── SwipeGesture tests ──

    #[test]
    fn swipe_recognises_rapid_motion() {
        let mut rec = SwipeGesture::new();
        rec.process(&Event::touch_begin(0, 0, 0), 0);
        rec.process(&Event::touch_move(20, 0, 0), 20);
        rec.process(&Event::touch_move(40, 0, 0), 40);
        let result = rec.process(&Event::touch_end(60, 0, 0), 60);
        // 60px in 60ms = 1.0 px/ms > 0.5 threshold
        assert!(matches!(result, Some(Event::Swipe { .. })));
    }

    #[test]
    fn swipe_requires_minimum_distance() {
        let mut rec = SwipeGesture::new();
        rec.process(&Event::touch_begin(0, 0, 0), 0);
        // Only 10px move → less than 30px minimum
        let result = rec.process(&Event::touch_end(10, 0, 0), 50);
        assert!(result.is_none());
    }

    #[test]
    fn swipe_requires_minimum_velocity() {
        let mut rec = SwipeGesture::new();
        rec.process(&Event::touch_begin(0, 0, 0), 0);
        // 40px in 500ms = 0.08 px/ms < 0.5 threshold
        let result = rec.process(&Event::touch_end(40, 0, 0), 500);
        assert!(result.is_none());
    }

    // ── PinchGesture tests ──

    #[test]
    fn pinch_recognises_two_finger_zoom() {
        let mut rec = PinchGesture::new();
        rec.process(&Event::touch_begin(0, 100, 0), 0);
        rec.process(&Event::touch_begin(100, 100, 1), 0);
        // Move fingers apart
        let result = rec.process(&Event::touch_move(0, 100, 0), 10);
        assert!(result.is_none()); // baseline not yet set
        let result = rec.process(&Event::touch_move(200, 100, 1), 10);
        // 200px / 100px = 2.0 scale
        assert!(matches!(result, Some(Event::Pinch { scale }) if (scale - 2.0).abs() < 0.001));
    }

    #[test]
    fn pinch_resets_when_touch_lost() {
        let mut rec = PinchGesture::new();
        rec.process(&Event::touch_begin(0, 0, 0), 0);
        rec.process(&Event::touch_begin(100, 0, 1), 0);
        rec.process(&Event::touch_end(100, 0, 1), 50);
        // Only 1 touch left → no pinch
        let result = rec.process(&Event::touch_move(10, 0, 0), 60);
        assert!(result.is_none());
    }

    // ── RotateGesture tests ──

    #[test]
    fn rotate_recognises_two_finger_turn() {
        let mut rec = RotateGesture::new();
        // Fingers horizontally aligned: (0,100) and (100,100) → angle = π
        rec.process(&Event::touch_begin(0, 100, 0), 0);
        rec.process(&Event::touch_begin(100, 100, 1), 0);

        // Move both fingers significantly: finger0 to (0,0), finger1 to (100,0)
        // angle changes from π to ~0 (atan2(-100, 100) = -π/4 which is
        // different from π by >0.05 rad)
        rec.process(&Event::touch_move(0, 0, 0), 10);
        // Now both at (0,0) and (100,0) → new baseline established
        rec.process(&Event::touch_move(100, 0, 1), 10);

        // Rotate: finger0 down to (50,50), finger1 stays at (100,0)
        // Angle = atan2(50, 50) = π/4 ≈ 0.785 → >0.05 from baseline 0
        let result = rec.process(&Event::touch_move(50, 50, 0), 20);
        assert!(matches!(result, Some(Event::Rotate { .. })));
    }

    // ── GestureEngine tests ──

    #[test]
    fn engine_processes_chain() {
        let mut engine = GestureEngine::new();
        // Tap event should be recognised through the chain
        let r1 = engine.process(&Event::touch_begin(10, 10, 0), 0);
        assert!(r1.is_none());
        let r2 = engine.process(&Event::touch_end(10, 10, 0), 100);
        assert!(matches!(r2, Some(Event::Tap { .. })));
    }

    #[test]
    fn engine_reset_clears_all() {
        let mut engine = GestureEngine::new();
        engine.process(&Event::touch_begin(10, 10, 0), 0);
        engine.reset_all();
        // After reset, tap should start fresh
        let r1 = engine.process(&Event::touch_begin(20, 20, 1), 200);
        assert!(r1.is_none());
        let r2 = engine.process(&Event::touch_end(20, 20, 1), 250);
        assert!(matches!(r2, Some(Event::Tap { .. })));
    }

    #[test]
    fn engine_default_is_new() {
        let engine = GestureEngine::default();
        assert_eq!(engine.last_timestamp(), 0);
    }
}
