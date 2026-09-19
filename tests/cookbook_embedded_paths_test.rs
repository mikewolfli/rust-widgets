// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pins the API surface the cookbook's `embedded` chapter teaches.
//!
//! That chapter previously documented a `rust_widgets::embedded` module —
//! `EmbeddedConfig`, `ResourceManager`, `WidgetPool`, `DpiScaler`,
//! `LightweightStyle`, `InputFilter`, `TouchPoint`, `init_embedded`,
//! `init_desktop` — none of which exists in the crate. Approximately 660 lines of
//! examples could not compile, and nothing failed, because docs are not built as
//! tests.
//!
//! This test is the tripwire the chapter lacked: every path it names is resolved
//! here, so a rename or removal breaks a build rather than silently leaving the
//! documentation teaching a module that is gone. It also asserts the behaviour the
//! chapter claims, so a chapter that says "returns false" cannot quietly become
//! wrong when the implementation changes.
//!
//! # Why some halves are gated
//!
//! The chapter teaches both styles deliberately: runtime queries for behaviour,
//! `#[cfg]` only where a symbol's *existence* differs. The platform runtime is not
//! linked on `mini` (`alloc_frugal`), so those assertions carry the same gate the
//! chapter documents.

use rust_widgets::core::PlatformFamily;
use rust_widgets::platform::portable::copy_rows;
use rust_widgets::platform::profile;
use rust_widgets::platform::types::default_capabilities_for;
use rust_widgets::platform::{FrameBuffer, SurfaceGeometry};
use rust_widgets::render_engine;

/// §2 — the profile queries the chapter's table lists, available on every profile.
#[test]
fn chapter_profile_queries_resolve_and_are_consistent() {
    // Every accessor the chapter names.
    let name = profile::profile_name();
    let has_os = profile::has_os_runtime();
    let full = profile::full_widget_set();
    let frugal = profile::is_alloc_frugal();

    assert!(!name.is_empty(), "profile_name must report something");

    // The chapter's table claims `mini` is the frugal profile and that a stripped
    // profile has no full widget set. Assert the invariants rather than the values,
    // so this holds on whichever profile the test is compiled for.
    if frugal {
        assert_eq!(name, "mini", "only `mini` sets alloc_frugal");
        assert!(!full, "a frugal profile cannot have the full widget set");
    }
    if !has_os {
        assert!(!full, "no OS runtime implies a stripped widget set");
    }
    if full {
        assert!(has_os, "the full widget set requires an OS runtime");
    }
}

/// §1 — the chapter says `supports_surfaces()` is the runtime answer to "can I mount
/// a drawing surface?", and that a stripped profile answers `false`.
#[test]
fn chapter_supports_surfaces_matches_the_profile() {
    let supported = rust_widgets::supports_surfaces();
    if profile::stripped_widget_set() {
        assert!(
            !supported,
            "the chapter states a stripped profile reports `false` rather than \
             mounting a blank surface"
        );
    }
}

/// §3 — the frame-pacing API, including the clamp the chapter highlights.
#[test]
fn chapter_frame_pacing_api_resolves() {
    // Clamped to 1..=240, and the applied value is returned.
    let applied_low = render_engine::set_embedded_target_fps(0);
    let applied_high = render_engine::set_embedded_target_fps(999);
    assert!((1..=240).contains(&applied_low));
    assert!((1..=240).contains(&applied_high));

    let applied = render_engine::set_embedded_target_fps(60);
    assert_eq!(applied, 60);
    assert_eq!(render_engine::embedded_target_fps(), 60);

    // `submit_embedded_task` returns an id and takes a frame-index callback.
    let task_id: u64 = render_engine::submit_embedded_task("chapter-test", |_frame| {});
    assert!(task_id > 0, "a queued task must get an id");
}

/// §4 — the stats struct the chapter documents field by field, including its derives.
#[test]
fn chapter_engine_stats_resolve() {
    let stats = render_engine::embedded_engine_stats();

    // The chapter promises every one of these fields, and that the type is
    // comparable so a smoke test can assert on it directly.
    let _: bool = stats.initialized;
    let _: bool = stats.running;
    let _: u64 = stats.frame_count;
    let _: usize = stats.pending_task_count;
    let _: usize = stats.window_count;
    let _: usize = stats.button_count;
    let _: u32 = stats.target_fps;

    // `PartialEq`, as documented in §4.
    assert_eq!(stats.clone(), stats);
}

/// §5 — the surface API: explicit stride, the refusal contract, and `copy_rows`.
#[test]
fn chapter_surface_api_resolves_and_behaves_as_documented() {
    // A padded stride, which is the case the chapter explains at length.
    let geometry = SurfaceGeometry { width: 320, height: 240, stride: 320 * 4 + 16 };
    let mut frame = FrameBuffer::new();
    assert!(frame.resize(geometry), "a padded-but-valid stride must be accepted");
    assert!(
        frame.frame().len() >= geometry.stride * 239 + 320 * 4,
        "the frame must cover the last visible row (the chapter's `required_end` point)"
    );

    // `tight` is the no-padding constructor.
    assert_eq!(SurfaceGeometry::tight(64, 48).stride, 64 * 4);

    // A stride narrower than one packed row is refused, not garbled.
    let bad = SurfaceGeometry { width: 8, height: 4, stride: 8 };
    let mut frame2 = FrameBuffer::new();
    assert!(!frame2.resize(bad), "an overlapping stride must be refused");

    // `copy_rows` leaves padding untouched — the chapter's explicit assertion.
    let padded = SurfaceGeometry { width: 2, height: 2, stride: 12 };
    let src = [1u8; 24];
    let mut dst = [0u8; 24];
    assert!(copy_rows(&src, &mut dst, padded));
    assert_eq!(&dst[8..12], &[0u8; 4], "padding must not be written");

    // A zero-sized surface is a valid no-op, not a failure.
    assert!(copy_rows(&[], &mut [], SurfaceGeometry { width: 0, height: 8, stride: 0 }));

    // A region that does not fit is refused rather than clipped.
    let tight = SurfaceGeometry::tight(4, 4);
    assert!(!copy_rows(&[0u8; 16], &mut [0u8; 64], tight));
}

/// §5 — the portable host identity the chapter quotes.
#[test]
fn chapter_portable_host_identity_resolves() {
    use rust_widgets::platform::types::Platform;

    let host = rust_widgets::platform::portable::instance();
    assert_eq!(host.backend_name(), rust_widgets::platform::portable::BACKEND_NAME);
    assert_eq!(host.backend_name(), "portable");
    assert_eq!(
        rust_widgets::platform::portable::FAMILY,
        PlatformFamily::Embedded,
        "the chapter explains why reporting Desktop would be wrong"
    );
}

/// §7 — the capability contract the chapter tells callers to compare against.
#[test]
fn chapter_capability_contract_resolves() {
    let expected = default_capabilities_for(PlatformFamily::Embedded);
    // The chapter prints these three; referencing them proves they exist.
    let _ = (expected.dpi_scaling, expected.ime, expected.native_menu);
}

/// §2 — the one documented place a `cfg` gate is correct: a symbol whose existence
/// differs. Asserted only where the runtime is actually linked.
#[cfg(not(alloc_frugal))]
#[test]
fn chapter_platform_runtime_resolves_where_it_exists() {
    let _: fn() -> &'static str = rust_widgets::platform::backend_name;
    let _ = rust_widgets::platform::capabilities();
    let _ = rust_widgets::platform::get_platform();
    let _: fn() = rust_widgets::platform::quit;
}

/// The module the old chapter taught must **not** come back unnoticed: if
/// something ever introduces `rust_widgets::embedded`, this test should be
/// updated deliberately rather than the chapter silently becoming true again.
#[test]
fn chapter_does_not_name_a_missing_module() {
    let chapter = include_str!("../cookbook/en/src/chapters/embedded.md");
    // Match only a *code* import, i.e. one that looks like a Rust source line
    // (`    use rust_widgets::embedded::…;`). The chapter quotes the old path in
    // its explanatory note, and that must not trip this check.
    let offending: Vec<&str> = chapter
        .lines()
        .filter(|line| line.trim_start().starts_with("use rust_widgets::embedded::"))
        .collect();
    assert!(
        offending.is_empty(),
        "the embedded chapter has code importing `rust_widgets::embedded::…`, which does not \
         exist in the crate: {offending:?} — either add the module or fix the chapter"
    );

    // And it must still be a *tutorial*, not merely a profile-differences note:
    // the APIs it walks through have to be present.
    for required in [
        "profile::profile_name",
        "set_embedded_target_fps",
        "submit_embedded_task",
        "embedded_engine_stats",
        "SurfaceGeometry",
        "copy_rows",
        "FrameBuffer",
        "supports_surfaces",
    ] {
        assert!(
            chapter.contains(required),
            "the embedded chapter no longer teaches `{required}`, so it has lost the \
             walkthrough the tripwire exists to protect"
        );
    }
}

/// The chapter points at a runnable example; it must exist and stay referenced.
#[test]
fn chapter_runnable_example_exists() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/embedded_host.rs");
    assert!(
        std::path::Path::new(path).is_file(),
        "the embedded chapter tells readers to run `examples/embedded_host.rs`, \
         which is missing"
    );
}
