//! Windows UIAutomation bridge via NotifyWinEvent.
//!
//! Uses the classic win32 `NotifyWinEvent` API to raise accessibility
//! events for screen readers. This is the simplest and most compatible
//! approach, supported by Narrator, JAWS, NVDA, and other ATs.
//!
//! For focus changes: `NotifyWinEvent(EVENT_OBJECT_FOCUS, hwnd, OBJID_CLIENT, 0)`
//!
//! Reference: https://learn.microsoft.com/en-us/windows/win32/winauto/event-constants

use super::AccessibilityBridge;
use crate::core::ObjectId;
use std::collections::HashMap;
use std::sync::Mutex;

/// Windows UIAutomation bridge using NotifyWinEvent.
pub struct WindowsAccessibilityBridge {
    names: Mutex<HashMap<ObjectId, String>>,
    /// Mapping from widget ObjectId to native HWND pointer (as usize).
    native_handles: Mutex<HashMap<ObjectId, usize>>,
}

impl WindowsAccessibilityBridge {
    pub fn new() -> Self {
        Self { names: Mutex::new(HashMap::new()), native_handles: Mutex::new(HashMap::new()) }
    }

    /// Register a native HWND handle for the given widget id.
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

    /// Post a Win32 accessibility event via NotifyWinEvent.
    fn post_event(&self, id: ObjectId, event: u32) -> bool {
        let ptr = match self.native_handles.lock() {
            Ok(h) => h.get(&id).copied(),
            Err(_) => return false,
        };
        let Some(_ptr) = ptr else { return false };
        #[cfg(target_os = "windows")]
        {
            let hwnd = ptr as winapi::um::winnt::HWND;
            unsafe {
                winapi::um::winuser::NotifyWinEvent(
                    event,
                    hwnd,
                    winapi::um::winuser::OBJID_CLIENT,
                    0,
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = event;
        }
        true
    }
}

impl Default for WindowsAccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessibilityBridge for WindowsAccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        if let Ok(mut names) = self.names.lock() {
            names.insert(id, name.to_string());
        }
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        self.names.lock().ok().and_then(|names| names.get(&id).cloned())
    }

    fn notify_name_changed(&self, id: ObjectId) {
        self.post_event(id, winapi::um::winuser::EVENT_OBJECT_NAMECHANGE);
    }

    fn notify_value_changed(&self, id: ObjectId) {
        self.post_event(id, winapi::um::winuser::EVENT_OBJECT_VALUECHANGE);
    }

    fn notify_state_changed(&self, id: ObjectId) {
        self.post_event(id, winapi::um::winuser::EVENT_OBJECT_STATECHANGE);
    }

    fn notify_focus_changed(&self, id: ObjectId) {
        self.post_event(id, winapi::um::winuser::EVENT_OBJECT_FOCUS);
    }
}
