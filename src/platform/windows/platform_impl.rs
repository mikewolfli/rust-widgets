// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `impl Platform for WindowsPlatform` — the main trait implementation.

use crate::core::{ObjectId, PlatformFamily};
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::ime::ImeBridge;
use crate::platform::{Platform, PlatformCapabilities, WindowStateFlag};

use crate::platform::windows::notify;
use crate::platform::windows::types::*;
use crate::platform::DropEvent;
use std::sync::atomic::Ordering;

/// SAFETY: All Win32 FFI calls in this module follow standard Windows API safety patterns:
/// - `CreateWindowExW` return values are checked for null (via `hwnd.is_null()`) before use.
/// - `GetLastError` is implicitly checked via the null-return convention; a null HWND indicates
///   that the caller should inspect `GetLastError` for the specific failure code.
/// - Pointers passed to Win32 functions must remain valid for the duration of the call; wide
///   strings (`to_wide`) are kept alive via local variables that live across the `unsafe` block.
/// - `ShowWindow` / `UpdateWindow` / `MoveWindow` operate on previously-validated HWNDs.
impl Platform for WindowsPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn set_window_state(&self, widget_id: ObjectId, flag: WindowStateFlag, on: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        self.state.set_window_state(widget_id, flag, on);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                GetWindowLongW, SetWindowLongW, ShowWindow, GWL_STYLE, SW_MAXIMIZE, SW_MINIMIZE,
                SW_RESTORE, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
            };
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                match flag {
                    WindowStateFlag::Maximized => unsafe {
                        let cmd = if on { SW_MAXIMIZE } else { SW_RESTORE };
                        ShowWindow(hwnd, cmd);
                    },
                    WindowStateFlag::Minimized => unsafe {
                        let cmd = if on { SW_MINIMIZE } else { SW_RESTORE };
                        ShowWindow(hwnd, cmd);
                    },
                    WindowStateFlag::Fullscreen => {
                        // Win32 has no "full screen" window flag: it is achieved by
                        // dropping the frame styles and filling the monitor. Reapply
                        // the styles to leave it, which is what the WM would do.
                        unsafe {
                            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                            let frame =
                                WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
                            let new_style = if on { style & !frame } else { style | frame };
                            SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                            ShowWindow(hwnd, SW_MAXIMIZE);
                        }
                    }
                    WindowStateFlag::Resizable => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        let new_style = if on {
                            style | WS_THICKFRAME | WS_MAXIMIZEBOX
                        } else {
                            style & !(WS_THICKFRAME | WS_MAXIMIZEBOX)
                        };
                        SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                    },
                    WindowStateFlag::Decorated => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        let decorative = WS_CAPTION | WS_THICKFRAME;
                        let new_style = if on { style | decorative } else { style & !decorative };
                        SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                    },
                }
            }
        }
        true
    }

    fn is_window_in_state(&self, widget_id: ObjectId, flag: WindowStateFlag) -> Option<bool> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                GetWindowLongW, IsIconic, IsZoomed, GWL_STYLE, WS_CAPTION, WS_MAXIMIZEBOX,
                WS_MINIMIZEBOX, WS_THICKFRAME,
            };
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let value = match flag {
                    WindowStateFlag::Maximized => unsafe { IsZoomed(hwnd) != 0 },
                    WindowStateFlag::Minimized => unsafe { IsIconic(hwnd) != 0 },
                    // Full screen is "no frame styles left", the inverse of the
                    // Decorated computation below.
                    WindowStateFlag::Fullscreen => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        style & (WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX) == 0
                    },
                    WindowStateFlag::Resizable => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        style & (WS_THICKFRAME | WS_MAXIMIZEBOX) != 0
                    },
                    WindowStateFlag::Decorated => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        style & (WS_CAPTION | WS_THICKFRAME) != 0
                    },
                };
                return Some(value);
            }
        }
        self.state.window_state(widget_id, flag)
    }

    fn set_window_min_size(&self, widget_id: ObjectId, width: u32, height: u32) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        // Win32 has no setter: the value is consumed by the `WM_GETMINMAXINFO`
        // handler in `wnd_proc`, which reads it back from this state model. That
        // is why the write is recorded even for a state-only window.
        self.state.set_window_min_size(widget_id, width, height);
        // Force a recompute so a *shrink* of the constraint takes effect now
        // rather than at the next user resize.
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
        }
        true
    }

    fn window_min_size(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return None;
        }
        self.state.window_min_size(widget_id)
    }

    fn set_window_icon(&self, widget_id: ObjectId, path: &str) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE,
                LR_LOADFROMFILE, WM_SETICON,
            };
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let wide = Self::to_wide(path);
                // SAFETY: `wide` is a NUL-terminated UTF-16 buffer that outlives
                // the call; LoadImageW returns a fresh HICON or null.
                let icon = unsafe {
                    LoadImageW(
                        std::ptr::null_mut(),
                        wide.as_ptr(),
                        IMAGE_ICON,
                        0,
                        0,
                        LR_LOADFROMFILE | LR_DEFAULTSIZE,
                    )
                };
                if icon.is_null() {
                    log::warn!("[rust_widgets][windows] set_window_icon: could not load '{path}'");
                    return false;
                }
                // Set both the small (taskbar) and big (alt-tab) icons, which is
                // what the shell reads.
                unsafe {
                    SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
                    SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
                }
            }
        }
        self.state.set_window_icon(widget_id, path);
        true
    }

    fn window_icon(&self, widget_id: ObjectId) -> Option<String> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return None;
        }
        // Win32 stores an HICON, not a path, so the recorded request is the answer.
        self.state.window_icon(widget_id)
    }

    fn backend_name(&self) -> &'static str {
        "WindowsPlatform"
    }

    /// Reads installed physical memory via `GlobalMemoryStatusEx`.
    fn total_memory_mb(&self) -> Option<u64> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::sysinfoapi::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
            // SAFETY: `MEMORYSTATUSEX` is a plain C struct; zeroing it and setting
            // `dwLength` is exactly what the API contract requires. The call only
            // writes into our own stack value.
            let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
            status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
            if ok != 0 {
                return Some(status.ullTotalPhys / (1024 * 1024));
            }
        }
        None
    }

    /// Reports `true` when the system is running on battery power.
    ///
    /// `GetSystemPowerStatus` sets `ACLineStatus` to 0 while discharging; 1 means
    /// AC, and 255 means "unknown", which is treated as AC so a desktop is never
    /// mistaken for a laptop on battery.
    fn is_on_battery(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winbase::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
            // SAFETY: `SYSTEM_POWER_STATUS` is a plain C struct filled by the call
            // from our own stack value.
            let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
            let ok = unsafe { GetSystemPowerStatus(&mut status) };
            if ok != 0 {
                return status.ACLineStatus == 0;
            }
        }
        false
    }

    /// Samples this process's working set against total physical memory.
    fn process_memory_utilization(&self) -> Option<f32> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::processthreadsapi::GetCurrentProcess;
            use winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
            // SAFETY: both structs are plain C layouts owned by this stack frame.
            let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let ok =
                unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
            if ok != 0 {
                let total = self.total_memory_mb()? as f64 * 1024.0 * 1024.0;
                if total > 0.0 {
                    let ratio = (counters.WorkingSetSize as f64 / total) as f32;
                    return Some(ratio.clamp(0.0, 1.0));
                }
            }
        }
        None
    }

    /// CPU load has no cheap, stable Win32 query here, so this backend reports
    /// `None` and the adaptive monitor keeps its default.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// Hands the job file to the shell's `Print` verb via PowerShell.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        let status = std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!(
                "Start-Process -FilePath '{}' -Verb Print -PassThru | Out-Null",
                job_file.display()
            ))
            .status();
        if let Ok(status) = status {
            if status.success() {
                return Ok(());
            }
        }
        Err(format!(
            "no Windows print command could submit job file '{}'; every candidate \
             PowerShell/spooler invocation failed",
            job_file.display()
        ))
    }

    /// The shell `print` verb is always available on Windows.
    fn has_print_support(&self) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "print /? 2>NUL"])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// A library-painted widget gets a child `HWND` of its own class; `WM_PAINT`
    /// blits a frame from `widget::runtime`. See `windows/canvas.rs`.
    ///
    /// Gated on the same profile conditions as `canvas.rs`: without a widget
    /// registry there is no frame to render, so the trait defaults apply and
    /// `supports_surfaces()` reports `false`.
    #[cfg(widgets_unstripped)]
    fn mount_surface(&self, parent: ObjectId, id: ObjectId, rect: crate::core::Rect) -> bool {
        let Some(parent_hwnd) = self.get_native_handle(parent) else {
            log::error!("[windows] mount_surface: unknown parent window {parent}");
            return false;
        };
        let Some(hwnd) = super::canvas::mount_canvas(parent_hwnd, id, rect) else {
            return false;
        };
        self.bind_native_handle(id, hwnd);
        crate::widget::runtime::set_geometry(id, rect);
        true
    }

    #[cfg(widgets_unstripped)]
    fn resize_surface(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        let Some(hwnd) = super::canvas::hwnd_for_widget(id) else {
            log::error!("[windows] resize_surface: id={id} is not mounted");
            return false;
        };
        if !super::canvas::resize_canvas(hwnd, rect) {
            return false;
        }
        crate::widget::runtime::set_geometry(id, rect);
        true
    }

    #[cfg(widgets_unstripped)]
    fn unmount_surface(&self, id: ObjectId) -> bool {
        let Some(hwnd) = super::canvas::hwnd_for_widget(id) else {
            log::error!("[windows] unmount_surface: id={id} is not mounted");
            return false;
        };
        super::canvas::unmount_canvas(hwnd)
    }

    /// `true` only when the widget surface exists for this profile.
    #[cfg(widgets_unstripped)]
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// The window's current client size, asked of Win32.
    ///
    /// `GetClientRect` is the authority once the user has dragged the window edge; the
    /// backend's recorded size is only a fallback for a window Win32 cannot answer for
    /// (off-Windows builds, or an id with no `HWND`).
    #[cfg(target_os = "windows")]
    fn window_client_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
        use winapi::um::winuser::GetClientRect;
        if let Some(hwnd) = self.get_native_handle(window_id) {
            let mut rect = winapi::um::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
            // SAFETY: `hwnd` came from this backend's own handle table and Win32 fills
            // the RECT we pass. Zero dimensions are rejected below rather than reported.
            if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
                let width = (rect.right - rect.left).max(0) as u32;
                let height = (rect.bottom - rect.top).max(0) as u32;
                if width > 0 && height > 0 {
                    return Some((width, height));
                }
            }
        }
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id)
            .or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    fn queue_resize_trigger(&self, window_id: ObjectId, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Invalidate the canvas window so the OS sends a fresh `WM_PAINT`.
    #[cfg(widgets_unstripped)]
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        match super::canvas::hwnd_for_widget(id) {
            Some(hwnd) => {
                super::canvas::invalidate_canvas(hwnd);
                true
            }
            None => false,
        }
    }
    fn invalidate_surface_rect(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        match super::canvas::hwnd_for_widget(id) {
            Some(hwnd) => super::canvas::invalidate_canvas_rect(hwnd, rect),
            None => false,
        }
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// The host's native window handle for a widget.
    ///
    /// `WindowsPlatform` already keeps this mapping for its own use —
    /// `mount_surface` resolves a parent window through it — but the mapping was
    /// reachable only as an *inherent* method, so the trait method kept its `None`
    /// default and the public `native_handle()` accessor reported "no native
    /// object" for windows that plainly had one.
    fn get_native_handle(&self, widget: ObjectId) -> Option<usize> {
        #[cfg(target_os = "windows")]
        {
            WindowsPlatform::get_native_handle(self, widget).map(|hwnd| hwnd as usize)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = widget;
            None
        }
    }
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: true,
            typed_widget_trigger: true,
        }
    }
    fn dpi_scale_factor(&self) -> f32 {
        #[cfg(target_os = "windows")]
        {
            // Query the actual DPI of the primary monitor.
            unsafe {
                let hdc = winapi::um::winuser::GetDC(std::ptr::null_mut());
                if hdc.is_null() {
                    return 1.0;
                }
                let dpi = winapi::um::wingdi::GetDeviceCaps(hdc, winapi::um::wingdi::LOGPIXELSX);
                winapi::um::winuser::ReleaseDC(std::ptr::null_mut(), hdc);
                dpi as f32 / 96.0
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            1.0
        }
    }
    fn init(&self) {
        self.runtime_initialized.store(true, Ordering::SeqCst);
        #[cfg(target_os = "windows")]
        {
            // SAFETY: The platform instance is stored in a `OnceLock<Box<dyn Platform>>`
            // (see `runtime.rs`), so it lives for the entire program duration (`'static`).
            let static_self: &'static WindowsPlatform =
                unsafe { std::mem::transmute::<&WindowsPlatform, &'static WindowsPlatform>(self) };
            notify::register_active_platform(static_self);
        }
        #[cfg(target_os = "windows")]
        unsafe {
            use winapi::um::commctrl::InitCommonControls;
            InitCommonControls();
        }
    }
    fn run(&self) {
        #[cfg(target_os = "windows")]
        unsafe {
            use std::ptr::null_mut;
            use std::thread;
            use std::time::Duration;
            use winapi::um::winuser::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_QUIT,
            };
            self.runtime_running.store(true, Ordering::SeqCst);
            while self.runtime_running.load(Ordering::SeqCst) {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if msg.message == WM_QUIT {
                        self.runtime_running.store(false, Ordering::SeqCst);
                        break;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if self.runtime_running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(10));
                }
            }
            self.runtime_running.store(false, Ordering::SeqCst);
        }
    }
    fn quit(&self) {
        #[cfg(target_os = "windows")]
        unsafe {
            use winapi::um::winuser::PostQuitMessage;
            self.runtime_running.store(false, Ordering::SeqCst);
            PostQuitMessage(0);
        }
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Beyond the authoritative `BackendState` record, the Win32 backend keeps
    /// per-widget entries in the native handle map (`handles`). All of them must be
    /// purged, otherwise a UI rebuilt in a create/destroy loop would leak one entry
    /// per discarded widget. Every lock is scoped to its own statement so no two
    /// guards are ever held at the same time.
    ///
    /// Only the library's own bookkeeping is released here: no Win32 message is
    /// sent and no window is destroyed — the process-wide HWND may still be owned
    /// elsewhere, so `DestroyWindow` is deliberately not called.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.remove(&widget_id);
            } else {
                log::error!("[rust_widgets][windows] destroy_widget: handles mutex poisoned");
            }
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            unsafe extern "system" {
                fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
            }
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, ShowWindow, UpdateWindow, SW_SHOW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            };
            notify::ensure_window_class_registered();
            let class_name = Self::to_wide("RustWidgetsWindowClass");
            let title_wide = Self::to_wide(title);
            // SAFETY: GetModuleHandleW with a null module name returns the handle to
            // the calling process's executable (HINSTANCE). This is safe per MSDN and
            // the return value is only used as the hInstance parameter for CreateWindowExW.
            // A null module name is explicitly documented to be valid.
            let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    title_wide.as_ptr(),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    x,
                    y,
                    width as i32,
                    height as i32,
                    null_mut(),
                    null_mut(),
                    hinstance as _,
                    null_mut(),
                )
            };
            if hwnd.is_null() {
                log::error!(
                    "[rust_widgets][windows] create_window failed for title='{}' (GetLastError={})",
                    title,
                    unsafe { winapi::um::errhandlingapi::GetLastError() }
                );
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::Window, title, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            // `WS_OVERLAPPEDWINDOW` is titled + resizable, so a fresh Win32 window
            // starts restored, windowed, resizable and decorated.
            self.state.init_window_state(
                widget_id,
                crate::platform::state::WindowStateRecord::new_window(),
            );
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                UpdateWindow(hwnd);
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let widget_id =
                self.state.create_widget(WindowsHandleKind::Window, title, x, y, width, height);
            self.state.init_window_state(
                widget_id,
                crate::platform::state::WindowStateRecord::new_window(),
            );
            widget_id
        }
    }
    fn set_clipboard_text(&self, _text: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winbase::GlobalAlloc;
            use winapi::um::winbase::{GlobalLock, GlobalUnlock, GHND};
            use winapi::um::winuser::{
                CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData, CF_UNICODETEXT,
            };

            let text_utf16: Vec<u16> = _text.encode_utf16().chain(std::iter::once(0)).collect();
            let byte_size = text_utf16.len() * 2;
            // SAFETY: Win32 clipboard API calls with proper error checking.

            unsafe {
                if OpenClipboard(std::ptr::null_mut()) == 0 {
                    return false;
                }
                if EmptyClipboard() == 0 {
                    CloseClipboard();
                    return false;
                }
                let h_mem = GlobalAlloc(GHND, byte_size);
                if h_mem.is_null() {
                    CloseClipboard();
                    return false;
                }
                let p_dest = GlobalLock(h_mem) as *mut u16;
                if p_dest.is_null() {
                    GlobalUnlock(h_mem);
                    CloseClipboard();
                    return false;
                }
                std::ptr::copy_nonoverlapping(text_utf16.as_ptr(), p_dest, text_utf16.len());
                GlobalUnlock(h_mem);
                let ret = SetClipboardData(CF_UNICODETEXT, h_mem as _);
                CloseClipboard();
                ret as isize != 0
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = _text;
            false
        }
    }
    fn get_clipboard_text(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winbase::GlobalLock;
            use winapi::um::winuser::{
                CloseClipboard, GetClipboardData, OpenClipboard, CF_UNICODETEXT,
            };

            // SAFETY: Win32 clipboard API calls with proper error checking.
            let result = unsafe {
                if OpenClipboard(std::ptr::null_mut()) == 0 {
                    return String::new();
                }
                let h_mem = GetClipboardData(CF_UNICODETEXT);
                if h_mem.is_null() {
                    CloseClipboard();
                    return String::new();
                }
                let p_src = GlobalLock(h_mem) as *const u16;
                if p_src.is_null() {
                    CloseClipboard();
                    return String::new();
                }
                let mut len = 0;
                while *p_src.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(p_src, len);
                let text = String::from_utf16_lossy(slice);
                CloseClipboard();
                text
            };
            result
        }
        #[cfg(not(target_os = "windows"))]
        {
            String::new()
        }
    }
    fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        #[cfg(target_os = "windows")]
        {
            // State-backed drag-drop (platform native OLE pending)
            // Full OLE implementation (IDropSource/IDropTarget) requires:
            // DoDragDrop, OleInitialize, RegisterDragDrop, RevokeDragDrop
            // See: https://learn.microsoft.com/en-us/windows/win32/shell/dragdrop
            self.state.begin_drag(source_widget_id, mime, payload)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (source_widget_id, mime, payload);
            false
        }
    }
    fn poll_drop_event(&self) -> Option<DropEvent> {
        #[cfg(target_os = "windows")]
        {
            // State-backed drop event polling (OLE polling pending)
            self.state.pop_drop_event()
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    fn inject_drop_event(&self, event: DropEvent) -> bool {
        #[cfg(target_os = "windows")]
        {
            // State-backed drop event injection (OLE injection pending)
            self.state.inject_drop_event(event)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = event;
            false
        }
    }

    // ── IME support ─────────────────────────────────────────────────────────

    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.ime_enabled(widget_id)
    }

    fn ime_bridge(&self) -> Option<&dyn ImeBridge> {
        Some(&self.ime_bridge)
    }

    fn clipboard_backend(&self) -> Option<&dyn RichClipboardBackend> {
        Some(&self.clipboard)
    }

    #[cfg(target_os = "windows")]
    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        Some(&self.a11y_bridge)
    }
}
