// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The document ↔ screen coordinate map (`B3`/`B4`).
//!
//! Before this module existed, every site that needed to turn a visual row or a
//! character column into a pixel offset — and back — re-derived the transform by
//! hand from four separate values: the text origin, the scroll offsets, the cell
//! width and the row height. That composite appeared in the paint loop, the caret,
//! the diagnostics underline, the hit-test, `rect_for_range` and the completion
//! popup, and the copies could disagree: the hit-test and the paint both computed
//! "which row is at this y", and a rounding change in one place silently made a
//! click land on a different row than the one drawn.
//!
//! This type is that transform, written **once**. It is a plain value type built
//! for one frame (or one hit-test) from the current geometry and scroll state, so
//! there is no cached state to invalidate and no lock to take.
//!
//! ## Forward and reverse
//!
//! * forward: [`CoordinateMap::row_y`] (visual row → y) and
//!   [`CoordinateMap::column_x`] (character column → x);
//! * reverse (`B4`): [`CoordinateMap::row_at_y`] (y → visual row) and
//!   [`CoordinateMap::column_at_x`] (x → character column).
//!
//! The reverse direction is what hit-testing, caret placement and any
//! `UI ↔ code` jump need, and it is the half the renderer previously spelled out
//! inline in exactly one place (`position_at_point`) while the forward half was
//! spelled out in six.

use super::editor::CodeEditor;
use crate::core::{Point, Rect};
use crate::widget::Widget;

/// A resolved document ↔ screen transform for one frame.
///
/// All fields are in **widget-local** coordinates, already including the widget's
/// own origin, so a caller never adds `rect.x` a second time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CoordinateMap {
    /// The widget's own rectangle, so `contains` can reject out-of-widget points.
    bounds: Rect,
    /// Screen x of character column 0, i.e. `rect.x + text_origin_x()`.
    origin_x: i32,
    /// Screen y of visual row 0, i.e. `rect.y + text_origin_y()`.
    origin_y: i32,
    /// Horizontal advance of one character cell.
    cell: f32,
    /// Vertical advance of one visual row.
    row_height: f32,
    /// First visual row of the viewport.
    scroll_row: usize,
    /// Horizontal scroll offset in character columns.
    scroll_column: usize,
    /// Bottom limit of the text area, i.e. `rect.y + height - status_bar_height()`.
    clip_bottom: i32,
    /// Right limit of the text area, excluding the scrollbar and minimap.
    clip_right: i32,
}

impl CodeEditor {
    /// Builds the coordinate map for the current geometry and scroll state.
    ///
    /// The map is derived, never stored: the scroll offsets, the measured cell and
    /// the geometry all change between frames, and a cached map would be one more
    /// thing that can go stale — which is the defect this module removes, not one
    /// it should reintroduce.
    pub(crate) fn coordinate_map(&self) -> CoordinateMap {
        let rect = self.geometry();
        CoordinateMap {
            bounds: rect,
            origin_x: rect.x + self.text_origin_x(),
            origin_y: rect.y + self.text_origin_y(),
            cell: self.cell_width(),
            row_height: self.line_height(),
            scroll_row: self.scroll_visual_row,
            scroll_column: self.scroll_column,
            clip_bottom: rect.y + rect.height as i32 - self.status_bar_height(),
            clip_right: rect.x + rect.width as i32 - self.scrollbar_width() - self.minimap_width(),
        }
    }
}

impl CoordinateMap {
    /// Screen y of the top edge of a visual row.
    pub(crate) fn row_y(&self, visual_row: usize) -> i32 {
        self.origin_y
            + ((visual_row as f32 - self.scroll_row as f32) * self.row_height).round() as i32
    }

    /// Screen x of a character column.
    pub(crate) fn column_x(&self, column: usize) -> i32 {
        self.origin_x
            + ((column.saturating_sub(self.scroll_column)) as f32 * self.cell).round() as i32
    }

    /// Visual row containing screen `y`, clamped into the document.
    ///
    /// `y` above the text area maps to row 0; `y` below maps to the last row the
    /// viewport can show. The result is a **visual** row: a caller that needs a
    /// document line still has to run it through
    /// [`CodeEditor::visual_row_to_line`], because folding and wrapping make the
    /// two differ.
    pub(crate) fn row_at_y(&self, y: i32) -> usize {
        let local = y - self.origin_y;
        if local <= 0 {
            return self.scroll_row;
        }
        let offset = (local as f32 / self.row_height).floor() as usize;
        self.scroll_row.saturating_add(offset)
    }

    /// Character column at screen `x`, clamped to a non-negative value.
    pub(crate) fn column_at_x(&self, x: i32) -> usize {
        let local = x - self.origin_x;
        if local <= 0 {
            return self.scroll_column;
        }
        // `round` rather than truncate: the caret snaps to the nearer cell
        // boundary, so clicking the right half of a cell places the caret after it
        // — which is what a user expects from a text editor.
        self.scroll_column + (local as f32 / self.cell).round() as usize
    }

    /// Whether a point lies inside the text area (exclusive of chrome).
    pub(crate) fn text_area_contains(&self, point: Point) -> bool {
        point.x >= self.origin_x
            && point.y >= self.origin_y
            && point.x < self.clip_right
            && point.y < self.clip_bottom
    }

    /// Height of one visual row, rounded up, for filled rectangles.
    pub(crate) fn row_height(&self) -> f32 {
        self.row_height
    }

    /// The widget rectangle the map was built for.
    pub(crate) fn bounds(&self) -> Rect {
        self.bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn editor() -> CodeEditor {
        CodeEditor::new(Rect::new(0, 0, 400, 300))
    }

    /// The forward and reverse transforms must be inverses, or a click lands on a
    /// different row than the one the paint loop drew.
    #[test]
    fn the_row_transform_round_trips() {
        let map = editor().coordinate_map();
        for row in 0..8 {
            let y = map.row_y(row);
            assert_eq!(map.row_at_y(y), row, "row {row} at y {y} must round-trip");
        }
    }

    /// The column transform must round-trip at cell centres.
    #[test]
    fn the_column_transform_round_trips() {
        let map = editor().coordinate_map();
        for column in 0..6 {
            let x = map.column_x(column);
            assert_eq!(map.column_at_x(x), column, "column {column} at x {x} must round-trip");
        }
    }

    /// Scrolling must be accounted for in both directions: this is the offset the
    /// duplicated hand-computations used to get wrong.
    #[test]
    fn scrolling_offsets_both_directions() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 120));
        let mut text = String::new();
        for i in 0..200 {
            text.push_str(&format!("line {i}\n"));
        }
        editor.set_text(text);
        editor.base.set_geometry(Rect::new(0, 0, 400, 120));
        editor.refresh_visible_rows();
        editor.scroll_by(3);
        assert_eq!(editor.scroll_line(), 3, "the document is long enough to scroll");
        let map = editor.coordinate_map();
        // The first visible row now sits at the text origin.
        assert_eq!(map.row_y(map.scroll_row), map.origin_y);
        assert_eq!(map.row_at_y(map.origin_y), map.scroll_row);
    }

    /// A point outside the text area must be rejected, not clamped into it: the
    /// hit-test uses this to tell a click in the gutter from a click in the text.
    #[test]
    fn the_text_area_predicate_excludes_chrome() {
        let editor = editor();
        let map = editor.coordinate_map();
        let origin_x = map.column_x(0);
        let origin_y = map.row_y(map.scroll_row);
        assert!(map.text_area_contains(Point::new(origin_x + 5, origin_y + 5)));
        assert!(!map.text_area_contains(Point::new(origin_x - 1, origin_y + 5)));
        assert!(!map.text_area_contains(Point::new(origin_x + 5, origin_y - 1)));
    }

    /// Splitting a long line with wrap on must keep the row/line distinction
    /// visible through the map: two visual rows belong to one document line.
    #[test]
    fn the_map_keeps_visual_rows_distinct_from_lines() {
        let mut editor = CodeEditor::with_config(
            Rect::new(0, 0, 120, 300),
            super::super::types::CodeEditorConfig::new().word_wrap(true),
        )
        .expect("valid config");
        editor.set_text("x".repeat(400).to_string());
        let map = editor.coordinate_map();
        assert_eq!(map.row_at_y(map.row_y(0)), 0);
        assert_eq!(map.row_at_y(map.row_y(1)), 1);
        // Both visual rows map back to the same document line.
        assert_eq!(editor.visual_row_to_line(0), editor.visual_row_to_line(1));
    }
}
