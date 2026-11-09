// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Free helpers shared by the editor's submodules.
//!
//! Split out of `editor.rs` so the file that owns the widget state and its accessors stays
//! under the 2000-line limit (principle #9). These are pure functions over text, spans and
//! positions; none of them touches editor state.

use super::super::syntax::{merge_adjacent, LanguageId};
use super::super::types::{SearchOptions, TextPosition, TokenSpan};
use crate::widget::text_utils::floor_char_boundary;
use alloc::string::String;
use alloc::vec::Vec;

// ─────────────────────────────────────────────────────────────────────────────
// Free helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns whether `candidate` closes the bracket `ch`.
pub(super) fn closes_bracket(ch: char, candidate: char) -> bool {
    matches!((ch, candidate), ('(', ')') | ('[', ']') | ('{', '}'))
}

/// Returns whether `candidate` opens the bracket `ch`.
pub(super) fn opens_bracket(ch: char, candidate: char) -> bool {
    matches!((ch, candidate), (')', '(') | (']', '[') | ('}', '{'))
}

/// `true` when `byte` falls inside one of the given spans.
///
/// The spans are the string and comment tokens of one line; a character inside one
/// of them is text, not syntax, so bracket matching must step over it.
pub(super) fn is_skipped(byte: usize, spans: &[(usize, usize)]) -> bool {
    spans.iter().any(|(start, end)| byte >= *start && byte < *end)
}

/// Byte offset and character at char index `column` of `text`.
///
/// The inverse of `LineGrid::char_of_byte`, for the scanning paths that need both
/// values at once and have the text in hand.
pub(super) fn byte_and_char_at(text: &str, column: usize) -> Option<(usize, char)> {
    text.char_indices().nth(column)
}

/// Returns the number of decimal digits of `value` (at least 1).
pub(crate) fn decimal_digits(value: usize) -> usize {
    let mut digits = 1usize;
    let mut rest = value;
    while rest >= 10 {
        rest /= 10;
        digits += 1;
    }
    digits
}

/// Shifts a line index by a signed delta, saturating at zero.
///
/// Used when a locked range has to follow an edit that inserted or removed lines
/// above it. Saturating rather than wrapping matters: a removal larger than the
/// index must land the span at line 0, not at `usize::MAX`.
pub(super) fn shift_index(index: usize, delta: isize) -> usize {
    if delta >= 0 {
        index.saturating_add(delta as usize)
    } else {
        index.saturating_sub(delta.unsigned_abs())
    }
}

/// Converts a byte offset in `text` into a [`TextPosition`].
///
/// The offset is clamped to a character boundary and to the text length, so a
/// caller may pass the end of an insertion (`start + removed + inserted`) without
/// checking whether it still lands inside the pre-edit text. Columns are counted
/// in characters, matching every other position in the editor.
pub(super) fn position_in_text(text: &str, offset: usize) -> TextPosition {
    let offset = floor_char_boundary(text, offset.min(text.len()));
    let mut line = 0usize;
    let mut column = 0usize;
    for ch in text[..offset].chars() {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    TextPosition::new(line, column)
}

/// Returns the character length of `text`.
pub(super) fn char_len_of(text: &str) -> usize {
    text.chars().count()
}

/// Returns the character length of the final line of `payload`.
pub(super) fn trailing_line_len(payload: &str) -> usize {
    payload.rsplit('\n').next().map(|line| line.chars().count()).unwrap_or(0)
}

/// Returns whether a match satisfies the whole-word constraint.
pub(super) fn whole_word_ok(
    haystack: &[char],
    start: usize,
    length: usize,
    options: SearchOptions,
    language: LanguageId,
) -> bool {
    if !options.whole_word {
        return true;
    }
    let before_ok = start == 0 || !language.is_word_char(haystack[start - 1]);
    let after = start + length;
    let after_ok = after >= haystack.len() || !language.is_word_char(haystack[after]);
    before_ok && after_ok
}

/// Upper bound on minimap strip rows.
///
/// A strip taller than this gains no legibility — a bar per pixel row is already
/// finer than the eye resolves at that width — while making the summary and the
/// per-frame loop proportionally longer. Clamping also keeps the summary a fixed
/// size across window resizes in the common case.
pub(crate) const MAX_MINIMAP_ROWS: usize = 256;

/// Everything an edit path can tell the history about what it changed.
///
/// The three fields are independently optional because the paths know different
/// amounts: typing knows the byte range it replaced and the text that was there;
/// a line command that rebuilt the document only has the before/after pair. The
/// most specific description wins, and anything missing is reconstructed from the
/// buffer before the edit overwrites it.
pub(super) struct Checkpoint {
    /// `(start_offset, removed_len, inserted)` in pre-edit coordinates.
    pub(super) range: Option<(usize, usize, String)>,
    /// Text the edit replaced, read before the write.
    pub(super) removed: Option<String>,
    /// The document as it was before the edit, for the snapshot fallback.
    pub(super) before: Option<String>,
}

/// Returns the minimal `(start, removed_len, inserted)` change between two texts.
///
/// Used only by the edit paths that rebuild the document by hand and therefore
/// have no range to report (line commands, sort, trim). The common prefix and
/// suffix are trimmed, so the recorded edit is the smallest one that reproduces
/// `after` — which keeps a checkpoint's memory proportional to what changed even
/// when the caller could only describe the result.
///
/// Returns `None` when the texts are equal.
pub(super) fn diff_range(before: &str, after: &str) -> Option<(usize, usize, String)> {
    if before == after {
        return None;
    }
    let before_bytes = before.as_bytes();
    let after_bytes = after.as_bytes();
    let max_prefix = before_bytes.len().min(after_bytes.len());
    let mut prefix = 0usize;
    while prefix < max_prefix && before_bytes[prefix] == after_bytes[prefix] {
        prefix += 1;
    }
    // Do not cut a multi-byte character in half: back up to the nearest boundary.
    while prefix > 0 && !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = (before_bytes.len() - prefix).min(after_bytes.len() - prefix);
    let mut suffix = 0usize;
    while suffix < max_suffix
        && before_bytes[before_bytes.len() - 1 - suffix]
            == after_bytes[after_bytes.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0 && !after.is_char_boundary(after.len() - suffix) {
        suffix -= 1;
    }
    while suffix > 0 && !before.is_char_boundary(before.len() - suffix) {
        suffix -= 1;
    }
    let removed_len = before_bytes.len().saturating_sub(prefix + suffix);
    let inserted_end = after_bytes.len().saturating_sub(suffix);
    Some((prefix, removed_len, after[prefix..inserted_end].to_string()))
}

/// Clamps highlighter spans to `text` and merges the adjacent ones.
///
/// A third-party highlighter can return offsets past the line end or inside a
/// multi-byte character; both are repaired here rather than at every call site,
/// because a malformed span reaching the renderer panics the whole frame.
pub(super) fn clamp_and_merge(text: &str, raw: Vec<TokenSpan>) -> Vec<TokenSpan> {
    let length = text.len();
    let mut spans: Vec<TokenSpan> = Vec::with_capacity(raw.len());
    for mut span in raw {
        span.start = floor_char_boundary(text, span.start.min(length));
        span.end = floor_char_boundary(text, span.end.min(length));
        if span.end > span.start {
            spans.push(span);
        }
    }
    spans.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));
    merge_adjacent(spans)
}
