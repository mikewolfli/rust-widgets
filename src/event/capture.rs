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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager_has_no_capture() {
        let mgr = PointerCaptureManager::new();
        assert!(mgr.capturing_widget().is_none());
    }

    #[test]
    fn test_set_capture_returns_true_for_new_capture() {
        let mut mgr = PointerCaptureManager::new();
        assert!(mgr.set_capture(42));
        assert_eq!(mgr.capturing_widget(), Some(42));
    }

    #[test]
    fn test_set_capture_returns_false_for_same_widget() {
        let mut mgr = PointerCaptureManager::new();
        assert!(mgr.set_capture(42));
        assert!(!mgr.set_capture(42)); // already captured
    }

    #[test]
    fn test_set_capture_replaces_previous_capture() {
        let mut mgr = PointerCaptureManager::new();
        mgr.set_capture(10);
        assert!(mgr.set_capture(20)); // replace
        assert_eq!(mgr.capturing_widget(), Some(20));
    }

    #[test]
    fn test_release_capture_returns_true_when_capturing() {
        let mut mgr = PointerCaptureManager::new();
        mgr.set_capture(42);
        assert!(mgr.release_capture());
        assert!(mgr.capturing_widget().is_none());
    }

    #[test]
    fn test_release_capture_returns_false_when_not_capturing() {
        let mut mgr = PointerCaptureManager::new();
        assert!(!mgr.release_capture());
    }

    #[test]
    fn test_has_capture() {
        let mut mgr = PointerCaptureManager::new();
        assert!(!mgr.has_capture(42));
        mgr.set_capture(42);
        assert!(mgr.has_capture(42));
        assert!(!mgr.has_capture(99));
        mgr.release_capture();
        assert!(!mgr.has_capture(42));
    }
}
