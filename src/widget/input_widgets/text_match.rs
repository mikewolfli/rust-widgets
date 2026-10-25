// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Allocation-free case-fold comparisons shared by the input family (I-P05).
//!
//! # Why this exists
//!
//! The combo-box family compared a typed prefix against every row by lower-casing both sides:
//! `candidate.to_lowercase().starts_with(&needle)`, once per candidate per keystroke, while a
//! filter was active and a popup was open. That is one `String` allocation per row per frame for
//! a question that is a byte-wise comparison for ASCII — which is every row in the ordinary
//! case. `crate::widget::text_utils` already carries the substring and prefix forms; the suffix
//! and equality forms below are the two it lacks, and all four live together here so the family
//! has one vocabulary rather than five local copies.
//!
//! # Contract
//!
//! `needle` is folded **once per keystroke** with [`fold`] and then compared against every
//! candidate. For an ASCII needle the comparison is byte-wise and allocates nothing. A
//! non-ASCII needle takes the owned path, because case folding Unicode is not a byte
//! operation; that path is the one the previous code paid for every candidate.

use crate::compat::String;

/// Folds a typed query to the form every comparison below expects.
///
/// Callers fold once per keystroke, not once per candidate.
pub(crate) fn fold(needle: &str) -> String {
    needle.to_lowercase()
}

/// How long a type-ahead prefix stays alive after its last keystroke, in milliseconds.
///
/// The combo-box family's first-letter search accumulates characters until a row matches; the
/// buffer used to live until the control was cleared by some unrelated event, so a stale prefix
/// from seconds ago kept intercepting the first letter of the next search. 800 ms is long enough
/// to type a short prefix and short enough that a new search starts clean (I-P06). The constant
/// lives here so every control in the family uses the same number rather than four copies.
pub(crate) const TYPE_AHEAD_TIMEOUT_MS: u64 = 800;

/// Whether `haystack` contains `needle`, ignoring ASCII case.
pub(crate) fn contains(haystack: &str, needle: &str) -> bool {
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

/// Whether `haystack` starts with `needle`, ignoring ASCII case.
pub(crate) fn starts_with(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !needle.is_ascii() {
        return haystack.to_lowercase().starts_with(needle);
    }
    match haystack.as_bytes().get(..needle.len()) {
        Some(prefix) => prefix.eq_ignore_ascii_case(needle.as_bytes()),
        None => false,
    }
}

/// Whether `haystack` ends with `needle`, ignoring ASCII case.
#[cfg(widgets_unstripped)]
pub(crate) fn ends_with(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !needle.is_ascii() {
        return haystack.to_lowercase().ends_with(needle);
    }
    let haystack = haystack.as_bytes();
    match haystack.len().checked_sub(needle.len()) {
        Some(start) => haystack[start..].eq_ignore_ascii_case(needle.as_bytes()),
        None => false,
    }
}

/// Whether the two strings are equal, ignoring ASCII case.
#[cfg(widgets_unstripped)]
pub(crate) fn eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    if !left.is_ascii() || !right.is_ascii() {
        return left.to_lowercase() == right.to_lowercase();
    }
    left.as_bytes().eq_ignore_ascii_case(right.as_bytes())
}

/// Orders two strings case-insensitively, for an alphabetical insert position (I-P09).
///
/// The ASCII path walks both byte strings and compares folded bytes, so it allocates nothing;
/// a non-ASCII pair falls back to the owned lowercase comparison, which is the only correct way
/// to order those cases.
pub(crate) fn cmp_ignore_ascii_case(left: &str, right: &str) -> core::cmp::Ordering {
    if !left.is_ascii() || !right.is_ascii() {
        return left.to_lowercase().cmp(&right.to_lowercase());
    }
    let left = left.as_bytes();
    let right = right.as_bytes();
    for (left_byte, right_byte) in left.iter().zip(right.iter()) {
        match left_byte.to_ascii_lowercase().cmp(&right_byte.to_ascii_lowercase()) {
            core::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    left.len().cmp(&right.len())
}

/// Whether `text` contains `needle` as a whole word, ignoring ASCII case.
///
/// A "word" is a run of alphanumeric characters, which is the split the completion filter has
/// always used.
#[cfg(widgets_unstripped)]
pub(crate) fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.split(|ch: char| !ch.is_alphanumeric()).any(|word| eq(word, needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(widgets_unstripped)]
    #[test]
    fn ascii_comparisons_ignore_case_without_allocating() {
        assert!(contains("Hello World", &fold("WORLD")));
        assert!(starts_with("Hello", &fold("he")));
        assert!(ends_with("Hello", &fold("LO")));
        assert!(eq("ABC", &fold("abc")));
        assert!(!eq("ABC", &fold("abd")));
        assert!(!ends_with("Hello", &fold("hell")));
        assert!(contains_word("one two three", &fold("TWO")));
        assert!(!contains_word("one two three", &fold("tw")));
    }

    #[cfg(widgets_unstripped)]
    #[test]
    fn a_non_ascii_needle_still_matches() {
        assert!(contains("Straße", &fold("ß")));
        assert!(starts_with("Éclair", &fold("é")));
        assert!(eq("Éclair", &fold("éclair")));
    }

    #[cfg(widgets_unstripped)]
    #[test]
    fn an_empty_needle_matches_everything() {
        assert!(contains("anything", ""));
        assert!(starts_with("anything", ""));
        assert!(ends_with("anything", ""));
    }

    #[test]
    fn case_insensitive_order_sorts_alphabetically() {
        use core::cmp::Ordering;
        assert_eq!(cmp_ignore_ascii_case("apple", "Zebra"), Ordering::Less);
        assert_eq!(cmp_ignore_ascii_case("Zebra", "apple"), Ordering::Greater);
        assert_eq!(cmp_ignore_ascii_case("Apple", "apple"), Ordering::Equal);
        assert_eq!(cmp_ignore_ascii_case("app", "apple"), Ordering::Less);
    }
}
