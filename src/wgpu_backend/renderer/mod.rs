// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WGPU renderer implementation.
//!
//! Split by responsibility: `resources` owns the device resources kept across calls, `frame` owns
//! the renderer itself together with its readback plumbing, and `encode` owns the 2D and mesh
//! encoders.
mod encode;
mod frame;
mod resources;

pub(crate) use super::shaders;
pub use frame::WgpuRenderer;

#[cfg(test)]
mod tests;
