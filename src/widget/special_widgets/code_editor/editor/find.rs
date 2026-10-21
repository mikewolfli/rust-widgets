// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Find/replace, bracket matching and occurrences.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Find and replace ────────────────────────────────────────────────────

    /// Returns the find-bar state.
    pub fn find_state(&self) -> &FindState {
        &self.find
    }

    /// Opens the find bar, optionally revealing the replacement row.
    pub fn open_find(&mut self, replace: bool) {
        self.find.visible = true;
        self.find.replace_visible = replace;
        self.find.focus_replace = false;
        self.dismiss_completion();
        self.close_context_menu();
        self.recompute_match_cache();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes the find bar.
    pub fn close_find(&mut self) {
        if !self.find.visible {
            return;
        }
        self.find.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns `true` while the find bar is on screen.
    pub fn find_visible(&self) -> bool {
        self.find.visible
    }

    /// Sets the search query and recomputes hits.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.find.query = query.into();
        self.find.current = 0;
        self.recompute_match_cache();
        self.search_changed.emit(self.find.match_count());
        self.base.request_redraw();
    }

    /// Sets the replacement text.
    pub fn set_search_replacement(&mut self, replacement: impl Into<String>) {
        self.find.replacement = replacement.into();
        self.base.request_redraw();
    }

    /// Updates the matching policy and recomputes hits.
    pub fn set_search_options(&mut self, options: SearchOptions) {
        self.find.options = options;
        self.find.current = 0;
        self.recompute_match_cache();
        self.search_changed.emit(self.find.match_count());
        self.base.request_redraw();
    }

    /// Returns the current search hits.
    pub fn search_matches(&self) -> &[SearchMatch] {
        &self.find.matches
    }

    /// Finds every occurrence of `query` under the current options.
    pub fn find_all(&self, query: &str) -> Vec<SearchMatch> {
        self.find_all_with(query, self.find.options)
    }

    /// Finds every occurrence of `query` under explicit `options`.
    ///
    /// This is the shared engine behind the find bar and the multi-cursor
    /// occurrence commands, so both agree on what a match is.
    pub fn find_all_with(&self, query: &str, options: SearchOptions) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle: Vec<char> = query.chars().collect();
        let language = self.config.language;
        let model = self.model.borrow();
        let mut matches = Vec::new();
        for (line_index, line) in model.lines.iter().enumerate() {
            let haystack: Vec<char> = line.chars().collect();
            if haystack.len() < needle.len() {
                continue;
            }
            let mut start = 0usize;
            while start + needle.len() <= haystack.len() {
                let candidate = &haystack[start..start + needle.len()];
                let equal = if options.case_sensitive {
                    candidate == needle.as_slice()
                } else {
                    candidate
                        .iter()
                        .map(|ch| ch.to_lowercase().collect::<String>())
                        .collect::<Vec<_>>()
                        == needle
                            .iter()
                            .map(|ch| ch.to_lowercase().collect::<String>())
                            .collect::<Vec<_>>()
                };
                if equal && whole_word_ok(&haystack, start, needle.len(), options, language) {
                    matches.push(SearchMatch {
                        line: line_index,
                        start_column: start,
                        end_column: start + needle.len(),
                    });
                    start += needle.len();
                } else {
                    start += 1;
                }
            }
        }
        matches
    }

    /// Recomputes the hit list, reusing everything outside `dirty`.
    ///
    /// A keystroke changes one line, so re-scanning the document for hits is
    /// wasted work — and in a large file it is the dominant per-key cost. This
    /// removes the hits of the lines in `dirty`, rescans only those, and splices
    /// the results back in place. The list stays in document order, which the
    /// navigation commands rely on.
    ///
    /// `dirty` is `(first_line, inserted_lines, removed_lines)` describing the
    /// edit in line terms: lines `first_line..first_line + removed_lines` were
    /// replaced by `inserted_lines` new ones. `None` means "the shape of the
    /// document changed and nothing can be reused", which falls back to a full
    /// scan.
    pub(super) fn recompute_match_cache_incremental(
        &mut self,
        dirty: Option<(usize, usize, usize)>,
    ) {
        let query = self.find.query.clone();
        if query.is_empty() {
            // No query means no hits, and finding that out costs nothing.
            self.find.matches.clear();
            self.find.current = 0;
            return;
        }
        let Some((first, inserted, removed)) = dirty else {
            self.recompute_match_cache();
            return;
        };
        let options = self.find.options;
        // Drop the hits of the replaced lines from the existing list.
        let last_old = first.saturating_add(removed);
        self.find.matches.retain(|hit| hit.line < first || hit.line >= last_old);
        // Rescan only the lines that now exist at that position.
        let last_new = first.saturating_add(inserted).min(self.line_count());
        let mut fresh: Vec<SearchMatch> = Vec::new();
        for line in first..last_new {
            let Some(text) = self.line_text(line) else { break };
            self.collect_matches_in_line(line, &text, &query, options, &mut fresh);
        }
        // Hits after the edited region keep their relative order; shift them by
        // the line-count delta so they point at the right lines.
        if inserted != removed {
            let delta = inserted as isize - removed as isize;
            for hit in self.find.matches.iter_mut().skip_while(|hit| hit.line < last_old) {
                hit.line = (hit.line as isize + delta).max(0) as usize;
            }
        }
        // Insert the fresh hits where they belong: after the retained hits that
        // precede them, before the ones that follow.
        let insert_at = self.find.matches.partition_point(|hit| hit.line < first);
        self.find.matches.splice(insert_at..insert_at, fresh);
        self.find.current = if self.find.matches.is_empty() {
            0
        } else {
            self.find.current.min(self.find.matches.len() - 1)
        };
    }

    /// Appends every match of `query` within one line to `out`.
    pub(super) fn collect_matches_in_line(
        &self,
        line_index: usize,
        text: &str,
        query: &str,
        options: SearchOptions,
        out: &mut Vec<SearchMatch>,
    ) {
        let needle: Vec<char> = query.chars().collect();
        if needle.is_empty() {
            return;
        }
        let haystack: Vec<char> = text.chars().collect();
        if haystack.len() < needle.len() {
            return;
        }
        let language = self.config.language;
        let mut start = 0usize;
        while start + needle.len() <= haystack.len() {
            let window = &haystack[start..start + needle.len()];
            let equal = if options.case_sensitive {
                window == needle.as_slice()
            } else {
                window
                    .iter()
                    .flat_map(|ch| ch.to_lowercase())
                    .eq(needle.iter().flat_map(|ch| ch.to_lowercase()))
            };
            if equal && whole_word_ok(&haystack, start, needle.len(), options, language) {
                out.push(SearchMatch {
                    line: line_index,
                    start_column: start,
                    end_column: start + needle.len(),
                });
                start += needle.len();
            } else {
                start += 1;
            }
        }
    }

    pub(super) fn recompute_match_cache(&mut self) {
        let query = self.find.query.clone();
        self.find.matches = self.find_all(&query);
        self.find.current = if self.find.matches.is_empty() {
            0
        } else {
            self.find.current.min(self.find.matches.len() - 1)
        };
    }

    /// Advances to the next hit (or the previous one), wrapping around.
    pub fn find_next(&mut self, backwards: bool) -> bool {
        if self.find.matches.is_empty() {
            return false;
        }
        let count = self.find.matches.len();
        self.find.current = if backwards {
            (self.find.current + count - 1) % count
        } else {
            (self.find.current + 1) % count
        };
        self.reveal_current_match()
    }

    /// Reveals and selects the active hit.
    pub fn reveal_current_match(&mut self) -> bool {
        let Some(hit) = self.find.matches.get(self.find.current).cloned() else {
            return false;
        };
        self.cursor.anchor = TextPosition::new(hit.line, hit.start_column);
        self.cursor.head = TextPosition::new(hit.line, hit.end_column);
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Replaces the active hit and advances to the next one.
    pub fn replace_current(&mut self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let Some(hit) = self.find.matches.get(self.find.current).cloned() else {
            return false;
        };
        let replacement = self.find.replacement.clone();
        let start = TextPosition::new(hit.line, hit.start_column);
        let head = TextPosition::new(hit.line, hit.start_column + replacement.chars().count());
        self.splice_range(start, TextPosition::new(hit.line, hit.end_column), &replacement, head);
        self.find_next(false)
    }

    /// Replaces every hit in one undoable step; returns how many were replaced.
    pub fn replace_all(&mut self) -> usize {
        if !self.is_editable() || self.find.matches.is_empty() {
            return 0;
        }
        let replacement = self.find.replacement.clone();
        let before = self.text();
        let mut lines = before.split('\n').map(|line| line.to_string()).collect::<Vec<_>>();
        // Group hits per line, then apply each group right-to-left so earlier
        // columns stay valid while the line is rebuilt.
        let mut groups: Vec<(usize, Vec<SearchMatch>)> = Vec::new();
        for hit in self.find.matches.iter().rev() {
            match groups.last_mut() {
                Some((line, group)) if *line == hit.line => group.push(hit.clone()),
                _ => groups.push((hit.line, vec![hit.clone()])),
            }
        }
        let mut replaced = 0usize;
        for (line_index, group) in groups {
            let Some(line) = lines.get_mut(line_index) else { continue };
            let chars: Vec<char> = line.chars().collect();
            let mut rebuilt = String::new();
            let mut cursor = 0usize;
            for hit in group.iter().rev() {
                if hit.start_column < cursor || hit.end_column > chars.len() {
                    continue;
                }
                rebuilt.extend(&chars[cursor..hit.start_column]);
                rebuilt.push_str(&replacement);
                cursor = hit.end_column;
                replaced += 1;
            }
            rebuilt.extend(&chars[cursor.min(chars.len())..]);
            *line = rebuilt;
        }
        if replaced == 0 {
            return 0;
        }
        let after = join_lines(&lines);
        let head = self.cursor.head;
        self.commit_edit(before, after, head);
        self.recompute_match_cache();
        self.search_changed.emit(self.find.match_count());
        replaced
    }

    pub(crate) fn backspace_find_query(&mut self) {
        if self.find.focus_replace {
            self.find.replacement.pop();
            self.base.request_redraw();
            return;
        }
        self.find.query.pop();
        let query = self.find.query.clone();
        self.set_search_query(query);
    }

    pub(crate) fn append_find_query(&mut self, text: &str) {
        if self.find.focus_replace {
            self.find.replacement.push_str(text);
            self.base.request_redraw();
            return;
        }
        let mut query = self.find.query.clone();
        query.push_str(text);
        self.set_search_query(query);
    }

    // ── Bracket matching and occurrences ────────────────────────────────────

    /// Returns the caret bracket and its match, when enabled.
    ///
    /// Cached by `(revision, caret)`: resolving a pair scans to its partner, and the paint path
    /// asks once per frame. A still frame therefore costs a comparison, not a document walk.
    pub fn matching_brackets(&self) -> Option<(TextPosition, TextPosition)> {
        if !self.config.highlight_brackets {
            return None;
        }
        let revision = self.revision;
        let caret = self.cursor.head;
        if let Some(cached) = *self.bracket_cache.borrow() {
            if cached.revision == revision && cached.caret == caret {
                return cached.pair;
            }
        }
        let pair = self.resolve_matching_brackets();
        *self.bracket_cache.borrow_mut() = Some(BracketCache { revision, caret, pair });
        pair
    }

    /// Resolves the caret bracket against the document, without the cache.
    fn resolve_matching_brackets(&self) -> Option<(TextPosition, TextPosition)> {
        let candidate = match self.char_at(self.cursor.head).filter(|ch| pairs::is_bracket(*ch)) {
            Some(ch) => (self.cursor.head, ch),
            None => {
                let previous = self.cursor.head.column.checked_sub(1)?;
                let position = TextPosition::new(self.cursor.head.line, previous);
                let ch = self.char_at(position).filter(|ch| pairs::is_bracket(*ch))?;
                (position, ch)
            }
        };
        let (position, ch) = candidate;
        // A `{` in a string literal is text, not a bracket. The matcher used to scan the
        // raw line, so a caret next to `printf("{}")` highlighted that brace against the
        // block's closing brace two lines down — two unrelated characters marked as a
        // pair. The lexer already knows which spans are strings and comments, so the
        // question is asked once per position scanned.
        if !self.is_code_position(position) {
            return None;
        }
        self.find_matching_bracket(position, ch).map(|matched| (position, matched))
    }

    /// `true` when `position` is code rather than part of a string or a comment.
    pub(super) fn is_code_position(&self, position: TextPosition) -> bool {
        let Some(text) = self.line_text(position.line) else { return true };
        let byte = LineGrid::new(&text, self.config.tab_width).byte_of_char(position.column);
        !self.tokens_on_line(position.line).iter().any(|span| {
            matches!(span.kind, TokenKind::String | TokenKind::Comment)
                && byte >= span.start
                && byte < span.end
        })
    }

    pub(super) fn find_matching_bracket(
        &self,
        start: TextPosition,
        ch: char,
    ) -> Option<TextPosition> {
        let forward = matches!(ch, '(' | '[' | '{');
        let mut depth = 0isize;
        if forward {
            for line in start.line..self.line_count() {
                let Some(text) = self.line_text(line) else { continue };
                let skip = self.skipped_spans(line);
                for (column, (byte, candidate)) in text.char_indices().enumerate() {
                    if line == start.line && column <= start.column {
                        continue;
                    }
                    if is_skipped(byte, &skip) {
                        continue;
                    }
                    if candidate == ch {
                        depth += 1;
                    } else if closes_bracket(ch, candidate) {
                        if depth == 0 {
                            return Some(TextPosition::new(line, column));
                        }
                        depth -= 1;
                    }
                }
            }
        } else {
            for line in (0..=start.line).rev() {
                let Some(text) = self.line_text(line) else { continue };
                let skip = self.skipped_spans(line);
                let limit = if line == start.line { start.column } else { text.chars().count() };
                // Walked backwards so the nearest candidate wins, which is what the
                // reader means by "the bracket that matches mine".
                for column in (0..limit).rev() {
                    let Some((byte, candidate)) = byte_and_char_at(&text, column) else { continue };
                    if is_skipped(byte, &skip) {
                        continue;
                    }
                    if candidate == ch {
                        depth += 1;
                    } else if opens_bracket(ch, candidate) {
                        if depth == 0 {
                            return Some(TextPosition::new(line, column));
                        }
                        depth -= 1;
                    }
                }
            }
        }
        None
    }

    /// Byte spans of `line` that hold a string or a comment.
    ///
    /// Bracket matching walks these spans past: a bracket inside one is not a
    /// structural bracket, so it can neither open nor close a pair.
    pub(super) fn skipped_spans(&self, line: usize) -> Vec<(usize, usize)> {
        self.tokens_on_line(line)
            .iter()
            .filter(|span| matches!(span.kind, TokenKind::String | TokenKind::Comment))
            .map(|span| (span.start, span.end))
            .collect()
    }

    /// Returns every occurrence of the currently selected word.
    pub fn occurrences_of_selection(&self) -> Vec<SearchMatch> {
        if !self.config.highlight_occurrences {
            return Vec::new();
        }
        let Some(selected) = self.selected_text() else { return Vec::new() };
        if selected.is_empty() || selected.contains('\n') || selected.chars().count() > 64 {
            return Vec::new();
        }
        // Reuse `find_all_with` with whole-word, case-sensitive matching without
        // disturbing the user's own find-bar policy.
        self.find_all_with(&selected, SearchOptions { case_sensitive: true, whole_word: true })
    }

    /// Refreshes [`Self::occurrence_cache`] if the selection or text changed.
    ///
    /// Called once per paint, before the rows are drawn. The computation it guards
    /// is a whole-document search, so it must not run per row (it used to run per
    /// frame); the guard is the (selection, revision) pair, which are exactly the
    /// two inputs the answer depends on.
    pub(crate) fn prepare_occurrences(&mut self) {
        let bounds = self.cursor.bounds();
        if let Some(cache) = &self.occurrence_cache {
            if cache.bounds == bounds && cache.revision == self.revision {
                return;
            }
        }
        let matches = self.occurrences_of_selection();
        self.occurrence_cache = Some(OccurrenceCache { bounds, revision: self.revision, matches });
    }
}
