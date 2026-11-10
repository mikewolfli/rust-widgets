// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The real built-in lexer (`builtin-languages`).
//!
//! A lexer, not a parser: it classifies keywords, types, strings, chars,
//! numbers, comments, operators and call-site functions for every
//! [`LanguageId`] family. Grammar-aware highlighting remains a plugin concern.

use super::super::{
    merge_adjacent, LanguageId, LineState, SyntaxHighlighter, TokenKind, TokenSpan,
};
use alloc::string::String;
use alloc::vec::Vec;
/// Keyword/identifier-aware lexer covering the [`LanguageId`] families.
///
/// This is a lexer, not a parser: it classifies keywords, types, strings,
/// chars, numbers, comments, operators, and call-site functions. Grammar-aware
/// highlighting is a plugin concern — see the module docs.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuiltinHighlighter {
    language: LanguageId,
}

impl BuiltinHighlighter {
    /// Creates a highlighter for `language`.
    pub fn new(language: LanguageId) -> Self {
        Self { language }
    }

    /// Returns the language this highlighter tokenizes.
    pub fn language(&self) -> LanguageId {
        self.language
    }

    fn word_token_kind(&self, word: &str, followed_by_call: bool) -> TokenKind {
        // Both tables are kept in byte order (a unit test pins that), so classification is a
        // binary search instead of a linear `contains` over every keyword — which ran once
        // per identifier on every painted line (J-P07).
        if self.language.keywords().binary_search(&word).is_ok() {
            TokenKind::Keyword
        } else if self.language.type_names().binary_search(&word).is_ok() {
            TokenKind::Type
        } else if followed_by_call {
            TokenKind::Function
        } else {
            TokenKind::Identifier
        }
    }
}

impl SyntaxHighlighter for BuiltinHighlighter {
    fn highlight_line(&self, line: &str, mut state: LineState) -> (Vec<TokenSpan>, LineState) {
        let len = line.len();
        let mut spans: Vec<TokenSpan> = Vec::new();
        let mut index = 0usize;

        // ── Continuation: the line starts inside a construct opened above. ──
        //
        // This is the half the old single-line signature could not express. A
        // block comment, a Python triple-quoted string and a Rust raw string are
        // all able to span any number of lines, so the first thing every line
        // must do is finish whatever the line above left open.
        if !state.is_clean() {
            match state {
                LineState::BlockComment | LineState::NestedBlockComment { .. } => {
                    let depth = match state {
                        LineState::NestedBlockComment { depth } => depth.max(2),
                        _ => 1,
                    };
                    let (scan_end, remaining) = scan_block_comment(
                        line,
                        depth,
                        matches!(self.language, LanguageId::Rust | LanguageId::Swift),
                    );
                    if scan_end > 0 {
                        spans.push(TokenSpan::new(0, scan_end, TokenKind::BlockComment));
                    }
                    index = scan_end;
                    if remaining > 0 {
                        return (spans, block_comment_state(remaining));
                    }
                    state = LineState::Clean;
                }
                LineState::MultiLineString | LineState::MultiLineCharString => {
                    let quote = if state == LineState::MultiLineString { "\"\"\"" } else { "'''" };
                    // Python processes backslash escapes inside a triple-quoted string,
                    // so `\"\"\"` does not close it. Scanning verbatim ended the string
                    // on an escaped delimiter and coloured the rest of the line as code.
                    let end = match find_unescaped(line, 0, quote) {
                        Some(offset) => offset + quote.len(),
                        None => {
                            if len > 0 {
                                spans.push(TokenSpan::new(0, len, TokenKind::String));
                            }
                            return (spans, state);
                        }
                    };
                    spans.push(TokenSpan::new(0, end, TokenKind::String));
                    index = end;
                    state = LineState::Clean;
                }
                LineState::RawStringTick => {
                    let end = match line.find('`') {
                        Some(offset) => offset + 1,
                        None => {
                            if len > 0 {
                                spans.push(TokenSpan::new(0, len, TokenKind::String));
                            }
                            return (spans, state);
                        }
                    };
                    spans.push(TokenSpan::new(0, end, TokenKind::String));
                    index = end;
                    state = LineState::Clean;
                }
                LineState::RawString { hashes } => {
                    // A Rust raw string closes on `"` followed by `hashes` `#`. The closer is built
                    // by the same helper the opening path uses, so the two cannot drift (ED6-02).
                    let close = raw_string_closer(hashes);
                    let end = match line.find(&close) {
                        Some(offset) => offset + close.len(),
                        None => {
                            if len > 0 {
                                spans.push(TokenSpan::new(0, len, TokenKind::String));
                            }
                            return (spans, state);
                        }
                    };
                    spans.push(TokenSpan::new(0, end, TokenKind::String));
                    index = end;
                    state = LineState::Clean;
                }
                LineState::HtmlComment => {
                    let end = match line.find("-->") {
                        Some(offset) => offset + 3,
                        None => {
                            if len > 0 {
                                spans.push(TokenSpan::new(0, len, TokenKind::BlockComment));
                            }
                            return (spans, state);
                        }
                    };
                    spans.push(TokenSpan::new(0, end, TokenKind::BlockComment));
                    index = end;
                    state = LineState::Clean;
                }
                LineState::Clean => {}
            }
        }

        while index < len {
            let rest = &line[index..];
            let ch = match rest.chars().next() {
                Some(ch) => ch,
                None => break,
            };

            // `#` starts a comment (Python) or a preprocessor line (Rust/C).
            if ch == '#' {
                if self.language.uses_hash_comments() {
                    spans.push(TokenSpan::new(index, len, TokenKind::Comment));
                    break;
                }
                if self.language.supports_preprocessor() {
                    let end = if self.language == LanguageId::Rust {
                        rest.find(']').map(|offset| index + offset + 1).unwrap_or(len)
                    } else {
                        len
                    };
                    let end = end.min(len);
                    spans.push(TokenSpan::new(index, end, TokenKind::Preprocessor));
                    index = end;
                    continue;
                }
            }
            if self.language.supports_slash_comments() && rest.starts_with("//") {
                spans.push(TokenSpan::new(index, len, TokenKind::Comment));
                break;
            }
            if self.language.supports_dash_comments() && rest.starts_with("--") {
                spans.push(TokenSpan::new(index, len, TokenKind::Comment));
                break;
            }
            if self.language.supports_html_comments() && rest.starts_with("<!--") {
                match rest.find("-->") {
                    Some(offset) => {
                        let end = (index + offset + 3).min(len);
                        spans.push(TokenSpan::new(index, end, TokenKind::BlockComment));
                        index = end;
                        continue;
                    }
                    None => {
                        spans.push(TokenSpan::new(index, len, TokenKind::BlockComment));
                        return (spans, LineState::HtmlComment);
                    }
                }
            }
            // A Rust raw string: `r"…"`, `r#"…"#`, `r##"…"##`. Colours the
            // whole literal, and only spills across lines when the closing
            // delimiter is absent — which is exactly what the old signature
            // could not represent.
            if self.language == LanguageId::Rust {
                if let Some((open_len, hashes)) = rust_raw_string_open(rest) {
                    let close = raw_string_closer(hashes);
                    let end = match rest[open_len..].find(&close) {
                        Some(offset) => index + open_len + offset + close.len(),
                        None => {
                            spans.push(TokenSpan::new(index, len, TokenKind::String));
                            return (spans, LineState::RawString { hashes });
                        }
                    };
                    spans.push(TokenSpan::new(index, end.min(len), TokenKind::String));
                    index = end.min(len);
                    continue;
                }
            }
            if self.language.supports_block_comments() && rest.starts_with("/*") {
                let nestable = matches!(self.language, LanguageId::Rust | LanguageId::Swift);
                // The opener itself is already consumed: scanning `rest` from byte 0 would
                // count it as a second, nested opener and never reach depth 0.
                let (scanned, remaining) = scan_block_comment(&rest[2..], 1, nestable);
                let end = (index + 2 + scanned).min(len);
                spans.push(TokenSpan::new(index, end, TokenKind::BlockComment));
                if remaining > 0 {
                    // Unterminated: the block comment continues on the next line, and
                    // saying so is the whole point of `LineState`. A Rust/Swift nested
                    // comment carries how many levels are still open so a later `*/`
                    // cannot close it early.
                    return (spans, block_comment_state(remaining));
                }
                index = end;
                continue;
            }

            // Zig's multiline string literal is a run of lines whose first
            // non-whitespace token is `\\`. Every such line is independent, so this
            // needs no carried `LineState`: colour the remainder of the line and keep
            // lexing the next line from `Clean`.
            if self.language == LanguageId::Zig
                && rest.starts_with("\\\\")
                && line[..index].trim().is_empty()
            {
                spans.push(TokenSpan::new(index, len, TokenKind::String));
                break;
            }

            // A Zig quoted identifier (`@"main"`, `@"type"`) is one identifier, not an
            // `@` followed by a string literal.
            if self.language == LanguageId::Zig && rest.starts_with("@\"") {
                let end = rest[2..]
                    .find('"')
                    .map(|offset| (index + 2 + offset + 1).min(len))
                    .unwrap_or(len);
                spans.push(TokenSpan::new(index, end, TokenKind::Identifier));
                index = end;
                continue;
            }

            // String and character literals, including Python triple quotes and
            // Go raw strings.
            if ch == '"' || ch == '\'' || (self.language == LanguageId::Go && ch == '`') {
                let triple_double =
                    self.language == LanguageId::Python && rest.starts_with("\"\"\"");
                let triple_single = self.language == LanguageId::Python && rest.starts_with("'''");
                let quote = if triple_double {
                    "\"\"\""
                } else if triple_single {
                    "'''"
                } else {
                    &rest[..ch.len_utf8()]
                };
                let search_from = quote.len();
                // Backslash escapes protect the delimiter in normal strings and
                // chars, and Python applies them inside triple quotes too. Go raw
                // strings (backticks) and shell single-quoted strings scan verbatim.
                let escape_aware = ch != '`' && !(self.language == LanguageId::Shell && ch == '\'');
                let found = if escape_aware {
                    find_unescaped(rest, search_from, quote)
                        .map(|offset| index + offset + quote.len())
                        .map(|end| end.min(len))
                } else {
                    rest[search_from.min(rest.len())..]
                        .find(quote)
                        .map(|offset| (index + search_from + offset + quote.len()).min(len))
                };
                let end = match found {
                    Some(end) => end,
                    None => {
                        // Unterminated. A multi-line-capable literal spills into
                        // the next line; a plain one is simply unterminated and
                        // the editor must not claim a state change it cannot undo.
                        let spill = if triple_double {
                            Some(LineState::MultiLineString)
                        } else if triple_single {
                            Some(LineState::MultiLineCharString)
                        } else if ch == '`' {
                            Some(LineState::RawStringTick)
                        } else {
                            None
                        };
                        spans.push(TokenSpan::new(index, len, TokenKind::String));
                        return (spans, spill.unwrap_or(LineState::Clean));
                    }
                };
                let kind = if quote.len() == 1 && ch == '\'' && self.language.single_quote_is_char()
                {
                    TokenKind::Character
                } else {
                    TokenKind::String
                };
                spans.push(TokenSpan::new(index, end, kind));
                index = end;
                continue;
            }

            // Numeric literals, including `_` separators and type suffixes.
            if ch.is_ascii_digit() {
                let mut end = index + ch.len_utf8();
                while end < len {
                    let next = line[end..].chars().next().unwrap_or(' ');
                    if next.is_ascii_alphanumeric() || next == '.' || next == '_' {
                        end += next.len_utf8();
                    } else {
                        break;
                    }
                }
                spans.push(TokenSpan::new(index, end, TokenKind::Number));
                index = end;
                continue;
            }

            // Identifiers, with heuristics for call sites and qualified paths.
            if ch.is_alphabetic() || ch == '_' {
                let mut end = index + ch.len_utf8();
                while end < len {
                    let next = line[end..].chars().next().unwrap_or(' ');
                    if self.language.is_word_char(next) {
                        end += next.len_utf8();
                    } else {
                        break;
                    }
                }
                let word = &line[index..end];
                let followed_by_call = line[end..].trim_start().starts_with('(');
                let after_qualifier =
                    line[..index].trim_end().ends_with('.') || line[..index].ends_with("::");
                let kind = if after_qualifier {
                    TokenKind::Identifier
                } else {
                    self.word_token_kind(word, followed_by_call)
                };
                spans.push(TokenSpan::new(index, end, kind));
                index = end;
                continue;
            }

            // Whitespace carries no style.
            if ch.is_whitespace() {
                index += ch.len_utf8();
                continue;
            }

            if is_operator_char(ch) {
                let mut end = index + ch.len_utf8();
                while end < len {
                    let next = line[end..].chars().next().unwrap_or(' ');
                    if is_operator_char(next) {
                        end += next.len_utf8();
                    } else {
                        break;
                    }
                }
                spans.push(TokenSpan::new(index, end, TokenKind::Operator));
                index = end;
                continue;
            }

            // Brackets and any remaining scalar are plain text.
            spans.push(TokenSpan::new(index, index + ch.len_utf8(), TokenKind::Plain));
            index += ch.len_utf8();
        }

        (merge_adjacent(spans), state)
    }

    fn language_name(&self) -> &str {
        self.language.name()
    }
}

/// The line state for a block comment with `depth` open levels remaining.
fn block_comment_state(depth: u32) -> LineState {
    if depth > 1 {
        LineState::NestedBlockComment { depth }
    } else {
        LineState::BlockComment
    }
}

/// Scans a block comment body and returns `(bytes consumed, open levels left)`.
///
/// `depth` is the number of `/*` already open when the scan starts. Nested
/// comments are only recognised for languages that actually nest them (Rust and
/// Swift): for a C-family language the first `*/` still closes the comment, which
/// is the same rule the compiler applies.
fn scan_block_comment(text: &str, depth: u32, nestable: bool) -> (usize, u32) {
    let mut depth = depth.max(1);
    let mut cursor = 0usize;
    while cursor < text.len() {
        let rest = &text[cursor..];
        if nestable && rest.starts_with("/*") {
            depth += 1;
            cursor += 2;
            continue;
        }
        if rest.starts_with("*/") {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                return (cursor, 0);
            }
            continue;
        }
        cursor += rest.chars().next().map(|ch| ch.len_utf8()).unwrap_or(1);
    }
    (text.len(), depth)
}

/// Byte offset of the first unescaped occurrence of `quote` at or after `from`.
///
/// A backslash escapes the **next character**, which may be multi-byte, so the
/// cursor advances by that character's length rather than by one byte. This is
/// the one scanner both the continuation arm and the opening arm use, so a
/// Python triple-quoted string and a plain string cannot disagree about where
/// their delimiter is.
fn find_unescaped(text: &str, from: usize, quote: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut cursor = from.min(text.len());
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor += 1;
            cursor += text[cursor..].chars().next().map(|ch| ch.len_utf8()).unwrap_or(0);
            continue;
        }
        if text[cursor..].starts_with(quote) {
            return Some(cursor);
        }
        cursor += text[cursor..].chars().next().map(|ch| ch.len_utf8()).unwrap_or(1);
    }
    None
}

/// Returns the opening length and `#` count of a Rust raw string at `text`'s start.
///
/// Recognises `r"`, `r#"`, `r##"` … An identifier that merely begins with `r`
/// (for example `return`) does not match, because the character after the `r`
/// must be `#` or the quote itself.
fn rust_raw_string_open(text: &str) -> Option<(usize, u8)> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'r') {
        return None;
    }
    let mut hashes = 0u8;
    let mut cursor = 1usize;
    while bytes.get(cursor) == Some(&b'#') {
        hashes = hashes.saturating_add(1);
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'"') {
        Some((cursor + 1, hashes))
    } else {
        None
    }
}

/// Builds the closing delimiter for a Rust raw string with `hashes` `#`.
fn raw_string_closer(hashes: u8) -> String {
    let mut close = String::with_capacity(1 + hashes as usize);
    close.push('"');
    for _ in 0..hashes {
        close.push('#');
    }
    close
}

/// Returns `true` for characters that form operator runs.
fn is_operator_char(ch: char) -> bool {
    matches!(
        ch,
        '+' | '-'
            | '*'
            | '/'
            | '%'
            | '='
            | '<'
            | '>'
            | '!'
            | '&'
            | '|'
            | '^'
            | '~'
            | '?'
            | ':'
            | ';'
            | ','
            | '.'
    )
}
