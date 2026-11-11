// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TableWidget drawing.

use super::*;

impl Draw for TableWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // A source change signal cannot borrow the control mutably, so it only marks the state
        // dirty; the repair runs here, once, before anything reads a row or a column index.
        self.drain_data_updates();
        let rect = self.base.geometry();
        // The table's content sits inside its own surface, not flush against it. Every row
        // used to start at the control's literal top edge, so the first row's text began at
        // y=0 — with no header row above it and no margin anywhere, the glyph box's top edge
        // landed exactly on the frame's own stroke. The rows are laid out from the inset
        // content box instead, and the header row the inset reserves is what the first
        // content row then sits below.
        let content = ControlMetrics::band_inset(rect, TABLE_INSET);
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the grid lines, the focused-row highlight
        // and the text colour used to be hardcoded literals, so light and dark rendered
        // identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("table");
        let mut surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A panel
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        // One palette read for the surface, grid and accent roles. Each of these used to take
        // the global theme lock separately, so every table took three locks per frame.
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        // A disabled table used to gate input only (its event handler returns early), so the
        // surface, grid and text stayed at full contrast. The ink recedes toward the surface it
        // is painted on — the crate's one disabled-ink rule — and the grid, header band and
        // border below are all derived from that ink and surface, so they move together while the
        // rows keep their geometry. `table:disabled` carries the palette's own disabled token.
        let ink = self.base.disabled_ink_on(ink, surface);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The row and column separators are the theme's `outline_variant` — the **weak**
        // separator role — so a table's grid lines are visibly weaker than the focus ring,
        // which uses the stronger `outline`. Both previously resolved to the same grey,
        // because the grid line was a blend of the surface and the ring read `secondary`;
        // reading the two roles keeps them distinct in every appearance, and lets a theme
        // dim its grid lines without touching every control that draws one.
        let grid_ink = palette
            .as_ref()
            .map(|colors| colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.12));
        // The focused row is a selection state, so it reads the theme's accent token and is
        // laid over the surface, which keeps it legible in either appearance.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let focused_bg = surface.blend(&accent, 0.30);

        // Draw background and border over the control's own rectangle, so the surface is
        // the whole control; the rows below are inset from it.
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
        // Header row: the band the content rows begin below. It carries the surface's own
        // ink as a rule, so a table with a model reads as a table before its first row.
        let column_count = self.column_count();
        if let Some(header) = self.header_band().filter(|_| column_count > 0) {
            context.fill_rect(header, surface.blend(&ink, 0.06));
            context.draw_line(
                crate::core::Point::new(header.x, header.y + header.height as i32),
                crate::core::Point::new(
                    header.x + header.width as i32,
                    header.y + header.height as i32,
                ),
                border,
            );
            let font = effective_font(style);
            let mut left = content.x - self.horizontal_offset() as i32;
            let rtl = crate::widget::view_widgets::rtl_columns::is_rtl();
            for column in 0..column_count {
                let width = self.resolved_column_width(column, content.width);
                if column >= self.scroll_column {
                    let band_x = if rtl {
                        crate::widget::view_widgets::rtl_columns::mirror_span(left, width, content)
                    } else {
                        left
                    };
                    // LTR stops once the next column starts past the right edge; RTL walks
                    // leftwards, so it stops once a column ends before the left edge.
                    let out_of_view = if rtl {
                        band_x + width as i32 <= content.x
                    } else {
                        band_x >= content.x + content.width as i32
                    };
                    if out_of_view {
                        break;
                    }
                    let band = Rect::new(band_x, header.y, width, header.height);
                    // The title precedence lives in one accessor: explicit structure, then a
                    // host label, then the model/source's own column name (RW-5).
                    if let Some(label) = self.column_title(column) {
                        context.draw_text_fitted_ellipsis(
                            context.text_line(band, font),
                            &label,
                            font,
                            ink,
                            crate::widget::view_widgets::rtl_columns::leading_alignment(),
                        );
                    }
                }
                left += width as i32;
            }
        }
        // The rows begin at the header's bottom edge, which is what keeps the first content
        // row off y=0 (the defect the inset exists to fix).
        // A hovered row reads the same accent at a **lighter** weight than the focused row: it is
        // pointer position rather than a committed selection, so it must not compete with the row
        // that is actually selected.
        let hovered_bg = surface.blend(&accent, 0.12);

        let row_count = self.row_count();
        if row_count == 0 {
            // Nothing to paint: the view's own placeholder (RW-17) is drawn in the rows band so a
            // host-supplied caption sits inside the control rather than on top of it.
            let band = if self.header_visible { self.rows_band() } else { content };
            paint_collection_placeholder(
                context,
                band,
                &self.placeholder,
                ink,
                effective_font(style),
            );
            return;
        }

        // A bound source is read through one bounded window per frame (RW-7). The window is taken
        // out of the field so the paint loop can borrow `self` mutably for its text buffer while
        // the cells are borrowed from the local.
        if self.data_source.is_some() {
            let band = self.rows_band();
            let content_width = content.width;
            let needed_rows = self.visible_row_span(band);
            let needed_columns = self.visible_column_span(content_width);
            let revision = self.data_source.as_ref().map(|source| source.revision()).unwrap_or(0);
            let generation = self.data_generation.load(Ordering::Relaxed);
            let matches = self.window_cache.as_ref().is_some_and(|window| {
                window.generation == generation
                    && window.revision == revision
                    && window.row_start == self.scroll_row
                    && window.column_start == self.scroll_column
                    && window.rows >= needed_rows
                    && window.columns >= needed_columns
            });
            if !matches {
                if let Some(source) = self.data_source.clone() {
                    let cells = source.fetch_window(
                        self.scroll_row,
                        needed_rows,
                        self.scroll_column,
                        needed_columns,
                    );
                    self.window_cache = Some(TableWindow {
                        generation,
                        revision,
                        row_start: self.scroll_row,
                        column_start: self.scroll_column,
                        rows: needed_rows,
                        columns: needed_columns,
                        cells,
                    });
                    self.window_fetches = self.window_fetches.saturating_add(1);
                }
            }
        }
        let window = self.window_cache.take();

        // Draw grid from the model or from the source's window.
        {
            let col_count = column_count;
            let current_row = self.focused_row;
            let font = effective_font(style);
            // The capacity of this buffer survives across frames; see `TableWidget::cell_text`.
            //
            // The column offsets are accumulated once for the whole paint instead of being
            // re-summed per cell. With a single `set_column_width` override the accessor walked
            // `0..index` for every call, making one frame O(R·C²) map lookups; the accumulation
            // below is exactly what it computes, in one pass, and gives the same geometry.
            //
            // The row y is likewise accumulated: `row_offset(r)` summed `0..r` for every visible
            // row, so one frame re-walked the prefix O(R²) times. `scroll_offset_px()` already is
            // `row_offset(scroll_row)`, which cancels the first row's offset, so the band's own
            // top is the starting point and each step adds the row it just painted.
            let band = self.rows_band();
            let mut y = band.y;
            for r in self.scroll_row..row_count {
                let row_h = self.resolved_row_height(r) as i32;
                if y + row_h > band.y + band.height as i32 {
                    break;
                }
                if y < band.y {
                    y += row_h;
                    continue;
                }
                let row_box = crate::core::Rect::new(band.x, y, band.width, row_h as u32);
                if Some(r) == current_row {
                    context.fill_rect(row_box, focused_bg);
                } else if Some(r) == self.hovered_row {
                    context.fill_rect(row_box, hovered_bg);
                }
                let mut col_x = content.x - self.horizontal_offset() as i32;
                let rtl = crate::widget::view_widgets::rtl_columns::is_rtl();
                for c in 0..col_count {
                    // The column's box follows the same composition the hit test and the header
                    // read, so a click, the heading and the cell cannot disagree about which
                    // column an x belongs to.
                    let col_w = self.resolved_column_width(c, content.width);
                    let ltr_x = col_x;
                    col_x += col_w as i32;
                    if c < self.scroll_column {
                        continue;
                    }
                    let x = if rtl {
                        crate::widget::view_widgets::rtl_columns::mirror_span(ltr_x, col_w, content)
                    } else {
                        ltr_x
                    };
                    // Past the trailing edge nothing can be painted — the cell is clipped away —
                    // so stop instead of fetching data and measuring text for every column off
                    // screen. RTL walks leftwards, so its trailing edge is the left one.
                    let out_of_view = if rtl {
                        x + col_w as i32 <= content.x
                    } else {
                        x >= content.x + content.width as i32
                    };
                    if out_of_view {
                        break;
                    }
                    self.cell_text.clear();
                    // One read per cell: the delegate and the fallback text draw want the same
                    // value. A source is read through the cached window (zero further source
                    // calls per cell); a model is read through its borrowing `fill_data`.
                    let has_cell = match window.as_ref().and_then(|window| window.cell(r, c)) {
                        Some(text) => {
                            self.cell_text.push_str(text);
                            true
                        }
                        None => match self.model.as_ref() {
                            Some(model) => model.fill_data(r, c, &mut self.cell_text),
                            None => false,
                        },
                    };
                    let mut painted_by_delegate = false;
                    if let Some(delegate) = self.delegate.as_ref() {
                        let cell = crate::core::Rect::new(x, y, col_w, row_h as u32);
                        painted_by_delegate = delegate.paint(
                            context,
                            cell,
                            r,
                            c,
                            if has_cell { Some(self.cell_text.as_str()) } else { None },
                        );
                    }
                    if !painted_by_delegate && has_cell {
                        // The cell is the band: a text origin of `y + row_h / 2` would put the
                        // glyph box's *top* edge on the row's middle line and draw every label
                        // half a line low. `text_line` derives the centred line box instead, and
                        // it also bounds the label so a long cell value cannot run into the next
                        // column.
                        let cell = crate::core::Rect::new(x, y, col_w, row_h as u32);
                        if !self.cell_text.is_empty() {
                            context.draw_text_fitted_ellipsis(
                                context.text_line(cell, font),
                                &self.cell_text,
                                font,
                                ink,
                                crate::widget::view_widgets::rtl_columns::leading_alignment(),
                            );
                        }
                    }
                    // Draw column separator. The boundary is this column's trailing edge: the
                    // left edge in LTR, the right edge in RTL.
                    if c + 1 < col_count {
                        let edge = if rtl { x } else { x + col_w as i32 };
                        context.draw_line(
                            crate::core::Point::new(edge, y),
                            crate::core::Point::new(edge, y + row_h),
                            grid_ink,
                        );
                    }
                }
                // Draw row separator
                if r + 1 < row_count {
                    context.draw_line(
                        crate::core::Point::new(content.x, y + row_h),
                        crate::core::Point::new(content.x + content.width as i32, y + row_h),
                        grid_ink,
                    );
                }
                y += row_h;
            }
        }

        // Hand the window back for the next frame.
        self.window_cache = window;

        // Tab and Shortcut focus must be visible on the table as on every other control (U3-04).
        let radius = self.style().resolved_corner_radii().to_array()[0];
        self.base.draw_focus_ring_if_needed(context, rect, radius, surface);
    }
}
