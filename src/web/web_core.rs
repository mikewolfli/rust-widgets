//! Shared core for WebEngineViewEnhanced and WebViewEnhanced.
//!
//! Both types delegate to [`WebViewCore`] to avoid ~95% code duplication.
//! Each wrapper adds only its own unique signals and behavior on top of this core.

use super::history::{BrowserHistory, SessionHistory};
use super::js_engine::{JsContext, JsEngine, JsResult, JsValue, SimpleJsEngine};
use super::plugins::PluginManager;
use super::privacy::{CookieJar, PrivacySettings, TrackingProtection};
use crate::core::Rect;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, WidgetKind};

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
    pub _error_occurred: Signal1<String>,
    pub navigation_state_changed: Signal1<(bool, bool)>,
    pub console_message: Signal1<(String, u32, String)>,
    pub content: String,
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
            _error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
            console_message: Signal1::new(),
            content: String::new(),
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
        self.content = html.to_string();
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
        self.content = String::from_utf8_lossy(data).to_string();
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
        if !self.url.is_empty() {
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
        self.navigation_state_changed
            .emit((self.can_go_back(), self.can_go_forward()));
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
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Widget trait delegation for WebViewCore (used by both wrappers)
// ---------------------------------------------------------------------------
macro_rules! delegate_widget {
    ($wrapper:ty) => {
        impl Widget for $wrapper {
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
    use crate::core::Rect;
    use crate::web::privacy::{BrowsingData, Cookie};

    #[test]
    fn test_web_view_core_new() {
        let core = WebViewCore::new(
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_url("about:blank".to_string());
        // URL unchanged, so load should be short-circuited
        assert!(!core.is_loading());
        assert_eq!(core.load_progress(), 0);
    }

    #[test]
    fn test_web_view_core_load_url() {
        let mut core = WebViewCore::new(
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.load_data(b"raw data", "text/plain", "https://data.url");
        assert_eq!(core.url(), "https://data.url");
        assert_eq!(core.title(), "Data: text/plain");
        assert_eq!(core.content(), "raw data");
    }

    #[test]
    fn test_web_view_core_go_back_forward() {
        let mut core = WebViewCore::new(
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
            Rect::new(0, 0, 800, 600),
            "test_webview",
            "about:blank",
        );
        core.set_javascript_enabled(false);
        let result = core.evaluate_javascript("var x = 1;");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("JavaScript is disabled"));
    }

    #[test]
    fn test_web_view_core_evaluate_javascript_enabled() {
        let mut core = WebViewCore::new(
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
            WidgetKind::WebView,
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
