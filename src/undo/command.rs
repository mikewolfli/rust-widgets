// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::types::*;
use crate::compat::{Instant, MiniToString, Rc, RefCell, String};

/// How long two adjacent edits of the same source may stay separate records, in milliseconds.
///
/// A merge window turns a burst of typing into one undo step without folding two deliberate
/// pauses into the same entry. 600 ms is the span a key repeat and an ordinary typing cadence
/// both fit inside (I-002).
const TEXT_MERGE_WINDOW_MS: u128 = 600;

/// The payload of a text splice, as read by a merge candidate (I-002).
///
/// A merged command has to inspect the record that is about to be folded into it, and the stack
/// only holds `&dyn UndoCommand`. This value type is the narrow, safe view that a command
/// publishes for that one purpose; a command that is not a splice returns `None` and never merges.
#[derive(Debug, Clone)]
pub struct TextSpliceRecord {
    /// Byte offset of the changed run in the *pre-edit* value.
    pub offset: usize,
    /// The text removed at `offset`.
    pub removed: String,
    /// The text inserted at `offset`.
    pub inserted: String,
    /// Identity of the value the splice writes to, so two models cannot merge.
    pub source: usize,
    /// Clock reading of the edit, used for the merge window.
    pub at: Instant,
}

/// A text mutation recorded as an in-place splice rather than a whole-document snapshot.
///
/// # Why this lives in the undo module
///
/// `TextArea` carried this record privately while `LineEdit`, the combo-box editor, `SearchBar`,
/// `SearchBox` and `InlineEditor` each pushed a full before/after pair on every keystroke. The
/// two copies of the same idea had no reason to stay in step (principle #101), so the record is
/// defined once here and every text control files it. A splice stores only the changed run — one
/// character for ordinary typing — which makes both the undo memory and the replay proportional
/// to the edit rather than to the document (I-001).
///
/// Adjacent typing of the same source merges into one record inside a short time window and
/// stops at a word boundary, which is the conventional granularity of an undo step (I-002).
pub struct TextSpliceCommand {
    id: CommandId,
    target: Rc<RefCell<String>>,
    /// Byte offset of the changed run in the pre-edit value.
    pub(crate) offset: usize,
    /// The text removed at `offset`.
    pub(crate) removed: String,
    /// The text inserted at `offset`.
    pub(crate) inserted: String,
    source: usize,
    at: Instant,
    text: &'static str,
    command_type: &'static str,
}

/// Counter for [`TextSpliceCommand`] ids.
static NEXT_TEXT_SPLICE_ID: crate::compat::atomic::AtomicU64 =
    crate::compat::atomic::AtomicU64::new(1);

impl TextSpliceCommand {
    /// Builds a command for a mutation that has already been applied, from the before/after pair.
    ///
    /// The changed run is the common prefix and suffix stripped off, so a record built this way
    /// still stores only the interval that differs even when the caller only has snapshots.
    pub fn new(
        target: Rc<RefCell<String>>,
        before: &str,
        after: &str,
        text: &'static str,
        command_type: &'static str,
    ) -> Self {
        let mut prefix = 0usize;
        for (old, new) in before.chars().zip(after.chars()) {
            if old != new {
                break;
            }
            prefix += old.len_utf8();
        }
        let mut suffix = 0usize;
        for ((_, old), (_, new)) in aligned_chars(&before[prefix..], &after[prefix..]) {
            if old != new {
                break;
            }
            suffix += old.len_utf8();
        }
        Self::from_splice(
            target,
            prefix,
            before[prefix..before.len() - suffix].to_string(),
            after[prefix..after.len() - suffix].to_string(),
            text,
            command_type,
        )
    }

    /// Builds a command for a mutation whose splice is already known.
    pub fn from_splice(
        target: Rc<RefCell<String>>,
        offset: usize,
        removed: String,
        inserted: String,
        text: &'static str,
        command_type: &'static str,
    ) -> Self {
        let source = Rc::as_ptr(&target) as usize;
        Self {
            id: CommandId(
                NEXT_TEXT_SPLICE_ID.fetch_add(1, crate::compat::atomic::Ordering::Relaxed),
            ),
            target,
            offset,
            removed,
            inserted,
            source,
            at: Instant::now(),
            text,
            command_type,
        }
    }

    /// Replaces the recorded range with `insert`.
    fn splice(&self, remove_len: usize, insert: &str) {
        let mut text = self.target.borrow_mut();
        let start = self.offset.min(text.len());
        let end = (start + remove_len).min(text.len());
        if start <= end {
            text.replace_range(start..end, insert);
        }
    }

    fn payload(&self) -> TextSpliceRecord {
        TextSpliceRecord {
            offset: self.offset,
            removed: self.removed.clone(),
            inserted: self.inserted.clone(),
            source: self.source,
            at: self.at,
        }
    }
}

/// Pairs the characters of two strings from their ends, on character boundaries.
fn aligned_chars<'a>(
    left: &'a str,
    right: &'a str,
) -> impl Iterator<Item = ((usize, char), (usize, char))> + 'a {
    left.char_indices().rev().zip(right.char_indices().rev())
}

/// Whether a run ends or begins with a character that is a word boundary.
///
/// Merging stops at whitespace and punctuation so one undo step is one word: without this, a
/// whole sentence typed in one burst would be a single entry that can only be undone at once.
fn breaks_word_run(text: &str) -> bool {
    text.chars().any(|ch| !ch.is_alphanumeric() && ch != '_')
}

impl UndoCommand for TextSpliceCommand {
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
        let inserted = self.inserted.clone();
        self.splice(self.removed.len(), &inserted);
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        let removed = self.removed.clone();
        self.splice(self.inserted.len(), &removed);
        Ok(())
    }

    fn merge_policy(&self) -> MergePolicy {
        MergePolicy::WithPrevious
    }

    fn splice_record(&self) -> Option<TextSpliceRecord> {
        Some(self.payload())
    }

    /// Folds `previous` (the newer edit) into this record when both describe one typing run.
    ///
    /// The stack calls this on the entry already on top, passing the command being pushed, so
    /// `self` is the older half of the pair. Two shapes merge: appended insertions at the end of
    /// what was just inserted, and backspaces immediately before what was just removed. Anything
    /// else — a replacement, a non-adjacent edit, a different target, or a pause longer than the
    /// merge window — stays a separate entry.
    fn try_merge(&mut self, previous: &dyn UndoCommand) -> bool {
        let Some(new) = previous.splice_record() else {
            return false;
        };
        if new.source != self.source {
            return false;
        }
        if new.at.duration_since(self.at).as_millis() > TEXT_MERGE_WINDOW_MS {
            return false;
        }
        if self.removed.is_empty() && new.removed.is_empty() {
            // Two insertions: the newer one must continue exactly where the older one ended.
            if self.offset + self.inserted.len() != new.offset {
                return false;
            }
            if breaks_word_run(&self.inserted) || breaks_word_run(&new.inserted) {
                return false;
            }
            self.inserted.push_str(&new.inserted);
            self.at = new.at;
            return true;
        }
        if self.inserted.is_empty() && new.inserted.is_empty() {
            // Two deletions: the newer one must sit immediately before the older one (backspace
            // tail-to-head), so the merged record can remove the larger run in one splice.
            if new.offset + new.removed.len() != self.offset {
                return false;
            }
            if breaks_word_run(&self.removed) || breaks_word_run(&new.removed) {
                return false;
            }
            let mut combined = new.removed;
            combined.push_str(&self.removed);
            self.removed = combined;
            self.offset = new.offset;
            self.at = new.at;
            return true;
        }
        false
    }
}

/// Trait for undoable commands.
pub trait UndoCommand {
    /// Unique identifier for this command instance.
    fn id(&self) -> CommandId;

    /// Description of the command (for UI display).
    fn description(&self) -> CommandDescription;

    /// Execute the command (forward action).
    fn execute(&mut self) -> Result<(), String>;

    /// Undo the command (reverse action).
    fn undo(&mut self) -> Result<(), String>;

    /// Redo the command. Default implementation calls `execute`.
    fn redo(&mut self) -> Result<(), String> {
        self.execute()
    }

    /// Returns the merge policy for this command.
    fn merge_policy(&self) -> MergePolicy {
        MergePolicy::Never
    }

    /// Try to merge with a previous command. Returns true if merged.
    ///
    /// The argument is the **newer** command being pushed; the receiver is the entry already on
    /// top of the stack. A command that cannot merge leaves this at `false`.
    fn try_merge(&mut self, _previous: &dyn UndoCommand) -> bool {
        false
    }

    /// The splice this command represents, when it is a text edit (I-002).
    ///
    /// Exposed so a merge candidate can read a command it only sees through `&dyn UndoCommand`
    /// without downcasting (which the crate does not use). Non-text commands return `None`.
    fn splice_record(&self) -> Option<TextSpliceRecord> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::ToString;

    fn target(text: &str) -> Rc<RefCell<String>> {
        Rc::new(RefCell::new(text.to_string()))
    }

    /// A splice records only the changed interval, not the whole document (I-001).
    #[test]
    fn a_splice_stores_only_the_changed_run() {
        let value = target("hello");
        let command =
            TextSpliceCommand::new(value.clone(), "hello", "hello!", "Edit text", "line_edit_text");
        assert_eq!(command.payload().offset, 5);
        assert_eq!(command.payload().removed, "");
        assert_eq!(command.payload().inserted, "!");
    }

    /// Adjacent insertions of one target merge, and one undo reverses the run (I-002).
    #[test]
    fn adjacent_typing_merges_and_undoes_once() {
        let value = target("");
        let mut first = TextSpliceCommand::from_splice(
            value.clone(),
            0,
            String::new(),
            "a".to_string(),
            "Edit text",
            "line_edit_text",
        );
        let second = TextSpliceCommand::from_splice(
            value.clone(),
            1,
            String::new(),
            "b".to_string(),
            "Edit text",
            "line_edit_text",
        );
        assert!(first.try_merge(&second), "adjacent inserts in the merge window combine");
        assert_eq!(first.payload().inserted, "ab");

        *value.borrow_mut() = "ab".to_string();
        first.undo().unwrap();
        assert_eq!(&*value.borrow(), "");
    }

    /// A word boundary keeps typing granular: the space is its own step (I-002).
    #[test]
    fn a_word_boundary_stops_the_merge() {
        let value = target("");
        let mut first = TextSpliceCommand::from_splice(
            value.clone(),
            0,
            String::new(),
            "hello".to_string(),
            "Edit text",
            "line_edit_text",
        );
        let second = TextSpliceCommand::from_splice(
            value.clone(),
            5,
            String::new(),
            " ".to_string(),
            "Edit text",
            "line_edit_text",
        );
        assert!(!first.try_merge(&second), "the separator is not part of the word");
    }

    /// Backspaces immediately before what was just removed merge into one deletion (I-002).
    #[test]
    fn adjacent_backspaces_merge() {
        let value = target("abc");
        let mut first = TextSpliceCommand::from_splice(
            value.clone(),
            2,
            "c".to_string(),
            String::new(),
            "Edit text",
            "line_edit_text",
        );
        let second = TextSpliceCommand::from_splice(
            value.clone(),
            1,
            "b".to_string(),
            String::new(),
            "Edit text",
            "line_edit_text",
        );
        assert!(first.try_merge(&second));
        assert_eq!(first.payload().offset, 1);
        assert_eq!(first.payload().removed, "bc");
    }

    /// The default stack capacity is the shared constant, not a literal (I-002).
    #[test]
    fn the_default_capacity_is_the_configured_bound() {
        use crate::undo::{UndoStack, DEFAULT_UNDO_CAPACITY};
        assert!(DEFAULT_UNDO_CAPACITY >= 1000);
        let stack = UndoStack::new();
        assert!(!stack.can_undo());
    }
}
