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
