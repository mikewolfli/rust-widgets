// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! EmojiPicker — a searchable, categorised grid of glyphs the **caller supplies**.
//!
//! # Why this ships no glyph data
//!
//! This crate bundles no binary assets, and an emoji set is a large one: the
//! Unicode emoji table is thousands of code points plus their names and keywords
//! for search, which would dwarf the library. So this control is the **picker
//! shell** — the grid, the search, the category tabs, the recent list and the
//! keyboard navigation — and the glyphs arrive through `set_glyphs`.
//!
//! (Names are written as code spans rather than intra-doc links: this module is
//! gated on `full_widgets`, and `check_docs.sh` requires the doc set to build under
//! `mini` too, where these types do not exist.)
//!
//! That is not a limitation worked around; it is the correct split. A caller who
//! wants the Unicode set supplies the table, a caller who wants ten custom
//! reactions supplies ten, and this control's behaviour is identical either way.
//!
//! # What a glyph is
//!
//! An `EmojiGlyph` is an id, a display string, a category, and search keywords.
//! The display string is *whatever the caller's font can draw*: this control
//! renders it as text through the ordinary text path, so it supports a Unicode
//! emoji, an ASCII emoticon, a short code, or a single character from a private-use
//! area — with no branch in this module.
//!
//! # Relationship to `Icon`
//!
//! `Icon` draws geometry from a fixed built-in set (`IconName::from_name` matches a
//! closed token table). It is not a glyph renderer, so it cannot stand in for this:
//! the two answer different questions and neither can be expressed in terms of the
//! other.

//! # Reachability
//!
//! Registered in the widget factory as `emoji_picker` (aliases `emojipicker`,
//! `emoji`), so it is reachable by name from the declarative JSON path
//! (`"emojipicker"`), from a CSS selector (`EmojiPicker`), and through the typed
//! `create_emoji_picker` on `ControlBackend`.

use crate::compat::{Cell, HashMap, RefCell};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The category a glyph belongs to, which is what the tabs filter by.
///
/// A plain `String` rather than an enum: categories are the caller's taxonomy
/// (a chat app's set differs from an editor's), and a closed enum would force every
/// caller into this crate's idea of what categories exist.
pub type Category = String;

/// One selectable glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiGlyph {
    /// Stable identifier, unique across the picker.
    pub id: String,
    /// The string drawn in the cell, in whatever form the caller's font supports.
    pub symbol: String,
    /// The category this glyph belongs to; matched against the tab list.
    pub category: Category,
    /// Terms the search matches against, in addition to `symbol`.
    ///
    /// Separate from `symbol` because the thing a user types is rarely the thing
    /// that is drawn: one searches for "happy" to find a face, or "check" to find a
    /// tick. Matching only the symbol would make search nearly useless.
    pub keywords: Vec<String>,
}

impl EmojiGlyph {
    /// Creates a glyph with `category` and no keywords.
    pub fn new(
        id: impl Into<String>,
        symbol: impl Into<String>,
        category: impl Into<Category>,
    ) -> Self {
        Self {
            id: id.into(),
            symbol: symbol.into(),
            category: category.into(),
            keywords: Vec::new(),
        }
    }

    /// Adds one search keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Adds several search keywords.
    pub fn keywords(mut self, keywords: &[&str]) -> Self {
        self.keywords.extend(keywords.iter().map(|keyword| (*keyword).to_string()));
        self
    }

    /// Returns whether this glyph matches `needle`, case-insensitively.
    ///
    /// Matches the symbol, the id, and every keyword, so a caller can search by
    /// whichever of the three they know.
    pub fn matches(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let needle = needle.to_lowercase();
        self.symbol.to_lowercase().contains(&needle)
            || self.id.to_lowercase().contains(&needle)
            || self.keywords.iter().any(|keyword| keyword.to_lowercase().contains(&needle))
    }
}

/// The height of the search box, the tab strip, and one grid cell.
const SEARCH_HEIGHT: u32 = 30;
const TAB_HEIGHT: u32 = 26;
const CELL_SIZE: u32 = 34;

/// Height of the caption strip drawn over the foot of the picker while a glyph is hovered.
const FOOTER_HEIGHT: u32 = 20;

/// How many "recently used" ids the picker remembers.
///
/// Twenty is a full row of the default grid, which is how many a user can see at
/// once without scrolling the recent tab — beyond that the list stops being a
/// shortcut and becomes another search.
const MAX_RECENT: usize = 20;

/// Measured cell glyph extents for one device scale.
///
/// The grid measured every visible cell's text on every frame. A cell is a fixed size and the
/// glyph table changes only through `set_glyphs`, so the width and height of one glyph's text are
/// stable facts about `(glyph, scale)`.
struct GlyphMetricsCache {
    dpi_scale_bits: u32,
    /// `(glyph index, width, height)`, in first-seen order.
    entries: Vec<(usize, u32, u32)>,
}

/// EmojiPicker — a searchable grid of caller-supplied glyphs.
pub struct EmojiPicker {
    base: BaseWidget,
    /// Every glyph the caller supplied, in the order given.
    glyphs: Vec<EmojiGlyph>,
    /// The category tabs, in display order. The pseudo-category
    /// [`Self::RECENT_CATEGORY`] is prepended automatically.
    categories: Vec<Category>,
    /// The selected tab index, into the tab list the picker computes.
    active_tab: usize,
    /// The current search text.
    search: String,
    /// Recently chosen glyph ids, most recent first.
    recent: Vec<String>,
    /// How many recent ids the control remembers.
    recent_capacity: usize,
    /// `id -> index` into [`Self::glyphs`], rebuilt when the table changes.
    ///
    /// The recent projection and `set_glyphs`' own pruning used to scan the whole table per id,
    /// and the projection runs again on the pointer path.
    glyph_index: HashMap<String, usize>,
    /// The visible count and whether it needs recomputing.
    visible_dirty: Cell<bool>,
    visible_count_cache: Cell<usize>,
    /// Measured cell-text extents, keyed by glyph index and device scale.
    glyph_metrics: RefCell<GlyphMetricsCache>,
    /// The grid index the keyboard cursor is on, if any.
    cursor: Option<usize>,
    /// The grid index the pointer is over, if any.
    hovered: Option<usize>,
    /// Scratch index buffer for the visible glyph projection.
    ///
    /// Filled by [`Self::refresh_visible_indices`] and read by the draw path. The projection used
    /// to be a fresh `Vec` per call, and the hit-test and cursor paths each built their own — so
    /// every pointer move or scroll step allocated the window it was about to look at. The
    /// buffer is reused; its length is only meaningful immediately after a refresh.
    visible_indices: Vec<usize>,
    /// Emitted when a glyph is chosen, with its id.
    pub glyph_chosen: Signal1<String>,
    /// Emitted when the hovered glyph changes.
    ///
    /// The payload is the glyph's `keywords` joined with `", "` when it has any,
    /// and its `id` when it has none — the same vocabulary search matches, so a
    /// tooltip and a query agree about what a glyph is called. An empty string
    /// means the pointer is no longer over a glyph.
    pub glyph_hover_changed: Signal1<String>,
}

impl EmojiPicker {
    /// The pseudo-category holding recently chosen glyphs.
    ///
    /// Reserved rather than caller-supplied: it is derived from the picker's own
    /// history, so a caller declaring a category with this name would collide with
    /// behaviour it cannot control.
    pub const RECENT_CATEGORY: &'static str = "__recent__";

    /// Whether the recent tab is shown.
    ///
    /// Hidden until something has been chosen: an empty recent tab on first use is
    /// a dead end a user has to click past.
    fn recent_tab_visible(&self) -> bool {
        !self.recent.is_empty()
    }

    /// Creates an empty picker.
    ///
    /// Defaults: no glyphs, no categories, the first tab selected, no search.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::EmojiPicker, geometry, "EmojiPicker"),
            glyphs: Vec::new(),
            categories: Vec::new(),
            active_tab: 0,
            search: String::new(),
            recent: Vec::new(),
            recent_capacity: MAX_RECENT,
            glyph_index: HashMap::new(),
            visible_dirty: Cell::new(true),
            visible_count_cache: Cell::new(0),
            glyph_metrics: RefCell::new(GlyphMetricsCache {
                dpi_scale_bits: 0,
                entries: Vec::new(),
            }),
            cursor: None,
            hovered: None,
            visible_indices: Vec::new(),
            glyph_chosen: Signal1::new(),
            glyph_hover_changed: Signal1::new(),
        }
    }

    /// Compatibility spelling of [`Self::glyph_hover_changed`] (W-21): the same emitted signal.
    pub fn hovered_glyph_changed(&self) -> &Signal1<String> {
        &self.glyph_hover_changed
    }

    /// Replaces the glyph table.
    ///
    /// The category tabs are **not** derived from the glyphs: a caller that declares
    /// its tab order should keep it, and deriving would reorder the tabs whenever a
    /// glyph was added. Use [`Self::set_categories`] for the tabs.
    pub fn set_glyphs(&mut self, glyphs: Vec<EmojiGlyph>) {
        self.glyphs = glyphs;
        // Rebuild the id lookup once instead of scanning the table per recent id, and drop
        // recents whose id no longer exists.
        self.glyph_index.clear();
        for (index, glyph) in self.glyphs.iter().enumerate() {
            self.glyph_index.entry(glyph.id.clone()).or_insert(index);
        }
        let lookup = &self.glyph_index;
        self.recent.retain(|id| lookup.contains_key(id));
        self.cursor = None;
        self.hovered = None;
        self.invalidate_visible();
        self.glyph_metrics.borrow_mut().entries.clear();
        self.base.request_redraw();
    }

    /// Returns the glyph table.
    pub fn glyphs(&self) -> &[EmojiGlyph] {
        &self.glyphs
    }

    /// Sets the category tabs, in display order.
    pub fn set_categories(&mut self, categories: Vec<Category>) {
        self.categories = categories;
        self.active_tab = 0;
        self.hovered = None;
        self.invalidate_visible();
        self.base.request_redraw();
    }

    /// Returns the caller-declared categories.
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    /// The tab labels the picker actually shows, recent first when non-empty.
    pub fn tabs(&self) -> Vec<Category> {
        let mut tabs = Vec::new();
        if self.recent_tab_visible() {
            tabs.push(Self::RECENT_CATEGORY.to_string());
        }
        tabs.extend(self.categories.iter().cloned());
        tabs
    }

    /// The label of strip tab `index`, recent pseudo-category first when visible.
    ///
    /// [`Self::tabs`] builds an owned `Vec<String>` per call; `draw_tabs` and `tab_at`
    /// asked it every frame and every mouse move. Indexing the two sources directly
    /// keeps the same order with nothing materialised.
    fn tab_label(&self, index: usize) -> Option<&str> {
        if self.recent_tab_visible() {
            if index == 0 {
                return Some(Self::RECENT_CATEGORY);
            }
            self.categories.get(index - 1).map(String::as_str)
        } else {
            self.categories.get(index).map(String::as_str)
        }
    }

    /// How many tabs the strip shows; see [`Self::tab_label`] for the order.
    fn tab_count(&self) -> usize {
        self.recent_tab_visible() as usize + self.categories.len()
    }

    /// The fixed chrome faces. Built once through `OnceLock`: `Font` owns its family
    /// as a `String`, so a constructor call per frame was a fresh allocation for a
    /// face that never changes (the same arrangement `effective_font`'s default uses).
    fn sans_font_11() -> &'static Font {
        static FONT: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        FONT.get_or_init(|| Font::simple("Sans", 11.0))
    }
    fn sans_font_12() -> &'static Font {
        static FONT: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        FONT.get_or_init(|| Font::simple("Sans", 12.0))
    }
    fn sans_font_16() -> &'static Font {
        static FONT: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        FONT.get_or_init(|| Font::simple("Sans", 16.0))
    }

    /// The tab index of the selected tab.
    pub fn active_tab(&self) -> usize {
        self.active_tab
    }

    /// Selects a tab by index, ignoring one that does not exist.
    ///
    /// Selecting a tab clears the search: the two are alternative filters, and
    /// keeping a search while switching tabs would show an empty grid for a
    /// non-empty category, which reads as a bug.
    pub fn set_active_tab(&mut self, index: usize) {
        if index >= self.tabs().len() {
            return;
        }
        self.active_tab = index;
        self.search.clear();
        self.cursor = None;
        self.hovered = None;
        self.invalidate_visible();
        self.base.request_redraw();
    }

    /// The current search text.
    pub fn search(&self) -> &str {
        &self.search
    }

    /// Sets the search text, clearing the cursor.
    pub fn set_search(&mut self, search: &str) {
        if self.search == search && self.cursor.is_none() {
            return;
        }
        self.search = search.to_string();
        self.cursor = None;
        self.hovered = None;
        self.invalidate_visible();
        self.base.request_redraw();
    }

    /// The number of recently chosen ids the control remembers.
    pub fn recent_capacity(&self) -> usize {
        self.recent_capacity
    }

    /// Sets the recent capacity, trimming the list to the newest entries.
    pub fn set_recent_capacity(&mut self, capacity: usize) {
        if self.recent_capacity == capacity {
            return;
        }
        self.recent_capacity = capacity;
        if self.recent.len() > capacity {
            self.recent.truncate(capacity);
            // A trimmed recent list can hide the tab entirely.
            if self.recent.is_empty() {
                self.active_tab = 0;
            }
            self.invalidate_visible();
        }
        self.base.request_redraw();
    }

    /// The ids recently chosen, most recent first.
    pub fn recent(&self) -> &[String] {
        &self.recent
    }

    /// Clears the recent list.
    pub fn clear_recent(&mut self) {
        self.recent.clear();
        self.active_tab = 0;
        self.hovered = None;
        self.invalidate_visible();
        self.base.request_redraw();
    }

    /// The glyphs currently visible, in grid order.
    ///
    /// # The one place the tabs, the search and the grid agree
    ///
    /// The filter itself is defined once, in [`Self::glyph_is_visible`], and every projection
    /// (this accessor, the hit-test, the cursor and the draw path) goes through it — so the
    /// drawing loop, the hit-test and the keyboard cursor cannot disagree about what is on
    /// screen. This allocating form is kept as the public accessor; the internal paths use the
    /// same predicate without materialising a vector.
    ///
    /// A non-empty search overrides the tab: searching means "find this anywhere",
    /// and restricting it to the current category would hide matches the user can
    /// see exist in another tab's label.
    pub fn visible_glyphs(&self) -> Vec<&EmojiGlyph> {
        if self.recent_tab_active() {
            // In recency order, not table order: the list exists to be a shortcut.
            return self
                .recent
                .iter()
                .filter_map(|id| self.glyph_index.get(id))
                .filter_map(|index| self.glyphs.get(*index))
                .collect();
        }
        self.glyphs.iter().filter(|glyph| self.glyph_is_visible(glyph)).collect()
    }

    /// Borrowed form of [`Self::visible_glyphs`]: the visible order as indices into [`Self::glyphs`].
    ///
    /// The buffer is the one the draw path refills, so using this accessor costs no allocation.
    /// The allocating accessor stays available for callers that want glyph references directly.
    pub fn visible_glyphs_slice(&mut self) -> &[usize] {
        self.refresh_visible_indices();
        &self.visible_indices
    }

    /// Whether the recent pseudo-category is the active tab.
    fn recent_tab_active(&self) -> bool {
        self.recent_tab_visible() && self.active_tab == 0
    }

    /// Whether `glyph` belongs to the current projection.
    ///
    /// The one definition of the filter, shared by the allocating accessor and the internal
    /// paths. Ordering is a separate question (the recent tab lists by recency, every other
    /// projection by table order), which is why it is not folded in here.
    fn glyph_is_visible(&self, glyph: &EmojiGlyph) -> bool {
        if !self.search.is_empty() {
            return glyph.matches(&self.search);
        }
        if self.recent_tab_active() {
            return self.recent.iter().any(|id| &glyph.id == id);
        }
        let recent_offset = usize::from(self.recent_tab_visible());
        self.active_tab
            .checked_sub(recent_offset)
            .and_then(|index| self.categories.get(index))
            .is_some_and(|category| &glyph.category == category)
    }

    /// The glyph at grid index `index`, if the grid is that long.
    pub fn glyph_at_index(&self, index: usize) -> Option<&EmojiGlyph> {
        // Walked directly rather than through `visible_glyphs().into_iter().nth(index)`: that
        // materialised the whole projection (and dropped it) to read one element, on the hover
        // and click paths.
        if self.recent_tab_active() {
            return self
                .recent
                .iter()
                .filter_map(|id| self.glyph_index.get(id))
                .filter_map(|index| self.glyphs.get(*index))
                .nth(index);
        }
        self.glyphs.iter().filter(|glyph| self.glyph_is_visible(glyph)).nth(index)
    }

    /// Marks the cached visible count stale; the next `visible_len` recomputes it once.
    fn invalidate_visible(&self) {
        self.visible_dirty.set(true);
    }

    /// The number of glyphs the grid currently shows, without materialising the projection.
    ///
    /// The count is the only thing the hit-test and cursor paths need on every pointer move,
    /// and it used to be an O(glyphs) scan each time. It is recomputed once per filter change
    /// and cached in a `Cell`, because those paths hold `&self`.
    fn visible_len(&self) -> usize {
        if self.visible_dirty.get() {
            let count = if self.recent_tab_active() {
                self.recent.iter().filter(|id| self.glyph_index.contains_key(*id)).count()
            } else {
                self.glyphs.iter().filter(|glyph| self.glyph_is_visible(glyph)).count()
            };
            self.visible_count_cache.set(count);
            self.visible_dirty.set(false);
        }
        self.visible_count_cache.get()
    }

    /// Refills [`Self::visible_indices`] and returns how many entries it holds.
    ///
    /// The order matches [`Self::visible_glyphs`]: recency order for the recent tab, table order
    /// otherwise.
    fn refresh_visible_indices(&mut self) -> usize {
        let mut indices = core::mem::take(&mut self.visible_indices);
        indices.clear();
        if self.recent_tab_active() {
            for id in &self.recent {
                if let Some(index) = self.glyph_index.get(id) {
                    indices.push(*index);
                }
            }
        } else {
            indices.extend(
                self.glyphs
                    .iter()
                    .enumerate()
                    .filter(|(_, glyph)| self.glyph_is_visible(glyph))
                    .map(|(index, _)| index),
            );
        }
        self.visible_indices = indices;
        self.visible_indices.len()
    }

    /// Chooses `glyph`, recording it as recent and emitting it.
    ///
    /// Returns the id, so a caller can chain. The glyph is located by id rather than
    /// taken by reference, which is what lets the caller pass an id it read from
    /// [`Self::recent`].
    pub fn choose(&mut self, glyph_id: &str) -> Option<String> {
        if !self.glyphs.iter().any(|glyph| glyph.id == glyph_id) {
            return None;
        }
        // Most recent first, and never twice: a repeat choice moves the entry to the
        // front rather than duplicating it.
        self.recent.retain(|id| id != glyph_id);
        self.recent.insert(0, glyph_id.to_string());
        self.recent.truncate(self.recent_capacity);
        self.invalidate_visible();
        self.glyph_chosen.emit(glyph_id.to_string());
        self.base.request_redraw();
        Some(glyph_id.to_string())
    }

    /// The keyboard cursor's grid index, if any.
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// The number of cells that fit in one grid row.
    ///
    /// At least one, so a narrow picker still has a row rather than dividing by
    /// zero when the cursor moves vertically.
    fn columns(&self) -> usize {
        let width = self.geometry().width;
        ((width / CELL_SIZE).max(1)) as usize
    }

    /// The number of whole grid rows the control's height can show.
    ///
    /// At least one, so a short picker still has a page to move by and a PageDown
    /// cannot become a no-op merely because the control was laid out small.
    fn visible_rows(&self) -> usize {
        let rect = self.geometry();
        let grid_height = rect.height.saturating_sub(SEARCH_HEIGHT + TAB_HEIGHT);
        (grid_height / CELL_SIZE).max(1) as usize
    }

    /// The caption a hovered glyph contributes to the footer and the hover signal.
    ///
    /// Keywords first because they are the vocabulary a user searches by; the id is
    /// the fallback so every glyph has a name even when the caller supplied none.
    fn glyph_caption(glyph: &EmojiGlyph) -> String {
        if glyph.keywords.is_empty() {
            glyph.id.clone()
        } else {
            glyph.keywords.join(", ")
        }
    }

    /// The rectangle of the grid cell at `index`, given the visible count.
    ///
    /// The count is passed in rather than recomputed: `cell_rect` used to call
    /// `visible_glyphs()` per cell, so painting N cells allocated N projection vectors.
    fn cell_rect_at(&self, index: usize, count: usize) -> Option<Rect> {
        if index >= count {
            return None;
        }
        let rect = self.geometry();
        let columns = self.columns();
        let column = index % columns;
        let row = index / columns;
        let grid_top = rect.y + (SEARCH_HEIGHT + TAB_HEIGHT) as i32;
        Some(Rect::new(
            rect.x + (column as u32 * CELL_SIZE) as i32,
            grid_top + (row as u32 * CELL_SIZE) as i32,
            CELL_SIZE,
            CELL_SIZE,
        ))
    }

    /// The rectangle of the grid cell at `index`.
    ///
    /// Test-only convenience: the paint and hit-test paths pass the visible count to
    /// [`Self::cell_rect_at`] so they do not re-project the visible set per cell.
    #[cfg(test)]
    fn cell_rect(&self, index: usize) -> Option<Rect> {
        let count = self.visible_len();
        self.cell_rect_at(index, count)
    }

    /// The grid index under `pos`, when the pointer is over a cell.
    ///
    /// The cell is computed from the position directly. Scanning every cell (and asking
    /// `cell_rect` per candidate, which re-projected the visible set) made a hit test O(N²).
    fn cell_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        let grid_top = rect.y + (SEARCH_HEIGHT + TAB_HEIGHT) as i32;
        if pos.y < grid_top || pos.x < rect.x {
            return None;
        }
        let columns = self.columns();
        let column = ((pos.x - rect.x) as u32 / CELL_SIZE) as usize;
        if column >= columns {
            return None;
        }
        let row = ((pos.y - grid_top) as u32 / CELL_SIZE) as usize;
        let index = row * columns + column;
        (index < self.visible_len()).then_some(index)
    }

    /// The tab index under `pos`, when the pointer is over the tab strip.
    fn tab_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        let strip = Rect::new(rect.x, rect.y + SEARCH_HEIGHT as i32, rect.width, TAB_HEIGHT);
        if !strip.contains_point(pos) {
            return None;
        }
        // Tabs are laid out left to right at a fixed width, matching `draw_tabs`.
        let offset = (pos.x - strip.x).max(0) as u32;
        let index = (offset / Self::tab_width()) as usize;
        if index < self.tab_count() {
            Some(index)
        } else {
            None
        }
    }

    /// The width one tab occupies.
    fn tab_width() -> u32 {
        72
    }

    /// The search box's rectangle.
    fn search_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, SEARCH_HEIGHT)
    }

    /// Moves the keyboard cursor by `delta` cells, wrapping at the grid's ends.
    ///
    /// # The first press *enters* the grid
    ///
    /// With no cursor set, a forward move lands on cell 0 and a backward move on the
    /// last cell, rather than offsetting from an assumed 0. Offsetting would make
    /// the first Right skip to cell 1, which reads as an off-by-one to a user whose
    /// only intent was "start here".
    ///
    /// Wrapping rather than clamping: a picker is a grid of equals, and stopping at
    /// the last cell would make arrowing through a long list feel broken at the edge.
    fn move_cursor(&mut self, delta: i32) {
        let count = self.visible_len();
        if count == 0 {
            self.cursor = None;
            return;
        }
        let next = match self.cursor {
            Some(current) => (current as i32 + delta).rem_euclid(count as i32),
            // Nothing chosen yet: a forward move enters at the start, a backward one
            // at the end.
            None if delta >= 0 => 0,
            None => count as i32 - 1,
        };
        self.cursor = Some(next as usize);
        self.base.request_redraw();
    }

    /// Moves the cursor by `rows` whole grid rows without wrapping.
    ///
    /// # Why the wheel clamps while the arrow keys wrap
    ///
    /// Arrows are a deliberate walk through a grid of equals, so wrapping at an edge keeps
    /// them useful. A wheel notch is a scroll gesture: continuing to spin it at the top of the
    /// list must not jump the cursor to the bottom, which is what a wrapping move would do. The
    /// cursor starts at the first cell for a downward gesture and at the last for an upward one,
    /// matching [`Self::move_cursor`]'s first-press behaviour.
    ///
    /// Returns whether the cursor moved, so the caller repaints only on a change.
    fn scroll_cursor_rows(&mut self, rows: i32) -> bool {
        let count = self.visible_len();
        if count == 0 {
            let changed = self.cursor.is_some();
            self.cursor = None;
            return changed;
        }
        let step = rows.saturating_mul(self.columns() as i32);
        let next = match self.cursor {
            Some(current) => (current as i32 + step).clamp(0, count as i32 - 1),
            None if step < 0 => count as i32 - 1,
            None => 0,
        } as usize;
        if self.cursor == Some(next) {
            return false;
        }
        self.cursor = Some(next);
        true
    }
}

impl Widget for EmojiPicker {
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
        Size::new(280, 240)
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
            "glyph_chosen" => {
                Some(EventSignalRef::mapped("glyph_chosen", &self.glyph_chosen, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "glyph_hover_changed" => Some(EventSignalRef::mapped(
                "glyph_hover_changed",
                &self.glyph_hover_changed,
                |v| crate::widget::capability::CapabilityValue::String(v.clone()),
            )),
            _ => None,
        }
    }
}

/// `EmojiPicker`'s property contract.
///
/// The glyph table is written through `set_glyphs`; the property layer reports the
/// derived counts, the selected tab and the search, following the same convention
/// as the other list-valued controls.
impl WidgetProperties for EmojiPicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "glyph_count" => Ok(CapabilityValue::UInt(self.glyphs().len() as u64)),
            "visible_glyph_count" => Ok(CapabilityValue::UInt(self.visible_len() as u64)),
            "tab_count" => Ok(CapabilityValue::UInt(self.tabs().len() as u64)),
            "active_tab" => Ok(CapabilityValue::UInt(self.active_tab() as u64)),
            "search" => Ok(CapabilityValue::String(self.search().to_string())),
            "recent_count" => Ok(CapabilityValue::UInt(self.recent().len() as u64)),
            "recent_capacity" => Ok(CapabilityValue::UInt(self.recent_capacity() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "active_tab" => {
                self.set_active_tab(expect_usize(value)?);
                Ok(())
            }
            "search" => {
                let search = expect_string(value)?;
                self.set_search(&search);
                Ok(())
            }
            // Derived from the glyph table and the tabs, which are written through
            // `set_glyphs` / `set_categories` / `choose`.
            "glyph_count" | "visible_glyph_count" | "tab_count" | "recent_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "recent_capacity" => {
                self.set_recent_capacity(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `EMOJI_PICKER_PROPERTIES`.
        property_names_of![
            "glyph_count",
            "visible_glyph_count",
            "tab_count",
            "active_tab",
            "search",
            "recent_count",
            "recent_capacity",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `emoji_picker` publishes.
    ///
    /// `choose` takes the *id* of the glyph to pick, so a bare invocation has
    /// nothing to choose — answering [`CapabilityAccessError::OutOfRange`] says the
    /// name is right and the payload is missing, rather than `UnknownCommand`,
    /// which would deny a command the control really has. `set_glyphs`,
    /// `set_categories` and `set_search` are payload-carrying writes for the same
    /// reason (the trait default would already say `OutOfRange` for the three
    /// `set_*` names; they are listed here so the published set is answered in one
    /// place).
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "choose" | "set_glyphs" | "set_categories" | "set_search" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for EmojiPicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // The picker's shell is chrome — panel, search field, tab strip, and their inks — and
        // resolves through the theme like every other control's. It used to be a set of
        // hardcoded light-theme literals, so the light and dark renders were byte-identical
        // apart from the window behind them: a dark-appearance picker was a white card. Only
        // the glyph cells are content (the caller's symbols), and those are drawn in the
        // resolved ink rather than a fixed colour.
        let (panel, ink, border, field, strip) = self.chrome_colors();
        let style = self.base.style();
        context.face_with_gradient(
            rect,
            panel,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);
        self.draw_search(context, field, ink);
        self.draw_tabs(context, strip, ink, panel);
        // The projection is filled into the control's reusable index buffer instead of a fresh
        // `Vec<&EmojiGlyph>` per frame; the grid only needs to read the indices it contains.
        let visible_count = self.refresh_visible_indices();
        self.draw_grid(context, visible_count, ink);
        self.draw_footer(context, strip, ink);
    }
}

impl EmojiPicker {
    /// The shell's colours, resolved once so the four painters cannot disagree.
    ///
    /// Caller's style first, then the theme's resolved style for `emoji_picker`, then a
    /// literal. The derived steps (field, strip, borders, dim ink) come from the resolved
    /// panel and ink rather than from literals of their own, which is what makes one
    /// appearance swap reach every band.
    fn chrome_colors(&self) -> (Color, Color, Color, Color, Color) {
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("emoji_picker");
        // # Why the panel does not fall back to the window fill
        //
        // `resolved_theme_style("emoji_picker")` cannot help here: `"emoji_picker"` is absent
        // from `WidgetRole`'s table, so the lookup classifies it as `Surface` and resolves to
        // `theme.colors.background` — the window's own colour. Reading that made the panel
        // *equal* to the surface it opened over, i.e. the popup had no visible edge at all.
        //
        // The `surface_container` role is the one step up from the page, which is what a
        // popup's face is: it reads as *on top of* the window rather than as a continuation of
        // it. A window-fill resolution is treated as "no answer" and stepped away from, the
        // same guard `drop_zone` and `otp_input` use, so a theme that genuinely wants the panel
        // flush with the page can still set `background_color` explicitly.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let panel = style
            .background_color
            .filter(|resolved| *resolved != window_fill)
            .or_else(|| crate::style::layer_color(crate::style::LayerColor::SurfaceContainer))
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(252, 252, 254));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or_else(|| panel.contrast_color());
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| panel.blend(&ink, 0.18));
        // The search field and the tab strip are bands *of* the panel, so each is one step
        // toward the ink: a step toward the ink is always visible in either appearance,
        // whereas a literal near-white is only visible on a light panel.
        let field = panel.blend(&ink, 0.05);
        let strip = panel.blend(&ink, 0.08);
        (panel, ink, border, field, strip)
    }

    /// Draws the search box.
    fn draw_search(&self, context: &mut RenderContext, field: Color, ink: Color) {
        let rect = self.search_rect();
        context.fill_rect(rect, field);
        context.draw_line_stroke(
            Point::new(rect.x, rect.y + rect.height as i32),
            Point::new(rect.x + rect.width as i32, rect.y + rect.height as i32),
            ink.blend(&field, 0.7),
            1,
        );
        // A magnifier glyph, drawn as geometry so it needs no font support. It is chrome, so
        // it follows the resolved ink rather than a fixed grey.
        let glyph_ink = ink.blend(&field, 0.45);
        let cx = rect.x + 14;
        let cy = rect.y + rect.height as i32 / 2;
        context.draw_circle_stroke(Point::new(cx, cy - 1), 5, glyph_ink, 1);
        context.draw_line_stroke(
            Point::new(cx + 4, cy + 3),
            Point::new(cx + 8, cy + 7),
            glyph_ink,
            1,
        );

        // Borrowed in both branches: a placeholder is a literal, and the typed text is
        // already owned by the control — cloning it for the draw call allocated a copy
        // of the search string every frame.
        let (text, color) = if self.search.is_empty() {
            // A placeholder is supposed to recede, so it is the resolved ink dimmed toward
            // the field rather than a fixed grey chosen for one appearance.
            ("Search", ink.blend(&field, 0.45))
        } else {
            (self.search.as_str(), ink)
        };
        // The origin is the glyph's **top** edge, so the label is centred by half the
        // difference between the field and the line box. The fixed `+ 20` was written for
        // one font size: at 12 px the glyph box spans 20..32 in a 30 px field, so its bottom
        // two rows were painted over the separator line below.
        let font = Self::sans_font_12();
        let text_h = context.measure_text("M", font).height;
        // Fitted to the field so a long query cannot paint past the box (and under the
        // underline that closes it).
        let search_width = ((rect.width as i32) - 28 - 8).max(0) as u32;
        context.draw_text_fitted_cow(
            Rect::new(
                rect.x + 28,
                rect.y + (rect.height as i32 - text_h as i32) / 2,
                search_width.max(1),
                text_h.max(1),
            ),
            text,
            font,
            color,
            HorizontalAlignment::Left,
        );
    }

    /// Draws the category tab strip.
    fn draw_tabs(&self, context: &mut RenderContext, strip_bg: Color, ink: Color, panel: Color) {
        let rect = self.geometry();
        let strip = Rect::new(rect.x, rect.y + SEARCH_HEIGHT as i32, rect.width, TAB_HEIGHT);
        context.face_with_gradient(
            strip,
            strip_bg,
            self.base.style().background_gradient.as_ref(),
            self.base.style().surface.unwrap_or_default(),
            self.base.style().border_radius.unwrap_or(0),
            crate::core::Color::BLACK,
        );
        // Resolved once per frame, outside the loop: only one tab is active, but the
        // theme lookup used to sit inside the loop body and run once per tab.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or_else(|| ink.legible_on(panel, 3.0));
        // Indexed in place; see `tab_label` for why `tabs()` is not called here.
        for index in 0..self.tab_count() {
            let Some(tab) = self.tab_label(index) else { break };
            let tab_rect = Rect::new(
                strip.x + (index as u32 * Self::tab_width()) as i32,
                strip.y,
                Self::tab_width(),
                TAB_HEIGHT,
            );
            // A tab wider than the strip is clipped rather than drawn past the edge.
            if tab_rect.x >= strip.x + strip.width as i32 {
                break;
            }
            let active = index == self.active_tab;
            if active {
                // The active tab is a raised face of its own strip, so it takes the panel
                // colour one step lighter than the strip in either appearance.
                context.fill_rect(tab_rect, panel);
                context.draw_line_stroke(
                    Point::new(tab_rect.x, tab_rect.y + TAB_HEIGHT as i32 - 2),
                    Point::new(
                        tab_rect.x + tab_rect.width as i32,
                        tab_rect.y + TAB_HEIGHT as i32 - 2,
                    ),
                    accent,
                    2,
                );
            }
            let label: &str = if tab == Self::RECENT_CATEGORY { "Recent" } else { tab };
            // The inactive tabs are secondary by design, so they are the ink dimmed toward
            // the strip rather than a second literal. The label is line-box centred (the
            // fixed `+ 17` was tuned to one font size) and fitted to the tab so a long
            // category name cannot bleed into its neighbours.
            let color = if active { ink } else { ink.blend(&strip_bg, 0.45) };
            let label_font = Self::sans_font_11();
            let label_line = context.text_line(tab_rect, label_font);
            let label_width = tab_rect.width.saturating_sub(16).max(1);
            context.draw_text_fitted_cow(
                Rect::new(tab_rect.x + 8, label_line.y, label_width, label_line.height.max(1)),
                label,
                label_font,
                color,
                HorizontalAlignment::Left,
            );
        }
    }

    /// Draws the hovered glyph's caption over the picker's foot.
    ///
    /// The strip is painted only while a glyph is hovered, so the resting picker is
    /// byte-identical to one without hover support. It is an overlay rather than a
    /// reserved band because reserving one would move every grid row up on controls
    /// that never hover.
    fn draw_footer(&self, context: &mut RenderContext, background: Color, ink: Color) {
        let Some(index) = self.hovered else {
            return;
        };
        let Some(glyph) = self.glyph_at_index(index) else {
            return;
        };
        let caption = Self::glyph_caption(glyph);
        let rect = self.geometry();
        let footer = Rect::new(
            rect.x,
            rect.y + rect.height as i32 - FOOTER_HEIGHT as i32,
            rect.width,
            FOOTER_HEIGHT,
        );
        // A picker too short to have a grid at all has no footer to overlay either.
        if footer.y < rect.y + (SEARCH_HEIGHT + TAB_HEIGHT) as i32 {
            return;
        }
        context.fill_rect(footer, background);
        context.draw_line_stroke(
            Point::new(footer.x, footer.y),
            Point::new(footer.x + footer.width as i32, footer.y),
            ink.blend(&background, 0.7),
            1,
        );
        let font = Self::sans_font_11();
        let line = context.text_line(footer, font);
        context.draw_text_fitted_cow(
            Rect::new(
                footer.x + 10,
                line.y,
                footer.width.saturating_sub(20).max(1),
                line.height.max(1),
            ),
            &caption,
            font,
            ink,
            HorizontalAlignment::Left,
        );
    }

    /// Draws the glyph grid from the indices in [`Self::visible_indices`].
    ///
    /// `visible_count` must come from the refresh that preceded this call. Indices are resolved
    /// against `glyphs` here rather than through a materialised `Vec<&EmojiGlyph>`, so the draw
    /// path allocates nothing for the projection.
    fn draw_grid(&self, context: &mut RenderContext, visible_count: usize, ink: Color) {
        // One font for the whole grid: the cell glyphs used to build a fresh `Font` per cell,
        // twice (once to measure, once to draw), and the placeholder built a third.
        let glyph_font = Self::sans_font_16();
        if visible_count == 0 {
            let rect = self.geometry();
            // The placeholder recedes by alpha, not by being a second literal grey.
            context.draw_text(
                Point::new(rect.x + 10, rect.y + (SEARCH_HEIGHT + TAB_HEIGHT) as i32 + 20),
                "No glyphs",
                Self::sans_font_11(),
                ink.with_alpha(150),
                HorizontalAlignment::Left,
            );
            return;
        }
        // One accent read for the whole grid. It used to be taken inside the selected-cell
        // branch, so a cursor moving across the grid re-locked the manager per frame.
        let accent = crate::style::palette_shared()
            .map(|colors| colors.primary)
            .unwrap_or_else(|| ink.legible_on(Color::rgb(252, 252, 254), 3.0));
        // One measurement per `(glyph, device scale)` rather than one per cell per frame.
        let dpi_scale_bits = context.dpi_scale().to_bits();
        {
            let mut metrics = self.glyph_metrics.borrow_mut();
            if metrics.dpi_scale_bits != dpi_scale_bits {
                metrics.entries.clear();
                metrics.dpi_scale_bits = dpi_scale_bits;
            }
        }
        for index in 0..visible_count {
            let Some(glyph) = self
                .visible_indices
                .get(index)
                .and_then(|glyph_index| self.glyphs.get(*glyph_index))
            else {
                break;
            };
            let Some(cell) = self.cell_rect_at(index, visible_count) else {
                continue;
            };
            // A cell outside the control's own rectangle would paint over a
            // neighbour, so the grid is clipped to the picker. Rows are laid out in order, so
            // the first cell below the control ends the grid ("continue" still walked every
            // remaining off-screen glyph).
            let rect = self.geometry();
            if cell.y + cell.height as i32 > rect.y + rect.height as i32 {
                break;
            }
            let selected = self.cursor == Some(index);
            if selected {
                // The cursor cell is a selection, so it reads the theme's primary token
                // rather than a literal blue.
                context.fill_rounded_rect(cell, 4, accent.with_alpha(48));
            }
            let glyph_index = *self.visible_indices.get(index).unwrap_or(&usize::MAX);
            let (glyph_width, glyph_height) = {
                let cached = self
                    .glyph_metrics
                    .borrow()
                    .entries
                    .iter()
                    .find(|(cached_index, _, _)| *cached_index == glyph_index)
                    .map(|(_, width, height)| (*width, *height));
                match cached {
                    Some(extent) => extent,
                    None => {
                        let measured = context.measure_text(&glyph.symbol, glyph_font);
                        let extent = (measured.width, measured.height);
                        self.glyph_metrics.borrow_mut().entries.push((
                            glyph_index,
                            extent.0,
                            extent.1,
                        ));
                        extent
                    }
                }
            };
            context.draw_text(
                Point::new(
                    cell.x + (cell.width as i32 - glyph_width as i32) / 2,
                    // The origin is the glyph box's top edge, so centring is half the
                    // difference of the line boxes; the old `(cell + ascent)/2` form began
                    // the box a full ascent below the cell's middle.
                    cell.y + (cell.height as i32 - glyph_height as i32) / 2,
                ),
                &glyph.symbol,
                glyph_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for EmojiPicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if self.search_rect().contains_point(*pos) {
                    // Clicking the search box focuses it, which is what makes typing
                    // go there without a separate focus control.
                    self.cursor = None;
                    return;
                }
                if let Some(tab) = self.tab_at(*pos) {
                    self.set_active_tab(tab);
                    return;
                }
                if let Some(index) = self.cell_at(*pos) {
                    self.cursor = Some(index);
                    if let Some(glyph) = self.glyph_at_index(index) {
                        let id = glyph.id.clone();
                        self.choose(&id);
                    }
                }
            }
            Event::TextInput { text } if text.chars().all(|ch| !ch.is_control()) => {
                let mut search = self.search.clone();
                search.push_str(text);
                self.set_search(&search);
            }
            // A pointer over a cell names that glyph; a pointer off the grid clears the
            // caption. The signal fires only on a change, so a sweep across one cell does
            // not repeat the same tooltip every motion event.
            Event::MouseMove { pos } => {
                let next = self.cell_at(*pos);
                if next != self.hovered {
                    self.hovered = next;
                    let caption = next
                        .and_then(|index| self.glyph_at_index(index))
                        .map(Self::glyph_caption)
                        .unwrap_or_default();
                    self.glyph_hover_changed.emit(caption);
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered.take().is_some() {
                    self.glyph_hover_changed.emit(String::new());
                    self.base.request_redraw();
                }
            }
            // One grid row per wheel notch. `delta.y` is measured in notches and
            // `WheelScroll` scales it to lines at the desktop default of three per notch, so the
            // line count is divided back out here to keep the grid step per notch.
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                let rows = scroll.lines_y / 3;
                // A positive notch moves the content down, so the cursor walks toward the top.
                if rows != 0 && self.scroll_cursor_rows(-rows) {
                    self.base.request_redraw();
                }
            }
            Event::KeyDown((key, _)) | Event::KeyPress { key, .. } => match *key {
                // Backspace edits the search, which is the only text this control has.
                8 => {
                    let mut search = self.search.clone();
                    search.pop();
                    self.set_search(&search);
                }
                37 => self.move_cursor(-1), // Left
                39 => self.move_cursor(1),  // Right
                38 => {
                    let columns = self.columns() as i32;
                    self.move_cursor(-columns);
                } // Up
                40 => {
                    let columns = self.columns() as i32;
                    self.move_cursor(columns);
                } // Down
                // PageDown/PageUp move by the rows the picker can actually show, so
                // one keystroke is one screenful rather than a blind fixed count.
                34 => {
                    let rows = self.visible_rows() as i32;
                    if self.scroll_cursor_rows(rows) {
                        self.base.request_redraw();
                    }
                }
                33 => {
                    let rows = self.visible_rows() as i32;
                    if self.scroll_cursor_rows(-rows) {
                        self.base.request_redraw();
                    }
                }
                13 => {
                    // Enter chooses the cursor's glyph, which is how the grid is
                    // driven entirely from the keyboard.
                    if let Some(index) = self.cursor {
                        if let Some(glyph) = self.glyph_at_index(index) {
                            let id = glyph.id.clone();
                            self.choose(&id);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn glyphs() -> Vec<EmojiGlyph> {
        vec![
            EmojiGlyph::new("smile", ":-)", "faces").keywords(&["happy", "smile"]),
            EmojiGlyph::new("sad", ":-(", "faces").keyword("unhappy"),
            EmojiGlyph::new("tick", "v", "symbols").keywords(&["check", "done"]),
            EmojiGlyph::new("cross", "x", "symbols").keyword("no"),
            EmojiGlyph::new("star", "*", "shapes").keyword("favourite"),
        ]
    }

    fn picker() -> EmojiPicker {
        let mut p = EmojiPicker::new(Rect::new(0, 0, 280, 240));
        p.set_glyphs(glyphs());
        p.set_categories(vec!["faces".to_string(), "symbols".to_string(), "shapes".to_string()]);
        p
    }

    /// Renders and returns the RGBA frame.
    fn render(p: &mut EmojiPicker, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        p.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    #[test]
    fn emoji_picker_creation_defaults() {
        let p = EmojiPicker::new(Rect::new(0, 0, 280, 240));
        assert_eq!(p.kind(), WidgetKind::EmojiPicker);
        assert!(p.glyphs().is_empty());
        assert!(p.categories().is_empty());
        assert_eq!(p.active_tab(), 0);
        assert_eq!(p.search(), "");
        assert!(p.recent().is_empty());
        assert_eq!(p.cursor(), None);
    }

    #[test]
    fn emoji_picker_holds_caller_supplied_glyphs() {
        let p = picker();
        assert_eq!(p.glyphs().len(), 5);
        // The symbol is whatever the caller's font can draw, so nothing here parses it.
        assert_eq!(p.glyphs()[0].symbol, ":-)");
    }

    #[test]
    fn emoji_picker_recent_tab_is_hidden_until_something_is_chosen() {
        let mut p = picker();
        assert_eq!(p.tabs().len(), 3, "no recent tab on first use");
        p.choose("tick");
        assert_eq!(p.tabs().len(), 4);
        assert_eq!(p.tabs()[0], EmojiPicker::RECENT_CATEGORY);
    }

    #[test]
    fn emoji_picker_glyph_matches_symbol_id_and_keywords() {
        let glyph = &glyphs()[0];
        assert!(glyph.matches(":-)"), "the symbol matches");
        assert!(glyph.matches("smile"), "the id matches");
        assert!(glyph.matches("happy"), "a keyword matches");
        assert!(glyph.matches("HAPPY"), "matching is case-insensitive");
        assert!(glyph.matches(""), "an empty needle matches everything");
        assert!(!glyph.matches("tick"));
    }

    #[test]
    fn emoji_picker_visible_glyphs_follow_the_active_tab() {
        let mut p = picker();
        let faces = p.visible_glyphs();
        assert_eq!(faces.len(), 2);
        assert_eq!(faces[0].id, "smile");

        p.set_active_tab(1); // symbols
        let symbols = p.visible_glyphs();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].id, "tick");
    }

    #[test]
    fn emoji_picker_visible_glyphs_slice_matches_the_allocating_accessor() {
        let mut p = picker();
        let expected: Vec<String> = p.visible_glyphs().iter().map(|g| g.id.clone()).collect();
        let indices: Vec<usize> = p.visible_glyphs_slice().to_vec();
        let ids: Vec<String> =
            indices.iter().filter_map(|&i| p.glyphs().get(i)).map(|g| g.id.clone()).collect();
        assert_eq!(ids, expected);

        p.set_active_tab(1); // symbols
        let expected: Vec<String> = p.visible_glyphs().iter().map(|g| g.id.clone()).collect();
        let indices: Vec<usize> = p.visible_glyphs_slice().to_vec();
        let ids: Vec<String> =
            indices.iter().filter_map(|&i| p.glyphs().get(i)).map(|g| g.id.clone()).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn emoji_picker_visible_glyphs_overrides_the_tab_when_searching() {
        let mut p = picker();
        p.set_active_tab(1); // symbols
        p.set_search("favourite");
        // Searching means "find this anywhere"; restricting to the tab would hide a
        // match the user can see exists in another tab.
        let found = p.visible_glyphs();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "star");
    }

    #[test]
    fn emoji_picker_search_matches_keywords_not_just_symbols() {
        let mut p = picker();
        p.set_search("check");
        let found = p.visible_glyphs();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "tick");
    }

    #[test]
    fn emoji_picker_search_with_no_match_yields_nothing() {
        let mut p = picker();
        p.set_search("zzzz");
        assert!(p.visible_glyphs().is_empty());
    }

    #[test]
    fn emoji_picker_selecting_a_tab_clears_the_search() {
        let mut p = picker();
        p.set_search("happy");
        p.set_active_tab(1);
        // Keeping a search while switching tabs would show an empty grid for a
        // non-empty category, which reads as a bug.
        assert_eq!(p.search(), "");
    }

    #[test]
    fn emoji_picker_set_active_tab_ignores_an_unknown_index() {
        let mut p = picker();
        p.set_active_tab(9);
        assert_eq!(p.active_tab(), 0);
    }

    #[test]
    fn emoji_picker_choose_records_recency_most_recent_first() {
        let mut p = picker();
        p.choose("smile");
        p.choose("tick");
        p.choose("sad");
        assert_eq!(p.recent(), &["sad", "tick", "smile"]);
    }

    #[test]
    fn emoji_picker_choosing_twice_moves_to_the_front_without_duplicating() {
        let mut p = picker();
        p.choose("smile");
        p.choose("tick");
        p.choose("smile");
        assert_eq!(p.recent(), &["smile", "tick"], "no duplicate entry");
    }

    #[test]
    fn emoji_picker_recent_tab_lists_glyphs_in_recency_order() {
        let mut p = picker();
        p.choose("tick");
        p.choose("smile");
        p.set_active_tab(0); // the recent tab
        let recent = p.visible_glyphs();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, "smile");
        assert_eq!(recent[1].id, "tick");
    }

    #[test]
    fn emoji_picker_recent_caps_its_length() {
        let mut p = EmojiPicker::new(Rect::new(0, 0, 280, 240));
        // More glyphs than the cap.
        let many: Vec<EmojiGlyph> =
            (0..30).map(|index| EmojiGlyph::new(format!("g{index}"), "o", "all")).collect();
        p.set_glyphs(many);
        p.set_categories(vec!["all".to_string()]);
        for index in 0..30 {
            p.choose(&format!("g{index}"));
        }
        assert_eq!(p.recent().len(), MAX_RECENT);
    }

    #[test]
    fn emoji_picker_choose_rejects_an_unknown_id() {
        let mut p = picker();
        assert_eq!(p.choose("nope"), None);
        assert!(p.recent().is_empty());
    }

    #[test]
    fn emoji_picker_glyph_chosen_signal_carries_the_id() {
        let mut p = picker();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        p.glyph_chosen.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*id).clone());
            }
        });
        p.choose("smile");
        p.choose("tick");
        assert_eq!(*seen.lock().expect("signal lock poisoned"), vec!["smile", "tick"]);
    }

    #[test]
    fn emoji_picker_set_glyphs_drops_vanished_recents() {
        let mut p = picker();
        p.choose("smile");
        p.choose("tick");
        // A recent id whose glyph is gone would emit something unresolvable.
        p.set_glyphs(vec![EmojiGlyph::new("tick", "v", "symbols")]);
        assert_eq!(p.recent(), &["tick"]);
    }

    #[test]
    fn emoji_picker_clear_recent_resets_the_tab() {
        let mut p = picker();
        p.choose("smile");
        p.set_active_tab(1);
        p.clear_recent();
        assert!(p.recent().is_empty());
        assert_eq!(p.active_tab(), 0, "the recent tab is gone, so the selection moves");
        assert_eq!(p.tabs().len(), 3);
    }

    #[test]
    fn emoji_picker_categories_are_not_derived_from_glyphs() {
        let mut p = EmojiPicker::new(Rect::new(0, 0, 280, 240));
        p.set_categories(vec!["z".to_string(), "a".to_string()]);
        p.set_glyphs(glyphs());
        // The caller's tab order is kept: deriving would reorder whenever a glyph
        // was added.
        assert_eq!(p.categories(), &["z", "a"]);
    }

    // ── Interaction ─────────────────────────────────────────────────────────

    #[test]
    fn emoji_picker_click_on_a_cell_chooses_it() {
        let mut p = picker();
        // The first cell is the first face.
        let cell = p.cell_rect(0).expect("cell 0");
        p.handle_event(&Event::mouse_press(cell.x + 10, cell.y + 10, 1));
        assert_eq!(p.recent().first().map(String::as_str), Some("smile"));
        assert_eq!(p.cursor(), Some(0));
    }

    #[test]
    fn emoji_picker_click_on_a_tab_switches_it() {
        let mut p = picker();
        let rect = p.geometry();
        // The second tab starts one tab-width in.
        let x = rect.x + EmojiPicker::tab_width() as i32 + 10;
        p.handle_event(&Event::mouse_press(x, rect.y + SEARCH_HEIGHT as i32 + 10, 1));
        assert_eq!(p.active_tab(), 1);
    }

    #[test]
    fn emoji_picker_click_past_the_last_tab_does_nothing() {
        let mut p = picker();
        let rect = p.geometry();
        let x = rect.x + (EmojiPicker::tab_width() * 10) as i32;
        p.handle_event(&Event::mouse_press(x, rect.y + SEARCH_HEIGHT as i32 + 10, 1));
        assert_eq!(p.active_tab(), 0);
    }

    #[test]
    fn emoji_picker_typing_filters_the_grid() {
        let mut p = picker();
        for ch in ["c", "h", "e", "c", "k"] {
            p.handle_event(&Event::TextInput { text: ch.to_string() });
        }
        assert_eq!(p.search(), "check");
        assert_eq!(p.visible_glyphs().len(), 1);

        p.handle_event(&Event::KeyDown((8, 0)));
        assert_eq!(p.search(), "chec");
    }

    #[test]
    fn emoji_picker_ctrl_characters_are_not_searched_for() {
        let mut p = picker();
        p.handle_event(&Event::TextInput { text: "\u{7}".to_string() });
        assert_eq!(p.search(), "", "a control character must not enter the query");
    }

    #[test]
    fn emoji_picker_arrow_keys_move_the_cursor_and_wrap() {
        let mut p = picker();
        let count = p.visible_glyphs().len();
        assert_eq!(count, 2);

        p.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(p.cursor(), Some(0));
        p.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(p.cursor(), Some(1));
        // Wrapping at the end rather than stopping: a grid of equals should not feel
        // broken at its edge.
        p.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(p.cursor(), Some(0));
        p.handle_event(&Event::KeyDown((37, 0)));
        assert_eq!(p.cursor(), Some(1));
    }

    #[test]
    fn emoji_picker_vertical_arrows_move_by_a_row() {
        let mut p = EmojiPicker::new(Rect::new(0, 0, 280, 240));
        p.set_glyphs(glyphs());
        p.set_categories(vec!["faces".to_string()]);
        let count = p.visible_glyphs().len();
        assert_eq!(count, 2);
        // The fixture is wide enough for several columns, so both faces sit in one
        // row and a vertical move wraps within it.
        assert!(p.columns() >= 2, "the fixture must be wider than one cell");

        p.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(p.cursor(), Some(0));
        // Down moves by a whole row, which at this width wraps back to 0.
        p.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(p.cursor(), Some(0));
    }

    #[test]
    fn emoji_picker_enter_chooses_the_cursor_glyph() {
        let mut p = picker();
        p.handle_event(&Event::KeyDown((39, 0))); // cursor to 0
        p.handle_event(&Event::KeyDown((13, 0)));
        assert_eq!(p.recent().first().map(String::as_str), Some("smile"));
    }

    /// The wheel moves the cursor one grid row per notch and clamps at both ends.
    #[test]
    fn emoji_picker_wheel_moves_a_row_and_clamps() {
        let mut p = EmojiPicker::new(Rect::new(0, 0, CELL_SIZE * 3, 240));
        let table: Vec<EmojiGlyph> =
            (0..10).map(|n| EmojiGlyph::new(format!("g{n}"), "*", "faces")).collect();
        p.set_glyphs(table);
        p.set_categories(vec!["faces".to_string()]);
        assert_eq!(p.columns(), 3, "the fixture is exactly three cells wide");
        p.cursor = Some(0);

        // One notch down moves one whole rows: three cells at this width.
        p.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(p.cursor(), Some(3), "a downward notch moves one grid row");
        p.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(p.cursor(), Some(6));

        // Spinning on clamps at the last cell rather than wrapping to the top.
        for _ in 0..5 {
            p.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        }
        assert_eq!(p.cursor(), Some(9), "the cursor clamps at the last cell");

        // And the opposite gesture clamps at the first cell.
        for _ in 0..10 {
            p.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        }
        assert_eq!(p.cursor(), Some(0), "the cursor clamps at the first cell");
    }

    /// PageDown/PageUp move by the whole rows the picker can show and clamp at the ends.
    #[test]
    fn emoji_picker_page_keys_move_by_a_visible_page() {
        let mut p = EmojiPicker::new(Rect::new(0, 0, CELL_SIZE * 3, 240));
        let table: Vec<EmojiGlyph> =
            (0..60).map(|n| EmojiGlyph::new(format!("g{n}"), "*", "faces")).collect();
        p.set_glyphs(table);
        p.set_categories(vec!["faces".to_string()]);
        assert_eq!(p.columns(), 3);
        assert_eq!(p.visible_rows(), 5, "240 px of picker leaves five whole grid rows");
        p.cursor = Some(0);

        p.handle_event(&Event::KeyDown((34, 0)));
        assert_eq!(p.cursor(), Some(15), "PageDown moves one visible page");
        p.handle_event(&Event::KeyDown((34, 0)));
        assert_eq!(p.cursor(), Some(30));
        p.handle_event(&Event::KeyDown((33, 0)));
        assert_eq!(p.cursor(), Some(15), "PageUp moves the same distance back");

        // A page step near the end clamps at the last cell rather than wrapping or
        // running past the grid.
        p.cursor = Some(55);
        p.handle_event(&Event::KeyDown((34, 0)));
        assert_eq!(p.cursor(), Some(59));
        p.handle_event(&Event::KeyDown((33, 0)));
        assert_eq!(p.cursor(), Some(44));
    }

    /// Hovering names the glyph by its keywords, falling back to its id.
    #[test]
    fn emoji_picker_hover_emits_keywords_or_id() {
        let mut p = picker();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = std::sync::Arc::clone(&seen);
        p.glyph_hover_changed.connect(move |caption| {
            sink.lock().unwrap().push((*caption).clone());
        });

        let first = p.cell_rect(0).expect("cell 0");
        p.handle_event(&Event::mouse_move(first.x + 5, first.y + 5));
        let second = p.cell_rect(1).expect("cell 1");
        p.handle_event(&Event::mouse_move(second.x + 5, second.y + 5));
        p.handle_event(&Event::mouse_leave(0, 0));

        assert_eq!(
            *seen.lock().unwrap(),
            vec!["happy, smile".to_string(), "unhappy".to_string(), String::new()],
            "keywords first, then a single keyword, then the empty leave value"
        );

        // A glyph with no keywords is named by its id.
        let mut plain = EmojiPicker::new(Rect::new(0, 0, 280, 240));
        plain.set_glyphs(vec![EmojiGlyph::new("plain", "o", "faces")]);
        plain.set_categories(vec!["faces".to_string()]);
        let plain_seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let plain_sink = std::sync::Arc::clone(&plain_seen);
        plain.glyph_hover_changed.connect(move |caption| {
            plain_sink.lock().unwrap().push((*caption).clone());
        });
        let cell = plain.cell_rect(0).expect("cell 0");
        plain.handle_event(&Event::mouse_move(cell.x + 5, cell.y + 5));
        assert_eq!(*plain_seen.lock().unwrap(), vec!["plain".to_string()]);
    }

    /// The hovered glyph's caption is painted over the picker's foot and removed on leave.
    #[test]
    fn emoji_picker_hover_paints_a_caption_strip() {
        let mut p = picker();
        let resting = render(&mut p, Size::new(280, 240));

        let cell = p.cell_rect(0).expect("cell 0");
        p.handle_event(&Event::mouse_move(cell.x + 5, cell.y + 5));
        let hovered = render(&mut p, Size::new(280, 240));
        assert_ne!(resting, hovered, "a hovered glyph adds the caption strip");

        p.handle_event(&Event::mouse_leave(0, 0));
        let left = render(&mut p, Size::new(280, 240));
        assert_eq!(resting, left, "leaving restores the resting drawing exactly");
    }

    #[test]
    fn emoji_picker_enter_without_a_cursor_does_nothing() {
        let mut p = picker();
        p.handle_event(&Event::KeyDown((13, 0)));
        assert!(p.recent().is_empty());
    }

    #[test]
    fn emoji_picker_cursor_resets_when_the_search_changes() {
        let mut p = picker();
        p.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(p.cursor(), Some(0));
        // The grid's contents changed, so a stale cursor would point at a different
        // glyph.
        p.set_search("check");
        assert_eq!(p.cursor(), None);
    }

    #[test]
    fn emoji_picker_disabled_ignores_clicks() {
        let mut p = picker();
        p.set_enabled(false);
        let cell = p.cell_rect(0).expect("cell 0");
        p.handle_event(&Event::mouse_press(cell.x + 10, cell.y + 10, 1));
        assert!(p.recent().is_empty());
    }

    // ── Drawing ─────────────────────────────────────────────────────────────

    #[test]
    fn emoji_picker_draw_empty_paints_without_panicking() {
        let mut p = EmojiPicker::new(Rect::new(0, 0, 280, 240));
        let rgba = render(&mut p, Size::new(280, 240));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn emoji_picker_draw_zero_geometry_does_not_panic() {
        let mut p = picker();
        let rgba = render(&mut p, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn emoji_picker_different_tabs_paint_different_frames() {
        let mut p = picker();
        let faces = render(&mut p, Size::new(280, 240));
        p.set_active_tab(1);
        let symbols = render(&mut p, Size::new(280, 240));
        assert_ne!(faces, symbols, "switching tabs must change the grid");
    }

    #[test]
    fn emoji_picker_cursor_is_visible() {
        let mut p = picker();
        let uncursored = render(&mut p, Size::new(280, 240));
        p.cursor = Some(0);
        let cursored = render(&mut p, Size::new(280, 240));
        assert_ne!(uncursored, cursored, "the cursor cell must be highlighted");
    }

    #[test]
    fn emoji_picker_recent_tab_is_visible_after_a_choice() {
        let mut p = picker();
        let before = render(&mut p, Size::new(280, 240));
        p.choose("tick");
        let after = render(&mut p, Size::new(280, 240));
        assert_ne!(before, after, "the recent tab must appear");
    }

    // ── Property contract ───────────────────────────────────────────────────

    #[test]
    fn emoji_picker_properties_round_trip() {
        let mut p = picker();
        p.set("active_tab", CapabilityValue::UInt(1)).unwrap();
        assert_eq!(p.active_tab(), 1);

        p.set("search", CapabilityValue::String("happy".to_string())).unwrap();
        assert_eq!(p.search(), "happy");

        assert!(p.set("active_tab", CapabilityValue::Bool(true)).is_err());
        assert!(p.set("search", CapabilityValue::UInt(1)).is_err());
    }

    #[test]
    fn emoji_picker_derived_properties_are_read_only() {
        let mut p = picker();
        // Read the counts before choosing: `choose` adds the recent tab, which
        // changes what is visible, and this test is about which names are writable.
        assert_eq!(p.get("glyph_count").unwrap(), CapabilityValue::UInt(5));
        assert_eq!(p.get("tab_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(p.get("visible_glyph_count").unwrap(), CapabilityValue::UInt(2));
        assert_eq!(p.get("recent_count").unwrap(), CapabilityValue::UInt(0));

        p.choose("smile");
        assert_eq!(p.get("recent_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(p.get("tab_count").unwrap(), CapabilityValue::UInt(4));

        for name in ["glyph_count", "visible_glyph_count", "tab_count", "recent_count"] {
            assert_eq!(
                p.set(name, CapabilityValue::UInt(9)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
    }

    #[test]
    fn emoji_glyph_builders_accumulate_keywords() {
        let glyph = EmojiGlyph::new("a", "A", "c").keyword("one").keywords(&["two", "three"]);
        assert_eq!(glyph.keywords, vec!["one", "two", "three"]);
    }

    /// The recent capacity is a field with accessors, and trimming keeps the newest entries.
    #[test]
    fn the_recent_capacity_is_configurable() {
        let mut p = picker();
        p.set_glyphs(glyphs());
        assert_eq!(p.recent_capacity(), MAX_RECENT);

        p.set_recent_capacity(2);
        for id in ["smile", "sad", "tick"] {
            p.choose(id);
        }
        assert_eq!(p.recent().len(), 2, "the list is capped at the configured size");
        assert_eq!(p.recent()[0], "tick", "the newest entry stays first");

        // Shrinking an existing list trims it immediately.
        p.set_recent_capacity(1);
        assert_eq!(p.recent(), &["tick".to_string()]);
        assert_eq!(p.get("recent_capacity").unwrap(), CapabilityValue::UInt(1));
    }

    /// The visible count is computed once per filter change, not on every pointer move.
    #[test]
    fn the_visible_count_is_cached_until_the_filter_changes() {
        let mut p = picker();
        p.set_glyphs(glyphs());
        p.set_categories(vec!["faces".to_string(), "symbols".to_string()]);
        let first = p.visible_len();
        assert!(first > 0);
        assert!(!p.visible_dirty.get(), "the first query fills the cache");
        assert_eq!(p.visible_len(), first);
        assert!(!p.visible_dirty.get(), "a second query does not recompute");

        p.set_search("tick");
        assert!(p.visible_dirty.get(), "a filter change invalidates the count");
        assert_eq!(p.visible_len(), 1);
    }

    /// The glyph-id index is built once, so the recent projection is a lookup per id.
    #[test]
    fn the_glyph_index_is_rebuilt_with_the_table() {
        let mut p = picker();
        p.set_glyphs(glyphs());
        assert_eq!(p.glyph_index.get("smile"), Some(&0));
        assert_eq!(p.glyph_index.len(), glyphs().len());

        // A replaced table drops the old ids.
        p.set_glyphs(vec![EmojiGlyph::new("only", "o", "faces")]);
        assert!(!p.glyph_index.contains_key("smile"));
        assert_eq!(p.glyph_index.get("only"), Some(&0));
    }

    /// A cell's text is measured once per glyph and device scale.
    #[test]
    fn cell_metrics_are_cached_per_glyph() {
        let mut p = picker();
        p.set_glyphs(glyphs());
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        p.draw(&mut context);
        let first = p.glyph_metrics.borrow().entries.len();
        assert!(first > 0, "the first paint measures the visible cells");
        p.draw(&mut context);
        assert_eq!(
            p.glyph_metrics.borrow().entries.len(),
            first,
            "a second frame reuses the measurements"
        );
        backend.end_frame();
    }
}
