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
    let levels = levels_for(text, clusters);
    visual_order_from_levels(&levels).unwrap_or_else(|| (0..clusters.len()).collect())
}

/// The bidi level of each cluster, in logical order.
///
/// The UAX #9 paragraph analysis is kept separate from the permutation so a caller can memoise it:
/// [`crate::render::text::shaping`] stores it with the shaped run and hands the same buffer back on
/// a repeat (G-17). Without the `text-bidi` feature every level is `0`, which is the documented
/// left-to-right fallback.
pub fn levels_for(text: &str, clusters: &[(usize, usize)]) -> Vec<u8> {
    #[cfg(feature = "text-bidi")]
    {
        // One cluster cannot produce a run to reverse, and skipping the analysis keeps the
        // single-character label on the cheap path.
        if clusters.len() <= 1 {
            let levels = vec![0; clusters.len()];
            return levels;
        }
        // `None` leaves the paragraph's base direction to be resolved from its first strong
        // character, which is what a label needs: the same control draws `Sign in` left-to-right
        // and `تسجيل الدخول` right-to-left without the caller having to say which.
        let info = unicode_bidi::BidiInfo::new(text, None);
        // One level per cluster. Every character of a grapheme takes its base's level, so the
        // level at the cluster's first byte is the cluster's level.
        clusters
            .iter()
            .map(|&(start, _)| info.levels.get(start).map_or(0, |level| level.number()))
            .collect()
    }
    #[cfg(not(feature = "text-bidi"))]
    {
        let _ = text;
        let mut levels = Vec::new();
        levels.resize(clusters.len(), 0u8);
        levels
    }
}

/// The visual permutation implied by already-computed bidi levels, or `None` for the identity.
///
/// `None` is the **zero-allocation** fast path: an all-even level list means no right-to-left run,
/// so the line is already in paint order and no permutation vector is built. This is the branch
/// every left-to-right line takes, and the reason reordering is free for them — not "close
/// enough", but the identity (G-17).
///
/// `Some(order)` is always a permutation of `0..levels.len()`.
pub fn visual_order_from_levels(levels: &[u8]) -> Option<Vec<usize>> {
    let highest = levels.iter().copied().max().unwrap_or(0);
    let lowest_odd = levels.iter().copied().filter(|level| level % 2 == 1).min()?;
    let mut order: Vec<usize> = (0..levels.len()).collect();

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
    Some(order)
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
    fn an_all_even_level_list_needs_no_permutation() {
        // The cached-LTR fast path (G-17): `None` rather than an allocated identity vector.
        assert_eq!(visual_order_from_levels(&[0, 0, 0]), None);
        assert_eq!(visual_order_from_levels(&[]), None);
        // An odd level still reorders, so the fast path does not swallow a real RTL run.
        assert_eq!(visual_order_from_levels(&[1, 1]), Some(vec![1, 0]));
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

    // ── B26.11 regression cases: Arabic/Hebrew, mixed runs, long strings, missing faces ──────

    /// An all-Arabic line must be painted right to left.
    ///
    /// The mirror of the Hebrew case: Arabic is the other script the plan names, and a line that
    /// reversed for Hebrew but not Arabic would be a script-specific bug the Hebrew test alone
    /// cannot catch.
    #[test]
    fn an_arabic_run_is_reversed() {
        // `مرحبا` (marhaba), logical-first; visual order is the reverse.
        let text = "\u{0645}\u{0631}\u{062d}\u{0628}\u{0627}";
        let reversed: String = text.chars().rev().collect();
        assert_eq!(visual_string(text), reversed, "an all-RTL Arabic line must be reversed");
    }

    /// A mixed Arabic + Latin + digit line must keep each run internally readable while placing
    /// the numbers where the algorithm says.
    ///
    /// This is the case a naive reverse gets wrong: the digits and the Latin word inside an RTL
    /// paragraph must not themselves reverse.
    #[test]
    fn a_mixed_arabic_latin_digit_line_keeps_each_run_readable() {
        let text = "\u{0645}\u{0631}\u{062d}\u{0628}\u{0627} abc 123";
        let visual = visual_string(text);
        assert!(visual.contains("abc"), "the Latin run must stay readable: {visual}");
        assert!(visual.contains("123"), "the digits must stay in order: {visual}");
        // The digits are a left-to-right run, so they must not read as `321`.
        assert!(!visual.contains("321"), "digits must not reverse: {visual}");
        // And the whole line is still a permutation of the input.
        let mut a: Vec<char> = visual.chars().collect();
        let mut b: Vec<char> = text.chars().collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "reordering must not drop or duplicate a cluster");
    }

    /// A long mixed-script line must still be a permutation and stay internally consistent.
    ///
    /// Long strings are where an off-by-one in the run scan appears; the single-word cases above
    /// would not catch it.
    #[test]
    fn a_long_mixed_line_is_a_permutation() {
        let text = "The quick \u{05e9}\u{05dc}\u{05d5}\u{05dd} brown fox \u{0645}\u{0631}\u{062d}\u{0628}\u{0627} jumps 42 over 7 lazy dogs \u{05d0}\u{05d1}\u{05d2}";
        let ranges = per_char(text);
        let mut order = visual_order(text, &ranges);
        order.sort_unstable();
        assert_eq!(
            order,
            (0..ranges.len()).collect::<Vec<_>>(),
            "a long mixed line must reorder to a permutation"
        );
    }

    /// A line of characters no compiled-in face covers must still reorder without panicking and
    /// without changing the text — a missing face is a *glyph* problem, not an ordering one.
    ///
    /// This is the "字体缺失" half of B26.11: the reorder works from bidi *classes*, not from
    /// glyphs (see the module doc), so the absence of a face must not affect it. Cuneiform is
    /// deliberately a script no bundled face covers.
    #[test]
    fn a_line_with_no_available_face_still_reorders_as_a_permutation() {
        // U+12000 CUNEIFORM SIGN A, U+12001 CUNEIFORM SIGN AB — no bundled face covers these.
        let text = "\u{12000}\u{12001}\u{12002}";
        let ranges = per_char(text);
        let mut order = visual_order(text, &ranges);
        order.sort_unstable();
        assert_eq!(
            order,
            (0..ranges.len()).collect::<Vec<_>>(),
            "a face-less line must still be a valid permutation"
        );
    }

    /// The reorder must be independent of how the line was **wrapped**: a bidi line split into two
    /// visual rows must reorder each row the same way it reorders the whole.
    ///
    /// This is the "换行" half of B26.11. Wrapping decides *which* characters are on a row; the
    /// reorder decides their visual order within it. A row therefore reorders to a permutation of
    /// its own characters, and the concatenation of the rows is a permutation of the line — the
    /// property that keeps a wrapped RTL paragraph readable.
    #[test]
    fn each_wrapped_row_reorders_independently() {
        // A two-word RTL line split at the space, as a wrapper would split it.
        let first = "\u{05e9}\u{05dc}\u{05d5}\u{05dd}"; // shalom
        let second = "\u{05e2}\u{05d5}\u{05dc}\u{05dd}"; // olam
        for row in [first, second] {
            let ranges = per_char(row);
            let mut order = visual_order(row, &ranges);
            order.sort_unstable();
            assert_eq!(
                order,
                (0..ranges.len()).collect::<Vec<_>>(),
                "each wrapped row must reorder to a permutation of itself"
            );
            // The row is reversed (all RTL), the same as it would be un-wrapped.
            let reversed: String = row.chars().rev().collect();
            assert_eq!(
                visual_string(row),
                reversed,
                "the row's own order must not depend on the split"
            );
        }
    }
}
