//! Dual-engine render abstraction for native and embedded runtime paths.

use crate::core::RuntimeProfile;
use crate::platform::get_platform;

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
    fn create_button(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64;
}

/// Native desktop engine backed by platform adapters.
pub struct NativeRenderEngine;

impl NativeRenderEngine {
    /// Create native engine.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativeRenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

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

    fn create_button(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        get_platform().create_button(parent, text, x, y, width, height)
    }
}

/// Embedded engine that currently reuses platform stubs and keeps a separate profile identity.
pub struct EmbeddedRenderEngine;

impl EmbeddedRenderEngine {
    /// Create embedded engine.
    pub fn new() -> Self {
        Self
    }
}

impl Default for EmbeddedRenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderEngine for EmbeddedRenderEngine {
    fn name(&self) -> &'static str {
        "embedded-render-engine"
    }

    fn profile(&self) -> RuntimeProfile {
        RuntimeProfile::Embedded
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

    fn create_button(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        get_platform().create_button(parent, text, x, y, width, height)
    }
}

/// Build default engine for compile-time profile.
pub fn default_render_engine() -> Box<dyn RenderEngine> {
    if cfg!(feature = "embedded") {
        Box::new(EmbeddedRenderEngine::new())
    } else {
        Box::new(NativeRenderEngine::new())
    }
}
