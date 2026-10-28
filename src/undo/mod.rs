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
// `ValueCommand` is the shared plumbing behind the six value-editing inputs, and those inputs are
// compiled whenever the widget set is unstripped (`widgets_unstripped`) — not only on a device
// profile. Gating the provider on `full_widgets` was one term narrower than its consumers, so an
// unstripped build with no device profile (the `desktop-runtime,controls-custom` set the
// cross-target gates use) failed with `unresolved import crate::undo::ValueCommand`.
#[cfg(widgets_unstripped)]
mod value_command;

pub use command::*;
pub use stack::*;
pub use types::*;
#[cfg(widgets_unstripped)]
pub(crate) use value_command::ValueCommand;
