//! Win32 event notification helpers.
//!
//! This module provides Win32 event-handling helpers for the Windows platform
//! backend. All functions are `#[cfg(target_os = "windows")]`-gated to prevent
//! unused warnings on non-Windows build targets.

use crate::core::ObjectId;
use crate::platform::windows::types::{WindowsHandleKind, WindowsPlatform};
use crate::platform::WidgetTriggerEvent;
use crate::platform::WidgetTriggerKind;
use std::sync::OnceLock;

/// Registers the `RustWidgetsWindowClass` window class via `RegisterClassW`.
/// Safe to call multiple times — registration happens exactly once.
#[cfg(target_os = "windows")]
pub(crate) fn ensure_window_class_registered() {
    unsafe extern "system" {
        fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
    }
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| unsafe {
        use std::ptr::null_mut;
        use winapi::um::winuser::{
            LoadCursorW, RegisterClassW, CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WNDCLASSW,
        };
        let class_name = WindowsPlatform::to_wide("RustWidgetsWindowClass");
        let hinstance = GetModuleHandleW(std::ptr::null());
        let mut wnd_class: WNDCLASSW = std::mem::zeroed();
        wnd_class.style = CS_HREDRAW | CS_VREDRAW;
        wnd_class.lpfnWndProc = Some(super::types::rw_wnd_proc);
        wnd_class.hInstance = hinstance as _;
        wnd_class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wnd_class.lpszClassName = class_name.as_ptr();
        // RegisterClassW returns an ATOM (nonzero on success, 0 on failure).
        // If registration fails, the OnceLock prevents retry. The subsequent
        // CreateWindowExW call in create_window will return null, which is
        // already handled by the caller.
        if RegisterClassW(&wnd_class) == 0 {
            log::error!("[windows] RegisterClassW failed — window class not registered");
        }
    });
}

/// Global pointer to the active `WindowsPlatform` instance, stored at `init()` time.
#[cfg(target_os = "windows")]
static ACTIVE_WINDOWS_PLATFORM: OnceLock<usize> = OnceLock::new();

/// Returns a reference to the active `WindowsPlatform`, or `None` if not yet initialized.
#[cfg(target_os = "windows")]
pub(crate) fn active_windows_platform() -> Option<&'static WindowsPlatform> {
    let ptr = *ACTIVE_WINDOWS_PLATFORM.get()? as *const WindowsPlatform;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

/// Register the active platform pointer so that the window procedure can find it.
/// Called once during `WindowsPlatform::init()`.
#[cfg(target_os = "windows")]
pub(crate) fn register_active_platform(platform: &'static WindowsPlatform) {
    if let Err(prev) = ACTIVE_WINDOWS_PLATFORM.set(platform as *const WindowsPlatform as usize) {
        log::error!("[windows] register_active_platform already set — ignoring duplicate");
        let _ = prev;
    }
}

/// Map a Win32 notification code to a `WidgetTriggerKind` for the given widget kind.
#[cfg(target_os = "windows")]
pub(crate) fn control_notify_kind_for_widget(
    kind: WindowsHandleKind,
    notify_code: u32,
) -> Option<WidgetTriggerKind> {
    const BN_CLICKED: u32 = 0;
    const CBN_SELCHANGE: u32 = 1;
    const CBN_EDITCHANGE: u32 = 5;
    const EN_CHANGE: u32 = 0x0300;
    const LBN_SELCHANGE: u32 = 1;
    match kind {
        WindowsHandleKind::Button
        | WindowsHandleKind::CheckBox
        | WindowsHandleKind::RadioButton => {
            if notify_code == BN_CLICKED {
                Some(WidgetTriggerKind::Clicked)
            } else {
                None
            }
        }
        WindowsHandleKind::LineEdit => {
            if notify_code == EN_CHANGE {
                Some(WidgetTriggerKind::ValueChanged)
            } else {
                None
            }
        }
        WindowsHandleKind::ComboBox => {
            if notify_code == CBN_SELCHANGE {
                Some(WidgetTriggerKind::SelectionChanged)
            } else if notify_code == CBN_EDITCHANGE {
                Some(WidgetTriggerKind::ValueChanged)
            } else {
                None
            }
        }
        WindowsHandleKind::ListBox => {
            if notify_code == LBN_SELCHANGE {
                Some(WidgetTriggerKind::SelectionChanged)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Enqueue a typed trigger event derived from a Win32 notification code.
/// Returns `true` if the event was successfully enqueued.
#[cfg(target_os = "windows")]
pub(crate) fn enqueue_control_notify_event(
    platform: &WindowsPlatform,
    widget_id: ObjectId,
    notify_code: u32,
) -> bool {
    let widget_kind = match platform.state.kind_of(widget_id) {
        Some(kind) => kind,
        None => return false,
    };
    let kind = match control_notify_kind_for_widget(widget_kind, notify_code) {
        Some(kind) => kind,
        None => return false,
    };
    if let Ok(mut events) = platform.menu_state.pending_widget_events.lock() {
        events.push_back(WidgetTriggerEvent { widget_id, kind });
        // Dual-emit: a ComboBox selection change also counts as a value change
        // for cross-platform event parity.
        if widget_kind == WindowsHandleKind::ComboBox && kind == WidgetTriggerKind::SelectionChanged
        {
            events
                .push_back(WidgetTriggerEvent { widget_id, kind: WidgetTriggerKind::ValueChanged });
        }
        return true;
    }
    false
}

/// Resolve a `WidgetTriggerKind` from a widget kind and raw notification code.
/// Used by the `WM_NOTIFY` handler in the window procedure.
#[cfg(target_os = "windows")]
pub(crate) fn notify_kind_for_widget(
    kind: WindowsHandleKind,
    notify_code: u32,
) -> Option<WidgetTriggerKind> {
    if kind == WindowsHandleKind::Slider {
        return Some(WidgetTriggerKind::ValueChanged);
    }
    if notify_code == 0 {
        return None;
    }
    None
}
