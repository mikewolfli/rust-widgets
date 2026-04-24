//! Render engine trait — unified contract for native and embedded engines.

use crate::core::RuntimeProfile;

/// Unified rendering/runtime engine abstraction.
pub trait RenderEngine: Send + Sync {
    /// Engine display name.
    fn name(&self) -> &'static str;
    /// Runtime profile category.
    fn profile(&self) -> RuntimeProfile;
    /// Initialize engine resources.
    fn init(&self);
    /// Run engine event loop.
    fn run(&self);
    /// Request event loop shutdown.
    fn quit(&self);
    /// Create a top-level window.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64;
    /// Create a button control.
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64;
}
