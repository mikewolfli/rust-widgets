//! Pointer capture management.
use crate::core::ObjectId;
/// Manages pointer capture for drag operations.
#[derive(Debug, Default)]
pub struct PointerCaptureManager {
    /// Widget currently capturing pointer events, if any.
    capturing_widget: Option<ObjectId>,
}
impl PointerCaptureManager {
    /// Creates a new pointer capture manager.
    pub fn new() -> Self {
        Self::default()
    }
    /// Returns the widget currently capturing pointer events, if any.
    pub fn capturing_widget(&self) -> Option<ObjectId> {
        self.capturing_widget
    }
    /// Sets pointer capture to a widget.
    pub fn set_capture(&mut self, widget_id: ObjectId) -> bool {
        if self.capturing_widget == Some(widget_id) {
            return false;
        }
        self.capturing_widget = Some(widget_id);
        true
    }
    /// Releases pointer capture from any widget.
    pub fn release_capture(&mut self) -> bool {
        if self.capturing_widget.is_none() {
            return false;
        }
        self.capturing_widget = None;
        true
    }
    /// Checks if a widget has pointer capture.
    pub fn has_capture(&self, widget_id: ObjectId) -> bool {
        self.capturing_widget == Some(widget_id)
    }
}
