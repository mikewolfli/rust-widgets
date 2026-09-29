// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Built-in tokenizer and the syntax-highlighting plugin seam.
//!
//! Real grammar-aware highlighting (tree-sitter, LSP semantic tokens) belongs
//! in a plugin crate. What lives here is a dependency-free lexer good enough to
//! power keyword/string/comment/number colouring, plus the [`SyntaxHighlighter`]
//! trait that a plugin implements to replace it wholesale.

use super::types::{TokenKind, TokenSpan};
use alloc::vec::Vec;

/// Lexer state carried across a line boundary.
///
/// Real syntax highlighting is **stateful**: a block comment, a multi-line
/// string (`"""`, `r#"…"#`) or a nested-delimiter run opened on one line
/// changes how the next line must be read. A highlighter that only ever sees
/// one line cannot know whether it starts inside a string, which is why the
/// seam has to carry this value in and out of every call.
///
/// The type is intentionally a small `enum` rather than an opaque token: it has
/// to be `Copy` so the line-state cache can store one per line, and it has to be
/// comparable so an edit can stop re-lexing once the state it produces matches
/// the state that was already recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum LineState {
    /// The line starts outside any construct; the next line does too.
    #[default]
    Clean,
    /// Inside a `/* … */` block comment.
    BlockComment,
    /// Inside a double-quoted multi-line string (`"""` for Python, `"` for
    /// a line-continued string in the C family).
    MultiLineString,
    /// Inside a single-quoted multi-line string (`'''` for Python).
    MultiLineCharString,
    /// Inside a Rust raw string, carrying the number of `#` delimiters.
    RawString {
        /// How many `#` characters delimit the raw string (`r#"` → 1).
        hashes: u8,
    },
    /// Inside a Go backtick raw string.
    RawStringTick,
}

impl LineState {
    /// Returns the state a freshly opened document starts in.
    pub fn initial() -> Self {
        Self::Clean
    }

    /// Returns `true` when this state is [`Self::Clean`].
    pub fn is_clean(self) -> bool {
        matches!(self, Self::Clean)
    }

    /// Returns `true` when the state does not end at a line boundary, i.e. the
    /// next line is a continuation. Used by the renderer to keep a continuation
    /// row visually distinct from a fresh one.
    pub fn continues(self) -> bool {
        !self.is_clean()
    }
}

/// Single-line lexer interface.
///
/// Implement this to plug a real grammar into the editor:
///
/// ```
/// use rust_widgets::widget::special_widgets::code_editor::{
///     LineState, SyntaxHighlighter, TokenKind, TokenSpan,
/// };
///
/// struct EverythingIsAComment;
///
/// impl SyntaxHighlighter for EverythingIsAComment {
///     fn highlight_line(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
///         let spans = if line.is_empty() {
///             Vec::new()
///         } else {
///             vec![TokenSpan::new(0, line.len(), TokenKind::Comment)]
///         };
///         (spans, state)
///     }
///
///     fn language_name(&self) -> &str {
///         "Comment Only"
///     }
/// }
/// ```
///
/// Implementors must be `Send + Sync` so a host may run the tokenizer on a
/// worker thread and hand the finished spans back to the UI thread. Without
/// that bound the editor can only ever re-highlight synchronously inside
/// `draw`, which is what makes a long file stutter no matter how fast the
/// lexer itself is.
pub trait SyntaxHighlighter: Send + Sync {
    /// Splits `line` into ordered, non-overlapping [`TokenSpan`]s, given the
    /// state the line starts in, and returns the state the line ends in.
    ///
    /// Spans that fall outside the line or split a UTF-8 sequence are clamped
    /// by the editor, so implementations cannot corrupt rendering by returning
    /// malformed offsets.
    fn highlight_line(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState);

    /// Human-readable language name used by the status bar and plugin registry.
    fn language_name(&self) -> &str {
        "Plain Text"
    }

    /// Returns `true` when `line` starts its own construct and the previous
    /// line's state can be ignored for correctness (not merely for speed).
    ///
    /// The default is the conservative `false`: a stateful highlighter always
    /// consumes the incoming state. Stateless highlighters may override this
    /// with `true` so the editor can skip re-lexing a region after an edit.
    fn resets_each_line(&self) -> bool {
        false
    }
}

/// Language families understood by the built-in lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum LanguageId {
    /// Rust source (`//`, `/* */`, `"`, `'`, `#[]`).
    Rust,
    /// Python source (`#`, `"""`, `'''`) — indentation-sensitive, no block comments.
    Python,
    /// JavaScript or TypeScript source.
    JavaScript,
    /// Go source, including backtick raw strings.
    Go,
    /// C-family source, including `#define` preprocessor lines.
    CLike,
    /// Markdown-ish plain text.
    Markdown,
    /// No highlighting at all.
    #[default]
    PlainText,
}

impl LanguageId {
    /// Returns the display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::Go => "Go",
            Self::CLike => "C",
            Self::Markdown => "Markdown",
            Self::PlainText => "Plain Text",
        }
    }

    /// Returns the number of spaces a tab expands to for this language.
    pub fn default_tab_width(self) -> usize {
        super::types::DEFAULT_TAB_WIDTH
    }

    /// Returns `true` when `#` starts a line comment instead of a preprocessor.
    pub fn uses_hash_comments(self) -> bool {
        matches!(self, Self::Python)
    }

    /// Returns `true` when `/* ... */` block comments are supported.
    pub fn supports_block_comments(self) -> bool {
        matches!(self, Self::Rust | Self::JavaScript | Self::Go | Self::CLike)
    }

    /// Returns `true` when `#`-prefixed preprocessor lines are supported.
    pub fn supports_preprocessor(self) -> bool {
        matches!(self, Self::Rust | Self::CLike)
    }

    /// Returns `true` when `ch` may appear inside an identifier.
    ///
    /// Also used for word-wise caret movement and whole-word search, so it is
    /// public: plugins can override the notion of "word" for their language.
    pub fn is_word_char(self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    /// Returns the keyword set of the language family.
    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
                "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super",
                "trait", "true", "type", "unsafe", "use", "where", "while", "union",
            ],
            Self::Python => &[
                "and", "as", "assert", "async", "await", "break", "class", "continue", "def",
                "del", "elif", "else", "except", "False", "finally", "for", "from", "global", "if",
                "import", "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise",
                "return", "True", "try", "while", "with", "yield",
            ],
            Self::JavaScript => &[
                "async",
                "await",
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "default",
                "delete",
                "do",
                "else",
                "export",
                "extends",
                "false",
                "finally",
                "for",
                "function",
                "if",
                "import",
                "in",
                "instanceof",
                "let",
                "new",
                "null",
                "of",
                "return",
                "static",
                "super",
                "switch",
                "this",
                "throw",
                "true",
                "try",
                "typeof",
                "undefined",
                "var",
                "void",
                "while",
                "yield",
            ],
            Self::Go => &[
                "break",
                "case",
                "chan",
                "const",
                "continue",
                "default",
                "defer",
                "else",
                "fallthrough",
                "for",
                "func",
                "go",
                "goto",
                "if",
                "import",
                "interface",
                "map",
                "package",
                "range",
                "return",
                "select",
                "struct",
                "switch",
                "type",
                "var",
            ],
            Self::CLike => &[
                "auto", "break", "case", "const", "continue", "default", "do", "else", "enum",
                "extern", "for", "goto", "if", "inline", "register", "restrict", "return",
                "sizeof", "static", "struct", "switch", "typedef", "union", "volatile", "while",
            ],
            Self::Markdown | Self::PlainText => &[],
        }
    }

    /// Returns the built-in type and primitive names of the language family.
    pub fn type_names(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str",
                "u8", "u16", "u32", "u64", "u128", "usize", "String", "Vec", "Option", "Result",
                "Box", "Rc", "Arc",
            ],
            Self::Python => {
                &["bool", "bytes", "dict", "float", "int", "list", "set", "str", "tuple"]
            }
            Self::JavaScript => {
                &["Array", "Boolean", "Map", "Number", "Object", "Promise", "Set", "String"]
            }
            Self::Go => {
                &["bool", "byte", "error", "float32", "float64", "int", "rune", "string", "uint"]
            }
            Self::CLike => &["char", "double", "float", "int", "long", "short", "void"],
            Self::Markdown | Self::PlainText => &[],
        }
    }

    /// Returns `true` when `#` opens a line comment for this language.
    pub fn comment_prefix(self) -> &'static str {
        if self.uses_hash_comments() {
            "# "
        } else {
            "// "
        }
    }

    /// Returns `true` when the language has a line-comment syntax at all.
    ///
    /// This is the `D2` predicate: a structure-aware command is *meaningless* for
    /// a language that has no such structure, and inserting `// ` into plain text
    /// or a Markdown heading is not a degraded toggle, it is corruption. Callers
    /// consult this and refuse rather than guess, so the failure is visible.
    pub fn supports_line_comments(self) -> bool {
        !matches!(self, Self::Markdown | Self::PlainText)
    }

    /// Returns `true` when bracket/quote auto-pairing is meaningful.
    ///
    /// Pairing is **not** degraded by language: it is already gated by
    /// [`super::types::CodeEditorConfig::auto_close_brackets`], which is the user's
    /// explicit choice, and inserting a matching delimiter is a formatting
    /// preference rather than a claim about grammar. The one command whose
    /// incorrect guess corrupts text is [`Self::line_comment_prefix`], which is why
    /// that one *is* degraded. This predicate exists so a host can still ask the
    /// question without the editor deciding for it.
    pub fn supports_auto_pairing(self) -> bool {
        !matches!(self, Self::Markdown | Self::PlainText)
    }

    /// Returns the line-comment prefix, or `None` when there is none.
    ///
    /// The `Option` is what makes the degradation explicit at the call site: a
    /// command that needs a comment token cannot silently fall back to a wrong
    /// one.
    pub fn line_comment_prefix(self) -> Option<&'static str> {
        if self.supports_line_comments() {
            Some(self.comment_prefix())
        } else {
            None
        }
    }
}

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
        if self.language.keywords().contains(&word) {
            TokenKind::Keyword
        } else if self.language.type_names().contains(&word) {
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
                LineState::BlockComment => {
                    let end = match line.find("*/") {
                        Some(offset) => offset + 2,
                        None => {
                            // The whole line is comment body; state is unchanged
                            // and the next line continues it.
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
                LineState::MultiLineString | LineState::MultiLineCharString => {
                    let quote = if state == LineState::MultiLineString { "\"\"\"" } else { "'''" };
                    let end = match line.find(quote) {
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
                    // A Rust raw string closes on `"` followed by `hashes` `#`.
                    let mut close = String::with_capacity(1 + hashes as usize);
                    close.push('"');
                    for _ in 0..hashes {
                        close.push('#');
                    }
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
            if !self.language.uses_hash_comments() && rest.starts_with("//") {
                spans.push(TokenSpan::new(index, len, TokenKind::Comment));
                break;
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
                match rest.find("*/") {
                    Some(offset) => {
                        let end = (index + offset + 2).min(len);
                        spans.push(TokenSpan::new(index, end, TokenKind::BlockComment));
                        index = end;
                        continue;
                    }
                    None => {
                        // Unterminated: the block comment continues on the next
                        // line, and saying so is the whole point of `LineState`.
                        spans.push(TokenSpan::new(index, len, TokenKind::BlockComment));
                        return (spans, LineState::BlockComment);
                    }
                }
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
                // chars. Go raw strings (backticks) and Python triple quotes are
                // scanned verbatim.
                let escape_aware = quote.len() == 1 && ch != '`';
                let found = if escape_aware {
                    let mut cursor = search_from;
                    let mut found = None;
                    while cursor < rest.len() {
                        if rest.as_bytes()[cursor] == b'\\' {
                            // Skip the backslash **and the escaped character**, whose byte length
                            // is not 1. Advancing by a literal two bytes assumed an ASCII escape,
                            // so highlighting a line such as `"a\é"` left `cursor` inside the `é`
                            // and the `rest[cursor..]` below panicked the whole frame.
                            cursor += 1;
                            cursor +=
                                rest[cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
                            continue;
                        }
                        if rest[cursor..].starts_with(quote) {
                            found = Some((index + cursor + quote.len()).min(len));
                            break;
                        }
                        cursor += rest[cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    }
                    found
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
                let kind = if quote.len() == 1 && ch == '\'' && !self.language.uses_hash_comments()
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

/// Coalesces neighbouring spans that share a token kind and are contiguous.
pub(crate) fn merge_adjacent(spans: Vec<TokenSpan>) -> Vec<TokenSpan> {
    let mut merged: Vec<TokenSpan> = Vec::with_capacity(spans.len());
    for span in spans {
        if span.is_empty() {
            continue;
        }
        match merged.last_mut() {
            Some(last) if last.kind == span.kind && last.end == span.start => last.end = span.end,
            _ => merged.push(span),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(highlighter: &BuiltinHighlighter, line: &str) -> Vec<TokenKind> {
        spans_of(highlighter, line).into_iter().map(|span| span.kind).collect()
    }

    /// Runs one line from a clean state and returns just the spans.
    ///
    /// The trait is stateful, so a test that only wants "what does this line look
    /// like on its own" must pass [`LineState::Clean`]; the multi-line tests in
    /// this module call `highlight_line` directly to thread a real state.
    fn spans_of(highlighter: &BuiltinHighlighter, line: &str) -> Vec<TokenSpan> {
        highlighter.highlight_line(line, LineState::Clean).0
    }

    #[test]
    fn rust_keywords_types_and_functions_are_distinguished() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = "fn main(x: u32) { }";
        let spans = spans_of(&highlighter, line);
        let tokens: Vec<(TokenKind, &str)> =
            spans.iter().map(|span| (span.kind, &line[span.start..span.end])).collect();
        assert!(tokens.contains(&(TokenKind::Keyword, "fn")));
        assert!(tokens.contains(&(TokenKind::Function, "main")));
        assert!(tokens.contains(&(TokenKind::Type, "u32")));
    }

    #[test]
    fn rust_block_and_line_comments_do_not_leak() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = "let a = 1; // fn not_keyword";
        let spans = spans_of(&highlighter, line);
        let comment = spans.last().expect("comment span");
        assert_eq!(comment.kind, TokenKind::Comment);
        assert_eq!(&line[comment.start..comment.end], "// fn not_keyword");

        let block = "/* let mut */ let b = 2;";
        let spans = spans_of(&highlighter, block);
        assert_eq!(spans[0].kind, TokenKind::BlockComment);
        assert_eq!(&block[spans[0].start..spans[0].end], "/* let mut */");
    }

    #[test]
    fn string_literals_with_escapes_and_quotes_terminate_correctly() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = r#"let s = "a\"b"; let c = 'x';"#;
        let spans = spans_of(&highlighter, line);
        let strings: Vec<&str> = spans
            .iter()
            .filter(|span| span.kind == TokenKind::String)
            .map(|span| &line[span.start..span.end])
            .collect();
        assert_eq!(strings.len(), 1, "escaped quote must not end the literal early");
        let chars: Vec<&str> = spans
            .iter()
            .filter(|span| span.kind == TokenKind::Character)
            .map(|span| &line[span.start..span.end])
            .collect();
        assert_eq!(chars, vec!["'x'"]);
    }

    #[test]
    fn python_uses_hash_comments_and_triple_quoted_strings() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Python);
        assert_eq!(kinds(&highlighter, "# note").first(), Some(&TokenKind::Comment));
        let line = "x = \"\"\"multi word\"\"\"";
        let spans = spans_of(&highlighter, line);
        let string = spans.iter().find(|span| span.kind == TokenKind::String).expect("string");
        assert_eq!(&line[string.start..string.end], "\"\"\"multi word\"\"\"");
    }

    #[test]
    fn go_raw_strings_and_rust_attributes_are_recognized() {
        let go = BuiltinHighlighter::new(LanguageId::Go);
        let raw = "s := `back \\ tick`";
        let spans = spans_of(&go, raw);
        let string = spans.iter().find(|span| span.kind == TokenKind::String).expect("raw string");
        assert_eq!(&raw[string.start..string.end], "`back \\ tick`");

        let rust = BuiltinHighlighter::new(LanguageId::Rust);
        let attribute = "#[derive(Debug)]";
        let spans = spans_of(&rust, attribute);
        assert_eq!(spans[0].kind, TokenKind::Preprocessor);
        assert_eq!(&attribute[spans[0].start..spans[0].end], "#[derive(Debug)]");
    }

    #[test]
    fn qualified_paths_are_not_mistaken_for_functions() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = "std::mem::size_of::<u32>()";
        let spans = spans_of(&highlighter, line);
        assert!(
            !spans.iter().any(|span| span.kind == TokenKind::Function),
            "qualified path segments must stay identifiers"
        );
    }

    #[test]
    fn spans_always_cover_the_line_in_order_without_overlap() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = r#"fn f() -> Vec<String> { let v = vec![1.5, 2]; } // done"#;
        let spans = spans_of(&highlighter, line);
        let mut previous_end = 0usize;
        for span in &spans {
            assert!(span.start >= previous_end, "spans must not overlap or regress");
            assert!(line.is_char_boundary(span.start));
            assert!(line.is_char_boundary(span.end));
            previous_end = span.end;
        }
        assert_eq!(spans.last().map(|span| span.end), Some(line.len()));
    }

    #[test]
    fn unicode_identifiers_do_not_split_characters() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = "let 变量 = \"值\";";
        let spans = spans_of(&highlighter, line);
        for span in &spans {
            assert!(line.is_char_boundary(span.start), "span start must be a char boundary");
            assert!(line.is_char_boundary(span.end), "span end must be a char boundary");
        }
    }

    #[test]
    fn plain_text_language_produces_no_keyword_tokens() {
        let highlighter = BuiltinHighlighter::new(LanguageId::PlainText);
        let spans = spans_of(&highlighter, "fn main() {}");
        assert!(!spans.iter().any(|span| span.kind == TokenKind::Keyword));
        assert_eq!(highlighter.language_name(), "Plain Text");
    }

    #[test]
    fn empty_line_produces_no_spans() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        assert!(spans_of(&highlighter, "").is_empty());
    }

    /// A backslash escape may protect a **multi-byte** character.
    ///
    /// The string scanner advanced `cursor += 2` on a backslash, assuming a one-byte escaped
    /// character. Highlighting a line such as `"a\é"` therefore left the cursor inside the `é`
    /// and the next `rest[cursor..]` panicked — on every frame that line was visible. The skip is
    /// now the backslash's own byte plus the escaped character's real UTF-8 length.
    #[test]
    fn an_escape_may_protect_a_multibyte_character() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        for line in ["\"a\\é\"", "\"x\\世\"", "\"\\🎉\"", "'\\é'"] {
            let spans = spans_of(&highlighter, line);
            for span in &spans {
                assert!(
                    line.is_char_boundary(span.start) && line.is_char_boundary(span.end),
                    "span {span:?} must sit on character boundaries of {line:?}"
                );
            }
        }
    }

    // ── Cross-line state (the whole reason `LineState` exists) ───────────────

    /// A block comment opened on one line must keep the **next** line commented.
    ///
    /// This is the defect the state-carrying seam fixes. With the old single-line
    /// signature the second line was lexed as fresh code, so `let x = 1;` inside a
    /// comment was coloured as a keyword and an identifier — the file *looked*
    /// broken even though the text was untouched.
    #[test]
    fn a_block_comment_keeps_the_next_line_commented() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let (first, state) = highlighter.highlight_line("/* start", LineState::Clean);
        assert_eq!(state, LineState::BlockComment, "the comment must stay open");
        assert_eq!(first[0].kind, TokenKind::BlockComment);

        let (second, state) = highlighter.highlight_line("let x = 1;", state);
        assert_eq!(state, LineState::BlockComment, "still no `*/` seen");
        assert!(
            second.iter().all(|span| span.kind == TokenKind::BlockComment),
            "every span of a comment body line is a comment, got {second:?}"
        );

        let (third, state) = highlighter.highlight_line("end */ fn main() {}", state);
        assert_eq!(state, LineState::Clean, "the comment closed here");
        let kinds: Vec<TokenKind> = third.iter().map(|span| span.kind).collect();
        assert!(kinds.contains(&TokenKind::BlockComment), "the lead-in is comment");
        assert!(kinds.contains(&TokenKind::Keyword), "code after `*/` is lexed again");
    }

    /// A Python triple-quoted string spans lines the same way.
    #[test]
    fn a_python_triple_quote_keeps_the_next_line_a_string() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Python);
        let (_, state) = highlighter.highlight_line("s = \"\"\"start", LineState::Clean);
        assert_eq!(state, LineState::MultiLineString);

        let (body, state) = highlighter.highlight_line("def not_code():", state);
        assert_eq!(state, LineState::MultiLineString);
        assert!(
            body.iter().all(|span| span.kind == TokenKind::String),
            "the body is string content, not code, got {body:?}"
        );

        let (_, state) = highlighter.highlight_line("end\"\"\"", state);
        assert_eq!(state, LineState::Clean);
    }

    /// A Rust raw string spans lines, and its `#` count must be honoured.
    ///
    /// `r#"…"#` closes on `"#`, not on a bare `"`. Losing the count would make
    /// the lexer end the string a `#` too early and colour the rest as code.
    #[test]
    fn a_rust_raw_string_spans_lines_and_keeps_its_hash_count() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let (first, state) = highlighter.highlight_line("let s = r#\"a", LineState::Clean);
        assert_eq!(state, LineState::RawString { hashes: 1 });
        assert!(first.iter().any(|span| span.kind == TokenKind::String));

        // A bare quote inside the raw string must NOT close it.
        let (body, state) = highlighter.highlight_line("let x = \" still raw", state);
        assert_eq!(state, LineState::RawString { hashes: 1 }, "a bare quote cannot close r#\"");
        assert!(body.iter().all(|span| span.kind == TokenKind::String));

        let (_, state) = highlighter.highlight_line("done\"#;", state);
        assert_eq!(state, LineState::Clean);
    }

    /// A single-quoted string in the C family does **not** spill across lines.
    ///
    /// `rust_raw_string_open` also has to not fire on an identifier that merely
    /// starts with `r`; `return` must lex as a keyword, not as the start of a raw
    /// string.
    #[test]
    fn a_bare_quote_does_not_open_a_multi_line_construct() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let (_, state) = highlighter.highlight_line("let c = 'a", LineState::Clean);
        assert_eq!(state, LineState::Clean, "an unterminated char is not a state change");

        let kinds = kinds(&highlighter, "return value;");
        assert!(kinds.contains(&TokenKind::Keyword), "`return` is a keyword: {kinds:?}");
        assert!(!kinds.contains(&TokenKind::String), "`return` opens no raw string");
    }

    /// An empty line inside a construct must not clear the state.
    #[test]
    fn an_empty_line_inside_a_comment_keeps_it_open() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let (_, state) = highlighter.highlight_line("/*", LineState::Clean);
        let (spans, state) = highlighter.highlight_line("", state);
        assert!(spans.is_empty());
        assert_eq!(state, LineState::BlockComment);
    }
}
