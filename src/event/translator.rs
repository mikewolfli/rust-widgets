// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Touch-to-mouse event translator.
//!
//! Converts touch events (`TouchBegin`, `TouchMove`, `TouchEnd`) into
//! equivalent mouse events (`MousePress`, `MouseMove`, `MouseRelease`)
//! so that existing mouse-only widgets can respond to touch input
//! without modification.
//!
//! This is a **non-destructive bridge** — touch events pass through
//! and are also translated to mouse events. Widgets that natively
//! handle touch receive the original touch events; legacy widgets
//! receive the synthesized mouse events.

use crate::compat::HashMap;
use crate::event::types::{mouse_button, Event, TouchId};

/// Touch-to-mouse event translator.
///
/// Tracks active touch points and synthesizes equivalent mouse events:
///
/// | Touch Event | Synthesized Mouse Event            |
/// |-------------|-------------------------------------|
/// | `TouchBegin` | `MousePress` (primary) + `MouseEnter` |
/// | `TouchMove` | `MouseMove`, if the touch is active |
/// | `TouchEnd` | `MouseRelease` (primary) + `MouseLeave`, if the touch is active |
///
/// A move or end whose touch was never begun (or was already ended) synthesizes
/// nothing, so the synthesized press/release stay paired.
///
/// # Example
///
/// ```rust
/// use rust_widgets::core::Point;
/// use rust_widgets::event::translator::TouchEventTranslator;
/// use rust_widgets::event::Event;
///
/// let mut translator = TouchEventTranslator::new();
/// let touch = Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 };
/// let mouse_events: Vec<Event> = translator.translate(&touch);
/// assert!(!mouse_events.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct TouchEventTranslator {
    /// Maps active touch IDs to their last known position.
    active_touches: HashMap<TouchId, (/* x */ i32, /* y */ i32)>,
    /// Whether touch-to-mouse translation is enabled.
    enabled: bool,
}

impl TouchEventTranslator {
    /// Creates a new translator with translation enabled.
    pub fn new() -> Self {
        Self { active_touches: HashMap::new(), enabled: true }
    }

    /// Creates a disabled translator (passthrough, no synthesis).
    pub fn disabled() -> Self {
        Self { active_touches: HashMap::new(), enabled: false }
    }

    /// Enable or disable translation.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns whether translation is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Translate a single event into zero or more synthesized mouse events.
    ///
    /// Returns a vector of synthesized events (may be empty if the input
    /// is not a touch event or if translation is disabled).
    pub fn translate(&mut self, event: &Event) -> Vec<Event> {
        if !self.enabled {
            return Vec::new();
        }

        match *event {
            Event::TouchBegin { pos, touch_id } => {
                self.active_touches.insert(touch_id, (pos.x, pos.y));
                // The mouse bridge reports the primary button: a touch is a left-click as far as a
                // mouse-only consumer is concerned. Emitting button `0` made every consumer that
                // checks `button == mouse_button::PRIMARY` (`1`) reject the synthesized press.
                vec![
                    Event::MousePress { pos, button: mouse_button::PRIMARY, modifiers: 0 },
                    Event::MouseEnter { pos },
                ]
            }
            Event::TouchMove { pos, touch_id } => {
                if let Some(pos_ref) = self.active_touches.get_mut(&touch_id) {
                    *pos_ref = (pos.x, pos.y);
                    vec![Event::MouseMove { pos }]
                } else {
                    Vec::new()
                }
            }
            Event::TouchEnd { pos, touch_id } => {
                // Only a touch that was actually begun can end. An unknown end (a stray event, or
                // a repeat after the first consumed the id) used to synthesize a release and a
                // leave for a press that never happened; unknown `TouchMove` is already ignored,
                // so this restores the symmetry.
                if self.active_touches.remove(&touch_id).is_none() {
                    return Vec::new();
                }
                vec![
                    Event::MouseRelease { pos, button: mouse_button::PRIMARY },
                    Event::MouseLeave { pos },
                ]
            }
            _ => Vec::new(),
        }
    }

    /// Clear all tracked touch points.
    pub fn clear(&mut self) {
        self.active_touches.clear();
    }

    /// Number of currently tracked touch points.
    pub fn active_touch_count(&self) -> usize {
        self.active_touches.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn touch_begin_generates_mouse_press_and_enter() {
        let mut t = TouchEventTranslator::new();
        let ev = Event::TouchBegin { pos: Point::new(10, 20), touch_id: 1 };
        let result = t.translate(&ev);
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], Event::MousePress { .. }));
        assert!(matches!(result[1], Event::MouseEnter { .. }));
        assert_eq!(t.active_touch_count(), 1);
    }

    /// The synthesized press must carry the primary button, not `0`.
    ///
    /// A touch is a left-click to a mouse-only consumer, and those consumers compare against
    /// `mouse_button::PRIMARY` (`1`); the bridge emitted `0`, which every such widget rejected.
    #[test]
    fn touch_begin_press_carries_the_primary_button() {
        let mut t = TouchEventTranslator::new();
        let ev = Event::TouchBegin { pos: Point::new(3, 4), touch_id: 1 };
        let result = t.translate(&ev);
        match result[0] {
            Event::MousePress { pos, button, .. } => {
                assert_eq!(pos, Point::new(3, 4));
                assert_eq!(button, crate::event::types::mouse_button::PRIMARY);
            }
            _ => panic!("expected a synthesized MousePress"),
        }
    }

    /// The synthesized release must carry the primary button, not `0`.
    #[test]
    fn touch_end_release_carries_the_primary_button() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 });
        let result = t.translate(&Event::TouchEnd { pos: Point::new(9, 8), touch_id: 1 });
        match result[0] {
            Event::MouseRelease { pos, button } => {
                assert_eq!(pos, Point::new(9, 8));
                assert_eq!(button, crate::event::types::mouse_button::PRIMARY);
            }
            _ => panic!("expected a synthesized MouseRelease"),
        }
    }

    /// An end for a touch that never began synthesizes nothing.
    #[test]
    fn touch_end_without_begin_is_ignored() {
        let mut t = TouchEventTranslator::new();
        let ev = Event::TouchEnd { pos: Point::new(10, 20), touch_id: 42 };
        assert!(
            t.translate(&ev).is_empty(),
            "an end without a matching begin must not synthesize a release"
        );
        assert_eq!(t.active_touch_count(), 0);
    }

    /// A repeated end synthesizes nothing the second time.
    #[test]
    fn repeated_touch_end_is_ignored() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 });
        let first = t.translate(&Event::TouchEnd { pos: Point::new(5, 5), touch_id: 1 });
        assert_eq!(first.len(), 2, "the first end is the real one");

        let second = t.translate(&Event::TouchEnd { pos: Point::new(5, 5), touch_id: 1 });
        assert!(second.is_empty(), "a repeated end must not synthesize a second release");
    }

    /// A normal begin/end pair still produces exactly one press and one release.
    #[test]
    fn normal_begin_end_pairing_is_unchanged() {
        let mut t = TouchEventTranslator::new();
        let begin = t.translate(&Event::TouchBegin { pos: Point::new(1, 1), touch_id: 7 });
        assert_eq!(begin.len(), 2);
        let end = t.translate(&Event::TouchEnd { pos: Point::new(2, 2), touch_id: 7 });
        assert_eq!(end.len(), 2);
        assert_eq!(t.active_touch_count(), 0);
    }

    /// Multi-touch: ending one touch must not consume another's tracking.
    #[test]
    fn multi_touch_ends_are_independent() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(1, 1), touch_id: 1 });
        t.translate(&Event::TouchBegin { pos: Point::new(2, 2), touch_id: 2 });

        let end_one = t.translate(&Event::TouchEnd { pos: Point::new(1, 1), touch_id: 1 });
        assert_eq!(end_one.len(), 2, "touch 1's end is a real end");
        assert_eq!(t.active_touch_count(), 1, "touch 2 is still active");

        // A repeat of touch 1's end is ignored even though touch 2 remains active.
        assert!(t.translate(&Event::TouchEnd { pos: Point::new(1, 1), touch_id: 1 }).is_empty());

        let end_two = t.translate(&Event::TouchEnd { pos: Point::new(2, 2), touch_id: 2 });
        assert_eq!(end_two.len(), 2);
        assert_eq!(t.active_touch_count(), 0);
    }

    /// A disabled translator tracks no touches, so a later end synthesizes nothing;
    /// re-enabling restores full synthesis.
    #[test]
    fn disabled_then_reenabled_translation() {
        let mut t = TouchEventTranslator::disabled();
        // While disabled, no touch is tracked.
        assert!(t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 }).is_empty());
        assert_eq!(t.active_touch_count(), 0);

        // Re-enabled: a begin/end pair is translated normally.
        t.set_enabled(true);
        assert!(t.is_enabled());
        let begin = t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 });
        assert_eq!(begin.len(), 2);
        let end = t.translate(&Event::TouchEnd { pos: Point::new(1, 1), touch_id: 1 });
        assert_eq!(end.len(), 2);
    }

    #[test]
    fn touch_move_generates_mouse_move() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 });
        let ev = Event::TouchMove { pos: Point::new(10, 20), touch_id: 1 };
        let result = t.translate(&ev);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Event::MouseMove { .. }));
    }

    #[test]
    fn touch_end_generates_mouse_release_and_leave() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 });
        let ev = Event::TouchEnd { pos: Point::new(10, 20), touch_id: 1 };
        let result = t.translate(&ev);
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], Event::MouseRelease { .. }));
        assert!(matches!(result[1], Event::MouseLeave { .. }));
        assert_eq!(t.active_touch_count(), 0);
    }

    #[test]
    fn unknown_touch_id_ignored() {
        let mut t = TouchEventTranslator::new();
        let ev = Event::TouchMove { pos: Point::new(10, 20), touch_id: 999 };
        let result = t.translate(&ev);
        assert!(result.is_empty());
    }

    #[test]
    fn disabled_translator_produces_no_events() {
        let mut t = TouchEventTranslator::disabled();
        let ev = Event::TouchBegin { pos: Point::new(10, 20), touch_id: 1 };
        let result = t.translate(&ev);
        assert!(result.is_empty());
    }

    #[test]
    fn non_touch_events_produce_no_translation() {
        let mut t = TouchEventTranslator::new();
        let ev = Event::MousePress { pos: Point::new(10, 20), button: 0, modifiers: 0 };
        let result = t.translate(&ev);
        assert!(result.is_empty());
    }

    #[test]
    fn clear_removes_all_tracked_touches() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 1 });
        t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: 2 });
        assert_eq!(t.active_touch_count(), 2);
        t.clear();
        assert_eq!(t.active_touch_count(), 0);
    }

    #[test]
    fn multiple_touches_tracked_independently() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(10, 20), touch_id: 1 });
        t.translate(&Event::TouchBegin { pos: Point::new(30, 40), touch_id: 2 });
        assert_eq!(t.active_touch_count(), 2);

        let r1 = t.translate(&Event::TouchMove { pos: Point::new(15, 25), touch_id: 1 });
        assert_eq!(r1.len(), 1);

        let r2 = t.translate(&Event::TouchEnd { pos: Point::new(35, 45), touch_id: 2 });
        assert_eq!(r2.len(), 2);
        assert_eq!(t.active_touch_count(), 1);
    }
}
