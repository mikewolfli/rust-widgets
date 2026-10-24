// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Line-indexed document model with tabs, folds and the splice primitive.
//!
//! The document always owns **at least one line**, even when empty. That
//! invariant is what makes every index operation in the editor total: there is
//! no line index that can be "out of range because the buffer is empty".
//!
//! The model also mirrors its text into an `Rc<RefCell<String>>` so the shared
//! `ValueCommand<String>` undo command can be reused unchanged instead of
//! introducing a second history implementation.

use super::types::{EditorBuffer, FoldRegion, LineEnding, TextPosition};
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

/// Shared editing core: line index, tabs, folds and undo targets.
#[derive(Debug)]
pub(crate) struct EditorModel {
    /// Document lines; never empty.
    pub(crate) lines: Vec<String>,
    /// Open buffers, in tab order.
    pub(crate) all_buffers: Vec<EditorBuffer>,
    /// Index of the active buffer.
    pub(crate) active_tab: usize,
    /// Fold regions of the active buffer.
    pub(crate) folds: Vec<FoldRegion>,
    /// Mirror of the active buffer used by undo commands.
    pub(crate) text: Rc<RefCell<String>>,
    /// Pristine text of the active buffer's save point.
    pub(crate) saved: String,
    /// `true` when `text` differs from `saved`.
    ///
    /// Maintained as a flag rather than derived on demand. Comparing the two
    /// strings is O(document), and the paint path asks "is this buffer dirty?"
    /// once per tab per frame — at a million lines the comparison alone measured
    /// most of a frame. Every mutation of `text` or `saved` goes through a method
    /// that updates this, so it cannot drift.
    pub(crate) dirty: bool,
    /// Byte offset at which each line starts in the joined document.
    ///
    /// A block summary tree over each line's byte length plus its newline. A
    /// prefix is the sum of whole-block totals plus the entries inside one block,
    /// and a single-line edit updates one entry. A line insertion or deletion shifts
    /// one block and updates its total, so an Enter no longer invalidates the whole
    /// table. The cache is built lazily, so a bulk write pays nothing until an
    /// offset is asked for.
    prefix_offsets: Option<PrefixIndex>,
    /// The terminator the document was loaded with.
    line_ending: LineEnding,
}

impl EditorModel {
    /// Creates a model holding one empty `untitled` buffer.
    pub(crate) fn new() -> Self {
        let mut model = Self {
            lines: vec![String::new()],
            all_buffers: Vec::new(),
            active_tab: 0,
            folds: Vec::new(),
            text: Rc::new(RefCell::new(String::new())),
            saved: String::new(),
            dirty: false,
            prefix_offsets: None,
            line_ending: LineEnding::Lf,
        };
        model.all_buffers.push(EditorBuffer::new("untitled", ""));
        model
    }

    /// Replaces the whole document, splitting it into lines.
    ///
    /// A document saved with `\r\n` terminators is normalised to `\n` here, and
    /// the original terminator is remembered so a host can save it back
    /// unchanged. Before this, every `\r` stayed at the end of its line, where it
    /// counted as a character: columns were off by one at every line end and a
    /// literal carriage return was painted into the buffer.
    pub(crate) fn set_text(&mut self, text: String) {
        if text.contains('\r') {
            self.line_ending = LineEnding::Crlf;
        }
        let text = normalize_line_endings(text);
        *self.text.borrow_mut() = text.clone();
        self.lines = split_lines(&text);
        self.lines_replaced();
        if let Some(buffer) = self.all_buffers.get_mut(self.active_tab) {
            buffer.text = text;
        }
        self.refresh_dirty();
        self.normalize_folds();
    }

    /// Returns the terminator the document was loaded with.
    pub(crate) fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Records the line terminator explicitly, for a host that changes it.
    pub(crate) fn set_line_ending(&mut self, ending: LineEnding) {
        self.line_ending = ending;
    }

    /// Marks the line index as replaced, so the prefix cache is rebuilt lazily.
    ///
    /// Called wherever `lines` is written directly rather than through
    /// [`Self::splice_with_range`]. Rebuilding lazily rather than eagerly keeps a
    /// bulk write (loading a file, a line command) from paying for offsets it may
    /// never need.
    pub(crate) fn lines_replaced(&mut self) {
        self.prefix_offsets = None;
    }

    /// Recomputes [`Self::dirty`] after a bulk change.
    ///
    /// Only called where the two strings were just written; per-keystroke paths
    /// set `dirty = true` directly, because a keystroke inside a document is a
    /// modification by definition and comparing megabytes to discover that is
    /// exactly the cost this flag exists to avoid.
    pub(crate) fn refresh_dirty(&mut self) {
        let same = {
            let text = self.text.borrow();
            text.as_str() == self.saved.as_str()
        };
        self.dirty = !same;
    }

    /// Switches the active tab, persisting the outgoing buffer's text first.
    pub(crate) fn apply_tab_switch(&mut self, tab: usize) {
        if tab >= self.all_buffers.len() {
            return;
        }
        self.persist_active();
        self.load_buffer(tab);
    }

    /// Persists the active buffer's current text and dirty flag into its slot.
    ///
    /// Used by [`Self::apply_tab_switch`] and by the editor's close path when
    /// the buffer being closed is not the active one.
    pub(crate) fn persist_active(&mut self) {
        let outgoing = self.text.borrow().clone();
        let outgoing_dirty = self.dirty;
        if let Some(active) = self.all_buffers.get_mut(self.active_tab) {
            active.modified = outgoing_dirty;
            active.text = outgoing;
        }
    }

    /// Loads buffer `tab` into the active state without persisting the outgoing
    /// one.
    ///
    /// Used by [`Self::apply_tab_switch`] and by the editor's close path, where
    /// the outgoing buffer has already been removed and must not be written back.
    pub(crate) fn load_buffer(&mut self, tab: usize) {
        let incoming = self.all_buffers[tab].text.clone();
        self.active_tab = tab;
        self.set_text(incoming);
        self.saved = self.all_buffers[tab].text.clone();
        self.dirty = self.all_buffers[tab].modified;
    }

    /// Returns the joined document text.
    #[allow(dead_code)] // used by the `cfg(test)` splice helper and the buffer tests
    pub(crate) fn full_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Returns a line, or `""` when out of range.
    pub(crate) fn line(&self, index: usize) -> &str {
        self.lines.get(index).map(|line| line.as_str()).unwrap_or("")
    }

    /// Returns the number of lines (always >= 1).
    pub(crate) fn line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    /// Returns the character length of a line.
    pub(crate) fn line_len(&self, line: usize) -> usize {
        self.line(line).chars().count()
    }

    /// Clamps a character column to the line length.
    pub(crate) fn clamp_column(&self, line: usize, column: usize) -> usize {
        column.min(self.line_len(line))
    }

    /// Clamps a position into the document.
    pub(crate) fn clamp_position(&self, position: TextPosition) -> TextPosition {
        let line = position.line.min(self.lines.len().saturating_sub(1));
        TextPosition::new(line, self.clamp_column(line, position.column))
    }

    /// Converts a character column to a byte offset, clamped to the line end.
    pub(crate) fn byte_offset(&self, line: usize, column: usize) -> usize {
        let text = self.line(line);
        let mut chars = text.char_indices();
        for _ in 0..column {
            if chars.next().is_none() {
                return text.len();
            }
        }
        chars.next().map(|(offset, _)| offset).unwrap_or(text.len())
    }

    /// Returns whether a line is hidden by a folded region.
    pub(crate) fn is_hidden(&self, line: usize) -> bool {
        self.folds
            .iter()
            .any(|region| region.folded && line > region.start_line && line <= region.end_line)
    }

    /// Returns the index of the fold region starting at `start_line`.
    pub(crate) fn fold_at(&self, start_line: usize) -> Option<usize> {
        self.folds.iter().position(|region| region.start_line == start_line)
    }

    /// Returns the number of lines currently hidden by folds.
    pub(crate) fn folded_line_count(&self) -> usize {
        self.folds.iter().map(FoldRegion::hidden_lines).sum()
    }

    /// Drops fold regions that no longer span more than one line after an edit.
    pub(crate) fn normalize_folds(&mut self) {
        let last = self.lines.len().saturating_sub(1);
        for region in &mut self.folds {
            region.start_line = region.start_line.min(last);
            region.end_line = region.end_line.clamp(region.start_line, last);
        }
        self.folds.retain(|region| region.end_line > region.start_line);
    }

    /// Returns the indices of all lines that are not hidden by a fold.
    pub(crate) fn visible_lines(&self) -> Vec<usize> {
        (0..self.lines.len()).filter(|line| !self.is_hidden(*line)).collect()
    }

    /// Extracts the text between two positions, newlines included.
    pub(crate) fn extract(&self, start: TextPosition, end: TextPosition) -> String {
        let start = self.clamp_position(start);
        let end = self.clamp_position(end);
        if start >= end {
            return String::new();
        }
        if start.line == end.line {
            let text = self.line(start.line);
            let from = self.byte_offset(start.line, start.column);
            let to = self.byte_offset(end.line, end.column);
            return text[from.min(to)..to.max(from)].to_string();
        }
        let mut result = String::new();
        let first = self.line(start.line);
        result.push_str(&first[self.byte_offset(start.line, start.column)..]);
        for line in (start.line + 1)..end.line {
            result.push('\n');
            result.push_str(self.line(line));
        }
        result.push('\n');
        let last = self.line(end.line);
        result.push_str(&last[..self.byte_offset(end.line, end.column)]);
        result
    }

    /// Replaces `[start, end)` with `replacement`, re-indexing the document.
    ///
    /// Positions are clamped, so the splice is total: it can never panic or
    /// silently drop text. Returns the resulting full document text.
    ///
    /// A thin wrapper over [`Self::splice_with_range`]; the tests use it to check
    /// the splice itself, while the widget's edit paths call the range-reporting
    /// form so the undo entry knows what changed.
    #[cfg(test)]
    pub(crate) fn splice(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        replacement: &str,
    ) -> String {
        let (rebuilt, _, _) = self.splice_with_range(start, end, replacement);
        rebuilt.unwrap_or_else(|| self.full_text())
    }

    /// Replaces `[start, end)` and reports the byte range that changed.
    ///
    /// The returned `(text, start_offset, removed_len)` is what an undo command
    /// needs: the byte range the edit replaced, in pre-edit coordinates. Deriving
    /// it here rather than recomputing it in the caller keeps the two in step —
    /// the offsets come from the same clamping the splice itself applied.
    /// `text` is `Some` only when the edit rebuilt the document: a single-line
    /// edit mutates the index in place and the mirror by splice, so the caller
    /// does not need (and must not pay for) a whole-document copy.
    ///
    /// The edit is applied **in place** when it stays inside one line, which is
    /// what typing always does. Rebuilding the document — joining every line into
    /// a string, splicing that, then splitting it back — is O(file) per keystroke;
    /// this path is O(line). Only an edit that crosses a line boundary still
    /// rebuilds, because only then do the line boundaries themselves move.
    pub(crate) fn splice_with_range(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        replacement: &str,
    ) -> (Option<String>, usize, usize) {
        let start = self.clamp_position(start);
        let end = if end < start { start } else { self.clamp_position(end) };
        let start_offset =
            self.prefix_offset(start.line) + self.byte_offset(start.line, start.column);
        let end_offset = self.prefix_offset(end.line) + self.byte_offset(end.line, end.column);

        // Single-line edit: change the line, leave the rest of the index alone.
        // A replacement containing a newline is *not* single-line even when the
        // range is, because it would split one line into several.
        if start.line == end.line && !replacement.contains('\n') {
            let from = self.byte_offset(start.line, start.column);
            let to = self.byte_offset(end.line, end.column).max(from);
            let fits =
                self.lines.get(start.line).is_some_and(|line| from <= to && to <= line.len());
            if fits {
                let removed_len = to - from;
                if let Some(line) = self.lines.get_mut(start.line) {
                    line.replace_range(from..to, replacement);
                }
                // The edit changed this line's byte length, so update the one
                // length entry. A Fenwick update refreshes every prefix that
                // depends on it in O(log n); truncating the tail instead made the
                // next offset lookup walk the document again.
                if let Some(index) = self.prefix_offsets.as_mut() {
                    let value = self.lines.get(start.line).map(|line| line.len() + 1).unwrap_or(1);
                    index.set(start.line, value);
                }
                self.dirty = true;
                self.refresh_mirror(start_offset, removed_len, replacement);
                return (None, start_offset, removed_len);
            }
        }

        // Cross-line edit: the line boundaries move, so only the affected line
        // range is rebuilt. The document is **not** joined, spliced and split
        // again: that was an O(file) copy on every Enter and every multi-line
        // paste. Here the replaced lines are rebuilt from their byte slices and
        // spliced into the line index, which is proportional to the edit.
        let first_text = self.line(start.line).to_string();
        let last_text = self.line(end.line).to_string();
        let first_byte = self.byte_offset(start.line, start.column).min(first_text.len());
        let last_byte = self.byte_offset(end.line, end.column).min(last_text.len());
        let mut merged =
            String::with_capacity(first_text.len() + replacement.len() + last_text.len());
        merged.push_str(&first_text[..first_byte]);
        merged.push_str(replacement);
        merged.push_str(&last_text[last_byte..]);
        let new_lines = split_lines(&merged);
        let new_count = new_lines.len();
        let removed_lines = end.line + 1 - start.line;
        self.lines.splice(start.line..=end.line, new_lines);
        // The index stays valid: the replaced line range is removed and the new
        // lengths are inserted into the same block summary tree. Invalidating it
        // here is what made every Enter rebuild the offsets on the next edit.
        if let Some(index) = self.prefix_offsets.as_mut() {
            let inserted: Vec<usize> = self
                .lines
                .iter()
                .skip(start.line)
                .take(new_count)
                .map(|line| line.len() + 1)
                .collect();
            index.splice(start.line, removed_lines, &inserted);
        }
        self.normalize_folds();
        self.dirty = true;
        let removed_len = end_offset.saturating_sub(start_offset);
        self.refresh_mirror(start_offset, removed_len, replacement);
        (None, start_offset, removed_len)
    }

    /// Applies the same replacement to the mirrored text.
    ///
    /// The mirror is spliced directly instead of being re-derived by joining the
    /// line index: joining is O(document), and the whole point of the in-place
    /// path is that a keystroke does not touch the whole document.
    fn refresh_mirror(&mut self, start: usize, removed_len: usize, replacement: &str) {
        let mut text = self.text.borrow_mut();
        let end = start.saturating_add(removed_len);
        if start > text.len() || end > text.len() {
            // The mirror is out of step (a caller changed the document without
            // going through the splice path); rebuild it from the index.
            let joined = self.lines.join("\n");
            *text = joined;
            return;
        }
        text.replace_range(start..end, replacement);
    }

    /// Returns the byte offset of a position in the joined document.
    ///
    /// This is the offset an undo range is expressed in: the joined text is what
    /// `text` mirrors, so a range recorded against it can be replayed directly.
    /// The position is clamped, so the result is always a valid offset. Takes
    /// `&mut self` because it fills the prefix cache on the way.
    pub(crate) fn total_byte_offset(&mut self, position: TextPosition) -> usize {
        let position = self.clamp_position(position);
        self.prefix_offset(position.line) + self.byte_offset(position.line, position.column)
    }

    /// Returns the line containing `offset` in the joined document.
    ///
    /// Clamped, so an offset past the end reports the last line rather than
    /// panicking. This is the inverse of [`Self::total_byte_offset`] for the one
    /// direction the edit paths need: a byte offset in, a line number out.
    pub(crate) fn line_of_byte_offset(&self, offset: usize) -> usize {
        let mut consumed = 0usize;
        for (index, line) in self.lines.iter().enumerate() {
            let end = consumed + line.len();
            if offset <= end {
                return index;
            }
            consumed = end + 1;
        }
        self.lines.len().saturating_sub(1)
    }

    /// Returns the byte offset where `line` starts, using the prefix cache.
    ///
    /// The prefix cache is built lazily in one forward pass and then answers
    /// every offset in O(log n).
    fn prefix_offset(&mut self, line: usize) -> usize {
        if self.prefix_offsets.is_none() {
            let lengths = self.lines.iter().map(|line| line.len() + 1);
            self.prefix_offsets = Some(PrefixIndex::from_lengths(lengths));
        }
        self.prefix_offsets.as_ref().map(|index| index.prefix(line)).unwrap_or(0)
    }
}

/// A block summary tree over per-line byte lengths.
///
/// The prefix table used to be a `Vec` truncated at the edited line, and then a
/// Fenwick tree that could update one entry but had to be rebuilt when a line was
/// inserted or deleted. This form keeps both properties: a point edit changes one
/// entry, and an insertion/deletion shifts one small block and adjusts that
/// block's total. A prefix sums whole blocks plus one intra-block slice, so the
/// cost is `O(block + blocks)` and never a full document walk unless the index is
/// being built after a bulk load.
#[derive(Debug)]
struct PrefixIndex {
    /// Length entries, in line order, split into fixed-size blocks.
    blocks: Vec<Vec<usize>>,
    /// Per-block sums, parallel to `blocks`.
    totals: Vec<usize>,
    /// Maximum entries per block.
    block_cap: usize,
}

impl PrefixIndex {
    fn from_lengths(lengths: impl Iterator<Item = usize>) -> Self {
        let values: Vec<usize> = lengths.collect();
        let block_cap = 128usize;
        let mut blocks: Vec<Vec<usize>> = Vec::new();
        for chunk in values.chunks(block_cap) {
            blocks.push(chunk.to_vec());
        }
        if blocks.is_empty() {
            blocks.push(Vec::new());
        }
        let totals = blocks.iter().map(|block| block.iter().sum()).collect();
        Self { blocks, totals, block_cap }
    }

    /// Returns the sum of the first `index` entries.
    fn prefix(&self, index: usize) -> usize {
        let mut remaining = index;
        let mut sum = 0usize;
        for (block_index, block) in self.blocks.iter().enumerate() {
            if remaining >= block.len() {
                sum += self.totals[block_index];
                remaining -= block.len();
            } else {
                sum += block[..remaining].iter().sum::<usize>();
                break;
            }
        }
        sum
    }

    /// Locates the block and offset containing logical index `index`.
    fn locate(&self, index: usize) -> (usize, usize) {
        let mut remaining = index;
        for (block_index, block) in self.blocks.iter().enumerate() {
            if remaining < block.len() {
                return (block_index, remaining);
            }
            remaining -= block.len();
        }
        let last = self.blocks.len().saturating_sub(1);
        (last, self.blocks.get(last).map(Vec::len).unwrap_or(0))
    }

    /// Replaces one entry and refreshes its block total.
    fn set(&mut self, index: usize, value: usize) {
        let (block_index, offset) = self.locate(index);
        let Some(slot) = self.blocks.get_mut(block_index).and_then(|block| block.get_mut(offset))
        else {
            return;
        };
        if *slot == value {
            return;
        }
        *slot = value;
        self.totals[block_index] = self.blocks[block_index].iter().sum();
    }

    /// Inserts one entry at `index`, splitting a full block.
    fn insert(&mut self, index: usize, value: usize) {
        let (block_index, offset) = self.locate(index);
        let block = &mut self.blocks[block_index];
        let offset = offset.min(block.len());
        block.insert(offset, value);
        self.totals[block_index] += value;
        if block.len() > self.block_cap {
            let tail = block.split_off(block.len() / 2);
            let tail_total: usize = tail.iter().sum();
            self.blocks.insert(block_index + 1, tail);
            self.totals[block_index] = self.blocks[block_index].iter().sum();
            self.totals.insert(block_index + 1, tail_total);
        }
    }

    /// Removes the entry at `index`.
    fn remove(&mut self, index: usize) {
        let (block_index, offset) = self.locate(index);
        let Some(block) = self.blocks.get_mut(block_index) else { return };
        if offset >= block.len() {
            return;
        }
        let removed = block.remove(offset);
        self.totals[block_index] -= removed;
        if block.is_empty() && self.blocks.len() > 1 {
            self.blocks.remove(block_index);
            self.totals.remove(block_index);
        }
    }

    /// Replaces `removed` entries at `index` with `values`.
    fn splice(&mut self, index: usize, removed: usize, values: &[usize]) {
        for _ in 0..removed {
            self.remove(index);
        }
        for (offset, value) in values.iter().enumerate() {
            self.insert(index + offset, *value);
        }
    }
}

/// Converts every terminator in `text` to `\n`.
///
/// Lone `\r` (old Macintosh convention) is normalised too, so no path can end
/// up with a carriage return inside a line.
pub(crate) fn normalize_line_endings(text: String) -> String {
    if !text.contains('\r') {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Splits a document into lines, always yielding at least one line.
pub(crate) fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    text.split('\n').map(|line| line.to_string()).collect()
}

/// Joins lines back into a document.
pub(crate) fn join_lines(lines: &[String]) -> String {
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_owns_exactly_one_line() {
        let model = EditorModel::new();
        assert_eq!(model.line_count(), 1);
        assert_eq!(model.line(0), "");
        assert_eq!(model.full_text(), "");
    }

    #[test]
    fn set_text_indexes_lines_including_trailing_newline() {
        let mut model = EditorModel::new();
        model.set_text("a\nb\n".to_string());
        assert_eq!(model.lines, vec!["a".to_string(), "b".to_string(), String::new()]);
        assert_eq!(model.full_text(), "a\nb\n");
    }

    #[test]
    fn byte_offset_and_clamping_are_character_aware() {
        let mut model = EditorModel::new();
        model.set_text("héllo".to_string());
        assert_eq!(model.line_len(0), 5);
        // 'é' is two bytes, so column 2 is byte offset 3.
        assert_eq!(model.byte_offset(0, 2), 3);
        assert_eq!(model.byte_offset(0, 999), "héllo".len());
        assert_eq!(model.clamp_column(0, 999), 5);
    }

    #[test]
    fn splice_mid_line_replaces_exact_range() {
        let mut model = EditorModel::new();
        model.set_text("hello world".to_string());
        let result = model.splice(TextPosition::new(0, 6), TextPosition::new(0, 11), "there");
        assert_eq!(result, "hello there");
    }

    #[test]
    fn splice_across_lines_joins_them() {
        let mut model = EditorModel::new();
        model.set_text("one\ntwo\nthree".to_string());
        let result = model.splice(TextPosition::new(0, 3), TextPosition::new(1, 3), "\n");
        assert_eq!(result, "one\n\nthree");
    }

    #[test]
    fn splice_removing_a_newline_merges_lines() {
        let mut model = EditorModel::new();
        model.set_text("ab\ncd".to_string());
        let result = model.splice(TextPosition::new(0, 2), TextPosition::new(1, 0), "");
        assert_eq!(result, "abcd");
        assert_eq!(model.lines, vec!["abcd".to_string()]);
    }

    #[test]
    fn splice_with_reversed_range_inserts_instead_of_panicking() {
        let mut model = EditorModel::new();
        model.set_text("abc".to_string());
        let result = model.splice(TextPosition::new(0, 3), TextPosition::new(0, 0), "X");
        assert_eq!(result, "abcX");
    }

    #[test]
    fn splice_handles_out_of_range_positions() {
        let mut model = EditorModel::new();
        model.set_text("abc".to_string());
        let result = model.splice(TextPosition::new(99, 99), TextPosition::new(99, 99), "!");
        assert_eq!(result, "abc!");
    }

    #[test]
    fn extract_spans_lines_and_is_empty_for_collapsed_range() {
        let mut model = EditorModel::new();
        model.set_text("alpha\nbeta\ngamma".to_string());
        assert_eq!(model.extract(TextPosition::new(0, 0), TextPosition::new(0, 0)), "");
        assert_eq!(
            model.extract(TextPosition::new(0, 0), TextPosition::new(2, 2)),
            "alpha\nbeta\nga"
        );
        assert_eq!(model.extract(TextPosition::new(1, 1), TextPosition::new(1, 3)), "et");
    }

    #[test]
    fn folds_hide_only_interior_lines_and_normalize_after_edits() {
        let mut model = EditorModel::new();
        model.set_text("a\nb\nc\nd".to_string());
        model.folds.push(FoldRegion::new(1, 2, true));
        assert!(!model.is_hidden(0));
        assert!(!model.is_hidden(1), "the fold header stays visible");
        assert!(model.is_hidden(2));
        assert!(!model.is_hidden(3));
        assert_eq!(model.folded_line_count(), 1);
        assert_eq!(model.visible_lines(), vec![0, 1, 3]);

        // Shrinking the document must clamp the region, not panic.
        model.set_text("only".to_string());
        assert!(model.folds.is_empty());
    }

    #[test]
    fn tab_switch_persists_outgoing_buffer_text() {
        let mut model = EditorModel::new();
        model.set_text("alpha".to_string());
        model.all_buffers.push(EditorBuffer::new("second", "beta"));
        model.apply_tab_switch(1);
        assert_eq!(model.full_text(), "beta");
        assert_eq!(model.active_tab, 1);
        assert_eq!(model.all_buffers[0].text, "alpha");

        model.apply_tab_switch(0);
        assert_eq!(model.full_text(), "alpha");
    }

    #[test]
    fn tab_switch_to_invalid_index_is_a_noop() {
        let mut model = EditorModel::new();
        model.set_text("alpha".to_string());
        model.apply_tab_switch(7);
        assert_eq!(model.active_tab, 0);
        assert_eq!(model.full_text(), "alpha");
    }

    #[test]
    fn join_lines_round_trips_with_split_lines() {
        let text = "a\n\nb\n";
        assert_eq!(join_lines(&split_lines(text)), text);
    }

    /// The prefix index answers offsets after a single-line edit without rebuilding.
    #[test]
    fn the_prefix_index_tracks_single_line_edits() {
        let mut model = EditorModel::new();
        model.set_text("aa\nbb\ncc".to_string());
        // Force the index to be built, then edit the first line.
        assert_eq!(model.total_byte_offset(TextPosition::new(2, 1)), 7);
        model.splice_with_range(TextPosition::new(0, 1), TextPosition::new(0, 1), "XYZ");
        // "aXYZa\nbb\ncc": line 2 now starts at 9.
        assert_eq!(model.total_byte_offset(TextPosition::new(2, 1)), 10);
        model.splice_with_range(TextPosition::new(0, 0), TextPosition::new(0, 1), "");
        // "XYZa\nbb\ncc": line 2 now starts at 8.
        assert_eq!(model.total_byte_offset(TextPosition::new(2, 2)), 10);
        assert_eq!(model.line(2), "cc");
    }

    /// A cross-line splice rebuilds only the affected lines and keeps the mirror.
    #[test]
    fn a_cross_line_splice_keeps_the_mirror_in_sync() {
        let mut model = EditorModel::new();
        model.set_text("alpha\nbeta\ngamma".to_string());
        let (rebuilt, start, removed) =
            model.splice_with_range(TextPosition::new(1, 2), TextPosition::new(1, 2), "\nnew");
        assert!(rebuilt.is_none(), "the document is spliced in place");
        assert_eq!(start, 8);
        assert_eq!(removed, 0);
        assert_eq!(model.full_text(), "alpha\nbe\nnewta\ngamma");
        assert_eq!(*model.text.borrow(), "alpha\nbe\nnewta\ngamma");
    }

    /// Line endings are normalised once and the original form is remembered.
    #[test]
    fn crlf_is_normalised_on_load() {
        let mut model = EditorModel::new();
        model.set_text("one\r\ntwo\r\n".to_string());
        assert_eq!(model.line(0), "one");
        assert_eq!(model.line(1), "two");
        assert_eq!(model.line_ending(), LineEnding::Crlf);
        assert_eq!(model.full_text(), "one\ntwo\n");
    }
}
