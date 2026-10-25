// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! RibbonBar (Office-style ribbon) widget.
//!
//! Displays a horizontal row of tab headers (like TabWidget) and a content
//! panel below them containing labelled groups of icon/text items.
//!
//! Layout (simplified Office 2007+ style):
//! ┌──────────────────────────────────────────────────┐
//! │ Tab1 │ Tab2 │ Tab3 │ ...               [▲] (min)│  ← tab header row
//! ├──────┴──────┴──────┴─────────────────────────────┤
//! │ Group1   │ Group2   │ Group3   │ Group4          │
//! │ ┌──────┐ │ ┌──────┐ │ ┌──────┐ │ ┌──────┐       │
//! │ │ Icon │ │ │ Icon │ │ │Icon  │ │ │Icon  │       │
//! │ │ Text │ │ │Text  │ │ │Text  │ │ │Text  │       │
//! │ └──────┘ │ └──────┘ │ └──────┘ │ └──────┘       │  ← large items
//! │ ──────── │ ──────── │ ──────── │ ────────        │
//! │ \[A\] \[B\]  │ \[C\] \[D\]  │ \[E\] \[F\]  │ \[G\] \[H\]        │  ← small items row
//! │ ──────── │ ──────── │ ──────── │ ────────        │
//! │ \[I\] \[J\]  │          │          │                 │  ← small items row
//! │──────────│──────────│──────────│──────────        │  ← group separator
//! │ Group1   │ Group2   │ Group3   │ Group4           │  ← group title row
//! └──────────────────────────────────────────────────┘

use crate::compat::Rc;
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string_list, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::TooltipManager;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// ── Layout constants ──────────────────────────────────────────────────────────

const TAB_HEIGHT: i32 = 20;
const TAB_SPACING: i32 = 1;
/// Horizontal padding around a tab's measured title.
const TAB_TEXT_PADDING: i32 = 12;
/// Horizontal padding inside one quick-access item.
const QUICK_ACCESS_PADDING: i32 = 4;
/// Gap between two quick-access items.
const QUICK_ACCESS_GAP: i32 = 2;
/// Width reserved for the overflow marker.
const QUICK_ACCESS_OVERFLOW_WIDTH: i32 = 20;
/// Height of one row in the overflow panel.
const QUICK_ACCESS_OVERFLOW_ROW: u32 = 18;
const PANEL_TOP_MARGIN: i32 = 2; // gap between tab row and content panel
const GROUP_PADDING: i32 = 4;
const LARGE_ICON_SIZE: i32 = 32;
const SMALL_ICON_SIZE: i32 = 16;
const ITEM_SPACING: i32 = 4;
const MINIMIZE_BUTTON_SIZE: i32 = 16;
const MINIMIZE_MARGIN: i32 = 4;
const GROUP_TITLE_HEIGHT: i32 = 16;

// ── Types ─────────────────────────────────────────────────────────────────────

/// A group within a ribbon tab.
#[derive(Debug, Clone)]
pub struct RibbonGroup {
    title: String,
    items: Vec<RibbonItem>,
    /// Whether the group paints its items as a gallery grid.
    gallery: bool,
}

impl RibbonGroup {
    /// Creates a new ribbon group with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), items: Vec::new(), gallery: false }
    }

    /// Returns the group title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the group title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Returns all items in this group.
    pub fn items(&self) -> &[RibbonItem] {
        &self.items
    }

    /// Returns mutable access to all items in this group.
    pub fn items_mut(&mut self) -> &mut Vec<RibbonItem> {
        &mut self.items
    }

    /// Whether the group paints a gallery grid.
    pub fn is_gallery(&self) -> bool {
        self.gallery
    }

    /// Makes the group a gallery: its items are painted as a three-column grid of
    /// tiles instead of the command layout. Off by default.
    pub fn set_gallery(&mut self, gallery: bool) {
        self.gallery = gallery;
    }
}

/// An item (button) within a ribbon group.
#[derive(Debug, Clone)]
pub struct RibbonItem {
    text: String,
    icon_text: String,
    icon_source: Option<IconSource>,
    tooltip: String,
    enabled: bool,
    checkable: bool,
    checked: bool,
    large: bool,
}

impl RibbonItem {
    /// Creates a new ribbon item.
    ///
    /// The item starts with **no icon**. `icon_text` used to be seeded with the item's own text,
    /// and the paint path draws its first character as the icon — so a ribbon created without
    /// explicit icons showed a lone `P` above `Paste`, `B` before `Bold`, and so on. That is a
    /// duplicated label, not an icon, and it is the first thing a reader sees. An item therefore
    /// stays text-only until the caller supplies [`RibbonItem::set_icon_text`] or a typed source.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            icon_text: String::new(),
            icon_source: None,
            text,
            tooltip: String::new(),
            enabled: true,
            checkable: false,
            checked: false,
            large: false,
        }
    }

    /// Returns the item text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the item text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Returns the icon text.
    pub fn icon_text(&self) -> &str {
        &self.icon_text
    }

    /// Sets the icon text.
    pub fn set_icon_text(&mut self, icon: impl Into<String>) {
        self.icon_text = icon.into();
        self.icon_source = None;
    }

    /// Returns the typed icon source, if one is assigned.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Replaces the typed icon source without changing the item's text label.
    /// Replaces the typed icon source.
    ///
    /// The legacy `icon_text` is cleared with it, because the crate's typed setters share **one**
    /// active state and the most recent explicit set wins (blue27 §19.6). Leaving the text in place
    /// would make `set_icon_source(None)` resurrect the text that a later `set_icon_source(Some(..))`
    /// had already replaced -- and every other control that keeps a legacy text icon (`Action`, `Fab`)
    /// clears it here.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_text.clear();
        self.icon_source = source;
    }

    /// Returns the tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Sets the tooltip.
    pub fn set_tooltip(&mut self, tip: impl Into<String>) {
        self.tooltip = tip.into();
    }

    /// Returns whether the item is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets whether the item is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns whether the item is checkable.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Sets whether the item is checkable.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }

    /// Returns whether the item is checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets whether the item is checked.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Returns whether the item is displayed in large mode.
    pub fn is_large(&self) -> bool {
        self.large
    }

    /// Sets whether the item is displayed in large mode.
    pub fn set_large(&mut self, large: bool) {
        self.large = large;
    }
}

/// One quick-access command.
///
/// The short-label list the ribbon shipped with could only express text, so an entry could not
/// carry an icon, a checked state or a separator. This type is the attribute-carrying form; the
/// legacy [`RibbonBar::quick_access`] labels remain supported when no items are installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickAccessItem {
    /// The short label painted beside the icon.
    pub label: String,
    /// An optional icon glyph, painted left of the label.
    pub icon: String,
    /// Whether the item paints a check mark and toggles it on click.
    pub checkable: bool,
    /// The current checked state, meaningful only when `checkable` is set.
    pub checked: bool,
    /// Whether this entry is a separator rule rather than a command.
    pub separator: bool,
}

impl QuickAccessItem {
    /// Creates a plain quick-access command.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: String::new(),
            checkable: false,
            checked: false,
            separator: false,
        }
    }

    /// Creates a separator rule.
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            icon: String::new(),
            checkable: false,
            checked: false,
            separator: true,
        }
    }

    /// Sets the leading icon glyph.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// Marks the item as checkable and, optionally, currently checked.
    pub fn with_checkable(mut self, checked: bool) -> Self {
        self.checkable = true;
        self.checked = checked;
        self
    }
}

/// RibbonBar (Office-style ribbon) widget.
pub struct RibbonBar {
    base: BaseWidget,
    tabs: Vec<String>,
    groups: Vec<Vec<RibbonGroup>>,
    current_tab_index: usize,
    expanded: bool,
    minimized: bool,
    // Interaction state
    hovered_tab: Option<usize>,
    hovered_item: Option<(usize, usize, usize)>, // (tab, group, item)
    minimize_hovered: bool,
    /// The panel a tab click opens over a minimized ribbon (Office semantics).
    ///
    /// The click leaves `minimized` set, so the state is "minimized, with the clicked tab's
    /// panel shown"; the pointer leaving the bar, a second click on the open tab, or
    /// choosing an item from the panel takes it back down.
    temporarily_expanded: bool,
    /// A label painted as the application button left of the tabs.
    application_button: Option<String>,
    /// Short labels of the quick-access commands, painted at the trailing end of the
    /// tab row.
    quick_access: Vec<String>,
    /// Attribute-carrying quick-access commands. When non-empty these take precedence over
    /// [`Self::quick_access`].
    quick_access_items: Vec<QuickAccessItem>,
    /// Whether commands that do not fit collapse behind an overflow marker.
    quick_access_overflow_menu: bool,
    /// Whether the overflow panel is open.
    quick_access_overflow_open: bool,
    /// Indices of tabs that are context tabs (tinted with the accent).
    context_tabs: Vec<usize>,
    /// The item the keyboard is focused on.
    focused_item: Option<(usize, usize, usize)>,
    /// Whether KeyTips are showing.
    key_tips_visible: bool,
    /// Explicit KeyTip strings, keyed by item address.
    key_tips: Vec<(usize, usize, usize, String)>,
    /// The content panel's reveal, so minimizing animates like every other popup.
    panel_reveal: crate::widget::menu_toolbar::popup_reveal::PopupReveal,

    /// Emitted when the current tab changes; payload is the new tab index.
    pub current_tab_changed: Signal1<usize>,
    /// Emitted when an item is triggered; payload is (tab, group, item).
    pub item_triggered: Signal1<(usize, usize, usize)>,
    /// Emitted when the application button is pressed.
    pub application_clicked: GenericSignal,
    /// Emitted with the quick-access index when one of its labels is pressed.
    pub quick_access_clicked: Signal1<usize>,
    /// Emitted when the overflow marker is pressed. Carries no payload; the caller opens its
    /// own menu, or reads [`RibbonBar::quick_access_overflowed_indices`] for the hidden entries.
    pub quick_access_overflow_clicked: GenericSignal,
    /// Emitted with `(tab, group)` when a group's overflow marker is pressed.
    pub group_overflow_clicked: Signal1<(usize, usize)>,
    /// The group and item boxes for the current layout, built once per layout signature
    /// (K-P08).
    ///
    /// `hit_item` runs on every pointer move and used to rebuild `group_rects` plus one
    /// `compute_item_rects` per group — several `Vec` allocations — for a layout that changes only
    /// with the geometry, the tab, the expand state or the item list. The cache is keyed on a
    /// fold over exactly those facts, so it cannot serve a stale table, and the painter reads the
    /// same table the hit test does.
    layout_cache: crate::compat::RefCell<Option<(u64, Rc<Vec<GroupLayout>>)>>,
    /// The host-side hover/delay/show pipeline for item tooltips (ADV-10).
    tooltips: TooltipManager,
    /// The derived hover id currently registered with the tooltip pipeline.
    tooltip_source: Option<crate::core::ObjectId>,
    /// Monotonic clock fed to the tooltip pipeline from [`Widget::tick`].
    tooltip_now_ms: u64,
}

/// One group's box and the boxes of the items the layout could place inside it.
///
/// The painter and the hit test both need `(group index, group rect, item rects)`, so they share
/// this one table instead of each deriving it (K-P08).
type GroupLayout = (usize, Rect, Vec<(usize, Rect)>);

/// The stable hover id for one ribbon item.
///
/// The tooltip pipeline addresses a hover source by id; an item is not a widget, so the id is
/// derived from the owner's and the item's address. The mix is the one the toolbar uses, so the
/// two controls cannot collide on an accidental linear id.
fn hover_id(owner: ObjectId, tab: usize, group: usize, item: usize) -> ObjectId {
    let address = (tab as u64) << 42 ^ (group as u64) << 21 ^ item as u64;
    owner.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (address + 1)
}

/// The chrome colours one ribbon draw pass uses.
///
/// Resolved once per `Draw` from the explicit style, then the theme's resolved style for the
/// control, then a literal, and carried into the three drawing helpers. Before this the ribbon's
/// four chrome families — the strip, the tabs, the minimize button and the panel — were every one
/// a literal, so a light/dark switch left the whole bar unchanged and the rendering census reported
/// it as theme-blind. The item colours derive from the panel so they read against a light backdrop
/// and a dark one without a second table of literals.
struct RibbonPalette {
    /// The strip behind the tabs.
    strip: Color,
    /// An inactive tab's fill, and the minimize button's resting fill.
    tab: Color,
    /// The active tab and the panel body, which is one step off the strip.
    surface: Color,
    /// A hovered tab, and the panel's top rule.
    hover: Color,
    /// Borders and separator rules.
    border: Color,
    /// Label text.
    ink: Color,
    /// Secondary text: the group titles.
    muted: Color,
    /// An item's resting fill.
    item: Color,
    /// A hovered item's fill.
    item_hover: Color,
    /// A checked item's fill.
    item_checked: Color,
    /// The accent an item's border and icon carry.
    accent: Color,
}

impl RibbonPalette {
    /// Derives the palette from the resolved style and the active theme.
    ///
    /// Both lookups are the cached, revision-validated forms (`resolved_theme_style_shared` and
    /// `palette_shared`), so the whole palette is resolved from a refcount bump and a single
    /// palette read rather than a manager lock per field.
    fn resolve(style: &crate::style::WidgetStyle) -> Self {
        let theme = crate::style::resolved_theme_style_shared("ribbon_bar");
        // One palette read for the whole pass: `palette_shared` is the revision-validated cache, so
        // this is a refcount bump rather than the manager lock the old four-field read took
        // (K-P17).
        let (window_fill, foreground, secondary, primary) = match crate::style::palette_shared() {
            Some(colors) => {
                (colors.background, colors.foreground, colors.secondary, colors.primary)
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
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `ribbon_bar` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`.
        // A strip painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let strip = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != strip)
            .unwrap_or_else(|| strip.blend(&secondary, 0.45));

        Self {
            strip,
            tab: strip.blend(&ink, 0.10),
            surface: strip.blend(&ink, 0.22),
            hover: strip.blend(&ink, 0.16),
            border,
            ink,
            muted: ink.blend(&strip, 0.55),
            item: strip.blend(&ink, 0.22),
            item_hover: strip.blend(&primary, 0.35),
            item_checked: strip.blend(&primary, 0.55),
            accent: primary,
        }
    }
}

impl RibbonBar {
    /// Creates a new `RibbonBar` with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RibbonBar, geometry, "RibbonBar"),
            tabs: Vec::new(),
            groups: Vec::new(),
            current_tab_index: 0,
            expanded: true,
            minimized: false,
            hovered_tab: None,
            hovered_item: None,
            minimize_hovered: false,
            temporarily_expanded: false,
            application_button: None,
            quick_access: Vec::new(),
            quick_access_items: Vec::new(),
            quick_access_overflow_menu: false,
            quick_access_overflow_open: false,
            context_tabs: Vec::new(),
            focused_item: None,
            key_tips_visible: false,
            key_tips: Vec::new(),
            panel_reveal: crate::widget::menu_toolbar::popup_reveal::PopupReveal::new(true),
            current_tab_changed: Signal1::new(),
            item_triggered: Signal1::new(),
            application_clicked: GenericSignal::new(),
            quick_access_clicked: Signal1::new(),
            quick_access_overflow_clicked: GenericSignal::new(),
            group_overflow_clicked: Signal1::new(),
            layout_cache: crate::compat::RefCell::new(None),
            tooltips: TooltipManager::new(geometry),
            tooltip_source: None,
            tooltip_now_ms: 0,
        }
    }

    // ── Tab management ─────────────────────────────────────────────────────

    /// Adds a tab with the given title and returns its index.
    pub fn add_tab(&mut self, title: impl Into<String>) -> usize {
        let idx = self.tabs.len();
        self.tabs.push(title.into());
        self.groups.push(Vec::new());
        if self.tabs.len() == 1 {
            self.current_tab_index = 0;
        }
        idx
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Removes the tab at the given index.
    ///
    /// The current tab follows the tab it pointed at: removing an *earlier* tab shifts
    /// the rest left, so the index is decremented to keep naming the same tab. Removing
    /// the visible tab clamps to the last one. Either way the picture changed, so a
    /// repaint is requested.
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        let before = self.current_tab_index;
        self.tabs.remove(index);
        self.groups.remove(index);
        if self.tabs.is_empty() {
            self.current_tab_index = 0;
        } else if index < before {
            self.current_tab_index = before - 1;
        } else if self.current_tab_index >= self.tabs.len() {
            self.current_tab_index = self.tabs.len() - 1;
        }
        if self.current_tab_index != before {
            self.current_tab_changed.emit(self.current_tab_index);
        }
        self.base.request_redraw();
    }

    /// Returns the title of the tab at the given index.
    pub fn tab_title(&self, index: usize) -> Option<&str> {
        self.tabs.get(index).map(|s| s.as_str())
    }

    /// Sets the title of the tab at the given index.
    pub fn set_tab_title(&mut self, index: usize, title: impl Into<String>) {
        if let Some(t) = self.tabs.get_mut(index) {
            *t = title.into();
        }
    }

    // ── Group management ───────────────────────────────────────────────────

    /// Adds a group to the specified tab and returns the group index.
    pub fn add_group(&mut self, tab_index: usize, group_title: impl Into<String>) -> usize {
        if tab_index >= self.groups.len() {
            // Grow groups vector if needed
            self.groups.resize_with(tab_index + 1, Vec::new);
            // Also grow tabs
            if tab_index >= self.tabs.len() {
                self.tabs.resize(tab_index + 1, String::new());
            }
        }
        let groups = &mut self.groups[tab_index];
        let idx = groups.len();
        groups.push(RibbonGroup::new(group_title));
        idx
    }

    // ── Item management ────────────────────────────────────────────────────

    /// Adds a small item to the specified group.
    pub fn add_item(
        &mut self,
        tab_index: usize,
        group_index: usize,
        text: impl Into<String>,
    ) -> usize {
        let item = RibbonItem::new(text);
        self.insert_item(tab_index, group_index, item)
    }

    /// Adds a large item to the specified group.
    pub fn add_large_item(
        &mut self,
        tab_index: usize,
        group_index: usize,
        text: impl Into<String>,
    ) -> usize {
        let mut item = RibbonItem::new(text);
        item.set_large(true);
        self.insert_item(tab_index, group_index, item)
    }

    fn insert_item(&mut self, tab_index: usize, group_index: usize, item: RibbonItem) -> usize {
        if let Some(groups) = self.groups.get_mut(tab_index) {
            if let Some(group) = groups.get_mut(group_index) {
                let items = group.items_mut();
                let idx = items.len();
                items.push(item);
                return idx;
            }
        }
        0
    }

    /// Returns the number of items in the specified group.
    pub fn item_count(&self, tab_index: usize, group_index: usize) -> usize {
        self.groups
            .get(tab_index)
            .and_then(|g| g.get(group_index))
            .map(|g| g.items().len())
            .unwrap_or(0)
    }

    /// Sets the enabled state of an item.
    pub fn set_item_enabled(
        &mut self,
        tab_index: usize,
        group_index: usize,
        item_index: usize,
        enabled: bool,
    ) {
        if let Some(item) = self.item_mut(tab_index, group_index, item_index) {
            item.set_enabled(enabled);
        }
    }

    /// Returns enabled state for an item.
    pub fn item_enabled(
        &self,
        tab_index: usize,
        group_index: usize,
        item_index: usize,
    ) -> Option<bool> {
        self.item_ref(tab_index, group_index, item_index).map(|item| item.is_enabled())
    }

    /// Sets the checked state of an item.
    pub fn set_item_checked(
        &mut self,
        tab_index: usize,
        group_index: usize,
        item_index: usize,
        checked: bool,
    ) {
        if let Some(item) = self.item_mut(tab_index, group_index, item_index) {
            if item.is_checkable() {
                item.set_checked(checked);
            }
        }
    }

    /// Returns checked state for an item.
    pub fn item_checked(
        &self,
        tab_index: usize,
        group_index: usize,
        item_index: usize,
    ) -> Option<bool> {
        self.item_ref(tab_index, group_index, item_index).map(|item| item.is_checked())
    }

    fn item_mut(
        &mut self,
        tab_index: usize,
        group_index: usize,
        item_index: usize,
    ) -> Option<&mut RibbonItem> {
        self.groups.get_mut(tab_index)?.get_mut(group_index)?.items_mut().get_mut(item_index)
    }

    /// Returns the item at `(tab_index, group_index, item_index)` without
    /// mutating the ribbon, or `None` when any of the three indices is out of
    /// range — the group and item are addressed by index, not id, so the
    /// indices are only meaningful for the current layout.
    pub fn item_ref(
        &self,
        tab_index: usize,
        group_index: usize,
        item_index: usize,
    ) -> Option<&RibbonItem> {
        self.groups.get(tab_index)?.get(group_index)?.items().get(item_index)
    }

    // ── Tab selection ──────────────────────────────────────────────────────

    /// Sets the current (active) tab index.
    pub fn set_current_tab(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.current_tab_index {
            self.current_tab_index = index;
            self.current_tab_changed.emit(index);
            self.base.request_redraw();
        }
    }

    /// Returns the current (active) tab index.
    pub fn current_tab(&self) -> usize {
        self.current_tab_index
    }

    // ── Expanded / minimized ──────────────────────────────────────────────

    /// Sets the expanded state (panel visible or hidden).
    ///
    /// A same-value write returns early: the flag is what the panel's visibility is drawn from, so
    /// an unchanged one cannot change a pixel.
    pub fn set_expanded(&mut self, expanded: bool) {
        if self.expanded == expanded {
            return;
        }
        self.expanded = expanded;
        if !expanded {
            self.temporarily_expanded = false;
        }
        self.sync_panel_reveal();
        self.base.request_redraw();
    }

    /// Returns whether the ribbon panel is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Sets the minimized state.
    ///
    /// Returns early only when the write cannot change anything: the flag is already what the
    /// caller asked for **and** the panel is not currently expanded. Setting `minimized = true` on a
    /// ribbon that is flagged minimized but has been re-expanded (which `set_expanded(true)` alone
    /// reaches) collapses the panel, so that case must still run.
    pub fn set_minimized(&mut self, minimized: bool) {
        let changes_something = self.minimized != minimized || (minimized && self.expanded);
        if !changes_something {
            return;
        }
        self.minimized = minimized;
        if minimized {
            self.expanded = false;
            self.temporarily_expanded = false;
        }
        self.sync_panel_reveal();
        self.base.request_redraw();
    }

    /// Returns whether the ribbon is minimized.
    pub fn is_minimized(&self) -> bool {
        self.minimized
    }

    /// Removes all tabs, groups, and items.
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.groups.clear();
        self.current_tab_index = 0;
        self.hovered_tab = None;
        self.hovered_item = None;
        self.focused_item = None;
        self.temporarily_expanded = false;
    }

    /// The application button's label, if one is installed.
    pub fn application_button(&self) -> Option<&str> {
        self.application_button.as_deref()
    }

    /// Installs or removes the application button left of the tabs.
    ///
    /// Off by default, so the tab row keeps its original geometry.
    pub fn set_application_button(&mut self, label: Option<String>) {
        if self.application_button == label {
            return;
        }
        self.application_button = label;
        self.base.request_redraw();
    }

    /// The application button's box, or `None` when none is installed.
    pub fn application_button_rect(&self) -> Option<Rect> {
        self.application_button.as_ref()?;
        let row = self.tab_row_rect();
        Some(Rect::new(row.x, row.y, Self::APPLICATION_BUTTON_WIDTH, row.height))
    }

    /// Width the application button reserves on the tab row.
    const APPLICATION_BUTTON_WIDTH: u32 = 48;

    /// The quick-access labels, in order.
    pub fn quick_access(&self) -> &[String] {
        &self.quick_access
    }

    /// Replaces the quick-access command labels.
    ///
    /// The labels are painted at the trailing end of the tab row, left of the minimize
    /// button. An empty list paints nothing, which is the default.
    pub fn set_quick_access(&mut self, labels: Vec<String>) {
        if self.quick_access == labels {
            return;
        }
        self.quick_access = labels;
        self.base.request_redraw();
    }

    /// The attribute-carrying quick-access commands.
    pub fn quick_access_items(&self) -> &[QuickAccessItem] {
        &self.quick_access_items
    }

    /// Replaces the quick-access commands with attribute-carrying items.
    ///
    /// An empty list restores the legacy label layout.
    pub fn set_quick_access_items(&mut self, items: Vec<QuickAccessItem>) {
        if self.quick_access_items == items {
            return;
        }
        self.quick_access_items = items;
        self.quick_access_overflow_open = false;
        self.base.request_redraw();
    }

    /// Whether commands that do not fit collapse behind an overflow marker.
    pub fn quick_access_overflow_menu(&self) -> bool {
        self.quick_access_overflow_menu
    }

    /// Enables or disables the quick-access overflow marker.
    pub fn set_quick_access_overflow_menu(&mut self, enabled: bool) {
        if self.quick_access_overflow_menu == enabled {
            return;
        }
        self.quick_access_overflow_menu = enabled;
        if !enabled {
            self.quick_access_overflow_open = false;
        }
        self.base.request_redraw();
    }

    /// Whether the quick-access overflow panel is open.
    pub fn is_quick_access_overflow_open(&self) -> bool {
        self.quick_access_overflow_open
    }

    /// Indices of the quick-access items the overflow marker hides.
    pub fn quick_access_overflowed_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        for index in self.visible_quick_access_items()..self.quick_access_items.len() {
            indices.push(index);
        }
        indices
    }

    /// Width one quick-access item reserves on the tab row.
    fn quick_access_item_width(item: &QuickAccessItem, font: &crate::core::Font) -> i32 {
        if item.separator {
            return 9;
        }
        let icon = if item.icon.is_empty() { 0 } else { 14 };
        let check = if item.checkable { 12 } else { 0 };
        icon + check + estimate_text_width(&item.label, font, 1.0) as i32 + QUICK_ACCESS_PADDING * 2
    }

    /// The horizontal span the quick-access strip may use, left to right.
    fn quick_access_span(&self) -> (i32, i32) {
        let row = self.tab_row_rect();
        let left = match self.tabs.len().checked_sub(1).and_then(|i| self.tab_rect(i)) {
            Some(last) => last.x + last.width as i32 + 4,
            None => {
                row.x
                    + if self.application_button.is_some() {
                        Self::APPLICATION_BUTTON_WIDTH as i32
                    } else {
                        0
                    }
            }
        };
        let right = row.x + row.width as i32 - MINIMIZE_BUTTON_SIZE - MINIMIZE_MARGIN * 2;
        (left, right.max(left))
    }

    /// How many leading items fit in the quick-access span.
    fn visible_quick_access_items(&self) -> usize {
        if self.quick_access_items.is_empty() {
            return 0;
        }
        let (left, right) = self.quick_access_span();
        let available = (right - left).max(0);
        let font = effective_font(self.style());
        let mut used = 0;
        let mut visible = 0;
        for (index, item) in self.quick_access_items.iter().enumerate() {
            let width = Self::quick_access_item_width(item, font);
            let reserve_overflow =
                if self.quick_access_overflow_menu { QUICK_ACCESS_OVERFLOW_WIDTH } else { 0 };
            if used + width + reserve_overflow > available {
                break;
            }
            used += width + QUICK_ACCESS_GAP;
            visible = index + 1;
        }
        if !self.quick_access_overflow_menu {
            self.quick_access_items.len()
        } else {
            visible
        }
    }

    /// The box of quick-access item `index`, or `None` when it is not visible.
    pub fn quick_access_item_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.visible_quick_access_items() {
            return None;
        }
        let row = self.tab_row_rect();
        let (left, _) = self.quick_access_span();
        let font = effective_font(self.style());
        let mut x = left;
        for item in self.quick_access_items.iter().take(index) {
            x += Self::quick_access_item_width(item, font) + QUICK_ACCESS_GAP;
        }
        let width = Self::quick_access_item_width(&self.quick_access_items[index], font).max(1);
        Some(Rect::new(x, row.y + 2, width as u32, row.height.saturating_sub(4)))
    }

    /// The overflow marker's box, when it is painted.
    pub fn quick_access_overflow_rect(&self) -> Option<Rect> {
        if !self.quick_access_overflow_menu
            || self.quick_access_items.is_empty()
            || self.visible_quick_access_items() >= self.quick_access_items.len()
        {
            return None;
        }
        let row = self.tab_row_rect();
        let (left, right) = self.quick_access_span();
        let x = right.saturating_sub(QUICK_ACCESS_OVERFLOW_WIDTH).max(left);
        Some(Rect::new(
            x,
            row.y + 2,
            QUICK_ACCESS_OVERFLOW_WIDTH as u32,
            row.height.saturating_sub(4),
        ))
    }

    /// The overflow panel's box, when it is open.
    pub fn quick_access_overflow_panel_rect(&self) -> Option<Rect> {
        if !self.quick_access_overflow_open {
            return None;
        }
        let marker = self.quick_access_overflow_rect()?;
        let hidden = self.quick_access_overflowed_indices().len() as u32;
        Some(Rect::new(
            marker.x,
            marker.y + marker.height as i32,
            marker.width.max(120),
            hidden.max(1) * QUICK_ACCESS_OVERFLOW_ROW,
        ))
    }

    /// Emits a quick-access click and toggles a checkable item.
    fn emit_quick_access(&mut self, index: usize) {
        if let Some(item) = self.quick_access_items.get_mut(index) {
            if item.checkable {
                item.checked = !item.checked;
                self.base.request_redraw();
            }
        }
        self.quick_access_clicked.emit(index);
    }

    /// The box of quick-access label `index`, or `None` when out of range.
    pub fn quick_access_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.quick_access.len() {
            return None;
        }
        let row = self.tab_row_rect();
        let reserved = MINIMIZE_BUTTON_SIZE + MINIMIZE_MARGIN * 2;
        let mut right = row.x + row.width as i32 - reserved;
        // Lay the labels out from the end so the row's trailing end is stable.
        for i in (0..self.quick_access.len()).rev() {
            let width = Self::quick_access_width(&self.quick_access[i]);
            let rect = Rect::new(right - width, row.y + 2, width.max(1) as u32, row.height - 4);
            if i == index {
                return Some(rect);
            }
            right = rect.x - 4;
        }
        None
    }

    /// A quick-access label's width from its character count at the tab font.
    fn quick_access_width(label: &str) -> i32 {
        (label.chars().count() as i32 * 7 + 8).max(12)
    }

    /// The tabs marked as context tabs.
    pub fn context_tabs(&self) -> &[usize] {
        &self.context_tabs
    }

    /// Marks tabs as context tabs, which paints them with the accent tint.
    pub fn set_context_tabs(&mut self, indices: Vec<usize>) {
        let mut indices: Vec<usize> =
            indices.into_iter().filter(|index| *index < self.tabs.len()).collect();
        indices.sort_unstable();
        indices.dedup();
        if self.context_tabs == indices {
            return;
        }
        self.context_tabs = indices;
        self.base.request_redraw();
    }

    /// Adds a tab and marks it as a context tab, returning its index.
    pub fn add_context_tab(&mut self, title: impl Into<String>) -> usize {
        let index = self.add_tab(title);
        self.context_tabs.push(index);
        self.context_tabs.sort_unstable();
        index
    }

    /// Whether tab `index` is a context tab.
    pub fn is_context_tab(&self, index: usize) -> bool {
        self.context_tabs.binary_search(&index).is_ok()
    }

    /// The item the keyboard is focused on.
    pub fn focused_item(&self) -> Option<(usize, usize, usize)> {
        self.focused_item
    }

    /// Sets the keyboard-focused item, or clears it with `None`.
    pub fn set_focused_item(&mut self, item: Option<(usize, usize, usize)>) {
        if self.focused_item == item {
            return;
        }
        self.focused_item = item;
        self.base.request_redraw();
    }

    /// Registers the KeyTip string for an item.
    ///
    /// A KeyTip is the short letter sequence Office shows over commands when Alt is
    /// pressed. Matching is by the tip's first character, which is enough for a unique
    /// letter per tab.
    pub fn set_key_tip(&mut self, tab: usize, group: usize, item: usize, tip: impl Into<String>) {
        let tip = tip.into();
        if let Some(slot) =
            self.key_tips.iter_mut().find(|(t, g, i, _)| *t == tab && *g == group && *i == item)
        {
            slot.3 = tip;
        } else {
            self.key_tips.push((tab, group, item, tip));
        }
        self.base.request_redraw();
    }

    /// The KeyTip registered for an item, if any.
    pub fn key_tip(&self, tab: usize, group: usize, item: usize) -> Option<&str> {
        self.key_tips
            .iter()
            .find(|(t, g, i, _)| *t == tab && *g == group && *i == item)
            .map(|(_, _, _, tip)| tip.as_str())
    }

    /// Whether the KeyTip badges are showing.
    pub fn key_tips_visible(&self) -> bool {
        self.key_tips_visible
    }

    /// Shows or hides the KeyTip badges.
    pub fn set_key_tips_visible(&mut self, visible: bool) {
        if self.key_tips_visible == visible {
            return;
        }
        self.key_tips_visible = visible;
        self.base.request_redraw();
    }

    /// Every item address in the current tab, in draw order.
    fn current_tab_items(&self) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        if let Some(groups) = self.groups.get(self.current_tab_index) {
            for (gi, group) in groups.iter().enumerate() {
                for ii in 0..group.items().len() {
                    out.push((self.current_tab_index, gi, ii));
                }
            }
        }
        out
    }

    /// Moves the keyboard focus by `delta` items inside the current tab, wrapping.
    fn move_focus(&mut self, delta: i32) {
        let items = self.current_tab_items();
        if items.is_empty() || delta == 0 {
            return;
        }
        let position =
            self.focused_item.and_then(|focus| items.iter().position(|entry| *entry == focus));
        let start = match position {
            Some(index) => index as i32,
            None if delta > 0 => -1,
            None => items.len() as i32,
        };
        let index = (start + delta).rem_euclid(items.len() as i32) as usize;
        self.focused_item = Some(items[index]);
        self.base.request_redraw();
    }

    /// Triggers the item at `address`, respecting enabled/checkable state.
    fn trigger_item(&mut self, tab: usize, gi: usize, ii: usize) -> bool {
        let Some(item) = self.item_mut(tab, gi, ii) else {
            return false;
        };
        if !item.is_enabled() {
            return false;
        }
        if item.is_checkable() {
            item.set_checked(!item.is_checked());
        }
        self.item_triggered.emit((tab, gi, ii));
        true
    }

    /// The overflow marker's box for group `gi`, when the group could not show all of
    /// its items.
    pub fn group_overflow_rect(&self, gi: usize) -> Option<Rect> {
        let groups = self.groups.get(self.current_tab_index)?;
        let group = groups.get(gi)?;
        // One cached table serves the placed count and the group box; this used to build
        // `group_rects` twice and re-lay out the group's items on every call, and the painter calls
        // it once per group per frame (K-P08).
        let layout = self.item_layout();
        let (_, rect, placed) = layout.iter().find(|(index, _, _)| *index == gi)?;
        if placed.len() >= group.items().len() {
            return None;
        }
        let rect = *rect;
        Some(Rect::new(
            rect.x + rect.width as i32 - 22,
            rect.y + rect.height as i32 - GROUP_TITLE_HEIGHT - 18,
            18,
            16,
        ))
    }

    /// Applies the panel reveal to the current expanded state.
    fn sync_panel_reveal(&mut self) {
        self.panel_reveal.set_open(self.expanded);
    }

    /// Registers the hovered item's tooltip with the hover pipeline (ADV-10).
    ///
    /// `RibbonItem::tooltip` had a field, an accessor and a setter from the start and **no reader**:
    /// the painter and the event path never looked at it, so a caller's hint was silently dropped
    /// and a long item label that the layout truncates could not be read anywhere. The pipeline is
    /// the control's own [`TooltipManager`], the same shape `tool_bar` uses, because a widget cannot
    /// reach a host's manager instance.
    fn sync_hover_tooltip(&mut self) {
        let Some((tab, gi, ii)) = self.hovered_item else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let text = self
            .groups
            .get(tab)
            .and_then(|groups| groups.get(gi))
            .and_then(|group| group.items().get(ii))
            .map(|item| item.tooltip().trim().to_string())
            .filter(|text| !text.is_empty());
        match text {
            Some(text) => {
                let id = hover_id(self.base.id(), tab, gi, ii);
                self.tooltips.register(id, text);
                self.tooltips.note_hover(id);
                self.tooltip_source = Some(id);
            }
            None => {
                if self.tooltip_source.take().is_some() {
                    self.tooltips.note_leave();
                }
            }
        }
    }

    /// Whether a tooltip is still waiting for its delay (or visible).
    fn tooltip_owes_frames(&self) -> bool {
        self.tooltip_source.is_some() || self.tooltips.tooltip().is_some()
    }

    // ── Layout helpers ────────────────────────────────────────────────────

    /// Returns the rectangle for the tab headers row.
    fn tab_row_rect(&self) -> Rect {
        let g = self.geometry();
        Rect::new(g.x, g.y, g.width, TAB_HEIGHT as u32)
    }

    /// Returns the rectangle of a single tab header.
    fn tab_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() {
            return None;
        }
        let row = self.tab_row_rect();
        let total_tabs = self.tabs.len() as i32;
        // Reserve space for minimize button and the application button.
        let reserve = MINIMIZE_BUTTON_SIZE
            + MINIMIZE_MARGIN * 2
            + if self.application_button.is_some() {
                Self::APPLICATION_BUTTON_WIDTH as i32
            } else {
                0
            };
        let avail = (row.width as i32).saturating_sub(reserve);
        if total_tabs == 0 {
            return None;
        }
        // The crate's fit rule (`nav_widgets::tab_bar` documents it): when the tabs do not
        // fit at a readable width they shrink to `available / count` instead of running
        // past the strip, and the spacing counts into the fit — `count` widths plus
        // `count - 1` gaps is what has to land inside the reserve.
        let gaps = (total_tabs - 1) * TAB_SPACING;
        let fit_width = ((avail - gaps).max(total_tabs)) / total_tabs;
        let tab_w = fit_width.max(1) as u32;
        let lead = if self.application_button.is_some() {
            Self::APPLICATION_BUTTON_WIDTH as i32
        } else {
            0
        };
        let x_offset = row.x + lead + index as i32 * (tab_w as i32 + TAB_SPACING);
        Some(Rect::new(x_offset, row.y, tab_w, row.height))
    }

    /// Returns the rectangle for the content panel (below the tab headers).
    fn panel_rect(&self) -> Rect {
        let g = self.geometry();
        let y = g.y + TAB_HEIGHT + PANEL_TOP_MARGIN;
        let h = (g.y + g.height as i32).saturating_sub(y);
        if h < 0 {
            return Rect::new(g.x, y, g.width, 0);
        }
        Rect::new(g.x, y, g.width, h as u32)
    }

    /// Returns the rectangle for the minimize toggle button.
    fn minimize_button_rect(&self) -> Rect {
        let g = self.geometry();
        let x = g.x + g.width as i32 - MINIMIZE_BUTTON_SIZE - MINIMIZE_MARGIN;
        let y = g.y + (TAB_HEIGHT - MINIMIZE_BUTTON_SIZE) / 2;
        Rect::new(x, y, MINIMIZE_BUTTON_SIZE as u32, MINIMIZE_BUTTON_SIZE as u32)
    }

    /// Computes group rectangles for the current tab's content panel.
    /// Returns a vector of (group_index, rect).
    fn group_rects(&self) -> Vec<(usize, Rect)> {
        let panel = self.panel_rect();
        if panel.width == 0 || panel.height == 0 {
            return Vec::new();
        }
        let groups = match self.groups.get(self.current_tab_index) {
            Some(g) if !g.is_empty() => g,
            _ => return Vec::new(),
        };
        let total = groups.len() as i32;
        // Divide available width equally among groups. The paddings count into the fit —
        // `total` widths plus `total` paddings must land inside the panel — so when the
        // equal share would not fit, it shrinks to `available / count` (the rule
        // `nav_widgets::tab_bar` applies to its tabs) instead of laying the last groups out
        // past the panel edge.
        let pad = GROUP_PADDING * 2;
        let avail_w = (panel.width as i32).saturating_sub(pad * total);
        if total == 0 || avail_w <= 0 {
            return Vec::new();
        }
        let group_w = (avail_w / total).max(1) as u32;
        let mut result = Vec::with_capacity(groups.len());
        let mut x = panel.x + GROUP_PADDING;
        for (i, _) in groups.iter().enumerate() {
            let gr = Rect::new(
                x,
                panel.y + GROUP_PADDING,
                group_w,
                panel.height.saturating_sub(GROUP_PADDING as u32 * 2),
            );
            result.push((i, gr));
            x += group_w as i32 + GROUP_PADDING * 2;
        }
        result
    }

    /// A cheap fold over every fact the item layout reads (K-P08).
    ///
    /// The layout is a pure function of the panel rectangle, the current tab, the expand state and
    /// the shape of that tab's groups (each group's item count, whether it is a gallery, and each
    /// item's large/small classification). Folding those into one `u64` is O(items) integer work
    /// with no allocation, which is what makes the cache self-correcting: nothing has to remember
    /// to invalidate it when an item is added, resized or reclassified.
    fn item_layout_signature(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let panel = self.panel_rect();
        for value in [
            panel.x as u64,
            panel.y as u64,
            u64::from(panel.width),
            u64::from(panel.height),
            self.current_tab_index as u64,
            u64::from(self.expanded),
        ] {
            hash ^= value;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        match self.groups.get(self.current_tab_index) {
            Some(groups) => {
                hash ^= groups.len() as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                for group in groups {
                    for value in [group.items().len() as u64, u64::from(group.is_gallery())] {
                        hash ^= value;
                        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                    }
                    for item in group.items() {
                        hash ^= u64::from(item.is_large());
                        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                    }
                }
            }
            None => {
                hash ^= u64::MAX;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }

    /// The group and item boxes for the current layout, from the frame cache (K-P08).
    fn item_layout(&self) -> Rc<Vec<GroupLayout>> {
        let signature = self.item_layout_signature();
        if let Some((cached_signature, layout)) = self.layout_cache.borrow().as_ref() {
            if *cached_signature == signature {
                return Rc::clone(layout);
            }
        }
        let mut layout: Vec<GroupLayout> = Vec::new();
        for (gi, group_rect) in self.group_rects() {
            let rects = self.compute_item_rects(gi, &group_rect);
            layout.push((gi, group_rect, rects));
        }
        let layout = Rc::new(layout);
        *self.layout_cache.borrow_mut() = Some((signature, Rc::clone(&layout)));
        layout
    }

    /// Hit-test: given a position, returns which tab (if any) is under the cursor.
    ///
    /// Uses `Rect::contains_point` so the far edge stays **exclusive**, matching the
    /// crate-wide convention. A hand-written `<= x + width` here made the boundary
    /// column belong to the earlier tab while `draw` paints the later one over it.
    fn hit_tab(&self, pos: Point) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                if tr.contains_point(pos) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Hit-test: given a position, returns which (group, item) is under the cursor.
    fn hit_item(&self, pos: Point) -> Option<(usize, usize, usize)> {
        // `minimized` alone does not hide the items: a tab click temporarily expands the
        // panel over a minimized ribbon (leaving `minimized` set), and the items must hit
        // there. `expanded` is the one flag the picture is drawn from.
        if !self.expanded {
            return None;
        }
        let layout = self.item_layout();
        let groups = self.groups.get(self.current_tab_index)?;
        for (gi, _, item_rects) in layout.iter() {
            let group_ref = match groups.get(*gi) {
                Some(gr) => gr,
                None => continue,
            };
            let items = group_ref.items();
            if items.is_empty() {
                continue;
            }
            for (ii, ir) in item_rects {
                // Exclusive far edge, per `Rect::contains_point`. The hand-written
                // comparison that stood here let the boundary column hit the earlier
                // item even though `draw` paints the later one over it.
                if ir.contains_point(pos) && *ii < items.len() {
                    return Some((self.current_tab_index, *gi, *ii));
                }
            }
        }
        None
    }

    /// Returns rectangles for all items within a group.
    /// Returns (item_index, rect) pairs.
    fn compute_item_rects(&self, group_index: usize, group_rect: &Rect) -> Vec<(usize, Rect)> {
        let groups = match self.groups.get(self.current_tab_index) {
            Some(g) => g,
            None => return Vec::new(),
        };
        let group = match groups.get(group_index) {
            Some(g) => g,
            None => return Vec::new(),
        };
        let items = group.items();
        if items.is_empty() {
            return Vec::new();
        }

        // Reserve bottom area for group title
        let content_h =
            (group_rect.height as i32).saturating_sub(GROUP_TITLE_HEIGHT + ITEM_SPACING);
        if content_h <= 0 {
            return Vec::new();
        }

        let cx = group_rect.x + ITEM_SPACING;
        let cw = (group_rect.width as i32).saturating_sub(ITEM_SPACING * 2);
        if cw <= 0 {
            return Vec::new();
        }

        let mut result = Vec::new();
        // A gallery group lays its items out as a tile grid, not as commands.
        if group.is_gallery() {
            let columns = 3usize;
            let tile_w = (cw / columns as i32).max(1);
            let tile_h = (content_h / 3).max(1);
            let bottom = group_rect.y + group_rect.height as i32 - GROUP_TITLE_HEIGHT;
            for (index, _) in items.iter().enumerate() {
                let col = index % columns;
                let row = index / columns;
                let y = group_rect.y + ITEM_SPACING + row as i32 * tile_h;
                if y + tile_h > bottom {
                    break;
                }
                result.push((
                    index,
                    Rect::new(
                        cx + col as i32 * tile_w,
                        y,
                        tile_w.saturating_sub(2) as u32,
                        tile_h.saturating_sub(2) as u32,
                    ),
                ));
            }
            return result;
        }
        // Separate large and small items
        let large_items: Vec<usize> =
            items.iter().enumerate().filter(|(_, it)| it.is_large()).map(|(i, _)| i).collect();
        let small_items: Vec<usize> =
            items.iter().enumerate().filter(|(_, it)| !it.is_large()).map(|(i, _)| i).collect();

        if large_items.is_empty() && small_items.is_empty() {
            return Vec::new();
        }

        // Layout: Large items first (one per row), then small items in a grid
        let mut current_y = group_rect.y + ITEM_SPACING;

        // Large items, bounded by the group title rule the small rows already honour —
        // without the bound a stack of large items laid itself out past the group and the
        // panel into the window below.
        let bottom = group_rect.y + group_rect.height as i32 - GROUP_TITLE_HEIGHT;
        for &li in &large_items {
            let item_h = LARGE_ICON_SIZE + 16 + ITEM_SPACING; // icon + label + spacing
            if current_y + item_h > bottom {
                break;
            }
            let ir = Rect::new(cx, current_y, cw as u32, item_h as u32);
            result.push((li, ir));
            current_y += item_h + ITEM_SPACING;
        }

        // Small items — arrange in rows of 2 across the group width
        if !small_items.is_empty() {
            let small_per_row = 2usize;
            let small_w = (cw / small_per_row as i32) as u32;
            let small_h = SMALL_ICON_SIZE + 4 + 12; // icon + gap + text

            for (chunk_idx, chunk) in small_items.chunks(small_per_row).enumerate() {
                let y_pos = current_y + chunk_idx as i32 * (small_h + ITEM_SPACING);
                if y_pos + small_h > group_rect.y + group_rect.height as i32 - GROUP_TITLE_HEIGHT {
                    break; // Out of space
                }
                for (col, &si) in chunk.iter().enumerate() {
                    let x_pos = cx + col as i32 * (small_w as i32 + ITEM_SPACING);
                    let ir = Rect::new(x_pos, y_pos, small_w, small_h as u32);
                    result.push((si, ir));
                }
            }
        }

        result
    }

    /// Draws the minimize/expand button.
    fn draw_minimize_button(&self, context: &mut RenderContext, palette: &RibbonPalette) {
        let btn_rect = self.minimize_button_rect();
        let bg = if self.minimize_hovered { palette.hover } else { palette.tab };
        context.fill_rect(btn_rect, bg);
        context.draw_rect(btn_rect, palette.border);

        // Draw arrow: ^ when the panel is shown, v when it is closed
        let mid_x = btn_rect.x + btn_rect.width as i32 / 2;
        let mid_y = btn_rect.y + btn_rect.height as i32 / 2;
        let arrow_color = palette.ink;
        if !self.expanded {
            // Downward arrow (v)
            context.draw_line(
                Point::new(mid_x - 3, mid_y - 1),
                Point::new(mid_x, mid_y + 2),
                arrow_color,
            );
            context.draw_line(
                Point::new(mid_x, mid_y + 2),
                Point::new(mid_x + 3, mid_y - 1),
                arrow_color,
            );
        } else {
            // Upward arrow (^)
            context.draw_line(
                Point::new(mid_x - 3, mid_y + 1),
                Point::new(mid_x, mid_y - 2),
                arrow_color,
            );
            context.draw_line(
                Point::new(mid_x, mid_y - 2),
                Point::new(mid_x + 3, mid_y + 1),
                arrow_color,
            );
        }
    }

    /// Draws a single tab header.
    fn draw_tab(
        &self,
        context: &mut RenderContext,
        index: usize,
        tab_rect: Rect,
        palette: &RibbonPalette,
    ) {
        let is_current = index == self.current_tab_index;
        let is_hovered = self.hovered_tab == Some(index);

        let bg = if is_current {
            palette.surface
        } else if is_hovered {
            palette.hover
        } else if self.is_context_tab(index) {
            // A context tab is a tab whose presence depends on the selection; the accent
            // tint is the standard way to state that without a second tab strip.
            palette.tab.blend(&palette.accent, 0.18)
        } else {
            palette.tab
        };
        let border = palette.border;

        context.fill_rect(tab_rect, bg);
        // Bottom edge of the current tab blends into the panel — only when the panel is
        // actually shown; over a minimized ribbon the active tab is an ordinary tab with a
        // closed bottom edge, or it would read as merging into nothing.
        if is_current && self.expanded {
            // Overdraw bottom border so it merges with the panel
            let bottom =
                Rect::new(tab_rect.x, tab_rect.y + tab_rect.height as i32 - 1, tab_rect.width, 2);
            context.fill_rect(bottom, bg);
        } else {
            context.draw_rect(tab_rect, border);
        }

        // Draw tab title (centered)
        let text = &self.tabs[index];
        // A disabled tab's label is dimmed rather than turned into a fixed grey that only read on
        // the light appearance.
        let text_color = if self.base.is_enabled() { palette.ink } else { palette.muted };
        // A glyph origin is the box's top-left, so the old `tab_rect.y + tab_rect.height / 2`
        // put that top edge on the tab's middle line and drew the title half a line low. The
        // centred line box is derived from the same rectangle the label is centred on.
        // The **effective font** — the resolved theme/caller font — so the tab title honours the
        // theme body font and the user's text scale (D09-STYLE-01).
        let font = effective_font(self.style());
        let line = context.text_line(tab_rect, font);
        let title_band = Rect::new(tab_rect.x, line.y, tab_rect.width, line.height);
        context.draw_text_fitted_cow(
            title_band,
            text,
            font,
            text_color,
            HorizontalAlignment::Center,
        );
    }

    /// Draws the content panel for the current tab (groups + items).
    fn draw_panel(&self, context: &mut RenderContext, palette: &RibbonPalette) {
        // `expanded` is the one flag the picture is drawn from: `set_minimized(true)` forces
        // it down, and a temporarily expanded panel (a tab click over a minimized ribbon)
        // holds it up while `minimized` stays set.
        if !self.expanded && self.panel_reveal.is_closed() {
            return;
        }
        let panel = self.panel_rect();
        if panel.width == 0 || panel.height == 0 {
            return;
        }
        let revealing = !self.panel_reveal.is_closed();
        if revealing {
            let revealed = self.panel_reveal.revealed(
                panel,
                crate::widget::menu_toolbar::popup_reveal::RevealDirection::Down,
                crate::widget::menu_toolbar::popup_reveal::PopupReveal::travel_cap_for(
                    crate::core::Size::new(panel.width, panel.height),
                ),
            );
            context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
        }

        // ── Panel background (gradient-like: a rule, then the body) ──
        let top_strip = Rect::new(panel.x, panel.y, panel.width, 3);
        context.fill_rect(top_strip, palette.hover);
        let body = Rect::new(panel.x, panel.y + 3, panel.width, panel.height.saturating_sub(3));
        context.fill_rect(body, palette.surface);

        // Panel border
        context.draw_rect(Rect::new(panel.x, panel.y, panel.width, panel.height), palette.border);

        // Get current tab groups
        let groups = match self.groups.get(self.current_tab_index) {
            Some(g) if !g.is_empty() => g,
            _ => return,
        };

        let layout = self.item_layout();
        for (gi, gr, item_rects) in layout.iter() {
            let group = &groups[*gi];

            // ── Draw items within this group ──
            for (ii, ir) in item_rects {
                if let Some(item) = groups[*gi].items.get(*ii) {
                    let is_hovered = self.hovered_item == Some((self.current_tab_index, *gi, *ii));

                    // Background
                    let item_bg = if item.is_checked() {
                        palette.item_checked
                    } else if is_hovered && item.is_enabled() {
                        palette.item_hover
                    } else {
                        palette.item
                    };
                    context.fill_rect(*ir, item_bg);

                    // Border on hover/checked
                    if is_hovered || item.is_checked() {
                        context.draw_rect(*ir, palette.accent);
                    }

                    let fg = if !item.is_enabled() { palette.muted } else { palette.ink };

                    if self.focused_item == Some((self.current_tab_index, *gi, *ii)) {
                        context.draw_rect(*ir, palette.accent);
                    }
                    if self.key_tips_visible {
                        if let Some(tip) = self.key_tip(self.current_tab_index, *gi, *ii) {
                            let badge = Rect::new(ir.x + 2, ir.y + 2, 18, 14);
                            context.fill_rect(badge, palette.accent);
                            let font = effective_font(self.style());
                            context.draw_text_fitted_cow(
                                badge,
                                tip,
                                font,
                                palette.accent.contrast_color(),
                                HorizontalAlignment::Center,
                            );
                        }
                    }

                    if item.is_large() {
                        // Large: icon above, label below. Both are centred on the item and the
                        // glyph box starts at the origin, so `Center` is the alignment the item
                        //'s own middle line needs; the old `Left` started every label at the
                        // item's centre, which is why a large item's text ran past its column.
                        let item_font = effective_font(self.style());
                        let mut label_y = ir.y + 2;
                        if let Some(source) = item.icon_source() {
                            let icon_rect = Rect::new(
                                ir.x + (ir.width as i32 - LARGE_ICON_SIZE) / 2,
                                ir.y + 2,
                                LARGE_ICON_SIZE as u32,
                                LARGE_ICON_SIZE as u32,
                            );
                            if let Err(error) = crate::widget::paint_icon(
                                context,
                                source.into(),
                                icon_rect,
                                palette.accent,
                            ) {
                                log::warn!(
                                    "ribbon item ({},{},{}) icon failed: {error:?}",
                                    self.current_tab_index,
                                    gi,
                                    ii
                                );
                            }
                            label_y = ir.y + LARGE_ICON_SIZE + 2;
                        } else if !item.icon_text().is_empty() {
                            let icon_char = item.icon_text().chars().next().unwrap_or(' ');
                            let icon_band =
                                Rect::new(ir.x, ir.y + 2, ir.width, LARGE_ICON_SIZE as u32);
                            context.draw_text_fitted_cow(
                                context.text_line(icon_band, item_font),
                                &icon_char.to_string(),
                                item_font,
                                palette.accent,
                                HorizontalAlignment::Center,
                            );
                            label_y = ir.y + LARGE_ICON_SIZE + 2;
                        }
                        let label_band =
                            Rect::new(ir.x + 2, label_y, ir.width.saturating_sub(4), 16);
                        context.draw_text_fitted_cow(
                            context.text_line(label_band, item_font),
                            item.text(),
                            item_font,
                            fg,
                            HorizontalAlignment::Center,
                        );
                    } else {
                        // Small: icon and text side by side. The icon column is reserved only when
                        // the item actually has an icon; without one the label starts at the
                        // item's own inset instead of at an empty `SMALL_ICON_SIZE` gap.
                        let item_font = effective_font(self.style());
                        let item_line = context.text_line(*ir, item_font);
                        let mut label_x = ir.x + 4;
                        if let Some(source) = item.icon_source() {
                            let side = item_line.height.clamp(1, SMALL_ICON_SIZE as u32);
                            let icon_rect = Rect::new(ir.x + 2, item_line.y, side, side);
                            if let Err(error) = crate::widget::paint_icon(
                                context,
                                source.into(),
                                icon_rect,
                                palette.accent,
                            ) {
                                log::warn!(
                                    "ribbon item ({},{},{}) icon failed: {error:?}",
                                    self.current_tab_index,
                                    gi,
                                    ii
                                );
                            }
                            label_x = ir.x + SMALL_ICON_SIZE + 4;
                        } else if !item.icon_text().is_empty() {
                            let icon_char = item.icon_text().chars().next().unwrap_or(' ');
                            context.draw_text(
                                Point::new(ir.x + 2, item_line.y),
                                &icon_char.to_string(),
                                item_font,
                                palette.accent,
                                HorizontalAlignment::Left,
                            );
                            label_x = ir.x + SMALL_ICON_SIZE + 4;
                        }
                        let label_band = Rect::new(
                            label_x,
                            item_line.y,
                            (ir.x + ir.width as i32 - label_x).max(0) as u32,
                            item_line.height.max(1),
                        );
                        context.draw_text_fitted_cow(
                            label_band,
                            item.text(),
                            item_font,
                            fg,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }

            // ── Group separator line, between groups only ──
            let sep_x = gr.x + gr.width as i32 + GROUP_PADDING;
            if *gi + 1 < groups.len() && sep_x < panel.x + panel.width as i32 {
                context.draw_line(
                    Point::new(sep_x, gr.y),
                    Point::new(sep_x, gr.y + gr.height as i32 - GROUP_TITLE_HEIGHT),
                    palette.border,
                );
            }

            // ── Overflow marker ──
            //
            // A group that could not place every item says so with a marker; a host
            // connects `group_overflow_clicked` to show the rest in a menu.
            if let Some(rect) = self.group_overflow_rect(*gi) {
                context.fill_rect(rect, palette.item_hover);
                let font = effective_font(self.style());
                context.draw_text_fitted_cow(
                    rect,
                    "…",
                    font,
                    palette.ink,
                    HorizontalAlignment::Center,
                );
            }

            // ── Group title at bottom ──
            let title_y = gr.y + gr.height as i32 - GROUP_TITLE_HEIGHT;
            let title_rect = Rect::new(gr.x, title_y, gr.width, GROUP_TITLE_HEIGHT as u32);
            context.fill_rect(title_rect, palette.strip);
            // The group title is a single line in a strip that is only one line tall, so its own
            // centred line box is where the text belongs; `title_y + GROUP_TITLE_HEIGHT / 2` put
            // the glyph box's top edge on the strip's middle line instead.
            let title_font = effective_font(self.style());
            let title_line = context.text_line(title_rect, title_font);
            context.draw_text(
                Point::new(gr.x + 2, title_line.y),
                group.title(),
                title_font,
                palette.muted,
                HorizontalAlignment::Left,
            );
        }
        if revealing {
            context.pop_clip();
        }
    }
}

// ── Widget trait ──────────────────────────────────────────────────────────────

impl Widget for RibbonBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // The width is measured from the tab labels rather than fixed at 800: a ribbon with two
        // short tabs reported the same width as one with ten long ones, so a layout had to
        // shrink or clip it after the fact.
        let font = effective_font(self.style());
        let app = if self.application_button.is_some() {
            Self::APPLICATION_BUTTON_WIDTH as i32
        } else {
            0
        };
        let tabs: i32 = self
            .tabs
            .iter()
            .map(|title| estimate_text_width(title, font, 1.0) as i32 + TAB_TEXT_PADDING * 2)
            .sum::<i32>()
            + TAB_SPACING * self.tabs.len().saturating_sub(1) as i32;
        let quick = self
            .quick_access_items
            .iter()
            .map(|item| Self::quick_access_item_width(item, font) + QUICK_ACCESS_GAP)
            .sum::<i32>();
        let width =
            (app + tabs + quick + MINIMIZE_BUTTON_SIZE + MINIMIZE_MARGIN * 2).max(320) as u32;
        crate::core::Size::new(width, 120)
    }

    /// Advances the panel reveal and the tooltip clocks; `true` while either owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.tooltip_now_ms = self.tooltip_now_ms.saturating_add(u64::from(delta_ms));
        let tooltip_changed = self.tooltips.tick(u64::from(delta_ms), self.tooltip_now_ms);
        self.panel_reveal.tick(delta_ms) || tooltip_changed || self.tooltip_owes_frames()
    }

    /// Whether the panel is mid-transition or a tooltip is pending/visible.
    fn is_animating(&self) -> bool {
        self.panel_reveal.is_animating() || self.tooltip_owes_frames()
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
            "current_tab_changed" => Some(EventSignalRef::mapped(
                "current_tab_changed",
                &self.current_tab_changed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "item_triggered" => {
                Some(EventSignalRef::mapped("item_triggered", &self.item_triggered, |v| {
                    // The declared shape is `Tuple3` of `UInt`, so the payload arrives as the
                    // (tab, group, item) indices rather than its `Debug` spelling — a subscriber
                    // that read the schema can read the three indices as numbers.
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                        CapabilityValue::UInt(v.2 as u64),
                    ]))
                }))
            }
            "application_clicked" => {
                Some(EventSignalRef::unit("application_clicked", &self.application_clicked))
            }
            "group_overflow_clicked" => Some(EventSignalRef::mapped(
                "group_overflow_clicked",
                &self.group_overflow_clicked,
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Int(v.0 as i64),
                        crate::widget::capability::CapabilityValue::Int(v.1 as i64),
                    ]))
                },
            )),
            "quick_access_clicked" => Some(EventSignalRef::mapped(
                "quick_access_clicked",
                &self.quick_access_clicked,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
            "quick_access_overflow_clicked" => Some(EventSignalRef::unit(
                "quick_access_overflow_clicked",
                &self.quick_access_overflow_clicked,
            )),
            _ => None,
        }
    }
}

/// `RibbonBar`'s property contract.
///
/// `tab_count` is derived from the tab vector and therefore read-only;
/// `current_tab` is index-valued and written through `expect_usize`, which
/// rejects a negative or out-of-range `Int` rather than wrapping it.
impl WidgetProperties for RibbonBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tab_count" => Ok(CapabilityValue::UInt(self.tab_count() as u64)),
            "current_tab" => Ok(CapabilityValue::UInt(self.current_tab() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            "minimized" => Ok(CapabilityValue::Bool(self.is_minimized())),
            "quick_access_items" => Ok(CapabilityValue::Tuple(
                self.quick_access_items
                    .iter()
                    .map(|item| {
                        CapabilityValue::String(format!(
                            "{}|{}|{}|{}|{}",
                            item.label,
                            item.icon,
                            item.checkable as u8,
                            item.checked as u8,
                            item.separator as u8
                        ))
                    })
                    .collect(),
            )),
            "quick_access_overflow_menu" => {
                Ok(CapabilityValue::Bool(self.quick_access_overflow_menu()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_tab" => {
                self.set_current_tab(expect_usize(value)?);
                Ok(())
            }
            "expanded" => {
                self.set_expanded(expect_bool(value)?);
                Ok(())
            }
            "minimized" => {
                self.set_minimized(expect_bool(value)?);
                Ok(())
            }
            "quick_access_items" => {
                let mut items = Vec::new();
                for entry in expect_string_list(value)? {
                    let mut parts = entry.splitn(5, '|');
                    let label = parts.next().unwrap_or_default().to_string();
                    let icon = parts.next().unwrap_or_default().to_string();
                    let checkable = parts.next().is_some_and(|token| token == "1");
                    let checked = parts.next().is_some_and(|token| token == "1");
                    let separator = parts.next().is_some_and(|token| token == "1");
                    items.push(QuickAccessItem { label, icon, checkable, checked, separator });
                }
                self.set_quick_access_items(items);
                Ok(())
            }
            "quick_access_overflow_menu" => {
                self.set_quick_access_overflow_menu(expect_bool(value)?);
                Ok(())
            }
            "tab_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tab_count",
            "current_tab",
            "expanded",
            "minimized",
            "quick_access_items",
            "quick_access_overflow_menu",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `ribbon_bar` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every tab,
    /// group and item, which is exactly what [`RibbonBar::clear`] does. `add_tab`
    /// takes a title and `add_group` / `add_item` / `add_large_item` take the tab,
    /// group and label they place, so a payload-less invocation of any of them is
    /// refused as [`CapabilityAccessError::OutOfRange`] — the names are right and the
    /// arguments are what is missing, which is not `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_tab" | "add_group" | "add_item" | "add_large_item" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

// ── EventHandler trait ────────────────────────────────────────────────────────

impl EventHandler for RibbonBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                let prev_tab = self.hovered_tab;
                let prev_item = self.hovered_item;
                let prev_min = self.minimize_hovered;

                self.hovered_tab = self.hit_tab(*pos);
                self.hovered_item = self.hit_item(*pos);
                self.minimize_hovered = self.minimize_button_rect().contains(*pos);

                if prev_tab != self.hovered_tab
                    || prev_item != self.hovered_item
                    || prev_min != self.minimize_hovered
                {
                    // A different item is a different tooltip: re-register before the redraw.
                    if prev_item != self.hovered_item {
                        self.sync_hover_tooltip();
                    }
                    self.base.request_redraw();
                }
                // The bubble follows the pointer; this is the manager's own follow mode.
                self.tooltips.note_pointer(*pos);
            }

            Event::MousePress { pos, button: 1, .. } => {
                if self.application_button_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.application_clicked.emit();
                    return;
                }
                // An open overflow panel takes the press before the items behind it.
                if let Some(panel) = self.quick_access_overflow_panel_rect() {
                    if panel.contains_point(*pos) {
                        let row =
                            ((pos.y - panel.y) / QUICK_ACCESS_OVERFLOW_ROW as i32).max(0) as usize;
                        if let Some(index) =
                            self.quick_access_overflowed_indices().get(row).copied()
                        {
                            self.quick_access_overflow_open = false;
                            self.emit_quick_access(index);
                            return;
                        }
                    } else {
                        self.quick_access_overflow_open = false;
                        self.base.request_redraw();
                    }
                }
                if self.quick_access_overflow_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.quick_access_overflow_open = !self.quick_access_overflow_open;
                    self.quick_access_overflow_clicked.emit();
                    self.base.request_redraw();
                    return;
                }
                for index in 0..self.visible_quick_access_items() {
                    if self
                        .quick_access_item_rect(index)
                        .is_some_and(|rect| rect.contains_point(*pos))
                    {
                        self.emit_quick_access(index);
                        return;
                    }
                }
                for index in 0..self.quick_access.len() {
                    if self.quick_access_rect(index).is_some_and(|rect| rect.contains_point(*pos)) {
                        self.quick_access_clicked.emit(index);
                        return;
                    }
                }
                for gi in 0..self
                    .groups
                    .get(self.current_tab_index)
                    .map(|groups| groups.len())
                    .unwrap_or(0)
                {
                    if self.group_overflow_rect(gi).is_some_and(|rect| rect.contains_point(*pos)) {
                        self.group_overflow_clicked.emit((self.current_tab_index, gi));
                        return;
                    }
                }
                // Check minimize button first
                if self.minimize_button_rect().contains(*pos) {
                    self.minimized = !self.minimized;
                    self.expanded = !self.minimized;
                    self.temporarily_expanded = false;
                    self.sync_panel_reveal();
                    self.base.request_redraw();
                    return;
                }

                // Check tab header click. Over a minimized ribbon a tab click opens that
                // tab's panel temporarily (Office semantics): `minimized` stays set, and
                // the pointer leaving the bar, a second click on the open tab, or picking
                // an item takes the panel back down.
                if let Some(tab_idx) = self.hit_tab(*pos) {
                    if self.current_tab_index != tab_idx {
                        self.set_current_tab(tab_idx);
                        if self.minimized && !self.expanded {
                            self.temporarily_expanded = true;
                            self.expanded = true;
                            self.sync_panel_reveal();
                            self.base.request_redraw();
                        }
                    } else if self.minimized && self.expanded {
                        // The open tab was clicked again: the panel goes back down.
                        self.temporarily_expanded = false;
                        self.expanded = false;
                        self.sync_panel_reveal();
                        self.base.request_redraw();
                    } else if self.minimized {
                        // Clicking the active tab while minimized opens its panel.
                        self.temporarily_expanded = true;
                        self.expanded = true;
                        self.sync_panel_reveal();
                        self.base.request_redraw();
                    }
                    return;
                }

                // Check item click
                if let Some((tab, gi, ii)) = self.hit_item(*pos) {
                    if self.trigger_item(tab, gi, ii) {
                        self.focused_item = Some((tab, gi, ii));
                        // An item chosen from a temporarily opened panel collapses it,
                        // the way the ribbon goes away once the command is picked.
                        if self.temporarily_expanded {
                            self.temporarily_expanded = false;
                            self.expanded = false;
                            self.sync_panel_reveal();
                        }
                        self.base.request_redraw();
                    }
                }
            }

            Event::KeyPress { key, modifiers: _ } => match *key {
                // Alt toggles the KeyTip badges, and F10 focuses the first command.
                18 => self.set_key_tips_visible(!self.key_tips_visible),
                key_codes::F10 => {
                    self.set_key_tips_visible(true);
                    self.focused_item = self.current_tab_items().first().copied();
                    self.base.request_redraw();
                }
                key_codes::ESCAPE => {
                    if self.key_tips_visible {
                        self.set_key_tips_visible(false);
                    } else if self.temporarily_expanded {
                        self.temporarily_expanded = false;
                        self.expanded = false;
                        self.sync_panel_reveal();
                        self.base.request_redraw();
                    } else {
                        self.focused_item = None;
                        self.base.request_redraw();
                    }
                }
                // Left/Right walk the tabs; a minimized ribbon opens the new tab
                // temporarily, the same way a click does.
                key_codes::LEFT | key_codes::RIGHT => {
                    if !self.tabs.is_empty() {
                        let delta = if *key == key_codes::RIGHT { 1 } else { -1 };
                        let next = (self.current_tab_index as i32 + delta)
                            .rem_euclid(self.tabs.len() as i32)
                            as usize;
                        self.set_current_tab(next);
                        self.focused_item = None;
                        if self.minimized {
                            self.temporarily_expanded = true;
                            self.expanded = true;
                            self.sync_panel_reveal();
                        }
                    }
                }
                key_codes::UP => self.move_focus(-1),
                key_codes::DOWN => self.move_focus(1),
                key_codes::HOME => {
                    self.focused_item = self.current_tab_items().first().copied();
                    self.base.request_redraw();
                }
                key_codes::END => {
                    self.focused_item = self.current_tab_items().last().copied();
                    self.base.request_redraw();
                }
                key_codes::ENTER | key_codes::SPACE => {
                    if let Some((tab, gi, ii)) = self.focused_item {
                        self.trigger_item(tab, gi, ii);
                    }
                }
                // A letter while the tips are showing activates the first item whose
                // KeyTip starts with it.
                _ if self.key_tips_visible => {
                    if let Some(letter) = char::from_u32(*key) {
                        let wanted = letter.to_ascii_uppercase();
                        let hit = self.key_tips.iter().find_map(|(tab, gi, ii, tip)| {
                            tip.chars()
                                .next()
                                .is_some_and(|first| first.to_ascii_uppercase() == wanted)
                                .then_some((*tab, *gi, *ii))
                        });
                        if let Some((tab, gi, ii)) = hit {
                            self.set_current_tab(tab);
                            self.trigger_item(tab, gi, ii);
                            self.set_key_tips_visible(false);
                        }
                    }
                }
                _ => {}
            },

            Event::MouseLeave { .. } => {
                // Every hover slot is painted, so leaving has to clear all three — and a
                // panel opened over a minimized ribbon closes when the pointer leaves the
                // bar (Office semantics).
                let changed = self.hovered_tab.is_some()
                    || self.hovered_item.is_some()
                    || self.minimize_hovered
                    || self.temporarily_expanded;
                self.hovered_tab = None;
                self.hovered_item = None;
                self.minimize_hovered = false;
                // Leaving the bar hides any hint; `note_leave` also drops the visible bubble.
                self.tooltip_source = None;
                self.tooltips.note_leave();
                if self.temporarily_expanded {
                    self.temporarily_expanded = false;
                    self.expanded = false;
                    self.sync_panel_reveal();
                }
                if changed {
                    self.base.request_redraw();
                }
            }

            _ => { /* Other events are not relevant */ }
        }
    }
}

// ── Draw trait ────────────────────────────────────────────────────────────────

impl Draw for RibbonBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // Every chrome colour below was a literal, so a light/dark switch left the whole bar
        // unchanged and the rendering census reported the control as theme-blind. The palette
        // resolves the explicit style first, then the theme's resolved style, then a literal.
        let palette = RibbonPalette::resolve(self.base.style());

        // ── Overall background ──
        let g = self.geometry();
        context.fill_rect(g, palette.strip);

        // ── Draw tab headers ──
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                self.draw_tab(context, i, tr, &palette);
            }
        }

        // ── Application button ──
        if let Some(label) = self.application_button.clone() {
            if let Some(rect) = self.application_button_rect() {
                context.fill_rect(rect, palette.surface);
                context.draw_rect(rect, palette.border);
                let font = effective_font(self.style());
                let line = context.text_line(rect, font);
                context.draw_text_fitted_cow(
                    Rect::new(rect.x + 2, line.y, rect.width.saturating_sub(4), line.height),
                    label.as_str(),
                    font,
                    palette.accent,
                    HorizontalAlignment::Center,
                );
            }
        }

        // ── Quick-access labels ──
        if self.quick_access_items.is_empty() {
            for index in 0..self.quick_access.len() {
                if let Some(rect) = self.quick_access_rect(index) {
                    let font = effective_font(self.style());
                    let line = context.text_line(rect, font);
                    context.draw_text_fitted_cow(
                        rect,
                        self.quick_access[index].as_str(),
                        font,
                        palette.ink,
                        HorizontalAlignment::Center,
                    );
                    let _ = line;
                }
            }
        } else {
            let font = effective_font(self.style());
            for index in 0..self.visible_quick_access_items() {
                let Some(rect) = self.quick_access_item_rect(index) else {
                    continue;
                };
                let item = &self.quick_access_items[index];
                if item.separator {
                    let x = rect.x + rect.width as i32 / 2;
                    context.draw_line(
                        Point::new(x, rect.y + 3),
                        Point::new(x, rect.y + rect.height as i32 - 3),
                        palette.border,
                    );
                    continue;
                }
                let mut x = rect.x + QUICK_ACCESS_PADDING;
                let line = context.text_line(rect, font);
                if !item.icon.is_empty() {
                    context.draw_text_fitted_cow(
                        Rect::new(x, line.y, 14, line.height),
                        item.icon.as_str(),
                        font,
                        palette.accent,
                        HorizontalAlignment::Center,
                    );
                    x += 14;
                }
                if item.checkable {
                    let ink = if item.checked { palette.accent } else { palette.muted };
                    context.draw_text_fitted_cow(
                        Rect::new(x, line.y, 12, line.height),
                        if item.checked { "✓" } else { " " },
                        font,
                        ink,
                        HorizontalAlignment::Center,
                    );
                    x += 12;
                }
                context.draw_text_fitted_cow(
                    Rect::new(
                        x,
                        line.y,
                        (rect.x + rect.width as i32 - x).max(0) as u32,
                        line.height,
                    ),
                    item.label.as_str(),
                    font,
                    palette.ink,
                    HorizontalAlignment::Left,
                );
            }
            if let Some(marker) = self.quick_access_overflow_rect() {
                context.fill_rect(marker, palette.tab);
                context.draw_text_fitted_cow(
                    marker,
                    "»",
                    font,
                    palette.ink,
                    HorizontalAlignment::Center,
                );
            }
            if let Some(panel) = self.quick_access_overflow_panel_rect() {
                context.fill_rect(panel, palette.surface);
                context.draw_rect(panel, palette.border);
                for (row, index) in self.quick_access_overflowed_indices().into_iter().enumerate() {
                    let Some(item) = self.quick_access_items.get(index) else {
                        continue;
                    };
                    let band = Rect::new(
                        panel.x + 2,
                        panel.y + row as i32 * QUICK_ACCESS_OVERFLOW_ROW as i32,
                        panel.width.saturating_sub(4),
                        QUICK_ACCESS_OVERFLOW_ROW,
                    );
                    if item.separator {
                        let y = band.y + band.height as i32 / 2;
                        context.draw_line(
                            Point::new(band.x, y),
                            Point::new(band.x + band.width as i32, y),
                            palette.border,
                        );
                    } else {
                        context.draw_text_fitted_cow(
                            band,
                            item.label.as_str(),
                            font,
                            palette.ink,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }
        }

        // ── Draw minimize button ──
        self.draw_minimize_button(context, &palette);

        // ── Draw content panel (if expanded) ──
        self.draw_panel(context, &palette);

        // The tooltip pipeline owns the bubble; keeping it in the same control means a host does
        // not need a second mounted widget to get hint text from the ribbon (ADV-10).
        let geometry = self.geometry();
        self.tooltips.set_geometry(geometry);
        self.tooltips.draw(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ribbon_item_typed_icon_renders_and_legacy_text_replaces_it() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "ribbon-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let tab = ribbon.add_tab("Home");
        let group = ribbon.add_group(tab, "Clipboard");
        let item = ribbon.add_item(tab, group, "Paste");
        let item_ref = ribbon.item_mut(tab, group, item).expect("added ribbon item");
        item_ref.set_icon_source(Some(IconSource::static_geometry(&RING)));
        assert_eq!(item_ref.icon_source(), Some(IconSource::static_geometry(&RING)));
        item_ref.set_icon_text("📋");
        assert_eq!(item_ref.icon_source(), None);
        item_ref.set_icon_source(Some(IconSource::static_geometry(&RING)));

        let svg = crate::widget::svg::render_to_svg(&mut ribbon);
        assert!(svg.contains("fill-rule=\"nonzero\""));
    }

    /// An explicit clear is Empty, not "show whatever came before": the typed setter and the text
    /// setter share one active state, so clearing typed content must not resurrect the text that a
    /// previous typed set had already replaced (blue27 §19.6).
    #[test]
    fn clearing_a_ribbon_item_icon_does_not_resurrect_its_legacy_text() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "ribbon-ring-clear",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let tab = ribbon.add_tab("Home");
        let group = ribbon.add_group(tab, "Clipboard");
        let item = ribbon.add_item(tab, group, "Paste");
        let item_ref = ribbon.item_mut(tab, group, item).expect("added ribbon item");
        assert_eq!(item_ref.icon_text(), "", "an item has no icon until one is set");

        item_ref.set_icon_source(Some(IconSource::static_geometry(&RING)));
        assert_eq!(item_ref.icon_text(), "", "the typed set replaces the legacy text icon");
        item_ref.set_icon_source(None);
        assert_eq!(item_ref.icon_source(), None);
        assert_eq!(item_ref.icon_text(), "", "an explicit clear is Empty, not the old text");
    }

    /// `set_minimized` is idempotent **except** where the collapse is still owed.
    ///
    /// # What this pins
    ///
    /// The setter now returns early on a same-value write (WDGT-15). That short-circuit must not
    /// swallow the one case where re-writing `true` still changes the picture: `set_expanded(true)`
    /// re-opens the panel without clearing the minimized flag, so a following
    /// `set_minimized(true)` is the "collapse it again" command. Guarding on the flag alone made
    /// that call a no-op, which is a visible regression dressed up as an optimisation.
    #[test]
    fn setting_minimized_again_still_collapses_a_re_expanded_panel() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 480, 160));

        assert!(ribbon.is_expanded(), "a fresh ribbon shows its panel");
        ribbon.set_minimized(true);
        assert!(!ribbon.is_expanded(), "minimizing hides the panel");

        // A same-value write while already collapsed has nothing to do.
        ribbon.set_minimized(true);
        assert!(!ribbon.is_expanded());

        // Re-opening the panel leaves the minimized flag set; writing `true` again must collapse it.
        ribbon.set_expanded(true);
        assert!(ribbon.is_expanded());
        assert!(ribbon.is_minimized());
        ribbon.set_minimized(true);
        assert!(!ribbon.is_expanded(), "the second minimize must still collapse the panel");
    }

    #[test]
    fn ribbon_item_state_accessors_handle_valid_and_oob_indices() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let tab = ribbon.add_tab("Home");
        let group = ribbon.add_group(tab, "Clipboard");
        let item = ribbon.add_item(tab, group, "Paste");

        assert_eq!(ribbon.item_enabled(tab, group, item), Some(true));
        ribbon.set_item_enabled(tab, group, item, false);
        assert_eq!(ribbon.item_enabled(tab, group, item), Some(false));
        assert_eq!(ribbon.item_enabled(99, 99, 99), None);

        if let Some(entry) = ribbon.item_mut(tab, group, item) {
            entry.set_checkable(true);
        }
        assert_eq!(ribbon.item_checked(tab, group, item), Some(false));
        ribbon.set_item_checked(tab, group, item, true);
        assert_eq!(ribbon.item_checked(tab, group, item), Some(true));
        assert_eq!(ribbon.item_checked(99, 99, 99), None);
    }

    /// A tab click over a minimized ribbon opens that tab's panel temporarily (Office
    /// semantics), and picking an item from it takes the panel back down.
    #[test]
    fn a_minimized_ribbon_opens_the_clicked_tab_temporarily() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let home = ribbon.add_tab("Home");
        let clipboard = ribbon.add_group(home, "Clipboard");
        let paste = ribbon.add_item(home, clipboard, "Paste");
        ribbon.item_mut(home, clipboard, paste).expect("added item").set_checkable(true);
        let insert = ribbon.add_tab("Insert");
        let tables = ribbon.add_group(insert, "Tables");
        let table = ribbon.add_item(insert, tables, "Table");
        ribbon.item_mut(insert, tables, table).expect("added item").set_checkable(true);

        ribbon.set_minimized(true);
        assert!(!ribbon.is_expanded());

        // Clicking the *other* tab opens its panel; the ribbon stays minimized underneath.
        let tr = ribbon.tab_rect(insert).expect("tab rect");
        ribbon.handle_event(&Event::mouse_press(tr.x + tr.width as i32 / 2, tr.y + 2, 1));
        assert_eq!(ribbon.current_tab(), insert);
        assert!(ribbon.is_expanded(), "the clicked tab's panel opens over the minimized ribbon");
        assert!(ribbon.is_minimized(), "the ribbon stays minimized underneath");

        // The temporary panel's items are hittable, and choosing one collapses the panel.
        let (_, gr) = ribbon.group_rects()[0];
        let rects = ribbon.compute_item_rects(0, &gr);
        let (_, ir) = rects.first().copied().expect("the temporary panel laid out its item");
        ribbon.handle_event(&Event::mouse_press(
            ir.x + ir.width as i32 / 2,
            ir.y + ir.height as i32 / 2,
            1,
        ));
        assert_eq!(
            ribbon.item_checked(insert, tables, 0),
            Some(true),
            "the item was triggered from the temporary panel"
        );
        assert!(!ribbon.is_expanded(), "choosing from the temporary panel collapses it");
        assert!(ribbon.is_minimized());
    }

    /// The pointer leaving the bar clears every hover slot and closes a temporarily opened
    /// panel.
    #[test]
    fn the_pointer_leaving_clears_hover_and_closes_the_temporary_panel() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let home = ribbon.add_tab("Home");
        let clipboard = ribbon.add_group(home, "Clipboard");
        ribbon.add_item(home, clipboard, "Paste");

        let tr = ribbon.tab_rect(home).expect("tab rect");
        ribbon.handle_event(&Event::MouseMove { pos: Point::new(tr.x + 2, tr.y + 2) });
        assert_eq!(ribbon.hovered_tab, Some(home));

        ribbon.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert_eq!(ribbon.hovered_tab, None, "leaving clears the tab hover");
        assert_eq!(ribbon.hovered_item, None);

        // And a panel opened over a minimized ribbon closes when the pointer leaves.
        ribbon.set_minimized(true);
        ribbon.handle_event(&Event::mouse_press(tr.x + 2, tr.y + 2, 1));
        assert!(ribbon.is_expanded(), "the click opened the active tab's panel");
        ribbon.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert!(!ribbon.is_expanded(), "leaving the bar closes the temporary panel");
        assert!(ribbon.is_minimized());
    }

    /// Large items stop at the group title rule; a stack of them must not lay itself out
    /// past the group into the window below.
    #[test]
    fn large_items_that_do_not_fit_stop_at_the_group_title() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let home = ribbon.add_tab("Home");
        let group = ribbon.add_group(home, "Shapes");
        for label in ["One", "Two", "Three", "Four"] {
            ribbon.add_large_item(home, group, label);
        }

        let (_, gr) = ribbon.group_rects()[0];
        let rects = ribbon.compute_item_rects(0, &gr);
        assert!(!rects.is_empty(), "at least one large item fits");
        let bottom = gr.y + gr.height as i32 - GROUP_TITLE_HEIGHT;
        for (_, r) in &rects {
            assert!(
                r.y + r.height as i32 <= bottom,
                "item rect {:?} crosses the title rule at {bottom}",
                r
            );
        }
        assert!(rects.len() < 4, "four large items cannot fit one group: the rest must be dropped");
    }

    /// Group separators are drawn **between** groups only, so a one-group ribbon has no
    /// separator line at all (the SVG backend emits every stroked line as `<line`, and the
    /// minimize button's arrow accounts for the other two).
    #[test]
    fn the_group_separator_appears_only_between_groups() {
        let mut one = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let home = one.add_tab("Home");
        let g = one.add_group(home, "Clipboard");
        one.add_item(home, g, "Paste");
        let lines_one = crate::widget::svg::render_to_svg(&mut one).matches("<line").count();

        let mut two = RibbonBar::new(Rect::new(0, 0, 480, 160));
        let home = two.add_tab("Home");
        let g0 = two.add_group(home, "Clipboard");
        two.add_item(home, g0, "Paste");
        let g1 = two.add_group(home, "Font");
        two.add_item(home, g1, "Bold");
        let lines_two = crate::widget::svg::render_to_svg(&mut two).matches("<line").count();

        assert_eq!(lines_two, lines_one + 1, "exactly one rule was added for the second group");
    }

    /// Tabs (and groups) that cannot fit at a readable width shrink to `available / count`
    /// — the rule `nav_widgets::tab_bar` applies — instead of running past the bar.
    #[test]
    fn tabs_that_cannot_fit_shrink_instead_of_running_past_the_bar() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 200, 160));
        for i in 0..8 {
            ribbon.add_tab(format!("Tab {i}"));
        }
        let geom = ribbon.geometry();
        let reserve = MINIMIZE_BUTTON_SIZE + MINIMIZE_MARGIN * 2;
        let right_limit = geom.x + geom.width as i32 - reserve;
        for i in 0..8 {
            let r = ribbon.tab_rect(i).expect("tab rect");
            assert!(
                r.x >= geom.x && r.x + r.width as i32 <= right_limit,
                "tab {i} at {:?} leaves the strip (limit {right_limit})",
                r
            );
        }

        // The same rule keeps groups inside the panel when their minimums would not fit.
        let tab = ribbon.add_tab("Home");
        for i in 0..5 {
            let g = ribbon.add_group(tab, format!("G{i}"));
            ribbon.add_item(tab, g, "X");
        }
        ribbon.set_current_tab(tab);
        let panel = ribbon.panel_rect();
        for (_, gr) in ribbon.group_rects() {
            assert!(
                gr.x + gr.width as i32 <= panel.x + panel.width as i32,
                "group at {:?} runs past the panel {:?}",
                gr,
                panel
            );
        }
    }

    /// Tabs and commands are reachable from the keyboard, and KeyTips activate.
    #[test]
    fn the_keyboard_walks_tabs_and_commands_and_key_tips_fire() {
        use crate::event::{key_codes, Event, EventHandler};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 600, 160));
        let home = ribbon.add_tab("Home");
        let insert = ribbon.add_tab("Insert");
        let group = ribbon.add_group(home, "Clipboard");
        ribbon.add_item(home, group, "Copy");
        ribbon.add_item(home, group, "Paste");
        ribbon.set_current_tab(home);

        let fired = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&fired);
        ribbon.item_triggered.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        ribbon.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        assert_eq!(ribbon.current_tab(), insert);
        ribbon.handle_event(&Event::key_press(key_codes::LEFT, 0));
        assert_eq!(ribbon.current_tab(), home);

        ribbon.handle_event(&Event::key_press(key_codes::DOWN, 0));
        assert!(ribbon.focused_item().is_some());
        ribbon.handle_event(&Event::key_press(key_codes::ENTER, 0));
        assert_eq!(fired.load(Ordering::SeqCst), 1);

        ribbon.set_key_tip(home, group, 1, "P");
        ribbon.set_key_tips_visible(true);
        ribbon.handle_event(&Event::key_press(80, 0));
        assert_eq!(fired.load(Ordering::SeqCst), 2, "the KeyTip must fire its item");
        assert!(!ribbon.key_tips_visible(), "and then hide the badges");
    }

    /// The application button, quick-access labels and context tabs are opt-in chrome.
    #[test]
    fn application_quick_access_and_context_tabs_are_opt_in() {
        use crate::event::{Event, EventHandler};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 600, 160));
        let home = ribbon.add_tab("Home");
        ribbon.set_current_tab(home);
        assert!(ribbon.application_button_rect().is_none());

        let app_hits = Arc::new(AtomicUsize::new(0));
        let qat_hits = Arc::new(AtomicUsize::new(0));
        let a = Arc::clone(&app_hits);
        let q = Arc::clone(&qat_hits);
        ribbon.application_clicked.connect(move || {
            a.fetch_add(1, Ordering::SeqCst);
        });
        ribbon.quick_access_clicked.connect(move |_| {
            q.fetch_add(1, Ordering::SeqCst);
        });

        ribbon.set_application_button(Some("File".to_string()));
        ribbon.set_quick_access(vec!["Save".to_string(), "Undo".to_string()]);
        let app = ribbon.application_button_rect().expect("the button exists");
        ribbon.handle_event(&Event::mouse_press(app.x + 4, app.y + 4, 1));
        assert_eq!(app_hits.load(Ordering::SeqCst), 1);
        let qat = ribbon.quick_access_rect(1).expect("the second label exists");
        ribbon.handle_event(&Event::mouse_press(qat.x + 2, qat.y + 2, 1));
        assert_eq!(qat_hits.load(Ordering::SeqCst), 1);

        let context = ribbon.add_context_tab("Picture Tools");
        assert!(ribbon.is_context_tab(context));
        assert!(ribbon.tab_rect(context).is_some());
    }

    /// A gallery group lays its items out as a tile grid.
    #[test]
    fn a_gallery_group_uses_a_tile_grid() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 600, 200));
        let tab = ribbon.add_tab("Home");
        let group = ribbon.add_group(tab, "Styles");
        for label in ["A", "B", "C", "D"] {
            ribbon.add_item(tab, group, label);
        }
        ribbon.set_current_tab(tab);
        let before = ribbon.compute_item_rects(0, &ribbon.group_rects()[0].1);
        assert_eq!(before.len(), 4);

        ribbon.groups[tab][group].set_gallery(true);
        let gallery = ribbon.compute_item_rects(0, &ribbon.group_rects()[0].1);
        assert_eq!(gallery.len(), 4);
        assert_eq!(gallery[0].1.x, gallery[1].1.x - gallery[0].1.width as i32 - 2);
        assert!(gallery[3].1.y > gallery[0].1.y, "the fourth tile wraps to a second row");
    }

    /// Minimizing reveals or hides the panel through the shared driver.
    #[test]
    fn minimizing_the_panel_animates() {
        use crate::widget::Widget;

        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 600, 160));
        let tab = ribbon.add_tab("Home");
        let group = ribbon.add_group(tab, "G");
        ribbon.add_item(tab, group, "X");
        ribbon.set_current_tab(tab);
        assert!(ribbon.is_expanded());

        ribbon.set_minimized(true);
        assert!(!ribbon.is_expanded());
        assert!(ribbon.is_animating(), "the collapse is a transition");
        for _ in 0..120 {
            ribbon.tick(16);
        }
        assert!(!ribbon.is_animating());
    }

    #[test]
    fn quick_access_items_overflow_and_the_hint_follows_the_tabs() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 200, 160));
        let narrow = ribbon.size_hint().width;
        for index in 0..8 {
            ribbon.add_tab(format!("A much longer tab title {index}"));
        }
        assert!(ribbon.size_hint().width > narrow, "the hint follows the measured tabs");

        ribbon.set_quick_access_items(vec![
            QuickAccessItem::new("Save").with_icon("S"),
            QuickAccessItem::new("Undo").with_checkable(true),
            QuickAccessItem::separator(),
            QuickAccessItem::new("Redo"),
        ]);
        ribbon.set_quick_access_overflow_menu(true);
        assert_eq!(ribbon.quick_access_items().len(), 4);
        // A very narrow bar cannot fit all four, so the tail becomes overflow.
        ribbon.set_geometry(Rect::new(0, 0, 80, 160));
        assert!(!ribbon.quick_access_overflowed_indices().is_empty());
        assert!(ribbon.quick_access_overflow_rect().is_some());
        assert!(ribbon.event_signal_dyn("quick_access_overflow_clicked").is_some());
    }

    /// A hovered item hands its stored tooltip to the pipeline (ADV-10).
    #[test]
    fn hovering_an_item_registers_its_tooltip() {
        let mut ribbon = RibbonBar::new(Rect::new(0, 0, 600, 200));
        ribbon.add_tab("Home");
        ribbon.add_group(0, "Clipboard");
        let index = ribbon.add_item(0, 0, "Paste");
        ribbon.groups[0][0].items_mut()[index].set_tooltip("Paste the clipboard");
        ribbon.set_expanded(true);

        let layout = ribbon.item_layout();
        let (_, _, items) = layout.first().expect("the group has a layout");
        let item_rect = items.first().expect("the item has a box").1;
        ribbon.handle_event(&Event::mouse_move(item_rect.x + 2, item_rect.y + 2));
        assert!(ribbon.tooltip_source.is_some(), "the hover registered a source");

        // The manager shows it after its delay; one long tick is enough.
        ribbon.tick(1_000);
        assert_eq!(ribbon.tooltips.tooltip(), Some("Paste the clipboard"));

        // And leaving hides it.
        ribbon.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert!(ribbon.tooltips.tooltip().is_none());
    }
}
