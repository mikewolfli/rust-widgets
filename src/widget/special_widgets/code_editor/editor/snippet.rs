// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Snippet expansion sessions.
//!
//! The minimal `insert_snippet` contract strips `$0…$9` and drops the caret on
//! the first marker. A host that expands a real template needs more: it needs to
//! know where the placeholders are, to move between them, and to stop navigating
//! when the user leaves the snippet. This module owns that session state and the
//! parser both entry points share, so there is one definition of what a marker
//! means.

use super::*;

/// One stop produced by the parser, in document order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParsedStop {
    /// Marker number, `0..=9`.
    pub(crate) index: u8,
    /// Character offset of the placeholder's first character.
    pub(crate) start: usize,
    /// Character offset just past the placeholder (equal to `start` for `$1`).
    pub(crate) end: usize,
}

/// The text and stops a snippet source resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedSnippet {
    /// The document text to insert, with every marker removed.
    pub(crate) text: String,
    /// Stops in document order, so `insert_snippet` can keep its "first marker
    /// in the text" caret rule.
    pub(crate) stops: Vec<ParsedStop>,
    /// Character offset of `$0`, when the snippet has one.
    pub(crate) final_caret: Option<usize>,
}

/// Parses `$0..$9` and `${N:placeholder}` markers.
///
/// A backslash escapes a literal `$` (`\$`). An unrecognised `${…}` sequence is
/// copied verbatim rather than interpreted, so a document that contains shell or
/// template syntax is not silently rewritten. A braced number outside `0..=9` is
/// copied verbatim too; an unbraced `$12` is the documented `$1` marker followed
/// by a literal `2`, which is how the one-digit contract spells. A missing closing
/// brace makes the whole sequence literal.
pub(crate) fn parse_snippet(snippet: &str) -> ParsedSnippet {
    let chars: Vec<char> = snippet.chars().collect();
    let mut text = String::with_capacity(snippet.len());
    let mut stops = Vec::new();
    let mut final_caret = None;
    let mut count = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && chars.get(i + 1) == Some(&'$') {
            text.push('$');
            count += 1;
            i += 2;
            continue;
        }
        if ch == '$' {
            if let Some(digit) = chars.get(i + 1).copied().filter(|ch| ch.is_ascii_digit()) {
                let index = digit as u8 - b'0';
                stops.push(ParsedStop { index, start: count, end: count });
                if index == 0 {
                    final_caret = Some(count);
                }
                i += 2;
                continue;
            }
            if chars.get(i + 1) == Some(&'{') {
                if let Some((index, next, placeholder)) = parse_braced(&chars, i) {
                    let start = count;
                    for ch in placeholder {
                        text.push(ch);
                        count += 1;
                    }
                    stops.push(ParsedStop { index, start, end: count });
                    if index == 0 {
                        final_caret = Some(count);
                    }
                    i = next;
                    continue;
                }
            }
        }
        text.push(ch);
        count += 1;
        i += 1;
    }
    ParsedSnippet { text, stops, final_caret }
}

/// Parses `${N}` / `${N:placeholder}` starting at `start` (`$`).
///
/// Returns `(number, index after the closing brace, placeholder characters)`.
fn parse_braced(chars: &[char], start: usize) -> Option<(u8, usize, Vec<char>)> {
    let mut i = start + 2;
    let mut number: u32 = 0;
    let mut digits = 0usize;
    while let Some(ch) = chars.get(i).copied().filter(|ch| ch.is_ascii_digit()) {
        number = number * 10 + (ch as u32 - '0' as u32);
        digits += 1;
        i += 1;
    }
    if digits == 0 || number > 9 {
        return None;
    }
    let index = number as u8;
    match chars.get(i).copied() {
        Some('}') => Some((index, i + 1, Vec::new())),
        Some(':') => {
            i += 1;
            let mut placeholder = Vec::new();
            while let Some(ch) = chars.get(i).copied() {
                if ch == '\\' {
                    let next = chars.get(i + 1).copied()?;
                    placeholder.push(next);
                    i += 2;
                    continue;
                }
                if ch == '}' {
                    return Some((index, i + 1, placeholder));
                }
                placeholder.push(ch);
                i += 1;
            }
            None
        }
        _ => None,
    }
}

/// Returns the position `chars` characters into `text`, starting at `origin`.
pub(crate) fn advance_position(origin: TextPosition, text: &str, chars: usize) -> TextPosition {
    let mut line = origin.line;
    let mut column = origin.column;
    for ch in text.chars().take(chars) {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    TextPosition::new(line, column)
}

/// Builds the session stops from a parsed snippet inserted at `origin`.
///
/// Navigation order is `$1…$9` and then `$0`; within one number, document order
/// is preserved so a repeated marker is visited left to right.
fn session_stops(parsed: &ParsedSnippet, origin: TextPosition) -> Vec<SnippetStop> {
    let mut stops: Vec<SnippetStop> = parsed
        .stops
        .iter()
        .map(|stop| SnippetStop {
            index: stop.index,
            start: advance_position(origin, &parsed.text, stop.start),
            end: advance_position(origin, &parsed.text, stop.end),
        })
        .collect();
    stops.sort_by_key(|stop| (stop.index == 0, stop.index, stop.start));
    stops
}

/// Maps one position through an edit that replaced `[start, old_end)` with text
/// ending at `new_end`.
///
/// Positions before the edit do not move; positions after it shift by the net
/// line/column delta. A position strictly inside the replaced range has no
/// meaningful new location, which is what tells the caller to end the session.
fn translate_position(
    position: TextPosition,
    start: TextPosition,
    old_end: TextPosition,
    new_end: TextPosition,
) -> Option<TextPosition> {
    if position <= start {
        return Some(position);
    }
    if position >= old_end {
        let line_delta = new_end.line as isize - old_end.line as isize;
        let column_delta = if position.line == old_end.line && new_end.line == old_end.line {
            new_end.column as isize - old_end.column as isize
        } else {
            0
        };
        let line = (position.line as isize + line_delta).max(0) as usize;
        let column = (position.column as isize + column_delta).max(0) as usize;
        return Some(TextPosition::new(line, column));
    }
    None
}

impl CodeEditor {
    /// Inserts `snippet` and starts a placeholder session over its tab stops.
    ///
    /// Supports `$0…$9` (a bare caret stop), `${N:placeholder}` (the placeholder
    /// text is inserted and selected, so typing replaces it) and `\$` for a
    /// literal dollar sign. Returns `None` when the editor is read-only or the
    /// snippet is empty; a snippet with no markers still inserts its text and
    /// returns an empty session, so the caller can tell "inserted, nothing to
    /// navigate" from "refused".
    ///
    /// The session is ended by [`Self::cancel_snippet_session`], by
    /// [`Self::next_placeholder`] past the last stop, and by any edit the position
    /// translation cannot describe.
    pub fn insert_snippet_session(&mut self, snippet: &str) -> Option<SnippetSession> {
        if !self.is_editable() || snippet.is_empty() {
            return None;
        }
        // Expand tabs *before* parsing so the character offsets the parser
        // records are the offsets the document receives. `insert` expands tabs,
        // and expanding an already-expanded string is a no-op.
        let expanded = self.expand_tabs(snippet);
        let parsed = parse_snippet(&expanded);
        if parsed.text.is_empty() && parsed.stops.is_empty() {
            return None;
        }
        let origin = self.cursor.bounds().0;
        self.insert(&parsed.text);
        let stops = session_stops(&parsed, origin);
        let session = SnippetSession::new(stops);
        self.snippet_session = Some(session.clone());
        if !session.is_empty() {
            let _ = self.apply_snippet_stop(0);
        }
        self.base.request_redraw();
        Some(session)
    }

    /// Whether a snippet session is running.
    pub fn is_in_snippet(&self) -> bool {
        self.snippet_session.is_some()
    }

    /// Moves the caret to the next tab stop.
    ///
    /// Returns `false` — and ends the session — when the caret is already on the
    /// last stop, which is the final `$0`. The text is never rolled back: leaving
    /// a snippet navigates, it does not cancel the edit.
    pub fn next_placeholder(&mut self) -> bool {
        let next = self
            .snippet_session
            .as_mut()
            .and_then(|session| session.advance().then_some(session.current));
        match next {
            Some(index) => self.apply_snippet_stop(index),
            None => {
                if self.snippet_session.take().is_some() {
                    self.base.request_redraw();
                }
                false
            }
        }
    }

    /// Moves the caret to the previous tab stop, returning `false` on the first.
    pub fn previous_placeholder(&mut self) -> bool {
        let previous = self
            .snippet_session
            .as_mut()
            .and_then(|session| session.retreat().then_some(session.current));
        match previous {
            Some(index) => self.apply_snippet_stop(index),
            None => false,
        }
    }

    /// Ends the session without touching the inserted text.
    ///
    /// Returns whether a session was running. The caret stays where it is, which
    /// is the difference between cancelling navigation and undoing the insertion.
    pub fn cancel_snippet_session(&mut self) -> bool {
        let running = self.snippet_session.take().is_some();
        if running {
            self.base.request_redraw();
        }
        running
    }

    /// Places the caret on one stop without changing which stop is current.
    pub(crate) fn apply_snippet_stop(&mut self, index: usize) -> bool {
        let Some(stop) = self.snippet_session.as_ref().and_then(|session| session.stops.get(index))
        else {
            return false;
        };
        let start = self.clamp_position(stop.start);
        let end = self.clamp_position(stop.end);
        if start == end {
            self.move_caret_to(start, false);
        } else {
            self.extra_cursors.clear();
            self.cursor = Cursor { head: end, anchor: start };
            self.goal.active = false;
            self.ensure_caret_visible();
            self.emit_cursor_and_selection();
            self.base.request_redraw();
        }
        true
    }

    /// Moves the session's stops through a committed edit.
    ///
    /// `edit` is `(start, old_end, new_end)` in pre/post-edit coordinates. `None`
    /// means the edit path could not describe the change (a whole-document
    /// replacement, an undo), which ends the session rather than guessing.
    pub(crate) fn translate_snippet_session(
        &mut self,
        edit: Option<(TextPosition, TextPosition, TextPosition)>,
    ) {
        let Some(mut session) = self.snippet_session.clone() else {
            return;
        };
        let Some((start, old_end, new_end)) = edit else {
            self.snippet_session = None;
            return;
        };
        let mut translated = Vec::with_capacity(session.stops.len());
        for stop in &session.stops {
            let (Some(stop_start), Some(stop_end)) = (
                translate_position(stop.start, start, old_end, new_end),
                translate_position(stop.end, start, old_end, new_end),
            ) else {
                self.snippet_session = None;
                return;
            };
            translated.push(SnippetStop { index: stop.index, start: stop_start, end: stop_end });
        }
        session.stops = translated;
        self.snippet_session = Some(session);
    }
}

#[cfg(test)]
mod tests {
    use super::{advance_position, parse_snippet};
    use crate::widget::special_widgets::code_editor::types::TextPosition;

    #[test]
    fn markers_are_stripped_and_recorded_in_order() {
        let parsed = parse_snippet("fn $1(a: ${2:T}) -> $0");
        assert_eq!(parsed.text, "fn (a: T) -> ");
        assert_eq!(parsed.stops.len(), 3);
        assert_eq!(parsed.final_caret, Some(parsed.text.chars().count()));
        assert_eq!(parsed.stops[0].index, 1);
        assert_eq!(parsed.stops[0].start, 3);
        assert_eq!(parsed.stops[0].end, 3);
        assert_eq!(parsed.stops[1].index, 2);
        assert_eq!(parsed.stops[1].end - parsed.stops[1].start, 1, "the placeholder is selected");
    }

    #[test]
    fn a_missing_brace_is_copied_verbatim() {
        let parsed = parse_snippet("${not a marker} $12 $");
        assert_eq!(parsed.text, "${not a marker} 2 $");
        assert_eq!(parsed.stops.len(), 1);
        assert_eq!(parsed.stops[0].index, 1, "$12 is $1 followed by a literal 2");
    }

    #[test]
    fn escaped_dollars_stay_literal() {
        let parsed = parse_snippet("\\$1 costs $1");
        assert_eq!(parsed.text, "$1 costs ");
        assert_eq!(parsed.stops.len(), 1);
    }

    #[test]
    fn a_char_offset_advances_across_lines() {
        let origin = TextPosition::new(2, 5);
        assert_eq!(advance_position(origin, "ab\ncd", 5), TextPosition::new(3, 2));
        assert_eq!(advance_position(origin, "ab", 1), TextPosition::new(2, 6));
    }
}
