//! Custom-painted control backend scaffold.

use crate::compat::Mutex;
use crate::control_backend::types::CustomControlState;
use crate::core::ObjectId;

/// Custom-painted control backend scaffold.
pub struct CustomPaintControlBackend {
    pub(crate) state: Mutex<CustomControlState>,
}

impl CustomPaintControlBackend {
    /// Create custom-painted control backend.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CustomControlState {
                next_widget_id: 1,
                ..CustomControlState::default()
            }),
        }
    }

    pub(crate) fn alloc_widget_id(&self) -> ObjectId {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let widget_id = state.next_widget_id;
        state.next_widget_id = state.next_widget_id.saturating_add(1);
        widget_id
    }
}

impl Default for CustomPaintControlBackend {
    fn default() -> Self {
        Self::new()
    }
}
