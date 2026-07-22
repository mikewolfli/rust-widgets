//! GPU-accelerated rendering backend (WGPU).
//!
//! `wgpu_backend` at the crate root is the canonical implementation.
//! This module re-exports its types so that `crate::render::gpu::*` resolves
//! to the same symbols regardless of whether the caller goes through `render::gpu`
//! or uses the crate-local path directly.

pub mod gpu_types;

pub use gpu_types::{GpuCapability, GpuRenderer};
