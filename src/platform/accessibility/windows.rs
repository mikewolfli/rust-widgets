//! Windows UIAutomation bridge via NotifyWinEvent.
//!
//! Uses the classic win32 `NotifyWinEvent` API to raise accessibility
//! events for screen readers. This is the simplest and most compatible
//! approach, supported by Narrator, JAWS, NVDA, and other ATs.
//!
//! For focus changes: `NotifyWinEvent(EVENT_OBJECT_FOCUS, hwnd, OBJID_CLIENT, 0)`
//!
//! Reference: <https://learn.microsoft.com/en-us/windows/win32/winauto/event-constants>
//!
//! ## UIA integration helpers (cfg-gated)
//!
//! The `uia_control_type()` function maps [`super::A11yRole`] to UIA control
//! type IDs for modern UI Automation (UIA) provider conformance.

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
        let Some(hwnd_val) = ptr else { return false };
        #[cfg(target_os = "windows")]
        {
            let hwnd = hwnd_val as winapi::shared::windef::HWND;
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

// ─── UIA integration helpers ────────────────────────────────────────────

/// UIA control type IDs for UI Automation provider conformance.
///
/// Reference: <https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controltypesoverview>
#[cfg(target_os = "windows")]
pub mod uia_constants {
    /// UIA_ButtonControlTypeId
    pub const UIA_BUTTON_CONTROL_TYPE: u32 = 50000u32;
    /// UIA_CheckBoxControlTypeId
    pub const UIA_CHECKBOX_CONTROL_TYPE: u32 = 50001u32;
    /// UIA_ComboBoxControlTypeId
    pub const UIA_COMBOBOX_CONTROL_TYPE: u32 = 50003u32;
    /// UIA_EditControlTypeId
    pub const UIA_EDIT_CONTROL_TYPE: u32 = 50004u32;
    /// UIA_GroupControlTypeId
    pub const UIA_GROUP_CONTROL_TYPE: u32 = 50008u32;
    /// UIA_ImageControlTypeId
    pub const UIA_IMAGE_CONTROL_TYPE: u32 = 50009u32;
    /// UIA_ListItemControlTypeId
    pub const UIA_LISTITEM_CONTROL_TYPE: u32 = 50010u32;
    /// UIA_ListControlTypeId
    pub const UIA_LIST_CONTROL_TYPE: u32 = 50011u32;
    /// UIA_MenuControlTypeId
    pub const UIA_MENU_CONTROL_TYPE: u32 = 50012u32;
    /// UIA_MenuBarControlTypeId
    pub const UIA_MENUBAR_CONTROL_TYPE: u32 = 50013u32;
    /// UIA_MenuItemControlTypeId
    pub const UIA_MENUITEM_CONTROL_TYPE: u32 = 50014u32;
    /// UIA_ProgressBarControlTypeId
    pub const UIA_PROGRESSBAR_CONTROL_TYPE: u32 = 50016u32;
    /// UIA_RadioButtonControlTypeId
    pub const UIA_RADIOBUTTON_CONTROL_TYPE: u32 = 50017u32;
    /// UIA_ScrollBarControlTypeId
    pub const UIA_SCROLLBAR_CONTROL_TYPE: u32 = 50018u32;
    /// UIA_SliderControlTypeId
    pub const UIA_SLIDER_CONTROL_TYPE: u32 = 50019u32;
    /// UIA_SpinButtonControlTypeId
    pub const UIA_SPINBUTTON_CONTROL_TYPE: u32 = 50020u32;
    /// UIA_StatusBarControlTypeId
    pub const UIA_STATUSBAR_CONTROL_TYPE: u32 = 50021u32;
    /// UIA_TabControlTypeId
    pub const UIA_TAB_CONTROL_TYPE: u32 = 50022u32;
    /// UIA_TableControlTypeId
    pub const UIA_TABLE_CONTROL_TYPE: u32 = 50023u32;
    /// UIA_TextControlTypeId
    pub const UIA_TEXT_CONTROL_TYPE: u32 = 50024u32;
    /// UIA_ToolTipControlTypeId
    pub const UIA_TOOLTIP_CONTROL_TYPE: u32 = 50026u32;
    /// UIA_TreeControlTypeId
    pub const UIA_TREE_CONTROL_TYPE: u32 = 50028u32;
    /// UIA_WindowControlTypeId
    pub const UIA_WINDOW_CONTROL_TYPE: u32 = 50031u32;
    /// UIA_HyperlinkControlTypeId
    pub const UIA_HYPERLINK_CONTROL_TYPE: u32 = 50032u32;
    /// UIA_DataGridControlTypeId
    pub const UIA_DATAGRID_CONTROL_TYPE: u32 = 50035u32;
}

/// Map an [`super::A11yRole`] to the corresponding UIA control type ID.
#[cfg(target_os = "windows")]
pub fn uia_control_type_id(role: &super::A11yRole) -> u32 {
    use uia_constants::*;
    match role {
        super::A11yRole::Button | super::A11yRole::Switch => UIA_BUTTON_CONTROL_TYPE,
        super::A11yRole::Label | super::A11yRole::Heading | super::A11yRole::Paragraph => {
            UIA_TEXT_CONTROL_TYPE
        }
        super::A11yRole::TextField => UIA_EDIT_CONTROL_TYPE,
        super::A11yRole::CheckBox => UIA_CHECKBOX_CONTROL_TYPE,
        super::A11yRole::RadioButton => UIA_RADIOBUTTON_CONTROL_TYPE,
        super::A11yRole::Slider => UIA_SLIDER_CONTROL_TYPE,
        super::A11yRole::ProgressBar => UIA_PROGRESSBAR_CONTROL_TYPE,
        super::A11yRole::List => UIA_LIST_CONTROL_TYPE,
        super::A11yRole::Table => UIA_TABLE_CONTROL_TYPE,
        super::A11yRole::Image => UIA_IMAGE_CONTROL_TYPE,
        super::A11yRole::Link => UIA_HYPERLINK_CONTROL_TYPE,
        super::A11yRole::Group | super::A11yRole::StatusBar => UIA_GROUP_CONTROL_TYPE,
        super::A11yRole::Window => UIA_WINDOW_CONTROL_TYPE,
        super::A11yRole::Dialog | super::A11yRole::Alert => UIA_WINDOW_CONTROL_TYPE,
        super::A11yRole::Menu => UIA_MENU_CONTROL_TYPE,
        super::A11yRole::MenuItem => UIA_MENUITEM_CONTROL_TYPE,
        super::A11yRole::Tab => UIA_TAB_CONTROL_TYPE,
        super::A11yRole::ComboBox => UIA_COMBOBOX_CONTROL_TYPE,
        super::A11yRole::SpinButton => UIA_SPINBUTTON_CONTROL_TYPE,
        super::A11yRole::ToolTip => UIA_TOOLTIP_CONTROL_TYPE,
        super::A11yRole::Tree => UIA_TREE_CONTROL_TYPE,
        super::A11yRole::Unknown => UIA_GROUP_CONTROL_TYPE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<WindowsAccessibilityBridge>();
        assert_sync::<WindowsAccessibilityBridge>();
    }

    #[test]
    fn test_uia_control_type_mapping() {
        #[cfg(target_os = "windows")]
        {
            assert_eq!(
                uia_control_type_id(&super::A11yRole::Button),
                uia_constants::UIA_BUTTON_CONTROL_TYPE
            );
            assert_eq!(
                uia_control_type_id(&super::A11yRole::TextField),
                uia_constants::UIA_EDIT_CONTROL_TYPE
            );
            assert_eq!(
                uia_control_type_id(&super::A11yRole::Window),
                uia_constants::UIA_WINDOW_CONTROL_TYPE
            );
        }
    }
}
