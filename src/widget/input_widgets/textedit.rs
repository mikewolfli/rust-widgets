// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Multi-line text edit widget.
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::coercion::{
    expect_horizontal_alignment, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// One laid-out row of the wrapped document: the byte range of the value it paints.
///
/// The half-open range `start..end` is what makes a row addressable without re-wrapping: the
/// selection band, the caret and the pointer hit test all read a row's own bytes rather than
/// counting characters from the start of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowSpan {
    /// First byte of the value this row paints.
    pub start: usize,
    /// One past the last byte of the value this row paints, including a trailing newline when the
    /// row ends at one.
    pub end: usize,
}

/// Multi-line text edit widget.
pub struct TextEdit {
    base: BaseWidget,
    text: String,
    placeholder_text: String,
    max_length: Option<usize>,
    read_only: bool,
    line_wrap: bool,
    /// The caret, as a **byte** offset into the value.
    ///
    /// # Why this control gained a caret
    ///
    /// It was append-only: there was no insertion point at all, so typing could only add to the end
    /// and `Backspace` was `text.pop()`. A document you can only edit at its end is not an editor, and
    /// the field's own name promises otherwise. The caret defaults to the **end**, so a caller that
    /// only ever appended sees the behaviour it had before.
    cursor: usize,
    /// The end of the selection that is **not** the caret, as a byte offset, or `None`.
    ///
    /// Stored as an anchor rather than an ordered pair so that "which end is the caret" is a fact
    /// rather than an inference — every other text control in this crate arrived at the same shape
    /// after the ordered-pair version lost that information (see `rich_edit`'s `extend_caret`).
    selection_anchor: Option<usize>,
    /// Whether a pointer drag is extending a selection.
    dragging_selection: bool,
    /// Whether this editor currently owns keyboard focus.
    ///
    /// # Why this control needed it too
    ///
    /// The `KeyPress` arm ran for every event, so an editor nobody had clicked into still consumed
    /// the keyboard — with two on a page, one keystroke edited both. The framework does deliver
    /// `FocusGained` / `FocusLost`, so the fact was available; this field is where it is recorded.
    /// Defaults to `false`, and the pointer path focuses on press, which is how a user reaches it.
    focused: bool,
    /// How far down the laid-out rows the view is scrolled, in **rows**.
    ///
    /// Rows rather than lines: this control wraps, so one logical line can be several rows and the
    /// viewport has to move by what is drawn, not by what the newlines say.
    first_visible_row: usize,
    /// How each line of the document is aligned within the editor's interior.
    ///
    /// Horizontal only: the document is laid out as rows down the interior, so a `top`/`bottom`
    /// value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// Emitted after the text changes: on edits, and after an undo/redo restore.
    /// Not emitted when a `set_text` is given the text the widget already holds.
    pub text_changed: Signal1<String>,
}

impl TextEdit {
    /// Creates an empty text edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TextEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            read_only: false,
            line_wrap: true,
            cursor: 0,
            selection_anchor: None,
            dragging_selection: false,
            focused: false,
            first_visible_row: 0,
            alignment: crate::core::Alignment::Left,
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            text_changed: Signal1::new(),
        }
    }
    /// Returns the current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    // ─── The caret and the selection ───

    /// The caret's byte offset into the value.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Places the caret at `offset`, clamped and snapped to a character boundary.
    ///
    /// This is the programmatic "put the caret here", not a movement, so it **drops any selection**.
    /// The `Shift`-aware path a *user* takes is [`Self::select_with_modifiers`], which keeps the
    /// anchor on purpose (the distinction every other text control in the crate now documents).
    pub fn set_cursor(&mut self, offset: usize) {
        let clamped = floor_char_boundary(&self.text, offset.min(self.text.len()));
        if self.cursor != clamped || self.selection_anchor.is_some() {
            self.cursor = clamped;
            self.selection_anchor = None;
            self.scroll_caret_into_view();
            self.base.request_redraw();
        }
    }

    /// The selected range as ordered `(start, end)` byte offsets, or `None` when nothing is selected.
    ///
    /// The single derivation every reader goes through — [`Self::selected_text`], the paint and the
    /// deletion paths — so "is there a selection" and "which bytes are in it" cannot be answered two
    /// different ways.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let caret = floor_char_boundary(&self.text, self.cursor.min(self.text.len()));
        let anchor = floor_char_boundary(&self.text, anchor.min(self.text.len()));
        if anchor == caret {
            return None;
        }
        Some(if anchor < caret { (anchor, caret) } else { (caret, anchor) })
    }

    /// The selected text, or an empty string when nothing is selected.
    pub fn selected_text(&self) -> String {
        match self.selection_range() {
            Some((start, end)) => self.text[start..end].to_string(),
            None => String::new(),
        }
    }

    /// The anchor a `Shift`-extend grows from, if one is set.
    pub fn selection_anchor(&self) -> Option<usize> {
        self.selection_anchor
    }

    /// Drops any selection, leaving the caret where it is.
    pub fn clear_selection(&mut self) {
        if self.selection_anchor.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Selects the whole value and puts the caret at its end.
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.text.len();
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }

    /// Moves the caret to `target` (a byte offset), honouring the modifier keys held.
    ///
    /// # The single entry point for every movement
    ///
    /// Left, Right, Up, Down, Home, End and the pointer all funnel through here, so "what Shift
    /// means" is written once (principle #101):
    ///
    /// * **Shift** — extend: the caret moves to `target` while the anchor stays put. With no anchor
    ///   yet the caret is adopted as one, so the first extension grows from where the gesture began.
    /// * **No modifier** — replace: the caret moves and any selection is dropped.
    ///
    /// The anchor deliberately does **not** move on a later extension, so `Shift+End` after
    /// `Shift+Home` sweeps to the other side of the same anchor instead of re-anchoring.
    pub fn select_with_modifiers(&mut self, target: usize, modifiers: crate::shortcut::Modifiers) {
        let target = floor_char_boundary(&self.text, target.min(self.text.len()));
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
            self.cursor = target;
        } else {
            self.selection_anchor = None;
            self.cursor = target;
        }
        self.normalize_selection();
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }

    /// Collapses a zero-width anchor/caret pair to "no selection".
    fn normalize_selection(&mut self) {
        if self.selection_anchor == Some(self.cursor) {
            self.selection_anchor = None;
        }
    }

    /// Starts a pointer selection at `offset`.
    ///
    /// The anchor is **not** normalised here: this is the start of a gesture and a drag grows from
    /// it, so collapsing a press that landed on the caret would delete the anchor it just set. The
    /// gesture's end normalises instead ([`Self::end_drag`]).
    pub fn press_at(&mut self, offset: usize, modifiers: crate::shortcut::Modifiers) {
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
            self.cursor = offset;
        } else {
            self.selection_anchor = Some(offset);
            self.cursor = offset;
        }
        self.dragging_selection = true;
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }

    /// Extends an in-progress pointer selection to `offset`.
    pub fn drag_to(&mut self, offset: usize) {
        if !self.dragging_selection {
            return;
        }
        self.cursor = floor_char_boundary(&self.text, offset.min(self.text.len()));
        self.normalize_selection();
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }

    /// Ends a pointer selection, collapsing a gesture that never moved. Returns whether one was in
    /// progress.
    pub fn end_drag(&mut self) -> bool {
        let was_dragging = std::mem::replace(&mut self.dragging_selection, false);
        if was_dragging {
            self.normalize_selection();
        }
        was_dragging
    }

    /// Removes the selected range if there is one, returning whether anything was removed.
    ///
    /// Every mutating path funnels through here, so "typing over a selection replaces it" is one
    /// implementation rather than a rule each of `insert_str`, `backspace` and `delete_forward` has
    /// to remember. The caret ends at the range's start and the anchor is dropped, because the
    /// selection it described no longer exists.
    fn replace_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };
        self.text.replace_range(start..end, "");
        self.cursor = start;
        self.selection_anchor = None;
        true
    }

    /// The byte offset one character before `offset`.
    fn previous_char_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let caret = floor_char_boundary(&self.text, offset.min(self.text.len()));
        self.text[..caret].char_indices().next_back().map(|(index, _)| index).unwrap_or(0)
    }

    /// The byte offset one character after `offset`.
    fn next_char_boundary(&self, offset: usize) -> usize {
        if offset >= self.text.len() {
            return self.text.len();
        }
        let caret = floor_char_boundary(&self.text, offset.min(self.text.len()));
        self.text[caret..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| caret + index)
            .unwrap_or(self.text.len())
    }

    /// Inserts `text` at the caret, replacing the selection when there is one.
    ///
    /// # Why this is the one path that inserts
    ///
    /// It replaces the old "push onto the end" behaviour, so typing, Enter and any future paste all
    /// share the max-length gate, the undo snapshot and the caret advance. The candidate is checked
    /// **whole** by [`Self::within_max_length`], which is what makes the limit survive a paste (the
    /// gate's own docs record that it used to be read only when the limit itself was set).
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let before = self.text.clone();
        self.replace_selection();
        let at = floor_char_boundary(&self.text, self.cursor.min(self.text.len()));
        let mut candidate = self.text.clone();
        candidate.insert_str(at, text);
        if !self.within_max_length(&candidate) {
            // Refused **whole**, so the caller knows nothing went in rather than being handed a
            // silently truncated tail (see `set_text`'s docs for why that is the honest answer).
            return;
        }
        self.text = candidate;
        self.cursor = at + text.len();
        self.selection_anchor = None;
        self.record_edit(before);
        self.scroll_caret_into_view();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Deletes the selection, or the character before the caret. Returns whether anything changed.
    pub fn backspace(&mut self) -> bool {
        let before = self.text.clone();
        if self.replace_selection() {
            self.record_edit(before);
            self.scroll_caret_into_view();
            self.text_changed.emit(self.text.clone());
            self.base.request_redraw();
            return true;
        }
        if self.cursor == 0 {
            return false;
        }
        let start = self.previous_char_boundary(self.cursor);
        let end = floor_char_boundary(&self.text, self.cursor);
        self.text.replace_range(start..end, "");
        self.cursor = start;
        self.record_edit(before);
        self.scroll_caret_into_view();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
        true
    }

    /// Deletes the selection, or the character after the caret. Returns whether anything changed.
    pub fn delete_forward(&mut self) -> bool {
        let before = self.text.clone();
        if self.replace_selection() {
            self.record_edit(before);
            self.scroll_caret_into_view();
            self.text_changed.emit(self.text.clone());
            self.base.request_redraw();
            return true;
        }
        if self.cursor >= self.text.len() {
            return false;
        }
        let start = floor_char_boundary(&self.text, self.cursor);
        let end = self.next_char_boundary(self.cursor);
        self.text.replace_range(start..end, "");
        self.record_edit(before);
        self.scroll_caret_into_view();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
        true
    }

    /// Files one undo step and emits `changed`.
    ///
    /// Shared so every mutating path records the same kind of snapshot: a path that skipped it would
    /// make `Primary+Z` jump past the edit, and one that emitted `changed` alone would leave a host
    /// bound to it unaware the value had moved.
    fn record_edit(&mut self, before: String) {
        if self.restoring_history {
            return;
        }
        *self.history_target.borrow_mut() = self.text.clone();
        self.undo_stack.push(Box::new(TextSnapshotCommand::new(
            self.history_target.clone(),
            before,
            self.text.clone(),
            "text_edit_text",
        )));
        self.text_changed.emit(self.text.clone());
    }

    /// How each line of the document is aligned within the editor's interior.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how each line of the document is aligned within the editor's interior.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the document is laid
    /// out as rows down the interior by the editor's own layout — the property route refuses it
    /// through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }
    /// Sets text and emits text_changed signal if different.
    ///
    /// # The one gate every path funnels through
    ///
    /// Typing, pasting, undo/redo and the property route all end here, so the `max_length` check
    /// belongs here rather than in each caller. It used to be in none of them: the limit was read
    /// only when the limit itself was set, so it did not survive a single keystroke.
    ///
    /// An over-long candidate is **refused whole** rather than truncated to fit. Truncating a paste
    /// would silently discard the tail of what the user handed the control, and a user cannot tell a
    /// completed paste from a cut one; refusing it moves the decision back to the caller, which is
    /// where the limit was declared in the first place. The one truncation that does happen is in
    /// [`Self::set_max_length`], where a *lowered* limit must apply to text already in the field and
    /// there is no caller left to refuse.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        if !self.within_max_length(&text) {
            return;
        }
        let before = self.text.clone();
        self.text = text;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "text_edit_text",
            )));
        }
        // The caret moves to the end, which is the position a replacement leaves it at and the one
        // this control used to have implicitly (every edit appended).
        //
        // The viewport goes to the **top** and is deliberately *not* asked to follow the caret: a
        // whole-document replacement is not a movement the user made, and scrolling to the end of a
        // value the caller just installed would open a file at its last page. The caret still follows
        // the text, so an edit made immediately after a `set_text` lands at the end as it always did —
        // which is the guarantee an eviction like "scroll the caret into view" would have quietly
        // broken for the append-only callers this control still has.
        self.cursor = self.text.len();
        self.selection_anchor = None;
        self.first_visible_row = 0;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    // ─── The wrap table: the bridge between a pixel and an offset ───

    /// One laid-out row: the byte range of the value it covers.
    ///
    /// # Why the layout has to be *recorded* rather than re-derived
    ///
    /// This control wraps by measuring each glyph against the interior's width, so which character
    /// starts a row depends on the font, the width and every glyph before it. Turning a click's `y`
    /// into an offset therefore needs the row boundaries — and recomputing them at hit-test time
    /// would be a second implementation of the break rule, free to disagree with the one that
    /// painted. The table is built once per frame by the paint path and read by the pointer path, so
    /// there is one rule and two readers.
    pub fn layout_rows(&self) -> Vec<RowSpan> {
        let rect = self.geometry();
        let padding = 4;
        let font = Font::default();
        let interior_width = rect.width.saturating_sub(padding as u32 * 2);
        self.rows_for(interior_width, &font, |ch| {
            // One measurement per character, matching the painter — and taken from the **renderer's**
            // own metrics rather than an estimate model.
            //
            // # Why the estimate was not good enough
            //
            // `estimate_text_width` measures `font.size() * 0.6` per character, which is right for
            // neither CJK (roughly double) nor a proportional face (an `i` is a third of a `w`).
            // The row breaks were therefore computed for a font nobody was drawing with, so a row
            // could be declared full before the ink actually reached the interior's edge — or, worse
            // for a hit test, the break could sit at a different character than the one the painter
            // wrapped at. Measuring through `measure_text` makes the table and the ink the same
            // answer.
            self.char_advance(ch, &font)
        })
    }

    /// The advance of one character under the renderer's own metrics.
    ///
    /// A throwaway backend is the **measurement surface**: `measure_text` writes no command, so a
    /// zero-sized surface is enough and nothing is rasterised while a layout is computed.
    fn char_advance(&self, ch: char, font: &Font) -> u32 {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = crate::render::RenderContext::new(&mut measurement);
        context.measure_text(&ch.to_string(), font).width
    }

    /// The advance of `text` under the renderer's own metrics.
    fn text_advance(&self, text: &str) -> u32 {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = crate::render::RenderContext::new(&mut measurement);
        context.measure_text(text, &Font::default()).width
    }

    /// The row spans the value lays out into at `interior_width`.
    ///
    /// Split from [`Self::layout_rows`] so it can be exercised without a renderer: the measurement
    /// is a parameter, which makes the break rule testable with exact numbers instead of the font's.
    fn rows_for<F>(&self, interior_width: u32, font: &Font, mut advance_of: F) -> Vec<RowSpan>
    where
        F: FnMut(char) -> u32,
    {
        let _ = font;
        let mut rows = Vec::new();
        let mut row_start = 0usize;
        let mut row_width = 0.0f32;
        for (offset, ch) in self.text.char_indices() {
            if ch == '\n' {
                // The newline ends the row and belongs to it, so the next row starts after it. Its
                // own advance is not counted: it paints nothing.
                rows.push(RowSpan { start: row_start, end: offset + 1 });
                row_start = offset + 1;
                row_width = 0.0;
                continue;
            }
            let advance = advance_of(ch) as f32;
            if self.line_wrap && row_width + advance > interior_width as f32 && offset > row_start {
                rows.push(RowSpan { start: row_start, end: offset });
                row_start = offset;
                row_width = 0.0;
            }
            row_width += advance;
        }
        // The trailing remainder is a row even when empty, so a value ending in a newline still has a
        // row for the empty line after it — where a caret can sit.
        rows.push(RowSpan { start: row_start, end: self.text.len() });
        rows
    }

    /// The row index a caret at `offset` belongs to.
    pub fn row_of_offset(&self, offset: usize) -> usize {
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        let rows = self.layout_rows();
        rows.iter().position(|row| offset < row.end).unwrap_or_else(|| rows.len().saturating_sub(1))
    }

    /// The first visible row, i.e. how far the viewport is scrolled.
    pub fn first_visible_row(&self) -> usize {
        self.first_visible_row
    }

    /// The top of a laid-out row within the control, in control coordinates.
    fn row_top(&self, row: usize) -> i32 {
        let padding = 4;
        let line_height = Font::default().effective_line_height().max(1.0) as i32;
        self.geometry().y + padding + row as i32 * line_height
    }

    /// How many whole rows fit in the interior.
    fn visible_row_count(&self) -> usize {
        let padding = 4;
        let line_height = Font::default().effective_line_height().max(1.0) as i32;
        let usable = self.geometry().height as i32 - padding * 2;
        // At least one, so a control shorter than a single row still shows a row rather than
        // dividing by zero or scrolling past everything.
        ((usable / line_height).max(1)) as usize
    }

    /// Scrolls the viewport the minimum amount that brings the caret's row into view.
    ///
    /// Minimal on purpose: the viewport moves only when the caret is above or below the window it
    /// already shows, so reading near the top does not make the field jump.
    fn scroll_caret_into_view(&mut self) {
        let row = self.row_of_offset(self.cursor);
        let visible = self.visible_row_count();
        if row < self.first_visible_row {
            self.first_visible_row = row;
        } else if row >= self.first_visible_row + visible {
            // Put the caret's row on the last visible one, so the row being typed on is the row the
            // user can see.
            self.first_visible_row = row + 1 - visible;
        }
    }

    /// The byte offset the point `pos` falls on.
    ///
    /// # How a pixel becomes an offset
    ///
    /// The row comes from the **wrap table** ([`Self::layout_rows`]) rather than from a second
    /// implementation of the break rule, and within the row the offset is found by measuring the
    /// prefix with the renderer's own measurement ([`Self::text_advance`]) — the same model the
    /// painter lays the row out with — so the marker cannot drift off the column it belongs to.
    /// model the painter and the caret use. An assumed advance per character would be a fourth copy
    /// of the width model, which is the defect the caret's own history records (`col * 7`).
    ///
    /// Every value returned is a character boundary by construction, because it is only ever taken at
    /// one, and an offset never escapes the value it indexes.
    pub fn offset_at_point(&self, pos: crate::core::Point) -> usize {
        let rows = self.layout_rows();
        if rows.is_empty() {
            return 0;
        }
        let padding = 4;
        let line_height = Font::default().effective_line_height().max(1.0) as i32;
        let origin_x = self.geometry().x + padding;
        let row = ((pos.y - self.row_top(0)).max(0) / line_height) as usize;
        let row = (self.first_visible_row + row).min(rows.len() - 1);
        let span = rows[row];
        self.offset_within_row(span, pos.x - origin_x)
    }

    /// The byte offset within `span` that the horizontal coordinate `x` points at.
    fn offset_within_row(&self, span: RowSpan, x: i32) -> usize {
        let target = x.max(0) as u32;
        let line = &self.text[span.start..span.end];
        let mut best = span.start;
        let mut prefix_width = 0u32;
        for (index, ch) in line.char_indices() {
            let advance = self.char_advance(ch, &Font::default());
            // The midpoint decides which side of a character's centre the pointer is on, which is
            // what makes clicking the left half of a glyph put the caret before it.
            if target < prefix_width + advance / 2 {
                break;
            }
            prefix_width += advance;
            best = span.start + index + ch.len_utf8();
        }
        floor_char_boundary(&self.text, best.min(self.text.len()))
    }

    /// The offset in `span` that keeps the caret's **column** when moving between rows.
    ///
    /// This is what makes Up/Down feel right: the column is measured from the caret's own row and
    /// re-applied to the destination, so a vertical move lands under the character it started over.
    /// A destination row shorter than the column saturates at its end, and the row's own newline is
    /// excluded so the caret never lands past the text the user can see.
    fn offset_in_row(&self, span: RowSpan, from: usize) -> usize {
        let source_row = self.row_of_offset(from);
        let rows = self.layout_rows();
        let source = rows.get(source_row).copied().unwrap_or(RowSpan { start: from, end: from });
        let column = self
            .text_advance(&self.text[source.start..from.min(source.end).max(source.start)])
            as i32;
        let end = if self.text[..span.end].ends_with('\n') { span.end - 1 } else { span.end };
        self.offset_within_row(RowSpan { start: span.start, end }, column)
    }
    /// Returns placeholder text.
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }
    /// Sets placeholder text.
    pub fn set_placeholder_text(&mut self, text: String) {
        self.placeholder_text = text;
        self.base.request_redraw();
    }
    /// Returns maximum text length.
    pub fn max_length(&self) -> Option<usize> {
        self.max_length
    }
    /// Sets maximum text length.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// The field was stored, published (getter, setter, schema row, round-trip test) and read by
    /// **nothing in the edit path**: the truncation below only ran when the *limit itself* changed,
    /// so `set_max_length(Some(10))` on a short field and then typing twenty characters produced a
    /// twenty-character field. A limit that only applies to the value that was already there is not
    /// a limit — it is a one-shot trim with a misleading name.
    ///
    /// The gate is now [`Self::within_max_length`], which every path that appends or replaces text
    /// consults, and this setter keeps its truncation because the *existing* text has to conform too.
    pub fn set_max_length(&mut self, max_length: Option<usize>) {
        self.max_length = max_length;
        // Truncate if needed (using floor_char_boundary to avoid mid-char panic)
        if let Some(max) = max_length {
            if self.text.len() > max {
                let boundary = floor_char_boundary(&self.text, max);
                let truncated = self.text[..boundary].to_string();
                self.set_text(truncated);
            }
        }
    }

    /// Whether `candidate` is short enough to replace the current text.
    ///
    /// # Why a byte count and not a character count
    ///
    /// `max_length` is documented as a *length* and the schema publishes it as `UInt`, and the
    /// truncation already in [`Self::set_max_length`] measures bytes (`self.text.len()`). A gate that
    /// counted characters would let a CJK document through at three bytes per character and then be
    /// shortened by the next call to the setter — two rules for one number, which is how a limit
    /// becomes untrustworthy. Bytes are also what a caller sizing a database column is counting.
    ///
    /// # Why the whole candidate rather than the appended character
    ///
    /// `set_text` is the one place text enters the control (typing, pasting and the property route
    /// all funnel through it), so the check belongs there -- one gate, not one per caller. Checking
    /// only the appended character would let a *paste* through, which is the case a limit most
    /// obviously exists for.
    fn within_max_length(&self, candidate: &str) -> bool {
        match self.max_length {
            Some(max) => candidate.len() <= max,
            None => true,
        }
    }
    /// Returns whether the widget is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.base.request_redraw();
    }
    /// Returns whether line wrap mode is enabled.
    pub fn line_wrap(&self) -> bool {
        self.line_wrap
    }
    /// Sets line wrap mode.
    pub fn set_line_wrap(&mut self, wrap: bool) {
        self.line_wrap = wrap;
        self.base.request_redraw();
    }
    /// Returns number of lines in the text.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        }
    }
    /// Returns text at specified line (0-indexed).
    pub fn line_text(&self, line: usize) -> Option<&str> {
        let mut start = 0;
        let mut current_line = 0;
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                if current_line == line {
                    return Some(&self.text[start..i]);
                }
                start = i + 1;
                current_line += 1;
            }
        }
        if current_line == line {
            Some(&self.text[start..])
        } else {
            None
        }
    }
    /// Appends text to the end.
    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let mut next = self.text.clone();
        next.push_str(text);
        self.set_text(next);
    }
    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }
    /// Returns whether the text edit is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Undo the latest text mutation.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Redo the latest undone text mutation.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is a text mutation to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone text mutation to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        self.restoring_history = true;
        self.text = text;
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }
}
// Implement Widget trait
impl Widget for TextEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        Size::new(dimensions::TEXT_EDIT_DEFAULT_WIDTH, dimensions::TEXT_EDIT_DEFAULT_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `TextEdit`'s property contract.
///
/// `WidgetKind::TextEdit` is the kind the capability layer pairs with the
/// [`TerminalView`](crate::widget::TerminalView) control
/// (`capability::properties::terminal_view_capability`), so the old centralised
/// `TextEdit` arms were answered by `TerminalView`, not by this widget — despite
/// `TEXT_EDIT_PROPERTIES` existing, its names are all marked non-readable and
/// non-writable, so the multi-line editor below never served a property. The
/// contract for the kind lives beside `TerminalView`; this widget publishes none
/// of its own rather than claiming another control's.
impl WidgetProperties for TextEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text.clone())),
            "placeholder_text" => Ok(CapabilityValue::String(self.placeholder_text.clone())),
            "max_length" => match self.max_length {
                // An unset limit is `Null`, not a numeric sentinel. `LineEdit` answers
                // the same question the same way, and the schema declares this property
                // `UInt` — so returning `u64::MAX` for "unset" made the value
                // un-writable: reading a `textedit` with no limit and writing the value
                // back installed a nonsensical cap (or failed outright on a 32-bit
                // target, where `usize::try_from(u64::MAX)` is out of range). A property
                // whose read cannot be written back is a broken round trip.
                Some(limit) => Ok(CapabilityValue::UInt(limit as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "read_only" => Ok(CapabilityValue::Bool(self.read_only)),
            "line_wrap" => Ok(CapabilityValue::Bool(self.line_wrap)),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        // These five are real, working accessors on this control, so the
        // property route must reach them. Forwarding everything to
        // `base_property_set` meant `rw_widget_property_set(id, "text", ..)`
        // answered `UnknownProperty` even though `set_text` worked.
        match name {
            "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_text(text);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "placeholder_text" => match value {
                CapabilityValue::String(text) => {
                    self.set_placeholder_text(text);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "max_length" => match value {
                CapabilityValue::UInt(limit) => {
                    let limit =
                        usize::try_from(limit).map_err(|_| CapabilityAccessError::OutOfRange)?;
                    self.set_max_length(Some(limit));
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "read_only" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_read_only(flag);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "line_wrap" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_line_wrap(flag);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                return Ok(());
            }
            _ => {}
        }
        base_property_set(self, name, value)
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Must list every name `TEXT_EDIT_PROPERTIES` declares, because `get` /
        // `set` below answer all five. Publishing only the shared four while the
        // schema promised these names is the mismatch
        // `schema_and_contract_publish_the_same_names` exists to catch.
        property_names_of![
            "text",
            "placeholder_text",
            "max_length",
            "read_only",
            "line_wrap",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `text_edit` publishes.
    ///
    /// Every name in the set assigns state — the text, the placeholder, the length
    /// limit, the read-only flag, the wrap mode — and each needs a payload, so the whole
    /// set is answered through the property route. The names are placed in
    /// `TEXT_EDIT_PROPERTIES` rather than served by this widget (see the module docs on
    /// why), but the capability still resolves `text_edit` to `TextEdit`, so the refusal
    /// has to live here: returning `UnknownCommand` would make `invoke_command` report
    /// `UnsupportedOnWidget` for names the control does publish.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text"
            | "set_placeholder_text"
            | "set_max_length"
            | "set_read_only"
            | "set_line_wrap" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for TextEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained { .. } => {
                self.focused = true;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.base.request_redraw();
            }
            // A press places the caret and starts a selection. It used to do nothing at all, so the
            // only way to put the caret anywhere was to delete to that point.
            Event::MousePress { pos, button, modifiers, .. } if *button == 1 => {
                // A press focuses the control, which is how the user reaches it with the pointer.
                self.focused = true;
                let offset = self.offset_at_point(*pos);
                self.press_at(offset, crate::shortcut::Modifiers::from_event_bits(*modifiers));
            }
            Event::MouseMove { pos } => {
                if self.dragging_selection {
                    let offset = self.offset_at_point(*pos);
                    self.drag_to(offset);
                }
            }
            // A release anywhere ends the gesture: the anchor stays where the press put it, so the
            // selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.end_drag();
            }
            // Platform-committed text (D09-INPUT-01), routed into the same `insert_str` the
            // `KeyPress` printable arm below calls. This is where a printable character or an IME
            // commit actually arrives from the desktop backends, so without this branch typing did
            // not reach the value. `read_only` is checked here for the same reason it is checked in
            // the key path — a read-only editor must not accept committed text either.
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if !self.focused || self.read_only {
                    return;
                }
                self.insert_str(text);
            }
            Event::KeyPress { key, modifiers } => {
                // An unfocused editor owns no keys — see the `focused` field for what that prevents.
                if !self.focused {
                    return;
                }
                // The event carries the framework's wire bitmask; translate it once, here, so nothing
                // below re-derives a bit.
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                let primary = mods.contains(crate::shortcut::Modifiers::PRIMARY);

                if primary && *key == 90 {
                    let _ = self.undo();
                    return;
                }
                if primary && *key == 89 {
                    let _ = self.redo();
                    return;
                }
                if primary && *key == 65 {
                    // Primary+A: select the whole document.
                    self.select_all();
                    return;
                }
                if self.read_only {
                    return;
                }

                match *key {
                    8 => {
                        self.backspace();
                    }
                    46 => {
                        // Delete — the character *after* the caret. Unhandled before, so it fell
                        // through to the printable arm where key 46 is `.`.
                        self.delete_forward();
                    }
                    13 => {
                        // Enter — a newline at the caret, replacing the selection like any insert.
                        self.insert_str("\n");
                    }
                    37 => {
                        // Left — one character back. At the start the caret cannot move, but a plain
                        // press still owes the user a deselect.
                        if self.cursor > 0 {
                            let target = self.previous_char_boundary(self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    39 => {
                        // Right — one character forward; see the Left arm.
                        if self.cursor < self.text.len() {
                            let target = self.next_char_boundary(self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    // Up/Down move by **laid-out row**, not by logical line: this control wraps, so
                    // the row above is what "up" means to a user looking at the screen.
                    38 => {
                        let rows = self.layout_rows();
                        let row = self.row_of_offset(self.cursor);
                        if row > 0 {
                            let target = self.offset_in_row(rows[row - 1], self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    40 => {
                        let rows = self.layout_rows();
                        let row = self.row_of_offset(self.cursor);
                        if row + 1 < rows.len() {
                            let target = self.offset_in_row(rows[row + 1], self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    36 => {
                        // Home — the start of the *row*, or of the document with Primary held.
                        // Plain Home reaching the document start would make it useless for
                        // returning to the line being edited.
                        let target = if primary {
                            0
                        } else {
                            let rows = self.layout_rows();
                            rows.get(self.row_of_offset(self.cursor)).map_or(0, |row| row.start)
                        };
                        self.select_with_modifiers(target, mods);
                    }
                    35 => {
                        // End — the end of the row, or of the document with Primary held.
                        let target = if primary {
                            self.text.len()
                        } else {
                            let rows = self.layout_rows();
                            rows.get(self.row_of_offset(self.cursor)).map_or(
                                self.text.len(),
                                |row| {
                                    // Exclude the row's own newline, so End is the end of the text the
                                    // user sees rather than the start of the next row.
                                    let end = row.end.min(self.text.len());
                                    if self.text[..end].ends_with('\n') {
                                        end - 1
                                    } else {
                                        end
                                    }
                                },
                            )
                        };
                        self.select_with_modifiers(target, mods);
                    }
                    _ => {
                        // Printable character. A control chord is not text: the catch-all used to
                        // accept every key in the printable range regardless of the modifiers beside
                        // it, so `Primary+B` was typed as a literal `b`.
                        //
                        // The three modifiers are tested one at a time on purpose:
                        // `Modifiers::contains` compares the *value* of the mask, so the combined
                        // `contains(CTRL | ALT | META)` form is true only when all three are held.
                        let chord = mods.contains(crate::shortcut::Modifiers::CTRL)
                            || mods.contains(crate::shortcut::Modifiers::ALT)
                            || mods.contains(crate::shortcut::Modifiers::META);
                        if !chord {
                            if let Some(ch) = char::from_u32(*key).filter(|c| !c.is_control()) {
                                if ch.is_ascii_graphic() || ch == ' ' || ch == '\t' {
                                    self.insert_str(&ch.to_string());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Draw for TextEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding;

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the field, its border and its text unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        // The fallback name matters: the role table is keyed on **role** names, so `text_edit`
        // (the factory name) is not in it and would classify as `Surface`, i.e. the window fill.
        // `line_edit` is, and resolves to the field interior plus the theme's foreground.
        let theme = crate::style::resolved_theme_style("text_edit")
            .or_else(|| crate::style::resolved_theme_style("line_edit"));
        let field_from_theme = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .unwrap_or_else(|| Color::rgb(255, 255, 255));
        // The window fill, read as its own lock acquisition and copied out as a value, so the
        // guard is dropped before anything else touches the theme.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        // The filter is on the **resolved** value, not only on the theme's: the active theme is
        // applied to every control before it is drawn, and a control absent from the role table
        // resolves its background to the window fill itself — so letting that value through
        // unfiltered would paint the field in the window's own colour, which is invisible on
        // screen. A caller's own colour still wins.
        let field = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => field_from_theme,
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The border is one step from the field toward the ink, so it is visible on either
        // appearance rather than being a fixed grey a dark theme would render illegible.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| field.blend(&ink, 0.22));

        // Draw background
        context.face_with_gradient(
            rect,
            field,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Draw border
        context.draw_rect(rect, border);
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.text
        };
        if !display_text.is_empty() {
            // The placeholder is de-emphasised from the control's own ink rather than being a
            // fixed grey that a dark theme would render illegible.
            let text_color = if self.text.is_empty() { ink.blend(&field, 0.45) } else { ink };
            let font = Font::default();
            // `line_wrap` used to be stored, published (`get`/`set`/schema row/round-trip test) and
            // read by nothing: this was a single `draw_text` that ran the whole document off the
            // right edge and past the bottom. It was even commented as a placeholder -- "in real
            // implementation would handle line wrapping" -- which is the shape of a stored promise
            // rather than of a missing line.
            //
            // The value is laid out over the field's *interior*, inset by the same padding the
            // origin uses, so a wrapped line ends where the field ends rather than at the border.
            let interior = Rect::new(
                text_x,
                text_y,
                rect.width.saturating_sub(padding as u32 * 2),
                rect.height.saturating_sub(padding as u32 * 2),
            );
            self.draw_text_layout(context, display_text, interior, &font, text_color);
        }

        // ── The caret and the selection ──
        //
        // Drawn from the same wrap table the text was laid out with, so the marker sits on the row
        // and column the glyphs actually occupy. Measured any other way it would drift under wrapping,
        // which is the whole reason the table exists.
        if !self.text.is_empty() {
            let font = Font::default();
            let rows = self.layout_rows();
            let line_height = font.effective_line_height().max(1.0) as i32;
            let interior_width = rect.width.saturating_sub(padding as u32 * 2);

            // The selection band first, so the glyphs stay legible on top of it.
            if let Some((sel_start, sel_end)) = self.selection_range() {
                let selection_fill =
                    crate::style::semantic_color(crate::style::SemanticColor::Info)
                        .map(|accent| accent.blend(&field, 0.72))
                        .unwrap_or_else(|| field.blend(&ink, 0.18));
                for (index, span) in rows.iter().enumerate() {
                    if index < self.first_visible_row {
                        continue;
                    }
                    let top = self.row_top(index);
                    if top + line_height > rect.y + rect.height as i32 {
                        break;
                    }
                    // The overlap of the selection with this row, so a multi-row range highlights
                    // each row only over its own part rather than as one block.
                    let from = sel_start.max(span.start).min(span.end);
                    let to = sel_end.min(span.end).max(from);
                    if to <= from {
                        continue;
                    }
                    let left = text_x + self.text_advance(&self.text[span.start..from]) as i32;
                    let width = self.text_advance(&self.text[from..to]);
                    // The band is clipped to the interior, so a row wider than the field cannot
                    // paint outside the box it belongs to.
                    let width = width.min(interior_width);
                    if width > 0 {
                        context.fill_rect(
                            Rect::new(left, top, width, line_height as u32),
                            selection_fill,
                        );
                    }
                }
            }

            // Then the caret, at the measured end of its row's prefix. The row it is on is translated
            // through the scroll, so the marker follows the text instead of its document position.
            let caret_row = self.row_of_offset(self.cursor);
            if caret_row >= self.first_visible_row {
                let top = self.row_top(caret_row);
                if top + line_height <= rect.y + rect.height as i32 {
                    let span = rows
                        .get(caret_row)
                        .copied()
                        .unwrap_or(RowSpan { start: self.text.len(), end: self.text.len() });
                    let caret_in_row =
                        floor_char_boundary(&self.text, self.cursor.min(span.end).max(span.start));
                    let prefix = &self.text[span.start..caret_in_row.min(self.text.len())];
                    let advance = self.text_advance(prefix) as i32;
                    let caret = crate::style::semantic_color(crate::style::SemanticColor::Info)
                        .unwrap_or(ink);
                    let x = text_x + advance;
                    context.draw_line(
                        crate::core::Point::new(x, top),
                        crate::core::Point::new(x, top + line_height - 2),
                        caret,
                    );
                }
            }
        }
    }
}

impl TextEdit {
    /// Paints `text` inside `interior`, honouring [`Self::line_wrap`].
    ///
    /// # The two layouts
    ///
    /// * **Wrapped** (`line_wrap: true`, the default): a line that would cross the interior's right
    ///   edge continues on the next row, so the whole document is readable without scrolling
    ///   sideways. This is the mode a document editor wants, and it is what the field's own name
    ///   promises.
    /// * **Unwrapped** (`line_wrap: false`): each `\n`-separated line is drawn in full and a line
    ///   wider than the field is clipped at the border rather than folded. This is the mode a source
    ///   editor wants, where folding a long line makes the indentation invisible.
    ///
    /// # Why the rows are walked manually
    ///
    /// The render context has `draw_text` and a per-`\n` `draw_text_line`, but neither wraps: the
    /// break has to be chosen *by the text*, which needs the font's own advance widths. So the walk
    /// measures glyph by glyph and breaks where the measurement says the row is full -- one
    /// measurement per character, which for a field-sized document is what keeps a long paragraph
    /// from being drawn as one unbounded string.
    fn draw_text_layout(
        &self,
        context: &mut RenderContext,
        text: &str,
        interior: Rect,
        font: &Font,
        color: Color,
    ) {
        let line_height = context.measure_text("M", font).height.max(1) as i32;
        let mut pen_y = interior.y;
        // Each laid-out row is placed by this control's horizontal alignment, so a centred or
        // right-aligned document is positioned within the interior rather than always flush with
        // its leading edge. The rows are laid out as one block below, so the alignment is a
        // property of every row rather than of the run.
        let align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        // A row is skipped rather than clipped when it would cross the interior's bottom: a
        // half-height row of glyphs reads as a rendering error, the same rule the list view's rows
        // follow.
        let bottom = interior.y + interior.height as i32;
        let mut row = String::new();
        let mut row_width = 0.0f32;
        for ch in text.chars() {
            if ch == '\n' {
                pen_y = Self::flush_row(
                    context,
                    &row,
                    interior,
                    pen_y,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
                row.clear();
                row_width = 0.0;
                continue;
            }
            let advance = context.measure_text(&ch.to_string(), font).width;
            // The break is taken *before* the glyph that would overflow, so the last glyph on a row
            // is never the one that crossed the edge. A row is never left empty by the break: a
            // single glyph wider than the interior has to go somewhere, and an empty row followed by
            // the overflowing glyph is worse than a row that is one glyph too wide.
            if self.line_wrap
                && row_width + advance as f32 > interior.width as f32
                && !row.is_empty()
            {
                pen_y = Self::flush_row(
                    context,
                    &row,
                    interior,
                    pen_y,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
                row.clear();
                row_width = 0.0;
            }
            row.push(ch);
            row_width += advance as f32;
        }
        if !row.is_empty() {
            Self::flush_row(
                context,
                &row,
                interior,
                pen_y,
                line_height,
                bottom,
                font,
                color,
                align,
            );
        }
    }

    /// Draws one laid-out row at `pen_y` and returns the next row's origin.
    ///
    /// Returns `pen_y` unchanged when the row would cross `bottom`, so the caller needs no bound of
    /// its own and cannot advance the pen past the interior it was given.
    ///
    /// `align` is the control's own horizontal alignment, applied to the row's `interior` rather
    /// than to a private origin, so a centred or right-aligned row is measured against the same box
    /// the wrap used. `draw_text_fitted` is what turns that alignment into an origin, and it fits
    /// the row into the interior so a run wider than the box is clipped at its trailing edge.
    #[allow(clippy::too_many_arguments)]
    fn flush_row(
        context: &mut RenderContext,
        row: &str,
        interior: Rect,
        pen_y: i32,
        line_height: i32,
        bottom: i32,
        font: &Font,
        color: Color,
        align: HorizontalAlignment,
    ) -> i32 {
        if pen_y + line_height > bottom {
            return pen_y;
        }
        context.draw_text_fitted(
            Rect::new(interior.x, pen_y, interior.width, line_height as u32),
            row,
            font,
            color,
            align,
        );
        pen_y + line_height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn textedit_property_route_reaches_its_own_accessors() {
        // `text_edit` publishes these five names, so the property route must
        // accept them rather than answering `UnknownProperty`.
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        use crate::widget::capability::types::CapabilityValue;

        te.set("text", CapabilityValue::String("hello".to_string()))
            .expect("`text` is a published, writable property");
        assert_eq!(te.text(), "hello");
        assert_eq!(te.get("text").unwrap(), CapabilityValue::String("hello".to_string()));

        te.set("placeholder_text", CapabilityValue::String("type here".to_string()))
            .expect("`placeholder_text` is writable");
        assert_eq!(te.placeholder_text(), "type here");

        te.set("max_length", CapabilityValue::UInt(16)).expect("`max_length` is writable");
        assert_eq!(te.max_length(), Some(16));

        te.set("read_only", CapabilityValue::Bool(true)).expect("`read_only` is writable");
        assert!(te.is_read_only());

        te.set("line_wrap", CapabilityValue::Bool(false)).expect("`line_wrap` is writable");
        assert!(!te.line_wrap());

        // A type mismatch is still reported rather than silently coerced.
        assert!(te.set("read_only", CapabilityValue::UInt(1)).is_err());
    }

    #[test]
    fn textedit_creation_defaults() {
        let te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.text().is_empty());
        assert!(te.placeholder_text().is_empty());
        assert_eq!(te.max_length(), None);
        assert!(!te.is_read_only());
        assert!(te.line_wrap());
        assert!(te.is_empty());
        assert_eq!(te.line_count(), 1);
    }

    #[test]
    fn textedit_set_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Hello World".to_string());
        assert_eq!(te.text(), "Hello World");
        assert!(!te.is_empty());
    }

    #[test]
    fn textedit_set_text_empty() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Some text".to_string());
        te.set_text(String::new());
        assert!(te.text().is_empty());
    }

    #[test]
    fn textedit_placeholder() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.placeholder_text().is_empty());
        te.set_placeholder_text("Enter text here".to_string());
        assert_eq!(te.placeholder_text(), "Enter text here");
        te.set_placeholder_text(String::new());
        assert!(te.placeholder_text().is_empty());
    }

    #[test]
    fn textedit_max_length() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert_eq!(te.max_length(), None);
        te.set_max_length(Some(10));
        assert_eq!(te.max_length(), Some(10));
        te.set_max_length(None);
        assert_eq!(te.max_length(), None);
    }

    #[test]
    fn textedit_max_length_truncates() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Hello World Too Long".to_string());
        te.set_max_length(Some(10));
        assert_eq!(te.text().len(), 10);
    }

    #[test]
    fn textedit_read_only() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(!te.is_read_only());
        te.set_read_only(true);
        assert!(te.is_read_only());
        te.set_read_only(false);
        assert!(!te.is_read_only());
    }

    #[test]
    fn textedit_line_wrap() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.line_wrap());
        te.set_line_wrap(false);
        assert!(!te.line_wrap());
        te.set_line_wrap(true);
        assert!(te.line_wrap());
    }

    #[test]
    fn textedit_line_count() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert_eq!(te.line_count(), 1);
        te.set_text("Line 1\nLine 2\nLine 3".to_string());
        assert_eq!(te.line_count(), 3);
    }

    #[test]
    fn textedit_line_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("First\nSecond\nThird".to_string());
        assert_eq!(te.line_text(0), Some("First"));
        assert_eq!(te.line_text(1), Some("Second"));
        assert_eq!(te.line_text(2), Some("Third"));
        assert_eq!(te.line_text(5), None);
    }

    #[test]
    fn textedit_append() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.append("Hello");
        assert_eq!(te.text(), "Hello");
        te.append(" World");
        assert_eq!(te.text(), "Hello World");
    }

    #[test]
    fn textedit_undo_redo_restores_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("one");
        te.set_text("two");
        assert!(te.undo());
        assert_eq!(te.text(), "one");
        assert!(te.redo());
        assert_eq!(te.text(), "two");
    }

    #[test]
    fn textedit_control_z_and_control_y_drive_history() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        te.set_text("before");
        te.set_text("after");
        // Bit 3 is the event's Meta/Command bit, i.e. the portable primary accelerator
        // (`Modifiers::from_event_bits` folds it into both PRIMARY and CTRL). The physical-Control
        // bit is bit 1.
        //
        // This test used to pass with `modifiers: 2` for a different reason: the catch-all arm typed
        // every unmapped printable key, so Ctrl+Z wrote a literal `Z` — which *also* changed the
        // value, and the assertion only looked at the text. Fixing the chord guard exposed it.
        te.handle_event(&Event::key_press(90, 0b1000));
        assert_eq!(te.text(), "before");
        te.handle_event(&Event::key_press(89, 0b1000));
        assert_eq!(te.text(), "after");
    }

    #[test]
    fn textedit_clear() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Some text".to_string());
        te.clear();
        assert!(te.is_empty());
    }

    #[test]
    fn textedit_geometry_delegation() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_geometry(Rect::new(10, 10, 400, 300));
        assert_eq!(te.geometry(), Rect::new(10, 10, 400, 300));
    }

    #[test]
    fn textedit_visibility() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.is_visible());
        te.hide();
        assert!(!te.is_visible());
        te.show();
        assert!(te.is_visible());
    }

    #[test]
    fn textedit_enabled() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.is_enabled());
        te.set_enabled(false);
        assert!(!te.is_enabled());
        te.set_enabled(true);
        assert!(te.is_enabled());
    }

    #[test]
    fn textedit_id_kind() {
        let te_a = TextEdit::new(Rect::new(0, 0, 100, 100));
        let te_b = TextEdit::new(Rect::new(0, 0, 100, 100));
        assert_ne!(te_a.id(), te_b.id());
        assert_eq!(te_a.kind(), WidgetKind::TextEdit);
        assert_eq!(te_b.kind(), WidgetKind::TextEdit);
    }

    #[test]
    fn textedit_signal_accessors() {
        let te = TextEdit::new(Rect::new(0, 0, 100, 100));
        let _ = &te.text_changed;
    }

    // ── `max_length` bounds every path into the field ──

    /// The limit is enforced on **typing**, not only on the value that was already there.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `max_length` was stored, published (getter, setter, schema row, round-trip test) and read by
    /// nothing in the edit path: the truncation only ran when the *limit itself* changed. So
    /// `set_max_length(Some(10))` on a short field followed by typing twenty characters produced a
    /// twenty-character field, and the property's reader could not tell — it answered `Some(10)`
    /// either way. This drives the actual keystrokes through `handle_event`, which is the path a
    /// user takes.
    #[test]
    fn max_length_bounds_typing_not_only_the_existing_value() {
        use crate::event::Event;
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        // The keyboard is gated on focus; a user reaches an editor by clicking it.
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        te.set_max_length(Some(5));
        for ch in ['a', 'b', 'c', 'd', 'e', 'f', 'g'] {
            te.handle_event(&Event::KeyPress { key: ch as u32, modifiers: 0 });
        }
        assert_eq!(te.text(), "abcde", "the field stopped at the limit, and did not lose a prefix");
        assert_eq!(te.text().len(), 5);

        // A paste is the case a limit most obviously exists for, so it is refused wholesale rather
        // than silently cut: a user cannot tell a completed paste from a truncated one.
        let before = te.text().to_string();
        te.set_text("a much longer pasted string");
        assert_eq!(te.text(), before, "an over-long paste is refused, not truncated");

        // Deleting still works, so a full field is not a stuck one.
        te.handle_event(&Event::KeyPress { key: 8, modifiers: 0 });
        assert_eq!(te.text(), "abcd");
        // And the field is usable again now that there is room.
        te.handle_event(&Event::KeyPress { key: 'z' as u32, modifiers: 0 });
        assert_eq!(te.text(), "abcdz");

        // Raising the limit re-opens the field; clearing it removes the bound entirely.
        te.set_max_length(Some(10));
        te.set_text("0123456789");
        assert_eq!(te.text(), "0123456789");
        te.set_max_length(None);
        te.set_text("a string of any length at all");
        assert_eq!(te.text(), "a string of any length at all");

        // Lowering the limit truncates the text already in the field, on a char boundary.
        let mut unicode = TextEdit::new(Rect::new(0, 0, 300, 200));
        unicode.set_text("你好世界");
        unicode.set_max_length(Some(7));
        assert_eq!(unicode.text(), "你好", "a multi-byte truncation lands on a boundary");
    }

    // ── `line_wrap` decides the layout of the value ──

    /// Wrapping folds a long line onto the next row; not wrapping keeps one row and clips.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `line_wrap` was stored, published (`get`/`set`/schema row/round-trip test) and read by
    /// nothing. `draw` was a single `draw_text` of the whole document, commented in place as "in
    /// real implementation would handle line wrapping" — a stored promise. This asserts the two
    /// settings paint different pictures *and* that the difference is the wrapping: with wrapping on
    /// the value occupies several rows, with it off exactly one.
    #[test]
    fn line_wrap_folds_the_value_and_its_absence_does_not() {
        use crate::widget::svg::{render_to_svg, text_ink_boxes};

        let build = |wrap: bool| {
            let mut te = TextEdit::new(Rect::new(0, 0, 60, 200));
            te.set_line_wrap(wrap);
            te.set_text("the quick brown fox jumps over the lazy dog");
            te
        };

        let wrapped = render_to_svg(&mut build(true));
        let unwrapped = render_to_svg(&mut build(false));
        assert_ne!(
            wrapped, unwrapped,
            "the two settings must not render identically -- that was the dead state"
        );

        // The question `line_wrap` answers is "how many rows does the value occupy", so that is what
        // is asserted -- a bare "the SVG differs" would pass on a one-pixel shift. The layout emits
        // one `draw_text` per row and one glyph-geometry `<path>` per `draw_text`, so the row count
        // is the number of text ink boxes.
        let wrapped_rows = text_ink_boxes(&wrapped).len();
        let unwrapped_rows = text_ink_boxes(&unwrapped).len();
        assert!(
            wrapped_rows > unwrapped_rows,
            "wrapping must use more rows: {wrapped_rows} vs {unwrapped_rows}"
        );
        // Not wrapping is exactly one row of text, however long the line is.
        assert_eq!(unwrapped_rows, 1, "an unwrapped value stays on one row");

        // An explicit newline breaks on either setting, because it is the text's own decision rather
        // than the layout's.
        let mut explicit = TextEdit::new(Rect::new(0, 0, 300, 200));
        explicit.set_line_wrap(false);
        explicit.set_text("first\nsecond");
        assert_eq!(
            text_ink_boxes(&render_to_svg(&mut explicit)).len(),
            2,
            "an explicit newline is a break regardless of `line_wrap`"
        );

        // A value taller than the field is clipped rather than drawn outside it: the row count is
        // bounded by the interior's height (30 px less the padding, over an 8 px line).
        let mut overflowing = TextEdit::new(Rect::new(0, 0, 40, 30));
        overflowing.set_text("word ".repeat(40));
        let rows = text_ink_boxes(&render_to_svg(&mut overflowing)).len();
        assert!(rows <= 3, "a small field draws only the rows it has room for, not {rows} rows");
    }

    // ─── The caret (the capability this control did not have) ───

    /// The framework's Shift bit on an event mask.
    const SHIFT_BIT: u32 = 0b0001;
    /// The framework's primary-accelerator bit on an event mask.
    const PRIMARY_BIT: u32 = 0b1000;

    fn editor(text: &str) -> TextEdit {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text(text.to_string());
        // Focused, because the keyboard is gated on it — a user reaches an editor by clicking it, and
        // a fixture that skipped that would be testing a state no user can be in. The gate itself is
        // covered by `an_unfocused_editor_ignores_the_keyboard`.
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        // `set_text` leaves the caret at the end, which is the position this control used to have
        // implicitly; every test below that cares about the caret places it explicitly.
        te
    }

    // ─── D09-INPUT-01: platform-committed text reaches the value ───

    /// A `TextInput` from the platform must enter the editor's value.
    ///
    /// # The defect this pins (D09-INPUT-01)
    ///
    /// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
    /// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
    /// test feeds `TextInput` to prove that path itself is wired.
    #[test]
    fn text_input_enters_the_value() {
        let mut te = editor("");
        te.handle_event(&Event::TextInput { text: "héllo".to_string() });
        assert_eq!(te.text(), "héllo");
    }

    /// An IME commit enters the value the same way.
    #[test]
    fn ime_commit_enters_the_value() {
        let mut te = editor("");
        te.handle_event(&Event::ime_commit("你好"));
        assert_eq!(te.text(), "你好");
    }

    /// A `TextInput` replaces a selection rather than appending to it.
    #[test]
    fn text_input_replaces_the_selection() {
        let mut te = editor("abcdef");
        te.select_all();
        te.handle_event(&Event::TextInput { text: "Z".to_string() });
        assert_eq!(te.text(), "Z", "the committed text replaced the selected range");
    }

    /// Committed text respects `max_length`.
    #[test]
    fn text_input_respects_the_max_length() {
        let mut te = editor("");
        te.set_max_length(Some(3));
        te.handle_event(&Event::TextInput { text: "abcdef".to_string() });
        assert!(te.text().len() <= 3, "the limit applies: got {:?}", te.text());
    }

    /// A read-only editor ignores committed text, just as it ignores a printable key.
    #[test]
    fn read_only_ignores_text_input() {
        let mut te = editor("locked");
        te.set_read_only(true);
        te.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(te.text(), "locked");
    }

    /// An unfocused editor ignores committed text, matching the key gate.
    #[test]
    fn unfocused_ignores_text_input() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(te.text(), "");
    }

    /// Typing mid-document inserts **at the caret** instead of at the end.
    ///
    /// # The defect this pins
    ///
    /// The value was append-only: every printable key ran `next.push(ch)` and then `set_text`, so
    /// there was no insertion point at all. Correcting one character in the middle of a document was
    /// impossible — the only edit available was to delete everything after it and retype. This is the
    /// difference between a text editor and a text sink, and the control's own name promises the
    /// former.
    #[test]
    fn typing_inserts_at_the_caret_not_at_the_end() {
        let mut te = editor("ac");
        te.set_cursor(1);
        te.handle_event(&Event::key_press(98, 0)); // 'b'
        assert_eq!(te.text(), "abc", "the character went between `a` and `c`");
        assert_eq!(te.cursor(), 2, "and the caret advanced past it");
    }

    /// The caret defaults to the **end**, so a caller that only appended sees no change.
    ///
    /// This is the compatibility half of adding a caret: the control's previous contract was
    /// "typing appends", and it still is whenever the caret has not been moved.
    #[test]
    fn typing_with_an_untouched_caret_still_appends() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        assert_eq!(te.cursor(), 0);
        te.handle_event(&Event::key_press(97, 0)); // 'a'
        te.handle_event(&Event::key_press(98, 0)); // 'b'
        assert_eq!(te.text(), "ab", "an empty field's typing lands where it always did");
        assert_eq!(te.cursor(), 2);
    }

    /// Backspace deletes **behind the caret**, not the last character of the document.
    ///
    /// # The defect this pins
    ///
    /// `Backspace` was `text.pop()`, which removes the final character wherever the user was looking.
    /// With the caret in the middle it deleted the wrong character entirely — the end of the document
    /// rather than the one behind the cursor.
    #[test]
    fn backspace_deletes_behind_the_caret() {
        let mut te = editor("abcd");
        te.set_cursor(2);
        te.handle_event(&Event::key_press(8, 0));
        assert_eq!(te.text(), "acd", "the `b` behind the caret went, not the `d`");
        assert_eq!(te.cursor(), 1);
    }

    /// Delete removes the character **after** the caret.
    ///
    /// # The defect this pins
    ///
    /// `Delete` was not handled, so key 46 fell through to the printable arm where 46 is `.` —
    /// pressing Delete typed a full stop. The one key whose entire job is forward deletion was the one
    /// key that inserted text (the same defect `textarea` had).
    #[test]
    fn delete_removes_the_character_after_the_caret() {
        let mut te = editor("abc");
        te.set_cursor(1);
        te.handle_event(&Event::key_press(46, 0));
        assert_eq!(te.text(), "ac", "the `b` ahead of the caret went");
        assert_eq!(te.cursor(), 1, "and the caret stayed put");
    }

    // ─── Selection ───

    /// A Shift-arrow extends; a plain arrow replaces.
    #[test]
    fn a_shift_arrow_extends_and_a_plain_arrow_replaces() {
        let mut te = editor("abcdef");
        te.set_cursor(4);

        te.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(te.selected_text(), "d", "Shift+Left selects the character behind");
        te.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(te.selected_text(), "cd", "the second extends the same range");

        te.handle_event(&Event::key_press(37, 0));
        assert_eq!(te.selection_anchor(), None, "a plain arrow is not an extend");
        assert_eq!(te.selected_text(), "");
    }

    /// Typing over a selection replaces it, and the whole thing is **one** undo step.
    ///
    /// # Why the undo count is asserted
    ///
    /// The obvious implementation routes each keystroke through its own snapshot, so replacing a
    /// selection costs two presses to undo (the insert, then the deletion). Recording one step per
    /// user action is what makes `Primary+Z` mean "take back what I just did".
    #[test]
    fn typing_over_a_selection_replaces_it_in_one_undo_step() {
        let mut te = editor("hello world");
        te.set_cursor(5);
        for _ in 0..5 {
            te.handle_event(&Event::key_press(37, SHIFT_BIT));
        }
        assert_eq!(te.selected_text(), "hello");

        te.handle_event(&Event::key_press(88, 0)); // 'X'
        assert_eq!(te.text(), "X world", "the selection was consumed");
        assert!(te.undo(), "one press");
        assert_eq!(te.text(), "hello world", "one undo restores the whole operation");
    }

    /// Home and End are row bounds, and Primary reaches the document.
    #[test]
    fn home_and_end_are_row_bounds_and_primary_reaches_the_document() {
        let mut te = editor("one\ntwo\nthree");
        te.set_cursor(6); // inside the second line
        te.handle_event(&Event::key_press(36, 0));
        assert_eq!(te.cursor(), 4, "plain Home is the start of this row");
        te.handle_event(&Event::key_press(35, 0));
        assert_eq!(te.cursor(), 7, "plain End is the end of this row, before its newline");
        te.handle_event(&Event::key_press(36, PRIMARY_BIT));
        assert_eq!(te.cursor(), 0, "Primary+Home is the start of the document");
        te.handle_event(&Event::key_press(35, PRIMARY_BIT));
        assert_eq!(te.cursor(), 13, "Primary+End is the end of the document");
    }

    /// Primary+A selects the whole document.
    #[test]
    fn primary_a_selects_the_whole_document() {
        let mut te = editor("one\ntwo");
        te.set_cursor(0);
        te.handle_event(&Event::key_press(65, PRIMARY_BIT));
        assert_eq!(te.selected_text(), "one\ntwo");
    }

    /// A control chord is not text.
    #[test]
    fn a_control_chord_is_not_typed_into_the_editor() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        for (name, bits) in [("Ctrl", 0b0010u32), ("Alt", 0b0100), ("Primary", PRIMARY_BIT)] {
            te.handle_event(&Event::key_press(98, bits));
            assert!(te.text().is_empty(), "{name}+b must not be typed");
        }
        te.handle_event(&Event::key_press(98, 0));
        assert_eq!(te.text(), "b", "a plain `b` is content");
    }

    // ─── The wrap table ───

    /// A newline starts a new row, and the row includes its own terminator.
    #[test]
    fn rows_follow_the_newlines() {
        let te = editor("ab\ncd");
        let rows = te.layout_rows();
        assert_eq!(rows.len(), 2, "two logical lines, two rows when wrapping is off");
        assert_eq!((rows[0].start, rows[0].end), (0, 3), "the first row owns its newline");
        assert_eq!((rows[1].start, rows[1].end), (3, 5));
    }

    /// A value ending in a newline still has a final row, so the caret can sit on the empty line.
    #[test]
    fn a_trailing_newline_leaves_a_final_row() {
        let te = editor("ab\n");
        let rows = te.layout_rows();
        assert_eq!(rows.len(), 2, "the empty line after the newline is a row");
        assert_eq!(rows[1].start, 3);
        assert_eq!(rows[1].end, 3, "and it is empty");
    }

    /// Wrapping splits a long line into rows that **together cover the whole value**.
    ///
    /// This is the invariant that makes the table safe to use for hit testing: no byte may fall
    /// outside every row, or a click past a break would resolve to the wrong offset.
    #[test]
    fn wrapped_rows_tile_the_whole_value() {
        let mut te = TextEdit::new(Rect::new(0, 0, 60, 200));
        te.set_text("aaaa bbbb cccc dddd eeee ffff".to_string());
        assert!(te.line_wrap(), "the fixture must be in wrapping mode");
        let rows = te.layout_rows();
        assert!(rows.len() > 1, "a 60 px field must wrap this value into several rows");
        assert_eq!(rows[0].start, 0, "the first row starts at the beginning");
        assert_eq!(rows[rows.len() - 1].end, te.text().len(), "the last row reaches the end");
        for pair in rows.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "rows are contiguous, with no gap or overlap");
        }
    }

    /// With wrapping off, one row per logical line, however wide.
    #[test]
    fn line_wrap_off_gives_one_row_per_line() {
        let mut te = TextEdit::new(Rect::new(0, 0, 60, 200));
        te.set_text("aaaa bbbb cccc dddd\neeee ffff".to_string());
        te.set_line_wrap(false);
        let rows = te.layout_rows();
        assert_eq!(rows.len(), 2, "no wrapping, so the two lines are the two rows");
    }

    /// The row for an offset is the row that contains it.
    #[test]
    fn an_offset_maps_to_the_row_containing_it() {
        let te = editor("ab\ncd");
        assert_eq!(te.row_of_offset(0), 0, "`a` is on the first row");
        assert_eq!(te.row_of_offset(2), 0, "the newline belongs to the first row");
        assert_eq!(te.row_of_offset(3), 1, "`c` is on the second row");
        assert_eq!(te.row_of_offset(5), 1, "and so is the end of the value");
    }

    // ─── The pointer ───

    /// A press places the caret, and the map is total on multi-byte wrapped text.
    #[test]
    fn a_press_places_the_caret_and_never_splits_a_character() {
        let mut te = TextEdit::new(Rect::new(0, 0, 120, 200));
        te.set_text("héllo 中文 wörld".to_string());
        let rect = te.geometry();

        for dy in (0..rect.height as i32).step_by(11) {
            for dx in (0..rect.width as i32).step_by(7) {
                te.handle_event(&Event::mouse_press(rect.x + dx, rect.y + dy, 1));
                let caret = te.cursor();
                assert!(caret <= te.text().len(), "the caret is inside the value at ({dx},{dy})");
                assert!(
                    te.text().is_char_boundary(caret),
                    "the caret at ({dx},{dy}) split a character: {caret} in {:?}",
                    te.text()
                );
            }
        }
    }

    /// A drag extends the selection; a hover that was never pressed does not move the caret.
    #[test]
    fn a_drag_extends_the_selection_but_a_bare_hover_does_not() {
        let mut te = editor("abcdef");
        let rect = te.geometry();

        te.handle_event(&Event::mouse_move(rect.x + 40, rect.y + 10));
        assert_eq!(te.cursor(), 6, "an unpressed hover must not move the caret");
        assert_eq!(te.selection_anchor(), None);

        te.handle_event(&Event::mouse_press(rect.x + 10, rect.y + 10, 1));
        let (start, _) = te.selection_range().unwrap_or((te.cursor(), 0));
        te.handle_event(&Event::mouse_move(rect.x + 44, rect.y + 10));
        let (_, end) = te.selection_range().unwrap_or((0, te.cursor()));
        assert!(end > start, "the drag grew the range: {start} -> {end}");

        te.handle_event(&Event::mouse_release(rect.x + 44, rect.y + 10, 1));
        let kept = te.selected_text();
        te.handle_event(&Event::mouse_move(rect.x + 4, rect.y + 10));
        assert_eq!(te.selected_text(), kept, "a hover after the release does not extend");
    }

    /// A click on the first row and a click on the second resolve to different rows.
    #[test]
    fn a_press_on_a_later_row_resolves_to_that_row() {
        let te = editor("ab\ncd");
        let rect = te.geometry();
        let line_height = Font::default().effective_line_height().max(1.0) as i32;

        let first = te.offset_at_point(crate::core::Point::new(rect.x + 6, rect.y + 6));
        let second =
            te.offset_at_point(crate::core::Point::new(rect.x + 6, rect.y + 6 + line_height));
        assert!(first < 3, "the top row is the first line, got {first}");
        assert!(second >= 3, "the row below is the second line, got {second}");
    }

    // ─── Scrolling ───

    /// Typing past the last visible row scrolls the viewport instead of writing out of sight.
    #[test]
    fn the_viewport_follows_the_caret_down() {
        let mut te = TextEdit::new(Rect::new(0, 0, 200, 60));
        te.set_text("one\ntwo\nthree\nfour\nfive\nsix".to_string());
        // `set_text` installs a whole document, so the view opens at its **top** even though the
        // caret goes to the end (see that method's note): opening a file at its last page would be
        // the wrong reading position for a replacement the user did not make.
        assert_eq!(te.first_visible_row(), 0, "a freshly installed document opens at the top");

        // An *edit* at the end, which is a movement the user made, does scroll the view.
        te.insert_str("!");
        assert!(
            te.first_visible_row() > 0,
            "editing at the last row scrolled the viewport, got {}",
            te.first_visible_row()
        );

        // And back to the start: the viewport returns, minimally.
        te.set_cursor(0);
        assert_eq!(te.first_visible_row(), 0, "returning to the first row shows the first row");
    }

    /// A value that fits never scrolls.
    #[test]
    fn a_value_that_fits_never_scrolls() {
        let mut te = editor("one\ntwo");
        te.select_all();
        assert_eq!(te.first_visible_row(), 0, "nothing to scroll to");
    }

    // ─── Undo ───

    /// A deletion through the caret is undoable, and restores the caret with the text.
    #[test]
    fn a_caret_deletion_is_undoable() {
        let mut te = editor("abcd");
        te.set_cursor(2);
        te.handle_event(&Event::key_press(8, 0));
        assert_eq!(te.text(), "acd");
        assert!(te.undo());
        assert_eq!(te.text(), "abcd", "the deleted character came back");
    }

    /// `max_length` bounds a mid-document insert too, and refuses it whole.
    #[test]
    fn the_limit_bounds_an_insert_at_the_caret() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        te.set_max_length(Some(3));
        te.set_text("abc".to_string());
        te.set_cursor(1);
        te.insert_str("z");
        assert_eq!(te.text(), "abc", "the insert was refused, not truncated");
    }

    /// An unfocused editor ignores the keyboard, and a focused one accepts it.
    ///
    /// # The defect this pins
    ///
    /// The `KeyPress` arm ran for **every** event, so an editor nobody had clicked into still consumed
    /// the keyboard. With two editors on a page one keystroke edited both, and a control the user had
    /// never reached swallowed accelerators the host meant for something else. The framework does
    /// deliver `FocusGained` / `FocusLost`, so the fact was available — the control simply never
    /// checked it.
    #[test]
    fn an_unfocused_editor_ignores_the_keyboard() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.handle_event(&Event::key_press(97, 0)); // 'a'
        assert!(te.text().is_empty(), "an unfocused editor must not take the keystroke");

        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        te.handle_event(&Event::key_press(97, 0));
        assert_eq!(te.text(), "a", "once focused it does");

        te.handle_event(&Event::FocusLost);
        te.handle_event(&Event::key_press(98, 0)); // 'b'
        assert_eq!(te.text(), "a", "and losing focus takes the keyboard away again");
    }

    /// A press focuses the editor, which is how a user reaches it with the pointer.
    #[test]
    fn a_press_focuses_the_editor() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        let rect = te.geometry();
        te.handle_event(&Event::mouse_press(rect.x + 6, rect.y + 6, 1));
        te.handle_event(&Event::key_press(97, 0)); // 'a'
        assert_eq!(te.text(), "a", "the press made the editor own the keyboard");
    }
}
