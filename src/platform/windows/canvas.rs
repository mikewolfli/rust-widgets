// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native surface for **self-drawn** widgets on Windows.
//!
//! Registers a dedicated child window class (`RustWidgetsCanvasClass`) whose
//! window procedure paints the mounted widget and forwards input to it. A
//! distinct class is used rather than extending `RustWidgetsWindowClass`, so the
//! canvas message path stays independent of menu/control command routing.
//!
//! See `docs/plans/custom_widget_mounting.md`.
//!
//! # Feature gate
//!
//! This module renders through `crate::widget::runtime`, which `mini`/`embedded`
//! do not compile (see `src/widget/mod.rs`). The gate is applied here as well as
//! on the `target_os` so those profiles build without a widget registry, and
//! `supports_surfaces()` reports `false` instead of promising a surface that
//! cannot be painted.

#![cfg(all(target_os = "windows", widgets_unstripped))]

use crate::core::{MutexExt, ObjectId, Point, Rect};
use crate::event::Event;
use crate::platform::types::MousePhase;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HDC, HWND, RECT};
use winapi::um::wingdi::{
    StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use winapi::um::winuser::{
    BeginPaint, CreateWindowExW, DefWindowProcW, EndPaint, GetClientRect, InvalidateRect,
    LoadCursorW, RegisterClassW, SetFocus, SetWindowPos, TrackMouseEvent, UpdateWindow, CS_HREDRAW,
    CS_OWNDC, CS_VREDRAW, IDC_ARROW, PAINTSTRUCT, SWP_NOACTIVATE, SWP_NOZORDER, TME_LEAVE,
    TRACKMOUSEEVENT, WM_CHAR, WM_CLOSE, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSELEAVE, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETFOCUS, WM_SIZE, WM_TOUCH, WM_UNICHAR, WNDCLASSW,
    WS_CHILD, WS_TABSTOP, WS_VISIBLE,
};
// Touch-only Win32 entry points. Grouped under one gate so a build without the
// `touch` capability does not import symbols it never calls (which would be an
// unused-import warning) and does not need `WM_TOUCH` handling at all.
#[cfg(feature = "touch")]
use winapi::um::winuser::{
    CloseTouchInputHandle, GetTouchInputInfo, RegisterTouchWindow, TOUCHEVENTF_DOWN,
    TOUCHEVENTF_MOVE, TOUCHEVENTF_UP, TOUCHINPUT, TWF_WANTPALM,
};

/// Child-window class name used for every self-drawn canvas.
const CANVAS_CLASS: &str = "RustWidgetsCanvasClass";

/// Maps a canvas `HWND` to the widget registry id it paints.
///
/// # Lock policy
///
/// Every accessor below goes through [`MutexExt::lock_guard`], which recovers from a
/// poisoned lock instead of panicking. That is not a stylistic choice: these maps are
/// read and written from `canvas_wnd_proc`, and a panic raised inside an
/// `extern "system"` callback cannot unwind across the FFI boundary — Rust aborts the
/// process. A poisoned map (some other thread panicked while holding it) therefore used
/// to turn a recoverable bookkeeping fault into a hard crash of the whole application.
/// The sibling module `windows::types` already answers a poisoned handle map by logging
/// and returning `None`; this module now follows the same policy, so the backend has one
/// answer to the question rather than two.
///
/// Only the id is stored: geometry lives in `widget::runtime` (the widget owns
/// it) and the `HWND` itself is also kept on the window as `GWLP_USERDATA`, so a
/// message handler can recover the id without a map lookup if needed.
fn canvases() -> &'static Mutex<HashMap<usize, ObjectId>> {
    static CANVASES: OnceLock<Mutex<HashMap<usize, ObjectId>>> = OnceLock::new();
    CANVASES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns the canvas's origin in its parent's client area.
///
/// # Why this is asked of Win32 rather than remembered
///
/// The origin used to be recorded at mount time and updated only by [`resize_canvas`].
/// That is correct for the two paths this module controls, and wrong for every other one: a
/// host that moves the child `HWND` through Win32 directly (`SetWindowPos` on a handle it
/// obtained from `get_native_handle`, a parent layout that repositions its children, a
/// platform that re-places child windows on a DPI change) moves the canvas without telling
/// this module, and the recorded origin goes stale.
///
/// Because widget geometry is **absolute**, a stale origin does not merely shift a
/// highlight — it delivers the press to whichever control happens to occupy the stale
/// point. Measured shape of the failure: with the canvas moved from `(0,0)` to `(200,0)` by
/// an out-of-band path, a control at absolute `x = 220` is reported at `x = 20`, so the press
/// runs a *different* control's `on_click`.
///
/// Asking Win32 removes the possibility: `MapWindowPoints` converts the canvas's own
/// client-area origin into the parent's coordinates, and it is correct after every move
/// however it happened. The macOS canvas already reads its origin live for this exact
/// reason (`macos/canvas.rs::view_origin` — "read live rather than cached because AppKit is
/// free to move the view ... without telling this module"); this is the Windows spelling of
/// the same rule.
///
/// `None` when Win32 cannot answer, which makes the callers drop the input rather than
/// dispatch it at a guessed position — a dropped click is recoverable, a misrouted one runs
/// the wrong handler.
fn canvas_origin(hwnd: HWND) -> Option<(i32, i32)> {
    use winapi::shared::windef::POINT;
    use winapi::um::errhandlingapi::{GetLastError, SetLastError};
    use winapi::um::winuser::{GetParent, MapWindowPoints};
    // SAFETY: `hwnd` is a live child window this module created; `MapWindowPoints` only reads
    // the point we pass and writes the converted value back into the same place.
    unsafe {
        let parent = GetParent(hwnd);
        if parent.is_null() {
            // A canvas always has a parent. Reporting that here beats dispatching input at
            // an origin this function cannot vouch for.
            log::error!("[windows] canvas_origin: hwnd {hwnd:?} has no parent window");
            return None;
        }
        // The canvas's own client origin, in parent-client coordinates.
        let mut origin = POINT { x: 0, y: 0 };
        // `MapWindowPoints` returns the pixel offset in the low/high words of its return
        // value, and `0` on failure. A canvas sitting exactly at its parent's client origin
        // is therefore a **legitimate** zero: the point did not move. The documented way to
        // tell that zero apart from a real failure is to clear the thread's last error
        // before the call and inspect it after (see the MapWindowPoints reference).
        SetLastError(0);
        let result = MapWindowPoints(hwnd, parent, &mut origin, 1);
        if map_failed(result, GetLastError()) {
            log::error!("[windows] canvas_origin: MapWindowPoints failed for hwnd {hwnd:?}");
            return None;
        }
        Some((origin.x, origin.y))
    }
}

/// Interprets `MapWindowPoints`'s return value.
///
/// A zero return is *not* enough to call failure: a zero horizontal and vertical
/// displacement (the canvas is at its parent's client origin) is reported the same way a
/// declined call is. Only "zero **and** a non-zero last error" is a failure.
fn map_failed(result: i32, last_error: u32) -> bool {
    result == 0 && last_error != 0
}

/// Encodes a Rust string as a NUL-terminated UTF-16 buffer for Win32 APIs.
///
/// Delegates to [`WindowsPlatform::to_wide`] rather than keeping a second
/// implementation: this file previously had its own `encode_utf16` copy, so the
/// same contract existed twice and could drift (the shared one uses
/// `OsStrExt::encode_wide`).
fn to_wide(value: &str) -> Vec<u16> {
    crate::platform::windows::types::WindowsPlatform::to_wide(value)
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
        WM_LBUTTONDBLCLK => {
            forward_mouse(hwnd, lparam, MousePhase::DoubleClick);
            0
        }
        WM_RBUTTONDOWN => {
            forward_mouse(hwnd, lparam, MousePhase::SecondaryPress);
            0
        }
        WM_RBUTTONUP => {
            forward_mouse(hwnd, lparam, MousePhase::SecondaryRelease);
            0
        }
        WM_MOUSEMOVE => {
            forward_mouse(hwnd, lparam, MousePhase::Drag);
            0
        }
        // The wheel arrives with a *screen* point and its delta in `wParam`; see
        // `windows::types::forward_window_wheel` for the decoding, which this mirrors so a
        // canvas and a window scroll identically.
        WM_MOUSEWHEEL => {
            forward_wheel(hwnd, wparam, lparam);
            0
        }
        // The produced character. A canvas hosts widgets that can be typed into exactly as a
        // window does, and forwarding only `WM_KEYDOWN`'s virtual-key code is what left a
        // mounted text control unable to accept a digit or any IME text — the same defect the
        // window procedure had.
        WM_CHAR | WM_UNICHAR => {
            forward_char(hwnd, wparam);
            0
        }
        WM_MOUSELEAVE => {
            // Win32 only sends this after the window asks for it, so the request is
            // (re)issued on every move (see `forward_mouse`). This is the one case the
            // coordinate-based hover transition cannot observe: the pointer is outside, so
            // the previously hovered control would otherwise stay highlighted.
            //
            // Routed as an event rather than applied with a global `clear_hover`, matching
            // `windows::types`' window procedure: the two surfaces of one window must agree
            // about what a leave means, and a global clear would also drop the highlight of a
            // control the pointer is still inside on the *other* surface.
            if let Some(widget_id) = widget_id_of(hwnd) {
                let Some(origin) = canvas_origin(hwnd) else {
                    return 0;
                };
                let position = Point::new(origin.0, origin.1);
                crate::platform::platform_facts().route_pointer_event(
                    widget_id,
                    &Event::MouseLeave { pos: position },
                    position,
                );
                invalidate_canvas(hwnd);
            }
            0
        }
        WM_TOUCH => {
            // `touch` is an independently composable capability, so a Windows build
            // without it has no `Event::Touch*` variants to build (they are gated in
            // `crate::event::types`). Fall through to `DefWindowProcW` rather than
            // referencing `forward_touch`, whose whole body would fail to resolve —
            // and note that `RegisterTouchWindow` below is gated the same way, so
            // Win32 never sends this message in that configuration either.
            #[cfg(feature = "touch")]
            {
                forward_touch(hwnd, wparam, lparam);
                0
            }
            #[cfg(not(feature = "touch"))]
            {
                let _ = (wparam, lparam);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_KEYDOWN => {
            forward_key(hwnd, wparam);
            0
        }
        // The key came back up — see `forward_key_release` for why a surface needs this arm as
        // much as a window does.
        WM_KEYUP => {
            forward_key_release(hwnd, wparam);
            0
        }
        // The canvas regained the OS keyboard. The inverse of the `WM_KILLFOCUS` arm below, for
        // the same reason the window procedure now has one: clearing a focus fact without ever
        // restoring it leaves a control that can be typed into looking unfocused and routing keys
        // to the window instead. See `windows::types` for the full reasoning — the two surfaces of
        // one window must agree, which is exactly the asymmetry this arm removes.
        WM_SETFOCUS => {
            if let Some(widget_id) = widget_id_of(hwnd) {
                crate::widget::runtime::report_state(
                    widget_id,
                    crate::widget::runtime::StateFact::Focused(true),
                );
                invalidate_canvas(hwnd);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        // The canvas lost the OS keyboard, so the library must stop believing a control
        // inside it is focused.
        //
        // # Why the canvas needs its own arm
        //
        // The canvas is created with `WS_TABSTOP` and calls `SetFocus` on every press
        // (see `forward_mouse`), so it really can hold the keyboard. Without this arm the
        // window arm's defect reappears one level down: click into a mounted text control,
        // alt-tab to another application, and the control keeps blinking its caret and
        // drawing its focus ring while the keystrokes go elsewhere. The window procedure was
        // fixed without its canvas half, which is exactly the asymmetry `blit_frame`'s own
        // doc warns about.
        WM_KILLFOCUS => {
            if let Some(widget_id) = widget_id_of(hwnd) {
                crate::widget::runtime::report_state(
                    widget_id,
                    crate::widget::runtime::StateFact::Focused(false),
                );
                invalidate_canvas(hwnd);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        // A canvas is a child surface with no title bar and no system menu, so Win32 never
        // synthesizes `WM_CLOSE` for it — but `DefWindowProcW` would honour one arriving from
        // elsewhere and destroy the window.
        //
        // Swallowed rather than forwarded: a canvas is destroyed by
        // [`unmount_canvas`], which also releases the two registry entries that point at
        // this `HWND`. Letting the default handler run would destroy the child behind that
        // function's back and leave both maps holding a dead handle — a later
        // `hwnd_for_widget` would then hand out a dangling `HWND` that Win32 may have already
        // recycled for an unrelated window.
        WM_CLOSE => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Returns the widget id painted by `hwnd`.
fn widget_id_of(hwnd: HWND) -> Option<ObjectId> {
    canvases().lock().ok()?.get(&(hwnd as usize)).copied()
}

/// Renders the mounted widget into the window's client area.
unsafe fn paint_canvas(hwnd: HWND) {
    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let hdc: HDC = BeginPaint(hwnd, &mut paint);

    let Some((width, height)) = client_size(hwnd) else {
        EndPaint(hwnd, &paint);
        return;
    };

    let Some(widget_id) = widget_id_of(hwnd) else {
        log::error!("[windows] canvas: WM_PAINT for an hwnd with no widget id");
        EndPaint(hwnd, &paint);
        return;
    };
    // `render_frame_cached` rather than `render_frame`: it carries the previous frame
    // forward and repaints only the damage, so a widget in `RepaintMode::Dirty` does
    // not re-rasterise every pixel on each `WM_PAINT`. The returned frame is complete
    // — the swap in `blit_frame` still walks it — because `StretchDIBits` presents a
    // whole bitmap; the saving is in what was drawn, not in what is converted.
    let Some(frame) = crate::widget::runtime::render_frame_cached(
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

    blit_frame(hdc, width, height, &frame);
    EndPaint(hwnd, &paint);
}

/// The client area of `hwnd` in pixels, or `None` when Win32 cannot answer.
///
/// Shared by the two painters so neither has to repeat the `GetClientRect` dance —
/// and so a zero-sized window is rejected in one place rather than in two.
pub(crate) unsafe fn client_size(hwnd: HWND) -> Option<(u32, u32)> {
    let mut rect: RECT = std::mem::zeroed();
    if GetClientRect(hwnd, &mut rect) == 0 {
        log::error!("[windows] GetClientRect failed");
        return None;
    }
    Some(((rect.right - rect.left).max(1) as u32, (rect.bottom - rect.top).max(1) as u32))
}

/// Blits a top-down RGBA frame into `hdc`, covering `width` x `height` pixels.
///
/// # Why this is one function rather than two copies
///
/// Two painters now present a frame on Windows: a mounted surface (its own child
/// `HWND`) and a window's whole widget tree. They must agree about channel order
/// and row order, and the only way to guarantee that is for one implementation to
/// be the one they both call — the same reasoning the Linux backend records for its
/// `blit_rgba`. A second copy would be correct on the day it was written and would
/// drift the first time either end was touched.
pub(crate) unsafe fn blit_frame(hdc: HDC, width: u32, height: u32, frame: &[u8]) {
    if width == 0 || height == 0 {
        return;
    }
    let expected = width as usize * height as usize * 4;
    if frame.len() < expected {
        log::error!(
            "[windows] blit_frame: a {width}x{height} frame needs {expected} bytes, got {}",
            frame.len()
        );
        return;
    }

    // 32-bit top-down BGRA. The frame is RGBA, so red and blue are swapped while
    // copying; alpha is forced opaque because StretchDIBits with BI_RGB ignores it.
    let mut buffer = vec![0u8; expected];
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
        log::error!("[windows] blit_frame: StretchDIBits failed for {width}x{height}");
    }
}

/// Marks the whole canvas as needing a repaint.
pub(crate) fn invalidate_canvas(hwnd: HWND) {
    // SAFETY: a null RECT invalidates the entire client area, which is intended.
    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

/// Marks `hwnd` as needing a repaint, whichever kind of window it turns out to be.
///
/// # Why the hook is chosen here and not by the caller
///
/// `Platform::invalidate_surface` is handed an id, and this backend resolves it to a
/// canvas child or a toplevel before it knows which it found. The two are not
/// interchangeable: a canvas paints only its own client area, while a toplevel erases
/// its background (`WM_ERASEBKGND` returns 1) and repaints its whole child list — which
/// is what makes a repaint of the *window* the right response to a change in one of its
/// ordinary children, none of which owns an `HWND` (see `paint_window_tree`).
///
/// `GetParent` is exactly that distinction and is the driver's own answer to it, so the
/// caller does not have to carry a kind alongside the handle. A null parent means
/// toplevel; `InvalidateRect` is the same call either way, but the two doors are kept
/// distinct so a later change to one (a canvas that invalidates only its damage rect,
/// say) cannot silently change the other.
pub(crate) fn invalidate_for_handle(hwnd: HWND) {
    // SAFETY: `hwnd` is a live window handle this backend resolved from its own state;
    // `GetParent` only reads it and a null RECT (below) means "the whole client area".
    unsafe {
        if winapi::um::winuser::GetParent(hwnd).is_null() {
            // A toplevel: its paint procedure redraws the tree, so the whole client area
            // is the right damage.
            InvalidateRect(hwnd, std::ptr::null(), 0);
        } else {
            invalidate_canvas(hwnd);
        }
    }
}

/// Invalidates one rectangle of a canvas's client area.
///
/// # Why this rejects a rectangle outside the client area
///
/// `InvalidateRect` accepts any coordinates and simply unions them into the update
/// region; a rectangle wholly outside the window contributes nothing but is still
/// reported as accepted. Returning `true` for it would tell the caller its damage had
/// been delivered when the region is unchanged — so this checks first and reports
/// `false`, driving the caller's fallback to a whole-client invalidation.
///
/// Returns `false` when the rectangle is empty or lies wholly outside the client area,
/// or when the client rect cannot be queried.
pub(crate) fn invalidate_canvas_rect(hwnd: HWND, rect: Rect) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }

    let mut client: RECT = unsafe { core::mem::zeroed() };
    // SAFETY: `client` is a valid, writable RECT for the duration of the call.
    let have_client = unsafe { GetClientRect(hwnd, &mut client) != 0 };
    if !have_client {
        return false;
    }

    let x = rect.x;
    let y = rect.y;
    let right = rect.x.saturating_add(rect.width as i32);
    let bottom = rect.y.saturating_add(rect.height as i32);
    if x >= client.right || y >= client.bottom || right <= client.left || bottom <= client.top {
        return false;
    }

    let clip = RECT { left: x, top: y, right, bottom };
    // SAFETY: `clip` outlives the call and is a valid RECT; the HWND came from the
    // caller's canvas table, so it addresses a live child window.
    unsafe {
        InvalidateRect(hwnd, &clip, 0);
    }
    true
}

/// Translates a Win32 mouse message into a widget event and delivers it.
///
/// The coordinates Win32 reports are relative to this child window, so they are
/// offset by the canvas origin to reach the absolute space the widget tree uses.
/// Routing then goes through the platform's hit test, so a click on a widget nested
/// inside the mounted one reaches that widget rather than the surface owner.
unsafe fn forward_mouse(hwnd: HWND, lparam: LPARAM, phase: MousePhase) {
    let Some(widget_id) = widget_id_of(hwnd) else {
        return;
    };
    // The low/high words of lparam hold signed client-area coordinates.
    let x = (lparam & 0xFFFF) as u16 as i16 as i32;
    let y = ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32;
    let Some((origin_x, origin_y)) = canvas_origin(hwnd) else {
        return;
    };
    let position = Point::new(origin_x + x, origin_y + y);
    let modifiers = current_modifiers();
    let event = match phase {
        MousePhase::Press => Event::mouse_press_with(
            position.x,
            position.y,
            crate::event::mouse_button::PRIMARY,
            modifiers,
        ),
        MousePhase::Release => {
            Event::MouseRelease { pos: position, button: crate::event::mouse_button::PRIMARY }
        }
        MousePhase::Drag => Event::MouseMove { pos: position },
        MousePhase::SecondaryPress => Event::mouse_press_with(
            position.x,
            position.y,
            crate::event::mouse_button::SECONDARY,
            modifiers,
        ),
        MousePhase::SecondaryRelease => {
            Event::MouseRelease { pos: position, button: crate::event::mouse_button::SECONDARY }
        }
        MousePhase::DoubleClick => {
            Event::mouse_double_click(position.x, position.y, crate::event::mouse_button::PRIMARY)
        }
    };
    let delivered =
        crate::platform::platform_facts().route_pointer_event(widget_id, &event, position);
    if matches!(phase, MousePhase::Drag) {
        // Ask Win32 for a `WM_MOUSELEAVE` on the next exit. The request is consumed
        // by the event it produces, so it must be re-issued on every move — without
        // it the leave message never arrives and hover would stick.
        let mut track: TRACKMOUSEEVENT = std::mem::zeroed();
        track.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
        track.dwFlags = TME_LEAVE;
        track.hwndTrack = hwnd;
        TrackMouseEvent(&mut track);
    }
    if delivered {
        if matches!(phase, MousePhase::Press | MousePhase::SecondaryPress | MousePhase::DoubleClick)
        {
            // A click can move focus to a nested control; give the canvas the
            // keyboard so subsequent keys are delivered here.
            SetFocus(hwnd);
        }
        invalidate_canvas(hwnd);
    }
}

/// Translates a `WM_MOUSEWHEEL` on a canvas into a widget wheel event and delivers it.
///
/// Mirrors `windows::types::forward_window_wheel`, including the two Win32 quirks it
/// records: the delta is in `wParam`'s high word and the point is in *screen* space. The
/// only difference is the extra canvas origin, because widget geometry is absolute while
/// this child window's client area starts at its own corner.
unsafe fn forward_wheel(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    use winapi::um::winuser::{GET_WHEEL_DELTA_WPARAM, WHEEL_DELTA};
    let Some(widget_id) = widget_id_of(hwnd) else {
        return;
    };
    let raw_delta = GET_WHEEL_DELTA_WPARAM(wparam);
    if raw_delta == 0 {
        return;
    }
    let mut screen = winapi::shared::windef::POINT {
        x: (lparam & 0xFFFF) as u16 as i16 as i32,
        y: ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32,
    };
    if winapi::um::winuser::ScreenToClient(hwnd, &mut screen) == 0 {
        return;
    }
    let Some((origin_x, origin_y)) = canvas_origin(hwnd) else {
        return;
    };
    let position = Point::new(origin_x + screen.x, origin_y + screen.y);
    let notches = match raw_delta / WHEEL_DELTA {
        0 if raw_delta > 0 => 1,
        0 => -1,
        whole => whole as i32,
    };
    let event = Event::Wheel { delta: Point::new(0, -notches), modifiers: current_modifiers() };
    if crate::platform::platform_facts().route_pointer_event(widget_id, &event, position) {
        invalidate_canvas(hwnd);
    }
}

/// Translates a `WM_TOUCH` message into widget touch events and delivers them.
///
/// # Why this exists
///
/// Without it the gesture engine never sees a `TouchBegin`: `is_touch()` accepts only
/// `Touch*` and gesture variants, so `GestureEngine::process` was never called with real
/// input and all eleven recognizers were reachable only from unit tests. Win32 reports
/// finger contacts as `WM_TOUCH` carrying *screen* coordinates and its own per-contact
/// ids; both are translated here.
///
/// # Coordinate space
///
/// `TOUCHINPUT` carries coordinates in *hundredths of a pixel* in *screen* space. They
/// are converted to whole pixels relative to this window, then offset by the canvas
/// origin to reach the absolute space the widget tree uses — the same space
/// `forward_mouse` produces, so a touch and a click at the same place agree.
///
/// # Feature gate
///
/// Compiled only with `touch`, because `Event::TouchBegin`/`TouchMove`/`TouchEnd`
/// are themselves gated in `crate::event::types`. The `WM_TOUCH` arm of `wnd_proc`
/// and the `RegisterTouchWindow` opt-in at mount time carry the same gate, so a
/// Windows build without the capability never reaches this function at all.
///
/// # One message, several contacts
///
/// A single `WM_TOUCH` can describe multiple simultaneous fingers — that is how the
/// backend feeds the two independent contacts `Pinch`/`Rotate` need. Each contact is
/// dispatched separately so the recognizers see one event per finger, which is the
/// shape their state machines expect.
#[cfg(feature = "touch")]
unsafe fn forward_touch(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    let input_count = (wparam & 0xFFFF) as u32;
    if input_count == 0 {
        return;
    }
    let mut inputs: Vec<TOUCHINPUT> = vec![std::mem::zeroed(); input_count as usize];
    let size = std::mem::size_of::<TOUCHINPUT>() as i32;
    if GetTouchInputInfo(lparam as *mut _, input_count, inputs.as_mut_ptr(), size) == 0 {
        // The handle must still be closed, or Win32 leaks it — but only on the path
        // where we are not going to inspect the contacts.
        CloseTouchInputHandle(lparam as *mut _);
        return;
    }

    let Some(widget_id) = widget_id_of(hwnd) else {
        CloseTouchInputHandle(lparam as *mut _);
        return;
    };
    let Some((origin_x, origin_y)) = canvas_origin(hwnd) else {
        return;
    };
    let mut delivered = false;

    for input in inputs.iter().take(input_count as usize) {
        // Hundredths of a pixel, screen-relative. `round()` rather than truncation so
        // a contact 1.6 px into a control is reported inside it, matching the pixel a
        // mouse click at the same place would hit.
        let local_x = (input.x as f32 / 100.0).round() as i32;
        let local_y = (input.y as f32 / 100.0).round() as i32;
        // `TOUCHINPUT` is in screen coordinates, so the canvas origin has to be added
        // before the window-relative conversion. `ScreenToClient` needs the point in
        // screen space, which is what we already have.
        let mut screen_point = winapi::shared::windef::POINT { x: local_x, y: local_y };
        if winapi::um::winuser::ScreenToClient(hwnd, &mut screen_point) == 0 {
            continue;
        }
        let position = Point::new(origin_x + screen_point.x, origin_y + screen_point.y);

        // `dwID` is Win32's per-contact identifier; it is stable for the whole
        // contact, which is exactly what `TouchId` must be for the recognizers to
        // track fingers across move/end.
        let touch_id = input.dwID as u64;
        let flags = input.dwFlags;
        let event = if flags & TOUCHEVENTF_DOWN != 0 {
            Event::TouchBegin { pos: position, touch_id }
        } else if flags & TOUCHEVENTF_UP != 0 {
            Event::TouchEnd { pos: position, touch_id }
        } else if flags & TOUCHEVENTF_MOVE != 0 {
            Event::TouchMove { pos: position, touch_id }
        } else {
            // Neither down, up nor move — nothing this layer can express.
            continue;
        };
        if crate::platform::platform_facts().route_pointer_event(widget_id, &event, position) {
            delivered = true;
        }
    }

    // The touch handle is owned by the caller of the window procedure, so it is
    // released exactly once, after every contact has been read from it.
    CloseTouchInputHandle(lparam as *mut _);

    if delivered {
        invalidate_canvas(hwnd);
    }
}

/// Translates a produced character into a widget text-input event and delivers it.
///
/// # Why a canvas needs this at all
///
/// `WM_KEYDOWN` carries a *virtual-key code*: `0x31` for the digit `1`, `0xBE` for `.`,
/// `0xE5` (`VK_PROCESSKEY`) for whatever an IME is composing. Forwarding that code as if
/// it were a character is what made a mounted text control print the wrong glyphs for
/// digits and punctuation and nothing at all for IME text. Win32 translates the key and
/// reposts it as `WM_CHAR`, which is the message text input must consume.
///
/// Control characters are dropped for the reason `windows::types::forward_window_char`
/// records: they are commands (`WM_KEYDOWN` already delivered them) rather than text.
unsafe fn forward_char(hwnd: HWND, wparam: WPARAM) {
    let Some(widget_id) = widget_id_of(hwnd) else {
        return;
    };
    let value = wparam as u32;
    if value < 0x20 || value == 0x7F {
        return;
    }
    let Some(character) = char::from_u32(value) else {
        log::debug!("[windows] canvas: WM_CHAR delivered U+{value:04X}, not a Unicode scalar");
        return;
    };
    // Keys follow focus, and a canvas that never took focus has none: without this the
    // wheel of `focused_widget` would reach a control in another tree.
    let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
    if crate::widget::runtime::dispatch_event(target, &Event::text_input(character.to_string())) {
        invalidate_canvas(hwnd);
    }
}

/// Translates a Win32 key message into a widget event and delivers it.
///
/// Tab is consumed here to move focus, matching the other backends: it is not a
/// printable character, so forwarding it to the widget would be a no-op and the user
/// could never leave the first control.
unsafe fn forward_key(hwnd: HWND, wparam: WPARAM) {
    let Some(widget_id) = widget_id_of(hwnd) else {
        return;
    };
    let key = wparam as u32;
    const VK_TAB: u32 = 0x09;
    const WIDGET_SHIFT: u32 = 1;
    if key == VK_TAB {
        let forward = current_modifiers() & WIDGET_SHIFT == 0;
        crate::widget::runtime::focus_next(forward);
        invalidate_canvas(hwnd);
        return;
    }
    let modifiers = current_modifiers();
    let event = Event::KeyPress { key, modifiers };
    // Keys follow focus: with nothing focused, the surface owner keeps them.
    let target = key_target_for_id(widget_id);
    if crate::widget::runtime::dispatch_event(target, &event) {
        invalidate_canvas(hwnd);
    }
}

/// Forwards a `WM_KEYUP` to the control that received the matching press.
///
/// # Why the canvas needs a release arm too
///
/// The window procedure gained one for the same reason, and a canvas hosts the same kind of
/// widget: a mounted control that tracks a held key must learn the key came up. The target is
/// resolved through `key_target_for_id`, the helper `forward_key` also uses, so a release cannot
/// be delivered to a different control than its press.
///
/// Tab is deliberately not special-cased here: focus traversal happens on the press, and repeating
/// it on the release would move focus twice for one tab.
unsafe fn forward_key_release(hwnd: HWND, wparam: WPARAM) {
    let Some(widget_id) = widget_id_of(hwnd) else {
        return;
    };
    let event = Event::KeyRelease { key: wparam as u32, modifiers: current_modifiers() };
    let target = key_target_for_id(widget_id);
    if crate::widget::runtime::dispatch_event(target, &event) {
        invalidate_canvas(hwnd);
    }
}

/// The widget a key message should be delivered to for surface `widget_id`.
///
/// Shared by [`forward_key`] and [`forward_key_release`] so the two cannot disagree about which
/// control owns the keyboard — the defect two separate copies of
/// `focused_widget().unwrap_or(..)` would eventually produce.
fn key_target_for_id(widget_id: ObjectId) -> ObjectId {
    crate::widget::runtime::focused_widget().unwrap_or(widget_id)
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
    // One canvas per widget. A second mount would create a second child `HWND` recorded
    // against the same id, and every reverse lookup (`hwnd_for_widget`) would then answer
    // with an **arbitrary** one of the two — `HashMap` iteration order — so `resize_surface`
    // and `unmount_surface` could operate on the wrong window and leak the other. Both would
    // also paint the same widget into the shared frame cache at different sizes.
    //
    // Refused rather than silently accepted: the caller asked for something that cannot be
    // honoured unambiguously, and `mount_surface` returning `false` is how this trait reports
    // that (see `windows::canvas`'s note on the honest `false`).
    if hwnd_for_widget(id).is_some() {
        log::error!(
            "[windows] mount_surface: id={id} already has a canvas; a widget can be mounted \
             once, so this request was refused"
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
        canvases().lock_guard().insert(hwnd as usize, id);
        // Opt in to `WM_TOUCH`. Win32 delivers finger contacts only to windows that
        // asked, and the call is what makes the touch path above reachable. A failure
        // is not fatal: a machine with no digitiser simply keeps using mouse input, so
        // it is logged rather than treated as a mount error.
        // `TWF_WANTPALM` suppresses the default press-and-hold "palm check" delay,
        // which would otherwise make a tap take ~1s to be delivered.
        //
        // Gated on `touch` together with `forward_touch`: without the capability there
        // is no touch event to deliver, so asking Win32 for the messages would only
        // produce a `WM_TOUCH` the window procedure has nothing to do with.
        #[cfg(feature = "touch")]
        {
            if RegisterTouchWindow(hwnd, TWF_WANTPALM) == 0 {
                log::debug!(
                    "[windows] mount_surface: RegisterTouchWindow declined for hwnd {hwnd:p}; \
                     touch input will not be delivered to this canvas"
                );
            }
        }
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
///
/// # Why the registry entry is released even when the destroy fails
///
/// `DestroyWindow` failing means the window is still alive, so the entry is *not* stale —
/// but leaving it in place while reporting `false` makes the caller (`unmount_surface`)
/// believe the surface is gone while a later `hwnd_for_widget` still hands out a live
/// `HWND` for a widget the library no longer considers mounted. Those two facts cannot both
/// be true. The entry is therefore always released, and the failure is reported through the
/// return value so the caller can say so.
///
/// The widget registry is the authority on whether a surface exists, and this function only
/// maintains this module's own table, so removing the entry can never hide a mount from the
/// library — only from this backend's lookup, which is exactly what "unmounted" means here.
pub(crate) fn unmount_canvas(hwnd: HWND) -> bool {
    // SAFETY: `hwnd` came from `mount_canvas`; DestroyWindow is valid on a child
    // window and synchronously delivers WM_DESTROY.
    let destroyed = unsafe {
        use winapi::um::winuser::DestroyWindow;
        DestroyWindow(hwnd) != 0
    };
    if !destroyed {
        log::error!(
            "[windows] unmount_surface: DestroyWindow failed for hwnd {hwnd:?}; the window is \
             still alive, but its registry entry was released so this backend can no longer \
             route input or invalidation to it"
        );
    }
    let removed = canvases().lock_guard().remove(&(hwnd as usize)).is_some();
    if !removed {
        // Reached when `unmount_canvas` is called for a handle this module does not track —
        // a double unmount, or a canvas destroyed behind its back. Reported rather than
        // silently answering the caller's question with "yes, removed".
        log::error!("[windows] unmount_surface: hwnd {hwnd:?} was not a mounted canvas");
    }
    destroyed
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

    #[test]
    fn a_zero_displacement_is_not_a_map_failure() {
        // A canvas at the parent's client origin maps its point by (0, 0), which
        // MapWindowPoints reports as a zero return with no error set. That is the
        // success case, not a declined call.
        assert!(!map_failed(0, 0), "zero with no error is a successful no-move");
        assert!(map_failed(0, 5), "zero with an error is a real failure");
        assert!(!map_failed(1, 5), "a non-zero return succeeds regardless of a stale error");
    }
}
