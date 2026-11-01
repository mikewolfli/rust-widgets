// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `ANativeWindow*` the host registered for each window, and its reference accounting.
//!
//! # Why one process-wide store and not a field on the backend
//!
//! The pointer is produced by the JNI entry point that receives the Java `Surface`, and that
//! entry has no platform instance to reach. A field would therefore have to be written by a second
//! path that could drift from the reader (principle #101); one store with two readers cannot. A
//! surface belongs to the thread that owns the view hierarchy, so a thread-local is the same scope
//! Android itself gives it.
//!
//! # Ownership
//!
//! `ANativeWindow_fromSurface` returns a **new reference** the caller owns, and
//! `ANativeWindow_release` is the matching drop. Registration therefore takes ownership of exactly
//! one reference and `clear` releases it — without that, a host that recreates its surface on
//! every rotation leaks one native window per rotation.

use crate::compat::{HashMap, RefCell};

thread_local! {
    /// Window id -> the `ANativeWindow*` address the host registered for it.
    static NATIVE_WINDOWS: RefCell<HashMap<u64, usize>> = RefCell::new(HashMap::new());
}

/// Records the `ANativeWindow*` address for `window`.
///
/// Refuses a null pointer or a zero window id: neither can name a target, and recording one would
/// make `peek` report a surface that cannot be presented into.
#[cfg(feature = "android-jni")]
pub(crate) fn register(window: u64, pointer: usize) -> bool {
    if window == 0 || pointer == 0 {
        return false;
    }
    NATIVE_WINDOWS.with(|map| {
        map.borrow_mut().insert(window, pointer);
    });
    true
}

/// Forgets and releases the pointer recorded for `window`.
///
/// Returns whether one was recorded.
#[cfg(feature = "android-jni")]
pub(crate) fn clear(window: u64) -> bool {
    let removed = NATIVE_WINDOWS.with(|map| map.borrow_mut().remove(&window));
    match removed {
        Some(pointer) => {
            super::release_native_window(pointer);
            true
        }
        None => false,
    }
}

/// The address recorded for `window`, if any.
///
/// Read by [`AndroidPlatform::raw_window_handle`](super::platform_impl) when the
/// `raw-window-handle` feature is on; the writer is the `android-jni` bridge.
#[cfg(feature = "raw-window-handle")]
pub(crate) fn peek(window: u64) -> Option<usize> {
    NATIVE_WINDOWS.with(|map| map.borrow().get(&window).copied())
}

#[cfg(all(test, feature = "android-jni"))]
mod tests {
    use super::{clear, peek, register};

    /// A host build has no `ANativeWindow`, so the tests exercise the registry itself: a pointer
    /// that is never dereferenced, released by a no-op on this target.
    #[test]
    fn a_registered_pointer_is_readable_and_clear_releases_it_once() {
        let window = 9_001;
        assert!(register(window, 0x1000));
        assert_eq!(peek(window), Some(0x1000));
        assert!(clear(window), "the first clear releases the registration");
        assert!(!clear(window), "a second clear has nothing to release");
        #[cfg(feature = "raw-window-handle")]
        assert_eq!(peek(window), None);
    }

    #[test]
    fn a_null_pointer_or_zero_window_is_refused() {
        assert!(!register(0, 0x1000));
        assert!(!register(9_002, 0));
        #[cfg(feature = "raw-window-handle")]
        assert_eq!(peek(9_002), None);
    }
}

/// Releases one reference to an `ANativeWindow` the host handed the library.
///
/// # Ownership contract
///
/// `ANativeWindow_fromSurface` returns a **new reference** the caller owns; the NDK documents
/// `ANativeWindow_release` as the matching drop.
/// [`surface_registry::register`] therefore takes ownership of exactly one reference, and
/// [`surface_registry::clear`] releases it — that is what keeps a host that recreates its surface
/// on every rotation from leaking one window per rotation.
///
/// Off an Android target there is no such symbol and nothing to release: the registration path
/// refuses a null pointer, and a non-Android host build cannot produce a real `ANativeWindow`.
#[cfg(feature = "android-jni")]
pub(crate) fn release_native_window(pointer: usize) {
    #[cfg(target_os = "android")]
    {
        if pointer != 0 {
            // SAFETY: the pointer came from `ANativeWindow_fromSurface`, which transferred one
            // reference to this library (see the ownership contract above). The caller of
            // `clear_native_window` has already removed it from the map, so no second release
            // can reach the same reference, and `ANativeWindow_release` only decrements.
            unsafe { an_ndk::ANativeWindow_release(pointer as *mut core::ffi::c_void) }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = pointer;
    }
}

/// The NDK surface-window entry points this module calls.
///
/// Declared here rather than pulled in as an NDK binding crate: the module needs exactly two
/// calls, and the crate deliberately carries no binding generator (the same reasoning as the
/// OpenHarmony `XComponent` bridge).
#[cfg(all(feature = "android-jni", target_os = "android"))]
mod an_ndk {
    extern "C" {
        /// `android/native_window_jni.h`: takes a reference from a Java `Surface`.
        ///
        /// Both parameters are opaque JNI pointers; declaring them as `c_void` keeps this module
        /// independent of the `jni` crate, which is an optional dependency (a build without
        /// `android-jni` must still compile this file).
        pub(super) fn ANativeWindow_fromSurface(
            env: *mut core::ffi::c_void,
            surface: *mut core::ffi::c_void,
        ) -> *mut core::ffi::c_void;
        /// `android/native_window.h`: drops one reference to a native window.
        pub(super) fn ANativeWindow_release(window: *mut core::ffi::c_void);
    }
}

/// Converts a Java `Surface` into an `ANativeWindow*` and returns it as an address.
///
/// Returns `0` when the JNI environment or the surface is null, and off an Android target.
/// The returned address carries the **one owned reference** described by
/// [`release_native_window`].
#[cfg(feature = "android-jni")]
pub(crate) fn native_window_from_surface(
    env: *mut jni::sys::JNIEnv,
    surface: jni::sys::jobject,
) -> usize {
    #[cfg(target_os = "android")]
    {
        if env.is_null() || surface.is_null() {
            return 0;
        }
        // SAFETY: both pointers come from the JVM's own call frame; the NDK call only reads
        // them and transfers one native-window reference to us. They are passed as opaque
        // pointers because the NDK signature does not care which JNI wrapper type produced them.
        unsafe {
            an_ndk::ANativeWindow_fromSurface(
                env as *mut core::ffi::c_void,
                surface as *mut core::ffi::c_void,
            ) as usize
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (env, surface);
        0
    }
}
