// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The compared-row model: line kinds, one row, whitespace policy and view mode.

use std::borrow::Cow;

/// Diff line state.
///
/// Classifies how the two sides relate for one compared line. Which variant a
/// line receives is decided by the diff algorithm, not by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// The line is present and identical on both sides.
    Equal,
    /// The line exists only on the right-hand side.
    Added,
    /// The line exists only on the left-hand side.
    Removed,
    /// The line exists on both sides but its content differs.
    Changed,
}

/// One compared line entry.
///
/// `left` and `right` are the two sides of the same row; they are `None` when
/// that side has no line at this position, which is what distinguishes
/// [`DiffKind::Added`] (left is `None`) from [`DiffKind::Removed`] (right is
/// `None`). Nothing enforces that invariant, so a hand-built entry can express
/// combinations the diff algorithm never produces, such as `Changed` with one
/// side `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Content shown in the left column, or `None` for a line that only exists
    /// on the right.
    pub left: Option<String>,
    /// Content shown in the right column, or `None` for a line that only exists
    /// on the left.
    pub right: Option<String>,
    /// How the two sides relate; see [`DiffKind`].
    pub kind: DiffKind,
    /// Whether a `Changed` pair differs only in whitespace.
    ///
    /// Precomputed with the row so the paint path can dim it without re-scanning the text: a
    /// whitespace-only change is common in reformatting commits and is not the same event as a
    /// semantic edit, which is the distinction this flag lets the view show.
    pub blank_only: bool,
    /// The byte spans that differ, as `(left_span, right_span)`, for a `Changed` row.
    ///
    /// Computed once when the row list is built. The paint path used to call
    /// `differing_spans` for every visible changed row on every frame, allocating two `Vec`s
    /// per row per frame; this carries the answer instead.
    pub changed_spans: Option<((usize, usize), (usize, usize))>,
}

impl DiffLine {
    /// Builds one compared row, deriving `changed_spans` and `blank_only`.
    pub(super) fn new(left: Option<String>, right: Option<String>, kind: DiffKind) -> Self {
        let changed_spans = match (&left, &right, kind) {
            (Some(left), Some(right), DiffKind::Changed) => Some(differing_spans(left, right)),
            _ => None,
        };
        let blank_only = match (&left, &right, kind) {
            (Some(left), Some(right), DiffKind::Changed) => {
                left != right && collapse_whitespace(left) == collapse_whitespace(right)
            }
            _ => false,
        };
        Self { left, right, kind, blank_only, changed_spans }
    }
}

/// How whitespace differences are judged when comparing the two snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhitespaceMode {
    /// Every byte counts; the default.
    #[default]
    Show,
    /// Trailing whitespace is ignored, so an end-of-line reformat is not a change.
    IgnoreEol,
    /// All whitespace is ignored, so a re-indent is not a change.
    IgnoreAll,
    /// All whitespace is ignored for *matching*, but a row that differs only in whitespace is
    /// still reported as `Changed` with [`DiffLine::blank_only`] set, so a reader can see that a
    /// re-indent happened without it masquerading as an edit.
    MarkBlankChanges,
}

impl WhitespaceMode {
    /// The token spelling published as the `whitespace_mode` property.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::IgnoreEol => "ignore_eol",
            Self::IgnoreAll => "ignore_all",
            Self::MarkBlankChanges => "mark_blank_changes",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "show" | "exact" => Some(Self::Show),
            "ignore_eol" | "ignore_trailing" => Some(Self::IgnoreEol),
            "ignore_all" | "ignore_whitespace" => Some(Self::IgnoreAll),
            "mark_blank_changes" | "mark_blank" => Some(Self::MarkBlankChanges),
            _ => None,
        }
    }

    /// The comparison key for one line.
    ///
    /// Borrows the line in the common `Show` mode: normalising every line into a fresh `String`
    /// made each comparison copy the whole document even when no rule asked for a copy.
    pub(super) fn key_for(self, line: &str) -> Cow<'_, str> {
        match self {
            Self::Show => Cow::Borrowed(line),
            Self::IgnoreEol => Cow::Borrowed(line.trim_end()),
            Self::IgnoreAll | Self::MarkBlankChanges => Cow::Owned(collapse_whitespace(line)),
        }
    }

    /// Whether whitespace-only pairs are kept as `blank_only` rows.
    pub(super) fn marks_blank_changes(self) -> bool {
        matches!(self, Self::MarkBlankChanges)
    }
}

/// How the two sides are presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffViewMode {
    /// Two columns, one per snapshot; the default.
    #[default]
    SideBySide,
    /// One column with `-`/`+` prefixes, the conventional patch form.
    Unified,
}

impl DiffViewMode {
    /// The token spelling published in `view_mode_changed`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SideBySide => "side_by_side",
            Self::Unified => "unified",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "side_by_side" | "split" => Some(Self::SideBySide),
            "unified" | "patch" => Some(Self::Unified),
            _ => None,
        }
    }
}

/// Removes every whitespace character from a line, for whitespace-insensitive comparison.
pub(super) fn collapse_whitespace(line: &str) -> String {
    line.chars().filter(|ch| !ch.is_whitespace()).collect()
}

/// Widens `span` to the word boundaries it touches in `text`.
///
/// A word character is a letter, a digit or `_`, which is the same set a reader calls "part of an
/// identifier" in the code this control is mostly used for. When the span is empty (an insertion
/// at a point) the word under that point is selected instead, so a pure insertion still has
/// something to tint.
pub(super) fn word_spans(text: &str, span: (usize, usize)) -> (usize, usize) {
    if span.0 >= text.len() && span.1 >= text.len() {
        return span;
    }
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut start = span.0.min(text.len());
    let mut end = span.1.max(start).min(text.len());
    if start == end {
        // An empty span sits between two characters; take the word on either side.
        start = text[..start]
            .char_indices()
            .rev()
            .find(|(_, ch)| !is_word(*ch))
            .map_or(0, |(i, ch)| i + ch.len_utf8());
        end = text[end..]
            .char_indices()
            .find(|(_, ch)| !is_word(*ch))
            .map_or(text.len(), |(i, _)| end + i);
        return (start, end);
    }
    while start > 0 {
        let Some((index, ch)) = text[..start].char_indices().next_back() else {
            break;
        };
        if !is_word(ch) {
            break;
        }
        start = index;
    }
    while end < text.len() {
        let Some(ch) = text[end..].chars().next() else {
            break;
        };
        if !is_word(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    (start, end)
}

/// The byte spans that differ between two lines.
///
/// The common prefix and suffix are trimmed; what remains is the changed run on each side. That
/// is the character granularity of a line diff; [`word_spans`] widens it when the reader asked for
/// word granularity, so a single edited word leaves one word tinted rather than the whole line.
pub(super) fn differing_spans(left: &str, right: &str) -> ((usize, usize), (usize, usize)) {
    let left_chars: Vec<(usize, char)> = left.char_indices().collect();
    let right_chars: Vec<(usize, char)> = right.char_indices().collect();
    let mut prefix = 0usize;
    while prefix < left_chars.len()
        && prefix < right_chars.len()
        && left_chars[prefix].1 == right_chars[prefix].1
    {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < left_chars.len().saturating_sub(prefix)
        && suffix < right_chars.len().saturating_sub(prefix)
        && left_chars[left_chars.len() - 1 - suffix].1
            == right_chars[right_chars.len() - 1 - suffix].1
    {
        suffix += 1;
    }
    let left_start = left_chars.get(prefix).map_or(left.len(), |(offset, _)| *offset);
    let left_end = left_chars
        .get(left_chars.len().saturating_sub(suffix))
        .map_or(left.len(), |(offset, _)| *offset);
    let right_start = right_chars.get(prefix).map_or(right.len(), |(offset, _)| *offset);
    let right_end = right_chars
        .get(right_chars.len().saturating_sub(suffix))
        .map_or(right.len(), |(offset, _)| *offset);
    ((left_start, left_end.max(left_start)), (right_start, right_end.max(right_start)))
}
