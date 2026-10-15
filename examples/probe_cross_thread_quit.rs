// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stages the host that is *not* the loop's driver: `run()` on one thread, a watcher thread that
//! ends the session.
//!
//! # What this answers
//!
//! A smoke test, a CI job or a scripted demo has to end its own session, and no toolkit accepts a
//! stop from a thread that does not own the loop. The feedback this probe was written against
//! reported the consequence on a real desktop session: `--exit-after-ms 500` still running after
//! 60 s, because `quit()` from the watcher thread was refused and the only remaining exit was
//! `std::process::exit`, which is indistinguishable from a killed process.
//!
//! The library's answer is that the request is *recorded* and the loop's own frame tick reads it
//! (see `rust_widgets::platform::stop`). This probe exercises that on a real backend — a live GTK
//! session when `gtk-native` is on, otherwise the state loop — and prints what each step observed
//! rather than asserting in the dark:
//!
//! * **A** — a request made before any loop has run is recorded, and says so honestly;
//! * **B** — with the loop running, a request from the watcher thread ends it, and `run()` returns
//!   normally (this is the fact a CI job needs: a clean return, not a process kill);
//! * **C** — the request stopped exactly one loop, so a second session in the same process is not
//!   stopped by the first one's request;
//! * **D** (GTK only) — the window that was open during the staged session is a mapped, composited
//!   toplevel, which is the "is it really on screen" question a headless host cannot answer.
//!
//! Run with:
//!     cargo run --no-default-features --features desktop --example probe_cross_thread_quit
//!     cargo run --no-default-features --features "desktop,gtk-native" --example probe_cross_thread_quit
//!
//! Exit codes: `0` = every check held, `1` = a check failed (the line says which), `2` = this host
//! cannot stage it (no display session for GTK), which is a skip and not a failure.

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
        "probe_cross_thread_quit: this profile has no platform event loop (alloc-frugal or a \
         non-desktop target), so a cross-thread stop cannot be staged here."
    );
}

#[cfg(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn staged() {
    use std::time::{Duration, Instant};

    /// How long the watcher waits before it decides the session has gone on long enough. Long
    /// enough that the loop has certainly reached a frame, short enough to keep the probe quick.
    const WATCHER_DELAY: Duration = Duration::from_millis(400);
    /// The budget for the loop to return after the request. A loop that ignores the request runs
    /// for the rest of this budget and then fails the check — it does not hang the probe.
    const RETURN_BUDGET: Duration = Duration::from_secs(20);

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    println!("=== cross-thread quit probe ===");
    println!("backend            : {}", rust_widgets::platform::backend_name());
    println!("runtime gui mode   : {:?}", rust_widgets::platform::runtime_gui_mode());
    println!("dpi scale factor   : {}", rust_widgets::platform::dpi_scale_factor());
    println!("loop has ticked    : {}", rust_widgets::platform::stop::loop_has_ticked());

    // ── A: the request is recorded even when nothing can read it yet ──────────────────────────
    println!("\nA. before any loop has run (fresh process)");
    let answered = rust_widgets::quit();
    println!(
        "   quit() answered {answered}, requested={}",
        rust_widgets::platform::stop::is_requested()
    );
    check(
        rust_widgets::platform::stop::is_requested(),
        "a request made with no loop running is still recorded, so a loop started later honours it",
    );
    // Cleared here so the staged session below starts from "nothing asked", which is what a host
    // that has not decided anything yet looks like.
    let _ = rust_widgets::platform::stop::take();

    // ── B: a running loop, ended by a watcher thread ──────────────────────────────────────────
    println!("\nB. loop running, request from another thread");
    rust_widgets::init();
    let window = rust_widgets::create_window("cross-thread quit probe", 40, 40, 480, 320);
    println!("   window id = {window}");
    check(window != 0, "a real window was created (the id a session is ended with)");
    rust_widgets::show_widget(window);

    let (answered_tx, answered_rx) = std::sync::mpsc::channel();
    let watcher = std::thread::spawn(move || {
        std::thread::sleep(WATCHER_DELAY);
        let asked = rust_widgets::quit();
        let _ = answered_tx.send(asked);
    });

    let run_started = Instant::now();
    rust_widgets::run();
    let run_took = run_started.elapsed();
    let watcher_answered = answered_rx
        .recv_timeout(RETURN_BUDGET)
        .expect("the watcher thread must answer within the budget");
    watcher.join().expect("the watcher thread must not panic");

    println!("   run() returned after {run_took:?}");
    println!("   watcher: quit() -> {watcher_answered}");
    check(
        run_took < RETURN_BUDGET,
        "run() really returned (the loop ended instead of being killed)",
    );
    check(
        watcher_answered,
        "the watcher thread was told the request will be honoured (answering `false` would tell a \
         CI job to kill its own process)",
    );
    check(
        rust_widgets::platform::stop::loop_has_ticked(),
        "the loop drove frames, so the request had a reader on the loop's own thread",
    );

    // ── C: one request stops one loop ─────────────────────────────────────────────────────────
    println!("\nC. the request was consumed by the loop it stopped");
    check(
        !rust_widgets::platform::stop::is_requested(),
        "no request is left over, so a second session in this process is not stopped by the first",
    );

    // ── D: the window that was open really was on screen (GTK session only) ───────────────────
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    {
        println!("\nD. composition of the window the session had open");
        gtk_composition_report();
    }
    #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
    {
        println!(
            "\nD. skipped: this build has no toolkit to ask about composition \
             (`--features desktop,gtk-native` on a host with a display session stages it)"
        );
    }

    // ── E: the same ending arranged by the loop itself — no watcher thread at all ─────────────
    println!("\nE. a session that ends itself: schedule_on_main(300ms, quit)");
    {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        use std::sync::Arc;

        /// Where the callback ran, so the caller can compare it with the thread it asked from.
        static RAN_ON: AtomicU64 = AtomicU64::new(0);
        const SCHEDULE_AFTER: Duration = Duration::from_millis(300);

        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        let asking_thread = format!("{:?}", std::thread::current().id());
        let scheduled = rust_widgets::schedule_on_main(SCHEDULE_AFTER, move || {
            RAN_ON.store(
                format!("{:?}", std::thread::current().id())
                    .trim_start_matches("ThreadId(")
                    .trim_end_matches(')')
                    .parse()
                    .unwrap_or(0),
                Ordering::SeqCst,
            );
            flag.store(true, Ordering::SeqCst);
            // The whole point of being on the loop's thread: `quit` performs the backend's own
            // stop from here, so nothing has to be dispatched across threads.
            let _ = rust_widgets::quit();
        });
        check(scheduled, "the running loop accepted the work (it reads the queue every frame)");

        let started = Instant::now();
        rust_widgets::run();
        let took = started.elapsed();
        println!("   run() returned after {took:?}; callback ran={}", ran.load(Ordering::SeqCst));
        check(
            ran.load(Ordering::SeqCst),
            "the callback ran (a queued item that never runs would leave the session unstoppable)",
        );
        check(took >= SCHEDULE_AFTER, "the callback did not run before its deadline");
        check(
            RAN_ON.load(Ordering::SeqCst)
                == asking_thread
                    .trim_start_matches("ThreadId(")
                    .trim_end_matches(')')
                    .parse()
                    .unwrap_or(1),
            "the callback ran on the loop's own thread, not on the thread that scheduled it",
        );
    }

    let _ = rust_widgets::close_widget(window);

    println!(
        "\n{}",
        if failures == 0 {
            "probe_cross_thread_quit: OK (every check held)".to_string()
        } else {
            format!("probe_cross_thread_quit: {failures} check(s) failed")
        }
    );
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

/// Reports whether the GTK toplevel the staged session created is a mapped, composited window.
///
/// # Why this is evidence and not decoration
///
/// "A window exists" and "a window is on screen" are different claims, and a headless host cannot
/// tell them apart: GDK creates the toplevel when the library asks, and maps it only once the
/// loop has run and the X server has composed it. `is_mapped` is GDK's answer to the second claim,
/// read here after `gtk::main()` returned — so the window reported below was up *while* the loop
/// was running, not merely constructed.
///
/// This is the GTK half of the designer-feedback item that no display session could answer
/// ("窗口承载、合成与真实 DPI 切换").
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
fn gtk_composition_report() {
    use gtk::prelude::*;

    let toplevels = gtk::Window::list_toplevels();
    println!("   GTK toplevels still open: {}", toplevels.len());
    for (index, widget) in toplevels.iter().enumerate() {
        // `list_toplevels` hands back `Widget`s (the list is GTK's, which knows only widgets), so
        // each is narrowed to the toplevel it must be: anything else in this list would not be a
        // window and has nothing to report here.
        let Some(window) = widget.clone().downcast::<gtk::Window>().ok() else {
            continue;
        };
        let (width, height) = window.size();
        println!(
            "   [{index}] mapped={} visible={} realized={} title={:?} size={}x{} scale={}",
            window.is_mapped(),
            window.is_visible(),
            window.is_realized(),
            window.title().unwrap_or_default(),
            width,
            height,
            window.scale_factor(),
        );
    }
    println!("   read: `mapped=true` means GDK handed the window to the X server to composite.");
}
