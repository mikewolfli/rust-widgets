// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stages display enumeration, window ownership and the change revision on a live session.
//!
//! # What this answers
//!
//! The designer feedback (`docs/issues/feedback-20261015.md` §十二) reported that a host could not
//! enumerate displays, could not ask which display a window was on, and had no way to learn that the
//! display set changed — so "keep the window visible after a monitor is unplugged" could not be
//! checked against a real screen. This probe pins the three readings a host reconciles windows
//! against, on the session it runs in:
//!
//! * **A** — `displays()` returns at least one display on a real desktop session, each with a scale
//!   of at least `1.0` and a work area contained in its bounds;
//! * **B** — `display_of_window` places a window the library created on a display that is in the
//!   list, and answers `None` for an id that names no window;
//! * **C** — `display_generation()` is non-zero, so a host has a value to compare across frames.
//!
//! # Which backends stage it
//!
//! Linux/GTK (GDK's monitor list) and macOS/cocoa (`NSScreen`; see `src/platform/macos/displays.rs`).
//! The Windows backend implements the same three methods (`src/platform/windows/displays.rs`) but has
//! no probe run in this crate's gate suite; every other backend answers the honest default (empty),
//! which this probe reports as "not staged here" rather than as a failure.
//!
//! Run with:
//!     cargo run --no-default-features --features "desktop" --example probe_displays
//!     cargo run --no-default-features --features "desktop,gtk-native" --example probe_displays
//!
//! Exit codes: `0` = every check held, `1` = a check failed (the line says which), `2` = this host
//! cannot stage it (no display session, or the operator did not allow opening a window).

/// Whether the operator has allowed this probe to open a real window. Same contract as the sibling
/// probes: opening windows on someone's live desktop must be a deliberate act, not a side effect of
/// running an example (`probe_file_picker` documents the measured cost of learning this the hard
/// way).
#[cfg(any(
    all(target_os = "linux", feature = "gtk-native"),
    all(target_os = "macos", feature = "cocoa-legacy")
))]
fn gui_probe_allowed() -> bool {
    if std::env::var("RW_GUI_PROBE_OK").ok().as_deref() == Some("1") {
        return true;
    }
    println!(
        "this probe opens a real window on the desktop it runs on; set RW_GUI_PROBE_OK=1 to stage \
         it (and prefer a machine whose screen nobody is using)"
    );
    false
}

#[cfg(not(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
)))]
fn main() {
    println!(
        "probe_displays: this profile has no platform backend (alloc-frugal or a non-desktop \
         target), so there is nothing to stage."
    );
}

#[cfg(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn main() {
    #[cfg(any(
        all(target_os = "linux", feature = "gtk-native"),
        all(target_os = "macos", feature = "cocoa-legacy")
    ))]
    {
        run_probe();
    }

    #[cfg(not(any(
        all(target_os = "linux", feature = "gtk-native"),
        all(target_os = "macos", feature = "cocoa-legacy")
    )))]
    {
        println!(
            "probe_displays: this backend does not enumerate displays here (`--features \
             desktop,gtk-native` on a host with a display session, or `desktop` on macOS, stages \
             it); displays() is the honest empty list, so there is nothing to stage."
        );
    }
}

#[cfg(all(
    not(alloc_frugal),
    any(
        all(target_os = "linux", feature = "gtk-native"),
        all(target_os = "macos", feature = "cocoa-legacy")
    )
))]
fn run_probe() {
    if !gui_probe_allowed() {
        std::process::exit(2);
    }

    const TITLE: &str = "rust-widgets display probe";
    const SIZE: (u32, u32) = (320, 240);

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    println!("=== display probe ===");
    println!("backend            : {}", rust_widgets::platform::backend_name());
    println!("runtime gui mode   : {:?}", rust_widgets::platform::runtime_gui_mode());
    #[cfg(target_os = "linux")]
    println!("DISPLAY            : {}", std::env::var("DISPLAY").unwrap_or_default());

    rust_widgets::init();
    let displays = rust_widgets::platform::displays();
    println!("\nA. displays()");
    println!("   count: {}", displays.len());
    for display in &displays {
        println!(
            "   id={} bounds=({},{}) {}x{} work=({},{}) {}x{} scale={} primary={}",
            display.id.get(),
            display.bounds.x,
            display.bounds.y,
            display.bounds.width,
            display.bounds.height,
            display.work_area.x,
            display.work_area.y,
            display.work_area.width,
            display.work_area.height,
            display.scale,
            display.is_primary
        );
    }
    check(!displays.is_empty(), "a real session reports at least one display");
    check(displays.iter().all(|d| d.scale >= 1.0), "every display reports a scale of at least 1.0");
    check(
        displays.iter().all(|d| {
            d.work_area.x >= d.bounds.x
                && d.work_area.y >= d.bounds.y
                && d.work_area.width <= d.bounds.width
                && d.work_area.height <= d.bounds.height
        }),
        "every work area is contained in its display's bounds",
    );

    println!("\nB. display_of_window");
    let window = rust_widgets::create_window(TITLE, 64, 48, SIZE.0, SIZE.1);
    check(window != 0, "the library created a window");
    rust_widgets::show_widget(window);
    // Let the window manager place the toplevel, so its position is the mapped one.
    std::thread::sleep(std::time::Duration::from_millis(300));

    let owner = rust_widgets::platform::display_of_window(window);
    println!("   window {window} is on display {owner:?}");
    check(
        owner.is_some_and(|id| displays.iter().any(|d| d.id == id)),
        "the window's display is one of the enumerated displays",
    );
    check(
        rust_widgets::platform::display_of_window(0).is_none(),
        "an id that names no window answers None rather than a display",
    );

    println!("\nC. display_generation()");
    let generation = rust_widgets::platform::display_generation();
    println!("   generation: {generation}");
    check(generation != 0, "the change revision is a real value, not the `0` sentinel");

    rust_widgets::hide_widget(window);

    println!("\n=== {} check(s) failed ===", failures);
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
