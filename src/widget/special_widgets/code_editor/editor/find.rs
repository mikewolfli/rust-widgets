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
        self.goto_bar_visible = false;
        self.goto_mode = false;
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
        self.goto_bar_visible = false;
        self.goto_mode = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns `true` while the find bar is on screen.
    pub fn find_visible(&self) -> bool {
        self.find.visible
    }

    /// Returns whether the find bar is acting as a "go to line" bar.
    pub fn goto_bar_visible(&self) -> bool {
        self.goto_bar_visible
    }

    /// Opens the "go to line" bar, reusing the find bar's input row.
    pub fn open_goto_bar(&mut self) {
        self.goto_bar_visible = true;
        self.goto_mode = true;
        self.find.visible = true;
        self.find.query.clear();
        self.find.replace_visible = false;
        self.find.focus_replace = false;
        self.dismiss_completion();
        self.close_context_menu();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes the "go to line" bar.
    pub fn close_goto_bar(&mut self) {
        if !self.goto_bar_visible && !self.goto_mode {
            return;
        }
        self.goto_bar_visible = false;
        self.goto_mode = false;
        self.find.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Opens or closes the "go to line" bar.
    pub fn set_goto_bar_visible(&mut self, visible: bool) {
        if visible {
            self.open_goto_bar();
        } else {
            self.close_goto_bar();
        }
    }

    /// Jumps to the line number currently typed into the goto bar.
    ///
    /// Line numbers are one-based, as every editor and compiler reports them.
    /// A non-numeric or zero entry closes the bar without moving the caret rather
    /// than jumping to an invented line.
    pub fn commit_goto(&mut self) -> bool {
        let query = self.find.query.trim().to_string();
        let line = query.parse::<usize>().ok();
        let moved = match line {
            Some(line) if line > 0 => {
                self.goto_line(line - 1);
                true
            }
            _ => false,
        };
        self.close_goto_bar();
        moved
    }

    /// Sets the search query and recomputes hits.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.find.query = query.into();
        self.find.current = 0;
        self.recompute_match_cache();
        self.search_changed.emit(self.find.match_count());
        self.emit_search_result();
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
        self.emit_search_result();
        self.base.request_redraw();
    }

    /// Emits the active hit, total and its document position.
    ///
    /// `current` and `line`/`column` are one-based (the values a status bar
    /// shows), and `current` is zero when there is no hit.
    pub fn emit_search_result(&self) {
        let total = self.find.matches.len();
        let (current, line, column) = self
            .find
            .matches
            .get(self.find.current)
            .map(|hit| (self.find.current + 1, hit.line + 1, hit.start_column + 1))
            .unwrap_or((0, 0, 0));
        self.search_result.emit((current, total, line, column));
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
        // Lowercase the needle once. The old loop lowercased every character of
        // the needle on every window position, allocating a `String` per character.
        let needle_lower = lowercase_chars(&needle);
        let needle_ascii = needle.iter().all(|ch| ch.is_ascii());
        let language = self.config.language;
        let model = self.model.borrow();
        let mut matches = Vec::new();
        for (line_index, line) in model.lines.iter().enumerate() {
            // A line shorter than the needle cannot contain it; this is a byte-length test,
            // and a line's byte length is never below its character count.
            if line.len() < needle.len() {
                continue;
            }
            if options.use_regex {
                let haystack: Vec<char> = line.chars().collect();
                for (start, end) in regex_matches(&haystack, &needle, options.case_sensitive) {
                    if whole_word_ok(&haystack, start, end - start, options, language) {
                        matches.push(SearchMatch {
                            line: line_index,
                            start_column: start,
                            end_column: end,
                        });
                    }
                }
                continue;
            }
            for (start, end) in
                scan_literal_line(line, &needle, &needle_lower, needle_ascii, options, language)
            {
                matches.push(SearchMatch {
                    line: line_index,
                    start_column: start,
                    end_column: end,
                });
            }
        }
        drop(model);
        if options.in_selection && !self.cursor.is_collapsed() {
            let (start, end) = self.cursor.bounds();
            matches.retain(|hit| {
                let hit_start = TextPosition::new(hit.line, hit.start_column);
                let hit_end = TextPosition::new(hit.line, hit.end_column);
                hit_start >= start && hit_end <= end
            });
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
        let needle_lower = lowercase_chars(&needle);
        let needle_ascii = needle.iter().all(|ch| ch.is_ascii());
        if options.use_regex {
            let haystack: Vec<char> = text.chars().collect();
            for (start, end) in regex_matches(&haystack, &needle, options.case_sensitive) {
                if whole_word_ok(&haystack, start, end - start, options, self.config.language) {
                    out.push(SearchMatch {
                        line: line_index,
                        start_column: start,
                        end_column: end,
                    });
                }
            }
            return;
        }
        if text.len() < needle.len() {
            return;
        }
        let language = self.config.language;
        for (start, end) in
            scan_literal_line(text, &needle, &needle_lower, needle_ascii, options, language)
        {
            out.push(SearchMatch { line: line_index, start_column: start, end_column: end });
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
        // Anchor a fresh hit list to the caret: the first "next" must go to the
        // nearest hit after the insertion point, not back to the top of the file.
        // Re-querying used to leave `current` at 0, so the first navigation jumped
        // to the document start regardless of where the reader was working.
        self.find.current = self.anchor_match_index();
    }

    /// Index of the first hit at or after the caret, or 0 when there is none.
    fn anchor_match_index(&self) -> usize {
        let caret = self.cursor.head;
        self.find
            .matches
            .iter()
            .position(|hit| TextPosition::new(hit.line, hit.start_column) >= caret)
            .unwrap_or(0)
    }

    /// Advances to the next hit (or the previous one), wrapping around.
    pub fn find_next(&mut self, backwards: bool) -> bool {
        if self.find.matches.is_empty() {
            return false;
        }
        let count = self.find.matches.len();
        if backwards {
            self.find.wrapped = self.find.current == 0;
            self.find.current = (self.find.current + count - 1) % count;
        } else {
            self.find.wrapped = self.find.current + 1 >= count;
            self.find.current = (self.find.current + 1) % count;
        }
        self.reveal_current_match();
        self.emit_search_result();
        true
    }

    /// Whether the last find navigation wrapped past a document edge.
    pub fn find_wrapped(&self) -> bool {
        self.find.wrapped
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
        self.emit_search_result();
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
        let revision = self.revision.get();
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
        self.find_all_with(
            &selected,
            SearchOptions { case_sensitive: true, whole_word: true, ..SearchOptions::default() },
        )
    }

    /// Refreshes [`Self::occurrence_cache`] if the selection or text changed.
    ///
    /// Called once per paint, before the rows are drawn. The computation it guards
    /// is a whole-document search, so it must not run per row (it used to run per
    /// frame); the guard is the (selection, revision) pair, which are exactly the
    /// two inputs the answer depends on.
    pub(crate) fn prepare_occurrences(&mut self) {
        let bounds = self.cursor.bounds();
        let revision = self.revision.get();
        if let Some(cache) = &self.occurrence_cache {
            if cache.bounds == bounds && cache.revision == revision {
                return;
            }
        }
        let matches = self.occurrences_of_selection();
        self.occurrence_cache = Some(OccurrenceCache { bounds, revision, matches });
    }
}

// ── Regular-expression subset ──────────────────────────────────────────────
//
// A code search needs anchors, classes and a few quantifiers, not the full
// regular-expression language, so the editor carries this small matcher rather
// than a dependency. Every construct it understands is documented on
// `SearchOptions::use_regex`; anything else is matched literally.

#[derive(Debug, Clone)]
enum RegexAtom {
    Literal(char),
    Any,
    Class { negated: bool, ranges: Vec<(char, char)> },
    Digit,
    Word,
    Space,
    WordBoundary,
    LineStart,
    LineEnd,
}

#[derive(Debug, Clone)]
struct RegexStep {
    atom: RegexAtom,
    min: usize,
    max: Option<usize>,
}

/// Returns every non-overlapping match of `pattern` in `haystack`.
pub(crate) fn regex_matches(
    haystack: &[char],
    pattern: &[char],
    case_sensitive: bool,
) -> Vec<(usize, usize)> {
    let alternatives = parse_alternatives(pattern);
    if alternatives.iter().all(|steps| steps.is_empty()) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut index = 0usize;
    while index <= haystack.len() {
        let mut matched = None;
        for steps in &alternatives {
            if let Some(end) = match_steps(steps, haystack, index, index, 0, case_sensitive) {
                if end > index {
                    matched = Some(end);
                    break;
                }
            }
        }
        match matched {
            Some(end) => {
                out.push((index, end));
                index = end;
            }
            None => index += 1,
        }
    }
    out
}

fn parse_alternatives(pattern: &[char]) -> Vec<Vec<RegexStep>> {
    let mut alternatives: Vec<Vec<RegexStep>> = vec![Vec::new()];
    let mut index = 0usize;
    while index < pattern.len() {
        let ch = pattern[index];
        if ch == '|' {
            alternatives.push(Vec::new());
            index += 1;
            continue;
        }
        let atom = match ch {
            '\\' => {
                index += 1;
                match pattern.get(index).copied() {
                    Some('d') => Some(RegexAtom::Digit),
                    Some('w') => Some(RegexAtom::Word),
                    Some('s') => Some(RegexAtom::Space),
                    Some('b') => Some(RegexAtom::WordBoundary),
                    Some(other) => Some(RegexAtom::Literal(other)),
                    None => None,
                }
            }
            '.' => Some(RegexAtom::Any),
            '^' => Some(RegexAtom::LineStart),
            '$' => Some(RegexAtom::LineEnd),
            '[' => {
                let (atom, next) = parse_class(pattern, index);
                index = next.saturating_sub(1);
                Some(atom)
            }
            other => Some(RegexAtom::Literal(other)),
        };
        index += 1;
        if let Some(atom) = atom {
            let (min, max, next) = parse_quantifier(pattern, index);
            index = next;
            if let Some(steps) = alternatives.last_mut() {
                steps.push(RegexStep { atom, min, max });
            }
        }
    }
    alternatives
}

fn parse_class(pattern: &[char], open: usize) -> (RegexAtom, usize) {
    let mut index = open + 1;
    let negated = pattern.get(index) == Some(&'^');
    if negated {
        index += 1;
    }
    let mut ranges: Vec<(char, char)> = Vec::new();
    let mut first = true;
    while index < pattern.len() {
        let ch = pattern[index];
        if ch == ']' && !first {
            return (RegexAtom::Class { negated, ranges }, index + 1);
        }
        first = false;
        let low = if ch == '\\' {
            index += 1;
            match pattern.get(index).copied() {
                Some('d') => {
                    ranges.push(('0', '9'));
                    index += 1;
                    continue;
                }
                Some('w') => {
                    ranges.push(('a', 'z'));
                    ranges.push(('A', 'Z'));
                    ranges.push(('0', '9'));
                    ranges.push(('_', '_'));
                    index += 1;
                    continue;
                }
                Some('s') => {
                    ranges.push((' ', ' '));
                    ranges.push(('\t', '\t'));
                    index += 1;
                    continue;
                }
                Some(other) => other,
                None => break,
            }
        } else {
            ch
        };
        index += 1;
        if pattern.get(index) == Some(&'-') && pattern.get(index + 1) != Some(&']') {
            index += 1;
            let high = pattern.get(index).copied().unwrap_or(low);
            index += 1;
            ranges.push((low, high));
        } else {
            ranges.push((low, low));
        }
    }
    (RegexAtom::Class { negated, ranges }, index)
}

fn parse_quantifier(pattern: &[char], index: usize) -> (usize, Option<usize>, usize) {
    match pattern.get(index) {
        Some('*') => (0, None, index + 1),
        Some('+') => (1, None, index + 1),
        Some('?') => (0, Some(1), index + 1),
        Some('{') => {
            let mut end = index + 1;
            while end < pattern.len() && pattern[end] != '}' {
                end += 1;
            }
            if end >= pattern.len() {
                return (1, Some(1), index);
            }
            let body: String = pattern[index + 1..end].iter().collect();
            let mut parts = body.split(',');
            let min = parts.next().and_then(|part| part.trim().parse::<usize>().ok());
            let max = parts.next().and_then(|part| part.trim().parse::<usize>().ok());
            match min {
                Some(min) => (min, max.or(Some(min)), end + 1),
                None => (1, Some(1), index),
            }
        }
        _ => (1, Some(1), index),
    }
}

fn match_steps(
    steps: &[RegexStep],
    text: &[char],
    position: usize,
    start: usize,
    depth: usize,
    case_sensitive: bool,
) -> Option<usize> {
    // A pathological pattern must not recurse without bound.
    if depth > 256 {
        return None;
    }
    let Some(step) = steps.first() else { return Some(position) };
    let rest = &steps[1..];
    match &step.atom {
        RegexAtom::LineStart => {
            if position == start {
                match_steps(rest, text, position, start, depth + 1, case_sensitive)
            } else {
                None
            }
        }
        RegexAtom::LineEnd => {
            if position == text.len() {
                match_steps(rest, text, position, start, depth + 1, case_sensitive)
            } else {
                None
            }
        }
        RegexAtom::WordBoundary => {
            if is_word_boundary(text, position) {
                match_steps(rest, text, position, start, depth + 1, case_sensitive)
            } else {
                None
            }
        }
        _ => {
            let limit = step.max.unwrap_or(text.len() + 1);
            let mut positions = alloc::vec![position];
            let mut current = position;
            while positions.len() - 1 < limit {
                match match_atom(&step.atom, text, current, case_sensitive) {
                    Some(next) if next > current => {
                        positions.push(next);
                        current = next;
                    }
                    _ => break,
                }
            }
            if positions.len() - 1 < step.min {
                return None;
            }
            let mut take = positions.len() - 1;
            loop {
                if let Some(end) =
                    match_steps(rest, text, positions[take], start, depth + 1, case_sensitive)
                {
                    return Some(end);
                }
                if take == step.min {
                    return None;
                }
                take -= 1;
            }
        }
    }
}

fn match_atom(
    atom: &RegexAtom,
    text: &[char],
    position: usize,
    case_sensitive: bool,
) -> Option<usize> {
    let ch = *text.get(position)?;
    let matched = match atom {
        RegexAtom::Literal(want) => chars_eq(*want, ch, case_sensitive),
        RegexAtom::Any => true,
        RegexAtom::Digit => ch.is_ascii_digit(),
        RegexAtom::Word => ch.is_alphanumeric() || ch == '_',
        RegexAtom::Space => ch.is_whitespace(),
        RegexAtom::Class { negated, ranges } => {
            let inside = ranges.iter().any(|(low, high)| {
                let ch = if case_sensitive { ch } else { ch.to_ascii_lowercase() };
                let low = if case_sensitive { *low } else { low.to_ascii_lowercase() };
                let high = if case_sensitive { *high } else { high.to_ascii_lowercase() };
                ch >= low && ch <= high
            });
            inside != *negated
        }
        RegexAtom::WordBoundary | RegexAtom::LineStart | RegexAtom::LineEnd => false,
    };
    if matched {
        Some(position + 1)
    } else {
        None
    }
}

fn chars_eq(left: char, right: char, case_sensitive: bool) -> bool {
    if case_sensitive {
        left == right
    } else if left.is_ascii() && right.is_ascii() {
        left.eq_ignore_ascii_case(&right)
    } else {
        left.to_lowercase().eq(right.to_lowercase())
    }
}

/// Lowercases a character sequence once, preserving multi-character expansions.
fn lowercase_chars(chars: &[char]) -> Vec<char> {
    chars.iter().flat_map(|ch| ch.to_lowercase()).collect()
}

/// Compares one equal-length window against the needle, ignoring case.
///
/// The ASCII fast path avoids allocating while scanning the overwhelmingly
/// common source-text case; the general path compares against the needle that
/// was already lowercased by the caller.
fn window_matches_ignore_case(
    window: &[char],
    needle: &[char],
    needle_ascii: bool,
    needle_lower: &[char],
) -> bool {
    if needle_ascii && window.iter().all(|ch| ch.is_ascii()) {
        return window.iter().zip(needle).all(|(left, right)| left.eq_ignore_ascii_case(right));
    }
    window.iter().flat_map(|ch| ch.to_lowercase()).eq(needle_lower.iter().copied())
}

/// Every literal (non-regex) match in one line, as `(start_column, end_column)` char columns.
///
/// # Why the line is not collected into a `Vec<char>`
///
/// The scan used to materialise every line as a character vector so a window could be sliced
/// out of it. On a large file that was one allocation of `O(line)` per line per keystroke of
/// the query, on a path that already has to look at every line. The walk here is over the
/// line's bytes with a running character column: it allocates only the result.
///
/// The one case that keeps the materialised form is a **non-ASCII, case-insensitive**
/// needle, where Unicode lowercasing can expand one character into several and a 1:1 window
/// comparison would no longer be equivalent. That path is rare enough that matching the old
/// semantics is worth its cost.
fn scan_literal_line(
    line: &str,
    needle: &[char],
    needle_lower: &[char],
    needle_ascii: bool,
    options: SearchOptions,
    language: LanguageId,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    if !options.case_sensitive && !needle_ascii {
        let haystack: Vec<char> = line.chars().collect();
        if haystack.len() < needle.len() {
            return out;
        }
        let mut start = 0usize;
        while start + needle.len() <= haystack.len() {
            let window = &haystack[start..start + needle.len()];
            let equal = window_matches_ignore_case(window, needle, needle_ascii, needle_lower);
            if equal && whole_word_ok(&haystack, start, needle.len(), options, language) {
                out.push((start, start + needle.len()));
                start += needle.len();
            } else {
                start += 1;
            }
        }
        return out;
    }
    let mut column = 0usize;
    let mut byte = 0usize;
    while byte < line.len() {
        let mut matched = true;
        let mut cursor = byte;
        for wanted in needle {
            match line[cursor..].chars().next() {
                Some(ch) => {
                    let equal = if options.case_sensitive {
                        ch == *wanted
                    } else {
                        ch.eq_ignore_ascii_case(wanted)
                    };
                    if !equal {
                        matched = false;
                        break;
                    }
                    cursor += ch.len_utf8();
                }
                None => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            let before_ok =
                line[..byte].chars().next_back().is_none_or(|ch| !language.is_word_char(ch));
            let after_ok =
                line[cursor..].chars().next().is_none_or(|ch| !language.is_word_char(ch));
            if !options.whole_word || (before_ok && after_ok) {
                out.push((column, column + needle.len()));
                while byte < cursor {
                    let ch = line[byte..].chars().next().unwrap_or(' ');
                    byte += ch.len_utf8();
                    column += 1;
                }
                continue;
            }
        }
        let ch = line[byte..].chars().next().unwrap_or(' ');
        byte += ch.len_utf8();
        column += 1;
    }
    out
}

fn is_word_boundary(text: &[char], position: usize) -> bool {
    let before = position.checked_sub(1).and_then(|index| text.get(index)).copied();
    let after = text.get(position).copied();
    let is_word = |ch: Option<char>| ch.is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
    is_word(before) != is_word(after)
}
