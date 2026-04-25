//! Event types and handler trait.
use crate::core::{Point, Size};
/// A unique identifier for a touch contact point (used by `touch` and `holographic` features).
pub type TouchId = u64;

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
    MouseDown((Point, u32)),
    /// Legacy pointer/button release payload.
    MouseUp((Point, u32)),
    /// Legacy pointer move payload.
    MouseMoveLegacy((Point, u32)),
    /// Legacy keyboard press payload.
    KeyDown((u32, u32)),
    /// Legacy keyboard release payload.
    KeyUp((u32, u32)),
    /// Legacy focus gained event.
    FocusGained,
    /// Legacy focus lost event.
    FocusLost,
    /// Pointer moved inside active surface.
    MouseMove { pos: Point },
    /// Pointer/button press.
    MousePress { pos: Point, button: u32 },
    /// Pointer double-click.
    MouseDoubleClick { pos: Point, button: u32 },
    /// Pointer/button release.
    MouseRelease { pos: Point, button: u32 },
    /// Pointer entered widget bounds.
    MouseEnter { pos: Point },
    /// Pointer left widget bounds.
    MouseLeave { pos: Point },
    /// Keyboard key press.
    KeyPress { key: u32, modifiers: u32 },
    /// Keyboard key release.
    KeyRelease { key: u32, modifiers: u32 },
    /// Repaint request.
    Paint,
    /// Resize notification.
    Resize { size: Size },
    /// Timer fired.
    Timer { id: u32 },
    /// Mouse wheel / scroll event.
    Wheel { delta: Point, modifiers: u32 },
    /// Free-form custom event payload.
    Custom { name: String, payload: Vec<u8> },
    /// Event loop shutdown signal.
    Quit,
    // ── Touch / Gesture events (gated behind `touch` feature) ──
    /// Finger touched surface (replaces MouseDown on touch devices).
    #[cfg(feature = "touch")]
    TouchBegin { pos: Point, touch_id: TouchId },
    /// Finger lifted from surface (replaces MouseUp on touch devices).
    #[cfg(feature = "touch")]
    TouchEnd { pos: Point, touch_id: TouchId },
    /// Finger moved on surface (replaces MouseMove on touch devices).
    #[cfg(feature = "touch")]
    TouchMove { pos: Point, touch_id: TouchId },
    /// Quick tap-and-release gesture (≈ click).
    #[cfg(feature = "touch")]
    Tap { pos: Point },
    /// Two rapid taps in succession (≈ double-click).
    #[cfg(feature = "touch")]
    DoubleTap { pos: Point },
    /// Finger held stationary ≥ 500ms.
    #[cfg(feature = "touch")]
    LongPress { pos: Point },
    /// Rapid linear finger motion.
    #[cfg(feature = "touch")]
    Swipe {
        start: Point,
        end: Point,
        velocity: f32,
    },
    /// Two-finger pinch (scale < 1 = zoom out, > 1 = zoom in).
    #[cfg(feature = "touch")]
    Pinch { scale: f32 },
    /// Two-finger rotation in radians.
    #[cfg(feature = "touch")]
    Rotate { angle: f32 },
    /// Finger drag with motion tracking.
    #[cfg(feature = "touch")]
    Drag {
        pos: Point,
        touch_id: TouchId,
        delta: Point,
    },
    // ── Holographic / 3D events (BLUE8 P4-5, gated behind `holographic` feature) ──
    /// 3D touch/gesture with depth information (holographic).
    #[cfg(feature = "holographic")]
    HolographicTouch {
        pos: Point,
        depth: f32,
        touch_id: TouchId,
    },
}
impl Event {
    /// Creates a mouse press event.
    pub fn mouse_press(x: i32, y: i32, button: u32) -> Self {
        Self::MousePress {
            pos: Point::new(x, y),
            button,
        }
    }
    /// Creates a mouse release event.
    pub fn mouse_release(x: i32, y: i32, button: u32) -> Self {
        Self::MouseRelease {
            pos: Point::new(x, y),
            button,
        }
    }
    /// Creates a mouse double-click event.
    pub fn mouse_double_click(x: i32, y: i32, button: u32) -> Self {
        Self::MouseDoubleClick {
            pos: Point::new(x, y),
            button,
        }
    }
    /// Creates a mouse move event.
    pub fn mouse_move(x: i32, y: i32) -> Self {
        Self::MouseMove {
            pos: Point::new(x, y),
        }
    }
    /// Creates a mouse enter event.
    pub fn mouse_enter(x: i32, y: i32) -> Self {
        Self::MouseEnter {
            pos: Point::new(x, y),
        }
    }
    /// Creates a mouse leave event.
    pub fn mouse_leave(x: i32, y: i32) -> Self {
        Self::MouseLeave {
            pos: Point::new(x, y),
        }
    }
    /// Creates a key press event.
    pub fn key_press(key: u32, modifiers: u32) -> Self {
        Self::KeyPress { key, modifiers }
    }
    /// Creates a key release event.
    pub fn key_release(key: u32, modifiers: u32) -> Self {
        Self::KeyRelease { key, modifiers }
    }
    /// Creates a paint/repaint request event.
    pub fn paint() -> Self {
        Self::Paint
    }
    /// Creates a resize event.
    pub fn resize(width: u32, height: u32) -> Self {
        Self::Resize {
            size: Size::new(width, height),
        }
    }
    /// Creates a timer event.
    pub fn timer(id: u32) -> Self {
        Self::Timer { id }
    }
    /// Creates a mouse wheel event.
    pub fn wheel(delta_x: i32, delta_y: i32, modifiers: u32) -> Self {
        Self::Wheel {
            delta: Point::new(delta_x, delta_y),
            modifiers,
        }
    }
    // ── Touch / Gesture constructors ──
    /// Creates a touch begin event.
    #[cfg(feature = "touch")]
    pub fn touch_begin(x: i32, y: i32, touch_id: TouchId) -> Self {
        Self::TouchBegin {
            pos: Point::new(x, y),
            touch_id,
        }
    }
    /// Creates a touch end event.
    #[cfg(feature = "touch")]
    pub fn touch_end(x: i32, y: i32, touch_id: TouchId) -> Self {
        Self::TouchEnd {
            pos: Point::new(x, y),
            touch_id,
        }
    }
    /// Creates a touch move event.
    #[cfg(feature = "touch")]
    pub fn touch_move(x: i32, y: i32, touch_id: TouchId) -> Self {
        Self::TouchMove {
            pos: Point::new(x, y),
            touch_id,
        }
    }
    /// Creates a tap gesture event.
    #[cfg(feature = "touch")]
    pub fn tap(x: i32, y: i32) -> Self {
        Self::Tap {
            pos: Point::new(x, y),
        }
    }
    /// Creates a double-tap gesture event.
    #[cfg(feature = "touch")]
    pub fn double_tap(x: i32, y: i32) -> Self {
        Self::DoubleTap {
            pos: Point::new(x, y),
        }
    }
    /// Creates a long-press gesture event.
    #[cfg(feature = "touch")]
    pub fn long_press(x: i32, y: i32) -> Self {
        Self::LongPress {
            pos: Point::new(x, y),
        }
    }
    /// Creates a swipe gesture event.
    #[cfg(feature = "touch")]
    pub fn swipe(start_x: i32, start_y: i32, end_x: i32, end_y: i32, velocity: f32) -> Self {
        Self::Swipe {
            start: Point::new(start_x, start_y),
            end: Point::new(end_x, end_y),
            velocity,
        }
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
        Self::Drag {
            pos: Point::new(x, y),
            touch_id,
            delta: Point::new(delta_x, delta_y),
        }
    }

    /// Creates a holographic (3D) touch event.
    #[cfg(feature = "holographic")]
    pub fn holographic_touch(x: i32, y: i32, depth: f32, touch_id: TouchId) -> Self {
        Self::HolographicTouch {
            pos: Point::new(x, y),
            depth,
            touch_id,
        }
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
            | Self::Swipe { .. } => Some(GestureClass::Single),
            #[cfg(feature = "touch")]
            Self::Pinch { .. } | Self::Rotate { .. } => Some(GestureClass::Multi),
            #[cfg(feature = "holographic")]
            Self::HolographicTouch { .. } => Some(GestureClass::Holographic),
            _ => None,
        }
    }

    /// Returns `true` if the event is a touch-related variant.
    pub fn is_touch(&self) -> bool {
        #[cfg(feature = "touch")]
        {
            return matches!(
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
            ) || {
                #[cfg(feature = "holographic")]
                {
                    matches!(self, Self::HolographicTouch { .. })
                }
                #[cfg(not(feature = "holographic"))]
                {
                    false
                }
            };
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
            Event::Swipe {
                start,
                end,
                velocity,
            } => {
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
    fn drag_fields_correct() {
        let e = Event::drag(50, 60, 1, 5, -3);
        match e {
            Event::Drag {
                pos,
                touch_id,
                delta,
            } => {
                assert_eq!(pos.x, 50);
                assert_eq!(pos.y, 60);
                assert_eq!(touch_id, 1);
                assert_eq!(delta.x, 5);
                assert_eq!(delta.y, -3);
            }
            _ => panic!("expected Drag"),
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
