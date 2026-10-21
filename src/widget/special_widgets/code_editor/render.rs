// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Painting for the [`CodeEditor`]: the `Draw` entry point and the module layout.
//!
//! Rendering is a pure function of editor state plus the [`RenderContext`]; it never
//! mutates the document. The paint helpers are split by what they paint (see the
//! `mod` declarations), and every one of them reads its geometry from `metrics` —
//! so the painter and the hit-test in `editor.rs` still share one definition of
//! where a character sits.

use super::editor::{decimal_digits, CodeEditor};
use super::grid::{LineGrid, RowLayout};
use super::types::{
    MarkerSeverity, SearchMatch, SyntaxPalette, TextPosition, TokenKind, MIN_TOUCH_TARGET,
};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::{RenderContext, TEXT_FIT_MARGIN};
use crate::widget::metrics::effective_font;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{Draw, Widget};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

// Paint helpers, split by what they paint. The `Draw` entry point below is the
// only caller; the modules are private.
mod caret;
mod chrome;
mod diagnostics;
mod metrics;
mod minimap_status;
mod panels;
mod popups;
mod rows;

// `editor.rs` stores the tab hit rectangles under `super::render::TabSlot`, and the
// paint helpers share the chrome palette and per-row geometry.
pub(crate) use chrome::TabSlot;
use chrome::{EditorChrome, RowGeometry};

// Shared measurements: the tab and gutter metrics are used by more than one paint
// module, so they are re-exported here rather than repeated in each `use`.
pub(super) use chrome::{
    TAB_CLOSE_SLOT, TAB_LEFT_PAD, TAB_MARKER_SLOT, TAB_MAX_WIDTH, TAB_MIN_WIDTH,
};
pub(super) use metrics::{row_height_of, GUTTER_LEFT_PAD};

impl Draw for CodeEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        // Measure the character cell **first**: every coordinate below — token origins, caret x, the
        // column ruler, hit-testing margins — is `column * cell_width()`, and reading the configured
        // constant instead of the font's real advance is what produced overlapping text.
        self.measure_cell_width(context);
        self.refresh_visible_rows();
        // Lex the visible rows with `&mut self` before the immutable paint helpers
        // run. `tokens_on_line` takes `&self` and cannot fill its own cache, so
        // without this every frame would re-tokenize every visible row — and the
        // per-line entry state would never be advanced past the viewport.
        self.prepare_visible_line_cache();
        self.prepare_minimap();
        // One document-wide search per (selection, revision) pair, not per frame and
        // certainly not per row.
        self.prepare_occurrences();
        // Measured row layouts are keyed by what they were measured from: the text
        // revision and the font size. A change to either makes every cached x a
        // statement about text that is no longer on screen.
        let layout_key = (self.revision(), self.config.font_size.to_bits(), self.geometry().width);
        if self.row_layout_key.get() != layout_key {
            self.row_layouts.borrow_mut().clear();
            self.row_layout_key.set(layout_key);
        }
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
        self.draw_diagnostics(context, &visible, &chrome);
        visible.clear();
        self.draw_scrollbar(context, rect, &chrome);
        self.draw_minimap(context, rect, &chrome);
        self.draw_status_bar(context, rect, &chrome);
        // The IME composition is an overlay on the buffer, drawn after the rows so it
        // sits above their glyphs and before the popups.
        self.draw_preedit(context, rect, &chrome);
        self.draw_completion(context, &chrome);
        self.draw_context_menu(context, &chrome);
    }
}
