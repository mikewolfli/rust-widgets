// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Source rows: measurement and painting.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

impl CodeEditor {
    /// Draws source rows and returns the document line index painted per row.
    pub(super) fn draw_source_rows(
        &mut self,
        context: &mut RenderContext,
        rect: Rect,
        row_height: f32,
        chrome: &EditorChrome,
    ) -> Vec<usize> {
        let left = rect.x + self.text_origin_x();
        let right = rect.x + rect.width as i32 - self.scrollbar_width() - self.minimap_width();
        let width = (right - left).max(1) as u32;
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let height = (bottom - top).max(0) as u32;
        if height == 0 || width == 0 {
            return Vec::new();
        }

        context.push_clip(left, top, width, height);

        let map = self.coordinate_map();
        let rows_budget = (height as f32 / row_height).ceil() as usize + 1;
        let wrapped = self.config.word_wrap;
        let columns = self.text_columns().max(1);
        let font = self.font();
        let palette = self.palette.clone();
        let active_line = self.cursor.head.line;
        let brackets = self.matching_brackets();
        // The occurrence list is prepared once per frame by `prepare_occurrences`
        // (which rescans the document only when the selection or text changed).
        // Cloning the short result here is one small allocation per frame; the old
        // call rescanned every line of the file on every frame.
        let occurrences: Vec<SearchMatch> = self
            .occurrence_cache
            .as_ref()
            .filter(|cache| cache.bounds == self.cursor.bounds())
            .map(|cache| cache.matches.clone())
            .unwrap_or_default();
        let current_match = self.find.matches.get(self.find.current).cloned();
        let (selection_start, selection_end) = self.cursor.bounds();
        let mut painted: Vec<usize> = Vec::new();

        let mut row = 0usize;
        for line in self.viewport_document_lines() {
            let segments = self.wrap_segments(line);
            if row + segments <= self.scroll_visual_row {
                row += segments;
                continue;
            }
            if painted.len() >= rows_budget {
                break;
            }
            let text_guard = self.line_text_borrowed(line);
            let text: &str = &text_guard;
            // One grid per line: it owns the char ↔ byte ↔ column conversion for
            // every helper below, so a CJK glyph advances two cells in the
            // backdrops, the text run and the caret alike.
            let grid = LineGrid::new(text, self.config.tab_width);
            // The measured layout of the row, when the row is not soft-wrapped.
            // Wrapped rows keep the grid: their segment boundaries are computed
            // from grid columns before any measurement exists (see `wrap_segments`),
            // and mixing the two models inside one row would put the wrap point in
            // one place and the text in another.
            //
            // A cached layout is reused rather than re-shaped: the cache is keyed by the text
            // revision and every metric the measurement used (see `row_layout_key`), so a hit
            // means the numbers still describe the glyphs on screen. A miss measures the row
            // and stores it, which is also what the caret, selection and hit-test read.
            let layout = if wrapped {
                None
            } else {
                match self.cached_row_layout(line) {
                    Some(cached) => Some(cached),
                    None => {
                        let measured = self.measure_row_layout(context, line, text);
                        self.remember_row_layout(line, measured.clone());
                        Some(measured)
                    }
                }
            };
            let runs = if layout.is_some() { self.row_runs(line, text) } else { Vec::new() };
            // How far a continuation row is indented: the line's own leading
            // whitespace, clamped so it can never push the wrapped text off the row.
            // Computed once per line, not per segment.
            let cell = self.cell_width();
            let continuation = if wrapped {
                let indent = grid.leading_indent_columns() as f32 * cell;
                indent.min(columns.saturating_sub(4) as f32 * cell)
            } else {
                0.0
            };
            for segment in 0..segments {
                let visual_row = row + segment;
                if visual_row < self.scroll_visual_row {
                    continue;
                }
                let y = map.row_y(visual_row);
                let (segment_start, segment_end) = if wrapped {
                    grid.segment_range(segment, columns)
                } else {
                    (0, grid.char_len())
                };
                // One geometry for every element of this row: backdrops, guides,
                // glyphs, whitespace marks and the caret all read it.
                let geom = RowGeometry {
                    layout: layout.as_ref(),
                    grid: &grid,
                    cell,
                    first_column: grid.column_of_char(segment_start),
                    indent: if segment > 0 { continuation } else { 0.0 },
                };

                if self.config.highlight_active_line && line == active_line {
                    context.fill_rect(
                        Rect::new(rect.x, y, rect.width, row_height.ceil() as u32),
                        chrome.active_line,
                    );
                }

                self.draw_line_backdrops(
                    context,
                    left,
                    y,
                    row_height,
                    line,
                    &geom,
                    segment_start,
                    segment_end,
                    current_match.as_ref(),
                    &occurrences,
                    chrome,
                );

                if self.config.show_indent_guides {
                    self.draw_indent_guides(
                        context,
                        left,
                        y,
                        line,
                        segment_start,
                        segment_end,
                        text,
                        &geom,
                        chrome,
                    );
                }

                self.draw_row_text(
                    context,
                    left,
                    y,
                    &font,
                    &palette,
                    line,
                    text,
                    &geom,
                    &runs,
                    segment_start,
                    segment_end,
                    &brackets,
                    selection_start,
                    selection_end,
                    row_height,
                    chrome,
                );
                if self.config.show_whitespace {
                    self.draw_whitespace_marks(
                        context,
                        left,
                        y,
                        text,
                        &geom,
                        segment_start,
                        segment_end,
                        row_height,
                        chrome,
                    );
                }
                painted.push(line);
            }
            // Store the measured row for the caret, the selection and the click
            // handler, which all run with `&self` and no render context.
            row += segments;
        }

        self.draw_caret(context, row_height, chrome);
        context.pop_clip();
        // Layouts for rows that scrolled away are dropped: the cache is bounded by
        // the viewport, not by the document.
        self.prune_row_layouts(&painted);
        painted
    }

    /// The screen x of a measured offset inside the line.
    ///
    /// Horizontal scrolling is still expressed in columns (that is what the wheel
    /// and `scroll_column` speak), so it is converted to pixels once here.
    pub(super) fn screen_x(&self, left: i32, x: f32) -> i32 {
        left + (x - self.scroll_column as f32 * self.cell_width()).round() as i32
    }

    /// The byte ranges the row is drawn in, each with the token kind that colours
    /// it.
    ///
    /// One definition of the partition: [`Self::measure_row_layout`] measures these
    /// runs with `shape_text` and `draw_row_text` paints them. If the two built
    /// their own run lists, the measured x of a run and the x it is drawn at could
    /// differ — which is exactly the caret/ink mismatch this whole change is
    /// about.
    pub(super) fn row_runs(&self, line: usize, text: &str) -> Vec<(usize, usize, TokenKind)> {
        let mut runs: Vec<(usize, usize, TokenKind)> = Vec::new();
        let mut pen = 0usize;
        for span in self.tokens_on_line(line) {
            let start = floor_char_boundary(text, span.start.min(text.len()));
            let end = floor_char_boundary(text, span.end.min(text.len()));
            if start > pen {
                runs.push((pen, start, TokenKind::Plain));
            }
            let from = start.max(pen);
            if end > from {
                runs.push((from, end, span.kind));
                pen = end;
            }
        }
        if pen < text.len() {
            runs.push((pen, text.len(), TokenKind::Plain));
        }
        runs
    }

    /// Measures one row and returns its character-boundary x positions.
    ///
    /// # Why each *run* is shaped separately
    ///
    /// The renderer picks the face for a string from its first strong character, and
    /// a slice that starts with a CJK glyph is shaped by the CJK face while the same
    /// characters inside a Latin-led line come from the Latin face. Measuring the
    /// runs the painter will actually draw is therefore the only way the numbers and
    /// the ink agree; measuring the whole line once would misplace every CJK token
    /// after a Latin token.
    ///
    /// A tab is one cluster whose advance is its own (near zero) glyph width, so it
    /// is replaced here by the distance to the next tab stop, in pixels.
    pub(super) fn measure_row_layout(
        &self,
        context: &RenderContext,
        line: usize,
        text: &str,
    ) -> RowLayout {
        let font = self.editor_font();
        let mut xs: Vec<f32> = Vec::with_capacity(text.chars().count() + 1);
        let tab_stop = self.config.tab_width.max(1) as f32 * self.cell_width();
        let mut x = 0.0f32;
        for (from, to, _) in self.row_runs(line, text) {
            let piece = &text[from..to];
            let shaped = context.shape_text(piece, &font);
            for cluster in shaped.clusters() {
                let chars = cluster.text.chars().count().max(1);
                let advance = if cluster.text.chars().all(|ch| ch == '\t') {
                    let offset = x % tab_stop;
                    if offset <= f32::EPSILON {
                        tab_stop
                    } else {
                        tab_stop - offset
                    }
                } else {
                    cluster.advance
                };
                // A cluster spanning several chars (a ligature, a combining pair)
                // has one advance; its internal boundaries are interpolated so the
                // caret can still be placed inside it.
                for index in 0..chars {
                    xs.push(x + advance * (index as f32 / chars as f32));
                }
                x += advance;
            }
        }
        xs.push(x);
        RowLayout::from_boundaries(xs)
    }

    /// Returns the cached layout of `line`, when one was measured for the current key.
    ///
    /// The entry is a clone of a shared handle (`RowLayout::xs` is an `Rc`), so a hit costs a
    /// refcount bump rather than re-shaping the line.
    pub(super) fn cached_row_layout(&self, line: usize) -> Option<RowLayout> {
        let cache = self.row_layouts.borrow();
        cache.iter().find(|(key, _)| *key == line).map(|(_, layout)| layout.clone())
    }

    /// Stores the row's measured geometry for the caret, selection and hit-test.
    pub(super) fn remember_row_layout(&self, line: usize, layout: RowLayout) {
        let mut cache = self.row_layouts.borrow_mut();
        match cache.iter_mut().find(|(key, _)| *key == line) {
            Some(slot) => slot.1 = layout,
            None => cache.push((line, layout)),
        }
    }

    /// Drops layouts for lines that were not painted this frame.
    pub(super) fn prune_row_layouts(&self, painted: &[usize]) {
        self.row_layouts.borrow_mut().retain(|(line, _)| painted.contains(line));
    }

    /// Measured x of a character boundary, when the line has been painted.
    pub(crate) fn cached_x(&self, line: usize, char_index: usize) -> Option<f32> {
        let cache = self.row_layouts.borrow();
        let (_, layout) = cache.iter().find(|(key, _)| *key == line)?;
        layout.x_of_char(char_index)
    }

    /// Measured character index at a pixel offset inside the line.
    pub(super) fn cached_char_at(&self, line: usize, x: f32) -> Option<usize> {
        let cache = self.row_layouts.borrow();
        let (_, layout) = cache.iter().find(|(key, _)| *key == line)?;
        layout.char_at_x(x)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_line_backdrops(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        row_height: f32,
        line: usize,
        geom: &RowGeometry<'_>,
        segment_start: usize,
        segment_end: usize,
        current_match: Option<&SearchMatch>,
        occurrences: &[SearchMatch],
        chrome: &EditorChrome,
    ) {
        let highlight = |from: usize, to: usize| -> Option<(i32, u32)> {
            let start = from.max(segment_start);
            let end = to.min(segment_end);
            if end <= start {
                return None;
            }
            // Character indices in, screen x out, through the measured layout when it
            // exists: a CJK run is twice as wide as its char count, and a
            // proportional face is not even uniform within one script.
            let from_x = geom.x_of(start);
            let to_x = geom.x_of(end);
            let x = self.screen_x(left, from_x);
            let width = (to_x - from_x).round().max(1.0) as u32;
            Some((x, width))
        };
        // Search hits. `find.matches` is sorted by line, so the hits for this row
        // are a contiguous slice found by binary search rather than a full scan
        // of every match in the document, once per painted row.
        let first = self.find.matches.partition_point(|hit| hit.line < line);
        for hit in self.find.matches[first..].iter().take_while(|hit| hit.line == line) {
            if let Some((x, width)) = highlight(hit.start_column, hit.end_column) {
                context
                    .fill_rect(Rect::new(x, y, width, row_height.ceil() as u32), chrome.search_hit);
            }
        }
        if let Some(hit) = current_match.filter(|hit| hit.line == line) {
            if let Some((x, width)) = highlight(hit.start_column, hit.end_column) {
                context.fill_rect(
                    Rect::new(x, y, width, row_height.ceil() as u32),
                    chrome.search_current,
                );
            }
        }
        // Selected-word occurrences. Zed marks them with a **background tint**, not
        // an outline: a box around every hit draws attention to the box rather than
        // the repeated word, and on a dense line the boxes overlap into a grid. The
        // tint is the accent faded into the surface, so it stays behind the glyphs
        // instead of competing with the selection band.
        for hit in occurrences.iter().filter(|hit| hit.line == line) {
            if let Some((x, width)) = highlight(hit.start_column, hit.end_column) {
                context
                    .fill_rect(Rect::new(x, y, width, row_height.ceil() as u32), chrome.occurrence);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_indent_guides(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        line: usize,
        segment_start: usize,
        segment_end: usize,
        text: &str,
        geom: &RowGeometry<'_>,
        chrome: &EditorChrome,
    ) {
        let step = self.config.tab_width.max(1);
        // Indentation is counted in characters (a tab is one), and the guide's x is
        // taken from the measured layout: in a proportional face `4 spaces` is not
        // `4 × cell`, so a grid-placed guide would float away from the code it
        // indents.
        let mut indent = 0usize;
        for ch in text.chars() {
            match ch {
                ' ' | '\t' => indent += 1,
                _ => break,
            }
        }
        let height = self.line_height().round() as i32;
        // The innermost guide on the caret's line is the one the caret is *inside*,
        // so it is the one that answers "which block am I in". Zed paints exactly
        // that guide in its `editor_indent_guide_active` colour; the others stay
        // hairlines. Only the caret's line is promoted — a whole file of accent
        // guides would be louder than the code.
        let active_guide = (line == self.cursor.head.line)
            .then(|| indent / step * step)
            .filter(|level| *level > 0);
        let mut level = step;
        while level <= indent {
            if level >= segment_start && level < segment_end {
                let x = self.screen_x(left, geom.x_of(level));
                if Some(level) == active_guide {
                    context.fill_rect(
                        Rect::new(x, y, 1, height as u32),
                        chrome.accent.blend(&chrome.surface, 0.35),
                    );
                } else {
                    context.draw_line(
                        Point::new(x, y),
                        Point::new(x, y + height),
                        chrome.indent_guide,
                    );
                }
            }
            level += step;
        }
    }

    /// Draws a run of source text, expanding `\t` to the next tab stop.
    ///
    /// A tab has no glyph of its own, and the renderer would advance it by the
    /// tab character's (zero) width. Splitting the run at each tab and placing
    /// the following sub-run at the grid column the tab advances to keeps text
    /// after a literal tab where the caret says it is.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_text_run(
        &self,
        context: &mut RenderContext,
        left: i32,
        baseline: i32,
        geom: &RowGeometry<'_>,
        byte: usize,
        piece: &str,
        font: &Font,
        color: Color,
    ) {
        if piece.is_empty() {
            return;
        }
        let mut char_index = geom.char_of_byte(byte);
        let mut run_start = 0usize;
        let mut run_char = char_index;
        for (offset, ch) in piece.char_indices() {
            if ch == '\t' {
                if offset > run_start {
                    context.draw_text(
                        Point::new(self.screen_x(left, geom.x_of(run_char)), baseline),
                        &piece[run_start..offset],
                        font,
                        color,
                        HorizontalAlignment::Left,
                    );
                }
                char_index += 1;
                run_start = offset + 1;
                run_char = char_index;
            } else {
                char_index += 1;
            }
        }
        if run_start < piece.len() {
            context.draw_text(
                Point::new(self.screen_x(left, geom.x_of(run_char)), baseline),
                &piece[run_start..],
                font,
                color,
                HorizontalAlignment::Left,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_row_text(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        font: &Font,
        palette: &SyntaxPalette,
        line: usize,
        text: &str,
        geom: &RowGeometry<'_>,
        runs: &[(usize, usize, TokenKind)],
        segment_start: usize,
        segment_end: usize,
        brackets: &Option<(TextPosition, TextPosition)>,
        selection_start: TextPosition,
        selection_end: TextPosition,
        row_height: f32,
        chrome: &EditorChrome,
    ) {
        let seg_byte_start = geom.byte_of_char(segment_start);
        let seg_byte_end = geom.byte_of_char(segment_end);
        if seg_byte_start >= seg_byte_end {
            // An empty (or segment-free) row can still own the caret's selection
            // cell, but there is no glyph to paint.
            return;
        }

        // Selection background for this visual row, measured the same way the glyphs
        // are: the selected span's two boundaries give its pixel extent.
        if !self.cursor.is_collapsed() {
            let row_start = TextPosition::new(line, segment_start);
            let row_end = TextPosition::new(line, segment_end);
            if selection_start <= row_end && selection_end >= row_start {
                let from = selection_start
                    .column
                    .max(segment_start)
                    .max(if selection_start.line < line { 0 } else { selection_start.column });
                let to = if selection_end.line > line {
                    segment_end
                } else {
                    selection_end.column.min(segment_end)
                };
                if to > from {
                    let from_x = geom.x_of(from);
                    let to_x = geom.x_of(to);
                    let x = self.screen_x(left, from_x);
                    let width = (to_x - from_x).round().max(1.0) as u32;
                    context.fill_rect(
                        Rect::new(x, y, width, row_height.ceil() as u32),
                        chrome.selection,
                    );
                }
            }
        }

        // The row's glyph origin comes from the same line-box derivation the gutter
        // uses, rather than from `row_height * 0.78`. That fraction was an ascent
        // fitted to one font at one row height, so any row-height or font change
        // moved the source text off centre — and it disagreed with the line numbers
        // drawn beside it.
        let row_band =
            Rect::new(left, y, (segment_end - segment_start) as u32, row_height.ceil() as u32);
        let baseline = context.text_line(row_band, font).y;
        // Body text is the resolved ink, not the palette's light-preset literal: the
        // palette carries the *syntax hues*, which a caller may replace, but plain
        // running text has to stay legible against the resolved surface.
        let plain_ink = chrome.ink;
        let bracket_bytes = brackets.as_ref().map(|(open, close)| {
            (
                (open.line == line).then(|| geom.byte_of_char(open.column)),
                (close.line == line).then(|| geom.byte_of_char(close.column)),
            )
        });
        let (open_byte, close_byte) = bracket_bytes.unwrap_or((None, None));

        // Bracket-pair markers go **under** the glyphs. A filled rounded box, not an
        // outline: an outline that encloses a `{` reads as a selection rectangle,
        // while a tinted box is the "these two go together" mark Zed draws. The box
        // is painted before the runs so the bracket stays the darkest thing in it.
        let bracket_box = chrome.accent.blend(&chrome.surface, 0.82);
        if let Some((open, close)) = brackets {
            for position in [open, close] {
                if position.line != line {
                    continue;
                }
                let column = position.column;
                if column < segment_start || column >= segment_end {
                    continue;
                }
                // The width is the measured width of the bracket itself, not a
                // cell: in a proportional face `{` is wider than `|`.
                let from_x = geom.x_of(column);
                let to_x = geom.x_of(column + 1);
                let width = (to_x - from_x).round().max(1.0) as u32;
                let x = self.screen_x(left, from_x);
                context.fill_rounded_rect(
                    Rect::new(x - 1, y + 1, width + 2, row_height.ceil().max(3.0) as u32 - 2),
                    2,
                    bracket_box,
                );
            }
        }

        for &(from, to, kind) in runs {
            let start = floor_char_boundary(text, from.max(seg_byte_start).min(seg_byte_end));
            let end = floor_char_boundary(text, to.min(seg_byte_end));
            if end <= start {
                continue;
            }
            // The palette carries the *syntax hues*, but three of its categories are
            // not hues at all — `Plain`, `Identifier` and `Operator` are the default
            // running text. Kinds with no syntax meaning defer to the resolved ink;
            // the rest keep their palette hue but are pushed clear of the surface, so
            // a theme swap cannot make a keyword unreadable.
            let mut color = match kind {
                TokenKind::Plain | TokenKind::Identifier | TokenKind::Operator => plain_ink,
                _ => palette.color_for(kind).legible_on(chrome.surface, 4.5),
            };
            // Bracket-pair highlight: repaint both glyphs in the bracket colour.
            if Some(start) == open_byte || Some(start) == close_byte {
                color = chrome.accent;
            }
            // Deliberately drawn in the **regular** face. A synthesised bold or italic
            // keyword is wider than the cell the grid reserved for it, so it ran into
            // the next token (the overlap visible in the demo's `fn f(n`). Zed's
            // default themes colour tokens without changing their weight, and that is
            // also the only shape a fixed-pitch layout can guarantee.
            self.draw_text_run(
                context,
                left,
                baseline,
                geom,
                start,
                &text[start..end],
                font,
                color,
            );
        }

        // Bracket-pair markers. A **filled rounded box** behind the glyphs, not an
        // outline: an outline that encloses a `{` reads as a selection rectangle,
        // while a filled box is the "these two go together" mark Zed draws (it uses
        // a box that hugs the glyph, painted under the text). The width is measured,
        // because in a proportional face `{` is wider than `|`.
        // Bracket-pair markers are painted before the runs (see above), so nothing
        // is drawn on top of the glyphs here.
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_whitespace_marks(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        text: &str,
        geom: &RowGeometry<'_>,
        segment_start: usize,
        segment_end: usize,
        row_height: f32,
        chrome: &EditorChrome,
    ) {
        let dot_y = y + (row_height * 0.5).round() as i32;
        let from = geom.byte_of_char(segment_start);
        let to = geom.byte_of_char(segment_end);
        for (char_index, ch) in (segment_start..).zip(text[from..to].chars()) {
            let start_x = geom.x_of(char_index);
            let end_x = geom.x_of(char_index + 1);
            let x = self.screen_x(left, start_x);
            match ch {
                ' ' => context.fill_rect(
                    Rect::new(x + ((end_x - start_x) * 0.5) as i32, dot_y, 1, 1),
                    chrome.whitespace_mark,
                ),
                '\t' => {
                    context.draw_line(
                        Point::new(x + 2, dot_y),
                        Point::new(self.screen_x(left, end_x) - 2, dot_y),
                        chrome.whitespace_mark,
                    );
                }
                _ => {}
            }
        }
    }

    pub(super) fn draw_column_ruler(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        let column = self.config.column_ruler;
        if column == 0 || column < self.scroll_column {
            return;
        }
        // The ruler is the one element that stays on the **grid**: "column 100" is a
        // count of character cells, which is what an author writing a ruler line
        // means, even when the face in use is proportional.
        let x = self.coordinate_map().column_x(column);
        let right = rect.x + rect.width as i32 - self.scrollbar_width() - self.minimap_width();
        if x >= right {
            return;
        }
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        // Dashed, in the indent-guide tone. A solid separator-coloured line read as a
        // pane boundary (visible in the demo screenshot as a full-height rule beside
        // the code); a ruler is a *guide*, so it takes the guide colour and a broken
        // stroke — the same treatment Zed gives `editor_indent_guide`-family marks.
        const DASH: i32 = 3;
        const PERIOD: i32 = 7;
        let mut y = top;
        while y < bottom {
            let end = (y + DASH).min(bottom);
            context.draw_line(Point::new(x, y), Point::new(x, end), chrome.indent_guide);
            y += PERIOD;
        }
    }
}
