//! macOS NSAccessibility protocol bridge.
//!
//! Exposes widget information to VoiceOver and other assistive technologies
//! on macOS via NSAccessibilityPostNotification.
//!
//! Widget handles (native NSView/NSControl pointers) must be registered with
//! the bridge for notifications to reach the correct accessibility element.
//!
//! ## NSAccessibility protocol helpers (cfg-gated)
//!
//! The `ns_accessibility_role()` and `ns_accessibility_subrole()` functions
//! map [`super::A11yRole`] to their corresponding `NSAccessibilityRole`
//! string constants for NSAccessibility protocol conformance.

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

// ─── NSAccessibility protocol helpers ───────────────────────────────────

/// Map an [`super::A11yRole`] to the corresponding `NSAccessibilityRole`
/// string constant used by the NSAccessibility protocol.
///
/// Reference: <https://developer.apple.com/documentation/appkit/nsaccessibilityrole>
#[cfg(target_os = "macos")]
pub fn ns_accessibility_role(role: &super::A11yRole) -> &'static str {
    match role {
        super::A11yRole::Button => "NSAccessibilityButtonRole",
        super::A11yRole::Label | super::A11yRole::Heading | super::A11yRole::Paragraph => {
            "NSAccessibilityStaticTextRole"
        }
        super::A11yRole::TextField => "NSAccessibilityTextFieldRole",
        super::A11yRole::CheckBox => "NSAccessibilityCheckBoxRole",
        super::A11yRole::RadioButton => "NSAccessibilityRadioButtonRole",
        super::A11yRole::Slider => "NSAccessibilitySliderRole",
        super::A11yRole::ProgressBar => "NSAccessibilityProgressIndicatorRole",
        super::A11yRole::List => "NSAccessibilityListRole",
        super::A11yRole::Table => "NSAccessibilityTableRole",
        super::A11yRole::Image => "NSAccessibilityImageRole",
        super::A11yRole::Link => "NSAccessibilityLinkRole",
        super::A11yRole::Group => "NSAccessibilityGroupRole",
        super::A11yRole::Window => "NSAccessibilityWindowRole",
        super::A11yRole::Dialog | super::A11yRole::Alert => "NSAccessibilityDialogRole",
        super::A11yRole::Menu => "NSAccessibilityMenuRole",
        super::A11yRole::MenuItem => "NSAccessibilityMenuItemRole",
        super::A11yRole::Tab => "NSAccessibilityTabRole",
        super::A11yRole::Switch => "NSAccessibilityCheckBoxRole",
        super::A11yRole::ComboBox => "NSAccessibilityComboBoxRole",
        super::A11yRole::SpinButton => "NSAccessibilityIncrementorRole",
        super::A11yRole::StatusBar => "NSAccessibilityGroupRole",
        super::A11yRole::ToolTip => "NSAccessibilityHelpTagRole",
        super::A11yRole::Tree => "NSAccessibilityOutlineRole",
        super::A11yRole::Unknown => "NSAccessibilityUnknownRole",
    }
}

/// Map an [`super::A11yRole`] to an optional `NSAccessibilitySubrole`
/// string constant for more precise element classification.
///
/// Reference: <https://developer.apple.com/documentation/appkit/nsaccessibilitysubrole>
#[cfg(target_os = "macos")]
pub fn ns_accessibility_subrole(role: &super::A11yRole) -> Option<&'static str> {
    match role {
        super::A11yRole::Switch => Some("NSAccessibilitySwitchSubrole"),
        super::A11yRole::ToolTip => Some("NSAccessibilityToolbarSubrole"),
        _ => None,
    }
}

/// Convenience function to post any NSAccessibility notification string.
#[cfg(target_os = "macos")]
pub fn post_ns_accessibility_notification(element_ptr: usize, notification: &str) {
    unsafe {
        let element: id = std::mem::transmute(element_ptr);
        let ns_name = NSString::alloc(nil).init_str(notification);
        NSAccessibilityPostNotification(element, ns_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_mapping() {
        // Role strings should match expected NSAccessibilityRole constants
        assert_eq!(
            ns_accessibility_role(&crate::platform::accessibility::A11yRole::Button),
            "NSAccessibilityButtonRole"
        );
        assert_eq!(
            ns_accessibility_role(&crate::platform::accessibility::A11yRole::TextField),
            "NSAccessibilityTextFieldRole"
        );
        assert_eq!(
            ns_accessibility_role(&crate::platform::accessibility::A11yRole::CheckBox),
            "NSAccessibilityCheckBoxRole"
        );
        assert_eq!(
            ns_accessibility_role(&crate::platform::accessibility::A11yRole::Window),
            "NSAccessibilityWindowRole"
        );
        assert_eq!(
            ns_accessibility_role(&crate::platform::accessibility::A11yRole::Unknown),
            "NSAccessibilityUnknownRole"
        );
    }

    #[test]
    fn test_subrole_mapping() {
        assert_eq!(
            ns_accessibility_subrole(&crate::platform::accessibility::A11yRole::Switch),
            Some("NSAccessibilitySwitchSubrole")
        );
        assert_eq!(
            ns_accessibility_subrole(&crate::platform::accessibility::A11yRole::Button),
            None
        );
    }

    #[test]
    fn test_bridge_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<MacOSAccessibilityBridge>();
        assert_sync::<MacOSAccessibilityBridge>();
    }
}
