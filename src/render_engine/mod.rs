// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Render engine module providing native and embedded rendering backends.
//!
//! This module contains the engine trait contract (`EngineTrait`) and concrete
//! implementations for both native desktop rendering and embedded (lightweight)
//! rendering with independent lifecycle management.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/platform/profile.rs:38` (`use crate::render_engine::RenderEngine`), `src/bindings/binding_impl.rs:1465` (`crate::render_engine::set_embedded_target_fps`).
/// Embedded runtime state, task queue, and shared engine internals.
pub mod embedded;
/// Embedded render engine with independent lifecycle and resource registry.
pub mod embedded_engine;
/// Render engine trait — unified contract for native and embedded engines.
pub mod engine_trait;
/// Native desktop render engine backed by platform adapters.
pub mod native;
// Re-exports
pub use embedded::*;
pub use embedded_engine::*;
pub use engine_trait::*;
pub use native::*;
