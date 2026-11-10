// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Behavioural tests for the built-in lexer and the cross-line state machine.

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

/// Every language the editor advertises must classify its own comments and
/// strings, which is the defect the seven-language set had: Java/C++ fell into
/// the C family and SQL/HTML had no rule at all.
#[test]
fn the_common_languages_have_their_own_comment_and_string_rules() {
    let java = BuiltinHighlighter::new(LanguageId::Java);
    let spans = kinds(&java, "int x = 1; // note");
    assert_eq!(spans.last(), Some(&TokenKind::Comment));

    let cpp = BuiltinHighlighter::new(LanguageId::Cpp);
    let spans = kinds(&cpp, "/* note */ auto x = 1;");
    assert_eq!(spans.first(), Some(&TokenKind::BlockComment));

    let sql = BuiltinHighlighter::new(LanguageId::Sql);
    let spans = kinds(&sql, "select 'x' -- note");
    assert_eq!(spans.last(), Some(&TokenKind::Comment));
    assert!(spans.contains(&TokenKind::String), "single quotes are strings in SQL");

    let yaml = BuiltinHighlighter::new(LanguageId::Yaml);
    assert_eq!(kinds(&yaml, "key: value # note").last(), Some(&TokenKind::Comment));

    let json = BuiltinHighlighter::new(LanguageId::Json);
    assert!(!kinds(&json, "{\"a\": true}").contains(&TokenKind::Comment));
}

/// An HTML comment spans lines and must keep the following line commented.
#[test]
fn an_html_comment_spans_lines() {
    let highlighter = BuiltinHighlighter::new(LanguageId::Html);
    let (_, state) = highlighter.highlight_line("<!-- start", LineState::Clean);
    assert_eq!(state, LineState::HtmlComment);
    let (spans, state) = highlighter.highlight_line("still a comment -->", state);
    assert_eq!(state, LineState::Clean);
    assert!(spans.iter().all(|span| span.kind == TokenKind::BlockComment));
}

/// Single quotes in JavaScript are strings, not character literals.
#[test]
fn javascript_single_quotes_are_strings() {
    let highlighter = BuiltinHighlighter::new(LanguageId::JavaScript);
    let spans = kinds(&highlighter, "const a = 'x';");
    assert!(spans.contains(&TokenKind::String));
    assert!(!spans.contains(&TokenKind::Character));
}

/// Zig uses `//` comments, has no block comment, treats `'a'` as a character and
/// recognises both `\\` multiline string lines and `@"…"` quoted identifiers.
#[test]
fn zig_comments_literals_and_multiline_strings_are_recognized() {
    let highlighter = BuiltinHighlighter::new(LanguageId::Zig);
    assert_eq!(kinds(&highlighter, "// note").first(), Some(&TokenKind::Comment));
    assert!(
        !kinds(&highlighter, "/* not a block comment */").contains(&TokenKind::BlockComment),
        "Zig has no block comments"
    );

    let literal = "const c = 'x'; const s = \"text\";";
    let spans = spans_of(&highlighter, literal);
    assert!(spans.iter().any(|span| span.kind == TokenKind::Character));
    assert!(spans.iter().any(|span| span.kind == TokenKind::String));

    let multiline = "    \\\\already inside a multiline string";
    let spans = spans_of(&highlighter, multiline);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, TokenKind::String);

    let quoted = "@\"type\"";
    let spans = spans_of(&highlighter, quoted);
    assert_eq!(spans[0].kind, TokenKind::Identifier);
    assert_eq!(&quoted[spans[0].start..spans[0].end], "@\"type\"");
}

/// The keyword and type tables are binary-searched, so every language's list must be
/// in byte order. This pins the invariant that makes the lookup valid — without it a
/// new keyword appended out of order would silently stop highlighting (J-P07).
#[test]
fn keyword_tables_are_sorted() {
    const LANGUAGES: &[LanguageId] = &[
        LanguageId::Rust,
        LanguageId::Python,
        LanguageId::JavaScript,
        LanguageId::Go,
        LanguageId::Zig,
        LanguageId::CLike,
        LanguageId::Java,
        LanguageId::CSharp,
        LanguageId::Cpp,
        LanguageId::Kotlin,
        LanguageId::Swift,
        LanguageId::Html,
        LanguageId::Xml,
        LanguageId::Css,
        LanguageId::Json,
        LanguageId::Yaml,
        LanguageId::Toml,
        LanguageId::Shell,
        LanguageId::Sql,
        LanguageId::Markdown,
        LanguageId::PlainText,
    ];
    for language in LANGUAGES {
        let keywords = language.keywords();
        assert!(
            keywords.windows(2).all(|pair| pair[0] <= pair[1]),
            "{:?} keywords are not sorted",
            language
        );
        let types = language.type_names();
        assert!(
            types.windows(2).all(|pair| pair[0] <= pair[1]),
            "{:?} type names are not sorted",
            language
        );
    }
}
