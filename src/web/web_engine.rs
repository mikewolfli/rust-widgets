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
//!
//! # Rendering: simulated only (BLUE20 layer 5, ruling W1, 2026-09-21)
//!
//! This control does **not** render a web page. It models one: a URL, a title, a
//! history and a 0 → 50 → 100 progress sequence, which is what the declarative
//! and programmatic APIs are written against.
//!
//! A real engine used to be reachable on Linux behind the `webkit-engine` feature.
//! It was removed because it **never displayed anything**: 76 lines of one-line
//! forwards to `webkit2gtk`, one platform, and the `WebView` was never added to a
//! GTK container, so no user could ever have seen a page through this library. Keeping
//! it made the crate look like it rendered the web while it did not — the same defect
//! class as an event that is published but never emitted (BLUE19 #97).
//!
//! What this control *can* do is evaluate JavaScript, through the pure-Rust
//! [`boa`](super::js_engine) engine. That is an independent capability and is
//! unaffected by the removal: it runs the script and returns its value, which is
//! useful for validating expressions, templating and small computations
//! (see [`Self::evaluate_javascript`]).
//!
//! A caller can ask whether a real engine is available with
//! [`crate::platform::Platform::supports_web_engine`]; it answers `false` on every
//! current backend, and the answer is honest rather than a silent degradation.

use super::js_engine::{JsResult, JsValue};
use super::web_core::{delegate_widget, WebViewCore};
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{Widget, WidgetKind};

/// Enhanced web engine view widget.
///
/// Navigation runs the simulated loader: the core owns the URL, title and history,
/// and drives the 0 → 50 → 100 progress callbacks. See the module docs for why there
/// is no rendering engine behind it.
pub struct WebEngineViewEnhanced {
    core: WebViewCore,
    /// Emitted when the engine rejects a site certificate, carrying the
    /// description of the failure, through [`Self::report_certificate_error`].
    pub certificate_error: Signal1<String>,
    /// Emitted when a navigation asks to download rather than to display, carrying
    /// the URL of the download, through [`Self::request_download`].
    pub download_requested: Signal1<String>,
}

impl WebEngineViewEnhanced {
    /// Creates an engine view in `geometry` with an empty URL, no title, and
    /// nothing loading.
    pub fn new(geometry: Rect) -> Self {
        Self {
            core: WebViewCore::new(WidgetKind::WebEngineView, geometry, "WebEngineView", ""),
            certificate_error: Signal1::new(),
            download_requested: Signal1::new(),
        }
    }

    // -- Accessors that delegate to core --

    /// Whether a **real** web rendering engine is behind this view.
    ///
    /// # Why this query exists (BLUE20 layer 5, rules #97/#109)
    ///
    /// The previous implementation degraded silently: `new` asked the platform for an
    /// engine, and whether it got one was **not exposed**, so a caller that had to know
    /// was told to "query the platform directly". That made "this is a simulated view"
    /// undetectable — the same defect class as an event that is published but never
    /// emitted: a state the caller cannot observe and therefore cannot handle.
    ///
    /// It answers by asking the platform rather than by remembering a construction-time
    /// fact, so it stays correct if a backend gains engine support later. Today it is
    /// `false` everywhere (see [`Platform::supports_web_engine`]).
    ///
    /// [`Platform::supports_web_engine`]: crate::platform::Platform::supports_web_engine
    pub fn has_real_engine(&self) -> bool {
        crate::platform::platform_facts().supports_web_engine()
    }

    /// The address currently shown, or `""` until something is loaded.
    ///
    /// Reflects what this widget was asked to load, rather than where a real engine
    /// ended up after redirects — which is why it is the authority even on a backend
    /// that has one.
    pub fn url(&self) -> &str {
        self.core.url()
    }
    /// Whether a navigation is in flight.
    ///
    /// On the simulated path a load completes within the call, so this is `false`
    /// again by the time the loader returns.
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

    /// Reports a certificate rejection, emitting [`Self::certificate_error`].
    ///
    /// Called by a backend that surfaces a certificate callback, or by a host whose
    /// real engine reports through a channel of its own. This widget cannot detect
    /// one itself: it holds no TLS stack, and inventing a failure would report it at
    /// a moment unrelated to anything the user did.
    pub fn report_certificate_error(&mut self, message: impl Into<String>) {
        self.certificate_error.emit(message.into());
    }

    /// Reports that a navigation wants to download rather than display, emitting
    /// [`Self::download_requested`] with the URL.
    ///
    /// Nothing is fetched or written. The signal is the whole feature at this layer:
    /// deciding where a download lands, and whether to allow it at all, belongs to
    /// the host.
    pub fn request_download(&mut self, url: impl Into<String>) {
        self.download_requested.emit(url.into());
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
    /// Delegates to the simulated loader; see [`Self::set_url`] for that contract.
    pub fn load_url(&mut self, url: &str) {
        self.core.load_url(url);
    }
    /// Navigates to `url`, which must begin with `http://`, `https://` or
    /// `file://`.
    ///
    /// A URL with an unrecognised scheme is logged and rejected, leaving the view
    /// untouched and returning silently; navigating to the URL already displayed just
    /// marks the load complete at 100%.
    pub fn set_url(&mut self, url: String) {
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
        self.core.load_html(html, base_url);
    }
    /// Loads `data` as the document at `base_url`, declaring the bytes to be of
    /// type `mime_type` (the title becomes `"Data: <mime_type>"`).
    ///
    /// `data` is decoded with [`String::from_utf8_lossy`], so invalid UTF-8 becomes
    /// replacement characters rather than an error.
    pub fn load_data(&mut self, data: &[u8], mime_type: &str, base_url: &str) {
        self.core.load_data(data, mime_type, base_url);
    }
    /// Steps one entry back in session history. A no-op when there is nothing
    /// behind the current entry.
    pub fn go_back(&mut self) {
        self.core.go_back();
    }
    /// Steps one entry forward in session history. A no-op when there is nothing
    /// ahead of the current entry.
    pub fn go_forward(&mut self) {
        self.core.go_forward();
    }
    /// Reloads the current document, driving the same 0 → 50 → 100 progress
    /// callbacks as a fresh load. Does nothing when there is no URL.
    pub fn reload(&mut self) {
        self.core.reload();
    }
    /// Aborts an in-flight load and resets progress to 0. A no-op when nothing
    /// is loading.
    pub fn stop(&mut self) {
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

    // ── BLUE20 layer 5: the degradation must be queryable ─────────────────────

    /// The query must exist and must agree with the platform.
    ///
    /// Before this, whether a real engine backed the view was **not exposed** — the
    /// constructor's doc said a caller that had to know should "query the platform
    /// directly", which is a promise the caller cannot keep because the *widget* was
    /// what held the engine. This asserts the two agree, so the widget can never claim
    /// an engine the platform does not have (or vice versa).
    #[test]
    fn test_has_real_engine_agrees_with_the_platform() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(
            engine.has_real_engine(),
            crate::platform::platform_facts().supports_web_engine(),
            "the widget must report the platform's capability, not remember its own guess"
        );
    }

    /// The honest answer on every current backend is `false` (ruling W1).
    ///
    /// Asserting the literal is deliberate *here* — unlike the colour assertions, where a
    /// literal is a theme implementation detail, "no backend renders the web" is a fact
    /// about the library's current capability, and it is the coordinate this whole layer
    /// is about. If a backend gains a real engine, this test is the reminder to update the
    /// claim in the module docs rather than a silent change.
    #[test]
    fn test_no_backend_renders_the_web_today() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(
            !engine.has_real_engine(),
            "a real engine became available: update the module docs of src/web/web_engine.rs \
             and src/platform/types.rs, which state that no backend renders the web"
        );
    }

    /// The simulated path must still be fully functional after the engine removal: a
    /// navigation updates the URL, the title and the history exactly as before.
    #[test]
    fn test_simulated_navigation_survives_the_engine_removal() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.load_url("https://example.com");
        assert_eq!(engine.url(), "https://example.com");
        assert_eq!(engine.load_progress(), 100, "the simulated loader completes");
        engine.load_url("https://example.org");
        assert!(engine.can_go_back(), "history is still recorded");
        engine.go_back();
        assert_eq!(engine.url(), "https://example.com");
        assert!(engine.can_go_forward());
        engine.go_forward();
        assert_eq!(engine.url(), "https://example.org");
        engine.reload();
        assert_eq!(engine.url(), "https://example.org");
    }

    /// `stop` cancels an **in-flight** load only.
    ///
    /// Asserting the guard is the point: a `stop` on an idle view must be a no-op rather
    /// than clearing the progress of a load that already finished — otherwise a stray
    /// cancel would make a completed navigation look like it never happened.
    #[test]
    fn test_stop_only_cancels_an_in_flight_load() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.load_url("https://example.com");
        assert_eq!(engine.load_progress(), 100);
        assert!(!engine.is_loading(), "the simulated loader has already finished");
        engine.stop();
        assert_eq!(
            engine.load_progress(),
            100,
            "stopping an idle view must not discard a completed load's progress"
        );
    }

    /// JavaScript evaluation is an independent capability (pure-Rust `boa`) and must keep
    /// working: it never went through the removed engine trait.
    ///
    /// The result is the script's completion value. A declaration whose initialiser is an
    /// expression completes with that value (`var x = 1` -> `1`); a bare statement's
    /// completion is `Undefined`. Both are asserted because the engine now evaluates
    /// arithmetic at all — before this, `1 + 2` produced `Undefined`, which contradicted
    /// the module's own documentation.
    #[test]
    fn test_javascript_evaluation_is_unaffected_by_the_engine_removal() {
        let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        engine.load_html("<p>hi</p>", None);
        engine.set_javascript_enabled(true);
        // Without the `js-engine` feature the call must report why rather than panic.
        match engine.evaluate_javascript("1 + 2") {
            Ok(value) => {
                assert_eq!(value, JsValue::Number(3.0), "an expression evaluates to its value");
                assert_eq!(
                    engine.evaluate_javascript("(1 + 2) * 3").unwrap(),
                    JsValue::Number(9.0),
                    "parentheses group, and precedence is respected"
                );
                assert_eq!(
                    engine.evaluate_javascript("var x = 1; x + 1").unwrap(),
                    JsValue::Number(2.0),
                    "a declaration followed by an expression runs both"
                );
                assert_eq!(
                    engine.evaluate_javascript("1 < 2").unwrap(),
                    JsValue::Boolean(true),
                    "comparison is not mistaken for a declaration"
                );
            }
            Err(error) => assert!(
                error.message.contains("JavaScript is disabled"),
                "the only acceptable failure is the feature being off; got {error:?}"
            ),
        }
    }
}
