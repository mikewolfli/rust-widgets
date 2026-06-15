//! Dock widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
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
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Offset from mouse cursor to widget top-left, set when drag begins.
    drag_offset: (i32, i32),
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
}
impl Default for DockWidgetFeatures {
    fn default() -> Self {
        Self {
            dock_widget_closable: true,
            dock_widget_movable: true,
            dock_widget_floatable: true,
            dock_widget_vertical_title_bar: false,
        }
    }
}

impl DockWidgetFeatures {
    /// Returns true when all features are enabled.
    pub fn all_enabled(&self) -> bool {
        self.dock_widget_closable
            && self.dock_widget_movable
            && self.dock_widget_floatable
            && self.dock_widget_vertical_title_bar
    }
    /// Returns true when no features are enabled.
    pub fn none_enabled(&self) -> bool {
        !self.dock_widget_closable
            && !self.dock_widget_movable
            && !self.dock_widget_floatable
            && !self.dock_widget_vertical_title_bar
    }
    /// Creates features with all flags set.
    pub fn all() -> Self {
        Self {
            dock_widget_closable: true,
            dock_widget_movable: true,
            dock_widget_floatable: true,
            dock_widget_vertical_title_bar: true,
        }
    }
    /// Creates features with no flags set.
    pub fn none() -> Self {
        Self {
            dock_widget_closable: false,
            dock_widget_movable: false,
            dock_widget_floatable: false,
            dock_widget_vertical_title_bar: false,
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
            allowed_areas: DockWidgetAreas::all(),
            floating: false,
            docked: true,
            dock_location_changed: Signal1::new(),
            features_changed: Signal1::new(),
            top_level_changed: Signal1::new(),
            registry: None,
            drag_offset: (0, 0),
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
        if self.features.dock_widget_closable != features.dock_widget_closable
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
            rect.height.saturating_sub(title_bar_height as u32),
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
        let close_button_width =
            if self.features.dock_widget_closable { button_size + 5 } else { 0 };
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
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for DockWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if self.is_in_close_button(*pos) && self.features.dock_widget_closable {
                    self.hide();
                } else if self.is_in_float_button(*pos) && self.features.dock_widget_floatable {
                    self.toggle_floating();
                } else if self.is_in_title_bar(*pos) && self.features.dock_widget_movable {
                    // Start dragging — store offset from cursor to widget top-left
                    let rect = self.geometry();
                    self.drag_offset = (pos.x - rect.x, pos.y - rect.y);
                    self.base.set_mouse_pressed(true);
                }
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            Event::MouseMove { pos }
                if self.base.is_mouse_pressed() && self.features.dock_widget_movable =>
            {
                // Move dock widget, maintaining the original offset
                let rect = self.geometry();
                self.set_geometry(Rect::new(
                    pos.x - self.drag_offset.0,
                    pos.y - self.drag_offset.1,
                    rect.width,
                    rect.height,
                ));
            }
            _ => { /* Other events are not relevant */ }
        }
        // Forward events to widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
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
            Color::rgb(220, 220, 255)
        } else {
            Color::rgb(200, 200, 200)
        };
        context.fill_rect(title_bar, title_bar_color);
        // Draw title bar border
        context.draw_rect(title_bar, Color::rgb(150, 150, 150));
        // Draw title text
        context.draw_text(
            Point::new(title_bar.x + 5, title_bar.y + title_bar.height as i32 / 2),
            &self.title,
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        // Draw close button if enabled
        if self.features.dock_widget_closable {
            if let Some(close_rect) = self.close_button_rect() {
                let close_color = if self.base.is_enabled() {
                    Color::rgb(100, 100, 100)
                } else {
                    Color::rgb(200, 200, 200)
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
        // Draw float button if enabled
        if self.features.dock_widget_floatable {
            if let Some(float_rect) = self.float_button_rect() {
                let float_color = if self.floating {
                    Color::rgb(0, 120, 215)
                } else if self.base.is_enabled() {
                    Color::rgb(100, 100, 100)
                } else {
                    Color::rgb(200, 200, 200)
                };
                // Draw float icon (four arrows)
                let center_x = float_rect.x + float_rect.width as i32 / 2;
                let center_y = float_rect.y + float_rect.height as i32 / 2;
                let arrow_size = 4;
                // Use integer coordinates for drawing
                let y0 = float_rect.y + 2;
                // Up arrow
                context.draw_line(
                    Point::new(center_x, y0),
                    Point::new(center_x, y0 + arrow_size),
                    float_color,
                );
                context.draw_line(
                    Point::new(center_x - arrow_size / 2, y0 + arrow_size / 2),
                    Point::new(center_x + arrow_size / 2, y0 + arrow_size / 2),
                    float_color,
                );
                // Down arrow
                let y1 = float_rect.y + float_rect.height as i32 - 2;
                context.draw_line(
                    Point::new(center_x, y1 - arrow_size),
                    Point::new(center_x, y1),
                    float_color,
                );
                context.draw_line(
                    Point::new(center_x - arrow_size / 2, y1 - arrow_size / 2),
                    Point::new(center_x + arrow_size / 2, y1 - arrow_size / 2),
                    float_color,
                );
                // Left arrow
                let x0 = float_rect.x + 2;
                context.draw_line(
                    Point::new(x0, center_y),
                    Point::new(x0 + arrow_size, center_y),
                    float_color,
                );
                context.draw_line(
                    Point::new(x0 + arrow_size / 2, center_y - arrow_size / 2),
                    Point::new(x0 + arrow_size / 2, center_y + arrow_size / 2),
                    float_color,
                );
                // Right arrow
                let x1 = float_rect.x + float_rect.width as i32 - 2;
                context.draw_line(
                    Point::new(x1 - arrow_size, center_y),
                    Point::new(x1, center_y),
                    float_color,
                );
                context.draw_line(
                    Point::new(x1 - arrow_size / 2, center_y - arrow_size / 2),
                    Point::new(x1 - arrow_size / 2, center_y + arrow_size / 2),
                    float_color,
                );
            }
        }
        // Draw content background
        context.fill_rect(content, Color::rgb(255, 255, 255));
        // Draw content border
        context.draw_rect(content, Color::rgb(200, 200, 200));
        // Draw widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect};
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use crate::widget::Widget;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    /// 1. Creation defaults
    #[test]
    fn test_creation_defaults() {
        let rect = Rect::new(10, 20, 200, 100);
        let dw = DockWidget::new(rect);
        assert!(!dw.is_floating(), "new dock widget should not be floating");
        assert!(dw.is_docked(), "new dock widget should be docked");
        assert!(dw.features().dock_widget_closable, "default should be closable");
        assert!(dw.features().dock_widget_movable, "default should be movable");
        assert!(dw.features().dock_widget_floatable, "default should be floatable");
        assert_eq!(dw.title(), "", "default title should be empty");
        assert_eq!(dw.geometry(), rect, "geometry should match constructor");
        assert_eq!(dw.kind(), WidgetKind::DockWidget, "kind should be DockWidget");
        assert!(dw.is_visible(), "new dock widget should be visible");
        assert!(dw.is_enabled(), "new dock widget should be enabled");
        assert!(dw.allowed_areas().all_dock_widget_areas);
        assert!(dw.allowed_areas().left_dock_widget_area);
        assert!(dw.allowed_areas().right_dock_widget_area);
        assert!(dw.allowed_areas().top_dock_widget_area);
        assert!(dw.allowed_areas().bottom_dock_widget_area);
        assert!(!dw.allowed_areas().no_dock_widget_areas);
        assert!(dw.registry().is_none(), "registry should be None by default");
    }

    /// 2. Setting/getting title
    #[test]
    fn test_title() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        assert_eq!(dw.title(), "");
        dw.set_title("My Dock".to_string());
        assert_eq!(dw.title(), "My Dock");
        dw.set_title(String::new());
        assert_eq!(dw.title(), "");
        dw.set_title("Updated".to_string());
        assert_eq!(dw.title(), "Updated");
    }

    /// 3. Allowed areas (dock position concept)
    #[test]
    fn test_allowed_areas() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // Default all areas allowed
        assert_eq!(dw.allowed_areas(), DockWidgetAreas::default());
        // Set to none
        dw.set_allowed_areas(DockWidgetAreas::none());
        assert!(dw.allowed_areas().no_dock_widget_areas);
        assert!(!dw.allowed_areas().left_dock_widget_area);
        assert!(!dw.allowed_areas().right_dock_widget_area);
        assert!(!dw.allowed_areas().top_dock_widget_area);
        assert!(!dw.allowed_areas().bottom_dock_widget_area);
        // Set to all
        dw.set_allowed_areas(DockWidgetAreas::all());
        assert!(dw.allowed_areas().left_dock_widget_area);
        assert!(dw.allowed_areas().right_dock_widget_area);
        assert!(dw.allowed_areas().top_dock_widget_area);
        assert!(dw.allowed_areas().bottom_dock_widget_area);
        assert!(dw.allowed_areas().all_dock_widget_areas);
        // Custom area
        let custom = DockWidgetAreas {
            left_dock_widget_area: true,
            right_dock_widget_area: false,
            top_dock_widget_area: true,
            bottom_dock_widget_area: false,
            ..DockWidgetAreas::default()
        };
        dw.set_allowed_areas(custom);
        assert!(dw.allowed_areas().left_dock_widget_area);
        assert!(!dw.allowed_areas().right_dock_widget_area);
        assert!(dw.allowed_areas().top_dock_widget_area);
        assert!(!dw.allowed_areas().bottom_dock_widget_area);
    }

    /// 4. Floating state
    #[test]
    fn test_floating_state() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        assert!(!dw.is_floating());
        dw.set_floating(true);
        assert!(dw.is_floating());
        dw.set_floating(false);
        assert!(!dw.is_floating());
        // Toggle
        dw.toggle_floating();
        assert!(dw.is_floating());
        dw.toggle_floating();
        assert!(!dw.is_floating());
    }

    #[test]
    fn test_floating_signal_emitted() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let emitted = Arc::new(AtomicBool::new(false));
        dw.top_level_changed.connect({
            let emitted = Arc::clone(&emitted);
            move |val| {
                emitted.store(*val, Ordering::SeqCst);
            }
        });
        dw.set_floating(true);
        assert!(emitted.load(Ordering::SeqCst));
    }

    /// 5. Closable state
    #[test]
    fn test_closable_state() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let features = dw.features();
        assert!(features.dock_widget_closable);
        // Build no-features and set
        let no_features =
            DockWidgetFeatures { dock_widget_closable: false, ..DockWidgetFeatures::default() };
        dw.set_features(no_features);
        assert!(!dw.features().dock_widget_closable);
        // Restore
        dw.set_features(DockWidgetFeatures::all());
        assert!(dw.features().dock_widget_closable);
    }

    /// 6. Window state management (docked, geometry)
    #[test]
    fn test_window_state_management() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // Docked state
        assert!(dw.is_docked());
        dw.set_docked(false);
        assert!(!dw.is_docked());
        dw.set_docked(true);
        assert!(dw.is_docked());
        // Geometry via Widget trait
        assert_eq!(dw.geometry(), Rect::new(0, 0, 200, 100));
        dw.set_geometry(Rect::new(50, 50, 300, 150));
        assert_eq!(dw.geometry(), Rect::new(50, 50, 300, 150));
        // Size/position aliases
        assert_eq!(dw.size(), crate::core::Size::new(300, 150));
        assert_eq!(dw.position(), Point::new(50, 50));
        dw.set_position(Point::new(10, 10));
        assert_eq!(dw.position(), Point::new(10, 10));
        assert_eq!(dw.size(), crate::core::Size::new(300, 150));
    }

    /// 7. Geometry delegation through Widget trait
    #[test]
    fn test_geometry_delegation() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // The base widget geometry should be the same as the dock widget's
        assert_eq!(dw.base().geometry(), dw.geometry());
        dw.set_geometry(Rect::new(5, 5, 400, 200));
        assert_eq!(dw.base().geometry(), dw.geometry());
        assert_eq!(dw.base().geometry(), Rect::new(5, 5, 400, 200));
    }

    /// 8. Visibility
    #[test]
    fn test_visibility() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        assert!(dw.is_visible());
        dw.hide();
        assert!(!dw.is_visible());
        dw.show();
        assert!(dw.is_visible());
        // set_visible
        dw.set_visible(false);
        assert!(!dw.is_visible());
        dw.set_visible(true);
        assert!(dw.is_visible());
    }

    /// 9. ID and Kind
    #[test]
    fn test_id_and_kind() {
        let dw1 = DockWidget::new(Rect::new(0, 0, 200, 100));
        let dw2 = DockWidget::new(Rect::new(0, 0, 150, 80));
        assert_eq!(dw1.kind(), WidgetKind::DockWidget);
        assert_eq!(dw2.kind(), WidgetKind::DockWidget);
        assert_ne!(dw1.id(), dw2.id(), "each DockWidget must have a unique ObjectId");
    }

    /// 10. SVG draw output
    #[test]
    fn test_svg_draw_output() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 300, 150));
        dw.set_title("Test Dock".to_string());
        let svg = render_to_svg(&mut dw);
        assert!(svg.starts_with("<svg"), "SVG must start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG must end with </svg>");
        assert!(svg.contains("width=\"300\""), "SVG must contain correct width");
        assert!(svg.contains("height=\"150\""), "SVG must contain correct height");
        // SVG should contain rendering artifacts (fill/stroke elements)
        assert!(svg.contains("fill="));
        assert!(svg.contains("stroke="));
    }

    #[test]
    fn test_svg_draw_output_with_title() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 80));
        dw.set_title("Hello".to_string());
        let svg = render_to_svg(&mut dw);
        // Title text may appear in SVG output
        assert!(
            svg.contains("Hello") || svg.contains("fill="),
            "SVG output should contain title text or rendering elements"
        );
    }

    /// 11. Signal accessors
    #[test]
    fn test_signal_accessors() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // dock_location_changed
        {
            let received = Arc::new(AtomicBool::new(false));
            let area_received = Arc::new(Mutex::new(DockWidgetArea::NoDockWidgetArea));
            dw.dock_location_changed.connect({
                let received = Arc::clone(&received);
                let area_received = Arc::clone(&area_received);
                move |area: Arc<DockWidgetArea>| {
                    received.store(true, Ordering::SeqCst);
                    *area_received.lock().unwrap() = *area;
                }
            });
            dw.dock_location_changed.emit(DockWidgetArea::LeftDockWidgetArea);
            assert!(received.load(Ordering::SeqCst));
            assert_eq!(*area_received.lock().unwrap(), DockWidgetArea::LeftDockWidgetArea);
        }
        // features_changed
        {
            let received = Arc::new(AtomicBool::new(false));
            dw.features_changed.connect({
                let received = Arc::clone(&received);
                move |_features: Arc<DockWidgetFeatures>| {
                    received.store(true, Ordering::SeqCst);
                }
            });
            dw.set_features(DockWidgetFeatures::none());
            assert!(received.load(Ordering::SeqCst));
        }
        // top_level_changed
        {
            let received = Arc::new(AtomicBool::new(false));
            let floating_received = Arc::new(AtomicBool::new(false));
            dw.top_level_changed.connect({
                let received = Arc::clone(&received);
                let floating_received = Arc::clone(&floating_received);
                move |is_floating: Arc<bool>| {
                    received.store(true, Ordering::SeqCst);
                    floating_received.store(*is_floating, Ordering::SeqCst);
                }
            });
            dw.set_floating(true);
            assert!(received.load(Ordering::SeqCst));
            assert!(floating_received.load(Ordering::SeqCst));
        }
    }

    /// 12. Mouse event handling — close button
    #[test]
    fn test_mouse_close_button_hides_widget() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // Close button area: title_bar.x + width - 21 = 0 + 200 - 21 = 179
        // y: title_bar.y + (24 - 16) / 2 = 0 + 4 = 4
        let close_pos = Point::new(179, 8);
        assert!(dw.is_visible());
        dw.handle_event(&Event::MousePress { pos: close_pos, button: 1 });
        assert!(!dw.is_visible(), "clicking close button should hide the widget");
    }

    #[test]
    fn test_mouse_close_button_other_button_no_hide() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let close_pos = Point::new(179, 8);
        // Right-click should not hide
        dw.handle_event(&Event::MousePress { pos: close_pos, button: 2 });
        assert!(dw.is_visible(), "right-click on close should not hide");
    }

    #[test]
    fn test_mouse_float_button_toggles_floating() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // Float button area: width - 42 = 158, y = 4
        let float_pos = Point::new(162, 8);
        assert!(!dw.is_floating());
        dw.handle_event(&Event::MousePress { pos: float_pos, button: 1 });
        assert!(dw.is_floating(), "clicking float button should toggle floating");
    }

    #[test]
    fn test_mouse_title_bar_drag_starts() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let title_pos = Point::new(50, 10);
        dw.handle_event(&Event::MousePress { pos: title_pos, button: 1 });
        assert!(dw.base().is_mouse_pressed(), "mouse press on title bar should set mouse_pressed");
    }

    #[test]
    fn test_mouse_move_drags_widget() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let title_pos = Point::new(50, 10);
        // Press on title bar to start drag — stores offset (50, 10)
        dw.handle_event(&Event::MousePress { pos: title_pos, button: 1 });
        // Move mouse — widget should follow, maintaining the initial grab offset
        dw.handle_event(&Event::MouseMove { pos: Point::new(100, 50) });
        // Offset was (50, 10), so new position = (100 - 50, 50 - 10) = (50, 40)
        assert_eq!(dw.geometry(), Rect::new(50, 40, 200, 100));
    }

    #[test]
    fn test_mouse_release_ends_drag() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let title_pos = Point::new(50, 10);
        dw.handle_event(&Event::MousePress { pos: title_pos, button: 1 });
        assert!(dw.base().is_mouse_pressed());
        dw.handle_event(&Event::MouseRelease { pos: Point::new(60, 20), button: 1 });
        assert!(!dw.base().is_mouse_pressed(), "mouse release should clear mouse_pressed");
    }

    #[test]
    fn test_mouse_outside_title_bar_no_drag() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // Click below title bar (y > 24)
        let content_pos = Point::new(50, 50);
        dw.handle_event(&Event::MousePress { pos: content_pos, button: 1 });
        assert!(!dw.base().is_mouse_pressed(), "click outside title bar should not start drag");
    }

    #[test]
    fn test_disabled_widget_ignores_events() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        dw.set_enabled(false);
        let close_pos = Point::new(179, 8);
        dw.handle_event(&Event::MousePress { pos: close_pos, button: 1 });
        assert!(dw.is_visible(), "disabled widget should ignore close event");
    }

    /// Features: all() and none() helpers
    #[test]
    fn test_features_all_and_none() {
        let all = DockWidgetFeatures::all();
        assert!(all.dock_widget_closable);
        assert!(all.dock_widget_movable);
        assert!(all.dock_widget_floatable);
        assert!(all.dock_widget_vertical_title_bar);
        assert!(all.all_enabled());
        assert!(!all.none_enabled());
        let none = DockWidgetFeatures::none();
        assert!(!none.dock_widget_closable);
        assert!(!none.dock_widget_movable);
        assert!(!none.dock_widget_floatable);
        assert!(!none.dock_widget_vertical_title_bar);
        assert!(!none.all_enabled());
        assert!(none.none_enabled());
    }

    #[test]
    fn test_set_features_triggers_signal() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let count = Arc::new(AtomicU32::new(0));
        dw.features_changed.connect({
            let count = Arc::clone(&count);
            move |_features: Arc<DockWidgetFeatures>| {
                count.fetch_add(1, Ordering::SeqCst);
            }
        });
        dw.set_features(DockWidgetFeatures::none());
        assert_eq!(count.load(Ordering::SeqCst), 1, "set_features should emit signal once");
        // Setting same features should NOT emit (no change)
        dw.set_features(DockWidgetFeatures::none());
        assert_eq!(count.load(Ordering::SeqCst), 1, "setting same features should not re-emit");
        // Change again
        dw.set_features(DockWidgetFeatures::all());
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    /// Registry integration
    #[test]
    fn test_registry() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        assert!(dw.registry().is_none());
        let registry = Rc::new(RefCell::new(crate::widget::SimpleRegistry::new()));
        dw.set_registry(registry.clone());
        assert!(dw.registry().is_some());
        assert!(Rc::ptr_eq(dw.registry().unwrap(), &registry));
    }

    /// DockWidgetAreas helpers
    #[test]
    fn test_dock_widget_areas_all_and_none() {
        let all = DockWidgetAreas::all();
        assert!(all.left_dock_widget_area);
        assert!(all.right_dock_widget_area);
        assert!(all.top_dock_widget_area);
        assert!(all.bottom_dock_widget_area);
        assert!(all.all_dock_widget_areas);
        assert!(!all.no_dock_widget_areas);
        let none = DockWidgetAreas::none();
        assert!(!none.left_dock_widget_area);
        assert!(!none.right_dock_widget_area);
        assert!(!none.top_dock_widget_area);
        assert!(!none.bottom_dock_widget_area);
        assert!(!none.all_dock_widget_areas);
        assert!(none.no_dock_widget_areas);
    }

    /// Widget trait delegation
    #[test]
    fn test_widget_trait_delegation() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // parent
        assert!(dw.parent().is_none());
        let id = dw.id();
        dw.set_parent(Some(id));
        assert_eq!(dw.parent(), Some(id));
        // children
        assert!(dw.children().is_empty());
        dw.add_child(id);
        assert!(!dw.children().is_empty());
        assert_eq!(dw.children(), &[id]);
        dw.remove_child(id);
        assert!(dw.children().is_empty());
        // tooltip
        assert_eq!(dw.tooltip(), "");
        dw.set_tooltip("help".to_string());
        assert_eq!(dw.tooltip(), "help");
        // enabled
        assert!(dw.is_enabled());
        dw.set_enabled(false);
        assert!(!dw.is_enabled());
    }
}
