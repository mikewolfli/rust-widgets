// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `TextArea`'s viewport: the display-row table (soft wrap included), the geometry a caret or a
//! click resolves against, and the `Draw` implementation.
//!
//! The rows here are the one table the paint, the caret and the hit test all read, so a scrolled
//! or wrapped field draws and hit-tests on the same rows.

use crate::compat::{Rc, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::metrics::{dimensions, effective_font};
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::widget_trait::InputMethodQuery;
use crate::widget::{Draw, Widget};
use std::collections::HashMap;

use super::model::{LineSpan, TextArea};

/// One display row: the byte range of the value it paints.
///
/// A logical line is one row unless soft wrap is on and the line is wider than the field, in which
/// case the same line contributes several rows. `end` excludes the line's terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DisplayRow {
    /// First byte of the row's text.
    pub(crate) start: usize,
    /// One past the last byte of the row's text, excluding a newline.
    pub(crate) end: usize,
}

/// Memoised display rows, keyed by the inputs the wrapping reads.
pub(crate) struct DisplayRowCache {
    pub(crate) revision: u64,
    width: u32,
    soft_wrap: bool,
    margin: i32,
    /// The full font the rows were wrapped with, not only its size.
    ///
    /// Wrapping is a width measurement, so a face or weight change at the same point size moves
    /// every row boundary. Keying the cache on `size().to_bits()` alone left it answering from the
    /// previous face (I-07).
    font: Font,
    pub(crate) rows: Rc<Vec<DisplayRow>>,
    /// The logical spans the rows were built from, retained so the next edit can reuse the
    /// untouched paragraphs at either end (I-006).
    spans: Rc<Vec<LineSpan>>,
    /// One entry per logical paragraph, parallel to `spans`.
    paragraphs: Vec<ParagraphRow>,
    /// Index into `rows` of each paragraph's first row, parallel to `spans`.
    row_starts: Vec<usize>,
    /// Document byte length when the cache was built, used to validate a pending splice.
    text_len: usize,
}

/// One logical paragraph's wrapped rows, keyed by its own content revision and the style inputs.
///
/// A soft-wrap pass used to re-measure every glyph of the document on any edit. A paragraph's own
/// text, width and font decide its breaks, so an unchanged paragraph hands back its previous rows.
/// The `Rc` keeps reusing an untouched paragraph to a refcount bump.
#[derive(Clone)]
pub(crate) struct ParagraphRow {
    /// Fingerprint of the paragraph's visible text.
    content_revision: u64,
    /// The width the paragraph was wrapped at.
    width: u32,
    /// Whether soft wrap was on when the rows were built.
    soft_wrap: bool,
    font: Font,
    /// `(relative start, relative end)` pairs inside the paragraph's content.
    rows: Rc<Vec<(usize, usize)>>,
}

/// A byte splice that [`TextArea::display_rows`] can apply without rebuilding every paragraph.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PendingEdit {
    /// Byte offset in the new text where the changed run begins.
    pub(crate) start: usize,
    /// Bytes removed at `start` from the pre-edit text.
    pub(crate) removed: usize,
    /// Bytes inserted at `start` in the new text.
    pub(crate) inserted: usize,
}

/// A cheap content hash for one line's wrap-cache key.
fn line_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The logical paragraph containing `offset`, clamped to a real span.
fn paragraph_index_at(spans: &[LineSpan], offset: usize) -> usize {
    spans.partition_point(|&(start, _, _)| start <= offset).saturating_sub(1)
}

/// Breaks one paragraph's content into `(relative start, relative end)` rows.
///
/// This is the same measurement the whole-table path always made, expressed relative to the
/// paragraph start so an unchanged row can be moved when earlier text shifts.
fn wrap_paragraph_rows(
    content: &str,
    width: u32,
    font: &Font,
    context: &RenderContext<'_>,
) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    let mut row_start = 0usize;
    let mut row_width = 0.0f32;
    let mut last_break: Option<usize> = None;
    for (offset, ch) in content.char_indices() {
        let mut glyph = [0u8; 4];
        let advance = context.measure_text(ch.encode_utf8(&mut glyph), font).width as f32;
        if row_width + advance > width as f32 && offset > row_start {
            let break_at = last_break
                .filter(|boundary| *boundary > row_start && *boundary <= offset)
                .unwrap_or(offset);
            rows.push((row_start, break_at));
            row_start = break_at;
            row_width = 0.0;
            for (_, carried) in content[row_start..offset].char_indices() {
                let mut glyph = [0u8; 4];
                row_width +=
                    context.measure_text(carried.encode_utf8(&mut glyph), font).width as f32;
            }
            last_break = None;
        }
        if ch.is_whitespace() || ch == '-' {
            last_break = Some(offset + ch.len_utf8());
        }
        row_width += advance;
    }
    rows.push((row_start, content.len()));
    rows
}

/// Memoised selection-band advances, keyed by the document revision and the full font.
pub(crate) struct BandWidthCache {
    revision: u64,
    font: Font,
    /// Measured `text[from..to]` advances, keyed by the byte range.
    ///
    /// A hash lookup replaces the old linear `Vec` scan (I-P61). The entry count is bounded so a
    /// long-lived document cannot grow the cache without limit; a full cache is dropped rather
    /// than evicted one entry at a time, which keeps the bookkeeping trivial.
    pub(crate) entries: HashMap<(usize, usize), i32>,
}

/// Per-character advances for one display row, keyed by the document revision and font (I-P09).
///
/// The pointer hit test used to shape every glyph of the row on every press and drag step. The
/// row is immutable for a given revision, so the cumulative boundaries are built once and read
/// by every following click; a caret move does not invalidate them.
pub(crate) struct LineAdvanceCache {
    revision: u64,
    font: Font,
    start: usize,
    end: usize,
    /// `(byte offset relative to `start`, cumulative advance)` at every character boundary.
    cuts: Vec<(usize, i32)>,
}

/// Upper bound on memoised selection-band advances (I-P61).
const BAND_WIDTH_CACHE_LIMIT: usize = 256;

impl TextArea {
    /// The first logical line the caret's row shows, i.e. how far the viewport is scrolled.
    pub fn first_visible_line(&self) -> usize {
        self.first_visible_line
    }

    /// How many whole lines fit in the control's painting area.
    ///
    /// Derived from the control's own height and the same [`LINE_H`] the paint path advances by, so
    /// "a line fits" means the same thing to the scroll arithmetic and to the rows that are drawn.
    /// Always at least one, because a control shorter than a single line still has to show something
    /// rather than dividing by zero or scrolling past everything.
    pub(crate) fn visible_line_count(&self) -> usize {
        let first_line_top = FIRST_LINE_INSET + (LINE_H - self.line_box_height()) / 2;
        let usable = self.geometry().height as i32 - first_line_top;
        ((usable / LINE_H).max(1)) as usize
    }

    /// The logical line a caret at `offset` sits on.
    ///
    /// Used by the tests and retained as documentation of the line index space; the paint and hit
    /// test now read [`Self::display_row_of`], which accounts for soft wrap.
    #[cfg(test)]
    pub(crate) fn line_index_of(&self, offset: usize) -> usize {
        let caret = floor_char_boundary(&self.text, offset.min(self.text.len()));
        let spans = self.line_spans();
        spans.partition_point(|&(start, _, _)| start <= caret).saturating_sub(1)
    }

    /// Scrolls the viewport the minimum amount that brings the caret's line into view.
    ///
    /// # The defect this fixes
    ///
    /// The paint path already stopped at the last whole line it could fit, so a value taller than
    /// the control did not overrun its own rectangle. What was missing was any way to reach the lines
    /// it skipped: typing past the bottom row wrote into a line the user could not see, and there was
    /// no key or gesture that brought it into view. That is the "invisible but editable" state the
    /// probe measured — the ink box stayed inside the field while the caret walked off the bottom.
    ///
    /// The scroll is *minimal*: the viewport moves only when the caret is above or below the window
    /// it currently shows, so reading text near the top does not make the field jump.
    pub(crate) fn scroll_caret_into_view(&mut self) {
        let caret_row = self.display_row_of(self.cursor_pos);
        self.first_visible_line = crate::widget::text_utils::first_visible_row_for(
            caret_row,
            self.first_visible_line,
            self.visible_line_count(),
        );
        self.ensure_caret_visible();
    }

    /// Scrolls the viewport by `lines` rows toward the end of the document.
    ///
    /// Positive values reveal later lines, negative values reveal earlier ones. The result is
    /// clamped so the first visible line can never scroll past the last window of the document,
    /// which keeps a wheel notch at either end from leaving the field showing blank rows.
    /// Returns whether the viewport actually moved, so the caller repaints only on a change.
    pub(crate) fn scroll_lines(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let max_first = self.max_first_visible_line();
        let next =
            (self.first_visible_line as i64 + lines as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_line {
            return false;
        }
        self.first_visible_line = next;
        true
    }

    /// The largest first-visible line that still fills the viewport, i.e. the scroll clamp.
    fn max_first_visible_line(&self) -> usize {
        self.display_rows().len().saturating_sub(self.visible_line_count())
    }

    /// Places `line` at the top of the viewport, clamped so the field never scrolls past its end.
    ///
    /// The same clamp [`Self::scroll_lines`] applies: a line past the last window is pinned to
    /// the last window, and a line that is already first is a no-op with no repaint.
    pub fn set_first_visible_line(&mut self, line: usize) {
        let next = line.min(self.max_first_visible_line());
        if next == self.first_visible_line {
            return;
        }
        self.first_visible_line = next;
        self.base.request_redraw();
    }

    /// Scrolls the viewport so `line` is the first visible line.
    ///
    /// A convenience spelling of [`Self::set_first_visible_line`] for hosts that mirror an
    /// external view: the line is clamped to the document's last window rather than being
    /// accepted as an arbitrary scroll position.
    pub fn scroll_to_line(&mut self, line: usize) {
        self.set_first_visible_line(line);
    }
}

impl TextArea {
    /// The width a display row has for text, after the field's padding and the content margin.
    pub(crate) fn content_width(&self) -> u32 {
        let inset = dimensions::TEXT_FIELD_PADDING_H as i32 + self.content_margin;
        self.geometry().width.saturating_sub((inset * 2).max(0) as u32)
    }

    /// The display rows the paint, the caret and the hit test all read.
    ///
    /// With soft wrap off this is exactly one row per logical line, which is what keeps the
    /// existing layout and its pixels unchanged. The cache is keyed by the mutation counter rather
    /// than the document text, so a hit is an integer comparison.
    ///
    /// When only the text changed, a pending splice lets this rebuild solely the paragraphs it
    /// touched and reuse the untouched ends (I-006). A width, wrap, margin or font change takes
    /// the whole-table path, because it moves every paragraph.
    pub(crate) fn display_rows(&self) -> Rc<Vec<DisplayRow>> {
        let width = self.content_width();
        let font = self.text_font().clone();
        {
            let cache = self.display_rows_cache.borrow();
            if let Some(cache) = cache.as_ref() {
                if cache.revision == self.revision
                    && cache.width == width
                    && cache.soft_wrap == self.soft_wrap
                    && cache.margin == self.content_margin
                    && cache.font == font
                {
                    return Rc::clone(&cache.rows);
                }
            }
        }
        let pending = self.pending_row_edit.take();
        let incremental = {
            let cache = self.display_rows_cache.borrow();
            match (cache.as_ref(), pending) {
                (Some(cached), Some(edit))
                    if cached.width == width
                        && cached.soft_wrap == self.soft_wrap
                        && cached.margin == self.content_margin
                        && cached.font == font
                        && edit.start <= self.text.len()
                        && edit.start + edit.inserted <= self.text.len()
                        && edit.start + edit.removed <= cached.text_len
                        && cached.text_len + edit.inserted == self.text.len() + edit.removed =>
                {
                    self.incremental_display_rows(cached, edit, width, &font)
                }
                _ => None,
            }
        };
        let next = incremental.unwrap_or_else(|| {
            let cache = self.display_rows_cache.borrow();
            self.build_display_rows(width, &font, cache.as_ref())
        });
        let rows = Rc::clone(&next.rows);
        *self.display_rows_cache.borrow_mut() = Some(next);
        rows
    }

    /// Rebuilds only the paragraphs a splice touched, reusing the untouched ends (I-006).
    ///
    /// Returns `None` when the old and new line tables do not line up — for example a splice that
    /// removes the terminator between two paragraphs and therefore merges them. The caller then
    /// takes the whole-table path, so the returned rows always match a full rebuild byte for byte.
    pub(crate) fn incremental_display_rows(
        &self,
        cached: &DisplayRowCache,
        edit: PendingEdit,
        width: u32,
        font: &Font,
    ) -> Option<DisplayRowCache> {
        let spans = self.line_spans();
        let old_spans = &cached.spans;
        let old_count = old_spans.len();
        let new_count = spans.len();
        let first_old = paragraph_index_at(old_spans, edit.start);
        let last_old = if edit.removed == 0 {
            first_old
        } else {
            paragraph_index_at(old_spans, edit.start + edit.removed - 1)
        };
        let prefix = first_old.min(old_count).min(new_count);
        let suffix = old_count.saturating_sub(last_old + 1);
        if prefix + suffix > new_count {
            return None;
        }
        let new_suffix_start = new_count - suffix;
        let old_suffix_start = last_old + 1;
        if new_suffix_start < prefix {
            return None;
        }
        // The untouched tail must still begin at the same paragraph boundary, shifted by the
        // splice. Removing the terminator that separated the changed run from the tail merges
        // the two paragraphs, and that is not a reusable suffix.
        if suffix > 0 {
            let delta = edit.inserted as isize - edit.removed as isize;
            let expected = old_spans[old_suffix_start].0 as isize + delta;
            if expected < 0 || spans[new_suffix_start].0 as isize != expected {
                return None;
            }
        }

        let mut rows: Vec<DisplayRow> = Vec::new();
        let mut paragraphs: Vec<ParagraphRow> = Vec::with_capacity(new_count);
        let mut row_starts: Vec<usize> = Vec::with_capacity(new_count);

        // The prefix is byte-for-byte untouched, so its absolute rows can be copied as they are.
        let prefix_row_end = cached.row_starts.get(prefix).copied().unwrap_or(cached.rows.len());
        rows.extend_from_slice(&cached.rows[..prefix_row_end.min(cached.rows.len())]);
        row_starts.extend_from_slice(cached.row_starts.get(..prefix).unwrap_or(&[]));
        paragraphs.extend(cached.paragraphs.iter().take(prefix).cloned());

        // Only the middle is measured. Every untouched paragraph keeps its cached rows.
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        for &(start, content_end, _) in spans.iter().take(new_suffix_start).skip(prefix) {
            row_starts.push(rows.len());
            let content = &self.text[start..content_end];
            let entry = self.wrap_paragraph(width, font, &context, content, line_hash(content));
            for &(rel_start, rel_end) in entry.rows.iter() {
                rows.push(DisplayRow { start: start + rel_start, end: start + rel_end });
            }
            paragraphs.push(entry);
        }

        // The tail's content is unchanged; its relative rows shift with the new span start.
        for offset in 0..suffix {
            let new_index = new_suffix_start + offset;
            let old_index = old_suffix_start + offset;
            let start = spans[new_index].0;
            row_starts.push(rows.len());
            let entry = cached.paragraphs.get(old_index)?.clone();
            for &(rel_start, rel_end) in entry.rows.iter() {
                rows.push(DisplayRow { start: start + rel_start, end: start + rel_end });
            }
            paragraphs.push(entry);
        }

        if rows.is_empty() {
            rows.push(DisplayRow { start: 0, end: 0 });
        }
        Some(DisplayRowCache {
            revision: self.revision,
            width,
            soft_wrap: self.soft_wrap,
            margin: self.content_margin,
            font: font.clone(),
            rows: Rc::new(rows),
            spans,
            paragraphs,
            row_starts,
            text_len: self.text.len(),
        })
    }

    /// Builds the whole display-row table, wrapping long paragraphs when soft wrap is on.
    ///
    /// This is the width/style-change fallback. It may reuse a paragraph whose content, width and
    /// font are unchanged from `previous`, but it visits every logical paragraph.
    pub(crate) fn build_display_rows(
        &self,
        width: u32,
        font: &Font,
        previous: Option<&DisplayRowCache>,
    ) -> DisplayRowCache {
        let spans = self.line_spans();
        let reusable = previous.filter(|cache| {
            cache.width == width
                && cache.soft_wrap == self.soft_wrap
                && cache.margin == self.content_margin
                && cache.font == *font
        });
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        let mut rows = Vec::new();
        let mut paragraphs = Vec::with_capacity(spans.len());
        let mut row_starts = Vec::with_capacity(spans.len());
        for (index, &(start, content_end, _)) in spans.iter().enumerate() {
            row_starts.push(rows.len());
            let content = &self.text[start..content_end];
            let content_revision = line_hash(content);
            let reused = reusable
                .and_then(|cache| cache.paragraphs.get(index))
                .filter(|entry| {
                    entry.content_revision == content_revision
                        && entry.width == width
                        && entry.soft_wrap == self.soft_wrap
                        && entry.font == *font
                })
                .cloned();
            let entry = reused.unwrap_or_else(|| {
                self.wrap_paragraph(width, font, &context, content, content_revision)
            });
            for &(rel_start, rel_end) in entry.rows.iter() {
                rows.push(DisplayRow { start: start + rel_start, end: start + rel_end });
            }
            paragraphs.push(entry);
        }
        if rows.is_empty() {
            rows.push(DisplayRow { start: 0, end: 0 });
        }
        DisplayRowCache {
            revision: self.revision,
            width,
            soft_wrap: self.soft_wrap,
            margin: self.content_margin,
            font: font.clone(),
            rows: Rc::new(rows),
            spans,
            paragraphs,
            row_starts,
            text_len: self.text.len(),
        }
    }

    /// Measures one logical paragraph, returning its relative rows and cache key.
    fn wrap_paragraph(
        &self,
        width: u32,
        font: &Font,
        context: &RenderContext<'_>,
        content: &str,
        content_revision: u64,
    ) -> ParagraphRow {
        let rows = if !self.soft_wrap || width == 0 || content.is_empty() {
            Rc::new(vec![(0usize, content.len())])
        } else {
            Rc::new(wrap_paragraph_rows(content, width, font, context))
        };
        ParagraphRow {
            content_revision,
            width,
            soft_wrap: self.soft_wrap,
            font: font.clone(),
            rows,
        }
    }

    /// The display row a caret at `offset` belongs to.
    pub(crate) fn display_row_of(&self, offset: usize) -> usize {
        let rows = self.display_rows();
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        rows.iter().rposition(|row| row.start <= offset).unwrap_or(0)
    }

    /// The display row a viewport row paints.
    pub(crate) fn display_row(&self, row: usize) -> Option<DisplayRow> {
        self.display_rows().get(self.first_visible_line + row).copied()
    }

    /// Scrolls the value horizontally so the caret is visible.
    ///
    /// Soft-wrapped rows never scroll sideways, because they have no overflow to reveal.
    fn ensure_caret_visible(&mut self) {
        if self.soft_wrap {
            self.scroll_x = 0;
            return;
        }
        let width = self.content_width() as i32;
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        let line_start = self.text[..caret].rfind('\n').map(|index| index + 1).unwrap_or(0);
        let advance = self.text_advance(&self.text[line_start..caret]);
        let room = 8.min(width / 4);
        if advance - self.scroll_x > width - room {
            self.scroll_x = (advance - width + room).max(0);
        } else if advance < self.scroll_x {
            self.scroll_x = advance;
        }
        self.scroll_x = self.scroll_x.max(0);
    }

    /// The byte offset of the character *before* the caret.
    ///
    /// Delegates to the shared [`crate::widget::text_utils::previous_char_boundary`] (WDGT-11).
    pub(crate) fn previous_char_boundary(&self) -> usize {
        crate::widget::text_utils::previous_char_boundary(&self.text, self.cursor_pos)
    }

    /// The byte offset one character after the caret.
    pub(crate) fn next_char_boundary(&self) -> usize {
        crate::widget::text_utils::next_char_boundary(&self.text, self.cursor_pos)
    }

    /// The caret's own line start, in bytes.
    ///
    /// A caret sitting exactly after a `\n` belongs to the *following* line (which may be empty),
    /// which is what makes `End` then `Home` land on the line the user sees rather than jumping to
    /// the previous one.
    pub(crate) fn line_start_of_caret(&self) -> usize {
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        self.text[..caret].rfind('\n').map(|i| i + 1).unwrap_or(0)
    }

    /// The offset just past the caret's own line, **excluding** that line's terminator.
    ///
    /// Excluding the newline is what makes `End` put the caret at the end of the visible line
    /// rather than at the start of the next one.
    pub(crate) fn line_end_of_caret(&self) -> usize {
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        match self.text[caret..].find('\n') {
            Some(offset) => caret + offset,
            None => self.text.len(),
        }
    }

    /// The caret one *logical* line up, at the same column where that column exists.
    ///
    /// The column counts characters, not bytes, so a line of multi-byte text does not pull the
    /// caret past the matching glyph. A destination line shorter than the column saturates at its
    /// end, which is the conventional behaviour: one Up from a long line onto a short one lands at
    /// the short line's end rather than refusing to move. With no line above, the caret keeps its
    /// own position, so the key is never silently swallowed.
    pub(crate) fn line_up(&self) -> usize {
        let start = self.line_start_of_caret();
        if start == 0 {
            return self.cursor_pos;
        }
        let column = self.column_of_caret(start);
        // `start - 1` is the newline terminating the line above, so that line ends there.
        let previous_start = self.text[..start - 1].rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.byte_index_at_column(previous_start, start - 1, column)
    }

    /// The caret one *logical* line down, at the same column where that column exists.
    ///
    /// See [`Self::line_up`] for the column convention and the saturation rule.
    pub(crate) fn line_down(&self) -> usize {
        let end = self.line_end_of_caret();
        if end >= self.text.len() {
            return self.cursor_pos;
        }
        let column = self.column_of_caret(self.line_start_of_caret());
        // `end` is this line's newline, so the next line starts one past it.
        let next_start = end + 1;
        let next_end = match self.text[next_start..].find('\n') {
            Some(offset) => next_start + offset,
            None => self.text.len(),
        };
        self.byte_index_at_column(next_start, next_end, column)
    }

    /// The caret's column within its line, counted in **characters**.
    ///
    /// Character-counted rather than byte-counted because the column is compared against another
    /// line's glyphs: a byte count would over-count a multi-byte line and walk the caret further
    /// than the vertical movement covers.
    fn column_of_caret(&self, line_start: usize) -> usize {
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        self.text[line_start..caret].chars().count()
    }

    /// The caret's column within its own display row, counted in characters.
    ///
    /// With soft wrap a logical line can occupy several rows, so the column has to be measured
    /// from the row's start rather than from the line's.
    pub(crate) fn column_in_display_row(&self, offset: usize) -> usize {
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        let row_start =
            self.display_rows().iter().rfind(|row| row.start <= offset).map_or(0, |row| row.start);
        self.text[row_start..offset].chars().count()
    }

    /// The byte offset of `column` characters into `text[line_start..line_end]`, saturating at
    /// `line_end` when the line is shorter.
    pub(crate) fn byte_index_at_column(
        &self,
        line_start: usize,
        line_end: usize,
        column: usize,
    ) -> usize {
        self.text[line_start..line_end]
            .char_indices()
            .nth(column)
            .map(|(offset, _)| line_start + offset)
            .unwrap_or(line_end)
    }
}

/// Approximate line height in pixels for `Font::default()`.
///
/// # Why a row height is still a constant, when the character width is not
///
/// `CHAR_W` was removed: the advance of a glyph is a fact about the *face*, and multiplying a
/// character count by `8` disagreed with the renderer by up to 12 px on CJK (measured). Row height
/// is a different kind of number — it is the control's own layout decision (how far apart the rows
/// are), not a measurement of any glyph — so a single value is correct here and the paint path and
/// the hit test share it.
pub(crate) const LINE_H: i32 = 16;
/// The first line's inset below the control's top edge.
///
/// A named constant rather than a literal inside `draw`, because the pixel-to-offset map in
/// [`TextArea::byte_index_at`] has to place a click on the same rows the glyphs were painted on;
/// two copies of `4` is how a click and its text drift apart at the first line.
pub(crate) const FIRST_LINE_INSET: i32 = 4;

impl Draw for TextArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a queued find/replace intent first, so the caret and the selection this frame
        // paints are the ones the strip asked for (COMP-A8).
        #[cfg(full_widgets)]
        self.poll_find_replace_bar();
        // A text area is the one field whose height is **not** a fixed band: it is
        // multi-line, so the lines legitimately span whatever height the caller gives it.
        // `rect.height` therefore stays. Everything *inside* is measured from the field's
        // own padding rather than from a local literal, so the first line starts where a
        // single-line field's does instead of at a second, unshared `y = 4`.
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;

        // -- Background --
        let bg = self.style().background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.face_with_gradient_corners(
            rect,
            bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // -- Border --
        let border = self.style().border_color.unwrap_or(Color::rgb(200, 200, 200));
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // -- Text --
        let text_color = self.style().text_color.unwrap_or(Color::rgb(0, 0, 0));
        // The placeholder is the value's own ink damped toward the field it sits on — the same
        // derivation `line_edit` and `auto_complete_edit` use — rather than a fixed grey that
        // ignored the appearance. A literal here rendered a light-theme hint over a dark field.
        // The *disabled* ink is a different fact (see `disabled_ink_on`) and is not this colour.
        let field_bg = bg;
        let placeholder_color = text_color.blend(&field_bg, 0.55);

        // The first line's own glyph box, centred on a line-height band below the top edge.
        // The origin of a text run is the *top-left corner of its glyph box*, so the old
        // `rect.y + padding` put that corner 4 px below the border and drew the first line
        // half a line high — the same off-by-a-half-line defect the single-line fields had,
        // and the reason the first line sat at `y = 4` in `text_area.svg` while its own
        // placeholder sat beside it on a different baseline.
        //
        // The band is one line tall and starts one inset below the top border, where a
        // single-line field's content starts, so a text area's first line and a `line_edit`'s
        // value sit on the same row when the two are laid out at the same height. Deriving
        // the band from the *first line* rather than from the whole control is what keeps a
        // 120 px text area from putting its first line halfway down the control.
        let font = self.text_font();
        let first_band = Rect::new(origin_x, rect.y + FIRST_LINE_INSET, rect.width, LINE_H as u32);
        let first_line = context.text_line(first_band, font);
        let text_top = first_line.y;

        // ── Clip to the field's inner box ──
        //
        // This area has no soft wrap and no horizontal scroll, so a line longer than the field
        // was painted straight across the control's right edge and over its neighbours (the
        // runtime only clips a widget whose `children_clip()` is set, which a text area does not
        // override). The selection band and the caret followed the same overflow. Clipping keeps
        // the visible part exactly where it was — the origin does not move, so no existing
        // layout shifts — and makes the field a window onto its own content, which is the same
        // answer `code_editor` and `properties_panel` already give.
        let inner = Rect::new(
            origin_x,
            rect.y.saturating_add(1),
            rect.width.saturating_sub(2 * dimensions::TEXT_FIELD_PADDING_H),
            rect.height.saturating_sub(2),
        );
        context.push_clip(inner.x, inner.y, inner.width, inner.height);

        if self.text.is_empty() && !self.placeholder.is_empty() && !self.focused {
            // Draw placeholder in gray, on the same first-line box the value would use.
            context.draw_text(
                Point::new(origin_x, text_top),
                &self.placeholder,
                font,
                placeholder_color,
                HorizontalAlignment::Left,
            );
        } else if !self.text.is_empty() {
            // ── The selection, behind the glyphs ──
            //
            // Painted first, so the value's ink sits on top of it. Each logical line contributes
            // the part of the range that falls on it, which is what makes a multi-line selection
            // read as continuous instead of becoming one full-width bar down the whole control.
            // A band drawn *after* the text would cover the very words the user just selected,
            // which is why the order here is load-bearing rather than incidental.
            if let Some((sel_start, sel_end)) = self.selection_range() {
                let highlight = self.selection_color(&text_color, &field_bg);
                // One pass over the display rows, so a scrolled field highlights the same rows it
                // draws and a soft-wrapped line contributes one band per wrapped row.
                let mut y = text_top;
                for row in 0.. {
                    let Some(display) = self.display_row(row) else {
                        break;
                    };
                    // I-P62: stop at the bottom edge. The loop used to walk every display row of
                    // the document, doing overlap and band-width work for rows far below the
                    // viewport.
                    if y + LINE_H > rect.y + rect.height as i32 {
                        break;
                    }
                    // The overlap of `[sel_start, sel_end)` with this display row's own content.
                    let from = sel_start.max(display.start).min(display.end);
                    let to = sel_end.min(display.end).max(from);
                    if to > from {
                        // Measured, not multiplied: the band's two edges have to land on the same
                        // pixels the glyphs do, and the glyphs are laid out with the face's own
                        // advances (see `advance_of_caret_prefix` for what a fixed width costs).
                        let left = origin_x + self.content_margin - self.scroll_x
                            + self.band_advance(display.start, from);
                        let width = self.band_advance(from, to);
                        if width > 0 {
                            // `fill_rect`, not `draw_rect`: the latter is a one-pixel outline,
                            // so a selected run painted a hollow box around the glyphs (the
                            // glyphs themselves were not highlighted at all). `TextEdit` fills
                            // the same band from the same arithmetic.
                            context.fill_rect(
                                Rect::new(left, y, width as u32, LINE_H as u32),
                                highlight,
                            );
                        }
                    }
                    y += LINE_H;
                }
            }

            let mut y = text_top;
            // Rows are drawn from the scroll position, so the field is a window onto the document
            // rather than always showing its first line. `display_row` is the one lookup of
            // "which text is on row n", shared with the pixel-to-offset map below, so a click and
            // the glyphs it lands on cannot disagree about the scroll or the wrap.
            for row in 0.. {
                let Some(display) = self.display_row(row) else {
                    break;
                };
                if y + LINE_H > rect.y + rect.height as i32 {
                    break;
                }
                context.draw_text(
                    Point::new(origin_x + self.content_margin - self.scroll_x, y),
                    &self.text[display.start..display.end],
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
                y += LINE_H;
            }
        }

        // -- Cursor --
        // I-116: the caret is only painted during the visible half of its blink cycle.
        if self.focused && self.cursor_blink.is_visible() {
            let cursor_x = self.cursor_screen_x(origin_x);
            let cursor_y = self.cursor_screen_y(text_top);
            context.draw_line(
                Point::new(cursor_x, cursor_y),
                Point::new(cursor_x, cursor_y + LINE_H),
                text_color,
            );

            // The uncommitted composition is painted inline at the caret, where it will land
            // when it is committed, with an underline marking it as preedit. This block is
            // already inside the interior clip, so a long preedit cannot paint over the border.
            if let Some(preedit) = self.composition.as_deref() {
                let preedit_width = context.measure_text(preedit, font).width as i32;
                context.draw_text(
                    Point::new(cursor_x, cursor_y),
                    preedit,
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
                let underline_y = cursor_y + LINE_H;
                context.draw_line(
                    Point::new(cursor_x, underline_y),
                    Point::new(cursor_x + preedit_width, underline_y),
                    text_color,
                );
            }
        }
        context.pop_clip();
        self.draw_scrollbar(context);

        // ── Focus ring ──
        //
        // Only a keyboard focus draws it: a pointer (or touch) press focuses the area without
        // claiming visual focus, so no ring appears under the cursor.
        if self.base.draws_focus_ring() {
            let ring = crate::widget::metrics::FocusRing::for_control(
                rect,
                self.style().border_radius.unwrap_or(0),
            );
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    crate::widget::metrics::focus_ring_color(bg.contrast_color()),
                    crate::widget::metrics::FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

impl TextArea {
    /// Advances the caret blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate-wide `tick(delta_ms) -> bool` convention: a host that renders on demand keeps
    /// owning the clock, and a blurred area never asks for a frame (I-116).
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.focused {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }

    /// The scrollbar's track rectangle, or `None` when the value fits the viewport (I-118).
    fn scrollbar_track(&self) -> Option<(Rect, usize, usize)> {
        let total = self.line_spans().len();
        let visible = self.visible_line_count();
        if total <= visible {
            return None;
        }
        let rect = self.geometry();
        let width = 4u32;
        let track = Rect::new(
            rect.x + rect.width.saturating_sub(width + 2) as i32,
            rect.y.saturating_add(2),
            width,
            rect.height.saturating_sub(4),
        );
        Some((track, total, visible))
    }

    /// Hands the standard scrollbar the track and the metrics it needs.
    ///
    /// Returns the track when the value overflows the viewport. The companion then owns the
    /// thumb rectangle, the `pos`-to-offset mapping and the visible predicate, so the paint and
    /// the drag cannot disagree about the thumb they act on (I-128).
    fn sync_scrollbar(&mut self) -> Option<Rect> {
        let (track, total, visible) = self.scrollbar_track()?;
        self.scrollbar.set_track(track);
        self.scrollbar.set_metrics(
            total.min(u32::MAX as usize) as u32,
            visible.min(u32::MAX as usize) as u32,
            self.first_visible_line.min(u32::MAX as usize) as u32,
        );
        Some(track)
    }

    /// Whether `pos` is on the scrollbar track (and therefore grabs the thumb).
    pub(crate) fn scrollbar_contains(&self, pos: Point) -> bool {
        self.scrollbar_track().map(|(track, _, _)| track.contains_point(pos)).unwrap_or(false)
    }

    /// Scrolls the value so the scrollbar dragged to `y` lands there (I-118).
    pub(crate) fn scrollbar_scroll_to(&mut self, y: i32) {
        let Some(track) = self.sync_scrollbar() else {
            return;
        };
        let next = self.scrollbar.drag_thumb(Point::new(track.x, y)) as usize;
        if next != self.first_visible_line {
            self.first_visible_line = next;
        }
    }

    /// Paints the right-edge scrollbar when the value is taller than the viewport (I-118).
    fn draw_scrollbar(&mut self, context: &mut RenderContext) {
        let Some(track) = self.sync_scrollbar() else {
            return;
        };
        if !self.scrollbar.is_visible() {
            return;
        }
        let paper = self.style().background_color.unwrap_or(Color::WHITE);
        let ink = self.style().text_color.unwrap_or(Color::BLACK);
        let track_color = ink.blend(&paper, 0.18);
        let thumb_color = ink.blend(&paper, 0.45);
        context.fill_rect(track, track_color);
        let thumb = self.scrollbar.thumb_rect(track);
        if thumb.width > 0 && thumb.height > 0 {
            context.fill_rect(thumb, thumb_color);
        }
    }

    /// The resolved body font the control paints and measures text with.
    ///
    /// Draw and the caret/hit-test helpers must read the same font, so a themed or scaled body font
    /// reaches both the ink and the arithmetic that places the caret and maps a click to an offset.
    pub(crate) fn text_font(&self) -> &Font {
        effective_font(self.style())
    }

    /// The line box the effective font occupies, shared by the paint grid and the hit test.
    ///
    /// The paint path centres each row's glyph box in its [`LINE_H`] band via `text_line`, which
    /// derives the box from the font's **effective** line height — so the grid arithmetic must read
    /// the same number here rather than the point size (`size_i32`), which only equals the line
    /// height for a font with no explicit leading.
    pub(crate) fn line_box_height(&self) -> i32 {
        self.text_font().effective_line_height().max(1.0) as i32
    }

    /// The band colour a selection is painted in.
    ///
    /// Derived from the field's own ink and fill rather than a fixed blue, so a selection reads on
    /// the light *and* the dark appearance without a second theme token to keep in step — the same
    /// derivation the placeholder uses a few lines above. A literal here would have painted a
    /// light-theme highlight over a dark field.
    fn selection_color(&self, text_color: &Color, field_bg: &Color) -> Color {
        // Two steps, not one: blending the ink toward the fill has to land the band *between* the
        // two, because a single blend of black into white is either invisible behind the glyphs or
        // dark enough to swallow them.
        text_color.blend(field_bg, 0.3).blend(field_bg, 0.25)
    }

    /// Computes the screen-space X coordinate of the cursor.
    ///
    /// The prefix is sliced through [`floor_char_boundary`] rather than at `cursor_pos` directly:
    /// the caret is a byte index that `set_cursor_pos` now keeps on a boundary, and an *undo*
    /// restore can replace the value under it, so slicing defensively here keeps the paint path
    /// total whatever the caret holds. A draw that can panic is worse than a caret in the wrong
    /// place — it takes the frame down.
    fn cursor_screen_x(&self, origin_x: i32) -> i32 {
        origin_x + self.content_margin - self.scroll_x
            + self.advance_of_caret_prefix(self.cursor_pos)
    }

    /// The advance of `text[from..to]`, memoised for the current revision and font.
    pub(crate) fn band_advance(&self, from: usize, to: usize) -> i32 {
        let font = self.text_font();
        {
            let cache = self.band_width_cache.borrow();
            if let Some(cache) = cache.as_ref() {
                if cache.revision == self.revision && cache.font == *font {
                    if let Some(width) = cache.entries.get(&(from, to)) {
                        return *width;
                    }
                }
            }
        }
        let width = if from < to { self.text_advance(&self.text[from..to]) } else { 0 };
        let mut cache = self.band_width_cache.borrow_mut();
        let stale = match cache.as_ref() {
            Some(entry) => entry.revision != self.revision || entry.font != *font,
            None => true,
        };
        if stale
            || cache.as_ref().is_some_and(|entry| entry.entries.len() >= BAND_WIDTH_CACHE_LIMIT)
        {
            *cache = Some(BandWidthCache {
                revision: self.revision,
                font: font.clone(),
                entries: HashMap::new(),
            });
        }
        if let Some(entry) = cache.as_mut() {
            entry.entries.insert((from, to), width);
        }
        width
    }

    /// The advance of the text on the caret's line, before `offset`, in pixels.
    ///
    /// # Why this is measured and not multiplied
    ///
    /// The paint path draws each line through the renderer's own shaper, so the ink's width is
    /// whatever the *face* says. Locating the caret by `characters * CHAR_W` was therefore a second,
    /// disagreeing model of the same text — measured, for `Font::default()` at 14 px, as:
    ///
    /// ```text
    /// "i"       renderer 4   CHAR_W model 8     (4 px out)
    /// "中文"    renderer 28  CHAR_W model 16    (12 px out)
    /// ```
    ///
    /// A caret 12 px from its character is on the wrong character, and every hit test that agreed
    /// with it put the insertion point somewhere the user did not click. Measuring through
    /// `measure_text` — the entry point the renderer itself uses — makes the two the same answer by
    /// construction rather than by a constant that has to be kept in step.
    fn advance_of_caret_prefix(&self, offset: usize) -> i32 {
        let caret = floor_char_boundary(&self.text, offset.min(self.text.len()));
        // The display row is the unit the painter advances by: without soft wrap it starts at the
        // logical line, and with it, at the wrapped segment the caret sits on.
        let rows = self.display_rows();
        let row_start = rows.iter().rfind(|row| row.start <= caret).map_or(0, |row| row.start);
        // One cached measurement source for the caret: the same revision-keyed table the selection
        // band reads, rather than a fresh shape of the same prefix every frame (I-005).
        self.band_advance(row_start, caret)
    }

    /// The advance of `text` under the renderer's own metrics.
    ///
    /// A throwaway backend is used as a **measurement surface**: `RenderContext::measure_text`
    /// writes no command, so a zero-sized surface is enough and nothing is rasterised on a hit test
    /// or a caret draw.
    fn text_advance(&self, text: &str) -> i32 {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        context.measure_text(text, self.text_font()).width as i32
    }

    /// The byte offset the horizontal coordinate `x` points at, within the display row
    /// `start..end`.
    ///
    /// The row's per-character advances are cached by [`Self::revision`] and the font, so a press
    /// or drag step reads a table instead of shaping every glyph again (I-P09). The first click
    /// after an edit builds the table once; moving the caret does not invalidate it.
    pub(crate) fn column_at_x(&self, start: usize, end: usize, x: i32) -> usize {
        if start >= end {
            return 0;
        }
        let font = self.text_font().clone();
        let fresh = {
            let cache = self.line_advance_cache.borrow();
            match cache.as_ref() {
                Some(entry)
                    if entry.revision == self.revision
                        && entry.font == font
                        && entry.start == start
                        && entry.end == end =>
                {
                    Some(entry.cuts.clone())
                }
                _ => None,
            }
        };
        let cuts = match fresh {
            Some(cuts) => cuts,
            None => {
                let row = &self.text[start..end];
                let mut cuts: Vec<(usize, i32)> = Vec::with_capacity(row.len() + 1);
                cuts.push((0, 0));
                Self::with_measurement(|context| {
                    let mut advance = 0i32;
                    let mut glyph = [0u8; 4];
                    for (index, ch) in row.char_indices() {
                        advance +=
                            context.measure_text(ch.encode_utf8(&mut glyph), &font).width as i32;
                        cuts.push((index + ch.len_utf8(), advance));
                    }
                });
                let cached = LineAdvanceCache {
                    revision: self.revision,
                    font: font.clone(),
                    start,
                    end,
                    cuts: cuts.clone(),
                };
                *self.line_advance_cache.borrow_mut() = Some(cached);
                cuts
            }
        };
        let target = x.max(0);
        let mut best = 0usize;
        for window in cuts.windows(2) {
            let (_, previous) = window[0];
            let (offset, cumulative) = window[1];
            let advance = cumulative - previous;
            if target < previous + advance / 2 {
                break;
            }
            best = offset;
        }
        best
    }

    /// Runs `f` against a throwaway measurement surface.
    ///
    /// `measure_text` writes no command, so a zero-sized backend is enough and nothing is
    /// rasterised while a layout is computed.
    fn with_measurement<R>(f: impl FnOnce(&RenderContext) -> R) -> R {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        f(&RenderContext::new(&mut measurement))
    }

    /// Computes the screen-space Y coordinate of the cursor (top of cursor line).
    ///
    /// The caret's own line is measured **relative to the viewport**, so a scrolled field draws the
    /// marker on the row the character is actually painted on rather than at its document position —
    /// which would put it above the field, or below it, once anything had been scrolled past.
    fn cursor_screen_y(&self, origin_y: i32) -> i32 {
        let row = self.display_row_of(self.cursor_pos).saturating_sub(self.first_visible_line);
        origin_y + row as i32 * LINE_H
    }

    /// The caret's window rectangle and byte offset, as a native IME asks for them.
    ///
    /// # Why the horizontal position is measured without a paint pass
    ///
    /// The IME query is answered on a `&self` borrow between frames, when no `RenderContext` is
    /// available to the caller. The x is the **cached** advance of the text before the caret on
    /// its own line — the same measurement source [`Self::cursor_screen_x`] reads during a paint
    /// (I-005) — so the anchor and the ink cannot disagree. The vertical position is the caret's
    /// visible row, so a scrolled area reports the row it actually draws the marker on rather
    /// than the caret's document position.
    pub(crate) fn caret_query(&self) -> InputMethodQuery {
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        let caret_x =
            origin_x + self.content_margin - self.scroll_x + self.advance_of_caret_prefix(caret);

        // The first line's band is the same one `draw` lays the glyphs out in: one line tall, one
        // inset below the top edge. The line height is measured once per font and cached, so an
        // IME query does not build a throwaway paint backend on every keystroke (I-P09).
        let band = Rect::new(origin_x, rect.y + FIRST_LINE_INSET, rect.width, LINE_H as u32);
        let line_height = self.measured_line_height();
        let text_top = band.y + ((band.height as i32 - line_height) / 2).max(0);
        let row = self.display_row_of(self.cursor_pos).saturating_sub(self.first_visible_line);
        InputMethodQuery {
            caret_rect: Rect::new(caret_x, text_top + row as i32 * LINE_H, 1, LINE_H as u32),
            caret_byte: Some(caret),
        }
    }

    /// The renderer's line height for the current body font, measured once per font.
    ///
    /// `RenderContext::text_line` derives its box from `measure_text("M", font)`. Repeating that
    /// measurement for every IME query built a throwaway backend per call; this caches the one
    /// number it needs (I-P09).
    fn measured_line_height(&self) -> i32 {
        let font = self.text_font().clone();
        if let Some((cached_font, height)) = self.line_height_cache.borrow().as_ref() {
            if *cached_font == font {
                return *height as i32;
            }
        }
        let height =
            Self::with_measurement(|context| context.measure_text("M", &font).height).max(1);
        *self.line_height_cache.borrow_mut() = Some((font, height));
        height as i32
    }
}
