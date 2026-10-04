// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pinch gesture recognizer.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{distance, GestureRecognizer};

/// Tracks two touch points and emits `Pinch` when the distance between
/// them changes significantly.
#[derive(Debug, Clone)]
pub struct PinchTouch {
    /// Latest position of this touch in logical pixels, screen coordinates.
    pub pos: Point,
    /// Identifier of the tracked touch, used to match move/end events to it.
    pub id: TouchId,
}

/// Recognizes a two-finger pinch gesture and emits `Event::Pinch { scale }`.
///
/// Tracks two touch points and computes a scale factor relative to the
/// initial distance between them. Only emits when the scale change exceeds 5%.
#[derive(Debug, Clone)]
pub struct PinchGesture {
    touches: Vec<PinchTouch>,
    initial_distance: Option<f32>,
}

impl PinchGesture {
    /// Creates a recognizer with no tracked touches and no baseline distance.
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
                    // A non-finite separation (defensive: `distance` is finite for
                    // integer coordinates, but a poisoned baseline must never reach a
                    // scale consumer) cannot produce a ratio. Drop the baseline so a
                    // later finite separation re-anchors instead of dividing by it.
                    if !current.is_finite() {
                        self.initial_distance = None;
                        return None;
                    }
                    match self.initial_distance {
                        // First measurement after both fingers are down: anchor and
                        // wait for a real change.
                        None => {
                            self.initial_distance = Some(current);
                        }
                        Some(initial) => {
                            // Degenerate baseline: the fingers landed coincident (or a
                            // previous collapse stored a zero). A ratio against zero is
                            // undefined, so re-anchor once the fingers separate and
                            // never divide by it.
                            if !initial.is_finite() || initial <= 0.0 {
                                if current > 0.0 {
                                    self.initial_distance = Some(current);
                                }
                                return None;
                            }
                            // Fingers collapsed onto one another. A scale of zero is a
                            // valid ratio (fully pinched together), but storing the zero
                            // distance as the new baseline would make every later move
                            // divide by zero, so keep the last usable baseline.
                            if current <= 0.0 {
                                return Some(Event::Pinch { scale: 0.0 });
                            }
                            let scale = current / initial;
                            if !scale.is_finite() {
                                self.initial_distance = Some(current);
                                return None;
                            }
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

crate::impl_default_via_new!(PinchGesture);

#[cfg(test)]
mod tests {
    use super::*;

    /// S-28: two fingers that land on the same point must still be able to
    /// produce a pinch once they separate. The zero baseline must re-anchor
    /// rather than poison every later move.
    #[test]
    fn coincident_start_recovers_after_separation() {
        let mut gesture = PinchGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 2 }, 0)
            .is_none());

        // First separation re-anchors the zero baseline: no ratio is defined yet.
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(10, 0), touch_id: 2 }, 10)
            .is_none());
        // The second separation must now report the spread relative to the anchor.
        let produced =
            gesture.process(&Event::TouchMove { pos: Point::new(20, 0), touch_id: 2 }, 20);
        match produced {
            Some(Event::Pinch { scale }) => assert!(
                scale > 1.0,
                "separating after a coincident start must zoom in, got scale {scale}"
            ),
            other => panic!("a coincident start must recover into a pinch, got {other:?}"),
        }
    }

    /// S-28: a mid-gesture collapse (distance -> 0) followed by separation must
    /// still produce a pinch, and the collapse itself is a usable zero ratio
    /// rather than a poisoned baseline.
    #[test]
    fn mid_gesture_collapse_then_separate_recovers() {
        let mut gesture = PinchGesture::new();
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0)
            .is_none());
        assert!(gesture
            .process(&Event::TouchBegin { pos: Point::new(10, 0), touch_id: 2 }, 0)
            .is_none());

        // Collapse the fingers together: a zero scale, but the baseline survives.
        let collapsed =
            gesture.process(&Event::TouchMove { pos: Point::new(0, 0), touch_id: 2 }, 10);
        match collapsed {
            Some(Event::Pinch { scale }) => assert_eq!(scale, 0.0),
            other => panic!("collapsing fingers must emit a zero scale, got {other:?}"),
        }

        // Separating again must produce a pinch against the pre-collapse baseline.
        let produced =
            gesture.process(&Event::TouchMove { pos: Point::new(20, 0), touch_id: 2 }, 20);
        match produced {
            Some(Event::Pinch { scale }) => {
                assert!(scale > 1.0, "separating after a collapse must zoom in, got scale {scale}")
            }
            other => panic!("a collapse must not poison later separation, got {other:?}"),
        }
    }
}
