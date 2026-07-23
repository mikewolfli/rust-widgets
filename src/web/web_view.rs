//! WebViewEnhanced widget — a web view wrapper.
//!
//! This type shares ~95% of its implementation with [`WebEngineViewEnhanced`](super::web_engine::WebEngineViewEnhanced).
//! Both delegate to a common [`WebViewCore`] to avoid code duplication.
//!
//! **Unique to this type:**
//! - `WidgetKind::WebEngineView`
//! - Initial URL is `"about:blank"` (instead of empty string)
//! - `reload()` skips reloading if the URL is `"about:blank"`
//! - Does NOT expose `set_plugins_enabled` or `set_private_browsing` (unlike the engine variant)

use super::js_engine::{JsResult, JsValue};
use super::web_core::{delegate_widget, WebViewCore};
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{Draw, Widget, WidgetKind};

/// Enhanced web view widget.
pub struct WebViewEnhanced {
    core: WebViewCore,
}

impl WebViewEnhanced {
    pub fn new(geometry: Rect) -> Self {
        Self {
            core: WebViewCore::new(WidgetKind::WebEngineView, geometry, "WebView", "about:blank"),
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

impl Draw for WebViewEnhanced {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let enabled = self.base().is_enabled();
        let border_color = Color::rgb(180, 180, 180);
        let bg_color = Color::rgb(255, 255, 255);

        // ── Outer background & border ──
        context.fill_rect(rect, bg_color);
        context.draw_rect(rect, border_color);

        // ── Address bar area (30px at top) ──
        let addr_h = 30u32;
        let addr_rect = Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, addr_h);
        context.fill_rect(addr_rect, Color::rgb(245, 245, 245));
        // Bottom border of address bar
        context.draw_line(
            Point::new(addr_rect.x, addr_rect.y + addr_rect.height as i32 - 1),
            Point::new(
                addr_rect.x + addr_rect.width as i32,
                addr_rect.y + addr_rect.height as i32 - 1,
            ),
            border_color,
        );

        // URL text centered in address bar
        let url_color = if enabled { Color::rgb(60, 60, 60) } else { Color::rgb(180, 180, 180) };
        context.draw_text(
            Point::new(addr_rect.x + 6, addr_rect.y + 8),
            &self.core.url,
            &Font::new("Arial", 12.0, false, false),
            url_color,
            HorizontalAlignment::Left,
        );

        // ── Loading indicator ──
        if self.core.loading {
            let bar_y = addr_rect.y + addr_rect.height as i32;
            let bar_w = (rect.width * self.core.load_progress as u32 / 100).max(1);
            context.fill_rect(Rect::new(rect.x + 1, bar_y, bar_w, 3), Color::rgb(51, 153, 255));
        }

        // ── Content area ──
        let content_y =
            addr_rect.y + addr_rect.height as i32 + if self.core.loading { 3 } else { 0 };
        let content_h = (rect.y + rect.height as i32) - content_y - 1;
        if content_h > 0 {
            let content_rect = Rect::new(rect.x + 1, content_y, rect.width - 2, content_h as u32);
            // Content background
            context.fill_rect(content_rect, bg_color);

            // Title display
            if !self.core.title.is_empty() {
                let title_color =
                    if enabled { Color::rgb(20, 20, 20) } else { Color::rgb(170, 170, 170) };
                context.draw_text(
                    Point::new(content_rect.x + 4, content_rect.y + 4),
                    &self.core.title,
                    &Font::bold("Arial", 14.0),
                    title_color,
                    HorizontalAlignment::Left,
                );
            }

            // Content snippet preview (first line of HTML content)
            if !self.core.content.is_empty() {
                let snippet = if self.core.content.len() > 200 {
                    format!("{}...", &self.core.content[..200])
                } else {
                    self.core.content.clone()
                };
                let text_color =
                    if enabled { Color::rgb(80, 80, 80) } else { Color::rgb(190, 190, 190) };
                let text_y = content_rect.y + (if self.core.title.is_empty() { 4 } else { 24 });
                context.draw_text(
                    Point::new(content_rect.x + 4, text_y),
                    &snippet,
                    &Font::new("monospace", 10.0, false, false),
                    text_color,
                    HorizontalAlignment::Left,
                );
            }

            // Empty state: show "about:blank" placeholder
            if self.core.url == "about:blank" && self.core.content.is_empty() {
                let placeholder_color = Color::rgb(200, 200, 200);
                context.draw_text(
                    Point::new(content_rect.x + 4, content_rect.y + 4),
                    "about:blank",
                    &Font::new("Arial", 13.0, false, false),
                    placeholder_color,
                    HorizontalAlignment::Left,
                );
            }
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
        assert!(result.unwrap_err().message.contains("JavaScript is disabled"));
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
