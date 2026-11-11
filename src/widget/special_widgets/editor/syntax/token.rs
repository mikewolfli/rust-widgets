// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Token categories and spans.

#[cfg(any(feature = "code-editor", feature = "builtin-languages"))]
use alloc::vec::Vec;
// ── Token styling ────────────────────────────────────────────────────────────

/// Token categories produced by a `SyntaxHighlighter`.
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

/// Coalesces neighbouring spans that share a token kind and are contiguous.
///
/// Shared by the built-in lexer and the editor's span sanitiser: a third-party
/// highlighter's output and the built-in one are both normalised by this one
/// function, so the renderer sees a single span grammar.
///
/// Both consumers carry their own feature gate (`code-editor` for the sanitiser,
/// `builtin-languages` for the lexer), so a Markdown-only build with neither does
/// not compile it at all.
#[cfg(any(feature = "code-editor", feature = "builtin-languages"))]
pub(crate) fn merge_adjacent(spans: Vec<TokenSpan>) -> Vec<TokenSpan> {
    let mut merged: Vec<TokenSpan> = Vec::with_capacity(spans.len());
    for span in spans {
        if span.is_empty() {
            continue;
        }
        match merged.last_mut() {
            Some(last) if last.kind == span.kind && last.end == span.start => last.end = span.end,
            _ => merged.push(span),
        }
    }
    merged
}
