// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tab widget.
//!
//! # The tab strip is assembled from measured widths
//!
//! BLUE22 §B.8 lists this control's defect as "tab width by hand". The width is measured and
//! clamped by [`TabWidget::tab_width_hints`], and the run — where each tab starts — is a single
//! pass over those widths plus the gap. It used to be handed to a `FlexLayout` through a
//! `CompositeBuilder`, which created a temporary label widget per tab on **every** call; the draw
//! pass and the hit test both ask for the run, so that was one temporary widget per tab per frame.
//! All tabs declare a fixed width and the strip is sized to fit, so the layout's answer is exactly
//! this arithmetic (the stripped-profile arm already computed it that way).

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::dnd::{DragSource, DropEffect, DropTarget};
use crate::event::{DragPayload, DragSession, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::save_state::SaveState;
use crate::widget::IconSource;
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
/// Tab widget.
pub struct TabWidget {
    base: BaseWidget,
    tabs: Vec<Tab>,
    current_index: usize,
    tab_position: TabPosition,
    tab_shape: TabShape,
    closable: bool,
    movable: bool,
    /// Index of the tab a move gesture started on, and the drag state machine that decides
    /// whether the gesture has travelled far enough to count as a drag.
    ///
    /// Two pieces because they answer two different questions: the session says *whether*
    /// the pointer has moved past the click threshold, and the index says *which tab* is
    /// being carried. Tracking only the index (as `TabBar` did until it gained a session)
    /// makes "click" and "drag" the same gesture, so a stray pixel of movement while
    /// selecting a tab would silently reorder it.
    drag_session: Option<DragSession>,
    /// The slot the carried tab occupies now, updated as the live preview reorders the strip.
    dragging_from: Option<usize>,
    /// The slot the carried tab occupied when the live gesture began.
    ///
    /// Kept beside [`Self::dragging_from`] so a coordinator drop can tell whether the two
    /// paths observed the same gesture: if the payload's own index matches this origin, the
    /// live path has already reordered and the drop must use the current slot instead.
    live_drag_origin: Option<usize>,
    /// Emitted with the new index when the selected tab changes; not emitted
    /// when the same index is re-applied.
    pub current_changed: Signal1<usize>,
    /// Emitted when the user requests that the tab at this absolute index be
    /// closed. The `closable` flag only gates hit-testing of the close button;
    /// this widget does not remove the tab itself — the host must handle the
    /// request and call `remove_tab`, so the index is still valid when emitted.
    pub tab_close_requested: Signal1<usize>,
    /// Emitted when a movable tab is dragged to a new index; the payload is
    /// `(old_index, new_index)`.
    ///
    /// `movable` was declared but read by nothing, and this signal did not exist, so a host
    /// could neither turn reordering on nor learn that it had happened. The drag gesture in
    /// [`TabWidget::handle_event`] is what emits it.
    pub tab_moved: Signal1<(usize, usize)>,
    /// Emitted with the effect of a completed tab drag.
    ///
    /// The strip reorders as the pointer moves, so a `Move` reports that the gesture
    /// committed; `None` reports that the session ended without a committed drop.
    pub drag_finished: Signal1<DropEffect>,
    /// The tab a coordinator drag taken from this strip started on.
    ///
    /// Consumed by `on_drag_finished`: a transfer to another strip inserted the tab (and its
    /// page) there, so this strip's copy has to go. The live pointer path keeps its own
    /// [`Self::dragging_from`], which tracks a slot that moves while the gesture is in flight;
    /// this one is the press-time slot the payload named.
    pending_drag_tab: core::cell::Cell<Option<usize>>,
    /// Whether the drop that ended the current gesture was committed **by this strip**.
    ///
    /// A reorder finishes in `on_drop`, so the source half must not remove the tab a second
    /// time. A transfer commits in the target's strip, leaving this false — which is exactly
    /// when the source's copy has to go.
    drop_committed_here: core::cell::Cell<bool>,
    /// Emitted when a context menu is requested on a tab.
    ///
    /// The payload is the request position in window coordinates; the widget only emits it
    /// when the point is inside a tab, because a menu about a tab makes no sense on the
    /// content page. The host supplies the menu (close, pin, and whatever else its tab
    /// policy defines) and resolves the tab from the position with the same geometry the
    /// strip paints.
    pub context_menu_requested: Signal1<Point>,
    /// Emitted with the tab index when a press lands on a visible tab in the strip.
    ///
    /// This is the completion `current_changed` cannot provide: a press on the tab that is
    /// *already* selected changes no index, so a host that wants to react to the click itself (open a
    /// document, focus an editor, reveal a panel) had no event to subscribe to.
    pub tab_bar_clicked: Signal1<usize>,
    /// Emitted with the tab index when a double click lands on a visible tab in the strip.
    ///
    /// The reference tab widget uses this gesture to detach a tab into a window or to rename it;
    /// the control only reports it, because both policies belong to the host's tab model.
    pub tab_bar_double_clicked: Signal1<usize>,
    /// Whether the strip is drawn in document mode.
    ///
    /// Document mode is the "one flat row of tabs over one page" presentation: the strip uses the
    /// control's own surface instead of a per-tab tint, and the current tab is marked by an accent
    /// rule under its caption rather than by being filled with the page colour. It is a presentation
    /// flag only — no index, page or hit test changes.
    document_mode: bool,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Memoised preferred width per tab, keyed by the inputs the measurement reads.
    ///
    /// `tab_width_hints` is asked by the paint path **and** by every pointer move's hit test, and
    /// each answer used to run `estimate_text_width` per tab — with `text-shaping` that is a cluster
    /// segmentation and a glyph-advance lookup per label. The width depends on exactly three
    /// things: the title, whether the tab reserves an icon slot, and the effective font. The cache
    /// stores all three as its key and re-measures when any of them differs, so there is no
    /// explicit invalidation list to keep in sync (a missed `add`/`remove`/`set_text`/theme hook
    /// cannot leave a stale width behind). Strip width and tab position are not part of the key:
    /// they only decide the overflow division, which is integer arithmetic over these widths.
    tab_width_cache: RefCell<TabWidthCache>,
    /// Pixels the tab strip is scrolled by when it overflows.
    overflow_scroll: i32,
    /// Whether the strip shows scroll arrows when the tabs do not fit.
    show_overflow_buttons: bool,
    /// How a title too long for its tab is shortened.
    elide_mode: TabElideMode,
    /// The tab under the pointer, for the tooltip path.
    hovered_tab: Option<usize>,
    /// An optional widget placed in the unused strip corner.
    corner_widget: Option<ObjectId>,
}

/// How a tab title that does not fit its box is shortened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabElideMode {
    /// Clip the title (the historical behaviour).
    #[default]
    None,
    /// Shorten the title and append an ellipsis.
    ElideRight,
}

/// The measurement cache behind [`TabWidget::tab_width_hints`].
///
/// `font: None` marks an empty cache, so the first call always measures. `fingerprint` is a hash of
/// `(title, has_icon)` for every tab in strip order; a different tab count or a different caption
/// changes it.
#[derive(Default)]
struct TabWidthCache {
    font: Option<Font>,
    fingerprint: u64,
    widths: Vec<i32>,
}
/// Tab information.
pub struct Tab {
    title: String,
    #[cfg(feature = "image")]
    icon: Option<Image>,
    /// Typed icon content for this tab.
    ///
    /// The legacy [`Tab::icon`] image and this source are one active state, not two: whichever was
    /// assigned most recently wins, and the other is cleared. That keeps `Tab::icon_source` from
    /// reporting content the tab is not actually drawing.
    icon_source: Option<IconSource>,
    tooltip: String,
    enabled: bool,
    widget: Option<ObjectId>,
    /// Whether this tab is offered in the strip.
    ///
    /// A hidden tab keeps its position, its page and its index: it is not drawn, it occupies no
    /// strip width, it cannot be pressed, and the selection skips over it. Hiding is therefore a
    /// *presentation* fact, not a second model — the index a caller already holds still addresses
    /// the same tab. See [`TabWidget::set_tab_visible`].
    visible: bool,
    /// Caller-owned payload, carried through moves and lookups untouched.
    ///
    /// A `TabWidget` item has no identity beyond its slot, and a host that keeps its own document
    /// per tab needs somewhere to put the handle that survives a reorder. This is that slot; the
    /// control never reads it.
    data: Option<u64>,
}
/// Tab position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabPosition {
    /// Tabs at the top
    #[default]
    North,
    /// Tabs at the bottom
    South,
    /// Tabs at the left
    West,
    /// Tabs at the right
    East,
}

impl TabPosition {
    /// Parses a property token, accepting exactly the spellings
    /// [`tab_position_token`] publishes.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "north" => Some(TabPosition::North),
            "south" => Some(TabPosition::South),
            "west" => Some(TabPosition::West),
            "east" => Some(TabPosition::East),
            _ => None,
        }
    }
}

/// The token `tab_position` is carried as, matching the schema row's accepted spellings.
fn tab_position_token(position: TabPosition) -> &'static str {
    match position {
        TabPosition::North => "north",
        TabPosition::South => "south",
        TabPosition::West => "west",
        TabPosition::East => "east",
    }
}
/// Tab shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabShape {
    /// Rounded tabs
    #[default]
    Rounded,
    /// Triangular tabs
    Triangular,
    /// Rectangular tabs
    Rectangular,
}

impl TabShape {
    /// Parses a property token, accepting exactly the spellings
    /// [`tab_shape_token`] publishes.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "rounded" => Some(TabShape::Rounded),
            "triangular" => Some(TabShape::Triangular),
            "rectangular" => Some(TabShape::Rectangular),
            _ => None,
        }
    }
}

/// The token `tab_shape` is carried as, matching the schema row's accepted spellings.
fn tab_shape_token(shape: TabShape) -> &'static str {
    match shape {
        TabShape::Rounded => "rounded",
        TabShape::Triangular => "triangular",
        TabShape::Rectangular => "rectangular",
    }
}
impl Tab {
    /// Creates a new tab.
    pub fn new(title: String) -> Self {
        Self {
            title,
            #[cfg(feature = "image")]
            icon: None,
            icon_source: None,
            tooltip: String::new(),
            enabled: true,
            widget: None,
            visible: true,
            data: None,
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
    #[cfg(feature = "image")]
    /// Returns icon.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    #[cfg(feature = "image")]
    /// Sets icon.
    ///
    /// Replaces any typed [`Tab::icon_source`] previously assigned, mirroring the active-state
    /// rule the typed setter uses. A non-RGBA8 image is normalised once here so the draw path can
    /// borrow its bytes instead of transcoding every frame.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon.map(crate::widget::icon_source::normalize_rgba8_image);
        self.icon_source = None;
    }

    /// Assigns typed, already-resolved icon content to this tab.
    ///
    /// Passing `None` clears the typed source **and** the legacy [`Tab::icon`] image, so a caller
    /// that explicitly clears a tab icon never sees the previously set one reappear.
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

    /// Clears the tab's icon, whatever form it was stored in.
    pub fn clear_icon_source(&mut self) {
        self.set_icon_source(None);
    }

    /// Whether this tab reserves a leading icon slot.
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
    /// Returns tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }
    /// Sets tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    /// Returns whether tab is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Sets enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
    }
    /// Whether this tab is offered in the strip.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    /// Sets whether this tab is offered in the strip.
    ///
    /// The value-level setter only: it cannot recompute the strip or repair the selection, because
    /// a `Tab` has no back-reference to the control that paints it. Use
    /// [`TabWidget::set_tab_visible`], which does both.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    /// The caller-owned payload assigned to this tab.
    pub fn data(&self) -> Option<u64> {
        self.data
    }
    /// Sets the caller-owned payload. The control never interprets it.
    pub fn set_data(&mut self, data: Option<u64>) {
        self.data = data;
    }
}

impl TabWidget {
    /// Creates a tab widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabWidget, geometry, "TabWidget"),
            tabs: Vec::new(),
            current_index: 0,
            tab_position: TabPosition::North,
            tab_shape: TabShape::Rounded,
            closable: false,
            movable: false,
            drag_session: None,
            dragging_from: None,
            live_drag_origin: None,
            current_changed: Signal1::new(),
            tab_close_requested: Signal1::new(),
            tab_moved: Signal1::new(),
            drag_finished: Signal1::new(),
            pending_drag_tab: core::cell::Cell::new(None),
            drop_committed_here: core::cell::Cell::new(false),
            context_menu_requested: Signal1::new(),
            tab_bar_clicked: Signal1::new(),
            tab_bar_double_clicked: Signal1::new(),
            document_mode: false,
            registry: None,
            tab_width_cache: RefCell::new(TabWidthCache::default()),
            overflow_scroll: 0,
            show_overflow_buttons: false,
            elide_mode: TabElideMode::None,
            hovered_tab: None,
            corner_widget: None,
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
    /// Adds a tab.
    ///
    /// The tab's widget is linked in **both** directions (BLUE22 §B.6 rule 5): `add_child` alone
    /// leaves the child believing it has no parent, so a tree walk from the root never reaches a
    /// tab's page. `remove_tab` unlinks the same way, so removal is observable from both sides.
    pub fn add_tab(&mut self, title: String, widget: Option<ObjectId>) -> usize {
        let mut tab = Tab::new(title);
        tab.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
        }
        self.tabs.push(tab);
        let index = self.tabs.len().saturating_sub(1);
        self.sync_pages();
        index
    }
    /// Inserts a tab at position.
    /// Inserts a tab before the given index.
    ///
    /// An `index` past the end appends rather than panicking, matching
    /// [`Self::add_tab`] and the rest of the container family (`tab_bar`,
    /// `pie_menu` clamp the same way).
    pub fn insert_tab(&mut self, index: usize, title: String, widget: Option<ObjectId>) {
        let was_empty = self.tabs.is_empty();
        let before = self.current_index;
        let index = index.min(self.tabs.len());
        let mut tab = Tab::new(title);
        tab.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
        }
        self.tabs.insert(index, tab);
        if !was_empty && self.current_index >= index {
            self.current_index += 1;
        }
        self.sync_pages();
        self.base.request_redraw();
        // The page is the same one, but its index is not: a subscriber holding an index has to
        // hear about the shift or it will address the neighbouring tab.
        if before != self.current_index {
            self.current_changed.emit(self.current_index);
        }
    }
    /// Removes a tab.
    ///
    /// The selection follows the tab it named: removing an earlier tab shifts the rest left, so
    /// the index is decremented to keep pointing at the same page; removing the selected tab
    /// clamps to the last remaining one. A change of index is announced through
    /// `current_changed`, because a subscriber keyed by index would otherwise be looking at a
    /// different page under the same number.
    pub fn remove_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            let before = self.current_index;
            if let Some(widget_id) = self.tabs[index].widget {
                // Only a page this strip still owns is hidden. A cross-strip transfer has
                // already reparented it into the target's child list (the drop commits before
                // the source is told), so unlinking would clear the new parent and hiding would
                // blank the page in the strip that just received it.
                let was_ours = self.base.remove_child_linked(widget_id);
                if was_ours {
                    // The page is no longer this container's content, so it must stop painting
                    // where the tab used to be. Removing it from the child list is not enough: a
                    // detached control keeps its own visibility flag, and a host that renders by
                    // id would still draw it over whatever the tab shows now.
                    crate::set_widget_visible(widget_id, false);
                }
            }
            self.tabs.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.tabs.is_empty() {
                self.current_index = 0;
            }
            // A removal can leave the selection on a tab that is not offered (the repaired index
            // may name a hidden one), and a hidden tab has no page on screen. The selection moves
            // to the nearest visible tab; when none is left the index is kept and nothing shows,
            // because there is nothing to select.
            if self.current_index < self.tabs.len() && !self.tabs[self.current_index].visible {
                if let Some(next) = self.nearest_visible_index(self.current_index) {
                    self.current_index = next;
                }
            }
            self.sync_pages();
            self.base.request_redraw();
            if before != self.current_index {
                self.current_changed.emit(self.current_index);
            }
        }
    }
    /// Returns number of tabs.
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
    /// Returns the selected tab index, or `None` when the widget has no tabs.
    pub fn selected_index(&self) -> Option<usize> {
        self.tabs.get(self.current_index).map(|_| self.current_index)
    }

    /// Returns current tab index.
    #[deprecated(note = "use `selected_index` (Option) — WDGT-13")]
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current tab index.
    ///
    /// A hidden tab is refused: selecting one would show the strip's state as "no tab is current"
    /// while `selected_index()` kept naming a page that is not offered. An out-of-range index is
    /// refused for the same reason it always was.
    pub fn set_current_index(&mut self, index: usize) {
        if index >= self.tabs.len() || !self.tabs[index].visible {
            return;
        }
        if self.current_index != index {
            self.current_index = index;
            self.sync_pages();
            self.current_changed.emit(index);
            self.base.request_redraw();
        }
    }
    /// Returns current tab widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.tabs.get(self.current_index).and_then(|tab| tab.widget)
    }
    /// Returns tab at index.
    pub fn tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }

    /// Serialises the selected tab, so a host can restore it on the next launch.
    ///
    /// The tab's *title and content* belong to the host's model; only the selection is user
    /// state, so only the index is stored.
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(5);
        state.push(TAB_STATE_VERSION);
        state.extend_from_slice(&(self.current_index as u32).to_le_bytes());
        state
    }

    /// Restores a blob produced by [`Self::save_state`].
    ///
    /// Returns `false` for a malformed blob, or when the widget currently has no tabs (there is
    /// no selection to restore). An index past the current last tab is clamped to that tab: the
    /// tab set is the host's model, and a saved index is a preference rather than an assertion
    /// that the set has not shrunk.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        if state.len() != 5 || state[0] != TAB_STATE_VERSION {
            return false;
        }
        if self.tabs.is_empty() {
            return false;
        }
        let index = u32::from_le_bytes([state[1], state[2], state[3], state[4]]) as usize;
        let index = index.min(self.tabs.len() - 1);
        // A saved index may name a tab that is currently hidden. Restoring the *selection* means
        // "this page is the visible one", so the nearest visible tab is what the blob can honestly
        // restore; when every tab is hidden there is no selection to restore and the blob is
        // refused rather than silently landing on a tab the user cannot see.
        let index = if self.tabs[index].visible {
            index
        } else {
            match self.nearest_visible_index(index) {
                Some(visible) => visible,
                None => return false,
            }
        };
        self.set_current_index(index);
        true
    }

    /// Places the current page in the content box.
    ///
    /// # Why the tab widget places its own page
    ///
    /// A tab page is a child of the tab widget, not a track in a layout: the widget owns
    /// exactly one visible rectangle (its content box) and one selection. The page is
    /// therefore placed from that model — the current page fills the content box. Without it
    /// the tree painter drew every page at whatever rectangle it was constructed with
    /// (normally `0x0`), so switching tabs changed the strip and nothing else.
    ///
    /// The write goes through the public geometry entry rather than a registry of draw
    /// closures, because the tree painter already draws mounted children: this is the
    /// placement half, not a second drawing path (rule #101).
    fn place_current_page(&self) {
        if let Some(widget_id) = self.current_widget() {
            let rect = self.content_rect();
            crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
        }
    }

    /// Applies the page model to the mounted pages: the current page is shown and fills the
    /// content box, every other page is hidden.
    ///
    /// # Why visibility is part of the model
    ///
    /// The page list is the control's content model, and only one page is on screen at a
    /// time. The tree painter skips a hidden node **and its whole subtree**, so hiding the
    /// inactive pages is what keeps them from painting over the current one. It is also what
    /// keeps a click on the content area from reaching a page the user cannot see. A caller
    /// that deliberately hides a page still wins: the next selection change re-derives the
    /// flag from the model, exactly as a tab container must.
    fn sync_pages(&self) {
        for (index, tab) in self.tabs.iter().enumerate() {
            if let Some(widget_id) = tab.widget {
                // A hidden tab's page is hidden too: the tab is not offered, so its page is not
                // part of the presented state.
                crate::set_widget_visible(widget_id, index == self.current_index && tab.visible);
            }
        }
        self.place_current_page();
    }
    /// Returns mutable tab at index.
    pub fn tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }

    /// Whether the tab at `index` is offered in the strip, or `None` for an unknown index.
    pub fn is_tab_visible(&self, index: usize) -> Option<bool> {
        self.tabs.get(index).map(|tab| tab.visible)
    }

    /// Shows or hides the tab at `index`, returning `false` for an unknown index.
    ///
    /// # What hiding does, and what it does not
    ///
    /// A hidden tab is not drawn, occupies no strip width (so the run closes up and the overflow
    /// rule sees one fewer tab), cannot be pressed, and its page is hidden. Its *index*, page and
    /// payload are untouched: hiding is a presentation fact rather than a second model, so an index
    /// a caller already holds keeps addressing the same tab.
    ///
    /// If the hidden tab was the selected one, the selection moves to the nearest visible tab (the
    /// one after it, else the one before it) and `current_changed` is emitted — leaving the selection
    /// on a tab that is not offered would make `selected_index()` report a page the user cannot see.
    /// Hiding *every* tab leaves the index where it is and shows no page; there is nothing to select.
    pub fn set_tab_visible(&mut self, index: usize, visible: bool) -> bool {
        if index >= self.tabs.len() {
            return false;
        }
        if self.tabs[index].visible == visible {
            return true;
        }
        self.tabs[index].visible = visible;
        let mut selection_changed = false;
        if !visible && self.current_index == index {
            if let Some(next) = self.nearest_visible_index(index) {
                self.current_index = next;
                selection_changed = true;
            }
        }
        self.sync_pages();
        self.base.request_redraw();
        if selection_changed {
            self.current_changed.emit(self.current_index);
        }
        true
    }

    /// The nearest visible tab to `from`, searching forward first and then backward.
    fn nearest_visible_index(&self, from: usize) -> Option<usize> {
        if let Some(next) = self.tabs.iter().skip(from).position(|tab| tab.visible) {
            return Some(from + next);
        }
        self.tabs[..from.min(self.tabs.len())].iter().rposition(|tab| tab.visible)
    }

    /// The visible tab `delta` slots away from `from`, wrapping around the strip.
    ///
    /// Returns `None` when no other tab is visible: a one-tab strip has nowhere to step to, and a
    /// strip with no visible tab has no selection to move.
    fn step_visible_index(&self, from: usize, delta: isize) -> Option<usize> {
        let count = self.tabs.len();
        if count == 0 {
            return None;
        }
        let start = from.min(count - 1) as isize;
        for step in 1..=count as isize {
            let candidate = (start + delta * step).rem_euclid(count as isize) as usize;
            if self.tabs[candidate].visible {
                return Some(candidate);
            }
        }
        None
    }

    /// Returns whether the strip is drawn in document mode.
    pub fn document_mode(&self) -> bool {
        self.document_mode
    }

    /// Enables or disables document-mode strip painting.
    ///
    /// Requests a redraw on a real change, because the flag changes how the strip's surface and the
    /// current tab's accent are painted. It changes nothing about indices, pages or hit testing.
    pub fn set_document_mode(&mut self, document_mode: bool) {
        if self.document_mode == document_mode {
            return;
        }
        self.document_mode = document_mode;
        self.base.request_redraw();
    }

    /// The payload carried by the tab at `index`.
    pub fn tab_data(&self, index: usize) -> Option<u64> {
        self.tabs.get(index).and_then(|tab| tab.data)
    }

    /// Sets the payload carried by the tab at `index`, returning `false` for an unknown index.
    ///
    /// The control stores and moves the value with its tab (a reorder takes the payload with it)
    /// and never interprets it.
    pub fn set_tab_data(&mut self, index: usize, data: Option<u64>) -> bool {
        let Some(tab) = self.tabs.get_mut(index) else {
            return false;
        };
        tab.data = data;
        true
    }

    /// Replaces the page widget of the tab at `index`, keeping the container's model in step.
    ///
    /// # Why this exists beside [`Tab::set_widget`]
    ///
    /// A page is not only a field: it is linked into the container's child list, placed in the
    /// content box when selected, and hidden when not. A caller that reaches a `Tab` through
    /// [`Self::tab_mut`] and assigns `widget` changes the field and none of those three facts, so
    /// the new page is neither shown nor hidden and the old one keeps painting. This method is
    /// the container-level entry that performs all four steps; `Tab::set_widget` remains for
    /// callers that are about to call `add_tab` (which links) and for tests that drive the raw
    /// field deliberately.
    ///
    /// Returns `false` for an out-of-range index, so a caller cannot mistake a dropped write for
    /// success.
    pub fn set_tab_widget(&mut self, index: usize, widget: Option<ObjectId>) -> bool {
        if index >= self.tabs.len() {
            return false;
        }
        let previous = self.tabs[index].widget;
        if previous == widget {
            return true;
        }
        if let Some(widget_id) = previous {
            self.base.remove_child_linked(widget_id);
            crate::set_widget_visible(widget_id, false);
        }
        self.tabs[index].widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
        }
        self.sync_pages();
        self.base.request_redraw();
        true
    }
    /// Assigns typed icon content to the tab at `index`.
    ///
    /// # Why an index and not a stable item id
    ///
    /// A `TabWidget` item has no identity beyond its position, and inventing one here would be a
    /// second selection model the control does not otherwise have. The caller therefore states the
    /// position it means, and an out-of-range index is reported rather than silently dropped.
    pub fn set_item_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), crate::widget::IconItemError> {
        match self.tabs.get_mut(index) {
            Some(tab) => {
                tab.set_icon_source(source);
                self.base.request_redraw();
                Ok(())
            }
            None => {
                Err(crate::widget::IconItemError::IndexOutOfBounds { index, len: self.tabs.len() })
            }
        }
    }
    /// Returns the text of the tab at the given index.
    pub fn tab_text(&self, index: usize) -> Option<&str> {
        self.tabs.get(index).map(|t| t.title.as_str())
    }
    /// Sets the text of the tab at the given index.
    pub fn set_tab_text(&mut self, index: usize, text: String) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.title = text;
        }
        self.base.request_redraw();
    }
    /// Returns tab position.
    pub fn tab_position(&self) -> TabPosition {
        self.tab_position
    }
    /// Sets tab position.
    pub fn set_tab_position(&mut self, position: TabPosition) {
        if self.tab_position == position {
            return;
        }
        self.tab_position = position;
        // The strip moved, so the content box moved with it: the selected page has to follow,
        // or it keeps the rectangle the previous side implied until something else resizes it.
        self.place_current_page();
        self.base.request_redraw();
    }
    /// Returns tab shape.
    pub fn tab_shape(&self) -> TabShape {
        self.tab_shape
    }
    /// Sets tab shape.
    pub fn set_tab_shape(&mut self, shape: TabShape) {
        if self.tab_shape == shape {
            return;
        }
        self.tab_shape = shape;
        self.base.request_redraw();
    }
    /// Returns whether tabs are closable.
    pub fn closable(&self) -> bool {
        self.closable
    }
    /// Sets closable state.
    pub fn set_closable(&mut self, closable: bool) {
        if self.closable == closable {
            return;
        }
        self.closable = closable;
        self.base.request_redraw();
    }
    /// Returns whether tabs are movable.
    ///
    /// When true, pressing a tab and dragging the pointer past a neighbour's midpoint
    /// reorders the tabs and emits [`Self::tab_moved`]. When false the same gesture only
    /// selects, so the flag decides whether a drag has any effect at all.
    pub fn movable(&self) -> bool {
        self.movable
    }
    /// Sets movable state.
    ///
    /// A gesture already in progress is abandoned: turning reordering off mid-drag must
    /// not let the release that follows still move a tab, which is what dropping the live
    /// drag state here prevents.
    pub fn set_movable(&mut self, movable: bool) {
        if self.movable == movable {
            return;
        }
        self.movable = movable;
        if !movable {
            self.cancel_tab_drag();
        }
        self.base.request_redraw();
    }
    /// Moves the tab at `from` to `to` and emits [`Self::tab_moved`] with `(from, to)`.
    ///
    /// # Why `to` is clamped rather than rejected
    ///
    /// A drag reports positions from pointer coordinates, and a pointer past the last tab
    /// is "dropped at the end" — the intent — not a caller bug. An out-of-range `from`,
    /// however, can only be a caller error, so it is rejected without emitting.
    ///
    /// # Why the selection travels with the tab
    ///
    /// `current_index` is a slot, but the user's mental model is that the *page* moved.
    /// Leaving the index alone would silently switch to whichever tab took the slot the
    /// dragged one vacated.
    ///
    /// Returns `true` when a move happened.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() {
            return false;
        }
        let to = to.min(self.tabs.len() - 1);
        if to == from {
            return false;
        }

        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        self.current_index = match self.current_index {
            c if c == from => to,
            c if from < c && c <= to => c - 1,
            c if to <= c && c < from => c + 1,
            c => c,
        };

        self.tab_moved.emit((from, to));
        self.base.request_redraw();
        true
    }
    /// Starts a move gesture on the tab under `pos`.
    ///
    /// Only a movable, enabled tab arms a drag, so a click that happens to jitter still
    /// selects and nothing else. The session is opened with the tab's index as its payload
    /// so a future drop target can tell which tab is being carried.
    fn begin_tab_drag(&mut self, pos: Point) {
        if !self.movable {
            return;
        }
        let Some(index) = self.tab_at_position(pos) else {
            return;
        };
        if !self.tabs[index].enabled {
            return;
        }
        let payload = DragPayload::new(TAB_DRAG_TYPE, index.to_string()).with_origin(pos);
        self.drag_session = Some(DragSession::begin(payload, pos));
        self.dragging_from = Some(index);
        self.live_drag_origin = Some(index);
    }
    /// Reorders the tabs as the pointer moves past a neighbour's midpoint.
    ///
    /// # Why the move is applied live rather than on release
    ///
    /// The tab strip swaps the two tabs the moment the pointer crosses the midpoint, so
    /// the strip the user sees while dragging is the arrangement they will get. Deferring
    /// it to the release makes the gesture feel unresponsive and gives no feedback about
    /// where the tab will land.
    fn update_tab_drag(&mut self, pos: Point) {
        let (active, from) = {
            let Some(session) = self.drag_session.as_mut() else {
                return;
            };
            session.update(pos, TAB_DRAG_THRESHOLD);
            (session.is_active(), self.dragging_from)
        };
        if !active {
            return;
        }
        let Some(from) = from else {
            return;
        };
        // The neighbour the pointer has passed: a move to the right lands on the next tab
        // once the pointer is past *that* tab's midpoint, and symmetrically to the left.
        // Deriving the target from geometry rather than from the dragged tab's own rect is
        // what lets a fast drag cross several neighbours without stalling on each one.
        let Some(target) = self.tab_at_position(pos) else {
            return;
        };
        if target == from {
            return;
        }
        // Move one step toward the target rather than jumping the whole distance: the tabs
        // between the two indices have to shift by one, which a single `move_tab(from,
        // target)` accomplishes, and the live `move_tab` re-emits so the host sees each
        // swap the user saw.
        self.move_tab(from, target);
        self.dragging_from = Some(target);
    }
    /// Ends a move gesture, keeping the reordering already applied.
    ///
    /// The session is closed without a further move: `update_tab_drag` already placed the
    /// tab under the pointer, so acting on the release position again would double-apply
    /// the last step. The carried slot is retained until `on_drag_finished`, because a
    /// coordinator that observed the same gesture still has to commit against it.
    fn end_tab_drag(&mut self) {
        self.drag_session = None;
    }
    /// Drops any in-progress move gesture without reordering further.
    fn cancel_tab_drag(&mut self) {
        self.drag_session = None;
        self.dragging_from = None;
        self.live_drag_origin = None;
    }
    /// Returns tab rectangle at index.
    ///
    /// Tab width is **measured**, not a constant. The literal `100` meant a title longer than
    /// ~13 characters was clipped at a fixed point and a two-character title reserved the same
    /// 100 px as a nine-character one, so the strip's appearance had nothing to do with its
    /// contents. Measuring the titles and clamping the result (the same `[40, 200]` window
    /// `tab_bar` uses) is what makes the band a function of what the tabs say.
    ///
    /// The computed width is also what the **overflow** rule divides up: when the tabs no
    /// longer fit the strip, every tab gets `width / count` so they all stay visible, rather
    /// than the later ones being drawn past the control's own right edge (the SVG backend emits
    /// absolute coordinates, so those tabs simply left the picture).
    /// The declared width of each tab as a **hint triple**, not a single number.
    ///
    /// # Why three numbers rather than one (BLUE22 · G-1)
    ///
    /// This used to return one width per tab, and `tab_run` handed each to `add_sized`, which
    /// writes the value into `min`, `pref` **and** `max` alike. For a strip that fits that is
    /// exactly right. For one that does not, it made the control lie: it had already decided the
    /// tabs must share the strip (the overflow rule below), computed each share, and then told the
    /// layout "this tab is never allowed to be narrower than its share" — while handing the layout
    /// a band smaller than the sum of those shares. `FlexLayout` could only believe the declaration
    /// and scale the run down to fit, so the strip's own overflow arithmetic and the layout's
    /// answer were each applied once and the two rounding steps compounded. Two 72 px tabs plus a
    /// 2 px gap in a 300 px strip came out 71 and 63 instead of a clean share of 149 each.
    ///
    /// The fix is not in the layout: it is that the **declaration has to say what the control
    /// means**. A tab's `min` is the narrowest it can be drawn, and that is
    /// [`MIN_TAB_WIDTH`]'s floor, not the share — the share is what the tab *wants* given that the
    /// strip is crowded, which is precisely `pref`/`max`. With the ceiling stated, the layout
    /// reaches the answer in one step and the 1 px compounding disappears, because it is no longer
    /// squeezing a child past a floor the child never had.
    ///
    /// Returns `(min, pref, max)` per tab, in strip order.
    ///
    /// A hidden tab reports `(0, 0, 0)`: it is not offered, so it neither claims a share of the
    /// strip nor forces the overflow rule to divide one more way. The vector keeps one entry per
    /// tab (rather than one per *visible* tab) so its positions still address `self.tabs` by
    /// index — the run and the hit test both rely on that alignment.
    fn tab_width_hints(&self) -> crate::compat::Vec<(i32, i32, i32)> {
        let rect = self.geometry();
        let count = self.tabs.len();
        if count == 0 {
            return crate::compat::Vec::new();
        }
        let measured = self.measured_tab_widths();
        let visible: crate::compat::Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.visible)
            .map(|(index, _)| index)
            .collect();
        if visible.is_empty() {
            return crate::compat::vec![(0, 0, 0); count];
        }
        let spacing = scaled_tab_spacing();
        let gaps = spacing * (visible.len() as i32 - 1);
        let total: i32 = visible.iter().map(|index| measured[*index]).sum::<i32>() + gaps;
        let available = match self.tab_position {
            TabPosition::North | TabPosition::South => rect.width as i32,
            TabPosition::West | TabPosition::East => rect.height as i32,
        };
        let mut hints = crate::compat::vec![(0, 0, 0); count];
        if total <= available {
            // The strip fits: the measurement is both the wish and the bound, so the three numbers
            // agree and the drawn run is exactly what it always was.
            for index in visible {
                hints[index] = (measured[index], measured[index], measured[index]);
            }
            return hints;
        }
        // Overflow: share the strip equally among the tabs that are offered. The gap must come out
        // of the band *before* the division, otherwise the last tab's trailing gap is charged to
        // the tabs themselves and the run overshoots the strip.
        let share = ((available - gaps) / visible.len() as i32).max(MIN_TAB_WIDTH / 2);
        // `pref == max == share` with `min` at the floor: the tab *wants* no more than its share, and
        // can survive down to the floor. A layout given this reaches the share itself rather than
        // having to squeeze past a floor that claimed to be the share.
        let floor = MIN_TAB_WIDTH / 2;
        for index in visible {
            hints[index] = (floor.min(share), share, share);
        }
        hints
    }

    /// The measured (and clamped) preferred width of every tab, from the cache when its key still
    /// matches.
    ///
    /// The key is "whatever the measurement reads": the caption text, whether the tab reserves an
    /// icon slot, and the effective font. That makes the cache self-invalidating — a caller cannot
    /// reach a state where the width is a function of something the key omitted — and it costs one
    /// hash of the titles plus a `Font` comparison per call instead of a shaping pass per tab.
    fn measured_tab_widths(&self) -> crate::compat::Vec<i32> {
        use std::hash::{Hash, Hasher};

        let font = effective_font(self.style()).clone();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for tab in &self.tabs {
            tab.title.hash(&mut hasher);
            tab.has_icon().hash(&mut hasher);
        }
        let fingerprint = hasher.finish();

        {
            let cache = self.tab_width_cache.borrow();
            if cache.font.as_ref() == Some(&font) && cache.fingerprint == fingerprint {
                return cache.widths.clone();
            }
        }

        // The shared estimate, not `chars().count() * TAB_CHAR_WIDTH`: one derivation of a
        // label's advance for the whole crate, matching what `TabView::tab_widths` uses and
        // what the renderer draws with. The hand-rolled form also mis-measured any
        // non-Latin caption, since it charged a fixed 8 px per cluster.
        let widths: crate::compat::Vec<i32> = self
            .tabs
            .iter()
            .map(|tab| {
                (estimate_text_width(&tab.title, &font, 1.0) as i32
                    + TAB_TEXT_PADDING
                    + if tab.has_icon() { (TAB_ICON_SIDE + TAB_ICON_GAP) as i32 } else { 0 })
                .clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH)
            })
            .collect();

        *self.tab_width_cache.borrow_mut() =
            TabWidthCache { font: Some(font), fingerprint, widths: widths.clone() };
        widths
    }

    /// The tab strip as a run, in strip order and strip coordinates starting at `(0, 0)`.
    ///
    /// Each tab's width comes from [`Self::tab_width_hints`] — already measured and, on a crowded
    /// strip, already reduced to its share — and the strip is sized to exactly `sum(widths) +
    /// gaps`. A layout given fixed boxes in a strip that fits returns them unchanged, so the run is
    /// one pass of integer additions. This used to build a `CompositeBuilder`, a `FlexLayout` and a
    /// temporary `label` widget per tab; `tab_rects` is called by both the paint path and the hit
    /// test, which made that one widget per tab per event. See the module docs.
    ///
    /// Keeping the assembly's origin at zero is what lets one function serve all four
    /// `TabPosition`s, whose tab boxes differ only by the translation `tab_rects` applies.
    fn tab_run(&self) -> crate::compat::Vec<Rect> {
        let hints = self.tab_width_hints();
        if hints.is_empty() {
            return crate::compat::Vec::new();
        }
        let horizontal = matches!(self.tab_position, TabPosition::North | TabPosition::South);
        let spacing = scaled_tab_spacing();
        let mut placed: crate::compat::Vec<Rect> = crate::compat::Vec::with_capacity(hints.len());
        let mut cursor = 0i32;
        for (index, (_, preferred, _)) in hints.iter().enumerate() {
            // A hidden tab keeps a slot in the run, so every later index still lines up with its
            // rectangle, but the slot has no extent and consumes neither width nor a gap.
            if !self.tabs.get(index).is_some_and(|tab| tab.visible) {
                placed.push(if horizontal {
                    Rect::new(cursor, 0, 0, TAB_HEIGHT as u32)
                } else {
                    Rect::new(0, cursor, TAB_HEIGHT as u32, 0)
                });
                continue;
            }
            let rect = if horizontal {
                Rect::new(cursor, 0, *preferred as u32, TAB_HEIGHT as u32)
            } else {
                Rect::new(0, cursor, TAB_HEIGHT as u32, *preferred as u32)
            };
            placed.push(rect);
            cursor += *preferred + spacing;
        }
        placed
    }

    /// The close affordance's rectangle inside `tab_rect`.
    ///
    /// # Why this is a method and not two copies
    ///
    /// `draw` and `handle_event` both need this rectangle, and they had it twice — differently.
    /// The paint path used `CLOSE_SIZE` and centred on `tab_line.y` (the title's own line box, from
    /// which the ✕ is drawn); the hit test used a bare `12` and centred on `tab_rect.y` (the whole
    /// tab). Two consequences, both small enough to miss by eye:
    ///
    /// * the literal `12` was a second copy of `CLOSE_SIZE`, so changing the glyph size would have
    ///   moved what is painted and left what is clickable where it was;
    /// * the two origins differ by half a line, so the glyph's top and bottom rows fell outside its
    ///   own hit area while a band of the tab *above* the glyph was clickable.
    ///
    /// # Why the line box is measured here rather than passed in
    ///
    /// `handle_event` has no `RenderContext` — a pointer event arrives without one — so the hit test
    /// cannot call `text_line`. It can ask the same context-free quantity the centred line is built
    /// from: `estimate_line_height`. `text_line` centres a glyph box of `measure_text("M")` height
    /// inside the band, and `estimate_line_height` is that height's context-free counterpart — the
    /// same relationship `Label::implicit_size` relies on. Deriving both from corresponding heights
    /// keeps the two origins within a pixel rather than half a line, and having **one** derivation is
    /// what stops them being edited apart again.
    ///
    /// Returns `None` when the control is not closable, so a caller cannot render or hit-test an
    /// affordance that is not offered.
    fn close_button_rect(&self, tab_rect: Rect, font: &Font) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        let line_height = crate::widget::metrics::estimate_line_height(font, 1.0) as i32;
        let line_top = tab_rect.y + ((tab_rect.height as i32 - line_height) / 2).max(0);
        let close_x = tab_rect.x + tab_rect.width as i32 - CLOSE_SIZE - CLOSE_LABEL_GAP;
        let close_y = line_top + (line_height - CLOSE_SIZE) / 2;
        Some(Rect::new(close_x, close_y, CLOSE_SIZE as u32, CLOSE_SIZE as u32))
    }

    fn tab_rect(&self, index: usize) -> Option<Rect> {
        self.tab_rects().get(index).copied()
    }

    /// Every tab's rectangle, in strip order, derived **once**.
    ///
    /// # Why this exists
    ///
    /// [`Self::tab_rect`] used to call [`Self::tab_run`] on every invocation, and the draw pass
    /// asks for every tab (and the hit test asks for every tab on a press). A strip of `n` tabs
    /// therefore paid `n` runs **per frame**. This method is the single derivation the callers that
    /// need more than one box use. `tab_run` is now one pass over the measured widths (see its
    /// docs), so the remaining cost is proportional to the tab count rather than to widget
    /// construction and a layout pass per tab.
    fn tab_rects(&self) -> Vec<Rect> {
        let rect = self.geometry();
        let tab_height = TAB_HEIGHT as u32;
        let scroll = self.overflow_scroll;
        self.tab_run()
            .into_iter()
            .map(|tab| {
                let tab_width = tab.width;
                let along = tab.x - scroll;
                match self.tab_position {
                    TabPosition::North => Rect::new(rect.x + along, rect.y, tab_width, tab_height),
                    TabPosition::South => Rect::new(
                        rect.x + along,
                        rect.y + rect.height as i32 - TAB_HEIGHT,
                        tab_width,
                        tab_height,
                    ),
                    // A vertical strip runs **down** the control: the layout's horizontal answer
                    // is the position along the strip, and the caption's width becomes the box
                    // width (the label stays horizontal, as every shipped tab strip draws it).
                    TabPosition::West => Rect::new(rect.x, rect.y + along, tab_width, tab_height),
                    TabPosition::East => Rect::new(
                        rect.x + rect.width as i32 - tab_width as i32,
                        rect.y + tab.y - scroll,
                        tab_width,
                        tab_height,
                    ),
                }
            })
            .collect()
    }
    /// The strip's own band — where the tabs are drawn, as opposed to the page below them.
    ///
    /// Derived from the same four `TabPosition` arms as [`Self::content_rect`], so the strip and
    /// the page are complementary by construction. Document mode fills this band with the control's
    /// surface; the per-tab painting sits on top of it.
    fn tab_strip_rect(&self) -> Rect {
        let rect = self.geometry();
        let tab_height = TAB_HEIGHT as u32;
        match self.tab_position {
            TabPosition::North => Rect::new(rect.x, rect.y, rect.width, tab_height),
            TabPosition::South => {
                Rect::new(rect.x, rect.y + rect.height as i32 - TAB_HEIGHT, rect.width, tab_height)
            }
            TabPosition::West => Rect::new(rect.x, rect.y, tab_height, rect.height),
            TabPosition::East => {
                Rect::new(rect.x + rect.width as i32 - TAB_HEIGHT, rect.y, tab_height, rect.height)
            }
        }
    }

    /// The rule document mode draws under the current tab, along the strip's inner edge.
    fn document_rule_rect(&self, tab_rect: Rect) -> Rect {
        let thickness: u32 = 2;
        match self.tab_position {
            TabPosition::North => Rect::new(
                tab_rect.x,
                tab_rect.y + tab_rect.height as i32 - thickness as i32,
                tab_rect.width,
                thickness,
            ),
            TabPosition::South => Rect::new(tab_rect.x, tab_rect.y, tab_rect.width, thickness),
            TabPosition::West => Rect::new(
                tab_rect.x + tab_rect.width as i32 - thickness as i32,
                tab_rect.y,
                thickness,
                tab_rect.height,
            ),
            TabPosition::East => Rect::new(tab_rect.x, tab_rect.y, thickness, tab_rect.height),
        }
    }

    /// Returns content rectangle.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let tab_height = TAB_HEIGHT;
        match self.tab_position {
            TabPosition::North => Rect::new(
                rect.x,
                rect.y + tab_height,
                rect.width,
                rect.height.saturating_sub(tab_height as u32),
            ),
            TabPosition::South => {
                Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(tab_height as u32))
            }
            TabPosition::West => Rect::new(
                rect.x + tab_height,
                rect.y,
                rect.width.saturating_sub(tab_height as u32),
                rect.height,
            ),
            TabPosition::East => {
                Rect::new(rect.x, rect.y, rect.width.saturating_sub(tab_height as u32), rect.height)
            }
        }
    }
    /// Whether the strip draws scroll arrows when the tabs overflow.
    pub fn show_overflow_buttons(&self) -> bool {
        self.show_overflow_buttons
    }

    /// Enables or disables the overflow arrows.
    ///
    /// Off by default, so the historical proportional shrink is unchanged. When on and the
    /// tabs do not fit, the strip reserves an arrow at each end and the user can scroll it.
    pub fn set_show_overflow_buttons(&mut self, show: bool) {
        if self.show_overflow_buttons == show {
            return;
        }
        self.show_overflow_buttons = show;
        if !show {
            self.overflow_scroll = 0;
        }
        self.base.request_redraw();
    }

    /// The current tab-strip scroll offset along its axis.
    pub fn overflow_scroll(&self) -> i32 {
        self.overflow_scroll
    }

    /// The largest useful overflow offset.
    ///
    /// The run's end is the far edge of the last tab that has an extent along the strip. A hidden
    /// tab still holds a slot in the run (so indices line up) but has no extent, and counting it
    /// would let the strip scroll past its last visible tab by one gap.
    fn max_overflow_scroll(&self) -> i32 {
        let horizontal = matches!(self.tab_position, TabPosition::North | TabPosition::South);
        let run = self.tab_run();
        let end = run
            .iter()
            .filter(|tab| if horizontal { tab.width > 0 } else { tab.height > 0 })
            .map(
                |tab| if horizontal { tab.x + tab.width as i32 } else { tab.y + tab.height as i32 },
            )
            .max()
            .unwrap_or(0);
        let strip = match self.tab_position {
            TabPosition::North | TabPosition::South => self.geometry().width as i32,
            TabPosition::West | TabPosition::East => self.geometry().height as i32,
        };
        (end - strip).max(0)
    }

    /// Scrolls the strip by `delta` pixels, clamped to the useful range.
    pub fn scroll_tabs(&mut self, delta: i32) {
        let next = (self.overflow_scroll + delta).clamp(0, self.max_overflow_scroll());
        if next == self.overflow_scroll {
            return;
        }
        self.overflow_scroll = next;
        self.base.request_redraw();
    }

    /// How an over-long title is shortened.
    pub fn elide_mode(&self) -> TabElideMode {
        self.elide_mode
    }

    /// Sets the title elision mode; the default clips, as before.
    pub fn set_elide_mode(&mut self, mode: TabElideMode) {
        if self.elide_mode == mode {
            return;
        }
        self.elide_mode = mode;
        self.base.request_redraw();
    }

    /// The tab under the pointer, if any.
    pub fn hovered_tab(&self) -> Option<usize> {
        self.hovered_tab
    }

    /// The tooltip of the tab under the pointer, if it has one.
    ///
    /// The tooltip text was stored and never reachable: no pointer path recorded a hovered
    /// tab and no query exposed its text. A host can now paint a tooltip from this answer.
    pub fn hovered_tooltip(&self) -> Option<&str> {
        let index = self.hovered_tab?;
        self.tabs.get(index).map(|tab| tab.tooltip.as_str()).filter(|text| !text.is_empty())
    }

    /// An optional widget placed in the strip's unused corner.
    pub fn corner_widget(&self) -> Option<ObjectId> {
        self.corner_widget
    }

    /// Sets (or clears) the corner widget and places it.
    pub fn set_corner_widget(&mut self, widget: Option<ObjectId>) {
        if self.corner_widget == widget {
            return;
        }
        if let Some(previous) = self.corner_widget {
            self.base.remove_child(previous);
        }
        self.corner_widget = widget;
        if let Some(widget) = widget {
            self.base.add_child_linked(widget);
            self.place_corner_widget();
        }
        self.base.request_redraw();
    }

    /// The rectangle the corner widget occupies.
    pub fn corner_rect(&self) -> Rect {
        const CORNER_WIDTH: u32 = 80;
        let rect = self.geometry();
        match self.tab_position {
            TabPosition::North => Rect::new(
                rect.x + rect.width.saturating_sub(CORNER_WIDTH) as i32,
                rect.y,
                CORNER_WIDTH,
                TAB_HEIGHT as u32,
            ),
            TabPosition::South => Rect::new(
                rect.x + rect.width.saturating_sub(CORNER_WIDTH) as i32,
                rect.y + rect.height as i32 - TAB_HEIGHT,
                CORNER_WIDTH,
                TAB_HEIGHT as u32,
            ),
            TabPosition::West => Rect::new(
                rect.x,
                rect.y + rect.height.saturating_sub(CORNER_WIDTH) as i32,
                TAB_HEIGHT as u32,
                CORNER_WIDTH,
            ),
            TabPosition::East => Rect::new(
                rect.x + rect.width as i32 - TAB_HEIGHT,
                rect.y + rect.height.saturating_sub(CORNER_WIDTH) as i32,
                TAB_HEIGHT as u32,
                CORNER_WIDTH,
            ),
        }
    }

    fn place_corner_widget(&self) {
        if let Some(widget) = self.corner_widget {
            let rect = self.corner_rect();
            crate::set_widget_geometry(widget, rect.x, rect.y, rect.width, rect.height);
        }
    }

    /// The two overflow arrow boxes, or `None` when the strip has no overflow.
    fn overflow_button_rects(&self) -> Option<(Rect, Rect)> {
        const BUTTON: u32 = 16;
        if !self.show_overflow_buttons || self.max_overflow_scroll() == 0 {
            return None;
        }
        let rect = self.geometry();
        let first = match self.tab_position {
            TabPosition::North => Rect::new(rect.x, rect.y, BUTTON, TAB_HEIGHT as u32),
            TabPosition::South => Rect::new(
                rect.x,
                rect.y + rect.height as i32 - TAB_HEIGHT,
                BUTTON,
                TAB_HEIGHT as u32,
            ),
            TabPosition::West => Rect::new(rect.x, rect.y, TAB_HEIGHT as u32, BUTTON),
            TabPosition::East => Rect::new(
                rect.x + rect.width as i32 - TAB_HEIGHT,
                rect.y,
                TAB_HEIGHT as u32,
                BUTTON,
            ),
        };
        let second = match self.tab_position {
            TabPosition::North => Rect::new(
                rect.x + rect.width.saturating_sub(BUTTON) as i32,
                rect.y,
                BUTTON,
                TAB_HEIGHT as u32,
            ),
            TabPosition::South => Rect::new(
                rect.x + rect.width.saturating_sub(BUTTON) as i32,
                rect.y + rect.height as i32 - TAB_HEIGHT,
                BUTTON,
                TAB_HEIGHT as u32,
            ),
            TabPosition::West => Rect::new(
                rect.x,
                rect.y + rect.height.saturating_sub(BUTTON) as i32,
                TAB_HEIGHT as u32,
                BUTTON,
            ),
            TabPosition::East => Rect::new(
                rect.x + rect.width as i32 - TAB_HEIGHT,
                rect.y + rect.height.saturating_sub(BUTTON) as i32,
                TAB_HEIGHT as u32,
                BUTTON,
            ),
        };
        Some((first, second))
    }

    /// Returns index of tab at position.
    fn tab_at_position(&self, pos: Point) -> Option<usize> {
        self.tab_rects().into_iter().position(|tab_rect| tab_rect.contains(pos))
    }
}
/// Height of the tab strip, in logical pixels.
///
/// Read from the shared metric rather than restated as a literal: the strip height is one
/// number across the control family, and a second copy is how siblings drift (rule #101).
const TAB_HEIGHT: i32 = crate::widget::metrics::dimensions::TAB_HEIGHT as i32;

/// Horizontal gap between adjacent tabs.
const TAB_SPACING: i32 = 2;

/// `TAB_SPACING` as the strip actually charges it.
///
/// The run reads the device's text-scale preference through `LayoutContext::default()`, so the gap
/// widens with it. The strip's band and the width hints must therefore be sized with the *same*
/// gap, or the run overflows the band it was given and the proportional shrink takes a pixel off
/// the tabs: a 100 px strip of two shares (each 49, the nominal 2 counted once) drew as two 48s
/// once the run charged 3. Deriving the band from this same value keeps the declaration and the
/// run in agreement on every device.
fn scaled_tab_spacing() -> i32 {
    let context = crate::layout::types::LayoutContext::default();
    let scale = context.layout_scale.max(context.font_scale);
    (TAB_SPACING as f32 * scale).round() as i32
}

/// Shortens a tab title to `width`, adding the ellipsis a shortened title must keep.
///
/// The fit itself is [`text_utils::elide_to_width`](crate::widget::text_utils::elide_to_width),
/// the crate's one width-budget elider, so a tab title is cut exactly as a label or a segment
/// caption is. The rule added here is this control's floor: a title that must be shortened
/// never paints as nothing. The shared primitive reports "not even the ellipsis fits" as an
/// empty string, while a blank tab would read as a tab with no caption rather than a tab whose
/// caption was cut.
fn elide_tab_title<'a>(
    text: &'a str,
    width: u32,
    measure: &mut dyn FnMut(&str) -> u32,
) -> Cow<'a, str> {
    if width == 0 || text.is_empty() {
        return Cow::Borrowed("");
    }
    let fitted = crate::widget::text_utils::elide_to_width(text, width, measure);
    if fitted.is_empty() {
        Cow::Borrowed("\u{2026}")
    } else {
        fitted
    }
}

/// Widest a measured tab may become, and narrowest it may stay.
///
/// The same window `tab_bar` clamps to, so the two tab controls agree about what a tab looks
/// like even though they lay their bands out differently.
const MIN_TAB_WIDTH: i32 = 40;
const MAX_TAB_WIDTH: i32 = 200;

/// Padding added to a measured title before it is clamped.
const TAB_TEXT_PADDING: i32 = 24;

/// Square side of the leading icon slot a tab reserves, in logical pixels.
///
/// One number for both the width measurement and the paint path, so the room the strip allocates
/// and the room the icon occupies cannot disagree.
const TAB_ICON_SIDE: u32 = 12;
/// Gap between a tab's icon slot and its title.
const TAB_ICON_GAP: u32 = 4;

/// Side of a tab's close button, in logical pixels.
const CLOSE_SIZE: i32 = 12;
/// Gap between the close glyph and the tab's trailing edge.
///
/// Named because the paint path, the title's clip width and the hit test all need the same
/// distance; the title additionally reserves [`CLOSE_TITLE_CLEARANCE`] so the elided caption stops
/// before the glyph rather than underneath it.
const CLOSE_LABEL_GAP: i32 = 5;
/// Extra room the title reserves past [`CLOSE_LABEL_GAP`].
const CLOSE_TITLE_CLEARANCE: i32 = 5;

/// The drag payload type a tab move carries.
///
/// Named rather than an empty string so a drop target (or a future cross-control reorder)
/// can recognise that this gesture is a tab move and not, say, a card drag. The payload's
/// item id is the dragged tab's index, spelled as the same decimal the property layer uses.
const TAB_DRAG_TYPE: &str = "tab_widget_tab";

/// How far the pointer must travel before a press counts as a drag.
///
/// Without a threshold every click that jitters by one pixel would reorder a tab, because
/// the press and the move are the same gesture to the OS.
const TAB_DRAG_THRESHOLD: i32 = 4;

/// Version tag of the blob produced by [`TabWidget::save_state`].
const TAB_STATE_VERSION: u8 = 1;

impl SaveState for TabWidget {
    fn state_version(&self) -> u16 {
        TAB_STATE_VERSION as u16
    }

    fn save_state(&self) -> Vec<u8> {
        TabWidget::save_state(self)
    }

    fn restore_state(&mut self, state: &[u8]) -> bool {
        TabWidget::restore_state(self, state)
    }
}

// Implement Widget trait
impl Widget for TabWidget {
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
        crate::core::Size::new(300, 200)
    }

    /// Opts a movable strip into the runtime's drag coordinator as a source.
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        if !self.movable {
            return None;
        }
        // A coordinator press calls this hook before it records the payload. When no live
        // pointer gesture is open, any carried slot still here belongs to an older gesture
        // and must be forgotten, or the drop would commit against a stale tab.
        if self.drag_session.is_none() {
            self.dragging_from = None;
            self.live_drag_origin = None;
        }
        Some(self)
    }

    /// Opts a movable strip into the runtime's drag coordinator as a drop target.
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        if self.movable {
            Some(self)
        } else {
            None
        }
    }

    /// Emits a context-menu request only when a tab is under the point.
    ///
    /// A request on the content page belongs to the page's own control (if any), not to the
    /// strip, so answering `false` there keeps the host from showing a tab menu over content.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        if self.tab_at_position(pos).is_none() {
            return false;
        }
        self.context_menu_requested.emit(pos);
        true
    }

    /// Moves the tab widget and re-places the current page.
    ///
    /// A layout positions the tab widget through the same geometry entry point, so this is
    /// where a window or panel resize reaches the visible page.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_current_page();
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "tab_close_requested" => Some(EventSignalRef::mapped(
                "tab_close_requested",
                &self.tab_close_requested,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "tab_moved" => Some(EventSignalRef::mapped("tab_moved", &self.tab_moved, |v| {
                // The declared shape is `Tuple2` of `UInt`, so the payload arrives as the
                // (old_index, new_index) pair rather than its `Debug` spelling.
                CapabilityValue::Tuple(crate::compat::Vec::from([
                    CapabilityValue::UInt(v.0 as u64),
                    CapabilityValue::UInt(v.1 as u64),
                ]))
            })),
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| CapabilityValue::String(format!("{v:?}")),
            )),
            "drag_finished" => {
                Some(EventSignalRef::mapped("drag_finished", &self.drag_finished, |v| {
                    CapabilityValue::String(v.as_str().to_string())
                }))
            }
            "tab_bar_clicked" => {
                Some(EventSignalRef::mapped("tab_bar_clicked", &self.tab_bar_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "tab_bar_double_clicked" => Some(EventSignalRef::mapped(
                "tab_bar_double_clicked",
                &self.tab_bar_double_clicked,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            _ => None,
        }
    }
}

/// A tab drag carries the tab's index in the item id and its caption in the label.
///
/// The index is the reorder key the drop target reads back, so a coordinator-only host that
/// never drives the pointer path can still reorder the strip. The caption and the page travel in
/// the label and in [`DragPayload::attached`] because a *different* strip inserts those, and an
/// index means nothing outside the strip that produced it.
impl DragSource for TabWidget {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.movable {
            return None;
        }
        let index = self.tab_at_position(pos)?;
        let tab = self.tabs.get(index)?;
        if !tab.enabled {
            return None;
        }
        self.pending_drag_tab.set(Some(index));
        self.drop_committed_here.set(false);
        let payload = DragPayload::new(TAB_DRAG_TYPE, index.to_string())
            .with_label(tab.title.clone())
            .with_origin(pos);
        let payload = match tab.widget {
            Some(page) => payload.with_attached(page),
            None => payload,
        };
        Some(payload)
    }

    /// Ends the gesture, removing the source copy of a tab that was transferred away.
    ///
    /// A reorder inside this strip already committed in `on_drop`, so nothing is removed. A
    /// transfer inserted the tab (and reparented its page) in another strip and reported `Move`,
    /// which is the signal that this strip's copy has to go; a refusal reports `None` and the tab
    /// stays where it was.
    fn on_drag_finished(&mut self, effect: DropEffect) {
        let moved_away = effect.consumes_source() && !self.drop_committed_here.get();
        if moved_away {
            if let Some(index) = self.pending_drag_tab.take() {
                self.remove_tab(index);
            }
        }
        self.pending_drag_tab.set(None);
        self.drop_committed_here.set(false);
        self.cancel_tab_drag();
        self.drag_finished.emit(effect);
    }
}

/// A dropped tab moves to the tab under the pointer.
///
/// # Reorder versus transfer
///
/// A payload taken from this strip names one of its tabs, so the drop moves it — using the live
/// path's current slot when both paths observed the same gesture. A payload taken from *another*
/// strip names an index that means nothing here, so the drop inserts the carried caption at the
/// tab under the pointer and adopts the page it carries.
impl DropTarget for TabWidget {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        if !self.movable || !payload.is_type(TAB_DRAG_TYPE) {
            return false;
        }
        if payload.came_from(self.base.id()) {
            return payload.item_id.parse::<usize>().is_ok_and(|index| index < self.tabs.len());
        }
        !payload.label.is_empty()
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) {
            return DropEffect::None;
        }
        let Some(target) = self.tab_at_position(pos) else {
            return DropEffect::None;
        };
        if payload.came_from(self.base.id()) {
            let Ok(original) = payload.item_id.parse::<usize>() else {
                return DropEffect::None;
            };
            // The pointer path reorders live and records the carried tab's current slot in
            // `dragging_from`. Using that slot when its origin matches the payload keeps the
            // coordinator's commit idempotent when both paths observed the same gesture;
            // otherwise the payload's own index is still the tab's slot.
            let from = match (self.live_drag_origin, self.dragging_from) {
                (Some(origin), Some(current)) if origin == original => current,
                _ => original,
            };
            if from >= self.tabs.len() {
                return DropEffect::None;
            }
            if !self.move_tab(from, target) {
                // A drop onto the dragged tab itself is accepted without moving anything.
                if from != target {
                    return DropEffect::None;
                }
            }
        } else {
            // `insert_tab` clamps and links the page: `add_child_linked` also takes the page out
            // of the strip it came from, which is why the source's later `remove_tab` must not
            // unlink it again.
            self.insert_tab(target, payload.label.clone(), payload.attached);
        }
        self.drop_committed_here.set(true);
        DropEffect::Move
    }

    /// Highlights the band the tab would land on.
    fn preview_rect(&self, payload: &DragPayload, pos: Point) -> Option<Rect> {
        if !self.can_accept(payload) {
            return None;
        }
        self.tab_rect(self.tab_at_position(pos)?)
    }
}

/// `TabWidget`'s property contract.
///
/// The doc comment here used to record that `closable`, `movable` and `tab_position` were
/// declared by `TAB_WIDGET_PROPERTIES` but answered by nothing, "so they stay exactly as
/// they were (not served)". That was a declaration nothing reads: the schema offered a
/// property the control refused, so a host could neither enable dragging nor learn whether
/// it was on. All three are served now, and `movable` additionally drives the drag gesture
/// in [`TabWidget::handle_event`] rather than merely being stored.
impl WidgetProperties for TabWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tab_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "current_index" => Ok(CapabilityValue::UInt(self.selected_index().unwrap_or(0) as u64)),
            // Symmetric with the writer below: a value that can be set can be read back.
            // Without this arm the property would be write-only, which is the one-directional
            // contract rule #97 forbids.
            "text" | "title" => Ok(CapabilityValue::String(
                self.tabs.first().map(|tab| tab.title.clone()).unwrap_or_default(),
            )),
            "closable" => Ok(CapabilityValue::Bool(self.closable())),
            "movable" => Ok(CapabilityValue::Bool(self.movable())),
            "tab_position" => {
                Ok(CapabilityValue::String(tab_position_token(self.tab_position).to_string()))
            }
            // `set_tab_shape` was a served method with no property arm, so a host could call it
            // directly but could neither read the shape back nor set it through the property
            // contract. Both directions are here, over the same token vocabulary the schema row
            // declares.
            "tab_shape" => Ok(CapabilityValue::String(tab_shape_token(self.tab_shape).to_string())),
            "document_mode" => Ok(CapabilityValue::Bool(self.document_mode())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            // The label route reaches every other control by name, and `create_tab_widget`
            // passes the caller's text through it. Refusing `text`/`title` here meant that
            // text was silently dropped: the factory built a tab widget, the shared label
            // helper called `set("text", ...)`, and this match fell through to
            // `base_property_set` — which has no such arm, so the title never reached a tab.
            // Applying it to the first tab makes the control answer the same property name
            // its constructor's parameter describes.
            "text" | "title" => {
                let text = expect_string(value)?;
                if let Some(first) = self.tabs.first_mut() {
                    first.title = text;
                } else {
                    self.add_tab(text, None);
                }
                self.base.request_redraw();
                Ok(())
            }
            "closable" => {
                self.set_closable(expect_bool(value)?);
                Ok(())
            }
            "movable" => {
                self.set_movable(expect_bool(value)?);
                Ok(())
            }
            // An unknown token is a parse failure, not a different placement: accepting
            // `"North"` and keeping `North` anyway would report success for a write that
            // changed nothing. The accepted spellings are the schema row's.
            "tab_position" => {
                let token = expect_string(value)?;
                let position =
                    TabPosition::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_tab_position(position);
                Ok(())
            }
            "tab_shape" => {
                let token = expect_string(value)?;
                let shape =
                    TabShape::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_tab_shape(shape);
                Ok(())
            }
            "document_mode" => {
                self.set_document_mode(expect_bool(value)?);
                Ok(())
            }
            // Derived from the tab list; the old writer had no arm for it either.
            "tab_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tab_count",
            "current_index",
            "text",
            "title",
            "closable",
            "movable",
            "tab_position",
            "tab_shape",
            "document_mode",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tab_widget` publishes.
    ///
    /// `add_tab` takes the title and the optional page widget and `remove_tab`
    /// takes the index to remove, so neither can complete without a payload: they
    /// are refused as [`CapabilityAccessError::OutOfRange`], meaning the name is
    /// valid and the argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_tab" | "remove_tab" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for TabWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // The move gesture runs before the content forwarding below and reads only the tab
        // strip, so a drag that started on a tab never reaches the page widget — which is
        // what makes the strip behave as one control rather than as a set of drop targets.
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if let Some((back, forward)) = self.overflow_button_rects() {
                    if back.contains_point(*pos) {
                        self.scroll_tabs(-MIN_TAB_WIDTH);
                        return;
                    }
                    if forward.contains_point(*pos) {
                        self.scroll_tabs(MIN_TAB_WIDTH);
                        return;
                    }
                }
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        // Check if the click is on the close button area.
                        //
                        // The rectangle comes from `close_button_rect`, the **same** method
                        // `draw` uses. It used to be written out here with a bare `12` centred on
                        // the tab instead of on the title's line box, so changing the glyph size
                        // moved what was painted and left what was clickable behind — and the two
                        // origins differed by half a line, which put the glyph's outer rows outside
                        // its own hit area.
                        if let Some(tab_rect) = self.tab_rect(index) {
                            let close_rect =
                                self.close_button_rect(tab_rect, effective_font(self.style()));
                            if close_rect.is_some_and(|close_rect| close_rect.contains(*pos)) {
                                self.tab_close_requested.emit(index);
                                return;
                            }
                        }
                        self.set_current_index(index);
                        // The press is reported even when it selected the tab that was already
                        // current: a host reacting to the click itself (open a document, focus an
                        // editor) has no other event to subscribe to, because `current_changed`
                        // only fires on an index change.
                        self.tab_bar_clicked.emit(index);
                        // Selecting on press and *arming* the drag are separate: the
                        // selection has already happened, and only a pointer that travels
                        // past the threshold turns the same gesture into a reorder.
                        self.begin_tab_drag(*pos);
                    }
                }
            }
            // A double click on a tab is a request about that tab (detach it, rename it), which is
            // the host's policy: the control reports the index and changes nothing.
            Event::MouseDoubleClick { pos, button, .. } if *button == 1 => {
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.tab_bar_double_clicked.emit(index);
                    }
                }
            }
            Event::MouseMove { pos } => {
                let hovered = self.tab_at_position(*pos);
                if hovered != self.hovered_tab {
                    self.hovered_tab = hovered;
                    self.base.request_redraw();
                }
                self.update_tab_drag(*pos);
            }
            Event::MouseLeave { .. } => {
                if self.hovered_tab.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } if *key == crate::event::key_codes::TAB => {
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if mods.contains(crate::shortcut::Modifiers::CTRL) && self.tabs.len() > 1 {
                    let forward = !mods.contains(crate::shortcut::Modifiers::SHIFT);
                    let delta = if forward { 1 } else { -1 };
                    // The step skips hidden tabs, so Ctrl+Tab cannot land on a tab the strip does
                    // not offer.
                    if let Some(next) = self.step_visible_index(self.current_index, delta) {
                        self.set_current_index(next);
                    }
                }
            }
            // Arrow keys walk the strip when it holds the keyboard focus. Before this the only
            // keyboard path was Ctrl+Tab, so a focused strip could not be navigated at all — which
            // is the gesture a user reaches for first.
            Event::KeyPress { key, .. }
                if self.base.focus_reason().is_some()
                    && (*key == crate::event::key_codes::LEFT
                        || *key == crate::event::key_codes::RIGHT) =>
            {
                let delta = if *key == crate::event::key_codes::RIGHT { 1 } else { -1 };
                if let Some(next) = self.step_visible_index(self.current_index, delta) {
                    self.set_current_index(next);
                }
                // The strip consumed the key: forwarding it to the page as well would move the
                // page's own selection in the same gesture.
                return;
            }
            Event::MouseRelease { .. } => self.end_tab_drag(),
            _ => {}
        }
        let allow_child_event = match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos }
            // A menu request is positional, like the pointer events above: one issued on
            // the strip is about a tab, so forwarding it to the page would offer the page a
            // menu request it never received the click for.
            | Event::ContextMenu { pos } => self.content_rect().contains(*pos),
            _ => true,
        };
        // Forward content events only to the current widget.
        if allow_child_event {
            if let Some(widget_id) = self.current_widget() {
                if let Some(ref reg) = self.registry {
                    reg.borrow_mut().set_widget_geometry(widget_id, self.content_rect());
                    reg.borrow_mut().forward_event(widget_id, event);
                }
            }
        }
    }
}
impl Draw for TabWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let _rect = self.geometry();
        let content_rect = self.content_rect();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("tab_widget");
        // `tab_widget` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The content area's fill is therefore a step toward
        // the foreground, so the page reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(255, 255, 255));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(200, 200, 200));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let content_background = resolved.blend(&text_color, 0.08);
        // Tab chrome is derived from the resolved pair: an inactive tab is pressed
        // toward the foreground, a disabled one further still, and the current tab
        // stays at the content colour so it reads as connected to its page.
        let inactive_tab = content_background.blend(&text_color, 0.06);
        let disabled_tab = content_background.blend(&text_color, 0.14);
        // Document mode paints one flat row: the strip takes the control's own surface and every
        // tab sits on it, so the row reads as a single band rather than as a set of raised tabs.
        let strip_background = resolved.blend(&text_color, 0.02);
        let document_mode = self.document_mode;
        // A disabled tab's label is muted against the tab's own fill, which is what makes it
        // read as unavailable. This is used for the label; the fill above is the other half
        // of the same signal, so both come from the resolved pair.
        let disabled_text = text_color.blend(&disabled_tab, 0.5);
        // The close affordance is secondary chrome, not a second literal.
        let close_color = text_color.blend(&content_background, 0.4);
        // The accent that marks the current tab in document mode comes from the theme's own accent
        // token rather than a literal, so it follows an appearance switch.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.accent)
            .unwrap_or(Color::PRIMARY);

        // Document mode fills the strip with the control's surface, so the row reads as one band
        // instead of a set of raised tabs. The other modes leave the strip as it was — the window's
        // own fill showing between the tabs.
        if document_mode {
            context.fill_rect(self.tab_strip_rect(), strip_background);
        }

        // Draw content background
        context.fill_rect(content_rect, content_background);
        // Draw content border
        context.draw_rect(content_rect, border);
        // Overflow arrows are chrome: reserved at each end of the strip only when the
        // caller asked for them and the tabs actually overflow.
        if let Some((back, forward)) = self.overflow_button_rects() {
            for button in [back, forward] {
                context.fill_rect(button, content_background);
                context.draw_rect(button, border);
            }
            let glyph = text_color.blend(&content_background, 0.2);
            let (bx, by) = (back.x + back.width as i32 / 2, back.y + back.height as i32 / 2);
            let (fx, fy) =
                (forward.x + forward.width as i32 / 2, forward.y + forward.height as i32 / 2);
            let half = 3;
            match self.tab_position {
                TabPosition::North | TabPosition::South => {
                    context.draw_line_stroke(
                        Point::new(bx + half, by - half),
                        Point::new(bx - half, by),
                        glyph,
                        1,
                    );
                    context.draw_line_stroke(
                        Point::new(bx - half, by),
                        Point::new(bx + half, by + half),
                        glyph,
                        1,
                    );
                    context.draw_line_stroke(
                        Point::new(fx - half, fy - half),
                        Point::new(fx + half, fy),
                        glyph,
                        1,
                    );
                    context.draw_line_stroke(
                        Point::new(fx + half, fy),
                        Point::new(fx - half, fy + half),
                        glyph,
                        1,
                    );
                }
                TabPosition::West | TabPosition::East => {
                    context.draw_line_stroke(
                        Point::new(bx - half, by + half),
                        Point::new(bx, by - half),
                        glyph,
                        1,
                    );
                    context.draw_line_stroke(
                        Point::new(bx, by - half),
                        Point::new(bx + half, by + half),
                        glyph,
                        1,
                    );
                    context.draw_line_stroke(
                        Point::new(fx - half, fy - half),
                        Point::new(fx, fy + half),
                        glyph,
                        1,
                    );
                    context.draw_line_stroke(
                        Point::new(fx, fy + half),
                        Point::new(fx + half, fy - half),
                        glyph,
                        1,
                    );
                }
            }
        }
        // Draw tabs
        //
        // One `tab_rects()` call for the whole strip: asking `tab_rect(i)` in a loop rebuilt the
        // flex layout once per tab (see `tab_rects`).
        for (i, tab_rect) in self.tab_rects().into_iter().enumerate() {
            if i < self.tabs.len() {
                let tab = &self.tabs[i];
                // A hidden tab still holds a slot in the run (so indices line up) but occupies no
                // strip room and is not painted.
                if !tab.visible {
                    continue;
                }
                let is_current = i == self.current_index;
                let is_enabled = tab.enabled;
                // Draw tab background
                let bg_color = if !is_enabled {
                    disabled_tab
                } else if is_current {
                    if document_mode {
                        strip_background
                    } else {
                        content_background
                    }
                } else if document_mode {
                    strip_background
                } else {
                    inactive_tab
                };
                match self.tab_shape {
                    TabShape::Rounded => {
                        let radius = 4;
                        context.fill_rounded_rect(tab_rect, radius, bg_color);
                        // A current tab shares its edge with the content area, so its
                        // outline uses the surface border; a plain tab is outlined more
                        // faintly.
                        let border_color = if !is_enabled || is_current {
                            border
                        } else {
                            border.blend(&content_background, 0.5)
                        };
                        context.draw_rounded_rect_stroke(tab_rect, radius, border_color, 1);
                    }
                    TabShape::Triangular => {
                        // Real triangular tab: apex at the top-center, base along
                        // the bottom edge of the tab strip.
                        let apex_x = tab_rect.x + tab_rect.width as i32 / 2;
                        let base_y = tab_rect.y + tab_rect.height as i32;
                        let points = [
                            Point::new(apex_x, tab_rect.y),
                            Point::new(tab_rect.x, base_y),
                            Point::new(tab_rect.x + tab_rect.width as i32, base_y),
                        ];
                        context.draw_path(&points, true, bg_color, true, 0);
                        let border_color = if !is_enabled || is_current {
                            border
                        } else {
                            border.blend(&content_background, 0.5)
                        };
                        context.draw_path(&points, true, border_color, false, 1);
                    }
                    _ => {
                        context.fill_rect(tab_rect, bg_color);
                        let border_color = if !is_enabled || is_current {
                            border
                        } else {
                            border.blend(&content_background, 0.5)
                        };
                        context.draw_rect(tab_rect, border_color);
                    }
                };
                // Document mode marks the current tab with an accent rule along the strip's inner
                // edge: the tab is not filled with the page colour, so something has to say which
                // one is current.
                if document_mode && is_current && is_enabled {
                    context.fill_rect(self.document_rule_rect(tab_rect), accent);
                }
                // Draw tab text.
                //
                // Centred on both axes and bounded to the tab. The previous form put the glyph
                // origin at the tab's midpoint and asked for `Left`, so a title started at the
                // tab's centre and ran off its right edge, with its top edge on the tab's
                // vertical midpoint. `draw_text_fitted` with `Center` states the intent and
                // elides a title that cannot fit, instead of letting it leave the control.
                let text_color = if !is_enabled { disabled_text } else { text_color };
                let font = effective_font(style);
                let tab_line = context.text_line(tab_rect, font);
                // ── Icon ──
                //
                // A tab carrying an icon reserves a leading square, and the caption band starts
                // after it instead of underneath it. The room charged here is exactly
                // `TAB_ICON_SIDE + TAB_ICON_GAP`, the same allowance `tab_width_hints` added, so
                // what the strip measured and what is painted cannot disagree.
                let mut title_x = tab_rect.x;
                let mut title_width = tab_rect.width.saturating_sub(if self.closable {
                    (CLOSE_SIZE + CLOSE_LABEL_GAP + CLOSE_TITLE_CLEARANCE) as u32
                } else {
                    0
                });
                if tab.has_icon() {
                    let side = TAB_ICON_SIDE.min(tab_line.height.max(1));
                    let icon_rect = Rect {
                        x: tab_rect.x + TAB_ICON_GAP as i32,
                        y: tab_line.y + (tab_line.height.saturating_sub(side) / 2) as i32,
                        width: side,
                        height: side,
                    };
                    if let Some(source) = tab.icon_source {
                        if let Err(error) =
                            crate::widget::paint_icon(context, source.into(), icon_rect, text_color)
                        {
                            log::warn!("tab icon could not be painted: {error:?}");
                        }
                    }
                    #[cfg(feature = "image")]
                    if tab.icon_source.is_none() {
                        if let Some(image) = tab.icon.as_ref() {
                            // `set_icon` normalises to RGBA8, so this borrow is the whole image and
                            // no per-frame transcode can be needed.
                            if let crate::image::ImageData::Rgba8(pixels) = &image.inner.data {
                                // The shared fit rule, so a non-square image is fitted rather than
                                // stretched into the square slot.
                                let fitted = crate::widget::icon_source::fit_image_into(
                                    image.width(),
                                    image.height(),
                                    icon_rect,
                                );
                                context.draw_image(
                                    fitted.x,
                                    fitted.y,
                                    fitted.width,
                                    fitted.height,
                                    pixels,
                                );
                            }
                        }
                    }
                    title_x += (TAB_ICON_SIDE + TAB_ICON_GAP) as i32;
                    title_width = title_width.saturating_sub(TAB_ICON_SIDE + TAB_ICON_GAP);
                }
                let title_band =
                    Rect { x: title_x, y: tab_line.y, width: title_width, height: tab_line.height };
                let title_text = if self.elide_mode == TabElideMode::ElideRight {
                    let mut measure = |candidate: &str| context.measure_text(candidate, font).width;
                    elide_tab_title(&tab.title, title_width, &mut measure)
                } else {
                    Cow::Borrowed(tab.title.as_str())
                };
                context.draw_text_fitted_cow(
                    title_band,
                    &title_text,
                    font,
                    text_color,
                    HorizontalAlignment::Center,
                );
                // Draw the close affordance if one is offered.
                //
                // The rectangle is the **same** one `handle_event` tests against
                // (`close_button_rect`), so what looks clickable is clickable. It used to be
                // derived here from `tab_line.y` while the hit test used `tab_rect.y` — the two
                // ruled the same row and disagreed by half a line.
                if let Some(close_rect) = self.close_button_rect(tab_rect, font) {
                    let close_x = close_rect.x;
                    let close_y = close_rect.y;
                    context.draw_line(
                        Point::new(close_x, close_y),
                        Point::new(close_x + CLOSE_SIZE, close_y + CLOSE_SIZE),
                        close_color,
                    );
                    context.draw_line(
                        Point::new(close_x + CLOSE_SIZE, close_y),
                        Point::new(close_x, close_y + CLOSE_SIZE),
                        close_color,
                    );
                }
            }
        }
        // Draw current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(widget_id, content_rect);
                context.push_clip(
                    content_rect.x,
                    content_rect.y,
                    content_rect.width,
                    content_rect.height,
                );
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_clip();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect, Size};
    use crate::widget::svg::{render_to_svg, render_widget_to_svg};
    use crate::widget::IconName;
    use std::sync::{Arc, Mutex};

    /// A request on the strip is a tab request; the same request on the page is not.
    #[test]
    fn a_context_menu_request_needs_a_tab_under_the_point() {
        let mut tabs = TabWidget::new(Rect::new(0, 0, 300, 200));
        tabs.add_tab("One".to_string(), None);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        tabs.context_menu_requested.connect_ref(move |pos: &Point| {
            sink.lock().unwrap().push(*pos);
        });

        let on_tab = Point::new(5, 5);
        assert!(tabs.context_menu_event(on_tab));
        // Below the strip is the content page: not a tab request.
        assert!(!tabs.context_menu_event(Point::new(150, 150)));
        assert_eq!(*seen.lock().unwrap(), vec![on_tab]);
    }

    /// Helper to create unique test ObjectIds.
    fn wid1() -> ObjectId {
        1001
    }
    fn wid2() -> ObjectId {
        1002
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────

    /// A crowded strip's tabs get **exactly** their declared share, with nothing shaved off.
    ///
    /// # The defect this pins (BLUE22 · G-1)
    ///
    /// The strip was sized at `sum(tab widths)` while the layout also charges `TAB_SPACING` for
    /// every adjacent pair, so the band the run was given was short by that total. `remaining` went
    /// negative and the proportional shrink pass took the difference off the tabs — one pixel each,
    /// which is why a 64 px share was drawn as 63. Nothing was *wrong* in either half; the band and
    /// the run simply disagreed about whether the gaps were part of the run's extent.
    ///
    /// The assertion is the whole share, per tab, because "the shortfall is distributed" is exactly
    /// what a 1 px loss looks like and exactly what must not happen here.
    #[test]
    fn tabwidget_a_crowded_strip_gives_every_tab_its_full_share() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 100, 120));
        tw.add_tab("Tab 1".to_string(), None);
        tw.add_tab("Tab 2".to_string(), None);

        let hints = tw.tab_width_hints();
        assert_eq!(hints.len(), 2, "two tabs, two declarations");
        // Crowded: the shares must be equal and come from the overflow rule.
        let (min, share, max) = hints[0];
        assert_eq!(hints[1], (min, share, max), "an overflowing strip shares equally");
        assert!(min < share, "a crowded tab may be squeezed below its share: {hints:?}");

        for index in 0..2 {
            let rect = tw.tab_rect(index).expect("the tab has a band");
            assert_eq!(
                rect.width as i32, share,
                "tab {index} must be drawn at its declared share, not one pixel narrower"
            );
        }
        // And the two boxes plus the gap are the strip exactly, so nothing was deferred either.
        let first = tw.tab_rect(0).expect("tab 0");
        let second = tw.tab_rect(1).expect("tab 1");
        assert_eq!(
            second.x,
            first.x + first.width as i32 + scaled_tab_spacing(),
            "the second tab begins one gap after the first"
        );
        assert!(
            second.x + second.width as i32 <= tw.geometry().width as i32,
            "the run fits inside the control"
        );
    }

    /// A strip that fits is untouched by the overflow rule: the three numbers agree and the run is
    /// exactly the measurement.
    ///
    /// The companion to the test above — the G-1 fix must not make a comfortable strip behave as a
    /// crowded one, which is why the fitted case returns `min == pref == max`.
    #[test]
    fn tabwidget_a_strip_that_fits_declares_each_tab_as_fixed() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 600, 120));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        for (index, (min, pref, max)) in tw.tab_width_hints().into_iter().enumerate() {
            assert_eq!((min, max), (pref, pref), "tab {index} must be fixed when the strip fits");
            let rect = tw.tab_rect(index).expect("the tab has a band");
            assert_eq!(rect.width as i32, pref, "tab {index} draws at its measured width");
        }
    }

    #[test]
    fn tabwidget_creation_defaults() {
        let tw = TabWidget::new(Rect::new(10, 20, 400, 300));
        assert_eq!(tw.kind(), WidgetKind::TabWidget);
        assert_eq!(tw.geometry(), Rect::new(10, 20, 400, 300));
        assert_eq!(tw.count(), 0);
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
        assert!(tw.current_widget().is_none());
        assert_eq!(tw.tab_position(), TabPosition::North);
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
        assert!(!tw.closable());
        assert!(!tw.movable());
        assert!(tw.is_visible());
        assert!(tw.is_enabled());
        assert!(tw.children().is_empty());
        assert!(tw.registry().is_none());
    }

    // ── 2. Adding tabs ────────────────────────────────────────────────────────

    #[test]
    fn tabwidget_add_tab_returns_index_and_increments_count() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        let idx0 = tw.add_tab("Tab A".to_string(), None);
        assert_eq!(idx0, 0);
        assert_eq!(tw.count(), 1);

        let idx1 = tw.add_tab("Tab B".to_string(), Some(wid1()));
        assert_eq!(idx1, 1);
        assert_eq!(tw.count(), 2);

        let idx2 = tw.add_tab("Tab C".to_string(), Some(wid2()));
        assert_eq!(idx2, 2);
        assert_eq!(tw.count(), 3);
    }

    #[test]
    fn tabwidget_add_tab_with_widget_updates_children() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("With Widget".to_string(), Some(wid1()));
        let children = tw.children();
        assert_eq!(children.len(), 1);
        assert!(children.contains(&wid1()));
    }

    #[test]
    fn tabwidget_add_tab_without_widget_does_not_add_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("No Widget".to_string(), None);
        assert!(tw.children().is_empty());
    }

    // ── 3. Inserting tabs at specific index ───────────────────────────────────

    #[test]
    fn tabwidget_insert_tab_at_front_shifts_indices() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.insert_tab(0, "Inserted".to_string(), None);
        assert_eq!(tw.count(), 3);
        assert_eq!(tw.tab_text(0), Some("Inserted"));
        assert_eq!(tw.tab_text(1), Some("A"));
        assert_eq!(tw.tab_text(2), Some("B"));
    }

    #[test]
    fn tabwidget_insert_tab_at_end_acts_like_add() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.insert_tab(1, "B".to_string(), None);
        assert_eq!(tw.count(), 2);
        assert_eq!(tw.tab_text(1), Some("B"));
    }

    #[test]
    fn tabwidget_insert_tab_shifts_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.set_current_index(1);
        tw.insert_tab(0, "X".to_string(), None);
        // current_index was 1, now should be 2 because a tab was inserted before it
        assert_eq!(tw.selected_index().unwrap_or(0), 2);
    }

    #[test]
    fn tabwidget_insert_tab_with_widget_adds_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.insert_tab(0, "X".to_string(), Some(wid1()));
        let children = tw.children();
        assert_eq!(children.len(), 1);
        assert!(children.contains(&wid1()));
    }

    /// An index past the end appends instead of panicking.
    ///
    /// `insert_tab` passed the caller's index straight to `Vec::insert`, which panics
    /// when it exceeds the length. `tab_bar` and `pie_menu` clamp, so a caller that
    /// asked to insert past the end crashed here and appended everywhere else.
    #[test]
    fn tabwidget_insert_past_the_end_appends() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.insert_tab(9, "C".to_string(), None);
        assert_eq!(tw.count(), 3);
        assert_eq!(tw.tab(2).map(|t| t.title.as_str()), Some("C"), "past the end lands last");
    }

    // ── 4. Removing tabs ─────────────────────────────────────────────────────

    #[test]
    fn tabwidget_remove_tab_reduces_count() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        assert_eq!(tw.count(), 3);

        tw.remove_tab(1);
        assert_eq!(tw.count(), 2);
        assert_eq!(tw.tab_text(0), Some("A"));
        assert_eq!(tw.tab_text(1), Some("C"));
    }

    #[test]
    fn tabwidget_remove_tab_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.remove_tab(5); // out of bounds
        assert_eq!(tw.count(), 1);
    }

    #[test]
    fn tabwidget_remove_last_tab_resets_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 0);
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn tabwidget_remove_tab_adjusts_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        tw.set_current_index(2);
        tw.remove_tab(2);
        assert_eq!(tw.selected_index().unwrap_or(0), 1);
    }

    #[test]
    fn tabwidget_remove_tab_with_widget_removes_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), Some(wid1()));
        tw.add_tab("B".to_string(), Some(wid2()));
        assert_eq!(tw.children().len(), 2);
        tw.remove_tab(0);
        assert_eq!(tw.children().len(), 1);
        assert!(!tw.children().contains(&wid1()));
    }

    // ── 5. Getting / setting current index ────────────────────────────────────

    #[test]
    fn tabwidget_set_current_index_normal() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), Some(wid1()));
        tw.add_tab("B".to_string(), Some(wid2()));
        tw.set_current_index(1);
        assert_eq!(tw.selected_index().unwrap_or(0), 1);
        assert_eq!(tw.current_widget(), Some(wid2()));
    }

    #[test]
    fn tabwidget_set_current_index_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.set_current_index(10);
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn tabwidget_set_current_index_same_value_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.set_current_index(0); // already 0
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn tabwidget_current_widget_on_empty_returns_none() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.current_widget(), None);
    }

    #[test]
    fn tabwidget_set_current_index_round_trip() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        tw.set_current_index(2);
        assert_eq!(tw.selected_index().unwrap_or(0), 2);
        tw.set_current_index(0);
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
        tw.set_current_index(1);
        assert_eq!(tw.selected_index().unwrap_or(0), 1);
    }

    // ── 6. Tab count after operations ─────────────────────────────────────────

    #[test]
    fn tabwidget_count_after_mixed_operations() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.count(), 0);
        tw.add_tab("A".to_string(), None);
        assert_eq!(tw.count(), 1);
        tw.add_tab("B".to_string(), None);
        assert_eq!(tw.count(), 2);
        tw.insert_tab(1, "C".to_string(), None);
        assert_eq!(tw.count(), 3);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 2);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 1);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 0);
    }

    // ── 7. Setting tab text and tooltip ───────────────────────────────────────

    #[test]
    fn tabwidget_set_tab_text_updates_and_retrieves() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Original".to_string(), None);
        assert_eq!(tw.tab_text(0), Some("Original"));
        tw.set_tab_text(0, "Updated".to_string());
        assert_eq!(tw.tab_text(0), Some("Updated"));
    }

    #[test]
    fn tabwidget_set_tab_text_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_text(0, "Nope".to_string());
        // No panic, no change
        assert_eq!(tw.tab_text(0), None);
    }

    #[test]
    fn tabwidget_tab_text_returns_none_for_invalid_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Only".to_string(), None);
        assert_eq!(tw.tab_text(1), None);
        assert_eq!(tw.tab_text(usize::MAX), None);
    }

    #[test]
    fn tabwidget_tab_tooltip_set_and_get() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        let tab = tw.tab_mut(0).unwrap();
        assert_eq!(tab.tooltip(), "");
        tab.set_tooltip("Helpful hint".to_string());
        assert_eq!(tab.tooltip(), "Helpful hint");
        let tab_ref = tw.tab(0).unwrap();
        assert_eq!(tab_ref.tooltip(), "Helpful hint");
    }

    // ── 8. Enabling / disabling tabs ──────────────────────────────────────────

    #[test]
    fn tabwidget_tab_enabled_default() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        assert!(tw.tab(0).unwrap().is_enabled());
    }

    #[test]
    fn tabwidget_disable_tab_via_tab_mut() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        let tab = tw.tab_mut(0).unwrap();
        tab.set_enabled(false);
        assert!(!tw.tab(0).unwrap().is_enabled());
        // Re-enable
        let tab = tw.tab_mut(0).unwrap();
        tab.set_enabled(true);
        assert!(tw.tab(0).unwrap().is_enabled());
    }

    #[test]
    fn tabwidget_tab_title_via_tab_api() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Hello".to_string(), None);
        assert_eq!(tw.tab(0).unwrap().title(), "Hello");
        tw.tab_mut(0).unwrap().set_title("World".to_string());
        assert_eq!(tw.tab(0).unwrap().title(), "World");
    }

    // ── 9. Tab position and shape configuration ───────────────────────────────

    #[test]
    fn tabwidget_tab_position_default_is_north() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.tab_position(), TabPosition::North);
    }

    #[test]
    fn tabwidget_set_tab_position_cycle() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_position(TabPosition::South);
        assert_eq!(tw.tab_position(), TabPosition::South);
        tw.set_tab_position(TabPosition::West);
        assert_eq!(tw.tab_position(), TabPosition::West);
        tw.set_tab_position(TabPosition::East);
        assert_eq!(tw.tab_position(), TabPosition::East);
        tw.set_tab_position(TabPosition::North);
        assert_eq!(tw.tab_position(), TabPosition::North);
    }

    #[test]
    fn tabwidget_tab_shape_default_is_rounded() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
    }

    #[test]
    fn tabwidget_set_tab_shape() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_shape(TabShape::Triangular);
        assert_eq!(tw.tab_shape(), TabShape::Triangular);
        tw.set_tab_shape(TabShape::Rectangular);
        assert_eq!(tw.tab_shape(), TabShape::Rectangular);
        tw.set_tab_shape(TabShape::Rounded);
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
    }

    /// The close affordance is one rectangle, offered to both the painter and the hit test.
    ///
    /// # The defect this pins
    ///
    /// `draw` and `handle_event` each derived this rectangle themselves, and they disagreed:
    /// the paint path used [`CLOSE_SIZE`] and centred on the title's **line box**; the hit test
    /// used a bare `12` and centred on the whole **tab**. Two consequences, both a few pixels and
    /// therefore easy to miss by eye:
    ///
    /// * the literal was a second copy of `CLOSE_SIZE`, so resizing the glyph would move what is
    ///   drawn and leave what is clickable behind — the classic two-derivations drift;
    /// * the origins differ by half a line, so the glyph's outer rows were outside its own hit area
    ///   while a band of tab *above* the glyph accepted clicks and closed the tab.
    ///
    /// # Why the assertions are about *both* halves
    //
    /// Asserting only that the rectangle exists would pass against the duplicated code, since both
    /// copies produced *a* rectangle. The property that was missing is that they are the **same**
    /// rectangle, so the check is: the drawn affordance's box, as the pointer path sees it, must (a)
    /// be inside the tab, (b) be hit by a press at its own centre, and (c) not be hit by a press
    /// just above it — the region the old hit rect wrongly claimed.
    #[test]
    fn the_close_affordance_is_the_same_rect_for_painting_and_hit_testing() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("One".to_string(), None);
        tw.set_closable(true);
        let tab_rect = tw.tab_rect(0).expect("the tab is placed");
        let font = Font::default();
        let close = tw.close_button_rect(tab_rect, &font).expect("the tab is closable");

        assert!(
            close.x >= tab_rect.x
                && close.x + close.width as i32 <= tab_rect.x + tab_rect.width as i32,
            "the affordance must sit inside its tab: {close:?} in {tab_rect:?}"
        );
        assert_eq!(
            (close.width, close.height),
            (CLOSE_SIZE as u32, CLOSE_SIZE as u32),
            "and be exactly one glyph, so the constant is the only size there is"
        );

        // The centre is inside — the case both old copies got right.
        let centre = Point::new(close.x + CLOSE_SIZE / 2, close.y + CLOSE_SIZE / 2);
        assert!(close.contains(centre), "the glyph's own centre must be clickable: {centre:?}");

        // And the row the *old* hit rect wrongly claimed, just above the glyph, must not be. The
        // two derivations agreed on this control's nominal metrics *by coincidence* — a 24 px tab
        // and a 14 px line box happen to give the same origin — so the discriminating assertion is
        // made against a **device-scaled font**, where they genuinely diverge. Deriving the
        // affordance from the line box is what makes it track the text it sits beside; deriving it
        // from the tab's own height is what leaves it behind when the text grows.
        let scaled_font = Font::simple("sans-serif", Font::default().size() * 2.0);
        let scaled_close = tw.close_button_rect(tab_rect, &scaled_font).expect("still closable");
        assert_ne!(
            scaled_close.y, close.y,
            "a larger font moves the line box, so it must move the affordance — a hit rect \
             centred on the tab's own height would not budge: {scaled_close:?} vs {close:?}"
        );
        // The direction follows the line box, not a guess: `line_top` is the line box centred in
        // the tab, floored at the tab's top edge, and the glyph is then centred *on the line*. So a
        // line box taller than the tab pins the glyph to the tab's bottom, while a short one
        // centres it. Asserting the relation rather than a direction is what keeps this test
        // measuring the derivation instead of restating it.
        let scaled_line = crate::widget::metrics::estimate_line_height(&scaled_font, 1.0) as i32;
        let expected_line_top = tab_rect.y + ((tab_rect.height as i32 - scaled_line) / 2).max(0);
        assert_eq!(
            scaled_close.y,
            expected_line_top + (scaled_line - CLOSE_SIZE) / 2,
            "the affordance is centred on the line box, not on the tab"
        );

        // A control that does not offer the affordance offers no rectangle to either path.
        tw.set_closable(false);
        assert!(tw.close_button_rect(tab_rect, &font).is_none());
    }

    #[test]
    fn tabwidget_closable_movable_default_false() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(!tw.closable());
        assert!(!tw.movable());
        tw.set_closable(true);
        tw.set_movable(true);
        assert!(tw.closable());
        assert!(tw.movable());
        tw.set_closable(false);
        tw.set_movable(false);
        assert!(!tw.closable());
        assert!(!tw.movable());
    }

    // ── 10. Min / max size via Widget trait ───────────────────────────────────

    #[test]
    fn tabwidget_min_size_default_none() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.min_size(), None);
        assert_eq!(tw.max_size(), None);
    }

    #[test]
    fn tabwidget_set_min_max_size() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_min_size(Some(Size::new(100, 80)));
        tw.set_max_size(Some(Size::new(800, 600)));
        assert_eq!(tw.min_size(), Some(Size::new(100, 80)));
        assert_eq!(tw.max_size(), Some(Size::new(800, 600)));
    }

    // ── 11. Signal accessors ──────────────────────────────────────────────────

    #[test]
    fn tabwidget_current_changed_signal_emits_on_set_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        let emitted = Arc::new(std::sync::Mutex::new(None));
        tw.current_changed.connect({
            let emitted = Arc::clone(&emitted);
            move |idx: Arc<usize>| {
                *emitted.lock().unwrap() = Some(*idx);
            }
        });

        tw.set_current_index(1);
        assert_eq!(*emitted.lock().unwrap(), Some(1));
    }

    #[test]
    fn tabwidget_current_changed_not_emitted_for_same_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        tw.current_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<usize>| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        tw.set_current_index(0); // same as default
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tabwidget_current_changed_not_emitted_for_out_of_bounds() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);

        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        tw.current_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<usize>| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        tw.set_current_index(5); // out of bounds
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tabwidget_tab_close_requested_signal_accessible() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        let emitted = Arc::new(std::sync::Mutex::new(None));
        tw.tab_close_requested.connect({
            let emitted = Arc::clone(&emitted);
            move |idx: Arc<usize>| {
                *emitted.lock().unwrap() = Some(*idx);
            }
        });
        // Manually emit — tabwidget doesn't auto-close, but the signal is public
        tw.tab_close_requested.emit(1usize);
        assert_eq!(*emitted.lock().unwrap(), Some(1));
    }

    // ── 12. Geometry delegation ───────────────────────────────────────────────

    #[test]
    fn tabwidget_geometry_via_widget_trait() {
        let tw = TabWidget::new(Rect::new(5, 10, 200, 150));
        assert_eq!(tw.geometry(), Rect::new(5, 10, 200, 150));
        assert_eq!(tw.geometry(), Rect::new(5, 10, 200, 150));
    }

    #[test]
    fn tabwidget_set_geometry_via_widget_trait() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        tw.set_geometry(Rect::new(20, 30, 500, 400));
        assert_eq!(tw.geometry(), Rect::new(20, 30, 500, 400));
        assert_eq!(tw.position(), Point::new(20, 30));
        assert_eq!(tw.size(), Size::new(500, 400));
    }

    #[test]
    fn tabwidget_set_position_and_size() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 200, 100));
        tw.set_position(Point::new(50, 60));
        assert_eq!(tw.position(), Point::new(50, 60));
        assert_eq!(tw.size(), Size::new(200, 100));

        tw.set_size(Size::new(300, 150));
        assert_eq!(tw.position(), Point::new(50, 60));
        assert_eq!(tw.size(), Size::new(300, 150));
    }

    // ── 13. Widget ID and kind ────────────────────────────────────────────────

    #[test]
    fn tabwidget_kind_is_tabwidget() {
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.kind(), WidgetKind::TabWidget);
    }

    #[test]
    fn tabwidget_id_is_unique() {
        let tw1 = TabWidget::new(Rect::new(0, 0, 100, 100));
        let tw2 = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_ne!(tw1.id(), tw2.id());
    }

    #[test]
    fn tabwidget_accessible_name_falls_back_to_kind() {
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.accessible_name(), "TabWidget");
    }

    #[test]
    fn tabwidget_accessible_role_is_kind() {
        use crate::platform::accessibility::AccessibleRole;
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.accessible_role(), AccessibleRole::TabGroup);
    }

    // ── 14. Disabled state blocks events ──────────────────────────────────────

    #[test]
    fn tabwidget_disabled_does_not_switch_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);
        // Disable the entire widget
        tw.set_enabled(false);

        // Click on tab 1's position
        // tab_rect(0) with North position at Rect(0,0,300,200) starts at x=0, y=0
        let event = Event::mouse_press(0, 0, 1);
        tw.handle_event(&event);

        // current_index should still be 0 because the widget is disabled
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn tabwidget_enabled_switches_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // The second tab's position comes from the control's own layout, not from a literal:
        // tab widths are measured from their titles, so a hardcoded x would pin this test to one
        // font's metrics and break every time a title changed. Reading `tab_rect` is also the
        // stronger assertion — it checks that a click lands on the tab the control believes is
        // there, which is the property that matters.
        let second = tw.tab_rect(1).expect("two tabs were added, so index 1 has a rectangle");
        let click_x = second.x + second.width as i32 / 2;
        let event = Event::mouse_press(click_x, second.y + 2, 1);
        tw.handle_event(&event);

        assert_eq!(tw.selected_index().unwrap_or(0), 1);
    }

    #[test]
    fn tabwidget_disabled_tab_does_not_switch_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        // Disable tab at index 1
        tw.tab_mut(1).unwrap().set_enabled(false);

        tw.set_current_index(0);

        // Click on tab 1's position
        let event = Event::mouse_press(102, 0, 1);
        tw.handle_event(&event);

        // Should NOT switch because tab 1 is disabled
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn tabwidget_right_click_does_not_switch() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // Right-click on tab 1's position
        let event = Event::mouse_press(102, 0, 3);
        tw.handle_event(&event);

        // Should NOT switch because only button 1 (left click) triggers switch
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn tabwidget_click_outside_tabs_does_not_change_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // Click far to the right of any tab
        let event = Event::mouse_press(500, 0, 1);
        tw.handle_event(&event);

        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    // ── 15. SVG output verification ───────────────────────────────────────────

    #[test]
    fn tabwidget_svg_output_via_render_to_svg() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Home".to_string(), None);
        tw.add_tab("Settings".to_string(), None);

        let svg = render_to_svg(&mut tw);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
        assert!(svg.contains("width=\"300\""), "SVG should contain width=\"300\"");
        assert!(svg.contains("height=\"200\""), "SVG should contain height=\"200\"");
        // Both tab titles are drawn. They are glyph geometry, not `<text>` elements, so the titles
        // are checked as ink on the **tab strip**: the strip is the control's top `TAB_HEIGHT`
        // rows, and one label per tab means one ink box per tab, each inside its own tab rather
        // than a single run spanning the strip.
        //
        // # Why the boxes come from the crate reader, not a local `d` parser
        //
        // This used to split `d` on `M` and read the four integers of `M{x} {y}h{w}v{h}` — the
        // **bitmap** path's grammar and only its grammar. An outline face writes
        // `M17.00 23.84L...Z` with fractional vertices, which that reader took as a rectangle at
        // `(17, 23)` sized by the next two numbers: geometry that is not ink and not where the
        // glyph is, so both titles were located off the strip and the assertion below failed on a
        // correctly drawn control. `text_ink_boxes` knows both grammars and the `data-text`
        // provenance tag; one reader, so the two cannot drift apart.
        let title_boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert_eq!(title_boxes.len(), 2, "one label per tab: {title_boxes:?}");
        for (index, title) in title_boxes.iter().enumerate() {
            assert!(
                (0..TAB_HEIGHT + 2).contains(&title.1) && title.3 <= TAB_HEIGHT + 2,
                "tab {index}'s title must sit on the tab strip, got {title:?}"
            );
            assert!(title.2 > title.0, "tab {index}'s title laid down ink: {title:?}");
        }
        // The two tabs are laid side by side, so the second label starts to the right of the
        // first: a widget that drew both titles at the same x would overlap them.
        assert!(
            title_boxes[1].0 > title_boxes[0].2,
            "the second tab's title follows the first: {title_boxes:?}"
        );
        // Should contain fill and stroke attributes from the rendering
        assert!(svg.contains("fill=") || svg.contains("stroke="));
    }

    #[test]
    fn tabwidget_svg_output_with_explicit_geometry() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 120, 80));
        tw.add_tab("X".to_string(), None);

        let svg = render_widget_to_svg(&mut tw, Rect::new(0, 0, 120, 80));
        assert!(svg.contains("width=\"120\""));
        assert!(svg.contains("height=\"80\""));
    }

    // ── 16. Tab accessor (tab / tab_mut) ──────────────────────────────────────

    #[test]
    fn tabwidget_tab_accessor_out_of_bounds() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(tw.tab(0).is_none());
        assert!(tw.tab_mut(0).is_none());
    }

    #[test]
    fn tabwidget_tab_mut_allows_widget_assignment() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        tw.tab_mut(0).unwrap().set_widget(Some(wid1()));
        // Widget assigned to tab 0 is also the current widget (index 0)
        assert_eq!(tw.tab(0).unwrap().widget(), Some(wid1()));
        assert_eq!(tw.current_widget(), Some(wid1()));
    }

    // ── 17. Registry round-trip ───────────────────────────────────────────────

    #[test]
    fn tabwidget_registry_set_and_get() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(tw.registry().is_none());
        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        tw.set_registry(reg.clone());
        assert!(tw.registry().is_some());
        // Verify pointer identity
        assert!(Rc::ptr_eq(&reg, tw.registry().unwrap()));
    }

    // ── 18. Tab icon set/get ───────────────────────────────────────────────────

    #[cfg(feature = "image")]
    #[test]
    fn tabwidget_tab_icon_default_none() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        assert!(tw.tab(0).unwrap().icon().is_none());
    }

    // ── 18b. Typed tab icons and their draw ────────────────────────────────────

    #[test]
    fn tabwidget_set_item_icon_source_reports_out_of_bounds() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);

        assert_eq!(
            tw.set_item_icon_source(3, Some(IconSource::Builtin(IconName::Info))),
            Err(crate::widget::IconItemError::IndexOutOfBounds { index: 3, len: 1 })
        );
        assert!(tw.set_item_icon_source(0, Some(IconSource::Builtin(IconName::Info))).is_ok());
        assert_eq!(tw.tab(0).unwrap().icon_source(), Some(IconSource::Builtin(IconName::Info)));
    }

    #[cfg(feature = "image")]
    #[test]
    fn tabwidget_typed_and_legacy_icon_share_one_active_state() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);

        tw.tab_mut(0).unwrap().set_icon(Some(
            Image::try_from_rgba(vec![9; 4], 1, 1).expect("the buffer matches its declared size"),
        ));
        assert!(tw.tab(0).unwrap().has_icon());
        assert!(tw.tab(0).unwrap().icon().is_some());
        assert!(tw.tab(0).unwrap().icon_source().is_none());

        // The typed setter is the more recent assignment, so the legacy image must not survive it.
        tw.set_item_icon_source(0, Some(IconSource::Builtin(IconName::Save))).unwrap();
        assert!(tw.tab(0).unwrap().icon().is_none());
        assert_eq!(tw.tab(0).unwrap().icon_source(), Some(IconSource::Builtin(IconName::Save)));

        // ... and the legacy setter replaces the typed source in turn.
        tw.tab_mut(0).unwrap().set_icon(Some(
            Image::try_from_rgba(vec![0; 4], 1, 1).expect("the buffer matches its declared size"),
        ));
        assert!(tw.tab(0).unwrap().icon_source().is_none());

        // An explicit clear removes both forms, so nothing can reappear afterwards.
        tw.tab_mut(0).unwrap().clear_icon_source();
        assert!(!tw.tab(0).unwrap().has_icon());
        assert!(tw.tab(0).unwrap().icon().is_none());
    }

    /// The count of `<path` elements, for the "the icon is actually drawn" assertion.
    fn path_count(svg: &str) -> usize {
        svg.matches("<path").count()
    }

    #[test]
    fn tabwidget_typed_icon_adds_geometry_to_the_drawn_tab() {
        let mut plain = TabWidget::new(Rect::new(0, 0, 300, 200));
        plain.add_tab("Tab".to_string(), None);
        let without = render_widget_to_svg(&mut plain, Rect::new(0, 0, 300, 200));

        let mut with_icon = TabWidget::new(Rect::new(0, 0, 300, 200));
        with_icon.add_tab("Tab".to_string(), None);
        with_icon.set_item_icon_source(0, Some(IconSource::Builtin(IconName::Info))).unwrap();
        let with = render_widget_to_svg(&mut with_icon, Rect::new(0, 0, 300, 200));

        // The compound geometry is one path, and the caption is identical in both (the band stays
        // the same width), so the drawn difference is exactly the icon.
        assert_eq!(
            path_count(&with),
            path_count(&without) + 1,
            "a typed tab icon must reach the drawing backend exactly once"
        );
    }

    #[cfg(feature = "image")]
    #[test]
    fn tabwidget_legacy_image_icon_reaches_the_drawing_backend() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        let without = render_widget_to_svg(&mut tw, Rect::new(0, 0, 300, 200));
        assert!(!without.contains("<image"), "an iconless tab draws no image");

        tw.tab_mut(0).unwrap().set_icon(Some(
            Image::try_from_rgba(vec![200; 4 * 4 * 4], 4, 4)
                .expect("the buffer matches its declared size"),
        ));
        let with = render_widget_to_svg(&mut tw, Rect::new(0, 0, 300, 200));
        assert!(with.contains("<image"), "a legacy tab image must reach the backend");
    }

    // ── 19. Movable tabs (drag to reorder) ─────────────────────────────────────

    /// Builds a strip of `count` tabs with wide, well-separated bands.
    fn movable_strip(count: usize) -> TabWidget {
        let mut tw = TabWidget::new(Rect::new(0, 0, 600, 200));
        for i in 0..count {
            tw.add_tab(format!("T{i}"), None);
        }
        tw.set_movable(true);
        tw
    }

    fn titles(tw: &TabWidget) -> Vec<String> {
        (0..tw.count()).map(|i| tw.tab_text(i).unwrap().to_string()).collect()
    }

    #[test]
    fn tabwidget_move_tab_reorders_and_emits() {
        let mut tw = movable_strip(3);
        let captured = Arc::new(Mutex::new(None::<(usize, usize)>));
        tw.tab_moved.connect({
            let captured = Arc::clone(&captured);
            move |value: Arc<(usize, usize)>| {
                *captured.lock().unwrap() = Some(*value);
            }
        });

        assert!(tw.move_tab(0, 2));
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
        assert_eq!(*captured.lock().unwrap(), Some((0, 2)));

        // A move to the same index is not a move, so nothing is emitted.
        assert!(!tw.move_tab(1, 1));
        assert_eq!(*captured.lock().unwrap(), Some((0, 2)));

        // An out-of-range source is a caller error and does not reorder or emit.
        assert!(!tw.move_tab(99, 0));
        assert_eq!(*captured.lock().unwrap(), Some((0, 2)));
    }

    #[test]
    fn tabwidget_move_tab_clamps_the_target_to_the_last_tab() {
        let mut tw = movable_strip(3);
        // "Dropped past the end" is "moved to the end", not a refusal.
        assert!(tw.move_tab(0, 99));
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
    }

    #[test]
    fn tabwidget_move_tab_keeps_the_selection_with_its_page() {
        let mut tw = movable_strip(3);
        tw.set_current_index(2);
        // Dragging the current tab must not leave the selection on the slot it vacated.
        assert!(tw.move_tab(2, 0));
        assert_eq!(titles(&tw), vec!["T2", "T0", "T1"]);
        assert_eq!(tw.selected_index().unwrap_or(0), 0);

        // And dragging another tab across the selection shifts it by one slot.
        tw.set_current_index(0); // the dragged tab
        assert!(tw.move_tab(2, 0));
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
        assert_eq!(tw.selected_index().unwrap_or(0), 1);
    }

    /// A press on a tab of a *movable* strip that then travels reorders it.
    #[test]
    fn tabwidget_drag_reorders_a_movable_tab() {
        let mut tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let press = Point::new(first.x + first.width as i32 / 2, first.y + 5);

        tw.handle_event(&Event::MousePress { pos: press, button: 1, modifiers: 0 });
        // A press alone must not reorder: the gesture is still a click at this point.
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);

        // Travel past the drag threshold and onto the third tab.
        tw.handle_event(&Event::MouseMove {
            pos: Point::new(third.x + third.width as i32 / 2, press.y),
        });
        assert_eq!(
            titles(&tw),
            vec!["T1", "T2", "T0"],
            "crossing two neighbours must carry the tab to the far end"
        );

        tw.handle_event(&Event::MouseRelease {
            pos: Point::new(third.x + third.width as i32 / 2, press.y),
            button: 1,
        });
        // The release settles the gesture; the live reorder already happened.
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
    }

    /// The whole point of the flag: with it off, the very same gesture must not reorder.
    #[test]
    fn tabwidget_drag_does_nothing_when_not_movable() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 600, 200));
        for i in 0..3 {
            tw.add_tab(format!("T{i}"), None);
        }
        assert!(!tw.movable());

        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let y = first.y + 5;
        tw.handle_event(&Event::MousePress {
            pos: Point::new(first.x + first.width as i32 / 2, y),
            button: 1,
            modifiers: 0,
        });
        tw.handle_event(&Event::MouseMove { pos: Point::new(third.x + third.width as i32 / 2, y) });
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);
        // The press still selected, which is the behaviour a non-movable strip keeps.
        assert_eq!(tw.selected_index().unwrap_or(0), 0);
    }

    /// A press-and-jitter must stay a click rather than a reorder.
    #[test]
    fn tabwidget_a_jitter_below_the_threshold_does_not_reorder() {
        let mut tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let press = Point::new(first.x + first.width as i32 / 2, first.y + 5);
        tw.handle_event(&Event::MousePress { pos: press, button: 1, modifiers: 0 });
        tw.handle_event(&Event::MouseMove { pos: Point::new(press.x + 1, press.y) });
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);
    }

    /// Turning `movable` off mid-drag must abandon the gesture, not finish it.
    #[test]
    fn tabwidget_set_movable_false_cancels_a_live_drag() {
        let mut tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let y = first.y + 5;
        tw.handle_event(&Event::MousePress {
            pos: Point::new(first.x + first.width as i32 / 2, y),
            button: 1,
            modifiers: 0,
        });
        tw.handle_event(&Event::MouseMove {
            pos: Point::new(first.x + first.width as i32 / 2 + 10, y),
        });
        tw.set_movable(false);
        tw.handle_event(&Event::MouseMove { pos: Point::new(third.x + third.width as i32 / 2, y) });
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);
    }

    /// Unmounts a widget registered by a test, even on panic.
    #[cfg(not(alloc_frugal))]
    struct MountGuard(crate::core::ObjectId);

    #[cfg(not(alloc_frugal))]
    impl Drop for MountGuard {
        fn drop(&mut self) {
            crate::widget::runtime::unregister(self.0);
        }
    }

    /// A coordinator-only press, move and release reorders the strip and reports `Move` to
    /// the source, without the pointer handler ever seeing the gesture.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn tabwidget_coordinator_reorders_a_dragged_tab() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};
        use std::sync::{Arc, Mutex};

        let tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let press = Point::new(first.x + first.width as i32 / 2, first.y + 5);
        let drop = Point::new(third.x + third.width as i32 / 2, third.y + 5);

        let finished = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&finished);
        tw.drag_finished.connect_ref(move |effect: &DropEffect| {
            *sink.lock().unwrap() = Some(*effect);
        });
        let id = register(crate::compat::Box::new(tw)).expect("mount the strip");
        let _guard = MountGuard(id);

        let viewport = Rect::new(0, 0, 800, 300);
        let mut coordinator = DragCoordinator::new();
        assert_eq!(
            coordinator.on_event(&Event::mouse_press(press.x, press.y, PRIMARY), viewport),
            None
        );
        assert_eq!(coordinator.on_event(&Event::mouse_move(drop.x, drop.y), viewport), None);
        assert_eq!(
            coordinator.on_event(&Event::mouse_release(drop.x, drop.y, PRIMARY), viewport),
            Some(DropEffect::Move)
        );

        let observed = with_widget_mut(id, |widget| {
            let tw = (widget as &mut dyn core::any::Any)
                .downcast_mut::<TabWidget>()
                .expect("the mounted control is a tab widget");
            (titles(tw), tw.selected_index())
        })
        .expect("the strip is still mounted");
        assert_eq!(observed.0, vec!["T1", "T2", "T0"]);
        assert_eq!(observed.1, Some(2), "the selection follows the tab that moved");
        assert_eq!(*finished.lock().unwrap(), Some(DropEffect::Move));
    }

    /// A tab dragged onto a *second* strip moves there with its page.
    ///
    /// The payload's index addresses the source strip's tabs, so the target inserts the carried
    /// caption and adopts the attached page; the source — told the effect was `Move` — removes its
    /// own copy without unlinking the page it no longer owns.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn tabwidget_a_tab_dragged_to_another_strip_moves_with_its_page() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};

        let mut source = TabWidget::new(Rect::new(0, 0, 600, 200));
        let page = register(crate::compat::Box::new(crate::widget::Label::new(
            "page".to_string(),
            Rect::new(0, 0, 100, 40),
        )))
        .expect("mount the page");
        let _page_guard = MountGuard(page);
        source.add_tab("T0".to_string(), Some(page));
        source.add_tab("T1".to_string(), None);
        source.set_movable(true);
        let first = source.tab_rect(0).expect("tab 0 has a band");
        let press = Point::new(first.x + first.width as i32 / 2, first.y + 5);

        let mut target = TabWidget::new(Rect::new(0, 300, 600, 200));
        target.add_tab("X0".to_string(), None);
        target.set_movable(true);
        let slice = target.tab_rect(0).expect("the target has a band");
        let drop = Point::new(slice.x + slice.width as i32 / 2, slice.y + 5);

        let source_id = register(crate::compat::Box::new(source)).expect("mount the source");
        let _source_guard = MountGuard(source_id);
        let target_id = register(crate::compat::Box::new(target)).expect("mount the target");
        let _target_guard = MountGuard(target_id);

        let viewport = Rect::new(0, 0, 700, 600);
        let mut coordinator = DragCoordinator::new();
        assert_eq!(
            coordinator.on_event(&Event::mouse_press(press.x, press.y, PRIMARY), viewport),
            None
        );
        assert_eq!(coordinator.on_event(&Event::mouse_move(drop.x, drop.y), viewport), None);
        assert_eq!(
            coordinator.on_event(&Event::mouse_release(drop.x, drop.y, PRIMARY), viewport),
            Some(DropEffect::Move)
        );

        let (source_titles, target_titles) = with_widget_mut(source_id, |widget| {
            let source = (widget as &mut dyn core::any::Any)
                .downcast_mut::<TabWidget>()
                .expect("a tab widget");
            titles(source)
        })
        .and_then(|source_titles| {
            with_widget_mut(target_id, |widget| {
                let target = (widget as &mut dyn core::any::Any)
                    .downcast_mut::<TabWidget>()
                    .expect("a tab widget");
                (source_titles, titles(target))
            })
        })
        .expect("both strips are still mounted");
        assert_eq!(source_titles, vec!["T1"], "the source copy is gone");
        assert_eq!(target_titles, vec!["T0", "X0"], "the tab landed before tab 0");

        // The page followed the tab: it is the target's child now, not the source's.
        let owned = |id| {
            with_widget_mut(id, |widget| widget.base().children().to_vec()).unwrap_or_default()
        };
        assert!(!owned(source_id).contains(&page), "the source released the page");
        assert!(owned(target_id).contains(&page), "the target adopted the page");
    }

    /// A host that feeds the same gesture to both the pointer handler and the coordinator
    /// must reorder once, not twice.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn tabwidget_a_live_drag_and_the_coordinator_do_not_double_reorder() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};

        let tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let press = Point::new(first.x + first.width as i32 / 2, first.y + 5);
        let drop = Point::new(third.x + third.width as i32 / 2, third.y + 5);
        let id = register(crate::compat::Box::new(tw)).expect("mount the strip");
        let _guard = MountGuard(id);

        let viewport = Rect::new(0, 0, 800, 300);
        let mut coordinator = DragCoordinator::new();
        let press_event = Event::mouse_press(press.x, press.y, PRIMARY);
        let move_event = Event::mouse_move(drop.x, drop.y);
        let release_event = Event::mouse_release(drop.x, drop.y, PRIMARY);

        let _ = with_widget_mut(id, |widget| widget.handle_event(&press_event));
        let _ = coordinator.on_event(&press_event, viewport);
        let _ = coordinator.on_event(&move_event, viewport);
        let _ = with_widget_mut(id, |widget| widget.handle_event(&move_event));
        let effect = coordinator.on_event(&release_event, viewport);
        let _ = with_widget_mut(id, |widget| widget.handle_event(&release_event));

        assert_eq!(effect, Some(DropEffect::Move));
        let observed = with_widget_mut(id, |widget| {
            let tw = (widget as &mut dyn core::any::Any)
                .downcast_mut::<TabWidget>()
                .expect("the mounted control is a tab widget");
            titles(tw)
        })
        .expect("the strip is still mounted");
        assert_eq!(observed, vec!["T1", "T2", "T0"], "the commit must not move the tab again");
    }

    #[test]
    fn tabwidget_movable_is_a_served_property() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };
        use crate::widget::capability::CapabilityValue;

        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(crate::widget::capability::widget_property_names(&tw)
            .expect("a tab widget declares properties")
            .contains(&"movable"));
        widget_property_set(&mut tw, "movable", CapabilityValue::Bool(true)).unwrap();
        assert!(tw.movable());
        assert_eq!(widget_property_get(&tw, "movable"), Ok(CapabilityValue::Bool(true)));

        // `tab_position` and `closable` round-trip through their tokens too.
        widget_property_set(&mut tw, "tab_position", CapabilityValue::String("south".to_string()))
            .unwrap();
        assert_eq!(tw.tab_position(), TabPosition::South);
        assert_eq!(
            widget_property_get(&tw, "tab_position"),
            Ok(CapabilityValue::String("south".to_string()))
        );
        assert!(widget_property_set(
            &mut tw,
            "tab_position",
            CapabilityValue::String("South".to_string())
        )
        .is_err());
        assert_eq!(tw.tab_position(), TabPosition::South);
    }

    /// `tab_shape` is served in both directions, over the schema's own token vocabulary.
    ///
    /// Before this, `set_tab_shape` existed and the JSON loader read a `tab_shape` key, but the
    /// property contract had no arm for it: a host could not read the shape back or set it
    /// through the property route, and the schema did not declare it either.
    #[test]
    fn tabwidget_shape_round_trips_through_the_property_contract() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };
        use crate::widget::capability::CapabilityValue;

        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(crate::widget::capability::widget_property_names(&tw)
            .expect("a tab widget declares properties")
            .contains(&"tab_shape"));

        widget_property_set(
            &mut tw,
            "tab_shape",
            CapabilityValue::String("triangular".to_string()),
        )
        .unwrap();
        assert_eq!(tw.tab_shape(), TabShape::Triangular);
        assert_eq!(
            widget_property_get(&tw, "tab_shape"),
            Ok(CapabilityValue::String("triangular".to_string()))
        );

        // An unknown token is a refused write, not a silent fallback to the current shape.
        assert!(widget_property_set(
            &mut tw,
            "tab_shape",
            CapabilityValue::String("Rounded".to_string())
        )
        .is_err());
        assert_eq!(tw.tab_shape(), TabShape::Triangular);
    }

    /// The canonical getter reports `None` until a tab exists (WDGT-13).
    #[test]
    fn selected_index_is_none_for_an_empty_tab_widget() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.selected_index(), None);
        tw.add_tab("A".to_string(), None);
        assert_eq!(tw.selected_index(), Some(0));
    }

    /// Removing a tab before the selection renumbers it, and the shift is announced.
    ///
    /// A subscriber keyed by index would otherwise keep the old number and address the
    /// neighbouring tab after the removal.
    #[test]
    fn tabwidget_remove_tab_announces_the_index_shift() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        for title in ["A", "B", "C"] {
            tw.add_tab(title.to_string(), None);
        }
        tw.set_current_index(2);

        let emitted = Arc::new(std::sync::Mutex::new(None));
        tw.current_changed.connect({
            let emitted = Arc::clone(&emitted);
            move |idx: Arc<usize>| {
                *emitted.lock().unwrap() = Some(*idx);
            }
        });

        tw.remove_tab(0);
        assert_eq!(tw.selected_index().unwrap_or(0), 1, "the selected tab keeps its identity");
        assert_eq!(*emitted.lock().unwrap(), Some(1), "the new index is announced");
    }

    /// The measured widths are reused until an input the measurement reads changes.
    ///
    /// This pins the cache contract (WDGT-16): a repeated `tab_width_hints` call must not
    /// re-measure, a changed caption must, and a font change must — the three inputs the key is
    /// derived from. The strip width is deliberately *not* one of them: it only decides the
    /// overflow division, so resizing must not force a re-shape either.
    #[test]
    fn tabwidget_widths_are_cached_and_invalidated_by_their_inputs() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 600, 200));
        tw.add_tab("One".to_string(), None);
        tw.add_tab("Two".to_string(), None);

        let first = tw.tab_width_hints();
        assert_eq!(first.len(), 2);
        assert_eq!(first, tw.tab_width_hints(), "an unchanged strip reuses the measurement");

        // Resizing changes the overflow arithmetic, not the measured widths.
        tw.set_geometry(Rect::new(0, 0, 80, 200));
        let narrow = tw.tab_width_hints();
        assert_eq!(
            narrow[0].0,
            MIN_TAB_WIDTH / 2,
            "overflow lowers the floor, not the measurement"
        );

        // A longer caption must be re-measured.
        tw.set_geometry(Rect::new(0, 0, 600, 200));
        tw.set_tab_text(1, "A considerably longer tab caption".to_string());
        let renamed = tw.tab_width_hints();
        assert!(renamed[1].1 > first[1].1, "the changed caption is measured again");
        assert_eq!(renamed[0].1, first[0].1, "the untouched tab keeps its width");

        // A font change changes every advance, so the cache key's font component must invalidate.
        let mut style = tw.style().clone();
        style.font = Some(Font::builder().family("Arial").size(28.0).build());
        tw.set_style(style);
        let larger = tw.tab_width_hints();
        assert!(larger[0].1 > first[0].1, "a larger font re-measures even when no caption changed");
    }

    /// The tab elider is the shared width-budget primitive plus this control's floor: a
    /// shortened title never paints as nothing.
    #[test]
    fn a_tab_title_is_elided_by_the_shared_primitive_with_an_ellipsis_floor() {
        let font = Font::default();
        let mut measure = |text: &str| estimate_text_width(text, &font, 1.0);

        // A title that fits is borrowed untouched, so the common case allocates nothing.
        let fitted = elide_tab_title("Short", 200, &mut measure);
        assert!(matches!(fitted, Cow::Borrowed("Short")), "{fitted:?}");

        // A title that does not fit is shortened on a character boundary and keeps the marker.
        let budget = measure("A very");
        let fitted = elide_tab_title("A very long tab title", budget, &mut measure);
        assert!(fitted.ends_with('\u{2026}'), "the shortened title keeps the marker: {fitted:?}");
        assert!(fitted.chars().count() < "A very long tab title".chars().count());

        // Even when the marker alone is wider than the budget, the tab paints the marker
        // rather than an empty caption.
        assert_eq!(elide_tab_title("A very long tab title", 1, &mut measure), "\u{2026}");
    }

    /// Overflow scrolling, elision, the corner widget and Ctrl+Tab are wired.
    #[test]
    fn overflow_elide_corner_and_ctrl_tab_are_wired() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 120, 100));
        for index in 0..6 {
            tw.add_tab(format!("A very long tab title {index}"), None);
        }
        tw.scroll_tabs(40);
        assert!(tw.overflow_scroll() > 0);
        tw.set_elide_mode(TabElideMode::ElideRight);
        assert_eq!(tw.elide_mode(), TabElideMode::ElideRight);
        tw.set_corner_widget(Some(0xC0FFEE));
        assert_eq!(tw.corner_widget(), Some(0xC0FFEE));
        assert!(tw.corner_rect().width > 0);

        let mut keys = TabWidget::new(Rect::new(0, 0, 300, 100));
        keys.add_tab("a".to_string(), None);
        keys.add_tab("b".to_string(), None);
        keys.handle_event(&crate::event::Event::KeyPress {
            key: crate::event::key_codes::TAB,
            modifiers: 0b0010,
        });
        assert_eq!(keys.selected_index(), Some(1), "Ctrl+Tab advances");
        keys.handle_event(&crate::event::Event::KeyPress {
            key: crate::event::key_codes::TAB,
            modifiers: 0b0011,
        });
        assert_eq!(keys.selected_index(), Some(0), "Ctrl+Shift+Tab goes back");
    }
}
