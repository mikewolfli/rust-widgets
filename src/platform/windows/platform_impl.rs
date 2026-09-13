// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `impl Platform for WindowsPlatform` — the main trait implementation.

use crate::core::Orientation;
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::ime::ImeBridge;
use crate::platform::{
    EchoMode, EmbeddedCapabilityContract, NativeCapabilityContract, Platform, PlatformCapabilities,
    WidgetTriggerEvent, WidgetTriggerKind, WindowStateFlag,
};

use crate::platform::windows::helpers::*;
use crate::platform::windows::notify;
use crate::platform::windows::types::*;
use crate::platform::DropEvent;
use std::sync::atomic::Ordering;

#[cfg(target_os = "windows")]
use winapi::shared::windef::HMENU;

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
    fn show_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, true);
        #[cfg(target_os = "windows")]
        {
            use crate::platform::windows::dialogs::present_native_dialog;
            if let Some(kind) = self.state.kind_of(widget_id) {
                if matches!(
                    kind,
                    WindowsHandleKind::MessageBox
                        | WindowsHandleKind::FileDialog
                        | WindowsHandleKind::ColorDialog
                        | WindowsHandleKind::FontDialog
                        | WindowsHandleKind::DirectoryDialog
                ) {
                    present_native_dialog(self, widget_id, kind);
                    return;
                }
            }
        }
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            // SAFETY: hwnd is guaranteed valid by get_native_handle() which only returns
            // handles registered via bind_native_handle(). ShowWindow and UpdateWindow are
            // safe to call on a valid HWND per MSDN documentation. The window is owned by
            // this platform instance and the HWND remains valid for the lifetime of the widget.
            unsafe {
                use winapi::um::winuser::{ShowWindow, UpdateWindow, SW_SHOW};
                ShowWindow(hwnd, SW_SHOW);
                UpdateWindow(hwnd);
            }
        }
    }
    fn hide_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, false);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::{ShowWindow, SW_HIDE};
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::MoveWindow;
                MoveWindow(hwnd, x, y, width as i32, height as i32, 1);
            }
        }
    }
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        let _ = self.state.set_text(widget_id, text);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            // SAFETY: hwnd is a valid HWND from get_native_handle(). SetWindowTextW
            // accepts a null-terminated wide string (guaranteed by to_wide() which appends
            // a null terminator). Per MSDN, SetWindowTextW is thread-safe for distinct
            // windows and the buffer is read-only during the call.
            unsafe {
                use winapi::um::winuser::SetWindowTextW;
                let text_wide = Self::to_wide(text);
                SetWindowTextW(hwnd, text_wide.as_ptr());
            }
        }
    }
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    use winapi::um::winuser::{GetWindowTextLengthW, GetWindowTextW};
                    let len = GetWindowTextLengthW(hwnd);
                    if len > 0 {
                        let mut buffer = vec![0u16; (len + 1) as usize];
                        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), len + 1);
                        if copied > 0 {
                            return String::from_utf16_lossy(&buffer[..copied as usize]);
                        }
                    }
                }
            }
        }
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::EnableWindow;
                EnableWindow(hwnd, if enabled { 1 } else { 0 });
            }
        }
    }
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    use winapi::um::winuser::IsWindowEnabled;
                    return IsWindowEnabled(hwnd) != 0;
                }
            }
        }
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        if visible {
            self.show_widget(widget_id);
        } else {
            self.hide_widget(widget_id);
        }
    }
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    use winapi::um::winuser::IsWindowVisible;
                    return IsWindowVisible(hwnd) != 0;
                }
            }
        }
        self.state.visible(widget_id)
    }

    fn set_widget_value(&self, widget_id: ObjectId, value: f64) -> bool {
        // The kind gates the Win32 message: sending PBM_SETPOS to a BUTTON would
        // be meaningless. Gate on the *recorded* kind so the decision is testable
        // on non-Windows hosts too.
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !WindowsPlatform::kind_accepts_numeric_value(kind) {
            return false;
        }
        self.state.set_value(widget_id, value);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::{PBM_SETPOS, TBM_SETPOS, UDM_SETPOS32};
            use winapi::um::winuser::SendMessageW;
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                match kind {
                    // Trackbar and up-down are common controls; the message differs
                    // per class, so the recorded kind picks it. `wparam` is `usize`
                    // and `lparam` is `isize` in the Win32 ABI.
                    super::types::WindowsHandleKind::Slider => unsafe {
                        SendMessageW(hwnd, TBM_SETPOS, 1, value.round() as isize);
                    },
                    super::types::WindowsHandleKind::SpinBox
                    | super::types::WindowsHandleKind::DoubleSpinBox => unsafe {
                        SendMessageW(hwnd, UDM_SETPOS32, 0, value.round() as isize);
                    },
                    _ => unsafe {
                        SendMessageW(hwnd, PBM_SETPOS, value.round() as usize, 0);
                    },
                }
            }
        }
        true
    }

    fn widget_value(&self, widget_id: ObjectId) -> Option<f64> {
        let kind = self.state.kind_of(widget_id)?;
        if !WindowsPlatform::kind_accepts_numeric_value(kind) {
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::{PBM_GETPOS, TBM_GETPOS, UDM_GETPOS32};
            use winapi::um::winuser::SendMessageW;
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let raw = match kind {
                    super::types::WindowsHandleKind::Slider => unsafe {
                        SendMessageW(hwnd, TBM_GETPOS, 0, 0)
                    },
                    super::types::WindowsHandleKind::SpinBox
                    | super::types::WindowsHandleKind::DoubleSpinBox => unsafe {
                        SendMessageW(hwnd, UDM_GETPOS32, 0, 0)
                    },
                    _ => unsafe { SendMessageW(hwnd, PBM_GETPOS, 0, 0) },
                };
                return Some(raw as f64);
            }
        }
        self.state.value(widget_id)
    }

    fn set_widget_range(&self, widget_id: ObjectId, min: f64, max: f64) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(
            kind,
            super::types::WindowsHandleKind::Slider
                | super::types::WindowsHandleKind::SpinBox
                | super::types::WindowsHandleKind::DoubleSpinBox
        ) {
            return false;
        }
        self.state.set_range(widget_id, min, max);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::{PBM_SETRANGE32, TBM_SETRANGE, UDM_SETRANGE32};
            use winapi::um::winuser::SendMessageW;
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let min_i = min.round() as isize;
                let max_i = max.round() as isize;
                match kind {
                    super::types::WindowsHandleKind::Slider => unsafe {
                        // TBM_SETRANGE takes `(min << 16) | max` in lParam, so each
                        // bound is a 16-bit signed value — enough for every slider
                        // range used in this crate, and it is what the native
                        // trackbar stores.
                        let lo = (min_i as i16 as u16) as isize;
                        let hi = (max_i as i16 as u16) as isize;
                        let packed = (lo << 16) | (hi & 0xFFFF);
                        SendMessageW(hwnd, TBM_SETRANGE, 1, packed);
                    },
                    super::types::WindowsHandleKind::SpinBox => unsafe {
                        SendMessageW(hwnd, UDM_SETRANGE32, min_i as usize, max_i);
                    },
                    _ => unsafe {
                        SendMessageW(hwnd, PBM_SETRANGE32, min_i as usize, max_i);
                    },
                }
            }
        }
        true
    }

    fn widget_range(&self, widget_id: ObjectId) -> Option<(f64, f64)> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(
            kind,
            super::types::WindowsHandleKind::Slider
                | super::types::WindowsHandleKind::SpinBox
                | super::types::WindowsHandleKind::DoubleSpinBox
        ) {
            return None;
        }
        self.state.range(widget_id)
    }

    fn set_widget_selected_index(&self, widget_id: ObjectId, index: Option<usize>) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        let Some(selected) = index else {
            // Win32 combo/list boxes always have a valid selection (index 0 after
            // items exist); a true "no selection" state is not representable, so
            // report the limitation instead of faking it (principle #37).
            return false;
        };
        match kind {
            super::types::WindowsHandleKind::ComboBox
            | super::types::WindowsHandleKind::FontComboBox => {
                self.combo_box_set_current_index(widget_id, selected)
            }
            super::types::WindowsHandleKind::ListBox => {
                self.list_box_set_current_index(widget_id, selected)
            }
            _ => false,
        }
    }

    fn widget_selected_index(&self, widget_id: ObjectId) -> Option<usize> {
        match self.state.kind_of(widget_id)? {
            super::types::WindowsHandleKind::ComboBox
            | super::types::WindowsHandleKind::FontComboBox => {
                self.combo_box_current_index(widget_id)
            }
            super::types::WindowsHandleKind::ListBox => self.list_box_current_index(widget_id),
            _ => None,
        }
    }

    fn set_widget_checked(&self, widget_id: ObjectId, checked: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(
            kind,
            super::types::WindowsHandleKind::CheckBox
                | super::types::WindowsHandleKind::RadioButton
                | super::types::WindowsHandleKind::ToggleButton
        ) {
            return false;
        }
        self.state.set_checked(widget_id, checked);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, BM_SETCHECK, BST_CHECKED, BST_UNCHECKED};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let state = if checked { BST_CHECKED } else { BST_UNCHECKED } as usize;
                unsafe {
                    SendMessageW(hwnd, BM_SETCHECK, state, 0);
                }
            }
        }
        true
    }

    fn is_widget_checked(&self, widget_id: ObjectId) -> Option<bool> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(
            kind,
            super::types::WindowsHandleKind::CheckBox
                | super::types::WindowsHandleKind::RadioButton
                | super::types::WindowsHandleKind::ToggleButton
        ) {
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, BM_GETCHECK, BST_CHECKED};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let state = unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) };
                return Some(state == BST_CHECKED as isize);
            }
        }
        self.state.checked(widget_id)
    }

    fn set_widget_step(&self, widget_id: ObjectId, step: f64) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(
            kind,
            super::types::WindowsHandleKind::Slider
                | super::types::WindowsHandleKind::SpinBox
                | super::types::WindowsHandleKind::DoubleSpinBox
        ) {
            return false;
        }
        self.state.set_step(widget_id, step);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::{TBM_SETLINESIZE, UDM_SETACCEL};
            use winapi::um::winuser::SendMessageW;
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                match kind {
                    super::types::WindowsHandleKind::Slider => unsafe {
                        SendMessageW(hwnd, TBM_SETLINESIZE, 0, step.round() as isize);
                    },
                    // The up-down's stride is expressed through an acceleration
                    // table; a single-entry table gives a constant step.
                    _ => unsafe {
                        use winapi::um::commctrl::UDACCEL;
                        let mut accel: [UDACCEL; 1] = std::mem::zeroed();
                        accel[0].nInc = step.round().clamp(1.0, 65535.0) as u32;
                        accel[0].nSec = 0;
                        SendMessageW(hwnd, UDM_SETACCEL, 1, accel.as_mut_ptr() as isize);
                    },
                }
            }
        }
        true
    }

    fn widget_step(&self, widget_id: ObjectId) -> Option<f64> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(
            kind,
            super::types::WindowsHandleKind::Slider
                | super::types::WindowsHandleKind::SpinBox
                | super::types::WindowsHandleKind::DoubleSpinBox
        ) {
            return None;
        }
        self.state.step(widget_id)
    }

    fn set_widget_indeterminate(&self, widget_id: ObjectId, indeterminate: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(
            kind,
            super::types::WindowsHandleKind::ProgressBar
                | super::types::WindowsHandleKind::ActivityIndicator
                | super::types::WindowsHandleKind::ProgressDialog
        ) {
            return false;
        }
        self.state.set_indeterminate(widget_id, indeterminate);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::PBM_SETMARQUEE;
            use winapi::um::winuser::SendMessageW;
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                // `PBM_SETMARQUEE` starts/stops the marquee animation; the control
                // must have been created with `PBS_MARQUEE` for it to take effect.
                let start = if indeterminate { 1usize } else { 0usize };
                unsafe {
                    SendMessageW(hwnd, PBM_SETMARQUEE, start, 30);
                }
            }
        }
        true
    }

    fn is_widget_indeterminate(&self, widget_id: ObjectId) -> Option<bool> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(
            kind,
            super::types::WindowsHandleKind::ProgressBar
                | super::types::WindowsHandleKind::ActivityIndicator
                | super::types::WindowsHandleKind::ProgressDialog
        ) {
            return None;
        }
        self.state.indeterminate(widget_id)
    }

    fn set_widget_read_only(&self, widget_id: ObjectId, read_only: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return false;
        }
        self.state.set_read_only(widget_id, read_only);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, EM_SETREADONLY};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let flag = if read_only { 1usize } else { 0usize };
                unsafe {
                    SendMessageW(hwnd, EM_SETREADONLY as u32, flag, 0);
                }
            }
        }
        true
    }

    fn is_widget_read_only(&self, widget_id: ObjectId) -> Option<bool> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return None;
        }
        self.state.read_only(widget_id)
    }

    fn set_widget_max_length(&self, widget_id: ObjectId, max_length: u32) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return false;
        }
        self.state.set_max_length(widget_id, max_length);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, EM_SETLIMITTEXT};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    SendMessageW(hwnd, EM_SETLIMITTEXT as u32, max_length as usize, 0);
                }
            }
        }
        true
    }

    fn widget_max_length(&self, widget_id: ObjectId) -> Option<u32> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return None;
        }
        self.state.max_length(widget_id)
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
        // handler in `rw_wnd_proc`, which reads it back from this state model. That
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

    fn set_widget_selection(&self, widget_id: ObjectId, start: u32, end: u32) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return false;
        }
        self.state.set_selection(widget_id, start, end);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, EM_SETSEL};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                // EM_SETSEL takes (start, end) in lParam as (start << 16) | end for
                // classic edit controls; the newer form uses lParam = -1 to
                // select all. Explicit bounds are passed here.
                let packed = ((start as isize) << 16) | (end as isize);
                unsafe {
                    SendMessageW(hwnd, EM_SETSEL as u32, 0, packed);
                }
            }
        }
        true
    }

    fn widget_selection(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return None;
        }
        // `EM_GETSEL` returns the selection packed into the *return value* rather
        // than through out-parameters, which the `SendMessageW` signature cannot
        // express; the recorded range is therefore the authoritative answer here.
        self.state.selection(widget_id)
    }

    fn set_widget_placeholder(&self, widget_id: ObjectId, text: &str) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::EM_SETCUEBANNER;
            use winapi::um::winuser::SendMessageW;
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let wide = Self::to_wide(text);
                // `EM_SETCUEBANNER`: wparam = TRUE to show the cue even when the
                // control is focused; lparam = pointer to the cue string.
                unsafe {
                    SendMessageW(hwnd, EM_SETCUEBANNER as u32, 1, wide.as_ptr() as isize);
                }
            }
        }
        self.state.set_placeholder(widget_id, text);
        true
    }

    fn widget_placeholder(&self, widget_id: ObjectId) -> Option<String> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return None;
        }
        // Win32 exposes no `EM_GETCUEBANNER`, so the recorded text is the answer.
        self.state.placeholder(widget_id)
    }

    fn set_widget_echo_mode(&self, widget_id: ObjectId, mode: EchoMode) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return false;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, EM_SETPASSWORDCHAR};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                // A non-zero password char masks input; 0 restores plain text.
                // `NoEcho` has no Win32 equivalent and is reported as a refusal
                // below rather than silently behaving like `Normal`.
                let ch = match mode {
                    EchoMode::Normal => 0u32,
                    // U+2022 BULLET, the conventional password mask on Windows.
                    EchoMode::Password => 0x2022u32,
                    EchoMode::NoEcho => {
                        log::warn!(
                            "[rust_widgets][windows] set_widget_echo_mode: NoEcho has no Win32 \
                             equivalent; refusing"
                        );
                        return false;
                    }
                };
                unsafe {
                    SendMessageW(hwnd, EM_SETPASSWORDCHAR as u32, ch as usize, 0);
                }
                // Force a repaint, or the mask change is not drawn until the next
                // natural invalidation.
                unsafe {
                    use winapi::shared::minwindef::FALSE;
                    use winapi::um::winuser::InvalidateRect;
                    InvalidateRect(hwnd, std::ptr::null(), FALSE);
                }
            }
        }
        self.state.set_echo_mode(widget_id, mode);
        true
    }

    fn widget_echo_mode(&self, widget_id: ObjectId) -> Option<EchoMode> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::LineEdit) {
            return None;
        }
        self.state.echo_mode(widget_id)
    }

    fn set_slider_orientation(&self, widget_id: ObjectId, orientation: Orientation) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::Slider) {
            return false;
        }
        self.state.set_orientation(widget_id, orientation);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::TBS_VERT;
            use winapi::um::winuser::{
                GetWindowLongW, SetWindowLongW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
                SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
            };
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                    let new_style = if orientation == Orientation::Vertical {
                        style | TBS_VERT
                    } else {
                        style & !TBS_VERT
                    };
                    SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                    // SWP_FRAMECHANGED asks the control to re-read its style. This
                    // is the only lever Win32 offers — there is no TBM_* message
                    // for orientation — which is why the API is creation-time.
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                    );
                }
            }
        }
        true
    }

    fn slider_orientation(&self, widget_id: ObjectId) -> Option<Orientation> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::Slider) {
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::TBS_VERT;
            use winapi::um::winuser::{GetWindowLongW, GWL_STYLE};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
                return Some(if style & TBS_VERT != 0 {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                });
            }
        }
        self.state.orientation(widget_id)
    }

    fn set_widget_tristate(&self, widget_id: ObjectId, enabled: bool) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(
            kind,
            super::types::WindowsHandleKind::CheckBox
                | super::types::WindowsHandleKind::RadioButton
        ) {
            return false;
        }
        self.state.set_tristate(widget_id, enabled);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                GetWindowLongW, SetWindowLongW, BS_3STATE, BS_AUTO3STATE, BS_AUTOCHECKBOX,
                BS_CHECKBOX, GWL_STYLE,
            };
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                    // Swap the button type between the 2-state and 3-state
                    // variants, preserving the "auto" behaviour and every other
                    // style bit.
                    let new_style = if enabled {
                        if style & BS_AUTOCHECKBOX != 0 {
                            (style & !BS_AUTOCHECKBOX) | BS_AUTO3STATE
                        } else if style & BS_CHECKBOX != 0 {
                            (style & !BS_CHECKBOX) | BS_3STATE
                        } else {
                            style | BS_AUTO3STATE
                        }
                    } else if style & BS_AUTO3STATE != 0 {
                        (style & !BS_AUTO3STATE) | BS_AUTOCHECKBOX
                    } else if style & BS_3STATE != 0 {
                        (style & !BS_3STATE) | BS_CHECKBOX
                    } else {
                        style
                    };
                    SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                }
            }
        }
        true
    }

    fn is_widget_tristate(&self, widget_id: ObjectId) -> Option<bool> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(
            kind,
            super::types::WindowsHandleKind::CheckBox
                | super::types::WindowsHandleKind::RadioButton
        ) {
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{GetWindowLongW, BS_3STATE, BS_AUTO3STATE, GWL_STYLE};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
                return Some(style & (BS_3STATE | BS_AUTO3STATE) != 0);
            }
        }
        self.state.tristate(widget_id)
    }

    fn set_widget_group(&self, widget_id: ObjectId, group: &str) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::RadioButton) {
            return false;
        }
        self.state.set_group(widget_id, group);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{GetWindowLongW, SetWindowLongW, GWL_STYLE, WS_GROUP};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                // `WS_GROUP` marks where a mutually-exclusive run *begins*: buttons
                // up to the next WS_GROUP toggle together. Setting it on the first
                // button of a group is what Win32 uses to express this, so every
                // button in a named group gets the flag and the run is closed by
                // the following control.
                unsafe {
                    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                    SetWindowLongW(hwnd, GWL_STYLE, (style | WS_GROUP) as i32);
                }
            }
        }
        true
    }

    fn widget_group(&self, widget_id: ObjectId) -> Option<String> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::RadioButton) {
            return None;
        }
        // Win32 has no group *name* to read back — only the WS_GROUP style bit — so
        // the recorded name is the answer.
        self.state.group(widget_id)
    }

    fn set_widget_scroll_position(&self, widget_id: ObjectId, x: i32, y: i32) -> bool {
        let Some(kind) = self.state.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, super::types::WindowsHandleKind::ScrollArea) {
            return false;
        }
        self.state.set_scroll(widget_id, x, y);
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SetScrollPos, SB_HORZ, SB_VERT};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                // The container is created with WS_HSCROLL | WS_VSCROLL, so the
                // standard scroll-bar messages apply. `SetScrollPos` takes `c_int`
                // parameters, hence the explicit casts.
                unsafe {
                    SetScrollPos(hwnd, SB_HORZ as i32, x as i32, 1);
                    SetScrollPos(hwnd, SB_VERT as i32, y as i32, 1);
                }
            }
        }
        true
    }

    fn widget_scroll_position(&self, widget_id: ObjectId) -> Option<(i32, i32)> {
        let kind = self.state.kind_of(widget_id)?;
        if !matches!(kind, super::types::WindowsHandleKind::ScrollArea) {
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{GetScrollPos, SB_HORZ, SB_VERT};
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let x = unsafe { GetScrollPos(hwnd, SB_HORZ as i32) };
                let y = unsafe { GetScrollPos(hwnd, SB_VERT as i32) };
                return Some((x, y));
            }
        }
        self.state.scroll(widget_id)
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
        Err("system print command failed on windows".to_string())
    }

    /// The shell `print` verb is always available on Windows.
    fn has_print_support(&self) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "print /? 2>NUL"])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Win32 primitives this backend constructs natively.
    ///
    /// `SpinBox` uses `UPDOWN_CLASS` (`msctls_updown32`), `ListView` uses
    /// `SysListView32`, and `ScrollArea` a `WS_HSCROLL | WS_VSCROLL` child window.
    /// Publishing them here lets control routing promote them to
    /// `NativePreferred` without any `cfg(target_os)` in the routing table.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    fn native_widget_kinds(&self) -> &'static [crate::widget::WidgetKind] {
        use crate::widget::WidgetKind;
        &[WidgetKind::SpinBox, WidgetKind::ListView, WidgetKind::ScrollArea]
    }

    /// A self-drawn widget gets a child `HWND` of its own class; `WM_PAINT`
    /// blits a frame from `widget::runtime`. See `windows/canvas.rs`.
    ///
    /// Gated on the same profile conditions as `canvas.rs`: without a widget
    /// registry there is no frame to render, so the trait defaults apply and
    /// `supports_custom_widgets()` reports `false`.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    fn mount_custom_widget(&self, parent: ObjectId, id: ObjectId, rect: crate::core::Rect) -> bool {
        let Some(parent_hwnd) = self.get_native_handle(parent) else {
            log::error!("[windows] mount_custom_widget: unknown parent window {parent}");
            return false;
        };
        let Some(hwnd) = super::canvas::mount_canvas(parent_hwnd, id, rect) else {
            return false;
        };
        self.bind_native_handle(id, hwnd);
        crate::widget::runtime::set_geometry(id, rect);
        true
    }

    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    fn resize_custom_widget(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        let Some(hwnd) = super::canvas::hwnd_for_widget(id) else {
            log::error!("[windows] resize_custom_widget: id={id} is not mounted");
            return false;
        };
        if !super::canvas::resize_canvas(hwnd, rect) {
            return false;
        }
        crate::widget::runtime::set_geometry(id, rect);
        true
    }

    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    fn unmount_custom_widget(&self, id: ObjectId) -> bool {
        let Some(hwnd) = super::canvas::hwnd_for_widget(id) else {
            log::error!("[windows] unmount_custom_widget: id={id} is not mounted");
            return false;
        };
        super::canvas::unmount_canvas(hwnd)
    }

    /// `true` only when the self-drawn surface exists for this profile.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    fn supports_custom_widgets(&self) -> bool {
        true
    }

    /// Invalidate the canvas window so the OS sends a fresh `WM_PAINT`.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    fn repaint_custom_widget(&self, id: ObjectId) -> bool {
        match super::canvas::hwnd_for_widget(id) {
            Some(hwnd) => {
                super::canvas::invalidate_canvas(hwnd);
                true
            }
            None => false,
        }
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
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
    fn native_capability_contract(&self) -> Option<NativeCapabilityContract> {
        Some(NativeCapabilityContract::from_platform_caps(self.capabilities()))
    }
    fn embedded_capability_contract(&self) -> Option<EmbeddedCapabilityContract> {
        None
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
                    // Accelerators must be offered to every window that has an
                    // HACCEL table before normal dispatch: TranslateAcceleratorW
                    // turns a matching key press into the item's WM_COMMAND, and
                    // returns 0 for everything else so the message falls through
                    // to TranslateMessage/DispatchMessageW unchanged.
                    let translated = self.try_translate_accelerator(&msg);
                    if !translated {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
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
    /// per-widget entries in four side tables: the native handle map (`handles`),
    /// the menu ownership map (`menu_owner_window`), the menu command map
    /// (`menu_command_to_item`) and the native-dialog metadata (`dialog_data`).
    /// Control command ids (`control_command_to_widget`, keyed by command id
    /// rather than widget id) are swept by value so no entry outlives its widget.
    /// All of them must be purged, otherwise a UI rebuilt in a create/destroy
    /// loop would leak one entry per discarded widget. Every lock is scoped to its
    /// own statement so no two guards are ever held at the same time.
    ///
    /// Only the library's own bookkeeping is released here: no Win32 message is
    /// sent and no window is destroyed — the process-wide HWND may still be owned
    /// elsewhere, so `DestroyWindow` is deliberately not called.
    ///
    /// A window's accelerator table is released with it, otherwise each
    /// create/destroy cycle would leak an `HACCEL`.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            crate::platform::windows::accel::release_accelerator_table(widget_id);
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.remove(&widget_id);
            } else {
                log::error!("[rust_widgets][windows] destroy_widget: handles mutex poisoned");
            }
            if let Ok(mut owners) = self.menu_state.menu_owner_window.lock() {
                owners.remove(&widget_id);
            } else {
                log::error!(
                    "[rust_widgets][windows] destroy_widget: menu_owner_window mutex poisoned"
                );
            }
            if let Ok(mut map) = self.menu_state.menu_command_to_item.lock() {
                map.retain(|_, item| *item != widget_id);
            } else {
                log::error!(
                    "[rust_widgets][windows] destroy_widget: menu_command_to_item mutex poisoned"
                );
            }
            if let Ok(mut map) = self.menu_state.control_command_to_widget.lock() {
                map.retain(|_, owner| *owner != widget_id);
            } else {
                log::error!(
                    "[rust_widgets][windows] destroy_widget: control_command_to_widget mutex poisoned"
                );
            }
            if let Ok(mut data) = self.dialog_data.lock() {
                data.remove(&widget_id);
            } else {
                log::error!("[rust_widgets][windows] destroy_widget: dialog_data mutex poisoned");
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
    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, BS_PUSHBUTTON, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
            };
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Button");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | BS_PUSHBUTTON,
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
                log::error!(
                    "[rust_widgets][windows] create_button failed for text='{}' (GetLastError={})",
                    text,
                    unsafe { winapi::um::errhandlingapi::GetLastError() }
                );
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::Button, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            unsafe {
                self.bind_control_command(widget_id, hwnd);
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.state.create_widget(WindowsHandleKind::Button, text, x, y, width, height)
        }
    }
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if let Some(id) = try_create_label(self, parent, text, x, y, width, height) {
                return id;
            }
            log::error!("[rust_widgets][windows] create_label failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, BS_AUTOCHECKBOX, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
            };
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Button");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | BS_AUTOCHECKBOX,
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
                log::error!("[rust_widgets][windows] create_checkbox failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::CheckBox, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            unsafe {
                self.bind_control_command(widget_id, hwnd);
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, BS_AUTORADIOBUTTON, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
            };
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Button");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | BS_AUTORADIOBUTTON,
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
                log::error!("[rust_widgets][windows] create_radio_button failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::RadioButton, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            unsafe {
                self.bind_control_command(widget_id, hwnd);
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, ES_AUTOVSCROLL, ES_LEFT, ES_MULTILINE, WS_BORDER, WS_CHILD,
                WS_TABSTOP, WS_VISIBLE,
            };
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Edit");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP
                        | WS_VISIBLE
                        | WS_CHILD
                        | WS_BORDER
                        | ES_LEFT
                        | ES_MULTILINE
                        | ES_AUTOVSCROLL,
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
                log::error!("[rust_widgets][windows] create_line_edit failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::LineEdit, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            unsafe {
                self.bind_control_command(widget_id, hwnd);
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if let Some(widget_id) = try_create_slider(self, parent, x, y, width, height) {
            return widget_id;
        }
        log::error!("[rust_widgets][windows] create_slider failed (parent={parent})");
        0
    }
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if let Some(widget_id) = try_create_progress_bar(self, parent, x, y, width, height) {
            return widget_id;
        }
        log::error!("[rust_widgets][windows] create_progress_bar failed (parent={parent})");
        0
    }
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if let Some(widget_id) = try_create_combo_box(self, parent, x, y, width, height) {
            return widget_id;
        }
        log::error!("[rust_widgets][windows] create_combo_box failed (parent={parent})");
        0
    }
    fn combo_box_add_item(&self, combo_box: ObjectId, text: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ADDSTRING, CB_ERR};
            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let text_wide = Self::to_wide(text);
            let result =
                unsafe { SendMessageW(hwnd, CB_ADDSTRING, 0, text_wide.as_ptr() as isize) };
            result != CB_ERR
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (combo_box, text);
            false
        }
    }
    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_RESETCONTENT};
            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            unsafe {
                SendMessageW(hwnd, CB_RESETCONTENT, 0, 0);
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = combo_box;
            false
        }
    }
    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETCURSEL, CB_SETCURSEL};
            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            // CB_GETCURSEL returns -1 (CB_ERR) if nothing is selected, or the current 0-based index.
            let previous = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
            // CB_SETCURSEL returns the PREVIOUS selection index on success, or CB_ERR (-1) on failure.
            let result = unsafe { SendMessageW(hwnd, CB_SETCURSEL, index, 0) };
            if result == CB_ERR {
                return false;
            }
            // Fire trigger only when the selection actually changes: previous index != new index.
            if previous != index as isize {
                let _ = self
                    .inject_widget_trigger_event(combo_box, WidgetTriggerKind::SelectionChanged);
                let _ =
                    self.inject_widget_trigger_event(combo_box, WidgetTriggerKind::ValueChanged);
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (combo_box, index);
            false
        }
    }
    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETCURSEL};
            let hwnd = self.get_native_handle(combo_box)?;
            let result = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
            if result == CB_ERR {
                None
            } else {
                Some(result as usize)
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = combo_box;
            None
        }
    }
    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETCOUNT};
            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let result = unsafe { SendMessageW(hwnd, CB_GETCOUNT, 0, 0) };
            if result == CB_ERR || result < 0 {
                0
            } else {
                result as usize
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = combo_box;
            0
        }
    }
    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETLBTEXT, CB_GETLBTEXTLEN};
            let hwnd = self.get_native_handle(combo_box)?;
            let len = unsafe { SendMessageW(hwnd, CB_GETLBTEXTLEN, index, 0) };
            if len == CB_ERR || len < 0 {
                return None;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let copied =
                unsafe { SendMessageW(hwnd, CB_GETLBTEXT, index, buf.as_mut_ptr() as isize) };
            if copied == CB_ERR || copied < 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..copied as usize]))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (combo_box, index);
            None
        }
    }
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, LBS_NOTIFY, WS_BORDER, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
                WS_VSCROLL,
            };
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("ListBox");
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    null_mut(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | WS_BORDER | WS_VSCROLL | LBS_NOTIFY,
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
                log::error!("[rust_widgets][windows] create_list_box failed (parent={parent})");
                return 0;
            }
            let widget_id = self.state.create_widget(
                WindowsHandleKind::ListBox,
                "ListBox",
                x,
                y,
                width,
                height,
            );
            self.bind_native_handle(widget_id, hwnd);
            unsafe {
                self.bind_control_command(widget_id, hwnd);
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ADDSTRING, LB_ERR};
            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let text_wide = Self::to_wide(text);
            let result =
                unsafe { SendMessageW(hwnd, LB_ADDSTRING, 0, text_wide.as_ptr() as isize) };
            result != LB_ERR
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, text);
            false
        }
    }
    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_DELETESTRING, LB_ERR};
            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let result = unsafe { SendMessageW(hwnd, LB_DELETESTRING, index, 0) };
            result != LB_ERR
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, index);
            false
        }
    }
    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_RESETCONTENT};
            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            unsafe {
                SendMessageW(hwnd, LB_RESETCONTENT, 0, 0);
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = list_box;
            false
        }
    }
    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_SETCURSEL};
            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let result = unsafe { SendMessageW(hwnd, LB_SETCURSEL, index, 0) };
            if result == LB_ERR {
                return false;
            }
            let _ = self.inject_widget_trigger_event(list_box, WidgetTriggerKind::SelectionChanged);
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, index);
            false
        }
    }
    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_GETCURSEL};
            let hwnd = self.get_native_handle(list_box)?;
            let result = unsafe { SendMessageW(hwnd, LB_GETCURSEL, 0, 0) };
            if result == LB_ERR {
                None
            } else {
                Some(result as usize)
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = list_box;
            None
        }
    }
    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_GETCOUNT};
            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let result = unsafe { SendMessageW(hwnd, LB_GETCOUNT, 0, 0) };
            if result == LB_ERR || result < 0 {
                0
            } else {
                result as usize
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = list_box;
            0
        }
    }
    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_GETTEXT, LB_GETTEXTLEN};
            let hwnd = self.get_native_handle(list_box)?;
            let len = unsafe { SendMessageW(hwnd, LB_GETTEXTLEN, index, 0) };
            if len == LB_ERR || len < 0 {
                return None;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let copied =
                unsafe { SendMessageW(hwnd, LB_GETTEXT, index, buf.as_mut_ptr() as isize) };
            if copied == LB_ERR || copied < 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..copied as usize]))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, index);
            None
        }
    }
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{CreateWindowExW, WS_BORDER, WS_CHILD, WS_VISIBLE};
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Static");
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    null_mut(),
                    WS_VISIBLE | WS_CHILD | WS_BORDER,
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
                log::error!("[rust_widgets][windows] create_panel failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::Panel, "Panel", x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::CreateMenu;
            let _ = (parent, x, y, width, height);
            let menu_bar_handle = unsafe { CreateMenu() };
            if menu_bar_handle.is_null() {
                log::error!("[rust_widgets][windows] create_menu_bar failed (parent={parent})");
                return 0;
            }
            let widget_id = self.state.create_widget(
                WindowsHandleKind::MenuBar,
                "MenuBar",
                x,
                y,
                width,
                height,
            );
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.insert(widget_id, menu_bar_handle as usize);
            }
            if let Ok(mut owners) = self.menu_state.menu_owner_window.lock() {
                owners.insert(widget_id, parent);
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{AppendMenuW, CreatePopupMenu, DrawMenuBar, MF_POPUP};
            let parent_menu = {
                let handles = match self.menu_state.handles.lock() {
                    Ok(handles) => handles,
                    Err(e) => {
                        log::error!(
                            "[rust_widgets][windows] create_menu: handles mutex poisoned: {:?}",
                            e
                        );
                        return 0;
                    }
                };
                match handles.get(&parent).copied() {
                    Some(value) => value,
                    None => {
                        log::error!(
                            "[rust_widgets][windows] create_menu failed: invalid parent={parent}"
                        );
                        return 0;
                    }
                }
            };
            let submenu_handle = unsafe { CreatePopupMenu() };
            if submenu_handle.is_null() {
                log::error!("[rust_widgets][windows] create_menu failed (parent={parent})");
                return 0;
            }
            let text_wide = Self::to_wide(text);
            let append_ok = unsafe {
                AppendMenuW(
                    parent_menu as HMENU,
                    MF_POPUP,
                    submenu_handle as usize,
                    text_wide.as_ptr(),
                )
            };
            if append_ok == 0 {
                log::error!(
                    "[rust_widgets][windows] create_menu failed: AppendMenuW failed (parent={parent})"
                );
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::Menu, text, x, y, width, height);
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.insert(widget_id, submenu_handle as usize);
            }
            let owner_window = match self.menu_state.menu_owner_window.lock() {
                Ok(owners) => owners.get(&parent).copied(),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] create_menu: menu_owner_window mutex poisoned"
                    );
                    None
                }
            };
            if let Some(window_id) = owner_window {
                if let Ok(mut owners) = self.menu_state.menu_owner_window.lock() {
                    owners.insert(widget_id, window_id);
                }
                if let Some(hwnd) = self.get_native_handle(window_id) {
                    unsafe {
                        DrawMenuBar(hwnd);
                    }
                }
            }
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{DrawMenuBar, SetMenu};
            let hwnd = match self.get_native_handle(window) {
                Some(hwnd) => hwnd,
                None => {
                    log::error!(
                        "[rust_widgets][windows] attach_menu_bar_to_window failed: invalid window={window}"
                    );
                    return false;
                }
            };
            let menu_handle = {
                let handles = match self.menu_state.handles.lock() {
                    Ok(handles) => handles,
                    Err(e) => {
                        log::error!("[rust_widgets][windows] attach_menu_bar_to_window: handles mutex poisoned: {:?}", e);
                        return false;
                    }
                };
                match handles.get(&menu_bar).copied() {
                    Some(value) => value,
                    None => {
                        log::error!(
                            "[rust_widgets][windows] attach_menu_bar_to_window failed: invalid menu_bar={menu_bar}"
                        );
                        return false;
                    }
                }
            };
            let set_ok = unsafe { SetMenu(hwnd, menu_handle as HMENU) };
            if set_ok == 0 {
                log::error!(
                    "[rust_widgets][windows] attach_menu_bar_to_window failed: SetMenu returned 0"
                );
                return false;
            }
            unsafe {
                DrawMenuBar(hwnd);
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (window, menu_bar);
            false
        }
    }
    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{AppendMenuW, DrawMenuBar, MF_STRING};
            let parent_handle = {
                let handles = match self.menu_state.handles.lock() {
                    Ok(handles) => handles,
                    Err(e) => {
                        log::error!(
                            "[rust_widgets][windows] menu_add_item: handles mutex poisoned: {:?}",
                            e
                        );
                        return 0;
                    }
                };
                match handles.get(&parent_menu).copied() {
                    Some(value) => value,
                    None => {
                        log::error!(
                            "[rust_widgets][windows] menu_add_item failed: invalid parent_menu={parent_menu}"
                        );
                        return 0;
                    }
                }
            };
            let command_id = self.menu_state.next_command_id.fetch_add(1, Ordering::SeqCst) as u32;
            // Win32 shows a shortcut by convention as "Label\tAccelerator" in the
            // menu text; the accelerator itself is registered separately below.
            let label = match shortcut.map(str::trim).filter(|s| !s.is_empty()) {
                Some(chord) => format!("{text}\t{chord}"),
                None => text.to_string(),
            };
            let text_wide = Self::to_wide(&label);
            let append_ok = unsafe {
                AppendMenuW(
                    parent_handle as HMENU,
                    MF_STRING,
                    command_id as usize,
                    text_wide.as_ptr(),
                )
            };
            if append_ok == 0 {
                log::error!(
                    "[rust_widgets][windows] menu_add_item failed: AppendMenuW failed (parent_menu={parent_menu})"
                );
                return 0;
            }
            let item_id = self.state.create_widget(WindowsHandleKind::Menu, text, 0, 0, 0, 0);
            if let Ok(mut map) = self.menu_state.menu_command_to_item.lock() {
                map.insert(command_id, item_id);
            }
            let owner_window = match self.menu_state.menu_owner_window.lock() {
                Ok(owners) => owners.get(&parent_menu).copied(),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] menu_add_item: menu_owner_window mutex poisoned"
                    );
                    None
                }
            };
            // Register a real accelerator so the chord is translated by the
            // message loop into a WM_COMMAND for this item. Without this the
            // shortcut would only be printed in the label.
            if let (Some(chord), Some(window_id)) = (shortcut, owner_window) {
                match crate::platform::windows::accel::parse_accelerator(Some(chord)) {
                    Some(accel) => {
                        crate::platform::windows::accel::install_accelerator(
                            accel, command_id, window_id,
                        );
                        crate::platform::windows::accel::record_shortcut_text(command_id, chord);
                    }
                    None => {
                        // Not fatal: the item still works when clicked. It is
                        // logged so an unusable chord is not silently accepted.
                        log::warn!(
                            "[rust_widgets][windows] menu_add_item: shortcut {chord:?} could not \
                             be translated into a Win32 accelerator; the item will have no chord"
                        );
                    }
                }
            }
            if let Some(window_id) = owner_window {
                if let Ok(mut owners) = self.menu_state.menu_owner_window.lock() {
                    owners.insert(item_id, window_id);
                }
                if let Some(hwnd) = self.get_native_handle(window_id) {
                    unsafe {
                        DrawMenuBar(hwnd);
                    }
                }
            }
            item_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent_menu, text, shortcut);
            0
        }
    }
    /// Returns the accelerator text registered for a Win32 menu item.
    fn menu_item_shortcut(&self, menu_item: ObjectId) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            let command_id = {
                let map = self.menu_state.menu_command_to_item.lock().ok()?;
                map.iter().find(|(_, item)| **item == menu_item).map(|(id, _)| *id)?
            };
            crate::platform::windows::accel::shortcut_text_for(command_id)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = menu_item;
            None
        }
    }
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        #[cfg(target_os = "windows")]
        {
            match self.menu_state.pending_menu_events.lock() {
                Ok(mut events) => events.pop_front().map(|event| event.widget_id),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] poll_menu_triggered: pending_menu_events mutex poisoned"
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
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut events) = self.menu_state.pending_menu_events.lock() {
                events.push_back(WidgetTriggerEvent {
                    widget_id: menu_item_id,
                    kind: WidgetTriggerKind::Clicked,
                });
                return true;
            }
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = menu_item_id;
            false
        }
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut events) = self.menu_state.pending_widget_events.lock() {
                if let Some(event) = events.pop_front() {
                    return Some(event);
                }
            }
            if let Ok(mut events) = self.menu_state.pending_menu_events.lock() {
                if let Some(event) = events.pop_front() {
                    return Some(event);
                }
            }
            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut events) = self.menu_state.pending_widget_events.lock() {
                events.push_back(WidgetTriggerEvent { widget_id, kind });
                return true;
            }
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (widget_id, kind);
            false
        }
    }
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{CreateWindowExW, WS_CHILD, WS_VISIBLE};
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Static");
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    null_mut(),
                    WS_VISIBLE | WS_CHILD,
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
                log::error!("[rust_widgets][windows] create_tool_bar failed (parent={parent})");
                return 0;
            }
            let widget_id = self.state.create_widget(
                WindowsHandleKind::ToolBar,
                "ToolBar",
                x,
                y,
                width,
                height,
            );
            self.bind_native_handle(widget_id, hwnd);
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, SS_LEFT, SS_NOPREFIX, WS_CHILD, WS_VISIBLE,
            };
            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Static");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_VISIBLE | WS_CHILD | SS_LEFT | SS_NOPREFIX,
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
                log::error!("[rust_widgets][windows] create_status_bar failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state.create_widget(WindowsHandleKind::StatusBar, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            widget_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    // ...implement other required methods as stubs...
    fn create_message_box(
        &self,
        parent: ObjectId,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            // Non-blocking creation: register a state handle + native-dialog
            // metadata; the modal MessageBoxW is presented from show_widget.
            let id =
                self.state.create_widget(WindowsHandleKind::MessageBox, text, x, y, width, height);
            if let Ok(mut data) = self.dialog_data.lock() {
                data.insert(
                    id,
                    Win32DialogData {
                        parent_hwnd: self
                            .get_native_handle(parent)
                            .map(|h| h as usize)
                            .unwrap_or(0),
                        title: title.to_string(),
                    },
                );
            }
            id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, title, text, x, y, width, height);
            0
        }
    }
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            // Non-blocking creation; the modal GetOpenFileNameW is presented
            // from show_widget and the selected path is stored into state text.
            let id =
                self.state.create_widget(WindowsHandleKind::FileDialog, "", x, y, width, height);
            if let Ok(mut data) = self.dialog_data.lock() {
                data.insert(
                    id,
                    Win32DialogData {
                        parent_hwnd: self
                            .get_native_handle(parent)
                            .map(|h| h as usize)
                            .unwrap_or(0),
                        title: "Open File".to_string(),
                    },
                );
            }
            id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            // Non-blocking creation; the modal ChooseColorW is presented from
            // show_widget and the selected #RRGGBB is stored into state text.
            let id =
                self.state.create_widget(WindowsHandleKind::ColorDialog, "", x, y, width, height);
            if let Ok(mut data) = self.dialog_data.lock() {
                data.insert(
                    id,
                    Win32DialogData {
                        parent_hwnd: self
                            .get_native_handle(parent)
                            .map(|h| h as usize)
                            .unwrap_or(0),
                        title: "Choose Color".to_string(),
                    },
                );
            }
            id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            // Non-blocking creation; the modal ChooseFontW is presented from
            // show_widget and the selected family name is stored into state text.
            let id =
                self.state.create_widget(WindowsHandleKind::FontDialog, "", x, y, width, height);
            if let Ok(mut data) = self.dialog_data.lock() {
                data.insert(
                    id,
                    Win32DialogData {
                        parent_hwnd: self
                            .get_native_handle(parent)
                            .map(|h| h as usize)
                            .unwrap_or(0),
                        title: "Choose Font".to_string(),
                    },
                );
            }
            id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_spin_box(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_spin_box failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_list_view(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_list_view failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_scroll_area(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_scroll_area failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_group_box(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_group_box(
                self, parent, title, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_group_box failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, title, x, y, width, height);
            0
        }
    }
    fn create_frame(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_frame(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_frame failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_tab_widget(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_tab_widget failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_splitter(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_splitter failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_toggle_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_toggle_button(
                self, parent, text, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_toggle_button failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }

    fn create_calendar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_calendar(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_calendar failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_scroll_bar(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_scroll_bar failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_double_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_double_spin_box(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_double_spin_box failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }

    fn create_font_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_font_combo_box(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_font_combo_box failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_context_menu(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(WindowsHandleKind::ContextMenu, "ContextMenu", x, y, width, height)
    }
    fn create_popup_window(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(WindowsHandleKind::PopupWindow, title, x, y, width, height)
    }
    fn create_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(WindowsHandleKind::Dialog, title, x, y, width, height)
    }
    fn create_input_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(WindowsHandleKind::InputDialog, "Input", x, y, width, height)
    }
    fn create_progress_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(WindowsHandleKind::ProgressDialog, "Progress", x, y, width, height)
    }
    fn create_directory_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            // Non-blocking creation; the modal IFileDialog (FOS_PICKFOLDERS) is
            // presented from show_widget and the chosen directory path is stored
            // into state text.
            let id = self.state.create_widget(
                WindowsHandleKind::DirectoryDialog,
                title,
                x,
                y,
                width,
                height,
            );
            if let Ok(mut data) = self.dialog_data.lock() {
                data.insert(
                    id,
                    Win32DialogData {
                        parent_hwnd: self
                            .get_native_handle(parent)
                            .map(|h| h as usize)
                            .unwrap_or(0),
                        title: if title.is_empty() {
                            "Select Folder".to_string()
                        } else {
                            title.to_string()
                        },
                    },
                );
            }
            id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, title, x, y, width, height);
            0
        }
    }
    fn create_date_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_date_picker(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_date_picker failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_time_picker(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_time_picker failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_date_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) = crate::platform::windows::helpers::try_create_date_time_picker(
                self, parent, x, y, width, height,
            ) {
                return widget_id;
            }
            log::error!("[rust_widgets][windows] create_date_time_picker failed (parent={parent})");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_activity_indicator(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if self.state.kind_of(parent).is_none() {
                return 0;
            }
            if let Some(widget_id) =
                crate::platform::windows::helpers::try_create_activity_indicator(
                    self, parent, x, y, width, height,
                )
            {
                return widget_id;
            }
            log::error!(
                "[rust_widgets][windows] create_activity_indicator failed (parent={parent})"
            );
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
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

#[cfg(target_os = "windows")]
impl WindowsPlatform {
    /// Whether this Win32 handle kind carries a primary numeric value.
    ///
    /// This is the backend's own answer about the controls it creates — each
    /// value-carrying kind maps to a different common control (trackbar, progress
    /// bar, up-down) and therefore a different message. It gates
    /// `PBM_SETPOS` / `TBM_SETPOS` / `UDM_SETPOS32` before they are sent and keeps
    /// `widget_value` from inventing a value for a control that has none.
    fn kind_accepts_numeric_value(kind: super::types::WindowsHandleKind) -> bool {
        use super::types::WindowsHandleKind as K;
        matches!(
            kind,
            K::Slider
                | K::ProgressBar
                | K::SpinBox
                | K::DoubleSpinBox
                | K::ScrollBar
                | K::ActivityIndicator
                | K::ProgressDialog
        )
    }

    /// Offers a message to the accelerator table of the currently active window.
    ///
    /// Returns `true` when an accelerator matched, in which case the message has
    /// been consumed and Win32 has posted the item's `WM_COMMAND` instead.
    ///
    /// This is an inherent method rather than a `Platform` trait method: it is
    /// called only from [`Platform::run`]'s message pump, and putting it on the
    /// trait would invite other backends to implement a concept that does not
    /// exist on them.
    ///
    /// Only the active window is consulted. Accelerators belong to the focused
    /// window, so a background window must not swallow a chord typed into the
    /// foreground one.
    pub(crate) fn try_translate_accelerator(&self, msg: &winapi::um::winuser::MSG) -> bool {
        use winapi::um::winuser::{GetActiveWindow, TranslateAcceleratorW};
        // SAFETY: GetActiveWindow is a side-effect-free query; it returns a live
        // HWND for this thread or null.
        let active = unsafe { GetActiveWindow() };
        if active.is_null() {
            return false;
        }
        let Some(window_id) = self.widget_id_by_native_handle(active) else {
            return false;
        };
        let Some(table) = crate::platform::windows::accel::accel_table_for(window_id) else {
            return false;
        };
        // SAFETY: `active` is a live window for this process, `table` is an
        // HACCEL created by `accel::install_accelerator`, and `msg` is the message
        // being dispatched. TranslateAcceleratorW only reads it and posts
        // WM_COMMAND, so `msg` does not need to outlive the call.
        let translated = unsafe { TranslateAcceleratorW(active, table, msg as *const _ as *mut _) };
        translated != 0
    }
}
