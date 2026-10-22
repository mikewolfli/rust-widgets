// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Host-reported HarmonyOS environment facts (BLUE28 B28.07).
//!
//! # Why the host reports these three facts
//!
//! The OpenHarmony SDK answers them through APIs that need an ArkTS-side object this Rust
//! library does not own: `OH_WindowManager_GetWindowAvoidArea` takes the window id the ArkTS
//! `window` object carries, `OH_ArkUI_RegisterSystemColorModeChangeEvent` is a callback rather
//! than a synchronous query, and `fontSizeScale` lives in the ArkTS `Configuration`. The bridge
//! therefore exposes three explicit setters the ArkTS host calls from its own lifecycle:
//!
//! * [`rw_harmony_set_theme`] — `1` dark, `0` light, `-1` unknown;
//! * [`rw_harmony_set_font_scale`] — the `Configuration.fontSizeScale` multiplier;
//! * [`rw_harmony_set_safe_area`] — the merged avoid-area rectangles in vp.
//!
//! Before the host calls a setter the corresponding fact answers `None` / `1.0`. That is the
//! honest "the OS has not told us" answer, not a fabricated light theme or a zero inset.
//!
//! `rw_harmony_set_window_id` is the binding the plan asks for; it is stored so a later native
//! query can use it, and it does not itself claim to read a safe area.

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::OnceLock;

/// The ArkTS window id, or `-1` when the host has not bound one.
static WINDOW_ID: AtomicI32 = AtomicI32::new(-1);
/// `1` dark, `0` light, `-1` unknown.
static THEME: AtomicI32 = AtomicI32::new(-1);
/// `fontSizeScale` as `f32` bits; `1.0` until reported.
static FONT_SCALE_BITS: AtomicU32 = AtomicU32::new(1.0_f32.to_bits());
/// Whether the safe-area setter has been called.
static SAFE_AREA_SET: AtomicBool = AtomicBool::new(false);
static SAFE_TOP: AtomicU32 = AtomicU32::new(0);
static SAFE_BOTTOM: AtomicU32 = AtomicU32::new(0);
static SAFE_LEFT: AtomicU32 = AtomicU32::new(0);
static SAFE_RIGHT: AtomicU32 = AtomicU32::new(0);
/// Bumped by every setter, for `environment_generation`.
static GENERATION: AtomicU32 = AtomicU32::new(0);

fn bump() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// The bound ArkTS window id, when the host supplied one.
///
/// Read by `create_window` so the id the host bound is visible in the backend's diagnostics;
/// the native `OH_WindowManager` query needs it when the ArkTS bridge supplies a window object.
pub(crate) fn window_id() -> Option<i32> {
    let id = WINDOW_ID.load(Ordering::Relaxed);
    (id >= 0).then_some(id)
}

/// The system colour mode, or `None` before the host reports one.
pub(crate) fn theme() -> Option<bool> {
    match THEME.load(Ordering::Relaxed) {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}

/// The `fontSizeScale` multiplier, or `1.0` until reported.
pub(crate) fn font_scale() -> f32 {
    f32::from_bits(FONT_SCALE_BITS.load(Ordering::Relaxed))
}

/// The merged safe-area insets, or `None` until the host reports them.
pub(crate) fn safe_area() -> Option<crate::platform::types::SafeAreaInsets> {
    if !SAFE_AREA_SET.load(Ordering::Relaxed) {
        return None;
    }
    Some(crate::platform::types::SafeAreaInsets {
        top: SAFE_TOP.load(Ordering::Relaxed),
        bottom: SAFE_BOTTOM.load(Ordering::Relaxed),
        left: SAFE_LEFT.load(Ordering::Relaxed),
        right: SAFE_RIGHT.load(Ordering::Relaxed),
    })
}

/// A revision that changes when any reported fact changes.
pub(crate) fn generation() -> u64 {
    u64::from(GENERATION.load(Ordering::Relaxed))
}

/// Binds the ArkTS window object to the library.
///
/// Returns `1`. The id is stored verbatim; a negative value means "no window" and clears the
/// binding.
#[no_mangle]
pub extern "C" fn rw_harmony_set_window_id(window_id: i32) -> i32 {
    WINDOW_ID.store(window_id, Ordering::Relaxed);
    bump();
    1
}

/// Reports the system colour mode: `1` dark, `0` light, `-1` unknown.
#[no_mangle]
pub extern "C" fn rw_harmony_set_theme(dark: i32) -> i32 {
    THEME.store(dark.clamp(-1, 1), Ordering::Relaxed);
    bump();
    1
}

/// Reports an ArkTS lifecycle transition (BLUE28 B28.24).
///
/// `state` is `0` active (`onForeground`), `1` inactive (partially obscured), or `2` background
/// (`onBackground`). The library stops the animation clock for the latter two and restarts it on
/// `0`. Returns `1` when the code was understood.
#[no_mangle]
pub extern "C" fn rw_harmony_set_lifecycle(state: i32) -> i32 {
    let phase = match state {
        0 => crate::platform::MobileLifecycleState::Active,
        1 => crate::platform::MobileLifecycleState::Inactive,
        2 => crate::platform::MobileLifecycleState::Background,
        _ => return 0,
    };
    bump();
    i32::from(crate::platform::lifecycle::set_state(phase))
}

/// The one IME state machine the HarmonyOS backend exposes (BLUE28 B28.12).
///
/// ArkTS owns the soft keyboard and the composition protocol; it forwards both through the C
/// entry points below. The platform-independent `ime_macos` state machine is reused unchanged
/// because its AppKit calls are gated on macOS.
static IME_BRIDGE: OnceLock<crate::platform::ime_macos::MacOsImeBridge> = OnceLock::new();

pub(crate) fn ime_bridge() -> &'static crate::platform::ime_macos::MacOsImeBridge {
    IME_BRIDGE.get_or_init(crate::platform::ime_macos::MacOsImeBridge::new)
}

/// Reports a marked-text update from the ArkTS input method.
///
/// `text` is a NUL-terminated UTF-8 composition and `cursor_utf16` its selection start, or `-1`
/// for the end. Returns `1` when accepted, `0` for a null or non-UTF-8 string.
#[no_mangle]
// The pointer is the C ABI this function exports; null is rejected before the dereference.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn rw_harmony_ime_marked_text(
    text: *const core::ffi::c_char,
    cursor_utf16: i32,
) -> i32 {
    if text.is_null() {
        return 0;
    }
    // SAFETY: the C ABI contract documented above is a NUL-terminated string valid for this call;
    // only bytes up to the terminator are read.
    let value = unsafe { core::ffi::CStr::from_ptr(text) };
    let Ok(value) = value.to_str() else {
        return 0;
    };
    ime_bridge().set_marked_text(value, cursor_utf16, cursor_utf16);
    1
}

/// Reports a committed string from the ArkTS input method.
///
/// Returns `1` when accepted; the text reaches the focused widget as `Event::ImeCommit`.
#[no_mangle]
// See `rw_harmony_ime_marked_text`: the pointer is the C ABI, and null is rejected first.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn rw_harmony_ime_commit(text: *const core::ffi::c_char) -> i32 {
    if text.is_null() {
        return 0;
    }
    // SAFETY: see `rw_harmony_ime_marked_text`; the pointer is NUL-terminated and read only.
    let value = unsafe { core::ffi::CStr::from_ptr(text) };
    let Ok(value) = value.to_str() else {
        return 0;
    };
    ime_bridge().commit_text(value);
    1
}

/// Reports `window.getWindowProperties().orientation` as an Android-style rotation code.
///
/// The values are `0` portrait, `1` landscape-left, `2` portrait-upside-down and
/// `3` landscape-right; ArkTS hosts map their own enum onto these. Returns `1` when accepted.
#[no_mangle]
pub extern "C" fn rw_harmony_set_orientation(rotation: i32) -> i32 {
    let accepted = crate::platform::lifecycle::set_rotation(rotation);
    if accepted {
        bump();
    }
    i32::from(accepted)
}

/// Reports the ArkTS `Configuration.fontSizeScale`.
///
/// A non-finite or non-positive value is refused rather than handed to a layout that would
/// divide by it; the previous value stays.
#[no_mangle]
pub extern "C" fn rw_harmony_set_font_scale(scale: f32) -> i32 {
    if !scale.is_finite() || scale <= 0.0 {
        return 0;
    }
    FONT_SCALE_BITS.store(scale.to_bits(), Ordering::Relaxed);
    bump();
    1
}

/// Reports the merged safe-area insets in vp.
///
/// Negative inputs are clamped to zero. Once called, the fact is no longer `None` even when all
/// four values are zero: zero is a real measurement (a device with no system UI over the window).
#[no_mangle]
pub extern "C" fn rw_harmony_set_safe_area(top: i32, bottom: i32, left: i32, right: i32) -> i32 {
    SAFE_TOP.store(top.max(0) as u32, Ordering::Relaxed);
    SAFE_BOTTOM.store(bottom.max(0) as u32, Ordering::Relaxed);
    SAFE_LEFT.store(left.max(0) as u32, Ordering::Relaxed);
    SAFE_RIGHT.store(right.max(0) as u32, Ordering::Relaxed);
    SAFE_AREA_SET.store(true, Ordering::Relaxed);
    bump();
    1
}
