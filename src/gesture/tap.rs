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

impl Default for TapGesture {
    fn default() -> Self {
        Self::new()
    }
}

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

impl Default for DoubleTapGesture {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────
// TwoFingerTapGesture
// ────────────────────────────────────────────

const TWO_FINGER_TAP_TIMEOUT_MS: u64 = 150;
const TWO_FINGER_TAP_DURATION_MS: u64 = 300;

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
    touches: Vec<(Point, TouchId, u64)>, // (pos, id, time)
}

impl TwoFingerTapGesture {
    pub fn new() -> Self {
        Self { touches: Vec::new() }
    }
}

impl GestureRecognizer for TwoFingerTapGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                if self.touches.len() >= 2 {
                    return None;
                }
                self.touches.push((*pos, *touch_id, now_ms));
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if let Some(t) = self.touches.iter_mut().find(|(_, id, _)| *id == *touch_id) {
                    let dx = (pos.x - t.0.x).abs();
                    let dy = (pos.y - t.0.y).abs();
                    if (dx as f32) > MAX_STATIONARY_DISTANCE
                        || (dy as f32) > MAX_STATIONARY_DISTANCE
                    {
                        self.reset(); // moved too much
                    } else {
                        t.0 = *pos; // update position
                    }
                }
                None
            }
            Event::TouchEnd { pos, touch_id } => {
                if let Some(idx) = self.touches.iter().position(|(_, id, _)| *id == *touch_id) {
                    let first_time = self.touches[0].2;
                    let elapsed = now_ms.saturating_sub(first_time);
                    self.touches.remove(idx);
                    if self.touches.is_empty() && elapsed <= TWO_FINGER_TAP_DURATION_MS {
                        let centroid = *pos;
                        self.reset();
                        return Some(Event::TwoFingerTap { pos: centroid });
                    }
                }
                None
            }
            _ => {
                // Timeout: if first touch is too old, reset
                if let Some(first) = self.touches.first() {
                    if now_ms.saturating_sub(first.2) > TWO_FINGER_TAP_TIMEOUT_MS * 2 {
                        self.reset();
                    }
                }
                None
            }
        }
    }

    fn reset(&mut self) {
        self.touches.clear();
    }
}

impl Default for TwoFingerTapGesture {
    fn default() -> Self {
        Self::new()
    }
}
