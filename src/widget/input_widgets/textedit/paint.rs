// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`TextEdit`]'s paint path: the memoised format/marker tables, the scrollbar and `impl Draw`.
//!
//! Every measurement here reads the model: the wrap table from `super::model`, the format runs
//! derived from the span list and the marker advances saved for the current selection.

use super::model::*;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{Draw, Widget};

use std::collections::HashMap;
use std::rc::Rc;

impl TextEdit {
    /// The format runs the paint path consumes, merged into contiguous equal formats.
    ///
    /// Built from the span boundaries once per `(revision, spans_revision)` pair. The old painter
    /// called `format_at` — a reverse linear scan of `spans` — for *every character of every
    /// painted row*, which is O(characters × spans) per frame (I-P54).
    fn format_runs(&self) -> Rc<Vec<(usize, usize, CharFormat)>> {
        if let Some(cache) = self.format_cache.borrow().as_ref() {
            if cache.revision == self.revision && cache.spans_revision == self.spans_revision {
                return Rc::clone(&cache.runs);
            }
        }
        let len = self.text.len();
        let mut bounds = vec![0usize, len];
        for span in &self.spans {
            bounds.push(span.range.start.min(len));
            bounds.push(span.range.end.min(len));
        }
        bounds.sort_unstable();
        bounds.dedup();
        let mut runs: Vec<(usize, usize, CharFormat)> = Vec::new();
        for window in bounds.windows(2) {
            let (start, end) = (window[0], window[1]);
            if start >= end {
                continue;
            }
            let format = self
                .spans
                .iter()
                .rev()
                .find(|span| span.range.contains(&start))
                .map_or(CharFormat::default(), |span| span.format);
            if let Some(last) = runs.last_mut() {
                if last.1 == start && last.2 == format {
                    last.1 = end;
                    continue;
                }
            }
            runs.push((start, end, format));
        }
        let runs = Rc::new(runs);
        *self.format_cache.borrow_mut() = Some(FormatRunCache {
            revision: self.revision,
            spans_revision: self.spans_revision,
            runs: Rc::clone(&runs),
        });
        runs
    }

    /// The advance of `text[from..to]` for the selection band and the caret, memoised per
    /// revision, selection and font (IP-001).
    ///
    /// `context` is the frame's measurement surface. Only a miss touches the renderer, so a
    /// selection drag reads one table instead of re-shaping every visible row's prefix.
    fn marker_advance(&self, context: &RenderContext, font: &Font, from: usize, to: usize) -> u32 {
        let selection = self.selection_range();
        {
            let cache = self.marker_advance_cache.borrow();
            if let Some(entry) = cache.as_ref() {
                if entry.revision == self.revision
                    && entry.selection == selection
                    && entry.font == *font
                {
                    if let Some(width) = entry.entries.get(&(from, to)) {
                        return *width;
                    }
                }
            }
        }
        let width = context.measure_text(&self.text[from..to], font).width;
        let mut cache = self.marker_advance_cache.borrow_mut();
        let stale = match cache.as_ref() {
            Some(entry) => {
                entry.revision != self.revision
                    || entry.selection != selection
                    || entry.font != *font
            }
            None => true,
        };
        if stale {
            *cache = Some(MarkerAdvanceCache {
                revision: self.revision,
                selection,
                font: font.clone(),
                entries: HashMap::new(),
            });
        }
        if let Some(entry) = cache.as_mut() {
            entry.entries.insert((from, to), width);
        }
        width
    }

    /// Paints the right-edge scrollbar when the value is taller than the viewport (I-015).
    fn draw_scrollbar(&self, context: &mut RenderContext) {
        let Some((track, total, visible)) = self.scrollbar_track() else {
            return;
        };
        let paper = self.style().background_color.unwrap_or(Color::WHITE);
        let ink = self.style().text_color.unwrap_or(Color::BLACK);
        context.fill_rect(track, ink.blend(&paper, 0.18));
        context.fill_rect(self.scrollbar_thumb(track, total, visible), ink.blend(&paper, 0.45));
    }
}

impl Draw for TextEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a queued find/replace intent first, so the caret and the selection this frame
        // paints are the ones the strip asked for (COMP-A8).
        #[cfg(full_widgets)]
        self.poll_find_replace_bar();
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        // I-122: with wrapping off the text is a window onto a long line; the scroll offset moves
        // the ink, the selection and the caret together.
        let text_x = rect.x + padding - self.scroll_x;
        let text_y = rect.y + padding;

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the field, its border and its text unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        // The fallback name matters: the role table is keyed on **role** names, so `text_edit`
        // (the factory name) is not in it and would classify as `Surface`, i.e. the window fill.
        // `line_edit` is, and resolves to the field interior plus the theme's foreground.
        let theme = crate::style::resolved_theme_style_shared("text_edit")
            .or_else(|| crate::style::resolved_theme_style_shared("line_edit"));
        let field_from_theme = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .unwrap_or_else(|| Color::rgb(255, 255, 255));
        // The window fill, read from the shared palette snapshot, so no manager guard is held
        // while anything else touches the theme.
        let window_fill =
            crate::style::palette_shared().map(|colors| colors.background).unwrap_or(Color::WHITE);
        // The filter is on the **resolved** value, not only on the theme's: the active theme is
        // applied to every control before it is drawn, and a control absent from the role table
        // resolves its background to the window fill itself — so letting that value through
        // unfiltered would paint the field in the window's own colour, which is invisible on
        // screen. A caller's own colour still wins.
        let field = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => field_from_theme,
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The border is one step from the field toward the ink, so it is visible on either
        // appearance rather than being a fixed grey a dark theme would render illegible.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| field.blend(&ink, 0.22));

        // Draw background
        context.face_with_gradient_corners(
            rect,
            field,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        // Draw border (rounded with the fill, so the corners are not cut open)
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.text
        };
        // The wrap table is built **once per frame** and read by the ink, the selection band and
        // the caret. The painter used to re-measure the document glyph by glyph while the markers
        // read the table, so every frame laid the document out twice and the two could disagree
        // about where a row ended. It is empty for the placeholder, which is not the document and
        // therefore has no table of its own.
        let rows = if self.text.is_empty() { Rc::new(Vec::new()) } else { self.layout_rows() };
        // Resolved once for the ink, the selection band and the caret: all three place markers
        // with the same font, and the marker block below runs after the ink block's scope.
        let font = self.text_font();
        if !display_text.is_empty() {
            // The placeholder is de-emphasised from the control's own ink rather than being a
            // fixed grey that a dark theme would render illegible.
            let text_color = if self.text.is_empty() { ink.blend(&field, 0.45) } else { ink };
            // `line_wrap` used to be stored, published (`get`/`set`/schema row/round-trip test) and
            // read by nothing: this was a single `draw_text` that ran the whole document off the
            // right edge and past the bottom. It was even commented as a placeholder -- "in real
            // implementation would handle line wrapping" -- which is the shape of a stored promise
            // rather than of a missing line.
            //
            // The value is laid out over the field's *interior*, inset by the same padding the
            // origin uses, so a wrapped line ends where the field ends rather than at the border.
            let interior = Rect::new(
                text_x,
                text_y,
                rect.width.saturating_sub(padding as u32 * 2),
                rect.height.saturating_sub(padding as u32 * 2),
            );
            // A long unwrapped row must not paint over the border or its neighbours.
            context.push_clip(
                rect.x + padding,
                rect.y + padding,
                rect.width.saturating_sub(padding as u32 * 2),
                rect.height.saturating_sub(padding as u32 * 2),
            );
            if self.text.is_empty() {
                self.draw_text_layout(context, display_text, interior, font, text_color);
            } else {
                self.draw_rows(context, &rows, interior, font, text_color);
            }
            context.pop_clip();
        }

        // ── The caret and the selection ──
        //
        // Drawn from the same wrap table the text was laid out with, so the marker sits on the row
        // and column the glyphs actually occupy. Measured any other way it would drift under wrapping,
        // which is the whole reason the table exists.
        if !self.text.is_empty() && self.focused && self.cursor_blink.is_visible() {
            let line_height = self.line_box_height();
            let interior_width = rect.width.saturating_sub(padding as u32 * 2);

            // One measurement surface for the markers (I-P54): the selection edges and the caret
            // prefix are measured against it instead of each allocating its own backend.
            Self::with_measurement(|measure| {
                // The selection band first, so the glyphs stay legible on top of it.
                if let Some((sel_start, sel_end)) = self.selection_range() {
                    let selection_fill =
                        crate::style::semantic_color(crate::style::SemanticColor::Info)
                            .map(|accent| accent.blend(&field, 0.72))
                            .unwrap_or_else(|| field.blend(&ink, 0.18));
                    for (index, span) in rows.iter().enumerate().skip(self.first_visible_row) {
                        // The row's top is its position **in the viewport**, which is what the scroll
                        // offset means: the text, the selection and the caret are all drawn from this
                        // one row index.
                        let top = self.row_top(index - self.first_visible_row);
                        if top + line_height > rect.y + rect.height as i32 {
                            break;
                        }
                        // The overlap of the selection with this row, so a multi-row range highlights
                        // each row only over its own part rather than as one block.
                        let from = sel_start.max(span.start).min(span.end);
                        let to = sel_end.min(span.end).max(from);
                        if to <= from {
                            continue;
                        }
                        let left =
                            text_x + self.marker_advance(measure, font, span.start, from) as i32;
                        let width = self.marker_advance(measure, font, from, to);
                        // The band is clipped to the interior, so a row wider than the field cannot
                        // paint outside the box it belongs to.
                        let width = width.min(interior_width);
                        if width > 0 {
                            context.fill_rect(
                                Rect::new(left, top, width, line_height as u32),
                                selection_fill,
                            );
                        }
                    }
                }

                // Then the caret, at the measured end of its row's prefix. The row it is on is translated
                // through the scroll, so the marker follows the text instead of its document position.
                let caret_row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                if caret_row >= self.first_visible_row {
                    let top = self.row_top(caret_row - self.first_visible_row);
                    if top + line_height <= rect.y + rect.height as i32 {
                        let span = rows
                            .get(caret_row)
                            .copied()
                            .unwrap_or(RowSpan { start: self.text.len(), end: self.text.len() });
                        let caret_in_row = floor_char_boundary(
                            &self.text,
                            self.cursor.min(span.end).max(span.start),
                        );
                        let advance = self.marker_advance(
                            measure,
                            font,
                            span.start,
                            caret_in_row.min(self.text.len()),
                        ) as i32;
                        let caret = crate::style::semantic_color(crate::style::SemanticColor::Info)
                            .unwrap_or(ink);
                        let x = text_x + advance;
                        context.draw_line(
                            crate::core::Point::new(x, top),
                            crate::core::Point::new(x, top + line_height - 2),
                            caret,
                        );
                    }
                }
            });
        }
        // I-015: the scrollbar is painted last so it sits above the text it scrolls.
        self.draw_scrollbar(context);

        // ── Focus ring ──
        //
        // Only a keyboard focus draws it: a pointer (or touch) press focuses the editor without
        // claiming visual focus, so no ring appears under the cursor.
        if self.base.draws_focus_ring() {
            let ring = crate::widget::metrics::FocusRing::for_control(
                rect,
                self.style().border_radius.unwrap_or(0),
            );
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    crate::widget::metrics::focus_ring_color(field.contrast_color()),
                    crate::widget::metrics::FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

impl TextEdit {
    /// Paints the wrap table, starting at the viewport's first visible row.
    ///
    /// # Why the table and not a second walk
    ///
    /// The painter used to re-measure the document glyph by glyph while the caret and the selection
    /// read [`Self::layout_rows`] — two derivations of the same breaks on every frame, one of them
    /// allocating a `String` per character. Reading the table makes the ink a **reader** of the one
    /// wrap rule instead of a second implementation of it.
    ///
    /// It also makes the viewport real: the old walk always began at the interior's top edge, so
    /// `first_visible_row` (which the caret, the hit test and the selection all index from) moved the
    /// markers over text that had not scrolled — and, once the caret's row left the interior, made
    /// the caret disappear while the text stayed. Starting the walk at `first_visible_row` puts the
    /// ink, the band and the caret on one row grid.
    fn draw_rows(
        &self,
        context: &mut RenderContext,
        rows: &[RowSpan],
        interior: Rect,
        font: &Font,
        color: Color,
    ) {
        let line_height = self.line_box_height();
        let align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        let bottom = interior.y + interior.height as i32;
        for (view_row, span) in self.rows_from_viewport(rows) {
            let top = self.row_top(view_row);
            if top + line_height > bottom {
                break;
            }
            // The row's own newline is not ink; it is what the break consumed.
            let end = if self.text[..span.end].ends_with('\n') { span.end - 1 } else { span.end };
            let row = &self.text[span.start..end.max(span.start)];
            if self.spans.is_empty() {
                Self::flush_row(
                    context,
                    row,
                    interior,
                    top,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
            } else {
                self.flush_row_rich(
                    context,
                    span.start,
                    row,
                    interior,
                    top,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
            }
        }
    }

    /// Paints one row whose characters may carry different [`CharFormat`]s.
    ///
    /// The unstyled path stays in [`Self::flush_row`], so a document with no spans draws exactly
    /// as it did before this method existed. Here the row is split into format runs, each run is
    /// measured with its own face (a bold run is wider than the regular one), and the runs are
    /// laid end to end from an origin chosen by the control's horizontal alignment.
    #[allow(clippy::too_many_arguments)]
    fn flush_row_rich(
        &self,
        context: &mut RenderContext,
        row_start: usize,
        row: &str,
        interior: Rect,
        pen_y: i32,
        line_height: i32,
        bottom: i32,
        font: &Font,
        color: Color,
        align: HorizontalAlignment,
    ) -> i32 {
        if pen_y + line_height > bottom {
            return pen_y;
        }
        // Split the row into contiguous runs of one format from the cached, document-wide run
        // table (I-P54). The old loop called `format_at` — a reverse scan of every span — once per
        // character, and then measured every run twice and cloned its face twice.
        let face = |format: CharFormat| {
            let mut face = font.clone();
            if format.bold {
                face = face.with_bold(true);
            }
            if format.italic {
                face = face.with_italic(true);
            }
            face
        };
        let runs = self.format_runs();
        let row_end = row_start + row.len();
        let mut pieces: Vec<(usize, usize, CharFormat, Font, i32)> = Vec::new();
        let first = runs.partition_point(|(_, end, _)| *end <= row_start);
        for (run_start, run_end, format) in runs.iter().skip(first) {
            if *run_start >= row_end {
                break;
            }
            let start = (*run_start).max(row_start) - row_start;
            let end = (*run_end).min(row_end) - row_start;
            if start >= end {
                continue;
            }
            let run_face = face(*format);
            let advance = context.measure_text(&row[start..end], &run_face).width as i32;
            pieces.push((start, end, *format, run_face, advance));
        }
        let total: i32 = pieces.iter().map(|piece| piece.4).sum();
        let origin = match align {
            HorizontalAlignment::Center => interior.x + (interior.width as i32 - total) / 2,
            HorizontalAlignment::Right => interior.x + interior.width as i32 - total,
            HorizontalAlignment::Left => interior.x,
        };
        let mut pen_x = origin;
        for (start, end, format, face, advance) in pieces {
            let run = &row[start..end];
            let ink = format.color.unwrap_or(color);
            context.draw_text(Point::new(pen_x, pen_y), run, &face, ink, HorizontalAlignment::Left);
            if format.underline {
                let underline_y = pen_y + line_height - 2;
                context.draw_line(
                    Point::new(pen_x, underline_y),
                    Point::new(pen_x + advance, underline_y),
                    ink,
                );
            }
            pen_x += advance;
        }
        pen_y + line_height
    }

    /// The wraps to paint, in view order, as `(view_row, span)`.
    ///
    /// `view_row` is the row's index **from the top of the viewport**, which is what
    /// [`Self::row_top`] and the caret both measure with. The painter stops early at the interior's
    /// bottom; this yields every remaining row, so a caller can ask "what would be painted first"
    /// without a render context.
    ///
    /// It starts at [`Self::first_visible_row`] rather than at zero because the ink and the markers
    /// have to share one row grid — see [`Self::draw_rows`] for the defect that came from the
    /// painter ignoring the scroll offset.
    pub(crate) fn rows_from_viewport<'a>(
        &self,
        rows: &'a [RowSpan],
    ) -> impl Iterator<Item = (usize, &'a RowSpan)> {
        let first = self.first_visible_row;
        rows.iter().enumerate().skip(first).map(move |(index, span)| (index - first, span))
    }

    /// Paints `text` inside `interior`, honouring [`Self::line_wrap`].
    ///
    /// # The two layouts
    ///
    /// * **Wrapped** (`line_wrap: true`, the default): a line that would cross the interior's right
    ///   edge continues on the next row, so the whole document is readable without scrolling
    ///   sideways. This is the mode a document editor wants, and it is what the field's own name
    ///   promises.
    /// * **Unwrapped** (`line_wrap: false`): each `\n`-separated line is drawn in full and a line
    ///   wider than the field is clipped at the border rather than folded. This is the mode a source
    ///   editor wants, where folding a long line makes the indentation invisible.
    ///
    /// # Why the rows are walked manually
    ///
    /// The render context has `draw_text` and a per-`\n` `draw_text_line`, but neither wraps: the
    /// break has to be chosen *by the text*, which needs the font's own advance widths. So the walk
    /// measures glyph by glyph and breaks where the measurement says the row is full -- one
    /// measurement per character, which for a field-sized document is what keeps a long paragraph
    /// from being drawn as one unbounded string.
    ///
    /// This is the **placeholder's** painter. A real document is painted from the wrap table by
    /// [`Self::draw_rows`], so the ink and the caret share one derivation instead of two.
    fn draw_text_layout(
        &self,
        context: &mut RenderContext,
        text: &str,
        interior: Rect,
        font: &Font,
        color: Color,
    ) {
        let line_height = context.measure_text("M", font).height.max(1) as i32;
        let mut pen_y = interior.y;
        // Each laid-out row is placed by this control's horizontal alignment, so a centred or
        // right-aligned document is positioned within the interior rather than always flush with
        // its leading edge. The rows are laid out as one block below, so the alignment is a
        // property of every row rather than of the run.
        let align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        // A row is skipped rather than clipped when it would cross the interior's bottom: a
        // half-height row of glyphs reads as a rendering error, the same rule the list view's rows
        // follow.
        let bottom = interior.y + interior.height as i32;
        let mut row = String::new();
        let mut row_width = 0.0f32;
        for ch in text.chars() {
            if ch == '\n' {
                pen_y = Self::flush_row(
                    context,
                    &row,
                    interior,
                    pen_y,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
                row.clear();
                row_width = 0.0;
                continue;
            }
            let advance = {
                // Measured from the glyph's own UTF-8 bytes: `ch.to_string()` allocated a `String`
                // per character, once per painted frame.
                let mut glyph = [0u8; 4];
                context.measure_text(ch.encode_utf8(&mut glyph), font).width
            };
            // The break is taken *before* the glyph that would overflow, so the last glyph on a row
            // is never the one that crossed the edge. A row is never left empty by the break: a
            // single glyph wider than the interior has to go somewhere, and an empty row followed by
            // the overflowing glyph is worse than a row that is one glyph too wide.
            if self.line_wrap
                && row_width + advance as f32 > interior.width as f32
                && !row.is_empty()
            {
                pen_y = Self::flush_row(
                    context,
                    &row,
                    interior,
                    pen_y,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
                row.clear();
                row_width = 0.0;
            }
            row.push(ch);
            row_width += advance as f32;
        }
        if !row.is_empty() {
            Self::flush_row(
                context,
                &row,
                interior,
                pen_y,
                line_height,
                bottom,
                font,
                color,
                align,
            );
        }
    }

    /// Draws one laid-out row at `pen_y` and returns the next row's origin.
    ///
    /// Returns `pen_y` unchanged when the row would cross `bottom`, so the caller needs no bound of
    /// its own and cannot advance the pen past the interior it was given.
    ///
    /// `align` is the control's own horizontal alignment, applied to the row's `interior` rather
    /// than to a private origin, so a centred or right-aligned row is measured against the same box
    /// the wrap used. `draw_text_fitted` is what turns that alignment into an origin, and it fits
    /// the row into the interior so a run wider than the box is clipped at its trailing edge.
    #[allow(clippy::too_many_arguments)]
    fn flush_row(
        context: &mut RenderContext,
        row: &str,
        interior: Rect,
        pen_y: i32,
        line_height: i32,
        bottom: i32,
        font: &Font,
        color: Color,
        align: HorizontalAlignment,
    ) -> i32 {
        if pen_y + line_height > bottom {
            return pen_y;
        }
        context.draw_text_fitted_cow(
            Rect::new(interior.x, pen_y, interior.width, line_height as u32),
            row,
            font,
            color,
            align,
        );
        pen_y + line_height
    }
}
