//! Tab widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// Tab widget.
pub struct TabWidget {
    base: BaseWidget,
    tabs: Vec<Tab>,
    current_index: usize,
    tab_position: TabPosition,
    tab_shape: TabShape,
    closable: bool,
    movable: bool,
    pub current_changed: Signal1<usize>,
    pub tab_close_requested: Signal1<usize>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
/// Tab information.
pub struct Tab {
    title: String,
    icon: Option<Image>,
    tooltip: String,
    enabled: bool,
    widget: Option<ObjectId>,
}
/// Tab position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabPosition {
    /// Tabs at the top
    #[default]
    North,
    /// Tabs at the bottom
    South,
    /// Tabs at the left
    West,
    /// Tabs at the right
    East,
}
/// Tab shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabShape {
    /// Rounded tabs
    #[default]
    Rounded,
    /// Triangular tabs
    ///
    /// Note: Triangular tabs currently render as rectangular.
    #[doc(hidden)]
    Triangular,
    /// Rectangular tabs
    Rectangular,
}
impl Tab {
    /// Creates a new tab.
    pub fn new(title: String) -> Self {
        Self { title, icon: None, tooltip: String::new(), enabled: true, widget: None }
    }
    /// Returns title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
    /// Returns icon.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    /// Sets icon.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }
    /// Returns tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }
    /// Sets tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    /// Returns whether tab is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Sets enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
    }
}
impl TabWidget {
    /// Creates a tab widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabWidget, geometry, "TabWidget"),
            tabs: Vec::new(),
            current_index: 0,
            tab_position: TabPosition::North,
            tab_shape: TabShape::Rounded,
            closable: false,
            movable: false,
            current_changed: Signal1::new(),
            tab_close_requested: Signal1::new(),
            registry: None,
        }
    }
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Adds a tab.
    pub fn add_tab(&mut self, title: String, widget: Option<ObjectId>) -> usize {
        let mut tab = Tab::new(title);
        tab.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.tabs.push(tab);
        self.tabs.len().saturating_sub(1)
    }
    /// Inserts a tab at position.
    pub fn insert_tab(&mut self, index: usize, title: String, widget: Option<ObjectId>) {
        let mut tab = Tab::new(title);
        tab.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.tabs.insert(index, tab);
        if self.current_index >= index {
            self.current_index += 1;
        }
    }
    /// Removes a tab.
    pub fn remove_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            if let Some(widget_id) = self.tabs[index].widget {
                self.base.remove_child(widget_id);
            }
            self.tabs.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.tabs.is_empty() {
                self.current_index = 0;
            }
        }
    }
    /// Returns number of tabs.
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
    /// Returns current tab index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current tab index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.tabs.len() && self.current_index != index {
            self.current_index = index;
            self.current_changed.emit(index);
        }
    }
    /// Returns current tab widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.tabs.get(self.current_index).and_then(|tab| tab.widget)
    }
    /// Returns tab at index.
    pub fn tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }
    /// Returns mutable tab at index.
    pub fn tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }
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
    /// Returns tab position.
    pub fn tab_position(&self) -> TabPosition {
        self.tab_position
    }
    /// Sets tab position.
    pub fn set_tab_position(&mut self, position: TabPosition) {
        self.tab_position = position;
    }
    /// Returns tab shape.
    pub fn tab_shape(&self) -> TabShape {
        self.tab_shape
    }
    /// Sets tab shape.
    pub fn set_tab_shape(&mut self, shape: TabShape) {
        self.tab_shape = shape;
    }
    /// Returns whether tabs are closable.
    pub fn closable(&self) -> bool {
        self.closable
    }
    /// Sets closable state.
    pub fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
    }
    /// Returns whether tabs are movable.
    pub fn movable(&self) -> bool {
        self.movable
    }
    /// Sets movable state.
    pub fn set_movable(&mut self, movable: bool) {
        self.movable = movable;
    }
    /// Returns tab rectangle at index.
    fn tab_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() {
            return None;
        }
        let rect = self.geometry();
        let tab_height = 24;
        let tab_width = 100;
        let spacing = 2;
        match self.tab_position {
            TabPosition::North => {
                let x = rect.x + (tab_width + spacing) as i32 * index as i32;
                Some(Rect::new(x, rect.y, tab_width, tab_height as u32))
            }
            TabPosition::South => {
                let x = rect.x + (tab_width + spacing) as i32 * index as i32;
                Some(Rect::new(
                    x,
                    rect.y + rect.height as i32 - tab_height,
                    tab_width,
                    tab_height as u32,
                ))
            }
            TabPosition::West => {
                let y = rect.y + (tab_height + spacing as i32) * index as i32;
                Some(Rect::new(rect.x, y, tab_width, tab_height as u32))
            }
            TabPosition::East => {
                let y = rect.y + (tab_height + spacing as i32) * index as i32;
                Some(Rect::new(
                    rect.x + rect.width as i32 - tab_width as i32,
                    y,
                    tab_width,
                    tab_height as u32,
                ))
            }
        }
    }
    /// Returns content rectangle.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let tab_height = 24;
        match self.tab_position {
            TabPosition::North => {
                Rect::new(rect.x, rect.y + tab_height, rect.width, rect.height - tab_height as u32)
            }
            TabPosition::South => {
                Rect::new(rect.x, rect.y, rect.width, rect.height - tab_height as u32)
            }
            TabPosition::West => {
                Rect::new(rect.x + tab_height, rect.y, rect.width - tab_height as u32, rect.height)
            }
            TabPosition::East => {
                Rect::new(rect.x, rect.y, rect.width - tab_height as u32, rect.height)
            }
        }
    }
    /// Returns index of tab at position.
    fn tab_at_position(&self, pos: Point) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if let Some(tab_rect) = self.tab_rect(i) {
                if tab_rect.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }
}
// Implement Widget trait
impl Widget for TabWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for TabWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button } = event {
            if *button == 1 {
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        // Check if the click is on the close button area
                        if self.closable {
                            let close_size = 12;
                            if let Some(tab_rect) = self.tab_rect(index) {
                                let close_x = tab_rect.x + tab_rect.width as i32 - close_size - 5;
                                let close_y =
                                    tab_rect.y + (tab_rect.height as i32 - close_size) / 2;
                                let close_rect = Rect::new(
                                    close_x,
                                    close_y,
                                    close_size as u32,
                                    close_size as u32,
                                );
                                if close_rect.contains(*pos) {
                                    self.tab_close_requested.emit(index);
                                    return;
                                }
                            }
                        }
                        self.set_current_index(index);
                    }
                }
            }
        }
        // Forward events to current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for TabWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let _rect = self.geometry();
        let content_rect = self.content_rect();
        // Draw content background
        context.fill_rect(content_rect, Color::rgb(255, 255, 255));
        // Draw content border
        context.draw_rect(content_rect, Color::rgb(200, 200, 200));
        // Draw tabs
        for i in 0..self.tabs.len() {
            if let Some(tab_rect) = self.tab_rect(i) {
                let tab = &self.tabs[i];
                let is_current = i == self.current_index;
                let is_enabled = tab.enabled;
                // Draw tab background
                let bg_color = if !is_enabled {
                    Color::rgb(240, 240, 240)
                } else if is_current {
                    Color::rgb(255, 255, 255)
                } else {
                    Color::rgb(230, 230, 230)
                };
                match self.tab_shape {
                    TabShape::Rounded => {
                        let radius = 4;
                        context.fill_rounded_rect(tab_rect, radius, bg_color);
                        let border_color = if !is_enabled || is_current {
                            Color::rgb(200, 200, 200)
                        } else {
                            Color::rgb(180, 180, 180)
                        };
                        context.draw_rounded_rect_stroke(tab_rect, radius, border_color, 1);
                    }
                    _ => {
                        context.fill_rect(tab_rect, bg_color);
                        let border_color = if !is_enabled || is_current {
                            Color::rgb(200, 200, 200)
                        } else {
                            Color::rgb(180, 180, 180)
                        };
                        context.draw_rect(tab_rect, border_color);
                    }
                };
                // Draw tab text
                let text_color = if !is_enabled {
                    Color::rgb(150, 150, 150)
                } else {
                    Color::rgb(0, 0, 0)
                };
                context.draw_text(
                    Point::new(
                        tab_rect.x + tab_rect.width as i32 / 2,
                        tab_rect.y + tab_rect.height as i32 / 2,
                    ),
                    &tab.title,
                    &Font::default(),
                    text_color,
                    HorizontalAlignment::Left,
                );
                // Draw close button if closable
                if self.closable {
                    let close_size = 12;
                    let close_x = tab_rect.x + tab_rect.width as i32 - close_size - 5;
                    let close_y = tab_rect.y + (tab_rect.height as i32 - close_size) / 2;
                    context.draw_line(
                        Point::new(close_x, close_y),
                        Point::new(close_x + close_size, close_y + close_size),
                        Color::rgb(100, 100, 100),
                    );
                    context.draw_line(
                        Point::new(close_x + close_size, close_y),
                        Point::new(close_x, close_y + close_size),
                        Color::rgb(100, 100, 100),
                    );
                }
            }
        }
        // Draw current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect, Size};
    use crate::widget::svg::{render_to_svg, render_widget_to_svg};
    use std::sync::Arc;

    /// Helper to create unique test ObjectIds.
    fn wid1() -> ObjectId {
        1001
    }
    fn wid2() -> ObjectId {
        1002
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────────

    #[test]
    fn tabwidget_creation_defaults() {
        let tw = TabWidget::new(Rect::new(10, 20, 400, 300));
        assert_eq!(tw.kind(), WidgetKind::TabWidget);
        assert_eq!(tw.geometry(), Rect::new(10, 20, 400, 300));
        assert_eq!(tw.count(), 0);
        assert_eq!(tw.current_index(), 0);
        assert!(tw.current_widget().is_none());
        assert_eq!(tw.tab_position(), TabPosition::North);
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
        assert!(!tw.closable());
        assert!(!tw.movable());
        assert!(tw.is_visible());
        assert!(tw.is_enabled());
        assert!(tw.children().is_empty());
        assert!(tw.registry().is_none());
    }

    // ── 2. Adding tabs ────────────────────────────────────────────────────────

    #[test]
    fn tabwidget_add_tab_returns_index_and_increments_count() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        let idx0 = tw.add_tab("Tab A".to_string(), None);
        assert_eq!(idx0, 0);
        assert_eq!(tw.count(), 1);

        let idx1 = tw.add_tab("Tab B".to_string(), Some(wid1()));
        assert_eq!(idx1, 1);
        assert_eq!(tw.count(), 2);

        let idx2 = tw.add_tab("Tab C".to_string(), Some(wid2()));
        assert_eq!(idx2, 2);
        assert_eq!(tw.count(), 3);
    }

    #[test]
    fn tabwidget_add_tab_with_widget_updates_children() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("With Widget".to_string(), Some(wid1()));
        let children = tw.children();
        assert_eq!(children.len(), 1);
        assert!(children.contains(&wid1()));
    }

    #[test]
    fn tabwidget_add_tab_without_widget_does_not_add_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("No Widget".to_string(), None);
        assert!(tw.children().is_empty());
    }

    // ── 3. Inserting tabs at specific index ───────────────────────────────────

    #[test]
    fn tabwidget_insert_tab_at_front_shifts_indices() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.insert_tab(0, "Inserted".to_string(), None);
        assert_eq!(tw.count(), 3);
        assert_eq!(tw.tab_text(0), Some("Inserted"));
        assert_eq!(tw.tab_text(1), Some("A"));
        assert_eq!(tw.tab_text(2), Some("B"));
    }

    #[test]
    fn tabwidget_insert_tab_at_end_acts_like_add() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.insert_tab(1, "B".to_string(), None);
        assert_eq!(tw.count(), 2);
        assert_eq!(tw.tab_text(1), Some("B"));
    }

    #[test]
    fn tabwidget_insert_tab_shifts_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.set_current_index(1);
        tw.insert_tab(0, "X".to_string(), None);
        // current_index was 1, now should be 2 because a tab was inserted before it
        assert_eq!(tw.current_index(), 2);
    }

    #[test]
    fn tabwidget_insert_tab_with_widget_adds_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.insert_tab(0, "X".to_string(), Some(wid1()));
        let children = tw.children();
        assert_eq!(children.len(), 1);
        assert!(children.contains(&wid1()));
    }

    // ── 4. Removing tabs ─────────────────────────────────────────────────────

    #[test]
    fn tabwidget_remove_tab_reduces_count() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        assert_eq!(tw.count(), 3);

        tw.remove_tab(1);
        assert_eq!(tw.count(), 2);
        assert_eq!(tw.tab_text(0), Some("A"));
        assert_eq!(tw.tab_text(1), Some("C"));
    }

    #[test]
    fn tabwidget_remove_tab_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.remove_tab(5); // out of bounds
        assert_eq!(tw.count(), 1);
    }

    #[test]
    fn tabwidget_remove_last_tab_resets_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 0);
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_remove_tab_adjusts_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        tw.set_current_index(2);
        tw.remove_tab(2);
        assert_eq!(tw.current_index(), 1);
    }

    #[test]
    fn tabwidget_remove_tab_with_widget_removes_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), Some(wid1()));
        tw.add_tab("B".to_string(), Some(wid2()));
        assert_eq!(tw.children().len(), 2);
        tw.remove_tab(0);
        assert_eq!(tw.children().len(), 1);
        assert!(!tw.children().contains(&wid1()));
    }

    // ── 5. Getting / setting current index ────────────────────────────────────

    #[test]
    fn tabwidget_set_current_index_normal() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), Some(wid1()));
        tw.add_tab("B".to_string(), Some(wid2()));
        tw.set_current_index(1);
        assert_eq!(tw.current_index(), 1);
        assert_eq!(tw.current_widget(), Some(wid2()));
    }

    #[test]
    fn tabwidget_set_current_index_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.set_current_index(10);
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_set_current_index_same_value_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.set_current_index(0); // already 0
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_current_widget_on_empty_returns_none() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.current_widget(), None);
    }

    #[test]
    fn tabwidget_set_current_index_round_trip() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        tw.set_current_index(2);
        assert_eq!(tw.current_index(), 2);
        tw.set_current_index(0);
        assert_eq!(tw.current_index(), 0);
        tw.set_current_index(1);
        assert_eq!(tw.current_index(), 1);
    }

    // ── 6. Tab count after operations ─────────────────────────────────────────

    #[test]
    fn tabwidget_count_after_mixed_operations() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.count(), 0);
        tw.add_tab("A".to_string(), None);
        assert_eq!(tw.count(), 1);
        tw.add_tab("B".to_string(), None);
        assert_eq!(tw.count(), 2);
        tw.insert_tab(1, "C".to_string(), None);
        assert_eq!(tw.count(), 3);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 2);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 1);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 0);
    }

    // ── 7. Setting tab text and tooltip ───────────────────────────────────────

    #[test]
    fn tabwidget_set_tab_text_updates_and_retrieves() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Original".to_string(), None);
        assert_eq!(tw.tab_text(0), Some("Original"));
        tw.set_tab_text(0, "Updated".to_string());
        assert_eq!(tw.tab_text(0), Some("Updated"));
    }

    #[test]
    fn tabwidget_set_tab_text_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_text(0, "Nope".to_string());
        // No panic, no change
        assert_eq!(tw.tab_text(0), None);
    }

    #[test]
    fn tabwidget_tab_text_returns_none_for_invalid_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Only".to_string(), None);
        assert_eq!(tw.tab_text(1), None);
        assert_eq!(tw.tab_text(usize::MAX), None);
    }

    #[test]
    fn tabwidget_tab_tooltip_set_and_get() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        let tab = tw.tab_mut(0).unwrap();
        assert_eq!(tab.tooltip(), "");
        tab.set_tooltip("Helpful hint".to_string());
        assert_eq!(tab.tooltip(), "Helpful hint");
        let tab_ref = tw.tab(0).unwrap();
        assert_eq!(tab_ref.tooltip(), "Helpful hint");
    }

    // ── 8. Enabling / disabling tabs ──────────────────────────────────────────

    #[test]
    fn tabwidget_tab_enabled_default() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        assert!(tw.tab(0).unwrap().is_enabled());
    }

    #[test]
    fn tabwidget_disable_tab_via_tab_mut() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        let tab = tw.tab_mut(0).unwrap();
        tab.set_enabled(false);
        assert!(!tw.tab(0).unwrap().is_enabled());
        // Re-enable
        let tab = tw.tab_mut(0).unwrap();
        tab.set_enabled(true);
        assert!(tw.tab(0).unwrap().is_enabled());
    }

    #[test]
    fn tabwidget_tab_title_via_tab_api() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Hello".to_string(), None);
        assert_eq!(tw.tab(0).unwrap().title(), "Hello");
        tw.tab_mut(0).unwrap().set_title("World".to_string());
        assert_eq!(tw.tab(0).unwrap().title(), "World");
    }

    // ── 9. Tab position and shape configuration ───────────────────────────────

    #[test]
    fn tabwidget_tab_position_default_is_north() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.tab_position(), TabPosition::North);
    }

    #[test]
    fn tabwidget_set_tab_position_cycle() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_position(TabPosition::South);
        assert_eq!(tw.tab_position(), TabPosition::South);
        tw.set_tab_position(TabPosition::West);
        assert_eq!(tw.tab_position(), TabPosition::West);
        tw.set_tab_position(TabPosition::East);
        assert_eq!(tw.tab_position(), TabPosition::East);
        tw.set_tab_position(TabPosition::North);
        assert_eq!(tw.tab_position(), TabPosition::North);
    }

    #[test]
    fn tabwidget_tab_shape_default_is_rounded() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
    }

    #[test]
    fn tabwidget_set_tab_shape() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_shape(TabShape::Triangular);
        assert_eq!(tw.tab_shape(), TabShape::Triangular);
        tw.set_tab_shape(TabShape::Rectangular);
        assert_eq!(tw.tab_shape(), TabShape::Rectangular);
        tw.set_tab_shape(TabShape::Rounded);
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
    }

    #[test]
    fn tabwidget_closable_movable_default_false() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(!tw.closable());
        assert!(!tw.movable());
        tw.set_closable(true);
        tw.set_movable(true);
        assert!(tw.closable());
        assert!(tw.movable());
        tw.set_closable(false);
        tw.set_movable(false);
        assert!(!tw.closable());
        assert!(!tw.movable());
    }

    // ── 10. Min / max size via Widget trait ───────────────────────────────────

    #[test]
    fn tabwidget_min_size_default_none() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.min_size(), None);
        assert_eq!(tw.max_size(), None);
    }

    #[test]
    fn tabwidget_set_min_max_size() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_min_size(Some(Size::new(100, 80)));
        tw.set_max_size(Some(Size::new(800, 600)));
        assert_eq!(tw.min_size(), Some(Size::new(100, 80)));
        assert_eq!(tw.max_size(), Some(Size::new(800, 600)));
    }

    // ── 11. Signal accessors ──────────────────────────────────────────────────

    #[test]
    fn tabwidget_current_changed_signal_emits_on_set_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        let emitted = Arc::new(std::sync::Mutex::new(None));
        tw.current_changed.connect({
            let emitted = Arc::clone(&emitted);
            move |idx: Arc<usize>| {
                *emitted.lock().unwrap() = Some(*idx);
            }
        });

        tw.set_current_index(1);
        assert_eq!(*emitted.lock().unwrap(), Some(1));
    }

    #[test]
    fn tabwidget_current_changed_not_emitted_for_same_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        tw.current_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<usize>| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        tw.set_current_index(0); // same as default
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tabwidget_current_changed_not_emitted_for_out_of_bounds() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);

        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        tw.current_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<usize>| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        tw.set_current_index(5); // out of bounds
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tabwidget_tab_close_requested_signal_accessible() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        let emitted = Arc::new(std::sync::Mutex::new(None));
        tw.tab_close_requested.connect({
            let emitted = Arc::clone(&emitted);
            move |idx: Arc<usize>| {
                *emitted.lock().unwrap() = Some(*idx);
            }
        });
        // Manually emit — tabwidget doesn't auto-close, but the signal is public
        tw.tab_close_requested.emit(1usize);
        assert_eq!(*emitted.lock().unwrap(), Some(1));
    }

    // ── 12. Geometry delegation ───────────────────────────────────────────────

    #[test]
    fn tabwidget_geometry_via_widget_trait() {
        let tw = TabWidget::new(Rect::new(5, 10, 200, 150));
        assert_eq!(tw.geometry(), Rect::new(5, 10, 200, 150));
        assert_eq!(tw.geometry(), Rect::new(5, 10, 200, 150));
    }

    #[test]
    fn tabwidget_set_geometry_via_widget_trait() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        tw.set_geometry(Rect::new(20, 30, 500, 400));
        assert_eq!(tw.geometry(), Rect::new(20, 30, 500, 400));
        assert_eq!(tw.position(), Point::new(20, 30));
        assert_eq!(tw.size(), Size::new(500, 400));
    }

    #[test]
    fn tabwidget_set_position_and_size() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 200, 100));
        tw.set_position(Point::new(50, 60));
        assert_eq!(tw.position(), Point::new(50, 60));
        assert_eq!(tw.size(), Size::new(200, 100));

        tw.set_size(Size::new(300, 150));
        assert_eq!(tw.position(), Point::new(50, 60));
        assert_eq!(tw.size(), Size::new(300, 150));
    }

    // ── 13. Widget ID and kind ────────────────────────────────────────────────

    #[test]
    fn tabwidget_kind_is_tabwidget() {
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.kind(), WidgetKind::TabWidget);
    }

    #[test]
    fn tabwidget_id_is_unique() {
        let tw1 = TabWidget::new(Rect::new(0, 0, 100, 100));
        let tw2 = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_ne!(tw1.id(), tw2.id());
    }

    #[test]
    fn tabwidget_accessible_name_falls_back_to_kind() {
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.accessible_name(), "TabWidget");
    }

    #[test]
    fn tabwidget_accessible_role_is_kind() {
        use crate::platform::accessibility::AccessibleRole;
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.accessible_role(), AccessibleRole::TabGroup);
    }

    // ── 14. Disabled state blocks events ──────────────────────────────────────

    #[test]
    fn tabwidget_disabled_does_not_switch_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);
        // Disable the entire widget
        tw.set_enabled(false);

        // Click on tab 1's position
        // tab_rect(0) with North position at Rect(0,0,300,200) starts at x=0, y=0
        let event = Event::mouse_press(0, 0, 1);
        tw.handle_event(&event);

        // current_index should still be 0 because the widget is disabled
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_enabled_switches_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // Click on tab 1's position
        // tab_rect(1) with North at Rect(0,0,300,200): x=(100+2)*1=102, y=0
        let event = Event::mouse_press(102, 0, 1);
        tw.handle_event(&event);

        assert_eq!(tw.current_index(), 1);
    }

    #[test]
    fn tabwidget_disabled_tab_does_not_switch_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        // Disable tab at index 1
        tw.tab_mut(1).unwrap().set_enabled(false);

        tw.set_current_index(0);

        // Click on tab 1's position
        let event = Event::mouse_press(102, 0, 1);
        tw.handle_event(&event);

        // Should NOT switch because tab 1 is disabled
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_right_click_does_not_switch() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // Right-click on tab 1's position
        let event = Event::mouse_press(102, 0, 3);
        tw.handle_event(&event);

        // Should NOT switch because only button 1 (left click) triggers switch
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_click_outside_tabs_does_not_change_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // Click far to the right of any tab
        let event = Event::mouse_press(500, 0, 1);
        tw.handle_event(&event);

        assert_eq!(tw.current_index(), 0);
    }

    // ── 15. SVG output verification ───────────────────────────────────────────

    #[test]
    fn tabwidget_svg_output_via_render_to_svg() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Home".to_string(), None);
        tw.add_tab("Settings".to_string(), None);

        let svg = render_to_svg(&mut tw);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
        assert!(svg.contains("width=\"300\""), "SVG should contain width=\"300\"");
        assert!(svg.contains("height=\"200\""), "SVG should contain height=\"200\"");
        // Should contain tab text
        assert!(svg.contains("Home"));
        assert!(svg.contains("Settings"));
        // Should contain fill and stroke attributes from the rendering
        assert!(svg.contains("fill=") || svg.contains("stroke="));
    }

    #[test]
    fn tabwidget_svg_output_with_explicit_geometry() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 120, 80));
        tw.add_tab("X".to_string(), None);

        let svg = render_widget_to_svg(&mut tw, Rect::new(0, 0, 120, 80));
        assert!(svg.contains("width=\"120\""));
        assert!(svg.contains("height=\"80\""));
    }

    // ── 16. Tab accessor (tab / tab_mut) ──────────────────────────────────────

    #[test]
    fn tabwidget_tab_accessor_out_of_bounds() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(tw.tab(0).is_none());
        assert!(tw.tab_mut(0).is_none());
    }

    #[test]
    fn tabwidget_tab_mut_allows_widget_assignment() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        tw.tab_mut(0).unwrap().set_widget(Some(wid1()));
        // Widget assigned to tab 0 is also the current widget (index 0)
        assert_eq!(tw.tab(0).unwrap().widget(), Some(wid1()));
        assert_eq!(tw.current_widget(), Some(wid1()));
    }

    // ── 17. Registry round-trip ───────────────────────────────────────────────

    #[test]
    fn tabwidget_registry_set_and_get() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(tw.registry().is_none());
        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        tw.set_registry(reg.clone());
        assert!(tw.registry().is_some());
        // Verify pointer identity
        assert!(Rc::ptr_eq(&reg, tw.registry().unwrap()));
    }

    // ── 18. Tab icon set/get ───────────────────────────────────────────────────

    #[test]
    fn tabwidget_tab_icon_default_none() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        assert!(tw.tab(0).unwrap().icon().is_none());
    }
}
