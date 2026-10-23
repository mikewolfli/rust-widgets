// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Byte-index helpers shared by the text-editing controls.

use crate::compat::String;

/// Case-insensitive substring test, allocating nothing for an ASCII needle.
///
/// `needle` must already be lower-cased (callers fold the query once per filter pass, not
/// once per candidate). For an ASCII needle the comparison is byte-wise over `str::as_bytes`
/// and allocates nothing — the controls that filter lists per keystroke used to call
/// `candidate.to_lowercase().contains(&lower)`, which allocated one `String` per candidate per
/// frame while a filter was active. Non-ASCII needles keep the owned path, because folding
/// Unicode case is not a byte-wise operation.
#[cfg(not(alloc_frugal))]
pub(crate) fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !needle.is_ascii() {
        return haystack.to_lowercase().contains(needle);
    }
    let needle = needle.as_bytes();
    let haystack = haystack.as_bytes();
    needle.len() <= haystack.len()
        && haystack.windows(needle.len()).any(|window| window.eq_ignore_ascii_case(needle))
}

/// Case-insensitive prefix test, allocating nothing for an ASCII needle.
///
/// Same contract as [`contains_ignore_ascii_case`]: `needle` is already lower-cased, and a
/// non-ASCII needle takes the owned path. The command palette ranks a prefix match above a
/// substring match, and it used to build a lowered copy of every candidate to answer this.
#[cfg(not(alloc_frugal))]
pub(crate) fn starts_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !needle.is_ascii() {
        return haystack.to_lowercase().starts_with(needle);
    }
    let needle = needle.as_bytes();
    match haystack.as_bytes().get(..needle.len()) {
        Some(prefix) => prefix.eq_ignore_ascii_case(needle),
        None => false,
    }
}

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

/// The byte index of the `count`-th character in `text`, or `text.len()` when it has fewer.
///
/// # Why a character count is not a byte offset
///
/// `max_length` on a text field counts **characters** — that is what a user typing sees, and what the
/// `counter` reports — while every slice in this crate is in bytes. `text[..max]` therefore panics on
/// any multi-byte input, and `text.len() <= max` is the wrong test: a five-character CJK value is
/// fifteen bytes and was being rejected as "over a limit of five". This is the one conversion between
/// those two frames, so no control has to spell it out.
pub fn byte_index_of_char(text: &str, count: usize) -> usize {
    match text.char_indices().nth(count) {
        Some((index, _)) => index,
        None => text.len(),
    }
}

/// The byte offset of the character boundary **before** `offset`.
///
/// # Why this is shared
///
/// `LineEdit`, `TextArea`, `TextEdit` and `MentionEdit` each carried a copy, and the copies had
/// drifted: three read a caret out of a field, one took an offset, and one stepped back a *byte* and
/// floored instead of scanning the prefix (WDGT-11). Taking the offset as a parameter is what lets
/// one implementation serve all of them, including a control that holds more than one caret.
///
/// Stepping back one byte and flooring is the same answer as walking the prefix for its last
/// boundary — the byte before a boundary is always inside the previous character, or is that
/// character's own first byte — and it does not read the prefix.
pub fn previous_char_boundary(text: &str, offset: usize) -> usize {
    let boundary = floor_char_boundary(text, offset);
    if boundary == 0 {
        return 0;
    }
    floor_char_boundary(text, boundary - 1)
}

/// The byte offset of the character boundary **after** `offset`.
///
/// At or past the end this returns `text.len()`, so a caret can never step out of the value.
pub fn next_char_boundary(text: &str, offset: usize) -> usize {
    let boundary = floor_char_boundary(text, offset);
    match text[boundary..].chars().next() {
        Some(ch) => boundary + ch.len_utf8(),
        None => text.len(),
    }
}

/// Removes the selected range from `text`, returning the caret position to place.
///
/// `selection` is the ordered range a control's own `selection_range` reported. The ends are
/// floored to character boundaries again here so the helper is total even for a caller that built
/// the range from offsets it did not clamp — the same rule the three text controls had copied
/// three times (WDGT-11). Returns `None` when there is no selection; the caller still clears its
/// anchor, because an absent range and a collapsed one are the same state.
pub(crate) fn replace_selected_range(
    text: &mut String,
    selection: Option<(usize, usize)>,
) -> Option<usize> {
    let (start, end) = selection?;
    let len = text.len();
    let start = floor_char_boundary(text, start.min(len));
    let end = floor_char_boundary(text, end.min(len));
    let (start, end) = if start < end { (start, end) } else { (end, start) };
    text.replace_range(start..end, "");
    Some(start)
}

/// The first visible row after scrolling so `caret_row` is inside a `visible`-row viewport.
///
/// This is the `TextArea`/`TextEdit` scroll-follow rule, which had two copies that differed only in
/// the field names (WDGT-11). A zero-height viewport returns the current origin rather than the
/// `caret_row + 1` the arithmetic would otherwise produce, which put the caret just off screen.
pub(crate) fn first_visible_row_for(
    caret_row: usize,
    first_visible: usize,
    visible: usize,
) -> usize {
    if visible == 0 {
        return first_visible;
    }
    if caret_row < first_visible {
        caret_row
    } else if caret_row >= first_visible + visible {
        caret_row + 1 - visible
    } else {
        first_visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The `alloc_frugal` profiles have no `std` prelude, so this test's `Vec` must be named.
    use crate::compat::Vec;

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

    /// The two stepping helpers round-trip on every character of a mixed string.
    ///
    /// # What this pins
    ///
    /// Four controls used to step the caret with their own copy of this arithmetic, and one of them
    /// stepped back a byte instead of scanning. Unifying them is only safe if the shared answer is
    /// the same for every input, so this walks a string containing a 1-, 2-, 3- and 4-byte character
    /// and asserts both directions land exactly on the neighbouring boundaries — including that a
    /// caret at the end cannot step out of the value.
    #[test]
    fn character_stepping_visits_every_boundary_exactly_once() {
        let text = "aé世🎉b";
        let boundaries: Vec<usize> =
            text.char_indices().map(|(index, _)| index).chain([text.len()]).collect();
        assert_eq!(boundaries, vec![0, 1, 3, 6, 10, 11], "the fixture's boundaries");

        for pair in boundaries.windows(2) {
            assert_eq!(next_char_boundary(text, pair[0]), pair[1], "forward from {}", pair[0]);
            assert_eq!(previous_char_boundary(text, pair[1]), pair[0], "back from {}", pair[1]);
        }
        // The ends are total: no offset can step outside the value.
        assert_eq!(previous_char_boundary(text, 0), 0);
        assert_eq!(next_char_boundary(text, text.len()), text.len());
        assert_eq!(next_char_boundary(text, text.len() + 99), text.len());
        assert_eq!(previous_char_boundary(text, text.len() + 99), 10);

        // An offset inside a character rounds down before stepping, exactly as the caret placement
        // that produced it does.
        assert_eq!(previous_char_boundary(text, 2), 0, "offset 2 is inside 'é'");
        assert_eq!(next_char_boundary(text, 2), 3, "and stepping forward leaves that character");
        assert_eq!(previous_char_boundary("", 0), 0);
        assert_eq!(next_char_boundary("", 0), 0);
    }

    /// The selection-removal helper is the one implementation TextArea/TextEdit share (WDGT-11).
    #[test]
    fn removing_a_selection_returns_the_caret_and_respects_boundaries() {
        let mut text = String::from("aé世b");
        // Select "é世" (bytes 1..6), passed in reverse order as a control may report it.
        assert_eq!(replace_selected_range(&mut text, Some((6, 1))), Some(1));
        assert_eq!(text, "ab");

        let mut untouched = String::from("abc");
        assert_eq!(replace_selected_range(&mut untouched, None), None);
        assert_eq!(untouched, "abc", "no selection leaves the value alone");
    }

    /// The scroll-follow rule is shared by both multi-line controls; check its three outcomes.
    #[test]
    fn the_caret_row_is_brought_into_the_viewport() {
        assert_eq!(first_visible_row_for(2, 5, 3), 2, "above the viewport scrolls up");
        assert_eq!(first_visible_row_for(9, 5, 3), 7, "below it scrolls just enough");
        assert_eq!(first_visible_row_for(6, 5, 3), 5, "inside it leaves the origin alone");
        assert_eq!(first_visible_row_for(6, 5, 0), 5, "a zero-height viewport keeps the origin");
    }
}
