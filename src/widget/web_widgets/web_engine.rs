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
use crate::widget::{BaseWidget, Widget, WidgetKind};
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
    /// Embedded real JavaScript engine (boa_engine) for script evaluation.
    #[cfg(feature = "js-engine")]
    js_engine: Option<crate::web::BoaJsEngine>,
}
// Newtype structs for render pipeline symbol imports, wrapping WebEngineView.
pub struct WebEnginePage(pub WebEngineView);
impl WebEnginePage {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngine(pub WebEngineView);
impl WebEngine {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineSettings(pub WebEngineView);
impl WebEngineSettings {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineDownloadItem(pub WebEngineView);
impl WebEngineDownloadItem {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineCookieStore(pub WebEngineView);
impl WebEngineCookieStore {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineWebChannel(pub WebEngineView);
impl WebEngineWebChannel {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineFindTextResult(pub WebEngineView);
impl WebEngineFindTextResult {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineNotification(pub WebEngineView);
impl WebEngineNotification {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineScriptDialog(pub WebEngineView);
impl WebEngineScriptDialog {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
pub struct WebEngineContextMenuRequest(pub WebEngineView);
impl WebEngineContextMenuRequest {
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
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
            #[cfg(feature = "js-engine")]
            js_engine: None,
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
    pub fn set_title(&mut self, title: String) {
        if self.title != title {
            self.title = title.clone();
            self.title_changed.emit(title);
            self.base.request_redraw();
        }
    }
    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }
    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }
    pub fn is_javascript_enabled(&self) -> bool {
        self.javascript_enabled
    }
    pub fn is_plugins_enabled(&self) -> bool {
        self.plugins_enabled
    }
    pub fn is_private_browsing(&self) -> bool {
        self.private_browsing
    }

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

    pub fn set_url(&mut self, url: String) {
        if self.url != url {
            self.url = url.clone();
            self.url_changed.emit(url.clone());
            self.begin_loading();
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
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
    pub fn go_forward(&mut self) {
        if self.can_go_forward {
            // See `go_back` — modeled navigation state only.
            self.can_go_forward = false;
            self.can_go_back = true;
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            // Re-emits the load lifecycle without re-fetching (no network).
            self.begin_loading();
        }
    }
    pub fn stop(&mut self) {
        self.finish_loading();
    }
    pub fn evaluate_javascript(&mut self, script: &str) -> Result<String, String> {
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
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.javascript_enabled = enabled;
    }
    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.plugins_enabled = enabled;
    }
    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.private_browsing = enabled;
    }
    fn update_navigation_state(&self) {
        self.navigation_state_changed.emit((self.can_go_back, self.can_go_forward));
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
            Event::MousePress { pos: _, button }
                // Handle mouse click — simulate link navigation
                if *button == 1 => {
                    let new_url = format!("https://example.com/{}", 12345);
                    self.set_url(new_url);
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
