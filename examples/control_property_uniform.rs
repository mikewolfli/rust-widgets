//! Runtime proof that the unified native-control property API needs **one** code
//! path on every OS.
//!
//! Nothing in this file branches on `target_os`, imports a platform crate, or
//! calls a `cfg`-gated helper. Every property is set and read through the same
//! `Platform` methods, and each backend maps them onto whatever its own native
//! control exposes — reporting `false`/`None` when it has nothing to map to.
//!
//! Run with `cargo run --example control_property_uniform` (desktop).
//!
//! Expected output on macOS (real AppKit controls on the main thread):
//!
//! ```text
//! backend = cocoa
//! slider  set value 42 -> true
//! slider  value       -> Some(42.0)
//! slider  range       -> Some((0.0, 200.0))
//! spinbox set value 7 -> true
//! combo   select idx 1 -> true
//! combo   index       -> Some(1)
//! checkbox set true   -> true
//! checkbox checked    -> Some(true)
//! button  set value   -> false (the control has no numeric value here)
//! handle  checked     -> Some(true)
//! ```
//!
//! The last property line is the point: a button has no numeric value, so the
//! backend says so instead of silently accepting the write. That is a legitimate
//! per-OS capability difference — the *call* is still identical everywhere.

use rust_widgets::app::{App, CheckBoxHandle, WidgetHandle};
use rust_widgets::core::ObjectId;
use rust_widgets::platform::{get_platform, EchoMode};
use rust_widgets::WindowStateFlag;

fn main() {
    App::new().init();

    let platform = get_platform();
    println!("backend = {}", platform.backend_name());

    // A window is needed so the controls have a real parent on every backend.
    let parent: ObjectId = platform.create_window("Control property probe", 40, 40, 640, 400);
    if parent == 0 {
        eprintln!("this backend creates no native window; nothing to probe");
        return;
    }

    // ── Slider: value + range ────────────────────────────────────────────
    let slider = platform.create_slider(parent, 20, 20, 240, 24);
    let _ = platform.set_widget_range(slider, 0.0, 200.0);
    let wrote = platform.set_widget_value(slider, 42.0);
    println!("slider  set value 42 -> {wrote}");
    println!("slider  value       -> {:?}", platform.widget_value(slider));
    println!("slider  range       -> {:?}", platform.widget_range(slider));

    // ── Spin box: value ──────────────────────────────────────────────────
    let spin = platform.create_spin_box(parent, 20, 60, 160, 24);
    let wrote = platform.set_widget_value(spin, 7.0);
    println!("spinbox set value 7 -> {wrote}");

    // ── Combo box: selection index ───────────────────────────────────────
    let combo = platform.create_combo_box(parent, 20, 100, 200, 24);
    let _ = platform.combo_box_add_item(combo, "First");
    let _ = platform.combo_box_add_item(combo, "Second");
    let wrote = platform.set_widget_selected_index(combo, Some(1));
    println!("combo   select idx 1 -> {wrote}");
    println!("combo   index       -> {:?}", platform.widget_selected_index(combo));

    // ── Check box: checked state ─────────────────────────────────────────
    let check = platform.create_checkbox(parent, "Enable", 20, 140, 160, 24);
    let wrote = platform.set_widget_checked(check, true);
    println!("checkbox set true   -> {wrote}");
    println!("checkbox checked    -> {:?}", platform.is_widget_checked(check));

    // ── A control without the property reports absence, not a fake value ──
    let button = platform.create_button(parent, "OK", 20, 180, 120, 28);
    let wrote = platform.set_widget_value(button, 1.0);
    println!("button  set value   -> {wrote} (the control has no numeric value here)");

    // ── Step, indeterminate and read-only follow the same shape ──────────
    let step_ok = platform.set_widget_step(slider, 5.0);
    println!("slider  set step 5  -> {step_ok}");
    println!("slider  step        -> {:?}", platform.widget_step(slider));

    let progress = platform.create_progress_bar(parent, 20, 220, 240, 24);
    let busy = platform.set_widget_indeterminate(progress, true);
    println!("progress busy       -> {busy}");
    println!("progress is busy    -> {:?}", platform.is_widget_indeterminate(progress));

    let entry = platform.create_line_edit(parent, "editable", 20, 260, 200, 24);
    let ro = platform.set_widget_read_only(entry, true);
    println!("lineedit read only  -> {ro}");
    println!("lineedit is ro      -> {:?}", platform.is_widget_read_only(entry));

    // ── Selection / placeholder / echo mode (per-OS capability, same call) ──
    let sel = platform.set_widget_selection(entry, 0, 4);
    println!("lineedit select 0..4-> {sel}/{:?}", platform.widget_selection(entry));

    let ph = platform.set_widget_placeholder(entry, "Type here");
    println!(
        "lineedit placeholder-> {ph}/{:?} (macOS NSTextView has none)",
        platform.widget_placeholder(entry)
    );

    let echo = platform.set_widget_echo_mode(entry, EchoMode::Password);
    println!(
        "lineedit password   -> {echo}/{:?} (macOS picks the class)",
        platform.widget_echo_mode(entry)
    );

    let no_echo = platform.set_widget_echo_mode(entry, EchoMode::NoEcho);
    println!("lineedit NoEcho     -> {no_echo} (no toolkit has this; must be false)");

    // ── Window state: the same call shape as every other property ────────
    for flag in [
        WindowStateFlag::Maximized,
        WindowStateFlag::Minimized,
        WindowStateFlag::Fullscreen,
        WindowStateFlag::Resizable,
        WindowStateFlag::Decorated,
    ] {
        let set_on = platform.set_window_state(parent, flag, true);
        let read_on = platform.is_window_in_state(parent, flag);
        let set_off = platform.set_window_state(parent, flag, false);
        let read_off = platform.is_window_in_state(parent, flag);
        println!(
            "window  {flag:<10} on={set_on}/{read_on:?} off={set_off}/{read_off:?}",
            flag = format!("{flag:?}")
        );
    }

    // ── A control must refuse window state, not silently record it ───────
    let refused = platform.set_window_state(button, WindowStateFlag::Maximized, true);
    println!("button  set maximized -> {refused} (a button is not a window)");

    // ── Window minimum size and icon ─────────────────────────────────────
    println!("window  min size (unset) -> {:?}", platform.window_min_size(parent));
    let set_min = platform.set_window_min_size(parent, 320, 240);
    println!("window  set min 320x240  -> {set_min}/{:?}", platform.window_min_size(parent));

    // A path that cannot be decoded must fail rather than be recorded as success.
    let bad_icon = platform.set_window_icon(parent, "/definitely/not/here.png");
    println!("window  bad icon        -> {bad_icon} (must be false)");

    // Same call shape through the type-safe handle layer — still no `cfg`.
    let boxed = CheckBoxHandle::from_raw(check);
    println!("handle  checked     -> {:?}", boxed.checked());
}
