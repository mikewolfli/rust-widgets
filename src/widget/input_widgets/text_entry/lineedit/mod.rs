// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Single-line text edit widget.
//!
//! [`LineEdit`] is split by seam so each file answers one question:
//!
//! * `model` — the value, the caret, the selection, the undo history and every editing operation
//!   that changes them;
//! * `paint` — the field band, the memoised measurement tables, the caret rectangle and
//!   `impl Draw`;
//! * `events` — `impl Widget`, `impl EventHandler`, the key router and the drag/drop traits;
//! * `properties` — the validator plumbing, the decoration setters and `impl WidgetProperties`.
//!
//! The type itself is defined once, in `model`, and every other file adds an `impl` block for it. The published surface is re-exported here, so `input_widgets::lineedit::LineEdit` and
//! `input_widgets::lineedit::EchoMode` keep naming the same items.

mod events;
mod model;
mod paint;
mod properties;

#[cfg(test)]
mod tests;

pub use self::model::{LineEdit, LineEditAction};
pub use crate::platform::EchoMode;
pub use crate::widget::input_widgets::input_validator::{
    InputValidator, ValidationState, Validator,
};
