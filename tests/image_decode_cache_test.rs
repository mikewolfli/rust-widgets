// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Checks that the shared decode cache really removes the duplicate decodes it exists for.
//!
//! # Why this is a check and not a unit test
//!
//! The claim is about **three controls and a process-wide budget**, not about one function: it says
//! that an avatar and a tool button naming the same file decode it once between them, and that the
//! accounting can be read back. A unit test on the cache's own map cannot reach that — it would prove
//! the map works and say nothing about whether the controls use it.
//!
//! # Why the whole file is gated on `image`
//!
//! `rust_widgets::image` only exists with `feature = "image"` (see `src/lib.rs`), and so does every
//! function this file calls. Without the gate the crate's own module was reported missing on the
//! `ohos` cross target — `cannot find `image` in `rust_widgets`` — which failed
//! `check_harmony_cross.sh` for a test that configuration cannot run in the first place. An
//! integration test has no per-item gate to reach for, so the file states its requirement once.
#![cfg(feature = "image")]

use std::io::Write;

use rust_widgets::image::cache::{self, stats};

/// A 1x1 PNG of a solid colour, encoded by the crate's own encoder.
///
/// Three distinct colours are what a **budget** assertion needs: the cache is keyed on content, so
/// three requests for one colour are one entry and would prove nothing about eviction. Encoding
/// through `image::encode` rather than hand-assembling an IDAT is the same reasoning the existing
/// fixture records — a hand-built deflate stream that *looks* right fails in the inflater.
fn coloured_png(rgba: [u8; 4]) -> Vec<u8> {
    use rust_widgets::image::format::{DecodedImage, ImageData, ImageFormat};
    let decoded = DecodedImage::new(ImageFormat::Rgba8, ImageData::Rgba8(rgba.to_vec()), 1, 1);
    rust_widgets::image::encode(&decoded, ImageFormat::Png)
        .expect("the crate's encoder produces a PNG")
}

/// A 1x1 PNG, so the fixture goes through a real decoder.
fn png_fixture() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

#[test]
fn a_shared_icon_decodes_once_for_the_whole_toolbar() {
    let path = std::env::temp_dir().join("rw_cache_check_icon.png");
    std::fs::File::create(&path)
        .and_then(|mut file| file.write_all(&png_fixture()))
        .expect("write the fixture");

    cache::clear();

    // Twelve tool buttons on one icon, which is an ordinary toolbar.
    let before = stats();
    let mut first: Option<usize> = None;
    for _ in 0..12 {
        let pixels = cache::file_rgba8_or_none(&path).expect("the icon loads");
        // Every button must get the *same* allocation, not twelve copies of the same bytes.
        let address = std::sync::Arc::as_ptr(&pixels) as usize;
        assert_eq!(*first.get_or_insert(address), address, "one allocation for twelve buttons");
    }
    let after = stats();

    let requests = after.requests - before.requests;
    let misses = after.misses - before.misses;
    let hits = after.hits - before.hits;
    println!("twelve buttons on one icon: requests={requests} misses={misses} hits={hits}");
    assert_eq!(requests, 12, "twelve requests");
    assert_eq!(misses, 1, "one decode, not twelve");
    assert_eq!(hits, 11, "eleven saved");

    // The image-view path shares with it: a second request for the same bytes is a hit.
    let mid = stats();
    let _ = cache::pixels_rgba8_or_none(&png_fixture()).expect("the image decodes");
    let end = stats();
    println!(
        "one image-view request: requests={} misses={}",
        end.requests - mid.requests,
        end.misses - mid.misses
    );
    assert_eq!(end.hits + end.misses, end.requests, "the books must balance");
    assert!(end.bytes > 0, "the cache reports what it holds");

    // A budget of zero disables storage, and the counters must say so rather than lying.
    cache::set_budget_bytes(0);
    cache::clear();
    let before = stats();
    for _ in 0..3 {
        let _ = cache::file_rgba8_or_none(&path).expect("still decodes");
    }
    let after = stats();
    println!(
        "with a zero budget: misses={} hits={}",
        after.misses - before.misses,
        after.hits - before.hits
    );
    assert_eq!(after.misses - before.misses, 3, "a disabled cache decodes every time");
    assert_eq!(after.hits - before.hits, 0, "and claims no saves");

    // ── The sized handle shares the cache's own allocation ──
    //
    // This is the seam U2 turns on: a control that wants a *drawable* handle must not pay a copy for
    // pixels the cache already holds. `from_cached_file` and `file_rgba8_or_none` therefore have to
    // hand out **one** allocation, which `Arc::ptr_eq` decides — not "looks the same".
    cache::set_budget_bytes(cache::stats().budget_bytes.max(1 << 20));
    cache::clear();
    let unsized_pixels = cache::file_rgba8_or_none(&path).expect("the icon loads");
    let handle = rust_widgets::widget::SharedIconPixels::from_cached_file(&path)
        .expect("the sized entry point loads the same file");
    // A handle built over the *unsized* entry's allocation is the third spelling of the same lookup,
    // so all three must agree about which allocation they are talking about.
    let from_unsized = rust_widgets::widget::SharedIconPixels::from_shared_rgba8(
        std::sync::Arc::clone(&unsized_pixels),
        handle.width(),
        handle.height(),
    )
    .expect("the entry's extent matches its pixels");
    assert!(
        handle.shares_allocation_with(&from_unsized),
        "the sized handle must wrap the cache's allocation, not a copy of it"
    );
    assert_eq!((handle.width(), handle.height()), (1, 1), "the decoded extent survives the seam");

    // ── The budget is a real bound, not a number in a report ──
    //
    // One entry's worth of budget: each distinct colour must evict rather than accumulate, so the
    // cache cannot grow with the number of icons a project uses.
    let colours = [[0u8, 0, 255, 255], [0, 255, 0, 255], [255, 255, 255, 255], [10, 20, 30, 255]];
    // One 1x1 RGBA8 entry: the 4 bytes-per-pixel the cache charges for a 1x1 image. Named rather
    // than inlined so the budget below reads as "exactly one entry", not as a magic 4.
    const BYTES_PER_PIXEL: usize = 4;
    let one_entry_bytes = BYTES_PER_PIXEL;
    cache::set_budget_bytes(one_entry_bytes);
    cache::clear();
    let mut peak = 0usize;
    for colour in colours {
        let _ = cache::pixels_rgba8_or_none(&coloured_png(colour)).expect("each colour decodes");
        peak = peak.max(stats().bytes);
        assert!(
            stats().bytes <= one_entry_bytes,
            "the cache holds {} bytes with a {one_entry_bytes}-byte budget",
            stats().bytes
        );
    }
    println!("peak bytes with a one-entry budget: {peak}");
    assert!(stats().evictions > 0, "staying inside the budget must have cost evictions");

    // ── The icon seam still works when the budget refuses to store ──
    //
    // A host that would rather pay CPU than memory sets a zero budget; the handle must still be
    // produced (the whole point is that a *budget* is not a *failure*), it just cannot be shared.
    cache::set_budget_bytes(0);
    let unshared = rust_widgets::widget::SharedIconPixels::from_cached_file(&path)
        .expect("a zero budget decodes rather than fails");
    assert_eq!((unshared.width(), unshared.height()), (1, 1));

    let _ = std::fs::remove_file(&path);
    println!("decode cache checks passed.");
}
