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
//! | `snippet` | Snippet sessions and tab-stop navigation |
//! | `structural` | Structural selection and the navigation stack |
//! | `inlays` | Host overlays, bookmark lifecycle and the diff baseline |
//! | `events` | Event routing: keys, IME, pointer, wheel |
//! | `widget_impl` | The `Widget` and property contracts |

use super::buffer::{
    join_lines, normalize_line_endings, split_lines, EditorBufferSlot, EditorModel,
};
use super::grid::{LineGrid, RowLayout};
use super::multicursor::{self, MultiCursor};
use super::overlays::{CodeLens, InlayHint, SnippetSession, SnippetStop};
use super::pairs::{self, PairAction};
use super::range_command::{TextRangeCommand, TypingAccum, TypingCommand};
use super::syntax::{BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter};
use super::types::{
    CaretStyle, CodeEditorConfig, CompletionSource, CompletionState, ContextMenuState, Cursor,
    CursorGoal, DiagnosticMarker, DocumentCompletions, DocumentScale, DocumentWordIndex,
    EditRejection, EditorBuffer, ExtraSelection, FindState, FoldRegion, InlineDiagnostic,
    LineEnding, MarkerSeverity, MenuItem, ReadOnlySpan, SearchMatch, SearchOptions, SyntaxPalette,
    TextChange, TextPosition, TokenKind, TokenSpan, ViewportSnapshot, VisualLine, MAX_COMPLETIONS,
};
use crate::compat::{lock, Arc, MiniVec, Mutex};
use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::LineNumberGutter;
use crate::widget::{BaseWidget, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

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
mod inlays;
mod minimap_summary;
mod rows;
mod snippet;
mod structural;
mod tabs;
mod tokens;
mod viewport;
mod widget_impl;

/// Shared, cloneable handle to the editing model.
pub(crate) type SharedModel = Rc<RefCell<EditorModel>>;

/// A shared document handle, re-exported from the `document` submodule.
pub use document::CodeEditorDocument;

pub(crate) use self::minimap_summary::minimap_partition;
/// The minimap summary types, re-exported from the `minimap_summary` submodule.
pub use self::minimap_summary::{MiniTone, MinimapBucket};

/// The navigation-history bound, re-exported from the `structural` submodule.
pub use self::structural::MAX_NAVIGATION_HISTORY;

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

/// The live link between a [`CodeEditor`] and a [`LineNumberGutter`].
///
/// The editor owns the gutter so it can push its line range and viewport into it; the gutter's
/// `line_clicked` stream is parked in the shared cell and drained by
/// [`CodeEditor::poll_line_number_gutter`], which routes through the editor's existing
/// [`CodeEditor::goto_line`] command.
struct LineNumberGutterBinding {
    /// The gutter kept in step with the editor's viewport.
    gutter: LineNumberGutter,
    /// The document line a click reported since the last poll.
    clicked: Arc<Mutex<Option<u64>>>,
    /// The subscription, dropped with the binding so a detach leaves no slot behind.
    _scope: ConnectionScope,
}

/// What [`CodeEditor::occurrence_cache`] was computed from.
///
/// The fields are the complete input of "which words recur": the selection
/// being looked for, the document it was looked for in, and whether the feature
/// is switched on at all.
#[derive(Debug, Clone)]
pub(crate) struct OccurrenceCache {
    /// Selection the search used.
    pub(crate) bounds: (TextPosition, TextPosition),
    /// Document revision the search ran against.
    pub(crate) revision: u64,
    /// Whether occurrence highlighting was enabled when the search ran.
    pub(crate) highlight: bool,
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

/// Cached line→visual-row table used by inline diagnostics under wrapping.
///
/// The key is the derived-layout generation; the value is `(line, first row)`
/// pairs in ascending line order, which the marker mapping binary-searches.
pub(crate) type DiagnosticRows = Option<(u64, Vec<(usize, usize)>)>;

/// View state saved per open buffer, so switching tabs restores a buffer's own
/// caret, selection, scroll position and folds instead of resetting them.
///
/// Stored in a map keyed by the buffer's **stable id**, not by its tab position: closing a tab
/// shifts every later position, and a position-keyed store would hand one buffer's caret to its
/// neighbour.
#[derive(Debug, Clone, Default)]
pub(crate) struct BufferViewState {
    pub(crate) cursor: Cursor,
    pub(crate) extra_cursors: Vec<Cursor>,
    pub(crate) goal: CursorGoal,
    pub(crate) scroll_line: usize,
    pub(crate) scroll_column: usize,
    pub(crate) scroll_visual_row: usize,
    pub(crate) folds: Vec<FoldRegion>,
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
    pub(crate) extra_cursors: Vec<Cursor>,
    pub(crate) goal: CursorGoal,
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
    /// Line whose diagnostic message is currently shown as a hover bubble.
    ///
    /// Set from pointer motion over a line that carries a marker, so a reader can see the
    /// full message of any line rather than only the caret's (ED-03).
    pub(crate) hovered_diagnostic: Option<usize>,
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
    /// Fold regions of this view's active buffer.
    ///
    /// Fold state is **view state**, not document state: two views of one document can have
    /// different regions collapsed, the same way they can have different carets. It moved here
    ///from `EditorModel` so a shared document no longer forces one view's folds onto the other.
    /// The per-buffer snapshots live in [`BufferViewState::folds`] alongside caret and scroll.
    pub(crate) folds: Vec<FoldRegion>,
    /// The union of the folded regions as sorted, disjoint `[start, end]` line ranges.
    ///
    /// [`CodeEditor::is_hidden`] answers "is this line inside a fold" for every visible line on
    /// every frame. The regions can nest (a folded block inside another folded block), so
    /// "the last region that starts before this line" is not enough to decide — one earlier
    /// region may extend past several later ones. Their union is disjoint by construction,
    /// which makes the query two binary searches. It is rebuilt whenever the fold set
    /// changes (J-P04).
    pub(crate) hidden_ranges: Vec<(usize, usize)>,
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
    /// Measured geometry of each soft-wrapped segment of the rows last painted.
    ///
    /// A wrapped row is drawn segment by segment, each measured as its own string. Those
    /// measurements are keyed the same way [`Self::row_layouts`] is (the cache is dropped
    /// whenever the layout key moves), so a repaint with no edit reuses them instead of
    /// re-shaping every visible segment. Keyed by `(line, segment)`, kept sorted so the
    /// lookup is a binary search and pruning is a retain over the painted lines.
    pub(crate) segment_layouts: RefCell<Vec<((usize, usize), RowLayout)>>,
    /// Reused document-line window for the paint passes.
    ///
    /// `viewport_document_lines` used to return a freshly collected `Vec` (or an `IntoIter`
    /// built from one) twice per frame — once for the rows and once for the gutter. The
    /// window is derived state, so it lives in one buffer that both passes refill.
    pub(crate) viewport_lines_scratch: RefCell<Vec<usize>>,
    /// Reused token-run buffer for `draw_source_rows`.
    pub(crate) row_run_scratch: RefCell<Vec<(usize, usize, TokenKind)>>,
    /// Reused painted-line list for `draw_source_rows`.
    pub(crate) painted_scratch: RefCell<Vec<usize>>,
    /// Reused occurrence list for `draw_source_rows`.
    ///
    /// The cache itself is keyed and current; the paint loop only needs to read it, but it
    /// calls `&self` helpers that re-borrow the editor, so the matches are copied into this
    /// buffer (whose capacity survives) instead of cloning a fresh `Vec` every frame.
    pub(crate) occurrences_scratch: RefCell<Vec<SearchMatch>>,
    /// Reused status-row text for `draw_status_bar`.
    pub(crate) status_text_scratch: RefCell<String>,
    /// Reused gutter line-number buffer.
    pub(crate) gutter_number_scratch: RefCell<String>,
    /// Reused sorted fold list for `draw_gutter`.
    pub(crate) gutter_folds_scratch: RefCell<Vec<FoldRegion>>,
    /// Reused severity list for `draw_gutter`.
    pub(crate) gutter_severities_scratch: RefCell<Vec<(usize, MarkerSeverity)>>,
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
        usize,
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
    /// The document this view looks at: the buffer set, the revision counter and every
    /// buffer's history.
    ///
    /// Shared by every view created from the same [`CodeEditorDocument`] handle. The view keeps
    /// the *selection* of which buffer it shows (`active_buffer_id`) and points `model` at that
    /// buffer's text, so two views can edit two tabs of one document set.
    pub(crate) document: CodeEditorDocument,
    /// Stable id of the buffer this view shows.
    ///
    /// View state, not document state: a split view may show a different tab of the same
    /// document set. An id, not a position, because positions shift when an earlier buffer is
    /// closed while this view still points at the buffer that moved — which is exactly how a
    /// per-buffer snapshot used to end up attached to its neighbour.
    active_buffer_id: u64,
    /// Cached position of `active_buffer_id` in the document's slot list.
    ///
    /// Maintained by activation and reconciliation. It is what the index-shaped public API
    /// reports, and it is the position a closed active buffer re-homes *from*.
    active_index: usize,
    /// The buffer-list generation these fields were last reconciled with.
    ///
    /// [`CodeEditor::sync_with_document`] compares it with the document's counter: equal means
    /// the tab list did not change since the last look, so the check is one integer comparison
    /// rather than a walk over the tabs on every frame.
    seen_generation: u64,
    /// Undo history of the buffer this view shows.
    ///
    /// A shared handle, not an owned stack: the history belongs to the buffer, so two views of
    /// it undo the same timeline. Switching buffers re-points this at the incoming slot.
    undo_stack: Rc<RefCell<UndoStack>>,
    /// Caret, scroll and fold state of each buffer this view has shown, keyed by buffer id.
    ///
    /// Keyed by the document's stable buffer id rather than by tab position: closing a buffer
    /// shifts every later position, and a position-keyed map would hand one buffer's caret and
    /// folds to its neighbour.
    buffer_views: BTreeMap<u64, BufferViewState>,
    /// The typing run the next keystroke may merge into, if any.
    active_typing: Option<Rc<RefCell<TypingAccum>>>,
    /// `true` while the editor considers itself focused.
    ///
    /// Starts `true` because an editor is constructed as an active insertion
    /// point; a `FocusLost` event stops the caret blink without hiding it.
    focused: bool,
    /// The caret shape painted at the insertion point.
    pub(crate) caret_style: CaretStyle,
    /// Highlights owned by the host.
    pub(crate) extra_selections: Vec<ExtraSelection>,
    /// Lines bookmarked by the user.
    pub(crate) bookmarks: Vec<usize>,
    /// Baseline text an inline diff marker is computed against.
    ///
    /// Owned by the editor rather than the configuration because it is document
    /// content, not a rendering preference. `None` disables the comparison even
    /// when [`CodeEditorConfig::show_inline_diff`] is on.
    diff_baseline: Option<String>,
    /// The baseline split into lines, so the paint pass indexes instead of
    /// re-splitting the text for every visible row.
    diff_baseline_lines: Vec<String>,
    /// Host-supplied inline annotations, sorted by `(line, column)`.
    pub(crate) inlay_hints: Vec<InlayHint>,
    /// Host-supplied line affordances, sorted by line.
    pub(crate) code_lenses: Vec<CodeLens>,
    /// Whether the paint pass draws [`Self::code_lenses`] (RW-115).
    ///
    /// A visibility switch, not a data operation: hiding lenses leaves the host's data in place, so
    /// toggling them back is not a re-push.
    pub(crate) code_lenses_enabled: bool,
    /// The active snippet expansion, when one is running.
    snippet_session: Option<SnippetSession>,
    /// Caret positions recorded by `push_navigation_position`, oldest first.
    navigation_history: Vec<TextPosition>,
    /// Position inside [`Self::navigation_history`] of the current entry.
    navigation_index: usize,
    /// Selections saved before each structural expansion, for `shrink_selection` (RW-114).
    ///
    /// Each entry is the `(anchor, head)` pair an [`Self::expand_selection`] replaced. The stack is
    /// cleared whenever the document changes: a saved position addresses text that no longer
    /// exists after an edit, an undo or a replacement, and restoring it would put the caret on an
    /// invented offset.
    structural_selection_history: Vec<(TextPosition, TextPosition)>,
    /// Lines carrying a breakpoint.
    pub(crate) breakpoints: Vec<usize>,
    /// Whether the gutter reserves a column for breakpoint marks.
    pub(crate) breakpoint_gutter: bool,
    /// Whether the find bar is currently acting as a "go to line" bar.
    pub(crate) goto_bar_visible: bool,
    /// Whether the next Enter in the find bar should jump to a line number.
    pub(crate) goto_mode: bool,
    /// `true` when typing replaces the character after the caret.
    pub(crate) overwrite: bool,
    /// Grey text shown after the caret, supplied by a completion host.
    pub(crate) inline_suggestion: Option<String>,
    /// Progress of the popup fade-in, in `0.0..=1.0`.
    pub(crate) popup_reveal: f32,
    /// Strength of the bracket-pair fade, in `0.0..=1.0`.
    pub(crate) bracket_flash: f32,
    /// Progress of the fold reveal fade, in `0.0..=1.0`.
    pub(crate) fold_reveal: f32,
    /// Target first visual row for eased scrolling, when smoothing is enabled.
    smooth_scroll_target: Option<usize>,
    /// `true` while a press that began on the scrollbar is being dragged.
    pub(crate) scrollbar_dragging: bool,
    /// `true` while a press that began in the gutter is selecting whole lines.
    pub(crate) gutter_dragging: bool,
    /// Line the gutter drag started on.
    pub(crate) gutter_anchor: Option<usize>,
    /// Position of the most recent double-click, used to recognise a triple-click.
    pub(crate) double_click_at: Option<Point>,
    /// Anchor of an in-progress column (rectangular) selection.
    ///
    /// A column selection is expressed as ordinary carets — one per covered line — so
    /// every editing command and the single undo entry work unchanged (ED-04).
    pub(crate) column_select_anchor: Option<TextPosition>,
    /// `true` while a column-selection drag is extending.
    pub(crate) column_selecting: bool,
    /// `true` when a press landed inside the selection, so a drag moves it.
    pub(crate) dragging_text: bool,
    /// Where a text drag started, in document coordinates.
    pub(crate) drag_origin: Option<TextPosition>,
    /// Cached word index for the built-in completion provider.
    completion_index: Option<super::types::DocumentWordIndex>,
    /// Generation counter for derived layout state (folds, wraps, text).
    layout_generation: u64,
    /// Generation counter for the diagnostic marker list (GX-04).
    ///
    /// A marker is not document content, so the document revision does not move when one is added,
    /// replaced or removed. The formatted diagnostic labels are keyed by this counter, which the
    /// three marker mutators bump, so a message that changed is reformatted exactly once.
    pub(crate) diagnostics_generation: u64,
    /// Formatted diagnostic labels, keyed by marker, generation and font (GX-04).
    ///
    /// `RefCell` because the painter takes `&self`. The vector is small and bounded: it holds one
    /// entry per label painted, and it is cleared when it reaches the cap, so a pathological
    /// document cannot grow it without bound.
    pub(crate) diagnostic_labels: RefCell<Vec<super::types::DiagnosticLabelCache>>,
    /// The find bar's summary text and its measured advance (GX-04).
    ///
    /// `draw_find_bar` takes `&mut self`, so this needs no interior mutability.
    pub(crate) find_summary_cache: Option<super::types::FindSummaryCache>,
    /// Formatted fold summaries, keyed by the fold's end/hidden count and the font (GX-04).
    pub(crate) fold_labels: RefCell<Vec<super::types::FoldLabelCache>>,
    /// Cached line→row table used by inline diagnostics under wrapping.
    diagnostic_rows: RefCell<DiagnosticRows>,
    /// How many times the diagnostics row table has been rebuilt, for tests.
    #[cfg(test)]
    pub(crate) diagnostic_cache_builds: core::cell::Cell<u32>,
    /// Region that changed since the last host frame, when it is smaller than the
    /// whole widget (the caret blink).
    damage: RefCell<Option<Rect>>,
    /// Cached bracket-pair index for fold inference.
    bracket_index: Option<(u64, Vec<(TextPosition, TextPosition)>)>,
    restoring_history: bool,
    /// Bytes retained by the most recent undo checkpoint.
    ///
    /// Recorded at push time so a test can assert the history cost tracks the edit
    /// rather than the document. Nothing reads it in production, hence `cfg(test)`
    /// on the accessor rather than on the field — keeping the field unconditional
    /// avoids a cfg-shaped hole in the struct initialiser.
    last_checkpoint_bytes: usize,
    /// A monotone counter bumped on every committed text change (BLUE26 B26.08).
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
    ///
    /// Shared through an `Rc<Cell<_>>` because two editors can present **one document**
    /// ([`CodeEditor::with_document`]): an edit made through either view must be visible as a
    /// revision change to both, or the second view keeps serving caches keyed by a stale text.
    revision: Rc<core::cell::Cell<u64>>,

    /// Emitted when text changes.
    ///
    /// This carries the whole document and is therefore emitted only by the paths that
    /// already materialise it: a wholesale replacement ([`Self::set_text`], including
    /// [`Self::load_document`]) and **every** undo or redo, whose restore path rebuilds
    /// the text from the model. A tab switch emits [`Self::text_change`] only, because
    /// the incoming buffer is already stored and cloning it into the signal would
    /// duplicate the load. Per-keystroke edits emit [`Self::text_change`] instead (see
    /// [`TextChange`]); a host that needs the full text after every edit can subscribe
    /// to `text_change` and call [`Self::text`] when its own contract requires it.
    pub text_changed: Signal1<String>,
    /// Emitted for **every** committed text change, as a line-range description.
    ///
    /// One whole-document clone per keystroke was the single largest constant in the
    /// typing path; the description costs the size of the edit instead.
    pub text_change: Signal1<TextChange>,
    /// Emitted with the marker index when a diagnostic marker is clicked in the gutter.
    pub marker_clicked: Signal1<usize>,
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
    /// Emitted with `(current hit, total hits, line, column)` after a search change.
    pub search_result: Signal1<(usize, usize, usize, usize)>,
    /// Emitted with the new font size after a zoom command.
    pub zoom_changed: Signal1<f32>,
    /// Emitted when a line's breakpoint state changes.
    pub breakpoint_toggled: Signal1<(usize, bool)>,
    /// Emitted when a gutter marker on a line is activated.
    pub marker_activated: Signal1<usize>,
    /// Emitted when the editor rebuilds its palette for a new appearance.
    pub palette_changed: GenericSignal,
    /// Emitted with the pointer position when a context menu is requested.
    ///
    /// Always emitted, before (and independently of) the built-in menu, so a
    /// host that sets [`CodeEditorConfig::built_in_context_menu`] to `false` owns
    /// the menu entirely and one that leaves it on can still observe the request.
    pub context_menu_requested: Signal1<Point>,
    /// The line-number gutter this editor keeps in step, when one is attached.
    line_number_gutter: Option<LineNumberGutterBinding>,
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
        let undo_limit = config.undo_stack_limit;
        let document = CodeEditorDocument::new(String::new());
        document.set_history_limit(undo_limit);
        let model = document.slot_model(0);
        let undo_stack = document.slot_history(0);
        let revision = document.revision_handle();
        let active_buffer_id = document.buffer_id(0).unwrap_or(1);
        let seen_generation = document.generation();
        let mut buffer_views = BTreeMap::new();
        buffer_views.insert(active_buffer_id, BufferViewState::default());
        let mut editor = Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "CodeEditor"),
            model,
            config,
            document,
            active_buffer_id,
            active_index: 0,
            seen_generation,
            undo_stack,
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
            hovered_diagnostic: None,
            clipboard_has_text: None,
            read_only_spans: Vec::new(),
            folds: Vec::new(),
            hidden_ranges: Vec::new(),
            folded_lines: MiniVec::new(),
            visual_row_prefix: Vec::new(),
            line_states: Vec::new(),
            line_tokens: Vec::new(),
            occurrence_cache: None,
            bracket_cache: RefCell::new(None),
            row_layouts: RefCell::new(Vec::new()),
            segment_layouts: RefCell::new(Vec::new()),
            viewport_lines_scratch: RefCell::new(Vec::new()),
            row_run_scratch: RefCell::new(Vec::new()),
            painted_scratch: RefCell::new(Vec::new()),
            occurrences_scratch: RefCell::new(Vec::new()),
            status_text_scratch: RefCell::new(String::new()),
            gutter_number_scratch: RefCell::new(String::new()),
            gutter_folds_scratch: RefCell::new(Vec::new()),
            gutter_severities_scratch: RefCell::new(Vec::new()),
            row_layout_key: core::cell::Cell::new((
                0,
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
            buffer_views,
            active_typing: None,
            focused: true,
            caret_style: CaretStyle::Line,
            extra_selections: Vec::new(),
            bookmarks: Vec::new(),
            diff_baseline: None,
            diff_baseline_lines: Vec::new(),
            inlay_hints: Vec::new(),
            code_lenses: Vec::new(),
            code_lenses_enabled: true,
            snippet_session: None,
            navigation_history: Vec::new(),
            navigation_index: 0,
            structural_selection_history: Vec::new(),
            breakpoints: Vec::new(),
            breakpoint_gutter: false,
            goto_bar_visible: false,
            goto_mode: false,
            overwrite: false,
            inline_suggestion: None,
            popup_reveal: 1.0,
            bracket_flash: 0.0,
            fold_reveal: 1.0,
            smooth_scroll_target: None,
            scrollbar_dragging: false,
            gutter_dragging: false,
            gutter_anchor: None,
            double_click_at: None,
            column_select_anchor: None,
            column_selecting: false,
            dragging_text: false,
            drag_origin: None,
            completion_index: None,
            layout_generation: 0,
            diagnostics_generation: 0,
            diagnostic_labels: RefCell::new(Vec::new()),
            find_summary_cache: None,
            fold_labels: RefCell::new(Vec::new()),
            diagnostic_rows: RefCell::new(None),
            #[cfg(test)]
            diagnostic_cache_builds: core::cell::Cell::new(0),
            damage: RefCell::new(None),
            bracket_index: None,
            restoring_history: false,
            last_checkpoint_bytes: 0,
            revision,
            text_changed: Signal1::new(),
            text_change: Signal1::new(),
            marker_clicked: Signal1::new(),
            cursor_moved: Signal1::new(),
            selection_changed: Signal1::new(),
            tab_changed: Signal1::new(),
            search_changed: Signal1::new(),
            fold_changed: Signal1::new(),
            completion_changed: Signal1::new(),
            search_result: Signal1::new(),
            zoom_changed: Signal1::new(),
            breakpoint_toggled: Signal1::new(),
            marker_activated: Signal1::new(),
            palette_changed: GenericSignal::new(),
            context_menu_requested: Signal1::new(),
            line_number_gutter: None,
        };
        // An editable editor is an active insertion point the moment it exists, so its caret blinks
        // from construction rather than waiting for a focus event this widget has no notion of.
        if !editor.config.read_only {
            editor.caret_blink.start();
        }
        editor.refresh_derived_state();
        editor
    }

    /// Follows a [`LineNumberGutter`]'s viewport and click stream.
    ///
    /// This is the companion binding for COMP-A8. The editor owns the gutter because only a
    /// mutable owner can push the current line range into it: the range and visible window are
    /// written immediately on attach, and [`Self::poll_line_number_gutter`] compares them with the
    /// editor's own state on every poll so scrolling, edits and buffer switches reach the gutter
    /// without host bookkeeping.
    ///
    /// The reverse direction is the gutter's `line_clicked` stream: a click is parked in the
    /// binding and applied through [`Self::goto_line`], the editor's existing command, so the
    /// gutter does not need a second navigation path. Attaching replaces any previous gutter, so
    /// calling this twice leaves one subscription.
    pub fn attach_line_number_gutter(&mut self, gutter: LineNumberGutter) {
        let clicked = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&clicked);
        let scope = ConnectionScope::new();
        let redraw = self.base.redraw_requested_signal().clone();
        gutter.line_clicked.connect_scoped(&scope, move |line| {
            *lock(&sink) = Some(*line);
            // A gutter click is a command, so the poll that applies it runs next frame.
            redraw.emit();
        });
        self.line_number_gutter = Some(LineNumberGutterBinding { gutter, clicked, _scope: scope });
        self.apply_editor_range_to_gutter();
        self.base.request_redraw();
    }

    /// Stops following the attached gutter and returns it.
    pub fn detach_line_number_gutter(&mut self) -> Option<LineNumberGutter> {
        let binding = self.line_number_gutter.take()?;
        self.base.request_redraw();
        Some(binding.gutter)
    }

    /// Whether a [`LineNumberGutter`] is attached.
    pub fn is_line_number_gutter_attached(&self) -> bool {
        self.line_number_gutter.is_some()
    }

    /// The attached gutter, if any.
    pub fn line_number_gutter(&self) -> Option<&LineNumberGutter> {
        self.line_number_gutter.as_ref().map(|binding| &binding.gutter)
    }

    /// The attached gutter for editing, if any.
    pub fn line_number_gutter_mut(&mut self) -> Option<&mut LineNumberGutter> {
        self.line_number_gutter.as_mut().map(|binding| &mut binding.gutter)
    }

    /// Applies the editor's range and viewport to the attached gutter and drains a pending click.
    ///
    /// Returns whether anything changed: a pushed range/window/current line or a click routed
    /// through [`Self::goto_line`]. A second call with nothing pending returns `false`.
    pub fn poll_line_number_gutter(&mut self) -> bool {
        let pending =
            self.line_number_gutter.as_ref().and_then(|binding| lock(&binding.clicked).take());
        let mut changed = false;
        if let Some(line) = pending {
            self.goto_line(line.saturating_sub(1) as usize);
            changed = true;
        }
        // Push after applying a click, so the caret line the command produced is written in the
        // same poll rather than leaving the gutter one frame behind.
        changed |= self.apply_editor_range_to_gutter();
        changed
    }

    /// Writes the editor's line range and visible window into the attached gutter.
    ///
    /// Using only public accessors keeps the gutter's own paint and hit-test arithmetic as the
    /// single reading of the range. Returns whether any stored value moved.
    fn apply_editor_range_to_gutter(&mut self) -> bool {
        let first = self.scroll_line().saturating_add(1) as u64;
        let line_count = self.line_count() as u64;
        let remaining = self.line_count().saturating_sub(self.scroll_line());
        let visible = self.visible_rows().min(remaining).max(1) as u64;
        let current = self.cursor.head.line.saturating_add(1) as u64;
        let Some(binding) = self.line_number_gutter.as_mut() else {
            return false;
        };
        let mut changed = false;
        if binding.gutter.first_line() != 1 || binding.gutter.line_count() != line_count {
            binding.gutter.set_range(1, line_count);
            changed = true;
        }
        if binding.gutter.visible_lines() != (first, visible) {
            binding.gutter.set_visible_lines(first, visible);
            changed = true;
        }
        if binding.gutter.current_line() != current {
            binding.gutter.set_current_line(current);
            changed = true;
        }
        changed
    }

    /// Returns the active configuration.
    pub fn config(&self) -> &CodeEditorConfig {
        &self.config
    }

    /// Replaces the editor's configuration at runtime (RW-112).
    ///
    /// A host that toggles the minimap, wrapping or line numbers used to have to rebuild the
    /// control, losing its undo history and selection. The fields are copied into the live
    /// configuration instead; the undo limit is forwarded to the document because that is where the
    /// history bound lives, and one full-surface damage rectangle is recorded so the next frame
    /// repaints what the change altered.
    ///
    /// The configuration is validated exactly as [`Self::with_config`] validates it, so a value
    /// that could not be constructed cannot be installed later either.
    pub fn set_config(&mut self, config: CodeEditorConfig) -> Result<(), &'static str> {
        config.validate()?;
        let undo_limit = config.undo_stack_limit;
        let language_changed = self.config.language != config.language;
        let wrap_geometry_changed = self.config.word_wrap != config.word_wrap
            || self.config.tab_width != config.tab_width
            || self.config.show_line_numbers != config.show_line_numbers
            || self.config.show_minimap != config.show_minimap
            || self.config.font_size.to_bits() != config.font_size.to_bits()
            || self.config.font_family != config.font_family;
        self.config = config;
        self.document.set_history_limit(undo_limit);
        if language_changed {
            self.invalidate_line_states();
        }
        if wrap_geometry_changed {
            // The wrap geometry feeds the row-prefix table and the measured cell width;
            // without these the document keeps the old ruler until the next edit.
            self.measured_geometry = None;
            self.refresh_derived_state();
        }
        // The occurrence list now carries `highlight_occurrences` in its key, but a
        // language change still alters what counts as a word boundary; dropping the
        // cache here keeps `set_config` from relying on the next paint to notice.
        self.occurrence_cache = None;
        self.mark_display_damage();
        self.base.request_layout();
        Ok(())
    }

    /// Shows or hides the gutter line numbers at runtime (RW-112).
    pub fn set_show_line_numbers(&mut self, show: bool) {
        if self.config.show_line_numbers == show {
            return;
        }
        self.config.show_line_numbers = show;
        // The gutter width is part of `text_columns`, so the wrapped-row table has to be
        // rebuilt with it; otherwise row/line mapping keeps using the old width.
        self.refresh_derived_state();
        self.mark_display_damage();
        self.base.request_layout();
    }

    /// Shows or hides the minimap at runtime (RW-112).
    pub fn set_show_minimap(&mut self, show: bool) {
        if self.config.show_minimap == show {
            return;
        }
        self.config.show_minimap = show;
        self.refresh_derived_state();
        self.mark_display_damage();
        self.base.request_layout();
    }

    /// Turns soft wrapping on or off at runtime (RW-112).
    pub fn set_word_wrap(&mut self, wrap: bool) {
        if self.config.word_wrap == wrap {
            return;
        }
        self.config.word_wrap = wrap;
        self.refresh_derived_state();
        self.mark_display_damage();
        self.base.request_layout();
    }

    /// Shows or hides the whitespace dots and tab arrows at runtime (RW-112).
    pub fn set_show_whitespace(&mut self, show: bool) {
        if self.config.show_whitespace == show {
            return;
        }
        self.config.show_whitespace = show;
        self.mark_display_damage();
    }

    /// Records the whole surface as damaged and asks for a frame.
    ///
    /// A display setting changes what every row paints (a gutter appears, lines re-wrap), so the
    /// partial caret-blink rectangle is the wrong channel: the whole box is the smallest honest
    /// damage.
    fn mark_display_damage(&mut self) {
        let full = self.base.geometry();
        *self.damage.borrow_mut() = Some(full);
        self.base.request_redraw();
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

    /// Returns `true` when the editor is treated as focused.
    ///
    /// A blurred editor keeps its caret visible but stops asking for frames; the
    /// previous code animated the caret forever, so a window with ten editors kept
    /// every one of them repainting.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Records a focus change and starts or stops the caret blink accordingly.
    ///
    /// The caret stays **visible** when focus is lost: a hidden caret on a
    /// blurred editor reads as a rendering failure rather than as "focus is
    /// elsewhere". Only the animation stops.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        if focused && self.is_editable() {
            self.caret_blink.start();
        } else {
            self.caret_blink.stop();
        }
        self.base.request_redraw();
    }

    /// Returns the caret shape.
    pub fn caret_style(&self) -> CaretStyle {
        self.caret_style
    }

    /// Sets the caret shape.
    pub fn set_caret_style(&mut self, style: CaretStyle) {
        if self.caret_style == style {
            return;
        }
        self.caret_style = style;
        self.base.request_redraw();
    }

    /// Returns `true` when typing overwrites the character after the caret.
    pub fn overwrite_mode(&self) -> bool {
        self.overwrite
    }

    /// Sets overwrite mode.
    pub fn set_overwrite_mode(&mut self, overwrite: bool) {
        if self.overwrite == overwrite {
            return;
        }
        self.overwrite = overwrite;
        self.base.request_redraw();
    }

    /// Toggles between insert and overwrite mode, as the `Insert` key does.
    pub fn toggle_overwrite_mode(&mut self) -> bool {
        self.overwrite = !self.overwrite;
        self.base.request_redraw();
        self.overwrite
    }

    /// Returns the inline completion hint, if one is set.
    pub fn inline_suggestion(&self) -> Option<&str> {
        self.inline_suggestion.as_deref()
    }

    /// Sets the grey completion hint painted after the caret.
    ///
    /// The hint is never inserted into the document; accepting it is the host's
    /// call (it can read this value and call [`CodeEditor::insert`]).
    pub fn set_inline_suggestion(&mut self, suggestion: Option<String>) {
        if self.inline_suggestion == suggestion {
            return;
        }
        self.inline_suggestion = suggestion;
        self.base.request_redraw();
    }

    /// Returns the host-owned highlight ranges.
    pub fn extra_selections(&self) -> &[ExtraSelection] {
        &self.extra_selections
    }

    /// Replaces the host-owned highlight ranges.
    pub fn set_extra_selections(&mut self, selections: Vec<ExtraSelection>) {
        if self.extra_selections == selections {
            return;
        }
        self.extra_selections = selections;
        self.base.request_redraw();
    }

    /// Adds one host-owned highlight range.
    pub fn add_extra_selection(&mut self, selection: ExtraSelection) {
        if selection.start >= selection.end {
            return;
        }
        self.extra_selections.push(selection);
        self.base.request_redraw();
    }

    /// Removes every host-owned highlight range.
    pub fn clear_extra_selections(&mut self) {
        if self.extra_selections.is_empty() {
            return;
        }
        self.extra_selections.clear();
        self.base.request_redraw();
    }

    /// Returns the bookmarked lines, ascending and deduplicated.
    pub fn bookmarks(&self) -> &[usize] {
        &self.bookmarks
    }

    /// Toggles a bookmark on `line`, clamped to the document.
    pub fn toggle_bookmark(&mut self, line: usize) -> bool {
        let line = line.min(self.line_count().saturating_sub(1));
        let bookmarked = if let Some(index) = self.bookmarks.iter().position(|entry| *entry == line)
        {
            self.bookmarks.remove(index);
            false
        } else {
            self.bookmarks.push(line);
            self.bookmarks.sort_unstable();
            true
        };
        self.base.request_redraw();
        bookmarked
    }

    /// Moves the caret to the next bookmark after it, wrapping to the first.
    pub fn goto_next_bookmark(&mut self) -> bool {
        let Some(target) = self
            .bookmarks
            .iter()
            .find(|line| **line > self.cursor.head.line)
            .or_else(|| self.bookmarks.first())
            .copied()
        else {
            return false;
        };
        self.move_caret_to(TextPosition::new(target, 0), false);
        true
    }

    /// Returns whether `undo`/`redo` are allowed to run.
    pub fn undo_redo_enabled(&self) -> bool {
        self.config.undo_redo_enabled
    }

    /// Enables or disables `undo`/`redo` without discarding the history.
    pub fn set_undo_redo_enabled(&mut self, enabled: bool) {
        if self.config.undo_redo_enabled == enabled {
            return;
        }
        self.config.undo_redo_enabled = enabled;
    }

    /// Returns how many undo steps the active buffer holds.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.borrow().undo_count()
    }

    /// Sets the undo-step limit for every buffer of the document.
    ///
    /// The history belongs to the document, so the limit is applied to every buffer rather
    /// than only to the one this view shows. Two views with different limits therefore agree
    /// on the last one applied, which is stated here rather than silently.
    pub fn set_undo_limit(&mut self, limit: usize) {
        self.config.undo_stack_limit = limit;
        self.document.set_history_limit(limit);
    }

    /// Returns the lines carrying a breakpoint, ascending.
    pub fn breakpoints(&self) -> &[usize] {
        &self.breakpoints
    }

    /// Returns whether the gutter reserves space for breakpoint marks.
    pub fn is_breakpoint_gutter(&self) -> bool {
        self.breakpoint_gutter
    }

    /// Shows or hides the breakpoint gutter column.
    pub fn set_breakpoint_gutter(&mut self, visible: bool) {
        if self.breakpoint_gutter == visible {
            return;
        }
        self.breakpoint_gutter = visible;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether `line` carries a breakpoint.
    pub fn is_breakpoint(&self, line: usize) -> bool {
        self.breakpoints.binary_search(&line).is_ok()
    }

    /// Adds a breakpoint to `line`, returning whether it was added.
    pub fn add_breakpoint(&mut self, line: usize) -> bool {
        let line = line.min(self.line_count().saturating_sub(1));
        if self.is_breakpoint(line) {
            return false;
        }
        let index = self.breakpoints.partition_point(|entry| *entry < line);
        self.breakpoints.insert(index, line);
        self.breakpoint_toggled.emit((line, true));
        self.base.request_redraw();
        true
    }

    /// Removes the breakpoint from `line`, returning whether one was removed.
    pub fn remove_breakpoint(&mut self, line: usize) -> bool {
        let Ok(index) = self.breakpoints.binary_search(&line) else { return false };
        self.breakpoints.remove(index);
        self.breakpoint_toggled.emit((line, false));
        self.base.request_redraw();
        true
    }

    /// Toggles the breakpoint on `line`, returning the resulting state.
    pub fn toggle_breakpoint(&mut self, line: usize) -> bool {
        if self.is_breakpoint(line.min(self.line_count().saturating_sub(1))) {
            self.remove_breakpoint(line.min(self.line_count().saturating_sub(1)));
            false
        } else {
            self.add_breakpoint(line);
            true
        }
    }

    /// Increases the font size by one point.
    pub fn zoom_in(&mut self) {
        self.apply_zoom(self.config.font_size + 1.0);
    }

    /// Decreases the font size by one point.
    pub fn zoom_out(&mut self) {
        self.apply_zoom(self.config.font_size - 1.0);
    }

    /// Restores the default font size.
    pub fn reset_zoom(&mut self) {
        self.apply_zoom(super::types::DEFAULT_FONT_SIZE);
    }

    /// Applies a new font size and invalidates everything measured from the old one.
    fn apply_zoom(&mut self, size: f32) {
        let size = size.clamp(6.0, 48.0);
        if (size - self.config.font_size).abs() < f32::EPSILON {
            return;
        }
        self.config.font_size = size;
        // Clearing the explicit advance restores the derived ratio.
        self.config.line_advance = 0.0;
        self.measured_geometry = None;
        self.refresh_visible_rows();
        // A different cell width changes `text_columns`, so the wrapped line→row table
        // diagnostics cache is stale even though the text did not move.
        self.bump_layout_generation();
        self.base.request_layout();
        self.base.request_redraw();
        self.zoom_changed.emit(size);
    }

    /// The screen position of the IME composition's start, when one is active.
    ///
    /// The preedit is drawn at the caret; a host that needs to place a candidate
    /// window reads this instead of recomputing the row height and column width.
    /// The offset of `Preedit::cursor` inside the composition is *not* included,
    /// because that is the host's text layout, not the document's.
    pub fn preedit_screen_origin(&self) -> Option<Point> {
        self.preedit.as_ref()?;
        let map = self.coordinate_map();
        let row = self.caret_visual_row();
        Some(Point::new(self.caret_x(self.cursor.head), map.row_y(row)))
    }

    /// Sets the text drawn while the document is empty.
    pub fn set_placeholder_text(&mut self, text: Option<String>) {
        if self.config.placeholder_text == text {
            return;
        }
        self.config.placeholder_text = text;
        self.base.request_redraw();
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention. The caret here was drawn at full strength
    /// unconditionally, so a host had no reason to keep scheduling frames and the caret read as a
    /// frozen marker rather than a live insertion point.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let mut running = false;
        let mut caret_changed = false;
        if self.is_editable() && self.focused {
            let was_visible = self.caret_blink.is_visible();
            running = self.caret_blink.tick(delta_ms);
            caret_changed = was_visible != self.caret_blink.is_visible();
        }
        if self.config.fade_popups
            && (self.completion.visible || self.context_menu.visible)
            && self.popup_reveal < 1.0
        {
            self.popup_reveal = (self.popup_reveal + delta_ms as f32 / 120.0).min(1.0);
            running = true;
        }
        if self.config.animate_brackets && self.bracket_flash > 0.0 {
            self.bracket_flash = (self.bracket_flash - delta_ms as f32 / 180.0).max(0.0);
            running = true;
        }
        if self.config.animate_folds && self.fold_reveal < 1.0 {
            self.fold_reveal = (self.fold_reveal + delta_ms as f32 / 150.0).min(1.0);
            running = true;
        }
        if self.smooth_scroll_target.is_some() {
            running = true;
            self.step_smooth_scroll();
        }
        if caret_changed {
            self.mark_caret_damage();
        }
        if running {
            self.base.request_frame_repaint();
        }
        running
    }

    /// Records the caret's rectangle as the region that needs repainting.
    fn mark_caret_damage(&mut self) {
        let row = self.caret_visual_row();
        let y = self.coordinate_map().row_y(row);
        let x = self.caret_x(self.cursor.head);
        *self.damage.borrow_mut() =
            Some(Rect::new(x.saturating_sub(1), y, 4, self.line_height().ceil().max(1.0) as u32));
    }

    /// Takes the region that changed since the last call, if it is a partial one.
    ///
    /// A host may repaint only this rectangle after a blink tick instead of the
    /// whole editor; an edit clears it so the next frame is full-width.
    pub fn take_damage_rect(&mut self) -> Option<Rect> {
        self.damage.borrow_mut().take()
    }

    /// Advances one step toward the pending eased scroll target.
    fn step_smooth_scroll(&mut self) {
        let Some(target) = self.smooth_scroll_target else { return };
        let current = self.scroll_visual_row;
        let step = 4usize;
        let next = if target > current {
            (current + step).min(target)
        } else {
            current.saturating_sub(step).max(target)
        };
        if next != current {
            self.scroll_visual_row = next;
            self.scroll_line = self.visual_row_to_line(next);
            self.base.request_redraw();
        }
        if next == target {
            self.smooth_scroll_target = None;
        }
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
        if self.config.tab_width == width {
            return;
        }
        self.config.tab_width = width;
        self.refresh_derived_state();
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

    /// Appends the active language name to `out`, reusing its capacity.
    ///
    /// The status row asks for this name once per frame; [`Self::language_name`] returns an
    /// owned `String` (the public contract), so the paint path writes into its own buffer
    /// instead of paying for a fresh string per paint.
    pub(crate) fn write_language_name(&self, out: &mut String) {
        let language = self.active_language();
        if let Some((_, highlighter)) =
            self.language_highlighters.iter().find(|(registered, _)| *registered == language)
        {
            out.push_str(highlighter.language_name());
            return;
        }
        if let Some(highlighter) = &self.highlighter {
            out.push_str(highlighter.language_name());
            return;
        }
        out.push_str(language.name());
    }

    /// Returns the language in force for the active buffer.
    ///
    /// A buffer's [`EditorBuffer::language`] override wins over
    /// [`CodeEditorConfig::language`]. This is what makes one widget host several
    /// tabs of different languages (whitepaper §14.2): the field existed before but
    /// nothing consulted it, so every tab was lexed as the configured language.
    pub fn active_language(&self) -> LanguageId {
        self.document.language_at(self.active_buffer()).flatten().unwrap_or(self.config.language)
    }

    /// Sets the built-in language and resets indentation to its default.
    ///
    /// The setting applies to the active buffer when it has a language override,
    /// and to the editor default otherwise, so a multi-language host does not need
    /// two entry points.
    pub fn set_language(&mut self, language: LanguageId) {
        self.config.language = language;
        self.config.tab_width = language.default_tab_width();
        self.document.set_language_at(self.active_buffer(), Some(language));
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
        let changed = self.document.set_language_at(index, language);
        if changed && index == self.active_buffer() {
            self.invalidate_line_states();
            self.base.request_redraw();
        }
        changed
    }

    /// Returns the language recorded for the buffer at `index`.
    pub fn buffer_language(&self, index: usize) -> Option<LanguageId> {
        self.document.language_at(index).flatten()
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

mod helpers;

use helpers::{
    byte_and_char_at, char_len_of, clamp_and_merge, closes_bracket, diff_range, is_skipped,
    opens_bracket, position_in_text, shift_index, trailing_line_len, whole_word_ok, Checkpoint,
};
pub(crate) use helpers::{decimal_digits, MAX_MINIMAP_ROWS};

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

#[cfg(test)]
mod gutter_binding_tests {
    use super::*;

    fn editor_with_lines(count: usize) -> CodeEditor {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 200));
        let text = (0..count).map(|line| format!("line {line}")).collect::<Vec<_>>().join("\n");
        editor.set_text(text);
        editor
    }

    /// Attach pushes the range/window, poll pushes later edits and scrolls, and detach stops both.
    #[test]
    fn the_gutter_follows_the_editor_range_and_viewport() {
        let mut editor = editor_with_lines(30);
        editor.attach_line_number_gutter(LineNumberGutter::new(Rect::new(0, 0, 40, 200)));
        assert!(editor.is_line_number_gutter_attached());
        let gutter = editor.line_number_gutter().expect("attached");
        assert_eq!(gutter.line_count(), 30, "attach pushes the document length");
        assert_eq!(gutter.visible_lines().0, 1, "attach pushes the first visible line");
        assert!(!editor.poll_line_number_gutter(), "an unchanged editor is not re-applied");

        let longer = (0..40).map(|line| format!("line {line}")).collect::<Vec<_>>().join("\n");
        editor.set_text(longer);
        assert!(editor.poll_line_number_gutter(), "an edit reaches the gutter");
        assert_eq!(editor.line_number_gutter().expect("attached").line_count(), 40);

        editor.scroll_by(3);
        assert!(editor.poll_line_number_gutter(), "a scroll reaches the gutter");
        assert_eq!(editor.line_number_gutter().expect("attached").visible_lines().0, 4);
        assert!(!editor.poll_line_number_gutter(), "the second poll has nothing to do");

        let gutter = editor.detach_line_number_gutter().expect("detached");
        assert!(!editor.is_line_number_gutter_attached());
        editor.scroll_by(2);
        assert!(!editor.poll_line_number_gutter(), "a detached gutter is not polled");
        assert_eq!(gutter.visible_lines().0, 4, "detach stops updates");
    }

    /// A gutter click is routed through the editor's existing `goto_line` command.
    #[test]
    fn a_gutter_click_moves_the_caret() {
        let mut editor = editor_with_lines(10);
        editor.attach_line_number_gutter(LineNumberGutter::new(Rect::new(0, 0, 40, 200)));
        editor.line_number_gutter_mut().expect("attached").line_clicked.emit(4);

        assert!(editor.poll_line_number_gutter(), "the click is applied");
        assert_eq!(editor.cursor().0, 3, "a gutter line is one-based; goto_line is zero-based");
        assert!(!editor.poll_line_number_gutter(), "the click is drained once");
    }

    /// RW-112: the four display settings flip on an open editor, and the change damages the
    /// whole surface so the next frame repaints it.
    #[test]
    fn display_settings_change_at_runtime() {
        let mut editor =
            CodeEditor::with_config(Rect::new(0, 0, 420, 300), CodeEditorConfig::default())
                .expect("the default configuration is valid");
        let _ = editor.take_damage_rect();

        editor.set_show_line_numbers(false);
        assert!(!editor.config().show_line_numbers);
        assert_eq!(
            editor.take_damage_rect(),
            Some(editor.geometry()),
            "a display toggle damages the whole surface"
        );

        editor.set_show_minimap(true);
        assert!(editor.config().show_minimap);
        editor.set_word_wrap(true);
        assert!(editor.config().word_wrap);
        editor.set_show_whitespace(true);
        assert!(editor.config().show_whitespace);

        // The undo history survives the change: the rebuild was what this replaced.
        editor.insert("hello");
        editor.set_show_line_numbers(true);
        editor.undo();
        assert!(editor.text().is_empty(), "the history is still there");

        // A configuration that could not be constructed cannot be installed later.
        let mut bad = CodeEditorConfig::default();
        bad.tab_width = 0;
        assert!(editor.set_config(bad).is_err());
        assert!(editor.config().tab_width > 0, "a refused config leaves the old one in place");
    }
}
