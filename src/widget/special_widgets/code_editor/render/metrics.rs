// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Metrics and hit-testing geometry.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

// ─────────────────────────────────────────────────────────────────────────────
// Free helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Rounds the configured line advance up to a tab-strip height.
pub(crate) fn row_height_of(line_advance: f32) -> i32 {
    (line_advance * 1.75).round().max(MIN_TOUCH_TARGET as f32) as i32
}

/// Leading padding inside the gutter, before the line numbers.
pub(crate) const GUTTER_LEFT_PAD: i32 = 10;
/// Trailing padding inside the gutter, between the fold column and the text.
pub(crate) const GUTTER_RIGHT_PAD: i32 = 8;
/// Gap between the gutter's own edge and column 0 of the text.
pub(crate) const GUTTER_TEXT_GAP: i32 = 6;

impl CodeEditor {
    pub(crate) fn line_height(&self) -> f32 {
        // One derivation for the whole widget: `resolved_line_advance` turns the
        // configured ratio into pixels, and an explicit `line_advance` override
        // wins. Rows, hit-testing and the gutter all read this.
        self.config.resolved_line_advance().max(1.0)
    }

    /// The font the editor's **chrome** draws with: the resolved theme/caller font.
    ///
    /// The document body has its own monospace face ([`CodeEditor::font`], from
    /// `CodeEditorConfig`), but the tab strip, find bar, gutter numbers, status row, completion list
    /// and context menu are ordinary UI chrome and take the theme's face and text scale
    /// (D09-STYLE-01). Named once so every chrome site measures and paints with the same font.
    pub(crate) fn chrome_font(&self) -> &Font {
        effective_font(self.style())
    }

    /// The horizontal advance of one character cell.
    ///
    /// Returns the **measured** advance once [`Self::measure_cell_width`] has run, and the
    /// configured `space_advance` before the first frame — there is no measuring before a backend
    /// is open, and the first frame's layout needs a number.
    pub(crate) fn cell_width(&self) -> f32 {
        self.measured_cell_width.get().unwrap_or_else(|| self.config.space_advance.max(1.0))
    }

    /// The font the body text is drawn in.
    ///
    /// One accessor so the measurement uses the same `Font` the drawing does — the draw sites built
    /// the font inline, which is exactly how a measurement and a draw come to disagree.
    pub(crate) fn editor_font(&self) -> crate::core::Font {
        crate::core::Font::new(self.config.font_family.clone(), self.config.font_size, false, false)
    }

    /// Re-measures the character cell with the backend's own font metrics.
    ///
    /// # Why this is measured rather than read from the config
    ///
    /// `config.space_advance` is hand-written (`7.0` by default) while the renderer draws text with
    /// the *real* font, and the two disagreed:
    ///
    /// ```text
    /// measured:  " " = 4    "M" = 8    "0123456789" = 78 (7.8 each)    "中文" = 26 (13 each)
    /// config:    everything = 7.0
    /// ```
    ///
    /// Every column past the first therefore drifted by 0.8px — a whole character by column ten — and
    /// CJK by 6px *per character*. Because the editor positions each syntax token at `column * cell`
    /// and then draws that token's text at that origin, the drift landed the pieces on top of one
    /// another: overlapping, unreadable text.
    ///
    /// # Why a digit, and why inside `draw`
    ///
    /// A digit rather than a space: the space measured **4** against a real advance of **7.8**, so it
    /// is not a reliable sample of the grid. Inside `draw` because the font metrics belong to the
    /// **backend**, which is only reachable while a context is open.
    ///
    /// # Why one cell and not per-character widths
    ///
    /// The editor is a fixed-pitch grid by construction: `text_columns`, caret hit-testing and the
    /// column ruler all compute a column as an integer multiple of one cell. Per-character shaping
    /// would be a different editor, not a fix for this one.
    pub(crate) fn measure_cell_width(&mut self, context: &mut RenderContext) {
        if self.measured_geometry == Some(self.geometry()) {
            return;
        }
        let font = self.editor_font();
        // `TextMetrics::width` is a `u32` in **logical** pixels, already rounded by the backend, so it
        // is directly comparable with the configured `space_advance` (also logical pixels).
        //
        // A digit is one cell wide in a monospace face, and this is the number the renderer will
        // actually advance by — which is the whole point: the placement and the drawing now read the
        // same measurement instead of one reading a hand-written constant.
        let advance = context.backend().measure_text("0", &font).width;
        // `> 0` guards a backend that cannot measure and returns 0: accepting that would collapse every
        // column onto x=0, which is worse than the drift being fixed.
        self.measured_cell_width.set(Some(if advance > 0 {
            advance as f32
        } else {
            self.config.space_advance.max(1.0)
        }));
        self.measured_geometry = Some(self.geometry());
        // The measured cell changes how many segments a line wraps into, so the
        // row-prefix table — built from `text_columns()` — is now stale.
        self.rebuild_visual_row_prefix();
    }

    pub(crate) fn text_columns(&self) -> usize {
        let width = self.geometry().width as i32 - self.text_origin_x() - self.minimap_width();
        (width.max(1) as f32 / self.cell_width()).floor().max(1.0) as usize
    }

    pub(crate) fn compute_visible_rows(&self) -> usize {
        // The status row is chrome, not text: counting its pixels as usable row
        // space left the last one or two lines painted underneath it.
        let usable =
            self.geometry().height as i32 - self.text_origin_y() - self.status_bar_height();
        (usable.max(1) as f32 / self.line_height()).floor().max(1.0) as usize
    }

    pub(crate) fn refresh_visible_rows(&mut self) {
        self.visible_rows = self.compute_visible_rows();
    }

    /// Returns the gutter width for the current geometry.
    ///
    /// Layout is `[pad][numbers, right-aligned][fold chevron][pad] | text`, which
    /// is the order Zed and every terminal editor use: the digits end in a
    /// straight column beside the source, and the fold affordance sits between
    /// them and the text rather than pushing the numbers around.
    pub fn gutter_width(&self) -> i32 {
        GUTTER_LEFT_PAD + self.line_number_slot() + self.fold_marker_width() + GUTTER_RIGHT_PAD
    }

    /// Width of the line-number column, or `0` when numbers are hidden.
    ///
    /// At least three digits wide even in a two-line file, so the gutter does not
    /// jump a cell every time the document crosses 9 → 10 or 99 → 100 (Zed's
    /// `min_line_number_digits` has the same job).
    pub(crate) fn line_number_slot(&self) -> i32 {
        if !self.config.show_line_numbers {
            return 0;
        }
        let digits = decimal_digits(self.line_count()).max(3);
        (digits as f32 * self.cell_width()).round() as i32
    }

    pub(crate) fn fold_marker_width(&self) -> i32 {
        (self.cell_width() * 1.5).round().max(12.0) as i32
    }

    /// x offset (from the widget's left edge) of the fold-chevron column.
    ///
    /// Shared by the painter and the hit-test, so the `v` a user sees is the `v`
    /// a click must land on.
    pub(crate) fn fold_marker_offset(&self) -> i32 {
        GUTTER_LEFT_PAD + self.line_number_slot() + 2
    }

    /// Returns the minimap width, or 0 when disabled.
    ///
    /// Roughly seven monospace cells, which is what a Zed/VS Code minimap occupies:
    /// the previous four cells were too narrow for the indent + length bars to say
    /// anything about the file's shape.
    pub fn minimap_width(&self) -> i32 {
        if self.config.show_minimap {
            (self.cell_width() * 7.0).round().clamp(48.0, 120.0) as i32
        } else {
            0
        }
    }

    /// Returns the scrollbar width.
    pub fn scrollbar_width(&self) -> i32 {
        if self.config.show_minimap {
            6
        } else {
            10
        }
    }

    /// Returns the x coordinate where source text begins.
    pub fn text_origin_x(&self) -> i32 {
        self.gutter_width() + GUTTER_TEXT_GAP
    }

    /// Returns the y coordinate where the first visual row begins.
    pub fn text_origin_y(&self) -> i32 {
        self.tab_strip_height() + self.find_bar_height()
    }

    pub(crate) fn tab_strip_height(&self) -> i32 {
        (self.line_height() * 1.75).round().max(MIN_TOUCH_TARGET as f32) as i32
    }

    pub(crate) fn status_bar_height(&self) -> i32 {
        if self.config.show_status_bar {
            MIN_TOUCH_TARGET
        } else {
            0
        }
    }

    /// Returns the find-bar height, or 0 when hidden.
    pub fn find_bar_height(&self) -> i32 {
        if !self.find.visible {
            return 0;
        }
        let rows = if self.find.replace_visible { 2 } else { 1 };
        rows * MIN_TOUCH_TARGET + 6
    }

    /// Maps a widget-local point to a text position, or `None` outside the text area.
    ///
    /// The reverse direction of the coordinate map: the same transform the paint
    /// loop uses, read backwards, so a click always lands on the row and column that
    /// were drawn there. Two conversions are needed on the way back, because the map
    /// speaks grid columns while the document speaks char indices: the grid column is
    /// resolved to the nearest character boundary of **that line**, and the row is
    /// resolved to its wrap segment so a click on a wrapped continuation line lands
    /// on the characters that line actually shows.
    pub fn position_at_point(&self, point: Point) -> Option<TextPosition> {
        let map = self.coordinate_map();
        if !map.text_area_contains(point) {
            return None;
        }
        let row = map.row_at_y(point.y);
        let line = self.visual_row_to_line(row);
        // Measured x positions are what the painter actually used, so a click lands
        // on the glyph that was drawn there. The grid is the fallback for a line
        // that has not been painted yet (it is rebuilt on every frame).
        let left = self.geometry().x + self.text_origin_x();
        let scroll_px = (self.scroll_column as f32 * self.cell_width()).round() as i32;
        if let Some(column) = self.cached_char_at(line, (point.x - left + scroll_px) as f32) {
            return Some(self.clamp_position(TextPosition::new(line, column)));
        }
        let text = self.line_text(line).unwrap_or_default();
        let grid = LineGrid::new(&text, self.config.tab_width);
        let column = if self.config.word_wrap {
            // A wrapped row shows one segment; the click is measured from that
            // segment's first column, less the continuation indent. Without this the
            // click was measured from column zero of the **line**, so every
            // continuation row hit-tested to the wrong characters.
            let columns = self.text_columns().max(1);
            let segment = row.saturating_sub(self.line_to_visual_row(line));
            let (segment_start, _) = grid.segment_range(segment, columns);
            let cell = self.cell_width();
            let indent = if segment == 0 {
                0.0
            } else {
                (grid.leading_indent_columns() as f32 * cell)
                    .min(columns.saturating_sub(4) as f32 * cell)
            };
            let local = ((point.x - left) as f32 - indent).max(0.0);
            let offset = (local / cell).round() as usize;
            let target = grid.column_of_char(segment_start) + offset;
            grid.char_at_column(target)
        } else {
            let column_cells = map.column_at_x(point.x);
            grid.char_at_column(column_cells)
        };
        Some(self.clamp_position(TextPosition::new(line, column)))
    }

    /// Returns the widget-local rectangle of a text range.
    ///
    /// Uses the coordinate map for both axes, so the selection a host draws and the
    /// selection the editor paints cannot disagree about where a range sits.
    pub fn rect_for_range(&self, start: TextPosition, end: TextPosition) -> Option<Rect> {
        let start = self.clamp_position(start);
        let end = self.clamp_position(end);
        if end < start {
            return None;
        }
        let map = self.coordinate_map();
        let start_row = self.visual_row_of(start);
        let end_row = self.visual_row_of(end);
        let top = map.row_y(start_row);
        // Both edges come from the same measured x the caret uses, so a selection
        // outline starts and ends exactly where the highlight and the cursor do.
        let left = self.caret_x(start);
        let rows = end_row.saturating_sub(start_row) + 1;
        let width = if end_row > start_row {
            map.bounds().width.max(1)
        } else {
            (self.caret_x(end) - left).max(1) as u32
        };
        let height = (rows as f32 * map.row_height()).round().max(1.0) as u32;
        Some(Rect::new(left, top, width, height))
    }

    /// Rectangle of the context menu, or `None` when closed.
    pub fn context_menu_rect(&self) -> Option<Rect> {
        if !self.context_menu.visible {
            return None;
        }
        let rect = self.geometry();
        let width = (rect.width as i32 / 2).clamp(180, 260) as u32;
        let height = self.context_menu.items.len().max(1) as u32 * MIN_TOUCH_TARGET as u32;
        let max_x = (rect.width as i32 - width as i32).max(0);
        let max_y = (rect.height as i32 - height as i32).max(0);
        Some(Rect::new(
            rect.x + self.context_menu.position.x.clamp(0, max_x),
            rect.y + self.context_menu.position.y.clamp(0, max_y),
            width,
            height,
        ))
    }

    /// Rectangle of the completion popup, clamped into the widget bounds.
    pub fn completion_rect(&self) -> Rect {
        let rect = self.geometry();
        let width = (rect.width as i32 / 2).clamp(180, 340) as u32;
        let height = self.completion.items.len().max(1) as u32 * MIN_TOUCH_TARGET as u32;
        let map = self.coordinate_map();
        let caret_row = self.line_to_visual_row(self.cursor.head.line);
        // One row below the caret line, so the popup never covers what is being
        // completed.
        let y = map.row_y(caret_row + 1);
        let max_y = (rect.y + rect.height as i32 - height as i32).max(rect.y);
        let max_x = (rect.x + rect.width as i32 - width as i32).max(rect.x);
        Rect::new(
            (rect.x + self.text_origin_x()).min(max_x),
            y.min(max_y.max(rect.y)),
            width,
            height,
        )
    }
}
