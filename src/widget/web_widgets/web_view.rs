use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Widget, WidgetKind};
/// WebView widget for displaying web content.
pub struct WebView {
    base: BaseWidget,
    url: String,
    loading: bool,
    pending_load: bool,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
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
}
impl WebView {
    const LOAD_TIMER_ID: u32 = 1;

    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebView, geometry, "WebView"),
            url: "about:blank".to_string(),
            loading: false,
            pending_load: false,
            title: "".to_string(),
            can_go_back: false,
            can_go_forward: false,
            loading_started: Signal1::new(),
            loading_finished: Signal1::new(),
            title_changed: Signal1::new(),
            url_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
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
    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }
    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
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
            self.url = url;
            self.url_changed.emit(self.url.clone());
            self.begin_loading();
        }
    }
    pub fn load_url(&mut self, url: &str) {
        self.set_url(url.to_string());
    }
    pub fn load_html(&mut self, _html: &str) {
        // In a real implementation, this would load the HTML
        // For now, we'll just simulate it
        self.url = "data:text/html".to_string();
        self.title = "HTML Content".to_string();
        self.begin_loading();
        self.finish_loading();
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
    }
    pub fn go_back(&mut self) {
        if self.can_go_back {
            // In a real implementation, this would navigate back
            // For now, we'll just simulate it
            self.can_go_back = false;
            self.can_go_forward = true;
            self.navigation_state_changed.emit((self.can_go_back, self.can_go_forward));
        }
    }
    pub fn go_forward(&mut self) {
        if self.can_go_forward {
            // In a real implementation, this would navigate forward
            // For now, we'll just simulate it
            self.can_go_back = true;
            self.can_go_forward = false;
            self.navigation_state_changed.emit((self.can_go_back, self.can_go_forward));
        }
    }
    pub fn reload(&mut self) {
        // In a real implementation, this would reload the current page
        self.begin_loading();
    }
    pub fn stop(&mut self) {
        // In a real implementation, this would stop loading
        self.finish_loading();
    }
    pub fn set_title(&mut self, title: String) {
        if self.title != title {
            self.title = title;
            self.title_changed.emit(self.title.clone());
        }
    }
    pub fn evaluate_javascript(&mut self, _script: &str) -> Option<String> {
        // In a real implementation, this would evaluate the JavaScript
        // For now, we'll just return None
        None
    }
}
impl Widget for WebView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

use crate::render::RenderContext;
use crate::widget::Draw;

impl EventHandler for WebView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::Timer { id } if *id == Self::LOAD_TIMER_ID && self.pending_load => {
                self.finish_loading();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for WebView {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let g = self.geometry();
        ctx.fill_rect(g, Color::WHITE);
        ctx.draw_rect(g, Color::rgb(200, 200, 200));
        ctx.draw_text(
            Point::new(g.x + 4, g.y + 16),
            self.title(),
            &Font::default_ui(),
            Color::BLACK,
        );
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn web_view_draw_produces_svg() {
        let mut wv = WebView::new(Rect::new(0, 0, 300, 200));
        let svg = crate::widget::svg::render_to_svg(&mut wv);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn web_view_url_defaults() {
        let wv = WebView::new(Rect::new(0, 0, 300, 200));
        assert_eq!(wv.url(), "about:blank");
        assert!(!wv.is_loading());
        assert!(wv.title().is_empty());
    }

    #[test]
    fn web_view_set_url_starts_then_finishes_on_timer() {
        let mut wv = WebView::new(Rect::new(0, 0, 300, 200));
        let started = Arc::new(Mutex::new(Vec::<String>::new()));
        let finished = Arc::new(Mutex::new(Vec::<String>::new()));

        let started_sink = started.clone();
        wv.loading_started.connect(move |url| {
            if let Ok(mut guard) = started_sink.lock() {
                guard.push(url.as_ref().clone());
            }
        });

        let finished_sink = finished.clone();
        wv.loading_finished.connect(move |url| {
            if let Ok(mut guard) = finished_sink.lock() {
                guard.push(url.as_ref().clone());
            }
        });

        wv.set_url("https://example.com".to_string());
        assert!(wv.is_loading());
        assert_eq!(started.lock().expect("started lock poisoned").len(), 1);
        assert_eq!(finished.lock().expect("finished lock poisoned").len(), 0);

        wv.handle_event(&Event::timer(WebView::load_timer_id()));
        assert!(!wv.is_loading());
        assert_eq!(finished.lock().expect("finished lock poisoned").len(), 1);
    }

    #[test]
    fn web_view_stop_finishes_pending_load() {
        let mut wv = WebView::new(Rect::new(0, 0, 300, 200));
        wv.set_url("https://rust-lang.org".to_string());
        assert!(wv.is_loading());

        wv.stop();
        assert!(!wv.is_loading());
    }
}
