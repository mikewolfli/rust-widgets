// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool bar widget.
//!
//! # The strip is assembled from its items
//!
//! BLUE22 §B.8 lists this control's defect as "item positions from a step literal". The first fix
//! handed every item to a [`FlexLayout`](crate::layout::FlexLayout) and created a temporary label
//! widget per item just to learn its box. That made every hit test and every paint O(n) widget
//! creations plus a layout pass — measured as the "ToolBar degrades to O(n²) per frame" item in
//! the 2026-10-21 scan. The items all share one fixed advance, so the same boxes are now computed
//! directly: `item_bands` is one pass of integer additions, and nothing is mounted to measure it.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::access::tool_bar_orientation_to_str;
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_toolbar_orientation};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The width of the drag grip at the strip's trailing end: 12 px.
///
/// The grip is a hit target, not just ink: a press inside it starts a drag rather than
/// activating the item underneath, so it has to be wide enough to acquire on a touch screen
/// while still leaving nearly the whole strip for actions.
const TOOLBAR_DRAG_GRIP_WIDTH: u32 = 12;

/// Orientation of a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolBarOrientation {
    /// Items laid out left to right.
    Horizontal,
    /// Items laid out top to bottom.
    Vertical,
}

/// Where a toolbar may be docked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolBarArea {
    /// Along the top edge of the window.
    Top,
    /// Along the bottom edge.
    Bottom,
    /// Along the left edge.
    Left,
    /// Along the right edge.
    Right,
}

/// Every docking area, in the order a toolbar usually offers them.
pub const ALL_TOOL_BAR_AREAS: [ToolBarArea; 4] =
    [ToolBarArea::Top, ToolBarArea::Bottom, ToolBarArea::Left, ToolBarArea::Right];

/// How an item's label and icon are arranged inside its button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolBarButtonStyle {
    /// The label only. This is the historical rendering and stays the default.
    #[default]
    TextOnly,
    /// The icon only; the label is not painted.
    IconOnly,
    /// The icon followed by the label on one line.
    TextBesideIcon,
    /// The icon above the label.
    TextUnderIcon,
}
/// A button entry in the toolbar.
#[derive(Debug, Clone)]
pub struct ToolBarItem {
    id: String,
    text: String,
    tooltip: String,
    checkable: bool,
    checked: bool,
    enabled: bool,
    separator: bool,
    /// Optional icon stand-in text, painted according to the bar's button style.
    icon: Option<String>,
    /// Per-item square size override, in logical pixels.
    icon_size: Option<u32>,
}
impl ToolBarItem {
    /// Creates an enabled, unchecked, non-separator item.
    ///
    /// `id` identifies the item in the `action_triggered` signal; `text` is what
    /// is painted on the button. The tooltip starts empty.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            tooltip: String::new(),
            checkable: false,
            checked: false,
            enabled: true,
            separator: false,
            icon: None,
            icon_size: None,
        }
    }
    /// Creates a separator: an item with an empty id and text and
    /// [`Self::is_separator`] true.
    ///
    /// Separators occupy a fixed gap, are never hit-testable as actions, and are
    /// skipped when a press is dispatched.
    pub fn separator() -> Self {
        let mut t = Self::new("", "");
        t.set_separator(true);
        t
    }

    // --- Accessors ---

    /// The item's identity, emitted through `action_triggered` when it is chosen.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Replaces the item's identity. Changing it changes what a later activation
    /// emits, so any handler matching on the old id stops firing.
    pub fn set_id(&mut self, id: impl Into<String>) {
        self.id = id.into();
    }

    /// The label painted on the item.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the label.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// The item's hover tooltip, or `""` when none was set.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Sets the hover tooltip. Note that this widget only stores it: nothing in
    /// `ToolBar`'s own drawing shows a tooltip.
    pub fn set_tooltip(&mut self, tooltip: impl Into<String>) {
        self.tooltip = tooltip.into();
    }

    /// Whether the item toggles its checked state when activated.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Makes the item toggle on activation, or turns that off. Turning it off
    /// leaves `checked` as it is.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }

    /// Whether the item is currently checked. Only meaningful when
    /// [`Self::is_checkable`] is true — a non-checkable item keeps whatever
    /// value was set here but nothing will ever toggle it.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked state directly, regardless of [`Self::is_checkable`].
    /// Use `ToolBar::set_item_checked`, which respects checkability, if the
    /// caller is not sure which kind of item it holds.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Whether the item can be activated. A disabled item still draws (in grey)
    /// but ignores presses.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the item.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether this item is a visual divider rather than an action.
    pub fn is_separator(&self) -> bool {
        self.separator
    }

    /// Makes this item a separator (`true`) or an ordinary action (`false`).
    ///
    /// A separator is drawn as a rule and occupies a fixed gap, but setting this
    /// does not clear the id: a press can never reach a separator through
    /// `hit_item`, so nothing is emitted either way.
    pub fn set_separator(&mut self, separator: bool) {
        self.separator = separator;
    }

    /// The item's icon stand-in text, if any.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// Sets the icon stand-in text. Like a dropdown item's icon this is a short
    /// character run the painter draws, not image data.
    pub fn set_icon(&mut self, icon: Option<String>) {
        self.icon = icon;
    }

    /// Sets the icon and returns `self`, for chaining onto [`ToolBarItem::new`].
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// This item's own square size, overriding the bar's `icon_size` when set.
    pub fn icon_size(&self) -> Option<u32> {
        self.icon_size
    }

    /// Sets this item's square size, or clears the override with `None`.
    pub fn set_icon_size(&mut self, size: Option<u32>) {
        self.icon_size = size;
    }
}
/// Toolbar widget.
pub struct ToolBar {
    base: BaseWidget,
    orientation: ToolBarOrientation,
    icon_size: f32,
    movable: bool,
    floatable: bool,
    /// The pointer offset from the control's origin while a drag is in flight, or `None`
    /// when no drag is active. Storing the offset rather than the press point means every
    /// move can recompute the geometry from the live pointer without accumulating rounding
    /// error.
    drag_state: Option<Point>,
    /// The strip's band at the moment the drag started.
    ///
    /// A release outside this rectangle is what "dragged out of the dock band" means; the
    /// current band travels with the control and so cannot answer that question on its own.
    drag_dock_band: Option<Rect>,
    /// The areas this toolbar may be docked to.
    ///
    /// Defaults to every area, which is the pre-wiring behaviour: a host that never
    /// constrains the bar sees the same dock decisions.
    allowed_areas: Vec<ToolBarArea>,
    /// How each item's icon and label are arranged.
    button_style: ToolBarButtonStyle,
    /// The first item the strip paints when the items overflow its width.
    overflow_offset: usize,
    /// Whether the customize affordance is painted at the trailing edge.
    editable: bool,
    /// Emitted when the customize affordance is pressed.
    pub customize_requested: GenericSignal,
    /// Whether the toolbar currently lives in its own top-level window. See
    /// [`ToolBar::is_top_level`] for why this is not a `BaseWidget` flag.
    top_level: bool,
    items: Vec<ToolBarItem>,
    hovered_index: Option<usize>,
    /// Emitted with the id of the item that was activated.
    ///
    /// Fires on primary-button press over an enabled, non-separator item. For a
    /// checkable item the checked flag has already been toggled by the time this
    /// is emitted.
    pub action_triggered: Signal1<String>,
    /// Emitted by [`ToolBar::set_orientation`] with `true` for horizontal and
    /// `false` for vertical. Not emitted when the orientation is unchanged.
    pub orientation_changed: Signal1<bool>,
    /// Emitted when the toolbar is docked or floated, by
    /// [`ToolBar::set_top_level`]. [`ToolBar::is_floatable`] separately records the
    /// permission to float.
    pub top_level_changed: Signal1<bool>,
    /// Emitted when the toolbar's visibility changes, by [`ToolBar::set_visible`].
    pub visibility_changed: Signal1<bool>,
    /// Emitted when the toolbar is docked or floated, in parallel with
    /// [`Self::top_level_changed`].
    ///
    /// A host that re-lays out its window around a floating toolbar subscribes here; the signal
    /// carries the new floating state so the re-layout does not have to read the control back.
    pub float_changed: Signal1<bool>,
}
impl ToolBar {
    /// Creates a horizontal toolbar with a default icon size of 24 pixels, and
    /// with both movable and floatable permitted.
    ///
    /// Starts with no items; add them with [`ToolBar::add_action`],
    /// [`ToolBar::add_separator`] or by pushing to [`ToolBar::items`].
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolBar, geometry, "ToolBar"),
            orientation: ToolBarOrientation::Horizontal,
            icon_size: 24.0,
            movable: true,
            floatable: true,
            drag_state: None,
            drag_dock_band: None,
            allowed_areas: ALL_TOOL_BAR_AREAS.to_vec(),
            button_style: ToolBarButtonStyle::TextOnly,
            overflow_offset: 0,
            editable: false,
            customize_requested: GenericSignal::new(),
            top_level: false,
            items: Vec::new(),
            hovered_index: None,
            action_triggered: Signal1::new(),
            orientation_changed: Signal1::new(),
            top_level_changed: Signal1::new(),
            visibility_changed: Signal1::new(),
            float_changed: Signal1::new(),
        }
    }
    /// The direction items are laid out in.
    pub fn orientation(&self) -> ToolBarOrientation {
        self.orientation
    }
    /// The icon size in **pixels**.
    ///
    /// It sets each item's square size; an item may override it through
    /// [`ToolBarItem::set_icon_size`]. Labels are painted with the resolved theme font.
    pub fn icon_size(&self) -> f32 {
        self.icon_size
    }
    /// Whether the toolbar is allowed to be dragged to another dock position.
    pub fn is_movable(&self) -> bool {
        self.movable
    }
    /// Whether the toolbar is allowed to be torn off into a floating window.
    ///
    /// This is the *permission*; the current docking state is
    /// [`ToolBar::is_top_level`], reached through [`ToolBar::set_top_level`].
    pub fn is_floatable(&self) -> bool {
        self.floatable
    }
    /// The current items, in draw order.
    pub fn items(&self) -> &[ToolBarItem] {
        &self.items
    }
    /// Sets the layout direction and repaints.
    ///
    /// Emits `orientation_changed` with `true` for horizontal, and only when the
    /// value actually changes.
    pub fn set_orientation(&mut self, o: ToolBarOrientation) {
        let changed = self.orientation != o;
        self.orientation = o;
        if changed {
            self.orientation_changed.emit(o == ToolBarOrientation::Horizontal);
        }
        self.base.request_redraw();
    }
    /// Sets the icon size in **pixels** and repaints.
    ///
    /// Values below 8 are raised to 8, which is the smallest size at which an
    /// item remains clickable rather than collapsing to a sliver.
    pub fn set_icon_size(&mut self, size: f32) {
        let size = size.max(8.0);
        if self.icon_size == size {
            return;
        }
        self.icon_size = size;
        self.clamp_overflow();
        self.base.request_redraw();
    }
    /// Allows or forbids dragging the toolbar; repaints either way.
    pub fn set_movable(&mut self, v: bool) {
        if self.movable == v {
            return;
        }
        self.movable = v;
        if !v {
            // A drag that loses its permission must end rather than continue with the old
            // anchor: the next move would otherwise keep moving a toolbar the host has just
            // pinned in place.
            self.drag_state = None;
            self.drag_dock_band = None;
        }
        self.base.request_redraw();
    }
    /// Allows or forbids tearing the toolbar off; repaints either way.
    ///
    /// Forbidding it also docks a currently-floating toolbar, so the permission and
    /// the state cannot disagree.
    pub fn set_floatable(&mut self, v: bool) {
        if self.floatable == v {
            return;
        }
        self.floatable = v;
        if !v {
            // Docking through the setter keeps `top_level_changed` truthful: the
            // state really did change, and a host tracking it must hear about it.
            self.set_top_level(false);
        }
        self.base.request_redraw();
    }
    /// Whether the toolbar is currently shown.
    pub fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    /// Shows or hides the toolbar and repaints.
    ///
    /// Emits `visibility_changed`, and only on a real change: a caller using the
    /// signal to relayout a surrounding window must not do that work for a write
    /// that changed nothing.
    ///
    /// This is the wrapper that makes the signal reachable. `BaseWidget` owns the
    /// flag and knows nothing about toolbar signals, so without this method
    /// `visibility_changed` could only be emitted from inside `BaseWidget` — where
    /// the signal does not exist.
    pub fn set_visible(&mut self, visible: bool) {
        if self.base.is_visible() == visible {
            return;
        }
        if visible {
            self.base.show();
        } else {
            self.base.hide();
        }
        self.base.request_redraw();
        self.visibility_changed.emit(visible);
    }
    /// Whether the toolbar is currently docked into a window (`false`) or hosted in
    /// its own top-level window (`true`).
    ///
    /// This is the toolbar's own state, not a `BaseWidget` flag: whether a control
    /// has been torn off into a separate window is a toolbar concept, and inventing
    /// a base-level flag for it would put a toolbar-only idea in every control.
    pub fn is_top_level(&self) -> bool {
        self.top_level
    }
    /// Moves the toolbar between docked and floating, and repaints.
    ///
    /// Emits `top_level_changed`, and only on a real change. As with
    /// [`Self::set_visible`], this exists so the signal has an emitter at all:
    /// `is_floatable` records the *permission* to float, which is a different
    /// question from whether the toolbar *is* floating, and only the latter is what
    /// the signal reports.
    ///
    /// Returns `false` without emitting when the toolbar is not floatable, so a
    /// caller cannot float a toolbar the host forbade. The refusal is silent in the
    /// signal stream but visible in the return value, which is where a caller can
    /// act on it.
    pub fn set_top_level(&mut self, top_level: bool) -> bool {
        if top_level && !self.floatable {
            return false;
        }
        if self.top_level == top_level {
            return false;
        }
        self.top_level = top_level;
        self.base.request_redraw();
        self.top_level_changed.emit(top_level);
        // The float state is the fact a host needs to re-flow its window around this strip;
        // `top_level_changed` already reports it, and `float_changed` is the host-facing name
        // for the same change. Emitting both keeps a listener from having to know which one
        // this control happens to expose.
        self.float_changed.emit(top_level);
        true
    }
    /// Appends an action item and returns its index in [`ToolBar::items`].
    ///
    /// `id` is what a later `action_triggered` carries and `text` is the visible
    /// label. The item starts enabled and unchecked. The returned index is the
    /// item's position, so removing items later invalidates it.
    ///
    /// Repaints: the strip is painted from the item list this grows.
    pub fn add_action(&mut self, id: impl Into<String>, text: impl Into<String>) -> usize {
        let idx = self.items.len();
        self.items.push(ToolBarItem::new(id, text));
        self.clamp_overflow();
        self.base.request_redraw();
        idx
    }
    /// Appends a visual divider.
    ///
    /// Unlike [`ToolBar::add_action`] this returns nothing, so a caller that
    /// needs the separator's index must read `items().len() - 1`.
    ///
    /// Repaints, for the same reason as [`ToolBar::add_action`].
    pub fn add_separator(&mut self) {
        self.items.push(ToolBarItem::separator());
        self.clamp_overflow();
        self.base.request_redraw();
    }
    /// Removes every item, including separators. Invalidates all previously
    /// returned indices.
    ///
    /// Repaints: an empty strip is a visible change.
    pub fn clear(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.items.clear();
        self.overflow_offset = 0;
        self.base.request_redraw();
    }
    /// Enables or disables the item at `index`, and repaints.
    ///
    /// An out-of-range index is ignored rather than treated as an error; read
    /// [`ToolBar::item_enabled`] afterwards to confirm the change took effect.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.set_enabled(enabled);
        }
        self.base.request_redraw();
    }
    /// Returns enabled state for item at index.
    ///
    /// `None` when `index` is out of range — which is how a caller notices that
    /// [`ToolBar::set_item_enabled`] silently ignored its index.
    pub fn item_enabled(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_enabled())
    }
    /// Sets the checked state of the item at `index`, and repaints.
    ///
    /// Only takes effect on a checkable item; on any other item the call does
    /// nothing, silently. An out-of-range index is likewise ignored.
    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        if let Some(item) = self.items.get_mut(index) {
            if item.is_checkable() {
                item.set_checked(checked);
            }
        }
        self.base.request_redraw();
    }
    /// Returns checked state for item at index.
    ///
    /// `None` when `index` is out of range. For a non-checkable item this
    /// returns `Some(false)` (or whatever was set directly on the item) rather
    /// than `None`, so it does not tell the caller whether the item is togglable
    /// — check `items()[index].is_checkable()` for that.
    pub fn item_checked(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_checked())
    }

    /// The docking areas this toolbar accepts.
    pub fn allowed_areas(&self) -> &[ToolBarArea] {
        &self.allowed_areas
    }

    /// Restricts the docking areas.
    ///
    /// An empty list means "no docking allowed", which is a legitimate configuration for
    /// a floating-only palette. The list keeps the order the caller gave it, so a dock
    /// menu can present the host's preference order rather than a fixed one.
    pub fn set_allowed_areas(&mut self, areas: impl IntoIterator<Item = ToolBarArea>) {
        let next: Vec<ToolBarArea> = areas.into_iter().collect();
        if self.allowed_areas == next {
            return;
        }
        self.allowed_areas = next;
        self.base.request_redraw();
    }

    /// Whether `area` is among the accepted docking areas.
    pub fn allows_area(&self, area: ToolBarArea) -> bool {
        self.allowed_areas.contains(&area)
    }

    /// How each item arranges its icon and label.
    pub fn button_style(&self) -> ToolBarButtonStyle {
        self.button_style
    }

    /// Changes the item presentation. [`ToolBarButtonStyle::TextOnly`] is the default and
    /// paints exactly what the toolbar painted before this option existed.
    pub fn set_button_style(&mut self, style: ToolBarButtonStyle) {
        if self.button_style == style {
            return;
        }
        self.button_style = style;
        self.base.request_redraw();
    }

    /// The first item painted when the items overflow the strip.
    pub fn overflow_offset(&self) -> usize {
        self.overflow_offset
    }

    /// Whether some items are outside the strip's width.
    pub fn is_overflowing(&self) -> bool {
        self.visible_capacity_with(0) < self.items.len()
    }

    /// Whether the customize affordance is painted.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Shows or hides the customize affordance. It is off by default, so the default
    /// strip has no extra target.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        self.base.request_redraw();
    }

    /// How many leading items fit when `extra` more pixels are reserved at the end.
    fn visible_capacity_with(&self, extra: u32) -> usize {
        if self.items.is_empty() {
            return 0;
        }
        let band = self.band_rect();
        let horizontal = self.orientation == ToolBarOrientation::Horizontal;
        let main_len = if horizontal { band.width } else { band.height };
        let inset = dimensions::TOOLBAR_ITEM_INSET * 2;
        let mut available = main_len.saturating_sub(inset) as i32;
        let fixed = if self.movable { TOOLBAR_DRAG_GRIP_WIDTH } else { 0 }
            + if self.editable { 20 } else { 0 }
            + extra;
        available -= fixed as i32;
        let mut used = 0i32;
        let mut count = 0usize;
        for item in &self.items {
            let step = if item.is_separator() {
                dimensions::TOOLBAR_SPACING
            } else {
                item.icon_size().unwrap_or(self.icon_size as u32) + dimensions::TOOLBAR_SPACING
            } as i32;
            if used + step > available && count > 0 {
                break;
            }
            used += step;
            count += 1;
        }
        count.max(1)
    }

    /// The capacity the strip paints at, accounting for its own overflow button.
    fn effective_capacity(&self) -> usize {
        let plain = self.visible_capacity_with(0);
        if plain < self.items.len() {
            self.visible_capacity_with(20)
        } else {
            plain
        }
    }

    /// The visible item indices in draw order.
    #[allow(dead_code)]
    fn visible_range(&self) -> core::ops::Range<usize> {
        let capacity = self.effective_capacity();
        let start = self.overflow_offset.min(self.items.len());
        start..start.saturating_add(capacity).min(self.items.len())
    }

    /// Keeps the overflow offset inside the list after a mutation.
    fn clamp_overflow(&mut self) {
        let capacity = self.effective_capacity();
        let max_offset = self.items.len().saturating_sub(capacity);
        self.overflow_offset = self.overflow_offset.min(max_offset);
    }

    /// The overflow button's box, or `None` when everything fits.
    pub fn overflow_button_rect(&self) -> Option<Rect> {
        if !self.is_overflowing() {
            return None;
        }
        let band = self.band_rect();
        match self.orientation {
            ToolBarOrientation::Horizontal => {
                let grip = if self.movable { TOOLBAR_DRAG_GRIP_WIDTH } else { 0 };
                let right = band.x + band.width as i32 - grip as i32 - 20;
                Some(Rect::new(right, band.y, 20, band.height))
            }
            ToolBarOrientation::Vertical => {
                let grip = if self.movable { TOOLBAR_DRAG_GRIP_WIDTH } else { 0 };
                let bottom = band.y + band.height as i32 - grip as i32 - 20;
                Some(Rect::new(band.x, bottom, band.width, 20))
            }
        }
    }

    /// The customize button's box, or `None` when the strip is not editable.
    pub fn customize_button_rect(&self) -> Option<Rect> {
        if !self.editable {
            return None;
        }
        let band = self.band_rect();
        let overflow = self.is_overflowing() as u32 * 20;
        let grip = if self.movable { TOOLBAR_DRAG_GRIP_WIDTH } else { 0 };
        match self.orientation {
            ToolBarOrientation::Horizontal => {
                let right = band.x + band.width as i32 - grip as i32 - overflow as i32 - 20;
                Some(Rect::new(right, band.y, 20, band.height))
            }
            ToolBarOrientation::Vertical => {
                let bottom = band.y + band.height as i32 - grip as i32 - overflow as i32 - 20;
                Some(Rect::new(band.x, bottom, band.width, 20))
            }
        }
    }

    /// The strip the control actually paints: a full-width band
    /// [`dimensions::TOOLBAR_HEIGHT`] tall, centred in the area the control was given.
    ///
    /// # Why the strip is not the control's rectangle
    ///
    /// A toolbar is a **row of items of a fixed height**; it is not a panel. Painting
    /// `geometry()` made the 240x120 census cell a 120 px-tall strip — a slab four times the
    /// height of the control's own `size_hint`, which is a rectangle shaped like a toolbar
    /// rather than a toolbar. [`ControlMetrics::full_width_band`] keeps the full width (a
    /// toolbar *is* as wide as its window) and takes the strip's own height, so the ink and
    /// the reported size agree. This one band is what [`ToolBar::item_bands`] — and therefore
    /// the hit test — and `draw` both read.
    fn band_rect(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TOOLBAR_HEIGHT)
    }

    /// The drag grip: a [`TOOLBAR_DRAG_GRIP_WIDTH`]-wide strip at the band's trailing end.
    ///
    /// # Why the grip is derived from the band, not from the control's rectangle
    ///
    /// The band is what is painted and what every item box is measured against (see
    /// [`Self::band_rect`]). Deriving the grip from anything else would put the affordance in
    /// a place the strip does not cover on a short control, or leave it floating below a tall
    /// one. The trailing end follows the orientation: the right edge of a horizontal strip,
    /// the bottom edge of a vertical one.
    fn drag_band(&self) -> Rect {
        let band = self.band_rect();
        match self.orientation {
            ToolBarOrientation::Horizontal => {
                let width = TOOLBAR_DRAG_GRIP_WIDTH.min(band.width);
                Rect::new(band.x + band.width as i32 - width as i32, band.y, width, band.height)
            }
            ToolBarOrientation::Vertical => {
                let height = TOOLBAR_DRAG_GRIP_WIDTH.min(band.height);
                Rect::new(band.x, band.y + band.height as i32 - height as i32, band.width, height)
            }
        }
    }

    /// Ends an in-flight drag, floating the toolbar when it was released away from its dock.
    fn finish_drag(&mut self, pos: Point) {
        self.drag_state = None;
        let Some(dock_band) = self.drag_dock_band.take() else {
            return;
        };
        // "Out of the dock band" is the release point, not the moved rectangle: the control
        // follows the pointer, so its own band always sits under the release. The band captured
        // at press is the fixed reference that makes the phrase meaningful.
        if self.floatable && !dock_band.contains_point(pos) {
            let _ = self.set_top_level(true);
        }
    }

    /// Every item's box, placed by the one advance every item shares.
    ///
    /// # Why this is arithmetic rather than a layout pass
    ///
    /// A toolbar item has no intrinsic size to negotiate: a button is `icon_size + spacing` along
    /// the strip, a separator is `spacing`, and the cross axis is the band inset at both ends. The
    /// previous form built a `CompositeBuilder`, a `FlexLayout`, and a temporary `label` widget per
    /// item on **every** call — and this method is called by the hit test and by `draw`, so a
    /// 40-item toolbar created 40 widgets and ran a full layout pass per pointer event and per
    /// frame. The boxes are the same ones the layout produced (the tests below pin the relation),
    /// so computing them directly removes the work without changing a pixel.
    ///
    /// The callers read this list once per pass: `draw` and `hit_item` each walk it in place
    /// rather than asking a per-index helper, which used to rebuild the whole list per index and
    /// made both paths O(N²) in the item count (REND-01).
    fn item_bands(&self) -> Vec<Rect> {
        let band = self.band_rect();
        if self.items.is_empty() {
            return Vec::new();
        }
        let horizontal = self.orientation == ToolBarOrientation::Horizontal;
        let inset = dimensions::TOOLBAR_ITEM_INSET as i32;
        let cross = if horizontal {
            band.height.saturating_sub(dimensions::TOOLBAR_ITEM_INSET * 2)
        } else {
            band.width.saturating_sub(dimensions::TOOLBAR_ITEM_INSET * 2)
        };
        // Items scrolled off the leading edge keep a zero-size band, so the list's
        // indices still address the same item while no hit test can reach a hidden one.
        let start = self.overflow_offset.min(self.items.len());
        let mut placed = vec![Rect::new(0, 0, 0, 0); start];
        // The offset runs along the strip's main axis; the cross-axis position is the band's origin
        // plus the same inset the layout used as padding.
        let mut offset = if horizontal { band.x } else { band.y } + inset;
        for item in &self.items[start..] {
            let step = if item.is_separator() {
                dimensions::TOOLBAR_SPACING
            } else {
                item.icon_size().unwrap_or(self.icon_size as u32) + dimensions::TOOLBAR_SPACING
            };
            let rect = if horizontal {
                let rect = Rect::new(offset, band.y + inset, step, cross);
                offset += step as i32;
                rect
            } else {
                let rect = Rect::new(band.x + inset, offset, cross, step);
                offset += step as i32;
                rect
            };
            placed.push(rect);
        }
        placed
    }
    fn hit_item(&self, pos: Point) -> Option<usize> {
        // `Rect::contains_point`, not a hand-written comparison: the crate's
        // convention is an inclusive origin with an **exclusive** far edge (see
        // `geometry.rs`), and `draw` paints these rects left to right so a later
        // sibling overpaints the boundary column. Including it in the hit-test made
        // that one pixel resolve to the *earlier* item — the opposite of what is drawn
        // there.
        //
        // One `item_bands` call for the whole probe: a per-index helper used to recompute every
        // box for every index, so a pointer move over an N-item strip was O(N²) (REND-01).
        self.item_bands().iter().position(|band| band.contains_point(pos))
    }
}
impl Widget for ToolBar {
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
        crate::core::Size::new(400, dimensions::TOOLBAR_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "action_triggered" => {
                Some(EventSignalRef::mapped("action_triggered", &self.action_triggered, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "orientation_changed" => Some(EventSignalRef::mapped(
                "orientation_changed",
                &self.orientation_changed,
                |v| CapabilityValue::Bool(*v),
            )),
            "top_level_changed" => {
                Some(EventSignalRef::mapped("top_level_changed", &self.top_level_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "visibility_changed" => {
                Some(EventSignalRef::mapped("visibility_changed", &self.visibility_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "float_changed" => {
                Some(EventSignalRef::mapped("float_changed", &self.float_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "customize_requested" => {
                Some(EventSignalRef::unit("customize_requested", &self.customize_requested))
            }
            _ => None,
        }
    }
}

/// `ToolBar`'s property contract.
///
/// `orientation` is published as its token spelling and parsed back through
/// `expect_toolbar_orientation`; `icon_size` crosses the boundary as `Float`,
/// narrowed to `f32` by `expect_f32`.
impl WidgetProperties for ToolBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "orientation" => Ok(CapabilityValue::String(
                tool_bar_orientation_to_str(self.orientation()).to_string(),
            )),
            "icon_size" => Ok(CapabilityValue::Float(self.icon_size() as f64)),
            "floatable" => Ok(CapabilityValue::Bool(self.is_floatable())),
            "movable" => Ok(CapabilityValue::Bool(self.is_movable())),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "movable" => {
                self.set_movable(expect_bool(value)?);
                Ok(())
            }
            "floatable" => {
                self.set_floatable(expect_bool(value)?);
                Ok(())
            }
            "icon_size" => {
                self.set_icon_size(expect_f32(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_toolbar_orientation(value)?);
                Ok(())
            }
            // Derived from the item list, which is mutated through `add_action` /
            // `add_separator` rather than by assigning a count.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "orientation",
            "icon_size",
            "movable",
            "floatable",
            "item_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tool_bar` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every item,
    /// which is exactly what [`ToolBar::clear`] does. `add_action` takes an id and a
    /// label and `add_separator` appends a divider whose position the caller may
    /// care about, so a payload-less invocation of either is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the names are right and the argument
    /// is what is missing, which is not `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_action" | "add_separator" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ToolBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                // The grip is not an item: hovering it must not light up the action underneath
                // the affordance the pointer is actually over.
                let over_grip = self.movable && self.drag_band().contains_point(*pos);
                self.hovered_index = if over_grip { None } else { self.hit_item(*pos) };
                if !self.movable {
                    return;
                }
                if let Some(offset) = self.drag_state {
                    let current = self.geometry();
                    let next = Rect::new(
                        pos.x - offset.x,
                        pos.y - offset.y,
                        current.width,
                        current.height,
                    );
                    if next != current {
                        // `set_geometry` stores the rectangle but does not repaint — the drag is
                        // only visible because the next frame is requested here.
                        self.base.set_geometry(next);
                        self.base.request_redraw();
                    }
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                // The grip wins over the item it overlaps, and only when dragging is permitted.
                // A non-movable toolbar draws no grip, so a press there stays an ordinary press.
                if self.movable && self.drag_band().contains_point(*pos) {
                    let origin = self.geometry();
                    self.drag_state = Some(Point::new(pos.x - origin.x, pos.y - origin.y));
                    self.drag_dock_band = Some(self.band_rect());
                    return;
                }
                if self.customize_button_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.customize_requested.emit();
                    return;
                }
                if self.overflow_button_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    let max = self.items.len().saturating_sub(self.effective_capacity());
                    self.overflow_offset =
                        if self.overflow_offset >= max { 0 } else { self.overflow_offset + 1 };
                    self.base.request_redraw();
                    return;
                }
                if let Some(idx) = self.hit_item(*pos) {
                    if let Some(item) = self.items.get_mut(idx) {
                        if item.is_enabled() && !item.is_separator() {
                            if item.is_checkable() {
                                item.set_checked(!item.is_checked());
                            }
                            let id = item.id().to_string();
                            self.action_triggered.emit(id);
                        }
                    }
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                self.finish_drag(*pos);
            }
            Event::Wheel { delta, .. } if self.is_overflowing() => {
                // The wheel walks the overflow window rather than doing nothing: once a
                // strip overflows, the hidden items have no other pointer path.
                let max = self.items.len().saturating_sub(self.effective_capacity());
                if delta.y > 0 {
                    self.overflow_offset = self.overflow_offset.saturating_sub(1);
                } else if delta.y < 0 {
                    self.overflow_offset = (self.overflow_offset + 1).min(max);
                }
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for ToolBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **strip**, not the control's rectangle: see `band_rect`. Every measurement below
        // — the fill, the rule, the separators and the item squares — is taken from this one
        // band, and the item bands (and therefore the hit test) read the same one.
        let rect = self.band_rect();
        let style = self.style();
        // Background
        //
        // From the style, not a literal. This painted `Color::rgb(245, 245, 245)` and so
        // stayed light in a dark theme: the theme resolved a toolbar's colour, handed it
        // to the widget, and the widget ignored it — a light band across the top of a
        // dark window with no way for the caller to change it.
        // The toolbar's own surface, resolved once, is the **base every item state is
        // derived from**. The comment above records the first half of this fix (the strip's
        // background was a literal); these six literals are the other half — with the
        // background themed and the items not, a dark toolbar kept a row of pale blue buttons
        // whose checked and hover states were inverted against their own strip (checked
        // `rgb(180,210,255)` was *lighter* than hover `rgb(210,230,255)`'s neighbours and both
        // were lighter than the surface they sat on, so a checked item read as a hole).
        let surface = style.background_color.unwrap_or(Color::rgb(245, 245, 245));
        let border = style.border_color.unwrap_or_else(|| surface.contrast_color().with_alpha(60));
        context.face_with_gradient(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Draw bottom border line
        let y = rect.y + rect.height as f32 as i32 - 1;
        context.draw_line(Point::new(rect.x, y), Point::new(rect.x + rect.width as i32, y), border);
        // A selection indicator is a step from the item's own rest fill, so it stays a
        // selection in either appearance. `primary` is the palette's active/selected slot.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|theme| theme.colors.primary)
            .unwrap_or(Color::rgb(0, 120, 215));
        let resting = surface.blend(&surface.contrast_color(), 0.04);
        let hovered_fill = resting.blend(&accent, 0.18);
        let checked_fill = resting.blend(&accent, 0.34);

        // One pass over the item boxes for the whole paint. A per-index helper derives its answer
        // from the same `item_bands`, which walks every item; calling it inside this loop made
        // painting an N-item toolbar O(N²) (REND-01). The boxes are a pure function of the band,
        // the item count and `icon_size`, so computing them once is the same picture at one pass.
        let bands = self.item_bands();
        for (i, &item_r) in bands.iter().enumerate() {
            let item = &self.items[i];
            if item_r.width == 0 || item_r.height == 0 {
                // An item scrolled out of the overflow window keeps its index but has no box.
                continue;
            }
            if item.is_separator() {
                // Both separators read the same resolved border colour. The horizontal one
                // already did; the vertical one was a literal `rgb(200,200,200)`, so the same
                // toolbar drew its two dividers in two different colours.
                match self.orientation {
                    ToolBarOrientation::Horizontal => {
                        let mid_x = item_r.x + (item_r.width as i32) / 2;
                        context.draw_line(
                            Point::new(mid_x, rect.y + 4),
                            Point::new(mid_x, rect.y + rect.height as i32 - 4),
                            border,
                        );
                    }
                    ToolBarOrientation::Vertical => {
                        let mid_y = item_r.y + item_r.height as i32 / 2;
                        context.draw_line(
                            Point::new(rect.x + 4, mid_y),
                            Point::new(rect.x + rect.width as i32 - 4, mid_y),
                            border,
                        );
                    }
                }
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            let bg = if item.is_checked() {
                checked_fill
            } else if is_hovered {
                hovered_fill
            } else {
                resting
            };
            let item_band = Rect::new(item_r.x, item_r.y, item_r.width, item_r.height);
            context.fill_rect(item_band, bg);
            if is_hovered || item.is_checked() {
                context.draw_rect(item_band, accent);
            }
            // The ink is chosen from the fill it lands on. The old rule was "black, or grey
            // when disabled", which on a dark strip was black text on a near-black button.
            let ink = surface.contrast_color();
            let fg = if item.is_enabled() { bg.contrast_color() } else { ink.with_alpha(130) };
            // The **effective font** — the resolved theme/caller font — so toolbar item labels
            // honour the theme body font and the user's text scale (D09-STYLE-01).
            let font = effective_font(style);
            let line = context.text_line(item_band, font);
            let icon = item.icon();
            match self.button_style {
                // The historical path: one centred label, no icon, no second layout.
                ToolBarButtonStyle::TextOnly => {
                    context.draw_text_fitted_cow(
                        line,
                        item.text(),
                        font,
                        fg,
                        HorizontalAlignment::Center,
                    );
                }
                ToolBarButtonStyle::IconOnly => {
                    if let Some(icon) = icon {
                        context.draw_text(
                            Point::new(item_band.x, line.y),
                            icon,
                            font,
                            fg,
                            HorizontalAlignment::Center,
                        );
                    }
                }
                ToolBarButtonStyle::TextBesideIcon => {
                    let icon_width =
                        item.icon_size().unwrap_or(self.icon_size as u32).min(item_band.width);
                    if let Some(icon) = icon {
                        context.draw_text_fitted_cow(
                            Rect::new(item_band.x, line.y, icon_width, line.height.max(1)),
                            icon,
                            font,
                            fg,
                            HorizontalAlignment::Center,
                        );
                    }
                    let label_x = item_band.x + icon_width as i32;
                    let label_width =
                        (item_band.x + item_band.width as i32 - label_x).max(0) as u32;
                    context.draw_text_fitted_cow(
                        Rect::new(label_x, line.y, label_width, line.height.max(1)),
                        item.text(),
                        font,
                        fg,
                        HorizontalAlignment::Left,
                    );
                }
                ToolBarButtonStyle::TextUnderIcon => {
                    let icon_side = item
                        .icon_size()
                        .unwrap_or(self.icon_size as u32)
                        .min(item_band.height / 2)
                        .min(item_band.width);
                    if let Some(icon) = icon {
                        context.draw_text_fitted_cow(
                            Rect::new(item_band.x, item_band.y, item_band.width, icon_side.max(1)),
                            icon,
                            font,
                            fg,
                            HorizontalAlignment::Center,
                        );
                    }
                    let label_y = item_band.y + icon_side as i32;
                    context.draw_text_fitted_cow(
                        Rect::new(
                            item_band.x,
                            label_y,
                            item_band.width,
                            (item_band.y + item_band.height as i32 - label_y).max(0) as u32,
                        ),
                        item.text(),
                        font,
                        fg,
                        HorizontalAlignment::Center,
                    );
                }
            }
        }

        // ── Trailing affordances ──
        let affordance_ink = surface.contrast_color().with_alpha(150);
        if let Some(rect) = self.customize_button_rect() {
            // Three rules read as a "customize" handle without needing a font glyph.
            let step = (rect.height / 5).max(1) as i32;
            for row in 0..3 {
                let y = rect.y + step * (row + 1);
                context.draw_line(
                    Point::new(rect.x + 5, y),
                    Point::new(rect.x + rect.width as i32 - 5, y),
                    affordance_ink,
                );
            }
        }
        if let Some(rect) = self.overflow_button_rect() {
            // A chevron pointing along the strip: overflowing items are further along it.
            let mid = rect.y + rect.height as i32 / 2;
            let cx = rect.x + rect.width as i32 / 2;
            let points = match self.orientation {
                ToolBarOrientation::Horizontal => vec![
                    Point::new(cx - 2, mid - 4),
                    Point::new(cx + 3, mid),
                    Point::new(cx - 2, mid + 4),
                ],
                ToolBarOrientation::Vertical => vec![
                    Point::new(cx - 4, mid - 2),
                    Point::new(cx, mid + 3),
                    Point::new(cx + 4, mid - 2),
                ],
            };
            context.execute_command(crate::render::RenderCommand::DrawPath {
                points,
                closed: false,
                color: affordance_ink,
                filled: false,
                width: 1,
            });
        }

        // ── The drag affordance ──
        //
        // Only a movable toolbar can be dragged, so only a movable toolbar advertises it. The
        // six dots are a 2x3 grid on a horizontal strip and a 3x2 grid on a vertical one, placed
        // inside the same trailing band the press reads — the ink and the hit target are one
        // derivation.
        if self.movable {
            let grip = self.drag_band();
            let grip_ink = surface.contrast_color().with_alpha(110);
            let dot: u32 = 2;
            let gap: i32 = 3;
            let (columns, rows) = match self.orientation {
                ToolBarOrientation::Horizontal => (2i32, 3i32),
                ToolBarOrientation::Vertical => (3i32, 2i32),
            };
            let total_width = columns * dot as i32 + (columns - 1) * gap;
            let total_height = rows * dot as i32 + (rows - 1) * gap;
            let start_x = grip.x + (grip.width as i32 - total_width) / 2;
            let start_y = grip.y + (grip.height as i32 - total_height) / 2;
            for row in 0..rows {
                for column in 0..columns {
                    let x = start_x + column * (dot as i32 + gap);
                    let y = start_y + row * (dot as i32 + gap);
                    context.fill_rect(Rect::new(x, y, dot, dot), grip_ink);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_item_state_accessors_handle_valid_and_oob_indices() {
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        let idx = tool_bar.add_action("save", "Save");

        assert_eq!(tool_bar.item_enabled(idx), Some(true));
        tool_bar.set_item_enabled(idx, false);
        assert_eq!(tool_bar.item_enabled(idx), Some(false));
        assert_eq!(tool_bar.item_enabled(99), None);

        tool_bar.items[idx].set_checkable(true);
        assert_eq!(tool_bar.item_checked(idx), Some(false));
        tool_bar.set_item_checked(idx, true);
        assert_eq!(tool_bar.item_checked(idx), Some(true));
        assert_eq!(tool_bar.item_checked(99), None);
    }

    /// The items tile the strip in sequence, each with the width its icon size implies.
    ///
    /// # What this pins
    ///
    /// BLUE22 §F.2.2 describes this control as "item positions from a step literal", and the
    /// property that replaces it is that an item's box is what the *layout* reports. The test
    /// asserts the sequence directly: each item's leading edge is the previous one's trailing edge
    /// plus the toolbar's own spacing, so an item that grew (a wider icon) moves every later one
    /// rather than overlapping it.
    #[test]
    fn the_items_tile_the_strip_in_sequence() {
        // The band is the strip's own height, clamped to whatever the control was given: a census
        // cell is 240x120, and a form usually gives the strip exactly its height. Use a rectangle
        // tall enough to hold the strip, so the assertion is about the item boxes rather than about
        // the clamping.
        let strip_height = dimensions::TOOLBAR_HEIGHT + dimensions::TOOLBAR_ITEM_INSET * 2;
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, strip_height));
        tool_bar.set_icon_size(24.0);
        tool_bar.add_action("save", "Save");
        tool_bar.add_separator();
        tool_bar.add_action("open", "Open");
        let bands = tool_bar.item_bands();
        assert_eq!(bands.len(), 3, "one band per item, separators included");
        assert_eq!(bands[0].x, dimensions::TOOLBAR_ITEM_INSET as i32);
        for pair in bands.windows(2) {
            assert_eq!(
                pair[1].x,
                pair[0].x + pair[0].width as i32,
                "each item follows the previous one's own box — the run is contiguous: {bands:?}"
            );
        }
        // A button is one icon square *plus the spacing*, which is the control's own historic step:
        // `icon_size + TOOLBAR_SPACING` is an item's advance. A separator is one spacing wide. Both
        // read the same constant, so an item's advance and the separator's own box cannot drift.
        let button = 24u32 + dimensions::TOOLBAR_SPACING;
        assert_eq!(bands[0].width, button);
        assert_eq!(bands[1].width, dimensions::TOOLBAR_SPACING);
        assert_eq!(bands[2].width, button);
        // Every item is inset vertically from the **strip**, so the hover fill is a row, not the
        // cell. The item's own declared height is the strip minus the inset at both ends, and the
        // layout's padding is that same inset — two readings of one constant, so the assertion is
        // stated against the constant rather than against the layout's arithmetic.
        let strip = tool_bar.band_rect();
        let expected_height =
            dimensions::TOOLBAR_HEIGHT.saturating_sub(dimensions::TOOLBAR_ITEM_INSET * 2);
        for band in &bands {
            assert_eq!(band.y, strip.y + dimensions::TOOLBAR_ITEM_INSET as i32);
            assert_eq!(band.height, expected_height);
        }
    }

    /// A wider icon size moves every later item rather than only the one it belongs to.
    ///
    /// This is §B.9's rule in its mechanical form — "the position of a sibling is derived from the
    /// size of the one before it" — which a running accumulator of constants can also satisfy, and
    /// which the previous step-per-item form did. The difference is that the *layout* now owns it:
    /// the same assertion holds for a vertical toolbar without a second code path.
    #[test]
    fn a_wider_icon_moves_every_later_item() {
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, dimensions::TOOLBAR_HEIGHT));
        tool_bar.set_icon_size(24.0);
        tool_bar.add_action("a", "A");
        tool_bar.add_action("b", "B");
        let before = tool_bar.item_bands()[1].x;

        tool_bar.set_icon_size(40.0);
        let after = tool_bar.item_bands()[1].x;
        assert_eq!(after - before, 16, "the second item moves by exactly the growth of the first");
    }

    /// A vertical toolbar stacks its items down the strip with the same relation.
    #[test]
    fn a_vertical_toolbar_stacks_its_items_down_the_strip() {
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 40, 240));
        tool_bar.set_orientation(ToolBarOrientation::Vertical);
        tool_bar.add_action("a", "A");
        tool_bar.add_action("b", "B");
        let bands = tool_bar.item_bands();
        assert_eq!(bands[0].x, bands[1].x, "both items occupy the one column");
        assert_eq!(
            bands[1].y,
            bands[0].y + bands[0].height as i32,
            "the second item follows the first one's own box down the strip: {bands:?}"
        );
    }

    /// A press on the grip followed by a move translates the control in real time.
    ///
    /// `movable` used to be a stored permission with no gesture behind it: the flag could be
    /// read, set and even published through the property contract, and the toolbar still could
    /// not be dragged. The grip band at the strip's trailing end is the missing entry point;
    /// this pins that a move while it is held writes the new rectangle to the base.
    #[test]
    fn dragging_the_grip_moves_the_toolbar() {
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        // Docking is a separate behaviour; keep this test about the translation itself.
        tool_bar.set_floatable(false);
        let grip = tool_bar.drag_band();
        let press = Point::new(grip.x + 2, grip.y + 2);

        tool_bar.handle_event(&Event::MousePress { pos: press, button: 1, modifiers: 0 });
        tool_bar.handle_event(&Event::MouseMove { pos: Point::new(press.x + 40, press.y + 10) });

        assert_eq!(tool_bar.geometry().x, 40, "the strip follows the pointer horizontally");
        assert_eq!(tool_bar.geometry().y, 10, "and vertically");
        assert_eq!(
            tool_bar.geometry().width,
            240,
            "a drag moves the control, it does not resize it"
        );
        assert_eq!(tool_bar.geometry().height, 36);
    }

    /// `movable = false` is a real refusal, not a drawn decoration.
    #[test]
    fn a_non_movable_toolbar_ignores_the_drag_gesture() {
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        tool_bar.set_movable(false);
        let grip = tool_bar.drag_band();
        let press = Point::new(grip.x + 2, grip.y + 2);
        let before = tool_bar.geometry();

        tool_bar.handle_event(&Event::MousePress { pos: press, button: 1, modifiers: 0 });
        tool_bar.handle_event(&Event::MouseMove { pos: Point::new(press.x + 40, press.y + 10) });

        assert_eq!(tool_bar.geometry(), before, "a toolbar that is not movable must stay put");
    }

    /// Releasing away from the strip's dock band floats a floatable toolbar and says so.
    ///
    /// The host can only re-lay out around a torn-off toolbar if the control tells it that the
    /// docking state changed; `float_changed` is that notification, emitted alongside the
    /// pre-existing `top_level_changed`.
    #[test]
    fn releasing_outside_the_dock_band_floats_and_emits_float_changed() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        let floated = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicUsize::new(0));
        {
            let floated = Arc::clone(&floated);
            let count = Arc::clone(&count);
            tool_bar.float_changed.connect(move |value| {
                floated.store(*value, Ordering::SeqCst);
                count.fetch_add(1, Ordering::SeqCst);
            });
        }

        let grip = tool_bar.drag_band();
        let press = Point::new(grip.x + 2, grip.y + 2);
        tool_bar.handle_event(&Event::MousePress { pos: press, button: 1, modifiers: 0 });
        let away = Point::new(press.x + 400, press.y + 200);
        tool_bar.handle_event(&Event::MouseMove { pos: away });
        tool_bar.handle_event(&Event::MouseRelease { pos: away, button: 1 });

        assert!(tool_bar.is_top_level(), "a release away from the dock must float the strip");
        assert_eq!(count.load(Ordering::SeqCst), 1, "the host is told exactly once");
        assert!(floated.load(Ordering::SeqCst), "and told the new floating state");
    }

    /// The dock areas default to every edge and can be restricted.
    #[test]
    fn allowed_areas_default_to_every_edge_and_can_be_restricted() {
        let mut bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        assert_eq!(bar.allowed_areas(), &ALL_TOOL_BAR_AREAS);
        assert!(bar.allows_area(ToolBarArea::Left));

        bar.set_allowed_areas([ToolBarArea::Top, ToolBarArea::Bottom]);
        assert!(bar.allows_area(ToolBarArea::Top));
        assert!(!bar.allows_area(ToolBarArea::Left));

        let before = bar.allowed_areas().len();
        bar.set_allowed_areas([ToolBarArea::Top, ToolBarArea::Bottom]);
        assert_eq!(bar.allowed_areas().len(), before, "an unchanged list is a no-op");

        bar.set_allowed_areas([]);
        assert!(bar.allowed_areas().is_empty(), "empty means floating-only");
    }

    /// A per-item icon size widens that item and everything after it.
    #[test]
    fn a_per_item_icon_size_widens_only_that_item() {
        let mut bar = ToolBar::new(Rect::new(0, 0, 400, 36));
        bar.add_action("a", "A");
        bar.add_action("b", "B");
        let before = bar.item_bands();
        let step = before[0].width;
        assert_eq!(before[1].width, step);

        bar.items[1].set_icon_size(Some(48));
        let after = bar.item_bands();
        assert_eq!(after[0].width, step, "the first item keeps the bar's size");
        assert!(after[1].width > step, "the override widens only its own item");
        assert_eq!(after[1].x, after[0].x + after[0].width as i32);
    }

    /// Overflowing items get a chevron; each press advances the window.
    #[test]
    fn the_overflow_chevron_walks_the_hidden_items() {
        let mut bar = ToolBar::new(Rect::new(0, 0, 70, 36));
        for index in 0..8 {
            bar.add_action(format!("id{index}"), format!("T{index}"));
        }
        assert!(bar.is_overflowing());
        let chevron = bar.overflow_button_rect().expect("overflow has an affordance");
        assert_eq!(bar.visible_range().start, 0);

        bar.handle_event(&Event::mouse_press(chevron.x + 2, chevron.y + 2, 1));
        assert_eq!(bar.visible_range().start, 1, "a press advances the visible window");
        bar.handle_event(&Event::mouse_press(chevron.x + 2, chevron.y + 2, 1));
        assert_eq!(bar.visible_range().start, 2);
    }

    /// The customize affordance is opt-in and reports its press.
    #[test]
    fn the_customize_affordance_is_opt_in_and_reports_a_press() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        bar.add_action("a", "A");
        assert!(bar.customize_button_rect().is_none(), "off by default");
        bar.set_editable(true);
        let rect = bar.customize_button_rect().expect("editable strips show the handle");

        let presses = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&presses);
        bar.customize_requested.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        bar.handle_event(&Event::mouse_press(rect.x + 2, rect.y + 2, 1));
        assert_eq!(presses.load(Ordering::SeqCst), 1);
    }

    /// Icon-only presentation paints the icon instead of the label.
    #[test]
    fn icon_only_presentation_hides_the_label() {
        let mut bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        let index = bar.add_action("a", "Open");
        bar.items[index].set_icon(Some("O".to_string()));
        assert_eq!(bar.button_style(), ToolBarButtonStyle::TextOnly, "text is the default");
        let text_only = crate::widget::svg::render_to_svg(&mut bar);

        bar.set_button_style(ToolBarButtonStyle::IconOnly);
        let icon_only = crate::widget::svg::render_to_svg(&mut bar);
        assert_ne!(text_only, icon_only, "the style must reach the painter");

        bar.set_button_style(ToolBarButtonStyle::TextBesideIcon);
        let beside = crate::widget::svg::render_to_svg(&mut bar);
        assert_ne!(beside, icon_only);
    }
}
