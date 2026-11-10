// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WebViewEnhanced widget — a web view wrapper.
//!
//! This type shares ~95% of its implementation with [`WebEngineViewEnhanced`].
//! Both delegate to a common [`WebViewCore`] to avoid code duplication.
//!
//! **Unique to this type:**
//! - `WidgetKind::WebEngineView`
//! - Initial URL is `"about:blank"` (instead of empty string)
//! - `reload()` skips reloading if the URL is `"about:blank"`
//! - Does NOT expose `set_plugins_enabled` or `set_private_browsing` (unlike the engine variant)

use super::js_engine::{JsResult, JsValue};
use super::web_core::{delegate_widget, WebViewCore};
use rust_widgets::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{ConnectionScope, GenericSignal, Signal1};
use rust_widgets::style::WidgetStyle;
use rust_widgets::widget::metrics::{dimensions, ControlMetrics};
use rust_widgets::widget::{Draw, Widget, WidgetKind};
use std::cell::RefCell;

/// Enhanced web view widget.
pub struct WebViewEnhanced {
    core: WebViewCore,
    painter: WebViewPainter,
}

/// The browser-chrome and body preview painter shared by [`WebViewEnhanced`] and
/// [`WebEngineViewEnhanced`](crate::webview::web_engine_enhanced::WebEngineViewEnhanced).
///
/// Both wrappers own a [`WebViewCore`] but only one of them used to know how to paint it, so the
/// engine-flavoured wrapper produced an empty frame. The drawing lives here rather than in either
/// wrapper so the two cannot drift into two slightly different pictures of the same state.
pub(crate) struct WebViewPainter {
    /// Fonts are constructed once rather than three times per painted frame. A `Font` owns
    /// its family name, so `Font::new` in `draw` was three heap allocations per frame.
    url_font: Font,
    title_font: Font,
    content_font: Font,
    placeholder_font: Font,
    /// Content-preview cache, keyed by [`WebViewCore::content_revision`]. The revision changes
    /// exactly when the body does, so a hit cannot show a preview of a different document —
    /// the previous key (length plus first/last byte) could be shared by distinct bodies.
    snippet_cache: RefCell<Option<SnippetCache>>,
}

/// One cached body preview and the content revision it was built from.
struct SnippetCache {
    revision: u64,
    text: String,
}

impl WebViewPainter {
    pub(crate) fn new() -> Self {
        Self {
            url_font: Font::new("Arial", 12.0, false, false),
            title_font: Font::bold("Arial", 14.0),
            content_font: Font::new("monospace", 10.0, false, false),
            placeholder_font: Font::new("Arial", 13.0, false, false),
            snippet_cache: RefCell::new(None),
        }
    }
}

impl WebViewEnhanced {
    /// Creates a web view in `geometry` showing `about:blank`, with no title and
    /// nothing loading.
    ///
    /// Unlike [`WebEngineViewEnhanced::new`](crate::webview::web_engine_enhanced::WebEngineViewEnhanced::new),
    /// this type never acquires a native engine: every operation runs on the
    /// simulated loader, which emits 0% → 50% → 100% progress and stores the URL
    /// without performing any network I/O.
    pub fn new(geometry: Rect) -> Self {
        Self {
            core: WebViewCore::new(WidgetKind::WebEngineView, geometry, "WebView", "about:blank"),
            painter: WebViewPainter::new(),
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

    /// The core's load-progress signal, in percent.
    ///
    /// See [`WebEngineViewEnhanced::loading_progress_signal`](crate::webview::web_engine_enhanced::WebEngineViewEnhanced::loading_progress_signal)
    /// for why the core's signals need an accessor on the wrapper.
    pub fn loading_progress_signal(&self) -> &Signal1<u8> {
        &self.core.loading_progress
    }

    /// The core's navigation-state signal, carrying `(can_go_back, can_go_forward)`.
    pub fn navigation_state_changed_signal(&self) -> &Signal1<(bool, bool)> {
        &self.core.navigation_state_changed
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
    /// [`Self::load_url`]; navigating to the URL already displayed is a no-op.
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
    /// Advances an in-flight asynchronous load, if any (`web-http` only).
    ///
    /// The host calls this once per frame/tick. It returns `true` while a load is still in flight and
    /// `false` once it has completed, failed, or there was nothing to poll. With the `web-http`
    /// feature, `set_url` starts the fetch on a helper thread and this method delivers its result on
    /// the caller's thread — the fetch never blocks `set_url`.
    pub fn poll_load(&mut self) -> bool {
        #[cfg(feature = "web-http")]
        {
            self.core.poll_load()
        }
        #[cfg(not(feature = "web-http"))]
        {
            false
        }
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

impl WebViewPainter {
    /// Paints `core`'s browser chrome, loading indicator and body preview.
    ///
    /// Taking the core by reference is what lets both enhanced wrappers share one picture of
    /// the same state instead of one of them delegating to the other's instance.
    pub(crate) fn draw(&self, core: &WebViewCore, context: &mut RenderContext) {
        let rect = core.base.geometry();
        let enabled = core.base.is_enabled();

        // Same three-level resolution `WebEngineView::draw` uses: the control's explicit style
        // first, then the theme's resolved style for this control, then the shared palette. The
        // literal fallbacks are the pre-theme colours this draw used to hardcode, so a build with
        // no theme at all paints exactly what it painted before.
        let style = core.base.style();
        let theme = rust_widgets::style::resolved_theme_style_shared("web_engine_view");
        let palette = rust_widgets::style::palette_shared();
        let window_fill = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|resolved| resolved.background_color))
            .or_else(|| palette.as_ref().map(|colors| colors.background))
            .unwrap_or(Color::rgb(255, 255, 255));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|resolved| resolved.text_color))
            .or_else(|| palette.as_ref().map(|colors| colors.foreground))
            .unwrap_or(Color::rgb(60, 60, 60));
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|resolved| resolved.border_color))
            .unwrap_or(Color::rgb(180, 180, 180));
        let accent =
            palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::rgb(51, 153, 255));

        let bg_color = window_fill;

        // ── Outer background & border ──
        context.fill_rect(rect, bg_color);
        context.draw_rect(rect, border_color);

        // ── Address bar area ──
        //
        // The height is the shared `WEB_URL_BAR_HEIGHT`, not a second literal: `WebEngineView`
        // paints its URL band from the same constant, and the two wrappers showing different
        // band heights for the same chrome was the drift. The band is clamped to the control, so
        // a short view cannot paint its chrome outside its own rectangle.
        let bar_band = ControlMetrics::top_band(rect, dimensions::WEB_URL_BAR_HEIGHT);
        let addr_rect = Rect::new(
            rect.x + 1,
            rect.y + 1,
            rect.width.saturating_sub(2),
            bar_band.height.saturating_sub(1),
        );
        // The bar is the page fill eased toward the ink, which is the old 245/245/245 on a light
        // theme and stays distinct from the page on a dark one.
        context.fill_rect(addr_rect, window_fill.blend(&ink, 0.05));
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
        let url_color = if enabled { ink } else { ink.blend(&window_fill, 0.6) };
        context.draw_text(
            Point::new(addr_rect.x + 6, addr_rect.y + 8),
            &core.url,
            &self.url_font,
            url_color,
            HorizontalAlignment::Left,
        );

        // ── Loading indicator ──
        if core.loading {
            let bar_y = addr_rect.y + addr_rect.height as i32;
            let bar_w = (rect.width * core.load_progress as u32 / 100).max(1);
            context.fill_rect(Rect::new(rect.x + 1, bar_y, bar_w, 3), accent);
        }

        // ── Content area ──
        let content_y = addr_rect.y + addr_rect.height as i32 + if core.loading { 3 } else { 0 };
        let content_h = (rect.y + rect.height as i32) - content_y - 1;
        if content_h > 0 {
            let content_rect =
                Rect::new(rect.x + 1, content_y, rect.width.saturating_sub(2), content_h as u32);
            // Content background
            context.fill_rect(content_rect, bg_color);

            // Title display
            if !core.title.is_empty() {
                let title_color = if enabled { ink } else { ink.blend(&window_fill, 0.55) };
                context.draw_text(
                    Point::new(content_rect.x + 4, content_rect.y + 4),
                    &core.title,
                    &self.title_font,
                    title_color,
                    HorizontalAlignment::Left,
                );
            }

            // Content snippet preview (the body's first line).
            if !core.content.is_empty() {
                let content = &core.content;
                // The cache key is the core's monotonic content revision, so a different body
                // can never reuse the previous preview even when the two share a length and
                // their first and last bytes.
                let revision = core.content_revision();
                let stale = {
                    let cache = self.snippet_cache.borrow();
                    match cache.as_ref() {
                        Some(cached) => cached.revision != revision,
                        None => true,
                    }
                };
                if stale {
                    let text = if content.len() > 200 {
                        let boundary =
                            rust_widgets::widget::text_utils::floor_char_boundary(content, 197);
                        format!("{}...", &content[..boundary])
                    } else {
                        content.clone()
                    };
                    *self.snippet_cache.borrow_mut() = Some(SnippetCache { revision, text });
                }
                let snippet_guard = self.snippet_cache.borrow();
                let snippet =
                    snippet_guard.as_ref().map(|cached| cached.text.as_str()).unwrap_or("");
                let text_color = if enabled {
                    ink.blend(&window_fill, 0.1)
                } else {
                    ink.blend(&window_fill, 0.7)
                };
                let text_y = content_rect.y + (if core.title.is_empty() { 4 } else { 24 });
                context.draw_text(
                    Point::new(content_rect.x + 4, text_y),
                    snippet,
                    &self.content_font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }

            // Empty state: show "about:blank" placeholder
            if core.url == "about:blank" && core.content.is_empty() {
                let placeholder_color = ink.blend(&window_fill, 0.75);
                context.draw_text(
                    Point::new(content_rect.x + 4, content_rect.y + 4),
                    "about:blank",
                    &self.placeholder_font,
                    placeholder_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl Draw for WebViewEnhanced {
    fn draw(&mut self, context: &mut RenderContext) {
        self.painter.draw(&self.core, context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "web-http"))]
    use crate::webview::privacy::BrowsingData;
    use rust_widgets::core::Rect;

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

    /// The wrapper forwards the core's duplicate-URL no-op.
    ///
    /// `load_url`/`set_url` only delegate; this pins that a repeated current URL does not start
    /// a second load or append history through the wrapper either.
    #[test]
    fn repeating_the_current_url_is_a_no_op_through_the_wrapper() {
        use std::sync::{Arc, Mutex};

        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        let starts = Arc::new(Mutex::new(0usize));
        let sink = starts.clone();
        view.core.loading_started.connect(move |_| {
            *sink.lock().unwrap() += 1;
        });

        view.set_url("https://example.com/".to_string());
        assert_eq!(*starts.lock().unwrap(), 1);
        let back_entries = view.history().back_entries().len();

        view.set_url("https://example.com/".to_string());
        assert_eq!(*starts.lock().unwrap(), 1, "the load must not restart");
        assert_eq!(
            view.history().back_entries().len(),
            back_entries,
            "the repeat must not record another history entry"
        );
        assert_eq!(view.url(), "https://example.com/");
    }

    #[cfg(not(feature = "web-http"))]
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
    fn test_web_view_draw_multibyte_content() {
        use rust_widgets::widget::svg::render_to_svg;
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        let long_multibyte_html = format!(
            "<p>{}</p>",
            "Café crème brûlée with € and £ symbols for testing multi-byte snippet boundaries in web preview. ".repeat(5)
        );
        view.load_html(&long_multibyte_html, None);
        let svg = render_to_svg(&mut view);
        assert!(!svg.is_empty(), "drawing web view with multibyte HTML content must not panic");
    }

    /// Two bodies that share a length and first/last byte must not share a cached preview.
    ///
    /// The old cache key was `(len, first, last)`, so `<p>AAAA</p>` and `<p>BBBB</p>` collided
    /// and the second document painted the first one's text until something else changed.
    #[test]
    fn bodies_with_the_same_shape_do_not_share_a_preview() {
        let _theme_guard = rust_widgets::style::theme_test_guard();
        let mut view = WebViewEnhanced::new(Rect::new(0, 0, 400, 240));

        view.load_html("<p>AAAA</p>", None);
        let _ = rust_widgets::widget::svg::render_to_svg(&mut view);
        let first = {
            let cache = view.painter.snippet_cache.borrow();
            cache.as_ref().map(|cached| cached.text.clone()).expect("the first preview was built")
        };

        // Same length and same first/last byte as the previous body.
        view.load_html("<p>BBBB</p>", None);
        let _ = rust_widgets::widget::svg::render_to_svg(&mut view);
        let second = {
            let cache = view.painter.snippet_cache.borrow();
            cache.as_ref().map(|cached| cached.text.clone()).expect("the second preview was built")
        };

        assert_eq!(first, "<p>AAAA</p>");
        assert_eq!(second, "<p>BBBB</p>", "the preview follows the new body, not the old key");
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

    #[cfg(not(feature = "web-http"))]
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

    #[cfg(not(feature = "web-http"))]
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
