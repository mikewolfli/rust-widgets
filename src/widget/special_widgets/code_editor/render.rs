// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Painting for the [`CodeEditor`]: geometry metrics, hit-testing geometry and
//! the `Draw` implementation.
//!
//! Rendering is a pure function of editor state plus the [`RenderContext`]; it
//! never mutates the document. All geometry lives here so the paint code and the
//! hit-testing code in `editor.rs` cannot drift apart.

use super::editor::{decimal_digits, CodeEditor};
use super::types::{
    MarkerSeverity, SearchMatch, SyntaxPalette, TextPosition, TokenKind, MIN_TOUCH_TARGET,
};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::{Draw, Widget};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

// ─────────────────────────────────────────────────────────────────────────────
// Chrome colours
// ─────────────────────────────────────────────────────────────────────────────

/// Every chrome colour the editor paints, resolved for the active appearance.
///
/// # Why this exists
///
/// The paint code below used to spell every surface, border and label colour as a
/// literal. A literal cannot move when the appearance switches, so the editor looked
/// identical in a light and a dark theme — the rendering census reported it as
/// `P3=NO`, i.e. theme-blind chrome.
///
/// # Precedence
///
/// Each field resolves **explicit style → active theme → the light-preset literal**, the
/// same order [`crate::theme::apply_active_theme`] documents: a colour the caller set on
/// the widget wins, otherwise the theme supplies one, and the literal is only the last
/// resort for a process with no theme at all.
///
/// The theme read inside [`EditorChrome::resolve`] happens through
/// [`crate::style::resolved_theme_style`], which takes and releases the manager lock
/// internally. No lock is held across the call, so the non-re-entrant global mutex is
/// never re-entered.
struct EditorChrome {
    /// The document surface: the editor's dominant painted colour.
    surface: Color,
    /// The frame and tab outline.
    border: Color,
    /// Tab strip, find bar and status bar fill.
    chrome_background: Color,
    /// Gutter fill, one step away from the surface.
    gutter_background: Color,
    /// Minimap fill, one step away from the surface.
    minimap_background: Color,
    /// Scrollbar track fill.
    scrollbar_track: Color,
    /// Scrollbar thumb fill.
    scrollbar_thumb: Color,
    /// A hairline between two chrome bands.
    separator: Color,
    /// Primary chrome label colour.
    ink: Color,
    /// Secondary chrome label colour, for inactive tabs and hints.
    dim_ink: Color,
    /// The selection / caret accent.
    accent: Color,
    /// The fill behind the caret's line.
    active_line: Color,
    /// Vertical indent-guide hairlines.
    indent_guide: Color,
    /// The dot / dash that marks a space or a tab.
    whitespace_mark: Color,
    /// Selection highlight behind selected text.
    selection: Color,
    /// Search-hit highlight.
    search_hit: Color,
    /// Highlight for the search hit the caret is on.
    search_current: Color,
    /// Outline marking other occurrences of the selected word.
    occurrence: Color,
    /// Popup (completion, context menu) fill.
    popup_background: Color,
    /// Popup outline.
    popup_border: Color,
    /// The fill behind the selected popup row.
    popup_selected: Color,
}

impl EditorChrome {
    /// Resolves every chrome colour for the current appearance.
    fn resolve(editor: &CodeEditor) -> Self {
        let style = editor.base.style().clone();
        let theme = crate::style::resolved_theme_style("code_editor");

        // Explicit style first, then the theme's resolved style for this control, then
        // the light preset's literal. That field order is the documented precedence, so a
        // caller's single override does not have to restate the palette.
        let pick = |explicit: Option<Color>, themed: Option<Color>, literal: Color| -> Color {
            explicit.or(themed).unwrap_or(literal)
        };
        let themed_bg = theme.as_ref().and_then(|t| t.background_color);
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        let surface = pick(style.background_color, themed_bg, editor.palette.background);
        let ink = pick(style.text_color, themed_ink, editor.palette.color_for(TokenKind::Plain));
        let border = pick(style.border_color, themed_border, editor.palette.border_color);

        // The accent is the theme's `primary`: the hue a theme is expected to vary most,
        // which is what makes the caret and the selection follow the appearance. It is
        // read here as its own lock acquisition, released before the semantic reads
        // below — the global manager's mutex is not re-entrant.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(editor.palette.bracket_color);

        // Search highlights and the error-tinted frame are semantic: they mean "this is
        // the hit you are on" and "this is an error", so they read the theme's semantic
        // tokens rather than a literal that only happens to be amber today.
        let warning = crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .unwrap_or(editor.palette.search_color);
        let error = crate::style::semantic_color(crate::style::SemanticColor::Error)
            .unwrap_or(editor.palette.border_color);

        // Bands beside the surface are derived from it, so each stays one visible step
        // away from the surface in either appearance rather than being two independent
        // literals that happened to look related in the light preset.
        let chrome_background = surface.blend(&ink, 0.06);

        Self {
            surface,
            // A frame in the surface's own colour would leave the editor to bleed into the
            // window, so a border that resolves equal to the surface is re-derived from
            // the error token, which is never the surface in a real theme.
            border: if border == surface { error } else { border },
            chrome_background,
            gutter_background: surface.blend(&ink, 0.03),
            minimap_background: surface.blend(&ink, 0.015),
            scrollbar_track: surface.blend(&ink, 0.05),
            scrollbar_thumb: surface.blend(&ink, 0.22),
            separator: surface.blend(&ink, 0.12),
            ink,
            dim_ink: ink.blend(&surface, 0.45),
            accent,
            active_line: surface.blend(&ink, 0.04),
            indent_guide: surface.blend(&ink, 0.1),
            whitespace_mark: surface.blend(&ink, 0.25),
            selection: accent.blend(&surface, 0.7),
            search_hit: warning.blend(&surface, 0.55),
            search_current: warning.blend(&surface, 0.15),
            occurrence: accent.blend(&surface, 0.6),
            popup_background: surface.blend(&ink, 0.02),
            popup_border: if border == surface { error } else { border },
            popup_selected: accent.blend(&surface, 0.8),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Drawing
// ─────────────────────────────────────────────────────────────────────────────

impl Draw for CodeEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        // Measure the character cell **first**: every coordinate below — token origins, caret x, the
        // column ruler, hit-testing margins — is `column * cell_width()`, and reading the configured
        // constant instead of the font's real advance is what produced overlapping text.
        self.measure_cell_width(context);
        self.refresh_visible_rows();
        let rect = self.geometry();
        let row_height = self.line_height();

        // Chrome colours come from the active theme, so a light/dark switch is visible on
        // the editor rather than only in the window behind it. The theme read inside
        // `resolve` is a separate manager lock, taken and released there — the global
        // manager's mutex is not re-entrant, so no guard is alive in this scope.
        let chrome = EditorChrome::resolve(self);

        // Background and border.
        context.fill_rect(rect, chrome.surface);
        context.draw_rect(rect, chrome.border);

        self.draw_tab_strip(context, rect, &chrome);
        self.draw_find_bar(context, rect, &chrome);
        self.draw_gutter(context, rect, &chrome);
        let mut visible = self.draw_source_rows(context, rect, row_height, &chrome);
        self.draw_column_ruler(context, rect, &chrome);
        self.draw_diagnostics(context, rect, &visible);
        visible.clear();
        self.draw_scrollbar(context, rect, &chrome);
        self.draw_minimap(context, rect, &chrome);
        self.draw_status_bar(context, rect, &chrome);
        self.draw_completion(context, &chrome);
        self.draw_context_menu(context, &chrome);
    }
}

impl CodeEditor {
    fn draw_tab_strip(&mut self, context: &mut RenderContext, rect: Rect, chrome: &EditorChrome) {
        let buffers = self.model.borrow().all_buffers.clone();
        let active = self.active_buffer();
        let strip_height = row_height_of(self.config.line_advance);
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, strip_height as u32),
            chrome.chrome_background,
        );
        let mut x = rect.x + 6;
        let advance = self.cell_width();
        for (index, buffer) in buffers.iter().enumerate() {
            let model = self.model.borrow();
            let dirty = index == active && model.saved != *model.text.borrow();
            drop(model);
            let label = if index == active && dirty {
                format!("{} •", buffer.title)
            } else {
                buffer.title.clone()
            };
            let width = ((label.chars().count() as f32 + 3.0) * advance).round() as u32;
            if x + width as i32 > rect.x + rect.width as i32 {
                break;
            }
            let tab_rect = Rect::new(x, rect.y + 2, width, strip_height.saturating_sub(4) as u32);
            if index == active {
                context.fill_rect(tab_rect, chrome.surface);
                context.draw_rect(tab_rect, chrome.border);
            }
            context.draw_text(
                Point::new(tab_rect.x + 6, tab_rect.y + strip_height - 6),
                &label,
                &Font::default(),
                if index == active { chrome.ink } else { chrome.dim_ink },
                HorizontalAlignment::Left,
            );
            x += width as i32 + 2;
        }
        context.draw_line(
            Point::new(rect.x, rect.y + strip_height),
            Point::new(rect.x + rect.width as i32, rect.y + strip_height),
            chrome.separator,
        );
    }

    fn draw_find_bar(&mut self, context: &mut RenderContext, rect: Rect, chrome: &EditorChrome) {
        if !self.find.visible {
            return;
        }
        let top = rect.y + row_height_of(self.config.line_advance);
        let height = self.find_bar_height();
        context
            .fill_rect(Rect::new(rect.x, top, rect.width, height as u32), chrome.chrome_background);
        context.draw_line(
            Point::new(rect.x, top + height),
            Point::new(rect.x + rect.width as i32, top + height),
            chrome.separator,
        );

        let query_summary = if self.find.query.is_empty() {
            "Find".to_string()
        } else {
            format!("{} / {}", self.find.display_index(), self.find.match_count())
        };
        context.draw_text(
            Point::new(rect.x + 8, top + 20),
            &query_summary,
            &Font::default(),
            chrome.dim_ink,
            HorizontalAlignment::Left,
        );
        // Positioned from the **measured** advance of the summary text in the font it is drawn in.
        // The previous version used `space_advance * 8.0` — a hand-written constant multiplied by a
        // made-up column count — which put the query field at a fixed x that had no relation to where
        // the summary actually ended. Measuring both makes the gap between them what it claims to be.
        let summary_font = Font::default();
        let summary_advance = context.backend().measure_text(&query_summary, &summary_font).width;
        let query_x = rect.x + 8 + summary_advance as i32 + self.cell_width() as i32;
        context.draw_text(
            Point::new(query_x, top + 20),
            if self.find.query.is_empty() { "(type to search)" } else { &self.find.query },
            &Font::default(),
            chrome.ink,
            HorizontalAlignment::Left,
        );

        if self.find.replace_visible {
            let row_two = top + MIN_TOUCH_TARGET + 3;
            context.draw_text(
                Point::new(rect.x + 8, row_two + 18),
                "Replace",
                &Font::default(),
                chrome.dim_ink,
                HorizontalAlignment::Left,
            );
            context.draw_text(
                Point::new(query_x, row_two + 18),
                if self.find.replacement.is_empty() {
                    "(replacement)"
                } else {
                    &self.find.replacement
                },
                &Font::default(),
                chrome.ink,
                HorizontalAlignment::Left,
            );
        }

        let close = Rect::new(
            rect.x + rect.width as i32 - MIN_TOUCH_TARGET - 4,
            top + 3,
            MIN_TOUCH_TARGET as u32,
            MIN_TOUCH_TARGET as u32,
        );
        // The close affordance is a dismissal, so it carries the theme's error colour
        // rather than the literal red it used to: the same token means "this removes
        // something" everywhere else in the library.
        let close_color = crate::style::semantic_color(crate::style::SemanticColor::Error)
            .unwrap_or(chrome.accent);
        context.draw_rect(close, close_color);
        context.draw_text(
            Point::new(close.x + 9, close.y + 19),
            "x",
            &Font::default(),
            close_color,
            HorizontalAlignment::Left,
        );
    }

    fn draw_gutter(&mut self, context: &mut RenderContext, rect: Rect, chrome: &EditorChrome) {
        let width = self.gutter_width();
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let height = (bottom - top).max(0) as u32;
        if height == 0 {
            return;
        }
        context.fill_rect(Rect::new(rect.x, top, width as u32, height), chrome.gutter_background);
        context.draw_line(
            Point::new(rect.x + width, top),
            Point::new(rect.x + width, top + height as i32),
            chrome.separator,
        );

        let row_height = self.config.line_advance.max(1.0);
        let fold_width = self.fold_marker_width();
        let rows_budget = (height as f32 / row_height).floor() as usize;
        let mut row = 0usize;
        for line in self.visible_document_lines() {
            let segments = self.wrap_segments(line);
            if row >= self.scroll_visual_row + rows_budget {
                break;
            }
            if row >= self.scroll_visual_row {
                let y = top + ((row - self.scroll_visual_row) as f32 * row_height).round() as i32;
                if self.config.show_line_numbers {
                    context.draw_text(
                        Point::new(rect.x + fold_width, y + (row_height * 0.78) as i32),
                        &format!("{}", line + 1),
                        &Font::default(),
                        if line == self.cursor.head.line { chrome.ink } else { chrome.dim_ink },
                        HorizontalAlignment::Left,
                    );
                }
                // Fold affordance: `>` for folded, `v` for foldable, `.` otherwise.
                let foldable = self.fold_regions().iter().any(|region| region.start_line == line);
                let marker = if foldable && self.is_line_folded(line) {
                    ">"
                } else if foldable {
                    "v"
                } else {
                    "."
                };
                context.draw_text(
                    Point::new(rect.x + 2, y + (row_height * 0.78) as i32),
                    marker,
                    &Font::default(),
                    if foldable { chrome.dim_ink } else { chrome.separator },
                    HorizontalAlignment::Left,
                );
                // Severity dot; the top-most severity wins.
                if let Some(severity) = self.highest_severity_on_line(line) {
                    context.fill_rect(
                        Rect::new(rect.x + fold_width + 4, y + 5, 5, 5),
                        severity.color(),
                    );
                }
            }
            row += segments.max(1);
        }
    }

    /// Draws source rows and returns the document line index painted per row.
    fn draw_source_rows(
        &mut self,
        context: &mut RenderContext,
        rect: Rect,
        row_height: f32,
        chrome: &EditorChrome,
    ) -> Vec<usize> {
        let left = rect.x + self.text_origin_x();
        let right = rect.x + rect.width as i32 - self.scrollbar_width() - self.minimap_width();
        let width = (right - left).max(1) as u32;
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let height = (bottom - top).max(0) as u32;
        if height == 0 || width == 0 {
            return Vec::new();
        }

        context.push_clip(left, top, width, height);

        let rows_budget = (height as f32 / row_height).ceil() as usize + 1;
        let wrapped = self.config.word_wrap;
        let font = self.font();
        let palette = self.palette.clone();
        let active_line = self.cursor.head.line;
        let brackets = self.matching_brackets();
        let occurrences = self.occurrences_of_selection();
        let current_match = self.find.matches.get(self.find.current).cloned();
        let (selection_start, selection_end) = self.cursor.bounds();
        let mut painted: Vec<usize> = Vec::new();

        let mut row = 0usize;
        for line in self.visible_document_lines() {
            let segments = self.wrap_segments(line);
            if row + segments <= self.scroll_visual_row {
                row += segments;
                continue;
            }
            if painted.len() >= rows_budget {
                break;
            }
            let text = self.line_text(line).unwrap_or_default();
            let chars: Vec<char> = text.chars().collect();
            for segment in 0..segments {
                let visual_row = row + segment;
                if visual_row < self.scroll_visual_row {
                    continue;
                }
                let y = top
                    + ((visual_row - self.scroll_visual_row) as f32 * row_height).round() as i32;
                let segment_start = if wrapped { segment * self.text_columns().max(1) } else { 0 };
                let segment_end = if wrapped {
                    (segment_start + self.text_columns().max(1)).min(chars.len())
                } else {
                    chars.len()
                };

                if self.config.highlight_active_line && line == active_line {
                    context.fill_rect(
                        Rect::new(rect.x, y, rect.width, row_height.ceil() as u32),
                        chrome.active_line,
                    );
                }

                self.draw_line_backdrops(
                    context,
                    left,
                    y,
                    row_height,
                    line,
                    segment_start,
                    segment_end,
                    current_match.as_ref(),
                    &occurrences,
                    chrome,
                );

                if self.config.show_indent_guides {
                    self.draw_indent_guides(
                        context,
                        left,
                        y,
                        segment_start,
                        segment_end,
                        &chars,
                        chrome,
                    );
                }

                self.draw_row_text(
                    context,
                    left,
                    y,
                    &font,
                    &palette,
                    line,
                    segment_start,
                    segment_end,
                    &chars,
                    &brackets,
                    selection_start,
                    selection_end,
                    row_height,
                    chrome,
                );
                if self.config.show_whitespace {
                    self.draw_whitespace_marks(
                        context,
                        left,
                        y,
                        &chars,
                        segment_start,
                        segment_end,
                        row_height,
                        chrome,
                    );
                }
                painted.push(line);
            }
            row += segments;
        }

        self.draw_caret(context, left, top, row_height, chrome);
        context.pop_clip();
        painted
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_line_backdrops(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        row_height: f32,
        line: usize,
        segment_start: usize,
        segment_end: usize,
        current_match: Option<&SearchMatch>,
        occurrences: &[SearchMatch],
        chrome: &EditorChrome,
    ) {
        let cell = self.cell_width();
        // Search hits.
        for hit in self.find.matches.iter().filter(|hit| hit.line == line) {
            if let (Some(from), Some(to)) =
                self.clip_columns(hit.start_column, hit.end_column, segment_start, segment_end)
            {
                let x = left + ((from as f32 - self.scroll_column as f32) * cell).round() as i32;
                let width = (((to - from) as f32) * cell).round().max(1.0) as u32;
                context
                    .fill_rect(Rect::new(x, y, width, row_height.ceil() as u32), chrome.search_hit);
            }
        }
        if let Some(hit) = current_match.filter(|hit| hit.line == line) {
            if let (Some(from), Some(to)) =
                self.clip_columns(hit.start_column, hit.end_column, segment_start, segment_end)
            {
                let x = left + ((from as f32 - self.scroll_column as f32) * cell).round() as i32;
                let width = (((to - from) as f32) * cell).round().max(1.0) as u32;
                context.fill_rect(
                    Rect::new(x, y, width, row_height.ceil() as u32),
                    chrome.search_current,
                );
            }
        }
        // Selected-word occurrences.
        for hit in occurrences.iter().filter(|hit| hit.line == line) {
            if let (Some(from), Some(to)) =
                self.clip_columns(hit.start_column, hit.end_column, segment_start, segment_end)
            {
                let x = left + ((from as f32 - self.scroll_column as f32) * cell).round() as i32;
                let width = (((to - from) as f32) * cell).round().max(1.0) as u32;
                context
                    .draw_rect(Rect::new(x, y, width, row_height.ceil() as u32), chrome.occurrence);
            }
        }
    }

    fn clip_columns(
        &self,
        from: usize,
        to: usize,
        segment_start: usize,
        segment_end: usize,
    ) -> (Option<usize>, Option<usize>) {
        let start = from.max(segment_start);
        let end = to.min(segment_end);
        if end <= start {
            (None, None)
        } else {
            (Some(start), Some(end))
        }
    }

    fn draw_indent_guides(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        segment_start: usize,
        segment_end: usize,
        chars: &[char],
        chrome: &EditorChrome,
    ) {
        let cell = self.cell_width();
        let width = self.config.tab_width.max(1);
        let mut column = 0usize;
        while column < chars.len() {
            let indent_end = column + width;
            let all_space = chars[column..indent_end.min(chars.len())]
                .iter()
                .all(|ch| *ch == ' ' || *ch == '\t');
            if !all_space {
                break;
            }
            if column >= segment_start && column < segment_end {
                let x = left + ((column as f32 - self.scroll_column as f32) * cell).round() as i32;
                context.draw_line(
                    Point::new(x, y),
                    Point::new(x, y + self.config.line_advance.round() as i32),
                    chrome.indent_guide,
                );
            }
            column = indent_end;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_row_text(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        font: &Font,
        palette: &SyntaxPalette,
        line: usize,
        segment_start: usize,
        segment_end: usize,
        chars: &[char],
        brackets: &Option<(TextPosition, TextPosition)>,
        selection_start: TextPosition,
        selection_end: TextPosition,
        row_height: f32,
        chrome: &EditorChrome,
    ) {
        let cell = self.cell_width();
        let text: String =
            chars[segment_start.min(chars.len())..segment_end.min(chars.len())].iter().collect();
        if text.is_empty() {
            return;
        }

        // Selection background for this visual row.
        if !self.cursor.is_collapsed() {
            let row_start = TextPosition::new(line, segment_start);
            let row_end = TextPosition::new(line, segment_end);
            if selection_start <= row_end && selection_end >= row_start {
                let from = selection_start
                    .column
                    .max(segment_start)
                    .max(if selection_start.line < line { 0 } else { selection_start.column });
                let to = if selection_end.line > line {
                    segment_end
                } else {
                    selection_end.column.min(segment_end)
                };
                if to > from {
                    let x =
                        left + ((from as f32 - self.scroll_column as f32) * cell).round() as i32;
                    let width = (((to - from) as f32) * cell).round().max(1.0) as u32;
                    context.fill_rect(
                        Rect::new(x, y, width, row_height.ceil() as u32),
                        chrome.selection,
                    );
                }
            }
        }

        // Token colouring. Token spans are line-relative byte offsets, so they
        // are converted to character offsets before intersecting the visible
        // segment.
        let spans = self.tokens_on_line(line);
        let baseline = y + (row_height * 0.78).round() as i32;
        let mut cursor_char = segment_start;
        for span in spans {
            let start_char = byte_to_char_index(self, line, span.start);
            let end_char = byte_to_char_index(self, line, span.end);
            if end_char <= segment_start || start_char >= segment_end {
                continue;
            }
            let from = start_char.max(segment_start);
            let to = end_char.min(segment_end);
            if to <= from {
                continue;
            }
            if from > cursor_char {
                let plain: String = chars[cursor_char..from.min(chars.len())].iter().collect();
                if !plain.is_empty() {
                    context.draw_text(
                        Point::new(
                            left + ((cursor_char as f32 - self.scroll_column as f32) * cell).round()
                                as i32,
                            baseline,
                        ),
                        &plain,
                        font,
                        // Body text is the resolved ink, not the palette's light-preset
                        // literal: the palette carries the *syntax hues*, which a caller may
                        // replace, but plain running text has to stay legible against the
                        // resolved surface.
                        chrome.ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
            let piece: String = chars[from..to.min(chars.len())].iter().collect();
            let x = left + ((from as f32 - self.scroll_column as f32) * cell).round() as i32;
            let span_font = match span.kind {
                TokenKind::Keyword => font.clone().with_bold(true),
                TokenKind::Type | TokenKind::Function => font.clone().with_bold(false),
                TokenKind::Comment | TokenKind::BlockComment => font.clone().with_italic(true),
                _ => font.clone(),
            };
            let mut color = palette.color_for(span.kind);
            // Bracket-pair highlight: repaint both glyphs in the bracket colour.
            if let Some((open, close)) = brackets {
                if (open.line == line && open.column == from)
                    || (close.line == line && close.column == from)
                {
                    color = chrome.accent;
                }
            }
            context.draw_text(
                Point::new(x, baseline),
                &piece,
                &span_font,
                color,
                HorizontalAlignment::Left,
            );
            cursor_char = to;
        }

        // Bracket-pair background boxes (drawn on top of the glyphs).
        if let Some((open, close)) = brackets {
            for position in [open, close] {
                if position.line != line {
                    continue;
                }
                let column = position.column;
                if column < segment_start || column >= segment_end {
                    continue;
                }
                let x = left + ((column as f32 - self.scroll_column as f32) * cell).round() as i32;
                context.draw_rect(
                    Rect::new(x - 1, y, (cell).round() as u32 + 2, row_height.ceil() as u32),
                    chrome.accent,
                );
            }
        }
        let _ = (selection_end, segment_end);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_whitespace_marks(
        &self,
        context: &mut RenderContext,
        left: i32,
        y: i32,
        chars: &[char],
        segment_start: usize,
        segment_end: usize,
        row_height: f32,
        chrome: &EditorChrome,
    ) {
        let cell = self.cell_width();
        let dot_y = y + (row_height * 0.5).round() as i32;
        let from = segment_start.min(chars.len());
        let to = segment_end.min(chars.len());
        for (offset, ch) in chars[from..to].iter().enumerate() {
            let column = from + offset;
            let x = left + ((column as f32 - self.scroll_column as f32) * cell).round() as i32;
            match ch {
                ' ' => context.fill_rect(
                    Rect::new(x + (cell * 0.5) as i32, dot_y, 1, 1),
                    chrome.whitespace_mark,
                ),
                '\t' => context.draw_line(
                    Point::new(x + 2, dot_y),
                    Point::new(x + cell as i32 - 2, dot_y),
                    chrome.whitespace_mark,
                ),
                _ => {}
            }
        }
    }

    fn draw_column_ruler(&self, context: &mut RenderContext, rect: Rect, chrome: &EditorChrome) {
        let column = self.config.column_ruler;
        if column == 0 || column < self.scroll_column {
            return;
        }
        let cell = self.cell_width();
        let x = rect.x
            + self.text_origin_x()
            + ((column as f32 - self.scroll_column as f32) * cell).round() as i32;
        let right = rect.x + rect.width as i32 - self.scrollbar_width() - self.minimap_width();
        if x >= right {
            return;
        }
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        context.draw_line(Point::new(x, top), Point::new(x, bottom), chrome.separator);
    }

    fn draw_caret(
        &self,
        context: &mut RenderContext,
        left: i32,
        top: i32,
        row_height: f32,
        chrome: &EditorChrome,
    ) {
        // Secondary carets first so the primary caret paints on top.
        let all_carets = self.cursors();
        for caret in all_carets.carets().iter().copied() {
            if caret == self.cursor {
                continue;
            }
            if caret.is_collapsed() {
                let row = self.line_to_visual_row(caret.head.line);
                let y = top
                    + ((row as f32 - self.scroll_visual_row as f32) * row_height).round() as i32;
                let x = left
                    + (((caret.head.column.saturating_sub(self.scroll_column)) as f32)
                        * self.cell_width())
                    .round() as i32;
                context.fill_rect(Rect::new(x, y, 2, row_height.ceil() as u32), chrome.accent);
            } else if let Some(rect) = self.rect_for_range(caret.bounds().0, caret.bounds().1) {
                context.fill_rect(rect, chrome.selection);
            }
        }
        // Primary caret.
        if self.cursor.is_collapsed() {
            let row = self.line_to_visual_row(self.cursor.head.line);
            let y =
                top + ((row as f32 - self.scroll_visual_row as f32) * row_height).round() as i32;
            let x = left
                + (((self.cursor.head.column.saturating_sub(self.scroll_column)) as f32)
                    * self.cell_width())
                .round() as i32;
            context.fill_rect(Rect::new(x, y, 2, row_height.ceil() as u32), chrome.accent);
        } else {
            let (start, end) = self.cursor.bounds();
            if let Some(rect) = self.rect_for_range(start, end) {
                context.draw_rect(rect, chrome.accent);
            }
        }
    }

    fn draw_diagnostics(&self, context: &mut RenderContext, rect: Rect, visible: &[usize]) {
        let inline = self.inline_diagnostics();
        if inline.is_empty() {
            return;
        }
        let row_height = self.config.line_advance.max(1.0);
        let top = rect.y + self.text_origin_y();
        let cell = self.cell_width();
        for (row, marker) in inline {
            if row < self.scroll_visual_row {
                continue;
            }
            let y = top
                + ((row as f32 - self.scroll_visual_row as f32 + 1.0) * row_height).round() as i32;
            let x = rect.x
                + self.text_origin_x()
                + ((marker.start_column().saturating_sub(1)) as f32 * cell).round() as i32;
            let width = (((marker.finish_column() - marker.start_column() + 1).max(1)) as f32
                * cell)
                .round() as u32;
            context.draw_line(
                Point::new(x, y + 2),
                Point::new(x + width as i32, y + 2),
                marker.severity.color(),
            );
            let label = format!("{}: {}", marker.severity.badge(), marker.message);
            context.draw_text(
                Point::new(x, y + (row_height * 0.75) as i32),
                &label,
                &Font::default(),
                marker.severity.color(),
                HorizontalAlignment::Left,
            );
            let _ = visible;
        }
    }

    fn draw_scrollbar(&self, context: &mut RenderContext, rect: Rect, chrome: &EditorChrome) {
        let total = self.total_visual_rows().max(1);
        let rows = self.visible_rows().max(1);
        if total <= rows {
            return;
        }
        let width = self.scrollbar_width();
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let track_height = (bottom - top).max(1) as u32;
        let track = Rect::new(
            rect.x + rect.width as i32 - width - self.minimap_width(),
            top,
            width as u32,
            track_height,
        );
        context.fill_rect(track, chrome.scrollbar_track);
        let thumb_height = ((rows as f32 / total as f32) * track_height as f32).max(12.0) as u32;
        let max_offset = total.saturating_sub(rows) as f32;
        let ratio = if max_offset <= 0.0 {
            0.0
        } else {
            (self.scroll_visual_row as f32 / max_offset).clamp(0.0, 1.0)
        };
        let thumb_y =
            top + ((track_height.saturating_sub(thumb_height)) as f32 * ratio).round() as i32;
        context.fill_rect(
            Rect::new(track.x + 2, thumb_y, (width - 4).max(2) as u32, thumb_height),
            chrome.scrollbar_thumb,
        );
    }

    fn draw_minimap(&self, context: &mut RenderContext, rect: Rect, chrome: &EditorChrome) {
        let width = self.minimap_width();
        if width == 0 {
            return;
        }
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let height = (bottom - top).max(1) as u32;
        let x = rect.x + rect.width as i32 - width;
        context.fill_rect(Rect::new(x, top, width as u32, height), chrome.minimap_background);

        let line_count = self.line_count().max(1);
        let scale = height as f32 / line_count as f32;
        let dot = scale.max(1.0);
        for (index, text) in self.model.borrow().lines.iter().enumerate() {
            let y = top + (index as f32 * scale).round() as i32;
            let indent = text.chars().take_while(|ch| ch.is_whitespace()).count();
            let length = text.trim_end().chars().count().saturating_sub(indent);
            if length == 0 {
                continue;
            }
            let bar_width =
                ((length as f32 / 120.0) * (width - 8) as f32).clamp(1.0, (width - 8) as f32);
            context.fill_rect(
                Rect::new(
                    x + 4
                        + ((indent as f32 / 120.0) * (width - 8) as f32).min((width - 8) as f32)
                            as i32,
                    y,
                    bar_width as u32,
                    dot.ceil().max(1.0) as u32,
                ),
                chrome.scrollbar_thumb,
            );
        }
        // Viewport indicator.
        let rows = self.visible_rows().max(1);
        let viewport_top = top + (self.scroll_visual_row as f32 * scale).round() as i32;
        let viewport_height = ((rows as f32 * scale).max(6.0)) as u32;
        context.draw_rect(
            Rect::new(x, viewport_top, width as u32, viewport_height.min(height)),
            chrome.accent,
        );
    }

    fn draw_status_bar(&self, context: &mut RenderContext, rect: Rect, chrome: &EditorChrome) {
        let height = self.status_bar_height();
        let top = rect.y + rect.height as i32 - height;
        context
            .fill_rect(Rect::new(rect.x, top, rect.width, height as u32), chrome.chrome_background);
        context.draw_line(
            Point::new(rect.x, top),
            Point::new(rect.x + rect.width as i32, top),
            chrome.separator,
        );

        let selected = self.selected_text().map(|text| text.chars().count()).unwrap_or(0);
        let fold_note = if self.folded_line_count() > 0 {
            format!("  folded:{}", self.folded_line_count())
        } else {
            String::new()
        };
        let caret_note = if self.has_multiple_cursors() {
            format!("  carets:{}", self.cursors().len())
        } else {
            String::new()
        };
        // The status row is a **single band**: the label and the caret position are laid
        // out against the same rectangle and fitted inside it, so a long language name or
        // a wide selection count shortens instead of running off the editor's edge. The two
        // used to be placed from opposite edges of the whole width with no bound at all,
        // which is why `Plain Text  1 lines` and `Ln 1, Col 1` both ended up painted past
        // the control.
        let status_band = Rect::new(rect.x + 8, top + 5, rect.width.saturating_sub(20), 16);
        let font = Font::default();
        let left_label = format!(
            "{}  {} lines{}{}",
            self.language_name(),
            self.line_count(),
            fold_note,
            caret_note
        );
        let caret_rect = {
            let right_label = if selected > 0 {
                format!(
                    "{} selected   Ln {}, Col {}",
                    selected,
                    self.cursor.head.line + 1,
                    self.cursor.head.column + 1
                )
            } else {
                format!("Ln {}, Col {}", self.cursor.head.line + 1, self.cursor.head.column + 1)
            };
            let width = context.measure_text(&right_label, &font).width;
            // The caret label keeps its own strip at the right; the left label is fitted
            // into what is left, which is the information order a reader expects (where the
            // cursor is matters more than how many lines the file has).
            let strip = Rect::new(
                (status_band.x + status_band.width as i32 - width as i32).max(status_band.x),
                status_band.y,
                width.min(status_band.width),
                status_band.height,
            );
            context.draw_text_fitted(
                strip,
                &right_label,
                &font,
                chrome.dim_ink,
                HorizontalAlignment::Right,
            );
            Rect::new(
                status_band.x,
                status_band.y,
                (strip.x - status_band.x).max(0) as u32,
                status_band.height,
            )
        };
        context.draw_text_fitted(
            caret_rect,
            &left_label,
            &font,
            chrome.dim_ink,
            HorizontalAlignment::Left,
        );

        // The diagnostic tally is a summary of severity counts, so it reads the dimmest
        // chrome ink rather than a literal that reads on only one appearance.
        let diagnostics = format!(
            "E{} W{} I{}",
            self.marker_count(MarkerSeverity::Error),
            self.marker_count(MarkerSeverity::Warning),
            self.marker_count(MarkerSeverity::Info)
        );
        let tally = Rect::new(status_band.x, top + 5, status_band.width, 12);
        context.draw_text_fitted(
            tally,
            &diagnostics,
            &font,
            chrome.dim_ink,
            HorizontalAlignment::Right,
        );
    }

    fn draw_completion(&self, context: &mut RenderContext, chrome: &EditorChrome) {
        if !self.completion.visible {
            return;
        }
        let rect = self.completion_rect();
        context.fill_rect(rect, chrome.popup_background);
        context.draw_rect(rect, chrome.popup_border);
        for (index, item) in self.completion.items.iter().enumerate() {
            let row_y = rect.y + index as i32 * MIN_TOUCH_TARGET;
            if index == self.completion.selected {
                context.fill_rect(
                    Rect::new(rect.x + 1, row_y, rect.width - 2, MIN_TOUCH_TARGET as u32),
                    chrome.popup_selected,
                );
            }
            let sigil = item.kind.map(|kind| format!("{} ", kind)).unwrap_or_default();
            let label = match &item.detail {
                Some(detail) => format!("{}{}  {}", sigil, item.label, detail),
                None => format!("{}{}", sigil, item.label),
            };
            context.draw_text(
                Point::new(rect.x + 8, row_y + 19),
                &label,
                &Font::default(),
                chrome.ink,
                HorizontalAlignment::Left,
            );
        }
    }

    fn draw_context_menu(&self, context: &mut RenderContext, chrome: &EditorChrome) {
        if !self.context_menu.visible {
            return;
        }
        let Some(rect) = self.context_menu_rect() else { return };
        context.fill_rect(rect, chrome.popup_background);
        context.draw_rect(rect, chrome.popup_border);
        for (index, item) in self.context_menu.items.iter().enumerate() {
            let row_y = rect.y + index as i32 * MIN_TOUCH_TARGET;
            if index == self.context_menu.selected && !item.disabled {
                context.fill_rect(
                    Rect::new(rect.x + 1, row_y, rect.width - 2, MIN_TOUCH_TARGET as u32),
                    chrome.popup_selected,
                );
            }
            // A disabled row is the dim ink; an enabled one is the resolved body ink.
            let color = if item.disabled { chrome.dim_ink } else { chrome.ink };
            context.draw_text(
                Point::new(rect.x + 8, row_y + 19),
                &item.label,
                &Font::default(),
                color,
                HorizontalAlignment::Left,
            );
            if let Some(shortcut) = &item.shortcut {
                context.draw_text(
                    Point::new(rect.x + rect.width as i32 - 8, row_y + 19),
                    shortcut,
                    &Font::default(),
                    chrome.dim_ink,
                    HorizontalAlignment::Right,
                );
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Free helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Rounds the configured line advance up to a tab-strip height.
fn row_height_of(line_advance: f32) -> i32 {
    (line_advance * 1.75).round().max(MIN_TOUCH_TARGET as f32) as i32
}

/// Converts a line-relative byte offset into a character index.
fn byte_to_char_index(editor: &CodeEditor, line: usize, byte: usize) -> usize {
    let text = editor.line_text(line).unwrap_or_default();
    let byte = floor_boundary(&text, byte.min(text.len()));
    text[..byte].chars().count()
}

/// Returns the largest `char` boundary at or below `byte`.
fn floor_boundary(text: &str, byte: usize) -> usize {
    let mut offset = byte.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}
impl CodeEditor {
    pub(crate) fn line_height(&self) -> f32 {
        self.config.line_advance.max(1.0)
    }

    /// The horizontal advance of one character cell.
    ///
    /// Returns the **measured** advance once [`Self::measure_cell_width`] has run, and the
    /// configured `space_advance` before the first frame — there is no measuring before a backend
    /// is open, and the first frame's layout needs a number.
    pub(crate) fn cell_width(&self) -> f32 {
        self.measured_cell_width.get().unwrap_or_else(|| self.config.space_advance.max(1.0))
    }

    /// The font the body text is drawn in.
    ///
    /// One accessor so the measurement uses the same `Font` the drawing does — the draw sites built
    /// the font inline, which is exactly how a measurement and a draw come to disagree.
    pub(crate) fn editor_font(&self) -> crate::core::Font {
        crate::core::Font::new(self.config.font_family.clone(), self.config.font_size, false, false)
    }

    /// Re-measures the character cell with the backend's own font metrics.
    ///
    /// # Why this is measured rather than read from the config
    ///
    /// `config.space_advance` is hand-written (`7.0` by default) while the renderer draws text with
    /// the *real* font, and the two disagreed:
    ///
    /// ```text
    /// measured:  " " = 4    "M" = 8    "0123456789" = 78 (7.8 each)    "中文" = 26 (13 each)
    /// config:    everything = 7.0
    /// ```
    ///
    /// Every column past the first therefore drifted by 0.8px — a whole character by column ten — and
    /// CJK by 6px *per character*. Because the editor positions each syntax token at `column * cell`
    /// and then draws that token's text at that origin, the drift landed the pieces on top of one
    /// another: overlapping, unreadable text.
    ///
    /// # Why a digit, and why inside `draw`
    ///
    /// A digit rather than a space: the space measured **4** against a real advance of **7.8**, so it
    /// is not a reliable sample of the grid. Inside `draw` because the font metrics belong to the
    /// **backend**, which is only reachable while a context is open.
    ///
    /// # Why one cell and not per-character widths
    ///
    /// The editor is a fixed-pitch grid by construction: `text_columns`, caret hit-testing and the
    /// column ruler all compute a column as an integer multiple of one cell. Per-character shaping
    /// would be a different editor, not a fix for this one.
    pub(crate) fn measure_cell_width(&mut self, context: &mut RenderContext) {
        if self.measured_geometry == Some(self.geometry()) {
            return;
        }
        let font = self.editor_font();
        // `TextMetrics::width` is a `u32` in **logical** pixels, already rounded by the backend, so it
        // is directly comparable with the configured `space_advance` (also logical pixels).
        //
        // A digit is one cell wide in a monospace face, and this is the number the renderer will
        // actually advance by — which is the whole point: the placement and the drawing now read the
        // same measurement instead of one reading a hand-written constant.
        let advance = context.backend().measure_text("0", &font).width;
        // `> 0` guards a backend that cannot measure and returns 0: accepting that would collapse every
        // column onto x=0, which is worse than the drift being fixed.
        self.measured_cell_width.set(Some(if advance > 0 {
            advance as f32
        } else {
            self.config.space_advance.max(1.0)
        }));
        self.measured_geometry = Some(self.geometry());
    }

    pub(crate) fn text_columns(&self) -> usize {
        let width = self.geometry().width as i32 - self.text_origin_x() - self.minimap_width();
        (width.max(1) as f32 / self.cell_width()).floor().max(1.0) as usize
    }

    pub(crate) fn compute_visible_rows(&self) -> usize {
        let usable = self.geometry().height as i32 - self.text_origin_y();
        (usable.max(1) as f32 / self.line_height()).floor().max(1.0) as usize
    }

    pub(crate) fn refresh_visible_rows(&mut self) {
        self.visible_rows = self.compute_visible_rows();
    }

    /// Returns the gutter width for the current geometry.
    pub fn gutter_width(&self) -> i32 {
        let marker = self.fold_marker_width();
        if !self.config.show_line_numbers {
            return marker;
        }
        let digits = decimal_digits(self.line_count());
        marker
            + (digits as f32 * self.cell_width()).round() as i32
            + (self.cell_width() * 1.5).round() as i32
    }

    pub(crate) fn fold_marker_width(&self) -> i32 {
        (self.cell_width() * 2.0).round().max(16.0) as i32
    }

    /// Returns the minimap width, or 0 when disabled.
    pub fn minimap_width(&self) -> i32 {
        if self.config.show_minimap {
            (self.cell_width() * 4.0).round().max(24.0) as i32
        } else {
            0
        }
    }

    /// Returns the scrollbar width.
    pub fn scrollbar_width(&self) -> i32 {
        if self.config.show_minimap {
            6
        } else {
            10
        }
    }

    /// Returns the x coordinate where source text begins.
    pub fn text_origin_x(&self) -> i32 {
        self.gutter_width() + (self.cell_width() * 0.5).round() as i32
    }

    /// Returns the y coordinate where the first visual row begins.
    pub fn text_origin_y(&self) -> i32 {
        self.tab_strip_height() + self.find_bar_height()
    }

    pub(crate) fn tab_strip_height(&self) -> i32 {
        (self.line_height() * 1.75).round().max(MIN_TOUCH_TARGET as f32) as i32
    }

    pub(crate) fn status_bar_height(&self) -> i32 {
        MIN_TOUCH_TARGET
    }

    /// Returns the find-bar height, or 0 when hidden.
    pub fn find_bar_height(&self) -> i32 {
        if !self.find.visible {
            return 0;
        }
        let rows = if self.find.replace_visible { 2 } else { 1 };
        rows * MIN_TOUCH_TARGET + 6
    }

    /// Maps a widget-local point to a text position, or `None` outside the text area.
    pub fn position_at_point(&self, point: Point) -> Option<TextPosition> {
        let rect = self.geometry();
        if point.x < rect.x || point.y < rect.y {
            return None;
        }
        let local_y = point.y - rect.y - self.text_origin_y();
        if local_y < 0 {
            return None;
        }
        let row = self.scroll_visual_row + (local_y as f32 / self.line_height()).floor() as usize;
        let line = self.visual_row_to_line(row);
        let local_x = point.x - rect.x - self.text_origin_x();
        let column = if local_x <= 0 {
            0
        } else {
            (local_x as f32 / self.cell_width()).round() as usize + self.scroll_column
        };
        Some(self.clamp_position(TextPosition::new(line, column)))
    }

    /// Returns the widget-local rectangle of a text range.
    pub fn rect_for_range(&self, start: TextPosition, end: TextPosition) -> Option<Rect> {
        let start = self.clamp_position(start);
        let end = self.clamp_position(end);
        if end < start {
            return None;
        }
        let rect = self.geometry();
        let start_row = self.line_to_visual_row(start.line);
        let end_row = self.line_to_visual_row(end.line);
        let top = rect.y
            + self.text_origin_y()
            + ((start_row as f32 - self.scroll_visual_row as f32) * self.line_height()).round()
                as i32;
        let rows = end_row.saturating_sub(start_row) + 1;
        let left = rect.x
            + self.text_origin_x()
            + ((start.column as f32 - self.scroll_column as f32) * self.cell_width()).round()
                as i32;
        let width = if end.line > start.line {
            rect.width.max(1)
        } else {
            (((end.column.saturating_sub(start.column)) as f32) * self.cell_width())
                .round()
                .max(1.0) as u32
        };
        let height = (rows as f32 * self.line_height()).round().max(1.0) as u32;
        Some(Rect::new(left, top, width, height))
    }

    /// Rectangle of the context menu, or `None` when closed.
    pub fn context_menu_rect(&self) -> Option<Rect> {
        if !self.context_menu.visible {
            return None;
        }
        let rect = self.geometry();
        let width = (rect.width as i32 / 2).clamp(180, 260) as u32;
        let height = self.context_menu.items.len().max(1) as u32 * MIN_TOUCH_TARGET as u32;
        let max_x = (rect.width as i32 - width as i32).max(0);
        let max_y = (rect.height as i32 - height as i32).max(0);
        Some(Rect::new(
            rect.x + self.context_menu.position.x.clamp(0, max_x),
            rect.y + self.context_menu.position.y.clamp(0, max_y),
            width,
            height,
        ))
    }

    /// Rectangle of the completion popup, clamped into the widget bounds.
    pub fn completion_rect(&self) -> Rect {
        let rect = self.geometry();
        let width = (rect.width as i32 / 2).clamp(180, 340) as u32;
        let height = self.completion.items.len().max(1) as u32 * MIN_TOUCH_TARGET as u32;
        let caret_row = self.line_to_visual_row(self.cursor.head.line);
        let y = rect.y
            + self.text_origin_y()
            + ((caret_row as f32 - self.scroll_visual_row as f32 + 1.0) * self.line_height())
                .round() as i32;
        let max_y = (rect.y + rect.height as i32 - height as i32).max(rect.y);
        let max_x = (rect.x + rect.width as i32 - width as i32).max(rect.x);
        Rect::new(
            (rect.x + self.text_origin_x()).min(max_x),
            y.min(max_y.max(rect.y)),
            width,
            height,
        )
    }
}
