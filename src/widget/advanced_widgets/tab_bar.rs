//! Standalone TabBar widget — decoupled from TabWidget, draws a row/column of tabs.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{RenderContext, TextMetrics};
use crate::signal::Signal1;

use crate::widget::container_widgets::tabwidget::{TabPosition, TabShape};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};

const TAB_HEIGHT: i32 = 24;
const TAB_MIN_WIDTH: u32 = 40;
const TAB_MAX_WIDTH: u32 = 200;
const TAB_SPACING: i32 = 2;
const CLOSE_SIZE: i32 = 12;
const CLOSE_PADDING: i32 = 5;

/// A single tab in a `TabBar`.
pub struct TabBarTab {
    title: String,
    icon: Option<Image>,
    tooltip: String,
    enabled: bool,
}

impl TabBarTab {
    /// Creates a new tab with the given title.
    pub fn new(title: String) -> Self {
        Self { title, icon: None, tooltip: String::new(), enabled: true }
    }

    /// Returns the tab title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the tab title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Returns the tab icon, if any.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }

    /// Sets the tab icon.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }

    /// Returns the tab tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Sets the tab tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }

    /// Returns whether the tab is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets the enabled state of the tab.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Standalone tab bar widget that displays a row (or column) of tabs.
pub struct TabBar {
    base: BaseWidget,
    tabs: Vec<TabBarTab>,
    current_index: Option<usize>,
    hovered_index: Option<usize>,
    tab_position: TabPosition,
    tab_shape: TabShape,
    closable: bool,
    movable: bool,
    tab_min_width: u32,
    tab_max_width: u32,
    /// Emitted when the current tab index changes.
    pub current_changed: Signal1<usize>,
    /// Emitted when a tab close is requested (closable tabs only).
    pub tab_close_requested: Signal1<usize>,
    /// Emitted when a tab is moved; payload is `(old_index, new_index)`.
    pub tab_moved: Signal1<(usize, usize)>,
}

impl TabBar {
    /// Creates a new `TabBar` with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabBar, geometry, "TabBar"),
            tabs: Vec::new(),
            current_index: None,
            hovered_index: None,
            tab_position: TabPosition::North,
            tab_shape: TabShape::Rounded,
            closable: false,
            movable: false,
            tab_min_width: TAB_MIN_WIDTH,
            tab_max_width: TAB_MAX_WIDTH,
            current_changed: Signal1::new(),
            tab_close_requested: Signal1::new(),
            tab_moved: Signal1::new(),
        }
    }

    // ---------------------------------------------------------------------------
    // Tab management
    // ---------------------------------------------------------------------------

    /// Adds a tab with the given title and returns the index of the new tab.
    pub fn add_tab(&mut self, title: String) -> usize {
        self.tabs.push(TabBarTab::new(title));
        let idx = self.tabs.len().saturating_sub(1);
        if self.current_index.is_none() {
            self.current_index = Some(idx);
        }
        idx
    }

    /// Inserts a tab at the given index.
    pub fn insert_tab(&mut self, index: usize, title: String) {
        let idx = index.min(self.tabs.len());
        self.tabs.insert(idx, TabBarTab::new(title));
        // Adjust current_index if needed.
        if let Some(cur) = self.current_index {
            if cur >= idx {
                self.current_index = Some(cur + 1);
            }
        } else {
            self.current_index = Some(idx);
        }
    }

    /// Removes the tab at the given index.
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        // Adjust current_index.
        match self.current_index {
            Some(cur) if cur == index => {
                if self.tabs.is_empty() {
                    self.current_index = None;
                } else if cur >= self.tabs.len() {
                    self.current_index = Some(self.tabs.len() - 1);
                }
                // Otherwise stays the same (next tab slides into same position).
            }
            Some(cur) if cur > index => {
                self.current_index = Some(cur - 1);
            }
            // No action needed for this transition
            _ => {}
        }
    }

    /// Removes all tabs.
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.current_index = None;
        self.hovered_index = None;
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns the number of tabs.
    pub fn count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns `true` if there are no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    // ---------------------------------------------------------------------------
    // Tab text
    // ---------------------------------------------------------------------------

    /// Returns the text of the tab at the given index.
    pub fn tab_text(&self, index: usize) -> Option<&str> {
        self.tabs.get(index).map(|t| t.title.as_str())
    }

    /// Sets the text of the tab at the given index.
    pub fn set_tab_text(&mut self, index: usize, text: String) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.title = text;
        }
    }

    // ---------------------------------------------------------------------------
    // Tab enabled state
    // ---------------------------------------------------------------------------

    /// Returns whether the tab at the given index is enabled.
    pub fn tab_enabled(&self, index: usize) -> Option<bool> {
        self.tabs.get(index).map(|t| t.enabled)
    }

    /// Sets the enabled state of the tab at the given index.
    pub fn set_tab_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.enabled = enabled;
        }
    }

    // ---------------------------------------------------------------------------
    // Current index
    // ---------------------------------------------------------------------------

    /// Returns the current tab index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Sets the current tab index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.tabs.len() {
            let changed = self.current_index != Some(index);
            self.current_index = Some(index);
            if changed {
                self.current_changed.emit(index);
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Tab position
    // ---------------------------------------------------------------------------

    /// Returns the tab position.
    pub fn tab_position(&self) -> TabPosition {
        self.tab_position
    }

    /// Sets the tab position. Requests redraw if the position changes.
    pub fn set_tab_position(&mut self, position: TabPosition) {
        if self.tab_position != position {
            self.tab_position = position;
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Tab shape
    // ---------------------------------------------------------------------------

    /// Returns the tab shape.
    pub fn tab_shape(&self) -> TabShape {
        self.tab_shape
    }

    /// Sets the tab shape. Requests redraw.
    pub fn set_tab_shape(&mut self, shape: TabShape) {
        if self.tab_shape != shape {
            self.tab_shape = shape;
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Closable / Movable
    // ---------------------------------------------------------------------------

    /// Returns whether tabs are closable.
    pub fn closable(&self) -> bool {
        self.closable
    }

    /// Sets whether tabs should show a close button.
    pub fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
    }

    /// Returns whether tabs are movable.
    pub fn movable(&self) -> bool {
        self.movable
    }

    /// Sets whether tabs can be moved via drag-and-drop.
    pub fn set_movable(&mut self, movable: bool) {
        self.movable = movable;
    }

    // ---------------------------------------------------------------------------
    // Tab dimensions
    // ---------------------------------------------------------------------------

    /// Returns the minimum tab width.
    pub fn tab_min_width(&self) -> u32 {
        self.tab_min_width
    }

    /// Sets the minimum tab width.
    pub fn set_tab_min_width(&mut self, width: u32) {
        self.tab_min_width = width.max(1);
    }

    /// Returns the maximum tab width.
    pub fn tab_max_width(&self) -> u32 {
        self.tab_max_width
    }

    /// Sets the maximum tab width.
    pub fn set_tab_max_width(&mut self, width: u32) {
        self.tab_max_width = width.max(self.tab_min_width);
    }

    // ---------------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------------

    /// Returns the rect for the tab at the given index.
    fn tab_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() {
            return None;
        }
        let rect = self.base.geometry();
        let spacing = TAB_SPACING;
        match self.tab_position {
            TabPosition::North | TabPosition::South => {
                let tab_width = self.compute_tab_width(index);
                let x = rect.x + (tab_width as i32 + spacing) * index as i32;
                let y = if self.tab_position == TabPosition::North {
                    rect.y
                } else {
                    rect.y + rect.height as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, tab_width, TAB_HEIGHT as u32))
            }
            TabPosition::West | TabPosition::East => {
                let tab_height = TAB_HEIGHT as u32;
                let y = rect.y + (TAB_HEIGHT + spacing) * index as i32;
                let x = if self.tab_position == TabPosition::West {
                    rect.x
                } else {
                    rect.x + rect.width as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, TAB_HEIGHT as u32, tab_height))
            }
        }
    }

    /// Computes the width of a tab, taking into account whether a close button
    /// is rendered and clamping to the configured min/max.
    fn compute_tab_width(&self, index: usize) -> u32 {
        let text = self.tabs[index].title.as_str();
        let metrics = self.measure_text_approx(text);
        let mut w = metrics.width + 12; // horizontal padding
        if self.closable {
            w += (CLOSE_SIZE + CLOSE_PADDING) as u32;
        }
        w.clamp(self.tab_min_width, self.tab_max_width)
    }

    /// Roughly measure text width (fallback if no RenderContext handy).
    fn measure_text_approx(&self, text: &str) -> TextMetrics {
        // Approximate: each character ~8 logical pixels wide, line height = font size.
        let char_width = 8;
        let width = (text.len() as u32).saturating_mul(char_width);
        let height = 16;
        TextMetrics { width, height, ascent: 12, descent: 4 }
    }

    /// Returns the index of the tab at the given point, or None.
    fn tab_at_position(&self, pos: Point) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if let Some(r) = self.tab_rect(i) {
                if r.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Returns the close-button rect for the tab at the given index, if closable.
    fn close_rect(&self, index: usize) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        self.tab_rect(index).map(|r| {
            let cx = r.x + r.width as i32 - CLOSE_SIZE - CLOSE_PADDING;
            let cy = r.y + (r.height as i32 - CLOSE_SIZE) / 2;
            Rect::new(cx, cy, CLOSE_SIZE as u32, CLOSE_SIZE as u32)
        })
    }

    /// Draws a single tab with the given shape.
    fn draw_tab(&self, context: &mut RenderContext, index: usize, tab_rect: Rect) {
        let tab = &self.tabs[index];
        let is_current = self.current_index == Some(index);
        let is_hovered = self.hovered_index == Some(index);
        let is_enabled = tab.enabled;

        // Background color.
        let bg = if !is_enabled {
            Color::from_rgb(240, 240, 240)
        } else if is_current {
            Color::from_rgb(255, 255, 255)
        } else if is_hovered {
            Color::from_rgb(235, 235, 250)
        } else {
            Color::from_rgb(230, 230, 230)
        };

        // Border color.
        let border = if !is_enabled {
            Color::from_rgb(200, 200, 200)
        } else if is_current {
            Color::from_rgb(180, 180, 200)
        } else {
            Color::from_rgb(180, 180, 180)
        };

        // Draw tab shape.
        match self.tab_shape {
            TabShape::Rounded => {
                // For simplicity, fill the rect then draw a border.
                context.fill_rect(tab_rect, bg);
                context.draw_rect(tab_rect, border);
                // Top-left and top-right rounded corner hints (visual only via fill).
                if is_current {
                    // Overdraw bottom edge so it blends with content area.
                    let bottom_rect = Rect::new(
                        tab_rect.x,
                        tab_rect.y + tab_rect.height as i32 - 1,
                        tab_rect.width,
                        2,
                    );
                    context.fill_rect(bottom_rect, bg);
                }
            }
            TabShape::Triangular => {
                // Draw a trapezoid-ish shape: fill a rect then clip top corners.
                context.fill_rect(tab_rect, bg);
                context.draw_rect(tab_rect, border);
            }
            TabShape::Rectangular => {
                context.fill_rect(tab_rect, bg);
                context.draw_rect(tab_rect, border);
            }
        }

        // Text color.
        let text_color =
            if !is_enabled { Color::from_rgb(150, 150, 150) } else { Color::from_rgb(0, 0, 0) };

        // Draw tab title.
        let text_x = tab_rect.x + 6;
        let text_y = tab_rect.y + tab_rect.height as i32 / 2;
        context.draw_text(Point::new(text_x, text_y), &tab.title, &Font::default(), text_color);

        // Draw close button if closable.
        if let Some(close_rect) = self.close_rect(index) {
            let close_color = if !is_enabled {
                Color::from_rgb(180, 180, 180)
            } else {
                Color::from_rgb(100, 100, 100)
            };
            context.draw_line(
                Point::new(close_rect.x, close_rect.y),
                Point::new(
                    close_rect.x + close_rect.width as i32,
                    close_rect.y + close_rect.height as i32,
                ),
                close_color,
            );
            context.draw_line(
                Point::new(close_rect.x + close_rect.width as i32, close_rect.y),
                Point::new(close_rect.x, close_rect.y + close_rect.height as i32),
                close_color,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------
impl Widget for TabBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

// ---------------------------------------------------------------------------
// EventHandler trait
// ---------------------------------------------------------------------------
impl EventHandler for TabBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let prev = self.hovered_index;
                self.hovered_index = self.tab_at_position(*pos);
                if prev != self.hovered_index {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button } if *button == 1 => {
                // Check close button first.
                if self.closable {
                    for i in 0..self.tabs.len() {
                        if let Some(close_rect) = self.close_rect(i) {
                            if close_rect.contains(*pos) && self.tabs[i].enabled {
                                self.tab_close_requested.emit(i);
                                return;
                            }
                        }
                    }
                }
                // Otherwise, select tab.
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.set_current_index(index);
                    }
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                // Check close button first.
                if self.closable {
                    for i in 0..self.tabs.len() {
                        if let Some(close_rect) = self.close_rect(i) {
                            if close_rect.contains(*pos) && self.tabs[i].enabled {
                                self.tab_close_requested.emit(i);
                                return;
                            }
                        }
                    }
                }
                // Otherwise, select tab.
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.set_current_index(index);
                    }
                }
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Draw trait
// ---------------------------------------------------------------------------
impl Draw for TabBar {
    fn draw(&mut self, context: &mut RenderContext) {
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                self.draw_tab(context, i, tr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn tabbar_creation_defaults() {
        let tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.is_empty());
        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), None);
        assert!(!tb.closable());
        assert!(!tb.movable());
        assert_eq!(tb.tab_min_width(), 40);
        assert_eq!(tb.tab_max_width(), 200);
    }

    #[test]
    fn tabbar_add_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        let idx = tb.add_tab("Tab 1".to_string());
        assert_eq!(idx, 0);
        assert_eq!(tb.count(), 1);
        assert_eq!(tb.current_index(), Some(0));
        assert_eq!(tb.tab_text(0), Some("Tab 1"));
    }

    #[test]
    fn tabbar_multiple_tabs() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("First".to_string());
        tb.add_tab("Second".to_string());
        tb.add_tab("Third".to_string());
        assert_eq!(tb.count(), 3);
        assert_eq!(tb.tab_text(0), Some("First"));
        assert_eq!(tb.tab_text(1), Some("Second"));
        assert_eq!(tb.tab_text(2), Some("Third"));
    }

    #[test]
    fn tabbar_insert_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("C".to_string());
        tb.insert_tab(1, "B".to_string());
        assert_eq!(tb.count(), 3);
        assert_eq!(tb.tab_text(1), Some("B"));
    }

    #[test]
    fn tabbar_remove_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.add_tab("C".to_string());
        tb.remove_tab(1);
        assert_eq!(tb.count(), 2);
        assert_eq!(tb.tab_text(1), Some("C"));
    }

    #[test]
    fn tabbar_clear() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.clear();
        assert!(tb.is_empty());
        assert_eq!(tb.current_index(), None);
    }

    #[test]
    fn tabbar_set_current_index() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.set_current_index(1);
        assert_eq!(tb.current_index(), Some(1));
        tb.set_current_index(0);
        assert_eq!(tb.current_index(), Some(0));
    }

    #[test]
    fn tabbar_set_tab_text() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("Old".to_string());
        tb.set_tab_text(0, "New".to_string());
        assert_eq!(tb.tab_text(0), Some("New"));
    }

    #[test]
    fn tabbar_tab_enabled() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("Tab".to_string());
        assert_eq!(tb.tab_enabled(0), Some(true));
        tb.set_tab_enabled(0, false);
        assert_eq!(tb.tab_enabled(0), Some(false));
    }

    #[test]
    fn tabbar_closable_movable() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(!tb.closable());
        tb.set_closable(true);
        assert!(tb.closable());
        assert!(!tb.movable());
        tb.set_movable(true);
        assert!(tb.movable());
    }

    #[test]
    fn tabbar_tab_position_shape() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert_eq!(tb.tab_position(), TabPosition::North);
        tb.set_tab_position(TabPosition::South);
        assert_eq!(tb.tab_position(), TabPosition::South);
        assert_eq!(tb.tab_shape(), TabShape::Rounded);
        tb.set_tab_shape(TabShape::Rectangular);
        assert_eq!(tb.tab_shape(), TabShape::Rectangular);
    }

    #[test]
    fn tabbar_min_max_width() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_tab_min_width(50);
        assert_eq!(tb.tab_min_width(), 50);
        tb.set_tab_max_width(100);
        assert_eq!(tb.tab_max_width(), 100);
    }

    #[test]
    fn tabbar_signal_accessors() {
        let tb = TabBar::new(Rect::new(0, 0, 400, 24));
        let _ = &tb.current_changed;
        let _ = &tb.tab_close_requested;
        let _ = &tb.tab_moved;
    }

    #[test]
    fn tabbar_geometry_delegation() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_geometry(Rect::new(10, 10, 500, 30));
        assert_eq!(tb.geometry(), Rect::new(10, 10, 500, 30));
    }

    #[test]
    fn tabbar_visibility() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.is_visible());
        tb.hide();
        assert!(!tb.is_visible());
        tb.show();
        assert!(tb.is_visible());
    }

    #[test]
    fn tabbar_id_kind() {
        let a = TabBar::new(Rect::new(0, 0, 400, 24));
        let b = TabBar::new(Rect::new(0, 0, 400, 24));
        assert_ne!(a.id(), b.id());
        assert_eq!(a.kind(), WidgetKind::TabBar);
    }

    #[test]
    fn tabbar_draw_produces_svg() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("One".to_string());
        tb.add_tab("Two".to_string());
        tb.set_current_index(0);
        let svg = crate::widget::svg::render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 100);
    }

    #[test]
    fn tabbar_mouse_click_selects_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("First".to_string());
        tb.add_tab("Second".to_string());
        // Click on the first tab area
        tb.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1 });
        assert_eq!(tb.current_index(), Some(0));
        // Hover sets hovered_index
        tb.handle_event(&Event::MouseMove { pos: Point::new(5, 5) });
    }
}
