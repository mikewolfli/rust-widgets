// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WebEngineViewEnhanced widget — a web engine view wrapper.
//!
//! This type shares ~95% of its implementation with [`WebViewEnhanced`](super::web_view::WebViewEnhanced).
//! Both delegate to a common [`WebViewCore`] to avoid code duplication.
//!
//! **Unique to this type:**
//! - `WidgetKind::WebEngineView`
//! - Additional signals: `certificate_error`, `download_requested`
//! - Additional methods: `set_plugins_enabled`, `set_private_browsing`

use super::js_engine::{JsResult, JsValue};
use super::web_core::{delegate_widget, WebViewCore};
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::platform::types::NativeWebEngine;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{Widget, WidgetKind};

/// Enhanced web engine view widget.
///
/// When the active platform backend can host a real web engine
/// (`Platform::create_web_engine`), navigation delegates to it; otherwise the
/// simulated 0→50→100 progress callbacks are used. The engine is obtained at
/// construction and held behind the platform-neutral [`NativeWebEngine`] trait,
/// so this widget names no platform crate — see principle #36.
pub struct WebEngineViewEnhanced {
    core: WebViewCore,
    /// Emitted when the engine rejects a site certificate, carrying the
    /// description of the failure. Nothing in this widget emits it yet; it exists
    /// for backends that surface a certificate callback.
    pub certificate_error: Signal1<String>,
    /// Emitted when a navigation asks to download rather than to display,
    /// carrying the URL of the download. Nothing in this widget emits it yet.
    pub download_requested: Signal1<String>,
    /// Real engine when the backend provides one, `None` for the simulated path.
    webkit_backend: Option<Box<dyn NativeWebEngine>>,
}

impl WebEngineViewEnhanced {
    /// Creates an engine view in `geometry` with an empty URL, no title, and
    /// nothing loading.
    ///
    /// The active backend is asked for a native web engine at this point: if
    /// [`Platform::create_web_engine`](crate::platform::Platform::create_web_engine)
    /// returns one, navigation is forwarded to it; if it returns `None` — a
    /// headless host, for instance — the widget degrades to the simulated path
    /// rather than failing. Which of the two happened is not exposed, so a caller
    /// that must know should query the platform directly.
    pub fn new(geometry: Rect) -> Self {
        // Ask the active backend rather than testing `cfg(target_os)`: a headless
        // Linux host returns `None` here and the widget degrades to simulation.
        let webkit_backend = crate::platform::platform_facts().create_web_engine();

        Self {
            core: WebViewCore::new(WidgetKind::WebEngineView, geometry, "WebEngineView", ""),
            certificate_error: Signal1::new(),
            download_requested: Signal1::new(),
            webkit_backend,
        }
    }

    // -- Accessors that delegate to core --

    /// The address currently shown, or `""` until something is loaded.
    ///
    /// Read back from the core even when a native engine is driving, so it
    /// reflects what this widget asked the engine to load rather than what the
    /// engine ended up at after redirects.
    pub fn url(&self) -> &str {
        self.core.url()
    }
    /// Whether a navigation is in flight.
    ///
    /// On the simulated path a load completes within the call, so this is `false`
    /// again by the time the loader returns; treat it as meaningful only while a
    /// native engine is driving the load.
    pub fn is_loading(&self) -> bool {
        self.core.is_loading()
    }
    /// The page title, or `""` when none has been set or extracted.
    pub fn title(&self) -> &str {
        self.core.title()
    }
    /// Load completion as a percentage, `0`..=`100`.
    ///
    /// 100 means the last navigation finished; it says nothing about whether the
    /// document was valid.
    pub fn load_progress(&self) -> u8 {
        self.core.load_progress()
    }
    /// Whether the session history has an entry behind the current one.
    pub fn can_go_back(&self) -> bool {
        self.core.can_go_back()
    }
    /// Whether the session history has an entry ahead of the current one.
    pub fn can_go_forward(&self) -> bool {
        self.core.can_go_forward()
    }
    /// The engine preferences in force.
    pub fn settings(&self) -> &super::WebSettings {
        self.core.settings()
    }
    /// The engine preferences in force, mutably, for changing several at once
    /// without going through a setter per field.
    pub fn settings_mut(&mut self) -> &mut super::WebSettings {
        self.core.settings_mut()
    }
    /// The security preferences in force.
    pub fn security(&self) -> &super::SecuritySettings {
        self.core.security()
    }
    /// The security preferences in force, mutably.
    pub fn security_mut(&mut self) -> &mut super::SecuritySettings {
        self.core.security_mut()
    }
    /// This view's cookie jar.
    pub fn cookies(&self) -> &super::privacy::CookieJar {
        self.core.cookies()
    }
    /// This view's cookie jar, mutably.
    pub fn cookies_mut(&mut self) -> &mut super::privacy::CookieJar {
        self.core.cookies_mut()
    }
    /// The tracking-protection state, including the blocked-request count.
    pub fn privacy(&self) -> &super::privacy::TrackingProtection {
        self.core.privacy()
    }
    /// The tracking-protection state, mutably.
    pub fn privacy_mut(&mut self) -> &mut super::privacy::TrackingProtection {
        self.core.privacy_mut()
    }
    /// The registered plugins.
    pub fn plugins(&self) -> &super::plugins::PluginManager {
        self.core.plugins()
    }
    /// The registered plugins, mutably.
    pub fn plugins_mut(&mut self) -> &mut super::plugins::PluginManager {
        self.core.plugins_mut()
    }
    /// Session history for the back/forward buttons.
    pub fn history(&self) -> &super::history::SessionHistory {
        self.core.history()
    }
    /// The longer-term browsing history, distinct from [`Self::history`].
    pub fn browser_history(&self) -> &super::history::BrowserHistory {
        self.core.browser_history()
    }

    // -- Methods that delegate to core --

    /// Navigates to `url`.
    ///
    /// With a native engine present the load is handed to it, and a failure is
    /// logged and then ignored — the previous page stays on screen and this
    /// method still returns `()`, so a caller that must react to a failed load
    /// should watch the engine's own error signal instead. Without one, this
    /// delegates to the simulated loader (see [`Self::set_url`] for that
    /// contract).
    pub fn load_url(&mut self, url: &str) {
        if let Some(ref mut backend) = self.webkit_backend {
            if let Err(error) = backend.load_url(url) {
                log::warn!("[web] native engine load_url failed: {error}");
            }
            return;
        }
        self.core.load_url(url);
    }
    /// Navigates to `url`, which must begin with `http://`, `https://` or
    /// `file://`.
    ///
    /// Simulated path (no native engine): a URL with an unrecognised scheme is
    /// logged and rejected, leaving the view untouched and returning silently;
    /// navigating to the URL already displayed just marks the load complete at
    /// 100%.
    ///
    /// Native path: the load is handed to the engine (failures logged, not
    /// reported to the caller) *and* the core state is updated too, so the URL,
    /// title and history stay in step with what the engine was asked to show.
    pub fn set_url(&mut self, url: String) {
        if let Some(ref mut backend) = self.webkit_backend {
            if let Err(error) = backend.load_url(&url) {
                log::warn!("[web] native engine load_url failed: {error}");
            }
            self.core.set_url(url);
            return;
        }
        self.core.set_url(url);
    }
    /// Loads `html` as the document, with `base_url` as the address it is
    /// considered to have come from — used to resolve relative links and, when
    /// `None`, replaced with `"data:text/html"`.
    ///
    /// The title becomes `"HTML Content"` and the body is stored verbatim: no
    /// parsing, scripting or sanitising happens, so this is a way to display
    /// markup, not to run a page.
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        if let Some(ref mut backend) = self.webkit_backend {
            if let Err(error) = backend.load_html(html, base_url) {
                log::warn!("[web] native engine load_html failed: {error}");
            }
            self.core.load_html(html, base_url);
            return;
        }
        self.core.load_html(html, base_url);
    }
    /// Loads `data` as the document at `base_url`, declaring the bytes to be of
    /// type `mime_type` (the title becomes `"Data: <mime_type>"`).
    ///
    /// The engine trait has no native equivalent, so this always takes the
    /// simulated path even when a native engine is present. `data` is decoded
    /// with [`String::from_utf8_lossy`], so invalid UTF-8 becomes replacement
    /// characters rather than an error.
    pub fn load_data(&mut self, data: &[u8], mime_type: &str, base_url: &str) {
        // `load_data` has no native equivalent on the engine trait; the core path
        // still runs, and the early return below keeps the prior behaviour of
        // routing through core only when a native engine is present.
        self.core.load_data(data, mime_type, base_url);
    }
    /// Steps one entry back in session history. A no-op when there is nothing
    /// behind the current entry, or when a native engine is present and returns
    /// without the entry — the core's history is not consulted on that path.
    pub fn go_back(&mut self) {
        if let Some(ref mut backend) = self.webkit_backend {
            backend.go_back();
            return;
        }
        self.core.go_back();
    }
    /// Steps one entry forward in session history. A no-op when there is nothing
    /// ahead of the current entry.
    pub fn go_forward(&mut self) {
        if let Some(ref mut backend) = self.webkit_backend {
            backend.go_forward();
            return;
        }
        self.core.go_forward();
    }
    /// Reloads the current document, driving the same 0 → 50 → 100 progress
    /// callbacks as a fresh load on the simulated path. Does nothing when there
    /// is no URL.
    pub fn reload(&mut self) {
        if let Some(ref mut backend) = self.webkit_backend {
            backend.reload();
            return;
        }
        self.core.reload();
    }
    /// Aborts an in-flight load and resets progress to 0. A no-op when nothing
    /// is loading.
    pub fn stop(&mut self) {
        if let Some(ref mut backend) = self.webkit_backend {
            backend.stop_loading();
            return;
        }
        self.core.stop();
    }
    /// Sets the title, emitting the core's `title_changed` signal only when the
    /// value actually differs.
    ///
    /// Always applied to the core, engine or not, so the widget's own view of the
    /// title is the authority rather than the engine's.
    pub fn set_title(&mut self, title: String) {
        self.core.set_title(title);
    }
    /// Runs `script` in the page context and returns its value.
    ///
    /// Fails with an error whose message is `"JavaScript is disabled"` when
    /// [`WebSettings::javascript_enabled`](super::WebSettings::javascript_enabled)
    /// is `false`, and with the evaluator's own error otherwise. Console output
    /// produced by the script is forwarded to the core's `console_message`
    /// signal.
    pub fn evaluate_javascript(&mut self, script: &str) -> JsResult<JsValue> {
        self.core.evaluate_javascript(script)
    }
    /// Turns script evaluation on or off by setting
    /// [`WebSettings::javascript_enabled`](super::WebSettings::javascript_enabled).
    /// Existing content is unaffected.
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.core.set_javascript_enabled(enabled);
    }
    /// The decoded document body most recently loaded, or `""` if none.
    pub fn content(&self) -> &str {
        self.core.content()
    }
    /// The document body most recently loaded.
    ///
    /// Identical to [`Self::content`] — the two names exist because one reads
    /// naturally for a markup payload and the other for a fetched body.
    pub fn html(&self) -> &str {
        self.core.html()
    }

    // -- Unique methods on WebEngineViewEnhanced --

    /// Enables or disables plugin support by setting
    /// [`WebSettings::plugins_enabled`](super::WebSettings::plugins_enabled).
    /// Plugins already registered are not unloaded by turning this off.
    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.core.settings.plugins_enabled = enabled;
    }

    /// Enters or leaves private browsing.
    ///
    /// Turning it **on** also replaces the current tracking protection with
    /// [`PrivacySettings::strict`](super::privacy::PrivacySettings::strict),
    /// discarding the previous policy and its blocked-request count. Turning it
    /// off resets the flag only — strict protection stays in force, and the
    /// relaxed policy that preceded it is not restored.
    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.core.settings.private_browsing = enabled;
        if enabled {
            self.core.privacy =
                super::privacy::TrackingProtection::new(super::privacy::PrivacySettings::strict());
        }
    }

    /// Erases the parts of the local browsing state selected by `data`, as
    /// described by [`BrowsingData`](super::privacy::BrowsingData).
    ///
    /// Clearing history empties the browsing history *and* resets the
    /// back/forward stack.
    pub fn clear_browsing_data(&mut self, data: super::privacy::BrowsingData) {
        self.core.clear_browsing_data(data);
    }
}

// Delegate Widget trait to core via the shared macro
delegate_widget!(WebEngineViewEnhanced);

impl EventHandler for WebEngineViewEnhanced {
    fn handle_event(&mut self, event: &Event) {
        self.core.base.handle_event(event);
        if !self.core.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, modifiers } = event {
            self.core.handle_key_event(*key, *modifiers);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::web::privacy::BrowsingData;

    #[test]
    fn test_web_engine_view_new() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 1024, 768));
        assert_eq!(engine.url(), "");
        assert!(!engine.is_loading());
        assert_eq!(engine.title(), "");
        assert_eq!(engine.load_progress(), 0);
        assert!(!engine.can_go_back());
        assert!(!engine.can_go_forward());
    }

    #[test]
    fn test_web_engine_view_load_url() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 1024, 768));
        engine.load_url("https://example.com");
        assert_eq!(engine.url(), "https://example.com");
        assert!(!engine.is_loading());
        assert_eq!(engine.load_progress(), 100);
    }

    #[test]
    fn test_web_engine_view_set_url() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.set_url("https://rust-lang.org".to_string());
        assert_eq!(engine.url(), "https://rust-lang.org");
    }

    #[test]
    fn test_web_engine_view_navigate_back_and_forward() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(!engine.can_go_back());
        assert!(!engine.can_go_forward());

        engine.load_url("https://page1.com");
        assert!(!engine.can_go_back());

        engine.load_url("https://page2.com");
        assert!(engine.can_go_back());
        assert!(!engine.can_go_forward());

        engine.go_back();
        assert!(engine.can_go_forward());
        assert_eq!(engine.url(), "https://page1.com");

        engine.go_forward();
        assert_eq!(engine.url(), "https://page2.com");
    }

    #[test]
    fn test_web_engine_view_reload() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.load_url("https://example.com");
        engine.reload();
        assert!(!engine.is_loading());
        assert_eq!(engine.load_progress(), 100);
    }

    #[test]
    fn test_web_engine_view_stop() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        // stop when not loading is a no-op
        engine.stop();
        assert!(!engine.is_loading());
    }

    #[test]
    fn test_web_engine_view_set_title() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(engine.title(), "");
        engine.set_title("Rust Widgets".to_string());
        assert_eq!(engine.title(), "Rust Widgets");
    }

    #[test]
    fn test_web_engine_view_load_html() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.load_html("<h1>Hello</h1>", Some("https://base.url"));
        assert_eq!(engine.url(), "https://base.url");
        assert_eq!(engine.title(), "HTML Content");
        assert_eq!(engine.html(), "<h1>Hello</h1>");
    }

    #[test]
    fn test_web_engine_view_load_data() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.load_data(b"binary content", "application/octet-stream", "https://data.url");
        assert_eq!(engine.url(), "https://data.url");
        assert_eq!(engine.title(), "Data: application/octet-stream");
        assert_eq!(engine.content(), "binary content");
    }

    #[test]
    fn test_web_engine_view_evaluate_javascript() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        let result = engine.evaluate_javascript("42");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), JsValue::Number(42.0));
    }

    #[test]
    fn test_web_engine_view_javascript_disabled() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.set_javascript_enabled(false);
        let result = engine.evaluate_javascript("1 + 1");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("JavaScript is disabled"));
    }

    #[test]
    fn test_web_engine_view_set_plugins_enabled() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(!engine.settings().plugins_enabled);
        engine.set_plugins_enabled(true);
        assert!(engine.settings().plugins_enabled);
    }

    #[test]
    fn test_web_engine_view_set_private_browsing() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(!engine.settings().private_browsing);
        engine.set_private_browsing(true);
        assert!(engine.settings().private_browsing);
    }

    #[test]
    fn test_web_engine_view_clear_browsing_data() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.load_url("https://example.com");
        assert!(!engine.browser_history().is_empty());
        engine.clear_browsing_data(BrowsingData {
            cookies: false,
            history: true,
            ..Default::default()
        });
        assert!(engine.browser_history().is_empty());
    }

    #[test]
    fn test_web_engine_view_signals_initialized() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        let _ = &engine.certificate_error;
        let _ = &engine.download_requested;
    }

    #[test]
    fn test_web_engine_view_settings_and_security() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(engine.settings().javascript_enabled);
        assert!(engine.security().block_popups);
    }

    #[test]
    fn test_web_engine_view_settings_mut() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.settings_mut().javascript_enabled = false;
        engine.settings_mut().webgl_enabled = false;
        assert!(!engine.settings().javascript_enabled);
        assert!(!engine.settings().webgl_enabled);
    }

    #[test]
    fn test_web_engine_view_security_mut() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.security_mut().block_popups = false;
        engine.security_mut().allow_insecure_content = true;
        assert!(!engine.security().block_popups);
        assert!(engine.security().allow_insecure_content);
    }

    #[test]
    fn test_web_engine_view_cookies_and_privacy() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(engine.cookies().is_empty());
        assert_eq!(engine.privacy().blocked_count(), 0);
    }

    #[test]
    fn test_web_engine_view_plugins_access() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(engine.plugins().list().is_empty());
    }
}
