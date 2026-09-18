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
//! documentation teaching a module that is gone.
//!
//! # Why the two halves are gated separately
//!
//! `supports_surfaces` and the portable drawing surface exist on every profile,
//! but `backend_name` / `capabilities` / `get_platform` are part of the platform
//! runtime, which a stripped profile (`mini`/`embedded`) does not link. The
//! chapter's capability-query example is therefore documented as a device-build
//! idiom, and this test mirrors that with the same gate rather than asserting
//! those symbols exist everywhere.

/// The profile/surface API the rewritten chapter uses — available on every
/// profile, including the stripped ones the chapter is about.
#[test]
fn cookbook_embedded_chapter_surface_paths_resolve() {
    // `supports_surfaces` is the live replacement for the deprecated
    // `supports_custom_widgets` alias; the chapter teaches the live name.
    let _: fn() -> bool = rust_widgets::supports_surfaces;

    // The drawing surface the chapter's "Repainting" section uses.
    let geometry = rust_widgets::platform::SurfaceGeometry::tight(320, 240);
    let mut buffer = rust_widgets::platform::FrameBuffer::new();
    assert!(buffer.resize(geometry), "a tightly packed geometry must be usable");
    let frame = buffer.frame_mut();
    assert_eq!(frame.len(), 320 * 240 * 4);
}

/// The runtime-fact queries the chapter tells callers to use instead of
/// branching on the profile. Gated on `device_profile` because the platform
/// runtime is not linked on `mini`/`embedded`.
#[cfg(device_profile)]
#[test]
fn cookbook_embedded_chapter_capability_queries_resolve() {
    let _: fn() -> &'static str = rust_widgets::platform::backend_name;
    let caps = rust_widgets::platform::capabilities();
    // Referencing the fields proves they exist; the values are backend facts.
    let _ = (caps.ime, caps.accessibility, caps.dpi_scaling);
    let _ = rust_widgets::platform::get_platform();
}

/// The module the old chapter taught must **not** come back unnoticed: if
/// something ever introduces `rust_widgets::embedded`, this test should be
/// updated deliberately rather than the chapter silently becoming true again.
#[test]
fn cookbook_embedded_chapter_no_longer_names_a_missing_module() {
    let chapter = include_str!("../cookbook/en/src/chapters/embedded.md");
    // Match only a *code* import, i.e. one that looks like a Rust source line
    // (`    use rust_widgets::embedded::…;`). The chapter's own explanation quotes
    // the old path inline in prose, and that must not trip this check.
    let offending: Vec<&str> = chapter
        .lines()
        .filter(|line| line.trim_start().starts_with("use rust_widgets::embedded::"))
        .collect();
    assert!(
        offending.is_empty(),
        "the embedded chapter has code importing `rust_widgets::embedded::…`, which does not \
         exist in the crate: {offending:?} — either add the module or fix the chapter"
    );
    // The chapter's own replacement claims must stay present.
    assert!(chapter.contains("supports_surfaces"));
    assert!(chapter.contains("SurfaceGeometry"));

    // The same stale import appeared in `advanced-topics.md`, so the tripwire
    // covers both documents: whichever one somebody edits, a reintroduced
    // `rust_widgets::embedded::` import fails the build.
    let advanced = include_str!("../cookbook/en/src/chapters/advanced-topics.md");
    let stale: Vec<&str> = advanced
        .lines()
        .filter(|line| line.trim_start().starts_with("use rust_widgets::embedded::"))
        .collect();
    assert!(
        stale.is_empty(),
        "advanced-topics.md imports `rust_widgets::embedded::…`, which does not exist in the \
         crate: {stale:?}"
    );
}
