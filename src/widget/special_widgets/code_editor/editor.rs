// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The [`CodeEditor`] widget: state machine, editing commands and input plumbing.

use super::buffer::{join_lines, split_lines, EditorModel};
use super::multicursor::{self, MultiCursor, OffsetEdit};
use super::pairs::{self, PairAction};
use super::range_command::TextRangeCommand;
use super::syntax::{BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter};
use super::types::{
    CodeEditorConfig, CompletionSource, CompletionState, ContextMenuState, Cursor, CursorGoal,
    DiagnosticMarker, DocumentCompletions, DocumentScale, EditorBuffer, FindState, FoldRegion,
    InlineDiagnostic, MarkerSeverity, MenuItem, ReadOnlySpan, SearchMatch, SearchOptions,
    SyntaxPalette, TextPosition, TokenKind, TokenSpan, ViewportSnapshot, VisualLine,
    MAX_COMPLETIONS,
};
use crate::compat::MiniVec;
use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, TextSnapshotCommand, UndoCommand, UndoStack};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{BaseWidget, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
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
    /// Per-language highlighters, consulted before the global override and the
    /// built-in lexer.
    ///
    /// This is the `D1` registry: one widget may show three views in three
    /// languages, and each view must be lexed by the engine registered for *its*
    /// language rather than by a single editor-wide highlighter.
    language_highlighters: Vec<(LanguageId, Box<dyn SyntaxHighlighter>)>,
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

    /// The horizontal advance of one character cell, **measured from the backend's font**.
    ///
    /// `None` until the first [`CodeEditor::measure_cell_width`] call, which has to happen inside
    /// `draw` because font metrics belong to the backend. Before then `cell_width()` falls back to the
    /// configured `space_advance`. A `Cell` because the paint path reads this for every glyph on every
    /// line while holding `&self`.
    pub(crate) measured_cell_width: core::cell::Cell<Option<f32>>,
    /// The geometry [`CodeEditor::measured_cell_width`] was measured against.
    ///
    /// Re-measured when the geometry changes rather than every frame, so a window drag does not run
    /// `measure_text` per paint — and so a DPI change (which arrives as a new geometry) does.
    pub(crate) measured_geometry: Option<Rect>,

    pub(crate) find: FindState,
    pub(crate) completion: CompletionState,
    pub(crate) context_menu: ContextMenuState,

    pub(crate) markers: Vec<DiagnosticMarker>,
    /// Cached answer to "does the clipboard hold text?", for menu enablement.
    ///
    /// `open_context_menu` used to ask the platform directly. On desktop that read
    /// is the **machine-global OS clipboard**, whose handle is exclusive
    /// (`OpenClipboard`): it can block, it can fail, and it returns an empty string
    /// while another process holds it — so a menu row could be disabled by a
    /// transient failure. A query that only needs a boolean must not perform I/O.
    ///
    /// Three writers keep this honest, so it is a cache and not a guess:
    ///
    /// * [`CodeEditor::set_clipboard_has_text`] — the host, which knows the truth
    ///   (subscribe to the platform's clipboard change notification);
    /// * `copy` / `cut` — they just wrote text, so the answer is `true` by
    ///   construction;
    /// * `paste` — it just read the clipboard, so it records what it saw.
    ///
    /// `None` means "not known yet": `open_context_menu` probes the platform once
    /// and caches the result, so the common case (no host wiring) costs one read per
    /// unknown state rather than one per menu open.
    clipboard_has_text: Option<bool>,
    /// Document ranges that refuse edits while the rest of the buffer stays
    /// editable.
    ///
    /// This is the `A4` layer of the designer contract: the generated widget tree
    /// is locked while the property area around it can still be typed into. The
    /// whole-buffer switch in [`CodeEditorConfig::read_only`] cannot express that
    /// split, which is why this exists separately rather than as a flag.
    pub(crate) read_only_spans: Vec<ReadOnlySpan>,
    /// Lines whose fold marker is currently collapsed, for O(1) gutter queries.
    folded_lines: MiniVec<usize>,
    /// Visual rows before each document line, for O(1) row ↔ line conversion.
    ///
    /// `visual_row_prefix[i]` is the number of visual rows contributed by visible
    /// document lines before line `i`, so the row a line starts at is an array read
    /// and the reverse is a binary search. It is derived state, rebuilt by
    /// [`CodeEditor::rebuild_visual_row_prefix`] whenever the things it depends on
    /// change: the line index, the folds, wrapping, and the cell width that decides
    /// how many segments a line wraps into. Without it,
    /// [`CodeEditor::visual_row_to_line`] walks forward from the top of the document
    /// on every hit-test and every scroll, which is `B4`'s defect.
    visual_row_prefix: Vec<usize>,
    /// Per-line lexer entry state, so highlighting is incremental rather than
    /// re-derived from line 0 on every frame.
    ///
    /// `line_states[i]` is the [`LineState`] line `i` **starts** in, and the
    /// vector always holds `line_count() + 1` entries: the extra trailing entry
    /// is the state the document ends in. Two invariants make this cheap:
    ///
    /// * a highlighter line whose entry state is [`LineState::Clean`] also leaves
    ///   the next line clean, so a whole clean run needs no further work;
    /// * after an edit only the lines from the change down to the first line whose
    ///   entry state matches what was already recorded must be re-lexed.
    line_states: Vec<LineState>,
    /// How many lines of the entry-state chain have actually been computed.
    ///
    /// `line_states` is a dense vector whose untouched tail holds `Clean`
    /// defaults, so length alone cannot say which entries are real; this counter
    /// does. An edit resets it to the first affected line so the chain is
    /// re-derived from there.
    walked_lines: usize,
    /// Token spans per line, filled lazily and invalidated with `line_states`.
    line_tokens: Vec<Option<Vec<TokenSpan>>>,
    /// Per-strip-row minimap summaries, rebuilt on edits and read while painting.
    ///
    /// This is what keeps the minimap's per-frame cost off the document size:
    /// the summary is proportional to the strip height, so a million-line file
    /// paints as fast as a fifty-line one.
    pub(crate) minimap_buckets: Vec<MinimapBucket>,
    /// Line count [`CodeEditor::minimap_buckets`] was built from.
    ///
    /// An edit that adds or removes lines invalidates the bucket boundaries even
    /// when the bucket count happens to come out the same, so the count is
    /// tracked separately from the vector length.
    minimap_line_count: usize,
    /// What the editor has turned off because the document is very large.
    ///
    /// Chosen when the text is set so a level cannot change mid-scroll, and
    /// reported in the status bar so the reduced behaviour is visible rather than
    /// mysterious — a silently disabled feature reads as a bug.
    pub(crate) scale: DocumentScale,
    pub(crate) mouse_selecting: bool,
    /// The caret's blink state, advanced by [`CodeEditor::tick`].
    ///
    /// A code editor's caret is the single strongest signal that the buffer will receive typing, and
    /// a solid one reads as a frozen selection instead. The tempo and phase come from
    /// [`crate::style::CursorBlink`] so this caret cannot drift from the one in a text field.
    pub(crate) caret_blink: crate::style::CursorBlink,
    undo_stack: UndoStack,
    restoring_history: bool,
    /// Bytes retained by the most recent undo checkpoint.
    ///
    /// Recorded at push time so a test can assert the history cost tracks the edit
    /// rather than the document. Nothing reads it in production, hence `cfg(test)`
    /// on the accessor rather than on the field — keeping the field unconditional
    /// avoids a cfg-shaped hole in the struct initialiser.
    last_checkpoint_bytes: usize,

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
            language_highlighters: Vec::new(),
            completion_source: Box::new(DocumentCompletions),
            palette: SyntaxPalette::for_active_appearance(),
            cursor: Cursor::default(),
            extra_cursors: Vec::new(),
            goal: CursorGoal::default(),
            scroll_line: 0,
            scroll_column: 0,
            scroll_visual_row: 0,
            visible_rows: 0,
            measured_cell_width: core::cell::Cell::new(None),
            measured_geometry: None,
            find: FindState::default(),
            completion: CompletionState::default(),
            context_menu: ContextMenuState::default(),
            markers: Vec::new(),
            clipboard_has_text: None,
            read_only_spans: Vec::new(),
            folded_lines: MiniVec::new(),
            visual_row_prefix: Vec::new(),
            line_states: Vec::new(),
            line_tokens: Vec::new(),
            walked_lines: 0,
            minimap_buckets: Vec::new(),
            minimap_line_count: 0,
            scale: DocumentScale::Full,
            mouse_selecting: false,
            caret_blink: crate::style::CursorBlink::new(),
            undo_stack: UndoStack::new(),
            restoring_history: false,
            last_checkpoint_bytes: 0,
            text_changed: Signal1::new(),
            cursor_moved: Signal1::new(),
            selection_changed: Signal1::new(),
            tab_changed: Signal1::new(),
            search_changed: Signal1::new(),
            fold_changed: Signal1::new(),
            completion_changed: Signal1::new(),
        };
        // An editable editor is an active insertion point the moment it exists, so its caret blinks
        // from construction rather than waiting for a focus event this widget has no notion of.
        if !editor.config.read_only {
            editor.caret_blink.start();
        }
        editor.refresh_derived_state();
        editor
    }

    /// Returns the active configuration.
    pub fn config(&self) -> &CodeEditorConfig {
        &self.config
    }

    /// Returns `true` when the editor rejects mutations.
    ///
    /// This is the *effective* answer, not just the caller's setting: a document
    /// large enough to be view-only also rejects mutations, and a host asking
    /// "can this be edited?" needs to hear about both. [`Self::is_editable`] is
    /// the negation, named for the positive question.
    pub fn is_read_only(&self) -> bool {
        !self.is_editable()
    }

    /// Enables or disables editing.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.config.read_only = read_only;
        // A read-only editor shows no caret at all, so there is nothing to animate; an editable one
        // resumes blinking from a visible caret.
        if read_only {
            self.caret_blink.stop();
        } else {
            self.caret_blink.start();
        }
        self.base.request_redraw();
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention. The caret here was drawn at full strength
    /// unconditionally, so a host had no reason to keep scheduling frames and the caret read as a
    /// frozen marker rather than a live insertion point.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.is_editable() {
            return false;
        }
        let running = self.caret_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }

    /// Whether the caret is in the visible half of its blink cycle.
    ///
    /// Exposed so a test can assert the blink without capturing pixels, and so a host that draws
    /// its own overlay caret can follow the same phase.
    pub fn is_caret_visible(&self) -> bool {
        self.caret_blink.is_visible()
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
        // The cached entry-state chain was produced by the previous highlighter,
        // so it is meaningless for this one.
        self.invalidate_line_states();
        self.base.request_redraw();
    }

    /// Removes a custom highlighter, restoring the built-in lexer.
    pub fn clear_highlighter(&mut self) {
        self.highlighter = None;
        self.invalidate_line_states();
        self.base.request_redraw();
    }

    /// Registers a highlighter for one language, replacing any previous one.
    ///
    /// A per-language registration wins over [`Self::set_highlighter`]'s global
    /// override for buffers whose language matches, so a host can plug a real
    /// engine into one language while the rest keep the built-in lexer. The cache
    /// is dropped when the active buffer's language is the one updated.
    pub fn register_language_highlighter(
        &mut self,
        language: LanguageId,
        highlighter: Box<dyn SyntaxHighlighter>,
    ) {
        match self.language_highlighters.iter_mut().find(|(registered, _)| *registered == language)
        {
            Some(slot) => slot.1 = highlighter,
            None => self.language_highlighters.push((language, highlighter)),
        }
        if self.active_language() == language {
            self.invalidate_line_states();
        }
        self.base.request_redraw();
    }

    /// Removes the highlighter registered for one language.
    ///
    /// Returns `true` when one was registered. The active buffer's cache is dropped
    /// when its language is the one removed, so what is on screen is re-lexed by the
    /// built-in lexer immediately rather than at the next edit.
    pub fn unregister_language_highlighter(&mut self, language: LanguageId) -> bool {
        let before = self.language_highlighters.len();
        self.language_highlighters.retain(|(registered, _)| *registered != language);
        let removed = self.language_highlighters.len() != before;
        if removed && self.active_language() == language {
            self.invalidate_line_states();
            self.base.request_redraw();
        }
        removed
    }

    /// Returns the languages that have a registered highlighter.
    pub fn registered_languages(&self) -> Vec<LanguageId> {
        self.language_highlighters.iter().map(|(language, _)| *language).collect()
    }

    /// Returns the language name reported by the highlighter that would lex `language`.
    ///
    /// This answers "what engine is actually in force?" without invoking it, which
    /// is what a status bar or a capability panel needs.
    pub fn highlighter_name_for(&self, language: LanguageId) -> String {
        if let Some((_, highlighter)) =
            self.language_highlighters.iter().find(|(registered, _)| *registered == language)
        {
            return highlighter.language_name().to_string();
        }
        if let Some(highlighter) = &self.highlighter {
            return highlighter.language_name().to_string();
        }
        language.name().to_string()
    }

    /// Installs a custom completion provider.
    pub fn set_completion_source(&mut self, source: Box<dyn CompletionSource>) {
        self.completion_source = source;
    }

    /// Returns the level of feature reduction in force for this document.
    ///
    /// A host can read this to report why a feature is unavailable. It is decided
    /// when the text is set, so it never changes while the user scrolls.
    pub fn document_scale(&self) -> DocumentScale {
        self.scale
    }

    /// Returns whether a mutation is currently permitted.
    ///
    /// Combines the caller's setting with the level the document size imposed, so
    /// a very large file cannot be edited even when `read_only` is off.
    pub fn is_editable(&self) -> bool {
        !self.config.read_only && self.scale.allows_editing()
    }

    // ── Range-level read-only (A4) ──────────────────────────────────────────

    /// Returns the document ranges that currently refuse edits.
    pub fn read_only_spans(&self) -> &[ReadOnlySpan] {
        &self.read_only_spans
    }

    /// Replaces the set of locked ranges.
    ///
    /// Empty spans are dropped, so a host may build them from positions without
    /// having to check whether the range collapsed.
    pub fn set_read_only_spans(&mut self, spans: Vec<ReadOnlySpan>) {
        self.read_only_spans = spans.into_iter().filter(|span| !span.is_empty()).collect();
        self.base.request_redraw();
    }

    /// Adds one locked range.
    pub fn add_read_only_span(&mut self, span: ReadOnlySpan) {
        if !span.is_empty() {
            self.read_only_spans.push(span);
            self.base.request_redraw();
        }
    }

    /// Removes every locked range.
    pub fn clear_read_only_spans(&mut self) {
        if !self.read_only_spans.is_empty() {
            self.read_only_spans.clear();
            self.base.request_redraw();
        }
    }

    /// Returns whether a single position may be edited.
    ///
    /// A position inside a locked span is not editable even when the buffer as a
    /// whole is. Useful for a host that wants to refuse a drag *before* it starts,
    /// or to grey out a command.
    pub fn is_position_editable(&self, position: TextPosition) -> bool {
        self.is_editable() && !self.read_only_spans.iter().any(|span| span.contains(position))
    }

    /// Returns whether replacing `[start, end)` would touch a locked range.
    ///
    /// This is the guard every mutation path consults: an edit is refused when it
    /// overlaps any locked span, so a selection cannot be used to delete generated
    /// lines. A collapsed range is an insertion point and is refused exactly when
    /// it lands inside a locked span.
    pub fn is_range_editable(&self, start: TextPosition, end: TextPosition) -> bool {
        self.is_editable() && !self.read_only_spans.iter().any(|span| span.overlaps(start, end))
    }

    /// Shifts every locked span after an edit that changed `line_count` lines
    /// starting at `first_line`.
    ///
    /// Locked ranges are expressed in document coordinates, so an edit above one
    /// moves it. A host that edits its own buffer through this widget gets the
    /// shift applied automatically by the edit paths; this entry point exists for
    /// a host that mutates the document out of band and then tells the editor.
    ///
    /// `lines_delta` is positive when lines were inserted and negative when they
    /// were removed.
    pub fn shift_read_only_spans_after(&mut self, first_line: usize, lines_delta: isize) {
        if self.read_only_spans.is_empty() || lines_delta == 0 {
            return;
        }
        for span in &mut self.read_only_spans {
            if span.start_line >= first_line {
                span.start_line = shift_index(span.start_line, lines_delta);
            }
            if span.end_line >= first_line {
                span.end_line = shift_index(span.end_line, lines_delta);
            }
        }
        self.read_only_spans.retain(|span| !span.is_empty());
        self.base.request_redraw();
    }

    /// Applies the internal shift an edit performed at `first_line` implies.
    ///
    /// Called after a committed edit whose line range is known, so a host that
    /// installed spans through [`Self::add_read_only_span`] does not have to track
    /// line numbers by hand. A span that the edit landed *inside* is left alone:
    /// its text was rewritten by the edit, and moving it would lock unrelated
    /// lines. The host re-derives spans for that case.
    fn shift_read_only_spans_for_edit(
        &mut self,
        first_line: usize,
        lines_now: usize,
        lines_before: usize,
    ) {
        let delta = lines_now as isize - lines_before as isize;
        if delta == 0 || self.read_only_spans.is_empty() {
            return;
        }
        let edit_end = first_line + lines_before;
        for span in &mut self.read_only_spans {
            // A span that starts strictly after the edited block moves wholesale.
            if span.start_line >= edit_end {
                span.start_line = shift_index(span.start_line, delta);
                span.end_line = shift_index(span.end_line, delta);
            } else if span.end_line >= edit_end {
                // The span straddles the edit: its tail moved.
                span.end_line = shift_index(span.end_line, delta);
            }
        }
        self.read_only_spans.retain(|span| !span.is_empty());
    }

    /// Returns the active language name.
    ///
    /// Resolves the language the same way highlighting does: a per-buffer override
    /// wins, then the editor configuration. The name reported is the one the engine
    /// that will actually lex the buffer gives, so a plugin that renames itself is
    /// reflected here rather than the bare language family name.
    pub fn language_name(&self) -> String {
        self.highlighter_name_for(self.active_language())
    }

    /// Returns the language in force for the active buffer.
    ///
    /// A buffer's [`EditorBuffer::language`] override wins over
    /// [`CodeEditorConfig::language`]. This is what makes one widget host several
    /// tabs of different languages (whitepaper §14.2): the field existed before but
    /// nothing consulted it, so every tab was lexed as the configured language.
    pub fn active_language(&self) -> LanguageId {
        self.model
            .borrow()
            .all_buffers
            .get(self.model.borrow().active_tab)
            .and_then(|buffer| buffer.language)
            .unwrap_or(self.config.language)
    }

    /// Sets the built-in language and resets indentation to its default.
    ///
    /// The setting applies to the active buffer when it has a language override,
    /// and to the editor default otherwise, so a multi-language host does not need
    /// two entry points.
    pub fn set_language(&mut self, language: LanguageId) {
        self.config.language = language;
        self.config.tab_width = language.default_tab_width();
        let active = self.model.borrow().active_tab;
        if let Some(buffer) = self.model.borrow_mut().all_buffers.get_mut(active) {
            buffer.language = Some(language);
        }
        // A different language lexes the same text differently, so every cached
        // state and span is stale.
        self.invalidate_line_states();
        self.base.request_redraw();
    }

    /// Sets the language of the buffer at `index` without switching to it.
    ///
    /// Returns `false` for an out-of-range index. When the buffer is active the
    /// cached lexer state is dropped, because what is on screen just changed
    /// language.
    pub fn set_buffer_language(&mut self, index: usize, language: Option<LanguageId>) -> bool {
        let active = self.model.borrow().active_tab;
        let changed = {
            let mut model = self.model.borrow_mut();
            match model.all_buffers.get_mut(index) {
                Some(buffer) => {
                    buffer.language = language;
                    true
                }
                None => false,
            }
        };
        if changed && index == active {
            self.invalidate_line_states();
            self.base.request_redraw();
        }
        changed
    }

    /// Returns the language recorded for the buffer at `index`.
    pub fn buffer_language(&self, index: usize) -> Option<LanguageId> {
        self.model.borrow().all_buffers.get(index).and_then(|buffer| buffer.language)
    }

    /// Returns the keyword set of the language in force for the active buffer.
    pub fn keywords(&self) -> &'static [&'static str] {
        self.active_language().keywords()
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
        // Wholesale replacement invalidates every cached state; there is no
        // meaningful "from line N" because no line survived unchanged. Locked
        // spans are positions in the *old* document, so they are dropped rather
        // than silently re-anchored to unrelated text; a host that replaces the
        // document also rebuilds its spans.
        self.read_only_spans.clear();
        self.invalidate_line_states();
        self.minimap_line_count = 0;
        self.scale = DocumentScale::for_lines(self.line_count());
        // A level that forbids editing has to be reflected in the buffer the
        // widget already gates on, so every mutation entry point agrees without
        // repeating the check.
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

    /// Returns the line containing `offset` in the joined document.
    pub fn line_of_byte_offset(&self, offset: usize) -> usize {
        self.model.borrow().line_of_byte_offset(offset)
    }

    /// Returns the character length of a line, or 0 when out of range.
    pub fn line_len(&self, line: usize) -> usize {
        self.model.borrow().line_len(line)
    }

    // ── Editing primitives ──────────────────────────────────────────────────

    /// Inserts text at the caret, replacing any selection.
    pub fn insert(&mut self, text: &str) {
        if !self.is_editable() || text.is_empty() {
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
        self.splice_and_record(start, end, payload, head);
    }

    /// Splices the model and records the change as one undoable range edit.
    ///
    /// This is the code editor's only edit path: it derives the changed byte
    /// range from the same splice that applies it, so the history entry and the
    /// document cannot disagree. The alternative — snapshotting the whole
    /// document before and after — costs two full copies per keystroke.
    fn splice_and_record(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        payload: &str,
        head: TextPosition,
    ) {
        // The single choke point for every range edit, so the locked-span guard
        // lives here: a splice that overlaps a generated region is refused before
        // anything is recorded or written. `is_editable` is already checked by the
        // public entry points, but they cannot see the clamped positions this path
        // derives, and a caller that passed through `splice_range` unchecked would
        // otherwise slip a generated-line edit past the lock.
        if !self.is_range_editable(start, end) {
            return;
        }
        // Capture the text being replaced **before** the splice runs. Slicing the
        // post-edit buffer with a pre-edit offset would record whatever now sits
        // there, so undo would restore the edit instead of the original — which is
        // exactly the defect this path had.
        let removed = {
            let mut model = self.model.borrow_mut();
            let start = model.clamp_position(start);
            let end = model.clamp_position(end);
            let from = model.total_byte_offset(start);
            let to = model.total_byte_offset(end).max(from);
            let text = model.text.borrow();
            text.get(from..to.min(text.len())).unwrap_or_default().to_string()
        };
        let (rebuilt, start_offset, removed_len) =
            self.model.borrow_mut().splice_with_range(start, end, payload);
        let checkpoint = Checkpoint {
            range: Some((start_offset, removed_len, payload.to_string())),
            removed: Some(removed),
            before: None,
        };
        self.record_and_settle_inner(rebuilt, checkpoint, head);
    }

    /// Applies an edit that has already been computed.
    ///
    /// Callers either spliced the model themselves (`splice_payload`,
    /// `splice_range`) or rebuilt the text by hand (line commands); this writes
    /// the result back so both paths agree, then records one undo checkpoint.
    fn commit_edit(&mut self, before: String, after: String, head: TextPosition) {
        // A line transform that changes nothing must not mark the document dirty
        // or fire `text_changed`: the text is byte-for-byte what it already was,
        // so there is no edit to commit or undo.
        if before == after {
            return;
        }
        // A caller that rebuilt the text by hand has no offset to report, so the
        // range is recovered by diffing the two texts. The replaced text comes from
        // `before` directly: the range is in pre-edit coordinates, so slicing the
        // post-edit buffer with it would record the wrong bytes.
        let range = diff_range(&before, &after);
        // Line commands bypass `splice_and_record`, so the locked-span guard has to
        // be applied to the *diff* of the two texts: the byte span the command
        // rewrote is exactly what its positions would be checked against. Without
        // this, `delete_line` on a generated line would succeed.
        if let Some((start, removed_len, inserted)) = &range {
            // The touched region is `[start, start + removed_len + inserted.len())`
            // in the pre-edit text; the tails of that region are what the guard
            // needs as positions.
            let guard_start = position_in_text(&before, *start);
            let guard_end = position_in_text(&before, start + removed_len + inserted.len());
            // `start + removed_len + inserted.len()` can exceed the pre-edit
            // length, so `guard_end` is clamped to the document by the helper.
            if !self.is_range_editable(guard_start, guard_end) {
                return;
            }
        }
        let removed = range.as_ref().map(|(start, removed_len, _)| {
            before.get(*start..start + removed_len).unwrap_or_default().to_string()
        });
        let checkpoint = Checkpoint { range, removed, before: Some(before) };
        self.record_and_settle_inner(Some(after), checkpoint, head);
    }

    /// Writes the spliced text back, records one checkpoint and refreshes state.
    ///
    /// `after` is the rebuilt document when the splice had to rebuild it, or
    /// `None` when the model already applied the edit in place. `checkpoint`
    /// carries whatever the caller already knows about the edit: the byte range it
    /// replaced, the text that was there, and the pre-edit document. All three are
    /// optional because the edit paths know different amounts — typing knows the
    /// range, a line command knows only the before/after pair. The most specific
    /// description available is used, and anything missing is reconstructed from
    /// the buffer before it is overwritten.
    fn record_and_settle_inner(
        &mut self,
        after: Option<String>,
        checkpoint: Checkpoint,
        head: TextPosition,
    ) {
        // An in-place edit already left the model correct, so there is nothing to
        // write back. The document is materialised once, for the `text_changed`
        // signal: subscribers receive the new text, and that contract is what makes
        // the signal useful. The comparison against the buffer is only meaningful
        // for the rebuilt path — an in-place edit is applied by definition.
        let (after, already_applied) = match after {
            Some(text) => {
                let same = {
                    let model = self.model.borrow();
                    let current = model.text.borrow();
                    current.as_str() == text.as_str()
                };
                (text, same)
            }
            None => (self.model.borrow().text.borrow().clone(), true),
        };
        // Everything below that reads the "before" state has to happen before the
        // write. The recorded range is in pre-edit coordinates, so slicing the
        // post-edit buffer with it would capture whatever now sits there — for
        // `a-b-c` → `a+b+c` that turned the undo entry into the edited text, and
        // undoing restored the edit instead of the original.
        let removed = match checkpoint.removed {
            Some(text) => Some(text),
            None => match &checkpoint.range {
                Some((start, removed_len, _)) => {
                    let model = self.model.borrow();
                    let text = model.text.borrow();
                    let end = start.saturating_add(*removed_len).min(text.len());
                    Some(
                        if *start <= end
                            && text.is_char_boundary(*start)
                            && text.is_char_boundary(end)
                        {
                            text[*start..end].to_string()
                        } else {
                            String::new()
                        },
                    )
                }
                None => None,
            },
        };
        let before_snapshot = if checkpoint.before.is_some() {
            checkpoint.before
        } else if already_applied {
            None
        } else {
            Some(self.model.borrow().text.borrow().clone())
        };
        if !already_applied {
            self.model.borrow_mut().set_text(after.clone());
        }
        // The match cache can be updated from the edit's line range instead of a
        // full rescan. `dirty` is `(first_line, lines_now, lines_before)`: the
        // lines the edit replaced at that position. It is `None` when the caller
        // could not describe the change, which falls back to a full rescan.
        let dirty = match (&checkpoint.range, &removed) {
            (Some((start, _removed_len, inserted)), Some(removed_text)) => {
                let first = self.line_of_byte_offset(*start);
                let lines_now = inserted.matches('\n').count() + 1;
                let lines_before = removed_text.matches('\n').count() + 1;
                Some((first, lines_now, lines_before))
            }
            _ => None,
        };
        // A locked range that sits *below* the edit has moved by the net line
        // count. Applying the shift here — where the edit's line range is already
        // derived for the match cache — keeps the lock statements and the match
        // cache from disagreeing about where the edit landed.
        if let Some((first, lines_now, lines_before)) = dirty {
            self.shift_read_only_spans_for_edit(first, lines_now, lines_before);
        }
        if !self.restoring_history {
            self.push_checkpoint(checkpoint.range, removed, before_snapshot);
        }
        self.cursor = Cursor { head: self.clamp_position(head), anchor: self.clamp_position(head) };
        self.goal.active = false;
        // Re-lex only from the first line the edit could have changed. The
        // caret lands at or after the edited region in every caller, so taking
        // the minimum of the two ends is a safe lower bound: re-lexing a line or
        // two too many is harmless, missing one would leave stale colours.
        self.invalidate_line_states_from(self.cursor.head.line.min(head.line));
        // A keystroke inside the document is a modification by definition, so the
        // flag is set directly rather than re-derived by comparing the whole
        // document against its save point.
        self.model.borrow_mut().dirty = true;
        // A one-line edit cannot change any other line's bucket contribution
        // unless the longest line in its bucket was the one edited, so re-folding
        // the edited line is enough. A line count change invalidates the whole
        // bucket map and is caught by `prepare_minimap` on the next frame.
        self.patch_minimap_line(self.cursor.head.line.min(head.line));
        self.refresh_derived_state_after(dirty);
        self.text_changed.emit(after);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Pushes one undo checkpoint for an edit.
    ///
    /// A range edit stores only the text it replaced; anything else falls back to
    /// `TextSnapshotCommand`, which stores the whole document twice. Typing always
    /// takes the range path, so a keystroke's history cost is the size of the
    /// edit rather than the size of the file.
    fn push_checkpoint(
        &mut self,
        range: Option<(usize, usize, String)>,
        removed: Option<String>,
        before_snapshot: Option<String>,
    ) {
        let target = self.model.borrow().text.clone();
        self.last_checkpoint_bytes = match (range, removed) {
            (Some((start, _removed_len, inserted)), Some(removed)) => {
                let bytes = removed.len() + inserted.len();
                self.undo_stack.push(Box::new(TextRangeCommand::new(
                    target,
                    start,
                    removed,
                    inserted,
                    "code_editor_edit",
                )));
                bytes
            }
            // No usable range (the caller could not describe the change, or the
            // recorded offsets do not fit the pre-edit text): fall back to the
            // whole-document snapshot, which is correct, if costly.
            (_, _) => {
                let current = target.borrow().clone();
                let before = before_snapshot.unwrap_or_else(|| current.clone());
                let bytes = before.len() + current.len();
                self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                    target,
                    before,
                    current,
                    "code_editor_edit",
                )));
                bytes
            }
        };
    }

    /// Drops cached lexer state for every line from `first` down.
    ///
    /// The token cache is dropped outright because the spans themselves may
    /// have changed. The entry state of `first` is **kept**: an edit below a
    /// line cannot change what that line starts inside, so resetting it to
    /// `Clean` would make the lexer read a block-comment continuation as fresh
    /// code. `ensure_line_states` re-derives forward from there and stops as
    /// soon as a line leaves the same state it already had.
    pub(crate) fn invalidate_line_states_from(&mut self, first: usize) {
        for slot in self.line_tokens.iter_mut().skip(first) {
            *slot = None;
        }
        self.walked_lines = self.walked_lines.min(first);
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
        if !self.is_editable() {
            return;
        }
        self.splice_and_record(start, end, payload, head);
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
        if !self.is_editable() || carets.is_empty() || carets.len() != edits.len() {
            return false;
        }
        // Multi-caret is a distinct write path from `splice_and_record`, so the
        // locked-span guard has to be repeated here rather than inherited. The
        // whole batch is refused when *any* caret's edit touches a locked range:
        // applying the rest would leave the carets out of step with the document.
        if edits.iter().any(|edit| !self.is_range_editable(edit.start, edit.end)) {
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
        // Record exactly one checkpoint. `set_text` below pushes its own entry when
        // history is live, so pushing here as well would make one multi-caret edit
        // undo in two steps — the buffer would come back half-edited. The range is
        // derived from the before/after pair because a multi-caret projection
        // rewrites several disjoint spans at once and has no single offset to
        // report.
        let range = diff_range(&before, &after);
        let checkpoint = if self.restoring_history {
            None
        } else {
            let target = self.model.borrow().text.clone();
            Some(match range {
                Some((start, removed_len, inserted)) => {
                    let removed =
                        before.get(start..start + removed_len).unwrap_or_default().to_string();
                    Box::new(TextRangeCommand::new(
                        target,
                        start,
                        removed,
                        inserted,
                        "code_editor_edit",
                    )) as Box<dyn UndoCommand>
                }
                None => Box::new(TextSnapshotCommand::new(
                    target,
                    before.clone(),
                    after.clone(),
                    "code_editor_edit",
                )),
            })
        };
        // `set_text` would add a second entry of its own, so suppress it and push
        // the single checkpoint computed above instead.
        let was_restoring = self.restoring_history;
        self.restoring_history = true;
        self.model.borrow_mut().set_text(after.clone());
        self.restoring_history = was_restoring;
        if let Some(command) = checkpoint {
            self.undo_stack.push(command);
        }
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
    ///
    /// A **collapsed** caret counts as "none selected", which is what the summary above
    /// promises and what every editor does. Reading the caret's bounds literally instead
    /// made a collapsed caret sort the single line it sat on — a no-op for text that is
    /// already one line, and a silent partial sort otherwise, which is worse than either
    /// sorting everything or refusing.
    pub fn sort_lines(&mut self) {
        if !self.is_editable() {
            return;
        }
        let has_selection = self.has_selection();
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        // A trailing newline produces a final empty element. Sorting it would move
        // that empty line to the top of the document — `"b\na\n"` came back as
        // `"\na\nb"` — so it is held back and appended unchanged. It is a line
        // terminator's artefact, not a line the user can see or reorder.
        let trailing_blank = lines.last().is_some_and(|line| line.is_empty());
        let sortable = if trailing_blank { lines.len() - 1 } else { lines.len() };
        if sortable > 0 {
            // No selection: the whole sortable document. Otherwise the lines the
            // selection touches, inclusive of both ends.
            let (first, last) = if has_selection {
                (start.line.min(sortable - 1), end.line.min(sortable - 1))
            } else {
                (0, sortable - 1)
            };
            lines[first..=last].sort();
        }
        let after = join_lines(&lines);
        if after == before {
            return;
        }
        let head = TextPosition::new(sortable.saturating_sub(1), 0);
        self.commit_edit(before, after, head);
        self.cursor.anchor = TextPosition::new(0, 0);
        self.emit_cursor_and_selection();
    }

    /// Removes trailing whitespace from every line.
    pub fn trim_trailing_whitespace(&mut self) {
        if !self.is_editable() {
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
    /// Returns `true` when the character was fully handled. Pairing is governed by
    /// the explicit [`CodeEditorConfig::auto_close_brackets`] switch rather than by
    /// language: it is a formatting preference, not a claim about grammar, so it is
    /// not among the commands D2 degrades.
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
        if !self.is_editable() || self.cursor.is_collapsed() {
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
        if !self.is_editable() {
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
    ///
    /// Degrades explicitly for languages with no line-comment syntax: rather than
    /// inserting `// ` into Markdown or plain text, the command does nothing. The
    /// language is the *active buffer's*, not the editor default, so a Markdown tab
    /// and a Rust tab in the same widget behave differently.
    pub fn toggle_line_comment(&mut self) {
        if !self.is_editable() {
            return;
        }
        let Some(token) = self.active_language().line_comment_prefix() else {
            return;
        };
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
        if !self.is_editable() || direction == 0 {
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
        if !self.is_editable() {
            return false;
        }
        let before_tab = self.active_buffer();
        if self.undo_stack.undo().is_err() {
            return false;
        }
        if before_tab != self.active_buffer() {
            // Undoing a tab switch restored a different document; the locked
            // ranges belonged to the previous document and must not leak into
            // this one.
            self.read_only_spans.clear();
        }
        self.restore_history_text();
        true
    }

    /// Redoes the most recently undone edit.
    pub fn redo(&mut self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let before_tab = self.active_buffer();
        if self.undo_stack.redo().is_err() {
            return false;
        }
        if before_tab != self.active_buffer() {
            self.read_only_spans.clear();
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
        // The undo command already wrote the buffer directly (a range command
        // splices, a snapshot assigns), so `set_text` here would see an unchanged
        // document and return early — leaving the derived state stale. Rebuild the
        // line index from the buffer instead of routing through `set_text`.
        let restored = self.model.borrow().text.borrow().clone();
        self.restoring_history = true;
        self.model.borrow_mut().set_text(restored.clone());
        self.restoring_history = false;
        self.invalidate_line_states();
        self.minimap_line_count = 0;
        self.scale = DocumentScale::for_lines(self.line_count());
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

    /// Returns the horizontal scroll offset in character columns.
    ///
    /// The companion to [`Self::scroll_line`] for a host that links two panes and
    /// needs to reproduce a viewport exactly rather than approximately.
    pub fn scroll_column(&self) -> usize {
        self.scroll_column
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

    // ── Multi-instance linkage (D3) ─────────────────────────────────────────

    /// Captures where this editor is looking, for a host that links two panes.
    ///
    /// A side-by-side comparison needs both instances to agree on the viewport;
    /// the *policy* (follow the caret? follow the scroll only? lock the columns?)
    /// belongs to the host, which is why only the capture and the apply live here.
    pub fn viewport_snapshot(&self) -> ViewportSnapshot {
        ViewportSnapshot {
            first_visual_row: self.scroll_visual_row,
            scroll_column: self.scroll_column,
            caret: self.cursor.head,
        }
    }

    /// Applies a viewport snapshot from another instance.
    ///
    /// Scrolls and moves the caret without touching undo history or the modified
    /// flag: following a *linked* pane is not an edit of this buffer. The caret is
    /// clamped to this document, so linking a long file to a short one cannot put
    /// the caret past the end.
    pub fn apply_viewport(&mut self, snapshot: ViewportSnapshot) {
        self.scroll_column = snapshot.scroll_column;
        // The row is clamped through the same transform a scroll uses, so a linked
        // pane whose `scroll_line` lags the row cannot desynchronise the two.
        let visible = self.visible_rows().max(1);
        let max_row = self.visual_row_count().saturating_sub(visible);
        self.scroll_visual_row = snapshot.first_visual_row.min(max_row);
        self.scroll_line = self.visual_row_to_line(self.scroll_visual_row);
        let caret = self.clamp_position(snapshot.caret);
        let moved = caret != self.cursor.head;
        self.cursor = Cursor { head: caret, anchor: caret };
        self.goal.active = false;
        if moved {
            self.emit_cursor_and_selection();
        }
        self.base.request_redraw();
    }

    /// Returns the first visible visual row, the linked-pane coordinate.
    ///
    /// Distinct from [`Self::scroll_line`], which is a *document line*: with
    /// wrapping or folding the two differ, and a host linking panes must use the
    /// row or the panes drift apart by exactly the fold they disagree about.
    pub fn first_visual_row(&self) -> usize {
        self.scroll_visual_row
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
        self.refresh_derived_state_after(None);
    }

    /// [`Self::refresh_derived_state`] with a hint about what the edit changed.
    ///
    /// `dirty` describes the edited line range so the match cache can rescan only
    /// those lines instead of the whole document. Callers that changed the shape
    /// of the document in a way they cannot describe pass `None`, which is always
    /// correct and merely slower.
    fn refresh_derived_state_after(&mut self, dirty: Option<(usize, usize, usize)>) {
        {
            let model = self.model.borrow();
            self.folded_lines.clear();
            for region in &model.folds {
                if region.folded {
                    self.folded_lines.push(region.start_line);
                }
            }
        }
        // The document may have gained or lost lines since the last refresh, so
        // the per-line lexer cache is reconciled here rather than in `draw`.
        let count = self.line_count();
        if self.line_states.len() != count + 1 {
            self.line_states.resize(count + 1, LineState::Clean);
            self.line_tokens.resize(count, None);
        }
        self.clamp_cursor_to_visible();
        self.recompute_match_cache_incremental(dirty);
        self.refresh_visible_rows();
        // The document shape changed (lines, folds, or the wrap geometry the callers
        // above may have moved), so the row-prefix table is rebuilt here rather than
        // in the lookup, which only has `&self`.
        self.rebuild_visual_row_prefix();
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
    ///
    /// # Why this reads the model, not the cache
    ///
    /// This used to scan `folded_lines`, a *derived* cache rebuilt by
    /// `refresh_derived_state`, while [`CodeEditor::folded_line_count`] and the
    /// render path's row visibility both answered from the model. Two sources for
    /// one fact is how they drift: the cache is only as fresh as the last refresh,
    /// so a state change that had not yet triggered one could be reported
    /// differently by this predicate than by the count. The model is the single
    /// authority; the cache is a render-path detail and no longer decides public
    /// answers.
    pub fn is_line_folded(&self, line: usize) -> bool {
        self.model.borrow().folds.iter().any(|region| region.folded && region.start_line == line)
    }

    /// Returns whether a line is hidden *inside* a folded region.
    ///
    /// Distinct from [`Self::is_line_folded`], which reports the fold's own start
    /// line — the one row a collapsed region still shows. This answers the other
    /// half: is the line currently invisible because some region swallowed it.
    pub fn is_line_hidden(&self, line: usize) -> bool {
        self.model.borrow().is_hidden(line)
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

    /// Returns the document lines that are visible, folds respected.
    ///
    /// When nothing is folded this is a contiguous range, so it is produced by
    /// arithmetic instead of by filtering every line of the document. The old
    /// implementation allocated a `Vec` of the whole document on every call, and
    /// the paint path calls this several times per frame — which is what made a
    /// large file cost milliseconds per repaint even with nothing to draw.
    pub(crate) fn visible_document_lines(&self) -> Vec<usize> {
        if self.folded_lines.is_empty() {
            return (0..self.line_count()).collect();
        }
        self.model.borrow().visible_lines()
    }

    /// Returns the visible document lines within the viewport window, plus slack.
    ///
    /// This is the iterator the paint loop should use: it never materialises the
    /// document, so its cost depends on the viewport.
    pub(crate) fn viewport_document_lines(&self) -> alloc::vec::IntoIter<usize> {
        if self.folded_lines.is_empty() {
            let slack = self.visible_rows.max(1) + 1;
            let first = self.scroll_visual_row.saturating_sub(slack).min(self.line_count());
            let last = (self.scroll_visual_row + slack * 2).min(self.line_count());
            return (first..last).collect::<Vec<usize>>().into_iter();
        }
        let slack = self.visible_rows.max(1) + 1;
        let first_row = self.scroll_visual_row.saturating_sub(slack);
        let last_row = self.scroll_visual_row + slack * 2;
        let mut rows = 0usize;
        let mut out = Vec::new();
        for line in self.visible_document_lines() {
            let segments = self.wrap_segments(line);
            if rows + segments > first_row && rows <= last_row {
                out.push(line);
            }
            rows += segments;
            if rows > last_row {
                break;
            }
        }
        out.into_iter()
    }

    /// Fills `line_states` / `line_tokens` for the rows about to be painted.
    ///
    /// The paint helpers take `&self`, so the one place that can mutate the cache
    /// is here, before them. Only the on-screen window is prepared: a document
    /// with a million lines costs the same as one with fifty, which is the
    /// invariant that keeps scrolling smooth (principle: per-frame work is
    /// proportional to the viewport, never to the document).
    pub(crate) fn prepare_visible_line_cache(&mut self) {
        let budget = self.visible_rows.max(1) + 1;
        let first = self.scroll_visual_row.saturating_sub(budget);
        let last = (self.scroll_visual_row + budget * 2).min(self.line_count());
        // Advance the entry-state chain up to the first painted line. This is
        // where a multi-line construct opened far above is resolved, so it is
        // bounded by the distance from the last known-clean line, not by the
        // document length.
        self.ensure_line_states(last);
        for line in first..last {
            self.fill_tokens_on_line(line);
        }
    }

    /// Returns the cached entry state of `line`, if the lexer has reached it.
    ///
    /// The renderer uses this to tell a continuation row apart from a fresh one
    /// without re-running the lexer, which is what makes a multi-line block
    /// comment paint as one visually continuous band.
    pub fn line_state(&self, line: usize) -> LineState {
        self.line_states.get(line).copied().unwrap_or_default()
    }

    // ── Minimap summary ─────────────────────────────────────────────────────

    /// Ensures the minimap summary matches the current document and strip size.
    ///
    /// Rebuilds only when the bucket count is stale (first paint, a resize, or
    /// an edit that changed the line count). Steady-state repaints read the
    /// existing buckets, so scrolling costs nothing here.
    pub(crate) fn prepare_minimap(&mut self) {
        if !self.config.show_minimap {
            return;
        }
        let rows = self.minimap_rows();
        let count = self.line_count();
        let per_bucket_now = count.div_ceil(rows.max(1)).max(1);
        let expected = count.div_ceil(per_bucket_now);
        let stale = self.minimap_buckets.len() != expected || self.minimap_line_count != count;
        if stale {
            self.rebuild_minimap(rows);
            self.minimap_line_count = count;
        }
    }

    /// Returns how many strip rows the minimap has room for.
    fn minimap_rows(&self) -> usize {
        let rect = self.geometry();
        let top = self.text_origin_y();
        let usable = rect.height as i32 - top - self.status_bar_height();
        (usable.max(1) as usize).clamp(1, MAX_MINIMAP_ROWS)
    }

    /// Returns how many lines have cached token spans.
    ///
    /// Exposed so tests can assert the cache stays proportional to the viewport
    /// rather than to the document; there is no production caller.
    #[cfg(test)]
    pub(crate) fn cached_token_lines(&self) -> usize {
        self.line_tokens.iter().filter(|slot| slot.is_some()).count()
    }

    /// Returns whether `line` has a cached token span list.
    #[cfg(test)]
    pub(crate) fn has_cached_tokens(&self, line: usize) -> bool {
        self.line_tokens.get(line).map(|slot| slot.is_some()).unwrap_or(false)
    }

    /// Fills the entry-state chain up to `upto`, for tests of the increments.
    #[cfg(test)]
    pub(crate) fn prime_line_states(&mut self, upto: usize) {
        self.ensure_line_states(upto);
    }

    /// Returns how many bytes the top undo entry retains.
    ///
    /// Exposed so a test can assert a keystroke's history cost follows the edit
    /// and not the document; there is no production caller. The count is taken
    /// when the entry is pushed, because a `dyn UndoCommand` cannot be asked for
    /// its footprint through the trait.
    #[cfg(test)]
    pub(crate) fn debug_last_checkpoint_bytes(&self) -> usize {
        self.last_checkpoint_bytes
    }

    /// Whether the row-prefix table is currently absent.
    ///
    /// Exposed so a test can assert that the fold-free, wrap-free fast path really
    /// does skip the O(document) build instead of silently paying for it.
    #[cfg(test)]
    pub(crate) fn visual_row_prefix_is_empty(&self) -> bool {
        self.visual_row_prefix.is_empty()
    }

    /// Length of the row-prefix table (`line_count + 1` when built).
    #[cfg(test)]
    pub(crate) fn visual_row_prefix_len(&self) -> usize {
        self.visual_row_prefix.len()
    }

    /// Rebuilds every minimap bucket from scratch.
    ///
    /// Called when the bucket count or the document size changed — opening a
    /// file, switching tabs, or an edit that added or removed lines. The cost is
    /// one pass over the document, which is the same order as the load itself
    /// and therefore not something a keystroke ever pays.
    pub(crate) fn rebuild_minimap(&mut self, rows: usize) {
        let rows = rows.max(1);
        let count = self.line_count();
        self.minimap_buckets.clear();
        self.minimap_buckets.resize(rows, MinimapBucket::default());
        if count == 0 {
            return;
        }
        let per_bucket = count.div_ceil(rows).max(1);
        for line in 0..count {
            let bucket = line / per_bucket;
            let Some(text) = self.line_text(line) else { break };
            MinimapBucket::accumulate(&mut self.minimap_buckets[bucket], &text);
        }
    }

    /// Re-folds `line` into its bucket after an edit, without a full pass.
    ///
    /// The bucket keeps a running best (`indent`, `length`) rather than a sum,
    /// so a changed line may only ever *raise* the recorded maximum. A line that
    /// shrank leaves the bucket showing a bar one row too long until the next
    /// rebuild, which is the correct trade: a stale-but-close minimap is visible
    /// only at a glance, while a per-keystroke document pass is not affordable.
    pub(crate) fn patch_minimap_line(&mut self, line: usize) {
        let rows = self.minimap_buckets.len();
        let count = self.line_count();
        if rows == 0 || count == 0 {
            return;
        }
        let per_bucket = count.div_ceil(rows).max(1);
        let bucket = line / per_bucket;
        // Read the text before taking the mutable bucket borrow: `line_text`
        // borrows the model, and holding both at once is a conflict.
        let Some(text) = self.line_text(line) else { return };
        let Some(slot) = self.minimap_buckets.get_mut(bucket) else { return };
        MinimapBucket::accumulate(slot, &text);
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
        // When the row-prefix table exists it already holds the exact total as its
        // trailing entry, so the count comes off the same table the lookups use
        // instead of a second traversal that could disagree with it.
        let rows: usize = if self.visual_row_prefix.len() == self.line_count() + 1 {
            self.visual_row_prefix.last().copied().unwrap_or(self.line_count())
        } else if self.folded_lines.is_empty() {
            // The common case: no folds, so every line below the last fold is
            // visible and the total is a single multiply. Walking the document
            // here is what made scrolling a million-line file cost milliseconds
            // per frame.
            self.line_count()
        } else {
            self.visible_document_lines().iter().map(|line| self.wrap_segments(*line)).sum()
        };
        let rows = if self.config.word_wrap && self.visual_row_prefix.len() != self.line_count() + 1
        {
            // Wrapping makes rows depend on each line's length, so the count
            // cannot be derived without measuring — but only the visible span is
            // needed for layout, and the exact total only feeds the scrollbar.
            self.wrapped_row_estimate()
        } else {
            rows
        };
        rows.saturating_add(self.inline_diagnostics().len())
    }

    /// Estimates the wrapped row count without walking every line.
    ///
    /// A wrapped document's row count is only used to size the scrollbar thumb,
    /// so an estimate is enough: it is exact whenever no line wraps, and never
    /// zero, so the thumb cannot divide by zero or vanish.
    fn wrapped_row_estimate(&self) -> usize {
        if self.folded_lines.is_empty() {
            let columns = self.text_columns().max(1);
            let budget = self.total_line_chars();
            budget.div_ceil(columns).max(self.line_count())
        } else {
            self.visible_document_lines().iter().map(|line| self.wrap_segments(*line)).sum()
        }
    }

    /// Returns the visual row at which a document line starts.
    ///
    /// `B4`: the two cases that need a walk — wrapping and folding — now read a
    /// prefix table instead of walking to `line` on every call, so the cost is
    /// paid once per document-shape change rather than once per lookup. The
    /// fold-free, wrap-free case keeps its O(1) closed form and never builds the
    /// table, which is what keeps a million-line scroll free of work.
    pub(crate) fn line_to_visual_row(&self, line: usize) -> usize {
        if self.folded_lines.is_empty() && !self.config.word_wrap {
            return line;
        }
        self.visual_row_prefix
            .get(line)
            .copied()
            .unwrap_or_else(|| self.visual_row_count_fallback(line))
    }

    /// Returns the document line occupying a visual row.
    ///
    /// The reverse of [`Self::line_to_visual_row`]. With the prefix table present
    /// this is a binary search; the fold-free, wrap-free case is the identity.
    pub(crate) fn visual_row_to_line(&self, row: usize) -> usize {
        let last = self.line_count().saturating_sub(1);
        if self.folded_lines.is_empty() && !self.config.word_wrap {
            return row.min(last);
        }
        if self.visual_row_prefix.len() == self.line_count() + 1 {
            // `partition_point` finds the first line whose start row is strictly
            // past `row`; the line before it owns the row. The table is monotone
            // because every visible line contributes at least one row.
            let index = self.visual_row_prefix.partition_point(|start| *start <= row);
            return index.saturating_sub(1).min(last);
        }
        self.visual_row_to_line_fallback(row)
    }

    /// The uncached forward lookup, used only when the prefix table is absent.
    ///
    /// This is the pre-`B4` implementation kept as a correctness backstop: a caller
    /// that reads a coordinate before any refresh must still get a right answer,
    /// even at the old cost.
    fn visual_row_count_fallback(&self, line: usize) -> usize {
        let mut row = 0usize;
        for document_line in self.visible_document_lines() {
            if document_line >= line {
                break;
            }
            row += self.wrap_segments(document_line);
        }
        row
    }

    /// The uncached reverse lookup, used only when the prefix table is absent.
    fn visual_row_to_line_fallback(&self, row: usize) -> usize {
        let mut cursor = 0usize;
        let mut last = 0usize;
        for document_line in self.visible_document_lines() {
            last = document_line;
            let segments = self.wrap_segments(document_line);
            if row < cursor + segments {
                return document_line;
            }
            cursor += segments;
        }
        last
    }

    /// Rebuilds [`Self::visual_row_prefix`] for the current document shape.
    ///
    /// Called from the same places that invalidate the other derived caches —
    /// edits, tab switches, fold changes and the first cell measurement — so the
    /// table cannot describe a document that no longer exists. The fold-free,
    /// wrap-free document deliberately skips the build: its row and line numbers
    /// are equal, and paying O(document) for a table that would only restate that
    /// would put the cost straight back on the hot path.
    pub(crate) fn rebuild_visual_row_prefix(&mut self) {
        self.visual_row_prefix.clear();
        if self.folded_lines.is_empty() && !self.config.word_wrap {
            return;
        }
        let count = self.line_count();
        let columns = self.text_columns().max(1);
        let folded = !self.folded_lines.is_empty();
        let mut prefix = Vec::with_capacity(count + 1);
        let mut row = 0usize;
        {
            let model = self.model.borrow();
            for line in 0..count {
                prefix.push(row);
                if folded && model.is_hidden(line) {
                    continue;
                }
                row += if self.config.word_wrap {
                    self.line_len(line).max(1).div_ceil(columns).max(1)
                } else {
                    1
                };
            }
        }
        // The trailing entry is the total row count, so a binary search that runs
        // off the end still has a value to stop on.
        prefix.push(row);
        self.visual_row_prefix = prefix;
    }

    /// Returns the summed character count of every line.
    ///
    /// Used only to size the wrapped-scrollbar estimate; it walks the line index
    /// but not the text, and no line's contents are copied.
    fn total_line_chars(&self) -> usize {
        let model = self.model.borrow();
        model.lines.iter().map(|line| line.chars().count()).sum()
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
        let mut result: Vec<InlineDiagnostic> = if self.folded_lines.is_empty() {
            // Nothing is hidden, so a marker's visual row is its line number when
            // wrapping is off. Building a line-to-row table here would walk the
            // whole document on every frame for no reason.
            if !self.config.word_wrap {
                self.markers.iter().map(|marker| (marker.line, marker.clone())).collect()
            } else {
                let columns = self.text_columns().max(1);
                self.markers
                    .iter()
                    .map(|marker| {
                        let row: usize = (0..marker.line)
                            .map(|line| self.line_len(line).max(1).div_ceil(columns))
                            .sum();
                        (row, marker.clone())
                    })
                    .collect()
            }
        } else {
            let mut rows_by_line: Vec<(usize, usize)> = Vec::new();
            let mut row = 0usize;
            for line in self.visible_document_lines() {
                rows_by_line.push((line, row));
                row += self.wrap_segments(line);
            }
            self.markers
                .iter()
                .filter_map(|marker| {
                    rows_by_line
                        .iter()
                        .find(|(line, _)| *line == marker.line)
                        .map(|(_, row)| (*row, marker.clone()))
                })
                .collect()
        };
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
        // An untracked switch records no `TabSwitchCommand`, so any range or
        // snapshot commands still on the stack belong to the previous document.
        // Replaying them against this one would splice by the wrong offsets, so
        // the history is invalidated on the cross-document boundary.
        self.undo_stack.clear();
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
        // A different buffer is a different document: every cached state and every
        // minimap bucket belongs to the buffer being left.
        self.invalidate_line_states();
        self.minimap_line_count = 0;
        // Locked ranges are document coordinates too, so they are dropped rather
        // than inherited by the buffer being entered. A host that re-locks the
        // new buffer re-adds them after the switch.
        self.read_only_spans.clear();
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
        // Closing a tab mutates slot indices and removes a document, so any
        // pending history — which may reference a removed slot or the removed
        // document's text mirror — is invalidated rather than left to mis-target
        // a surviving buffer.
        self.undo_stack.clear();

        // Persist the active buffer's text to its own slot *before* the index
        // shifts, unless the active buffer itself is the one being closed — its
        // text must not be written into a surviving slot.
        let closed_is_active = self.model.borrow().active_tab == index;
        if !closed_is_active {
            self.model.borrow_mut().persist_active();
        }

        let next_active = {
            let mut model = self.model.borrow_mut();
            model.all_buffers.remove(index);
            if model.active_tab > index {
                model.active_tab -= 1;
            } else if model.active_tab >= model.all_buffers.len() {
                model.active_tab = model.all_buffers.len().saturating_sub(1);
            }
            model.active_tab
        };

        self.model.borrow_mut().load_buffer(next_active);
        self.after_buffer_switch(next_active);
        true
    }

    /// Returns `true` when the active buffer has unsaved edits.
    pub fn is_modified(&self) -> bool {
        // A maintained flag, not a comparison: `saved` holds a full copy of the
        // document, so `saved != text` is one pass over the file.
        self.model.borrow().dirty
    }

    /// Marks the active buffer as saved.
    pub fn mark_saved(&mut self) {
        let current = self.model.borrow().text.borrow().clone();
        let mut model = self.model.borrow_mut();
        model.saved = current.clone();
        model.dirty = false;
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

    /// Recomputes the hit list, reusing everything outside `dirty`.
    ///
    /// A keystroke changes one line, so re-scanning the document for hits is
    /// wasted work — and in a large file it is the dominant per-key cost. This
    /// removes the hits of the lines in `dirty`, rescans only those, and splices
    /// the results back in place. The list stays in document order, which the
    /// navigation commands rely on.
    ///
    /// `dirty` is `(first_line, inserted_lines, removed_lines)` describing the
    /// edit in line terms: lines `first_line..first_line + removed_lines` were
    /// replaced by `inserted_lines` new ones. `None` means "the shape of the
    /// document changed and nothing can be reused", which falls back to a full
    /// scan.
    fn recompute_match_cache_incremental(&mut self, dirty: Option<(usize, usize, usize)>) {
        let query = self.find.query.clone();
        if query.is_empty() {
            // No query means no hits, and finding that out costs nothing.
            self.find.matches.clear();
            self.find.current = 0;
            return;
        }
        let Some((first, inserted, removed)) = dirty else {
            self.recompute_match_cache();
            return;
        };
        let options = self.find.options;
        // Drop the hits of the replaced lines from the existing list.
        let last_old = first.saturating_add(removed);
        self.find.matches.retain(|hit| hit.line < first || hit.line >= last_old);
        // Rescan only the lines that now exist at that position.
        let last_new = first.saturating_add(inserted).min(self.line_count());
        let mut fresh: Vec<SearchMatch> = Vec::new();
        for line in first..last_new {
            let Some(text) = self.line_text(line) else { break };
            self.collect_matches_in_line(line, &text, &query, options, &mut fresh);
        }
        // Hits after the edited region keep their relative order; shift them by
        // the line-count delta so they point at the right lines.
        if inserted != removed {
            let delta = inserted as isize - removed as isize;
            for hit in self.find.matches.iter_mut().skip_while(|hit| hit.line < last_old) {
                hit.line = (hit.line as isize + delta).max(0) as usize;
            }
        }
        // Insert the fresh hits where they belong: after the retained hits that
        // precede them, before the ones that follow.
        let insert_at = self.find.matches.partition_point(|hit| hit.line < first);
        self.find.matches.splice(insert_at..insert_at, fresh);
        self.find.current = if self.find.matches.is_empty() {
            0
        } else {
            self.find.current.min(self.find.matches.len() - 1)
        };
    }

    /// Appends every match of `query` within one line to `out`.
    fn collect_matches_in_line(
        &self,
        line_index: usize,
        text: &str,
        query: &str,
        options: SearchOptions,
        out: &mut Vec<SearchMatch>,
    ) {
        let needle: Vec<char> = query.chars().collect();
        if needle.is_empty() {
            return;
        }
        let haystack: Vec<char> = text.chars().collect();
        if haystack.len() < needle.len() {
            return;
        }
        let language = self.config.language;
        let mut start = 0usize;
        while start + needle.len() <= haystack.len() {
            let window = &haystack[start..start + needle.len()];
            let equal = if options.case_sensitive {
                window == needle.as_slice()
            } else {
                window
                    .iter()
                    .flat_map(|ch| ch.to_lowercase())
                    .eq(needle.iter().flat_map(|ch| ch.to_lowercase()))
            };
            if equal && whole_word_ok(&haystack, start, needle.len(), options, language) {
                out.push(SearchMatch {
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
        if !self.is_editable() {
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
        if !self.is_editable() || self.find.matches.is_empty() {
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

    /// Tells the editor whether the clipboard currently holds text.
    ///
    /// A host that subscribes to the platform's clipboard-change notification calls
    /// this, after which opening the context menu never touches the OS clipboard.
    /// Without the call the editor probes once and caches the answer, so the menu
    /// is still correct on the first open — it just pays one platform read.
    pub fn set_clipboard_has_text(&mut self, has_text: bool) {
        self.clipboard_has_text = Some(has_text);
    }

    /// Forgets the cached clipboard state, so the next menu re-probes it.
    ///
    /// For a host that cannot be notified of clipboard changes: call this when the
    /// application loses and regains focus, which is the point at which another
    /// program may have replaced the clipboard.
    pub fn invalidate_clipboard_has_text(&mut self) {
        self.clipboard_has_text = None;
    }

    /// Returns whether the clipboard holds text, probing the platform once.
    ///
    /// A host-injected answer (or one this widget recorded itself) wins; only an
    /// unknown state reads the platform. The read is cached, because the answer
    /// cannot change without either this widget or the host observing it — the menu
    /// is the wrong place to discover it and the wrong place to pay for it.
    pub fn clipboard_has_text(&mut self) -> bool {
        if let Some(known) = self.clipboard_has_text {
            return known;
        }
        let probe = !crate::clipboard::ClipboardManager::text().is_empty();
        self.clipboard_has_text = Some(probe);
        probe
    }

    /// Copies the selection to the system clipboard.
    pub fn copy(&mut self) -> bool {
        let Some(selected) = self.selected_text() else { return false };
        if selected.is_empty() {
            return false;
        }
        crate::clipboard::ClipboardManager::set_text(selected);
        // We just wrote text, so the cached answer is known without reading it back.
        self.clipboard_has_text = Some(true);
        true
    }

    /// Cuts the selection to the system clipboard.
    pub fn cut(&mut self) -> bool {
        if !self.is_editable() || !self.copy() {
            return false;
        }
        self.delete_selection();
        true
    }

    /// Pastes the system clipboard at the caret.
    pub fn paste(&mut self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let text = crate::clipboard::ClipboardManager::text();
        // Record what the read saw, so the menu does not have to read it again.
        self.clipboard_has_text = Some(!text.is_empty());
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
    ///
    /// The `paste` row's state comes from [`Self::clipboard_has_text`], which is
    /// cached rather than read here. Opening a menu is a pure UI action and must not
    /// touch the machine-global OS clipboard, whose handle is exclusive (it can
    /// block, and it reports empty while another process holds it — which would
    /// disable `paste` for a reason the user cannot see).
    pub fn open_context_menu(&mut self, position: Point) {
        let has_selection = self.has_selection();
        let clipboard_has_text = self.clipboard_has_text();
        let read_only = !self.is_editable();
        // A selection that overlaps a locked range cannot be mutated, so the
        // menu reports the refusal before the command runs. Without this every
        // actionable row would look enabled and then silently do nothing.
        let (selection_start, selection_end) = self.cursor.bounds();
        let selection_locked =
            has_selection && !self.is_range_editable(selection_start, selection_end);
        let caret_locked = !self.is_position_editable(self.cursor.head);
        let mutation_blocked = read_only || selection_locked || caret_locked;
        let selection_mutation_blocked = read_only || selection_locked;
        self.context_menu.items = vec![
            MenuItem::new("cut", "Cut")
                .with_shortcut("Cmd+X")
                .disabled(!has_selection || selection_mutation_blocked),
            MenuItem::new("copy", "Copy").with_shortcut("Cmd+C").disabled(!has_selection),
            MenuItem::new("paste", "Paste")
                .with_shortcut("Cmd+V")
                .disabled(mutation_blocked || !clipboard_has_text),
            MenuItem::new("select_all", "Select All").with_shortcut("Cmd+A"),
            MenuItem::new("find", "Find").with_shortcut("Cmd+F"),
            MenuItem::new("replace", "Replace").with_shortcut("Cmd+H"),
            MenuItem::new("toggle_comment", "Toggle Comment")
                .with_shortcut("Cmd+/")
                .disabled(mutation_blocked),
            MenuItem::new("fold", "Fold Block").with_shortcut("Cmd+["),
            MenuItem::new("unfold_all", "Unfold All"),
            MenuItem::new("indent", "Indent").with_shortcut("Cmd+I").disabled(mutation_blocked),
            MenuItem::new("outdent", "Outdent").with_shortcut("Cmd+[").disabled(mutation_blocked),
            MenuItem::new("duplicate", "Duplicate Line")
                .with_shortcut("Cmd+Shift+D")
                .disabled(mutation_blocked),
            MenuItem::new("delete_line", "Delete Line")
                .with_shortcut("Cmd+Shift+K")
                .disabled(mutation_blocked),
            MenuItem::new("sort_lines", "Sort Lines").disabled(selection_mutation_blocked),
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

    /// Runs one line through the active highlighter, returning spans and exit state.
    ///
    /// Resolution order is the whole point of the seam: a highlighter registered
    /// for the buffer's language wins, then the editor-wide override, then the
    /// built-in lexer for that language. The language is resolved per call rather
    /// than cached, because a buffer switch changes it and the cache invalidation
    /// that follows (`invalidate_line_states`) is what makes the switch visible.
    fn highlight_with_state(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
        let language = self.active_language();
        if let Some((_, highlighter)) =
            self.language_highlighters.iter().find(|(registered, _)| *registered == language)
        {
            return highlighter.highlight_line(line, state);
        }
        match &self.highlighter {
            Some(highlighter) => highlighter.highlight_line(line, state),
            None => BuiltinHighlighter::new(language).highlight_line(line, state),
        }
    }

    /// Returns the lexer state line `index` starts in.
    ///
    /// The cache is rebuilt lazily: `line_states` is grown to the current line
    /// count, and entries beyond the last one that is known to be accurate are
    /// re-derived by walking forward from that point. A line that both starts
    /// and ends [`LineState::Clean`] cannot influence the next line, so a clean
    /// run is filled in without running the lexer at all — which is what keeps
    /// this proportional to the edited region rather than to the document.
    ///
    /// The returned state for a line is only meaningful once the chain has
    /// reached it; [`CodeEditor::line_state`] reports the cached value, which is
    /// `Clean` for any line the walk has not needed yet.
    fn ensure_line_states(&mut self, upto: usize) {
        let count = self.line_count();
        // The vector holds one entry per line plus a trailing sentinel carrying
        // the document's final state, so `line_states[line]` is always valid and
        // `line_states[line + 1]` is the state that line ends in.
        if self.line_states.len() != count + 1 {
            self.line_states.resize(count + 1, LineState::Clean);
            self.line_tokens.resize(count, None);
            self.walked_lines = 0;
        }
        let target = upto.min(count);
        // `walked_lines` is how far the entry-state chain has been *proven*.
        // Every entry below it came out of the highlighter, so it is trustworthy
        // even when non-clean; above it, entries only hold the `Clean` default
        // and prove nothing.
        let mut line = self.walked_lines.min(target);
        if line >= target {
            return;
        }
        let mut state = self.line_states[line];
        while line < target {
            let Some(text) = self.line_text(line) else { return };
            let (_, next) = self.highlight_with_state(&text, state);
            self.line_states[line + 1] = next;
            // This line's entry state is now proven and its exit state recorded.
            self.walked_lines = line + 1;
            if next == state && state.is_clean() {
                // Both ends are `Clean`, so this line cannot influence the next
                // one. Every remaining line in a pure-code region is therefore
                // also `Clean`, and the walk can stop: that is what keeps a
                // 10 000-line block comment from being re-lexed on every
                // keystroke while still walking cheaply through plain code.
                break;
            }
            state = next;
            line += 1;
        }
    }

    /// Drops every cached lexer state, forcing a full re-derivation.
    ///
    /// Used when the highlighter or the language changes: the entry-state chain
    /// is produced *by* the highlighter, so it cannot survive a swap.
    pub(crate) fn invalidate_line_states(&mut self) {
        for slot in self.line_tokens.iter_mut() {
            *slot = None;
        }
        for state in self.line_states.iter_mut() {
            *state = LineState::Clean;
        }
        self.walked_lines = 0;
    }

    /// Returns the token spans of a single line.
    ///
    /// Uses the installed highlighter when present, otherwise the built-in
    /// lexer. Spans from a third-party highlighter are clamped to `char`
    /// boundaries and to the line length, so drawing can never panic.
    ///
    /// Results are cached per line and keyed by the lexer state the line starts
    /// in, so a repaint of an unchanged document does no lexing at all.
    pub fn tokens_on_line(&self, line: usize) -> Vec<TokenSpan> {
        if let Some(cached) = self.line_tokens.get(line).and_then(|slot| slot.as_ref()) {
            return cached.clone();
        }
        let Some(text) = self.line_text(line) else { return Vec::new() };
        let state = self.line_states.get(line).copied().unwrap_or_default();
        let (raw, _) = self.highlight_with_state(&text, state);
        clamp_and_merge(&text, raw)
    }

    /// Returns the token spans of `line`, computing and caching them.
    ///
    /// The paint path holds `&self` and therefore cannot fill the cache; this
    /// `&mut` variant is what `draw` calls once per visible row so the cache is
    /// populated before the immutable readers run.
    pub(crate) fn fill_tokens_on_line(&mut self, line: usize) {
        if self.line_tokens.get(line).map(|slot| slot.is_some()).unwrap_or(false) {
            return;
        }
        let Some(text) = self.line_text(line) else { return };
        let state = self.line_states.get(line).copied().unwrap_or_default();
        let (raw, _) = self.highlight_with_state(&text, state);
        let merged = clamp_and_merge(&text, raw);
        if let Some(slot) = self.line_tokens.get_mut(line) {
            *slot = Some(merged);
        }
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
            Event::MousePress { pos, button, .. } if *button == 1 => self.pointer_press(*pos),
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
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

    /// The editor announces its selection, or the line the caret is on.
    ///
    /// A generic value lookup would find nothing useful here: the editor publishes no `value`
    /// property (its buffer is far too large to read on every announcement), so the default would
    /// announce empty and a screen-reader user would hear the editor's name with no sense of where
    /// they are. The line under the caret is the fact a user is actually navigating by, and a
    /// selection supersedes it because that is what the next command will act on.
    fn accessible_value(&self) -> String {
        let (start, end) = self.cursor.bounds();
        if start != end {
            return self.selected_text().unwrap_or_default();
        }
        self.line_text(self.cursor.head.line).unwrap_or_default()
    }

    impl_widget_property_hooks!();

    // The caret blink is driven through the trait so the animation bus reaches it -- an
    // editor whose caret never blinks reads as frozen.
    fn tick(&mut self, delta_ms: u32) -> bool {
        CodeEditor::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.is_editable()
    }
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "cursor_moved" => {
                Some(EventSignalRef::mapped("cursor_moved", &self.cursor_moved, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ])
                }))
            }
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    match v {
                        Some(value) => CapabilityValue::String(format!("{value:?}")),
                        None => CapabilityValue::Null,
                    }
                }))
            }
            "tab_changed" => Some(EventSignalRef::mapped("tab_changed", &self.tab_changed, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "fold_changed" => {
                Some(EventSignalRef::mapped("fold_changed", &self.fold_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "search_changed" => {
                Some(EventSignalRef::mapped("search_changed", &self.search_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "completion_changed" => {
                Some(EventSignalRef::mapped("completion_changed", &self.completion_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `CodeEditor`'s property contract, published under the `RichEdit` kind.
///
/// The capability layer makes `WidgetKind::RichEdit` the kind of this control
/// (`code_editor_capability`), and this impl reproduces exactly what the old
/// `read_input_props` / `write_input_props` arms answered for that kind. The
/// `RichEdit` widget is a different control that shares the kind; it keeps its
/// own file-local contract, and the downcast decides which one answers.
impl WidgetProperties for CodeEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "line_count" => Ok(CapabilityValue::UInt(self.line_count() as u64)),
            "cursor_line" => Ok(CapabilityValue::UInt(self.cursor().0 as u64)),
            "cursor_column" => Ok(CapabilityValue::UInt(self.cursor().1 as u64)),
            "marker_count" => Ok(CapabilityValue::UInt(self.markers().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            // Position, size and diagnostic count are derived from the buffer and
            // the caret.
            "line_count" | "cursor_line" | "cursor_column" | "marker_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // `CODE_EDITOR_PROPERTIES` declares `marker_count` readable, so the
        // contract must answer it: a name in the schema that no contract serves is
        // a promise the caller cannot keep, and the bidirectional schema test
        // rejects exactly that.
        property_names_of![
            "text",
            "line_count",
            "cursor_line",
            "cursor_column",
            "marker_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `code_editor` publishes.
    ///
    /// `undo` is the genuine zero-argument action here. It reports `false` when the
    /// undo stack is empty or the editor is read-only, which is the "could not
    /// handle it" case, so it is answered with
    /// [`CapabilityAccessError::OutOfRange`] rather than a success that did
    /// nothing. `append_line` takes the line to append and `set_text` / `set_cursor`
    /// assign state through the property route, so those are refused the same way
    /// — the names are right and the payload is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "undo" => {
                if self.undo() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "append_line" | "set_text" | "set_markers" | "set_cursor" => {
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

/// Shifts a line index by a signed delta, saturating at zero.
///
/// Used when a locked range has to follow an edit that inserted or removed lines
/// above it. Saturating rather than wrapping matters: a removal larger than the
/// index must land the span at line 0, not at `usize::MAX`.
fn shift_index(index: usize, delta: isize) -> usize {
    if delta >= 0 {
        index.saturating_add(delta as usize)
    } else {
        index.saturating_sub(delta.unsigned_abs())
    }
}

/// Converts a byte offset in `text` into a [`TextPosition`].
///
/// The offset is clamped to a character boundary and to the text length, so a
/// caller may pass the end of an insertion (`start + removed + inserted`) without
/// checking whether it still lands inside the pre-edit text. Columns are counted
/// in characters, matching every other position in the editor.
fn position_in_text(text: &str, offset: usize) -> TextPosition {
    let offset = floor_char_boundary(text, offset.min(text.len()));
    let mut line = 0usize;
    let mut column = 0usize;
    for ch in text[..offset].chars() {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    TextPosition::new(line, column)
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

/// Upper bound on minimap strip rows.
///
/// A strip taller than this gains no legibility — a bar per pixel row is already
/// finer than the eye resolves at that width — while making the summary and the
/// per-frame loop proportionally longer. Clamping also keeps the summary a fixed
/// size across window resizes in the common case.
pub(crate) const MAX_MINIMAP_ROWS: usize = 256;

/// Everything an edit path can tell the history about what it changed.
///
/// The three fields are independently optional because the paths know different
/// amounts: typing knows the byte range it replaced and the text that was there;
/// a line command that rebuilt the document only has the before/after pair. The
/// most specific description wins, and anything missing is reconstructed from the
/// buffer before the edit overwrites it.
struct Checkpoint {
    /// `(start_offset, removed_len, inserted)` in pre-edit coordinates.
    range: Option<(usize, usize, String)>,
    /// Text the edit replaced, read before the write.
    removed: Option<String>,
    /// The document as it was before the edit, for the snapshot fallback.
    before: Option<String>,
}

/// Returns the minimal `(start, removed_len, inserted)` change between two texts.
///
/// Used only by the edit paths that rebuild the document by hand and therefore
/// have no range to report (line commands, sort, trim). The common prefix and
/// suffix are trimmed, so the recorded edit is the smallest one that reproduces
/// `after` — which keeps a checkpoint's memory proportional to what changed even
/// when the caller could only describe the result.
///
/// Returns `None` when the texts are equal.
fn diff_range(before: &str, after: &str) -> Option<(usize, usize, String)> {
    if before == after {
        return None;
    }
    let before_bytes = before.as_bytes();
    let after_bytes = after.as_bytes();
    let max_prefix = before_bytes.len().min(after_bytes.len());
    let mut prefix = 0usize;
    while prefix < max_prefix && before_bytes[prefix] == after_bytes[prefix] {
        prefix += 1;
    }
    // Do not cut a multi-byte character in half: back up to the nearest boundary.
    while prefix > 0 && !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = (before_bytes.len() - prefix).min(after_bytes.len() - prefix);
    let mut suffix = 0usize;
    while suffix < max_suffix
        && before_bytes[before_bytes.len() - 1 - suffix]
            == after_bytes[after_bytes.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0 && !after.is_char_boundary(after.len() - suffix) {
        suffix -= 1;
    }
    while suffix > 0 && !before.is_char_boundary(before.len() - suffix) {
        suffix -= 1;
    }
    let removed_len = before_bytes.len().saturating_sub(prefix + suffix);
    let inserted_end = after_bytes.len().saturating_sub(suffix);
    Some((prefix, removed_len, after[prefix..inserted_end].to_string()))
}

/// Clamps highlighter spans to `text` and merges the adjacent ones.
///
/// A third-party highlighter can return offsets past the line end or inside a
/// multi-byte character; both are repaired here rather than at every call site,
/// because a malformed span reaching the renderer panics the whole frame.
fn clamp_and_merge(text: &str, raw: Vec<TokenSpan>) -> Vec<TokenSpan> {
    let length = text.len();
    let mut spans: Vec<TokenSpan> = Vec::with_capacity(raw.len());
    for mut span in raw {
        span.start = floor_char_boundary(text, span.start.min(length));
        span.end = floor_char_boundary(text, span.end.min(length));
        if span.end > span.start {
            spans.push(span);
        }
    }
    spans.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));
    super::syntax::merge_adjacent(spans)
}

/// One row of the minimap: the longest non-blank line folded into it.
///
/// A bucket holds a *summary*, not a sum, so painting reads one value per strip
/// row instead of walking the document. `blank` is tracked separately from
/// `length == 0` because a bucket containing only whitespace lines is blank,
/// while one containing a line of spaces is not.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MinimapBucket {
    /// Indent column of the longest line seen in this bucket.
    pub(crate) indent: usize,
    /// Text length of that line, indentation excluded.
    pub(crate) length: usize,
    /// `true` when no non-blank line has landed in this bucket.
    pub(crate) blank: bool,
}

impl MinimapBucket {
    /// Folds one line into the bucket, keeping the longest.
    fn accumulate(slot: &mut Self, text: &str) {
        let indent = text.chars().take_while(|ch| ch.is_whitespace()).count();
        let length = text.trim_end().chars().count().saturating_sub(indent);
        if length == 0 {
            return;
        }
        if slot.blank || length > slot.length {
            slot.indent = indent;
            slot.length = length;
        }
        slot.blank = false;
    }
}

#[cfg(test)]
mod diff_range_tests {
    use super::diff_range;

    #[test]
    fn a_single_substitution_is_one_range() {
        assert_eq!(diff_range("a-b-c", "a+b+c"), Some((1, 3, "+b+".to_string())));
    }

    #[test]
    fn pure_insertion_and_deletion() {
        assert_eq!(diff_range("ac", "abc"), Some((1, 0, "b".to_string())));
        assert_eq!(diff_range("abc", "ac"), Some((1, 1, String::new())));
    }

    #[test]
    fn identical_texts_produce_no_edit() {
        assert_eq!(diff_range("same", "same"), None);
        assert_eq!(diff_range("", ""), None);
    }

    #[test]
    fn multibyte_boundaries_are_respected() {
        let (start, removed, inserted) = diff_range("héllo", "hello").unwrap();
        assert!("héllo".is_char_boundary(start));
        assert!("héllo".is_char_boundary(start + removed));
        assert_eq!(inserted, "e");
    }

    #[test]
    fn replacing_the_whole_text() {
        assert_eq!(diff_range("abc", "xyz"), Some((0, 3, "xyz".to_string())));
    }
}
