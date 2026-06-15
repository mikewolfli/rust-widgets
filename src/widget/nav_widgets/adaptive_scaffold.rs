//! AdaptiveScaffold — a cross-platform adaptive scaffold widget.
//!
//! The AdaptiveScaffold combines an AppBar (top bar), a main content area,
//! and an optional BottomNavigationBar into a single cohesive layout.
//! It is designed to work on both iOS (Cupertino) and Android (Material)
//! platforms by using the same underlying AppBar and BottomNavigationBar
//! primitives with platform-appropriate defaults.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::nav_widgets::app_bar::AppBar;
use crate::widget::nav_widgets::bottom_navigation_bar::NavItem;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// AdaptiveScaffold — a cross-platform adaptive scaffold combining AppBar +
/// content area + optional BottomNavigationBar.
///
/// The scaffold manages layout internally: the AppBar occupies the top region,
/// the content area fills the space between the AppBar and the bottom navigation
/// bar, and the BottomNavigationBar (when enabled) sits at the bottom.
///
/// # Example
///
/// ```text
/// let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
/// scaffold.add_nav_item("★", "Favorites");
/// scaffold.add_nav_item("✉", "Messages");
/// scaffold.set_show_bottom_nav(true);
/// ```
pub struct AdaptiveScaffold {
    base: BaseWidget,
    /// The title text displayed in the AppBar.
    title: String,
    /// Internal AppBar widget.
    app_bar: AppBar,
    /// Optional body widget to render in the content area.
    body_widget: Option<Box<dyn Widget>>,
    /// Whether to show the bottom navigation bar.
    show_bottom_nav: bool,
    /// Navigation items for the bottom bar.
    nav_items: Vec<NavItem>,
    /// Currently selected bottom nav index.
    selected_nav_index: usize,
    /// Emitted when the bottom navigation selection changes.
    pub nav_selected: Signal1<usize>,
}

impl AdaptiveScaffold {
    /// Creates a new AdaptiveScaffold with the given title and geometry.
    ///
    /// The scaffold is created with the AppBar visible and bottom navigation
    /// hidden by default. Use [`set_show_bottom_nav`](Self::set_show_bottom_nav)
    /// to enable the bottom bar and [`add_nav_item`](Self::add_nav_item) to
    /// populate it.
    pub fn new(title: &str, geometry: Rect) -> Self {
        let app_bar_height: u32 = 56;
        let app_bar =
            AppBar::new(title, Rect::new(geometry.x, geometry.y, geometry.width, app_bar_height));

        Self {
            base: BaseWidget::new(WidgetKind::AdaptiveScaffold, geometry, "AdaptiveScaffold"),
            title: title.to_string(),
            app_bar,
            body_widget: None,
            show_bottom_nav: false,
            nav_items: Vec::new(),
            selected_nav_index: 0,
            nav_selected: Signal1::new(),
        }
    }

    /// Sets the title displayed in the AppBar.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.app_bar.set_title(title);
    }

    /// Returns the current title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets whether the AppBar back button is shown.
    pub fn set_show_back(&mut self, show_back: bool) {
        self.app_bar.set_show_back(show_back);
    }

    /// Returns whether the AppBar back button is shown.
    pub fn show_back(&self) -> bool {
        self.app_bar.show_back()
    }

    /// Sets the action text displayed on the right side of the AppBar.
    pub fn set_action_text(&mut self, text: &str) {
        self.app_bar.set_action_text(text);
    }

    /// Returns the current AppBar action text.
    pub fn action_text(&self) -> &str {
        self.app_bar.action_text()
    }

    /// Sets the optional body widget displayed in the content area.
    pub fn set_body_widget(&mut self, widget: Option<Box<dyn Widget>>) {
        self.body_widget = widget;
        self.base.request_redraw();
    }

    /// Returns a reference to the optional body widget.
    pub fn body_widget(&self) -> Option<&dyn Widget> {
        self.body_widget.as_deref()
    }

    /// Returns a mutable reference to the optional body widget.
    pub fn body_widget_mut(&mut self) -> Option<&mut Box<dyn Widget>> {
        self.body_widget.as_mut()
    }

    /// Sets whether the bottom navigation bar is visible.
    pub fn set_show_bottom_nav(&mut self, show: bool) {
        self.show_bottom_nav = show;
        self.base.request_redraw();
    }

    /// Returns whether the bottom navigation bar is visible.
    pub fn show_bottom_nav(&self) -> bool {
        self.show_bottom_nav
    }

    /// Adds a bottom navigation item with the given icon and label.
    pub fn add_nav_item(&mut self, icon: &str, label: &str) {
        self.nav_items.push(NavItem { icon: icon.to_string(), label: label.to_string() });
        self.base.request_redraw();
    }

    /// Removes all bottom navigation items.
    pub fn clear_nav_items(&mut self) {
        self.nav_items.clear();
        self.selected_nav_index = 0;
        self.base.request_redraw();
    }

    /// Returns the number of bottom navigation items.
    pub fn nav_item_count(&self) -> usize {
        self.nav_items.len()
    }

    /// Sets the selected bottom navigation index.
    pub fn set_selected_nav_index(&mut self, index: usize) {
        let clamped =
            if self.nav_items.is_empty() { 0 } else { index.min(self.nav_items.len() - 1) };
        if self.selected_nav_index != clamped {
            self.selected_nav_index = clamped;
            self.nav_selected.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the current selected bottom navigation index.
    pub fn selected_nav_index(&self) -> usize {
        self.selected_nav_index
    }

    /// Returns the content area rect (between AppBar and bottom nav).
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let top_offset: i32 = 56; // AppBar height
        let bottom_offset: i32 = if self.show_bottom_nav { 56 } else { 0 };
        Rect::new(
            rect.x,
            rect.y + top_offset,
            rect.width,
            (rect.height as i32 - top_offset - bottom_offset).max(0) as u32,
        )
    }

    /// Returns the bottom nav bar rect.
    fn bottom_nav_rect(&self) -> Option<Rect> {
        if !self.show_bottom_nav || self.nav_items.is_empty() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(rect.x, rect.y + rect.height as i32 - 56, rect.width, 56))
    }
}

impl Widget for AdaptiveScaffold {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for AdaptiveScaffold {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // ── Background ──
        context.fill_rect(rect, Color::rgba(245, 246, 248, 255));

        // ── AppBar ──
        self.app_bar.draw(context);

        // ── Body content area (body widget drawing is delegated to the
        //     widget's own render path since Box<dyn Widget> doesn't expose Draw)
        let content_rect = self.content_rect();
        context.fill_rect(content_rect, Color::rgba(245, 246, 248, 255));

        // ── Bottom navigation bar ──
        if self.show_bottom_nav && !self.nav_items.is_empty() {
            let nav_rect = self.bottom_nav_rect().unwrap();
            self.draw_bottom_nav(context, nav_rect);
        }
    }
}

impl AdaptiveScaffold {
    /// Draws the bottom navigation bar manually within the scaffold.
    fn draw_bottom_nav(&mut self, context: &mut RenderContext, nav_rect: Rect) {
        let item_count = self.nav_items.len();
        if item_count == 0 {
            return;
        }

        let tab_width = nav_rect.width / item_count as u32;

        // Background
        context.fill_rect(nav_rect, Color::WHITE);

        // Top border
        context.draw_line(
            Point::new(nav_rect.x, nav_rect.y),
            Point::new(nav_rect.x + nav_rect.width as i32, nav_rect.y),
            Color::DIVIDER,
        );

        let icon_font_size = (nav_rect.height as f32 * 0.32).clamp(14.0, 28.0);
        let label_font_size = (nav_rect.height as f32 * 0.18).clamp(9.0, 14.0);

        let icon_font = crate::core::Font::new("sans-serif", icon_font_size, false, false);
        let label_font = crate::core::Font::new("sans-serif", label_font_size, false, false);

        for (i, item) in self.nav_items.iter().enumerate() {
            let tab_x = nav_rect.x + (i as u32 * tab_width) as i32;
            let tab_rect = Rect::new(tab_x, nav_rect.y, tab_width, nav_rect.height);

            let is_selected = i == self.selected_nav_index;
            let icon_color = if is_selected { Color::PRIMARY } else { Color::MEDIUM_GRAY };
            let label_color = icon_color;

            let icon_metrics = context.measure_text(&item.icon, &icon_font);
            let label_metrics = context.measure_text(&item.label, &label_font);

            let total_content_height = icon_metrics.height + label_metrics.height + 4;
            let content_y = tab_rect.y + (tab_rect.height as i32 - total_content_height as i32) / 2;

            // Icon
            let icon_x = tab_rect.x + (tab_rect.width as i32 - icon_metrics.width as i32) / 2;
            let icon_y = content_y + icon_metrics.ascent as i32;
            context.draw_text(Point::new(icon_x, icon_y), &item.icon, &icon_font, icon_color);

            // Label
            let label_x = tab_rect.x + (tab_rect.width as i32 - label_metrics.width as i32) / 2;
            let label_y = content_y + icon_metrics.height as i32 + 4 + label_metrics.ascent as i32;
            context.draw_text(Point::new(label_x, label_y), &item.label, &label_font, label_color);

            // Selected indicator line
            if is_selected {
                let indicator_height = 3u32;
                let indicator_width = (tab_width * 3 / 5).max(20).min(tab_width);
                let indicator_x = tab_rect.x + (tab_rect.width as i32 - indicator_width as i32) / 2;
                let indicator_rect =
                    Rect::new(indicator_x, nav_rect.y, indicator_width, indicator_height);
                context.fill_rounded_rect(indicator_rect, 1, Color::PRIMARY);
            }
        }
    }
}

impl EventHandler for AdaptiveScaffold {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                let rect = self.geometry();
                let app_bar_height: i32 = 56;

                // AppBar zone (top)
                if pos.y >= rect.y && pos.y < rect.y + app_bar_height {
                    self.app_bar.handle_event(event);
                    // Bridge AppBar signals
                    return;
                }

                // Bottom nav zone
                if self.show_bottom_nav && !self.nav_items.is_empty() {
                    let nav_top = rect.y + rect.height as i32 - 56;
                    if pos.y >= nav_top && pos.y <= rect.y + rect.height as i32 {
                        // Hit-test bottom nav items
                        let item_count = self.nav_items.len();
                        let tab_width = rect.width / item_count as u32;
                        let relative_x = pos.x - rect.x;
                        if relative_x >= 0 {
                            let index = (relative_x as u32 / tab_width) as usize;
                            if index < item_count {
                                self.set_selected_nav_index(index);
                                return;
                            }
                        }
                    }
                }

                // Body zone — delegate to body widget if present
                let content_rect = self.content_rect();
                if let Some(body) = self.body_widget.as_mut() {
                    if content_rect.contains_point(*pos) {
                        body.handle_event(event);
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
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn make_scaffold() -> AdaptiveScaffold {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        scaffold.add_nav_item("★", "Favorites");
        scaffold.add_nav_item("✉", "Messages");
        scaffold.add_nav_item("⌂", "Home");
        scaffold.set_show_bottom_nav(true);
        scaffold
    }

    #[test]
    fn adaptive_scaffold_creation() {
        let scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.kind(), WidgetKind::AdaptiveScaffold);
        assert_eq!(scaffold.title(), "Home");
        assert!(!scaffold.show_back());
        assert!(!scaffold.show_bottom_nav());
        assert_eq!(scaffold.geometry(), Rect::new(0, 0, 375, 812));
    }

    #[test]
    fn adaptive_scaffold_title_accessors() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.title(), "Home");

        scaffold.set_title("Settings");
        assert_eq!(scaffold.title(), "Settings");
    }

    #[test]
    fn adaptive_scaffold_back_accessors() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert!(!scaffold.show_back());

        scaffold.set_show_back(true);
        assert!(scaffold.show_back());
    }

    #[test]
    fn adaptive_scaffold_action_text_accessors() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.action_text(), "");

        scaffold.set_action_text("Save");
        assert_eq!(scaffold.action_text(), "Save");
    }

    #[test]
    fn adaptive_scaffold_bottom_nav_toggle() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert!(!scaffold.show_bottom_nav());

        scaffold.set_show_bottom_nav(true);
        assert!(scaffold.show_bottom_nav());

        scaffold.set_show_bottom_nav(false);
        assert!(!scaffold.show_bottom_nav());
    }

    #[test]
    fn adaptive_scaffold_add_nav_items() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        assert_eq!(scaffold.nav_item_count(), 0);

        scaffold.add_nav_item("★", "Favorites");
        assert_eq!(scaffold.nav_item_count(), 1);

        scaffold.add_nav_item("✉", "Messages");
        assert_eq!(scaffold.nav_item_count(), 2);
    }

    #[test]
    fn adaptive_scaffold_clear_nav_items() {
        let mut scaffold = make_scaffold();
        assert_eq!(scaffold.nav_item_count(), 3);

        scaffold.clear_nav_items();
        assert_eq!(scaffold.nav_item_count(), 0);
    }

    #[test]
    fn adaptive_scaffold_select_nav_index() {
        let mut scaffold = make_scaffold();
        assert_eq!(scaffold.selected_nav_index(), 0);

        scaffold.set_selected_nav_index(2);
        assert_eq!(scaffold.selected_nav_index(), 2);

        scaffold.set_selected_nav_index(0);
        assert_eq!(scaffold.selected_nav_index(), 0);
    }

    #[test]
    fn adaptive_scaffold_nav_selected_signal_emits() {
        let mut scaffold = make_scaffold();
        let captured = Arc::new(AtomicUsize::new(usize::MAX));
        let c = captured.clone();
        scaffold.nav_selected.connect(move |val: Arc<usize>| {
            c.store(*val, Ordering::SeqCst);
        });

        scaffold.set_selected_nav_index(1);
        assert_eq!(captured.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn adaptive_scaffold_mouse_click_nav() {
        let mut scaffold = make_scaffold();
        // Bottom nav is at y 812-56 = 756 to 812
        // Tab width = 375/3 = 125 per tab
        // Click on tab 2 (x in [250..374])
        scaffold.handle_event(&Event::MousePress { pos: Point::new(300, 780), button: 1 });
        assert_eq!(scaffold.selected_nav_index(), 2);
    }

    #[test]
    fn adaptive_scaffold_svg_output() {
        let mut scaffold = make_scaffold();
        let svg = render_to_svg(&mut scaffold);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn adaptive_scaffold_svg_without_bottom_nav() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        let svg = render_to_svg(&mut scaffold);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn adaptive_scaffold_content_rect() {
        let mut scaffold = AdaptiveScaffold::new("Home", Rect::new(0, 0, 375, 812));
        // Without bottom nav: content = y 56, h 812-56 = 756
        let content = scaffold.content_rect();
        assert_eq!(content, Rect::new(0, 56, 375, 756));

        scaffold.set_show_bottom_nav(true);
        scaffold.add_nav_item("★", "Test");
        // With bottom nav: content = y 56, h 812-56-56 = 700
        let content = scaffold.content_rect();
        assert_eq!(content, Rect::new(0, 56, 375, 700));
    }
}
