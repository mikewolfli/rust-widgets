//! The WASM gallery demo's application module, exposed as a library.
//!
//! `main` stays a stub (project convention). The browser has no `main`, so the
//! cdylib exports this one start function; it calls the same `app::run()` the
//! native stub does, which keeps a single construction path.

pub mod app;

/// Browser entry point.
///
/// `#[wasm_bindgen(start)]` runs this once after the module is instantiated, at
/// which point the page has already parsed the `<canvas id="wgpu-canvas">` the
/// backend binds to.
#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    app::run();
}
