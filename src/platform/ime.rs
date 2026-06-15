//! IME (Input Method Editor) bridge infrastructure.
//!
//! Provides the `ImeBridge` trait for platform IME integration,
//! IME event types, and a mock implementation for testing.

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
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ImeCandidatePosition {
    pub x: i32,
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

impl MockImeBridge {
    pub fn new() -> Self {
        Self {
            focused_widget: crate::compat::Mutex::new(None),
            active: crate::compat::Mutex::new(false),
            committed_text: crate::compat::Mutex::new(String::new()),
            composition: crate::compat::Mutex::new(ImeComposition::default()),
            candidate_position: crate::compat::Mutex::new(ImeCandidatePosition::default()),
        }
    }

    pub fn set_active(&self, active: bool) {
        *self.active.lock().unwrap() = active;
    }

    pub fn focused_widget(&self) -> Option<ObjectId> {
        *self.focused_widget.lock().unwrap()
    }

    /// Returns the last text committed via [`commit_text`](ImeBridge::commit_text).
    pub fn last_committed_text(&self) -> String {
        self.committed_text.lock().unwrap().clone()
    }

    /// Returns the last composition set via [`set_composition`](ImeBridge::set_composition).
    pub fn last_composition(&self) -> ImeComposition {
        self.composition.lock().unwrap().clone()
    }

    /// Returns the last candidate window position set via [`set_candidate_window_position`](ImeBridge::set_candidate_window_position).
    pub fn last_candidate_position(&self) -> ImeCandidatePosition {
        *self.candidate_position.lock().unwrap()
    }
}

impl ImeBridge for MockImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = Some(widget_id);
    }

    fn focus_out(&self, _widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = None;
    }

    fn commit_text(&self, text: &str) {
        *self.committed_text.lock().unwrap() = text.to_string();
    }

    fn set_composition(&self, composition: &ImeComposition) {
        *self.composition.lock().unwrap() = composition.clone();
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        *self.candidate_position.lock().unwrap() = position;
    }

    fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
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
}
