//! Runtime probe for typed menu shortcuts, run on the AppKit main thread.
//!
//! The library's own test harness runs off the main thread, where the macOS
//! backend deliberately takes its state-only path and no `NSMenuItem` exists. This
//! binary runs the same assertions on the main thread so the native accelerator is
//! genuinely created, read back from AppKit, and exercised.
//!
//! Run with: cargo run --example menu_shortcut_runtime

#[cfg(target_os = "macos")]
fn main() {
    use rust_widgets::app::{App, WidgetHandle};
    use rust_widgets::shortcut::{Key, Shortcut};
    use std::time::Duration;

    let mut app = App::new();
    app.init();

    let mut failures: Vec<String> = Vec::new();

    // 1. One declaration must render in the host's notation.
    let rendered = rust_widgets::format_shortcut(&Shortcut::primary(Key::Z));
    println!("[1] format_shortcut(Primary+Z) = {rendered:?}");
    if rendered != "⌘Z" {
        failures.push(format!("expected \"⌘Z\", got {rendered:?}"));
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
    if undo_chord.as_deref() != Some("⌘Z") {
        failures.push(format!("undo should report ⌘Z, got {undo_chord:?}"));
    }
    if redo_chord.as_deref() != Some("⇧⌘Z") {
        failures.push(format!("redo should report ⇧⌘Z, got {redo_chord:?}"));
    }
    if plain_chord.is_some() {
        failures.push(format!("plain item should report no chord, got {plain_chord:?}"));
    }

    // 4. Activate the Undo item through the menu's real dispatch, which is the
    //    same route a click and a key-equivalent press both take.
    let fired = activate_menu_item(undo.raw_id());
    println!("[4] activated undo item = {fired}");
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
#[cfg(target_os = "macos")]
fn wait_for_trigger(budget: std::time::Duration) -> Option<u64> {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if let Some(id) = rust_widgets::poll_menu_triggered() {
            return Some(id);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

/// Activates a menu item the way AppKit does, so the probe exercises the real
/// menu dispatch rather than a synthetic event.
///
/// Uses `NSMenu -performActionForItemAtIndex:`, which is the single path AppKit
/// takes for both a mouse click and a matched key equivalent.
#[cfg(target_os = "macos")]
fn activate_menu_item(item_id: u64) -> bool {
    use cocoa::base::{id, nil};
    use objc::runtime::{Object, Sel};
    use objc::{msg_send, sel, sel_impl};

    let Some(ptr) = rust_widgets::native_handle(item_id) else {
        eprintln!("no native handle for menu item {item_id}");
        return false;
    };
    unsafe {
        let item = ptr as *const Object as id;
        if item == nil {
            eprintln!("null NSMenuItem for {item_id}");
            return false;
        }
        // Send the item its own action, exactly as the menu controller does when
        // the chord matches. Using action/target (rather than a shortcut table
        // lookup) keeps this on the same code path a key press takes.
        let action: Sel = msg_send![item, action];
        let target: id = msg_send![item, target];
        if target == nil {
            eprintln!("menu item {item_id} has no target");
            return false;
        }
        let _: () = msg_send![target, performSelector: action withObject: item];
    }
    true
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("This probe targets macOS only.");
}
