// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event types and handler trait.
use crate::compat::{Box, String, Vec};
use crate::core::{Point, Size};
/// A unique identifier for a touch contact point (used by `touch` and `holographic` features).
pub type TouchId = u64;

/// The mouse button codes carried by [`Event::MousePress`] and friends.
///
/// These are named because every widget that cares about a *specific* button was
/// comparing against a bare literal (`button == 1`), which made the two facts that
/// matter — "is this the primary button?" and "is this a secondary click that
/// should open a context menu?" — impossible to grep for and easy to mistype.
///
/// The numbering follows the platform convention shared by the desktop backends:
/// 1 is primary, 2 is secondary, 3 is middle.
pub mod mouse_button {
    /// The primary (usually left) button, used for activation.
    pub const PRIMARY: u32 = 1;
    /// The secondary (usually right) button, used to open a context menu.
    pub const SECONDARY: u32 = 2;
    /// The middle button.
    pub const MIDDLE: u32 = 3;
}

/// Why a widget received or lost keyboard focus.
///
/// # Why the reason must travel with the event
///
/// "This widget has focus" and "the user is navigating with the keyboard" are two
/// different facts, and only the second should draw a focus ring. The rule is exactly
/// this distinction: what is drawn as a focus ring is
/// `activeFocus && (reason == Tab | Backtab | Shortcut)`.
///
/// Without the reason, a control can only know *that* it is focused, so it either
/// draws a ring on every click (which looks broken on a mouse-driven desktop) or
/// never draws one (which makes keyboard navigation invisible). Neither is a
/// tuning problem: the information was simply not delivered.
///
/// The variants mirror the standard toolkit `FocusReason` set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FocusReason {
    /// A pointer press moved focus — clicking must not show a focus ring.
    Pointer,
    /// `Tab` moved focus forward; the canonical "the user is on the keyboard" signal.
    #[default]
    Tab,
    /// `Shift+Tab` (or `Backtab`) moved focus backward. Also keyboard navigation,
    /// but some controls animate the ring's entry from the other side.
    BackTab,
    /// A keyboard accelerator or `Shortcut` invoked the widget.
    Shortcut,
    /// The application moved focus itself (dialogs, form focus order, programmatic
    /// `focus_widget`), with no user navigation behind it.
    Programmatic,
}

impl FocusReason {
    /// Whether a control should paint a focus ring for this reason.
    ///
    /// A pointer press is the one reason that must **not**: the pointer already tells
    /// the user where they are, and a ring drawn under the cursor reads as a stuck
    /// highlight. This is the standard rule. This is a method rather than a call site
    /// predicate so the one place that knows the answer is the one place that names
    /// the reasons — a new variant must be classified here, not at each draw site.
    pub fn draws_focus_ring(self) -> bool {
        match self {
            FocusReason::Pointer => false,
            FocusReason::Tab | FocusReason::BackTab | FocusReason::Shortcut => true,
            // Programmatic focus is the application saying "this control is current".
            // Showing the ring reflects what the application asked for, and without it
            // a form that focuses its first field on open would look unfocused.
            FocusReason::Programmatic => true,
        }
    }

    /// Whether this reason came from the keyboard.
    ///
    /// Read by controls that change *how* they respond (a list that scrolls its
    /// selection into view on a keyboard move, but not on a click).
    pub fn is_keyboard(self) -> bool {
        matches!(self, FocusReason::Tab | FocusReason::BackTab | FocusReason::Shortcut)
    }
}

/// Screen orientation enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenOrientation {
    /// Device in portrait orientation.
    Portrait,
    /// Device in landscape orientation.
    Landscape,
    /// Device in reverse portrait orientation (180° rotated).
    ReversePortrait,
    /// Device in reverse landscape orientation (180° rotated).
    ReverseLandscape,
}

/// Gesture complexity / detection tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureClass {
    /// Single-point gesture (tap, long-press, swipe).
    Single,
    /// Multi-point gesture (pinch, rotate).
    Multi,
    /// Holographic/3D gesture (Z-axis detected, gated behind `holographic` feature).
    #[cfg(feature = "holographic")]
    Holographic,
}
/// Event payload variants routed through the event loop.
#[derive(Debug, Clone)]
pub enum Event {
    /// Legacy pointer/button press payload.
    ///
    /// This is a legacy variant kept for backward compatibility.
    /// Use `MousePress` instead.
    MouseDown((Point, u32)),
    /// Legacy pointer/button release payload.
    ///
    /// This is a legacy variant kept for backward compatibility.
    /// Use `MouseRelease` instead.
    MouseUp((Point, u32)),
    /// Legacy pointer move payload.
    ///
    /// This is a legacy variant kept for backward compatibility.
    /// Use `MouseMove` instead.
    MouseMoveLegacy((Point, u32)),
    /// Legacy keyboard press payload.
    ///
    /// This is a legacy variant kept for backward compatibility.
    /// Use `KeyPress` instead.
    KeyDown((u32, u32)),
    /// Legacy keyboard release payload.
    ///
    /// This is a legacy variant kept for backward compatibility.
    /// Use `KeyRelease` instead.
    KeyUp((u32, u32)),
    /// Focus gained, with the reason it moved.
    ///
    /// The reason is part of the payload rather than a separate query because a
    /// control must decide "draw a focus ring?" *while handling the event*, and a
    /// later lookup on a shared focus manager could already describe a different
    /// move. Use [`FocusReason::draws_focus_ring`] rather than matching on the
    /// variants at each draw site.
    FocusGained {
        /// What caused the focus to move to this widget.
        reason: FocusReason,
    },
    /// Legacy focus lost event.
    ///
    /// This is a legacy variant kept for backward compatibility.
    /// Use the `focus_lost` signal on `BaseWidget` instead.
    FocusLost,
    /// Pointer moved inside active surface.
    ///
    /// Sent for every hover motion; it does not imply a button is held. Use
    /// [`Event::PointerMove`] when stylus pressure or tilt is needed.
    MouseMove {
        /// New pointer position, relative to the window origin (not the widget).
        pos: Point,
    },
    /// Pointer/button press.
    ///
    /// The press is not delivered to the widget under the pointer until the
    /// backend has hit-tested it; `pos` is still window-relative.
    MousePress {
        /// Press position, relative to the window origin.
        pos: Point,
        /// Which button was pressed; see [`mouse_button`].
        button: u32,
    },
    /// Pointer double-click.
    ///
    /// The press that starts the gesture is reported separately as
    /// [`Event::MousePress`], so a handler that acts on both must guard against
    /// firing twice.
    MouseDoubleClick {
        /// Click position, relative to the window origin.
        pos: Point,
        /// Which button was double-clicked; see [`mouse_button`].
        button: u32,
    },
    /// Pointer/button release.
    ///
    /// Every press is expected to be followed by a release, including presses
    /// drained by [`Event::MouseLeave`], so handlers can rely on it to end a drag.
    MouseRelease {
        /// Release position, relative to the window origin.
        pos: Point,
        /// Which button was released; see [`mouse_button`].
        button: u32,
    },
    /// Pointer entered widget bounds.
    MouseEnter {
        /// Entry position, relative to the window origin.
        pos: Point,
    },
    /// Pointer left widget bounds.
    MouseLeave {
        /// Exit position, relative to the window origin.
        pos: Point,
    },
    /// Keyboard key press.
    KeyPress {
        /// Key code in the framework's convention; see [`crate::shortcut::Key::from_key_code`].
        key: u32,
        /// Modifier bitmask in the framework's convention, where the Meta bit
        /// means the primary accelerator; see
        /// [`crate::shortcut::Modifiers::from_event_bits`].
        modifiers: u32,
    },
    /// Keyboard key release.
    KeyRelease {
        /// Key code, using the same convention as [`Event::KeyPress`].
        key: u32,
        /// Modifier bitmask, using the same convention as [`Event::KeyPress`].
        modifiers: u32,
    },
    /// Text committed by keyboard layout, IME, virtual keyboard, or paste-like input.
    TextInput {
        /// The committed text. Already filtered through the active keyboard
        /// layout, so it is not derivable from a raw key code.
        text: String,
    },
    /// IME preedit/composition text changed without committing to the widget value.
    ImePreedit {
        /// The composition string under construction.
        text: String,
        /// Byte offset of the insertion point within `text`, for anchoring the
        /// candidate window.
        cursor: usize,
    },
    /// IME composition committed text to the widget value.
    ImeCommit {
        /// The final, committed text; the matching preedit must be discarded.
        text: String,
    },
    /// Repaint request.
    Paint,
    /// Resize notification.
    Resize {
        /// The new content size in logical pixels.
        size: Size,
    },
    /// Timer fired.
    Timer {
        /// Identifier of the timer that fired, as passed when the timer was armed.
        id: u32,
    },
    /// Mouse wheel / scroll event.
    ///
    /// The delta is in wheel notches rather than pixels, and its sign is the
    /// reverse of the platform "scroll amount": a positive `delta.y` means the
    /// content moves down (one notch toward the user). One notch is also the
    /// conventional multiplier for a line scroll, `-delta.y` lines.
    Wheel {
        /// Scroll delta: `y` is vertical (positive = scroll down), `x` is
        /// horizontal (positive = scroll right), both in wheel notches.
        delta: Point,
        /// Modifier bitmask, using the same convention as [`Event::KeyPress`].
        modifiers: u32,
    },
    /// Free-form custom event payload.
    Custom {
        /// The event name the sender and receiver agree on.
        name: String,
        /// Opaque payload; the framework does not interpret it.
        payload: Vec<u8>,
    },
    /// Screen orientation changed (portrait ↔ landscape).
    OrientationChanged {
        /// The orientation now in effect.
        orientation: ScreenOrientation,
    },
    /// Event loop shutdown signal.
    Quit,
    // ── Touch / Gesture events (gated behind `touch` feature) ──
    /// Finger touched surface (replaces MouseDown on touch devices).
    #[cfg(feature = "touch")]
    TouchBegin {
        /// Touch position, relative to the window origin.
        pos: Point,
        /// Identifies the finger for the whole contact, so multi-touch streams
        /// can be separated.
        touch_id: TouchId,
    },
    /// Finger lifted from surface (replaces MouseUp on touch devices).
    #[cfg(feature = "touch")]
    TouchEnd {
        /// Touch position, relative to the window origin.
        pos: Point,
        /// The contact identifier reported by the matching [`Event::TouchBegin`].
        touch_id: TouchId,
    },
    /// Finger moved on surface (replaces MouseMove on touch devices).
    #[cfg(feature = "touch")]
    TouchMove {
        /// Current touch position, relative to the window origin.
        pos: Point,
        /// The contact identifier reported by the matching [`Event::TouchBegin`].
        touch_id: TouchId,
    },
    /// Quick tap-and-release gesture (≈ click).
    #[cfg(feature = "touch")]
    Tap {
        /// Tap position, relative to the window origin.
        pos: Point,
    },
    /// Two rapid taps in succession (≈ double-click).
    #[cfg(feature = "touch")]
    DoubleTap {
        /// Position of the second tap, relative to the window origin.
        pos: Point,
    },
    /// Finger held stationary ≥ 500ms.
    #[cfg(feature = "touch")]
    LongPress {
        /// Press position, relative to the window origin.
        pos: Point,
    },
    /// Rapid linear finger motion.
    #[cfg(feature = "touch")]
    Swipe {
        /// Where the swipe started, relative to the window origin.
        start: Point,
        /// Where the swipe ended, relative to the window origin.
        end: Point,
        /// Swipe speed, in logical pixels per second.
        velocity: f32,
    },
    /// Two-finger pinch (scale < 1 = zoom out, > 1 = zoom in).
    #[cfg(feature = "touch")]
    Pinch {
        /// Size ratio against the initial finger separation: `1.0` means no
        /// change, below `1.0` is a pinch in, above `1.0` a spread out.
        scale: f32,
    },
    /// Two-finger rotation in radians.
    #[cfg(feature = "touch")]
    Rotate {
        /// Rotation since the gesture began, in radians; positive is clockwise
        /// in screen space (y grows downward).
        angle: f32,
    },
    /// Finger drag with motion tracking.
    #[cfg(feature = "touch")]
    Drag {
        /// Current finger position, relative to the window origin.
        pos: Point,
        /// The contact identifier reported by the matching [`Event::TouchBegin`].
        touch_id: TouchId,
        /// Movement since the previous drag event, in logical pixels. This is a
        /// per-event step, not an offset from the gesture start.
        delta: Point,
    },
    /// Two-finger tap (≈ right-click equivalent on touchscreens).
    #[cfg(feature = "touch")]
    TwoFingerTap {
        /// Tap position, relative to the window origin.
        pos: Point,
    },
    /// Two-finger swipe (e.g., page navigation with two fingers).
    #[cfg(feature = "touch")]
    TwoFingerSwipe {
        /// Midpoint between the two fingers when the swipe started.
        centroid_start: Point,
        /// Midpoint between the two fingers when the swipe ended.
        centroid_end: Point,
        /// Swipe speed, in logical pixels per second.
        velocity: f32,
    },
    /// Velocity-based fling/flick with vector velocity (vx, vy).
    #[cfg(feature = "touch")]
    Fling {
        /// Position the fling originated from, relative to the window origin; it
        /// does not track the pointer, as the finger has already left.
        pos: Point,
        /// Initial velocity as a vector in logical pixels per second: `x` is
        /// horizontal, `y` vertical (positive `y` is downward).
        velocity: Point,
        /// The contact identifier reported by the matching [`Event::TouchBegin`].
        touch_id: TouchId,
    },
    // ── Holographic / 3D events (BLUE8 P4-5, gated behind `holographic` feature) ──
    /// 3D touch/gesture with depth information (holographic).
    #[cfg(feature = "holographic")]
    HolographicTouch {
        /// Touch position projected onto the interaction plane.
        pos: Point,
        /// Distance along the Z axis in **centimetres**; positive is toward the
        /// user.
        depth: f32,
        /// The contact identifier for the whole gesture.
        touch_id: TouchId,
    },
    // ── Pointer / Stylus events (BLUE11 R8.1) ──
    /// Pointer/stylus press with pressure and tilt.
    PointerPress {
        /// Press position, relative to the window origin.
        pos: Point,
        /// Which button or barrel switch was pressed; see [`mouse_button`].
        button: u32,
        /// Tip pressure in the normalized range `0.0..=1.0`, where `1.0` is the
        /// device maximum. A mouse reports `0.5` on press.
        pressure: f32,
        /// Stylus tilt away from the surface normal about the X axis; `0.0` is
        /// upright. The sign convention is device-specific.
        tilt_x: f32,
        /// Stylus tilt about the Y axis; `0.0` is upright. The sign convention is
        /// device-specific.
        tilt_y: f32,
    },
    /// Pointer/stylus move with pressure and tilt.
    PointerMove {
        /// Current position, relative to the window origin.
        pos: Point,
        /// Tip pressure in the normalized range `0.0..=1.0`; `0.0` while hovering
        /// without contact.
        pressure: f32,
        /// Stylus tilt about the X axis; `0.0` is upright.
        tilt_x: f32,
        /// Stylus tilt about the Y axis; `0.0` is upright.
        tilt_y: f32,
    },
    /// Pointer/stylus release with pressure.
    PointerRelease {
        /// Release position, relative to the window origin.
        pos: Point,
        /// Which button or barrel switch was released; see [`mouse_button`].
        button: u32,
        /// Tip pressure at the moment of release, in the normalized range
        /// `0.0..=1.0`; `0.0` when the tip was already lifted away.
        pressure: f32,
    },
    // ── Gamepad Events (BLUE11 R8.3) ──
    /// Gamepad button press.
    GamepadPress {
        /// Device-specific button index, as reported by the platform's gamepad
        /// API; there is no cross-platform button numbering here.
        button: u32,
    },
    /// Gamepad button release.
    GamepadRelease {
        /// The index reported by the matching [`Event::GamepadPress`].
        button: u32,
    },
    /// Gamepad axis movement.
    GamepadAxis {
        /// Device-specific axis index.
        axis: u32,
        /// Normalized axis position: `-1.0` to `1.0` for a stick, `0.0` to `1.0`
        /// for a trigger, with `0.0` at rest in both cases.
        value: f32,
    },
    /// Gamepad connected.
    GamepadConnected {
        /// Identifier assigned to the gamepad for the lifetime of the connection.
        id: u32,
    },
    /// Gamepad disconnected.
    GamepadDisconnected {
        /// The identifier given by the matching [`Event::GamepadConnected`].
        id: u32,
    },
}
impl Event {
    /// Creates a mouse press event.
    pub fn mouse_press(x: i32, y: i32, button: u32) -> Self {
        Self::MousePress { pos: Point::new(x, y), button }
    }
    /// Creates a mouse release event.
    pub fn mouse_release(x: i32, y: i32, button: u32) -> Self {
        Self::MouseRelease { pos: Point::new(x, y), button }
    }
    /// Creates a mouse double-click event.
    pub fn mouse_double_click(x: i32, y: i32, button: u32) -> Self {
        Self::MouseDoubleClick { pos: Point::new(x, y), button }
    }
    /// Creates a mouse move event.
    pub fn mouse_move(x: i32, y: i32) -> Self {
        Self::MouseMove { pos: Point::new(x, y) }
    }
    /// Creates a mouse enter event.
    pub fn mouse_enter(x: i32, y: i32) -> Self {
        Self::MouseEnter { pos: Point::new(x, y) }
    }
    /// Creates a mouse leave event.
    pub fn mouse_leave(x: i32, y: i32) -> Self {
        Self::MouseLeave { pos: Point::new(x, y) }
    }
    /// Creates a key press event.
    pub fn key_press(key: u32, modifiers: u32) -> Self {
        Self::KeyPress { key, modifiers }
    }
    /// Creates a key release event.
    pub fn key_release(key: u32, modifiers: u32) -> Self {
        Self::KeyRelease { key, modifiers }
    }
    /// Creates a committed text input event.
    pub fn text_input(text: impl Into<String>) -> Self {
        Self::TextInput { text: text.into() }
    }
    /// Creates an IME preedit/composition event.
    pub fn ime_preedit(text: impl Into<String>, cursor: usize) -> Self {
        Self::ImePreedit { text: text.into(), cursor }
    }
    /// Creates an IME commit event.
    pub fn ime_commit(text: impl Into<String>) -> Self {
        Self::ImeCommit { text: text.into() }
    }
    /// Creates a paint/repaint request event.
    pub fn paint() -> Self {
        Self::Paint
    }
    /// Creates a resize event.
    pub fn resize(width: u32, height: u32) -> Self {
        Self::Resize { size: Size::new(width, height) }
    }
    /// Creates a timer event.
    pub fn timer(id: u32) -> Self {
        Self::Timer { id }
    }
    /// Creates a mouse wheel event.
    pub fn wheel(delta_x: i32, delta_y: i32, modifiers: u32) -> Self {
        Self::Wheel { delta: Point::new(delta_x, delta_y), modifiers }
    }
    // ── Touch / Gesture constructors ──
    /// Creates a touch begin event.
    #[cfg(feature = "touch")]
    pub fn touch_begin(x: i32, y: i32, touch_id: TouchId) -> Self {
        Self::TouchBegin { pos: Point::new(x, y), touch_id }
    }
    /// Creates a touch end event.
    #[cfg(feature = "touch")]
    pub fn touch_end(x: i32, y: i32, touch_id: TouchId) -> Self {
        Self::TouchEnd { pos: Point::new(x, y), touch_id }
    }
    /// Creates a touch move event.
    #[cfg(feature = "touch")]
    pub fn touch_move(x: i32, y: i32, touch_id: TouchId) -> Self {
        Self::TouchMove { pos: Point::new(x, y), touch_id }
    }
    /// Creates a tap gesture event.
    #[cfg(feature = "touch")]
    pub fn tap(x: i32, y: i32) -> Self {
        Self::Tap { pos: Point::new(x, y) }
    }
    /// Creates a double-tap gesture event.
    #[cfg(feature = "touch")]
    pub fn double_tap(x: i32, y: i32) -> Self {
        Self::DoubleTap { pos: Point::new(x, y) }
    }
    /// Creates a long-press gesture event.
    #[cfg(feature = "touch")]
    pub fn long_press(x: i32, y: i32) -> Self {
        Self::LongPress { pos: Point::new(x, y) }
    }
    /// Creates a swipe gesture event.
    #[cfg(feature = "touch")]
    pub fn swipe(start_x: i32, start_y: i32, end_x: i32, end_y: i32, velocity: f32) -> Self {
        Self::Swipe { start: Point::new(start_x, start_y), end: Point::new(end_x, end_y), velocity }
    }
    /// Creates a pinch gesture event.
    #[cfg(feature = "touch")]
    pub fn pinch(scale: f32) -> Self {
        Self::Pinch { scale }
    }
    /// Creates a rotate gesture event.
    #[cfg(feature = "touch")]
    pub fn rotate(angle: f32) -> Self {
        Self::Rotate { angle }
    }
    /// Creates a drag event.
    #[cfg(feature = "touch")]
    pub fn drag(x: i32, y: i32, touch_id: TouchId, delta_x: i32, delta_y: i32) -> Self {
        Self::Drag { pos: Point::new(x, y), touch_id, delta: Point::new(delta_x, delta_y) }
    }
    /// Creates a two-finger tap gesture event.
    #[cfg(feature = "touch")]
    pub fn two_finger_tap(x: i32, y: i32) -> Self {
        Self::TwoFingerTap { pos: Point::new(x, y) }
    }
    /// Creates a two-finger swipe gesture event.
    #[cfg(feature = "touch")]
    pub fn two_finger_swipe(
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        velocity: f32,
    ) -> Self {
        Self::TwoFingerSwipe {
            centroid_start: Point::new(start_x, start_y),
            centroid_end: Point::new(end_x, end_y),
            velocity,
        }
    }
    /// Creates a fling gesture event.
    #[cfg(feature = "touch")]
    pub fn fling(x: i32, y: i32, vx: i32, vy: i32, touch_id: TouchId) -> Self {
        Self::Fling { pos: Point::new(x, y), velocity: Point::new(vx, vy), touch_id }
    }

    /// Creates a holographic (3D) touch event.
    #[cfg(feature = "holographic")]
    pub fn holographic_touch(x: i32, y: i32, depth: f32, touch_id: TouchId) -> Self {
        Self::HolographicTouch { pos: Point::new(x, y), depth, touch_id }
    }

    /// Creates a pointer press event.
    pub fn pointer_press(pos: Point, button: u32, pressure: f32, tilt_x: f32, tilt_y: f32) -> Self {
        Event::PointerPress { pos, button, pressure, tilt_x, tilt_y }
    }

    /// Creates a pointer move event.
    pub fn pointer_move(pos: Point, pressure: f32, tilt_x: f32, tilt_y: f32) -> Self {
        Event::PointerMove { pos, pressure, tilt_x, tilt_y }
    }

    /// Creates a pointer release event.
    pub fn pointer_release(pos: Point, button: u32, pressure: f32) -> Self {
        Event::PointerRelease { pos, button, pressure }
    }

    /// Creates a gamepad press event.
    pub fn gamepad_press(button: u32) -> Self {
        Event::GamepadPress { button }
    }

    /// Creates a gamepad release event.
    pub fn gamepad_release(button: u32) -> Self {
        Event::GamepadRelease { button }
    }

    /// Creates a gamepad axis event.
    pub fn gamepad_axis(axis: u32, value: f32) -> Self {
        Event::GamepadAxis { axis, value }
    }

    /// Creates a gamepad connected event.
    pub fn gamepad_connected(id: u32) -> Self {
        Event::GamepadConnected { id }
    }

    /// Creates a gamepad disconnected event.
    pub fn gamepad_disconnected(id: u32) -> Self {
        Event::GamepadDisconnected { id }
    }

    /// Creates an orientation changed event.
    pub fn orientation_changed(orientation: ScreenOrientation) -> Self {
        Self::OrientationChanged { orientation }
    }

    /// Creates a quit event.
    pub fn quit() -> Self {
        Self::Quit
    }

    /// Returns the `GestureClass` for touch/gesture variants, or `None` for non-gesture events.
    pub fn gesture_class(&self) -> Option<GestureClass> {
        match self {
            #[cfg(feature = "touch")]
            Self::Tap { .. }
            | Self::DoubleTap { .. }
            | Self::LongPress { .. }
            | Self::Swipe { .. }
            | Self::Fling { .. }
            | Self::Drag { .. } => Some(GestureClass::Single),
            #[cfg(feature = "touch")]
            Self::Pinch { .. }
            | Self::Rotate { .. }
            | Self::TwoFingerTap { .. }
            | Self::TwoFingerSwipe { .. } => Some(GestureClass::Multi),
            #[cfg(feature = "holographic")]
            Self::HolographicTouch { .. } => Some(GestureClass::Holographic),
            _ => None,
        }
    }

    /// Returns `true` if the event is a touch-related variant.
    pub fn is_touch(&self) -> bool {
        #[cfg(feature = "touch")]
        {
            matches!(
                self,
                Self::TouchBegin { .. }
                    | Self::TouchEnd { .. }
                    | Self::TouchMove { .. }
                    | Self::Tap { .. }
                    | Self::DoubleTap { .. }
                    | Self::LongPress { .. }
                    | Self::Swipe { .. }
                    | Self::Pinch { .. }
                    | Self::Rotate { .. }
                    | Self::Drag { .. }
                    | Self::TwoFingerTap { .. }
                    | Self::TwoFingerSwipe { .. }
                    | Self::Fling { .. }
            ) || {
                #[cfg(feature = "holographic")]
                {
                    matches!(self, Self::HolographicTouch { .. })
                }
                #[cfg(not(feature = "holographic"))]
                {
                    false
                }
            }
        }
        #[cfg(not(feature = "touch"))]
        {
            #[cfg(feature = "holographic")]
            {
                matches!(self, Self::HolographicTouch { .. })
            }
            #[cfg(not(feature = "holographic"))]
            {
                false
            }
        }
    }
}
/// Trait implemented by event targets.
pub trait EventHandler {
    /// Handle a single dispatched event.
    fn handle_event(&mut self, event: &Event);
}
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "touch")]
    #[test]
    fn touch_begin_creation() {
        let e = Event::touch_begin(10, 20, 1);
        match e {
            Event::TouchBegin { pos, touch_id } => {
                assert_eq!(pos.x, 10);
                assert_eq!(pos.y, 20);
                assert_eq!(touch_id, 1);
            }
            _ => panic!("expected TouchBegin"),
        }
    }

    #[cfg(feature = "touch")]
    #[test]
    fn touch_end_creation() {
        let e = Event::touch_end(30, 40, 2);
        match e {
            Event::TouchEnd { pos, touch_id } => {
                assert_eq!(pos.x, 30);
                assert_eq!(pos.y, 40);
                assert_eq!(touch_id, 2);
            }
            _ => panic!("expected TouchEnd"),
        }
    }

    #[cfg(feature = "touch")]
    #[test]
    fn tap_gesture_class_is_single() {
        let e = Event::tap(5, 10);
        assert_eq!(e.gesture_class(), Some(GestureClass::Single));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn pinch_gesture_class_is_multi() {
        let e = Event::pinch(0.5);
        assert_eq!(e.gesture_class(), Some(GestureClass::Multi));
    }

    #[test]
    fn mouse_event_gesture_class_is_none() {
        let e = Event::mouse_press(1, 2, 0);
        assert_eq!(e.gesture_class(), None);
    }

    #[test]
    fn text_input_and_ime_events_carry_unicode_text() {
        match Event::text_input("你好") {
            Event::TextInput { text } => assert_eq!(text, "你好"),
            _ => panic!("expected TextInput"),
        }
        match Event::ime_preedit("に", 1) {
            Event::ImePreedit { text, cursor } => {
                assert_eq!(text, "に");
                assert_eq!(cursor, 1);
            }
            _ => panic!("expected ImePreedit"),
        }
        match Event::ime_commit("本") {
            Event::ImeCommit { text } => assert_eq!(text, "本"),
            _ => panic!("expected ImeCommit"),
        }
    }

    #[cfg(feature = "touch")]
    #[test]
    fn is_touch_true_for_touch_variants() {
        assert!(Event::touch_begin(0, 0, 0).is_touch());
        assert!(Event::touch_end(0, 0, 0).is_touch());
        assert!(Event::touch_move(0, 0, 0).is_touch());
        assert!(Event::tap(0, 0).is_touch());
        assert!(Event::double_tap(0, 0).is_touch());
        assert!(Event::long_press(0, 0).is_touch());
        assert!(Event::swipe(0, 0, 10, 10, 100.0).is_touch());
        assert!(Event::pinch(1.0).is_touch());
        assert!(Event::rotate(0.5).is_touch());
        assert!(Event::drag(0, 0, 0, 1, 1).is_touch());
        assert!(Event::two_finger_tap(0, 0).is_touch());
        assert!(Event::two_finger_swipe(0, 0, 10, 10, 50.0).is_touch());
        assert!(Event::fling(0, 0, 1, 1, 0).is_touch());
    }

    #[test]
    fn is_touch_false_for_mouse_variants() {
        assert!(!Event::mouse_press(1, 2, 0).is_touch());
        assert!(!Event::mouse_move(1, 2).is_touch());
        assert!(!Event::key_press(32, 0).is_touch());
        assert!(!Event::paint().is_touch());
        assert!(!Event::quit().is_touch());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn swipe_fields_correct() {
        let e = Event::swipe(0, 0, 100, 200, 500.0);
        match e {
            Event::Swipe { start, end, velocity } => {
                assert_eq!(start.x, 0);
                assert_eq!(start.y, 0);
                assert_eq!(end.x, 100);
                assert_eq!(end.y, 200);
                assert!((velocity - 500.0).abs() < f32::EPSILON);
            }
            _ => panic!("expected Swipe"),
        }
    }

    #[cfg(feature = "touch")]
    #[test]
    fn two_finger_tap_creation() {
        let e = Event::two_finger_tap(15, 25);
        match e {
            Event::TwoFingerTap { pos } => {
                assert_eq!(pos.x, 15);
                assert_eq!(pos.y, 25);
            }
            _ => panic!("expected TwoFingerTap"),
        }
    }

    #[cfg(feature = "touch")]
    #[test]
    fn two_finger_tap_gesture_class_is_multi() {
        let e = Event::two_finger_tap(10, 20);
        assert_eq!(e.gesture_class(), Some(GestureClass::Multi));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn two_finger_swipe_fields_correct() {
        let e = Event::two_finger_swipe(0, 0, 100, 200, 300.0);
        match e {
            Event::TwoFingerSwipe { centroid_start, centroid_end, velocity } => {
                assert_eq!(centroid_start.x, 0);
                assert_eq!(centroid_start.y, 0);
                assert_eq!(centroid_end.x, 100);
                assert_eq!(centroid_end.y, 200);
                assert!((velocity - 300.0).abs() < f32::EPSILON);
            }
            _ => panic!("expected TwoFingerSwipe"),
        }
    }

    #[cfg(feature = "touch")]
    #[test]
    fn two_finger_swipe_gesture_class_is_multi() {
        let e = Event::two_finger_swipe(0, 0, 10, 10, 50.0);
        assert_eq!(e.gesture_class(), Some(GestureClass::Multi));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn fling_creation() {
        let e = Event::fling(50, 60, 10, -5, 3);
        match e {
            Event::Fling { pos, velocity, touch_id } => {
                assert_eq!(pos.x, 50);
                assert_eq!(pos.y, 60);
                assert_eq!(velocity.x, 10);
                assert_eq!(velocity.y, -5);
                assert_eq!(touch_id, 3);
            }
            _ => panic!("expected Fling"),
        }
    }

    #[cfg(feature = "touch")]
    #[test]
    fn fling_gesture_class_is_single() {
        let e = Event::fling(0, 0, 100, 50, 0);
        assert_eq!(e.gesture_class(), Some(GestureClass::Single));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn is_touch_true_for_new_gesture_variants() {
        assert!(Event::two_finger_tap(0, 0).is_touch());
        assert!(Event::two_finger_swipe(0, 0, 10, 10, 50.0).is_touch());
        assert!(Event::fling(0, 0, 1, 1, 0).is_touch());
    }

    #[test]
    fn orientation_changed_creation() {
        let e = Event::orientation_changed(ScreenOrientation::Landscape);
        match e {
            Event::OrientationChanged { orientation } => {
                assert_eq!(orientation, ScreenOrientation::Landscape);
            }
            _ => panic!("expected OrientationChanged"),
        }
    }

    #[test]
    fn orientation_changed_not_touch() {
        let e = Event::orientation_changed(ScreenOrientation::Portrait);
        assert!(!e.is_touch());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn drag_fields_correct() {
        let e = Event::drag(50, 60, 1, 5, -3);
        match e {
            Event::Drag { pos, touch_id, delta } => {
                assert_eq!(pos.x, 50);
                assert_eq!(pos.y, 60);
                assert_eq!(touch_id, 1);
                assert_eq!(delta.x, 5);
                assert_eq!(delta.y, -3);
            }
            _ => panic!("expected Drag"),
        }
    }

    #[test]
    fn pointer_press_constructor() {
        let e = Event::pointer_press(Point::new(10, 20), 0, 0.5, 0.1, 0.2);
        match e {
            Event::PointerPress { pos, button, pressure, tilt_x, tilt_y } => {
                assert_eq!(pos, Point::new(10, 20));
                assert_eq!(button, 0);
                assert!((pressure - 0.5).abs() < 1e-6);
                assert!((tilt_x - 0.1).abs() < 1e-6);
                assert!((tilt_y - 0.2).abs() < 1e-6);
            }
            _ => panic!("Expected PointerPress"),
        }
    }

    #[test]
    fn pointer_move_constructor() {
        let e = Event::pointer_move(Point::new(30, 40), 0.8, -0.1, 0.3);
        match e {
            Event::PointerMove { pos, .. } => {
                assert_eq!(pos, Point::new(30, 40));
            }
            _ => panic!("Expected PointerMove"),
        }
    }

    #[test]
    fn pointer_release_constructor() {
        let e = Event::pointer_release(Point::new(50, 60), 1, 0.0);
        match e {
            Event::PointerRelease { pos, button, .. } => {
                assert_eq!(pos, Point::new(50, 60));
                assert_eq!(button, 1);
            }
            _ => panic!("Expected PointerRelease"),
        }
    }

    #[test]
    fn gamepad_press_constructor() {
        let e = Event::gamepad_press(3);
        match e {
            Event::GamepadPress { button } => assert_eq!(button, 3),
            _ => panic!("Expected GamepadPress"),
        }
    }

    #[test]
    fn gamepad_release_constructor() {
        let e = Event::gamepad_release(7);
        match e {
            Event::GamepadRelease { button } => assert_eq!(button, 7),
            _ => panic!("Expected GamepadRelease"),
        }
    }

    #[test]
    fn gamepad_axis_constructor() {
        let e = Event::gamepad_axis(1, -0.5);
        match e {
            Event::GamepadAxis { axis, value } => {
                assert_eq!(axis, 1);
                assert!((value - (-0.5)).abs() < 1e-6);
            }
            _ => panic!("Expected GamepadAxis"),
        }
    }

    #[test]
    fn gamepad_connected_constructor() {
        let e = Event::gamepad_connected(42);
        match e {
            Event::GamepadConnected { id } => assert_eq!(id, 42),
            _ => panic!("Expected GamepadConnected"),
        }
    }

    #[test]
    fn gamepad_disconnected_constructor() {
        let e = Event::gamepad_disconnected(99);
        match e {
            Event::GamepadDisconnected { id } => assert_eq!(id, 99),
            _ => panic!("Expected GamepadDisconnected"),
        }
    }
}

/// Scheduling priority for queued events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPriority {
    /// High-priority events (e.g. quit/system control).
    High,
    /// Normal-priority events (default user/application traffic).
    Normal,
    /// Idle-priority events, delivered when no higher-priority events exist.
    Idle,
}

/// Async task that can be scheduled on the event loop (BLUE11 R8.2).
pub struct AsyncTask {
    /// Unique task ID.
    pub id: u64,
    /// The closure to execute.
    pub task: Box<dyn FnOnce() + Send>,
}

impl AsyncTask {
    /// Boxes `f` as a task carrying `id`.
    ///
    /// `id` is chosen by the caller and is not checked for uniqueness; it is
    /// carried so a scheduler can refer to the task, and `AsyncTask` itself never
    /// reads it.
    pub fn new<F>(id: u64, f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self { id, task: Box::new(f) }
    }
}

#[cfg(not(alloc_frugal))]
use crate::compat::mpsc::{self, TryRecvError};
#[cfg(not(alloc_frugal))]
use crate::compat::{Mutex, OnceLock};

#[cfg(not(alloc_frugal))]
fn channel() -> &'static (mpsc::Sender<AsyncTask>, Mutex<mpsc::Receiver<AsyncTask>>) {
    static CHANNEL: OnceLock<(mpsc::Sender<AsyncTask>, Mutex<mpsc::Receiver<AsyncTask>>)> =
        OnceLock::new();
    CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (tx, Mutex::new(rx))
    })
}

/// Schedules a closure to run on the event loop thread.
///
/// Safe to call from any thread; the task will be delivered to the event loop
/// via a global `mpsc` channel. The event loop drains tasks each frame via
/// [`drain_tasks()`].
#[cfg(not(alloc_frugal))]
pub fn schedule_task<F>(id: u64, f: F)
where
    F: FnOnce() + Send + 'static,
{
    let task = AsyncTask::new(id, f);
    let (tx, _) = channel();
    // The channel lives for the program's lifetime, so send will never fail.
    let _ = tx.send(task);
}

/// Drain all pending async tasks (called by event loop each frame).
#[cfg(not(alloc_frugal))]
pub fn drain_tasks() {
    let (_, rx_mutex) = channel();
    let Ok(rx) = rx_mutex.lock() else { return };
    loop {
        match rx.try_recv() {
            Ok(task) => (task.task)(),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                // Channel disconnected — no more tasks will arrive.
                break;
            }
        }
    }
}
