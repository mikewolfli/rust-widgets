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
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{Widget, WidgetKind};

/// Enhanced web engine view widget.
pub struct WebEngineViewEnhanced {
    core: WebViewCore,
    pub certificate_error: Signal1<String>,
    pub download_requested: Signal1<String>,
}

impl WebEngineViewEnhanced {
    pub fn new(geometry: Rect) -> Self {
        Self {
            core: WebViewCore::new(WidgetKind::WebEngineView, geometry, "WebEngineView", ""),
            certificate_error: Signal1::new(),
            download_requested: Signal1::new(),
        }
    }

    // -- Accessors that delegate to core --

    pub fn url(&self) -> &str {
        self.core.url()
    }
    pub fn is_loading(&self) -> bool {
        self.core.is_loading()
    }
    pub fn title(&self) -> &str {
        self.core.title()
    }
    pub fn load_progress(&self) -> u8 {
        self.core.load_progress()
    }
    pub fn can_go_back(&self) -> bool {
        self.core.can_go_back()
    }
    pub fn can_go_forward(&self) -> bool {
        self.core.can_go_forward()
    }
    pub fn settings(&self) -> &super::WebSettings {
        self.core.settings()
    }
    pub fn settings_mut(&mut self) -> &mut super::WebSettings {
        self.core.settings_mut()
    }
    pub fn security(&self) -> &super::SecuritySettings {
        self.core.security()
    }
    pub fn security_mut(&mut self) -> &mut super::SecuritySettings {
        self.core.security_mut()
    }
    pub fn cookies(&self) -> &super::privacy::CookieJar {
        self.core.cookies()
    }
    pub fn cookies_mut(&mut self) -> &mut super::privacy::CookieJar {
        self.core.cookies_mut()
    }
    pub fn privacy(&self) -> &super::privacy::TrackingProtection {
        self.core.privacy()
    }
    pub fn privacy_mut(&mut self) -> &mut super::privacy::TrackingProtection {
        self.core.privacy_mut()
    }
    pub fn plugins(&self) -> &super::plugins::PluginManager {
        self.core.plugins()
    }
    pub fn plugins_mut(&mut self) -> &mut super::plugins::PluginManager {
        self.core.plugins_mut()
    }
    pub fn history(&self) -> &super::history::SessionHistory {
        self.core.history()
    }
    pub fn browser_history(&self) -> &super::history::BrowserHistory {
        self.core.browser_history()
    }

    // -- Methods that delegate to core --

    pub fn load_url(&mut self, url: &str) {
        self.core.load_url(url);
    }
    pub fn set_url(&mut self, url: String) {
        self.core.set_url(url);
    }
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        self.core.load_html(html, base_url);
    }
    pub fn load_data(&mut self, data: &[u8], mime_type: &str, base_url: &str) {
        self.core.load_data(data, mime_type, base_url);
    }
    pub fn go_back(&mut self) {
        self.core.go_back();
    }
    pub fn go_forward(&mut self) {
        self.core.go_forward();
    }
    pub fn reload(&mut self) {
        self.core.reload();
    }
    pub fn stop(&mut self) {
        self.core.stop();
    }
    pub fn set_title(&mut self, title: String) {
        self.core.set_title(title);
    }
    pub fn evaluate_javascript(&mut self, script: &str) -> JsResult<JsValue> {
        self.core.evaluate_javascript(script)
    }
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.core.set_javascript_enabled(enabled);
    }
    pub fn content(&self) -> &str {
        self.core.content()
    }
    pub fn html(&self) -> &str {
        self.core.html()
    }

    // -- Unique methods on WebEngineViewEnhanced --

    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.core.settings.plugins_enabled = enabled;
    }

    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.core.settings.private_browsing = enabled;
        if enabled {
            self.core.privacy =
                super::privacy::TrackingProtection::new(super::privacy::PrivacySettings::strict());
        }
    }

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
