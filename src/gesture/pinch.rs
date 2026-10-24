// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pinch gesture recognizer.

use crate::core::Point;
use crate::event::{Event, TouchId};

use super::{distance, GestureKind, GestureRecognizer, GestureState};

/// Tracks two touch points and emits `Pinch` when the distance between
/// them changes significantly.
#[derive(Debug, Clone)]
pub struct PinchTouch {
    /// Latest position of this touch in logical pixels, screen coordinates.
    pub pos: Point,
    /// Identifier of the tracked touch, used to match move/end events to it.
    pub id: TouchId,
}

/// Recognizes a two-finger pinch gesture and emits `Event::Pinch { scale, center }`.
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
                                return Some(Event::Pinch { scale: 0.0, center: self.center() });
                            }
                            let scale = current / initial;
                            if !scale.is_finite() {
                                self.initial_distance = Some(current);
                                return None;
                            }
                            // Only emit if scale differs significantly
                            if (scale - 1.0).abs() > 0.05 {
                                self.initial_distance = Some(current); // Update baseline
                                return Some(Event::Pinch { scale, center: self.center() });
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

    fn kind(&self) -> GestureKind {
        GestureKind::Pinch
    }

    fn status(&self) -> GestureState {
        if self.touches.len() == 2 {
            GestureState::Maybe
        } else {
            GestureState::Finished
        }
    }

    fn partial(&self) -> Option<Event> {
        // A pinch is describable before it is committed: report the scale against the current
        // baseline and the centre of the two touches. Returning `None` with fewer than two
        // contacts is the honest answer — there is no midpoint to name.
        if self.touches.len() != 2 {
            return None;
        }
        let current = distance(self.touches[0].pos, self.touches[1].pos);
        let scale = match self.initial_distance {
            Some(initial) if initial > 0.0 => current / initial,
            _ => 1.0,
        };
        Some(Event::Pinch { scale, center: self.center() })
    }
}

impl PinchGesture {
    /// The midpoint between the two tracked touches.
    ///
    /// The origin when fewer than two contacts are tracked: there is no midpoint, and the
    /// recogniser only emits a pinch with two contacts, so this is the defensive answer rather
    /// than a value a consumer can observe on a real gesture.
    pub fn center(&self) -> Point {
        match self.touches.as_slice() {
            [first, second, ..] => {
                Point::new((first.pos.x + second.pos.x) / 2, (first.pos.y + second.pos.y) / 2)
            }
            _ => Point::new(0, 0),
        }
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
            Some(Event::Pinch { scale, .. }) => assert!(
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
            Some(Event::Pinch { scale, .. }) => assert_eq!(scale, 0.0),
            other => panic!("collapsing fingers must emit a zero scale, got {other:?}"),
        }

        // Separating again must produce a pinch against the pre-collapse baseline.
        let produced =
            gesture.process(&Event::TouchMove { pos: Point::new(20, 0), touch_id: 2 }, 20);
        match produced {
            Some(Event::Pinch { scale, .. }) => {
                assert!(scale > 1.0, "separating after a collapse must zoom in, got scale {scale}")
            }
            other => panic!("a collapse must not poison later separation, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod pinch_center_tests {
    use super::*;

    /// The emitted pinch carries the midpoint of the two touches, which is the anchor a
    /// consumer zooms around.
    #[test]
    fn the_emitted_event_carries_the_touch_midpoint() {
        let mut gesture = PinchGesture::new();
        gesture.process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0);
        gesture.process(&Event::TouchBegin { pos: Point::new(10, 0), touch_id: 2 }, 0);
        let produced =
            gesture.process(&Event::TouchMove { pos: Point::new(30, 0), touch_id: 2 }, 1);
        match produced {
            Some(Event::Pinch { center, .. }) => {
                assert_eq!(center, Point::new(15, 0), "the midpoint moved with the second finger")
            }
            other => panic!("expected a pinch, got {other:?}"),
        }
    }

    /// A pinch in progress is previewable before it crosses the commit threshold.
    #[test]
    fn a_partial_preview_is_available_before_commit() {
        let mut gesture = PinchGesture::new();
        assert!(gesture.partial().is_none(), "no contacts, no midpoint");
        gesture.process(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }, 0);
        gesture.process(&Event::TouchBegin { pos: Point::new(10, 0), touch_id: 2 }, 0);
        // Below the 5% emit threshold: `process` stays silent.
        assert!(gesture
            .process(&Event::TouchMove { pos: Point::new(10, 0), touch_id: 2 }, 1)
            .is_none());
        match gesture.partial() {
            Some(Event::Pinch { scale, center }) => {
                assert_eq!(scale, 1.0, "the baseline is unchanged");
                assert_eq!(center, Point::new(5, 0));
            }
            other => panic!("expected a preview, got {other:?}"),
        }
        assert_eq!(gesture.status(), GestureState::Maybe);
    }
}
