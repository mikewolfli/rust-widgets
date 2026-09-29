// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Range-scoped undo command for the code editor.
//!
//! A keystroke changes a few characters. The shared
//! [`crate::undo::TextSnapshotCommand`] records that by storing **two complete
//! copies of the document** — the state before and the state after — which at a
//! million lines is tens of megabytes allocated per key press and milliseconds
//! of copying.
//!
//! This command stores the edit instead: the byte range it replaced and the text
//! it put there. Undo re-applies the inverse splice. Two consequences matter:
//!
//! * memory per checkpoint is proportional to the edit, not the file;
//! * the cost does not grow as the document does, so a large file keeps typing
//!   smooth and its history stays affordable.
//!
//! The command still owns an `Rc<RefCell<String>>` target, matching the shared
//! command's contract, so the code editor's history stack keeps working
//! unchanged.

use crate::compat::{atomic::AtomicU64, atomic::Ordering, Rc, RefCell, String};
use crate::undo::{CommandDescription, CommandId, UndoCommand};

static NEXT_RANGE_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

/// One undoable replacement of a byte range within a document.
#[derive(Debug)]
pub(crate) struct TextRangeCommand {
    id: CommandId,
    /// The shared document buffer, same handle the snapshot command uses.
    target: Rc<RefCell<String>>,
    /// Byte offset the replacement started at.
    start: usize,
    /// Text that was at `[start, start + removed_len)` before the edit.
    removed: String,
    /// Text the edit wrote in its place.
    inserted: String,
    command_type: &'static str,
}

impl TextRangeCommand {
    /// Records a replacement of `[start, start + removed.len())` by `inserted`.
    ///
    /// Both offsets are byte offsets into the document as it was **before** the
    /// edit, which is what `undo` needs to splice the original text back in.
    pub(crate) fn new(
        target: Rc<RefCell<String>>,
        start: usize,
        removed: String,
        inserted: String,
        command_type: &'static str,
    ) -> Self {
        Self {
            id: CommandId(NEXT_RANGE_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            start,
            removed,
            inserted,
            command_type,
        }
    }

    /// Returns the byte length of the text this command inserted.
    ///
    /// Exposed for tests that assert a checkpoint stores the edit rather than the
    /// whole document; there is no production caller.
    #[cfg(test)]
    pub(crate) fn inserted_len(&self) -> usize {
        self.inserted.len()
    }

    /// Replaces `[start, start + from_len)` with `replacement` in the document.
    ///
    /// Returns `false` when the range does not fit the current document, which
    /// means the history is out of step with the buffer. Reporting that instead
    /// of panicking lets the caller drop the checkpoint rather than corrupt the
    /// document.
    fn splice(&self, start: usize, from_len: usize, replacement: &str) -> bool {
        let mut text = self.target.borrow_mut();
        let end = start.saturating_add(from_len);
        if end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return false;
        }
        text.replace_range(start..end, replacement);
        true
    }
}

impl UndoCommand for TextRangeCommand {
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
        if self.splice(self.start, self.removed.len(), &self.inserted) {
            Ok(())
        } else {
            Err("text range command is out of step with the document".to_string())
        }
    }

    fn undo(&mut self) -> Result<(), String> {
        if self.splice(self.start, self.inserted.len(), &self.removed) {
            Ok(())
        } else {
            Err("text range command is out of step with the document".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::UndoCommand;

    /// Builds a command over a shared document.
    fn command(
        text: &str,
        start: usize,
        removed: &str,
        inserted: &str,
    ) -> (TextRangeCommand, Rc<RefCell<String>>) {
        let target = Rc::new(RefCell::new(text.to_string()));
        let command =
            TextRangeCommand::new(target.clone(), start, removed.to_string(), inserted.to_string(), "test");
        (command, target)
    }

    #[test]
    fn undo_restores_exactly_the_replaced_range() {
        let (mut command, target) = command("hello world", 6, "world", "there");
        *target.borrow_mut() = "hello there".to_string();

        assert!(command.undo().is_ok());
        assert_eq!(*target.borrow(), "hello world", "undo must reinstate the original text");

        assert!(command.execute().is_ok());
        assert_eq!(*target.borrow(), "hello there", "redo must reapply the edit");
    }

    #[test]
    fn a_pure_insertion_removes_nothing_on_undo() {
        let (mut command, target) = command("ac", 1, "", "b");
        *target.borrow_mut() = "abc".to_string();

        assert!(command.undo().is_ok());
        assert_eq!(*target.borrow(), "ac");
    }

    #[test]
    fn a_pure_deletion_inserts_nothing_on_redo() {
        let (mut command, target) = command("abc", 1, "b", "");
        *target.borrow_mut() = "ac".to_string();

        assert!(command.undo().is_ok());
        assert_eq!(*target.borrow(), "abc");
        assert!(command.execute().is_ok());
        assert_eq!(*target.borrow(), "ac");
    }

    #[test]
    fn a_multibyte_edit_round_trips() {
        let (mut command, target) = command("héllo", 1, "é", "e");
        *target.borrow_mut() = "hello".to_string();

        assert!(command.undo().is_ok());
        assert_eq!(*target.borrow(), "héllo");
    }

    /// A checkpoint stores the edit, not the document.
    ///
    /// This is the property that keeps typing affordable in a large file: the
    /// shared snapshot command stores two full copies per keystroke.
    #[test]
    fn the_checkpoint_size_follows_the_edit_not_the_document() {
        let big = "x".repeat(1_000_000);
        let (command, _target) = command(&big, 500_000, "x", "y");
        assert_eq!(
            command.inserted_len(),
            1,
            "a one-character edit must record one character, not the file"
        );
    }

    /// An out-of-step range reports an error instead of corrupting the document.
    #[test]
    fn a_range_that_no_longer_fits_is_rejected() {
        let (mut command, target) = command("short", 2, "ort", "X");
        // The document shrank beneath the recorded range.
        *target.borrow_mut() = "s".to_string();
        assert!(command.undo().is_err(), "undo must refuse rather than panic");
        assert_eq!(*target.borrow(), "s", "the document is left untouched");
    }

    /// A range that would split a character is rejected, not panicked on.
    #[test]
    fn a_range_inside_a_character_is_rejected() {
        let (mut command, target) = command("é", 1, "", "x");
        *target.borrow_mut() = "é".to_string();
        assert!(
            command.execute().is_err(),
            "offset 1 is inside `é`, so the edit cannot be applied"
        );
    }
}
