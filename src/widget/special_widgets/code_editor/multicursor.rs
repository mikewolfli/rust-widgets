//! Multiple carets: an ordered, deduplicated caret set plus the edit-projection
//! helper that applies one logical edit at every caret in a single pass.
//!
//! Zed's signature editing model is "one command, many carets". A single widget
//! can own that: the caret set, the merge/deduplicate rules, and the pure
//! function that maps a batch of replacements onto the document text are all
//! local state. Everything above it (LSP rename-all, AI multi-edit) is a plugin
//! concern and stays out of this module.

use super::types::{Cursor, TextPosition};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// An ordered set of carets. The primary caret is the one the status bar and
/// single-caret APIs report; secondary carets are all the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiCursor {
    /// Every caret, sorted by position, primary included.
    carets: Vec<Cursor>,
    /// The designated primary caret; always present in `carets`.
    primary: Cursor,
}

impl Default for MultiCursor {
    fn default() -> Self {
        Self::new(Cursor::default())
    }
}

impl MultiCursor {
    /// Creates a set holding only `primary`.
    pub fn new(primary: Cursor) -> Self {
        Self { carets: vec![primary], primary }
    }

    /// Creates a set from a primary caret and any number of extras.
    pub fn from_parts(primary: Cursor, extras: &[Cursor]) -> Self {
        let mut set = Self { carets: Vec::new(), primary };
        set.carets.push(primary);
        for caret in extras {
            set.carets.push(*caret);
        }
        set.normalize();
        if !set.carets.contains(&primary) {
            set.carets.push(primary);
            set.normalize();
        }
        set
    }

    /// Returns every caret, sorted by position.
    pub fn carets(&self) -> &[Cursor] {
        &self.carets
    }

    /// Returns the primary caret.
    pub fn primary(&self) -> Cursor {
        self.primary
    }

    /// Returns every caret except the primary one, in sorted order.
    pub fn secondaries(&self) -> Vec<Cursor> {
        self.carets.iter().copied().filter(|caret| *caret != self.primary).collect()
    }

    /// Returns the number of carets.
    pub fn len(&self) -> usize {
        self.carets.len()
    }

    /// Returns `true` when there are no carets (never for a live editor).
    pub fn is_empty(&self) -> bool {
        self.carets.is_empty()
    }

    /// Returns `true` when more than one caret is present.
    pub fn has_multiple(&self) -> bool {
        self.carets.len() > 1
    }

    /// Iterates over the carets in sorted order.
    pub fn iter(&self) -> core::slice::Iter<'_, Cursor> {
        self.carets.iter()
    }

    /// Adds a caret, keeping the set sorted and free of duplicates.
    ///
    /// Returns `true` when the set grew.
    pub fn push(&mut self, caret: Cursor) -> bool {
        if self.carets.contains(&caret) {
            return false;
        }
        self.carets.push(caret);
        self.normalize();
        true
    }

    /// Removes every caret except the primary one.
    pub fn clear_secondaries(&mut self) {
        self.carets.retain(|caret| *caret == self.primary);
    }

    /// Replaces the primary caret, keeping it inside the set.
    pub fn set_primary(&mut self, caret: Cursor) {
        self.carets.retain(|existing| *existing != self.primary);
        self.carets.push(caret);
        self.primary = caret;
        self.normalize();
    }

    /// Sorts by `(line, column, anchor)` and drops exact duplicates.
    fn normalize(&mut self) {
        self.carets.sort_by(|a, b| {
            a.head
                .line
                .cmp(&b.head.line)
                .then_with(|| a.head.column.cmp(&b.head.column))
                .then_with(|| a.anchor.line.cmp(&b.anchor.line))
                .then_with(|| a.anchor.column.cmp(&b.anchor.column))
        });
        self.carets.dedup();
    }
}

/// One replacement expressed in absolute character offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OffsetEdit {
    /// Inclusive start offset.
    pub start: usize,
    /// Exclusive end offset.
    pub end: usize,
    /// Replacement text.
    pub insert: String,
}

/// Converts a line/column position into an absolute character offset.
pub(crate) fn char_offset(lines: &[String], position: TextPosition) -> usize {
    let mut offset = 0usize;
    for line in 0..position.line.min(lines.len()) {
        offset += lines.get(line).map(|text| text.chars().count()).unwrap_or(0) + 1;
    }
    let line_len = lines.get(position.line).map(|text| text.chars().count()).unwrap_or(0);
    offset + position.column.min(line_len)
}

/// Converts an absolute character offset back into a line/column position.
pub(crate) fn position_at_offset(lines: &[String], offset: usize) -> TextPosition {
    let mut remaining = offset;
    for (index, line) in lines.iter().enumerate() {
        let length = line.chars().count();
        if remaining <= length {
            return TextPosition::new(index, remaining);
        }
        remaining -= length + 1;
    }
    let last = lines.len().saturating_sub(1);
    TextPosition::new(last, lines.get(last).map(|line| line.chars().count()).unwrap_or(0))
}

/// Applies a batch of edits in a single pass over `text`.
///
/// Edits are sorted by start offset and applied left-to-right while counting the
/// cumulative length delta, so every caret ends up correct even when earlier
/// edits changed the document length. Overlapping edits are dropped (the
/// earliest wins).
///
/// Returns the new text and, per input edit, the absolute offset where its caret
/// should land (`None` when the edit was dropped).
pub(crate) fn project(text: &str, edits: &[OffsetEdit]) -> (String, Vec<Option<usize>>) {
    let chars: Vec<char> = text.chars().collect();
    let mut order: Vec<usize> = (0..edits.len()).collect();
    order.sort_by_key(|&index| (edits[index].start, edits[index].end));

    let mut result = String::with_capacity(text.len());
    let mut carets: Vec<Option<usize>> = vec![None; edits.len()];
    let mut cursor = 0usize;
    let mut delta: isize = 0;

    for &index in &order {
        let edit = &edits[index];
        let start = edit.start.min(chars.len());
        let end = edit.end.min(chars.len()).max(start);
        if start < cursor {
            // Overlaps an edit already applied; skip it.
            continue;
        }
        result.extend(chars[cursor..start].iter());
        let new_start = (start as isize + delta).max(0) as usize;
        let insert_len = edit.insert.chars().count();
        result.push_str(&edit.insert);
        carets[index] = Some(new_start + insert_len);
        delta += insert_len as isize - (end - start) as isize;
        cursor = end;
    }

    result.extend(chars[cursor.min(chars.len())..].iter());
    (result, carets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caret(line: usize, column: usize) -> Cursor {
        Cursor { head: TextPosition::new(line, column), anchor: TextPosition::new(line, column) }
    }

    #[test]
    fn a_fresh_set_holds_only_the_primary() {
        let set = MultiCursor::new(caret(1, 2));
        assert_eq!(set.len(), 1);
        assert!(!set.has_multiple());
        assert_eq!(set.primary(), caret(1, 2));
    }

    #[test]
    fn push_keeps_carets_sorted_and_unique() {
        let mut set = MultiCursor::new(caret(2, 0));
        assert!(set.push(caret(0, 4)));
        assert!(!set.push(caret(0, 4)), "duplicates are rejected");
        assert!(set.push(caret(1, 1)));
        let heads: Vec<(usize, usize)> =
            set.carets().iter().map(|c| (c.head.line, c.head.column)).collect();
        assert_eq!(heads, vec![(0, 4), (1, 1), (2, 0)]);
        assert_eq!(set.len(), 3);
        assert!(set.has_multiple());
        assert_eq!(set.primary().head.line, 2);
    }

    #[test]
    fn clear_secondaries_keeps_the_primary() {
        let mut set = MultiCursor::from_parts(caret(1, 0), &[caret(0, 0), caret(2, 0)]);
        assert_eq!(set.len(), 3);
        set.clear_secondaries();
        assert_eq!(set.len(), 1);
        assert_eq!(set.primary(), caret(1, 0));
    }

    #[test]
    fn char_offsets_round_trip_across_lines() {
        let lines: Vec<String> = vec![String::from("abc"), String::from("de"), String::from("")];
        assert_eq!(char_offset(&lines, TextPosition::new(0, 1)), 1);
        assert_eq!(char_offset(&lines, TextPosition::new(1, 0)), 4);
        assert_eq!(char_offset(&lines, TextPosition::new(2, 0)), 7);
        assert_eq!(position_at_offset(&lines, 4), TextPosition::new(1, 0));
        assert_eq!(position_at_offset(&lines, 5), TextPosition::new(1, 1));
        assert_eq!(position_at_offset(&lines, 7), TextPosition::new(2, 0));
    }

    #[test]
    fn project_inserts_at_every_caret_and_reports_new_offsets() {
        let edits = vec![
            OffsetEdit { start: 0, end: 0, insert: String::from("X") },
            OffsetEdit { start: 2, end: 2, insert: String::from("Y") },
        ];
        let (text, carets) = project("abc", &edits);
        assert_eq!(text, "XabYc");
        assert_eq!(carets, vec![Some(1), Some(4)]);
    }

    #[test]
    fn project_deletes_ranges_from_the_right() {
        let edits = vec![
            OffsetEdit { start: 0, end: 1, insert: String::new() },
            OffsetEdit { start: 4, end: 5, insert: String::new() },
        ];
        let (text, carets) = project("aXbYc", &edits);
        assert_eq!(text, "XbY");
        assert_eq!(carets, vec![Some(0), Some(3)]);
    }

    #[test]
    fn project_drops_overlapping_edits() {
        let edits = vec![
            OffsetEdit { start: 0, end: 3, insert: String::from("Z") },
            OffsetEdit { start: 1, end: 2, insert: String::from("!") },
        ];
        let (text, carets) = project("abcd", &edits);
        assert_eq!(text, "Zd");
        assert_eq!(carets[0], Some(1));
        assert_eq!(carets[1], None, "the overlapping edit is dropped");
    }

    #[test]
    fn project_clamps_out_of_range_offsets() {
        let edits = vec![OffsetEdit { start: 99, end: 99, insert: String::from("!") }];
        let (text, carets) = project("ab", &edits);
        assert_eq!(text, "ab!");
        assert_eq!(carets, vec![Some(3)]);
    }
}
