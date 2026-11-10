// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared core for WebEngineViewEnhanced and WebViewEnhanced.
//!
//! Both types delegate to [`WebViewCore`] to avoid ~95% code duplication.
//! Each wrapper adds only its own unique signals and behavior on top of this core.
//!
//! # Navigation
//!
//! By default, all navigation methods are **simulated** — no real web engine is used.
//! Progress callbacks are emitted as 0% → 50% → 100% to allow UI bindings
//! to observe load lifecycle events.
//!
//! Enable the `web-http` feature (`cargo build --features web-http`) to replace the
//! simulated progress with real HTTP fetching via `ureq`. In that mode, `http://` and
//! `https://` URLs are fetched over the network; responses are stored as content and
//! the HTML `<title>` is extracted automatically.
//!
//! Regardless of feature selection, `file://` URLs and the `simulation_engine` trait
//! always take priority over the real HTTP path.

use super::history::{BrowserHistory, SessionHistory};
use super::js_engine::{JsContext, JsEngine, JsResult, JsValue, SimpleJsEngine};
use super::plugins::PluginManager;
use super::privacy::{CookieJar, PrivacySettings, TrackingProtection};
use rust_widgets::core::Rect;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::{BaseWidget, WidgetKind};

/// A pluggable simulation engine that can be set externally (e.g. in tests)
/// to inject mock content or simulate different network conditions.
pub trait SimulationEngine: Send {
    /// Simulate navigation to the given URL and return simulated content.
    fn simulate_navigation(&mut self, url: &str) -> Result<String, String>;
}

/// The result of one asynchronous HTTP fetch, delivered from the helper thread.
///
/// Carries the generation it was started for so [`WebViewCore::poll_load`] can discard a result whose
/// navigation has since been superseded.
#[cfg(feature = "web-http")]
struct HttpLoadOutcome {
    generation: u64,
    result: Result<String, String>,
}

/// Performs one blocking HTTP GET and returns the response body, or a message describing the failure.
///
/// This is the only part that blocks, and it runs on a helper thread — never on the UI thread — so a
/// slow server cannot freeze the caller. Keeping it a free function makes it obvious that it touches
/// no view state; the thread's only output is the returned `Result`, delivered over a channel.
/// Maximum bytes read from one HTTP response body.
///
/// A web view substitutes for a browser, but its fetcher is not a browser: an
/// unbounded `read_to_string` against a hostile or misconfigured server could
/// allocate until the process is killed. 16 MiB is larger than any HTML document a
/// browser would keep as a page (larger payloads are downloads, not documents).
#[cfg(feature = "web-http")]
const HTTP_BODY_LIMIT: u64 = 16 * 1024 * 1024;

/// Wall-clock budget for one fetch (connect + headers + body).
///
/// Without it a stalled server blocked the helper thread — and therefore the load —
/// indefinitely; the fetch has no cancellation channel of its own.
#[cfg(feature = "web-http")]
const HTTP_TIMEOUT: core::time::Duration = core::time::Duration::from_secs(30);

#[cfg(feature = "web-http")]
fn fetch_http_content(url: &str) -> Result<String, String> {
    let agent =
        ureq::Agent::config_builder().timeout_global(Some(HTTP_TIMEOUT)).build().new_agent();
    match agent.get(url).call() {
        Ok(mut response) => response
            .body_mut()
            .with_config()
            .limit(HTTP_BODY_LIMIT)
            .read_to_string()
            .map_err(|e| format!("Failed to read response body from '{url}': {e}")),
        Err(e) => Err(format!("HTTP request failed for '{url}': {e}")),
    }
}

/// Shared fields used by both WebEngineViewEnhanced and WebViewEnhanced.
pub struct WebViewCore {
    pub base: BaseWidget,
    pub url: String,
    pub loading: bool,
    pub title: String,
    pub load_progress: u8,
    pub history: SessionHistory,
    pub browser_history: BrowserHistory,
    pub js_engine: Box<dyn JsEngine>,
    pub js_context: JsContext,
    pub cookies: CookieJar,
    pub privacy: TrackingProtection,
    pub plugins: PluginManager,
    pub settings: super::WebSettings,
    pub security: super::SecuritySettings,
    pub loading_started: Signal1<String>,
    pub loading_finished: Signal1<String>,
    pub loading_progress: Signal1<u8>,
    pub title_changed: Signal1<String>,
    pub url_changed: Signal1<String>,
    /// Emitted when a load fails, through `WebEngine::report_error`.
    ///
    /// Was named `_error_occurred`, which marked it as unused while it was still a
    /// `pub` field: a leading underscore on a public item tells readers the wrong
    /// thing and hides the field from a search for `error_occurred`. The name is now
    /// the one callers would look for.
    pub error_occurred: Signal1<String>,
    pub navigation_state_changed: Signal1<(bool, bool)>,
    pub console_message: Signal1<(String, u32, String)>,
    pub content: String,
    /// Monotonic version of [`Self::content`], bumped on every write.
    ///
    /// A painter caches a rendered preview of the body. Comparing the body itself every frame
    /// is O(document), while this counter is O(1) and changes exactly when the text does, so a
    /// cache keyed on it cannot serve a preview of a previous document (the older key used the
    /// length plus the first and last byte, which two different bodies can share).
    content_revision: u64,
    /// Optional simulation engine for injecting mock data in tests.
    /// When set, navigation methods will delegate to this engine
    /// instead of the default simulated 0→50→100 progress.
    pub simulation_engine: Option<Box<dyn SimulationEngine>>,
    /// The in-flight asynchronous HTTP load, if any (`web-http` only).
    ///
    /// # Why the fetch does not run on the calling thread
    ///
    /// `set_url` used to perform the whole request and body read inline, so a slow server froze the
    /// UI thread for the round trip and a superseded navigation could not be abandoned. The request
    /// now runs on a helper thread and its result is delivered through this channel, which
    /// [`Self::poll_load`] drains. `load_generation` is bumped on every navigation, so a result that
    /// belongs to an older generation is dropped instead of overwriting a newer page.
    #[cfg(feature = "web-http")]
    pending_load: Option<std::sync::mpsc::Receiver<HttpLoadOutcome>>,
    /// Monotonic id of the current navigation; see [`Self::pending_load`].
    #[cfg(feature = "web-http")]
    load_generation: u64,
}

impl WebViewCore {
    pub fn new(
        kind: WidgetKind,
        geometry: Rect,
        widget_name: &'static str,
        initial_url: &str,
    ) -> Self {
        Self {
            base: BaseWidget::new(kind, geometry, widget_name),
            url: initial_url.to_string(),
            loading: false,
            title: String::new(),
            load_progress: 0,
            history: SessionHistory::default(),
            browser_history: BrowserHistory::new(),
            js_engine: Box::new(SimpleJsEngine::new()),
            js_context: JsContext::new(),
            cookies: CookieJar::new(),
            privacy: TrackingProtection::new(PrivacySettings::balanced()),
            plugins: PluginManager::new(),
            settings: super::WebSettings::default(),
            security: super::SecuritySettings::default(),
            loading_started: Signal1::new(),
            loading_finished: Signal1::new(),
            loading_progress: Signal1::new(),
            title_changed: Signal1::new(),
            url_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
            console_message: Signal1::new(),
            content: String::new(),
            content_revision: 0,
            simulation_engine: None,
            #[cfg(feature = "web-http")]
            pending_load: None,
            #[cfg(feature = "web-http")]
            load_generation: 0,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn load_progress(&self) -> u8 {
        self.load_progress
    }
    pub fn can_go_back(&self) -> bool {
        self.history.can_go_back()
    }
    pub fn can_go_forward(&self) -> bool {
        self.history.can_go_forward()
    }
    pub fn settings(&self) -> &super::WebSettings {
        &self.settings
    }
    pub fn settings_mut(&mut self) -> &mut super::WebSettings {
        &mut self.settings
    }
    pub fn security(&self) -> &super::SecuritySettings {
        &self.security
    }
    pub fn security_mut(&mut self) -> &mut super::SecuritySettings {
        &mut self.security
    }
    pub fn cookies(&self) -> &CookieJar {
        &self.cookies
    }
    pub fn cookies_mut(&mut self) -> &mut CookieJar {
        &mut self.cookies
    }
    pub fn privacy(&self) -> &TrackingProtection {
        &self.privacy
    }
    pub fn privacy_mut(&mut self) -> &mut TrackingProtection {
        &mut self.privacy
    }
    pub fn plugins(&self) -> &PluginManager {
        &self.plugins
    }
    pub fn plugins_mut(&mut self) -> &mut PluginManager {
        &mut self.plugins
    }
    pub fn history(&self) -> &SessionHistory {
        &self.history
    }
    pub fn browser_history(&self) -> &BrowserHistory {
        &self.browser_history
    }

    pub fn load_url(&mut self, url: &str) {
        self.set_url(url.to_string());
    }

    /// Navigate to the given URL.
    ///
    /// Validates the URL scheme (`http://`, `https://`, or `file://`) before
    /// accepting it. If a [`SimulationEngine`] is set, it takes priority — the
    /// engine's returned content replaces any real or simulated data.
    ///
    /// # Real HTTP fetching (`web-http` feature)
    ///
    /// When the `web-http` feature is enabled, `http://` and `https://` URLs
    /// are fetched over the network using `ureq`. The response body is stored
    /// as content and the HTML `<title>` is extracted automatically.
    /// `file://` URLs always fall back to simulated progress.
    ///
    /// # Simulated mode (default)
    ///
    /// Without `web-http`, progress is emitted as 0% → 50% → 100% with a brief
    /// delay, mimicking load lifecycle events without actual network access.
    pub fn set_url(&mut self, url: String) {
        // Validate URL scheme.
        if !url.starts_with("http://")
            && !url.starts_with("https://")
            && !url.starts_with("file://")
        {
            // Reported through the signal as well as the log: a log line is invisible
            // to a caller, and a rejected navigation that produced no event left the
            // host waiting for a load that was never going to start.
            let message = format!(
                "Invalid URL scheme for '{url}' \u{2014} must start with http://, https://, or file://"
            );
            log::error!("[web] {message}");
            self.error_occurred.emit(message);
            return;
        }

        if self.url == url {
            // A repeat navigation to the page already open is a no-op. The old early return
            // forced the progress to 100 while leaving an in-flight load marked as loading,
            // so a view could report a finished load that had not delivered anything. Doing
            // nothing keeps both facts truthful: an in-flight load stays in flight and an idle
            // view stays idle.
            return;
        }

        // Whether this navigation produced a body of its own. Only then is a snapshot recorded:
        // the simulated (non-fetching) path leaves the previous body on screen, and caching that
        // under the new URL would tell a later Back that this page showed the old document.
        let mut produced_body = false;

        // Delegate to simulation engine if one is set (primarily for testing).
        if let Some(ref mut engine) = self.simulation_engine {
            match engine.simulate_navigation(&url) {
                Ok(content) => {
                    self.store_content(content);
                    produced_body = true;
                }
                Err(e) => {
                    // The engine is the source of the failure, so it is reported through the
                    // same signal an invalid scheme uses; a log line is invisible to the host.
                    let message = format!("simulation engine error for '{url}': {e}");
                    log::error!("[web] {message}");
                    self.error_occurred.emit(message);
                    return;
                }
            }
        }

        #[cfg(feature = "web-http")]
        self.invalidate_pending_load();

        self.url = url.clone();
        self.loading = true;
        self.load_progress = 0;
        // The progress strip starts at zero, so a host mirroring the signal has to hear it: only
        // the completion used to be published, and the strip jumped straight to full.
        self.loading_progress.emit(self.load_progress);
        self.url_changed.emit(url.clone());
        self.loading_started.emit(url.clone());
        self.record_session_navigation(url.clone());

        // ── Real HTTP fetch (web-http feature) ──
        //
        // The request runs on a helper thread and its outcome is delivered through a channel drained
        // by [`Self::poll_load`]. Running it inline here blocked the caller for the whole round trip
        // (a slow server froze the UI) and made a superseded navigation impossible to abandon; a
        // superseded load is now dropped by generation rather than by luck of timing.
        #[cfg(feature = "web-http")]
        {
            // A simulation engine is the source of the body; fetching the same URL over the
            // network would overwrite exactly the content the caller asked the engine for.
            if produced_body {
                self.load_progress = 50;
            } else if url.starts_with("http://") || url.starts_with("https://") {
                self.load_progress = 10;
                self.loading_progress.emit(self.load_progress);

                let generation = self.load_generation;
                let request_url = url.clone();
                let (sender, receiver) = std::sync::mpsc::channel();
                self.pending_load = Some(receiver);

                // `spawn` can fail only on resource exhaustion; fall back to the synchronous
                // path in that case so the navigation still completes rather than hanging on a
                // channel that nothing will ever write to.
                let spawned =
                    std::thread::Builder::new().name("rw-web-http".to_string()).spawn(move || {
                        let result = fetch_http_content(&request_url);
                        // A send error means the receiver was dropped (the view was destroyed);
                        // there is nothing to report to and no leak, so it is ignored.
                        let _ = sender.send(HttpLoadOutcome { generation, result });
                    });
                if spawned.is_err() {
                    log::error!(
                        "[web] could not start the HTTP fetch thread for '{url}'; loading inline"
                    );
                    self.pending_load = None;
                    let result = fetch_http_content(&url);
                    self.apply_http_outcome(generation, result);
                }
            } else {
                // file:// URL fallback. This used to sleep 50 ms on the caller's thread
                // to "simulate progress"; a UI navigation must never block the frame loop.
                self.load_progress = 50;
            }
        }

        // ── Simulated mode (no web-http feature) ──
        #[cfg(not(feature = "web-http"))]
        {
            // Report the intermediate step without sleeping: the caller may be the UI thread.
            self.load_progress = 50;
            self.loading_progress.emit(self.load_progress);
        }

        // When an asynchronous HTTP fetch is in flight, the load is *not* finished here: the 100%
        // progress and `loading_finished` are emitted by [`Self::poll_load`] when the result arrives
        // (or by the error path when it fails). Emitting them inline would announce a completion for a
        // page whose bytes have not been read yet.
        #[cfg(feature = "web-http")]
        if self.pending_load.is_some() {
            // Only the completion is deferred: `history.navigate` above has already changed
            // `can_go_back`, and `stop()` ends the load without reaching `apply_http_outcome`,
            // so the state change has to be published here or it never is. The URL bar and the
            // progress strip changed too, so the frame has to be requested before the return.
            self.update_navigation_state();
            self.base.request_redraw();
            return;
        }

        // Emit progress at 100% and finish.
        self.load_progress = 100;
        self.loading = false;
        self.loading_progress.emit(self.load_progress);
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.update_navigation_state();
        self.record_page_visit();
        if produced_body {
            self.record_current_page();
        }
        self.base.request_redraw();
    }

    /// SIMULATED: No real web engine — loads HTML content with simulated
    /// 0% → 100% progress callbacks.
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        #[cfg(feature = "web-http")]
        self.invalidate_pending_load();

        self.url = base_url.unwrap_or("data:text/html").to_string();
        // A document load is a navigation, so it belongs in the session history: without this
        // an HTML page was invisible to `go_back`, which could only ever reach pages visited
        // through `set_url`. The URL is the one this loader has always published
        // (`base_url`, or `data:text/html`); a per-document synthetic URL would change the
        // public `url()` value that the wrappers document and tests assert, so the stable
        // existing URL is kept and two anonymous HTML documents share one history spelling.
        self.record_session_navigation(self.url.clone());
        self.title = "HTML Content".to_string();
        self.loading = true;
        self.load_progress = 0;
        self.loading_started.emit(self.url.clone());
        self.store_content(html.to_string());

        // Emit progress at 50%.
        self.load_progress = 50;
        self.loading_progress.emit(self.load_progress);

        // Emit progress at 100% and finish.
        self.load_progress = 100;
        self.loading = false;
        self.loading_progress.emit(self.load_progress);
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        // A document load is a navigation, so the panel-facing history records it exactly as
        // `set_url` does; otherwise `browser_history()` described a different session than
        // `history()` for the same loads.
        self.record_page_visit();
        self.record_current_page();
        self.base.request_redraw();
    }

    /// SIMULATED: No real web engine — loads binary data as a string with simulated
    /// 0% → 50% → 100% progress callbacks.
    pub fn load_data(&mut self, data: &[u8], mime_type: &str, base_url: &str) {
        #[cfg(feature = "web-http")]
        self.invalidate_pending_load();

        self.url = base_url.to_string();
        // See `load_html`: a data load is a navigation and must be reachable through Back.
        self.record_session_navigation(self.url.clone());
        self.title = format!("Data: {mime_type}");
        self.loading = true;
        self.load_progress = 0;
        self.loading_started.emit(self.url.clone());
        self.store_content(String::from_utf8_lossy(data).to_string());

        // Emit progress at 50%.
        self.load_progress = 50;
        self.loading_progress.emit(self.load_progress);

        // Emit progress at 100% and finish.
        self.load_progress = 100;
        self.loading = false;
        self.loading_progress.emit(self.load_progress);
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        // A data load changes the displayed URL, so the address bar must hear about it;
        // `load_html` already published this and the history paths were fixed for the same
        // reason, which left a data load as the one navigation that moved silently.
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.record_page_visit();
        self.record_current_page();
        self.base.request_redraw();
    }

    /// SIMULATED: No real web engine — navigates back in history with simulated
    /// loading callbacks.
    pub fn go_back(&mut self) {
        if let Some(url) = self.history.go_back() {
            #[cfg(feature = "web-http")]
            self.invalidate_pending_load();

            self.url = url;
            self.restore_current_page();
            self.loading = true;
            self.load_progress = 0;
            self.url_changed.emit(self.url.clone());
            self.loading_started.emit(self.url.clone());

            // Emit progress at 50%.
            self.load_progress = 50;
            self.loading_progress.emit(self.load_progress);

            // Emit progress at 100% and finish.
            self.load_progress = 100;
            self.loading = false;
            self.loading_progress.emit(self.load_progress);
            self.loading_finished.emit(self.url.clone());
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }

    /// SIMULATED: No real web engine — navigates forward in history with simulated
    /// loading callbacks.
    pub fn go_forward(&mut self) {
        if let Some(url) = self.history.go_forward() {
            #[cfg(feature = "web-http")]
            self.invalidate_pending_load();

            self.url = url;
            self.restore_current_page();
            self.loading = true;
            self.load_progress = 0;
            self.url_changed.emit(self.url.clone());
            self.loading_started.emit(self.url.clone());

            // Emit progress at 50%.
            self.load_progress = 50;
            self.loading_progress.emit(self.load_progress);

            // Emit progress at 100% and finish.
            self.load_progress = 100;
            self.loading = false;
            self.loading_progress.emit(self.load_progress);
            self.loading_finished.emit(self.url.clone());
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }

    /// SIMULATED: No real web engine — simulates a page reload with
    /// 0% → 50% → 100% progress callbacks.
    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            #[cfg(feature = "web-http")]
            self.invalidate_pending_load();

            self.loading = true;
            self.load_progress = 0;
            self.loading_started.emit(self.url.clone());

            // Emit progress at 50%.
            self.load_progress = 50;
            self.loading_progress.emit(self.load_progress);

            // Emit progress at 100% and finish.
            self.load_progress = 100;
            self.loading = false;
            self.loading_progress.emit(self.load_progress);
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }

    pub fn stop(&mut self) {
        // Cancel any in-flight asynchronous fetch: dropping the receiver closes the channel and
        // bumping the generation invalidates a result already in flight, so a helper thread cannot
        // deliver a page after the caller asked it to stop.
        #[cfg(feature = "web-http")]
        self.invalidate_pending_load();
        if self.loading {
            self.loading = false;
            let had_progress = self.load_progress != 0;
            self.load_progress = 0;
            // A host mirroring the strip would otherwise keep showing the progress of a load that
            // was cancelled while `load_progress()` already reads zero.
            if had_progress {
                self.loading_progress.emit(self.load_progress);
            }
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }

    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title.clone();
        self.title_changed.emit(title);
        // The title belongs to the page on screen, so the snapshot Back replays has to carry it
        // too; otherwise stepping away and back restored the old title.
        self.record_current_page();
        self.base.request_redraw();
    }

    pub fn evaluate_javascript(&mut self, script: &str) -> JsResult<JsValue> {
        if !self.settings.javascript_enabled {
            return Err(super::js_engine::JsError::new(format!(
                "JavaScript is disabled for this view, so the {} byte script was not run; set \
                 `settings.javascript_enabled` to true to allow evaluation",
                script.len()
            )));
        }
        let result = self.js_engine.evaluate(script, &mut self.js_context)?;
        for msg in self.js_context.console_messages() {
            let level = format!("{:?}", msg.level);
            self.console_message.emit((level, msg.line, msg.message.clone()));
        }
        // The context accumulates messages for as long as it lives, so a second evaluation
        // would re-emit everything the first one logged. The signal is the only reader;
        // clearing after forwarding makes each call report exactly its own output.
        self.js_context.clear_console();
        Ok(result)
    }

    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.settings.javascript_enabled = enabled;
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    /// The version of [`Self::content`] (see the field docs).
    ///
    /// A repaint can key a derived cache on this instead of hashing or comparing the whole
    /// body every frame.
    pub fn content_revision(&self) -> u64 {
        self.content_revision
    }

    /// Replaces the body and bumps [`Self::content_revision`].
    fn store_content(&mut self, content: String) {
        self.content = content;
        self.content_revision = self.content_revision.wrapping_add(1);
    }

    /// Records the document on screen against the current session-history entry.
    ///
    /// Called once a page reaches its final state — the engine returned a body, the async fetch
    /// delivered one, or a document/data loader finished — because that is the first moment the
    /// view knows what the entry displayed. `go_back`/`go_forward` replay it, so stepping through
    /// history restores the body instead of leaving the page being left on screen.
    ///
    /// The body is copied once per completed navigation; [`SessionHistory`] bounds both the size
    /// of one cached body and their total, so a long session cannot pin unbounded memory.
    fn record_current_page(&mut self) {
        if self.url.is_empty() {
            return;
        }
        let page = super::history::PageSnapshot {
            title: self.title.clone(),
            content: self.content.clone(),
        };
        self.history.set_current_page(Some(page));
    }

    /// Replays the body recorded for the current history entry, when there is one.
    ///
    /// An entry with no snapshot (a page larger than the per-entry cap, or one whose body was
    /// dropped by the total budget) keeps the previous behaviour: the URL moves and the body is
    /// left as it was, rather than blanking the view for a page no source can re-fetch yet.
    fn restore_current_page(&mut self) {
        let Some(page) = self.history.current_page().cloned() else {
            return;
        };
        if self.title != page.title {
            self.title = page.title;
            self.title_changed.emit(self.title.clone());
        }
        self.store_content(page.content);
    }

    pub fn html(&self) -> &str {
        &self.content
    }

    /// Whether this session may record where it went.
    ///
    /// `WebSettings::private_browsing` is documented as "avoids persisting cookies and history",
    /// but nothing read it: a host that entered a private session still got a visit list and a
    /// back/forward stack that outlived the session.
    fn records_history(&self) -> bool {
        !self.settings.private_browsing
    }

    /// Pushes `url` on the session stack, unless this is a private session.
    fn record_session_navigation(&mut self, url: String) {
        if !self.records_history() {
            return;
        }
        self.history.navigate(url);
    }

    /// Appends the current page to the visit list, unless this is a private session.
    fn record_page_visit(&mut self) {
        if !self.records_history() {
            return;
        }
        self.browser_history.add_entry(self.url.clone(), self.title.clone());
    }

    pub fn clear_browsing_data(&mut self, data: super::privacy::BrowsingData) {
        if data.cookies {
            self.cookies.clear();
        }
        if data.plugin_data {
            // The core owns the plugin registry and it is the one non-cookie store the model can
            // actually hold; `PluginManager::clear` existed but no path called it.
            self.plugins.clear();
        }
        if data.history {
            self.browser_history.clear();
            self.history.clear();
            // Both stacks are gone, so `can_go_back`/`can_go_forward` flipped. This signal is the
            // only channel a host has for that, and every other path that mutates the stacks
            // publishes it; clearing also changes what the view paints (history panel, buttons).
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }

    fn update_navigation_state(&self) {
        self.navigation_state_changed.emit((self.can_go_back(), self.can_go_forward()));
    }

    #[cfg(feature = "web-http")]
    fn invalidate_pending_load(&mut self) {
        self.pending_load = None;
        self.load_generation = self.load_generation.wrapping_add(1);
    }

    /// Ends a failed load: clears the loading flag, emits the error, and repaints.
    ///
    /// # Why this is separate from the success tail
    ///
    /// A failed fetch used to fall through to the same 100% / `loading_finished` emission the
    /// success path used, so a caller could not tell a rendered page from a network error — the
    /// "reported success for something that did not happen" failure the project's principles forbid.
    /// A failure now emits `error_occurred` and **not** `loading_finished`, leaving `load_progress`
    /// where it stalled so a progress indicator does not falsely complete.
    #[cfg(feature = "web-http")]
    fn finish_load_with_error(&mut self, message: String) {
        log::error!("[web] {message}");
        self.loading = false;
        self.error_occurred.emit(message);
        self.update_navigation_state();
        self.base.request_redraw();
    }

    /// Applies the outcome of an HTTP fetch, if it still belongs to the current navigation.
    ///
    /// `generation` must equal [`Self::load_generation`]; a mismatch means a later `set_url`
    /// superseded this load, so its bytes must not overwrite the newer page.
    #[cfg(feature = "web-http")]
    fn apply_http_outcome(&mut self, generation: u64, result: Result<String, String>) {
        if generation != self.load_generation {
            // A stale response for a navigation that has been replaced: drop it silently rather
            // than reporting it — it is not an error, it is simply no longer wanted.
            return;
        }
        match result {
            Ok(body) => {
                self.load_progress = 60;
                self.loading_progress.emit(self.load_progress);
                self.store_content(body);
                if let Some(title_start) = self.content.find("<title>") {
                    let after = &self.content[title_start..];
                    if let Some(title_end) = after.find("</title>") {
                        self.title = after[7..title_end].to_string();
                    }
                }
                self.load_progress = 100;
                self.loading = false;
                self.loading_progress.emit(self.load_progress);
                self.loading_finished.emit(self.url.clone());
                self.title_changed.emit(self.title.clone());
                self.record_page_visit();
                self.record_current_page();
                self.update_navigation_state();
                self.base.request_redraw();
            }
            Err(message) => self.finish_load_with_error(message),
        }
    }

    /// Advances an in-flight asynchronous load, if any (`web-http` only).
    ///
    /// The host calls this from its frame/tick loop. It returns `true` while a load is still pending
    /// (so the caller knows to keep polling) and `false` once the load has completed or failed. This
    /// is the entry point that replaces the old inline blocking: nothing in `set_url` waits on the
    /// network, and the signals fire on the caller's own thread through this poll.
    #[cfg(feature = "web-http")]
    pub fn poll_load(&mut self) -> bool {
        let Some(receiver) = self.pending_load.as_ref() else {
            return false;
        };
        match receiver.try_recv() {
            Ok(outcome) => {
                self.pending_load = None;
                self.apply_http_outcome(outcome.generation, outcome.result);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // The helper thread vanished without sending (panicked). Do not leave the view
                // stuck "loading" forever: report it as a failed load.
                self.pending_load = None;
                self.finish_load_with_error(String::from(
                    "HTTP fetch thread ended without delivering a result",
                ));
                false
            }
        }
    }

    /// Handle common key events for navigation.
    pub fn handle_key_event(&mut self, key: u32, modifiers: u32) {
        match key {
            37 if modifiers == 0 => {
                self.go_back();
            }
            39 if modifiers == 0 => {
                self.go_forward();
            }
            116 => {
                self.reload();
            }
            // Ctrl+R reloads. The event bitmask's Control bit (2) is the semantic one; bit 1 is
            // Shift, and testing the wrong bit made Shift+R reload while Ctrl+R did nothing.
            82 if rust_widgets::shortcut::Modifiers::from_event_bits(modifiers)
                .contains(rust_widgets::shortcut::Modifiers::CTRL) =>
            {
                self.reload();
            }
            _ => { /* Other keys are not relevant */ }
        }
    }
}

// ---------------------------------------------------------------------------
// Widget trait delegation for WebViewCore (used by both wrappers)
// ---------------------------------------------------------------------------
macro_rules! delegate_widget {
    ($wrapper:ty) => {
        impl Widget for $wrapper {
            fn base(&self) -> &rust_widgets::widget::BaseWidget {
                &self.core.base
            }
            fn base_mut(&mut self) -> &mut rust_widgets::widget::BaseWidget {
                &mut self.core.base
            }
            fn id(&self) -> ObjectId {
                self.core.base.id()
            }
            fn kind(&self) -> WidgetKind {
                self.core.base.kind()
            }
            fn geometry(&self) -> Rect {
                self.core.base.geometry()
            }
            fn set_geometry(&mut self, geometry: Rect) {
                self.core.base.set_geometry(geometry);
            }
            fn min_size(&self) -> Option<Size> {
                self.core.base.min_size()
            }
            fn max_size(&self) -> Option<Size> {
                self.core.base.max_size()
            }
            fn set_min_size(&mut self, min_size: Option<Size>) {
                self.core.base.set_min_size(min_size);
            }
            fn set_max_size(&mut self, max_size: Option<Size>) {
                self.core.base.set_max_size(max_size);
            }
            fn parent(&self) -> Option<ObjectId> {
                self.core.base.parent()
            }
            fn set_parent(&mut self, parent: Option<ObjectId>) {
                self.core.base.set_parent(parent);
            }
            fn children(&self) -> &[ObjectId] {
                self.core.base.children()
            }
            fn add_child(&mut self, child: ObjectId) {
                self.core.base.add_child(child);
            }
            fn remove_child(&mut self, child: ObjectId) {
                self.core.base.remove_child(child);
            }
            fn show(&mut self) {
                self.core.base.show();
            }
            fn hide(&mut self) {
                self.core.base.hide();
            }
            fn is_visible(&self) -> bool {
                self.core.base.is_visible()
            }
            fn set_enabled(&mut self, enabled: bool) {
                self.core.base.set_enabled(enabled);
            }
            fn is_enabled(&self) -> bool {
                self.core.base.is_enabled()
            }
            fn set_tooltip(&mut self, tooltip: String) {
                self.core.base.set_tooltip(tooltip);
            }
            fn tooltip(&self) -> &str {
                self.core.base.tooltip()
            }
            fn style(&self) -> &WidgetStyle {
                self.core.base.style()
            }
            fn set_style(&mut self, style: WidgetStyle) {
                self.core.base.set_style(style);
            }
            fn connection_scope(&self) -> &ConnectionScope {
                self.core.base.connection_scope()
            }
            fn hover_signal(&self) -> &Signal1<Point> {
                self.core.base.hover_signal()
            }
            fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
                self.core.base.mouse_down_signal()
            }
            fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
                self.core.base.mouse_up_signal()
            }
            fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
                self.core.base.key_down_signal()
            }
            fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
                self.core.base.key_up_signal()
            }
            fn focus_gained_signal(&self) -> &GenericSignal {
                self.core.base.focus_gained_signal()
            }
            fn focus_lost_signal(&self) -> &GenericSignal {
                self.core.base.focus_lost_signal()
            }
            fn redraw_requested_signal(&self) -> &GenericSignal {
                self.core.base.redraw_requested_signal()
            }
            fn layout_requested_signal(&self) -> &GenericSignal {
                self.core.base.layout_requested_signal()
            }
        }
    };
}

pub(crate) use delegate_widget;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webview::privacy::{BrowsingData, Cookie};
    use rust_widgets::core::Rect;

    #[test]
    fn test_web_view_core_new() {
        let core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        assert_eq!(core.url(), "about:blank");
        assert!(!core.is_loading());
        assert_eq!(core.title(), "");
        assert_eq!(core.load_progress(), 0);
        assert!(core.settings().javascript_enabled);
        assert!(core.security().block_popups);
    }

    /// The simulated loader finishes synchronously, which is only true without `web-http`.
    #[cfg(not(feature = "web-http"))]
    #[test]
    fn test_web_view_core_set_url() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_url("https://example.com".to_string());
        assert_eq!(core.url(), "https://example.com");
        // set_url simulates loading and completing
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 100);
    }

    /// With `web-http`, an HTTP navigation is in flight until `poll_load` delivers it.
    #[cfg(feature = "web-http")]
    #[test]
    fn http_navigation_starts_in_flight_without_blocking() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_url("https://example.invalid".to_string());
        assert_eq!(core.url(), "https://example.invalid");
        assert!(core.is_loading(), "the fetch runs on a helper thread, not inline");
    }

    #[test]
    fn test_web_view_core_duplicate_url() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "https://example.com",
        );
        // The navigation is a no-op: it must not fabricate a completed load.
        core.set_url("https://example.com".to_string());
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 0, "nothing was loaded, so nothing completed");
        assert!(core.history.current().is_none(), "a repeat navigation records no history entry");
    }

    /// Repeating the current URL while its load is in flight leaves that load alone.
    ///
    /// The old early return set `load_progress` to 100 without clearing `loading`, so the view
    /// simultaneously claimed a finished and an in-flight load. A no-op keeps the in-flight
    /// load and its progress intact and emits no second `loading_started`.
    #[test]
    fn repeating_the_current_url_while_loading_keeps_the_load() {
        use std::sync::{Arc, Mutex};

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "https://example.com",
        );
        core.loading = true;
        core.load_progress = 40;
        let started = Arc::new(Mutex::new(0usize));
        let sink = started.clone();
        core.loading_started.connect(move |_| {
            *sink.lock().unwrap() += 1;
        });

        core.set_url("https://example.com".to_string());

        assert!(core.is_loading(), "the original load is still in flight");
        assert_eq!(core.load_progress(), 40, "its progress is not overwritten");
        assert_eq!(*started.lock().unwrap(), 0, "no second loading_started is emitted");
    }

    /// A simulation-engine failure is reported through `error_occurred`.
    ///
    /// The failure path used to stop at a log line, so a host that injected an engine to
    /// simulate a network error never learned that the navigation failed.
    #[test]
    fn a_simulation_failure_is_reported_through_error_occurred() {
        use std::sync::{Arc, Mutex};

        struct FailingEngine;
        impl SimulationEngine for FailingEngine {
            fn simulate_navigation(&mut self, _url: &str) -> Result<String, String> {
                Err(String::from("simulated network failure"))
            }
        }

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.simulation_engine = Some(Box::new(FailingEngine));
        let reported = Arc::new(Mutex::new(Vec::new()));
        let sink = reported.clone();
        core.error_occurred.connect(move |message: Arc<String>| {
            sink.lock().unwrap().push((*message).clone());
        });

        core.set_url("https://example.com/fails".to_string());

        assert_eq!(core.url(), "about:blank", "a failed simulation does not navigate");
        assert!(!core.is_loading());
        let messages = reported.lock().unwrap();
        assert_eq!(messages.len(), 1, "exactly one failure is reported");
        assert!(messages[0].contains("simulated network failure"), "got {messages:?}");
    }

    #[test]
    fn test_web_view_core_load_url() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_url("https://rust-lang.org");
        assert_eq!(core.url(), "https://rust-lang.org");
    }

    #[test]
    fn test_web_view_core_load_html() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_html("<h1>Hello</h1>", Some("https://base.url"));
        assert_eq!(core.url(), "https://base.url");
        assert_eq!(core.title(), "HTML Content");
        assert_eq!(core.content(), "<h1>Hello</h1>");
        assert!(!core.is_loading());
    }

    #[test]
    fn test_web_view_core_load_data() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_data(b"raw data", "text/plain", "https://data.url");
        assert_eq!(core.url(), "https://data.url");
        assert_eq!(core.title(), "Data: text/plain");
        assert_eq!(core.content(), "raw data");
    }

    /// An HTML document is a navigation, so Back returns to it.
    #[test]
    fn an_html_load_is_reachable_through_back() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_html("<p>first</p>", None);
        core.set_url("https://next.invalid/".to_string());
        assert!(core.can_go_back(), "the HTML document is behind the new URL");

        core.go_back();
        assert_eq!(core.url(), "data:text/html", "Back restores the loaded document's URL");
        assert!(!core.can_go_back(), "there is nothing older than the first document");
    }

    /// A data payload is a navigation too, and keeps its base URL in session history.
    #[test]
    fn a_data_load_is_reachable_through_back() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_data(b"payload", "text/plain", "https://data.url");
        core.set_url("https://next.invalid/".to_string());
        assert!(core.can_go_back());

        core.go_back();
        assert_eq!(core.url(), "https://data.url");
    }

    /// A document load is recorded in the panel-facing browser history too.
    ///
    /// `load_html`/`load_data` called `history.navigate` (session stack) but never
    /// `browser_history.add_entry`, so the two models described different sessions.
    #[test]
    fn a_document_load_enters_the_browser_history() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_html("<p>one</p>", Some("https://doc.url"));
        assert_eq!(core.browser_history().len(), 1);
        assert_eq!(core.browser_history().entries()[0].url, "https://doc.url");

        core.load_data(b"data", "text/plain", "https://data.url");
        assert_eq!(core.browser_history().len(), 2, "a data load is a visit as well");
    }

    /// A title a host sets belongs to the page, so Back restores the host's title.
    #[test]
    fn a_host_title_survives_a_back_navigation() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_html("<p>one</p>", None);
        core.set_title("Mine".to_string());
        core.set_url("https://next.invalid/".to_string());

        core.go_back();
        assert_eq!(
            core.title(),
            "Mine",
            "the snapshot carries the title the page had when it was left"
        );
    }

    /// Back restores the body the previous entry displayed, not just its URL.
    ///
    /// Two anonymous `load_html` documents share the `data:text/html` URL by contract, so the
    /// snapshot — not the URL — is what tells them apart: without it Back moved the address and
    /// left the document being left on screen.
    #[test]
    fn going_back_restores_the_previous_documents_body() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_html("<p>first</p>", None);
        core.load_html("<p>second</p>", None);
        assert_eq!(core.content(), "<p>second</p>");

        core.go_back();
        assert_eq!(core.url(), "data:text/html");
        assert_eq!(core.content(), "<p>first</p>", "the entry's own body is restored");
        assert_eq!(core.title(), "HTML Content");

        core.go_forward();
        assert_eq!(core.content(), "<p>second</p>", "forward restores the newer body too");
    }

    /// A data load records its body as well, so Back replays the payload.
    #[test]
    fn going_back_restores_a_data_payload() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_data(b"first payload", "text/plain", "https://data.url");
        core.load_data(b"second payload", "text/plain", "https://data2.url");

        core.go_back();
        assert_eq!(core.url(), "https://data.url");
        assert_eq!(core.content(), "first payload");
        assert_eq!(core.title(), "Data: text/plain");
    }

    /// A data load announces the URL it moved to, like the HTML loader does.
    #[test]
    fn a_data_load_announces_the_new_url() {
        use std::sync::{Arc, Mutex};

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        core.url_changed.connect(move |url: Arc<String>| sink.lock().unwrap().push((*url).clone()));

        core.load_data(b"payload", "text/plain", "https://data.url");
        assert_eq!(*seen.lock().unwrap(), ["https://data.url"]);
        assert_eq!(core.url(), "https://data.url");
    }

    /// Clearing history publishes the disabled back/forward state.
    #[test]
    fn clearing_history_announces_the_disabled_back_button() {
        use std::sync::{Arc, Mutex};

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_url("https://page1.invalid".to_string());
        core.set_url("https://page2.invalid".to_string());
        assert!(core.can_go_back());

        let seen = Arc::new(Mutex::new(Vec::<(bool, bool)>::new()));
        let sink = seen.clone();
        core.navigation_state_changed.connect(move |state: Arc<(bool, bool)>| {
            sink.lock().unwrap().push(*state);
        });

        core.clear_browsing_data(BrowsingData {
            cookies: false,
            history: true,
            ..Default::default()
        });
        assert!(!core.can_go_back());
        assert_eq!(*seen.lock().unwrap(), [(false, false)], "the reset is published");
    }

    /// A navigation whose fetch is still in flight publishes the new back/forward state.
    ///
    /// Only the completion is deferred; `history.navigate` has already changed the stacks, and
    /// `stop()` ends the load without reaching `apply_http_outcome`.
    #[cfg(feature = "web-http")]
    #[test]
    fn an_in_flight_navigation_announces_the_back_state() {
        use std::sync::{Arc, Mutex};

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "https://first.example",
        );
        let seen = Arc::new(Mutex::new(Vec::<(bool, bool)>::new()));
        let sink = seen.clone();
        core.navigation_state_changed.connect(move |state: Arc<(bool, bool)>| {
            sink.lock().unwrap().push(*state);
        });

        // A simulated navigation first, so the in-flight hop has a page behind it.
        core.set_url("file://first".to_string());
        // Loopback port 9 refuses immediately, so the helper thread does not linger.
        core.set_url("http://127.0.0.1:9/".to_string());
        assert!(core.can_go_back());
        assert!(
            seen.lock().unwrap().contains(&(true, false)),
            "the new back state is published before the completion events: {:?}",
            seen.lock().unwrap()
        );
        core.stop();
    }

    /// A navigation that produced no body of its own does not fabricate one for the history entry.
    ///
    /// The simulated path leaves the old document on screen (no source to fetch from), so recording
    /// that body under the new URL would make a later Back claim the new page showed the old one.
    #[test]
    fn a_simulated_navigation_does_not_snapshot_the_previous_body() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_html("<p>only</p>", None);
        core.set_url("file://next".to_string());
        assert_eq!(core.content(), "<p>only</p>", "the simulated path leaves the body alone");

        core.go_back();
        assert_eq!(core.content(), "<p>only</p>", "the HTML entry restores its own body");
        core.go_forward();
        assert_eq!(core.content(), "<p>only</p>", "no body was cached for the file navigation");
        assert_eq!(core.history.current_page().map(|page| page.title.as_str()), None);
    }

    #[cfg(feature = "web-http")]
    #[test]
    fn non_http_navigation_discards_superseded_http_results() {
        fn assert_invalidates_pending_load(
            core: &mut WebViewCore,
            navigate: impl FnOnce(&mut WebViewCore),
        ) {
            let (sender, receiver) = std::sync::mpsc::channel();
            let stale_generation = core.load_generation.wrapping_add(1);
            core.load_generation = stale_generation;
            core.pending_load = Some(receiver);

            navigate(core);

            assert!(core.pending_load.is_none());
            assert_ne!(core.load_generation, stale_generation);
            let expected = (
                core.url.clone(),
                core.title.clone(),
                core.content.clone(),
                core.loading,
                core.load_progress,
            );
            core.apply_http_outcome(
                stale_generation,
                Ok("<title>stale</title><p>stale response</p>".to_string()),
            );
            assert_eq!(
                (
                    core.url.clone(),
                    core.title.clone(),
                    core.content.clone(),
                    core.loading,
                    core.load_progress,
                ),
                expected
            );
            drop(sender);
        }

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "https://old.example",
        );

        assert_invalidates_pending_load(&mut core, |core| {
            core.set_url("file://new-document".to_string());
        });
        assert_invalidates_pending_load(&mut core, |core| {
            core.load_html("<p>new HTML</p>", Some("data:text/html"));
        });
        assert_invalidates_pending_load(&mut core, |core| {
            core.load_data(b"new data", "text/plain", "data:text/plain");
        });

        core.history.navigate("file://history-back".to_string());
        core.history.navigate("file://history-forward".to_string());
        assert_invalidates_pending_load(&mut core, |core| core.go_back());
        assert_invalidates_pending_load(&mut core, |core| core.go_forward());
        assert_invalidates_pending_load(&mut core, |core| core.reload());
    }

    #[test]
    fn test_web_view_core_go_back_forward() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        assert!(!core.can_go_back());
        assert!(!core.can_go_forward());

        core.set_url("https://page1.com".to_string());
        core.set_url("https://page2.com".to_string());
        assert!(core.can_go_back());
        assert!(!core.can_go_forward());

        core.go_back();
        // After going back, we can go forward
        assert!(core.can_go_forward());
        assert_eq!(core.url(), "https://page1.com");

        core.go_forward();
        assert_eq!(core.url(), "https://page2.com");
    }

    #[test]
    fn test_web_view_core_reload() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        // reload on empty URL does nothing
        core.reload();
        assert!(!core.is_loading());

        core.load_url("https://example.com");
        core.reload();
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 100);
    }

    #[test]
    fn test_web_view_core_stop() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        // stop when not loading is a no-op
        core.stop();
        assert!(!core.is_loading());

        // Force loading state then stop
        core.loading = true;
        core.stop();
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 0);
    }

    #[test]
    fn test_web_view_core_set_title() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        assert_eq!(core.title(), "");
        core.set_title("New Title".to_string());
        assert_eq!(core.title(), "New Title");
    }

    #[test]
    fn test_web_view_core_evaluate_javascript_disabled() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_javascript_enabled(false);
        let result = core.evaluate_javascript("var x = 1;");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("JavaScript is disabled"));
    }

    #[test]
    fn test_web_view_core_evaluate_javascript_enabled() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_javascript_enabled(true);
        let result = core.evaluate_javascript("42");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), JsValue::Number(42.0));
    }

    /// Each evaluation reports only the console output it produced.
    ///
    /// The context accumulates every message for the life of the view, so forwarding the
    /// whole slice on every call re-emitted the first script's output when the second one
    /// ran — a duplicated signal for output the host had already handled.
    #[test]
    fn console_messages_are_reported_once_per_evaluation() {
        use std::sync::{Arc, Mutex};

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_javascript_enabled(true);
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        core.console_message.connect(move |value: Arc<(String, u32, String)>| {
            sink.lock().unwrap().push(value.2.clone());
        });

        core.evaluate_javascript("console.log(\"first\")").unwrap();
        assert_eq!(*messages.lock().unwrap(), ["first"], "only the first script's output");

        core.evaluate_javascript("console.log(\"second\")").unwrap();
        assert_eq!(
            *messages.lock().unwrap(),
            ["first", "second"],
            "the second call must not replay the first message"
        );
    }

    /// Stepping through session history announces the URL the view moved to.
    ///
    /// `set_url` emitted `url_changed` but `go_back`/`go_forward` changed the URL silently,
    /// so an address bar bound to the signal kept showing the page that was left.
    #[test]
    fn going_through_history_announces_the_new_url() {
        use std::sync::{Arc, Mutex};

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_url("https://page1.invalid".to_string());
        core.set_url("https://page2.invalid".to_string());

        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        core.url_changed.connect(move |value: Arc<String>| {
            sink.lock().unwrap().push((*value).clone());
        });

        core.go_back();
        assert_eq!(*seen.lock().unwrap(), ["https://page1.invalid"]);
        core.go_forward();
        assert_eq!(
            *seen.lock().unwrap(),
            ["https://page1.invalid", "https://page2.invalid"],
            "forward reports where it landed"
        );
    }

    #[test]
    fn test_web_view_core_clear_browsing_data_cookies_only() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.cookies.add(Cookie::new(
            "test".to_string(),
            "value".to_string(),
            "example.com".to_string(),
        ));
        assert!(!core.cookies.is_empty());

        core.clear_browsing_data(BrowsingData {
            cookies: true,
            history: false,
            ..Default::default()
        });
        assert!(core.cookies.is_empty());
    }

    #[cfg(not(feature = "web-http"))]
    #[test]
    fn test_web_view_core_clear_browsing_data_history_only() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        // Navigate to two URLs so the session history has a back stack
        core.set_url("https://page1.com".to_string());
        core.set_url("https://example.com".to_string());
        assert!(!core.browser_history.is_empty());
        assert!(core.history.can_go_back());

        core.clear_browsing_data(BrowsingData {
            cookies: false,
            history: true,
            ..Default::default()
        });
        assert!(core.browser_history.is_empty());
        assert!(!core.history.can_go_back());
    }

    #[test]
    fn test_web_view_core_handle_key_event_back() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_url("https://page1.com".to_string());
        core.set_url("https://page2.com".to_string());
        assert!(core.can_go_back());

        // Left arrow key (37) should trigger go_back
        core.handle_key_event(37, 0);
        assert_eq!(core.url(), "https://page1.com");
    }

    #[test]
    fn test_web_view_core_handle_key_event_forward() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_url("https://page1.com".to_string());
        core.set_url("https://page2.com".to_string());
        core.go_back();
        assert!(core.can_go_forward());

        // Right arrow key (39) should trigger go_forward
        core.handle_key_event(39, 0);
        assert_eq!(core.url(), "https://page2.com");
    }

    #[test]
    fn test_web_view_core_handle_key_event_reload() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_url("https://example.com");
        // F5 key (116) should trigger reload
        core.handle_key_event(116, 0);
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 100);
    }

    #[test]
    fn test_web_view_core_handle_key_event_ctrl_r() {
        use std::sync::{Arc, Mutex};

        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_url("https://example.com");
        // Count reloads by their first observable step: every reload emits `loading_started`.
        let reloads = Arc::new(Mutex::new(0usize));
        let sink = reloads.clone();
        core.loading_started.connect(move |_| {
            if let Ok(mut count) = sink.lock() {
                *count += 1;
            }
        });

        // Shift+R (bit 0) must not reload — it used to, because `1` was read as Ctrl.
        core.handle_key_event(82, 1);
        assert_eq!(*reloads.lock().unwrap(), 0, "Shift+R must not reload the page");

        // Ctrl+R (bit 1, value 2) must reload.
        core.handle_key_event(82, 2);
        assert_eq!(*reloads.lock().unwrap(), 1, "Ctrl+R must reload the page");
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 100);
    }

    #[test]
    fn test_web_view_core_initial_navigation_state() {
        let core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 1024, 768),
            "engine_view",
            "",
        );
        assert!(!core.can_go_back());
        assert!(!core.can_go_forward());
        assert_eq!(core.url(), "");
    }

    #[test]
    fn test_web_view_core_navigation_state_changes() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(10, 10, 640, 480),
            "nav_test",
            "about:blank",
        );
        core.set_url("https://page1.com".to_string());
        core.set_url("https://site-a.com".to_string());
        // After two navigations, we can go back
        assert!(core.can_go_back());
        assert!(!core.can_go_forward());

        core.go_back();
        // After going back, we can go forward again
        assert!(core.can_go_forward());
        assert_eq!(core.url(), "https://page1.com");
    }

    #[test]
    fn test_web_view_core_signals_are_initialized() {
        let core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "signals_test",
            "",
        );
        // Signals should exist and be ready to connect
        let _ = &core.loading_started;
        let _ = &core.loading_finished;
        let _ = &core.loading_progress;
        let _ = &core.title_changed;
        let _ = &core.url_changed;
        let _ = &core.navigation_state_changed;
        let _ = &core.console_message;
    }
}
