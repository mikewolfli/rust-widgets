//! Event types and handler trait.
use crate::core::{Point, Size};
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
    /// Creates a quit event.
    pub fn quit() -> Self {
        Self::Quit
    }
}
/// Trait implemented by event targets.
pub trait EventHandler {
    /// Handle a single dispatched event.
    fn handle_event(&mut self, event: &Event);
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
