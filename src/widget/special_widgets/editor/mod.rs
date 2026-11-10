// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The shared editor layer (BLUE31 §1.3–1.5).
//!
//! This is **not** a third public editor control. It is the lower layer both
//! [`CodeEditor`](crate::widget::special_widgets::code_editor::CodeEditor) and
//! [`MarkdownEditor`](crate::widget::special_widgets::markdown_editor::MarkdownEditor)
//! build on, and the place a host finds the types it shares with them:
//!
//! | Module | Contents | Gate |
//! |--------|----------|------|
//! | [`syntax`] | `LanguageId`, `LineState`, `TokenKind`, `TokenSpan`, `SyntaxHighlighter`, `BuiltinHighlighter` | always (under `full_widgets`) |
//! | [`caret`] | `CaretStyle` | always |
//! | [`companions`] | `FindBar` | either editor feature |
//!
//! `text` is private: it carries [`TextPosition`], which is reachable through
//! [`syntax`] because it appears in `SyntaxHighlighter`'s public signature.
//!
//! # Feature independence
//!
//! Nothing here depends on `code_editor` or `markdown_editor`, so a build that
//! enables only one of the two compiles this layer once and neither widget drags
//! the other in (BLUE31 §1.8).

pub mod caret;
pub mod syntax;
mod text;

pub use caret::CaretStyle;

/// Companion controls that belong to the editor family, not to one widget.
#[cfg(any(feature = "code-editor", feature = "markdown-editor"))]
pub mod companions;
