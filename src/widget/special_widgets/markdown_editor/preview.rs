// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Markdown preview: block/inline parsing, styling and row drawing.

use super::*;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::special_widgets::code_editor::{
    BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter, TokenKind,
};

/// One run of preview text with the style the inline parser resolved for it.
pub(crate) struct PreviewRun {
    pub(crate) text: String,
    pub(crate) style: PreviewStyle,
    /// The measured advance, resolved when the line was parsed so a still frame measures nothing.
    pub(crate) width: u32,
}

/// The inline style of one preview run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewStyle {
    /// Ordinary body text.
    Text,
    /// `**strong**`: drawn with a one-pixel overprint, like every bold run in this crate.
    Bold,
    /// `*emphasis*`: the face has no italic, so emphasis is a slight ink shift.
    Italic,
    /// `~~struck~~`: drawn faint with a rule through it.
    Strike,
    /// `` `code` ``: tinted backing.
    Code,
    /// `[label](target)`: accent ink with an underline.
    Link,
    /// Syntax that was recognised and is not shown as body text (fences, list markers).
    Dim,
    /// A list bullet, task box or quote bar.
    Accent,
    /// One token of a highlighted fenced block.
    Token(TokenKind),
}

/// How a preview row is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewRow {
    /// Runs drawn from left to right.
    Inline,
    /// A heading, carrying its level.
    Heading(u8),
    /// A horizontal rule.
    Rule,
    /// An opening or closing code fence.
    Fence,
    /// The body of a fenced code block.
    Code,
    /// A table row (the runs already contain the cell separators).
    Table,
}

/// A parsed preview row, cached by line index and text hash.
pub(crate) struct ParsedLine {
    pub(crate) runs: Vec<PreviewRun>,
    pub(crate) row: PreviewRow,
}

/// Where a line sits relative to a fenced code block.
#[derive(Clone, Copy)]
pub(crate) enum BlockStart {
    /// Outside any fence.
    Normal,
    /// The opening or closing fence line itself.
    Fence,
    /// Inside a fence, with the lexer state the line starts in.
    Code(LanguageId, LineState),
}

/// A stable hash of one preview line, used as the cache key beside its index.
fn text_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// The cache key of one parsed preview row.
///
/// A key is the row's text plus every input the parse reads: its block context (a fenced-code
/// line is tokenised differently from prose), the GFM and highlighting flags, whether a table
/// divider follows, and the font size its runs were measured at. The row's position is not an
/// input to the parse, so it is not part of the key.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PreviewKey {
    /// Hash of the row's source text.
    text: u64,
    /// Hash of the block context [`parse_preview_line`] is given.
    block: u64,
    /// `gfm | highlight << 1 | table_follows << 2`.
    flags: u8,
    /// The measurement font size, as raw bits.
    font_size_bits: u32,
}

impl PreviewKey {
    /// The key for `line` parsed with `block` under the editor's current options.
    pub(crate) fn new(
        line: &str,
        block: BlockStart,
        gfm: bool,
        highlight: bool,
        table_follows: bool,
        font: &Font,
    ) -> Self {
        Self {
            text: text_hash(line),
            block: block_cache_key(block),
            flags: u8::from(gfm) | (u8::from(highlight) << 1) | (u8::from(table_follows) << 2),
            font_size_bits: font.size().to_bits(),
        }
    }
}

/// A stable hash of the block context a preview row is parsed in.
fn block_cache_key(block: BlockStart) -> u64 {
    use core::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match block {
        BlockStart::Normal => 0u8.hash(&mut hasher),
        BlockStart::Fence => 1u8.hash(&mut hasher),
        BlockStart::Code(language, state) => {
            2u8.hash(&mut hasher);
            language.hash(&mut hasher);
            state.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// The info string of a fenced code block opener, if `trimmed` opens one.
pub(crate) fn fence_marker(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("```").or_else(|| trimmed.strip_prefix("~~~"))?;
    Some(rest.trim())
}

/// Maps a fence's info string onto one of the shared lexer's languages.
pub(crate) fn language_from_info(info: &str) -> LanguageId {
    let token = info.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    match token.as_str() {
        "rust" | "rs" => LanguageId::Rust,
        "python" | "py" => LanguageId::Python,
        "javascript" | "js" | "typescript" | "ts" | "tsx" | "jsx" => LanguageId::JavaScript,
        "go" | "golang" => LanguageId::Go,
        "c" | "h" | "cpp" | "c++" | "cc" | "hpp" | "java" | "cs" | "csharp" | "kt" => {
            LanguageId::CLike
        }
        "markdown" | "md" => LanguageId::Markdown,
        _ => LanguageId::PlainText,
    }
}

/// Builds one measured run.
fn push_run(
    runs: &mut Vec<PreviewRun>,
    text: &str,
    style: PreviewStyle,
    font: &Font,
    context: &RenderContext,
) {
    if text.is_empty() {
        return;
    }
    runs.push(PreviewRun {
        text: text.to_string(),
        style,
        width: context.measure_text(text, font).width,
    });
}

/// Parses one preview row.
///
/// This is where the GFM block constructs are recognised: fenced code (through the per-revision
/// fence table), headings, bullets and ordered items, task boxes, quotes, tables, rules, images
/// and the inline emphasis/code/link/delete run styles. Everything the previous preview did not
/// know about arrived as literal source text.
pub(crate) fn parse_preview_line(
    line: &str,
    block: BlockStart,
    gfm: bool,
    highlight: bool,
    table_follows: bool,
    font: &Font,
    context: &RenderContext,
) -> ParsedLine {
    let mut runs = Vec::new();
    match block {
        BlockStart::Fence => {
            push_run(&mut runs, line.trim(), PreviewStyle::Dim, font, context);
            return ParsedLine { runs, row: PreviewRow::Fence };
        }
        BlockStart::Code(language, state) => {
            if highlight {
                let highlighter = BuiltinHighlighter::new(language);
                let (spans, _) = highlighter.highlight_line(line, state);
                if spans.is_empty() {
                    push_run(&mut runs, line, PreviewStyle::Token(TokenKind::Plain), font, context);
                } else {
                    let mut cursor = 0usize;
                    for span in spans {
                        let start = span.start.min(line.len()).max(cursor);
                        let end = span.end.min(line.len()).max(start);
                        if start > cursor {
                            push_run(
                                &mut runs,
                                &line[cursor..start],
                                PreviewStyle::Token(TokenKind::Plain),
                                font,
                                context,
                            );
                        }
                        if end > start {
                            push_run(
                                &mut runs,
                                &line[start..end],
                                PreviewStyle::Token(span.kind),
                                font,
                                context,
                            );
                        }
                        cursor = end;
                    }
                    if cursor < line.len() {
                        push_run(
                            &mut runs,
                            &line[cursor..],
                            PreviewStyle::Token(TokenKind::Plain),
                            font,
                            context,
                        );
                    }
                }
            } else {
                push_run(&mut runs, line, PreviewStyle::Text, font, context);
            }
            return ParsedLine { runs, row: PreviewRow::Code };
        }
        BlockStart::Normal => {}
    }

    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    // Heading: the leading `#` markers are syntax, so they are consumed rather than rendered.
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if hashes > 0 && hashes <= 6 && trimmed[hashes..].starts_with(' ') {
        let level = hashes as u8;
        let heading_font = font.with_size(font.size() + (7 - hashes) as f32);
        let body = trimmed[hashes..].trim_start();
        parse_inline(body, gfm, &heading_font, context, &mut runs);
        return ParsedLine { runs, row: PreviewRow::Heading(level) };
    }

    if gfm && is_rule(trimmed) {
        return ParsedLine { runs, row: PreviewRow::Rule };
    }
    if gfm && is_table_divider(trimmed) {
        return ParsedLine { runs, row: PreviewRow::Rule };
    }

    let mut body = trimmed;
    let mut prefix_width = 0u32;
    if gfm && body.starts_with('>') {
        push_run(&mut runs, "▏", PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text("▏ ", font).width;
        body = body.trim_start_matches('>').trim_start();
    } else if body.starts_with("- [ ] ") || body.starts_with("- [x] ") || body.starts_with("- [X] ")
    {
        let checked = !body.starts_with("- [ ]");
        let marker = if checked { "☑ " } else { "☐ " };
        push_run(&mut runs, marker, PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text(marker, font).width;
        body = &body[6..];
    } else if let Some(rest) = strip_bullet(body) {
        push_run(&mut runs, "• ", PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text("• ", font).width;
        body = rest;
    } else if let Some((marker, rest)) = strip_ordered(body) {
        let marker = format!("{marker} ");
        push_run(&mut runs, &marker, PreviewStyle::Accent, font, context);
        prefix_width = context.measure_text(&marker, font).width;
        body = rest;
    }

    // A table needs its delimiter row (or pipes on both outer edges), exactly as GFM does; one
    // `|` in prose is not a table.
    if gfm && body.contains('|') && (table_follows || body.trim().starts_with('|')) {
        let row = render_table_row(body, font, context, &mut runs);
        return ParsedLine { runs, row };
    }

    // The indent is kept so a nested list item still lines up under its parent.
    if indent > 0 && prefix_width == 0 {
        let pad = " ".repeat(indent);
        push_run(&mut runs, &pad, PreviewStyle::Text, font, context);
    }
    parse_inline(body, gfm, font, context, &mut runs);
    ParsedLine { runs, row: PreviewRow::Inline }
}

/// Strips a bullet marker, returning the item body.
fn strip_bullet(text: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = text.strip_prefix(marker) {
            return Some(rest);
        }
    }
    None
}

/// Strips an ordered-list marker, returning the number and the item body.
fn strip_ordered(text: &str) -> Option<(&str, &str)> {
    let digits = text.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let rest = &text[digits..];
    let rest = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    Some((&text[..digits], rest))
}

/// Whether a line is a horizontal rule (`---`, `***`, `___`).
fn is_rule(trimmed: &str) -> bool {
    let mut marker = None;
    let mut count = 0usize;
    for ch in trimmed.chars() {
        if ch == ' ' {
            continue;
        }
        match marker {
            None if ch == '-' || ch == '*' || ch == '_' => marker = Some(ch),
            Some(existing) if existing == ch => {}
            _ => return false,
        }
        count += 1;
    }
    count >= 3
}

/// Whether a table row is the `|---|:--:|` divider.
pub(crate) fn is_table_divider(trimmed: &str) -> bool {
    if !trimmed.contains('-') || !trimmed.contains('|') {
        return false;
    }
    trimmed.chars().all(|ch| matches!(ch, '|' | '-' | ':' | ' ' | '\t'))
}

/// Splits a table row into measured cells with separators between them.
///
/// Columns are separated but not padded to a shared width: padding needs the widest cell in the
/// whole table, which is a table-wide layout pass rather than a per-row one, and this preview
/// renders rows independently. The cells themselves keep their inline markup.
fn render_table_row(
    body: &str,
    font: &Font,
    context: &RenderContext,
    runs: &mut Vec<PreviewRun>,
) -> PreviewRow {
    let trimmed = body.trim().trim_start_matches('|').trim_end_matches('|');
    for (index, cell) in trimmed.split('|').enumerate() {
        if index > 0 {
            push_run(runs, " │ ", PreviewStyle::Dim, font, context);
        }
        let cell = cell.trim();
        // The cell's own inline markup still applies, so a bold cell is bold.
        parse_inline(cell, true, font, context, runs);
    }
    PreviewRow::Table
}

/// Splits inline markdown into styled runs.
// The parser peeks ahead while consuming (`**`/`__` need a second character), so a plain
// `while let Some(ch) = chars.next()` is the honest shape; clippy's iterator-lint suggestion
// would borrow the iterator twice.
#[allow(clippy::while_let_on_iterator)]
#[allow(clippy::while_let_loop)]
fn parse_inline(
    text: &str,
    gfm: bool,
    font: &Font,
    context: &RenderContext,
    runs: &mut Vec<PreviewRun>,
) {
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                let mut closed = false;
                loop {
                    let Some(next) = chars.next() else {
                        break;
                    };
                    if next == '*' && chars.peek() == Some(&'*') {
                        chars.next();
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Bold } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '_' if chars.peek() == Some(&'_') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == '_' && chars.peek() == Some(&'_') {
                        chars.next();
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Bold } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '*' | '_' => {
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let marker = ch;
                let mut inner = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == marker {
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Italic } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '~' if gfm && chars.peek() == Some(&'~') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == '~' && chars.peek() == Some(&'~') {
                        chars.next();
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                push_run(
                    runs,
                    &inner,
                    if closed { PreviewStyle::Strike } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            '`' => {
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut inner = String::new();
                for next in chars.by_ref() {
                    if next == '`' {
                        break;
                    }
                    inner.push(next);
                }
                push_run(runs, &inner, PreviewStyle::Code, font, context);
            }
            '!' if chars.peek() == Some(&'[') => {
                chars.next();
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut alt = String::new();
                let mut target = String::new();
                while let Some(next) = chars.next() {
                    if next == ']' {
                        if chars.peek() == Some(&'(') {
                            chars.next();
                            for part in chars.by_ref() {
                                if part == ')' {
                                    break;
                                }
                                target.push(part);
                            }
                        }
                        break;
                    }
                    alt.push(next);
                }
                let label = if alt.is_empty() { target } else { alt };
                push_run(runs, "▣ ", PreviewStyle::Accent, font, context);
                push_run(runs, &label, PreviewStyle::Link, font, context);
            }
            '[' => {
                push_run(runs, &core::mem::take(&mut plain), PreviewStyle::Text, font, context);
                let mut label = String::new();
                let mut is_link = false;
                while let Some(next) = chars.next() {
                    if next == ']' {
                        if chars.peek() == Some(&'(') {
                            chars.next();
                            for target in chars.by_ref() {
                                if target == ')' {
                                    break;
                                }
                            }
                            is_link = true;
                        }
                        break;
                    }
                    label.push(next);
                }
                push_run(
                    runs,
                    &label,
                    if is_link { PreviewStyle::Link } else { PreviewStyle::Text },
                    font,
                    context,
                );
            }
            _ => plain.push(ch),
        }
    }
    push_run(runs, &plain, PreviewStyle::Text, font, context);
}

/// The ink a fenced-code token is drawn in.
fn token_color(kind: TokenKind, ink: Color, accent: Color, surface: Color) -> Color {
    match kind {
        TokenKind::Keyword | TokenKind::Preprocessor => accent,
        TokenKind::String | TokenKind::Character => {
            crate::style::semantic_color(crate::style::SemanticColor::Success)
                .map(|token| token.legible_on(surface, 3.0))
                .unwrap_or(ink)
        }
        TokenKind::Number => crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .map(|token| token.legible_on(surface, 3.0))
            .unwrap_or(ink),
        TokenKind::Comment | TokenKind::BlockComment => ink.blend(&surface, 0.45),
        TokenKind::Type | TokenKind::Function => {
            crate::style::semantic_color(crate::style::SemanticColor::Info)
                .map(|token| token.legible_on(surface, 3.0))
                .unwrap_or(ink)
        }
        _ => ink,
    }
}

/// Draws one parsed preview row.
pub(crate) fn draw_parsed_line(
    context: &mut RenderContext,
    band: Rect,
    parsed: &ParsedLine,
    base_font: &Font,
    ink: Color,
    accent: Color,
    surface: Color,
) {
    match parsed.row {
        PreviewRow::Rule => {
            context.fill_rect(
                Rect::new(band.x, band.y + (band.height as i32 / 2), band.width, 1),
                ink.blend(&surface, 0.5),
            );
            return;
        }
        PreviewRow::Code => {
            context.fill_rect(band, ink.with_alpha(12));
        }
        PreviewRow::Fence => {}
        // The heading's runs were measured with the level's own size; the drawing font below
        // re-derives the same size.
        PreviewRow::Heading(_) => {}
        PreviewRow::Inline | PreviewRow::Table => {}
    }
    let font = match parsed.row {
        PreviewRow::Heading(level) => {
            base_font.with_size(base_font.size() + (7 - level.min(6)) as f32)
        }
        _ => base_font.clone(),
    };
    let mut x = band.x;
    for run in &parsed.runs {
        draw_preview_run(context, run, &font, x, band.y, ink, accent, surface);
        x += run.width as i32;
    }
}

/// Draws one styled run at `x`.
fn draw_preview_run(
    context: &mut RenderContext,
    run: &PreviewRun,
    font: &Font,
    x: i32,
    y: i32,
    ink: Color,
    accent: Color,
    surface: Color,
) {
    let color = match run.style {
        PreviewStyle::Text => ink,
        PreviewStyle::Bold => ink,
        PreviewStyle::Italic => ink.blend(&surface, 0.15),
        PreviewStyle::Strike => ink.blend(&surface, 0.45),
        PreviewStyle::Code => ink,
        PreviewStyle::Link => accent,
        PreviewStyle::Dim => ink.blend(&surface, 0.5),
        PreviewStyle::Accent => accent,
        PreviewStyle::Token(kind) => token_color(kind, ink, accent, surface),
    };
    if matches!(run.style, PreviewStyle::Code) {
        context.fill_rect(
            Rect::new(x - 2, y, run.width + 4, TEXT_ROW_HEIGHT as u32),
            ink.with_alpha(24),
        );
    }
    context.draw_text(Point::new(x, y), &run.text, font, color, HorizontalAlignment::Left);
    match run.style {
        PreviewStyle::Bold => {
            // The face has one weight, so bold is the crate's one-pixel overprint.
            context.draw_text(
                Point::new(x + 1, y),
                &run.text,
                font,
                color,
                HorizontalAlignment::Left,
            );
        }
        PreviewStyle::Link => {
            context.fill_rect(Rect::new(x, y + TEXT_ROW_HEIGHT - 3, run.width.max(1), 1), color);
        }
        PreviewStyle::Strike => {
            context.fill_rect(Rect::new(x, y + TEXT_ROW_HEIGHT / 2, run.width.max(1), 1), color);
        }
        _ => {}
    }
}
