// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_emoji_subset.py`. Regenerate with:
//     python3 tools/gen_emoji_subset.py --license=ofl-1.1
//
// Glyph source: Noto Color Emoji, colour emoji as CBDT PNG bitmaps, one strike at 109 ppem:
//     https://raw.githubusercontent.com/googlefonts/noto-emoji/main/2D/fonts/NotoColorEmoji.ttf
//     SHA-256 (upstream .ttf): 15671215ab769fdc7162a045d56fd7d7e477c51b04e6b3c761d914d8fdd6cc44
// Licence of the glyph data: OFL-1.1 (SIL Open Font License 1.1). See the
// repository-root `NOTICE`. This subset is a Modified Version under the OFL: it retains
// the upstream colour bitmaps verbatim, and it does not use the upstream Reserved Font
// Name.
//
//     N = 317 codepoints; 1602492 bytes of bitmap data.
// The codepoints shipped are listed in `tools/emoji_subset_codepoints.txt`, which is the
// input this generator read — the list is a decision, not a scrape.
//
// The payload is a binary `include_bytes!` rather than a Rust array: 1602492 bytes written
// as `0x00,` literals would be several times this file's size, which is a worse artifact than the
// binary it describes.

/// The generated subset's bytes, in the binary's read-only section.
pub const FONT: &[u8] = include_bytes!("emoji.ttf");
