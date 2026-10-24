// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The generated vector faces, one module per opt-in data feature.
//!
//! # Why each face is a module rather than a constant
//!
//! The payload is a binary `include_bytes!` rather than a Rust array: 36 KB written as `0x00,`
//! literals would be ~150 KB of source, which is a worse artifact than the binary it describes.
//! Each module's header records where the bytes came from and under which licence — see the
//! repository-root `NOTICE`, which `tools/check_font_licenses.sh` requires this to agree with.
//!
//! # Why nothing here is named after the upstream font
//!
//! OFL's Reserved Font Name clause applies to a Modified Version, and a subset is one. The
//! upstream names live on as `covers` strings (a font *faces* an application by its family
//! name, which is a different thing), never as this crate's public identifiers.

// Test-only variable faces: a `wght` face and a `wdth`-only face, built for the variable-font
// tests so they can assert exact geometry. Compiled only under `test`, so no shipped binary
// carries them; they live here because every embedded font payload has to have its home under
// this directory (see `tools/check_font_data_is_opt_in.sh`).
#[cfg(all(test, feature = "text-shaping"))]
pub(crate) mod variable_fixtures;

#[cfg(feature = "fonts-vector-latin")]
mod latin;
#[cfg(feature = "fonts-vector-latin")]
pub use latin::FONT as LATIN;

#[cfg(feature = "fonts-complex")]
mod arabic;
#[cfg(feature = "fonts-complex")]
pub use arabic::FONT as ARABIC;

// The scalable CJK face (G-4c). Distinct from `fonts-cjk-bitmap`: this one carries **outlines**, so
// it needs the rasteriser and can be drawn at any px size, at the cost of ~7x the bytes for the
// same coverage. See `tools/cjk_vector_codepoints.txt` for why its coverage is narrower.
#[cfg(feature = "fonts-cjk")]
#[cfg_attr(docsrs, doc(cfg(feature = "fonts-cjk")))]
mod cjk;
#[cfg(feature = "fonts-cjk")]
pub use cjk::FONT as CJK;

// The sharded CJK vector face (`fonts-cjk-shards`). Each shard is one `include_bytes!` payload
// behind its own feature; the generated module names the shard a character lives in so a host can
// load one script rather than all of CJK. See `tools/cjk_vector_shards.txt`.
#[cfg(any(
    feature = "fonts-cjk-shard-latin",
    feature = "fonts-cjk-shard-symbols",
    feature = "fonts-cjk-shard-kana",
    feature = "fonts-cjk-shard-fullwidth",
    feature = "fonts-cjk-shard-han"
))]
/// The sharded CJK vector face: the `fonts-cjk` coverage partitioned on Unicode block boundaries.
///
/// Each shard is one `include_bytes!` payload behind its own `fonts-cjk-shard-<id>` feature, and
/// [`cjk_shards::shard_for`] names the shard a character lives in. Use
/// [`cjk_shard_loader::load_shard_for`] to make a shard resident on demand — that is the point of
/// splitting, and it needs the `runtime-fonts` feature to have somewhere to register the bytes.
#[cfg_attr(
    docsrs,
    doc(cfg(any(
        feature = "fonts-cjk-shard-latin",
        feature = "fonts-cjk-shard-symbols",
        feature = "fonts-cjk-shard-kana",
        feature = "fonts-cjk-shard-fullwidth",
        feature = "fonts-cjk-shard-han"
    )))
)]
pub mod cjk_shards;

// Loading a shard on demand, into the runtime face list. Needs both a shard to load and the
// registry to load it into; without `runtime-fonts` a shard would have to be compiled into
// `active_faces`, which is the behaviour the shards exist to avoid.
#[cfg(all(
    feature = "runtime-fonts",
    any(
        feature = "fonts-cjk-shard-latin",
        feature = "fonts-cjk-shard-symbols",
        feature = "fonts-cjk-shard-kana",
        feature = "fonts-cjk-shard-fullwidth",
        feature = "fonts-cjk-shard-han"
    )
))]
#[cfg_attr(
    docsrs,
    doc(cfg(all(
        feature = "runtime-fonts",
        any(
            feature = "fonts-cjk-shard-latin",
            feature = "fonts-cjk-shard-symbols",
            feature = "fonts-cjk-shard-kana",
            feature = "fonts-cjk-shard-fullwidth",
            feature = "fonts-cjk-shard-han"
        )
    )))
)]
pub mod cjk_shard_loader;

#[cfg(feature = "fonts-emoji-color")]
mod emoji;
#[cfg(feature = "fonts-emoji-color")]
pub use emoji::FONT as EMOJI;

mod assets;
pub use assets::*;
// `invalidate_face_cache` is `pub(crate)` and exists only where runtime faces do.
#[cfg(feature = "runtime-fonts")]
pub(crate) use assets::invalidate_face_cache;

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache is an accelerator, not an authority: a hit must equal the un-cached answer.
    ///
    /// # Why this is asserted rather than assumed
    ///
    /// The benchmark in `benches/font_bench.rs` measures `face_for_char` against
    /// `face_for_char_uncached`; that comparison is only meaningful if the two are the *same
    /// function* modulo the cache. A cache that returned a different face — from a stale index, a
    /// generation bug, or a bad slot mapping — would make the benchmark compare apples to oranges
    /// while every timing looked plausible. This is the assertion that keeps the control honest.
    #[cfg(any(
        feature = "text-shaping",
        feature = "fonts-vector-latin",
        feature = "fonts-complex",
        cjk_outline_face
    ))]
    #[test]
    fn the_cached_and_uncached_lookups_agree() {
        // The registry is global: hold the tests' registry lock so a parallel registration cannot
        // land between the cached and the un-cached lookup.
        #[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
        let _guard = crate::render::text::runtime_fonts::runtime_fonts_test_guard();
        // A spread that exercises: a covered ASCII letter, an uncovered private-use codepoint
        // (cached as "no face"), whitespace, and — when the build carries them — CJK and an
        // emoji, so both the compiled-list and the shape of the answer are covered.
        let probes = ['A', 'z', '\u{0}', ' ', '\u{E000}', '\u{4E2D}', '\u{3042}', '\u{1F600}'];
        for ch in probes {
            let cached = face_for_char(ch);
            let uncached = face_for_char_uncached(ch);
            assert_eq!(
                cached.map(|face| face.name),
                uncached.map(|face| face.name),
                "cache and un-cached lookup disagree for U+{:04X} ({ch:?})",
                ch as u32
            );
        }
    }

    /// A repeated lookup returns the first answer, and a build with no face answers `None`.
    ///
    /// The second half is the honest-absence contract: `face_for_char` on a build with an empty
    /// compiled list and no registered face must say "no face", not fabricate one.
    #[cfg(any(
        feature = "text-shaping",
        feature = "fonts-vector-latin",
        feature = "fonts-complex",
        cjk_outline_face
    ))]
    #[test]
    fn a_repeated_lookup_is_stable() {
        #[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
        let _guard = crate::render::text::runtime_fonts::runtime_fonts_test_guard();
        let first = face_for_char('A');
        for _ in 0..100 {
            assert_eq!(
                face_for_char('A').map(|face| face.name),
                first.map(|face| face.name),
                "the cached answer must not change across repeated lookups"
            );
        }
    }
}
