//! Runtime probe for typed menu shortcuts, run on the host's UI main thread.
//!
//! The library's own test harness runs off the main thread, where the macOS
//! backend deliberately takes its state-only path and no native `NSMenuItem`
//! exists. This binary runs the same assertions on the main thread so the native
//! accelerator is genuinely created, read back from the host toolkit, and
//! exercised.
//!
//! Everything platform-specific is asked of the backend at runtime: the notation
//! comes from `PlatformShortcutStyle::current()`, and activating an item goes
//! through [`rust_widgets::platform::Platform::activate_menu_item`]. This file
//! therefore contains no `cfg(target_os)` branch and no toolkit import — it is
//! ordinary upper-layer calling code, exactly like an application would be.
//!
//! Run with: cargo run --example menu_shortcut_runtime

use rust_widgets::platform::get_platform;
use rust_widgets::shortcut::{Key, PlatformShortcutStyle, Shortcut};
use std::time::Duration;

fn main() {
    use rust_widgets::app::{App, WidgetHandle};

    let mut app = App::new();
    app.init();

    let mut failures: Vec<String> = Vec::new();

    // 1. One declaration must render in the host's own notation, whatever that is.
    let rendered = rust_widgets::format_shortcut(&Shortcut::primary(Key::Z));
    let style = PlatformShortcutStyle::current();
    println!("[1] format_shortcut(Primary+Z) = {rendered:?} (style {style:?})");
    let expected_undo = style.format(&Shortcut::primary(Key::Z));
    if rendered != expected_undo {
        failures.push(format!("expected {expected_undo:?}, got {rendered:?}"));
    }

    let win = app.new_window("Menu Shortcut Runtime", 0, 0, 900, 600);
    let bar = win.new_menu_bar(0, 0, 0, 0);
    let edit = win.new_menu(&bar, "Edit", 0, 0, 0, 0);

    let undo = win.new_menu_item_with_shortcut(&edit, "Undo", Some(Shortcut::primary(Key::Z)));
    let redo =
        win.new_menu_item_with_shortcut(&edit, "Redo", Some(Shortcut::primary_shift(Key::Z)));
    let plain = win.new_menu_item_with_shortcut(&edit, "No Chord", None);

    // 2. Attach, so the items are reachable.
    let attached = win.attach_menu_bar(&bar);
    println!("[2] menu bar attached = {attached}");
    if !attached {
        failures.push("menu bar failed to attach".to_string());
    }
    for (name, id) in [("undo", undo.raw_id()), ("redo", redo.raw_id()), ("plain", plain.raw_id())]
    {
        if id == 0 {
            failures.push(format!("{name} item was not created"));
        }
    }

    // 3. Each item must report the accelerator that was registered for it.
    let undo_chord = rust_widgets::menu_item_shortcut(undo.raw_id());
    let redo_chord = rust_widgets::menu_item_shortcut(redo.raw_id());
    let plain_chord = rust_widgets::menu_item_shortcut(plain.raw_id());
    println!("[3] undo={undo_chord:?} redo={redo_chord:?} plain={plain_chord:?}");
    let expected_redo = style.format(&Shortcut::primary_shift(Key::Z));
    if undo_chord.as_deref() != Some(expected_undo.as_str()) {
        failures.push(format!("undo should report {expected_undo:?}, got {undo_chord:?}"));
    }
    if redo_chord.as_deref() != Some(expected_redo.as_str()) {
        failures.push(format!("redo should report {expected_redo:?}, got {redo_chord:?}"));
    }
    if plain_chord.is_some() {
        failures.push(format!("plain item should report no chord, got {plain_chord:?}"));
    }

    // 4. Activate the Undo item through the host's own menu dispatch. The backend
    //    reports whether it has such a route; a backend without a native menu says
    //    so honestly instead of the probe branching on the OS itself.
    let platform = get_platform();
    if !platform.activate_menu_item(undo.raw_id()) {
        failures.push(format!(
            "the {} backend has no native menu dispatch for this item",
            platform.backend_name()
        ));
    }
    match wait_for_trigger(Duration::from_secs(2)) {
        Some(id) if id == undo.raw_id() => println!("[4] poll_menu_triggered -> {id} (Undo)"),
        Some(id) => failures.push(format!("fired item {id}, expected {}", undo.raw_id())),
        None => failures.push("activation never reached the trigger queue".to_string()),
    }

    println!();
    if failures.is_empty() {
        println!("ALL CHECKS PASSED");
        std::process::exit(0);
    }
    for failure in &failures {
        eprintln!("FAIL: {failure}");
    }
    std::process::exit(1);
}

/// Polls the menu trigger queue until an item arrives or `budget` elapses.
fn wait_for_trigger(budget: Duration) -> Option<u64> {
    use std::time::Instant;
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if let Some(id) = rust_widgets::poll_menu_triggered() {
            return Some(id);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}
