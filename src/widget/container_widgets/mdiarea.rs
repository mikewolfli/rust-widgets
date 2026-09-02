//! MDI area widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
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
        self.base.request_redraw();
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
                self.base.request_redraw();
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
        self.base.request_redraw();
    }
    /// Returns background.
    pub fn background(&self) -> Background {
        self.background
    }
    /// Sets background.
    pub fn set_background(&mut self, background: Background) {
        self.background = background;
        self.base.request_redraw();
    }
    /// Returns activation order.
    pub fn activation_order(&self) -> ActivationOrder {
        self.activation_order
    }
    /// Sets activation order.
    pub fn set_activation_order(&mut self, order: ActivationOrder) {
        self.activation_order = order;
        self.base.request_redraw();
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
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
                context.fill_rect(rect, Color::rgb(240, 240, 240));
            }
            Background::Gradient => {
                // Draw gradient background
                for y in 0..rect.height as i32 {
                    let ratio = y as f32 / rect.height as f32;
                    let color = Color::rgb(
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
                            Color::rgb(245, 245, 245)
                        } else {
                            Color::rgb(235, 235, 235)
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
            let bg_color =
                if is_active { Color::rgb(255, 255, 255) } else { Color::rgb(250, 250, 250) };
            context.fill_rect(frame_rect, bg_color);
            // Draw frame border
            let border_color =
                if is_active { Color::rgb(0, 120, 215) } else { Color::rgb(200, 200, 200) };
            context.draw_rect(frame_rect, border_color);
            // Draw title bar
            let title_bar_height = 24;
            let title_bar_color =
                if is_active { Color::rgb(0, 120, 215) } else { Color::rgb(180, 180, 180) };
            context.fill_rect(
                Rect::new(frame_rect.x, frame_rect.y, frame_rect.width, title_bar_height as u32),
                title_bar_color,
            );
            // Draw title text
            let text_color =
                if is_active { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) };
            context.draw_text(
                Point::new(frame_rect.x + 5, frame_rect.y + title_bar_height / 2),
                &subwindow.title,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );
            // Draw close button if closable
            if subwindow.closable {
                let close_size = 12;
                let close_x = frame_rect.x + frame_rect.width as i32 - close_size - 5;
                let close_y = frame_rect.y + (title_bar_height - close_size) / 2;
                let close_color =
                    if is_active { Color::rgb(255, 255, 255) } else { Color::rgb(100, 100, 100) };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect};
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;

    // ── Helper constants ──────────────────────────────────────────────────

    fn widget_id_1() -> ObjectId {
        9201
    }
    fn widget_id_2() -> ObjectId {
        9202
    }
    fn widget_id_3() -> ObjectId {
        9203
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────

    #[test]
    fn mdiarea_creation_defaults() {
        let area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert_eq!(area.sub_window_count(), 0, "should have no sub-windows");
        assert_eq!(area.active_sub_window(), None, "no active sub-window");
        assert_eq!(area.view_mode(), ViewMode::SubWindowView, "default view mode");
        assert_eq!(area.background(), Background::Plain, "default background");
        assert_eq!(
            area.activation_order(),
            ActivationOrder::StackingOrder,
            "default activation order"
        );
        assert!(area.is_visible(), "should be visible");
        assert!(area.is_enabled(), "should be enabled");
        assert_eq!(area.geometry(), Rect::new(0, 0, 600, 400));
        assert_eq!(area.kind(), WidgetKind::MdiArea);
        assert!(area.registry().is_none(), "registry should be None by default");
    }

    // ── 2. Adding sub-windows ─────────────────────────────────────────────

    #[test]
    fn mdiarea_add_sub_window() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        let idx = area.add_sub_window(widget_id_1(), Rect::new(10, 10, 200, 150));
        assert_eq!(idx, 0, "first sub-window index should be 0");
        assert_eq!(area.sub_window_count(), 1);
        assert_eq!(
            area.active_sub_window(),
            Some(widget_id_1()),
            "first sub-window becomes active"
        );

        let idx2 = area.add_sub_window(widget_id_2(), Rect::new(50, 50, 300, 200));
        assert_eq!(idx2, 1);
        assert_eq!(area.sub_window_count(), 2);

        // Children should include both widget IDs
        let children = area.children();
        assert!(children.contains(&widget_id_1()));
        assert!(children.contains(&widget_id_2()));
    }

    #[test]
    fn mdiarea_add_sub_window_sets_z_order() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));

        if let Some(sw) = area.sub_window(0) {
            assert_eq!(sw.z_order(), 0);
        }
        if let Some(sw) = area.sub_window(1) {
            assert_eq!(sw.z_order(), 1);
        }
    }

    // ── 3. Cascading / tiling ─────────────────────────────────────────────

    #[test]
    fn mdiarea_cascade_sub_windows() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 200, 150));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 200, 150));
        area.add_sub_window(widget_id_3(), Rect::new(0, 0, 200, 150));

        area.cascade_sub_windows();

        // Each sub-window is offset by 30px
        if let Some(sw) = area.sub_window(0) {
            assert_eq!(sw.geometry().x, 0);
            assert_eq!(sw.geometry().y, 0);
        }
        if let Some(sw) = area.sub_window(1) {
            assert_eq!(sw.geometry().x, 30);
            assert_eq!(sw.geometry().y, 30);
        }
        if let Some(sw) = area.sub_window(2) {
            assert_eq!(sw.geometry().x, 60);
            assert_eq!(sw.geometry().y, 60);
        }
    }

    #[test]
    fn mdiarea_cascade_empty_is_noop() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.cascade_sub_windows(); // should not panic
        assert_eq!(area.sub_window_count(), 0);
    }

    #[test]
    fn mdiarea_tile_sub_windows() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_3(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100)); // 4th

        area.tile_sub_windows();

        // 4 items -> 2 cols, 2 rows; cell width = 400/2 = 200, cell height = 300/2 = 150
        if let Some(sw) = area.sub_window(1) {
            assert_eq!(sw.geometry().x, 200, "item 1 should be in column 1");
            assert_eq!(sw.geometry().y, 0);
        }
        if let Some(sw) = area.sub_window(2) {
            assert_eq!(sw.geometry().x, 0, "item 2 should be in column 0");
            assert_eq!(sw.geometry().y, 150, "item 2 should be in row 1");
        }
        if let Some(sw) = area.sub_window(3) {
            assert_eq!(sw.geometry().x, 200);
            assert_eq!(sw.geometry().y, 150);
        }
    }

    #[test]
    fn mdiarea_tile_empty_is_noop() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.tile_sub_windows(); // should not panic
        assert_eq!(area.sub_window_count(), 0);
    }

    // ── 4. Removing sub-windows ───────────────────────────────────────────

    #[test]
    fn mdiarea_remove_sub_window() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 200, 150));
        area.add_sub_window(widget_id_2(), Rect::new(50, 50, 300, 200));

        area.remove_sub_window(widget_id_1());
        assert_eq!(area.sub_window_count(), 1);
        assert!(!area.children().contains(&widget_id_1()));
        // The remaining one becomes active
        assert_eq!(area.active_sub_window(), Some(widget_id_2()));
    }

    #[test]
    fn mdiarea_remove_sub_window_nonexistent_is_noop() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.remove_sub_window(9999); // not present
        assert_eq!(area.sub_window_count(), 1);
    }

    #[test]
    fn mdiarea_remove_last_sub_window_clears_active() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.remove_sub_window(widget_id_1());
        assert_eq!(area.sub_window_count(), 0);
        assert_eq!(area.active_sub_window(), None);
    }

    #[test]
    fn mdiarea_remove_sub_window_adjusts_active_index() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_3(), Rect::new(0, 0, 100, 100));
        area.set_active_sub_window(widget_id_3()); // active index = 2

        // Remove first sub-window (index 0 < active index 2) -> active shifts to 1
        area.remove_sub_window(widget_id_1());
        assert_eq!(area.active_sub_window(), Some(widget_id_3()), "active widget should stay same");

        // Now remove the widget at the new active index (widget_id_3 is now at index 1)
        area.remove_sub_window(widget_id_3());
        // active becomes 0 (widget_id_2)
        assert_eq!(area.active_sub_window(), Some(widget_id_2()));
    }

    // ── 5. Active sub-window management ───────────────────────────────────

    #[test]
    fn mdiarea_set_active_sub_window() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));

        assert_eq!(area.active_sub_window(), Some(widget_id_1()), "first added is active");

        area.set_active_sub_window(widget_id_2());
        assert_eq!(area.active_sub_window(), Some(widget_id_2()));

        // Activating non-existent widget is no-op
        area.set_active_sub_window(9999);
        assert_eq!(area.active_sub_window(), Some(widget_id_2()), "should not change");
    }

    #[test]
    fn mdiarea_activate_next_previous_sub_window() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_3(), Rect::new(0, 0, 100, 100));

        assert_eq!(area.active_sub_window(), Some(widget_id_1()));

        area.activate_next_sub_window();
        assert_eq!(area.active_sub_window(), Some(widget_id_2()));

        area.activate_next_sub_window();
        assert_eq!(area.active_sub_window(), Some(widget_id_3()));

        // Wrap around
        area.activate_next_sub_window();
        assert_eq!(area.active_sub_window(), Some(widget_id_1()));

        // Previous
        area.activate_previous_sub_window();
        assert_eq!(area.active_sub_window(), Some(widget_id_3()));

        area.activate_previous_sub_window();
        assert_eq!(area.active_sub_window(), Some(widget_id_2()));
    }

    #[test]
    fn mdiarea_activate_next_previous_empty_is_noop() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.activate_next_sub_window();
        area.activate_previous_sub_window();
        assert_eq!(area.active_sub_window(), None);
    }

    // ── 6. Geometry delegation ────────────────────────────────────────────

    #[test]
    fn mdiarea_geometry_delegation() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert_eq!(area.geometry(), Rect::new(0, 0, 600, 400));

        area.set_geometry(Rect::new(10, 20, 800, 600));
        assert_eq!(area.geometry(), Rect::new(10, 20, 800, 600));
        assert_eq!(area.base().geometry(), area.geometry());
    }

    #[test]
    fn mdiarea_position_and_size() {
        let mut area = MdiArea::new(Rect::new(5, 10, 600, 400));
        assert_eq!(area.position(), Point::new(5, 10));
        assert_eq!(area.size(), crate::core::Size::new(600, 400));

        area.set_position(Point::new(20, 30));
        assert_eq!(area.geometry(), Rect::new(20, 30, 600, 400));

        area.set_size(crate::core::Size::new(800, 600));
        assert_eq!(area.geometry(), Rect::new(20, 30, 800, 600));
    }

    // ── 7. Visibility ─────────────────────────────────────────────────────

    #[test]
    fn mdiarea_visibility() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert!(area.is_visible());

        area.hide();
        assert!(!area.is_visible());

        area.show();
        assert!(area.is_visible());

        area.set_visible(false);
        assert!(!area.is_visible());

        area.set_visible(true);
        assert!(area.is_visible());
    }

    #[test]
    fn mdiarea_enabled_state() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert!(area.is_enabled());

        area.set_enabled(false);
        assert!(!area.is_enabled());

        area.set_enabled(true);
        assert!(area.is_enabled());
    }

    // ── 8. ID / Kind ──────────────────────────────────────────────────────

    #[test]
    fn mdiarea_kind() {
        let area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert_eq!(area.kind(), WidgetKind::MdiArea);
    }

    #[test]
    fn mdiarea_id_is_unique() {
        let a1 = MdiArea::new(Rect::new(0, 0, 100, 100));
        let a2 = MdiArea::new(Rect::new(0, 0, 100, 100));
        assert_ne!(a1.id(), a2.id(), "each MdiArea must have a unique id");
    }

    // ── 9. SVG draw output ────────────────────────────────────────────────

    #[test]
    fn mdiarea_draw_produces_svg_output() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        let svg = render_to_svg(&mut area);
        assert!(svg.starts_with("<svg"), "SVG must start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG must end with </svg>");
        assert!(svg.contains("width=\"400\""), "SVG must contain correct width");
        assert!(svg.contains("height=\"300\""), "SVG must contain correct height");
        assert!(svg.contains("fill="), "SVG should contain fill attributes");
        assert!(svg.len() > 100, "SVG output should be substantial");
    }

    #[test]
    fn mdiarea_draw_with_subwindow_produces_svg() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(20, 30, 200, 150));
        area.set_background(Background::Gradient);
        let svg = render_to_svg(&mut area);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("fill="));
        assert!(svg.len() > 200);
    }

    #[test]
    fn mdiarea_draw_with_pattern_background_produces_svg() {
        let mut area = MdiArea::new(Rect::new(0, 0, 60, 60));
        area.set_background(Background::Pattern);
        let svg = render_to_svg(&mut area);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("fill="));
    }

    #[test]
    fn mdiarea_draw_no_background_produces_svg() {
        let mut area = MdiArea::new(Rect::new(0, 0, 100, 80));
        area.set_background(Background::NoBackground);
        let svg = render_to_svg(&mut area);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 50);
    }

    // ── 10. Signal accessors ─────────────────────────────────────────────

    #[test]
    fn mdiarea_subwindow_activated_signal_emits_on_set() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));

        let emitted = Arc::new(AtomicUsize::new(0));
        let last_id = Arc::new(AtomicU64::new(0));
        {
            let e = emitted.clone();
            let lid = last_id.clone();
            area.subwindow_activated.connect(move |id: Arc<ObjectId>| {
                e.fetch_add(1, Ordering::SeqCst);
                lid.store(*id, Ordering::SeqCst);
            });
        }

        // Emitted once on add (first sub-window becomes active)
        assert_eq!(emitted.load(Ordering::SeqCst), 0, "signal not connected during add");

        // Connect first, then set active
        let emitted2 = Arc::new(AtomicUsize::new(0));
        let e2 = emitted2.clone();
        area.subwindow_activated.connect(move |_: Arc<ObjectId>| {
            e2.fetch_add(1, Ordering::SeqCst);
        });

        area.set_active_sub_window(widget_id_2());
        assert_eq!(emitted2.load(Ordering::SeqCst), 1, "signal should emit on set_active");

        // Setting same value does not emit
        area.set_active_sub_window(widget_id_2());
        assert_eq!(emitted2.load(Ordering::SeqCst), 1, "same value should not emit");
    }

    #[test]
    fn mdiarea_subwindow_activated_on_remove_and_next() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));

        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        area.subwindow_activated.connect(move |_: Arc<ObjectId>| {
            h.fetch_add(1, Ordering::SeqCst);
        });

        // Removing the active sub-window triggers activation of another
        let before = hits.load(Ordering::SeqCst);
        area.remove_sub_window(widget_id_1());
        assert!(
            hits.load(Ordering::SeqCst) > before,
            "removing active sub-window should emit activation signal"
        );
    }

    // ── View mode / Background / Activation order setters ─────────────────

    #[test]
    fn mdiarea_view_mode_enum() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert_eq!(area.view_mode(), ViewMode::SubWindowView);

        area.set_view_mode(ViewMode::TabbedView);
        assert_eq!(area.view_mode(), ViewMode::TabbedView);

        area.set_view_mode(ViewMode::SubWindowView);
        assert_eq!(area.view_mode(), ViewMode::SubWindowView);
    }

    #[test]
    fn mdiarea_background_enum() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert_eq!(area.background(), Background::Plain);

        area.set_background(Background::NoBackground);
        assert_eq!(area.background(), Background::NoBackground);

        area.set_background(Background::Gradient);
        assert_eq!(area.background(), Background::Gradient);

        area.set_background(Background::Pattern);
        assert_eq!(area.background(), Background::Pattern);
    }

    #[test]
    fn mdiarea_activation_order_enum() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert_eq!(area.activation_order(), ActivationOrder::StackingOrder);

        area.set_activation_order(ActivationOrder::CreationOrder);
        assert_eq!(area.activation_order(), ActivationOrder::CreationOrder);

        area.set_activation_order(ActivationOrder::HistoryOrder);
        assert_eq!(area.activation_order(), ActivationOrder::HistoryOrder);
    }

    // ── Sub-window property access ────────────────────────────────────────

    #[test]
    fn mdiarea_sub_window_accessors() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(10, 20, 200, 150));

        if let Some(sw) = area.sub_window(0) {
            assert_eq!(sw.widget(), widget_id_1());
            assert_eq!(sw.geometry(), Rect::new(10, 20, 200, 150));
            assert_eq!(sw.title(), "");
            assert!(sw.is_closable());
            assert!(sw.is_movable());
            assert!(sw.is_resizable());
            assert!(!sw.is_minimized());
            assert!(!sw.is_maximized());
            assert_eq!(sw.z_order(), 0);
        }

        // Mutable access
        if let Some(sw) = area.sub_window_mut(0) {
            sw.set_title("Doc".to_string());
            sw.set_minimized(true);
            sw.set_closable(false);
            sw.set_z_order(42);
        }

        if let Some(sw) = area.sub_window(0) {
            assert_eq!(sw.title(), "Doc");
            assert!(sw.is_minimized());
            assert!(!sw.is_closable());
            assert_eq!(sw.z_order(), 42);
        }

        // Out of bounds
        assert!(area.sub_window(5).is_none());
        assert!(area.sub_window_mut(5).is_none());
    }

    #[test]
    fn mdiarea_sub_window_set_geometry() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));

        if let Some(sw) = area.sub_window_mut(0) {
            sw.set_geometry(Rect::new(50, 60, 300, 200));
        }

        if let Some(sw) = area.sub_window(0) {
            assert_eq!(sw.geometry(), Rect::new(50, 60, 300, 200));
        }
    }

    // ── Mouse event activates sub-window ─────────────────────────────────

    #[test]
    fn mdiarea_mouse_click_activates_sub_window() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 200, 150));
        area.add_sub_window(widget_id_2(), Rect::new(100, 100, 200, 150));

        // First sub-window is active by default
        assert_eq!(area.active_sub_window(), Some(widget_id_1()));

        // Click on second sub-window area
        area.handle_event(&Event::MousePress { pos: Point::new(150, 150), button: 1 });
        assert_eq!(area.active_sub_window(), Some(widget_id_2()));

        // Click back on first sub-window
        area.handle_event(&Event::MousePress { pos: Point::new(20, 20), button: 1 });
        assert_eq!(area.active_sub_window(), Some(widget_id_1()));
    }

    #[test]
    fn mdiarea_mouse_click_outside_sub_window_is_noop() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 200, 150));

        assert_eq!(area.active_sub_window(), Some(widget_id_1()));

        // Click outside all sub-windows
        area.handle_event(&Event::MousePress { pos: Point::new(500, 300), button: 1 });
        assert_eq!(area.active_sub_window(), Some(widget_id_1()), "should not change");
    }

    // ── Arrange icons ─────────────────────────────────────────────────────

    #[test]
    fn mdiarea_arrange_icons() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 200, 150));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 200, 150));

        // Mark both as minimized
        if let Some(sw) = area.sub_window_mut(0) {
            sw.set_minimized(true);
        }
        if let Some(sw) = area.sub_window_mut(1) {
            sw.set_minimized(true);
        }

        area.arrange_icons();
        // Both should now be placed at the bottom of the area
        if let Some(sw) = area.sub_window(0) {
            assert_eq!(sw.geometry().width, 100);
            assert_eq!(sw.geometry().height, 80);
            // Positioned at bottom
            assert_eq!(sw.geometry().y, 300 - 80 - 10);
        }
    }

    #[test]
    fn mdiarea_arrange_icons_empty_is_noop() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.arrange_icons(); // should not panic
        assert_eq!(area.sub_window_count(), 0);
    }
}
