// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android platform backend.
//!
//! This module provides a state-driven implementation of the `Platform` trait
//! for Android, with optional JNI integration gated behind the `android-jni`
//! feature flag.

pub mod platform_impl;
pub mod surface_registry;
pub mod types;

pub use types::AndroidPlatform;

// The `ANativeWindow*` registry. Re-exported here because the JNI bridge and the platform impl are
// in different siblings; the ownership contract lives in `surface_registry`'s module docs.
#[cfg(feature = "raw-window-handle")]
pub(crate) use surface_registry::peek as native_window;
#[cfg(feature = "android-jni")]
pub(crate) use surface_registry::{
    clear as clear_native_window, register as register_native_window,
};
#[cfg(feature = "android-jni")]
pub(crate) use surface_registry::{native_window_from_surface, release_native_window};
