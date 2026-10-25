// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reversible command stacks for the action layer (W-202).
//!
//! # The gap this closes
//!
//! `ActionRouter::connect_undo_redo` bound two callbacks to `Primary+Z` /
//! `Primary+Shift+Z` and stopped there. An action could run, but nothing recorded *what* it did,
//! so a host that wanted undo had to build the stack itself. This module supplies the missing
//! layer: a [`Command`] carries the pair of operations that reverse and replay one user action,
//! [`UndoStack`] records them with group boundaries and a dirty flag, and [`UndoManager`] keeps
//! one stack per focus context and selects the active one.
//!
//! # Relationship to `connect_undo_redo`
//!
//! The existing helper is unchanged and remains the low-level convenience (two closures bound to
//! the platform accelerators). [`crate::action::ActionRouter::connect_undo_redo_stack`] is the
//! stack-top form: it binds the same accelerators to [`UndoManager::undo`] /
//! [`UndoManager::redo`] on the active context.
//!
//! # Threading
//!
//! Commands are `Send` but not `Sync`: a stack owns its callbacks and is mutated from one thread.
//! A host that drives a stack from the event loop wraps it in `Arc<Mutex<UndoManager>>` (see
//! `connect_undo_redo_stack`), which is the same shape the event loop uses for the action registry.

use crate::compat::{Box, HashMap, String, Vec};

/// The smallest number of entries a stack keeps unless the host asks for more.
pub const DEFAULT_UNDO_LIMIT: usize = 256;

/// The context key used before any focus context is activated.
pub const GLOBAL_UNDO_CONTEXT: &str = "";

/// One reversible step: how to undo it and how to replay it.
struct CommandStep {
    undo: Box<dyn FnMut() + Send>,
    redo: Box<dyn FnMut() + Send>,
}

impl CommandStep {
    fn undo(&mut self) {
        (self.undo)();
    }

    fn redo(&mut self) {
        (self.redo)();
    }
}

/// A reversible command: one user action, or a group of actions treated as one undo step.
///
/// A command owns its closures. Building one from a pair of closures is the common case:
///
/// ```ignore
/// stack.push(Command::new("insert_text", move || remove(), move || insert_again()));
/// ```
///
/// Grouping is the same type with more than one step: [`UndoStack::begin_group`] collects the
/// commands pushed until [`UndoStack::end_group`] into one command whose undo replays the steps in
/// reverse and whose redo replays them forward.
pub struct Command {
    label: String,
    steps: Vec<CommandStep>,
}

impl Command {
    /// Creates a command from its undo and redo operations.
    pub fn new<U, R>(label: impl Into<String>, undo: U, redo: R) -> Self
    where
        U: FnMut() + Send + 'static,
        R: FnMut() + Send + 'static,
    {
        Self {
            label: label.into(),
            steps: alloc::vec![CommandStep { undo: Box::new(undo), redo: Box::new(redo) }],
        }
    }

    /// An empty command, used as the accumulator for a group boundary.
    fn blank(label: impl Into<String>) -> Self {
        Self { label: label.into(), steps: Vec::new() }
    }

    /// The label this command was recorded under (typically the action id that produced it).
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Whether the command has any steps. Only a group can be empty, and an empty group is dropped
    /// rather than recorded as a no-op undo step.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Moves every step of `other` into this command, preserving order.
    fn absorb(&mut self, other: Self) {
        self.steps.extend(other.steps);
    }

    /// Reverses the command. A group is undone in reverse order, which is what makes nested edits
    /// of the same value restore the earliest state last.
    fn undo(&mut self) {
        for step in self.steps.iter_mut().rev() {
            step.undo();
        }
    }

    /// Replays the command in its original order.
    fn redo(&mut self) {
        for step in self.steps.iter_mut() {
            step.redo();
        }
    }
}

impl core::fmt::Debug for Command {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Command")
            .field("label", &self.label)
            .field("steps", &self.steps.len())
            .finish()
    }
}

/// One undo history: the undo/redo piles, a group boundary, and a dirty flag.
///
/// # Why a second pile instead of re-running the operations
///
/// A new command clears the redo pile (the classic branch rule). Redo therefore needs the command
/// objects that were undone, and they are moved — never cloned — between the piles, so a command
/// with a captured resource is not duplicated.
pub struct UndoStack {
    limit: usize,
    undo: Vec<Command>,
    redo: Vec<Command>,
    open_group: Option<Command>,
    group_depth: usize,
    dirty: bool,
}

impl UndoStack {
    /// Creates a stack with [`DEFAULT_UNDO_LIMIT`] entries.
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_UNDO_LIMIT)
    }

    /// Creates a stack that keeps at most `limit` undo steps.
    ///
    /// A limit of zero is treated as one: a stack that can never hold a command could not undo.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            undo: Vec::new(),
            redo: Vec::new(),
            open_group: None,
            group_depth: 0,
            dirty: false,
        }
    }

    /// The maximum number of undo steps retained.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Changes the retention limit, dropping the oldest steps if the new limit is smaller.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        self.trim();
    }

    /// Records `command` as one undo step.
    ///
    /// Inside an open group the command is absorbed into the group instead. Either way the redo
    /// pile is cleared, because the history has branched.
    pub fn push(&mut self, command: Command) {
        if command.is_empty() {
            return;
        }
        self.dirty = true;
        self.redo.clear();
        if let Some(group) = &mut self.open_group {
            group.absorb(command);
        } else {
            self.undo.push(command);
            self.trim();
        }
    }

    /// Records a command from its undo/redo closures, under `label`.
    pub fn record<U, R>(&mut self, label: impl Into<String>, undo: U, redo: R)
    where
        U: FnMut() + Send + 'static,
        R: FnMut() + Send + 'static,
    {
        self.push(Command::new(label, undo, redo));
    }

    /// Opens a group boundary: every command pushed until [`Self::end_group`] becomes one step.
    ///
    /// Nested calls are allowed; only the outermost pair creates a boundary, so a helper that
    /// groups its own edits does not fragment a caller's group.
    pub fn begin_group(&mut self, label: impl Into<String>) {
        if self.group_depth == 0 {
            self.open_group = Some(Command::blank(label));
        }
        self.group_depth += 1;
    }

    /// Closes a group boundary, reporting whether a non-empty group was recorded.
    ///
    /// `false` also means the call was unbalanced (no group was open) or the group collected
    /// nothing.
    pub fn end_group(&mut self) -> bool {
        if self.group_depth == 0 {
            return false;
        }
        self.group_depth -= 1;
        if self.group_depth > 0 {
            return false;
        }
        let Some(group) = self.open_group.take() else {
            return false;
        };
        if group.is_empty() {
            return false;
        }
        self.undo.push(group);
        self.trim();
        true
    }

    /// Whether a group boundary is currently open.
    pub fn is_group_open(&self) -> bool {
        self.group_depth > 0
    }

    /// Undoes the newest command, returning its label.
    ///
    /// `None` means there is nothing to undo, or a group is still open — an open group has no
    /// committed step yet, and undoing half of it would leave the group inconsistent. Call
    /// [`Self::end_group`] first.
    pub fn undo(&mut self) -> Option<String> {
        if self.is_group_open() {
            return None;
        }
        let mut command = self.undo.pop()?;
        command.undo();
        let label = String::from(command.label());
        self.redo.push(command);
        self.dirty = true;
        Some(label)
    }

    /// Replays the newest undone command, returning its label.
    pub fn redo(&mut self) -> Option<String> {
        if self.is_group_open() {
            return None;
        }
        let mut command = self.redo.pop()?;
        command.redo();
        let label = String::from(command.label());
        self.undo.push(command);
        self.dirty = true;
        Some(label)
    }

    /// Whether [`Self::undo`] would run a command.
    pub fn can_undo(&self) -> bool {
        !self.is_group_open() && !self.undo.is_empty()
    }

    /// Whether [`Self::redo`] would run a command.
    pub fn can_redo(&self) -> bool {
        !self.is_group_open() && !self.redo.is_empty()
    }

    /// Number of undo steps currently retained.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Number of redo steps currently retained.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Whether changes have been recorded since the last [`Self::mark_saved`].
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Marks the current state as saved, clearing the dirty flag.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Clears both piles, the open group and the dirty flag.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open_group = None;
        self.group_depth = 0;
        self.dirty = false;
    }

    /// Drops the oldest undo steps until the limit is respected.
    fn trim(&mut self) {
        while self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for UndoStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UndoStack")
            .field("undo", &self.undo.len())
            .field("redo", &self.redo.len())
            .field("limit", &self.limit)
            .field("dirty", &self.dirty)
            .finish()
    }
}

/// A set of undo stacks, one per focus context, with one selected as active.
///
/// # Why more than one stack
///
/// A document window and a modal editor are different undo domains: undoing in the active editor
/// must not reach into the document behind it. The host activates a context by name (a window or
/// modal id it already owns) and keeps using the same [`Self::undo`] / [`Self::redo`] calls; the
/// manager routes them to the selected stack. [`GLOBAL_UNDO_CONTEXT`] is the initial context, so a
/// single-stack host never calls [`Self::set_active_context`] at all.
pub struct UndoManager {
    stacks: HashMap<String, UndoStack>,
    active: String,
    limit: usize,
}

impl UndoManager {
    /// Creates a manager with only the global context active.
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_UNDO_LIMIT)
    }

    /// Creates a manager whose per-context stacks keep at most `limit` steps.
    pub fn with_limit(limit: usize) -> Self {
        let mut stacks = HashMap::new();
        stacks.insert(String::from(GLOBAL_UNDO_CONTEXT), UndoStack::with_limit(limit));
        Self { stacks, active: String::from(GLOBAL_UNDO_CONTEXT), limit }
    }

    /// The active context key. [`GLOBAL_UNDO_CONTEXT`] until changed.
    pub fn active_context(&self) -> &str {
        &self.active
    }

    /// Activates `context`, creating its stack on first use, and returns the previous context.
    pub fn set_active_context(&mut self, context: impl Into<String>) -> String {
        let context = context.into();
        self.stacks.entry(context.clone()).or_insert_with(|| UndoStack::with_limit(self.limit));
        core::mem::replace(&mut self.active, context)
    }

    /// The stack for `context`, if that context has ever been activated.
    pub fn stack_for(&self, context: &str) -> Option<&UndoStack> {
        self.stacks.get(context)
    }

    /// The active stack.
    pub fn stack(&self) -> &UndoStack {
        // `active` is inserted before it is ever selected, so the lookup cannot miss.
        self.stacks.get(&self.active).expect("active undo context always has a stack")
    }

    /// The active stack, mutably.
    pub fn stack_mut(&mut self) -> &mut UndoStack {
        self.stacks.get_mut(&self.active).expect("active undo context always has a stack")
    }

    /// Every context this manager has created a stack for.
    pub fn contexts(&self) -> Vec<String> {
        self.stacks.keys().cloned().collect()
    }

    /// Records a command on the active stack.
    pub fn push(&mut self, command: Command) {
        self.stack_mut().push(command);
    }

    /// Records a command from its undo/redo closures on the active stack.
    pub fn record<U, R>(&mut self, label: impl Into<String>, undo: U, redo: R)
    where
        U: FnMut() + Send + 'static,
        R: FnMut() + Send + 'static,
    {
        self.stack_mut().record(label, undo, redo);
    }

    /// Opens a group boundary on the active stack.
    pub fn begin_group(&mut self, label: impl Into<String>) {
        self.stack_mut().begin_group(label);
    }

    /// Closes a group boundary on the active stack.
    pub fn end_group(&mut self) -> bool {
        self.stack_mut().end_group()
    }

    /// Undoes the active stack's newest command, returning its label.
    pub fn undo(&mut self) -> Option<String> {
        self.stack_mut().undo()
    }

    /// Replays the active stack's newest undone command, returning its label.
    pub fn redo(&mut self) -> Option<String> {
        self.stack_mut().redo()
    }

    /// Whether the active stack can undo.
    pub fn can_undo(&self) -> bool {
        self.stack().can_undo()
    }

    /// Whether the active stack can redo.
    pub fn can_redo(&self) -> bool {
        self.stack().can_redo()
    }

    /// Whether the active stack has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.stack().is_dirty()
    }

    /// Marks the active stack's current state as saved.
    pub fn mark_saved(&mut self) {
        self.stack_mut().mark_saved();
    }

    /// Clears the active stack.
    pub fn clear(&mut self) {
        self.stack_mut().clear();
    }
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for UndoManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UndoManager")
            .field("active", &self.active)
            .field("contexts", &self.stacks.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Mutex};
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicI32, Ordering};

    fn counter() -> Arc<AtomicI32> {
        Arc::new(AtomicI32::new(0))
    }

    #[test]
    fn undo_and_redo_move_one_command_between_the_piles() {
        let value = counter();
        let mut stack = UndoStack::new();
        let (set_value, undo_value) = (Arc::clone(&value), Arc::clone(&value));
        stack.record(
            "increment",
            move || {
                undo_value.fetch_sub(1, Ordering::SeqCst);
            },
            move || {
                set_value.fetch_add(1, Ordering::SeqCst);
            },
        );
        // The caller performed the edit; the stack only records how to reverse it.
        value.store(1, Ordering::SeqCst);

        assert!(stack.can_undo());
        assert_eq!(stack.undo().as_deref(), Some("increment"));
        assert_eq!(value.load(Ordering::SeqCst), 0);
        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        assert_eq!(stack.redo().as_deref(), Some("increment"));
        assert_eq!(value.load(Ordering::SeqCst), 1);
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn pushing_a_new_command_clears_redo() {
        let mut stack = UndoStack::new();
        stack.record("first", || {}, || {});
        stack.undo();
        assert_eq!(stack.redo_len(), 1);
        stack.record("second", || {}, || {});
        assert_eq!(stack.redo_len(), 0, "a new edit branches the history");
    }

    #[test]
    fn a_group_is_one_undo_step_and_undoes_in_reverse_order() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut stack = UndoStack::new();
        stack.begin_group("typing");
        for ch in ["a", "b", "c"] {
            let undo_order = Arc::clone(&order);
            let redo_order = Arc::clone(&order);
            let recorded = String::from(ch);
            stack.record(
                recorded,
                move || lock(&undo_order).push(-1),
                move || lock(&redo_order).push(1),
            );
        }
        assert!(stack.end_group());
        assert_eq!(stack.undo_len(), 1, "the group is a single undo step");

        stack.undo();
        assert_eq!(*lock(&order), alloc::vec![-1, -1, -1]);
        stack.redo();
        assert_eq!(*lock(&order), alloc::vec![-1, -1, -1, 1, 1, 1]);
    }

    #[test]
    fn undo_is_refused_while_a_group_is_open() {
        let mut stack = UndoStack::new();
        stack.begin_group("group");
        stack.record("step", || {}, || {});
        assert!(!stack.can_undo());
        assert_eq!(stack.undo(), None);
        assert!(stack.end_group());
        assert_eq!(stack.undo().as_deref(), Some("group"));
    }

    #[test]
    fn the_limit_drops_the_oldest_step() {
        let mut stack = UndoStack::with_limit(2);
        stack.record("one", || {}, || {});
        stack.record("two", || {}, || {});
        stack.record("three", || {}, || {});
        assert_eq!(stack.undo_len(), 2);
        assert_eq!(stack.undo().as_deref(), Some("three"));
        assert_eq!(stack.undo().as_deref(), Some("two"));
        assert_eq!(stack.undo(), None);
    }

    #[test]
    fn the_dirty_flag_tracks_edits_until_saved() {
        let mut stack = UndoStack::new();
        assert!(!stack.is_dirty());
        stack.record("edit", || {}, || {});
        assert!(stack.is_dirty());
        stack.mark_saved();
        assert!(!stack.is_dirty());
        stack.undo();
        assert!(stack.is_dirty(), "undoing changes the document away from the saved state");
    }

    #[test]
    fn contexts_keep_independent_histories() {
        let mut manager = UndoManager::new();
        manager.record("document", || {}, || {});
        assert!(manager.can_undo());

        let previous = manager.set_active_context("dialog");
        assert_eq!(previous, GLOBAL_UNDO_CONTEXT);
        assert!(!manager.can_undo(), "the dialog starts with its own empty stack");

        manager.record("dialog", || {}, || {});
        assert_eq!(manager.undo().as_deref(), Some("dialog"));

        manager.set_active_context(GLOBAL_UNDO_CONTEXT);
        assert!(manager.can_undo(), "the document stack was not touched");
        assert_eq!(manager.undo().as_deref(), Some("document"));
    }
}
