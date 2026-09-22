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

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// ── Layout constants ──────────────────────────────────────────────────────────

const TAB_HEIGHT: i32 = 20;
const TAB_MIN_WIDTH: u32 = 50;
const TAB_SPACING: i32 = 1;
const PANEL_TOP_MARGIN: i32 = 2; // gap between tab row and content panel
const GROUP_PADDING: i32 = 4;
const GROUP_MIN_WIDTH: u32 = 80;
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
}

impl RibbonGroup {
    /// Creates a new ribbon group with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), items: Vec::new() }
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
}

/// An item (button) within a ribbon group.
#[derive(Debug, Clone)]
pub struct RibbonItem {
    text: String,
    icon_text: String,
    tooltip: String,
    enabled: bool,
    checkable: bool,
    checked: bool,
    large: bool,
}

impl RibbonItem {
    /// Creates a new ribbon item.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            icon_text: text.clone(),
            text: text.clone(),
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
    hovered_item: Option<(usize, usize, usize)>, // (group, item)
    minimize_hovered: bool,

    /// Emitted when the current tab changes; payload is the new tab index.
    pub current_tab_changed: Signal1<usize>,
    /// Emitted when an item is triggered; payload is (tab, group, item).
    pub item_triggered: Signal1<(usize, usize, usize)>,
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
    /// The theme read is a separate manager lock, taken and released inside
    /// `resolved_theme_style`, so it is not held across the draw — the global manager's mutex is
    /// not re-entrant. The window fill is read as its own scoped acquisition and copied out, so
    /// no guard is held while the rest of the palette is computed.
    fn resolve(style: &crate::style::WidgetStyle) -> Self {
        let theme = crate::style::resolved_theme_style("ribbon_bar");
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
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
            current_tab_changed: Signal1::new(),
            item_triggered: Signal1::new(),
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
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        self.groups.remove(index);
        if self.tabs.is_empty() {
            self.current_tab_index = 0;
        } else if self.current_tab_index >= self.tabs.len() {
            self.current_tab_index = self.tabs.len() - 1;
        }
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
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
        self.base.request_redraw();
    }

    /// Returns whether the ribbon panel is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Sets the minimized state.
    pub fn set_minimized(&mut self, minimized: bool) {
        self.minimized = minimized;
        if minimized {
            self.expanded = false;
        }
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
        // Reserve space for minimize button
        let reserve = MINIMIZE_BUTTON_SIZE + MINIMIZE_MARGIN * 2;
        let avail = (row.width as i32).saturating_sub(reserve);
        if total_tabs == 0 {
            return None;
        }
        let tab_w = (avail / total_tabs).max(TAB_MIN_WIDTH as i32) as u32;
        let x_offset = row.x + index as i32 * (tab_w as i32 + TAB_SPACING);
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
        // Divide available width equally among groups
        let pad = GROUP_PADDING * 2;
        let avail_w = (panel.width as i32).saturating_sub(pad * total);
        if total == 0 || avail_w <= 0 {
            return Vec::new();
        }
        let group_w = (avail_w / total).max(GROUP_MIN_WIDTH as i32) as u32;
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
        if self.minimized || !self.expanded {
            return None;
        }
        for (gi, group) in self.group_rects() {
            let group_ref = match self.groups.get(self.current_tab_index) {
                Some(g) => match g.get(gi) {
                    Some(gr) => gr,
                    None => continue,
                },
                None => continue,
            };
            let items = group_ref.items();
            if items.is_empty() {
                continue;
            }
            // Compute item rectangles within this group
            let item_rects = self.compute_item_rects(gi, &group);
            for (ii, ir) in item_rects {
                // Exclusive far edge, per `Rect::contains_point`. The hand-written
                // comparison that stood here let the boundary column hit the earlier
                // item even though `draw` paints the later one over it.
                if ir.contains_point(pos) && ii < items.len() {
                    return Some((self.current_tab_index, gi, ii));
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

        // Large items
        for &li in &large_items {
            let item_h = LARGE_ICON_SIZE + 16 + ITEM_SPACING; // icon + label + spacing
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

        // Draw arrow: ^ when expanded, v when minimized
        let mid_x = btn_rect.x + btn_rect.width as i32 / 2;
        let mid_y = btn_rect.y + btn_rect.height as i32 / 2;
        let arrow_color = palette.ink;
        if self.minimized {
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
        } else {
            palette.tab
        };
        let border = palette.border;

        context.fill_rect(tab_rect, bg);
        // Bottom edge of current tab blends into panel
        if is_current {
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
        let font = Font::default();
        let line = context.text_line(tab_rect, &font);
        let title_band = Rect::new(tab_rect.x, line.y, tab_rect.width, line.height);
        context.draw_text_fitted(title_band, text, &font, text_color, HorizontalAlignment::Center);
    }

    /// Draws the content panel for the current tab (groups + items).
    fn draw_panel(&self, context: &mut RenderContext, palette: &RibbonPalette) {
        if self.minimized || !self.expanded {
            return;
        }
        let panel = self.panel_rect();
        if panel.width == 0 || panel.height == 0 {
            return;
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

        let group_rects = self.group_rects();
        for (gi, gr) in &group_rects {
            let group = &groups[*gi];

            // ── Draw items within this group ──
            let item_rects = self.compute_item_rects(*gi, gr);
            for (ii, ir) in &item_rects {
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

                    if item.is_large() {
                        // Large: icon text centered, label below
                        let icon_center =
                            Point::new(ir.x + ir.width as i32 / 2, ir.y + LARGE_ICON_SIZE / 2);
                        // Draw a simulated icon area (first character of icon_text)
                        let icon_char = item.icon_text().chars().next().unwrap_or('?');
                        context.draw_text(
                            icon_center,
                            &icon_char.to_string(),
                            &Font::default(),
                            palette.accent,
                            HorizontalAlignment::Left,
                        );
                        // Label below
                        let label_y = ir.y + LARGE_ICON_SIZE + 2;
                        context.draw_text(
                            Point::new(ir.x + ir.width as i32 / 2, label_y),
                            item.text(),
                            &Font::default(),
                            fg,
                            HorizontalAlignment::Left,
                        );
                    } else {
                        // Small: icon and text side by side
                        let icon_char = item.icon_text().chars().next().unwrap_or('?');
                        // The icon and the label share one line box centred in the item, so
                        // the two rule the same row. Halving `ir.height` put the glyph box's
                        // top edge on the item's middle line instead of centring the box.
                        let item_font = Font::default();
                        let item_line = context.text_line(*ir, &item_font);
                        context.draw_text(
                            Point::new(ir.x + 2, item_line.y),
                            &icon_char.to_string(),
                            &item_font,
                            palette.accent,
                            HorizontalAlignment::Left,
                        );
                        context.draw_text(
                            Point::new(ir.x + SMALL_ICON_SIZE + 4, item_line.y),
                            item.text(),
                            &item_font,
                            fg,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }

            // ── Group separator line (right edge) ──
            let sep_x = gr.x + gr.width as i32 + GROUP_PADDING;
            if sep_x < panel.x + panel.width as i32 {
                context.draw_line(
                    Point::new(sep_x, gr.y),
                    Point::new(sep_x, gr.y + gr.height as i32 - GROUP_TITLE_HEIGHT),
                    palette.border,
                );
            }

            // ── Group title at bottom ──
            let title_y = gr.y + gr.height as i32 - GROUP_TITLE_HEIGHT;
            let title_rect = Rect::new(gr.x, title_y, gr.width, GROUP_TITLE_HEIGHT as u32);
            context.fill_rect(title_rect, palette.strip);
            // The group title is a single line in a strip that is only one line tall, so its own
            // centred line box is where the text belongs; `title_y + GROUP_TITLE_HEIGHT / 2` put
            // the glyph box's top edge on the strip's middle line instead.
            let title_font = Font::default();
            let title_line = context.text_line(title_rect, &title_font);
            context.draw_text(
                Point::new(gr.x + 2, title_line.y),
                group.title(),
                &title_font,
                palette.muted,
                HorizontalAlignment::Left,
            );
        }
    }
}

// ── Widget trait ──────────────────────────────────────────────────────────────

impl Widget for RibbonBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(800, 120)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            "tab_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["tab_count", "current_tab", "expanded", "minimized", BASE_PROPERTY_NAMES]
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
                    self.base.request_redraw();
                }
            }

            Event::MousePress { pos, button: 1 } => {
                // Check minimize button first
                if self.minimize_button_rect().contains(*pos) {
                    self.minimized = !self.minimized;
                    self.expanded = !self.minimized;
                    self.base.request_redraw();
                    return;
                }

                // Check tab header click
                if let Some(tab_idx) = self.hit_tab(*pos) {
                    if self.current_tab_index != tab_idx {
                        self.set_current_tab(tab_idx);
                    } else if self.minimized {
                        // Clicking the active tab while minimized expands it
                        self.minimized = false;
                        self.expanded = true;
                        self.base.request_redraw();
                    }
                    return;
                }

                // Check item click
                if let Some((tab, gi, ii)) = self.hit_item(*pos) {
                    if let Some(item) = self.item_mut(tab, gi, ii) {
                        if item.is_enabled() {
                            if item.is_checkable() {
                                item.set_checked(!item.is_checked());
                            }
                            self.item_triggered.emit((tab, gi, ii));
                            self.base.request_redraw();
                        }
                    }
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
        let palette = RibbonPalette::resolve(&self.base.style().clone());

        // ── Overall background ──
        let g = self.geometry();
        context.fill_rect(g, palette.strip);

        // ── Draw tab headers ──
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                self.draw_tab(context, i, tr, &palette);
            }
        }

        // ── Draw minimize button ──
        self.draw_minimize_button(context, &palette);

        // ── Draw content panel (if expanded) ──
        self.draw_panel(context, &palette);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
