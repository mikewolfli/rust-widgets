//! Windows platform types, structs, enums, and traits.

use super::notify;
use crate::platform::state::BackendState;
use crate::platform::{Platform, WidgetTriggerEvent, WidgetTriggerKind};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WindowsHandleKind {
    Window,
    Button,
    Label,
    CheckBox,
    RadioButton,
    LineEdit,
    ListBox,
    Panel,
    MenuBar,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
    ProgressBar,
    Slider,
    ComboBox,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
    SpinBox,
    ListView,
    ScrollArea,
}
#[cfg(target_os = "windows")]
pub(crate) unsafe extern "system" fn rust_widgets_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use winapi::um::winuser::NMHDR;
    use winapi::um::winuser::{
        DefWindowProcW, GetDlgCtrlID, PostQuitMessage, WM_COMMAND, WM_DESTROY, WM_NOTIFY,
    };
    match msg {
        WM_COMMAND => {
            let command_id = (wparam & 0xFFFF) as u32;
            let notify_code = ((wparam >> 16) & 0xFFFF) as u32;
            if let Some(platform) = notify::active_windows_platform() {
                if let Ok(map) = platform.menu_state.menu_command_to_item.lock() {
                    if let Some(item_id) = map.get(&command_id).copied() {
                        if let Ok(mut queue) = platform.menu_state.pending_menu_events.lock() {
                            queue.push_back(WidgetTriggerEvent {
                                widget_id: item_id,
                                kind: WidgetTriggerKind::Clicked,
                            });
                        }
                        return 0;
                    }
                }
                if let Ok(map) = platform.menu_state.control_command_to_widget.lock() {
                    if let Some(widget_id) = map.get(&command_id).copied() {
                        if notify::enqueue_control_notify_event(platform, widget_id, notify_code) {
                            return 0;
                        }
                    }
                }
                if lparam != 0 {
                    let hwnd_from = lparam as HWND;
                    let fallback_command_id = unsafe { GetDlgCtrlID(hwnd_from) } as u32;
                    if fallback_command_id != 0 {
                        if let Ok(map) = platform.menu_state.control_command_to_widget.lock() {
                            if let Some(widget_id) = map.get(&fallback_command_id).copied() {
                                if notify::enqueue_control_notify_event(
                                    platform,
                                    widget_id,
                                    notify_code,
                                ) {
                                    return 0;
                                }
                            }
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NOTIFY => {
            if let Some(platform) = notify::active_windows_platform() {
                let hdr = lparam as *const NMHDR;
                if !hdr.is_null() {
                    let hwnd_from = unsafe { (*hdr).hwndFrom };
                    let notify_code = unsafe { (*hdr).code };
                    if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd_from) {
                        if let Some(kind) =
                            platform.state.kind_of(widget_id).and_then(|widget_kind| {
                                notify::notify_kind_for_widget(widget_kind, notify_code)
                            })
                        {
                            if let Ok(mut events) = platform.menu_state.pending_widget_events.lock()
                            {
                                events.push_back(WidgetTriggerEvent { widget_id, kind });
                            }
                            return 0;
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
#[cfg(target_os = "windows")]
impl WindowsPlatform {
    pub fn to_wide(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }
    pub fn get_native_handle(&self, id: u64) -> Option<HWND> {
        #[cfg(target_os = "windows")]
        {
            match self.menu_state.handles.lock() {
                Ok(handles) => handles.get(&id).map(|&h| h as HWND),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] get_native_handle: handles mutex poisoned"
                    );
                    None
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    pub fn bind_native_handle(&self, id: u64, hwnd: HWND) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.insert(id, hwnd as usize);
            } else {
                // Handle lock error explicitly
            }
            self.a11y_bridge.register_handle(id, hwnd as usize);
        }
    }
    #[cfg(target_os = "windows")]
    /// # Safety
    ///
    /// Caller must ensure that `hwnd` is a valid native window handle
    /// and that it remains valid for the duration of this call.
    /// Modifying the window's identifier via `SetWindowLongPtrW` can
    /// affect window procedure behavior; callers should ensure this
    /// is done only for windows owned by this platform adapter.
    pub unsafe fn bind_control_command(&self, widget_id: u64, hwnd: HWND) {
        use winapi::um::winuser::{SetWindowLongPtrW, GWLP_ID};
        let command_id = self.menu_state.next_command_id.fetch_add(1, Ordering::SeqCst) as u32;
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_ID, command_id as isize);
        }
        if let Ok(mut map) = self.menu_state.control_command_to_widget.lock() {
            map.insert(command_id, widget_id);
        }
    }
    #[cfg(target_os = "windows")]
    pub fn widget_id_by_native_handle(&self, hwnd: HWND) -> Option<u64> {
        match self.menu_state.handles.lock() {
            Ok(handles) => handles
                .iter()
                .find_map(|(widget_id, native)| ((*native as HWND) == hwnd).then_some(*widget_id)),
            Err(_) => {
                log::error!(
                    "[rust_widgets][windows] widget_id_by_native_handle: handles mutex poisoned"
                );
                None
            }
        }
    }
}
/// Extension trait for downcasting `dyn Platform` to concrete platform types.
pub trait PlatformDowncast {
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;
}
impl PlatformDowncast for dyn Platform {
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}
#[cfg(target_os = "windows")]
use winapi::shared::windef::HWND;
#[cfg(not(target_os = "windows"))]
type HWND = *mut std::ffi::c_void;
// Windows backend shell.
#[cfg(target_os = "windows")]
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(target_os = "windows")]
use std::sync::Mutex;
/// Windows platform backend struct definition
pub struct WindowsPlatform {
    pub state: BackendState<WindowsHandleKind>,
    pub runtime_initialized: AtomicBool,
    pub runtime_running: AtomicBool,
    #[cfg(target_os = "windows")]
    pub menu_state: Win32MenuState,
    // Removed handle_state: Win32HandleState, as Win32HandleState is not defined in state.rs
    /// Platform IME bridge for text input method integration (Windows TSF).
    pub ime_bridge: crate::platform::ime_stubs::windows::WindowsImeBridge,
    /// Platform rich clipboard backend.
    pub clipboard: crate::platform::clipboard_stubs::windows::WindowsClipboard,
    /// Platform accessibility bridge for UIAutomation notifications.
    #[cfg(target_os = "windows")]
    pub a11y_bridge: crate::platform::accessibility::windows::WindowsAccessibilityBridge,
    #[cfg(not(target_os = "windows"))]
    pub a11y_bridge: (),
}
/// Win32 menu state holder.
/// Reserved for Windows platform menu integration — stores HWND handles and
/// command-to-widget mappings. Only compiled on `cfg(windows)` targets.
/// Marked `#[allow(dead_code)]` on non-Windows builds where it is never constructed.
#[allow(dead_code)]
pub struct Win32MenuState {
    // SAFETY: HWND is only used on the main thread, and Win32MenuState is not shared across threads in this context.
    pub(crate) handles: Mutex<HashMap<u64, usize>>,
    pub(crate) menu_owner_window: Mutex<HashMap<u64, u64>>,
    pub(crate) menu_command_to_item: Mutex<HashMap<u32, u64>>,
    pub(crate) control_command_to_widget: Mutex<HashMap<u32, u64>>,
    pub(crate) pending_menu_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    pub(crate) pending_widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    pub(crate) next_command_id: AtomicU64,
}
#[cfg(target_os = "windows")]
impl Win32MenuState {
    fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            menu_owner_window: Mutex::new(HashMap::new()),
            menu_command_to_item: Mutex::new(HashMap::new()),
            control_command_to_widget: Mutex::new(HashMap::new()),
            pending_menu_events: Mutex::new(VecDeque::new()),
            pending_widget_events: Mutex::new(VecDeque::new()),
            next_command_id: AtomicU64::new(1000),
        }
    }
}
#[cfg(target_os = "windows")]
// Extension trait for native Win32 Slider (Trackbar) integration
impl WindowsPlatform {
    pub fn new() -> Self {
        WindowsPlatform {
            state: BackendState::new(),
            runtime_initialized: AtomicBool::new(false),
            runtime_running: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            menu_state: Win32MenuState::new(),
            ime_bridge: crate::platform::ime_stubs::windows::WindowsImeBridge,
            clipboard: crate::platform::clipboard_stubs::windows::WindowsClipboard,
            #[cfg(target_os = "windows")]
            a11y_bridge: crate::platform::accessibility::windows::WindowsAccessibilityBridge::new(),
            #[cfg(not(target_os = "windows"))]
            a11y_bridge: (),
        }
    }
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

pub trait WindowsPlatformExtSlider {
    fn try_create_slider(
        platform: &dyn Platform,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<u64>;
}
impl WindowsPlatformExtSlider for WindowsPlatform {
    fn try_create_slider(
        platform: &dyn Platform,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<u64> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::InitCommonControls;
            use winapi::um::winuser::{WS_BORDER, WS_CHILD, WS_VISIBLE};
            unsafe {
                InitCommonControls();
            }
            let this = platform.as_any().downcast_ref::<WindowsPlatform>()?;
            if !this.state.contains_widget(parent) {
                return None;
            }
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            use std::ptr::null_mut;
            use winapi::um::commctrl::TRACKBAR_CLASS;
            use winapi::um::winuser::CreateWindowExW;
            let parent_hwnd = this.get_native_handle(parent)?;
            let class: Vec<u16> = OsStr::new(TRACKBAR_CLASS).encode_wide().chain(Some(0)).collect();
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    null_mut(),
                    WS_CHILD | WS_VISIBLE | WS_BORDER,
                    x,
                    y,
                    width as i32,
                    height as i32,
                    parent_hwnd,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if hwnd.is_null() {
                return None;
            }
            let widget_id =
                this.state.create_widget(WindowsHandleKind::Slider, "Slider", x, y, width, height);
            this.bind_native_handle(widget_id, hwnd);
            Some(widget_id)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (platform, parent, x, y, width, height);
            None
        }
    }
}
