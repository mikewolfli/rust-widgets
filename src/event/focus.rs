//! Focus management for widgets.

use crate::core::ObjectId;
use crate::signal::{ConnectionScope, GenericSignal};

/// Manages keyboard focus across widgets.
#[derive(Debug)]
pub struct FocusManager {
    /// Currently focused widget, if any.
    focused_widget: Option<ObjectId>,
    /// Signal emitted when focus changes.
    pub focus_changed: GenericSignal,
    /// Scoped connections for focus tracking.
    connection_scope: ConnectionScope,
}

impl FocusManager {
    /// Creates a new focus manager.
    pub fn new() -> Self {
        Self {
            focused_widget: None,
            focus_changed: GenericSignal::new(),
            connection_scope: ConnectionScope::new(),
        }
    }

    /// Returns the currently focused widget, if any.
    pub fn focused_widget(&self) -> Option<ObjectId> {
        self.focused_widget
    }

    /// Sets focus to a widget.
    pub fn set_focus(&mut self, widget_id: ObjectId) -> bool {
        if self.focused_widget == Some(widget_id) {
            return false;
        }
        self.focused_widget = Some(widget_id);
        self.focus_changed.emit();
        true
    }

    /// Clears focus from any widget.
    pub fn clear_focus(&mut self) -> bool {
        if self.focused_widget.is_none() {
            return false;
        }
        self.focused_widget = None;
        self.focus_changed.emit();
        true
    }

    /// Checks if a widget has focus.
    pub fn has_focus(&self, widget_id: ObjectId) -> bool {
        self.focused_widget == Some(widget_id)
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}
