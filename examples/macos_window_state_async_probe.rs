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

use std::thread::sleep;
use std::time::Duration;

use rust_widgets::app::App;
use rust_widgets::platform::get_platform;
use rust_widgets::WindowStateFlag;

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
