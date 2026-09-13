// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Verifies that every macOS `create_*` returns a *usable* widget id.
//!
//! Before this probe existed, 20 macOS `create_*` functions recorded a widget in
//! the state model but never registered a handle, so every property call on those
//! controls reported "unknown id" — `get_widget_text` returned `""`,
//! `is_widget_visible` returned `false`, and the uniform property API silently
//! refused. The bug was invisible because the constructors returned a non-zero id.
//!
//! This probe asserts the observable contract instead of the return value: for
//! every kind, the created id must be discoverable by the property API (text set
//! via `set_widget_text` must read back, and the widget must report visible).
//!
//! Run with `cargo run --example macos_created_controls_are_usable`.

use rust_widgets::app::App;
use rust_widgets::core::ObjectId;
use rust_widgets::platform::get_platform;

fn main() {
    App::new().init();
    let platform = get_platform();
    println!("backend = {}", platform.backend_name());

    let window = platform.create_window("Created-controls probe", 40, 40, 900, 700);
    if window == 0 {
        eprintln!("no window");
        return;
    }

    // (label, id) for every constructor that previously skipped handle
    // registration.
    let created: Vec<(&str, ObjectId)> = vec![
        ("list_view", platform.create_list_view(window, 10, 10, 120, 60)),
        ("group_box", platform.create_group_box(window, "grp", 10, 80, 120, 60)),
        ("frame", platform.create_frame(window, 10, 150, 120, 60)),
        ("tab_widget", platform.create_tab_widget(window, 10, 220, 120, 60)),
        ("splitter", platform.create_splitter(window, 10, 290, 120, 60)),
        ("toggle_button", platform.create_toggle_button(window, "tog", 10, 360, 120, 30)),
        ("calendar", platform.create_calendar(window, 10, 400, 120, 60)),
        ("scroll_bar", platform.create_scroll_bar(window, 10, 470, 120, 30)),
        ("double_spin_box", platform.create_double_spin_box(window, 10, 510, 120, 30)),
        ("font_combo_box", platform.create_font_combo_box(window, 10, 550, 120, 30)),
        ("context_menu", platform.create_context_menu(window, 10, 590, 120, 30)),
        ("popup_window", platform.create_popup_window(window, "pop", 10, 630, 120, 30)),
        ("dialog", platform.create_dialog(window, "dlg", 200, 10, 120, 60)),
        ("input_dialog", platform.create_input_dialog(window, 200, 80, 120, 60)),
        ("progress_dialog", platform.create_progress_dialog(window, 200, 150, 120, 60)),
        ("directory_dialog", platform.create_directory_dialog(window, "dir", 200, 220, 120, 60)),
        ("date_picker", platform.create_date_picker(window, 200, 290, 120, 30)),
        ("time_picker", platform.create_time_picker(window, 200, 330, 120, 30)),
        ("date_time_picker", platform.create_date_time_picker(window, 200, 370, 120, 30)),
        ("activity_indicator", platform.create_activity_indicator(window, 200, 410, 120, 30)),
    ];

    let mut unusable = Vec::new();
    for (label, id) in &created {
        if *id == 0 {
            println!("{label:20} -> 0 (constructor refused)");
            unusable.push(*label);
            continue;
        }

        // The observable contract: a registered handle means the property API can
        // find the widget. `set_widget_text` returning a readable value is the
        // proof, because the state fallback is only reachable *through* the
        // handle lookup.
        let probe_text = format!("probe-{label}");
        platform.set_widget_text(*id, &probe_text);
        let read_back = platform.get_widget_text(*id);
        let visible = platform.is_widget_visible(*id);

        let ok = read_back == probe_text && visible;
        println!(
            "{label:20} -> id={id:<4} text_readback={:<22} visible={visible} {}",
            format!("\"{read_back}\""),
            if ok { "OK" } else { "UNUSABLE" }
        );
        if !ok {
            unusable.push(*label);
        }
    }

    println!();
    if unusable.is_empty() {
        println!("all {} created controls are usable", created.len());
    } else {
        println!("UNUSABLE ({}): {:?}", unusable.len(), unusable);
    }
}
