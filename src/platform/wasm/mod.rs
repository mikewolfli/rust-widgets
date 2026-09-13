// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WASM/WebAssembly platform backend.
//!
//! This backend uses a state-driven model backed by `BackendState<WasmHandleKind>`
//! for widget lifecycle. On `target_arch = "wasm32"` the event loop is driven by
//! `request_animation_frame` to avoid blocking the browser. On non-WASM targets
//! a simple polling fallback is used for development / testing.

pub mod platform_impl;
pub mod types;

pub use types::*;
