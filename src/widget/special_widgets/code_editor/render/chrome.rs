// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Chrome colours and per-row geometry.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

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
pub(super) struct EditorChrome {
    /// The document surface: the editor's dominant painted colour.
    pub(super) surface: Color,
    /// The frame and tab outline.
    pub(super) border: Color,
    /// Tab strip, find bar and status bar fill.
    pub(super) chrome_background: Color,
    /// Gutter fill, one step away from the surface.
    pub(super) gutter_background: Color,
    /// Minimap fill, one step away from the surface.
    pub(super) minimap_background: Color,
    /// Scrollbar track fill.
    pub(super) scrollbar_track: Color,
    /// Scrollbar thumb fill.
    pub(super) scrollbar_thumb: Color,
    /// A hairline between two chrome bands.
    pub(super) separator: Color,
    /// Primary chrome label colour.
    pub(super) ink: Color,
    /// Secondary chrome label colour, for inactive tabs and hints.
    pub(super) dim_ink: Color,
    /// The selection / caret accent.
    pub(super) accent: Color,
    /// The fill behind the caret's line.
    pub(super) active_line: Color,
    /// Vertical indent-guide hairlines.
    pub(super) indent_guide: Color,
    /// The dot / dash that marks a space or a tab.
    pub(super) whitespace_mark: Color,
    /// Selection highlight behind selected text.
    pub(super) selection: Color,
    /// Search-hit highlight.
    pub(super) search_hit: Color,
    /// Highlight for the search hit the caret is on.
    pub(super) search_current: Color,
    /// Background tint marking other occurrences of the selected word.
    pub(super) occurrence: Color,
    /// Popup (completion, context menu) fill.
    pub(super) popup_background: Color,
    /// Popup outline.
    pub(super) popup_border: Color,
    /// The fill behind the selected popup row.
    pub(super) popup_selected: Color,
}

impl EditorChrome {
    /// Resolves every chrome colour for the current appearance.
    pub(super) fn resolve(editor: &CodeEditor) -> Self {
        // Borrowed: `resolve` only reads the style. The clone copied the whole
        // `WidgetStyle` (`Font.family` string and gradient stops) once per editor frame.
        let style = editor.base.style();
        let theme = crate::style::resolved_theme_style_shared("code_editor");

        // Explicit style first, then the theme's resolved style for this control, then
        // the light preset's literal. That field order is the documented precedence, so a
        // caller's single override does not have to restate the palette.
        let pick = |explicit: Option<Color>, themed: Option<Color>, literal: Color| -> Color {
            explicit.or(themed).unwrap_or(literal)
        };
        let themed_bg = theme.as_ref().and_then(|t| t.background_color);
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        // The accent is the theme's `primary`: the hue a theme is expected to vary most,
        // which is what makes the caret and the selection follow the appearance. The same
        // lock acquisition reads the raised surface the document is written on: a window
        // background is the *window's* colour, and painting the buffer with it makes the
        // light appearance read as a grey slab (measured in the demo screenshot) while
        // Zed's buffer sits on a lighter card. When the theme distinguishes the two, the
        // container wins; when it does not, there is one surface and it is used as-is.
        let (accent, themed_surface) = crate::style::theme_manager()
            .current_theme()
            .map(|active| {
                (
                    active.colors.primary,
                    (active.colors.surface_container != active.colors.background)
                        .then_some(active.colors.surface_container),
                )
            })
            .unwrap_or((editor.palette.bracket_color, None));

        // A surface the **caller** set wins; a surface the *theme* derived from this
        // control's role does not.
        //
        // The editor's `WidgetKind` is `RichEdit` (the capability layer's choice, see
        // `CodeEditor`'s property contract), so `apply_active_theme` fills its style from
        // the *editable field* role — `input_background()`, a sunken tone that measured
        // `rgb(184,184,184)` on the macOS light preset. Painting the buffer with it made
        // the editor a grey slab inside a `#F5F5F7` window. A code editor is a document
        // surface, not a form field, so a theme-derived background defers to the theme's
        // `surface_container`; a background the caller wrote itself
        // (`theme_derived == false`) is still taken as-is.
        let caller_surface = if style.theme_derived { None } else { style.background_color };
        let surface =
            caller_surface.or(themed_surface).or(themed_bg).unwrap_or(editor.palette.background);
        let ink = pick(style.text_color, themed_ink, editor.palette.color_for(TokenKind::Plain));
        let border = pick(style.border_color, themed_border, editor.palette.border_color);

        // The accent was read with the surface above (one lock acquisition).

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
            minimap_background: surface.blend(&ink, 0.04),
            scrollbar_track: surface.blend(&ink, 0.05),
            scrollbar_thumb: surface.blend(&ink, 0.22),
            separator: surface.blend(&ink, 0.12),
            ink,
            // The secondary ink is used on both the document surface and the chrome bands (tab
            // strip, status bar, find bar), and it has to be readable on whichever it lands on.
            // Blending the ink halfway to the *surface* produced `rgb(155,155,155)` on the dark
            // appearance while the status bar behind it is `rgb(78,78,78)` — 2.99:1, under even
            // the 3:1 large-text floor. Deriving it against the chrome band, which is the
            // lighter of the two backdrops, keeps it secondary while clearing the floor on both.
            dim_ink: ink.legible_on(chrome_background, 4.5).blend(&chrome_background, 0.30),
            accent,
            active_line: surface.blend(&ink, 0.04),
            indent_guide: surface.blend(&ink, 0.1),
            whitespace_mark: surface.blend(&ink, 0.25),
            selection: accent.blend(&surface, 0.7),
            search_hit: warning.blend(&surface, 0.55),
            search_current: warning.blend(&surface, 0.15),
            // A background tint, not an outline: see `draw_line_backdrops`. 0.82 is
            // far enough from the surface to read on a light and a dark preset without
            // being confused with the selection band (`selection` is 0.7 of the same
            // accent, so the two are deliberately one step apart).
            occurrence: accent.blend(&surface, 0.82),
            popup_background: surface.blend(&ink, 0.02),
            popup_border: if border == surface { error } else { border },
            popup_selected: accent.blend(&surface, 0.82),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tabs
// ─────────────────────────────────────────────────────────────────────────────

/// One drawn tab: which buffer it shows, and the two hit regions inside it.
///
/// Produced by [`CodeEditor::draw_tab_strip`] and stored in `tab_hits`, so the
/// pointer handler reads the rectangles the painter actually used. Re-deriving
/// them from a character count was the previous model, and it put the close
/// affordance somewhere the label was not: the tab was laid out in **cells**
/// (`chars × cell_width`) while the title was measured in a proportional face.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TabSlot {
    /// Index into the editor's buffer list.
    pub(crate) buffer: usize,
    /// The whole tab, including its padding.
    pub(crate) rect: Rect,
    /// The close affordance inside the tab; empty when there is nothing to close.
    pub(crate) close: Rect,
}

/// Horizontal padding between a tab's edge and its title.
pub(crate) const TAB_LEFT_PAD: i32 = 10;
/// Slot reserved for the modified dot, so the title never moves when it appears.
pub(crate) const TAB_MARKER_SLOT: i32 = 12;
/// Slot reserved for the close affordance.
pub(crate) const TAB_CLOSE_SLOT: i32 = 24;
/// A tab never grows past this, so one long path cannot take the whole strip.
pub(crate) const TAB_MAX_WIDTH: i32 = 240;
/// A tab never shrinks past this, so a one-character title still looks like a tab.
pub(crate) const TAB_MIN_WIDTH: i32 = 84;

// ─────────────────────────────────────────────────────────────────────────────
// Drawing
// ─────────────────────────────────────────────────────────────────────────────

/// The geometry of one *visual row*: character index → x inside the text area.
///
/// # Why this replaced the `(grid, layout, segment_start)` argument triple
///
/// The painter converted char indices with `x_of(layout, grid, index)`, and the
/// `grid` fallback measured from the line's *first* character. On a wrapped row
/// that meant segment two began at `columns * cell_width` — off the right edge of
/// the text area — so every continuation row except the first painted nothing,
/// and a caret or selection on it pointed somewhere else entirely.
///
/// A row has to know where *it* starts, in characters and in pixels. That is what
/// this type carries, and the painter, the caret and the hit-test all read it, so
/// the three cannot disagree about a wrapped row again.
pub(super) struct RowGeometry<'a> {
    /// Measured boundaries, present when the line is not soft-wrapped.
    pub(super) layout: Option<&'a RowLayout>,
    pub(super) grid: &'a LineGrid<'a>,
    pub(super) cell: f32,
    /// Grid column at which this visual row's first character sits.
    pub(super) first_column: usize,
    /// Pixels a wrapped continuation row is indented by.
    pub(super) indent: f32,
}

impl<'a> RowGeometry<'a> {
    /// x of character `index` inside this row's text area.
    pub(super) fn x_of(&self, index: usize) -> f32 {
        if let Some(x) = self.layout.and_then(|layout| layout.x_of_char(index)) {
            return x;
        }
        let column = self.grid.column_of_char(index).saturating_sub(self.first_column);
        self.indent + column as f32 * self.cell
    }

    pub(super) fn byte_of_char(&self, index: usize) -> usize {
        self.grid.byte_of_char(index)
    }

    pub(super) fn char_of_byte(&self, byte: usize) -> usize {
        self.grid.char_of_byte(byte)
    }
}
