// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The embedded font payloads `rust_widgets` ships behind its `fonts-*` features.
//!
//! # Why the payloads are in a crate of their own
//!
//! crates.io caps a package at 10 MiB compressed, and the font payloads are the one part of the
//! library that is both large (2.4 MB) and already compressed — outlines and PNG bitmaps, which
//! gzip cannot shrink. 2.9.3 published at 10,357,752 bytes, 127 KB under the ceiling; the next
//! round of code and tests took 2.9.4 to 10,918,287 and crates.io answered HTTP 413.
//!
//! Dropping them from `rust_widgets`' `include` list is not a fix: they are `include_bytes!`d by
//! the default profiles, so the packaged crate would fail its own verification build with
//! `couldn't read …ttf`. Keeping them beside the code means code and data compete for the same
//! 10 MiB forever. Here, the library package excludes them (`"!crates/**/*"`), and a consumer who
//! names a `fonts-*` feature pulls this crate in as a dependency.
//!
//! # What this crate is not
//!
//! There is no font API here — no parsing, no shaping, no fallback. Each module is one
//! `pub const FONT: &[u8]` holding the bytes, exactly as the generated wrapper in `rust_widgets`
//! held them before the split. The licence record (upstream project, URL, SHA-256, generator)
//! stays in each module's header, where it always was, and this crate's `NOTICE` repeats it.
//!
//! # Why one feature per face
//!
//! `rust_widgets` maps `fonts-vector-latin` → `rust-widgets-font/latin`, `fonts-cjk` →
//! `.../cjk`, `fonts-emoji-color` → `.../emoji`, and each `fonts-cjk-shard-*` to its shard. The
//! mapping is one-to-one, so a consumer that asks for one face compiles one face — and a mapping
//! mistake is a compile error rather than a silently empty font.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "arabic")]
pub mod arabic;
#[cfg(feature = "cjk")]
pub mod cjk;
#[cfg(feature = "cjk-shard-fullwidth")]
pub mod cjk_shard_fullwidth;
#[cfg(feature = "cjk-shard-han")]
pub mod cjk_shard_han;
#[cfg(feature = "cjk-shard-kana")]
pub mod cjk_shard_kana;
#[cfg(feature = "cjk-shard-latin")]
pub mod cjk_shard_latin;
#[cfg(feature = "cjk-shard-symbols")]
pub mod cjk_shard_symbols;
#[cfg(feature = "emoji")]
pub mod emoji;
#[cfg(feature = "latin")]
pub mod latin;
