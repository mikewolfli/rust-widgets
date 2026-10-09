// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

use super::{A11yState, AccessibilityBridge};
use crate::compat::{HashMap, MiniToString, Mutex, String};
use crate::core::ObjectId;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSString};

extern "C" {
    /// C function from ApplicationServices framework.
    /// void NSAccessibilityPostNotification(id element, NSString *notification);
    fn NSAccessibilityPostNotification(element: id, notification: id);
}

/// macOS NSAccessibility bridge implementation.
pub struct MacOSAccessibilityBridge {
    names: Mutex<HashMap<ObjectId, String>>,
    /// Full accessibility state per widget (D09-A11Y-02), mirrored from `submit_node_state`.
    nodes: Mutex<HashMap<ObjectId, A11yState>>,
    /// Mapping from widget ObjectId to native NSView/NSControl pointer (as *mut c_void).
    native_handles: Mutex<HashMap<ObjectId, usize>>,
}

/// Create an autoreleased `NSString` from a Rust string.
///
/// `NSString::init_str` returns a +1-retained object; left unmanaged it leaks
/// on every accessibility notification. `autorelease` hands ownership to the
/// enclosing autorelease pool so the string is reclaimed after the current run
/// loop turn — the correct lifetime for a value that is only read during the
/// `NSAccessibilityPostNotification` call that follows.
///
/// SAFETY: must be called on a thread with an active autorelease pool (the
/// AppKit main thread always has one). Callers below invoke it on the same
/// thread that owns the accessibility elements.
unsafe fn autoreleased_nsstring(s: &str) -> id {
    NSString::alloc(nil).init_str(s).autorelease()
}

impl MacOSAccessibilityBridge {
    /// Create an empty bridge with no names or native handles registered.
    pub fn new() -> Self {
        Self {
            names: Mutex::new(HashMap::new()),
            nodes: Mutex::new(HashMap::new()),
            native_handles: Mutex::new(HashMap::new()),
        }
    }

    /// Register a native Cocoa handle for the given widget id.
    pub fn register_handle(&self, id: ObjectId, ptr: usize) {
        if let Some(mut handles) = crate::compat::try_lock(&self.native_handles) {
            handles.insert(id, ptr);
        }
    }

    /// Remove a native handle registration.
    pub fn unregister_handle(&self, id: ObjectId) {
        if let Some(mut handles) = crate::compat::try_lock(&self.native_handles) {
            handles.remove(&id);
        }
    }

    /// Post an NSAccessibility notification on the native element for the given widget.
    fn post_notification(&self, id: ObjectId, notification_name: &str) -> bool {
        let ptr = match crate::compat::try_lock(&self.native_handles) {
            Some(h) => h.get(&id).copied(),
            None => return false,
        };
        let Some(ptr) = ptr else { return false };
        // SAFETY: `ptr` is an object pointer this backend stored from a live native
        // accessibility element; the `id` transmute is the standard pointer-width
        // reinterpretation (usize -> object pointer), and the AppKit notification
        // call only reads the element. Wrapped in `catch_unwind` for safety across
        // the FFI boundary.
        let result = std::panic::catch_unwind(|| unsafe {
            let element: id = std::mem::transmute(ptr);
            let ns_name = autoreleased_nsstring(notification_name);
            // C function from ApplicationServices: NSAccessibilityPostNotification
            NSAccessibilityPostNotification(element, ns_name);
            true
        });
        result.unwrap_or(false)
    }
}

crate::impl_default_via_new!(MacOSAccessibilityBridge);

impl AccessibilityBridge for MacOSAccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        if let Some(mut names) = crate::compat::try_lock(&self.names) {
            names.insert(id, name.to_string());
        }
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        crate::compat::try_lock(&self.names).and_then(|names| names.get(&id).cloned())
    }

    /// Stores the full node state (D09-A11Y-02) alongside the name, so a VoiceOver client can be
    /// handed a control's role, value and checked state rather than only its name.
    fn submit_node_state(&self, id: ObjectId, state: &A11yState) {
        if let Some(mut names) = crate::compat::try_lock(&self.names) {
            names.insert(id, state.label.clone());
        }
        if let Some(mut nodes) = crate::compat::try_lock(&self.nodes) {
            nodes.insert(id, state.clone());
        }
    }

    fn node_state(&self, id: ObjectId) -> Option<A11yState> {
        crate::compat::try_lock(&self.nodes).and_then(|nodes| nodes.get(&id).cloned())
    }

    /// Removes the name and state entries on unmount (D09-A11Y-03), so the maps do not keep one
    /// entry per historical widget id.
    fn unregister_node(&self, id: ObjectId) {
        if let Some(mut names) = crate::compat::try_lock(&self.names) {
            names.remove(&id);
        }
        if let Some(mut nodes) = crate::compat::try_lock(&self.nodes) {
            nodes.remove(&id);
        }
    }

    fn node_count(&self) -> usize {
        crate::compat::try_lock(&self.nodes).map(|nodes| nodes.len()).unwrap_or(0)
    }

    fn notify_name_changed(&self, id: ObjectId) {
        self.post_notification(id, "NSAccessibilityNameChangedNotification");
    }

    fn notify_value_changed(&self, id: ObjectId) {
        self.post_notification(id, "NSAccessibilityValueChangedNotification");
    }

    fn notify_state_changed(&self, id: ObjectId) {
        // A state change (enabled/checked/value-of-state) must be reported as a
        // value change, not a focus change — VoiceOver actions state transitions
        // and focus moves differently. Posting the focus constant here (as the
        // code once did) makes assistive output wrong; the Windows backend
        // already distinguishes EVENT_OBJECT_STATECHANGE from EVENT_OBJECT_FOCUS.
        self.post_notification(id, "NSAccessibilityValueChangedNotification");
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
    // SAFETY: `element_ptr` is an object pointer this crate created; the `id`
    // transmute is the standard usize -> object-pointer reinterpretation, and
    // `NSAccessibilityPostNotification` only reads the element it is given.
    unsafe {
        let element: id = std::mem::transmute(element_ptr);
        let ns_name = autoreleased_nsstring(notification);
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

    /// D09-A11Y-02: the full state is stored and read back, not only the label.
    #[test]
    fn submitted_state_is_stored_in_full_not_only_the_label() {
        use crate::platform::accessibility::{A11yRole, A11yState};
        let bridge = MacOSAccessibilityBridge::new();
        let full = A11yState {
            role: A11yRole::CheckBox,
            label: "Dark mode".to_string(),
            description: "Toggle the theme".to_string(),
            enabled: true,
            checked: Some(true),
            children: vec![8, 9],
            ..A11yState::default()
        };
        bridge.submit_node_state(4, &full);
        assert_eq!(bridge.node_state(4), Some(full), "every field round-trips");
        assert_eq!(bridge.accessibility_name(4).as_deref(), Some("Dark mode"));
    }

    /// D09-A11Y-03: unmount removes both the name and the state entry.
    #[test]
    fn unmount_removes_entries_so_counts_return_to_baseline() {
        use crate::platform::accessibility::{A11yRole, A11yState};
        let bridge = MacOSAccessibilityBridge::new();
        let id = 12u64;
        bridge.submit_node_state(
            id,
            &A11yState { role: A11yRole::Button, label: "Go".to_string(), ..A11yState::default() },
        );
        assert_eq!(bridge.node_count(), 1);
        assert!(bridge.accessibility_name(id).is_some());

        bridge.unregister_node(id);
        assert_eq!(bridge.node_count(), 0, "the node is gone");
        assert!(bridge.accessibility_name(id).is_none(), "and the name is gone");
        assert!(bridge.node_state(id).is_none(), "and the state is gone");
    }
}
