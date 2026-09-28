// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The coverage-cache benchmark: what `face_for_char`'s cache actually saves.
//!
//! # What is under test
//!
//! `font_assets::face_for_char` sits on the **per-glyph, per-frame** path
//! (`paint_active` → `VectorSource::paint` → `face_for`). Un-cached, each call runs
//! `ttf_parser::Face::parse` over every candidate face before finding the one that covers the
//! character. The cache (`COVERAGE_CACHE_SLOTS` = 64, direct-mapped on the codepoint) is supposed
//! to remove that repeated parse. This benchmark is the measurement of "supposed to".
//!
//! # Why the comparison is `face_for_char` vs `face_for_char_uncached`
//!
//! A single absolute timing would say nothing — it would not distinguish "the cache helps" from
//! "the parse is cheap". The two functions are the *same lookup* with and without the cache, so the
//! ratio is the cache's effect and nothing else. `the_cached_and_uncached_lookups_agree`
//! (`font_assets::tests`) asserts they return identical answers, so the comparison cannot silently
//! become apples-to-oranges.
//!
//! # The two access patterns, and why both are here
//!
//! * **repeat** — one character looked up many times. This is the case the cache exists for (the
//!   same label across frames; a repeated character in a run) and where the win should be largest.
//! * **distinct** — a short run of *different* characters, cycled. A direct-mapped cache with 64
//!   slots does not collide on a caption, so this is close to repeat in effect; it is measured
//!   separately because it is the shape a real label has, and a cache that only helped the
//!   artificial single-character case would be a cache that did not help a UI.
//!
//! # Running
//!
//! ```text
//! cargo bench --no-default-features --features "desktop,fonts-vector-latin,fonts-complex,fonts-cjk" \
//!     --bench font_bench
//! ```
//!
//! With no vector face enabled there is nothing to look up: the compiled face list is empty and
//! every lookup is a miss. The benchmark still builds and runs (`face_for_char` exists whenever the
//! gate that names it does), but the numbers are the empty-list case, which is why the recommended
//! command enables the faces.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
use criterion::{criterion_group, criterion_main, Criterion};

// Gated like every benchmark below: with no vector face there is nothing to look up, so these
// imports would be unused (and a `warning` a `-D warnings` CI job would reject).
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
use rust_widgets::render::text;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
use rust_widgets::render::text::{Cell, InkKind};

/// One character, many lookups: the pattern the cache exists for.
///
/// The cached side should be dramatically cheaper than the un-cached side, because the un-cached
/// side re-parses a face per call and the cached side reads one slot.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
fn bench_repeat_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("face_for_char/repeat");
    group.bench_function("cached", |b| b.iter(|| text::face_for_char('A')));
    group.bench_function("uncached", |b| b.iter(|| text::face_for_char_uncached('A')));
    group.finish();
}

/// A short caption of *distinct* characters, cycled: the shape a real label has.
///
/// "Ag" plus a CJK ideograph when the build carries one, so a build with `fonts-cjk` measures the
/// two-face walk (Latin then CJK) rather than a single-face one.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
fn bench_caption_lookup(c: &mut Criterion) {
    let caption: Vec<char> = if text::face_for_char('\u{4E2D}').is_some() {
        vec!['H', 'e', 'l', 'l', 'o', ' ', '\u{4E2D}', '\u{6587}']
    } else {
        vec!['H', 'e', 'l', 'l', 'o', ' ', 'W', 'd']
    };

    let mut group = c.benchmark_group("face_for_char/caption");
    group.bench_function("cached", |b| {
        b.iter(|| {
            for &ch in &caption {
                std::hint::black_box(text::face_for_char(ch));
            }
        })
    });
    group.bench_function("uncached", |b| {
        b.iter(|| {
            for &ch in &caption {
                std::hint::black_box(text::face_for_char_uncached(ch));
            }
        })
    });
    group.finish();
}

/// The end-to-end path the cache sits under: rasterising one glyph's coverage.
///
/// `paint_active` is what the software and wgpu backends call per glyph per frame; it walks the
/// glyph stack, which reaches `face_for_char` through `VectorSource`. This measures the whole
/// resolve-and-paint, so the cache's contribution is shown where it actually matters rather than in
/// isolation. A CJK glyph is measured when the build carries one, because that path walks two
/// faces (the Latin face says no, the CJK face says yes).
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
fn bench_paint_active(c: &mut Criterion) {
    let cell = Cell::new(16, 16);
    let mut buffer = vec![0u8; cell.area() * 4];

    let mut group = c.benchmark_group("paint_active 16x16");
    group.bench_function("latin 'A'", |b| {
        b.iter(|| {
            let painted = text::paint_active('A', cell, &mut buffer);
            debug_assert!(painted.is_some(), "a Latin glyph must paint on a vector-face build");
        })
    });
    if text::face_for_char('\u{4E2D}').is_some() {
        group.bench_function("cjk '\u{4E2D}'", |b| {
            b.iter(|| {
                let painted = text::paint_active('\u{4E2D}', cell, &mut buffer);
                debug_assert!(painted.is_some(), "a covered ideograph must paint");
                if let Some(painted) = painted {
                    debug_assert_eq!(painted.ink, InkKind::Coverage);
                }
            })
        });
    }
    group.finish();
}

// Every benchmark is gated on `not(wasm32)` (criterion has no wasm build) and on a vector-face
// feature (there is nothing to look up without one). Grouping them together keeps the gates in one
// place.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
criterion_group!(font_benches, bench_repeat_lookup, bench_caption_lookup, bench_paint_active);

/// The entry point for configurations where the gated `criterion_main!` above is compiled out.
///
/// A bench target must have a `main` in every configuration (a crate-level `#![cfg]` would remove
/// it and make `cargo check --all-targets --target wasm32-unknown-unknown` fail with E0601). With no
/// vector face there is no lookup to measure, so the body is empty on purpose. `criterion` is a
/// host-only dev-dependency, so the wasm32 arm cannot reference it either.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
criterion_main!(font_benches);

/// The non-vector-face host entry point: a benchmark with nothing to measure still needs a `main`.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
)))]
fn main() {}
