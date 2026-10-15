// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stages `Platform::choose_path` — "let the user pick a file" — and answers it without a human.
//!
//! # What this answers
//!
//! The designer feedback reported that a host could neither open the operating system's picker nor
//! get a browsable one from the library alone (`FileDialog` is painted by the library and *filled*
//! by the host, so it lists nothing by itself). This probe exercises the added route on a live
//! session and pins the three answers a host branches on:
//!
//! * **A** — the capability is queryable **before** the call, and it agrees with what the call
//!   does: a backend that reports `file_dialog: false` answers `Err(Unsupported)` rather than
//!   something a host would read as "cancelled";
//! * **B** — on a backend that reports `true`, the picker really opens and the modal loop really
//!   returns a path-or-cancel answer. The dialog is *dismissed programmatically* (a timeout inside
//!   its nested loop calls `response(Cancel)`) so the check needs no human at the keyboard: the
//!   observable fact is `Ok(None)`, which can only come from a dialog that was up and answered;
//! * **C** — a thread the toolkit forbids a modal window on gets `Err(Unavailable)`, not a crash
//!   and not a silent `Ok(None)`.
//!
//! Run with:
//!     cargo run --no-default-features --features desktop --example probe_file_picker
//!     cargo run --no-default-features --features "desktop,gtk-native" --example probe_file_picker
//!
//! Exit codes: `0` = every check held, `1` = a check failed (the line says which), `2` = this host
//! cannot stage it (no display session), which is a skip rather than a failure.

#[cfg(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn main() {
    staged()
}

#[cfg(not(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
)))]
fn main() {
    println!(
        "probe_file_picker: this profile has no platform picker (alloc-frugal or a non-desktop \
         target), so there is nothing to stage."
    );
}

#[cfg(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn staged() {
    use rust_widgets::platform::{PathPickerError, PathPurpose};

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    println!("=== file picker probe ===");
    println!("backend            : {}", rust_widgets::platform::backend_name());
    println!("runtime gui mode   : {:?}", rust_widgets::platform::runtime_gui_mode());
    rust_widgets::init();
    // Asked *after* `init`, which is the order a host uses (it initialises, then reads the
    // capabilities it will plan its UI around). The flag itself is a build fact on the GTK
    // backend: availability per call is what the method reports.
    let capability = rust_widgets::platform::capabilities().file_dialog;
    println!("file_dialog flag   : {capability}");

    // ── A: the flag and the call agree ───────────────────────────────────────────────────────
    println!("\nA. capability flag versus what the call answers");
    if capability {
        println!("   the flag is true, so the staged run below is B/C");
    } else {
        let answer = rust_widgets::choose_path(PathPurpose::OpenFile, "pick a file");
        println!("   choose_path answered {answer:?}");
        check(
            matches!(answer, Err(PathPickerError::Unsupported)),
            "a backend that reports `file_dialog: false` answers `Unsupported`, so a host can pick \
             its fallback from the flag instead of provoking this failure",
        );
        // The other two purposes must agree with the flag as well: three purposes, one capability.
        for purpose in [PathPurpose::SaveFile, PathPurpose::SelectDirectory] {
            let answer = rust_widgets::choose_path(purpose, "pick a path");
            check(
                matches!(answer, Err(PathPickerError::Unsupported)),
                "every purpose answers `Unsupported` on a backend with no picker",
            );
        }
    }

    // ── B: a live dialog, dismissed programmatically ─────────────────────────────────────────
    if capability {
        println!(
            "\nB. the picker opens and answers (dismissed by a timer inside its own modal loop)"
        );
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // The timer has to be armed *before* the call: `choose_path` runs the dialog's nested
            // main loop, so a timeout registered now fires inside it and can answer the dialog.
            // This is also what makes the check need no human — see this probe's docs.
            // `gtk::prelude::*` brings in the traits the calls below need (`Cast::downcast`,
            // `GtkWindowExt::list_toplevels`), which is why it is imported where they are used.
            use gtk::prelude::*;
            use std::time::{Duration, Instant};

            const DISMISS_AFTER_MS: u64 = 400;
            let armed = std::rc::Rc::new(std::cell::Cell::new(false));
            let armed_for_timer = armed.clone();
            gtk::glib::timeout_add_local(
                std::time::Duration::from_millis(DISMISS_AFTER_MS),
                move || {
                    // Find the modal dialog among the open toplevels and answer Cancel. Anything
                    // that is not a `Dialog` is another window and is left alone.
                    let mut dismissed = false;
                    for widget in gtk::Window::list_toplevels() {
                        if let Ok(dialog) = widget.downcast::<gtk::Dialog>() {
                            if dialog.is_modal() {
                                dialog.response(gtk::ResponseType::Cancel);
                                dismissed = true;
                            }
                        }
                    }
                    armed_for_timer.set(dismissed);
                    gtk::glib::ControlFlow::Break
                },
            );

            let started = Instant::now();
            let answer = rust_widgets::choose_path(PathPurpose::OpenFile, "probe: pick a file");
            let took = started.elapsed();
            println!("   choose_path answered {answer:?} after {took:?}");
            if matches!(answer, Err(PathPickerError::Unavailable)) {
                // A `gtk-native` build in a shell with no display session: the flag says the
                // backend has a chooser, and the call says *this session* cannot show it. That is
                // a host limitation, not a defect (principle #103: report it, do not fail).
                println!(
                    "   unsupported host (no display session for GTK), so the dialog cannot be \
                     staged here"
                );
                std::process::exit(2);
            }
            check(
                matches!(answer, Ok(None)),
                "the picker opened, its modal loop ran, and the dismissal came back as `Ok(None)` \
                 (a cancellation is an answer, not an error)",
            );
            check(
                took >= Duration::from_millis(DISMISS_AFTER_MS / 2),
                "the call really waited on the dialog instead of returning immediately",
            );
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {
            println!(
                "   skipped: this build has no toolkit to stage a real dialog with \
                 (`--features desktop,gtk-native` on a host with a display session stages it)"
            );
        }

        // ── C: a thread the toolkit refuses ─────────────────────────────────────────────────
        println!("\nC. asking from a thread the toolkit forbids a modal window on");
        let from_worker = std::thread::spawn(|| {
            rust_widgets::choose_path(PathPurpose::OpenFile, "probe: from a worker thread")
        })
        .join()
        .expect("the worker thread must not panic");
        println!("   worker thread answered {from_worker:?}");
        // Two honest outcomes: the toolkit has no thread rule (the call succeeds or the user would
        // have to answer it), or it refuses. What is *not* allowed is a crash or a silent `Ok(None)`
        // that a host would read as "the user cancelled".
        check(
            !matches!(from_worker, Ok(None)),
            "a refused cross-thread request is an error, not a cancellation a host would misread",
        );
    }

    println!(
        "\n{}",
        if failures == 0 {
            "probe_file_picker: OK (every check held)".to_string()
        } else {
            format!("probe_file_picker: {failures} check(s) failed")
        }
    );
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
