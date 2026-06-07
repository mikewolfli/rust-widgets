//! MDI area widget.
use crate::core::{Color, Font, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::{BaseWidget, Draw, Image, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// MDI area widget.
pub struct MdiArea {
    base: BaseWidget,
    subwindows: Vec<MdiSubWindow>,
    active_subwindow: Option<usize>,
    view_mode: ViewMode,
    background: Background,
    activation_order: ActivationOrder,
    pub subwindow_activated: Signal1<ObjectId>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
/// MDI sub-window.
pub struct MdiSubWindow {
    widget: ObjectId,
    geometry: Rect,
    title: String,
    icon: Option<Image>,
    minimized: bool,
    maximized: bool,
    closable: bool,
    movable: bool,
    resizable: bool,
    z_order: i32,
}
/// MDI view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Sub-window mode
    #[default]
    SubWindowView,
    /// Tabbed view
    TabbedView,
}
/// MDI background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Background {
    /// No background
    NoBackground,
    /// Plain color background
    #[default]
    Plain,
    /// Gradient background
    Gradient,
    /// Pattern background
    Pattern,
}
/// MDI activation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivationOrder {
    /// Activation follows creation order
    CreationOrder,
    /// Activation follows stacking order
    #[default]
    StackingOrder,
    /// Activation follows history order
    HistoryOrder,
}
impl MdiSubWindow {
    /// Creates a new MDI sub-window.
    pub fn new(widget: ObjectId, geometry: Rect) -> Self {
        Self {
            widget,
            geometry,
            title: String::new(),
            icon: None,
            minimized: false,
            maximized: false,
            closable: true,
            movable: true,
            resizable: true,
            z_order: 0,
        }
    }
    /// Returns widget.
    pub fn widget(&self) -> ObjectId {
        self.widget
    }
    /// Returns geometry.
    pub fn geometry(&self) -> Rect {
        self.geometry
    }
    /// Sets geometry.
    pub fn set_geometry(&mut self, geometry: Rect) {
        self.geometry = geometry;
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
    /// Returns whether sub-window is minimized.
    pub fn is_minimized(&self) -> bool {
        self.minimized
    }
    /// Sets minimized state.
    pub fn set_minimized(&mut self, minimized: bool) {
        self.minimized = minimized;
    }
    /// Returns whether sub-window is maximized.
    pub fn is_maximized(&self) -> bool {
        self.maximized
    }
    /// Sets maximized state.
    pub fn set_maximized(&mut self, maximized: bool) {
        self.maximized = maximized;
    }
    /// Returns whether sub-window is closable.
    pub fn is_closable(&self) -> bool {
        self.closable
    }
    /// Sets closable state.
    pub fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
    }
    /// Returns whether sub-window is movable.
    pub fn is_movable(&self) -> bool {
        self.movable
    }
    /// Sets movable state.
    pub fn set_movable(&mut self, movable: bool) {
        self.movable = movable;
    }
    /// Returns whether sub-window is resizable.
    pub fn is_resizable(&self) -> bool {
        self.resizable
    }
    /// Sets resizable state.
    pub fn set_resizable(&mut self, resizable: bool) {
        self.resizable = resizable;
    }
    /// Returns z-order.
    pub fn z_order(&self) -> i32 {
        self.z_order
    }
    /// Sets z-order.
    pub fn set_z_order(&mut self, z_order: i32) {
        self.z_order = z_order;
    }
}
impl MdiArea {
    /// Creates an MDI area.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MdiArea, geometry, "MdiArea"),
            subwindows: Vec::new(),
            active_subwindow: None,
            view_mode: ViewMode::SubWindowView,
            background: Background::Plain,
            activation_order: ActivationOrder::StackingOrder,
            subwindow_activated: Signal1::new(),
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
    /// Adds a sub-window.
    pub fn add_sub_window(&mut self, widget: ObjectId, geometry: Rect) -> usize {
        let mut subwindow = MdiSubWindow::new(widget, geometry);
        subwindow.z_order = self.subwindows.len() as i32;
        self.base.add_child(widget);
        self.subwindows.push(subwindow);
        let index = self.subwindows.len().saturating_sub(1);
        if self.active_subwindow.is_none() {
            self.active_subwindow = Some(index);
            self.subwindow_activated.emit(widget);
        }
        index
    }
    /// Removes a sub-window.
    pub fn remove_sub_window(&mut self, widget: ObjectId) {
        if let Some(index) = self.subwindows.iter().position(|sw| sw.widget == widget) {
            self.base.remove_child(widget);
            self.subwindows.remove(index);
            if self.active_subwindow == Some(index) {
                self.active_subwindow = None;
                // Try to activate another sub-window
                if !self.subwindows.is_empty() {
                    let new_index = index.min(self.subwindows.len() - 1);
                    self.active_subwindow = Some(new_index);
                    self.subwindow_activated.emit(self.subwindows[new_index].widget);
                }
            } else if let Some(active_index) = self.active_subwindow {
                if active_index > index {
                    self.active_subwindow = Some(active_index - 1);
                }
            }
        }
    }
    /// Returns number of sub-windows.
    pub fn sub_window_count(&self) -> usize {
        self.subwindows.len()
    }
    /// Returns active sub-window.
    pub fn active_sub_window(&self) -> Option<ObjectId> {
        self.active_subwindow.and_then(|index| self.subwindows.get(index).map(|sw| sw.widget))
    }
    /// Sets active sub-window.
    pub fn set_active_sub_window(&mut self, widget: ObjectId) {
        if let Some(index) = self.subwindows.iter().position(|sw| sw.widget == widget) {
            if self.active_subwindow != Some(index) {
                self.active_subwindow = Some(index);
                self.subwindow_activated.emit(widget);
            }
        }
    }
    /// Returns sub-window at index.
    pub fn sub_window(&self, index: usize) -> Option<&MdiSubWindow> {
        self.subwindows.get(index)
    }
    /// Returns mutable sub-window at index.
    pub fn sub_window_mut(&mut self, index: usize) -> Option<&mut MdiSubWindow> {
        self.subwindows.get_mut(index)
    }
    /// Returns view mode.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }
    /// Sets view mode.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }
    /// Returns background.
    pub fn background(&self) -> Background {
        self.background
    }
    /// Sets background.
    pub fn set_background(&mut self, background: Background) {
        self.background = background;
    }
    /// Returns activation order.
    pub fn activation_order(&self) -> ActivationOrder {
        self.activation_order
    }
    /// Sets activation order.
    pub fn set_activation_order(&mut self, order: ActivationOrder) {
        self.activation_order = order;
    }
    /// Cascade sub-windows.
    pub fn cascade_sub_windows(&mut self) {
        let area_rect = self.geometry();
        let count = self.subwindows.len();
        if count == 0 {
            return;
        }
        let offset = 30;
        let max_width =
            (area_rect.width as f32 - offset as f32 * (count as f32 - 1.0)).max(0.0) as u32;
        let max_height =
            (area_rect.height as f32 - offset as f32 * (count as f32 - 1.0)).max(0.0) as u32;
        for (i, subwindow) in self.subwindows.iter_mut().enumerate() {
            let x = area_rect.x as f32 + offset as f32 * i as f32;
            let y = area_rect.y as f32 + offset as f32 * i as f32;
            subwindow.geometry = Rect::new(x as i32, y as i32, max_width, max_height);
        }
    }
    /// Tile sub-windows.
    pub fn tile_sub_windows(&mut self) {
        let area_rect = self.geometry();
        let count = self.subwindows.len();
        if count == 0 {
            return;
        }
        let cols = (count as f32).sqrt().ceil() as usize;
        let rows = (count as f32 / cols as f32).ceil() as usize;
        let cell_width = (area_rect.width as f32 / cols as f32).max(0.0) as u32;
        let cell_height = (area_rect.height as f32 / rows as f32).max(0.0) as u32;
        for (i, subwindow) in self.subwindows.iter_mut().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = area_rect.x as f32 + cell_width as f32 * col as f32;
            let y = area_rect.y as f32 + cell_height as f32 * row as f32;
            subwindow.geometry = Rect::new(x as i32, y as i32, cell_width, cell_height);
        }
    }
    /// Arranges minimized sub-windows.
    pub fn arrange_icons(&mut self) {
        let area_rect = self.geometry();
        let mut minimized: Vec<_> = self.subwindows.iter_mut().filter(|sw| sw.minimized).collect();
        let count = minimized.len();
        if count == 0 {
            return;
        }
        let icon_width = 100;
        let icon_height = 80;
        let spacing = 10;
        let cols = ((area_rect.width as f32 - spacing as f32)
            / (icon_width as f32 + spacing as f32))
            .floor() as usize;
        let _rows = (count as f32 / cols as f32).ceil() as usize;
        for (i, subwindow) in minimized.iter_mut().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = area_rect.x + spacing + (icon_width as i32 + spacing) * col as i32;
            let y = area_rect.y + area_rect.height as i32
                - icon_height
                - spacing
                - (icon_height + spacing) * row as i32;
            subwindow.geometry = Rect::new(x, y, icon_width, icon_height as u32);
        }
    }
    /// Activates next sub-window.
    pub fn activate_next_sub_window(&mut self) {
        if self.subwindows.is_empty() {
            return;
        }
        let current = self.active_subwindow.unwrap_or(0);
        let next = (current + 1) % self.subwindows.len();
        self.set_active_sub_window(self.subwindows[next].widget);
    }
    /// Activates previous sub-window.
    pub fn activate_previous_sub_window(&mut self) {
        if self.subwindows.is_empty() {
            return;
        }
        let current = self.active_subwindow.unwrap_or(0);
        let prev = if current == 0 { self.subwindows.len() - 1 } else { current - 1 };
        self.set_active_sub_window(self.subwindows[prev].widget);
    }
    /// Returns sub-window at position.
    fn sub_window_at_position(&self, pos: Point) -> Option<usize> {
        // Check from top (highest z-order) to bottom
        let mut sorted_indices: Vec<usize> = (0..self.subwindows.len()).collect();
        sorted_indices.sort_by_key(|&i| -self.subwindows[i].z_order);
        for index in sorted_indices {
            let subwindow = &self.subwindows[index];
            if subwindow.geometry.contains(pos) {
                return Some(index);
            }
        }
        None
    }
}
// Implement Widget trait
impl Widget for MdiArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for MdiArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if let Event::MousePress { pos, button } = event {
            if *button == 1 {
                if let Some(index) = self.sub_window_at_position(*pos) {
                    self.set_active_sub_window(self.subwindows[index].widget);
                }
            }
        }
        // Forward events to active sub-window via registry
        if let Some(widget_id) = self.active_sub_window() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for MdiArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        // Draw background
        match self.background {
            Background::NoBackground => {
                // No background
            }
            Background::Plain => {
                context.fill_rect(rect, Color::from_rgb(240, 240, 240));
            }
            Background::Gradient => {
                // Draw gradient background
                for y in 0..rect.height as i32 {
                    let ratio = y as f32 / rect.height as f32;
                    let color = Color::from_rgb(
                        (240.0 * (1.0 - ratio) + 200.0 * ratio) as u8,
                        (240.0 * (1.0 - ratio) + 200.0 * ratio) as u8,
                        (240.0 * (1.0 - ratio) + 200.0 * ratio) as u8,
                    );
                    context.draw_line(
                        Point::new(rect.x, rect.y + y),
                        Point::new(rect.x + rect.width as i32, rect.y + y),
                        color,
                    );
                }
            }
            Background::Pattern => {
                // Draw pattern background
                let pattern_size = 20;
                for y in 0..(rect.height / pattern_size) as i32 {
                    for x in 0..(rect.width / pattern_size) as i32 {
                        let color = if (x + y) % 2 == 0 {
                            Color::from_rgb(245, 245, 245)
                        } else {
                            Color::from_rgb(235, 235, 235)
                        };
                        context.fill_rect(
                            Rect::new(
                                (rect.x as f32 + x as f32 * pattern_size as f32) as i32,
                                (rect.y as f32 + y as f32 * pattern_size as f32) as i32,
                                pattern_size,
                                pattern_size,
                            ),
                            color,
                        );
                    }
                }
            }
        }
        // Draw sub-windows
        // Sort by z-order (lowest first, so highest draws last)
        let mut sorted_indices: Vec<usize> = (0..self.subwindows.len()).collect();
        sorted_indices.sort_by_key(|&i| self.subwindows[i].z_order);
        for index in sorted_indices {
            let subwindow = &self.subwindows[index];
            let is_active = self.active_subwindow == Some(index);
            // Draw sub-window frame
            let frame_rect = subwindow.geometry;
            // Draw frame background
            let bg_color = if is_active {
                Color::from_rgb(255, 255, 255)
            } else {
                Color::from_rgb(250, 250, 250)
            };
            context.fill_rect(frame_rect, bg_color);
            // Draw frame border
            let border_color = if is_active {
                Color::from_rgb(0, 120, 215)
            } else {
                Color::from_rgb(200, 200, 200)
            };
            context.draw_rect(frame_rect, border_color);
            // Draw title bar
            let title_bar_height = 24;
            let title_bar_color = if is_active {
                Color::from_rgb(0, 120, 215)
            } else {
                Color::from_rgb(180, 180, 180)
            };
            context.fill_rect(
                Rect::new(frame_rect.x, frame_rect.y, frame_rect.width, title_bar_height as u32),
                title_bar_color,
            );
            // Draw title text
            let text_color =
                if is_active { Color::from_rgb(255, 255, 255) } else { Color::from_rgb(0, 0, 0) };
            context.draw_text(
                Point::new(frame_rect.x + 5, frame_rect.y + title_bar_height / 2),
                &subwindow.title,
                &Font::default(),
                text_color,
            );
            // Draw close button if closable
            if subwindow.closable {
                let close_size = 12;
                let close_x = frame_rect.x + frame_rect.width as i32 - close_size - 5;
                let close_y = frame_rect.y + (title_bar_height - close_size) / 2;
                let close_color = if is_active {
                    Color::from_rgb(255, 255, 255)
                } else {
                    Color::from_rgb(100, 100, 100)
                };
                context.draw_line(
                    Point::new(close_x, close_y),
                    Point::new(close_x + close_size, close_y + close_size),
                    close_color,
                );
                context.draw_line(
                    Point::new(close_x + close_size, close_y),
                    Point::new(close_x, close_y + close_size),
                    close_color,
                );
            }
            // Draw widget content via registry
            let content_rect = Rect::new(
                frame_rect.x,
                frame_rect.y + title_bar_height,
                frame_rect.width,
                frame_rect.height.saturating_sub(title_bar_height as u32),
            );
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(subwindow.widget, context);
            }
            let _content_rect = content_rect;
        }
    }
}
