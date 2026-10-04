// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tap gesture recognizers: tap, double-tap, and two-finger tap.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{GestureRecognizer, DOUBLE_TAP_TIMEOUT_MS, MAX_STATIONARY_DISTANCE};

// ────────────────────────────────────────────
// TapGesture
// ────────────────────────────────────────────

/// Recognises a quick tap: touch-down followed by touch-up within
/// 300 ms and with negligible movement.
#[derive(Debug, Clone)]
pub struct TapGesture {
    start_pos: Option<Point>,
    start_time: Option<u64>,
    touch_id: Option<TouchId>,
}

impl TapGesture {
    /// Creates a recognizer that is not tracking a touch.
    pub fn new() -> Self {
        Self { start_pos: None, start_time: None, touch_id: None }
    }
}

impl GestureRecognizer for TapGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                None
            }
            Event::TouchEnd { pos, touch_id } => {
                if self.touch_id != Some(*touch_id) {
                    return None;
                }
                let start = self.start_pos?;
                let start_time = self.start_time?;
                let dt = now_ms.saturating_sub(start_time);
                if dt < super::TAP_TIMEOUT_MS
                    && super::distance(start, *pos) < MAX_STATIONARY_DISTANCE
                {
                    let result = Event::Tap { pos: *pos };
                    self.reset();
                    return Some(result);
                }
                self.reset();
                None
            }
            Event::TouchMove { pos, touch_id } => {
                // Cancel if finger moved too far
                if self.touch_id == Some(*touch_id) {
                    if let Some(start) = self.start_pos {
                        if super::distance(start, *pos) >= MAX_STATIONARY_DISTANCE {
                            self.reset();
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.start_pos = None;
        self.start_time = None;
        self.touch_id = None;
    }
}

crate::impl_default_via_new!(TapGesture);

// ────────────────────────────────────────────
// DoubleTapGesture
// ────────────────────────────────────────────

/// Recognises two quick taps within 400ms.
#[derive(Debug, Clone)]
pub struct DoubleTapGesture {
    first_tap_pos: Option<Point>,
    first_tap_time: Option<u64>,
    waiting_for_second: bool,
}

impl DoubleTapGesture {
    /// Creates a recognizer that is not waiting for a second tap.
    pub fn new() -> Self {
        Self { first_tap_pos: None, first_tap_time: None, waiting_for_second: false }
    }
}

impl GestureRecognizer for DoubleTapGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::Tap { pos } => {
                if self.waiting_for_second {
                    if let (Some(first_pos), Some(first_time)) =
                        (self.first_tap_pos, self.first_tap_time)
                    {
                        let dt = now_ms.saturating_sub(first_time);
                        if dt <= DOUBLE_TAP_TIMEOUT_MS
                            && super::distance(first_pos, *pos) < MAX_STATIONARY_DISTANCE * 2.0
                        {
                            let result = Event::DoubleTap { pos: *pos };
                            self.reset();
                            return Some(result);
                        }
                    }
                    // Second tap too slow — start over
                    self.first_tap_pos = Some(*pos);
                    self.first_tap_time = Some(now_ms);
                } else {
                    self.first_tap_pos = Some(*pos);
                    self.first_tap_time = Some(now_ms);
                    self.waiting_for_second = true;
                }
                None
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.first_tap_pos = None;
        self.first_tap_time = None;
        self.waiting_for_second = false;
    }
}

crate::impl_default_via_new!(DoubleTapGesture);

// ────────────────────────────────────────────
// TwoFingerTapGesture
// ────────────────────────────────────────────

const TWO_FINGER_TAP_TIMEOUT_MS: u64 = 150;
const TWO_FINGER_TAP_DURATION_MS: u64 = 300;
/// How many fingers make a two-finger gesture.
///
/// Named rather than inlined because the recogniser previously omitted this check
/// entirely; a bare `2` reads like an arithmetic detail instead of the gesture's
/// defining precondition.
const TWO_FINGER_COUNT: usize = 2;

/// Two-finger tap recognizer.
///
/// Detects a simultaneous two-finger tap (≈ right-click on touchscreens).
/// Both fingers must land within `TWO_FINGER_TAP_TIMEOUT_MS` of each
/// other and neither may move significantly.
///
/// ## Output event
/// - [`Event::TwoFingerTap { pos }`] — centroid of both touches
#[derive(Debug)]
pub struct TwoFingerTapGesture {
    /// (touchdown_pos, current_pos, touch_id, start_time)
    touches: Vec<(Point, Point, TouchId, u64)>,
    /// Completed (lifted) touches: `(start_time, end_time, end_pos)`. Kept
    /// around until the second finger lifts so every start/end time can be
    /// checked and the release centroid can be computed from where the fingers
    /// actually lifted.
    completed: Vec<(u64, u64, Point)>,
}

impl TwoFingerTapGesture {
    /// Creates a recognizer tracking no touches and with no recorded lifts.
    pub fn new() -> Self {
        Self { touches: Vec::new(), completed: Vec::new() }
    }
}

impl GestureRecognizer for TwoFingerTapGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                if self.touches.len() >= 2 {
                    return None;
                }
                // Store both touchdown_pos and current_pos as the same point initially
                self.touches.push((*pos, *pos, *touch_id, now_ms));
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if let Some(t) = self.touches.iter_mut().find(|(_, _, id, _)| *id == *touch_id) {
                    // Compare against the **original** touchdown position to prevent drift
                    let dx = (pos.x - t.0.x).abs();
                    let dy = (pos.y - t.0.y).abs();
                    if (dx as f32) > MAX_STATIONARY_DISTANCE
                        || (dy as f32) > MAX_STATIONARY_DISTANCE
                    {
                        self.reset(); // moved too much
                    } else {
                        t.1 = *pos; // update current position only
                    }
                }
                None
            }
            Event::TouchEnd { pos, touch_id } => {
                if let Some(idx) = self.touches.iter().position(|(_, _, id, _)| *id == *touch_id) {
                    // Read the start time by identity, *before* the removal below: indexing
                    // `touches[0]` afterwards is only valid while another finger remains,
                    // so the single-finger case would have panicked on an empty list.
                    let start_time = self.touches[idx].3;
                    self.touches.remove(idx);
                    // Record the release position (not the last move or the landing
                    // point) so the centroid reflects where the finger actually lifted.
                    self.completed.push((start_time, now_ms, *pos));
                    // A *two-finger* tap requires that two fingers were actually seen.
                    // Testing only `touches.is_empty()` accepted a single finger, because
                    // one finger lifting also empties the list — so every ordinary tap was
                    // also reported as `TwoFingerTap`. Counting the completed lifts is
                    // what distinguishes "both fingers came and went" from "one finger
                    // came and went".
                    if self.touches.is_empty() && self.completed.len() == TWO_FINGER_COUNT {
                        let (s0, e0, p0) = self.completed[0];
                        let (s1, e1, p1) = self.completed[1];
                        // Both fingers must land within the timeout of each other, and
                        // each finger's own hold must be within the duration bound.
                        // Checking the touch sequence itself — not a Timer — covers
                        // reversed release order and an over-long first finger.
                        let landing_gap = s0.abs_diff(s1);
                        let dur0 = e0.saturating_sub(s0);
                        let dur1 = e1.saturating_sub(s1);
                        if landing_gap <= TWO_FINGER_TAP_TIMEOUT_MS
                            && dur0 <= TWO_FINGER_TAP_DURATION_MS
                            && dur1 <= TWO_FINGER_TAP_DURATION_MS
                        {
                            // Centroid of the two release positions (i64 sums avoid
                            // overflow for large coordinates).
                            let centroid = Point::new(
                                ((p0.x as i64 + p1.x as i64) / 2) as i32,
                                ((p0.y as i64 + p1.y as i64) / 2) as i32,
                            );
                            self.reset();
                            return Some(Event::TwoFingerTap { pos: centroid });
                        }
                        self.reset();
                    } else if self.touches.is_empty() {
                        // Fewer than two fingers: this is not a two-finger gesture at all.
                        // Clear the partial record so a later single tap cannot combine
                        // with this one to reach the count.
                        self.reset();
                    }
                }
                None
            }
            _ => {
                // Timeout: if first touch is too old, reset
                if let Some(first) = self.touches.first() {
                    if now_ms.saturating_sub(first.3) > TWO_FINGER_TAP_TIMEOUT_MS * 2 {
                        self.reset();
                    }
                }
                None
            }
        }
    }

    fn reset(&mut self) {
        self.touches.clear();
        self.completed.clear();
    }
}

crate::impl_default_via_new!(TwoFingerTapGesture);

#[cfg(test)]
mod tests {
    use super::*;

    /// S-85: two fingers landing 500ms apart must not be accepted as a
    /// simultaneous two-finger tap, even when both lifts are individually quick.
    #[test]
    fn two_finger_tap_rejects_slow_inter_landing() {
        let mut gesture = TwoFingerTapGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(30, 10), touch_id: 2 }, 500)
            .is_none());
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(10, 10), touch_id: 1 }, 520)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(30, 10), touch_id: 2 }, 530);
        assert!(
            produced.is_none(),
            "a 500ms inter-landing gap must not be a TwoFingerTap, got {produced:?}"
        );
    }

    /// S-85: the first-released finger's over-long hold must invalidate the
    /// whole sequence, not just the last finger's duration.
    #[test]
    fn two_finger_tap_rejects_long_first_finger() {
        let mut gesture = TwoFingerTapGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(30, 10), touch_id: 2 }, 10)
            .is_none());
        // Finger 1 holds 400ms (> 300ms) before lifting.
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(10, 10), touch_id: 1 }, 400)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(30, 10), touch_id: 2 }, 60);
        assert!(
            produced.is_none(),
            "an over-long first finger must invalidate the tap, got {produced:?}"
        );
    }

    /// S-86: the centroid must use the release positions, not the landing
    /// positions, when there are no moves in between.
    #[test]
    fn two_finger_tap_uses_release_positions_for_centroid() {
        let mut gesture = TwoFingerTapGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(30, 10), touch_id: 2 }, 10)
            .is_none());
        // Lift at a different location than the landing point.
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(10, 20), touch_id: 1 }, 20)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(30, 20), touch_id: 2 }, 30);
        match produced {
            Some(Event::TwoFingerTap { pos }) => assert_eq!(pos, Point::new(20, 20)),
            other => panic!("expected TwoFingerTap, got {other:?}"),
        }
    }

    /// S-85/S-86: reversed release order still yields the correct tap and
    /// centroid, and the landing/duration bounds are enforced by the sequence.
    #[test]
    fn two_finger_tap_reversed_release_order() {
        let mut gesture = TwoFingerTapGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(30, 10), touch_id: 2 }, 10)
            .is_none());
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(30, 20), touch_id: 2 }, 20)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(10, 20), touch_id: 1 }, 30);
        match produced {
            Some(Event::TwoFingerTap { pos }) => assert_eq!(pos, Point::new(20, 20)),
            other => panic!("reversed release order must still be a tap, got {other:?}"),
        }
    }

    /// S-85: the 150ms landing and 300ms duration bounds are inclusive.
    #[test]
    fn two_finger_tap_accepts_boundaries() {
        let mut gesture = TwoFingerTapGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 }, 0)
            .is_none());
        // Exactly 150ms after the first finger lands.
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(30, 10), touch_id: 2 }, 150)
            .is_none());
        // Each finger holds exactly 300ms.
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(10, 10), touch_id: 1 }, 300)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(30, 10), touch_id: 2 }, 450);
        match produced {
            Some(Event::TwoFingerTap { pos }) => assert_eq!(pos, Point::new(20, 10)),
            other => panic!("boundary timings must still be a tap, got {other:?}"),
        }
    }
}
