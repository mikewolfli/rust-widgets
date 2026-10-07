// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Press-hold gesture recognizers: long press, pan, and long-press-drag.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{
    distance, point_delta, point_delta_abs, GestureRecognizer, LONG_PRESS_MAX_MOVE,
    LONG_PRESS_MIN_MS, PAN_MIN_DISTANCE,
};

// ────────────────────────────────────────────
// LongPressGesture
// ────────────────────────────────────────────

/// Recognises a stationary hold >= 500 ms.
#[derive(Debug, Clone)]
pub struct LongPressGesture {
    start_pos: Option<Point>,
    start_time: Option<u64>,
    touch_id: Option<TouchId>,
    fired: bool,
}

impl LongPressGesture {
    /// Creates a recognizer that is idle and has not fired.
    pub fn new() -> Self {
        Self { start_pos: None, start_time: None, touch_id: None, fired: false }
    }
}

impl GestureRecognizer for LongPressGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        // If already fired, wait for the matching finger's release before
        // re-arming. An unrelated finger's end must not reset a fired gesture.
        if self.fired {
            if let Event::TouchEnd { touch_id, .. } = event {
                if self.touch_id == Some(*touch_id) {
                    self.reset();
                }
            }
            return None;
        }

        match event {
            // The first finger owns the gesture; extra fingers are ignored until
            // it lifts, so they cannot restart or reassign the hold.
            Event::TouchBegin { pos, touch_id } if self.touch_id.is_none() => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                None
            }
            Event::TouchEnd { touch_id, .. } => {
                // Only the tracked finger ends the hold; an unrelated finger's
                // release must leave the tracked one undisturbed.
                if self.touch_id == Some(*touch_id) {
                    self.reset();
                }
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if self.touch_id == Some(*touch_id) {
                    if let Some(start) = self.start_pos {
                        // Cancel if finger moved too far
                        if distance(start, *pos) > LONG_PRESS_MAX_MOVE {
                            self.reset();
                        }
                    }
                }
                None
            }
            _ => {
                // Frame-count-based timeout check: fire when duration is exceeded
                // on any non-touch event, not just Timer.
                if let Some(start_time) = self.start_time {
                    let elapsed = now_ms.saturating_sub(start_time);
                    if elapsed >= LONG_PRESS_MIN_MS {
                        if let Some(pos) = self.start_pos {
                            self.fired = true;
                            return Some(Event::LongPress { pos });
                        }
                    }
                }
                None
            }
        }
    }

    fn reset(&mut self) {
        self.start_pos = None;
        self.start_time = None;
        self.touch_id = None;
        self.fired = false;
    }
}

crate::impl_default_via_new!(LongPressGesture);

// ────────────────────────────────────────────
// PanGesture (G1)
// ────────────────────────────────────────────

/// Continuous drag tracking recognizer.
///
/// Emits `Event::Drag { pos, touch_id, delta }` on every `TouchMove`
/// after a matching `TouchBegin`. Useful for scrolling, panning, and
/// slider drag operations.
///
/// ## State machine
/// - `Idle` — waiting for touch, or moved less than `PAN_MIN_DISTANCE` so far
/// - `Tracking` — past the threshold, emitting Drag on every move
///
/// ## Why there is a threshold
///
/// A recogniser that emits `Drag` on the very first `TouchMove` cannot be told
/// apart from a tap by its consumer: `ScrollArea` scrolls on `Drag::delta`, so a
/// finger that lands and jitters by one pixel — or a swipe that merely *starts*
/// stationary — would scroll the content before the gesture was known. Waiting for
/// `PAN_MIN_DISTANCE` of total travel makes the first emitted `Drag` mean "this is
/// a drag", not "this is a touch".
///
/// The threshold is measured against the **touch-down** point rather than the
/// previous move, so a slow drift cannot accumulate past it one pixel at a time and
/// still be reported as a drag from the first step.
///
/// ## Events consumed
/// - `TouchBegin { pos, touch_id }` → starts tracking
/// - `TouchMove { pos, touch_id }` → emits Drag with delta once past the threshold
/// - `TouchEnd { pos, touch_id }` → stops tracking (no event emitted)
#[derive(Debug, Clone)]
pub struct PanGesture {
    active: bool,
    touch_id: Option<TouchId>,
    last_pos: Option<Point>,
    /// Where the finger went down, kept as the threshold's origin.
    start_pos: Option<Point>,
    /// Set once total travel exceeds `PAN_MIN_DISTANCE`; from then on every move emits.
    dragging: bool,
}

impl PanGesture {
    /// Creates a recognizer that is not tracking any touch.
    pub fn new() -> Self {
        Self { active: false, touch_id: None, last_pos: None, start_pos: None, dragging: false }
    }
}

impl GestureRecognizer for PanGesture {
    fn process(&mut self, event: &Event, _now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } if !self.active => {
                self.active = true;
                self.touch_id = Some(*touch_id);
                self.last_pos = Some(*pos);
                self.start_pos = Some(*pos);
                self.dragging = false;
                None
            }
            Event::TouchMove { pos, touch_id }
                if self.active && Some(*touch_id) == self.touch_id =>
            {
                // Decide whether the drag has started *before* consuming the step,
                // so the first emitted `delta` covers the whole travel since the
                // last delivered position (touch-down) rather than only the step
                // that happened to cross the threshold.
                if !self.dragging {
                    let travelled = super::distance(self.start_pos?, *pos);
                    if travelled < PAN_MIN_DISTANCE {
                        // Below threshold: hold the position back so the accumulated
                        // displacement is not silently discarded.
                        return None;
                    }
                    self.dragging = true;
                }

                let delta = if let Some(last) = self.last_pos {
                    let (dx, dy) = point_delta(last, *pos);
                    Point::new(dx, dy)
                } else {
                    Point::new(0, 0)
                };
                self.last_pos = Some(*pos);

                Some(Event::Drag { pos: *pos, touch_id: *touch_id, delta })
            }
            Event::TouchEnd { pos: _, touch_id } if Some(*touch_id) == self.touch_id => {
                self.reset();
                None
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.active = false;
        self.touch_id = None;
        self.last_pos = None;
        self.start_pos = None;
        self.dragging = false;
    }
}

crate::impl_default_via_new!(PanGesture);

// ────────────────────────────────────────────
// LongPressDragGesture (G4)
// ────────────────────────────────────────────

/// Long press then drag recognizer.
///
/// Emits `Event::LongPress` when the finger is held stationary for
/// `LONG_PRESS_MIN_MS`, then emits `Event::Drag` on subsequent
/// `TouchMove` events.
///
/// ## State machine
/// - `Idle` -> `Holding` (TouchBegin) -> `Fired` (after timeout) -> `Dragging` (first move)
///
/// ## Events consumed
/// - `TouchBegin` -> starts the hold timer
/// - `TouchMove` -> cancels if before timer; emits Drag if after
/// - `TouchEnd` -> resets
#[derive(Debug, Clone)]
pub struct LongPressDragGesture {
    start_pos: Option<Point>,
    start_time: Option<u64>,
    touch_id: Option<TouchId>,
    long_press_fired: bool,
    dragging: bool,
    last_pos: Option<Point>,
}

impl LongPressDragGesture {
    /// Creates a recognizer that is idle: no held touch, no long press fired,
    /// and not dragging.
    pub fn new() -> Self {
        Self {
            start_pos: None,
            start_time: None,
            touch_id: None,
            long_press_fired: false,
            dragging: false,
            last_pos: None,
        }
    }
}

impl GestureRecognizer for LongPressDragGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } if self.start_pos.is_none() => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                self.long_press_fired = false;
                self.dragging = false;
                self.last_pos = Some(*pos);
                None
            }
            Event::TouchMove { pos, touch_id } if self.touch_id == Some(*touch_id) => {
                if self.dragging {
                    // Already in drag mode — emit Drag
                    let delta = if let Some(last) = self.last_pos {
                        let (dx, dy) = point_delta(last, *pos);
                        Point::new(dx, dy)
                    } else {
                        Point::new(0, 0)
                    };
                    self.last_pos = Some(*pos);
                    return Some(Event::Drag { pos: *pos, touch_id: *touch_id, delta });
                }
                if self.long_press_fired {
                    // First move after long press — enter drag mode
                    self.dragging = true;
                    self.last_pos = Some(*pos);
                    let delta = if let Some(start) = self.start_pos {
                        let (dx, dy) = point_delta(start, *pos);
                        Point::new(dx, dy)
                    } else {
                        Point::new(0, 0)
                    };
                    return Some(Event::Drag { pos: *pos, touch_id: *touch_id, delta });
                }
                // Before long press fired — check if movement exceeds threshold
                if let Some(start) = self.start_pos {
                    let (dx, dy) = point_delta_abs(start, *pos);
                    if (dx as f32) > LONG_PRESS_MAX_MOVE || (dy as f32) > LONG_PRESS_MAX_MOVE {
                        // Moved too much — cancel
                        self.reset();
                    }
                }
                None
            }
            Event::TouchEnd { pos: _, touch_id } if self.touch_id == Some(*touch_id) => {
                self.reset();
                None
            }
            _ => {
                // Check for long-press timeout only on timer events while holding
                if !self.long_press_fired && !self.dragging {
                    if let (Some(start_pos), Some(start)) = (self.start_pos, self.start_time) {
                        if now_ms >= start
                            && now_ms - start >= LONG_PRESS_MIN_MS
                            && matches!(event, Event::Timer { .. })
                        {
                            self.long_press_fired = true;
                            return Some(Event::LongPress { pos: start_pos });
                        }
                    }
                }
                None
            }
        }
    }

    fn reset(&mut self) {
        self.start_pos = None;
        self.start_time = None;
        self.touch_id = None;
        self.long_press_fired = false;
        self.dragging = false;
        self.last_pos = None;
    }
}

crate::impl_default_via_new!(LongPressDragGesture);

#[cfg(test)]
mod tests {
    use super::*;

    /// S-29: the first `Drag` must deliver the whole accumulated displacement
    /// since touch-down, not just the step that crossed the threshold.
    #[test]
    fn first_drag_delivers_accumulated_displacement() {
        let mut pan = PanGesture::new();
        assert!(pan
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());

        // 7 px is below the threshold: no event, and no displacement consumed.
        assert!(pan
            .process(&Event::TouchMove { pos: Point::new(7, 0), touch_id: 1 }, 10)
            .is_none());

        // Crossing the threshold: the first Drag must carry all 9 px.
        let first = pan.process(&Event::TouchMove { pos: Point::new(9, 0), touch_id: 1 }, 20);
        match first {
            Some(Event::Drag { delta, .. }) => assert_eq!(
                delta,
                Point::new(9, 0),
                "the first Drag must cover the whole travel since touch-down"
            ),
            other => panic!("crossing the threshold must emit a Drag, got {other:?}"),
        }

        // Later deltas are per-step and must not repeat the accumulated travel.
        let second = pan.process(&Event::TouchMove { pos: Point::new(11, 0), touch_id: 1 }, 30);
        match second {
            Some(Event::Drag { delta, .. }) => assert_eq!(delta, Point::new(2, 0)),
            other => panic!("a subsequent move must emit a step delta, got {other:?}"),
        }
    }

    /// S-29 acceptance: a Pan ignores an unrelated finger's end and keeps
    /// tracking its own touch.
    #[test]
    fn pan_ignores_an_unrelated_finger_end() {
        let mut pan = PanGesture::new();
        assert!(pan
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        // An unrelated finger lifting must not end the tracked drag.
        assert!(pan
            .process(&Event::TouchEnd { pos: Point::new(99, 99), touch_id: 2 }, 10)
            .is_none());
        let drag = pan.process(&Event::TouchMove { pos: Point::new(50, 0), touch_id: 1 }, 20);
        assert!(
            matches!(drag, Some(Event::Drag { .. })),
            "the tracked finger must still drag after an unrelated end, got {drag:?}"
        );
    }

    /// S-30: an unrelated finger's end must not cancel a holding long press.
    #[test]
    fn unrelated_finger_end_does_not_cancel_a_holding_long_press() {
        let mut gesture = LongPressGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 20), touch_id: 1 }, 0)
            .is_none());
        // Finger 2 lifting must not cancel finger 1's hold.
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(99, 99), touch_id: 2 }, 100)
            .is_none());
        let produced = gesture.process(&Event::Timer { id: 0 }, super::LONG_PRESS_MIN_MS + 1);
        assert!(
            matches!(produced, Some(Event::LongPress { pos }) if pos == Point::new(10, 20)),
            "the tracked finger must still fire LongPress after an unrelated end, got {produced:?}"
        );
    }

    /// S-30: once fired, only the matching finger's end resets the gesture;
    /// an unrelated finger's end and any new finger's begin are ignored.
    #[test]
    fn unrelated_finger_end_does_not_reset_a_fired_long_press() {
        let mut gesture = LongPressGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 20), touch_id: 1 }, 0)
            .is_none());
        let produced = gesture.process(&Event::Timer { id: 0 }, super::LONG_PRESS_MIN_MS + 1);
        assert!(matches!(produced, Some(Event::LongPress { .. })));

        // Fired: an unrelated finger's end must not clear the fired state.
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(99, 99), touch_id: 2 }, 700)
            .is_none());
        // A new finger trying to begin while the original is still held is ignored.
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(5, 5), touch_id: 3 }, 750)
            .is_none());
        // No second fire: the recogniser is still waiting for finger 1 to lift.
        assert!(gesture.process(&Event::Timer { id: 0 }, 1400).is_none());
    }

    /// S-30: a second finger landing while a hold is in progress must not
    /// restart or reassign the tracked touch.
    #[test]
    fn second_finger_begin_is_ignored_while_holding() {
        let mut gesture = LongPressGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 20), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(50, 60), touch_id: 2 }, 100)
            .is_none());
        let produced = gesture.process(&Event::Timer { id: 0 }, super::LONG_PRESS_MIN_MS + 1);
        assert!(
            matches!(produced, Some(Event::LongPress { pos }) if pos == Point::new(10, 20)),
            "a second finger must not reassign the hold, got {produced:?}"
        );
    }
}
