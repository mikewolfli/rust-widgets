// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bounded probe for the end-to-end presentation path (BLUE29 §7, B29.09 / B29.12 / B29.13).
//!
//! The probe does what the acceptance criteria ask, against a **real window** on whichever
//! desktop backend this build selects:
//!
//! 1. asks the backend whether it can present this window (and skips honestly when it cannot);
//! 2. creates a [`FramePresenter`] from the window's native handle;
//! 3. presents a known frame and reads the presented pixels back, asserting **byte equality**
//!    with what was handed over (the BGRA/RGBA mapping included);
//! 4. changes only one row band, marks only that band damaged, and asserts the whole presented
//!    frame still equals the new frame — which is what proves the full-frame redraw from the
//!    mirror texture is correct;
//! 5. resizes the target and repeats the check at the new size;
//! 6. runs 300 frames with band damage and asserts the counters (`frames_presented` tracks the
//!    damaged frames, an undamaged present submits nothing);
//! 7. presents an unchanged frame and asserts the answer is `Idle` with no new present, which is
//!    the "an idle window costs nothing" criterion.
//!
//! # Why the checks run from inside the event loop
//!
//! A window that is never composited reports `occluded` to the graphics backend, because the
//! compositor has not put it on screen yet. The work therefore runs on the loop's own thread, one
//! turn after `run()` has started, through the library's public main-thread queue — the same way a
//! real host presents from inside its paint callback. Running the checks straight after
//! `show_widget` would test "presenting to a window that is not on screen yet", which is a
//! different question and the one the `Occluded` branch answers.
//!
//! # Exit codes
//!
//! `0` = every invariant held, `1` = one failed, `3` = skip (this backend cannot present the
//! window, so the probe has nothing to assert).
//!
//! # Running it
//!
//! ```text
//! cargo run --no-default-features --features desktop,gpu-wgpu,raw-window-handle \
//!   --example present_smoke
//! ```
//!
//! `tools/run_present_macos.sh` runs exactly that with a bounded timeout and reports the exit
//! code, so the plan's "real window, present succeeded, pixels verified" row is a command anyone
//! can re-run.

#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn main() {
    use core::sync::atomic::Ordering;
    use std::time::Duration;

    const WIDTH: u32 = 320;
    const HEIGHT: u32 = 200;

    rust_widgets::init();
    let platform = rust_widgets::platform::get_platform();
    println!("present_smoke: backend={}", platform.backend_name());

    let window = rust_widgets::create_window("present smoke", 80, 80, WIDTH, HEIGHT);
    if window == 0 {
        eprintln!("present_smoke: FAIL the backend created no window");
        std::process::exit(1);
    }
    rust_widgets::show_widget(window);

    if !platform.frame_presentation_supported(window) {
        println!(
            "present_smoke: SKIP {} cannot present this window (no native target)",
            platform.backend_name()
        );
        std::process::exit(3);
    }

    // The window is on screen only after the loop has run a turn, so the checks are queued onto
    // the loop's thread and the loop is ended from inside them. The return value says whether a
    // loop has *already* ticked; `false` is the normal answer here (this runs before `run()`),
    // and the work stays queued for the first tick, which is exactly what is wanted.
    let _already_ticking =
        rust_widgets::platform::main_thread::schedule_in(Duration::from_millis(250), move || {
            let failures = run_checks(window, WIDTH, HEIGHT);
            FAILURES.fetch_add(failures, Ordering::SeqCst);
            // Phase two needs *more* loop turns (a repaint happens between them), so it continues
            // the chain and ends the session itself.
            start_wired_phase();
        });

    rust_widgets::run();

    let failures = FAILURES.load(Ordering::SeqCst);
    if failures == 0 {
        println!("present_smoke: PASS");
        std::process::exit(0);
    }
    eprintln!("present_smoke: FAIL {failures} invariant(s) did not hold");
    std::process::exit(1);
}

#[cfg(not(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
)))]
fn main() {
    println!(
        "present_smoke: SKIP this build has no presentation path \
         (needs the desktop profile plus `gpu-wgpu` and `raw-window-handle`)"
    );
    std::process::exit(3);
}

/// Invariants that did not hold. Set by the check bodies, read after `run()` returns.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
static FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Runs every presentation invariant. Returns the number that did not hold.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn run_checks(window: u64, width: u32, height: u32) -> u32 {
    use rust_widgets::render::present::{FramePresenter, PresentationStatus, SurfaceFailure};
    use rust_widgets::wgpu_backend::RenderQuality;

    const BAND: u32 = 16;
    const FRAMES: u32 = 300;

    let platform = rust_widgets::platform::get_platform();
    let Some(raw_window_handle) = platform.raw_window_handle(window) else {
        eprintln!("present_smoke: FAIL the backend claimed it can present but named no handle");
        return 1;
    };
    let raw_display_handle = platform.raw_display_handle();

    let mut presenter = match FramePresenter::new(
        raw_window_handle,
        raw_display_handle,
        (width, height),
        1.0,
        RenderQuality::ByteExact,
    ) {
        Ok(presenter) => presenter,
        Err(error) => {
            eprintln!("present_smoke: FAIL the presenter could not be created: {error}");
            return 1;
        }
    };
    println!(
        "present_smoke: target {}x{} format={:?} alpha={:?} present={:?}",
        presenter.config().width,
        presenter.config().height,
        presenter.config().format,
        presenter.config().alpha_mode,
        presenter.config().present_mode,
    );

    let mut failures = 0u32;
    let mut frame = pattern_frame(width, height, 0);
    // A window whose display is asleep (or whose session is not composited) reports `occluded`
    // until the compositor wakes. Retrying is the documented response to a transient failure
    // (B29.11), so the probe retries for a bounded time and then says the host is the limit
    // rather than reporting a library defect (principle #103).
    let mut occluded_retries = 0u32;
    let first = loop {
        presenter.mark_full_damage();
        match presenter.present_and_read_back(&frame) {
            Ok((_, pixels)) => {
                if presenter.status() == PresentationStatus::Skipped
                    && presenter.last_failure() == Some(SurfaceFailure::Occluded)
                {
                    if occluded_retries >= 40 {
                        println!(
                            "present_smoke: SKIP(host-limited) the window server keeps this \
                             window occluded after {} retries; the display is asleep or the \
                             session has no composited desktop",
                            occluded_retries
                        );
                        std::process::exit(3);
                    }
                    occluded_retries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                break (pixels, presenter.status());
            }
            Err(error) => {
                eprintln!("present_smoke: FAIL the first frame could not be verified: {error}");
                return failures + 1;
            }
        }
    };
    failures += report_pixel_difference("first frame", &frame, &first.0);
    if first.1 != PresentationStatus::Presented {
        eprintln!(
            "present_smoke: FAIL first present answered {} (failure: {:?})",
            first.1.label(),
            presenter.last_failure().map(|failure| failure.label())
        );
        failures += 1;
    }

    // A single damaged band: 16 rows are uploaded, but the presented frame must be whole.
    // The painted rows and the reported damage must be the **same** rows, or the probe would be
    // asserting that an unreported change was uploaded — the opposite of the contract.
    paint_band(&mut frame, width, BAND, BAND, [250, 30, 40, 255]);
    presenter.mark_damage(&[(0, BAND as i32, width, BAND)]);
    match presenter.present_and_read_back(&frame) {
        Ok((_, pixels)) => failures += report_pixel_difference("band update", &frame, &pixels),
        Err(error) => {
            eprintln!("present_smoke: FAIL the band update could not be verified: {error}");
            failures += 1;
        }
    }

    // Smooth delivery: a bridged command stream drawn by the painter engine and blitted onto the
    // target, then read back. This is the `Smooth` half's delivery path — no CPU rasteriser and no
    // pixel upload are involved.
    {
        use rust_widgets::wgpu_backend::{PixelRect, Rgba8, WgpuDrawCommand};
        let red = Rgba8 { r: 250, g: 20, b: 30, a: 255 };
        let green = Rgba8 { r: 20, g: 220, b: 40, a: 255 };
        let commands = [
            WgpuDrawCommand::Clear { color: Rgba8 { r: 255, g: 255, b: 255, a: 255 } },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 8, y: 8, width: 40, height: 24 },
                color: red,
                clip: None,
            },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 20, y: 14, width: 8, height: 8 },
                color: green,
                clip: None,
            },
        ];
        match presenter.present_commands_and_read_back(&commands) {
            Ok((status, pixels)) => {
                if status != PresentationStatus::Presented {
                    eprintln!(
                        "present_smoke: FAIL the command stream answered {} (failure: {:?})",
                        status.label(),
                        presenter.last_failure().map(|failure| failure.label())
                    );
                    failures += 1;
                } else {
                    let at = |x: u32, y: u32| {
                        let start = ((y * width + x) * 4) as usize;
                        [pixels[start], pixels[start + 1], pixels[start + 2], pixels[start + 3]]
                    };
                    let outside = at(1, 1);
                    let filled = at(10, 10);
                    let overlapping = at(24, 18);
                    if outside != [255, 255, 255, 255]
                        || filled != [red.r, red.g, red.b, red.a]
                        || overlapping != [green.r, green.g, green.b, green.a]
                    {
                        eprintln!(
                            "present_smoke: FAIL the command stream reached the compositor with \
                             unexpected pixels (outside {outside:?}, filled {filled:?}, \
                             overlapping {overlapping:?})"
                        );
                        failures += 1;
                    } else {
                        println!(
                            "present_smoke: command stream presented and read back at the \
                             expected pixels"
                        );
                    }
                }
            }
            Err(error) => {
                eprintln!("present_smoke: FAIL the command stream could not be verified: {error}");
                failures += 1;
            }
        }
    }

    // An unchanged frame must cost nothing.
    let presented_before = presenter.stats().frames_presented;
    match presenter.present(&frame) {
        Ok(PresentationStatus::Idle) => {
            if presenter.stats().frames_presented != presented_before {
                eprintln!("present_smoke: FAIL an undamaged present still presented a frame");
                failures += 1;
            }
        }
        Ok(status) => {
            eprintln!("present_smoke: FAIL an undamaged present answered {}", status.label());
            failures += 1;
        }
        Err(error) => {
            eprintln!("present_smoke: FAIL an undamaged present errored: {error}");
            failures += 1;
        }
    }

    // A resize: new target size, one full upload, pixels verified again.
    let (resized_width, resized_height) = (width / 2, height / 2);
    presenter.resize((resized_width, resized_height), 1.0);
    let mut resized = pattern_frame(resized_width, resized_height, 1);
    match presenter.present_and_read_back(&resized) {
        Ok((_, pixels)) => failures += report_pixel_difference("resize", &resized, &pixels),
        Err(error) => {
            eprintln!("present_smoke: FAIL the resized frame could not be verified: {error}");
            failures += 1;
        }
    }

    // Steady state: band damage only, presented once per frame.
    let presented_before = presenter.stats().frames_presented;
    for index in 0..FRAMES {
        let band = index % resized_height.max(1);
        paint_band(&mut resized, resized_width, band, 1, [index as u8, 90, 160, 255]);
        presenter.mark_damage(&[(0, band as i32, resized_width, 1)]);
        match presenter.present(&resized) {
            Ok(PresentationStatus::Presented) => {}
            Ok(status) => {
                eprintln!(
                    "present_smoke: FAIL frame {index} answered {} instead of presented \
                     (failure: {:?})",
                    status.label(),
                    presenter.last_failure().map(|failure| failure.label())
                );
                failures += 1;
                break;
            }
            Err(error) => {
                eprintln!("present_smoke: FAIL frame {index} errored: {error}");
                failures += 1;
                break;
            }
        }
    }
    let presented = presenter.stats().frames_presented - presented_before;
    if presented != u64::from(FRAMES) {
        eprintln!("present_smoke: FAIL {presented} of {FRAMES} damaged frames were presented");
        failures += 1;
    }
    if let Err(error) = presenter.wait_idle() {
        eprintln!("present_smoke: FAIL the device never reached idle: {error}");
        failures += 1;
    }
    println!(
        "present_smoke: stats offered={} presented={} skipped={} rows={}",
        presenter.stats().frames_offered,
        presenter.stats().frames_presented,
        presenter.stats().frames_skipped,
        presenter.stats().rows_uploaded,
    );

    // The capability question is **per window**, and this is the measurement: the live window
    // answers `true` (that is what let this probe start) while an id that names no window answers
    // `false`. A process-wide answer would have to pick one value for both, which is why
    // `frame_presentation_supported` takes the window.
    let stale = u64::MAX - 7;
    if platform.frame_presentation_supported(stale) {
        eprintln!(
            "present_smoke: FAIL the capability is reported for id {stale}, which names no window"
        );
        failures += 1;
    } else {
        println!("present_smoke: capability is per window (live: true, unknown id {stale}: false)");
    }

    // Teardown in the order the contract requires: the presenter borrows the window, so it goes
    // first, and then the window is hidden. Closing the window while a presenter is alive would
    // leave the presenter holding a dead native object, which is exactly what the contract forbids.
    drop(presenter);
    rust_widgets::app::WindowHandle::from_raw(window).close();
    println!("present_smoke: teardown (presenter dropped, window closed)");
    failures
}

/// Phase two: the intent the public API records, consumed by the library's own canvas.
///
/// # What this measures that phase one cannot
///
/// Phase one drives `FramePresenter` by hand, so it proves the target works but says nothing about
/// whether the **canvas** consults the intent. This phase sets the intent through
/// `WindowHandle::set_frame_presentation`, forces repaints, and then reads
/// `present::presented_frames` — the counter the canvas path increments. A non-zero count is the
/// only evidence that a real `drawRect:` reached the compositor through the presentation path
/// rather than blitting.
///
/// It then switches back to `Default` and forces another repaint: the counter must **not** grow,
/// which is what proves the default delivery path is still the default.
///
/// # Why several repaints
///
/// The target is created *inside* the first `drawRect:` of the window, i.e. before the compositor
/// has ever shown it, so the first acquisition is legitimately refused as `occluded` (the backend
/// asks the window server, which does not consider a window visible before its first commit). The
/// probe therefore forces a bounded number of repaints and succeeds on the first one that gets
/// through, exactly as a real host's frame loop would.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn start_wired_phase() {
    use core::sync::atomic::Ordering;
    use rust_widgets::app::{WidgetHandle, WindowHandle};
    use rust_widgets::render::present::FramePresentation;
    use std::time::Duration;

    let window = rust_widgets::create_window("present wired", 80, 80, 160, 120);
    if window == 0 {
        eprintln!("present_smoke: FAIL the backend created no window for the wired phase");
        FAILURES.fetch_add(1, Ordering::SeqCst);
        let _ = rust_widgets::quit();
        return;
    }
    rust_widgets::show_widget(window);
    if !WindowHandle::from_raw(window).set_frame_presentation(FramePresentation::Exact) {
        // This backend's canvas does not consume the intent yet (or this window cannot present).
        // That is a host limitation of *this build*, reported by the API itself, so the phase is
        // skipped rather than failed (principle #103).
        println!(
            "present_smoke: SKIP(wiring) {} does not consume a presentation intent yet",
            rust_widgets::platform::get_platform().backend_name()
        );
        let _ = rust_widgets::quit();
        return;
    }
    // A child's text change is a real invalidation: the canvas repaints the tree on the next
    // display cycle, which is where the intent is read. The first repaint waits for the window to
    // be composited: a window that has not been shown yet is legitimately `occluded`, and the
    // probe's job is to test the intent, not to race the window server (principle #103).
    let label = WindowHandle::from_raw(window).new_label("wired", 8, 8, 120, 20);
    println!("present_smoke: wired phase set intent={:?}", FramePresentation::Exact);
    let _ =
        rust_widgets::platform::main_thread::schedule_in(Duration::from_millis(600), move || {
            wired_step(window, label.raw_id(), 0)
        });
}

/// Measures the two lifecycle facts a frame loop depends on, on a real window.
///
/// 1. **Idle submits nothing.** With no invalidation the canvas is never asked to paint, so the
///    presented-frame counter must not move for the idle window. This is B29.12's acceptance in the
///    only place it can be measured: a real compositor session.
/// 2. **Hide/show resumes.** Hiding the window stops presentation (the compositor stops asking for
///    frames); showing it and invalidating must present again, with no failure recorded. That is
///    B29.11's "minimise / restore" half.
///
/// The idle window defaults to 60 s (the plan's number) and can be shortened with
/// `PRESENT_IDLE_SECONDS` for a quick local run; the gate uses the default.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn idle_check(window: u64, label: u64) {
    use core::sync::atomic::Ordering;
    use rust_widgets::render::present::presented_frames;
    use std::time::Duration;

    let seconds = std::env::var("PRESENT_IDLE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(60);
    let before = presented_frames(window);
    println!("present_smoke: idling for {seconds} s with no invalidation");
    let _ =
        rust_widgets::platform::main_thread::schedule_in(Duration::from_secs(seconds), move || {
            let after = presented_frames(window);
            if after != before {
                eprintln!(
                    "present_smoke: FAIL {after} frames were presented while the window was idle \
                     ({before} before the wait): an idle window must submit nothing"
                );
                FAILURES.fetch_add(1, Ordering::SeqCst);
            } else {
                println!(
                    "present_smoke: idle window submitted nothing for {seconds} s \
                     ({after} presented, unchanged)"
                );
            }
            // Hide, wait, show, invalidate: presentation must resume.
            rust_widgets::hide_widget(window);
            let _ = rust_widgets::platform::main_thread::schedule_in(
                Duration::from_millis(400),
                move || {
                    rust_widgets::show_widget(window);
                    rust_widgets::set_widget_text(label, "wired: restored");
                    let _ = rust_widgets::platform::main_thread::schedule_in(
                        Duration::from_millis(500),
                        move || {
                            let resumed = presented_frames(window);
                            if resumed <= after {
                                // A refusal reason that is a host condition (the compositor does
                                // not consider the window visible, e.g. a sleeping display) is
                                // reported as a skip, not as a library failure (principle #103).
                                let refusal = rust_widgets::render::present::last_refusal(window);
                                match refusal.as_deref() {
                                    Some("occluded") => println!(
                                        "present_smoke: SKIP(host-limited) presentation did not \
                                         resume because the window server reports the window \
                                         occluded (display asleep or session not composited)"
                                    ),
                                    other => {
                                        eprintln!(
                                            "present_smoke: FAIL presentation did not resume \
                                             after the window was shown again ({after} → \
                                             {resumed}, refusal: {other:?})"
                                        );
                                        FAILURES.fetch_add(1, Ordering::SeqCst);
                                    }
                                }
                            } else {
                                println!(
                                    "present_smoke: presentation resumed after hide/show \
                                     ({after} → {resumed})"
                                );
                            }
                            // Phase three drives this same window with the `Smooth` intent and
                            // ends the session itself.
                            start_smooth_phase(window, label);
                        },
                    );
                },
            );
        });
}

/// Phase three: the `Smooth` intent delivers the frame as **draw commands**.
///
/// # Why it drives the window the wired phase already presented from
///
/// The `Smooth` path is gated per frame: one command the pipelines cannot draw faithfully sends the
/// whole frame back to the reference rasteriser (B29.10). This phase separates two facts that are
/// easy to confuse:
///
/// * the canvas **records and offers** the command stream (proved by `presented_command_frames`), or
/// * the stream is refused and the reason is named (proved by `smooth_fallback_reason`), or
/// * the stream was drawable and the **surface** refused the frame (proved by `last_refusal`).
///
/// All three are results; a silent fourth answer would not be, which is what the `else` branch
/// checks. That distinction is exactly the one principle #103 asks for.
///
/// The window is the wired phase's own, not a fresh one: a newly created window is reported
/// `occluded` by this host's window server for as long as it is not composited, which made the
/// phase unable to tell "the stream was refused" from "the window was never shown". Driving a
/// window that has just presented proves the surface works, so the answer is about the stream.
/// Its tree also carries a label, so the frame under test is a real control tree rather than bare
/// chrome.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn start_smooth_phase(window: u64, label: u64) {
    use core::sync::atomic::Ordering;
    use rust_widgets::app::WindowHandle;
    use rust_widgets::render::present::FramePresentation;
    use std::time::Duration;

    if window == 0 {
        eprintln!("present_smoke: FAIL the smooth phase has no window to drive");
        FAILURES.fetch_add(1, Ordering::SeqCst);
        let _ = rust_widgets::quit();
        return;
    }
    if !WindowHandle::from_raw(window).set_frame_presentation(FramePresentation::Smooth) {
        println!(
            "present_smoke: SKIP(wiring) {} does not consume a presentation intent yet",
            rust_widgets::platform::get_platform().backend_name()
        );
        let _ = rust_widgets::quit();
        return;
    }
    println!("present_smoke: smooth phase set intent=Smooth");
    // The **same** window the wired phase just presented from, for two reasons. It carries a real
    // control tree (a label), which is the stream worth asking about; and its surface is known to
    // composite on this host, so a refusal here means the *stream* was refused rather than the
    // window server leaving a fresh window occluded forever.
    let _ =
        rust_widgets::platform::main_thread::schedule_in(Duration::from_millis(200), move || {
            rust_widgets::set_widget_text(label, "smooth: a");
            if let Some(widget) = rust_widgets::widget::runtime::widget_id_for_host_window(window) {
                rust_widgets::widget::runtime::request_repaint(widget);
            }
            let _ = rust_widgets::platform::main_thread::schedule_in(
                Duration::from_millis(600),
                move || {
                    use rust_widgets::render::present::{
                        has_target, last_outcome, last_refusal, presented_command_frames,
                        smooth_fallback_reason,
                    };
                    let commands = presented_command_frames(window);
                    if commands > 0 {
                        println!(
                            "present_smoke: canvas delivered {commands} frame(s) as draw commands \
                         (Smooth)"
                        );
                    } else if let Some(reason) = smooth_fallback_reason(window) {
                        println!(
                        "present_smoke: SKIP(smooth-unbridged) the frame still contains commands \
                         the pipelines cannot draw faithfully ({reason}); it was delivered \
                         byte-exactly"
                    );
                    } else if let Some(reason) = last_refusal(window) {
                        // The stream *was* drawable: the pipelines accepted every command and the
                        // target refused the frame itself (a surface that cannot be acquired on
                        // this host). That is a named second answer, not a silent fallback.
                        println!(
                            "present_smoke: SKIP(present-refused) the recorded frame was \
                             drawable on the GPU, but this window's surface refused it ({reason})"
                        );
                    } else {
                        eprintln!(
                            "present_smoke: FAIL the Smooth intent neither delivered commands nor \
                         said why (outcome {:?}, target {})",
                            last_outcome(window),
                            has_target(window)
                        );
                        FAILURES.fetch_add(1, Ordering::SeqCst);
                    }
                    let _ = rust_widgets::quit();
                },
            );
        });
}

/// One repaint attempt of the wired phase; chains itself until the target presents or the budget
/// is spent.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn wired_step(window: u64, label: u64, step: u32) {
    use core::sync::atomic::Ordering;
    use rust_widgets::app::WindowHandle;
    use rust_widgets::render::present::{
        has_target, last_outcome, presented_frames, FramePresentation,
    };
    use std::time::Duration;

    /// Repaints to force before concluding the canvas never presents. At 150 ms each this is about
    /// 4.5 s, the same bounded wait the presenter phase gives a fresh window before it declares the
    /// host unable to composite (principle #103).
    const STEPS: u32 = 30;

    rust_widgets::set_widget_text(
        label,
        if step.is_multiple_of(2) { "wired: a" } else { "wired: b" },
    );
    let _ =
        rust_widgets::platform::main_thread::schedule_in(Duration::from_millis(150), move || {
            let presented = presented_frames(window);
            if presented > 0 {
                println!(
                    "present_smoke: canvas presented {presented} frame(s) through the target \
                 (step {step})"
                );
                // Back to the default delivery path, then force another repaint and check that the
                // counter stops: the default path must still own the window.
                let _ = WindowHandle::from_raw(window)
                    .set_frame_presentation(FramePresentation::Default);
                let before = presented;
                let _ = rust_widgets::platform::main_thread::schedule_in(
                    Duration::from_millis(200),
                    move || {
                        rust_widgets::set_widget_text(label, "wired: default");
                        let _ = rust_widgets::platform::main_thread::schedule_in(
                            Duration::from_millis(300),
                            move || {
                                let after = presented_frames(window);
                                if after != before {
                                    eprintln!(
                                    "present_smoke: FAIL the counter grew from {before} to \
                                     {after} after the window returned to the default delivery path"
                                );
                                    FAILURES.fetch_add(1, Ordering::SeqCst);
                                } else {
                                    println!(
                                        "present_smoke: default delivery path still owns the \
                                         window ({after} presented, unchanged)"
                                    );
                                }
                                // Re-enable the path, then measure the two lifecycle facts: an
                                // idle window submits nothing, and a hidden-then-shown window
                                // presents again.
                                let _ = WindowHandle::from_raw(window)
                                    .set_frame_presentation(FramePresentation::Exact);
                                rust_widgets::set_widget_text(label, "wired: lifecycle");
                                let _ = rust_widgets::platform::main_thread::schedule_in(
                                    Duration::from_millis(300),
                                    move || idle_check(window, label),
                                );
                            },
                        );
                    },
                );
                return;
            }
            if step + 1 >= STEPS {
                let refusal = rust_widgets::render::present::last_refusal(window);
                if refusal.as_deref() == Some("occluded") {
                    println!(
                        "present_smoke: SKIP(host-limited) the wired window stays occluded after \
                         {STEPS} repaints; the window server has not composited it"
                    );
                    std::process::exit(3);
                }
                eprintln!(
                    "present_smoke: FAIL no frame reached the compositor through the canvas after \
                 {STEPS} repaints (target={} last={:?} refusal={:?})",
                    has_target(window),
                    last_outcome(window),
                    refusal
                );
                FAILURES.fetch_add(1, Ordering::SeqCst);
                let _ = rust_widgets::quit();
                return;
            }
            wired_step(window, label, step + 1);
        });
}

/// A frame with four quadrants, a gradient and two opaque alpha values.
///
/// The content matters: a single flat colour would pass byte equality even with a wrong channel
/// order or a wrong row stride, which are exactly the mistakes this probe is for.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn pattern_frame(width: u32, height: u32, salt: u8) -> Vec<u8> {
    let mut frame = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let offset = (y as usize * width as usize + x as usize) * 4;
            let left = x < width / 2;
            let top = y < height / 2;
            let pixel = match (left, top) {
                (true, true) => [255, 0, 0, 255],
                (false, true) => [0, 255, 0, 255],
                (true, false) => [0, 0, 255, 255],
                (false, false) => {
                    let ramp = (x * 255 / width.max(1)) as u8;
                    [ramp, ramp, ramp, 255]
                }
            };
            frame[offset] = pixel[0].wrapping_add(salt);
            frame[offset + 1] = pixel[1];
            frame[offset + 2] = pixel[2];
            frame[offset + 3] = pixel[3];
        }
    }
    frame
}

/// Paints `height` rows starting at `y` a solid colour.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn paint_band(frame: &mut [u8], width: u32, y: u32, height: u32, colour: [u8; 4]) {
    let row_bytes = width as usize * 4;
    for row in y..y + height {
        let start = row as usize * row_bytes;
        let end = start + row_bytes;
        if end > frame.len() {
            return;
        }
        for pixel in frame[start..end].as_chunks_mut::<4>().0 {
            *pixel = colour;
        }
    }
}

/// Reports how far the presented pixels are from the frame, returning `1` when they differ.
#[cfg(all(
    full_widgets,
    feature = "raw-window-handle",
    feature = "gpu-wgpu",
    not(target_arch = "wasm32")
))]
fn report_pixel_difference(label: &str, expected: &[u8], actual: &[u8]) -> u32 {
    if expected == actual {
        println!("present_smoke: {label} byte-identical ({} bytes)", expected.len());
        return 0;
    }
    if expected.len() != actual.len() {
        eprintln!(
            "present_smoke: FAIL {label} length mismatch: expected {} bytes, read {}",
            expected.len(),
            actual.len()
        );
        return 1;
    }
    let differing = expected.iter().zip(actual).filter(|(a, b)| a != b).count();
    let max_delta = expected.iter().zip(actual).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    eprintln!(
        "present_smoke: FAIL {label} differs in {differing}/{} bytes (max delta {max_delta})",
        expected.len()
    );
    1
}
