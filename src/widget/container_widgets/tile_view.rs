//! TileView widget — swipeable tiled page view (BLUE13 R2.8).
//!
//! A `TileView` displays one page (tile) at a time with horizontal swipe
//! navigation. Users can switch pages via left/right arrow keys or programmatic
//! control. A dot indicator at the bottom shows the current page position.

use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A tile view that displays one page at a time with horizontal swipe navigation.
pub struct TileView {
    base: BaseWidget,
    /// Number of tiles/pages.
    page_count: u32,
    /// Currently visible page index.
    current_page: u32,
    /// Signal emitted on page change.
    pub page_changed: Signal1<u32>,
}

impl TileView {
    /// Creates a new TileView widget with the given geometry.
    ///
    /// Defaults: 1 page, current page 0.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TileView, rect, "TileView"),
            page_count: 1,
            current_page: 0,
            page_changed: Signal1::new(),
        }
    }

    /// Returns the number of tiles/pages.
    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    /// Sets the number of tiles/pages and clamps the current page if needed.
    pub fn set_page_count(&mut self, count: u32) {
        let count = count.max(1);
        self.page_count = count;
        if self.current_page >= self.page_count {
            self.current_page = self.page_count.saturating_sub(1);
        }
    }

    /// Returns the currently visible page index.
    pub fn current_page(&self) -> u32 {
        self.current_page
    }

    /// Sets the currently visible page index, clamped to valid range.
    ///
    /// Emits `page_changed` when the page actually changes.
    pub fn set_current_page(&mut self, page: u32) {
        let clamped = page.min(self.page_count.saturating_sub(1));
        if self.current_page == clamped {
            return;
        }
        self.current_page = clamped;
        self.page_changed.emit(self.current_page);
        self.base.changed.emit();
    }

    /// Navigate to the next page.
    fn next_page(&mut self) {
        if self.page_count > 1 {
            self.set_current_page(self.current_page + 1);
        }
    }

    /// Navigate to the previous page.
    fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.set_current_page(self.current_page - 1);
        }
    }
}

impl Widget for TileView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
    }
}

impl EventHandler for TileView {
    fn handle_event(&mut self, event: &Event) {
        // Delegate to base for signal emissions.
        self.base.handle_event(event);

        // Handle left/right arrow keys for page navigation.
        if let Event::KeyDown((key, _modifiers)) = event {
            match *key {
                37 => {
                    // Left arrow -> previous page
                    self.prev_page();
                }
                39 => {
                    // Right arrow -> next page
                    self.next_page();
                }
                _ => {}
            }
        }
    }
}

impl Draw for TileView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Draw background.
        let bg = self.style().background_color.unwrap_or(Color::WHITE);
        context.fill_rect(rect, bg);

        // Draw a thin border to define the view area.
        let border = self.style().border_color.unwrap_or(Color::from_rgb(200, 200, 200));
        context.draw_rect_stroke(rect, border, 1);

        // Draw the current page number text centered in the tile area.
        let text = format!("Page {}", self.current_page + 1);
        let font = Font::default();
        let metrics = context.measure_text(&text, &font);
        let text_color = self.style().text_color.unwrap_or(Color::from_rgb(60, 60, 60));
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;
        let text_x = cx - (metrics.width as i32 / 2);
        let text_y = cy - (metrics.height as i32 / 2);
        context.draw_text(Point::new(text_x, text_y), &text, &font, text_color);

        // Draw page indicator dots at the bottom.
        let dot_count = self.page_count;
        if dot_count > 0 {
            let dot_radius: u32 = 3;
            let dot_spacing: i32 = 12;
            let dots_total_width = (dot_count as i32 - 1) * dot_spacing;
            let start_x = rect.x + (rect.width as i32 - dots_total_width) / 2;
            let dots_y = rect.y + rect.height as i32 - 14_i32;

            for i in 0..dot_count {
                let dx = start_x + (i as i32) * dot_spacing;
                let center = Point::new(dx, dots_y);

                if i == self.current_page {
                    // Filled dot for current page.
                    let dot_color = self.style().text_color.unwrap_or(Color::from_rgb(0, 120, 215));
                    context.fill_circle(center, dot_radius, dot_color);
                } else {
                    // Outline dot for other pages.
                    let dot_color =
                        self.style().border_color.unwrap_or(Color::from_rgb(180, 180, 180));
                    context.draw_circle_stroke(center, dot_radius, dot_color, 1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    #[test]
    fn tile_view_creation() {
        let tv = TileView::new(Rect::new(0, 0, 300, 400));
        assert_eq!(tv.page_count(), 1);
        assert_eq!(tv.current_page(), 0);
        assert_eq!(tv.kind(), WidgetKind::TileView);
    }

    #[test]
    fn tile_view_switch_page() {
        let mut tv = TileView::new(Rect::new(0, 0, 300, 400));
        tv.set_page_count(3);
        assert_eq!(tv.page_count(), 3);

        tv.set_current_page(1);
        assert_eq!(tv.current_page(), 1);

        // Clamp above max.
        tv.set_current_page(10);
        assert_eq!(tv.current_page(), 2);

        // Navigate back.
        tv.set_current_page(0);
        assert_eq!(tv.current_page(), 0);
    }

    #[test]
    fn tile_view_page_count() {
        let mut tv = TileView::new(Rect::new(0, 0, 300, 400));
        tv.set_page_count(5);
        assert_eq!(tv.page_count(), 5);

        // Minimum is 1.
        tv.set_page_count(0);
        assert_eq!(tv.page_count(), 1);

        // Current page clamps when count shrinks.
        // First set to page 4 (0-indexed, so page_count=5 means valid range is 0-4).
        tv.set_page_count(5);
        tv.set_current_page(4);
        tv.set_page_count(3);
        // 4 >= 3 → clamped to 2 (3-1).
        assert_eq!(tv.current_page(), 2);

        // Clamp to last valid page when count shrinks further.
        tv.set_page_count(1);
        assert_eq!(tv.current_page(), 0);
    }

    #[test]
    fn tile_view_draw_no_panic() {
        let mut tv = TileView::new(Rect::new(0, 0, 300, 400));
        tv.set_page_count(4);
        tv.set_current_page(2);

        let mut backend = SoftwarePaintBackend::new(Size::new(300, 400), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        tv.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn tile_view_draw_zero_geometry_no_panic() {
        let mut tv = TileView::new(Rect::new(0, 0, 0, 0));
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        tv.draw(&mut context);
        backend.end_frame();
    }

    #[test]
    fn tile_view_keyboard_navigation() {
        let mut tv = TileView::new(Rect::new(0, 0, 300, 400));
        tv.set_page_count(3);

        // Initially page 0.
        assert_eq!(tv.current_page(), 0);

        // Right arrow -> page 1.
        tv.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(tv.current_page(), 1);

        // Right arrow -> page 2.
        tv.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(tv.current_page(), 2);

        // Right arrow -> stays at page 2 (last page).
        tv.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(tv.current_page(), 2);

        // Left arrow -> page 1.
        tv.handle_event(&Event::KeyDown((37, 0)));
        assert_eq!(tv.current_page(), 1);

        // Left arrow -> page 0.
        tv.handle_event(&Event::KeyDown((37, 0)));
        assert_eq!(tv.current_page(), 0);

        // Left arrow -> stays at page 0 (first page).
        tv.handle_event(&Event::KeyDown((37, 0)));
        assert_eq!(tv.current_page(), 0);
    }

    #[test]
    fn tile_view_page_changed_signal() {
        let mut tv = TileView::new(Rect::new(0, 0, 300, 400));
        tv.set_page_count(3);

        let last_page = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let lp = last_page.clone();
        tv.page_changed.connect(move |p| {
            *lp.lock().unwrap() = *p;
        });

        tv.set_current_page(1);
        assert_eq!(*last_page.lock().unwrap(), 1);

        tv.set_current_page(2);
        assert_eq!(*last_page.lock().unwrap(), 2);
    }

    #[test]
    fn tile_view_size_hint() {
        let tv = TileView::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tv.size_hint(), Size::new(200, 200));
    }

    #[test]
    fn tile_view_geometry_delegation() {
        let mut tv = TileView::new(Rect::new(0, 0, 300, 400));
        tv.set_geometry(Rect::new(10, 10, 200, 300));
        assert_eq!(tv.geometry(), Rect::new(10, 10, 200, 300));
    }
}
