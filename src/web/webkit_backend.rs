//! WebKitGTK backend for real web engine support (Linux-only).
//!
//! This module provides a [`WebKitBackend`] that wraps a `webkit2gtk::WebView`
//! and implements the [`SimulationEngine`] trait from [`web_core`](super::web_core).
//!
//! All items are gated behind `#[cfg(all(feature = "webkit-engine", target_os = "linux"))]`.
//! On macOS/Windows, the webkit-engine feature is a no-op since webkit2gtk
//! is a Linux-only library. The fallback simulation is always available.

use super::web_core::SimulationEngine;
use webkit2gtk::WebViewExt;

/// Real WebKitGTK backend that wraps a `webkit2gtk::WebView`.
///
/// Created by [`WebKitBackend::new()`]. If GTK/WebKit is not available or
/// initialization fails, returns an error so the caller can fall back to
/// the simulated engine.
pub struct WebKitBackend {
    webview: webkit2gtk::WebView,
}

// WebKitGTK widgets are thread-safe (glib Send wrapper)
unsafe impl Send for WebKitBackend {}

impl WebKitBackend {
    /// Create a new WebKit backend.
    ///
    /// Returns `Err` if GTK is not initialized or the `webkit2gtk::WebView`
    /// cannot be created (e.g. no display available or WebKit runtime
    /// not installed).
    pub fn new() -> Result<Self, String> {
        // Attempt to create a WebKit WebView, returning Err gracefully if
        // GTK is not initialised or no display is available (common in
        // test/headless environments).
        //
        // We wrap the call in std::panic::catch_unwind because
        // webkit2gtk's WebView::new() asserts GTK is initialised and
        // panics otherwise.
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| webkit2gtk::WebView::new()));
        match result {
            Ok(webview) => Ok(Self { webview }),
            Err(_) => Err("GTK not initialized or no display available".to_string()),
        }
    }

    /// Load the given URL in the WebKit view.
    pub fn load_url(&mut self, url: &str) -> Result<(), String> {
        self.webview.load_uri(url);
        Ok(())
    }

    /// Load HTML content with an optional base URL.
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<(), String> {
        self.webview.load_html(html, base_url);
        Ok(())
    }

    /// Navigate back in the session history.
    pub fn go_back(&mut self) {
        self.webview.go_back();
    }

    /// Navigate forward in the session history.
    pub fn go_forward(&mut self) {
        self.webview.go_forward();
    }

    /// Reload the current page.
    pub fn reload(&mut self) {
        self.webview.reload();
    }

    /// Stop any ongoing page load.
    pub fn stop_loading(&mut self) {
        self.webview.stop_loading();
    }
}

impl SimulationEngine for WebKitBackend {
    /// Simulate navigation by delegating to the real WebKit view.
    ///
    /// Since real page loads are asynchronous, this method returns an
    /// `Ok` with a descriptive message rather than blocking for content.
    fn simulate_navigation(&mut self, url: &str) -> Result<String, String> {
        self.load_url(url)?;
        Ok(format!("WebKit navigation started to: {}", url))
    }
}
