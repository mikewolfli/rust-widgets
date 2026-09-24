// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Value types shared by the code editor submodules.
//!
//! Everything here is plain data: no editor logic, no rendering, no I/O. This
//! keeps the widget state machine in `editor.rs` and the drawing code in
//! `render.rs` free of type churn.

use crate::core::{Color, Point};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

// ── Shared constants ─────────────────────────────────────────────────────────

/// Default monospace family used by the editor surface.
pub const DEFAULT_FONT_FAMILY: &str = "Menlo";
/// Default font size in points for the editor surface.
pub const DEFAULT_FONT_SIZE: f32 = 13.0;
/// Default tab width measured in spaces.
pub const DEFAULT_TAB_WIDTH: usize = 4;
/// Minimum height of any touch-driven hit target, in logical pixels.
pub const MIN_TOUCH_TARGET: i32 = 28;
/// Upper bound on completion candidates produced by one request.
pub const MAX_COMPLETIONS: usize = 24;

// ── Token styling ────────────────────────────────────────────────────────────

/// Token categories produced by a [`super::syntax::SyntaxHighlighter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenKind {
    /// Unstyled source text.
    Plain,
    /// Language keyword (`fn`, `let`, `struct`, ...).
    Keyword,
    /// Type or primitive name (`String`, `u32`, ...).
    Type,
    /// Function or method name.
    Function,
    /// String literal, including delimiters.
    String,
    /// Character literal.
    Character,
    /// Numeric literal.
    Number,
    /// Line comment.
    Comment,
    /// Block comment.
    BlockComment,
    /// Preprocessor or attribute line (`#[...]`, `#define`).
    Preprocessor,
    /// Operator or punctuation run.
    Operator,
    /// Identifier that is not a keyword, type or function.
    Identifier,
}

/// One styled span of a single source line.
///
/// `start`/`end` are byte offsets into the line. Spans returned by
/// `CodeEditor::tokens_on_line` are guaranteed to fall on `char` boundaries;
/// spans from a third-party highlighter are clamped, never trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSpan {
    /// Start byte offset inside the line.
    pub start: usize,
    /// End byte offset (exclusive) inside the line.
    pub end: usize,
    /// Token category.
    pub kind: TokenKind,
}

impl TokenSpan {
    /// Creates a span from byte offsets.
    pub fn new(start: usize, end: usize, kind: TokenKind) -> Self {
        Self { start, end, kind }
    }

    /// Returns `true` when the span covers no text.
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Returns the byte length of the span.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// Colour palette for token categories and editor decorations.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxPalette {
    /// Foreground colour per token kind.
    pub colors: Vec<(TokenKind, Color)>,
    /// Bracket-pair highlight colour.
    pub bracket_color: Color,
    /// Active-line background.
    pub active_line_color: Color,
    /// Indent guide colour.
    pub indent_guide_color: Color,
    /// Selection background.
    pub selection_color: Color,
    /// Search-hit background.
    pub search_color: Color,
    /// Active search-hit background.
    pub search_current_color: Color,
    /// Occurrence-outline colour.
    pub occurrence_color: Color,
    /// Gutter background.
    pub gutter_background: Color,
    /// Editor background.
    pub background: Color,
    /// Border colour.
    pub border_color: Color,
    /// Chrome (tab strip, status bar, find bar) background.
    pub chrome_background: Color,
    /// Primary chrome text colour.
    pub chrome_text: Color,
    /// Dimmed chrome text colour.
    pub chrome_dim_text: Color,
}

impl Default for SyntaxPalette {
    fn default() -> Self {
        Self::light()
    }
}

impl SyntaxPalette {
    /// The palette for a light editor background.
    ///
    /// # Why the light palette is the `Default`
    ///
    /// `Default` has to pick one, and this is the one an unthemed build drew — keeping it means
    /// a build with no theme is byte-for-byte unchanged. `CODE_EDITOR`'s constructor picks
    /// between this and [`Self::dark`] by the active appearance, so a themed host never gets
    /// the other one by accident.
    pub fn light() -> Self {
        Self {
            colors: vec![
                (TokenKind::Plain, Color::rgb(43, 55, 74)),
                (TokenKind::Keyword, Color::rgb(160, 62, 176)),
                (TokenKind::Type, Color::rgb(26, 110, 168)),
                (TokenKind::Function, Color::rgb(30, 130, 96)),
                (TokenKind::String, Color::rgb(174, 100, 34)),
                (TokenKind::Character, Color::rgb(150, 96, 30)),
                (TokenKind::Number, Color::rgb(186, 88, 48)),
                (TokenKind::Comment, Color::rgb(96, 112, 102)),
                (TokenKind::BlockComment, Color::rgb(96, 112, 102)),
                (TokenKind::Preprocessor, Color::rgb(120, 96, 60)),
                (TokenKind::Operator, Color::rgb(72, 88, 112)),
                (TokenKind::Identifier, Color::rgb(43, 55, 74)),
            ],
            bracket_color: Color::rgb(120, 158, 212),
            active_line_color: Color::rgb(243, 247, 253),
            indent_guide_color: Color::rgb(228, 234, 244),
            selection_color: Color::rgb(206, 226, 250),
            search_color: Color::rgb(255, 232, 158),
            search_current_color: Color::rgb(255, 198, 92),
            occurrence_color: Color::rgb(150, 176, 214),
            gutter_background: Color::rgb(246, 248, 252),
            background: Color::rgb(252, 253, 255),
            border_color: Color::rgb(188, 197, 211),
            chrome_background: Color::rgb(238, 242, 248),
            chrome_text: Color::rgb(70, 84, 106),
            chrome_dim_text: Color::rgb(140, 152, 170),
        }
    }

    /// The palette for a dark editor background.
    ///
    /// # Why this is a second palette and not a blend
    ///
    /// Syntax colours are not chrome: each one is the *identity* of a token category, the way
    /// the rising green is the identity of a candle. Blending the light set toward the ink would
    /// collapse the distinctions a reader scans by (a comment and a keyword would converge), so
    /// the hues are chosen at the tones they read at on a dark ground — the same set of
    /// categories, restated for the other appearance. The field is `pub`, so a host that wants
    /// its own scheme still replaces the whole value.
    ///
    /// Regression (BLUE21 P4-4): the constructor used `Self::default()` unconditionally, so a
    /// dark host got a near-white editor slab with dark ink inside its dark window.
    pub fn dark() -> Self {
        Self {
            colors: vec![
                (TokenKind::Plain, Color::rgb(212, 218, 228)),
                (TokenKind::Keyword, Color::rgb(197, 134, 192)),
                (TokenKind::Type, Color::rgb(86, 156, 214)),
                (TokenKind::Function, Color::rgb(220, 220, 170)),
                (TokenKind::String, Color::rgb(206, 145, 120)),
                (TokenKind::Character, Color::rgb(206, 145, 120)),
                (TokenKind::Number, Color::rgb(181, 206, 168)),
                (TokenKind::Comment, Color::rgb(106, 153, 85)),
                (TokenKind::BlockComment, Color::rgb(106, 153, 85)),
                (TokenKind::Preprocessor, Color::rgb(155, 179, 217)),
                (TokenKind::Operator, Color::rgb(212, 218, 228)),
                (TokenKind::Identifier, Color::rgb(212, 218, 228)),
            ],
            bracket_color: Color::rgb(86, 156, 214),
            active_line_color: Color::rgb(38, 40, 44),
            indent_guide_color: Color::rgb(52, 55, 60),
            selection_color: Color::rgb(38, 79, 120),
            search_color: Color::rgb(97, 79, 36),
            search_current_color: Color::rgb(140, 110, 40),
            occurrence_color: Color::rgb(86, 156, 214),
            gutter_background: Color::rgb(30, 32, 36),
            background: Color::rgb(24, 26, 30),
            border_color: Color::rgb(62, 66, 74),
            chrome_background: Color::rgb(32, 34, 38),
            chrome_text: Color::rgb(204, 210, 220),
            chrome_dim_text: Color::rgb(140, 148, 162),
        }
    }

    /// The palette that matches the active appearance, or the light one when no theme is active.
    ///
    /// Read through `crate::style`, not `crate::theme`: the theme module only exists in a build
    /// with a device profile, and the editor is compiled in every one.
    pub fn for_active_appearance() -> Self {
        let dark = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.appearance == crate::style::AppearanceMode::Dark)
            .unwrap_or(false);
        if dark {
            Self::dark()
        } else {
            Self::light()
        }
    }
}

impl SyntaxPalette {
    /// Returns the foreground for `kind`, falling back to `Plain`.
    pub fn color_for(&self, kind: TokenKind) -> Color {
        self.colors
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, color)| *color)
            .or_else(|| {
                self.colors
                    .iter()
                    .find(|(candidate, _)| *candidate == TokenKind::Plain)
                    .map(|(_, color)| *color)
            })
            .unwrap_or(Color::rgb(43, 55, 74))
    }

    /// Updates the colour of one token kind, inserting it when absent.
    pub fn set_color(&mut self, kind: TokenKind, color: Color) {
        match self.colors.iter_mut().find(|(candidate, _)| *candidate == kind) {
            Some(entry) => entry.1 = color,
            None => self.colors.push((kind, color)),
        }
    }
}

// ── Positions and cursor ─────────────────────────────────────────────────────

/// A caret location addressed in *character* columns, not bytes.
///
/// Character columns keep every movement/selection API UTF-8 agnostic: offsets
/// are converted to byte offsets only at the splice boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct TextPosition {
    /// Zero-based line index.
    pub line: usize,
    /// Zero-based character column.
    pub column: usize,
}

impl TextPosition {
    /// Creates a position.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Caret position plus the anchor used for range selection.
///
/// `head` is the movable end, `anchor` the fixed end; `head == anchor` means
/// "no selection".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Movable end of the selection.
    pub head: TextPosition,
    /// Fixed end of the selection.
    pub anchor: TextPosition,
}

impl Cursor {
    /// Returns `true` when no text is selected.
    pub fn is_collapsed(&self) -> bool {
        self.head == self.anchor
    }

    /// Returns the normalized `(start, end)` selection bounds.
    pub fn bounds(&self) -> (TextPosition, TextPosition) {
        if self.head < self.anchor {
            (self.head, self.anchor)
        } else {
            (self.anchor, self.head)
        }
    }

    /// Returns the number of lines the selection touches, inclusive.
    pub fn line_span(&self) -> usize {
        let (start, end) = self.bounds();
        end.line.saturating_sub(start.line) + 1
    }
}

/// Desired caret column preserved across vertical movement.
///
/// Without this, moving down through a short line and continuing would silently
/// destroy the caret's preferred column (Zed and VS Code both preserve it).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CursorGoal {
    pub(crate) active: bool,
    pub(crate) column: usize,
}

// ── Diagnostics ──────────────────────────────────────────────────────────────

/// Diagnostic marker severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarkerSeverity {
    /// Informational hint.
    Info,
    /// Warning.
    Warning,
    /// Hard error.
    Error,
}

impl MarkerSeverity {
    /// Returns the decoration colour for this severity.
    pub fn color(self) -> Color {
        match self {
            Self::Info => Color::rgb(74, 125, 201),
            Self::Warning => Color::rgb(216, 155, 52),
            Self::Error => Color::rgb(206, 82, 73),
        }
    }

    /// Returns the badge text used by inline diagnostics.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Returns a sort rank where errors come first.
    pub fn rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

/// One diagnostic marker bound to a line, optionally to a column range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticMarker {
    /// Zero-based line index.
    pub line: usize,
    /// Human-readable message.
    pub message: String,
    /// Severity shown in the gutter, status bar and inline list.
    pub severity: MarkerSeverity,
    /// Optional one-based start column.
    pub column: Option<usize>,
    /// Optional one-based end column, inclusive.
    pub end_column: Option<usize>,
}

impl DiagnosticMarker {
    /// Creates a whole-line marker.
    pub fn new(line: usize, message: impl Into<String>, severity: MarkerSeverity) -> Self {
        Self { line, message: message.into(), severity, column: None, end_column: None }
    }

    /// Attaches a one-based column range.
    pub fn with_range(mut self, column: usize, end_column: usize) -> Self {
        self.column = Some(column);
        self.end_column = Some(end_column);
        self
    }

    /// Returns the one-based start column, defaulting to 1.
    pub fn start_column(&self) -> usize {
        self.column.unwrap_or(1).max(1)
    }

    /// Returns the one-based end column, never before the start column.
    pub fn finish_column(&self) -> usize {
        self.end_column.unwrap_or_else(|| self.start_column()).max(self.start_column())
    }
}

// ── Buffers, folds, visual rows ──────────────────────────────────────────────

/// A multi-buffer editor tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorBuffer {
    /// Tab title shown in the tab strip.
    pub title: String,
    /// Full text contents.
    pub text: String,
    /// Language override for this buffer.
    pub language: Option<super::syntax::LanguageId>,
    /// Whether the buffer differs from its last save point.
    pub modified: bool,
}

impl EditorBuffer {
    /// Creates a buffer from a title and contents.
    pub fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        Self { title: title.into(), text: text.into(), language: None, modified: false }
    }
}

/// A foldable region of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldRegion {
    /// First line of the region, which stays visible while folded.
    pub start_line: usize,
    /// Last line of the region, hidden while folded.
    pub end_line: usize,
    /// `true` when currently collapsed.
    pub folded: bool,
}

impl FoldRegion {
    /// Creates a region in the given fold state.
    pub fn new(start_line: usize, end_line: usize, folded: bool) -> Self {
        Self { start_line, end_line: end_line.max(start_line), folded }
    }

    /// Returns the number of hidden lines contributed by this region.
    pub fn hidden_lines(&self) -> usize {
        if self.folded {
            self.end_line.saturating_sub(self.start_line)
        } else {
            0
        }
    }
}

/// One visual row mapping back to a document line and wrap segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualLine {
    /// Document line index.
    pub line: usize,
    /// Second-based wrap segment inside that line.
    pub segment: usize,
    /// Character offset where this visual row starts.
    pub start_column: usize,
    /// Character offset where this visual row ends, exclusive.
    pub end_column: usize,
}

/// One inline diagnostic row: `(visual_row, marker)`.
pub(crate) type InlineDiagnostic = (usize, DiagnosticMarker);

// ── Find & replace ───────────────────────────────────────────────────────────

/// Matching policy for find/replace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    /// Match case-sensitively.
    pub case_sensitive: bool,
    /// Match whole words only.
    pub whole_word: bool,
}

/// A search hit expressed as a line plus character range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Zero-based line index.
    pub line: usize,
    /// Zero-based start character column.
    pub start_column: usize,
    /// Zero-based end character column, exclusive.
    pub end_column: usize,
}

/// State of the in-editor find bar.
#[derive(Debug, Clone, Default)]
pub struct FindState {
    /// Whether the find bar is visible.
    pub visible: bool,
    /// Current query.
    pub query: String,
    /// Replacement text.
    pub replacement: String,
    /// Whether the replacement row is visible.
    pub replace_visible: bool,
    /// Matching policy.
    pub options: SearchOptions,
    /// All hits for `query`, recomputed on every mutation.
    pub matches: Vec<SearchMatch>,
    /// Index into `matches` of the active hit.
    pub current: usize,
    /// `true` when focus sits in the replacement field.
    pub focus_replace: bool,
}

impl FindState {
    /// Returns the number of hits.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Returns the one-based index of the active hit, or 0 when there are none.
    pub fn display_index(&self) -> usize {
        if self.matches.is_empty() {
            0
        } else {
            self.current.min(self.matches.len() - 1) + 1
        }
    }
}

// ── Completion ───────────────────────────────────────────────────────────────

/// One autocomplete candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// Text inserted when accepted.
    pub label: String,
    /// Optional trailing detail (type, signature).
    pub detail: Option<String>,
    /// Optional leading kind sigil (`f`, `t`, `#`).
    pub kind: Option<char>,
}

impl CompletionItem {
    /// Creates a plain candidate.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), detail: None, kind: None }
    }

    /// Adds a detail string.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Adds a kind sigil.
    pub fn with_kind(mut self, kind: char) -> Self {
        self.kind = Some(kind);
        self
    }
}

/// Completion provider interface — the plugin seam for LSP-backed completion.
pub trait CompletionSource {
    /// Returns candidate rows for `prefix` within `text`, capped at `limit`.
    fn completions(&self, text: &str, prefix: &str, limit: usize) -> Vec<CompletionItem>;
}

/// Default provider: distinct identifiers already present in the document.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentCompletions;

impl CompletionSource for DocumentCompletions {
    fn completions(&self, text: &str, prefix: &str, limit: usize) -> Vec<CompletionItem> {
        let limit = limit.max(1);
        if prefix.is_empty() {
            return Vec::new();
        }
        let mut seen: Vec<&str> = Vec::new();
        let mut cursor = 0usize;
        while cursor < text.len() {
            let ch = match text[cursor..].chars().next() {
                Some(ch) => ch,
                None => break,
            };
            if ch.is_alphanumeric() || ch == '_' {
                let start = cursor;
                let mut end = cursor + ch.len_utf8();
                while end < text.len() {
                    let next = text[end..].chars().next().unwrap_or(' ');
                    if next.is_alphanumeric() || next == '_' {
                        end += next.len_utf8();
                    } else {
                        break;
                    }
                }
                let word = &text[start..end];
                if word != prefix && word.starts_with(prefix) && !seen.contains(&word) {
                    seen.push(word);
                    if seen.len() >= limit {
                        break;
                    }
                }
                cursor = end;
            } else {
                cursor += ch.len_utf8();
            }
        }
        seen.into_iter().map(CompletionItem::new).collect()
    }
}

/// A transient completion popup.
#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    /// `true` while the popup is on screen.
    pub visible: bool,
    /// Candidate rows.
    pub items: Vec<CompletionItem>,
    /// Highlighted row index.
    pub selected: usize,
    /// Character column where the replaced prefix starts.
    pub anchor_column: usize,
}

impl CompletionState {
    /// Returns the highlighted candidate, if any.
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        if self.items.is_empty() {
            return None;
        }
        self.items.get(self.selected.min(self.items.len() - 1))
    }
}

// ── Context menu ─────────────────────────────────────────────────────────────

/// Single row of the editor context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// Row label.
    pub label: String,
    /// Stable identifier passed to `CodeEditor::activate_menu_item`.
    pub id: &'static str,
    /// Optional keyboard hint shown on the right.
    pub shortcut: Option<String>,
    /// Dimmed and non-activatable when `true`.
    pub disabled: bool,
}

impl MenuItem {
    /// Creates an actionable row.
    pub fn new(id: &'static str, label: impl Into<String>) -> Self {
        Self { label: label.into(), id, shortcut: None, disabled: false }
    }

    /// Adds a shortcut hint.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Marks the row disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Open context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    /// `true` while the menu is on screen.
    pub visible: bool,
    /// Top-left anchor in widget-local coordinates.
    pub position: Point,
    /// Menu rows.
    pub items: Vec<MenuItem>,
    /// Highlighted row index.
    pub selected: usize,
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self { visible: false, position: Point::new(0, 0), items: Vec::new(), selected: 0 }
    }
}

// ── Configuration ────────────────────────────────────────────────────────────

/// Editor configuration.
///
/// Build with [`CodeEditorConfig::new`] (or `default()`) and chain setters.
/// [`CodeEditorConfig::validate`] rejects impossible values so a
/// misconfiguration fails loudly instead of silently rendering wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeEditorConfig {
    /// Language used by the built-in highlighter.
    pub language: super::syntax::LanguageId,
    /// Spaces a `\t` expands to.
    pub tab_width: usize,
    /// `true` inserts spaces, `false` inserts a literal `\t`.
    pub insert_spaces: bool,
    /// `false` disables every mutation entry point.
    pub read_only: bool,
    /// Draw indent guide columns.
    pub show_indent_guides: bool,
    /// Highlight the bracket matching the one under the caret.
    pub highlight_brackets: bool,
    /// Render a minimap on the trailing edge.
    pub show_minimap: bool,
    /// Highlight the caret line background.
    pub highlight_active_line: bool,
    /// Soft-wrap lines wider than the text area.
    pub word_wrap: bool,
    /// Render gutter line numbers.
    pub show_line_numbers: bool,
    /// Outline every occurrence of the selected word.
    pub highlight_occurrences: bool,
    /// Auto-insert the closing bracket/quote when an opener is typed.
    pub auto_close_brackets: bool,
    /// Render a dot for every space and an arrow for every tab.
    pub show_whitespace: bool,
    /// Column at which a vertical ruler is drawn; 0 disables the ruler.
    pub column_ruler: usize,
    /// Font size in points.
    pub font_size: f32,
    /// Monospace family.
    pub font_family: String,
    /// Horizontal advance of one space, in logical pixels.
    pub space_advance: f32,
    /// Vertical advance of one line, in logical pixels.
    pub line_advance: f32,
}

impl Default for CodeEditorConfig {
    fn default() -> Self {
        Self {
            language: super::syntax::LanguageId::PlainText,
            tab_width: DEFAULT_TAB_WIDTH,
            insert_spaces: true,
            read_only: false,
            show_indent_guides: true,
            highlight_brackets: true,
            show_minimap: true,
            highlight_active_line: true,
            word_wrap: false,
            show_line_numbers: true,
            highlight_occurrences: true,
            auto_close_brackets: true,
            show_whitespace: false,
            column_ruler: 0,
            font_size: DEFAULT_FONT_SIZE,
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            space_advance: 7.0,
            line_advance: 16.0,
        }
    }
}

impl CodeEditorConfig {
    /// Creates the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the language and adopts its default tab width.
    pub fn language(mut self, language: super::syntax::LanguageId) -> Self {
        self.language = language;
        self.tab_width = language.default_tab_width();
        self
    }

    /// Sets the tab width in spaces.
    pub fn tab_width(mut self, tab_width: usize) -> Self {
        self.tab_width = tab_width;
        self
    }

    /// Selects spaces (`true`) or literal tabs (`false`) for indentation.
    pub fn insert_spaces(mut self, insert_spaces: bool) -> Self {
        self.insert_spaces = insert_spaces;
        self
    }

    /// Enables or disables editing.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Enables or disables indent guides.
    pub fn show_indent_guides(mut self, show: bool) -> Self {
        self.show_indent_guides = show;
        self
    }

    /// Enables or disables bracket matching.
    pub fn highlight_brackets(mut self, highlight: bool) -> Self {
        self.highlight_brackets = highlight;
        self
    }

    /// Enables or disables the minimap.
    pub fn show_minimap(mut self, show: bool) -> Self {
        self.show_minimap = show;
        self
    }

    /// Enables or disables active-line highlighting.
    pub fn highlight_active_line(mut self, highlight: bool) -> Self {
        self.highlight_active_line = highlight;
        self
    }

    /// Enables or disables soft wrapping.
    pub fn word_wrap(mut self, wrap: bool) -> Self {
        self.word_wrap = wrap;
        self
    }

    /// Enables or disables gutter line numbers.
    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Enables or disables occurrence highlighting.
    pub fn highlight_occurrences(mut self, highlight: bool) -> Self {
        self.highlight_occurrences = highlight;
        self
    }

    /// Enables or disables auto-closing bracket and quote insertion.
    pub fn auto_close_brackets(mut self, auto_close: bool) -> Self {
        self.auto_close_brackets = auto_close;
        self
    }

    /// Enables or disables whitespace rendering.
    pub fn show_whitespace(mut self, show: bool) -> Self {
        self.show_whitespace = show;
        self
    }

    /// Sets the ruler column (0 disables the ruler).
    pub fn column_ruler(mut self, column: usize) -> Self {
        self.column_ruler = column;
        self
    }

    /// Sets the font size in points.
    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    /// Sets the monospace family.
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = family.into();
        self
    }

    /// Sets the horizontal advance of one space.
    pub fn space_advance(mut self, advance: f32) -> Self {
        self.space_advance = advance;
        self
    }

    /// Sets the vertical advance of one line.
    pub fn line_advance(mut self, advance: f32) -> Self {
        self.line_advance = advance;
        self
    }

    /// Validates the configuration, returning the first violated constraint.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.tab_width == 0 || self.tab_width > 16 {
            return Err("tab_width must be in 1..=16");
        }
        if !self.font_size.is_finite() || self.font_size <= 0.0 {
            return Err("font_size must be a positive finite number");
        }
        if self.font_family.trim().is_empty() {
            return Err("font_family must not be empty");
        }
        if !self.space_advance.is_finite() || self.space_advance <= 0.0 {
            return Err("space_advance must be positive");
        }
        if !self.line_advance.is_finite() || self.line_advance < self.font_size {
            return Err("line_advance must be >= font_size");
        }
        if self.column_ruler > 10_000 {
            return Err("column_ruler must be at most 10000");
        }
        Ok(())
    }
}
