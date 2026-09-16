// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
    /// Creates a web view in `geometry` showing `about:blank`, with no title and
    /// nothing loading.
    ///
    /// Unlike [`WebEngineViewEnhanced::new`](super::web_engine::WebEngineViewEnhanced::new),
    /// this type never acquires a native engine: every operation runs on the
    /// simulated loader, which emits 0% → 50% → 100% progress and stores the URL
    /// without performing any network I/O.
    pub fn new(geometry: Rect) -> Self {
        Self {
            core: WebViewCore::new(WidgetKind::WebEngineView, geometry, "WebView", "about:blank"),
        }
    }

    // -- Accessors that delegate to core --

    /// The address currently shown, `"about:blank"` until something is loaded.
    pub fn url(&self) -> &str {
        self.core.url()
    }
    /// Whether a navigation is in flight.
    ///
    /// Always `false` by the time a loader returns: the simulated path completes
    /// the whole 0 → 50 → 100 sequence inside the call, so this only reports
    /// `true` if read from a loading-progress callback.
    pub fn is_loading(&self) -> bool {
        self.core.is_loading()
    }
    /// The page title, or `""` when none has been set or inferred.
    pub fn title(&self) -> &str {
        self.core.title()
    }
    /// Load completion as a percentage, `0`..=`100`. 100 means the last
    /// navigation finished; it says nothing about the document's validity.
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
    /// The view's preferences.
    pub fn settings(&self) -> &super::WebSettings {
        self.core.settings()
    }
    /// The view's preferences, mutably, for changing several at once.
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
    /// Session history backing the back/forward state.
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
    /// The URL must begin with `http://`, `https://` or `file://`; anything else
    /// is logged and rejected, leaving the view as it was, and this method returns
    /// without reporting the refusal.
    pub fn load_url(&mut self, url: &str) {
        self.core.load_url(url);
    }
    /// Navigates to `url`, taking ownership of it. Same scheme validation as
    /// [`Self::load_url`].
    pub fn set_url(&mut self, url: String) {
        self.core.set_url(url);
    }
    /// Loads `html` as the document, with `base_url` as the address it came from
    /// — `None` becomes `"data:text/html"`.
    ///
    /// The title becomes `"HTML Content"` and the body is stored verbatim: no
    /// parsing, scripting or sanitising happens.
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        self.core.load_html(html, base_url);
    }
    /// Loads `data` as the document at `base_url`, declaring the bytes to be of
    /// type `mime_type`, which also becomes the title as `"Data: <mime_type>"`.
    ///
    /// `data` is decoded with [`String::from_utf8_lossy`], so invalid UTF-8
    /// becomes replacement characters rather than an error.
    pub fn load_data(&mut self, data: &[u8], mime_type: &str, base_url: &str) {
        self.core.load_data(data, mime_type, base_url);
    }
    /// Steps one entry back in session history, driving simulated loading
    /// callbacks. A no-op when there is nothing behind the current entry.
    pub fn go_back(&mut self) {
        self.core.go_back();
    }
    /// Steps one entry forward in session history, driving simulated loading
    /// callbacks. A no-op when there is nothing ahead of the current entry.
    pub fn go_forward(&mut self) {
        self.core.go_forward();
    }
    /// Reload the current page. Skips reloading if the URL is "about:blank".
    pub fn reload(&mut self) {
        if self.core.url != "about:blank" {
            self.core.reload();
        }
    }
    /// Aborts an in-flight load and resets progress to 0. A no-op when nothing
    /// is loading.
    pub fn stop(&mut self) {
        self.core.stop();
    }
    /// Sets the title, emitting `title_changed` only when the value actually
    /// differs from the current one.
    pub fn set_title(&mut self, title: String) {
        self.core.set_title(title);
    }
    /// Runs `script` and returns its value.
    ///
    /// Fails with a `"JavaScript is disabled"` error when
    /// [`WebSettings::javascript_enabled`](super::WebSettings::javascript_enabled)
    /// is `false`. Scripts run against this view's own context, so state does not
    /// leak between views.
    pub fn evaluate_javascript(&mut self, script: &str) -> JsResult<JsValue> {
        self.core.evaluate_javascript(script)
    }
    /// Turns script evaluation on or off by setting
    /// [`WebSettings::javascript_enabled`](super::WebSettings::javascript_enabled).
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.core.set_javascript_enabled(enabled);
    }
    /// The decoded document body most recently loaded, or `""` if none.
    pub fn content(&self) -> &str {
        self.core.content()
    }
    /// The document body most recently loaded.
    ///
    /// Identical to [`Self::content`]; the two names exist because one reads
    /// naturally for a markup payload and the other for a fetched body.
    pub fn html(&self) -> &str {
        self.core.html()
    }

    /// Erases the parts of the local browsing state selected by `data`, as
    /// described by [`BrowsingData`](super::privacy::BrowsingData).
    ///
    /// Clearing history empties the browsing history *and* resets the
    /// back/forward stack, so [`Self::can_go_back`] and [`Self::can_go_forward`]
    /// both become `false`.
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
        let addr_rect = Rect::new(rect.x + 1, rect.y + 1, rect.width.saturating_sub(2), addr_h);
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
            let content_rect =
                Rect::new(rect.x + 1, content_y, rect.width.saturating_sub(2), content_h as u32);
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
