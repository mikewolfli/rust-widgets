// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Byte-index helpers shared by the text-editing controls.

/// Rounds `index` down to the nearest UTF-8 character boundary in `text`.
///
/// # Why this is hand-written
///
/// `str::floor_char_boundary` is not stable at this crate's MSRV (1.87), so the
/// six text controls that need it each carried their own copy. One copy lives here
/// instead; the implementations were byte-identical, which is the whole argument
/// for sharing them.
///
/// # Guarantee
///
/// The returned index is always `<= index`, always `<= text.len()`, and always
/// points at a character boundary — so `&text[..floor_char_boundary(text, i)]` is
/// total for every `i`. An `index` past the end returns `text.len()`, which is
/// itself a boundary, so a cursor positioned after the last byte cannot panic.
pub fn floor_char_boundary(text: &str, index: usize) -> usize {
    let len = text.len();
    if index >= len {
        return len;
    }
    let bytes = text.as_bytes();
    let mut boundary = index;
    // UTF-8 continuation bytes match the `10xxxxxx` bit pattern; walk back over
    // them to land on the leading byte of the character the index fell inside.
    while boundary > 0 && bytes[boundary] & 0xC0 == 0x80 {
        boundary -= 1;
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_boundaries_are_identity() {
        let text = "hello";
        for i in 0..=text.len() {
            assert_eq!(floor_char_boundary(text, i), i);
        }
    }

    #[test]
    fn index_past_the_end_clamps_to_the_length() {
        assert_eq!(floor_char_boundary("hello", 5), 5);
        assert_eq!(floor_char_boundary("hello", 99), 5);
        assert_eq!(floor_char_boundary("", 0), 0);
        assert_eq!(floor_char_boundary("", 7), 0);
    }

    #[test]
    fn interior_indices_land_on_a_real_boundary() {
        // 'é' is two bytes (0xC3 0xA9); '世' is three (0xE4 0xB8 0x96).
        // Valid boundaries are therefore 0, 1, 3, 6, 7.
        let text = "aé世b";
        assert_eq!(text.len(), 7);
        // Every offset must yield a sliceable prefix.
        for i in 0..=text.len() {
            let boundary = floor_char_boundary(text, i);
            assert!(boundary <= i, "must round down, got {boundary} for {i}");
            assert!(text.is_char_boundary(boundary));
        }
        assert_eq!(floor_char_boundary(text, 2), 1); // inside 'é'
        assert_eq!(floor_char_boundary(text, 3), 3); // start of '世'
        assert_eq!(floor_char_boundary(text, 5), 3); // inside '世'
        assert_eq!(floor_char_boundary(text, 6), 6); // start of 'b'
    }

    #[test]
    fn astral_characters_are_not_split() {
        // An emoji is four bytes; every offset inside it must round to its start.
        let text = "🎉";
        assert_eq!(text.len(), 4);
        for i in 0..4 {
            assert_eq!(floor_char_boundary(text, i), 0, "offset {i} is inside the emoji");
        }
        assert_eq!(floor_char_boundary(text, 4), 4);
    }

    #[test]
    fn every_prefix_of_a_multibyte_string_is_sliceable() {
        let text = "héllo wörld 世界 🎉";
        for i in 0..=text.len() + 3 {
            let _ = &text[..floor_char_boundary(text, i)];
        }
    }
}
