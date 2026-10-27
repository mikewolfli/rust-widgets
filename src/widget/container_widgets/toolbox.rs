// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool box widget.
use crate::core::{Color, HorizontalAlignment, ObjectId, Orientation, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::metrics::effective_font;

use crate::widget::capability::coercion::{
    expect_orientation, expect_string, expect_usize, orientation_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{
    BaseWidget, Draw, IconItemError, IconSource, SimpleRegistry, Widget, WidgetKind,
};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

use super::theme_snapshot::ThemeSnapshot;

/// The height of one tab in a vertical tool box, in logical pixels.
const ITEM_HEIGHT: u32 = 32;
/// The width of one tab in a horizontal tool box, in logical pixels.
const ITEM_WIDTH: u32 = 120;
/// The smallest page the content area is allowed to keep.
///
/// # Why the page cannot simply be what is left over
///
/// `content_rect` used to be `rect.height - item_height * items.len()`, floored at zero. At the
/// census size (120 px) with four items that is `120 - 128 = 0`: every item's tab was drawn and
/// the page — the whole point of a tool box — had no pixels at all, so the control rendered as a
/// list of tabs beside nothing. The reference tool box gives the page the remaining body and keeps at
/// least a line of it; this constant is that rule, in the same shape as `meter`'s reserved
/// reading band and `splitter`'s minimum pane.
const MIN_CONTENT_EXTENT: u32 = 24;

/// Tool box widget.
///
/// # Overflow
///
/// The tab strip and the page share one axis, so they cannot both have the room they would like.
/// The page keeps `MIN_CONTENT_EXTENT` and the *strip* gives way: when the tabs no longer fit,
/// the strip is scrolled by whole tabs (see [`ToolBox::scroll_offset`]) rather than letting an
/// item be placed past the control's edge. An item is therefore either drawn whole, inside the
/// strip, or not drawn at all.
pub struct ToolBox {
    base: BaseWidget,
    items: Vec<ToolBoxItem>,
    current_index: usize,
    orientation: Orientation,
    /// Index of the first tab drawn, i.e. the tab strip's scroll offset in whole tabs.
    ///
    /// Kept in `usize` rather than as a pixel offset because a tab is drawn as a whole unit: a
    /// partial tab at the top of the strip would be a tab whose label is cut in half, and the
    /// overflow rule above is deliberately "whole tabs, or none".
    scroll_offset: usize,
    /// The smooth-scroll position in tab units while a selection change animates.
    scroll_render: f32,
    /// Whether selection changes animate the strip's scroll.
    smooth_scroll: bool,
    /// Height of one vertical tab. Defaults to [`ITEM_HEIGHT`].
    item_height: u32,
    /// Emitted with the new page index whenever the current page changes through
    /// [`ToolBox::set_current_index`] or user input. `clear()` resets the index to `0`
    /// without emitting, because it also empties the page vector.
    ///
    /// This is the *selection* signal. The per-item open state is reported separately by
    /// [`Self::item_expanded_changed`], because an item can open or close without the selection
    /// moving (`ToolBoxMode::NonExclusive`).
    pub current_changed: Signal1<usize>,
    /// Emitted with `(item_index, expanded)` whenever an item's page opens or closes.
    pub item_expanded_changed: Signal1<(usize, bool)>,
    /// How items' open states interact.
    mode: ToolBoxMode,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// The item under the pointer, for the tooltip query (C-28).
    hovered_item: Option<usize>,
}
/// How a tool box treats its items' open state.
///
/// # Why the modes exist
///
/// A tool box page used to have exactly one state: whichever item `current_index` named was shown.
/// The reference control offers three policies, and they differ in *what closing one page does to
/// the others*, which is a decision only the host can make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolBoxMode {
    /// At most one item is open. Opening another collapses the one that was open.
    #[default]
    Exclusive,
    /// Any number of items may be open at once; their pages share the content box.
    NonExclusive,
    /// At most one item is open, and a press on the open item collapses it — the page behaves like
    /// a popup that the user dismisses by pressing its own tab again.
    Popup,
}

impl ToolBoxMode {
    /// The factory spelling of this mode, matching `TOOL_BOX_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            ToolBoxMode::Exclusive => "exclusive",
            ToolBoxMode::NonExclusive => "non_exclusive",
            ToolBoxMode::Popup => "popup",
        }
    }

    /// Parses a factory spelling into a mode.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "exclusive" => ToolBoxMode::Exclusive,
            "non_exclusive" | "nonexclusive" => ToolBoxMode::NonExclusive,
            "popup" => ToolBoxMode::Popup,
            _ => return None,
        })
    }
}

/// Tool box item.
pub struct ToolBoxItem {
    text: String,
    icon_source: Option<IconSource>,
    #[cfg(feature = "image")]
    icon: Option<Image>,
    tooltip: String,
    enabled: bool,
    widget: Option<ObjectId>,
    /// Whether this item's page is open.
    ///
    /// The flag is the item's own, not derived from `current_index`: in
    /// [`ToolBoxMode::NonExclusive`] several items are open at once, so "which page is visible" is a
    /// per-item answer. [`ToolBox::set_item_expanded`] is the entry that keeps the pages, the
    /// geometry and the signal in step.
    expanded: bool,
}
impl ToolBoxItem {
    /// Creates a new tool box item.
    pub fn new(text: String) -> Self {
        Self {
            text,
            icon_source: None,
            #[cfg(feature = "image")]
            icon: None,
            tooltip: String::new(),
            enabled: true,
            widget: None,
            expanded: false,
        }
    }
    /// Returns text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets text.
    ///
    /// This is the value-level setter only: it cannot request a repaint. A `ToolBoxItem` held by a
    /// control should be changed through [`ToolBox::set_item_text`].
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
    /// Returns the typed icon source, if one is assigned.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }
    /// Replaces the typed icon source and clears the legacy image slot.
    ///
    /// An explicit `None` clears **both** forms: leaving the image behind would keep the item's icon
    /// slot reserved and painted for an icon the caller just cleared.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_source = source;
        #[cfg(feature = "image")]
        {
            self.icon = None;
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
    /// A non-RGBA8 image is normalised once here, so the tile can borrow its bytes instead of
    /// transcoding every frame.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon.map(crate::widget::icon_source::normalize_rgba8_image);
        self.icon_source = None;
    }

    /// Whether this item reserves an icon tile.
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
    /// Returns whether item is enabled.
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
    /// Whether this item's page is open.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }
    /// Sets whether this item's page is open.
    ///
    /// The value-level setter only: it cannot show or hide the page, place it, or tell anyone.
    /// Use [`ToolBox::set_item_expanded`] for the container-level entry that does all three.
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }
}
impl ToolBox {
    /// Creates a tool box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Toolbox, geometry, "ToolBox"),
            items: Vec::new(),
            current_index: 0,
            orientation: Orientation::Vertical,
            scroll_offset: 0,
            scroll_render: 0.0,
            smooth_scroll: false,
            item_height: ITEM_HEIGHT,
            current_changed: Signal1::new(),
            item_expanded_changed: Signal1::new(),
            mode: ToolBoxMode::Exclusive,
            registry: None,
            hovered_item: None,
        }
    }
    /// Adds an item.
    ///
    /// The first item added to an empty tool box is open, which is the state the control has always
    /// started in: an empty strip beside an empty page becomes a strip beside a page.
    pub fn add_item(&mut self, text: String, widget: Option<ObjectId>) -> usize {
        let was_empty = self.items.is_empty();
        let mut item = ToolBoxItem::new(text);
        item.widget = widget;
        item.expanded = was_empty;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
        }
        self.items.push(item);
        self.clamp_scroll();
        self.sync_pages();
        self.items.len().saturating_sub(1)
    }
    /// Inserts an item at position.
    /// Inserts an item before the given index.
    ///
    /// An `index` past the end appends rather than panicking, matching
    /// [`Self::add_item`] and the rest of the container family (`tab_bar`,
    /// `pie_menu` clamp the same way). The inserted item starts collapsed unless it is the first
    /// item of an empty tool box (see [`Self::add_item`]).
    pub fn insert_item(&mut self, index: usize, text: String, widget: Option<ObjectId>) {
        let was_empty = self.items.is_empty();
        let index = index.min(self.items.len());
        let mut item = ToolBoxItem::new(text);
        item.widget = widget;
        item.expanded = was_empty;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
        }
        self.items.insert(index, item);
        if !was_empty && self.current_index >= index {
            self.current_index += 1;
        }
        self.clamp_scroll();
        self.sync_pages();
    }
    /// Removes an item.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            if let Some(widget_id) = self.items[index].widget {
                self.base.remove_child_linked(widget_id);
                // See `StackedWidget::remove_widget`: a removed page must not stay visible.
                crate::set_widget_visible(widget_id, false);
            }
            let was_open = self.items[index].expanded;
            self.items.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.items.is_empty() {
                self.current_index = 0;
            } else if !self.items.iter().any(|item| item.expanded) {
                // The visible page was the one removed. The control has always kept a page on
                // screen, so the nearest slot takes over rather than leaving an empty content box.
                let next = self.current_index.min(self.items.len() - 1);
                self.items[next].expanded = true;
                self.current_index = next;
                if was_open {
                    self.item_expanded_changed.emit((next, true));
                }
            }
            self.clamp_scroll();
            self.sync_pages();
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns the selected item index, or `None` when the toolbox is empty.
    pub fn selected_index(&self) -> Option<usize> {
        self.items.get(self.current_index).map(|_| self.current_index)
    }

    /// Returns current item index.
    #[deprecated(note = "use `selected_index` (Option) — WDGT-13")]
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current item index.
    ///
    /// The new page's tab is scrolled into view if overflow had hidden it; the selection is not
    /// silently applied to a page the user cannot see the tab of.
    ///
    /// Selecting also opens the page: in [`ToolBoxMode::Exclusive`] and [`ToolBoxMode::Popup`] every
    /// other item closes, and in [`ToolBoxMode::NonExclusive`] the others keep their state. A
    /// selection change is reported through [`Self::current_changed`], and an open state that
    /// moved through [`Self::item_expanded_changed`].
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.items.len() && self.current_index != index {
            self.current_index = index;
            self.scroll_current_into_view();
            self.open_exclusive(index);
            self.sync_pages();
            self.current_changed.emit(index);
            self.base.request_redraw();
        }
    }

    /// The policy the items' open states follow.
    pub fn mode(&self) -> ToolBoxMode {
        self.mode
    }

    /// Sets the open-state policy.
    ///
    /// Switching to a single-page mode closes every page but the selected one, so the policy is
    /// true after the write rather than only for the next press. Switching to
    /// [`ToolBoxMode::NonExclusive`] changes nothing: whatever is open stays open.
    pub fn set_mode(&mut self, mode: ToolBoxMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        if !matches!(mode, ToolBoxMode::NonExclusive) {
            let keep = self.current_index.min(self.items.len().saturating_sub(1));
            self.open_exclusive(keep);
        }
        self.sync_pages();
        self.base.request_redraw();
    }

    /// Whether the item at `index` has an open page, or `None` for an unknown index.
    pub fn item_expanded(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.expanded)
    }

    /// The indices of the open items, in display order.
    pub fn expanded_items(&self) -> Vec<usize> {
        self.items.iter().enumerate().filter(|(_, item)| item.expanded).map(|(i, _)| i).collect()
    }

    /// Opens or closes the item at `index`, returning `false` for an unknown index.
    ///
    /// # How the modes read this
    ///
    /// * [`ToolBoxMode::Exclusive`] — opening one item closes the rest; closing the open item is
    ///   allowed and leaves every page closed.
    /// * [`ToolBoxMode::NonExclusive`] — opening and closing are independent; several pages may be
    ///   open and they share the content box.
    /// * [`ToolBoxMode::Popup`] — opening one item closes the rest, and closing the open item is the
    ///   gesture that dismisses the popup.
    ///
    /// The page's visibility and rectangle are applied from the item flag
    /// (`Self::sync_pages`), and a real change is announced through
    /// [`Self::item_expanded_changed`]; a repeated write is a no-op.
    pub fn set_item_expanded(&mut self, index: usize, expanded: bool) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.items[index].expanded == expanded {
            return true;
        }
        if expanded {
            let previous = self.current_index;
            self.current_index = index;
            self.open_exclusive(index);
            self.items[index].expanded = expanded;
            if previous != index {
                self.current_changed.emit(index);
            }
        } else {
            self.items[index].expanded = false;
        }
        self.scroll_current_into_view();
        self.sync_pages();
        self.base.request_redraw();
        self.item_expanded_changed.emit((index, expanded));
        true
    }

    /// Opens `keep` and closes every other item in the modes that allow one page only.
    fn open_exclusive(&mut self, keep: usize) {
        if matches!(self.mode, ToolBoxMode::NonExclusive) {
            if let Some(item) = self.items.get_mut(keep) {
                item.expanded = true;
            }
            return;
        }
        for (index, item) in self.items.iter_mut().enumerate() {
            item.expanded = index == keep;
        }
    }

    /// Applies the page model to the mounted pages: every open item's page is shown and fills its
    /// share of [`Self::content_rect`], every other page is hidden.
    ///
    /// # Why this is the control's job
    ///
    /// A tool box shows the pages of its *open* items beside its strip. Linking a page as a child
    /// was only half the model: the tree painter draws every visible child, so the closed pages
    /// painted over the open one (and over the strip). Hiding them is also what keeps a click from
    /// reaching a page the user cannot see.
    fn sync_pages(&self) {
        #[cfg(not(alloc_frugal))]
        {
            for item in self.items.iter() {
                if let Some(widget_id) = item.widget {
                    crate::set_widget_visible(widget_id, item.expanded);
                }
            }
            self.place_pages();
        }
    }

    /// The rectangles the open pages occupy inside the content box.
    ///
    /// One open page fills the box, which is the only state the single-page modes produce. In
    /// [`ToolBoxMode::NonExclusive`] several pages are open at once and they tile the box along its
    /// long axis, each taking an equal share — the same "share the band" rule the strip uses.
    fn page_rects(&self, content: Rect) -> Vec<(usize, Rect)> {
        if self.mode == ToolBoxMode::Popup {
            // C-29: the open page is presented in a floating layer beside its trigger, not in the
            // content band. `popup_rect` is the single derivation the painter, the hit test and
            // the child placement share.
            return match self.popup_rect(content) {
                Some(rect) => self
                    .items
                    .iter()
                    .position(|item| item.expanded)
                    .map(|index| crate::compat::vec![(index, rect)])
                    .unwrap_or_default(),
                None => Vec::new(),
            };
        }
        let open: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.expanded)
            .map(|(index, _)| index)
            .collect();
        if open.is_empty() {
            return Vec::new();
        }
        let count = open.len() as u32;
        let share = content.height / count;
        open.into_iter()
            .enumerate()
            .map(|(slot, index)| {
                let offset = slot as u32 * share;
                let height = if slot as u32 + 1 == count {
                    content.height.saturating_sub(offset)
                } else {
                    share
                };
                (
                    index,
                    Rect::new(
                        content.x,
                        content.y.saturating_add_unsigned(offset),
                        content.width,
                        height,
                    ),
                )
            })
            .collect()
    }

    /// The floating rectangle a [`ToolBoxMode::Popup`] page occupies (C-29).
    ///
    /// Anchored to the open item's own tab and placed just beyond it along the strip's short
    /// axis, then clamped inside the content box so it can never leave the control. `None` when no
    /// item is open (there is nothing to present) or the mode is not `Popup`.
    fn popup_rect(&self, content: Rect) -> Option<Rect> {
        if self.mode != ToolBoxMode::Popup {
            return None;
        }
        let open = self.items.iter().position(|item| item.expanded)?;
        let anchor = self.item_rect(open)?;
        const GAP: i32 = 4;
        const POPUP_MAJOR: u32 = 240;
        const POPUP_MINOR: u32 = 180;
        match self.orientation {
            Orientation::Vertical => {
                let height = content.height.min(POPUP_MINOR);
                let max_y = content.y + content.height.saturating_sub(height) as i32;
                let y =
                    (anchor.y + anchor.height as i32 + GAP).clamp(content.y, max_y.max(content.y));
                Some(Rect::new(content.x, y, content.width, height))
            }
            Orientation::Horizontal => {
                let width = content.width.min(POPUP_MAJOR);
                let max_x = content.x + content.width.saturating_sub(width) as i32;
                let x =
                    (anchor.x + anchor.width as i32 + GAP).clamp(content.x, max_x.max(content.x));
                Some(Rect::new(x, content.y, width, content.height))
            }
        }
    }

    /// The item under the pointer, for the tooltip path (C-28).
    pub fn hovered_item(&self) -> Option<usize> {
        self.hovered_item
    }

    /// The tooltip of the item under the pointer, if it has one (C-28).
    ///
    /// The text was stored and there was no pointer path or query, so a host could not reach it.
    /// An empty tooltip reports `None` rather than `Some("")`, which is the difference between
    /// "no tooltip" and "a tooltip that renders nothing".
    pub fn hovered_tooltip(&self) -> Option<&str> {
        let index = self.hovered_item?;
        self.items.get(index).map(|item| item.tooltip.as_str()).filter(|text| !text.is_empty())
    }

    /// Places the open pages in the content box.
    ///
    /// The box comes from the same `content_rect` the painter uses, so a page and the area drawn
    /// around it cannot disagree about where the page is.
    fn place_pages(&self) {
        #[cfg(not(alloc_frugal))]
        {
            // One content derivation for every page in this pass (C-P10).
            let content = self.content_rect();
            for (index, rect) in self.page_rects(content) {
                if let Some(widget_id) = self.items[index].widget {
                    crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
                }
            }
        }
    }
    /// Returns current item widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.items.get(self.current_index).and_then(|item| item.widget)
    }
    /// Returns item at index.
    pub fn item(&self, index: usize) -> Option<&ToolBoxItem> {
        self.items.get(index)
    }
    /// Returns mutable item at index.
    ///
    /// A mutation made through this reference is **not** followed by a repaint, because the item
    /// has no back-reference to the control that paints it. Use [`Self::set_item_text`] or
    /// [`Self::set_item_icon_source`] when the change must be visible.
    pub fn item_mut(&mut self, index: usize) -> Option<&mut ToolBoxItem> {
        self.items.get_mut(index)
    }

    /// Replaces an item's text and repaints the control.
    ///
    /// # Why this exists
    ///
    /// `ToolBoxItem::set_text` mutates a value that the strip paints, but the item cannot ask the
    /// control for a repaint — so a text change made through [`Self::item_mut`] stayed invisible
    /// until some unrelated repaint happened. This entry point closes the loop and short-circuits
    /// an equal value, so setting the same text does not invalidate a frame.
    ///
    /// Returns whether the text changed.
    pub fn set_item_text(&mut self, index: usize, text: impl Into<String>) -> bool {
        let text = text.into();
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.text == text {
            return false;
        }
        item.text = text;
        self.base.request_redraw();
        true
    }
    /// Assigns a typed icon source to an existing item index.
    pub fn set_item_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let len = self.items.len();
        let Some(item) = self.items.get_mut(index) else {
            return Err(IconItemError::IndexOutOfBounds { index, len });
        };
        item.set_icon_source(source);
        self.base.request_redraw();
        Ok(())
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        // The other axis has a different extent and a different tab size, so an offset that was
        // legal in one is not necessarily legal in the other.
        self.clamp_scroll();
        self.place_pages();
        self.base.request_redraw();
    }

    /// Removes all items from the toolbox.
    pub fn clear(&mut self) {
        for item in self.items.drain(..) {
            if let Some(widget_id) = item.widget {
                self.base.remove_child_linked(widget_id);
                crate::set_widget_visible(widget_id, false);
            }
        }
        self.current_index = 0;
        self.scroll_offset = 0;
        self.scroll_render = 0.0;
        self.base.request_redraw();
    }
    /// The extent one tab occupies along the strip's axis.
    fn item_extent(&self) -> u32 {
        match self.orientation {
            Orientation::Vertical => self.item_height.max(1),
            Orientation::Horizontal => ITEM_WIDTH,
        }
    }

    /// The height of one vertical tab.
    pub fn item_height(&self) -> u32 {
        self.item_height
    }

    /// Sets the height of one vertical tab.
    ///
    /// Returns nothing because every positive value is valid; zero is clamped to one. The
    /// value is the strip's own metric, not a content measurement, and it defaults to
    /// `ITEM_HEIGHT` so an existing toolbox keeps its pixels.
    pub fn set_item_height(&mut self, height: u32) {
        let height = height.max(1);
        if self.item_height == height {
            return;
        }
        self.item_height = height;
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether scroll changes animate.
    pub fn smooth_scroll(&self) -> bool {
        self.smooth_scroll
    }

    /// Enables or disables the scroll tween.
    ///
    /// Off by default, so a selection still snaps exactly as it did. When on, the strip's
    /// scroll position eases toward the selected tab over subsequent [`Widget::tick`] frames.
    pub fn set_smooth_scroll(&mut self, smooth: bool) {
        if self.smooth_scroll == smooth {
            return;
        }
        self.smooth_scroll = smooth;
        if !smooth {
            self.scroll_render = self.scroll_offset as f32;
        }
        self.base.request_redraw();
    }

    /// How much room the tab strip has, along its own axis.
    fn strip_extent(&self) -> u32 {
        let rect = self.geometry();
        match self.orientation {
            Orientation::Vertical => rect.height,
            Orientation::Horizontal => rect.width,
        }
    }

    /// The largest value [`Self::scroll_offset`] may take. Zero when every tab fits.
    ///
    /// The strip is never allowed to spend the page's own `MIN_CONTENT_EXTENT`: that is what
    /// makes "the page keeps at least one line" true at *any* tab count, including the case
    /// where the tabs alone are taller than the whole control.
    pub fn max_scroll(&self) -> usize {
        let extent = self.strip_extent();
        let visible =
            (extent.saturating_sub(MIN_CONTENT_EXTENT) / self.item_extent().max(1)) as usize;
        self.items.len().saturating_sub(visible)
    }

    /// Returns the tab strip's scroll offset, in whole tabs.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls the tab strip to `offset`, clamped to [`Self::max_scroll`].
    ///
    /// The clamp is what makes this a total function for a caller: an offset past the end simply
    /// shows the last full strip, which is what a scroll-into-view means. Requests a redraw only
    /// when the offset actually changes.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        let clamped = offset.min(self.max_scroll());
        if self.scroll_offset != clamped {
            self.scroll_offset = clamped;
            if !self.smooth_scroll {
                self.scroll_render = clamped as f32;
            }
            self.base.request_redraw();
        }
    }

    /// Re-clamps the scroll offset after the item list or the geometry changed.
    ///
    /// # Why this is needed separately from the setter
    ///
    /// Every other mutator on this control can shrink the strip — removing an item, clearing the
    /// list, resizing the control — and a stale offset would then scroll a strip that fits,
    /// which shows *fewer* tabs than there is room for. Every one of those paths ends here rather
    /// than each remembering the clamp, because a rule that lives in one place cannot be applied
    /// in three of four.
    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
    }

    /// Keeps the page's own tab inside the visible strip, scrolling it into view when it is not.
    ///
    /// A selection made by keyboard or by the property contract can name a tab that overflow has
    /// scrolled off, and a page whose tab cannot be seen is a page the user cannot tell is open.
    fn scroll_current_into_view(&mut self) {
        let extent = self.strip_extent();
        let visible =
            (extent.saturating_sub(MIN_CONTENT_EXTENT) / self.item_extent().max(1)) as usize;
        if visible == 0 {
            // No tab fits alongside the page's own minimum, so there is nothing to scroll to;
            // scrolling would only hide a tab to show another.
            return;
        }
        if self.current_index < self.scroll_offset {
            self.scroll_offset = self.current_index;
        } else if self.current_index >= self.scroll_offset + visible {
            self.scroll_offset = self.current_index + 1 - visible;
        }
        self.clamp_scroll();
    }

    /// Returns item rectangle at index.
    ///
    /// # Why the result is intersected with the strip
    ///
    /// The index is offset by the strip's scroll position and the rectangle is then clipped to
    /// the strip, so an item that overflow has pushed past the strip's end yields a *partial*
    /// rectangle — up to and including an empty one — rather than a rectangle outside the
    /// control. That is the invariant the draw path and hit testing both rely on: an item either
    /// has pixels inside the strip or is reported as having none. Previously the position was
    /// `item_extent * index` with no reference to the control's extent at all, so item 5 of 6 in a
    /// 120 px control was painted from y = 160 downward, entirely outside the control.
    fn item_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let rect = self.geometry();
        let extent = self.item_extent();
        // Item positions are relative to the rendered scroll fraction, which equals the
        // integer offset when smoothing is off. A negative relative slot is fully hidden.
        let relative = index as f32
            - if self.smooth_scroll { self.scroll_render } else { self.scroll_offset as f32 };
        if relative < 0.0 {
            return None;
        }
        let position = (relative * extent as f32).round() as u32;
        match self.orientation {
            Orientation::Horizontal => {
                let strip = Rect::new(rect.x, rect.y, rect.width, rect.height);
                let full = Rect::new(
                    rect.x.saturating_add_unsigned(position),
                    rect.y,
                    ITEM_WIDTH,
                    rect.height,
                );
                full.intersection(&strip)
            }
            Orientation::Vertical => {
                let strip = Rect::new(rect.x, rect.y, rect.width, rect.height);
                let full = Rect::new(
                    rect.x,
                    rect.y.saturating_add_unsigned(position),
                    rect.width,
                    self.item_height,
                );
                full.intersection(&strip)
            }
        }
    }

    /// Returns content rectangle.
    ///
    /// The page starts where the *visible* strip ends — not where an unbounded strip would have
    /// ended — so the page cannot be pushed off the control by tabs that overflow. It keeps at
    /// least [`MIN_CONTENT_EXTENT`], which is what stops a tab list from reducing the page to
    /// zero height; the tabs, not the page, are what give way when both cannot fit.
    ///
    /// The scrolling has an important consequence worth stating: because at most
    /// `(extent - MIN_CONTENT_EXTENT) / item_extent` tabs are visible at once, the strip's end and
    /// the page's start are always inside the control, and their sum can never exceed it.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let extent = self.item_extent();
        // How many whole tabs the strip can show beside the reserved page.
        let capacity = self.strip_extent().saturating_sub(MIN_CONTENT_EXTENT) / extent.max(1);
        // The strip only occupies as much as it **has** tabs, capped by that capacity. Using
        // the capacity alone reserved room for tabs that do not exist: with two items in a
        // 300px vertical box the page started at `8 * 32 = 256`, so 192px of the control
        // was empty strip and the page was a 44px sliver at the bottom.
        let strip_used = (self.items.len() as u32).min(capacity) * extent;
        match self.orientation {
            Orientation::Horizontal => {
                let remaining = rect.width.saturating_sub(strip_used);
                let content_width = if self.items.len() as u32 <= capacity {
                    remaining.max(MIN_CONTENT_EXTENT).min(rect.width)
                } else {
                    remaining
                };
                Rect::new(
                    rect.x.saturating_add_unsigned(strip_used),
                    rect.y,
                    content_width,
                    rect.height,
                )
            }
            Orientation::Vertical => {
                let remaining = rect.height.saturating_sub(strip_used);
                let content_height = if self.items.len() as u32 <= capacity {
                    remaining.max(MIN_CONTENT_EXTENT).min(rect.height)
                } else {
                    remaining
                };
                Rect::new(
                    rect.x,
                    rect.y.saturating_add_unsigned(strip_used),
                    rect.width,
                    content_height,
                )
            }
        }
    }
    /// Returns index of item at position.
    fn item_at_position(&self, pos: Point) -> Option<usize> {
        // C-P10: the strip is a uniform run, so the slot is solved arithmetically and only the
        // candidate is confirmed against the painted rectangle. A mismatch falls back to the
        // neighbouring slots and then to the scan, so the shortcut can only make it faster.
        let rect = self.geometry();
        let coordinate = match self.orientation {
            Orientation::Horizontal => pos.x - rect.x,
            Orientation::Vertical => pos.y - rect.y,
        };
        if coordinate < 0 {
            return None;
        }
        let extent = self.item_extent().max(1) as f32;
        let scroll =
            if self.smooth_scroll { self.scroll_render } else { self.scroll_offset as f32 };
        let slot = (coordinate as f32 / extent + scroll).floor();
        if slot >= 0.0 {
            let index = slot as usize;
            if self.item_rect(index).is_some_and(|item_rect| item_rect.contains(pos)) {
                return Some(index);
            }
            for neighbour in [index.saturating_sub(1), index + 1] {
                if self.item_rect(neighbour).is_some_and(|item_rect| item_rect.contains(pos)) {
                    return Some(neighbour);
                }
            }
        }
        for i in 0..self.items.len() {
            if let Some(item_rect) = self.item_rect(i) {
                if item_rect.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }
}
// Implement Widget trait
impl Widget for ToolBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // C-019: the current page's control plus the strip it shares the main axis with. The
        // historical 200x250 is the floor when no page is mounted.
        let Some(content) = self.current_widget().and_then(|widget| {
            crate::widget::runtime::with_widget(widget, |widget| widget.size_hint())
        }) else {
            return crate::core::Size::new(200, 250);
        };
        match self.orientation {
            Orientation::Horizontal => crate::core::Size::new(
                content.width.saturating_add(ITEM_WIDTH),
                content.height.max(ITEM_HEIGHT),
            ),
            Orientation::Vertical => crate::core::Size::new(
                content.width.max(ITEM_WIDTH),
                content.height.saturating_add(ITEM_HEIGHT),
            ),
        }
    }

    /// Moves the tool box and re-places the visible pages.
    ///
    /// The content box is a function of the control's rectangle (and of the orientation),
    /// so a layout that moves the tool box has to move the pages with it.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_pages();
    }

    /// Eases the strip toward the selected tab when smooth scrolling is on.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.smooth_scroll {
            return false;
        }
        let target = self.scroll_offset as f32;
        let delta = target - self.scroll_render;
        if delta.abs() <= 0.01 {
            self.scroll_render = target;
            return false;
        }
        let step = (delta_ms as f32 / 120.0).clamp(0.0, 1.0);
        self.scroll_render += delta * step;
        if (target - self.scroll_render).abs() <= 0.01 {
            self.scroll_render = target;
            self.base.request_redraw();
            return false;
        }
        self.base.request_redraw();
        true
    }

    fn is_animating(&self) -> bool {
        self.smooth_scroll && (self.scroll_render - self.scroll_offset as f32).abs() > 0.01
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "item_expanded_changed" => Some(EventSignalRef::mapped(
                "item_expanded_changed",
                &self.item_expanded_changed,
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::Bool(v.1),
                    ]))
                },
            )),
            _ => None,
        }
    }
}

/// `ToolBox`'s property contract.
impl WidgetProperties for ToolBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "current_index" => Ok(CapabilityValue::UInt(self.selected_index().unwrap_or(0) as u64)),
            "orientation" => {
                Ok(CapabilityValue::String(orientation_to_str(self.orientation()).to_string()))
            }
            "mode" => Ok(CapabilityValue::String(self.mode().as_str().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_orientation(value)?);
                Ok(())
            }
            "mode" => {
                let token = expect_string(value)?;
                let mode =
                    ToolBoxMode::from_name(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_mode(mode);
                Ok(())
            }
            // Derived from the item list.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TOOL_BOX_PROPERTIES`.
        property_names_of![
            "item_count",
            "current_index",
            "orientation",
            "mode",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tool_box` publishes.
    ///
    /// `add_item` takes the item's label and `remove_item` takes the index to
    /// remove, so neither can complete without a payload: they are refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_item" | "remove_item" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl ToolBox {
    /// The tint for an item's embedded icon tile, derived from the resolved style.
    ///
    /// # Why this is a named function (issue §9 item #7)
    ///
    /// The tile colour used to be one of two hardcoded RGB literals, so an embedded icon ignored the
    /// active theme while every other pixel of the item honoured it. Deriving it from the control's
    /// own `border_color` / `background_color` gives embedded icons the same colour source as the rest
    /// of the control, and naming the derivation makes it testable without driving a real `Image`.
    ///
    /// The selected tile keeps the full ink colour; an unselected tile is that ink blended halfway
    /// toward the surface, so it reads as the same colour family at lower emphasis.
    ///
    /// The typed `IconSource` slot is available even when the legacy image feature is disabled, so
    /// the derivation must be available in every profile that can paint toolbox items.
    pub(crate) fn icon_tile_color(ink: Color, surface: Color, is_current: bool) -> Color {
        if is_current {
            ink
        } else {
            ink.blend(&surface, 0.5)
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
}
impl EventHandler for ToolBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if let Some(index) = self.item_at_position(*pos) {
                    if self.items[index].enabled {
                        // The three modes differ in what a press *means*: a selection in the
                        // single-page modes, a toggle of that page in non-exclusive mode, and a
                        // dismissal when the open page's own tab is pressed in popup mode.
                        match self.mode {
                            ToolBoxMode::Exclusive => self.set_current_index(index),
                            ToolBoxMode::NonExclusive | ToolBoxMode::Popup => {
                                let expanded = self.items[index].expanded;
                                self.set_item_expanded(index, !expanded);
                            }
                        }
                    }
                }
            }
            Event::MouseMove { pos } => {
                // C-28: track the hovered item so a host can drive the existing TooltipManager.
                let hovered = self.item_at_position(*pos);
                if hovered != self.hovered_item {
                    self.hovered_item = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_item.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } => {
                // A tab that overflow has scrolled off the strip is still reachable by the arrow
                // key that walks toward it: `set_current_index` scrolls the strip to keep the
                // selection's tab visible, so the two agree on where the strip sits and the
                // keyboard cannot select a page whose tab is nowhere on screen.
                let next = match (self.orientation, key) {
                    // Vertical: Up/Down; Horizontal: Left/Right
                    (Orientation::Vertical, 38) | (Orientation::Horizontal, 37) => {
                        // Up/Left
                        if self.current_index > 0 {
                            Some(self.current_index - 1)
                        } else {
                            None
                        }
                    }
                    (Orientation::Vertical, 40) | (Orientation::Horizontal, 39) => {
                        // Down/Right
                        let next = self.current_index + 1;
                        if next < self.items.len() {
                            Some(next)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(idx) = next {
                    self.set_current_index(idx);
                }
            }
            _ => {}
        }
        // C-P10: one content derivation for the guard and for the page hit test, and the popup's
        // own floating rect when that mode is active.
        let content = self.content_rect();
        let allow_child_event = match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos } => {
                self.popup_rect(content).unwrap_or(content).contains(*pos)
            }
            _ => true,
        };
        // Forward content events only to the open page that owns the point; a coordinate-free event
        // goes to the selected page, which is the one a keyboard path is about.
        if allow_child_event {
            if let Some(ref reg) = self.registry {
                match event {
                    Event::MousePress { pos, .. }
                    | Event::MouseRelease { pos, .. }
                    | Event::MouseMove { pos }
                    | Event::MouseDoubleClick { pos, .. }
                    | Event::ContextMenu { pos } => {
                        let hit = self
                            .page_rects(content)
                            .into_iter()
                            .find(|(_, rect)| rect.contains(*pos))
                            .and_then(|(index, _)| self.items[index].widget);
                        if let Some(widget_id) = hit {
                            reg.borrow_mut().forward_event(widget_id, event);
                        }
                    }
                    _ => {
                        if let Some(widget_id) = self.current_widget() {
                            reg.borrow_mut().forward_event(widget_id, event);
                        }
                    }
                }
            }
        }
    }
}
impl Draw for ToolBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // The page is drawn **before** the tabs, and the tabs are drawn only where `item_rect`
        // says they are. Both are consequences of the same rule: the strip and the page share the
        // control's axis, so anything the tabs are allowed to paint must already be inside the
        // strip, and the page must not paint over the tabs that sit above it.
        let content_rect = self.content_rect();
        // Every colour below used to be a literal, so a themed toolbox kept a white
        // page and light tabs inside a dark window. The style is read once and each
        // literal becomes that field's fallback, so nothing that was visible before
        // becomes invisible now.
        let style = self.base.style();
        // Draw content background.
        //
        // `tool_box` is absent from the role table, so it classifies as `Surface` and the
        // active theme writes the *window fill* into `style.background_color`. Taking that
        // value at face value painted the content area in the window's own colour — the two
        // rectangles in the snapshot were byte-identical, so an empty toolbox read as a bare
        // border with a hole in it. `splitter`, `stacked_widget`, `mdi_area` and `carousel`
        // all detect exactly this case and re-derive a visible step; this one did not.
        let (window_fill, accent) = {
            // One theme read for the frame (C-P01): the window fill and the accent token used to
            // be a single manager visit here, but the resolved style was a second one.
            let theme = ThemeSnapshot::capture("tool_box");
            match theme.palette {
                Some(palette) => (palette.background, palette.accent),
                None => (Color::WHITE, Color::rgb(100, 100, 200)),
            }
        };
        let page = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&style.text_color.unwrap_or(Color::BLACK), 0.08),
        };
        // The selected tab is a state highlight derived from the theme's accent token rather
        // than a hardcoded light-blue literal. The literal was invisible-proof in a light
        // theme (it is what the no-theme fallback still uses) but painted a glaring pale block
        // inside a dark one. A tint of the accent over the page keeps the selection visible in
        // both appearances, and the accent itself draws the selected frame.
        let selected_background = page.blend(&accent, 0.18);
        let selected_border = accent;
        // Draw content background
        context.fill_rect(
            Rect::new(content_rect.x, content_rect.y, content_rect.width, content_rect.height),
            page,
        );
        // Draw content border
        context.draw_rect(
            Rect::new(content_rect.x, content_rect.y, content_rect.width, content_rect.height),
            style
                .border_color
                .filter(|resolved| *resolved != page)
                .unwrap_or_else(|| page.blend(&Color::BLACK, 0.25)),
        );
        // Every open page is drawn, clipped to its own share of the content box. In the single-page
        // modes that is exactly the one page the selection names; in non-exclusive mode it is each
        // open page in its own band. This happens *before* the tab strip so the tabs stay on top of
        // their own surface: a page that painted a border or a background of its own could still
        // reach into the strip, and the tabs are the control's chrome rather than its content.
        //
        // C-29: `Popup` mode draws its page in the floating rect and adds the layer's own frame,
        // so the presentation actually reads as a popup rather than as the ordinary page band.
        for (index, page_rect) in self.page_rects(content_rect) {
            if let Some(widget_id) = self.items[index].widget {
                if self.mode == ToolBoxMode::Popup {
                    context.fill_rect(page_rect, page);
                    context.draw_rect(page_rect, style.border_color.unwrap_or(selected_border));
                }
                if let Some(ref reg) = self.registry {
                    // Two acquisitions are deliberate, not an oversight: the guard must not be held
                    // across `draw_widget`, because a nested container on the page borrows the same
                    // registry to draw its own children and would panic as an aliased `RefCell`
                    // borrow. Each acquisition is O(1), so there is nothing to merge here (VIEW-N04).
                    reg.borrow_mut().set_widget_geometry(widget_id, page_rect);
                    context.push_clip(page_rect.x, page_rect.y, page_rect.width, page_rect.height);
                    reg.borrow_mut().draw_widget(widget_id, context);
                    context.pop_clip();
                }
            }
        }
        // Draw items
        for i in 0..self.items.len() {
            if let Some(item_rect) = self.item_rect(i) {
                let item = &self.items[i];
                let is_current = i == self.current_index;
                let is_enabled = item.enabled;
                // Draw item background
                //
                // The tab strip is this widget's own surface, so it uses the style;
                // the *selected* tab is a state highlight, not a colour, and is kept
                // as the literal it always was (swapping it for `border_color` would
                // have turned the selection into a plain border-coloured box).
                let bg_color = if !is_enabled {
                    style.background_color.unwrap_or(Color::rgb(240, 240, 240))
                } else if is_current {
                    selected_background
                } else {
                    style.background_color.unwrap_or(Color::rgb(240, 240, 240))
                };
                context.fill_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    bg_color,
                );
                // Draw item border
                let border_color = if !is_enabled {
                    style.border_color.unwrap_or(Color::rgb(200, 200, 200))
                } else if is_current {
                    selected_border
                } else {
                    style.border_color.unwrap_or(Color::rgb(200, 200, 200))
                };
                context.draw_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    border_color,
                );
                let padding = 5i32;
                let icon_size = 16u32;
                let has_icon = item.has_icon();
                let text_x = if has_icon {
                    item_rect.x + icon_size as i32 + padding + 4
                } else {
                    item_rect.x + padding
                };
                if has_icon {
                    let icon_x = item_rect.x + padding;
                    let icon_y = item_rect.y + (item_rect.height as i32 - icon_size as i32) / 2;
                    let icon_rect = Rect::new(icon_x, icon_y, icon_size, icon_size);
                    // Draw a small rounded square as the icon tile.
                    //
                    // The tile colour is **derived from the resolved style**, not a literal: the
                    // selected tile uses the control's own `border_color` (the same token the
                    // selected item's frame already uses) and an unselected tile is that colour
                    // blended toward the surface. Two hardcoded RGB literals here meant the embedded
                    // icon ignored the theme entirely \u2014 the colour-source gap the icon audit
                    // (issue \u00a79 item #7) flagged. `text_color` remains the fallback so a build
                    // with no active theme still paints a visible tile.
                    let tile_ink = style.border_color.unwrap_or(Color::rgb(100, 100, 200));
                    let surface = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
                    let icon_bg_color = Self::icon_tile_color(tile_ink, surface, is_current);
                    context.fill_rounded_rect(icon_rect, 3, icon_bg_color);
                    let glyph_rect = Rect::new(icon_x + 2, icon_y + 2, 12, 12);
                    if let Some(source) = item.icon_source {
                        if let Err(error) = crate::widget::paint_icon(
                            context,
                            source.into(),
                            glyph_rect,
                            style.text_color.unwrap_or(Color::rgb(255, 255, 255)),
                        ) {
                            log::warn!("toolbox item {i} icon failed: {error:?}");
                        }
                    }
                    // The legacy `Image` slot reserved this tile and then painted a stand-in dot
                    // instead of the image the caller set. It now draws the image itself, through
                    // the same seam every other legacy image slot in the crate uses.
                    #[cfg(feature = "image")]
                    if item.icon_source.is_none() {
                        if let Some(image) = item.icon.as_ref() {
                            if !crate::widget::draw_icon_image(context, image, glyph_rect) {
                                log::warn!("toolbox item {i} icon image could not be drawn");
                            }
                        }
                    }
                }
                // Draw item text
                //
                // A disabled item keeps its dimmed literal: that muted grey *is* how
                // the state is communicated, and `text_color` would repaint it at
                // full strength and erase the distinction.
                let item_text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
                let text_color =
                    if !is_enabled { Color::rgb(150, 150, 150) } else { item_text_color };
                // The label is centred on the item's own line box. `item_rect.y +
                // item_rect.height / 2` placed the glyph box's top edge on the item's middle
                // line instead, which drew the text half a line low.
                // The **effective font** — the resolved theme/caller font — not a hardcoded default,
                // so the item label honours the theme body font and the user's text scale
                // (D09-STYLE-01).
                let text_font = effective_font(style);
                let item_line = context.text_line(item_rect, text_font);
                // Fitted inside the item: horizontal tabs are a fixed width, so a long item name
                // used to paint over the next tab (and, vertically, outside the control).
                let text_width = (item_rect.x + item_rect.width as i32 - text_x).max(0) as u32;
                context.draw_text_fitted_cow(
                    Rect::new(text_x, item_line.y, text_width, item_line.height.max(1)),
                    &item.text,
                    text_font,
                    text_color,
                    HorizontalAlignment::Left,
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // ── Helper constants ──────────────────────────────────────────────────

    fn widget_id_1() -> ObjectId {
        9101
    }
    fn widget_id_2() -> ObjectId {
        9102
    }
    fn widget_id_3() -> ObjectId {
        9103
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────

    #[test]
    fn toolbox_creation_defaults() {
        let tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.count(), 0, "should have no items");
        assert_eq!(tb.selected_index().unwrap_or(0), 0, "current index should be 0");
        assert_eq!(tb.current_widget(), None, "no current widget");
        assert_eq!(tb.orientation(), Orientation::Vertical, "default orientation");
        assert!(tb.is_visible(), "should be visible");
        assert!(tb.is_enabled(), "should be enabled");
        assert_eq!(tb.geometry(), Rect::new(0, 0, 200, 160));
        assert_eq!(tb.kind(), WidgetKind::Toolbox);
        assert!(tb.registry().is_none(), "registry should be None by default");
    }

    // ── 2. Adding items ───────────────────────────────────────────────────

    #[test]
    fn toolbox_add_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        let idx = tb.add_item("First".to_string(), None);
        assert_eq!(idx, 0, "first item index should be 0");
        assert_eq!(tb.count(), 1);

        let idx2 = tb.add_item("Second".to_string(), Some(widget_id_1()));
        assert_eq!(idx2, 1, "second item index should be 1");
        assert_eq!(tb.count(), 2);

        // The widget associated with the second item becomes a child
        let children = tb.children();
        assert!(children.contains(&widget_id_1()), "child widget should be tracked");
    }

    #[test]
    fn toolbox_add_item_without_widget_does_not_add_child() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("NoWidget".to_string(), None);
        assert!(tb.children().is_empty(), "no children when no widget is given");
    }

    #[test]
    fn toolbox_typed_item_icon_is_checked_rendered_and_cleared_with_items() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "toolbox-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut toolbox = ToolBox::new(Rect::new(0, 0, 200, 160));
        toolbox.add_item("Page".to_string(), None);
        assert_eq!(
            toolbox.set_item_icon_source(1, Some(IconSource::static_geometry(&RING))),
            Err(IconItemError::IndexOutOfBounds { index: 1, len: 1 })
        );
        toolbox
            .set_item_icon_source(0, Some(IconSource::static_geometry(&RING)))
            .expect("existing toolbox item accepts typed source");
        assert!(render_to_svg(&mut toolbox).contains("fill-rule=\"nonzero\""));
        toolbox.clear();
        assert_eq!(
            toolbox.set_item_icon_source(0, None),
            Err(IconItemError::IndexOutOfBounds { index: 0, len: 0 })
        );
    }

    /// A legacy item `Image` used to reserve the tile and then paint a stand-in dot instead of the
    /// image the caller set -- the API promised an icon and drew a decoration.
    #[cfg(feature = "image")]
    #[test]
    fn toolbox_legacy_item_image_is_drawn_and_cleared() {
        let mut toolbox = ToolBox::new(Rect::new(0, 0, 200, 160));
        toolbox.add_item("Page".to_string(), None);
        let plain = render_to_svg(&mut toolbox);
        assert!(!plain.contains("<image"), "an item with no icon draws no image");

        toolbox.item_mut(0).expect("the item exists").set_icon(Some(
            Image::try_from_rgba(vec![200; 4 * 4 * 4], 4, 4)
                .expect("the buffer matches its declared size"),
        ));
        assert!(toolbox.item(0).expect("the item exists").has_icon());
        assert!(render_to_svg(&mut toolbox).contains("<image"), "the image must reach the backend");

        // An explicit typed clear clears the legacy image too, so the tile cannot keep painting an
        // icon the caller just cleared.
        toolbox.set_item_icon_source(0, None).expect("the item exists");
        assert!(!toolbox.item(0).expect("the item exists").has_icon());
        assert!(
            !render_to_svg(&mut toolbox).contains("<image"),
            "clear must release the image slot"
        );
    }

    // ── 3. Setting/getting current index ──────────────────────────────────

    #[test]
    fn toolbox_get_set_current_index() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        assert_eq!(tb.selected_index().unwrap_or(0), 0, "starts at 0");
        tb.set_current_index(1);
        assert_eq!(tb.selected_index().unwrap_or(0), 1);
        tb.set_current_index(0);
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn toolbox_set_current_index_out_of_bounds_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("Only".to_string(), None);
        tb.set_current_index(5); // out of bounds
        assert_eq!(tb.selected_index().unwrap_or(0), 0, "should remain 0");
    }

    #[test]
    fn toolbox_set_current_index_same_value_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.set_current_index(0); // already 0, should not emit signal
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    // ── 4. Item count ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_item_count() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.count(), 0);
        tb.add_item("A".to_string(), None);
        assert_eq!(tb.count(), 1);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        assert_eq!(tb.count(), 3);
    }

    // ── 5. Removing items ─────────────────────────────────────────────────

    #[test]
    fn toolbox_remove_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), Some(widget_id_1()));
        tb.add_item("C".to_string(), None);

        tb.remove_item(1);
        assert_eq!(tb.count(), 2);
        // The widget should be removed from children
        assert!(!tb.children().contains(&widget_id_1()), "child widget should be removed");
    }

    #[test]
    fn toolbox_remove_item_out_of_bounds_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.remove_item(5); // out of bounds
        assert_eq!(tb.count(), 1);
    }

    #[test]
    fn toolbox_remove_item_adjusts_current_index() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        tb.set_current_index(2);

        // Remove item at index 0 (< current_index), current decrements to 1
        tb.remove_item(0);
        assert_eq!(tb.selected_index().unwrap_or(0), 1);

        // Removing last item leaves empty -> current_index resets to 0
        tb.remove_item(1);
        tb.remove_item(0);
        assert_eq!(tb.count(), 0);
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn toolbox_remove_item_above_current_index_does_not_change_it() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(0);
        // Remove item at index 1 (> current_index 0), current stays 0
        tb.remove_item(1);
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    // ── 6. Clear all items ────────────────────────────────────────────────

    #[test]
    fn toolbox_clear_all_items() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), Some(widget_id_1()));
        tb.add_item("B".to_string(), Some(widget_id_2()));
        tb.add_item("C".to_string(), Some(widget_id_3()));

        // Remove all items one by one (no dedicated clear() method)
        tb.remove_item(0);
        tb.remove_item(0);
        tb.remove_item(0);

        assert_eq!(tb.count(), 0);
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
        assert!(tb.children().is_empty(), "all child widgets should be removed");
        assert_eq!(tb.current_widget(), None);
    }

    // ── 7. Setting/getting item text/tooltip ──────────────────────────────

    #[test]
    fn toolbox_item_text_and_tooltip() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("Hello".to_string(), None);
        tb.add_item("World".to_string(), None);

        if let Some(item) = tb.item(0) {
            assert_eq!(item.text(), "Hello");
            assert_eq!(item.tooltip(), "");
        }

        // Modify item via item_mut
        if let Some(item) = tb.item_mut(1) {
            item.set_text("Modified".to_string());
            item.set_tooltip("Tooltip text".to_string());
        }

        if let Some(item) = tb.item(1) {
            assert_eq!(item.text(), "Modified");
            assert_eq!(item.tooltip(), "Tooltip text");
        }

        // Getting item at out-of-bounds index
        assert!(tb.item(5).is_none());
        assert!(tb.item_mut(5).is_none());
    }

    // ── 8. Geometry delegation ────────────────────────────────────────────

    #[test]
    fn toolbox_geometry_delegation() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.geometry(), Rect::new(0, 0, 200, 160));

        tb.set_geometry(Rect::new(10, 20, 300, 200));
        assert_eq!(tb.geometry(), Rect::new(10, 20, 300, 200));
        assert_eq!(tb.base().geometry(), tb.geometry());
    }

    #[test]
    fn toolbox_position_and_size() {
        let mut tb = ToolBox::new(Rect::new(5, 10, 200, 150));
        assert_eq!(tb.position(), Point::new(5, 10));
        assert_eq!(tb.size(), crate::core::Size::new(200, 150));

        tb.set_position(Point::new(20, 30));
        assert_eq!(tb.geometry(), Rect::new(20, 30, 200, 150));

        tb.set_size(crate::core::Size::new(300, 200));
        assert_eq!(tb.geometry(), Rect::new(20, 30, 300, 200));
    }

    // ── 9. Visibility ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_visibility() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert!(tb.is_visible());

        tb.hide();
        assert!(!tb.is_visible());

        tb.show();
        assert!(tb.is_visible());

        tb.set_visible(false);
        assert!(!tb.is_visible());

        tb.set_visible(true);
        assert!(tb.is_visible());
    }

    #[test]
    fn toolbox_enabled_state() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert!(tb.is_enabled());

        tb.set_enabled(false);
        assert!(!tb.is_enabled());

        tb.set_enabled(true);
        assert!(tb.is_enabled());
    }

    // ── 10. ID / Kind ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_kind() {
        let tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.kind(), WidgetKind::Toolbox);
    }

    #[test]
    fn toolbox_id_is_unique() {
        let tb1 = ToolBox::new(Rect::new(0, 0, 100, 100));
        let tb2 = ToolBox::new(Rect::new(0, 0, 100, 100));
        assert_ne!(tb1.id(), tb2.id(), "each ToolBox must have a unique id");
    }

    // ── 11. SVG draw output ───────────────────────────────────────────────

    #[test]
    fn toolbox_draw_produces_svg_output() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 300, 160));
        tb.add_item("Item1".to_string(), None);
        tb.add_item("Item2".to_string(), None);
        let svg = render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"), "SVG must start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG must end with </svg>");
        assert!(svg.contains("width=\"300\""), "SVG must contain correct width");
        assert!(svg.contains("height=\"160\""), "SVG must contain correct height");
        assert!(svg.contains("fill="), "SVG should contain fill attributes");
        assert!(svg.len() > 100, "SVG output should be substantial");
    }

    /// An embedded icon's tile colour is derived from the style, not a hardcoded literal.
    ///
    /// # The defect this pins (issue §9 item #7)
    ///
    /// `ToolBox`'s icon tile used two fixed RGB literals, so an embedded icon ignored the active
    /// theme while every other pixel of the item honoured it. `icon_tile_color` now takes the ink and
    /// surface from the resolved style; this asserts the two observable properties: the selected tile
    /// is exactly the ink, and an unselected tile is a *different*, surface-blended colour (so it is
    /// not the same literal in disguise).
    ///
    #[test]
    fn an_embedded_icon_tile_is_derived_from_the_style() {
        let ink = Color::rgb(10, 20, 30);
        let surface = Color::rgb(250, 250, 250);

        assert_eq!(
            ToolBox::icon_tile_color(ink, surface, true),
            ink,
            "the selected tile must use the style ink directly"
        );
        let unselected = ToolBox::icon_tile_color(ink, surface, false);
        assert_ne!(unselected, ink, "an unselected tile must be a distinct, blended colour");
        assert_ne!(
            unselected,
            Color::rgb(180, 180, 200),
            "the tile must not be the old hardcoded literal"
        );
        // Blending toward a light surface must lighten the ink, not darken it.
        assert!(
            unselected.r > ink.r && unselected.g > ink.g && unselected.b > ink.b,
            "blending toward a light surface must lighten the tile: ink={ink:?} tile={unselected:?}"
        );
    }

    #[test]
    fn toolbox_draw_empty_produces_svg() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 100, 50));
        let svg = render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 50);
    }

    // ── 12. Signal accessors ──────────────────────────────────────────────

    #[test]
    fn toolbox_current_changed_signal_emits_on_set() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);

        let emitted = Arc::new(AtomicUsize::new(0));
        let e = emitted.clone();
        tb.current_changed.connect(move |idx: Arc<usize>| {
            e.store(*idx, Ordering::SeqCst);
        });

        tb.set_current_index(1);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);

        tb.set_current_index(2);
        assert_eq!(emitted.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn toolbox_current_changed_does_not_emit_for_same_value() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);

        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        tb.current_changed.connect(move |_: Arc<usize>| {
            h.fetch_add(1, Ordering::SeqCst);
        });

        // Set to same index (0) — should not emit
        tb.set_current_index(0);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    // ── 13. Mouse click selects item ──────────────────────────────────────

    #[test]
    fn toolbox_mouse_click_selects_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("First".to_string(), None);
        tb.add_item("Second".to_string(), None);

        assert_eq!(tb.selected_index().unwrap_or(0), 0);

        // Default orientation is Vertical; item_rect(1) has y = 0 + 32*1 = 32
        // Click on item 1 at (10, 40) which is within vertical item 1: (0, 32, 200, 32)
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 1, "clicking item should select it");
    }

    #[test]
    fn toolbox_mouse_click_on_disabled_item_does_not_select() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        // Disable item 1
        if let Some(item) = tb.item_mut(1) {
            item.set_enabled(false);
        }

        assert_eq!(tb.selected_index().unwrap_or(0), 0);
        // Click on item 1 (y=32), should not select because item is disabled
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1, modifiers: 0 });
        assert_eq!(
            tb.selected_index().unwrap_or(0),
            0,
            "clicking disabled item should not change current"
        );
    }

    #[test]
    fn toolbox_mouse_click_on_empty_region_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        assert_eq!(tb.selected_index().unwrap_or(0), 0);

        // Click far below items (y=200, but geometry only goes to 160)
        // Since item_at_position returns None for out-of-bounds, no change
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 200), button: 1, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    // ── 14. Disabled state blocks events ──────────────────────────────────

    #[test]
    fn toolbox_disabled_state_blocks_mouse_events() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(0);

        // Disable the entire toolbox
        tb.set_enabled(false);

        // Try clicking on item 1
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1, modifiers: 0 });
        // current_index should remain 0 because event handling returns early when disabled
        assert_eq!(
            tb.selected_index().unwrap_or(0),
            0,
            "disabled toolbox should not process mouse events"
        );
    }

    #[test]
    fn toolbox_disabled_re_enable_processes_events() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        tb.set_enabled(false);
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 0);

        // Re-enable and click again
        tb.set_enabled(true);
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1, modifiers: 0 });
        assert_eq!(
            tb.selected_index().unwrap_or(0),
            1,
            "after re-enable, mouse clicks should work"
        );
    }

    // ── 16. Keyboard navigation (vertical) ────────────────────────────────

    #[test]
    fn toolbox_key_down_selects_next_item_vertical() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        assert_eq!(tb.selected_index().unwrap_or(0), 0);

        tb.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 1);

        tb.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 2);
    }

    #[test]
    fn toolbox_key_up_selects_prev_item_vertical() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(1);

        tb.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn toolbox_key_navigation_stops_at_boundaries() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);

        // Already at first item, Up should stay
        tb.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 0);

        // Already at last item, Down should stay
        tb.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn toolbox_key_navigation_horizontal() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 400, 100));
        tb.set_orientation(Orientation::Horizontal);
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);

        assert_eq!(tb.selected_index().unwrap_or(0), 0);

        // Right arrow advances
        tb.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 1);

        // Left arrow goes back
        tb.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
    }

    // ── 17. Clear method ──────────────────────────────────────────────────

    #[test]
    fn toolbox_clear_removes_all_items() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), Some(widget_id_1()));
        tb.add_item("C".to_string(), None);
        tb.set_current_index(2);

        tb.clear();

        assert_eq!(tb.count(), 0);
        assert_eq!(tb.selected_index().unwrap_or(0), 0);
        assert!(tb.children().is_empty(), "child widgets should be removed");
    }

    // ── 18. Overflow: the page's minimum and the tab strip's containment ──────

    /// The defect: `content_rect` was `rect.height - item_height * items.len()`, floored at zero,
    /// so four 32 px tabs in the 120 px census control left the page **0** px tall.
    ///
    /// The page is observed through the drawing, because that is what the defect was visible as:
    /// the page rectangle the control emits must have a positive height, and it must be
    /// *distinct* from the tab strip rather than collapsed onto it.
    #[test]
    fn toolbox_page_keeps_an_extent_when_the_tabs_would_consume_it() {
        // Enough tabs to swallow the whole control at the fixed tab height.
        for count in [4usize, 6, 20] {
            let mut tb = ToolBox::new(Rect::new(0, 0, 240, 120));
            for i in 0..count {
                tb.add_item(format!("Item {i}"), None);
            }
            let svg = render_to_svg(&mut tb);
            // The page is the full-width rectangle below the tab strip; the strip's own rects are
            // only as tall as one tab. The tallest full-width rectangle is the page.
            let page_height = svg
                .lines()
                .filter(|l| l.contains("<rect"))
                .filter_map(|l| {
                    let n: Vec<i32> = l
                        .split(|c: char| !(c.is_ascii_digit() || c == '-'))
                        .filter(|s| !s.is_empty())
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    (n.len() >= 4).then(|| (n[0], n[1], n[2], n[3]))
                })
                .filter(|(_, _, w, _)| *w == 240)
                .map(|(_, _, _, h)| h)
                .max()
                .unwrap_or(0);
            assert!(
                page_height > 0,
                "{count} tabs on a 120px box left the page {page_height}px tall"
            );
        }
    }

    /// The defect: `item_rect` was unclamped, so item `n` was placed at `item_height * n` with no
    /// reference to the control's extent and later items were painted outside it.
    #[test]
    fn toolbox_no_item_rect_escapes_the_control() {
        let geometry = Rect::new(0, 0, 240, 120);
        for count in [1usize, 3, 4, 6, 20] {
            let mut tb = ToolBox::new(geometry);
            for i in 0..count {
                tb.add_item(format!("Item {i}"), None);
            }
            for i in 0..count {
                if let Some(r) = tb.item_rect(i) {
                    assert!(
                        r.y >= geometry.y && r.bottom() <= geometry.bottom(),
                        "item {i} of {count} at {r:?} escapes {geometry:?}"
                    );
                    assert!(r.width > 0 && r.height > 0, "item {i} of {count} is empty: {r:?}");
                }
            }
        }
    }

    /// The overflow rule must be *driven*, not just computed: with more tabs than fit, the strip
    /// can scroll, and selecting an off-screen tab brings its tab into view.
    #[test]
    fn toolbox_overflow_scrolls_and_the_page_follows_the_middle_tab() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 240, 120));
        for i in 0..10 {
            tb.add_item(format!("Item {i}"), None);
        }
        assert!(tb.max_scroll() > 0, "ten 32px tabs cannot fit a 24px page in 120px");

        tb.set_current_index(9);
        assert!(tb.scroll_offset() > 0, "selecting a scrolled-off tab must bring it into view");
        assert!(tb.item_rect(9).is_some(), "the selected tab must be on screen after scrolling");

        // An out-of-range scroll request is clamped rather than leaving the strip blank.
        tb.set_scroll_offset(usize::MAX);
        assert_eq!(tb.scroll_offset(), tb.max_scroll());
        tb.set_scroll_offset(0);
        assert_eq!(tb.scroll_offset(), 0);
    }

    /// The rendered SVG must contain nothing outside the control, at any tab count.
    #[test]
    fn toolbox_draw_never_paints_outside_the_control() {
        for count in [1usize, 4, 6, 20] {
            let mut tb = ToolBox::new(Rect::new(0, 0, 240, 120));
            for i in 0..count {
                tb.add_item(format!("Item {i}"), None);
            }
            let svg = render_to_svg(&mut tb);
            for line in svg.lines().filter(|l| l.contains("<rect") || l.contains("<line")) {
                let nums: Vec<i32> = line
                    .split(|c: char| !(c.is_ascii_digit() || c == '-'))
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                assert!(
                    nums.iter().all(|v| *v >= 0),
                    "{count} items painted outside the control: {line}"
                );
            }
        }
    }

    /// An index past the end appends instead of panicking.
    ///
    /// `insert_item` passed the caller's index straight to `Vec::insert`, which panics
    /// when it exceeds the length. `tab_bar` and `pie_menu` clamp, so a caller that
    /// asked to insert past the end crashed here and appended everywhere else.
    #[test]
    fn toolbox_insert_past_the_end_appends() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 240, 120));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.insert_item(9, "C".to_string(), None);
        assert_eq!(tb.count(), 3);
        assert_eq!(tb.item(2).map(|i| i.text()), Some("C"), "past the end lands last");
    }

    /// The row height is a settable metric, and a smooth scroll eases toward the target.
    #[test]
    fn item_height_and_smooth_scroll_are_wired() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 100));
        for index in 0..6 {
            tb.add_item(format!("Item {index}"), None);
        }
        tb.set_item_height(48);
        assert_eq!(tb.item_height(), 48);
        assert_eq!(tb.item_rect(1).map(|rect| rect.height), Some(48));
        tb.set_smooth_scroll(true);
        tb.set_scroll_offset(1);
        assert!(tb.is_animating());
        assert!(tb.tick(60));
        assert!(!tb.tick(1000));
        assert!(!tb.is_animating());
    }

    /// C-28: the pointer path records the hovered item and exposes its tooltip.
    #[test]
    fn hovering_an_item_exposes_its_tooltip() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 300));
        tb.add_item("Group".to_string(), None);
        tb.item_mut(0).expect("item").tooltip = "First group".to_string();
        assert_eq!(tb.hovered_tooltip(), None);
        tb.handle_event(&crate::event::Event::MouseMove { pos: crate::core::Point::new(10, 10) });
        assert_eq!(tb.hovered_item(), Some(0));
        assert_eq!(tb.hovered_tooltip(), Some("First group"));
        tb.handle_event(&crate::event::Event::MouseLeave { pos: crate::core::Point::new(10, 10) });
        assert_eq!(tb.hovered_item(), None);
    }

    /// C-29: popup mode presents the page in a floating rectangle, not the page band.
    #[test]
    fn popup_mode_uses_a_floating_rect() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 300));
        tb.add_item("Group".to_string(), None);
        tb.add_item("Other".to_string(), None);
        tb.set_mode(ToolBoxMode::Popup);
        assert!(tb.set_item_expanded(0, true));
        let content = tb.content_rect();
        let popup = tb.popup_rect(content).expect("an open popup has a rect");
        assert_eq!(popup.height, 180, "the floating layer has its own extent");
        assert!(popup.y >= content.y, "the layer stays inside the content box");
        assert_ne!(popup, content, "it is a presentation of its own, not the page band");
        assert_eq!(tb.page_rects(content), vec![(0, popup)]);
    }

    /// C-019: the hint is the current page plus the tab strip on the main axis.
    #[test]
    fn the_size_hint_follows_the_current_page() {
        use crate::widget::base_widgets::label::Label;

        let label = Label::new("page".to_string(), Rect::new(0, 0, 120, 80));
        let expected = label.size_hint();
        let id =
            crate::widget::runtime::register(Box::new(label)).expect("the test control mounts");
        let mut tool_box = ToolBox::new(Rect::new(0, 0, 200, 250));
        tool_box.add_item("page".to_string(), Some(id));
        let hint = tool_box.size_hint();
        match tool_box.orientation() {
            Orientation::Horizontal => {
                assert_eq!(hint.width, expected.width + ITEM_WIDTH);
                assert_eq!(hint.height, expected.height.max(ITEM_HEIGHT));
            }
            Orientation::Vertical => {
                assert_eq!(hint.width, expected.width.max(ITEM_WIDTH));
                assert_eq!(hint.height, expected.height + ITEM_HEIGHT);
            }
        }
        let _ = crate::widget::runtime::unregister(id);
    }
}
