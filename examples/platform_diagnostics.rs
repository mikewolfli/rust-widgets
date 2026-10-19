// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Runs the unified platform diagnostic probe on this host.
//!
//! This is the desktop harness for BLUE28 B28.01. It prints the stable JSON form, one line per
//! fact, and a final `RESULT: PASS` / `RESULT: FAIL` line. A fact this backend honestly does not
//! have is reported as `skip` and does not fail the run.
//!
//! Run with:
//!
//! ```text
//! cargo run --example platform_diagnostics
//! ```
//!
//! The same probe is reachable from C as `rw_platform_diagnostics()` (mask) and
//! `rw_platform_diagnostics_json()` (this JSON), and from Android through
//! `nativePlatformProbe`.

fn main() {
    rust_widgets::init();
    let diagnostics = rust_widgets::platform::diagnostics::run();

    println!("{}", diagnostics.to_json());
    for entry in &diagnostics.entries {
        println!(
            "bit {:>2} {:<20} {:<5} {}",
            entry.bit,
            entry.name,
            entry.status.as_str(),
            entry.detail
        );
    }
    println!("{}", diagnostics.summary());
    println!(
        "host: dpi_scale_factor={} displays={:?}",
        rust_widgets::platform::dpi_scale_factor(),
        rust_widgets::platform::displays()
    );
    let environment = rust_widgets::style::EnvironmentSnapshot::from_provider(
        &rust_widgets::style::environment::PlatformEnvironment,
    );
    println!(
        "environment: locale={:?} direction={:?} contrast={} motion={:?} theme={:?}",
        environment.locale,
        environment.text_direction,
        environment.high_contrast,
        environment.motion_preference,
        environment.color_scheme
    );
    println!(
        "system_fonts: {} families (first 3: {:?})",
        rust_widgets::platform::get_platform().system_fonts().len(),
        rust_widgets::platform::get_platform()
            .system_fonts()
            .iter()
            .take(3)
            .map(|font| font.family.clone())
            .collect::<Vec<_>>()
    );

    if diagnostics.is_pass() {
        println!("RESULT: PASS");
    } else {
        println!("RESULT: FAIL");
        std::process::exit(1);
    }
}
