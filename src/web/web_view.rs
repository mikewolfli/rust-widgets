//! WebViewEnhanced widget — a web view wrapper.
//!
//! This type shares ~95% of its implementation with [`WebEngineViewEnhanced`](super::web_engine::WebEngineViewEnhanced).
//! Both delegate to a common [`WebViewCore`] to avoid code duplication.
//!
//! **Unique to this type:**
//! - `WidgetKind::WebView`
//! - Initial URL is `"about:blank"` (instead of empty string)
//! - `reload()` skips reloading if the URL is `"about:blank"`
//! - Does NOT expose `set_plugins_enabled` or `set_private_browsing` (unlike the engine variant)

use super::js_engine::{JsResult, JsValue};
use super::web_core::{delegate_widget, WebViewCore};
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{Widget, WidgetKind};

/// Enhanced web view widget.
pub struct WebViewEnhanced {
    core: WebViewCore,
}

impl WebViewEnhanced {
    pub fn new(geometry: Rect) -> Self {
        Self {
            core: WebViewCore::new(WidgetKind::WebView, geometry, "WebView", "about:blank"),
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
    /// Reload the current page. Skips reloading if the URL is "about:blank".
    pub fn reload(&mut self) {
        if self.core.url != "about:blank" {
            self.core.reload();
        }
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

    pub fn clear_browsing_data(&mut self, data: super::privacy::BrowsingData) {
        self.core.clear_browsing_data(data);
    }
}

// Delegate Widget trait to core via the shared macro
delegate_widget!(WebViewEnhanced);

impl EventHandler for WebViewEnhanced {
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
    fn test_web_view_new() {
        let view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(view.url(), "about:blank");
        assert!(!view.is_loading());
        assert_eq!(view.title(), "");
        assert_eq!(view.load_progress(), 0);
        assert!(!view.can_go_back());
        assert!(!view.can_go_forward());
    }

    #[test]
    fn test_web_view_set_url() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        view.set_url("https://example.com".to_string());
        assert_eq!(view.url(), "https://example.com");
        assert!(!view.is_loading());
        assert_eq!(view.load_progress(), 100);
    }

    #[test]
    fn test_web_view_load_url() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        view.load_url("https://rust-lang.org");
        assert_eq!(view.url(), "https://rust-lang.org");
    }

    #[test]
    fn test_web_view_navigate_back_and_forward() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(!view.can_go_back());
        assert!(!view.can_go_forward());

        view.load_url("https://page1.com");
        view.load_url("https://page2.com");
        assert!(view.can_go_back());
        assert!(!view.can_go_forward());

        view.go_back();
        assert!(view.can_go_forward());
        assert_eq!(view.url(), "https://page1.com");

        view.go_forward();
        assert_eq!(view.url(), "https://page2.com");
    }

    #[test]
    fn test_web_view_reload_skips_about_blank() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        // reload on about:blank should be a no-op
        view.reload();
        assert!(!view.is_loading());
        assert_eq!(view.url(), "about:blank");

        // After loading a real URL, reload should work
        view.load_url("https://example.com");
        view.reload();
        assert!(!view.is_loading());
        assert_eq!(view.load_progress(), 100);
    }

    #[test]
    fn test_web_view_stop() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        view.stop();
        assert!(!view.is_loading());
    }

    #[test]
    fn test_web_view_set_title() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(view.title(), "");
        view.set_title("My Page".to_string());
        assert_eq!(view.title(), "My Page");
    }

    #[test]
    fn test_web_view_load_html() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        view.load_html("<p>Hello</p>", None);
        assert_eq!(view.url(), "data:text/html");
        assert_eq!(view.title(), "HTML Content");
        assert_eq!(view.html(), "<p>Hello</p>");
    }

    #[test]
    fn test_web_view_load_data() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        view.load_data(b"some bytes", "text/plain", "https://data.url");
        assert_eq!(view.url(), "https://data.url");
        assert_eq!(view.title(), "Data: text/plain");
        assert_eq!(view.content(), "some bytes");
    }

    #[test]
    fn test_web_view_evaluate_javascript() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        let result = view.evaluate_javascript("var x = 10; x");
        assert!(result.is_ok());
    }

    #[test]
    fn test_web_view_evaluate_javascript_disabled() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        view.set_javascript_enabled(false);
        let result = view.evaluate_javascript("1 + 1");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("JavaScript is disabled"));
    }

    #[test]
    fn test_web_view_settings_and_security() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(view.settings().javascript_enabled);
        assert!(!view.settings().plugins_enabled);
        assert!(view.security().block_popups);

        view.settings_mut().javascript_enabled = false;
        assert!(!view.settings().javascript_enabled);
    }

    #[test]
    fn test_web_view_cookies_privacy_plugins() {
        let view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(view.cookies().is_empty());
        assert_eq!(view.privacy().blocked_count(), 0);
        assert!(view.plugins().list().is_empty());
    }

    #[test]
    fn test_web_view_clear_browsing_data() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        view.load_url("https://example.com");
        assert!(!view.browser_history().is_empty());
        view.clear_browsing_data(BrowsingData {
            cookies: false,
            history: true,
            ..Default::default()
        });
        assert!(view.browser_history().is_empty());
    }

    #[test]
    fn test_web_view_history_access() {
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert!(view.history().current().is_none());
        assert!(view.browser_history().is_empty());

        view.load_url("https://example.com");
        assert!(view.history().current().is_some());
        assert!(!view.browser_history().is_empty());
    }
}
