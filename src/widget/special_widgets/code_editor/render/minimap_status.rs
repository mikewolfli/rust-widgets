// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scrollbar, minimap and status row.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

impl CodeEditor {
    pub(super) fn draw_scrollbar(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        let total = self.total_visual_rows().max(1);
        let rows = self.visible_rows().max(1);
        if total <= rows {
            return;
        }
        let width = self.scrollbar_width();
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let track_height = (bottom - top).max(1) as u32;
        let track = Rect::new(
            rect.x + rect.width as i32 - width - self.minimap_width(),
            top,
            width as u32,
            track_height,
        );
        context.fill_rect(track, chrome.scrollbar_track);
        let thumb_height = ((rows as f32 / total as f32) * track_height as f32).max(12.0) as u32;
        let max_offset = total.saturating_sub(rows) as f32;
        let ratio = if max_offset <= 0.0 {
            0.0
        } else {
            (self.scroll_visual_row as f32 / max_offset).clamp(0.0, 1.0)
        };
        let thumb_y =
            top + ((track_height.saturating_sub(thumb_height)) as f32 * ratio).round() as i32;
        context.fill_rect(
            Rect::new(track.x + 2, thumb_y, (width - 4).max(2) as u32, thumb_height),
            chrome.scrollbar_thumb,
        );
    }

    /// Draws the minimap.
    ///
    /// A minimap is a *summary*: one short bar per strip row, not one per
    /// document line. The previous version iterated every line of the document
    /// on every frame and issued a `fill_rect` for each non-blank one, which cost
    /// 10.5 ms per frame at a million lines — the file opened and the editor
    /// stopped responding.
    ///
    /// The per-bucket summaries are maintained by
    /// [`CodeEditor::rebuild_minimap`] on edits and read here, so painting is
    /// bounded by the strip height and a million-line file costs the same per
    /// frame as a fifty-line one.
    ///
    /// # What changed from the first version
    ///
    /// Every bar was one pixel of `scrollbar_thumb` grey on a background 1.5 % away
    /// from the surface: the strip read as an empty column with a few specks in it.
    /// Now each bar carries its line's **tone** (comment, string, code) in the
    /// syntax palette, two pixels tall where the scale allows, and the strip has its
    /// own ground and a leading hairline so its extent is visible. The viewport is a
    /// translucent accent band instead of an opaque one, which was heavy enough to
    /// hide the bars underneath it.
    pub(super) fn draw_minimap(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        let width = self.minimap_width();
        if width == 0 {
            return;
        }
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let height = (bottom - top).max(1) as u32;
        let x = rect.x + rect.width as i32 - width;
        context.fill_rect(Rect::new(x, top, width as u32, height), chrome.minimap_background);
        context.draw_line(Point::new(x, top), Point::new(x, top + height as i32), chrome.separator);

        let total_rows = self.total_visual_rows().max(1);
        let rows_visible = self.visible_rows().max(1);
        let rows = self.minimap_buckets.len();
        if rows == 0 {
            return;
        }
        // A thumbnail of a code page: columns are scaled to a nominal 100-column
        // width, so an 80-column line stops short of the strip's edge the way it
        // stops short of the buffer's.
        const PAGE_COLUMNS: f32 = 100.0;
        let inner = (width - 10).max(1) as f32;
        let row_height = height as f32 / rows as f32;
        // Bucket pitch. A short document does not get stretched over the whole strip:
        // 120 buckets over 561 px is 4.7 px each, and a 2 px bar in every 4.7 px slot
        // drew a **dashed column** instead of a page. Capping the pitch at 3 px keeps
        // the document compacted at the top (the way a minimap shows a short file), and
        // the bar is as tall as the pitch so consecutive lines touch instead of
        // flickering.
        let pitch = row_height.min(3.0);
        let bar_height = pitch.round().max(1.0) as u32;
        for (bucket, summary) in self.minimap_buckets.iter().enumerate() {
            if summary.blank {
                continue;
            }
            let y = top + (bucket as f32 * pitch).round() as i32;
            let indent = (summary.indent as f32 / PAGE_COLUMNS * inner).min(inner * 0.5);
            let bar_width =
                (summary.length as f32 / PAGE_COLUMNS * inner).clamp(1.0, inner - indent);
            let kind = match summary.tone {
                super::super::editor::MiniTone::Comment => TokenKind::Comment,
                super::super::editor::MiniTone::String => TokenKind::String,
                super::super::editor::MiniTone::Plain => TokenKind::Plain,
            };
            // Syntax hues are the *identity* of the bar, but at two pixels tall a full
            // saturation stripe is a distraction; two thirds toward the hue keeps the
            // texture while staying quiet.
            let ink = match kind {
                TokenKind::Plain => chrome.scrollbar_thumb,
                _ => self.palette.color_for(kind).blend(&chrome.minimap_background, 0.25),
            };
            context
                .fill_rect(Rect::new(x + 5 + indent as i32, y, bar_width as u32, bar_height), ink);
        }
        // Viewport indicator. A file that fits in one screen has nothing to indicate
        // — drawing the marker there produced a full-height accent frame around the
        // strip (visible in the first run of this demo).
        if total_rows > rows_visible {
            let scale = height as f32 / total_rows as f32;
            let viewport_top = top + (self.scroll_visual_row as f32 * scale).round() as i32;
            let viewport_height = ((rows_visible as f32 * scale).max(8.0)) as u32;
            let band = Rect::new(x, viewport_top, width as u32, viewport_height.min(height));
            // Translucent, not opaque: the bars under the viewport must stay readable.
            context.draw_rect(band, chrome.accent.blend(&chrome.surface, 0.45));
            context.fill_rect(band, chrome.accent.blend(&chrome.surface, 0.9));
            context.draw_rect(band, chrome.accent.blend(&chrome.surface, 0.55));
        }
    }

    pub(super) fn draw_status_bar(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        chrome: &EditorChrome,
    ) {
        let height = self.status_bar_height();
        let top = rect.y + rect.height as i32 - height;
        context
            .fill_rect(Rect::new(rect.x, top, rect.width, height as u32), chrome.chrome_background);
        context.draw_line(
            Point::new(rect.x, top),
            Point::new(rect.x + rect.width as i32, top),
            chrome.separator,
        );

        let selected = self.selection_char_count();
        let fold_note = if self.folded_line_count() > 0 {
            format!("  folded:{}", self.folded_line_count())
        } else {
            String::new()
        };
        let caret_note = if self.has_multiple_cursors() {
            format!("  carets:{}", self.cursors().len())
        } else {
            String::new()
        };
        // The status row is a **single band**: the label and the caret position are laid
        // out against the same rectangle and fitted inside it, so a long language name or
        // a wide selection count shortens instead of running off the editor's edge. The two
        // used to be placed from opposite edges of the whole width with no bound at all,
        // which is why `Plain Text  1 lines` and `Ln 1, Col 1` both ended up painted past
        // the control.
        let status_band = Rect::new(rect.x + 8, top + 5, rect.width.saturating_sub(20), 16);
        let font = self.chrome_font();
        // A reduced level is part of the editor's state, so it belongs on the
        // status row: a silently disabled feature reads as a bug, while a named
        // one reads as a decision the user can act on.
        let scale_note = match self.scale.notice() {
            Some(note) => format!("  [{note}]"),
            None => String::new(),
        };
        let left_label = format!(
            "{}  {} lines{}{}{}",
            self.language_name(),
            self.line_count(),
            scale_note,
            fold_note,
            caret_note
        );
        let caret_rect = {
            // Space between the status row's right-hand items, in pixels.
            const STATUS_ITEM_GAP: i32 = 12;
            let right_label = if selected > 0 {
                format!(
                    "{} selected   Ln {}, Col {}",
                    selected,
                    self.cursor.head.line + 1,
                    self.cursor.head.column + 1
                )
            } else {
                format!("Ln {}, Col {}", self.cursor.head.line + 1, self.cursor.head.column + 1)
            };
            let width = context.measure_text(&right_label, font).width;
            // `draw_text_fitted` insets **both** ends by `TEXT_FIT_MARGIN`, so a rect sized to the
            // measured width truncates the very label it was measured from ("Ln 1, C...").
            // Reserve the inset on both sides; the right edge stays on the band, so the label
            // still ends where it used to.
            let fit_padding = TEXT_FIT_MARGIN * 2;
            let strip_width = width.saturating_add(fit_padding).min(status_band.width);
            let strip = Rect::new(
                (status_band.x + status_band.width as i32 - strip_width as i32).max(status_band.x),
                status_band.y,
                strip_width,
                status_band.height,
            );
            // The diagnostic tally is a summary of severity counts, so it reads the dimmest
            // chrome ink. It sits **left of** the caret label: both used to be right-aligned
            // into the same band, so `E0 W1 I1` was painted over `Ln 1, Col 1` and neither
            // reached the reader.
            let diagnostics = format!(
                "E{} W{} I{}",
                self.marker_count(MarkerSeverity::Error),
                self.marker_count(MarkerSeverity::Warning),
                self.marker_count(MarkerSeverity::Info)
            );
            let tally_width = context.measure_text(&diagnostics, font).width + fit_padding;
            let tally_right = (strip.x - STATUS_ITEM_GAP).max(status_band.x);
            let tally_x = (tally_right - tally_width as i32).max(status_band.x);
            let tally =
                Rect::new(tally_x, status_band.y, (tally_right - tally_x).max(0) as u32, 12);
            context.draw_text_fitted_cow(
                tally,
                &diagnostics,
                font,
                chrome.dim_ink,
                HorizontalAlignment::Right,
            );
            context.draw_text_fitted_cow(
                strip,
                &right_label,
                font,
                chrome.dim_ink,
                HorizontalAlignment::Right,
            );
            // The left label ends where the tally begins, so the three items cannot overlap.
            Rect::new(
                status_band.x,
                status_band.y,
                (tally_x - STATUS_ITEM_GAP - status_band.x).max(0) as u32,
                status_band.height,
            )
        };
        context.draw_text_fitted_cow(
            caret_rect,
            &left_label,
            font,
            chrome.dim_ink,
            HorizontalAlignment::Left,
        );
    }
}
