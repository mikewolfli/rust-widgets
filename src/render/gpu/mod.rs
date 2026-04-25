//! GPU-accelerated rendering backend (WGPU).
//!
//! `wgpu_backend` at the crate root is the canonical implementation.
//! This module re-exports its types so that `crate::render::gpu::*` resolves
//! to the same symbols regardless of whether the caller goes through `render::gpu`
//! or uses the crate-local path directly.

/// GPU rendering capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuCapability {
    /// Basic 2D rendering
    Basic2D,
    /// Texture support
    Texture,
    /// Shader support
    Shader,
    /// Compute shaders
    Compute,
    /// Instanced rendering
    Instancing,
    /// Anti-aliasing
    AntiAliasing,
}

/// GPU renderer trait.
pub trait GpuRenderer {
    /// Initializes the GPU renderer.
    fn initialize(&mut self) -> Result<(), String>;
    /// Returns available GPU capabilities.
    fn capabilities(&self) -> &[GpuCapability];
    /// Checks if a specific capability is available.
    fn has_capability(&self, capability: GpuCapability) -> bool;
    /// Renders a frame.
    fn render_frame(&mut self) -> Result<(), String>;
    /// Returns GPU memory usage in bytes.
    fn memory_usage(&self) -> u64;
    /// Returns GPU vendor information.
    fn vendor_info(&self) -> &str;
}
