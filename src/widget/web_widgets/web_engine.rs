use crate::core::{Color, Font, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Widget, WidgetKind};
/// Web engine view widget for web content rendering.
pub struct WebEngineView {
    base: BaseWidget,
    url: String,
    loading: bool,
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
impl WebEngineView {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineView, geometry, "WebEngineView"),
            url: "".to_string(),
            loading: false,
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
    pub fn set_url(&mut self, url: String) {
        if self.url != url {
            self.url = url.clone();
            self.url_changed.emit(url.clone());
            self.loading = true;
            self.loading_started.emit(url.clone());
            // In a real implementation, this would start loading the URL
            // For now, we'll just simulate it
            self.loading = false;
            self.loading_finished.emit(url.clone());
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
    pub fn load_html(&mut self, _html: &str) {
        // In a real implementation, this would load the HTML
        // For now, we'll just simulate it
        self.url = "data:text/html".to_string();
        self.title = "HTML Content".to_string();
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }
    pub fn load_data(&mut self, _data: &[u8], _mime_type: &str, _encoding: &str, base_url: &str) {
        // In a real implementation, this would load the data
        // For now, we'll just simulate it
        self.url = base_url.to_string();
        self.title = "Data Content".to_string();
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }
    pub fn go_back(&mut self) {
        if self.can_go_back {
            // In a real implementation, this would navigate back
            // For now, we'll just simulate it
            self.can_go_back = false;
            self.can_go_forward = true;
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
    pub fn go_forward(&mut self) {
        if self.can_go_forward {
            // In a real implementation, this would navigate forward
            // For now, we'll just simulate it
            self.can_go_forward = false;
            self.can_go_back = true;
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }
    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            // In a real implementation, this would reload the page
            // For now, we'll just simulate it
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }
    pub fn stop(&mut self) {
        if self.loading {
            // In a real implementation, this would stop loading
            // For now, we'll just simulate it
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }
    pub fn evaluate_javascript(&mut self, _script: &str) -> Result<String, String> {
        // In a real implementation, this would evaluate the JavaScript
        // For now, we'll just return a placeholder
        Ok("Result".to_string())
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
        self.navigation_state_changed
            .emit((self.can_go_back, self.can_go_forward));
    }
}
impl Widget for WebEngineView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

use crate::render::RenderContext;
use crate::widget::Draw;

impl EventHandler for WebEngineView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
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
                    _ => {}
                }
            }
            _ => {}
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
        );
        // Content area hint
        if self.is_loading() {
            ctx.draw_text(
                Point::new(g.x + 4, g.y + g.height as i32 / 2),
                "Loading...",
                &Font::default_ui(),
                Color::rgb(150, 150, 150),
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
}
