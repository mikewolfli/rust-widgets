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
    let _ = rust_widgets::image::Image::from_bytes_rgba8_shared(&png_fixture())
        .expect("the image decodes");
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

    let _ = std::fs::remove_file(&path);
    println!("decode cache checks passed.");
}
