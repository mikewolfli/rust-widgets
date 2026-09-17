// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Language bindings module — provides FFI, interop bridges, and
//! binding implementations for integrating with other languages.
//!
//! # Reachability
//!
//! **State:** Exposed over the C ABI (`rw_create_widget_of_kind`, `rw_destroy_widget`, `rw_widget_property_names`). The whole module exists to serve foreign-language callers, and `tools/check_abi.sh` gates the published header.
mod binding_impl;
pub use binding_impl::*;

#[cfg(feature = "jni")]
pub mod java_jni;
