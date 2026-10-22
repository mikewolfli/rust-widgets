// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Lexer state chaining and token queries.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Tokenization ────────────────────────────────────────────────────────

    /// Runs one line through the active highlighter, returning spans and exit state.
    ///
    /// Resolution order is the whole point of the seam: a highlighter registered
    /// for the buffer's language wins, then the editor-wide override, then the
    /// built-in lexer for that language. The language is resolved per call rather
    /// than cached, because a buffer switch changes it and the cache invalidation
    /// that follows (`invalidate_line_states`) is what makes the switch visible.
    pub(super) fn highlight_with_state(
        &self,
        line: &str,
        state: LineState,
    ) -> (Vec<TokenSpan>, LineState) {
        let language = self.active_language();
        if let Some((_, highlighter)) =
            self.language_highlighters.iter().find(|(registered, _)| *registered == language)
        {
            return highlighter.highlight_line(line, state);
        }
        match &self.highlighter {
            Some(highlighter) => highlighter.highlight_line(line, state),
            None => BuiltinHighlighter::new(language).highlight_line(line, state),
        }
    }

    /// Returns the lexer state line `index` starts in.
    ///
    /// The cache is rebuilt lazily: `line_states` is grown to the current line
    /// count, and entries beyond the last one that is known to be accurate are
    /// re-derived by walking forward from that point. A line that both starts
    /// and ends [`LineState::Clean`] cannot influence the next line, so a clean
    /// run is filled in without running the lexer at all — which is what keeps
    /// this proportional to the edited region rather than to the document.
    ///
    /// The returned state for a line is only meaningful once the chain has
    /// reached it; [`CodeEditor::line_state`] reports the cached value, which is
    /// `Clean` for any line the walk has not needed yet.
    pub(super) fn ensure_line_states(&mut self, upto: usize) {
        let count = self.line_count();
        // The vector holds one entry per line plus a trailing sentinel carrying
        // the document's final state, so `line_states[line]` is always valid and
        // `line_states[line + 1]` is the state that line ends in.
        if self.line_states.len() != count + 1 {
            self.line_states.resize(count + 1, LineState::Clean);
            self.line_tokens.resize(count, None);
            self.walked_lines = 0;
        }
        let target = upto.min(count);
        // `walked_lines` is how far the entry-state chain has been *proven*.
        // Every entry below it came out of the highlighter, so it is trustworthy
        // even when non-clean; above it, entries only hold the `Clean` default
        // and prove nothing.
        let mut line = self.walked_lines.min(target);
        if line >= target {
            return;
        }
        let mut state = self.line_states[line];
        while line < target {
            let Some(text) = self.line_text(line) else { return };
            let (_, next) = self.highlight_with_state(&text, state);
            self.line_states[line + 1] = next;
            // This line's entry state is now proven and its exit state recorded.
            self.walked_lines = line + 1;
            if next == state && state.is_clean() {
                // Both ends are `Clean`, so this line cannot influence the next
                // one. Every remaining line in a pure-code region is therefore
                // also `Clean`, and the walk can stop: that is what keeps a
                // 10 000-line block comment from being re-lexed on every
                // keystroke while still walking cheaply through plain code.
                break;
            }
            state = next;
            line += 1;
        }
    }

    /// Drops every cached lexer state, forcing a full re-derivation.
    ///
    /// Used when the highlighter or the language changes: the entry-state chain
    /// is produced *by* the highlighter, so it cannot survive a swap.
    pub(crate) fn invalidate_line_states(&mut self) {
        // Any reason to drop the lexer state is also a reason the measured row layouts may be
        // stale: they were measured in the token runs this state produced.
        self.layout_lexer_generation = self.layout_lexer_generation.wrapping_add(1);
        for slot in self.line_tokens.iter_mut() {
            *slot = None;
        }
        for state in self.line_states.iter_mut() {
            *state = LineState::Clean;
        }
        self.walked_lines = 0;
        // The bracket cache is keyed by `(revision, caret)`, but the *classification* of a
        // position also depends on the lexer: a language change re-lexes the same text without
        // touching the revision, so a pair that was “code” can become “inside a string”. The
        // lexer state being thrown away is exactly the event that invalidates the cache.
        *self.bracket_cache.borrow_mut() = None;
    }

    /// Returns the token spans of a single line.
    ///
    /// Uses the installed highlighter when present, otherwise the built-in
    /// lexer. Spans from a third-party highlighter are clamped to `char`
    /// boundaries and to the line length, so drawing can never panic.
    ///
    /// Results are cached per line and keyed by the lexer state the line starts
    /// in, so a repaint of an unchanged document does no lexing at all.
    ///
    /// # What this does **not** guarantee
    ///
    /// The entry state used is whatever is cached, and for a line the paint path has not reached
    /// that is the `Clean` default — so a continuation line of a block comment or a multi-line
    /// string is segmented as if it were plain code. The answer therefore depends on scroll
    /// history for off-screen lines. When that matters, use
    /// [`CodeEditor::tokens_on_line_resolved`], which proves the entry state first.
    pub fn tokens_on_line(&self, line: usize) -> Vec<TokenSpan> {
        if let Some(cached) = self.line_tokens.get(line).and_then(|slot| slot.as_ref()) {
            return cached.clone();
        }
        let Some(text) = self.line_text(line) else { return Vec::new() };
        let state = self.line_states.get(line).copied().unwrap_or_default();
        let (raw, _) = self.highlight_with_state(&text, state);
        clamp_and_merge(&text, raw)
    }

    /// Returns the token spans of `line`, computing and caching them.
    ///
    /// The paint path holds `&self` and therefore cannot fill the cache; this
    /// `&mut` variant is what `draw` calls once per visible row so the cache is
    /// populated before the immutable readers run.
    pub(crate) fn fill_tokens_on_line(&mut self, line: usize) {
        if self.line_tokens.get(line).map(|slot| slot.is_some()).unwrap_or(false) {
            return;
        }
        let Some(text) = self.line_text(line) else { return };
        let state = self.line_states.get(line).copied().unwrap_or_default();
        let (raw, _) = self.highlight_with_state(&text, state);
        let merged = clamp_and_merge(&text, raw);
        if let Some(slot) = self.line_tokens.get_mut(line) {
            *slot = Some(merged);
        }
    }

    /// Returns the token colour for a span.
    pub fn token_color(&self, span: &TokenSpan) -> Color {
        self.palette.color_for(span.kind)
    }

    /// Returns the token kind at a character position, if styled.
    pub fn token_kind_at(&self, position: TextPosition) -> TokenKind {
        let Some(line) = self.line_text(position.line) else { return TokenKind::Plain };
        let Some(span) = self.tokens_on_line(position.line).into_iter().find(|span| {
            position.column >= char_len_of(&line[..span.start])
                && position.column < char_len_of(&line[..span.end])
        }) else {
            return TokenKind::Plain;
        };
        span.kind
    }
}
