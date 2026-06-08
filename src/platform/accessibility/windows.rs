//! Windows UIAutomation accessibility bridge (stub).
//!
//! Will use IUIAutomation Provider API to expose widget tree to screen readers.
//! Reference: UIAutomation MSDN, IRawElementProviderSimple, UiaRaiseAutomationEvent

use super::AccessibilityBridge;
use crate::core::ObjectId;

/// Windows UIAutomation bridge — placeholder.
///
/// This will be implemented with the IUIAutomation COM interface.
/// For now, all notification methods are stubs.
pub struct WindowsAccessibilityBridge {
    names: std::sync::Mutex<std::collections::HashMap<ObjectId, String>>,
}

impl WindowsAccessibilityBridge {
    pub fn new() -> Self {
        Self { names: std::sync::Mutex::new(std::collections::HashMap::new()) }
    }
}

impl Default for WindowsAccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessibilityBridge for WindowsAccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        self.names.lock().unwrap().insert(id, name.to_string());
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        self.names.lock().unwrap().get(&id).cloned()
    }

    fn notify_name_changed(&self, _id: ObjectId) {
        log::info!(
            "[Windows UIA] notify_name_changed: placeholder — will use UiaRaiseAutomationEvent"
        );
    }

    fn notify_value_changed(&self, _id: ObjectId) {
        log::info!("[Windows UIA] notify_value_changed: placeholder");
    }

    fn notify_state_changed(&self, _id: ObjectId) {
        log::info!("[Windows UIA] notify_state_changed: placeholder");
    }

    fn notify_focus_changed(&self, _id: ObjectId) {
        log::info!("[Windows UIA] notify_focus_changed: placeholder — will use UiaRaiseAutomationEvent(UIA_AutomationFocusChangedEventId)");
    }
}
