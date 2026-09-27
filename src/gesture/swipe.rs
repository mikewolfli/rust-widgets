// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Swipe and fling gesture recognizers.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{distance, GestureRecognizer, SWIPE_MIN_DISTANCE, SWIPE_MIN_VELOCITY};

// ────────────────────────────────────────────
// SwipeGesture
// ────────────────────────────────────────────

/// Recognises a rapid directional swipe gesture.
#[derive(Debug, Clone)]
pub struct SwipeGesture {
    start_pos: Option<Point>,
    start_time: Option<u64>,
    last_pos: Option<Point>,
    last_time: Option<u64>,
    touch_id: Option<TouchId>,
}

impl SwipeGesture {
    /// Creates a recognizer with no touch in flight.
    pub fn new() -> Self {
        Self { start_pos: None, start_time: None, last_pos: None, last_time: None, touch_id: None }
    }
}

impl GestureRecognizer for SwipeGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.last_pos = Some(*pos);
                self.last_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if self.touch_id == Some(*touch_id) {
                    self.last_pos = Some(*pos);
                    self.last_time = Some(now_ms);
                }
                None
            }
            Event::TouchEnd { pos, touch_id } => {
                if self.touch_id != Some(*touch_id) {
                    return None;
                }
                let start = self.start_pos?;
                let start_time = self.start_time?;
                let total_dist = distance(start, *pos);

                // Must exceed minimum distance
                if total_dist < SWIPE_MIN_DISTANCE {
                    self.reset();
                    return None;
                }

                let dt = now_ms.saturating_sub(start_time);
                // `Event::Swipe::velocity` is documented in logical pixels per
                // **second**, so scale the per-millisecond ratio. Emitting px/ms here
                // made this recogniser disagree with `FlingGesture` by 1000x on a
                // field both of them populate.
                let velocity = if dt > 0 { total_dist / dt as f32 * 1000.0 } else { 0.0 };

                if velocity >= SWIPE_MIN_VELOCITY {
                    let result = Event::Swipe { start, end: *pos, velocity };
                    self.reset();
                    return Some(result);
                }
                self.reset();
                None
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.start_pos = None;
        self.start_time = None;
        self.last_pos = None;
        self.last_time = None;
        self.touch_id = None;
    }
}

crate::impl_default_via_new!(SwipeGesture);

// ────────────────────────────────────────────
// TwoFingerSwipeGesture
// ────────────────────────────────────────────

/// Two-finger swipe recognizer.
///
/// Detects when two fingers move together in the same direction
/// (e.g., page navigation on a touchpad).
///
/// Tracks the centroid (average position) of both fingers and
/// emits on release if centroid displacement exceeds threshold.
///
/// ## Output event
/// - [`Event::TwoFingerSwipe { centroid_start, centroid_end, velocity }`]
#[derive(Debug)]
pub struct TwoFingerSwipeGesture {
    touches: Vec<(Point, TouchId)>,
    /// The centroid baseline and the instant it was taken, as **one** value.
    ///
    /// They are set on the same statement and read on the same statement, so two `Option`s
    /// could only ever disagree by mistake — and the pair was previously read with
    /// `start_time.unwrap_or(now_ms)`, an unreachable fallback that would have computed
    /// `elapsed == 0 → max(1)` and an absurd velocity had it ever been taken. Pairing them
    /// makes "both or neither" a type fact rather than an invariant to remember.
    baseline: Option<(u64, Point)>,
    last_centroid: Option<Point>,
}

impl TwoFingerSwipeGesture {
    /// Creates a recognizer tracking no fingers and no centroid baseline.
    pub fn new() -> Self {
        Self { touches: Vec::new(), baseline: None, last_centroid: None }
    }

    fn compute_centroid(touches: &[(Point, TouchId)]) -> Option<Point> {
        if touches.is_empty() {
            return None;
        }
        let sum_x: i32 = touches.iter().map(|(p, _)| p.x).sum();
        let sum_y: i32 = touches.iter().map(|(p, _)| p.y).sum();
        Some(Point::new(sum_x / touches.len() as i32, sum_y / touches.len() as i32))
    }
}

impl GestureRecognizer for TwoFingerSwipeGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                if self.touches.len() >= 2 {
                    return None;
                }
                self.touches.push((*pos, *touch_id));
                // Start tracking centroid when second finger arrives
                if self.touches.len() == 2 {
                    // One write, so the baseline point and its timestamp cannot drift apart.
                    if let Some(centroid) = Self::compute_centroid(&self.touches) {
                        self.baseline = Some((now_ms, centroid));
                        self.last_centroid = Some(centroid);
                    }
                }
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if let Some(t) = self.touches.iter_mut().find(|(_, id)| *id == *touch_id) {
                    t.0 = *pos;
                }
                // Update last_centroid when both fingers are active
                if self.touches.len() == 2 {
                    self.last_centroid = Self::compute_centroid(&self.touches);
                }
                None
            }
            Event::TouchEnd { pos: _, touch_id } => {
                self.touches.retain(|(_, id)| *id != *touch_id);
                if self.touches.is_empty() {
                    // Both fingers lifted — evaluate swipe
                    let result = if let (Some((started_at, start)), Some(end)) =
                        (self.baseline, self.last_centroid)
                    {
                        let dx = (end.x - start.x).abs();
                        let dy = (end.y - start.y).abs();
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        // No fallback: the baseline's timestamp is part of the same `Option` as
                        // the start point, so reaching this arm guarantees it is present.
                        let elapsed = now_ms.saturating_sub(started_at).max(1) as f32;
                        // Logical pixels per second, matching `Event::TwoFingerSwipe`'s
                        // documented unit and the other swipe recognisers.
                        let velocity = dist / elapsed * 1000.0;
                        if dist >= super::SWIPE_MIN_DISTANCE
                            && velocity >= super::SWIPE_MIN_VELOCITY
                        {
                            Some(Event::TwoFingerSwipe {
                                centroid_start: start,
                                centroid_end: end,
                                velocity,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    self.reset();
                    return result;
                }
                None
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.touches.clear();
        self.baseline = None;
        self.last_centroid = None;
    }
}

crate::impl_default_via_new!(TwoFingerSwipeGesture);

// ────────────────────────────────────────────
// FlingGesture
// ────────────────────────────────────────────

/// Minimum speed (px/s) for a flick to be recognised as a fling.
///
/// Same unit as [`SWIPE_MIN_VELOCITY`](super::SWIPE_MIN_VELOCITY) and the public
/// event fields: logical pixels per second. It is lower than the swipe threshold
/// because a fling is allowed to cover a shorter distance.
const FLING_MIN_VELOCITY: f32 = 300.0;
/// Minimum travel (px) for a fling, which is shorter than a swipe's requirement.
const FLING_MIN_DISTANCE: f32 = 15.0;
/// Length of the trailing sample window (ms) used to estimate fling velocity.
const VELOCITY_WINDOW_MS: u64 = 100;

/// Velocity-based fling/flick recognizer.
///
/// Detects a short, fast finger flick intended to trigger inertial
/// scrolling. Unlike [`SwipeGesture`] which requires a minimum
/// distance of 30px, `FlingGesture` can detect shorter motions
/// if they are fast enough (velocity > `FLING_MIN_VELOCITY`, in px/s).
///
/// Uses a sliding-window velocity estimate (last ~100ms of movement)
/// to distinguish flicks from slow pans.
///
/// ## Output event
/// - [`Event::Fling { pos, velocity, touch_id }`]
#[derive(Debug)]
pub struct FlingGesture {
    start_pos: Option<Point>,
    start_time: Option<u64>,
    touch_id: Option<TouchId>,
    samples: Vec<(Point, u64)>, // (pos, time_ms) ring buffer
}

impl FlingGesture {
    /// Creates a recognizer with no touch in flight and an empty sample window.
    pub fn new() -> Self {
        Self { start_pos: None, start_time: None, touch_id: None, samples: Vec::new() }
    }

    fn compute_velocity(&self) -> Option<Point> {
        let now = self.samples.last()?.1;
        let cutoff = now.saturating_sub(VELOCITY_WINDOW_MS);
        // Find samples within the window
        let recent: Vec<_> = self.samples.iter().filter(|(_, t)| *t >= cutoff).collect();
        if recent.len() < 2 {
            return None;
        }
        let first = recent.first()?;
        let last = recent.last()?;
        let dt = last.1.saturating_sub(first.1).max(1) as f32;
        let dx = (last.0.x - first.0.x) as f32;
        let dy = (last.0.y - first.0.y) as f32;
        // `Event::Fling::velocity` is documented as logical pixels per **second**,
        // so the per-millisecond ratio is scaled by 1000. Keep this in step with the
        // `Swipe` variants, which convert the same way.
        Some(Point::new((dx / dt * 1000.0).round() as i32, (dy / dt * 1000.0).round() as i32))
    }
}

impl GestureRecognizer for FlingGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } if self.start_pos.is_none() => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                self.samples.clear();
                self.samples.push((*pos, now_ms));
                None
            }
            Event::TouchMove { pos, touch_id } if self.touch_id == Some(*touch_id) => {
                self.samples.push((*pos, now_ms));
                // Trim old samples
                let cutoff = now_ms.saturating_sub(VELOCITY_WINDOW_MS * 2);
                self.samples.retain(|(_, t)| *t >= cutoff);
                None
            }
            Event::TouchEnd { pos, touch_id } if self.touch_id == Some(*touch_id) => {
                self.samples.push((*pos, now_ms));
                let velocity = self.compute_velocity();
                let total_distance = if let Some(start) = self.start_pos {
                    let dx = (pos.x - start.x).abs();
                    let dy = (pos.y - start.y).abs();
                    ((dx * dx + dy * dy) as f32).sqrt()
                } else {
                    0.0
                };
                let speed = if let Some(v) = velocity {
                    ((v.x * v.x + v.y * v.y) as f32).sqrt()
                } else {
                    0.0
                };
                self.reset();
                if total_distance >= FLING_MIN_DISTANCE || speed >= FLING_MIN_VELOCITY {
                    Some(Event::Fling {
                        pos: *pos,
                        velocity: velocity.unwrap_or(Point::new(0, 0)),
                        touch_id: *touch_id,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.start_pos = None;
        self.start_time = None;
        self.touch_id = None;
        self.samples.clear();
    }
}

crate::impl_default_via_new!(FlingGesture);

#[cfg(test)]
mod tests {
    use super::*;

    /// D-9: a two-finger swipe that travels far enough reports a sane velocity.
    ///
    /// # The defect this pins
    ///
    /// The baseline centroid and its start time were two separate `Option`s, read as
    /// `now_ms.saturating_sub(self.start_time.unwrap_or(now_ms))`. The fallback was unreachable —
    /// both were set on one statement — but had it ever been taken it would have produced
    /// `elapsed == 0 → max(1)` and an absurd velocity. Storing the pair as one `Option` makes
    /// "both or neither" a type fact, and this test is what proves the elapsed time is real:
    /// the velocity must reflect the 200 ms the gesture actually took, not the `1 ms` floor.
    #[test]
    fn a_two_finger_swipe_velocity_uses_the_real_elapsed_time() {
        let mut gesture = TwoFingerSwipeGesture::new();

        // Two fingers land at t=0, well within the minimum distance from where they end.
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 100), touch_id: 2 }, 0)
            .is_none());

        // They move together to +200 in x over 200 ms.
        let moved = Point::new(200, 50);
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(200, 0), touch_id: 1 }, 200)
            .is_none());
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(200, 100), touch_id: 2 }, 200)
            .is_none());

        // The first finger lifts: still one down, so no evaluation yet.
        assert!(gesture.process(&Event::TouchEnd { pos: moved, touch_id: 1 }, 200).is_none());
        let Some(Event::TwoFingerSwipe { centroid_start, centroid_end, velocity }) =
            gesture.process(&Event::TouchEnd { pos: moved, touch_id: 2 }, 200)
        else {
            panic!("a 200px two-finger travel must produce a swipe");
        };

        assert_eq!(centroid_start, Point::new(0, 50));
        assert_eq!(centroid_end, Point::new(200, 50));
        // 200 px in 200 ms is 1000 px/s. The `1 ms` floor would have reported 200000 px/s.
        assert!(
            (velocity - 1000.0).abs() < 0.5,
            "the velocity must use the real 200 ms elapsed time, not the clamp; got {velocity}"
        );
    }

    /// A release with no second finger ever having landed produces nothing.
    #[test]
    fn a_single_finger_never_produces_a_two_finger_swipe() {
        let mut gesture = TwoFingerSwipeGesture::new();
        let _ = gesture.process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0);
        let _ = gesture.process(&Event::TouchMove { pos: Point::new(500, 0), touch_id: 1 }, 100);
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(500, 0), touch_id: 1 }, 100)
            .is_none());
    }
}
