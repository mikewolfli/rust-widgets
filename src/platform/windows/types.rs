// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows platform types, structs, enums, and traits.

use super::notify;
use crate::platform::state::BackendState;
use crate::platform::{Platform, WidgetTriggerEvent};
use winapi::um::winuser::{GetWindowLongPtrW, SetWindowLongPtrW, GWLP_ID, GWLP_USERDATA};

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
        DefWindowProcW, GetClientRect, GetDlgCtrlID, PostQuitMessage, WM_CHAR, WM_CLOSE,
        WM_COMMAND, WM_DESTROY, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_KILLFOCUS,
        WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSELEAVE, WM_MOUSEMOVE, WM_MOUSEWHEEL,
        WM_NOTIFY, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE, WM_UNICHAR,
    };
    match msg {
        // The window's own painting: every control the window owns, drawn as one frame.
        //
        // # Why the top-level window paints anything at all
        //
        // The library paints every `WidgetKind` itself (`control_backend`'s one mechanism),
        // so most controls have **no native control of their own** to draw them. A mounted
        // surface gets a child `HWND` of the canvas class, but the ordinary children —
        // buttons, check boxes, labels — get none, and nothing else painted them: the window
        // showed its chrome over an empty client area, i.e. a blank white board.
        //
        // This arm is the Windows half of the window-level painter the Linux backend already
        // has; both call `render_frame_tree`, which walks the window's child list.
        WM_PAINT => {
            paint_window_tree(hwnd);
            0
        }
        // Painting covers the whole client area, so let it erase too. Returning non-zero
        // skips the background fill and avoids the single white frame a fresh expose would
        // otherwise flash before the tree is drawn (the canvas procedure does the same).
        WM_ERASEBKGND => 1,
        // ── Input ───────────────────────────────────────────────────────────
        //
        // The window's tree painter is also its input surface.
        //
        // # Why the window procedure routes pointer events at all
        //
        // Painting a window's controls means the *window* is where the pointer lands:
        // an ordinary control has no native `HWND` of its own to receive a click, so
        // these messages arrive here and nowhere else. The canvas procedure already
        // does this for a mounted surface; without the same arms on the window, a demo
        // whose controls were created through `create_*` had controls that were visible
        // with correct geometry and callbacks that **never ran** — nothing looked wrong,
        // which is what made it worth its own comment.
        //
        // The coordinates Win32 reports are relative to this window's client area, and
        // controls are positioned in that same space (`frame_origin` is `(0, 0)` for a
        // window), so no origin offset is applied — unlike the canvas, which sits at an
        // offset inside its parent.
        WM_MOUSEMOVE => {
            forward_window_mouse(hwnd, lparam, MousePhase::Drag);
            0
        }
        WM_LBUTTONDOWN => {
            forward_window_mouse(hwnd, lparam, MousePhase::Press);
            0
        }
        WM_LBUTTONUP => {
            forward_window_mouse(hwnd, lparam, MousePhase::Release);
            0
        }
        // A second click is a press with a different multiplicity, and the library's router
        // keys double-click handling off `button`'s second bit (see `event::mouse_button`).
        // Without this arm Win32's own double-click synthesis would be swallowed by
        // `DefWindowProcW` and a `Button` would simply activate twice.
        WM_LBUTTONDBLCLK => {
            forward_window_mouse(hwnd, lparam, MousePhase::DoubleClick);
            0
        }
        // The secondary button is a press/release pair of its own: context menus, and any
        // control that distinguishes a right-click, are unreachable without them.
        WM_RBUTTONDOWN => {
            forward_window_mouse(hwnd, lparam, MousePhase::SecondaryPress);
            0
        }
        WM_RBUTTONUP => {
            forward_window_mouse(hwnd, lparam, MousePhase::SecondaryRelease);
            0
        }
        // The wheel is a *screen*-relative point plus a signed delta in `wParam`'s high word
        // (`GET_WHEEL_DELTA_WPARAM`), not a client point in `lParam`. Both halves are decoded
        // in `forward_window_wheel`, which is also where the `WHEEL_DELTA`-multiples rule is
        // applied — a scroll area needs the direction, not the raw tick count.
        WM_MOUSEWHEEL => {
            forward_window_wheel(hwnd, wparam, lparam);
            0
        }
        // A hover highlight has to be cleared when the pointer leaves, or it sticks.
        WM_MOUSELEAVE => {
            if let Some(window_id) = window_widget_for(hwnd) {
                // The point is only carried into the `MouseLeave` the router delivers, and
                // the pointer has already left the client area, so zero is the honest
                // coordinate rather than a stale one.
                //
                // The leave is **routed**, not applied to the tree globally. An
                // unconditional `clear_hover` would also drop the highlight of a control
                // the pointer is still inside on a *canvas* child — a mixed window has
                // both, and the window's leave fires when the pointer moves from the
                // window into the canvas. So the same event a move would produce is
                // dispatched here, and only the widget that actually loses the pointer
                // clears its hover.
                let leave = crate::event::Event::MouseLeave { pos: crate::core::Point::new(0, 0) };
                crate::widget::runtime::dispatch_pointer_event(
                    window_id,
                    &leave,
                    crate::core::Point::new(0, 0),
                );
                invalidate_window(hwnd);
            }
            0
        }
        // Keys go to whatever the pointer router focused, so typing reaches a field the
        // user clicked rather than always the window. Tab is forwarded too, which is how
        // focus moves between controls.
        //
        // Tab is handled here rather than by the shared dispatch for the reason
        // `windows::canvas::forward_key` records: it is not a printable character, so
        // sending it to a widget is a no-op and the user could never leave the first
        // control. The canvas learned this first and the window did not, so the very
        // keyboard the window gives focus to could not be used to leave it.
        WM_KEYDOWN => {
            if let Some(window_id) = window_widget_for(hwnd) {
                const VK_TAB: u32 = 0x09;
                const WIDGET_SHIFT: u32 = 1;
                if wparam as u32 == VK_TAB {
                    let forward = super::canvas::current_modifiers() & WIDGET_SHIFT == 0;
                    crate::widget::runtime::focus_next(forward);
                    invalidate_window(hwnd);
                    return 0;
                }
                let target = crate::widget::runtime::focused_widget().unwrap_or(window_id);
                let event = crate::event::Event::KeyPress {
                    key: wparam as u32,
                    modifiers: super::canvas::current_modifiers(),
                };
                if crate::widget::runtime::dispatch_event(target, &event) {
                    invalidate_window(hwnd);
                }
            }
            0
        }
        // The **character** the key produced, which `WM_KEYDOWN` cannot supply.
        //
        // # Why forwarding `WM_KEYDOWN` was not enough
        //
        // `wParam` of `WM_KEYDOWN` is a *virtual-key code*: `0x41` for A, but `0x31` for the
        // digit `1`, `0xBE` for `.`, `0xBB` for `+`. A text control that receives those as
        // characters prints a row of unrelated Latin letters for a typed number, and IME,
        // dead keys and any non-Latin layout (`VK_PROCESSKEY` = 0xE5) produce nothing at all.
        //
        // Win32 translates the key into a character and reposts it as `WM_CHAR` (and
        // `WM_UNICHAR` for `WM_UNICHAR`-aware windows), which is the message a text control
        // must consume. `TranslateMessage` in the run loop is what performs the translation,
        // and the canvas procedure had the same gap — see `canvas::forward_char`.
        WM_CHAR | WM_UNICHAR => {
            if let Some(window_id) = window_widget_for(hwnd) {
                forward_window_char(hwnd, window_id, wparam as u32);
            }
            0
        }
        // The window lost the OS keyboard (the user clicked another application).
        //
        // # Why the library must be told
        //
        // The library tracks focus in `widget::runtime` so a caret and a focus ring have one
        // owner. `SetFocus` on the window is what *gives* it that keyboard (see the click arm),
        // and nothing gave it back: after alt-tabbing away, the last control stayed focused and
        // kept drawing its focus ring and blinking its caret while the keystrokes went to
        // another process. Clearing here is the exact inverse of the `SetFocus` above.
        WM_KILLFOCUS => {
            if let Some(window_id) = window_widget_for(hwnd) {
                crate::widget::runtime::report_state(
                    window_id,
                    crate::widget::runtime::StateFact::Focused(false),
                );
                invalidate_window(hwnd);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        // The user resized the window (or the window manager did). Report the new client
        // size so the host can re-run its layout: without this a window resized by the
        // user kept every child at the geometry it had for the previous size, because
        // nothing else tells the library the window changed.
        //
        // # The id must be the *registry* id, and this arm was the one that forgot
        //
        // `widget_id_by_native_handle` answers with the id `bind_native_handle` stamped
        // into `GWLP_USERDATA`, and `create_window` stamps the **platform** id — the one
        // `WindowsPlatformState::create_widget` allocated, starting again at 1. The widget
        // tree the layout walks is keyed by **registry** id. Every other window message
        // performs the second hop through `window_widget_for` (`WM_KILLFOCUS`, the mouse
        // arms, `paint_target_for`); this arm did not, so it handed
        // `crate::queue_resize_trigger` a platform id.
        //
        // The two id spaces are both allocated from small integers, so for a plain
        // single-window application they happen to **coincide** and the mistake is
        // invisible — which is exactly why it survived. As soon as the first platform
        // window is not also the first registry widget, the platform id names either
        // nothing or, worse, an unrelated live widget. `queue_resize_trigger` refuses an
        // id that is not `is_mounted` and returns `false`; that `false` used to be
        // discarded here, so the resize disappeared with no error at any log level — the
        // identical silent no-op the GTK backend documents having fixed in
        // `linux/platform_impl.rs` ("reporting the platform id made every resize a silent
        // no-op").
        //
        // `window_widget_for` is that fix, and the failure is now reported rather than
        // swallowed: a resize that cannot be attributed to a widget is a defect in the
        // host's window set-up, not a routine no-op.
        WM_SIZE => {
            if let Some(widget_id) = unsafe { window_widget_for(hwnd) } {
                let mut rect =
                    winapi::shared::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
                // SAFETY: `hwnd` is the window this procedure was called for, and
                // Win32 fills the RECT we hand it. A failure leaves the zeros, which
                // are rejected below rather than reported as a size.
                if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
                    let width = (rect.right - rect.left).max(0) as u32;
                    let height = (rect.bottom - rect.top).max(0) as u32;
                    if width > 0
                        && height > 0
                        && !crate::queue_resize_trigger(widget_id, width, height)
                    {
                        log::warn!(
                            "[windows] WM_SIZE for widget {widget_id} ({width}x{height}) was not \
                             accepted by the backend; the window layout did not re-run"
                        );
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            let command_id = (wparam & 0xFFFF) as u32;
            let notify_code = ((wparam >> 16) & 0xFFFF) as u32;
            if let Some(platform) = notify::active_windows_platform() {
                if let Ok(map) = platform.menu_state.control_command_to_widget.lock() {
                    if let Some(widget_id) = map.get(&command_id).copied() {
                        if notify::enqueue_control_notify_event(platform, widget_id, notify_code) {
                            return 0;
                        }
                    }
                }
                // A native control whose id was not stamped by `SetWindowLongPtrW` still
                // identifies itself through the `HWND` in `lParam`, which is what
                // `GetDlgCtrlID` reads back. Kept as the fallback for a control the host
                // created itself and handed to this backend.
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
        // `WM_NOTIFY` is the common-controls route: a ListView scrolls, a TreeView expands, a
        // Tab control is asked for its new page. Nothing in this backend posts it, because the
        // library paints every control and no native common control exists — but a host that
        // puts one of its *own* native controls in a window this backend owns still reaches
        // here, and this is the arm that turns it into a library trigger.
        WM_NOTIFY => {
            if let Some(platform) = notify::active_windows_platform() {
                let hdr = lparam as *const NMHDR;
                if !hdr.is_null() {
                    let hwnd_from = unsafe { (*hdr).hwndFrom };
                    let notify_code = unsafe { (*hdr).code };
                    if let Some(widget_id) =
                        unsafe { platform.widget_id_by_native_handle(hwnd_from) }
                    {
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
                if let Some(widget_id) = unsafe { platform.widget_id_by_native_handle(hwnd) } {
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
        // The user asked to close the window, by the title bar's X or the system menu.
        //
        // # Why this is not left to `DefWindowProcW`
        //
        // The default handler calls `DestroyWindow`, which reaches the library only as a
        // `WM_DESTROY` — a message with no widget identity attached. The window therefore
        // closed without its `on_close` callbacks running, while a programmatic
        // `WindowHandle::close` did run them, so the two paths disagreed about what closing
        // a window means. `crate::close_widget` is that one path (`BaseWidget::closed`),
        // documented as "the signal is the one path a close is announced on, so there is no
        // second table to keep in step" — routing the OS close through it is what makes the
        // statement true for both.
        //
        // `DefWindowProcW` still runs afterwards, which destroys the window and reaches
        // `WM_DESTROY` below.
        WM_CLOSE => {
            if let Some(window_id) = window_widget_for(hwnd) {
                if !crate::close_widget(window_id) {
                    // The id exists in the registry (the painter found it) but carries no
                    // widget to close. Reported rather than swallowed: a close that ran no
                    // callback is exactly the silent failure this arm exists to remove.
                    log::error!(
                        "[windows] WM_CLOSE for window widget {window_id}, which is not a \
                         live widget; no close callback was run"
                    );
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

/// Paints a window's whole widget tree into its client area.
///
/// # Why the top-level window paints at all
///
/// Since 2.0 the library paints **every** `WidgetKind` itself, so a control created with
/// `create_button` / `create_checkbox` / … has no native Win32 control behind it. The only
/// Windows painter used to be the canvas child window that `mount_surface` creates, so a
/// window whose children were created through the ordinary `create_*` path — every control
/// in `demo/control` and `demo/finance` — had nothing drawing them and showed a blank
/// client area.
///
/// This is the Windows counterpart of the window-content painter the Linux backend has: a
/// mounted surface (a child `HWND`) still paints itself, and the tree painter here covers
/// everything that has no surface of its own. A window may therefore mix both kinds.
///
/// # The id translation, and why both hops are needed
///
/// Win32 hands this procedure **its own** `HWND`. Resolving what to draw therefore takes
/// two steps, in this order:
///
/// 1. `HWND` → the id the platform knows it by (`widget_id_by_native_handle`);
/// 2. that id → the widget-registry id (`widget_id_for_host_window`), which is the id the
///    tree lives under.
///
/// Skipping a hop would look up an id that addresses no widget and draw nothing — a
/// silently blank window, which is the failure this function exists to remove.
#[cfg(target_os = "windows")]
unsafe fn paint_window_tree(hwnd: HWND) {
    use winapi::um::winuser::{BeginPaint, EndPaint, PAINTSTRUCT};

    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut paint);

    let Some((width, height)) = super::canvas::client_size(hwnd) else {
        EndPaint(hwnd, &paint);
        return;
    };
    if let Some(widget_id) = paint_target_for(hwnd) {
        // `render_frame_tree`, and **not** `render_frame_cached`.
        //
        // # Why this must walk the tree
        //
        // `render_frame_cached` bottoms out in `render_frame`, which draws **one** widget —
        // the root — and never touches `direct_children_of`. On this backend an ordinary
        // control (`create_button`, `create_label`, …) has no native control of its own and
        // is not a mounted surface either, so the window's own painter is the *only* thing
        // that can draw it. Asking for the root's single-widget frame therefore blitted the
        // window's background and chrome over an otherwise empty client area: exactly the
        // blank white board this arm exists to remove.
        //
        // This was a real defect, not a hypothetical one: the comment above this arm already
        // claimed "both call `render_frame_tree`, which walks the window's child list" while
        // the code called something that does not. The Linux backend
        // (`linux/platform_impl.rs`, the `connect_draw` window painter) is the working
        // reference and calls this same function.
        //
        // # Why the cache is not lost
        //
        // The previous code used the cached entry point to avoid re-rasterising the whole
        // window on every expose. `render_frame_tree` has no size-keyed cache of its own, so
        // the frame is built per paint. That is the correct trade here: a stale or partial
        // frame is a wrong picture, whereas re-rasterising is only slow, and correctness of
        // what is on screen outranks the saving. The clear colour must therefore cover the
        // client area, which `render_frame_tree`'s own `begin_frame(clear)` does — and it
        // does so even when no child paints.
        let Some(frame) = crate::widget::runtime::render_frame_tree(
            widget_id,
            crate::core::Size::new(width, height),
            crate::core::Color::WHITE,
        ) else {
            // Not an error: a window whose association was torn down between the expose and
            // this call has nothing to present, and the `WM_ERASEBKGND` arm already left the
            // background filled. Reported at debug so a blank window is still diagnosable.
            log::debug!("[windows] WM_PAINT: widget id={widget_id} produced no frame to present");
            EndPaint(hwnd, &paint);
            return;
        };
        super::canvas::blit_frame(hdc, width, height, &frame);
    }

    EndPaint(hwnd, &paint);
}

/// The widget-registry id of the window whose client area `hwnd` is.
///
/// Extracted from [`paint_target_for`] because the input arms need the same resolution:
/// a `WM_MOUSEMOVE` and a `WM_PAINT` for the same window must agree on which widget tree
/// they name, and two copies of a two-hop translation is how they would stop agreeing.
///
/// `hwnd` → platform id → widget-registry id, in that order. Win32 hands every callback
/// the `HWND`, the backend binds `HWND`s against **platform** ids, and the tree is keyed by
/// **registry** id, so both hops are required.
#[cfg(target_os = "windows")]
unsafe fn window_widget_for(hwnd: HWND) -> Option<u64> {
    crate::widget::runtime::widget_id_for_host_window(widget_id_by_native_handle(hwnd)?)
}

/// The platform's own id for `hwnd`, or `None` when this backend did not create it.
#[cfg(target_os = "windows")]
unsafe fn widget_id_by_native_handle(hwnd: HWND) -> Option<u64> {
    // SAFETY: forwarded from this function's own unsafe contract — `hwnd` is a live handle
    // Win32 handed to a callback, which is what `widget_id_by_native_handle` requires.
    unsafe { notify::active_windows_platform()?.widget_id_by_native_handle(hwnd) }
}

/// Marks `hwnd`'s whole client area as needing a repaint.
#[cfg(target_os = "windows")]
unsafe fn invalidate_window(hwnd: HWND) {
    // SAFETY: a null `RECT` invalidates the entire client area, which is intended.
    winapi::um::winuser::InvalidateRect(hwnd, std::ptr::null(), 0);
}

#[cfg(target_os = "windows")]
use crate::platform::MousePhase;

/// Translates a Win32 mouse message on a **window** into a widget event and delivers it.
///
/// # Coordinate space, and why the window is not offset
///
/// The low and high words of `lparam` hold client-area coordinates, and a child control's
/// geometry is in that same window-relative space, so the point is used as Win32 reports
/// it. The **window widget itself** carries its screen position (`demo/control`'s window is
/// at `x: 100, y: 100`), because that is the geometry it was created with and a window has
/// no parent to be relative to.
///
/// Those are two different spaces in one tree, which is why [`crate::widget::runtime::widget_at`]
/// treats a root as a container rather than testing it against its own bounds: the root's
/// rectangle describes where the OS put the window, and the point describes where the
/// pointer is inside it. Asking the root to contain a client point is the mismatch that
/// made every click miss.
///
/// # Routing
///
/// `dispatch_pointer_event` hit-tests the point against the window's child tree, so a click
/// on a button reaches that button. Child geometry is window-relative, which is the space
/// `position` is already in.
#[cfg(target_os = "windows")]
unsafe fn forward_window_mouse(hwnd: HWND, lparam: isize, phase: MousePhase) {
    use crate::core::Point;
    let Some(window_id) = window_widget_for(hwnd) else {
        return;
    };
    let client_x = (lparam & 0xFFFF) as u16 as i16 as i32;
    let client_y = ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32;
    // Client coordinates, with **no** window offset.
    //
    // A window's own geometry holds the `x`/`y` the caller asked the operating system
    // for, while its children are placed in client coordinates — two different spaces, as
    // `widget::runtime::frame_origin` documents for the painter. A child therefore sits at
    // exactly the client point Win32 reports, and the root is the widget whose bounds do
    // not describe that space at all.
    let position = Point::new(client_x, client_y);
    let modifiers = crate::platform::windows::canvas::current_modifiers();
    let event = match phase {
        MousePhase::Press => {
            crate::event::Event::mouse_press_with(position.x, position.y, MOUSE_PRIMARY, modifiers)
        }
        MousePhase::Release => {
            crate::event::Event::MouseRelease { pos: position, button: MOUSE_PRIMARY }
        }
        MousePhase::Drag => crate::event::Event::MouseMove { pos: position },
        MousePhase::SecondaryPress => crate::event::Event::mouse_press_with(
            position.x,
            position.y,
            crate::event::mouse_button::SECONDARY,
            modifiers,
        ),
        MousePhase::SecondaryRelease => crate::event::Event::MouseRelease {
            pos: position,
            button: crate::event::mouse_button::SECONDARY,
        },
        MousePhase::DoubleClick => {
            crate::event::Event::mouse_double_click(position.x, position.y, MOUSE_PRIMARY)
        }
    };
    let delivered = crate::widget::runtime::dispatch_pointer_event(window_id, &event, position);
    if matches!(phase, MousePhase::Drag) {
        // The request is consumed by the event it produces, so it must be re-issued on
        // every move — without it the leave message never arrives and hover sticks.
        let mut track: winapi::um::winuser::TRACKMOUSEEVENT = std::mem::zeroed();
        track.cbSize = std::mem::size_of::<winapi::um::winuser::TRACKMOUSEEVENT>() as u32;
        track.dwFlags = winapi::um::winuser::TME_LEAVE;
        track.hwndTrack = hwnd;
        winapi::um::winuser::TrackMouseEvent(&mut track);
    }
    if delivered {
        // Any button press can move focus to a nested control, so the window keeps the
        // keyboard and subsequent keys are delivered here. A right-click counts: the
        // library's router focuses whatever it hits regardless of button.
        if matches!(phase, MousePhase::Press | MousePhase::SecondaryPress | MousePhase::DoubleClick)
        {
            winapi::um::winuser::SetFocus(hwnd);
        }
        invalidate_window(hwnd);
    }
}

/// The library's button number for the primary (left) button.
///
/// Spelled once here rather than as a bare `1` at each construction site: the value is
/// [`crate::event::mouse_button::PRIMARY`], and a raw `1` next to a named `SECONDARY`
/// reads as if the two came from different conventions.
#[cfg(target_os = "windows")]
const MOUSE_PRIMARY: u32 = crate::event::mouse_button::PRIMARY;

/// Translates a `WM_MOUSEWHEEL` into a widget wheel event and delivers it.
///
/// # Why this is not `forward_window_mouse`
///
/// Win32 packs the wheel message differently from a pointer message in two ways, and both
/// matter:
///
/// 1. the delta is the **high word of `wParam`**, signed, in multiples of `WHEEL_DELTA`
///    (120) — `lParam` holds no wheel data at all; and
/// 2. the point in `lParam` is in **screen** coordinates, not client ones, because Win32
///    delivers the wheel to the focused window wherever the pointer happens to be.
///
/// Decoding the delta as pixels would make one notch scroll 120 lines, and skipping the
/// `ScreenToClient` conversion would scroll whatever control sits at the pointer's *screen*
/// position, which is normally a different control (or none) entirely.
///
/// # Why the wheel goes to the widget under the pointer
///
/// That is what every desktop toolkit does, and it is what makes scrolling the control the
/// user is pointing at work while a text field elsewhere keeps focus. The library's router
/// already hit-tests a point against the tree, so no extra routing logic is needed here.
///
/// The division by `WHEEL_DELTA` rounds the sub-notch deltas a high-resolution or
/// touchpad-driven wheel produces to the nearest whole notch. Rounding *away* from zero
/// when the quotient is `0` is deliberate: a delta of `-1..-119` is still a scroll in that
/// direction, and discarding it would make a slow touchpad scroll do nothing at all.
#[cfg(target_os = "windows")]
unsafe fn forward_window_wheel(hwnd: HWND, wparam: usize, lparam: isize) {
    use crate::core::Point;
    use winapi::um::winuser::{ScreenToClient, GET_WHEEL_DELTA_WPARAM, WHEEL_DELTA};
    let Some(window_id) = window_widget_for(hwnd) else {
        return;
    };
    // `GET_WHEEL_DELTA_WPARAM` masks and sign-extends the high word for us.
    let raw_delta = GET_WHEEL_DELTA_WPARAM(wparam);
    if raw_delta == 0 {
        return;
    }
    // The point arrives in screen space; the tree is laid out in client space.
    let mut screen = winapi::shared::windef::POINT {
        x: (lparam & 0xFFFF) as u16 as i16 as i32,
        y: ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32,
    };
    if ScreenToClient(hwnd, &mut screen) == 0 {
        return;
    }
    let position = Point::new(screen.x, screen.y);
    // The widget layer speaks in whole notches with a downward-positive convention; a
    // positive Win32 delta means "away from the user", so the sign is inverted.
    let notches = match raw_delta / WHEEL_DELTA {
        0 if raw_delta > 0 => 1,
        0 => -1,
        whole => whole as i32,
    };
    let event = crate::event::Event::Wheel {
        delta: Point::new(0, -notches),
        modifiers: crate::platform::windows::canvas::current_modifiers(),
    };
    if crate::widget::runtime::dispatch_pointer_event(window_id, &event, position) {
        invalidate_window(hwnd);
    }
}

/// Delivers a produced character to the focused widget of `window_id`.
///
/// # Why this is separate from `forward_window_mouse`'s key handling
///
/// The character comes from `WM_CHAR`/`WM_UNICHAR`, not from `WM_KEYDOWN`, and the two
/// carry different things: `WM_KEYDOWN`'s `wParam` is a virtual-key code, while this is the
/// character the key produced (see the `WM_CHAR` arm for why forwarding the code was
/// wrong). `Event::TextInput` is the event the widget layer defines for it, so a text
/// control receives the character and a non-text control ignores it.
///
/// # Which controls can be typed into is decided by the library
///
/// Nothing is filtered here: `dispatch_event` reaches the focused widget and the widget
/// decides whether a character means anything to it. That is the same contract the Linux
/// backend's key handler follows, and it keeps this backend from carrying a list of "text
/// kinds" that would drift from the widget set.
#[cfg(target_os = "windows")]
unsafe fn forward_window_char(hwnd: HWND, window_id: u64, character: u32) {
    // Control characters (backspace 0x08, tab 0x09, escape 0x1B, the C0 range generally)
    // are *commands*, not text, and `WM_KEYDOWN` already delivered them as key presses.
    // Forwarding them here as `TextInput` would ask a `LineEdit` to insert a paragraph
    // mark for Enter and a literal tab character for Tab, which is not what either means.
    //
    // `VK_PROCESSKEY` (0xE5) is the IME's "I am composing" placeholder: Win32 posts it as
    // a `WM_KEYDOWN` and the real characters arrive later as `WM_CHAR`, so dropping it here
    // is correct rather than lossy.
    if character < 0x20 || character == 0x7F {
        return;
    }
    let Some(character) = char::from_u32(character) else {
        // A surrogate half or an invalid scalar. `WM_CHAR` can deliver UTF-16 code units
        // for text outside the BMP, which are not valid `char`s on their own; reporting
        // rather than guessing keeps a mojibake string out of the document.
        log::debug!(
            "[windows] WM_CHAR delivered U+{character:04X}, which is not a Unicode scalar value"
        );
        return;
    };
    let target = crate::widget::runtime::focused_widget().unwrap_or(window_id);
    // `Event::text_input` rather than a struct literal: the variant carries a `String`
    // (the committed text, which may be several code units once a layout or an IME has
    // had its say), so the constructor is the one place that spelling lives.
    let event = crate::event::Event::text_input(character.to_string());
    if crate::widget::runtime::dispatch_event(target, &event) {
        invalidate_window(hwnd);
    }
}

/// Resolves the widget-registry id whose tree this window should paint.
///
/// Both hops are done here rather than inline so the resolution has one definition, and
/// so a failure can say *which* hop failed: an unknown handle and a known handle with no
/// widget behind it are different faults, and collapsing them into one `None` is how a
/// missing association goes unnoticed.
#[cfg(target_os = "windows")]
unsafe fn paint_target_for(hwnd: HWND) -> Option<u64> {
    let Some(host) = widget_id_by_native_handle(hwnd) else {
        log::error!("[windows] WM_PAINT for hwnd {hwnd:?}, which has no platform id bound to it");
        return None;
    };
    // `host` is the **platform** id `create_window` returned, so the registry lookup is the
    // second hop and `host` is its input. Feeding it to `widget_id_for_host_window` is not
    // the same question: that function's argument is a widget-registry id.
    match crate::widget::runtime::widget_id_for_host_window(host) {
        Some(id) => Some(id),
        None => {
            log::error!(
                "[windows] window hwnd {hwnd:?} (platform id {host}) has no widget-registry \
                 association yet; nothing to paint"
            );
            None
        }
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
    /// # Why this is a map and not `GWLP_USERDATA`
    ///
    /// The two directions have different information available. Given an `HWND`, Win32 can
    /// answer "which widget is this?" through `GWLP_USERDATA` — see
    /// [`Self::widget_id_by_native_handle`], which is the direction every window procedure
    /// needs and the hot path. Given an **id**, nothing in Win32 can name the window: the
    /// association exists only because this backend created it, so it has to be remembered.
    /// That is what this map is, and it is the only per-widget side table still kept here.
    ///
    /// Only ids that were bound via [`Self::bind_native_handle`] are present; a widget id the
    /// library created without a host window has no handle and yields `None`. A poisoned map
    /// is logged and treated as `None` rather than panicking inside a message-pump callback.
    pub fn get_native_handle(&self, id: u64) -> Option<HWND> {
        #[cfg(target_os = "windows")]
        {
            match self.menu_state.handles.lock() {
                Ok(handles) => handles.get(&id).map(|&h| h as HWND),
                Err(_) => {
                    log::error!(
                        "[rust_widgets][windows] get_native_handle: handles mutex poisoned; \
                         widget id={id} cannot be resolved to its window"
                    );
                    None
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = id;
            None
        }
    }
    /// Records `hwnd` as the native window for `id`.
    ///
    /// # Both directions are recorded, and why both are needed
    ///
    /// * `GWLP_USERDATA` on the window answers "which widget is this `HWND`?" — the question
    ///   every window procedure asks, in O(1) with no lock.
    /// * `menu_state.handles` answers "which `HWND` is this widget?" — the question the
    ///   platform API asks when it is handed an id, which Win32 cannot answer at all.
    ///   See [`Self::get_native_handle`] for why that direction needs a map.
    ///
    /// Binding an id twice replaces the previous handle. A null `hwnd` is rejected: it names
    /// no window, so recording it would destroy a previously valid association.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a valid window handle that stays alive for as long as it is recorded
    /// here: it is written into `GWLP_USERDATA` and stored in the id → `HWND` map, and a
    /// later `WM_*` message for it would be dispatched against the widget it names.
    #[cfg(target_os = "windows")]
    pub unsafe fn bind_native_handle(&self, id: u64, hwnd: HWND) {
        #[cfg(target_os = "windows")]
        {
            if hwnd.is_null() {
                log::error!(
                    "[rust_widgets][windows] bind_native_handle called with a null HWND for \
                     widget id={id}; the association was not recorded"
                );
                return;
            }
            // SAFETY: `hwnd` is non-null and was created by this backend (or adopted from the
            // host through `create_window`), so it is a live window; `GWLP_USERDATA` is
            // reserved for the owning application by Win32's own contract and holds nothing
            // else. The value round-trips as `isize` and is read back by
            // `widget_id_by_native_handle`.
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, id as isize);
            }
            match self.menu_state.handles.lock() {
                Ok(mut handles) => {
                    handles.insert(id, hwnd as usize);
                }
                Err(_) => log::error!(
                    "[rust_widgets][windows] bind_native_handle: handles mutex poisoned; \
                     widget id={id} cannot be resolved back to its window"
                ),
            }
            self.a11y_bridge.register_handle(id, hwnd as usize);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (id, hwnd);
        }
    }
    /// Stamps `hwnd` with a fresh command id and maps that id to `widget_id`.
    ///
    /// # Why a control needs this at all
    ///
    /// Win32 reports a control activation as `WM_COMMAND`, whose `wParam` carries the
    /// control's **`GWLP_ID`** and not its `HWND`. The `WM_COMMAND` arm can therefore only
    /// resolve the originating widget if the id was stamped and recorded at creation time,
    /// which is what this does. `DlgCtrlID` is not a substitute: it reads back whatever is
    /// already in `GWLP_ID`, so a control that was never stamped answers `0`.
    ///
    /// # Who calls it
    ///
    /// A host that puts one of its *own* native controls in a window this backend owns —
    /// the library creates none, because it paints every `WidgetKind` itself. That is why a
    /// control made by this crate never reaches here, and why the `WM_COMMAND` arm is still
    /// correct: it resolves the host's controls rather than a library control's.
    ///
    /// # Safety
    ///
    /// Caller must ensure that `hwnd` is a valid native window handle and that it remains
    /// valid for the duration of this call. Modifying the window's identifier via
    /// `SetWindowLongPtrW` can affect window procedure behavior; callers should ensure this
    /// is done only for windows owned by this platform adapter.
    #[cfg(target_os = "windows")]
    pub unsafe fn bind_control_command(&self, widget_id: u64, hwnd: HWND) {
        if hwnd.is_null() {
            log::error!(
                "[rust_widgets][windows] bind_control_command called with a null HWND for \
                 widget id={widget_id}; WM_COMMAND will not resolve it"
            );
            return;
        }
        let command_id = self.menu_state.next_command_id.fetch_add(1, Ordering::SeqCst) as u32;
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_ID, command_id as isize);
        }
        match self.menu_state.control_command_to_widget.lock() {
            Ok(mut map) => {
                map.insert(command_id, widget_id);
            }
            Err(_) => log::error!(
                "[rust_widgets][windows] bind_control_command: the command map is poisoned, so \
                 command id {command_id} (widget {widget_id}) was not recorded and WM_COMMAND \
                 will not resolve it"
            ),
        }
    }
    /// Forgets both halves of the association [`Self::bind_native_handle`] recorded.
    ///
    /// # Why `bind` alone is not enough
    ///
    /// [`Self::unmount_surface`] destroys the canvas window. The id → `HWND` map would then
    /// keep pointing at a handle Win32 has destroyed and may already have recycled for an
    /// unrelated window, so a later [`Self::get_native_handle`] would hand out an `HWND`
    /// belonging to something else — and `invalidate_surface` would call `GetParent` on it.
    /// That is worse than a stale entry: it is a wrong entry that looks valid.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a live window handle, so that clearing its `GWLP_USERDATA` cannot
    /// write through a recycled handle. Call it *before* destroying the window, which is
    /// what `unmount_surface` does.
    #[cfg(target_os = "windows")]
    pub unsafe fn unbind_native_handle(&self, id: u64, hwnd: HWND) {
        if !hwnd.is_null() {
            // SAFETY: the caller guarantees a live handle; this only clears the marker, so a
            // later `widget_id_by_native_handle` for the same handle answers `None` rather
            // than naming a widget that is no longer mounted.
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        match self.menu_state.handles.lock() {
            Ok(mut handles) => {
                handles.remove(&id);
            }
            Err(_) => log::error!(
                "[rust_widgets][windows] unbind_native_handle: handles mutex poisoned; the \
                 entry for widget id={id} was not released"
            ),
        }
        self.a11y_bridge.unregister_handle(id);
    }
    /// Returns the widget id whose native handle is `hwnd`.
    ///
    /// # Why the identity is read from the window rather than looked up
    ///
    /// Win32 hands every callback an `HWND` and nothing else, while a widget is addressed by
    /// a library id. The association has to be stored somewhere, and `GWLP_USERDATA` on the
    /// window is where Win32 convention puts it — one write at bind time, one read at lookup
    /// time, no map and no lock. The first version of this used a `HashMap<u64, usize>` and
    /// answered with a **linear scan under a mutex**, which every `WM_MOUSEMOVE`, `WM_PAINT`
    /// and `WM_SIZE` paid, on the message thread that is also the only thread painting.
    ///
    /// `None` is the honest answer for a handle this backend did not bind — a window the
    /// host created and handed over without `bind_native_handle`, or a canvas that has since
    /// been unmounted. It is not an error and is not logged here; the callers that need one
    /// (`paint_target_for`) say so with the context that makes it diagnosable.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a live window handle. Win32 callbacks are the only callers and they
    /// receive it from the message, which is what makes it live for the duration of the call.
    #[cfg(target_os = "windows")]
    pub unsafe fn widget_id_by_native_handle(&self, hwnd: HWND) -> Option<u64> {
        if hwnd.is_null() {
            return None;
        }
        // SAFETY: the caller guarantees a live handle; `GetWindowLongPtrW` reads one
        // pointer-sized word and does not dereference it.
        let marker = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        (marker != 0).then_some(marker as u64)
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
    /// Whether [`Platform::init`] has already run.
    ///
    /// Read at the top of `init` to make repeat entry a no-op: `init` registers the
    /// platform and calls `InitCommonControls`, and doing either twice is wasteful at
    /// best.
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
/// Per-widget state this backend keeps outside the shared [`BackendState`].
///
/// # What is here, and what used to be
///
/// It began as the menu/command router and the name stuck. Two of its original maps were
/// genuinely dead and have been removed rather than left as scaffolding:
///
/// * `menu_command_to_item` / `pending_menu_events` had **no writer anywhere in the
///   crate**, so the `WM_COMMAND` branch that read them could never be taken. A router
///   whose inputs are never produced is a claim, not a capability (principle #4/#41).
/// * `pending_widget_events` is fed by `WM_NOTIFY` from the host's own adopted controls,
///   which is a real path and stays.
///
/// Every field below is written by a live producer and read by a live consumer.
#[cfg(target_os = "windows")]
pub struct Win32MenuState {
    /// Widget id → `HWND`, for every widget this backend bound a window to.
    ///
    /// Only the **reverse** direction needs a map: `HWND` → id is answered by Win32 through
    /// `GWLP_USERDATA` (see [`WindowsPlatform::widget_id_by_native_handle`]), which is the
    /// hot path taken by every message procedure. Nothing in Win32 can turn an id into a
    /// window, so that direction is remembered here.
    pub(crate) handles: Mutex<HashMap<u64, usize>>,
    /// Command id → widget id, for a native control the host put in one of this backend's
    /// windows. Stamped onto the control with `SetWindowLongPtrW(GWLP_ID)`.
    pub(crate) control_command_to_widget: Mutex<HashMap<u32, u64>>,
    /// Triggers produced by native notifications, waiting for the host to poll them.
    pub(crate) pending_widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    /// Allocator for the command ids [`WindowsPlatform::bind_control_command`] stamps.
    /// Seeded past the range a dialog template uses for its own ids, so a template control
    /// and a stamped one cannot collide.
    pub(crate) next_command_id: AtomicU64,
}
#[cfg(target_os = "windows")]
impl Win32MenuState {
    fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            control_command_to_widget: Mutex::new(HashMap::new()),
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
