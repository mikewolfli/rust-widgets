//! Windows platform types, structs, enums, and traits.

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
    ToolBar,
    StatusBar,
    ProgressBar,
    Slider,
    ComboBox,
    // Add other widget kinds as needed
}
#[cfg(target_os = "windows")]
unsafe extern "system" fn rust_widgets_wnd_proc(
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
            if let Some(platform) = active_windows_platform() {
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
                        if enqueue_control_notify_event(platform, widget_id, notify_code) {
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
                                if enqueue_control_notify_event(platform, widget_id, notify_code) {
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
            if let Some(platform) = active_windows_platform() {
                let hdr = lparam as *const NMHDR;
                if !hdr.is_null() {
                    let hwnd_from = unsafe { (*hdr).hwndFrom };
                    let notify_code = unsafe { (*hdr).code };
                    if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd_from) {
                        if let Some(kind) =
                            platform.state.kind_of(widget_id).and_then(|widget_kind| {
                                notify_kind_for_widget(widget_kind, notify_code)
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
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    pub fn get_native_handle(&self, id: u64) -> Option<HWND> {
        #[cfg(target_os = "windows")]
        {
            let handles = self.menu_state.handles.lock().ok()?;
            handles.get(&id).map(|&h| h as HWND)
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
                return;
            }
        }
    }
    #[cfg(target_os = "windows")]
    pub unsafe fn bind_control_command(&self, widget_id: u64, hwnd: HWND) {
        use winapi::um::winuser::{SetWindowLongPtrW, GWLP_ID};
        let command_id = self
            .menu_state
            .next_command_id
            .fetch_add(1, Ordering::SeqCst) as u32;
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_ID, command_id as isize);
        }
        if let Ok(mut map) = self.menu_state.control_command_to_widget.lock() {
            map.insert(command_id, widget_id);
        }
    }
    #[cfg(target_os = "windows")]
    pub fn widget_id_by_native_handle(&self, hwnd: HWND) -> Option<u64> {
        let handles = self.menu_state.handles.lock().ok()?;
        handles
            .iter()
            .find_map(|(widget_id, native)| ((*native as HWND) == hwnd).then_some(*widget_id))
    }
}
// Extension trait for downcasting Platform trait object to WindowsPlatform

pub trait PlatformDowncast {
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;
}
impl PlatformDowncast for dyn super::Platform {
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        // This is a stub; actual implementation may use Any or other trait object logic
        None
    }
}
// Removed unresolved import crate::state::BackendState
// Win32 API types and functions
#[cfg(target_os = "windows")]

use winapi::shared::windef::HMENU;
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
#[cfg(target_os = "windows")]
use std::sync::OnceLock;
// ...existing code...
// ...existing code...
/// Windows platform backend struct definition
pub struct WindowsPlatform {
    pub state: BackendState<WindowsHandleKind>,
    pub runtime_initialized: AtomicBool,
    pub runtime_running: AtomicBool,
    #[cfg(target_os = "windows")]
    pub menu_state: Win32MenuState,
    // Removed handle_state: Win32HandleState, as Win32HandleState is not defined in state.rs
}
#[allow(dead_code)]
pub struct Win32MenuState {
    // SAFETY: HWND is only used on the main thread, and Win32MenuState is not shared across threads in this context.
    handles: Mutex<HashMap<u64, usize>>,
    menu_owner_window: Mutex<HashMap<u64, u64>>,
    menu_command_to_item: Mutex<HashMap<u32, u64>>,
    control_command_to_widget: Mutex<HashMap<u32, u64>>,
    pending_menu_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    pending_widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    next_command_id: AtomicU64,
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
// Stub trait for native Win32 ProgressBar integration

impl WindowsPlatform {
    pub fn new() -> Self {
        WindowsPlatform {
            state: BackendState::new(),
            runtime_initialized: AtomicBool::new(false),
            runtime_running: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            menu_state: Win32MenuState::new(),
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
        platform: &dyn super::Platform,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<u64>;
}
impl WindowsPlatformExtSlider for WindowsPlatform {
    fn try_create_slider(
        platform: &dyn super::Platform,
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
            // Use a direct cast for demonstration; actual implementation should use Any for trait objects
            let this =
                unsafe { &*(platform as *const dyn super::Platform as *const WindowsPlatform) };
            if !this.state.contains_widget(parent) {
                return None;
            }
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            use std::ptr::null_mut;
            use winapi::um::commctrl::TRACKBAR_CLASS;
            use winapi::um::winuser::CreateWindowExW;
            let parent_hwnd = this.get_native_handle(parent)?;
            let class: Vec<u16> = OsStr::new(TRACKBAR_CLASS)
                .encode_wide()
                .chain(Some(0))
                .collect();
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
                this.state
                    .create_widget(WindowsHandleKind::Slider, "Slider", x, y, width, height);
            this.bind_native_handle(widget_id, hwnd);
            return Some(widget_id);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (platform, parent, x, y, width, height);
            None
        }
    }
}
#[cfg(all(test, target_os = "windows"))]

