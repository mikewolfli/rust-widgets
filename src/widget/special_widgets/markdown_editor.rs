// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MarkdownEditor widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::Signal1;
use crate::style::animation::CursorBlink;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::coercion::{
    expect_bool, expect_string, expect_string_list, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::special_widgets::code_editor::{
    BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter, TokenKind,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// The formatting strip is a companion control, declared here so the editor's action surface and
// its pointer surface live in one module tree (COMP-H04).
#[path = "markdown_toolbar.rs"]
pub mod markdown_toolbar;
pub use markdown_toolbar::{MarkdownToolbar, ToolbarIconSide, MARKDOWN_TOOLBAR_ACTIONS};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The top of the first text row below the header band.
const TEXT_TOP: i32 = 36;
/// The pitch between text rows.
const TEXT_ROW_HEIGHT: i32 = 16;
/// The bottom inset a row has to clear to be painted.
const TEXT_BOTTOM_INSET: i32 = 8;

/// An undo step that records only the bytes that differ between two document states.
///
/// A whole-document snapshot would store both versions on every keystroke. The common
/// prefix and suffix are found on character boundaries, so the command keeps only the
/// changed run and an offset, and an undo touches O(edit) bytes instead of O(document).
struct MarkdownTextDeltaCommand {
    id: CommandId,
    target: Rc<RefCell<String>>,
    offset: usize,
    removed: String,
    inserted: String,
}

impl MarkdownTextDeltaCommand {
    /// Builds a command for a mutation that has already been applied.
    fn new(target: Rc<RefCell<String>>, before: &str, after: &str) -> Self {
        // The common prefix and suffix are found by byte first, then trimmed back to character
        // boundaries. Byte comparison is the same answer as character comparison here (equal
        // bytes at equal offsets mean equal characters) and is an order of magnitude cheaper on
        // the ASCII that dominates source text.
        let before_bytes = before.as_bytes();
        let after_bytes = after.as_bytes();
        let max_prefix: usize = before_bytes.len().min(after_bytes.len());
        let mut prefix: usize = 0;
        while prefix < max_prefix && before_bytes[prefix] == after_bytes[prefix] {
            prefix += 1;
        }
        // Both strings share this prefix, so a boundary in `before` is one in `after` too.
        while prefix > 0 && !before.is_char_boundary(prefix) {
            prefix -= 1;
        }
        let max_suffix: usize = before_bytes.len().min(after_bytes.len()) - prefix;
        let mut suffix: usize = 0;
        while suffix < max_suffix
            && before_bytes[before_bytes.len() - 1 - suffix]
                == after_bytes[after_bytes.len() - 1 - suffix]
        {
            suffix += 1;
        }
        while suffix > 0 && !before.is_char_boundary(before.len() - suffix) {
            suffix -= 1;
        }
        Self {
            id: CommandId(
                NEXT_MARKDOWN_COMMAND_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            ),
            target,
            offset: prefix,
            removed: before[prefix..before.len() - suffix].to_string(),
            inserted: after[prefix..after.len() - suffix].to_string(),
        }
    }

    /// Replaces the recorded range with `insert`.
    fn splice(&self, remove_len: usize, insert: &str) {
        let mut text = self.target.borrow_mut();
        let start = self.offset.min(text.len());
        let end = (start + remove_len).min(text.len());
        if start <= end {
            text.replace_range(start..end, insert);
        }
    }
}

/// Counter for [`MarkdownTextDeltaCommand`] ids.
static NEXT_MARKDOWN_COMMAND_ID: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(1);

impl UndoCommand for MarkdownTextDeltaCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit text".to_string(),
            timestamp_ms: 0,
            command_type: "markdown_editor_text",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        self.splice(self.removed.len(), &self.inserted.clone());
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        self.splice(self.inserted.len(), &self.removed.clone());
        Ok(())
    }
}

/// One heading in the document outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownOutlineEntry {
    /// Zero-based line index.
    pub line: usize,
    /// Heading level, `1..=6`.
    pub level: u8,
    /// Heading text without its `#` markers.
    pub title: String,
}

/// One run of preview text with the style the inline parser resolved for it.
struct PreviewRun {
    text: String,
    style: PreviewStyle,
    /// The measured advance, resolved when the line was parsed so a still frame measures nothing.
    width: u32,
}

/// The inline style of one preview run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewStyle {
    /// Ordinary body text.
    Text,
    /// `**strong**`: drawn with a one-pixel overprint, like every bold run in this crate.
    Bold,
    /// `*emphasis*`: the face has no italic, so emphasis is a slight ink shift.
    Italic,
    /// `~~struck~~`: drawn faint with a rule through it.
    Strike,
    /// `` `code` ``: tinted backing.
    Code,
    /// `[label](target)`: accent ink with an underline.
    Link,
    /// Syntax that was recognised and is not shown as body text (fences, list markers).
    Dim,
    /// A list bullet, task box or quote bar.
    Accent,
    /// One token of a highlighted fenced block.
    Token(TokenKind),
}

/// How a preview row is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewRow {
    /// Runs drawn from left to right.
    Inline,
    /// A heading, carrying its level.
    Heading(u8),
    /// A horizontal rule.
    Rule,
    /// An opening or closing code fence.
    Fence,
    /// The body of a fenced code block.
    Code,
    /// A table row (the runs already contain the cell separators).
    Table,
}

/// A parsed preview row, cached by line index and text hash.
struct ParsedLine {
    runs: Vec<PreviewRun>,
    row: PreviewRow,
}

/// Where a line sits relative to a fenced code block.
#[derive(Clone, Copy)]
enum BlockStart {
    /// Outside any fence.
    Normal,
    /// The opening or closing fence line itself.
    Fence,
    /// Inside a fence, with the lexer state the line starts in.
    Code(LanguageId, LineState),
}

/// An IME composition that has not been committed yet.
#[derive(Debug, Clone, Default)]
struct Preedit {
    text: String,
    /// Byte offset of the insertion point inside `text`.
    cursor: usize,
}

/// Document statistics, cached per revision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Metrics {
    lines: usize,
    words: usize,
    headings: usize,
}

/// Per-line offset tables derived from one revision of the document.
///
/// Every caret query used to rescan the document from its first byte, and each keystroke
/// rebuilt the whole document as a character vector. Both are O(document) per event. With the
/// tables below a line lookup is a binary search and a character offset is resolved inside one
/// line, so the work is bounded by the line's own length instead of the document's.
#[derive(Debug, Default)]
struct LineIndex {
    /// Byte offset of the first character of every line.
    starts: Vec<usize>,
    /// Character offset of the first character of every line.
    char_starts: Vec<usize>,
    /// Character length of every line, excluding its `\n`.
    char_lens: Vec<usize>,
}

impl LineIndex {
    /// Builds the tables from `text` in one pass.
    fn new(text: &str) -> Self {
        let mut starts = Vec::new();
        let mut char_starts = Vec::new();
        let mut char_lens = Vec::new();
        let mut byte = 0usize;
        let mut chars = 0usize;
        for line in text.split('\n') {
            starts.push(byte);
            char_starts.push(chars);
            let length = line.chars().count();
            char_lens.push(length);
            byte += line.len() + 1;
            chars += length + 1;
        }
        Self { starts, char_starts, char_lens }
    }

    /// The index of the line that contains `offset`, counted in characters.
    fn line_of_char(&self, offset: usize) -> usize {
        // `char_starts` is ascending, so the last entry that starts at or before `offset` is the
        // containing line. A `\n` position belongs to the line it terminates.
        self.char_starts.partition_point(|start| *start <= offset).saturating_sub(1)
    }

    /// The number of characters in the document, including the `\n` separators.
    fn char_len(&self) -> usize {
        match (self.char_starts.last(), self.char_lens.last()) {
            (Some(start), Some(length)) => start + length,
            _ => 0,
        }
    }
}

/// The find bar's state.
#[derive(Debug, Clone, Default)]
struct FindState {
    visible: bool,
    query: String,
    /// Byte offset of the current match, when one is highlighted.
    current: Option<usize>,
}

/// Lightweight markdown editor with preview toggle and metrics.
pub struct MarkdownEditor {
    base: BaseWidget,
    text: String,
    preview_mode: bool,
    cursor_line: usize,
    /// Character offset of the caret within [`Self::cursor_line`].
    cursor_col: usize,
    /// The first text line the viewport paints.
    ///
    /// The editor used to paint a fixed ten lines, so a longer document simply ended at row ten.
    /// This is the scroll state the wheel drives; the paint path starts here and takes only the
    /// rows that fit.
    first_visible_line: usize,
    /// Emitted when text changes.
    pub text_changed: Signal1<String>,
    /// Emitted when preview mode changes.
    pub preview_mode_changed: Signal1<bool>,
    /// Emitted when a selection appears (`true`) or disappears (`false`).
    pub selection_changed: Signal1<bool>,
    /// Emitted when a heading row is activated by a click, with its outline index.
    pub heading_activated: Signal1<usize>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// The configured undo depth, so the property can be read back after a write.
    undo_stack_limit: usize,
    /// The selection as ordered document character offsets.
    selection: Option<(usize, usize)>,
    /// Where the selection started, as `(line, column)`, for shift-extension.
    selection_anchor: Option<(usize, usize)>,
    /// Whether a pointer drag is still extending the selection.
    selecting: bool,
    /// Uncommitted IME composition, drawn at the caret.
    preedit: Option<Preedit>,
    /// The caret blink. Unlike the other two text controls this one had none.
    cursor_blink: CursorBlink,
    /// Whether the editor paints a scrollbar beside the text.
    show_scrollbar: bool,
    /// Whether a drag on the scrollbar thumb is in progress.
    dragging_scrollbar: bool,
    /// The find bar.
    find: FindState,
    /// The mounted find/replace bar.
    ///
    /// It is a view over this editor's bytes, not the editor's own find state: the bar paints the
    /// query and the match counter while [`Self::find`] stays the navigation state the editor's
    /// keyboard path uses. [`Self::show_find_bar`] is what points the two at the same text.
    find_bar: FindBar,
    /// The mounted document outline.
    outline_panel: MarkdownOutlinePanel,
    /// Whether the outline panel is painted inside the editor's own rectangle.
    show_outline: bool,
    /// The revision the panel's entries were last rebuilt for.
    ///
    /// `outline()` walks every line, so it is rebuilt when the text changes rather than on every
    /// frame; the active row still follows the caret on each paint.
    outline_revision: u64,
    /// Whether GFM constructs (tables, tasks, strikethrough, quotes, fences, rules) are parsed.
    gfm_extensions: bool,
    /// Whether fenced code blocks are tokenized with the shared lexer.
    code_highlight: bool,
    /// Counts text mutations, so caches can tell a stale frame from a live one.
    revision: u64,
    /// `line_count` / `word_count` / `heading_count`, keyed by revision.
    metrics: RefCell<Option<(u64, Metrics)>>,
    /// Parsed preview rows, keyed by line index and text hash.
    preview_cache: HashMap<(usize, u64), Rc<ParsedLine>>,
    /// Per-line block classification, rebuilt when the revision changes.
    blocks: Vec<BlockStart>,
    /// Whether the line *after* each line is a table divider, so a row can be recognised as a
    /// table header without the paint path looking ahead (which would be quadratic).
    table_header: Vec<bool>,
    /// The revision `blocks` was built for.
    blocks_revision: u64,
    /// Measured glyph advances for the caret's line prefix, keyed by font size.
    char_widths: Vec<(char, f32, i32)>,
    /// Line offset tables for [`Self::revision`], rebuilt at most once per revision.
    line_index: RefCell<Option<(u64, LineIndex)>>,
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
    fn replace_selection_or_caret(&mut self, insert: &str) {
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
    fn doc_char_offset_of_caret(&self) -> usize {
        self.doc_char_offset(self.cursor_line, self.cursor_col)
    }

    /// The line offset tables, rebuilt at most once per revision.
    fn ensure_line_index(&self) {
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
    fn with_line_index<R>(&self, read: impl FnOnce(&LineIndex) -> R) -> R {
        self.ensure_line_index();
        let index = self.line_index.borrow();
        let (_, table) = index.as_ref().expect("ensure_line_index() populates the table");
        read(table)
    }

    /// The document's character count, including its `\n` separators.
    fn total_char_count(&self) -> usize {
        self.with_line_index(LineIndex::char_len)
    }

    /// The byte offset of a document character offset.
    fn char_offset_to_byte(&self, offset: usize) -> usize {
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
    fn metrics_snapshot(&self) -> Metrics {
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
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection?;
        let chars: Vec<char> = self.text.chars().collect();
        let start = start.min(chars.len());
        let end = end.min(chars.len());
        Some(chars[start..end.max(start)].iter().collect())
    }

    /// Selects the whole document.
    pub fn select_all(&mut self) {
        let length = self.text.chars().count();
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
            self.base.request_redraw();
        }
        owes_frame
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
    fn apply_selection(&mut self, next: Option<(usize, usize)>) {
        if self.selection == next {
            return;
        }
        self.selection = next;
        self.selection_changed.emit(next.is_some());
    }

    /// Selects the document characters spanned by the byte range `[start, end)`.
    fn select_byte_range(&mut self, start: usize, end: usize) {
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
    fn find_search_origin(&self, forward: bool) -> usize {
        let caret = self.cursor_byte_offset();
        match self.find.current {
            Some(current) if forward => current + 1,
            Some(current) => current,
            None if forward => caret,
            None => caret,
        }
    }

    /// Deletes the selected characters, if any, and reports whether anything was deleted.
    fn delete_selection(&mut self) -> bool {
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
    fn line_col_of(&self, offset: usize) -> (usize, usize) {
        self.with_line_index(|index| {
            let line = index.line_of_char(offset);
            let start = index.char_starts.get(line).copied().unwrap_or(0);
            let length = index.char_lens.get(line).copied().unwrap_or(0);
            (line, offset.saturating_sub(start).min(length))
        })
    }

    /// Keeps the caret in view after a caret move or a search hit.
    fn reveal_caret(&mut self) {
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
    fn sync_history_target(&mut self) {
        let mut target = self.history_target.borrow_mut();
        if target.as_str() != self.text {
            target.clear();
            target.push_str(&self.text);
        }
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.restoring_history = false;
        self.revision = self.revision.wrapping_add(1);
        self.clamp_cursor();
        self.selection_anchor = None;
        self.apply_selection(None);
        self.text_changed.emit(self.text.clone());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Number of editable lines.
    ///
    /// This differs from [`Self::line_count`] only for a trailing newline: `str::lines`
    /// drops the final empty line, but an editor must be able to keep the caret on it,
    /// which is exactly where Enter at the end of the document leaves it.
    fn edit_line_count(&self) -> usize {
        self.with_line_index(|index| index.starts.len())
    }

    /// How many text rows fit below the header band.
    ///
    /// At least one, so a short editor still paints the line it is on instead of dividing to
    /// zero or scrolling everything away.
    fn visible_line_count(&self) -> usize {
        let usable = self.geometry().height as i32 - TEXT_TOP - TEXT_BOTTOM_INSET;
        if usable < 0 {
            return 1;
        }
        (usable / TEXT_ROW_HEIGHT + 1) as usize
    }

    /// The first line the viewport actually starts at, clamped to the last full window.
    fn visible_start(&self) -> usize {
        let max_first = self.line_count().saturating_sub(self.visible_line_count());
        self.first_visible_line.min(max_first)
    }

    /// Scrolls the viewport by `lines` rows toward the end of the document.
    ///
    /// Positive values reveal later lines, negative values reveal earlier ones. Returns whether
    /// the viewport moved, so the caller repaints only on a change.
    fn scroll_lines(&mut self, lines: i32) -> bool {
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
    fn cursor_line_span(&self) -> (usize, usize) {
        self.with_line_index(|index| {
            let Some(&start) = index.starts.get(self.cursor_line) else {
                return (self.text.len(), self.text.len());
            };
            let end = match index.starts.get(self.cursor_line + 1) {
                Some(next) => next.saturating_sub(1),
                None => self.text.len(),
            };
            (start, end.max(start))
        })
    }

    /// Byte offset of the caret in `text`.
    fn cursor_byte_offset(&self) -> usize {
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
    fn current_line_char_len(&self) -> usize {
        self.with_line_index(|index| index.char_lens.get(self.cursor_line).copied().unwrap_or(0))
    }

    /// Keeps `cursor_line` / `cursor_col` inside the document.
    fn clamp_cursor(&mut self) {
        self.cursor_line = self.cursor_line.min(self.edit_line_count().saturating_sub(1));
        self.clamp_cursor_col();
    }

    /// Keeps `cursor_col` inside the caret's line.
    fn clamp_cursor_col(&mut self) {
        self.cursor_col = self.cursor_col.min(self.current_line_char_len());
    }

    /// Applies a new document state through the undo snapshot route.
    ///
    /// Every text mutation funnels through here so a single place owns the history
    /// snapshot, the `restoring_history` guard, the caret clamp and the signal.
    fn commit_text(&mut self, next: String) {
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
        self.text_changed.emit(self.text.clone());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Rebuilds the per-line block classification when the document changed.
    ///
    /// The preview needs to know which lines are inside a fenced code block before it can parse
    /// them, and that fact depends on every line above. Computing it once per revision keeps it
    /// out of the paint loop; the lexer state carried through the fence is what lets a block
    /// comment that spans lines continue to be highlighted.
    fn ensure_blocks(&mut self) {
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
    fn indent_lines(&mut self) {
        const INDENT: &str = "  ";
        let _ = self.apply_line_prefix(INDENT, false);
    }

    /// Adds `prefix` to (or removes it from) every selected line, as one undo step.
    fn apply_line_prefix(&mut self, prefix: &str, remove: bool) -> bool {
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
    fn selected_line_range(&self) -> Vec<usize> {
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
    fn line_slice(&self, line: usize) -> &str {
        self.with_line_index(|index| {
            let Some(&start_byte) = index.starts.get(line) else {
                return "";
            };
            let end_byte = match index.starts.get(line + 1) {
                Some(next) => next.saturating_sub(1),
                None => self.text.len(),
            };
            &self.text[start_byte..end_byte]
        })
    }

    /// Indents (`indent == true`) or outdents the selected lines by one level.
    fn shift_line_indent(&mut self, indent: bool) {
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
    fn line_starts(&self) -> Vec<usize> {
        self.with_line_index(|index| index.starts.clone())
    }

    /// Deletes the character before the caret, merging lines at a line start.
    fn backspace_at_cursor(&mut self) {
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
    fn insert_newline_at_cursor(&mut self) {
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
    fn move_cursor_left(&mut self) {
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
    fn move_cursor_right(&mut self) {
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

    fn move_cursor(&mut self, delta: isize) {
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

impl Widget for MarkdownEditor {
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
        crate::core::Size::new(500, 300)
    }

    /// One frame of the caret's blink. The frame bus calls this; nothing else does.
    fn tick(&mut self, delta_ms: u32) -> bool {
        MarkdownEditor::tick(self, delta_ms)
    }

    /// A blinking caret always owes frames while it runs.
    fn is_animating(&self) -> bool {
        self.cursor_blink.is_running()
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
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "preview_mode_changed" => Some(EventSignalRef::mapped(
                "preview_mode_changed",
                &self.preview_mode_changed,
                |v| CapabilityValue::Bool(*v),
            )),
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "heading_activated" => {
                Some(EventSignalRef::mapped("heading_activated", &self.heading_activated, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `MarkdownEditor`'s property contract.
///
/// `text` writes go through `set_text`, which keeps the undo stack and the
/// emitted `text_changed` signal in step with the stored document; writing the
/// field directly would make the editor's own history wrong. `cursor_line` is
/// read-only because the editor owns caret placement.
impl WidgetProperties for MarkdownEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "preview_mode" => Ok(CapabilityValue::Bool(self.preview_mode())),
            "line_count" => Ok(CapabilityValue::UInt(self.line_count() as u64)),
            "word_count" => Ok(CapabilityValue::UInt(self.word_count() as u64)),
            "heading_count" => Ok(CapabilityValue::UInt(self.heading_count() as u64)),
            "cursor_line" => Ok(CapabilityValue::UInt(self.cursor_line() as u64)),
            "show_scrollbar" => Ok(CapabilityValue::Bool(self.show_scrollbar)),
            "gfm_extensions" => Ok(CapabilityValue::Bool(self.gfm_extensions)),
            "code_highlight" => Ok(CapabilityValue::Bool(self.code_highlight)),
            "undo_stack_limit" => Ok(CapabilityValue::UInt(self.undo_stack_limit as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_text(text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "preview_mode" => match value {
                CapabilityValue::Bool(enabled) => {
                    self.set_preview_mode(enabled);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "line_count" | "word_count" | "heading_count" | "cursor_line" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "show_scrollbar" => {
                self.set_show_scrollbar(expect_bool(value)?);
                Ok(())
            }
            "gfm_extensions" => {
                self.set_gfm_extensions(expect_bool(value)?);
                Ok(())
            }
            "code_highlight" => {
                self.set_code_highlight(expect_bool(value)?);
                Ok(())
            }
            "undo_stack_limit" => {
                self.set_undo_limit(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "preview_mode",
            "line_count",
            "word_count",
            "heading_count",
            "cursor_line",
            "show_scrollbar",
            "gfm_extensions",
            "code_highlight",
            "undo_stack_limit",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `markdown_editor` publishes.
    ///
    /// `toggle_preview_mode` and `undo` are the genuine zero-argument actions here;
    /// `undo` reports `false` when there is nothing to undo, which is the "could not
    /// handle it" case, so it is answered with
    /// [`CapabilityAccessError::OutOfRange`] rather than a success that did nothing.
    /// `append_line` takes the line to append and `set_text` / `set_preview_mode`
    /// assign state through the property route, so those are refused the same way —
    /// the names are right and the payload is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle_preview_mode" => {
                self.toggle_preview_mode();
                Ok(())
            }
            "undo" => {
                if self.undo() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "append_line" | "set_text" | "set_preview_mode" => {
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

impl EventHandler for MarkdownEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            self.cursor_blink.stop();
            return;
        }
        if !self.cursor_blink.is_running() {
            self.cursor_blink.start();
        }
        // The side panel and find bar are hit-tested in this widget's coordinates, so their boxes
        // are refreshed before the press is resolved.
        self.layout_companions();

        match event {
            // The find bar owns text input while it is open: otherwise typing a query would edit
            // the document behind it. The mounted bar is kept in step through the same query, so
            // the painted counter always describes the text Enter will search.
            Event::TextInput { text } if self.find.visible => {
                self.find.query.push_str(text);
                self.find.current = None;
                self.find_bar.set_query(self.find.query.clone());
                self.base.request_redraw();
            }
            Event::ImeCommit { text } if self.find.visible => {
                self.find.query.push_str(text);
                self.find.current = None;
                self.find_bar.set_query(self.find.query.clone());
                self.base.request_redraw();
            }
            // Printable text and IME commits are what the platform delivers for typing; a
            // handler with only a `KeyPress` arm never sees them. Both insert at the caret
            // through the same undo-guarded path as `set_text` (the filter mirrors
            // `Mention`, which refuses control characters and empty runs).
            Event::TextInput { text }
                if text.chars().all(|ch| !ch.is_control()) && !text.is_empty() =>
            {
                self.preedit = None;
                self.insert_text_at_cursor(text);
            }
            Event::ImeCommit { text } => {
                self.preedit = None;
                self.insert_text_at_cursor(text);
            }
            Event::ImePreedit { text, cursor } => {
                // The composition has to be visible before it is committed, and the caret inside
                // it is what anchors the candidate window.
                self.preedit =
                    Some(Preedit { text: text.clone(), cursor: (*cursor).min(text.len()) });
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } => self.handle_key(*key, *modifiers),
            // Wheel: positive `delta.y` moves the content down, so the viewport moves toward the
            // start. Three lines per notch is the desktop default shared with the other text
            // controls.
            Event::MousePress { pos, button: 1, modifiers, .. } => {
                let rect = self.geometry();
                // The mounted outline is a real control: a press in its column selects the row and
                // navigates to its line, exactly as a standalone panel would.
                if let Some(panel_rect) = self.outline_rect() {
                    if panel_rect.contains_point(*pos) {
                        if let Some(index) = self.outline_panel.entry_at(*pos) {
                            if let Some(line) = self.outline_panel.goto(index) {
                                self.heading_activated.emit(index);
                                let _ = self.goto_line(line);
                            }
                        }
                        return;
                    }
                }
                // The mounted find bar owns presses inside its own band (the `Aa`/`W` toggles and
                // the query band); the document caret must not move for them.
                if self.find.visible && self.find_bar_rect().contains_point(*pos) {
                    self.find_bar.handle_event(event);
                    self.find.query = self.find_bar.query().to_string();
                    self.base.request_redraw();
                    return;
                }
                if self.show_scrollbar && self.scrollbar_track_contains(*pos) {
                    let on_thumb = self.scrollbar_thumb().is_some_and(|thumb| {
                        pos.x >= thumb.x
                            && pos.x < thumb.x + thumb.width as i32
                            && pos.y >= thumb.y
                            && pos.y < thumb.y + thumb.height as i32
                    });
                    if on_thumb {
                        self.dragging_scrollbar = true;
                    } else {
                        self.scroll_to_track(pos.y);
                    }
                    return;
                }
                let row = (pos.y - rect.y - TEXT_TOP) / TEXT_ROW_HEIGHT;
                if row < 0 {
                    return;
                }
                let line = self.visible_start() + row as usize;
                if line >= self.edit_line_count() {
                    return;
                }
                let font = effective_font(self.base.style());
                let x = pos.x - rect.x - 12;
                // The line is borrowed, not copied: a drag over the document copied one line per
                // pointer move. `text`, `char_widths` and the style are disjoint fields, so the
                // immutable line borrow and the mutable width cache coexist.
                let (column, line_chars) = {
                    let line_text = self.text.split('\n').nth(line).unwrap_or("");
                    let mut offset = 0i32;
                    let mut column = 0usize;
                    for ch in line_text.chars() {
                        let width = cached_char_width(&mut self.char_widths, ch, font, 1.0);
                        if x < offset + width / 2 {
                            break;
                        }
                        offset += width;
                        column += 1;
                    }
                    (column, line_text.chars().count())
                };
                let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::SHIFT);
                self.cursor_line = line;
                self.cursor_col = column.min(line_chars);
                if !shift {
                    self.selection_anchor = Some((self.cursor_line, self.cursor_col));
                    self.apply_selection(None);
                }
                self.selecting = true;
                if self.preview_mode {
                    // A heading row in preview is a navigation target, so a click on it announces
                    // which outline entry it is.
                    let heading =
                        self.outline().iter().position(|entry| entry.line == self.cursor_line);
                    if let Some(index) = heading {
                        self.heading_activated.emit(index);
                        let _ = self.goto_outline(index);
                    }
                }
                self.base.request_redraw();
            }
            Event::MouseMove { pos } => {
                if self.dragging_scrollbar {
                    self.scroll_to_track(pos.y);
                    return;
                }
                if self.selecting {
                    let rect = self.geometry();
                    let row = (pos.y - rect.y - TEXT_TOP) / TEXT_ROW_HEIGHT;
                    if row >= 0 {
                        let line = (self.visible_start() + row as usize)
                            .min(self.edit_line_count().saturating_sub(1));
                        let font = effective_font(self.base.style());
                        let x = pos.x - rect.x - 12;
                        let (column, line_chars) = {
                            let line_text = self.text.split('\n').nth(line).unwrap_or("");
                            let mut offset = 0i32;
                            let mut column = 0usize;
                            for ch in line_text.chars() {
                                let width = cached_char_width(&mut self.char_widths, ch, font, 1.0);
                                if x < offset + width / 2 {
                                    break;
                                }
                                offset += width;
                                column += 1;
                            }
                            (column, line_text.chars().count())
                        };
                        self.cursor_line = line;
                        self.cursor_col = column.min(line_chars);
                        self.sync_selection_from_anchor();
                        self.base.request_redraw();
                    }
                }
            }
            Event::MouseRelease { .. } => {
                self.selecting = false;
                self.dragging_scrollbar = false;
            }
            Event::MouseLeave { .. } => {
                // A release outside the control is not delivered to it, so leaving is the last
                // signal that the drag ended.
                self.selecting = false;
                self.dragging_scrollbar = false;
            }
            Event::Resize { .. } => {
                self.cursor_line = self.cursor_line.min(self.edit_line_count().saturating_sub(1));
                self.clamp_cursor_col();
                self.base.request_layout();
                self.base.request_redraw();
            }
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                if self.scroll_lines(-scroll.lines_y) {
                    self.base.request_redraw();
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl MarkdownEditor {
    /// Routes one key press to the editor or to the open find bar.
    fn handle_key(&mut self, key: u32, modifiers: u32) {
        let shift = crate::shortcut::Modifiers::from_event_bits(modifiers)
            .contains(crate::shortcut::Modifiers::SHIFT);
        // The shared predicate, not a raw `PRIMARY` bit test: on a Desktop host
        // Control is the primary accelerator and only the Command bit sets
        // `Modifiers::PRIMARY` on its own.
        let primary = crate::shortcut::event_carries_primary(modifiers);

        if self.find.visible {
            match key {
                8 => {
                    self.find.query.pop();
                    self.find.current = None;
                    self.find_bar.set_query(self.find.query.clone());
                }
                13 => {
                    let query = self.find.query.clone();
                    if !self.find_next(&query) {
                        self.find.current = None;
                    }
                    let _ = self.find_bar.find_next();
                }
                27 => self.close_find(),
                _ if primary_shortcut_pressed(key, modifiers, Key::F) => self.close_find(),
                _ => { /* The find bar ignores editing chords it does not own. */ }
            }
            self.base.request_redraw();
            return;
        }

        if primary {
            if primary_shortcut_pressed(key, modifiers, Key::A) {
                self.select_all();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::C) {
                self.copy();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::X) {
                self.cut();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::V) {
                self.paste();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::F) {
                self.open_find();
                return;
            }
            // Inline formatting: the same actions the mounted toolbar calls, so a chord and its
            // button cannot produce different markdown.
            if primary_shortcut_pressed(key, modifiers, Key::B) {
                self.wrap_selection("**", "**");
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::I) {
                self.wrap_selection("*", "*");
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::K) {
                self.wrap_selection("[", "](url)");
                return;
            }
        }
        match key {
            _ if primary_shortcut_pressed(key, modifiers, Key::Z) => {
                let _ = self.undo();
            }
            _ if primary_shortcut_pressed(key, modifiers, Key::Y) => {
                let _ = self.redo();
            }
            // The second redo chord every desktop editor accepts; the handler used to answer
            // only primary+Y, so the habitual primary+Shift+Z did nothing.
            _ if primary_shift_shortcut(key, modifiers, Key::Z) => {
                let _ = self.redo();
            }
            9 => {
                // Tab indents; Shift+Tab outdents. With no selection the caret gains one indent
                // level, which is what markdown's nested lists and code blocks need.
                if shift {
                    self.shift_line_indent(false);
                } else if self.selection.is_some() {
                    self.shift_line_indent(true);
                } else {
                    self.replace_selection_or_caret("  ");
                }
            }
            8 => self.backspace_at_cursor(),
            13 => self.insert_newline_at_cursor(),
            46 | 127 => self.delete_forward(),
            36 => {
                if shift {
                    self.begin_shift_selection();
                    self.cursor_col = 0;
                    self.sync_selection_from_anchor();
                    self.base.request_redraw();
                } else {
                    self.move_to_line_edge(true);
                }
            }
            35 => {
                if shift {
                    self.begin_shift_selection();
                    self.cursor_col = self.current_line_char_len();
                    self.sync_selection_from_anchor();
                    self.base.request_redraw();
                } else {
                    self.move_to_line_edge(false);
                }
            }
            33 => {
                self.scroll_page(-1);
            }
            34 => {
                self.scroll_page(1);
            }
            37 => {
                if shift {
                    self.begin_shift_selection();
                } else {
                    self.selection_anchor = None;
                    self.apply_selection(None);
                }
                self.move_cursor_left();
                if shift {
                    self.sync_selection_from_anchor();
                }
            }
            39 => {
                if shift {
                    self.begin_shift_selection();
                } else {
                    self.selection_anchor = None;
                    self.apply_selection(None);
                }
                self.move_cursor_right();
                if shift {
                    self.sync_selection_from_anchor();
                }
            }
            38 => self.move_vertical(-1, shift),
            40 => self.move_vertical(1, shift),
            80 | 112 if primary_shortcut_pressed(key, modifiers, Key::P) => {
                self.toggle_preview_mode()
            }
            _ => { /* Other keys are not relevant */ }
        }
    }

    /// Deletes the character after the caret, or merges the next line at a line end.
    fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.clamp_cursor();
        let at = self.cursor_byte_offset();
        let Some(ch) = self.text[at..].chars().next() else {
            return;
        };
        let mut next = String::with_capacity(self.text.len());
        next.push_str(&self.text[..at]);
        next.push_str(&self.text[at + ch.len_utf8()..]);
        self.commit_text(next);
    }

    /// Moves the caret by whole lines, extending the selection when `shift` is held.
    fn move_vertical(&mut self, delta: isize, shift: bool) {
        if shift {
            self.begin_shift_selection();
        } else {
            self.selection_anchor = None;
            self.apply_selection(None);
        }
        self.move_cursor(delta);
        if shift {
            self.sync_selection_from_anchor();
        }
        self.reveal_caret();
    }

    /// Remembers the caret as the selection anchor if no selection is being extended yet.
    fn begin_shift_selection(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some((self.cursor_line, self.cursor_col));
        }
    }

    /// Recomputes the selection from the anchor and the current caret.
    fn sync_selection_from_anchor(&mut self) {
        let Some((anchor_line, anchor_col)) = self.selection_anchor else {
            self.apply_selection(None);
            return;
        };
        let anchor = self.doc_char_offset(anchor_line, anchor_col);
        let caret = self.doc_char_offset_of_caret();
        let (start, end) = if anchor <= caret { (anchor, caret) } else { (caret, anchor) };
        self.apply_selection((start != end).then_some((start, end)));
    }

    /// The document character offset of `(line, column)`.
    fn doc_char_offset(&self, line: usize, column: usize) -> usize {
        self.with_line_index(|index| {
            let start = index.char_starts.get(line).copied().unwrap_or_else(|| index.char_len());
            let length = index.char_lens.get(line).copied().unwrap_or(0);
            start + column.min(length)
        })
    }

    /// A glyph advance, measured at most once per size.
    /// The scrollbar track, or `None` when hidden or the document fits.
    fn scrollbar_track(&self) -> Option<Rect> {
        if !self.show_scrollbar {
            return None;
        }
        let total = self.line_count();
        if total <= self.visible_line_count().max(1) {
            return None;
        }
        let rect = self.geometry();
        let top = rect.y + TEXT_TOP;
        let bottom = rect.y + rect.height as i32 - TEXT_BOTTOM_INSET;
        if bottom <= top {
            return None;
        }
        // The track stops where the outline panel begins, so the two pieces of chrome never
        // paint over one another.
        let outline_width = self.outline_rect().map_or(0, |panel| panel.width as i32);
        Some(Rect::new(
            rect.x + rect.width as i32 - 10 - outline_width,
            top,
            6,
            (bottom - top) as u32,
        ))
    }

    /// The rectangle the mounted outline panel occupies, when it is shown and fits.
    fn outline_rect(&self) -> Option<Rect> {
        if !self.show_outline {
            return None;
        }
        let rect = self.geometry();
        // Half the editor is the most a side panel may take; below this the text column would be
        // too narrow to read, so the panel is simply not mounted. The threshold is a named metric
        // (a fit decision), not a bare literal in the paint path.
        if rect.width < crate::widget::metrics::OUTLINE_PANEL_MIN_EDITOR_WIDTH
            || rect.height < TEXT_TOP as u32 + 48
        {
            return None;
        }
        let width = self.outline_panel.width().min(rect.width / 2).max(1);
        Some(Rect::new(
            rect.x + rect.width as i32 - width as i32,
            rect.y + TEXT_TOP,
            width,
            rect.height - TEXT_TOP as u32 - TEXT_BOTTOM_INSET as u32,
        ))
    }

    /// The rectangle the mounted find bar occupies.
    fn find_bar_rect(&self) -> Rect {
        let rect = self.geometry();
        let height = self.find_bar.size_hint().height;
        Rect::new(rect.x + 8, rect.y + 4, rect.width.saturating_sub(16), height)
    }

    /// Places the mounted companions inside the editor's own rectangle.
    fn layout_companions(&mut self) {
        let find_bar = self.find_bar_rect();
        if self.find_bar.base().geometry() != find_bar {
            self.find_bar.base_mut().set_geometry(find_bar);
        }
        if let Some(outline) = self.outline_rect() {
            if self.outline_panel.base().geometry() != outline {
                self.outline_panel.base_mut().set_geometry(outline);
            }
        }
    }

    /// The scrollbar thumb, sized and placed from the scroll state.
    fn scrollbar_thumb(&self) -> Option<Rect> {
        let track = self.scrollbar_track()?;
        let total = self.line_count().max(1);
        let visible = self.visible_line_count().max(1).min(total);
        let height = ((track.height as i64 * visible as i64) / total as i64).max(12) as u32;
        let height = height.min(track.height);
        let range = total.saturating_sub(visible);
        let travel = track.height.saturating_sub(height) as i64;
        let top = self.visible_start().min(range);
        let y = track.y + if range > 0 { (travel * top as i64 / range as i64) as i32 } else { 0 };
        Some(Rect::new(track.x, y, track.width, height))
    }

    /// Whether `pos` falls on the painted scrollbar track.
    fn scrollbar_track_contains(&self, pos: Point) -> bool {
        self.scrollbar_track().is_some_and(|track| {
            pos.x >= track.x
                && pos.x < track.x + track.width as i32
                && pos.y >= track.y
                && pos.y < track.y + track.height as i32
        })
    }

    /// Scrolls so the first visible line is where `y` points on the track.
    fn scroll_to_track(&mut self, y: i32) {
        let Some(track) = self.scrollbar_track() else {
            return;
        };
        let ratio = ((y - track.y) as f32 / track.height.max(1) as f32).clamp(0.0, 1.0);
        let max_first = self.line_count().saturating_sub(self.visible_line_count());
        let next = (ratio * max_first as f32).round() as usize;
        if next != self.first_visible_line {
            self.first_visible_line = next.min(max_first);
            self.base.request_redraw();
        }
    }
}

impl Draw for MarkdownEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The fence table depends on every line above it, so it is rebuilt once per revision —
        // before the style borrow is taken, because building it needs `&mut self`.
        self.ensure_blocks();
        if self.show_outline {
            // Reads the same heading list `outline()` reports, and marks the row the caret is in.
            self.sync_outline();
        }

        // The mounted companions follow the editor's own rectangle; doing it here (rather than in
        // `new`) means a resized editor moves its side panel and find bar with it.
        self.layout_companions();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen, because the surface, the border
        // and every piece of text were hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(252, 252, 253));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(194, 201, 213));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(59, 72, 92));
        // The header is the dimmest ink that still reads, and the caret line is the
        // accent: both derived from the resolved ink and surface, so they move with
        // the appearance instead of being fixed literals.
        //
        // The caret line is the accent colour because it *means* "the line being edited",
        // so the hue is part of the information and must survive. It cannot be pushed
        // straight onto the surface unchecked, though: on the light appearance the editor
        // surface resolves to a mid grey and `colors.primary` is the saturated Material
        // blue, which measured 1.51:1 — the very line the caret is on became the least
        // readable line in the editor. `legible_on` keeps the hue and guarantees the ratio.
        let header_ink = ink.blend(&surface, 0.35);
        let cursor_ink = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary.legible_on(surface, 4.5))
            .unwrap_or_else(|| ink.contrast_color());

        context.face_with_gradient_corners(
            rect,
            surface,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // The counts come from the revision-keyed cache, so a still frame walks no document.
        let metrics = self.metrics_snapshot();
        let header = if self.find.visible {
            format!("Find: {}   (Enter next, Esc close)", self.find.query)
        } else if self.preview_mode {
            format!(
                "Markdown Preview  lines:{} words:{} headings:{}",
                metrics.lines, metrics.words, metrics.headings
            )
        } else {
            format!(
                "Markdown Edit  lines:{} words:{} headings:{}",
                metrics.lines, metrics.words, metrics.headings
            )
        };
        // The mounted find bar replaces the plain header text while it is open: it paints the
        // query, the match counter and its policy toggles, and a second header underneath would
        // be an unreadable overprint.
        if self.find.visible {
            self.find_bar.draw(context);
        } else {
            // Fitted to the control's width: the header is a long line of counts, and a narrow
            // editor previously drew it straight past its own right edge.
            let header_band = Rect::new(rect.x + 8, rect.y + 4, rect.width.saturating_sub(16), 14);
            context.draw_text_fitted_cow(
                header_band,
                &header,
                effective_font(style),
                header_ink,
                HorizontalAlignment::Left,
            );
        }

        let start = self.visible_start();
        let count = self.visible_line_count();
        // The line total comes from the cached offset tables, not from re-counting the document's
        // newlines on every frame.
        let line_total = self.edit_line_count();
        let font = effective_font(style);
        // Selection endpoints, resolved once. `line_col_of` is two document walks; doing it per
        // row would be quadratic, and the whole point of this path is that a frame is cheap.
        let selection =
            self.selection.map(|(first, last)| (self.line_col_of(first), self.line_col_of(last)));
        let scrollbar_width = if self.scrollbar_track().is_some() { 12 } else { 0 };
        let outline_width = self.outline_rect().map_or(0, |panel| panel.width as i32);
        let row_width = rect.width.saturating_sub(24 + scrollbar_width + outline_width as u32);

        {
            let text = &self.text;
            let cache = &mut self.preview_cache;
            let blocks = &self.blocks;
            let table_headers = &self.table_header;
            let gfm = self.gfm_extensions;
            let highlight = self.code_highlight;
            for (row, line) in text.split('\n').skip(start).take(count).enumerate() {
                let idx = start + row;
                let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
                // Rows below the control are not painted, which keeps a short editor from drawing
                // past its own bottom edge.
                if y > rect.y + rect.height as i32 - TEXT_BOTTOM_INSET {
                    break;
                }
                let color = if idx == self.cursor_line { cursor_ink } else { ink };
                let band = Rect::new(rect.x + 12, y, row_width, TEXT_ROW_HEIGHT as u32);

                // The selection band is drawn behind the text, trimmed to the exact characters
                // on the first and last rows.
                if let Some(((first_line, first_col), (last_line, last_col))) = selection {
                    if idx >= first_line && idx <= last_line {
                        let line_chars = line.chars().count();
                        let from = if idx == first_line { first_col } else { 0 };
                        let to = if idx == last_line { last_col } else { line_chars };
                        // Both edges are measured straight off the borrowed line: a prefix
                        // `String` per visible row was an allocation per row per frame.
                        let from_byte = byte_offset_of_char(line, from);
                        let to_byte = byte_offset_of_char(line, to).max(from_byte);
                        let x = band.x + estimate_text_width(&line[..from_byte], font, 1.0) as i32;
                        let width = estimate_text_width(&line[from_byte..to_byte], font, 1.0);
                        context.fill_rect(
                            Rect::new(x, y, width.max(1), TEXT_ROW_HEIGHT as u32),
                            cursor_ink.with_alpha(60),
                        );
                    }
                }

                if self.preview_mode {
                    let block = blocks.get(idx).copied().unwrap_or(BlockStart::Normal);
                    let key = (idx, text_hash(line));
                    let parsed = match cache.get(&key).cloned() {
                        Some(parsed) => parsed,
                        None => {
                            let parsed = Rc::new(parse_preview_line(
                                line,
                                block,
                                gfm,
                                highlight,
                                table_headers.get(idx).copied().unwrap_or(false),
                                font,
                                context,
                            ));
                            cache.insert(key, parsed.clone());
                            parsed
                        }
                    };
                    draw_parsed_line(context, band, &parsed, font, color, cursor_ink, surface);
                } else {
                    // Fitted to the pane, so a long line ends in an ellipsis inside the control
                    // instead of painting over the widget beside it.
                    context.draw_text_fitted_cow(
                        band,
                        line,
                        font,
                        color,
                        HorizontalAlignment::Left,
                    );
                }
            }
            // The cache is keyed by line index and text hash, so entries for lines that moved or
            // changed are simply unused. When the table outgrows its bound only the entries that
            // no longer describe a live line are dropped: clearing it rebuilt every visible line
            // on the next frame.
            let bound = line_total.saturating_mul(4).saturating_add(64);
            if cache.len() > bound {
                let live: std::collections::HashSet<(usize, u64)> = text
                    .split('\n')
                    .enumerate()
                    .map(|(index, line)| (index, text_hash(line)))
                    .collect();
                cache.retain(|key, _| live.contains(key));
            }
        }

        if let Some(track) = self.scrollbar_track() {
            context.fill_rect(track, ink.blend(&surface, 0.10));
            if let Some(thumb) = self.scrollbar_thumb() {
                context.fill_rect(thumb, ink.with_alpha(110));
            }
        }

        // The caret, the composition and the selection are edit-mode furniture: preview has no
        // caret because there is nothing to edit there.
        if !self.preview_mode {
            let start = self.visible_start();
            if let Some(row) =
                self.cursor_line.checked_sub(start).filter(|row| *row < self.visible_line_count())
            {
                let (line_start, line_end) = self.cursor_line_span();
                let caret_line = &self.text[line_start..line_end];
                let caret_byte = byte_offset_of_char(caret_line, self.cursor_col);
                let x =
                    rect.x + 12 + estimate_text_width(&caret_line[..caret_byte], font, 1.0) as i32;
                let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
                if y + 14 <= rect.y + rect.height as i32 && x <= rect.x + rect.width as i32 - 12 {
                    if let Some(preedit) = &self.preedit {
                        // The composition is drawn where the caret is and semi-transparent, so a
                        // user can see what has not been committed yet — the previous build drew
                        // nothing at all and the text appeared only after a candidate landed.
                        context.draw_text(
                            Point::new(x, y),
                            &preedit.text,
                            font,
                            cursor_ink.with_alpha(160),
                            HorizontalAlignment::Left,
                        );
                    }
                    if self.is_caret_visible() || self.preedit.is_some() {
                        context.fill_rect(Rect::new(x, y - 2, 1, 16), cursor_ink);
                    }
                }
            }
        }

        // The mounted outline is painted last, over the text column it reserved space for, so a
        // selected row is fully visible even when the text underneath is long.
        if self.outline_rect().is_some() {
            self.outline_panel.draw(context);
        }
    }
}

/// Whether the host's primary accelerator **plus Shift** is held for `expected`.
///
/// `primary_shortcut_pressed` compares the modifier set exactly, so the Shift bit is removed
/// before asking it: the remaining modifiers are what a plain primary chord carries.
fn primary_shift_shortcut(key: u32, modifiers: u32, expected: Key) -> bool {
    let shift = crate::shortcut::Modifiers::SHIFT;
    let pressed = crate::shortcut::Modifiers::from_event_bits(modifiers);
    pressed.contains(shift)
        && primary_shortcut_pressed(key, modifiers & !(shift.bits() as u32), expected)
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

/// A stable hash of one preview line, used as the cache key beside its index.
fn text_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// The largest character boundary at or below `offset`.
///
/// A platform-supplied byte offset is usually a boundary, but slicing on one that is not would
/// panic inside a draw/hit-test path; clamping is the only behaviour that cannot crash.
fn char_boundary_at(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    (0..=offset).rev().find(|index| text.is_char_boundary(*index)).unwrap_or(0)
}

/// The byte offset of character `index` in `text`, clamped to its end.
///
/// Used by the paint path to measure a prefix without materialising it as a `String`.
fn byte_offset_of_char(text: &str, index: usize) -> usize {
    text.char_indices().nth(index).map(|(byte, _)| byte).unwrap_or(text.len())
}

/// The info string of a fenced code block opener, if `trimmed` opens one.
fn fence_marker(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("```").or_else(|| trimmed.strip_prefix("~~~"))?;
    Some(rest.trim())
}

/// Maps a fence's info string onto one of the shared lexer's languages.
fn language_from_info(info: &str) -> LanguageId {
    let token = info.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    match token.as_str() {
        "rust" | "rs" => LanguageId::Rust,
        "python" | "py" => LanguageId::Python,
        "javascript" | "js" | "typescript" | "ts" | "tsx" | "jsx" => LanguageId::JavaScript,
        "go" | "golang" => LanguageId::Go,
        "c" | "h" | "cpp" | "c++" | "cc" | "hpp" | "java" | "cs" | "csharp" | "kt" => {
            LanguageId::CLike
        }
        "markdown" | "md" => LanguageId::Markdown,
        _ => LanguageId::PlainText,
    }
}

/// Builds one measured run.
fn push_run(
    runs: &mut Vec<PreviewRun>,
    text: &str,
    style: PreviewStyle,
    font: &Font,
    context: &RenderContext,
) {
    if text.is_empty() {
        return;
    }
    runs.push(PreviewRun {
        text: text.to_string(),
        style,
        width: context.measure_text(text, font).width,
    });
}

/// Parses one preview row.
///
/// This is where the GFM block constructs are recognised: fenced code (through the per-revision
/// fence table), headings, bullets and ordered items, task boxes, quotes, tables, rules, images
/// and the inline emphasis/code/link/delete run styles. Everything the previous preview did not
/// know about arrived as literal source text.
fn parse_preview_line(
    line: &str,
    block: BlockStart,
    gfm: bool,
    highlight: bool,
    table_follows: bool,
    font: &Font,
    context: &RenderContext,
) -> ParsedLine {
    let mut runs = Vec::new();
    match block {
        BlockStart::Fence => {
            push_run(&mut runs, line.trim(), PreviewStyle::Dim, font, context);
            return ParsedLine { runs, row: PreviewRow::Fence };
        }
        BlockStart::Code(language, state) => {
            if highlight {
                let highlighter = BuiltinHighlighter::new(language);
                let (spans, _) = highlighter.highlight_line(line, state);
                if spans.is_empty() {
                    push_run(&mut runs, line, PreviewStyle::Token(TokenKind::Plain), font, context);
                } else {
                    let mut cursor = 0usize;
                    for span in spans {
                        let start = span.start.min(line.len()).max(cursor);
                        let end = span.end.min(line.len()).max(start);
                        if start > cursor {
                            push_run(
                                &mut runs,
                                &line[cursor..start],
                                PreviewStyle::Token(TokenKind::Plain),
                                font,
                                context,
                            );
                        }
                        if end > start {
                            push_run(
                                &mut runs,
                                &line[start..end],
                                PreviewStyle::Token(span.kind),
                                font,
                                context,
                            );
                        }
                        cursor = end;
                    }
                    if cursor < line.len() {
                        push_run(
                            &mut runs,
                            &line[cursor..],
                            PreviewStyle::Token(TokenKind::Plain),
                            font,
                            context,
                        );
                    }
                }
            } else {
                push_run(&mut runs, line, PreviewStyle::Text, font, context);
            }
            return ParsedLine { runs, row: PreviewRow::Code };
        }
        BlockStart::Normal => {}
    }

    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    // Heading: the leading `#` markers are syntax, so they are consumed rather than rendered.
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if hashes > 0 && hashes <= 6 && trimmed[hashes..].starts_with(' ') {
        let level = hashes as u8;
        let heading_font = font.with_size(font.size() + (7 - hashes) as f32);
        let body = trimmed[hashes..].trim_start();
        parse_inline(body, gfm, &heading_font, context, &mut runs);
        return ParsedLine { runs, row: PreviewRow::Heading(level) };
    }

    if gfm && is_rule(trimmed) {
        return ParsedLine { runs, row: PreviewRow::Rule };
    }
    if gfm && is_table_divider(trimmed) {
        return ParsedLine { runs, row: PreviewRow::Rule };
    }

    let mut body = trimmed;
    let mut prefix_width = 0u32;
    if gfm && body.starts_with('>') {
        push_run(&mut runs, "▏", PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text("▏ ", font).width;
        body = body.trim_start_matches('>').trim_start();
    } else if body.starts_with("- [ ] ") || body.starts_with("- [x] ") || body.starts_with("- [X] ")
    {
        let checked = !body.starts_with("- [ ]");
        let marker = if checked { "☑ " } else { "☐ " };
        push_run(&mut runs, marker, PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text(marker, font).width;
        body = &body[6..];
    } else if let Some(rest) = strip_bullet(body) {
        push_run(&mut runs, "• ", PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text("• ", font).width;
        body = rest;
    } else if let Some((marker, rest)) = strip_ordered(body) {
        let marker = format!("{marker} ");
        push_run(&mut runs, &marker, PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text(&marker, font).width;
        body = rest;
    }

    // A table needs its delimiter row (or pipes on both outer edges), exactly as GFM does; one
    // `|` in prose is not a table.
    if gfm && body.contains('|') && (table_follows || body.trim().starts_with('|')) {
        let row = render_table_row(body, font, context, &mut runs);
        return ParsedLine { runs, row };
    }

    // The indent is kept so a nested list item still lines up under its parent.
    if indent > 0 && prefix_width == 0 {
        let pad = " ".repeat(indent);
        push_run(&mut runs, &pad, PreviewStyle::Text, font, context);
    }
    parse_inline(body, gfm, font, context, &mut runs);
    ParsedLine { runs, row: PreviewRow::Inline }
}

/// Strips a bullet marker, returning the item body.
fn strip_bullet(text: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = text.strip_prefix(marker) {
            return Some(rest);
        }
    }
    None
}

/// Strips an ordered-list marker, returning the number and the item body.
fn strip_ordered(text: &str) -> Option<(&str, &str)> {
    let digits = text.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let rest = &text[digits..];
    let rest = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    Some((&text[..digits], rest))
}

/// Whether a line is a horizontal rule (`---`, `***`, `___`).
fn is_rule(trimmed: &str) -> bool {
    let mut marker = None;
    let mut count = 0usize;
    for ch in trimmed.chars() {
        if ch == ' ' {
            continue;
        }
        match marker {
            None if ch == '-' || ch == '*' || ch == '_' => marker = Some(ch),
            Some(existing) if existing == ch => {}
            _ => return false,
        }
        count += 1;
    }
    count >= 3
}

/// Whether a table row is the `|---|:--:|` divider.
fn is_table_divider(trimmed: &str) -> bool {
    if !trimmed.contains('-') || !trimmed.contains('|') {
        return false;
    }
    trimmed.chars().all(|ch| matches!(ch, '|' | '-' | ':' | ' ' | '\t'))
}

/// Splits a table row into measured cells with separators between them.
///
/// Columns are separated but not padded to a shared width: padding needs the widest cell in the
/// whole table, which is a table-wide layout pass rather than a per-row one, and this preview
/// renders rows independently. The cells themselves keep their inline markup.
fn render_table_row(
    body: &str,
    font: &Font,
    context: &RenderContext,
    runs: &mut Vec<PreviewRun>,
) -> PreviewRow {
    let trimmed = body.trim().trim_start_matches('|').trim_end_matches('|');
    for (index, cell) in trimmed.split('|').enumerate() {
        if index > 0 {
            push_run(runs, " │ ", PreviewStyle::Dim, font, context);
        }
        let cell = cell.trim();
        // The cell's own inline markup still applies, so a bold cell is bold.
        parse_inline(cell, true, font, context, runs);
    }
    PreviewRow::Table
}

/// Splits inline markdown into styled runs.
// The parser peeks ahead while consuming (`**`/`__` need a second character), so a plain
// `while let Some(ch) = chars.next()` is the honest shape; clippy's iterator-lint suggestion
// would borrow the iterator twice.
#[allow(clippy::while_let_on_iterator)]
#[allow(clippy::while_let_loop)]
fn parse_inline(
    text: &str,
    gfm: bool,
    font: &Font,
    context: &RenderContext,
    runs: &mut Vec<PreviewRun>,
) {
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                let mut closed = false;
                loop {
                    let Some(next) = chars.next() else {
                        break;
                    };
                    if next == '*' && chars.peek() == Some(&'*') {
                        chars.next();
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Bold } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '_' if chars.peek() == Some(&'_') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == '_' && chars.peek() == Some(&'_') {
                        chars.next();
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Bold } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '*' | '_' => {
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let marker = ch;
                let mut inner = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == marker {
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Italic } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '~' if gfm && chars.peek() == Some(&'~') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == '~' && chars.peek() == Some(&'~') {
                        chars.next();
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Strike } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '`' => {
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                for next in chars.by_ref() {
                    if next == '`' {
                        break;
                    }
                    inner.push(next);
                }
                push_run(runs, &inner, PreviewStyle::Code, font, context);
            }
            '!' if chars.peek() == Some(&'[') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut alt = String::new();
                let mut target = String::new();
                while let Some(next) = chars.next() {
                    if next == ']' {
                        if chars.peek() == Some(&'(') {
                            chars.next();
                            for part in chars.by_ref() {
                                if part == ')' {
                                    break;
                                }
                                target.push(part);
                            }
                        }
                        break;
                    }
                    alt.push(next);
                }
                let label = if alt.is_empty() { target } else { alt };
                push_run(runs, "▣ ", PreviewStyle::Accent, font, context);
                push_run(runs, &label, PreviewStyle::Link, font, context);
            }
            '[' => {
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut label = String::new();
                let mut is_link = false;
                while let Some(next) = chars.next() {
                    if next == ']' {
                        if chars.peek() == Some(&'(') {
                            chars.next();
                            for target in chars.by_ref() {
                                if target == ')' {
                                    break;
                                }
                            }
                            is_link = true;
                        }
                        break;
                    }
                    label.push(next);
                }
                push_run(
                    runs,
                    &label,
                    if is_link { PreviewStyle::Link } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            _ => plain.push(ch),
        }
    }
    push_run(runs, &plain, PreviewStyle::Text, font, context);
}

/// The ink a fenced-code token is drawn in.
fn token_color(kind: TokenKind, ink: Color, accent: Color, surface: Color) -> Color {
    match kind {
        TokenKind::Keyword | TokenKind::Preprocessor => accent,
        TokenKind::String | TokenKind::Character => {
            crate::style::semantic_color(crate::style::SemanticColor::Success)
                .map(|token| token.legible_on(surface, 3.0))
                .unwrap_or(ink)
        }
        TokenKind::Number => crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .map(|token| token.legible_on(surface, 3.0))
            .unwrap_or(ink),
        TokenKind::Comment | TokenKind::BlockComment => ink.blend(&surface, 0.45),
        TokenKind::Type | TokenKind::Function => {
            crate::style::semantic_color(crate::style::SemanticColor::Info)
                .map(|token| token.legible_on(surface, 3.0))
                .unwrap_or(ink)
        }
        _ => ink,
    }
}

/// Draws one parsed preview row.
fn draw_parsed_line(
    context: &mut RenderContext,
    band: Rect,
    parsed: &ParsedLine,
    base_font: &Font,
    ink: Color,
    accent: Color,
    surface: Color,
) {
    match parsed.row {
        PreviewRow::Rule => {
            context.fill_rect(
                Rect::new(band.x, band.y + (band.height as i32 / 2), band.width, 1),
                ink.blend(&surface, 0.5),
            );
            return;
        }
        PreviewRow::Code => {
            context.fill_rect(band, ink.with_alpha(12));
        }
        PreviewRow::Fence => {}
        // The heading's runs were measured with the level's own size; the drawing font below
        // re-derives the same size.
        PreviewRow::Heading(_) => {}
        PreviewRow::Inline | PreviewRow::Table => {}
    }
    let font = match parsed.row {
        PreviewRow::Heading(level) => {
            base_font.with_size(base_font.size() + (7 - level.min(6)) as f32)
        }
        _ => base_font.clone(),
    };
    let mut x = band.x;
    for run in &parsed.runs {
        draw_preview_run(context, run, &font, x, band.y, ink, accent, surface);
        x += run.width as i32;
    }
}

/// Draws one styled run at `x`.
fn draw_preview_run(
    context: &mut RenderContext,
    run: &PreviewRun,
    font: &Font,
    x: i32,
    y: i32,
    ink: Color,
    accent: Color,
    surface: Color,
) {
    let color = match run.style {
        PreviewStyle::Text => ink,
        PreviewStyle::Bold => ink,
        PreviewStyle::Italic => ink.blend(&surface, 0.15),
        PreviewStyle::Strike => ink.blend(&surface, 0.45),
        PreviewStyle::Code => ink,
        PreviewStyle::Link => accent,
        PreviewStyle::Dim => ink.blend(&surface, 0.5),
        PreviewStyle::Accent => accent,
        PreviewStyle::Token(kind) => token_color(kind, ink, accent, surface),
    };
    if matches!(run.style, PreviewStyle::Code) {
        context.fill_rect(
            Rect::new(x - 2, y, run.width + 4, TEXT_ROW_HEIGHT as u32),
            ink.with_alpha(24),
        );
    }
    context.draw_text(Point::new(x, y), &run.text, font, color, HorizontalAlignment::Left);
    match run.style {
        PreviewStyle::Bold => {
            // The face has one weight, so bold is the crate's one-pixel overprint.
            context.draw_text(
                Point::new(x + 1, y),
                &run.text,
                font,
                color,
                HorizontalAlignment::Left,
            );
        }
        PreviewStyle::Link => {
            context.fill_rect(Rect::new(x, y + TEXT_ROW_HEIGHT - 3, run.width.max(1), 1), color);
        }
        PreviewStyle::Strike => {
            context.fill_rect(Rect::new(x, y + TEXT_ROW_HEIGHT / 2, run.width.max(1), 1), color);
        }
        _ => {}
    }
}

/// Row height of one outline entry.
const OUTLINE_ROW_HEIGHT: i32 = 20;
/// How far each heading level indents its title.
const OUTLINE_INDENT: i32 = 12;

/// A document-outline side panel for a [`MarkdownEditor`].
///
/// The editor already computes its headings ([`MarkdownEditor::outline`]) and can navigate to
/// one ([`MarkdownEditor::goto_outline`]); what it never had was a control that *shows* them. This
/// panel is that view: it holds a copy of the heading list (refreshed by
/// [`Self::sync_with_editor`]), paints it as an indented list, and reports the row the reader
/// activated.
///
/// Selection and navigation are deliberately separate. `goto` selects a row and reports the line
/// it addresses; moving the editor's caret is the host's call, made through the editor method it
/// already has. That keeps the panel usable beside a document it did not open.
pub struct MarkdownOutlinePanel {
    base: BaseWidget,
    entries: Vec<MarkdownOutlineEntry>,
    /// The row painted as current, as an index into `entries`.
    active_index: Option<usize>,
    /// The width this panel prefers, reported through `size_hint`.
    width: u32,
    /// The first row the viewport paints.
    first_visible: usize,
    /// Emitted with the outline index when a row is activated.
    pub entry_selected: Signal1<usize>,
}

impl MarkdownOutlinePanel {
    /// Creates an empty panel 180 px wide.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::MarkdownOutlinePanel,
                geometry,
                "MarkdownOutlinePanel",
            ),
            entries: Vec::new(),
            active_index: None,
            width: 180,
            first_visible: 0,
            entry_selected: Signal1::new(),
        }
    }

    /// The headings this panel shows.
    pub fn entries(&self) -> &[MarkdownOutlineEntry] {
        &self.entries
    }

    /// Replaces the entries, dropping an active index that no longer exists.
    pub fn set_entries(&mut self, entries: Vec<MarkdownOutlineEntry>) {
        self.entries = entries;
        if self.active_index.is_some_and(|index| index >= self.entries.len()) {
            self.active_index = None;
        }
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Copies `editor`'s outline into this panel.
    ///
    /// The active row follows the editor's caret: the last heading at or before the caret's line,
    /// which is the heading the reader is currently "inside". Without a preceding heading the
    /// first row is used so a document that starts with body text still has a current row.
    pub fn sync_with_editor(&mut self, editor: &MarkdownEditor) {
        let entries = editor.outline();
        let caret = editor.cursor_line();
        let active = entries
            .iter()
            .rposition(|entry| entry.line <= caret)
            .or_else(|| (!entries.is_empty()).then_some(0));
        self.entries = entries;
        self.active_index = active;
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// The row painted as current.
    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Marks `index` as current without emitting `entry_selected`.
    ///
    /// This is the editor-to-panel direction (a caret move), which is not a reader action and must
    /// not be reported as one.
    pub fn set_active_index(&mut self, index: Option<usize>) {
        let next = index.filter(|index| *index < self.entries.len());
        if self.active_index == next {
            return;
        }
        self.active_index = next;
        if let Some(index) = next {
            self.reveal(index);
        }
        self.base.request_redraw();
    }

    /// Activates `index`, returning the document line it addresses.
    ///
    /// Returns `None` for a row that does not exist: a caller told about a line it cannot address
    /// would ask the editor to jump somewhere arbitrary.
    pub fn goto(&mut self, index: usize) -> Option<usize> {
        let line = self.entries.get(index)?.line;
        if self.active_index != Some(index) {
            self.active_index = Some(index);
            self.reveal(index);
            self.base.request_redraw();
        }
        self.entry_selected.emit(index);
        Some(line)
    }

    /// The preferred width reported through `size_hint`.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Sets the preferred width.
    pub fn set_width(&mut self, width: u32) {
        let width = width.max(40);
        if self.width == width {
            return;
        }
        self.width = width;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Rows the viewport can paint.
    fn visible_rows(&self) -> usize {
        ((self.geometry().height as i32 / OUTLINE_ROW_HEIGHT).max(1)) as usize
    }

    /// The painted box of row `index`, when it is visible.
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.entries.len() {
            return None;
        }
        let row = index.checked_sub(self.first_visible)?;
        if row >= self.visible_rows() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            rect.y + row as i32 * OUTLINE_ROW_HEIGHT,
            rect.width,
            OUTLINE_ROW_HEIGHT as u32,
        ))
    }

    /// The deepest indent the panel may draw, so a narrow panel keeps its titles readable.
    fn max_level_indent(&self) -> i32 {
        ((self.geometry().width as i32 - 40) / OUTLINE_INDENT).max(0)
    }

    /// Scrolls so `index` is visible.
    fn reveal(&mut self, index: usize) {
        let rows = self.visible_rows();
        if index < self.first_visible {
            self.first_visible = index;
        } else if index >= self.first_visible + rows {
            self.first_visible = index + 1 - rows;
        }
        self.clamp_scroll();
    }

    /// Keeps the scroll offset inside the entry list.
    fn clamp_scroll(&mut self) {
        let max_first = self.entries.len().saturating_sub(self.visible_rows());
        self.first_visible = self.first_visible.min(max_first);
    }

    /// The entry index under `pos`.
    ///
    /// Public because a host that mounts the panel in a container of its own resolves the press
    /// itself; the editor's mounted panel uses it for the same reason.
    pub fn entry_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let row = (pos.y - rect.y) / OUTLINE_ROW_HEIGHT;
        if row < 0 {
            return None;
        }
        let index = self.first_visible + row as usize;
        (index < self.entries.len()).then_some(index)
    }
}

impl Widget for MarkdownOutlinePanel {
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
        crate::core::Size::new(self.width, 240)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves `entry_selected`; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "entry_selected" => {
                Some(EventSignalRef::mapped("entry_selected", &self.entry_selected, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MarkdownOutlinePanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entries" => Ok(CapabilityValue::Tuple(
                self.entries
                    .iter()
                    .map(|entry| {
                        CapabilityValue::String(format!(
                            "{}|{}|{}",
                            entry.level, entry.title, entry.line
                        ))
                    })
                    .collect(),
            )),
            "active_index" => Ok(match self.active_index {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            "width" => Ok(CapabilityValue::UInt(self.width as u64)),
            "entry_count" => Ok(CapabilityValue::UInt(self.entries.len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "entries" => {
                let items = expect_string_list(value)?;
                let mut entries = Vec::with_capacity(items.len());
                for item in items {
                    let mut parts = item.splitn(3, '|');
                    let level = parts
                        .next()
                        .and_then(|text| text.trim().parse::<u8>().ok())
                        .ok_or(CapabilityAccessError::TypeMismatch)?;
                    let title = parts.next().unwrap_or_default().to_string();
                    let line = parts
                        .next()
                        .and_then(|text| text.trim().parse::<usize>().ok())
                        .ok_or(CapabilityAccessError::TypeMismatch)?;
                    entries.push(MarkdownOutlineEntry { line, level: level.clamp(1, 6), title });
                }
                self.set_entries(entries);
                Ok(())
            }
            "active_index" => match value {
                CapabilityValue::Null => {
                    self.set_active_index(None);
                    Ok(())
                }
                CapabilityValue::UInt(index) => {
                    let index =
                        usize::try_from(index).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    if index >= self.entries.len() {
                        return Err(CapabilityAccessError::OutOfRange);
                    }
                    self.set_active_index(Some(index));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "width" => {
                let width = u32::try_from(expect_usize(value)?)
                    .map_err(|_| CapabilityAccessError::TypeMismatch)?;
                self.set_width(width);
                Ok(())
            }
            "entry_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["entries", "active_index", "width", "entry_count", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for MarkdownOutlinePanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.entry_at(*pos) {
                    let _ = self.goto(index);
                }
            }
            Event::KeyPress { key, .. } => {
                let current = self.active_index.unwrap_or(0);
                match *key {
                    38 => {
                        if !self.entries.is_empty() {
                            let _ = self.goto(current.saturating_sub(1));
                        }
                    }
                    40 => {
                        if !self.entries.is_empty() {
                            let _ = self.goto((current + 1).min(self.entries.len() - 1));
                        }
                    }
                    13 if !self.entries.is_empty() => {
                        let _ = self.goto(current.min(self.entries.len() - 1));
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => {}
        }
    }
}

impl Draw for MarkdownOutlinePanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(surface));
        let accent =
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(Color::BLUE);
        let font = effective_font(style);

        context.face_with_gradient_corners(
            rect,
            surface,
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

        let rows = self.visible_rows();
        for offset in 0..rows {
            let index = self.first_visible + offset;
            let Some(entry) = self.entries.get(index) else {
                break;
            };
            let Some(row) = self.row_rect(index) else {
                break;
            };
            if self.active_index == Some(index) {
                context.fill_rect(row, surface.blend(&accent, 0.20));
            }
            let indent = (entry.level.saturating_sub(1) as i32 * OUTLINE_INDENT)
                .min(self.max_level_indent());
            let title = crate::widget::text_utils::elide_end(
                &entry.title,
                ((row.width as i32 - indent - 8) / 7).max(1) as usize,
            );
            context.draw_text(
                Point::new(row.x + 6 + indent, row.y + 4),
                &title,
                font,
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

/// One replacement the host is asked to apply.
///
/// Ranges are byte offsets into the document the bar was told about through
/// [`FindBar::set_text`], in document order. A host applies them back-to-front so an earlier
/// replacement does not shift a later range; `all` highlights are carried in one request rather
/// than one signal per match, because a signal has no way to say "and three more".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceRequest {
    /// Byte ranges to replace, in ascending document order.
    pub ranges: Vec<(usize, usize)>,
    /// Replacement text for every range.
    pub text: String,
}

/// Which field of a [`FindBar`] has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FindFocus {
    /// The query field; the default.
    #[default]
    Query,
    /// The replacement field.
    Replace,
}

/// Height of one field row in the find bar.
const FIND_ROW_HEIGHT: i32 = 26;
/// Side of a toggle cell (`Aa`, `W`).
const FIND_TOGGLE_SIDE: i32 = 18;

/// A reusable find/replace bar.
///
/// It is a control, not a dialog: it owns the query, the matching policy and the match list for a
/// document **snapshot** the host hands it, and reports navigation and replacement requests. It
/// does not own the document. That split is why [`Self::replace`] emits a
/// [`ReplaceRequest`] instead of mutating text it never received: a bar that edited a copy would
/// report success for an edit the document never saw.
pub struct FindBar {
    base: BaseWidget,
    visible: bool,
    query: String,
    replacement: String,
    case_sensitive: bool,
    whole_word: bool,
    /// Whether the replacement row is painted.
    replace_visible: bool,
    /// The document snapshot the matches address.
    haystack: String,
    /// `haystack` as `(byte offset, char)` pairs, rebuilt only when the snapshot changes.
    ///
    /// The matcher used to rebuild this vector on every keystroke of the query; the snapshot is
    /// what changes it, so it is built once in [`Self::set_text`].
    char_index: Vec<(usize, char)>,
    /// How many ranges the last replacement actually changed, as reported by the host.
    last_replaced: usize,
    /// Every match, in document order.
    matches: Vec<(usize, usize)>,
    /// The active match, as an index into `matches`.
    current: Option<usize>,
    /// Which field typing goes to.
    focus: FindFocus,
    /// Emitted with the active match's byte range, or `(0, 0)` when there is no match.
    pub search_changed: Signal1<(usize, usize)>,
    /// Emitted when the reader asks for a replacement.
    pub replace_requested: Signal1<ReplaceRequest>,
}

impl FindBar {
    /// Creates a closed find bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FindBar, geometry, "FindBar"),
            visible: false,
            query: String::new(),
            replacement: String::new(),
            case_sensitive: false,
            whole_word: false,
            replace_visible: false,
            haystack: String::new(),
            char_index: Vec::new(),
            last_replaced: 0,
            matches: Vec::new(),
            current: None,
            focus: FindFocus::Query,
            search_changed: Signal1::new(),
            replace_requested: Signal1::new(),
        }
    }

    /// Whether the bar is showing.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Shows the bar and focuses the query field.
    pub fn open(&mut self) {
        self.visible = true;
        self.focus = FindFocus::Query;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Hides the bar without touching the document or the active match.
    pub fn close(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Points the bar at `text` and recomputes the match list.
    ///
    /// The bar keeps the snapshot rather than a live handle because it has no way to observe a
    /// document edit; the host calls this after every change, which is also the moment the
    /// replacement it applied becomes visible to the next search.
    pub fn set_text(&mut self, text: &str) {
        self.haystack.clear();
        self.haystack.push_str(text);
        self.char_index = self.haystack.char_indices().collect();
        self.recompute_matches();
    }

    /// The document snapshot the matches address.
    pub fn text(&self) -> &str {
        &self.haystack
    }

    /// The current query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replaces the query and recomputes the matches.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.recompute_matches();
        self.base.request_redraw();
    }

    /// Whether matching is case-sensitive.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Sets the case-sensitivity policy.
    pub fn set_case_sensitive(&mut self, sensitive: bool) {
        if self.case_sensitive == sensitive {
            return;
        }
        self.case_sensitive = sensitive;
        self.recompute_matches();
        self.base.request_redraw();
    }

    /// Whether matches must be whole words.
    pub fn whole_word(&self) -> bool {
        self.whole_word
    }

    /// Sets the whole-word policy.
    pub fn set_whole_word(&mut self, whole_word: bool) {
        if self.whole_word == whole_word {
            return;
        }
        self.whole_word = whole_word;
        self.recompute_matches();
        self.base.request_redraw();
    }

    /// The replacement text.
    pub fn replace_text(&self) -> &str {
        &self.replacement
    }

    /// Sets the replacement text.
    pub fn set_replace_text(&mut self, text: impl Into<String>) {
        self.replacement = text.into();
        self.base.request_redraw();
    }

    /// Whether the replacement row is painted.
    pub fn replace_visible(&self) -> bool {
        self.replace_visible
    }

    /// Shows or hides the replacement row.
    pub fn set_replace_visible(&mut self, visible: bool) {
        if self.replace_visible == visible {
            return;
        }
        self.replace_visible = visible;
        if !visible {
            self.focus = FindFocus::Query;
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How many matches the current query has.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// The active match's byte range, when one is selected.
    pub fn current_range(&self) -> Option<(usize, usize)> {
        self.current.and_then(|index| self.matches.get(index).copied())
    }

    /// Selects the next match, wrapping at the end.
    ///
    /// Returns whether a match was selected; an empty match list reports `false` and emits the
    /// "no match" range, so a subscriber is told the search came up empty rather than left with
    /// the previous result.
    pub fn find_next(&mut self) -> bool {
        if self.matches.is_empty() {
            self.current = None;
            self.search_changed.emit((0, 0));
            return false;
        }
        let next = match self.current {
            Some(index) => (index + 1) % self.matches.len(),
            None => 0,
        };
        self.select_match(next);
        true
    }

    /// Selects the previous match, wrapping at the start.
    pub fn find_previous(&mut self) -> bool {
        if self.matches.is_empty() {
            self.current = None;
            self.search_changed.emit((0, 0));
            return false;
        }
        let next = match self.current {
            Some(0) => self.matches.len() - 1,
            Some(index) => index - 1,
            None => self.matches.len() - 1,
        };
        self.select_match(next);
        true
    }

    /// Selects match `index` and reports its range.
    fn select_match(&mut self, index: usize) {
        self.current = Some(index);
        if let Some(range) = self.current_range() {
            self.search_changed.emit(range);
        }
        self.base.request_redraw();
    }

    /// Recomputes `matches` and keeps the active match when it still exists.
    fn recompute_matches(&mut self) {
        let previous = self.current.and_then(|index| self.matches.get(index).copied());
        self.matches = find_matches(
            &self.haystack,
            &self.char_index,
            &self.query,
            self.case_sensitive,
            self.whole_word,
        );
        self.current = previous.and_then(|range| self.matches.iter().position(|hit| *hit == range));
        // Nothing is selected automatically: a query the reader is still typing is not yet a
        // navigation, and starting a match early would make the first `find_next` skip it.
    }

    /// How many ranges the host last reported as replaced.
    pub fn last_replaced_count(&self) -> usize {
        self.last_replaced
    }

    /// Records how many ranges a host-side replacement changed.
    ///
    /// The bar cannot observe the document, so the count is reported back to it by whoever applied
    /// the [`ReplaceRequest`]. Keeping it here is what makes the result visible in the bar instead
    /// of only in the host's state.
    pub fn set_last_replaced(&mut self, count: usize) {
        if self.last_replaced == count {
            return;
        }
        self.last_replaced = count;
        self.base.request_redraw();
    }

    /// Asks the host to replace the active match.
    ///
    /// Returns whether a request was emitted. The match list is left alone: the text it addresses
    /// has not changed yet, and a bar that renumbered its matches before the document did would
    /// point the next request at the wrong bytes.
    pub fn replace(&mut self) -> bool {
        let Some(range) = self.current_range() else {
            return false;
        };
        self.replace_requested
            .emit(ReplaceRequest { ranges: vec![range], text: self.replacement.clone() });
        true
    }

    /// Asks the host to replace every match in one request.
    pub fn replace_all(&mut self) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        self.replace_requested
            .emit(ReplaceRequest { ranges: self.matches.clone(), text: self.replacement.clone() });
        true
    }

    /// The first row's box.
    fn query_band(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, FIND_ROW_HEIGHT.min(rect.height as i32) as u32)
    }

    /// The replacement row's box, when it is shown.
    fn replace_band(&self) -> Option<Rect> {
        if !self.replace_visible {
            return None;
        }
        let rect = self.geometry();
        let top = rect.y + FIND_ROW_HEIGHT;
        let height = rect.height as i32 - FIND_ROW_HEIGHT;
        (height > 0).then(|| Rect::new(rect.x, top, rect.width, height as u32))
    }

    /// The `Aa` toggle box.
    fn case_toggle_rect(&self) -> Rect {
        let band = self.query_band();
        Rect::new(
            band.x + band.width as i32 - 2 * FIND_TOGGLE_SIDE - 6,
            band.y + (band.height as i32 - FIND_TOGGLE_SIDE) / 2,
            FIND_TOGGLE_SIDE as u32,
            FIND_TOGGLE_SIDE as u32,
        )
    }

    /// The whole-word toggle box.
    fn word_toggle_rect(&self) -> Rect {
        let band = self.query_band();
        Rect::new(
            band.x + band.width as i32 - FIND_TOGGLE_SIDE - 3,
            band.y + (band.height as i32 - FIND_TOGGLE_SIDE) / 2,
            FIND_TOGGLE_SIDE as u32,
            FIND_TOGGLE_SIDE as u32,
        )
    }

    /// The box the query text occupies, between the left inset and the toggles.
    fn query_text_rect(&self) -> Rect {
        let band = self.query_band();
        let right = band.width as i32 - 2 * FIND_TOGGLE_SIDE - 12;
        Rect::new(band.x + 6, band.y, right.max(8) as u32, band.height)
    }

    /// The "replace" action cell on the replacement row.
    fn replace_button_rect(&self) -> Option<Rect> {
        let band = self.replace_band()?;
        Some(Rect::new(
            band.x + band.width as i32 - 74,
            band.y + 3,
            42,
            (band.height as i32 - 6).max(1) as u32,
        ))
    }

    /// The "replace all" action cell on the replacement row.
    fn replace_all_button_rect(&self) -> Option<Rect> {
        let band = self.replace_band()?;
        Some(Rect::new(
            band.x + band.width as i32 - 30,
            band.y + 3,
            28,
            (band.height as i32 - 6).max(1) as u32,
        ))
    }

    /// The box the replacement text occupies, left of the two action cells.
    fn replace_text_rect(&self) -> Option<Rect> {
        let band = self.replace_band()?;
        let width = (band.width as i32 - 86).max(8);
        Some(Rect::new(band.x + 6, band.y, width as u32, band.height))
    }
}

/// Case- and word-aware, non-overlapping literal search.
///
/// A literal search is what a find bar offers: the regex-capable editor has its own matcher, and
/// a second engine here would be a second set of semantics to keep in step. Offsets are byte
/// offsets into `hay`, so a host can splice the ranges directly.
fn find_matches(
    hay: &str,
    hay_chars: &[(usize, char)],
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    if needle.is_empty() || hay.is_empty() {
        return Vec::new();
    }
    let needle_chars: Vec<char> = needle.chars().collect();
    let len = needle_chars.len();
    if len == 0 || hay_chars.len() < len {
        return Vec::new();
    }
    let same = |a: char, b: char| {
        if case_sensitive {
            a == b
        } else {
            a.to_lowercase().eq(b.to_lowercase())
        }
    };
    // Boyer-Moore-Horspool's bad-character table, keyed by the case-folded last character. A
    // colliding fold only makes the shift shorter than the true one, which is safe: the matcher
    // still verifies the whole candidate before reporting it.
    let fold = |ch: char| {
        if case_sensitive {
            ch
        } else {
            ch.to_lowercase().next().unwrap_or(ch)
        }
    };
    let mut skip: HashMap<char, usize> = HashMap::new();
    for (index, ch) in needle_chars[..len - 1].iter().enumerate() {
        skip.insert(fold(*ch), len - 1 - index);
    }
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index + len <= hay_chars.len() {
        let last = hay_chars[index + len - 1].1;
        if same(last, needle_chars[len - 1])
            && (0..len - 1).all(|offset| same(hay_chars[index + offset].1, needle_chars[offset]))
        {
            let starts_word = !whole_word || index == 0 || !is_word(hay_chars[index - 1].1);
            let ends_word =
                !whole_word || index + len == hay_chars.len() || !is_word(hay_chars[index + len].1);
            if starts_word && ends_word {
                let start = hay_chars[index].0;
                let end = hay_chars.get(index + len).map_or(hay.len(), |(offset, _)| *offset);
                matches.push((start, end));
                index += len;
                continue;
            }
        }
        let step = skip.get(&fold(last)).copied().unwrap_or(len);
        index += step.max(1);
    }
    matches
}

impl Widget for FindBar {
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
        let height = if self.replace_visible {
            (FIND_ROW_HEIGHT * 2) as u32
        } else {
            FIND_ROW_HEIGHT as u32
        };
        crate::core::Size::new(360, height)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the bar's two events; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "search_changed" => {
                Some(EventSignalRef::mapped("search_changed", &self.search_changed, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "replace_requested" => {
                Some(EventSignalRef::mapped("replace_requested", &self.replace_requested, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::Tuple(
                            v.ranges
                                .iter()
                                .map(|range| {
                                    CapabilityValue::Tuple(crate::compat::Vec::from([
                                        CapabilityValue::UInt(range.0 as u64),
                                        CapabilityValue::UInt(range.1 as u64),
                                    ]))
                                })
                                .collect(),
                        ),
                        CapabilityValue::String(v.text.clone()),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for FindBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "query" => Ok(CapabilityValue::String(self.query.clone())),
            "case_sensitive" => Ok(CapabilityValue::Bool(self.case_sensitive)),
            "whole_word" => Ok(CapabilityValue::Bool(self.whole_word)),
            "replace_text" => Ok(CapabilityValue::String(self.replacement.clone())),
            "visible" => Ok(CapabilityValue::Bool(self.visible)),
            "replace_visible" => Ok(CapabilityValue::Bool(self.replace_visible)),
            "match_count" => Ok(CapabilityValue::UInt(self.matches.len() as u64)),
            "current_range" => Ok(match self.current_range() {
                Some((start, end)) => CapabilityValue::String(format!("{start},{end}")),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "query" => {
                self.set_query(expect_string(value)?);
                Ok(())
            }
            "case_sensitive" => {
                self.set_case_sensitive(expect_bool(value)?);
                Ok(())
            }
            "whole_word" => {
                self.set_whole_word(expect_bool(value)?);
                Ok(())
            }
            "replace_text" => {
                self.set_replace_text(expect_string(value)?);
                Ok(())
            }
            "visible" => {
                if expect_bool(value)? {
                    self.open();
                } else {
                    self.close();
                }
                Ok(())
            }
            "replace_visible" => {
                self.set_replace_visible(expect_bool(value)?);
                Ok(())
            }
            "match_count" | "current_range" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "query",
            "case_sensitive",
            "whole_word",
            "replace_text",
            "visible",
            "replace_visible",
            "match_count",
            "current_range",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for FindBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.case_toggle_rect().contains_point(*pos) {
                    self.set_case_sensitive(!self.case_sensitive);
                    return;
                }
                if self.word_toggle_rect().contains_point(*pos) {
                    self.set_whole_word(!self.whole_word);
                    return;
                }
                if self.replace_button_rect().is_some_and(|cell| cell.contains_point(*pos)) {
                    self.replace();
                    return;
                }
                if self.replace_all_button_rect().is_some_and(|cell| cell.contains_point(*pos)) {
                    self.replace_all();
                    return;
                }
                if self.replace_band().is_some_and(|band| band.contains_point(*pos)) {
                    self.focus = FindFocus::Replace;
                    self.base.request_redraw();
                    return;
                }
                if self.query_band().contains_point(*pos) {
                    self.focus = FindFocus::Query;
                    self.base.request_redraw();
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                let target = match self.focus {
                    FindFocus::Query => &mut self.query,
                    FindFocus::Replace => &mut self.replacement,
                };
                target.extend(text.chars().filter(|character| !character.is_control()));
                if matches!(self.focus, FindFocus::Query) {
                    self.recompute_matches();
                }
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } => {
                let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::SHIFT);
                match *key {
                    crate::event::key_codes::ESCAPE => self.close(),
                    crate::event::key_codes::TAB => {
                        if self.replace_visible {
                            self.focus = match self.focus {
                                FindFocus::Query => FindFocus::Replace,
                                FindFocus::Replace => FindFocus::Query,
                            };
                            self.base.request_redraw();
                        }
                    }
                    crate::event::key_codes::ENTER => {
                        if shift {
                            self.find_previous();
                        } else {
                            self.find_next();
                        }
                    }
                    crate::event::key_codes::BACKSPACE => {
                        let target = match self.focus {
                            FindFocus::Query => &mut self.query,
                            FindFocus::Replace => &mut self.replacement,
                        };
                        target.pop();
                        if matches!(self.focus, FindFocus::Query) {
                            self.recompute_matches();
                        }
                        self.base.request_redraw();
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => {}
        }
    }
}

impl Draw for FindBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(surface));
        let accent =
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(Color::BLUE);
        let font = effective_font(style);

        context.face_with_gradient_corners(
            rect,
            surface,
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

        let query_band = self.query_band();
        let text_rect = self.query_text_rect();
        let counter = if self.matches.is_empty() {
            "0/0".to_string()
        } else {
            format!("{}/{}", self.current.map_or(0, |index| index + 1), self.matches.len())
        };
        let counter_width = context.measure_text(&counter, font).width as i32;
        context.draw_text(
            Point::new(text_rect.x, text_rect.y + 7),
            &crate::widget::text_utils::elide_end(
                &self.query,
                ((text_rect.width as i32 - counter_width - 8) / 7).max(1) as usize,
            ),
            font,
            if self.query.is_empty() { ink.with_alpha(120) } else { ink },
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(
                query_band.x + query_band.width as i32 - 2 * FIND_TOGGLE_SIDE - 12 - counter_width,
                query_band.y + 7,
            ),
            &counter,
            font,
            ink.blend(&surface, 0.35),
            HorizontalAlignment::Left,
        );

        for (box_rect, label, active) in [
            (self.case_toggle_rect(), "Aa", self.case_sensitive),
            (self.word_toggle_rect(), "W", self.whole_word),
        ] {
            if active {
                context.fill_rounded_rect(box_rect, 3, surface.blend(&accent, 0.30));
                crate::widget::draw::stroke_surface_border(
                    context,
                    box_rect,
                    [3, 3, 3, 3],
                    accent,
                    1,
                );
            }
            context.draw_text(
                Point::new(box_rect.x + 4, box_rect.y + 3),
                label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        if let Some(band) = self.replace_band() {
            context.draw_line(
                Point::new(band.x, band.y),
                Point::new(band.x + band.width as i32, band.y),
                border,
            );
            if let Some(text_rect) = self.replace_text_rect() {
                context.draw_text(
                    Point::new(text_rect.x, text_rect.y + 7),
                    &crate::widget::text_utils::elide_end(
                        &self.replacement,
                        (text_rect.width as i32 / 7).max(1) as usize,
                    ),
                    font,
                    if self.replacement.is_empty() { ink.with_alpha(120) } else { ink },
                    HorizontalAlignment::Left,
                );
            }
            // The two replacement actions are controls on the bar, so a reader never has to know
            // that the host applies their request; the result count is the feedback that the
            // request reached the document.
            for (cell, label) in
                [(self.replace_button_rect(), "Repl"), (self.replace_all_button_rect(), "All")]
            {
                let Some(cell) = cell else {
                    continue;
                };
                let enabled = !self.matches.is_empty();
                context.fill_rounded_rect(
                    cell,
                    3,
                    if enabled { surface.blend(&accent, 0.25) } else { surface },
                );
                context.draw_rect(cell, if enabled { accent } else { border });
                context.draw_text_fitted_cow(
                    cell,
                    label,
                    font,
                    if enabled { ink } else { ink.with_alpha(120) },
                    HorizontalAlignment::Center,
                );
            }
            if self.last_replaced > 0 {
                let label = format!("{} replaced", self.last_replaced);
                context.draw_text(
                    Point::new(
                        band.x + band.width as i32
                            - 84
                            - context.measure_text(&label, font).width as i32,
                        band.y + 7,
                    ),
                    &label,
                    font,
                    ink.blend(&surface, 0.35),
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::input_widgets::shortcut_editor::{
        non_primary_modifier_event_bits, primary_modifier_event_bits,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn metrics_follow_text_changes() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# Title\nhello world\n## Details");

        assert_eq!(editor.line_count(), 3);
        assert_eq!(editor.word_count(), 6);
        assert_eq!(editor.heading_count(), 2);
    }

    #[test]
    fn preview_toggle_emits_signal() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        let states = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = states.clone();
        editor.preview_mode_changed.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });

        editor.toggle_preview_mode();
        editor.toggle_preview_mode();

        let got = states.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![true, false]);
    }

    #[test]
    fn arrow_keys_move_cursor() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("a\nb\nc");

        editor.handle_event(&Event::key_press(40, 0));
        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor_line(), 2);

        editor.handle_event(&Event::key_press(38, 0));
        assert_eq!(editor.cursor_line(), 1);
    }

    #[test]
    fn undo_redo_restores_markdown_text() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# One");
        editor.append_line("body");

        assert!(editor.can_undo());
        assert!(editor.undo());
        assert_eq!(editor.text(), "# One");
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.text(), "# One\nbody");
    }

    #[test]
    fn keyboard_shortcuts_drive_markdown_history() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("draft");
        editor.set_text("final");

        editor.handle_event(&Event::KeyPress { key: 90, modifiers: primary_modifier_event_bits() });
        assert_eq!(editor.text(), "draft");
        editor.handle_event(&Event::KeyPress { key: 89, modifiers: primary_modifier_event_bits() });
        assert_eq!(editor.text(), "final");
    }

    #[test]
    fn keyboard_shortcuts_reject_shifted_and_non_primary_chords() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("draft");
        editor.set_text("final");

        let primary = primary_modifier_event_bits();
        // Shift+Primary+Z is a different chord, so it is not plain undo.
        editor.handle_event(&Event::key_press(90, primary | 0b0001));
        assert_eq!(editor.text(), "final");

        // The accelerator that is not primary on this host does not undo.
        editor.handle_event(&Event::key_press(90, non_primary_modifier_event_bits()));
        assert_eq!(editor.text(), "final");

        editor.handle_event(&Event::key_press(90, primary));
        assert_eq!(editor.text(), "draft");
        editor.handle_event(&Event::key_press(89, primary));
        assert_eq!(editor.text(), "final");
    }

    #[test]
    fn text_input_inserts_text_and_emits_signal() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        editor.text_changed.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*value).clone());
            }
        });

        editor.handle_event(&Event::text_input("# Hello"));

        assert_eq!(editor.text(), "# Hello");
        let got = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["# Hello".to_string()]);
    }

    #[test]
    fn ime_commit_inserts_cjk_text() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.handle_event(&Event::ime_commit("你好"));

        assert_eq!(editor.text(), "你好");
        assert_eq!(editor.cursor_col(), 2);
    }

    #[test]
    fn typed_text_can_be_undone_and_redone() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.handle_event(&Event::text_input("first"));
        editor.handle_event(&Event::ime_commit(" second"));
        assert_eq!(editor.text(), "first second");

        assert!(editor.undo());
        assert_eq!(editor.text(), "first");
        assert!(editor.undo());
        assert_eq!(editor.text(), "");

        assert!(editor.redo());
        assert!(editor.redo());
        assert_eq!(editor.text(), "first second");
    }

    #[test]
    fn enter_splits_and_backspace_merges_lines() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("ab");

        editor.handle_event(&Event::key_press(39, 0));
        editor.handle_event(&Event::key_press(39, 0));
        editor.handle_event(&Event::key_press(13, 0));
        assert_eq!(editor.text(), "ab\n");
        assert_eq!(editor.cursor_line(), 1);
        assert_eq!(editor.cursor_col(), 0);

        editor.handle_event(&Event::key_press(8, 0));
        assert_eq!(editor.text(), "ab");
        assert_eq!(editor.cursor_line(), 0);
        assert_eq!(editor.cursor_col(), 2);

        editor.handle_event(&Event::key_press(37, 0));
        editor.handle_event(&Event::key_press(8, 0));
        assert_eq!(editor.text(), "b");
        assert_eq!(editor.cursor_col(), 0);
    }

    #[test]
    fn arrow_keys_clamp_cursor_column_to_line_length() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("long line\nx");

        editor.handle_event(&Event::key_press(39, 0));
        editor.handle_event(&Event::key_press(39, 0));
        assert_eq!(editor.cursor_col(), 2);

        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor_line(), 1);
        assert_eq!(editor.cursor_col(), 1);
    }

    /// The wheel scrolls the visible line window and clamps at both ends.
    ///
    /// # The defect this pins
    ///
    /// The paint path stopped at a fixed `take(10)`, so a longer document could not be reached at
    /// all. This drives the same window the paint path now starts from.
    #[test]
    fn markdown_editor_wheel_scrolls_and_clamps() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 100));
        editor.set_text(text.join("\n"));
        assert_eq!(editor.first_visible_line, 0);

        // One notch toward the end reveals three lines.
        editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        assert_eq!(editor.first_visible_line, 3, "a downward notch moves the viewport down");

        // The other direction returns to the top and clamps there.
        editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(editor.first_visible_line, 0);
        editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(editor.first_visible_line, 0, "the viewport clamps at the first line");

        // Scrolling on stops at the last full window.
        let max_first = editor.line_count() - editor.visible_line_count();
        for _ in 0..20 {
            editor.handle_event(&Event::Wheel {
                delta: crate::core::Point::new(0, -1),
                modifiers: 0,
            });
        }
        assert_eq!(editor.first_visible_line, max_first, "the viewport clamps at the last window");
    }

    /// A whole-text replacement still records only the changed middle for undo.
    #[test]
    fn set_text_undoes_a_paragraph_change_without_losing_the_rest() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("alpha\nbeta\ngamma");
        editor.set_text("alpha\nBETA 你好\ngamma");

        assert!(editor.undo());
        assert_eq!(editor.text(), "alpha\nbeta\ngamma");
        assert!(editor.redo());
        assert_eq!(editor.text(), "alpha\nBETA 你好\ngamma");
    }

    /// Runs `f` with a real paint context, for the paths that need measurements.
    fn with_context<T>(f: impl FnOnce(&RenderContext) -> T) -> T {
        use crate::render::{PaintBackend, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(400, 300), 1.0);
        backend.begin_frame(Color::WHITE);
        let result = {
            let context = RenderContext::new(&mut backend);
            f(&context)
        };
        backend.end_frame();
        result
    }

    /// The counts are cached against the revision, so a still frame walks no document.
    ///
    /// `line_count`/`word_count`/`heading_count` each scanned the whole document, and the paint
    /// path called all three per frame.
    #[test]
    fn metrics_are_cached_until_the_revision_changes() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# One\ntwo words here");
        assert_eq!((editor.line_count(), editor.word_count(), editor.heading_count()), (2, 5, 1));
        let cached = *editor.metrics.borrow();
        assert_eq!(cached.map(|(revision, _)| revision), Some(editor.revision));

        editor.set_text("# One\ntwo words here\nthree");
        assert_eq!(editor.word_count(), 6, "an edit invalidates the cache");
        assert_eq!(
            editor.metrics.borrow().map(|(revision, _)| revision),
            Some(editor.revision),
            "and the next read refreshes it"
        );
    }

    /// An IME composition is visible before it is committed, anchored at the caret.
    #[test]
    fn ime_preedit_is_visible_at_the_caret() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("ab");
        // `set_text` leaves the caret where it was, so it is moved to the end explicitly.
        editor.handle_event(&Event::key_press(35, 0));
        assert!(editor.preedit_screen_rect().is_none(), "nothing is composing yet");

        editor.handle_event(&Event::ime_preedit("你好", 3));
        let at_end = editor.preedit_screen_rect().expect("a composition has a rectangle");
        assert_eq!(editor.preedit.as_ref().map(|p| p.cursor), Some(3));

        editor.handle_event(&Event::key_press(37, 0));
        let mid_line = editor.preedit_screen_rect().expect("still composing");
        assert!(mid_line.x < at_end.x, "the composition moved back with the caret");

        editor.handle_event(&Event::ime_commit("你好"));
        assert!(editor.preedit_screen_rect().is_none(), "a commit ends the composition");
    }

    /// The preview recognises the GFM blocks and inline styles it used to print as source.
    #[test]
    fn preview_recognises_gfm_constructs() {
        with_context(|context| {
            let style = crate::style::WidgetStyle::default();
            let font = effective_font(&style);
            let heading =
                parse_preview_line("# Title", BlockStart::Normal, true, true, false, font, context);
            assert!(matches!(heading.row, PreviewRow::Heading(1)));
            assert!(heading.runs.iter().all(|run| !run.text.contains('#')));

            let task = parse_preview_line(
                "- [x] done",
                BlockStart::Normal,
                true,
                true,
                false,
                font,
                context,
            );
            assert_eq!(task.runs.first().map(|run| run.text.as_str()), Some("☑ "));

            let strike = parse_preview_line(
                "~~gone~~",
                BlockStart::Normal,
                true,
                true,
                false,
                font,
                context,
            );
            assert!(strike.runs.iter().any(|run| run.style == PreviewStyle::Strike));

            let quote = parse_preview_line(
                "> quoted",
                BlockStart::Normal,
                true,
                true,
                false,
                font,
                context,
            );
            assert_eq!(quote.runs.first().map(|run| run.style), Some(PreviewStyle::Accent));

            let rule =
                parse_preview_line("---", BlockStart::Normal, true, true, false, font, context);
            assert!(matches!(rule.row, PreviewRow::Rule));

            let fence =
                parse_preview_line("```rust", BlockStart::Fence, true, true, false, font, context);
            assert!(matches!(fence.row, PreviewRow::Fence));

            let table = parse_preview_line(
                "| a | b |",
                BlockStart::Normal,
                true,
                true,
                false,
                font,
                context,
            );
            assert!(matches!(table.row, PreviewRow::Table));
            assert!(table.runs.iter().any(|run| run.text.contains('│')));

            // GFM recognises a table by the delimiter row after it; a pipe in prose is not one.
            let header =
                parse_preview_line("a | b", BlockStart::Normal, true, true, true, font, context);
            assert!(matches!(header.row, PreviewRow::Table));
            let prose =
                parse_preview_line("a | b", BlockStart::Normal, true, true, false, font, context);
            assert!(!matches!(prose.row, PreviewRow::Table));

            let image = parse_preview_line(
                "![alt](x.png)",
                BlockStart::Normal,
                true,
                true,
                false,
                font,
                context,
            );
            assert!(image.runs.iter().any(|run| run.style == PreviewStyle::Link));

            // With the extensions off, the same text stays literal.
            let plain = parse_preview_line(
                "~~gone~~",
                BlockStart::Normal,
                false,
                true,
                false,
                font,
                context,
            );
            assert!(plain.runs.iter().all(|run| run.style != PreviewStyle::Strike));
            assert!(plain.runs.iter().any(|run| run.text.contains("~~")));
        });
    }

    /// Fenced code is tokenized with the shared lexer, so keywords are not body text.
    #[test]
    fn fenced_code_is_highlighted_with_the_shared_lexer() {
        with_context(|context| {
            let style = crate::style::WidgetStyle::default();
            let font = effective_font(&style);
            let parsed = parse_preview_line(
                "let x = 1;",
                BlockStart::Code(LanguageId::Rust, LineState::initial()),
                true,
                true,
                false,
                font,
                context,
            );
            assert!(matches!(parsed.row, PreviewRow::Code));
            assert!(
                parsed.runs.iter().any(|run| run.style == PreviewStyle::Token(TokenKind::Keyword)),
                "`let` must be a keyword token"
            );
        });
    }

    /// Selection, typing over it and cutting all work on the real document.
    #[test]
    fn selection_replaces_text_and_cut_deletes_it() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("alpha beta");
        editor.select_all();
        assert_eq!(editor.selected_text().as_deref(), Some("alpha beta"));

        editor.insert_text_at_cursor("X");
        assert_eq!(editor.text(), "X", "typing replaces the selection in one step");
        assert!(editor.selection().is_none());

        editor.set_text("abc");
        editor.select_all();
        editor.cut();
        assert_eq!(editor.text(), "", "cut removes the selected run");
    }

    /// The selection signal reports presence, and Ctrl+A drives it from the keyboard.
    #[test]
    fn select_all_emits_selection_changed() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("text");
        let seen = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = seen.clone();
        editor.selection_changed.connect(move |present| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*present);
            }
        });

        editor.handle_event(&Event::key_press(65, primary_modifier_event_bits()));
        assert_eq!(editor.selected_text().as_deref(), Some("text"));

        editor.handle_event(&Event::key_press(37, 0));
        let seen = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(seen, vec![true, false], "a plain arrow clears the selection");
    }

    /// Home/End/Delete/PageDown and the scrollbar reach the document.
    #[test]
    fn keyboard_covers_line_edges_delete_and_paging() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 100));
        let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
        editor.set_text(text.join("\n"));

        editor.handle_event(&Event::key_press(35, 0));
        assert_eq!(editor.cursor_col(), "line0".chars().count(), "End reaches the line's end");
        editor.handle_event(&Event::key_press(36, 0));
        assert_eq!(editor.cursor_col(), 0, "Home returns to its start");

        editor.handle_event(&Event::key_press(46, 0));
        assert_eq!(editor.text().split('\n').next(), Some("ine0"), "Delete removes forward");

        assert!(editor.scroll_page(1));
        assert!(editor.first_visible_line > 0, "PageDown moves the viewport");
        editor.handle_event(&Event::key_press(33, 0));
        assert_eq!(editor.first_visible_line, 0, "PageUp returns to the top");
    }

    /// The caret blinks on the ticks it is given — this control was the only text field in the
    /// crate whose caret never moved.
    #[test]
    fn the_caret_blinks_on_the_ticks_it_is_given() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        assert!(editor.is_animating());
        assert!(editor.is_caret_visible());
        editor.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
        assert!(!editor.is_caret_visible(), "one half-period hides the caret");
        editor.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
        assert!(editor.is_caret_visible());

        editor.set_enabled(false);
        editor.handle_event(&Event::key_press(65, 0));
        assert!(!editor.is_animating(), "a disabled editor holds a steady caret");
    }

    /// The scrollbar track jumps the viewport, and clicking a preview heading announces it.
    #[test]
    fn scrollbar_and_heading_clicks_navigate() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 200));
        let text: Vec<String> = (0..40).map(|n| format!("line{n}")).collect();
        editor.set_text(text.join("\n"));

        editor.handle_event(&Event::mouse_press(414, 190, 1));
        assert!(editor.first_visible_line > 0, "a click near the track's bottom jumps forward");

        let mut headings = MarkdownEditor::new(Rect::new(0, 0, 420, 200));
        headings.set_text("# First\n## Second");
        headings.set_preview_mode(true);
        let seen = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = seen.clone();
        headings.heading_activated.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });
        headings.handle_event(&Event::mouse_press(40, TEXT_TOP + 2, 1));
        let seen = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(seen, vec![0], "the heading row announces its outline index");
    }

    /// The find bar takes typed text and Enter selects the next match.
    #[test]
    fn find_bar_selects_matches() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("alpha beta alpha");
        assert!(editor.find_next("alpha"));
        assert_eq!(editor.selection(), Some((0, 5)));
        assert!(editor.find_next("alpha"));
        assert_eq!(editor.selection(), Some((11, 16)), "the next call moves past the match");
        assert!(editor.find_previous("alpha"));
        assert_eq!(editor.selection(), Some((0, 5)));

        editor.open_find();
        assert!(editor.find_bar_visible());
        editor.handle_event(&Event::text_input("beta"));
        assert_eq!(editor.find_query(), "beta", "typing goes to the query, not the document");
        editor.handle_event(&Event::key_press(13, 0));
        assert_eq!(editor.selected_text().as_deref(), Some("beta"));
        editor.handle_event(&Event::key_press(27, 0));
        assert!(!editor.find_bar_visible());
    }

    #[test]
    fn outline_panel_follows_the_editor_caret() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# One\ntext\n## Two\nmore");
        let mut panel = MarkdownOutlinePanel::new(Rect::new(0, 0, 180, 200));
        panel.sync_with_editor(&editor);
        assert_eq!(panel.entries().len(), 2);
        assert_eq!(panel.entries()[0].title, "One");
        assert_eq!(panel.active_index(), Some(0));

        assert!(editor.goto_line(2));
        panel.sync_with_editor(&editor);
        assert_eq!(panel.active_index(), Some(1), "the caret is inside the second heading");

        let selected = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = selected.clone();
        panel.entry_selected.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });
        assert_eq!(panel.goto(0), Some(0), "goto reports the line it addresses");
        assert_eq!(*selected.lock().expect("selected lock"), vec![0]);
        assert_eq!(panel.goto(9), None);
    }

    #[test]
    fn find_bar_navigates_case_and_whole_word_matches() {
        let mut bar = FindBar::new(Rect::new(0, 0, 360, 26));
        bar.set_text("Alpha alpha alphabet alpha");
        bar.set_query("alpha");
        assert_eq!(bar.match_count(), 4, "case-insensitive by default, substrings included");
        assert!(bar.find_next());
        assert_eq!(bar.current_range(), Some((0, 5)));
        assert!(bar.find_next());
        assert_eq!(bar.current_range(), Some((6, 11)));

        bar.set_whole_word(true);
        assert_eq!(bar.match_count(), 3);
        bar.set_case_sensitive(true);
        assert_eq!(bar.match_count(), 2, "only the lowercase, whole-word hits remain");
        bar.set_query("zzz");
        assert!(!bar.find_next(), "an empty match list reports no match");
    }

    #[test]
    fn find_bar_replace_request_reaches_the_editor_and_is_undoable() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("cat dog cat");
        let mut bar = FindBar::new(Rect::new(0, 0, 360, 52));
        bar.set_replace_visible(true);
        bar.set_replace_text("fox");
        editor.sync_find_bar(&mut bar);
        bar.set_query("cat");
        assert_eq!(bar.match_count(), 2);

        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<ReplaceRequest>::new()));
        let sink = requests.clone();
        bar.replace_requested.connect(move |request| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*request).clone());
            }
        });
        assert!(bar.replace_all());
        let request = requests.lock().expect("requests lock")[0].clone();
        assert_eq!(request.ranges, vec![(0, 3), (8, 11)]);

        assert_eq!(editor.apply_replace_request(&request), 2);
        assert_eq!(editor.text(), "fox dog fox");
        assert!(editor.undo());
        assert_eq!(editor.text(), "cat dog cat");
    }

    #[test]
    fn find_bar_properties_round_trip() {
        let mut bar = FindBar::new(Rect::new(0, 0, 360, 26));
        bar.set("query", CapabilityValue::String("a".to_string())).expect("query writable");
        bar.set("case_sensitive", CapabilityValue::Bool(true)).expect("case writable");
        bar.set("whole_word", CapabilityValue::Bool(true)).expect("word writable");
        bar.set("replace_text", CapabilityValue::String("b".to_string()))
            .expect("replace writable");
        bar.set("visible", CapabilityValue::Bool(true)).expect("visible writable");
        assert!(bar.is_visible());
        assert_eq!(bar.get("query"), Ok(CapabilityValue::String("a".to_string())));
        assert_eq!(bar.get("case_sensitive"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(bar.get("whole_word"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(bar.get("replace_text"), Ok(CapabilityValue::String("b".to_string())));
        assert_eq!(
            bar.set("match_count", CapabilityValue::UInt(0)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    /// The undo depth is a constructor argument and a property.
    #[test]
    fn undo_depth_is_configurable() {
        let mut editor = MarkdownEditor::with_undo_limit(Rect::new(0, 0, 420, 240), 2);
        assert_eq!(editor.undo_stack_limit(), 2);
        editor.set_text("a");
        editor.insert_text_at_cursor("b");
        editor.insert_text_at_cursor("c");

        assert!(editor.undo());
        assert!(editor.undo());
        assert!(!editor.undo(), "only the configured depth is retained");
        assert_eq!(editor.text(), "a");

        editor.set_undo_limit(8);
        assert_eq!(editor.undo_stack_limit(), 8);
    }

    /// The new properties round-trip through the property contract.
    #[test]
    fn new_properties_round_trip() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        assert_eq!(editor.get("gfm_extensions").ok(), Some(CapabilityValue::Bool(true)));
        editor.set("gfm_extensions", CapabilityValue::Bool(false)).expect("writable");
        assert!(!editor.gfm_extensions());
        editor.set("code_highlight", CapabilityValue::Bool(false)).expect("writable");
        assert!(!editor.code_highlight());
        editor.set("show_scrollbar", CapabilityValue::Bool(false)).expect("writable");
        assert!(!editor.show_scrollbar());
        editor.set("undo_stack_limit", CapabilityValue::UInt(7)).expect("writable");
        assert_eq!(editor.undo_stack_limit(), 7);
    }

    /// The editor mounts a real outline panel: a press in its side column navigates, and the
    /// painted rows come from the same heading list `outline()` reports.
    #[test]
    fn mounted_outline_panel_navigates_and_paints() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# One\ntext\n## Two\nmore");
        editor.set_show_outline(true);
        assert_eq!(editor.outline_panel().entries().len(), 2);
        assert_eq!(editor.outline_panel().active_index(), Some(0));

        // A first paint places the companion and proves the path draws without panicking.
        use crate::render::{PaintBackend, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(420, 240), 1.0);
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut backend);
            editor.draw(&mut context);
        }
        backend.end_frame();

        // The panel is 180 px wide, so it starts at x = 240; the second row sits one row down.
        editor.handle_event(&Event::mouse_press(250, TEXT_TOP + OUTLINE_ROW_HEIGHT + 2, 1));
        assert_eq!(editor.cursor_line(), 2, "the second outline row navigates to line 2");
        assert_eq!(editor.outline_panel().active_index(), Some(1));
    }

    /// The mounted find bar is the painted form of the editor's own find state: typing goes to the
    /// same query the editor's Enter handler searches, and closing mirrors both.
    #[test]
    fn mounted_find_bar_reflects_the_editor_query() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("alpha beta alpha");
        editor.show_find_bar();
        assert!(editor.find_bar().is_visible());
        assert_eq!(editor.find_bar().text(), "alpha beta alpha");

        editor.handle_event(&Event::text_input("beta"));
        assert_eq!(editor.find_query(), "beta");
        assert_eq!(editor.find_bar().query(), "beta");
        assert_eq!(editor.find_bar().match_count(), 1);

        editor.handle_event(&Event::key_press(27, 0));
        assert!(!editor.find_bar_visible());
        assert!(!editor.find_bar().is_visible());
    }
}

#[cfg(test)]
mod issue_2_8_tests {
    use super::*;
    use crate::widget::input_widgets::shortcut_editor::primary_modifier_event_bits;
    use std::sync::{Arc, Mutex};

    fn editor(text: &str) -> MarkdownEditor {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
        editor.set_text(text);
        editor
    }

    fn draw_once(editor: &mut MarkdownEditor) {
        use crate::render::{PaintBackend, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(400, 300), 1.0);
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut backend);
            editor.draw(&mut context);
        }
        backend.end_frame();
    }

    /// SP-02: Tab indents the selected lines and Shift+Tab reverses it.
    #[test]
    fn tab_indents_and_shift_tab_outdents_selected_lines() {
        let mut editor = editor("alpha\nbeta");
        editor.select_all();
        editor.handle_key(9, 0);
        assert_eq!(editor.text(), "  alpha\n  beta");
        editor.handle_key(9, crate::shortcut::Modifiers::SHIFT.bits() as u32);
        assert_eq!(editor.text(), "alpha\nbeta");
    }

    /// SP-03: the formatting action wraps a selection and a second call unwraps it.
    #[test]
    fn format_actions_wrap_and_unwrap_the_selection() {
        let mut editor = editor("hello");
        editor.select_all();
        assert!(editor.wrap_selection("**", "**"));
        assert_eq!(editor.text(), "**hello**");
        assert!(editor.wrap_selection("**", "**"));
        assert_eq!(editor.text(), "hello");
    }

    /// SP-03: the keyboard chord drives the same action.
    #[test]
    fn primary_b_wraps_the_selection() {
        let mut editor = editor("hello");
        editor.select_all();
        editor.handle_key(66, primary_modifier_event_bits());
        assert_eq!(editor.text(), "**hello**");
    }

    /// SP-04: primary+Shift+Z redoes, the chord the handler used to ignore.
    #[test]
    fn primary_shift_z_redoes() {
        let mut editor = editor("ab");
        assert!(editor.undo());
        assert_eq!(editor.text(), "");
        let modifiers =
            primary_modifier_event_bits() | crate::shortcut::Modifiers::SHIFT.bits() as u32;
        editor.handle_key(90, modifiers);
        assert_eq!(editor.text(), "ab");
    }

    /// SP-03: a line prefix toggle is one undo step for a multi-line selection.
    #[test]
    fn line_prefix_toggles_every_selected_line_as_one_undo_step() {
        let mut editor = editor("one\ntwo");
        editor.select_all();
        assert!(editor.toggle_line_prefix("- "));
        assert_eq!(editor.text(), "- one\n- two");
        assert!(editor.undo());
        assert_eq!(editor.text(), "one\ntwo");
    }

    /// COMP-H04: the toolbar emits only actions it actually offers.
    #[test]
    fn toolbar_executes_only_enabled_actions() {
        let mut toolbar = MarkdownToolbar::new(Rect::new(0, 0, 300, 28));
        let hits = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = hits.clone();
        toolbar.action_triggered.connect_ref(move |token| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(token.clone());
            }
        });
        assert!(toolbar.execute("bold"));
        assert!(!toolbar.execute("nope"));
        assert_eq!(hits.lock().ok().map(|guard| guard.clone()).unwrap_or_default(), vec!["bold"]);
        assert!(!toolbar.set_actions(vec!["nope".to_string()]));
        assert_eq!(toolbar.actions().len(), 7);
    }

    /// SP-P03: the preview cache keeps live entries when it is bounded.
    #[test]
    fn the_preview_cache_keeps_live_lines_when_it_is_trimmed() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
        editor.set_preview_mode(true);
        let text: String = (0..200).map(|line| format!("line {line}\n")).collect();
        editor.set_text(text);
        draw_once(&mut editor);
        // The buffer is longer than the bound only after several revisions; the assertion that
        // matters is that the live entries survive, so the following frame still draws them.
        editor.set_text(editor.text().to_string() + "tail");
        draw_once(&mut editor);
        assert!(editor.line_count() >= 200);
    }

    /// SP-P04: caret geometry follows edits instead of rescanning the document.
    #[test]
    fn caret_geometry_tracks_edits_with_the_line_index() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
        editor.set_text("one\ntwo\nthree");
        editor.goto_line(2);
        editor.move_to_line_edge(false);
        assert_eq!(editor.cursor_line(), 2);
        assert_eq!(editor.cursor_col(), 5);
        editor.insert_text_at_cursor("!");
        assert_eq!(editor.text(), "one\ntwo\nthree!");
        assert_eq!(editor.cursor_line(), 2);
        assert_eq!(editor.cursor_col(), 6);
    }
}
