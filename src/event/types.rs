//! Event types and handler trait.

use crate::core::{Point, Size};

/// Event payload variants routed through the event loop.
#[derive(Debug, Clone)]
pub enum Event {
    /// Pointer moved inside active surface.
    MouseMove { pos: Point },
    /// Pointer/button press.
    MousePress { pos: Point, button: u32 },
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
