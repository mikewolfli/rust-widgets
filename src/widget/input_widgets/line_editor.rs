// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The single-line value editor an editable drop-down field carries (I-06).
//!
//! # Why this is its own type
//!
//! A drop-down that accepts typed values is a **single-line editor plus a list**. Before this
//! type existed, `ComboBox::set_editable(true)` only switched on a type-ahead mode: the field had
//! no caret, no selection, no IME commit and no undo, and a custom value could only be written by
//! the host through `set_current_text`. This type is the text model such a field needs, and it is
//! deliberately the *same* model `EditableComboBox` implements inline (byte caret, optional
//! anchor, `UndoStack` over a shared history string), so the two editable choosers cannot drift
//! into two different ideas of where the caret may stand.
//!
//! The type is crate-internal: it is an implementation detail of the controls that own it, not a
//! widget and not a published API.

use core::cell::RefCell;

use crate::compat::{Box, MiniToString, Rc, String};
use crate::undo::{TextSpliceCommand, UndoStack};
use crate::widget::text_utils::{floor_char_boundary, next_char_boundary, previous_char_boundary};

/// A single-line value: the text, the caret, the selection anchor and the undo history.
pub(crate) struct LineEditor {
    text: String,
    /// The insertion point, in bytes from the start of the value.
    cursor: usize,
    /// The other end of a selection, in bytes. `None` means no selection.
    anchor: Option<usize>,
    undo_stack: UndoStack,
    /// The string the undo commands read and write.
    history_target: Rc<RefCell<String>>,
    /// True while a history replay writes the text, so the replay is not recorded again.
    restoring: bool,
}

impl LineEditor {
    /// An empty editor with the caret at the start.
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            anchor: None,
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring: false,
        }
    }

    /// The current value.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The caret's byte offset.
    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selected byte range, or `None` when nothing is selected.
    pub(crate) fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        let (start, end) =
            if anchor <= self.cursor { (anchor, self.cursor) } else { (self.cursor, anchor) };
        (start != end).then_some((start, end))
    }

    /// Replaces the value. `record` puts the previous value on the undo stack.
    ///
    /// A programmatic write is *not* an edit the user made, so callers that assign a value rather
    /// than type one pass `false`: undoing must not resurrect a value the host itself replaced.
    pub(crate) fn set_text(&mut self, text: String, record: bool) -> bool {
        if self.text == text {
            return false;
        }
        // The record is the changed interval, not the whole value: `TextSpliceCommand::new`
        // derives it from the before/after pair (I-001).
        let before = core::mem::replace(&mut self.text, text);
        let after = self.text.clone();
        self.cursor = self.text.len();
        self.anchor = None;
        self.push_history(&before, &after, record);
        true
    }

    /// Replaces the selection (or inserts at the caret) with `text`.
    pub(crate) fn insert(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let (start, end) = self.selection().unwrap_or((self.cursor, self.cursor));
        let removed = self.text[start..end].to_string();
        self.text.replace_range(start..end, text);
        self.cursor = start + text.len();
        self.anchor = None;
        self.push_splice(start, &removed, text);
        true
    }

    /// Deletes the selection, or the character before the caret.
    pub(crate) fn backspace(&mut self) -> bool {
        let (start, end) = self
            .selection()
            .unwrap_or((previous_char_boundary(&self.text, self.cursor), self.cursor));
        self.delete_range(start, end)
    }

    /// Deletes the selection, or the character after the caret.
    pub(crate) fn delete_forward(&mut self) -> bool {
        let (start, end) =
            self.selection().unwrap_or((self.cursor, next_char_boundary(&self.text, self.cursor)));
        self.delete_range(start, end)
    }

    /// Removes `start..end`, recording one undo step.
    fn delete_range(&mut self, start: usize, end: usize) -> bool {
        if start == end {
            return false;
        }
        let removed = self.text[start..end].to_string();
        self.text.replace_range(start..end, "");
        self.cursor = start;
        self.anchor = None;
        self.push_splice(start, &removed, "");
        true
    }

    /// Moves the caret by `delta` characters, extending the selection when asked.
    pub(crate) fn move_caret(&mut self, delta: i32, extend: bool) -> bool {
        let mut target = self.cursor;
        if delta < 0 {
            for _ in 0..delta.unsigned_abs() {
                target = previous_char_boundary(&self.text, target);
            }
        } else {
            for _ in 0..delta.unsigned_abs() {
                target = next_char_boundary(&self.text, target);
            }
        }
        self.set_caret(target, extend)
    }

    /// Places the caret at `position`, extending the selection when asked.
    pub(crate) fn set_caret(&mut self, position: usize, extend: bool) -> bool {
        let target = floor_char_boundary(&self.text, position.min(self.text.len()));
        let next_anchor = if extend { Some(self.anchor.unwrap_or(self.cursor)) } else { None };
        if self.cursor == target && self.anchor == next_anchor {
            return false;
        }
        self.cursor = target;
        self.anchor = next_anchor;
        true
    }

    /// Selects the whole value.
    pub(crate) fn select_all(&mut self) -> bool {
        if self.text.is_empty() {
            return false;
        }
        let changed = self.selection() != Some((0, self.text.len()));
        self.anchor = Some(0);
        self.cursor = self.text.len();
        changed
    }

    /// Steps back one recorded edit.
    pub(crate) fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history();
        true
    }

    /// Steps forward one undone edit.
    pub(crate) fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history();
        true
    }

    /// Whether an edit can be undone.
    pub(crate) fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Whether an undone edit can be re-applied.
    pub(crate) fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Reads the value the undo command just wrote and parks the caret at its end.
    fn restore_history(&mut self) {
        self.restoring = true;
        self.text = self.history_target.borrow().clone();
        self.restoring = false;
        self.cursor = self.text.len();
        self.anchor = None;
    }

    /// Records `before` → the current value as one undoable step.
    ///
    /// Used for a programmatic write, where only the diff is needed; the normal edit paths pass
    /// their splice directly (I-001).
    fn push_history(&mut self, before: &str, after: &str, record: bool) {
        if !record || self.restoring {
            return;
        }
        *self.history_target.borrow_mut() = after.to_string();
        self.undo_stack.push(Box::new(TextSpliceCommand::new(
            self.history_target.clone(),
            before,
            after,
            "Edit text",
            "combo_box_text",
        )));
    }

    /// Records an in-place splice as one undoable step (I-001).
    fn push_splice(&mut self, offset: usize, removed: &str, inserted: &str) {
        if self.restoring {
            return;
        }
        {
            let mut target = self.history_target.borrow_mut();
            let start = offset.min(target.len());
            let end = (start + removed.len()).min(target.len());
            target.replace_range(start..end, inserted);
        }
        self.undo_stack.push(Box::new(TextSpliceCommand::from_splice(
            self.history_target.clone(),
            offset,
            removed.to_string(),
            inserted.to_string(),
            "Edit text",
            "combo_box_text",
        )));
    }
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::ToString;

    #[test]
    fn adjacent_typing_merges_into_one_undo_step() {
        let mut editor = LineEditor::new();
        assert!(editor.insert("ab"));
        assert!(editor.insert("c"));
        assert_eq!(editor.text(), "abc");
        assert_eq!(editor.cursor(), 3);
        // Two adjacent insertions inside the merge window are one step (I-002).
        assert!(editor.undo());
        assert_eq!(editor.text(), "");
        assert!(editor.redo());
        assert_eq!(editor.text(), "abc");
    }

    #[test]
    fn typing_after_a_space_is_a_separate_undo_step() {
        let mut editor = LineEditor::new();
        editor.insert("hello");
        editor.insert(" ");
        editor.insert("world");
        // Word-boundary merge: undo removes the last word, then the space, then the first word.
        assert!(editor.undo());
        assert_eq!(editor.text(), "hello ");
        assert!(editor.undo());
        assert_eq!(editor.text(), "hello");
    }

    #[test]
    fn a_selection_is_replaced_by_the_next_insert() {
        let mut editor = LineEditor::new();
        editor.set_text("hello".to_string(), false);
        assert!(editor.set_caret(0, false));
        assert!(editor.set_caret(5, true));
        assert_eq!(editor.selection(), Some((0, 5)));
        assert!(editor.insert("bye"));
        assert_eq!(editor.text(), "bye");
        assert_eq!(editor.selection(), None);
    }

    #[test]
    fn a_programmatic_write_is_not_undoable() {
        let mut editor = LineEditor::new();
        editor.set_text("typed".to_string(), true);
        editor.set_text("assigned".to_string(), false);
        // Only the typed edit is on the stack; undoing it restores the value before it, not the
        // programmatic write the host made afterwards.
        assert_eq!(editor.text(), "assigned");
        assert!(editor.undo());
        assert_eq!(editor.text(), "");
        assert!(!editor.can_undo());
    }

    #[test]
    fn backspace_and_delete_stop_at_the_ends() {
        let mut editor = LineEditor::new();
        assert!(!editor.backspace());
        assert!(!editor.delete_forward());
        editor.set_text("ab".to_string(), false);
        assert!(editor.backspace());
        assert_eq!(editor.text(), "a");
        assert!(!editor.delete_forward());
        assert!(editor.set_caret(0, false));
        assert!(editor.delete_forward());
        assert_eq!(editor.text(), "");
    }
}
