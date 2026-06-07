//! Press-hold gesture recognizers: long press, pan, and long-press-drag.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{distance, GestureRecognizer, LONG_PRESS_MAX_MOVE, LONG_PRESS_MIN_MS};

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
    pub fn new() -> Self {
        Self { start_pos: None, start_time: None, touch_id: None, fired: false }
    }
}

impl GestureRecognizer for LongPressGesture {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        // If already fired, wait for release before re-arming
        if self.fired {
            if matches!(event, Event::TouchEnd { .. }) {
                self.reset();
            }
            return None;
        }

        match event {
            Event::TouchBegin { pos, touch_id } => {
                self.start_pos = Some(*pos);
                self.start_time = Some(now_ms);
                self.touch_id = Some(*touch_id);
                None
            }
            Event::TouchEnd { .. } => {
                self.reset();
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
                // Check time threshold on non-touch events (e.g. timer ticks)
                if let Some(start_time) = self.start_time {
                    let elapsed = now_ms.saturating_sub(start_time);
                    // Only fire on a timer event or when duration is exceeded
                    if elapsed >= LONG_PRESS_MIN_MS && matches!(event, Event::Timer { .. }) {
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

impl Default for LongPressGesture {
    fn default() -> Self {
        Self::new()
    }
}

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
/// - `Idle` — waiting for touch
/// - `Tracking` — finger down, emitting Drag on every move
///
/// ## Events consumed
/// - `TouchBegin { pos, touch_id }` → starts tracking
/// - `TouchMove { pos, touch_id }` → emits Drag with delta
/// - `TouchEnd { pos, touch_id }` → stops tracking (no event emitted)
#[derive(Debug, Clone)]
pub struct PanGesture {
    active: bool,
    touch_id: Option<TouchId>,
    last_pos: Option<Point>,
}

impl PanGesture {
    pub fn new() -> Self {
        Self { active: false, touch_id: None, last_pos: None }
    }
}

impl GestureRecognizer for PanGesture {
    fn process(&mut self, event: &Event, _now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } if !self.active => {
                self.active = true;
                self.touch_id = Some(*touch_id);
                self.last_pos = Some(*pos);
                None
            }
            Event::TouchMove { pos, touch_id }
                if self.active && Some(*touch_id) == self.touch_id =>
            {
                let delta = if let Some(last) = self.last_pos {
                    Point::new(pos.x - last.x, pos.y - last.y)
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
    }
}

impl Default for PanGesture {
    fn default() -> Self {
        Self::new()
    }
}

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
                        Point::new(pos.x - last.x, pos.y - last.y)
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
                        Point::new(pos.x - start.x, pos.y - start.y)
                    } else {
                        Point::new(0, 0)
                    };
                    return Some(Event::Drag { pos: *pos, touch_id: *touch_id, delta });
                }
                // Before long press fired — check if movement exceeds threshold
                if let Some(start) = self.start_pos {
                    let dx = (pos.x - start.x).abs();
                    let dy = (pos.y - start.y).abs();
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
                // Check for long-press timeout on any event while holding
                if !self.long_press_fired && !self.dragging {
                    if let (Some(start_pos), Some(start)) = (self.start_pos, self.start_time) {
                        if now_ms >= start && now_ms - start >= LONG_PRESS_MIN_MS {
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

impl Default for LongPressDragGesture {
    fn default() -> Self {
        Self::new()
    }
}
