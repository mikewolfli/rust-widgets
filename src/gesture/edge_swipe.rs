// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Edge-swipe back gesture (EX4-06).
//!
//! # What it adds, and what it deliberately reuses
//!
//! A touch that starts inside a narrow edge band and travels inward is the platform convention for
//! "go back". The motion is an ordinary horizontal swipe, so this recogniser owns no new state
//! machine: it decides **whether the touch began at an edge** and then feeds the same
//! [`SwipeGesture`] every other swipe uses. Distance, velocity, axis locking and the reversal
//! cancellation are therefore identical to a normal swipe — the two cannot drift.
//!
//! # What the caller does with it
//!
//! The recogniser emits the ordinary [`Event::Swipe`], which keeps the event vocabulary unchanged.
//! The in-tree consumer is [`NavigationStack`], whose touch build exposes `handle_back_swipe` and
//! runs it from the `Event::Swipe` arm of `handle_event`: it pops when the swipe began in the
//! leading edge strip and travelled inward, so a host only has to route the event it already
//! receives. The library does not pop a stack it does not own, because a swipe can also mean
//! "dismiss this panel" and only the host knows which one is on top.
//!
//! [`NavigationStack`]: crate::widget::nav_widgets::navigation_stack::NavigationStack
//!
//! # Why the surface width is a constructor argument
//!
//! "Inside the right edge" needs the width of the surface being swiped, and a recogniser has no
//! access to the widget tree. Passing it once at construction keeps the check a subtraction instead
//! of a per-event lookup, and lets a resized surface re-create the recogniser with its new width.

use crate::core::{Point, Rect};
use crate::event::Event;
use crate::gesture::engine::{GestureKind, GestureRecognizer, GestureState};
use crate::gesture::swipe::SwipeGesture;

/// The edge a back swipe must start from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSide {
    /// The left edge; the inward direction is rightward.
    Left,
    /// The right edge; the inward direction is leftward.
    Right,
}

impl EdgeSide {
    /// Whether a swipe from this edge ends at the position the gesture intends.
    ///
    /// A left-edge back gesture travels right and a right-edge one travels left. A vertical or
    /// outward swipe is not a back gesture even when it started at an edge.
    fn accepts(self, start: Point, end: Point) -> bool {
        match self {
            Self::Left => end.x > start.x,
            Self::Right => end.x < start.x,
        }
    }
}

/// The default width of the edge band, in logical pixels.
///
/// Sixteen is the band Android's predictive-back guidance uses as a starting point; a host that
/// needs a wider target for a specific device calls [`EdgeSwipeRecognizer::with_edge_width`].
pub const DEFAULT_EDGE_WIDTH: i32 = 16;

/// Recognises an inward swipe that began inside a surface's left or right edge band.
#[derive(Debug, Clone)]
pub struct EdgeSwipeRecognizer {
    inner: SwipeGesture,
    edge: EdgeSide,
    surface: Rect,
    edge_width: i32,
    /// The touch that started inside the band, while it is still in flight.
    touch_id: Option<u64>,
}

impl EdgeSwipeRecognizer {
    /// Creates a recogniser for `edge` of `surface`.
    pub fn new(edge: EdgeSide, surface: Rect) -> Self {
        Self::with_edge_width(edge, surface, DEFAULT_EDGE_WIDTH)
    }

    /// Creates a recogniser with an explicit edge-band width, clamped to at least one pixel.
    pub fn with_edge_width(edge: EdgeSide, surface: Rect, edge_width: i32) -> Self {
        Self {
            inner: SwipeGesture::new(),
            edge,
            surface,
            edge_width: edge_width.max(1),
            touch_id: None,
        }
    }

    /// Updates the surface bounds after a resize or a rotation.
    pub fn set_surface(&mut self, surface: Rect) {
        if self.surface != surface {
            self.surface = surface;
            self.reset();
        }
    }

    /// The edge this recogniser watches.
    pub fn edge(&self) -> EdgeSide {
        self.edge
    }

    /// Whether `pos` is inside the configured edge band.
    pub fn is_in_edge_band(&self, pos: Point) -> bool {
        let left = self.surface.x;
        let right = self.surface.x + self.surface.width as i32;
        match self.edge {
            EdgeSide::Left => pos.x >= left && pos.x < left + self.edge_width,
            EdgeSide::Right => pos.x < right && pos.x >= right - self.edge_width,
        }
    }
}

impl GestureRecognizer for EdgeSwipeRecognizer {
    fn kind(&self) -> GestureKind {
        GestureKind::Custom("edge_swipe")
    }

    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event> {
        match event {
            Event::TouchBegin { pos, touch_id } => {
                if !self.is_in_edge_band(*pos) {
                    self.touch_id = None;
                    self.inner.reset();
                    return None;
                }
                self.touch_id = Some(*touch_id);
                self.inner.process(event, now_ms);
                None
            }
            Event::TouchMove { touch_id, .. } | Event::TouchEnd { touch_id, .. } => {
                if self.touch_id != Some(*touch_id) {
                    return None;
                }
                let result = self.inner.process(event, now_ms);
                if let Some(Event::Swipe { start, end, velocity }) = result {
                    if self.edge.accepts(start, end) {
                        self.touch_id = None;
                        return Some(Event::Swipe { start, end, velocity });
                    }
                    self.touch_id = None;
                    return None;
                }
                if matches!(event, Event::TouchEnd { .. }) {
                    // The inner recogniser rejected the motion (too short, a reversal, or below
                    // the velocity floor); the contact is over either way.
                    self.touch_id = None;
                }
                None
            }
            _ => None,
        }
    }

    fn status(&self) -> GestureState {
        // The inner recogniser has no phase API; "tracking an edge contact" is the one fact this
        // layer owns, and it is enough for a host that paints a predictive-back preview.
        if self.touch_id.is_some() {
            GestureState::Maybe
        } else {
            GestureState::Finished
        }
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.touch_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    fn surface() -> Rect {
        Rect::new(0, 0, 400, 800)
    }

    fn touch(event: Event) -> Event {
        event
    }

    #[test]
    fn an_inward_swipe_from_the_left_edge_is_a_back_gesture() {
        let mut recognizer = EdgeSwipeRecognizer::new(EdgeSide::Left, surface());
        assert!(recognizer
            .process(&touch(Event::TouchBegin { pos: Point::new(5, 400), touch_id: 1 }), 0)
            .is_none());
        assert!(recognizer
            .process(&touch(Event::TouchMove { pos: Point::new(60, 400), touch_id: 1 }), 20)
            .is_none());
        let emitted = recognizer
            .process(&touch(Event::TouchEnd { pos: Point::new(120, 400), touch_id: 1 }), 40);
        assert!(matches!(emitted, Some(Event::Swipe { .. })));
    }

    #[test]
    fn a_swipe_that_does_not_start_at_the_edge_is_ignored() {
        let mut recognizer = EdgeSwipeRecognizer::new(EdgeSide::Left, surface());
        assert!(recognizer
            .process(&touch(Event::TouchBegin { pos: Point::new(200, 400), touch_id: 2 }), 0)
            .is_none());
        assert!(recognizer
            .process(&touch(Event::TouchMove { pos: Point::new(260, 400), touch_id: 2 }), 20)
            .is_none());
        assert!(recognizer
            .process(&touch(Event::TouchEnd { pos: Point::new(320, 400), touch_id: 2 }), 40)
            .is_none());
    }

    #[test]
    fn a_swipe_outward_from_the_edge_is_not_a_back_gesture() {
        let mut recognizer = EdgeSwipeRecognizer::new(EdgeSide::Right, surface());
        assert!(recognizer
            .process(&touch(Event::TouchBegin { pos: Point::new(395, 400), touch_id: 3 }), 0)
            .is_none());
        assert!(recognizer
            .process(&touch(Event::TouchMove { pos: Point::new(340, 400), touch_id: 3 }), 20)
            .is_none());
        // Outward means rightward on the right edge, which this recogniser must refuse.
        assert!(recognizer
            .process(&touch(Event::TouchEnd { pos: Point::new(399, 400), touch_id: 3 }), 40)
            .is_none());
    }

    #[test]
    fn the_edge_band_tracks_the_surface_it_was_given() {
        let mut recognizer = EdgeSwipeRecognizer::new(EdgeSide::Right, surface());
        assert!(recognizer.is_in_edge_band(Point::new(395, 10)));
        assert!(!recognizer.is_in_edge_band(Point::new(300, 10)));
        recognizer.set_surface(Rect::new(0, 0, 800, 800));
        assert!(!recognizer.is_in_edge_band(Point::new(395, 10)));
        assert!(recognizer.is_in_edge_band(Point::new(795, 10)));
    }
}
