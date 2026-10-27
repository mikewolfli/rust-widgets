// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CommandPalette productivity widget.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Single command entry displayed in command palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    /// Stable command identifier.
    pub id: String,
    /// Human-friendly command title.
    pub title: String,
    /// Optional command category used for display context.
    pub category: String,
    /// Optional keywords used by query matching.
    pub keywords: Vec<String>,
    /// The keyboard shortcut shown at the row's trailing edge.
    pub shortcut: Option<String>,
}

/// The declared shape of one palette entry, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `entries` property, and the control owns both ends of it: the
/// field names are [`CommandEntry`]'s own fields. `id` and `title` are required because
/// [`CommandEntry::new`] requires them; `category`, `keywords` and `shortcut` are optional
/// because an entry that omits them is the ordinary command.
pub const COMMAND_ENTRY_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "title",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "category",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "keywords",
                crate::widget::capability::PropertyValueKind::StringList,
            ),
            crate::widget::capability::RecordField::optional(
                "shortcut",
                crate::widget::capability::PropertyValueKind::String,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`COMMAND_ENTRY_SCHEMA`].
fn command_entry_record(entry: &CommandEntry) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(entry.id.clone()))
        .with("title", CapabilityValue::String(entry.title.clone()))
        .with("category", CapabilityValue::String(entry.category.clone()))
        .with(
            "keywords",
            CapabilityValue::Tuple(
                entry
                    .keywords
                    .iter()
                    .map(|keyword| CapabilityValue::String(keyword.clone()))
                    .collect(),
            ),
        );
    if let Some(shortcut) = &entry.shortcut {
        record = record.with("shortcut", CapabilityValue::String(shortcut.clone()));
    }
    record
}

/// Rebuilds one [`CommandEntry`] from a row of [`COMMAND_ENTRY_SCHEMA`].
fn command_entry_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<CommandEntry, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let category = match record.get("category") {
        None | Some(CapabilityValue::Null) => String::new(),
        Some(CapabilityValue::String(category)) => category.clone(),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let keywords = match record.get("keywords") {
        None | Some(CapabilityValue::Null) => Vec::new(),
        Some(CapabilityValue::Tuple(items)) => items
            .iter()
            .map(|item| match item {
                CapabilityValue::String(keyword) => Ok(keyword.clone()),
                _ => Err(CapabilityAccessError::TypeMismatch),
            })
            .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let shortcut = match record.get("shortcut") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(shortcut)) => Some(shortcut.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(CommandEntry { id: text("id")?, title: text("title")?, category, keywords, shortcut })
}

impl CommandEntry {
    /// Creates a command entry with id and title.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            category: String::new(),
            keywords: Vec::new(),
            shortcut: None,
        }
    }

    /// Sets command category.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Sets command keywords.
    pub fn with_keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the shortcut label shown at the row's trailing edge.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// Palette-style command launcher with query filtering and keyboard navigation.
pub struct CommandPalette {
    base: BaseWidget,
    entries: Vec<CommandEntry>,
    query: String,
    filtered_indices: Vec<usize>,
    highlighted_index: Option<usize>,
    /// The first filtered row drawn; keeps the highlight reachable past the visible rows.
    scroll_offset: usize,
    /// Command ids in most-recently-used order, boosting their match score.
    mru: Vec<String>,
    row_height: u32,
    /// Whether the result list is grouped under category headings.
    group_by_category: bool,
    /// The display slot table, rebuilt when the filtered set, the grouping or the scroll moves.
    ///
    /// Both the paint and the hit test read this; the hit test used to build the whole table just
    /// to take one element out of it (SP-P71).
    slots: Vec<PaletteSlot>,
    /// One cached presentation per filtered row: the text as painted and the matched run.
    ///
    /// The paint used to `format!` the title and lower-case it twice per row per frame to find
    /// the highlight (SP-P72); both are computed when the result set is refreshed.
    rows: Vec<RowPresentation>,
    /// The painted text of every entry, rebuilt only when the entry list changes (SP-116).
    display_texts: Vec<String>,
    /// The ASCII characters each entry contains, as a 128-bit mask (SP-116).
    ///
    /// Every scoring tier requires the query's first character to appear in one of the entry's
    /// fields, so an entry whose mask lacks it cannot match. The mask turns that fact into one
    /// bit test instead of a scan of the title, id, category and keywords.
    entry_masks: Vec<u128>,
    /// Reused contiguous-score buffer, so a keystroke does not reallocate the result set.
    contiguous_scores: Vec<(usize, i32)>,
    /// Reused fuzzy-score buffer (SP-116).
    fuzzy_scores: Vec<(usize, i32)>,
    /// The caption drawn when the query matches nothing (SP-64).
    empty_text: String,
    /// Whether the footer's key hints are drawn (SP-64).
    show_hints: bool,
    /// The footer text; empty falls back to the built-in hint line.
    hint_text: String,
    /// Emitted when command is activated. Payload is command id.
    pub command_activated: Signal1<String>,
    /// Emitted when query changes. Payload is current query.
    pub query_changed: Signal1<String>,
}

impl CommandPalette {
    /// Creates an empty command palette.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "CommandPalette"),
            entries: Vec::new(),
            query: String::new(),
            filtered_indices: Vec::new(),
            highlighted_index: None,
            scroll_offset: 0,
            mru: Vec::new(),
            row_height: 24,
            group_by_category: false,
            slots: Vec::new(),
            rows: Vec::new(),
            display_texts: Vec::new(),
            entry_masks: Vec::new(),
            contiguous_scores: Vec::new(),
            fuzzy_scores: Vec::new(),
            empty_text: String::from("No matching commands"),
            show_hints: true,
            hint_text: String::new(),
            command_activated: Signal1::new(),
            query_changed: Signal1::new(),
        }
    }

    /// Replaces command entries.
    pub fn set_entries(&mut self, entries: Vec<CommandEntry>) {
        self.entries = entries;
        self.rebuild_entry_cache();
        self.refresh_filter();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Rebuilds the per-entry text and first-character masks.
    ///
    /// Both are pure functions of the entry list, so they move only when `set_entries` replaces it
    /// rather than on every keystroke (SP-116).
    fn rebuild_entry_cache(&mut self) {
        let mut texts = Vec::with_capacity(self.entries.len());
        let mut masks = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let text = if entry.category.is_empty() {
                entry.title.clone()
            } else {
                format!("{} · {}", entry.category, entry.title)
            };
            let mut mask = 0u128;
            for field in [&entry.title, &entry.id, &entry.category] {
                for ch in field.chars() {
                    if ch.is_ascii() {
                        mask |= 1u128 << (ch.to_ascii_lowercase() as u32);
                    }
                }
            }
            for keyword in &entry.keywords {
                for ch in keyword.chars() {
                    if ch.is_ascii() {
                        mask |= 1u128 << (ch.to_ascii_lowercase() as u32);
                    }
                }
            }
            texts.push(text);
            masks.push(mask);
        }
        self.display_texts = texts;
        self.entry_masks = masks;
    }

    /// Returns all command entries.
    pub fn entries(&self) -> &[CommandEntry] {
        &self.entries
    }

    /// Sets query string and refreshes filtered command list.
    pub fn set_query(&mut self, query: impl Into<String>) {
        let next = query.into();
        if self.query == next {
            return;
        }
        self.query = next;
        self.refresh_filter();
        self.query_changed.emit(self.query.clone());
        self.base.request_redraw();
    }

    /// Clears current query.
    pub fn clear_query(&mut self) {
        self.set_query(String::new());
    }

    /// Returns current query string.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns filtered command count.
    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Returns highlighted command position in filtered list.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index.filter(|idx| *idx < self.filtered_indices.len())
    }

    /// Sets row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(1);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Whether the result list is grouped under category headings.
    pub fn group_by_category(&self) -> bool {
        self.group_by_category
    }

    /// Groups the filtered result list under a heading per category.
    ///
    /// The heading is display chrome only: it does not enter the filtered index space, so
    /// `highlighted_index`, the MRU ranking and activation keep addressing entries.
    pub fn set_group_by_category(&mut self, grouped: bool) {
        if self.group_by_category == grouped {
            return;
        }
        self.group_by_category = grouped;
        self.refresh_filter();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns command entry at filtered index.
    pub fn filtered_entry(&self, filtered_index: usize) -> Option<&CommandEntry> {
        let entry_index = *self.filtered_indices.get(filtered_index)?;
        self.entries.get(entry_index)
    }

    /// Moves highlighted command by signed delta.
    pub fn move_highlight(&mut self, delta: isize) {
        if self.filtered_indices.is_empty() {
            self.highlighted_index = None;
            return;
        }

        let current = self.highlighted_index.unwrap_or(0) as isize;
        let max = self.filtered_indices.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.highlighted_index = Some(next);
        // Keep the highlighted row inside the visible window.
        let visible = self.visible_rows().max(1);
        self.scroll_offset = self.scroll_offset.min(next);
        if next >= self.scroll_offset + visible {
            self.scroll_offset = next + 1 - visible;
        }
        self.rebuild_slots();
        self.base.request_redraw();
    }

    /// How many result rows fit under the query header.
    fn visible_rows(&self) -> usize {
        let rect = self.geometry();
        (rect.height.saturating_sub(self.header_height()).saturating_sub(self.footer_height())
            / self.row_height.max(1)) as usize
    }

    /// The header band's height.
    fn header_height(&self) -> u32 {
        self.row_height
    }

    /// The footer's hint band, or zero when hints are off.
    fn footer_height(&self) -> u32 {
        if self.show_hints {
            self.row_height.clamp(1, 18)
        } else {
            0
        }
    }

    /// The caption painted when the query matches nothing.
    pub fn empty_text(&self) -> &str {
        &self.empty_text
    }

    /// Sets the empty-result caption; an empty string restores the default.
    pub fn set_empty_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        let text = if text.is_empty() { String::from("No matching commands") } else { text };
        if self.empty_text == text {
            return;
        }
        self.empty_text = text;
        self.base.request_redraw();
    }

    /// Whether the footer's key hints are drawn.
    pub fn show_hints(&self) -> bool {
        self.show_hints
    }

    /// Shows or hides the footer's key hints, recounting the visible rows.
    pub fn set_show_hints(&mut self, show: bool) {
        if self.show_hints == show {
            return;
        }
        self.show_hints = show;
        self.move_highlight(0);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The footer's hint text; an empty string means the built-in line.
    pub fn hint_text(&self) -> &str {
        &self.hint_text
    }

    /// Overrides the footer's hint text.
    pub fn set_hint_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.hint_text == text {
            return;
        }
        self.hint_text = text;
        self.base.request_redraw();
    }

    /// The number of filtered rows scrolled out of view.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Records that `id` was activated, so it sorts first next time.
    pub fn record_use(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.mru.retain(|existing| existing != &id);
        self.mru.insert(0, id);
        self.refresh_filter();
        self.base.request_redraw();
    }

    /// Replaces the most-recently-used list.
    pub fn set_mru(&mut self, ids: Vec<String>) {
        if self.mru == ids {
            return;
        }
        self.mru = ids;
        self.refresh_filter();
        self.base.request_redraw();
    }

    /// Returns the most-recently-used ids.
    pub fn mru(&self) -> &[String] {
        &self.mru
    }

    /// Appends host-committed text (from an IME or a keyboard layout) to the query.
    pub fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.query.push_str(text);
        self.refresh_filter();
        self.query_changed.emit(self.query.clone());
        self.base.request_redraw();
    }

    /// Activates highlighted command, records it as used and emits its id.
    ///
    /// The record is what makes the most-recently-used ordering update through the
    /// **default** keyboard and pointer paths. Previously only a host calling `record_use`
    /// explicitly could feed the list, so the "recently used" feature never updated for a
    /// user who drove the palette with the keyboard.
    pub fn activate_highlighted(&mut self) -> bool {
        let Some(filtered_index) = self.highlighted_index() else {
            return false;
        };
        let Some(entry) = self.filtered_entry(filtered_index) else {
            return false;
        };
        let id = entry.id.clone();
        self.record_use(id.clone());
        self.command_activated.emit(id);
        true
    }

    fn refresh_filter(&mut self) {
        let query = self.query.to_lowercase();
        // One pass over the entries (SP-116). An entry that matches a contiguous tier goes into
        // the contiguous buffer; one that does not is scored against the scattered tier at once.
        // The scattered buffer is only used when **no** contiguous match exists, which is exactly
        // the rule the two-pass form expressed by scanning everything a second time.
        self.contiguous_scores.clear();
        self.fuzzy_scores.clear();
        let first = query.chars().next();
        for (index, entry) in self.entries.iter().enumerate() {
            // The mask prunes an entry that cannot contain the query's first character without
            // touching any of its strings. A non-ASCII query keeps the full scan, because the
            // mask covers ASCII only.
            if let Some(first) = first {
                if first.is_ascii() {
                    let mask = self.entry_masks.get(index).copied().unwrap_or(0);
                    if mask & (1u128 << (first as u32)) == 0 {
                        continue;
                    }
                }
            }
            match Self::score_entry_with(entry, &query, false) {
                Some(score) => self.contiguous_scores.push((index, score)),
                None => {
                    if let Some(score) = Self::score_entry_with(entry, &query, true) {
                        self.fuzzy_scores.push((index, score));
                    }
                }
            }
        }
        let use_fuzzy = self.contiguous_scores.is_empty() && !query.is_empty();
        // The result set reuses a member buffer rather than allocating a fresh vector per key.
        let mut scored = if use_fuzzy {
            core::mem::take(&mut self.fuzzy_scores)
        } else {
            core::mem::take(&mut self.contiguous_scores)
        };

        // One sort, three keys: score descending, then most-recently-used, then input order.
        //
        // The previous form sorted by score and then **re-sorted by MRU position**, so any
        // recently-used command jumped ahead of every command that was not in the MRU list — a
        // worse textual match beat a better one, the exact opposite of what its own comment
        // claimed (SP-63).
        let rank = |index: usize| {
            self.entries
                .get(index)
                .and_then(|entry| self.mru.iter().position(|id| id == &entry.id))
                .unwrap_or(usize::MAX)
        };
        scored.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| rank(a.0).cmp(&rank(b.0))).then_with(|| a.0.cmp(&b.0))
        });
        self.filtered_indices = scored.iter().map(|(index, _)| *index).collect();
        if self.group_by_category {
            // Stable: within one category the score/MRU order is preserved, so grouping
            // changes presentation only and never promotes a worse textual match.
            self.filtered_indices.sort_by(|a, b| {
                let category = |index: usize| {
                    self.entries.get(index).map(|entry| entry.category.as_str()).unwrap_or("")
                };
                category(*a).cmp(category(*b))
            });
        }
        self.scroll_offset = 0;

        if self.filtered_indices.is_empty() {
            self.highlighted_index = None;
        } else {
            self.highlighted_index = Some(0);
        }
        self.rebuild_rows();
        self.rebuild_slots();

        // Hand the buffer back for the next key, keeping its allocation.
        scored.clear();
        if use_fuzzy {
            self.fuzzy_scores = scored;
        } else {
            self.contiguous_scores = scored;
        }
    }

    /// Recomputes the painted text and match highlight for every filtered row.
    fn rebuild_rows(&mut self) {
        let folded_query = self.query.to_lowercase();
        self.rows.clear();
        self.rows.reserve(self.filtered_indices.len());
        for filtered_index in 0..self.filtered_indices.len() {
            let Some(entry_index) = self.filtered_indices.get(filtered_index).copied() else {
                self.rows.push(RowPresentation::default());
                continue;
            };
            let Some(text) = self.display_texts.get(entry_index) else {
                self.rows.push(RowPresentation::default());
                continue;
            };
            let highlight = if folded_query.is_empty() {
                None
            } else {
                // Folded once, here, instead of twice per row per frame. The painted text itself
                // comes from the per-entry cache, so a key no longer clones a formatted string.
                let folded = text.to_lowercase();
                folded
                    .find(&folded_query)
                    .map(|offset| (text[..offset].chars().count(), self.query.chars().count()))
            };
            self.rows.push(RowPresentation { entry_index: Some(entry_index), highlight });
        }
    }

    /// Rebuilds the display slot table for the current scroll offset.
    fn rebuild_slots(&mut self) {
        self.slots.clear();
        let mut last_category: Option<&str> = None;
        let mut pending: Vec<PaletteSlot> = Vec::with_capacity(self.filtered_indices.len());
        for filtered_index in self.scroll_offset..self.filtered_indices.len() {
            let category = self
                .filtered_entry(filtered_index)
                .map(|entry| entry.category.as_str())
                .unwrap_or("");
            if self.group_by_category && !category.is_empty() && last_category != Some(category) {
                pending.push(PaletteSlot { header: true, filtered_index });
                last_category = Some(category);
            }
            pending.push(PaletteSlot { header: false, filtered_index });
        }
        self.slots = pending;
    }

    /// Scores every entry, optionally allowing the scattered-subsequence tier.
    fn score_entry_with(entry: &CommandEntry, query: &str, allow_fuzzy: bool) -> Option<i32> {
        if query.is_empty() {
            return Some(1);
        }

        // Compared case-insensitively against the already-folded query without allocating:
        // this used to build a lowered `String` for the title, the id, the category and one
        // per keyword, for every entry, on every keystroke.
        use crate::widget::text_utils::{
            contains_ignore_ascii_case, starts_with_ignore_ascii_case,
        };

        if starts_with_ignore_ascii_case(&entry.title, query) {
            return Some(120);
        }
        if contains_ignore_ascii_case(&entry.title, query) {
            return Some(100);
        }
        if contains_ignore_ascii_case(&entry.id, query) {
            return Some(80);
        }
        if contains_ignore_ascii_case(&entry.category, query) {
            return Some(60);
        }
        if entry.keywords.iter().any(|keyword| contains_ignore_ascii_case(keyword, query)) {
            return Some(50);
        }

        if !allow_fuzzy {
            return None;
        }
        // Last tier: a subsequence match, so `gfh` reaches `git fetch`. A contiguous hit always
        // outranks it (the tiers above), and the tier only runs when nothing matched contiguously,
        // so every existing ranking is unchanged (SP-65).
        let title = subsequence_score(&entry.title, query);
        let keywords =
            entry.keywords.iter().filter_map(|keyword| subsequence_score(keyword, query)).max();
        match (title, keywords) {
            (None, None) => None,
            (title, keywords) => Some(title.unwrap_or(0).max(keywords.unwrap_or(0)).min(40)),
        }
    }

    fn append_query_char(&mut self, key: u32) {
        if let Some(ch) = char::from_u32(key) {
            if ch.is_ascii_graphic() || ch == ' ' {
                self.query.push(ch);
                self.refresh_filter();
                self.query_changed.emit(self.query.clone());
                self.base.request_redraw();
            }
        }
    }

    fn backspace_query(&mut self) {
        if self.query.pop().is_some() {
            self.refresh_filter();
            self.query_changed.emit(self.query.clone());
            self.base.request_redraw();
        }
    }

    /// The display slot at `pos`, as the index it addresses in the filtered list.
    ///
    /// The hit test and the paint share one derivation: the query header occupies the first
    /// band, and the scroll offset is applied before the division. The old form divided the
    /// raw pointer offset by the row height, which made the first row unclickable and every
    /// row below it off by the header's height — before scrolling — and off by the whole
    /// scroll offset once the list was scrolled.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.y < rect.y || pos.y >= rect.y + rect.height as i32 {
            return None;
        }
        let body_top = rect.y + self.header_height() as i32;
        if pos.y < body_top {
            return None;
        }
        let row = ((pos.y - body_top) / self.row_height.max(1) as i32) as usize;
        // Indexing the cached slot table rather than materialising it; the paint and the hit test
        // read the same rows (SP-P71).
        let slot = *self.display_slots().get(row)?;
        if slot.header {
            return None;
        }
        Some(slot.filtered_index)
    }

    fn pick_at_position(&mut self, pos: Point) {
        if let Some(filtered_index) = self.row_at(pos) {
            self.highlighted_index = Some(filtered_index);
            self.base.request_redraw();
        }
    }

    /// One painted row: a category heading or a filtered command.
    ///
    /// `filtered_index` is always a valid position in `filtered_indices`; for a heading it
    /// names the first entry of that category, which is what the paint reads the label from.
    /// The table is kept in [`Self::slots`] and rebuilt by [`Self::rebuild_slots`].
    fn display_slots(&self) -> &[PaletteSlot] {
        &self.slots
    }
}

#[derive(Clone, Copy)]
struct PaletteSlot {
    /// Whether this slot is a category heading rather than a command row.
    header: bool,
    /// The filtered-list position this slot paints.
    filtered_index: usize,
}

/// One filtered row's cached matched run, built with the result set.
///
/// The row's painted text lives in [`CommandPalette::display_texts`], indexed by `entry_index`, so
/// a result-set refresh copies no formatted string (SP-116).
#[derive(Clone, Copy, Default)]
struct RowPresentation {
    /// The entry this row paints, or `None` for a placeholder row.
    entry_index: Option<usize>,
    /// The matched run as `(character offset, character count)`, for the highlight box.
    highlight: Option<(usize, usize)>,
}

/// Scores a scattered subsequence match, or `None` when `query` is not a subsequence.
///
/// The score rewards a hit on the first character and penalises gaps, so a tight match ranks
/// above a scattered one. It is capped below the contiguous tiers by the caller.
fn subsequence_score(text: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return None;
    }
    let mut matched = 0usize;
    let mut gaps = 0usize;
    let mut last_hit: Option<usize> = None;
    let mut first_hit: Option<usize> = None;
    let mut query_chars = query.chars().map(|ch| ch.to_ascii_lowercase());
    let mut wanted = query_chars.next()?;
    for (index, ch) in text.chars().enumerate() {
        if ch.to_ascii_lowercase() != wanted {
            continue;
        }
        if let Some(previous) = last_hit {
            gaps += index.saturating_sub(previous + 1);
        } else {
            first_hit = Some(index);
        }
        last_hit = Some(index);
        matched += 1;
        match query_chars.next() {
            Some(next) => wanted = next,
            None => break,
        }
    }
    if matched != query.chars().count() {
        return None;
    }
    let lead = first_hit.unwrap_or(0).min(20) as i32;
    Some((40 - lead - gaps.min(20) as i32).max(1))
}

impl Widget for CommandPalette {
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
        crate::core::Size::new(400, 300 + self.footer_height())
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
            "command_activated" => {
                Some(EventSignalRef::mapped("command_activated", &self.command_activated, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "query_changed" => {
                Some(EventSignalRef::mapped("query_changed", &self.query_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `CommandPalette`'s property contract.
///
/// `query` writes go through `set_query` so the filtered result set and the
/// `query_changed` signal stay consistent with the stored string. The counts are
/// derived from that filtered set and are therefore read-only.
impl WidgetProperties for CommandPalette {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The whole entry list as records (BLUE26 B26.09): `set_entries` takes a list the bare
            // command channel cannot carry, so the list is the property and `set_entries` is its
            // published command form.
            "entries" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &COMMAND_ENTRY_SCHEMA,
                    self.entries().iter().map(command_entry_record).collect::<Vec<_>>(),
                )))
            }
            "query" => Ok(CapabilityValue::String(self.query().to_string())),
            "entry_count" => Ok(CapabilityValue::UInt(self.entries().len() as u64)),
            "filtered_count" => Ok(CapabilityValue::UInt(self.filtered_count() as u64)),
            "highlighted_index" => match self.highlighted_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "group_by_category" => Ok(CapabilityValue::Bool(self.group_by_category())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "query" => match value {
                CapabilityValue::String(query) => {
                    self.set_query(query);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.set_row_height(height);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "group_by_category" => match value {
                CapabilityValue::Bool(grouped) => {
                    self.set_group_by_category(grouped);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "entry_count" | "filtered_count" | "highlighted_index" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            // A whole-list replacement through the control's own `set_entries`, which keeps the
            // query and re-filters.
            "entries" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&COMMAND_ENTRY_SCHEMA).map_err(|violation| {
                    log::warn!("command_palette.entries refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("entries"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let entries = list.items().iter().map(command_entry_from_record).collect::<Result<
                    Vec<_>,
                    CapabilityAccessError,
                >>(
                )?;
                self.set_entries(entries);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "entries",
            "query",
            "entry_count",
            "filtered_count",
            "highlighted_index",
            "row_height",
            "group_by_category",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `command_palette` publishes.
    ///
    /// `clear_query` runs bare: it empties the search box and has no argument to be
    /// missing. `activate_highlighted` needs a highlighted row to act on, and a
    /// palette that has been cleared or filtered down to nothing has none, so that
    /// case reports [`CapabilityAccessError::OutOfRange`] — the same "nothing to act
    /// on" answer used elsewhere in the crate rather than a success for a command that
    /// ran nothing. `set_entries` carries its list through the `entries` records
    /// property (B26.03); `move_highlight` takes a signed delta whose direction cannot
    /// be inferred and has no property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_query" => {
                self.clear_query();
                Ok(())
            }
            "activate_highlighted" => {
                if self.activate_highlighted() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "move_highlight" => Err(CapabilityAccessError::OutOfRange),
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for CommandPalette {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                self.pick_at_position(*pos);
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                self.pick_at_position(*pos);
                let _ = self.activate_highlighted();
            }
            Event::KeyPress { key, modifiers } => {
                let primary = crate::shortcut::event_carries_primary(*modifiers);
                let key_value = crate::shortcut::Key::from_key_code(*key);
                match key_value {
                    Some(crate::shortcut::Key::Up) => self.move_highlight(-1),
                    Some(crate::shortcut::Key::Down) => self.move_highlight(1),
                    Some(crate::shortcut::Key::Enter) => {
                        let _ = self.activate_highlighted();
                    }
                    Some(crate::shortcut::Key::Escape) => self.clear_query(),
                    Some(crate::shortcut::Key::Backspace) => self.backspace_query(),
                    Some(crate::shortcut::Key::N) if primary => {
                        self.move_highlight(1);
                    }
                    Some(crate::shortcut::Key::P) if primary => {
                        self.move_highlight(-1);
                    }
                    _ => self.append_query_char(*key),
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => self.push_text(text),
            Event::ImePreedit { .. } => {}
            Event::Wheel { delta, .. } if delta.y != 0 => {
                let max = self.filtered_indices.len().saturating_sub(1);
                let next = if delta.y > 0 {
                    (self.scroll_offset + 1).min(max)
                } else {
                    self.scroll_offset.saturating_sub(1)
                };
                if self.scroll_offset != next {
                    self.scroll_offset = next;
                    self.rebuild_slots();
                    self.base.request_redraw();
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for CommandPalette {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("command_palette");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(background));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The query header and the highlighted row are chrome states, derived from
        // the resolved colours so they follow the appearance.
        let header_background = background.blend(&text_color, 0.06);
        let highlight = background.blend(&text_color, 0.18);

        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        let header = Rect::new(rect.x, rect.y, rect.width, self.row_height);
        context.fill_rect(header, header_background);
        // Centred inside the header row by half the line box: the origin is the glyph's **top**
        // edge, so the row's midline put a 14 px query at 12..26 in a 24 px row.
        // The **effective font** — the resolved theme/caller font — for the query and the rows
        // (D09-STYLE-01).
        let query_font = effective_font(style);
        let query_h = context.measure_text("M", query_font).height;
        context.draw_text_fitted_cow(
            Rect::new(
                header.x + 8,
                header.y + (header.height as i32 - query_h as i32) / 2,
                header.width.saturating_sub(16),
                query_h,
            ),
            &format!("> {}", self.query),
            query_font,
            text_color,
            HorizontalAlignment::Left,
        );

        let mut y = rect.y + self.row_height as i32;
        // The footer's hint line reserves its own band, so the rows cannot paint over it.
        let footer_height = self.footer_height();
        let limit = rect.y + rect.height as i32 - footer_height as i32;

        if self.filtered_indices.is_empty() {
            // An empty result used to leave the list blank; the caption distinguishes "nothing
            // matched" from "nothing loaded" (SP-64).
            context.draw_text_fitted_cow(
                Rect::new(rect.x + 8, y, rect.width.saturating_sub(16), self.row_height),
                &self.empty_text,
                effective_font(style),
                text_color.blend(&background, 0.35),
                HorizontalAlignment::Left,
            );
        }

        let slots = self.display_slots();
        for slot in slots.iter().copied() {
            if y + self.row_height as i32 > limit {
                break;
            }
            let filtered_index = slot.filtered_index;
            if slot.header {
                if let Some(entry) = self.filtered_entry(filtered_index) {
                    let band =
                        Rect::new(rect.x + 8, y, rect.width.saturating_sub(16), self.row_height);
                    context.draw_text_fitted_cow(
                        context.text_line(band, effective_font(style)),
                        &entry.category,
                        effective_font(style),
                        text_color.blend(&background, 0.35),
                        HorizontalAlignment::Left,
                    );
                }
                y += self.row_height as i32;
                continue;
            }

            if self.highlighted_index == Some(filtered_index) {
                context.fill_rect(
                    Rect::new(rect.x + 1, y, rect.width.saturating_sub(2), self.row_height),
                    highlight,
                );
            }

            if let Some(entry) = self.filtered_entry(filtered_index) {
                let trailing = if entry.shortcut.is_some() { 64 } else { 0 };
                let band = Rect::new(
                    rect.x + 8,
                    y,
                    rect.width.saturating_sub(16 + trailing),
                    self.row_height,
                );
                let font = effective_font(style);
                let line_box = context.text_line(band, font);
                // Fitted to the row and centred by its line box. The old origin was the row's
                // midpoint used as a glyph top edge, so every result sat half a line low.
                //
                // The text and its matched run come from the cached presentation, so neither
                // `format!` nor `to_lowercase` runs on the paint path (SP-P72), and the text
                // itself is borrowed from the per-entry cache rather than cloned (SP-116).
                let presentation = self.rows.get(filtered_index).copied().unwrap_or_default();
                let Some(entry_index) = presentation.entry_index else {
                    y += self.row_height as i32;
                    continue;
                };
                let text = self.display_texts.get(entry_index).map(String::as_str).unwrap_or("");
                context.draw_text_fitted_cow(
                    line_box,
                    text,
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
                // Match highlight: a tinted box behind the matched run, measured with the same
                // font the row is drawn in so it lands on the glyphs rather than near them.
                if let Some((offset, count)) = presentation.highlight {
                    let start_x = band.x
                        + context
                            .measure_text(&text.chars().take(offset).collect::<String>(), font)
                            .width as i32;
                    let width = context
                        .measure_text(
                            &text.chars().skip(offset).take(count).collect::<String>(),
                            font,
                        )
                        .width;
                    context
                        .fill_rect(Rect::new(start_x, y, width.max(1), self.row_height), highlight);
                }
                if let Some(shortcut) = entry.shortcut.as_deref() {
                    let key_box = Rect::new(
                        rect.x + rect.width as i32 - 60,
                        y + 2,
                        52,
                        self.row_height.saturating_sub(4),
                    );
                    context.draw_text_line_cow(
                        key_box,
                        shortcut,
                        font,
                        text_color,
                        HorizontalAlignment::Right,
                    );
                }
            }

            y += self.row_height as i32;
        }

        // The footer's key hints: the keys a palette is driven by are not discoverable from the
        // list alone, and a footnote is the conventional place to state them (SP-64).
        if footer_height > 0 {
            let footer = Rect::new(
                rect.x,
                rect.y + rect.height as i32 - footer_height as i32,
                rect.width,
                footer_height,
            );
            context.fill_rect(footer, header_background);
            context.draw_line(
                Point::new(footer.x, footer.y),
                Point::new(footer.x + footer.width as i32, footer.y),
                border,
            );
            let hints = if self.hint_text.is_empty() {
                "↑↓ select · ↵ run · esc clear"
            } else {
                self.hint_text.as_str()
            };
            let font = effective_font(style);
            context.draw_text_fitted_cow(
                context.text_line(
                    Rect::new(
                        footer.x + 8,
                        footer.y,
                        footer.width.saturating_sub(16),
                        footer.height,
                    ),
                    font,
                ),
                hints,
                font,
                text_color.blend(&background, 0.3),
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_entries() -> Vec<CommandEntry> {
        vec![
            CommandEntry::new("file.open", "Open File")
                .with_category("File")
                .with_keywords(["open", "load"]),
            CommandEntry::new("file.save", "Save File")
                .with_category("File")
                .with_keywords(["save", "persist"]),
            CommandEntry::new("view.toggle_sidebar", "Toggle Sidebar")
                .with_category("View")
                .with_keywords(["sidebar", "panel"]),
        ]
    }

    #[test]
    fn query_filters_and_ranks_entries() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 160));
        palette.set_entries(sample_entries());

        assert_eq!(palette.filtered_count(), 3);
        palette.set_query("save");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|entry| entry.id.as_str()), Some("file.save"));

        palette.set_query("file");
        assert_eq!(palette.filtered_count(), 2);
        assert_eq!(palette.filtered_entry(0).map(|entry| entry.id.as_str()), Some("file.open"));
    }

    #[test]
    fn keyboard_navigation_and_activation_work() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 160));
        palette.set_entries(sample_entries());

        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        palette.command_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        palette.handle_event(&Event::key_press(40, 0));
        assert_eq!(palette.highlighted_index(), Some(1));
        palette.handle_event(&Event::key_press(13, 0));

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["file.save".to_string()]);
    }

    #[test]
    fn typing_and_backspace_drive_query() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 160));
        palette.set_entries(sample_entries());

        palette.handle_event(&Event::key_press('s' as u32, 0));
        palette.handle_event(&Event::key_press('a' as u32, 0));
        assert_eq!(palette.query(), "sa");
        assert_eq!(palette.filtered_count(), 1);

        palette.handle_event(&Event::key_press(8, 0));
        assert_eq!(palette.query(), "s");
        assert_eq!(palette.filtered_count(), 2);

        palette.handle_event(&Event::key_press(27, 0));
        assert_eq!(palette.query(), "");
        assert_eq!(palette.filtered_count(), 3);
    }

    #[test]
    fn default_state() {
        let palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        assert_eq!(palette.query(), "");
        assert_eq!(palette.filtered_count(), 0);
        assert_eq!(palette.highlighted_index(), None);
        assert!(palette.entries().is_empty());
    }

    #[test]
    fn empty_entries_handling() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(Vec::new());
        assert_eq!(palette.filtered_count(), 0);
        assert_eq!(palette.highlighted_index(), None);
        // Activation should not panic
        assert!(!palette.activate_highlighted());

        // Navigation should not panic
        palette.move_highlight(1);
        assert_eq!(palette.highlighted_index(), None);
    }

    #[test]
    fn set_query_programmatically() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(vec![
            CommandEntry::new("a", "Alpha"),
            CommandEntry::new("b", "Beta"),
            CommandEntry::new("g", "Gamma"),
        ]);

        palette.set_query("beta");
        assert_eq!(palette.query(), "beta");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("b"));

        // Clear query shows all
        palette.clear_query();
        assert_eq!(palette.query(), "");
        assert_eq!(palette.filtered_count(), 3);
    }

    #[test]
    fn case_insensitive_matching() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(vec![
            CommandEntry::new("openFile", "Open File"),
            CommandEntry::new("openFolder", "Open Folder"),
        ]);

        palette.set_query("open");
        assert_eq!(palette.filtered_count(), 2);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("openFile"));

        palette.set_query("OPEN");
        assert_eq!(palette.filtered_count(), 2);

        // No match
        palette.set_query("CLOSE");
        assert_eq!(palette.filtered_count(), 0);
    }

    /// A pointer press lands on the row it is drawn on, header and scroll included.
    ///
    /// # The defect this pins
    ///
    /// `pick_at_position` divided `pos.y - rect.y` by the row height, so the query header's
    /// band was counted as row 0: the first visible row could not be chosen, every row below
    /// it was off by one, and once the list scrolled the whole mapping was off by the scroll
    /// offset. The paint and the hit test now read one slot list.
    #[test]
    fn a_click_lands_on_the_row_it_is_painted_on() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 120));
        palette.set_entries(
            (0..6)
                .map(|index| CommandEntry::new(format!("cmd{index}"), format!("Cmd {index}")))
                .collect(),
        );
        assert_eq!(palette.row_height(), 24);

        // The header occupies the first band; a press there is not a row.
        palette.handle_event(&Event::mouse_press(10, 10, 1));
        assert_eq!(palette.highlighted_index(), Some(0), "a header press leaves the highlight");

        // The first body row starts after the header and must select filtered row 0.
        palette.handle_event(&Event::mouse_press(10, 30, 1));
        assert_eq!(palette.highlighted_index(), Some(0));

        // Scroll two rows out and the same pointer position addresses the row now painted there.
        palette.handle_event(&Event::wheel(0, 1, 0));
        palette.handle_event(&Event::wheel(0, 1, 0));
        assert_eq!(palette.scroll_offset(), 2);
        palette.handle_event(&Event::mouse_press(10, 30, 1));
        assert_eq!(palette.highlighted_index(), Some(2));
    }

    /// The default keyboard path feeds the most-recently-used list.
    #[test]
    fn activating_through_the_keyboard_records_the_command_as_used() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 160));
        palette.set_entries(sample_entries());
        assert!(palette.mru().is_empty());

        palette.handle_event(&Event::key_press(40, 0));
        palette.handle_event(&Event::key_press(13, 0));

        assert_eq!(palette.mru().first().map(String::as_str), Some("file.save"));
    }

    /// Grouping inserts a heading per category without changing the entry index space.
    #[test]
    fn grouping_adds_a_heading_per_category() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 200));
        palette.set_entries(sample_entries());
        assert!(!palette.group_by_category());

        palette.set_group_by_category(true);
        let slots = palette.display_slots();
        // Three entries in two categories: one heading per category plus the entries.
        assert_eq!(slots.iter().filter(|slot| slot.header).count(), 2);
        assert_eq!(slots.iter().filter(|slot| !slot.header).count(), 3);
        assert_eq!(palette.filtered_count(), 3, "the entries are still addressable");
    }

    #[test]
    fn category_and_keyword_based_matching() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(vec![
            CommandEntry::new("file.open", "Open")
                .with_category("File")
                .with_keywords(["load", "browse"]),
            CommandEntry::new("edit.find", "Find")
                .with_category("Edit")
                .with_keywords(["search", "locate"]),
        ]);

        // Match category
        palette.set_query("file");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("file.open"));

        // Match keyword
        palette.set_query("load");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("file.open"));

        // Match keyword on second entry
        palette.set_query("search");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("edit.find"));

        // No match
        palette.set_query("nonexistent");
        assert_eq!(palette.filtered_count(), 0);
    }

    /// The single scoring pass preserves the tier order: contiguous matches occupy the result set
    /// alone, and the scattered tier runs only when nothing contiguous matched (SP-116).
    #[test]
    fn the_single_pass_keeps_contiguous_matches_ahead_of_the_fuzzy_fallback() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 240));
        palette.set_entries(vec![
            CommandEntry::new("sidebar", "Sidebar"),
            CommandEntry::new("save_as", "Save As"),
            CommandEntry::new("git_fetch", "Git Fetch"),
        ]);

        // `s` starts both titles (120 each), so the input order breaks the tie; the scattered
        // tier must not add anything beside them.
        palette.set_query("s");
        assert_eq!(palette.filtered_count(), 2);
        assert_eq!(palette.filtered_entry(0).map(|entry| entry.id.as_str()), Some("sidebar"));
        assert_eq!(palette.filtered_entry(1).map(|entry| entry.id.as_str()), Some("save_as"));

        // With no contiguous hit, the scattered tier takes over: `gfh` reaches `Git Fetch`.
        palette.set_query("gfh");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|entry| entry.id.as_str()), Some("git_fetch"));

        // An uppercase query folds to the same mask and still finds the entry.
        palette.set_query("SA");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|entry| entry.id.as_str()), Some("save_as"));
    }

    /// The entry text and its mask are built once per entry list, not per keystroke (SP-116).
    #[test]
    fn the_painted_text_and_mask_are_cached_with_the_entry_list() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 240));
        palette.set_entries(vec![CommandEntry::new("open", "Open").with_category("File")]);
        assert_eq!(palette.display_texts, vec![String::from("File · Open")]);
        let mask = palette.entry_masks[0];
        assert_ne!(mask & (1u128 << ('o' as u32)), 0);
        assert_ne!(mask & (1u128 << ('f' as u32)), 0);
        assert_eq!(mask & (1u128 << ('z' as u32)), 0);

        // The same buffers survive a query change; only the filtered indices move.
        let mask_before = palette.entry_masks.clone();
        let text_before = palette.display_texts.clone();
        palette.set_query("op");
        assert_eq!(palette.entry_masks, mask_before);
        assert_eq!(palette.display_texts, text_before);
    }
}
