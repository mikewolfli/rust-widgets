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
//!
//! # Touch cancel (D09-EVT-02)
//!
//! A platform `TouchCancel` is a withdrawn contact, not a completed one. It is represented by
//! an internal event ([`TOUCH_CANCEL_EVENT_NAME`], built by [`touch_cancel`]) which this
//! translator consumes by dropping the tracked touch and synthesizing **nothing** — an
//! activating `MouseRelease` would turn a cancellation into a click. Touch-aware controls
//! receive the cancel event and reset their latch without activating.

use crate::compat::{HashMap, MiniToString, Vec};
use crate::event::types::{mouse_button, Event, TouchId};

/// Canonical name of the internal touch-cancel event (D09-EVT-02).
///
/// # Why a name and not a public `Event` variant
///
/// A platform `TouchCancel` was normalised into a normal `TouchEnd` by every backend, so a
/// consumer could not tell a withdrawn contact from a completed one and committed the
/// cancelled gesture (button click, switch toggle, signature commit). The distinct signal is
/// carried as a [`Event::Custom`] event with this canonical name rather than as a new public
/// `Event` variant: `Custom` is the crate's existing escape hatch for a framework-internal
/// signal (compare `crate::event::r#loop::ANIMATION_FRAME_EVENT_NAME`), so adding it keeps the
/// public `Event` enum — and the "every variant has a producer" contract — unchanged while
/// the routing to consumers is fully typed through the helpers below.
///
/// A caller that recognises a platform cancel posts this event to the affected target;
/// [`TouchEventTranslator::translate`] consumes it (dropping the tracked touch without
/// synthesising an activating release) and the touch-aware controls reset their latch
/// without firing `clicked`/`toggled`/a stroke commit.
pub const TOUCH_CANCEL_EVENT_NAME: &str = "touch_cancel";

/// Builds the internal touch-cancel event for `touch_id` at `pos` (D09-EVT-02).
///
/// The payload is `touch_id` (`u64`) then the position (`x`, `y` as `i32`), little-endian,
/// kept in one place so the constructor and [`touch_cancel_payload`] cannot disagree about
/// the layout.
pub fn touch_cancel(pos: crate::core::Point, touch_id: TouchId) -> Event {
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&touch_id.to_le_bytes());
    payload.extend_from_slice(&pos.x.to_le_bytes());
    payload.extend_from_slice(&pos.y.to_le_bytes());
    Event::Custom { name: TOUCH_CANCEL_EVENT_NAME.to_string(), payload }
}

/// Whether `event` is the internal touch-cancel event (D09-EVT-02).
pub fn is_touch_cancel(event: &Event) -> bool {
    matches!(event, Event::Custom { name, .. } if name == TOUCH_CANCEL_EVENT_NAME)
}

/// Decodes the `(pos, touch_id)` carried by a touch-cancel event, or `None` for any other
/// event. The inverse of [`touch_cancel`].
pub fn touch_cancel_payload(event: &Event) -> Option<(crate::core::Point, TouchId)> {
    let Event::Custom { name, payload } = event else {
        return None;
    };
    if name != TOUCH_CANCEL_EVENT_NAME {
        return None;
    }
    if payload.len() < 16 {
        return None;
    }
    let touch_id = TouchId::from_le_bytes(payload[0..8].try_into().ok()?);
    let x = i32::from_le_bytes(payload[8..12].try_into().ok()?);
    let y = i32::from_le_bytes(payload[12..16].try_into().ok()?);
    Some((crate::core::Point::new(x, y), touch_id))
}

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
    ///
    /// # D09-EVT-07: disabling clears the tracked touch points
    ///
    /// `translate` returns early while disabled, so a `TouchEnd` arriving in that window
    /// was never consumed and its `active_touches` entry survived. Re-enabling then let a
    /// late or repeated end for that id match the stale entry and synthesize a
    /// `MouseRelease`/`MouseLeave` for a cycle that had no matching `MousePress` — an
    /// orphan release. Clearing on every disable makes each enabled cycle start from a
    /// known-empty set, so a release can only be produced for a begin seen in the *same*
    /// cycle. A no-op transition (`set_enabled` to the current value) still clears, which
    /// is what makes "disable to take input over" safe even if it is called twice.
    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.active_touches.clear();
        }
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
            _ if is_touch_cancel(event) => {
                // D09-EVT-02: a cancelled contact is **not** a completed one. Drop the tracked
                // touch so a later end for the same id cannot pair with it, and synthesize
                // nothing: a `MouseRelease` would let a mouse-only consumer treat the withdrawn
                // contact as a click. The touch-aware consumer resets its own latch from the
                // cancel event itself, so nothing here needs to activate or commit.
                if let Some((_, touch_id)) = touch_cancel_payload(event) {
                    self.active_touches.remove(&touch_id);
                } else {
                    // A malformed cancel carries no id; the safe reading is "some contact was
                    // withdrawn", so clear the tracked set rather than leave a stale entry.
                    self.active_touches.clear();
                }
                Vec::new()
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

    /// D09-EVT-02: a touch cancel drops the tracked touch and synthesizes no activating
    /// release, and the tracking count returns to zero.
    #[test]
    fn touch_cancel_drops_the_touch_without_synthesizing_a_release() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(10, 10), touch_id: 1 });
        assert_eq!(t.active_touch_count(), 1);

        let cancel = touch_cancel(Point::new(10, 10), 1);
        assert!(is_touch_cancel(&cancel));
        let out = t.translate(&cancel);
        assert!(out.is_empty(), "a cancel must not synthesize an activating mouse release");
        assert_eq!(t.active_touch_count(), 0, "the cancelled touch must be dropped");

        // A late end for the cancelled id pairs with nothing.
        assert!(
            t.translate(&Event::TouchEnd { pos: Point::new(10, 10), touch_id: 1 }).is_empty(),
            "an end after a cancel must not synthesize a release"
        );
    }

    /// D09-EVT-02: the cancel payload round-trips through the codec.
    #[test]
    fn touch_cancel_payload_round_trips() {
        let pos = Point::new(-12, 345);
        let cancel = touch_cancel(pos, 42);
        assert_eq!(touch_cancel_payload(&cancel), Some((pos, 42)));
        // A non-cancel event decodes to `None`.
        assert_eq!(touch_cancel_payload(&Event::TouchBegin { pos, touch_id: 1 }), None);
        // A malformed cancel payload decodes to `None` rather than panicking.
        assert_eq!(
            touch_cancel_payload(&Event::Custom {
                name: TOUCH_CANCEL_EVENT_NAME.to_string(),
                payload: alloc::vec![1, 2, 3],
            }),
            None
        );
    }

    /// D09-EVT-02: a cancel for one of several touches drops only that touch.
    #[test]
    fn touch_cancel_of_one_id_keeps_the_others() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(1, 1), touch_id: 1 });
        t.translate(&Event::TouchBegin { pos: Point::new(2, 2), touch_id: 2 });
        assert_eq!(t.active_touch_count(), 2);

        let out = t.translate(&touch_cancel(Point::new(1, 1), 1));
        assert!(out.is_empty());
        assert_eq!(t.active_touch_count(), 1, "only the cancelled touch is dropped");

        // Touch 2 still ends normally.
        let end = t.translate(&Event::TouchEnd { pos: Point::new(2, 2), touch_id: 2 });
        assert_eq!(end.len(), 2);
        assert_eq!(t.active_touch_count(), 0);
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

    /// D09-EVT-07: disabling mid-touch and re-enabling must not let a late/repeated end
    /// synthesize an orphan release for a begin the current cycle never saw.
    #[test]
    fn disabling_clears_active_touches_so_reenable_produces_no_orphan_release() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(5, 5), touch_id: 1 });
        assert_eq!(t.active_touch_count(), 1);

        // The input layer takes over mid-touch.
        t.set_enabled(false);
        assert_eq!(
            t.active_touch_count(),
            0,
            "disabling must drop the tracked touch, or a stale end can pair with it later"
        );

        // While disabled the terminating end is ignored (translation is off)...
        assert!(t.translate(&Event::TouchEnd { pos: Point::new(5, 5), touch_id: 1 }).is_empty());

        // ...and after re-enabling, a late/repeated end for the old id must synthesize nothing,
        // because no press was emitted in this enabled cycle.
        t.set_enabled(true);
        let late = t.translate(&Event::TouchEnd { pos: Point::new(5, 5), touch_id: 1 });
        assert!(
            late.is_empty(),
            "a repeated end after re-enable must not produce an unmatched release"
        );
        assert_eq!(t.active_touch_count(), 0);

        // A genuine begin/end in the new cycle still pairs correctly.
        assert_eq!(t.translate(&Event::TouchBegin { pos: Point::new(7, 7), touch_id: 1 }).len(), 2);
        assert_eq!(t.translate(&Event::TouchEnd { pos: Point::new(8, 8), touch_id: 1 }).len(), 2);
    }

    /// D09-EVT-07: the same guarantee holds with several touches active when disabled.
    #[test]
    fn disabling_clears_all_active_touches_and_keeps_ids_reusable() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(1, 1), touch_id: 1 });
        t.translate(&Event::TouchBegin { pos: Point::new(2, 2), touch_id: 2 });
        t.translate(&Event::TouchBegin { pos: Point::new(3, 3), touch_id: 3 });
        assert_eq!(t.active_touch_count(), 3);

        t.set_enabled(false);
        assert_eq!(t.active_touch_count(), 0, "every tracked touch is cleared on disable");
        t.set_enabled(true);

        // All three old ids reused as ends: none may synthesize a release.
        for id in 1..=3 {
            assert!(
                t.translate(&Event::TouchEnd { pos: Point::new(9, 9), touch_id: id }).is_empty(),
                "id {id} had no begin in this enabled cycle, so its end must be ignored"
            );
        }

        // Fresh begins after the reset pair normally, one release each.
        for id in 1..=3 {
            assert_eq!(
                t.translate(&Event::TouchBegin { pos: Point::new(0, 0), touch_id: id }).len(),
                2
            );
        }
        for id in 1..=3 {
            assert_eq!(
                t.translate(&Event::TouchEnd { pos: Point::new(4, 4), touch_id: id }).len(),
                2
            );
        }
        assert_eq!(t.active_touch_count(), 0);
    }

    /// D09-EVT-07: disabling while already disabled still clears, and a same-value
    /// `set_enabled(true)` never re-adds stale state.
    #[test]
    fn repeated_disable_is_idempotent_and_never_revives_stale_touches() {
        let mut t = TouchEventTranslator::new();
        t.translate(&Event::TouchBegin { pos: Point::new(1, 1), touch_id: 9 });
        t.set_enabled(false);
        t.set_enabled(false);
        assert_eq!(t.active_touch_count(), 0);
        t.set_enabled(true);
        assert_eq!(t.active_touch_count(), 0, "re-enabling must not resurrect cleared touches");
        assert!(
            t.translate(&Event::TouchEnd { pos: Point::new(1, 1), touch_id: 9 }).is_empty(),
            "an end for a pre-disable begin must stay unmatched"
        );
    }
}
