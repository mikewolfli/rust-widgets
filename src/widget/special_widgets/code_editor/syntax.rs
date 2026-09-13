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

/// Single-line lexer interface.
///
/// Implement this to plug a real grammar into the editor:
///
/// ```
/// use rust_widgets::widget::special_widgets::code_editor::{
///     SyntaxHighlighter, TokenKind, TokenSpan,
/// };
///
/// struct EverythingIsAComment;
///
/// impl SyntaxHighlighter for EverythingIsAComment {
///     fn highlight_line(&self, line: &str) -> Vec<TokenSpan> {
///         if line.is_empty() {
///             Vec::new()
///         } else {
///             vec![TokenSpan::new(0, line.len(), TokenKind::Comment)]
///         }
///     }
///
///     fn language_name(&self) -> &str {
///         "Comment Only"
///     }
/// }
/// ```
pub trait SyntaxHighlighter {
    /// Splits `line` into ordered, non-overlapping [`TokenSpan`]s.
    ///
    /// Spans that fall outside the line or split a UTF-8 sequence are clamped
    /// by the editor, so implementations cannot corrupt rendering by returning
    /// malformed offsets.
    fn highlight_line(&self, line: &str) -> Vec<TokenSpan>;

    /// Human-readable language name used by the status bar and plugin registry.
    fn language_name(&self) -> &str {
        "Plain Text"
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
    fn highlight_line(&self, line: &str) -> Vec<TokenSpan> {
        let len = line.len();
        let mut spans: Vec<TokenSpan> = Vec::new();
        let mut index = 0usize;

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
            if self.language.supports_block_comments() && rest.starts_with("/*") {
                let end = rest.find("*/").map(|offset| index + offset + 2).unwrap_or(len).min(len);
                spans.push(TokenSpan::new(index, end, TokenKind::BlockComment));
                index = end;
                continue;
            }

            // String and character literals, including Python triple quotes and
            // Go raw strings.
            if ch == '"' || ch == '\'' || (self.language == LanguageId::Go && ch == '`') {
                let quote = if self.language == LanguageId::Python && rest.starts_with("\"\"\"") {
                    "\"\"\""
                } else if self.language == LanguageId::Python && rest.starts_with("'''") {
                    "'''"
                } else {
                    &rest[..ch.len_utf8()]
                };
                let search_from = quote.len();
                // Backslash escapes protect the delimiter in normal strings and
                // chars. Go raw strings (backticks) and Python triple quotes are
                // scanned verbatim.
                let escape_aware = quote.len() == 1 && ch != '`';
                let end = if escape_aware {
                    let bytes = rest.as_bytes();
                    let mut cursor = search_from;
                    let mut found = len;
                    while cursor < rest.len() {
                        if bytes[cursor] == b'\\' {
                            cursor += 2;
                            continue;
                        }
                        if rest[cursor..].starts_with(quote) {
                            found = (index + cursor + quote.len()).min(len);
                            break;
                        }
                        cursor += rest[cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    }
                    found
                } else {
                    let remainder = &rest[search_from.min(rest.len())..];
                    remainder
                        .find(quote)
                        .map(|offset| index + search_from + offset + quote.len())
                        .unwrap_or(len)
                        .min(len)
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

        merge_adjacent(spans)
    }

    fn language_name(&self) -> &str {
        self.language.name()
    }
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
        highlighter.highlight_line(line).into_iter().map(|span| span.kind).collect()
    }

    #[test]
    fn rust_keywords_types_and_functions_are_distinguished() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = "fn main(x: u32) { }";
        let spans = highlighter.highlight_line(line);
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
        let spans = highlighter.highlight_line(line);
        let comment = spans.last().expect("comment span");
        assert_eq!(comment.kind, TokenKind::Comment);
        assert_eq!(&line[comment.start..comment.end], "// fn not_keyword");

        let block = "/* let mut */ let b = 2;";
        let spans = highlighter.highlight_line(block);
        assert_eq!(spans[0].kind, TokenKind::BlockComment);
        assert_eq!(&block[spans[0].start..spans[0].end], "/* let mut */");
    }

    #[test]
    fn string_literals_with_escapes_and_quotes_terminate_correctly() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = r#"let s = "a\"b"; let c = 'x';"#;
        let spans = highlighter.highlight_line(line);
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
        let spans = highlighter.highlight_line(line);
        let string = spans.iter().find(|span| span.kind == TokenKind::String).expect("string");
        assert_eq!(&line[string.start..string.end], "\"\"\"multi word\"\"\"");
    }

    #[test]
    fn go_raw_strings_and_rust_attributes_are_recognized() {
        let go = BuiltinHighlighter::new(LanguageId::Go);
        let raw = "s := `back \\ tick`";
        let spans = go.highlight_line(raw);
        let string = spans.iter().find(|span| span.kind == TokenKind::String).expect("raw string");
        assert_eq!(&raw[string.start..string.end], "`back \\ tick`");

        let rust = BuiltinHighlighter::new(LanguageId::Rust);
        let attribute = "#[derive(Debug)]";
        let spans = rust.highlight_line(attribute);
        assert_eq!(spans[0].kind, TokenKind::Preprocessor);
        assert_eq!(&attribute[spans[0].start..spans[0].end], "#[derive(Debug)]");
    }

    #[test]
    fn qualified_paths_are_not_mistaken_for_functions() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = "std::mem::size_of::<u32>()";
        let spans = highlighter.highlight_line(line);
        assert!(
            !spans.iter().any(|span| span.kind == TokenKind::Function),
            "qualified path segments must stay identifiers"
        );
    }

    #[test]
    fn spans_always_cover_the_line_in_order_without_overlap() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        let line = r#"fn f() -> Vec<String> { let v = vec![1.5, 2]; } // done"#;
        let spans = highlighter.highlight_line(line);
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
        let spans = highlighter.highlight_line(line);
        for span in &spans {
            assert!(line.is_char_boundary(span.start), "span start must be a char boundary");
            assert!(line.is_char_boundary(span.end), "span end must be a char boundary");
        }
    }

    #[test]
    fn plain_text_language_produces_no_keyword_tokens() {
        let highlighter = BuiltinHighlighter::new(LanguageId::PlainText);
        let spans = highlighter.highlight_line("fn main() {}");
        assert!(!spans.iter().any(|span| span.kind == TokenKind::Keyword));
        assert_eq!(highlighter.language_name(), "Plain Text");
    }

    #[test]
    fn empty_line_produces_no_spans() {
        let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
        assert!(highlighter.highlight_line("").is_empty());
    }
}
