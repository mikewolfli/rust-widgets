//! The [`CodeEditor`] widget: state machine, editing commands and input plumbing.

use super::buffer::{join_lines, split_lines, EditorModel};
use super::multicursor::{self, MultiCursor, OffsetEdit};
use super::pairs::{self, PairAction};
use super::syntax::{BuiltinHighlighter, LanguageId, SyntaxHighlighter};
use super::types::{
    CodeEditorConfig, CompletionSource, CompletionState, ContextMenuState, Cursor, CursorGoal,
    DiagnosticMarker, DocumentCompletions, EditorBuffer, FindState, FoldRegion, InlineDiagnostic,
    MarkerSeverity, MenuItem, SearchMatch, SearchOptions, SyntaxPalette, TextPosition, TokenKind,
    TokenSpan, VisualLine, MAX_COMPLETIONS,
};
use crate::compat::MiniVec;
use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, TextSnapshotCommand, UndoCommand, UndoStack};
use crate::widget::{BaseWidget, Widget, WidgetKind};
use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicU64, Ordering};

/// Shared, cloneable handle to the editing model.
pub(crate) type SharedModel = Rc<RefCell<EditorModel>>;

/// One caret's edit in a multi-caret batch.
struct CaretEdit {
    start: TextPosition,
    end: TextPosition,
    insert: String,
    /// Character index within `insert` where the caret lands; defaults to the end.
    caret_in_insert: Option<usize>,
}

impl CaretEdit {
    fn new(start: TextPosition, end: TextPosition, insert: impl Into<String>) -> Self {
        Self { start, end, insert: insert.into(), caret_in_insert: None }
    }

    fn with_caret(mut self, index: usize) -> Self {
        self.caret_in_insert = Some(index);
        self
    }
}

/// One undoable buffer switch, so undo can also walk back through tabs.
struct TabSwitchCommand {
    editor: SharedModel,
    before: usize,
    after: usize,
}

impl TabSwitchCommand {
    fn next_id() -> CommandId {
        static NEXT: AtomicU64 = AtomicU64::new(0x434F_4445);
        CommandId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl UndoCommand for TabSwitchCommand {
    fn id(&self) -> CommandId {
        Self::next_id()
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Switch tab".to_string(),
            timestamp_ms: 0,
            command_type: "code_editor_tab",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        if let Ok(mut model) = self.editor.try_borrow_mut() {
            model.apply_tab_switch(self.after);
        }
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        if let Ok(mut model) = self.editor.try_borrow_mut() {
            model.apply_tab_switch(self.before);
        }
        Ok(())
    }
}

/// Self-contained, Zed-class code editor widget.
///
/// The widget owns the full editor surface: text index, selection, history,
/// viewport, folds, tabs, find/replace, completion, context menu and the input
/// plumbing for all of them. Language intelligence that a real product loads as
/// a plugin (tree-sitter grammars, LSP, DAP, collaboration) is intentionally
/// left to the host through the [`SyntaxHighlighter`] and [`CompletionSource`]
/// seams — see the module documentation for the full scope boundary.
///
/// # Example
///
/// ```
/// use rust_widgets::core::Rect;
/// use rust_widgets::widget::special_widgets::code_editor::{
///     CodeEditor, CodeEditorConfig, LanguageId, MarkerSeverity, DiagnosticMarker, TokenKind,
/// };
///
/// let mut editor = CodeEditor::with_config(
///     Rect::new(0, 0, 800, 500),
///     CodeEditorConfig::new().language(LanguageId::Rust),
/// )
/// .expect("valid configuration");
///
/// editor.set_text("fn main() {\n    let x = 1;\n}");
/// assert_eq!(editor.line_count(), 3);
/// assert!(editor
///     .tokens_on_line(0)
///     .iter()
///     .any(|span| span.kind == TokenKind::Keyword));
///
/// editor.set_cursor(1, 4, false);
/// editor.insert("let y = 2;\n    ");
/// assert_eq!(editor.line_text(1).as_deref(), Some("    let y = 2;"));
///
/// editor.set_markers(vec![DiagnosticMarker::new(1, "unused", MarkerSeverity::Warning)]);
/// assert_eq!(editor.marker_count(MarkerSeverity::Warning), 1);
/// ```
pub struct CodeEditor {
    pub(crate) base: BaseWidget,
    pub(crate) model: SharedModel,
    pub(crate) config: CodeEditorConfig,
    highlighter: Option<Box<dyn SyntaxHighlighter>>,
    completion_source: Box<dyn CompletionSource>,
    pub(crate) palette: SyntaxPalette,

    pub(crate) cursor: Cursor,
    /// Secondary carets accumulated by multi-cursor commands; the primary caret
    /// lives in `cursor`.
    extra_cursors: Vec<Cursor>,
    goal: CursorGoal,
    /// First visible document line.
    scroll_line: usize,
    /// Horizontal scroll offset in character columns.
    pub(crate) scroll_column: usize,
    /// First visual row of the viewport, used when wrap or folds are active.
    pub(crate) scroll_visual_row: usize,
    /// Cached row budget for the current geometry.
    pub(crate) visible_rows: usize,

    pub(crate) find: FindState,
    pub(crate) completion: CompletionState,
    pub(crate) context_menu: ContextMenuState,

    pub(crate) markers: Vec<DiagnosticMarker>,
    /// Lines whose fold marker is currently collapsed, for O(1) gutter queries.
    folded_lines: MiniVec<usize>,
    pub(crate) mouse_selecting: bool,
    undo_stack: UndoStack,
    restoring_history: bool,

    /// Emitted when text changes.
    pub text_changed: Signal1<String>,
    /// Emitted when the caret position changes.
    pub cursor_moved: Signal1<(usize, usize)>,
    /// Emitted when the selection changes; `None` means collapsed.
    pub selection_changed: Signal1<Option<Rect>>,
    /// Emitted when the active buffer changes.
    pub tab_changed: Signal1<usize>,
    /// Emitted when the find query or its hit list changes.
    pub search_changed: Signal1<usize>,
    /// Emitted when the fold set changes.
    pub fold_changed: Signal1<usize>,
    /// Emitted when the completion popup opens, closes or moves.
    pub completion_changed: Signal1<bool>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Construction and configuration
// ─────────────────────────────────────────────────────────────────────────────

impl CodeEditor {
    /// Creates an editor with the default configuration.
    ///
    /// The default configuration always validates, so this constructor cannot
    /// fail; use [`CodeEditor::with_config`] when supplying custom values.
    pub fn new(geometry: Rect) -> Self {
        let config = CodeEditorConfig::default();
        debug_assert!(config.validate().is_ok(), "default configuration must be valid");
        Self::build(geometry, config)
    }

    /// Creates an editor from an explicit configuration.
    ///
    /// Returns `Err` when [`CodeEditorConfig::validate`] rejects a value, so a
    /// bad configuration surfaces at construction rather than at first paint.
    pub fn with_config(geometry: Rect, config: CodeEditorConfig) -> Result<Self, &'static str> {
        config.validate()?;
        Ok(Self::build(geometry, config))
    }

    fn build(geometry: Rect, config: CodeEditorConfig) -> Self {
        let mut editor = Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "CodeEditor"),
            model: Rc::new(RefCell::new(EditorModel::new())),
            config,
            highlighter: None,
            completion_source: Box::new(DocumentCompletions),
            palette: SyntaxPalette::default(),
            cursor: Cursor::default(),
            extra_cursors: Vec::new(),
            goal: CursorGoal::default(),
            scroll_line: 0,
            scroll_column: 0,
            scroll_visual_row: 0,
            visible_rows: 0,
            find: FindState::default(),
            completion: CompletionState::default(),
            context_menu: ContextMenuState::default(),
            markers: Vec::new(),
            folded_lines: MiniVec::new(),
            mouse_selecting: false,
            undo_stack: UndoStack::new(),
            restoring_history: false,
            text_changed: Signal1::new(),
            cursor_moved: Signal1::new(),
            selection_changed: Signal1::new(),
            tab_changed: Signal1::new(),
            search_changed: Signal1::new(),
            fold_changed: Signal1::new(),
            completion_changed: Signal1::new(),
        };
        editor.refresh_derived_state();
        editor
    }

    /// Returns the active configuration.
    pub fn config(&self) -> &CodeEditorConfig {
        &self.config
    }

    /// Returns `true` when the editor rejects mutations.
    pub fn is_read_only(&self) -> bool {
        self.config.read_only
    }

    /// Enables or disables editing.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.config.read_only = read_only;
        self.base.request_redraw();
    }

    /// Returns the current tab width in spaces.
    pub fn tab_width(&self) -> usize {
        self.config.tab_width
    }

    /// Sets the tab width, ignoring widths outside `1..=16`.
    pub fn set_tab_width(&mut self, width: usize) {
        if width == 0 || width > 16 {
            return;
        }
        self.config.tab_width = width;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the configured monospace font.
    pub fn font(&self) -> Font {
        Font::simple(self.config.font_family.clone(), self.config.font_size)
    }

    /// Returns the active syntax palette.
    pub fn palette(&self) -> &SyntaxPalette {
        &self.palette
    }

    /// Replaces the syntax palette.
    pub fn set_palette(&mut self, palette: SyntaxPalette) {
        self.palette = palette;
        self.base.request_redraw();
    }

    /// Installs a custom highlighter, replacing the built-in lexer.
    pub fn set_highlighter(&mut self, highlighter: Box<dyn SyntaxHighlighter>) {
        self.highlighter = Some(highlighter);
        self.base.request_redraw();
    }

    /// Removes a custom highlighter, restoring the built-in lexer.
    pub fn clear_highlighter(&mut self) {
        self.highlighter = None;
        self.base.request_redraw();
    }

    /// Installs a custom completion provider.
    pub fn set_completion_source(&mut self, source: Box<dyn CompletionSource>) {
        self.completion_source = source;
    }

    /// Returns the active language name.
    pub fn language_name(&self) -> String {
        match &self.highlighter {
            Some(highlighter) => highlighter.language_name().to_string(),
            None => self.config.language.name().to_string(),
        }
    }

    /// Sets the built-in language and resets indentation to its default.
    pub fn set_language(&mut self, language: LanguageId) {
        self.config.language = language;
        self.config.tab_width = language.default_tab_width();
        self.base.request_redraw();
    }

    /// Returns the keyword set used by the built-in lexer.
    pub fn keywords(&self) -> &'static [&'static str] {
        self.config.language.keywords()
    }

    // ── Text access ─────────────────────────────────────────────────────────

    /// Returns the current buffer text.
    pub fn text(&self) -> String {
        self.model.borrow().text.borrow().clone()
    }

    /// Replaces the whole document, pushing one undo checkpoint.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let next = text.into();
        if self.text() == next {
            return;
        }
        let before = self.text();
        if !self.restoring_history {
            let target = self.model.borrow().text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                target,
                before,
                next.clone(),
                "code_editor_text",
            )));
        }
        self.model.borrow_mut().set_text(next.clone());
        self.clamp_cursor_to_document();
        self.refresh_derived_state();
        self.text_changed.emit(next);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one line, preserving the existing document.
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        let mut next = self.text();
        if !next.is_empty() {
            next.push('\n');
        }
        next.push_str(line.as_ref());
        self.set_text(next);
        let last = self.line_count().saturating_sub(1);
        self.move_caret_to(TextPosition::new(last, self.line_len(last)), false);
    }

    /// Returns the number of document lines (always >= 1).
    pub fn line_count(&self) -> usize {
        self.model.borrow().line_count()
    }

    /// Returns one line, or `None` when out of range.
    pub fn line_text(&self, line: usize) -> Option<String> {
        self.model.borrow().lines.get(line).cloned()
    }

    /// Returns the character length of a line, or 0 when out of range.
    pub fn line_len(&self, line: usize) -> usize {
        self.model.borrow().line_len(line)
    }

    // ── Editing primitives ──────────────────────────────────────────────────

    /// Inserts text at the caret, replacing any selection.
    pub fn insert(&mut self, text: &str) {
        if self.config.read_only || text.is_empty() {
            return;
        }
        let payload = self.expand_tabs(text);
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = self.insertion_edits(&carets, &payload);
            self.apply_caret_edits(&carets, edits);
            return;
        }
        self.splice_payload(&payload);
    }

    /// Inserts text while honouring the configured indent unit for tabs.
    pub fn insert_with_indent(&mut self, text: &str) {
        self.insert(text);
    }

    fn expand_tabs(&self, text: &str) -> String {
        if !self.config.insert_spaces || !text.contains('\t') {
            return text.to_string();
        }
        let unit = " ".repeat(self.config.tab_width);
        text.replace('\t', &unit)
    }

    /// Replaces the selection (or inserts) with `payload`, in one undo step.
    fn splice_payload(&mut self, payload: &str) {
        let (start, end) = self.cursor.bounds();
        let newlines = payload.matches('\n').count();
        let head = if newlines == 0 {
            TextPosition::new(start.line, start.column + payload.chars().count())
        } else {
            TextPosition::new(start.line + newlines, trailing_line_len(payload))
        };
        let before = self.text();
        let after = self.model.borrow_mut().splice(start, end, payload);
        self.commit_edit(before, after, head);
    }

    /// Applies an edit that has already been computed.
    ///
    /// Callers either spliced the model themselves (`splice_payload`,
    /// `splice_range`) or rebuilt the text by hand (line commands); this writes
    /// the result back so both paths agree, then records one undo checkpoint.
    fn commit_edit(&mut self, before: String, after: String, head: TextPosition) {
        let already_applied = {
            let model = self.model.borrow();
            let text = model.text.borrow();
            let same = text.as_str() == after.as_str();
            same
        };
        if !already_applied {
            self.model.borrow_mut().set_text(after.clone());
        }
        if !self.restoring_history {
            let target = self.model.borrow().text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                target,
                before,
                after.clone(),
                "code_editor_edit",
            )));
        }
        self.cursor = Cursor { head: self.clamp_position(head), anchor: self.clamp_position(head) };
        self.goal.active = false;
        self.refresh_derived_state();
        self.text_changed.emit(after);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Applies an edit that replaces `[start, end)` without moving the caret
    /// through the payload (used by backward deletions and range operations).
    fn splice_range(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        payload: &str,
        head: TextPosition,
    ) {
        if self.config.read_only {
            return;
        }
        let before = self.text();
        let after = self.model.borrow_mut().splice(start, end, payload);
        self.commit_edit(before, after, head);
    }

    // ── Multi-caret infrastructure ──────────────────────────────────────────

    /// Returns the caret set used for editing: secondaries sorted, primary last.
    fn carets_for_edit(&self) -> Vec<Cursor> {
        let mut list = self.extra_cursors.clone();
        list.sort_by_key(|caret| {
            (caret.head.line, caret.head.column, caret.anchor.line, caret.anchor.column)
        });
        list.push(self.cursor);
        list
    }

    /// Applies one edit per caret in a single pass, then rebuilds the caret set.
    fn apply_caret_edits(&mut self, carets: &[Cursor], edits: Vec<CaretEdit>) -> bool {
        if self.config.read_only || carets.is_empty() || carets.len() != edits.len() {
            return false;
        }
        let before = self.text();
        let lines = self.model.borrow().lines.clone();
        let offset_edits: Vec<OffsetEdit> = edits
            .iter()
            .map(|edit| OffsetEdit {
                start: multicursor::char_offset(&lines, self.clamp_position(edit.start)),
                end: multicursor::char_offset(&lines, self.clamp_position(edit.end)),
                insert: edit.insert.clone(),
            })
            .collect();
        let (after, offsets) = multicursor::project(&before, &offset_edits);
        if after == before {
            return false;
        }
        let new_lines = split_lines(&after);
        let mut new_carets: Vec<Cursor> = Vec::with_capacity(carets.len());
        for (index, caret) in carets.iter().enumerate() {
            let position = match (offsets.get(index).copied().flatten(), edits.get(index)) {
                (Some(end_offset), Some(edit)) => {
                    let insert_len = edit.insert.chars().count();
                    let inside = edit.caret_in_insert.unwrap_or(insert_len).min(insert_len);
                    let offset = end_offset.saturating_sub(insert_len - inside);
                    multicursor::position_at_offset(&new_lines, offset)
                }
                // A dropped (overlapping) edit leaves its caret where it was.
                _ => caret.head,
            };
            new_carets.push(Cursor { head: position, anchor: position });
        }
        let primary = new_carets.pop().unwrap_or(self.cursor);
        if !self.restoring_history {
            let target = self.model.borrow().text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                target,
                before,
                after.clone(),
                "code_editor_edit",
            )));
        }
        self.model.borrow_mut().set_text(after.clone());
        self.cursor = Cursor {
            head: self.clamp_position(primary.head),
            anchor: self.clamp_position(primary.anchor),
        };
        self.extra_cursors = new_carets
            .into_iter()
            .map(|caret| Cursor {
                head: self.clamp_position(caret.head),
                anchor: self.clamp_position(caret.anchor),
            })
            .collect();
        self.goal.active = false;
        self.refresh_derived_state();
        self.text_changed.emit(after);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Builds the per-caret edits for inserting `payload` at every caret.
    fn insertion_edits(&self, carets: &[Cursor], payload: &str) -> Vec<CaretEdit> {
        carets
            .iter()
            .map(|caret| {
                let (start, end) = caret.bounds();
                CaretEdit::new(start, end, payload)
            })
            .collect()
    }

    /// Builds a delete-aware edit for one caret (selection, pair, or one char).
    fn backspace_edit_for(&self, caret: Cursor) -> CaretEdit {
        if !caret.is_collapsed() {
            let (start, end) = caret.bounds();
            return CaretEdit::new(start, end, "");
        }
        let head = caret.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        if pairs::should_delete_pair(before, after) && head.column > 0 {
            let start = TextPosition::new(head.line, head.column - 1);
            let end = if head.column < self.line_len(head.line) {
                TextPosition::new(head.line, head.column + 1)
            } else {
                head
            };
            return CaretEdit::new(start, end, "");
        }
        let start = if head.column > 0 {
            TextPosition::new(head.line, head.column - 1)
        } else if head.line > 0 {
            TextPosition::new(head.line - 1, self.line_len(head.line - 1))
        } else {
            return CaretEdit::new(head, head, "");
        };
        CaretEdit::new(start, head, "")
    }

    /// Builds a delete-forward edit for one caret.
    fn forward_delete_edit_for(&self, caret: Cursor) -> CaretEdit {
        if !caret.is_collapsed() {
            let (start, end) = caret.bounds();
            return CaretEdit::new(start, end, "");
        }
        let head = caret.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        if pairs::should_delete_pair(before, after) {
            let start =
                if head.column > 0 { TextPosition::new(head.line, head.column - 1) } else { head };
            let end = after.map(|_| TextPosition::new(head.line, head.column + 1)).unwrap_or(head);
            return CaretEdit::new(start, end, "");
        }
        let end = self.step_horizontal(head, 1);
        CaretEdit::new(head, end, "")
    }

    /// Builds the newline insert (auto-indent + block opening) for one caret.
    fn newline_edit_for(&self, caret: Cursor) -> CaretEdit {
        let (start, end) = caret.bounds();
        let indent = self.line_indent(start.line);
        let char_before = self.char_before(start);
        let char_after = self.char_at(end);
        let opens_block = matches!(char_before, Some('{' | '(' | '['));
        let closes_block = matches!(char_after, Some('}' | ')' | ']'));
        let unit = self.indent_unit();
        let mut payload = String::from("\n");
        payload.push_str(&indent);
        if opens_block {
            payload.push_str(&unit);
        }
        if opens_block && closes_block {
            payload.push('\n');
            payload.push_str(&indent);
        }
        let caret_index =
            1 + indent.chars().count() + if opens_block { unit.chars().count() } else { 0 };
        CaretEdit::new(start, end, payload).with_caret(caret_index)
    }

    // ── Multi-caret public API ──────────────────────────────────────────────

    /// Returns the full caret set (primary plus secondaries).
    pub fn cursors(&self) -> MultiCursor {
        MultiCursor::from_parts(self.cursor, &self.extra_cursors)
    }

    /// Returns `true` when more than one caret is active.
    pub fn has_multiple_cursors(&self) -> bool {
        !self.extra_cursors.is_empty()
    }

    /// Returns the number of secondary carets.
    pub fn extra_cursor_count(&self) -> usize {
        self.extra_cursors.len()
    }

    /// Collapses every secondary caret, keeping the primary one.
    pub fn collapse_cursors(&mut self) -> bool {
        if self.extra_cursors.is_empty() {
            return false;
        }
        self.extra_cursors.clear();
        self.base.request_redraw();
        true
    }

    /// Adds a caret one line above the primary caret.
    pub fn add_cursor_above(&mut self) -> bool {
        self.add_caret_by(-1)
    }

    /// Adds a caret one line below the primary caret.
    pub fn add_cursor_below(&mut self) -> bool {
        self.add_caret_by(1)
    }

    fn add_caret_by(&mut self, direction: isize) -> bool {
        let head = self.cursor.head;
        let last = self.line_count().saturating_sub(1) as isize;
        let target = head.line as isize + direction;
        if target < 0 || target > last {
            return false;
        }
        let line = target as usize;
        let column = self.line_len(line).min(head.column);
        let caret = Cursor {
            head: TextPosition::new(line, column),
            anchor: TextPosition::new(line, column),
        };
        if caret == self.cursor || self.extra_cursors.contains(&caret) {
            return false;
        }
        self.extra_cursors.push(self.cursor);
        self.cursor = caret;
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Selects the next occurrence of the current selection and adds a caret.
    ///
    /// With no selection this selects the word under the caret, mirroring the
    /// first press of Zed/VS Code `Cmd+D`.
    pub fn select_next_occurrence(&mut self) -> bool {
        let Some(selected) = self.selected_text() else {
            self.select_word_at(self.cursor.head);
            return self.has_selection();
        };
        if selected.is_empty() || selected.contains('\n') || selected.chars().count() > 128 {
            return false;
        }
        let hits =
            self.find_all_with(&selected, SearchOptions { case_sensitive: true, whole_word: true });
        if hits.is_empty() {
            return false;
        }
        let primary = self.cursor.bounds();
        let set = self.cursors();
        let start_index = hits
            .iter()
            .position(|hit| {
                TextPosition::new(hit.line, hit.start_column) == primary.0
                    && TextPosition::new(hit.line, hit.end_column) == primary.1
            })
            .map(|index| index + 1)
            .unwrap_or(0);
        let mut chosen = None;
        for step in 0..hits.len() {
            let hit = &hits[(start_index + step) % hits.len()];
            let candidate = Cursor {
                head: TextPosition::new(hit.line, hit.end_column),
                anchor: TextPosition::new(hit.line, hit.start_column),
            };
            if set.carets().contains(&candidate) {
                continue;
            }
            chosen = Some(candidate);
            break;
        }
        let Some(candidate) = chosen else { return false };
        if !self.extra_cursors.contains(&self.cursor) {
            self.extra_cursors.push(self.cursor);
        }
        self.cursor = candidate;
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Selects every occurrence of the selection (or the caret word) at once.
    ///
    /// Returns the resulting caret count.
    pub fn select_all_occurrences(&mut self) -> usize {
        if !self.has_selection() {
            self.select_word_at(self.cursor.head);
        }
        let Some(selected) = self.selected_text() else { return 0 };
        if selected.is_empty() || selected.contains('\n') {
            return 0;
        }
        let hits =
            self.find_all_with(&selected, SearchOptions { case_sensitive: true, whole_word: true });
        if hits.is_empty() {
            return 0;
        }
        let primary = self.cursor.bounds();
        let mut extras = Vec::new();
        let mut primary_kept = false;
        for hit in &hits {
            let bounds = (
                TextPosition::new(hit.line, hit.start_column),
                TextPosition::new(hit.line, hit.end_column),
            );
            if bounds == primary {
                primary_kept = true;
                continue;
            }
            extras.push(Cursor { head: bounds.1, anchor: bounds.0 });
        }
        if !primary_kept {
            if let Some(first) = extras.first().copied() {
                self.cursor = first;
                extras.remove(0);
            }
        }
        self.extra_cursors = extras;
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        self.cursors().len()
    }

    // ── Line commands ───────────────────────────────────────────────────────

    /// Duplicates the lines the selection touches, below the original block.
    pub fn duplicate_line(&mut self) {
        if self.config.read_only {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let last = end.line.min(lines.len().saturating_sub(1));
        let block: Vec<String> = lines[start.line..=last].to_vec();
        let insert_at = last + 1;
        for (offset, line) in block.into_iter().enumerate() {
            lines.insert(insert_at + offset, line);
        }
        let after = join_lines(&lines);
        let first_copy = insert_at;
        let copy_count = end.line - start.line;
        self.commit_edit(before, after, TextPosition::new(first_copy, 0));
        // Select the duplicated block using the freshly written model.
        let select_last = first_copy + copy_count;
        let head = TextPosition::new(select_last, self.line_len(select_last));
        self.cursor = Cursor { head, anchor: TextPosition::new(first_copy, 0) };
        self.emit_cursor_and_selection();
    }

    /// Deletes the lines the selection touches.
    pub fn delete_line(&mut self) {
        if self.config.read_only {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let last = end.line.min(lines.len().saturating_sub(1));
        lines.drain(start.line..=last);
        if lines.is_empty() {
            lines.push(String::new());
        }
        let after = join_lines(&lines);
        let head_line = start.line.min(lines.len().saturating_sub(1));
        let head = TextPosition::new(head_line, 0);
        self.commit_edit(before, after, head);
    }

    /// Joins the caret line with the following one, trimming its indentation.
    pub fn join_lines(&mut self) {
        if self.config.read_only {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let line = end.line.min(lines.len().saturating_sub(1));
        if line + 1 >= lines.len() {
            return;
        }
        let head_column = self.line_len(line);
        let next = lines[line + 1].trim_start().to_string();
        let needs_space =
            !next.is_empty() && !lines[line].ends_with(' ') && !lines[line].ends_with('\t');
        if needs_space {
            lines[line].push(' ');
        }
        lines[line].push_str(&next);
        lines.remove(line + 1);
        let after = join_lines(&lines);
        let head = TextPosition::new(line, head_column);
        self.commit_edit(before, after, head);
        let _ = start;
    }

    /// Sorts the selected lines, or the whole document when none are selected.
    pub fn sort_lines(&mut self) {
        if self.config.read_only {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let start_line = start.line;
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let last = end.line.min(lines.len().saturating_sub(1));
        if start_line >= last {
            lines.sort();
        } else {
            lines[start_line..=last].sort();
        }
        let after = join_lines(&lines);
        if after == before {
            return;
        }
        let head = TextPosition::new(last, 0);
        self.commit_edit(before, after, head);
        self.cursor.anchor = TextPosition::new(start_line, 0);
        self.emit_cursor_and_selection();
    }

    /// Removes trailing whitespace from every line.
    pub fn trim_trailing_whitespace(&mut self) {
        if self.config.read_only {
            return;
        }
        let before = self.text();
        let lines: Vec<String> =
            before.split('\n').map(|line| line.trim_end().to_string()).collect();
        let after = join_lines(&lines);
        if after == before {
            return;
        }
        let head = self.cursor.head;
        self.commit_edit(before, after, head);
    }

    /// Moves the caret to the first non-blank column of `line`.
    pub fn goto_line(&mut self, line: usize) {
        let last = self.line_count().saturating_sub(1);
        let line = line.min(last);
        let text = self.line_text(line).unwrap_or_default();
        let column = text.chars().take_while(|ch| ch.is_whitespace()).count();
        self.extra_cursors.clear();
        self.move_caret_to(TextPosition::new(line, column), false);
    }

    // ── Auto-pairing ────────────────────────────────────────────────────────

    /// Tries to apply auto-pairing for a single typed character.
    ///
    /// Returns `true` when the character was fully handled.
    pub(crate) fn try_auto_pair(&mut self, typed: char) -> bool {
        if self.has_multiple_cursors() {
            return false;
        }
        let head = self.cursor.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        let has_selection = self.has_selection();
        match pairs::plan_typing(typed, before, after, has_selection) {
            PairAction::Plain => false,
            PairAction::SkipOver => {
                self.move_cursor(0, 1);
                true
            }
            PairAction::Pair => {
                let close = pairs::closing_for(typed).unwrap_or(typed);
                if has_selection {
                    self.wrap_selection(typed, close);
                } else {
                    let payload = [typed, close].iter().collect::<String>();
                    self.insert(&payload);
                    self.move_cursor(0, -1);
                }
                true
            }
        }
    }

    /// Wraps the selection in `open`/`close`, keeping the inner text selected.
    fn wrap_selection(&mut self, open: char, close: char) {
        let (start, end) = self.cursor.bounds();
        let selected = self.model.borrow().extract(start, end);
        let newlines = selected.matches('\n').count();
        let payload = [open.to_string(), selected.clone(), close.to_string()].concat();
        let inner_column = if newlines == 0 {
            start.column + 1 + selected.chars().count()
        } else {
            trailing_line_len(&selected)
        };
        let head = TextPosition::new(start.line + newlines, inner_column);
        self.splice_range(start, end, &payload, head);
        let inner_start = TextPosition::new(start.line, start.column + 1);
        let inner_end = TextPosition::new(start.line + newlines, inner_column);
        self.cursor = Cursor {
            head: self.clamp_position(inner_end),
            anchor: self.clamp_position(inner_start),
        };
        self.emit_cursor_and_selection();
    }

    /// Deletes the character before the caret, or the selection.
    pub fn backspace(&mut self) {
        if self.config.read_only {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets.iter().map(|caret| self.backspace_edit_for(*caret)).collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        let head = self.cursor.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        if pairs::should_delete_pair(before, after) && head.column > 0 {
            let start = TextPosition::new(head.line, head.column - 1);
            let end = if head.column < self.line_len(head.line) {
                TextPosition::new(head.line, head.column + 1)
            } else {
                head
            };
            self.splice_range(start, end, "", start);
            return;
        }
        let start = if head.column > 0 {
            TextPosition::new(head.line, head.column - 1)
        } else if head.line > 0 {
            TextPosition::new(head.line - 1, self.line_len(head.line - 1))
        } else {
            return;
        };
        self.splice_range(start, head, "", start);
    }

    /// Deletes the character after the caret, or the selection.
    pub fn delete_forward(&mut self) {
        if self.config.read_only {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets.iter().map(|caret| self.forward_delete_edit_for(*caret)).collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        let head = self.cursor.head;
        let end = self.step_horizontal(head, 1);
        if end == head {
            return;
        }
        self.splice_range(head, end, "", head);
    }

    /// Deletes the whole word before the caret.
    pub fn delete_word_backward(&mut self) {
        if self.config.read_only {
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        let start = self.previous_word_boundary(self.cursor.head);
        if start == self.cursor.head {
            return;
        }
        self.splice_range(start, self.cursor.head, "", start);
    }

    /// Deletes the current selection.
    pub fn delete_selection(&mut self) {
        if self.config.read_only || self.cursor.is_collapsed() {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets
                .iter()
                .map(|caret| {
                    let (start, end) = caret.bounds();
                    CaretEdit::new(start, end, "")
                })
                .collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        let (start, _) = self.cursor.bounds();
        let (_, end) = self.cursor.bounds();
        self.splice_range(start, end, "", start);
    }

    /// Inserts a newline, carrying the indentation over and opening blocks.
    pub fn insert_newline(&mut self) {
        if self.config.read_only {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets.iter().map(|caret| self.newline_edit_for(*caret)).collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        let (start, end) = self.cursor.bounds();
        let indent = self.line_indent(start.line);
        let char_before = self.char_before(start);
        let char_after = self.char_at(end);
        let opens_block = matches!(char_before, Some('{' | '(' | '['));
        let closes_block = matches!(char_after, Some('}' | ')' | ']'));
        let unit = self.indent_unit();

        let mut payload = String::from("\n");
        payload.push_str(&indent);
        if opens_block {
            payload.push_str(&unit);
        }
        let head = if opens_block && closes_block {
            payload.push('\n');
            payload.push_str(&indent);
            TextPosition::new(start.line + 1, indent.chars().count() + unit.chars().count())
        } else {
            TextPosition::new(
                start.line + 1,
                indent.chars().count() + if opens_block { unit.chars().count() } else { 0 },
            )
        };
        self.splice_range(start, end, &payload, head);
    }

    /// Inserts the indent unit, or indents the selection when it spans lines.
    pub fn insert_tab(&mut self) {
        if self.config.read_only {
            return;
        }
        if self.has_selection()
            && self.cursor.line_span() > 1
            && self.selection_aligns_with_indent()
        {
            self.indent_selection();
            return;
        }
        let unit = self.indent_unit();
        self.insert(&unit);
    }

    /// Removes up to one indent unit of leading whitespace before the caret.
    pub fn outdent(&mut self) {
        if self.config.read_only {
            return;
        }
        let line = self.cursor.head.line;
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(line).chars().collect();
        drop(provider);
        let removable = chars
            .iter()
            .take_while(|ch| **ch == ' ' || **ch == '\t')
            .count()
            .min(self.config.tab_width)
            .min(self.cursor.head.column);
        if removable == 0 {
            return;
        }
        self.splice_range(
            TextPosition::new(line, 0),
            TextPosition::new(line, removable),
            "",
            TextPosition::new(line, self.cursor.head.column - removable),
        );
    }

    /// Indents every line the selection touches, in one undo step.
    pub fn indent_selection(&mut self) {
        if self.config.read_only {
            return;
        }
        let unit = self.indent_unit();
        self.transform_selected_lines(
            |line, _| format!("{unit}{line}"),
            |column| column + unit.chars().count(),
        );
    }

    /// Outdents every line the selection touches, in one undo step.
    pub fn outdent_selection(&mut self) {
        if self.config.read_only {
            return;
        }
        let width = self.config.tab_width;
        self.transform_selected_lines(
            |line, _| {
                let removable =
                    line.chars().take_while(|ch| *ch == ' ' || *ch == '\t').count().min(width);
                line.chars().skip(removable).collect()
            },
            |column| column.saturating_sub(width),
        );
    }

    /// Toggles line comments across the selection, in one undo step.
    pub fn toggle_line_comment(&mut self) {
        if self.config.read_only {
            return;
        }
        let token = self.config.language.comment_prefix();
        let bare = token.trim_end();
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines = before.split('\n').map(|line| line.to_string()).collect::<Vec<_>>();
        let last = end.line.min(lines.len().saturating_sub(1));
        let range: Vec<usize> = (start.line..=last).collect();
        let all_commented = range
            .iter()
            .filter(|index| lines.get(**index).map(|line| !line.trim().is_empty()).unwrap_or(false))
            .all(|index| {
                lines.get(*index).map(|line| line.trim_start().starts_with(bare)).unwrap_or(false)
            });

        let delta: isize = if all_commented {
            -(token.chars().count() as isize)
        } else {
            token.chars().count() as isize
        };
        for index in &range {
            let Some(line) = lines.get_mut(*index) else { continue };
            if line.trim().is_empty() {
                continue;
            }
            let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
            let body: String = line.chars().skip(indent).collect();
            let stripped =
                body.strip_prefix(token).or_else(|| body.strip_prefix(bare)).unwrap_or(&body);
            let rebuilt = if all_commented {
                format!("{}{stripped}", line.chars().take(indent).collect::<String>())
            } else {
                format!("{}{token}{stripped}", line.chars().take(indent).collect::<String>())
            };
            *line = rebuilt;
        }
        let after = join_lines(&lines);
        let shift = |column: usize, line_index: usize| {
            let length = lines.get(line_index).map(|line| line.chars().count()).unwrap_or(0);
            ((column as isize + delta).clamp(0, length as isize)) as usize
        };
        let head = TextPosition::new(end.line, shift(end.column, end.line));
        let before_anchor = TextPosition::new(start.line, shift(start.column, start.line));
        self.commit_edit(before, after, head);
        self.cursor.anchor = before_anchor;
        self.emit_cursor_and_selection();
    }

    /// Moves the caret line up (`direction < 0`) or down, preserving text.
    pub fn move_line(&mut self, direction: isize) {
        if self.config.read_only || direction == 0 {
            return;
        }
        let line = self.cursor.head.line;
        let last = self.line_count().saturating_sub(1);
        let target = line as isize + direction;
        if target < 0 || target > last as isize {
            return;
        }
        let target = target as usize;
        let before = self.text();
        let mut lines = before.split('\n').map(|text| text.to_string()).collect::<Vec<_>>();
        if line >= lines.len() || target >= lines.len() {
            return;
        }
        lines.swap(line, target);
        let after = join_lines(&lines);
        let head = TextPosition::new(target, self.cursor.head.column);
        self.commit_edit(before, after, head);
        self.cursor.anchor = TextPosition::new(target, 0);
        self.cursor.head = TextPosition::new(target, self.line_len(target).min(head.column));
        self.emit_cursor_and_selection();
    }

    /// Applies `rewrite` to every line the selection touches.
    ///
    /// `adjust` maps a caret column through the same transformation so the
    /// selection survives the edit.
    fn transform_selected_lines<F, G>(&mut self, rewrite: F, adjust: G)
    where
        F: Fn(&str, usize) -> String,
        G: Fn(usize) -> usize,
    {
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines = before.split('\n').map(|line| line.to_string()).collect::<Vec<_>>();
        let last = end.line.min(lines.len().saturating_sub(1));
        for (index, line) in lines.iter_mut().enumerate() {
            if index >= start.line && index <= last {
                *line = rewrite(line, index);
            }
        }
        let after = join_lines(&lines);
        let head = TextPosition::new(end.line, adjust(end.column));
        let anchor = TextPosition::new(start.line, adjust(start.column));
        self.commit_edit(before, after, head);
        self.cursor.anchor = self.clamp_position(anchor);
        self.emit_cursor_and_selection();
    }

    fn indent_unit(&self) -> String {
        if self.config.insert_spaces {
            " ".repeat(self.config.tab_width)
        } else {
            "\t".to_string()
        }
    }

    fn line_indent(&self, line: usize) -> String {
        self.model.borrow().line(line).chars().take_while(|ch| *ch == ' ' || *ch == '\t').collect()
    }

    fn char_before(&self, position: TextPosition) -> Option<char> {
        let provider = self.model.borrow();
        let text = provider.line(position.line);
        let mut chars = text.chars().take(position.column);
        let mut last = chars.next()?;
        for ch in chars {
            last = ch;
        }
        Some(last)
    }

    fn char_at(&self, position: TextPosition) -> Option<char> {
        self.model.borrow().line(position.line).chars().nth(position.column)
    }

    // ── History ─────────────────────────────────────────────────────────────

    /// Undoes the most recent edit. Returns `false` when there is nothing to do.
    pub fn undo(&mut self) -> bool {
        if self.config.read_only || self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Redoes the most recently undone edit.
    pub fn redo(&mut self) -> bool {
        if self.config.read_only || self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns `true` when an undo is available.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns `true` when a redo is available.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Clears the edit history without touching the text.
    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
    }

    fn restore_history_text(&mut self) {
        let restored = self.model.borrow().text.borrow().clone();
        self.restoring_history = true;
        self.model.borrow_mut().set_text(restored.clone());
        self.restoring_history = false;
        self.clamp_cursor_to_document();
        self.refresh_derived_state();
        self.text_changed.emit(restored);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    // ── Cursor and selection ────────────────────────────────────────────────

    /// Returns the caret as `(line, column)`, both zero-based.
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor.head.line, self.cursor.head.column)
    }

    /// Returns the caret as a [`TextPosition`].
    pub fn caret(&self) -> TextPosition {
        self.cursor.head
    }

    /// Returns the selection anchor.
    pub fn anchor(&self) -> TextPosition {
        self.cursor.anchor
    }

    /// Returns `true` when text is selected.
    pub fn has_selection(&self) -> bool {
        !self.cursor.is_collapsed()
    }

    /// Returns the selected text, or `None` when the selection is collapsed.
    pub fn selected_text(&self) -> Option<String> {
        if self.cursor.is_collapsed() {
            return None;
        }
        let (start, end) = self.cursor.bounds();
        Some(self.model.borrow().extract(start, end))
    }

    /// Returns the normalized selection bounds.
    pub fn selection_bounds(&self) -> (TextPosition, TextPosition) {
        self.cursor.bounds()
    }

    /// Places the caret, optionally extending the selection.
    pub fn set_cursor(&mut self, line: usize, column: usize, extend_selection: bool) {
        self.move_caret_to(TextPosition::new(line, column), extend_selection);
    }

    /// Sets the caret from an explicit position.
    pub fn set_caret(&mut self, position: TextPosition, extend_selection: bool) {
        self.move_caret_to(position, extend_selection);
    }

    pub(crate) fn move_caret_to(&mut self, position: TextPosition, extend_selection: bool) {
        let clamped = self.clamp_position(position);
        if clamped == self.cursor.head && self.cursor.is_collapsed() && !extend_selection {
            return;
        }
        self.cursor.head = clamped;
        if !extend_selection {
            self.cursor.anchor = clamped;
            self.goal.active = false;
        }
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects the entire document.
    pub fn select_all(&mut self) {
        let last = self.line_count().saturating_sub(1);
        self.cursor.anchor = TextPosition::new(0, 0);
        self.cursor.head = TextPosition::new(last, self.line_len(last));
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects one line, including its trailing newline when present.
    pub fn select_line(&mut self, line: usize) {
        let last = self.line_count().saturating_sub(1);
        let line = line.min(last);
        self.cursor.anchor = TextPosition::new(line, 0);
        self.cursor.head = if line < last {
            TextPosition::new(line + 1, 0)
        } else {
            TextPosition::new(line, self.line_len(line))
        };
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects the word under a character position.
    pub fn select_word_at(&mut self, position: TextPosition) {
        let position = self.clamp_position(position);
        let (start, end) = self.word_bounds(position);
        self.cursor.anchor = start;
        self.cursor.head = end;
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects the word under a widget-local point.
    pub fn select_word_at_point(&mut self, point: Point) -> bool {
        match self.position_at_point(point) {
            Some(position) => {
                self.select_word_at(position);
                true
            }
            None => false,
        }
    }

    fn word_bounds(&self, position: TextPosition) -> (TextPosition, TextPosition) {
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(position.line).chars().collect();
        drop(provider);
        let is_word = |ch: char| self.config.language.is_word_char(ch);
        let mut start = position.column.min(chars.len());
        let mut end = start;
        while start > 0 && is_word(chars[start - 1]) {
            start -= 1;
        }
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        if start == end {
            return (position, position);
        }
        (TextPosition::new(position.line, start), TextPosition::new(position.line, end))
    }

    // ── Caret movement ──────────────────────────────────────────────────────

    /// Moves the caret one character left/right or one line up/down.
    pub fn move_cursor(&mut self, delta_line: isize, delta_column: isize) {
        self.move_cursor_with_selection(delta_line, delta_column, false);
    }

    /// Moves the caret, extending the selection when requested.
    pub fn move_cursor_with_selection(
        &mut self,
        delta_line: isize,
        delta_column: isize,
        extend_selection: bool,
    ) {
        let mut head = self.cursor.head;
        if delta_line != 0 {
            let lines = self.line_count();
            let target =
                (head.line as isize + delta_line).clamp(0, lines.saturating_sub(1) as isize);
            head.line = target as usize;
            let provider = self.model.borrow();
            head.column = if self.goal.active {
                provider.clamp_column(head.line, self.goal.column)
            } else {
                provider.clamp_column(head.line, head.column)
            };
            drop(provider);
            self.goal = CursorGoal { active: true, column: head.column };
        } else {
            self.goal.active = false;
            let direction: isize = if delta_column < 0 { -1 } else { 1 };
            for _ in 0..delta_column.unsigned_abs() {
                head = self.step_horizontal(head, direction);
            }
        }
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Crosses line boundaries, which is what every real editor does.
    fn step_horizontal(&self, position: TextPosition, direction: isize) -> TextPosition {
        let provider = self.model.borrow();
        if direction < 0 {
            if position.column > 0 {
                TextPosition::new(position.line, position.column - 1)
            } else if position.line > 0 {
                let line = position.line - 1;
                TextPosition::new(line, provider.line_len(line))
            } else {
                position
            }
        } else if position.column < provider.line_len(position.line) {
            TextPosition::new(position.line, position.column + 1)
        } else if position.line + 1 < provider.lines.len() {
            TextPosition::new(position.line + 1, 0)
        } else {
            position
        }
    }

    /// Moves the caret one word left (`direction < 0`) or right.
    pub fn move_word(&mut self, direction: isize, extend_selection: bool) {
        let head = if direction < 0 {
            self.previous_word_boundary(self.cursor.head)
        } else {
            self.next_word_boundary(self.cursor.head)
        };
        if head == self.cursor.head {
            return;
        }
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    fn previous_word_boundary(&self, position: TextPosition) -> TextPosition {
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(position.line).chars().collect();
        let previous_len = position.line.checked_sub(1).map(|line| provider.line_len(line));
        drop(provider);
        let is_word = |ch: char| self.config.language.is_word_char(ch);
        let mut column = position.column.min(chars.len());
        while column > 0 && !is_word(chars[column - 1]) {
            column -= 1;
        }
        while column > 0 && is_word(chars[column - 1]) {
            column -= 1;
        }
        if column == 0 && position.column == 0 {
            if let Some(previous_len) = previous_len {
                return TextPosition::new(position.line - 1, previous_len);
            }
        }
        TextPosition::new(position.line, column)
    }

    fn next_word_boundary(&self, position: TextPosition) -> TextPosition {
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(position.line).chars().collect();
        drop(provider);
        let is_word = |ch: char| self.config.language.is_word_char(ch);
        let mut column = position.column.min(chars.len());
        while column < chars.len() && is_word(chars[column]) {
            column += 1;
        }
        while column < chars.len() && !is_word(chars[column]) {
            column += 1;
        }
        TextPosition::new(position.line, column)
    }

    /// Moves the caret to the start (`direction < 0`) or end of the line.
    pub fn move_to_line_edge(&mut self, direction: isize, extend_selection: bool) {
        let head = if direction < 0 {
            TextPosition::new(self.cursor.head.line, 0)
        } else {
            TextPosition::new(self.cursor.head.line, self.line_len(self.cursor.head.line))
        };
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Moves the caret to the very start or end of the document.
    pub fn move_to_document_edge(&mut self, direction: isize, extend_selection: bool) {
        let head = if direction < 0 {
            TextPosition::new(0, 0)
        } else {
            let last = self.line_count().saturating_sub(1);
            TextPosition::new(last, self.line_len(last))
        };
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    fn selection_aligns_with_indent(&self) -> bool {
        let (start, _) = self.cursor.bounds();
        let chars: Vec<char> = self.model.borrow().line(start.line).chars().collect();
        chars[..start.column.min(chars.len())].iter().all(|ch| ch.is_whitespace())
    }

    // ── Viewport ────────────────────────────────────────────────────────────

    /// Returns the first visible document line.
    pub fn scroll_line(&self) -> usize {
        self.scroll_line
    }

    /// Returns the number of visible rows for the current geometry.
    pub fn visible_rows(&self) -> usize {
        if self.visible_rows == 0 {
            return self.compute_visible_rows();
        }
        self.visible_rows
    }

    /// Scrolls by a number of lines, clamped to the document.
    pub fn scroll_by(&mut self, delta_lines: isize) {
        let rows = self.visible_rows();
        let max_first = self.line_count().saturating_sub(rows);
        let target = (self.scroll_line as isize + delta_lines).clamp(0, max_first as isize);
        if target as usize != self.scroll_line {
            self.scroll_line = target as usize;
            self.scroll_visual_row = self.line_to_visual_row(self.scroll_line);
            self.base.request_redraw();
        }
    }

    /// Scrolls horizontally by a number of columns.
    pub fn scroll_columns_by(&mut self, delta_columns: isize) {
        let target = (self.scroll_column as isize + delta_columns).max(0);
        if target as usize != self.scroll_column {
            self.scroll_column = target as usize;
            self.base.request_redraw();
        }
    }

    /// Keeps the caret inside the viewport, scrolling only when necessary.
    fn ensure_caret_visible(&mut self) {
        self.refresh_visible_rows();
        let rows = self.visible_rows();
        let visual = self.line_to_visual_row(self.cursor.head.line);
        if visual < self.scroll_visual_row {
            self.scroll_visual_row = visual;
            self.scroll_line = self.cursor.head.line;
        } else if visual >= self.scroll_visual_row + rows {
            self.scroll_visual_row = visual.saturating_sub(rows.saturating_sub(1));
            self.scroll_line = self.visual_row_to_line(self.scroll_visual_row);
        }
        let columns = self.text_columns();
        if self.cursor.head.column < self.scroll_column {
            self.scroll_column = self.cursor.head.column;
        } else if self.cursor.head.column >= self.scroll_column + columns {
            self.scroll_column = self.cursor.head.column + 1 - columns;
        }
        let max_first = self.line_count().saturating_sub(rows);
        self.scroll_line = self.scroll_line.min(max_first);
    }

    fn clamp_cursor_to_document(&mut self) {
        self.cursor = Cursor {
            head: self.clamp_position(self.cursor.head),
            anchor: self.clamp_position(self.cursor.anchor),
        };
    }

    pub(crate) fn clamp_position(&self, position: TextPosition) -> TextPosition {
        self.model.borrow().clamp_position(position)
    }

    fn emit_cursor_and_selection(&self) {
        self.cursor_moved.emit((self.cursor.head.line, self.cursor.head.column));
        if self.cursor.is_collapsed() {
            self.selection_changed.emit(None);
        } else {
            let (start, end) = self.cursor.bounds();
            self.selection_changed.emit(Some(Rect::new(
                start.column as i32,
                start.line as i32,
                (end.column.saturating_sub(start.column)).max(1) as u32,
                (end.line.saturating_sub(start.line) + 1) as u32,
            )));
        }
    }

    /// Recomputes fold hints, match cache, row budget and the cursor clamp.
    fn refresh_derived_state(&mut self) {
        {
            let model = self.model.borrow();
            self.folded_lines.clear();
            for region in &model.folds {
                if region.folded {
                    self.folded_lines.push(region.start_line);
                }
            }
        }
        self.clamp_cursor_to_visible();
        self.recompute_match_cache();
        self.refresh_visible_rows();
    }

    // ── Folding ─────────────────────────────────────────────────────────────

    /// Folds `start_line..=end_line` into a single visible row.
    pub fn fold(&mut self, start_line: usize, end_line: usize) {
        let last = self.line_count().saturating_sub(1);
        let start = start_line.min(last);
        let end = end_line.max(start + 1).min(last);
        if end <= start {
            return;
        }
        {
            let mut model = self.model.borrow_mut();
            match model.fold_at(start) {
                Some(index) => {
                    if let Some(region) = model.folds.get_mut(index) {
                        region.end_line = end;
                        region.folded = true;
                    }
                }
                None => model.folds.push(FoldRegion::new(start, end, true)),
            }
        }
        self.after_fold_change();
    }

    /// Folds the innermost bracket region enclosing `line`.
    pub fn fold_block_at(&mut self, line: usize) -> bool {
        let start = match self.enclosing_bracket_start(line) {
            Some(start) => start,
            None => return false,
        };
        let end = match self.matching_bracket_line(start) {
            Some(end) => end,
            None => return false,
        };
        if end <= start {
            return false;
        }
        self.fold(start, end);
        true
    }

    /// Expands the region starting at `start_line`.
    pub fn unfold(&mut self, start_line: usize) -> bool {
        let changed = {
            let mut model = self.model.borrow_mut();
            match model.fold_at(start_line).and_then(|index| model.folds.get_mut(index)) {
                Some(region) => {
                    region.folded = false;
                    true
                }
                None => false,
            }
        };
        if changed {
            self.after_fold_change();
        }
        changed
    }

    /// Toggles the fold state of `start_line`, inferring the block when needed.
    pub fn toggle_fold(&mut self, start_line: usize) -> bool {
        let state = self
            .model
            .borrow()
            .folds
            .iter()
            .find(|region| region.start_line == start_line)
            .map(|region| region.folded);
        match state {
            Some(true) => self.unfold(start_line),
            Some(false) => {
                let changed = {
                    let mut model = self.model.borrow_mut();
                    match model.fold_at(start_line).and_then(|index| model.folds.get_mut(index)) {
                        Some(region) => {
                            region.folded = true;
                            true
                        }
                        None => false,
                    }
                };
                if changed {
                    self.after_fold_change();
                }
                changed
            }
            None => self.fold_block_at(start_line),
        }
    }

    /// Expands every fold region.
    pub fn unfold_all(&mut self) {
        let changed = {
            let mut model = self.model.borrow_mut();
            let mut changed = false;
            for region in &mut model.folds {
                if region.folded {
                    region.folded = false;
                    changed = true;
                }
            }
            changed
        };
        if changed {
            self.after_fold_change();
        }
    }

    /// Returns the fold regions of the active buffer.
    pub fn fold_regions(&self) -> Vec<FoldRegion> {
        self.model.borrow().folds.clone()
    }

    /// Returns how many lines are hidden by folds.
    pub fn folded_line_count(&self) -> usize {
        self.model.borrow().folded_line_count()
    }

    /// Returns `true` when `line` starts a currently folded region.
    pub fn is_line_folded(&self, line: usize) -> bool {
        self.folded_lines.as_slice().contains(&line)
    }

    fn after_fold_change(&mut self) {
        self.refresh_derived_state();
        self.fold_changed.emit(self.folded_line_count());
        self.base.request_layout();
        self.base.request_redraw();
    }

    fn clamp_cursor_to_visible(&mut self) {
        let hidden_region = {
            let model = self.model.borrow();
            if model.is_hidden(self.cursor.head.line) {
                model
                    .folds
                    .iter()
                    .find(|region| region.folded && self.cursor.head.line <= region.end_line)
                    .map(|region| region.start_line)
            } else {
                None
            }
        };
        if let Some(start) = hidden_region {
            let position = TextPosition::new(start, self.line_len(start));
            self.cursor = Cursor { head: position, anchor: position };
        }
    }

    /// Finds the line containing the bracket that encloses `line`.
    fn enclosing_bracket_start(&self, line: usize) -> Option<usize> {
        let model = self.model.borrow();
        let line = line.min(model.lines.len().saturating_sub(1));
        let mut depth = 0isize;
        for index in (0..=line).rev() {
            for ch in model.line(index).chars().rev() {
                match ch {
                    '}' | ')' | ']' => depth += 1,
                    '{' | '(' | '[' => {
                        depth -= 1;
                        if depth < 0 {
                            return Some(index);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Returns the line of the bracket closing the opener at `start`.
    fn matching_bracket_line(&self, start: usize) -> Option<usize> {
        let mut depth = 0isize;
        for line in start..self.line_count() {
            let length = self.line_len(line);
            for column in 0..length {
                match self.char_at(TextPosition::new(line, column)) {
                    Some('{') | Some('(') | Some('[') => depth += 1,
                    Some('}') | Some(')') | Some(']') => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(line);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    // ── Visual row mapping ─────────────────────────────────────────────────

    pub(crate) fn visible_document_lines(&self) -> Vec<usize> {
        self.model.borrow().visible_lines()
    }

    pub(crate) fn wrap_segments(&self, line: usize) -> usize {
        if !self.config.word_wrap {
            return 1;
        }
        let columns = self.text_columns().max(1);
        let length = self.line_len(line);
        if length == 0 {
            return 1;
        }
        length.div_ceil(columns)
    }

    pub(crate) fn total_visual_rows(&self) -> usize {
        let visible = self.visible_document_lines();
        let rows: usize = visible.iter().map(|line| self.wrap_segments(*line)).sum();
        rows.saturating_add(self.inline_diagnostics().len())
    }

    pub(crate) fn line_to_visual_row(&self, line: usize) -> usize {
        let mut row = 0usize;
        for document_line in self.visible_document_lines() {
            if document_line >= line {
                break;
            }
            row += self.wrap_segments(document_line);
        }
        row
    }

    pub(crate) fn visual_row_to_line(&self, row: usize) -> usize {
        let visible = self.visible_document_lines();
        let mut cursor = 0usize;
        for line in &visible {
            let segments = self.wrap_segments(*line);
            if row < cursor + segments {
                return *line;
            }
            cursor += segments;
        }
        visible.last().copied().unwrap_or(0)
    }

    /// Returns the visual rows of the whole document.
    pub fn visual_lines(&self) -> Vec<VisualLine> {
        let columns = self.text_columns().max(1);
        let mut result = Vec::new();
        for line in self.visible_document_lines() {
            let segments = self.wrap_segments(line);
            for segment in 0..segments {
                let start_column = segment * columns;
                let end_column = if self.config.word_wrap {
                    ((segment + 1) * columns).min(self.line_len(line))
                } else {
                    self.line_len(line)
                };
                result.push(VisualLine { line, segment, start_column, end_column });
            }
        }
        result
    }

    /// Returns the visible row range as `(first, last)`.
    pub fn visible_row_range(&self) -> (usize, usize) {
        let rows = self.visible_rows();
        (self.scroll_visual_row, self.scroll_visual_row + rows.saturating_sub(1))
    }

    /// Returns the total number of visual rows in the document.
    pub fn visual_row_count(&self) -> usize {
        self.total_visual_rows()
    }

    // ── Diagnostics ─────────────────────────────────────────────────────────

    /// Replaces the diagnostic markers.
    pub fn set_markers(&mut self, markers: Vec<DiagnosticMarker>) {
        self.markers = markers;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one diagnostic marker.
    pub fn add_marker(&mut self, marker: DiagnosticMarker) {
        self.markers.push(marker);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes every marker on `line`; returns how many were removed.
    pub fn clear_markers_on_line(&mut self, line: usize) -> usize {
        let before = self.markers.len();
        self.markers.retain(|marker| marker.line != line);
        let removed = before - self.markers.len();
        if removed > 0 {
            self.base.request_layout();
            self.base.request_redraw();
        }
        removed
    }

    /// Returns the diagnostic markers.
    pub fn markers(&self) -> &[DiagnosticMarker] {
        &self.markers
    }

    /// Returns the number of markers of a given severity.
    pub fn marker_count(&self, severity: MarkerSeverity) -> usize {
        self.markers.iter().filter(|marker| marker.severity == severity).count()
    }

    /// Sorts markers by line, then severity, then column.
    pub fn sort_markers(&mut self) {
        self.markers.sort_by(|a, b| {
            a.line
                .cmp(&b.line)
                .then_with(|| a.severity.rank().cmp(&b.severity.rank()))
                .then_with(|| a.start_column().cmp(&b.start_column()))
        });
        self.base.request_redraw();
    }

    /// Returns the top severity on `line`, where errors outrank warnings.
    pub(crate) fn highest_severity_on_line(&self, line: usize) -> Option<MarkerSeverity> {
        self.markers
            .iter()
            .filter(|marker| marker.line == line)
            .map(|marker| marker.severity)
            .min_by_key(|severity| severity.rank())
    }

    pub(crate) fn inline_diagnostics(&self) -> Vec<InlineDiagnostic> {
        if self.markers.is_empty() {
            return Vec::new();
        }
        let mut rows_by_line: Vec<(usize, usize)> = Vec::new();
        let mut row = 0usize;
        for line in self.visible_document_lines() {
            rows_by_line.push((line, row));
            row += self.wrap_segments(line);
        }
        let mut result: Vec<InlineDiagnostic> = self
            .markers
            .iter()
            .filter_map(|marker| {
                rows_by_line
                    .iter()
                    .find(|(line, _)| *line == marker.line)
                    .map(|(_, row)| (*row, marker.clone()))
            })
            .collect();
        result.sort_by_key(|(row, marker)| (*row, marker.severity.rank(), marker.start_column()));
        result
    }

    // ── Tabs ────────────────────────────────────────────────────────────────

    /// Returns the open buffers.
    pub fn buffers(&self) -> Vec<EditorBuffer> {
        self.model.borrow().all_buffers.clone()
    }

    /// Opens a new buffer and activates it, returning its index.
    pub fn open_buffer(&mut self, title: impl Into<String>, text: impl Into<String>) -> usize {
        let index = {
            let mut model = self.model.borrow_mut();
            model.all_buffers.push(EditorBuffer::new(title, text));
            model.all_buffers.len() - 1
        };
        self.activate_buffer(index);
        index
    }

    /// Returns the active buffer index.
    pub fn active_buffer(&self) -> usize {
        self.model.borrow().active_tab
    }

    /// Activates a buffer, pushing an undoable tab-switch entry.
    pub fn activate_buffer(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count || index == self.active_buffer() {
            return false;
        }
        let before = self.active_buffer();
        self.undo_stack.push(Box::new(TabSwitchCommand {
            editor: Rc::clone(&self.model),
            before,
            after: index,
        }));
        self.model.borrow_mut().apply_tab_switch(index);
        self.after_buffer_switch(index);
        true
    }

    /// Activates a buffer without recording an undo entry.
    pub fn activate_buffer_untracked(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count || index == self.active_buffer() {
            return false;
        }
        self.model.borrow_mut().apply_tab_switch(index);
        self.after_buffer_switch(index);
        true
    }

    fn after_buffer_switch(&mut self, index: usize) {
        self.cursor = Cursor::default();
        self.goal = CursorGoal::default();
        self.scroll_line = 0;
        self.scroll_column = 0;
        self.scroll_visual_row = 0;
        self.find = FindState {
            visible: self.find.visible,
            replace_visible: self.find.replace_visible,
            ..FindState::default()
        };
        self.dismiss_completion();
        self.refresh_derived_state();
        self.text_changed.emit(self.text());
        self.tab_changed.emit(index);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes a buffer. The last remaining buffer is emptied instead.
    pub fn close_buffer(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count {
            return false;
        }
        if count == 1 {
            self.set_text(String::new());
            return true;
        }
        let next_active = {
            let mut model = self.model.borrow_mut();
            model.all_buffers.remove(index);
            if model.active_tab > index || model.active_tab >= model.all_buffers.len() {
                model.active_tab = model.active_tab.saturating_sub(1);
            }
            model.active_tab
        };
        self.model.borrow_mut().apply_tab_switch(next_active);
        self.model.borrow_mut().active_tab = next_active;
        self.after_buffer_switch(next_active);
        true
    }

    /// Returns `true` when the active buffer has unsaved edits.
    pub fn is_modified(&self) -> bool {
        let model = self.model.borrow();
        let current = model.text.borrow().clone();
        model.saved != current
    }

    /// Marks the active buffer as saved.
    pub fn mark_saved(&mut self) {
        let current = self.model.borrow().text.borrow().clone();
        let mut model = self.model.borrow_mut();
        model.saved = current.clone();
        let active = model.active_tab;
        if let Some(buffer) = model.all_buffers.get_mut(active) {
            buffer.text = current;
            buffer.modified = false;
        }
    }

    /// Renames the active buffer.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        let mut model = self.model.borrow_mut();
        let active = model.active_tab;
        if let Some(buffer) = model.all_buffers.get_mut(active) {
            buffer.title = title;
        }
    }

    /// Returns the active buffer title.
    pub fn title(&self) -> String {
        let model = self.model.borrow();
        model
            .all_buffers
            .get(model.active_tab)
            .map(|buffer| buffer.title.clone())
            .unwrap_or_default()
    }

    // ── Find and replace ────────────────────────────────────────────────────

    /// Returns the find-bar state.
    pub fn find_state(&self) -> &FindState {
        &self.find
    }

    /// Opens the find bar, optionally revealing the replacement row.
    pub fn open_find(&mut self, replace: bool) {
        self.find.visible = true;
        self.find.replace_visible = replace;
        self.find.focus_replace = false;
        self.dismiss_completion();
        self.close_context_menu();
        self.recompute_match_cache();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes the find bar.
    pub fn close_find(&mut self) {
        if !self.find.visible {
            return;
        }
        self.find.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns `true` while the find bar is on screen.
    pub fn find_visible(&self) -> bool {
        self.find.visible
    }

    /// Sets the search query and recomputes hits.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.find.query = query.into();
        self.find.current = 0;
        self.recompute_match_cache();
        self.search_changed.emit(self.find.match_count());
        self.base.request_redraw();
    }

    /// Sets the replacement text.
    pub fn set_search_replacement(&mut self, replacement: impl Into<String>) {
        self.find.replacement = replacement.into();
        self.base.request_redraw();
    }

    /// Updates the matching policy and recomputes hits.
    pub fn set_search_options(&mut self, options: SearchOptions) {
        self.find.options = options;
        self.find.current = 0;
        self.recompute_match_cache();
        self.search_changed.emit(self.find.match_count());
        self.base.request_redraw();
    }

    /// Returns the current search hits.
    pub fn search_matches(&self) -> &[SearchMatch] {
        &self.find.matches
    }

    /// Finds every occurrence of `query` under the current options.
    pub fn find_all(&self, query: &str) -> Vec<SearchMatch> {
        self.find_all_with(query, self.find.options)
    }

    /// Finds every occurrence of `query` under explicit `options`.
    ///
    /// This is the shared engine behind the find bar and the multi-cursor
    /// occurrence commands, so both agree on what a match is.
    pub fn find_all_with(&self, query: &str, options: SearchOptions) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle: Vec<char> = query.chars().collect();
        let language = self.config.language;
        let model = self.model.borrow();
        let mut matches = Vec::new();
        for (line_index, line) in model.lines.iter().enumerate() {
            let haystack: Vec<char> = line.chars().collect();
            if haystack.len() < needle.len() {
                continue;
            }
            let mut start = 0usize;
            while start + needle.len() <= haystack.len() {
                let candidate = &haystack[start..start + needle.len()];
                let equal = if options.case_sensitive {
                    candidate == needle.as_slice()
                } else {
                    candidate
                        .iter()
                        .map(|ch| ch.to_lowercase().collect::<String>())
                        .collect::<Vec<_>>()
                        == needle
                            .iter()
                            .map(|ch| ch.to_lowercase().collect::<String>())
                            .collect::<Vec<_>>()
                };
                if equal && whole_word_ok(&haystack, start, needle.len(), options, language) {
                    matches.push(SearchMatch {
                        line: line_index,
                        start_column: start,
                        end_column: start + needle.len(),
                    });
                    start += needle.len();
                } else {
                    start += 1;
                }
            }
        }
        matches
    }

    fn recompute_match_cache(&mut self) {
        let query = self.find.query.clone();
        self.find.matches = self.find_all(&query);
        self.find.current = if self.find.matches.is_empty() {
            0
        } else {
            self.find.current.min(self.find.matches.len() - 1)
        };
    }

    /// Advances to the next hit (or the previous one), wrapping around.
    pub fn find_next(&mut self, backwards: bool) -> bool {
        if self.find.matches.is_empty() {
            return false;
        }
        let count = self.find.matches.len();
        self.find.current = if backwards {
            (self.find.current + count - 1) % count
        } else {
            (self.find.current + 1) % count
        };
        self.reveal_current_match()
    }

    /// Reveals and selects the active hit.
    pub fn reveal_current_match(&mut self) -> bool {
        let Some(hit) = self.find.matches.get(self.find.current).cloned() else {
            return false;
        };
        self.cursor.anchor = TextPosition::new(hit.line, hit.start_column);
        self.cursor.head = TextPosition::new(hit.line, hit.end_column);
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Replaces the active hit and advances to the next one.
    pub fn replace_current(&mut self) -> bool {
        if self.config.read_only {
            return false;
        }
        let Some(hit) = self.find.matches.get(self.find.current).cloned() else {
            return false;
        };
        let replacement = self.find.replacement.clone();
        let start = TextPosition::new(hit.line, hit.start_column);
        let head = TextPosition::new(hit.line, hit.start_column + replacement.chars().count());
        self.splice_range(start, TextPosition::new(hit.line, hit.end_column), &replacement, head);
        self.find_next(false)
    }

    /// Replaces every hit in one undoable step; returns how many were replaced.
    pub fn replace_all(&mut self) -> usize {
        if self.config.read_only || self.find.matches.is_empty() {
            return 0;
        }
        let replacement = self.find.replacement.clone();
        let before = self.text();
        let mut lines = before.split('\n').map(|line| line.to_string()).collect::<Vec<_>>();
        // Group hits per line, then apply each group right-to-left so earlier
        // columns stay valid while the line is rebuilt.
        let mut groups: Vec<(usize, Vec<SearchMatch>)> = Vec::new();
        for hit in self.find.matches.iter().rev() {
            match groups.last_mut() {
                Some((line, group)) if *line == hit.line => group.push(hit.clone()),
                _ => groups.push((hit.line, vec![hit.clone()])),
            }
        }
        let mut replaced = 0usize;
        for (line_index, group) in groups {
            let Some(line) = lines.get_mut(line_index) else { continue };
            let chars: Vec<char> = line.chars().collect();
            let mut rebuilt = String::new();
            let mut cursor = 0usize;
            for hit in group.iter().rev() {
                if hit.start_column < cursor || hit.end_column > chars.len() {
                    continue;
                }
                rebuilt.extend(&chars[cursor..hit.start_column]);
                rebuilt.push_str(&replacement);
                cursor = hit.end_column;
                replaced += 1;
            }
            rebuilt.extend(&chars[cursor.min(chars.len())..]);
            *line = rebuilt;
        }
        if replaced == 0 {
            return 0;
        }
        let after = join_lines(&lines);
        let head = self.cursor.head;
        self.commit_edit(before, after, head);
        self.recompute_match_cache();
        self.search_changed.emit(self.find.match_count());
        replaced
    }

    pub(crate) fn backspace_find_query(&mut self) {
        if self.find.focus_replace {
            self.find.replacement.pop();
            self.base.request_redraw();
            return;
        }
        self.find.query.pop();
        let query = self.find.query.clone();
        self.set_search_query(query);
    }

    pub(crate) fn append_find_query(&mut self, text: &str) {
        if self.find.focus_replace {
            self.find.replacement.push_str(text);
            self.base.request_redraw();
            return;
        }
        let mut query = self.find.query.clone();
        query.push_str(text);
        self.set_search_query(query);
    }

    // ── Bracket matching and occurrences ────────────────────────────────────

    /// Returns the caret bracket and its match, when enabled.
    pub fn matching_brackets(&self) -> Option<(TextPosition, TextPosition)> {
        if !self.config.highlight_brackets {
            return None;
        }
        let candidate = match self.char_at(self.cursor.head).filter(|ch| pairs::is_bracket(*ch)) {
            Some(ch) => (self.cursor.head, ch),
            None => {
                let previous = self.cursor.head.column.checked_sub(1)?;
                let position = TextPosition::new(self.cursor.head.line, previous);
                let ch = self.char_at(position).filter(|ch| pairs::is_bracket(*ch))?;
                (position, ch)
            }
        };
        let (position, ch) = candidate;
        self.find_matching_bracket(position, ch).map(|matched| (position, matched))
    }

    fn find_matching_bracket(&self, start: TextPosition, ch: char) -> Option<TextPosition> {
        let forward = matches!(ch, '(' | '[' | '{');
        let mut depth = 0isize;
        if forward {
            for line in start.line..self.line_count() {
                let from = if line == start.line { start.column + 1 } else { 0 };
                for column in from..self.line_len(line) {
                    let Some(candidate) = self.char_at(TextPosition::new(line, column)) else {
                        continue;
                    };
                    if candidate == ch {
                        depth += 1;
                    } else if closes_bracket(ch, candidate) {
                        if depth == 0 {
                            return Some(TextPosition::new(line, column));
                        }
                        depth -= 1;
                    }
                }
            }
        } else {
            for line in (0..=start.line).rev() {
                let from = if line == start.line { start.column } else { self.line_len(line) };
                for column in (0..from).rev() {
                    let Some(candidate) = self.char_at(TextPosition::new(line, column)) else {
                        continue;
                    };
                    if candidate == ch {
                        depth += 1;
                    } else if opens_bracket(ch, candidate) {
                        if depth == 0 {
                            return Some(TextPosition::new(line, column));
                        }
                        depth -= 1;
                    }
                }
            }
        }
        None
    }

    /// Returns every occurrence of the currently selected word.
    pub fn occurrences_of_selection(&self) -> Vec<SearchMatch> {
        if !self.config.highlight_occurrences {
            return Vec::new();
        }
        let Some(selected) = self.selected_text() else { return Vec::new() };
        if selected.is_empty() || selected.contains('\n') || selected.chars().count() > 64 {
            return Vec::new();
        }
        // Reuse `find_all_with` with whole-word, case-sensitive matching without
        // disturbing the user's own find-bar policy.
        self.find_all_with(&selected, SearchOptions { case_sensitive: true, whole_word: true })
    }

    // ── Clipboard ───────────────────────────────────────────────────────────

    /// Copies the selection to the system clipboard.
    pub fn copy(&mut self) -> bool {
        let Some(selected) = self.selected_text() else { return false };
        if selected.is_empty() {
            return false;
        }
        crate::clipboard::ClipboardManager::set_text(selected);
        true
    }

    /// Cuts the selection to the system clipboard.
    pub fn cut(&mut self) -> bool {
        if self.config.read_only || !self.copy() {
            return false;
        }
        self.delete_selection();
        true
    }

    /// Pastes the system clipboard at the caret.
    pub fn paste(&mut self) -> bool {
        if self.config.read_only {
            return false;
        }
        let text = crate::clipboard::ClipboardManager::text();
        if text.is_empty() {
            return false;
        }
        self.insert(&text);
        true
    }

    // ── Completion ──────────────────────────────────────────────────────────

    /// Returns the completion popup state.
    pub fn completion_state(&self) -> &CompletionState {
        &self.completion
    }

    /// Computes and shows completions for the prefix ending at the caret.
    ///
    /// Returns the number of candidates offered; 0 closes the popup.
    pub fn trigger_completion(&mut self) -> usize {
        let head = self.cursor.head;
        let prefix_start = self.prefix_start_column();
        let chars: Vec<char> = self.model.borrow().line(head.line).chars().collect();
        let prefix: String =
            chars[prefix_start.min(chars.len())..head.column.min(chars.len())].iter().collect();
        let text = self.text();
        let items = self.completion_source.completions(&text, &prefix, MAX_COMPLETIONS);
        if items.is_empty() {
            self.dismiss_completion();
            return 0;
        }
        self.completion.items = items;
        self.completion.selected = 0;
        self.completion.anchor_column = prefix_start;
        self.completion.visible = true;
        self.context_menu.visible = false;
        self.completion_changed.emit(true);
        self.base.request_layout();
        self.base.request_redraw();
        self.completion.items.len()
    }

    /// Hides the completion popup.
    pub fn dismiss_completion(&mut self) {
        if !self.completion.visible {
            return;
        }
        self.completion.visible = false;
        self.completion.items.clear();
        self.completion_changed.emit(false);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Moves the completion highlight by `delta` rows, wrapping around.
    pub fn move_completion_selection(&mut self, delta: isize) -> bool {
        if !self.completion.visible || self.completion.items.is_empty() {
            return false;
        }
        let count = self.completion.items.len();
        let current = self.completion.selected.min(count - 1) as isize;
        self.completion.selected = (current + delta).rem_euclid(count as isize) as usize;
        self.base.request_redraw();
        true
    }

    /// Accepts the highlighted completion, replacing the typed prefix.
    pub fn accept_completion(&mut self) -> bool {
        if !self.completion.visible {
            return false;
        }
        let Some(item) = self.completion.selected_item().cloned() else {
            self.dismiss_completion();
            return false;
        };
        let line = self.cursor.head.line;
        let start = TextPosition::new(line, self.completion.anchor_column);
        let end = TextPosition::new(line, self.cursor.head.column);
        self.completion.visible = false;
        self.completion.items.clear();
        let head = TextPosition::new(line, start.column + item.label.chars().count());
        self.splice_range(start, end, &item.label, head);
        self.completion_changed.emit(false);
        true
    }

    pub(crate) fn prefix_start_column(&self) -> usize {
        let head = self.cursor.head;
        let model = self.model.borrow();
        let chars: Vec<char> = model.line(head.line).chars().collect();
        let mut start = head.column.min(chars.len());
        while start > 0 {
            let ch = chars[start - 1];
            if self.config.language.is_word_char(ch) {
                start -= 1;
            } else {
                break;
            }
        }
        start
    }

    // ── Context menu ────────────────────────────────────────────────────────

    /// Returns the context-menu state.
    pub fn context_menu(&self) -> &ContextMenuState {
        &self.context_menu
    }

    /// Opens the context menu at a widget-local position.
    pub fn open_context_menu(&mut self, position: Point) {
        let has_selection = self.has_selection();
        let clipboard_has_text = !crate::clipboard::ClipboardManager::text().is_empty();
        let read_only = self.config.read_only;
        self.context_menu.items = vec![
            MenuItem::new("cut", "Cut")
                .with_shortcut("Cmd+X")
                .disabled(!has_selection || read_only),
            MenuItem::new("copy", "Copy").with_shortcut("Cmd+C").disabled(!has_selection),
            MenuItem::new("paste", "Paste")
                .with_shortcut("Cmd+V")
                .disabled(read_only || !clipboard_has_text),
            MenuItem::new("select_all", "Select All").with_shortcut("Cmd+A"),
            MenuItem::new("find", "Find").with_shortcut("Cmd+F"),
            MenuItem::new("replace", "Replace").with_shortcut("Cmd+H"),
            MenuItem::new("toggle_comment", "Toggle Comment")
                .with_shortcut("Cmd+/")
                .disabled(read_only),
            MenuItem::new("fold", "Fold Block").with_shortcut("Cmd+["),
            MenuItem::new("unfold_all", "Unfold All"),
            MenuItem::new("indent", "Indent").with_shortcut("Cmd+I").disabled(read_only),
            MenuItem::new("outdent", "Outdent").with_shortcut("Cmd+[").disabled(read_only),
            MenuItem::new("duplicate", "Duplicate Line")
                .with_shortcut("Cmd+Shift+D")
                .disabled(read_only),
            MenuItem::new("delete_line", "Delete Line")
                .with_shortcut("Cmd+Shift+K")
                .disabled(read_only),
            MenuItem::new("sort_lines", "Sort Lines").disabled(read_only),
            MenuItem::new("select_occurrences", "Select All Occurrences")
                .with_shortcut("Cmd+Shift+L"),
            MenuItem::new("add_cursor_below", "Add Cursor Below").with_shortcut("Alt+Down"),
            MenuItem::new("complete", "Complete").with_shortcut("Ctrl+Space"),
        ];
        self.context_menu.position = position;
        self.context_menu.selected =
            self.context_menu.items.iter().position(|item| !item.disabled).unwrap_or(0);
        self.context_menu.visible = true;
        self.completion.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes the context menu.
    pub fn close_context_menu(&mut self) {
        if !self.context_menu.visible {
            return;
        }
        self.context_menu.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Activates a context-menu row by its stable id.
    pub fn activate_menu_item(&mut self, id: &str) -> bool {
        let Some(index) = self.context_menu.items.iter().position(|item| item.id == id) else {
            return false;
        };
        if self.context_menu.items.get(index).map(|item| item.disabled).unwrap_or(true) {
            return false;
        }
        self.context_menu.visible = false;
        let handled = match id {
            "cut" => self.cut(),
            "copy" => self.copy(),
            "paste" => self.paste(),
            "select_all" => {
                self.select_all();
                true
            }
            "find" => {
                self.open_find(false);
                true
            }
            "replace" => {
                self.open_find(true);
                true
            }
            "toggle_comment" => {
                self.toggle_line_comment();
                true
            }
            "fold" => {
                let line = self.cursor.head.line;
                self.toggle_fold(line)
            }
            "unfold_all" => {
                self.unfold_all();
                true
            }
            "indent" => {
                self.indent_selection();
                true
            }
            "outdent" => {
                self.outdent_selection();
                true
            }
            "duplicate" => {
                self.duplicate_line();
                true
            }
            "delete_line" => {
                self.delete_line();
                true
            }
            "sort_lines" => {
                self.sort_lines();
                true
            }
            "select_occurrences" => {
                self.select_all_occurrences();
                true
            }
            "add_cursor_below" => self.add_cursor_below(),
            "complete" => self.trigger_completion() > 0,
            _ => false,
        };
        self.base.request_layout();
        self.base.request_redraw();
        handled
    }

    /// Moves the context-menu highlight by `delta` rows, wrapping around.
    pub fn move_menu_selection(&mut self, delta: isize) -> bool {
        if !self.context_menu.visible || self.context_menu.items.is_empty() {
            return false;
        }
        let count = self.context_menu.items.len() as isize;
        let current = self.context_menu.selected.min(count as usize - 1) as isize;
        self.context_menu.selected = (current + delta).rem_euclid(count) as usize;
        self.base.request_redraw();
        true
    }

    // ── Tokenization ────────────────────────────────────────────────────────

    /// Returns the token spans of a single line.
    ///
    /// Uses the installed highlighter when present, otherwise the built-in
    /// lexer. Spans from a third-party highlighter are clamped to `char`
    /// boundaries and to the line length, so drawing can never panic.
    pub fn tokens_on_line(&self, line: usize) -> Vec<TokenSpan> {
        let Some(text) = self.line_text(line) else { return Vec::new() };
        let raw = match &self.highlighter {
            Some(highlighter) => highlighter.highlight_line(&text),
            None => BuiltinHighlighter::new(self.config.language).highlight_line(&text),
        };
        let length = text.len();
        let mut spans: Vec<TokenSpan> = Vec::with_capacity(raw.len());
        for mut span in raw {
            span.start = floor_char_boundary(&text, span.start.min(length));
            span.end = floor_char_boundary(&text, span.end.min(length));
            if span.end > span.start {
                spans.push(span);
            }
        }
        spans.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));
        super::syntax::merge_adjacent(spans)
    }

    /// Returns the token colour for a span.
    pub fn token_color(&self, span: &TokenSpan) -> Color {
        self.palette.color_for(span.kind)
    }

    /// Returns the token kind at a character position, if styled.
    pub fn token_kind_at(&self, position: TextPosition) -> TokenKind {
        let Some(line) = self.line_text(position.line) else { return TokenKind::Plain };
        let Some(span) = self.tokens_on_line(position.line).into_iter().find(|span| {
            position.column >= char_len_of(&line[..span.start])
                && position.column < char_len_of(&line[..span.end])
        }) else {
            return TokenKind::Plain;
        };
        span.kind
    }

    // ── Input plumbing ──────────────────────────────────────────
    //
    // The keyboard, IME, pointer and wheel layer lives in `input.rs`; it only
    // drives the public editing commands declared above.

    /// CONTROL modifier bit used by this platform layer.
    pub(crate) const MODIFIER_CONTROL: u32 = 2;
}

// ─────────────────────────────────────────────────────────────────────────────
// Event handling
// ─────────────────────────────────────────────────────────────────────────────

impl EventHandler for CodeEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => self.dispatch_key(*key, *modifiers),
            Event::KeyRelease { .. } => {
                // Text-producing virtual keyboards deliver payload via
                // `TextInput`/`ImeCommit`; a key release never mutates the buffer.
            }
            Event::TextInput { text } => self.input_text(text),
            Event::ImeCommit { text } => self.input_text(text),
            Event::ImePreedit { .. } => {
                // Composition is host-owned. Redraw so the host can anchor its
                // candidate window against a stable caret.
                self.base.request_redraw();
            }
            Event::MousePress { pos, button } if *button == 1 => self.pointer_press(*pos),
            Event::MouseRelease { .. } => self.pointer_release(),
            Event::MouseMove { pos } => self.pointer_drag(*pos),
            Event::MouseDoubleClick { pos, .. } => {
                self.select_word_at_point(*pos);
            }
            Event::MouseEnter { .. } | Event::MouseLeave { .. } => {
                self.base.request_redraw();
            }
            Event::Wheel { delta, modifiers } => self.handle_wheel(*delta, *modifiers),
            Event::TouchBegin { pos, .. } => self.pointer_press(*pos),
            Event::TouchMove { pos, .. } => self.pointer_drag(*pos),
            Event::TouchEnd { .. } => self.pointer_release(),
            Event::LongPress { pos } => {
                self.select_word_at_point(*pos);
            }
            Event::Resize { .. } => {
                self.refresh_visible_rows();
                self.base.request_layout();
            }
            _ => {}
        }
    }
}

impl CodeEditor {
    fn handle_wheel(&mut self, delta: Point, modifiers: u32) {
        if modifiers & Self::MODIFIER_CONTROL != 0 {
            // Ctrl+wheel is the universal editor font-size gesture.
            let step = if delta.y < 0 { 1.0 } else { -1.0 };
            let size = (self.config.font_size + step).clamp(6.0, 48.0);
            if size != self.config.font_size {
                self.config.font_size = size;
                self.config.line_advance = (size * 1.35).max(size + 2.0);
                self.refresh_visible_rows();
                self.base.request_layout();
                self.base.request_redraw();
            }
            return;
        }
        if delta.y != 0 {
            let lines = (delta.y as f32 / self.line_height()).round();
            if lines.abs() >= 1.0 {
                self.scroll_by(lines as isize);
            }
        } else if delta.x != 0 {
            let columns = (delta.x as f32 / self.cell_width()).round();
            if columns.abs() >= 1.0 {
                self.scroll_columns_by(columns as isize);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Widget integration
// ─────────────────────────────────────────────────────────────────────────────

impl Widget for CodeEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// `CodeEditor` paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    fn size_hint(&self) -> Size {
        Size::new(640, 400)
    }

    fn accessible_name(&self) -> String {
        let tooltip = self.tooltip().trim();
        if tooltip.is_empty() {
            format!("Code editor ({})", self.language_name())
        } else {
            tooltip.to_string()
        }
    }

    fn accessible_description(&self) -> String {
        format!(
            "CodeEditor {} lines {}, caret {}:{}",
            self.language_name(),
            self.line_count(),
            self.cursor.head.line + 1,
            self.cursor.head.column + 1
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Free helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns whether `candidate` closes the bracket `ch`.
fn closes_bracket(ch: char, candidate: char) -> bool {
    matches!((ch, candidate), ('(', ')') | ('[', ']') | ('{', '}'))
}

/// Returns whether `candidate` opens the bracket `ch`.
fn opens_bracket(ch: char, candidate: char) -> bool {
    matches!((ch, candidate), (')', '(') | (']', '[') | ('}', '{'))
}

/// Returns the number of decimal digits of `value` (at least 1).
pub(crate) fn decimal_digits(value: usize) -> usize {
    let mut digits = 1usize;
    let mut rest = value;
    while rest >= 10 {
        rest /= 10;
        digits += 1;
    }
    digits
}

/// Returns the largest `char` boundary at or below `byte`.
fn floor_char_boundary(text: &str, byte: usize) -> usize {
    let mut offset = byte.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Returns the character length of `text`.
fn char_len_of(text: &str) -> usize {
    text.chars().count()
}

/// Returns the character length of the final line of `payload`.
fn trailing_line_len(payload: &str) -> usize {
    payload.rsplit('\n').next().map(|line| line.chars().count()).unwrap_or(0)
}

/// Returns whether a match satisfies the whole-word constraint.
fn whole_word_ok(
    haystack: &[char],
    start: usize,
    length: usize,
    options: SearchOptions,
    language: LanguageId,
) -> bool {
    if !options.whole_word {
        return true;
    }
    let before_ok = start == 0 || !language.is_word_char(haystack[start - 1]);
    let after = start + length;
    let after_ok = after >= haystack.len() || !language.is_word_char(haystack[after]);
    before_ok && after_ok
}
