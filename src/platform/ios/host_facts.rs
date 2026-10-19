// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Host-reported iOS lifecycle and orientation (BLUE28 B28.24).
//!
//! UIKit delivers these through the scene delegate (`sceneWillResignActive`,
//! `sceneDidBecomeActive`, `viewWillTransitionToSize:`) and the C entry points below are what a
//! host calls from those callbacks. The library cannot observe them itself: it does not own the
//! scene, and the state-only backend deliberately has no `UIApplicationDelegate`.
//!
//! The values land in [`crate::platform::lifecycle`], which is also where the animation clock is
//! stopped and restarted, so an iOS host and an Android host produce the same behaviour.
//!
//! ## Status codes
//!
//! * lifecycle: `0` active, `1` inactive, `2` background, anything else is refused;
//! * orientation: `1..=4` in UIKit's `UIInterfaceOrientation` order (portrait,
//!   portrait-upside-down, landscape-left, landscape-right), anything else is refused.
//!
//! A refused value returns `0` and leaves the previous state untouched.

use crate::platform::lifecycle;
use crate::platform::types::{MobileLifecycleState, ScreenOrientation};
use std::sync::OnceLock;

static IME_BRIDGE: OnceLock<crate::platform::ime_macos::MacOsImeBridge> = OnceLock::new();

/// The one IME state machine the iOS backend exposes (BLUE28 B28.12).
///
/// The platform-independent `ime_macos` state machine is reused unchanged: its AppKit calls are
/// gated on macOS, and on iOS it is exactly the marked-text/commit bookkeeping a host forwards.
pub(crate) fn ime_bridge() -> &'static crate::platform::ime_macos::MacOsImeBridge {
    IME_BRIDGE.get_or_init(crate::platform::ime_macos::MacOsImeBridge::new)
}

/// Reports a marked-text update from a host `UITextInput` (BLUE28 B28.12).
///
/// `text` is a NUL-terminated UTF-8 composition; `cursor_utf16` is the selection start in UTF-16
/// code units within it, or `-1` to place the cursor at the end. Returns `1` when accepted, `0`
/// for a null or non-UTF-8 string.
#[no_mangle]
// The pointer is part of the C ABI this function exists to export; null is rejected before the
// dereference, and the caller owns the string. Marking the export `unsafe` would not make a C
// caller safer, so the lint is allowed here with that contract stated.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn rw_ios_ime_marked_text(text: *const core::ffi::c_char, cursor_utf16: i32) -> i32 {
    if text.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees a NUL-terminated C string for the duration of this call, which
    // is the C ABI contract documented above; only the bytes up to the terminator are read.
    let value = unsafe { core::ffi::CStr::from_ptr(text) };
    let Ok(value) = value.to_str() else {
        return 0;
    };
    ime_bridge().set_marked_text(value, cursor_utf16, cursor_utf16);
    1
}

/// Reports a committed string from the host's text input (BLUE28 B28.12).
///
/// The text is delivered to the focused widget as `Event::ImeCommit`, which is the same path the
/// Android and desktop bridges use. Returns `1` when accepted, `0` for a null/non-UTF-8 string.
#[no_mangle]
// See `rw_ios_ime_marked_text`: the pointer is the C ABI, and null is rejected first.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn rw_ios_ime_commit(text: *const core::ffi::c_char) -> i32 {
    if text.is_null() {
        return 0;
    }
    // SAFETY: see `rw_ios_ime_marked_text`; the pointer is NUL-terminated and read only.
    let value = unsafe { core::ffi::CStr::from_ptr(text) };
    let Ok(value) = value.to_str() else {
        return 0;
    };
    ime_bridge().commit_text(value);
    1
}

/// Reports a scene lifecycle transition.
#[no_mangle]
pub extern "C" fn rw_ios_set_lifecycle(state: i32) -> i32 {
    let phase = match state {
        0 => MobileLifecycleState::Active,
        1 => MobileLifecycleState::Inactive,
        2 => MobileLifecycleState::Background,
        _ => return 0,
    };
    i32::from(lifecycle::set_state(phase))
}

/// Reports `UIInterfaceOrientation` (UIKit's own numbering).
#[no_mangle]
pub extern "C" fn rw_ios_set_orientation(orientation: i32) -> i32 {
    let value = match orientation {
        1 => ScreenOrientation::Portrait,
        2 => ScreenOrientation::PortraitUpsideDown,
        3 => ScreenOrientation::LandscapeLeft,
        4 => ScreenOrientation::LandscapeRight,
        _ => return 0,
    };
    i32::from(lifecycle::set_orientation(value))
}

/// Reports whether the host app is in the foreground (`1`) or background (`0`).
#[no_mangle]
pub extern "C" fn rw_ios_set_active(active: i32) -> i32 {
    rw_ios_set_lifecycle(if active != 0 { 0 } else { 2 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_lifecycle_code_is_refused() {
        let _guard = crate::compat::lock(&crate::platform::lifecycle::TEST_LOCK);
        assert_eq!(rw_ios_set_lifecycle(9), 0);
    }

    #[test]
    fn each_known_lifecycle_code_is_accepted() {
        let _guard = crate::compat::lock(&crate::platform::lifecycle::TEST_LOCK);
        assert_eq!(rw_ios_set_lifecycle(0), 1);
        assert_eq!(lifecycle::lifecycle_state(), Some(MobileLifecycleState::Active));
        assert_eq!(rw_ios_set_lifecycle(1), 1);
        assert_eq!(lifecycle::lifecycle_state(), Some(MobileLifecycleState::Inactive));
        assert_eq!(rw_ios_set_lifecycle(2), 1);
        assert_eq!(lifecycle::lifecycle_state(), Some(MobileLifecycleState::Background));
        // Leave the process active for other tests.
        let _ = rw_ios_set_lifecycle(0);
    }

    #[test]
    fn the_four_uikit_orientations_map_to_the_four_screen_orientations() {
        let _guard = crate::compat::lock(&crate::platform::lifecycle::TEST_LOCK);
        assert_eq!(rw_ios_set_orientation(1), 1);
        assert_eq!(lifecycle::orientation(), Some(ScreenOrientation::Portrait));
        assert_eq!(rw_ios_set_orientation(2), 1);
        assert_eq!(lifecycle::orientation(), Some(ScreenOrientation::PortraitUpsideDown));
        assert_eq!(rw_ios_set_orientation(3), 1);
        assert_eq!(lifecycle::orientation(), Some(ScreenOrientation::LandscapeLeft));
        assert_eq!(rw_ios_set_orientation(4), 1);
        assert_eq!(lifecycle::orientation(), Some(ScreenOrientation::LandscapeRight));
        assert_eq!(rw_ios_set_orientation(5), 0);
    }

    #[test]
    fn the_ime_hooks_accept_utf8_and_refuse_null() {
        let _guard = crate::compat::lock(&crate::platform::lifecycle::TEST_LOCK);
        let text = std::ffi::CString::new("にほ").expect("no interior nul");
        assert_eq!(rw_ios_ime_marked_text(text.as_ptr(), 0), 1);
        assert!(ime_bridge().has_marked_text());
        assert_eq!(rw_ios_ime_commit(text.as_ptr()), 1);
        assert!(!ime_bridge().has_marked_text(), "a commit clears the composition");
        assert_eq!(rw_ios_ime_marked_text(std::ptr::null(), 0), 0);
        assert_eq!(rw_ios_ime_commit(std::ptr::null()), 0);
    }
}
