// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows platform types, structs, enums, and traits.

use super::notify;
use crate::platform::state::BackendState;
use crate::platform::{Platform, WidgetTriggerEvent, WidgetTriggerKind};

pub use crate::platform::windows_notify::WindowsHandleKind;

/// The window procedure Win32 calls for the class this backend registers.
///
/// Named `wnd_proc`, not `rw_wnd_proc`: the `rw_` prefix belongs to the C ABI
/// boundary (`src/bindings/`), where a flat global namespace makes it necessary.
/// This function is an internal Win32 callback reached only through a class
/// registration and a `wnd_class.lpfnWndProc` assignment, so a prefix borrowed
/// from a different layer would suggest an ABI export that does not exist.
/// `tools/check_rw_prefix_is_abi_only.sh` enforces the boundary.
#[cfg(target_os = "windows")]
pub(crate) unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use winapi::um::winuser::NMHDR;
    use winapi::um::winuser::{
        DefWindowProcW, GetClientRect, GetDlgCtrlID, PostQuitMessage, WM_COMMAND, WM_DESTROY,
        WM_GETMINMAXINFO, WM_NOTIFY, WM_SIZE,
    };
    match msg {
        // The user resized the window (or the window manager did). Report the new client
        // size so the host can re-run its layout: without this a window resized by the
        // user kept every child at the geometry it had for the previous size, because
        // nothing else tells the library the window changed.
        WM_SIZE => {
            if let Some(platform) = notify::active_windows_platform() {
                if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd) {
                    let mut rect =
                        winapi::shared::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    // SAFETY: `hwnd` is the window this procedure was called for, and
                    // Win32 fills the RECT we hand it. A failure leaves the zeros, which
                    // are rejected below rather than reported as a size.
                    if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
                        let width = (rect.right - rect.left).max(0) as u32;
                        let height = (rect.bottom - rect.top).max(0) as u32;
                        if width > 0 && height > 0 {
                            crate::queue_resize_trigger(widget_id, width, height);
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
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
        // Win32 has no `SetWindowMinSize` API: the minimum is enforced by writing
        // `ptMinTrackSize` into the MINMAXINFO the system passes before a resize or
        // maximise. Returning 0 here after filling it in tells the system the
        // constraint was applied.
        WM_GETMINMAXINFO => {
            if let Some(platform) = notify::active_windows_platform() {
                if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd) {
                    if let Some((min_w, min_h)) = platform.state.window_min_size(widget_id) {
                        if !lparam_is_null(lparam) {
                            let info = lparam as *mut winapi::um::winuser::MINMAXINFO;
                            // SAFETY: Win32 passes a valid MINMAXINFO pointer in
                            // lParam for WM_GETMINMAXINFO for the duration of the
                            // call; we only write into it.
                            unsafe {
                                (*info).ptMinTrackSize.x = min_w as i32;
                                (*info).ptMinTrackSize.y = min_h as i32;
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

/// Whether the message's `lParam` is a null pointer.
///
/// Kept as a tiny helper so the `WM_GETMINMAXINFO` arm reads clearly and the
/// cast-and-compare happens in exactly one place.
#[cfg(target_os = "windows")]
fn lparam_is_null(lparam: isize) -> bool {
    lparam == 0
}
#[cfg(target_os = "windows")]
impl WindowsPlatform {
    /// Encodes `s` as a NUL-terminated UTF-16 buffer.
    ///
    /// This is the form every Win32 `*W` entry point expects (`CreateWindowExW`,
    /// `SetWindowTextW`, `MessageBoxW`, …). `String`/`&str` cannot be passed
    /// directly because Win32 wide strings are neither length-prefixed nor
    /// guarantee-terminated by Rust.
    ///
    /// `Vec` comes from `crate::compat` so this compiles under the `mini`
    /// profile, which is `no_std` and has no `std` prelude to resolve `Vec` from.
    pub fn to_wide(s: &str) -> crate::compat::Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }
    /// Returns the native window handle recorded for `id`, if any.
    ///
    /// Only ids that were bound via [`Self::bind_native_handle`] are present; a
    /// widget id that the library created without a host window has no handle and
    /// yields `None`. A poisoned handle map is logged and treated as `None` rather
    /// than panicking.
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
    /// Records `hwnd` as the native window for `id`.
    ///
    /// Also registers the handle with the accessibility bridge, so UIAutomation
    /// notifications can be raised against the real window. Binding an id twice
    /// replaces the previous handle. A poisoned map is ignored: the handle is
    /// dropped rather than panicking inside a message-pump callback.
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
    /// Returns the widget id whose native handle is `hwnd`.
    ///
    /// The reverse of [`Self::get_native_handle`], used to map a `WM_COMMAND`
    /// notification back to the library widget that owns it. Linear in the number
    /// of bound handles. `None` when no widget owns that handle, or when the handle
    /// map is poisoned (logged, not panicked).
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
    /// Downcasts to `T`, returning `None` when the backend is a different type.
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
    /// Host-side widget/surface state shared with every backend.
    pub state: BackendState<WindowsHandleKind>,
    /// Whether [`Platform::init`] has run.
    pub runtime_initialized: AtomicBool,
    /// Whether the run loop is currently active.
    pub runtime_running: AtomicBool,
    /// Menu and command-routing state (Win32 only).
    #[cfg(target_os = "windows")]
    pub menu_state: Win32MenuState,
    // Removed handle_state: Win32HandleState, as Win32HandleState is not defined in state.rs
    /// Platform IME bridge for text input method integration (Windows TSF).
    /// Uses `ime_windows::WindowsImeBridge` (real state machine, no fake COM vtables).
    pub ime_bridge: crate::platform::ime_windows::WindowsImeBridge,
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
/// command-to-widget mappings. Only compiled on Windows targets.
#[cfg(target_os = "windows")]
pub struct Win32MenuState {
    // SAFETY: HWND is only used on the main thread, and Win32MenuState is not shared across threads in this context.
    pub(crate) handles: Mutex<HashMap<u64, usize>>,
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
    /// Creates a backend with no windows, no menus and no bound handles.
    pub fn new() -> Self {
        WindowsPlatform {
            state: BackendState::new(),
            runtime_initialized: AtomicBool::new(false),
            runtime_running: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            menu_state: Win32MenuState::new(),
            ime_bridge: crate::platform::ime_windows::WindowsImeBridge::new(),
            clipboard: crate::platform::clipboard_stubs::windows::WindowsClipboard,
            #[cfg(target_os = "windows")]
            a11y_bridge: crate::platform::accessibility::windows::WindowsAccessibilityBridge::new(),
            #[cfg(not(target_os = "windows"))]
            a11y_bridge: (),
        }
    }
}

crate::impl_default_via_new!(WindowsPlatform);
