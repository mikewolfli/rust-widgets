// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MarkdownEditor widget.

// The formatting strip is a companion control, declared here so the editor's action surface and
// its pointer surface live in one module tree (COMP-H04).
#[path = "../markdown_toolbar.rs"]
pub mod markdown_toolbar;
pub use markdown_toolbar::{MarkdownToolbar, ToolbarIconSide, MARKDOWN_TOOLBAR_ACTIONS};

mod document;
mod editor;
mod events;
mod find;
mod outline;
mod paint;
mod preview;

pub use document::{MarkdownOutlineEntry, MarkdownTextChange};
pub use editor::MarkdownEditor;
pub use find::{FindBar, ReplaceRequest};
pub use outline::MarkdownOutlinePanel;

pub(crate) use document::*;
pub(crate) use find::*;
pub(crate) use preview::*;

#[cfg(test)]
mod issue_20261026_tests;
#[cfg(test)]
mod issue_2_8_tests;
#[cfg(test)]
mod tests;
