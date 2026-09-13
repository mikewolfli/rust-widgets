// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android platform backend.
//!
//! This module provides a state-driven implementation of the `Platform` trait
//! for Android, with optional JNI integration gated behind the `android-jni`
//! feature flag.

pub mod platform_impl;
pub mod types;

pub use types::AndroidPlatform;
