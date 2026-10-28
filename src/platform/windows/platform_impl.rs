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
use crate::platform::types::{windows_shell_spawnable, Display, DisplayId, SystemFont};
use crate::platform::windows::notify;
use crate::platform::windows::types::*;
use crate::platform::DropEvent;
use std::sync::atomic::Ordering;

// The environment facts this backend answers go through the shared cache in
// [`crate::platform::environment_cache`], the same wrapper the Linux, Android and locale paths
// use (P-302). This backend used to keep a local copy whose only invalidation trigger was its own
// one-second TTL, so a notification could not drop a reading and the TTL could drift from every
// other backend's. The shared wrapper records the process-wide notification revision as well as
// the time, so `Platform::notify_environment_changed` — and the `WM_SETTINGCHANGE` arm in
// `types::wnd_proc` that feeds it — invalidates every slot at once (P-208/P-302).
//
// Each slot is a named static rather than a closure-local `OnceLock`, so the fact it holds stays
// greppable:
//
// | fact | slot | source |
// |---|---|---|
// | `text_scale` | [`TEXT_SCALE_CACHE`] | `Software\\Microsoft\\Accessibility` registry value |
// | `system_theme` | [`SYSTEM_THEME_CACHE`] | `Themes\\Personalize` registry value |
// | `dpi_scale_factor` | [`DPI_SCALE_CACHE`] | `GetDeviceCaps(LOGPIXELSX)` |
// | `high_contrast` | [`HIGH_CONTRAST_CACHE`] | `SystemParametersInfoW(SPI_GETHIGHCONTRAST)` |
// | `reduce_motion` | [`REDUCE_MOTION_CACHE`] | `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)` |
// | `system_locale` | [`SYSTEM_LOCALE_CACHE`] | `GetUserDefaultLocaleName` |
#[cfg(all(target_os = "windows", not(alloc_frugal)))]
use crate::platform::environment_cache::{cached_environment_fact, EnvironmentFactCache};

/// The `mini` profile compiles no environment cache (see `platform::environment_cache`), so the
/// same call has to stay available without one: the reading goes to the registry or
/// `SystemParametersInfoW` on every question. That is the honest shape for a sized profile — it
/// trades the repeated read for the slot — and it keeps the call sites below identical in every
/// build.
#[cfg(all(target_os = "windows", alloc_frugal))]
fn cached_environment_fact<T: Copy>(
    _cache: &'static EnvironmentFactCache<T>,
    read: impl FnOnce() -> T,
) -> T {
    read()
}

/// The slot shape, declared only where [`cached_environment_fact`] really caches.
#[cfg(all(target_os = "windows", alloc_frugal))]
type EnvironmentFactCache<T> =
    std::sync::OnceLock<std::sync::Mutex<Option<(std::time::Instant, T)>>>;

#[cfg(target_os = "windows")]
static TEXT_SCALE_CACHE: EnvironmentFactCache<f32> = std::sync::OnceLock::new();
#[cfg(target_os = "windows")]
static SYSTEM_THEME_CACHE: EnvironmentFactCache<Option<bool>> = std::sync::OnceLock::new();
#[cfg(target_os = "windows")]
static DPI_SCALE_CACHE: EnvironmentFactCache<f32> = std::sync::OnceLock::new();
#[cfg(target_os = "windows")]
static HIGH_CONTRAST_CACHE: EnvironmentFactCache<Option<bool>> = std::sync::OnceLock::new();
#[cfg(target_os = "windows")]
static REDUCE_MOTION_CACHE: EnvironmentFactCache<Option<bool>> = std::sync::OnceLock::new();
#[cfg(target_os = "windows")]
static SYSTEM_LOCALE_CACHE: EnvironmentFactCache<Option<&'static str>> = std::sync::OnceLock::new();
// Only `set_widget_visible`'s `widgets_unstripped` body shows or hides a window, so a stripped
// profile would import these and never use them.
#[cfg(all(target_os = "windows", widgets_unstripped))]
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
const FRAME_INTERVAL_MS: u64 = crate::platform::types::FRAME_INTERVAL_MS;

/// Reads `CF_UNICODETEXT` from the Win32 clipboard, or `None` when the clipboard holds no text.
///
/// `Some(String::new())` is a **real** answer — an empty *text* clip — that the merging
/// [`Platform::get_clipboard_text`] cannot express, since it collapses "no text", "a non-text
/// format" and "the read failed" into the same `""`. That distinction is what
/// [`Platform::clipboard_text`] reports, and why this returns `Option` rather than `String`.
///
/// The clipboard is opened and closed inside the call, so a caller never holds it open across a
/// second clipboard call.
#[cfg(target_os = "windows")]
fn read_clipboard_text() -> Option<String> {
    use winapi::um::winbase::GlobalLock;
    use winapi::um::winuser::{CloseClipboard, GetClipboardData, OpenClipboard, CF_UNICODETEXT};

    // SAFETY: every Win32 call below is checked; `p_src` points into the clipboard's own memory,
    // which stays valid until `CloseClipboard`, and the scan stops at its NUL terminator.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let h_mem = GetClipboardData(CF_UNICODETEXT);
        if h_mem.is_null() {
            CloseClipboard();
            return None;
        }
        let p_src = GlobalLock(h_mem) as *const u16;
        if p_src.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0;
        while *p_src.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(p_src, len));
        CloseClipboard();
        Some(text)
    }
}

/// The localized string at `index`, or an empty string when it is absent.
///
/// # Safety
///
/// `strings` must be a live DirectWrite string collection.
#[cfg(target_os = "windows")]
unsafe fn dwrite_localized_string(
    strings: &windows::Win32::Graphics::DirectWrite::IDWriteLocalizedStrings,
    index: u32,
) -> String {
    if index >= strings.GetCount() {
        return String::new();
    }
    let Ok(length) = strings.GetStringLength(index) else {
        return String::new();
    };
    // The API writes `length` UTF-16 units plus a NUL, so the buffer is one unit longer.
    let mut buffer = vec![0u16; length as usize + 1];
    if strings.GetString(index, &mut buffer).is_err() {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

/// The file behind one DirectWrite font, when the system loader can name it.
///
/// # Safety
///
/// `font` must be a live DirectWrite font object.
#[cfg(target_os = "windows")]
unsafe fn dwrite_font_path(
    font: &windows::Win32::Graphics::DirectWrite::IDWriteFont,
) -> Option<std::path::PathBuf> {
    use windows::core::Interface;
    use windows::Win32::Graphics::DirectWrite::{IDWriteFontFile, IDWriteLocalFontFileLoader};

    let face = font.CreateFontFace().ok()?;
    let mut count = 0u32;
    face.GetFiles(&mut count, None).ok()?;
    if count == 0 {
        return None;
    }
    let mut files: Vec<Option<IDWriteFontFile>> = vec![None; count as usize];
    face.GetFiles(&mut count, Some(files.as_mut_ptr())).ok()?;
    files.truncate(count as usize);
    for file in files.into_iter().flatten() {
        let Ok(loader) = file.GetLoader() else {
            continue;
        };
        let Ok(local) = loader.cast::<IDWriteLocalFontFileLoader>() else {
            continue;
        };
        let mut key: *mut core::ffi::c_void = core::ptr::null_mut();
        let mut key_size = 0u32;
        if file.GetReferenceKey(&mut key, &mut key_size).is_err() || key.is_null() || key_size == 0
        {
            continue;
        }
        let Ok(length) = local.GetFilePathLengthFromKey(key, key_size) else {
            continue;
        };
        let mut buffer = vec![0u16; length as usize + 1];
        if local.GetFilePathFromKey(key, key_size, &mut buffer).is_err() {
            continue;
        }
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        if !path.is_empty() {
            return Some(std::path::PathBuf::from(path));
        }
    }
    None
}

/// The installed families from DirectWrite, one entry per family.
#[cfg(target_os = "windows")]
fn dwrite_system_fonts() -> Vec<SystemFont> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

    // SAFETY: the DirectWrite factory and every COM object reached from it are reference-counted by
    // the `windows` crate, so no object can dangle. `CoInitializeEx` is paired with
    // `CoUninitialize` only when it reported success (S_OK or S_FALSE); `RPC_E_CHANGED_MODE` means
    // another component owns the apartment model and must not be uninitialised here.
    unsafe {
        let com_ready = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let fonts = enumerate_directwrite();
        if com_ready {
            CoUninitialize();
        }
        fonts.unwrap_or_default()
    }
}

/// Walks the system collection and picks one face per family.
///
/// # Safety
///
/// Called only by [`dwrite_system_fonts`], inside its COM initialisation scope.
#[cfg(target_os = "windows")]
unsafe fn enumerate_directwrite() -> Option<Vec<SystemFont>> {
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_SHARED,
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    };

    let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
    let mut collection = None;
    factory.GetSystemFontCollection(&mut collection, false).ok()?;
    let collection = collection?;
    let mut out = Vec::new();
    for family_index in 0..collection.GetFontFamilyCount() {
        let Ok(family) = collection.GetFontFamily(family_index) else {
            continue;
        };
        let family_name = match family.GetFamilyNames() {
            Ok(names) => dwrite_localized_string(&names, 0),
            Err(_) => continue,
        };
        if family_name.trim().is_empty() {
            continue;
        }
        let mut best: Option<(SystemFont, u8)> = None;
        for font_index in 0..family.GetFontCount() {
            let Ok(font) = family.GetFont(font_index) else {
                continue;
            };
            let style = match font.GetFaceNames() {
                Ok(names) => dwrite_localized_string(&names, 0),
                Err(_) => String::new(),
            };
            let source = dwrite_font_path(&font);
            let regular = font.GetWeight() == DWRITE_FONT_WEIGHT_NORMAL
                && font.GetStyle() == DWRITE_FONT_STYLE_NORMAL
                && font.GetStretch() == DWRITE_FONT_STRETCH_NORMAL;
            // Prefer the regular face, and among equals prefer one with a readable file: the style
            // column should describe the file the chooser would register.
            let score = u8::from(regular) + u8::from(source.is_some()) * 2;
            let candidate = SystemFont { family: family_name.clone(), style, source };
            match &best {
                Some((_, best_score)) if *best_score >= score => {}
                _ => best = Some((candidate, score)),
            }
        }
        if let Some((font, _)) = best {
            out.push(font);
        }
    }
    Some(out)
}

/// The DWM margins struct, in its ABI form.
///
/// `dwmapi.h` spells four `int` fields in this order; nothing else from that header is needed, so
/// the type is declared here rather than enabling a whole `winapi` feature for one struct.
#[cfg(target_os = "windows")]
#[repr(C)]
#[allow(dead_code)]
struct DwmMargins {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
#[allow(non_snake_case)]
extern "system" {
    /// Sets one DWM window attribute.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a live window handle, and `pv_attribute` must point at
    /// `cb_attribute` readable bytes whose layout matches the attribute being set.
    fn DwmSetWindowAttribute(
        hwnd: winapi::shared::windef::HWND,
        dw_attribute: u32,
        pv_attribute: *const std::ffi::c_void,
        cb_attribute: u32,
    ) -> i32;

    /// Extends the DWM frame into the client area.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a live window handle, and `margins` must point at one valid `MARGINS`.
    fn DwmExtendFrameIntoClientArea(
        hwnd: winapi::shared::windef::HWND,
        margins: *const DwmMargins,
    ) -> i32;
}

/// One checked call to `DwmSetWindowAttribute`.
///
/// # Safety
///
/// `hwnd` must be live and `value` must point at `size` readable bytes for the attribute.
#[cfg(target_os = "windows")]
unsafe fn dwm_set_window_attribute(
    hwnd: winapi::shared::windef::HWND,
    attribute: u32,
    value: *const std::ffi::c_void,
    size: u32,
) -> i32 {
    // SAFETY: the caller guarantees the handle and buffer; this wrapper only forwards them.
    unsafe { DwmSetWindowAttribute(hwnd, attribute, value, size) }
}

/// Enables or disables client-area composition for a layered window.
///
/// # Safety
///
/// `hwnd` must be a live window handle; the margins are a local value that outlives the call.
#[cfg(target_os = "windows")]
unsafe fn dwm_extend_frame_into_client_area(
    hwnd: winapi::shared::windef::HWND,
    enable: bool,
) -> Result<(), i32> {
    let margins = if enable {
        DwmMargins { left: -1, top: -1, right: -1, bottom: -1 }
    } else {
        DwmMargins { left: 0, top: 0, right: 0, bottom: 0 }
    };
    let hr =
        // SAFETY: the caller guarantees a live handle; `margins` is this frame's local value.
        unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins as *const DwmMargins) };
    if hr >= 0 {
        Ok(())
    } else {
        Err(hr)
    }
}

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
                    // SAFETY: `hwnd` was resolved from the live widget map above and the call only changes window state.
                    WindowStateFlag::Maximized => unsafe {
                        let cmd = if on { SW_MAXIMIZE } else { SW_RESTORE };
                        ShowWindow(hwnd, cmd);
                    },
                    // SAFETY: `hwnd` was resolved from the live widget map above and the call only changes window state.
                    WindowStateFlag::Minimized => unsafe {
                        let cmd = if on { SW_MINIMIZE } else { SW_RESTORE };
                        ShowWindow(hwnd, cmd);
                    },
                    WindowStateFlag::Fullscreen => {
                        // Win32 has no "full screen" window flag: it is achieved by
                        // dropping the frame styles and filling the monitor. Reapply
                        // the styles to leave it, which is what the WM would do.
                        // SAFETY: `hwnd` is live; the style read/write pair is the documented way to toggle a frame.
                        unsafe {
                            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                            let frame =
                                WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
                            let new_style = if on { style & !frame } else { style | frame };
                            SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                            ShowWindow(hwnd, SW_MAXIMIZE);
                        }
                    }
                    // SAFETY: `hwnd` is live; the style read/write pair is the documented way to toggle a frame.
                    WindowStateFlag::Resizable => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        let new_style = if on {
                            style | WS_THICKFRAME | WS_MAXIMIZEBOX
                        } else {
                            style & !(WS_THICKFRAME | WS_MAXIMIZEBOX)
                        };
                        SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                    },
                    // SAFETY: `hwnd` is live; the style read/write pair is the documented way to toggle a frame.
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
                    // SAFETY: `hwnd` was resolved from the live widget map above; these are read-only queries.
                    WindowStateFlag::Maximized => unsafe { IsZoomed(hwnd) != 0 },
                    // SAFETY: `hwnd` was resolved from the live widget map above; these are read-only queries.
                    WindowStateFlag::Minimized => unsafe { IsIconic(hwnd) != 0 },
                    // Full screen is "no frame styles left", the inverse of the
                    // Decorated computation below.
                    // SAFETY: `hwnd` was resolved from the live widget map above; the style query is read-only.
                    WindowStateFlag::Fullscreen => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        style & (WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX) == 0
                    },
                    // SAFETY: `hwnd` was resolved from the live widget map above; the style query is read-only.
                    WindowStateFlag::Resizable => unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        style & (WS_THICKFRAME | WS_MAXIMIZEBOX) != 0
                    },
                    // SAFETY: `hwnd` was resolved from the live widget map above; the style query is read-only.
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
                // SAFETY: `hwnd` is live; `SetWindowPos` only repositions that window.
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

    /// Records a maximum content size and asks Win32 to recompute its frame constraints.
    ///
    /// Win32 has no `SetWindowMaxSize`; like the minimum, the value is consumed by the
    /// `WM_GETMINMAXINFO` handler in `wnd_proc` (`ptMaxTrackSize`). A `SetWindowPos` with only the
    /// `SWP_NO*` flags makes the system ask again immediately, so a *reduction* applies now rather
    /// than at the next user resize.
    fn set_window_max_size(&self, widget_id: ObjectId, width: u32, height: u32) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        self.state.set_window_max_size(widget_id, width, height);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
            };
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                // SAFETY: `hwnd` is live; the call only re-runs the min/max query and changes no
                // position, size or z-order (`SWP_NOSIZE` is what keeps the zero extents from
                // being applied as a new size).
                unsafe {
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
        }
        true
    }

    /// Toggles a layered window; see the trait method for what each toolkit expresses.
    ///
    /// `WS_EX_LAYERED` is Win32's window-transparency bit. On a compositing desktop the DWM then
    /// combines the window's alpha with what is behind it; `DwmExtendFrameIntoClientArea` with
    /// `-1` margins lets the client area participate, which is the part a plain layered window does
    /// not do. Removing the bit restores an ordinary opaque window. Colour-key transparency is
    /// deliberately not used: it would punch out one colour rather than honour the surface's own
    /// alpha channel.
    fn set_window_transparent(&self, widget_id: ObjectId, transparent: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA,
                WS_EX_LAYERED,
            };
            let Some(hwnd) = self.get_native_handle(widget_id) else {
                return false;
            };
            // SAFETY: `hwnd` is live; the style read/write pair is the documented way to toggle an
            // extended window style, and the layered attribute only changes this window's alpha.
            unsafe {
                let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
                let updated =
                    if transparent { style | WS_EX_LAYERED } else { style & !WS_EX_LAYERED };
                SetWindowLongW(hwnd, GWL_EXSTYLE, updated as i32);
                if transparent {
                    SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
                }
                if let Err(error) = dwm_extend_frame_into_client_area(hwnd, transparent) {
                    // Composition may be unavailable in a remote/headless session. The layered
                    // bit is still set, so this is a stated limit rather than a silent success.
                    log::debug!(
                        "[windows] transparent window {widget_id}: DWM composition unavailable \
                         ({error}); only the layered-window bit was applied"
                    );
                }
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = transparent;
            false
        }
    }

    /// Keeps the window above ordinary windows with `SetWindowPos(HWND_TOPMOST)`.
    fn set_window_always_on_top(&self, widget_id: ObjectId, on: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            };
            let Some(hwnd) = self.get_native_handle(widget_id) else {
                return false;
            };
            let insert_after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
            // SAFETY: `hwnd` is live; only its z-order changes, with position and size left alone.
            unsafe {
                SetWindowPos(
                    hwnd,
                    insert_after,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                ) != 0
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = on;
            false
        }
    }

    /// Sets the window's immersive dark-mode attribute through `DwmSetWindowAttribute`.
    ///
    /// The attribute id is `DWMWA_USE_IMMERSIVE_DARK_MODE` (20) on current Windows and 19 on the
    /// first builds that shipped it; both are tried because the documented id moved. A failure from
    /// both means this DWM does not expose the attribute, which is reported rather than claimed.
    fn set_window_preferred_theme(&self, widget_id: ObjectId, dark: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            let Some(hwnd) = self.get_native_handle(widget_id) else {
                return false;
            };
            // A Windows BOOL is an i32; `DwmSetWindowAttribute` reads exactly that many bytes.
            let value: i32 = if dark { 1 } else { 0 };
            let size = std::mem::size_of::<i32>() as u32;
            for attribute in [20u32, 19u32] {
                // SAFETY: `hwnd` is live and `value` outlives the call; the API copies
                // `size` bytes from it into its own storage.
                let hr = unsafe {
                    dwm_set_window_attribute(
                        hwnd,
                        attribute,
                        &value as *const i32 as *const _,
                        size,
                    )
                };
                if hr >= 0 {
                    return true;
                }
            }
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = dark;
            false
        }
    }

    /// Excludes the window from capture through `SetWindowDisplayAffinity`.
    ///
    /// `WDA_EXCLUDEFROMCAPTURE` (0x11) is the Windows 10 2004+ answer; older systems reject it and
    /// this reports `false` rather than falling back to `WDA_MONITOR`, which blanks the window
    /// instead of merely protecting it.
    fn set_window_content_protected(&self, widget_id: ObjectId, protected: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Window) {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SetWindowDisplayAffinity, WDA_NONE};
            const WDA_EXCLUDEFROMCAPTURE: u32 = 0x0000_0011;
            let Some(hwnd) = self.get_native_handle(widget_id) else {
                return false;
            };
            let affinity = if protected { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
            // SAFETY: `hwnd` is live and the affinity value is one of the two documented flags.
            unsafe { SetWindowDisplayAffinity(hwnd, affinity) != 0 }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = protected;
            false
        }
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
                // SAFETY: `hwnd` is live and `icon` is the handle produced by the load call just above.
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
        cached_environment_fact(&TEXT_SCALE_CACHE, || {
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
        })
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

    /// The Win32 clipboard sequence number, or `0` when the API reports none.
    ///
    /// `GetClipboardSequenceNumber` increments on every clipboard change system-wide; a host
    /// compares readings instead of subscribing to `WM_CLIPBOARDUPDATE` (BLUE28 B28.22).
    #[cfg(target_os = "windows")]
    fn clipboard_generation(&self) -> u64 {
        use winapi::um::winuser::GetClipboardSequenceNumber;
        // SAFETY: the API takes no arguments and returns a plain sequence number.
        u64::from(unsafe { GetClipboardSequenceNumber() })
    }

    #[cfg(not(target_os = "windows"))]
    fn clipboard_generation(&self) -> u64 {
        0
    }

    /// The system's app appearance, read from the Personalize registry key.
    ///
    /// # Why this key
    ///
    /// Windows stores the light/dark choice for *apps* (as distinct from the taskbar and the
    /// window frame) at
    /// `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
    /// The value is a `DWORD` where `0` means dark and `1` means light; this is the same key the
    /// shell and winit's Win32 backend read.
    ///
    /// # Why `None` on failure
    ///
    /// A missing key (older builds), a wrong value type, or an unreadable key means the OS did not
    /// tell us. Answering `Some(false)` would claim the user chose light, which the style layer
    /// would then render — the fabricated-default shape principle #37 forbids. High-contrast mode
    /// is deliberately not folded in: it is a separate accessibility setting, and a host that needs
    /// it reads [`crate::style::environment::EnvironmentProvider::high_contrast`].
    #[cfg(target_os = "windows")]
    fn system_theme(&self) -> Option<bool> {
        use winapi::um::winnt::{KEY_READ, REG_DWORD};
        use winapi::um::winreg::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER};
        cached_environment_fact(&SYSTEM_THEME_CACHE, || {
            // SAFETY: both names are NUL-terminated UTF-16 buffers that outlive the calls; `key` is
            // written only on success and closed on every path; `data`/`size`/`kind` are local
            // out-parameters sized before the query.
            unsafe {
                let subkey: Vec<u16> =
                    "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                let value_name: Vec<u16> =
                    "AppsUseLightTheme".encode_utf16().chain(std::iter::once(0)).collect();
                let mut key = std::ptr::null_mut();
                if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) != 0 {
                    return None;
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
                if status != 0 || kind != REG_DWORD {
                    return None;
                }
                // `0` is dark, `1` is light. Any other value is not a state this setting defines, so
                // it is treated as "cannot tell" rather than guessed.
                match data {
                    0 => Some(true),
                    1 => Some(false),
                    _ => None,
                }
            }
        })
    }

    /// The non-Windows arm inherits the trait's honest "cannot tell".
    ///
    /// Stated explicitly for the same reason as the `text_scale` pair above: one decision, both
    /// arms visible together.
    #[cfg(not(target_os = "windows"))]
    fn system_theme(&self) -> Option<bool> {
        None
    }

    /// Whether Windows' high-contrast mode is on, from `SPI_GETHIGHCONTRAST`.
    ///
    /// `HIGHCONTRASTW.dwFlags & HCF_HIGHCONTRASTON` is the OS's own answer, the same one
    /// `SystemParametersInfoW` reports to every application. `None` when the call fails.
    ///
    /// Cached for the shared one-second environment TTL: this is read on every style-environment
    /// poll, and `SystemParametersInfoW` is a synchronous system call. The TTL expiry is the only
    /// invalidation trigger today (see `cached_environment_fact`).
    #[cfg(target_os = "windows")]
    fn high_contrast(&self) -> Option<bool> {
        use winapi::shared::minwindef::DWORD;
        use winapi::um::winuser::{
            SystemParametersInfoW, HCF_HIGHCONTRASTON, HIGHCONTRASTW, SPI_GETHIGHCONTRAST,
        };
        cached_environment_fact(&HIGH_CONTRAST_CACHE, || {
            // SAFETY: `HIGHCONTRASTW` is a plain C struct sized by `cbSize`; the call writes only
            // into the local value, and the string pointer is left null because this query does
            // not use it.
            unsafe {
                let mut contrast: HIGHCONTRASTW = std::mem::zeroed();
                contrast.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
                let ok = SystemParametersInfoW(
                    SPI_GETHIGHCONTRAST,
                    contrast.cbSize,
                    &mut contrast as *mut HIGHCONTRASTW as *mut _,
                    0,
                );
                if ok == 0 {
                    return None;
                }
                let flags: DWORD = contrast.dwFlags;
                Some(flags & HCF_HIGHCONTRASTON != 0)
            }
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn high_contrast(&self) -> Option<bool> {
        None
    }

    /// Whether the client-area animation setting is off, from `SPI_GETCLIENTAREAANIMATION`.
    ///
    /// Windows stores this as a `BOOL`: `0` means animations are disabled, which is what the
    /// accessibility "Show animations in Windows" switch writes. `None` when the query fails.
    ///
    /// Cached for the shared one-second environment TTL; see `cached_environment_fact` for the
    /// invalidation trigger.
    #[cfg(target_os = "windows")]
    fn reduce_motion(&self) -> Option<bool> {
        use winapi::shared::minwindef::BOOL;
        use winapi::um::winuser::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION};
        cached_environment_fact(&REDUCE_MOTION_CACHE, || {
            // SAFETY: the out-parameter is a local `BOOL`, which is exactly what this action
            // writes.
            unsafe {
                let mut animations: BOOL = 1;
                let ok = SystemParametersInfoW(
                    SPI_GETCLIENTAREAANIMATION,
                    0,
                    &mut animations as *mut BOOL as *mut _,
                    0,
                );
                if ok == 0 {
                    return None;
                }
                Some(animations == 0)
            }
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn reduce_motion(&self) -> Option<bool> {
        None
    }

    /// The user's locale, from `GetUserDefaultLocaleName`.
    ///
    /// The API writes a BCP-47-ish tag such as `en-US` into the caller's buffer. An empty answer or
    /// a failure is `None`, never a fabricated `en-US`.
    ///
    /// Cached for the shared one-second environment TTL. `GetUserDefaultLocaleName` and the
    /// interner's global lock were both paid on every style-environment poll; the interned tag is a
    /// process-lifetime `&'static str`, so a cached hit hands back the same pointer the uncached
    /// call would have. See `cached_environment_fact` for the invalidation trigger.
    #[cfg(target_os = "windows")]
    fn system_locale(&self) -> Option<&'static str> {
        use winapi::um::winnls::GetUserDefaultLocaleName;
        // `LOCALE_NAME_MAX_LENGTH` from `winnls.h`; the API documents this as the required size.
        const LOCALE_NAME_MAX_LENGTH: usize = 85;
        cached_environment_fact(&SYSTEM_LOCALE_CACHE, || {
            // SAFETY: the buffer is owned by this closure frame and is exactly the documented size.
            // The returned length includes the NUL and is bounded by the buffer size.
            unsafe {
                let mut buffer = [0u16; LOCALE_NAME_MAX_LENGTH];
                let written = GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32);
                if written <= 0 {
                    return None;
                }
                let length = (written as usize - 1).min(buffer.len());
                let tag = String::from_utf16_lossy(&buffer[..length]);
                crate::platform::locale::intern(Some(tag))
            }
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn system_locale(&self) -> Option<&'static str> {
        None
    }

    /// A revision of the environment facts, derived from their current values.
    ///
    /// Delegated to [`crate::platform::locale::environment_revision`], the shared accessor, so the
    /// hashed fact set is the one the style snapshot consumes. Spelling the four facts out here is
    /// what let an accent-colour or reduced-transparency change go unnoticed for the life of the
    /// process (P-03), and what made adding a fact an eight-file edit (P-06).
    fn environment_generation(&self) -> u64 {
        crate::platform::locale::environment_revision(self)
    }

    /// The installed font families from DirectWrite, one entry per family.
    ///
    /// GDI's `EnumFontFamiliesExW` reports family names only. DirectWrite's
    /// `IDWriteFontFile` + `IDWriteLocalFontFileLoader` pair names the file behind a face, which
    /// is what lets a chosen system font actually render. The regular face is preferred, and
    /// among equal styles the one with a readable file.
    #[cfg(target_os = "windows")]
    fn system_fonts(&self) -> Vec<SystemFont> {
        dwrite_system_fonts()
    }

    #[cfg(not(target_os = "windows"))]
    fn system_fonts(&self) -> Vec<SystemFont> {
        Vec::new()
    }

    /// Applies a cursor shape through `LoadCursorW`/`SetCursor`.
    ///
    /// Win32 has no per-window cursor object; `SetCursor` changes the cursor for the current
    /// message. `WM_SETCURSOR` can reset it on the next move, which is why the runtime re-applies
    /// the intent on hover changes rather than per motion event.
    #[cfg(target_os = "windows")]
    fn set_cursor(&self, _window: ObjectId, icon: crate::platform::CursorIcon) -> bool {
        use crate::platform::CursorIcon;
        use winapi::um::winuser::{
            LoadCursorW, SetCursor, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO,
            IDC_SIZEALL, IDC_SIZENS, IDC_SIZEWE, IDC_WAIT,
        };
        let name = match icon {
            CursorIcon::Default => IDC_ARROW,
            CursorIcon::Pointer => IDC_HAND,
            CursorIcon::Text => IDC_IBEAM,
            CursorIcon::Crosshair | CursorIcon::ZoomIn | CursorIcon::ZoomOut => IDC_CROSS,
            CursorIcon::Move | CursorIcon::Grab | CursorIcon::Grabbing => IDC_SIZEALL,
            CursorIcon::NotAllowed => IDC_NO,
            CursorIcon::ResizeHorizontal | CursorIcon::ResizeNwSe | CursorIcon::ResizeNeSw => {
                IDC_SIZEWE
            }
            CursorIcon::ResizeVertical => IDC_SIZENS,
            CursorIcon::Wait | CursorIcon::Progress => IDC_WAIT,
            CursorIcon::Help => IDC_HELP,
        };
        // SAFETY: a shared system cursor is loaded by resource id and installed; neither call
        // takes ownership of a pointer.
        unsafe {
            let cursor = LoadCursorW(std::ptr::null_mut(), name);
            if cursor.is_null() {
                return false;
            }
            let _ = SetCursor(cursor);
        }
        true
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
        // `spawn` rather than `status`: the platform contract is "launch the print verb", and
        // `status()` blocked the print worker until PowerShell exited (PLAT-03). The async print
        // path already reports completion through its handle; a launch failure is still an
        // `Err`, and the child is reaped by the OS once PowerShell exits.
        std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Start-Process -FilePath $args[0] -Verb Print -PassThru | Out-Null")
            .arg(job_file)
            .spawn()
            .map(|_child| ())
            .map_err(|error| {
                format!("could not launch the Windows print verb for {job_file:?}: {error}")
            })
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
    /// id that names no window this backend owns answers `false` rather than quietly doing
    /// nothing — most ids arriving here are ordinary controls for which the library's own
    /// `visible` flag is the whole story, and the library paints those itself, so there is no
    /// window here to show or hide (P-203).
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) -> bool {
        #[cfg(widgets_unstripped)]
        {
            let Some(hwnd) = super::window_hwnd_for_widget_id(self, widget_id) else {
                return false;
            };
            // A canvas child is a surface, not a window: it is shown by mounting it, and
            // toggling it here would disagree with `unmount_surface`.
            if !unsafe { winapi::um::winuser::GetParent(hwnd) }.is_null() {
                return false;
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
            true
        }
        #[cfg(not(widgets_unstripped))]
        {
            let _ = (widget_id, visible);
            false
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

    /// The window's `HWND` as a `raw-window-handle` 0.6 Win32 handle.
    #[cfg(feature = "raw-window-handle")]
    fn raw_window_handle(&self, window: ObjectId) -> Option<raw_window_handle::RawWindowHandle> {
        #[cfg(target_os = "windows")]
        {
            let hwnd = WindowsPlatform::get_native_handle(self, window)?;
            let hwnd = core::num::NonZeroIsize::new(hwnd as isize)?;
            Some(raw_window_handle::RawWindowHandle::Win32(
                raw_window_handle::Win32WindowHandle::new(hwnd),
            ))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
            None
        }
    }

    /// The process's Win32 display handle; rwh's variant carries no data.
    #[cfg(all(feature = "raw-window-handle", target_os = "windows"))]
    fn raw_display_handle(&self) -> Option<raw_window_handle::RawDisplayHandle> {
        Some(raw_window_handle::RawDisplayHandle::Windows(
            raw_window_handle::WindowsDisplayHandle::new(),
        ))
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
            // `set/get_clipboard_text` use the Win32 clipboard (`OpenClipboard` /
            // `SetClipboardData`), so a copy is visible outside this process.
            clipboard: true,
            // `open_external` spawns `cmd /C start`.
            open_external: true,
            // `create_file_dialog` returns a logical handle and `choose_path` presents the
            // Win32 common dialogs (`GetOpenFileNameW` / `GetSaveFileNameW`,
            // `SHBrowseForFolderW`), so the flag and the methods are one implementation.
            file_dialog: true,
            // DirectWrite's system collection names each family's backing file through
            // `IDWriteFontFile` + `IDWriteLocalFontFileLoader` (B28.39).
            system_fonts: true,
        }
    }

    /// Presents the Win32 picker — see [`super::file_picker`] for which dialog answers which
    /// purpose, how Cancel is told apart from a dialog that failed to appear, and what the answer
    /// does and does not claim.
    fn choose_path(
        &self,
        purpose: crate::platform::PathPurpose,
        title: &str,
    ) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
        super::file_picker::choose_path(purpose, title)
    }

    /// [`Self::choose_path`] with the caller's filters in the dialog's filter list (P-203).
    ///
    /// The win32 common dialogs express the list as `label\0*.ext[;*.ext]\0…`, which is what
    /// `file_picker::filter_list` builds. A directory chooser ignores the list: `SHBrowseForFolderW`
    /// has no filter control.
    fn choose_path_with_filters(
        &self,
        purpose: crate::platform::PathPurpose,
        title: &str,
        filters: &[crate::platform::PathFilter],
    ) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
        super::file_picker::choose_path_with_filters(purpose, title, filters)
    }

    /// Hands a URL or path to the shell's default handler through `cmd /C start`.
    ///
    /// The empty `""` after `start` is its *window title* argument: without it, a quoted
    /// target would be consumed as the title and nothing would be opened. See
    /// [`Platform::open_external`] for the hand-off contract.
    fn open_external(&self, target: &str) -> Result<(), crate::platform::types::OpenError> {
        crate::platform::external::spawn_handler("cmd", &["/C", "start", ""], target)
    }

    /// Whether `cmd` is on this host's `PATH`.
    ///
    /// The companion of [`Self::open_external`]: that method opens through `cmd /C start`, so "is
    /// the handler installed" is exactly "is `cmd` findable on `PATH`" (as `cmd.exe`, through
    /// `PATHEXT`). See [`Platform::has_external_handler`] for why this is a separate,
    /// side-effect-free query rather than the capability flag (which reports only that the backend
    /// has the method).
    fn has_external_handler(&self) -> bool {
        crate::platform::external::program_is_on_path("cmd")
    }
    fn dpi_scale_factor(&self) -> f32 {
        #[cfg(target_os = "windows")]
        {
            cached_environment_fact(&DPI_SCALE_CACHE, || {
                // Query the actual DPI of the primary monitor.
                // SAFETY: the DC is released on every path; `GetDeviceCaps` only reads it.
                unsafe {
                    let hdc = winapi::um::winuser::GetDC(std::ptr::null_mut());
                    if hdc.is_null() {
                        return 1.0;
                    }
                    let dpi =
                        winapi::um::wingdi::GetDeviceCaps(hdc, winapi::um::wingdi::LOGPIXELSX);
                    winapi::um::winuser::ReleaseDC(std::ptr::null_mut(), hdc);
                    dpi as f32 / 96.0
                }
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            1.0
        }
    }

    /// The pointer's position in the desktop's screen space (SP-119).
    ///
    /// `GetCursorPos` answers in physical screen pixels, the same space `GetPixel` reads, so the
    /// two answers can be combined into "what colour is under the cursor". The coordinates are
    /// deliberately not divided by [`Self::dpi_scale_factor`]: the sampler is a screen-space read,
    /// not a window-local one.
    #[cfg(target_os = "windows")]
    fn global_pointer_position(&self) -> Option<crate::core::Point> {
        // SAFETY: `GetCursorPos` only writes into the caller's `POINT`; the stack value is valid
        // for the call and nothing is retained.
        unsafe {
            let mut point = winapi::shared::windef::POINT { x: 0, y: 0 };
            if winapi::um::winuser::GetCursorPos(&mut point) == 0 {
                return None;
            }
            Some(crate::core::Point::new(point.x, point.y))
        }
    }

    /// The colour of the desktop pixel at `position` (SP-119).
    ///
    /// `GetPixel` on the screen DC reads the composited desktop, which is exactly what an
    /// eyedropper picks. `CLR_INVALID` is the documented "that point is not on the desktop"
    /// answer, and it is mapped to `None` rather than to a colour.
    #[cfg(target_os = "windows")]
    fn sample_screen_color(&self, position: crate::core::Point) -> Option<crate::core::Color> {
        // SAFETY: the screen DC is released on every path, and `GetPixel` only reads it.
        unsafe {
            let hdc = winapi::um::winuser::GetDC(std::ptr::null_mut());
            if hdc.is_null() {
                return None;
            }
            let pixel = winapi::um::wingdi::GetPixel(hdc, position.x, position.y);
            winapi::um::winuser::ReleaseDC(std::ptr::null_mut(), hdc);
            if pixel == winapi::um::wingdi::CLR_INVALID {
                return None;
            }
            // `COLORREF` is 0x00BBGGRR.
            let red = (pixel & 0x0000_00FF) as u8;
            let green = ((pixel & 0x0000_FF00) >> 8) as u8;
            let blue = ((pixel & 0x00FF_0000) >> 16) as u8;
            Some(crate::core::Color::rgb(red, green, blue))
        }
    }

    /// The DPI scale of the monitor the window is on (P-207).
    ///
    /// [`Self::dpi_scale_factor`] answers the primary monitor's scale because it has no window to
    /// ask about. A host laying out one window on a mixed-DPI desktop needs that window's own
    /// monitor, which Win32 exposes as `GetDpiForWindow` (Windows 10 1607+) with a monitor-DC
    /// fallback on older systems. The lookup is live rather than cached: `WM_DPICHANGED` already
    /// invalidates the shared environment revision, so a cached global value would be re-read, and
    /// reading the window directly also reflects a move that happened without a message reaching
    /// this backend.
    #[cfg(target_os = "windows")]
    fn dpi_scale_factor_for_window(&self, window_id: ObjectId) -> f32 {
        #[cfg(widgets_unstripped)]
        let hwnd = super::window_hwnd_for_widget_id(self, window_id);
        #[cfg(not(widgets_unstripped))]
        let hwnd = self.get_native_handle(window_id);
        hwnd.map(super::displays::dpi_scale_factor_for_window_handle)
            .unwrap_or_else(|| self.dpi_scale_factor())
    }

    /// Non-Windows builds have no window to resolve; the global answer is the documented default.
    #[cfg(not(target_os = "windows"))]
    fn dpi_scale_factor_for_window(&self, _window_id: ObjectId) -> f32 {
        self.dpi_scale_factor()
    }

    /// The monitors Win32 reports; see [`super::displays`].
    ///
    /// This backend used to answer the trait's empty default, so a host could not enumerate
    /// monitors, locate a window on one, or notice that the set changed (see
    /// `docs/issues/feedback-20261015.md` §十二). `EnumDisplayMonitors` supplies all of it, and
    /// the enumeration is callable from any thread, so no main-thread guard is needed.
    fn displays(&self) -> Vec<Display> {
        super::displays::displays()
    }

    /// The monitor a window is on, from `MonitorFromWindow`; see [`super::displays`].
    ///
    /// # Both id spaces
    ///
    /// The id a host holds for a window is the **platform** id `create_window` returned, which the
    /// handle table resolves directly. A host that holds the window's **widget-registry** id (the
    /// window as a widget) reaches it through the same association `set_widget_visible` and the
    /// paint path use — [`super::window_hwnd_for_widget_id`], which resolves both. A profile with
    /// no widget runtime has neither a registry id nor that helper, so it resolves the platform id
    /// alone.
    #[cfg(all(target_os = "windows", widgets_unstripped))]
    fn display_of_window(&self, window_id: ObjectId) -> Option<DisplayId> {
        super::window_hwnd_for_widget_id(self, window_id)
            .and_then(super::displays::display_of_window_handle)
    }

    /// See the `widgets_unstripped` sibling above; a stripped profile has no registry ids.
    #[cfg(all(target_os = "windows", not(widgets_unstripped)))]
    fn display_of_window(&self, window_id: ObjectId) -> Option<DisplayId> {
        self.get_native_handle(window_id).and_then(super::displays::display_of_window_handle)
    }

    /// A revision of Win32's current display set; see [`super::displays::generation`].
    fn display_generation(&self) -> u64 {
        super::displays::generation()
    }

    /// Windows does not lay system UI over a window's client area.
    ///
    /// The taskbar and the window chrome live outside it, so all four safe-area distances are
    /// zero — a measurement, not a fallback. Returning `Some(zeros)` is what keeps the widget layer
    /// from substituting its portrait-phone default (44/34) on a desktop window. This matches
    /// winit's Win32 backend, which answers `PhysicalInsets::new(0, 0, 0, 0)`.
    fn safe_area_insets(
        &self,
        _window_id: ObjectId,
    ) -> Option<crate::platform::types::SafeAreaInsets> {
        Some(crate::platform::types::SafeAreaInsets { top: 0, bottom: 0, left: 0, right: 0 })
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
                let outcome = crate::drive_frame(FRAME_INTERVAL_MS as u32);
                if self.runtime_running.load(Ordering::SeqCst) {
                    // Block in the native message queue. An input message wakes this
                    // immediately (so the loop keeps zero input latency), while a still loop
                    // waits the longer idle interval instead of waking every 16 ms.
                    crate::platform::frame_wait::pause_between_frames(outcome.needs_another_frame);
                }
            }
            self.runtime_running.store(false, Ordering::SeqCst);
        }
    }
    fn quit(&self) -> bool {
        // `runtime_running` is the flag the Win32 loop reads every iteration, so storing it
        // stops the loop from any thread within one frame: that store is the stop, and it holds
        // however this was called. `PostQuitMessage` is an extra wake-up for the common case
        // where the caller *is* the pump's thread -- there it makes the loop notice immediately
        // instead of waiting for its next tick. It is Windows-only (no other backend has a pump
        // to wake) and it is thread-affine, posting to the *calling* thread's queue, so from
        // another thread it wakes nothing; the flag above is what ends the loop in that case.
        self.runtime_running.store(false, Ordering::SeqCst);
        // The Win32 pump may be waiting on `MsgWaitForMultipleObjectsEx`; the library's frame
        // wait must learn about the stop immediately rather than after the idle interval.
        crate::platform::frame_wait::wake();
        #[cfg(target_os = "windows")]
        // SAFETY: posted to the current thread queue; no pointer or handle is involved.
        unsafe {
            use winapi::um::winuser::PostQuitMessage;
            PostQuitMessage(0);
        }
        true
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
            self.menu_state.purge(widget_id);
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

    /// Creates a logical file-chooser handle.
    ///
    /// The native chooser in this backend is [`Self::choose_path`] (`GetOpenFileNameW` /
    /// `GetSaveFileNameW`), which is what the `file_dialog` capability flag describes. The library
    /// paints the chooser control itself, so this creator returns a state handle of the same kind
    /// rather than a second, shadow Win32 dialog object; a `0` would tell the caller there is no
    /// control while the capability says there is a picker (P-204).
    fn create_file_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(WindowsHandleKind::FileDialog, "Open File", x, y, width, height)
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
            // `width`/`height` are a **client** size on every other backend — AppKit's
            // `contentRect`, GTK's default size, the state backends' surface — and `new_window`
            // documents them that way. Win32's `CreateWindowExW` takes the *outer* size, so fold
            // the frame and caption in first. Without this the client came out a title bar shorter
            // than asked and a bottom-anchored status bar was laid out below the visible area: the
            // window looked like it had no status bar at all.
            let mut frame = winapi::shared::windef::RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            };
            // SAFETY: `frame` is a local `RECT` in client coordinates, which is exactly what
            // `AdjustWindowRect` documents; it only reads the style and menu flag.
            unsafe {
                winapi::um::winuser::AdjustWindowRect(&mut frame, WS_OVERLAPPEDWINDOW, 0);
            }
            let outer_width = frame.right - frame.left;
            let outer_height = frame.bottom - frame.top;
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    title_wide.as_ptr(),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    x,
                    y,
                    outer_width,
                    outer_height,
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
            // This backend now owns one more top-level window. The count is decremented in
            // `WM_DESTROY`; only when it returns to zero does the message loop quit (see
            // `types::release_library_window`). Registered after the handle is known-good so a
            // failed creation does not leave a phantom window in the count.
            super::types::register_library_window();
            // P-303: the TSF probe ran when the platform was constructed, which may have been
            // before this thread had a window at all, and its answer was then fixed for the life
            // of the process. Re-probe now that an `HWND` exists, so `capabilities().ime`
            // — which reads `has_native_ime` — reports an input context the host can really
            // have. One COM activation per window, not per frame.
            let _ = self.ime_bridge.refresh_native_availability();
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
            // The merging accessor: no text, a non-text format and a failed read all become the
            // same `""`. A caller that must tell them apart uses `clipboard_text` below.
            read_clipboard_text().unwrap_or_default()
        }
        #[cfg(not(target_os = "windows"))]
        {
            String::new()
        }
    }

    /// The clipboard's text, or `None` when there is no text to read.
    ///
    /// # Why this overrides the trait default on Windows
    ///
    /// The default maps an empty string to `None`, which would report an **empty text clip** — a
    /// real clipboard state Win32 can hold — as "no text". Asking the clipboard for its
    /// `CF_UNICODETEXT` handle directly tells the two apart: a null handle is "no text", a present
    /// handle whose contents are empty is `Some("")`. This is the same precision the Linux/GTK,
    /// Android, iOS and HarmonyOS backends already answer, and the distinction a paste action
    /// branches on (`docs/issues/feedback-20261015.md` §8.6).
    fn clipboard_text(&self) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            read_clipboard_text()
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
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
