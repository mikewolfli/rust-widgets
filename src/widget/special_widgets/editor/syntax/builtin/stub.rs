// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Honest fallback for builds without `builtin-languages`.
//!
//! The lexer's keyword and type tables are substantial, so a build that did not
//! ask for them compiles this stub instead. It reports the language it was
//! constructed with — the status bar and the plugin registry need an honest name
//! — and produces no tokens, which is exactly "no syntax highlighting is
//! compiled in" rather than a silently wrong colouring.

use super::super::{LanguageId, LineState, SyntaxHighlighter, TokenSpan};
use alloc::vec::Vec;

/// A highlight-less placeholder for the built-in lexer.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuiltinHighlighter {
    language: LanguageId,
}

impl BuiltinHighlighter {
    /// Creates a highlighter for `language`.
    pub fn new(language: LanguageId) -> Self {
        Self { language }
    }

    /// Returns the language this highlighter tokenizes.
    pub fn language(&self) -> LanguageId {
        self.language
    }
}

impl SyntaxHighlighter for BuiltinHighlighter {
    fn highlight_line(&self, _line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
        (Vec::new(), state)
    }

    fn language_name(&self) -> &str {
        self.language.name()
    }
}
