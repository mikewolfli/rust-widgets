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
                let velocity = if dt > 0 { total_dist / dt as f32 } else { 0.0 };

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

impl Default for SwipeGesture {
    fn default() -> Self {
        Self::new()
    }
}

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
    centroid_start: Option<Point>,
    last_centroid: Option<Point>,
    start_time: Option<u64>,
}

impl TwoFingerSwipeGesture {
    pub fn new() -> Self {
        Self { touches: Vec::new(), centroid_start: None, last_centroid: None, start_time: None }
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
                    self.centroid_start = Self::compute_centroid(&self.touches);
                    self.last_centroid = self.centroid_start;
                    self.start_time = Some(now_ms);
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
                    let result = if let (Some(start), Some(end)) =
                        (self.centroid_start, self.last_centroid)
                    {
                        let dx = (end.x - start.x).abs();
                        let dy = (end.y - start.y).abs();
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        let elapsed =
                            now_ms.saturating_sub(self.start_time.unwrap_or(now_ms)).max(1) as f32;
                        let velocity = dist / elapsed;
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
        self.centroid_start = None;
        self.last_centroid = None;
        self.start_time = None;
    }
}

impl Default for TwoFingerSwipeGesture {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────
// FlingGesture
// ────────────────────────────────────────────

const FLING_MIN_VELOCITY: f32 = 0.3;
const FLING_MIN_DISTANCE: f32 = 15.0;
const VELOCITY_WINDOW_MS: u64 = 100;

/// Velocity-based fling/flick recognizer.
///
/// Detects a short, fast finger flick intended to trigger inertial
/// scrolling. Unlike [`SwipeGesture`] which requires a minimum
/// distance of 30px, `FlingGesture` can detect shorter motions
/// if they are fast enough (velocity > `FLING_MIN_VELOCITY` px/ms).
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
        Some(Point::new((dx / dt * 1000.0) as i32, (dy / dt * 1000.0) as i32))
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
                if total_distance >= FLING_MIN_DISTANCE || speed >= FLING_MIN_VELOCITY * 1000.0 {
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

impl Default for FlingGesture {
    fn default() -> Self {
        Self::new()
    }
}
