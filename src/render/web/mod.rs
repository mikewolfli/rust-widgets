//! Web rendering: web engine view integration.
//!
//! Exposes `WebEngine` (rendering wrapper around `WebEngineViewEnhanced`)
//! and `WebView` (display adapter) for use by the rendering pipeline.
//!
//! ## Future wiring
//! These types are wired into `render::pipeline::special` for web widget
//! rendering. They are intentionally kept as a separate module to isolate
//! the web rendering dependency from the core render pipeline.
pub mod engine;
pub mod view;

#[cfg(not(feature = "desktop"))]
compile_error!("render::web requires the `desktop` feature");
