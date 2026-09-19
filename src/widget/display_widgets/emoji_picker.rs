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

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
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

/// How many "recently used" ids the picker remembers.
///
/// Twenty is a full row of the default grid, which is how many a user can see at
/// once without scrolling the recent tab — beyond that the list stops being a
/// shortcut and becomes another search.
const MAX_RECENT: usize = 20;

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
    /// The grid index the keyboard cursor is on, if any.
    cursor: Option<usize>,
    /// Emitted when a glyph is chosen, with its id.
    pub glyph_chosen: Signal1<String>,
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
            cursor: None,
            glyph_chosen: Signal1::new(),
        }
    }

    /// Replaces the glyph table.
    ///
    /// The category tabs are **not** derived from the glyphs: a caller that declares
    /// its tab order should keep it, and deriving would reorder the tabs whenever a
    /// glyph was added. Use [`Self::set_categories`] for the tabs.
    pub fn set_glyphs(&mut self, glyphs: Vec<EmojiGlyph>) {
        self.glyphs = glyphs;
        // A glyph that no longer exists cannot stay in the recent list, or choosing
        // it would emit an id nothing can resolve.
        let known: Vec<&str> = self.glyphs.iter().map(|glyph| glyph.id.as_str()).collect();
        self.recent.retain(|id| known.contains(&id.as_str()));
        self.cursor = None;
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

    /// The index of the selected tab.
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
        self.base.request_redraw();
    }

    /// The current search text.
    pub fn search(&self) -> &str {
        &self.search
    }

    /// Sets the search text, clearing the cursor.
    pub fn set_search(&mut self, search: &str) {
        self.search = search.to_string();
        self.cursor = None;
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
        self.base.request_redraw();
    }

    /// The glyphs currently visible, in grid order.
    ///
    /// # The one place the tabs, the search and the grid agree
    ///
    /// Filtering happens here and nowhere else, so the drawing loop, the hit-test
    /// and the keyboard cursor cannot disagree about what is on screen — which is
    /// the defect class this accessor removes.
    ///
    /// A non-empty search overrides the tab: searching means "find this anywhere",
    /// and restricting it to the current category would hide matches the user can
    /// see exist in another tab's label.
    pub fn visible_glyphs(&self) -> Vec<&EmojiGlyph> {
        if !self.search.is_empty() {
            return self.glyphs.iter().filter(|glyph| glyph.matches(&self.search)).collect();
        }
        let tabs = self.tabs();
        let Some(category) = tabs.get(self.active_tab) else {
            return Vec::new();
        };
        if category == Self::RECENT_CATEGORY {
            // In recency order, not table order: the list exists to be a shortcut.
            return self
                .recent
                .iter()
                .filter_map(|id| self.glyphs.iter().find(|glyph| &glyph.id == id))
                .collect();
        }
        self.glyphs.iter().filter(|glyph| &glyph.category == category).collect()
    }

    /// The glyph at grid index `index`, if the grid is that long.
    pub fn glyph_at_index(&self, index: usize) -> Option<&EmojiGlyph> {
        self.visible_glyphs().into_iter().nth(index)
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
        self.recent.truncate(MAX_RECENT);
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

    /// The rectangle of the grid cell at `index`.
    fn cell_rect(&self, index: usize) -> Option<Rect> {
        // Bounded by the visible glyph count so a stale cursor cannot address a cell
        // that is not drawn.
        if index >= self.visible_glyphs().len() {
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

    /// The grid index under `pos`, when the pointer is over a cell.
    fn cell_at(&self, pos: Point) -> Option<usize> {
        let count = self.visible_glyphs().len();
        (0..count).find(|index| self.cell_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
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
        if index < self.tabs().len() {
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
        let count = self.visible_glyphs().len();
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
}

impl Widget for EmojiPicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(280, 240)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            "visible_glyph_count" => Ok(CapabilityValue::UInt(self.visible_glyphs().len() as u64)),
            "tab_count" => Ok(CapabilityValue::UInt(self.tabs().len() as u64)),
            "active_tab" => Ok(CapabilityValue::UInt(self.active_tab() as u64)),
            "search" => Ok(CapabilityValue::String(self.search().to_string())),
            "recent_count" => Ok(CapabilityValue::UInt(self.recent().len() as u64)),
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
        context.fill_rect(rect, Color::rgb(252, 252, 254));
        context.draw_rect(rect, Color::rgb(210, 212, 218));
        self.draw_search(context);
        self.draw_tabs(context);
        let visible: Vec<EmojiGlyph> = self.visible_glyphs().into_iter().cloned().collect();
        self.draw_grid(context, &visible);
    }
}

impl EmojiPicker {
    /// Draws the search box.
    fn draw_search(&self, context: &mut RenderContext) {
        let rect = self.search_rect();
        context.fill_rect(rect, Color::rgb(244, 245, 248));
        context.draw_line_stroke(
            Point::new(rect.x, rect.y + rect.height as i32),
            Point::new(rect.x + rect.width as i32, rect.y + rect.height as i32),
            Color::rgb(214, 216, 222),
            1,
        );
        // A magnifier glyph, drawn as geometry so it needs no font support.
        let cx = rect.x + 14;
        let cy = rect.y + rect.height as i32 / 2;
        context.draw_circle_stroke(Point::new(cx, cy - 1), 5, Color::rgb(150, 154, 162), 1);
        context.draw_line_stroke(
            Point::new(cx + 4, cy + 3),
            Point::new(cx + 8, cy + 7),
            Color::rgb(150, 154, 162),
            1,
        );

        let (text, color) = if self.search.is_empty() {
            ("Search".to_string(), Color::rgb(160, 164, 172))
        } else {
            (self.search.clone(), Color::rgb(40, 44, 52))
        };
        context.draw_text(
            Point::new(rect.x + 28, rect.y + 20),
            &text,
            &Font::simple("Sans", 12.0),
            color,
            HorizontalAlignment::Left,
        );
    }

    /// Draws the category tab strip.
    fn draw_tabs(&self, context: &mut RenderContext) {
        let rect = self.geometry();
        let strip = Rect::new(rect.x, rect.y + SEARCH_HEIGHT as i32, rect.width, TAB_HEIGHT);
        context.fill_rect(strip, Color::rgb(238, 240, 244));
        let tabs = self.tabs();
        for (index, tab) in tabs.iter().enumerate() {
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
                context.fill_rect(tab_rect, Color::rgb(255, 255, 255));
                context.draw_line_stroke(
                    Point::new(tab_rect.x, tab_rect.y + TAB_HEIGHT as i32 - 2),
                    Point::new(
                        tab_rect.x + tab_rect.width as i32,
                        tab_rect.y + TAB_HEIGHT as i32 - 2,
                    ),
                    Color::rgb(66, 133, 244),
                    2,
                );
            }
            // The recent pseudo-category is shown by a clock-ish label rather than its
            // reserved identifier, which is not meant to be read.
            let label =
                if tab == Self::RECENT_CATEGORY { "Recent".to_string() } else { tab.clone() };
            let color = if active { Color::rgb(40, 44, 52) } else { Color::rgb(120, 124, 132) };
            context.draw_text(
                Point::new(tab_rect.x + 8, tab_rect.y + 17),
                &label,
                &Font::simple("Sans", 11.0),
                color,
                HorizontalAlignment::Left,
            );
        }
    }

    /// Draws the glyph grid.
    fn draw_grid(&self, context: &mut RenderContext, visible: &[EmojiGlyph]) {
        if visible.is_empty() {
            let rect = self.geometry();
            context.draw_text(
                Point::new(rect.x + 10, rect.y + (SEARCH_HEIGHT + TAB_HEIGHT) as i32 + 20),
                "No glyphs",
                &Font::simple("Sans", 11.0),
                Color::rgb(160, 164, 172),
                HorizontalAlignment::Left,
            );
            return;
        }
        for (index, glyph) in visible.iter().enumerate() {
            let Some(cell) = self.cell_rect(index) else {
                continue;
            };
            // A cell outside the control's own rectangle would paint over a
            // neighbour, so the grid is clipped to the picker.
            let rect = self.geometry();
            if cell.y + cell.height as i32 > rect.y + rect.height as i32 {
                continue;
            }
            let selected = self.cursor == Some(index);
            if selected {
                context.fill_rounded_rect(cell, 4, Color::rgb(226, 236, 254));
            }
            let metrics = context.measure_text(&glyph.symbol, &Font::simple("Sans", 16.0));
            context.draw_text(
                Point::new(
                    cell.x + (cell.width as i32 - metrics.width as i32) / 2,
                    cell.y + (cell.height as i32 + metrics.ascent as i32) / 2,
                ),
                &glyph.symbol,
                &Font::simple("Sans", 16.0),
                Color::rgb(40, 44, 52),
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
            Event::MousePress { pos, button } if *button == 1 => {
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
}
