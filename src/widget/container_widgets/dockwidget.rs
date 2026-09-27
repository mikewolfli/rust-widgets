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

/// How thick a dock widget's title bar is, in logical pixels — its height when it runs
/// horizontally and its width when it runs vertically.
///
/// The `24` this replaced was written twice (`title_bar_rect` and `content_rect`), which is how the
/// two could be changed apart. One number, read by both, so the bar and its complement always sum
/// to the widget.
const TITLE_BAR_THICKNESS: u32 = 24;

/// The side of a title-bar button, in logical pixels.
///
/// Written once because three numbers are derived from it — the close button's box, the float
/// button's box and the offset between them — and the three were previously three literals that had
/// to be kept in step by hand.
const TITLE_BUTTON_SIZE: u32 = 16;

/// The gap between a title-bar button and the edge, or the next button, in logical pixels.
const TITLE_BUTTON_GAP: u32 = 5;

/// Every real dock edge, in the order [`DockWidget::set_docked`] prefers when it has to pick one.
///
/// A constant rather than a literal array at the call site so the "which edge do we fall back to"
/// question has one answer, and so adding an area to [`DockWidgetArea`] makes this list the place it
/// has to be considered. [`DockWidgetArea::NoDockWidgetArea`] is deliberately absent: it is the
/// *absence* of an edge, not an edge to fall back onto.
const ALL_DOCK_AREAS: [DockWidgetArea; 4] = [
    DockWidgetArea::LeftDockWidgetArea,
    DockWidgetArea::RightDockWidgetArea,
    DockWidgetArea::TopDockWidgetArea,
    DockWidgetArea::BottomDockWidgetArea,
];

/// Which edge a dock widget's title bar runs along, and therefore which way its chrome faces.
///
/// A title bar on the left edge is a *column* with vertical text, not a *row* with horizontal text;
/// drawing the row form for every edge is what made `dock_location` a stored fact with no
/// consequence. Rows and columns are what a painter can actually express here, so the two are the
/// whole vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitleBarAxis {
    /// The title bar spans the widget's width: its text reads left to right.
    Horizontal,
    /// The title bar spans the widget's height: its text reads top to bottom.
    Vertical,
}

impl DockWidgetArea {
    /// The axis a title bar on this edge runs along.
    ///
    /// A floating or undocked widget has no edge, so its title bar is horizontal -- the form every
    /// dock has used until now, which is what keeps the default rendering unchanged.
    fn title_bar_axis(self) -> TitleBarAxis {
        match self {
            DockWidgetArea::LeftDockWidgetArea | DockWidgetArea::RightDockWidgetArea => {
                TitleBarAxis::Vertical
            }
            _ => TitleBarAxis::Horizontal,
        }
    }
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
    ///
    /// # Why this is not simply assigning the field
    ///
    /// `docked` and `floating` are two views of one state, and `draw` keys on `floating` alone. So
    /// `set_docked(false)` used to set a field nothing read: the panel kept the docked appearance
    /// while `is_docked()` answered `false`, and the two accessors disagreed about the same widget.
    /// Writing the field is what the caller asked for; keeping the pair consistent is what makes the
    /// answer visible, which is the whole point of a *setter*. The signal is emitted the same way
    /// [`Self::set_floating`] emits it, because a caller who watches `top_level_changed` and a
    /// caller who watches `is_docked()` must not be told different stories.
    ///
    /// Undocking to a real edge is refused for the same reason [`Self::set_dock_location`] refuses
    /// it: an edge that is not in `allowed_areas` is not a place this widget may be.
    pub fn set_docked(&mut self, docked: bool) {
        if self.docked == docked {
            return;
        }
        // `docked(false)` means the widget is now a top-level window, which is `floating`;
        // `docked(true)` means it is back on the edge it reports.
        self.set_floating(!docked);
        self.docked = docked;
        if docked && self.dock_location == DockWidgetArea::NoDockWidgetArea {
            // Back onto an edge, but which one was never recorded. The first allowed area is the
            // one the host declared this widget may use, and guessing `Left` would be a location
            // the widget might not be permitted to take.
            let first_allowed =
                ALL_DOCK_AREAS.into_iter().find(|area| self.allowed_areas.contains(*area));
            if let Some(area) = first_allowed {
                self.set_dock_location(area);
            }
        }
        self.base.request_redraw();
    }
    /// Toggles floating state.
    pub fn toggle_floating(&mut self) {
        self.set_floating(!self.floating);
    }
    /// Returns title bar rectangle.
    ///
    /// # Why this is now a function of `dock_location`
    ///
    /// It used to be a fixed `rect.width × 24` band at the widget's top, whatever edge the widget
    /// reported. So a widget docked to the left drew a *horizontal* title bar across a narrow
    /// column -- `dock_location` was stored, published, emitted by a signal and read by no geometry.
    /// A title bar on a left or right edge is a vertical strip, and one on a top or bottom edge (or
    /// on a floating widget, which has no edge) is the horizontal band this already was.
    fn title_bar_rect(&self) -> Rect {
        let rect = self.geometry();
        // Measured from one number and clamped to the widget's own extent, so a widget shorter than
        // its own title bar cannot draw the bar outside itself.
        let thickness = TITLE_BAR_THICKNESS;
        match self.dock_location.title_bar_axis() {
            TitleBarAxis::Horizontal => {
                Rect::new(rect.x, rect.y, rect.width, thickness.min(rect.height))
            }
            TitleBarAxis::Vertical => {
                Rect::new(rect.x, rect.y, thickness.min(rect.width), rect.height)
            }
        }
    }
    /// Returns content rectangle.
    ///
    /// The complement of [`Self::title_bar_rect`] within the widget, along whichever axis the title
    /// bar took. Derived from that function rather than measured again, so the two cannot disagree
    /// about where the bar ends and the content begins.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let title_bar = self.title_bar_rect();
        match self.dock_location.title_bar_axis() {
            TitleBarAxis::Horizontal => Rect::new(
                rect.x,
                rect.y + title_bar.height as i32,
                rect.width,
                rect.height.saturating_sub(title_bar.height),
            ),
            TitleBarAxis::Vertical => Rect::new(
                rect.x + title_bar.width as i32,
                rect.y,
                rect.width.saturating_sub(title_bar.width),
                rect.height,
            ),
        }
    }
    /// Returns close button rectangle.
    ///
    /// The button sits at the title bar's **trailing** end, which is what makes it follow the bar's
    /// axis: in a horizontal bar that end is the right edge, and in a vertical bar it is the bottom.
    /// A button pinned to the right edge regardless (the previous form) would land on a vertical
    /// bar's *side* — outside the strip and over the content.
    fn close_button_rect(&self) -> Option<Rect> {
        if !self.features.dock_widget_closable {
            return None;
        }
        self.title_button_rect(0)
    }
    /// Returns float button rectangle.
    ///
    /// `slot` 0 is the one nearest the trailing end, so the two buttons do not overlap and their
    /// order does not depend on which one exists.
    fn float_button_rect(&self) -> Option<Rect> {
        if !self.features.dock_widget_floatable {
            return None;
        }
        let slot = if self.features.dock_widget_closable { 1 } else { 0 };
        self.title_button_rect(slot)
    }
    /// The box of the `slot`-th button from the title bar's trailing end, centred across the bar.
    ///
    /// One derivation for both buttons: the slot index is what differs, so a change to the size, the
    /// gap or the axis moves both together rather than one of them. A bar too short to hold the slot
    /// still yields a box at the trailing end -- overlapping rather than placed outside -- because a
    /// button drawn outside its own title bar would sit over the content below it.
    fn title_button_rect(&self, slot: u32) -> Option<Rect> {
        let bar = self.title_bar_rect();
        let size = TITLE_BUTTON_SIZE;
        let gap = TITLE_BUTTON_GAP;
        // The slot's own advance from the trailing edge: slot 0 is one button plus its outer gap,
        // slot 1 is a whole further button-plus-gap. Written as `(slot + 1) * (size + gap)` because
        // that is the original arithmetic for slot 0 (`- size - gap`) with the further slots as its
        // natural extension -- an extra `gap` term would move every button off the end by 5 px,
        // which is a one-pixel-level regression no geometry number in this file would show.
        let trailing = (slot + 1) * (size + gap);
        let (x, y) = match self.dock_location.title_bar_axis() {
            TitleBarAxis::Horizontal => (
                bar.x + (bar.width as i32 - trailing as i32).max(0),
                bar.y + (bar.height as i32 - size as i32) / 2,
            ),
            TitleBarAxis::Vertical => (
                bar.x + (bar.width as i32 - size as i32) / 2,
                bar.y + (bar.height as i32 - trailing as i32).max(0),
            ),
        };
        Some(Rect::new(x, y, size, size))
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::DOCK_WIDGET_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::DOCK_WIDGET_DEFAULT_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `DockWidget`'s property contract.
///
/// `floating` and `docked` are two views of one state, so `docked` is published as a **write** as
/// well as a read: `set_docked` now keeps the pair consistent and emits `top_level_changed`, which
/// is what makes writing it observable. It is the inverse of `set_floating` in the sense that
/// matters to a caller (`docked == !floating`), and the two setters agree rather than one being a
/// "better" entry point.
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
            // `docked` had no arm in the centralised writer. It has one now that
            // `set_docked` keeps the pair consistent and emits, so writing it is
            // observable rather than a silent field assignment.
            "docked" => {
                self.set_docked(expect_bool(value)?);
                Ok(())
            }
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
        // Draw title text. The origin is the glyph's **top** edge, so the title is centred by
        // half the difference between the bar and the line box. Using the bar's midline put a
        // 14 px title at 12..26 in a 24 px bar — two pixels onto the border below it.
        //
        // On a vertical bar the label runs *down* the strip, one glyph per line: the renderer has no
        // rotated-text primitive, and a horizontal label inside a 24 px column is truncated to its
        // first glyph, which is what made a left-docked panel unreadable. The label's leading edge is
        // the bar's leading end, so it starts at the top of the column exactly as it starts at the
        // left of the row.
        let title_font = Font::default();
        // The line box is measured in the *device* scale, so it is used at its own width; the
        // centring arithmetic below casts once, here, rather than at each of the two bands.
        let title_h = context.measure_text("M", &title_font).height.max(1) as i32;
        let title_color =
            if self.base.is_enabled() { ink } else { ink.blend(&title_bar_color, 0.5) };
        // The trailing end is where the buttons live, so the label is bounded by however many are
        // actually shown -- one gap before the first, one after each button.
        let shown = u32::from(self.features.dock_widget_closable)
            + u32::from(self.features.dock_widget_floatable);
        let buttons =
            if shown == 0 { 0 } else { shown * TITLE_BUTTON_SIZE + (shown + 1) * TITLE_BUTTON_GAP };
        match self.dock_location.title_bar_axis() {
            TitleBarAxis::Horizontal => {
                context.draw_text_fitted(
                    Rect::new(
                        title_bar.x + TITLE_BUTTON_GAP as i32,
                        title_bar.y + (title_bar.height as i32 - title_h) / 2,
                        title_bar.width.saturating_sub(buttons + TITLE_BUTTON_GAP),
                        title_h as u32,
                    ),
                    &self.title,
                    &title_font,
                    title_color,
                    HorizontalAlignment::Left,
                );
            }
            TitleBarAxis::Vertical => {
                let label = Rect::new(
                    title_bar.x + (title_bar.width as i32 - title_h) / 2,
                    title_bar.y + TITLE_BUTTON_GAP as i32,
                    title_h as u32,
                    title_bar.height.saturating_sub(buttons + TITLE_BUTTON_GAP),
                );
                self.draw_vertical_title(context, label, &title_font, title_color);
            }
        }
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

impl DockWidget {
    /// Draws a title on a vertical bar: one glyph per line, advancing down `band`.
    ///
    /// # Why glyph by glyph rather than one call
    ///
    /// The renderer has no rotated-text primitive, and a single horizontal call inside a 24 px
    /// column is truncated to its first glyph -- which is exactly the unreadable left-docked panel
    /// `dock_location` was already promising to render. Stacking the glyphs is the form a column can
    /// actually express.
    ///
    /// # Truncation
    ///
    /// The band's height bounds the number of glyphs, and the run stops when the next glyph would
    /// cross it rather than being clipped mid-glyph. An ellipsis is deliberately not appended: there
    /// is no room for one in a 24 px column, and a lone `.` under the title reads as part of it.
    fn draw_vertical_title(
        &self,
        context: &mut RenderContext,
        band: Rect,
        font: &Font,
        color: Color,
    ) {
        let step = context.measure_text("M", font).height.max(1) as i32;
        let bottom = band.y + band.height as i32;
        let mut pen_y = band.y;
        // One `char` per line: a multi-byte character is a single glyph, and `chars()` is what
        // keeps a CJK title from being cut mid-character.
        for ch in self.title.chars() {
            if pen_y + step > bottom {
                break;
            }
            let glyph = ch.to_string();
            // Centred across the strip by the glyph's own measured width, so a narrow `i` and a wide
            // `W` share the same column rather than all starting at the left edge.
            let width = context.measure_text(&glyph, font).width;
            let x = band.x + (band.width as i32 - width as i32).max(0) / 2;
            context.draw_text(Point::new(x, pen_y), &glyph, font, color, HorizontalAlignment::Left);
            pen_y += step;
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

    // ── `dock_location` orients the title bar ──

    /// A title bar on a left or right edge is a vertical strip; on a top or bottom edge (or a
    /// floating widget, which has no edge) it is the horizontal band it always was.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `dock_location` was stored, published (`dock_location()`), emitted (`dock_location_changed`)
    /// and **read by no geometry**: every edge drew the same `width × 24` bar across the widget's
    /// top. A left-docked panel therefore got a horizontal title bar across a narrow column. The
    /// assertion is on the bar's *shape*, which is the thing a paint has to obey.
    #[test]
    fn dock_location_orientates_the_title_bar() {
        let build = |area| {
            let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
            dw.set_dock_location(area);
            dw
        };

        // Vertical edges: a strip as wide as the bar is thick, running the widget's whole height.
        for area in [DockWidgetArea::LeftDockWidgetArea, DockWidgetArea::RightDockWidgetArea] {
            let dw = build(area);
            let bar = dw.title_bar_rect();
            assert_eq!(bar.width, TITLE_BAR_THICKNESS, "{area:?} keeps a vertical strip");
            assert_eq!(bar.height, 100, "{area:?} runs the widget's whole height");
            // And the content is what the bar left over, beside it rather than below it.
            let content = dw.content_rect();
            assert_eq!(content.x, bar.width as i32, "{area:?} leaves the content beside the bar");
            assert_eq!(content.y, 0);
            assert_eq!(content.width, 200 - TITLE_BAR_THICKNESS);
            assert_eq!(content.height, 100);
        }

        // Horizontal edges, plus no edge at all: the pre-existing band.
        for area in [
            DockWidgetArea::TopDockWidgetArea,
            DockWidgetArea::BottomDockWidgetArea,
            DockWidgetArea::NoDockWidgetArea,
        ] {
            let dw = build(area);
            let bar = dw.title_bar_rect();
            assert_eq!(bar.height, TITLE_BAR_THICKNESS, "{area:?} keeps a horizontal band");
            assert_eq!(bar.width, 200);
            let content = dw.content_rect();
            assert_eq!(content.y, bar.height as i32, "{area:?} leaves the content below it");
            assert_eq!(content.height, 100 - TITLE_BAR_THICKNESS);
        }

        // The bar and its content tile the widget exactly, on whichever axis was taken.
        for area in ALL_DOCK_AREAS.into_iter().chain([DockWidgetArea::NoDockWidgetArea]) {
            let dw = build(area);
            let bar = dw.title_bar_rect();
            let content = dw.content_rect();
            assert_eq!(
                bar.width * bar.height + content.width * content.height,
                dw.geometry().width * dw.geometry().height,
                "{area:?}: the bar and the content are complements"
            );
        }
    }

    /// The close and float buttons follow the bar they sit in: the trailing end of a horizontal bar
    /// is its right edge, and of a vertical one its bottom.
    ///
    /// A button pinned to the right edge regardless would land *outside* a vertical strip, over the
    /// content area — the same class of defect as the bar itself, one level down.
    #[test]
    fn title_bar_buttons_follow_the_bar_they_sit_in() {
        let build = |area| {
            let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
            dw.set_dock_location(area);
            dw
        };

        let horizontal = build(DockWidgetArea::TopDockWidgetArea);
        let bar = horizontal.title_bar_rect();
        let close = horizontal.close_button_rect().expect("the default is closable");
        assert!(
            bar.contains(Point::new(close.x + 1, close.y + 1)),
            "the close button is inside a horizontal bar: {close:?} vs {bar:?}"
        );
        let float = horizontal.float_button_rect().expect("the default is floatable");
        assert!(
            float.x + float.width as i32 <= close.x,
            "the float button sits before the close button, not on top of it: {float:?} / {close:?}"
        );

        let vertical = build(DockWidgetArea::LeftDockWidgetArea);
        let bar = vertical.title_bar_rect();
        let close = vertical.close_button_rect().expect("the default is closable");
        assert!(
            bar.contains(Point::new(close.x + 1, close.y + 1)),
            "the close button is inside a vertical bar: {close:?} vs {bar:?}"
        );
        let float = vertical.float_button_rect().expect("the default is floatable");
        assert!(
            float.y + float.height as i32 <= close.y,
            "the float button sits above the close button in a vertical bar: {float:?} / {close:?}"
        );
        // The vertical bar's trailing end is its *bottom*, so the close button is near it.
        assert!(
            close.y > bar.y + bar.height as i32 / 2,
            "the close button is towards the bar's trailing end: {close:?} vs {bar:?}"
        );
    }

    /// `set_docked` now moves the whole state: `floating` follows it, the pair stays consistent, and
    /// the state change is announced.
    ///
    /// `docked` and `floating` are two views of one fact, and `draw` keys on `floating` alone. The
    /// old setter assigned `docked` and nothing else, so `is_docked()` answered `false` while the
    /// widget kept its docked appearance and `top_level_changed` never fired — the two accessors
    /// disagreed about the same widget.
    #[test]
    fn set_docked_moves_floating_and_announces_the_change() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let top_level_events = Arc::new(AtomicUsize::new(0));
        dw.top_level_changed.connect({
            let count = Arc::clone(&top_level_events);
            move |_floating: Arc<bool>| {
                count.fetch_add(1, Ordering::SeqCst);
            }
        });

        dw.set_docked(false);
        assert!(!dw.is_docked());
        assert!(dw.is_floating(), "undocking is what floating means");
        assert_eq!(
            top_level_events.load(Ordering::SeqCst),
            1,
            "the state change is announced, as `set_floating` announces it"
        );

        dw.set_docked(true);
        assert!(dw.is_docked());
        assert!(!dw.is_floating(), "re-docking is not floating");
        assert_eq!(top_level_events.load(Ordering::SeqCst), 2);
        // Re-docking picked a real edge rather than leaving the widget at `NoDockWidgetArea` while
        // claiming to be docked.
        assert_ne!(
            dw.dock_location(),
            DockWidgetArea::NoDockWidgetArea,
            "a docked widget reports a real edge"
        );

        // Idempotent: re-applying the state is not a transition.
        dw.set_docked(true);
        assert_eq!(
            top_level_events.load(Ordering::SeqCst),
            2,
            "re-applying the same state emits nothing"
        );
    }

    /// Re-docking respects `allowed_areas` rather than picking an edge the host forbade.
    #[test]
    fn re_docking_picks_an_allowed_edge() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // A host that offers only the left and bottom edges. `all_dock_widget_areas` is left off
        // deliberately: it is an override, and an override set to `true` would permit every edge
        // and make this test unable to tell a respected constraint from an ignored one.
        dw.set_allowed_areas(DockWidgetAreas {
            left_dock_widget_area: true,
            right_dock_widget_area: false,
            top_dock_widget_area: false,
            bottom_dock_widget_area: true,
            all_dock_widget_areas: false,
            no_dock_widget_areas: false,
        });
        dw.set_docked(false);
        dw.set_docked(true);
        let area = dw.dock_location();
        assert!(
            area == DockWidgetArea::LeftDockWidgetArea
                || area == DockWidgetArea::BottomDockWidgetArea,
            "the chosen edge is one of the two the host allowed, not {area:?}"
        );
    }

    /// A left-docked widget renders differently from a top-docked one -- the field reaches the pixels.
    ///
    /// The geometry assertions above say where the bar goes; this says the paint obeys them (and,
    /// for the vertical case, that the glyph run is actually stacked rather than truncated to one
    /// glyph by the 24 px column).
    #[test]
    fn the_dock_edges_render_differently() {
        let build = |area| {
            let mut dw = DockWidget::new(Rect::new(0, 0, 160, 120));
            dw.set_title("Panel".to_string());
            dw.set_dock_location(area);
            dw
        };
        let top = render_to_svg(&mut build(DockWidgetArea::TopDockWidgetArea));
        let left = render_to_svg(&mut build(DockWidgetArea::LeftDockWidgetArea));
        assert_ne!(
            top, left,
            "a left-docked panel must not render as a top-docked one -- that was the dead state"
        );
    }
}
