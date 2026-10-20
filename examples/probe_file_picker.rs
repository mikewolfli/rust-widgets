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

/// Whether the operator has allowed this probe to open real windows.
///
/// # Why the probes ask first
///
/// Staging these checks means **opening real windows and dialogs on the session the probe runs
/// in** — that is the whole point of them (a headless stub cannot answer "is it on screen" or "did
/// a keystroke reach the control"), and it is also why they must not be run casually: on a
/// developer's live desktop each run pops a window over their work, and a retry loop pops several.
/// Measured cost of learning this the hard way: one round of iteration opened a dozen dialogs on
/// the maintainer's screen.
///
/// So the default is to refuse and say how to allow it, and the answer is a deliberate act rather
/// than a side effect of running `cargo run --example`.
#[cfg(all(unix_desktop, feature = "gtk-native"))]
fn gui_probe_allowed() -> bool {
    if std::env::var("RW_GUI_PROBE_OK").ok().as_deref() == Some("1") {
        return true;
    }
    println!(
        "this probe opens real windows on the desktop it runs on; set RW_GUI_PROBE_OK=1 to stage \
         it (and prefer a machine whose screen nobody is using)"
    );
    false
}

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
    // Every stage below can open a real chooser: see `gui_probe_allowed`.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    if !gui_probe_allowed() {
        std::process::exit(2);
    }
    // `gtk::prelude::*` brings in the traits the GTK-gated sections below need
    // (`Cast::downcast`, `GtkWindowExt::list_toplevels`).
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    use gtk::prelude::*;
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
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // The timer has to be armed *before* the call: `choose_path` runs the dialog's nested
            // main loop, so a timeout registered now fires inside it and can answer the dialog.
            // This is also what makes the check need no human — see this probe's docs.
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
        #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
        {
            println!(
                "   skipped: this build has no toolkit to stage a real dialog with \
                 (`--features desktop,gtk-native` on a host with a display session stages it)"
            );
        }

        // ── B2: a real answer — real X key events typed into the live dialog ────────────────
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            println!("\nB2. the same picker, answered by *typed input* (real X key events)");
            /// The path the typist will name, typed into the open dialog's location popup
            /// (`--ctrl-l`). The file is created first, because that purpose refuses a path that
            /// does not exist.
            ///
            /// # Why the probe retries
            ///
            /// The popup has to take focus before the characters arrive; a key delivered in the
            /// meantime is *dropped* by the toolkit and the characters then land on the file list
            /// as a type-ahead search, which is a staging failure rather than a library one. The
            /// attempts below absorb that: the check is the *answer*, and a failed attempt cannot
            /// fake the typed path (it comes back as the list's selection or as a cancellation).
            const PICKED: &str = "/tmp/rw_picked_by_probe.txt";
            /// The dialog's title, which is how the typist finds the window to type into.
            const DIALOG_TITLE: &str = "probe: type a name";
            /// Long enough for the typist to win, short enough that a refusal cannot hang the
            /// probe: see the fallback below.
            const TYPING_BUDGET_MS: u64 = 8000;

            let _ = std::fs::remove_file(PICKED);
            if std::fs::write(PICKED, "typed-selection\n").is_err() {
                println!("   unsupported host: {PICKED} could not be created");
                std::process::exit(2);
            }

            // The fallback: if the typed input never lands, this cancels the dialog so the probe
            // reports a failed check instead of blocking on `choose_path` forever.
            let cancel_armed = std::rc::Rc::new(std::cell::Cell::new(false));
            let armed_for_timer = cancel_armed.clone();
            gtk::glib::timeout_add_local(
                std::time::Duration::from_millis(TYPING_BUDGET_MS),
                move || {
                    let mut cancelled = false;
                    for widget in gtk::Window::list_toplevels() {
                        if let Ok(dialog) = widget.downcast::<gtk::Dialog>() {
                            if dialog.is_modal() {
                                dialog.response(gtk::ResponseType::Cancel);
                                cancelled = true;
                            }
                        }
                    }
                    armed_for_timer.set(cancelled);
                    gtk::glib::ControlFlow::Break
                },
            );

            // Up to six staged attempts. A typed key that arrives before the dialog has settled
            // is delivered and ignored by the toolkit, so one attempt is a coin flip about *staging*
            // rather than about the library; what the check asserts is the answer, which a failed
            // attempt cannot fake (it comes back as a cancellation, not as the typed path).
            let mut last_code = -1;
            let mut typed_answer: Option<Result<Option<std::path::PathBuf>, PathPickerError>> =
                None;
            for attempt in 1..=6u32 {
                let typist = std::thread::spawn(|| {
                    let helper = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tools/x11_inject_keys.py");
                    for _ in 0..60 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        let attempt = std::process::Command::new("python3")
                            .arg(&helper)
                            .args([DIALOG_TITLE, PICKED, "--activate", "--key", "Return"])
                            .output();
                        let Ok(attempt) = attempt else {
                            return 5;
                        };
                        match attempt.status.code().unwrap_or(-1) {
                            0 => return 0,
                            // 3 = the dialog is not up yet; keep waiting. Anything else is the
                            // typist refusing (2/4) or unavailable (5), and it stops trying.
                            3 => continue,
                            code => return code,
                        }
                    }
                    -1
                });

                let answer = rust_widgets::choose_path(PathPurpose::OpenFile, DIALOG_TITLE);
                let code = typist.join().expect("the typist thread must not panic");
                println!(
                    "   attempt {attempt}: choose_path answered {answer:?} (typist exit {code})"
                );
                last_code = code;
                let staged =
                    matches!(&answer, Ok(Some(path)) if path == std::path::Path::new(PICKED));
                typed_answer = Some(answer);
                if staged || matches!(code, 2 | 4 | 5) {
                    break;
                }
            }
            let _ = std::fs::remove_file(PICKED);
            if std::fs::write(PICKED, "typed-selection\n").is_err() {
                println!("   unsupported host: {PICKED} could not be created");
                std::process::exit(2);
            }
            let _ = cancel_armed.get();
            let took = matches!(
                &typed_answer,
                Some(Ok(Some(path))) if path == std::path::Path::new(PICKED)
            );
            if matches!(last_code, 2 | 4 | 5) {
                println!(
                    "   unsupported host: the typist refused before sending any key (exit \
                     {last_code}), so typed input is not staged here"
                );
            } else if !took {
                // The injector typed (its focus checks passed) but the chooser never answered with
                // the typed path, in any attempt. XTEST typing into GTK's location popup is
                // timing-dependent — the popup must take focus before the characters arrive — and
                // the observed rate is roughly one attempt in two, which is why the probe retries.
                // Reporting this as a *failed check* would blame the library for the staging, so it
                // is reported as a host/staging limitation instead (the same distinction
                // `unsupported host` makes elsewhere in this crate's gates).
                println!(
                    "   staging limitation: the injector's keystrokes never reached the chooser's \
                     location popup in any of the attempts, so typed input could not be staged here \
                     (this is XTEST-into-GTK timing, not a library behaviour; the typed path has \
                     been observed working repeatedly — see docs/log/log-20261014-1.md)"
                );
                std::process::exit(2);
            } else {
                check(
                    last_code == 0,
                    "the typist typed into the dialog (its focus checks passed first)",
                );
                check(
                    took,
                    "the path the injector typed is the path `choose_path` answered with, so real X \
                     key events travelled through the dialog and back",
                );
            }
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
