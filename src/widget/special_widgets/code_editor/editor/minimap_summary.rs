// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The minimap's per-strip-row summary.
//!
//! Split out of `editor.rs` (principle #9). The summary is read by the painter
//! and exposed to hosts through `CodeEditor::minimap_summary`, so its data shape
//! and the partition arithmetic that produces it live together here.

/// What a minimap bar is made of, at the granularity a thumbnail can show.
///
/// Three tones, not a token palette: a minimap bar is one to three pixels tall, so
/// anything finer is invisible. Comment/string/code is enough to give a file the
/// texture Zed's minimap has (green comment blocks, warm string runs, plain code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MiniTone {
    /// Ordinary code.
    #[default]
    Plain,
    /// A comment line.
    Comment,
    /// A line carrying a string literal.
    String,
}

/// One row of the minimap: the longest non-blank line folded into it.
///
/// A bucket holds a *summary*, not a sum, so painting reads one value per strip
/// row instead of walking the document. `blank` is tracked separately from
/// `length == 0` because a bucket containing only whitespace lines is blank,
/// while one containing a line of spaces is not.
#[derive(Debug, Clone, Copy)]
pub struct MinimapBucket {
    /// First document line summarised into this bucket.
    ///
    /// A host drawing its own minimap decoration uses this to map a bucket back to a
    /// document region; the control's own painter does not need it.
    pub start_line: usize,
    /// Indent column of the longest line seen in this bucket.
    pub indent: usize,
    /// Text length of that line, indentation excluded.
    pub length: usize,
    /// What that line mostly is, for the bar's colour.
    pub tone: MiniTone,
    /// `true` when no non-blank line has landed in this bucket.
    pub blank: bool,
    /// How many non-blank lines landed in this bucket.
    ///
    /// The bucket keeps a summary rather than a sum of every line, so this is the one
    /// aggregate it tracks explicitly: it lets a host tell a dense block from a sparse one
    /// without re-reading the document.
    pub non_blank_lines: usize,
}

/// How many strip rows a document is summarized into, and how many lines each
/// bucket covers.
///
/// The two numbers have to be derived together. `rows` is a pixel budget, while a
/// bucket is a group of lines; resizing the vector to `rows` and dividing the
/// document by `ceil(count / rows)` leaves a blank tail whenever the division does
/// not come out even, and — worse — makes the "did anything change" check compare a
/// derived count against a differently derived size, so the summary was rebuilt on
/// every frame.
pub(crate) fn minimap_partition(count: usize, rows: usize) -> (usize, usize) {
    let per_bucket = count.div_ceil(rows.max(1)).max(1);
    let buckets = count.div_ceil(per_bucket).max(1);
    (buckets, per_bucket)
}

/// A bucket that has not seen a line yet is **blank**.
///
/// The derived default said `blank: false`, so every bucket the document did not
/// reach — 106 of 120 in the demo's 14-line file — looked like a line and the
/// minimap painted a 1 px bar for each of them: a dashed column down the whole
/// strip instead of a page.
impl Default for MinimapBucket {
    fn default() -> Self {
        Self {
            start_line: 0,
            indent: 0,
            length: 0,
            tone: MiniTone::Plain,
            blank: true,
            non_blank_lines: 0,
        }
    }
}

impl MinimapBucket {
    /// Folds one line into the bucket, keeping the longest.
    pub(crate) fn accumulate(slot: &mut Self, text: &str) {
        let indent = text.chars().take_while(|ch| ch.is_whitespace()).count();
        let length = text.trim_end().chars().count().saturating_sub(indent);
        if length == 0 {
            return;
        }
        if slot.blank || length > slot.length {
            slot.indent = indent;
            slot.length = length;
            let trimmed = text.trim_start();
            slot.tone = if trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.starts_with('#')
            {
                MiniTone::Comment
            } else if trimmed.contains('"') || trimmed.contains('\'') {
                MiniTone::String
            } else {
                MiniTone::Plain
            };
        }
        slot.blank = false;
    }
}
