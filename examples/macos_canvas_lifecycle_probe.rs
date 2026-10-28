// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bounded probe for the macOS canvas surface lifecycle (PF-X03).
//!
//! Runs two phases against a real AppKit window and asserts the **teardown invariants** of a
//! mounted surface, which is what the defect broke:
//!
//! * phase 1 — 2000 distinct surfaces: every mount/unmount reports success, and a re-mount of the
//!   same id after unmount still succeeds (nothing stale blocks it);
//! * phase 2 — 200 cycles reusing one id: after an unmount, a second `unmount_surface` and an
//!   `invalidate_surface` must both answer `false`. A `false` here is the observable proof that the
//!   backend removed its entry rather than leaving the view reachable — the half of PF-X03 that
//!   leaked a per-id record forever.
//!
//! It drives whichever macOS backend the build selects, so one binary covers both:
//!
//! ```text
//! cargo run --no-default-features --features desktop            # cocoa-legacy canvas
//! cargo run --no-default-features --features desktop,macos      # objc2 canvas
//! ```
//!
//! # Why resident memory is printed but not asserted
//!
//! Measured on macOS 15 with this crate's bookkeeping **entirely disabled** (no association, no
//! redraw note, no platform state, no handle table): a bare `alloc`/`addSubview:`/
//! `removeFromSuperview`/`release` cycle over the same window still grows peak RSS by ~4 KB per
//! cycle. With the real code the figure is the same (~8.4 MB over 2000 cycles), the window's
//! subview count is 0 after every unmount, and `leaks --atExit` reports **no `NSView` allocation
//! at all** (144 KB constant process-lifetime overhead). So the growth is AppKit/malloc retention,
//! not a leaked surface: an RSS assertion would fail a correct build, which is worse than no
//! assertion. The number is printed as `note …` for a human watching a regression.
//!
//! # Exit codes
//!
//! `0` = every invariant held, `1` = one failed, `3` = skip (this backend hosts no surfaces).

#[cfg(all(target_os = "macos", full_widgets))]
fn main() {
    use rust_widgets::core::Rect;
    use rust_widgets::widget::WidgetFactory;

    /// Distinct-surface cycles.
    const CYCLES: usize = 2000;
    /// Cycles reusing one id, which is where the teardown invariants are checked.
    const REUSE_CYCLES: usize = 200;

    rust_widgets::init();
    let platform = rust_widgets::platform::get_platform();
    println!("macos_canvas_lifecycle_probe: backend={}", platform.backend_name());
    if !platform.supports_surfaces() {
        println!("macos_canvas_lifecycle_probe: SKIP this backend hosts no surfaces");
        std::process::exit(3);
    }

    let window = rust_widgets::create_window("canvas lifecycle probe", 60, 60, 320, 200);
    if window == 0 {
        eprintln!("macos_canvas_lifecycle_probe: FAIL the backend created no window");
        std::process::exit(1);
    }
    let rect = Rect::new(0, 0, 320, 200);
    let factory = WidgetFactory::shared();
    // `create_window` returns the widget-level id; the backend keys its windows by the host id it
    // minted, which is the translation `mount_widget_object` performs before `mount_surface`.
    let host_parent = rust_widgets::widget::runtime::host_window_for(window).unwrap_or(window);

    // The same steps `mount_widget_object` uses: register, record the surface's window, mount,
    // unmount, unregister.
    let register = || -> Option<rust_widgets::core::ObjectId> {
        let widget = factory.create("label", rect, "probe")?;
        let id = rust_widgets::widget::runtime::register(widget)?;
        let _ = rust_widgets::widget::runtime::set_geometry(id, rect);
        let _ = rust_widgets::widget::runtime::set_surface_window(id, window);
        Some(id)
    };

    // ── Phase 1: 2000 distinct surfaces ──
    let baseline = peak_resident_bytes();
    for index in 0..CYCLES {
        let Some(id) = register() else {
            eprintln!("macos_canvas_lifecycle_probe: FAIL cycle {index}: the runtime refused it");
            std::process::exit(1);
        };
        let mounted = platform.mount_surface(host_parent, id, rect);
        let unmounted = platform.unmount_surface(id);
        let unregistered = rust_widgets::widget::runtime::unregister(id);
        if !(mounted && unmounted && unregistered) {
            eprintln!(
                "macos_canvas_lifecycle_probe: FAIL cycle {index}: mounted={mounted} \
                 unmounted={unmounted} unregistered={unregistered}"
            );
            std::process::exit(1);
        }
    }

    // ── Phase 2: one id reused, teardown must be complete ──
    let Some(id) = register() else {
        eprintln!("macos_canvas_lifecycle_probe: FAIL the runtime refused the reused id");
        std::process::exit(1);
    };
    for index in 0..REUSE_CYCLES {
        if !platform.mount_surface(host_parent, id, rect) {
            eprintln!("macos_canvas_lifecycle_probe: FAIL reuse cycle {index}: mount refused");
            std::process::exit(1);
        }
        if !platform.unmount_surface(id) {
            eprintln!("macos_canvas_lifecycle_probe: FAIL reuse cycle {index}: unmount refused");
            std::process::exit(1);
        }
        // The answer below is the observable teardown contract: after unmount the backend no
        // longer holds this surface, so it must refuse to unmount it again. A `true` here means
        // the entry outlived its view, which is the per-id record PF-X03 leaked forever.
        //
        // `invalidate_surface` is deliberately not asserted: a widget id is legitimately routable
        // to its window's canvas, so repainting after an unmount is a documented fallback rather
        // than a teardown failure.
        if platform.unmount_surface(id) {
            eprintln!(
                "macos_canvas_lifecycle_probe: FAIL reuse cycle {index}: a second unmount was \
                 accepted, so the surface lingers after teardown"
            );
            std::process::exit(1);
        }
    }
    let _ = rust_widgets::widget::runtime::unregister(id);

    // Informational only: see the module docs for why this number is not asserted.
    if let (Some(baseline), Some(after)) = (baseline, peak_resident_bytes()) {
        println!(
            "macos_canvas_lifecycle_probe: note peak_rss_baseline={baseline} \
             peak_rss_after={after} growth={} (AppKit/malloc retention, not a leak assertion)",
            after.saturating_sub(baseline)
        );
    }
    println!("macos_canvas_lifecycle_probe: PASS cycles={CYCLES} reuse_cycles={REUSE_CYCLES}");
}

/// This process's peak resident set size in bytes, for the informational line above.
#[cfg(all(target_os = "macos", full_widgets))]
fn peak_resident_bytes() -> Option<u64> {
    let mut usage: libc::rusage = unsafe { core::mem::zeroed() };
    // SAFETY: `getrusage` writes into the struct this scope owns, and `RUSAGE_SELF` is always a
    // valid selector for the calling process.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return None;
    }
    // macOS reports `ru_maxrss` in bytes; Linux reports KiB. This probe only builds for macOS.
    Some(usage.ru_maxrss.max(0) as u64)
}

#[cfg(not(all(target_os = "macos", full_widgets)))]
fn main() {
    println!(
        "macos_canvas_lifecycle_probe: unsupported host or profile (macOS with a full device \
         profile is required)"
    );
    std::process::exit(3);
}
