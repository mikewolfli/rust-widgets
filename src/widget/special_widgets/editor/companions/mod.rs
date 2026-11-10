// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Companion controls shared by the editor widgets.
//!
//! A companion is a control the editor family reuses but that is not part of a
//! document model — [`FindBar`](self::FindBar) is the first. It lives here rather than in
//! `markdown_editor` so the Markdown widget no longer owns a control that the
//! code editor also needs (BLUE31 §1.9.4).

mod find_bar;

/// The Markdown editor's model state; only that widget consumes it, so the
/// re-export carries its feature (BLUE31 §1.5).
#[cfg(feature = "markdown-editor")]
pub(crate) use find_bar::{literal_matches, FindState};
pub use find_bar::{FindBar, ReplaceRequest};
