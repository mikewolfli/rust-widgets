//! Dock widget.
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Dock widget.
pub struct DockWidget {
    base: BaseWidget,
    title: String,
    widget: Option<ObjectId>,
    features: DockWidgetFeatures,
    allowed_areas: DockWidgetAreas,
    floating: bool,
    docked: bool,
    pub dock_location_changed: Signal1<DockWidgetArea>,
    pub features_changed: Signal1<DockWidgetFeatures>,
    pub top_level_changed: Signal1<bool>,
}
/// Dock widget features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockWidgetFeatures {
    /// Dock widget can be closed
    pub dock_widget_closable: bool,
    /// Dock widget can be moved
    pub dock_widget_movable: bool,
    /// Dock widget can be floated
    pub dock_widget_floatable: bool,
    /// Dock widget can be vertical
    pub dock_widget_vertical_title_bar: bool,
    /// All features
    pub dock_widget_all_features: bool,
    /// No features
    pub dock_widget_no_features: bool,
}
impl Default for DockWidgetFeatures {
    fn default() -> Self {
        Self {
            dock_widget_closable: true,
            dock_widget_movable: true,
            dock_widget_floatable: true,
            dock_widget_vertical_title_bar: false,
            dock_widget_all_features: true,
            dock_widget_no_features: false,
        }
    }
}
impl DockWidgetFeatures {
    /// Creates features with all flags set.
    pub fn all() -> Self {
        Self {
            dock_widget_closable: true,
            dock_widget_movable: true,
            dock_widget_floatable: true,
            dock_widget_vertical_title_bar: true,
            dock_widget_all_features: true,
            dock_widget_no_features: false,
        }
    }
    /// Creates features with no flags set.
    pub fn none() -> Self {
        Self {
            dock_widget_closable: false,
            dock_widget_movable: false,
            dock_widget_floatable: false,
            dock_widget_vertical_title_bar: false,
            dock_widget_all_features: false,
            dock_widget_no_features: true,
        }
    }
}
/// Dock widget areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockWidgetAreas {
    /// Left dock area
    pub left_dock_widget_area: bool,
    /// Right dock area
    pub right_dock_widget_area: bool,
    /// Top dock area
    pub top_dock_widget_area: bool,
    /// Bottom dock area
    pub bottom_dock_widget_area: bool,
    /// All dock areas
    pub all_dock_widget_areas: bool,
    /// No dock areas
    pub no_dock_widget_areas: bool,
}
impl Default for DockWidgetAreas {
    fn default() -> Self {
        Self {
            left_dock_widget_area: true,
            right_dock_widget_area: true,
            top_dock_widget_area: true,
            bottom_dock_widget_area: true,
            all_dock_widget_areas: true,
            no_dock_widget_areas: false,
        }
    }
}
impl DockWidgetAreas {
    /// Creates areas with all flags set.
    pub fn all() -> Self {
        Self {
            left_dock_widget_area: true,
            right_dock_widget_area: true,
            top_dock_widget_area: true,
            bottom_dock_widget_area: true,
            all_dock_widget_areas: true,
            no_dock_widget_areas: false,
        }
    }
    /// Creates areas with no flags set.
    pub fn none() -> Self {
        Self {
            left_dock_widget_area: false,
            right_dock_widget_area: false,
            top_dock_widget_area: false,
            bottom_dock_widget_area: false,
            all_dock_widget_areas: false,
            no_dock_widget_areas: true,
        }
    }
}
/// Dock widget area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockWidgetArea {
    /// Left dock area
    LeftDockWidgetArea,
    /// Right dock area
    RightDockWidgetArea,
    /// Top dock area
    TopDockWidgetArea,
    /// Bottom dock area
    BottomDockWidgetArea,
    /// No dock area
    NoDockWidgetArea,
}
impl DockWidget {
    /// Creates a dock widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DockWidget, geometry, "DockWidget"),
            title: String::new(),
            widget: None,
            features: DockWidgetFeatures::default(),
            allowed_areas: DockWidgetAreas::default(),
            floating: false,
            docked: true,
            dock_location_changed: Signal1::new(),
            features_changed: Signal1::new(),
            top_level_changed: Signal1::new(),
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
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Returns features.
    pub fn features(&self) -> DockWidgetFeatures {
        self.features
    }
    /// Sets features.
    pub fn set_features(&mut self, features: DockWidgetFeatures) {
        if self.features.dock_widget_all_features != features.dock_widget_all_features
            || self.features.dock_widget_no_features != features.dock_widget_no_features
            || self.features.dock_widget_closable != features.dock_widget_closable
            || self.features.dock_widget_movable != features.dock_widget_movable
            || self.features.dock_widget_floatable != features.dock_widget_floatable
            || self.features.dock_widget_vertical_title_bar
                != features.dock_widget_vertical_title_bar
        {
            self.features = features;
            self.features_changed.emit(features);
        }
    }
    /// Returns allowed areas.
    pub fn allowed_areas(&self) -> DockWidgetAreas {
        self.allowed_areas
    }
    /// Sets allowed areas.
    pub fn set_allowed_areas(&mut self, areas: DockWidgetAreas) {
        self.allowed_areas = areas;
    }
    /// Returns whether dock widget is floating.
    pub fn is_floating(&self) -> bool {
        self.floating
    }
    /// Sets floating state.
    pub fn set_floating(&mut self, floating: bool) {
        if self.floating != floating {
            self.floating = floating;
            self.top_level_changed.emit(floating);
        }
    }
    /// Returns whether dock widget is docked.
    pub fn is_docked(&self) -> bool {
        self.docked
    }
    /// Sets docked state.
    pub fn set_docked(&mut self, docked: bool) {
        self.docked = docked;
    }
    /// Toggles floating state.
    pub fn toggle_floating(&mut self) {
        self.set_floating(!self.floating);
    }
    /// Returns title bar rectangle.
    fn title_bar_rect(&self) -> Rect {
        let rect = self.geometry();
        let title_bar_height = 24;
        Rect::new(rect.x, rect.y, rect.width, title_bar_height)
    }
    /// Returns content rectangle.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let title_bar_height = 24;
        Rect::new(
            rect.x,
            rect.y + title_bar_height,
            rect.width,
            rect.height - title_bar_height as u32,
        )
    }
    /// Returns close button rectangle.
    fn close_button_rect(&self) -> Option<Rect> {
        if !self.features.dock_widget_closable {
            return None;
        }
        let title_bar = self.title_bar_rect();
        let button_size = 16;
        Some(Rect::new(
            title_bar.x + title_bar.width as i32 - button_size - 5,
            title_bar.y + (title_bar.height as i32 - button_size) / 2,
            button_size as u32,
            button_size as u32,
        ))
    }
    /// Returns float button rectangle.
    fn float_button_rect(&self) -> Option<Rect> {
        if !self.features.dock_widget_floatable {
            return None;
        }
        let title_bar = self.title_bar_rect();
        let button_size = 16;
        let close_button_width = if self.features.dock_widget_closable {
            button_size + 5
        } else {
            0
        };
        Some(Rect::new(
            title_bar.x + title_bar.width as i32 - button_size - 5 - close_button_width,
            title_bar.y + (title_bar.height as i32 - button_size) / 2,
            button_size as u32,
            button_size as u32,
        ))
    }
    /// Returns whether point is in title bar.
    fn is_in_title_bar(&self, pos: Point) -> bool {
        self.title_bar_rect().contains(pos)
    }
    /// Returns whether point is in close button.
    fn is_in_close_button(&self, pos: Point) -> bool {
        if let Some(close_rect) = self.close_button_rect() {
            close_rect.contains(pos)
        } else {
            false
        }
    }
    /// Returns whether point is in float button.
    fn is_in_float_button(&self, pos: Point) -> bool {
        if let Some(float_rect) = self.float_button_rect() {
            float_rect.contains(pos)
        } else {
            false
        }
    }
}
// Implement Widget trait
impl Widget for DockWidget {
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
impl EventHandler for DockWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if self.is_in_close_button(*pos) && self.features.dock_widget_closable {
                        self.hide();
                    } else if self.is_in_float_button(*pos) && self.features.dock_widget_floatable {
                        self.toggle_floating();
                    } else if self.is_in_title_bar(*pos) && self.features.dock_widget_movable {
                        // Start dragging
                        self.base.set_mouse_pressed(true);
                    }
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.base.set_mouse_pressed(false);
                }
            }
            Event::MouseMove { pos } => {
                if self.base.is_mouse_pressed() && self.features.dock_widget_movable {
                    // Move dock widget
                    let rect = self.geometry();
                    self.set_geometry(Rect::new(pos.x, pos.y, rect.width, rect.height));
                }
            }
            _ => {}
        }
        // Forward events to widget
        if self.widget.is_some() {
            // TODO: Forward event to widget
        }
    }
}
impl Draw for DockWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let _rect = self.geometry();
        let title_bar = self.title_bar_rect();
        let content = self.content_rect();
        // Draw title bar
        let title_bar_color = if self.floating {
            Color::from_rgb(220, 220, 255)
        } else {
            Color::from_rgb(200, 200, 200)
        };
        context.fill_rect(title_bar, title_bar_color);
        // Draw title bar border
        context.draw_rect(title_bar, Color::from_rgb(150, 150, 150));
        // Draw title text
        context.draw_text(
            Point::new(title_bar.x + 5, title_bar.y + title_bar.height as i32 / 2),
            &self.title,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        // Draw close button if enabled
        if self.features.dock_widget_closable {
            if let Some(close_rect) = self.close_button_rect() {
                let close_color = if self.base.is_enabled() {
                    Color::from_rgb(100, 100, 100)
                } else {
                    Color::from_rgb(200, 200, 200)
                };
                context.draw_line(Point::new(close_rect.x, close_rect.y), Point::new(close_rect.x + close_rect.width as i32, close_rect.y + close_rect.height as i32), close_color);
                context.draw_line(Point::new(close_rect.x + close_rect.width as i32, close_rect.y), Point::new(close_rect.x, close_rect.y + close_rect.height as i32), close_color);
            }
        }
        // Draw float button if enabled
        if self.features.dock_widget_floatable {
            if let Some(float_rect) = self.float_button_rect() {
                let float_color = if self.floating {
                    Color::from_rgb(0, 120, 215)
                } else if self.base.is_enabled() {
                    Color::from_rgb(100, 100, 100)
                } else {
                    Color::from_rgb(200, 200, 200)
                };
                // Draw float icon (four arrows)
                let center_x = float_rect.x + float_rect.width as i32 / 2;
                let center_y = float_rect.y + float_rect.height as i32 / 2;
                let arrow_size = 4;
                // Use integer coordinates for drawing
                let y0 = float_rect.y + 2;
                // Up arrow
                context.draw_line(Point::new(center_x, y0), Point::new(center_x, y0 + arrow_size), float_color);
                context.draw_line(Point::new(center_x - arrow_size / 2, y0 + arrow_size / 2), Point::new(center_x + arrow_size / 2, y0 + arrow_size / 2), float_color);
                // Down arrow
                let y1 = float_rect.y + float_rect.height as i32 - 2;
                context.draw_line(Point::new(center_x, y1 - arrow_size), Point::new(center_x, y1), float_color);
                context.draw_line(Point::new(center_x - arrow_size / 2, y1 - arrow_size / 2), Point::new(center_x + arrow_size / 2, y1 - arrow_size / 2), float_color);
                // Left arrow
                let x0 = float_rect.x + 2;
                context.draw_line(Point::new(x0, center_y), Point::new(x0 + arrow_size, center_y), float_color);
                context.draw_line(Point::new(x0 + arrow_size / 2, center_y - arrow_size / 2), Point::new(x0 + arrow_size / 2, center_y + arrow_size / 2), float_color);
                // Right arrow
                let x1 = float_rect.x + float_rect.width as i32 - 2;
                context.draw_line(Point::new(x1 - arrow_size, center_y), Point::new(x1, center_y), float_color);
                context.draw_line(Point::new(x1 - arrow_size / 2, center_y - arrow_size / 2), Point::new(x1 - arrow_size / 2, center_y + arrow_size / 2), float_color);
            }
        }
        // Draw content background
        context.fill_rect(content, Color::from_rgb(255, 255, 255));
        // Draw content border
        context.draw_rect(content, Color::from_rgb(200, 200, 200));
        // Draw widget
        if self.widget.is_some() {
            // TODO: Draw widget in content area
        }
    }
}
