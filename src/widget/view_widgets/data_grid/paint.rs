// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `DataGrid` paint pass, cell geometry and hit testing.

use super::*;

impl Draw for DataGrid {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a page the bound bar reported, so the paint shows the page the reader asked for.
        self.poll_pagination();
        // Drain chooser visibility/order changes for the same reason: the frame that follows the
        // chooser click already reflects it (V-015).
        self.poll_column_chooser();
        // A bound filter row reports through the same shared-cell route (chapter 3, gap 2).
        self.poll_filter_row();
        // The companion links below are poll-only writes (COMP-A8): the strip's counts, the
        // summary row's aggregates and any queued header-menu intent are applied before the frame
        // that shows their effect.
        self.poll_grid_status_bar();
        self.poll_summary_row();
        self.poll_header_menu();
        let rect = self.base.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the cell fills, the borders and the text colour used
        // to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant). `data_grid` reaches the
        // theme through its `table` classification, which is `Input` — an interior that
        // moves with the appearance.
        // `WidgetStyle` carries a font-family `String` and a gradient `Vec`; cloning it to read
        // three `Copy` colour fields allocated both on every frame (WDGT-08). Copy the fields
        // under one short borrow instead.
        let (background_color, text_color, border_color) = {
            let style = self.base.style();
            (style.background_color, style.text_color, style.border_color)
        };
        let theme = crate::style::resolved_theme_style_shared("data_grid");
        let mut surface = background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `data_grid` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A panel
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        // One palette read for the surface, grid and accent roles (three theme locks before).
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        let border = border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // Cell chrome is the theme's `outline_variant` — the **weak** separator role — rather than a
        // local blend, so the grid is visibly weaker than the `outline`-strength focus ring (§5.4)
        // and a theme that tunes its separators moves this control with the rest of the table family
        // (`table_widget`, `virtual_table`, `tree_table`). The blend is the last rung for a build with
        // no theme.
        let cell_border = palette
            .as_ref()
            .map(|colors| colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.10));
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the frozen-column indicator follows the appearance rather than staying a literal blue.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let selected_bg = surface.blend(&accent, 0.22);

        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // The draw pass reads a borrowed view of the projection (F-P02): no per-row/`String`
        // clone happens on a scroll, because the view addresses the cached matrix by offset.
        let window_view = self.fetch_visible_window_view();
        if window_view.is_empty() {
            return;
        }

        // The column-title strip: the grid has always reserved it, and this is where it is
        // finally used. Names come from `set_header_labels` first, then the source's own
        // `column_name`, so two grids over different sources are distinguishable at the header row.
        let cells = self.cells_box();
        let header_font = effective_font(self.style());
        let header_y = cells.y - GRID_HEADER_HEIGHT as i32;
        if let Some(source) = self.data_source.as_ref() {
            let cells_right = cells.x + cells.width as i32;
            // Header titles are static; elide each one once and reuse it (F-P04).
            let generation = self.data_generation.load(Ordering::Relaxed);
            let column_count = self.column_count();
            // Extra header levels are drawn above the title row, one strip each (V-014). The
            // levels share the title row's column geometry, so a hidden column leaves no gap
            // here either.
            for (level, labels) in self.extra_header_rows.iter().enumerate() {
                let y = cells.y
                    - GRID_HEADER_HEIGHT as i32 * (self.extra_header_rows.len() - level + 1) as i32;
                for column in self.scroll_column..column_count {
                    if self.is_column_hidden(column) {
                        continue;
                    }
                    let x = cells.x + self.column_view_x(column);
                    if x >= cells_right {
                        break;
                    }
                    let width = self.column_width_at(column).max(1);
                    let band = Rect::new(x, y, width, GRID_HEADER_HEIGHT);
                    context.fill_rect(band, surface.blend(&ink, 0.05));
                    if let Some(label) = labels.get(column).filter(|label| !label.is_empty()) {
                        context.draw_text_fitted_cow(
                            context.text_line(band, header_font),
                            label,
                            header_font,
                            ink,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }
            let mut text_cache = self.header_text_cache.borrow_mut();
            if text_cache.labels_revision != self.header_text_revision
                || text_cache.generation != generation
                || text_cache.column_count != column_count
            {
                text_cache.entries.clear();
                text_cache.labels_revision = self.header_text_revision;
                text_cache.generation = generation;
                text_cache.column_count = column_count;
            }
            for column in self.scroll_column..column_count {
                // A hidden column has no painted header section (V-014).
                if self.is_column_hidden(column) {
                    continue;
                }
                let x = cells.x + self.column_view_x(column);
                if x >= cells_right {
                    break;
                }
                let width = self.column_width_at(column).max(1);
                let band = Rect::new(x, header_y, width, GRID_HEADER_HEIGHT);
                context.fill_rect(band, surface.blend(&ink, 0.06));
                let name = text_cache.entries.entry((column, width)).or_insert_with(|| {
                    let raw = self
                        .header_labels
                        .get(column)
                        .filter(|label| !label.is_empty())
                        .cloned()
                        .or_else(|| source.column_name(column));
                    raw.map_or_else(String::new, |name| {
                        crate::widget::text_utils::elide_end(&name, 24).into_owned()
                    })
                });
                if !name.is_empty() {
                    // Leave room for the sort arrow and the filter glyph before fitting the label.
                    let filter_slot = if self.header_filter_visible
                        && self.header_columns.get(column).is_some_and(|spec| spec.filterable)
                    {
                        12
                    } else {
                        0
                    };
                    let sort_slot = if self.sort_indicator_visible
                        && self.sort_specs.iter().any(|spec| spec.column == column)
                    {
                        12
                    } else {
                        0
                    };
                    let arrow = filter_slot + sort_slot;
                    let text_band = Rect::new(
                        band.x + 2,
                        band.y,
                        band.width.saturating_sub(4 + arrow),
                        band.height,
                    );
                    context.draw_text_fitted_cow(
                        context.text_line(text_band, header_font),
                        name.as_str(),
                        header_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
                if self.sort_indicator_visible {
                    if let Some(spec) = self.sort_specs.iter().find(|spec| spec.column == column) {
                        let icon_side = band.height.min(10);
                        let filter_slot = if self.header_filter_visible
                            && self.header_columns.get(column).is_some_and(|spec| spec.filterable)
                        {
                            12
                        } else {
                            0
                        };
                        let icon_rect = Rect::new(
                            band.x + band.width as i32 - filter_slot - icon_side as i32 - 3,
                            band.y + (band.height as i32 - icon_side as i32) / 2,
                            icon_side,
                            icon_side,
                        );
                        let icon_name = if spec.descending {
                            crate::widget::IconName::ArrowDown
                        } else {
                            crate::widget::IconName::ArrowUp
                        };
                        crate::widget::draw_icon_at(context, icon_rect, ink, icon_name);
                    }
                }
                // A filterable column advertises the affordance its header spec declares; the
                // glyph is the same one the standalone `GridViewHeader` paints.
                if self.header_filter_visible
                    && self.header_columns.get(column).is_some_and(|spec| spec.filterable)
                {
                    let icon_side = band.height.min(10);
                    let icon_rect = Rect::new(
                        band.x + band.width as i32 - icon_side as i32 - 3,
                        band.y + (band.height as i32 - icon_side as i32) / 2,
                        icon_side,
                        icon_side,
                    );
                    crate::widget::draw_icon_at(
                        context,
                        icon_rect,
                        ink.blend(&surface, 0.3),
                        crate::widget::IconName::Filter,
                    );
                }
            }
        }

        // The cells are laid out from the control's inset content box, not from its literal
        // top edge. The first row used to start at `rect.y`, so its border and its glyph box
        // sat on the frame's own stroke; the inset also leaves a header row's worth of room
        // above the first cell, which is what a grid reads as its column-title strip.
        let cells = self.cells_box();
        let cells_right = cells.x + cells.width as i32;
        let cells_bottom = cells.y + cells.height as i32;

        // Every cell's box comes from `cell_rect`, the same derivation `cell_at` reads, so a
        // painted cell and the cell a press resolves to can never drift apart.
        //
        // The fetched window starts at `scroll - overscan`, so its leading rows/columns are above/
        // left of the viewport. Convert each window-local index to its **global** projected index
        // before `cell_rect` (which subtracts the scroll) (D09-VIEW-01). The filtered/sorted
        // projection is left as-is: its row identity is not the source's projected index, so no
        // correction is applied here.
        let (row_origin, col_origin) = self.window_origin();
        if self.alternating_rows {
            for row_idx in 0..window_view.len() {
                let global_row = row_origin + row_idx;
                if global_row % 2 == 1 {
                    let y = self.cell_rect(global_row, 0).y;
                    context.fill_rect(
                        Rect::new(cells.x, y, cells.width, self.row_height),
                        surface.blend(&accent, 0.05),
                    );
                }
            }
        }
        for row_idx in 0..window_view.len() {
            if self.cell_rect(row_origin + row_idx, 0).y >= cells_bottom {
                break;
            }
            let Some(row) = window_view.row(row_idx) else { continue };
            for (col_idx, cell) in row.iter().enumerate() {
                let global_column = col_origin + col_idx;
                // Frozen columns are painted by the overdraw pass below, after every scrolling
                // cell. Skipping them here is what makes that pass the single authority for the
                // pinned band: painting them twice would also lay their text down twice.
                if global_column < self.frozen_columns || self.is_column_hidden(global_column) {
                    continue;
                }
                let cell_rect = self.cell_rect(row_origin + row_idx, global_column);
                if cell_rect.x >= cells_right {
                    break;
                }
                let _ = self.paint_cell_at(
                    context,
                    cell.as_ref(),
                    row_origin + row_idx,
                    global_column,
                    cell_border,
                    ink,
                    selected_bg,
                );
            }
        }

        // ── Frozen columns, repainted over the scrolling cells ──
        //
        // The render layer has no clip primitive (there is no `clip_rect` in `src/render`), so
        // the compatible way to pin a leading band is the same one the rest of the family uses:
        // paint the scrolling grid first and overdraw the frozen cells on top of it. Their boxes
        // come from `cell_rect`, which ignores `scroll_column` for a frozen column, so this pass
        // is also the proof that the pinned band and the hit test agree.
        if self.frozen_columns > 0 {
            for row_idx in 0..window_view.len() {
                let Some(row) = window_view.row(row_idx) else { continue };
                let global_row = row_origin + row_idx;
                if self.cell_rect(global_row, 0).y >= cells_bottom {
                    break;
                }
                for column in 0..self.frozen_columns {
                    if self.is_column_hidden(column) {
                        continue;
                    }
                    let local = column.saturating_sub(col_origin);
                    let cell_rect = self.cell_rect(global_row, column);
                    if cell_rect.x >= cells_right {
                        break;
                    }
                    // `draw_rect` is a stroke, so the scrolling cell's ink underneath must be
                    // covered with the surface before this cell's own border and value go down.
                    context.fill_rect(cell_rect, surface);
                    let value = row.get(local).and_then(|cell| cell.as_ref());
                    let _ = self.paint_cell_at(
                        context,
                        value,
                        global_row,
                        column,
                        cell_border,
                        ink,
                        selected_bg,
                    );
                }
            }
        }

        // ── Row detail bands (V-009) ──
        //
        // An expanded row reserves its detail height in the row geometry, so this pass only has
        // to paint the reserved band. The text comes from the provider once per painted band,
        // which keeps a collapsed or off-screen row free of calls.
        for row_idx in 0..window_view.len() {
            let global_row = row_origin + row_idx;
            if !self.expanded_detail_rows.contains(&global_row) {
                continue;
            }
            let row_rect = self.cell_rect(global_row, 0);
            let detail_top = row_rect.y + self.row_height as i32;
            if detail_top >= cells_bottom {
                break;
            }
            let height = self.detail_height(global_row);
            let band = Rect::new(cells.x, detail_top, cells.width, height);
            context.fill_rect(band, surface.blend(&ink, 0.04));
            context.draw_rect(band, cell_border);
            if let Some(text) = self.row_detail_text(global_row).filter(|text| !text.is_empty()) {
                let font = effective_font(self.style());
                context.draw_text_fitted_cow(
                    context.text_line(band, font),
                    &text,
                    font,
                    ink.blend(&surface, 0.2),
                    HorizontalAlignment::Left,
                );
            }
        }

        // ── Selection and hover, painted **over** the cells ──
        //
        // The selection was stored and never drawn: a press set `self.selection`, re-requested a
        // redraw, and the grid came back looking exactly the same. A stored state with no consumer
        // is the same shape as a declared token with no consumer — the user cannot tell a selected
        // cell from any other. It is drawn here, over the cell borders, so the marker is not
        // overpainted by the next row's line.
        //
        // The hover is the accent at **lower** alpha: pointer position rather than a committed
        // choice, so it points at the cell a click would select without competing with the one
        // already selected. Both come from `cell_rect`, the same derivation `cell_at` reads, so a
        // highlighted cell is always a cell that can be hit.
        if let Some((row, column)) = self.hovered_cell {
            if !self.selection_contains(row, column) {
                let box_rect = self.cell_rect(row, column);
                context.draw_rect_stroke(box_rect, accent.with_alpha(120), 1);
            }
            // A tooltip is drawn only for the cell under the pointer, so a large model costs
            // nothing: the callback runs once per frame at most, and only when installed
            // (V-014).
            if let Some(text) = self.cell_tooltip(row, column).filter(|text| !text.is_empty()) {
                let cell = self.cell_rect(row, column);
                let tip_height = GRID_HEADER_HEIGHT;
                let tip_y = (cell.y + cell.height as i32 + 2)
                    .min(cells.y + cells.height as i32 - tip_height as i32)
                    .max(cells.y);
                let tip = Rect::new(cell.x, tip_y, cell.width.max(80).min(cells.width), tip_height);
                context.fill_rect(tip, surface.blend(&ink, 0.06));
                context.draw_rect(tip, cell_border);
                let font = effective_font(self.style());
                context.draw_text_fitted_cow(
                    context.text_line(tip, font),
                    &text,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
        if let Some((row, column)) = self.current_cell.or(self.selection) {
            let box_rect = self.cell_rect(row, column);
            context.draw_rect_stroke(box_rect, accent, 2);
        }

        // The divider is the frozen band's **right edge**. That is the same arithmetic
        // `cell_rect` gives a frozen column's last trailing pixel, so the rule and the pinned
        // cells cannot disagree about where the band ends.
        if self.frozen_columns > 0 {
            let split_x = cells.x + self.column_prefix_width(self.frozen_columns);
            if split_x > cells.x {
                context.draw_line(
                    Point::new(split_x, cells.y),
                    Point::new(split_x, cells.y + cells.height as i32),
                    accent,
                );
            }
        }

        // Scroll-bar thumbs are opt-in. The grid is virtualized, so a grid without them can
        // still be scrolled by wheel/keyboard; the thumbs are the visible affordance that
        // says how much of the model is off-screen.
        if self.show_scrollbars {
            let total_rows = self.row_count().max(1);
            let visible_rows = self.visible_row_capacity().max(1).min(total_rows);
            let track = cells.height.max(1);
            let thumb = ((track as u64 * visible_rows as u64) / total_rows as u64)
                .max(24)
                .min(track as u64) as u32;
            let travel = track.saturating_sub(thumb);
            let max_scroll = total_rows.saturating_sub(visible_rows);
            let offset = if max_scroll == 0 {
                0
            } else {
                (travel as u64 * self.scroll_row.min(max_scroll) as u64 / max_scroll as u64) as u32
            };
            let band = Rect::new(rect.x + rect.width as i32 - 6, cells.y + offset as i32, 6, thumb);
            context.fill_rect(band, accent.with_alpha(120));
        }
    }
}

impl DataGrid {
    /// The area the cells may occupy: the grid's box minus the margin that keeps ink off the
    /// frame's stroke, with the column-title strip removed from the top.
    ///
    /// Both the draw pass and the hit test start here, so a cell's painted box and the box a
    /// press resolves to are the same rectangle by construction.
    pub(crate) fn cells_box(&self) -> Rect {
        let content = ControlMetrics::band_inset(self.base.geometry(), GRID_INSET);
        let strip = self.header_strip_height();
        let height = content.height.saturating_sub(strip);
        Rect::new(content.x, content.y + strip as i32, content.width, height)
    }

    /// The box of the cell at `(row, column)` in the *projection* — the index the data source
    /// serves, including the scroll offset.
    ///
    /// This is a pure function of the indices, so the column a press lands in is the column
    /// that was painted for it. The draw loop used to advance its own `x += column_width`
    /// cursor, which made a cell's box depend on how many cells happened to precede it and
    /// left the two derivations free to disagree.
    pub(crate) fn cell_rect(&self, row: usize, column: usize) -> Rect {
        let cells = self.cells_box();
        // The row also travels the scroll: `row` is a **global** projected index, so its screen y is
        // measured from the current scroll row, the same way the column is measured from
        // `scroll_column`. The offset accumulates every open detail band between the scroll
        // position and the row (V-009), so an expanded row shifts its followers down by exactly
        // the room its detail band takes.
        let row = self.row_view_offset(row);
        // A frozen column is pinned to the leading edge by its **own** index. The scrolling band
        // starts after the frozen prefix, and each column travels by the sum of the widths before
        // it — not by `index * width`, which is only true while every column is the same size.
        Rect::new(
            cells.x + self.column_view_x(column),
            cells.y + row,
            self.column_width_at(column),
            self.row_height,
        )
    }

    /// Paints one cell's border and value.
    ///
    /// Extracted so the scrolling pass and the frozen repaint use the **same** ink, font and
    /// alignment; two copies would let the pinned band drift away from the cells it sits over.
    fn paint_cell(
        &self,
        context: &mut RenderContext,
        cell_rect: Rect,
        value: Option<&String>,
        cell_border: Color,
        ink: Color,
        alignment: HorizontalAlignment,
    ) {
        context.draw_rect(cell_rect, cell_border);
        let Some(text) = value else {
            return;
        };
        // Guarded on the text being non-empty: an unguarded draw of an empty cell emits
        // `<text …></text>`, an element the rasteriser never produces.
        if text.is_empty() {
            return;
        }
        // The cell is the band. Centring by handing `y + row_h / 2` to `draw_text` put the glyph
        // box's *top* edge on the cell's middle line, so every value sat half a line low;
        // `text_line` derives the real centred box. Fitting as well keeps a long value inside its
        // own column instead of bleeding right.
        let font = effective_font(self.style());
        context.draw_text_fitted_cow(
            context.text_line(cell_rect, font),
            text,
            font,
            ink,
            alignment,
        );
    }

    /// Paints a cell through the delegate when one is installed, otherwise directly.
    ///
    /// The delegate's `false` answer is the documented "keep the grid's own rendering" path, so
    /// a host can decorate selected cells and leave the rest to the control (V-014).
    fn paint_cell_ink(
        &self,
        context: &mut RenderContext,
        cell_rect: Rect,
        value: Option<&String>,
        row: usize,
        column: usize,
        cell_border: Color,
        ink: Color,
    ) {
        if let Some(delegate) = self.cell_delegate.as_ref() {
            if delegate.paint(context, cell_rect, row, column, value.map(String::as_str)) {
                return;
            }
        }
        self.paint_cell(context, cell_rect, value, cell_border, ink, self.column_alignment(column));
    }

    /// The union box of a merged run (V-014).
    fn span_rect(&self, span: CellSpan) -> Rect {
        let origin = self.cell_rect(span.row, span.column);
        let last_row = span.row.saturating_add(span.row_span.saturating_sub(1));
        let last_column = span.column.saturating_add(span.column_span.saturating_sub(1));
        let last = self.cell_rect(last_row, last_column);
        let right = last.x + last.width as i32;
        let bottom = last.y + last.height as i32;
        Rect::new(
            origin.x,
            origin.y,
            (right - origin.x).max(0) as u32,
            (bottom - origin.y).max(0) as u32,
        )
    }

    /// Paints `(row, column)`, merging it with the run it anchors (V-014).
    ///
    /// Returns `true` when ink was laid down. A cell covered by another cell's span answers
    /// `false`: the origin paints the whole run, including this cell's area.
    pub(crate) fn paint_cell_at(
        &self,
        context: &mut RenderContext,
        value: Option<&String>,
        row: usize,
        column: usize,
        cell_border: Color,
        ink: Color,
        selected_bg: Color,
    ) -> bool {
        match self.span_covering(row, column) {
            Some(span) if span.row == row && span.column == column => {
                let rect = self.span_rect(span);
                if self.selection_contains(row, column) {
                    context.fill_rect(rect, selected_bg);
                }
                self.paint_cell_ink(context, rect, value, row, column, cell_border, ink);
                true
            }
            Some(_) => false,
            None => {
                let rect = self.cell_rect(row, column);
                if self.selection_contains(row, column) {
                    context.fill_rect(rect, selected_bg);
                }
                self.paint_cell_ink(context, rect, value, row, column, cell_border, ink);
                true
            }
        }
    }

    /// The projected `(row, column)` a point falls in, or `None` when it misses every cell
    /// (including a press in the column-title strip or outside the grid).
    pub fn cell_at(&self, point: Point) -> Option<(usize, usize)> {
        let cells = self.cells_box();
        if !cells.contains_point(point) {
            return None;
        }
        let dx = point.x - cells.x;
        let dy = point.y - cells.y;
        if dx < 0 || dy < 0 {
            return None;
        }
        let column = self.column_at_dx(dx)?;
        let row = self.row_at_dy(dy)?;
        if column >= self.column_count() || row >= self.row_count() {
            return None;
        }
        if !self.cell_rect(row, column).contains_point(point) {
            return None;
        }
        // A merged cell answers as its origin, so a press anywhere inside the run selects (and
        // draws the focus ring around) the cell that actually owns the value (V-014).
        if let Some(span) = self.span_covering(row, column) {
            return Some((span.row, span.column));
        }
        Some((row, column))
    }

    /// The column-title strip.
    pub(crate) fn header_band(&self) -> Rect {
        let cells = self.cells_box();
        let height = self.header_strip_height();
        Rect::new(cells.x, cells.y - height as i32, cells.width, height)
    }

    /// The column index in the title strip under `point`.
    pub(crate) fn header_column_at(&self, point: Point) -> Option<usize> {
        let band = self.header_band();
        if !band.contains_point(point) {
            return None;
        }
        let dx = point.x - band.x;
        if dx < 0 {
            return None;
        }
        self.column_at_dx(dx).filter(|column| *column < self.column_count())
    }
}
