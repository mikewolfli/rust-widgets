//! Native WebKitGTK web engine for the Linux backend.
//!
//! This is the only place in the crate that names `webkit2gtk`. Widgets in
//! `src/web/` drive a real browser engine through
//! [`NativeWebEngine`](crate::platform::types::NativeWebEngine) and never import
//! the platform crate — see principle #36.
//!
//! Compiled only for `target_os = "linux"` with the `webkit-engine` feature;
//! every other backend inherits the trait's `None` default, so `src/web/` falls
//! back to simulated navigation.

use crate::platform::types::NativeWebEngine;
use webkit2gtk::WebViewExt;

/// A `webkit2gtk::WebView` owned by the platform backend.
pub struct WebKitEngine {
    webview: webkit2gtk::WebView,
}

// WebKitGTK widgets are reference-counted GObjects on a single UI thread. The
// handle is `Send` so it can be stored inside the `WebEngineViewEnhanced` widget
// struct, which is moved between the constructor and the event loop. It must not
// be shared across threads (`Sync` is deliberately not implemented): all
// `webkit2gtk` calls must happen on the GTK main thread.
unsafe impl Send for WebKitEngine {}

impl WebKitEngine {
    /// Creates the engine, or `None` when GTK is not initialised or no display is
    /// reachable (the common headless-CI case).
    ///
    /// `webkit2gtk::WebView::new()` asserts that GTK is initialised and panics
    /// otherwise, so the call is wrapped in `catch_unwind` and the panic is
    /// converted into a clean `None` rather than aborting the host process.
    pub fn new() -> Option<Self> {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(webkit2gtk::WebView::new));
        match result {
            Ok(webview) => Some(Self { webview }),
            Err(_) => {
                log::warn!("[linux] WebKitEngine unavailable: GTK not initialised or no display");
                None
            }
        }
    }
}

impl NativeWebEngine for WebKitEngine {
    fn load_url(&mut self, url: &str) -> Result<(), String> {
        self.webview.load_uri(url);
        Ok(())
    }

    fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<(), String> {
        self.webview.load_html(html, base_url);
        Ok(())
    }

    fn go_back(&mut self) {
        self.webview.go_back();
    }

    fn go_forward(&mut self) {
        self.webview.go_forward();
    }

    fn reload(&mut self) {
        self.webview.reload();
    }

    fn stop_loading(&mut self) {
        self.webview.stop_loading();
    }
}
