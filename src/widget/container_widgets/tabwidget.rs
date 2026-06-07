//! Tab widget.
use crate::core::{Color, Font, ObjectId, Point, Rect};
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
        context.fill_rect(content_rect, Color::from_rgb(255, 255, 255));
        // Draw content border
        context.draw_rect(content_rect, Color::from_rgb(200, 200, 200));
        // Draw tabs
        for i in 0..self.tabs.len() {
            if let Some(tab_rect) = self.tab_rect(i) {
                let tab = &self.tabs[i];
                let is_current = i == self.current_index;
                let is_enabled = tab.enabled;
                // Draw tab background
                let bg_color = if !is_enabled {
                    Color::from_rgb(240, 240, 240)
                } else if is_current {
                    Color::from_rgb(255, 255, 255)
                } else {
                    Color::from_rgb(230, 230, 230)
                };
                context.fill_rect(tab_rect, bg_color);
                // Draw tab border
                let border_color = if !is_enabled || is_current {
                    Color::from_rgb(200, 200, 200)
                } else {
                    Color::from_rgb(180, 180, 180)
                };
                context.draw_rect(tab_rect, border_color);
                // Draw tab text
                let text_color = if !is_enabled {
                    Color::from_rgb(150, 150, 150)
                } else {
                    Color::from_rgb(0, 0, 0)
                };
                context.draw_text(
                    Point::new(
                        tab_rect.x + tab_rect.width as i32 / 2,
                        tab_rect.y + tab_rect.height as i32 / 2,
                    ),
                    &tab.title,
                    &Font::default(),
                    text_color,
                );
                // Draw close button if closable
                if self.closable {
                    let close_size = 12;
                    let close_x = tab_rect.x + tab_rect.width as i32 - close_size - 5;
                    let close_y = tab_rect.y + (tab_rect.height as i32 - close_size) / 2;
                    context.draw_line(
                        Point::new(close_x, close_y),
                        Point::new(close_x + close_size, close_y + close_size),
                        Color::from_rgb(100, 100, 100),
                    );
                    context.draw_line(
                        Point::new(close_x + close_size, close_y),
                        Point::new(close_x, close_y + close_size),
                        Color::from_rgb(100, 100, 100),
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
