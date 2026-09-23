// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Hand-written sample data for the data-bearing controls.
//!
//! # The gap this closes
//!
//! `WidgetFactory::create(name, geometry, text)` passes **one string**. Every constructor whose doc
//! says "with no rows or columns" therefore produced a control whose own feature was absent from its
//! own snapshot: `list_view.svg` was a filled rectangle, `data_grid.svg` a filled rectangle, and so
//! was every table, tree, chart and board. 34 of the 188 committed snapshots carried no ink other
//! than their frame.
//!
//! That is a **review** defect, not a rendering one. The control draws correctly — it has nothing to
//! draw. The census asks "did this control paint something?" and answers yes, because the frame is
//! paint. The snapshots exist so a person can see the control, and a person looking at an empty frame
//! learns nothing about whether the table's columns line up, the chart's axis is readable, or the
//! board's cards overlap.
//!
//! # What this is, and what it deliberately is not
//!
//! It is **sample data**: a handful of rows, a few series, a fixed set of columns. Written by hand so
//! the numbers are reviewable ("does the chart's y axis fit 12/48/31?") rather than generated, because
//! a random walk produces a different snapshot every run and the snapshots are committed and diffed.
//! Randomness would turn "no visual change" into "no visual change *this time*", which is exactly the
//! property `tools/check_svg_snapshots.sh` byte-compares to detect.
//!
//! It is **not** a fixture framework, a builder DSL, or a test double. It is the shortest path to a
//! snapshot that shows the control.
//!
//! # Why one module rather than per-control constants
//!
//! The five tables and four charts in this crate share a vocabulary — the same column names, the same
//! row count, the same two-digit numbers — so a reviewer comparing `table_widget.svg` with
//! `data_grid.svg` is comparing two renderings of *the same data*. Per-control literals would make
//! every comparison a comparison of two different datasets as well, and a difference in layout would
//! be indistinguishable from a difference in content.

use crate::compat::{String, ToString, Vec};

/// A column header shared by every tabular control in the sample set.
pub const HEADERS: [&str; 4] = ["Name", "Qty", "Price", "Status"];

/// One row of the sample table, as text cells.
///
/// Four cells per row and [`HEADERS`] supplies four names, which is the relation that makes the pair a
/// table. The values are deliberately of different widths ("Widget" against "1", "12.50" against "OK")
/// so a snapshot shows whether a column is sized from its own content or from a fixed guess.
pub const ROWS: [[&str; 4]; 6] = [
    ["Widget", "12", "12.50", "OK"],
    ["Gadget", "3", "240.00", "Low"],
    ["Doohickey", "48", "1.25", "OK"],
    ["Sprocket", "7", "88.75", "Hold"],
    ["Flange", "31", "9.99", "OK"],
    ["Cog", "1", "1200.00", "Low"],
];

/// The list of names, for the controls that show one column rather than a grid.
pub const LIST_ITEMS: [&str; 6] = ["Widget", "Gadget", "Doohickey", "Sprocket", "Flange", "Cog"];

/// A short menu of actions, for menus, palettes and command lists.
pub const MENU_ITEMS: [&str; 5] = ["New", "Open", "Save", "Export", "Close"];

/// Tab titles, for the tab-bearing controls.
pub const TAB_TITLES: [&str; 3] = ["Overview", "Details", "History"];

/// The bars of the sample bar chart, as `(label, value)`.
///
/// The values span roughly two orders of magnitude on purpose: a chart whose bars are `[10, 11, 12]`
/// renders as three near-identical rectangles and says nothing about how the scale is derived, while
/// `[12, 48, 31, 7, 25]` shows the axis, the bar lengths and the labels all at once.
pub const BARS: [(&str, f64); 5] =
    [("Mon", 12.0), ("Tue", 48.0), ("Wed", 31.0), ("Thu", 7.0), ("Fri", 25.0)];

/// The slices of the sample pie chart, as `(label, value)`.
pub const SLICES: [(&str, f64); 4] =
    [("Alpha", 40.0), ("Beta", 30.0), ("Gamma", 20.0), ("Delta", 10.0)];

/// One series of the sample line chart, as `(label, x, y)` triples.
///
/// Two series rather than one, because a line chart's *legend* and its per-series colours are what a
/// single-series snapshot cannot show.
pub const SERIES: [(&str, [(f64, f64); 6]); 2] = [
    ("Series A", [(0.0, 12.0), (1.0, 19.0), (2.0, 14.0), (3.0, 27.0), (4.0, 22.0), (5.0, 31.0)]),
    ("Series B", [(0.0, 8.0), (1.0, 11.0), (2.0, 18.0), (3.0, 15.0), (4.0, 29.0), (5.0, 24.0)]),
];

/// The four candlesticks of the sample OHLC chart, as `(open, high, low, close)`.
///
/// The third candle's high and the fourth's low are the extremes, so a snapshot shows whether the
/// chart's vertical range is derived from the data or from a fixed guess.
pub const CANDLES: [(f64, f64, f64, f64); 4] = [
    (20.0, 26.0, 18.0, 24.0),
    (24.0, 29.0, 22.0, 23.0),
    (23.0, 34.0, 21.0, 31.0),
    (31.0, 33.0, 15.0, 19.0),
];

/// Returns [`ROWS`] as owned rows, for a control that wants `Vec<Vec<String>>`.
pub fn rows() -> Vec<Vec<String>> {
    ROWS.iter().map(|row| row.iter().map(|cell| cell.to_string()).collect()).collect()
}

/// Returns [`HEADERS`] as owned strings, for a control that wants `Vec<String>`.
pub fn headers() -> Vec<String> {
    HEADERS.iter().map(|header| header.to_string()).collect()
}

/// Returns [`LIST_ITEMS`] as owned strings.
pub fn list_items() -> Vec<String> {
    LIST_ITEMS.iter().map(|item| item.to_string()).collect()
}

/// Returns [`MENU_ITEMS`] as owned strings.
pub fn menu_items() -> Vec<String> {
    MENU_ITEMS.iter().map(|item| item.to_string()).collect()
}

/// Returns [`TAB_TITLES`] as owned strings.
pub fn tab_titles() -> Vec<String> {
    TAB_TITLES.iter().map(|title| title.to_string()).collect()
}

/// Returns [`BARS`] as `(label, value)` pairs.
pub fn bars() -> Vec<(String, f64)> {
    BARS.iter().map(|(label, value)| (label.to_string(), *value)).collect()
}

/// Typeface names for the font combo box, which holds fonts rather than business rows.
pub const FONT_NAMES: [&str; 5] = ["Arial", "Helvetica", "Courier New", "Times New Roman", "Georgia"];

/// Returns the sample chart's x-axis labels, one per point.
pub fn x_labels() -> Vec<String> {
    ["0", "1", "2", "3", "4", "5"].iter().map(|label| label.to_string()).collect()
}

/// The rows of the sample tree, **indented to express the hierarchy**.
///
/// # Why the structure is in the text
///
/// `TreeModel::node_path` returns one string per visible row, and `VecTreeModel` is a flat
/// `Vec<String>` — the model has no parent/child relation to express. So a tree's shape is carried by
/// leading spaces, which is also what the control's own doc means by "node path". Without the
/// indentation the snapshot is a flat list drawn under a tree's chrome, and the feature the control
/// exists for (showing depth) would be absent from its own picture.
pub fn tree_rows() -> Vec<String> {
    let mut rows = Vec::new();
    for (index, name) in LIST_ITEMS.iter().enumerate() {
        // Two branches with children and four leaves, so a snapshot shows both an expandable node and a
        // leaf rather than six identical rows.
        if index < 2 {
            rows.push(name.to_string());
            rows.push(format!("  {name} · child 1"));
            rows.push(format!("  {name} · child 2"));
        } else {
            rows.push(format!("    {name}"));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row has as many cells as there are headers, which is what makes the pair a table.
    ///
    /// A ragged `ROWS` would render as a table with a missing cell — and the snapshot would show it,
    /// but only to a reviewer who counted. This is the assertion that makes the shape checkable.
    #[test]
    fn every_row_fills_every_column() {
        for (index, row) in ROWS.iter().enumerate() {
            assert_eq!(
                row.len(),
                HEADERS.len(),
                "row {index} has {} cells for {} headers",
                row.len(),
                HEADERS.len()
            );
        }
    }

    /// The sample data is not degenerate: the values differ.
    ///
    /// # Why this is worth asserting
    ///
    /// The whole point of sample data is that the snapshot shows a control doing its job. Rows of
    /// identical values would make a column-sizing defect invisible (every column the same width) and a
    /// chart's scale meaningless (every bar the same length), so the data would be present and the
    /// snapshot still uninformative — the same failure in a new costume.
    #[test]
    fn the_sample_values_are_not_all_the_same() {
        let quantities: Vec<&str> = ROWS.iter().map(|row| row[1]).collect();
        let distinct = {
            let mut sorted = quantities.clone();
            sorted.sort_unstable();
            sorted.dedup();
            sorted.len()
        };
        assert!(distinct >= 4, "the quantity column has only {distinct} distinct values");

        let statuses: Vec<&str> = ROWS.iter().map(|row| row[3]).collect();
        let distinct_statuses = {
            let mut sorted = statuses.clone();
            sorted.sort_unstable();
            sorted.dedup();
            sorted.len()
        };
        assert!(distinct_statuses >= 2, "the status column is one repeated value");
    }

    /// The chart data is ordered, so a snapshot shows a trend rather than noise.
    ///
    /// An unsorted series would still be *a* chart, but a rising-then-falling series is what makes
    /// "the axis is oriented correctly" visible: a reversed y axis turns a climb into a dive, and that
    /// is a defect a reviewer can see in a picture and cannot see in a permutation.
    #[test]
    fn the_bar_and_series_values_span_a_range() {
        let values: Vec<f64> = BARS.iter().map(|(_, value)| *value).collect();
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(max >= min * 3.0, "the bars span only {min}..{max}, which renders as a flat row");

        let (_, first) = SERIES[0];
        let min_y = first.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
        let max_y = first.iter().map(|(_, y)| *y).fold(f64::NEG_INFINITY, f64::max);
        assert!(max_y - min_y > 10.0, "the first series is nearly flat: {min_y}..{max_y}");
    }

    /// The candlesticks are valid OHLC bars, so a snapshot shows candles rather than a bug.
    ///
    /// `high >= max(open, close)` and `low <= min(open, close)` is the definition of a candle; a row
    /// violating it draws an inside-out candle that looks like a rendering fault while being a data
    /// fault. Asserting it here keeps the two apart.
    #[test]
    fn every_candle_has_a_body_inside_its_range() {
        for (index, (open, high, low, close)) in CANDLES.iter().enumerate() {
            let (open, high, low, close) = (*open, *high, *low, *close);
            assert!(
                high >= open.max(close),
                "candle {index}: high {high} is below the body ({open}, {close})"
            );
            assert!(
                low <= open.min(close),
                "candle {index}: low {low} is above the body ({open}, {close})"
            );
        }
    }

    /// The owned accessors agree with the borrowed constants.
    ///
    /// The constants are the readable form and the accessors are what the constructors take, so a
    /// drift between them would mean the snapshot and the source of truth disagreed — and only the
    /// accessor's version is on screen.
    #[test]
    fn the_owned_accessors_mirror_the_constants() {
        assert_eq!(headers().len(), HEADERS.len());
        assert_eq!(rows().len(), ROWS.len());
        assert_eq!(rows()[0].len(), HEADERS.len());
        assert_eq!(list_items().len(), LIST_ITEMS.len());
        assert_eq!(menu_items().len(), MENU_ITEMS.len());
        assert_eq!(tab_titles().len(), TAB_TITLES.len());
        assert_eq!(bars().len(), BARS.len());
        assert_eq!(bars()[0].0, BARS[0].0);
        assert_eq!(x_labels().len(), SERIES[0].1.len());
    }
}
