// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Simulated web-engine view widget (self-drawn chrome).
//!
//! `WebEngineView` draws a browser-style chrome (URL bar, content hint) and
//! models navigation state (loading/back/forward). It does **not** contain a
//! DOM layout or network stack, so page content itself is never rendered.
//!
//! JavaScript evaluation is real: with the `js-engine` feature the widget
//! evaluates scripts in a genuine embedded engine (boa_engine via
//! `crate::web::SimpleJsEngine`) and returns the actual result or error.
//! Without that feature, evaluation reports `Err` instead of faking success.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// First page id [`WebEngineView::create_page`] issues.
///
/// Deliberately not `0`: that is the "no widget" sentinel the runtime and the C ABI
/// use, so issuing it as a page id would make an absent page indistinguishable from
/// a real one.
pub const FIRST_PAGE_ID: ObjectId = 1;

/// Web engine view widget for web content rendering.
pub struct WebEngineView {
    base: BaseWidget,
    url: String,
    loading: bool,
    pending_load: bool,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
    javascript_enabled: bool,
    plugins_enabled: bool,
    private_browsing: bool,
    /// Emitted when the page starts loading.
    pub loading_started: Signal1<String>,
    /// Emitted when the page finishes loading.
    pub loading_finished: Signal1<String>,
    /// Emitted when the title changes.
    pub title_changed: Signal1<String>,
    /// Emitted when the URL changes.
    pub url_changed: Signal1<String>,
    /// Emitted when an error occurs.
    pub error_occurred: Signal1<String>,
    /// Emitted when the navigation state changes.
    pub navigation_state_changed: Signal1<(bool, bool)>,
    /// Emitted when a certificate error occurs.
    pub certificate_error: Signal1<String>,
    /// Emitted when a JavaScript console message is received.
    pub console_message: Signal1<(String, u32, String)>,
    /// Emitted when a download is requested.
    pub download_requested: Signal1<String>,
    /// Emitted when the page is created.
    pub page_created: Signal1<ObjectId>,
    /// Emitted when the page is destroyed.
    pub page_destroyed: Signal1<ObjectId>,
    /// Most recently loaded HTML source (kept for inspection; no DOM layout).
    html_content: String,
    /// Next id [`WebEngineView::create_page`] will issue.
    ///
    /// Starts at [`FIRST_PAGE_ID`] so a page id is never `0`: `0` is the "no widget"
    /// sentinel the C ABI and the runtime use, and handing one out as a page id
    /// would make an absent page indistinguishable from a real one.
    next_page_id: ObjectId,
    /// Embedded real JavaScript engine (boa_engine) for script evaluation.
    #[cfg(feature = "js-engine")]
    js_engine: Option<crate::web::BoaJsEngine>,
}
// Newtype structs for render pipeline symbol imports, wrapping WebEngineView.
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "page" symbol name. Behaves identically to the wrapped view.
pub struct WebEnginePage(pub WebEngineView);
impl WebEnginePage {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "engine" symbol name. Behaves identically to the wrapped view.
pub struct WebEngine(pub WebEngineView);
impl WebEngine {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "settings" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineSettings(pub WebEngineView);
impl WebEngineSettings {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "download item" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineDownloadItem(pub WebEngineView);
impl WebEngineDownloadItem {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "cookie store" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineCookieStore(pub WebEngineView);
impl WebEngineCookieStore {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "web channel" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineWebChannel(pub WebEngineView);
impl WebEngineWebChannel {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "find text result" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineFindTextResult(pub WebEngineView);
impl WebEngineFindTextResult {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "notification" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineNotification(pub WebEngineView);
impl WebEngineNotification {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "script dialog" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineScriptDialog(pub WebEngineView);
impl WebEngineScriptDialog {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Newtype wrapper exposing [`WebEngineView`] under the render pipeline's
/// "context menu request" symbol name. Behaves identically to the wrapped view.
pub struct WebEngineContextMenuRequest(pub WebEngineView);
impl WebEngineContextMenuRequest {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}

macro_rules! impl_web_engine_wrapper_traits {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Widget for $ty {
                fn base(&self) -> &BaseWidget {
                    self.0.base()
                }

                fn base_mut(&mut self) -> &mut BaseWidget {
                    self.0.base_mut()
                }
            }

            impl EventHandler for $ty {
                fn handle_event(&mut self, event: &Event) {
                    self.0.handle_event(event);
                }
            }

            impl Draw for $ty {
                fn draw(&mut self, ctx: &mut RenderContext) {
                    self.0.draw(ctx);
                }

                fn uses_custom_drawing(&self) -> bool {
                    self.0.uses_custom_drawing()
                }
            }
        )+
    };
}

impl_web_engine_wrapper_traits!(
    WebEnginePage,
    WebEngine,
    WebEngineSettings,
    WebEngineDownloadItem,
    WebEngineCookieStore,
    WebEngineWebChannel,
    WebEngineFindTextResult,
    WebEngineNotification,
    WebEngineScriptDialog,
    WebEngineContextMenuRequest,
);

impl WebEngineView {
    const LOAD_TIMER_ID: u32 = 1;

    /// Creates a view with empty URL, title and HTML source, not loading, with
    /// JavaScript enabled and plugins, private browsing and navigation history
    /// all off.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineView, geometry, "WebEngineView"),
            url: "".to_string(),
            loading: false,
            pending_load: false,
            title: "".to_string(),
            can_go_back: false,
            can_go_forward: false,
            javascript_enabled: true,
            plugins_enabled: false,
            private_browsing: false,
            loading_started: Signal1::new(),
            loading_finished: Signal1::new(),
            title_changed: Signal1::new(),
            url_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
            certificate_error: Signal1::new(),
            console_message: Signal1::new(),
            download_requested: Signal1::new(),
            page_created: Signal1::new(),
            page_destroyed: Signal1::new(),
            html_content: String::new(),
            next_page_id: FIRST_PAGE_ID,
            #[cfg(feature = "js-engine")]
            js_engine: None,
        }
    }
    /// Returns the current URL, or an empty string when nothing has been loaded.
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Returns whether a load is in progress. Because a load completes only when
    /// the load timer tick arrives, this can remain `true` until then.
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    /// Returns the page title, or an empty string when none has been set.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets the page title, emitting [`WebEngineView::title_changed`] and
    /// requesting a redraw only when the value actually differs.
    pub fn set_title(&mut self, title: String) {
        if self.title != title {
            self.title = title.clone();
            self.title_changed.emit(title);
            self.base.request_redraw();
        }
    }
    /// Returns whether going back is currently available. The flag reflects the
    /// simulated navigation state, not a real history stack.
    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }
    /// Returns whether going forward is currently available.
    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }
    /// Returns whether JavaScript evaluation is permitted for this view.
    pub fn is_javascript_enabled(&self) -> bool {
        self.javascript_enabled
    }
    /// Returns whether plugin content is enabled for this view.
    pub fn is_plugins_enabled(&self) -> bool {
        self.plugins_enabled
    }
    /// Returns whether the view is in private browsing mode.
    pub fn is_private_browsing(&self) -> bool {
        self.private_browsing
    }

    /// Returns the timer id the widget registers to complete a pending load;
    /// callers driving the event loop treat that timer's tick as the signal to
    /// finish loading.
    pub fn load_timer_id() -> u32 {
        Self::LOAD_TIMER_ID
    }

    fn begin_loading(&mut self) {
        self.loading = true;
        self.pending_load = true;
        self.loading_started.emit(self.url.clone());
        self.base.request_redraw();
    }

    fn finish_loading(&mut self) {
        if self.loading {
            self.loading = false;
            self.pending_load = false;
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }

    /// Replaces the URL and starts a simulated load, emitting
    /// [`WebEngineView::url_changed`] then [`WebEngineView::loading_started`] and
    /// refreshing the navigation flags. Does nothing if `url` is already current.
    pub fn set_url(&mut self, url: String) {
        if self.url != url {
            self.url = url.clone();
            self.url_changed.emit(url.clone());
            self.begin_loading();
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
    /// "Loads" the supplied HTML synchronously: stores the source for
    /// [`WebEngineView::html_source`], sets the URL to `data:text/html` and the
    /// title to `HTML Content`, then emits the load, title, URL and navigation
    /// signals in that order. No DOM is built and none of the HTML is rendered.
    pub fn load_html(&mut self, html: &str) {
        // Navigation and loading state are simulated (no DOM layout), but the
        // HTML source is retained so script evaluation and introspection work.
        self.html_content = html.to_string();
        self.url = "data:text/html".to_string();
        self.title = "HTML Content".to_string();
        self.begin_loading();
        self.finish_loading();
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }
    /// Return the most recently loaded HTML source.
    pub fn html_source(&self) -> &str {
        &self.html_content
    }
    /// Loads an in-memory payload, storing `data` decoded as lossy UTF-8, then
    /// setting the URL to `base_url` and the title to `Data Content`.
    ///
    /// The MIME type and encoding arguments are currently ignored because no
    /// decoding or layout is performed. Emits the same signals as `load_html`.
    pub fn load_data(&mut self, data: &[u8], _mime_type: &str, _encoding: &str, base_url: &str) {
        // Data payloads are retained for introspection; no binary media decode
        // or layout happens in this simulated view.
        self.html_content = String::from_utf8_lossy(data).into_owned();
        self.url = base_url.to_string();
        self.title = "Data Content".to_string();
        self.begin_loading();
        self.finish_loading();
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }
    /// Models a back navigation by flipping the availability flags: `can_go_back`
    /// becomes false and `can_go_forward` true. No history is kept and the URL is
    /// left unchanged. No-op when going back is unavailable.
    pub fn go_back(&mut self) {
        if self.can_go_back {
            // Navigation state is modeled, not a real page history: toggling
            // back/forward flips the availability flags.
            self.can_go_back = false;
            self.can_go_forward = true;
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
    /// Models a forward navigation with the same flag flip as `go_back` in
    /// reverse: `can_go_forward` becomes false and `can_go_back` true. No-op when
    /// going forward is unavailable.
    pub fn go_forward(&mut self) {
        if self.can_go_forward {
            // See `go_back` — modeled navigation state only.
            self.can_go_forward = false;
            self.can_go_back = true;
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
    /// Restarts the load for the current URL without fetching anything.
    ///
    /// Emits [`WebEngineView::loading_started`] again and marks a load pending, so
    /// `is_loading()` becomes `true`. It is completed the same way any other load is:
    /// the timer returned by [`WebEngineView::load_timer_id`] must be delivered, which
    /// emits [`WebEngineView::loading_finished`] and clears the flag. Call
    /// [`WebEngineView::stop`] to finish early.
    ///
    /// Nothing is fetched either way — this widget models the load lifecycle rather
    /// than performing network I/O.
    ///
    /// No-op when the URL is empty, because there is nothing to reload.
    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            self.begin_loading();
        }
    }
    /// Completes the load in progress, emitting [`WebEngineView::loading_finished`].
    /// No-op when nothing is loading.
    pub fn stop(&mut self) {
        self.finish_loading();
    }
    /// Evaluates `script` and returns its result rendered as a string.
    ///
    /// **Refused while JavaScript is disabled.** When
    /// [`WebEngineView::is_javascript_enabled`] is `false` this returns an `Err`
    /// without touching the engine, so the flag is enforced rather than advisory.
    /// Previously the flag was stored and never consulted, which meant a host that
    /// turned scripting off still executed any script it was handed — a security
    /// setting that silently did nothing.
    ///
    /// With the `js-engine` feature the script runs in a real embedded engine, created
    /// lazily on first use, and the engine's error text is returned on failure.
    /// Without that feature it always returns `Err`.
    pub fn evaluate_javascript(&mut self, script: &str) -> Result<String, String> {
        if !self.javascript_enabled {
            // Return before the engine is even created: a disabled page must not
            // have script of any kind evaluated for it.
            return Err("JavaScript is disabled for this view; call set_javascript_enabled(true) \
                 to permit evaluation"
                .to_string());
        }

        #[cfg(feature = "js-engine")]
        {
            let engine = self.js_engine.get_or_insert_with(crate::web::BoaJsEngine::new);
            engine.evaluate_to_string(script)
        }
        #[cfg(not(feature = "js-engine"))]
        {
            let _ = script;
            Err("JavaScript evaluation requires the `js-engine` feature".to_string())
        }
    }
    /// Sets whether JavaScript may be evaluated for this view.
    ///
    /// Enforced by [`WebEngineView::evaluate_javascript`], which refuses to run
    /// anything while this is `false`. Defaults to `true` at construction. It gates
    /// only this widget's own evaluation entry point; it is not a substitute for a
    /// sandbox in whatever engine the host may run elsewhere.
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.javascript_enabled = enabled;
    }
    /// Records whether plugin content is permitted. This is a state flag only and
    /// gates no rendering.
    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.plugins_enabled = enabled;
    }
    /// Records whether private browsing is in effect. This is a state flag only;
    /// no history or cookie state is cleared when it is set.
    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.private_browsing = enabled;
    }
    fn update_navigation_state(&self) {
        self.navigation_state_changed.emit((self.can_go_back, self.can_go_forward));
    }

    /// Reports a load failure, emitting [`WebEngineView::error_occurred`] and ending
    /// any load in progress.
    ///
    /// This widget performs no network I/O, so the failure is supplied by the host
    /// rather than discovered here. That is deliberate: a control that invented its
    /// own errors would report them at times unrelated to what the host actually
    /// observed. The load is ended first so a caller that reacts by retrying sees
    /// `loading == false` rather than a view that claims to still be loading.
    pub fn report_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        if self.loading {
            self.loading = false;
            self.pending_load = false;
        }
        self.base.request_redraw();
        self.error_occurred.emit(message);
    }

    /// Reports a TLS certificate problem, emitting
    /// [`WebEngineView::certificate_error`].
    ///
    /// Kept distinct from [`Self::report_error`] because the two call for different
    /// responses: a certificate failure is something a user may be asked to override
    /// once, while a load error is not.
    pub fn report_certificate_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.base.request_redraw();
        self.certificate_error.emit(message);
    }

    /// Records a JavaScript console message, emitting
    /// [`WebEngineView::console_message`] with `(message, line, source)`.
    pub fn report_console_message(
        &mut self,
        message: impl Into<String>,
        line: u32,
        source: impl Into<String>,
    ) {
        self.console_message.emit((message.into(), line, source.into()));
    }

    /// Reports a download request, emitting [`WebEngineView::download_requested`]
    /// with the URL.
    ///
    /// No file is fetched or written: the signal tells the host that a download was
    /// requested so it can decide what to do, which is the only part this widget can
    /// honestly do without an I/O stack.
    pub fn request_download(&mut self, url: impl Into<String>) {
        self.download_requested.emit(url.into());
    }

    /// Records that a child page was created, emitting
    /// [`WebEngineView::page_created`] with its id.
    ///
    /// The returned id is issued here rather than by the platform, because this
    /// widget models pages as bookkeeping and has no platform page objects to ask.
    pub fn create_page(&mut self) -> ObjectId {
        let id = self.next_page_id;
        self.next_page_id = self.next_page_id.saturating_add(1);
        self.page_created.emit(id);
        id
    }

    /// Records that the page `id` was destroyed, emitting
    /// [`WebEngineView::page_destroyed`].
    ///
    /// Returns `false` for an id this view never issued, so a host cannot make the
    /// view announce the destruction of a page it does not own.
    pub fn destroy_page(&mut self, id: ObjectId) -> bool {
        if id < FIRST_PAGE_ID || id >= self.next_page_id {
            return false;
        }
        self.page_destroyed.emit(id);
        true
    }
}
impl Widget for WebEngineView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `WebEngineView`'s property contract.
///
/// `WebEngineView` is a concrete type, not an alias: `WebView` is the alias
/// (`web_widgets/web_view.rs` re-exports `WebEngineView as WebView`), so this one
/// impl answers for the `WebEngineView` and `WebView` kinds alike.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch: `url`, `loading`
/// and `title` are readable, and none of them is writable through this contract.
/// (The old `WidgetKind::WebEngineView` write arm downcast to `MediaPlayer`, which
/// is why it answered `source` / `playing` / `position_ms`; those names belong to
/// `MediaPlayer` and are answered there.)
impl WidgetProperties for WebEngineView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "url" => Ok(CapabilityValue::String(self.url().to_string())),
            "loading" => Ok(CapabilityValue::Bool(self.is_loading())),
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "url" => {
                self.set_url(expect_string(value)?);
                Ok(())
            }
            // Loading is a state the load pipeline drives, not a value a caller
            // assigns; navigation is the way to change it (`set_url` / `reload`).
            "loading" => Err(CapabilityAccessError::ReadOnlyProperty),
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["url", "loading", "title", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `web_engine_view` publishes.
    ///
    /// `reload`, `go_back`, `go_forward` and `stop` are the browser's own
    /// payload-free navigation controls — the same four a toolbar's buttons drive —
    /// so a bare invocation performs them. Each is already guarded by the view's own
    /// state: going back with no history is a no-op, stopping when nothing is loading
    /// is a no-op, and reloading a view with no URL does nothing. Those are the right
    /// outcomes for a control and not errors, so they answer `Ok(())`: the command
    /// *ran*, it simply had nothing to change. Refusing them instead would make the
    /// four buttons a toolbar is built from unusable through the command layer on a
    /// freshly mounted view, which is the one state a toolbar always starts in.
    /// `load_url` needs the URL to load, which is the one thing the command name
    /// cannot supply, and `set_url` is the same write under its property name.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "reload" => {
                self.reload();
                Ok(())
            }
            "go_back" => {
                self.go_back();
                Ok(())
            }
            "go_forward" => {
                self.go_forward();
                Ok(())
            }
            "stop" => {
                self.stop();
                Ok(())
            }
            "load_url" => Err(CapabilityAccessError::OutOfRange),
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

use crate::render::RenderContext;
use crate::widget::Draw;

impl EventHandler for WebEngineView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if let Event::Timer { id } = event {
            if *id == Self::LOAD_TIMER_ID && self.pending_load {
                self.finish_loading();
            }
        }
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // A press on the view is reported through the shared `clicked` signal; the
            // host decides what it navigates to. Two defects were stacked here: the
            // position was discarded (so any click anywhere in the window navigated),
            // and the URL was the hard-coded literal `https://example.com/12345`,
            // identical on every press. A control cannot know which link was clicked
            // without a DOM hit-test, so inventing a URL was the wrong answer in both
            // respects — it is removed, and the click is reported instead.
            Event::MousePress { pos, button } if *button == 1 => {
                if self.geometry().contains_point(*pos) {
                    self.base.clicked.emit();
                }
            }
            Event::KeyPress { key, modifiers } => {
                match *key {
                    37 => {
                        // Left arrow — go back
                        self.go_back();
                    }
                    39 => {
                        // Right arrow — go forward
                        self.go_forward();
                    }
                    116 => {
                        // F5 — reload
                        self.reload();
                    }
                    82
                        // Ctrl+R — reload
                        if *modifiers == 1 => {
                            self.reload();
                        }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for WebEngineView {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let g = self.geometry();
        ctx.fill_rect(g, Color::WHITE);
        ctx.draw_rect(g, Color::rgb(200, 200, 200));
        // Draw URL bar
        let bar = Rect::new(g.x, g.y, g.width, 28);
        ctx.fill_rect(bar, Color::rgb(240, 240, 240));
        ctx.draw_text(
            Point::new(g.x + 4, g.y + 20),
            self.url(),
            &Font::default_ui(),
            Color::rgb(100, 100, 100),
            HorizontalAlignment::Left,
        );
        // Content area hint
        if self.is_loading() {
            ctx.draw_text(
                Point::new(g.x + 4, g.y + g.height as i32 / 2),
                "Loading...",
                &Font::default_ui(),
                Color::rgb(150, 150, 150),
                HorizontalAlignment::Left,
            );
        }
    }
    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::widget::WidgetKind;
    use std::sync::{Arc, Mutex};

    #[test]
    fn web_engine_view_draw_produces_svg() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        let svg = crate::widget::svg::render_to_svg(&mut wv);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn web_engine_view_url_and_title() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        assert!(wv.url().is_empty());
        assert!(wv.title().is_empty());
        wv.set_url("https://example.com".to_string());
        assert_eq!(wv.url(), "https://example.com");
    }

    #[test]
    fn web_engine_set_url_starts_then_finishes_on_timer() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        let started = Arc::new(Mutex::new(0usize));
        let finished = Arc::new(Mutex::new(0usize));

        let started_sink = started.clone();
        wv.loading_started.connect(move |_| {
            if let Ok(mut count) = started_sink.lock() {
                *count += 1;
            }
        });

        let finished_sink = finished.clone();
        wv.loading_finished.connect(move |_| {
            if let Ok(mut count) = finished_sink.lock() {
                *count += 1;
            }
        });

        wv.set_url("https://example.com".to_string());
        assert!(wv.is_loading());
        assert_eq!(*started.lock().expect("started lock poisoned"), 1);
        assert_eq!(*finished.lock().expect("finished lock poisoned"), 0);

        wv.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wv.is_loading());
        assert_eq!(*finished.lock().expect("finished lock poisoned"), 1);
    }

    #[test]
    fn web_engine_stop_finishes_pending_load() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        wv.set_url("https://rust-lang.org".to_string());
        assert!(wv.is_loading());

        wv.stop();
        assert!(!wv.is_loading());
    }

    /// Script evaluation must be refused while the flag is off.
    ///
    /// The defect this pins: `set_javascript_enabled(false)` stored a flag that
    /// `evaluate_javascript` never read, so a host that turned scripting off still
    /// executed whatever script it was handed — a security setting that silently did
    /// nothing. The assertion is on the refusal, which holds with or without the
    /// `js-engine` feature, so the test is meaningful in every build.
    #[test]
    fn evaluate_javascript_is_refused_while_disabled() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        assert!(wv.is_javascript_enabled(), "scripting is on by default");

        wv.set_javascript_enabled(false);
        let error =
            wv.evaluate_javascript("1 + 1").expect_err("a disabled view must not evaluate script");
        assert!(error.contains("disabled"), "the refusal must say why, got: {error:?}");

        // Re-enabling restores evaluation. With `js-engine` on it actually returns a
        // result; without it the failure must be the missing-feature one, never the
        // policy check — hence the two branches rather than an assumption.
        wv.set_javascript_enabled(true);
        match wv.evaluate_javascript("1 + 1") {
            Ok(value) => assert_eq!(value, "2", "the script must actually run once enabled"),
            Err(error) => assert!(
                error.contains("js-engine"),
                "without the feature the failure must name it, got: {error:?}"
            ),
        }
    }

    /// The engine must not even be created while scripting is disabled.
    ///
    /// `evaluate_javascript` returns before `get_or_insert_with`, so a disabled view
    /// never allocates an engine. Checked through behaviour rather than by reaching
    /// into the field: with the flag off, a script that would trap the engine is
    /// still refused by policy.
    #[test]
    fn a_disabled_view_refuses_script_before_looking_at_it() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        wv.set_javascript_enabled(false);

        // Malformed script: if the policy check were reached *after* evaluation, the
        // error would be a parse error instead of the policy refusal.
        let error = wv.evaluate_javascript("this is not valid javascript(((").expect_err("refused");
        assert!(error.contains("disabled"), "the policy check must run first, got: {error:?}");
    }

    /// `reload()` must go through the normal load lifecycle, not wedge the widget.
    ///
    /// It marks a load pending, so `is_loading()` is true until the load timer (or
    /// `stop`) completes it — the same contract `set_url` follows. The doc previously
    /// implied the load was never finished, which read as a stuck state.
    #[test]
    fn reload_runs_the_normal_load_lifecycle() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));

        // Nothing to reload before a URL exists.
        wv.reload();
        assert!(!wv.is_loading(), "reload with no URL is a no-op");

        wv.set_url("https://example.com/".to_string());
        wv.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wv.is_loading());

        wv.reload();
        assert!(wv.is_loading(), "reload starts a load again");
        wv.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wv.is_loading(), "the load timer completes a reloaded page");
    }

    #[test]
    fn web_engine_wrappers_delegate_widget_draw_and_event_handler() {
        fn assert_wrapper_behaves(mut wrapper: impl Widget + Draw) {
            assert_eq!(wrapper.base().kind(), WidgetKind::WebEngineView);
            let svg = crate::widget::svg::render_to_svg(&mut wrapper);
            assert!(svg.starts_with("<svg"));
            wrapper.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        }

        let geometry = Rect::new(0, 0, 240, 140);
        assert_wrapper_behaves(WebEnginePage::new(geometry));
        assert_wrapper_behaves(WebEngine::new(geometry));
        assert_wrapper_behaves(WebEngineSettings::new(geometry));
        assert_wrapper_behaves(WebEngineDownloadItem::new(geometry));
        assert_wrapper_behaves(WebEngineCookieStore::new(geometry));
        assert_wrapper_behaves(WebEngineWebChannel::new(geometry));
        assert_wrapper_behaves(WebEngineFindTextResult::new(geometry));
        assert_wrapper_behaves(WebEngineNotification::new(geometry));
        assert_wrapper_behaves(WebEngineScriptDialog::new(geometry));
        assert_wrapper_behaves(WebEngineContextMenuRequest::new(geometry));
    }

    #[test]
    fn web_engine_wrappers_forward_timer_completion() {
        let mut wrapper = WebEnginePage::new(Rect::new(0, 0, 320, 200));
        wrapper.inner_mut().set_url("https://example.com/path".to_string());
        assert!(wrapper.inner().is_loading());

        wrapper.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wrapper.inner().is_loading());
    }
}

#[test]
fn web_engine_evaluate_javascript_is_real_when_feature_enabled() {
    let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
    #[cfg(feature = "js-engine")]
    {
        // Real boa_engine evaluation: arithmetic and string results.
        assert_eq!(wv.evaluate_javascript("1 + 2 * 3").as_deref(), Ok("7"));
        assert_eq!(wv.evaluate_javascript("'a' + 'b'").as_deref(), Ok("ab"));
        // Syntax/runtime errors surface instead of a fake success value.
        assert!(wv.evaluate_javascript("this is not valid js {{").is_err());
    }
    #[cfg(not(feature = "js-engine"))]
    {
        // Honest failure instead of a placeholder "Result".
        assert!(wv.evaluate_javascript("1").is_err());
    }
}

#[test]
fn web_engine_load_html_retains_source() {
    let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
    wv.load_html("<html><body>Hello</body></html>");
    assert_eq!(wv.html_source(), "<html><body>Hello</body></html>");
    assert_eq!(wv.url(), "data:text/html");
}
