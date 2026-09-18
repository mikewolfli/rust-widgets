// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PieMenu (radial menu) widget.
//!
//! A circular popup menu that displays items as radial slices.
//! Users hover to highlight and click to select an item.

use std::f32::consts::TAU;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_f32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A single item in a `PieMenu`.
#[derive(Debug, Clone)]
pub struct PieMenuItem {
    text: String,
    icon_text: String,
    enabled: bool,
    angle_start: f32,
    angle_end: f32,
}

impl PieMenuItem {
    /// Creates an enabled item with the given label, no icon text, and its
    /// angles unset (`0.0` for both).
    ///
    /// The angles are placeholders until the item is inserted into a
    /// [`PieMenu`], which recomputes them to divide the circle evenly among all
    /// items.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            icon_text: String::new(),
            enabled: true,
            angle_start: 0.0,
            angle_end: 0.0,
        }
    }

    /// Returns the item's label.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the item's label. The slice angle is not affected, and no
    /// redraw is requested.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Returns the icon text, or an empty string when the item has no icon.
    ///
    /// This is a text stand-in for an icon, not image data.
    pub fn icon_text(&self) -> &str {
        &self.icon_text
    }

    /// Replaces the icon text. An empty string means "no icon".
    pub fn set_icon_text(&mut self, icon: impl Into<String>) {
        self.icon_text = icon.into();
    }

    /// Returns whether the item can be clicked. Defaults to `true`.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the item. A disabled item is still drawn but is
    /// skipped by hit testing, so hovering it clears the hover highlight rather
    /// than selecting it.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns the slice's start angle, in radians measured clockwise from the
    /// positive x axis (see the module drawing code), with `0.0` at the right
    /// of the menu centre.
    ///
    /// Managed by [`PieMenu`]; setting it directly is only meaningful for a
    /// standalone item.
    pub fn angle_start(&self) -> f32 {
        self.angle_start
    }

    /// Sets the slice's start angle in radians. Not validated, and the menu
    /// overwrites it whenever the item list changes.
    pub fn set_angle_start(&mut self, angle: f32) {
        self.angle_start = angle;
    }

    /// Returns the slice's exclusive end angle, in radians. The slice spans
    /// `angle_start .. angle_end`.
    pub fn angle_end(&self) -> f32 {
        self.angle_end
    }

    /// Sets the slice's exclusive end angle in radians. Not validated.
    pub fn set_angle_end(&mut self, angle: f32) {
        self.angle_end = angle;
    }
}

/// PieMenu (radial/circular menu) widget.
///
/// Displays items arranged radially around a center point. The menu
/// appears as a donut-like ring with labelled slices. Supports hover
/// highlighting, click selection, and keyboard dismissal.
pub struct PieMenu {
    base: BaseWidget,
    items: Vec<PieMenuItem>,
    radius: f32,
    inner_radius: f32,
    hovered_index: Option<usize>,
    current_index: usize,
    center: Point,
    animation_progress: f32,
    hover_color: Color,
    text_color: Color,
    /// Emitted with the index whose selection was applied. Fires from user
    /// clicks and from [`PieMenu::set_current_index`] — so programmatic
    /// selection is indistinguishable from a click, and a slot that reacts by
    /// calling `set_current_index` again will recurse. A
    /// [`PieMenu::set_current_index`] call with an out-of-range index emits
    /// nothing.
    pub triggered: Signal1<usize>,
    /// Emitted with the selected item's label, alongside `triggered`. Empty
    /// labels produce an empty payload; duplicate labels are indistinguishable.
    pub triggered_text: Signal1<String>,
    /// Emitted by [`PieMenu::show_at`], just before the menu becomes visible.
    pub about_to_show: GenericSignal,
    /// Emitted by [`PieMenu::hide`], after the menu is hidden and the hover
    /// highlight cleared.
    pub about_to_hide: GenericSignal,
}

impl PieMenu {
    /// Creates a new `PieMenu` centered at `center` with the given outer `radius`.
    pub fn new(center: Point, radius: f32) -> Self {
        let size = (radius * 2.0) as u32;
        let geometry = Rect::new(center.x - radius as i32, center.y - radius as i32, size, size);
        let inner_radius = radius * 0.35;
        Self {
            base: BaseWidget::new(WidgetKind::PieMenu, geometry, "PieMenu"),
            items: Vec::new(),
            radius,
            inner_radius,
            hovered_index: None,
            current_index: 0,
            center,
            animation_progress: 1.0,
            hover_color: Color::rgb(0, 120, 215),
            text_color: Color::rgb(30, 30, 30),
            triggered: Signal1::new(),
            triggered_text: Signal1::new(),
            about_to_show: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
        }
    }

    /// Returns the current (last selected) index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Sets the current index with bounds checking.
    ///
    /// An out-of-range index is silently ignored. On success this emits
    /// `triggered` and `triggered_text` and requests a redraw; the current
    /// index is **not** updated for an item that is disabled.
    pub fn set_current_index(&mut self, idx: usize) {
        if idx < self.items.len() {
            // Mirror the click/hit-test paths: a disabled item is not selectable,
            // so the index is left untouched and no signal fires.
            if !self.items[idx].is_enabled() {
                return;
            }
            self.current_index = idx;
            self.triggered.emit(idx);
            if let Some(text) = self.items.get(idx).map(|item| item.text().to_string()) {
                self.triggered_text.emit(text);
            }
            self.base.request_redraw();
        }
    }

    /// Adds a menu item and returns its index.
    pub fn add_item(&mut self, text: impl Into<String>) -> usize {
        self.add_item_with_icon(text, "")
    }

    /// Adds an item with a text icon and returns its index.
    ///
    /// Angles are recomputed so all items share the circle equally, which means
    /// adding an item moves every existing slice.
    pub fn add_item_with_icon(
        &mut self,
        text: impl Into<String>,
        icon: impl Into<String>,
    ) -> usize {
        let idx = self.items.len();
        let mut item = PieMenuItem::new(text);
        item.set_icon_text(icon);
        self.items.push(item);
        self.recalculate_angles();
        idx
    }

    /// Inserts an item at `index`, or appends when `index` is past the end.
    ///
    /// The new item has no icon text. Angles are recomputed for every item, and
    /// [`PieMenu::current_index`] is not adjusted, so it can end up naming a
    /// different item than before.
    pub fn insert_item(&mut self, index: usize, text: impl Into<String>) {
        let idx = index.min(self.items.len());
        self.items.insert(idx, PieMenuItem::new(text));
        self.recalculate_angles();
    }

    /// Removes the item at `index`; out-of-range indices are ignored.
    ///
    /// Angles are recomputed for the remaining items. [`PieMenu::current_index`]
    /// and [`PieMenu::hovered_index`] are not adjusted, so they can be left
    /// pointing past the end of the list.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            self.recalculate_angles();
        }
    }

    /// Removes all menu items and clears the hover highlight.
    ///
    /// [`PieMenu::current_index`] is left as-is even though it now names no
    /// item.
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

    /// Enables or disables the item at `index`, ignoring out-of-range indices.
    /// Disabling does not clear an existing hover highlight or redraw.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.set_enabled(enabled);
        }
    }

    /// Returns the outer radius of the menu.
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Sets the outer radius of the menu.
    ///
    /// Values below `10.0` are raised to `10.0`. The inner radius is pulled
    /// down if it would otherwise reach past 90% of the new outer radius, and
    /// the widget geometry is recomputed to the enclosing square.
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
    ///
    /// Clamped into `2.0 ..= 0.95 * radius`; the widget geometry is recomputed.
    /// A radius given as NaN clamps to `2.0` in practice only if it is
    /// comparable — passing NaN leaves the previous value semantics undefined
    /// by `f32::clamp` returning NaN; use finite values.
    pub fn set_inner_radius(&mut self, inner_radius: f32) {
        self.inner_radius = inner_radius.max(2.0).min(self.radius * 0.95);
        self.update_geometry();
    }

    /// Returns the center point of the menu.
    pub fn center(&self) -> Point {
        self.center
    }

    /// Sets the center point of the menu, in parent-relative logical pixels,
    /// and recomputes the widget geometry so it is the square of side
    /// `2 * radius` centred on that point.
    pub fn set_center(&mut self, center: Point) {
        self.center = center;
        self.update_geometry();
    }

    /// Returns the animation progress (0.0 to 1.0).
    pub fn animation_progress(&self) -> f32 {
        self.animation_progress
    }

    /// Sets the animation progress, clamped to `0.0 ..= 1.0`.
    ///
    /// The value is a plain stored number: the widget never advances it itself
    /// and does not request a redraw, so an animating caller must step it and
    /// repaint. `1.0` (fully shown) is the initial value.
    pub fn set_animation_progress(&mut self, progress: f32) {
        self.animation_progress = progress.clamp(0.0, 1.0);
    }

    /// Returns the hover highlight color.
    pub fn hover_color(&self) -> Color {
        self.hover_color
    }

    /// Sets the hover highlight color. Does not request a redraw.
    pub fn set_hover_color(&mut self, color: Color) {
        self.hover_color = color;
    }

    /// Returns the text color for labels.
    pub fn text_color(&self) -> Color {
        self.text_color
    }

    /// Sets the text color for labels. Does not request a redraw.
    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
    }

    /// Returns the currently hovered item index, if any.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// Shows the menu centred on `center` (parent-relative logical pixels).
    ///
    /// Clears the hover highlight, emits `about_to_show`, then makes the widget
    /// visible. The radius and item list are unchanged.
    pub fn show_at(&mut self, center: Point) {
        self.center = center;
        self.update_geometry();
        self.hovered_index = None;
        self.about_to_show.emit();
        self.base.show();
    }

    /// Hides the menu, clears the hover highlight, then emits `about_to_hide`.
    ///
    /// Note the ordering is the reverse of [`PieMenu::show_at`], which emits
    /// before showing. Emits unconditionally, even when already hidden.
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
            item.set_angle_start(i as f32 * slice);
            item.set_angle_end((i as f32 + 1.0) * slice);
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
            if angle >= item.angle_start() && angle < item.angle_end() {
                if item.is_enabled() {
                    return Some(i);
                }
                return None;
            }
        }
        None
    }

    /// Fills a pie slice wedge by drawing dense radial lines.
    #[allow(clippy::too_many_arguments)]
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
        let strips = ((outer_r - inner_r) * 0.5).clamp(4.0, 30.0) as u32;
        let strip_count = strips.max(4);

        for i in 0..strip_count {
            let frac = i as f32 / strip_count as f32;
            let r = inner_r + frac * (outer_r - inner_r);

            let sub_segments = (r * delta_angle * 0.25).clamp(4.0, 20.0) as u32;
            let sub_segments = sub_segments.clamp(3, 20);
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
        let inner_start =
            Point::from_f32(cx + inner_r * angle_start.cos(), cy + inner_r * angle_start.sin());
        let outer_start =
            Point::from_f32(cx + outer_r * angle_start.cos(), cy + outer_r * angle_start.sin());
        let inner_end =
            Point::from_f32(cx + inner_r * angle_end.cos(), cy + inner_r * angle_end.sin());
        let outer_end =
            Point::from_f32(cx + outer_r * angle_end.cos(), cy + outer_r * angle_end.sin());
        context.draw_line_stroke(inner_start, outer_start, color, 1);
        context.draw_line_stroke(inner_end, outer_end, color, 1);
    }

    /// Returns a colour for the slice at index `i`, cycling through a pleasant palette.
    fn slice_color(&self, i: usize) -> Color {
        const PALETTE: &[Color] = &[
            Color::rgb(173, 216, 230), // light blue
            Color::rgb(255, 182, 193), // light pink
            Color::rgb(152, 251, 152), // pale green
            Color::rgb(255, 218, 185), // peach
            Color::rgb(216, 191, 216), // thistle
            Color::rgb(255, 228, 181), // moccasin
            Color::rgb(175, 238, 238), // turquoise
            Color::rgb(255, 239, 213), // papaya whip
            Color::rgb(221, 160, 221), // plum
            Color::rgb(176, 224, 230), // powder blue
            Color::rgb(240, 230, 140), // khaki
            Color::rgb(255, 192, 203), // pink
        ];
        PALETTE[i % PALETTE.len()]
    }
}

impl Widget for PieMenu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(200, 200)
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
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `PieMenu`'s property contract.
///
/// All four properties are read-only projections of the menu's geometry and
/// selection state — none of them has a setter that would not fight the layout
/// that produced the radii — so there is no write arm for them and the schema
/// marks each one non-writable.
impl WidgetProperties for PieMenu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "radius" => Ok(CapabilityValue::Float(self.radius() as f64)),
            "inner_radius" => Ok(CapabilityValue::Float(self.inner_radius() as f64)),
            "current_index" => Ok(CapabilityValue::UInt(self.current_index() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "radius" => {
                self.set_radius(expect_f32(value)?);
                Ok(())
            }
            "inner_radius" => {
                self.set_inner_radius(expect_f32(value)?);
                Ok(())
            }
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            // Derived from the item vector, so there is nothing to assign.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "item_count",
            "radius",
            "inner_radius",
            "current_index",
            BASE_PROPERTY_NAMES
        ]
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
                        if item.is_enabled() {
                            let text = item.text().to_string();
                            self.triggered.emit(idx);
                            self.triggered_text.emit(text);
                            self.hide();
                        }
                    }
                }
            }
            Event::KeyPress { key, .. } if *key == 27 => {
                // Escape
                self.hide();
            }
            _ => { /* Other events are not relevant */ }
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
            let base_color = if !item.is_enabled() {
                Color::rgb(220, 220, 220)
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
                item.angle_start(),
                item.angle_end(),
                base_color,
            );
        }

        // Draw separator lines between slices (radial lines)
        for item in self.items.iter() {
            context.draw_line_stroke(
                Point::from_f32(
                    cx + inner_r * item.angle_start().cos(),
                    cy + inner_r * item.angle_start().sin(),
                ),
                Point::from_f32(
                    cx + outer_r * item.angle_start().cos(),
                    cy + outer_r * item.angle_start().sin(),
                ),
                Color::rgb(160, 160, 160),
                1,
            );
        }

        // Draw the outer ring border
        context.draw_circle_stroke(center, outer_r as u32, Color::rgb(140, 140, 140), 1);

        // Draw the inner donut hole circle
        context.fill_circle(center, inner_r as u32, Color::rgb(250, 250, 250));
        context.draw_circle_stroke(center, inner_r as u32, Color::rgb(180, 180, 180), 1);

        // Draw text labels centered in each slice
        let font = Font::default();
        for (i, item) in self.items.iter().enumerate() {
            if !item.is_enabled() {
                continue;
            }
            let mid_angle = (item.angle_start() + item.angle_end()) * 0.5;
            let label_r = (outer_r + inner_r) * 0.5;
            let lx = cx + label_r * mid_angle.cos();
            let ly = cy + label_r * mid_angle.sin();

            let label_text =
                if item.icon_text().is_empty() { item.text() } else { item.icon_text() };

            let text_color =
                if self.hovered_index == Some(i) { Color::WHITE } else { self.text_color };
            context.draw_text(
                Point::from_f32(lx, ly),
                label_text,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // Draw a small center dot
        context.fill_circle(center, 3, Color::rgb(100, 100, 100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    /// 1. Creating default PieMenu (verify kind, geometry, default state)
    #[test]
    fn test_default_creation() {
        let center = Point::new(200, 200);
        let radius = 100.0;
        let menu = PieMenu::new(center, radius);

        assert_eq!(menu.kind(), WidgetKind::PieMenu);
        assert_eq!(menu.center(), center);
        assert_eq!(menu.radius(), radius);
        assert!(menu.inner_radius() < radius);
        assert_eq!(menu.inner_radius(), radius * 0.35);
        assert_eq!(menu.current_index(), 0);
        assert_eq!(menu.hovered_index(), None);
        assert_eq!(menu.animation_progress(), 1.0);
        assert_eq!(menu.item_count(), 0);
        assert!(menu.is_visible());
        assert!(menu.is_enabled());
        assert!(menu.items().is_empty());

        // Default colors
        assert_eq!(menu.hover_color(), Color::rgb(0, 120, 215));
        assert_eq!(menu.text_color(), Color::rgb(30, 30, 30));

        // Geometry: centered at (200,200) with radius 100 => rect (100, 100, 200, 200)
        let geom = menu.geometry();
        assert_eq!(geom.x, 100);
        assert_eq!(geom.y, 100);
        assert_eq!(geom.width, 200);
        assert_eq!(geom.height, 200);
    }

    /// 2. Adding items
    #[test]
    fn test_add_item() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);

        let idx0 = menu.add_item("Cut");
        assert_eq!(idx0, 0);
        assert_eq!(menu.item_count(), 1);

        let idx1 = menu.add_item("Copy");
        assert_eq!(idx1, 1);
        assert_eq!(menu.item_count(), 2);

        let idx2 = menu.add_item_with_icon("Paste", "📋");
        assert_eq!(idx2, 2);
        assert_eq!(menu.item_count(), 3);

        // Verify items
        let items = menu.items();
        assert_eq!(items[0].text(), "Cut");
        assert_eq!(items[1].text(), "Copy");
        assert_eq!(items[2].text(), "Paste");
        assert_eq!(items[2].icon_text(), "📋");
    }

    /// 3. Inserting items at specific index
    #[test]
    fn test_insert_item_at_index() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("First");
        menu.add_item("Third");
        assert_eq!(menu.item_count(), 2);

        // Insert at middle
        menu.insert_item(1, "Second");
        assert_eq!(menu.item_count(), 3);
        assert_eq!(menu.items()[0].text(), "First");
        assert_eq!(menu.items()[1].text(), "Second");
        assert_eq!(menu.items()[2].text(), "Third");

        // Insert at beginning
        menu.insert_item(0, "Zero");
        assert_eq!(menu.items()[0].text(), "Zero");
        assert_eq!(menu.items()[1].text(), "First");

        // Insert beyond end => appends
        menu.insert_item(99, "Last");
        assert_eq!(menu.items().last().unwrap().text(), "Last");

        // Verify angles are recalculated (all items get equal slices)
        let count = menu.item_count();
        let slice = std::f32::consts::TAU / count as f32;
        for (i, item) in menu.items().iter().enumerate() {
            let expected_start = i as f32 * slice;
            let expected_end = (i as f32 + 1.0) * slice;
            assert!((item.angle_start() - expected_start).abs() < 0.001);
            assert!((item.angle_end() - expected_end).abs() < 0.001);
        }
    }

    /// 4. Removing items
    #[test]
    fn test_remove_item() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("A");
        menu.add_item("B");
        menu.add_item("C");
        assert_eq!(menu.item_count(), 3);

        // Remove middle
        menu.remove_item(1);
        assert_eq!(menu.item_count(), 2);
        assert_eq!(menu.items()[0].text(), "A");
        assert_eq!(menu.items()[1].text(), "C");

        // Remove out of bounds => no-op
        menu.remove_item(99);
        assert_eq!(menu.item_count(), 2);

        // Remove all
        menu.remove_item(1);
        menu.remove_item(0);
        assert_eq!(menu.item_count(), 0);
    }

    /// 5. Setting/getting current index
    #[test]
    fn test_current_index() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Red");
        menu.add_item("Green");
        menu.add_item("Blue");

        // Default is 0
        assert_eq!(menu.current_index(), 0);

        menu.set_current_index(1);
        assert_eq!(menu.current_index(), 1);

        menu.set_current_index(2);
        assert_eq!(menu.current_index(), 2);

        // Out of bounds => no-op
        menu.set_current_index(99);
        assert_eq!(menu.current_index(), 2);
    }

    /// 6. Item count
    #[test]
    fn test_item_count() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        assert_eq!(menu.item_count(), 0);

        menu.add_item("One");
        assert_eq!(menu.item_count(), 1);

        menu.add_item("Two");
        menu.add_item("Three");
        assert_eq!(menu.item_count(), 3);

        menu.clear();
        assert_eq!(menu.item_count(), 0);
    }

    /// 7. Setting item text and icon
    #[test]
    fn test_item_text_and_icon() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Initial");

        // Mutable access via items() slice is not directly mutable,
        // but we can use the item API through the menu.
        // Create a new item and check PieMenuItem's setters
        let mut item = PieMenuItem::new("Test");
        assert_eq!(item.text(), "Test");
        assert_eq!(item.icon_text(), "");

        item.set_text("Updated");
        assert_eq!(item.text(), "Updated");

        item.set_icon_text("🚀");
        assert_eq!(item.icon_text(), "🚀");

        // Also verify that add_item_with_icon sets icon correctly
        let mut menu2 = PieMenu::new(Point::new(50, 50), 60.0);
        menu2.add_item_with_icon("Save", "💾");
        assert_eq!(menu2.items()[0].text(), "Save");
        assert_eq!(menu2.items()[0].icon_text(), "💾");
    }

    /// 8. Enabling/disabling items
    #[test]
    fn test_enable_disable_items() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Alpha");
        menu.add_item("Beta");
        menu.add_item("Gamma");

        // All start enabled
        assert!(menu.items()[0].is_enabled());
        assert!(menu.items()[1].is_enabled());
        assert!(menu.items()[2].is_enabled());

        menu.set_item_enabled(1, false);
        assert!(menu.items()[0].is_enabled());
        assert!(!menu.items()[1].is_enabled());
        assert!(menu.items()[2].is_enabled());

        // Re-enable
        menu.set_item_enabled(1, true);
        assert!(menu.items()[1].is_enabled());

        // Out of bounds => no-op
        menu.set_item_enabled(99, false);
        assert_eq!(menu.item_count(), 3);
    }

    /// 9. Setting radius and center
    #[test]
    fn test_radius_and_center() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        assert_eq!(menu.radius(), 80.0);
        assert_eq!(menu.center(), Point::new(100, 100));

        // Set radius
        menu.set_radius(120.0);
        assert_eq!(menu.radius(), 120.0);
        // Clamped minimum
        menu.set_radius(5.0);
        assert_eq!(menu.radius(), 10.0);

        // Set center
        menu.set_center(Point::new(300, 400));
        assert_eq!(menu.center(), Point::new(300, 400));

        // Geometry updates with new center/radius
        let geom = menu.geometry();
        assert_eq!(geom.x, 290); // center.x(300) - radius(10) = 290
        assert_eq!(geom.width, 20);
        assert_eq!(geom.height, 20);
    }

    /// 10. Item visibility (via base widget delegation)
    #[test]
    fn test_item_visibility() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        assert!(menu.is_visible());

        menu.hide();
        assert!(!menu.is_visible());
        assert_eq!(menu.hovered_index(), None);

        menu.show();
        assert!(menu.is_visible());

        // show_at also makes it visible
        menu.hide();
        menu.show_at(Point::new(200, 200));
        assert!(menu.is_visible());
    }

    /// 11. Signal accessors (triggered, triggered_text, about_to_show, about_to_hide)
    #[test]
    fn test_signal_accessors() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Open");
        menu.add_item("Save");
        menu.add_item("Exit");

        // Test triggered signal via set_current_index
        let triggered = Arc::new(Mutex::new(None));
        menu.triggered.connect({
            let triggered = Arc::clone(&triggered);
            move |val: Arc<usize>| {
                *triggered.lock().unwrap() = Some(*val);
            }
        });

        menu.set_current_index(1);
        assert_eq!(*triggered.lock().unwrap(), Some(1));

        // Test triggered_text signal
        let triggered_text = Arc::new(Mutex::new(None::<String>));
        menu.triggered_text.connect({
            let triggered_text = Arc::clone(&triggered_text);
            move |val: Arc<String>| {
                *triggered_text.lock().unwrap() = Some(val.to_string());
            }
        });

        menu.set_current_index(2);
        assert_eq!(triggered_text.lock().unwrap().as_deref(), Some("Exit"));

        // Test about_to_show signal
        let show_fired = Arc::new(Mutex::new(false));
        menu.about_to_show.connect({
            let show_fired = Arc::clone(&show_fired);
            move || {
                *show_fired.lock().unwrap() = true;
            }
        });

        menu.hide();
        menu.show_at(Point::new(150, 150));
        assert!(*show_fired.lock().unwrap());

        // Test about_to_hide signal
        let hide_fired = Arc::new(Mutex::new(false));
        menu.about_to_hide.connect({
            let hide_fired = Arc::clone(&hide_fired);
            move || {
                *hide_fired.lock().unwrap() = true;
            }
        });

        menu.hide();
        assert!(*hide_fired.lock().unwrap());
    }

    /// 12. Geometry delegation
    #[test]
    fn test_geometry_delegation() {
        let mut menu = PieMenu::new(Point::new(50, 60), 40.0);

        // Geometry via Widget trait
        let geom = menu.geometry();
        assert_eq!(geom.x, 10); // 50 - 40
        assert_eq!(geom.y, 20); // 60 - 40
        assert_eq!(geom.width, 80);
        assert_eq!(geom.height, 80);

        // set_geometry through Widget trait
        menu.set_geometry(Rect::new(0, 0, 100, 100));
        assert_eq!(menu.geometry(), Rect::new(0, 0, 100, 100));

        // rect() alias
        assert_eq!(menu.geometry(), Rect::new(0, 0, 100, 100));

        // position / size
        assert_eq!(menu.position(), Point::new(0, 0));
        assert_eq!(menu.size(), crate::core::Size::new(100, 100));
    }

    /// 13. Widget ID and kind
    #[test]
    fn test_widget_id_and_kind() {
        let menu = PieMenu::new(Point::new(0, 0), 50.0);

        // Kind
        assert_eq!(menu.kind(), WidgetKind::PieMenu);

        // ID should be non-zero (Object generates unique IDs)
        assert_ne!(menu.id(), 0);

        // Two menus should have different IDs
        let menu2 = PieMenu::new(Point::new(10, 10), 30.0);
        assert_ne!(menu.id(), menu2.id());
    }

    /// 14. SVG output verification
    #[test]
    fn test_svg_output() {
        let mut menu = PieMenu::new(Point::new(50, 50), 50.0);
        menu.add_item("Cut");
        menu.add_item("Copy");
        menu.add_item("Paste");

        let svg = render_to_svg(&mut menu);

        // Should be a valid SVG string
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));

        // Geometry-derived viewBox/viewport
        assert!(svg.contains("width=\"100\""));
        assert!(svg.contains("height=\"100\""));

        // Should draw circle borders and center dot
        assert!(svg.contains("circle"));
        assert!(svg.contains("line"));

        // Empty menu also produces SVG with just circles
        let mut empty = PieMenu::new(Point::new(10, 10), 10.0);
        let empty_svg = render_to_svg(&mut empty);
        assert!(empty_svg.starts_with("<svg"));
    }

    /// 15. Disabled state blocks events
    #[test]
    fn test_disabled_state_blocks_events() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Test");

        // Disable the widget
        menu.set_enabled(false);
        assert!(!menu.is_enabled());

        // Hover events should not update hovered_index when disabled
        menu.handle_event(&Event::MouseMove { pos: Point::new(100, 100) });
        assert_eq!(menu.hovered_index(), None);

        // Mouse press should not trigger
        let triggered = Arc::new(Mutex::new(false));
        menu.triggered.connect({
            let triggered = Arc::clone(&triggered);
            move |_: Arc<usize>| {
                *triggered.lock().unwrap() = true;
            }
        });
        menu.handle_event(&Event::MousePress { pos: Point::new(100, 100), button: 1 });
        assert!(!*triggered.lock().unwrap());

        // Re-enable and verify events flow again
        // Use a point within the pie slice (dist >= inner_radius and <= radius)
        menu.set_enabled(true);
        menu.handle_event(&Event::MouseMove { pos: Point::new(150, 100) });
        assert_eq!(menu.hovered_index(), Some(0));
    }

    /// 16. Clear all items
    #[test]
    fn test_clear_items() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("A");
        menu.add_item("B");
        menu.add_item("C");
        menu.set_current_index(1);

        menu.clear();
        assert_eq!(menu.item_count(), 0);
        assert!(menu.items().is_empty());
        assert_eq!(menu.hovered_index(), None);
    }

    /// 17. Inner radius get/set
    #[test]
    fn test_inner_radius() {
        let mut menu = PieMenu::new(Point::new(100, 100), 100.0);

        // Default
        assert!((menu.inner_radius() - 35.0).abs() < 0.001);

        // Set inner radius
        menu.set_inner_radius(50.0);
        assert!((menu.inner_radius() - 50.0).abs() < 0.001);

        // Clamped to min 2.0
        menu.set_inner_radius(1.0);
        assert!((menu.inner_radius() - 2.0).abs() < 0.001);

        // Clamped to max radius * 0.95
        menu.set_inner_radius(200.0);
        assert!((menu.inner_radius() - 95.0).abs() < 0.001);
    }

    /// 18. show_at and hide
    #[test]
    fn test_show_at_and_hide() {
        let mut menu = PieMenu::new(Point::new(0, 0), 50.0);

        // Initially visible
        assert!(menu.is_visible());

        // show_at updates center and makes visible
        menu.show_at(Point::new(200, 300));
        assert_eq!(menu.center(), Point::new(200, 300));
        assert!(menu.is_visible());
        assert_eq!(menu.hovered_index(), None);

        // hide
        menu.hide();
        assert!(!menu.is_visible());
        assert_eq!(menu.hovered_index(), None);

        // Widget trait show/hide delegation
        menu.show();
        assert!(menu.is_visible());
        menu.hide();
        assert!(!menu.is_visible());
    }

    /// 19. Hover color and text color
    #[test]
    fn test_hover_and_text_colors() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);

        // Default hover color
        assert_eq!(menu.hover_color(), Color::rgb(0, 120, 215));

        // Change hover color
        menu.set_hover_color(Color::rgb(255, 0, 0));
        assert_eq!(menu.hover_color(), Color::rgb(255, 0, 0));

        // Default text color
        assert_eq!(menu.text_color(), Color::rgb(30, 30, 30));

        // Change text color
        menu.set_text_color(Color::rgb(255, 255, 255));
        assert_eq!(menu.text_color(), Color::rgb(255, 255, 255));
    }

    /// 20. Animation progress
    #[test]
    fn test_animation_progress() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);

        // Default
        assert!((menu.animation_progress() - 1.0).abs() < 0.001);

        // Set value
        menu.set_animation_progress(0.5);
        assert!((menu.animation_progress() - 0.5).abs() < 0.001);

        // Clamped to [0.0, 1.0]
        menu.set_animation_progress(-0.5);
        assert!((menu.animation_progress() - 0.0).abs() < 0.001);

        menu.set_animation_progress(1.5);
        assert!((menu.animation_progress() - 1.0).abs() < 0.001);
    }
}
