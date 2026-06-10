//! Android platform backend.
//!
//! This module provides a state-driven implementation of the `Platform` trait
//! for Android, with optional JNI integration gated behind the `android-jni`
//! feature flag.

pub mod platform_impl;
pub mod types;

pub use types::AndroidPlatform;
