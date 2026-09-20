// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::command::UndoCommand;
use super::types::*;
use crate::compat::{format, Box, String, Vec};

/// Undo/Redo stack with configurable capacity.
pub struct UndoStack {
    undo_stack: Vec<Box<dyn UndoCommand>>,
    redo_stack: Vec<Box<dyn UndoCommand>>,
    max_capacity: usize,
    clean_index: Option<usize>, // Index of "saved" state within the undo stack.
}

impl UndoStack {
    /// Creates a new `UndoStack` with a default capacity of 100.
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    /// Creates a new `UndoStack` with the specified maximum capacity.
    pub fn with_capacity(max: usize) -> Self {
        UndoStack {
            undo_stack: Vec::with_capacity(max.min(128)),
            redo_stack: Vec::new(),
            max_capacity: max,
            clean_index: None,
        }
    }

    /// Push a new command onto the undo stack, clearing the redo stack.
    ///
    /// If the previous command has a compatible merge policy, this will
    /// attempt to merge the new command into the previous one instead of
    /// pushing a separate entry.
    pub fn push(&mut self, command: Box<dyn UndoCommand>) {
        // Try merging with the last command.
        if self.merge_policy_allows(&*command) {
            if let Some(last) = self.undo_stack.last_mut() {
                if last.try_merge(command.as_ref()) {
                    // Merge succeeded — no new entry needed.
                    self.redo_stack.clear();
                    return;
                }
            }
        }

        self.redo_stack.clear();

        // Capacity enforcement and the clean-index fixup are shared with `redo`:
        // both append to the undo stack and both must honour the bound.
        self.push_undo_entry(command);
    }

    /// Append to the undo stack, enforcing `max_capacity` and moving `clean_index`.
    ///
    /// Shared by [`push`](Self::push) and [`redo`](Self::redo). `redo` used to append with a
    /// bare `Vec::push`, so it bypassed both: with `set_max_capacity` lowered while commands sat
    /// on the redo stack, redoing grew the stack past its own bound (unbounded memory in a
    /// long-lived editor session that re-tunes capacity) and left `clean_index` pointing at the
    /// wrong entry, so `is_clean()` could no longer identify the saved state.
    fn push_undo_entry(&mut self, command: Box<dyn UndoCommand>) {
        if self.max_capacity == 0 {
            // Zero-capacity stacks retain nothing; drop the command silently.
            return;
        }
        if self.undo_stack.len() >= self.max_capacity {
            self.undo_stack.remove(0);
            // Adjust clean_index if it was shifted.
            if let Some(ref mut idx) = self.clean_index {
                if *idx > 0 {
                    *idx -= 1;
                } else {
                    // The clean state was removed.
                    self.clean_index = None;
                }
            }
        }

        self.undo_stack.push(command);
    }

    /// Undo the most recent command, moving it to the redo stack.
    pub fn undo(&mut self) -> Result<(), String> {
        let mut command = self.undo_stack.pop().ok_or_else(|| {
            format!(
                "nothing to undo: the undo stack is empty ({} redoable command(s) pending)",
                self.redo_stack.len()
            )
        })?;
        command.undo()?;
        self.redo_stack.push(command);
        Ok(())
    }

    /// Redo the most recently undone command, moving it back to the undo stack.
    pub fn redo(&mut self) -> Result<(), String> {
        let mut command = self.redo_stack.pop().ok_or_else(|| {
            format!(
                "nothing to redo: the redo stack is empty ({} undoable command(s) pending)",
                self.undo_stack.len()
            )
        })?;
        command.redo()?;
        // Through `push_undo_entry`, not a bare push: redoing must not be able to grow the
        // stack past `max_capacity` or desynchronise `clean_index`. `push` already enforced
        // both; this path did not, so a capacity lowered while the redo stack was populated
        // was silently violated on the way back.
        self.push_undo_entry(command);
        Ok(())
    }

    /// Returns `true` if there are commands available to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there are commands available to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns the number of commands in the undo stack.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the number of commands in the redo stack.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Returns the human-readable text of the next command to undo.
    pub fn undo_text(&self) -> Option<String> {
        self.undo_stack.last().map(|c| c.description().text)
    }

    /// Returns the human-readable text of the next command to redo.
    pub fn redo_text(&self) -> Option<String> {
        self.redo_stack.last().map(|c| c.description().text)
    }

    /// Mark the current state as "clean" (e.g., saved).
    pub fn mark_clean(&mut self) {
        self.clean_index = Some(self.undo_stack.len());
    }

    /// Returns `true` if the current state is the "clean" (saved) state.
    ///
    /// A stack with no saved marker is clean only when empty.
    /// Once `mark_clean()` has been called, the stack is clean only
    /// when the undo position matches the recorded clean index.
    pub fn is_clean(&self) -> bool {
        match self.clean_index {
            Some(idx) => idx == self.undo_stack.len(),
            None => self.undo_stack.is_empty(),
        }
    }

    /// Clear all commands from both undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.clean_index = None;
    }

    /// Set the maximum capacity, dropping oldest commands if the new limit
    /// is smaller than the current undo stack length.
    pub fn set_max_capacity(&mut self, max: usize) {
        self.max_capacity = max;
        while self.undo_stack.len() > self.max_capacity {
            self.undo_stack.remove(0);
            if let Some(ref mut idx) = self.clean_index {
                if *idx > 0 {
                    *idx -= 1;
                } else {
                    self.clean_index = None;
                }
            }
        }
    }

    /// Check whether the command's merge policy tells us to attempt merging.
    fn merge_policy_allows(&self, command: &dyn UndoCommand) -> bool {
        if self.undo_stack.is_empty() {
            return false;
        }
        command.merge_policy() == MergePolicy::WithPrevious
    }
}

crate::impl_default_via_new!(UndoStack);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{Box, MiniToString};
    use std::time::SystemTime;

    // ── Test helpers ──

    /// Monotonic command-id source for the test fixtures.
    ///
    /// This used to be a `static mut NEXT_ID: u64` incremented inside an
    /// `unsafe` block. Rust's test harness runs tests on multiple threads, so
    /// that was a genuine data race (and `static mut` is a hard error in Rust
    /// 2024). An atomic gives the same unique-id behaviour without `unsafe`.
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn next_command_id() -> CommandId {
        CommandId(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    /// A basic command that does NOT merge. Used for most tests.
    struct TextCommand {
        id: CommandId,
        text: String,
        applied: String,
    }

    impl TextCommand {
        fn new(text: &str, initial: &str) -> Self {
            let id = next_command_id();
            TextCommand { id, text: text.to_string(), applied: initial.to_string() }
        }
    }

    impl UndoCommand for TextCommand {
        fn id(&self) -> CommandId {
            self.id
        }

        fn description(&self) -> CommandDescription {
            CommandDescription {
                text: format!("Edit: {}", self.text),
                timestamp_ms: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                command_type: "TextCommand",
            }
        }

        fn execute(&mut self) -> Result<(), String> {
            self.applied.push_str(&self.text);
            Ok(())
        }

        fn undo(&mut self) -> Result<(), String> {
            let len = self.applied.len();
            let remove_len = self.text.len();
            if remove_len > len {
                return Err(format!(
                    "cannot undo appending {} byte(s): only {} byte(s) are applied",
                    remove_len, len
                ));
            }
            self.applied.truncate(len - remove_len);
            Ok(())
        }

        fn merge_policy(&self) -> MergePolicy {
            MergePolicy::Never
        }
    }

    /// A command that supports merging with previous. Used for merge tests.
    struct MergeableTextCommand {
        id: CommandId,
        text: String,
        applied: String,
    }

    impl MergeableTextCommand {
        fn new(text: &str, initial: &str) -> Self {
            let id = next_command_id();
            MergeableTextCommand { id, text: text.to_string(), applied: initial.to_string() }
        }
    }

    impl UndoCommand for MergeableTextCommand {
        fn id(&self) -> CommandId {
            self.id
        }

        fn description(&self) -> CommandDescription {
            CommandDescription {
                text: format!("Edit: {}", self.text),
                timestamp_ms: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                command_type: "TextCommand",
            }
        }

        fn execute(&mut self) -> Result<(), String> {
            self.applied.push_str(&self.text);
            Ok(())
        }

        fn undo(&mut self) -> Result<(), String> {
            let len = self.applied.len();
            let remove_len = self.text.len();
            if remove_len > len {
                return Err(format!(
                    "cannot undo appending {} byte(s): only {} byte(s) are applied",
                    remove_len, len
                ));
            }
            self.applied.truncate(len - remove_len);
            Ok(())
        }

        fn merge_policy(&self) -> MergePolicy {
            MergePolicy::WithPrevious
        }

        fn try_merge(&mut self, previous: &dyn UndoCommand) -> bool {
            if previous.description().command_type != "TextCommand" {
                return false;
            }
            // Combine text and applied state so undoing once undoes the merged operation.
            let prev_action = previous.description().text.replacen("Edit: ", "", 1);
            self.text.push_str(&prev_action);
            self.applied.push_str(&prev_action);
            true
        }
    }

    // ── Tests ──

    #[test]
    fn test_push_undo_redo() {
        let mut stack = UndoStack::new();
        let mut cmd = TextCommand::new("hello", "");
        cmd.execute().unwrap();

        stack.push(Box::new(cmd));
        assert!(stack.can_undo());
        assert_eq!(stack.undo_count(), 1);
        assert!(!stack.can_redo());

        // Undo
        stack.undo().unwrap();
        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        // Redo
        stack.redo().unwrap();
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_multiple_undos() {
        let mut stack = UndoStack::new();

        let mut c1 = TextCommand::new("A", "");
        c1.execute().unwrap();
        stack.push(Box::new(c1));

        let mut c2 = TextCommand::new("B", "");
        c2.execute().unwrap();
        stack.push(Box::new(c2));

        let mut c3 = TextCommand::new("C", "");
        c3.execute().unwrap();
        stack.push(Box::new(c3));

        assert_eq!(stack.undo_count(), 3);

        stack.undo().unwrap();
        stack.undo().unwrap();
        assert_eq!(stack.undo_count(), 1);
        assert_eq!(stack.redo_count(), 2);

        stack.redo().unwrap();
        assert_eq!(stack.undo_count(), 2);
        assert_eq!(stack.redo_count(), 1);
    }

    #[test]
    fn test_redo_cleared_on_new_push() {
        let mut stack = UndoStack::new();

        let mut c1 = TextCommand::new("X", "");
        c1.execute().unwrap();
        stack.push(Box::new(c1));

        stack.undo().unwrap();
        assert!(stack.can_redo());

        // New push after undo should clear redo stack.
        let mut c2 = TextCommand::new("Y", "");
        c2.execute().unwrap();
        stack.push(Box::new(c2));

        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 1);
    }

    #[test]
    fn test_capacity_limit() {
        let mut stack = UndoStack::with_capacity(3);

        for i in 0..5 {
            let mut cmd = TextCommand::new(&i.to_string(), "");
            cmd.execute().unwrap();
            stack.push(Box::new(cmd));
        }

        // Only the 3 most recent should remain.
        assert_eq!(stack.undo_count(), 3);
    }

    #[test]
    fn test_mark_clean() {
        let mut stack = UndoStack::new();
        assert!(stack.is_clean(), "Empty stack should be clean");

        let mut c1 = TextCommand::new("A", "");
        c1.execute().unwrap();
        stack.push(Box::new(c1));

        assert!(!stack.is_clean(), "After push, should not be clean");

        stack.mark_clean();
        assert!(stack.is_clean(), "After mark_clean, should be clean");

        stack.undo().unwrap();
        assert!(!stack.is_clean(), "After undo, should not be clean");

        stack.redo().unwrap();
        assert!(stack.is_clean(), "After redo back, should be clean again");
    }

    #[test]
    fn test_merge_policy() {
        let mut stack = UndoStack::new();

        let mut c1 = MergeableTextCommand::new("A", "");
        c1.execute().unwrap();
        stack.push(Box::new(c1));

        let mut c2 = MergeableTextCommand::new("B", "");
        c2.execute().unwrap();
        stack.push(Box::new(c2));

        // After merge, there should be only one command in the stack.
        assert_eq!(stack.undo_count(), 1, "Commands should have merged");

        // Undoing the merged command should undo both A and B.
        stack.undo().unwrap();
        assert!(!stack.can_undo());
    }

    #[test]
    fn test_command_descriptions() {
        let mut stack = UndoStack::new();

        let mut cmd = TextCommand::new("Insert Hello", "");
        cmd.execute().unwrap();
        stack.push(Box::new(cmd));

        let undo_text = stack.undo_text();
        assert!(undo_text.is_some());

        let text = undo_text.unwrap();
        assert!(
            text.contains("Insert Hello"),
            "Description should contain action text, got: {}",
            text
        );

        stack.undo().unwrap();
        let redo_text = stack.redo_text();
        assert!(redo_text.is_some());
        assert!(redo_text.unwrap().contains("Insert Hello"));
    }

    #[test]
    fn test_clear() {
        let mut stack = UndoStack::new();

        let mut c1 = TextCommand::new("A", "");
        c1.execute().unwrap();
        stack.push(Box::new(c1));

        let mut c2 = TextCommand::new("B", "");
        c2.execute().unwrap();
        stack.push(Box::new(c2));

        stack.mark_clean();

        stack.undo().unwrap();

        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 0);
        assert!(stack.is_clean(), "After clear, stack should be clean");
    }

    #[test]
    fn test_undo_on_empty_stack_fails() {
        let mut stack = UndoStack::new();
        let result = stack.undo();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("nothing to undo") && err.contains("empty"), "{err}");
    }

    #[test]
    fn test_redo_on_empty_stack_fails() {
        let mut stack = UndoStack::new();
        let result = stack.redo();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("nothing to redo") && err.contains("empty"), "{err}");
    }

    #[test]
    fn test_set_max_capacity_drops_oldest() {
        let mut stack = UndoStack::with_capacity(10);

        for i in 0..5 {
            let mut cmd = TextCommand::new(&i.to_string(), "");
            cmd.execute().unwrap();
            stack.push(Box::new(cmd));
        }

        assert_eq!(stack.undo_count(), 5);

        stack.set_max_capacity(2);
        assert_eq!(stack.undo_count(), 2);
    }

    #[test]
    fn test_default_impl() {
        let stack: UndoStack = Default::default();
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 0);
        assert!(stack.is_clean());
    }

    #[test]
    fn test_zero_capacity_push_does_not_panic() {
        let mut stack = UndoStack::with_capacity(0);
        // Regression: pushing onto a zero-capacity stack used to call
        // `Vec::remove(0)` on an empty vector and panic.
        stack.push(Box::new(TextCommand::new("x", "")));
        assert_eq!(stack.undo_count(), 0);
        assert!(stack.undo().is_err());
    }

    #[test]
    fn test_set_max_capacity_zero_clears_and_accepts_pushes() {
        let mut stack = UndoStack::with_capacity(10);
        stack.push(Box::new(TextCommand::new("a", "")));
        stack.push(Box::new(TextCommand::new("b", "")));
        stack.set_max_capacity(0);
        assert_eq!(stack.undo_count(), 0);
        // New pushes are dropped without panicking.
        stack.push(Box::new(TextCommand::new("c", "")));
        assert_eq!(stack.undo_count(), 0);
    }

    /// `redo` must honour `max_capacity` exactly as `push` does.
    ///
    /// It used a bare `Vec::push`, so lowering the capacity while commands sat on the redo
    /// stack and then redoing grew the undo stack past its own bound. The bound is what caps
    /// memory in a long-lived editor session, and `is_clean()` keys off `clean_index`, which the
    /// same code path moved.
    #[test]
    fn test_redo_respects_max_capacity() {
        let mut stack = UndoStack::with_capacity(10);
        for i in 0..5 {
            let mut cmd = TextCommand::new(&format!("c{i}"), "");
            cmd.execute().unwrap();
            stack.push(Box::new(cmd));
        }
        for _ in 0..3 {
            stack.undo().unwrap();
        }
        assert_eq!(stack.undo_count(), 2);
        assert_eq!(stack.redo_count(), 3);

        // The capacity drops below what the redo path would produce.
        stack.set_max_capacity(2);

        stack.redo().unwrap();
        assert!(
            stack.undo_count() <= 2,
            "redo grew the stack past max_capacity=2: {}",
            stack.undo_count()
        );
        stack.redo().unwrap();
        assert!(
            stack.undo_count() <= 2,
            "redo grew the stack past max_capacity=2: {}",
            stack.undo_count()
        );
    }

    /// Redoing all the way back to the saved state must report `is_clean`.
    ///
    /// The clean index is moved by capacity eviction; a redo that appended without the fixup
    /// left `is_clean()` unable to identify the state the document was saved at.
    #[test]
    fn test_redo_keeps_clean_index_in_step() {
        let mut stack = UndoStack::with_capacity(8);
        for text in ["a", "b"] {
            let mut cmd = TextCommand::new(text, "");
            cmd.execute().unwrap();
            stack.push(Box::new(cmd));
        }
        stack.mark_clean();
        assert!(stack.is_clean());

        let mut cmd = TextCommand::new("c", "");
        cmd.execute().unwrap();
        stack.push(Box::new(cmd));
        assert!(!stack.is_clean());

        stack.undo().unwrap();
        assert!(stack.is_clean(), "undoing back to the saved state must be clean");

        stack.redo().unwrap();
        assert!(!stack.is_clean(), "redoing away from the saved state must be dirty");

        stack.undo().unwrap();
        assert!(stack.is_clean(), "undo must return to the saved state again");
    }
}
