// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! C ABI drawing surfaces and host-dispatched input events.
use super::*;
use crate::c_try;
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_uint;

/// Mounts a widget onto a surface and returns whether the backend accepted it.
///
/// The caller must have created the widget first; `parent` is the window it should
/// be displayed in, and `x`/`y`/`width`/`height` are the rectangle it occupies.
///
/// Returns `false` when the backend cannot display library-painted widgets
/// (`Platform::supports_surfaces()`), or when the id is not one this backend
/// created. A caller must treat `false` as "cannot display here" and say so, rather
/// than presenting an empty surface.
#[no_mangle]
pub extern "C" fn rw_mount_surface(
    parent: u64,
    widget_id: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> CBool {
    c_try!({ crate::mount_surface(parent, widget_id, crate::core::Rect::new(x, y, width, height)) })
}

/// Updates the rectangle of a mounted surface.
///
/// Returns `false` when `widget_id` is not mounted, which is the honest answer for
/// a surface that was never mounted or has already been released.
#[no_mangle]
pub extern "C" fn rw_resize_surface(
    widget_id: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> CBool {
    c_try!({ crate::resize_surface(widget_id, crate::core::Rect::new(x, y, width, height)) })
}

/// Releases a mounted surface.
///
/// The widget itself stays alive in the registry; only its display surface is
/// released. Returns `false` when it was not mounted.
#[no_mangle]
pub extern "C" fn rw_unmount_surface(widget_id: u64) -> CBool {
    c_try!({ crate::unmount_surface(widget_id) })
}

/// Marks a mounted surface — or a window the backend draws — as needing a repaint.
///
/// Returns `false` when the id names nothing this backend can repaint. A window id
/// is a legitimate argument: the library repaints a window to reveal the children
/// it draws into the window's frame (see `Platform::invalidate_surface`).
#[no_mangle]
pub extern "C" fn rw_invalidate_surface(widget_id: u64) -> CBool {
    c_try!({ crate::invalidate_surface(widget_id) })
}

/// Whether this backend can host library-painted widgets.
///
/// `true` means [`rw_mount_surface`] will produce a surface that repaints; it says
/// nothing about how a surface is presented. A host that builds a UI it cannot
/// display should ask this first, because the alternative is a blank window with no
/// error — the exact failure mode these entry points were added to remove.
#[no_mangle]
pub extern "C" fn rw_supports_surfaces() -> CBool {
    c_try!({ crate::platform::get_platform().supports_surfaces() })
}

/// Renders one frame of a mounted widget and hands the caller its RGBA pixels.
///
/// # Why this is a getter rather than a "blit into my buffer" call
///
/// The host owns the drawing API (ArkUI `Canvas`, Android `Bitmap`, `NSView`,
/// Skia), so the library cannot draw into it without knowing every toolkit. What it
/// can do — and what this does — is produce the frame in the one layout all of them
/// consume: top-down, straight (non-premultiplied) alpha, `width * height * 4`
/// bytes.
///
/// # Contract
///
/// `x`/`y`/`width`/`height` describe the surface rectangle to render. `out_width`
/// and `out_height` receive the pixel dimensions actually produced, `out_len` the
/// byte length of the returned buffer, and `out_stride` the bytes between the start
/// of two consecutive rows (always `width * 4` today, but published rather than
/// assumed, because a host that hard-codes it shears the image the day a backend
/// pads a row).
///
/// The pixel buffer is returned through `out_pixels`, which the caller owns and must
/// release with [`rw_free_bytes`]. It is set to null when the widget is not mounted,
/// has no `Draw` implementation, or the area is empty — and that null is **not** an
/// error to swallow: it is why the surface stayed blank.
///
/// # Why the buffer is an out-parameter and not the return value
///
/// The error channel (`c_try!`) has no `CAbiSafe` implementation for `*mut u8`, and
/// that absence is deliberate: a raw byte pointer cannot be round-tripped through the
/// numeric fallback the macro uses for its primitives. Publishing the pointer through
/// an out-parameter keeps the frame path on the same error handling every other
/// function here uses, instead of needing a second, weaker convention for one call.
///
/// Returns `true` when a frame was produced. On `false`, every out-parameter is left
/// untouched.
///
/// # Safety
///
/// Every out-pointer may be null, in which case that value is simply not reported. A
/// non-null pointer must be valid and writable for one value of its type for the
/// duration of the call.
#[no_mangle]
pub unsafe extern "C" fn rw_render_surface_frame(
    widget_id: u64,
    width: c_uint,
    height: c_uint,
    out_width: *mut c_uint,
    out_height: *mut c_uint,
    out_stride: *mut c_uint,
    out_len: *mut c_uint,
    out_pixels: *mut *mut u8,
) -> CBool {
    c_try!({
        let Some(frame) = crate::widget::runtime::render_frame_cached(
            widget_id,
            crate::core::Size::new(width, height),
            crate::core::Color::WHITE,
        ) else {
            return false;
        };
        let byte_len = frame.len() as c_uint;
        // The buffer is only handed to the caller when there is an output pointer to
        // receive it. `out_pixels` may be null (the safety contract above allows it,
        // and a caller probing the size/stride reports may pass null deliberately), in
        // which case a `forget` would drop the only owning pointer on the floor and
        // leak the frame. So the `Box<[u8]>` is leaked **only** on the transfer path;
        // otherwise it is left to drop and free normally at the end of the call.
        //
        // Leaked as one allocation, exactly like `rw_poll_drop_event`'s payload, so
        // `rw_free_bytes(ptr, len)` reconstructs it with the length the caller echoed
        // back. Handing out a `Vec` pointer without its length is what forced the
        // separate free function to exist in the first place.
        let mut buffer = frame.into_boxed_slice();
        let pointer = if out_pixels.is_null() {
            core::ptr::null_mut()
        } else {
            let pointer = buffer.as_mut_ptr();
            core::mem::forget(buffer);
            pointer
        };
        // SAFETY: each out-pointer is null-checked before use, and the caller's
        // contract is that a non-null one is writable for one value of its type.
        unsafe {
            if !out_pixels.is_null() {
                *out_pixels = pointer;
            }
            if !out_width.is_null() {
                *out_width = width;
            }
            if !out_height.is_null() {
                *out_height = height;
            }
            if !out_stride.is_null() {
                *out_stride = width * 4;
            }
            if !out_len.is_null() {
                *out_len = byte_len;
            }
        }
        true
    })
}

/// Reads the next widget awaiting a repaint, or `0` when none is queued.
///
/// # Why this is a queue rather than a callback
///
/// The host owns the event loop on these platforms, so the library cannot call
/// "please repaint" into it. It records which surfaces went stale and the host pulls
/// them — which is also what makes a burst of invalidations cost one entry instead of
/// one per call (they are coalesced, see
/// `BackendState::record_repaint_request`).
///
/// # Both id spaces
///
/// The id returned is the one the invalidation named, which is the **widget id**
/// for an ordinary control and may be a widget id or a mounted-surface id depending
/// on the caller. A host that needs a window handle from it can resolve it with the
/// same association `rw_get_widget_geometry` uses.
#[no_mangle]
pub extern "C" fn rw_take_pending_repaint() -> u64 {
    c_try!({ crate::take_pending_repaint().unwrap_or(0) })
}

/// Delivers a pointer event that arrived on a host-owned drawing surface.
///
/// # Why a host needs this, and why it was missing
///
/// On the desktop backends the library drives the event loop, so a click is routed inside the
/// toolkit callback (`windows/types.rs`' window procedure, `linux/platform_impl.rs`' GTK
/// handler). A host that owns its own loop — ArkUI, an Android `Activity`, iOS touch
/// handling, a browser page — is never called into, so it must hand its touches back. There
/// was no way to do that.
///
/// This is not a nicety. `src/platform/android/status.md`, `src/platform/ios/status.md` and
/// `docs/plans/harmony_integration.md` all instruct a host to call
/// `rw_dispatch_pointer_event(...)`, and `grep -rn rw_dispatch_pointer_event src/ include/`
/// returned nothing — the documented integration step, without which a mounted widget is
/// visible but completely inert, had no implementation at any layer.
///
/// # Arguments
///
/// * `root` — the widget subtree to resolve the point in, normally the window. The library
///   hit-tests `point` against it, so the host does **not** need to know which widget was
///   under the finger. Sending an event to a chosen widget id is the separate
///   `rw_dispatch_event_to_widget` below, and is only right when the host already knows the
///   target (a key event for the focused control, a text commit).
/// * `event_code` — `1` press, `2` release, `3` move, `4` enter, `5` leave. Any other value,
///   including `0`, returns `false` rather than being silently treated as a move: a
///   mis-encoded press that became a hover would highlight the control and never activate it,
///   with no error anywhere.
/// * `x`/`y` — in the **window's** coordinate space.
/// * `button` — reserved for a future multi-button path; the library's press/release events
///   carry the primary button, and the value is passed through unchanged.
///
/// # Returns
///
/// Whether a widget accepted the event. `false` means the point was outside every widget in
/// `root`, the resolved widget refused it (disabled, or blocked by a modal), or `event_code`
/// was not one of the five codes.
#[no_mangle]
pub extern "C" fn rw_dispatch_pointer_event(
    root: u64,
    event_code: c_uint,
    x: c_int,
    y: c_int,
    button: c_uint,
) -> CBool {
    c_try!({
        match pointer_event_from_code(event_code, x, y, button) {
            Some(event) => {
                crate::dispatch_pointer_event(root, &event, crate::core::Point::new(x, y))
            }
            None => false,
        }
    })
}

/// Delivers a pointer event straight to `widget_id`, without hit-testing.
///
/// The counterpart to `rw_dispatch_pointer_event` for a host that already knows the target. It
/// shares the same event codes, so a host needs one vocabulary rather than two.
///
/// Routing a pointer event this way **skips the library's hit test**, so a host that uses it
/// for touches must do its own; the pointer path above is the one the integration documents
/// name for that reason.
///
/// Keyboard, text and wheel input have their own carriers — a key code + modifier mask, a
/// committed text string, and a wheel delta — which the pointer `(x, y, button)` arguments
/// cannot express. Those use [`rw_dispatch_key_event`], [`rw_dispatch_text_event`] and
/// [`rw_dispatch_wheel_event`] below.
///
/// Returns whether the widget accepted it: `false` when the id is not a live widget, when a modal
/// blocks it, or when `event_code` is unknown. A **disabled** control is refused by
/// [`rw_dispatch_pointer_event`] before delivery; this entry point delivers to the live id it is
/// given, so a host that routes input straight to a control it disabled must check its own state.
#[no_mangle]
pub extern "C" fn rw_dispatch_event_to_widget(
    widget_id: u64,
    event_code: c_uint,
    x: c_int,
    y: c_int,
    button: c_uint,
) -> CBool {
    c_try!({
        match pointer_event_from_code(event_code, x, y, button) {
            Some(event) => crate::dispatch_event(widget_id, &event),
            None => false,
        }
    })
}

/// Delivers a key press/release straight to `widget_id`.
///
/// `key` and `modifiers` use the library's own conventions (see
/// [`crate::event::Event::KeyPress`]); `release` selects a `KeyRelease` over a `KeyPress`.
/// This is the direct-key counterpart to [`rw_dispatch_event_to_widget`], for a host that
/// already owns the focused control.
#[no_mangle]
pub extern "C" fn rw_dispatch_key_event(
    widget_id: u64,
    key: c_uint,
    modifiers: c_uint,
    release: CBool,
) -> CBool {
    c_try!({
        let event = if release {
            crate::event::Event::key_release(key, modifiers)
        } else {
            crate::event::Event::key_press(key, modifiers)
        };
        crate::dispatch_event(widget_id, &event)
    })
}

/// Delivers committed text (keyboard layout, IME, virtual keyboard, or paste) to `widget_id`.
///
/// The text is already filtered through the active keyboard layout, so it is not derivable from
/// a raw key code — which is exactly why it needs its own carrier rather than a key event.
#[no_mangle]
pub extern "C" fn rw_dispatch_text_event(widget_id: u64, text: *const c_char) -> CBool {
    c_try!({
        let event = crate::event::Event::text_input(c_str_or_default(text));
        crate::dispatch_event(widget_id, &event)
    })
}

/// Delivers a wheel/scroll event to `widget_id`.
///
/// The delta is in wheel notches (see [`crate::event::Event::Wheel`]); `delta_y` is vertical
/// (positive = scroll down) and `delta_x` is horizontal (positive = scroll right).
#[no_mangle]
pub extern "C" fn rw_dispatch_wheel_event(
    widget_id: u64,
    delta_x: c_int,
    delta_y: c_int,
    modifiers: c_uint,
) -> CBool {
    c_try!({
        let event = crate::event::Event::wheel(delta_x, delta_y, modifiers);
        crate::dispatch_event(widget_id, &event)
    })
}

/// Reports a container's new client size, so the app layer re-runs its layout.
///
/// # Why a host must call this
///
/// On these platforms the library holds no native window handle, so it cannot
/// subscribe to a size change: OpenHarmony delivers one to the ArkTS component's
/// `onAreaChange`, Android to the view's layout callback, iOS to the view
/// controller. Without forwarding it here, a window the user resized keeps every
/// child at the geometry it had for the previous size — the layout never re-runs,
/// and nothing looks wrong enough to report.
///
/// Returns `false` for an id this backend did not create, in which case neither the
/// size nor the trigger is recorded.
#[no_mangle]
pub extern "C" fn rw_report_window_resize(window_id: u64, width: c_uint, height: c_uint) -> CBool {
    c_try!({ crate::queue_resize_trigger(window_id, width, height) })
}
