// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tab strip, find bar and gutter.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

impl CodeEditor {
    pub(super) fn draw_tab_strip(
        &mut self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        // Only the tab titles are needed to paint the strip. Cloning
        // `all_buffers` would copy every open buffer's full `text` on every frame,
        // which at a million lines measured 17 ms per frame — the dominant cost in
        // the whole paint path, and invisible in the profile of any single draw
        // helper. Titles are short and there are few of them, so this list is
        // bounded by the tab count rather than by the document.
        let titles: Vec<String> =
            self.model.borrow().all_buffers.iter().map(|buffer| buffer.title.clone()).collect();
        let active = self.active_buffer();
        // The dirty flag is a maintained boolean, not a string comparison: the
        // saved text is a full copy of the document, so comparing against it
        // costs one pass over the file, once per tab, every frame.
        let active_dirty = self.model.borrow().dirty;
        let strip_height = row_height_of(self.config.resolved_line_advance());
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, strip_height as u32),
            chrome.chrome_background,
        );
        let font = self.chrome_font();
        let line_box = context.measure_text("M", font).height as i32;
        let title_y = rect.y + (strip_height - line_box) / 2;
        // Closing is only offered when another buffer would remain. With one file
        // open, `close_buffer` empties the last buffer (there is no pane to close),
        // and a button that silently erases the document is worse than no button.
        let closeable = titles.len() > 1;
        let mut slots: Vec<TabSlot> = Vec::with_capacity(titles.len());
        let mut x = rect.x + 6;
        for (index, title) in titles.iter().enumerate() {
            let dirty = index == active && active_dirty;
            // The dirty marker is a **drawn dot**, not the `U+2022` bullet the label used to
            // carry. No bundled face covers the general-punctuation bullet's block with an outline
            // that clips inside a tab's narrow cell, so the character fell back to an 8x8 bitmap
            // and the tab read as a blob. Keeping it out of the label also keeps the *title* the
            // title: the marker is state, and state drawn as text is state the next reader has to
            // parse out of a string.
            let label = title.as_str();
            // Measured in the face the title is drawn in. `chars × cell_width` is a
            // lie for a proportional face: `main.rs` and `WWWWWWWW` have the same
            // count and different widths, so tabs were both too wide and too narrow.
            let measured = context.measure_text(label, font).width as i32;
            let trailing = TAB_MARKER_SLOT + if closeable { TAB_CLOSE_SLOT } else { 8 };
            let width =
                (TAB_LEFT_PAD + measured + trailing).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH) as u32;
            if x + width as i32 > rect.x + rect.width as i32 {
                break;
            }
            let tab_rect = Rect::new(x, rect.y + 3, width, strip_height.saturating_sub(4) as u32);
            let hovered = self.hovered_tab == Some(index);
            if index == active {
                // The tab is chrome, and the **tab**, not the editor, is what gets the radius: a
                // 104 px-wide strip at 4 px corners reads as a tab, while rounding the whole editor
                // would round the surface a document is read on. `0` here was a literal standing in
                // for "this is not the editor's own face", which is why the resolved radius is the
                // honest value to pass — a theme that rounds its tabs says so once, in `borders`.
                context.face_with_gradient(
                    tab_rect,
                    chrome.surface,
                    self.style().background_gradient.as_ref(),
                    self.style().surface.unwrap_or_default(),
                    self.style().border_radius.unwrap_or(0),
                    Color::BLACK,
                );
                context.draw_rect(tab_rect, chrome.border);
                // A 2 px accent rule along the tab's top edge is the one mark that
                // reads at a glance across a strip with several open files. It is
                // the accent colour rather than a second grey, so it follows the
                // appearance (and the user's accent choice) with no extra config.
                context
                    .fill_rect(Rect::new(tab_rect.x, tab_rect.y, tab_rect.width, 2), chrome.accent);
            } else {
                // Hovering an inactive tab washes it toward the surface it will take
                // when clicked, which is the only affordance that says "this is
                // clickable" on a strip whose active tab is the one with the frame.
                if hovered {
                    context.fill_rounded_rect(
                        tab_rect,
                        self.style().border_radius.unwrap_or(0),
                        chrome.surface.blend(&chrome.ink, 0.04),
                    );
                }
                // A hairline between inactive tabs, so adjacent titles do not read
                // as one run-on label.
                context.draw_line(
                    Point::new(tab_rect.x + tab_rect.width as i32 - 1, tab_rect.y + 6),
                    Point::new(
                        tab_rect.x + tab_rect.width as i32 - 1,
                        tab_rect.y + tab_rect.height as i32 - 6,
                    ),
                    chrome.separator,
                );
            }
            // The label is **vertically centred in the tab**. It used to be placed at
            // `tab_rect.y + strip_height - 6`, which is below the strip: the title was
            // clipped away and the active tab looked empty (visible in the demo's first
            // screenshot).
            //
            // Fitted, not drawn at full width: a tab is clamped to `TAB_MAX_WIDTH`, and
            // an over-long path must ellipsize inside its tab rather than run over the
            // next one. `draw_text_fitted` insets both ends by `TEXT_FIT_MARGIN`, so the
            // bounds are widened by the same margin to keep the title at `TAB_LEFT_PAD`.
            let label_left = tab_rect.x + TAB_LEFT_PAD;
            let label_right = tab_rect.x + tab_rect.width as i32 - trailing;
            let label_bounds = Rect::new(
                label_left - TEXT_FIT_MARGIN as i32,
                title_y,
                (label_right - label_left + 2 * TEXT_FIT_MARGIN as i32).max(0) as u32,
                line_box.max(1) as u32,
            );
            context.draw_text_fitted_cow(
                label_bounds,
                label,
                font,
                if index == active { chrome.ink } else { chrome.dim_ink },
                HorizontalAlignment::Left,
            );
            // The marker sits in its own slot between the title and the close box, so
            // toggling it on and off cannot move either neighbour.
            let marker_center = label_right + TAB_MARKER_SLOT / 2;
            if index == active && dirty {
                let radius = (line_box as f32 * 0.16).round().max(1.0) as u32;
                context.fill_circle_aa(
                    Point::new(marker_center, title_y + line_box / 2 - 1),
                    radius,
                    chrome.ink,
                );
            }
            let close = if closeable {
                let size = 16u32;
                Rect::new(
                    tab_rect.x + tab_rect.width as i32 - TAB_CLOSE_SLOT + 4,
                    tab_rect.y + (tab_rect.height as i32 - size as i32) / 2,
                    size,
                    size,
                )
            } else {
                Rect::new(0, 0, 0, 0)
            };
            if closeable && (index == active || hovered) {
                // The cross is two strokes, not the `×` glyph: no bundled face is
                // guaranteed to cover U+00D7, and a fallback box inside a 16 px target
                // reads as a rendering fault.
                let cx = close.x + close.width as i32 / 2;
                let cy = close.y + close.height as i32 / 2;
                let ink = if hovered { chrome.ink } else { chrome.dim_ink };
                for (from, to) in [
                    (Point::new(cx - 3, cy - 3), Point::new(cx + 3, cy + 3)),
                    (Point::new(cx + 3, cy - 3), Point::new(cx - 3, cy + 3)),
                ] {
                    context.draw_line(from, to, ink);
                }
            }
            slots.push(TabSlot { buffer: index, rect: tab_rect, close });
            x += width as i32 + 2;
        }
        // The hit rects are published only after the whole strip is laid out: the
        // pointer handler may read them at any time, and a half-built list would let
        // a click land on a tab that is not on screen.
        *self.tab_hits.borrow_mut() = slots;
        context.draw_line(
            Point::new(rect.x, rect.y + strip_height),
            Point::new(rect.x + rect.width as i32, rect.y + strip_height),
            chrome.separator,
        );
    }

    pub(super) fn draw_find_bar(
        &mut self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        if !self.find.visible {
            return;
        }
        let top = rect.y + row_height_of(self.config.resolved_line_advance());
        let height = self.find_bar_height();
        context
            .fill_rect(Rect::new(rect.x, top, rect.width, height as u32), chrome.chrome_background);
        context.draw_line(
            Point::new(rect.x, top + height),
            Point::new(rect.x + rect.width as i32, top + height),
            chrome.separator,
        );

        let query_summary = if self.find.query.is_empty() {
            "Find".to_string()
        } else {
            format!("{} / {}", self.find.display_index(), self.find.match_count())
        };
        context.draw_text(
            Point::new(rect.x + 8, top + 20),
            &query_summary,
            self.chrome_font(),
            chrome.dim_ink,
            HorizontalAlignment::Left,
        );
        // Positioned from the **measured** advance of the summary text in the font it is drawn in.
        // The previous version used `space_advance * 8.0` — a hand-written constant multiplied by a
        // made-up column count — which put the query field at a fixed x that had no relation to where
        // the summary actually ended. Measuring both makes the gap between them what it claims to be.
        let summary_font = self.chrome_font();
        let summary_advance = context.backend().measure_text(&query_summary, summary_font).width;
        let query_x = rect.x + 8 + summary_advance as i32 + self.cell_width() as i32;
        context.draw_text(
            Point::new(query_x, top + 20),
            if self.find.query.is_empty() { "(type to search)" } else { &self.find.query },
            self.chrome_font(),
            chrome.ink,
            HorizontalAlignment::Left,
        );

        if self.find.replace_visible {
            let row_two = top + MIN_TOUCH_TARGET + 3;
            context.draw_text(
                Point::new(rect.x + 8, row_two + 18),
                "Replace",
                self.chrome_font(),
                chrome.dim_ink,
                HorizontalAlignment::Left,
            );
            context.draw_text(
                Point::new(query_x, row_two + 18),
                if self.find.replacement.is_empty() {
                    "(replacement)"
                } else {
                    &self.find.replacement
                },
                self.chrome_font(),
                chrome.ink,
                HorizontalAlignment::Left,
            );
        }

        let close = Rect::new(
            rect.x + rect.width as i32 - MIN_TOUCH_TARGET - 4,
            top + 3,
            MIN_TOUCH_TARGET as u32,
            MIN_TOUCH_TARGET as u32,
        );
        // The close affordance is a dismissal, so it carries the theme's error colour
        // rather than the literal red it used to: the same token means "this removes
        // something" everywhere else in the library.
        let close_color = crate::style::semantic_color(crate::style::SemanticColor::Error)
            .unwrap_or(chrome.accent);
        context.draw_rect(close, close_color);
        context.draw_text(
            Point::new(close.x + 9, close.y + 19),
            "x",
            self.chrome_font(),
            close_color,
            HorizontalAlignment::Left,
        );
    }

    pub(super) fn draw_gutter(
        &mut self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        let width = self.gutter_width();
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let height = (bottom - top).max(0) as u32;
        if height == 0 {
            return;
        }
        context.fill_rect(Rect::new(rect.x, top, width as u32, height), chrome.gutter_background);
        context.draw_line(
            Point::new(rect.x + width, top),
            Point::new(rect.x + width, top + height as i32),
            chrome.separator,
        );

        let row_height = self.line_height();
        let map = self.coordinate_map();
        let rows_budget = (height as f32 / row_height).floor() as usize;
        let number_font = self.editor_font();
        // Measured in the face the numbers are drawn in, so the number and the line
        // it numbers sit on one baseline.
        let number_box = context.measure_text("0", &number_font).height as i32;
        let numbers_right = rect.x + GUTTER_LEFT_PAD + self.line_number_slot();
        let fold_x = rect.x + self.fold_marker_offset();
        // Folds are read **once**, not per painted row: `fold_regions()` clones the
        // model's list, and asking it inside the loop was one clone per line per frame.
        let folds = self.fold_regions();
        let active_line = self.cursor.head.line;
        let mut row = 0usize;
        // The windowed iterator, not `visible_document_lines`: the loop below
        // stops early, but materialising the line list first would already have
        // walked the document, which is the cost this avoids.
        for line in self.viewport_document_lines() {
            let segments = self.wrap_segments(line);
            if row >= self.scroll_visual_row + rows_budget {
                break;
            }
            if row >= self.scroll_visual_row {
                let y = map.row_y(row);
                if self.config.show_line_numbers {
                    // Right-aligned against the number slot — the unit column is the
                    // last digit, which is how every terminal, IDE and Zed itself
                    // aligns line numbers. Left-aligned numbers were ragged as soon
                    // as the file passed nine lines.
                    //
                    // Relative mode keeps the caret's line absolute and numbers every
                    // other line by its distance from it (Zed's
                    // `relative_line_numbers`). An absolute number on the caret line is
                    // what lets a relative gutter still answer "which line am I on".
                    let number = self.line_number_label(line);
                    context.draw_text(
                        Point::new(numbers_right, y + (row_height as i32 - number_box) / 2),
                        &number,
                        &number_font,
                        if line == active_line { chrome.ink } else { chrome.dim_ink },
                        HorizontalAlignment::Right,
                    );
                }
                // Fold affordance: a chevron on foldable lines only. The `v`/`>`
                // pair is state; the `.` that used to fill every other row was
                // noise that made the gutter look like it was full of artefacts.
                let foldable = folds.iter().any(|region| region.start_line == line);
                if foldable {
                    let marker = if folds.iter().any(|r| r.folded && r.start_line == line) {
                        ">"
                    } else {
                        "v"
                    };
                    let line_box = context.measure_text("M", self.chrome_font()).height as i32;
                    context.draw_text(
                        Point::new(fold_x, y + (row_height as i32 - line_box) / 2),
                        marker,
                        self.chrome_font(),
                        chrome.dim_ink,
                        HorizontalAlignment::Left,
                    );
                }
                // Severity dot, at the gutter's leading edge so it never collides
                // with the digits. The top-most severity wins.
                if let Some(severity) = self.highest_severity_on_line(line) {
                    let diameter = 5u32;
                    let dot_y = y + (row_height as i32 - diameter as i32) / 2 + 1;
                    context.fill_rect(
                        Rect::new(rect.x + 3, dot_y, diameter, diameter),
                        severity.color(),
                    );
                }
            }
            row += segments.max(1);
        }
    }
}
