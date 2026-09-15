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

    if let Ok(expected) = std::fs::read_to_string(&snapshot_path) {
        assert_eq!(
            normalise_line_endings(&svg),
            normalise_line_endings(&expected),
            "Snapshot mismatch for {}. Run with UPDATE_SNAPSHOTS=1 to update.",
            name
        );
    } else {
        std::fs::write(&snapshot_path, &svg).expect("write initial snapshot");
        panic!("Snapshot {} did not exist — created. Commit and re-run.", snapshot_path);
    }
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
