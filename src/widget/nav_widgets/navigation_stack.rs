//! NavigationStack widget — a push/pop page navigation container.
//!
//! The NavigationStack widget manages a stack of pages (widgets) and displays the
//! topmost page along with a navigation bar. It supports push, pop, and pop-to-root
//! operations, similar to SwiftUI NavigationStack or UINavigationController.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Events emitted by NavigationStack when the page stack changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationEvent {
    /// A new page was pushed onto the stack.
    Pushed,
    /// The top page was popped from the stack.
    Popped,
    /// All pages were popped back to the root.
    PoppedToRoot,
}

/// Height of the navigation bar in logical pixels.
const NAV_BAR_HEIGHT: u32 = 44;

/// NavigationStack widget — a page-based navigation container.
///
/// Manages a stack of pages where only the topmost page is visible.
/// A navigation bar at the top shows the current page title and a back button
/// when there are pages below the top.
pub struct NavigationStack {
    base: BaseWidget,
    pages: Vec<Box<dyn Widget>>,
    navigation_bar_title: String,
    /// Emitted when the navigation state changes.
    pub navigation_changed: Signal1<NavigationEvent>,
}

impl NavigationStack {
    /// Creates a new NavigationStack widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::NavigationStack, geometry, "NavigationStack"),
            pages: Vec::new(),
            navigation_bar_title: String::new(),
            navigation_changed: Signal1::new(),
        }
    }

    /// Pushes a new page onto the navigation stack.
    /// The new page becomes the visible topmost page.
    pub fn push(&mut self, page: Box<dyn Widget>) {
        self.pages.push(page);
        self.navigation_changed.emit(NavigationEvent::Pushed);
        self.base.request_redraw();
    }

    /// Pops the topmost page from the stack and returns it.
    /// Returns `None` if there is only one page (the root) or the stack is empty.
    pub fn pop(&mut self) -> Option<Box<dyn Widget>> {
        if self.pages.len() <= 1 {
            return None;
        }
        let popped = self.pages.pop();
        self.navigation_changed.emit(NavigationEvent::Popped);
        self.base.request_redraw();
        popped
    }

    /// Returns a reference to the current (topmost) page, or `None` if the stack is empty.
    pub fn current_page(&self) -> Option<&dyn Widget> {
        self.pages.last().map(|p| p.as_ref())
    }

    /// Returns a mutable reference to the current (topmost) page, or `None` if the stack is empty.
    pub fn current_page_mut(&mut self) -> Option<&mut dyn Widget> {
        self.pages.last_mut().map(|p| p.as_mut())
    }

    /// Returns the number of pages in the stack.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Returns whether the stack has more than one page (i.e., popping is possible).
    pub fn can_pop(&self) -> bool {
        self.pages.len() > 1
    }

    /// Pops all pages except the root page (the first page).
    pub fn pop_to_root(&mut self) {
        if self.pages.is_empty() {
            return;
        }
        self.pages.drain(1..);
        self.navigation_changed.emit(NavigationEvent::PoppedToRoot);
        self.base.request_redraw();
    }

    /// Returns the current navigation bar title.
    pub fn navigation_bar_title(&self) -> &str {
        &self.navigation_bar_title
    }

    /// Sets the navigation bar title.
    pub fn set_navigation_bar_title(&mut self, title: &str) {
        self.navigation_bar_title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the content area rect (below the navigation bar).
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x,
            rect.y + NAV_BAR_HEIGHT as i32,
            rect.width,
            rect.height.saturating_sub(NAV_BAR_HEIGHT),
        )
    }

    /// Returns the navigation bar rect.
    fn nav_bar_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, NAV_BAR_HEIGHT.min(rect.height))
    }

    /// Returns the title to display in the navigation bar.
    fn display_title(&self) -> String {
        if !self.navigation_bar_title.is_empty() {
            self.navigation_bar_title.clone()
        } else if let Some(page) = self.current_page() {
            format!("{:?}", page.kind())
        } else {
            "Navigation".to_string()
        }
    }
}

impl Widget for NavigationStack {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for NavigationStack {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let is_enabled = self.base.is_enabled();

        // ── Draw Navigation Bar ──
        let nav_rect = self.nav_bar_rect();
        // Nav bar background
        context.fill_rect(nav_rect, Color::rgb(245, 246, 248));
        // Nav bar bottom border
        context.draw_line(
            Point::new(nav_rect.x, nav_rect.y + nav_rect.height as i32 - 1),
            Point::new(nav_rect.x + nav_rect.width as i32, nav_rect.y + nav_rect.height as i32 - 1),
            Color::rgba(200, 200, 200, 200),
        );

        // Back button (if can_pop)
        if self.can_pop() {
            let back_text = "< Back";
            let back_font = Font::simple("sans-serif", 13.0);
            let back_color = if is_enabled { Color::PRIMARY } else { Color::DISABLED_FOREGROUND };
            context.draw_text(
                Point::new(nav_rect.x + 8, nav_rect.y + 14),
                back_text,
                &back_font,
                back_color,
                HorizontalAlignment::Left,
            );
        }

        // Title
        let title_font = Font::simple("sans-serif", 15.0);
        let title = self.display_title();
        let text_color = if is_enabled { Color::BLACK } else { Color::DISABLED_FOREGROUND };
        let metrics = context.measure_text(&title, &title_font);
        let title_x = nav_rect.x + (nav_rect.width as i32 - metrics.width as i32) / 2;
        let title_y = nav_rect.y + 14;
        context.draw_text(
            Point::new(title_x.max(nav_rect.x + 4), title_y),
            &title,
            &title_font,
            text_color,
            HorizontalAlignment::Left,
        );

        // ── Draw content area background (light fill) ──
        let content = self.content_rect();
        if content.width > 0 && content.height > 0 {
            context.fill_rect(content, Color::WHITE);
        }
    }
}

impl EventHandler for NavigationStack {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    // Check if the back button was clicked
                    if self.can_pop() {
                        let nav_rect = self.nav_bar_rect();
                        let back_rect = Rect::new(nav_rect.x, nav_rect.y, 60, NAV_BAR_HEIGHT);
                        if back_rect.contains_point(*pos) {
                            self.pop();
                            return;
                        }
                    }

                    // Forward to current page
                    let content = self.content_rect();
                    if content.contains_point(*pos) {
                        if let Some(page) = self.pages.last_mut() {
                            page.handle_event(event);
                        }
                    }
                }
            }
            Event::MouseRelease { pos, button: _ } | Event::MouseMove { pos } => {
                let content = self.content_rect();
                if content.contains_point(*pos) {
                    if let Some(page) = self.pages.last_mut() {
                        page.handle_event(event);
                    }
                }
            }
            _ => {
                if let Some(page) = self.pages.last_mut() {
                    page.handle_event(event);
                }
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::display_widgets::badge::Badge;
    use std::sync::{Arc, Mutex};

    #[test]
    fn navigation_stack_default_creation() {
        let stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        assert_eq!(stack.kind(), WidgetKind::NavigationStack);
        assert_eq!(stack.page_count(), 0);
        assert!(!stack.can_pop());
        assert!(stack.current_page().is_none());
    }

    #[test]
    fn navigation_stack_push_and_pop() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 1);
        assert!(!stack.can_pop()); // Only one page, can't pop

        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 2);
        assert!(stack.can_pop());

        let popped = stack.pop();
        assert!(popped.is_some());
        assert_eq!(stack.page_count(), 1);
        assert!(!stack.can_pop());
    }

    #[test]
    fn navigation_stack_pop_to_root() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 3);

        stack.pop_to_root();
        assert_eq!(stack.page_count(), 1);
        assert!(!stack.can_pop());
    }

    #[test]
    fn navigation_stack_navigation_changed_signal() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        let events = Arc::new(Mutex::new(Vec::new()));

        stack.navigation_changed.connect({
            let events = Arc::clone(&events);
            move |event: Arc<NavigationEvent>| {
                events.lock().unwrap().push(event.as_ref().clone());
            }
        });

        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.pop();
        stack.pop_to_root();

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 4);
        assert_eq!(captured[0], NavigationEvent::Pushed);
        assert_eq!(captured[1], NavigationEvent::Pushed);
        assert_eq!(captured[2], NavigationEvent::Popped);
        assert_eq!(captured[3], NavigationEvent::PoppedToRoot);
    }

    #[test]
    fn navigation_stack_current_page() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        assert!(stack.current_page().is_none());

        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert!(stack.current_page().is_some());
    }

    #[test]
    fn navigation_stack_back_button_click() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 2);

        // Click on back button area (left side of nav bar)
        stack.handle_event(&Event::mouse_press(5, 10, 1));
        assert_eq!(stack.page_count(), 1);
    }

    #[test]
    fn navigation_stack_set_title() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        assert_eq!(stack.navigation_bar_title(), "");

        stack.set_navigation_bar_title("Settings");
        assert_eq!(stack.navigation_bar_title(), "Settings");
    }
}
