// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Visual rows, wrap segments and the minimap summary.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Visual row mapping ─────────────────────────────────────────────────

    /// Returns the document lines that are visible, folds respected.
    ///
    /// When nothing is folded this is a contiguous range, so it is produced by
    /// arithmetic instead of by filtering every line of the document. The old
    /// implementation allocated a `Vec` of the whole document on every call, and
    /// the paint path calls this several times per frame — which is what made a
    /// large file cost milliseconds per repaint even with nothing to draw.
    pub(crate) fn visible_document_lines(&self) -> Vec<usize> {
        if self.folded_lines.is_empty() {
            return (0..self.line_count()).collect();
        }
        self.visible_lines()
    }

    /// Returns the visible document lines within the viewport window, plus slack.
    ///
    /// This is the iterator the paint loop should use: it never materialises the
    /// document, so its cost depends on the viewport. With folds present the walk starts
    /// at the line the viewport actually begins on (read off the row-prefix table) instead
    /// of filtering every line of the file into a `Vec` first — that allocation was the
    /// folded counterpart of the full-document scan `visible_document_lines` avoids (J-P04).
    pub(crate) fn viewport_document_lines(&self) -> alloc::vec::IntoIter<usize> {
        if self.folded_lines.is_empty() {
            let slack = self.visible_rows.max(1) + 1;
            let first = self.scroll_visual_row.saturating_sub(slack).min(self.line_count());
            let last = (self.scroll_visual_row + slack * 2).min(self.line_count());
            return (first..last).collect::<Vec<usize>>().into_iter();
        }
        let slack = self.visible_rows.max(1) + 1;
        let first_row = self.scroll_visual_row.saturating_sub(slack);
        let last_row = self.scroll_visual_row + slack * 2;
        let count = self.line_count();
        // The prefix table holds the exact start row of every line; without it (a caller
        // that reads coordinates before any refresh) the walk starts at the top, which is
        // correct at the old cost.
        let start_line = if self.visual_row_prefix.len() == count + 1 {
            self.visual_row_to_line(first_row)
        } else {
            0
        };
        let mut rows = self.line_to_visual_row(start_line);
        let mut out = Vec::new();
        for line in start_line..count {
            if self.is_hidden(line) {
                continue;
            }
            let segments = self.wrap_segments(line);
            if rows + segments > first_row && rows <= last_row {
                out.push(line);
            }
            rows += segments;
            if rows > last_row {
                break;
            }
        }
        out.into_iter()
    }

    /// Fills `line_states` / `line_tokens` for the rows about to be painted.
    ///
    /// The paint helpers take `&self`, so the one place that can mutate the cache
    /// is here, before them. Only the on-screen window is prepared: a document
    /// with a million lines costs the same as one with fifty, which is the
    /// invariant that keeps scrolling smooth (principle: per-frame work is
    /// proportional to the viewport, never to the document).
    pub(crate) fn prepare_visible_line_cache(&mut self) {
        let budget = self.visible_rows.max(1) + 1;
        let first = self.scroll_visual_row.saturating_sub(budget);
        let last = (self.scroll_visual_row + budget * 2).min(self.line_count());
        // Advance the entry-state chain up to the first painted line. This is
        // where a multi-line construct opened far above is resolved, so it is
        // bounded by the distance from the last known-clean line, not by the
        // document length.
        self.ensure_line_states(last);
        for line in first..last {
            self.fill_tokens_on_line(line);
        }
    }

    /// Returns the cached entry state of `line`, if the lexer has reached it.
    ///
    /// The renderer uses this to tell a continuation row apart from a fresh one
    /// without re-running the lexer, which is what makes a multi-line block
    /// comment paint as one visually continuous band.
    ///
    /// # What this does **not** answer
    ///
    /// The cache is filled by the paint path, so a line the walk has not reached reads back
    /// `LineState::Clean` — its `Default` — which is the same value a genuinely clean line
    /// returns. `Clean` here therefore means "clean, *or* not looked at yet", and the accessor
    /// alone cannot say which. A caller that needs a dependable answer for a line that may not
    /// be on screen must either check [`CodeEditor::line_state_is_known`] first, or call
    /// [`CodeEditor::resolve_line_state`], which advances the chain so the answer is real.
    pub fn line_state(&self, line: usize) -> LineState {
        self.line_states.get(line).copied().unwrap_or_default()
    }

    /// Returns whether `line`'s entry state has been **proven** by the lexer.
    ///
    /// # Why the predicate has to be part of the public contract
    ///
    /// [`CodeEditor::line_state`] can only report the cached value, and the cache's untouched
    /// tail holds `Clean`. That makes two different facts — "this line starts outside any
    /// construct" and "this line has not been lexed yet" — share one representation, which is
    /// the "success-shaped default hiding a failure" pattern the project rules forbid. The
    /// distinction is real and reachable: right after `set_text("/* open\nstill\n*/")`,
    /// `line_state(1)` answers `Clean` while the line is in fact inside a block comment.
    ///
    /// This predicate restores the missing distinction without changing the value type:
    ///
    /// * `true`  — the walk reached `line`, so `line_state` is trustworthy even when non-clean;
    /// * `false` — `line_state` is the `Default` and proves nothing.
    ///
    /// A consumer that needs an answer rather than a guess follows `false` with
    /// [`CodeEditor::resolve_line_state`] (or simply calls that method directly).
    pub fn line_state_is_known(&self, line: usize) -> bool {
        // `walked_lines` is how far the entry-state chain has been proven, and it counts the
        // entry state itself: after the walk processes line `n` it records `n + 1` as the exit
        // state of `n`, which *is* the entry state of `n + 1`. So index `line` is proven exactly
        // when `line <= walked_lines`. Index 0 is always proven: a document starts `Clean`.
        line < self.line_states.len() && line <= self.walked_lines
    }

    /// Advances the entry-state chain to `line`, then returns the proven state.
    ///
    /// This is the "pay the cost, get a real answer" half of the contract. The paint path is not
    /// the only caller that needs a dependable state: a designer asks about lines that are not on
    /// screen — to fold a region, infer indentation, or decide "is this line inside a block
    /// comment" — and for those, [`CodeEditor::line_state`] would hand back the unproven default.
    ///
    /// The walk is bounded by the distance from the last proven line to `line`, not by the
    /// document: a request for a line far below the viewport costs a walk through the intervening
    /// lines (which stops early across plain code), and a request for a line already proven costs
    /// nothing.
    pub fn resolve_line_state(&mut self, line: usize) -> LineState {
        // `ensure_line_states(upto)` proves entry states for indices `0..=upto`, so asking for
        // `line` is exactly enough to make `line_state(line)` real.
        self.ensure_line_states(line);
        self.line_state(line)
    }

    /// Returns the token spans of `line` with its entry state **resolved** first.
    ///
    /// [`CodeEditor::tokens_on_line`] tokenizes with whatever entry state happens to be cached,
    /// so for a line the paint path has not reached it produces the `Clean`-based segmentation
    /// — which is wrong exactly where it matters (a continuation line of a block comment or a
    /// multi-line string). That makes the same query return different answers at different times
    /// depending on scroll history. This variant removes the dependence: it advances the chain to
    /// `line` first, so the spans are the ones a full walk would produce.
    pub fn tokens_on_line_resolved(&mut self, line: usize) -> Vec<TokenSpan> {
        self.resolve_line_state(line);
        // A span list computed before this line's entry state was proven was derived from the
        // `Clean` default. Drop it rather than return it as if it were the resolved answer.
        if let Some(slot) = self.line_tokens.get_mut(line) {
            *slot = None;
        }
        self.fill_tokens_on_line(line);
        self.tokens_on_line(line)
    }

    // ── Minimap summary ─────────────────────────────────────────────────────

    /// Ensures the minimap summary matches the current document and strip size.
    ///
    /// Rebuilds only when the bucket count is stale (first paint, a resize, or
    /// an edit that changed the line count). Steady-state repaints read the
    /// existing buckets, so scrolling costs nothing here.
    pub(crate) fn prepare_minimap(&mut self) {
        if !self.config.show_minimap {
            return;
        }
        let rows = self.minimap_rows();
        let count = self.line_count();
        // One bucket per strip row, but never more buckets than the partition can
        // fill: a 14-line file has 14 bars, and a long file has `ceil(count / per)`
        // bars rather than `rows` with a blank tail.
        //
        // The previous check derived `expected` from `count.div_ceil(per_bucket)` while
        // `rebuild_minimap` resized to `rows`, so for any document shorter than the
        // strip the two could never agree — the map was rebuilt, line by line, on
        // **every frame**. With a large document that is a full pass over the file per
        // frame behind a 60 fps repaint.
        let (buckets, _) = minimap_partition(count, rows);
        let stale = self.minimap_buckets.len() != buckets || self.minimap_line_count != count;
        if stale {
            self.rebuild_minimap(rows);
            self.minimap_line_count = count;
        }
    }

    /// Builds the minimap summary if it is stale and returns it.
    ///
    /// The control paints the minimap from this summary, and it is the same data a host
    /// needs to draw its own decoration beside the buffer. Exposing it means the host does
    /// not have to summarise the document a second time (the old `editor_view.rs` did), so
    /// the two views cannot disagree about where a block sits.
    ///
    /// `rows` is the number of strip rows to summarise into. The cached summary is keyed by
    /// the document's line count and the resulting bucket count, so passing the same `rows`
    /// the control paints with lets the next paint reuse it; a different value makes the next
    /// paint rebuild it. The summary is built here even when the control's own minimap is
    /// hidden, because a host decoration may still want it.
    pub fn minimap_summary(&mut self, rows: usize) -> &[MinimapBucket] {
        let rows = rows.max(1);
        let count = self.line_count();
        let (buckets, _) = minimap_partition(count, rows);
        if self.minimap_buckets.len() != buckets || self.minimap_line_count != count {
            self.rebuild_minimap(rows);
            self.minimap_line_count = count;
        }
        &self.minimap_buckets
    }

    /// Returns the minimap summary as last built, without building it.
    ///
    /// Empty until something has built it — the paint path, or a call to
    /// [`Self::minimap_summary`]. A read-only reader that must not trigger a document pass
    /// uses this and treats an empty slice as "not summarised yet".
    pub fn minimap_summary_cached(&self) -> &[MinimapBucket] {
        &self.minimap_buckets
    }

    /// Returns how many strip rows the minimap has room for.
    pub(crate) fn minimap_rows(&self) -> usize {
        let rect = self.geometry();
        let top = self.text_origin_y();
        let usable = rect.height as i32 - top - self.status_bar_height();
        (usable.max(1) as usize).clamp(1, MAX_MINIMAP_ROWS)
    }

    /// Returns how many lines have cached token spans.
    ///
    /// Exposed so tests can assert the cache stays proportional to the viewport
    /// rather than to the document; there is no production caller.
    #[cfg(test)]
    pub(crate) fn cached_token_lines(&self) -> usize {
        self.line_tokens.iter().filter(|slot| slot.is_some()).count()
    }

    /// Returns whether `line` has a cached token span list.
    #[cfg(test)]
    pub(crate) fn has_cached_tokens(&self, line: usize) -> bool {
        self.line_tokens.get(line).map(|slot| slot.is_some()).unwrap_or(false)
    }

    /// Fills the entry-state chain up to `upto`, for tests of the increments.
    #[cfg(test)]
    pub(crate) fn prime_line_states(&mut self, upto: usize) {
        self.ensure_line_states(upto);
    }

    /// Returns how many bytes the top undo entry retains.
    ///
    /// Exposed so a test can assert a keystroke's history cost follows the edit
    /// and not the document; there is no production caller. The count is taken
    /// when the entry is pushed, because a `dyn UndoCommand` cannot be asked for
    /// its footprint through the trait.
    #[cfg(test)]
    pub(crate) fn debug_last_checkpoint_bytes(&self) -> usize {
        self.last_checkpoint_bytes
    }

    /// Whether the row-prefix table is currently absent.
    ///
    /// Exposed so a test can assert that the fold-free, wrap-free fast path really
    /// does skip the O(document) build instead of silently paying for it.
    #[cfg(test)]
    pub(crate) fn visual_row_prefix_is_empty(&self) -> bool {
        self.visual_row_prefix.is_empty()
    }

    /// Length of the row-prefix table (`line_count + 1` when built).
    #[cfg(test)]
    pub(crate) fn visual_row_prefix_len(&self) -> usize {
        self.visual_row_prefix.len()
    }

    /// Rebuilds every minimap bucket from scratch.
    ///
    /// Called when the bucket count or the document size changed — opening a
    /// file, switching tabs, or an edit that added or removed lines. The cost is
    /// one pass over the document, which is the same order as the load itself
    /// and therefore not something a keystroke ever pays.
    pub(crate) fn rebuild_minimap(&mut self, rows: usize) {
        let count = self.line_count();
        let (buckets, per_bucket) = minimap_partition(count, rows);
        self.minimap_buckets.clear();
        self.minimap_buckets.resize(buckets, MinimapBucket::default());
        for (index, bucket) in self.minimap_buckets.iter_mut().enumerate() {
            bucket.start_line = index.saturating_mul(per_bucket);
        }
        if count == 0 {
            return;
        }
        for line in 0..count {
            let bucket = line / per_bucket;
            let Some(text) = self.line_text(line) else { break };
            let Some(slot) = self.minimap_buckets.get_mut(bucket) else { continue };
            MinimapBucket::accumulate(slot, &text);
            // `accumulate` keeps the longest line, so it cannot also count lines; the
            // count is maintained here, at the one place that sees every line exactly
            // once. `patch_minimap_line` deliberately does not touch it: it re-folds a
            // single line and would otherwise count the same line again.
            if !text.trim().is_empty() {
                slot.non_blank_lines = slot.non_blank_lines.saturating_add(1);
            }
        }
    }

    /// Re-folds `line` into its bucket after an edit, without a full pass.
    ///
    /// The bucket keeps a running best (`indent`, `length`) rather than a sum,
    /// so a changed line may only ever *raise* the recorded maximum. A line that
    /// shrank leaves the bucket showing a bar one row too long until the next
    /// rebuild, which is the correct trade: a stale-but-close minimap is visible
    /// only at a glance, while a per-keystroke document pass is not affordable.
    pub(crate) fn patch_minimap_line(&mut self, line: usize) {
        let rows = self.minimap_buckets.len();
        let count = self.line_count();
        if rows == 0 || count == 0 {
            return;
        }
        let (_, per_bucket) = minimap_partition(count, rows);
        let bucket = line / per_bucket;
        // Read the text before taking the mutable bucket borrow: `line_text`
        // borrows the model, and holding both at once is a conflict.
        let Some(text) = self.line_text(line) else { return };
        let Some(slot) = self.minimap_buckets.get_mut(bucket) else { return };
        MinimapBucket::accumulate(slot, &text);
    }

    pub(crate) fn wrap_segments(&self, line: usize) -> usize {
        if !self.config.word_wrap {
            return 1;
        }
        let columns = self.text_columns().max(1);
        // Segments are counted in **grid columns**, not chars: a CJK line wraps
        // twice as early as its char count suggests, and a tab spans several
        // columns. Borrowing the line avoids the `String` clone `line_text` makes.
        let model = self.model.borrow();
        LineGrid::new(model.line(line), self.config.tab_width).segments(columns)
    }

    pub(crate) fn total_visual_rows(&self) -> usize {
        // When the row-prefix table exists it already holds the exact total as its
        // trailing entry, so the count comes off the same table the lookups use
        // instead of a second traversal that could disagree with it.
        let rows: usize = if self.visual_row_prefix.len() == self.line_count() + 1 {
            self.visual_row_prefix.last().copied().unwrap_or(self.line_count())
        } else if self.folded_lines.is_empty() {
            // The common case: no folds, so every line below the last fold is
            // visible and the total is a single multiply. Walking the document
            // here is what made scrolling a million-line file cost milliseconds
            // per frame.
            self.line_count()
        } else {
            self.visible_document_lines().iter().map(|line| self.wrap_segments(*line)).sum()
        };
        let rows = if self.config.word_wrap && self.visual_row_prefix.len() != self.line_count() + 1
        {
            // Wrapping makes rows depend on each line's length, so the count
            // cannot be derived without measuring — but only the visible span is
            // needed for layout, and the exact total only feeds the scrollbar.
            self.wrapped_row_estimate()
        } else {
            rows
        };
        rows.saturating_add(self.inline_diagnostics().len())
    }

    /// Estimates the wrapped row count without walking every line.
    ///
    /// A wrapped document's row count is only used to size the scrollbar thumb,
    /// so an estimate is enough: it is exact whenever no line wraps, and never
    /// zero, so the thumb cannot divide by zero or vanish.
    pub(super) fn wrapped_row_estimate(&self) -> usize {
        if self.folded_lines.is_empty() {
            let columns = self.text_columns().max(1);
            let budget = self.total_line_chars();
            budget.div_ceil(columns).max(self.line_count())
        } else {
            self.visible_document_lines().iter().map(|line| self.wrap_segments(*line)).sum()
        }
    }

    /// Returns the visual row at which a document line starts.
    ///
    /// `B4`: the two cases that need a walk — wrapping and folding — now read a
    /// prefix table instead of walking to `line` on every call, so the cost is
    /// paid once per document-shape change rather than once per lookup. The
    /// fold-free, wrap-free case keeps its O(1) closed form and never builds the
    /// table, which is what keeps a million-line scroll free of work.
    pub(crate) fn line_to_visual_row(&self, line: usize) -> usize {
        if self.folded_lines.is_empty() && !self.config.word_wrap {
            return line;
        }
        self.visual_row_prefix
            .get(line)
            .copied()
            .unwrap_or_else(|| self.visual_row_count_fallback(line))
    }

    /// Returns the document line occupying a visual row.
    ///
    /// The reverse of [`Self::line_to_visual_row`]. With the prefix table present
    /// this is a binary search; the fold-free, wrap-free case is the identity.
    pub(crate) fn visual_row_to_line(&self, row: usize) -> usize {
        let last = self.line_count().saturating_sub(1);
        if self.folded_lines.is_empty() && !self.config.word_wrap {
            return row.min(last);
        }
        if self.visual_row_prefix.len() == self.line_count() + 1 {
            // `partition_point` finds the first line whose start row is strictly
            // past `row`; the line before it owns the row. The table is monotone
            // because every visible line contributes at least one row.
            let index = self.visual_row_prefix.partition_point(|start| *start <= row);
            return index.saturating_sub(1).min(last);
        }
        self.visual_row_to_line_fallback(row)
    }

    /// The uncached forward lookup, used only when the prefix table is absent.
    ///
    /// This is the pre-`B4` implementation kept as a correctness backstop: a caller
    /// that reads a coordinate before any refresh must still get a right answer,
    /// even at the old cost.
    pub(super) fn visual_row_count_fallback(&self, line: usize) -> usize {
        let mut row = 0usize;
        for document_line in self.visible_document_lines() {
            if document_line >= line {
                break;
            }
            row += self.wrap_segments(document_line);
        }
        row
    }

    /// The uncached reverse lookup, used only when the prefix table is absent.
    pub(super) fn visual_row_to_line_fallback(&self, row: usize) -> usize {
        let mut cursor = 0usize;
        let mut last = 0usize;
        for document_line in self.visible_document_lines() {
            last = document_line;
            let segments = self.wrap_segments(document_line);
            if row < cursor + segments {
                return document_line;
            }
            cursor += segments;
        }
        last
    }

    /// Rebuilds [`Self::visual_row_prefix`] for the current document shape.
    ///
    /// Called from the same places that invalidate the other derived caches —
    /// edits, tab switches, fold changes and the first cell measurement — so the
    /// table cannot describe a document that no longer exists. The fold-free,
    /// wrap-free document deliberately skips the build: its row and line numbers
    /// are equal, and paying O(document) for a table that would only restate that
    /// would put the cost straight back on the hot path.
    pub(crate) fn rebuild_visual_row_prefix(&mut self) {
        self.rebuild_visual_row_prefix_after(None);
    }

    /// [`Self::rebuild_visual_row_prefix`] with an incremental hint.
    ///
    /// `dirty` is `(first_line, lines_now, lines_before)` for a single contiguous edit. A
    /// row's contribution only depends on its own text, so every prefix value **above**
    /// the edit is still correct, and the values below it only change by the shift the
    /// edit introduced. Recomputing from `first_line` and stopping at the first line where
    /// the running total matches the recorded one therefore costs O(rows from the edit to
    /// the first unchanged wrap count) instead of O(document) per keystroke (J-P05).
    ///
    /// A line-count change invalidates the table's length, and a caller that cannot
    /// describe the edit passes `None`; both fall back to the full rebuild.
    pub(crate) fn rebuild_visual_row_prefix_after(&mut self, dirty: Option<(usize, usize, usize)>) {
        let count = self.line_count();
        if self.folded_lines.is_empty() && !self.config.word_wrap {
            self.visual_row_prefix.clear();
            return;
        }
        let columns = self.text_columns().max(1);
        let folded = !self.folded_lines.is_empty();
        if let Some((first, inserted, removed)) = dirty {
            if inserted == removed
                && self.visual_row_prefix.len() == count + 1
                && first < count
                && !self.visual_row_prefix.is_empty()
            {
                // Clone the handle so the model borrow is not tied to `self`; the loop
                // below mutates the prefix table while reading line text.
                let model = Rc::clone(&self.model);
                let model = model.borrow();
                // Fold visibility is read from a local copy of the merged ranges: the
                // loop holds a mutable borrow of `visual_row_prefix`, so it cannot call
                // `self.is_hidden` while it runs. The ranges are a handful of pairs.
                let hidden_ranges = self.hidden_ranges.clone();
                let hidden = |line: usize| {
                    let index = hidden_ranges.partition_point(|(start, _)| *start <= line);
                    index > 0 && hidden_ranges[index - 1].1 >= line
                };
                let prefix = &mut self.visual_row_prefix;
                let mut row = prefix[first];
                let mut line = first;
                while line < count {
                    if !(folded && hidden(line)) {
                        row += if self.config.word_wrap {
                            LineGrid::new(model.line(line), self.config.tab_width)
                                .segments(columns)
                                .max(1)
                        } else {
                            1
                        };
                    }
                    let previous = prefix[line + 1];
                    prefix[line + 1] = row;
                    line += 1;
                    // Re-converged: from here on every line contributes what it did
                    // before, so the recorded tail is already correct.
                    if line > first && row == previous && line < count {
                        break;
                    }
                }
                return;
            }
        }
        let mut prefix = Vec::with_capacity(count + 1);
        let mut row = 0usize;
        {
            let model = self.model.borrow();
            for line in 0..count {
                prefix.push(row);
                if folded && self.is_hidden(line) {
                    continue;
                }
                row += if self.config.word_wrap {
                    LineGrid::new(model.line(line), self.config.tab_width).segments(columns).max(1)
                } else {
                    1
                };
            }
        }
        // The trailing entry is the total row count, so a binary search that runs
        // off the end still has a value to stop on.
        prefix.push(row);
        self.visual_row_prefix = prefix;
    }

    /// Returns the summed character count of every line.
    ///
    /// Used only to size the wrapped-scrollbar estimate; it walks the line index
    /// but not the text, and no line's contents are copied.
    pub(super) fn total_line_chars(&self) -> usize {
        let model = self.model.borrow();
        model.lines.iter().map(|line| line.chars().count()).sum()
    }

    /// Returns the visual rows of the whole document.
    pub fn visual_lines(&self) -> Vec<VisualLine> {
        let columns = self.text_columns().max(1);
        let mut result = Vec::new();
        for line in self.visible_document_lines() {
            let segments = self.wrap_segments(line);
            for segment in 0..segments {
                let start_column = segment * columns;
                let end_column = if self.config.word_wrap {
                    ((segment + 1) * columns).min(self.line_len(line))
                } else {
                    self.line_len(line)
                };
                result.push(VisualLine { line, segment, start_column, end_column });
            }
        }
        result
    }

    /// Returns the visible row range as `(first, last)`.
    pub fn visible_row_range(&self) -> (usize, usize) {
        let rows = self.visible_rows();
        (self.scroll_visual_row, self.scroll_visual_row + rows.saturating_sub(1))
    }

    /// Returns the total number of visual rows in the document.
    pub fn visual_row_count(&self) -> usize {
        self.total_visual_rows()
    }
}
