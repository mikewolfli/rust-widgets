// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Behavioural tests for the sample fill.
//!
//! A child test module (`sample_fill/tests.rs`) rather than lines in the production file;
//! test length measures coverage, not coupling (principle #9).

#![cfg(test)]

use super::*;
use crate::widget::WidgetFactory;

/// The names [`apply`] actually handles, derived from its own source.
///
/// # Why this is parsed rather than listed
///
/// The previous literal list had drifted exactly once, and the drift was invisible:
/// four arms (`bottom_sheet`, `modal_bottom_sheet`, `dialog`, `message_box`) were
/// added without a name in the list, so nothing asserted their downcast still matched
/// (SNAP-N02). Reading the arm patterns means "which names are handled" and "which
/// names are tested" are the same fact, so a new arm cannot be untested by omission.
///
/// The scan is bounded to the body of `apply`: it starts at its signature and stops at
/// the first column-zero `}`, so the helper functions and the test module below are not
/// parsed. Arm patterns are the only lines before an `=>` that contain quoted names.
fn declared_arm_names() -> Vec<&'static str> {
    const SOURCE: &str = include_str!("../sample_fill.rs");
    let mut names = Vec::new();
    let mut in_apply = false;
    for line in SOURCE.lines() {
        if line.starts_with("pub fn apply(name: &str") {
            in_apply = true;
            continue;
        }
        if !in_apply {
            continue;
        }
        if line == "}" {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some((pattern, _)) = trimmed.split_once("=>") else { continue };
        let mut rest = pattern;
        while let Some(start) = rest.find('"') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('"') else { break };
            names.push(&after[..end]);
            rest = &after[end + 1..];
        }
    }
    names.sort_unstable();
    names.dedup();
    names
}

/// Every name [`apply`] claims to know actually reaches a control that exists.
///
/// # Why this is the load-bearing test
///
/// The fill is by **downcast**: `apply` matches a canonical name, then tries `widget_as_mut` on one
/// concrete type. If a control is renamed in the registry, or moves to a different type with the same
/// name, the match arm is still reached and the downcast simply returns `None` — the fill silently
/// stops working and the snapshot quietly goes back to being an empty frame. That is exactly the defect
/// this module exists to fix, so it must not be able to return unnoticed.
///
/// Building each named control through the real factory is what closes the loop: the downcast has to
/// succeed against the type the *registry* actually constructs.
#[test]
fn every_filled_name_downcasts_to_the_type_the_registry_builds() {
    let factory = WidgetFactory::new_with_defaults();
    let claimed = declared_arm_names();
    assert!(
        claimed.len() >= 60,
        "the arm scan found {} names, which is too few to be the real table",
        claimed.len()
    );
    for name in claimed {
        // A feature-gated control (the editors, BLUE31 §1.5) has an arm in the source but no
        // entry in this build's registry, so there is nothing to downcast to. The skip is derived
        // from the *same* gate the registration uses rather than from a hard list: a control that
        // is missing from a build which does enable its feature still reaches the assertion below
        // and fails, so a rename cannot hide behind this branch.
        //
        // `markdown_outline_panel` and `find_bar` are the two arms `apply` gates; the editors
        // themselves have no fill arm of their own. Early `continue`s rather than a computed bool:
        // each test is a `cfg!` literal in the build being compiled, and clippy folds a
        // boolean-returning `if`/`else` of literals — the fold would drop the gate.
        if name == "markdown_outline_panel" && !cfg!(feature = "markdown-editor") {
            assert!(
                factory.capability(name).is_none(),
                "{name} is gated out of this build but still registered"
            );
            continue;
        }
        if name == "find_bar" && !cfg!(any(feature = "code-editor", feature = "markdown-editor")) {
            assert!(
                factory.capability(name).is_none(),
                "{name} is gated out of this build but still registered"
            );
            continue;
        }
        let Some(mut widget) = factory.create(name, crate::widget::census::CENSUS_RECT, "Sample")
        else {
            panic!("{name} is claimed as fillable but the factory cannot build it");
        };
        assert!(
            apply(name, widget.as_mut()),
            "{name}: the fill matched nothing, so its snapshot would be an empty frame again"
        );
    }
}

/// A name with no data concept is left alone, and the call reports it.
///
/// The negative half matters as much as the positive: a fill that returned `true` for everything would
/// make the exporter's `sample-filled` count meaningless, and a control whose content was written by the
/// wrong arm would be counted as a success.
/// A control with no *data* concept is not filled by [`apply`].
///
/// # Why `slider` is no longer in this list
///
/// It was, and correctly: `slider` has no data to load. But it does have a **feature** that is
/// invisible in its default state — its tick marks are guarded on both a non-`NoTicks` position
/// and a non-zero interval, and the constructor sets neither, so the marks belonged to no
/// snapshot. The module fills it for the same reason it fills `group_box`'s tick, which is a
/// different question from "does it hold data"; `slider` therefore moved out of this list and
/// into the feature list covered by [`a_state_feature_is_switched_on_for_its_own_snapshot`].
#[test]
fn a_control_without_a_data_concept_is_not_filled() {
    let factory = WidgetFactory::new_with_defaults();
    for name in ["button", "label", "divider", "line_edit"] {
        let Some(mut widget) = factory.create(name, crate::widget::census::CENSUS_RECT, "Sample")
        else {
            continue;
        };
        assert!(!apply(name, widget.as_mut()), "{name} has no data to fill");
    }
}

/// Each control whose *state* is the feature has that state switched on for its snapshot.
///
/// # Why this needs its own assertion
///
/// A control that draws something in one state and nothing in the other makes every check the
/// crate has agree on a picture that shows no feature: the tick branch, the travel branch and
/// the tick-mark branch are all unreachable in the default state, so a defect inside them is
/// invisible to the snapshot, the census colour sample and every gate. This pins the set that
/// has been switched on, so a control dropping out of it is a failure rather than a quietly
/// emptier picture.
#[test]
fn a_state_feature_is_switched_on_for_its_own_snapshot() {
    use crate::widget::display_widgets::slider::{Slider, TickPosition};
    use crate::widget::display_widgets::switch::Switch;

    let factory = WidgetFactory::new_with_defaults();
    for name in ["check_box", "switch", "slider", "group_box"] {
        let Some(mut widget) = factory.create(name, crate::widget::census::CENSUS_RECT, "Sample")
        else {
            panic!("{name} is published by the registry but could not be built");
        };
        assert!(apply(name, widget.as_mut()), "{name} owns a state feature");
    }

    // The state each one depends on is genuinely on, rather than the fill merely returning
    // `true`: a `Slider` needs both halves of its guard, and the constructor sets neither.
    let mut slider = Slider::new(crate::core::Rect::new(0, 0, 120, 8));
    assert_eq!(slider.tick_position(), TickPosition::NoTicks, "the constructor default");
    assert!(apply("slider", &mut slider));
    assert_ne!(slider.tick_position(), TickPosition::NoTicks, "and the fill moved it");
    assert!(slider.tick_interval() > 0, "or the draw guard still refuses the marks");

    let mut switch = Switch::new(crate::core::Rect::new(0, 0, 60, 30));
    assert!(!switch.is_checked(), "the constructor default");
    assert!(apply("switch", &mut switch));
    assert!(switch.is_checked(), "and the fill turned it on");
}

/// The calendar's snapshot is reproducible: its date comes from the fill, not the clock.
///
/// # The defect this pins
///
/// `Calendar::new` seeds `selected_date`/`display_month` from `chrono::Local::now()`. That is
/// right for the control and fatal for a committed artifact: the selection highlight is a
/// function of the date, so `snapshots/svg/calendar.svg` changed at every midnight and
/// `check_svg_snapshots.sh` reported a diff on the day after it was committed — a red gate no
/// edit could explain, which the only available repair (re-commit the regenerated file)
/// turned into the false-green direction.
///
/// The assertion is what a *snapshot* needs rather than what a *control* needs: the fill must
/// land on a fixed day, and it must land there from every starting date. The control's own
/// today-default is asserted alongside, so a fix that pinned the constructor instead of the
/// sample cannot pass this.
#[test]
fn the_calendar_sample_is_pinned_to_a_fixed_day() {
    // Holds the crate-wide theme guard: this test renders twice and compares the two frames,
    // and a concurrent test that switches the active theme would otherwise change the accent
    // colours between them — which made the comparison fail with two perfectly correct pictures
    // that differed only in the theme's reds and the today-highlight fill
    // (`rgba(179,62,60)` vs `rgba(183,50,41)`, `rgba(255,193,7)` vs `rgba(255,213,79)`).
    // Same reason and same precedent as the render tests in `widget::svg` and `avatar`.
    let _theme_guard = crate::style::theme_test_guard();
    let rect = crate::core::Rect::new(0, 0, 240, 120);
    let mut calendar = Calendar::new(rect);
    assert_eq!(
        calendar.selected_date(),
        chrono::Local::now().date_naive(),
        "the constructor still opens on today, which a host depends on"
    );
    assert!(apply("calendar", &mut calendar), "the calendar owns a clock-derived default");
    assert_eq!(
        calendar.selected_date(),
        chrono::NaiveDate::from_ymd_opt(2026, 9, 16).expect("a valid literal date"),
        "the sample must name a fixed day or the snapshot changes every midnight"
    );
    // The displayed month follows the selection, so the grid shows the pinned month too.
    assert_eq!(calendar.display_month(), calendar.selected_date());
    // And the **today** accent is pinned to the same day. `draw` used to read the wall clock
    // itself, so this cell moved at every midnight even with the selection pinned above — the
    // snapshot was still date-dependent, just less obviously so. Without this assertion that
    // half of the rotation is free to come back.
    assert_eq!(
        calendar.today(),
        chrono::NaiveDate::from_ymd_opt(2026, 9, 16).expect("a valid literal date"),
        "the painted day must come from the fixture too, or the picture rotates nightly"
    );

    // Redrawing on a different wall-clock day must not change a single byte: that is the
    // property the pin exists for. The `today` field is the only clock-derived input left.
    let once = crate::widget::svg::render_to_svg(&mut calendar);
    let twice = crate::widget::svg::render_to_svg(&mut calendar);
    assert_eq!(once, twice, "a pinned calendar renders identically on every draw");
}

/// The table's two routes expose the same data, so `data_grid.svg` and `table_widget.svg` differ only
/// by layout.
///
/// Two datasets would make every comparison between those snapshots a comparison of content as well,
/// which is how a layout regression gets read as "the data changed".
#[test]
fn both_table_routes_expose_the_same_cells() {
    for row in 0..sample::ROWS.len() {
        for column in 0..sample::HEADERS.len() {
            assert_eq!(
                SampleTableModel.data(row, column),
                SampleTableSource.data(row, column),
                "row {row} column {column} disagrees between the two table routes"
            );
        }
    }
    assert_eq!(SampleTableModel.row_count(), SampleTableSource.row_count());
    assert_eq!(SampleTableModel.column_count(), SampleTableSource.column_count());
    // And an out-of-range cell is absent rather than fabricated.
    assert_eq!(SampleTableModel.data(99, 0), None);
    assert_eq!(SampleTableSource.data(0, 99), None);
}

/// Applying the popup sample twice reuses its mounted content instead of leaking a label.
///
/// # The defect this pins
///
/// The arm called `runtime::register(Box::new(Label…))` on every call and never unregistered the
/// previous label. `PopupWindow::set_content_widget` unlinks the replaced child but does not
/// unregister it — that is the caller's job, as `compose_container` documents — so each
/// `apply("popup_window", …)` left a mounted widget and its accessibility node behind. The fill
/// now registers only when the popup has no content yet.
#[test]
fn applying_the_popup_sample_twice_reuses_its_mounted_content() {
    let mut popup = PopupWindow::new(crate::core::Rect::new(0, 0, 240, 120));
    assert!(apply("popup_window", &mut popup), "the popup owns sample content");
    let first = popup.content_widget().expect("the sample mounts a content label");
    assert!(crate::widget::runtime::is_mounted(first));

    assert!(apply("popup_window", &mut popup));
    assert_eq!(
        popup.content_widget(),
        Some(first),
        "a second fill must not replace and abandon the mounted label"
    );
    assert!(crate::widget::runtime::is_mounted(first));
    assert!(crate::widget::runtime::unregister(first), "clean up the mounted fixture");
}
