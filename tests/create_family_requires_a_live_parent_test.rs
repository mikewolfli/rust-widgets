// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Every public `create_*` entry behaves the same way: a live parent is required, and a real one
//! produces a control.
//!
//! # Why this pins the *family* rather than one control
//!
//! A designer feedback round reported `create_file_dialog(0, 0, 0, 640, 420) == 0` and read it as
//! "the desktop backends have no file dialog, so the chooser is missing there". Both halves of that
//! reading are wrong, and nothing in the tree said so:
//!
//! * `0` is the documented answer for an **unusable parent**, and it is the answer for *every*
//!   non-window control — `create_button(0, …)` is `0` for the same reason. The call shape in the
//!   report, not the control, produced the `0`.
//! * The file chooser is not missing: it is a control like the others, and it mounts like the
//!   others (this crate paints every control itself — see `PlatformCapabilities`, which has no
//!   `file_dialog` flag for that reason).
//!
//! One control's contract being checked while its 16 siblings were not is what let the
//! misattribution stand as evidence. This test walks the whole family, so "the dialogs are the
//! broken ones" cannot be concluded from a single call again.
//!
//! # What is asserted
//!
//! 1. With `parent = 0`, **every** non-window entry answers `0` — including `message_box`,
//!    `file_dialog`, `color_dialog` and `font_dialog`, which are the ones a reader is most likely to
//!    mistake for host-provided dialogs.
//! 2. With a real window parent, **every** one of them mounts (a non-zero id), and the ids are
//!    distinct — so no two entries can be silently the same control.
//!
//! (1) alone would pass if creation were simply broken; (2) alone would pass if `parent` were
//! ignored. Both together are the contract the C ABI documents ("an unknown parent is rejected
//! with 0").

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::core::ObjectId;

/// One entry of the public creation surface, addressed by name so a failure says which one.
///
/// Every entry is spelled out rather than generated: the point is to enumerate the surface the
/// report reasoned about, and a list built from a registry would silently follow the registry if
/// an entry disappeared.
fn create_family(window: ObjectId) -> Vec<(&'static str, ObjectId)> {
    vec![
        ("button", rust_widgets::create_button(window, "b", 0, 0, 200, 80)),
        ("checkbox", rust_widgets::create_checkbox(window, "c", 0, 0, 200, 80)),
        ("line_edit", rust_widgets::create_line_edit(window, "x", 0, 0, 200, 80)),
        ("label", rust_widgets::create_label(window, "l", 0, 0, 200, 80)),
        ("radio_button", rust_widgets::create_radio_button(window, "r", 0, 0, 200, 80)),
        ("slider", rust_widgets::create_slider(window, 0, 0, 200, 80)),
        ("progress_bar", rust_widgets::create_progress_bar(window, 0, 0, 200, 80)),
        ("combo_box", rust_widgets::create_combo_box(window, 0, 0, 200, 80)),
        ("list_box", rust_widgets::create_list_box(window, 0, 0, 200, 80)),
        ("list_view", rust_widgets::create_list_view(window, 0, 0, 200, 80)),
        ("scroll_area", rust_widgets::create_scroll_area(window, 0, 0, 200, 80)),
        ("spin_box", rust_widgets::create_spin_box(window, 0, 0, 200, 80)),
        ("message_box", rust_widgets::create_message_box(window, "t", "x", 0, 0, 200, 80)),
        ("file_dialog", rust_widgets::create_file_dialog(window, 0, 0, 200, 80)),
        ("color_dialog", rust_widgets::create_color_dialog(window, 0, 0, 200, 80)),
        ("font_dialog", rust_widgets::create_font_dialog(window, 0, 0, 200, 80)),
        ("panel", rust_widgets::create_panel(window, 0, 0, 200, 80)),
    ]
}

/// `0` is the answer to an unusable parent, for the whole family — the dialogs included.
#[test]
fn every_control_refuses_the_no_parent_id() {
    let refused = create_family(0);
    let mounted: Vec<&str> =
        refused.iter().filter(|(_, id)| *id != 0).map(|(name, _)| *name).collect();
    assert!(
        mounted.is_empty(),
        "these entries created a control under the `0` parent id, which addresses no live \
         container: {mounted:?} — either the parent contract was dropped for them or they are \
         windows, and neither belongs in this list"
    );
    // The dialogs are called out because a single one of these returning `0` was once read as
    // "the desktop backends have no such control".
    for dialog in ["message_box", "file_dialog", "color_dialog", "font_dialog"] {
        assert!(
            refused.iter().any(|(name, _)| *name == dialog),
            "{dialog} left the family, so this test no longer covers the entry that was \
             misreported"
        );
    }
}

/// A real window parent is all any of them needs — the dialogs included.
#[test]
fn every_control_mounts_under_a_real_window_and_gets_a_distinct_id() {
    let window = rust_widgets::create_window("create-family-probe", 0, 0, 800, 600);
    assert_ne!(window, 0, "the probe needs a real window to parent into");

    let created = create_family(window);
    let failed: Vec<&str> =
        created.iter().filter(|(_, id)| *id == 0).map(|(name, _)| *name).collect();
    assert!(
        failed.is_empty(),
        "these entries produced no control under a live window parent: {failed:?} — on this \
         profile every control is painted by the library, so a `0` here means the constructor \
         registry has a hole"
    );

    let mut ids: Vec<ObjectId> = created.iter().map(|(_, id)| *id).collect();
    ids.sort_unstable();
    let distinct = {
        ids.dedup();
        ids.len()
    };
    assert_eq!(
        distinct,
        created.len(),
        "two entries were handed the same id, so one of them is not its own control"
    );
}
