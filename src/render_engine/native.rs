//! Native desktop render engine backed by platform adapters.

use super::engine_trait::RenderEngine;
use crate::core::RuntimeProfile;
#[cfg(not(feature = "mini"))]
use crate::platform::get_platform;

/// Native desktop engine backed by platform adapters.
pub struct NativeRenderEngine;

impl NativeRenderEngine {
    /// Create native engine.
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NativeRenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "mini"))]
impl RenderEngine for NativeRenderEngine {
    fn name(&self) -> &'static str {
        "native-render-engine"
    }

    fn profile(&self) -> RuntimeProfile {
        RuntimeProfile::Full
    }

    fn init(&self) {
        get_platform().init();
    }

    fn run(&self) {
        get_platform().run();
    }

    fn quit(&self) {
        get_platform().quit();
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        get_platform().create_window(title, x, y, width, height)
    }

    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        get_platform().create_button(parent, text, x, y, width, height)
    }
}
