// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A complete, runnable resource-constrained host.
//!
//! This is the executable companion to the cookbook's "Embedded / Resource-
//! Constrained Support" chapter — every API the chapter teaches appears here, so
//! the chapter cannot silently drift into describing something that does not
//! build. Run it directly:
//!
//! ```text
//! cargo run --no-default-features --features embedded --example embedded_host
//! cargo run --no-default-features --features mini     --example embedded_host
//! ```
//!
//! It also builds on `desktop` (with different reported values), so the default
//! `cargo check --examples` compiles it.
//!
//! The point is the *shape* of such a host: ask what the build is, drive a
//! frame-paced task queue, allocate a surface with an explicit stride, reshape a
//! foreign buffer, and read the runtime registry — all through runtime queries,
//! never through profile branches in the calling code.
//!
//! # Why some lines carry `#[cfg]` and this file is still the chapter's example
//!
//! Anything reaching the *platform runtime* (`backend_name`, `capabilities`,
//! `get_platform`, `quit`) is gated `not(alloc_frugal)`: `mini` links no OS runtime
//! at all, so those symbols genuinely do not exist there. That is a capability
//! difference expressed at compile time because the symbol's *existence* differs,
//! not a profile branch choosing behaviour. The chapter explains when each style is
//! right; the summary is that caller code branches on runtime queries for
//! *behaviour* and only uses `cfg` for *capability existence*.

use rust_widgets::core::PlatformFamily;
use rust_widgets::platform::portable::copy_rows;
use rust_widgets::platform::profile;
use rust_widgets::platform::types::default_capabilities_for;
#[cfg(not(alloc_frugal))]
use rust_widgets::platform::{self};
use rust_widgets::platform::{FrameBuffer, PlatformCapabilities, SurfaceGeometry};

fn main() {
    // ── 1. Ask the build what it is, instead of branching on the profile ──
    //
    // These are runtime answers. Caller code never contains `cfg(target_os)` or a
    // profile branch: the same code runs on a desktop and on a constrained target
    // and adapts by itself.
    println!("DEMO_PROFILE={}", profile::profile_name());
    // The platform runtime is absent on `mini` (no OS runtime is linked), so these
    // are a capability-existence difference and are gated at compile time.
    #[cfg(not(alloc_frugal))]
    println!("BACKEND={}", platform::backend_name());
    println!("HAS_OS_RUNTIME={}", profile::has_os_runtime());
    println!("FULL_WIDGET_SET={}", profile::full_widget_set());
    println!("ALLOC_FRUGAL={}", profile::is_alloc_frugal());
    println!("SUPPORTS_SURFACES={}", rust_widgets::supports_surfaces());

    // ── 2. Configure the frame budget ──
    //
    // The target FPS is clamped to a sane range and the *applied* value is
    // returned, so a caller that asks for 999 learns what it actually got rather
    // than assuming it got what it asked for.
    let applied = rust_widgets::render_engine::set_embedded_target_fps(60);
    println!("APPLIED_FPS={applied}");
    println!("TARGET_FPS={}", rust_widgets::render_engine::embedded_target_fps());

    // ── 3. Queue work onto the frame loop ──
    //
    // A task runs once, on the next frame, and receives that frame's index. This is
    // how a host schedules work without a thread of its own — the important property
    // on a constrained target, where the `mini` profile has no second thread to wake
    // at all (its loop paces by spinning on the same monotonic clock).
    let task_id =
        rust_widgets::render_engine::submit_embedded_task("embedded-host-tick", |frame_index| {
            println!("TASK_RAN_ON_FRAME={frame_index}");
        });
    println!("TASK_ID={task_id}");

    // ── 4. Allocate a drawing surface ──
    //
    // `SurfaceGeometry` carries an explicit `stride`, *not* just width and height.
    // Real panels are routinely padded to an alignment, and code that assumed
    // `width * 4` would shear the image on exactly those hosts. Using a padded
    // stride here exercises the path a real driver takes.
    let width = 320u32;
    let height = 240u32;
    let geometry = SurfaceGeometry { width, height, stride: width as usize * 4 + 16 };
    let mut frame = FrameBuffer::new();
    if !frame.resize(geometry) {
        // A stride narrower than one packed row cannot describe a frame; the call
        // refuses rather than producing a garbled one.
        eprintln!("surface geometry rejected");
        return;
    }
    println!("SURFACE_BYTES={}", frame.frame().len());

    // ── 5. Reshape a foreign buffer ──
    //
    // `copy_rows` is the portable host's one real service: a driver whose buffer is
    // padded differently (or whose origin is bottom-up) reshapes it here, instead of
    // every widget learning that host's layout. It is bounds-checked in both
    // directions and refuses rather than silently clipping — a clipped frame is a
    // rendering bug that hides itself.
    let row_count = geometry.stride * height as usize;
    let source = vec![0xABu8; row_count];
    let mut destination = vec![0u8; row_count];
    let copied = copy_rows(&source, &mut destination, geometry);
    println!("COPY_ROWS_OK={copied}");

    // ── 6. Read the runtime registry ──
    //
    // The diagnostics surface: whether the engine is initialised, how many frames
    // have elapsed, how much queued work is outstanding. On a device these are the
    // numbers a watchdog or a smoke test asserts on.
    let stats = rust_widgets::render_engine::embedded_engine_stats();
    println!("ENGINE_INITIALIZED={}", stats.initialized);
    println!("ENGINE_RUNNING={}", stats.running);
    println!("FRAME_COUNT={}", stats.frame_count);
    println!("PENDING_TASKS={}", stats.pending_task_count);
    println!("WINDOW_COUNT={}", stats.window_count);
    println!("BUTTON_COUNT={}", stats.button_count);

    // ── 7. Compare the live capability contract against the family default ──
    //
    // The same query the library uses when it negotiates. A caller can compare the
    // live answer with the expected one instead of guessing from the profile name.
    // (`capabilities()` needs the platform runtime, so `mini` reads only the
    // expected side.)
    let expected: PlatformCapabilities = default_capabilities_for(PlatformFamily::Embedded);
    println!("EXPECTED_EMBEDDED_DPI_SCALING={}", expected.dpi_scaling);
    println!("EXPECTED_EMBEDDED_IME={}", expected.ime);
    println!("EXPECTED_EMBEDDED_NATIVE_MENU={}", expected.native_menu);
    #[cfg(not(alloc_frugal))]
    {
        let live = platform::capabilities();
        println!("LIVE_DPI_SCALING={}", live.dpi_scaling);
        println!("LIVE_IME={}", live.ime);
        println!("LIVE_NATIVE_MENU={}", live.native_menu);
    }

    // ── 8. Tear down ──
    //
    // `quit()` stops the loop and clears the window/button registry plus any queued
    // tasks, so a host can tear down and re-initialise without stale state. It is
    // part of the platform runtime, hence absent on `mini` — where the engine's own
    // `embedded_engine_stats()` still reports state either way.
    #[cfg(not(alloc_frugal))]
    platform::quit();
    println!("DONE");
}
