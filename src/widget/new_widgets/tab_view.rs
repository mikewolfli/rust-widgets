//! TabView widget — iOS-style segmented tab page view.
//!
//! Displays a horizontal segmented tab bar at the top and a content area
//! below showing the selected tab's content. Supports add/remove/clear
//! operations on tabs and emits a `tab_changed` signal on selection.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A single tab page with a title, optional content, and optional icon name.
pub struct TabPage {
    /// Display title shown in the tab bar.
    pub title: String,
    /// Optional content widget displayed when this tab is selected.
    pub content: Option<Box<dyn Widget>>,
    /// Optional icon identifier for the tab.
    pub icon: Option<String>,
}

/// iOS-style segmented tab page view.
///
/// Manages a vector of `TabPage` instances and draws a top segmented bar
/// plus the content of the currently selected tab below.
pub struct TabView {
    base: BaseWidget,
    tabs: Vec<TabPage>,
    selected_index: usize,
    /// Emitted when the selected tab index changes.
    pub tab_changed: Signal1<usize>,
}

impl TabView {
    /// Creates a new empty TabView widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabView, geometry, "TabView"),
            tabs: Vec::new(),
            selected_index: 0,
            tab_changed: Signal1::new(),
        }
    }

    /// Adds a new tab page at the end of the tab list.
    /// If this is the first tab, it becomes the selected tab.
    pub fn add_tab(
        &mut self,
        title: impl Into<String>,
        content: Option<Box<dyn Widget>>,
        icon: Option<impl Into<String>>,
    ) {
        let was_empty = self.tabs.is_empty();
        self.tabs.push(TabPage { title: title.into(), content, icon: icon.map(|i| i.into()) });
        if was_empty {
            self.set_current_index(0);
        }
        self.base.request_redraw();
    }

    /// Removes the tab at the given index.
    /// Adjusts selection if the removed tab was selected.
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.tabs.len() {
            self.selected_index = self.tabs.len() - 1;
        }
        self.base.request_redraw();
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Removes all tabs and resets the selection.
    pub fn clear_tabs(&mut self) {
        self.tabs.clear();
        self.selected_index = 0;
        self.base.request_redraw();
    }

    /// Sets the current tab index. Clamped to valid range.
    /// Emits `tab_changed` if the index actually changed.
    pub fn set_current_index(&mut self, index: usize) {
        if self.tabs.is_empty() {
            return;
        }
        let clamped = index.min(self.tabs.len() - 1);
        if self.selected_index != clamped {
            self.selected_index = clamped;
            self.tab_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected tab index.
    pub fn current_index(&self) -> usize {
        self.selected_index
    }

    /// Returns a reference to the tabs vector.
    pub fn tabs(&self) -> &[TabPage] {
        &self.tabs
    }

    /// Returns a mutable reference to the tabs vector.
    pub fn tabs_mut(&mut self) -> &mut Vec<TabPage> {
        &mut self.tabs
    }
}

impl Widget for TabView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for TabView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let tab_bar_height: u32 = 40;
        let tab_bar_rect = Rect::new(rect.x, rect.y, rect.width, tab_bar_height);
        let content_y = rect.y + tab_bar_height as i32;
        let content_rect =
            Rect::new(rect.x, content_y, rect.width, rect.height.saturating_sub(tab_bar_height));

        // Draw tab bar background
        context.fill_rect(tab_bar_rect, Color::rgba(245, 245, 245, 255));

        if self.tabs.is_empty() {
            // Draw empty content area
            context.fill_rect(content_rect, Color::WHITE);
            return;
        }

        // Draw each tab header
        let tab_count = self.tabs.len() as u32;
        let tab_width = rect.width / tab_count.max(1);
        let font = Font::simple("sans-serif", 12.0);

        for i in 0..self.tabs.len() {
            let tab_x = rect.x + (i as u32 * tab_width) as i32;
            let tab_rect = Rect::new(tab_x, rect.y, tab_width, tab_bar_height);
            let is_selected = i == self.selected_index;

            // Background
            let bg_color = if is_selected { Color::WHITE } else { Color::rgba(235, 235, 235, 255) };
            context.fill_rect(tab_rect, bg_color);

            // Selected tab indicator line
            if is_selected {
                let indicator_rect =
                    Rect::new(tab_x, rect.y + tab_bar_height as i32 - 3, tab_width, 3);
                context.fill_rect(indicator_rect, Color::rgba(52, 120, 246, 255));
            }

            // Draw tab title (with icon prefix if available)
            let tab = &self.tabs[i];
            let display_text = if let Some(ref icon_name) = tab.icon {
                format!("{} {}", icon_name, tab.title)
            } else {
                tab.title.clone()
            };

            let text_color = if is_selected {
                Color::rgba(52, 120, 246, 255)
            } else {
                Color::rgba(80, 80, 80, 255)
            };

            let metrics = context.measure_text(&display_text, &font);
            let text_x = tab_x + (tab_width as i32 - metrics.width as i32) / 2;
            let text_y = rect.y
                + (tab_bar_height as i32 - metrics.height as i32) / 2
                + metrics.ascent as i32;
            context.draw_text(
                Point::new(text_x.max(tab_x), text_y),
                &display_text,
                &font,
                text_color,
            );
        }

        // Draw separator line below tab bar
        let separator_rect = Rect::new(rect.x, rect.y + tab_bar_height as i32 - 1, rect.width, 1);
        context.fill_rect(separator_rect, Color::rgba(200, 200, 200, 255));

        // Draw selected tab content area (child widget rendering is delegated)
        context.fill_rect(content_rect, Color::WHITE);
    }
}

impl EventHandler for TabView {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 && !self.tabs.is_empty() {
                    // Check if click is in the tab bar area
                    let rect = self.geometry();
                    let tab_bar_height: u32 = 40;
                    if pos.y >= rect.y && pos.y < rect.y + tab_bar_height as i32 {
                        let tab_count = self.tabs.len() as u32;
                        let tab_width = rect.width / tab_count.max(1);
                        let relative_x = (pos.x - rect.x) as u32;
                        let clicked_index = (relative_x / tab_width) as usize;
                        if clicked_index < self.tabs.len() {
                            self.set_current_index(clicked_index);
                        }
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
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn make_tab_view() -> TabView {
        TabView::new(Rect::new(0, 0, 300, 400))
    }

    #[test]
    fn tab_view_default_state() {
        let tv = make_tab_view();
        assert_eq!(tv.tab_count(), 0);
        assert_eq!(tv.current_index(), 0);
        assert_eq!(tv.kind(), WidgetKind::TabView);
    }

    #[test]
    fn tab_view_add_and_select() {
        let mut tv = make_tab_view();
        tv.add_tab("Tab 1", None, None::<&str>);
        tv.add_tab("Tab 2", None, None::<&str>);
        assert_eq!(tv.tab_count(), 2);
        assert_eq!(tv.current_index(), 0);

        tv.set_current_index(1);
        assert_eq!(tv.current_index(), 1);
    }

    #[test]
    fn tab_view_signal_emits() {
        let mut tv = make_tab_view();
        tv.add_tab("First", None, None::<&str>);
        tv.add_tab("Second", None, None::<&str>);

        let captured = Arc::new(AtomicUsize::new(usize::MAX));
        tv.tab_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                captured.store(*val, Ordering::SeqCst);
            }
        });

        tv.set_current_index(1);
        assert_eq!(captured.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn tab_view_remove_tab() {
        let mut tv = make_tab_view();
        tv.add_tab("A", None, None::<&str>);
        tv.add_tab("B", None, None::<&str>);
        tv.add_tab("C", None, None::<&str>);
        tv.set_current_index(2);
        tv.remove_tab(2);
        assert_eq!(tv.tab_count(), 2);
        assert_eq!(tv.current_index(), 1);
    }

    #[test]
    fn tab_view_clear_tabs() {
        let mut tv = make_tab_view();
        tv.add_tab("X", None, None::<&str>);
        tv.add_tab("Y", None, None::<&str>);
        tv.clear_tabs();
        assert_eq!(tv.tab_count(), 0);
        assert_eq!(tv.current_index(), 0);
    }

    #[test]
    fn tab_view_mouse_click_switches_tab() {
        let mut tv = make_tab_view();
        tv.add_tab("Foo", None, None::<&str>);
        tv.add_tab("Bar", None, None::<&str>);

        // Click on second tab header (x=150..299, y=0..40)
        tv.handle_event(&Event::MousePress { pos: Point::new(160, 20), button: 1 });
        assert_eq!(tv.current_index(), 1);
    }

    #[test]
    fn tab_view_svg_output() {
        let mut tv = make_tab_view();
        tv.add_tab("One", None, None::<&str>);
        tv.add_tab("Two", None, None::<&str>);
        let svg = render_to_svg(&mut tv);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn tab_view_set_current_index_noop_when_empty() {
        let mut tv = make_tab_view();
        tv.set_current_index(5); // no tabs, should not panic
        assert_eq!(tv.current_index(), 0);
    }
}
