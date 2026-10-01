// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `impl Platform for WindowsPlatform` — the main trait implementation.

use crate::core::{ObjectId, PlatformFamily};
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::ime::ImeBridge;
use crate::platform::{
    Platform, PlatformCapabilities, WidgetTriggerEvent, WidgetTriggerKind, WindowStateFlag,
};

// `String` and `Vec` are imported from the compat bridge rather than used bare:
// the `mini` profile is `no_std`, so the std prelude that normally supplies them
// is suppressed and this module failed to compile with 10 `cannot find type
// String/Vec in this scope` errors. Every other backend already routes alloc
// types through `compat` (see `platform/linux/types.rs`).
use crate::compat::{String, Vec};
use crate::platform::types::windows_shell_spawnable;
use crate::platform::windows::notify;
use crate::platform::windows::types::*;
use crate::platform::DropEvent;
use std::sync::atomic::Ordering;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{SW_HIDE, SW_SHOW};

// SAFETY: All Win32 FFI calls in this module follow standard Windows API safety patterns:
// - `CreateWindowExW` return values are checked for null (via `hwnd.is_null()`) before use.
// - `GetLastError` is implicitly checked via the null-return convention; a null HWND indicates
//   that the caller should inspect `GetLastError` for the specific failure code.
// - Pointers passed to Win32 functions must remain valid for the duration of the call; wide
//   strings (`to_wide`) are kept alive via local variables that live across the `unsafe` block.
// - `ShowWindow` / `UpdateWindow` / `MoveWindow` operate on previously-validated HWNDs.
//
// These are `//` rather than `///` because they describe the module's *implementation* discipline
// rather than declaring an item: as `///` they were a doc comment with nothing to attach to, and
// the next declaration's doc comment followed immediately. rustdoc reads that as one document —
// the `FRAME_INTERVAL_MS` paragraph was parsed as a continuation of the bullet list above — which
// `clippy::doc_lazy_continuation` flagged as `doc list item without indentation`. The note is not
// lost: it stays exactly where a reader of the unsafe code looks for it.

/// The frame interval this backend's message loop runs at, in milliseconds.
///
/// The same value as every other backend's twin constant, and named here for the same
/// reason: the sleep between iterations and the delta handed to [`crate::drive_frame`]
/// must be the same number, or every transition runs at the ratio between them. It is
/// defined unconditionally rather than under `cfg(target_os = "windows")` so the value
/// is visible to the loop body's readers on every host, which is where it belongs
/// (principle #42: the value is a frame rate, not an OS fact).
const FRAME_INTERVAL_MS: u64 = 16;

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

    /// The user's **text-size** preference, read from the accessibility setting.
    ///
    /// # Why not the DPI override
    ///
    /// `SPI_GETLOGICALDPIOVERRIDE` reports the DPI *layout* scale, which
    /// [`Self::dpi_scale_factor`] already answers. Text scaling is a separate, deliberate user
    /// choice — Settings → Accessibility → Text size — and Windows stores it as a percentage
    /// under `HKCU\\Software\\Microsoft\\Accessibility\\TextScaleFactor`.
    ///
    /// # Why this matters enough to implement
    ///
    /// [`crate::platform::profile::text_scale`] feeds [`crate::style::environment`], which
    /// scales every control's text. Before this override the trait default reported `1.0`, so a
    /// user who had set 150% got 100% text — the library paid for the plumbing and discarded
    /// the fact. That is the capability gap this closes, and it is the only universally
    /// unimplemented `Platform` method with a live consumer.
    ///
    /// # Return value
    ///
    /// `1.0` whenever the setting cannot be read (a pre-1703 system, no key, a wrong value type,
    /// or a stored `0` that would make text invisible). Every failure returns the honest "no
    /// preference reported" rather than a guess, which is what the trait's documentation
    /// requires.
    #[cfg(target_os = "windows")]
    fn text_scale(&self) -> f32 {
        use winapi::um::winnt::{KEY_READ, REG_DWORD};
        use winapi::um::winreg::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER};
        // SAFETY: both names are NUL-terminated UTF-16 buffers that outlive the calls; `key` is
        // written only on success and closed on every path; `data`/`size`/`kind` are local
        // out-parameters sized before the query.
        unsafe {
            let subkey: Vec<u16> = "Software\\Microsoft\\Accessibility"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let value_name: Vec<u16> =
                "TextScaleFactor".encode_utf16().chain(std::iter::once(0)).collect();
            let mut key = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) != 0 {
                return 1.0;
            }
            let mut data: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            let mut kind: u32 = 0;
            let status = RegQueryValueExW(
                key,
                value_name.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                &mut data as *mut u32 as *mut u8,
                &mut size,
            );
            RegCloseKey(key);
            if status != 0 || kind != REG_DWORD || data == 0 {
                return 1.0;
            }
            // Stored as a percentage (`125` means 125%).
            data as f32 / 100.0
        }
    }

    /// The non-Windows arm inherits the trait's "no preference reported" value.
    ///
    /// Stated explicitly rather than omitted so the Windows implementation above and this one
    /// read together: the pair is one decision ("report it where the host exposes it"), and a
    /// reader searching for `text_scale` should not have to reason about which `cfg` arm applies
    /// to the build they are looking at.
    #[cfg(not(target_os = "windows"))]
    fn text_scale(&self) -> f32 {
        1.0
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
    ///
    /// The path is passed through `$args[0]` rather than interpolated into the
    /// command text. Interpolating it inside a single-quoted PowerShell string
    /// meant a path containing `'` (e.g. `C:\tmp\it's.pdf`) would terminate the
    /// literal and the remainder would execute as PowerShell code.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        let status = std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Start-Process -FilePath $args[0] -Verb Print -PassThru | Out-Null")
            .arg(job_file)
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

    /// Whether this process can reach a print spooler.
    ///
    /// # Why the previous probe was wrong
    ///
    /// It spawned `cmd /C "print /? 2>NUL"` and reported `status.success()`.
    /// `print` is a `cmd` *built-in*, not an executable, so this measured whether
    /// the built-in's usage text exits zero — and the `2>NUL` was `cmd`-dialect
    /// redirection, so the check was not even asking a portable question.
    ///
    /// Principle #38 requires the test to be **spawnability**, not exit status,
    /// for exactly the reason documented on [`unix_print_clients_available`]: a
    /// spooler client that prints usage and exits non-zero is still installed.
    ///
    /// # Why the question is asked in-process
    ///
    /// `spawn_print_job` reaches the spooler through `Start-Process -Verb Print`,
    /// which is the shell's print verb resolved by the current process's own
    /// environment. Asking a child `cmd` about it answers a question about that
    /// child, not about this process. Probing whether `powershell` itself can be
    /// spawned is the fact that actually gates the submission path.
    fn has_print_support(&self) -> bool {
        windows_shell_spawnable()
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
        // SAFETY: `hwnd` was just created by `super::canvas::mount_canvas` (a live child
        // window) and is kept alive for as long as the surface is mounted.
        unsafe { self.bind_native_handle(id, hwnd) };
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
        // The handle association is released **before** the window is destroyed, so the
        // backend can never hand out an `HWND` that Win32 has reclaimed. See
        // `unbind_native_handle` for what a stale (rather than absent) entry would cost.
        // SAFETY: `hwnd` came from `mount_canvas` and is destroyed immediately below, so it
        // is live for the duration of this call.
        unsafe { self.unbind_native_handle(id, hwnd) };
        super::canvas::unmount_canvas(hwnd)
    }

    /// `true` only when the widget surface exists for this profile.
    #[cfg(widgets_unstripped)]
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Shows or hides a top-level window.
    ///
    /// # Why this has to exist
    ///
    /// Nothing else showed or hid a window on Win32. The trait default does nothing, so
    /// `WindowHandle::show()` — which `demo/control` calls and documents as the call that
    /// makes its window visible — was a silent no-op here, and the same for `hide()`. A
    /// window that only appears because `create_window` happened to pass `WS_VISIBLE` is a
    /// window whose visibility the host cannot actually control: hiding it would leave it on
    /// screen, and a host that created one hidden would never get it back.
    ///
    /// # Both id spaces
    ///
    /// A caller reaches here with either the **platform** id `create_window` returned (what a
    /// `WindowHandle` carries) or the **widget-registry** id of the window itself, depending
    /// on which layer it sits in. [`super::window_hwnd_for_widget_id`] resolves both, and an
    /// id that names no window this backend owns is ignored rather than reported: the trait
    /// returns `()`, and most ids arriving here are ordinary controls for which the library's
    /// own `visible` flag is the whole story.
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        #[cfg(widgets_unstripped)]
        {
            let Some(hwnd) = super::window_hwnd_for_widget_id(self, widget_id) else {
                return;
            };
            // A canvas child is a surface, not a window: it is shown by mounting it, and
            // toggling it here would disagree with `unmount_surface`.
            if !unsafe { winapi::um::winuser::GetParent(hwnd) }.is_null() {
                return;
            }
            // SAFETY: `hwnd` is a toplevel this backend created and still holds; `SW_SHOW`
            // and `SW_HIDE` only change its visibility state.
            unsafe {
                let command = if visible { SW_SHOW } else { SW_HIDE };
                winapi::um::winuser::ShowWindow(hwnd, command);
            }
            // A window revealed now must be painted now: the message loop only delivers
            // `WM_PAINT` for a non-empty update region, and revealing a window that nothing
            // has invalidated yet shows its old (or unstyled) pixels until something else
            // happens to dirty it.
            super::canvas::invalidate_for_handle(hwnd);
        }
        #[cfg(not(widgets_unstripped))]
        {
            let _ = (widget_id, visible);
        }
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
            let mut rect = winapi::shared::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
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
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    fn queue_resize_trigger(&self, window_id: ObjectId, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Invalidate the window that draws `id`, so the OS sends a fresh `WM_PAINT`.
    ///
    /// # Why this answers for two kinds of id
    ///
    /// A **mounted surface** has a canvas child window of its own, and invalidating that
    /// repaints just the surface. A **window** has the toplevel `HWND` it was created
    /// with, and invalidating that repaints the window's whole child list — which is where
    /// every ordinary control is drawn, since the library paints them and most have no
    /// native control behind them.
    ///
    /// Handling only the first case is what left `demo/control` blank: the tree painter
    /// existed, but nothing ever told the window it needed to repaint after its controls
    /// were created.
    ///
    /// # The two id spaces
    ///
    /// `id` is a **widget-registry** id (the tree), while this backend's handle table is
    /// keyed by the **platform** id `Platform::create_window` returned for the same window.
    /// `demo/control`'s window is registry id `6000285942072475648` and platform id `1`, so
    /// looking the registry id up in the handle table answered `None` — and this method
    /// reported `false` for a window it plainly owned, which made every repaint request for
    /// a window a silent no-op.
    ///
    /// The translation is therefore two hops, and it is the same pair the tree painter
    /// needs in the opposite direction: registry id → host window → platform id.
    ///
    /// # Why the two hooks are decided by the id, not by trying them in turn
    ///
    /// A canvas `HWND` and a toplevel `HWND` have deliberately different invalidation
    /// behaviour: `invalidate_canvas` marks exactly that child's client area, while a
    /// toplevel is repainted *with* its children — `WM_ERASEBKGND` returns 1, so
    /// `BeginPaint` hands out the whole client area and `paint_window_tree` redraws the
    /// tree over it. Asking Win32 whether the handle has a parent (`GetParent`) picks the
    /// hook for the id instead of guessing, and it is the same distinction the handle
    /// table's client/window kinds encode on the control side.
    ///
    /// A window's `HWND` is preferred over a canvas's when an id somehow has both, because
    /// the window is what draws the tracked children — but in practice an id is exactly
    /// one of the two.
    #[cfg(widgets_unstripped)]
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        if let Some(hwnd) = super::window_hwnd_for_widget_id(self, id) {
            super::canvas::invalidate_for_handle(hwnd);
            return true;
        }
        if let Some(hwnd) = super::canvas::hwnd_for_widget(id) {
            super::canvas::invalidate_canvas(hwnd);
            return true;
        }
        false
    }

    /// Invalidate one rectangle of the canvas window.
    ///
    /// Gated on `widgets_unstripped` for the same reason as the two methods above:
    /// it resolves an id through `super::canvas`, which only exists when the widget
    /// registry does. Without the gate a `windows + mini`/`embedded` build failed to
    /// compile with `cannot find canvas in super`.
    #[cfg(widgets_unstripped)]
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
    /// The host integrations this backend actually provides.
    ///
    /// # Why `native_menu` is `false`
    ///
    /// [`PlatformCapabilities::native_menu`] is documented as "native menu creation and
    /// trigger support", and this backend has none: `grep` over `src/platform/windows/`
    /// finds no `create_menu_bar`, `create_menu`, `menu_add_item`,
    /// `attach_menu_bar_to_window` or `poll_menu_triggered`, so every one of those falls
    /// through to the trait default. A host that trusted the flag would build a native
    /// menu bar and get id `0` back.
    ///
    /// The menu *data* path that does exist (`control_command_to_widget`, driven by
    /// `bind_control_command` and read by `WM_COMMAND`) serves a **host's** own native
    /// controls: the library creates none, because it paints every `WidgetKind` itself, so a
    /// control made through `create_*` never produces a `WM_COMMAND` at all. That is a
    /// routing facility for adopted controls, not a menu bar this backend can build, and it
    /// does not make this flag true. See the notes on the two fields it feeds.
    ///
    /// # Why `ime` is queried rather than asserted
    ///
    /// It was a hard-coded `true`, justified by "`ime_bridge()` returns the TSF bridge". The
    /// bridge **existed**, but its connection flag was set from a `GetProcAddress("TF_GetThreadMgr")`
    /// symbol lookup that called nothing — `msctf.dll` exports that symbol on essentially every
    /// Windows install — so the flag promised an OS IME connection that did not exist. The bridge
    /// now probes with a real `ImmGetContext` query (see `platform::ime_windows::native_ime_available`),
    /// and this flag reports that measurement rather than the presence of a module. On a host where
    /// the probe cannot reach an input context the flag is honestly `false`.
    ///
    /// `dpi_scale_factor()` queries `LOGPIXELSX` on the primary monitor's DC,
    /// `accessibility_bridge()` returns the MSAA/UIA bridge, and typed triggers come from the
    /// shared queue (principle #37).
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: self.ime_bridge.has_native_ime(),
            accessibility: true,
            native_menu: false,
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
        // `init` re-registers the platform and calls `InitCommonControls`, neither of
        // which is idempotent, so the flag guards a second entry rather than merely
        // recording that the first happened. Before this the field was written and
        // never read, so the name promised a guard the code did not have.
        if self.runtime_initialized.swap(true, Ordering::SeqCst) {
            return;
        }
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
                // One library frame after the message pass.
                //
                // A `WM_SIZE` handler queues a `Resized` event for the library (see
                // `crate::drain_triggers`); the Win32 message loop only delivers
                // the OS message, so without the drain the queue would never be read and no
                // window layout would re-run for a window the user resized.
                //
                // The drain alone was half a frame, though: it re-ran layout and advanced
                // no control, so every animation in the library was unreachable from a
                // Win32 window (BLUE24 §0A.1 measurement 1). `crate::drive_frame` is the
                // drain plus the animation step, in that order.
                //
                // Dispatching here is main-thread work: the window procedure ran on
                // this same thread, which is what the widgets require.
                crate::drive_frame(FRAME_INTERVAL_MS as u32);
                if self.runtime_running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
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
    /// Beyond the authoritative `BackendState` record, this backend keeps four per-widget
    /// side tables: the id → `HWND` map, the accessibility bridge's handle registration, the
    /// `control_command_to_widget` map, and the typed-trigger queue. All of them must be
    /// purged, otherwise a UI rebuilt in a create/destroy loop would leak one entry per
    /// discarded widget — and a queued trigger for a widget that no longer exists would be
    /// drained and dispatched against a recycled id.
    ///
    /// The `GWLP_USERDATA` marker on the window needs no cleanup of its own: it lives on the
    /// `HWND`, which dies with the window. Every lock guard is released at the end of its own
    /// statement so no two of this backend's mutexes are ever held at the same time.
    ///
    /// Only the library's own bookkeeping is released here: no Win32 message is
    /// sent and no window is destroyed — the process-wide HWND may still be owned
    /// elsewhere, so `DestroyWindow` is deliberately not called.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            match self.menu_state.handles.lock() {
                Ok(mut handles) => {
                    handles.remove(&widget_id);
                }
                Err(_) => log::error!(
                    "[rust_widgets][windows] destroy_widget: handles mutex poisoned; the entry \
                     for widget {widget_id} was not released"
                ),
            }
            match self.menu_state.control_command_to_widget.lock() {
                Ok(mut commands) => commands.retain(|_, owner| *owner != widget_id),
                Err(_) => log::error!(
                    "[rust_widgets][windows] destroy_widget: command map poisoned; command ids \
                     for widget {widget_id} were not released"
                ),
            }
            // A trigger already queued for a widget that is going away would otherwise be
            // drained and delivered to whatever id is recycled next. `macos_objc2` and
            // `wayland` both retain the same way; this backend did not.
            match self.menu_state.pending_widget_events.lock() {
                Ok(mut events) => events.retain(|event| event.widget_id != widget_id),
                Err(_) => log::error!(
                    "[rust_widgets][windows] destroy_widget: trigger queue poisoned; queued \
                     events for widget {widget_id} were not released"
                ),
            }
            self.a11y_bridge.unregister_handle(widget_id);
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    // ── Typed trigger queue ─────────────────────────────────────────────────
    //
    // `capabilities().typed_widget_trigger` reports `true`, and before these three methods
    // existed that was a **lie**: `WM_COMMAND` and `WM_NOTIFY` pushed events into
    // `menu_state.pending_widget_events`, and nothing ever popped them. A host that trusted
    // the flag, adopted one of its own native controls, clicked it and polled would get
    // `None` forever, while the queue grew without bound.
    //
    // The flag was not wrong to be `true` — the queue really is produced by this backend and
    // really is typed (`WidgetTriggerEvent` carries a `WidgetTriggerKind`, not a bare id).
    // What was missing was the drain. These are the same three methods `android`, `ios`,
    // `harmony` and `wayland` implement over their own queues, so the trait's contract is now
    // met the same way as its siblings.

    /// Pops the oldest queued typed activation.
    ///
    /// Reports nothing for a poisoned queue rather than panicking: the poll happens on
    /// whatever thread the host drains from, and a wedged queue is a condition to surface at
    /// the point of failure (logged below), not to unwind out of a trait method.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        #[cfg(target_os = "windows")]
        {
            match self.menu_state.pending_widget_events.lock() {
                Ok(mut events) => events.pop_front(),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] poll_widget_trigger_event: trigger queue \
                         poisoned; queued activations cannot be delivered"
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

    /// Pops the oldest activation, without its payload.
    ///
    /// # Why this maps onto the typed queue rather than a second store
    ///
    /// The two methods answer the same question at two levels of detail, and a backend that
    /// kept them in separate queues would be able to deliver the same activation twice (once
    /// through each). Deriving the untyped answer from the typed queue makes that impossible
    /// by construction: an event can only be popped once, from one place.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    /// Pops the oldest queued activation belonging to `widget_id`.
    ///
    /// The targeted pop [`crate::drain_widget_triggers_for`] uses, so a host that polls per
    /// widget does not steal a sibling's events. Unknown ids are refused rather than queued: a
    /// trigger for a widget that does not exist could only be dispatched against a recycled
    /// id, which is the defect `destroy_widget` now also guards against.
    fn pop_widget_trigger_event_for(&self, widget_id: ObjectId) -> Option<WidgetTriggerEvent> {
        #[cfg(target_os = "windows")]
        {
            if !self.state.contains_widget(widget_id) {
                return None;
            }
            match self.menu_state.pending_widget_events.lock() {
                Ok(mut events) => events
                    .iter()
                    .position(|event| event.widget_id == widget_id)
                    .and_then(|index| events.remove(index)),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] pop_widget_trigger_event_for: trigger queue \
                         poisoned; the activation for widget {widget_id} cannot be delivered"
                    );
                    None
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = widget_id;
            None
        }
    }

    /// Queues a typed activation as if the host's own control had produced it.
    ///
    /// This is the injection half of the same contract, and the one a test uses: it is how a
    /// host can synthesise an activation for a control that has no native notification route
    /// here (the library paints every `WidgetKind`, so most controls produce none).
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        #[cfg(target_os = "windows")]
        {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            match self.menu_state.pending_widget_events.lock() {
                Ok(mut events) => {
                    events.push_back(WidgetTriggerEvent { widget_id, kind });
                    true
                }
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] inject_widget_trigger_event: trigger queue \
                         poisoned; the activation for widget {widget_id} was not queued"
                    );
                    false
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (widget_id, kind);
            false
        }
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
            // SAFETY: `hwnd` was just returned by `CreateWindowExW` above and checked
            // non-null, so it is a live window for the life of this backend.
            unsafe { self.bind_native_handle(widget_id, hwnd) };
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

    // ── Accessibility metadata ──────────────────────────────────────────────

    /// Records a widget's accessible name and posts it to the OS.
    ///
    /// # Why this exists
    ///
    /// `capabilities().accessibility` promises a native accessibility bridge, and
    /// `accessibility_bridge()` does return a live `WindowsAccessibilityBridge`. But this backend
    /// overrode **neither** of these two methods — nor did it use `impl_platform_state_properties!`,
    /// which is how every other state-backed backend gets them — so the trait default answered
    /// `false`/empty and the bridge's `set_accessibility_name` was unreachable from the `Platform`
    /// API. A host that named a control for a screen reader got a silent no-op: the same "promised
    /// capability with no path to it" shape as the IME flag above.
    ///
    /// The `state` store is where the name lives (so it reads back even with no OS handle yet), and
    /// the bridge is what turns it into a real `NotifyWinEvent` for a registered handle.
    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        let stored = self.state.set_accessibility_name(widget_id, name);
        // The OS half is best-effort and does not change the answer: recording the name succeeded,
        // which is what the caller asked about. `set_accessibility_name` also stores the name in the
        // bridge, so a handle that was not registered yet still gets it when `register_handle` runs.
        #[cfg(target_os = "windows")]
        {
            // `AccessibilityBridge` is imported at the module top.
            self.a11y_bridge.set_accessibility_name(widget_id, name);
        }
        stored
    }

    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.state.accessibility_name(widget_id)
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
