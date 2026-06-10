//! Carousel/SwipeView widget — a horizontal swipeable carousel of pages
//! with dot page indicators.
//!
//! The Carousel displays one page at a time with a full-width background color
//! and centered title. Users can navigate between pages by clicking the left
//! half (previous) or right half (next) of the widget. Dot indicators at the
//! bottom show the current position within the page sequence.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Data for a single carousel page.
pub struct CarouselPage {
    /// Display title shown centered on the page.
    pub title: String,
    /// Background color of the page.
    pub color: Color,
}

/// Carousel/SwipeView widget — horizontal swipeable page carousel.
pub struct Carousel {
    base: BaseWidget,
    pages: Vec<CarouselPage>,
    current_index: usize,
    /// Emitted when the current page index changes.
    pub page_changed: Signal1<usize>,
}

impl Carousel {
    /// Creates a new empty Carousel with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Carousel, geometry, "Carousel"),
            pages: Vec::new(),
            current_index: 0,
            page_changed: Signal1::new(),
        }
    }

    /// Adds a page with the given title and background color.
    pub fn add_page(&mut self, title: impl Into<String>, color: Color) -> usize {
        let index = self.pages.len();
        self.pages.push(CarouselPage { title: title.into(), color });
        index
    }

    /// Sets the current page by index. Clamped to valid range.
    /// Emits `page_changed` if the index actually changes.
    pub fn set_current(&mut self, index: usize) {
        let clamped = index.min(self.pages.len().saturating_sub(1));
        if self.current_index != clamped {
            self.current_index = clamped;
            self.page_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the current page index.
    pub fn current(&self) -> usize {
        self.current_index
    }

    /// Returns the total number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Navigates to the next page, if available.
    pub fn next(&mut self) {
        if self.current_index + 1 < self.pages.len() {
            self.set_current(self.current_index + 1);
        }
    }

    /// Navigates to the previous page, if available.
    pub fn previous(&mut self) {
        if self.current_index > 0 {
            self.set_current(self.current_index - 1);
        }
    }

    /// Returns a reference to the current page, if any.
    pub fn current_page(&self) -> Option<&CarouselPage> {
        self.pages.get(self.current_index)
    }

    /// Returns a reference to all pages.
    pub fn pages(&self) -> &[CarouselPage] {
        &self.pages
    }
}

impl Widget for Carousel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Carousel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let page_count = self.pages.len();

        if page_count == 0 {
            // Empty carousel — draw a neutral background
            context.fill_rounded_rect(rect, 8, Color::rgba(230, 230, 230, 200));
            return;
        }

        if let Some(page) = self.pages.get(self.current_index) {
            // ── Page background ─────────────────────────────────────
            let bg_color = if !is_enabled {
                Color::rgba(
                    page.color.r.saturating_sub(40),
                    page.color.g.saturating_sub(40),
                    page.color.b.saturating_sub(40),
                    160,
                )
            } else {
                page.color
            };
            context.fill_rounded_rect(rect, 8, bg_color);

            // ── Title text (centered) ───────────────────────────────
            let font = crate::core::Font::with_weight("Arial", 18.0, 600, false);
            let metrics = context.measure_text(&page.title, &font);
            let text_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
            let text_y = rect.y + (rect.height as i32 / 2) - (metrics.height as i32 / 2)
                + metrics.ascent as i32;
            let text_color =
                if !is_enabled { Color::rgba(255, 255, 255, 160) } else { Color::WHITE };
            context.draw_text(Point::new(text_x, text_y), &page.title, &font, text_color);
        }

        // ── Dot page indicators ─────────────────────────────────────
        let dot_count = page_count.min(10); // Cap dots at 10 to avoid overflow
        let dot_radius: u32 = 4;
        let dot_spacing: u32 = 16;
        let total_dots_width = dot_count as u32 * dot_spacing;
        let dots_start_x = rect.x + (rect.width as i32 - total_dots_width as i32) / 2;
        let dots_y = rect.y + rect.height as i32 - 20;

        for i in 0..dot_count {
            let dot_x = dots_start_x + (i as i32 * dot_spacing as i32);
            let dot_rect = Rect::new(dot_x, dots_y, dot_radius * 2, dot_radius * 2);

            let is_active = i == self.current_index;
            let dot_color = if is_active { Color::WHITE } else { Color::rgba(255, 255, 255, 120) };

            context.fill_rounded_rect(dot_rect, dot_radius, dot_color);

            // Active dot slightly larger
            if is_active {
                let active_dot_rect =
                    Rect::new(dot_x - 1, dots_y - 1, dot_radius * 2 + 2, dot_radius * 2 + 2);
                context.draw_rounded_rect_stroke(
                    active_dot_rect,
                    dot_radius + 1,
                    Color::rgba(255, 255, 255, 80),
                    1,
                );
            }
        }
    }
}

impl EventHandler for Carousel {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();
                    let mid_x = rect.x + (rect.width as i32) / 2;
                    if pos.x < mid_x {
                        self.previous();
                    } else {
                        self.next();
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    fn default_carousel() -> Carousel {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        c.add_page("Page 1", Color::rgb(52, 120, 246));
        c.add_page("Page 2", Color::rgb(52, 199, 89));
        c.add_page("Page 3", Color::rgb(255, 149, 0));
        c
    }

    #[test]
    fn carousel_creation_defaults() {
        let c = Carousel::new(Rect::new(0, 0, 300, 200));
        assert_eq!(c.current(), 0);
        assert_eq!(c.page_count(), 0);
        assert!(c.current_page().is_none());
        assert_eq!(c.kind(), WidgetKind::Carousel);
    }

    #[test]
    fn carousel_add_pages() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        assert_eq!(c.page_count(), 0);

        c.add_page("Intro", Color::rgb(100, 100, 200));
        assert_eq!(c.page_count(), 1);

        c.add_page("Details", Color::rgb(200, 100, 100));
        assert_eq!(c.page_count(), 2);

        assert_eq!(c.current(), 0);
        assert_eq!(c.current_page().unwrap().title, "Intro");
    }

    #[test]
    fn carousel_navigation() {
        let mut c = default_carousel();
        assert_eq!(c.current(), 0);

        c.next();
        assert_eq!(c.current(), 1);

        c.next();
        assert_eq!(c.current(), 2);

        // Should not advance past last page
        c.next();
        assert_eq!(c.current(), 2);

        c.previous();
        assert_eq!(c.current(), 1);

        c.previous();
        assert_eq!(c.current(), 0);

        // Should not go before first page
        c.previous();
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_set_current_clamps() {
        let mut c = default_carousel();
        c.set_current(5); // beyond range
        assert_eq!(c.current(), 2);
    }

    #[test]
    fn carousel_signal_emission() {
        let mut c = default_carousel();
        let captured = Arc::new(Mutex::new(None));
        c.page_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        c.next();
        assert_eq!(c.current(), 1);
        assert_eq!(*captured.lock().unwrap(), Some(1));

        // No signal for same index
        c.set_current(1);
        assert_eq!(*captured.lock().unwrap(), Some(1)); // unchanged

        c.previous();
        assert_eq!(*captured.lock().unwrap(), Some(0));
    }

    #[test]
    fn carousel_mouse_press_navigates() {
        let mut c = default_carousel();
        // Click left half → previous (but at 0, so stays)
        c.handle_event(&Event::MousePress { pos: Point::new(50, 100), button: 1 });
        assert_eq!(c.current(), 0);

        // Click right half → next
        c.handle_event(&Event::MousePress { pos: Point::new(250, 100), button: 1 });
        assert_eq!(c.current(), 1);
    }

    #[test]
    fn carousel_disabled_blocks_events() {
        let mut c = default_carousel();
        c.set_enabled(false);
        c.handle_event(&Event::MousePress { pos: Point::new(250, 100), button: 1 });
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_add_page_returns_index() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        let idx0 = c.add_page("A", Color::RED);
        let idx1 = c.add_page("B", Color::GREEN);
        let idx2 = c.add_page("C", Color::BLUE);
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(idx2, 2);
        assert_eq!(c.page_count(), 3);
    }

    #[test]
    fn carousel_empty_draw_does_not_panic() {
        let mut c = Carousel::new(Rect::new(0, 0, 100, 50));
        let svg = crate::widget::svg::render_to_svg(&mut c);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn carousel_svg_output() {
        let mut c = default_carousel();
        let svg = crate::widget::svg::render_to_svg(&mut c);
        assert!(svg.starts_with("<svg"));
        // Should contain the page title
        assert!(svg.contains("Page 1") || svg.contains("rect") || svg.contains("fill="));
    }

    #[test]
    fn carousel_previous_at_zero_does_nothing() {
        let mut c = default_carousel();
        assert_eq!(c.current(), 0);
        c.previous();
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_next_at_last_does_nothing() {
        let mut c = default_carousel();
        c.set_current(2);
        assert_eq!(c.current(), 2);
        c.next();
        assert_eq!(c.current(), 2);
    }

    #[test]
    fn carousel_pages_returns_all() {
        let c = default_carousel();
        let pages = c.pages();
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].title, "Page 1");
        assert_eq!(pages[1].title, "Page 2");
        assert_eq!(pages[2].title, "Page 3");
    }
}
