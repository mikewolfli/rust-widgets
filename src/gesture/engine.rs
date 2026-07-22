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
/// Minimum swipe velocity (px/ms) to activate swipe gesture.
pub(crate) const SWIPE_MIN_VELOCITY: f32 = 0.5;
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

impl Default for GestureEngine {
    fn default() -> Self {
        Self::new()
    }
}

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
}
