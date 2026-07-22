//! PagerPageView — horizontal page view with dot indicators (ViewPager-style).
//!
//! Displays one page at a time from a list of child widgets. The user can
//! navigate between pages using left/right arrow keys. An optional row of
//! dot indicators is shown at the bottom.

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Dyn-safe wrapper trait that combines Widget access with a draw method.
/// This is necessary because `Draw` is not dyn-compatible.
pub trait WidgetAndDraw: Widget {
    /// Draw this widget using the provided render context.
    fn draw_widget(&mut self, context: &mut RenderContext);
}

impl<T: Widget + Draw> WidgetAndDraw for T {
    fn draw_widget(&mut self, context: &mut RenderContext) {
        self.draw(context);
    }
}

/// Horizontal page view with dot indicators.
///
/// Shows one child widget at a time. Arrow key events navigate between pages.
/// Dot indicators are rendered at the bottom when enabled.
/// Emits `page_changed` when the current page index changes.
pub struct PagerPageView {
    base: BaseWidget,
    pages: Vec<Box<dyn WidgetAndDraw>>,
    current_page: usize,
    page_indicator_visible: bool,
    /// Emitted when the current page changes with the new page index.
    pub page_changed: Signal1<usize>,
}

impl PagerPageView {
    /// Creates a new PagerPageView with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let base = BaseWidget::new(WidgetKind::PagerPageView, geometry, "PagerPageView");
        Self {
            base,
            pages: Vec::new(),
            current_page: 0,
            page_indicator_visible: true,
            page_changed: Signal1::new(),
        }
    }

    /// Adds a page widget at the end.
    pub fn add_page(&mut self, widget: Box<dyn WidgetAndDraw>) {
        self.pages.push(widget);
        self.base.request_redraw();
    }

    /// Removes the page at the given index.
    /// Returns the removed widget if the index was valid.
    pub fn remove_page(&mut self, index: usize) -> Option<Box<dyn WidgetAndDraw>> {
        if index < self.pages.len() {
            let result = Some(self.pages.remove(index));
            if self.current_page >= self.pages.len() && !self.pages.is_empty() {
                self.current_page = self.pages.len() - 1;
            } else if self.pages.is_empty() {
                self.current_page = 0;
            }
            self.base.request_redraw();
            result
        } else {
            None
        }
    }

    /// Returns the number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Returns the current page index.
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Sets the current page index. Clamped to valid range.
    pub fn set_current_page(&mut self, index: usize) {
        if self.pages.is_empty() {
            self.current_page = 0;
            return;
        }
        let clamped = index.min(self.pages.len() - 1);
        if clamped != self.current_page {
            self.current_page = clamped;
            self.page_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Sets whether dot indicators are visible.
    pub fn set_show_indicator(&mut self, visible: bool) {
        self.page_indicator_visible = visible;
        self.base.request_redraw();
    }

    /// Returns whether dot indicators are visible.
    pub fn show_indicator(&self) -> bool {
        self.page_indicator_visible
    }
}

impl Widget for PagerPageView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::PagerPageView
    }
}

impl Draw for PagerPageView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // ── Background ──
        context.fill_rect(rect, Color::rgba(245, 245, 245, 255));

        if self.pages.is_empty() {
            return;
        }

        // ── Draw current page content ──
        let page = &mut self.pages[self.current_page];
        page.set_geometry(rect);
        page.draw_widget(context);

        // ── Dot indicators (bottom) ──
        if self.page_indicator_visible && self.pages.len() > 1 {
            let dot_count = self.pages.len();
            let dot_radius = 4u32;
            let dot_spacing = 20i32;
            let total_dots_width = (dot_count as i32) * dot_spacing;
            let start_x = rect.x + (rect.width as i32 - total_dots_width) / 2 + dot_spacing / 2;
            let dots_y = rect.y + rect.height as i32 - 24;

            for i in 0..dot_count {
                let cx = start_x + (i as i32) * dot_spacing;
                let is_active = i == self.current_page;
                let color = if is_active {
                    Color::rgba(0, 122, 255, 255) // iOS blue
                } else {
                    Color::rgba(180, 180, 180, 200)
                };
                context.fill_circle_aa(Point::new(cx, dots_y), dot_radius, color);
            }
        }
    }
}

impl EventHandler for PagerPageView {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::KeyPress { key, .. } => {
                let left_arrow = 37u32;
                let right_arrow = 39u32;

                if *key == left_arrow {
                    if self.current_page > 0 {
                        self.current_page -= 1;
                        self.page_changed.emit(self.current_page);
                        self.base.request_redraw();
                    }
                } else if *key == right_arrow
                    && !self.pages.is_empty() && self.current_page + 1 < self.pages.len() {
                        self.current_page += 1;
                        self.page_changed.emit(self.current_page);
                        self.base.request_redraw();
                    }
            }
            Event::KeyDown((key, _)) => {
                let left_arrow = 37u32;
                let right_arrow = 39u32;

                if *key == left_arrow {
                    if self.current_page > 0 {
                        self.current_page -= 1;
                        self.page_changed.emit(self.current_page);
                        self.base.request_redraw();
                    }
                } else if *key == right_arrow
                    && !self.pages.is_empty() && self.current_page + 1 < self.pages.len() {
                        self.current_page += 1;
                        self.page_changed.emit(self.current_page);
                        self.base.request_redraw();
                    }
            }
            _ => {
                // Delegate events to the current page
                if !self.pages.is_empty() && self.current_page < self.pages.len() {
                    self.pages[self.current_page].handle_event(event);
                } else {
                    self.base.handle_event(event);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::base_widgets::label::Label;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn pager_page_view_creation() {
        let pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        assert_eq!(pv.kind(), WidgetKind::PagerPageView);
        assert_eq!(pv.page_count(), 0);
        assert_eq!(pv.current_page(), 0);
        assert!(pv.show_indicator());
    }

    #[test]
    fn pager_page_view_add_and_count() {
        let mut pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        pv.add_page(Box::new(Label::new("Page 1".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("Page 2".to_string(), Rect::new(0, 0, 300, 400))));
        assert_eq!(pv.page_count(), 2);
    }

    #[test]
    fn pager_page_view_remove_page() {
        let mut pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        pv.add_page(Box::new(Label::new("A".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("B".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("C".to_string(), Rect::new(0, 0, 300, 400))));
        assert_eq!(pv.page_count(), 3);

        let removed = pv.remove_page(1);
        assert!(removed.is_some());
        assert_eq!(pv.page_count(), 2);

        // Invalid index
        let removed = pv.remove_page(10);
        assert!(removed.is_none());
        assert_eq!(pv.page_count(), 2);
    }

    #[test]
    fn pager_page_view_set_current_page() {
        let mut pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        pv.add_page(Box::new(Label::new("A".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("B".to_string(), Rect::new(0, 0, 300, 400))));

        pv.set_current_page(1);
        assert_eq!(pv.current_page(), 1);

        // Clamp
        pv.set_current_page(10);
        assert_eq!(pv.current_page(), 1);
    }

    #[test]
    fn pager_page_view_page_changed_signal() {
        let mut pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        pv.add_page(Box::new(Label::new("A".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("B".to_string(), Rect::new(0, 0, 300, 400))));

        let fired = Arc::new(AtomicBool::new(false));
        let last_page = Arc::new(std::sync::Mutex::new(0usize));
        let f = fired.clone();
        let lp = last_page.clone();
        pv.page_changed.connect(move |idx| {
            f.store(true, Ordering::SeqCst);
            *lp.lock().unwrap() = *idx;
        });

        pv.set_current_page(1);
        assert!(fired.load(Ordering::SeqCst));
        assert_eq!(*last_page.lock().unwrap(), 1);
    }

    #[test]
    fn pager_page_view_arrow_key_navigation() {
        let mut pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        pv.add_page(Box::new(Label::new("A".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("B".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("C".to_string(), Rect::new(0, 0, 300, 400))));
        assert_eq!(pv.current_page(), 0);

        // Right arrow -> page 1
        pv.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(pv.current_page(), 1);

        // Right arrow -> page 2
        pv.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(pv.current_page(), 2);

        // Right arrow -> stays at page 2 (last page)
        pv.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(pv.current_page(), 2);

        // Left arrow -> page 1
        pv.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(pv.current_page(), 1);

        // Left arrow -> page 0
        pv.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(pv.current_page(), 0);

        // Left arrow -> stays at page 0 (first page)
        pv.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(pv.current_page(), 0);
    }

    #[test]
    fn pager_page_view_toggle_indicator() {
        let mut pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        assert!(pv.show_indicator());

        pv.set_show_indicator(false);
        assert!(!pv.show_indicator());

        pv.set_show_indicator(true);
        assert!(pv.show_indicator());
    }

    #[test]
    fn pager_page_view_svg_output() {
        let mut pv = PagerPageView::new(Rect::new(0, 0, 300, 400));
        pv.add_page(Box::new(Label::new("Hello".to_string(), Rect::new(0, 0, 300, 400))));
        pv.add_page(Box::new(Label::new("World".to_string(), Rect::new(0, 0, 300, 400))));
        let svg = render_to_svg(&mut pv);
        assert!(svg.starts_with("<svg"));
    }
}
