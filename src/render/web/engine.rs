//! `WebEngine` rendering wrapper that delegates to the main web engine module.
//!
//! This module provides a thin rendering-oriented wrapper around the full
//! [`WebEngineViewEnhanced`](crate::web::WebEngineViewEnhanced) widget. It is
//! used by the rendering pipeline to interact with a web engine view without
//! pulling in the entire widget implementation.

use crate::core::Rect;
use crate::web::WebEngineViewEnhanced;

/// Rendering wrapper for a web engine view.
///
/// Owns an inner [`WebEngineViewEnhanced`] and exposes the subset of its API
/// that the rendering layer cares about (navigation, loading, basic querying).
pub struct WebEngine {
    inner: WebEngineViewEnhanced,
}

impl WebEngine {
    /// Create a new `WebEngine` with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self { inner: WebEngineViewEnhanced::new(geometry) }
    }

    /// Access the inner widget by reference.
    pub fn inner(&self) -> &WebEngineViewEnhanced {
        &self.inner
    }

    /// Access the inner widget by mutable reference.
    pub fn inner_mut(&mut self) -> &mut WebEngineViewEnhanced {
        &mut self.inner
    }

    /// Load a URL in the engine.
    pub fn load_url(&mut self, url: &str) {
        self.inner.load_url(url);
    }

    /// Load HTML content, optionally with a base URL.
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        self.inner.load_html(html, base_url);
    }

    /// Navigate back in history, if possible.
    pub fn go_back(&mut self) {
        self.inner.go_back();
    }

    /// Navigate forward in history, if possible.
    pub fn go_forward(&mut self) {
        self.inner.go_forward();
    }

    /// Reload the current page.
    pub fn reload(&mut self) {
        self.inner.reload();
    }

    /// Stop the current page load.
    pub fn stop(&mut self) {
        self.inner.stop();
    }

    /// The currently loaded URL.
    pub fn url(&self) -> &str {
        self.inner.url()
    }

    /// Whether the engine is currently loading a page.
    pub fn is_loading(&self) -> bool {
        self.inner.is_loading()
    }

    /// The current page title.
    pub fn title(&self) -> &str {
        self.inner.title()
    }

    /// Current load progress (0–100).
    pub fn load_progress(&self) -> u8 {
        self.inner.load_progress()
    }

    /// Whether backward navigation is available.
    pub fn can_go_back(&self) -> bool {
        self.inner.can_go_back()
    }

    /// Whether forward navigation is available.
    pub fn can_go_forward(&self) -> bool {
        self.inner.can_go_forward()
    }
}
