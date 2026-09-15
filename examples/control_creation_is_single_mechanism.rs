// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Runtime proof that the host creates **no** controls and one mechanism exists.
//!
//! # What this replaced
//!
//! This example used to be `macos_created_controls_are_usable`, which asserted that
//! every macOS `create_*` registered a handle so the property API could find it. It
//! caught a real bug at the time: 20 constructors returned a non-zero id without
//! registering anything, so every property call silently reported "unknown id".
//!
//! That whole class of bug is gone with the native control path. The host now
//! supplies a window and a drawing surface, and the library paints every
//! `WidgetKind` (BLUE15 #55/#56), so a `create_*` that returned a live id would
//! announce a capability the backend does not have. The invariant worth checking
//! inverted: **controls must report absence, and only the window may be created.**
//!
//! # What it checks
//!
//! 1. Every per-kind `create_*` returns `0`, even under a valid window. This is the
//!    honest-default contract; a non-zero id here means a control path crept back in.
//! 2. The window *is* created and is findable by the property API — the capability
//!    that genuinely remains, and the one the original probe's handle check guarded.
//!
//! Run with `cargo run --example control_creation_is_single_mechanism`.
//! Exit code 0 means every assertion held.

use rust_widgets::app::App;
use rust_widgets::core::ObjectId;
use rust_widgets::platform::get_platform;

fn main() {
    App::new().init();
    let platform = get_platform();
    println!("backend = {}", platform.backend_name());

    let window = platform.create_window("Creation-mechanism probe", 40, 40, 900, 700);
    if window == 0 {
        eprintln!("FAIL: no window was created, so there is no host at all");
        std::process::exit(1);
    }
    println!("window               -> id={window} (the host's one primitive)");

    // A window the *library* creates must be addressable through the library's own
    // accessors — that is the capability the original probe guarded (`every macOS
    // create_* registered a handle so the property API could find it`), and it still
    // has to hold. It is asked through `lib.rs` rather than through the `Platform`
    // trait now: the host no longer owns controls, so the trait's control members
    // are honest defaults that report absence, and asserting a round-trip through
    // them would be asserting the architecture this refactor removed.
    let library_window = rust_widgets::create_window("Library window", 40, 40, 400, 300);
    if library_window == 0 {
        eprintln!("FAIL: the library could not create a window widget");
        std::process::exit(1);
    }
    let probe_text = "Singleton Window";
    rust_widgets::set_widget_text(library_window, probe_text);
    let window_readback = rust_widgets::get_widget_text(library_window);
    if window_readback != probe_text {
        eprintln!("FAIL: window text did not round-trip (got {window_readback:?})");
        std::process::exit(1);
    }
    println!("window text readback -> OK");

    // Every control member must refuse. A non-zero id here means the backend is
    // claiming a control it cannot paint.
    let controls: Vec<(&str, ObjectId)> = vec![
        ("button", platform.create_button(window, "b", 10, 10, 120, 30)),
        ("label", platform.create_label(window, "l", 10, 50, 120, 30)),
        ("checkbox", platform.create_checkbox(window, "c", 10, 90, 120, 30)),
        ("line_edit", platform.create_line_edit(window, "e", 10, 130, 120, 30)),
        ("radio_button", platform.create_radio_button(window, "r", 10, 170, 120, 30)),
        ("slider", platform.create_slider(window, 10, 210, 120, 30)),
        ("progress_bar", platform.create_progress_bar(window, 10, 250, 120, 30)),
        ("combo_box", platform.create_combo_box(window, 10, 290, 120, 30)),
        ("list_box", platform.create_list_box(window, 10, 330, 120, 60)),
        ("panel", platform.create_panel(window, 10, 400, 120, 60)),
        ("group_box", platform.create_group_box(window, "grp", 200, 10, 120, 60)),
        ("frame", platform.create_frame(window, 200, 80, 120, 60)),
        ("tab_widget", platform.create_tab_widget(window, 200, 150, 120, 60)),
        ("splitter", platform.create_splitter(window, 200, 220, 120, 60)),
        ("toggle_button", platform.create_toggle_button(window, "t", 200, 290, 120, 30)),
        ("spin_box", platform.create_spin_box(window, 200, 330, 120, 30)),
        ("scroll_area", platform.create_scroll_area(window, 200, 370, 120, 60)),
        ("list_view", platform.create_list_view(window, 400, 10, 120, 60)),
        ("date_picker", platform.create_date_picker(window, 400, 80, 120, 30)),
        ("time_picker", platform.create_time_picker(window, 400, 120, 120, 30)),
        ("activity_indicator", platform.create_activity_indicator(window, 400, 160, 120, 30)),
        ("message_box", platform.create_message_box(window, "m", "t", 400, 200, 120, 60)),
    ];

    let mut created_controls = Vec::new();
    for (label, id) in &controls {
        if *id != 0 {
            created_controls.push(*label);
            println!("{label:20} -> id={id} CLAIMED-A-CONTROL");
        }
    }

    if !created_controls.is_empty() {
        eprintln!(
            "FAIL: {} member(s) returned a live id: {created_controls:?}. The host must \
             create no control; a non-zero id means a second creation mechanism exists.",
            created_controls.len()
        );
        std::process::exit(1);
    }

    println!("{} control member(s) correctly reported absence", controls.len());
    println!("RESULT: PASS");
}
