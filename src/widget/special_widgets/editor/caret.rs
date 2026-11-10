// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! How a caret is painted, shared by every editor surface.
/// How the insertion point is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaretStyle {
    /// A thin vertical bar, the default in most editors.
    #[default]
    Line,
    /// A filled block covering the character cell.
    Block,
    /// A horizontal bar under the character cell.
    Underline,
}
