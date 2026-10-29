// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TextArea widget — multi-line text input (BLUE13 R2.5).
//!
//! The control is one type split by responsibility, not by line count:
//!
//! | Member | What it holds |
//! |---|---|
//! | `model` | The document: the text buffer, the caret and its selection anchor, the line table, the undo step and the find/replace wiring. |
//! | `paint` | The viewport: the display-row table (soft wrap included), the geometry a caret or a click resolves against, and the `Draw` implementation. |
//! | `events` | The runtime contracts: the pointer gesture state machine, `Widget`, `WidgetProperties`, `EventHandler` and drag/drop. |
//!
//! # Reachability
//!
//! [`TextArea`] is re-exported here, so every published path
//! (`input_widgets::textarea::TextArea`, and the parent module's own re-export) resolves exactly as
//! it did before the single file became a folder.

mod events;
mod model;
mod paint;
#[cfg(test)]
mod tests;

pub use self::model::TextArea;
