// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A generic "snapshot of one value" undo command.
//!
//! # Why this exists (WDGT-10)
//!
//! `date_edit`, `date_time_edit`, `time_edit`, `key_sequence_edit`, `tag_input` and
//! `shortcut_editor` each carried a private command type with the same fields and the same
//! `execute`/`undo` bodies: take an `Rc<RefCell<T>>`, a before value and an after value, and write
//! one or the other back. The only differences were the payload type and two description strings.
//!
//! One generic command replaces all six. The widgets still own their undo/redo *methods* (they
//! have different signals and caret/selection side effects), but the command plumbing — the part
//! that was copied — has one implementation.

use super::{CommandDescription, CommandId, UndoCommand};
use crate::compat::{atomic::AtomicU64, atomic::Ordering, Rc, RefCell, String, ToString};

static NEXT_VALUE_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

/// An already-applied mutation that can be reversed by restoring `before`.
pub(crate) struct ValueCommand<T> {
    id: CommandId,
    target: Rc<RefCell<T>>,
    before: T,
    after: T,
    text: &'static str,
    command_type: &'static str,
}

impl<T: Clone> ValueCommand<T> {
    /// Creates a command for a mutation that has already been applied.
    pub(crate) fn new(
        target: Rc<RefCell<T>>,
        before: T,
        after: T,
        text: &'static str,
        command_type: &'static str,
    ) -> Self {
        Self {
            id: CommandId(NEXT_VALUE_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
            text,
            command_type,
        }
    }
}

impl<T: Clone> UndoCommand for ValueCommand<T> {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: self.text.to_string(),
            timestamp_ms: 0,
            command_type: self.command_type,
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.after.clone();
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.before.clone();
        Ok(())
    }
}
