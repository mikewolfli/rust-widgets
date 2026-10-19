// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `NSAccessibility` bridge for the macOS objc2 backend (BLUE28 B28.13).
//!
//! # What a screen reader can read
//!
//! Every canvas view answers `isAccessibilityElement`, `accessibilityRole`,
//! `accessibilityLabel` and `accessibilityFrame` from the live widget registry (see
//! `macos_objc2::native`). VoiceOver therefore announces the focused control's label and role,
//! and an accessibility navigation step finds one element per canvas rather than an opaque
//! `NSView`.
//!
//! This module owns the notification half: the same stored node state the Windows/cocoa bridges
//! keep, plus the native pointer table `NSAccessibilityPostNotification` needs. The posting call
//! is `cfg(target_os = "macos")`-gated so the state half and its tests compile and run on any host.
//!
//! # Why one process-wide bridge
//!
//! AppKit has one accessibility server per application, and the canvas callbacks register
//! themselves as they are created. A global keeps `Platform::accessibility_bridge()` and the
//! view registration pointing at the same table without threading a pointer through every view.

use crate::compat::{lock, HashMap, Mutex, String};
use crate::core::ObjectId;
use crate::platform::accessibility::{A11yState, AccessibilityBridge};
use std::sync::OnceLock;

/// The bridge `Platform::accessibility_bridge()` hands out.
pub(crate) fn bridge() -> &'static MacOSObjc2AccessibilityBridge {
    static BRIDGE: OnceLock<MacOSObjc2AccessibilityBridge> = OnceLock::new();
    BRIDGE.get_or_init(MacOSObjc2AccessibilityBridge::new)
}

/// Which AppKit accessibility notification to post.
#[derive(Clone, Copy)]
enum NotificationName {
    NameChanged,
    ValueChanged,
    FocusedElementChanged,
}

/// `NSAccessibility` notification bridge backed by the widget registry.
pub struct MacOSObjc2AccessibilityBridge {
    names: Mutex<HashMap<ObjectId, String>>,
    nodes: Mutex<HashMap<ObjectId, A11yState>>,
    handles: Mutex<HashMap<ObjectId, usize>>,
}

impl MacOSObjc2AccessibilityBridge {
    /// Creates an empty bridge.
    pub fn new() -> Self {
        Self {
            names: Mutex::new(HashMap::new()),
            nodes: Mutex::new(HashMap::new()),
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Records the `NSView` that draws `id`, so notifications can name it.
    pub fn register_handle(&self, id: ObjectId, view_ptr: usize) {
        if view_ptr != 0 {
            lock(&self.handles).insert(id, view_ptr);
        }
    }

    /// Drops a view registration when its window goes away.
    pub fn unregister_handle(&self, id: ObjectId) {
        lock(&self.handles).remove(&id);
    }

    /// Posts one accessibility notification for the element drawing `id`.
    fn post(&self, id: ObjectId, notification: NotificationName) {
        let ptr = lock(&self.handles).get(&id).copied();
        let Some(ptr) = ptr else { return };
        #[cfg(target_os = "macos")]
        {
            use objc2::runtime::AnyObject;

            // SAFETY: `ptr` was registered by the canvas view that retains the object for the
            // window's lifetime (`NSWindow` owns its content view). The notification call only
            // reads the element.
            let element = unsafe { &*(ptr as *const AnyObject) };
            let name = unsafe {
                match notification {
                    NotificationName::NameChanged => {
                        objc2_app_kit::NSAccessibilityTitleChangedNotification
                    }
                    NotificationName::ValueChanged => {
                        objc2_app_kit::NSAccessibilityValueChangedNotification
                    }
                    NotificationName::FocusedElementChanged => {
                        objc2_app_kit::NSAccessibilityFocusedUIElementChangedNotification
                    }
                }
            };
            // SAFETY: `name` is one of AppKit's immutable notification constants and `element`
            // is the live view registered above.
            unsafe { objc2_app_kit::NSAccessibilityPostNotification(element, name) };
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (ptr, notification);
        }
    }
}

crate::impl_default_via_new!(MacOSObjc2AccessibilityBridge);

impl AccessibilityBridge for MacOSObjc2AccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        lock(&self.names).insert(id, name.to_string());
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        lock(&self.names).get(&id).cloned()
    }

    fn submit_node_state(&self, id: ObjectId, state: &A11yState) {
        lock(&self.names).insert(id, state.label.clone());
        lock(&self.nodes).insert(id, state.clone());
    }

    fn node_state(&self, id: ObjectId) -> Option<A11yState> {
        lock(&self.nodes).get(&id).cloned()
    }

    fn unregister_node(&self, id: ObjectId) {
        lock(&self.names).remove(&id);
        lock(&self.nodes).remove(&id);
        lock(&self.handles).remove(&id);
    }

    fn node_count(&self) -> usize {
        lock(&self.nodes).len()
    }

    fn notify_name_changed(&self, id: ObjectId) {
        self.post(id, NotificationName::NameChanged);
    }

    fn notify_value_changed(&self, id: ObjectId) {
        self.post(id, NotificationName::ValueChanged);
    }

    fn notify_state_changed(&self, id: ObjectId) {
        // A state change is announced through the value notification, matching the cocoa
        // bridge: posting the focus notification for a state transition misreports it.
        self.post(id, NotificationName::ValueChanged);
    }

    fn notify_focus_changed(&self, id: ObjectId) {
        self.post(id, NotificationName::FocusedElementChanged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::accessibility::A11yRole;

    #[test]
    fn submitted_state_is_stored_in_full_not_only_the_label() {
        let bridge = MacOSObjc2AccessibilityBridge::new();
        let state = A11yState {
            role: A11yRole::CheckBox,
            label: "Dark mode".to_string(),
            description: "Toggle the theme".to_string(),
            enabled: true,
            checked: Some(true),
            children: vec![8, 9],
            ..A11yState::default()
        };
        bridge.submit_node_state(4, &state);
        assert_eq!(bridge.node_state(4), Some(state));
        assert_eq!(bridge.accessibility_name(4).as_deref(), Some("Dark mode"));
    }

    #[test]
    fn unmount_removes_the_node_and_the_handle() {
        let bridge = MacOSObjc2AccessibilityBridge::new();
        bridge.register_handle(5, 0x1000);
        bridge.submit_node_state(
            5,
            &A11yState { role: A11yRole::Button, label: "Go".to_string(), ..A11yState::default() },
        );
        assert_eq!(bridge.node_count(), 1);
        bridge.unregister_node(5);
        assert_eq!(bridge.node_count(), 0);
        assert!(bridge.accessibility_name(5).is_none());
        assert!(bridge.node_state(5).is_none());
    }

    #[test]
    fn a_zero_handle_is_not_registered() {
        let bridge = MacOSObjc2AccessibilityBridge::new();
        bridge.register_handle(6, 0);
        assert!(lock(&bridge.handles).get(&6).is_none());
    }
}
