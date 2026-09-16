// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Probe: are macOS `toggleFullScreen:` / `miniaturize:` applied synchronously?
//!
//! The `control_property_uniform` example reads `is_window_in_state` immediately
//! after a write and observed `Minimized on=true/Some(false)` and
//! `Fullscreen off=true/Some(true)`. Either the write is not reaching AppKit, or
//! AppKit applies it asynchronously and the immediate read is stale.
//!
//! This probe distinguishes the two by sampling the state repeatedly after a
//! write, so the answer is measured rather than assumed.
//!
//! Run with `cargo run --example macos_window_state_async_probe`.

#[cfg(all(
    not(alloc_frugal),
    any(feature = "desktop", feature = "tablet", feature = "mobile")
))]
use std::thread::sleep;
#[cfg(all(
    not(alloc_frugal),
    any(feature = "desktop", feature = "tablet", feature = "mobile")
))]
use std::time::Duration;

// The probe drives `app::App` and the platform singleton, both compiled out under
// `mini`/`alloc_frugal`. `required-features = ["desktop"]` in `Cargo.toml` cannot
// express that, because it is satisfied by any listed feature being on, and
// `cargo --all-features` enables `desktop` and `mini` together. Gate `main` on the
// same condition the modules use, with a stub for the builds where it is absent.
#[cfg(all(
    not(alloc_frugal),
    any(feature = "desktop", feature = "tablet", feature = "mobile")
))]
use rust_widgets::app::App;
#[cfg(all(
    not(alloc_frugal),
    any(feature = "desktop", feature = "tablet", feature = "mobile")
))]
use rust_widgets::platform::get_platform;
#[cfg(all(
    not(alloc_frugal),
    any(feature = "desktop", feature = "tablet", feature = "mobile")
))]
use rust_widgets::WindowStateFlag;

#[cfg(all(not(alloc_frugal), any(feature = "desktop", feature = "tablet", feature = "mobile")))]
fn main() {
    App::new().init();
    let platform = get_platform();
    println!("backend = {}", platform.backend_name());

    let window = platform.create_window("Async probe", 60, 60, 480, 320);
    if window == 0 {
        eprintln!("no window");
        return;
    }

    for flag in [WindowStateFlag::Fullscreen, WindowStateFlag::Minimized] {
        println!("--- {flag:?} ---");
        println!("before        = {:?}", platform.is_window_in_state(window, flag));
        let wrote = platform.set_window_state(window, flag, true);
        println!("write(true)   = {wrote}");
        for delay_ms in [0u64, 50, 200, 600, 1500] {
            sleep(Duration::from_millis(delay_ms));
            println!("after +{delay_ms:>4}ms = {:?}", platform.is_window_in_state(window, flag));
        }
        let wrote = platform.set_window_state(window, flag, false);
        println!("write(false)  = {wrote}");
        for delay_ms in [0u64, 50, 200, 600, 1500] {
            sleep(Duration::from_millis(delay_ms));
            println!("back  +{delay_ms:>4}ms = {:?}", platform.is_window_in_state(window, flag));
        }
    }
}

/// The probe's subject does not exist in this build: `mini`/`embedded` compile out
/// the app lifecycle and the platform singleton. Report that rather than fail to
/// build, which a bare `required-features` gate cannot do under `--all-features`.
#[cfg(not(all(
    not(alloc_frugal),
    any(feature = "desktop", feature = "tablet", feature = "mobile")
)))]
fn main() {
    println!(
        "macos_window_state_async_probe: not applicable in this build — it needs a \
         device profile with the platform singleton; skipped."
    );
}
