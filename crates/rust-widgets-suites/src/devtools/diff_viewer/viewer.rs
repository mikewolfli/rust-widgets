// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The `DiffViewer` control: the compared rows, view state, paint and interaction.

/// The top of the first diff row below the column headings.
pub(super) const ROW_TOP: i32 = 34;
/// The pitch between diff rows.
const ROW_HEIGHT: i32 = 16;
/// How much room below a row's top its height still needs.
const ROW_BOTTOM_INSET: i32 = 14;

use core::cell::RefCell;
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect};
use rust_widgets::event::wheel::WheelScroll;
use rust_widgets::event::{key_codes, Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::special_widgets::editor::syntax::{
    BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter, TokenKind, TokenSpan,
};
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};
use std::borrow::Cow;
use std::collections::HashMap;

use super::model::*;

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
    pub(super) first_visible_line: usize,
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
    pub(super) display_rows: Vec<DisplayRow>,
    /// Measured widths of a changed row's inline run, keyed by display row and font size.
    ///
    /// The spans themselves are resolved when the rows are built; only their pixel widths need the
    /// renderer, so they are measured once per row and reused by every still frame.
    span_widths: RefCell<SpanWidths>,
    /// The font `span_widths` was measured with.
    ///
    /// The map is keyed by row only; a change of family, weight, style or spacing at the same
    /// size clears it here, which is what keeps a theme change from reusing stale advances without
    /// putting a heap `String` in every lookup key.
    span_widths_font: RefCell<Option<rust_widgets::core::Font>>,
    /// Whether `display_rows` needs rebuilding before the next use.
    pub(super) display_dirty: bool,
    /// Whether the compared lines are syntax-highlighted.
    ///
    /// Off by default: the viewer's own meaning is the diff, and a colour added for a token would
    /// compete with the added/removed/changed row tint that carries it. A host that shows source
    /// turns it on and names the language.
    syntax_highlight: bool,
    /// The language the highlighter tokenises when [`Self::syntax_highlight`] is on.
    language: LanguageId,
    /// Highlight spans for both sides of every line, built once per text change.
    pub(super) syntax_spans: Option<SyntaxSpanTable>,
    /// Whether [`Self::syntax_spans`] must be rebuilt before the next use.
    syntax_dirty: bool,
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
pub(super) enum RowSide {
    /// Both columns (side-by-side).
    Both,
    /// The left column only (a unified `-` row).
    Left,
    /// The right column only (a unified `+`, `~` or context row).
    Right,
}

/// What one painted row shows.
#[derive(Debug, Clone, Copy)]
pub(super) enum RowSource {
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
pub(super) struct DisplayRow {
    pub(super) source: RowSource,
    /// The left gutter's source number, kept for the structure tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) left_number: Option<usize>,
    /// The right gutter's source number, kept for the structure tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) right_number: Option<usize>,
    /// The gutter caption painted in the left column (or the single column in unified mode).
    pub(super) left_gutter: Option<String>,
    /// The gutter caption painted in the right column.
    pub(super) right_gutter: Option<String>,
    /// The fold marker's caption, when this row stands in for a folded run.
    fold_label: Option<String>,
}

/// The measured inline-run widths for each display row and font size.
type SpanWidths = HashMap<(usize, u32), ((i32, u32), (i32, u32))>;

/// Highlight spans for one line, per side: `(old side runs, new side runs)`.
type LineSyntaxSpans = (Vec<TokenSpan>, Vec<TokenSpan>);
/// The whole document's highlight result: the language it was built for and one entry per line.
type SyntaxSpanTable = (LanguageId, Vec<LineSyntaxSpans>);

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
            span_widths: RefCell::new(HashMap::new()),
            span_widths_font: RefCell::new(None),
            display_dirty: true,
            syntax_highlight: false,
            language: LanguageId::PlainText,
            syntax_spans: None,
            syntax_dirty: true,
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

    /// Whether the compared lines are syntax-highlighted.
    pub fn syntax_highlight(&self) -> bool {
        self.syntax_highlight
    }

    /// Enables or disables syntax highlighting over the diff rows.
    ///
    /// Off by default, which is the row-level presentation the viewer has always painted.
    pub fn set_syntax_highlight(&mut self, enabled: bool) {
        if self.syntax_highlight == enabled {
            return;
        }
        self.syntax_highlight = enabled;
        self.syntax_dirty = true;
        self.base.request_redraw();
    }

    /// The language tokenised when syntax highlighting is on.
    pub fn language(&self) -> LanguageId {
        self.language
    }

    /// Sets the language tokenised by syntax highlighting.
    ///
    /// [`LanguageId::PlainText`] produces no token spans, so enabling highlighting without a
    /// language leaves the paint path exactly as it was.
    pub fn set_language(&mut self, language: LanguageId) {
        if self.language == language {
            return;
        }
        self.language = language;
        self.syntax_dirty = true;
        self.base.request_redraw();
    }

    /// Rebuilds the token spans when the text or language changed.
    ///
    /// Both sides are lexed in their own line order, so a multi-line construct that the diff
    /// splits across an added or removed row keeps its state on the side the row belongs to.
    pub(super) fn ensure_syntax_spans(&mut self) {
        if !self.syntax_dirty {
            return;
        }
        self.syntax_dirty = false;
        if !self.syntax_highlight || self.language == LanguageId::PlainText {
            // "No tokens" is represented by an empty table, not by leaving the last language's
            // spans in place: the paint path reads the table whenever highlighting is on, so a
            // stale table painted Rust colours for a plain-text view (and for new text, since the
            // spans are only clamped to its length).
            self.syntax_spans = None;
            return;
        }
        let highlighter = BuiltinHighlighter::new(self.language);
        let mut left_state = LineState::initial();
        let mut right_state = LineState::initial();
        let mut rows = Vec::with_capacity(self.lines.len());
        for line in &self.lines {
            let left = match &line.left {
                Some(text) => {
                    let (spans, state) = highlighter.highlight_line(text, left_state);
                    left_state = state;
                    spans
                }
                None => Vec::new(),
            };
            let right = match &line.right {
                Some(text) => {
                    let (spans, state) = highlighter.highlight_line(text, right_state);
                    right_state = state;
                    spans
                }
                None => Vec::new(),
            };
            rows.push((left, right));
        }
        self.syntax_spans = Some((self.language, rows));
    }

    /// The token colour for `kind`, resolved against the row's ink and surface.
    fn token_color(kind: TokenKind, ink: Color, background: Color) -> Color {
        use rust_widgets::style::{semantic_color, SemanticColor};
        match kind {
            TokenKind::Keyword | TokenKind::Function => {
                semantic_color(SemanticColor::Info).unwrap_or(Color::BLUE)
            }
            TokenKind::Type => semantic_color(SemanticColor::Info).unwrap_or(Color::PRIMARY),
            TokenKind::String | TokenKind::Character => {
                semantic_color(SemanticColor::Success).unwrap_or(Color::GREEN)
            }
            TokenKind::Number => semantic_color(SemanticColor::Warning).unwrap_or(Color::ORANGE),
            TokenKind::Preprocessor => semantic_color(SemanticColor::Error).unwrap_or(Color::RED),
            TokenKind::Comment | TokenKind::BlockComment => ink.blend(&background, 0.45),
            TokenKind::Plain | TokenKind::Operator | TokenKind::Identifier => ink,
        }
    }

    /// Draws `text` at `origin`, colouring its token spans when syntax highlighting is on.
    ///
    /// Each run is measured as it is placed, so a token boundary is the only thing that moves a
    /// glyph; a line without spans (or with highlighting off) takes the single-call path the
    /// viewer has always used.
    fn draw_highlighted(
        &self,
        context: &mut RenderContext,
        origin: Point,
        text: &str,
        spans: &[TokenSpan],
        font: &rust_widgets::core::Font,
        ink: Color,
        background: Color,
    ) {
        if !self.syntax_highlight || spans.is_empty() {
            context.draw_text(origin, text, font, ink, HorizontalAlignment::Left);
            return;
        }
        let mut cursor = 0usize;
        let mut x = origin.x;
        for span in spans {
            let start = span.start.min(text.len());
            let end = span.end.min(text.len()).max(start);
            if start > cursor {
                if let Some(gap) = text.get(cursor..start) {
                    context.draw_text(
                        Point::new(x, origin.y),
                        gap,
                        font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                    x += context.measure_text(gap, font).width as i32;
                }
            }
            if let Some(run) = text.get(start..end) {
                context.draw_text(
                    Point::new(x, origin.y),
                    run,
                    font,
                    Self::token_color(span.kind, ink, background),
                    HorizontalAlignment::Left,
                );
                x += context.measure_text(run, font).width as i32;
            }
            cursor = cursor.max(end);
        }
        if let Some(tail) = text.get(cursor..) {
            context.draw_text(Point::new(x, origin.y), tail, font, ink, HorizontalAlignment::Left);
        }
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
        self.ensure_display_rows();
        // A folded line has no row of its own. Selecting it would leave the ring on nothing and
        // make the next arrow key step from a line the viewport does not paint, so a folded index
        // selects the nearest line that is painted. When the fold covers the whole document there
        // is no painted neighbour at all; clearing the selection is the honest answer, because
        // keeping the hidden index named a row that does not exist.
        self.set_selection(self.nearest_painted_line(index));
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
        // `first_visible_line` indexes the display rows, and unified mode inserts a row for every
        // changed line, so the same index names a different line in the other mode. Every sibling
        // setter that rebuilds the rows resets it; this one kept it, so toggling the presentation
        // moved a scrolled reader to an earlier line.
        self.first_visible_line = 0;
        self.display_dirty = true;
        self.ensure_display_rows();
        self.reconcile_selection_with_display();
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
    ///
    /// The measured advances are keyed by display row and font size, so they describe the spans
    /// that were current when they were measured. Recomputing the spans invalidates them; keeping
    /// them made a granularity switch tint the previous run's pixels until the rows were rebuilt.
    fn refresh_changed_spans(&mut self) {
        self.span_widths.borrow_mut().clear();
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
        self.ensure_display_rows();
        self.reconcile_selection_with_display();
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
        self.ensure_display_rows();
        self.reconcile_selection_with_display();
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
        self.ensure_display_rows();
        self.reconcile_selection_with_display();
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

    /// The number of cached measured span widths.
    ///
    /// Exposed to the tests so the invalidation of the measurement cache can be pinned without a
    /// pixel comparison.
    #[cfg(test)]
    pub(super) fn measured_span_rows(&self) -> usize {
        self.span_widths.borrow().len()
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
            // A fold marker paints a caption and a rule, not the lines it stands for: they are
            // exactly the lines this method documents as absent.
            RowSource::Fold { .. } => false,
        })
    }

    /// The painted line nearest to `index` (itself when it is painted).
    ///
    /// Used to keep the selection on a row the viewport paints when the line it names is hidden
    /// inside a fold.
    fn nearest_painted_line(&self, index: usize) -> Option<usize> {
        if self.display_row_of_line(index).is_some() {
            return Some(index);
        }
        let row = self.display_rows.iter().position(|row| {
            matches!(&row.source, RowSource::Fold { run, .. } if index >= run.0 && index < run.1)
        })?;
        let line_of = |row: &DisplayRow| match &row.source {
            RowSource::Line { index, .. } => Some(*index),
            RowSource::Fold { .. } => None,
        };
        // "Nearest" is a distance: the old code always took the painted row *below* the marker,
        // so a selection two lines into a collapsed hunk jumped to its far side instead of to the
        // adjoining context row.
        let above =
            row.checked_sub(1).and_then(|above| self.display_rows.get(above)).and_then(line_of);
        let below = self.display_rows.get(row + 1).and_then(line_of);
        match (above, below) {
            (Some(above), Some(below)) => {
                if index.saturating_sub(above) <= below.saturating_sub(index) {
                    Some(above)
                } else {
                    Some(below)
                }
            }
            (Some(above), None) => Some(above),
            (None, Some(below)) => Some(below),
            (None, None) => None,
        }
    }

    /// Keeps the selection on a line the rebuilt rows paint.
    fn reconcile_selection_with_display(&mut self) {
        let Some(selected) = self.selected_index else {
            return;
        };
        match self.nearest_painted_line(selected) {
            Some(nearest) if nearest != selected => self.set_selection(Some(nearest)),
            Some(_) => {}
            // The whole document folded into one marker: nothing is painted, so no line can be
            // selected.
            None => self.set_selection(None),
        }
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
    /// Zero when the pane is too short for even one row: the paint then shows nothing, and the
    /// hit test must agree rather than accepting a row the painter skipped.
    pub(super) fn visible_line_count(&self) -> usize {
        let usable = self.geometry().height as i32 - ROW_TOP - ROW_BOTTOM_INSET;
        if usable < 0 {
            return 0;
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
        self.span_widths.borrow_mut().clear();
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
                    // Context rows are ordinary rows of the current mode: unified mode paints one
                    // column, so a `Both` row would draw the same line twice in two places.
                    let side = if self.view_mode == DiffViewMode::Unified {
                        RowSide::Right
                    } else {
                        RowSide::Both
                    };
                    for offset in 0..context {
                        self.push_line_row(
                            start + offset,
                            side,
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
                            side,
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
        // The line list is replaced below, so the per-line token spans describe rows that no
        // longer exist.
        self.syntax_dirty = true;
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

        // Nothing is selectable when the pane is too short to paint a row: naming line 0 would
        // put the selection on a row the paint path skips.
        let next_selection =
            if self.lines.is_empty() || self.visible_line_count() == 0 { None } else { Some(0) };
        self.set_selection(next_selection);
        self.display_dirty = true;
        // The selection resets to the first row, so the viewport follows it. A comparison rebuilt
        // under a new whitespace or case rule could otherwise leave a stale scroll offset that
        // pointed past the new row count, and `scroll_lines` would then jump to the end.
        self.first_visible_line = 0;
        self.ensure_display_rows();
        // A fresh comparison whose first line sits inside a fold selects a row the paint path
        // skips: the fold setters already reconcile, and `recompute` must too, or
        // `selected_index` names line 0 while nothing is painted.
        self.reconcile_selection_with_display();
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
pub(super) enum EditOp {
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

pub(super) fn myers_diff(a: &[&str], b: &[&str]) -> Option<Vec<EditOp>> {
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
pub(super) fn build_diff_lines_with(
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
    fn capability_name(&self) -> Option<&'static str> {
        Some("diff_viewer")
    }
    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(500, 300)
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
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
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
            "ignore_case" => Ok(CapabilityValue::Bool(self.ignore_case())),
            "ignore_blank_lines" => Ok(CapabilityValue::Bool(self.ignore_blank_lines())),
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
            "ignore_case" => {
                self.set_ignore_case(expect_bool(value)?);
                Ok(())
            }
            "ignore_blank_lines" => {
                self.set_ignore_blank_lines(expect_bool(value)?);
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
            "ignore_case",
            "ignore_blank_lines",
            "whitespace_mode",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `diff_viewer` publishes.
    ///
    /// DEV-3: this control publishes **no** commands (`DIFF_VIEWER_PROPERTIES`' neighbour in
    /// `devtools::capability` is an empty list, with the 20261026 audit note explaining why
    /// `set_texts`/`select_index` stay Rust-side). Both names therefore answer
    /// [`CapabilityAccessError::UnknownCommand`] — the same honest answer the factory layer gives
    /// for a name that is not in the published list. The earlier doc claimed `select_index` had a
    /// "valid name, missing payload" route, which no published manifest could ever reach.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        let _ = name;
        Err(CapabilityAccessError::UnknownCommand)
    }
}

impl EventHandler for DiffViewer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::KeyPress { key, modifiers } = event {
            if self.base.focus_reason().is_none() {
                return;
            }
            match *key {
                key_codes::UP => {
                    if let Some(index) = self.selected_index {
                        if index > 0 && self.select_index(index - 1) {
                            // A row selected by the keyboard must be scrolled into view; without
                            // this the caret moved past the painted window and the reader could no
                            // longer see what was selected.
                            self.reveal_line(index - 1);
                        }
                    }
                }
                key_codes::DOWN => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.lines.len() && self.select_index(index + 1) {
                            self.reveal_line(index + 1);
                        }
                    }
                }
                key_codes::PAGE_UP => {
                    self.previous_change();
                }
                key_codes::PAGE_DOWN => {
                    self.next_change();
                }
                key_codes::ENTER => {
                    self.next_change();
                }
                _ if rust_widgets::shortcut::event_carries_primary(*modifiers)
                    && rust_widgets::shortcut::Key::from_key_code(*key)
                        == Some(rust_widgets::shortcut::Key::C) =>
                {
                    if let Some(patch) = self.selected_patch() {
                        let _ = rust_widgets::clipboard::ClipboardManager::set_text(patch);
                    }
                }
                _ => { /* Other keys are not relevant */ }
            }
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            // A click selects the row under it, or expands the fold marker under it. Rows, not
            // lines: a unified changed pair and a folded run both break the one-to-one mapping.
            let rect = self.geometry();
            if rect.contains_point(*pos) {
                // A row's fill starts ten pixels above its text origin, so that band is the hit
                // area. Clamping the band's top (and the viewport's painted row count) keeps a
                // click in the column headings or below the last row from selecting a row that is
                // not on screen.
                let top = pos.y - rect.y - (ROW_TOP - 10);
                if top >= 0 {
                    let row = (top / ROW_HEIGHT) as usize;
                    if row < self.visible_line_count() {
                        self.ensure_display_rows();
                        let display_row = self.visible_start() + row;
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
        // Token spans describe the rows that are about to be painted, so they are rebuilt after
        // the rows exist and before the style borrow is taken (the rebuild needs `&mut self`).
        self.ensure_syntax_spans();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes the appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = rust_widgets::style::resolved_theme_style_shared("diff_viewer");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| rust_widgets::core::Color::border_of(background));
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
        let row_rule = rust_widgets::style::palette_shared()
            .map(|colors| colors.outline_variant)
            .unwrap_or(divider);
        let selection =
            rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Info)
                .unwrap_or_else(|| background.blend(&text_color, 0.5));
        // Added / Removed / Changed are *states*, so they read the theme's semantic
        // tokens and are tinted over the resolved surface to stay legible in both
        // appearances.
        let added =
            rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Success)
                .map(|token| token.blend(&background, 0.85));
        let removed =
            rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Error)
                .map(|token| token.blend(&background, 0.85));
        let changed =
            rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Warning)
                .map(|token| token.blend(&background, 0.85));

        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        rust_widgets::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

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
        // A same-size font change (family, weight, italic, tracking) still invalidates the
        // measured advances; the map is cleared once per font rather than keyed by a heap string.
        {
            let mut measured_with = self.span_widths_font.borrow_mut();
            if measured_with.as_ref() != Some(font) {
                *measured_with = Some(font.clone());
                self.span_widths.borrow_mut().clear();
            }
        }
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
            let (left_spans, right_spans): (&[TokenSpan], &[TokenSpan]) = self
                .syntax_spans
                .as_ref()
                .and_then(|(_, rows)| rows.get(index))
                .map(|(left, right)| (left.as_slice(), right.as_slice()))
                .unwrap_or((&[], &[]));
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
                        self.draw_highlighted(
                            context,
                            Point::new(rect.x + 34, y),
                            text,
                            left_spans,
                            font,
                            row_ink,
                            background,
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
                        self.draw_highlighted(
                            context,
                            Point::new(mid_x + 34, y),
                            text,
                            right_spans,
                            font,
                            row_ink,
                            background,
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
                        let spans = match side {
                            RowSide::Left => left_spans,
                            _ => match line.kind {
                                DiffKind::Removed => left_spans,
                                _ => right_spans,
                            },
                        };
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
                        self.draw_highlighted(
                            context,
                            Point::new(rect.x + 38, y),
                            text,
                            spans,
                            font,
                            row_ink,
                            background,
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
                let mut span_widths = self.span_widths.borrow_mut();
                let widths =
                    span_widths.entry((offset, font.size().to_bits())).or_insert_with(|| {
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
        // Tab focus must be visible on the viewer as on every other control (W5-38).
        let radius = self.style().resolved_corner_radii().to_array()[0];
        self.base.draw_focus_ring(context, rect, radius);
        crate::veil_if_disabled(&self.base, context);
    }
}
