// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WGPU renderer implementation.
//!
//! Split by responsibility: `resources` owns the device resources kept across calls, `frame` owns
//! the renderer itself together with its readback plumbing, `shadow` owns the blurred-shadow
//! layer passes, and `encode` owns the 2D and mesh encoders.
mod dispatch;
mod effects;
mod encode;
mod frame;
mod pass;
mod resources;
mod shadow;
mod stream;
mod uploads;

pub(crate) use super::shaders;
pub use frame::WgpuRenderer;

#[cfg(test)]
mod tests;
