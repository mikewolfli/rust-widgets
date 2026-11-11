// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable drawing.

use super::*;

impl Draw for VirtualTable {
    fn draw(&mut self, context: &mut RenderContext) {
        // The source change signal only raises a flag; the repair runs once here, before any row
        // or column state is read.
        self.drain_data_updates();
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the cell fills and the cell text colour
        // used to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        // `WidgetStyle` carries a font-family `String` and a gradient `Vec`; cloning it to read
        // three `Copy` colour fields allocated both on every frame (WDGT-08). Copy the fields
        // under one short borrow instead.
        let (background_color, text_color, border_color) = {
            let style = self.base.style();
            (style.background_color, style.text_color, style.border_color)
        };
        let theme = crate::style::resolved_theme_style_shared("virtual_table");
        let mut surface = background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `virtual_table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // panel painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // the same distinction `Colors::input_background` draws for a field.
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
        // The cell grid is the theme's `outline_variant` — the **weak** separator role — so a
        // table's grid lines are visibly weaker than its `outline`-strength focus ring, which is
        // the separation §5.4 of the plan asks for. The blind `surface.blend(&ink, 0.10)` fallback
        // remains for a build with no theme; it is the last rung, not the derivation.
        //
        // Read the role rather than blending locally so this control and `table_widget` (same
        // `Table` kind, so the same grid) cannot drift apart: a theme that tunes its grid cannot
        // tune one without the other.
        let cell_border = palette
            .as_ref()
            .map(|colors| colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.10));
        // A hovered row reads the same accent at a **lighter** weight than a selection:
        // it is pointer position, not a committed state. The accent comes from the same
        // palette read above, so the hover costs no extra theme lock.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let hovered_bg = surface.blend(&accent, 0.12);

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

        if self.show_header {
            if let Some(source) = self.data_source.as_ref() {
                let font = effective_font(self.style());
                let (_, columns) = self.visible_grid(rect);
                let scroll_offset = self.column_offset(self.scroll_column);
                let mut painted = 0usize;
                for global in self.scroll_column..self.column_count() {
                    // A hidden column keeps its source index but has no slot (V-013).
                    if self.is_column_hidden(global) {
                        continue;
                    }
                    if painted >= columns {
                        break;
                    }
                    let width = self.column_width_at(global);
                    let band = Rect::new(
                        rect.x + 2 + self.column_offset(global) - scroll_offset,
                        rect.y + 2,
                        width.saturating_sub(2),
                        self.row_height.saturating_sub(2),
                    );
                    context.fill_rect(band, surface.blend(&ink, 0.08));
                    // The resolved label (host label, source name or the `Column N` fallback) is
                    // cached, so an unnamed column no longer formats its fallback per frame
                    // (VP-007).
                    let name = self.header_name(global, source.as_ref());
                    context.draw_text_fitted_ellipsis(
                        context.text_line(band, font),
                        name.as_str(),
                        font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                    painted += 1;
                }
            }
        }

        // An empty source paints its own caption in the cell band (RW-17), so a host does not have
        // to overlay text on a control it does not own.
        if self.row_count() == 0 {
            let top = rect.y + 2 + self.header_offset();
            let band = Rect::new(
                rect.x + 2,
                top,
                rect.width.saturating_sub(4),
                (rect.y + rect.height as i32 - 2 - top).max(0) as u32,
            );
            paint_collection_placeholder(
                context,
                band,
                &self.placeholder,
                ink,
                effective_font(self.style()),
            );
            return;
        }

        // Read the shared cached window (D09-VIEW-03): a cache hit hands back the existing `Arc`
        // rather than deep-cloning every row, column and `String` on every redraw. All the geometry
        // the layout needs is computed *before* the window borrow is taken, and the shared handle is
        // cloned out so the loop below reads it without holding a borrow of `self`.
        let (row_origin, col_origin) = self.window_origin();
        let (visible_rows, visible_columns) = self.visible_grid(rect);
        let Some(data) = self.visible_window_cells().cloned() else {
            return;
        };
        if data.is_empty() {
            return;
        }

        // The fetched window starts at `scroll - overscan`, so its first `overscan` rows/columns are
        // *above/left of* the viewport. Skip exactly that many before laying out, otherwise the
        // leading overscan data is painted as if it were the first visible row/column and non-zero
        // scroll shows shifted content (D09-VIEW-01). The skipped amount is `scroll - row_start`,
        // the same quantity the draw loop's local index must be offset by to reach the viewport.
        let leading_rows = self.scroll_row.saturating_sub(row_origin);
        let leading_cols = self.scroll_column.saturating_sub(col_origin);

        // The alternating wash is a row band under the cells, so it is painted before the cell
        // strokes. Off by default, which keeps the historical pixels (V-013).
        if self.alternating_rows {
            const INSET: i32 = 2;
            for local_row in 0..visible_rows {
                if (self.scroll_row + local_row) % 2 == 1 {
                    context.fill_rect(
                        Rect::new(
                            rect.x + INSET,
                            rect.y
                                + INSET
                                + self.header_offset()
                                + local_row as i32 * self.row_height as i32,
                            rect.width.saturating_sub(4),
                            self.row_height.saturating_sub(2),
                        ),
                        surface.blend(&accent, 0.05),
                    );
                }
            }
        }

        // The grid's extent and each cell's box come from `visible_grid`/`cell_rect`, the same
        // derivation `cell_at` reads, so the cell a press resolves to is the cell that was
        // painted. The loop no longer carries `x`/`y` accumulators — a cell's position is a
        // function of its own indices, which is what makes it answerable outside this loop.
        let visible = data
            .iter()
            .skip(leading_rows)
            .take(visible_rows)
            .map(|row| row.iter().skip(leading_cols).take(visible_columns));
        // The hovered row's viewport-local index, when the absolute hover is inside the
        // window actually painted. The band is drawn once, spanning the full row, before
        // the cells stroke over it — a whole-row tint is the affordance the family shares,
        // and a cell-sized one flickers as the pointer crosses column boundaries.
        let hovered_local = self
            .hovered_row
            .and_then(|abs| abs.checked_sub(self.scroll_row))
            .filter(|local| *local < visible_rows);
        if let Some(local_row) = hovered_local {
            // The band's top and height are the cell band's own: `INSET + header_offset() +
            // row * stride` tall and `stride - 2` high, so the tint covers the row's cells and
            // the 2 px gaps between them as one continuous stripe. Without the header offset the
            // tint sits one row above the cells it belongs to.
            const INSET: i32 = 2;
            context.fill_rect(
                Rect::new(
                    rect.x + INSET,
                    rect.y
                        + INSET
                        + self.header_offset()
                        + local_row as i32 * self.row_height as i32,
                    rect.width.saturating_sub(4),
                    self.row_height.saturating_sub(2),
                ),
                hovered_bg,
            );
        }
        // One font resolution for the whole grid (F-P07): the loop used to resolve the face twice
        // per visible cell, so a full screen paid hundreds of resolutions per frame.
        let cell_font = effective_font(self.style());
        for (row_index, row) in visible.enumerate() {
            // `local_column` counts painted slots, so a hidden column leaves no gap and the
            // ordinal still maps back to the absolute column `cell_at` answers with (V-013).
            let mut local_column = 0usize;
            for (column_index, cell) in row.enumerate() {
                let absolute_column = self.scroll_column + column_index;
                if self.is_column_hidden(absolute_column) {
                    continue;
                }
                let cell_rect = self.cell_rect(rect, row_index, local_column);
                local_column += 1;
                let absolute = (self.scroll_row + row_index, absolute_column);
                if self
                    .encode_cell_index(absolute.0, absolute.1)
                    .is_some_and(|index| self.selection.is_selected(index))
                {
                    context.fill_rect(cell_rect, surface.blend(&accent, 0.30));
                }
                context.draw_rect(cell_rect, cell_border);
                let painted_by_delegate = match self.delegate.as_ref() {
                    Some(delegate) => {
                        delegate.paint(context, cell_rect, absolute.0, absolute.1, cell.as_deref())
                    }
                    None => false,
                };
                if let Some(value) = cell.as_ref().filter(|_| !painted_by_delegate) {
                    // The cell is the band. `y + row_height / 2` as a `draw_text` origin placed
                    // the glyph box's *top* edge on the cell's middle line, so the value drew half
                    // a line low; `text_line` derives the centred box. Fitting keeps a wide value
                    // from spilling into the next column.
                    context.draw_text_fitted_ellipsis(
                        context.text_line(cell_rect, cell_font),
                        value.as_str(),
                        cell_font,
                        ink,
                        self.column_alignment(absolute_column),
                    );
                }
            }
        }

        // The thumb is derived (UX-04): a virtualized table whose rows run past the viewport
        // paints it whether or not a caller asked, because a scrolling table with no indication
        // is chrome that lies. The flag stays as a force-on override.
        let total_rows = self.row_count().max(1);
        let (rows, _) = self.visible_grid(rect);
        let visible_rows = rows.max(1).min(total_rows);
        // A 2 px inset keeps the thumb off the frame's own stroke.
        let track = Rect::new(
            rect.x,
            rect.y + 2,
            rect.width.saturating_sub(2),
            rect.height.saturating_sub(4).max(1),
        );
        if let Some(band) = crate::widget::overscroll::scroll_thumb_rect(
            track,
            total_rows,
            visible_rows,
            self.scroll_row,
            4,
            self.show_scrollbar,
        ) {
            context.fill_rect(band, accent.with_alpha(120));
        }
    }
}
