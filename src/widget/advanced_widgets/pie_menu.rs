//! PieMenu (radial/圆形菜单) widget.
//!
//! A circular popup menu that displays items as radial slices.
//! Users hover to highlight and click to select an item.

use std::f32::consts::TAU;

use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A single item in a `PieMenu`.
#[derive(Debug, Clone)]
pub struct PieMenuItem {
    pub text: String,
    pub icon_text: String,
    pub enabled: bool,
    pub angle_start: f32,
    pub angle_end: f32,
}

/// PieMenu (radial/circular menu) widget.
///
/// Displays items arranged radially around a center point. The menu
/// appears as a donut-like ring with labelled slices. Supports hover
/// highlighting, click selection, and keyboard dismissal.
#[allow(dead_code)]
pub struct PieMenu {
    base: BaseWidget,
    items: Vec<PieMenuItem>,
    radius: f32,
    inner_radius: f32,
    hovered_index: Option<usize>,
    center: Point,
    animation_progress: f32,
    pub background_color: Color,
    hover_color: Color,
    text_color: Color,
    pub triggered: Signal1<usize>,
    pub triggered_text: Signal1<String>,
    pub about_to_show: GenericSignal,
    pub about_to_hide: GenericSignal,
}

impl PieMenu {
    /// Creates a new `PieMenu` centered at `center` with the given outer `radius`.
    pub fn new(center: Point, radius: f32) -> Self {
        let size = (radius * 2.0) as u32;
        let geometry = Rect::new(
            center.x - radius as i32,
            center.y - radius as i32,
            size,
            size,
        );
        let inner_radius = radius * 0.35;
        Self {
            base: BaseWidget::new(WidgetKind::PieMenu, geometry, "PieMenu"),
            items: Vec::new(),
            radius,
            inner_radius,
            hovered_index: None,
            center,
            animation_progress: 1.0,
            background_color: Color::from_rgb(245, 245, 245),
            hover_color: Color::from_rgb(0, 120, 215),
            text_color: Color::from_rgb(30, 30, 30),
            triggered: Signal1::new(),
            triggered_text: Signal1::new(),
            about_to_show: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
        }
    }

    /// Adds a menu item and returns its index.
    pub fn add_item(&mut self, text: impl Into<String>) -> usize {
        self.add_item_with_icon(text, "")
    }

    /// Adds a menu item with an icon text and returns its index.
    pub fn add_item_with_icon(
        &mut self,
        text: impl Into<String>,
        icon: impl Into<String>,
    ) -> usize {
        let idx = self.items.len();
        self.items.push(PieMenuItem {
            text: text.into(),
            icon_text: icon.into(),
            enabled: true,
            angle_start: 0.0,
            angle_end: 0.0,
        });
        self.recalculate_angles();
        idx
    }

    /// Inserts a menu item at the given index.
    pub fn insert_item(&mut self, index: usize, text: impl Into<String>) {
        let idx = index.min(self.items.len());
        self.items.insert(
            idx,
            PieMenuItem {
                text: text.into(),
                icon_text: String::new(),
                enabled: true,
                angle_start: 0.0,
                angle_end: 0.0,
            },
        );
        self.recalculate_angles();
    }

    /// Removes the menu item at `index`.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            self.recalculate_angles();
        }
    }

    /// Removes all menu items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.hovered_index = None;
    }

    /// Returns the number of items in the menu.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns a slice of all menu items.
    pub fn items(&self) -> &[PieMenuItem] {
        &self.items
    }

    /// Sets whether the item at `index` is enabled.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.enabled = enabled;
        }
    }

    /// Returns the outer radius of the menu.
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Sets the outer radius of the menu.
    pub fn set_radius(&mut self, radius: f32) {
        self.radius = radius.max(10.0);
        self.inner_radius = self.inner_radius.min(self.radius * 0.9);
        self.update_geometry();
    }

    /// Returns the inner (donut hole) radius.
    pub fn inner_radius(&self) -> f32 {
        self.inner_radius
    }

    /// Sets the inner (donut hole) radius.
    pub fn set_inner_radius(&mut self, inner_radius: f32) {
        self.inner_radius = inner_radius.max(2.0).min(self.radius * 0.95);
        self.update_geometry();
    }

    /// Returns the center point of the menu.
    pub fn center(&self) -> Point {
        self.center
    }

    /// Sets the center point of the menu.
    pub fn set_center(&mut self, center: Point) {
        self.center = center;
        self.update_geometry();
    }

    /// Shows the menu at the given center position.
    pub fn show_at(&mut self, center: Point) {
        self.center = center;
        self.update_geometry();
        self.hovered_index = None;
        self.about_to_show.emit();
        self.base.show();
    }

    /// Hides the menu.
    pub fn hide(&mut self) {
        self.base.hide();
        self.hovered_index = None;
        self.about_to_hide.emit();
    }

    // ── Private helpers ──────────────────────────────────────────

    /// Recalculates the start/end angles for every item based on equal division.
    fn recalculate_angles(&mut self) {
        let count = self.items.len();
        if count == 0 {
            return;
        }
        let slice = TAU / count as f32;
        for (i, item) in self.items.iter_mut().enumerate() {
            item.angle_start = i as f32 * slice;
            item.angle_end = (i as f32 + 1.0) * slice;
        }
    }

    /// Updates the widget geometry to match the current center and radius.
    fn update_geometry(&mut self) {
        let size = (self.radius * 2.0) as u32;
        self.base.set_geometry(Rect::new(
            self.center.x - self.radius as i32,
            self.center.y - self.radius as i32,
            size,
            size,
        ));
    }

    /// Returns the index of the slice at the given position, or `None`.
    fn hit_test(&self, pos: Point) -> Option<usize> {
        let dx = (pos.x - self.center.x) as f32;
        let dy = (pos.y - self.center.y) as f32;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < self.inner_radius || dist > self.radius {
            return None;
        }
        let mut angle = dy.atan2(dx);
        if angle < 0.0 {
            angle += TAU;
        }
        for (i, item) in self.items.iter().enumerate() {
            if angle >= item.angle_start && angle < item.angle_end {
                if item.enabled {
                    return Some(i);
                }
                return None;
            }
        }
        None
    }

    /// Fills a pie slice wedge by drawing dense radial lines.
    fn fill_slice(
        &self,
        context: &mut RenderContext,
        center: Point,
        outer_r: f32,
        inner_r: f32,
        angle_start: f32,
        angle_end: f32,
        color: Color,
    ) {
        let cx = center.x as f32;
        let cy = center.y as f32;
        let delta_angle = angle_end - angle_start;

        // Number of radial strips to approximate the fill
        let strips = ((outer_r - inner_r) * 0.5).max(4.0).min(30.0) as u32;
        let strip_count = strips.max(4);

        for i in 0..strip_count {
            let frac = i as f32 / strip_count as f32;
            let r = inner_r + frac * (outer_r - inner_r);

            let sub_segments = (r * delta_angle * 0.25).max(4.0).min(20.0) as u32;
            let sub_segments = sub_segments.max(3).min(20);
            let step_a = delta_angle / sub_segments as f32;

            for j in 0..sub_segments {
                let a1 = angle_start + j as f32 * step_a;
                let a2 = angle_start + (j + 1) as f32 * step_a;
                context.draw_line_stroke(
                    Point::from_f32(cx + r * a1.cos(), cy + r * a1.sin()),
                    Point::from_f32(cx + r * a2.cos(), cy + r * a2.sin()),
                    color,
                    1,
                );
            }
        }

        // Side edges
        let inner_start = Point::from_f32(
            cx + inner_r * angle_start.cos(),
            cy + inner_r * angle_start.sin(),
        );
        let outer_start = Point::from_f32(
            cx + outer_r * angle_start.cos(),
            cy + outer_r * angle_start.sin(),
        );
        let inner_end = Point::from_f32(
            cx + inner_r * angle_end.cos(),
            cy + inner_r * angle_end.sin(),
        );
        let outer_end = Point::from_f32(
            cx + outer_r * angle_end.cos(),
            cy + outer_r * angle_end.sin(),
        );
        context.draw_line_stroke(inner_start, outer_start, color, 1);
        context.draw_line_stroke(inner_end, outer_end, color, 1);
    }

    /// Returns a colour for the slice at index `i`, cycling through a pleasant palette.
    fn slice_color(&self, i: usize) -> Color {
        const PALETTE: &[Color] = &[
            Color::from_rgb(173, 216, 230), // light blue
            Color::from_rgb(255, 182, 193), // light pink
            Color::from_rgb(152, 251, 152), // pale green
            Color::from_rgb(255, 218, 185), // peach
            Color::from_rgb(216, 191, 216), // thistle
            Color::from_rgb(255, 228, 181), // moccasin
            Color::from_rgb(175, 238, 238), // turquoise
            Color::from_rgb(255, 239, 213), // papaya whip
            Color::from_rgb(221, 160, 221), // plum
            Color::from_rgb(176, 224, 230), // powder blue
            Color::from_rgb(240, 230, 140), // khaki
            Color::from_rgb(255, 192, 203), // pink
        ];
        PALETTE[i % PALETTE.len()]
    }
}

impl Widget for PieMenu {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.about_to_show.emit();
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
        self.hovered_index = None;
        self.about_to_hide.emit();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
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

impl EventHandler for PieMenu {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.base.is_visible() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                self.hovered_index = self.hit_test(*pos);
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(idx) = self.hit_test(*pos) {
                    if let Some(item) = self.items.get(idx) {
                        if item.enabled {
                            let text = item.text.clone();
                            self.triggered.emit(idx);
                            self.triggered_text.emit(text);
                            self.hide();
                        }
                    }
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == 27 {
                    // Escape
                    self.hide();
                }
            }
            _ => {}
        }
    }
}

impl Draw for PieMenu {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.is_visible() || self.items.is_empty() {
            return;
        }

        let center = self.center;
        let outer_r = self.radius;
        let inner_r = self.inner_radius;
        let cx = center.x as f32;
        let cy = center.y as f32;

        // Draw each slice
        for (i, item) in self.items.iter().enumerate() {
            let is_hovered = self.hovered_index == Some(i);
            let base_color = if !item.enabled {
                Color::from_rgb(220, 220, 220)
            } else if is_hovered {
                self.hover_color
            } else {
                self.slice_color(i)
            };

            self.fill_slice(
                context,
                center,
                outer_r,
                inner_r,
                item.angle_start,
                item.angle_end,
                base_color,
            );
        }

        // Draw separator lines between slices (radial lines)
        for item in self.items.iter() {
            context.draw_line_stroke(
                Point::from_f32(
                    cx + inner_r * item.angle_start.cos(),
                    cy + inner_r * item.angle_start.sin(),
                ),
                Point::from_f32(
                    cx + outer_r * item.angle_start.cos(),
                    cy + outer_r * item.angle_start.sin(),
                ),
                Color::from_rgb(160, 160, 160),
                1,
            );
        }

        // Draw the outer ring border
        context.draw_circle_stroke(center, outer_r as u32, Color::from_rgb(140, 140, 140), 1);

        // Draw the inner donut hole circle
        context.fill_circle(center, inner_r as u32, Color::from_rgb(250, 250, 250));
        context.draw_circle_stroke(center, inner_r as u32, Color::from_rgb(180, 180, 180), 1);

        // Draw text labels centered in each slice
        let font = Font::default();
        for (i, item) in self.items.iter().enumerate() {
            if !item.enabled {
                continue;
            }
            let mid_angle = (item.angle_start + item.angle_end) * 0.5;
            let label_r = (outer_r + inner_r) * 0.5;
            let lx = cx + label_r * mid_angle.cos();
            let ly = cy + label_r * mid_angle.sin();

            let label_text = if item.icon_text.is_empty() {
                &item.text
            } else {
                &item.icon_text
            };

            let text_color = if self.hovered_index == Some(i) {
                Color::WHITE
            } else {
                self.text_color
            };
            context.draw_text(Point::from_f32(lx, ly), label_text, &font, text_color);
        }

        // Draw a small center dot
        context.fill_circle(center, 3, Color::from_rgb(100, 100, 100));
    }
}
