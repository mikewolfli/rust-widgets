// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Markdown document model: text-delta history, outline entries and line tables.

use crate::undo::{CommandDescription, CommandId, UndoCommand};
use std::cell::RefCell;
use std::rc::Rc;

/// The top of the first text row below the header band.
pub(crate) const TEXT_TOP: i32 = 36;
/// The pitch between text rows.
pub(crate) const TEXT_ROW_HEIGHT: i32 = 16;
/// The bottom inset a row has to clear to be painted.
pub(crate) const TEXT_BOTTOM_INSET: i32 = 8;

/// The interval one edit replaced, and the document length after it.
///
/// # Why the signal carries this instead of the whole document
///
/// `text_changed` used to clone the entire document on every keystroke, so a 200 KB note paid
/// 200 KB per character even when one character was inserted (SP-001). The payload names the
/// changed run instead: `before[..offset]` and `after[offset + removed.len()..]` are unchanged,
/// and a subscriber that needs the new document applies this to the copy it already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownTextChange {
    /// Byte offset where the replaced run began, on a character boundary.
    pub offset: usize,
    /// The text removed at `offset`; empty for an insertion.
    pub removed: String,
    /// The text inserted at `offset`; empty for a deletion.
    pub inserted: String,
    /// The document's byte length after the edit.
    pub len: usize,
}

/// The minimal run that differs between `before` and `after`, on character boundaries.
///
/// Returns `(offset, removed, inserted)`. One implementation feeds both the undo command and the
/// `text_changed` payload, so the two cannot disagree about which run changed.
pub(crate) fn changed_run<'a>(before: &'a str, after: &'a str) -> (usize, &'a str, &'a str) {
    let before_bytes = before.as_bytes();
    let after_bytes = after.as_bytes();
    let max_prefix: usize = before_bytes.len().min(after_bytes.len());
    let mut prefix: usize = 0;
    while prefix < max_prefix && before_bytes[prefix] == after_bytes[prefix] {
        prefix += 1;
    }
    // Both strings share this prefix, so a boundary in `before` is one in `after` too.
    while prefix > 0 && !before.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix: usize = before_bytes.len().min(after_bytes.len()) - prefix;
    let mut suffix: usize = 0;
    while suffix < max_suffix
        && before_bytes[before_bytes.len() - 1 - suffix]
            == after_bytes[after_bytes.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0 && !before.is_char_boundary(before.len() - suffix) {
        suffix -= 1;
    }
    (prefix, &before[prefix..before.len() - suffix], &after[prefix..after.len() - suffix])
}

/// An undo step that records only the bytes that differ between two document states.
///
/// A whole-document snapshot would store both versions on every keystroke. The common
/// prefix and suffix are found on character boundaries, so the command keeps only the
/// changed run and an offset, and an undo touches O(edit) bytes instead of O(document).
pub(crate) struct MarkdownTextDeltaCommand {
    id: CommandId,
    target: Rc<RefCell<String>>,
    offset: usize,
    removed: String,
    inserted: String,
}

impl MarkdownTextDeltaCommand {
    /// Builds a command for a mutation that has already been applied.
    pub(crate) fn new(target: Rc<RefCell<String>>, before: &str, after: &str) -> Self {
        let (offset, removed, inserted) = changed_run(before, after);
        Self {
            id: CommandId(
                NEXT_MARKDOWN_COMMAND_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            ),
            target,
            offset,
            removed: removed.to_string(),
            inserted: inserted.to_string(),
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
}

/// Counter for [`MarkdownTextDeltaCommand`] ids.
static NEXT_MARKDOWN_COMMAND_ID: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(1);

impl UndoCommand for MarkdownTextDeltaCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit text".to_string(),
            timestamp_ms: 0,
            command_type: "markdown_editor_text",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        self.splice(self.removed.len(), &self.inserted.clone());
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        self.splice(self.inserted.len(), &self.removed.clone());
        Ok(())
    }
}

/// One heading in the document outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownOutlineEntry {
    /// Zero-based line index.
    pub line: usize,
    /// Heading level, `1..=6`.
    pub level: u8,
    /// Heading text without its `#` markers.
    pub title: String,
}

/// An IME composition that has not been committed yet.
#[derive(Debug, Clone, Default)]
pub(crate) struct Preedit {
    pub(crate) text: String,
    /// Byte offset of the insertion point inside `text`.
    pub(crate) cursor: usize,
}

/// Document statistics, cached per revision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Metrics {
    pub(crate) lines: usize,
    pub(crate) words: usize,
    pub(crate) headings: usize,
}

/// Per-line offset tables derived from one revision of the document.
///
/// Every caret query used to rescan the document from its first byte, and each keystroke
/// rebuilt the whole document as a character vector. Both are O(document) per event. With the
/// tables below a line lookup is a binary search and a character offset is resolved inside one
/// line, so the work is bounded by the line's own length instead of the document's.
#[derive(Debug, Default)]
pub(crate) struct LineIndex {
    /// Byte offset of the first character of every line.
    pub(crate) starts: Vec<usize>,
    /// Character offset of the first character of every line.
    pub(crate) char_starts: Vec<usize>,
    /// Character length of every line, excluding its `\n`.
    pub(crate) char_lens: Vec<usize>,
}

impl LineIndex {
    /// Builds the tables from `text` in one pass.
    pub(crate) fn new(text: &str) -> Self {
        let mut starts = Vec::new();
        let mut char_starts = Vec::new();
        let mut char_lens = Vec::new();
        let mut byte = 0usize;
        let mut chars = 0usize;
        for line in text.split('\n') {
            starts.push(byte);
            char_starts.push(chars);
            let length = line.chars().count();
            char_lens.push(length);
            byte += line.len() + 1;
            chars += length + 1;
        }
        Self { starts, char_starts, char_lens }
    }

    /// The index of the line that contains `offset`, counted in characters.
    pub(crate) fn line_of_char(&self, offset: usize) -> usize {
        // `char_starts` is ascending, so the last entry that starts at or before `offset` is the
        // containing line. A `\n` position belongs to the line it terminates.
        self.char_starts.partition_point(|start| *start <= offset).saturating_sub(1)
    }

    /// The number of characters in the document, including the `\n` separators.
    pub(crate) fn char_len(&self) -> usize {
        match (self.char_starts.last(), self.char_lens.last()) {
            (Some(start), Some(length)) => start + length,
            _ => 0,
        }
    }

    /// The byte span `[start, end)` of `line` in a document of `text_len` bytes.
    ///
    /// `None` when `line` is past the end. Callers slice the document with this instead of
    /// rescanning it, which is what keeps hit-testing and caret placement on the same table.
    pub(crate) fn line_span(&self, line: usize, text_len: usize) -> Option<(usize, usize)> {
        let start = self.starts.get(line).copied()?.min(text_len);
        let end = match self.starts.get(line + 1) {
            Some(next) => next.saturating_sub(1).min(text_len),
            None => text_len,
        };
        Some((start, end.max(start)))
    }
}

/// The largest character boundary at or below `offset`.
///
/// A platform-supplied byte offset is usually a boundary, but slicing on one that is not would
/// panic inside a draw/hit-test path; clamping is the only behaviour that cannot crash.
pub(crate) fn char_boundary_at(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    (0..=offset).rev().find(|index| text.is_char_boundary(*index)).unwrap_or(0)
}

/// The byte offset of character `index` in `text`, clamped to its end.
///
/// Used by the paint path to measure a prefix without materialising it as a `String`.
pub(crate) fn byte_offset_of_char(text: &str, index: usize) -> usize {
    text.char_indices().nth(index).map(|(byte, _)| byte).unwrap_or(text.len())
}
