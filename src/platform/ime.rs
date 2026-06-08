//! IME (Input Method Editor) bridge infrastructure.
//!
//! Provides the [`ImeBridge`] trait for platform IME integration,
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImeCandidatePosition {
    pub x: i32,
    pub y: i32,
}

/// Platform IME bridge trait.
///
/// Each platform backend that supports IME should implement this trait
/// and return an instance from [`Platform::ime_bridge()`].
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
#[derive(Debug, Default)]
pub struct MockImeBridge {
    focused_widget: std::sync::Mutex<Option<ObjectId>>,
    active: std::sync::Mutex<bool>,
}

impl MockImeBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_active(&self, active: bool) {
        *self.active.lock().unwrap() = active;
    }

    pub fn focused_widget(&self) -> Option<ObjectId> {
        *self.focused_widget.lock().unwrap()
    }
}

impl ImeBridge for MockImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = Some(widget_id);
    }

    fn focus_out(&self, _widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = None;
    }

    fn commit_text(&self, _text: &str) {}

    fn set_composition(&self, _composition: &ImeComposition) {}

    fn set_candidate_window_position(&self, _position: ImeCandidatePosition) {}

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
}
