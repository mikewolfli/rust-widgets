// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MarkdownEditor` state, document editing, caret and scroll handling.

use super::*;
use crate::core::{Font, Rect};
use crate::signal::Signal1;
use crate::style::animation::CursorBlink;
use crate::undo::UndoStack;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::special_widgets::code_editor::{
    BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter,
};
use crate::widget::{BaseWidget, Widget, WidgetKind};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Lightweight markdown editor with preview toggle and metrics.
pub struct MarkdownEditor {
    pub(crate) base: BaseWidget,
    pub(crate) text: String,
    pub(crate) preview_mode: bool,
    pub(crate) cursor_line: usize,
    /// Character offset of the caret within [`Self::cursor_line`].
    pub(crate) cursor_col: usize,
    /// The first text line the viewport paints.
    ///
    /// The editor used to paint a fixed ten lines, so a longer document simply ended at row ten.
    /// This is the scroll state the wheel drives; the paint path starts here and takes only the
    /// rows that fit.
    pub(crate) first_visible_line: usize,
    /// Emitted when text changes.
    pub text_changed: Signal1<MarkdownTextChange>,
    /// Emitted when preview mode changes.
    pub preview_mode_changed: Signal1<bool>,
    /// Emitted when a selection appears (`true`) or disappears (`false`).
    pub selection_changed: Signal1<bool>,
    /// Emitted when a heading row is activated by a click, with its outline index.
    pub heading_activated: Signal1<usize>,
    pub(crate) undo_stack: UndoStack,
    pub(crate) history_target: Rc<RefCell<String>>,
    pub(crate) restoring_history: bool,
    /// The configured undo depth, so the property can be read back after a write.
    pub(crate) undo_stack_limit: usize,
    /// The selection as ordered document character offsets.
    pub(crate) selection: Option<(usize, usize)>,
    /// Where the selection started, as `(line, column)`, for shift-extension.
    pub(crate) selection_anchor: Option<(usize, usize)>,
    /// Whether a pointer drag is still extending the selection.
    pub(crate) selecting: bool,
    /// Uncommitted IME composition, drawn at the caret.
    pub(crate) preedit: Option<Preedit>,
    /// The caret blink. Unlike the other two text controls this one had none.
    pub(crate) cursor_blink: CursorBlink,
    /// Whether the editor paints a scrollbar beside the text.
    pub(crate) show_scrollbar: bool,
    /// Whether a drag on the scrollbar thumb is in progress.
    pub(crate) dragging_scrollbar: bool,
    /// The find bar.
    pub(crate) find: FindState,
    /// The mounted find/replace bar.
    ///
    /// It is a view over this editor's bytes, not the editor's own find state: the bar paints the
    /// query and the match counter while [`Self::find`] stays the navigation state the editor's
    /// keyboard path uses. [`Self::show_find_bar`] is what points the two at the same text.
    pub(crate) find_bar: FindBar,
    /// The mounted document outline.
    pub(crate) outline_panel: MarkdownOutlinePanel,
    /// Whether the outline panel is painted inside the editor's own rectangle.
    pub(crate) show_outline: bool,
    /// The revision the panel's entries were last rebuilt for.
    ///
    /// `outline()` walks every line, so it is rebuilt when the text changes rather than on every
    /// frame; the active row still follows the caret on each paint.
    pub(crate) outline_revision: u64,
    /// Whether GFM constructs (tables, tasks, strikethrough, quotes, fences, rules) are parsed.
    pub(crate) gfm_extensions: bool,
    /// Whether fenced code blocks are tokenized with the shared lexer.
    pub(crate) code_highlight: bool,
    /// Counts text mutations, so caches can tell a stale frame from a live one.
    pub(crate) revision: u64,
    /// `line_count` / `word_count` / `heading_count`, keyed by revision.
    pub(crate) metrics: RefCell<Option<(u64, Metrics)>>,
    /// Parsed preview rows, keyed by the row's content and the context it was parsed under.
    ///
    /// The key deliberately does **not** carry the row's index: inserting a line above the
    /// viewport shifted every index and re-parsed the whole visible page even though no visible
    /// text had changed.
    pub(crate) preview_cache: HashMap<PreviewKey, Rc<ParsedLine>>,
    /// The header line's formatting buffer, reused across frames.
    ///
    /// The header is rebuilt every frame from the revision-keyed metrics; `format!` gave it a
    /// fresh `String` each time, so a still editor allocated once per frame for text that had not
    /// changed.
    pub(crate) header_buffer: String,
    /// Per-line block classification, rebuilt when the revision changes.
    pub(crate) blocks: Vec<BlockStart>,
    /// Whether the line *after* each line is a table divider, so a row can be recognised as a
    /// table header without the paint path looking ahead (which would be quadratic).
    pub(crate) table_header: Vec<bool>,
    /// The revision `blocks` was built for.
    pub(crate) blocks_revision: u64,
    /// Measured glyph advances for the caret's line prefix, keyed by font size.
    pub(crate) char_widths: Vec<(char, f32, i32)>,
    /// Line offset tables for [`Self::revision`], rebuilt at most once per revision.
    pub(crate) line_index: RefCell<Option<(u64, LineIndex)>>,
}

impl MarkdownEditor {
    /// Creates editor.
    pub fn new(geometry: Rect) -> Self {
        Self::with_undo_limit(geometry, 100)
    }

    /// Creates an editor whose undo history keeps at most `undo_limit` steps.
    ///
    /// The stack used to be the default 100 with no way to reach it, so a long writing session
    /// silently lost everything past the hundredth edit.
    pub fn with_undo_limit(geometry: Rect, undo_limit: usize) -> Self {
        let undo_limit = undo_limit.max(1);
        let mut cursor_blink = CursorBlink::new();
        // The editor is an input surface by definition, so a freshly mounted one is already
        // waiting for typing; waiting for a focus event a host may not send would make this the
        // only text control in the crate whose caret never blinks.
        cursor_blink.start();
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "MarkdownEditor"),
            text: String::new(),
            preview_mode: false,
            cursor_line: 0,
            cursor_col: 0,
            first_visible_line: 0,
            text_changed: Signal1::new(),
            preview_mode_changed: Signal1::new(),
            selection_changed: Signal1::new(),
            heading_activated: Signal1::new(),
            undo_stack: UndoStack::with_capacity(undo_limit),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            undo_stack_limit: undo_limit,
            selection: None,
            selection_anchor: None,
            selecting: false,
            preedit: None,
            cursor_blink,
            show_scrollbar: true,
            dragging_scrollbar: false,
            find: FindState::default(),
            find_bar: FindBar::new(Rect::new(0, 0, 0, 0)),
            outline_panel: MarkdownOutlinePanel::new(Rect::new(0, 0, 0, 0)),
            show_outline: false,
            outline_revision: u64::MAX,
            gfm_extensions: true,
            code_highlight: true,
            revision: 0,
            metrics: RefCell::new(None),
            preview_cache: HashMap::new(),
            header_buffer: String::new(),
            blocks: Vec::new(),
            table_header: Vec::new(),
            blocks_revision: u64::MAX,
            char_widths: Vec::new(),
            line_index: RefCell::new(None),
        }
    }

    /// Returns markdown text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets markdown text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let next = text.into();
        self.commit_text(next);
    }

    /// Inserts `text` at the caret and leaves the caret after the inserted run.
    ///
    /// This is the entry point for platform text input (`Event::TextInput` and
    /// `Event::ImeCommit`). The write goes through `commit_text`, so it is one undo step
    /// and emits `text_changed` exactly like every other text mutation.
    pub fn insert_text_at_cursor(&mut self, text: &str) {
        // Control characters are protocol, not keystrokes: a `\n` delivered through
        // `TextInput` must not split a line, and an escape must not enter the document.
        // This is the same filter `Mention` applies to the same event.
        let filtered: String = text.chars().filter(|ch| !ch.is_control()).collect();
        if filtered.is_empty() {
            return;
        }
        // Typing over a selection replaces it in the same undo step.
        self.replace_selection_or_caret(&filtered);
    }

    /// Replaces the selection with `insert`, or inserts at the caret when nothing is selected.
    ///
    /// One commit, so typing over a selection is one undo step rather than a delete followed by
    /// an insert.
    pub(crate) fn replace_selection_or_caret(&mut self, insert: &str) {
        let caret = self.doc_char_offset_of_caret();
        let (start, end) = self.selection.unwrap_or((caret, caret));
        let total = self.total_char_count();
        let start = start.min(total);
        let end = end.min(total).max(start);
        // Only the affected byte range is spliced: the untouched head and tail are copied once
        // instead of the document being rebuilt character by character.
        let start_byte = self.char_offset_to_byte(start);
        let end_byte = self.char_offset_to_byte(end);
        let mut next =
            String::with_capacity(self.text.len() - (end_byte - start_byte) + insert.len());
        next.push_str(&self.text[..start_byte]);
        next.push_str(insert);
        next.push_str(&self.text[end_byte..]);
        let (line, col) = self.line_col_of(start);
        self.cursor_line = line;
        self.cursor_col = col + insert.chars().count();
        self.selection_anchor = None;
        self.apply_selection(None);
        self.commit_text(next);
    }

    /// The caret as a document character offset.
    pub(crate) fn doc_char_offset_of_caret(&self) -> usize {
        self.doc_char_offset(self.cursor_line, self.cursor_col)
    }

    /// The line offset tables, rebuilt at most once per revision.
    pub(crate) fn ensure_line_index(&self) {
        let fresh = self
            .line_index
            .borrow()
            .as_ref()
            .is_some_and(|(revision, _)| *revision == self.revision);
        if !fresh {
            let index = LineIndex::new(&self.text);
            *self.line_index.borrow_mut() = Some((self.revision, index));
        }
    }

    /// Runs `read` against the current line tables.
    pub(crate) fn with_line_index<R>(&self, read: impl FnOnce(&LineIndex) -> R) -> R {
        self.ensure_line_index();
        let index = self.line_index.borrow();
        let (_, table) = index.as_ref().expect("ensure_line_index() populates the table");
        read(table)
    }

    /// The document's character count, including its `\n` separators.
    pub(crate) fn total_char_count(&self) -> usize {
        self.with_line_index(LineIndex::char_len)
    }

    /// The byte offset of a document character offset.
    pub(crate) fn char_offset_to_byte(&self, offset: usize) -> usize {
        self.with_line_index(|index| {
            let line = index.line_of_char(offset);
            let start_byte = index.starts.get(line).copied().unwrap_or(self.text.len());
            let line_end = match index.starts.get(line + 1) {
                Some(next) => next.saturating_sub(1).min(self.text.len()),
                None => self.text.len(),
            };
            let line_length = index.char_lens.get(line).copied().unwrap_or(0);
            let column = offset.saturating_sub(index.char_starts.get(line).copied().unwrap_or(0));
            let column = column.min(line_length);
            start_byte
                + self.text[start_byte..line_end]
                    .char_indices()
                    .nth(column)
                    .map(|(byte, _)| byte)
                    .unwrap_or(line_end - start_byte)
        })
    }

    /// Appends one line to markdown.
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        let line = line.as_ref();
        let needs_separator = !self.text.is_empty();
        let mut next =
            String::with_capacity(self.text.len() + line.len() + usize::from(needs_separator));
        next.push_str(&self.text);
        if needs_separator {
            next.push('\n');
        }
        next.push_str(line);
        self.set_text(next);
        self.cursor_line = self.edit_line_count().saturating_sub(1);
        self.cursor_col = self.current_line_char_len();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Toggles preview mode.
    pub fn toggle_preview_mode(&mut self) {
        self.preview_mode = !self.preview_mode;
        self.preview_mode_changed.emit(self.preview_mode);
        self.base.request_redraw();
    }

    /// Sets preview mode.
    pub fn set_preview_mode(&mut self, preview_mode: bool) {
        if self.preview_mode == preview_mode {
            return;
        }
        self.preview_mode = preview_mode;
        self.preview_mode_changed.emit(preview_mode);
        self.base.request_redraw();
    }

    /// Returns whether preview mode is enabled.
    pub fn preview_mode(&self) -> bool {
        self.preview_mode
    }

    /// Returns line count.
    pub fn line_count(&self) -> usize {
        self.metrics_snapshot().lines
    }

    /// Returns word count.
    pub fn word_count(&self) -> usize {
        self.metrics_snapshot().words
    }

    /// Returns heading count.
    pub fn heading_count(&self) -> usize {
        self.metrics_snapshot().headings
    }

    /// The document statistics, computed once per revision.
    ///
    /// The three public counters each walked the whole document, and the paint path called all
    /// three per frame just to build its header line. One pass per revision answers all of them.
    pub(crate) fn metrics_snapshot(&self) -> Metrics {
        if let Some((revision, metrics)) = *self.metrics.borrow() {
            if revision == self.revision {
                return metrics;
            }
        }
        let metrics = self.text.lines().fold(Metrics::default(), |mut total, line| {
            total.lines += 1;
            total.words += line.split_whitespace().count();
            total.headings += usize::from(line.trim_start().starts_with('#'));
            total
        });
        *self.metrics.borrow_mut() = Some((self.revision, metrics));
        metrics
    }

    /// Returns headings in document order, as navigation targets.
    pub fn outline(&self) -> Vec<MarkdownOutlineEntry> {
        self.text
            .lines()
            .enumerate()
            .filter_map(|(line, text)| {
                let trimmed = text.trim_start();
                let level = trimmed.chars().take_while(|ch| *ch == '#').count();
                (level > 0).then_some(MarkdownOutlineEntry {
                    line,
                    level: level.min(6) as u8,
                    title: trimmed[level..].trim().to_string(),
                })
            })
            .collect()
    }

    /// Moves the caret to outline entry `index`.
    pub fn goto_outline(&mut self, index: usize) -> bool {
        let Some(entry) = self.outline().get(index).cloned() else {
            return false;
        };
        self.cursor_line = entry.line;
        self.cursor_col = 0;
        // Scroll the target into view rather than leaving it off screen.
        if entry.line < self.first_visible_line {
            self.first_visible_line = entry.line;
        } else if entry.line >= self.first_visible_line + self.visible_line_count() {
            self.first_visible_line = entry.line + 1 - self.visible_line_count();
        }
        self.base.request_redraw();
        true
    }

    /// Returns current line index.
    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }

    /// Returns the caret's character offset within the current line.
    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    /// The selection as ordered document character offsets.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    /// The selected text, when a selection exists.
    ///
    /// The selection is stored as ordered character offsets, so the text is taken by slicing the
    /// bytes those offsets address. The previous form collected the *whole* document into a
    /// character vector on every call, which made copying one word cost O(document).
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection?;
        let total = self.total_char_count();
        let start = start.min(total);
        let end = end.min(total).max(start);
        let start_byte = self.char_offset_to_byte(start);
        let end_byte = self.char_offset_to_byte(end);
        Some(self.text[start_byte..end_byte].to_string())
    }

    /// Selects the whole document.
    pub fn select_all(&mut self) {
        // The length comes from the cached line tables; counting the document's characters here
        // walked it once per call.
        let length = self.total_char_count();
        self.selection_anchor = Some((0, 0));
        self.apply_selection(if length == 0 { None } else { Some((0, length)) });
        self.cursor_line = self.edit_line_count().saturating_sub(1);
        self.cursor_col = self.current_line_char_len();
        self.base.request_redraw();
    }

    /// Copies the selection to the clipboard, reporting whether anything was copied.
    pub fn copy(&mut self) -> bool {
        let Some(text) = self.selected_text() else {
            return false;
        };
        crate::clipboard::ClipboardManager::set_text(text)
    }

    /// Cuts the selection to the clipboard, reporting whether anything was cut.
    pub fn cut(&mut self) -> bool {
        let Some(text) = self.selected_text() else {
            return false;
        };
        let copied = crate::clipboard::ClipboardManager::set_text(text);
        self.delete_selection();
        copied
    }

    /// Pastes the clipboard at the caret, replacing any selection.
    pub fn paste(&mut self) -> bool {
        let text = crate::clipboard::ClipboardManager::text();
        if text.is_empty() {
            return false;
        }
        let filtered: String = text.chars().filter(|ch| !ch.is_control() || *ch == '\n').collect();
        if filtered.is_empty() {
            return false;
        }
        self.replace_selection_or_caret(&filtered);
        true
    }

    /// Moves the caret to `line` and scrolls it into view.
    pub fn goto_line(&mut self, line: usize) -> bool {
        if line >= self.edit_line_count() {
            return false;
        }
        self.cursor_line = line;
        self.cursor_col = 0;
        self.selection_anchor = None;
        self.apply_selection(None);
        self.reveal_caret();
        self.base.request_redraw();
        true
    }

    /// Finds the next occurrence of `needle` at or after the caret, wrapping to the start.
    ///
    /// The match becomes the selection and the caret moves to its end, which is what makes
    /// repeated calls step through the document instead of finding the same match again.
    pub fn find_next(&mut self, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        let haystack = &self.text;
        let from = self.find_search_origin(true);
        let found = haystack
            .get(from..)
            .and_then(|tail| tail.find(needle).map(|offset| from + offset))
            .or_else(|| haystack.find(needle));
        let Some(byte) = found else {
            self.find.current = None;
            return false;
        };
        self.find.current = Some(byte);
        self.select_byte_range(byte, byte + needle.len());
        true
    }

    /// Finds the previous occurrence of `needle` before the caret, wrapping to the end.
    pub fn find_previous(&mut self, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        let haystack = &self.text;
        let from = self.find_search_origin(false);
        let found = haystack
            .get(..from.min(haystack.len()))
            .and_then(|head| head.rfind(needle))
            .or_else(|| haystack.rfind(needle));
        let Some(byte) = found else {
            self.find.current = None;
            return false;
        };
        self.find.current = Some(byte);
        self.select_byte_range(byte, byte + needle.len());
        true
    }

    /// Whether the find bar is showing.
    pub fn find_bar_visible(&self) -> bool {
        self.find.visible
    }

    /// The find bar's current query.
    pub fn find_query(&self) -> &str {
        &self.find.query
    }

    /// Opens the find bar and focuses its query field.
    pub fn open_find(&mut self) {
        self.show_find_bar();
    }

    /// Opens the mounted [`FindBar`] over this editor's text.
    ///
    /// This is the host-facing form of [`Self::open_find`]: the bar is told the current document
    /// snapshot and the query the editor already holds, so the painted counter and the editor's
    /// Enter-to-next match refer to the same bytes.
    pub fn show_find_bar(&mut self) {
        self.find.visible = true;
        self.find_bar.open();
        let text = self.text.clone();
        self.find_bar.set_text(&text);
        if !self.find.query.is_empty() {
            self.find_bar.set_query(self.find.query.clone());
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes the find bar, leaving the document and the current match untouched.
    pub fn close_find(&mut self) {
        if self.find.visible {
            self.find.visible = false;
            self.find_bar.close();
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The mounted find/replace bar.
    pub fn find_bar(&self) -> &FindBar {
        &self.find_bar
    }

    /// The mounted find/replace bar, for a host that wants to set its replacement or policy.
    pub fn find_bar_mut(&mut self) -> &mut FindBar {
        &mut self.find_bar
    }

    /// The mounted document outline.
    pub fn outline_panel(&self) -> &MarkdownOutlinePanel {
        &self.outline_panel
    }

    /// The mounted document outline, for a host that wants to set its width or listen to it.
    pub fn outline_panel_mut(&mut self) -> &mut MarkdownOutlinePanel {
        &mut self.outline_panel
    }

    /// Whether the outline panel is painted.
    pub fn show_outline(&self) -> bool {
        self.show_outline
    }

    /// Shows or hides the outline panel inside the editor's rectangle.
    pub fn set_show_outline(&mut self, show: bool) {
        if self.show_outline == show {
            return;
        }
        self.show_outline = show;
        if show {
            self.sync_outline();
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Rebuilds the mounted panel from this editor and marks the caret's heading.
    ///
    /// Entries are rebuilt only when the document revision changed; the active row is re-derived
    /// from the caret every call, so moving the caret is O(headings), not O(document).
    pub fn sync_outline(&mut self) {
        if self.outline_revision != self.revision {
            let entries = self.outline();
            self.outline_panel.set_entries(entries);
            self.outline_revision = self.revision;
        }
        let caret = self.cursor_line;
        let active = self
            .outline_panel
            .entries()
            .iter()
            .rposition(|entry| entry.line <= caret)
            .or_else(|| (!self.outline_panel.entries().is_empty()).then_some(0));
        self.outline_panel.set_active_index(active);
    }

    /// Points a [`FindBar`] at this editor's document.
    ///
    /// The bar keeps a snapshot of the text its byte offsets address, and it cannot observe an
    /// edit, so this is the call that keeps the two in step. The editor's own in-place find state
    /// is untouched: the bar is a second, reusable view over the same bytes.
    pub fn sync_find_bar(&self, bar: &mut FindBar) {
        bar.set_text(&self.text);
    }

    /// Applies a [`FindBar`] replacement request as one undo step.
    ///
    /// Returns how many ranges were replaced. A request whose ranges are out of order, overlap, or
    /// fall outside the document applies **nothing** and returns zero: a partial application would
    /// leave the text in a state neither the reader nor the bar asked for. Valid ranges are
    /// spliced from the end backwards so every earlier offset stays meaningful.
    pub fn apply_replace_request(&mut self, request: &ReplaceRequest) -> usize {
        let len = self.text.len();
        let mut last_start = len;
        for &(start, end) in request.ranges.iter().rev() {
            if start > end || end > len || end > last_start {
                return 0;
            }
            last_start = start;
        }
        if request.ranges.is_empty() {
            return 0;
        }
        let mut next = self.text.clone();
        for &(start, end) in request.ranges.iter().rev() {
            next.replace_range(start..end, &request.text);
        }
        let replaced = request.ranges.len();
        self.commit_text(next);
        // The bar holds a document snapshot, so it is re-pointed at the text the replacement just
        // produced; and it is told how many ranges actually changed, which is the feedback the
        // replacement row paints.
        self.find_bar.set_text(&self.text);
        self.find_bar.set_last_replaced(replaced);
        replaced
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is owed.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let owes_frame = self.cursor_blink.tick(delta_ms);
        if owes_frame {
            // A blink flips one glyph-wide bar, so a damage-tracking host is asked for that
            // rectangle instead of the whole control.
            self.request_caret_repaint();
        }
        owes_frame
    }

    /// The caret's own rectangle, or `None` when this frame has no caret on screen.
    ///
    /// Preview has no caret and an off-screen caret has nothing to paint, so both answer
    /// `None` and the caller keeps the whole-control repaint.
    #[cfg(not(alloc_frugal))]
    pub(crate) fn caret_rect(&self) -> Option<Rect> {
        if self.preview_mode {
            return None;
        }
        let row = self
            .cursor_line
            .checked_sub(self.visible_start())
            .filter(|row| *row < self.visible_line_count())?;
        let rect = self.geometry();
        let font = effective_font(self.base.style());
        let (line_start, line_end) = self.cursor_line_span();
        let line = &self.text[line_start..line_end];
        let caret_byte = byte_offset_of_char(line, self.cursor_col);
        let x = rect.x + 12 + estimate_text_width(&line[..caret_byte], font, 1.0) as i32;
        let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
        if y + 14 > rect.y + rect.height as i32 || x > rect.x + rect.width as i32 - 12 {
            return None;
        }
        // The composition is drawn at the caret, so a frame painting it owns that width too.
        let width = match &self.preedit {
            Some(preedit) => estimate_text_width(&preedit.text, font, 1.0).max(1),
            None => 1,
        };
        Some(Rect::new(x, y - 2, width, TEXT_ROW_HEIGHT as u32))
    }

    /// Submits one caret-blink frame as damage bounded by the caret's own rectangle.
    ///
    /// This mirrors the crate's other animated controls: a mounted control in a damage-tracking
    /// repaint mode asks for the rectangle it changed, while every other host keeps the
    /// whole-control request it already understood.
    pub(crate) fn request_caret_repaint(&self) {
        #[cfg(not(alloc_frugal))]
        {
            if let (Some(bounds), Some(id)) =
                (self.caret_rect(), crate::widget::runtime::registry_id_of(self.id()))
            {
                if crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full
                {
                    crate::widget::runtime::request_repaint_rect(id, bounds);
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// Whether the caret is drawn on the current frame.
    pub fn is_caret_visible(&self) -> bool {
        self.cursor_blink.is_visible()
    }

    /// Whether the editor paints a scrollbar.
    pub fn show_scrollbar(&self) -> bool {
        self.show_scrollbar
    }

    /// Shows or hides the scrollbar.
    pub fn set_show_scrollbar(&mut self, show: bool) {
        if self.show_scrollbar == show {
            return;
        }
        self.show_scrollbar = show;
        self.base.request_redraw();
    }

    /// Whether GFM extensions are parsed in preview.
    pub fn gfm_extensions(&self) -> bool {
        self.gfm_extensions
    }

    /// Enables or disables the GFM extension set.
    pub fn set_gfm_extensions(&mut self, enabled: bool) {
        if self.gfm_extensions == enabled {
            return;
        }
        self.gfm_extensions = enabled;
        self.preview_cache.clear();
        self.base.request_redraw();
    }

    /// Whether fenced code blocks are syntax-highlighted.
    pub fn code_highlight(&self) -> bool {
        self.code_highlight
    }

    /// Enables or disables fenced-code highlighting.
    pub fn set_code_highlight(&mut self, enabled: bool) {
        if self.code_highlight == enabled {
            return;
        }
        self.code_highlight = enabled;
        self.preview_cache.clear();
        self.base.request_redraw();
    }

    /// The configured undo depth.
    pub fn undo_stack_limit(&self) -> usize {
        self.undo_stack_limit
    }

    /// Sets the undo depth and trims the existing history to it.
    pub fn set_undo_limit(&mut self, limit: usize) {
        let limit = limit.max(1);
        if self.undo_stack_limit == limit {
            return;
        }
        self.undo_stack_limit = limit;
        self.undo_stack.set_max_capacity(limit);
    }

    /// The IME composition's screen rectangle, for a host placing its candidate window.
    ///
    /// The rectangle starts at the composition caret — the insertion point inside the preedit —
    /// not at the start of the composition, because that is where the candidate window belongs.
    /// Returns `None` when no composition is in progress: a rectangle for text that is not on
    /// screen would put the candidate window somewhere the user is not typing.
    pub fn preedit_screen_rect(&self) -> Option<Rect> {
        let preedit = self.preedit.as_ref()?;
        if preedit.text.is_empty() {
            return None;
        }
        let rect = self.geometry();
        let font = effective_font(self.base.style());
        let (line_start, line_end) = self.cursor_line_span();
        let caret_line = &self.text[line_start..line_end];
        let caret_byte = byte_offset_of_char(caret_line, self.cursor_col);
        let before_caret = &preedit.text[..char_boundary_at(&preedit.text, preedit.cursor)];
        let x = rect.x
            + 12
            + estimate_text_width(&caret_line[..caret_byte], font, 1.0) as i32
            + estimate_text_width(before_caret, font, 1.0) as i32;
        let row = self.cursor_line.saturating_sub(self.visible_start());
        let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
        let width = estimate_text_width(&preedit.text, font, 1.0).max(1);
        Some(Rect::new(x, y, width, TEXT_ROW_HEIGHT as u32))
    }

    /// Moves the caret to the start or the end of the current line.
    pub fn move_to_line_edge(&mut self, start: bool) {
        self.cursor_col = if start { 0 } else { self.current_line_char_len() };
        self.selection_anchor = None;
        self.apply_selection(None);
        self.base.request_redraw();
    }

    /// Scrolls the viewport by whole visible pages.
    ///
    /// A positive `direction` moves toward the end of the document. Returns whether the viewport
    /// moved.
    pub fn scroll_page(&mut self, direction: i32) -> bool {
        let page = self.visible_line_count().max(1) as i32;
        let moved = self.scroll_lines(direction.signum() * page);
        if moved {
            self.base.request_redraw();
        }
        moved
    }

    /// Stores a selection and reports the presence change.
    pub(crate) fn apply_selection(&mut self, next: Option<(usize, usize)>) {
        if self.selection == next {
            return;
        }
        self.selection = next;
        self.selection_changed.emit(next.is_some());
    }

    /// Selects the document characters spanned by the byte range `[start, end)`.
    pub(crate) fn select_byte_range(&mut self, start: usize, end: usize) {
        let start_char = self.text[..start.min(self.text.len())].chars().count();
        let end_char = start_char
            + self.text[start.min(self.text.len())..end.min(self.text.len())].chars().count();
        self.selection_anchor = Some(self.line_col_of(start_char));
        self.apply_selection(Some((start_char, end_char)));
        let (line, col) = self.line_col_of(end_char);
        self.cursor_line = line;
        self.cursor_col = col;
        self.reveal_caret();
        self.base.request_redraw();
    }

    /// Where the next forward/backward search starts.
    pub(crate) fn find_search_origin(&self, forward: bool) -> usize {
        let caret = self.cursor_byte_offset();
        match self.find.current {
            Some(current) if forward => current + 1,
            Some(current) => current,
            None if forward => caret,
            None => caret,
        }
    }

    /// Deletes the selected characters, if any, and reports whether anything was deleted.
    pub(crate) fn delete_selection(&mut self) -> bool {
        if self.selection.is_none() {
            return false;
        }
        self.replace_selection_or_caret("");
        true
    }

    /// Maps a document character offset to `(line, column)`.
    ///
    /// The lookup is a binary search over the cached line starts, so it costs O(log lines)
    /// rather than a walk from the top of the document on every caret move.
    pub(crate) fn line_col_of(&self, offset: usize) -> (usize, usize) {
        self.with_line_index(|index| {
            let line = index.line_of_char(offset);
            let start = index.char_starts.get(line).copied().unwrap_or(0);
            let length = index.char_lens.get(line).copied().unwrap_or(0);
            (line, offset.saturating_sub(start).min(length))
        })
    }

    /// Keeps the caret in view after a caret move or a search hit.
    pub(crate) fn reveal_caret(&mut self) {
        let visible = self.visible_line_count().max(1);
        if self.cursor_line < self.first_visible_line {
            self.first_visible_line = self.cursor_line;
        } else if self.cursor_line >= self.first_visible_line + visible {
            self.first_visible_line = self.cursor_line + 1 - visible;
        }
    }

    /// Reverts the most recent markdown text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise requests layout and redraw and clamps `cursor_line`.
    pub fn undo(&mut self) -> bool {
        // The undo commands splice into their shared scratch buffer, which is therefore synced
        // with the live document before the stack touches it. Syncing here (rather than after
        // every keystroke) keeps the edit path free of a whole-document copy.
        self.sync_history_target();
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the most recently undone markdown text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise requests layout and redraw.
    pub fn redo(&mut self) -> bool {
        self.sync_history_target();
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is a markdown text mutation to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone markdown text mutation to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Copies the live document into the buffer the undo commands splice into.
    ///
    /// The buffer is only read by [`UndoStack`] while an undo/redo runs, so it can lag the live
    /// text by any number of edits without becoming wrong.
    pub(crate) fn sync_history_target(&mut self) {
        let mut target = self.history_target.borrow_mut();
        if target.as_str() != self.text {
            target.clear();
            target.push_str(&self.text);
        }
    }

    pub(crate) fn restore_history_text(&mut self) {
        self.restoring_history = true;
        let before = core::mem::replace(&mut self.text, self.history_target.borrow().clone());
        self.restoring_history = false;
        self.revision = self.revision.wrapping_add(1);
        self.clamp_cursor();
        self.selection_anchor = None;
        self.apply_selection(None);
        self.emit_text_changed(&before);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Emits the changed run between `before` and the live document.
    ///
    /// The clone is only paid when a subscriber will receive it, and it is the changed run, not
    /// the document (SP-001).
    pub(crate) fn emit_text_changed(&mut self, before: &str) {
        if self.text_changed.slot_count() == 0 {
            return;
        }
        let (offset, removed, inserted) = changed_run(before, &self.text);
        self.text_changed.emit(MarkdownTextChange {
            offset,
            removed: removed.to_string(),
            inserted: inserted.to_string(),
            len: self.text.len(),
        });
    }

    /// Number of editable lines.
    ///
    /// This differs from [`Self::line_count`] only for a trailing newline: `str::lines`
    /// drops the final empty line, but an editor must be able to keep the caret on it,
    /// which is exactly where Enter at the end of the document leaves it.
    pub(crate) fn edit_line_count(&self) -> usize {
        self.with_line_index(|index| index.starts.len())
    }

    /// How many text rows fit below the header band.
    ///
    /// At least one, so a short editor still paints the line it is on instead of dividing to
    /// zero or scrolling everything away.
    pub(crate) fn visible_line_count(&self) -> usize {
        let usable = self.geometry().height as i32 - TEXT_TOP - TEXT_BOTTOM_INSET;
        if usable < 0 {
            return 1;
        }
        (usable / TEXT_ROW_HEIGHT + 1) as usize
    }

    /// The first line the viewport actually starts at, clamped to the last full window.
    pub(crate) fn visible_start(&self) -> usize {
        let max_first = self.line_count().saturating_sub(self.visible_line_count());
        self.first_visible_line.min(max_first)
    }

    /// Scrolls the viewport by `lines` rows toward the end of the document.
    ///
    /// Positive values reveal later lines, negative values reveal earlier ones. Returns whether
    /// the viewport moved, so the caller repaints only on a change.
    pub(crate) fn scroll_lines(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let max_first = self.line_count().saturating_sub(self.visible_line_count());
        let next =
            (self.first_visible_line as i64 + lines as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_line {
            return false;
        }
        self.first_visible_line = next;
        true
    }

    /// Byte span of the caret's line in `text`.
    ///
    /// Returns an empty span at the end of the document when the caret is out of range, so
    /// callers cannot index past the string even before [`Self::clamp_cursor`] runs.
    pub(crate) fn cursor_line_span(&self) -> (usize, usize) {
        self.with_line_index(|index| {
            index
                .line_span(self.cursor_line, self.text.len())
                .unwrap_or((self.text.len(), self.text.len()))
        })
    }

    /// Byte offset of the caret in `text`.
    pub(crate) fn cursor_byte_offset(&self) -> usize {
        let (start, end) = self.cursor_line_span();
        let line = &self.text[start..end];
        start
            + line
                .char_indices()
                .nth(self.cursor_col)
                .map(|(offset, _)| offset)
                .unwrap_or(line.len())
    }

    /// Character length of the caret's line.
    pub(crate) fn current_line_char_len(&self) -> usize {
        self.with_line_index(|index| index.char_lens.get(self.cursor_line).copied().unwrap_or(0))
    }

    /// Keeps `cursor_line` / `cursor_col` inside the document.
    pub(crate) fn clamp_cursor(&mut self) {
        self.cursor_line = self.cursor_line.min(self.edit_line_count().saturating_sub(1));
        self.clamp_cursor_col();
    }

    /// Keeps `cursor_col` inside the caret's line.
    pub(crate) fn clamp_cursor_col(&mut self) {
        self.cursor_col = self.cursor_col.min(self.current_line_char_len());
    }

    /// Applies a new document state through the undo snapshot route.
    ///
    /// Every text mutation funnels through here so a single place owns the history
    /// snapshot, the `restoring_history` guard, the caret clamp and the signal.
    pub(crate) fn commit_text(&mut self, next: String) {
        if self.text == next {
            return;
        }
        let before = core::mem::take(&mut self.text);
        self.text = next;
        self.revision = self.revision.wrapping_add(1);
        if !self.restoring_history {
            // The delta command records only the changed run. The shared buffer is *not* refreshed
            // here: it is synced once before an undo/redo, which is the only moment it is read.
            self.undo_stack.push(Box::new(MarkdownTextDeltaCommand::new(
                self.history_target.clone(),
                &before,
                &self.text,
            )));
        }
        self.clamp_cursor();
        self.emit_text_changed(&before);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Rebuilds the per-line block classification when the document changed.
    ///
    /// The preview needs to know which lines are inside a fenced code block before it can parse
    /// them, and that fact depends on every line above. Computing it once per revision keeps it
    /// out of the paint loop; the lexer state carried through the fence is what lets a block
    /// comment that spans lines continue to be highlighted.
    pub(crate) fn ensure_blocks(&mut self) {
        if self.blocks_revision == self.revision {
            return;
        }
        let mut blocks = Vec::new();
        let mut fence: Option<(LanguageId, LineState)> = None;
        // One lexer per fenced block, carried across its lines. A fresh highlighter per line reset
        // the interpreter state a multi-line construct depends on (block comments, template
        // literals) and re-created the lexer for every line of a long block.
        let mut highlighter: Option<(LanguageId, BuiltinHighlighter)> = None;
        for line in self.text.split('\n') {
            let trimmed = line.trim_start();
            match fence {
                Some((language, state)) => {
                    if fence_marker(trimmed).is_some() {
                        blocks.push(BlockStart::Fence);
                        fence = None;
                        highlighter = None;
                    } else {
                        blocks.push(BlockStart::Code(language, state));
                        if !highlighter.as_ref().is_some_and(|(active, _)| *active == language) {
                            highlighter = Some((language, BuiltinHighlighter::new(language)));
                        }
                        let next = match highlighter.as_ref() {
                            Some((_, lexer)) => lexer.highlight_line(line, state).1,
                            // Unreachable: the branch above always installs a lexer.
                            None => state,
                        };
                        fence = Some((language, next));
                    }
                }
                None => match fence_marker(trimmed) {
                    Some(info) => {
                        blocks.push(BlockStart::Fence);
                        let language = language_from_info(info);
                        highlighter = Some((language, BuiltinHighlighter::new(language)));
                        fence = Some((language, LineState::initial()));
                    }
                    None => blocks.push(BlockStart::Normal),
                },
            }
        }
        self.blocks = blocks;
        // GFM recognises a table by its *delimiter row*, so whether a line is a header depends on
        // the line after it. Resolving that once per revision keeps the paint path from looking
        // ahead per row.
        let lines: Vec<&str> = self.text.split('\n').collect();
        self.table_header = (0..lines.len())
            .map(|index| {
                lines.get(index + 1).is_some_and(|next| is_table_divider(next.trim_start()))
            })
            .collect();
        self.blocks_revision = self.revision;
    }

    /// Wraps the selection in `prefix`/`suffix`, or removes the pair when it is already there.
    ///
    /// This is the single formatting entry point: the keyboard chords below and the mounted
    /// [`MarkdownToolbar`] both call it, so a toolbar button and its shortcut cannot diverge. The
    /// write goes through `Self::commit_text`, so the wrap is one undo step.
    ///
    /// Returns whether the document changed.
    pub fn wrap_selection(&mut self, prefix: &str, suffix: &str) -> bool {
        if prefix.is_empty() && suffix.is_empty() {
            return false;
        }
        let caret = self.doc_char_offset_of_caret();
        let (start, end) = self.selection.unwrap_or((caret, caret));
        let start_byte = self.char_offset_to_byte(start);
        let end_byte = self.char_offset_to_byte(end);
        let wrapped =
            self.text[..start_byte].ends_with(prefix) && self.text[end_byte..].starts_with(suffix);
        let (next, caret_char, selection) = if wrapped {
            let mut next = String::with_capacity(self.text.len());
            next.push_str(&self.text[..start_byte - prefix.len()]);
            next.push_str(&self.text[start_byte..end_byte]);
            next.push_str(&self.text[end_byte + suffix.len()..]);
            let prefix_chars = prefix.chars().count();
            let selection =
                self.selection.map(|(first, last)| (first - prefix_chars, last - prefix_chars));
            (next, start - prefix_chars, selection)
        } else {
            let mut next = String::with_capacity(self.text.len() + prefix.len() + suffix.len());
            next.push_str(&self.text[..start_byte]);
            next.push_str(prefix);
            next.push_str(&self.text[start_byte..end_byte]);
            next.push_str(suffix);
            next.push_str(&self.text[end_byte..]);
            let prefix_chars = prefix.chars().count();
            let suffix_chars = suffix.chars().count();
            let selection =
                self.selection.map(|(first, last)| (first + prefix_chars, last + prefix_chars));
            // With no selection the caret lands between the two markers, which is where typing
            // the emphasised text begins.
            (next, end + prefix_chars + suffix_chars, selection)
        };
        self.commit_text(next);
        let (line, column) = self.line_col_of(caret_char);
        self.cursor_line = line;
        self.cursor_col = column;
        self.apply_selection(selection);
        self.base.request_redraw();
        true
    }

    /// Adds `prefix` to every selected line, or removes it when every selected line has it.
    ///
    /// A caret with no selection applies to its own line. The document is rewritten once and the
    /// change is one undo step, so a multi-line list toggle can be reverted in one action.
    pub fn toggle_line_prefix(&mut self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }
        let lines = self.selected_line_range();
        let all_prefixed = lines.iter().all(|line| {
            let text = self.line_slice(*line);
            text.is_empty() || text.starts_with(prefix)
        });
        self.apply_line_prefix(prefix, all_prefixed)
    }

    /// Adds one indent level to every selected line.
    pub(crate) fn indent_lines(&mut self) {
        const INDENT: &str = "  ";
        let _ = self.apply_line_prefix(INDENT, false);
    }

    /// Adds `prefix` to (or removes it from) every selected line, as one undo step.
    pub(crate) fn apply_line_prefix(&mut self, prefix: &str, remove: bool) -> bool {
        let had_selection = self.selection.is_some();
        let lines = self.selected_line_range();
        if lines.is_empty() {
            return false;
        }
        let starts = self.line_starts();
        let mut next = String::with_capacity(self.text.len() + lines.len() * prefix.len());
        let mut cursor = 0usize;
        for line in &lines {
            let Some(&start_byte) = starts.get(*line) else {
                continue;
            };
            let end_byte = match starts.get(line + 1) {
                Some(next) => next.saturating_sub(1),
                None => self.text.len(),
            };
            next.push_str(&self.text[cursor..start_byte]);
            let text = &self.text[start_byte..end_byte];
            if remove {
                next.push_str(text.strip_prefix(prefix).unwrap_or(text));
            } else {
                next.push_str(prefix);
                next.push_str(text);
            }
            cursor = end_byte;
        }
        next.push_str(&self.text[cursor..]);
        self.commit_text(next);
        // The selection follows the edited lines rather than being dropped: Tab then Shift+Tab is
        // one gesture on one block, and a reader would not expect the second key to apply to a
        // single line.
        let first = lines.first().copied().unwrap_or(0);
        let last = lines.last().copied().unwrap_or(first).min(self.edit_line_count() - 1);
        self.cursor_line = last;
        self.cursor_col = self.current_line_char_len();
        let start_char = self.doc_char_offset(first, 0);
        let end_char = self.doc_char_offset(last, self.current_line_char_len());
        self.selection_anchor = None;
        self.apply_selection(if had_selection {
            (start_char != end_char).then_some((start_char, end_char))
        } else {
            // A caret with no selection keeps a caret: indenting the line it is on must not
            // silently select the line.
            None
        });
        self.base.request_redraw();
        true
    }

    /// Every line the current selection touches, or the caret's line when nothing is selected.
    pub(crate) fn selected_line_range(&self) -> Vec<usize> {
        let (first, last) = match self.selection {
            Some((start, end)) => {
                let (first, _) = self.line_col_of(start);
                let (last, _) = self.line_col_of(end);
                (first, last)
            }
            None => (self.cursor_line, self.cursor_line),
        };
        let last = last.min(self.edit_line_count().saturating_sub(1));
        if first > last {
            return Vec::new();
        }
        (first..=last).collect()
    }

    /// The content of `line`, without its `\n`.
    pub(crate) fn line_slice(&self, line: usize) -> &str {
        self.with_line_index(|index| {
            index.line_span(line, self.text.len()).map_or("", |(start, end)| &self.text[start..end])
        })
    }

    /// The `(column, line length)` a pointer at `x` (relative to the text column) hits in `line`.
    ///
    /// The line comes from the cached offset tables rather than from a fresh scan for its `nth`
    /// `\n`, which was one pass over every line above the hit row per press and per drag move.
    /// The width cache is borrowed as its own field so it can be mutated beside the line borrow.
    pub(crate) fn hit_column(&mut self, line: usize, x: i32) -> (usize, usize) {
        let font = effective_font(self.base.style());
        self.ensure_line_index();
        let index = self.line_index.borrow();
        let Some((_, table)) = index.as_ref() else {
            return (0, 0);
        };
        let Some((start, end)) = table.line_span(line, self.text.len()) else {
            return (0, 0);
        };
        let mut offset = 0i32;
        let mut column = 0usize;
        for ch in self.text[start..end].chars() {
            let width = cached_char_width(&mut self.char_widths, ch, font, 1.0);
            if x < offset + width / 2 {
                break;
            }
            offset += width;
            column += 1;
        }
        (column, table.char_lens.get(line).copied().unwrap_or(column))
    }

    /// Indents (`indent == true`) or outdents the selected lines by one level.
    pub(crate) fn shift_line_indent(&mut self, indent: bool) {
        if indent {
            self.indent_lines();
            return;
        }
        let had_selection = self.selection.is_some();
        const INDENT: &str = "  ";
        // Outdent removes one indent step, or a single tab, from each selected line.
        let lines = self.selected_line_range();
        let starts = self.line_starts();
        let mut removals: Vec<(usize, usize)> = Vec::new();
        for line in &lines {
            let Some(&start_byte) = starts.get(*line) else {
                continue;
            };
            let end_byte = match starts.get(line + 1) {
                Some(next) => next.saturating_sub(1),
                None => self.text.len(),
            };
            let text = &self.text[start_byte..end_byte];
            if let Some(rest) = text.strip_prefix(INDENT) {
                removals.push((start_byte, start_byte + (text.len() - rest.len())));
            } else if text.starts_with('\t') {
                removals.push((start_byte, start_byte + 1));
            }
        }
        if removals.is_empty() {
            return;
        }
        let mut next = String::with_capacity(self.text.len());
        let mut cursor = 0usize;
        for (start_byte, end_byte) in removals {
            next.push_str(&self.text[cursor..start_byte]);
            cursor = end_byte;
        }
        next.push_str(&self.text[cursor..]);
        self.commit_text(next);
        let first = lines.first().copied().unwrap_or(0);
        let last = lines.last().copied().unwrap_or(first).min(self.edit_line_count() - 1);
        self.cursor_line = last;
        self.clamp_cursor_col();
        let start_char = self.doc_char_offset(first, 0);
        let end_char = self.doc_char_offset(last, self.current_line_char_len());
        self.selection_anchor = None;
        self.apply_selection(if had_selection {
            (start_char != end_char).then_some((start_char, end_char))
        } else {
            None
        });
        self.base.request_redraw();
    }

    /// Byte offsets of every line's first character, from the cached tables.
    pub(crate) fn line_starts(&self) -> Vec<usize> {
        self.with_line_index(|index| index.starts.clone())
    }

    /// Deletes the character before the caret, merging lines at a line start.
    pub(crate) fn backspace_at_cursor(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.clamp_cursor();
        let at = self.cursor_byte_offset();
        if self.cursor_col > 0 {
            let (start, _) = self.cursor_line_span();
            let removed = self.text[start..at].chars().next_back().map(char::len_utf8).unwrap_or(0);
            let mut next = String::with_capacity(self.text.len());
            next.push_str(&self.text[..at - removed]);
            next.push_str(&self.text[at..]);
            self.cursor_col -= 1;
            self.commit_text(next);
        } else if self.cursor_line > 0 {
            let (start, _) = self.cursor_line_span();
            let merged_col =
                self.text[..start - 1].chars().rev().take_while(|&ch| ch != '\n').count();
            let mut next = String::with_capacity(self.text.len());
            next.push_str(&self.text[..start - 1]);
            next.push_str(&self.text[start..]);
            self.cursor_line -= 1;
            self.cursor_col = merged_col;
            self.commit_text(next);
        }
    }

    /// Splits the caret's line at the caret.
    pub(crate) fn insert_newline_at_cursor(&mut self) {
        self.delete_selection();
        self.clamp_cursor();
        let at = self.cursor_byte_offset();
        let mut next = String::with_capacity(self.text.len() + 1);
        next.push_str(&self.text[..at]);
        next.push('\n');
        next.push_str(&self.text[at..]);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.commit_text(next);
    }

    /// Moves the caret one character left, wrapping to the previous line's end.
    pub(crate) fn move_cursor_left(&mut self) {
        self.clamp_cursor();
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.base.request_redraw();
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.current_line_char_len();
            self.base.request_redraw();
        }
    }

    /// Moves the caret one character right, wrapping to the next line's start.
    pub(crate) fn move_cursor_right(&mut self) {
        self.clamp_cursor();
        if self.cursor_col < self.current_line_char_len() {
            self.cursor_col += 1;
            self.base.request_redraw();
        } else if self.cursor_line + 1 < self.edit_line_count() {
            self.cursor_line += 1;
            self.cursor_col = 0;
            self.base.request_redraw();
        }
    }

    pub(crate) fn move_cursor(&mut self, delta: isize) {
        let lines = self.edit_line_count();
        let current = self.cursor_line as isize;
        let next = (current + delta).clamp(0, lines.saturating_sub(1) as isize) as usize;
        if next != self.cursor_line {
            self.cursor_line = next;
            // A column past the new line's end would put the caret outside the text, so
            // vertical movement keeps the column but clamps it to the line it landed on.
            self.clamp_cursor_col();
            self.base.request_redraw();
        }
    }
}

/// A glyph advance, measured at most once per font size.
///
/// Free rather than a method because the callers hold a borrow of the widget's style (and thus of
/// `self.base`) while they need the cache mutably; taking the cache as its own parameter keeps
/// those two borrows disjoint.
fn cached_char_width(cache: &mut Vec<(char, f32, i32)>, ch: char, font: &Font, scale: f32) -> i32 {
    let size = font.size();
    if let Some((_, _, width)) = cache
        .iter()
        .find(|(candidate, cached, _)| *candidate == ch && (*cached - size).abs() < f32::EPSILON)
    {
        return *width;
    }
    let mut buffer = [0u8; 4];
    let glyph = ch.encode_utf8(&mut buffer);
    let width = estimate_text_width(glyph, font, scale) as i32;
    cache.push((ch, size, width));
    width
}
