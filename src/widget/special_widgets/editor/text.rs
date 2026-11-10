// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Character-addressed text primitives shared by the editor layer.
//!
//! These types carry no editor state and no rendering: they are the vocabulary
//! every editor implementation (the code editor, the Markdown editor and the
//! host-facing plugin seam) addresses text with. Keeping them here — rather than
//! in either widget — is what lets `markdown_editor` compile without
//! `code_editor` (BLUE31 §1.8).

/// Default tab width measured in spaces.
pub const DEFAULT_TAB_WIDTH: usize = 4;
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
