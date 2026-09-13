// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Undo/Redo framework — generic undo stack supporting commands, grouping,
//! and cross-widget undo/redo operations.

mod command;
mod stack;
mod text_command;
mod types;

pub use command::*;
pub use stack::*;
pub(crate) use text_command::TextSnapshotCommand;
pub use types::*;
