// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Caret geometry and the IME composition overlay.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

impl CodeEditor {
    /// The x of a document position inside its visual row, and the row's segment
    /// index, for a row that has not been painted (or a wrapped one, which is never
    /// measured per character).
    ///
    /// Wrapped rows need two corrections the grid alone cannot express: the row
    /// starts at its segment's first character, and a continuation row is indented.
    /// Without them the caret on the second segment pointed at `column * cell` from
    /// the left edge of the *row*, which is where a character from the first segment
    /// was drawn.
    pub(super) fn unwrapped_x(&self, line: usize, column: usize) -> (usize, f32) {
        let text = self.line_text(line).unwrap_or_default();
        let grid = LineGrid::new(&text, self.config.tab_width);
        let cell = self.cell_width();
        if !self.config.word_wrap {
            return (0, grid.column_of_char(column) as f32 * cell);
        }
        let columns = self.text_columns().max(1);
        let (segment, segment_start) = grid.segment_of_char(column, columns);
        let first = grid.column_of_char(segment_start);
        let indent = if segment == 0 {
            0.0
        } else {
            let indent = grid.leading_indent_columns() as f32 * cell;
            indent.min(columns.saturating_sub(4) as f32 * cell)
        };
        (segment, indent + grid.column_of_char(column).saturating_sub(first) as f32 * cell)
    }

    /// The visual row the caret is painted on, including its wrap segment.
    pub(crate) fn caret_visual_row(&self) -> usize {
        self.visual_row_of(self.cursor.head).unwrap_or(0)
    }

    /// The visual row a document position is painted on, or `None` when the position is
    /// outside the document.
    ///
    /// The line's first row, plus the position's wrap segment. It is the row the
    /// selection rectangle and the caret both need, and deriving it from the shared
    /// `unwrapped_x` keeps it consistent with the x those two get.
    ///
    /// # Cost
    ///
    /// The line's start row comes from the row-prefix table (an `O(1)` index) and the wrap
    /// segment is measured from that line's text alone, so the lookup does **not** grow with
    /// the number of document lines. That is what makes it usable on the cursor and per-frame
    /// paths, unlike [`Self::visual_lines`], which materialises the whole document.
    ///
    /// A position inside a folded region reports the row of the next visible line, because a
    /// folded line contributes no rows of its own. Only a line index past the end returns
    /// `None`; an over-long column is clamped to the line end.
    pub fn visual_row_of(&self, position: TextPosition) -> Option<usize> {
        if position.line >= self.line_count() {
            return None;
        }
        Some(self.visual_row_index(position))
    }

    /// The visual row containing byte `offset` in the joined document.
    ///
    /// The convenience form of [`Self::visual_row_of`] for a host that has a byte span (a
    /// diagnostic or a source map entry) rather than a `TextPosition`. The offset is clamped
    /// to the document through [`Self::position_of`], so the cost is the same `O(log lines)`
    /// lookup plus one line measurement.
    pub fn visual_line_at(&self, offset: usize) -> Option<usize> {
        self.visual_row_of(self.position_of(offset))
    }

    /// The visual row of a position already known to be inside the document.
    fn visual_row_index(&self, position: TextPosition) -> usize {
        let first = self.line_to_visual_row(position.line);
        if !self.config.word_wrap {
            return first;
        }
        let column = position.column.min(self.line_len(position.line));
        first + self.unwrapped_x(position.line, column).0
    }

    /// The gutter label for `line`, as an owned `String`.
    ///
    /// Kept for the numbering-rule test: the paint path emits SVG paths, so a test
    /// cannot read the number back out of the output. Production callers go through
    /// [`Self::write_line_number_label`] to reuse one buffer across the gutter.
    #[cfg(test)]
    pub(crate) fn line_number_label(&self, line: usize) -> String {
        let mut out = String::new();
        self.write_line_number_label(line, &mut out);
        out
    }

    /// Writes the gutter label for `line` into `out`, reusing its capacity.
    ///
    /// The gutter repaints one label per visible row per frame; `format!` gave
    /// each row its own heap allocation. Callers clear one buffer before the loop
    /// and pass it back every row, so steady-state painting allocates nothing.
    pub(crate) fn write_line_number_label(&self, line: usize, out: &mut String) {
        use core::fmt::Write as _;
        out.clear();
        if self.config.relative_line_numbers && line != self.cursor.head.line {
            let _ = write!(out, "{}", line.abs_diff(self.cursor.head.line));
        } else {
            let _ = write!(out, "{}", line + 1);
        }
    }

    /// The grid column's x inside the row, measured when the row was painted.
    ///
    /// The caret, the selection outline and the diagnostic underline all go through
    /// this one function, so the cursor cannot point between two glyphs while the
    /// highlight covers them.
    pub(crate) fn caret_x(&self, position: TextPosition) -> i32 {
        let left = self.geometry().x + self.text_origin_x();
        if let Some(x) = self.cached_x(position.line, position.column) {
            if self.config.word_wrap {
                // The cached layout is measured for the whole line, while the
                // caret is painted on one segment. Subtract the segment's own
                // measured origin and add the continuation indent, the same way
                // the row itself is drawn.
                let text = self.line_text(position.line).unwrap_or_default();
                let grid = LineGrid::new(&text, self.config.tab_width);
                let columns = self.text_columns().max(1);
                let (segment, segment_start) = grid.segment_of_char(position.column, columns);
                let base = self.cached_x(position.line, segment_start).unwrap_or(0.0);
                let indent = if segment == 0 {
                    0.0
                } else {
                    (grid.leading_indent_columns() as f32 * self.cell_width())
                        .min(columns.saturating_sub(4) as f32 * self.cell_width())
                };
                return self.screen_x(left, x - base + indent);
            }
            return self.screen_x(left, x);
        }
        let (_, x) = self.unwrapped_x(position.line, position.column);
        self.screen_x(left, x)
    }

    pub(super) fn draw_caret(
        &self,
        context: &mut RenderContext,
        row_height: f32,
        chrome: &EditorChrome,
    ) {
        // Caret geometry comes from the same map the text rows used, so a caret can
        // no longer be one rounding step away from the glyph it is supposed to sit
        // beside.
        let map = self.coordinate_map();
        // Secondary carets first so the primary caret paints on top. A collapsed secondary caret is
        // the same blinking insertion point as the primary one, so it follows the same phase — two
        // carets flickering out of step would look like a rendering fault.
        let all_carets = self.cursors();
        for caret in all_carets.carets().iter().copied() {
            if caret == self.cursor {
                continue;
            }
            if caret.is_collapsed() {
                if !self.caret_blink.is_visible() {
                    continue;
                }
                let row = self.line_to_visual_row(caret.head.line)
                    + if self.config.word_wrap {
                        self.unwrapped_x(caret.head.line, caret.head.column).0
                    } else {
                        0
                    };
                let y = map.row_y(row);
                let x = self.caret_x(caret.head);
                self.draw_caret_shape(context, x, y, row_height, chrome);
            } else if let Some(rect) = self.rect_for_range(caret.bounds().0, caret.bounds().1) {
                context.fill_rect(rect, chrome.selection);
            }
        }
        // Primary caret. A selection is not a caret, so the blink gates only the collapsed form: a
        // selected range must stay fully visible in both halves of the cycle. A non-collapsed
        // selection needs no work here at all: it is drawn as a filled band per row in
        // `draw_row_text`, and an extra outline around its first row read as a second, unrelated
        // box when the selection spanned lines.
        if self.cursor.is_collapsed() && self.caret_blink.is_visible() {
            let row = self.caret_visual_row();
            let y = map.row_y(row);
            let x = self.caret_x(self.cursor.head);
            self.draw_caret_shape(context, x, y, row_height, chrome);
        }
    }

    /// Paints one caret in the configured shape.
    ///
    /// The default line keeps the previous 2 px bar byte for byte; block and
    /// underline are opt-in through [`CaretStyle`].
    fn draw_caret_shape(
        &self,
        context: &mut RenderContext,
        x: i32,
        y: i32,
        row_height: f32,
        chrome: &EditorChrome,
    ) {
        let height = row_height.ceil().max(1.0) as u32;
        match self.caret_style {
            CaretStyle::Line => {
                context.fill_rect(Rect::new(x, y, 2, height), chrome.accent);
            }
            CaretStyle::Block => {
                let width = self.cell_width().round().max(2.0) as u32;
                context.fill_rect(
                    Rect::new(x, y, width, height),
                    chrome.accent.blend(&chrome.surface, 0.45),
                );
            }
            CaretStyle::Underline => {
                let width = self.cell_width().round().max(2.0) as u32;
                context.fill_rect(
                    Rect::new(x, y + height.saturating_sub(2) as i32, width, 2),
                    chrome.accent,
                );
            }
        }
    }

    /// Paints the in-progress IME composition at the caret.
    ///
    /// # Why the editor draws it at all
    ///
    /// A composition string is not in the document: the input method owns it until
    /// the commit arrives. The previous editor dropped `ImePreedit` entirely, so a
    /// Chinese/Japanese/Korean user saw an empty caret while composing — the text
    /// only appeared once a candidate was committed, and a mis-typed candidate could
    /// not be corrected before it landed in the buffer.
    ///
    /// The overlay is painted after the rows and with an **opaque** surface behind it:
    /// the characters that follow the caret are still laid out where they were, so a
    /// translucent composition would sit on top of them. An opaque band, an accent
    /// underline and a caret marker at the composition's own cursor is the same
    /// three-part shape macOS and Zed show.
    pub(super) fn draw_preedit(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        let Some(preedit) = self.preedit.as_ref() else {
            return;
        };
        if preedit.text.is_empty() {
            return;
        }
        let area_left = rect.x + self.text_origin_x();
        let area_top = rect.y + self.text_origin_y();
        let area_right = rect.x + rect.width as i32 - self.scrollbar_width() - self.minimap_width();
        let area_bottom = rect.y + rect.height as i32 - self.status_bar_height();
        if area_right <= area_left || area_bottom <= area_top {
            return;
        }

        let map = self.coordinate_map();
        let row = self.caret_visual_row();
        let y = map.row_y(row);
        let x = self.caret_x(self.cursor.head);
        let row_height = self.line_height().ceil() as u32;
        let font = self.editor_font();
        let width = context.measure_text(&preedit.text, &font).width as i32;
        if width <= 0 {
            return;
        }

        // The composition may start left of the text area when the caret is scrolled
        // under the gutter. Clipping (rather than clamping the origin) keeps it
        // attached to the caret it belongs to.
        context.push_clip(
            area_left,
            area_top,
            (area_right - area_left) as u32,
            (area_bottom - area_top) as u32,
        );
        let band = Rect::new(x, y, width as u32, row_height);
        context.fill_rect(band, chrome.surface);
        // A hairline frame on the leading and trailing edges tells the reader where
        // the composition begins and ends even when the characters themselves are
        // narrow (or drawn in a face that covers them poorly).
        context.draw_line(Point::new(x, y), Point::new(x, y + row_height as i32), chrome.accent);
        let baseline = context.text_line(band, &font).y;
        context.draw_text(
            Point::new(x, baseline),
            &preedit.text,
            &font,
            chrome.ink,
            HorizontalAlignment::Left,
        );
        // The composition's own cursor, at the byte offset the IME reported. It is
        // clamped to a character boundary: a byte in the middle of a CJK code point
        // would otherwise slice the string and panic.
        let cursor_byte =
            floor_char_boundary(&preedit.text, preedit.cursor.min(preedit.text.len()));
        let cursor = context.measure_text(&preedit.text[..cursor_byte], &font).width as i32;
        context.fill_rect(Rect::new(x + cursor, y, 2, row_height), chrome.accent);
        // Underline last, so it also marks the space under the cursor marker.
        context.fill_rect(Rect::new(x, y + row_height as i32 - 2, width as u32, 1), chrome.accent);
        context.pop_clip();
    }
}
