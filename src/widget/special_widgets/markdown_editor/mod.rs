// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MarkdownEditor widget.

// The formatting strip is a companion control, declared here so the editor's action surface and
// its pointer surface live in one module tree (COMP-H04).
#[path = "../markdown_toolbar.rs"]
pub mod markdown_toolbar;
pub use markdown_toolbar::{MarkdownToolbar, ToolbarIconSide, MARKDOWN_TOOLBAR_ACTIONS};

mod block_renderer;
mod document;
mod editor;
mod events;
mod outline;
mod paint;
mod preview;

pub use block_renderer::{MarkdownBlockRender, MarkdownBlockRenderer, MarkdownBlockSpec};
pub use document::{MarkdownOutlineEntry, MarkdownTextChange};
pub use editor::MarkdownEditor;
pub use outline::MarkdownOutlinePanel;

pub(crate) use crate::widget::special_widgets::editor::companions::{literal_matches, FindState};
/// The find bar is a member of the editor family, not of this widget: it lives in
/// `editor::companions` and is re-exported here so every historical
/// `markdown_editor::FindBar` path keeps working (BLUE31 §1.9.4).
pub use crate::widget::special_widgets::editor::companions::{FindBar, ReplaceRequest};

pub(crate) use block_renderer::*;
pub(crate) use document::*;
pub(crate) use preview::*;

#[cfg(test)]
mod block_renderer_tests;
#[cfg(test)]
mod issue_20261026_tests;
#[cfg(test)]
mod issue_2_8_tests;
#[cfg(test)]
mod tests;
