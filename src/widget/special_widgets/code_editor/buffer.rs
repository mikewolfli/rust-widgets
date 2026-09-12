//! Line-indexed document model with tabs, folds and the splice primitive.
//!
//! The document always owns **at least one line**, even when empty. That
//! invariant is what makes every index operation in the editor total: there is
//! no line index that can be "out of range because the buffer is empty".
//!
//! The model also mirrors its text into an `Rc<RefCell<String>>` so the shared
//! [`TextSnapshotCommand`] undo command can be reused unchanged instead of
//! introducing a second history implementation.

use super::types::{EditorBuffer, FoldRegion, TextPosition};
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
        };
        model.all_buffers.push(EditorBuffer::new("untitled", ""));
        model
    }

    /// Replaces the whole document, splitting it into lines.
    pub(crate) fn set_text(&mut self, text: String) {
        *self.text.borrow_mut() = text.clone();
        self.lines = split_lines(&text);
        if let Some(buffer) = self.all_buffers.get_mut(self.active_tab) {
            buffer.text = text;
        }
        self.normalize_folds();
    }

    /// Switches the active tab, persisting the outgoing buffer's text first.
    pub(crate) fn apply_tab_switch(&mut self, tab: usize) {
        if tab >= self.all_buffers.len() {
            return;
        }
        let outgoing = self.text.borrow().clone();
        if let Some(active) = self.all_buffers.get_mut(self.active_tab) {
            active.modified = self.saved != outgoing;
            active.text = outgoing;
        }
        let incoming = self.all_buffers[tab].text.clone();
        self.active_tab = tab;
        self.set_text(incoming);
        self.saved = self.all_buffers[tab].text.clone();
    }

    /// Returns the joined document text.
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
    pub(crate) fn splice(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        replacement: &str,
    ) -> String {
        let before = self.full_text();
        let start = self.clamp_position(start);
        let end = if end < start { start } else { self.clamp_position(end) };
        let start_offset =
            self.line_offset(start.line) + self.byte_offset(start.line, start.column);
        let end_offset = self.line_offset(end.line) + self.byte_offset(end.line, end.column);
        let mut result = String::with_capacity(before.len() + replacement.len());
        result.push_str(&before[..start_offset]);
        result.push_str(replacement);
        result.push_str(&before[end_offset.min(before.len())..]);
        self.set_text(result.clone());
        result
    }

    /// Returns the byte offset where `line` starts in the joined document.
    fn line_offset(&self, line: usize) -> usize {
        let mut offset = 0usize;
        for index in 0..line {
            offset += self.lines.get(index).map(|l| l.len()).unwrap_or(0) + 1;
        }
        offset
    }
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
}
