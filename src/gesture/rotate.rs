//! Rotate gesture recognizer.

use crate::core::Point;
use crate::event::Event;

use super::GestureRecognizer;

use super::pinch::PinchTouch;

/// Tracks two touch points and emits `Rotate` when the angle between
/// them changes.
#[derive(Debug, Clone)]
pub struct RotateGesture {
    touches: Vec<PinchTouch>,
    previous_angle: Option<f32>,
}

impl RotateGesture {
    pub fn new() -> Self {
        Self { touches: Vec::with_capacity(2), previous_angle: None }
    }

    fn angle_between(a: Point, b: Point) -> f32 {
        let dx = (b.x - a.x) as f32;
        let dy = (b.y - a.y) as f32;
        dy.atan2(dx)
    }
}

impl GestureRecognizer for RotateGesture {
    fn process(&mut self, event: &Event, _now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                if self.touches.len() < 2 {
                    self.touches.push(PinchTouch { pos: *pos, id: *touch_id });
                    if self.touches.len() == 2 {
                        self.previous_angle =
                            Some(Self::angle_between(self.touches[0].pos, self.touches[1].pos));
                    }
                }
                None
            }
            Event::TouchMove { pos, touch_id } => {
                if let Some(t) = self.touches.iter_mut().find(|t| t.id == *touch_id) {
                    t.pos = *pos;
                }
                if self.touches.len() == 2 {
                    let current_angle =
                        Self::angle_between(self.touches[0].pos, self.touches[1].pos);
                    if let Some(prev) = self.previous_angle {
                        let delta = current_angle - prev;
                        // Only emit if change is significant (> 0.05 rad ≈ 3°)
                        if delta.abs() > 0.05 {
                            self.previous_angle = Some(current_angle);
                            return Some(Event::Rotate { angle: delta });
                        }
                    }
                }
                None
            }
            Event::TouchEnd { touch_id, .. } => {
                self.touches.retain(|t| t.id != *touch_id);
                if self.touches.len() < 2 {
                    self.previous_angle = None;
                }
                None
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.touches.clear();
        self.previous_angle = None;
    }
}

impl Default for RotateGesture {
    fn default() -> Self {
        Self::new()
    }
}
