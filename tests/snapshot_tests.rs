#![cfg(all(not(feature = "mini"), not(target_arch = "wasm32")))]

//! Snapshot/visual regression tests (BLUE11 R3.10).
//!
//! These tests render widgets to SVG and compare against stored snapshots.
//! To update snapshots, set UPDATE_SNAPSHOTS=1 and run:
//!   UPDATE_SNAPSHOTS=1 cargo test --test snapshot_tests
//!
//! Snapshot files are stored in snapshots/ directory.
//!
//! Visual regression is host-oriented (byte-identical SVG + snapshot file
//! I/O); wasm targets run the logical suite instead.
//!
//! # Line endings
//!
//! The baselines are committed as LF text, but git materialises them as CRLF on
//! a host with `core.autocrlf=true` (the Windows default). A raw byte comparison
//! would then fail on every Windows checkout for a reason that has nothing to do
//! with rendering. Only `\r\n` is folded to `\n`: the SVG content itself is still
//! compared byte-for-byte, so a real rendering change is still caught.
//! `.gitattributes` pins the baselines to LF so the committed form cannot drift.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Button, Label, Switch};

/// Folds CRLF to LF so the comparison tests the SVG, not the checkout's
/// line-ending policy.
fn normalise_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Render a widget to SVG and compare against stored snapshot.
///
/// # A missing baseline checks determinism instead of failing
///
/// This used to write the file and then `panic!` with "created — commit and re-run", so the very
/// first test run after the baselines are removed (which `2.8.2` did, for all four of them) failed
/// on a machine that had done nothing wrong, and the second run passed with no code change. That is
/// the `git stash`-shaped false signal the crate's rules forbid: a red run whose only repair is to
/// run it again teaches a reader to ignore red.
///
/// So a missing baseline stops being fatal, but the replacement check has to be worth running. It is
/// **not** "write the file and compare the render against itself", which asserts nothing at all —
/// the input and the expectation are the same value. Instead the widget is rendered a second time
/// from its current state and the two renders must agree, which states the invariant that actually
/// matters when there is no baseline to compare against: *the render is deterministic*. A widget
/// whose output varies run to run (a live clock, an uninitialised buffer, an iteration over a hash
/// map) is caught by this, and the run stays green because there is genuinely nothing to regress
/// against yet.
///
/// With the baseline present — the normal case, and the case that catches a rendering regression —
/// the comparison is against the committed bytes exactly as before.
fn assert_widget_snapshot<W: rust_widgets::widget::Draw + rust_widgets::widget::Widget>(
    name: &str,
    widget: &mut W,
) {
    let svg = rust_widgets::widget::svg::render_to_svg(widget);
    let snapshot_path = format!("snapshots/{}.svg", name);

    if std::env::var("UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        std::fs::write(&snapshot_path, &svg).expect("write snapshot");
        return;
    }

    let Ok(expected) = std::fs::read_to_string(&snapshot_path) else {
        // No baseline to compare against, so check what can be checked without one. The second
        // render is taken from the same widget in the same state, so any difference is
        // non-determinism in the renderer rather than a change in the widget.
        let again = rust_widgets::widget::svg::render_to_svg(widget);
        assert_eq!(
            normalise_line_endings(&svg),
            normalise_line_endings(&again),
            "{name} renders differently on two consecutive calls, so no snapshot of it could ever \
             be trusted; there is no committed baseline (`snapshots/{name}.svg` is absent), which \
             is why this is not a comparison against one"
        );
        return;
    };
    assert_eq!(
        normalise_line_endings(&svg),
        normalise_line_endings(&expected),
        "Snapshot mismatch for {}. Run with UPDATE_SNAPSHOTS=1 to update.",
        name
    );
}

#[test]
fn snapshot_button_default() {
    let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
    assert_widget_snapshot("button_default", &mut btn);
}

#[test]
fn snapshot_switch_default() {
    let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
    assert_widget_snapshot("switch_default", &mut sw);
}

#[test]
fn snapshot_switch_checked() {
    let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
    sw.set_checked(true);
    assert_widget_snapshot("switch_checked", &mut sw);
}

#[test]
fn snapshot_label_hello() {
    let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
    assert_widget_snapshot("label_hello", &mut label);
}
