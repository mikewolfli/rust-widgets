// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_font_subset.py`. Regenerate with:
//     python3 tools/gen_font_subset.py --face=cjk --license=ofl-1.1
//
// Glyph source: Noto Sans SC, subset to kana, CJK punctuation, fullwidth forms and common Han.
//     https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf
//     SHA-256 (upstream .ttf): faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9
//
// Licence of the glyph outlines: SIL Open Font License 1.1. See the repository-root `NOTICE`,
// which records this file by name together with the facts above. The subset is a Modified
// Version under the OFL and is renamed: it does not use the upstream Reserved Font Name.
//
// The subset is 361704 bytes (2053 glyphs, layout tables: GSUB, GPOS),
// covering 1391 codepoints (19 of those requested are absent from this upstream
// revision and were skipped).
// The codepoints shipped are listed in `tools/cjk_vector_codepoints.txt`, which is the input this
// generator read — the list is a decision, not a scrape.
// It is *not* compiled unless the feature that names it is enabled, and it is `include_bytes!`d
// rather than written into this source as an array: a 36 KB payload as `0x00,` literals would be
// ~150 KB of source, which is a worse artifact than the binary it describes.

/// The subset's bytes. Parsed on first use, never copied into a heap allocation.
pub const FONT: &[u8] = include_bytes!("cjk.ttf");
