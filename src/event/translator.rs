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
use crate::event::types::{Event, TouchId};

/// Touch-to-mouse event translator.
///
/// Tracks active touch points and synthesizes equivalent mouse events:
///
/// | Touch Event | Synthesized Mouse Event |
/// |-------------|------------------------|
/// | `TouchBegin` | `MousePress` + `MouseEnter` |
/// | `TouchMove` | `MouseMove` |
/// | `TouchEnd` | `MouseRelease` + `MouseLeave` |
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
                vec![Event::MousePress { pos, button: 0 }, Event::MouseEnter { pos }]
            }
            Event::TouchMove { pos, touch_id } => {
                if let std::collections::hash_map::Entry::Occupied(mut e) =
                    self.active_touches.entry(touch_id)
                {
                    e.insert((pos.x, pos.y));
                    vec![Event::MouseMove { pos }]
                } else {
                    Vec::new()
                }
            }
            Event::TouchEnd { pos, touch_id } => {
                self.active_touches.remove(&touch_id);
                vec![Event::MouseRelease { pos, button: 0 }, Event::MouseLeave { pos }]
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
        let ev = Event::MousePress { pos: Point::new(10, 20), button: 0 };
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
