// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native surface for **self-drawn** widgets on Windows.
//!
//! Registers a dedicated child window class (`RustWidgetsCanvasClass`) whose
//! window procedure paints the mounted widget and forwards input to it. A
//! distinct class is used rather than extending `RustWidgetsWindowClass`, so the
//! canvas message path stays independent of menu/control command routing.
//!
//! See `docs/plans/custom-paint_mounting.md`.
//!
//! # Feature gate
//!
//! This module renders through `crate::widget::runtime`, which `mini`/`embedded`
//! do not compile (see `src/widget/mod.rs`). The gate is applied here as well as
//! on the `target_os` so those profiles build without a widget registry, and
//! `supports_surfaces()` reports `false` instead of promising a surface that
//! cannot be painted.

#![cfg(all(target_os = "windows", widgets_unstripped))]

use crate::core::{ObjectId, Point, Rect};
use crate::event::Event;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HDC, HWND, RECT};
use winapi::um::wingdi::{
    StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use winapi::um::winuser::{
    BeginPaint, CreateWindowExW, DefWindowProcW, EndPaint, GetClientRect, InvalidateRect,
    LoadCursorW, RegisterClassW, SetWindowPos, UpdateWindow, CS_HREDRAW, CS_OWNDC, CS_VREDRAW,
    IDC_ARROW, PAINTSTRUCT, SWP_NOACTIVATE, SWP_NOZORDER, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_SIZE, WNDCLASSW, WS_CHILD, WS_TABSTOP,
    WS_VISIBLE,
};

/// Child-window class name used for every self-drawn canvas.
const CANVAS_CLASS: &str = "RustWidgetsCanvasClass";

/// Maps a canvas `HWND` to the widget registry id it paints.
///
/// Only the id is stored: geometry lives in `widget::runtime` (the widget owns
/// it) and the `HWND` itself is also kept on the window as `GWLP_USERDATA`, so a
/// message handler can recover the id without a map lookup if needed.
fn canvases() -> &'static Mutex<HashMap<usize, ObjectId>> {
    static CANVASES: OnceLock<Mutex<HashMap<usize, ObjectId>>> = OnceLock::new();
    CANVASES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Encodes a Rust string as a NUL-terminated UTF-16 buffer for Win32 APIs.
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" {
    /// Returns the module handle of the calling process when passed null.
    fn GetModuleHandleW(lp_module_name: *const u16) -> *mut std::ffi::c_void;
}

/// Registers `RustWidgetsCanvasClass`. Safe to call repeatedly.
pub(crate) fn ensure_canvas_class_registered() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        // SAFETY: RegisterClassW runs on the UI thread with a fully initialised
        // WNDCLASSW whose pointers outlive the call.
        unsafe {
            let class_name = to_wide(CANVAS_CLASS);
            let hinstance = GetModuleHandleW(std::ptr::null());
            let mut wnd_class: WNDCLASSW = std::mem::zeroed();
            // CS_OWNDC: the canvas owns its device context, so painting does not
            // contend with sibling controls for a shared DC.
            wnd_class.style = CS_HREDRAW | CS_VREDRAW | CS_OWNDC;
            wnd_class.lpfnWndProc = Some(canvas_wnd_proc);
            wnd_class.hInstance = hinstance as _;
            wnd_class.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
            wnd_class.lpszClassName = class_name.as_ptr();
            if RegisterClassW(&wnd_class) == 0 {
                log::error!("[windows] RegisterClassW failed for {CANVAS_CLASS}");
            }
        }
    });
}

/// Window procedure for canvas child windows.
unsafe extern "system" fn canvas_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_canvas(hwnd);
            0
        }
        // Painting covers the whole client area, so let it erase too. Returning
        // non-zero skips the background fill and avoids a one-frame white flash.
        WM_ERASEBKGND => 1,
        WM_SIZE => {
            invalidate_canvas(hwnd);
            0
        }
        WM_LBUTTONDOWN => {
            forward_mouse(hwnd, lparam, MousePhase::Press);
            0
        }
        WM_LBUTTONUP => {
            forward_mouse(hwnd, lparam, MousePhase::Release);
            0
        }
        WM_MOUSEMOVE => {
            forward_mouse(hwnd, lparam, MousePhase::Drag);
            0
        }
        WM_KEYDOWN => {
            forward_key(hwnd, wparam);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Which mouse event to synthesise.
#[derive(Clone, Copy)]
enum MousePhase {
    Press,
    Release,
    Drag,
}

/// Returns the widget id painted by `hwnd`.
fn widget_id_of(hwnd: HWND) -> Option<ObjectId> {
    canvases().lock().ok()?.get(&(hwnd as usize)).copied()
}

/// Renders the mounted widget into the window's client area.
unsafe fn paint_canvas(hwnd: HWND) {
    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let hdc: HDC = BeginPaint(hwnd, &mut paint);

    let mut rect: RECT = std::mem::zeroed();
    if GetClientRect(hwnd, &mut rect) == 0 {
        log::error!("[windows] canvas: GetClientRect failed");
        EndPaint(hwnd, &paint);
        return;
    }
    let width = (rect.right - rect.left).max(1) as u32;
    let height = (rect.bottom - rect.top).max(1) as u32;

    let Some(widget_id) = widget_id_of(hwnd) else {
        log::error!("[windows] canvas: WM_PAINT for an hwnd with no widget id");
        EndPaint(hwnd, &paint);
        return;
    };
    let Some(frame) = crate::widget::runtime::render_frame(
        widget_id,
        crate::core::Size::new(width, height),
        crate::core::Color::WHITE,
    ) else {
        log::error!(
            "[windows] canvas: widget id={widget_id} produced no frame \
             (unmounted, or it does not implement Draw)"
        );
        EndPaint(hwnd, &paint);
        return;
    };

    // 32-bit top-down BGRA. The frame is RGBA, so red and blue are swapped while
    // copying; alpha is forced opaque because StretchDIBits with BI_RGB ignores it.
    let mut buffer = vec![0u8; width as usize * height as usize * 4];
    for (index, pixel) in frame.chunks_exact(4).enumerate() {
        let offset = index * 4;
        buffer[offset] = pixel[2];
        buffer[offset + 1] = pixel[1];
        buffer[offset + 2] = pixel[0];
        buffer[offset + 3] = 255;
    }

    let mut info: BITMAPINFOHEADER = std::mem::zeroed();
    info.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.biWidth = width as i32;
    // A negative height selects a top-down DIB, matching the frame's row order.
    info.biHeight = -(height as i32);
    info.biPlanes = 1;
    info.biBitCount = 32;
    info.biCompression = BI_RGB;

    let scanned = StretchDIBits(
        hdc,
        0,
        0,
        width as i32,
        height as i32,
        0,
        0,
        width as i32,
        height as i32,
        buffer.as_ptr() as *const _,
        &info as *const _ as *const BITMAPINFO,
        DIB_RGB_COLORS,
        SRCCOPY,
    );
    if scanned == 0 {
        log::error!("[windows] canvas: StretchDIBits failed for {width}x{height}");
    }

    EndPaint(hwnd, &paint);
}

/// Marks the whole canvas as needing a repaint.
pub(crate) fn invalidate_canvas(hwnd: HWND) {
    // SAFETY: a null RECT invalidates the entire client area, which is intended.
    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

/// Translates a Win32 mouse message into a widget event and delivers it.
unsafe fn forward_mouse(hwnd: HWND, lparam: LPARAM, phase: MousePhase) {
    let Some(widget_id) = widget_id_of(hwnd) else {
        return;
    };
    // The low/high words of lparam hold signed client-area coordinates.
    let x = (lparam & 0xFFFF) as u16 as i16 as i32;
    let y = ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32;
    let position = Point::new(x, y);
    let event = match phase {
        MousePhase::Press => Event::MousePress { pos: position, button: 1 },
        MousePhase::Release => Event::MouseRelease { pos: position, button: 1 },
        MousePhase::Drag => Event::MouseMove { pos: position },
    };
    if crate::widget::runtime::dispatch_event(widget_id, &event) {
        invalidate_canvas(hwnd);
    }
}

/// Translates a Win32 key message into a widget event and delivers it.
unsafe fn forward_key(hwnd: HWND, wparam: WPARAM) {
    let Some(widget_id) = widget_id_of(hwnd) else {
        return;
    };
    let key = wparam as u32;
    let modifiers = current_modifiers();
    let event = Event::KeyPress { key, modifiers };
    if crate::widget::runtime::dispatch_event(widget_id, &event) {
        invalidate_canvas(hwnd);
    }
}

/// Reads the keyboard modifier state into the widget-layer bitfield.
/// Returns the modifier bitfield for the key currently being processed.
///
/// The widget-layer convention is shift = 1, control = 2, alt = 4,
/// meta/command = 8 (see `Modifiers::from_event_bits`). Windows has no Command
/// key, so bit 3 stays clear; Control is reported as physical Control, and any
/// `Shortcut::primary` binding is resolved against CTRL by the shortcut layer on
/// this platform.
pub(crate) unsafe fn current_modifiers() -> u32 {
    use winapi::um::winuser::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
    // Widget-layer bits (see `Modifiers::from_event_bits`).
    const WIDGET_SHIFT: u32 = 1;
    const WIDGET_CONTROL: u32 = 2;
    const WIDGET_ALT: u32 = 4;
    let mut modifiers = 0u32;
    // A negative result means the key is currently down.
    if GetKeyState(VK_SHIFT) < 0 {
        modifiers |= WIDGET_SHIFT;
    }
    if GetKeyState(VK_CONTROL) < 0 {
        modifiers |= WIDGET_CONTROL;
    }
    if GetKeyState(VK_MENU) < 0 {
        modifiers |= WIDGET_ALT;
    }
    modifiers
}

/// Creates and shows a canvas child window for `id`.
pub(crate) fn mount_canvas(parent: HWND, id: ObjectId, rect: Rect) -> Option<HWND> {
    ensure_canvas_class_registered();
    if !crate::widget::runtime::is_mounted(id) {
        log::error!(
            "[windows] mount_surface: id={id} is not in widget::runtime; \
             call runtime::register before mounting"
        );
        return None;
    }
    // SAFETY: all Win32 calls run on the UI thread; the class was registered just
    // above and `parent` is a live window handle supplied by the caller.
    unsafe {
        let class_name = to_wide(CANVAS_CLASS);
        let hinstance = GetModuleHandleW(std::ptr::null());
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            to_wide("").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            rect.x,
            rect.y,
            rect.width as i32,
            rect.height as i32,
            parent,
            std::ptr::null_mut(),
            hinstance as _,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            log::error!(
                "[windows] mount_surface: CreateWindowExW failed (GetLastError={})",
                winapi::um::errhandlingapi::GetLastError()
            );
            return None;
        }
        canvases().lock().expect("windows canvas lock poisoned").insert(hwnd as usize, id);
        invalidate_canvas(hwnd);
        UpdateWindow(hwnd);
        Some(hwnd)
    }
}

/// Moves and resizes a canvas child window.
pub(crate) fn resize_canvas(hwnd: HWND, rect: Rect) -> bool {
    // SAFETY: `hwnd` comes from `mount_canvas` and is a live child window.
    unsafe {
        let moved = SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            rect.x,
            rect.y,
            rect.width as i32,
            rect.height as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        if moved == 0 {
            log::error!("[windows] resize_surface: SetWindowPos failed");
            return false;
        }
        invalidate_canvas(hwnd);
        true
    }
}

/// Destroys a canvas child window and forgets its state.
pub(crate) fn unmount_canvas(hwnd: HWND) -> bool {
    // SAFETY: `hwnd` came from `mount_canvas`; DestroyWindow is valid on a child
    // window and synchronously delivers WM_DESTROY.
    unsafe {
        use winapi::um::winuser::DestroyWindow;
        if DestroyWindow(hwnd) == 0 {
            log::error!("[windows] unmount_surface: DestroyWindow failed");
            return false;
        }
    }
    canvases().lock().expect("windows canvas lock poisoned").remove(&(hwnd as usize)).is_some()
}

/// Looks up the canvas window for a mounted widget id.
pub(crate) fn hwnd_for_widget(id: ObjectId) -> Option<HWND> {
    let map = canvases().lock().ok()?;
    map.iter().find(|(_, widget)| **widget == id).map(|(hwnd, _)| *hwnd as HWND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_name_is_stable() {
        // The name is part of the registered Win32 class; changing it would
        // orphan windows created by an older build in the same process.
        assert_eq!(CANVAS_CLASS, "RustWidgetsCanvasClass");
    }

    #[test]
    fn to_wide_is_nul_terminated() {
        assert_eq!(to_wide("ab"), vec![0x61, 0x62, 0x00]);
        assert_eq!(to_wide(""), vec![0x00]);
    }

    #[test]
    fn unknown_hwnd_has_no_widget() {
        assert!(widget_id_of(0 as HWND).is_none());
    }

    #[test]
    fn hwnd_for_unknown_widget_is_none() {
        assert!(hwnd_for_widget(0xDEAD_BEEF).is_none());
    }
}
