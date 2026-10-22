// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The [`CodeEditor`] widget: state, construction and the shared helpers.
//!
//! # Module layout
//!
//! The editing surface is large enough that one file hid its structure (5512
//! lines before this split, against principle #9's 2000-line ceiling). Each
//! concern now lives in a child module — see the `mod` declarations below —
//! and this file owns the type, its fields, construction, the language and
//! highlighter configuration, and the free helpers the modules share.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | `document` | Text access and the edit-application machinery |
//! | `editing` | Multi-caret edits, line commands, typing, history |
//! | `caret` | Cursor, selection and caret movement |
//! | `viewport` | Scrolling, viewport linkage, caret visibility |
//! | `folding` | Fold regions and the visible-row projection |
//! | `rows` | Visual rows, wrap segments and the minimap summary |
//! | `diagnostics` | Diagnostic markers and inline diagnostics |
//! | `tabs` | Open buffers, activation, modified flag |
//! | `find` | Find/replace, bracket matching, occurrences |
//! | `clipboard` | Cut, copy and paste |
//! | `completion` | Completion popup and context menu |
//! | `tokens` | Lexer state chaining and token queries |
//! | `events` | Event routing: keys, IME, pointer, wheel |
//! | `widget_impl` | The `Widget` and property contracts |

use super::buffer::{join_lines, split_lines, EditorModel};
use super::grid::{LineGrid, RowLayout};
use super::multicursor::{self, MultiCursor, OffsetEdit};
use super::pairs::{self, PairAction};
use super::range_command::TextRangeCommand;
use super::syntax::{BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter};
use super::types::{
    CodeEditorConfig, CompletionSource, CompletionState, ContextMenuState, Cursor, CursorGoal,
    DiagnosticMarker, DocumentCompletions, DocumentScale, EditRejection, EditorBuffer, FindState,
    FoldRegion, InlineDiagnostic, MarkerSeverity, MenuItem, ReadOnlySpan, SearchMatch,
    SearchOptions, SyntaxPalette, TextPosition, TokenKind, TokenSpan, ViewportSnapshot, VisualLine,
    MAX_COMPLETIONS,
};
use crate::compat::MiniVec;
use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack, ValueCommand};
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

// The widget's code is split into focused submodules. They are private and each
// carries a complete `impl CodeEditor` block (or the whole trait impl it owns), so
// a move between these files cannot change the public surface.
mod caret;
mod clipboard;
mod completion;
mod diagnostics;
mod document;
mod editing;
mod events;
mod find;
mod folding;
mod rows;
mod tabs;
mod tokens;
mod viewport;
mod widget_impl;

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

/// What [`CodeEditor::occurrence_cache`] was computed from.
///
/// The two fields are the complete input of "which words recur": the selection
/// being looked for and the document it was looked for in.
#[derive(Debug, Clone)]
pub(crate) struct OccurrenceCache {
    /// Selection the search used.
    pub(crate) bounds: (TextPosition, TextPosition),
    /// Document revision the search ran against.
    pub(crate) revision: u64,
    /// Matching ranges, sorted by (line, column) like every other match list.
    pub(crate) matches: Vec<SearchMatch>,
}

/// What [`CodeEditor::bracket_cache`] was computed from.
///
/// Resolving a bracket pair is a scan from the caret to its partner, which for a top-level
/// brace is the rest of the document (or, backwards, its start). The paint path asks for the
/// pair once per frame, so caching the answer for an unchanged (revision, caret) is what keeps
/// a still frame from re-walking the file — the same per-frame document scan the minimap had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BracketCache {
    /// Document revision the pair was resolved against.
    pub(crate) revision: u64,
    /// Caret position the pair was resolved for.
    pub(crate) caret: TextPosition,
    /// The resolved pair, or `None` when the caret was not on a bracket.
    pub(crate) pair: Option<(TextPosition, TextPosition)>,
}

/// An IME composition that has not been committed yet.
///
/// `cursor` is the byte offset of the insertion point inside `text`, which is what a
/// host needs to place its candidate window. The editor only paints it; the document
/// is untouched until the commit event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Preedit {
    /// The composition string.
    pub(crate) text: String,
    /// Byte offset of the insertion point inside `text`.
    pub(crate) cursor: usize,
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
    /// Occurrences of the selected word, with the state they were computed from.
    ///
    /// The paint path used to call `occurrences_of_selection()` every frame, which
    /// runs a whole-document search whenever anything is selected. The cache is
    /// keyed by the selection bounds and the document revision, so it is recomputed
    /// exactly when one of those moves and never otherwise.
    pub(crate) occurrence_cache: Option<OccurrenceCache>,
    /// The last resolved bracket pair, keyed by the revision and the caret.
    ///
    /// `RefCell` because [`CodeEditor::matching_brackets`] is `&self`: both the painter and a
    /// host's query share it, and the cache is an implementation detail of that query rather
    /// than observable state.
    pub(crate) bracket_cache: RefCell<Option<BracketCache>>,
    /// Measured horizontal geometry per document line, for the rows last painted.
    ///
    /// The renderer draws with a **proportional** Latin face (see [`RowLayout`]), so
    /// `column * cell_width` is not where a character landed. The paint path measures
    /// each row's runs and stores the boundary positions here; the caret, the
    /// selection and the hit-test read them, which is what makes a click land on the
    /// glyph that was drawn. Entries are dropped when a line leaves the viewport and
    /// cleared when the revision, the font size or the geometry changes.
    ///
    /// `RefCell` because the painters and the public hit-test take `&self` — the same
    /// reason `measured_cell_width` is a `Cell`.
    pub(crate) row_layouts: RefCell<Vec<(usize, RowLayout)>>,
    /// Everything the cached [`Self::row_layouts`] were measured against.
    ///
    /// A layout's `xs` are a statement about one line's text, shaped with one font at one
    /// tab width. If any of those inputs changes, the cached numbers describe text that is no
    /// longer on screen, so the pair is compared before reuse and the cache is dropped when it
    /// differs. It is a `Cell` because the paint path that reads it holds `&self` in the
    /// row helpers.
    ///
    /// The lexer generation is part of the key even though it is not a *metric*: the runs a
    /// row is measured in come from the highlighter, and shaping each run separately can
    /// differ from shaping the line as a whole. A language or highlighter change therefore
    /// re-measures rather than reusing x positions computed from a different partition.
    pub(crate) row_layout_key: core::cell::Cell<(
        u64,
        u64,
        u32,
        usize,
        u32,
        u32,
        crate::widget::special_widgets::code_editor::syntax::LanguageId,
        u64,
    )>,
    /// Bumped whenever the cached lexer state is dropped, which is the event that can change a
    /// line's token runs without changing its text.
    pub(crate) layout_lexer_generation: u64,
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
    /// `true` while a press that began in the minimap is being dragged.
    ///
    /// The minimap is a scrollbar shaped like a document, so dragging inside it must
    /// keep scrolling even when the pointer leaves the strip — the same capture rule
    /// the text selection follows.
    pub(crate) minimap_dragging: bool,
    /// The IME composition under construction, if one is active.
    ///
    /// Zed draws the composing text **in the buffer** (with an underline and the
    /// candidate window anchored to it); this editor ignored `Event::ImePreedit`
    /// entirely, so a Chinese/Japanese input method showed nothing until the commit —
    /// the text appeared all at once, and the candidate window had no visible anchor.
    /// The preedit is drawn as an overlay at the caret (see `draw_preedit`); it is not
    /// inserted into the document until `ImeCommit`.
    pub(crate) preedit: Option<Preedit>,
    /// Screen rectangles of the tab strip, written by the painter.
    ///
    /// The tab layout is measured in the font the titles are drawn in, which only
    /// exists during a paint (`RenderContext` owns the backend). The pointer handler
    /// runs without one, so it reads the rectangles of the **last painted** strip —
    /// the same arrangement `row_layouts` uses for the text. Nothing is painted
    /// before the first frame, so the list starts empty and a click in the strip
    /// before then does nothing rather than guessing.
    pub(crate) tab_hits: RefCell<Vec<super::render::TabSlot>>,
    /// The tab the pointer is currently over, if any.
    ///
    /// Hover is what reveals the close affordance on an inactive tab and washes it
    /// toward the surface it will take when clicked (Zed's tab behaviour). It is
    /// maintained from `MouseEnter`/`MouseMove`, not recomputed per frame: a paint
    /// must not depend on where the pointer happened to be when it ran.
    pub(crate) hovered_tab: Option<usize>,
    pub(crate) mouse_selecting: bool,
    /// The pointer position of the last press or drag.
    ///
    /// A toolkit synthesises a motion event after a click, and the double-click
    /// handler runs *before* the matching release — so that motion arrived with
    /// `mouse_selecting` still set, extended the selection to the same position and
    /// collapsed the word that had just been selected. Ignoring a drag that has not
    /// moved is both correct and the fix (the double-click selection now survives).
    pub(crate) last_pointer_position: Option<Point>,
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
    /// A counter bumped on every committed text change (BLUE26 B26.08).
    ///
    /// # Why a text editor needs a version
    ///
    /// A host that reacts to `text_changed` — a linter, a source-mapper, a language server — keeps a
    /// position into the text it was given, and by the time its work completes the document may have
    /// moved on. Recording the revision with the request and comparing it on return is how the host
    /// detects "the position I computed is stale" instead of applying an answer to text that no
    /// longer exists. It is bumped at [`CodeEditor::record_and_settle_inner`], the single choke
    /// point every edit passes through, so it cannot miss a mutation or count a no-op.
    ///
    /// A monotone counter rather than a hash: a hash collides, and a collision here means a stale
    /// result is accepted as fresh — the exact failure the version exists to prevent.
    revision: u64,

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
            occurrence_cache: None,
            bracket_cache: RefCell::new(None),
            row_layouts: RefCell::new(Vec::new()),
            row_layout_key: core::cell::Cell::new((
                0,
                0,
                0,
                0,
                0,
                0,
                crate::widget::special_widgets::code_editor::syntax::LanguageId::default(),
                0,
            )),
            layout_lexer_generation: 0,
            walked_lines: 0,
            minimap_buckets: Vec::new(),
            minimap_line_count: 0,
            scale: DocumentScale::Full,
            mouse_selecting: false,
            minimap_dragging: false,
            preedit: None,
            tab_hits: RefCell::new(Vec::new()),
            hovered_tab: None,
            last_pointer_position: None,
            caret_blink: crate::style::CursorBlink::new(),
            undo_stack: UndoStack::new(),
            restoring_history: false,
            last_checkpoint_bytes: 0,
            revision: 0,
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

    // ── Input plumbing ──────────────────────────────────────────
    //
    // The keyboard, IME, pointer and wheel layer lives in `input.rs`; it only
    // drives the public editing commands declared above.

    /// CONTROL modifier bit used by this platform layer.
    pub(crate) const MODIFIER_CONTROL: u32 = 2;
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

/// `true` when `byte` falls inside one of the given spans.
///
/// The spans are the string and comment tokens of one line; a character inside one
/// of them is text, not syntax, so bracket matching must step over it.
fn is_skipped(byte: usize, spans: &[(usize, usize)]) -> bool {
    spans.iter().any(|(start, end)| byte >= *start && byte < *end)
}

/// Byte offset and character at char index `column` of `text`.
///
/// The inverse of `LineGrid::char_of_byte`, for the scanning paths that need both
/// values at once and have the text in hand.
fn byte_and_char_at(text: &str, column: usize) -> Option<(usize, char)> {
    text.char_indices().nth(column)
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

/// What a minimap bar is made of, at the granularity a thumbnail can show.
///
/// Three tones, not a token palette: a minimap bar is one to three pixels tall, so
/// anything finer is invisible. Comment/string/code is enough to give a file the
/// texture Zed's minimap has (green comment blocks, warm string runs, plain code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MiniTone {
    /// Ordinary code.
    #[default]
    Plain,
    /// A comment line.
    Comment,
    /// A line carrying a string literal.
    String,
}

/// The longest common **leading whitespace** of a block of text.
///
/// Used by the paste path to rebase a block on the caret line's indentation. Blank
/// lines are ignored: they carry no indentation to compare, and counting them would
/// collapse the common prefix to nothing. The comparison is a byte prefix, which is
/// correct for the only characters involved (` ` and `\t`).
pub(crate) fn common_leading_whitespace(text: &str) -> String {
    let mut common: Option<&str> = None;
    for line in text.split('\n') {
        let line = line.trim_end_matches(['\r']);
        if line.trim().is_empty() {
            continue;
        }
        let indent = &line[..line.len() - line.trim_start_matches([' ', '\t']).len()];
        common = Some(match common {
            None => indent,
            Some(previous) => {
                // Both sides are ASCII whitespace by construction, so a byte prefix is a
                // char prefix and the slice below cannot split a character.
                let shared = previous
                    .bytes()
                    .zip(indent.bytes())
                    .take_while(|(left, right)| left == right)
                    .count();
                &previous[..shared]
            }
        });
    }
    common.unwrap_or("").to_string()
}

/// One row of the minimap: the longest non-blank line folded into it.
///
/// A bucket holds a *summary*, not a sum, so painting reads one value per strip
/// row instead of walking the document. `blank` is tracked separately from
/// `length == 0` because a bucket containing only whitespace lines is blank,
/// while one containing a line of spaces is not.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MinimapBucket {
    /// Indent column of the longest line seen in this bucket.
    pub(crate) indent: usize,
    /// Text length of that line, indentation excluded.
    pub(crate) length: usize,
    /// What that line mostly is, for the bar's colour.
    pub(crate) tone: MiniTone,
    /// `true` when no non-blank line has landed in this bucket.
    pub(crate) blank: bool,
}

/// How many strip rows a document is summarized into, and how many lines each
/// bucket covers.
///
/// The two numbers have to be derived together. `rows` is a pixel budget, while a
/// bucket is a group of lines; resizing the vector to `rows` and dividing the
/// document by `ceil(count / rows)` leaves a blank tail whenever the division does
/// not come out even, and — worse — makes the "did anything change" check compare a
/// derived count against a differently derived size, so the summary was rebuilt on
/// every frame.
fn minimap_partition(count: usize, rows: usize) -> (usize, usize) {
    let per_bucket = count.div_ceil(rows.max(1)).max(1);
    let buckets = count.div_ceil(per_bucket).max(1);
    (buckets, per_bucket)
}

/// A bucket that has not seen a line yet is **blank**.
///
/// The derived default said `blank: false`, so every bucket the document did not
/// reach — 106 of 120 in the demo's 14-line file — looked like a line and the
/// minimap painted a 1 px bar for each of them: a dashed column down the whole
/// strip instead of a page.
impl Default for MinimapBucket {
    fn default() -> Self {
        Self { indent: 0, length: 0, tone: MiniTone::Plain, blank: true }
    }
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
            let trimmed = text.trim_start();
            slot.tone = if trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.starts_with('#')
            {
                MiniTone::Comment
            } else if trimmed.contains('"') || trimmed.contains('\'') {
                MiniTone::String
            } else {
                MiniTone::Plain
            };
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
