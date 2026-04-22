//! Tab widget.

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPosition {
    /// Tabs at the top
    North,
    /// Tabs at the bottom
    South,
    /// Tabs at the left
    West,
    /// Tabs at the right
    East,
}

impl Default for TabPosition {
    fn default() -> Self {
        Self::North
    }
}

/// Tab shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabShape {
    /// Rounded tabs
    Rounded,
    /// Triangular tabs
    Triangular,
    /// Rectangular tabs
    Rectangular,
}

impl Default for TabShape {
    fn default() -> Self {
        Self::Rounded
    }
}

impl Tab {
    /// Creates a new tab.
    pub fn new(title: String) -> Self {
        Self {
            title,
            icon: None,
            tooltip: String::new(),
            enabled: true,
            widget: None,
        }
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
        }
    }

    /// Adds a tab.
    pub fn add_tab(&mut self, title: String, widget: Option<ObjectId>) -> usize {
        let mut tab = Tab::new(title);
        tab.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.tabs.push(tab);
        self.tabs.len() - 1
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
        let tab_height = 24.0;
        let tab_width = 100.0;
        let spacing = 2.0;

        match self.tab_position {
            TabPosition::North => {
                let x = rect.x + (tab_width + spacing) * index as f32;
                Some(Rect::new(x, rect.y, tab_width, tab_height))
            }
            TabPosition::South => {
                let x = rect.x + (tab_width + spacing) * index as f32;
                Some(Rect::new(
                    x,
                    rect.y + rect.height - tab_height,
                    tab_width,
                    tab_height,
                ))
            }
            TabPosition::West => {
                let y = rect.y + (tab_height + spacing) * index as f32;
                Some(Rect::new(rect.x, y, tab_width, tab_height))
            }
            TabPosition::East => {
                let y = rect.y + (tab_height + spacing) * index as f32;
                Some(Rect::new(
                    rect.x + rect.width - tab_width,
                    y,
                    tab_width,
                    tab_height,
                ))
            }
        }
    }

    /// Returns content rectangle.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let tab_height = 24.0;

        match self.tab_position {
            TabPosition::North => Rect::new(
                rect.x,
                rect.y + tab_height,
                rect.width,
                rect.height - tab_height,
            ),
            TabPosition::South => Rect::new(rect.x, rect.y, rect.width, rect.height - tab_height),
            TabPosition::West => Rect::new(
                rect.x + tab_height,
                rect.y,
                rect.width - tab_height,
                rect.height,
            ),
            TabPosition::East => Rect::new(rect.x, rect.y, rect.width - tab_height, rect.height),
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
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}

impl EventHandler for TabWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);

        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if let Some(index) = self.tab_at_position(*pos) {
                        if self.tabs[index].enabled {
                            self.set_current_index(index);
                        }
                    }
                }
            }
            _ => {}
        }

        // Forward events to current widget
        if let Some(widget_id) = self.current_widget() {
            // TODO: Forward event to current widget
        }
    }
}

impl Draw for TabWidget {
    fn draw(&self, context: &mut RenderContext) {
        // Draw base widget
        self.base.draw(context);

        let rect = self.geometry();
        let content_rect = self.content_rect();

        // Draw content background
        context.fill_rect(
            content_rect.x,
            content_rect.y,
            content_rect.width,
            content_rect.height,
            Color::from_rgb(255, 255, 255),
        );

        // Draw content border
        context.draw_rect(
            content_rect.x,
            content_rect.y,
            content_rect.width,
            content_rect.height,
            Color::from_rgb(200, 200, 200),
        );

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

                context.fill_rect(
                    tab_rect.x,
                    tab_rect.y,
                    tab_rect.width,
                    tab_rect.height,
                    bg_color,
                );

                // Draw tab border
                let border_color = if !is_enabled {
                    Color::from_rgb(200, 200, 200)
                } else if is_current {
                    Color::from_rgb(200, 200, 200)
                } else {
                    Color::from_rgb(180, 180, 180)
                };

                context.draw_rect(
                    tab_rect.x,
                    tab_rect.y,
                    tab_rect.width,
                    tab_rect.height,
                    border_color,
                );

                // Draw tab text
                let text_color = if !is_enabled {
                    Color::from_rgb(150, 150, 150)
                } else {
                    Color::from_rgb(0, 0, 0)
                };

                context.draw_text(
                    tab_rect.x + tab_rect.width / 2.0,
                    tab_rect.y + tab_rect.height / 2.0,
                    &tab.title,
                    &Font::default(),
                    text_color,
                    Alignment::Center,
                );

                // Draw close button if closable
                if self.closable {
                    let close_size = 12.0;
                    let close_x = tab_rect.x + tab_rect.width - close_size - 5.0;
                    let close_y = tab_rect.y + (tab_rect.height - close_size) / 2.0;

                    context.draw_line(
                        close_x,
                        close_y,
                        close_x + close_size,
                        close_y + close_size,
                        Color::from_rgb(100, 100, 100),
                    );

                    context.draw_line(
                        close_x + close_size,
                        close_y,
                        close_x,
                        close_y + close_size,
                        Color::from_rgb(100, 100, 100),
                    );
                }
            }
        }

        // Draw current widget
        if let Some(widget_id) = self.current_widget() {
            // TODO: Draw current widget in content area
        }
    }
}
