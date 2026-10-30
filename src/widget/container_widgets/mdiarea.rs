// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MDI area widget.
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::save_state::SaveState;
use crate::widget::IconSource;
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::theme_snapshot::ThemeSnapshot;
/// MDI area widget.
pub struct MdiArea {
    base: BaseWidget,
    subwindows: Vec<MdiSubWindow>,
    active_subwindow: Option<usize>,
    view_mode: ViewMode,
    /// The edge the `TabbedView` tab strip occupies (C-003).
    tab_strip_position: TabStripPosition,
    /// The tab whose close button is under the pointer, when there is one (C-002).
    ///
    /// Hover is presentation state for a drawn affordance: without it the ✕ on every tab is
    /// indistinguishable from the tab's own border until it is clicked.
    hovered_tab_close: Option<usize>,
    background: Background,
    activation_order: ActivationOrder,
    /// Emitted with the child `ObjectId` whenever a sub-window becomes active,
    /// including the automatic activation of the first sub-window on add and of
    /// a replacement sub-window after the active one is removed.
    pub subwindow_activated: Signal1<ObjectId>,
    /// Emitted with `(child_id, new_title)` whenever a sub-window's title is changed through
    /// [`MdiArea::set_sub_window_title`].
    ///
    /// # Why the change is announced by the container
    ///
    /// A sub-window is a record (an id, a rectangle and some flags), not a control: it owns no
    /// signal and cannot repaint the area it sits in. Announcing a title change from the container
    /// entry is what keeps the signal reachable — `MdiSubWindow::set_title` stays a value-level
    /// setter, and this is the entry a host should use.
    pub subwindow_title_changed: Signal1<(ObjectId, String)>,
    /// Child ids in the order they were activated, most recent last.
    ///
    /// Only [`ActivationOrder::HistoryOrder`] reads this, but every activation records it so a
    /// later switch of order has a real history to traverse rather than starting empty.
    activation_history: Vec<ObjectId>,
    /// The live edge-resize gesture, if one is in progress.
    resize_drag: Option<SubwindowResize>,
    /// The live title-bar move gesture, if one is in progress (C-14).
    move_drag: Option<SubwindowMove>,
    /// The live tab-reorder gesture, if one is in progress (C-17).
    tab_drag: Option<(usize, Point)>,
    /// The geometry a maximized sub-window will return to, with its index.
    maximized_restore: Option<(usize, Rect)>,
    /// Viewport offset applied to every sub-window frame in `SubWindowView`.
    ///
    /// Zero is the default, so every existing placement, hit test and snapshot is unchanged.
    /// A non-zero offset pans the work area without moving the stored geometries, which is
    /// what lets a host scroll a desktop larger than the visible area.
    scroll_offset: Point,
    /// Optional shared registry for child forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Set by a sub-window title write; the next paint consumes it and schedules the frame that
    /// makes the new title visible.
    title_dirty: Rc<Cell<bool>>,
    /// Emitted with `(child_id, minimized)` when a sub-window is minimized or restored (C-15).
    pub subwindow_minimized_changed: Signal1<(ObjectId, bool)>,
    /// Emitted with `(old_index, new_index)` when a tab is dragged to a new slot (C-17).
    pub subwindow_tab_moved: Signal1<(usize, usize)>,
    /// Cached z-order index and the key it was produced under (C-P08).
    ///
    /// The order changes only when a sub-window is added, removed or raised, but the hit chain
    /// and the painter used to collect and sort it four times per frame/event.
    z_cache: RefCell<Option<(u64, Rc<Vec<usize>>)>>,
}
/// MDI sub-window.
pub struct MdiSubWindow {
    widget: ObjectId,
    geometry: Rect,
    title: String,
    #[cfg(feature = "image")]
    icon: Option<Image>,
    /// Typed icon content for this sub-window.
    ///
    /// Shares one active state with the legacy [`MdiSubWindow::icon`] image: whichever was assigned
    /// most recently wins, so a getter never reports content the title bar is not drawing.
    icon_source: Option<IconSource>,
    minimized: bool,
    maximized: bool,
    closable: bool,
    movable: bool,
    resizable: bool,
    z_order: i32,
    /// The owning area's title signal, installed when the record is attached.
    ///
    /// A sub-window is a record, not a control, so it has no signal of its own. Holding a clone of
    /// the area's signal lets [`Self::set_title`] announce the same event as the container entry;
    /// the two write paths then cannot disagree (C-09).
    title_sink: Option<Signal1<(ObjectId, String)>>,
    /// Set when a title write changed the text; the owning area turns it into a repaint.
    title_dirty: Option<Rc<Cell<bool>>>,
}
/// How an MDI area arranges its sub-windows.
///
/// # Not the same as [`crate::widget::view_widgets::list_view::ViewMode`]
///
/// Both types are called `ViewMode`, but they describe different axes and are
/// **not** interchangeable:
///
/// * this one — how an `MdiArea` lays its child windows out (free-floating
///   sub-windows vs. tabs inside the area);
/// * [`list_view::ViewMode`][lv] — how one `ListView` presents its *own items*
///   (list, icons, details, thumbnails).
///
/// They are deliberately left as two enums: merging them would create one type
/// whose variants are meaningless in the other's context, so a `match` could not
/// be exhaustive in either widget. See principle #49.
///
/// [lv]: crate::widget::view_widgets::list_view::ViewMode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Sub-window mode
    #[default]
    SubWindowView,
    /// Tabbed view
    TabbedView,
}
/// Where the `TabbedView` tab strip sits inside the area (C-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabStripPosition {
    /// The strip occupies the area's top edge, with the body below it.
    #[default]
    North,
    /// The strip occupies the area's bottom edge, with the body above it.
    South,
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
            #[cfg(feature = "image")]
            icon: None,
            icon_source: None,
            minimized: false,
            maximized: false,
            closable: true,
            movable: true,
            resizable: true,
            z_order: 0,
            title_sink: None,
            title_dirty: None,
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
    /// Replaces the title.
    ///
    /// A real change is announced through the owning area's `subwindow_title_changed` signal and
    /// marks the area for repaint; a write of the same text is a no-op. The announcement lives here
    /// as well as on the container setter so a host that reaches a record through
    /// [`MdiArea::sub_window_mut`] cannot silently bypass the event (C-09).
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        if let Some(sink) = &self.title_sink {
            sink.emit((self.widget, self.title.clone()));
        }
        if let Some(dirty) = &self.title_dirty {
            dirty.set(true);
        }
    }
    #[cfg(feature = "image")]
    /// Returns icon.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    #[cfg(feature = "image")]
    /// Sets icon.
    ///
    /// Replaces any typed [`MdiSubWindow::icon_source`] previously assigned. A non-RGBA8 image is
    /// normalised once here so the title-bar draw can borrow its bytes every frame.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon.map(crate::widget::icon_source::normalize_rgba8_image);
        self.icon_source = None;
    }

    /// Assigns typed, already-resolved icon content to this sub-window.
    ///
    /// `None` clears the typed source and the legacy image together, so an explicit clear cannot be
    /// followed by the previous icon reappearing.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_source = source;
        #[cfg(feature = "image")]
        {
            self.icon = None;
        }
    }

    /// Returns the typed icon source, if one is active.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Clears the sub-window's icon, whatever form it was stored in.
    pub fn clear_icon_source(&mut self) {
        self.set_icon_source(None);
    }

    /// Whether the title bar of this sub-window reserves a leading icon slot.
    pub fn has_icon(&self) -> bool {
        if self.icon_source.is_some() {
            return true;
        }
        #[cfg(feature = "image")]
        {
            self.icon.is_some()
        }
        #[cfg(not(feature = "image"))]
        {
            false
        }
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
            tab_strip_position: TabStripPosition::North,
            hovered_tab_close: None,
            background: Background::Plain,
            activation_order: ActivationOrder::StackingOrder,
            subwindow_activated: Signal1::new(),
            subwindow_title_changed: Signal1::new(),
            activation_history: Vec::new(),
            resize_drag: None,
            move_drag: None,
            tab_drag: None,
            maximized_restore: None,
            scroll_offset: Point::new(0, 0),
            registry: None,
            title_dirty: Rc::new(Cell::new(false)),
            subwindow_minimized_changed: Signal1::new(),
            subwindow_tab_moved: Signal1::new(),
            z_cache: RefCell::new(None),
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
    ///
    /// Repaints: the sub-window frame is painted from the list this grows.
    pub fn add_sub_window(&mut self, widget: ObjectId, geometry: Rect) -> usize {
        let mut subwindow = MdiSubWindow::new(widget, geometry);
        subwindow.z_order = self.subwindows.len() as i32;
        subwindow.title_sink = Some(self.subwindow_title_changed.clone());
        subwindow.title_dirty = Some(Rc::clone(&self.title_dirty));
        self.base.add_child_linked(widget);
        self.subwindows.push(subwindow);
        let index = self.subwindows.len().saturating_sub(1);
        if self.active_subwindow.is_none() {
            self.active_subwindow = Some(index);
            self.record_activation(widget);
            self.subwindow_activated.emit(widget);
        }
        self.base.request_redraw();
        index
    }
    /// Removes a sub-window.
    ///
    /// Repaints: removing a frame changes the painted picture. The active index is
    /// repaired in both directions — decremented when an earlier sub-window was removed
    /// (so activation stays on the same window), and re-pointed when the active one
    /// itself was removed.
    pub fn remove_sub_window(&mut self, widget: ObjectId) {
        if let Some(index) = self.subwindows.iter().position(|sw| sw.widget == widget) {
            self.base.remove_child_linked(widget);
            // A closed sub-window is no longer part of the area; clear its visibility so it
            // cannot paint over the remaining frames.
            crate::set_widget_visible(widget, false);
            self.subwindows.remove(index);
            self.activation_history.retain(|id| *id != widget);
            if self.resize_drag.is_some_and(|drag| drag.index == index) {
                self.resize_drag = None;
            }
            if self.maximized_restore.is_some_and(|(maximized, _)| maximized == index) {
                self.maximized_restore = None;
            } else if let Some((maximized, _)) = self.maximized_restore {
                if maximized > index {
                    self.maximized_restore =
                        self.maximized_restore.map(|(maximized, rect)| (maximized - 1, rect));
                }
            }
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
            self.base.request_redraw();
        }
    }
    /// Returns number of sub-windows.
    pub fn sub_window_count(&self) -> usize {
        self.subwindows.len()
    }

    /// Closes the active sub-window, returning whether one was closed.
    ///
    /// # What "close" means here
    ///
    /// The area owns the *frame*, not the host's document: closing removes the sub-window from
    /// this collection, unlinks its child (both directions) and hides it, exactly as
    /// [`Self::remove_sub_window`] does. The activation the removal implies is announced through
    /// [`Self::subwindow_activated`], so a host that mirrors the selection is told which window is
    /// now in front.
    pub fn close_active_sub_window(&mut self) -> bool {
        let Some(active) = self.active_sub_window() else {
            return false;
        };
        self.remove_sub_window(active);
        true
    }

    /// Closes every sub-window, returning how many were closed.
    ///
    /// The ids are collected before anything is removed, so the walk is not mutating the list it
    /// iterates and the activation repairs `remove_sub_window` performs still run once per
    /// window.
    pub fn close_all_sub_windows(&mut self) -> usize {
        let ids: Vec<ObjectId> = self.subwindows.iter().map(|subwindow| subwindow.widget).collect();
        let count = ids.len();
        for widget in ids {
            self.remove_sub_window(widget);
        }
        count
    }

    /// Serialises the window layout: the view mode, every sub-window's geometry and its
    /// z-order (C-001).
    ///
    /// A sub-window is a record rather than a mounted control, so the format is a compact
    /// position-indexed table instead of a tree walk. Entries follow [`Self::subwindows`]
    /// order; a restore applies to the same number of windows the caller re-added, exactly as
    /// [`super::splitter::Splitter::restore_state`] refuses a different pane count.
    pub fn save_state(&self) -> Vec<u8> {
        let count = self.subwindows.len();
        let mut out = Vec::with_capacity(MDI_STATE_HEADER + count * MDI_STATE_ENTRY);
        out.extend_from_slice(MDI_STATE_MAGIC);
        out.push(MDI_STATE_VERSION);
        out.push(match self.view_mode {
            ViewMode::SubWindowView => 0,
            ViewMode::TabbedView => 1,
        });
        out.extend_from_slice(&(count as u32).to_le_bytes());
        for subwindow in &self.subwindows {
            let geometry = subwindow.geometry;
            out.extend_from_slice(&geometry.x.to_le_bytes());
            out.extend_from_slice(&geometry.y.to_le_bytes());
            out.extend_from_slice(&geometry.width.to_le_bytes());
            out.extend_from_slice(&geometry.height.to_le_bytes());
            out.extend_from_slice(&subwindow.z_order.to_le_bytes());
        }
        out
    }

    /// Restores a blob produced by [`Self::save_state`] (C-001).
    ///
    /// Returns `false` for a malformed blob, an unknown version or a window count that no
    /// longer matches, leaving the area untouched. The table is parsed completely before any
    /// field is written, so a rejected blob can never half-restore.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        if state.len() < MDI_STATE_HEADER || &state[..4] != MDI_STATE_MAGIC {
            return false;
        }
        if state[4] != MDI_STATE_VERSION {
            return false;
        }
        let view_mode = match state[5] {
            0 => ViewMode::SubWindowView,
            1 => ViewMode::TabbedView,
            _ => return false,
        };
        let count = u32::from_le_bytes([state[6], state[7], state[8], state[9]]) as usize;
        if state.len() != MDI_STATE_HEADER + count * MDI_STATE_ENTRY
            || count != self.subwindows.len()
        {
            return false;
        }
        let mut parsed = Vec::with_capacity(count);
        for index in 0..count {
            let base = MDI_STATE_HEADER + index * MDI_STATE_ENTRY;
            let x = i32::from_le_bytes([
                state[base],
                state[base + 1],
                state[base + 2],
                state[base + 3],
            ]);
            let y = i32::from_le_bytes([
                state[base + 4],
                state[base + 5],
                state[base + 6],
                state[base + 7],
            ]);
            let width = u32::from_le_bytes([
                state[base + 8],
                state[base + 9],
                state[base + 10],
                state[base + 11],
            ]);
            let height = u32::from_le_bytes([
                state[base + 12],
                state[base + 13],
                state[base + 14],
                state[base + 15],
            ]);
            let z_order = i32::from_le_bytes([
                state[base + 16],
                state[base + 17],
                state[base + 18],
                state[base + 19],
            ]);
            parsed.push((Rect::new(x, y, width, height), z_order));
        }
        self.view_mode = view_mode;
        self.hovered_tab_close = None;
        for (subwindow, (geometry, z_order)) in self.subwindows.iter_mut().zip(parsed) {
            subwindow.geometry = geometry;
            subwindow.z_order = z_order;
        }
        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset);
        self.base.request_redraw();
        true
    }

    /// Replaces the title of the sub-window hosting `widget`.
    ///
    /// Returns `false` when no sub-window hosts `widget`, so a caller cannot mistake a dropped
    /// write for a successful one. On a real change the area repaints and emits
    /// [`Self::subwindow_title_changed`] with the new text — a title is painted in the title bar,
    /// so a silently assigned field would be invisible until some unrelated repaint happened.
    pub fn set_sub_window_title(&mut self, widget: ObjectId, title: String) -> bool {
        let Some(subwindow) = self.subwindows.iter_mut().find(|sw| sw.widget == widget) else {
            return false;
        };
        if subwindow.title == title {
            return true;
        }
        // The record's setter announces a change through the sink it was given. Here the area is
        // the caller, so it suppresses that path and emits once itself, keeping one announcement
        // per change (and one visible emit for the capability gate).
        let sink = subwindow.title_sink.take();
        subwindow.set_title(title);
        subwindow.title_sink = sink;
        self.title_dirty.set(false);
        self.base.request_redraw();
        self.subwindow_title_changed.emit((widget, subwindow.title.clone()));
        true
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
                self.record_activation(widget);
                self.subwindow_activated.emit(widget);
                self.base.request_redraw();
            }
        }
    }

    /// Records `widget` as the most recently activated sub-window.
    ///
    /// A repeated activation moves the id to the end rather than adding a duplicate, so the
    /// history reads as an ordering of windows rather than as a click log.
    fn record_activation(&mut self, widget: ObjectId) {
        self.activation_history.retain(|id| *id != widget);
        self.activation_history.push(widget);
    }
    /// Returns sub-window at index.
    pub fn sub_window(&self, index: usize) -> Option<&MdiSubWindow> {
        self.subwindows.get(index)
    }
    /// Returns mutable sub-window at index.
    pub fn sub_window_mut(&mut self, index: usize) -> Option<&mut MdiSubWindow> {
        self.subwindows.get_mut(index)
    }

    /// Assigns typed icon content to the sub-window owning `widget`.
    ///
    /// A sub-window is identified by the child `ObjectId` it hosts, which is the stable identity this
    /// collection already has, so no second selection model is introduced. A missing or duplicated
    /// child is reported instead of silently targeting the wrong window.
    pub fn set_item_icon_source(
        &mut self,
        widget: ObjectId,
        source: Option<IconSource>,
    ) -> Result<(), crate::widget::IconItemError> {
        let mut found: Option<usize> = None;
        for (index, subwindow) in self.subwindows.iter().enumerate() {
            if subwindow.widget == widget {
                if found.is_some() {
                    return Err(crate::widget::IconItemError::AmbiguousItemId { id: widget });
                }
                found = Some(index);
            }
        }
        match found {
            Some(index) => {
                self.subwindows[index].set_icon_source(source);
                self.base.request_redraw();
                Ok(())
            }
            None => Err(crate::widget::IconItemError::ItemNotFound { id: widget }),
        }
    }
    /// Returns view mode.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }
    /// Sets view mode.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
        if mode != ViewMode::TabbedView {
            self.hovered_tab_close = None;
        }
        self.base.request_redraw();
    }

    /// The edge the `TabbedView` tab strip occupies.
    pub fn tab_strip_position(&self) -> TabStripPosition {
        self.tab_strip_position
    }

    /// Moves the `TabbedView` tab strip to `position`.
    ///
    /// The strip and the body are one derivation ([`Self::tab_strip_rect`] and
    /// [`Self::subwindow_placement`]), so moving the strip moves the body it leaves behind.
    pub fn set_tab_strip_position(&mut self, position: TabStripPosition) {
        if self.tab_strip_position == position {
            return;
        }
        self.tab_strip_position = position;
        self.base.request_redraw();
    }

    /// The box the tab strip occupies in [`ViewMode::TabbedView`].
    ///
    /// One derivation, read by the paint *and* by [`Self::tab_at_point`], because a strip whose
    /// clickable tabs are not the tabs it drew is the same class of defect the row height had in
    /// `list_view` -- two independent derivations that drift.
    fn tab_strip_rect(&self) -> Rect {
        let rect = self.geometry();
        let height = TAB_STRIP_HEIGHT.min(rect.height as i32).max(0) as u32;
        let y = match self.tab_strip_position {
            TabStripPosition::North => rect.y,
            TabStripPosition::South => rect.y + rect.height as i32 - height as i32,
        };
        Rect::new(rect.x, y, rect.width, height)
    }

    /// Tab `index`'s own box, or `None` when it is past the strip's trailing edge.
    ///
    /// Tabs are laid out end to end from the strip's leading edge at an equal width, capped at
    /// [`TAB_MAX_WIDTH`]: a strip holding two tabs must not stretch each across half the work area,
    /// because tabs and a toolbar are read differently. A tab that would cross the trailing edge is
    /// not returned at all rather than returned truncated -- a half-drawn tab reads as a rendering
    /// error, and the caller can draw an overflow affordance instead.
    fn tab_rect(&self, index: usize) -> Option<Rect> {
        let count = self.subwindows.len();
        if count == 0 || index >= count {
            return None;
        }
        let strip = self.tab_strip_rect();
        let width = (strip.width / count as u32).clamp(1, TAB_MAX_WIDTH);
        let x = strip.x + (width as usize * index) as i32;
        if x + width as i32 > strip.x + strip.width as i32 {
            return None;
        }
        Some(Rect::new(x, strip.y, width, strip.height))
    }

    /// The tab index a point falls on, if it falls on a drawn tab.
    ///
    /// The inverse of [`Self::tab_rect`], and deliberately built on it for the same reason
    /// `list_view` builds its own inverse on its forward geometry.
    fn tab_at_point(&self, point: Point) -> Option<usize> {
        let strip = self.tab_strip_rect();
        if !strip.contains_point(point) {
            return None;
        }
        (0..self.subwindows.len())
            .find(|index| self.tab_rect(*index).is_some_and(|tab| tab.contains_point(point)))
    }

    /// Where sub-window `index` is placed under the current [`ViewMode`].
    ///
    /// # Why one struct rather than a branch in the paint
    ///
    /// The two modes differ in **two** things: the frame's box and whether the frame draws its own
    /// title bar. A paint that read `self.view_mode` at each of those places would be two chances to
    /// update one of them, and the symptom would be a tab strip naming a window whose title bar is
    /// also drawn -- or, worse, a click that activates a tab whose body is not the one on screen.
    /// Resolving the mode once and reading the result everywhere is what makes that impossible.
    ///
    /// # The modes
    ///
    /// | mode | frame | title bar |
    /// |---|---|---|
    /// | `SubWindowView` | the sub-window's own stored geometry | drawn |
    /// | `TabbedView` | the work area below the tab strip | not drawn (the tab names it) |
    ///
    /// # Why a non-active sub-window has no placement in `TabbedView`
    ///
    /// The mode shows exactly one window, and which one is [`Self::active_sub_window`]'s answer.
    /// Handing every sub-window the whole body would paint them all on top of each other, and the
    /// last one drawn -- not the active one -- would be the one the user sees.
    fn subwindow_placement(&self, index: usize) -> Option<SubwindowPlacement> {
        let subwindow = self.subwindows.get(index)?;
        match self.view_mode {
            ViewMode::SubWindowView => {
                // A maximized window owns the whole area, so the viewport offset does not
                // move it; every other frame is drawn at its stored position minus the pan.
                let frame = if subwindow.maximized {
                    subwindow.geometry
                } else {
                    self.view_frame(subwindow.geometry)
                };
                Some(SubwindowPlacement { frame, title_bar: true })
            }
            ViewMode::TabbedView => {
                if self.active_subwindow != Some(index) {
                    return None;
                }
                let rect = self.geometry();
                let strip = self.tab_strip_rect();
                // The body is the work area the strip leaves. Both edges are taken from the same
                // `geometry()` the strip was, so a strip clamped to a short area cannot leave the
                // body crossing the strip's own edge.
                let (top, bottom) = match self.tab_strip_position {
                    TabStripPosition::North => {
                        (strip.y + strip.height as i32, rect.y + rect.height as i32)
                    }
                    TabStripPosition::South => (rect.y, strip.y),
                };
                Some(SubwindowPlacement {
                    frame: Rect::new(rect.x, top, rect.width, (bottom - top).max(0) as u32),
                    title_bar: false,
                })
            }
        }
    }

    /// Returns background.
    pub fn background(&self) -> Background {
        self.background
    }

    /// The viewport pan applied to sub-window frames in `SubWindowView`.
    pub fn scroll_offset(&self) -> Point {
        self.scroll_offset
    }

    /// Sets the viewport pan.
    ///
    /// The stored geometries do not move: only the frames the painter and the hit test see
    /// are translated, so a pan is reversible and a window dragged while panned still lands
    /// where the pointer puts it.
    pub fn set_scroll_offset(&mut self, offset: Point) {
        // C-008: the pan is bounded by the content's bounding box, so a gesture cannot drag the
        // whole desktop off the viewport and leave an empty area with no way back.
        let offset = self.clamp_scroll_offset(offset);
        if self.scroll_offset == offset {
            return;
        }
        self.scroll_offset = offset;
        self.base.request_redraw();
    }

    /// Clamps `offset` so the content bounding box still covers the viewport where it can.
    ///
    /// The bounds are the union of the stored sub-window rectangles. When the content is smaller
    /// than the area on an axis the only legal pan is zero on that axis; when it is larger, the
    /// offset may range from the content's leading edge to its trailing edge minus the viewport.
    fn clamp_scroll_offset(&self, offset: Point) -> Point {
        let Some(bounds) =
            self.subwindows.iter().map(|subwindow| subwindow.geometry).reduce(|a, b| {
                let x = a.x.min(b.x);
                let y = a.y.min(b.y);
                let right = (a.x + a.width as i32).max(b.x + b.width as i32);
                let bottom = (a.y + a.height as i32).max(b.y + b.height as i32);
                Rect::new(x, y, (right - x).max(0) as u32, (bottom - y).max(0) as u32)
            })
        else {
            return Point::new(0, 0);
        };
        let area = self.geometry();
        // C-008: when the content is at least as large as the viewport on an axis, the pan may
        // range until the content exactly covers the viewport. When it is smaller, the pan may
        // only move it while the whole content stays inside the viewport. Either way a gesture
        // cannot leave an empty area with no way back.
        let (min_x, max_x) = if bounds.width >= area.width {
            (bounds.x - area.x, bounds.x + bounds.width as i32 - area.x - area.width as i32)
        } else {
            (bounds.x + bounds.width as i32 - area.x - area.width as i32, bounds.x - area.x)
        };
        let (min_y, max_y) = if bounds.height >= area.height {
            (bounds.y - area.y, bounds.y + bounds.height as i32 - area.y - area.height as i32)
        } else {
            (bounds.y + bounds.height as i32 - area.y - area.height as i32, bounds.y - area.y)
        };
        Point::new(offset.x.clamp(min_x, max_x), offset.y.clamp(min_y, max_y))
    }

    /// Pans the work area by `(dx, dy)`.
    pub fn scroll_by(&mut self, dx: i32, dy: i32) {
        self.set_scroll_offset(Point::new(
            self.scroll_offset.x.saturating_add(dx),
            self.scroll_offset.y.saturating_add(dy),
        ));
    }

    /// Translates a stored frame into the panned viewport.
    fn view_frame(&self, frame: Rect) -> Rect {
        Rect::new(
            frame.x.saturating_sub(self.scroll_offset.x),
            frame.y.saturating_sub(self.scroll_offset.y),
            frame.width,
            frame.height,
        )
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
        if self.activation_order == order {
            return;
        }
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
        // C-006: arrangement changes are visible state, so the frame is owed.
        let clamped = self.clamp_scroll_offset(self.scroll_offset);
        self.scroll_offset = clamped;
        self.base.request_redraw();
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
        // C-006: arrangement changes are visible state, so the frame is owed.
        let clamped = self.clamp_scroll_offset(self.scroll_offset);
        self.scroll_offset = clamped;
        self.base.request_redraw();
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
            .floor()
            .max(1.0) as usize;
        let _rows = count.div_ceil(cols);
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
    ///
    /// The order traversed is [`Self::activation_order`], not the storage order: `CreationOrder`
    /// walks the windows in the order they were added, `StackingOrder` from bottom to top by
    /// `z_order`, and `HistoryOrder` from the most recently activated window backwards through the
    /// activation history. Before this the field was stored and reported but every traversal used
    /// the array index, so the enum named a behaviour nothing performed.
    pub fn activate_next_sub_window(&mut self) {
        let cycle = self.activation_cycle();
        if cycle.is_empty() {
            return;
        }
        let current = self.active_subwindow.unwrap_or(cycle[0]);
        let position = cycle.iter().position(|index| *index == current).unwrap_or(0);
        let next = cycle[(position + 1) % cycle.len()];
        self.set_active_sub_window(self.subwindows[next].widget);
    }
    /// Activates previous sub-window.
    ///
    /// Walks the same [`Self::activation_order`] cycle as [`Self::activate_next_sub_window`] in the
    /// opposite direction. Under `HistoryOrder` activating a window moves it to the head of the
    /// cycle, so the two are not strict inverses of each other: previous from the most recently
    /// activated window wraps to the oldest entry.
    pub fn activate_previous_sub_window(&mut self) {
        let cycle = self.activation_cycle();
        if cycle.is_empty() {
            return;
        }
        let current = self.active_subwindow.unwrap_or(cycle[0]);
        let position = cycle.iter().position(|index| *index == current).unwrap_or(0);
        let previous = cycle[(position + cycle.len() - 1) % cycle.len()];
        self.set_active_sub_window(self.subwindows[previous].widget);
    }

    /// The sub-window indices in the order activation traverses them.
    fn activation_cycle(&self) -> Vec<usize> {
        match self.activation_order {
            ActivationOrder::CreationOrder => (0..self.subwindows.len()).collect(),
            ActivationOrder::StackingOrder => {
                let mut cycle: Vec<usize> = (0..self.subwindows.len()).collect();
                cycle.sort_by_key(|index| (self.subwindows[*index].z_order, *index));
                cycle
            }
            ActivationOrder::HistoryOrder => {
                // Most recent first: the head is the window the user just left, which is what
                // "next" should return -- an activation history that walked oldest-first would
                // answer the *least* recently used window, the opposite of the name.
                self.activation_history
                    .iter()
                    .rev()
                    .filter_map(|id| self.subwindows.iter().position(|sw| sw.widget == *id))
                    .collect()
            }
        }
    }

    /// Maximises or restores a sub-window, returning whether anything changed.
    ///
    /// A maximized sub-window fills the area and remembers the rectangle it came from, so a
    /// second toggle puts it back where it was. This is what a title-bar double-click calls; making
    /// it public gives a host the same transition without synthesising pointer events.
    pub fn toggle_maximized_sub_window(&mut self, widget: ObjectId) -> bool {
        let Some(index) = self.subwindows.iter().position(|sw| sw.widget == widget) else {
            return false;
        };
        if self.subwindows[index].maximized {
            self.subwindows[index].maximized = false;
            if let Some((maximized, rect)) = self.maximized_restore.take() {
                if maximized == index {
                    self.subwindows[index].geometry = rect;
                } else {
                    self.maximized_restore = Some((maximized, rect));
                }
            }
        } else {
            let previous = self.subwindows[index].geometry;
            self.maximized_restore = Some((index, previous));
            self.subwindows[index].maximized = true;
            self.subwindows[index].geometry = self.geometry();
        }
        self.set_active_sub_window(widget);
        self.base.request_redraw();
        true
    }

    /// Minimizes or restores sub-window `widget` (C-15).
    ///
    /// Returns `false` for a widget this area does not hold. Minimizing clears the maximized flag
    /// (a window cannot be both) and re-arranges the minimized icons, so the window takes its icon
    /// slot immediately. Restoring raises the window again. The change is announced through
    /// [`Self::subwindow_minimized_changed`].
    pub fn set_sub_window_minimized(&mut self, widget: ObjectId, minimized: bool) -> bool {
        let Some(index) = self.subwindows.iter().position(|sw| sw.widget == widget) else {
            return false;
        };
        if self.subwindows[index].minimized == minimized {
            return true;
        }
        self.subwindows[index].minimized = minimized;
        if minimized {
            self.subwindows[index].maximized = false;
            // A minimized window that owned the maximized restore slot must not hand its stored
            // rectangle back when another window is restored.
            if matches!(self.maximized_restore, Some((maximized, _)) if maximized == index) {
                self.maximized_restore = None;
            }
            self.arrange_icons();
        } else {
            self.set_active_sub_window(widget);
        }
        self.subwindow_minimized_changed.emit((widget, minimized));
        self.base.request_redraw();
        true
    }

    /// Whether sub-window `widget` is minimized.
    pub fn is_sub_window_minimized(&self, widget: ObjectId) -> bool {
        self.subwindows.iter().find(|sw| sw.widget == widget).is_some_and(|sw| sw.minimized)
    }

    /// The sub-window whose title bar contains `pos`, topmost first.
    ///
    /// Only the sub-window presentation draws title bars; in `TabbedView` the strip names the
    /// windows, so there is no bar to double-click.
    fn title_bar_at(&self, pos: Point) -> Option<usize> {
        if self.view_mode != ViewMode::SubWindowView {
            return None;
        }
        self.z_order_indices().iter().rev().copied().find(|index| {
            let frame = self.view_frame(self.subwindows[*index].geometry);
            let bar = Rect::new(
                frame.x,
                frame.y,
                frame.width,
                TITLE_BAR_HEIGHT.min(frame.height as i32).max(0) as u32,
            );
            bar.contains_point(pos)
        })
    }

    /// The sub-window indices in paint order — lowest z-order first — cached (C-P08).
    ///
    /// The z-order changes only when a window is added, removed or raised, yet the hit chain
    /// (`close_button_at`, `resize_edges_at`, `sub_window_at_position`) and the painter each built
    /// and sorted the index list on every pointer event and every frame. One cached list feeds all
    /// four, so the cost is paid on a real order change only.
    fn z_order_indices(&self) -> Rc<Vec<usize>> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for subwindow in &self.subwindows {
            subwindow.z_order.hash(&mut hasher);
            subwindow.widget.hash(&mut hasher);
        }
        let fingerprint = hasher.finish();
        if let Some((cached, indices)) = self.z_cache.borrow().as_ref() {
            if *cached == fingerprint {
                return Rc::clone(indices);
            }
        }
        let mut indices: Vec<usize> = (0..self.subwindows.len()).collect();
        indices.sort_by_key(|&index| self.subwindows[index].z_order);
        let indices = Rc::new(indices);
        *self.z_cache.borrow_mut() = Some((fingerprint, Rc::clone(&indices)));
        indices
    }

    /// The topmost resizable sub-window edge band under `pos`, if any.
    fn resize_edges_at(&self, pos: Point) -> Option<(usize, ResizeEdges)> {
        if self.view_mode != ViewMode::SubWindowView {
            return None;
        }
        for index in self.z_order_indices().iter().rev().copied() {
            let subwindow = &self.subwindows[index];
            if !subwindow.resizable || subwindow.maximized || subwindow.minimized {
                continue;
            }
            let frame = self.view_frame(subwindow.geometry);
            if !frame.contains_point(pos) {
                continue;
            }
            let right = frame.x + frame.width as i32;
            let bottom = frame.y + frame.height as i32;
            let edges = ResizeEdges {
                left: pos.x - frame.x < RESIZE_BAND,
                right: right - pos.x <= RESIZE_BAND,
                top: pos.y - frame.y < RESIZE_BAND,
                bottom: bottom - pos.y <= RESIZE_BAND,
            };
            // The topmost window owns the point, so a press that is not on its edge is not a
            // resize of a window *below* it either.
            return edges.any().then_some((index, edges));
        }
        None
    }

    /// Moves the edges of the sub-window being resized to follow `pos`.
    fn drag_resize(&mut self, pos: Point) {
        let Some(drag) = self.resize_drag else {
            return;
        };
        let Some(subwindow) = self.subwindows.get_mut(drag.index) else {
            self.resize_drag = None;
            return;
        };
        let dx = pos.x - drag.origin.x;
        let dy = pos.y - drag.origin.y;
        let mut x = drag.geometry.x;
        let mut y = drag.geometry.y;
        let mut width = drag.geometry.width as i32;
        let mut height = drag.geometry.height as i32;
        if drag.edges.left {
            let candidate = width - dx;
            if candidate >= MIN_SUBWINDOW_WIDTH as i32 {
                x = drag.geometry.x + dx;
                width = candidate;
            }
        }
        if drag.edges.right {
            width = (width + dx).max(MIN_SUBWINDOW_WIDTH as i32);
        }
        if drag.edges.top {
            let candidate = height - dy;
            if candidate >= MIN_SUBWINDOW_HEIGHT as i32 {
                y = drag.geometry.y + dy;
                height = candidate;
            }
        }
        if drag.edges.bottom {
            height = (height + dy).max(MIN_SUBWINDOW_HEIGHT as i32);
        }
        subwindow.geometry = Rect::new(x, y, width as u32, height as u32);
        self.base.request_redraw();
    }

    /// Moves the sub-window whose title bar is being dragged (C-14).
    ///
    /// The delta is measured from the press position against the geometry snapshot, so the move is
    /// absolute and a re-delivered pointer sample lands in the same place. The window is clamped so
    /// at least a title bar's worth stays inside the area: a window dragged fully off the desktop
    /// would be unreachable with the pointer.
    fn drag_move(&mut self, pos: Point) {
        let Some(drag) = self.move_drag else {
            return;
        };
        let area = self.geometry();
        let Some(subwindow) = self.subwindows.get_mut(drag.index) else {
            self.move_drag = None;
            return;
        };
        if subwindow.maximized {
            return;
        }
        let dx = pos.x - drag.origin.x;
        let dy = pos.y - drag.origin.y;
        let keep = TITLE_BAR_HEIGHT;
        let min_x = area.x - drag.geometry.width as i32 + keep;
        let max_x = area.x + area.width as i32 - keep;
        let min_y = area.y;
        let max_y = area.y + area.height as i32 - keep;
        let raw_x = drag.geometry.x.saturating_add(dx);
        let raw_y = drag.geometry.y.saturating_add(dy);
        let x = if min_x <= max_x { raw_x.clamp(min_x, max_x) } else { min_x };
        let y = if min_y <= max_y { raw_y.clamp(min_y, max_y) } else { min_y };
        subwindow.geometry = Rect::new(x, y, drag.geometry.width, drag.geometry.height);
        self.base.request_redraw();
    }

    /// The sub-window whose minimize/restore button is under `pos` (C-15), topmost first.
    fn minimize_button_at(&self, pos: Point) -> Option<usize> {
        for index in self.z_order_indices().iter().rev().copied() {
            let Some(placement) = self.subwindow_placement(index) else {
                continue;
            };
            if !placement.title_bar {
                continue;
            }
            if minimize_button_rect(placement.frame).contains_point(pos) {
                return Some(index);
            }
        }
        None
    }

    /// Reorders the tab strip when the carried tab is dragged onto another slot (C-17).
    ///
    /// The sub-window records move within [`Self::subwindows`]; their `z_order` values travel with
    /// them, so stacking and activation relations are preserved. The selection is re-resolved by
    /// widget id, and the index-keyed maximized-restore slot is re-resolved the same way.
    fn drag_tab_to(&mut self, pos: Point) {
        let Some((from, origin)) = self.tab_drag else {
            return;
        };
        let Some(to) = self.tab_at_point(pos) else {
            return;
        };
        if from == to || from >= self.subwindows.len() || to >= self.subwindows.len() {
            return;
        }
        let active_widget = self
            .active_subwindow
            .and_then(|index| self.subwindows.get(index))
            .map(|subwindow| subwindow.widget);
        let maximized_widget = self
            .maximized_restore
            .and_then(|(index, _)| self.subwindows.get(index))
            .map(|subwindow| subwindow.widget);
        let maximized_rect = self.maximized_restore.map(|(_, rect)| rect);
        let moved = self.subwindows.remove(from);
        self.subwindows.insert(to, moved);
        self.active_subwindow = active_widget
            .and_then(|widget| self.subwindows.iter().position(|sw| sw.widget == widget));
        self.maximized_restore = maximized_widget.and_then(|widget| {
            let index = self.subwindows.iter().position(|sw| sw.widget == widget)?;
            Some((index, maximized_rect?))
        });
        self.tab_drag = Some((to, origin));
        self.subwindow_tab_moved.emit((from, to));
        self.base.request_redraw();
    }
    /// The index of the sub-window whose close button is under `pos` (C-002).
    ///
    /// In `SubWindowView` the button is chrome of a *frame*, so the test walks the frames in
    /// paint order (highest z-order first). In `TabbedView` there is no per-frame title bar;
    /// the affordance is the ✕ drawn on each drawn tab, so that path is answered first.
    fn close_button_at(&self, pos: Point) -> Option<usize> {
        if self.view_mode == ViewMode::TabbedView {
            return self.tab_close_button_at(pos);
        }
        for index in self.z_order_indices().iter().rev().copied() {
            if !self.subwindows[index].closable {
                continue;
            }
            let Some(placement) = self.subwindow_placement(index) else {
                continue;
            };
            if !placement.title_bar {
                continue;
            }
            if close_button_rect(placement.frame).contains_point(pos) {
                return Some(index);
            }
        }
        None
    }

    /// The tab whose drawn close button is under `pos` (C-002).
    ///
    /// Built on [`Self::tab_rect`], the same forward geometry the paint uses, so the ✕ a user
    /// sees is the ✕ that hit-tests. A non-closable window's tab carries no button.
    fn tab_close_button_at(&self, pos: Point) -> Option<usize> {
        if self.view_mode != ViewMode::TabbedView {
            return None;
        }
        (0..self.subwindows.len()).find(|index| {
            self.subwindows[*index].closable
                && self.tab_rect(*index).is_some_and(|tab| tab_close_rect(tab).contains_point(pos))
        })
    }
    /// Returns sub-window at position.
    fn sub_window_at_position(&self, pos: Point) -> Option<usize> {
        // Check from top (highest z-order) to bottom
        for index in self.z_order_indices().iter().rev().copied() {
            let subwindow = &self.subwindows[index];
            if self.view_frame(subwindow.geometry).contains(pos) {
                return Some(index);
            }
        }
        None
    }
}
impl SaveState for MdiArea {
    fn state_version(&self) -> u16 {
        MDI_STATE_VERSION as u16
    }

    fn save_state(&self) -> Vec<u8> {
        MdiArea::save_state(self)
    }

    fn restore_state(&mut self, state: &[u8]) -> bool {
        MdiArea::restore_state(self, state)
    }
}

// Implement Widget trait
impl Widget for MdiArea {
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

    /// Moves the work area and keeps a maximized sub-window filling it (C-007).
    ///
    /// A maximized window's stored geometry *is* the area at the moment it was maximized. Left
    /// alone on a resize, it kept the old rectangle and left a gap (or overflowed) until the user
    /// restored and maximized it again.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        for subwindow in &mut self.subwindows {
            if subwindow.maximized {
                subwindow.geometry = geometry;
            }
        }
        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset);
    }

    fn size_hint(&self) -> crate::core::Size {
        // C-019: the active sub-window's control is the content this area shows, so its size hint
        // is the area's wish. With no mounted control the historical panel default is the floor.
        let measured = self.active_sub_window().and_then(|widget| {
            crate::widget::runtime::with_widget(widget, |widget| widget.size_hint())
        });
        measured.unwrap_or_else(|| {
            crate::core::Size::new(
                crate::widget::metrics::dimensions::PANEL_DEFAULT_WIDTH,
                crate::widget::metrics::dimensions::PANEL_DEFAULT_HEIGHT,
            )
        })
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
            "subwindow_activated" => Some(EventSignalRef::mapped(
                "subwindow_activated",
                &self.subwindow_activated,
                |v| CapabilityValue::UInt(*v),
            )),
            "subwindow_title_changed" => Some(EventSignalRef::mapped(
                "subwindow_title_changed",
                &self.subwindow_title_changed,
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0),
                        CapabilityValue::String(v.1.clone()),
                    ]))
                },
            )),
            "subwindow_minimized_changed" => Some(EventSignalRef::mapped(
                "subwindow_minimized_changed",
                &self.subwindow_minimized_changed,
                // `(id, minimized)` — an object id travels as `UInt`, the flag as `Bool`.
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0),
                        CapabilityValue::Bool(v.1),
                    ]))
                },
            )),
            "subwindow_tab_moved" => Some(EventSignalRef::mapped(
                "subwindow_tab_moved",
                &self.subwindow_tab_moved,
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                },
            )),
            _ => None,
        }
    }
}

/// `MdiArea`'s property contract.
///
/// `view_mode` uses this module's own [`ViewMode`] (sub-windows vs. tabs), not
/// `list_view::ViewMode`; the published tokens are the two this widget's reader
/// produced and nothing else.
impl WidgetProperties for MdiArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "subwindow_count" => Ok(CapabilityValue::UInt(self.sub_window_count() as u64)),
            "active_subwindow" => match self.active_sub_window() {
                Some(id) => Ok(CapabilityValue::UInt(id)),
                None => Ok(CapabilityValue::Null),
            },
            "view_mode" => {
                let token = match self.view_mode() {
                    ViewMode::SubWindowView => "sub_window_view",
                    ViewMode::TabbedView => "tabbed",
                };
                Ok(CapabilityValue::String(token.to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Two are read-only: the reader serves them and the writer refuses, so
            // the contract says so explicitly.
            "subwindow_count" | "active_subwindow" => Err(CapabilityAccessError::ReadOnlyProperty),
            // The view mode is a real setter on the control (`set_view_mode`, the entry point
            // Qt's `QMdiArea::setViewMode` answers), so the property route runs it rather than
            // refusing a write the control knows how to honor.
            "view_mode" => {
                let token = expect_string(value)?;
                let mode = match token.as_str() {
                    "sub_window_view" => ViewMode::SubWindowView,
                    "tabbed" => ViewMode::TabbedView,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_view_mode(mode);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    /// Returns the names this control serves through the property contract.
    ///
    /// Every name the reader answers is listed, including the two the writer
    /// refuses: the readable set — not the schema flag — is what callers can
    /// actually use.
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["subwindow_count", "active_subwindow", "view_mode", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `mdi_area` publishes.
    ///
    /// All three address a sub-window by its `ObjectId` (or, for `add_subwindow`, a
    /// `Rect` alongside it), so none can run on the bare name: there is no sub-window
    /// the control could pick that the caller did not name, and activating an
    /// arbitrary one would move focus the caller never asked to move. They are all
    /// [`CapabilityAccessError::OutOfRange`] — valid names whose arguments are what
    /// is missing. `set_view_mode` is a payload-carrying write published by the same
    /// capability and is refused the same way; an override that named only the
    /// non-`set_` commands dropped the trait default's `set_` acceptance, which is
    /// what the final arm restores.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_subwindow" | "remove_subwindow" | "activate_subwindow" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MdiArea {
    /// Sub-window activation and event forwarding stop while the area is disabled.
    ///
    /// Without the guard a disabled MDI area still raised whichever sub-window was clicked and
    /// forwarded the event into it, so `set_enabled(false)` did not take the workspace out of play.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        let mut hit_subwindow = false;
        let mut gesture_consumed = false;
        if let Event::MousePress { pos, button, .. } = event {
            if *button == 1 {
                // In `TabbedView` the tabs are the only clickable chrome, because the windows they
                // name carry no title bar of their own and all sit in the same body. Resolving the
                // tab first is what makes the strip interactive rather than decorative -- a mode
                // whose tabs cannot be clicked is the dead state this whole change closes.
                if self.view_mode == ViewMode::TabbedView {
                    // C-002: the ✕ drawn on a tab is checked before the tab itself. Closing is
                    // the press's whole effect; arming a tab-reorder or activating the tab would
                    // be the wrong gesture for a click on the close affordance.
                    if let Some(index) = self.close_button_at(*pos) {
                        let widget = self.subwindows[index].widget;
                        self.remove_sub_window(widget);
                    } else if let Some(index) = self.tab_at_point(*pos) {
                        hit_subwindow = true;
                        self.set_active_sub_window(self.subwindows[index].widget);
                        // C-17: a press on a tab also arms a reorder. The reorder only begins once
                        // the pointer crosses the drag threshold, so selecting a tab stays a click.
                        self.tab_drag = (self.subwindows.len() > 1).then_some((index, *pos));
                    }
                } else if let Some(index) = self.close_button_at(*pos) {
                    // The ✕ closes the window it belongs to — which is what the glyph says and
                    // what `remove_sub_window` (the area's own public close path) does. The press
                    // is *consumed* here: `hit_subwindow` stays false so it is neither
                    // re-targeted at the window it just closed nor at whichever window the
                    // removal activated.
                    let widget = self.subwindows[index].widget;
                    self.remove_sub_window(widget);
                } else if let Some(index) = self.minimize_button_at(*pos) {
                    // C-15: the title-bar button next to ✕ toggles minimize/restore.
                    let widget = self.subwindows[index].widget;
                    let minimized = !self.subwindows[index].minimized;
                    self.set_sub_window_minimized(widget, minimized);
                } else if let Some((index, edges)) = self.resize_edges_at(*pos) {
                    // Grabbing a frame edge is the area's gesture, not the child's: it is the
                    // frame's geometry being changed, so the press must not reach the content.
                    let widget = self.subwindows[index].widget;
                    self.set_active_sub_window(widget);
                    self.resize_drag = Some(SubwindowResize {
                        index,
                        edges,
                        origin: *pos,
                        geometry: self.subwindows[index].geometry,
                    });
                    self.base.set_mouse_pressed(true);
                    gesture_consumed = true;
                } else if let Some(index) = self.sub_window_at_position(*pos) {
                    hit_subwindow = true;
                    self.set_active_sub_window(self.subwindows[index].widget);
                    // C-14: a press on the title bar (and not on either button, which are tested
                    // above) starts a move. A maximized window has no meaningful frame to move —
                    // it fills the area — so it is left out.
                    let frame = self.view_frame(self.subwindows[index].geometry);
                    let bar = Rect::new(
                        frame.x,
                        frame.y,
                        frame.width,
                        TITLE_BAR_HEIGHT.min(frame.height as i32).max(0) as u32,
                    );
                    if self.subwindows[index].movable
                        && !self.subwindows[index].maximized
                        && bar.contains_point(*pos)
                    {
                        self.move_drag = Some(SubwindowMove {
                            index,
                            origin: *pos,
                            geometry: self.subwindows[index].geometry,
                        });
                        self.base.set_mouse_pressed(true);
                    }
                }
            }
        } else if let Event::MouseDoubleClick { pos, button } = event {
            if *button == 1 {
                if let Some(index) = self.title_bar_at(*pos) {
                    let widget = self.subwindows[index].widget;
                    self.toggle_maximized_sub_window(widget);
                    gesture_consumed = true;
                }
            }
        } else if let Event::MouseMove { pos } = event {
            // C-002: track which tab's close button the pointer is over, so the drawn ✕ can
            // show its hover state. While a tab is being carried the strip's own reorder
            // feedback owns the picture, so hover is left alone.
            if self.view_mode == ViewMode::TabbedView && self.tab_drag.is_none() {
                let hovered = self.tab_close_button_at(*pos);
                if hovered != self.hovered_tab_close {
                    self.hovered_tab_close = hovered;
                    self.base.request_redraw();
                }
            }
            if self.resize_drag.is_some() {
                self.drag_resize(*pos);
                gesture_consumed = true;
            } else if self.move_drag.is_some() {
                self.drag_move(*pos);
                gesture_consumed = true;
            } else if let Some((_, origin)) = self.tab_drag {
                // C-17: cross the drag threshold before reordering, so a click stays a click.
                if (pos.x - origin.x).abs() > 4 || (pos.y - origin.y).abs() > 4 {
                    self.drag_tab_to(*pos);
                    gesture_consumed = true;
                }
            }
        } else if let Event::MouseRelease { button, .. } = event {
            if *button == 1 {
                if self.resize_drag.take().is_some() {
                    self.base.set_mouse_pressed(false);
                    gesture_consumed = true;
                }
                if self.move_drag.take().is_some() {
                    self.base.set_mouse_pressed(false);
                    gesture_consumed = true;
                }
                if self.tab_drag.take().is_some() {
                    gesture_consumed = true;
                }
            }
        } else if let Event::MouseLeave { .. } = event {
            // Leaving the area ends the hover, so a ✕ cannot stay highlighted after the pointer
            // has gone somewhere else.
            if self.hovered_tab_close.take().is_some() {
                self.base.request_redraw();
            }
        } else if let Event::Wheel { delta, .. } = event {
            // C-16: the wheel pans the work area, the same viewport offset the public `scroll_by`
            // drives. `TabbedView` shows one page with no desktop to pan, so it is ignored there.
            if self.view_mode == ViewMode::SubWindowView {
                self.scroll_by(
                    -delta.x.saturating_mul(WHEEL_PAN_STEP),
                    -delta.y.saturating_mul(WHEEL_PAN_STEP),
                );
                gesture_consumed = true;
            }
        }
        let should_forward = if gesture_consumed {
            false
        } else {
            !matches!(event, Event::MousePress { .. }) || hit_subwindow
        };
        // Forward background-independent events, or pointer events that hit a subwindow.
        if should_forward {
            if let Some(widget_id) = self.active_sub_window() {
                if let Some(ref reg) = self.registry {
                    reg.borrow_mut().forward_event(widget_id, event);
                }
            }
        }
    }
}
/// Side of a sub-window's close button, in logical pixels.
///
/// A constant so the title's reserved width and the button's own geometry are derived from
/// one number rather than two literals that have to be kept in step.
const CLOSE_SIZE: i32 = 12;

/// Width reserved at the trailing edge of a sub-window's title bar for the close button
/// and its margins. Used to bound the title so the two cannot overlap.
const CLOSE_SIZE_SUM: u32 = 22;

/// Inset of the close button from the frame's trailing edge, in logical pixels.
///
/// `CLOSE_SIZE_SUM` is `CLOSE_SIZE + 2 * CLOSE_BUTTON_INSET`, so the title's reserved band and
/// the button's own box are derived from the same layout instead of two literals.
const CLOSE_BUTTON_INSET: i32 = 5;

/// The close button's box inside a sub-window's title bar.
///
/// Shared by the painter and the hit test. The ✕ was painted for every closable sub-window but
/// nothing hit-tested it, so a click on it activated the window and changed nothing — a drawn
/// affordance with no handler. Both sides now read this one derivation, so the glyph and the
/// click area cannot drift.
fn close_button_rect(frame: Rect) -> Rect {
    let y = frame.y + (TITLE_BAR_HEIGHT - CLOSE_SIZE) / 2;
    Rect::new(
        frame.x + frame.width as i32 - CLOSE_SIZE - CLOSE_BUTTON_INSET,
        y,
        CLOSE_SIZE as u32,
        CLOSE_SIZE as u32,
    )
}

/// The close button's box inside a `TabbedView` tab (C-002).
///
/// Shared by the painter and [`MdiArea::tab_close_button_at`], so the glyph and its click area
/// cannot drift. It reuses the frame button's side and inset, which keeps the two presentations
/// of a close affordance visually consistent.
fn tab_close_rect(tab: Rect) -> Rect {
    let y = tab.y + (tab.height as i32 - CLOSE_SIZE) / 2;
    Rect::new(
        tab.x + tab.width as i32 - CLOSE_SIZE - CLOSE_BUTTON_INSET,
        y,
        CLOSE_SIZE as u32,
        CLOSE_SIZE as u32,
    )
}

/// Side of the minimize/restore button, in logical pixels (C-15).
const MINIMIZE_SIZE: i32 = 12;
/// Pixels the work area pans per wheel notch (C-16).
const WHEEL_PAN_STEP: i32 = 24;
/// Gap between the minimize/restore button and the close button.
const MINIMIZE_GAP: i32 = 4;

/// The minimize/restore button's box inside a sub-window's title bar (C-15).
///
/// Shared by the painter and the hit test, exactly as [`close_button_rect`] is, so the glyph and
/// the click area cannot drift.
fn minimize_button_rect(frame: Rect) -> Rect {
    let close = close_button_rect(frame);
    Rect::new(
        close.x - MINIMIZE_SIZE - MINIMIZE_GAP,
        frame.y + (TITLE_BAR_HEIGHT - MINIMIZE_SIZE) / 2,
        MINIMIZE_SIZE as u32,
        MINIMIZE_SIZE as u32,
    )
}

/// Height of a sub-window's title bar, in logical pixels.
const TITLE_BAR_HEIGHT: i32 = 24;

/// How close to an edge a press must land to begin an edge-resize drag, in logical pixels.
const RESIZE_BAND: i32 = 4;

/// The narrowest a sub-window may become through an edge drag, in logical pixels.
const MIN_SUBWINDOW_WIDTH: u32 = 60;

/// The shortest a sub-window may become through an edge drag, in logical pixels.
const MIN_SUBWINDOW_HEIGHT: u32 = 40;

/// Magic marker of the blob produced by [`MdiArea::save_state`] (C-001).
const MDI_STATE_MAGIC: &[u8; 4] = b"MDI1";
/// Version tag of that blob.
const MDI_STATE_VERSION: u8 = 1;
/// `magic(4) + version(1) + view mode(1) + count(4)`.
const MDI_STATE_HEADER: usize = 10;
/// `geometry(16) + z-order(4)` per sub-window.
const MDI_STATE_ENTRY: usize = 20;

/// Which edges of a sub-window an in-progress drag is moving.
#[derive(Debug, Clone, Copy, Default)]
struct ResizeEdges {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

impl ResizeEdges {
    /// Whether the gesture moves any edge at all.
    fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

/// The live edge-resize gesture.
#[derive(Debug, Clone, Copy)]
struct SubwindowResize {
    index: usize,
    edges: ResizeEdges,
    origin: Point,
    geometry: Rect,
}

/// The live title-bar move gesture (C-14).
///
/// The snapshot is the window's geometry when the press happened, so the move is absolute — a
/// replayed or coalesced pointer sample lands in the same place rather than doubling the delta.
#[derive(Clone, Copy)]
struct SubwindowMove {
    index: usize,
    /// The pointer position the gesture started at, in area coordinates.
    origin: Point,
    /// The window's stored geometry at that moment (not the panned frame).
    geometry: Rect,
}

/// Square side of the leading icon slot a sub-window's title bar reserves, in logical pixels.
const SUBWINDOW_ICON_SIDE: u32 = 12;
/// Gap between that icon slot and the title text.
const SUBWINDOW_ICON_GAP: u32 = 4;
/// Inset of the title bar's content from the frame's leading edge.
const TITLE_BAR_INSET: i32 = 5;

/// The tab strip's height in `ViewMode::TabbedView`, in logical pixels.
///
/// Two pixels taller than a sub-window's own title bar: a tab carries the title *and* sits above
/// the body it names, so it is chrome twice over and reads as a strip rather than as a detached
/// title bar.
const TAB_STRIP_HEIGHT: i32 = TITLE_BAR_HEIGHT + 2;

/// The widest a single tab may grow in `ViewMode::TabbedView`, in logical pixels.
///
/// A strip of one or two tabs would otherwise stretch each to half the work area, which reads as
/// a toolbar rather than as tabs. Past this a tab stops growing and the free space is left at the
/// strip's trailing edge.
const TAB_MAX_WIDTH: u32 = 160;

/// How far a tab's label is inset from the tab's own edges, in logical pixels.
const TAB_TEXT_INSET: i32 = 6;

/// How a sub-window is *placed* under the current [`ViewMode`], independent of what it contains.
///
/// Produced by `MdiArea::subwindow_placement` and read by the paint, so the two modes cannot be
/// half-applied -- the defect `list_view`'s own `ViewMode` had, where four modes were stored,
/// published and read by nothing. See that method for what each mode does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SubwindowPlacement {
    /// The sub-window's frame, in the work area's coordinates.
    frame: Rect,
    /// Whether the frame's own title bar is drawn.
    ///
    /// False in `TabbedView`, where the tab strip names the sub-window: drawing both would name
    /// the same window twice, one above the other.
    title_bar: bool,
}

/// Paints a sub-window's icon into `icon_rect`, in `color`.
///
/// One function for both presentations of a sub-window — the sub-window view's title bar and the
/// tabbed view's tab — so the two cannot disagree about which active form is drawn.
fn draw_subwindow_icon(
    context: &mut RenderContext,
    subwindow: &MdiSubWindow,
    icon_rect: Rect,
    color: Color,
) {
    if let Some(source) = subwindow.icon_source {
        if let Err(error) = crate::widget::paint_icon(context, source.into(), icon_rect, color) {
            log::warn!("sub-window icon could not be painted: {error:?}");
        }
    }
    #[cfg(feature = "image")]
    if subwindow.icon_source.is_none() {
        if let Some(image) = subwindow.icon.as_ref() {
            // `set_icon` normalises to RGBA8, so the whole image is borrowed here.
            if let crate::image::ImageData::Rgba8(pixels) = &image.inner.data {
                // The shared fit rule, as `SharedIconPixels::fit_into` uses, so every image this
                // crate draws into an icon slot is fitted the same way.
                let fitted = crate::widget::icon_source::fit_image_into(
                    image.width(),
                    image.height(),
                    icon_rect,
                );
                // A fitted slot is a different rectangle from the source in general, so the draw
                // resamples rather than blitting (which would crop or paint nothing).
                context.draw_image_scaled(
                    fitted.x,
                    fitted.y,
                    fitted.width,
                    fitted.height,
                    image.width(),
                    image.height(),
                    pixels,
                );
            }
        }
    }
}

impl Draw for MdiArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // A title write performed through a sub-window record marks this flag instead of borrowing
        // the area; consuming it here schedules the frame that shows the new text.
        if self.title_dirty.replace(false) {
            self.base.request_redraw();
        }
        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, and the
        // plain background in particular was `rgb(240,240,240)` — byte-identical to the light
        // frame fill, so the area painted plenty of pixels and showed none of them. The census
        // reported it as both `ink = 0` and theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant. `ThemeSnapshot` folds the resolved style and the palette roles into
        // one visit for the frame (C-P01).
        let theme = ThemeSnapshot::capture("mdi_area");
        let style = self.base.style();
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = match theme.palette {
            Some(active) => {
                (active.background, active.foreground, active.secondary, active.primary)
            }
            None => (
                Color::rgb(240, 240, 240),
                Color::BLACK,
                Color::rgb(158, 158, 158),
                Color::rgb(33, 150, 243),
            ),
        };
        let ink = style
            .text_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `mdi_area` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`. A
        // work area painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let area = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != area)
            .unwrap_or_else(|| area.blend(&secondary, 0.45));

        // Draw base widget
        let rect = self.geometry();
        // Draw background
        match self.background {
            Background::NoBackground => {
                // No background
            }
            Background::Plain => {
                context.face_with_gradient_corners(
                    rect,
                    area,
                    self.style().background_gradient.as_ref(),
                    self.style().surface.unwrap_or_default(),
                    self.style().resolved_corner_radii().to_array(),
                    Color::BLACK,
                );
            }
            Background::Gradient => {
                // C-P09: one gradient face instead of one `draw_line` per pixel row. The ramp is
                // the same themed pair the row loop used, described once to the renderer.
                let top = area;
                let bottom = area.blend(&border, 0.55);
                let ramp = crate::style::Gradient::linear(
                    Point::new(rect.x, rect.y),
                    Point::new(rect.x, rect.y + rect.height as i32),
                )
                .with_stops(crate::compat::vec![
                    crate::style::GradientStop::new(0.0, top),
                    crate::style::GradientStop::new(1.0, bottom),
                ]);
                context.face_with_gradient_corners(
                    rect,
                    top,
                    Some(&ramp),
                    self.style().surface.unwrap_or_default(),
                    self.style().resolved_corner_radii().to_array(),
                    Color::BLACK,
                );
            }
            Background::Pattern => {
                // C-P09: a base fill plus one line per pattern step. The old form emitted a
                // `fill_rect` per 20 px cell, i.e. hundreds to thousands of commands per frame
                // for an unchanging result; a weave needs `rows + columns` lines instead.
                //
                // The base fill comes first. The lines below truncate to whole steps, so without
                // it the right and bottom remainders kept whatever the parent had painted, and an
                // area narrower or shorter than one step painted *nothing*.
                let pattern_size = 20;
                let alt = area.blend(&ink, 0.06);
                context.fill_rect(rect, area);
                let bottom = rect.y + rect.height as i32;
                let right = rect.x + rect.width as i32;
                // Every other row and every other column of the weave, offset by half a step so
                // the grid reads as a pattern rather than as a border.
                let mut y = rect.y + pattern_size / 2;
                while y < bottom {
                    context.draw_line(Point::new(rect.x, y), Point::new(right, y), alt);
                    y += pattern_size;
                }
                let mut x = rect.x + pattern_size / 2;
                while x < right {
                    context.draw_line(Point::new(x, rect.y), Point::new(x, bottom), alt);
                    x += pattern_size;
                }
            }
        }
        // In `TabbedView` the tab strip is the mode's whole visible difference at rest, so it is
        // drawn before the body: the active tab is the one whose window is about to be painted
        // below it, and painting the strip first keeps the body's own frame from covering it.
        if self.view_mode == ViewMode::TabbedView {
            self.draw_tab_strip(context, &area, &border, &ink, &primary);
        }
        // Draw sub-windows in paint order (lowest z-order first, so the highest draws last).
        // The order is cached, so a frame that did not raise a window does not re-sort (C-P08).
        for index in self.z_order_indices().iter().copied() {
            let Some(placement) = self.subwindow_placement(index) else {
                continue;
            };
            let subwindow = &self.subwindows[index];
            let is_active = self.active_subwindow == Some(index);
            // Draw sub-window frame
            let frame_rect = placement.frame;
            // Draw frame background
            let bg_color = if is_active { area.blend(&ink, 0.22) } else { area.blend(&ink, 0.14) };
            context.fill_rect(frame_rect, bg_color);
            // Draw frame border
            let border_color = if is_active { primary } else { border };
            context.draw_rect(frame_rect, border_color);
            // Draw title bar. `TabbedView` places the window under a tab that already names it, so
            // the frame draws no title bar of its own -- and, crucially, its content gets the whole
            // frame rather than the frame minus a bar nobody drew.
            let title_bar_height = if placement.title_bar { TITLE_BAR_HEIGHT } else { 0 };
            if placement.title_bar {
                let title_bar_color =
                    if is_active { primary } else { area.blend(&secondary, 0.55) };
                context.fill_rect(
                    Rect::new(
                        frame_rect.x,
                        frame_rect.y,
                        frame_rect.width,
                        title_bar_height as u32,
                    ),
                    title_bar_color,
                );
                // Draw title text. Centred on the **title bar's own line box**, not on
                // `title_bar_height / 2`: the latter put the glyph box's top edge on the bar's
                // middle line, so a 14 px label occupied `12..26` inside a 24 px bar and the
                // descenders crossed the bottom border. The close button below already used the
                // correct `(bar - size) / 2` form — one bar, two rules — so both now share this
                // line box.
                let title_bar_rect = Rect::new(
                    frame_rect.x,
                    frame_rect.y,
                    frame_rect.width,
                    title_bar_height as u32,
                );
                // The **effective font** — the resolved theme/caller font — for the subwindow title
                // (D09-STYLE-01).
                let title_font = effective_font(style);
                let title_line = context.text_line(title_bar_rect, title_font);
                let text_color = if is_active { primary.contrast_color() } else { ink };
                // ── Icon ──
                //
                // A sub-window with an icon reserves a leading square inside the title bar, and the
                // title band starts after it. The title bar is a fixed height and the text is
                // left-aligned, so the slot is the only thing that moves.
                let mut title_x = frame_rect.x + TITLE_BAR_INSET;
                let mut title_width =
                    title_bar_rect.width.saturating_sub(TITLE_BAR_INSET as u32 + CLOSE_SIZE_SUM);
                if subwindow.has_icon() {
                    let side = SUBWINDOW_ICON_SIDE.min(title_line.height.max(1));
                    let icon_rect = Rect {
                        x: frame_rect.x + TITLE_BAR_INSET,
                        y: title_line.y + (title_line.height.saturating_sub(side) / 2) as i32,
                        width: side,
                        height: side,
                    };
                    draw_subwindow_icon(context, subwindow, icon_rect, text_color);
                    title_x += (SUBWINDOW_ICON_SIDE + SUBWINDOW_ICON_GAP) as i32;
                    title_width =
                        title_width.saturating_sub(SUBWINDOW_ICON_SIDE + SUBWINDOW_ICON_GAP);
                }
                context.draw_text_fitted_cow(
                    Rect {
                        x: title_x,
                        y: title_line.y,
                        width: title_width,
                        height: title_line.height,
                    },
                    &subwindow.title,
                    title_font,
                    text_color,
                    HorizontalAlignment::Left,
                );
                // Draw close button if closable
                if subwindow.closable {
                    // One geometry for the ✕ and for the click that dismisses it: the button used
                    // to be positioned from the title's line box here and not hit-tested at all.
                    let close = close_button_rect(frame_rect);
                    let (close_x, close_y) = (close.x, close.y);
                    let close_size = CLOSE_SIZE;
                    let close_color = if is_active {
                        primary.contrast_color()
                    } else {
                        ink.blend(&title_bar_color, 0.35)
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
                // C-15: minimize/restore button, left of the close button. A minimized window
                // shows a restore square; an expanded or maximized one shows the minimize rule.
                {
                    let button = minimize_button_rect(frame_rect);
                    let button_color = if is_active {
                        primary.contrast_color()
                    } else {
                        ink.blend(&title_bar_color, 0.35)
                    };
                    if subwindow.minimized {
                        context.draw_rect(
                            Rect::new(
                                button.x + 2,
                                button.y + 2,
                                button.width.saturating_sub(4),
                                button.height.saturating_sub(4),
                            ),
                            button_color,
                        );
                    } else {
                        context.draw_line(
                            Point::new(button.x + 2, button.y + button.height as i32 - 3),
                            Point::new(
                                button.x + button.width as i32 - 2,
                                button.y + button.height as i32 - 3,
                            ),
                            button_color,
                        );
                    }
                }
            }
            // Draw widget content via registry
            let content_rect = Rect::new(
                frame_rect.x,
                frame_rect.y + title_bar_height,
                frame_rect.width,
                frame_rect.height.saturating_sub(title_bar_height as u32),
            );
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(subwindow.widget, content_rect);
                context.push_clip(
                    content_rect.x,
                    content_rect.y,
                    content_rect.width,
                    content_rect.height,
                );
                reg.borrow_mut().draw_widget(subwindow.widget, context);
                context.pop_clip();
            }
        }
    }
}

impl MdiArea {
    /// Paints the `TabbedView` tab strip: one tab per sub-window, the active one raised.
    ///
    /// Every tab's box comes from [`Self::tab_rect`], which is also what [`Self::tab_at_point`]
    /// resolves a click through, so the tab that is highlighted is the tab a click would activate.
    /// A tab past the strip's trailing edge is skipped by `tab_rect` returning `None`, and the loop
    /// stops rather than drawing a truncated tab.
    fn draw_tab_strip(
        &self,
        context: &mut RenderContext,
        area: &Color,
        border: &Color,
        ink: &Color,
        primary: &Color,
    ) {
        let strip = self.tab_strip_rect();
        context.fill_rect(strip, area.blend(ink, 0.06));
        // The rule sits on the edge the body is on, so a south strip separates from the body
        // above it rather than painting a line along the control's outer edge.
        let rule_y = match self.tab_strip_position {
            TabStripPosition::North => strip.y + strip.height as i32 - 1,
            TabStripPosition::South => strip.y,
        };
        context.draw_line(
            Point::new(strip.x, rule_y),
            Point::new(strip.x + strip.width as i32, rule_y),
            *border,
        );
        // The **effective font** — the resolved theme/caller font — for the tab labels
        // (D09-STYLE-01).
        let font = effective_font(self.style());
        for index in 0..self.subwindows.len() {
            let Some(tab) = self.tab_rect(index) else {
                break;
            };
            let is_active = self.active_subwindow == Some(index);
            // The active tab is drawn in the accent and the rest in the strip's own surface, which
            // is what makes "which window am I looking at" answerable without reading the title.
            let fill = if is_active { *primary } else { area.blend(ink, 0.12) };
            context.fill_rect(tab, fill);
            context.draw_rect(tab, *border);
            let text_color = if is_active { primary.contrast_color() } else { *ink };
            // The label's band is the tab minus its own padding, so a long title is ellipsised by
            // `draw_text_fitted` rather than running into the next tab. A tab for a window with an
            // icon gives up a leading square first, so the icon and the title share the tab the same
            // way they share the title bar in the sub-window view.
            let inset = TAB_TEXT_INSET.min(tab.width as i32 / 2);
            let mut label = Rect::new(
                tab.x + inset,
                tab.y,
                tab.width.saturating_sub(inset as u32 * 2),
                tab.height,
            );
            let subwindow = &self.subwindows[index];
            if subwindow.has_icon() {
                let line = context.text_line(label, font);
                let side = SUBWINDOW_ICON_SIDE.min(line.height.max(1));
                let icon_rect = Rect {
                    x: label.x,
                    y: line.y + (line.height.saturating_sub(side) / 2) as i32,
                    width: side,
                    height: side,
                };
                draw_subwindow_icon(context, subwindow, icon_rect, text_color);
                label.x += (SUBWINDOW_ICON_SIDE + SUBWINDOW_ICON_GAP) as i32;
                label.width = label.width.saturating_sub(SUBWINDOW_ICON_SIDE + SUBWINDOW_ICON_GAP);
            }
            // C-002: a closable tab reserves the close glyph's own band at its trailing edge,
            // so the label is elided before it can run under the ✕.
            if subwindow.closable {
                label.width = label.width.saturating_sub(CLOSE_SIZE_SUM);
            }
            context.draw_text_fitted_cow(
                context.text_line(label, font),
                &subwindow.title,
                font,
                text_color,
                HorizontalAlignment::Center,
            );
            // C-002: the per-tab close affordance. The same derivation serves the paint and
            // `tab_close_button_at`; the hovered tab's ✕ gets a filled backing so the target
            // reads before it is clicked.
            if subwindow.closable {
                let close = tab_close_rect(tab);
                let hovered = self.hovered_tab_close == Some(index);
                if hovered {
                    context.fill_rect(close, text_color.with_alpha_f32(0.22));
                }
                let close_color = if hovered { text_color } else { text_color.blend(&fill, 0.35) };
                let (close_x, close_y) = (close.x + 3, close.y + 3);
                let close_side = CLOSE_SIZE - 6;
                context.draw_line(
                    Point::new(close_x, close_y),
                    Point::new(close_x + close_side, close_y + close_side),
                    close_color,
                );
                context.draw_line(
                    Point::new(close_x + close_side, close_y),
                    Point::new(close_x, close_y + close_side),
                    close_color,
                );
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
    use crate::widget::IconName;
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
    fn a_title_written_through_the_record_is_announced_and_marks_the_area_dirty() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        let index = area.add_sub_window(widget_id_1(), Rect::new(0, 0, 200, 150));
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        area.subwindow_title_changed.connect({
            let seen = Arc::clone(&seen);
            move |value: Arc<(ObjectId, String)>| seen.lock().unwrap().push(value.as_ref().clone())
        });

        area.sub_window_mut(index).unwrap().set_title("Report".to_string());
        assert_eq!(area.sub_window(index).unwrap().title(), "Report");
        assert_eq!(*seen.lock().unwrap(), vec![(widget_id_1(), "Report".to_string())]);
        assert!(area.title_dirty.get(), "the area must schedule a repaint");

        // Writing the same text is a no-op: no second event, no second dirty mark.
        area.sub_window_mut(index).unwrap().set_title("Report".to_string());
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

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

    /// The view mode is a real setter on the control, so a document can switch
    /// the area between sub-window and tabbed presentation through the property
    /// route.
    #[test]
    fn the_view_mode_is_reachable_through_the_property_route() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        assert_eq!(area.view_mode(), ViewMode::SubWindowView, "default view mode");

        area.set("view_mode", CapabilityValue::String("tabbed".to_string())).expect("writable");
        assert_eq!(area.view_mode(), ViewMode::TabbedView);
        assert_eq!(
            area.get("view_mode").expect("readable"),
            CapabilityValue::String("tabbed".to_string())
        );

        // A token the control cannot parse is refused rather than guessed.
        assert_eq!(
            area.set("view_mode", CapabilityValue::String("split".to_string())),
            Err(CapabilityAccessError::TypeMismatch)
        );
        // The published command name needs its value through the property route.
        assert_eq!(area.command("set_view_mode"), Err(CapabilityAccessError::OutOfRange));
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

    // ── 9b. Sub-window icons and their draw ──────────────────────────────

    /// Builds one titled sub-window in sub-window view, ready to be drawn.
    fn titled_area() -> MdiArea {
        let mut area = MdiArea::new(Rect::new(0, 0, 240, 160));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 120, 90));
        area.sub_window_mut(0).expect("exists").set_title("Doc".to_string());
        area
    }

    #[test]
    fn mdiarea_set_item_icon_source_reports_a_missing_child() {
        let mut area = titled_area();
        assert_eq!(
            area.set_item_icon_source(widget_id_2(), Some(IconSource::Builtin(IconName::Info))),
            Err(crate::widget::IconItemError::ItemNotFound { id: widget_id_2() })
        );
        assert!(area
            .set_item_icon_source(widget_id_1(), Some(IconSource::Builtin(IconName::Info)))
            .is_ok());
        assert_eq!(
            area.sub_window(0).unwrap().icon_source(),
            Some(IconSource::Builtin(IconName::Info))
        );
    }

    #[test]
    fn mdiarea_sub_window_icon_adds_geometry_to_the_title_bar() {
        let without = render_to_svg(&mut titled_area());

        let mut with_icon = titled_area();
        with_icon
            .set_item_icon_source(widget_id_1(), Some(IconSource::Builtin(IconName::Info)))
            .expect("the child exists");
        let with = render_to_svg(&mut with_icon);

        // One compound geometry becomes exactly one extra `<path>`; the left-aligned title is
        // unchanged, so nothing else moves.
        assert_eq!(
            with.matches("<path").count(),
            without.matches("<path").count() + 1,
            "a typed sub-window icon must reach the drawing backend exactly once"
        );
    }

    #[cfg(feature = "image")]
    #[test]
    fn mdiarea_legacy_image_icon_reaches_the_drawing_backend() {
        let mut area = titled_area();
        let without = render_to_svg(&mut area);
        assert!(!without.contains("<image"), "an iconless sub-window draws no image");

        area.sub_window_mut(0).expect("exists").set_icon(Some(
            Image::try_from_rgba(vec![128; 4 * 4 * 4], 4, 4)
                .expect("the buffer matches its declared size"),
        ));
        let with = render_to_svg(&mut area);
        assert!(with.contains("<image"), "a legacy sub-window image must reach the backend");
    }

    #[cfg(feature = "image")]
    #[test]
    fn mdiarea_typed_and_legacy_icon_share_one_active_state() {
        let mut area = titled_area();
        area.sub_window_mut(0).expect("exists").set_icon(Some(
            Image::try_from_rgba(vec![3; 4], 1, 1).expect("the buffer matches its declared size"),
        ));
        assert!(area.sub_window(0).unwrap().has_icon());

        area.set_item_icon_source(widget_id_1(), Some(IconSource::Builtin(IconName::Save)))
            .expect("the child exists");
        assert!(area.sub_window(0).unwrap().icon().is_none());
        assert_eq!(
            area.sub_window(0).unwrap().icon_source(),
            Some(IconSource::Builtin(IconName::Save))
        );

        area.sub_window_mut(0).unwrap().clear_icon_source();
        assert!(!area.sub_window(0).unwrap().has_icon());
        assert!(area.sub_window(0).unwrap().icon().is_none());
    }

    #[test]
    fn mdiarea_tabbed_view_draws_the_sub_window_icon_too() {
        // Both presentations of one sub-window must show its icon: the tabbed view has no title
        // bar, so an icon wired only into the title bar would simply vanish there.
        let build = |icon: bool| {
            let mut area = MdiArea::new(Rect::new(0, 0, 240, 160));
            area.add_sub_window(widget_id_1(), Rect::new(10, 10, 120, 90));
            area.sub_window_mut(0).expect("exists").set_title("Doc".to_string());
            area.set_view_mode(ViewMode::TabbedView);
            if icon {
                area.set_item_icon_source(widget_id_1(), Some(IconSource::Builtin(IconName::Info)))
                    .expect("the child exists");
            }
            area
        };
        let without = render_to_svg(&mut build(false));
        let with = render_to_svg(&mut build(true));
        assert_eq!(
            with.matches("<path").count(),
            without.matches("<path").count() + 1,
            "the tabbed view must paint the sub-window icon exactly once"
        );
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
        area.handle_event(&Event::MousePress {
            pos: Point::new(150, 150),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(area.active_sub_window(), Some(widget_id_2()));

        // Click back on first sub-window
        area.handle_event(&Event::MousePress { pos: Point::new(20, 20), button: 1, modifiers: 0 });
        assert_eq!(area.active_sub_window(), Some(widget_id_1()));
    }

    #[test]
    fn mdiarea_mouse_click_outside_sub_window_is_noop() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 200, 150));

        assert_eq!(area.active_sub_window(), Some(widget_id_1()));

        // Click outside all sub-windows
        area.handle_event(&Event::MousePress {
            pos: Point::new(500, 300),
            button: 1,
            modifiers: 0,
        });
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

    #[test]
    fn mdiarea_arrange_icons_handles_narrow_area() {
        let mut area = MdiArea::new(Rect::new(0, 0, 50, 40));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 20, 20));
        area.sub_window_mut(0).expect("subwindow exists").set_minimized(true);
        area.arrange_icons();
        assert_eq!(area.sub_window(0).map(|window| window.geometry().width), Some(100));
    }

    // ── 11. `view_mode` decides where a sub-window is placed ────────────────

    /// A `TabbedView` area places its active window in the body below the tab strip, and only
    /// that one.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `view_mode` was stored, published (getter, `view_mode` property token, round-trip test) and
    /// read by nothing: `draw` painted the stored sub-window geometries and a title bar for each, so
    /// `TabbedView` rendered byte-identically to `SubWindowView`. Merely *branching* on the mode in
    /// `subwindow_placement` would leave the paint ignoring the result, so this asserts the numbers a
    /// paint has to obey: the frame starts below the strip, spans the work area, and the inactive
    /// windows have no placement at all rather than one under the active window.
    #[test]
    fn tabbed_view_stacks_every_window_in_one_body_below_the_strip() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 100, 80));
        area.add_sub_window(widget_id_2(), Rect::new(200, 200, 100, 80));
        area.set_active_sub_window(widget_id_2());
        area.set_view_mode(ViewMode::TabbedView);

        let strip = area.tab_strip_rect();
        assert_eq!(strip.y, 0);
        assert_eq!(strip.height, TAB_STRIP_HEIGHT as u32);
        assert_eq!(strip.width, 600);

        let active = area.subwindow_placement(1).expect("the active window is placed");
        assert_eq!(active.frame.x, 0, "the body spans the work area");
        assert_eq!(active.frame.width, 600);
        assert_eq!(
            active.frame.y,
            strip.y + strip.height as i32,
            "the body starts below the strip, not over it"
        );
        assert_eq!(
            active.frame.y + active.frame.height as i32,
            400,
            "and runs to the work area's bottom edge"
        );
        assert!(!active.title_bar, "the tab already names this window");

        // The window that is *not* active has no placement: two placements would paint both frames
        // over the same body and the later one would win, which is the wrong window on screen.
        assert!(area.subwindow_placement(0).is_none(), "only the active window occupies the body");

        // And the stored geometry is left alone, so switching back to `SubWindowView` restores it.
        area.set_view_mode(ViewMode::SubWindowView);
        let restored = area.subwindow_placement(0).expect("sub-window view places every window");
        assert_eq!(restored.frame, Rect::new(10, 10, 100, 80));
        assert!(restored.title_bar, "sub-window view draws its own title bar");
    }

    /// A tab is where the click says it is: the tab box the paint reads is the box the hit test
    /// resolves.
    ///
    /// `tab_rect` and `tab_at_point` are one derivation for the same reason `list_view`'s row
    /// geometry is: two independent calculations of "where is tab 3" drift, and the symptom is a
    /// click activating the tab next to the one that was clicked.
    #[test]
    fn a_tab_click_activates_the_tab_it_is_painted_on() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_3(), Rect::new(0, 0, 100, 100));
        area.set_view_mode(ViewMode::TabbedView);

        for index in 0..3 {
            let tab = area.tab_rect(index).expect("three tabs fit");
            let centre = Point::new(tab.x + tab.width as i32 / 2, tab.y + tab.height as i32 / 2);
            assert_eq!(
                area.tab_at_point(centre),
                Some(index),
                "the centre of tab {index} at {tab:?} must hit-test back to it"
            );
        }
        // Tabs are capped rather than stretched: three tabs of 600 px each would each be 200 wide,
        // which is past the cap and reads as a toolbar.
        assert_eq!(area.tab_rect(0).map(|tab| tab.width), Some(TAB_MAX_WIDTH));
        // A click on the strip past the last tab is no tab, not a clamp onto the last one.
        assert_eq!(area.tab_at_point(Point::new(599, 2)), None);
        // And outside the strip entirely is no tab either.
        assert_eq!(area.tab_at_point(Point::new(5, TAB_STRIP_HEIGHT + 40)), None);
    }

    /// The mode reaches the pixels: a `TabbedView` snapshot is not a `SubWindowView` one.
    ///
    /// The geometry assertions above say where the frames go; this says the paint actually obeys
    /// them. Without it a `subwindow_placement` that nothing called would still pass every number.
    #[test]
    fn the_two_view_modes_render_differently() {
        let build = |mode: ViewMode| {
            let mut area = MdiArea::new(Rect::new(0, 0, 240, 160));
            area.add_sub_window(widget_id_1(), Rect::new(10, 10, 120, 90));
            area.add_sub_window(widget_id_2(), Rect::new(40, 40, 120, 90));
            area.set_active_sub_window(widget_id_2());
            area.sub_window_mut(0).expect("exists").set_title("First".to_string());
            area.sub_window_mut(1).expect("exists").set_title("Second".to_string());
            area.set_view_mode(mode);
            area
        };
        let windowed = render_to_svg(&mut build(ViewMode::SubWindowView));
        let tabbed = render_to_svg(&mut build(ViewMode::TabbedView));
        assert_ne!(
            windowed, tabbed,
            "the two view modes must not render identically -- that was the dead state"
        );
        // The tab strip sits on the top row and the sub-window view puts a frame there instead.
        let tabbed_strip = build(ViewMode::TabbedView);
        assert_eq!(tabbed_strip.tab_strip_rect().height, TAB_STRIP_HEIGHT as u32);

        // The *body* is what the mode is actually about, and "the strings differ" alone does not
        // pin it: a `TabbedView` that still painted every window at its stored geometry would differ
        // from the windowed one by the tab strip alone. So the placement is asserted through the
        // rendering's own numbers -- the active window's frame must start at the strip's bottom
        // edge, and the inactive one must not be drawn at its stored geometry.
        let tabbed_area = build(ViewMode::TabbedView);
        let active_frame =
            tabbed_area.subwindow_placement(1).expect("the active window is placed").frame;
        assert_eq!(active_frame.y, TAB_STRIP_HEIGHT, "the tabbed body begins where the strip ends");
        assert_eq!(active_frame.y + active_frame.height as i32, 160);
        assert!(
            tabbed_area.subwindow_placement(0).is_none(),
            "and the inactive window is not painted at its stored geometry"
        );
        // The windowed mode, by contrast, uses the stored geometry for both.
        let windowed_area = build(ViewMode::SubWindowView);
        assert_eq!(
            windowed_area.subwindow_placement(0).expect("placed").frame,
            Rect::new(10, 10, 120, 90)
        );
    }

    /// The ✕ closes the window it belongs to instead of activating it.
    ///
    /// The button was painted for every closable sub-window but no code hit-tested it: a press on
    /// it resolved to the frame and called `set_active_sub_window`, so a drawn "close" affordance
    /// did nothing — and the press was then forwarded into the window it had just "activated".
    #[test]
    fn mdi_close_button_closes_the_sub_window() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(20, 20, 200, 150));
        area.add_sub_window(widget_id_2(), Rect::new(240, 20, 200, 150));
        assert_eq!(
            area.active_sub_window(),
            Some(widget_id_1()),
            "the first window is active before any press"
        );

        // Close the **inactive** window, so the assertion below distinguishes "closed it" from
        // "activated it".
        let frame = area.subwindow_placement(1).expect("placed").frame;
        let close = close_button_rect(frame);
        let pos = Point::new(close.x + close.width as i32 / 2, close.y + close.height as i32 / 2);
        area.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });

        assert_eq!(area.sub_window_count(), 1, "the ✕ must close its own window");
        assert_eq!(
            area.sub_window(0).map(|window| window.widget()),
            Some(widget_id_1()),
            "the surviving window is the untouched one"
        );
        assert_eq!(
            area.active_sub_window(),
            Some(widget_id_1()),
            "closing an inactive window must not activate it"
        );
    }

    /// The checker background reaches the work area's own edges.
    ///
    /// The tile loops truncated to whole tiles and never filled the base, so the right and bottom
    /// remainders (up to 19 px each) kept whatever the parent had painted — and an area smaller
    /// than one tile painted nothing at all.
    #[test]
    fn mdi_pattern_background_covers_its_whole_rect() {
        let mut area = MdiArea::new(Rect::new(0, 0, 250, 250));
        area.set_background(Background::Pattern);
        let svg = render_to_svg(&mut area);
        assert!(
            svg.contains(r#"<rect x="0" y="0" width="250" height="250" fill="rgba"#),
            "the checker must start from a base fill covering the whole work area"
        );
    }

    /// Every activation order actually drives `next`/`previous`.
    #[test]
    fn activation_order_decides_what_next_visits() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_3(), Rect::new(0, 0, 100, 100));
        // Reverse the stacking order so it differs from creation order.
        area.sub_window_mut(0).expect("first").set_z_order(2);
        area.sub_window_mut(1).expect("second").set_z_order(1);
        area.sub_window_mut(2).expect("third").set_z_order(0);

        area.set_active_sub_window(widget_id_1());
        area.set_activation_order(ActivationOrder::CreationOrder);
        area.activate_next_sub_window();
        assert_eq!(area.active_sub_window(), Some(widget_id_2()), "creation order is add order");

        area.set_active_sub_window(widget_id_1());
        area.set_activation_order(ActivationOrder::StackingOrder);
        area.activate_next_sub_window();
        assert_eq!(
            area.active_sub_window(),
            Some(widget_id_3()),
            "stacking order follows z_order, not the storage index"
        );

        area.set_activation_order(ActivationOrder::HistoryOrder);
        area.set_active_sub_window(widget_id_1());
        area.set_active_sub_window(widget_id_3());
        area.set_active_sub_window(widget_id_2());
        area.activate_next_sub_window();
        assert_eq!(
            area.active_sub_window(),
            Some(widget_id_3()),
            "history order returns the previously active window"
        );
        area.activate_previous_sub_window();
        assert_eq!(
            area.active_sub_window(),
            Some(widget_id_1()),
            "from the newest window, previous wraps through the history cycle"
        );
    }

    /// Dragging a frame edge resizes it, and a title-bar double-click maximizes and restores.
    #[test]
    fn edge_drag_resizes_and_title_bar_double_click_maximizes() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(20, 20, 200, 150));

        // Grab the right edge and pull it 30 px outward.
        area.handle_event(&Event::MousePress {
            pos: Point::new(219, 100),
            button: 1,
            modifiers: 0,
        });
        area.handle_event(&Event::MouseMove { pos: Point::new(249, 100) });
        area.handle_event(&Event::MouseRelease { pos: Point::new(249, 100), button: 1 });
        let resized = area.sub_window(0).expect("frame").geometry();
        assert_eq!(resized, Rect::new(20, 20, 230, 150), "the right edge followed the drag");

        // The drag cannot shrink the frame past its minimum.
        area.handle_event(&Event::MousePress {
            pos: Point::new(249, 100),
            button: 1,
            modifiers: 0,
        });
        area.handle_event(&Event::MouseMove { pos: Point::new(-400, 100) });
        area.handle_event(&Event::MouseRelease { pos: Point::new(-400, 100), button: 1 });
        assert_eq!(
            area.sub_window(0).expect("frame").geometry().width,
            MIN_SUBWINDOW_WIDTH,
            "the drag stops at the minimum width"
        );

        // Double-click the title bar: maximize, then restore.
        let before = area.sub_window(0).expect("frame").geometry();
        area.handle_event(&Event::MouseDoubleClick { pos: Point::new(40, 22), button: 1 });
        assert!(area.sub_window(0).expect("frame").is_maximized());
        assert_eq!(area.sub_window(0).expect("frame").geometry(), area.geometry());
        area.handle_event(&Event::MouseDoubleClick { pos: Point::new(40, 22), button: 1 });
        assert!(!area.sub_window(0).expect("frame").is_maximized());
        assert_eq!(area.sub_window(0).expect("frame").geometry(), before);
    }

    /// The viewport pan moves the painted frame and the hit test together, while the stored
    /// geometry stays put; a maximized window ignores the pan.
    #[test]
    fn the_viewport_pan_translates_frames_and_hit_tests() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(100, 40, 120, 100));
        let stored = area.sub_window(0).expect("frame").geometry();
        assert!(area.sub_window_at_position(Point::new(110, 50)).is_some());
        assert!(area.sub_window_at_position(Point::new(50, 35)).is_none());

        area.set_scroll_offset(Point::new(60, 10));
        assert_eq!(area.scroll_offset(), Point::new(60, 10));
        let placed = area.subwindow_placement(0).expect("placed").frame;
        assert_eq!(placed, Rect::new(40, 30, 120, 100), "the frame follows the pan");
        assert!(area.sub_window_at_position(Point::new(50, 35)).is_some());
        assert!(area.sub_window_at_position(Point::new(200, 50)).is_none());
        assert_eq!(
            area.sub_window(0).expect("frame").geometry(),
            stored,
            "stored frame is unchanged"
        );

        area.scroll_by(-10, -10);
        assert_eq!(area.scroll_offset(), Point::new(50, 0));

        area.toggle_maximized_sub_window(widget_id_1());
        assert_eq!(
            area.subwindow_placement(0).expect("placed").frame,
            area.geometry(),
            "a maximized window owns the area regardless of the pan"
        );
    }

    /// C-15: the title-bar button minimizes and restores, and the state is observable.
    #[test]
    fn the_minimize_button_toggles_the_icon_state() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 120, 100));
        let button = minimize_button_rect(area.subwindow_placement(0).expect("placed").frame);
        area.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point::new(button.x + 1, button.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert!(area.is_sub_window_minimized(widget_id_1()));
        assert!(area.set_sub_window_minimized(widget_id_1(), false));
        assert!(!area.is_sub_window_minimized(widget_id_1()));
    }

    /// C-14: dragging the title bar moves the window by the pointer delta.
    #[test]
    fn dragging_a_title_bar_moves_the_window() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 120, 100));
        area.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point::new(20, 15),
            button: 1,
            modifiers: 0,
        });
        area.handle_event(&crate::event::Event::MouseMove { pos: crate::core::Point::new(60, 45) });
        area.handle_event(&crate::event::Event::MouseRelease {
            pos: crate::core::Point::new(60, 45),
            button: 1,
        });
        let geometry = area.sub_window_mut(0).expect("window").geometry;
        assert_eq!(geometry.x, 50, "moved by the pointer delta");
        assert_eq!(geometry.y, 40);
    }

    /// C-16/C-008: the wheel pans the work area in sub-window mode, and the pan stays inside
    /// the content's bounding box.
    #[test]
    fn the_wheel_pans_the_work_area_within_the_content() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 600, 500));
        // A pan that would reveal empty space above the content is refused.
        area.handle_event(&crate::event::Event::Wheel {
            delta: crate::core::Point::new(0, 2),
            modifiers: 0,
        });
        assert_eq!(area.scroll_offset().y, 0);
        // The other direction reveals the lower part of the content.
        area.handle_event(&crate::event::Event::Wheel {
            delta: crate::core::Point::new(0, -5),
            modifiers: 0,
        });
        assert_eq!(area.scroll_offset().y, 120);
    }

    /// C-008: a direct pan is clamped to the content bounding box.
    #[test]
    fn a_pan_is_clamped_to_the_content_bounds() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 600, 500));
        area.set_scroll_offset(crate::core::Point::new(9_999, 9_999));
        assert_eq!(area.scroll_offset(), crate::core::Point::new(200, 200));
        area.set_scroll_offset(crate::core::Point::new(-50, -50));
        assert_eq!(area.scroll_offset(), crate::core::Point::new(0, 0));
    }

    /// C-007: a maximized sub-window follows the area's resize.
    #[test]
    fn a_maximized_sub_window_follows_a_resize() {
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(10, 10, 100, 80));
        assert!(area.toggle_maximized_sub_window(widget_id_1()));
        assert_eq!(area.sub_window(0).unwrap().geometry, Rect::new(0, 0, 400, 300));
        area.set_geometry(Rect::new(0, 0, 500, 350));
        assert_eq!(
            area.sub_window(0).unwrap().geometry,
            Rect::new(0, 0, 500, 350),
            "the maximized child fills the new area"
        );
    }

    /// C-006: cascade and tile request a frame.
    #[test]
    fn cascade_and_tile_request_a_repaint() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 80));
        let redraws = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&redraws);
        area.base.redraw_requested.connect(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        area.cascade_sub_windows();
        area.tile_sub_windows();
        assert!(redraws.load(Ordering::SeqCst) >= 2, "each arrangement owes a frame");
    }

    /// C-019: the size hint aggregates the active sub-window's control.
    #[test]
    fn the_size_hint_follows_the_active_sub_window() {
        use crate::widget::base_widgets::label::Label;

        let label = Label::new("child".to_string(), Rect::new(0, 0, 180, 90));
        let expected = label.size_hint();
        let id =
            crate::widget::runtime::register(Box::new(label)).expect("the test control mounts");
        let mut area = MdiArea::new(Rect::new(0, 0, 400, 300));
        area.add_sub_window(id, Rect::new(10, 10, 100, 80));
        assert_eq!(area.size_hint(), expected);
        let _ = crate::widget::runtime::unregister(id);
    }

    /// C-001: the view mode, geometries and z-orders round-trip through a saved blob.
    #[test]
    fn the_window_layout_round_trips_through_saved_state() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(10, 20, 120, 90));
        area.add_sub_window(widget_id_2(), Rect::new(200, 30, 140, 100));
        area.sub_window_mut(0).expect("window").set_z_order(7);
        area.sub_window_mut(1).expect("window").set_z_order(3);
        area.set_view_mode(ViewMode::TabbedView);
        let saved = area.save_state();

        let mut restored = MdiArea::new(Rect::new(0, 0, 600, 400));
        restored.add_sub_window(widget_id_1(), Rect::new(0, 0, 10, 10));
        restored.add_sub_window(widget_id_2(), Rect::new(0, 0, 10, 10));
        assert!(restored.restore_state(&saved));
        assert_eq!(restored.view_mode(), ViewMode::TabbedView);
        assert_eq!(restored.sub_window(0).unwrap().geometry(), Rect::new(10, 20, 120, 90));
        assert_eq!(restored.sub_window(1).unwrap().geometry(), Rect::new(200, 30, 140, 100));
        assert_eq!(restored.sub_window(0).unwrap().z_order(), 7);
        assert_eq!(restored.sub_window(1).unwrap().z_order(), 3);

        // A window-count mismatch and a malformed blob are refused without touching the area.
        let mut different = MdiArea::new(Rect::new(0, 0, 600, 400));
        different.add_sub_window(widget_id_1(), Rect::new(0, 0, 10, 10));
        assert!(!different.restore_state(&saved));
        assert_eq!(different.sub_window(0).unwrap().geometry(), Rect::new(0, 0, 10, 10));
        assert!(!restored.restore_state(b"MDI1"));
        assert_eq!(restored.view_mode(), ViewMode::TabbedView, "a rejected blob applies nothing");
    }

    /// C-002: a tab's drawn close button closes the document it belongs to and tracks hover.
    #[test]
    fn a_tab_close_button_closes_the_document_under_it() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.add_sub_window(widget_id_2(), Rect::new(0, 0, 100, 100));
        area.set_view_mode(ViewMode::TabbedView);
        let tab = area.tab_rect(0).expect("the first tab is drawn");
        let close = tab_close_rect(tab);
        let pos = Point::new(close.x + close.width as i32 / 2, close.y + close.height as i32 / 2);
        assert_eq!(area.tab_close_button_at(pos), Some(0), "the ✕ hit-tests where it is drawn");

        area.handle_event(&Event::MouseMove { pos });
        assert_eq!(area.hovered_tab_close, Some(0), "hovering the ✕ is tracked for the paint");
        area.handle_event(&Event::MouseLeave { pos });
        assert_eq!(area.hovered_tab_close, None, "leaving clears the hover state");

        area.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(area.sub_window_count(), 1, "the ✕ closes its own document");
        assert_eq!(area.sub_window(0).map(|window| window.widget()), Some(widget_id_2()));
    }

    /// C-003: a south strip sits on the bottom edge and the body takes the space above it.
    #[test]
    fn a_south_tab_strip_places_the_body_above_it() {
        let mut area = MdiArea::new(Rect::new(0, 0, 600, 400));
        area.add_sub_window(widget_id_1(), Rect::new(0, 0, 100, 100));
        area.set_view_mode(ViewMode::TabbedView);
        assert_eq!(area.tab_strip_position(), TabStripPosition::North);
        let north_strip = area.tab_strip_rect();
        let north_body = area.subwindow_placement(0).expect("placed").frame;
        assert_eq!(north_strip.y, 0);
        assert_eq!(north_body.y, north_strip.height as i32);

        area.set_tab_strip_position(TabStripPosition::South);
        assert_eq!(area.tab_strip_position(), TabStripPosition::South);
        let south_strip = area.tab_strip_rect();
        let south_body = area.subwindow_placement(0).expect("placed").frame;
        assert_eq!(south_strip.y + south_strip.height as i32, 400);
        assert_eq!(south_body.y, 0, "the body starts at the top edge");
        assert_eq!(
            south_body.y + south_body.height as i32,
            south_strip.y,
            "and ends where the strip begins"
        );
        // A click still resolves the tab on the edge it is painted on.
        let tab = area.tab_rect(0).expect("tab");
        assert_eq!(area.tab_at_point(Point::new(tab.x + tab.width as i32 / 2, tab.y + 2)), Some(0));
    }
}
