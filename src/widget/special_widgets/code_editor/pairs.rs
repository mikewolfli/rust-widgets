// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Auto-pairing: bracket/quote completion, skip-over and smart backspace.
//!
//! The rules live here as pure functions so they are unit-testable without a
//! widget, a document or a render context. The editor decides *when* a typed
//! character reaches these helpers; the helpers decide *what* it should become.

/// What the editor should do with one typed character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairAction {
    /// Insert `open` and `close`, placing the caret between them. When a
    /// selection is active the selection is wrapped instead.
    Pair,
    /// The closing character already sits after the caret; step over it.
    SkipOver,
    /// Insert the character verbatim.
    Plain,
}

/// Returns the closing character paired with `open`, if any.
pub fn closing_for(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

/// Returns the opening character paired with `close`, if any.
pub fn opening_for(close: char) -> Option<char> {
    match close {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

/// Returns `true` for the bracket characters managed by the editor.
pub fn is_bracket(ch: char) -> bool {
    matches!(ch, '(' | ')' | '[' | ']' | '{' | '}')
}

/// Returns `true` for the quote characters managed by the editor.
pub fn is_quote(ch: char) -> bool {
    matches!(ch, '"' | '\'' | '`')
}

/// Returns `true` when `ch` is treated as an identifier character by the pair
/// heuristics (used to avoid auto-pairing inside words, e.g. `don't`).
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Plans the action for typing `typed` at a caret.
///
/// * `before` / `after` are the neighbouring characters around the caret.
/// * `has_selection` is `true` when text is selected (the pair wraps it).
pub fn plan_typing(
    typed: char,
    before: Option<char>,
    after: Option<char>,
    has_selection: bool,
) -> PairAction {
    if has_selection {
        // Wrapping a selection applies to brackets and quotes alike.
        return if closing_for(typed).is_some() { PairAction::Pair } else { PairAction::Plain };
    }

    if is_quote(typed) {
        if after == Some(typed) {
            return PairAction::SkipOver;
        }
        // Never auto-close a quote directly after an identifier character, or
        // contractions/possessives would sprout a stray partner.
        return if before.map(is_word_char).unwrap_or(false) {
            PairAction::Plain
        } else {
            PairAction::Pair
        };
    }

    if closing_for(typed).is_some() {
        return PairAction::Pair;
    }

    if let Some(open) = opening_for(typed) {
        if after == Some(typed) && before == Some(open) {
            // Only step over a closer that directly matches the opener we are
            // sitting inside, so `)]` in normal text is still inserted.
            return PairAction::SkipOver;
        }
        if after == Some(typed) {
            return PairAction::SkipOver;
        }
        return PairAction::Plain;
    }

    PairAction::Plain
}

/// Returns `true` when backspacing between `before` and `after` should remove
/// both characters because they form an empty pair.
pub fn should_delete_pair(before: Option<char>, after: Option<char>) -> bool {
    match (before, after) {
        (Some(open), Some(close)) => closing_for(open) == Some(close),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brackets_pair_and_close() {
        assert_eq!(closing_for('('), Some(')'));
        assert_eq!(closing_for('['), Some(']'));
        assert_eq!(closing_for('{'), Some('}'));
        assert_eq!(closing_for('x'), None);
        assert_eq!(opening_for('}'), Some('{'));
    }

    #[test]
    fn openers_insert_their_partner() {
        assert_eq!(plan_typing('(', None, None, false), PairAction::Pair);
        assert_eq!(plan_typing('{', Some(' '), Some(' '), false), PairAction::Pair);
    }

    #[test]
    fn closer_skips_over_existing_partner() {
        assert_eq!(plan_typing(')', Some('('), Some(')'), false), PairAction::SkipOver);
        assert_eq!(plan_typing(']', None, Some(']'), false), PairAction::SkipOver);
        // A lone closing bracket is typed normally.
        assert_eq!(plan_typing(')', Some('x'), Some('y'), false), PairAction::Plain);
    }

    #[test]
    fn quotes_pair_but_not_inside_words() {
        assert_eq!(plan_typing('"', None, None, false), PairAction::Pair);
        assert_eq!(plan_typing('\'', Some('t'), None, false), PairAction::Plain);
        assert_eq!(plan_typing('"', Some(' '), Some('"'), false), PairAction::SkipOver);
    }

    #[test]
    fn selection_wraps_for_brackets_and_quotes_but_not_letters() {
        assert_eq!(plan_typing('(', None, None, true), PairAction::Pair);
        assert_eq!(plan_typing('"', None, None, true), PairAction::Pair);
        assert_eq!(plan_typing('a', None, None, true), PairAction::Plain);
    }

    #[test]
    fn empty_pair_is_deleted_together() {
        assert!(should_delete_pair(Some('('), Some(')')));
        assert!(should_delete_pair(Some('"'), Some('"')));
        assert!(!should_delete_pair(Some('a'), Some('b')));
        assert!(!should_delete_pair(Some('('), None));
    }
}
