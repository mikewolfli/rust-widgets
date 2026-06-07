//! Pinch gesture recognizer.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{distance, GestureRecognizer};

/// Tracks two touch points and emits `Pinch` when the distance between
/// them changes significantly.
#[derive(Debug, Clone)]
pub struct PinchTouch {
    pub pos: Point,
    pub id: TouchId,
}

#[derive(Debug, Clone)]
pub struct PinchGesture {
    touches: Vec<PinchTouch>,
    initial_distance: Option<f32>,
}

impl PinchGesture {
    pub fn new() -> Self {
        Self { touches: Vec::with_capacity(2), initial_distance: None }
    }
}

impl GestureRecognizer for PinchGesture {
    fn process(&mut self, event: &Event, _now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                if self.touches.len() < 2 {
                    self.touches.push(PinchTouch { pos: *pos, id: *touch_id });
                    if self.touches.len() == 2 {
                        self.initial_distance =
                            Some(distance(self.touches[0].pos, self.touches[1].pos));
                    }
                }
                None
            }
            Event::TouchMove { pos, touch_id } => {
                // Update matching touch
                if let Some(t) = self.touches.iter_mut().find(|t| t.id == *touch_id) {
                    t.pos = *pos;
                }
                if self.touches.len() == 2 {
                    let current = distance(self.touches[0].pos, self.touches[1].pos);
                    if let Some(initial) = self.initial_distance {
                        if initial > 0.0 {
                            let scale = current / initial;
                            // Only emit if scale differs significantly
                            if (scale - 1.0).abs() > 0.05 {
                                self.initial_distance = Some(current); // Update baseline
                                return Some(Event::Pinch { scale });
                            }
                        }
                    }
                }
                None
            }
            Event::TouchEnd { touch_id, .. } => {
                self.touches.retain(|t| t.id != *touch_id);
                if self.touches.len() < 2 {
                    self.initial_distance = None;
                }
                None
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.touches.clear();
        self.initial_distance = None;
    }
}

impl Default for PinchGesture {
    fn default() -> Self {
        Self::new()
    }
}
