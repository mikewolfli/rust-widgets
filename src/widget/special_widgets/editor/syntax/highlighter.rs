// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The single-line lexer seam and the cross-line state it carries.

use super::{TextPosition, TokenSpan};
use alloc::vec::Vec;
/// Lexer state carried across a line boundary.
///
/// Real syntax highlighting is **stateful**: a block comment, a multi-line
/// string (`"""`, `r#"…"#`) or a nested-delimiter run opened on one line
/// changes how the next line must be read. A highlighter that only ever sees
/// one line cannot know whether it starts inside a string, which is why the
/// seam has to carry this value in and out of every call.
///
/// The type is intentionally a small `enum` rather than an opaque token: it has
/// to be `Copy` so the line-state cache can store one per line, and it has to be
/// comparable so an edit can stop re-lexing once the state it produces matches
/// the state that was already recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum LineState {
    /// The line starts outside any construct; the next line does too.
    #[default]
    Clean,
    /// Inside a `/* … */` block comment.
    BlockComment,
    /// Inside a **nested** block comment (Rust, Swift), carrying the number of
    /// still-open comment levels at the start of the line. `BlockComment` is the
    /// one-level case; this variant is what lets `/* /* */ */` close at the right
    /// `*/` instead of ending at the inner one.
    NestedBlockComment {
        /// Open comment levels at the start of the line (never less than 2).
        depth: u32,
    },
    /// Inside a double-quoted multi-line string (`"""` for Python, `"` for
    /// a line-continued string in the C family).
    MultiLineString,
    /// Inside a single-quoted multi-line string (`'''` for Python).
    MultiLineCharString,
    /// Inside a Rust raw string, carrying the number of `#` delimiters.
    RawString {
        /// How many `#` characters delimit the raw string (`r#"` → 1).
        hashes: u8,
    },
    /// Inside a Go backtick raw string.
    RawStringTick,
    /// Inside an HTML/XML `<!-- … -->` comment.
    HtmlComment,
}

impl LineState {
    /// Returns the state a freshly opened document starts in.
    pub fn initial() -> Self {
        Self::Clean
    }

    /// Returns `true` when this state is [`Self::Clean`].
    pub fn is_clean(self) -> bool {
        matches!(self, Self::Clean)
    }

    /// Returns `true` when the state does not end at a line boundary, i.e. the
    /// next line is a continuation. Used by the renderer to keep a continuation
    /// row visually distinct from a fresh one.
    pub fn continues(self) -> bool {
        !self.is_clean()
    }
}

/// A structural unit a selection can expand to.
///
/// This is the vocabulary shared by the editor's `expand_selection` command and
/// the optional [`SyntaxHighlighter::selection_ranges`] hook. A grammar-aware
/// highlighter can answer with precise ranges; the editor's textual fallback is
/// used when it returns nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructuralKind {
    /// The identifier/word under the caret.
    Word,
    /// The whole line, including its terminator when there is one.
    Line,
    /// The innermost enclosing bracket pair (`()`, `[]` or `{}`).
    Bracket,
    /// The innermost enclosing brace block, or the paragraph around the caret.
    Block,
}

/// Single-line lexer interface.
///
/// Implement this to plug a real grammar into the editor:
///
/// ```
/// use rust_widgets::widget::special_widgets::editor::syntax::{
///     LineState, SyntaxHighlighter, TokenKind, TokenSpan,
/// };
///
/// struct EverythingIsAComment;
///
/// impl SyntaxHighlighter for EverythingIsAComment {
///     fn highlight_line(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
///         let spans = if line.is_empty() {
///             Vec::new()
///         } else {
///             vec![TokenSpan::new(0, line.len(), TokenKind::Comment)]
///         };
///         (spans, state)
///     }
///
///     fn language_name(&self) -> &str {
///         "Comment Only"
///     }
/// }
/// ```
///
/// Implementors must be `Send + Sync` so a host may run the tokenizer on a
/// worker thread and hand the finished spans back to the UI thread. Without
/// that bound the editor can only ever re-highlight synchronously inside
/// `draw`, which is what makes a long file stutter no matter how fast the
/// lexer itself is.
pub trait SyntaxHighlighter: Send + Sync {
    /// Splits `line` into ordered, non-overlapping [`TokenSpan`]s, given the
    /// state the line starts in, and returns the state the line ends in.
    ///
    /// Spans that fall outside the line or split a UTF-8 sequence are clamped
    /// by the editor, so implementations cannot corrupt rendering by returning
    /// malformed offsets.
    fn highlight_line(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState);

    /// Human-readable language name used by the status bar and plugin registry.
    fn language_name(&self) -> &str {
        "Plain Text"
    }

    /// Returns the structural ranges a grammar recognises around `position`.
    ///
    /// The list must be ordered from the innermost range outwards, so the editor
    /// can implement "expand once more" by taking the first range strictly larger
    /// than the current selection. The default returns no ranges, which is the
    /// honest answer for a tokenizer that has no structural model; the editor
    /// then falls back to its own textual scan.
    ///
    /// `text` is the whole document and `position` is a character-addressed
    /// location in it, matching every other position in this widget.
    fn selection_ranges(
        &self,
        text: &str,
        position: TextPosition,
        kind: StructuralKind,
    ) -> Vec<(TextPosition, TextPosition)> {
        let _ = (text, position, kind);
        Vec::new()
    }

    /// Returns `true` when `line` starts its own construct and the previous
    /// line's state can be ignored for correctness (not merely for speed).
    ///
    /// The default is the conservative `false`: a stateful highlighter always
    /// consumes the incoming state. Stateless highlighters may override this
    /// with `true` so the editor can skip re-lexing a region after an edit.
    fn resets_each_line(&self) -> bool {
        false
    }
}
