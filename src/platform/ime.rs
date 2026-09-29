// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! IME (Input Method Editor) bridge infrastructure.
//!
//! Provides the `ImeBridge` trait for platform IME integration,
//! IME event types, and a mock implementation for testing.

use crate::compat::{lock, String, ToString};
use crate::core::ObjectId;

/// IME composition state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImeComposition {
    /// Current composition string.
    pub text: String,
    /// Cursor position within the composition (byte offset).
    pub cursor_position: usize,
    /// Length of the selected text within the composition.
    pub selection_length: usize,
}

/// IME candidate window position.
///
/// In **screen** coordinates, not widget-relative ones, because the candidate
/// window is an OS-level window rather than part of the widget tree. Physical or
/// logical pixels depends on the backend, which is why this is documented rather
/// than converted here.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ImeCandidatePosition {
    /// Horizontal position in screen coordinates.
    pub x: i32,
    /// Vertical position in screen coordinates. Screen y grows downward on all
    /// supported backends.
    pub y: i32,
}

/// Platform IME bridge trait.
///
/// Each platform backend that supports IME should implement this trait
/// and return an instance from `Platform::ime_bridge()`.
pub trait ImeBridge: Send + Sync {
    /// Notify the IME that a widget has received focus and may accept IME input.
    fn focus_in(&self, widget_id: ObjectId);

    /// Notify the IME that a widget has lost focus.
    fn focus_out(&self, widget_id: ObjectId);

    /// Send a composed string to the currently focused widget.
    fn commit_text(&self, text: &str);

    /// Update the current composition preview (pre-edit text).
    fn set_composition(&self, composition: &ImeComposition);

    /// Set the position of the IME candidate window (in screen coordinates).
    fn set_candidate_window_position(&self, position: ImeCandidatePosition);

    /// Returns true if the platform currently has an active IME connection.
    fn is_active(&self) -> bool;
}

/// Delivers a bridge's composition and commit calls to the focused widget as
/// [`Event::ImePreedit`](crate::event::Event::ImePreedit) /
/// [`Event::ImeCommit`](crate::event::Event::ImeCommit).
///
/// # Why this exists
///
/// [`ImeBridge`] is the *platform* half of IME: it drives the OS candidate window and
/// receives the composition string. The *widget* half is `Event::ImePreedit` / `Event::ImeCommit`,
/// which controls like the code editor, `tag_input` and `search_bar` match on. Nothing joined
/// the two: the variants were published and handled but never produced by the library, so a
/// control that waited for `ImeCommit` was reachable only from a test. This is the join, and it
/// is a free function rather than a method on the bridge so that every backend gets the same
/// delivery without implementing it (the bridge trait stays about the OS connection).
///
/// Returns `false` when there is no focused widget to receive the event, which is the honest
/// answer for a composition that arrived with no caret to attach it to — an IME that is active
/// while focus is elsewhere must not write into a control the user did not select.
///
/// # Why this is gated on `not(alloc_frugal)`
///
/// The runtime registry — the thing that maps an id to a mounted control — does not exist in
/// `mini`, where controls are created and owned directly and there is no per-widget event
/// dispatch. There is also no IME there: `mini` is the profile with no platform singleton. So
/// the honest signature for that build is the same one, answering `false` because there is
/// nothing to deliver to — not a second signature and not a compile error at the call site
/// (rules #41/#53: an absent capability is a runtime answer, not an API fork).
#[cfg(not(alloc_frugal))]
pub fn deliver_composition(widget_id: ObjectId, composition: &ImeComposition) -> bool {
    let event = crate::event::Event::ImePreedit {
        text: composition.text.clone(),
        cursor: composition.cursor_position,
    };
    crate::widget::runtime::dispatch_event(widget_id, &event)
}

/// [`deliver_composition`] for an allocation-frugal build: there is no widget registry and no
/// IME, so nothing is delivered. See that function for why this is a body and not a signature.
#[cfg(alloc_frugal)]
pub fn deliver_composition(_widget_id: ObjectId, _composition: &ImeComposition) -> bool {
    false
}

/// Delivers committed IME text to `widget_id` as
/// [`Event::ImeCommit`](crate::event::Event::ImeCommit).
///
/// See [`deliver_composition`] for why this join exists. The same call also tells the live
/// text model about the commit (`LineEdit::commit_composition`) when the control exposes one,
/// because the two paths are the same user action: a control that matches on `Event::ImeCommit`
/// gets the event, and a `LineEdit` that holds composition state gets it resolved.
///
/// Returns `false` when `widget_id` is not a mounted control.
#[cfg(not(alloc_frugal))]
pub fn deliver_commit(widget_id: ObjectId, text: &str) -> bool {
    let event = crate::event::Event::ime_commit(text);
    crate::widget::runtime::dispatch_event(widget_id, &event)
}

/// [`deliver_commit`] for an allocation-frugal build. See [`deliver_composition`].
#[cfg(alloc_frugal)]
pub fn deliver_commit(_widget_id: ObjectId, _text: &str) -> bool {
    false
}

/// Mock IME bridge for testing.
#[derive(Debug)]
pub struct MockImeBridge {
    focused_widget: crate::compat::Mutex<Option<ObjectId>>,
    active: crate::compat::Mutex<bool>,
    /// Last text committed via [`commit_text`](ImeBridge::commit_text).
    pub(crate) committed_text: crate::compat::Mutex<String>,
    /// Last composition set via [`set_composition`](ImeBridge::set_composition).
    pub(crate) composition: crate::compat::Mutex<ImeComposition>,
    /// Last candidate window position set via [`set_candidate_window_position`](ImeBridge::set_candidate_window_position).
    pub(crate) candidate_position: crate::compat::Mutex<ImeCandidatePosition>,
}

crate::impl_default_via_new!(MockImeBridge);

impl MockImeBridge {
    /// Creates an inactive bridge with no focused widget, no committed text, a
    /// default (empty) composition, and a zeroed candidate position.
    pub fn new() -> Self {
        Self {
            focused_widget: crate::compat::Mutex::new(None),
            active: crate::compat::Mutex::new(false),
            committed_text: crate::compat::Mutex::new(String::new()),
            composition: crate::compat::Mutex::new(ImeComposition::default()),
            candidate_position: crate::compat::Mutex::new(ImeCandidatePosition::default()),
        }
    }

    /// Forces the mock's active flag, which `is_active` then reports.
    ///
    /// Real backends derive activity from their host connection; this setter
    /// exists so tests can exercise both states without one.
    pub fn set_active(&self, active: bool) {
        *lock(&self.active) = active;
    }

    /// Returns the widget id passed to the most recent `focus_in`, or `None`
    /// after a `focus_out` or before any focus call.
    pub fn focused_widget(&self) -> Option<ObjectId> {
        *lock(&self.focused_widget)
    }

    /// Returns the last text committed via [`commit_text`](ImeBridge::commit_text).
    pub fn last_committed_text(&self) -> String {
        lock(&self.committed_text).clone()
    }

    /// Returns the last composition set via [`set_composition`](ImeBridge::set_composition).
    pub fn last_composition(&self) -> ImeComposition {
        lock(&self.composition).clone()
    }

    /// Returns the last candidate window position set via [`set_candidate_window_position`](ImeBridge::set_candidate_window_position).
    pub fn last_candidate_position(&self) -> ImeCandidatePosition {
        *lock(&self.candidate_position)
    }
}

impl ImeBridge for MockImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = Some(widget_id);
    }

    fn focus_out(&self, _widget_id: ObjectId) {
        *lock(&self.focused_widget) = None;
    }

    fn commit_text(&self, text: &str) {
        *lock(&self.committed_text) = text.to_string();
    }

    fn set_composition(&self, composition: &ImeComposition) {
        *lock(&self.composition) = composition.clone();
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        *lock(&self.candidate_position) = position;
    }

    fn is_active(&self) -> bool {
        *lock(&self.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_focus_in_out() {
        let bridge = MockImeBridge::new();
        assert_eq!(bridge.focused_widget(), None);
        bridge.focus_in(42);
        assert_eq!(bridge.focused_widget(), Some(42));
        bridge.focus_out(42);
        assert_eq!(bridge.focused_widget(), None);
    }

    #[test]
    fn test_mock_active() {
        let bridge = MockImeBridge::new();
        assert!(!bridge.is_active());
        bridge.set_active(true);
        assert!(bridge.is_active());
    }

    #[test]
    fn test_ime_composition_default() {
        let comp = ImeComposition::default();
        assert!(comp.text.is_empty());
        assert_eq!(comp.cursor_position, 0);
    }

    #[test]
    fn test_ime_bridge_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<MockImeBridge>();
        assert_sync::<MockImeBridge>();
    }

    #[test]
    fn test_ime_composition_non_default() {
        let comp =
            ImeComposition { text: "你好".to_string(), cursor_position: 2, selection_length: 0 };
        assert_eq!(comp.text, "你好");
        assert_eq!(comp.cursor_position, 2);
        assert_eq!(comp.selection_length, 0);
    }

    #[test]
    fn test_ime_candidate_position() {
        let pos = ImeCandidatePosition { x: 100, y: 200 };
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 200);
        // Verify Copy semantics
        let pos2 = pos;
        assert_eq!(pos.x, pos2.x);
    }

    #[test]
    fn test_mock_commit_text_stores_text() {
        let bridge = MockImeBridge::new();
        bridge.commit_text("hello");
        assert_eq!(bridge.last_committed_text(), "hello");
    }

    #[test]
    fn test_mock_commit_text_empty_string() {
        let bridge = MockImeBridge::new();
        bridge.commit_text("");
        assert_eq!(bridge.last_committed_text(), "");
    }

    #[test]
    fn test_mock_commit_text_long_text() {
        let bridge = MockImeBridge::new();
        let long = "a".repeat(10_000);
        bridge.commit_text(&long);
        assert_eq!(bridge.last_committed_text(), long);
    }

    #[test]
    fn test_mock_commit_text_overwrites_previous() {
        let bridge = MockImeBridge::new();
        bridge.commit_text("first");
        bridge.commit_text("second");
        assert_eq!(bridge.last_committed_text(), "second");
    }

    #[test]
    fn test_mock_set_composition_stores_composition() {
        let bridge = MockImeBridge::new();
        let comp =
            ImeComposition { text: "compose".to_string(), cursor_position: 3, selection_length: 0 };
        bridge.set_composition(&comp);
        assert_eq!(bridge.last_composition(), comp);
    }

    #[test]
    fn test_mock_set_composition_with_cursor_position() {
        let bridge = MockImeBridge::new();
        let comp = ImeComposition {
            text: "你好世界".to_string(),
            cursor_position: 6,
            selection_length: 0,
        };
        bridge.set_composition(&comp);
        assert_eq!(bridge.last_composition().cursor_position, 6);
    }

    #[test]
    fn test_mock_set_composition_update() {
        let bridge = MockImeBridge::new();
        let first =
            ImeComposition { text: "abc".to_string(), cursor_position: 3, selection_length: 0 };
        bridge.set_composition(&first);
        let second =
            ImeComposition { text: "abcd".to_string(), cursor_position: 4, selection_length: 1 };
        bridge.set_composition(&second);
        assert_eq!(bridge.last_composition(), second);
    }

    #[test]
    fn test_mock_set_candidate_window_position_stores_position() {
        let bridge = MockImeBridge::new();
        let pos = ImeCandidatePosition { x: 320, y: 480 };
        bridge.set_candidate_window_position(pos);
        assert_eq!(bridge.last_candidate_position(), pos);
    }

    #[test]
    fn test_mock_candidate_position_roundtrip() {
        let bridge = MockImeBridge::new();
        let original = ImeCandidatePosition { x: 100, y: 200 };
        bridge.set_candidate_window_position(original);
        let retrieved = bridge.last_candidate_position();
        assert_eq!(retrieved.x, 100);
        assert_eq!(retrieved.y, 200);
    }

    #[test]
    fn test_mock_candidate_position_overwrites() {
        let bridge = MockImeBridge::new();
        bridge.set_candidate_window_position(ImeCandidatePosition { x: 0, y: 0 });
        bridge.set_candidate_window_position(ImeCandidatePosition { x: 999, y: 888 });
        assert_eq!(bridge.last_candidate_position(), ImeCandidatePosition { x: 999, y: 888 });
    }

    /// The join between the platform bridge and the widget event layer: a composition is
    /// delivered to the focused control as `Event::ImePreedit`, and a commit as
    /// `Event::ImeCommit`. Without this the two variants had **no producer anywhere** —
    /// controls that matched on them could never be reached outside a test.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn an_ime_composition_and_commit_reach_the_focused_widget() {
        use crate::event::{Event, EventHandler};
        use core::cell::RefCell;
        use std::rc::Rc;

        /// Records the IME events it receives into a shared log, so the test can read them
        /// after the widget has been handed to the runtime.
        struct Recorder {
            log: Rc<RefCell<Vec<String>>>,
            base: crate::widget::BaseWidget,
        }
        impl crate::widget::Widget for Recorder {
            fn base(&self) -> &crate::widget::BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
                &mut self.base
            }
        }
        impl EventHandler for Recorder {
            fn handle_event(&mut self, event: &Event) {
                match event {
                    Event::ImePreedit { text, cursor } => {
                        self.log.borrow_mut().push(format!("preedit:{text}@{cursor}"));
                    }
                    Event::ImeCommit { text } => {
                        self.log.borrow_mut().push(format!("commit:{text}"));
                    }
                    _ => {}
                }
            }
        }

        let log = Rc::new(RefCell::new(Vec::new()));
        let id = crate::widget::runtime::register(Box::new(Recorder {
            log: Rc::clone(&log),
            base: crate::widget::BaseWidget::new(
                crate::widget::WidgetKind::Label,
                crate::core::Rect::new(0, 0, 10, 10),
                "recorder",
            ),
        }))
        .expect("mount the recorder");

        let composition =
            ImeComposition { text: "にほ".to_string(), cursor_position: 6, selection_length: 0 };
        assert!(deliver_composition(id, &composition), "a preedit reaches the control");
        assert!(deliver_commit(id, "日本語"), "a commit reaches the control");

        assert_eq!(*log.borrow(), vec!["preedit:にほ@6", "commit:日本語"]);

        crate::widget::runtime::unregister(id);
    }
}
