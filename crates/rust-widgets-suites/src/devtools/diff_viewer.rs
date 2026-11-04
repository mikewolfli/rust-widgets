// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The diff viewer family: the viewer itself, its compared-row model and the file selector
//! beside it.
//!
//! The control's row model lives in [`model`], the viewer in [`viewer`], and the file selector
//! beside it in [`file_list`]. This file only declares and re-exports them, so the historical
//! `diff_viewer::DiffViewer` / `diff_viewer::DiffFileList` paths keep resolving.

mod file_list;
mod model;
mod viewer;

#[cfg(test)]
mod tests;

pub use file_list::{DiffFileList, FileDiffEntry, FileDiffStatus};
pub use model::{DiffKind, DiffLine, DiffViewMode, WhitespaceMode};
pub use viewer::DiffViewer;
