// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Language bindings module — provides FFI, interop bridges, and
//! binding implementations for integrating with other languages.
mod binding_impl;
pub use binding_impl::*;

#[cfg(feature = "jni")]
pub mod java_jni;
