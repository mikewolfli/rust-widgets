//! macOS NSAccessibility protocol bridge.
//!
//! This module will implement the NSAccessibility protocol methods
//! to expose widget information to VoiceOver and other assistive
//! technologies on macOS.
//!
//! Currently a placeholder; full implementation requires `objc2` crate bindings.

use super::AccessibilityBridge;
use crate::core::ObjectId;
use std::collections::HashMap;
use std::sync::Mutex;

/// macOS NSAccessibility bridge implementation.
pub struct MacOSAccessibilityBridge {
    names: Mutex<HashMap<ObjectId, String>>,
}

impl MacOSAccessibilityBridge {
    pub fn new() -> Self {
        Self { names: Mutex::new(HashMap::new()) }
    }
}

impl Default for MacOSAccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessibilityBridge for MacOSAccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        if let Ok(mut names) = self.names.lock() {
            names.insert(id, name.to_string());
        }
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        self.names.lock().ok().and_then(|names| names.get(&id).cloned())
    }

    fn notify_name_changed(&self, _id: ObjectId) {
        // TODO: Post NSAccessibilityNotificationName accessibilityNameChanged
        log::info!("[macos-a11y] notify_name_changed (not yet implemented)");
    }

    fn notify_value_changed(&self, _id: ObjectId) {
        log::info!("[macos-a11y] notify_value_changed (not yet implemented)");
    }

    fn notify_state_changed(&self, _id: ObjectId) {
        log::info!("[macos-a11y] notify_state_changed (not yet implemented)");
    }

    fn notify_focus_changed(&self, _id: ObjectId) {
        log::info!("[macos-a11y] notify_focus_changed (not yet implemented)");
    }
}
