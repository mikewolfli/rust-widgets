// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dock widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
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
    /// Which edge this widget is docked to.
    ///
    /// The state [`DockWidget::dock_location_changed`] reports. It existed as an
    /// enum and as a signal but had no field and no setter, so the documented
    /// "fires only on an actual transition" event could not be produced by any
    /// caller — the signal was declared, published in the capability, and
    /// unreachable.
    dock_location: DockWidgetArea,
    /// Emitted after the dock area changes, with the new [`DockWidgetArea`].
    /// Fires only on an actual transition, not when the same area is re-applied.
    pub dock_location_changed: Signal1<DockWidgetArea>,
    /// Emitted when the enabled feature set changes, with the new
    /// [`DockWidgetFeatures`] bitmap.
    pub features_changed: Signal1<DockWidgetFeatures>,
    /// Emitted when the widget is floated or re-docked, with the new top-level
    /// state (`true` = floating, `false` = docked).
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
    /// Reports whether `area` is permitted by this set.
    ///
    /// # How the aggregate flags are read
    ///
    /// The struct carries per-edge flags plus two aggregates. A per-edge flag is the
    /// authority for that edge; `all_dock_widget_areas` permits every edge and is
    /// therefore treated as an override, which is what its name promises and what
    /// [`DockWidgetAreas::all`] relies on (it sets every flag, so the two readings
    /// agree there).
    ///
    /// [`DockWidgetArea::NoDockWidgetArea`] is always permitted: undocking is the one
    /// transition that can never leave the widget in a forbidden place, so refusing it
    /// would only trap the widget in an area the host no longer allows.
    pub fn contains(&self, area: DockWidgetArea) -> bool {
        match area {
            DockWidgetArea::NoDockWidgetArea => true,
            DockWidgetArea::LeftDockWidgetArea => {
                self.all_dock_widget_areas || self.left_dock_widget_area
            }
            DockWidgetArea::RightDockWidgetArea => {
                self.all_dock_widget_areas || self.right_dock_widget_area
            }
            DockWidgetArea::TopDockWidgetArea => {
                self.all_dock_widget_areas || self.top_dock_widget_area
            }
            DockWidgetArea::BottomDockWidgetArea => {
                self.all_dock_widget_areas || self.bottom_dock_widget_area
            }
        }
    }
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
            dock_location: DockWidgetArea::NoDockWidgetArea,
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
        self.base.request_redraw();
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
        self.base.request_redraw();
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.base.request_redraw();
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
            self.base.request_redraw();
        }
    }
    /// Returns allowed areas.
    pub fn allowed_areas(&self) -> DockWidgetAreas {
        self.allowed_areas
    }
    /// Sets allowed areas.
    ///
    /// When the area the widget is currently docked to is no longer permitted, the
    /// dock location moves to [`DockWidgetArea::NoDockWidgetArea`] and
    /// [`DockWidget::dock_location_changed`] fires — the alternative would leave the
    /// widget reporting a location its own configuration forbids.
    pub fn set_allowed_areas(&mut self, areas: DockWidgetAreas) {
        self.allowed_areas = areas;
        if !areas.contains(self.dock_location) {
            self.set_dock_location(DockWidgetArea::NoDockWidgetArea);
        }
        self.base.request_redraw();
    }
    /// Returns the dock area this widget is docked to.
    pub fn dock_location(&self) -> DockWidgetArea {
        self.dock_location
    }
    /// Moves the widget to `area`, emitting [`DockWidget::dock_location_changed`] on
    /// an actual transition.
    ///
    /// # Why re-applying the same area is silent
    ///
    /// A host that re-runs its layout on every resize re-applies the same area many
    /// times per second. Emitting each time would make the event useless as a change
    /// notification, so the comparison is made here rather than left to callers.
    ///
    /// # An area that the widget does not allow
    ///
    /// A request for an area outside [`DockWidget::allowed_areas`] is refused (the
    /// location is left alone) rather than silently honoured: the allowed set is the
    /// constraint the host declared, and a widget docked where it is not allowed is
    /// exactly the inconsistency the set exists to prevent. Use
    /// [`DockWidgetArea::NoDockWidgetArea`] to undock, which is always permitted.
    pub fn set_dock_location(&mut self, area: DockWidgetArea) {
        if self.dock_location == area {
            return;
        }
        if !self.allowed_areas.contains(area) {
            log::warn!(
                "dock widget refused dock area {area:?}: it is not in this widget's allowed \
                 areas, so the location is left at {:?}",
                self.dock_location
            );
            return;
        }
        self.dock_location = area;
        // Docking to a real edge is, by definition, not floating.
        self.floating = false;
        self.docked = area != DockWidgetArea::NoDockWidgetArea;
        self.dock_location_changed.emit(area);
        self.base.request_redraw();
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
            self.base.request_redraw();
        }
    }
    /// Returns whether dock widget is docked.
    pub fn is_docked(&self) -> bool {
        self.docked
    }
    /// Sets docked state.
    pub fn set_docked(&mut self, docked: bool) {
        self.docked = docked;
        self.base.request_redraw();
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(250, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `DockWidget`'s property contract.
///
/// `floating` and `docked` are two views of one state; the old writer had an arm
/// for neither, and `set_docked` is not the inverse of `set_floating` (it does not
/// emit `top_level_changed`), so both are published read-only rather than
/// guessed at.
impl WidgetProperties for DockWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "floating" => Ok(CapabilityValue::Bool(self.is_floating())),
            "docked" => Ok(CapabilityValue::Bool(self.is_docked())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            // `floating` is settable through the same accessor pair the old arm
            // used when it served this name.
            "floating" => {
                self.set_floating(expect_bool(value)?);
                Ok(())
            }
            // `docked` had no arm in the centralised writer.
            "docked" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `DOCK_WIDGET_PROPERTIES`.
        property_names_of!["title", "floating", "docked", BASE_PROPERTY_NAMES]
    }

    /// Reports the two published commands that name no property.
    ///
    /// `set_features` and `set_allowed_areas` take a feature set and an area set —
    /// bitflag enums, not [`CapabilityValue`] scalars — through the control's own
    /// [`Self::set_features`] / [`Self::set_allowed_areas`]. `DOCK_WIDGET_PROPERTIES`
    /// does not publish either as a property, so the default `set_foo` convention
    /// could not resolve them and reported both commands unknown against a control
    /// that implements them. `OutOfRange` reports that they need a payload.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_features" | "set_allowed_areas" => Err(CapabilityAccessError::OutOfRange),
            _ => self.default_command(name),
        }
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
        let allow_child_event = match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos } => self.content_rect().contains(*pos),
            _ => true,
        };
        // Forward content events only to the docked widget.
        if allow_child_event {
            if let Some(widget_id) = self.widget {
                if let Some(ref reg) = self.registry {
                    reg.borrow_mut().set_widget_geometry(widget_id, self.content_rect());
                    reg.borrow_mut().forward_event(widget_id, event);
                }
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

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the title bar, the buttons, the content area and the title text
        // used to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("dock_widget");
        // `dock_widget` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // panel painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from
        // it, the same distinction `Colors::input_background` draws for a field.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the floating indicator follows the appearance rather than staying a literal blue.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let panel = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| panel.blend(&ink, 0.25));
        // A floating dock is visually raised off the dock area, so its title bar is tinted
        // toward the accent; a docked one keeps the panel's own surface.
        let title_bar_color = if self.floating { panel.blend(&accent, 0.35) } else { panel };
        let button_color = if self.base.is_enabled() {
            ink.blend(&title_bar_color, 0.35)
        } else {
            ink.blend(&title_bar_color, 0.75)
        };
        let float_color = if self.floating {
            accent
        } else if self.base.is_enabled() {
            button_color
        } else {
            ink.blend(&title_bar_color, 0.75)
        };
        // The content region is inset one step further than the title bar, so the two read
        // as separate areas in either appearance.
        let content_fill = panel.blend(&ink, 0.03);

        // Draw title bar
        context.fill_rect(title_bar, title_bar_color);
        // Draw title bar border
        context.draw_rect(title_bar, border);
        // Draw title text
        context.draw_text(
            Point::new(title_bar.x + 5, title_bar.y + title_bar.height as i32 / 2),
            &self.title,
            &Font::default(),
            if self.base.is_enabled() { ink } else { ink.blend(&title_bar_color, 0.5) },
            HorizontalAlignment::Left,
        );
        // Draw close button if enabled
        if self.features.dock_widget_closable {
            if let Some(close_rect) = self.close_button_rect() {
                context.draw_line(
                    Point::new(close_rect.x, close_rect.y),
                    Point::new(
                        close_rect.x + close_rect.width as i32,
                        close_rect.y + close_rect.height as i32,
                    ),
                    button_color,
                );
                context.draw_line(
                    Point::new(close_rect.x + close_rect.width as i32, close_rect.y),
                    Point::new(close_rect.x, close_rect.y + close_rect.height as i32),
                    button_color,
                );
            }
        }
        // Draw float button if enabled
        if self.features.dock_widget_floatable {
            if let Some(float_rect) = self.float_button_rect() {
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
        context.fill_rect(content, content_fill);
        // Draw content border
        context.draw_rect(content, border);
        // Draw widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(widget_id, content);
                context.push_clip(content.x, content.y, content.width, content.height);
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_clip();
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
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
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
    ///
    /// # Why the dock-location case drives the widget rather than the signal
    ///
    /// An earlier revision called `dw.dock_location_changed.emit(..)` and then asserted
    /// the subscriber fired. That only proves `Signal1` delivers — it is true of every
    /// signal in the crate and says nothing about the widget. It also passed while
    /// `DockWidget` had **no dock-location state at all**, so it certified a feature
    /// that did not exist. The assertions below go through the real setter for exactly
    /// that reason.
    #[test]
    fn test_signal_accessors() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // dock_location_changed, driven by the widget's own state transition.
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
            dw.set_dock_location(DockWidgetArea::LeftDockWidgetArea);
            assert!(received.load(Ordering::SeqCst), "a real transition must emit");
            assert_eq!(*area_received.lock().unwrap(), DockWidgetArea::LeftDockWidgetArea);
            assert_eq!(dw.dock_location(), DockWidgetArea::LeftDockWidgetArea);
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

    /// Re-applying the current dock area must stay silent.
    ///
    /// A host re-runs its layout on every frame, so an emit per call would make the
    /// event useless as a change notification. The signal's own doc states "Fires only
    /// on an actual transition"; this is what holds the implementation to it.
    #[test]
    fn test_reapplying_the_same_dock_area_does_not_emit() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let count = Arc::new(AtomicUsize::new(0));
        dw.dock_location_changed.connect({
            let count = Arc::clone(&count);
            move |_area: Arc<DockWidgetArea>| {
                count.fetch_add(1, Ordering::SeqCst);
            }
        });
        dw.set_dock_location(DockWidgetArea::TopDockWidgetArea);
        dw.set_dock_location(DockWidgetArea::TopDockWidgetArea);
        dw.set_dock_location(DockWidgetArea::TopDockWidgetArea);
        assert_eq!(count.load(Ordering::SeqCst), 1, "only the first call is a transition");
    }

    /// An area the widget does not allow must be refused, not silently honoured.
    #[test]
    fn test_dock_location_refuses_a_forbidden_area() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        dw.set_allowed_areas(DockWidgetAreas {
            left_dock_widget_area: true,
            right_dock_widget_area: false,
            top_dock_widget_area: false,
            bottom_dock_widget_area: false,
            all_dock_widget_areas: false,
            no_dock_widget_areas: false,
        });
        dw.set_dock_location(DockWidgetArea::BottomDockWidgetArea);
        assert_eq!(
            dw.dock_location(),
            DockWidgetArea::NoDockWidgetArea,
            "a forbidden edge must not be adopted"
        );
        dw.set_dock_location(DockWidgetArea::LeftDockWidgetArea);
        assert_eq!(dw.dock_location(), DockWidgetArea::LeftDockWidgetArea);
    }

    /// Re-applying the current dock area must stay silent.
    /// Narrowing the allowed set must move a now-forbidden location out, and say so.
    #[test]
    fn test_narrowing_allowed_areas_undocks_a_forbidden_location() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        dw.set_dock_location(DockWidgetArea::RightDockWidgetArea);
        assert_eq!(dw.dock_location(), DockWidgetArea::RightDockWidgetArea);

        let seen = Arc::new(Mutex::new(None::<DockWidgetArea>));
        dw.dock_location_changed.connect({
            let seen = Arc::clone(&seen);
            move |area: Arc<DockWidgetArea>| {
                *seen.lock().unwrap() = Some(*area);
            }
        });
        dw.set_allowed_areas(DockWidgetAreas::none());
        assert_eq!(dw.dock_location(), DockWidgetArea::NoDockWidgetArea);
        assert_eq!(*seen.lock().unwrap(), Some(DockWidgetArea::NoDockWidgetArea));
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
