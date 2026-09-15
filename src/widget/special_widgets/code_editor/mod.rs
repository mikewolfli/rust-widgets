// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CodeEditor widget — a self-contained, Zed-class source code editor model.
//!
//! # Scope boundary
//!
//! This widget deliberately implements the *whole editor surface that a single
//! widget can own*: text storage on a line index, a cursor with anchor/selection
//! (including multiple carets), an undo/redo history, viewport scrolling (line
//! and column axes), soft line wrapping, folding, multi-buffer tabs, bracket
//! matching, indent guides, in-editor find/replace with a visible find bar,
//! autocomplete, context menus, auto-pairing, line commands, configurable
//! tabs/spaces, whitespace/ruler overlays, and the input plumbing (keyboard,
//! IME, mouse, touch, wheel) for all of it.
//!
//! Everything that a real editor product would load as a *plugin crate* is
//! intentionally **not** in here, because it cannot be owned by one widget:
//!
//! | Concern | Where it belongs |
//! |---------|------------------|
//! | Tree-sitter grammar / real incremental parsing | `plugin: syntax` crate |
//! | Syntax highlight themes & semantic tokens | `plugin: theme` crate |
//! | LSP client (completion, hover, diagnostics, goto) | `plugin: lsp` crate |
//! | DAP / terminal / language REPLs | `plugin: debug` crate |
//! | Multiplayer (CRDT), collaboration | `plugin: collab` crate |
//! | File-system, workspace, VCS integration | host application |
//!
//! The widget therefore exposes *extension hooks* rather than pretending to be
//! those systems: [`SyntaxHighlighter`] (pluggable tokenizer, defaulting to a
//! keyword/string/comment/`fn`-name lexer), [`CompletionSource`] (pluggable
//! completion-provider, defaulting to document identifiers), and
//! [`DiagnosticMarker`] (host/LSP-pushed inline diagnostics).
//!
//! # Module layout
//!
//! The implementation is split into focused submodules; this file only declares
//! them and re-exports their public interface (principle.md #8). The submodules
//! are private, so their names are written as code spans rather than intra-doc
//! links (a link to a private item is a rustdoc error).
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | `types` | Positions, cursor, markers, tokens, palette, config, find/completion/menu state |
//! | `syntax` | Built-in lexer, [`LanguageId`], [`SyntaxHighlighter`] plugin seam |
//! | `buffer` | Line-indexed document model, tabs, folds, splice primitive |
//! | `multicursor` | Multiple carets: add/merge, occurrence selection, edit projection |
//! | `pairs` | Auto-pairing, bracket/quote skip-over and smart backspace |
//! | `editor` | The [`CodeEditor`] widget: state, commands, history |
//! | `input` | Keyboard/IME/pointer/wheel translation layer |
//! | `render` | Geometry metrics and the `Draw` implementation |
//! | `tests` | Behavioural test-suite |
//!
//! # Rust design notes (principle.md #28..#34)
//!
//! * No trait objects on the hot path: highlighting is a plain `enum` dispatch
//!   plus a tiny [`SyntaxHighlighter`] trait for the pluggable case.
//! * All state is owned; no manual allocation, no `unsafe`.
//! * Configuration is a Builder-style type ([`CodeEditorConfig`]) rather than a
//!   varargs setter cluster.
//! * Invalid configuration is rejected through `Result`, never silently ignored.
//!
//! # Device profiles
//!
//! Touch targets (find-bar buttons, completion rows, context-menu rows,
//! scrollbar hit area) are all >= 28 logical pixels so tablet/mobile remain
//! usable; triple-tap selects a line for touch devices while triple-click keeps
//! the desktop behaviour.

mod buffer;
mod editor;
mod input;
mod multicursor;
mod pairs;
mod render;
mod syntax;
mod types;

#[cfg(test)]
mod tests;

pub use editor::CodeEditor;
pub use multicursor::MultiCursor;
pub use syntax::{BuiltinHighlighter, LanguageId, SyntaxHighlighter};
pub use types::{
    CodeEditorConfig, CompletionItem, CompletionSource, CompletionState, ContextMenuState, Cursor,
    DiagnosticMarker, DocumentCompletions, EditorBuffer, FindState, FoldRegion, MarkerSeverity,
    MenuItem, SearchMatch, SearchOptions, SyntaxPalette, TextPosition, TokenKind, TokenSpan,
    VisualLine, DEFAULT_FONT_FAMILY, DEFAULT_FONT_SIZE, DEFAULT_TAB_WIDTH, MAX_COMPLETIONS,
    MIN_TOUCH_TARGET,
};
