// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Character-cell geometry for the editor's fixed-pitch grid.
//!
//! # Why a char is not a column
//!
//! The editor lays text out on a grid: a caret, a selection edge or a token
//! origin sits at `column * cell_width()`. A `char` is *not* one grid column
//! though — a CJK ideograph or an emoji occupies two terminal cells, a combining
//! mark occupies none, and a tab advances to the next tab stop. Treating every
//! `char` as one cell (the previous model) drew every CJK glyph one cell wide
//! while the renderer advanced two, so each following token landed *on top of*
//! the previous one: the Chinese comments in the demo were unreadable, and a
//! caret placed after them pointed at a glyph in the middle of the line.
//!
//! `TextPosition::column` stays a **char index** — that is the unit the document
//! model, the lexer and every command speak. This module is the only place that
//! converts between those char indices and the **grid columns** the renderer
//! and the coordinate map use.
//!
//! # Fast path
//!
//! Source text is overwhelmingly ASCII with no tabs. For such a line a char
//! index, a byte offset and a grid column are the same number, and
//! [`LineGrid::is_simple`] lets every conversion return without scanning. The
//! slow path walks `char_indices()` once and allocates nothing.
//!
//! `unicode-width` supplies the East Asian Wide/Fullwidth tables (it is
//! `no_std`, so the `mini`/`embedded` profiles keep it). It is the same table
//! terminal emulators and other editors use, so the editor's idea of "wide"
//! cannot disagree with the one the reader is used to.

use unicode_width::UnicodeWidthChar;

/// Grid columns a single non-tab character occupies.
///
/// Combining marks and zero-width joiners report `0`; CJK, Hangul, fullwidth
/// forms and emoji report `2`; everything else reports `1`. A control character
/// has no printable width and is treated as zero rather than "one mystery cell".
pub(crate) fn char_columns(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// Cells a tab advances from `column`: enough to reach the next tab stop.
///
/// A tab is not a fixed width — it depends on where it starts, which is why it
/// cannot be expressed by [`char_columns`].
pub(crate) fn tab_advance(column: usize, tab_width: usize) -> usize {
    let width = tab_width.max(1);
    width - (column % width)
}

/// The grid view of one document line.
///
/// Built per line while painting, per line while hit-testing: it borrows the
/// line's `&str` and holds no data that could go stale.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineGrid<'a> {
    text: &'a str,
    tab_width: usize,
    simple: bool,
}

impl<'a> LineGrid<'a> {
    /// Builds the grid for `text` with the editor's tab width.
    pub(crate) fn new(text: &'a str, tab_width: usize) -> Self {
        // `is_ascii` is a fast scan; `contains('\t')` only matters on the hit
        // path, so it is folded into the same condition.
        let simple = text.is_ascii() && !text.as_bytes().contains(&b'\t');
        Self { text, tab_width: tab_width.max(1), simple }
    }

    /// Number of characters in the line.
    pub(crate) fn char_len(&self) -> usize {
        if self.simple {
            self.text.len()
        } else {
            self.text.chars().count()
        }
    }

    /// Grid columns occupied by the whole line.
    pub(crate) fn columns(&self) -> usize {
        if self.simple {
            self.text.len()
        } else {
            self.column_of_char(self.char_len())
        }
    }

    /// Cells the character at `column` advances the pen by.
    pub(crate) fn advance(&self, ch: char, column: usize) -> usize {
        if ch == '\t' {
            tab_advance(column, self.tab_width)
        } else {
            char_columns(ch)
        }
    }

    /// Grid column at which character `index` starts (clamped to the line).
    pub(crate) fn column_of_char(&self, index: usize) -> usize {
        if self.simple {
            return index.min(self.text.len());
        }
        let mut column = 0usize;
        for (taken, ch) in self.text.chars().enumerate() {
            if taken == index {
                return column;
            }
            column += self.advance(ch, column);
        }
        column
    }

    /// Grid column at which character `index` starts (clamped to the line).
    /// Byte offset at which character `index` starts.
    pub(crate) fn byte_of_char(&self, index: usize) -> usize {
        if self.simple {
            return index.min(self.text.len());
        }
        self.text.char_indices().nth(index).map(|(byte, _)| byte).unwrap_or(self.text.len())
    }

    /// Character index at byte offset `byte` (clamped to the line).
    pub(crate) fn char_of_byte(&self, byte: usize) -> usize {
        if self.simple {
            return byte.min(self.text.len());
        }
        self.text[..byte.min(self.text.len())].chars().count()
    }

    /// Character index whose cell contains `column`, snapping to the nearer
    /// boundary.
    ///
    /// This is the inverse of [`Self::column_of_char`] and the rule a click
    /// obeys: the right half of a cell puts the caret *after* it. A zero-width
    /// character never owns a cell, so it is never hit-tested on its own.
    pub(crate) fn char_at_column(&self, column: usize) -> usize {
        if self.simple {
            return column.min(self.text.len());
        }
        let mut start = 0usize;
        let mut index = 0usize;
        for ch in self.text.chars() {
            let width = self.advance(ch, start);
            if width == 0 {
                index += 1;
                continue;
            }
            let half = width.div_ceil(2);
            if column < start + half {
                return index;
            }
            if column < start + width {
                return index + 1;
            }
            start += width;
            index += 1;
        }
        index
    }

    /// First character boundary at or after `column`.
    ///
    /// Used for wrap boundaries: a two-cell glyph is never split across two
    /// visual rows.
    pub(crate) fn char_at_or_after_column(&self, column: usize) -> usize {
        if self.simple {
            return column.min(self.text.len());
        }
        let mut start = 0usize;
        let mut index = 0usize;
        for ch in self.text.chars() {
            let width = self.advance(ch, start);
            if start >= column {
                return index;
            }
            start += width;
            index += 1;
        }
        index
    }

    /// How many visual rows the line wraps into at `columns` grid columns.
    pub(crate) fn segments(&self, columns: usize) -> usize {
        let columns = columns.max(1);
        let width = self.columns();
        if width == 0 {
            1
        } else {
            width.div_ceil(columns)
        }
    }

    /// Character range `[start, end)` painted on visual `segment`.
    ///
    /// The boundaries are char boundaries, so the two rows never share a
    /// character and no glyph is cut.
    pub(crate) fn segment_range(&self, segment: usize, columns: usize) -> (usize, usize) {
        let columns = columns.max(1);
        if segment == 0 && self.columns() <= columns {
            return (0, self.char_len());
        }
        let start = self.char_at_or_after_column(segment.saturating_mul(columns));
        let end = self.char_at_or_after_column(segment.saturating_add(1).saturating_mul(columns));
        (start, end.max(start))
    }

    /// The visual segment that shows character `index`, and that segment's first
    /// character.
    ///
    /// The inverse of [`Self::segment_range`] for a single character. It is what
    /// puts the caret on the row where the painter actually drew the character:
    /// a caret on the fifth wrapped row was previously placed on the line's
    /// **first** row, so scrolling to the caret showed the wrong part of the line.
    ///
    /// A position exactly on a wrap boundary belongs to the row that starts
    /// there (there is no character to the left of it on the next row), and a
    /// position at the end of the line belongs to the last row.
    pub(crate) fn segment_of_char(&self, index: usize, columns: usize) -> (usize, usize) {
        let columns = columns.max(1);
        let total = self.segments(columns);
        let target = index.min(self.char_len());
        for segment in 0..total {
            let (start, end) = self.segment_range(segment, columns);
            if target < end || segment + 1 == total {
                return (segment, start);
            }
        }
        (0, 0)
    }

    /// Grid columns of the line's leading whitespace.
    ///
    /// How far a wrapped continuation row is indented. Starting every continuation
    /// at column zero (the previous behaviour) threw the code's structure away on
    /// exactly the rows a reader needs it: an `if` body wrapped mid-expression read
    /// as a new top-level statement. Zed indents continuation rows to the line's own
    /// indentation for the same reason.
    pub(crate) fn leading_indent_columns(&self) -> usize {
        let mut column = 0usize;
        for ch in self.text.chars() {
            match ch {
                ' ' => column += 1,
                '\t' => column += tab_advance(column, self.tab_width),
                _ => break,
            }
        }
        column
    }
}

/// The measured horizontal geometry of one row.
///
/// # Why the editor cannot assume one width per character
///
/// The compiled Latin face this crate ships is **Open Sans**, a proportional
/// face, and `Font::family` does not change which face paints a glyph (the
/// vector source selects by coverage). A grid built from a single cell width is
/// therefore the wrong model for what the renderer actually draws: `m` is 12 px
/// while `0` is 7 px on a 13 pt line, so tokens placed at `column * cell`
/// overlapped and gaped at once.
///
/// This type holds the *measured* x of every character boundary, built while
/// painting from the very runs that are then drawn (`shape_text` of each run), so
/// what the caret points at and what the painter put on screen are the same
/// numbers. Lines with no measurement yet (a hit-test before the first paint, or
/// a wrapped row, which keeps the grid) fall back to [`LineGrid`].
#[derive(Debug, Clone, Default)]
pub(crate) struct RowLayout {
    /// x of each character boundary; `xs[0] == 0.0`, length is `char_len + 1`.
    xs: Vec<f32>,
}

impl RowLayout {
    /// Builds a layout from the x of each boundary, in order.
    pub(crate) fn from_boundaries(mut xs: Vec<f32>) -> Self {
        if xs.is_empty() {
            xs.push(0.0);
        }
        Self { xs }
    }

    /// x of the boundary that starts character `index`.
    pub(crate) fn x_of_char(&self, index: usize) -> Option<f32> {
        self.xs.get(index).copied()
    }

    /// Nearest character boundary to `x`, snapping to the nearer one.
    ///
    /// Same rule as [`LineGrid::char_at_column`]: the right half of a glyph
    /// belongs to the boundary after it.
    pub(crate) fn char_at_x(&self, x: f32) -> Option<usize> {
        let last = self.xs.len().checked_sub(1)?;
        if !x.is_finite() {
            return None;
        }
        if x <= self.xs[0] {
            return Some(0);
        }
        if x >= self.xs[last] {
            return Some(last);
        }
        // `xs` is non-decreasing: zero-width clusters repeat a value, which is
        // why this compares neighbours instead of trusting `binary_search`'s
        // `Ok` index to be the first of a run.
        let mut low = 0usize;
        let mut high = last;
        while low + 1 < high {
            let middle = (low + high) / 2;
            if self.xs[middle] <= x {
                low = middle;
            } else {
                high = middle;
            }
        }
        let left = self.xs[low];
        let right = self.xs[high];
        if x - left < right - x {
            Some(low)
        } else {
            Some(high)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_maps_between_characters_and_pixels() {
        let layout = RowLayout::from_boundaries(vec![0.0, 7.0, 15.0, 18.0]);
        assert_eq!(layout.x_of_char(2), Some(15.0));
        assert_eq!(layout.x_of_char(4), None);
        assert_eq!(layout.char_at_x(-1.0), Some(0));
        assert_eq!(layout.char_at_x(3.0), Some(0), "the left half of a glyph is its own");
        assert_eq!(layout.char_at_x(4.0), Some(1), "the right half belongs to the next one");
        assert_eq!(layout.char_at_x(99.0), Some(3));
    }

    #[test]
    fn ascii_is_one_column_per_char() {
        let grid = LineGrid::new("let x = 1;", 4);
        assert_eq!(grid.columns(), 10);
        assert_eq!(grid.column_of_char(4), 4);
        assert_eq!(grid.char_at_column(4), 4);
        assert_eq!(grid.byte_of_char(4), 4);
    }

    #[test]
    fn cjk_is_two_columns_per_character() {
        let grid = LineGrid::new("a中b", 4);
        assert_eq!(grid.columns(), 4);
        assert_eq!(grid.column_of_char(0), 0);
        assert_eq!(grid.column_of_char(1), 1, "the CJK glyph starts after 'a'");
        assert_eq!(grid.column_of_char(2), 3, "'b' starts two cells after it");
        assert_eq!(grid.char_at_column(1), 1);
        assert_eq!(
            grid.char_at_column(2),
            2,
            "the second cell of the glyph is its right half, so the caret goes after it"
        );
        assert_eq!(grid.char_of_byte(4), 2, "byte offsets and char indices differ now");
    }

    #[test]
    fn a_segment_lookup_is_the_inverse_of_the_range() {
        let grid = LineGrid::new("0123456789", 4);
        // columns 0..4, 4..8, 8..10
        assert_eq!(grid.segment_of_char(0, 4), (0, 0));
        assert_eq!(grid.segment_of_char(3, 4), (0, 0));
        assert_eq!(grid.segment_of_char(4, 4), (1, 4), "the boundary starts the next row");
        assert_eq!(grid.segment_of_char(7, 4), (1, 4));
        assert_eq!(grid.segment_of_char(8, 4), (2, 8));
        assert_eq!(grid.segment_of_char(10, 4), (2, 8), "the end belongs to the last row");
        for index in 0..=10usize {
            let (segment, start) = grid.segment_of_char(index, 4);
            let (range_start, range_end) = grid.segment_range(segment, 4);
            assert_eq!(start, range_start);
            assert!(index >= range_start && index <= range_end);
        }
    }

    #[test]
    fn leading_whitespace_is_measured_in_columns() {
        assert_eq!(LineGrid::new("    fn x()", 4).leading_indent_columns(), 4);
        assert_eq!(LineGrid::new("\tx", 4).leading_indent_columns(), 4);
        assert_eq!(LineGrid::new("  \tx", 4).leading_indent_columns(), 4);
        assert_eq!(LineGrid::new("fn x()", 4).leading_indent_columns(), 0);
        assert_eq!(
            LineGrid::new("    ", 4).leading_indent_columns(),
            4,
            "a blank line keeps its indent"
        );
    }

    #[test]
    fn a_tab_advances_to_the_next_stop() {
        let grid = LineGrid::new("\tx", 4);
        assert_eq!(grid.column_of_char(1), 4);
        assert_eq!(grid.columns(), 5);
        assert_eq!(grid.char_at_column(2), 1, "inside the tab belongs to the next char");
    }

    #[test]
    fn wrapping_counts_cells_not_chars() {
        let grid = LineGrid::new("中中中", 4);
        assert_eq!(grid.columns(), 6);
        assert_eq!(grid.segments(4), 2);
        assert_eq!(grid.segment_range(0, 4), (0, 2));
        assert_eq!(grid.segment_range(1, 4), (2, 3));
    }

    #[test]
    fn combining_marks_do_not_consume_a_cell() {
        // "e" + U+0301 combining acute.
        let grid = LineGrid::new("e\u{301}x", 4);
        assert_eq!(grid.columns(), 2);
        assert_eq!(grid.column_of_char(2), 1);
        assert_eq!(grid.char_at_column(1), 2);
    }
}
