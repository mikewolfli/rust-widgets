// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unified control backend contract.
//!
//! This trait defines the full widget-creation and lifecycle surface. Core
//! methods (window, button, checkbox, etc.) are required; all non-core methods
//! have safe default implementations returning `0`, `false`, `None`, or `()`
//! so that minimal backends (e.g. embedded-mini) only need to override the
//! ~15 core widget methods they support.
//!
//! Helper traits and pattern implementations are also provided in
//! [`custom`](crate::control_backend::custom) and
//! [`native`](crate::control_backend::native) modules for common use cases.

// One trait cannot be spread over several files: Rust has no way to declare the
// items of a single trait in more than one module. The defaulted half of the
// body is therefore carried by `macro_rules!` fragments in `defaults` and
// `defaults_catalog`, invoked from `core` at the position each run of methods
// originally occupied.
mod core;
mod defaults;
mod defaults_catalog;

pub use core::ControlBackend;
