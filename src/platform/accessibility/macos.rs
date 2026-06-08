//! macOS NSAccessibility protocol bridge.
//!
//! Exposes widget information to VoiceOver and other assistive technologies
//! on macOS via NSAccessibilityPostNotification.
//!
//! Widget handles (native NSView/NSControl pointers) must be registered with
//! the bridge for notifications to reach the correct accessibility element.

use super::AccessibilityBridge;
use crate::core::ObjectId;
use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use std::collections::HashMap;
use std::sync::Mutex;

extern "C" {
    /// C function from ApplicationServices framework.
    /// void NSAccessibilityPostNotification(id element, NSString *notification);
    fn NSAccessibilityPostNotification(element: id, notification: id);
}

/// macOS NSAccessibility bridge implementation.
pub struct MacOSAccessibilityBridge {
    names: Mutex<HashMap<ObjectId, String>>,
    /// Mapping from widget ObjectId to native NSView/NSControl pointer (as *mut c_void).
    native_handles: Mutex<HashMap<ObjectId, usize>>,
}

impl MacOSAccessibilityBridge {
    pub fn new() -> Self {
        Self { names: Mutex::new(HashMap::new()), native_handles: Mutex::new(HashMap::new()) }
    }

    /// Register a native Cocoa handle for the given widget id.
    pub fn register_handle(&self, id: ObjectId, ptr: usize) {
        if let Ok(mut handles) = self.native_handles.lock() {
            handles.insert(id, ptr);
        }
    }

    /// Remove a native handle registration.
    pub fn unregister_handle(&self, id: ObjectId) {
        if let Ok(mut handles) = self.native_handles.lock() {
            handles.remove(&id);
        }
    }

    /// Post an NSAccessibility notification on the native element for the given widget.
    fn post_notification(&self, id: ObjectId, notification_name: &str) -> bool {
        let ptr = match self.native_handles.lock() {
            Ok(h) => h.get(&id).copied(),
            Err(_) => return false,
        };
        let Some(ptr) = ptr else { return false };
        let result = std::panic::catch_unwind(|| unsafe {
            let element: id = std::mem::transmute(ptr);
            let ns_name = NSString::alloc(nil).init_str(notification_name);
            // C function from ApplicationServices: NSAccessibilityPostNotification
            NSAccessibilityPostNotification(element, ns_name);
            true
        });
        result.unwrap_or(false)
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

    fn notify_name_changed(&self, id: ObjectId) {
        self.post_notification(id, "NSAccessibilityNameChangedNotification");
    }

    fn notify_value_changed(&self, id: ObjectId) {
        self.post_notification(id, "NSAccessibilityValueChangedNotification");
    }

    fn notify_state_changed(&self, id: ObjectId) {
        self.post_notification(id, "NSAccessibilityFocusedUIElementChangedNotification");
    }

    fn notify_focus_changed(&self, id: ObjectId) {
        self.post_notification(id, "NSAccessibilityFocusedUIElementChangedNotification");
    }
}
