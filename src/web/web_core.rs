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

#[cfg(not(alloc_frugal))]
use super::history::{BrowserHistory, SessionHistory};
#[cfg(not(alloc_frugal))]
use super::js_engine::{JsContext, JsEngine, JsResult, JsValue, SimpleJsEngine};
#[cfg(not(alloc_frugal))]
use super::plugins::PluginManager;
#[cfg(not(alloc_frugal))]
use super::privacy::{CookieJar, PrivacySettings, TrackingProtection};
#[cfg(not(alloc_frugal))]
use crate::core::Rect;
#[cfg(not(alloc_frugal))]
use crate::signal::Signal1;
#[cfg(not(alloc_frugal))]
use crate::widget::{BaseWidget, WidgetKind};

/// A pluggable simulation engine that can be set externally (e.g. in tests)
/// to inject mock content or simulate different network conditions.
#[cfg(not(alloc_frugal))]
pub trait SimulationEngine: Send {
    /// Simulate navigation to the given URL and return simulated content.
    fn simulate_navigation(&mut self, url: &str) -> Result<String, String>;
}

/// The result of one asynchronous HTTP fetch, delivered from the helper thread.
///
/// Carries the generation it was started for so [`WebViewCore::poll_load`] can discard a result whose
/// navigation has since been superseded.
#[cfg(all(not(alloc_frugal), feature = "web-http"))]
struct HttpLoadOutcome {
    generation: u64,
    result: Result<String, String>,
}

/// Performs one blocking HTTP GET and returns the response body, or a message describing the failure.
///
/// This is the only part that blocks, and it runs on a helper thread — never on the UI thread — so a
/// slow server cannot freeze the caller. Keeping it a free function makes it obvious that it touches
/// no view state; the thread's only output is the returned `Result`, delivered over a channel.
#[cfg(all(not(alloc_frugal), feature = "web-http"))]
fn fetch_http_content(url: &str) -> Result<String, String> {
    match ureq::get(url).call() {
        Ok(response) => response
            .into_body()
            .read_to_string()
            .map_err(|e| format!("Failed to read response body from '{url}': {e}")),
        Err(e) => Err(format!("HTTP request failed for '{url}': {e}")),
    }
}

/// Shared fields used by both WebEngineViewEnhanced and WebViewEnhanced.
#[cfg(not(alloc_frugal))]
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
    /// Emitted when a load fails, through [`WebEngine::report_error`].
    ///
    /// Was named `_error_occurred`, which marked it as unused while it was still a
    /// `pub` field: a leading underscore on a public item tells readers the wrong
    /// thing and hides the field from a search for `error_occurred`. The name is now
    /// the one callers would look for.
    pub error_occurred: Signal1<String>,
    pub navigation_state_changed: Signal1<(bool, bool)>,
    pub console_message: Signal1<(String, u32, String)>,
    pub content: String,
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

#[cfg(not(alloc_frugal))]
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
            // URL already set — mark as fully loaded.
            self.load_progress = 100;
            return;
        }

        // Delegate to simulation engine if one is set (primarily for testing).
        if let Some(ref mut engine) = self.simulation_engine {
            match engine.simulate_navigation(&url) {
                Ok(content) => {
                    self.content = content;
                }
                Err(e) => {
                    log::error!("[web] simulation engine error for '{url}': {e}");
                    return;
                }
            }
        }

        #[cfg(feature = "web-http")]
        self.invalidate_pending_load();

        self.url = url.clone();
        self.loading = true;
        self.load_progress = 0;
        self.url_changed.emit(url.clone());
        self.loading_started.emit(url.clone());
        self.history.navigate(url.clone());

        // ── Real HTTP fetch (web-http feature) ──
        //
        // The request runs on a helper thread and its outcome is delivered through a channel drained
        // by [`Self::poll_load`]. Running it inline here blocked the caller for the whole round trip
        // (a slow server froze the UI) and made a superseded navigation impossible to abandon; a
        // superseded load is now dropped by generation rather than by luck of timing.
        #[cfg(feature = "web-http")]
        {
            if url.starts_with("http://") || url.starts_with("https://") {
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
                // file:// URL fallback — simulated progress.
                self.load_progress = 50;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // ── Simulated mode (no web-http feature) ──
        #[cfg(not(feature = "web-http"))]
        {
            // Emit progress at 50%.
            self.load_progress = 50;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // When an asynchronous HTTP fetch is in flight, the load is *not* finished here: the 100%
        // progress and `loading_finished` are emitted by [`Self::poll_load`] when the result arrives
        // (or by the error path when it fails). Emitting them inline would announce a completion for a
        // page whose bytes have not been read yet.
        #[cfg(feature = "web-http")]
        if self.pending_load.is_some() {
            return;
        }

        // Emit progress at 100% and finish.
        self.load_progress = 100;
        self.loading = false;
        self.loading_progress.emit(self.load_progress);
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.update_navigation_state();
        self.browser_history.add_entry(self.url.clone(), self.title.clone());
        self.base.request_redraw();
    }

    /// SIMULATED: No real web engine — loads HTML content with simulated
    /// 0% → 100% progress callbacks.
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        #[cfg(feature = "web-http")]
        self.invalidate_pending_load();

        self.url = base_url.unwrap_or("data:text/html").to_string();
        self.title = "HTML Content".to_string();
        self.loading = true;
        self.load_progress = 0;
        self.loading_started.emit(self.url.clone());
        self.content = html.to_string();

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
        self.base.request_redraw();
    }

    /// SIMULATED: No real web engine — loads binary data as a string with simulated
    /// 0% → 50% → 100% progress callbacks.
    pub fn load_data(&mut self, data: &[u8], mime_type: &str, base_url: &str) {
        #[cfg(feature = "web-http")]
        self.invalidate_pending_load();

        self.url = base_url.to_string();
        self.title = format!("Data: {mime_type}");
        self.loading = true;
        self.load_progress = 0;
        self.loading_started.emit(self.url.clone());
        self.content = String::from_utf8_lossy(data).to_string();

        // Emit progress at 50%.
        self.load_progress = 50;
        self.loading_progress.emit(self.load_progress);

        // Emit progress at 100% and finish.
        self.load_progress = 100;
        self.loading = false;
        self.loading_progress.emit(self.load_progress);
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }

    /// SIMULATED: No real web engine — navigates back in history with simulated
    /// loading callbacks.
    pub fn go_back(&mut self) {
        if let Some(url) = self.history.go_back() {
            #[cfg(feature = "web-http")]
            self.invalidate_pending_load();

            self.url = url;
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
            self.load_progress = 0;
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }

    pub fn set_title(&mut self, title: String) {
        if self.title != title {
            self.title = title.clone();
            self.title_changed.emit(title);
        }
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
        Ok(result)
    }

    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.settings.javascript_enabled = enabled;
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn html(&self) -> &str {
        &self.content
    }

    pub fn clear_browsing_data(&mut self, data: super::privacy::BrowsingData) {
        if data.cookies {
            self.cookies.clear();
        }
        if data.history {
            self.browser_history.clear();
            self.history.clear();
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
                self.content = body;
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
                self.browser_history.add_entry(self.url.clone(), self.title.clone());
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
            82 if modifiers == 1 => {
                self.reload();
            }
            _ => { /* Other keys are not relevant */ }
        }
    }
}

// ---------------------------------------------------------------------------
// Widget trait delegation for WebViewCore (used by both wrappers)
// ---------------------------------------------------------------------------
#[cfg(not(alloc_frugal))]
macro_rules! delegate_widget {
    ($wrapper:ty) => {
        impl Widget for $wrapper {
            fn base(&self) -> &crate::widget::BaseWidget {
                &self.core.base
            }
            fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
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

#[cfg(not(alloc_frugal))]
pub(crate) use delegate_widget;

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::web::privacy::{BrowsingData, Cookie};

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

    #[test]
    fn test_web_view_core_duplicate_url() {
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "https://example.com",
        );
        // First call sets the URL.
        assert_eq!(core.url(), "https://example.com");
        // Second call with same URL — load should be short-circuited.
        core.set_url("https://example.com".to_string());
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 100);
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
        let mut core = WebViewCore::new(
            WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_url("https://example.com");
        // 'R' key (82) with Ctrl modifier (1) should trigger reload
        core.handle_key_event(82, 1);
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
