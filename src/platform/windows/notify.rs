//! Win32 event notification helpers.

fn ensure_window_class_registered() {
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
        wnd_class.lpfnWndProc = Some(rust_widgets_wnd_proc);
        wnd_class.hInstance = hinstance as _;
        wnd_class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wnd_class.lpszClassName = class_name.as_ptr();
        let _ = RegisterClassW(&wnd_class);
    });
}
#[cfg(target_os = "windows")]
static ACTIVE_WINDOWS_PLATFORM: OnceLock<usize> = OnceLock::new();
#[cfg(target_os = "windows")]

fn active_windows_platform() -> Option<&'static WindowsPlatform> {
    let ptr = *ACTIVE_WINDOWS_PLATFORM.get()? as *const WindowsPlatform;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}
#[cfg(target_os = "windows")]

fn control_notify_kind_for_widget(
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
#[cfg(target_os = "windows")]

fn enqueue_control_notify_event(
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
        if widget_kind == WindowsHandleKind::ComboBox && kind == WidgetTriggerKind::SelectionChanged
        {
            events.push_back(WidgetTriggerEvent {
                widget_id,
                kind: WidgetTriggerKind::ValueChanged,
            });
        }
        return true;
    }
    false
}
#[cfg(target_os = "windows")]

fn notify_kind_for_widget(kind: WindowsHandleKind, notify_code: u32) -> Option<WidgetTriggerKind> {
    if kind == WindowsHandleKind::Slider {
        return Some(WidgetTriggerKind::ValueChanged);
    }
    if notify_code == 0 {
        return None;
    }
    None
}
// Helper to convert &str to wide string for Win32 API

