//! Virtual keyboard integration for touch-based text input (BLUE8 P4-7).
//!
//! This module provides a `VirtualKeyboard` controller that manages
//! keyboard show/hide state, layout adaptation (moving the focused
//! widget above the keyboard), and platform-specific OSK invocation.
//!
//! # Architecture
//!
//! - `VirtualKeyboard` — Main controller with state machine.
//! - `KeyboardNotch` — Represents the safe-area inset caused by the OSK.
//! - Platform backends integrate via `PlatformKeyboard` extension trait.
//!
//! # Integration points
//!
//! - `LineEdit`, `TextEdit`, `SpinBox`, `ComboBox` call
//!   `VirtualKeyboard::request_show()` on touch focus.
//! - The platform event loop calls `VirtualKeyboard::apply_layout_shift()`
//!   before rendering to shift content upward when the keyboard is visible.

use crate::core::{ObjectId, Rect};

/// State of the virtual keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardState {
    /// Keyboard is hidden (default).
    Hidden,
    /// Keyboard is being animated in (transition).
    Showing,
    /// Keyboard is visible on screen.
    Visible,
    /// Keyboard is being animated out (transition).
    Hiding,
}

impl KeyboardState {
    /// Returns `true` if the keyboard is either visible or in transition.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Showing | Self::Visible | Self::Hiding)
    }

    /// Returns `true` if the keyboard is fully visible.
    pub fn is_visible(self) -> bool {
        self == Self::Visible
    }
}

/// Safe-area inset caused by the virtual keyboard.
///
/// Describes how much the keyboard overlaps the bottom of the screen,
/// so the UI can shift the focused widget above this region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyboardNotch {
    /// Height of the keyboard overlay in logical pixels.
    pub height: u32,
    /// Duration (ms) of the show/hide animation.
    pub animation_ms: u32,
}

impl Default for KeyboardNotch {
    fn default() -> Self {
        Self { height: 0, animation_ms: 300 }
    }
}

impl KeyboardNotch {
    /// Create a notch with the given overlay height and default 300 ms animation.
    pub const fn new(height: u32) -> Self {
        Self { height, animation_ms: 300 }
    }

    /// Returns `true` when the notch is non-zero (keyboard is visible).
    pub fn is_present(self) -> bool {
        self.height > 0
    }
}

/// Virtual keyboard controller managing OSK lifecycle and layout adaptation.
#[derive(Debug)]
pub struct VirtualKeyboard {
    /// Current keyboard visibility state.
    state: KeyboardState,
    /// Current safe-area notch.
    notch: KeyboardNotch,
    /// The widget that requested the keyboard (if any).
    focused_widget: Option<ObjectId>,
    /// Original layout offset before keyboard appeared.
    original_offset_y: i32,
    /// Current vertical shift applied to compensate for the keyboard.
    shift_y: i32,
}

impl Default for VirtualKeyboard {
    fn default() -> Self {
        Self {
            state: KeyboardState::Hidden,
            notch: KeyboardNotch::default(),
            focused_widget: None,
            original_offset_y: 0,
            shift_y: 0,
        }
    }
}

impl VirtualKeyboard {
    /// Creates a new virtual keyboard controller.
    pub fn new() -> Self {
        Self::default()
    }

    // ── State queries ──

    /// Returns the current keyboard state.
    pub fn state(&self) -> KeyboardState {
        self.state
    }

    /// Returns the current keyboard safe-area notch.
    pub fn notch(&self) -> KeyboardNotch {
        self.notch
    }

    /// Returns the current vertical shift applied to the UI.
    pub fn shift_y(&self) -> i32 {
        self.shift_y
    }

    /// Returns the widget that currently holds keyboard focus (if any).
    pub fn focused_widget(&self) -> Option<ObjectId> {
        self.focused_widget
    }

    /// Returns `true` when the keyboard is visible or animating in.
    pub fn is_keyboard_active(&self) -> bool {
        self.state.is_active()
    }

    // ── State transitions ──

    /// Request the virtual keyboard to show, targeting a specific widget.
    ///
    /// Called by text-input widgets when they receive touch focus.
    /// `widget_rect` is the widget's geometry in screen coordinates.
    /// `screen_height` is the usable screen height before keyboard.
    /// `notch` describes the expected keyboard size.
    pub fn request_show(
        &mut self,
        widget_id: ObjectId,
        widget_rect: Rect,
        screen_height: u32,
        notch: KeyboardNotch,
    ) {
        self.focused_widget = Some(widget_id);
        self.notch = notch;
        self.state = KeyboardState::Showing;

        // Calculate how much we need to shift.
        let widget_bottom = widget_rect.y + widget_rect.height as i32;
        let available_height = screen_height.saturating_sub(self.notch.height) as i32;

        if widget_bottom > available_height {
            // Store the original offset before shifting.
            self.original_offset_y = self.shift_y;
            // Widget would be covered — shift up.
            self.shift_y = available_height - widget_bottom;
        } else {
            // Widget is already visible above keyboard area.
            self.original_offset_y = 0;
            self.shift_y = 0;
        }
    }

    /// Notify that the keyboard show animation has completed.
    pub fn on_shown(&mut self) {
        self.state = KeyboardState::Visible;
    }

    /// Request the virtual keyboard to hide.
    pub fn request_hide(&mut self) {
        self.state = KeyboardState::Hiding;
        self.focused_widget = None;
    }

    /// Notify that the keyboard hide animation has completed.
    pub fn on_hidden(&mut self) {
        self.state = KeyboardState::Hidden;
        self.notch = KeyboardNotch::default();
        // Restore original layout offset.
        self.shift_y = self.original_offset_y;
        self.original_offset_y = 0;
    }

    /// Apply the computed layout shift to a widget rectangle.
    ///
    /// This translates the rectangle upward by `shift_y` so that the
    /// widget remains visible when the keyboard is displayed.
    pub fn apply_layout_shift(&self, widget_rect: &mut Rect) {
        if self.shift_y != 0 {
            widget_rect.y += self.shift_y;
        }
    }

    /// Reset all state (used when focus is lost or window is deactivated).
    pub fn reset(&mut self) {
        self.state = KeyboardState::Hidden;
        self.notch = KeyboardNotch::default();
        self.focused_widget = None;
        self.shift_y = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_hidden() {
        let kb = VirtualKeyboard::new();
        assert_eq!(kb.state(), KeyboardState::Hidden);
        assert!(!kb.is_keyboard_active());
        assert_eq!(kb.shift_y(), 0);
        assert!(kb.focused_widget().is_none());
    }

    #[test]
    fn request_show_transitions_to_showing() {
        let mut kb = VirtualKeyboard::new();
        let rect = Rect::new(0, 100, 200, 40);
        kb.request_show(ObjectId::from(42u64), rect, 800, KeyboardNotch::new(300));
        assert_eq!(kb.state(), KeyboardState::Showing);
        assert!(kb.is_keyboard_active());
        assert_eq!(kb.focused_widget(), Some(ObjectId::from(42u64)));
    }

    #[test]
    fn on_shown_transitions_to_visible() {
        let mut kb = VirtualKeyboard::new();
        kb.request_show(
            ObjectId::from(1u64),
            Rect::new(0, 100, 200, 40),
            800,
            KeyboardNotch::new(300),
        );
        kb.on_shown();
        assert_eq!(kb.state(), KeyboardState::Visible);
    }

    #[test]
    fn request_hide_transitions_to_hiding() {
        let mut kb = VirtualKeyboard::new();
        kb.request_show(
            ObjectId::from(1u64),
            Rect::new(0, 100, 200, 40),
            800,
            KeyboardNotch::new(300),
        );
        kb.on_shown();
        kb.request_hide();
        assert_eq!(kb.state(), KeyboardState::Hiding);
    }

    #[test]
    fn on_hidden_resets_state() {
        let mut kb = VirtualKeyboard::new();
        kb.request_show(
            ObjectId::from(1u64),
            Rect::new(0, 100, 200, 40),
            800,
            KeyboardNotch::new(300),
        );
        kb.on_shown();
        kb.request_hide();
        kb.on_hidden();
        assert_eq!(kb.state(), KeyboardState::Hidden);
        assert_eq!(kb.shift_y(), 0);
        assert!(kb.focused_widget().is_none());
    }

    #[test]
    fn shift_applied_when_widget_would_be_covered() {
        let mut kb = VirtualKeyboard::new();
        let rect = Rect::new(0, 700, 200, 40); // Bottom at 740
        kb.request_show(
            ObjectId::from(1u64),
            rect,
            800,                     // Screen height
            KeyboardNotch::new(300), // Keyboard takes bottom 300
        );
        // Available = 800 - 300 = 500
        // Widget bottom = 740 > 500 → shift = 500 - 740 = -240
        assert_eq!(kb.shift_y(), -240);
    }

    #[test]
    fn no_shift_when_widget_above_keyboard() {
        let mut kb = VirtualKeyboard::new();
        let rect = Rect::new(0, 100, 200, 40); // Bottom at 140
        kb.request_show(ObjectId::from(1u64), rect, 800, KeyboardNotch::new(300));
        // Available = 800 - 300 = 500
        // Widget bottom = 140 ≤ 500 → no shift
        assert_eq!(kb.shift_y(), 0);
    }

    #[test]
    fn apply_layout_shift_modifies_rect() {
        let mut kb = VirtualKeyboard::new();
        let rect = Rect::new(0, 700, 200, 40);
        kb.request_show(ObjectId::from(1u64), rect, 800, KeyboardNotch::new(300));
        let mut shifted = Rect::new(10, 200, 100, 30);
        kb.apply_layout_shift(&mut shifted);
        assert_eq!(shifted.y, 200 + kb.shift_y());
    }

    #[test]
    fn reset_clears_focus_and_shift() {
        let mut kb = VirtualKeyboard::new();
        kb.request_show(
            ObjectId::from(1u64),
            Rect::new(0, 700, 200, 40),
            800,
            KeyboardNotch::new(300),
        );
        kb.on_shown();
        kb.reset();
        assert_eq!(kb.state(), KeyboardState::Hidden);
        assert_eq!(kb.shift_y(), 0);
        assert!(kb.focused_widget().is_none());
    }
}
