// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_cjk_shards.py`. Regenerate with:
//     python3 tools/gen_cjk_shards.py
//
// The CJK vector face, partitioned on Unicode block boundaries. Each shard is one
// `include_bytes!` payload behind its own `fonts-cjk-shard-<id>` feature, so a host that
// draws one script loads one script's glyphs. `shard_for` names the file a character lives
// in; `active_shards` lists the shards this build carries.
//
// Glyph source: Noto Sans SC, by the Noto project (Adobe and Google). The shards are a
// partition of the `fonts-cjk` face's coverage, cut from the same upstream revision.
//     https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf
//     SHA-256 (upstream .otf): faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9
//
// Licence of the glyph outlines: SIL Open Font License, Version 1.1. The repository-root
// `NOTICE` records this file by name together with the facts above.
//
// The union of the shards equals `tools/cjk_vector_codepoints.txt` (the `fonts-cjk` face's
// coverage), asserted by `tools/check_cjk_shards_cover_the_face.sh`.

#[cfg(feature = "fonts-cjk-shard-latin")]
#[path = "cjk_shard_latin.rs"]
mod cjk_shard_latin;
#[cfg(feature = "fonts-cjk-shard-latin")]
pub use cjk_shard_latin::FONT as LATIN;

#[cfg(feature = "fonts-cjk-shard-symbols")]
#[path = "cjk_shard_symbols.rs"]
mod cjk_shard_symbols;
#[cfg(feature = "fonts-cjk-shard-symbols")]
pub use cjk_shard_symbols::FONT as SYMBOLS;

#[cfg(feature = "fonts-cjk-shard-kana")]
#[path = "cjk_shard_kana.rs"]
mod cjk_shard_kana;
#[cfg(feature = "fonts-cjk-shard-kana")]
pub use cjk_shard_kana::FONT as KANA;

#[cfg(feature = "fonts-cjk-shard-han")]
#[path = "cjk_shard_han.rs"]
mod cjk_shard_han;
#[cfg(feature = "fonts-cjk-shard-han")]
pub use cjk_shard_han::FONT as HAN;

#[cfg(feature = "fonts-cjk-shard-fullwidth")]
#[path = "cjk_shard_fullwidth.rs"]
mod cjk_shard_fullwidth;
#[cfg(feature = "fonts-cjk-shard-fullwidth")]
pub use cjk_shard_fullwidth::FONT as FULLWIDTH;

/// One shard: its id, its bytes (when this build carries it), and its coverage.
pub struct Shard {
    /// The shard id, as spelled in `tools/cjk_vector_shards.txt`.
    pub id: &'static str,
    /// The family name a diagnostic prints for this shard.
    pub name: &'static str,
    /// The shard's bytes, or an empty slice when no feature carries it.
    pub bytes: &'static [u8],
    /// The range of codepoints this shard covers, inclusive at both ends.
    pub range: (u32, u32),
}

/// Every shard, in file order, with the bytes of the ones this build carries.
///
/// A shard whose feature is off has an empty `bytes` and is skipped by the loaders, so the
/// table's shape does not depend on the feature set — a test can enumerate every shard on
/// any build and assert which ones are resident.
pub const SHARDS: &[Shard] = &[
    Shard {
        id: "latin",
        name: "Noto Sans SC (latin)",
        range: (0x0020, 0x007E),
        #[cfg(feature = "fonts-cjk-shard-latin")]
        bytes: LATIN,
        #[cfg(not(feature = "fonts-cjk-shard-latin"))]
        bytes: &[],
    },
    Shard {
        id: "symbols",
        name: "Noto Sans SC (symbols)",
        range: (0x3000, 0x303F),
        #[cfg(feature = "fonts-cjk-shard-symbols")]
        bytes: SYMBOLS,
        #[cfg(not(feature = "fonts-cjk-shard-symbols"))]
        bytes: &[],
    },
    Shard {
        id: "kana",
        name: "Noto Sans SC (kana)",
        range: (0x3040, 0x30FF),
        #[cfg(feature = "fonts-cjk-shard-kana")]
        bytes: KANA,
        #[cfg(not(feature = "fonts-cjk-shard-kana"))]
        bytes: &[],
    },
    Shard {
        id: "han",
        name: "Noto Sans SC (han)",
        range: (0x4E00, 0x511F),
        #[cfg(feature = "fonts-cjk-shard-han")]
        bytes: HAN,
        #[cfg(not(feature = "fonts-cjk-shard-han"))]
        bytes: &[],
    },
    Shard {
        id: "fullwidth",
        name: "Noto Sans SC (fullwidth)",
        range: (0xFF00, 0xFFEF),
        #[cfg(feature = "fonts-cjk-shard-fullwidth")]
        bytes: FULLWIDTH,
        #[cfg(not(feature = "fonts-cjk-shard-fullwidth"))]
        bytes: &[],
    },
];

/// The shard that covers `ch`, or `None` when no shard's range contains it.
///
/// A range test rather than a glyph-table probe: the ranges partition the coverage, so a
/// character is in at most one shard and the answer is a comparison, not a parse. Whether
/// this **build** carries that shard is a separate question — see `Shard::bytes`.
pub fn shard_for(ch: char) -> Option<&'static Shard> {
    let codepoint = ch as u32;
    SHARDS.iter().find(|shard| {
        let (low, high) = shard.range;
        codepoint >= low && codepoint <= high
    })
}

/// The shards this build carries (a non-empty `bytes`), in file order.
pub fn active_shards() -> impl Iterator<Item = &'static Shard> {
    SHARDS.iter().filter(|shard| !shard.bytes.is_empty())
}

/// The total bytes this build's shards weigh. 411180 bytes when every shard is on.
pub const fn total_bytes() -> usize {
    let mut total = 0;
    let mut index = 0;
    while index < SHARDS.len() {
        total += SHARDS[index].bytes.len();
        index += 1;
    }
    total
}
