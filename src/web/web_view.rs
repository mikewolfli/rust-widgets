use super::history::{BrowserHistory, SessionHistory};
use super::js_engine::{JsContext, JsEngine, JsResult, JsValue, SimpleJsEngine};
use super::plugins::PluginManager;
use super::privacy::{CookieJar, PrivacySettings, TrackingProtection};
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Widget, WidgetKind};

pub struct WebViewEnhanced {
    base: BaseWidget,
    url: String,
    loading: bool,
    title: String,
    load_progress: u8,
    history: SessionHistory,
    browser_history: BrowserHistory,
    js_engine: Box<dyn JsEngine>,
    js_context: JsContext,
    cookies: CookieJar,
    privacy: TrackingProtection,
    plugins: PluginManager,
    settings: super::WebSettings,
    security: super::SecuritySettings,
    pub loading_started: Signal1<String>,
    pub loading_finished: Signal1<String>,
    pub loading_progress: Signal1<u8>,
    pub title_changed: Signal1<String>,
    pub url_changed: Signal1<String>,
    pub error_occurred: Signal1<String>,
    pub navigation_state_changed: Signal1<(bool, bool)>,
    pub console_message: Signal1<(String, u32, String)>,
}

impl WebViewEnhanced {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebView, geometry, "WebView"),
            url: "about:blank".to_string(),
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

    pub fn set_url(&mut self, url: String) {
        if self.url == url {
            return;
        }

        self.url = url.clone();
        self.loading = true;
        self.load_progress = 0;

        self.url_changed.emit(url.clone());
        self.loading_started.emit(url.clone());
        self.history.navigate(url.clone());

        self.load_progress = 50;
        self.loading_progress.emit(self.load_progress);

        self.load_progress = 100;
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.update_navigation_state();

        self.browser_history
            .add_entry(self.url.clone(), self.title.clone());
        self.base.request_redraw();
    }

    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        self.url = base_url.unwrap_or("data:text/html").to_string();
        self.title = "HTML Content".to_string();
        self.loading = true;
        self.load_progress = 0;

        self.loading_started.emit(self.url.clone());

        let _ = html;

        self.load_progress = 100;
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }

    pub fn load_data(&mut self, data: &[u8], mime_type: &str, base_url: &str) {
        self.url = base_url.to_string();
        self.title = format!("Data: {}", mime_type);
        self.loading = true;
        self.load_progress = 0;

        self.loading_started.emit(self.url.clone());

        let _ = data;

        self.load_progress = 100;
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }

    pub fn go_back(&mut self) {
        if let Some(url) = self.history.go_back() {
            self.url = url;
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(url) = self.history.go_forward() {
            self.url = url;
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }

    pub fn reload(&mut self) {
        if !self.url.is_empty() && self.url != "about:blank" {
            self.loading = true;
            self.load_progress = 0;
            self.loading_started.emit(self.url.clone());
            self.load_progress = 100;
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }

    pub fn stop(&mut self) {
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
            return Err(super::js_engine::JsError::new(
                "JavaScript is disabled".to_string(),
            ));
        }

        let result = self.js_engine.evaluate(script, &mut self.js_context)?;

        for msg in self.js_context.console_messages() {
            let level = format!("{:?}", msg.level);
            self.console_message
                .emit((level, msg.line, msg.message.clone()));
        }

        Ok(result)
    }

    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.settings.javascript_enabled = enabled;
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
        self.navigation_state_changed
            .emit((self.can_go_back(), self.can_go_forward()));
    }
}

impl Widget for WebViewEnhanced {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}

impl EventHandler for WebViewEnhanced {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => match *key {
                37 => {
                    if *modifiers == 0 {
                        self.go_back();
                    }
                }
                39 => {
                    if *modifiers == 0 {
                        self.go_forward();
                    }
                }
                116 => {
                    self.reload();
                }
                82 => {
                    if *modifiers == 1 {
                        self.reload();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}
