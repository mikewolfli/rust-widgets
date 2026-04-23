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
