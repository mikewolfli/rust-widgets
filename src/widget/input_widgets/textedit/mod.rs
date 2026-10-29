// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Multi-line text edit widget.
//!
//! `TextEdit` is split by seam so each file answers one question:
//!
//! * `model` — the value, the caret, the selection, the undo history, the wrap table and the
//!   find/replace binding;
//! * `composition` — the IME preedit state, held apart from the value;
//! * `paint` — the memoised format/marker tables, the scrollbar and `impl Draw`;
//! * `events` — `impl Widget`, `impl DragSource`, `impl WidgetProperties` and
//!   `impl EventHandler`.
//!
//! The type itself is defined once, in `model`, and every other file adds an `impl` block for
//! it. The published surface is re-exported here, so `input_widgets::textedit::TextEdit` and
//! `input_widgets::textedit::CharFormat` keep naming the same items.

mod composition;
mod events;
mod model;
mod paint;

#[cfg(test)]
mod tests;

pub use self::model::{CharFormat, RowSpan, TextEdit, TextSpan};
