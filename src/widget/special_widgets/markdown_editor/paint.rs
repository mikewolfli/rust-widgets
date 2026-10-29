// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Painting for `MarkdownEditor`.

use super::*;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::{Draw, Widget};
use std::rc::Rc;

impl Draw for MarkdownEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The fence table depends on every line above it, so it is rebuilt once per revision —
        // before the style borrow is taken, because building it needs `&mut self`.
        self.ensure_blocks();
        if self.show_outline {
            // Reads the same heading list `outline()` reports, and marks the row the caret is in.
            self.sync_outline();
        }

        // The mounted companions follow the editor's own rectangle; doing it here (rather than in
        // `new`) means a resized editor moves its side panel and find bar with it.
        self.layout_companions();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen, because the surface, the border
        // and every piece of text were hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(252, 252, 253));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(194, 201, 213));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(59, 72, 92));
        // The header is the dimmest ink that still reads, and the caret line is the
        // accent: both derived from the resolved ink and surface, so they move with
        // the appearance instead of being fixed literals.
        //
        // The caret line is the accent colour because it *means* "the line being edited",
        // so the hue is part of the information and must survive. It cannot be pushed
        // straight onto the surface unchecked, though: on the light appearance the editor
        // surface resolves to a mid grey and `colors.primary` is the saturated Material
        // blue, which measured 1.51:1 — the very line the caret is on became the least
        // readable line in the editor. `legible_on` keeps the hue and guarantees the ratio.
        let header_ink = ink.blend(&surface, 0.35);
        let cursor_ink = crate::style::palette_shared()
            .map(|colors| colors.primary.legible_on(surface, 4.5))
            .unwrap_or_else(|| ink.contrast_color());

        context.face_with_gradient_corners(
            rect,
            surface,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // The counts come from the revision-keyed cache, so a still frame walks no document.
        // The line is formatted into the editor's own buffer: `format!` allocated a fresh
        // `String` per frame for text that only changes with the document or the find query.
        let metrics = self.metrics_snapshot();
        let mut header = core::mem::take(&mut self.header_buffer);
        header.clear();
        {
            use core::fmt::Write;
            let _ = if self.find.visible {
                write!(header, "Find: {}   (Enter next, Esc close)", self.find.query)
            } else if self.preview_mode {
                write!(
                    header,
                    "Markdown Preview  lines:{} words:{} headings:{}",
                    metrics.lines, metrics.words, metrics.headings
                )
            } else {
                write!(
                    header,
                    "Markdown Edit  lines:{} words:{} headings:{}",
                    metrics.lines, metrics.words, metrics.headings
                )
            };
        }
        // The mounted find bar replaces the plain header text while it is open: it paints the
        // query, the match counter and its policy toggles, and a second header underneath would
        // be an unreadable overprint.
        if self.find.visible {
            self.find_bar.draw(context);
        } else {
            // Fitted to the control's width: the header is a long line of counts, and a narrow
            // editor previously drew it straight past its own right edge.
            let header_band = Rect::new(rect.x + 8, rect.y + 4, rect.width.saturating_sub(16), 14);
            context.draw_text_fitted_cow(
                header_band,
                &header,
                effective_font(style),
                header_ink,
                HorizontalAlignment::Left,
            );
        }
        // The buffer goes back into the editor for the next frame; it was only borrowed for the
        // duration of the call above.
        self.header_buffer = header;

        let start = self.visible_start();
        let count = self.visible_line_count();
        // The line total comes from the cached offset tables, not from re-counting the document's
        // newlines on every frame.
        let line_total = self.edit_line_count();
        let font = effective_font(style);
        // Selection endpoints, resolved once. `line_col_of` is two document walks; doing it per
        // row would be quadratic, and the whole point of this path is that a frame is cheap.
        let selection =
            self.selection.map(|(first, last)| (self.line_col_of(first), self.line_col_of(last)));
        let scrollbar_width = if self.scrollbar_track().is_some() { 12 } else { 0 };
        let outline_width = self.outline_rect().map_or(0, |panel| panel.width as i32);
        let row_width = rect.width.saturating_sub(24 + scrollbar_width + outline_width as u32);

        {
            let text = &self.text;
            let cache = &mut self.preview_cache;
            let blocks = &self.blocks;
            let table_headers = &self.table_header;
            let gfm = self.gfm_extensions;
            let highlight = self.code_highlight;
            for (row, line) in text.split('\n').skip(start).take(count).enumerate() {
                let idx = start + row;
                let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
                // Rows below the control are not painted, which keeps a short editor from drawing
                // past its own bottom edge.
                if y > rect.y + rect.height as i32 - TEXT_BOTTOM_INSET {
                    break;
                }
                let color = if idx == self.cursor_line { cursor_ink } else { ink };
                let band = Rect::new(rect.x + 12, y, row_width, TEXT_ROW_HEIGHT as u32);

                // The selection band is drawn behind the text, trimmed to the exact characters
                // on the first and last rows.
                if let Some(((first_line, first_col), (last_line, last_col))) = selection {
                    if idx >= first_line && idx <= last_line {
                        let line_chars = line.chars().count();
                        let from = if idx == first_line { first_col } else { 0 };
                        let to = if idx == last_line { last_col } else { line_chars };
                        // Both edges are measured straight off the borrowed line: a prefix
                        // `String` per visible row was an allocation per row per frame.
                        let from_byte = byte_offset_of_char(line, from);
                        let to_byte = byte_offset_of_char(line, to).max(from_byte);
                        let x = band.x + estimate_text_width(&line[..from_byte], font, 1.0) as i32;
                        let width = estimate_text_width(&line[from_byte..to_byte], font, 1.0);
                        context.fill_rect(
                            Rect::new(x, y, width.max(1), TEXT_ROW_HEIGHT as u32),
                            cursor_ink.with_alpha(60),
                        );
                    }
                }

                if self.preview_mode {
                    let block = blocks.get(idx).copied().unwrap_or(BlockStart::Normal);
                    // The key is the row's content and parse context, never its index, so an
                    // edit that shifts the page keeps every unchanged row parsed.
                    let key = PreviewKey::new(
                        line,
                        block,
                        gfm,
                        highlight,
                        table_headers.get(idx).copied().unwrap_or(false),
                        font,
                    );
                    let parsed = match cache.get(&key).cloned() {
                        Some(parsed) => parsed,
                        None => {
                            let parsed = Rc::new(parse_preview_line(
                                line,
                                block,
                                gfm,
                                highlight,
                                table_headers.get(idx).copied().unwrap_or(false),
                                font,
                                context,
                            ));
                            cache.insert(key, parsed.clone());
                            parsed
                        }
                    };
                    draw_parsed_line(context, band, &parsed, font, color, cursor_ink, surface);
                } else {
                    // Fitted to the pane, so a long line ends in an ellipsis inside the control
                    // instead of painting over the widget beside it.
                    context.draw_text_fitted_cow(
                        band,
                        line,
                        font,
                        color,
                        HorizontalAlignment::Left,
                    );
                }
            }
            // The cache is keyed by content and parse context, so entries for rows that moved or
            // changed are simply unused. When the table outgrows its bound only the entries that
            // no longer describe a live row are dropped: clearing it rebuilt every visible line
            // on the next frame.
            let bound = line_total.saturating_mul(4).saturating_add(64);
            if cache.len() > bound {
                let live: std::collections::HashSet<PreviewKey> = text
                    .split('\n')
                    .enumerate()
                    .map(|(index, line)| {
                        PreviewKey::new(
                            line,
                            blocks.get(index).copied().unwrap_or(BlockStart::Normal),
                            gfm,
                            highlight,
                            table_headers.get(index).copied().unwrap_or(false),
                            font,
                        )
                    })
                    .collect();
                cache.retain(|key, _| live.contains(key));
            }
        }

        if let Some(track) = self.scrollbar_track() {
            context.fill_rect(track, ink.blend(&surface, 0.10));
            if let Some(thumb) = self.scrollbar_thumb() {
                context.fill_rect(thumb, ink.with_alpha(110));
            }
        }

        // The caret, the composition and the selection are edit-mode furniture: preview has no
        // caret because there is nothing to edit there.
        if !self.preview_mode {
            let start = self.visible_start();
            if let Some(row) =
                self.cursor_line.checked_sub(start).filter(|row| *row < self.visible_line_count())
            {
                let (line_start, line_end) = self.cursor_line_span();
                let caret_line = &self.text[line_start..line_end];
                let caret_byte = byte_offset_of_char(caret_line, self.cursor_col);
                let x =
                    rect.x + 12 + estimate_text_width(&caret_line[..caret_byte], font, 1.0) as i32;
                let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
                if y + 14 <= rect.y + rect.height as i32 && x <= rect.x + rect.width as i32 - 12 {
                    if let Some(preedit) = &self.preedit {
                        // The composition is drawn where the caret is and semi-transparent, so a
                        // user can see what has not been committed yet — the previous build drew
                        // nothing at all and the text appeared only after a candidate landed.
                        context.draw_text(
                            Point::new(x, y),
                            &preedit.text,
                            font,
                            cursor_ink.with_alpha(160),
                            HorizontalAlignment::Left,
                        );
                    }
                    if self.is_caret_visible() || self.preedit.is_some() {
                        context.fill_rect(Rect::new(x, y - 2, 1, 16), cursor_ink);
                    }
                }
            }
        }

        // The mounted outline is painted last, over the text column it reserved space for, so a
        // selected row is fully visible even when the text underneath is long.
        if self.outline_rect().is_some() {
            self.outline_panel.draw(context);
        }
    }
}
