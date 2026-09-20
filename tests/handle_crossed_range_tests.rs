// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Crossed-range regression tests for the typed widget handles.
//!
//! # The defect class
//!
//! `SliderHandle`/`SpinBoxHandle`/`ProgressBarHandle` each keep an in-process
//! `(min, max)` mirror and clamp writes against it. The bounds are mutable
//! *fields*, not constants, so any caller can cross them — `set_range(100, 0)` is
//! a descending slider, a perfectly ordinary thing to want. `i32::clamp` and
//! `u32::clamp` **panic** on `min > max`, so the clamp was a landmine: the panic
//! fired on the *next* write, at a line that has nothing to do with the bad
//! range, on every backend including the self-drawn one. `set_min(100)` on a
//! `0..100` progress bar additionally forwarded `(100, 0)` to the backend, where
//! `widget_range()` read it back inverted.
//!
//! The repair is `ordered_clamp_*` (see `widget::numeric`), which clamps into the
//! span the two bounds define in either order. These tests pin the observable
//! contract: a crossed range is *accepted*, the value lands inside the span the
//! range defines, and the backend is never handed an inverted pair.

#![cfg(feature = "desktop")]

use rust_widgets::app::App;
use rust_widgets::app::WidgetHandle as _;

/// One `App` shared by the tests below; window/widget ids are per-test literals
/// so the tests can run in any order on the same thread pool.
fn app() -> App {
    let mut app = App::new();
    app.init();
    app
}

#[test]
fn slider_accepts_crossed_range_without_panicking() {
    let app = app();
    let win = app.new_window("slider-crossed", 0, 0, 400, 300);

    let s = win.new_slider(0, 0, 100, 20);
    s.set_range(0, 100);
    s.set_value(40);
    assert_eq!(s.value(), 40);

    // Descending range: value 40 stays inside the span [0, 100].
    s.set_range(100, 0);
    assert_eq!(s.value(), 40, "40 lies within the span the crossed range defines");

    // A value outside the span clamps to the span's near end.
    s.set_value(150);
    assert_eq!(s.value(), 100);
    s.set_value(-10);
    assert_eq!(s.value(), 0);

    // A range that excludes the value pulls it to the nearest bound, in either
    // written order — this is the ordering the crossed case relies on.
    s.set_range(200, 300);
    assert_eq!(s.value(), 200);
    s.set_range(300, 200);
    assert_eq!(s.value(), 200, "200 is inside the span [200, 300] either way round");
    s.set_range(400, 500);
    assert_eq!(s.value(), 400);

    // The backend must never be handed an inverted pair.
    if let Some((min, max)) = s.range() {
        assert!(min <= max, "backend stored inverted range ({min}, {max})");
    }
}

#[test]
fn spinner_accepts_crossed_range_without_panicking() {
    let app = app();
    let win = app.new_window("spinbox-crossed", 0, 0, 400, 300);

    let sb = win.new_spin_box(0, 0, 100, 20);
    sb.set_range(0, 100);
    sb.set_value(40);
    assert_eq!(sb.value(), 40);

    sb.set_range(100, 0);
    assert_eq!(sb.value(), 40, "40 lies within the span the crossed range defines");

    sb.set_value(150);
    assert_eq!(sb.value(), 100);
    sb.set_value(-10);
    assert_eq!(sb.value(), 0);

    if let Some((min, max)) = sb.range() {
        assert!(min <= max, "backend stored inverted range ({min}, {max})");
    }
}

#[test]
fn progress_bar_min_above_max_does_not_persist_inverted_range() {
    let app = app();
    let win = app.new_window("progress-crossed", 0, 0, 400, 300);

    let pb = win.new_progress_bar(0, 0, 100, 20);
    pb.set_range(0.0, 100.0);
    pb.set_value(40);
    assert_eq!(pb.value(), 40);

    // Crossing from below: `min` becomes 100 with `max` still 100, so the span
    // collapses to [100, 100] and 40 clamps up to 100. The point of the test is
    // that this is a *defined* result rather than a `u32::clamp` panic, and that
    // the backend is handed an ordered pair.
    pb.set_min(100);
    assert_eq!(pb.value(), 100, "the collapsed span [100, 100] pulls the value up");
    if let Some((min, max)) = pb.range() {
        assert!(min <= max, "set_min left the backend with ({min}, {max})");
    }

    // The value writes that follow must not panic either, and must stay in span.
    pb.set_value(150);
    assert_eq!(pb.value(), 100);
    pb.set_value(0);
    assert_eq!(pb.value(), 100);
}

#[test]
fn progress_bar_max_below_min_does_not_persist_inverted_range() {
    let app = app();
    let win = app.new_window("progress-crossed-2", 0, 0, 400, 300);

    let pb = win.new_progress_bar(0, 0, 100, 20);
    pb.set_range(0.0, 100.0);
    pb.set_value(40);

    // `set_max(0)` with `min` still 0 collapses the span to [0, 0].
    pb.set_max(0);
    assert_eq!(pb.value(), 0, "the collapsed span [0, 0] pulls the value down");
    if let Some((min, max)) = pb.range() {
        assert!(min <= max, "set_max left the backend with ({min}, {max})");
    }
    pb.set_value(150);
    assert_eq!(pb.value(), 0, "the value stays inside the collapsed span");
}

/// The ordinary, non-crossed case must not regress: the fix is an ordering, not
/// a widening of the accepted values.
#[test]
fn ordered_ranges_still_clamp_normally() {
    let app = app();
    let win = app.new_window("ordered", 0, 0, 400, 300);

    let s = win.new_slider(0, 0, 100, 20);
    s.set_range(10, 20);
    s.set_value(5);
    assert_eq!(s.value(), 10);
    s.set_value(99);
    assert_eq!(s.value(), 20);
    s.set_value(15);
    assert_eq!(s.value(), 15);

    let pb = win.new_progress_bar(0, 30, 100, 20);
    // `set_min`/`set_max` write the mirror the clamp reads; the trait's
    // `set_range` only reaches the backend, so it cannot be used to set up the
    // in-process range these assertions are about.
    pb.set_range(0.0, 10.0);
    pb.set_min(0);
    pb.set_max(10);
    pb.set_value(50);
    assert_eq!(pb.value(), 10);
    pb.set_value(3);
    assert_eq!(pb.value(), 3);
}
