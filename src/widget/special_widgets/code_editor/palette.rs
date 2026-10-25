// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The command palette: a discoverable, filterable list of editor commands.
//!
//! # Why the model lives here and the popup does not
//!
//! The editor publishes around two dozen commands (comment toggle, block indent, folding,
//! bookmarks, multi-cursor, find/replace, line commands), and every one of them was
//! reachable only by remembering its chord. A palette is the discovery surface for exactly
//! that gap. What a palette needs from the editor is a **list of entries** and an
//! **executor for a command id**; both are editor facts, not drawing facts, so they live
//! here. The popup itself — a floating list with its own chrome — is a companion control
//! (`COMP-J01`); this module is the model that companion binds to, which is also what lets
//! a host render the list with its own look.
//!
//! # Filtering
//!
//! The query is matched as a case-insensitive substring first and as a subsequence second,
//! so `cmt` still finds "Toggle Line Comment" while an exact prefix always ranks above a
//! scattered match. Substring matches score by position (earlier is better) and by how
//! short the label is, which keeps "Fold All" ahead of "Fold All Except…" for `fold all`.

use super::editor::CodeEditor;
use crate::signal::{GenericSignal, Signal1};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// One row of a command palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    /// Human-readable name shown in the list.
    pub label: String,
    /// Optional secondary text (a category, a description).
    pub hint: Option<String>,
    /// Optional chord spelling, shown right-aligned.
    pub shortcut: Option<String>,
    /// The stable id handed to the executor.
    pub command_id: String,
    /// `false` when the command cannot run in the current state.
    pub enabled: bool,
}

impl PaletteEntry {
    /// Creates an enabled entry.
    pub fn new(label: impl Into<String>, command_id: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            shortcut: None,
            command_id: command_id.into(),
            enabled: true,
        }
    }

    /// Adds secondary text.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Adds a chord spelling.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Marks the entry as unavailable, so the list can grey it out.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// A filterable list of commands with a selection.
///
/// The model is deliberately independent of any popup geometry: it answers "which entries
/// match this query, in what order" and "which one is selected", and emits the selection's
/// command id. A host binds those answers to whatever surface it has.
pub struct CommandPalette {
    entries: Vec<PaletteEntry>,
    /// Indices into `entries` that match the current query, best first.
    filtered: Vec<usize>,
    query: String,
    selected: usize,
    max_visible_rows: usize,
    placeholder: Option<String>,
    /// Whether the palette is showing.
    visible: bool,
    /// Emitted with the `command_id` when the selection is executed.
    pub committed: Signal1<String>,
    /// Emitted when the palette is dismissed without executing anything.
    pub dismissed: GenericSignal,
    /// Emitted after the query changes.
    pub query_changed: Signal1<String>,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    /// Creates an empty palette.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            filtered: Vec::new(),
            query: String::new(),
            selected: 0,
            max_visible_rows: 12,
            placeholder: None,
            visible: false,
            committed: Signal1::new(),
            dismissed: GenericSignal::new(),
            query_changed: Signal1::new(),
        }
    }

    /// Replaces the entry list, preserving the query and re-filtering.
    pub fn set_entries(&mut self, entries: Vec<PaletteEntry>) {
        self.entries = entries;
        self.refilter();
    }

    /// Returns every entry, in the order supplied.
    pub fn entries(&self) -> &[PaletteEntry] {
        &self.entries
    }

    /// Opens the palette, optionally with an initial query.
    pub fn open(&mut self, initial_query: &str) {
        self.visible = true;
        self.query = initial_query.to_string();
        self.refilter();
        self.selected = 0;
        self.query_changed.emit(self.query.clone());
    }

    /// Closes the palette and reports the dismissal.
    pub fn close(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.dismissed.emit();
    }

    /// Returns whether the palette is showing.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns the query typed so far.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replaces the query and re-filters.
    pub fn set_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        self.query = query.to_string();
        self.refilter();
        self.selected = 0;
        self.query_changed.emit(self.query.clone());
    }

    /// Returns the entry indices matching `query`, best first, without changing state.
    pub fn filter_for(&self, query: &str) -> Vec<usize> {
        let mut ranked: Vec<(i32, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let score = match_score(&entry.label, query)?;
                // A disabled entry is still listed (the reader can see the command exists)
                // but ranks below every enabled one when the query does not distinguish.
                let score = if entry.enabled { score } else { score - 10_000 };
                Some((score, index))
            })
            .collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        ranked.into_iter().map(|(_, index)| index).collect()
    }

    /// The filtered indices, best first.
    pub fn filtered(&self) -> &[usize] {
        &self.filtered
    }

    /// The selected row's index into `filtered`.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The selected entry, if any.
    pub fn selected_entry(&self) -> Option<&PaletteEntry> {
        self.entries.get(*self.filtered.get(self.selected)?)
    }

    /// Moves the highlight by `delta` rows, wrapping around.
    pub fn move_selection(&mut self, delta: isize) -> bool {
        if self.filtered.is_empty() {
            return false;
        }
        let count = self.filtered.len() as isize;
        let current = self.selected.min(self.filtered.len() - 1) as isize;
        self.selected = (current + delta).rem_euclid(count) as usize;
        true
    }

    /// Selects the row at `index` within the filtered list.
    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.filtered.len() {
            return false;
        }
        if self.selected == index {
            return false;
        }
        self.selected = index;
        true
    }

    /// Marks the highlight at `index` unavailable.
    ///
    /// A host that keeps its own entry list can update one row without rebuilding the
    /// palette, which is what "the selection is not currently actionable" needs.
    pub fn set_entry_enabled(&mut self, index: usize, enabled: bool) -> bool {
        let Some(entry) = self.entries.get_mut(index) else { return false };
        if entry.enabled == enabled {
            return false;
        }
        entry.enabled = enabled;
        self.refilter();
        true
    }

    /// Executes the selected entry, returning its command id.
    ///
    /// The palette closes and emits `committed`; a disabled selection executes nothing and
    /// leaves the palette open, which is the difference between "nothing happened" and
    /// "the command refused".
    pub fn execute_selected(&mut self) -> Option<&str> {
        let entry_index = *self.filtered.get(self.selected)?;
        if !self.entries.get(entry_index).is_some_and(|entry| entry.enabled) {
            return None;
        }
        let command = self.entries[entry_index].command_id.clone();
        self.visible = false;
        self.committed.emit(command);
        Some(self.entries[entry_index].command_id.as_str())
    }

    /// Rows the popup may show at once (`0` means "all").
    pub fn max_visible_rows(&self) -> usize {
        self.max_visible_rows
    }

    /// Sets the visible-row cap.
    pub fn set_max_visible_rows(&mut self, rows: usize) {
        self.max_visible_rows = rows;
    }

    /// The placeholder shown when the query is empty.
    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }

    /// Sets the placeholder.
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = Some(placeholder.into());
    }

    /// Recomputes the filtered list and clamps the selection into it.
    fn refilter(&mut self) {
        self.filtered = self.filter_for(&self.query);
        self.selected =
            if self.filtered.is_empty() { 0 } else { self.selected.min(self.filtered.len() - 1) };
    }
}

/// Scores `label` against `query`, or `None` when it does not match at all.
///
/// A substring match always outranks a subsequence match; within each class, earlier and
/// shorter is better. The scores are only ever compared, never displayed, so their scale
/// is arbitrary — what matters is that the ordering is total and stable.
fn match_score(label: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let label_lower = label.to_lowercase();
    let query_lower = query.to_lowercase();
    if let Some(position) = label_lower.find(&query_lower) {
        return Some(10_000 - position as i32 - label_lower.chars().count() as i32 / 4);
    }
    // Subsequence: every query character appears in order. Count the characters skipped,
    // so a tight match (`fmt` in `format`) beats a scattered one (`fmt` in `from_meta_tab`).
    let mut skipped = 0i32;
    let mut last = 0usize;
    let mut chars = label_lower.char_indices();
    for wanted in query_lower.chars() {
        loop {
            let (index, candidate) = chars.next()?;
            if candidate == wanted {
                last = index;
                break;
            }
            skipped += 1;
        }
    }
    Some(5_000 - skipped - last as i32)
}

impl CodeEditor {
    /// Every built-in editor command, as a palette entry.
    ///
    /// The list is the discovery surface `COMP-J01` asks for: each entry carries the id
    /// [`Self::execute_palette_command`] understands, so a host can drop these into a
    /// [`CommandPalette`] and wire `committed` straight back to the executor. Shortcuts are
    /// the chords [`Self::handle_key_press`] already binds.
    pub fn editor_commands(&self) -> Vec<PaletteEntry> {
        alloc::vec![
            PaletteEntry::new("Toggle Line Comment", "toggle_comment")
                .with_hint("Editing")
                .with_shortcut("Cmd+/"),
            PaletteEntry::new("Indent Selection", "indent").with_hint("Editing"),
            PaletteEntry::new("Outdent Selection", "outdent").with_hint("Editing"),
            PaletteEntry::new("Duplicate Line", "duplicate_line").with_hint("Editing"),
            PaletteEntry::new("Delete Line", "delete_line").with_hint("Editing"),
            PaletteEntry::new("Sort Lines", "sort_lines").with_hint("Editing"),
            PaletteEntry::new("Sort Lines Descending", "sort_lines_descending")
                .with_hint("Editing"),
            PaletteEntry::new("Sort Lines Unique", "sort_lines_unique").with_hint("Editing"),
            PaletteEntry::new("Move Line Up", "move_line_up").with_hint("Editing"),
            PaletteEntry::new("Move Line Down", "move_line_down").with_hint("Editing"),
            PaletteEntry::new("Fold Block", "fold_block").with_hint("Folding"),
            PaletteEntry::new("Fold All", "fold_all").with_hint("Folding"),
            PaletteEntry::new("Unfold All", "unfold_all").with_hint("Folding"),
            PaletteEntry::new("Toggle Bookmark", "toggle_bookmark").with_hint("Navigation"),
            PaletteEntry::new("Next Bookmark", "next_bookmark").with_hint("Navigation"),
            PaletteEntry::new("Go to Line", "goto_line")
                .with_hint("Navigation")
                .with_shortcut("Cmd+G"),
            PaletteEntry::new("Find", "find").with_hint("Search").with_shortcut("Cmd+F"),
            PaletteEntry::new("Replace", "replace").with_hint("Search").with_shortcut("Cmd+H"),
            PaletteEntry::new("Next Diagnostic", "next_diagnostic").with_hint("Diagnostics"),
            PaletteEntry::new("Previous Diagnostic", "previous_diagnostic")
                .with_hint("Diagnostics"),
            PaletteEntry::new("Add Cursor Above", "add_cursor_above").with_hint("Multi-cursor"),
            PaletteEntry::new("Add Cursor Below", "add_cursor_below").with_hint("Multi-cursor"),
            PaletteEntry::new("Select All Occurrences", "select_occurrences")
                .with_hint("Multi-cursor"),
            PaletteEntry::new("Collapse Cursors", "collapse_cursors").with_hint("Multi-cursor"),
            PaletteEntry::new("Completion", "complete").with_hint("Editing"),
            PaletteEntry::new("Copy", "copy").with_hint("Clipboard").with_shortcut("Cmd+C"),
            PaletteEntry::new("Cut", "cut").with_hint("Clipboard").with_shortcut("Cmd+X"),
            PaletteEntry::new("Paste", "paste").with_hint("Clipboard").with_shortcut("Cmd+V"),
            PaletteEntry::new("Select All", "select_all")
                .with_hint("Selection")
                .with_shortcut("Cmd+A"),
            PaletteEntry::new("Undo", "undo").with_hint("History").with_shortcut("Cmd+Z"),
            PaletteEntry::new("Redo", "redo").with_hint("History").with_shortcut("Cmd+Shift+Z"),
        ]
    }

    /// Runs the command named by a palette entry, returning whether it was recognised.
    ///
    /// Every arm calls the same method the key binding calls, so the palette cannot drift
    /// from the keyboard path. A command that needs state the editor does not have (no
    /// selection to sort, nothing to paste) returns `true` — it ran and declined — while an
    /// unknown id returns `false`.
    pub fn execute_palette_command(&mut self, command_id: &str) -> bool {
        match command_id {
            "toggle_comment" => self.toggle_line_comment(),
            "indent" => self.indent_selection(),
            "outdent" => self.outdent_selection(),
            "duplicate_line" => self.duplicate_line(),
            "delete_line" => self.delete_line(),
            "sort_lines" => self.sort_lines(),
            "sort_lines_descending" => self.sort_lines_descending(),
            "sort_lines_unique" => self.sort_lines_unique(),
            "move_line_up" => self.move_line(-1),
            "move_line_down" => self.move_line(1),
            "fold_block" => {
                let line = self.cursor.head.line;
                self.toggle_fold(line);
            }
            "fold_all" => {
                self.fold_all();
            }
            "unfold_all" => self.unfold_all(),
            "toggle_bookmark" => {
                let line = self.cursor.head.line;
                self.toggle_bookmark(line);
            }
            "next_bookmark" => {
                self.goto_next_bookmark();
            }
            "goto_line" => self.open_goto_bar(),
            "find" => self.open_find(false),
            "replace" => self.open_find(true),
            "next_diagnostic" => {
                self.next_marker();
            }
            "previous_diagnostic" => {
                self.previous_marker();
            }
            "add_cursor_above" => {
                self.add_cursor_above();
            }
            "add_cursor_below" => {
                self.add_cursor_below();
            }
            "select_occurrences" => {
                self.select_all_occurrences();
            }
            "collapse_cursors" => {
                self.collapse_cursors();
            }
            "complete" => {
                self.trigger_completion();
            }
            "copy" => {
                self.copy();
            }
            "cut" => {
                self.cut();
            }
            "paste" => {
                self.paste();
            }
            "select_all" => self.select_all(),
            "undo" => {
                self.undo();
            }
            "redo" => {
                self.redo();
            }
            _ => return false,
        }
        true
    }
}
