// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Swipe and fling gesture recognizers.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{distance, point_delta, tuning, GestureKind, GestureRecognizer};

// ────────────────────────────────────────────
// SwipeGesture
// ────────────────────────────────────────────

/// Recognises a rapid directional swipe gesture.
///
/// # Mid-gesture direction lock (W-205)
///
/// The recogniser used to decide everything on `TouchEnd`, so a finger that travelled far and
/// then reversed still had its earlier travel counted. The move path now locks the dominant axis
/// once the accumulated travel crosses [`GestureTuning::swipe_min_distance`], and a move whose
/// component along that axis points back the other way **cancels** the attempt (the finger drew
/// one direction and then undid it). `partial` exposes the locked direction and current speed so
/// a host can preview the swipe before the release confirms it.
#[derive(Debug, Clone)]
pub struct SwipeGesture {
    start_pos: Option<Point>,
    start_time: Option<u64>,
    last_pos: Option<Point>,
    last_time: Option<u64>,
    touch_id: Option<TouchId>,
    /// The locked (axis, sign) once the travel crossed the minimum distance.
    ///
    /// `sign` is the direction along the axis: `1` for right/down, `-1` for left/up. A move that
    /// reverses it cancels the attempt rather than letting the two directions average out.
    locked: Option<(SwipeAxis, i32)>,
}

/// The dominant axis of a swipe once the direction is locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwipeAxis {
    Horizontal,
    Vertical,
}

impl SwipeGesture {
    /// Creates a recognizer with no touch in flight.
    pub fn new() -> Self {
        Self {
            start_pos: None,
            start_time: None,
            last_pos: None,
            last_time: None,
            touch_id: None,
            locked: None,
        }
    }

    /// The `(axis, sign)` locked by the current move, if the direction is already known.
    fn locked_direction(&self) -> Option<(SwipeAxis, i32)> {
        self.locked
    }
}

impl GestureRecognizer for SwipeGesture {
    fn kind(&self) -> GestureKind {
        GestureKind::Swipe
    }

    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.last_pos = Some(*pos);
                self.last_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                self.locked = None;
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if self.touch_id == Some(*touch_id) {
                    self.last_pos = Some(*pos);
                    self.last_time = Some(now_ms);
                    self.update_lock(*pos);
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
                if total_dist < tuning().swipe_min_distance {
                    self.reset();
                    return None;
                }

                // Final confirmation must not contradict the locked direction: a release that
                // ends behind the start point along the locked axis is the same cancellation the
                // move path applies, in the case where no move reported it.
                if let Some((axis, sign)) = self.locked_direction() {
                    let main = match axis {
                        SwipeAxis::Horizontal => pos.x as i64 - start.x as i64,
                        SwipeAxis::Vertical => pos.y as i64 - start.y as i64,
                    };
                    if main != 0 && main.signum() as i32 != sign {
                        self.reset();
                        return None;
                    }
                }

                let dt = now_ms.saturating_sub(start_time);
                // `Event::Swipe::velocity` is documented in logical pixels per
                // **second**, so scale the per-millisecond ratio. Emitting px/ms here
                // made this recogniser disagree with `FlingGesture` by 1000x on a
                // field both of them populate.
                let velocity = if dt > 0 { total_dist / dt as f32 * 1000.0 } else { 0.0 };

                if velocity >= tuning().swipe_min_velocity {
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

    /// The in-progress swipe, once the direction has locked (W-205).
    ///
    /// Returning `Event::Swipe` rather than a bespoke preview type keeps the public event
    /// vocabulary unchanged: the start, current point and current speed are exactly the fields a
    /// previewing host reads. A sub-threshold drag reports nothing because it has no direction
    /// yet; that is the honest answer, not a zero-velocity swipe.
    fn partial(&self) -> Option<Event> {
        let _ = self.locked_direction()?;
        let start = self.start_pos?;
        let end = self.last_pos?;
        let started_at = self.start_time?;
        let last_at = self.last_time?;
        let dt = last_at.saturating_sub(started_at);
        let velocity = if dt > 0 { distance(start, end) / dt as f32 * 1000.0 } else { 0.0 };
        Some(Event::Swipe { start, end, velocity })
    }

    fn reset(&mut self) {
        self.start_pos = None;
        self.start_time = None;
        self.last_pos = None;
        self.last_time = None;
        self.touch_id = None;
        self.locked = None;
    }
}

impl SwipeGesture {
    /// Locks the dominant axis once the travel crosses the threshold, or cancels on a reversal.
    fn update_lock(&mut self, pos: Point) {
        let Some(start) = self.start_pos else {
            return;
        };
        let dx = pos.x as i64 - start.x as i64;
        let dy = pos.y as i64 - start.y as i64;
        match self.locked {
            None => {
                if distance(start, pos) < tuning().swipe_min_distance {
                    return;
                }
                let axis =
                    if dx.abs() >= dy.abs() { SwipeAxis::Horizontal } else { SwipeAxis::Vertical };
                let main = match axis {
                    SwipeAxis::Horizontal => dx,
                    SwipeAxis::Vertical => dy,
                };
                if main != 0 {
                    self.locked = Some((axis, main.signum() as i32));
                }
            }
            Some((axis, sign)) => {
                let main = match axis {
                    SwipeAxis::Horizontal => dx,
                    SwipeAxis::Vertical => dy,
                };
                if main != 0 && main.signum() as i32 != sign {
                    // The finger reversed along the axis it had committed to. Undo the attempt
                    // rather than letting the two opposite travels average into a bogus swipe.
                    self.reset();
                }
            }
        }
    }
}

crate::impl_default_via_new!(SwipeGesture);

// ────────────────────────────────────────────
// TwoFingerSwipeGesture
// ────────────────────────────────────────────

/// Multi-finger swipe recognizer.
///
/// Detects when two (or, with [`TwoFingerSwipeGesture::with_fingers`], three or more) fingers
/// move together in the same direction (e.g., page navigation on a touchpad).
///
/// Tracks the centroid (average position) of the contacts and emits on release if the centroid
/// displacement exceeds threshold. Two fingers emit [`Event::TwoFingerSwipe`]; a larger count
/// emits [`Event::MultiFingerSwipe`] so the meanings cannot be confused.
///
/// ## Output event
/// - [`Event::TwoFingerSwipe { centroid_start, centroid_end, velocity }`]
/// - [`Event::MultiFingerSwipe { fingers, .. }`] for three or more contacts
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
    /// Final (release) positions of lifted touches, in lift order.
    ///
    /// These — not the last move or the landing point — define the end centroid
    /// once every finger is up, so a release with no `TouchMove` still measures
    /// the travel the fingers actually made.
    touch_ends: Vec<Point>,
    /// How many contacts the gesture needs. `2` for the conventional two-finger swipe; a host
    /// that wants a three-finger navigation gesture constructs one with
    /// [`Self::with_fingers`].
    fingers: usize,
}

impl TwoFingerSwipeGesture {
    /// Creates a recognizer tracking no fingers and no centroid baseline.
    pub fn new() -> Self {
        Self { touches: Vec::new(), baseline: None, touch_ends: Vec::new(), fingers: 2 }
    }

    /// Creates a recognizer that requires `fingers` contacts (clamped to at least 2).
    ///
    /// A count above 2 emits [`Event::MultiFingerSwipe`] instead of
    /// [`Event::TwoFingerSwipe`], so an existing two-finger consumer cannot mistake a
    /// three-finger gesture for its own.
    pub fn with_fingers(fingers: usize) -> Self {
        Self { fingers: fingers.max(2), ..Self::new() }
    }

    /// How many contacts this recognizer requires.
    pub fn fingers(&self) -> usize {
        self.fingers
    }

    /// The mean of the tracked contacts, accumulated in `i64` so no intermediate sum can
    /// overflow.
    ///
    /// The implementation used to collect the coordinates into a temporary `Vec<Point>` and then
    /// call [`Self::centroid_of_points`], which allocated on every multi-touch event even though
    /// the two-finger case needs no container at all (W-P04). One pass over the contacts produces
    /// the same answer with no allocation.
    fn compute_centroid(touches: &[(Point, TouchId)]) -> Option<Point> {
        if touches.is_empty() {
            return None;
        }
        let mut sum_x: i64 = 0;
        let mut sum_y: i64 = 0;
        for (pos, _) in touches {
            sum_x += pos.x as i64;
            sum_y += pos.y as i64;
        }
        let count = touches.len() as i64;
        Some(Point::new((sum_x / count) as i32, (sum_y / count) as i32))
    }

    /// Average of `points`, computed in `i64` so large coordinates cannot
    /// overflow the intermediate sum before the division.
    fn centroid_of_points(points: &[Point]) -> Option<Point> {
        if points.is_empty() {
            return None;
        }
        let n = points.len() as i64;
        let sum_x: i64 = points.iter().map(|p| p.x as i64).sum();
        let sum_y: i64 = points.iter().map(|p| p.y as i64).sum();
        Some(Point::new((sum_x / n) as i32, (sum_y / n) as i32))
    }
}

impl GestureRecognizer for TwoFingerSwipeGesture {
    fn kind(&self) -> GestureKind {
        GestureKind::TwoFingerSwipe
    }

    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                if self.touches.len() >= self.fingers {
                    return None;
                }
                self.touches.push((*pos, *touch_id));
                // Start tracking the centroid only once every required finger is down.
                if self.touches.len() == self.fingers {
                    // One write, so the baseline point and its timestamp cannot drift apart.
                    if let Some(centroid) = Self::compute_centroid(&self.touches) {
                        self.baseline = Some((now_ms, centroid));
                    }
                }
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if let Some(t) = self.touches.iter_mut().find(|(_, id)| *id == *touch_id) {
                    t.0 = *pos;
                }
                None
            }
            Event::TouchEnd { pos, touch_id } => {
                // The release position is part of the gesture: a finger that lifts
                // without a final move still ends at `pos`, and the end centroid must
                // reflect that rather than the last move or the landing point.
                if let Some(t) = self.touches.iter_mut().find(|(_, id)| *id == *touch_id) {
                    t.0 = *pos;
                    self.touch_ends.push(*pos);
                }
                self.touches.retain(|(_, id)| *id != *touch_id);
                if self.touches.is_empty() {
                    // Both fingers lifted — evaluate the swipe using the recorded
                    // release positions as the end centroid.
                    let result = if let (Some((started_at, start)), Some(end)) =
                        (self.baseline, Self::centroid_of_points(&self.touch_ends))
                    {
                        let dx = end.x as f64 - start.x as f64;
                        let dy = end.y as f64 - start.y as f64;
                        let dist = (dx * dx + dy * dy).sqrt() as f32;
                        // No fallback: the baseline's timestamp is part of the same
                        // `Option` as the start point, so reaching this arm guarantees
                        // it is present.
                        let elapsed = now_ms.saturating_sub(started_at).max(1) as f32;
                        // Logical pixels per second, matching `Event::TwoFingerSwipe`'s
                        // documented unit and the other swipe recognisers.
                        let velocity = dist / elapsed * 1000.0;
                        if dist >= tuning().swipe_min_distance
                            && velocity >= tuning().swipe_min_velocity
                        {
                            // A two-finger gesture keeps the established variant; 3+ fingers
                            // get their own so the two meanings cannot be confused.
                            if self.fingers == 2 {
                                Some(Event::TwoFingerSwipe {
                                    centroid_start: start,
                                    centroid_end: end,
                                    velocity,
                                })
                            } else {
                                Some(Event::MultiFingerSwipe {
                                    fingers: u8::try_from(self.fingers).unwrap_or(u8::MAX),
                                    centroid_start: start,
                                    centroid_end: end,
                                    velocity,
                                })
                            }
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
        self.touch_ends.clear();
    }
}

crate::impl_default_via_new!(TwoFingerSwipeGesture);

// ────────────────────────────────────────────
// FlingGesture
// ────────────────────────────────────────────

/// Length of the trailing sample window (ms) used to estimate fling velocity.
const VELOCITY_WINDOW_MS: u64 = 100;
/// How many recent samples the fling estimator retains.
///
/// # Why a fixed capacity (W-P03)
///
/// The estimator used to rebuild a `Vec` of the samples inside the trailing window on every
/// `TouchEnd`, and push+`retain` on every `TouchMove`. A fast scroll therefore allocated and freed
/// a vector per event. The window is bounded by time, and pointer samples arrive far slower than
/// one per millisecond on every platform this crate supports, so 64 slots hold many windows' worth
/// of motion; the oldest is overwritten when a pathological producer exceeds that.
const FLING_SAMPLE_CAPACITY: usize = 64;

/// A fixed-capacity ring of `(position, time_ms)` pointer samples.
///
/// Writes are in place, trimming advances the head, and iteration walks oldest to newest — the
/// order the velocity estimator needs. No operation allocates.
#[derive(Debug, Clone)]
struct SampleRing {
    samples: [(Point, u64); FLING_SAMPLE_CAPACITY],
    head: usize,
    len: usize,
}

impl SampleRing {
    fn new() -> Self {
        Self { samples: [(Point::new(0, 0), 0); FLING_SAMPLE_CAPACITY], head: 0, len: 0 }
    }

    fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    fn push(&mut self, pos: Point, time_ms: u64) {
        if self.len < FLING_SAMPLE_CAPACITY {
            let index = (self.head + self.len) % FLING_SAMPLE_CAPACITY;
            self.samples[index] = (pos, time_ms);
            self.len += 1;
        } else {
            // Full: overwrite the oldest slot and advance the head to it.
            self.samples[self.head] = (pos, time_ms);
            self.head = (self.head + 1) % FLING_SAMPLE_CAPACITY;
        }
    }

    fn get(&self, index: usize) -> (Point, u64) {
        self.samples[(self.head + index) % FLING_SAMPLE_CAPACITY]
    }

    /// Drops samples older than `cutoff`, oldest first.
    fn retain_since(&mut self, cutoff: u64) {
        while self.len > 0 && self.get(0).1 < cutoff {
            self.head = (self.head + 1) % FLING_SAMPLE_CAPACITY;
            self.len -= 1;
        }
    }
}

/// Velocity-based fling/flick recognizer.
///
/// Detects a short, fast finger flick intended to trigger inertial
/// scrolling. A fling requires **both** a minimum travel and a valid trailing
/// velocity (both read from [`super::GestureTuning`], in px and px/s), so a slow
/// long drag — even one that covers plenty of distance — is not reported as a
/// flick, and a release after a stationary hold has no velocity to report.
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
    /// Recent `(pos, time_ms)` samples in a fixed ring (W-P03): no per-event allocation.
    samples: SampleRing,
}

impl FlingGesture {
    /// Creates a recognizer with no touch in flight and an empty sample window.
    pub fn new() -> Self {
        Self { start_pos: None, start_time: None, touch_id: None, samples: SampleRing::new() }
    }

    fn compute_velocity(&self) -> Option<Point> {
        let (_, now) = self.samples.get(self.samples.len.checked_sub(1)?);
        let cutoff = now.saturating_sub(VELOCITY_WINDOW_MS);
        // Find the oldest sample still inside the window, then measure from it to the newest.
        let mut first_index = 0;
        while first_index < self.samples.len && self.samples.get(first_index).1 < cutoff {
            first_index += 1;
        }
        if self.samples.len - first_index < 2 {
            return None;
        }
        let first = self.samples.get(first_index);
        let last = self.samples.get(self.samples.len - 1);
        let dt = last.1.saturating_sub(first.1).max(1) as f32;
        // The displacement is widened like `total_distance` below, so a fling between coordinates
        // near the `i32` extremes reports the true displacement rather than a wrapped one. The
        // final `as i32` saturates through the clamp, matching how a very fast fling is handled.
        let (raw_dx, raw_dy) = point_delta(first.0, last.0);
        let dx = raw_dx as f32;
        let dy = raw_dy as f32;
        // `Event::Fling::velocity` is documented as logical pixels per **second**,
        // so the per-millisecond ratio is scaled by 1000. Keep this in step with the
        // `Swipe` variants, which convert the same way. A velocity above `i32::MAX`
        // saturates rather than wrapping to a nonsensical negative speed.
        let vx = (dx / dt * 1000.0).clamp(i32::MIN as f32, i32::MAX as f32) as i32;
        let vy = (dy / dt * 1000.0).clamp(i32::MIN as f32, i32::MAX as f32) as i32;
        Some(Point::new(vx, vy))
    }
}

impl GestureRecognizer for FlingGesture {
    fn kind(&self) -> GestureKind {
        GestureKind::Fling
    }

    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } if self.start_pos.is_none() => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                self.samples.clear();
                self.samples.push(*pos, now_ms);
                None
            }
            Event::TouchMove { pos, touch_id } if self.touch_id == Some(*touch_id) => {
                self.samples.push(*pos, now_ms);
                // Trim old samples
                let cutoff = now_ms.saturating_sub(VELOCITY_WINDOW_MS * 2);
                self.samples.retain_since(cutoff);
                None
            }
            Event::TouchEnd { pos, touch_id } if self.touch_id == Some(*touch_id) => {
                self.samples.push(*pos, now_ms);
                let velocity = self.compute_velocity();
                // Compute magnitudes in `f64` so that ordinary-but-fast gestures
                // (e.g. 50 px in 1 ms -> 50000 px/s) cannot overflow `i32` when the
                // velocity component is squared, and large legal coordinates cannot
                // overflow the distance subtraction.
                let total_distance = if let Some(start) = self.start_pos {
                    let dx = pos.x as f64 - start.x as f64;
                    let dy = pos.y as f64 - start.y as f64;
                    (dx * dx + dy * dy).sqrt() as f32
                } else {
                    0.0
                };
                let speed = if let Some(v) = velocity {
                    let vx = v.x as f64;
                    let vy = v.y as f64;
                    (vx * vx + vy * vy).sqrt() as f32
                } else {
                    0.0
                };
                self.reset();
                // A fling needs *both* enough travel and a valid trailing velocity:
                // a slow long drag (or a hold-then-release) must not masquerade as a
                // high-speed flick just because it covered the distance.
                let tuning = tuning();
                if total_distance >= tuning.fling_min_distance && speed >= tuning.fling_min_velocity
                {
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

    // ── S-83: a fling needs both minimum distance AND a valid velocity ──

    /// A slow long drag covers the distance but has no trailing velocity, so it
    /// must not be reported as a `Fling` (the old OR contract emitted one with a
    /// zero velocity vector).
    #[test]
    fn slow_long_drag_is_not_a_fling() {
        let mut gesture = FlingGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(20, 0), touch_id: 1 }, 1000);
        assert!(
            produced.is_none(),
            "a slow long drag must not be reported as a Fling, got {produced:?}"
        );
    }

    /// A fast move followed by a stationary hold then release has no velocity in
    /// the trailing sample window and must not be a fling.
    #[test]
    fn hold_then_release_is_not_a_fling() {
        let mut gesture = FlingGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(30, 0), touch_id: 1 }, 50)
            .is_none());
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(30, 0), touch_id: 1 }, 400)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(30, 0), touch_id: 1 }, 450);
        assert!(produced.is_none(), "a hold-then-release must not be a Fling, got {produced:?}");
    }

    /// A short, fast flick has both enough travel and a valid velocity.
    #[test]
    fn short_fast_flick_is_a_fling() {
        let mut gesture = FlingGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(20, 0), touch_id: 1 }, 50);
        match produced {
            Some(Event::Fling { velocity, .. }) => assert_eq!(velocity, Point::new(400, 0)),
            other => panic!("a short fast flick must be a Fling, got {other:?}"),
        }
    }

    // ── S-84: widen before squaring so magnitudes cannot overflow i32 ──

    /// 50 px in 1 ms is 50000 px/s; squaring that as `i32` overflows. The
    /// magnitude must be computed after widening to `f64`.
    #[test]
    fn fling_velocity_magnitude_does_not_overflow_i32() {
        let mut gesture = FlingGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        let produced = gesture.process(&Event::TouchEnd { pos: Point::new(50, 0), touch_id: 1 }, 1);
        match produced {
            Some(Event::Fling { velocity, .. }) => {
                assert_eq!(velocity.x, 50000);
                assert_eq!(velocity.y, 0);
            }
            other => panic!("a fast flick must still be a Fling, got {other:?}"),
        }
    }

    /// 50000 px of two-finger travel squared as `i32` overflows; the distance
    /// magnitude must be computed after widening to `f64`.
    #[test]
    fn two_finger_swipe_distance_does_not_overflow_i32() {
        let mut gesture = TwoFingerSwipeGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 100), touch_id: 2 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(50000, 0), touch_id: 1 }, 100)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(50000, 100), touch_id: 2 }, 100);
        assert!(
            matches!(produced, Some(Event::TwoFingerSwipe { .. })),
            "large-coordinate travel must produce a swipe, got {produced:?}"
        );
    }

    // ── S-86: the release position defines the end centroid ──

    /// With no `TouchMove` at all, the release positions must still define the
    /// travel and the end centroid.
    #[test]
    fn two_finger_swipe_uses_release_positions_without_moves() {
        let mut gesture = TwoFingerSwipeGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 100), touch_id: 2 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(200, 0), touch_id: 1 }, 100)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(200, 100), touch_id: 2 }, 100);
        match produced {
            Some(Event::TwoFingerSwipe { centroid_start, centroid_end, .. }) => {
                assert_eq!(centroid_start, Point::new(0, 50));
                assert_eq!(centroid_end, Point::new(200, 50));
            }
            other => panic!("a release without moves must still produce a swipe, got {other:?}"),
        }
    }

    /// When the release position differs from the last move, the release wins.
    #[test]
    fn two_finger_swipe_release_positions_override_last_move() {
        let mut gesture = TwoFingerSwipeGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 100), touch_id: 2 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(100, 0), touch_id: 1 }, 50)
            .is_none());
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(100, 100), touch_id: 2 }, 50)
            .is_none());
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(200, 0), touch_id: 1 }, 100)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(200, 100), touch_id: 2 }, 100);
        match produced {
            Some(Event::TwoFingerSwipe { centroid_end, .. }) => {
                assert_eq!(
                    centroid_end,
                    Point::new(200, 50),
                    "release must override the last move"
                );
            }
            other => panic!("expected TwoFingerSwipe, got {other:?}"),
        }
    }

    /// The end centroid is order-independent of which finger lifts first.
    #[test]
    fn two_finger_swipe_release_order_is_consistent() {
        let mut gesture = TwoFingerSwipeGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 100), touch_id: 2 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(200, 100), touch_id: 2 }, 100)
            .is_none());
        let produced =
            gesture.process(&Event::TouchEnd { pos: Point::new(200, 0), touch_id: 1 }, 100);
        match produced {
            Some(Event::TwoFingerSwipe { centroid_end, .. }) => {
                assert_eq!(centroid_end, Point::new(200, 50));
            }
            other => panic!("reversed release order must still produce a swipe, got {other:?}"),
        }
    }

    /// W-205: the axis locks mid-gesture and the in-progress swipe is previewable.
    #[test]
    fn a_swipe_locks_its_axis_and_previews_before_release() {
        let mut gesture = SwipeGesture::new();
        let id = 1;
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: id }, 0)
            .is_none());
        // Below the minimum travel there is no direction to report yet.
        assert!(gesture.partial().is_none());
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(10, 1), touch_id: id }, 10)
            .is_none());
        assert!(gesture.partial().is_none(), "a sub-threshold drag has no direction");
        // Crossing the threshold locks horizontal.
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(80, 3), touch_id: id }, 20)
            .is_none());
        match gesture.partial() {
            Some(Event::Swipe { start, end, velocity }) => {
                assert_eq!(start, Point::new(0, 0));
                assert_eq!(end, Point::new(80, 3));
                assert!(velocity > 0.0, "the preview must carry the current speed, got {velocity}");
            }
            other => panic!("a locked swipe must preview as a Swipe, got {other:?}"),
        }
        match gesture.process(&Event::TouchEnd { pos: Point::new(160, 4), touch_id: id }, 30) {
            Some(Event::Swipe { start, end, .. }) => {
                assert_eq!(start, Point::new(0, 0));
                assert_eq!(end, Point::new(160, 4));
            }
            other => panic!("the release is still the final confirmation, got {other:?}"),
        }
    }

    /// W-205: a reverse move along the locked axis cancels the attempt instead of averaging out.
    #[test]
    fn a_swipe_that_reverses_is_cancelled() {
        let mut gesture = SwipeGesture::new();
        let id = 1;
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: id }, 0)
            .is_none());
        // Lock horizontal to the right.
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(100, 0), touch_id: id }, 10)
            .is_none());
        assert!(gesture.partial().is_some());
        // Draw back past the origin: the locked direction was right, so a negative component
        // along that axis is the reversal the recogniser must cancel on.
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(-10, 0), touch_id: id }, 20)
            .is_none());
        assert!(gesture.partial().is_none(), "a cancelled attempt has no preview");
        // The release cannot resurrect it, even though the total travel looks large enough.
        assert!(gesture
            .process(&Event::TouchEnd { pos: Point::new(-10, 0), touch_id: id }, 30)
            .is_none());
    }

    /// W-P03: the fling sample window is a fixed ring, newest-last, with time-based trimming.
    #[test]
    fn the_fling_sample_ring_keeps_the_newest_window_in_order() {
        let mut ring = SampleRing::new();
        for i in 0..5u64 {
            ring.push(Point::new(i as i32, 0), i * 10);
        }
        assert_eq!(ring.len, 5);
        assert_eq!(ring.get(0), (Point::new(0, 0), 0));
        assert_eq!(ring.get(4), (Point::new(4, 0), 40));

        // Trimming advances the head, so the oldest surviving sample becomes index 0.
        ring.retain_since(20);
        assert_eq!(ring.len, 3);
        assert_eq!(ring.get(0), (Point::new(2, 0), 20));
        assert_eq!(ring.get(2), (Point::new(4, 0), 40));

        // Filling past capacity overwrites the oldest slot instead of allocating.
        for i in 5..(FLING_SAMPLE_CAPACITY as u64 + 5) {
            ring.push(Point::new(i as i32, 0), i * 10);
        }
        assert_eq!(ring.len, FLING_SAMPLE_CAPACITY);
        let oldest = ring.get(0);
        let newest = ring.get(FLING_SAMPLE_CAPACITY - 1);
        assert!(oldest.1 <= newest.1, "the ring must stay oldest-to-newest");
        assert_eq!(newest.1, (FLING_SAMPLE_CAPACITY as u64 + 4) * 10);
    }
}

#[cfg(test)]
mod multi_finger_tests {
    use super::*;

    /// Three contacts are tracked to their own event, while two keep the established one.
    #[test]
    fn three_fingers_emit_the_multi_finger_variant() {
        let mut gesture = TwoFingerSwipeGesture::with_fingers(3);
        assert_eq!(gesture.fingers(), 3);
        for (index, (x, y)) in [(0, 0), (0, 10), (0, 20)].iter().enumerate() {
            assert!(gesture
                .process(
                    &Event::TouchBegin { pos: Point::new(*x, *y), touch_id: index as u64 + 1 },
                    0,
                )
                .is_none());
        }
        // All three lift 200 px to the right, 100 ms later.
        let mut produced = None;
        for (index, y) in [0, 10, 20].iter().enumerate() {
            produced = gesture.process(
                &Event::TouchEnd { pos: Point::new(200, *y), touch_id: index as u64 + 1 },
                100,
            );
        }
        match produced {
            Some(Event::MultiFingerSwipe { fingers, centroid_start, centroid_end, velocity }) => {
                assert_eq!(fingers, 3);
                assert_eq!(centroid_start, Point::new(0, 10));
                assert_eq!(centroid_end, Point::new(200, 10));
                assert!(velocity >= 500.0, "200 px in 100 ms is 2000 px/s, got {velocity}");
            }
            other => panic!("three fingers must emit a multi-finger swipe, got {other:?}"),
        }
    }

    /// A count below two is clamped, so a misconfigured recogniser cannot behave like a
    /// one-finger gesture that the two-finger event name contradicts.
    #[test]
    fn the_finger_count_is_clamped_to_at_least_two() {
        assert_eq!(TwoFingerSwipeGesture::with_fingers(0).fingers(), 2);
        assert_eq!(TwoFingerSwipeGesture::with_fingers(1).fingers(), 2);
        assert_eq!(TwoFingerSwipeGesture::with_fingers(4).fingers(), 4);
    }
}
