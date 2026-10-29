// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unified control backend contract — split into trait definition and mock implementations.
//!
//! This module is split from the original monolithic `trait_def.rs` (1730 lines, BLUE11 R9.1):
//! - `trait_def/` — the `ControlBackend` trait definition, in `core.rs` (declaration and
//!   required methods) and `defaults*.rs` (the defaulted bodies)
//! - `mock.rs` — test mock (`TestBackend`) and associated unit tests

#[cfg(test)]
mod mock;
// BLUE11: Allow module inception; `trait_def` is the trait definition sub-module
// inside `trait_def/` parent directory.
#[allow(clippy::module_inception)]
mod trait_def;

pub use trait_def::ControlBackend;
