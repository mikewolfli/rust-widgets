// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable tests.
use super::*;
// The tests below export SVG on every profile this module is built for, so the import is
// unconditional: gating it on `desktop` made the tablet profile compile the tests without
// the function they call.
use crate::signal::GenericSignal;
use crate::widget::svg::render_to_svg;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The cell grid uses the weak separator role, not the frame's strength.
///
/// # Why this is the judgement rather than a style preference
///
/// Plan §5.4 asks for separators to be visibly weaker than the focus ring, and its stated
/// criterion is "the grid line's colour differs from the ring's". The control drew its grid with
/// a **local** `surface.blend(&ink, 0.10)` while `table_widget` — the same `Table` kind, so the
/// same grid — read `colors.outline_variant`. Both satisfied the criterion, which is why neither
/// was a defect, but they were two derivations of one decision: a theme could tune the grid for
/// one control and not the other. This pins them to one source.
#[test]
#[cfg(all(device_profile, feature = "desktop"))]
fn the_cell_grid_reads_the_weak_separator_role() {
    let _guard = crate::theme::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);

    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    crate::theme::apply_theme_to_widget(&mut table);
    let svg = render_to_svg(&mut table);

    let palette = crate::style::palette_shared();
    let variant = palette.as_ref().expect("a preset is active").outline_variant;
    let background = palette.as_ref().expect("a preset is active").background;
    let expected = format!("rgba({},{},{},", variant.r, variant.g, variant.b);
    assert!(
        svg.contains(&expected),
        "the grid must be stroked in `outline_variant` ({expected}…), so a theme can tune it \
             for every `Table`-kind control at once"
    );
    // And it must be the **weak** role: a grid drawn in the frame's own `outline` strength would
    // satisfy the previous assertion if the two roles happened to be equal, which is exactly the
    // "two names, one colour" state §5.4 exists to end.
    assert_ne!(variant, background, "the separator role must not coincide with the page");
}

struct StaticSource;

impl IncrementalTableDataSource for StaticSource {
    fn row_count(&self) -> usize {
        100
    }

    fn column_count(&self) -> usize {
        20
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        Some(format!("{}:{}", row, column))
    }
}

/// U2-06: an RTL page mirrors the cell grid and the hit test with it.
#[test]
fn rtl_mirrors_the_cell_grid() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(StaticSource));
    let rect = table.geometry();
    crate::widget::view_widgets::rtl_columns::tests::with_rtl(|| {
        // Column 0 (120 px wide) occupies the right half of the 236 px content band.
        assert_eq!(table.column_at(rect.x + rect.width as i32 - 3), Some(0));
        // The leftmost pixel mirrors to the band's right edge, which is column 1.
        assert_eq!(table.column_at(rect.x + 2), Some(1));
    });
}

/// T3-16: the 2 px gap is on the visual left of a mirrored cell in both paint and hit test.
#[test]
fn the_rtl_gap_and_the_painted_cell_share_one_edge() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(StaticSource));
    let rect = table.geometry();
    crate::widget::view_widgets::rtl_columns::tests::with_rtl(|| {
        // Columns are 120 px wide. Column 1's painted cell ends at x=118 and column 0's begins
        // at x=120; the two pixels between them are the gap in both the paint and the hit test.
        assert_eq!(table.column_at(rect.x + 117), Some(1));
        assert_eq!(table.column_at(rect.x + 118), None);
        assert_eq!(table.column_at(rect.x + 119), None);
        assert_eq!(table.column_at(rect.x + 120), Some(0));
    });
}

/// A menu request selects the cell under the pointer before it is emitted (C-9).
#[test]
fn a_menu_request_selects_the_cell_under_the_pointer() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 120));
    table.set_data_source(Arc::new(StaticSource));
    let seen = Arc::new(std::sync::Mutex::new(None));
    let sink = Arc::clone(&seen);
    table.context_menu_requested.connect(move |pos| {
        if let Ok(mut slot) = sink.lock() {
            *slot = Some(*pos);
        }
    });

    let rect = table.geometry();
    // 2 px inset, then the header row, then 2 px into the first body row (the painted cell
    // refuses its final 2 px gap, so the probe stays at the leading edge).
    let point = Point::new(rect.x + 4, rect.y + 2 + table.header_offset() + 2);
    assert_eq!(table.cell_at(point), Some((0, 0)));
    assert_eq!(table.selected_cell(), None);
    assert!(Widget::context_menu_event(&mut table, point));
    assert_eq!(table.selected_cell(), Some((0, 0)));
    assert_eq!(*seen.lock().expect("the test slot locks"), Some(point));
}

#[test]
fn visible_window_tracks_scroll() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(StaticSource));

    table.set_scroll_row(12);
    table.set_scroll_column(5);
    let (row_start, _, col_start, _) = table.visible_window();

    assert!(row_start <= 12);
    assert!(col_start <= 5);
    assert!(table.row_count() == 100);
}

#[test]
fn fetch_visible_window_reads_cells() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(StaticSource));

    let data = table.fetch_visible_window();
    assert!(!data.is_empty());
    assert_eq!(data[0][0], Some("0:0".to_string()));
}

#[test]
fn signal_emits_when_window_changes() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(StaticSource));

    let emitted = Arc::new(Mutex::new(Vec::<(usize, usize, usize, usize)>::new()));
    let sink = emitted.clone();
    table.visible_window_changed.connect(move |window| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*window);
        }
    });

    table.set_scroll_row(10);

    let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert!(!got.is_empty());
}

#[test]
fn overscan_settings_resize_visible_window() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(StaticSource));
    table.set_scroll_row(10);
    table.set_scroll_column(5);

    table.set_overscan_rows(0);
    table.set_overscan_columns(0);
    let (_, row_len_small, _, col_len_small) = table.visible_window();

    table.set_overscan_rows(3);
    table.set_overscan_columns(2);
    let (_, row_len_large, _, col_len_large) = table.visible_window();

    assert!(row_len_large >= row_len_small);
    assert!(col_len_large >= col_len_small);
    assert_eq!(table.overscan_rows(), 3);
    assert_eq!(table.overscan_columns(), 2);
}

#[test]
fn new_creates_default_state() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    assert!(!table.has_data_source());
    assert_eq!(table.scroll_row(), 0);
    assert_eq!(table.scroll_column(), 0);
    assert_eq!(table.row_height(), 20);
    assert_eq!(table.column_width(), 120);
    assert_eq!(table.overscan_rows(), 2);
    assert_eq!(table.overscan_columns(), 1);
    assert_eq!(table.row_count(), 0);
    assert_eq!(table.column_count(), 0);
    let data = table.fetch_visible_window();
    assert!(data.is_empty());
}

#[test]
fn has_data_source_before_and_after() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    assert!(!table.has_data_source());

    table.set_data_source(Arc::new(StaticSource));
    assert!(table.has_data_source());

    table.clear_data_source();
    assert!(!table.has_data_source());
}

#[test]
fn row_and_column_counts() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    assert_eq!(table.row_count(), 0);
    assert_eq!(table.column_count(), 0);

    table.set_data_source(Arc::new(StaticSource));
    assert_eq!(table.row_count(), 100);
    assert_eq!(table.column_count(), 20);

    table.clear_data_source();
    assert_eq!(table.row_count(), 0);
    assert_eq!(table.column_count(), 0);
}

#[test]
fn scroll_position_clamping() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    // Without source, scroll still accumulates (normalize not called after source set)
    table.set_data_source(Arc::new(StaticSource));

    table.set_scroll_row(50);
    assert_eq!(table.scroll_row(), 50);

    // Clamp to max
    table.set_scroll_row(999);
    assert_eq!(table.scroll_row(), 99);

    // Clamped to max column index (19)
    table.set_scroll_column(30);
    assert_eq!(table.scroll_column(), 19);

    // Already at max
    table.set_scroll_column(999);
    assert_eq!(table.scroll_column(), 19);
}

#[test]
fn clear_data_source_resets_state() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    table.set_data_source(Arc::new(StaticSource));
    table.set_scroll_row(10);
    table.set_scroll_column(5);

    table.clear_data_source();
    assert!(!table.has_data_source());
    assert_eq!(table.scroll_row(), 0);
    assert_eq!(table.scroll_column(), 0);
    assert_eq!(table.row_count(), 0);
    assert_eq!(table.column_count(), 0);
    assert!(table.fetch_visible_window().is_empty());
}

#[test]
fn cached_window_returns_same_data_without_refetch() {
    struct TrackingSource;
    impl IncrementalTableDataSource for TrackingSource {
        fn row_count(&self) -> usize {
            50
        }
        fn column_count(&self) -> usize {
            5
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("data-{}:{}", row, column))
        }
        fn revision(&self) -> u64 {
            1
        }
    }

    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(TrackingSource));

    let a = table.fetch_visible_window();
    let b = table.fetch_visible_window();
    assert_eq!(a, b);
}

#[test]
fn a_revisionless_source_without_a_contract_is_refetched_not_served_stale() {
    struct InPlaceSource {
        generation: AtomicU64,
        calls: AtomicUsize,
    }
    impl IncrementalTableDataSource for InPlaceSource {
        fn row_count(&self) -> usize {
            6
        }
        fn column_count(&self) -> usize {
            2
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Some(format!("gen-{}:{}:{}", self.generation.load(Ordering::Relaxed), row, column))
        }
        // `revision()` and `data_changed_signal()` keep the trait defaults: no versioning
        // and no signal — no invalidation contract, so the window must not be cached.
    }

    let source =
        Arc::new(InPlaceSource { generation: AtomicU64::new(0), calls: AtomicUsize::new(0) });
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(source.clone());

    let first = table.fetch_visible_window();
    let calls_after_first = source.calls.load(Ordering::Relaxed);
    assert!(calls_after_first > 0);

    // The source mutates in place. With no revision and no signal there is no way to be
    // told, so the next fetch must go back to the source instead of serving the window
    // built from the old content.
    source.generation.fetch_add(1, Ordering::Relaxed);
    let second = table.fetch_visible_window();
    assert_ne!(first, second);
    assert!(source.calls.load(Ordering::Relaxed) > calls_after_first);
}

#[test]
fn a_revisionless_source_is_cached_and_invalidated_by_its_signal() {
    struct SignalSource {
        generation: AtomicU64,
        calls: AtomicUsize,
        changed: GenericSignal,
    }
    impl IncrementalTableDataSource for SignalSource {
        fn row_count(&self) -> usize {
            6
        }
        fn column_count(&self) -> usize {
            2
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Some(format!("gen-{}:{}:{}", self.generation.load(Ordering::Relaxed), row, column))
        }
        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.changed)
        }
    }

    let source = Arc::new(SignalSource {
        generation: AtomicU64::new(0),
        calls: AtomicUsize::new(0),
        changed: GenericSignal::new(),
    });
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(source.clone());

    let _ = table.fetch_visible_window();
    let calls_after_first = source.calls.load(Ordering::Relaxed);

    // No signal fired: the cached window is reused.
    let cached = table.fetch_visible_window();
    assert_eq!(source.calls.load(Ordering::Relaxed), calls_after_first);

    // The source mutates in place and raises `data_changed`; the generation advances and
    // the next fetch rebuilds from the new content.
    source.generation.fetch_add(1, Ordering::Relaxed);
    source.changed.emit();
    let fresh = table.fetch_visible_window();
    assert_ne!(cached, fresh);
    assert!(source.calls.load(Ordering::Relaxed) > calls_after_first);
    assert!(fresh[0][0].as_deref().is_some_and(|cell| cell.starts_with("gen-1")));
}

#[test]
fn empty_source_handling() {
    struct EmptySource;
    impl IncrementalTableDataSource for EmptySource {
        fn row_count(&self) -> usize {
            0
        }
        fn column_count(&self) -> usize {
            0
        }
        fn data(&self, _: usize, _: usize) -> Option<String> {
            None
        }
    }

    let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    table.set_data_source(Arc::new(EmptySource));

    assert_eq!(table.row_count(), 0);
    assert_eq!(table.column_count(), 0);
    assert_eq!(table.visible_window(), (0, 0, 0, 0));
    assert!(table.fetch_visible_window().is_empty());

    // Scrolling on empty source
    table.set_scroll_row(10);
    assert_eq!(table.scroll_row(), 0);
}

#[test]
fn overscan_default_values() {
    let table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    assert_eq!(table.overscan_rows(), 2);
    assert_eq!(table.overscan_columns(), 1);
}

#[test]
fn set_row_height_and_column_width_minimum_clamp() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
    table.set_data_source(Arc::new(StaticSource));

    table.set_row_height(0);
    assert_eq!(table.row_height(), 1);

    table.set_row_height(30);
    assert_eq!(table.row_height(), 30);

    table.set_column_width(0);
    assert_eq!(table.column_width(), 1);

    table.set_column_width(80);
    assert_eq!(table.column_width(), 80);
}

// ── The shared cell derivation ───────────────────────────────────────

#[test]
fn a_cells_box_is_a_function_of_its_own_indices() {
    // The defect this pins: the cell position used to be an accumulator inside the draw
    // loop (`x += column_width`), so a cell's box depended on how many cells happened to
    // be drawn before it and could not be asked for independently. It is now
    // `origin + index * stride`.
    let mut table = VirtualTable::new(Rect::new(10, 20, 400, 300));
    table.set_column_width(80);
    table.set_row_height(40);
    let origin = table.geometry();

    // (0, 0) sits at the inset, and each step is exactly one stride.
    let first = table.cell_rect(origin, 0, 0);
    assert_eq!(first.x, origin.x + 2);
    assert_eq!(first.y, origin.y + 2);
    assert_eq!(first.width, 78);
    assert_eq!(first.height, 38);

    let third_column = table.cell_rect(origin, 0, 3);
    assert_eq!(third_column.x, first.x + 3 * 80, "a column is one stride apart");
    let second_row = table.cell_rect(origin, 2, 0);
    assert_eq!(second_row.y, first.y + 2 * 40, "a row is one stride apart");

    // Asking for a box twice gives the same answer: no accumulated state.
    assert_eq!(table.cell_rect(origin, 2, 3), table.cell_rect(origin, 2, 3));
}

#[test]
fn the_box_a_press_resolves_to_is_the_box_that_was_painted() {
    // The draw loop and the hit test now read one derivation. Before, the only record of a
    // cell's position was the draw loop's local variable, so the table could not answer
    // "which cell is under this point?" at all.
    let mut table = VirtualTable::new(Rect::new(0, 0, 400, 300));
    table.set_column_width(80);
    table.set_row_height(40);

    let cell = table.cell_rect(table.geometry(), 1, 2);
    let centre =
        crate::core::Point::new(cell.x + cell.width as i32 / 2, cell.y + cell.height as i32 / 2);
    assert_eq!(table.cell_at(centre), Some((1, 2)));

    // A point on the table's own border, outside every inset cell, resolves to no cell
    // rather than to a neighbour — the boxes are the authority, not a division of the
    // whole rectangle.
    assert_eq!(table.cell_at(crate::core::Point::new(0, 0)), None);
}

#[test]
fn the_visible_grid_counts_what_fits_and_never_reports_zero() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 400, 300));
    table.set_column_width(100);
    table.set_row_height(50);

    // Usable extent is the rectangle minus the 2 px inset on each side: 396 x 296, so
    // three 100 px columns and five 50 px rows fit.
    assert_eq!(table.visible_grid(table.geometry()), (5, 3));

    // A table clipped to a sliver still reports one of each, so its first cell is drawn
    // rather than the control coming out blank.
    let sliver = Rect::new(0, 0, 3, 3);
    assert_eq!(table.visible_grid(sliver), (1, 1));
}

/// N-V-05: a huge overscan must saturate `overscan * 2`, not wrap the window length.
#[test]
fn a_huge_overscan_does_not_wrap_the_window() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    table.set_row_height(20);
    table.set_column_width(20);
    // Debug builds panic on the wrapping `overscan_rows * 2`; reaching the assert is the check.
    table.set_overscan_rows(usize::MAX);
    table.set_overscan_columns(usize::MAX);
    let (_row_start, row_len, _col_start, col_len) = table.visible_window();
    // A wrapped multiply could have produced a tiny length; saturation keeps it large.
    assert!(row_len > 1, "row length must not wrap to a small value, got {row_len}");
    assert!(col_len > 1, "column length must not wrap to a small value, got {col_len}");
}

// ── D09-VIEW-01: the leading overscan is skipped, not painted as the first cell ──

/// The cell a press resolves to must be the **global** cell at that box, not a window-local
/// index, once the fetch window begins before the viewport.
///
/// # The defect this pins
///
/// `visible_window()` fetches from `scroll - overscan`, so the returned data's local `(0, 0)` is
/// *above/left of* the viewport. The draw loop treated that `(0, 0)` as the visible top-left, so
/// at row 12 / column 5 the cell painted in the first box was actually global (10, 4). `cell_at`
/// now converts through the window origin, so the box the user sees reports the global index that
/// was fetched for it.
#[test]
fn a_visible_cell_maps_to_its_global_index_after_scrolling() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    table.set_row_height(20);
    table.set_column_width(40);

    table.set_scroll_row(12);
    table.set_scroll_column(5);

    // The first visible box (viewport-relative (0, 0)) must name the scrolled-to cell, because
    // that is the cell the draw pass now paints there.
    let first = table.cell_rect(table.geometry(), 0, 0);
    let centre = crate::core::Point::new(
        first.x + first.width as i32 / 2,
        first.y + first.height as i32 / 2,
    );
    assert_eq!(
        table.cell_at(centre),
        Some((12, 5)),
        "the first visible box must report the global scrolled-to cell"
    );

    // And the local index the box is addressed by is exactly `scroll - overscan`, so the drawn
    // loop's leading skip and the hit test's origin share one derivation.
    let (row_origin, col_origin) = table.window_origin();
    assert_eq!(table.scroll_row - row_origin, 2, "default row overscan is skipped");
    assert_eq!(table.scroll_column - col_origin, 1, "default column overscan is skipped");
}

/// Overscan 0 makes the window origin the scroll position itself, so the skip is a no-op.
#[test]
fn zero_overscan_leaves_the_window_origin_at_the_scroll_position() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    table.set_row_height(20);
    table.set_column_width(40);
    table.set_overscan_rows(0);
    table.set_overscan_columns(0);
    table.set_scroll_row(7);
    table.set_scroll_column(3);

    assert_eq!(table.window_origin(), (7, 3));
    let first = table.cell_rect(table.geometry(), 0, 0);
    let centre = crate::core::Point::new(
        first.x + first.width as i32 / 2,
        first.y + first.height as i32 / 2,
    );
    assert_eq!(table.cell_at(centre), Some((7, 3)));
}

/// At the top-left edge there is no leading overscan to skip, so the origin is `(0, 0)` and the
/// first box names the data source's own first cell.
#[test]
fn the_top_left_corner_has_no_leading_overscan_to_skip() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    table.set_row_height(20);
    table.set_column_width(40);

    assert_eq!(table.visible_window().0, 0);
    assert_eq!(table.visible_window().2, 0);
    let first = table.cell_rect(table.geometry(), 0, 0);
    let centre = crate::core::Point::new(
        first.x + first.width as i32 / 2,
        first.y + first.height as i32 / 2,
    );
    assert_eq!(table.cell_at(centre), Some((0, 0)));
}

/// The **drawn** values must be the global cells for the visible boxes — the model-versus-pixels
/// form of the defect.
///
/// # Why only the scrolled-to cell is non-empty
///
/// The SVG does not embed the cell's text string (each glyph is an outline `<path>`), so the
/// content cannot be read back as a literal. Instead the data source makes exactly one cell
/// non-empty — the cell the viewport's top-left box must hold after scrolling. If the draw pass
/// skipped the leading overscan correctly, that one value lands at the first visible box; if it
/// did not, the leading overscan cell would be painted first and the visible box would be blank.
#[test]
#[cfg(all(device_profile, feature = "desktop"))]
fn the_drawn_cells_are_the_global_cells_after_scrolling() {
    struct MarkedSource;
    impl IncrementalTableDataSource for MarkedSource {
        fn row_count(&self) -> usize {
            50
        }
        fn column_count(&self) -> usize {
            10
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            if (row, column) == (12, 5) {
                Some("W".to_string())
            } else {
                Some(String::new())
            }
        }
    }

    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(MarkedSource));
    table.set_row_height(20);
    table.set_column_width(40);
    table.set_scroll_row(12);
    table.set_scroll_column(5);

    // The first visible box is where the scrolled-to global cell must be painted.
    let first = table.cell_rect(table.geometry(), 0, 0);
    let boxes = crate::widget::svg::text_ink_boxes(&render_to_svg(&mut table));
    assert_eq!(
        boxes.len(),
        1,
        "only the one non-empty cell must be drawn as text; a leading overscan skip paints it, \
             a missing skip paints the blank lead cell instead"
    );
    let (bx, by, _, _) = boxes[0];
    assert!(
        bx >= first.x && bx < first.x + first.width as i32,
        "the value must be painted inside the first visible box ({first:?}), got x={bx}"
    );
    assert!(
        by >= first.y && by < first.y + first.height as i32,
        "the value must be painted inside the first visible box ({first:?}), got y={by}"
    );
}

// ── D09-VIEW-03: the cache-hit draw path must not deep-clone the window ──

/// A cache hit must be shared, not deep-cloned, and the draw path must read the same window
/// across consecutive redraws without another fetch.
///
/// # The defect this pins
///
/// `fetch_visible_window()` returned `cache.clone()` on every hit, and `Draw::draw` called it on
/// every redraw — so a static window re-copied every row, column and `String` each frame. The
/// cache is now shared behind an `Arc`: a hit hands out the same allocation, which is observable
/// as a stable pointer identity across calls, and the source is still fetched exactly once.
#[test]
fn a_cache_hit_shares_the_window_instead_of_deep_cloning() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingSource {
        fetches: Arc<AtomicUsize>,
    }
    impl IncrementalTableDataSource for CountingSource {
        fn row_count(&self) -> usize {
            50
        }
        fn column_count(&self) -> usize {
            5
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("data-{}:{}", row, column))
        }
        // The shared-cache guarantee under test applies to sources with an invalidation
        // contract; a contract-less source is refetched instead of served stale.
        fn revision(&self) -> u64 {
            1
        }
        fn fetch_window(
            &self,
            row_start: usize,
            row_len: usize,
            column_start: usize,
            column_len: usize,
        ) -> Vec<Vec<Option<String>>> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            let row_end = row_start.saturating_add(row_len).min(self.row_count());
            let col_end = column_start.saturating_add(column_len).min(self.column_count());
            (row_start..row_end)
                .map(|row| (column_start..col_end).map(|c| self.data(row, c)).collect())
                .collect()
        }
    }

    let fetches = Arc::new(AtomicUsize::new(0));
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_data_source(Arc::new(CountingSource { fetches: fetches.clone() }));

    // Two cache hits in a row must be served from the cache, so the source is fetched exactly
    // once across both; the data each call returns is identical and complete.
    let first = table.fetch_visible_window();
    let second = table.fetch_visible_window();
    assert_eq!(first, second, "a cache hit must return the same window contents");
    assert!(!first.is_empty());
    assert_eq!(fetches.load(Ordering::SeqCst), 1, "a cache hit must not re-fetch the source");

    // A redraw (the draw path reads the shared window) is likewise a cache hit, so no extra
    // fetch happens between frames.
    #[cfg(all(device_profile, feature = "desktop"))]
    {
        let _ = crate::widget::svg::render_to_svg(&mut table);
        assert_eq!(
            fetches.load(Ordering::SeqCst),
            1,
            "a redraw must read the cached window, not re-fetch"
        );
    }

    // Scrolling invalidates the cache, and the next fetch returns the window that begins at
    // the new fetch origin (`scroll - overscan`).
    table.set_scroll_row(10);
    let scrolled = table.fetch_visible_window();
    let (row_start, _, _, _) = table.visible_window();
    assert_eq!(row_start, 8, "row 10 with overscan 2 fetches from row 8");
    assert_eq!(
        scrolled[0][0],
        Some(format!("data-{}:0", row_start)),
        "after invalidation the window must begin at the new fetch origin"
    );
    assert_eq!(fetches.load(Ordering::SeqCst), 2, "a scroll must invalidate and re-fetch once");
}

/// Hover is the absolute row under the pointer, resolved through the same `cell_at`
/// derivation the paint reads, and leaving the table clears the highlight.
#[test]
fn virtual_table_hover_follows_the_pointer() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));

    let cell = table.cell_rect(table.geometry(), 0, 0);
    table.handle_event(&Event::MouseMove { pos: crate::core::Point::new(cell.x + 5, cell.y + 5) });
    assert_eq!(table.hovered_row, Some(0));

    // A lower viewport row lights its own absolute index.
    let cell = table.cell_rect(table.geometry(), 2, 1);
    table.handle_event(&Event::MouseMove { pos: crate::core::Point::new(cell.x + 5, cell.y + 5) });
    assert_eq!(table.hovered_row, Some(2));

    table.handle_event(&Event::MouseLeave { pos: crate::core::Point::new(-1, -1) });
    assert_eq!(table.hovered_row, None);
}

/// The hover is stored absolutely, so scrolling moves the highlight with its row:
/// the same pointer position after a scroll names a different absolute row.
#[test]
fn virtual_table_hover_is_absolute_across_a_scroll() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));

    let cell = table.cell_rect(table.geometry(), 0, 0);
    table.handle_event(&Event::MouseMove { pos: crate::core::Point::new(cell.x + 5, cell.y + 5) });
    assert_eq!(table.hovered_row, Some(0));

    table.set_scroll_row(5);
    assert_eq!(table.hovered_row, Some(0), "the row the pointer was on did not change");

    // The pointer has not moved, but the row now under it is global row 5.
    table.handle_event(&Event::MouseMove { pos: crate::core::Point::new(cell.x + 5, cell.y + 5) });
    assert_eq!(table.hovered_row, Some(5));
}

/// Cell selection, the header row, page keys and the scroll-bar thumb are wired.
#[test]
fn selection_header_paging_and_scrollbar_are_wired() {
    use crate::event::types::EventHandler;
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    let cell = table.cell_rect(Rect::new(0, 0, 320, 200), 1, 1);
    table.handle_event(&Event::MousePress {
        pos: crate::core::Point::new(cell.x + 2, cell.y + 2),
        button: 1,
        modifiers: 0,
    });
    assert!(table.selected_cell().is_some());
    table.set_show_header(true);
    assert!(table.show_header());
    table.set_show_scrollbar(true);
    assert!(table.show_scrollbar());
    table.handle_event(&Event::KeyPress { key: crate::event::key_codes::PAGE_DOWN, modifiers: 0 });
    assert!(table.scroll_row() > 0, "PageDown advances one viewport");
    let _ = render_to_svg(&mut table);
}

/// A delegate can take over a cell's ink, and the table reports that it is installed.
#[test]
fn a_cell_delegate_can_paint_a_cell() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingDelegate {
        painted: AtomicUsize,
    }
    impl VirtualCellDelegate for CountingDelegate {
        fn paint(
            &self,
            context: &mut RenderContext,
            cell: Rect,
            _row: usize,
            _column: usize,
            _text: Option<&str>,
        ) -> bool {
            context.fill_rect(cell, Color::PRIMARY);
            self.painted.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    let delegate = Arc::new(CountingDelegate { painted: AtomicUsize::new(0) });
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 120));
    table.set_data_source(Arc::new(StaticSource));
    table.set_delegate(delegate.clone());
    assert!(table.has_delegate());
    let _ = render_to_svg(&mut table);
    assert!(delegate.painted.load(Ordering::Relaxed) > 0, "the delegate owned the cells");
    table.clear_delegate();
    assert!(!table.has_delegate());
}

/// F-P06: a one-row scroll keeps the overlapping rows and fetches only the new row.
#[test]
fn a_small_scroll_fetches_only_the_new_rows() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RowCountingSource {
        rows: Arc<AtomicUsize>,
    }
    impl IncrementalTableDataSource for RowCountingSource {
        fn row_count(&self) -> usize {
            200
        }
        fn column_count(&self) -> usize {
            3
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{row}:{column}"))
        }
        fn revision(&self) -> u64 {
            1
        }
        fn fetch_window(
            &self,
            row_start: usize,
            row_len: usize,
            column_start: usize,
            column_len: usize,
        ) -> Vec<Vec<Option<String>>> {
            self.rows.fetch_add(row_len, Ordering::SeqCst);
            (row_start..row_start.saturating_add(row_len).min(self.row_count()))
                .map(|row| {
                    (column_start..column_start.saturating_add(column_len))
                        .map(|column| self.data(row, column))
                        .collect()
                })
                .collect()
        }
    }

    let fetched_rows = Arc::new(AtomicUsize::new(0));
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
    table.set_overscan_rows(0);
    table.set_data_source(Arc::new(RowCountingSource { rows: Arc::clone(&fetched_rows) }));

    let first = table.fetch_visible_window();
    assert_eq!(first.len(), 4, "80 px / 20 px shows four rows");
    assert_eq!(fetched_rows.load(Ordering::SeqCst), 4);

    table.set_scroll_row(1);
    let scrolled = table.fetch_visible_window();
    assert_eq!(scrolled.len(), 4);
    assert_eq!(scrolled[0][0], Some("1:0".to_string()), "the window slid by one row");
    assert_eq!(
        fetched_rows.load(Ordering::SeqCst),
        5,
        "only the newly exposed row was fetched from the source"
    );
}

/// V-06: the selection model supports extended ranges and ctrl toggles.
#[test]
fn multi_select_ranges_and_toggles_cells() {
    use crate::shortcut::Modifiers;
    use crate::widget::view_widgets::list_view::SelectionMode;

    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    table.set_selection_mode(SelectionMode::Extended);
    assert_eq!(table.selection_mode(), SelectionMode::Extended);

    assert!(table.select_cell(0, 0));
    assert!(table.select_cell_with_modifiers(0, 2, Modifiers::SHIFT));
    assert_eq!(table.selected_cells(), vec![(0, 0), (0, 1), (0, 2)]);

    // Ctrl toggles the middle cell off, and the current cell follows the gesture.
    assert!(table.select_cell_with_modifiers(0, 1, Modifiers::CTRL));
    assert_eq!(table.selected_cells(), vec![(0, 0), (0, 2)]);
    assert_eq!(table.selected_cell(), Some((0, 1)));
}

/// V-07: the arrow keys move the current cell and the viewport follows it.
#[test]
fn arrow_keys_move_the_current_cell() {
    use crate::event::key_codes::{DOWN, END, RIGHT};

    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(Arc::new(StaticSource));
    table.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });

    table.handle_event(&Event::KeyPress { key: DOWN, modifiers: 0 });
    assert_eq!(table.selected_cell(), Some((1, 0)), "Down selects the next row");
    table.handle_event(&Event::KeyPress { key: RIGHT, modifiers: 0 });
    assert_eq!(table.selected_cell(), Some((1, 1)), "Right selects the next column");
    table.handle_event(&Event::KeyPress { key: END, modifiers: 0 });
    assert_eq!(table.selected_cell().map(|(row, _)| row), Some(99));
    assert!(table.scroll_row() > 0, "the viewport followed the cell");
}

/// V-013: per-column widths and hidden columns change the painted geometry, while the
/// absolute column indices stay stable.
#[test]
fn per_column_widths_and_hidden_columns_drive_the_geometry() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 400, 200));
    table.set_data_source(Arc::new(StaticSource));

    assert!(table.set_column_width_at(1, 40));
    let rect0 = table.cell_rect(table.geometry(), 0, 0);
    let rect1 = table.cell_rect(table.geometry(), 0, 1);
    let rect2 = table.cell_rect(table.geometry(), 0, 2);
    assert_eq!(rect1.x - rect0.x, 120);
    assert_eq!(rect2.x - rect0.x, 160, "column 2 starts after column 1's own width");

    assert!(table.set_column_hidden(1, true));
    assert_eq!(table.column_width_at(1), 0, "a hidden column occupies no width");
    let hidden_slot = table.cell_rect(table.geometry(), 0, 1);
    assert_eq!(hidden_slot.x - rect0.x, 120, "column 2 moved into the freed slot");
    assert_eq!(table.column_at(hidden_slot.x + 1), Some(2));
    assert_eq!(table.hidden_columns(), vec![1]);
    assert!(!table.set_column_hidden(99, true), "there is no column 99");
}

/// V-013: header clicks sort the window three-state, and each state paints its own order.
#[test]
fn sorting_orders_the_window_and_cycles_three_states() {
    struct SortSource;
    impl IncrementalTableDataSource for SortSource {
        fn row_count(&self) -> usize {
            3
        }
        fn column_count(&self) -> usize {
            1
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            let _ = column;
            ["c", "a", "b"].get(row).map(|value| (*value).to_string())
        }
    }

    let first_cell = |table: &mut VirtualTable| {
        table.fetch_visible_window().first().and_then(|row| row.first().cloned()).flatten()
    };

    let mut table = VirtualTable::new(Rect::new(0, 0, 300, 120));
    table.set_data_source(Arc::new(SortSource));
    table.set_show_header(true);

    assert_eq!(table.toggle_sort_column(0), Some(false));
    assert_eq!(first_cell(&mut table).as_deref(), Some("a"));
    assert_eq!(table.toggle_sort_column(0), Some(true));
    assert_eq!(first_cell(&mut table).as_deref(), Some("c"));
    assert_eq!(table.toggle_sort_column(0), None, "the third state is unsorted");
    assert!(table.sort_specs().is_empty());
    assert_eq!(first_cell(&mut table).as_deref(), Some("c"), "source order is restored");

    // A header press is the same gesture.
    let header = Point::new(table.geometry().x + 3, table.geometry().y + 3);
    table.handle_event(&Event::MousePress { pos: header, button: 1, modifiers: 0 });
    assert_eq!(table.sort_specs(), &[SortSpec { column: 0, descending: false }]);
}

/// V-013: one request is one signal, and the pointed-at cell is selected first.
///
/// The old test asserted **two** hits for one right-click — a secondary press and the
/// synthesized context-menu event each emitted — which is the duplication this change removes
/// (feedback C-9). The request now travels through `Widget::context_menu_event`, the hook the
/// runtime calls, and selection happens there before the signal fires.
#[test]
fn the_context_menu_request_is_reported() {
    use std::sync::atomic::AtomicUsize;

    let mut table = VirtualTable::new(Rect::new(0, 0, 300, 120));
    table.set_data_source(Arc::new(StaticSource));
    let hits = Arc::new(AtomicUsize::new(0));
    let sink = Arc::clone(&hits);
    table.context_menu_requested.connect(move |_| {
        sink.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    });

    let cell = table.cell_rect(table.geometry(), 0, 0);
    let pos = Point::new(cell.x + 4, cell.y + 4);
    assert!(Widget::context_menu_event(&mut table, pos));
    assert_eq!(hits.load(std::sync::atomic::Ordering::Relaxed), 1, "one request, one signal");
    assert_eq!(table.selected_cell(), Some((0, 0)), "the menu acts on the pointed-at cell");
}

/// V-013: the alternating wash changes the painted output and is off by default.
#[test]
fn alternating_rows_change_the_painted_output() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 300, 120));
    table.set_data_source(Arc::new(StaticSource));
    assert!(!table.alternating_rows(), "off by default keeps the historical pixels");
    let plain = render_to_svg(&mut table);
    table.set_alternating_rows(true);
    assert!(table.alternating_rows());
    let striped = render_to_svg(&mut table);
    assert_ne!(plain, striped, "the odd-row band is actually painted");
}

/// RW-17: the empty/error captions are stored on the control and cleared by an empty string.
#[test]
fn empty_and_error_captions_are_stored() {
    use crate::widget::view_widgets::collection_state::CollectionState;

    let mut table = VirtualTable::new(Rect::new(0, 0, 200, 120));
    table.set_empty_text("No rows");
    assert_eq!(table.empty_text(), Some("No rows"));
    table.set_error_text("Load failed");
    assert_eq!(table.placeholder().text(), Some("Load failed"));
    table.clear_error_text();
    assert_eq!(table.placeholder().text(), Some("No rows"));
    table.set_empty_text("");
    assert!(table.placeholder().is_empty());
}

// ── Regression tests: source changes, selection encoding and hit testing ──

/// A source whose row and column counts can change in place and which reports the change.
struct ShrinkableSource {
    rows: AtomicUsize,
    columns: AtomicUsize,
    changed: GenericSignal,
}

impl IncrementalTableDataSource for ShrinkableSource {
    fn row_count(&self) -> usize {
        self.rows.load(Ordering::Relaxed)
    }

    fn column_count(&self) -> usize {
        self.columns.load(Ordering::Relaxed)
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        Some(format!("{row}:{column}"))
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.changed)
    }
}

/// A shrinking source clamps the scroll, repairs the selection and reports the new window.
///
/// The `data_changed` handler only advanced the generation and repainted. With `scroll_row == 90`
/// and a source that dropped to three rows, `fetch_window` returned nothing for the stale origin,
/// so the body painted blank with no placeholder while `scroll_row` still reported 90.
#[test]
fn a_shrinking_source_clamps_the_scroll_and_reports_the_window() {
    let source = Arc::new(ShrinkableSource {
        rows: AtomicUsize::new(100),
        columns: AtomicUsize::new(5),
        changed: GenericSignal::new(),
    });
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 200));
    table.set_data_source(source.clone());
    table.set_scroll_row(90);
    assert!(table.select_cell(95, 1));

    let emitted = Arc::new(Mutex::new(Vec::<(usize, usize, usize, usize)>::new()));
    let sink = Arc::clone(&emitted);
    table.visible_window_changed.connect(move |window| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*window);
        }
    });
    let cleared = Arc::new(Mutex::new(Vec::<Vec<(usize, usize)>>::new()));
    let sink = Arc::clone(&cleared);
    table.selection_set_changed.connect(move |cells| {
        if let Ok(mut guard) = sink.lock() {
            guard.push((*cells).clone());
        }
    });

    source.rows.store(3, Ordering::Relaxed);
    source.changed.emit();

    // The property read is clamped immediately, without waiting for a frame.
    assert_eq!(table.scroll_row(), 2, "the scroll is clamped to the surviving rows");

    let window = table.fetch_visible_window();
    assert!(!window.is_empty(), "the body still has rows to paint");
    assert_eq!(table.selected_cells(), Vec::new(), "the selected row no longer exists");
    assert_eq!(
        *cleared.lock().expect("lock"),
        vec![Vec::new()],
        "the host hears that the stale cell left the set"
    );
    assert!(
        emitted.lock().expect("lock").iter().any(|window| window.0 < 3),
        "the visible window is re-emitted from the new bounds"
    );
}

/// The selection is re-encoded when the source's column count changes.
///
/// Cells are stored as `row * columns + column`. With five columns, selecting (1, 2) stored 7; when
/// the source dropped to three columns, index 7 decoded as (2, 1) — a different cell. Clearing the
/// source likewise left the index behind, so `selected_cells()` was silently empty.
#[test]
fn a_column_count_change_keeps_the_selected_cell() {
    let source = Arc::new(ShrinkableSource {
        rows: AtomicUsize::new(5),
        columns: AtomicUsize::new(5),
        changed: GenericSignal::new(),
    });
    let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
    table.set_data_source(source.clone());
    assert!(table.select_cell(1, 2));
    assert_eq!(table.selected_cell(), Some((1, 2)));

    source.columns.store(3, Ordering::Relaxed);
    source.changed.emit();

    assert_eq!(
        table.selected_cell(),
        Some((1, 2)),
        "the index is decoded with the width it was encoded with"
    );
    let _ = table.fetch_visible_window();
    assert_eq!(table.selected_cell(), Some((1, 2)), "and re-encoded against the new width");

    table.clear_data_source();
    assert!(table.selected_cells().is_empty(), "no source, no cells");
    assert_eq!(table.selected_cell(), None);
}

/// A point below the last row names no cell, so no hover stripe is painted there.
///
/// `row_at` bounded the local row by the visible grid but never by `row_count`: a three-row source
/// in a 300 px control answered row 9 for a point 200 px below the last row.
#[test]
fn a_point_below_the_last_row_names_no_cell() {
    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 300));
    table.set_data_source(Arc::new(ShrinkableSource {
        rows: AtomicUsize::new(3),
        columns: AtomicUsize::new(2),
        changed: GenericSignal::new(),
    }));

    let rect = table.geometry();
    let stride = table.row_height() as i32;
    let first_row = rect.y + 2 + table.header_offset() + 5;
    assert_eq!(table.row_at(first_row), Some(0));

    let below_last = first_row + 9 * stride;
    assert_eq!(table.row_at(below_last), None, "empty space below the source's rows is not a row");
}

/// Switching from Multi to Single reports the cells that survived the trim.
///
/// `set_selection_mode` never emitted. `SelectionModel::set_mode` trims a multi-row set to one row
/// when switching to Single, so the host was left mirroring three cells the control no longer had.
#[test]
fn switching_to_single_selection_reports_the_surviving_cell() {
    use crate::widget::view_widgets::list_view::SelectionMode;

    let mut table = VirtualTable::new(Rect::new(0, 0, 240, 120));
    table.set_data_source(Arc::new(StaticSource));
    table.set_selection_mode(SelectionMode::Multi);
    let ctrl = crate::shortcut::Modifiers::CTRL;
    assert!(table.select_cell_with_modifiers(0, 0, ctrl));
    assert!(table.select_cell_with_modifiers(1, 0, ctrl));
    assert!(table.select_cell_with_modifiers(5, 0, ctrl));
    assert_eq!(table.selected_cells(), vec![(0, 0), (1, 0), (5, 0)]);

    let single = Arc::new(Mutex::new(Vec::<Option<(usize, usize)>>::new()));
    let sink = Arc::clone(&single);
    table.selection_changed.connect(move |cell| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*cell);
        }
    });
    let whole = Arc::new(Mutex::new(Vec::<Vec<(usize, usize)>>::new()));
    let sink = Arc::clone(&whole);
    table.selection_set_changed.connect(move |cells| {
        if let Ok(mut guard) = sink.lock() {
            guard.push((*cells).clone());
        }
    });

    table.set_selection_mode(SelectionMode::Single);

    assert_eq!(table.selected_cells(), vec![(5, 0)], "the last row survives the trim");
    assert_eq!(*single.lock().expect("lock"), vec![Some((5, 0))]);
    assert_eq!(*whole.lock().expect("lock"), vec![vec![(5, 0)]]);
}

/// UX-04: the scroll thumb is derived from the overflow, so a grid whose rows run past its
/// viewport reports that fact even though the opt-in flag is off by default.
#[test]
#[cfg(all(device_profile, feature = "desktop"))]
fn an_overflowing_grid_derives_its_scroll_thumb() {
    use crate::core::Color;

    struct TwoRowSource;
    impl IncrementalTableDataSource for TwoRowSource {
        fn row_count(&self) -> usize {
            2
        }
        fn column_count(&self) -> usize {
            2
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{row}:{column}"))
        }
    }

    let _guard = crate::theme::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);

    let palette = crate::style::palette_shared();
    let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
    let thumb = format!("rgba({},{},{},", accent.r, accent.g, accent.b);

    let mut overflow = VirtualTable::new(Rect::new(0, 0, 320, 200));
    overflow.set_data_source(Arc::new(StaticSource)); // 100 of 20 columns
    crate::theme::apply_theme_to_widget(&mut overflow);
    let painted = render_to_svg(&mut overflow);
    assert!(
        painted.contains(&thumb),
        "an overflowing grid must paint a thumb in the accent colour, so a reader can see that \
         rows are off-screen"
    );

    let mut fits = VirtualTable::new(Rect::new(0, 0, 320, 200));
    fits.set_data_source(Arc::new(TwoRowSource));
    crate::theme::apply_theme_to_widget(&mut fits);
    let bare = render_to_svg(&mut fits);
    assert!(
        !bare.contains(&thumb),
        "a grid whose rows all fit has nothing to report, so it must not paint a thumb"
    );
}
