// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Diagnostic underlines and messages.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

impl CodeEditor {
    /// Paints the diagnostic underlines and, on the caret's line only, the message.
    ///
    /// # Why the message moved
    ///
    /// The label used to be drawn at `row_y + row_height`, i.e. **on the next
    /// line's text** — the demo screenshot showed `warning: deep recursion` sitting
    /// on top of `let limit = 10;`. Messages now go at the end of the offending
    /// line, on that line's own baseline, and only when they fit before the
    /// scrollbar; otherwise the underline alone states the fact and the message is
    /// not crammed in. Restricting the message to the caret's line keeps a file
    /// with a thousand warnings from painting a thousand overlapping labels.
    pub(super) fn draw_diagnostics(
        &self,
        context: &mut RenderContext,
        visible: &[usize],
        chrome: &EditorChrome,
    ) {
        let inline = self.inline_diagnostics();
        if inline.is_empty() {
            return;
        }
        let row_height = self.line_height();
        let rect = self.geometry();
        let left = rect.x + self.text_origin_x();
        let right = rect.x + rect.width as i32 - self.scrollbar_width() - self.minimap_width();
        let map = self.coordinate_map();
        let cell = self.cell_width();
        let active_line = self.cursor.head.line;
        let font = self.chrome_font();
        let label_height = context.measure_text("M", font).height as i32;
        let wrapped = self.config.word_wrap;
        let columns = self.text_columns().max(1);
        for (_, marker) in inline {
            let line = marker.line;
            if !visible.contains(&line) {
                continue;
            }
            let text = self.line_text(line).unwrap_or_default();
            let grid = LineGrid::new(&text, self.config.tab_width);
            // The marker may sit on a wrapped continuation row. The underline belongs
            // on the row that actually shows those characters, and its x is measured
            // from that row's first column — the same rule the caret and the text use,
            // or the underline is drawn a segment away from the code it names.
            let (segment, segment_start) =
                if wrapped { grid.segment_of_char(marker.start_column(), columns) } else { (0, 0) };
            let row = self.line_to_visual_row(line) + segment;
            if row < self.scroll_visual_row {
                continue;
            }
            let (segment_end, indent) = if wrapped {
                (
                    grid.segment_range(segment, columns).1,
                    if segment > 0 {
                        (grid.leading_indent_columns() as f32 * cell)
                            .min(columns.saturating_sub(4) as f32 * cell)
                    } else {
                        0.0
                    },
                )
            } else {
                (grid.char_len(), 0.0)
            };
            let from_index =
                grid.column_of_char(marker.start_column().max(segment_start).min(segment_end));
            let to_index =
                grid.column_of_char(marker.finish_column().min(segment_end)).max(from_index + 1);
            let first_column = grid.column_of_char(segment_start);
            // Measured where available, so the underline sits under the glyphs it
            // names rather than under the grid cells they would have occupied. Wrapped
            // rows are not measured per character, so they use the row's grid columns.
            let x_of = |index: usize| -> f32 {
                if wrapped {
                    indent + grid.column_of_char(index).saturating_sub(first_column) as f32 * cell
                } else {
                    self.cached_x(line, index)
                        .unwrap_or_else(|| grid.column_of_char(index) as f32 * cell)
                }
            };
            let y = map.row_y(row);
            let x = self.screen_x(left, x_of(from_index));
            let width = (x_of(to_index) - x_of(from_index)).round().max(1.0) as u32;
            // A 2 px rule along the row's bottom edge: a squiggle would need a
            // per-character wave that costs more than it tells the reader.
            let underline_y = y + row_height.round() as i32 - 2;
            context.draw_line(
                Point::new(x, underline_y),
                Point::new(x + width as i32, underline_y),
                marker.severity.color(),
            );
            if line != active_line {
                continue;
            }
            let label = format!("{} {}", marker.severity.badge(), marker.message);
            let label_width = context.measure_text(&label, font).width as i32;
            // After the underlined text, not after the whole line: on a long or wrapped
            // line the end of the line is off screen, so the message would never be
            // shown; anchored to the marker it is always beside what it describes.
            let label_x = self.screen_x(left, x_of(to_index)) + 12;
            if label_x + label_width <= right {
                context.draw_text(
                    Point::new(label_x, y + (row_height as i32 - label_height) / 2),
                    &label,
                    font,
                    marker.severity.color().legible_on(chrome.surface, 4.5),
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}
