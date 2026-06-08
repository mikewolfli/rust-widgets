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
//! │ [A] [B]  │ [C] [D]  │ [E] [F]  │ [G] [H]        │  ← small items row
//! │ ──────── │ ──────── │ ──────── │ ────────        │
//! │ [I] [J]  │          │          │                 │  ← small items row
//! │──────────│──────────│──────────│──────────        │  ← group separator
//! │ Group1   │ Group2   │ Group3   │ Group4           │  ← group title row
//! └──────────────────────────────────────────────────┘

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
    fn hit_tab(&self, pos: Point) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                if pos.x >= tr.x
                    && pos.x <= tr.x + tr.width as i32
                    && pos.y >= tr.y
                    && pos.y <= tr.y + tr.height as i32
                {
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
                if pos.x >= ir.x
                    && pos.x <= ir.x + ir.width as i32
                    && pos.y >= ir.y
                    && pos.y <= ir.y + ir.height as i32
                    && ii < items.len()
                {
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
    fn draw_minimize_button(&self, context: &mut RenderContext) {
        let btn_rect = self.minimize_button_rect();
        let bg = if self.minimize_hovered {
            Color::from_rgb(200, 200, 210)
        } else {
            Color::from_rgb(230, 230, 235)
        };
        context.fill_rect(btn_rect, bg);
        context.draw_rect(btn_rect, Color::from_rgb(180, 180, 190));

        // Draw arrow: ^ when expanded, v when minimized
        let mid_x = btn_rect.x + btn_rect.width as i32 / 2;
        let mid_y = btn_rect.y + btn_rect.height as i32 / 2;
        let arrow_color = Color::from_rgb(60, 60, 60);
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
    fn draw_tab(&self, context: &mut RenderContext, index: usize, tab_rect: Rect) {
        let is_current = index == self.current_tab_index;
        let is_hovered = self.hovered_tab == Some(index);

        let bg = if is_current {
            Color::from_rgb(255, 255, 255)
        } else if is_hovered {
            Color::from_rgb(235, 235, 250)
        } else {
            Color::from_rgb(230, 230, 235)
        };
        let border = if is_current {
            Color::from_rgb(180, 180, 200)
        } else {
            Color::from_rgb(200, 200, 200)
        };

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
        let text_color = if self.base.is_enabled() {
            Color::from_rgb(0, 0, 0)
        } else {
            Color::from_rgb(150, 150, 150)
        };
        context.draw_text(
            Point::new(
                tab_rect.x + tab_rect.width as i32 / 2,
                tab_rect.y + tab_rect.height as i32 / 2,
            ),
            text,
            &Font::default(),
            text_color,
        );
    }

    /// Draws the content panel for the current tab (groups + items).
    fn draw_panel(&self, context: &mut RenderContext) {
        if self.minimized || !self.expanded {
            return;
        }
        let panel = self.panel_rect();
        if panel.width == 0 || panel.height == 0 {
            return;
        }

        // ── Panel background (gradient-like: light gray top, white body) ──
        let top_strip = Rect::new(panel.x, panel.y, panel.width, 3);
        context.fill_rect(top_strip, Color::from_rgb(235, 235, 240));
        let body = Rect::new(panel.x, panel.y + 3, panel.width, panel.height.saturating_sub(3));
        context.fill_rect(body, Color::from_rgb(252, 252, 252));

        // Panel border
        context.draw_rect(
            Rect::new(panel.x, panel.y, panel.width, panel.height),
            Color::from_rgb(200, 200, 200),
        );

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
                        Color::from_rgb(180, 210, 255)
                    } else if is_hovered && item.is_enabled() {
                        Color::from_rgb(210, 230, 255)
                    } else {
                        Color::from_rgb(252, 252, 252)
                    };
                    context.fill_rect(*ir, item_bg);

                    // Border on hover/checked
                    if is_hovered || item.is_checked() {
                        context.draw_rect(*ir, Color::from_rgb(0, 120, 215));
                    }

                    let fg = if !item.is_enabled() {
                        Color::from_rgb(180, 180, 180)
                    } else {
                        Color::from_rgb(0, 0, 0)
                    };

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
                            Color::from_rgb(50, 50, 150),
                        );
                        // Label below
                        let label_y = ir.y + LARGE_ICON_SIZE + 2;
                        context.draw_text(
                            Point::new(ir.x + ir.width as i32 / 2, label_y),
                            item.text(),
                            &Font::default(),
                            fg,
                        );
                    } else {
                        // Small: icon and text side by side
                        let icon_char = item.icon_text().chars().next().unwrap_or('?');
                        context.draw_text(
                            Point::new(ir.x + 2, ir.y + ir.height as i32 / 2),
                            &icon_char.to_string(),
                            &Font::default(),
                            Color::from_rgb(50, 50, 150),
                        );
                        context.draw_text(
                            Point::new(ir.x + SMALL_ICON_SIZE + 4, ir.y + ir.height as i32 / 2),
                            item.text(),
                            &Font::default(),
                            fg,
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
                    Color::from_rgb(200, 200, 200),
                );
            }

            // ── Group title at bottom ──
            let title_y = gr.y + gr.height as i32 - GROUP_TITLE_HEIGHT;
            let title_rect = Rect::new(gr.x, title_y, gr.width, GROUP_TITLE_HEIGHT as u32);
            context.fill_rect(title_rect, Color::from_rgb(245, 245, 248));
            context.draw_text(
                Point::new(gr.x + 2, title_y + GROUP_TITLE_HEIGHT / 2),
                group.title(),
                &Font::default(),
                Color::from_rgb(80, 80, 80),
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
        // ── Overall background ──
        let g = self.geometry();
        context.fill_rect(g, Color::from_rgb(240, 240, 245));

        // ── Draw tab headers ──
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                self.draw_tab(context, i, tr);
            }
        }

        // ── Draw minimize button ──
        self.draw_minimize_button(context);

        // ── Draw content panel (if expanded) ──
        self.draw_panel(context);
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
