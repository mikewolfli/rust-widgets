// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{CommandDescription, CommandId, UndoCommand};
use crate::compat::{atomic::AtomicU64, atomic::Ordering, MiniToString, Rc, RefCell, String};

static NEXT_TEXT_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

/// Snapshot command shared by text-editing widgets.
pub(crate) struct TextSnapshotCommand {
    id: CommandId,
    target: Rc<RefCell<String>>,
    before: String,
    after: String,
    command_type: &'static str,
}

impl TextSnapshotCommand {
    /// Creates a command for a mutation that has already been applied.
    pub(crate) fn new(
        target: Rc<RefCell<String>>,
        before: String,
        after: String,
        command_type: &'static str,
    ) -> Self {
        Self {
            id: CommandId(NEXT_TEXT_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
            command_type,
        }
    }
}

impl UndoCommand for TextSnapshotCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit text".to_string(),
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
