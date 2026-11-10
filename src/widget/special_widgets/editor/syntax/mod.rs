// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tokenizer seam and language vocabulary shared by every editor surface.
//!
//! Real grammar-aware highlighting (tree-sitter, LSP semantic tokens) belongs in
//! a plugin crate. What lives here is the [`SyntaxHighlighter`] trait every
//! host-facing seam is written against, the language/token vocabulary it speaks,
//! and — behind the `builtin-languages` feature — a dependency-free lexer good
//! enough to power keyword/string/comment/number colouring.
//!
//! The module is compiled for `full_widgets` regardless of which editor widget a
//! build selects, because `crates/rust-widgets-suites` uses the vocabulary on its
//! own (BLUE31 §1.5). `BuiltinHighlighter` is the one part whose *implementation*
//! depends on `builtin-languages`: with the feature off it is a stub that reports
//! the language and produces no tokens, which is the honest answer for a build
//! that did not compile the lexer.
//!
//! | Item | Sibling |
//! |------|---------|
//! | `TokenKind`, `TokenSpan` | `token` |
//! | `LineState`, `StructuralKind`, `SyntaxHighlighter` | `highlighter` |
//! | `LanguageId` | `language` |
//! | `BuiltinHighlighter` | `builtin` |

pub use super::text::{TextPosition, DEFAULT_TAB_WIDTH};

mod builtin;
mod highlighter;
mod language;
mod token;

pub use builtin::BuiltinHighlighter;
pub use highlighter::{LineState, StructuralKind, SyntaxHighlighter};
pub use language::LanguageId;
#[cfg(any(feature = "code-editor", feature = "builtin-languages"))]
pub(crate) use token::merge_adjacent;
pub use token::{TokenKind, TokenSpan};

#[cfg(all(test, feature = "builtin-languages"))]
mod tests;
