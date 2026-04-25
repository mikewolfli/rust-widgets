//! Web rendering: web engine view integration.
//!
//! Exposes `WebEngine` (rendering wrapper around `WebEngineViewEnhanced`)
//! and `WebView` (display adapter) for use by the rendering pipeline.
#![allow(dead_code)]
pub mod engine;
pub mod view;
