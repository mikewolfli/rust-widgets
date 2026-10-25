// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dock widget.
use crate::compat::Cow;
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Orientation, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::save_state::SaveState;
use crate::widget::text_utils::elide_end;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    /// The surface the host offers for edge docking, if it declared one.
    ///
    /// Without it a drag can only move the widget; with it, a drag that reaches an edge previews
    /// and then takes that edge. `None` is the default, which is what keeps every existing
    /// placement and rendering byte-identical.
    dock_area: Option<Rect>,
    /// The edge a drag is currently hovering, drawn as a preview.
    dock_hint: Option<DockWidgetArea>,
    /// An optional child that replaces the built-in title bar chrome.
    title_bar_widget: Option<ObjectId>,
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

/// How close to a surface edge a drag must come to preview that edge, in logical pixels.
const DOCK_EDGE_BAND: i32 = 24;

/// A snapped dock takes one part in this many of the surface along its docked axis.
const DOCK_SNAP_DIVISOR: u32 = 3;

/// Version tag of the blob produced by [`DockWidget::save_state`].
const DOCK_STATE_VERSION: u8 = 1;

impl SaveState for DockWidget {
    fn state_version(&self) -> u16 {
        DOCK_STATE_VERSION as u16
    }

    fn save_state(&self) -> Vec<u8> {
        DockWidget::save_state(self)
    }

    fn restore_state(&mut self, state: &[u8]) -> bool {
        DockWidget::restore_state(self, state)
    }
}

/// The byte a saved blob stores for `area`.
fn area_tag(area: DockWidgetArea) -> u8 {
    match area {
        DockWidgetArea::NoDockWidgetArea => 0,
        DockWidgetArea::LeftDockWidgetArea => 1,
        DockWidgetArea::RightDockWidgetArea => 2,
        DockWidgetArea::TopDockWidgetArea => 3,
        DockWidgetArea::BottomDockWidgetArea => 4,
    }
}

/// The inverse of [`area_tag`].
fn area_from_tag(tag: u8) -> Option<DockWidgetArea> {
    match tag {
        0 => Some(DockWidgetArea::NoDockWidgetArea),
        1 => Some(DockWidgetArea::LeftDockWidgetArea),
        2 => Some(DockWidgetArea::RightDockWidgetArea),
        3 => Some(DockWidgetArea::TopDockWidgetArea),
        4 => Some(DockWidgetArea::BottomDockWidgetArea),
        _ => None,
    }
}

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
            dock_area: None,
            dock_hint: None,
            title_bar_widget: None,
        }
    }
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
        self.place_content();
        self.place_title_bar_widget();
        self.base.request_redraw();
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }

    /// Declares the surface the host offers for edge docking.
    ///
    /// A drag that reaches an edge of this rectangle previews the edge and, on release, docks
    /// there. Without a surface a drag only moves the widget, which is the behaviour every
    /// existing caller already has.
    pub fn set_dock_area(&mut self, area: Option<Rect>) {
        if self.dock_area == area {
            return;
        }
        self.dock_area = area;
        if self.dock_hint.is_some() {
            self.dock_hint = None;
        }
        self.base.request_redraw();
    }

    /// Returns the surface edge docking was declared against, if any.
    pub fn dock_area(&self) -> Option<Rect> {
        self.dock_area
    }

    /// Returns the edge a drag is currently hovering, if any.
    pub fn dock_hint(&self) -> Option<DockWidgetArea> {
        self.dock_hint
    }

    /// The rectangle the widget would take if the current hint were committed.
    ///
    /// The preview the painter draws and the geometry a release applies are both read from here,
    /// so what the user sees is exactly what they get.
    pub fn snap_preview_rect(&self) -> Option<Rect> {
        let area = self.dock_area?;
        self.dock_hint.map(|hint| self.snap_rect(area, hint))
    }

    /// The rectangle a snap to `area` on `edge` occupies: a third of the surface along the
    /// docked axis and the whole extent across it.
    fn snap_rect(&self, area: Rect, edge: DockWidgetArea) -> Rect {
        let third_width = (area.width / DOCK_SNAP_DIVISOR).max(1);
        let third_height = (area.height / DOCK_SNAP_DIVISOR).max(1);
        match edge {
            DockWidgetArea::LeftDockWidgetArea => {
                Rect::new(area.x, area.y, third_width, area.height)
            }
            DockWidgetArea::RightDockWidgetArea => Rect::new(
                area.x + area.width as i32 - third_width as i32,
                area.y,
                third_width,
                area.height,
            ),
            DockWidgetArea::TopDockWidgetArea => {
                Rect::new(area.x, area.y, area.width, third_height)
            }
            DockWidgetArea::BottomDockWidgetArea => Rect::new(
                area.x,
                area.y + area.height as i32 - third_height as i32,
                area.width,
                third_height,
            ),
            DockWidgetArea::NoDockWidgetArea => area,
        }
    }

    /// The edge `pos` is hovering within [`DOCK_EDGE_BAND`] of, if it is allowed.
    fn dock_hint_at(&self, pos: Point) -> Option<DockWidgetArea> {
        if !self.features.dock_widget_movable {
            return None;
        }
        let area = self.dock_area?;
        if !area.contains_point(pos) {
            return None;
        }
        let distances = [
            (pos.x - area.x, DockWidgetArea::LeftDockWidgetArea),
            (area.x + area.width as i32 - pos.x, DockWidgetArea::RightDockWidgetArea),
            (pos.y - area.y, DockWidgetArea::TopDockWidgetArea),
            (area.y + area.height as i32 - pos.y, DockWidgetArea::BottomDockWidgetArea),
        ];
        distances
            .into_iter()
            .filter(|(distance, edge)| {
                *distance <= DOCK_EDGE_BAND && self.allowed_areas.contains(*edge)
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, edge)| edge)
    }

    /// Replaces the built-in title bar chrome with a child widget, or restores it.
    ///
    /// While a child is installed it occupies the whole title bar: the title text and the built-in
    /// close/float buttons are not drawn or hit-tested, because the child is expected to provide
    /// its own chrome. Passing `None` restores the built-in bar.
    pub fn set_title_bar_widget(&mut self, widget: Option<ObjectId>) {
        if self.title_bar_widget == widget {
            return;
        }
        if let Some(previous) = self.title_bar_widget {
            self.base.remove_child_linked(previous);
            crate::set_widget_visible(previous, false);
        }
        self.title_bar_widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
            crate::set_widget_visible(widget_id, true);
        }
        self.place_title_bar_widget();
        self.base.request_redraw();
    }

    /// Returns the child installed as the title bar, if any.
    pub fn title_bar_widget(&self) -> Option<ObjectId> {
        self.title_bar_widget
    }

    /// Places the title-bar child in the bar's rectangle.
    fn place_title_bar_widget(&self) {
        #[cfg(not(alloc_frugal))]
        if let Some(widget_id) = self.title_bar_widget {
            let rect = self.title_bar_rect();
            if let Some(ref registry) = self.registry {
                registry.borrow_mut().set_widget_geometry(widget_id, rect);
            }
            crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
        }
    }

    /// Serialises the persistent state: geometry, dock edge and top-level flags.
    pub fn save_state(&self) -> Vec<u8> {
        let rect = self.geometry();
        let mut state = Vec::with_capacity(20);
        state.push(DOCK_STATE_VERSION);
        state.push(area_tag(self.dock_location));
        state.push(u8::from(self.floating));
        state.push(u8::from(self.docked));
        state.extend_from_slice(&rect.x.to_le_bytes());
        state.extend_from_slice(&rect.y.to_le_bytes());
        state.extend_from_slice(&rect.width.to_le_bytes());
        state.extend_from_slice(&rect.height.to_le_bytes());
        state
    }

    /// Restores a state produced by [`Self::save_state`].
    ///
    /// Returns `false` for a malformed blob, leaving the widget untouched. The stored location is
    /// applied directly rather than through [`Self::set_dock_location`], because a saved state
    /// describes where the widget *is*, not a request to move it: refusing a location the caller
    /// has since disallowed would silently drop the geometry that goes with it.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        if state.len() != 20 || state[0] != DOCK_STATE_VERSION {
            return false;
        }
        let Some(dock_location) = area_from_tag(state[1]) else {
            return false;
        };
        let x = i32::from_le_bytes([state[4], state[5], state[6], state[7]]);
        let y = i32::from_le_bytes([state[8], state[9], state[10], state[11]]);
        let width = u32::from_le_bytes([state[12], state[13], state[14], state[15]]);
        let height = u32::from_le_bytes([state[16], state[17], state[18], state[19]]);
        self.dock_location = dock_location;
        self.floating = state[2] != 0;
        self.docked = state[3] != 0;
        self.base.set_geometry(Rect::new(x, y, width, height));
        self.place_content();
        self.place_title_bar_widget();
        self.base.request_redraw();
        true
    }
    /// Returns title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets title.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }
    /// Sets widget.
    ///
    /// The link is written in **both** directions, and a replacement unlinks the previous content
    /// first — see BLUE22 §B.6 rule 5 and [`BaseWidget::add_child_linked`] for why `add_child`
    /// alone leaves a child no tree walk can reach.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        if let Some(previous) = self.widget {
            if Some(previous) != widget {
                self.base.remove_child_linked(previous);
                // Detached content must not keep painting; see `Frame::set_widget` for why the
                // visibility flag, not only the parent link, has to be cleared.
                crate::set_widget_visible(previous, false);
            }
        }
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
            crate::set_widget_visible(widget_id, true);
        }
        self.place_content();
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
        // A left/right dock takes a vertical title bar, which changes the content box.
        self.place_content();
        self.place_title_bar_widget();
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
    /// Places the docked widget in the content box.
    ///
    /// # Why the main tree path needs this
    ///
    /// `DockWidget` used to place its child **only** when drawing through a host registry. A
    /// window painted by `render_frame_tree` draws the child at the child's own geometry, so a
    /// docked control built at `0x0` stayed at `0x0` and a docked control built at its old
    /// rectangle kept painting there after the dock moved. The write goes through the public
    /// geometry entry, the same one every other container uses, so both paths agree.
    fn place_content(&self) {
        #[cfg(not(alloc_frugal))]
        {
            if let Some(widget_id) = self.widget {
                let rect = self.content_rect();
                if let Some(ref registry) = self.registry {
                    registry.borrow_mut().set_widget_geometry(widget_id, rect);
                }
                crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
            }
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
        if self.title_bar_widget.is_some() {
            return false;
        }
        if let Some(close_rect) = self.close_button_rect() {
            close_rect.contains(pos)
        } else {
            false
        }
    }
    /// Returns whether point is in float button.
    fn is_in_float_button(&self, pos: Point) -> bool {
        if self.title_bar_widget.is_some() {
            return false;
        }
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn save_state_dyn(&self) -> Option<&dyn SaveState> {
        Some(self)
    }

    fn save_state_dyn_mut(&mut self) -> Option<&mut dyn SaveState> {
        Some(self)
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::DOCK_WIDGET_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::DOCK_WIDGET_DEFAULT_HEIGHT,
        )
    }

    /// Moves the dock and re-places its content.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_content();
        self.place_title_bar_widget();
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "dock_location_changed" => Some(EventSignalRef::mapped(
                "dock_location_changed",
                &self.dock_location_changed,
                |v| CapabilityValue::String(format!("{v:?}")),
            )),
            "features_changed" => {
                Some(EventSignalRef::mapped("features_changed", &self.features_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "top_level_changed" => {
                Some(EventSignalRef::mapped("top_level_changed", &self.top_level_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
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
            Event::MousePress { pos, button, .. } if *button == 1 => {
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
                if let Some(hint) = self.dock_hint.take() {
                    // Commit exactly the preview the user was shown: the rect is derived from the
                    // same `snap_rect` the painter used, so the edge and geometry cannot disagree.
                    if let Some(area) = self.dock_area {
                        self.set_dock_location(hint);
                        let snapped = self.snap_rect(area, hint);
                        self.set_geometry(snapped);
                    }
                    self.base.request_redraw();
                }
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
                let hint = self.dock_hint_at(*pos);
                if hint != self.dock_hint {
                    self.dock_hint = hint;
                    self.base.request_redraw();
                }
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
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("dock_widget");
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
        if let Some(widget_id) = self.title_bar_widget {
            // A caller-supplied title bar owns the whole strip: the built-in title and buttons
            // are not drawn beside it, because the child is expected to provide its own chrome.
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(widget_id, title_bar);
                context.push_clip(title_bar.x, title_bar.y, title_bar.width, title_bar.height);
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_clip();
            }
        } else {
            // Draw title text. The origin is the glyph's **top** edge, so the title is centred by
            // half the difference between the bar and the line box. Using the bar's midline put a
            // 14 px title at 12..26 in a 24 px bar — two pixels onto the border below it.
            //
            // On a vertical bar the label runs *down* the strip, one glyph per line: the renderer has no
            // rotated-text primitive, and a horizontal label inside a 24 px column is truncated to its
            // first glyph, which is what made a left-docked panel unreadable. The label's leading edge is
            // the bar's leading end, so it starts at the top of the column exactly as it starts at the
            // left of the row.
            // The **effective font** — the resolved theme/caller font — so the title honours the theme
            // body font and the user's text scale (D09-STYLE-01).
            let title_font = effective_font(style);
            // The line box is measured in the *device* scale, so it is used at its own width; the
            // centring arithmetic below casts once, here, rather than at each of the two bands.
            let title_h = context.measure_text("M", title_font).height.max(1) as i32;
            let title_color = self.base.disabled_ink_on(ink, title_bar_color);
            // The trailing end is where the buttons live, so the label is bounded by however many are
            // actually shown -- one gap before the first, one after each button.
            let shown = u32::from(self.features.dock_widget_closable)
                + u32::from(self.features.dock_widget_floatable);
            let buttons = if shown == 0 {
                0
            } else {
                shown * TITLE_BUTTON_SIZE + (shown + 1) * TITLE_BUTTON_GAP
            };
            match self.dock_location.title_bar_axis() {
                TitleBarAxis::Horizontal => {
                    context.draw_text_fitted_cow(
                        Rect::new(
                            title_bar.x + TITLE_BUTTON_GAP as i32,
                            title_bar.y + (title_bar.height as i32 - title_h) / 2,
                            title_bar.width.saturating_sub(buttons + TITLE_BUTTON_GAP),
                            title_h as u32,
                        ),
                        &self.title,
                        title_font,
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
                    self.draw_vertical_title(context, label, title_font, title_color);
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
        // A drag that is hovering an edge previews the rectangle a release would take. Nothing is
        // drawn unless a hint is live, so the resting rendering is unchanged.
        if let Some(preview) = self.snap_preview_rect() {
            context.fill_rect(preview, panel.blend(&ink, 0.12));
            context.draw_rect(preview, border);
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
    /// The band's height bounds the number of glyphs, and the run is cut with the shared
    /// `…` rather than simply stopped: the old loop dropped the remainder silently, so a
    /// clipped panel title read as if it ended where the strip ran out. The row budget is a
    /// character budget (one glyph per row), which is exactly what [`elide_end`] takes, and
    /// it keeps the cut between characters so a CJK or emoji title is never split.
    fn draw_vertical_title(
        &self,
        context: &mut RenderContext,
        band: Rect,
        font: &Font,
        color: Color,
    ) {
        let step = context.measure_text("M", font).height.max(1) as i32;
        let mut pen_y = band.y;
        // One `char` per line: a multi-byte character is a single glyph, and the budget is
        // applied on character boundaries by the shared primitive.
        let title = self.vertical_title(band.height, step);
        for ch in title.chars() {
            let glyph = ch.to_string();
            // Centred across the strip by the glyph's own measured width, so a narrow `i` and a wide
            // `W` share the same column rather than all starting at the left edge.
            let width = context.measure_text(&glyph, font).width;
            let x = band.x + (band.width as i32 - width as i32).max(0) / 2;
            context.draw_text(Point::new(x, pen_y), &glyph, font, color, HorizontalAlignment::Left);
            pen_y += step;
        }
    }

    /// The title as a vertical run: at most as many glyphs as the band has rows.
    ///
    /// The band's height divided by one row's height is the character budget, so the
    /// shared primitive owns the cut and appends `…` when the title does not fit. A title
    /// that fits is returned borrowed, which keeps the common case allocation-free.
    fn vertical_title(&self, band_height: u32, step: i32) -> Cow<'_, str> {
        if step <= 0 {
            return Cow::Borrowed("");
        }
        let rows = (band_height as i32 / step).max(0) as usize;
        if rows == 0 {
            return Cow::Borrowed("");
        }
        elide_end(&self.title, rows)
    }
}

/// One dock registered with a [`DockArea`].
#[derive(Debug, Clone, Copy)]
struct DockEntry {
    /// The dock widget's id.
    widget: ObjectId,
    /// The edge it is docked to.
    area: DockWidgetArea,
    /// Docks sharing a group are tabbed: one rectangle, one visible member.
    group: usize,
    /// Whether this member is the visible one in its group.
    active: bool,
    /// The smallest extent this dock accepts along the band's axis.
    ///
    /// Zero means "no floor", which is what every dock had before the field existed; the split
    /// rule then divides the band equally. A non-zero floor is honoured whenever the band can hold
    /// it, so a toolbar or a narrow panel cannot be squeezed to nothing by its neighbours.
    min_size: u32,
    /// The dock whose rectangle this one subdivides, if any.
    ///
    /// `Some(parent)` means the dock does **not** own a band: it shares `parent`'s rectangle and is
    /// placed inside it by [`DockArea::place_dock`]. This is what makes a nested split expressible —
    /// the four-band model alone could only put a dock on an edge, never beside another dock.
    split_of: Option<ObjectId>,
    /// The axis along which `split_of`'s rectangle is divided among its split children.
    split_orientation: Orientation,
}

/// The lookup tables one arrangement pass is built from.
///
/// # Why one pass builds them
///
/// The previous arrangement asked `self.docks` for the same facts inside loops: group floors
/// filtered the whole vector per group, `place_dock` found each widget with `iter().find`, and
/// every nested child scanned the vector again for its orientation and floor. That made one
/// arrangement quadratic in the dock count. Every one of those questions is answered here in a
/// single walk and then read in constant time (C-28).
struct DockTables {
    /// Widget → position in [`DockArea::docks`].
    index_by_widget: HashMap<ObjectId, usize>,
    /// First-level groups belonging to each edge, in first-appearance order.
    groups_by_edge: [Vec<usize>; 4],
    /// The visible-band members of each group, in registration order.
    members_by_group: HashMap<usize, Vec<ObjectId>>,
    /// The strictest floor any member of a group states.
    floor_by_group: HashMap<usize, u32>,
    /// The docks that subdivide each dock's rectangle, in registration order.
    children_by_parent: HashMap<ObjectId, Vec<ObjectId>>,
    /// Edges that own at least one top-level dock.
    occupied_edges: [bool; 4],
}

/// The gap at the inner boundary of an edge band that a resize gesture can grab.
const EDGE_HANDLE_EXTENT: u32 = 4;

/// Version tag of the blob produced by [`DockArea::save_state`].
const DOCK_AREA_STATE_VERSION: u8 = 1;

/// A host surface that arranges [`DockWidget`]s around a central widget.
///
/// # Why this exists
///
/// A dock widget knows how to draw and drag **itself**, but nothing owned the surface it
/// docks into. Without a host, a drag could only move a window and every dock edge was a
/// number with no rectangle behind it. This type is that owner: it splits the surface into
/// edge bands, gives each dock the band for its edge, lets several docks share a band as
/// tabs, and gives the centre whatever is left.
///
/// It is a layout owner rather than a `Widget`: it holds child ids, not their state, so it
/// can arrange docks that live in any registry and cannot paint over them. Call
/// [`Self::arrange`] after a change to push the computed rectangles into the widgets.
pub struct DockArea {
    /// The whole surface, including the bands and the centre.
    area: Rect,
    /// Registered docks in insertion order.
    docks: Vec<DockEntry>,
    /// The widget that takes the remaining centre, if any.
    central: Option<ObjectId>,
    /// Next group number to hand out.
    next_group: usize,
    /// Optional registry used to place children through the shared widget table.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Thickness of one edge band in logical pixels.
    edge_extent: u32,
    /// Per-edge overrides of [`Self::edge_extent`], indexed like [`ALL_DOCK_AREAS`].
    ///
    /// The four bands used to share one number, so widening a left tool strip also widened the
    /// bottom console. An override lets each edge carry its own thickness; `None` means "use the
    /// shared default".
    edge_extents: [Option<u32>; 4],
    /// Emitted with the dock that became the visible member of its tab group.
    ///
    /// A host had no way to learn that the user switched tabs in a dock group, or which dock
    /// became visible after a `remove_dock` repaired the arrangement.
    pub active_dock_changed: Signal1<ObjectId>,
    /// Emitted with the dock that was removed from this host.
    pub dock_removed: Signal1<ObjectId>,
    /// Emitted after any change to the arrangement: a dock added, removed, tabbed, resized or
    /// split, or the surface moved.
    ///
    /// # Arity
    ///
    /// The issue spells this `Signal0`. This crate has no zero-argument signal type: a
    /// payload-free notification is `Signal1<()>`, which is what `CupertinoNavigationBar`'s
    /// `back_pressed` and `ThemeManager`'s `theme_changed` already use. The name and the emission
    /// point are the ones the issue asks for; only the spelling differs.
    pub layout_changed: Signal1<()>,
    /// The live edge-resize gesture, if one is in progress: the grabbed edge and how far the
    /// pointer sat from that band's inner boundary when the gesture began.
    edge_drag: Option<(DockWidgetArea, i32)>,
}

impl DockArea {
    /// Creates an empty host for `area` with a third of the shorter side as each band.
    pub fn new(area: Rect) -> Self {
        let edge_extent = (area.width.min(area.height) / DOCK_SNAP_DIVISOR).max(1);
        Self {
            area,
            docks: Vec::new(),
            central: None,
            next_group: 1,
            registry: None,
            edge_extent,
            edge_extents: [None; 4],
            active_dock_changed: Signal1::new(),
            dock_removed: Signal1::new(),
            layout_changed: Signal1::new(),
            edge_drag: None,
        }
    }

    /// The surface every child is arranged in.
    pub fn area(&self) -> Rect {
        self.area
    }

    /// Moves and resizes the surface.
    pub fn set_area(&mut self, area: Rect) {
        if self.area == area {
            return;
        }
        self.area = area;
        self.arrange();
    }

    /// The thickness of one edge band.
    pub fn edge_extent(&self) -> u32 {
        self.edge_extent
    }

    /// Sets the thickness of one edge band. Zero is clamped to one pixel.
    ///
    /// This is the *default* band thickness: an edge that was given its own value through
    /// [`Self::set_edge_extent_for`] keeps it.
    pub fn set_edge_extent(&mut self, extent: u32) {
        let extent = extent.max(1);
        if self.edge_extent == extent {
            return;
        }
        self.edge_extent = extent;
        self.arrange();
    }

    /// The thickness `area`'s band currently has.
    pub fn edge_extent_for(&self, area: DockWidgetArea) -> u32 {
        match edge_index(area) {
            Some(index) => self.edge_extents[index].unwrap_or(self.edge_extent),
            None => self.edge_extent,
        }
    }

    /// Gives `area`'s band its own thickness, returning `false` for
    /// [`DockWidgetArea::NoDockWidgetArea`].
    ///
    /// # Why this exists beside `set_edge_extent`
    ///
    /// The issue asks for `set_edge_extent(area, extent)`. Rust has no method overloading and the
    /// existing one-argument entry is public API that cannot be removed, so the per-edge form is a
    /// second name rather than a second meaning for the same name. Zero is clamped to one pixel.
    pub fn set_edge_extent_for(&mut self, area: DockWidgetArea, extent: u32) -> bool {
        let Some(index) = edge_index(area) else {
            return false;
        };
        let extent = extent.max(1);
        if self.edge_extent_for(area) == extent {
            return true;
        }
        self.edge_extents[index] = Some(extent);
        self.arrange();
        true
    }

    /// The smallest extent the dock `widget` accepts along its band's axis, if it is registered.
    pub fn dock_min_size(&self, widget: ObjectId) -> Option<u32> {
        self.docks.iter().find(|dock| dock.widget == widget).map(|dock| dock.min_size)
    }

    /// Sets the smallest extent `widget` accepts, returning whether it is registered.
    ///
    /// The floor is honoured while the band can hold every member's floor; when it cannot, the
    /// band is shared equally instead, so the docks still tile it rather than overflowing it.
    pub fn set_dock_min_size(&mut self, widget: ObjectId, min_size: u32) -> bool {
        let Some(dock) = self.docks.iter_mut().find(|dock| dock.widget == widget) else {
            return false;
        };
        if dock.min_size == min_size {
            return true;
        }
        dock.min_size = min_size;
        self.arrange();
        true
    }

    /// Uses `registry` to place and show children in addition to the global helpers.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
        self.arrange();
    }

    /// Registers `widget` on `area` as its own tab group.
    ///
    /// Returns `false` for [`DockWidgetArea::NoDockWidgetArea`] and for a widget that is
    /// already registered, so a repeated call cannot create a duplicate band.
    pub fn add_dock(&mut self, widget: ObjectId, area: DockWidgetArea) -> bool {
        if area == DockWidgetArea::NoDockWidgetArea
            || self.docks.iter().any(|dock| dock.widget == widget)
        {
            return false;
        }
        let group = self.next_group;
        self.next_group = self.next_group.saturating_add(1);
        self.docks.push(DockEntry {
            widget,
            area,
            group,
            active: true,
            min_size: 0,
            split_of: None,
            split_orientation: Orientation::Vertical,
        });
        self.arrange();
        true
    }

    /// Removes `widget` from the host, returning whether it was registered.
    ///
    /// Emits [`Self::dock_removed`] and, through [`Self::arrange`], [`Self::layout_changed`].
    pub fn remove_dock(&mut self, widget: ObjectId) -> bool {
        let before = self.docks.len();
        self.docks.retain(|dock| dock.widget != widget);
        let removed = self.docks.len() != before;
        if removed {
            // A dock that was split inside the removed one loses its parent: it keeps its own
            // rectangle (the band model places it again) instead of pointing at a dock that is gone.
            for dock in self.docks.iter_mut().filter(|dock| dock.split_of == Some(widget)) {
                dock.split_of = None;
            }
            self.dock_removed.emit(widget);
            self.arrange();
        }
        removed
    }

    /// The registered dock ids, in insertion order.
    pub fn dock_ids(&self) -> Vec<ObjectId> {
        self.docks.iter().map(|dock| dock.widget).collect()
    }

    /// The edge `widget` is docked to, if it is registered.
    pub fn dock_area_of(&self, widget: ObjectId) -> Option<DockWidgetArea> {
        self.docks.iter().find(|dock| dock.widget == widget).map(|dock| dock.area)
    }

    /// Sets the widget that occupies the centre, or clears it with `None`.
    pub fn set_central_widget(&mut self, widget: Option<ObjectId>) {
        if self.central == widget {
            return;
        }
        self.central = widget;
        self.arrange();
    }

    /// The centre widget, if any.
    pub fn central_widget(&self) -> Option<ObjectId> {
        self.central
    }

    /// Puts `other` in `into`'s tab group.
    ///
    /// Both must be registered. `other` moves to `into`'s edge when the two were on
    /// different edges, and `into` becomes the visible tab. A widget cannot be tabbed with
    /// itself. Returns whether the grouping changed.
    pub fn tabify(&mut self, into: ObjectId, other: ObjectId) -> bool {
        if into == other {
            return false;
        }
        let Some(anchor) = self.docks.iter().find(|dock| dock.widget == into).copied() else {
            return false;
        };
        let Some(index) = self.docks.iter().position(|dock| dock.widget == other) else {
            return false;
        };
        if self.docks[index].group == anchor.group && self.docks[index].area == anchor.area {
            return false;
        }
        self.docks[index].group = anchor.group;
        self.docks[index].area = anchor.area;
        // A tabbed member shares its anchor's rectangle, so any split it had is meaningless.
        self.docks[index].split_of = None;
        self.docks[index].active = false;
        for dock in self.docks.iter_mut().filter(|dock| dock.group == anchor.group) {
            dock.active = dock.widget == into;
        }
        self.active_dock_changed.emit(into);
        self.arrange();
        true
    }

    /// Splits `widget` out of its tab group into a group of its own.
    ///
    /// Returns `false` when the widget is not registered or already alone.
    pub fn untabify(&mut self, widget: ObjectId) -> bool {
        let Some(index) = self.docks.iter().position(|dock| dock.widget == widget) else {
            return false;
        };
        let group = self.docks[index].group;
        if self.docks.iter().filter(|dock| dock.group == group).count() < 2 {
            return false;
        }
        let new_group = self.next_group;
        self.next_group = self.next_group.saturating_add(1);
        self.docks[index].group = new_group;
        self.docks[index].active = true;
        self.active_dock_changed.emit(widget);
        self.arrange();
        true
    }

    /// Makes `widget` the visible member of its tab group.
    ///
    /// Returns `false` when the widget is not registered or is already visible.
    pub fn activate_dock(&mut self, widget: ObjectId) -> bool {
        let Some(index) = self.docks.iter().position(|dock| dock.widget == widget) else {
            return false;
        };
        if self.docks[index].active {
            return false;
        }
        let group = self.docks[index].group;
        for dock in self.docks.iter_mut().filter(|dock| dock.group == group) {
            dock.active = dock.widget == widget;
        }
        self.active_dock_changed.emit(widget);
        self.arrange();
        true
    }

    /// Whether `widget` shares its band with at least one other dock.
    pub fn is_tabbed(&self, widget: ObjectId) -> bool {
        let Some(group) = self.group_of(widget) else {
            return false;
        };
        self.docks.iter().filter(|dock| dock.group == group).count() > 1
    }

    /// The visible member of `widget`'s tab group, if the widget is registered.
    pub fn active_dock(&self, widget: ObjectId) -> Option<ObjectId> {
        let group = self.group_of(widget)?;
        self.docks
            .iter()
            .find(|dock| dock.group == group && dock.active)
            .map(|dock| dock.widget)
            .or_else(|| self.docks.iter().find(|dock| dock.group == group).map(|dock| dock.widget))
    }

    /// The group number of `widget`, if it is registered.
    fn group_of(&self, widget: ObjectId) -> Option<usize> {
        self.docks.iter().find(|dock| dock.widget == widget).map(|dock| dock.group)
    }

    /// Computes every child rectangle: the edge bands first, the centre from what is left.
    ///
    /// Members of one tab group share a rectangle; a dock that was split beside another dock is
    /// laid out inside its parent's rectangle. The result is in edge order
    /// (left, right, top, bottom) and then group order, with the centre last, so a caller
    /// can zip it with ids without assuming insertion order.
    pub fn layout(&self) -> Vec<(ObjectId, Rect)> {
        self.layout_entries().into_iter().map(|(widget, rect, _)| (widget, rect)).collect()
    }

    /// The arrangement, with each dock's visibility in the same pass (C-28).
    ///
    /// # Why the flag travels with the rectangle
    ///
    /// [`Self::arrange`] used to ask "is this widget's dock the active one?" with
    /// `docks.iter().all(..)` for **every** rectangle, and the group tables were rebuilt by
    /// filtering the whole dock list per edge and per group — quadratic in the dock count on every
    /// arrangement. Both tables are built once here from one walk, and the visibility answer is
    /// read off the entry that produced the rectangle.
    fn layout_entries(&self) -> Vec<(ObjectId, Rect, bool)> {
        let tables = self.build_tables();
        let mut out = Vec::new();

        let (bands, centre) = self.carve_occupied(&tables.occupied_edges);
        for (area, band) in bands {
            let horizontal = matches!(
                area,
                DockWidgetArea::TopDockWidgetArea | DockWidgetArea::BottomDockWidgetArea
            );
            let Some(edge) = edge_index(area) else {
                continue;
            };
            let groups = &tables.groups_by_edge[edge];
            // A group's floor is the largest floor its members state: the tabbed members share one
            // rectangle, so the strictest one is the one that holds.
            let floors: Vec<u32> = groups
                .iter()
                .map(|group| tables.floor_by_group.get(group).copied().unwrap_or(0))
                .collect();
            let total = if horizontal { band.width } else { band.height };
            for (group, (offset, extent)) in groups.iter().zip(distribute(total, &floors)) {
                let rect = if horizontal {
                    Rect::new(band.x + offset as i32, band.y, extent, band.height)
                } else {
                    Rect::new(band.x, band.y + offset as i32, band.width, extent)
                };
                let Some(widgets) = tables.members_by_group.get(group) else {
                    continue;
                };
                for widget in widgets {
                    self.place_dock(*widget, rect, &mut out, 0, &tables);
                }
            }
        }
        if let Some(central) = self.central {
            out.push((central, centre, true));
        }
        out
    }

    /// Builds every lookup an arrangement pass needs, in one walk over `docks`.
    fn build_tables(&self) -> DockTables {
        let mut tables = DockTables {
            index_by_widget: HashMap::with_capacity(self.docks.len()),
            groups_by_edge: core::array::from_fn(|_| Vec::new()),
            members_by_group: HashMap::new(),
            floor_by_group: HashMap::new(),
            children_by_parent: HashMap::new(),
            occupied_edges: [false; 4],
        };
        // Per-edge group membership is checked with a set, so a repeated group does not rescan the
        // edge's growing list.
        let mut seen_groups: [HashSet<usize>; 4] = core::array::from_fn(|_| HashSet::new());
        for (index, dock) in self.docks.iter().enumerate() {
            tables.index_by_widget.insert(dock.widget, index);
            if let Some(parent) = dock.split_of {
                tables.children_by_parent.entry(parent).or_default().push(dock.widget);
                continue;
            }
            if let Some(edge) = edge_index(dock.area) {
                tables.occupied_edges[edge] = true;
                if seen_groups[edge].insert(dock.group) {
                    tables.groups_by_edge[edge].push(dock.group);
                }
            }
            tables.members_by_group.entry(dock.group).or_default().push(dock.widget);
            let floor = tables.floor_by_group.entry(dock.group).or_insert(0);
            *floor = (*floor).max(dock.min_size);
        }
        tables
    }

    /// Places `widget` in `rect` and, recursively, the docks that were split inside it.
    ///
    /// The depth bound is a guard rather than a modelled limit: a split chain longer than 16 is not
    /// a layout, and a bound turns a malformed graph into "stops descending" instead of a hang.
    fn place_dock(
        &self,
        widget: ObjectId,
        rect: Rect,
        out: &mut Vec<(ObjectId, Rect, bool)>,
        depth: u32,
        tables: &DockTables,
    ) {
        let Some(&index) = tables.index_by_widget.get(&widget) else {
            return;
        };
        let entry = &self.docks[index];
        out.push((widget, rect, entry.active));
        if depth >= 16 {
            return;
        }
        let Some(children) = tables.children_by_parent.get(&widget) else {
            return;
        };
        if children.is_empty() {
            return;
        }
        // The siblings were all created by `split_dock`, which stores the axis on each child; the
        // first one states it for the set. Every lookup below is O(1) through the widget table.
        let orientation = children
            .first()
            .and_then(|child| tables.index_by_widget.get(child))
            .map(|&child| self.docks[child].split_orientation)
            .unwrap_or(Orientation::Vertical);
        let floors: Vec<u32> = children
            .iter()
            .map(|child| {
                tables
                    .index_by_widget
                    .get(child)
                    .map(|&child| self.docks[child].min_size)
                    .unwrap_or(0)
            })
            .collect();
        let total = match orientation {
            Orientation::Vertical => rect.height,
            Orientation::Horizontal => rect.width,
        };
        for (child, (offset, extent)) in children.iter().zip(distribute(total, &floors)) {
            let child_rect = match orientation {
                Orientation::Vertical => {
                    Rect::new(rect.x, rect.y + offset as i32, rect.width, extent)
                }
                Orientation::Horizontal => {
                    Rect::new(rect.x + offset as i32, rect.y, extent, rect.height)
                }
            };
            self.place_dock(*child, child_rect, out, depth + 1, tables);
        }
    }

    /// The band each occupied edge owns, in the order they are carved out of the surface.
    ///
    /// One derivation for [`Self::layout_entries`] and [`Self::edge_handle_rects`], so "where the
    /// band is" and "where its resize handle is" cannot disagree.
    fn edge_bands(&self) -> Vec<(DockWidgetArea, Rect)> {
        self.carve().0
    }

    /// Carves the occupied bands out of the surface and returns them with what is left.
    fn carve(&self) -> (Vec<(DockWidgetArea, Rect)>, Rect) {
        // One walk builds the occupancy bitset; the edge loop below then never rescans `docks`.
        let mut occupied = [false; 4];
        for dock in &self.docks {
            if dock.split_of.is_none() {
                if let Some(index) = edge_index(dock.area) {
                    occupied[index] = true;
                }
            }
        }
        self.carve_occupied(&occupied)
    }

    /// [`Self::carve`] with the occupancy bitset supplied by the caller's table build.
    fn carve_occupied(&self, occupied: &[bool; 4]) -> (Vec<(DockWidgetArea, Rect)>, Rect) {
        let mut bands = Vec::with_capacity(ALL_DOCK_AREAS.len());
        let mut remaining = self.area;
        for edge in ALL_DOCK_AREAS {
            let Some(index) = edge_index(edge) else {
                continue;
            };
            if !occupied[index] {
                continue;
            }
            let extent = self.edge_extent_for(edge);
            match edge {
                DockWidgetArea::LeftDockWidgetArea => {
                    let thickness = extent.min(remaining.width);
                    bands.push((
                        edge,
                        Rect::new(remaining.x, remaining.y, thickness, remaining.height),
                    ));
                    remaining.x = remaining.x.saturating_add(thickness as i32);
                    remaining.width = remaining.width.saturating_sub(thickness);
                }
                DockWidgetArea::RightDockWidgetArea => {
                    let thickness = extent.min(remaining.width);
                    bands.push((
                        edge,
                        Rect::new(
                            remaining.x + remaining.width as i32 - thickness as i32,
                            remaining.y,
                            thickness,
                            remaining.height,
                        ),
                    ));
                    remaining.width = remaining.width.saturating_sub(thickness);
                }
                DockWidgetArea::TopDockWidgetArea => {
                    let thickness = extent.min(remaining.height);
                    bands.push((
                        edge,
                        Rect::new(remaining.x, remaining.y, remaining.width, thickness),
                    ));
                    remaining.y = remaining.y.saturating_add(thickness as i32);
                    remaining.height = remaining.height.saturating_sub(thickness);
                }
                DockWidgetArea::BottomDockWidgetArea => {
                    let thickness = extent.min(remaining.height);
                    bands.push((
                        edge,
                        Rect::new(
                            remaining.x,
                            remaining.y + remaining.height as i32 - thickness as i32,
                            remaining.width,
                            thickness,
                        ),
                    ));
                    remaining.height = remaining.height.saturating_sub(thickness);
                }
                DockWidgetArea::NoDockWidgetArea => {}
            }
        }
        (bands, remaining)
    }

    /// The grab strips at the inner boundary of every occupied edge band.
    ///
    /// A host forwards pointer positions to [`Self::begin_edge_drag`], [`Self::drag_edge_to`] and
    /// [`Self::end_edge_drag`], which is the same grab-and-drag contract `Splitter` implements for
    /// its own handles.
    pub fn edge_handle_rects(&self) -> Vec<(DockWidgetArea, Rect)> {
        self.edge_bands()
            .into_iter()
            .map(|(edge, band)| {
                let handle = match edge {
                    DockWidgetArea::LeftDockWidgetArea => Rect::new(
                        band.x + band.width as i32 - EDGE_HANDLE_EXTENT as i32,
                        band.y,
                        EDGE_HANDLE_EXTENT,
                        band.height,
                    ),
                    DockWidgetArea::RightDockWidgetArea => {
                        Rect::new(band.x, band.y, EDGE_HANDLE_EXTENT, band.height)
                    }
                    DockWidgetArea::TopDockWidgetArea => Rect::new(
                        band.x,
                        band.y + band.height as i32 - EDGE_HANDLE_EXTENT as i32,
                        band.width,
                        EDGE_HANDLE_EXTENT,
                    ),
                    DockWidgetArea::BottomDockWidgetArea => {
                        Rect::new(band.x, band.y, band.width, EDGE_HANDLE_EXTENT)
                    }
                    DockWidgetArea::NoDockWidgetArea => band,
                };
                (edge, handle)
            })
            .collect()
    }

    /// The occupied edge whose resize handle is under `pos`, if any.
    pub fn edge_handle_at(&self, pos: Point) -> Option<DockWidgetArea> {
        self.edge_handle_rects()
            .into_iter()
            .find(|(_, rect)| rect.contains(pos))
            .map(|(edge, _)| edge)
    }

    /// Begins an edge-resize gesture at `pos`, returning whether a handle was grabbed.
    ///
    /// The pointer's distance from the band's inner boundary is remembered, so the band follows the
    /// pointer without jumping when the grab happened a few pixels into the handle. The handle's own
    /// inner edge *is* the band's inner boundary (`edge_handle_rects` derives it that way), which is
    /// why the offset can be read from the handle rectangle alone.
    pub fn begin_edge_drag(&mut self, pos: Point) -> bool {
        let Some((edge, handle)) =
            self.edge_handle_rects().into_iter().find(|(_, rect)| rect.contains(pos))
        else {
            return false;
        };
        let grab = match edge {
            DockWidgetArea::LeftDockWidgetArea => (handle.x + handle.width as i32) - pos.x,
            DockWidgetArea::RightDockWidgetArea => pos.x - handle.x,
            DockWidgetArea::TopDockWidgetArea => (handle.y + handle.height as i32) - pos.y,
            DockWidgetArea::BottomDockWidgetArea => pos.y - handle.y,
            DockWidgetArea::NoDockWidgetArea => return false,
        };
        self.edge_drag = Some((edge, grab));
        true
    }

    /// Moves the grabbed edge so its inner boundary follows `pos`.
    ///
    /// Returns `false` when no gesture is in progress. The extent is clamped to one pixel, and the
    /// arrangement (and therefore [`Self::layout_changed`]) is updated on a real change.
    pub fn drag_edge_to(&mut self, pos: Point) -> bool {
        let Some((edge, grab)) = self.edge_drag else {
            return false;
        };
        let extent = match edge {
            DockWidgetArea::LeftDockWidgetArea => pos.x - self.area.x + grab,
            DockWidgetArea::RightDockWidgetArea => {
                self.area.x + self.area.width as i32 - pos.x + grab
            }
            DockWidgetArea::TopDockWidgetArea => pos.y - self.area.y + grab,
            DockWidgetArea::BottomDockWidgetArea => {
                self.area.y + self.area.height as i32 - pos.y + grab
            }
            DockWidgetArea::NoDockWidgetArea => return false,
        };
        let extent = extent.max(1) as u32;
        if self.edge_extent_for(edge) == extent {
            return true;
        }
        // A drag writes the edge's own override rather than the shared default, so resizing one
        // band does not move the other three.
        if let Some(index) = edge_index(edge) {
            self.edge_extents[index] = Some(extent);
        }
        self.arrange();
        true
    }

    /// Ends the edge-resize gesture, if one is in progress.
    pub fn end_edge_drag(&mut self) {
        self.edge_drag = None;
    }

    /// The edge currently being resized, if any.
    pub fn edge_dragging(&self) -> Option<DockWidgetArea> {
        self.edge_drag.map(|(edge, _)| edge)
    }

    /// Splits `widget` out of its band and places it beside `beside`, inside `beside`'s rectangle.
    ///
    /// `orientation` states the axis the two are laid along: [`Orientation::Vertical`] stacks them
    /// (one above the other), [`Orientation::Horizontal`] puts them side by side.
    ///
    /// Returns `false` when either dock is unknown, when they are the same dock, when `beside` is
    /// itself a nested split (its rectangle belongs to its parent) or when `widget` is already split
    /// beside `beside`. A tabbed member loses its tab group: a control cannot both share one
    /// rectangle with a tab and own a rectangle of its own.
    ///
    /// # Registration order
    ///
    /// The split dock is re-inserted next to its parent in the registration order, which is how the
    /// band's own group order places the two rectangles side by side. [`Self::save_state`] writes
    /// docks in registration order, so a host restoring a blob should rebuild in
    /// [`Self::dock_ids`] order — which is the order this call re-establishes.
    pub fn split_dock(
        &mut self,
        widget: ObjectId,
        beside: ObjectId,
        orientation: Orientation,
    ) -> bool {
        if widget == beside {
            return false;
        }
        let Some(index) = self.docks.iter().position(|dock| dock.widget == widget) else {
            return false;
        };
        // Swap the entry out first, so the insertion point below is computed over the list the
        // entry is not part of.
        let mut entry = self.docks.remove(index);
        let Some(anchor_index) = self.docks.iter().position(|dock| dock.widget == beside) else {
            self.docks.insert(index, entry);
            return false;
        };
        if self.docks[anchor_index].split_of.is_some() {
            self.docks.insert(index, entry);
            return false;
        }
        let anchor_group = self.docks[anchor_index].group;
        let anchor_area = self.docks[anchor_index].area;
        // The new group is inserted right after the anchor's group, so the band's own order puts
        // the two rectangles next to each other instead of the new one at the end of the band.
        let insert_at = self
            .docks
            .iter()
            .rposition(|dock| dock.group == anchor_group)
            .map(|position| position + 1)
            .unwrap_or(anchor_index + 1);
        let group = self.next_group;
        self.next_group = self.next_group.saturating_add(1);
        entry.area = anchor_area;
        entry.group = group;
        entry.active = true;
        entry.split_of = Some(beside);
        entry.split_orientation = orientation;
        self.docks.insert(insert_at.min(self.docks.len()), entry);
        self.active_dock_changed.emit(widget);
        self.arrange();
        true
    }

    /// Puts the nested dock `widget` back in its own band slot, returning whether it was nested.
    pub fn unsplit_dock(&mut self, widget: ObjectId) -> bool {
        let Some(dock) = self.docks.iter_mut().find(|dock| dock.widget == widget) else {
            return false;
        };
        if dock.split_of.take().is_none() {
            return false;
        }
        dock.active = true;
        self.arrange();
        true
    }

    /// The dock `widget` was split inside, if any.
    pub fn split_parent_of(&self, widget: ObjectId) -> Option<ObjectId> {
        self.docks.iter().find(|dock| dock.widget == widget).and_then(|dock| dock.split_of)
    }

    /// Pushes the computed rectangles into the widgets and hides inactive tabs.
    ///
    /// Emits [`Self::layout_changed`], which is the one place an arrangement change is announced:
    /// every entry point above ends here, so no path can rearrange the surface silently.
    pub fn arrange(&self) {
        for (widget, rect, visible) in self.layout_entries() {
            crate::set_widget_geometry(widget, rect.x, rect.y, rect.width, rect.height);
            crate::set_widget_visible(widget, visible);
            if let Some(ref registry) = self.registry {
                let mut registry = registry.borrow_mut();
                registry.set_widget_geometry(widget, rect);
            }
        }
        self.layout_changed.emit(());
    }

    /// Serialises the dock arrangement: each dock's edge, tab group and active flag.
    ///
    /// Docks are stored in registration order and their ids are not written, so a restored
    /// blob describes the layout a host should rebuild rather than binding it to ids that
    /// may differ between sessions.
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(6 + self.docks.len() * 6);
        state.push(DOCK_AREA_STATE_VERSION);
        state.extend_from_slice(&(self.docks.len() as u32).to_le_bytes());
        for dock in &self.docks {
            state.push(area_tag(dock.area));
            state.extend_from_slice(&(dock.group as u32).to_le_bytes());
            state.push(u8::from(dock.active));
        }
        state
    }

    /// Restores a state produced by [`Self::save_state`].
    ///
    /// Returns `false` for a malformed blob or a dock count that does not match this host's,
    /// leaving the current arrangement untouched. The next group number is advanced past
    /// every restored group so a later [`Self::add_dock`] cannot join one by accident.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        const HEADER: usize = 5;
        if state.len() < HEADER || state[0] != DOCK_AREA_STATE_VERSION {
            return false;
        }
        let count = u32::from_le_bytes([state[1], state[2], state[3], state[4]]) as usize;
        if count != self.docks.len() || state.len() != HEADER + count * 6 {
            return false;
        }
        let mut cursor = HEADER;
        for dock in self.docks.iter_mut() {
            let Some(area) = area_from_tag(state[cursor]) else {
                return false;
            };
            let group = u32::from_le_bytes([
                state[cursor + 1],
                state[cursor + 2],
                state[cursor + 3],
                state[cursor + 4],
            ]) as usize;
            dock.area = area;
            dock.group = group;
            dock.active = state[cursor + 5] != 0;
            // The blob describes bands, groups and the visible member — not nesting. Clearing the
            // split parent keeps the restored arrangement self-consistent: a dock that was nested
            // when the blob was written is a band member again, which is exactly what the blob says
            // about it.
            dock.split_of = None;
            cursor += 6;
        }
        self.next_group = self
            .docks
            .iter()
            .map(|dock| dock.group.saturating_add(1))
            .max()
            .unwrap_or(1)
            .max(self.next_group);
        self.arrange();
        true
    }
}

/// The index of `area` in [`ALL_DOCK_AREAS`], or `None` for
/// [`DockWidgetArea::NoDockWidgetArea`].
///
/// The index is what [`DockArea::edge_extents`] is keyed by, so the per-edge overrides and the
/// traversal order cannot disagree about which slot belongs to which edge.
fn edge_index(area: DockWidgetArea) -> Option<usize> {
    ALL_DOCK_AREAS.iter().position(|candidate| *candidate == area)
}

/// Splits `extent` between `floors.len()` slots along one axis, returning `(offset, size)` pairs.
///
/// # The floor rule
///
/// The surplus beyond the stated floors is shared equally, so two docks with the same floor get the
/// same size and a dock with a larger floor gets exactly the extra it asked for. When the band
/// cannot hold the floors at all (a narrow surface, a wide minimum), the band is shared equally
/// instead: the answer is then smaller than the request, but it still tiles the band exactly rather
/// than spilling past its end.
///
/// The last slot always takes the remainder, so the sizes add up to `extent` even when the division
/// does not come out even.
fn distribute(extent: u32, floors: &[u32]) -> Vec<(u32, u32)> {
    let count = floors.len();
    if count == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(count);
    let total_floor = floors.iter().fold(0u32, |total, floor| total.saturating_add(*floor));
    if total_floor >= extent {
        let share = extent / count as u32;
        let mut cursor = 0u32;
        for slot in 0..count {
            let size = if slot + 1 == count { extent.saturating_sub(cursor) } else { share };
            out.push((cursor, size));
            cursor = cursor.saturating_add(size);
        }
        return out;
    }
    let extra = (extent - total_floor) / count as u32;
    let mut cursor = 0u32;
    for (slot, floor) in floors.iter().enumerate() {
        let size = if slot + 1 == count {
            extent.saturating_sub(cursor)
        } else {
            floor.saturating_add(extra)
        };
        out.push((cursor, size));
        cursor = cursor.saturating_add(size);
    }
    out
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
        dw.handle_event(&Event::MousePress { pos: close_pos, button: 1, modifiers: 0 });
        assert!(!dw.is_visible(), "clicking close button should hide the widget");
    }

    #[test]
    fn test_mouse_close_button_other_button_no_hide() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let close_pos = Point::new(179, 8);
        // Right-click should not hide
        dw.handle_event(&Event::MousePress { pos: close_pos, button: 2, modifiers: 0 });
        assert!(dw.is_visible(), "right-click on close should not hide");
    }

    #[test]
    fn test_mouse_float_button_toggles_floating() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // Float button area: width - 42 = 158, y = 4
        let float_pos = Point::new(162, 8);
        assert!(!dw.is_floating());
        dw.handle_event(&Event::MousePress { pos: float_pos, button: 1, modifiers: 0 });
        assert!(dw.is_floating(), "clicking float button should toggle floating");
    }

    #[test]
    fn test_mouse_title_bar_drag_starts() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let title_pos = Point::new(50, 10);
        dw.handle_event(&Event::MousePress { pos: title_pos, button: 1, modifiers: 0 });
        assert!(dw.base().is_mouse_pressed(), "mouse press on title bar should set mouse_pressed");
    }

    #[test]
    fn test_mouse_move_drags_widget() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let title_pos = Point::new(50, 10);
        // Press on title bar to start drag — stores offset (50, 10)
        dw.handle_event(&Event::MousePress { pos: title_pos, button: 1, modifiers: 0 });
        // Move mouse — widget should follow, maintaining the initial grab offset
        dw.handle_event(&Event::MouseMove { pos: Point::new(100, 50) });
        // Offset was (50, 10), so new position = (100 - 50, 50 - 10) = (50, 40)
        assert_eq!(dw.geometry(), Rect::new(50, 40, 200, 100));
    }

    #[test]
    fn test_mouse_release_ends_drag() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        let title_pos = Point::new(50, 10);
        dw.handle_event(&Event::MousePress { pos: title_pos, button: 1, modifiers: 0 });
        assert!(dw.base().is_mouse_pressed());
        dw.handle_event(&Event::MouseRelease { pos: Point::new(60, 20), button: 1 });
        assert!(!dw.base().is_mouse_pressed(), "mouse release should clear mouse_pressed");
    }

    #[test]
    fn test_mouse_outside_title_bar_no_drag() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        // Click below title bar (y > 24)
        let content_pos = Point::new(50, 50);
        dw.handle_event(&Event::MousePress { pos: content_pos, button: 1, modifiers: 0 });
        assert!(!dw.base().is_mouse_pressed(), "click outside title bar should not start drag");
    }

    #[test]
    fn test_disabled_widget_ignores_events() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        dw.set_enabled(false);
        let close_pos = Point::new(179, 8);
        dw.handle_event(&Event::MousePress { pos: close_pos, button: 1, modifiers: 0 });
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

    /// A multi-byte vertical title is cut on character boundaries and marked with `…`.
    ///
    /// The old run stopped silently at the band's bottom, so a clipped title read as if it
    /// ended there. The row budget is a character budget, which the shared primitive owns.
    #[test]
    fn a_long_vertical_title_is_elided_by_the_row_budget() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 160, 120));
        dw.set_title(
            "\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}".to_string(),
        );
        // Four 14 px rows fit in a 56 px band: three glyphs plus the ellipsis.
        assert_eq!(
            dw.vertical_title(56, 14),
            "\u{4e16}\u{754c}\u{4f60}\u{2026}",
            "the cut lands between characters and is marked"
        );
        dw.set_title("\u{1f389}\u{1f389}\u{1f389}\u{1f389}\u{1f389}".to_string());
        assert_eq!(
            dw.vertical_title(56, 14),
            "\u{1f389}\u{1f389}\u{1f389}\u{2026}",
            "an emoji is a single character to the budget, not four bytes"
        );
    }

    /// A title that fits the band is borrowed, so the common vertical run allocates nothing.
    #[test]
    fn a_fitting_vertical_title_is_borrowed() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 160, 120));
        dw.set_title("Panel".to_string());
        assert!(matches!(dw.vertical_title(120, 14), crate::compat::Cow::Borrowed("Panel")));
    }

    /// A drag within the declared surface previews and then takes the nearest allowed edge.
    #[test]
    fn dragging_to_an_edge_previews_and_snaps() {
        let mut dw = DockWidget::new(Rect::new(100, 100, 200, 100));
        dw.set_dock_area(Some(Rect::new(0, 0, 600, 400)));

        // Start the drag on the title bar.
        dw.handle_event(&Event::MousePress { pos: Point::new(110, 110), button: 1, modifiers: 0 });
        // Move toward the left edge of the surface.
        dw.handle_event(&Event::MouseMove { pos: Point::new(6, 200) });
        assert_eq!(dw.dock_hint(), Some(DockWidgetArea::LeftDockWidgetArea));
        let preview = dw.snap_preview_rect().expect("preview");
        assert_eq!(preview, Rect::new(0, 0, 200, 400));

        dw.handle_event(&Event::MouseRelease { pos: Point::new(6, 200), button: 1 });
        assert_eq!(dw.dock_location(), DockWidgetArea::LeftDockWidgetArea, "the hint is applied");
        assert_eq!(dw.geometry(), preview, "the committed rect is the previewed rect");
        assert_eq!(dw.dock_hint(), None, "a committed drag leaves no hint behind");
    }

    /// Geometry, edge and top-level flags round-trip through a saved blob.
    #[test]
    fn the_dock_state_round_trips_through_a_saved_blob() {
        let mut dw = DockWidget::new(Rect::new(10, 20, 200, 100));
        dw.set_dock_location(DockWidgetArea::RightDockWidgetArea);
        let state = dw.save_state();

        let mut restored = DockWidget::new(Rect::new(0, 0, 10, 10));
        assert!(restored.restore_state(&state));
        assert_eq!(restored.dock_location(), DockWidgetArea::RightDockWidgetArea);
        assert_eq!(restored.geometry(), Rect::new(10, 20, 200, 100));
        assert!(restored.is_docked());
        assert!(!restored.is_floating());

        assert!(!restored.restore_state(&[]), "an empty blob is refused");
        let mut corrupt = state.clone();
        corrupt[1] = 99;
        assert!(!restored.restore_state(&corrupt), "an unknown edge tag is refused");
    }

    /// A custom title-bar child takes the bar as its box and suppresses the built-in buttons.
    #[test]
    fn a_title_bar_widget_replaces_the_builtin_chrome() {
        let mut dw = DockWidget::new(Rect::new(0, 0, 200, 100));
        assert_eq!(dw.title_bar_widget(), None);
        dw.set_title_bar_widget(Some(9100));
        assert_eq!(dw.title_bar_widget(), Some(9100));
        assert!(
            !dw.is_in_close_button(Point::new(179, 12)),
            "the built-in close button is not hit while a child owns the bar"
        );
        dw.set_title_bar_widget(None);
        assert!(dw.is_in_close_button(Point::new(179, 12)), "removing it restores the chrome");
    }

    /// The host arranges each edge around the centre, tabs two docks into one band, and
    /// round-trips the arrangement through a saved blob.
    #[test]
    fn dock_area_arranges_edges_tabs_and_round_trips_state() {
        let mut host = DockArea::new(Rect::new(0, 0, 300, 200));
        assert!(host.add_dock(1, DockWidgetArea::LeftDockWidgetArea));
        assert!(host.add_dock(2, DockWidgetArea::LeftDockWidgetArea));
        assert!(host.add_dock(3, DockWidgetArea::RightDockWidgetArea));
        assert!(host.add_dock(4, DockWidgetArea::BottomDockWidgetArea));
        assert!(!host.add_dock(1, DockWidgetArea::LeftDockWidgetArea), "no duplicates");
        assert!(!host.add_dock(5, DockWidgetArea::NoDockWidgetArea));
        assert_eq!(host.dock_ids(), vec![1, 2, 3, 4]);

        host.set_central_widget(Some(9));
        host.set_edge_extent(30);
        assert!(host.tabify(2, 1));
        assert!(host.is_tabbed(1));
        assert_eq!(host.active_dock(1), Some(2));
        assert!(host.activate_dock(1));
        assert_eq!(host.active_dock(1), Some(1));
        assert!(!host.activate_dock(1), "already visible");

        let rects: std::collections::HashMap<ObjectId, Rect> = host.layout().into_iter().collect();
        assert_eq!(rects[&1], Rect::new(0, 0, 30, 200), "the left band spans the full height");
        assert_eq!(rects[&1], rects[&2], "tabbed docks share one rectangle");
        assert_eq!(rects[&3], Rect::new(270, 0, 30, 200), "the right band hugs the right edge");
        assert_eq!(
            rects[&4],
            Rect::new(30, 170, 240, 30),
            "the bottom band spans what the side bands left"
        );
        assert_eq!(rects[&9], Rect::new(30, 0, 240, 170), "the centre takes the remainder");

        let state = host.save_state();
        let mut restored = DockArea::new(Rect::new(0, 0, 300, 200));
        for (id, area) in [
            (1, DockWidgetArea::LeftDockWidgetArea),
            (2, DockWidgetArea::LeftDockWidgetArea),
            (3, DockWidgetArea::RightDockWidgetArea),
            (4, DockWidgetArea::BottomDockWidgetArea),
        ] {
            assert!(restored.add_dock(id, area));
        }
        assert!(restored.restore_state(&state));
        assert!(restored.is_tabbed(1));
        assert_eq!(restored.active_dock(1), Some(1));
        assert_eq!(restored.dock_area_of(3), Some(DockWidgetArea::RightDockWidgetArea));
        assert!(!restored.restore_state(b"bad"), "a malformed blob is refused");

        assert!(host.untabify(1));
        assert!(!host.is_tabbed(1));
        assert!(host.remove_dock(4));
        assert!(!host.remove_dock(4));
        assert_eq!(host.dock_area_of(4), None);
    }

    /// C-28: visibility and nested placement come from the per-pass lookup tables, so every dock
    /// is reported exactly once with the active flag of its own entry.
    #[test]
    fn the_arrangement_tables_report_each_dock_once_with_its_visibility() {
        let mut host = DockArea::new(Rect::new(0, 0, 300, 200));
        for id in 1..=4 {
            assert!(host.add_dock(id, DockWidgetArea::LeftDockWidgetArea));
        }
        host.set_edge_extent(60);
        assert!(host.tabify(2, 1));
        assert!(host.tabify(4, 3));
        // `tabify` makes the dock being tabbed the active member; select one per group explicitly.
        assert!(host.activate_dock(1));
        assert!(host.activate_dock(3));

        let entries = host.layout_entries();
        let by_widget: std::collections::HashMap<ObjectId, (Rect, bool)> =
            entries.iter().map(|(widget, rect, visible)| (*widget, (*rect, *visible))).collect();
        assert_eq!(entries.len(), 4, "one entry per dock");
        assert!(by_widget[&1].1, "the active member of the first tab group is visible");
        assert!(!by_widget[&2].1, "the inactive tab is not");
        assert!(by_widget[&3].1);
        assert!(!by_widget[&4].1);
        assert_eq!(by_widget[&1].0, by_widget[&2].0, "tabbed docks share one rectangle");

        // Split children live inside their parent's rectangle: two of them divide its height.
        assert!(host.add_dock(5, DockWidgetArea::LeftDockWidgetArea));
        assert!(host.add_dock(6, DockWidgetArea::LeftDockWidgetArea));
        assert!(host.split_dock(5, 1, Orientation::Vertical));
        assert!(host.split_dock(6, 1, Orientation::Vertical));
        let entries = host.layout_entries();
        assert_eq!(entries.len(), 6, "both split children are placed through the children table");
        let by_widget: std::collections::HashMap<ObjectId, (Rect, bool)> =
            entries.iter().map(|(widget, rect, visible)| (*widget, (*rect, *visible))).collect();
        let parent = by_widget[&1].0;
        let first = by_widget[&5].0;
        let second = by_widget[&6].0;
        assert_eq!(first.x, parent.x);
        assert_eq!(first.width, parent.width);
        let (upper, lower) = if first.y <= second.y { (first, second) } else { (second, first) };
        assert_eq!(upper.y, parent.y, "the first split child starts at the parent's top edge");
        assert_eq!(lower.y, upper.y + upper.height as i32, "the children stack vertically");
        assert_eq!(
            first.height + second.height,
            parent.height,
            "the split children divide their parent's box between them"
        );
        assert!(by_widget[&5].1 && by_widget[&6].1, "a split child is active on creation");
    }
}
