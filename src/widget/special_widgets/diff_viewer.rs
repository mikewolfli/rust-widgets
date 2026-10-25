// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DiffViewer widget.

/// The top of the first diff row below the column headings.
const ROW_TOP: i32 = 34;
/// The pitch between diff rows.
const ROW_HEIGHT: i32 = 16;
/// How much room below a row's top its height still needs.
const ROW_BOTTOM_INSET: i32 = 14;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::borrow::Cow;
use std::collections::HashMap;

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
    fn new(left: Option<String>, right: Option<String>, kind: DiffKind) -> Self {
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
    fn key_for(self, line: &str) -> Cow<'_, str> {
        match self {
            Self::Show => Cow::Borrowed(line),
            Self::IgnoreEol => Cow::Borrowed(line.trim_end()),
            Self::IgnoreAll | Self::MarkBlankChanges => Cow::Owned(collapse_whitespace(line)),
        }
    }

    /// Whether whitespace-only pairs are kept as `blank_only` rows.
    fn marks_blank_changes(self) -> bool {
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

/// Side-by-side diff viewer for text snapshots.
pub struct DiffViewer {
    base: BaseWidget,
    left: String,
    right: String,
    lines: Vec<DiffLine>,
    selected_index: Option<usize>,
    /// The first painted row the viewport shows.
    ///
    /// The paint path used to stop at a fixed twelve rows, so a longer comparison was not
    /// reachable. This is the scroll state the wheel drives; it indexes [`Self::display_rows`],
    /// not [`Self::lines`], because one line can occupy two rows in unified mode and a folded run
    /// occupies one.
    first_visible_line: usize,
    /// How the two sides are presented.
    view_mode: DiffViewMode,
    /// Whether long unchanged runs are collapsed to one marker row.
    fold_unchanged: bool,
    /// How whitespace differences are judged.
    whitespace_mode: WhitespaceMode,
    /// The shortest unchanged run that is folded away.
    fold_threshold: usize,
    /// How many unchanged rows are kept visible at each end of a folded run.
    fold_context_lines: usize,
    /// Whether a `Changed` row is painted as a `-` row and a `+` row in unified mode.
    unified_changed_pair: bool,
    /// Whether the inline change tint covers whole words instead of the exact changed run.
    ///
    /// The spans are derived from the rows themselves, so switching this recomputes them for the
    /// rows already on screen; a word-granularity diff therefore needs no new comparison and no
    /// change to [`DiffLine`]'s public shape.
    word_granularity: bool,
    /// Whether case differences are ignored by the comparison.
    ignore_case: bool,
    /// Whether blank lines are normalised to one key, so a blank-line reshuffle is not a change.
    ignore_blank_lines: bool,
    /// The start line of every folded run the reader expanded.
    ///
    /// Keyed by the **whole** unchanged run, not by the marker's position inside it: with context
    /// rows kept visible, one expanded run is painted from several positions and a per-marker key
    /// would fold the remainder again.
    expanded_hunks: Vec<(usize, usize)>,
    /// The painted rows derived from `lines`, `view_mode`, the fold settings and the expansions.
    display_rows: Vec<DisplayRow>,
    /// Measured widths of a changed row's inline run, keyed by display row and font size.
    ///
    /// The spans themselves are resolved when the rows are built; only their pixel widths need the
    /// renderer, so they are measured once per row and reused by every still frame.
    span_widths: SpanWidths,
    /// Whether `display_rows` needs rebuilding before the next use.
    display_dirty: bool,
    /// Emitted after compare. Payload is changed-lines count.
    pub compared: Signal1<usize>,
    /// Emitted when the selected row changes, with the selected line index.
    pub selection_changed: Signal1<Option<usize>>,
    /// Emitted when the presentation mode changes, with its token spelling.
    pub view_mode_changed: Signal1<String>,
    /// Emitted when folding is enabled or disabled.
    pub fold_changed: Signal1<bool>,
    /// Emitted when a folded hunk is expanded, with the line index the run starts at.
    pub hunk_expanded: Signal1<usize>,
}

/// Which side of a row is painted by one display row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowSide {
    /// Both columns (side-by-side).
    Both,
    /// The left column only (a unified `-` row).
    Left,
    /// The right column only (a unified `+`, `~` or context row).
    Right,
}

/// What one painted row shows.
#[derive(Debug, Clone, Copy)]
enum RowSource {
    /// One compared line, painted on `side`.
    Line { index: usize, side: RowSide },
    /// A folded run of unchanged lines. `run` is the whole unchanged range (the expansion key);
    /// the marker's caption is built with the row.
    Fold { run: (usize, usize) },
}

/// One painted row, with the gutter numbers it shows precomputed.
///
/// The gutter text is built with the row, not at paint time: formatting a number per visible row
/// per frame allocated a string for every row of every still frame.
#[derive(Debug, Clone)]
struct DisplayRow {
    source: RowSource,
    /// The left gutter's source number, kept for the structure tests.
    #[cfg_attr(not(test), allow(dead_code))]
    left_number: Option<usize>,
    /// The right gutter's source number, kept for the structure tests.
    #[cfg_attr(not(test), allow(dead_code))]
    right_number: Option<usize>,
    /// The gutter caption painted in the left column (or the single column in unified mode).
    left_gutter: Option<String>,
    /// The gutter caption painted in the right column.
    right_gutter: Option<String>,
    /// The fold marker's caption, when this row stands in for a folded run.
    fold_label: Option<String>,
}

/// The measured inline-run widths for each display row and font size.
type SpanWidths = HashMap<(usize, u32), ((i32, u32), (i32, u32))>;

impl DiffViewer {
    /// Creates empty diff viewer.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "DiffViewer"),
            left: String::new(),
            right: String::new(),
            lines: Vec::new(),
            selected_index: None,
            first_visible_line: 0,
            view_mode: DiffViewMode::SideBySide,
            fold_unchanged: false,
            whitespace_mode: WhitespaceMode::default(),
            fold_threshold: 8,
            fold_context_lines: 0,
            unified_changed_pair: true,
            word_granularity: false,
            ignore_case: false,
            ignore_blank_lines: false,
            expanded_hunks: Vec::new(),
            display_rows: Vec::new(),
            span_widths: HashMap::new(),
            display_dirty: true,
            compared: Signal1::new(),
            selection_changed: Signal1::new(),
            view_mode_changed: Signal1::new(),
            fold_changed: Signal1::new(),
            hunk_expanded: Signal1::new(),
        }
    }

    /// Sets both text snapshots and recomputes diff.
    pub fn set_texts(&mut self, left: impl Into<String>, right: impl Into<String>) {
        self.left = left.into();
        self.right = right.into();
        self.first_visible_line = 0;
        self.expanded_hunks.clear();
        self.recompute();
    }

    /// Returns diff lines.
    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    /// Returns selected line index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Selects line index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.lines.len() {
            return false;
        }
        self.set_selection(Some(index));
        self.base.request_redraw();
        true
    }

    /// Stores a selection and reports the change.
    fn set_selection(&mut self, next: Option<usize>) {
        if self.selected_index == next {
            return;
        }
        self.selected_index = next;
        self.selection_changed.emit(next);
    }

    /// Returns count of non-equal lines.
    pub fn change_count(&self) -> usize {
        self.lines.iter().filter(|line| line.kind != DiffKind::Equal).count()
    }

    /// The presentation mode.
    pub fn view_mode(&self) -> DiffViewMode {
        self.view_mode
    }

    /// Sets the presentation mode.
    pub fn set_view_mode(&mut self, mode: DiffViewMode) {
        if self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
        self.display_dirty = true;
        self.view_mode_changed.emit(mode.as_str().to_string());
        self.base.request_redraw();
    }

    /// Whether inline changes are tinted word-by-word (`true`) or byte-by-byte (`false`).
    pub fn word_granularity(&self) -> bool {
        self.word_granularity
    }

    /// Chooses the inline change granularity.
    ///
    /// A word-granularity view widens the changed run to the word boundaries it touches, which is
    /// what a reader expects from a prose or code diff: `alpha` → `alphabet` tints `alphabet`
    /// rather than just the appended `bet`. The comparison itself is untouched, so the row set and
    /// [`Self::change_count`] are the same either way.
    pub fn set_word_granularity(&mut self, enabled: bool) {
        if self.word_granularity == enabled {
            return;
        }
        self.word_granularity = enabled;
        self.refresh_changed_spans();
        self.base.request_redraw();
    }

    /// Recomputes every changed row's inline spans under the current granularity.
    fn refresh_changed_spans(&mut self) {
        for line in &mut self.lines {
            let Some(((left_start, left_end), (right_start, right_end))) = line.changed_spans
            else {
                continue;
            };
            let (Some(left), Some(right)) = (line.left.as_deref(), line.right.as_deref()) else {
                continue;
            };
            if self.word_granularity {
                line.changed_spans = Some((
                    word_spans(left, (left_start, left_end)),
                    word_spans(right, (right_start, right_end)),
                ));
            } else {
                line.changed_spans = Some(differing_spans(left, right));
            }
        }
    }

    /// Enables collapsing long unchanged runs into one marker row.
    pub fn set_fold_unchanged(&mut self, fold: bool) {
        if self.fold_unchanged == fold {
            return;
        }
        self.fold_unchanged = fold;
        self.first_visible_line = 0;
        self.display_dirty = true;
        self.fold_changed.emit(fold);
        self.base.request_redraw();
    }

    /// Returns whether unchanged runs are folded.
    pub fn fold_unchanged(&self) -> bool {
        self.fold_unchanged
    }

    /// How whitespace differences are judged.
    pub fn whitespace_mode(&self) -> WhitespaceMode {
        self.whitespace_mode
    }

    /// Sets how whitespace differences are judged, recomputing the comparison.
    pub fn set_whitespace_mode(&mut self, mode: WhitespaceMode) {
        if self.whitespace_mode == mode {
            return;
        }
        self.whitespace_mode = mode;
        self.expanded_hunks.clear();
        self.recompute();
    }

    /// Whether case differences are ignored.
    pub fn ignore_case(&self) -> bool {
        self.ignore_case
    }

    /// Ignores case in the comparison, recomputing it.
    pub fn set_ignore_case(&mut self, ignore: bool) {
        if self.ignore_case == ignore {
            return;
        }
        self.ignore_case = ignore;
        self.expanded_hunks.clear();
        self.recompute();
    }

    /// Whether blank lines are normalised before comparison.
    pub fn ignore_blank_lines(&self) -> bool {
        self.ignore_blank_lines
    }

    /// Normalises blank lines before comparison, recomputing it.
    ///
    /// A blank line and a whitespace-only line then compare equal, so a reformat that only moves
    /// blank lines around no longer shows up as edits.
    pub fn set_ignore_blank_lines(&mut self, ignore: bool) {
        if self.ignore_blank_lines == ignore {
            return;
        }
        self.ignore_blank_lines = ignore;
        self.expanded_hunks.clear();
        self.recompute();
    }

    /// The comparison key for one line under every active rule.
    fn comparison_key<'a>(&self, line: &'a str) -> Cow<'a, str> {
        if self.ignore_blank_lines && line.trim().is_empty() {
            return Cow::Borrowed("");
        }
        let key = self.whitespace_mode.key_for(line);
        if self.ignore_case {
            match key {
                Cow::Borrowed(text) => Cow::Owned(text.to_lowercase()),
                Cow::Owned(text) => Cow::Owned(text.to_lowercase()),
            }
        } else {
            key
        }
    }

    /// The shortest unchanged run that is folded away.
    pub fn fold_threshold(&self) -> usize {
        self.fold_threshold
    }

    /// Sets the shortest unchanged run that is folded away.
    pub fn set_fold_threshold(&mut self, threshold: usize) {
        let threshold = threshold.max(1);
        if self.fold_threshold == threshold {
            return;
        }
        self.fold_threshold = threshold;
        self.first_visible_line = 0;
        self.display_dirty = true;
        self.base.request_redraw();
    }

    /// How many unchanged rows are kept visible at each end of a folded run.
    pub fn fold_context_lines(&self) -> usize {
        self.fold_context_lines
    }

    /// Sets how many unchanged rows are kept visible at each end of a folded run.
    pub fn set_fold_context_lines(&mut self, lines: usize) {
        if self.fold_context_lines == lines {
            return;
        }
        self.fold_context_lines = lines;
        self.first_visible_line = 0;
        self.display_dirty = true;
        self.base.request_redraw();
    }

    /// Whether unified mode paints a changed row as a `-` row and a `+` row.
    pub fn unified_changed_pair(&self) -> bool {
        self.unified_changed_pair
    }

    /// Sets whether unified mode paints a changed row as a `-` row and a `+` row.
    ///
    /// With the pair disabled the row is painted as a single `~` line carrying only the new
    /// text, which is what the viewer used to do and what lost the old text entirely.
    pub fn set_unified_changed_pair(&mut self, pair: bool) {
        if self.unified_changed_pair == pair {
            return;
        }
        self.unified_changed_pair = pair;
        self.first_visible_line = 0;
        self.display_dirty = true;
        self.base.request_redraw();
    }

    /// Expands the folded hunk at display row `row`.
    ///
    /// Returns `false` when the row is not a fold marker. Expanding emits `hunk_expanded` with
    /// the line index the run starts at, which is what a host needs to restore the same view.
    pub fn expand_hunk(&mut self, row: usize) -> bool {
        self.ensure_display_rows();
        let Some(DisplayRow { source: RowSource::Fold { run, .. }, .. }) =
            self.display_rows.get(row).cloned()
        else {
            return false;
        };
        if self.expanded_hunks.contains(&run) {
            return false;
        }
        self.expanded_hunks.push(run);
        self.display_dirty = true;
        self.hunk_expanded.emit(run.0);
        self.base.request_redraw();
        true
    }

    /// The number of painted rows.
    pub fn display_row_count(&mut self) -> usize {
        self.ensure_display_rows();
        self.display_rows.len()
    }

    /// Selects the next changed row after the current selection.
    pub fn next_change(&mut self) -> bool {
        let start = self.selected_index.map_or(0, |index| index + 1);
        if let Some(index) =
            (start..self.lines.len()).find(|index| self.lines[*index].kind != DiffKind::Equal)
        {
            let _ = self.select_index(index);
            self.reveal_line(index);
            true
        } else {
            false
        }
    }

    /// Selects the previous changed row before the current selection.
    pub fn previous_change(&mut self) -> bool {
        let end = self.selected_index.unwrap_or(self.lines.len());
        if let Some(index) = (0..end).rev().find(|index| self.lines[*index].kind != DiffKind::Equal)
        {
            let _ = self.select_index(index);
            self.reveal_line(index);
            true
        } else {
            false
        }
    }

    /// Scrolls `index` into view.
    fn reveal_line(&mut self, index: usize) {
        self.ensure_display_rows();
        let Some(row) = self.display_row_of_line(index) else {
            return;
        };
        let visible = self.visible_line_count();
        if row < self.first_visible_line {
            self.first_visible_line = row;
        } else if row >= self.first_visible_line + visible {
            self.first_visible_line = row + 1 - visible;
        }
        self.base.request_redraw();
    }

    /// The display row that paints line `index`, if the line is not folded away.
    fn display_row_of_line(&self, index: usize) -> Option<usize> {
        self.display_rows.iter().position(|row| match row.source {
            RowSource::Line { index: painted, .. } => painted == index,
            RowSource::Fold { run, .. } => index >= run.0 && index < run.1,
        })
    }

    /// The selected row as patch text, for copying.
    pub fn selected_patch(&self) -> Option<String> {
        let index = self.selected_index?;
        let line = self.lines.get(index)?;
        Some(match line.kind {
            DiffKind::Equal => line.left.clone().map(|text| format!(" {text}")).unwrap_or_default(),
            DiffKind::Added => {
                line.right.clone().map(|text| format!("+{text}")).unwrap_or_default()
            }
            DiffKind::Removed => {
                line.left.clone().map(|text| format!("-{text}")).unwrap_or_default()
            }
            DiffKind::Changed => format!(
                "-{}\n+{}",
                line.left.clone().unwrap_or_default(),
                line.right.clone().unwrap_or_default()
            ),
        })
    }

    /// How many diff rows fit below the column headings.
    ///
    /// At least one, so a short viewer still paints a row instead of scrolling everything away.
    fn visible_line_count(&self) -> usize {
        let usable = self.geometry().height as i32 - ROW_TOP - ROW_BOTTOM_INSET;
        if usable < 0 {
            return 1;
        }
        (usable / ROW_HEIGHT + 1) as usize
    }

    /// The first row the viewport actually starts at, clamped to the last full window.
    fn visible_start(&self) -> usize {
        let max_first = self.display_rows.len().saturating_sub(self.visible_line_count());
        self.first_visible_line.min(max_first)
    }

    /// Scrolls the viewport by `lines` rows toward the end of the comparison.
    ///
    /// Positive values reveal later rows, negative values reveal earlier ones. Returns whether the
    /// viewport moved, so the caller repaints only on a change.
    fn scroll_lines(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        self.ensure_display_rows();
        let max_first = self.display_rows.len().saturating_sub(self.visible_line_count());
        let next =
            (self.first_visible_line as i64 + lines as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_line {
            return false;
        }
        self.first_visible_line = next;
        true
    }

    /// Rebuilds the display rows when the inputs changed.
    fn ensure_display_rows(&mut self) {
        if !self.display_dirty {
            return;
        }
        self.display_dirty = false;
        self.display_rows.clear();
        // Measured span widths below are keyed by display row, so they belong to the rows being
        // rebuilt.
        self.span_widths.clear();
        let mut left_number = 1usize;
        let mut right_number = 1usize;
        let mut index = 0usize;
        while index < self.lines.len() {
            let line = &self.lines[index];
            if self.fold_unchanged && line.kind == DiffKind::Equal {
                let start = index;
                let mut end = index;
                while end < self.lines.len() && self.lines[end].kind == DiffKind::Equal {
                    end += 1;
                }
                let run = end - start;
                let context = self.fold_context_lines.min(run / 2);
                let expanded =
                    self.expanded_hunks.iter().any(|(first, last)| *first <= start && end <= *last);
                let folded = run > self.fold_threshold && run > context * 2 && !expanded;
                if folded {
                    for offset in 0..context {
                        self.push_line_row(
                            start + offset,
                            RowSide::Both,
                            false,
                            &mut left_number,
                            &mut right_number,
                        );
                    }
                    let fold_start = start + context;
                    let fold_end = end - context;
                    self.display_rows.push(DisplayRow {
                        source: RowSource::Fold { run: (start, end) },
                        left_number: None,
                        right_number: None,
                        left_gutter: None,
                        right_gutter: None,
                        fold_label: Some(if self.fold_unchanged {
                            format!(
                                "\u{22ef} {} unchanged lines \u{2014} click to expand",
                                fold_end.saturating_sub(fold_start)
                            )
                        } else {
                            format!(
                                "\u{22ef} {} unchanged lines",
                                fold_end.saturating_sub(fold_start)
                            )
                        }),
                    });
                    // The numbers are skipped, not reset: the rows the marker stands for are
                    // still part of each snapshot's line count.
                    left_number += fold_end.saturating_sub(fold_start);
                    right_number += fold_end.saturating_sub(fold_start);
                    for offset in (end - context)..end {
                        self.push_line_row(
                            offset,
                            RowSide::Both,
                            false,
                            &mut left_number,
                            &mut right_number,
                        );
                    }
                    index = end;
                    continue;
                }
            }
            match self.view_mode {
                DiffViewMode::SideBySide => {
                    self.push_line_row(
                        index,
                        RowSide::Both,
                        false,
                        &mut left_number,
                        &mut right_number,
                    );
                }
                DiffViewMode::Unified => {
                    let paired = self.unified_changed_pair
                        && line.kind == DiffKind::Changed
                        && line.left.is_some()
                        && line.right.is_some();
                    if paired {
                        self.push_line_row(
                            index,
                            RowSide::Left,
                            true,
                            &mut left_number,
                            &mut right_number,
                        );
                        self.push_line_row(
                            index,
                            RowSide::Right,
                            true,
                            &mut left_number,
                            &mut right_number,
                        );
                    } else {
                        self.push_line_row(
                            index,
                            RowSide::Right,
                            false,
                            &mut left_number,
                            &mut right_number,
                        );
                    }
                }
            }
            index += 1;
        }
    }

    /// Pushes the display row for `index` on `side`, advancing the gutter counters.
    fn push_line_row(
        &mut self,
        index: usize,
        side: RowSide,
        defer_left: bool,
        left_number: &mut usize,
        right_number: &mut usize,
    ) {
        let line = &self.lines[index];
        let left_number_for_row = line.left.as_ref().map(|_| *left_number);
        let right_number_for_row = line.right.as_ref().map(|_| *right_number);
        let (left_gutter, right_gutter) = match side {
            RowSide::Both => (
                left_number_for_row.map(|number| format!("{number:>4}")),
                right_number_for_row.map(|number| format!("{number:>4}")),
            ),
            RowSide::Left => (left_number_for_row.map(|number| format!("{number:>4} -")), None),
            RowSide::Right => {
                let prefix = match line.kind {
                    DiffKind::Added => "+",
                    DiffKind::Removed => "-",
                    DiffKind::Changed => {
                        if self.unified_changed_pair {
                            "+"
                        } else {
                            "~"
                        }
                    }
                    DiffKind::Equal => " ",
                };
                // A removed row in unified mode carries the left snapshot's number, exactly as
                // the paint path used to resolve it.
                let number = if matches!(line.kind, DiffKind::Removed) {
                    left_number_for_row
                } else {
                    right_number_for_row
                };
                (number.map(|number| format!("{number:>4} {prefix}")), None)
            }
        };
        self.display_rows.push(DisplayRow {
            source: RowSource::Line { index, side },
            left_number: left_number_for_row,
            right_number: right_number_for_row,
            left_gutter,
            right_gutter,
            fold_label: None,
        });
        // In a unified pair the left row is followed by the right row, so only the row that owns
        // each side advances that side's counter.
        let advance_left = !defer_left || side == RowSide::Left;
        let advance_right = !defer_left || side == RowSide::Right;
        if line.left.is_some() && advance_left {
            *left_number += 1;
        }
        if line.right.is_some() && advance_right {
            *right_number += 1;
        }
    }

    fn recompute(&mut self) {
        let left_lines: Vec<&str> = self.left.lines().collect();
        let right_lines: Vec<&str> = self.right.lines().collect();
        // Comparison keys: what the algorithm judges, while the rows keep showing the raw text.
        // Normalizing once per snapshot is what makes an "ignore whitespace" mode a comparison
        // rule rather than a rewrite of what the reader sees. The keys borrow the raw lines in
        // the common mode, so a comparison no longer copies both documents line by line.
        let left_keys: Vec<Cow<'_, str>> =
            left_lines.iter().map(|line| self.comparison_key(line)).collect();
        let right_keys: Vec<Cow<'_, str>> =
            right_lines.iter().map(|line| self.comparison_key(line)).collect();
        let left_refs: Vec<&str> = left_keys.iter().map(|key| key.as_ref()).collect();
        let right_refs: Vec<&str> = right_keys.iter().map(|key| key.as_ref()).collect();
        let mode = self.whitespace_mode;

        self.lines.clear();

        // # Size guard
        //
        // The Myers trace keeps one `V` snapshot per edit distance `D`, so its memory is
        // O((N + M) * D). The depth is bounded inside `myers_diff` by a byte budget; this guard
        // covers the other axis, a line grid so large that even a shallow trace would be
        // expensive. Both fall back to the positional comparison, which misreports insertions as
        // changes but is linear and never allocates a trace.
        if (left_lines.len() + 1).saturating_mul(right_lines.len() + 1) > 4_000_000 {
            let max_len = left_lines.len().max(right_lines.len());
            for idx in 0..max_len {
                let l = left_lines.get(idx).copied();
                let r = right_lines.get(idx).copied();
                let same = match (l, r) {
                    (Some(_), Some(_)) => left_refs[idx] == right_refs[idx],
                    _ => false,
                };
                let kind = match (l, r) {
                    (Some(_), Some(_)) if same => DiffKind::Equal,
                    (Some(_), Some(_)) => DiffKind::Changed,
                    (Some(_), None) => DiffKind::Removed,
                    (None, Some(_)) => DiffKind::Added,
                    (None, None) => DiffKind::Equal,
                };
                self.lines.push(DiffLine::new(
                    l.map(|s| s.to_string()),
                    r.map(|s| s.to_string()),
                    kind,
                ));
            }
        } else {
            self.lines = build_diff_lines_with(
                &left_lines,
                &right_lines,
                &left_refs,
                &right_refs,
                mode.marks_blank_changes(),
            );
        }

        if self.word_granularity {
            self.refresh_changed_spans();
        }

        let next_selection = if self.lines.is_empty() { None } else { Some(0) };
        self.set_selection(next_selection);
        self.display_dirty = true;
        self.ensure_display_rows();
        self.compared.emit(self.change_count());
        self.base.request_layout();
        self.base.request_redraw();
    }
}

/// One primitive step of an edit script.
///
/// The indexes are line indexes into the left (`Equal`/`Delete`) and right
/// (`Equal`/`Insert`) snapshots, so the script is a lossless description of how
/// the left document turns into the right one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditOp {
    /// Left line `l` and right line `r` are the same line.
    Equal(usize, usize),
    /// Left line `l` has no counterpart on the right.
    Delete(usize),
    /// Right line `r` has no counterpart on the left.
    Insert(usize),
}

/// Computes a shortest edit script with Myers' O(ND) greedy algorithm.
///
/// `V[k]` tracks the furthest `x` reached on diagonal `k = x - y`; one snapshot of
/// `V` is saved per round `d`, which is what the linear backtrack needs to recover
/// the steps. The script is assembled backwards from the end and then reversed.
///
/// # Memory bound
///
/// One `V` snapshot per round makes the trace O((N + M) * D), which for two large and wholly
/// different documents is unbounded in practice: at the 4-million-cell grid guard a 2 000-line
/// pair can still ask for hundreds of megabytes. The depth is therefore capped by a byte budget
/// ([`MYERS_TRACE_BUDGET_BYTES`]); when the edit distance does not fit, `None` is returned and the
/// caller falls back to the linear positional comparison rather than paying an unbounded cost.
const MYERS_TRACE_BUDGET_BYTES: usize = 8 * 1024 * 1024;

fn myers_diff(a: &[&str], b: &[&str]) -> Option<Vec<EditOp>> {
    let n = a.len();
    let m = b.len();
    let max = n + m;

    // `V` is indexed by `k` shifted by `offset`. The two spare slots cover the
    // `k - 1` / `k + 1` probes at the widest round and keep the empty-input case
    // (`max == 0`) in bounds.
    let offset = max + 1;
    let width = 2 * max + 3;
    let mut v = vec![0isize; width];
    let per_snapshot = width.saturating_mul(core::mem::size_of::<isize>()).max(1);
    let max_depth = (MYERS_TRACE_BUDGET_BYTES / per_snapshot).max(1);
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut reached = false;

    for (rounds, d) in (0..=max).enumerate() {
        if rounds >= max_depth {
            // The edit distance outgrew the trace budget; reporting that is the honest answer.
            return None;
        }
        trace.push(v.clone());
        let d = d as isize;
        let mut k = -d;
        while k <= d {
            let down = (k + offset as isize + 1) as usize;
            let right = (k + offset as isize - 1) as usize;
            let mut x =
                if k == -d || (k != d && v[right] < v[down]) { v[down] } else { v[right] + 1 };
            let mut y = x - k;
            while x < n as isize && y < m as isize && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(k + offset as isize) as usize] = x;
            if x >= n as isize && y >= m as isize {
                reached = true;
            }
            k += 2;
        }
        if reached {
            break;
        }
    }

    let mut ops = Vec::new();
    let mut x = n as isize;
    let mut y = m as isize;
    let mut d = (trace.len() - 1) as isize;
    while d >= 0 {
        let v = &trace[d as usize];
        let k = x - y;
        let prev_k = if k == -d
            || (k != d
                && v[(k - 1 + offset as isize) as usize] < v[(k + 1 + offset as isize) as usize])
        {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[(prev_k + offset as isize) as usize];
        let prev_y = prev_x - prev_k;
        // Walk back along the snake (the equal lines the forward pass skipped).
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            ops.push(EditOp::Equal(x as usize, y as usize));
        }
        if d > 0 {
            if x == prev_x {
                y -= 1;
                ops.push(EditOp::Insert(y as usize));
            } else {
                x -= 1;
                ops.push(EditOp::Delete(x as usize));
            }
        }
        x = prev_x;
        y = prev_y;
        d -= 1;
    }
    ops.reverse();
    Some(ops)
}

/// Turns an edit script into display rows.
///
/// A maximal run of `Delete`/`Insert` steps between two `Equal` steps is one change
/// region. Its deletions and insertions are paired positionally into `Changed` rows
/// (the existing side-by-side meaning) and any leftovers become `Removed` / `Added`
/// rows, which is exactly the `left`/`right: Option<String>` shape of [`DiffLine`].
///
/// `a_cmp` / `b_cmp` are the comparison keys (the same lines under the active
/// [`WhitespaceMode`]) while `a` / `b` are the text the rows display. Keeping the two apart is
/// what lets an ignore-whitespace mode change what counts as a change without changing what the
/// reader sees.
fn build_diff_lines_with(
    a: &[&str],
    b: &[&str],
    a_cmp: &[&str],
    b_cmp: &[&str],
    mark_blank: bool,
) -> Vec<DiffLine> {
    // The unchanged head and tail are already known to match, so Myers runs only over
    // the middle that can actually differ. Without this trim, editing one line near the
    // top of a long document made the edit script (and its trace) span the whole file.
    let prefix = a_cmp.iter().zip(b_cmp.iter()).take_while(|(left, right)| left == right).count();
    let mut suffix = 0usize;
    while suffix < a_cmp.len().saturating_sub(prefix)
        && suffix < b_cmp.len().saturating_sub(prefix)
        && a_cmp[a_cmp.len() - 1 - suffix] == b_cmp[b_cmp.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let a_middle = &a[prefix..a.len() - suffix];
    let b_middle = &b[prefix..b.len() - suffix];
    let a_cmp_middle = &a_cmp[prefix..a_cmp.len() - suffix];
    let b_cmp_middle = &b_cmp[prefix..b_cmp.len() - suffix];

    let mut lines = Vec::with_capacity(prefix + a_middle.len().max(b_middle.len()) + suffix);
    for index in 0..prefix {
        let kind =
            if mark_blank && a[index] != b[index] { DiffKind::Changed } else { DiffKind::Equal };
        lines.push(DiffLine::new(Some(a[index].to_string()), Some(b[index].to_string()), kind));
    }

    // A trace that outgrew its byte budget is reported as `None`; the positional fallback keeps
    // the comparison linear at the cost of reporting insertions as changes.
    let ops = myers_diff(a_cmp_middle, b_cmp_middle).unwrap_or_else(|| {
        (0..a_cmp_middle.len().max(b_cmp_middle.len()))
            .flat_map(|index| {
                let mut pair = Vec::new();
                if index < a_cmp_middle.len() {
                    pair.push(EditOp::Delete(index));
                }
                if index < b_cmp_middle.len() {
                    pair.push(EditOp::Insert(index));
                }
                pair
            })
            .collect()
    });
    let mut idx = 0usize;
    while idx < ops.len() {
        match ops[idx] {
            EditOp::Equal(l, r) => {
                let kind = if mark_blank && a_middle[l] != b_middle[r] {
                    DiffKind::Changed
                } else {
                    DiffKind::Equal
                };
                lines.push(DiffLine::new(
                    Some(a_middle[l].to_string()),
                    Some(b_middle[r].to_string()),
                    kind,
                ));
                idx += 1;
            }
            EditOp::Delete(_) | EditOp::Insert(_) => {
                let mut deletes = Vec::new();
                let mut inserts = Vec::new();
                while idx < ops.len() {
                    match ops[idx] {
                        EditOp::Equal(_, _) => break,
                        EditOp::Delete(l) => deletes.push(l),
                        EditOp::Insert(r) => inserts.push(r),
                    }
                    idx += 1;
                }
                let paired = deletes.len().min(inserts.len());
                for (&l, &r) in deletes.iter().zip(inserts.iter()).take(paired) {
                    lines.push(DiffLine::new(
                        Some(a_middle[l].to_string()),
                        Some(b_middle[r].to_string()),
                        DiffKind::Changed,
                    ));
                }
                for &l in &deletes[paired..] {
                    lines.push(DiffLine::new(
                        Some(a_middle[l].to_string()),
                        None,
                        DiffKind::Removed,
                    ));
                }
                for &r in &inserts[paired..] {
                    lines.push(DiffLine::new(None, Some(b_middle[r].to_string()), DiffKind::Added));
                }
            }
        }
    }
    for offset in 0..suffix {
        let left = a.len() - suffix + offset;
        let right = b.len() - suffix + offset;
        let kind =
            if mark_blank && a[left] != b[right] { DiffKind::Changed } else { DiffKind::Equal };
        lines.push(DiffLine::new(Some(a[left].to_string()), Some(b[right].to_string()), kind));
    }
    lines
}

impl Widget for DiffViewer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(500, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "compared" => Some(EventSignalRef::mapped("compared", &self.compared, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    match v {
                        Some(index) => CapabilityValue::UInt(*index as u64),
                        None => CapabilityValue::Null,
                    }
                }))
            }
            "view_mode_changed" => {
                Some(EventSignalRef::mapped("view_mode_changed", &self.view_mode_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "fold_changed" => {
                Some(EventSignalRef::mapped("fold_changed", &self.fold_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "hunk_expanded" => {
                Some(EventSignalRef::mapped("hunk_expanded", &self.hunk_expanded, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `DiffViewer`'s property contract.
///
/// Both texts are writable, but each name must be written through
/// `set_texts` — which recomputes the diff and emits `compared` — rather than
/// stored directly, so a reader never sees stale `line_count` / `change_count`
/// after a text write.
impl WidgetProperties for DiffViewer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "left_text" => Ok(CapabilityValue::String(self.left.clone())),
            "right_text" => Ok(CapabilityValue::String(self.right.clone())),
            "line_count" => Ok(CapabilityValue::UInt(self.lines().len() as u64)),
            "change_count" => Ok(CapabilityValue::UInt(self.change_count() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "view_mode" => Ok(CapabilityValue::String(self.view_mode.as_str().to_string())),
            "fold_unchanged" => Ok(CapabilityValue::Bool(self.fold_unchanged)),
            "fold_threshold" => Ok(CapabilityValue::UInt(self.fold_threshold as u64)),
            "fold_context_lines" => Ok(CapabilityValue::UInt(self.fold_context_lines as u64)),
            "unified_changed_pair" => Ok(CapabilityValue::Bool(self.unified_changed_pair)),
            "word_granularity" => Ok(CapabilityValue::Bool(self.word_granularity)),
            "whitespace_mode" => {
                Ok(CapabilityValue::String(self.whitespace_mode.as_str().to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "left_text" => match value {
                CapabilityValue::String(text) => {
                    let right = self.right.clone();
                    self.set_texts(text, right);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "right_text" => match value {
                CapabilityValue::String(text) => {
                    let left = self.left.clone();
                    self.set_texts(left, text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "line_count" | "change_count" | "selected_index" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "view_mode" => {
                let token = expect_string(value)?;
                let mode =
                    DiffViewMode::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_view_mode(mode);
                Ok(())
            }
            "fold_unchanged" => {
                self.set_fold_unchanged(expect_bool(value)?);
                Ok(())
            }
            "fold_threshold" => {
                self.set_fold_threshold(expect_usize(value)?);
                Ok(())
            }
            "fold_context_lines" => {
                self.set_fold_context_lines(expect_usize(value)?);
                Ok(())
            }
            "unified_changed_pair" => {
                self.set_unified_changed_pair(expect_bool(value)?);
                Ok(())
            }
            "word_granularity" => {
                self.set_word_granularity(expect_bool(value)?);
                Ok(())
            }
            "whitespace_mode" => {
                let token = expect_string(value)?;
                let mode = WhitespaceMode::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_whitespace_mode(mode);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "left_text",
            "right_text",
            "line_count",
            "change_count",
            "selected_index",
            "view_mode",
            "fold_unchanged",
            "fold_threshold",
            "fold_context_lines",
            "unified_changed_pair",
            "word_granularity",
            "whitespace_mode",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `diff_viewer` publishes.
    ///
    /// `select_index` names which diff line to select, and the control has no
    /// defensible default — line 0 of a two-sided diff is not "the interesting one",
    /// it is just the first — so a bare invocation is refused as
    /// [`CapabilityAccessError::OutOfRange`]: the name is valid and the index is what
    /// is missing. The property layer treats `selected_index` as read-only on purpose
    /// (see the note on `TimelineWidget` for the same rule), which is why the command
    /// is the only route and why it must not invent an argument. `set_texts` carries
    /// both snapshots.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "select_index" | "set_texts" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for DiffViewer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::KeyPress { key, modifiers } = event {
            match *key {
                38 => {
                    if let Some(index) = self.selected_index {
                        if index > 0 {
                            let _ = self.select_index(index - 1);
                        }
                    }
                }
                40 => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.lines.len() {
                            let _ = self.select_index(index + 1);
                        }
                    }
                }
                33 => {
                    self.previous_change();
                }
                34 => {
                    self.next_change();
                }
                13 => {
                    self.next_change();
                }
                _ if crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::PRIMARY)
                    && crate::shortcut::Key::from_key_code(*key)
                        == Some(crate::shortcut::Key::C) =>
                {
                    if let Some(patch) = self.selected_patch() {
                        let _ = crate::clipboard::ClipboardManager::set_text(patch);
                    }
                }
                _ => { /* Other keys are not relevant */ }
            }
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            // A click selects the row under it, or expands the fold marker under it. Rows, not
            // lines: a unified changed pair and a folded run both break the one-to-one mapping.
            let rect = self.geometry();
            let row = (pos.y - rect.y - ROW_TOP + 10) / ROW_HEIGHT;
            if row >= 0 {
                self.ensure_display_rows();
                let display_row = self.visible_start() + row as usize;
                match self.display_rows.get(display_row).map(|row| row.source) {
                    Some(RowSource::Fold { .. }) => {
                        self.expand_hunk(display_row);
                    }
                    Some(RowSource::Line { index, .. }) => {
                        let _ = self.select_index(index);
                    }
                    None => {}
                }
            }
        }
        // Wheel: positive `delta.y` moves the content down, so the viewport moves toward the
        // start. Three lines per notch is the desktop default shared with the text controls.
        if let Event::Wheel { delta, modifiers, .. } = event {
            let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
            if self.scroll_lines(-scroll.lines_y) {
                self.base.request_redraw();
            }
        }
    }
}

impl Draw for DiffViewer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // Rows are derived from the comparison and the fold state, so they are rebuilt once per
        // change rather than per frame. Done before the style borrow is taken because it needs
        // `&mut self`.
        self.ensure_display_rows();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes the appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("diff_viewer");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(background));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The split divider and the selection outline are secondary chrome, derived
        // from the resolved colours so they follow the appearance too.
        let divider = background.blend(&text_color, 0.15);
        // # Why the row rule reads `outline_variant` and not the blend above
        //
        // A row rule separates two *rows* of content; the focus ring is a different role. M6's
        // criterion is "the row line's colour is not the focus ring's", and the `outline_variant`
        // token is exactly the row-divider role the theme declares for it — the same one
        // `table_widget`, `virtual_table`, `data_table` and `tree_table` now read. A local blend
        // of the surface is a colour no theme can tune independently; the role is.
        let row_rule = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.outline_variant)
            .unwrap_or(divider);
        let selection = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .unwrap_or_else(|| background.blend(&text_color, 0.5));
        // Added / Removed / Changed are *states*, so they read the theme's semantic
        // tokens and are tinted over the resolved surface to stay legible in both
        // appearances.
        let added = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .map(|token| token.blend(&background, 0.85));
        let removed = crate::style::semantic_color(crate::style::SemanticColor::Error)
            .map(|token| token.blend(&background, 0.85));
        let changed = crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .map(|token| token.blend(&background, 0.85));

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        let mid_x = rect.x + (rect.width as i32 / 2);
        if self.view_mode == DiffViewMode::SideBySide {
            context.draw_line(
                Point::new(mid_x, rect.y),
                Point::new(mid_x, rect.y + rect.height as i32),
                divider,
            );
        }

        // The LEFT/RIGHT column headings and the diff line bodies take the **effective font**
        // (D09-STYLE-01), so the theme body font and the user's text scale reach them.
        let heading = if self.view_mode == DiffViewMode::Unified { "DIFF" } else { "LEFT" };
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 16),
            heading,
            effective_font(style),
            text_color,
            HorizontalAlignment::Left,
        );
        if self.view_mode == DiffViewMode::SideBySide {
            context.draw_text(
                Point::new(mid_x + 8, rect.y + 16),
                "RIGHT",
                effective_font(style),
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // Rows are built once per comparison / fold / expansion change, so the paint path does no
        // layout of its own: a changed row in unified mode is already two rows here and a folded
        // run is already one marker.
        let start = self.visible_start();
        let visible = self.visible_line_count();
        let font = effective_font(style);
        let rows = &self.display_rows;
        for (offset, display) in rows.iter().enumerate().skip(start).take(visible) {
            let y = rect.y + ROW_TOP + ((offset - start) as i32) * ROW_HEIGHT;
            if y + ROW_BOTTOM_INSET > rect.y + rect.height as i32 {
                break;
            }
            let (index, side) = match display.source {
                RowSource::Fold { .. } => {
                    // The marker is a control, not a decoration: clicking it restores the rows. Its
                    // caption was built with the row, so a still frame formats nothing.
                    context.draw_text(
                        Point::new(rect.x + 8, y),
                        display.fold_label.as_deref().unwrap_or(""),
                        font,
                        text_color.blend(&background, 0.4),
                        HorizontalAlignment::Left,
                    );
                    context.draw_line(
                        Point::new(rect.x + 2, y + 7),
                        Point::new(rect.x + rect.width as i32 - 2, y + 7),
                        row_rule,
                    );
                    continue;
                }
                RowSource::Line { index, side } => (index, side),
            };
            let Some(line) = self.lines.get(index) else {
                continue;
            };
            let bg = match line.kind {
                DiffKind::Equal => None,
                DiffKind::Added => added.or_else(|| Some(background.blend(&Color::GREEN, 0.12))),
                DiffKind::Removed => removed.or_else(|| Some(background.blend(&Color::RED, 0.12))),
                DiffKind::Changed => {
                    changed.or_else(|| Some(background.blend(&Color::YELLOW, 0.12)))
                }
            };
            if let Some(color) = bg {
                context.fill_rect(
                    Rect::new(rect.x + 2, y - 10, rect.width.saturating_sub(4), 14),
                    color,
                );
            }
            if self.selected_index == Some(index) {
                context.draw_rect(
                    Rect::new(rect.x + 2, y - 11, rect.width.saturating_sub(4), 16),
                    selection,
                );
            }
            // A whitespace-only change is real but peripheral, so its ink is weakened rather
            // than presented as an edit of the same weight as a semantic one.
            let row_ink =
                if line.blank_only { text_color.blend(&background, 0.45) } else { text_color };

            match side {
                RowSide::Both => {
                    // Each column is clipped to its own half, so an over-wide line cannot paint
                    // into the column beside it. The bound is the column, not the text, so every
                    // row that already fitted paints exactly the same pixels.
                    if let (Some(text), Some(gutter)) = (&line.left, display.left_gutter.as_deref())
                    {
                        context.push_clip(
                            rect.x + 2,
                            y - 11,
                            (mid_x - rect.x - 4).max(1) as u32,
                            16,
                        );
                        context.draw_text(
                            Point::new(rect.x + 4, y),
                            gutter,
                            font,
                            text_color.blend(&background, 0.5),
                            HorizontalAlignment::Left,
                        );
                        context.draw_text(
                            Point::new(rect.x + 34, y),
                            text,
                            font,
                            row_ink,
                            HorizontalAlignment::Left,
                        );
                        context.pop_clip();
                    }
                    if let (Some(text), Some(gutter)) =
                        (&line.right, display.right_gutter.as_deref())
                    {
                        context.push_clip(
                            mid_x + 2,
                            y - 11,
                            (rect.x + rect.width as i32 - mid_x - 4).max(1) as u32,
                            16,
                        );
                        context.draw_text(
                            Point::new(mid_x + 4, y),
                            gutter,
                            font,
                            text_color.blend(&background, 0.5),
                            HorizontalAlignment::Left,
                        );
                        context.draw_text(
                            Point::new(mid_x + 34, y),
                            text,
                            font,
                            row_ink,
                            HorizontalAlignment::Left,
                        );
                        context.pop_clip();
                    }
                }
                RowSide::Left | RowSide::Right => {
                    let (text, gutter) = match side {
                        RowSide::Left => (line.left.as_deref(), display.left_gutter.as_deref()),
                        _ => {
                            let text = match line.kind {
                                DiffKind::Added | DiffKind::Changed | DiffKind::Equal => {
                                    line.right.as_deref()
                                }
                                DiffKind::Removed => line.left.as_deref(),
                            };
                            (text, display.left_gutter.as_deref())
                        }
                    };
                    if let (Some(text), Some(gutter)) = (text, gutter) {
                        context.push_clip(
                            rect.x + 2,
                            y - 11,
                            rect.width.saturating_sub(4).max(1),
                            16,
                        );
                        context.draw_text(
                            Point::new(rect.x + 4, y),
                            gutter,
                            font,
                            text_color.blend(&background, 0.5),
                            HorizontalAlignment::Left,
                        );
                        context.draw_text(
                            Point::new(rect.x + 38, y),
                            text,
                            font,
                            row_ink,
                            HorizontalAlignment::Left,
                        );
                        context.pop_clip();
                    }
                }
            }

            // Word-level highlight: the spans were resolved when the rows were built, and their
            // pixel widths are measured once per row and reused by every still frame.
            if let (DiffKind::Changed, Some((left_span, right_span))) =
                (line.kind, line.changed_spans)
            {
                let (Some(left), Some(right)) = (&line.left, &line.right) else {
                    continue;
                };
                let tint = changed.unwrap_or(background).with_alpha(90);
                let draw_left = matches!(side, RowSide::Both | RowSide::Left);
                let draw_right = matches!(side, RowSide::Both | RowSide::Right);
                let widths =
                    self.span_widths.entry((offset, font.size().to_bits())).or_insert_with(|| {
                        let measure = |text: &str, span: (usize, usize)| {
                            let prefix = context.measure_text(&text[..span.0], font).width as i32;
                            let run = context.measure_text(&text[span.0..span.1], font).width;
                            (prefix, run)
                        };
                        (measure(left, left_span), measure(right, right_span))
                    });
                if draw_left {
                    let x_base = if side == RowSide::Both { rect.x + 34 } else { rect.x + 38 };
                    context.fill_rect(
                        Rect::new(x_base + widths.0 .0, y - 1, widths.0 .1.max(1), 14),
                        tint,
                    );
                }
                if draw_right {
                    let x_base = if side == RowSide::Both { mid_x + 34 } else { rect.x + 38 };
                    context.fill_rect(
                        Rect::new(x_base + widths.1 .0, y - 1, widths.1 .1.max(1), 14),
                        tint,
                    );
                }
            }

            context.draw_line(
                Point::new(rect.x + 2, y + 7),
                Point::new(rect.x + rect.width as i32 - 2, y + 7),
                row_rule,
            );
        }
    }
}

/// Removes every whitespace character from a line, for whitespace-insensitive comparison.
fn collapse_whitespace(line: &str) -> String {
    line.chars().filter(|ch| !ch.is_whitespace()).collect()
}

/// Widens `span` to the word boundaries it touches in `text`.
///
/// A word character is a letter, a digit or `_`, which is the same set a reader calls "part of an
/// identifier" in the code this control is mostly used for. When the span is empty (an insertion
/// at a point) the word under that point is selected instead, so a pure insertion still has
/// something to tint.
fn word_spans(text: &str, span: (usize, usize)) -> (usize, usize) {
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
fn differing_spans(left: &str, right: &str) -> ((usize, usize), (usize, usize)) {
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

/// How one file of a multi-file comparison changed.
///
/// This is the model-level status a file list displays; it is deliberately unrelated to
/// [`DiffKind`], which classifies a single *line*. A file can be `Modified` and every changed line
/// in it `Changed`, but the two answers are owned by different parts of the review view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileDiffStatus {
    /// The file exists only on the right-hand side.
    Added,
    /// The file exists only on the left-hand side.
    Removed,
    /// The file exists on both sides and differs.
    #[default]
    Modified,
    /// The file exists on both sides under different paths.
    Renamed,
}

impl FileDiffStatus {
    /// The token spelling published as part of the `files` property.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
            Self::Renamed => "renamed",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "added" | "a" | "+" => Some(Self::Added),
            "removed" | "deleted" | "d" | "-" => Some(Self::Removed),
            "modified" | "m" | "~" => Some(Self::Modified),
            "renamed" | "r" => Some(Self::Renamed),
            _ => None,
        }
    }

    /// The one-glyph marker drawn in a row's status cell.
    pub fn marker(self) -> &'static str {
        match self {
            Self::Added => "+",
            Self::Removed => "-",
            Self::Modified => "~",
            Self::Renamed => "R",
        }
    }
}

/// One file of a multi-file comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiffEntry {
    /// Repository-relative path of the file.
    pub path: String,
    /// How the file changed.
    pub status: FileDiffStatus,
    /// Added line count, as the host's own comparison reported it.
    pub additions: u32,
    /// Removed line count, as the host's own comparison reported it.
    pub deletions: u32,
}

impl FileDiffEntry {
    /// Creates an entry with the counts at zero.
    pub fn new(path: impl Into<String>, status: FileDiffStatus) -> Self {
        Self { path: path.into(), status, additions: 0, deletions: 0 }
    }

    /// Sets the added/removed counts.
    pub fn with_counts(mut self, additions: u32, deletions: u32) -> Self {
        self.additions = additions;
        self.deletions = deletions;
        self
    }

    /// The entry's published property spelling: `path|status|additions|deletions`.
    pub fn to_token(&self) -> String {
        format!("{}|{}|{}|{}", self.path, self.status.as_str(), self.additions, self.deletions)
    }

    /// Parses [`Self::to_token`]; an unknown status is a `TypeMismatch` for the caller.
    pub fn from_token(token: &str) -> Option<Self> {
        let mut parts = token.splitn(4, '|');
        let path = parts.next()?.to_string();
        let status = FileDiffStatus::from_token(parts.next()?)?;
        let additions = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
        let deletions = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
        Some(Self { path, status, additions, deletions })
    }
}

/// Height of one file row and the gap above the first row.
const FILE_ROW_HEIGHT: i32 = 22;
const FILE_TOP: i32 = 6;

/// A file selector that drives a multi-file [`DiffViewer`].
///
/// `DiffViewer::set_texts` compares exactly one pair of snapshots, so a review of several files
/// has always needed a host-owned list beside it. This control is that list: it holds the file set
/// and its own selection, and reports which file the reader picked so the host can feed the
/// corresponding pair to a viewer.
///
/// It does **not** load or compare files — it has no source of file contents — so it never claims
/// a file was read. `file_selected` means "the selection moved to this path" and `file_activated`
/// means "the reader asked to open this path" (a double-click or Enter).
pub struct DiffFileList {
    base: BaseWidget,
    files: Vec<FileDiffEntry>,
    /// The selected rows, as indices into `files`.
    selected: Vec<usize>,
    /// How many rows a press may add to the selection.
    selection_mode: crate::widget::view_widgets::list_view::SelectionMode,
    /// The first row the viewport paints.
    first_visible: usize,
    /// Emitted when the selection moves, with the newly selected path.
    pub file_selected: Signal1<String>,
    /// Emitted when a file is activated by Enter or a double-click, with its path.
    pub file_activated: Signal1<String>,
}

impl DiffFileList {
    /// Creates an empty file list.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DiffFileList, geometry, "DiffFileList"),
            files: Vec::new(),
            selected: Vec::new(),
            selection_mode: crate::widget::view_widgets::list_view::SelectionMode::Single,
            first_visible: 0,
            file_selected: Signal1::new(),
            file_activated: Signal1::new(),
        }
    }

    /// The files in display order.
    pub fn files(&self) -> &[FileDiffEntry] {
        &self.files
    }

    /// Replaces the file set.
    ///
    /// A selection that still names a row is kept (the reader's place in a refreshed list is
    /// worth preserving), and a selection that no longer exists is dropped. No signal fires for a
    /// programmatic replacement: nothing was *selected* by the reader, and a subscriber that
    /// treated this as a pick would open a file the reader never chose.
    pub fn set_files(&mut self, files: Vec<FileDiffEntry>) {
        // Remember the selection by path, not by index: a refreshed list may be reordered, and an
        // index kept across it would point at a different file.
        let selected_paths: Vec<String> = self
            .selected
            .iter()
            .filter_map(|index| self.files.get(*index))
            .map(|file| file.path.clone())
            .collect();
        self.files = files;
        self.selected = selected_paths
            .into_iter()
            .filter_map(|path| self.files.iter().position(|file| file.path == path))
            .collect();
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How many rows a press may select.
    pub fn selection_mode(&self) -> crate::widget::view_widgets::list_view::SelectionMode {
        self.selection_mode
    }

    /// Sets how many rows a press may select.
    pub fn set_selection_mode(
        &mut self,
        mode: crate::widget::view_widgets::list_view::SelectionMode,
    ) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        if !matches!(mode, crate::widget::view_widgets::list_view::SelectionMode::Multi) {
            self.selected.truncate(1);
        }
        self.base.request_redraw();
    }

    /// The selected paths, in display order.
    pub fn selected_paths(&self) -> Vec<&str> {
        self.selected
            .iter()
            .filter_map(|index| self.files.get(*index))
            .map(|file| file.path.as_str())
            .collect()
    }

    /// The first selected path, which is the one a viewer is expected to show.
    pub fn selected_path(&self) -> Option<&str> {
        self.selected_paths().first().copied()
    }

    /// Selects `path`, reporting the move.
    ///
    /// Returns `false` — and emits nothing — when no file has that path, because a subscriber told
    /// about a file that is not in the list would ask the viewer for contents that are not there.
    pub fn select_file(&mut self, path: &str) -> bool {
        let Some(index) = self.files.iter().position(|file| file.path == path) else {
            return false;
        };
        self.select_index(index);
        true
    }

    /// Selects the row at `index`, honouring the selection mode.
    pub fn select_index(&mut self, index: usize) {
        let Some(file) = self.files.get(index) else { return };
        let path = file.path.clone();
        let changed = match self.selection_mode {
            crate::widget::view_widgets::list_view::SelectionMode::None => false,
            crate::widget::view_widgets::list_view::SelectionMode::Multi => {
                if let Some(position) = self.selected.iter().position(|row| *row == index) {
                    self.selected.remove(position);
                } else {
                    self.selected.push(index);
                    self.selected.sort_unstable();
                }
                true
            }
            _ => {
                if self.selected == [index] {
                    false
                } else {
                    self.selected = vec![index];
                    true
                }
            }
        };
        if changed {
            self.reveal(index);
            self.file_selected.emit(path);
            self.base.request_redraw();
        }
    }

    /// Activates the row at `index`, reporting it.
    pub fn activate_index(&mut self, index: usize) {
        if let Some(file) = self.files.get(index) {
            let path = file.path.clone();
            self.file_activated.emit(path);
        }
    }

    /// The height one row occupies in the painted list.
    fn row_height() -> i32 {
        FILE_ROW_HEIGHT
    }

    /// Rows the viewport can paint.
    fn visible_rows(&self) -> usize {
        ((self.geometry().height as i32 - FILE_TOP).max(0) / Self::row_height()).max(1) as usize
    }

    /// Keeps `first_visible` inside the scrollable range.
    fn clamp_scroll(&mut self) {
        let max_first = self.files.len().saturating_sub(self.visible_rows());
        self.first_visible = self.first_visible.min(max_first);
    }

    /// Scrolls the list so `index` is visible.
    fn reveal(&mut self, index: usize) {
        let rows = self.visible_rows();
        if index < self.first_visible {
            self.first_visible = index;
        } else if index >= self.first_visible + rows {
            self.first_visible = index + 1 - rows;
        }
        self.clamp_scroll();
    }

    /// The row index under `pos`, when the point is on a file row.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let row = (pos.y - rect.y - FILE_TOP) / Self::row_height();
        if row < 0 {
            return None;
        }
        let index = self.first_visible + row as usize;
        (index < self.files.len()).then_some(index)
    }

    /// The painted box of row `index`, for hit tests and tests of the layout.
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        if index < self.first_visible || index >= self.files.len() {
            return None;
        }
        let row = (index - self.first_visible) as i32;
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            rect.y + FILE_TOP + row * Self::row_height(),
            rect.width,
            Self::row_height() as u32,
        ))
    }

    /// Scrolls by `rows`, returning whether the position moved.
    pub fn scroll_by(&mut self, rows: i32) -> bool {
        let before = self.first_visible;
        let next = self.first_visible as i32 + rows;
        self.first_visible = next.max(0) as usize;
        self.clamp_scroll();
        let moved = self.first_visible != before;
        if moved {
            self.base.request_redraw();
        }
        moved
    }
}

impl Widget for DiffFileList {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(240, 280)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the two names this control emits; see `DiffViewer`'s own note for why the arms
    /// are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "file_selected" => {
                Some(EventSignalRef::mapped("file_selected", &self.file_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "file_activated" => {
                Some(EventSignalRef::mapped("file_activated", &self.file_activated, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for DiffFileList {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "files" => Ok(CapabilityValue::Tuple(
                self.files.iter().map(|file| CapabilityValue::String(file.to_token())).collect(),
            )),
            "file_count" => Ok(CapabilityValue::UInt(self.files.len() as u64)),
            "selected_path" => Ok(match self.selected_path() {
                Some(path) => CapabilityValue::String(path.to_string()),
                None => CapabilityValue::Null,
            }),
            "selection_mode" => Ok(CapabilityValue::String(
                match self.selection_mode {
                    crate::widget::view_widgets::list_view::SelectionMode::Single => "single",
                    crate::widget::view_widgets::list_view::SelectionMode::Multi => "multi",
                    crate::widget::view_widgets::list_view::SelectionMode::Extended => "extended",
                    crate::widget::view_widgets::list_view::SelectionMode::None => "none",
                }
                .to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "files" => {
                let items = crate::widget::capability::coercion::expect_string_list(value)?;
                let mut files = Vec::with_capacity(items.len());
                for item in items {
                    let entry = FileDiffEntry::from_token(&item)
                        .ok_or(CapabilityAccessError::TypeMismatch)?;
                    files.push(entry);
                }
                self.set_files(files);
                Ok(())
            }
            "file_count" | "selected_path" => Err(CapabilityAccessError::ReadOnlyProperty),
            "selection_mode" => {
                let token = expect_string(value)?;
                let mode = match token.as_str() {
                    "single" => crate::widget::view_widgets::list_view::SelectionMode::Single,
                    "multi" => crate::widget::view_widgets::list_view::SelectionMode::Multi,
                    "extended" => crate::widget::view_widgets::list_view::SelectionMode::Extended,
                    "none" => crate::widget::view_widgets::list_view::SelectionMode::None,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_selection_mode(mode);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "files",
            "file_count",
            "selected_path",
            "selection_mode",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for DiffFileList {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.row_at(*pos) {
                    self.select_index(index);
                }
            }
            Event::MouseDoubleClick { pos, button: 1, .. } => {
                if let Some(index) = self.row_at(*pos) {
                    self.activate_index(index);
                }
            }
            Event::KeyPress { key, .. } => match *key {
                38 => {
                    let next = self.selected.first().map_or(0, |index| index.saturating_sub(1));
                    if !self.files.is_empty() {
                        self.select_index(next);
                    }
                }
                40 => {
                    if !self.files.is_empty() {
                        let next = self
                            .selected
                            .first()
                            .map_or(0, |index| (index + 1).min(self.files.len() - 1));
                        self.select_index(next);
                    }
                }
                13 => {
                    if let Some(index) = self.selected.first().copied() {
                        self.activate_index(index);
                    }
                }
                _ => { /* Other keys are not relevant */ }
            },
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                self.scroll_by(-scroll.lines_y);
            }
            _ => {}
        }
    }
}

impl Draw for DiffFileList {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("diff_viewer");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(background));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let selection = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .map(|token| token.blend(&background, 0.75))
            .unwrap_or_else(|| background.blend(&text_color, 0.25));
        let font = effective_font(style);

        // A translucent accent wash so a selected row keeps its text readable.
        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        let rows = self.visible_rows();
        for offset in 0..rows {
            let index = self.first_visible + offset;
            let Some(file) = self.files.get(index) else {
                break;
            };
            let Some(row) = self.row_rect(index) else {
                break;
            };
            if self.selected.contains(&index) {
                context.fill_rect(row, selection);
            }
            let status_color = match file.status {
                FileDiffStatus::Added => {
                    crate::style::semantic_color(crate::style::SemanticColor::Success)
                }
                FileDiffStatus::Removed => {
                    crate::style::semantic_color(crate::style::SemanticColor::Error)
                }
                FileDiffStatus::Modified => {
                    crate::style::semantic_color(crate::style::SemanticColor::Warning)
                }
                FileDiffStatus::Renamed => {
                    crate::style::semantic_color(crate::style::SemanticColor::Info)
                }
            }
            .unwrap_or(text_color);
            context.draw_text(
                Point::new(row.x + 6, row.y + 5),
                file.status.marker(),
                font,
                status_color,
                HorizontalAlignment::Left,
            );
            let counts = format!("+{} -{}", file.additions, file.deletions);
            let counts_width = context.measure_text(&counts, font).width as i32;
            let path_band = (row.width as i32 - 12 - counts_width - 8).max(24);
            let path =
                crate::widget::text_utils::elide_end(&file.path, (path_band / 7).max(1) as usize);
            context.draw_text(
                Point::new(row.x + 22, row.y + 5),
                &path,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
            context.draw_text(
                Point::new(row.x + row.width as i32 - counts_width - 6, row.y + 5),
                &counts,
                font,
                text_color.blend(&background, 0.45),
                HorizontalAlignment::Left,
            );
            context.draw_line(
                Point::new(row.x, row.y + row.height as i32 - 1),
                Point::new(row.x + row.width as i32, row.y + row.height as i32 - 1),
                background.blend(&text_color, 0.10),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// The exact-comparison entry point, for tests that pin the algorithm without a whitespace
    /// policy; the widget itself goes through `build_diff_lines_with`.
    fn build_diff_lines(a: &[&str], b: &[&str]) -> Vec<DiffLine> {
        build_diff_lines_with(a, b, a, b, false)
    }

    #[test]
    fn recompute_detects_changes() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nb\nc", "a\nB\nc\nd");

        assert_eq!(viewer.lines().len(), 4);
        assert_eq!(viewer.change_count(), 2);
        assert_eq!(viewer.lines()[1].kind, DiffKind::Changed);
        assert_eq!(viewer.lines()[3].kind, DiffKind::Added);
    }

    #[test]
    fn compared_signal_emits_change_count() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        let counts = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = counts.clone();
        viewer.compared.connect(move |count| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*count);
            }
        });

        viewer.set_texts("x", "y");

        let got = counts.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![1]);
    }

    #[test]
    fn arrow_keys_move_selected_line() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nb", "a\nb\nc");
        assert_eq!(viewer.selected_index(), Some(0));

        viewer.handle_event(&Event::key_press(40, 0));
        assert_eq!(viewer.selected_index(), Some(1));

        viewer.handle_event(&Event::key_press(40, 0));
        assert_eq!(viewer.selected_index(), Some(2));

        viewer.handle_event(&Event::key_press(38, 0));
        assert_eq!(viewer.selected_index(), Some(1));
    }

    #[test]
    fn leading_insertion_leaves_the_rest_equal() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("A\nB\nC", "X\nA\nB\nC");

        let lines = viewer.lines();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].kind, DiffKind::Added);
        assert_eq!(lines[0].left, None);
        assert_eq!(lines[0].right.as_deref(), Some("X"));
        assert!(lines[1..].iter().all(|line| line.kind == DiffKind::Equal));
        assert_eq!(lines[1].left.as_deref(), Some("A"));
        assert_eq!(viewer.change_count(), 1);
    }

    #[test]
    fn identical_inputs_are_all_equal() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("alpha\nbeta\ngamma", "alpha\nbeta\ngamma");

        assert_eq!(viewer.lines().len(), 3);
        assert!(viewer.lines().iter().all(|line| line.kind == DiffKind::Equal));
        assert_eq!(viewer.change_count(), 0);
    }

    #[test]
    fn change_count_matches_non_equal_rows() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("one\ntwo\nthree\nfour", "one\n2\nthree\nfour\nfive");

        let non_equal = viewer.lines().iter().filter(|line| line.kind != DiffKind::Equal).count();
        assert_eq!(viewer.change_count(), non_equal);
        assert!(non_equal > 0);
    }

    /// The wheel scrolls the visible diff window and clamps at both ends.
    ///
    /// # The defect this pins
    ///
    /// The paint path stopped at a fixed twelve rows, so a longer comparison could not be
    /// reached. This drives the window the paint path now starts from.
    #[test]
    fn diff_viewer_wheel_scrolls_and_clamps() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let joined = text.join("\n");
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 100));
        viewer.set_texts(joined.clone(), joined);
        assert_eq!(viewer.lines().len(), 20);
        assert_eq!(viewer.first_visible_line, 0);

        // One notch toward the end reveals three rows.
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(viewer.first_visible_line, 3, "a downward notch moves the viewport down");

        // The other direction returns to the top and clamps there.
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(viewer.first_visible_line, 0);
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(viewer.first_visible_line, 0, "the viewport clamps at the first row");

        // Scrolling on stops at the last full window.
        let max_first = viewer.lines().len() - viewer.visible_line_count();
        for _ in 0..20 {
            viewer.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        }
        assert_eq!(viewer.first_visible_line, max_first, "the viewport clamps at the last window");
    }

    /// Trimming the common head and tail keeps a middle edit to its own hunk.
    #[test]
    fn a_middle_edit_changes_only_its_own_hunk() {
        let before: Vec<String> = (0..600).map(|index| alloc::format!("line {index}")).collect();
        let mut after = before.clone();
        after[300] = "line three hundred".to_string();
        let before_refs: Vec<&str> = before.iter().map(String::as_str).collect();
        let after_refs: Vec<&str> = after.iter().map(String::as_str).collect();

        let lines = build_diff_lines(&before_refs, &after_refs);
        assert_eq!(lines.len(), before.len());
        assert_eq!(lines[0].kind, DiffKind::Equal);
        assert_eq!(lines[599].kind, DiffKind::Equal);
        assert_eq!(lines[300].kind, DiffKind::Changed);
        assert_eq!(
            lines.iter().filter(|line| line.kind != DiffKind::Equal).count(),
            1,
            "the unchanged head and tail must not be reported as changes"
        );
    }

    /// Unified mode paints a changed row as a removal and an addition, so the old text survives.
    ///
    /// The viewer used to paint only the new text with a `~` prefix, which is a diff that cannot
    /// answer "what changed".
    #[test]
    fn unified_mode_shows_the_old_text_of_a_changed_row() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nold\nb", "a\nnew\nb");
        viewer.set_view_mode(DiffViewMode::Unified);
        assert_eq!(viewer.display_row_count(), 4, "the changed row takes two painted rows");

        assert!(matches!(
            viewer.display_rows[1].source,
            RowSource::Line { index: 1, side: RowSide::Left }
        ));
        assert!(matches!(
            viewer.display_rows[2].source,
            RowSource::Line { index: 1, side: RowSide::Right }
        ));
        assert_eq!(viewer.display_rows[1].left_number, Some(2));
        assert_eq!(viewer.display_rows[2].right_number, Some(2));
        assert_eq!(viewer.lines()[1].left.as_deref(), Some("old"));
    }

    /// The pair expansion is optional, and disabling it restores the single `~` row.
    #[test]
    fn unified_changed_pair_can_be_disabled() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nold\nb", "a\nnew\nb");
        viewer.set_view_mode(DiffViewMode::Unified);
        viewer.set_unified_changed_pair(false);
        assert_eq!(viewer.display_row_count(), 3);
        assert!(matches!(
            viewer.display_rows[1].source,
            RowSource::Line { index: 1, side: RowSide::Right }
        ));
    }

    /// Selection, view-mode and fold changes are published.
    #[test]
    fn view_state_signals_are_emitted() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        let selections = Arc::new(Mutex::new(Vec::<Option<usize>>::new()));
        let sink = selections.clone();
        viewer.selection_changed.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });
        let modes = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = modes.clone();
        viewer.view_mode_changed.connect(move |mode| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(mode.as_ref().clone());
            }
        });
        let folds = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = folds.clone();
        viewer.fold_changed.connect(move |fold| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*fold);
            }
        });

        viewer.set_texts("a\nb", "a\nb\nc");
        assert!(viewer.select_index(1));
        assert!(viewer.select_index(1), "re-selecting the same row is not a change");
        viewer.set_view_mode(DiffViewMode::Unified);
        viewer.set_fold_unchanged(true);

        let selections = selections.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(selections, vec![Some(0), Some(1)], "the reset and the click are both reported");
        let modes = modes.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(modes, vec!["unified".to_string()]);
        let folds = folds.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(folds, vec![true]);
    }

    /// A fold marker is clickable and expands the run it stands for.
    #[test]
    fn clicking_the_fold_marker_expands_the_hunk() {
        let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
        let mut changed = text.clone();
        changed[25] = "line twenty-five".to_string();
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts(text.join("\n"), changed.join("\n"));
        viewer.set_fold_unchanged(true);

        let expanded = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = expanded.clone();
        viewer.hunk_expanded.connect(move |start| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*start);
            }
        });

        assert_eq!(viewer.display_row_count(), 6, "one marker + the change + the tail");
        assert!(matches!(viewer.display_rows[0].source, RowSource::Fold { run: (0, 25) }));

        // The click row is derived from the same constants the paint uses.
        viewer.handle_event(&Event::mouse_press(10, ROW_TOP + 2, 1));
        assert_eq!(viewer.display_row_count(), 30, "expanding restores every row");
        let expanded = expanded.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(expanded, vec![0]);
    }

    /// Whitespace policy changes what counts as a change without rewriting the visible text.
    #[test]
    fn whitespace_modes_change_what_counts_as_a_change() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a", "a ");
        assert_eq!(viewer.change_count(), 1);
        assert!(viewer.lines()[0].blank_only, "a trailing space is a blank-only change");

        viewer.set_whitespace_mode(WhitespaceMode::IgnoreEol);
        assert_eq!(viewer.change_count(), 0, "trailing whitespace is ignored");
        assert_eq!(viewer.lines()[0].left.as_deref(), Some("a"), "the display text is raw");
        assert_eq!(viewer.lines()[0].right.as_deref(), Some("a "));

        viewer.set_whitespace_mode(WhitespaceMode::IgnoreAll);
        viewer.set_texts("a b", "ab");
        assert_eq!(viewer.change_count(), 0, "all whitespace is ignored");

        viewer.set_whitespace_mode(WhitespaceMode::MarkBlankChanges);
        viewer.set_texts("a", "a ");
        assert_eq!(viewer.change_count(), 1, "blank-only rows are still reported");
        assert!(viewer.lines()[0].blank_only);
    }

    /// The differing spans are resolved when the row is built, not per frame.
    #[test]
    fn changed_spans_are_precomputed() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("x = 1", "x = 2");
        let line = &viewer.lines()[0];
        assert_eq!(line.kind, DiffKind::Changed);
        assert_eq!(line.changed_spans, Some(differing_spans("x = 1", "x = 2")));
        assert!(viewer
            .lines()
            .iter()
            .all(|line| line.kind == DiffKind::Changed || line.changed_spans.is_none()));
    }

    /// Character granularity is the default; word granularity widens the tint to the words the
    /// changed run touches, without changing the row set.
    #[test]
    fn word_granularity_widens_the_inline_span() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("alpha beta", "alphabet beta");
        assert!(!viewer.word_granularity());
        let char_span = viewer.lines()[0].changed_spans.expect("changed row");
        // The changed run under character granularity is the appended `bet`.
        assert_eq!(char_span.0, (5, 5));
        assert_eq!(char_span.1, (5, 8));

        viewer.set_word_granularity(true);
        assert!(viewer.word_granularity());
        let word_span = viewer.lines()[0].changed_spans.expect("changed row");
        let ((left_start, left_end), (right_start, right_end)) = word_span;
        assert_eq!(&"alpha beta"[left_start..left_end], "alpha");
        assert_eq!(&"alphabet beta"[right_start..right_end], "alphabet");

        // Switching back restores the character span rather than leaving the widened one.
        viewer.set_word_granularity(false);
        assert_eq!(viewer.lines()[0].changed_spans, Some(char_span));
    }

    /// A comparison rebuilt while word granularity is on keeps the widened spans.
    #[test]
    fn word_granularity_survives_a_recomparison() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_word_granularity(true);
        viewer.set_texts("total = 41", "total = 42");
        let span = viewer.lines()[0].changed_spans.expect("changed row");
        assert_eq!(&"total = 41"[span.0 .0..span.0 .1], "41");
        assert_eq!(&"total = 42"[span.1 .0..span.1 .1], "42");
    }

    fn file_list() -> DiffFileList {
        let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
        list.set_files(vec![
            FileDiffEntry::new("src/a.rs", FileDiffStatus::Modified).with_counts(3, 1),
            FileDiffEntry::new("src/b.rs", FileDiffStatus::Added).with_counts(10, 0),
            FileDiffEntry::new("src/c.rs", FileDiffStatus::Removed).with_counts(0, 7),
        ]);
        list
    }

    #[test]
    fn file_list_reports_selection_and_activation() {
        let mut list = file_list();
        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        list.file_selected.connect(move |path| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(path.as_str().to_string());
            }
        });
        let sink = activated.clone();
        list.file_activated.connect(move |path| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(path.as_str().to_string());
            }
        });

        assert!(list.select_file("src/b.rs"));
        assert!(!list.select_file("src/missing.rs"), "an unknown path is refused");
        let row = list.row_rect(1).expect("row 1 is visible");
        list.handle_event(&Event::mouse_double_click(row.x + 4, row.y + 4, 1));

        assert_eq!(list.selected_path(), Some("src/b.rs"));
        assert_eq!(
            selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec!["src/b.rs".to_string()]
        );
        assert_eq!(
            activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec!["src/b.rs".to_string()]
        );
    }

    #[test]
    fn file_list_files_property_round_trips() {
        let mut list = file_list();
        let read = list.get("files").expect("files readable");
        let written = list.set("files", read.clone());
        assert!(written.is_ok());
        assert_eq!(list.get("files").expect("files readable"), read);
        assert_eq!(list.get("file_count"), Ok(CapabilityValue::UInt(3)));
    }

    #[test]
    fn file_list_multi_selection_keeps_every_clicked_row() {
        let mut list = file_list();
        list.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Multi);
        list.select_index(0);
        list.select_index(2);
        assert_eq!(list.selected_paths(), vec!["src/a.rs", "src/c.rs"]);
        list.select_index(0);
        assert_eq!(list.selected_paths(), vec!["src/c.rs"]);
    }

    /// The Myers trace is bounded by bytes, and an over-deep comparison is reported as such.
    #[test]
    fn the_myers_trace_is_bounded() {
        let left: Vec<String> = (0..2000).map(|n| format!("left{n}")).collect();
        let right: Vec<String> = (0..2000).map(|n| format!("right{n}")).collect();
        let left_refs: Vec<&str> = left.iter().map(String::as_str).collect();
        let right_refs: Vec<&str> = right.iter().map(String::as_str).collect();
        assert!(
            myers_diff(&left_refs, &right_refs).is_none(),
            "a full-distance comparison must be refused rather than allocating the trace"
        );
        // The caller still gets a usable row list from the positional fallback.
        let lines = build_diff_lines(&left_refs, &right_refs);
        assert_eq!(lines.len(), 2000);
        assert!(lines.iter().all(|line| line.kind != DiffKind::Equal));
    }

    /// The fold threshold and the context kept around a marker are configurable.
    #[test]
    fn fold_threshold_and_context_lines_are_configurable() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let mut changed = text.clone();
        changed[15] = "line fifteen".to_string();
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts(text.join("\n"), changed.join("\n"));
        assert_eq!(viewer.fold_threshold(), 8);
        assert_eq!(viewer.fold_context_lines(), 0);

        viewer.set_fold_unchanged(true);
        viewer.set_fold_threshold(30);
        assert_eq!(viewer.display_row_count(), 20, "a threshold above the run keeps every row");

        viewer.set_fold_threshold(4);
        viewer.set_fold_context_lines(2);
        viewer.display_dirty = true;
        let rows = viewer.display_row_count();
        assert!(rows < 20, "the run is folded with context around it");
        assert!(matches!(viewer.display_rows[2].source, RowSource::Fold { .. }));
        assert!(matches!(viewer.display_rows[0].source, RowSource::Line { index: 0, .. }));
        assert!(matches!(viewer.display_rows[1].source, RowSource::Line { index: 1, .. }));
    }
}

#[cfg(test)]
mod issue_2_8_tests {
    use super::*;

    /// SP-08: the two normalisation switches remove noise the row set used to keep.
    #[test]
    fn ignore_case_and_blank_lines_remove_comparison_noise() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts("Alpha\n \nbeta", "alpha\n\nbeta");
        assert!(viewer.change_count() > 0);
        viewer.set_ignore_case(true);
        viewer.set_ignore_blank_lines(true);
        assert_eq!(viewer.change_count(), 0);
        assert!(viewer.ignore_case());
        assert!(viewer.ignore_blank_lines());
    }

    /// SP-P12: the gutters are precomputed with the rows rather than formatted per frame.
    #[test]
    fn display_rows_carry_their_gutter_captions() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts("one\ntwo", "one\nTWO");
        let count = viewer.display_row_count();
        assert!(count >= 2);
        let row = &viewer.display_rows[1];
        assert!(row.left_gutter.is_some() || row.right_gutter.is_some());
    }
}
