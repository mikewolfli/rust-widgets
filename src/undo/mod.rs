// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Undo/Redo framework — generic undo stack supporting commands, grouping,
//! and cross-widget undo/redo operations.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/input_widgets/rich_edit.rs:1` (text edits push onto the undo stack); 18 files reference it.

mod command;
mod stack;
mod types;
mod value_command;

pub use command::*;
pub use stack::*;
pub use types::*;
pub(crate) use value_command::ValueCommand;
