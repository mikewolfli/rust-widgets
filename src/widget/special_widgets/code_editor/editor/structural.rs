// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Structural selection and the caret navigation stack.
//!
//! Selection expansion has two sources. A grammar-aware
//! [`SyntaxHighlighter`] can answer [`SyntaxHighlighter::selection_ranges`] with
//! exact ranges; when it does not (the built-in lexer has no structural model,
//! and its default implementation returns nothing) the editor scans the text for
//! words, lines, bracket pairs and paragraphs. The fallback is deliberate rather
//! than a degraded mode: it makes the command usable with no plugin installed.

use super::super::syntax::StructuralKind;
use super::*;

impl CodeEditor {
    /// Expands the selection to the next structural unit of `kind`.
    ///
    /// The command is *progressive*: each call selects the smallest candidate that
    /// strictly contains the current selection, so repeated calls walk outwards
    /// (word → bracket pair → enclosing block). Returns `false` when there is no
    /// larger unit, which is the honest answer at the document root.
    ///
    /// A registered [`SyntaxHighlighter`]'s
    /// [`selection_ranges`](SyntaxHighlighter::selection_ranges) list is consulted
    /// first; the textual scan runs as well, so a partial grammar answer does not
    /// disable the fallback.
    pub fn expand_selection(&mut self, kind: StructuralKind) -> bool {
        let head = self.cursor.head;
        let (selection_start, selection_end) =
            if self.has_selection() { self.cursor.bounds() } else { (head, head) };
        let mut candidates: Vec<(TextPosition, TextPosition)> = Vec::new();
        let text = self.text();
        if let Some(highlighter) = self.active_selection_highlighter() {
            candidates.extend(
                highlighter
                    .selection_ranges(&text, head, kind)
                    .into_iter()
                    .filter(|(start, end)| start < end),
            );
        }
        candidates.extend(self.textual_structural_ranges(kind, head));
        candidates.retain(|(start, end)| *start <= head && head <= *end && start < end);
        candidates.sort_by_key(|(start, end)| {
            (end.line - start.line, end.column.saturating_sub(start.column))
        });
        candidates.dedup();
        let chosen = candidates.into_iter().find(|(start, end)| {
            *start <= selection_start
                && *end >= selection_end
                && (*start < selection_start || *end > selection_end)
        });
        let Some((start, end)) = chosen else {
            return false;
        };
        self.remember_structural_selection();
        self.apply_structural_selection(start, end);
        true
    }

    /// Restores the selection the last [`Self::expand_selection`] replaced (RW-114).
    ///
    /// Repeated calls walk back out of the expansions in reverse order, ending at the original
    /// caret. The history is cleared by every document change (see
    /// [`Self::structural_selection_history`]), so a stale position is never restored. Returns
    /// `false` when there is nothing to undo, leaving the selection untouched.
    pub fn shrink_selection(&mut self) -> bool {
        let Some((anchor, head)) = self.structural_selection_history.pop() else {
            return false;
        };
        self.extra_cursors.clear();
        self.cursor =
            Cursor { head: self.clamp_position(head), anchor: self.clamp_position(anchor) };
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Records the current selection before an expansion replaces it.
    ///
    /// A repeated identical push is skipped, so re-running the command after a refused expansion
    /// does not fill the stack with copies of one selection.
    fn remember_structural_selection(&mut self) {
        let selection = (self.cursor.anchor, self.cursor.head);
        if self.structural_selection_history.last().copied() == Some(selection) {
            return;
        }
        self.structural_selection_history.push(selection);
        if self.structural_selection_history.len() > MAX_STRUCTURAL_SELECTION_HISTORY {
            let excess = self.structural_selection_history.len() - MAX_STRUCTURAL_SELECTION_HISTORY;
            self.structural_selection_history.drain(0..excess);
        }
    }

    /// Records the caret as a navigation position.
    ///
    /// Consecutive calls from the same caret are one entry, so a host may call
    /// this unconditionally before every navigation command. Pushing after a
    /// [`Self::go_back`] discards the forward branch, which is what makes the
    /// stack a history rather than a log.
    pub fn push_navigation_position(&mut self) -> bool {
        let current = self.cursor.head;
        if self.navigation_history.len() == self.navigation_index + 1
            && self.navigation_history.get(self.navigation_index) == Some(&current)
        {
            return false;
        }
        self.navigation_history.truncate(self.navigation_index + 1);
        self.navigation_history.push(current);
        if self.navigation_history.len() > MAX_NAVIGATION_HISTORY {
            let excess = self.navigation_history.len() - MAX_NAVIGATION_HISTORY;
            self.navigation_history.drain(0..excess);
        }
        self.navigation_index = self.navigation_history.len().saturating_sub(1);
        true
    }

    /// Moves the caret to the previous navigation position.
    pub fn go_back(&mut self) -> bool {
        if self.navigation_index == 0 {
            return false;
        }
        self.navigation_index -= 1;
        let position = self.navigation_history[self.navigation_index];
        self.apply_navigation_position(position);
        true
    }

    /// Moves the caret to the next navigation position.
    pub fn go_forward(&mut self) -> bool {
        if self.navigation_index + 1 >= self.navigation_history.len() {
            return false;
        }
        self.navigation_index += 1;
        let position = self.navigation_history[self.navigation_index];
        self.apply_navigation_position(position);
        true
    }

    /// How many entries the navigation history currently holds.
    pub fn navigation_history_len(&self) -> usize {
        self.navigation_history.len()
    }

    /// The active highlighter for `kind` queries, if one is registered.
    fn active_selection_highlighter(&self) -> Option<&dyn SyntaxHighlighter> {
        let language = self.active_language();
        if let Some((_, highlighter)) =
            self.language_highlighters.iter().find(|(registered, _)| *registered == language)
        {
            return Some(highlighter.as_ref());
        }
        self.highlighter.as_deref()
    }

    /// The textual fallback ranges for `kind`, smallest first.
    fn textual_structural_ranges(
        &self,
        kind: StructuralKind,
        head: TextPosition,
    ) -> Vec<(TextPosition, TextPosition)> {
        match kind {
            StructuralKind::Word => {
                let (start, end) = self.word_bounds(head);
                if start < end {
                    vec![(start, end)]
                } else {
                    Vec::new()
                }
            }
            StructuralKind::Line => vec![self.line_structural_range(head.line)],
            StructuralKind::Bracket => self.enclosing_pairs(head, false),
            StructuralKind::Block => {
                let mut ranges = self.enclosing_pairs(head, true);
                if ranges.is_empty() {
                    if let Some(paragraph) = self.paragraph_range(head) {
                        ranges.push(paragraph);
                    }
                }
                ranges
            }
        }
    }

    /// The whole-line range a `Line` expansion selects.
    ///
    /// It includes the terminator when one follows, matching [`Self::select_line`],
    /// so repeated line operations do not leave the newline behind.
    fn line_structural_range(&self, line: usize) -> (TextPosition, TextPosition) {
        let last = self.line_count().saturating_sub(1);
        let line = line.min(last);
        let end = if line < last {
            TextPosition::new(line + 1, 0)
        } else {
            TextPosition::new(line, self.line_len(line))
        };
        (TextPosition::new(line, 0), end)
    }

    /// Every bracket pair enclosing `head`, innermost first.
    ///
    /// The scan runs once over the document with a stack, so a nested pair is
    /// found even when several later pairs are closed on the same line. `braces`
    /// restricts the result to `{ … }`, which is what `Block` means.
    ///
    /// Brackets inside a string or a comment are text, not structure: the lexer has
    /// already classified those spans, so they are skipped exactly as bracket matching
    /// skips them. Without this, `expand_selection(Block)` on a line holding `"{"`
    /// selected from that quote to the next brace in the file.
    fn enclosing_pairs(
        &self,
        head: TextPosition,
        braces: bool,
    ) -> Vec<(TextPosition, TextPosition)> {
        let text = self.text();
        let head_byte = self.offset_of(head);
        let mut stack: Vec<(usize, char)> = Vec::new();
        let mut pairs: Vec<(TextPosition, TextPosition)> = Vec::new();
        let mut line = 0usize;
        let mut line_start = 0usize;
        let mut skip = self.skipped_spans(0);
        for (byte, ch) in text.char_indices() {
            if ch == '\n' {
                line += 1;
                line_start = byte + 1;
                skip = self.skipped_spans(line);
                continue;
            }
            if is_skipped(byte.saturating_sub(line_start), &skip) {
                continue;
            }
            if is_opening_bracket(ch) {
                stack.push((byte, ch));
                continue;
            }
            if !is_closing_bracket(ch) {
                continue;
            }
            let Some(index) = stack.iter().rposition(|(_, open)| brackets_match(*open, ch)) else {
                continue;
            };
            let (open_byte, open) = stack[index];
            // Discard the matched opener together with any unmatched inner ones.
            stack.truncate(index);
            if braces && open != '{' {
                continue;
            }
            let close_end = byte + ch.len_utf8();
            if open_byte <= head_byte && head_byte <= close_end {
                pairs
                    .push((position_in_text(&text, open_byte), position_in_text(&text, close_end)));
            }
        }
        pairs
    }

    /// The blank-line-delimited paragraph around `head`.
    fn paragraph_range(&self, head: TextPosition) -> Option<(TextPosition, TextPosition)> {
        let last = self.line_count().saturating_sub(1);
        let line = head.line.min(last);
        if self.line_text(line).is_none_or(|text| text.trim().is_empty()) {
            return None;
        }
        let mut first = line;
        while first > 0 && self.line_text(first - 1).is_some_and(|text| !text.trim().is_empty()) {
            first -= 1;
        }
        let mut last_line = line;
        while last_line < last
            && self.line_text(last_line + 1).is_some_and(|text| !text.trim().is_empty())
        {
            last_line += 1;
        }
        let end = if last_line < last {
            TextPosition::new(last_line + 1, 0)
        } else {
            TextPosition::new(last_line, self.line_len(last_line))
        };
        Some((TextPosition::new(first, 0), end))
    }

    fn apply_structural_selection(&mut self, start: TextPosition, end: TextPosition) {
        self.extra_cursors.clear();
        self.cursor = Cursor { head: self.clamp_position(end), anchor: self.clamp_position(start) };
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    fn apply_navigation_position(&mut self, position: TextPosition) {
        self.extra_cursors.clear();
        self.move_caret_to(position, false);
    }
}

/// Upper bound on the navigation history.
///
/// The bound is a memory guarantee, not a policy: a host that navigates a million
/// times keeps the most recent positions rather than growing without limit.
pub const MAX_NAVIGATION_HISTORY: usize = 128;

/// Upper bound on the structural-selection history (RW-114).
///
/// An expansion stack is at most the document's nesting depth; the bound only keeps a pathological
/// document from growing the editor's memory without limit.
pub const MAX_STRUCTURAL_SELECTION_HISTORY: usize = 64;

fn is_opening_bracket(ch: char) -> bool {
    matches!(ch, '(' | '[' | '{')
}

fn is_closing_bracket(ch: char) -> bool {
    matches!(ch, ')' | ']' | '}')
}

fn brackets_match(open: char, close: char) -> bool {
    matches!((open, close), ('(', ')') | ('[', ']') | ('{', '}'))
}
