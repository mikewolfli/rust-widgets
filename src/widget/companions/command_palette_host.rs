// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `CommandPaletteHost` — one command palette shared by the editor, the terminal and the
//! application shell (COMP-A30).
//!
//! # Why a companion
//!
//! The editor's palette knew about editor commands, the terminal had its own list, and a host
//! that wanted both had no way to present one query over the union. Each copy grew its own
//! filter loop, its own selection arithmetic and its own activation path, so "run the highlighted
//! command" was implemented three times. This host owns that surface once.
//!
//! # What it owns
//!
//! The host owns the entry list, the query and the highlighted row. It does not own the commands
//! themselves: a source contributes [`PaletteEntryRef`] values, and
//! [`CommandPaletteHost::command_activated`] reports the chosen `id`, which the source maps back
//! to its own action. That split is what makes the palette a host rather than another editor
//! feature.
//!
//! # Filtering and selection
//!
//! The filter is matched case-insensitively against an entry's title, id and hint. The selected
//! index addresses the **filtered** list, because a palette must never highlight a row the user
//! cannot see; every filter change rewinds the selection to the first match and the scroll window
//! follows the selection.

use crate::compat::{String, ToString, Vec};
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::{
    PropertyValueKind, RecordField, RecordList, RecordSchema, RecordValue, WidgetProperties,
};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

pub use super::drop_down_popup_host::Direction;

/// Height of the query field above the rows.
const PALETTE_HEADER_HEIGHT: u32 = 36;
/// Height of one command row.
const PALETTE_ROW_HEIGHT: u32 = 28;
/// Horizontal inset of the query and the row text.
const PALETTE_INSET: u32 = 12;
/// The number of rows a palette asks for when nothing constrains its height.
const PALETTE_DEFAULT_ROWS: usize = 10;

/// Which surface contributed an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaletteSource {
    /// The code or markdown editor.
    Editor,
    /// The terminal view.
    Terminal,
    /// The application shell itself.
    #[default]
    Host,
}

impl PaletteSource {
    /// The token used by the property contract.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Terminal => "terminal",
            Self::Host => "host",
        }
    }

    /// Parses the token used by the property contract.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "editor" => Some(Self::Editor),
            "terminal" => Some(Self::Terminal),
            "host" => Some(Self::Host),
            _ => None,
        }
    }
}

/// One command a source contributes to the palette.
///
/// `id` is the source's own command identity; the palette never interprets it. `title` is what
/// the user reads, `hint` is secondary text (a shortcut, a category) and `source` says which
/// surface owns the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntryRef {
    /// The source's stable command identity.
    pub id: String,
    /// The text painted in the row and matched by the filter.
    pub title: String,
    /// Secondary text, also matched by the filter.
    pub hint: String,
    /// The surface that contributed the command.
    pub source: PaletteSource,
}

impl PaletteEntryRef {
    /// Creates an entry with an empty hint, owned by the host.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            hint: String::new(),
            source: PaletteSource::Host,
        }
    }

    /// Sets the secondary text.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Sets the contributing surface.
    pub fn with_source(mut self, source: PaletteSource) -> Self {
        self.source = source;
        self
    }
}

/// One row of the `entries` records property.
///
/// `hint` and `source` are optional so a document can add a command by its title alone; an
/// omitted `source` is [`PaletteSource::Host`], the same default [`PaletteEntryRef::new`] uses.
pub const PALETTE_ENTRY_SCHEMA: RecordSchema = RecordSchema {
    fields: &[
        RecordField::required("id", PropertyValueKind::String),
        RecordField::required("title", PropertyValueKind::String),
        RecordField::optional("hint", PropertyValueKind::String),
        RecordField::optional("source", PropertyValueKind::String),
    ],
    id_field: "id",
    ids_unique: true,
    ids_required: true,
};

/// Whether `haystack` contains `needle`, ignoring case.
///
/// The ASCII path compares bytes in place; non-ASCII text takes the Unicode case-folding
/// fallback. The same split the filter predicates use keeps a palette query allocation-free for
/// the common case.
fn matches_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.is_ascii() && needle.is_ascii() {
        return haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()));
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Whether an entry matches a query in its title, id or hint.
fn entry_matches(entry: &PaletteEntryRef, needle: &str) -> bool {
    matches_ignore_case(&entry.title, needle)
        || matches_ignore_case(&entry.id, needle)
        || matches_ignore_case(&entry.hint, needle)
}

/// A filterable list of commands a source contributes to one palette.
pub struct CommandPaletteHost {
    base: BaseWidget,
    entries: Vec<PaletteEntryRef>,
    filter: String,
    /// An index into the filtered list, not into `entries`.
    selected_index: usize,
    /// The first painted match, kept so the selection stays visible in a short palette.
    scroll_offset: usize,
    open: bool,
    /// Emitted with the activated command's `id`.
    pub command_activated: Signal1<String>,
    /// Emitted with the new query after it changes.
    pub filter_changed: Signal1<String>,
    /// Emitted when the palette is dismissed by Escape.
    pub cancelled: GenericSignal,
}

impl CommandPaletteHost {
    /// Creates a closed palette with no entries.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "CommandPaletteHost"),
            entries: Vec::new(),
            filter: String::new(),
            selected_index: 0,
            scroll_offset: 0,
            open: false,
            command_activated: Signal1::new(),
            filter_changed: Signal1::new(),
            cancelled: GenericSignal::new(),
        }
    }

    /// Opens the palette with an empty query and the first row selected.
    ///
    /// Opening is a lifecycle event, so the query reset is announced through
    /// [`Self::filter_changed`] only when it actually removed text.
    pub fn open(&mut self) {
        self.open = true;
        self.selected_index = 0;
        self.scroll_offset = 0;
        if self.filter.is_empty() {
            self.ensure_selection_visible();
        } else {
            self.apply_filter(String::new());
        }
        self.base.request_redraw();
    }

    /// Closes the palette without reporting a cancellation.
    pub fn close(&mut self) {
        if self.open {
            self.open = false;
            self.base.request_redraw();
        }
    }

    /// Whether the palette is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Replaces the entry list, rewinding the selection to the first match.
    pub fn set_entries(&mut self, entries: Vec<PaletteEntryRef>) {
        self.entries = entries;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.ensure_selection_visible();
        self.base.request_redraw();
    }

    /// Returns every entry, including those the current filter hides.
    pub fn entries(&self) -> &[PaletteEntryRef] {
        &self.entries
    }

    /// Returns the current query.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Replaces the query and rewinds the selection to the first match.
    ///
    /// A write of the value already stored is a no-op, so a binding that echoes the query back
    /// does not raise a second [`Self::filter_changed`].
    pub fn set_filter(&mut self, filter: impl Into<String>) {
        let filter = filter.into();
        if self.filter == filter {
            return;
        }
        self.apply_filter(filter);
    }

    /// The selected row's index in the filtered list.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Sets the selected row by its index in the filtered list.
    ///
    /// Returns `false` when the index names no visible row.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if index >= self.visible_count() {
            return false;
        }
        if self.selected_index != index {
            self.selected_index = index;
            self.ensure_selection_visible();
            self.base.request_redraw();
        }
        true
    }

    /// The entries that match the current query, in source order.
    pub fn visible_entries(&self) -> Vec<&PaletteEntryRef> {
        let needle = self.filter.trim();
        self.entries.iter().filter(|entry| entry_matches(entry, needle)).collect()
    }

    /// The selected entry, when the filtered list is not empty.
    pub fn selected_entry(&self) -> Option<&PaletteEntryRef> {
        self.selected_entry_index().and_then(|index| self.entries.get(index))
    }

    /// Moves the selection one row in `direction`, wrapping at the ends.
    pub fn move_selection(&mut self, direction: Direction) {
        let count = self.visible_count();
        if count == 0 {
            return;
        }
        let current = self.selected_index.min(count - 1);
        let next = match direction {
            Direction::Next => (current + 1) % count,
            Direction::Previous => (current + count - 1) % count,
        };
        if self.selected_index != next {
            self.selected_index = next;
            self.ensure_selection_visible();
            self.base.request_redraw();
        }
    }

    /// Activates the selected command, reporting whether one was activated.
    ///
    /// A successful activation emits [`Self::command_activated`] with the entry's `id` and
    /// closes the palette.
    pub fn activate_selected(&mut self) -> bool {
        if !self.open {
            return false;
        }
        let Some(index) = self.selected_entry_index() else {
            return false;
        };
        let id = self.entries[index].id.clone();
        self.command_activated.emit(id);
        self.close();
        true
    }

    /// Applies a new query, rewinds the selection and announces the change.
    fn apply_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.ensure_selection_visible();
        self.base.request_redraw();
        self.filter_changed.emit(self.filter.clone());
    }

    /// The number of entries matching the current query.
    fn visible_count(&self) -> usize {
        let needle = self.filter.trim();
        self.entries.iter().filter(|entry| entry_matches(entry, needle)).count()
    }

    /// The index in `entries` of the selected match.
    fn selected_entry_index(&self) -> Option<usize> {
        let needle = self.filter.trim();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry_matches(entry, needle))
            .nth(self.selected_index)
            .map(|(index, _)| index)
    }

    /// The number of rows that fit below the query field.
    fn displayed_rows(&self) -> usize {
        let available = self.geometry().height.saturating_sub(PALETTE_HEADER_HEIGHT);
        ((available / PALETTE_ROW_HEIGHT) as usize).max(1)
    }

    /// The largest valid first-row offset.
    fn max_scroll(&self) -> usize {
        self.visible_count().saturating_sub(self.displayed_rows())
    }

    /// Keeps the selection inside the filtered list and the window around the selection.
    fn ensure_selection_visible(&mut self) {
        let count = self.visible_count();
        if count == 0 {
            self.selected_index = 0;
            self.scroll_offset = 0;
            return;
        }
        self.selected_index = self.selected_index.min(count - 1);
        let visible = self.displayed_rows();
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_index + 1 - visible;
        }
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());
    }

    /// The row index under `pos`, when the pointer is over a painted row.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let top = rect.y + PALETTE_HEADER_HEIGHT as i32;
        if pos.y < top {
            return None;
        }
        let row = ((pos.y - top) as u32 / PALETTE_ROW_HEIGHT) as usize;
        if row >= self.displayed_rows() {
            return None;
        }
        let index = self.scroll_offset + row;
        (index < self.visible_count()).then_some(index)
    }

    /// Dismisses the palette and reports the cancellation.
    fn cancel(&mut self) {
        if self.open {
            self.close();
            self.cancelled.emit();
            self.base.request_redraw();
        }
    }
}

impl EventHandler for CommandPaletteHost {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.open {
            return;
        }
        match event {
            Event::TextInput { text } => {
                let mut next = self.filter.clone();
                next.push_str(text);
                self.set_filter(next);
            }
            Event::MouseMove { pos } => {
                if let Some(row) = self.row_at(*pos) {
                    let _ = self.set_selected_index(row);
                }
            }
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                match self.row_at(*pos) {
                    Some(row) => {
                        let _ = self.set_selected_index(row);
                        let _ = self.activate_selected();
                    }
                    None => {
                        if !self.geometry().contains_point(*pos) {
                            self.cancel();
                        }
                    }
                }
            }
            Event::Wheel { delta, .. } => {
                if delta.y == 0 {
                    return;
                }
                let step = if delta.y < 0 { 1 } else { -1 };
                let next = (self.scroll_offset as i32 + step).max(0) as usize;
                let next = next.min(self.max_scroll());
                if next != self.scroll_offset {
                    self.scroll_offset = next;
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } => match *key {
                key_codes::UP => self.move_selection(Direction::Previous),
                key_codes::DOWN => self.move_selection(Direction::Next),
                key_codes::HOME => {
                    self.selected_index = 0;
                    self.ensure_selection_visible();
                    self.base.request_redraw();
                }
                key_codes::END => {
                    let count = self.visible_count();
                    if count > 0 {
                        self.selected_index = count - 1;
                        self.ensure_selection_visible();
                        self.base.request_redraw();
                    }
                }
                key_codes::ENTER | key_codes::LINE_FEED => {
                    let _ = self.activate_selected();
                }
                key_codes::BACKSPACE => {
                    if self.filter.pop().is_some() {
                        let filter = self.filter.clone();
                        self.apply_filter(filter);
                    }
                }
                key_codes::ESCAPE => self.cancel(),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for CommandPaletteHost {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.open {
            return;
        }
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        let border = surface.blend(&ink, 0.2);
        let highlight = surface.blend(&accent, 0.2);

        context.fill_rect(rect, surface);

        let header_height = PALETTE_HEADER_HEIGHT.min(rect.height);
        let header = Rect::new(rect.x, rect.y, rect.width, header_height);
        let text = Rect::new(
            header.x + PALETTE_INSET as i32,
            header.y,
            header.width.saturating_sub(PALETTE_INSET * 2),
            header.height,
        );
        let (label, color) = if self.filter.is_empty() {
            ("Type a command", surface.blend(&ink, 0.5))
        } else {
            (self.filter.as_str(), ink)
        };
        context.draw_text_fitted_cow(text, label, font, color, HorizontalAlignment::Left);
        if header.height > 0 {
            let divider = Rect::new(rect.x, header.y + header.height as i32 - 1, rect.width, 1);
            context.fill_rect(divider, accent);
        }

        let top = rect.y + PALETTE_HEADER_HEIGHT as i32;
        if top < rect.bottom() {
            let needle = self.filter.trim();
            let visible = self.displayed_rows();
            let mut drawn = 0usize;
            for (match_index, entry) in self
                .entries
                .iter()
                .filter(|entry| entry_matches(entry, needle))
                .enumerate()
                .skip(self.scroll_offset)
                .take(visible)
            {
                let row_index = match_index - self.scroll_offset;
                let row = Rect::new(
                    rect.x,
                    top + (row_index as u32 * PALETTE_ROW_HEIGHT) as i32,
                    rect.width,
                    PALETTE_ROW_HEIGHT,
                );
                if row.y >= rect.bottom() {
                    break;
                }
                if match_index == self.selected_index {
                    context.fill_rect(row, highlight);
                }
                let title_width = (row.width / 2).saturating_sub(PALETTE_INSET);
                let title = Rect::new(row.x + PALETTE_INSET as i32, row.y, title_width, row.height);
                context.draw_text_fitted_cow(
                    title,
                    &entry.title,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
                let hint_width = (row.width / 2).saturating_sub(PALETTE_INSET);
                if hint_width > 0 && !entry.hint.is_empty() {
                    let hint = Rect::new(
                        row.right() - PALETTE_INSET as i32 - hint_width as i32,
                        row.y,
                        hint_width,
                        row.height,
                    );
                    context.draw_text_fitted_cow(
                        hint,
                        &entry.hint,
                        font,
                        surface.blend(&ink, 0.55),
                        HorizontalAlignment::Right,
                    );
                }
                drawn += 1;
            }
            if drawn == 0 {
                let empty = Rect::new(
                    rect.x + PALETTE_INSET as i32,
                    top,
                    rect.width.saturating_sub(PALETTE_INSET * 2),
                    PALETTE_ROW_HEIGHT,
                );
                context.draw_text_fitted_cow(
                    empty,
                    "No matching commands",
                    font,
                    surface.blend(&ink, 0.55),
                    HorizontalAlignment::Left,
                );
            }
        }

        context.draw_rect(rect, border);
    }
}

impl Widget for CommandPaletteHost {
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
        let rows = self.visible_count().clamp(1, PALETTE_DEFAULT_ROWS) as u32;
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(480, PALETTE_HEADER_HEIGHT + rows * PALETTE_ROW_HEIGHT),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "command_activated" => {
                Some(EventSignalRef::mapped("command_activated", &self.command_activated, |id| {
                    CapabilityValue::String(id.clone())
                }))
            }
            "filter_changed" => {
                Some(EventSignalRef::mapped("filter_changed", &self.filter_changed, |filter| {
                    CapabilityValue::String(filter.clone())
                }))
            }
            "cancelled" => Some(EventSignalRef::unit("cancelled", &self.cancelled)),
            _ => None,
        }
    }
}

/// One row of [`PALETTE_ENTRY_SCHEMA`].
fn palette_entry_record(entry: &PaletteEntryRef) -> RecordValue {
    let mut record = RecordValue::new()
        .with("id", CapabilityValue::String(entry.id.clone()))
        .with("title", CapabilityValue::String(entry.title.clone()))
        .with("source", CapabilityValue::String(entry.source.as_str().to_string()));
    if !entry.hint.is_empty() {
        record = record.with("hint", CapabilityValue::String(entry.hint.clone()));
    }
    record
}

/// Decodes the `entries` write into palette entries.
///
/// The whole list is decoded before `set_entries` runs, so a refused row leaves the palette
/// unchanged.
fn palette_entries_from_records(
    list: &RecordList,
) -> Result<Vec<PaletteEntryRef>, CapabilityAccessError> {
    let mut entries = Vec::with_capacity(list.items().len());
    for record in list.items() {
        let id = match record.get("id") {
            Some(CapabilityValue::String(value)) => value.clone(),
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let title = match record.get("title") {
            Some(CapabilityValue::String(value)) => value.clone(),
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let hint = match record.get("hint") {
            None | Some(CapabilityValue::Null) => String::new(),
            Some(CapabilityValue::String(value)) => value.clone(),
            Some(_) => return Err(CapabilityAccessError::TypeMismatch),
        };
        let source = match record.get("source") {
            None | Some(CapabilityValue::Null) => PaletteSource::Host,
            Some(CapabilityValue::String(token)) => {
                PaletteSource::from_name(token).ok_or(CapabilityAccessError::TypeMismatch)?
            }
            Some(_) => return Err(CapabilityAccessError::TypeMismatch),
        };
        entries.push(PaletteEntryRef { id, title, hint, source });
    }
    Ok(entries)
}

impl WidgetProperties for CommandPaletteHost {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entries" => Ok(CapabilityValue::Records(RecordList::from_items(
                &PALETTE_ENTRY_SCHEMA,
                self.entries.iter().map(palette_entry_record).collect(),
            ))),
            "filter" => Ok(CapabilityValue::String(self.filter.clone())),
            "selected_index" => Ok(if self.visible_count() == 0 {
                CapabilityValue::Null
            } else {
                CapabilityValue::UInt(self.selected_index as u64)
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "entries" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&PALETTE_ENTRY_SCHEMA).map_err(|violation| {
                    log::warn!("command_palette_host.entries refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("entries"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                self.set_entries(palette_entries_from_records(&list)?);
                Ok(())
            }
            "filter" => {
                self.set_filter(expect_string(value)?);
                Ok(())
            }
            "selected_index" => match value {
                CapabilityValue::Null => {
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.base.request_redraw();
                    Ok(())
                }
                other => {
                    let index = expect_usize(other)?;
                    if self.set_selected_index(index) {
                        Ok(())
                    } else {
                        Err(CapabilityAccessError::OutOfRange)
                    }
                }
            },
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["entries", "filter", "selected_index", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn palette() -> CommandPaletteHost {
        let mut palette = CommandPaletteHost::new(Rect::new(0, 0, 480, 320));
        palette.set_entries(vec![
            PaletteEntryRef::new("editor.save", "Save File").with_hint("Write to disk"),
            PaletteEntryRef::new("terminal.clear", "Clear Terminal")
                .with_source(PaletteSource::Terminal),
            PaletteEntryRef::new("host.reload", "Reload Window").with_source(PaletteSource::Host),
        ]);
        palette.open();
        palette
    }

    #[test]
    fn the_filter_narrows_case_insensitively() {
        let mut palette = palette();
        assert_eq!(palette.visible_entries().len(), 3);

        palette.set_filter("SAVE");
        assert_eq!(palette.visible_entries().len(), 1);
        assert_eq!(palette.visible_entries()[0].id.as_str(), "editor.save");

        // The hint is a filter input too, not only the title and the id.
        palette.set_filter("write");
        assert_eq!(palette.visible_entries().len(), 1);
        assert_eq!(palette.visible_entries()[0].id.as_str(), "editor.save");

        palette.set_filter("terminal");
        assert_eq!(palette.visible_entries().len(), 1);
        assert_eq!(palette.visible_entries()[0].id.as_str(), "terminal.clear");

        // A filter that matches nothing leaves a selection that is still valid (there is none).
        palette.set_filter("no-such-command");
        assert!(palette.visible_entries().is_empty());
        assert_eq!(palette.selected_index(), 0);
        assert!(palette.selected_entry().is_none());
    }

    #[test]
    fn activation_emits_the_id_and_closes() {
        let mut palette = palette();
        let fired = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&fired);
        palette.command_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*id).clone());
            }
        });

        palette.set_filter("reload");
        assert!(palette.activate_selected());

        assert_eq!(*fired.lock().unwrap(), vec![String::from("host.reload")]);
        assert!(!palette.is_open());
    }

    #[test]
    fn escape_cancels_the_palette() {
        let mut palette = palette();
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        palette.cancelled.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        palette.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(!palette.is_open());
    }
}
