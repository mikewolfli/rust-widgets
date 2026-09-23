// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bidirectional reordering — the **visual** order of one line of text (UAX #9).
//!
//! # Why a line needs reordering at all
//!
//! A bidirectional algorithm line has two orders and they are usually different:
//!
//! * **logical** — the order the characters are stored in, which is the order a cursor, a
//!   copy, or a byte offset works in;
//! * **visual** — the order they are painted in, which in a right-to-left run is the reverse.
//!
//! The crate's layout walks text in logical order and advances a pen left to right. For a
//! left-to-right line the two orders coincide, which is why nothing here changed any existing
//! output. For an Arabic or Hebrew line they do not, and painting in logical order draws the
//! word backwards — *the* defect this module exists to remove. Its severity is what makes it
//! worth a dependency: `نص` drawn mirror-image is not a missing glyph, it is a wrong word.
//!
//! # Where this sits
//!
//! Reordering is **logic, not font data** (the two axes the text layer keeps apart): it needs
//! no glyph, no outline and no face, only the characters' bidi classes. So it is compiled into
//! every profile, including `mini`, and it answers the one question the renderer asks —
//! *"in what order do I paint the clusters I already built?"*
//!
//! [`crate::core::TextDirection`] remains the type a *control* uses for its own geometry
//! ("which end of my track is the beginning"), and it stays a two-state type. A *line's* base
//! direction is a property of the line's own content, which a control holding a label cannot
//! know in advance, so it is resolved from the text here (UAX #9 P2/P3) rather than passed in.

use crate::compat::Vec;

/// The cluster order to paint, given the clusters' byte ranges in logical order.
///
/// `clusters` is one `(start, end)` byte range per cluster, in logical order and partitioning
/// a contiguous span of `text`. The returned vector holds **indices into `clusters`**, in the
/// order they must be painted. It is always a permutation of `0..clusters.len()`.
///
/// # Why clusters and not characters
///
/// Reordering characters would tear a grapheme in half: an Arabic letter and its combining
/// mark share one base direction, and reversing them separately paints the mark before the
/// letter it belongs to. Reordering the clusters keeps a grapheme a single unit — the same
/// unit the shaper built and the same unit the renderer draws.
pub fn visual_order(text: &str, clusters: &[(usize, usize)]) -> Vec<usize> {
    reorder(text, clusters)
}

#[cfg(feature = "text-bidi")]
fn reorder(text: &str, clusters: &[(usize, usize)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..clusters.len()).collect();
    if clusters.len() <= 1 {
        return order;
    }
    // `None` leaves the paragraph's base direction to be resolved from its first strong
    // character, which is what a label needs: the same control draws `Sign in` left-to-right
    // and `تسجيل الدخول` right-to-left without the caller having to say which.
    let info = unicode_bidi::BidiInfo::new(text, None);
    // One level per cluster. Every character of a grapheme takes its base's level, so the
    // level at the cluster's first byte is the cluster's level.
    let levels: Vec<u8> = clusters
        .iter()
        .map(|&(start, _)| info.levels.get(start).map_or(0, |l| l.number()))
        .collect();

    let highest = levels.iter().copied().max().unwrap_or(0);
    // No odd level anywhere means no right-to-left run: the line is already in visual order.
    // This is the branch every existing snapshot takes, and the reason reordering is free for
    // them — not "close enough", but the identity permutation.
    let Some(lowest_odd) = levels.iter().copied().filter(|level| level % 2 == 1).min() else {
        return order;
    };

    // UAX #9 L2, applied to the permutation instead of to the string: from the highest level
    // present down to the lowest *odd* level, reverse every contiguous span at or above that
    // level. Doing it on the index array is what lets the result stay a permutation of whole
    // clusters.
    for level in (lowest_odd..=highest).rev() {
        let mut i = 0;
        while i < levels.len() {
            if levels[i] < level {
                i += 1;
                continue;
            }
            let start = i;
            while i < levels.len() && levels[i] >= level {
                i += 1;
            }
            order[start..i].reverse();
        }
    }
    order
}

/// Without a compiled-in bidi engine a line is painted in logical order.
///
/// That is exactly right for a left-to-right line, and the crate's documented limitation for a
/// right-to-left one — stated rather than silently different, which is the property this
/// fallback exists to keep (compare [`crate::render::text`]'s note on font data being opt-in).
#[cfg(not(feature = "text-bidi"))]
fn reorder(text: &str, clusters: &[(usize, usize)]) -> Vec<usize> {
    let _ = text;
    (0..clusters.len()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::String;

    /// The byte ranges of a string's `char`s, which is all a cluster list is for a test that
    /// does not care about grapheme merging.
    fn per_char(text: &str) -> Vec<(usize, usize)> {
        text.char_indices().map(|(i, c)| (i, i + c.len_utf8())).collect()
    }

    /// Reorder and read the result back as a string, for legibility of the expectations.
    fn visual_string(text: &str) -> String {
        let ranges = per_char(text);
        let order = visual_order(text, &ranges);
        order.iter().map(|&i| &text[ranges[i].0..ranges[i].1]).collect()
    }

    #[test]
    fn a_left_to_right_line_is_untouched() {
        // The property every existing snapshot depends on: Latin text is the identity.
        assert_eq!(visual_string("Hello, world"), "Hello, world");
        assert_eq!(visual_string(""), "");
        assert_eq!(visual_string("a"), "a");
    }

    #[test]
    fn the_order_is_always_a_permutation() {
        // A reordering that dropped or duplicated a cluster would be far worse than a wrong
        // order, so it is asserted for several shapes rather than assumed from the algorithm.
        for text in ["Hello", "עברית", "abc עברית def", "مرحبا بالعالم", "123", "a1ב2c3"]
        {
            let ranges = per_char(text);
            let mut order = visual_order(text, &ranges);
            order.sort_unstable();
            assert_eq!(order, (0..ranges.len()).collect::<Vec<_>>(), "{text}");
        }
    }

    #[test]
    fn a_hebrew_run_is_reversed() {
        // `שלום` is stored logical-first, so the visual line is its reverse. Painting logical
        // order here is the defect: the word reads backwards on screen.
        let text = "\u{05e9}\u{05dc}\u{05d5}\u{05dd}";
        assert_eq!(
            visual_string(text),
            "\u{05dd}\u{05d5}\u{05dc}\u{05e9}",
            "an all-RTL line must be painted right to left"
        );
    }

    #[test]
    fn a_latin_embedding_keeps_the_latin_run_readable() {
        // A mixed line is where a naive reverse fails: the Latin word inside an RTL paragraph
        // must stay left-to-right internally while its *position* in the line is mirrored.
        let text = "\u{05d0}\u{05d1}\u{05d2} abc";
        let visual = visual_string(text);
        assert!(visual.contains("abc"), "the embedded run must not itself reverse: {visual}");
        assert!(
            visual.starts_with("abc"),
            "and it moves to the visual start of an RTL line: {visual}"
        );
    }

    #[test]
    fn combining_marks_travel_with_their_base() {
        // The reason clusters, not characters, are reordered: `a` + U+0301 is one cluster, and
        // it must move as one. Reordering the two scalars separately would put the accent first.
        let text = "a\u{0301}b";
        let ranges = vec![(0, 3), (3, 4)];
        assert_eq!(visual_order(text, &ranges), vec![0, 1], "an LTR line stays in order");
    }
}
