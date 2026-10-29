// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared helpers for the C ABI entry points.
use alloc::ffi::CString;
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_uint;
use core::ffi::CStr;

pub(crate) type CBool = bool;

/// Convert a stable C ABI pointer-event code into an [`crate::event::Event`].
///
/// # The codes, and why `0` is not one of them
///
/// A host that owns its own event loop forwards touches and mouse events through
/// `rw_dispatch_pointer_event`. It has to name *which* event it is, and an `int` is the only
/// vocabulary a C ABI shares, so the mapping is: `1` press, `2` release, `3` move, `4` enter,
/// `5` leave. Any other value — including `0` — yields `None`, and the caller refuses the call.
///
/// # Why an unknown code is refused rather than defaulted
///
/// The tempting default is "treat anything unrecognised as a move", because a move is the
/// least destructive. It is also the *worst* choice: a host that sends `0` because it
/// mis-encoded a press would have every click silently become a hover — the control highlights
/// and never activates, with no error anywhere. Reporting "I do not know this event" is the
/// answer a caller can act on.
///
/// Returning `Option` rather than a sentinel `Event` is the same reasoning: there is no
/// "invalid event" variant to smuggle out, so a caller cannot accidentally dispatch one.
pub(crate) fn pointer_event_from_code(
    code: c_uint,
    x: c_int,
    y: c_int,
    button: c_uint,
) -> Option<crate::event::Event> {
    // `button` is passed straight through; a move has none and the library ignores it there.
    let _ = button;
    match code {
        1 => Some(crate::event::Event::mouse_press(x, y, crate::event::mouse_button::PRIMARY)),
        2 => Some(crate::event::Event::mouse_release(x, y, crate::event::mouse_button::PRIMARY)),
        3 => Some(crate::event::Event::mouse_move(x, y)),
        4 => Some(crate::event::Event::MouseEnter { pos: crate::core::Point::new(x, y) }),
        5 => Some(crate::event::Event::MouseLeave { pos: crate::core::Point::new(x, y) }),
        _ => None,
    }
}

/// Convert stable C ABI trigger code to internal typed trigger enum.
pub(crate) fn trigger_kind_from_code(code: c_uint) -> crate::platform::WidgetTriggerKind {
    match code {
        1 => crate::platform::WidgetTriggerKind::Clicked,
        2 => crate::platform::WidgetTriggerKind::ValueChanged,
        3 => crate::platform::WidgetTriggerKind::SelectionChanged,
        4 => crate::platform::WidgetTriggerKind::Closed,
        _ => crate::platform::WidgetTriggerKind::Unknown,
    }
}

/// Convert nullable C string pointer to owned Rust `String`.
/// Logs a warning when a null pointer is received.
pub(crate) fn c_str_or_default(ptr: *const c_char) -> String {
    if ptr.is_null() {
        log::warn!("[bindings] c_str_or_default: received null C string pointer");
        return String::new();
    }
    // SAFETY: `ptr` was checked non-null above and the C ABI requires it to be NUL-terminated.
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

/// Convert a Rust string to a C string, returning an empty C string on failure.
///
/// Never panics — an interior NUL is reported rather than silently turning the answer into an
/// empty string at every call site. The log is the difference between "the caller asked for an
/// empty label" and "the label contained a NUL and was dropped".
pub(crate) fn c_string_or_empty(s: impl Into<String>) -> CString {
    match CString::new(s.into()) {
        Ok(cs) => cs,
        Err(nul_err) => {
            let pos = nul_err.nul_position();
            log::warn!(
                "[bindings] CString::new failed (interior NUL at position {pos}); returning an \
                 empty string instead"
            );
            CString::default()
        }
    }
}

/// [`c_string_or_empty`] as an owned raw pointer for the C ABI.
pub(crate) fn to_c_string_or_empty(s: impl Into<String>) -> *mut c_char {
    c_string_or_empty(s).into_raw()
}

/// Writes `items` into `out` as a space-separated, NUL-terminated list.
///
/// Shared by the enumeration entry points so they agree on the size convention:
/// the return is always the full byte length, which lets a caller discover the
/// requirement without a second API call.
pub(crate) fn write_space_separated(items: &[&str], out: *mut c_char, cap: c_uint) -> c_uint {
    // The required length is the sum of the items plus one separator between adjacent items.
    // It used to be measured after `items.join(" ")`, which allocated a second copy of the whole
    // list on every enumeration call (BIND-10). Writing is streamed for the same reason.
    let mut required = 0usize;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            required = required.saturating_add(1);
        }
        required = required.saturating_add(item.len());
    }
    let required_c = required as c_uint;
    if out.is_null() || cap == 0 {
        return required_c;
    }
    // Leave room for the terminator; copy at most `cap - 1` bytes.
    let writable = (cap as usize).saturating_sub(1).min(required);
    let mut written = 0usize;
    // SAFETY: `out`/`cap` were validated above; the loop writes at most `writable < cap` bytes and then a terminator.
    unsafe {
        for (index, item) in items.iter().enumerate() {
            if written >= writable {
                break;
            }
            if index > 0 {
                let separator = b" ";
                core::ptr::copy_nonoverlapping(
                    separator.as_ptr(),
                    out.add(written).cast::<u8>(),
                    1,
                );
                written += 1;
                if written >= writable {
                    break;
                }
            }
            let take = item.len().min(writable - written);
            core::ptr::copy_nonoverlapping(item.as_ptr(), out.add(written).cast::<u8>(), take);
            written += take;
        }
        *out.add(written) = 0;
    }
    required_c
}

/// Writes `text` into `out` as a NUL-terminated string, returning the length it needs.
///
/// Shared shape with [`write_space_separated`]: a null or zero-capacity `out` reports the
/// requirement without writing, so a caller can size a buffer in one extra call. A
/// shorter buffer is truncated rather than allowed to overrun, and the return still
/// reports the full length.
///
/// Kept private for the same reason as the caller: a public `extern "C"` function that
/// dereferences a raw pointer trips `clippy::not_unsafe_ptr_arg_deref`, and marking the
/// exported ABI `unsafe` would force every caller to reason about a contract the two-call
/// convention already documents.
pub(crate) fn write_c_string(text: &str, out: *mut c_char, cap: c_uint) -> c_uint {
    let bytes = text.as_bytes();
    let required = bytes.len() as c_uint;
    if out.is_null() || cap == 0 {
        return required;
    }
    let writable = (cap as usize).saturating_sub(1).min(bytes.len());
    // SAFETY: `out`/`cap` were validated above; the copy stays within `writable < cap` bytes and writes one terminator.
    unsafe {
        // `cast` rather than `as`, for the reason documented on
        // `write_space_separated`: `c_char` is not the same signedness everywhere.
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast::<u8>(), writable);
        *out.add(writable) = 0;
    }
    required
}

/// Reads a NUL-terminated C string, treating null as empty.
///
/// # Safety
///
/// `value` must be null or point to a NUL-terminated string.
pub(crate) unsafe fn read_c_string(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        CStr::from_ptr(value).to_string_lossy().into_owned()
    }
}

/// Parses `"x,y,w,h"` into a [`crate::core::Rect`].
///
/// Returns `None` unless all four components are present and parse, so a truncated
/// or malformed rectangle is refused rather than silently becoming a zero-sized one.
pub(crate) fn parse_rect_string(text: &str) -> Option<crate::core::Rect> {
    let mut parts = text.split(',');
    let x: i32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    let width: u32 = parts.next()?.trim().parse().ok()?;
    let height: u32 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(crate::core::Rect::new(x, y, width, height))
}
