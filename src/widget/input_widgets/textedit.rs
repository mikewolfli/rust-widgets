// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Multi-line text edit widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::coercion::{
    expect_horizontal_alignment, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font};
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::widget_trait::InputMethodQuery;
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

/// A styled run of a [`TextEdit`] document.
///
/// The range is a byte range into the value, and the format applies to every character it
/// covers. Spans are stored exactly as the caller gives them: they may overlap (a later span wins
/// where it does) and they need not cover the whole document — anything outside a span keeps the
/// control's own ink and weight.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    /// The byte range the format applies to.
    pub range: core::ops::Range<usize>,
    /// The format painted for that range.
    pub format: CharFormat,
}

/// The character attributes a [`TextSpan`] can carry.
///
/// The default is "inherit the control's own style": no weight change, no italic, no underline
/// and the themed text colour. That keeps an empty span list — which is the initial state —
/// drawing the document exactly as an unstyled editor does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharFormat {
    /// Paint the run in a bold face.
    pub bold: bool,
    /// Paint the run in an italic face.
    pub italic: bool,
    /// Draw a line under the run.
    pub underline: bool,
    /// Override the run's ink; `None` keeps the themed text colour.
    pub color: Option<Color>,
}

impl CharFormat {
    /// Whether this format changes anything about the painted run.
    pub fn is_plain(&self) -> bool {
        !self.bold && !self.italic && !self.underline && self.color.is_none()
    }
}

/// One text edit recorded as a splice rather than a whole-document snapshot (I-P52).
///
/// The old undo entry stored two full copies of the document, so every keystroke in a long
/// document allocated twice its length and the undo stack grew by O(document) per key. A splice
/// stores only the changed run — one character for ordinary typing — so both the entry and the
/// replay are proportional to the edit.
struct TextEditDeltaCommand {
    id: CommandId,
    target: Rc<RefCell<String>>,
    offset: usize,
    removed: String,
    inserted: String,
}

/// Counter for [`TextEditDeltaCommand`] ids.
static NEXT_TEXT_EDIT_COMMAND_ID: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(1);

impl TextEditDeltaCommand {
    /// A command for a mutation whose splice is already known.
    fn from_splice(
        target: Rc<RefCell<String>>,
        offset: usize,
        removed: String,
        inserted: String,
    ) -> Self {
        Self {
            id: CommandId(
                NEXT_TEXT_EDIT_COMMAND_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            ),
            target,
            offset,
            removed,
            inserted,
        }
    }

    /// A command derived from a before/after pair, used by `set_text`.
    ///
    /// Only the differing middle is recorded, so a programmatic replacement that differs by one
    /// character does not push the whole document onto the undo stack either.
    fn from_diff(target: Rc<RefCell<String>>, before: &str, after: &str) -> Self {
        let mut prefix = 0usize;
        for (old, new) in before.chars().zip(after.chars()) {
            if old != new {
                break;
            }
            prefix += old.len_utf8();
        }
        let mut suffix = 0usize;
        for ((_, old), (_, new)) in
            before[prefix..].char_indices().rev().zip(after[prefix..].char_indices().rev())
        {
            if old != new {
                break;
            }
            suffix += old.len_utf8();
        }
        Self::from_splice(
            target,
            prefix,
            before[prefix..before.len() - suffix].to_string(),
            after[prefix..after.len() - suffix].to_string(),
        )
    }

    /// Replaces the recorded range with `insert`.
    fn splice(&self, remove_len: usize, insert: &str) {
        let mut text = self.target.borrow_mut();
        let start = self.offset.min(text.len());
        let end = (start + remove_len).min(text.len());
        text.replace_range(start..end, insert);
    }
}

impl UndoCommand for TextEditDeltaCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit text".to_string(),
            timestamp_ms: 0,
            command_type: "text_edit_text",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        self.splice(self.removed.len(), &self.inserted.clone());
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        self.splice(self.inserted.len(), &self.removed.clone());
        Ok(())
    }
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
    /// Emitted when the selected range changes.
    pub selection_changed: GenericSignal,
    /// Emitted with the new caret byte offset whenever it moves.
    pub cursor_position_changed: Signal1<usize>,
    /// Last caret offset and range announced, so both signals are edge-triggered.
    caret_cursor_announced: usize,
    caret_selection_announced: Option<(usize, usize)>,
    /// The last wrap table, keyed by the inputs the wrapping reads.
    ///
    /// `layout_rows` is called by the paint path **and** by every caret/hit-test answer; before
    /// this cache a single Up key re-measured the whole document several times. The key is the full
    /// text plus the interior width, wrap flag and font — the exact inputs `rows_for` reads — so a
    /// change to any of them re-wraps and nothing else can leave a stale row behind (WDGT-01).
    row_cache: RefCell<Option<RowCache>>,
    /// Styled runs painted over the value, in caller order.
    ///
    /// Empty by default, which keeps the paint path on its original single-run drawing.
    spans: Vec<TextSpan>,
    /// Bumped on every value change, so the row cache can identify the document in O(1).
    revision: u64,
    /// Bumped whenever `spans` change, so the format-run cache invalidates independently of the
    /// text cache (I-P54).
    spans_revision: u64,
    /// Format runs for the current document, keyed by `(revision, spans_revision)` (I-P54).
    format_cache: RefCell<Option<FormatRunCache>>,
    /// The caret's blink state, advanced by [`TextEdit::tick`] (I-119).
    cursor_blink: crate::style::CursorBlink,
    /// Horizontal scroll offset in pixels, for an unwrapped row wider than the field (I-122).
    scroll_x: i32,
}

/// Memoised output of [`TextEdit::layout_rows`].
///
/// The text is identified by the mutation counter rather than stored: comparing the whole
/// document on every read was O(bytes) per keystroke, which is the same order as re-wrapping
/// it. The counter changes only when the value changes, so a hit is an integer comparison and
/// cannot collide.
struct RowCache {
    revision: u64,
    interior_width: u32,
    line_wrap: bool,
    font: Font,
    rows: Rc<Vec<RowSpan>>,
}

/// One contiguous run of a single [`CharFormat`] over the document.
///
/// Built from the span boundaries rather than by probing every character, so the paint path sees
/// one run per format change instead of one `format_at` scan per glyph (I-P54).
struct FormatRunCache {
    revision: u64,
    spans_revision: u64,
    runs: Rc<Vec<(usize, usize, CharFormat)>>,
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
            selection_changed: GenericSignal::new(),
            cursor_position_changed: Signal1::new(),
            caret_cursor_announced: 0,
            caret_selection_announced: None,
            row_cache: RefCell::new(None),
            spans: Vec::new(),
            revision: 0,
            spans_revision: 0,
            format_cache: RefCell::new(None),
            cursor_blink: crate::style::CursorBlink::new(),
            scroll_x: 0,
        }
    }
    /// Returns the current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The styled runs painted over the value.
    pub fn spans(&self) -> &[TextSpan] {
        &self.spans
    }

    /// Replaces the styled runs.
    ///
    /// Ranges are clamped to the current value and ranges that no longer address anything are
    /// dropped, so a caller editing the document cannot leave a span pointing past its end. An
    /// empty list restores the unstyled paint path byte for byte.
    pub fn set_spans(&mut self, spans: Vec<TextSpan>) {
        let len = self.text.len();
        let next: Vec<TextSpan> = spans
            .into_iter()
            .filter_map(|mut span| {
                span.range.start = floor_char_boundary(&self.text, span.range.start.min(len));
                span.range.end = floor_char_boundary(&self.text, span.range.end.min(len));
                (span.range.start < span.range.end).then_some(span)
            })
            .collect();
        if self.spans == next {
            return;
        }
        self.spans = next;
        // The format-run cache keys on this counter as well as the text revision, so a span-only
        // change (no text edit) still invalidates it (I-P54).
        self.spans_revision = self.spans_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The format runs the paint path consumes, merged into contiguous equal formats.
    ///
    /// Built from the span boundaries once per `(revision, spans_revision)` pair. The old painter
    /// called `format_at` — a reverse linear scan of `spans` — for *every character of every
    /// painted row*, which is O(characters × spans) per frame (I-P54).
    fn format_runs(&self) -> Rc<Vec<(usize, usize, CharFormat)>> {
        if let Some(cache) = self.format_cache.borrow().as_ref() {
            if cache.revision == self.revision && cache.spans_revision == self.spans_revision {
                return Rc::clone(&cache.runs);
            }
        }
        let len = self.text.len();
        let mut bounds = vec![0usize, len];
        for span in &self.spans {
            bounds.push(span.range.start.min(len));
            bounds.push(span.range.end.min(len));
        }
        bounds.sort_unstable();
        bounds.dedup();
        let mut runs: Vec<(usize, usize, CharFormat)> = Vec::new();
        for window in bounds.windows(2) {
            let (start, end) = (window[0], window[1]);
            if start >= end {
                continue;
            }
            let format = self
                .spans
                .iter()
                .rev()
                .find(|span| span.range.contains(&start))
                .map_or(CharFormat::default(), |span| span.format);
            if let Some(last) = runs.last_mut() {
                if last.1 == start && last.2 == format {
                    last.1 = end;
                    continue;
                }
            }
            runs.push((start, end, format));
        }
        let runs = Rc::new(runs);
        *self.format_cache.borrow_mut() = Some(FormatRunCache {
            revision: self.revision,
            spans_revision: self.spans_revision,
            runs: Rc::clone(&runs),
        });
        runs
    }

    /// The format that applies at `offset`, taken from the last span covering it.
    pub fn format_at(&self, offset: usize) -> CharFormat {
        self.spans
            .iter()
            .rev()
            .find(|span| span.range.contains(&offset))
            .map_or(CharFormat::default(), |span| span.format)
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
            self.notify_caret_change();
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
            self.notify_caret_change();
            self.base.request_redraw();
        }
    }

    /// Selects the whole value and puts the caret at its end.
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.text.len();
        self.scroll_caret_into_view();
        self.notify_caret_change();
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
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Collapses a zero-width anchor/caret pair to "no selection".
    fn normalize_selection(&mut self) {
        if self.selection_anchor == Some(self.cursor) {
            self.selection_anchor = None;
        }
    }

    /// Reports a caret/selection change to the host when either actually moved.
    ///
    /// Edge-triggered against the value last announced, so every entry point that moves the caret
    /// (keyboard, pointer, edit and history paths) can call this unconditionally.
    fn notify_caret_change(&mut self) {
        let selection = self.selection_range();
        if self.cursor != self.caret_cursor_announced {
            self.caret_cursor_announced = self.cursor;
            self.cursor_position_changed.emit(self.cursor);
        }
        if selection != self.caret_selection_announced {
            self.caret_selection_announced = selection;
            self.selection_changed.emit();
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
        self.notify_caret_change();
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
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Ends a pointer selection, collapsing a gesture that never moved. Returns whether one was in
    /// progress.
    pub fn end_drag(&mut self) -> bool {
        let was_dragging = std::mem::replace(&mut self.dragging_selection, false);
        if was_dragging {
            self.normalize_selection();
            self.notify_caret_change();
        }
        was_dragging
    }

    /// Advances the caret blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate-wide `tick(delta_ms) -> bool` convention, so a host that renders on demand owns
    /// the clock instead of the control reading the wall clock. A blurred editor never asks for a
    /// frame (I-119).
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.focused {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }

    /// The caret's window rectangle and byte offset, as a native IME asks for them (I-120).
    ///
    /// The caret's row comes from [`Self::layout_rows`] — the same table the paint path uses — so
    /// the candidate window is anchored to the row the user sees, including across soft wraps and
    /// after scrolling. The x coordinate is the measured advance of the row prefix, the same model
    /// the painter and the hit test use.
    fn caret_query(&self) -> InputMethodQuery {
        let rect = self.geometry();
        let padding = 4;
        let line_height = self.line_box_height();
        let rows = self.layout_rows();
        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
        let span = rows.get(row).copied().unwrap_or(RowSpan { start: 0, end: 0 });
        let caret = floor_char_boundary(&self.text, self.cursor.min(self.text.len()));
        let prefix = &self.text[span.start..caret.max(span.start).min(self.text.len())];
        let advance = self.text_advance(prefix) as i32;
        let view_row = row.saturating_sub(self.first_visible_row);
        let top = rect.y + padding + view_row as i32 * line_height;
        InputMethodQuery {
            caret_rect: Rect::new(
                rect.x + padding - self.scroll_x + advance,
                top,
                1,
                line_height as u32,
            ),
            caret_byte: Some(caret),
        }
    }

    /// The byte offset one character before `offset`.
    ///
    /// Delegates to the shared [`crate::widget::text_utils::previous_char_boundary`] (WDGT-11).
    fn previous_char_boundary(&self, offset: usize) -> usize {
        crate::widget::text_utils::previous_char_boundary(&self.text, offset)
    }

    /// The byte offset one character after `offset`.
    fn next_char_boundary(&self, offset: usize) -> usize {
        crate::widget::text_utils::next_char_boundary(&self.text, offset)
    }

    /// Inserts `text` at the caret, replacing the selection when there is one.
    ///
    /// # Why this is the one path that inserts
    ///
    /// It replaces the old "push onto the end" behaviour, so typing, Enter and any future paste all
    /// share the max-length gate, the undo snapshot and the caret advance. The candidate is checked
    /// **whole** by `Self::within_max_length`, which is what makes the limit survive a paste (the
    /// gate's own docs record that it used to be read only when the limit itself was set).
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // # The edit is applied in place, never through a candidate copy (I-P52/I-P53/I-P60)
        //
        // The old code cloned the whole value for the candidate and again for the undo snapshot,
        // and the post-edit signal cloned it a third time. Here the only allocation proportional
        // to the *edit* is the removed selection (empty for ordinary typing), and the max-length
        // gate is checked arithmetically before the splice instead of by building a candidate.
        let selection = self.selection_range();
        let removed_len = selection.map_or(0, |(start, end)| end - start);
        if !self.within_max_length_len(self.text.len() - removed_len + text.len()) {
            // Refused **whole**, so the caller knows nothing went in rather than being handed a
            // silently truncated tail (see `set_text`'s docs for why that is the honest answer).
            return;
        }
        let (at, removed) = match selection {
            Some((start, end)) => (start, self.text[start..end].to_string()),
            None => {
                let at = floor_char_boundary(&self.text, self.cursor.min(self.text.len()));
                (at, String::new())
            }
        };
        if removed_len > 0 {
            self.text.replace_range(at..at + removed_len, "");
        }
        self.text.insert_str(at, text);
        // Existing spans move with the splice, so formatting after the insertion point no longer
        // describes the wrong characters (I-P53).
        self.shift_spans(at, removed_len, text.len());
        self.cursor = at + text.len();
        self.selection_anchor = None;
        self.record_splice(at, removed, text.to_string());
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }

    /// Deletes the selection, or the character before the caret. Returns whether anything changed.
    pub fn backspace(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let removed = self.text[start..end].to_string();
            self.text.replace_range(start..end, "");
            self.shift_spans(start, end - start, 0);
            self.cursor = start;
            self.selection_anchor = None;
            self.record_splice(start, removed, String::new());
            self.scroll_caret_into_view();
            self.base.request_redraw();
            return true;
        }
        // A no-op backspace (caret at 0) must not pay for any snapshot (I-P60's twin).
        if self.cursor == 0 {
            return false;
        }
        let start = self.previous_char_boundary(self.cursor);
        let end = floor_char_boundary(&self.text, self.cursor);
        let removed = self.text[start..end].to_string();
        self.text.replace_range(start..end, "");
        self.shift_spans(start, end - start, 0);
        self.cursor = start;
        self.record_splice(start, removed, String::new());
        self.scroll_caret_into_view();
        self.base.request_redraw();
        true
    }

    /// Deletes the selection, or the character after the caret. Returns whether anything changed.
    pub fn delete_forward(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let removed = self.text[start..end].to_string();
            self.text.replace_range(start..end, "");
            self.shift_spans(start, end - start, 0);
            self.cursor = start;
            self.selection_anchor = None;
            self.record_splice(start, removed, String::new());
            self.scroll_caret_into_view();
            self.base.request_redraw();
            return true;
        }
        if self.cursor >= self.text.len() {
            return false;
        }
        let start = floor_char_boundary(&self.text, self.cursor);
        let end = self.next_char_boundary(self.cursor);
        let removed = self.text[start..end].to_string();
        self.text.replace_range(start..end, "");
        self.shift_spans(start, end - start, 0);
        self.record_splice(start, removed, String::new());
        self.scroll_caret_into_view();
        self.base.request_redraw();
        true
    }

    /// Shifts the existing spans across one splice (I-P53).
    ///
    /// Every offset that lies after the removed run moves by `inserted - removed`; an offset
    /// inside the removed run collapses onto the splice point; and a span that the splice empties
    /// is dropped. `set_spans` keeps its own clamp semantics — this is only for edits the user
    /// makes, where the styled text after the caret must stay styled.
    fn shift_spans(&mut self, at: usize, removed: usize, inserted: usize) {
        if self.spans.is_empty() {
            return;
        }
        let end = at + removed;
        let map = |pos: usize| -> usize {
            // A pure insertion shifts everything at or after the splice point; a deletion
            // leaves offsets up to and including the splice point where they are (I-P53).
            if pos < at || (removed > 0 && pos == at) {
                pos
            } else if pos >= end {
                (pos - removed + inserted).max(at)
            } else {
                at
            }
        };
        self.spans = self
            .spans
            .drain(..)
            .filter_map(|mut span| {
                span.range.start = map(span.range.start).min(self.text.len());
                span.range.end = map(span.range.end).min(self.text.len());
                (span.range.start < span.range.end).then_some(span)
            })
            .collect();
        self.spans_revision = self.spans_revision.wrapping_add(1);
    }

    /// Drops spans that no longer address the document after a whole-value replacement.
    fn clamp_spans(&mut self) {
        let len = self.text.len();
        self.spans = self
            .spans
            .drain(..)
            .filter_map(|mut span| {
                span.range.start = floor_char_boundary(&self.text, span.range.start.min(len));
                span.range.end = floor_char_boundary(&self.text, span.range.end.min(len));
                (span.range.start < span.range.end).then_some(span)
            })
            .collect();
        self.spans_revision = self.spans_revision.wrapping_add(1);
    }

    /// Files one splice as an undo step and emits `text_changed` exactly once (I-P52).
    ///
    /// `history_target` is advanced with the same splice rather than replaced by a whole-document
    /// clone, so the undo mirror stays current without a second O(document) allocation. The
    /// revision bump here is what makes the wrap table (I-P51) and the format cache (I-P54) cover
    /// every keyboard/IME edit path, not only `set_text` and undo.
    fn record_splice(&mut self, offset: usize, removed: String, inserted: String) {
        if self.restoring_history {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        {
            let mut target = self.history_target.borrow_mut();
            let start = offset.min(target.len());
            let end = (start + removed.len()).min(target.len());
            target.replace_range(start..end, &inserted);
        }
        self.undo_stack.push(Box::new(TextEditDeltaCommand::from_splice(
            self.history_target.clone(),
            offset,
            removed,
            inserted,
        )));
        self.text_changed.emit(self.text.clone());
        self.notify_caret_change();
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
        self.revision = self.revision.wrapping_add(1);
        self.clamp_spans();
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextEditDeltaCommand::from_diff(
                self.history_target.clone(),
                &before,
                &self.text,
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
        self.notify_caret_change();
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
    pub fn layout_rows(&self) -> Rc<Vec<RowSpan>> {
        let rect = self.geometry();
        let padding = 4;
        let font = self.text_font();
        let interior_width = rect.width.saturating_sub(padding as u32 * 2);
        if let Some(cache) = self.row_cache.borrow().as_ref() {
            if cache.interior_width == interior_width
                && cache.line_wrap == self.line_wrap
                && cache.revision == self.revision
                && cache.font == *font
            {
                // A cache hit used to clone the whole table. Callers only read it, so a
                // refcount bump is both sufficient and free.
                return Rc::clone(&cache.rows);
            }
        }
        // # One measurement surface for the whole document
        //
        // The advance callback used to call `char_advance`, which builds a `SoftwarePaintBackend`
        // **and** a `RenderContext` per character and allocates a `String` for each glyph. A
        // `layout_rows()` of a 10 000-character document therefore built 10 000 measurement
        // backends — and a single Up key built that table four times. The surface is created once
        // here, and the glyph's own UTF-8 bytes are measured straight out of a stack buffer.
        let rows = Rc::new(Self::with_measurement(|context| {
            self.rows_for(interior_width, font, |ch| {
                // One measurement per character, matching the painter — and taken from the
                // **renderer's** own metrics rather than an estimate model.
                //
                // # Why the estimate was not good enough
                //
                // `estimate_text_width` measures `font.size() * 0.6` per character, which is right
                // for neither CJK (roughly double) nor a proportional face (an `i` is a third of a
                // `w`). The row breaks were therefore computed for a font nobody was drawing with,
                // so a row could be declared full before the ink actually reached the interior's
                // edge — or, worse for a hit test, the break could sit at a different character
                // than the one the painter wrapped at. Measuring through `measure_text` makes the
                // table and the ink the same answer.
                let mut glyph = [0u8; 4];
                context.measure_text(ch.encode_utf8(&mut glyph), font).width
            })
        }));
        *self.row_cache.borrow_mut() = Some(RowCache {
            revision: self.revision,
            interior_width,
            line_wrap: self.line_wrap,
            font: font.clone(),
            rows: Rc::clone(&rows),
        });
        rows
    }

    /// The resolved body font the control paints and measures text with.
    ///
    /// Draw and the caret/hit-test helpers must read the same font, so a themed or scaled body font
    /// reaches both the ink and the arithmetic that places the caret and maps a click to an offset.
    fn text_font(&self) -> &Font {
        effective_font(self.style())
    }

    /// The line box the effective font occupies, shared by the paint grid and the hit test.
    fn line_box_height(&self) -> i32 {
        self.text_font().effective_line_height().max(1.0) as i32
    }

    /// Runs `f` against a throwaway measurement surface.
    ///
    /// `measure_text` writes no command, so a zero-sized backend is enough and nothing is
    /// rasterised while a layout is computed. This exists so callers that measure **many** glyphs
    /// (the wrap table, the click-to-offset walk) create one backend instead of one per glyph.
    fn with_measurement<R>(f: impl FnOnce(&RenderContext) -> R) -> R {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        f(&RenderContext::new(&mut measurement))
    }

    /// The advance of `text` under the renderer's own metrics.
    fn text_advance(&self, text: &str) -> u32 {
        Self::with_measurement(|context| context.measure_text(text, self.text_font()).width)
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
        // The byte offset just past the most recent place a word may be split, or `None` when
        // this row has no space or hyphen to break at.
        let mut last_break: Option<usize> = None;
        for (offset, ch) in self.text.char_indices() {
            if ch == '\n' {
                // The newline ends the row and belongs to it, so the next row starts after it. Its
                // own advance is not counted: it paints nothing.
                rows.push(RowSpan { start: row_start, end: offset + 1 });
                row_start = offset + 1;
                row_width = 0.0;
                last_break = None;
                continue;
            }
            let advance = advance_of(ch) as f32;
            if self.line_wrap && row_width + advance > interior_width as f32 && offset > row_start {
                // Break at the last word boundary rather than at an arbitrary character, which is
                // what keeps a word from being split across two rows. A row with no boundary (a
                // single long token) falls back to the old character break so it still fits.
                let break_at = last_break
                    .filter(|boundary| *boundary > row_start && *boundary <= offset)
                    .unwrap_or(offset);
                rows.push(RowSpan { start: row_start, end: break_at });
                row_start = break_at;
                // The characters between the break and the overflow are re-measured so they
                // start the new row at the correct width. Only a handful of glyphs are involved
                // and only on the frame a row actually wraps.
                row_width = 0.0;
                for (_, carried) in self.text[row_start..offset].char_indices() {
                    row_width += advance_of(carried) as f32;
                }
                last_break = None;
            }
            if ch.is_whitespace() || ch == '-' {
                last_break = Some(offset + ch.len_utf8());
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
        Self::row_of_offset_in(&self.layout_rows(), &self.text, offset)
    }

    /// The same answer, read from a wrap table the caller already has.
    ///
    /// Every caller that needs more than one answer about the same document (the arrow keys, the
    /// caret's paint) used to call [`Self::layout_rows`] once per answer, so one Up key laid the
    /// whole document out four times. Taking the table as a parameter is what lets the caller place
    /// it once.
    fn row_of_offset_in(rows: &[RowSpan], text: &str, offset: usize) -> usize {
        let offset = floor_char_boundary(text, offset.min(text.len()));
        rows.iter().position(|row| offset < row.end).unwrap_or_else(|| rows.len().saturating_sub(1))
    }

    /// The first visible row, i.e. how far the viewport is scrolled.
    pub fn first_visible_row(&self) -> usize {
        self.first_visible_row
    }

    /// The top of a laid-out row within the control, in control coordinates.
    fn row_top(&self, row: usize) -> i32 {
        let padding = 4;
        let line_height = self.line_box_height();
        self.geometry().y + padding + row as i32 * line_height
    }

    /// How many whole rows fit in the interior.
    fn visible_row_count(&self) -> usize {
        let padding = 4;
        let line_height = self.line_box_height();
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
        self.first_visible_row = crate::widget::text_utils::first_visible_row_for(
            row,
            self.first_visible_row,
            self.visible_row_count(),
        );
        self.ensure_caret_visible_horizontal();
    }

    /// Scrolls horizontally so the caret stays inside an unwrapped field (I-122).
    ///
    /// With `line_wrap` on there is no overflow to reveal, so the offset is reset. With it off,
    /// the caret's own row prefix is measured and the viewport moves the minimum amount that puts
    /// it in view — the horizontal counterpart of the vertical minimal scroll above.
    fn ensure_caret_visible_horizontal(&mut self) {
        if self.line_wrap {
            self.scroll_x = 0;
            return;
        }
        let padding = 4;
        let width = self.geometry().width.saturating_sub(padding * 2).max(1) as i32;
        let rows = self.layout_rows();
        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
        let span = rows.get(row).copied().unwrap_or(RowSpan { start: 0, end: 0 });
        let caret = floor_char_boundary(&self.text, self.cursor.min(span.end).max(span.start));
        let advance = self.text_advance(&self.text[span.start..caret]) as i32;
        let room = 8.min(width / 4);
        if advance - self.scroll_x > width - room {
            self.scroll_x = (advance - width + room).max(0);
        } else if advance < self.scroll_x {
            self.scroll_x = advance;
        }
        self.scroll_x = self.scroll_x.max(0);
    }

    /// Scrolls horizontally by `columns` characters of the body face.
    fn scroll_columns(&mut self, columns: i32) -> bool {
        if columns == 0 || self.line_wrap {
            return false;
        }
        let padding = 4;
        let width = self.geometry().width.saturating_sub(padding * 2).max(1) as i32;
        let rows = self.layout_rows();
        let widest = rows
            .iter()
            .map(|span| self.text_advance(&self.text[span.start..span.end]) as i32)
            .max()
            .unwrap_or(0);
        let max_scroll = (widest - width + 8).max(0);
        // Positive `columns` means the content moves right, so the viewport moves left.
        let step = self.text_font().size().round() as i32;
        let next = (self.scroll_x - columns.saturating_mul(step)).clamp(0, max_scroll);
        if next == self.scroll_x {
            return false;
        }
        self.scroll_x = next;
        true
    }

    /// Scrolls the viewport by `rows` laid-out rows toward the end of the value.
    ///
    /// The clamp stops at the last window that still fills the interior, so a wheel notch at the
    /// end cannot scroll the document off screen. Returns whether the viewport moved.
    fn scroll_rows(&mut self, rows: i32) -> bool {
        if rows == 0 {
            return false;
        }
        let max_first = self.layout_rows().len().saturating_sub(self.visible_row_count());
        let next =
            (self.first_visible_row as i64 + rows as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_row {
            return false;
        }
        self.first_visible_row = next;
        true
    }

    /// The byte offset the point `pos` falls on.
    ///
    /// # How a pixel becomes an offset
    ///
    /// The row comes from the **wrap table** ([`Self::layout_rows`]) rather than from a second
    /// implementation of the break rule, and within the row the offset is found by measuring the
    /// prefix with the renderer's own measurement (`Self::text_advance`) — the same model the
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
        let line_height = self.line_box_height();
        let origin_x = self.geometry().x + padding;
        let row = ((pos.y - self.row_top(0)).max(0) / line_height) as usize;
        let row = (self.first_visible_row + row).min(rows.len() - 1);
        let span = rows[row];
        self.offset_within_row(span, pos.x - origin_x + self.scroll_x)
    }

    /// The byte offset within `span` that the horizontal coordinate `x` points at.
    fn offset_within_row(&self, span: RowSpan, x: i32) -> usize {
        let target = x.max(0) as u32;
        let line = &self.text[span.start..span.end];
        let font = self.text_font();
        // One measurement surface for the whole row; the per-character form built a backend per
        // glyph, so a click allocated once per character it walked past.
        Self::with_measurement(|context| {
            let mut best = span.start;
            let mut prefix_width = 0u32;
            let mut glyph = [0u8; 4];
            for (index, ch) in line.char_indices() {
                let advance = context.measure_text(ch.encode_utf8(&mut glyph), font).width;
                // The midpoint decides which side of a character's centre the pointer is on, which
                // is what makes clicking the left half of a glyph put the caret before it.
                if target < prefix_width + advance / 2 {
                    break;
                }
                prefix_width += advance;
                best = span.start + index + ch.len_utf8();
            }
            floor_char_boundary(&self.text, best.min(self.text.len()))
        })
    }

    /// The offset in `span` that keeps the caret's **column** when moving between rows.
    ///
    /// This is what makes Up/Down feel right: the column is measured from the caret's own row and
    /// re-applied to the destination, so a vertical move lands under the character it started over.
    /// A destination row shorter than the column saturates at its end, and the row's own newline is
    /// excluded so the caret never lands past the text the user can see.
    /// [`Self::offset_in_row`], reading a wrap table the caller already has.
    fn offset_in_row_with(&self, rows: &[RowSpan], span: RowSpan, from: usize) -> usize {
        let source_row = Self::row_of_offset_in(rows, &self.text, from);
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
        if self.placeholder_text == text {
            return;
        }
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
    /// The gate is now `Self::within_max_length`, which every path that appends or replaces text
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
        self.within_max_length_len(candidate.len())
    }

    /// The length-only form of the gate, so an in-place edit can be refused before allocating a
    /// candidate copy (I-P52).
    fn within_max_length_len(&self, len: usize) -> bool {
        match self.max_length {
            Some(max) => len <= max,
            None => true,
        }
    }
    /// Returns whether the widget is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        if self.read_only == read_only {
            return;
        }
        self.read_only = read_only;
        self.base.request_redraw();
    }
    /// Returns whether line wrap mode is enabled.
    pub fn line_wrap(&self) -> bool {
        self.line_wrap
    }
    /// Sets line wrap mode.
    pub fn set_line_wrap(&mut self, wrap: bool) {
        if self.line_wrap == wrap {
            return;
        }
        self.line_wrap = wrap;
        if wrap {
            self.scroll_x = 0;
        }
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
        self.revision = self.revision.wrapping_add(1);
        self.clamp_spans();
        self.cursor = self.text.len();
        self.selection_anchor = None;
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// The byte range of the word around `offset`, for a double-click selection.
    ///
    /// A word is a run of non-whitespace characters; a press on whitespace selects that run, so
    /// a double click on a gap does something rather than silently collapsing the selection.
    fn word_range_at(&self, offset: usize) -> (usize, usize) {
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        let sampled = self.text[offset..]
            .chars()
            .next()
            .or_else(|| self.text[..offset].chars().next_back())
            .unwrap_or(' ');
        let same_class = |ch: char| ch.is_whitespace() == sampled.is_whitespace();
        let mut start = offset;
        for (index, ch) in self.text[..offset].char_indices().rev() {
            if !same_class(ch) {
                break;
            }
            start = index;
        }
        let mut end = offset;
        for (index, ch) in self.text[offset..].char_indices() {
            if !same_class(ch) {
                break;
            }
            end = offset + index + ch.len_utf8();
        }
        if start == end {
            if let Some((index, ch)) = self.text[..offset].char_indices().next_back() {
                return (index, index + ch.len_utf8());
            }
        }
        (start, end)
    }

    /// Copies the selected text to the platform clipboard.
    ///
    /// Returns the text it placed there, or `None` when there was nothing selected or the
    /// platform build has no clipboard.
    #[cfg(not(alloc_frugal))]
    fn copy_selection_to_clipboard(&self) -> Option<String> {
        let selected = self.selected_text();
        if selected.is_empty() {
            return None;
        }
        crate::set_clipboard_text(&selected);
        Some(selected)
    }

    /// The allocation-frugal build has no clipboard integration.
    #[cfg(alloc_frugal)]
    fn copy_selection_to_clipboard(&self) -> Option<String> {
        None
    }

    /// Inserts the platform clipboard's text at the caret.
    #[cfg(not(alloc_frugal))]
    fn paste_from_clipboard(&mut self) {
        let text = crate::get_clipboard_text();
        if !text.is_empty() {
            self.insert_str(&text);
        }
    }

    /// The allocation-frugal build has no clipboard integration.
    #[cfg(alloc_frugal)]
    fn paste_from_clipboard(&mut self) {}
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

    /// Advances the caret blink and reports whether another frame is owed (I-119).
    fn tick(&mut self, delta_ms: u32) -> bool {
        TextEdit::tick(self, delta_ms)
    }

    /// A focused editor keeps a blinking caret, so the host must keep scheduling frames (I-119).
    fn is_animating(&self) -> bool {
        self.focused
    }

    /// Reports the caret rectangle an IME candidate window anchors to (I-120).
    fn input_method_query(&self) -> Option<InputMethodQuery> {
        Some(self.caret_query())
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
            "selection_changed" => {
                Some(EventSignalRef::unit("selection_changed", &self.selection_changed))
            }
            "cursor_position_changed" => Some(EventSignalRef::mapped(
                "cursor_position_changed",
                &self.cursor_position_changed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
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
                // `Null` is the getter's "no limit" answer (and the schema default), so
                // the setter must accept it — `LineEdit` already does, and a property
                // whose read cannot be written back is a broken round trip.
                CapabilityValue::Null => {
                    self.set_max_length(None);
                    return Ok(());
                }
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
                // I-119: the caret only blinks while the editor owns focus.
                self.cursor_blink.start();
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.cursor_blink.stop();
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
            // A double click selects the word under the pointer.
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                self.focused = true;
                let offset = self.offset_at_point(*pos);
                let (start, end) = self.word_range_at(offset);
                self.selection_anchor = Some(start);
                self.cursor = end;
                self.dragging_selection = false;
                self.notify_caret_change();
                self.base.request_redraw();
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
            // Wheel: positive `delta.y` moves the content down, so the viewport moves toward the
            // start. The shared helper turns wheel notches into lines at the desktop default of
            // three lines per notch.
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                // I-122: the horizontal component is consumed too, so an unwrapped long line is
                // reachable by a horizontal wheel or trackpad gesture.
                let moved =
                    self.scroll_rows(-scroll.lines_y) | self.scroll_columns(scroll.columns_x);
                if moved {
                    self.base.request_redraw();
                }
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

                // I-121: selection and copy stay available on a read-only editor, but undo/redo
                // must not be: the history path is still a mutation.
                if primary && *key == 65 {
                    // Primary+A: select the whole document.
                    self.select_all();
                    return;
                }
                if primary && *key == 67 {
                    // Primary+C: copy the selection. Allowed while read-only, which is the one
                    // edit gesture a read-only editor still owes the user.
                    let _ = self.copy_selection_to_clipboard();
                    return;
                }
                if self.read_only {
                    return;
                }
                if primary && *key == 90 {
                    let _ = self.undo();
                    return;
                }
                if primary && *key == 89 {
                    let _ = self.redo();
                    return;
                }
                if primary && *key == 88 {
                    // Primary+X: copy, then remove the selection.
                    let _ = self.copy_selection_to_clipboard();
                    self.backspace();
                    return;
                }
                if primary && *key == 86 {
                    // Primary+V: paste at the caret, replacing any selection.
                    self.paste_from_clipboard();
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
                    //
                    // The wrap table is built **once** here and passed to both answers. It used to be
                    // rebuilt by `row_of_offset` and again by `offset_in_row` (which itself asked for
                    // `row_of_offset`), so one press laid the whole document out three times.
                    38 => {
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        if row > 0 {
                            let target = self.offset_in_row_with(&rows, rows[row - 1], self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    40 => {
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        if row + 1 < rows.len() {
                            let target = self.offset_in_row_with(&rows, rows[row + 1], self.cursor);
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
                            let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                            rows.get(row).map_or(0, |row| row.start)
                        };
                        self.select_with_modifiers(target, mods);
                    }
                    33 => {
                        // Page Up — one window of rows toward the start, keeping the column.
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        let page = self.visible_row_count().saturating_sub(1).max(1);
                        let target_row = row.saturating_sub(page);
                        let target = rows
                            .get(target_row)
                            .map_or(0, |span| self.offset_in_row_with(&rows, *span, self.cursor));
                        self.select_with_modifiers(target, mods);
                    }
                    34 => {
                        // Page Down — the mirror of Page Up.
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        let page = self.visible_row_count().saturating_sub(1).max(1);
                        let target_row = (row + page).min(rows.len().saturating_sub(1));
                        let target = rows.get(target_row).map_or(self.text.len(), |span| {
                            self.offset_in_row_with(&rows, *span, self.cursor)
                        });
                        self.select_with_modifiers(target, mods);
                    }
                    35 => {
                        // End — the end of the row, or of the document with Primary held.
                        let target = if primary {
                            self.text.len()
                        } else {
                            let rows = self.layout_rows();
                            let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                            rows.get(row).map_or(self.text.len(), |row| {
                                // Exclude the row's own newline, so End is the end of the text the
                                // user sees rather than the start of the next row.
                                let end = row.end.min(self.text.len());
                                if self.text[..end].ends_with('\n') {
                                    end - 1
                                } else {
                                    end
                                }
                            })
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
        // I-122: with wrapping off the text is a window onto a long line; the scroll offset moves
        // the ink, the selection and the caret together.
        let text_x = rect.x + padding - self.scroll_x;
        let text_y = rect.y + padding;

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the field, its border and its text unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        // The fallback name matters: the role table is keyed on **role** names, so `text_edit`
        // (the factory name) is not in it and would classify as `Surface`, i.e. the window fill.
        // `line_edit` is, and resolves to the field interior plus the theme's foreground.
        let theme = crate::style::resolved_theme_style_shared("text_edit")
            .or_else(|| crate::style::resolved_theme_style_shared("line_edit"));
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
        // The wrap table is built **once per frame** and read by the ink, the selection band and
        // the caret. The painter used to re-measure the document glyph by glyph while the markers
        // read the table, so every frame laid the document out twice and the two could disagree
        // about where a row ended. It is empty for the placeholder, which is not the document and
        // therefore has no table of its own.
        let rows = if self.text.is_empty() { Rc::new(Vec::new()) } else { self.layout_rows() };
        if !display_text.is_empty() {
            // The placeholder is de-emphasised from the control's own ink rather than being a
            // fixed grey that a dark theme would render illegible.
            let text_color = if self.text.is_empty() { ink.blend(&field, 0.45) } else { ink };
            let font = self.text_font();
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
            // A long unwrapped row must not paint over the border or its neighbours.
            context.push_clip(
                rect.x + padding,
                rect.y + padding,
                rect.width.saturating_sub(padding as u32 * 2),
                rect.height.saturating_sub(padding as u32 * 2),
            );
            if self.text.is_empty() {
                self.draw_text_layout(context, display_text, interior, font, text_color);
            } else {
                self.draw_rows(context, &rows, interior, font, text_color);
            }
            context.pop_clip();
        }

        // ── The caret and the selection ──
        //
        // Drawn from the same wrap table the text was laid out with, so the marker sits on the row
        // and column the glyphs actually occupy. Measured any other way it would drift under wrapping,
        // which is the whole reason the table exists.
        if !self.text.is_empty() && self.focused && self.cursor_blink.is_visible() {
            let line_height = self.line_box_height();
            let interior_width = rect.width.saturating_sub(padding as u32 * 2);

            // One measurement surface for the markers (I-P54): the selection edges and the caret
            // prefix are measured against it instead of each allocating its own backend.
            Self::with_measurement(|measure| {
                // The selection band first, so the glyphs stay legible on top of it.
                if let Some((sel_start, sel_end)) = self.selection_range() {
                    let selection_fill =
                        crate::style::semantic_color(crate::style::SemanticColor::Info)
                            .map(|accent| accent.blend(&field, 0.72))
                            .unwrap_or_else(|| field.blend(&ink, 0.18));
                    for (index, span) in rows.iter().enumerate().skip(self.first_visible_row) {
                        // The row's top is its position **in the viewport**, which is what the scroll
                        // offset means: the text, the selection and the caret are all drawn from this
                        // one row index.
                        let top = self.row_top(index - self.first_visible_row);
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
                        let left = text_x
                            + measure
                                .measure_text(&self.text[span.start..from], self.text_font())
                                .width as i32;
                        let width =
                            measure.measure_text(&self.text[from..to], self.text_font()).width;
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
                let caret_row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                if caret_row >= self.first_visible_row {
                    let top = self.row_top(caret_row - self.first_visible_row);
                    if top + line_height <= rect.y + rect.height as i32 {
                        let span = rows
                            .get(caret_row)
                            .copied()
                            .unwrap_or(RowSpan { start: self.text.len(), end: self.text.len() });
                        let caret_in_row = floor_char_boundary(
                            &self.text,
                            self.cursor.min(span.end).max(span.start),
                        );
                        let prefix = &self.text[span.start..caret_in_row.min(self.text.len())];
                        let advance = measure.measure_text(prefix, self.text_font()).width as i32;
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
            });
        }
    }
}

impl TextEdit {
    /// Paints the wrap table, starting at the viewport's first visible row.
    ///
    /// # Why the table and not a second walk
    ///
    /// The painter used to re-measure the document glyph by glyph while the caret and the selection
    /// read [`Self::layout_rows`] — two derivations of the same breaks on every frame, one of them
    /// allocating a `String` per character. Reading the table makes the ink a **reader** of the one
    /// wrap rule instead of a second implementation of it.
    ///
    /// It also makes the viewport real: the old walk always began at the interior's top edge, so
    /// `first_visible_row` (which the caret, the hit test and the selection all index from) moved the
    /// markers over text that had not scrolled — and, once the caret's row left the interior, made
    /// the caret disappear while the text stayed. Starting the walk at `first_visible_row` puts the
    /// ink, the band and the caret on one row grid.
    fn draw_rows(
        &self,
        context: &mut RenderContext,
        rows: &[RowSpan],
        interior: Rect,
        font: &Font,
        color: Color,
    ) {
        let line_height = self.line_box_height();
        let align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        let bottom = interior.y + interior.height as i32;
        for (view_row, span) in self.rows_from_viewport(rows) {
            let top = self.row_top(view_row);
            if top + line_height > bottom {
                break;
            }
            // The row's own newline is not ink; it is what the break consumed.
            let end = if self.text[..span.end].ends_with('\n') { span.end - 1 } else { span.end };
            let row = &self.text[span.start..end.max(span.start)];
            if self.spans.is_empty() {
                Self::flush_row(
                    context,
                    row,
                    interior,
                    top,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
            } else {
                self.flush_row_rich(
                    context,
                    span.start,
                    row,
                    interior,
                    top,
                    line_height,
                    bottom,
                    font,
                    color,
                    align,
                );
            }
        }
    }

    /// Paints one row whose characters may carry different [`CharFormat`]s.
    ///
    /// The unstyled path stays in [`Self::flush_row`], so a document with no spans draws exactly
    /// as it did before this method existed. Here the row is split into format runs, each run is
    /// measured with its own face (a bold run is wider than the regular one), and the runs are
    /// laid end to end from an origin chosen by the control's horizontal alignment.
    #[allow(clippy::too_many_arguments)]
    fn flush_row_rich(
        &self,
        context: &mut RenderContext,
        row_start: usize,
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
        // Split the row into contiguous runs of one format from the cached, document-wide run
        // table (I-P54). The old loop called `format_at` — a reverse scan of every span — once per
        // character, and then measured every run twice and cloned its face twice.
        let face = |format: CharFormat| {
            let mut face = font.clone();
            if format.bold {
                face = face.with_bold(true);
            }
            if format.italic {
                face = face.with_italic(true);
            }
            face
        };
        let runs = self.format_runs();
        let row_end = row_start + row.len();
        let mut pieces: Vec<(usize, usize, CharFormat, Font, i32)> = Vec::new();
        let first = runs.partition_point(|(_, end, _)| *end <= row_start);
        for (run_start, run_end, format) in runs.iter().skip(first) {
            if *run_start >= row_end {
                break;
            }
            let start = (*run_start).max(row_start) - row_start;
            let end = (*run_end).min(row_end) - row_start;
            if start >= end {
                continue;
            }
            let run_face = face(*format);
            let advance = context.measure_text(&row[start..end], &run_face).width as i32;
            pieces.push((start, end, *format, run_face, advance));
        }
        let total: i32 = pieces.iter().map(|piece| piece.4).sum();
        let origin = match align {
            HorizontalAlignment::Center => interior.x + (interior.width as i32 - total) / 2,
            HorizontalAlignment::Right => interior.x + interior.width as i32 - total,
            HorizontalAlignment::Left => interior.x,
        };
        let mut pen_x = origin;
        for (start, end, format, face, advance) in pieces {
            let run = &row[start..end];
            let ink = format.color.unwrap_or(color);
            context.draw_text(Point::new(pen_x, pen_y), run, &face, ink, HorizontalAlignment::Left);
            if format.underline {
                let underline_y = pen_y + line_height - 2;
                context.draw_line(
                    Point::new(pen_x, underline_y),
                    Point::new(pen_x + advance, underline_y),
                    ink,
                );
            }
            pen_x += advance;
        }
        pen_y + line_height
    }

    /// The wraps to paint, in view order, as `(view_row, span)`.
    ///
    /// `view_row` is the row's index **from the top of the viewport**, which is what
    /// [`Self::row_top`] and the caret both measure with. The painter stops early at the interior's
    /// bottom; this yields every remaining row, so a caller can ask "what would be painted first"
    /// without a render context.
    ///
    /// It starts at [`Self::first_visible_row`] rather than at zero because the ink and the markers
    /// have to share one row grid — see [`Self::draw_rows`] for the defect that came from the
    /// painter ignoring the scroll offset.
    fn rows_from_viewport<'a>(
        &self,
        rows: &'a [RowSpan],
    ) -> impl Iterator<Item = (usize, &'a RowSpan)> {
        let first = self.first_visible_row;
        rows.iter().enumerate().skip(first).map(move |(index, span)| (index - first, span))
    }

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
    ///
    /// This is the **placeholder's** painter. A real document is painted from the wrap table by
    /// [`Self::draw_rows`], so the ink and the caret share one derivation instead of two.
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
            let advance = {
                // Measured from the glyph's own UTF-8 bytes: `ch.to_string()` allocated a `String`
                // per character, once per painted frame.
                let mut glyph = [0u8; 4];
                context.measure_text(ch.encode_utf8(&mut glyph), font).width
            };
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
        context.draw_text_fitted_cow(
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

    /// The wrap table is memoised and re-wrapped when text, width or wrap mode changes (WDGT-01).
    #[test]
    fn the_row_table_is_cached_and_refreshes_when_an_input_changes() {
        let mut te = TextEdit::new(Rect::new(0, 0, 200, 100));
        te.set_text("hello\nworld");
        let first = te.layout_rows();
        assert_eq!(first.len(), 2);
        assert_eq!(te.layout_rows(), first, "a second read reuses the table");
        assert_eq!(te.row_cache.borrow().as_ref().expect("cache filled").rows, first);

        te.set_text("hello\nworld\nthird");
        assert_eq!(te.layout_rows().len(), 3, "a text change re-wraps");

        te.set_geometry(Rect::new(0, 0, 40, 100));
        let narrow = te.layout_rows();
        assert!(narrow.len() >= 3, "a narrower interior re-wraps: {narrow:?}");

        te.set_line_wrap(false);
        assert_eq!(te.layout_rows().len(), 3, "wrap off is one row per line");
    }

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

    /// The ink follows the scroll offset, on the same row grid as the caret.
    ///
    /// # The defect this pins
    ///
    /// `draw_text_layout` always walked the document from the interior's top edge, while
    /// `first_visible_row` moved the caret, the selection and the hit test. Once the caret's row
    /// scrolled past the interior, the painter still drew the document's **first** rows — and the
    /// caret, drawn from the same offset, fell outside the box and was skipped. The user then typed
    /// into a row that was not on screen, over text that claimed to be the document's start.
    ///
    /// The assertion is on the order the painter consumes: the first row it paints is
    /// `rows[first_visible_row]`, and the caret's own row is among the rows it reaches.
    #[test]
    fn the_painted_rows_follow_the_scroll_offset() {
        let mut te = TextEdit::new(Rect::new(0, 0, 200, 60));
        te.set_text("alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\neta\ntheta".to_string());
        assert_eq!(te.first_visible_row(), 0, "a freshly installed document opens at the top");

        // An edit at the end is a movement the user made, so the view follows the caret.
        te.insert_str("!");
        let scroll = te.first_visible_row();
        assert!(scroll > 0, "the caret's row left the field, so the view scrolled");

        let text = te.text().to_string();
        let rows = te.layout_rows();
        let painted: Vec<RowSpan> = te.rows_from_viewport(&rows).map(|(_, span)| *span).collect();
        assert_eq!(
            painted.first().copied(),
            rows.get(scroll).copied(),
            "the painter starts at the scroll offset, not at the document's start"
        );
        assert!(
            !painted.iter().any(|span| text[span.start..span.end].starts_with("alpha")),
            "the row above the viewport is not painted: {painted:?}"
        );
        assert!(
            painted.iter().any(|span| span.start <= te.cursor() && te.cursor() <= span.end),
            "the caret's own row is among the painted rows"
        );
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

    /// The wheel scrolls the wrapped row window and clamps at both ends.
    #[test]
    fn textedit_wheel_scrolls_and_clamps() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 80));
        te.set_text(text.join("\n"));
        assert_eq!(te.first_visible_row(), 0);

        // One notch toward the end of the value reveals three rows.
        te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        assert_eq!(te.first_visible_row(), 3, "a downward notch moves the viewport down");

        // The opposite notch returns toward the top and clamps there.
        te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(te.first_visible_row(), 0);
        te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(te.first_visible_row(), 0, "the viewport clamps at the first row");

        // Scrolling on stops at the last full window rather than past the document.
        let max_first = te.layout_rows().len() - te.visible_row_count();
        for _ in 0..20 {
            te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        }
        assert_eq!(te.first_visible_row(), max_first, "the viewport clamps at the last window");
    }

    /// Soft wrapping breaks at word boundaries instead of splitting a word.
    #[test]
    fn soft_wrap_breaks_at_word_boundaries() {
        let mut te = TextEdit::new(Rect::new(0, 0, 70, 400));
        te.set_text("alpha beta gamma delta".to_string());
        let text = te.text().to_string();
        let rows = te.layout_rows();
        assert!(rows.len() > 1, "the value has to wrap: {rows:?}");
        for pair in rows.windows(2) {
            let boundary = pair[0].end;
            assert!(
                text[..boundary].ends_with(' ') || boundary == text.len(),
                "a word must not be split at {boundary}: {rows:?}"
            );
        }
    }

    /// Styled spans are addressable and change the painted document.
    #[test]
    fn spans_format_the_named_run() {
        use crate::widget::svg::render_to_svg;
        let mut te = TextEdit::new(Rect::new(0, 0, 200, 100));
        te.set_text("bold and plain".to_string());
        let plain = render_to_svg(&mut te);
        te.set_spans(std::vec![TextSpan {
            range: 0..4,
            format: CharFormat { bold: true, ..CharFormat::default() },
        }]);
        assert!(te.format_at(0).bold);
        assert!(!te.format_at(6).bold);
        assert_ne!(plain, render_to_svg(&mut te), "a bold run must change the ink");
    }

    /// Primary+V pastes the clipboard at the caret.
    #[test]
    fn primary_v_pastes_at_the_caret() {
        let _clipboard = crate::clipboard::clipboard_test_guard();
        crate::set_clipboard_text("pasted");
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("ab".to_string());
        te.set_cursor(1);
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        te.handle_event(&Event::KeyPress { key: 86, modifiers: 0b1000 });
        assert_eq!(te.text(), "apastedb");
    }

    /// A double click selects the word the pointer landed on.
    #[test]
    fn a_double_click_selects_a_word() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 100));
        te.set_text("alpha beta".to_string());
        te.handle_event(&Event::MouseDoubleClick {
            pos: crate::core::Point::new(6, 10),
            button: 1,
        });
        assert_eq!(te.selected_text(), "alpha");
    }

    /// Page Down and Page Up move the caret by a window of rows.
    #[test]
    fn page_keys_move_by_a_window() {
        let mut te = TextEdit::new(Rect::new(0, 0, 200, 80));
        te.set_text((0..30).map(|n| format!("line {n}\n")).collect::<String>());
        te.set_cursor(0);
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        te.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        let after_down = te.cursor();
        assert!(after_down > 0, "Page Down must move the caret");
        te.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert!(te.cursor() < after_down, "Page Up must move it back");
    }

    /// The wrap cache is keyed by the mutation counter, not by the document text.
    #[test]
    fn the_wrap_cache_uses_the_revision_counter() {
        let mut te = TextEdit::new(Rect::new(0, 0, 200, 100));
        te.set_text("one".to_string());
        let first = te.layout_rows();
        let first_revision = te.revision;
        assert_eq!(te.row_cache.borrow().as_ref().expect("cache filled").revision, first_revision);
        assert_eq!(te.layout_rows(), first);
        te.set_text("one two".to_string());
        assert!(te.revision > first_revision, "a text change bumps the revision");
        let _ = te.layout_rows();
        assert_eq!(te.row_cache.borrow().as_ref().expect("cache filled").revision, te.revision);
        assert_ne!(te.layout_rows(), first);
    }

    /// I-P51: a keyboard edit must bump the revision the wrap table is keyed by, or the next
    /// frame reads the pre-edit breaks.
    #[test]
    fn keyboard_edits_refresh_the_wrap_table() {
        let mut te = TextEdit::new(Rect::new(0, 0, 80, 200));
        te.set_text("aaaa aaaa aaaa".to_string());
        te.set_cursor(0);
        let before = te.layout_rows().len();
        let before_revision = te.revision;

        te.insert_str("\n");
        assert!(te.revision > before_revision, "an insertion bumps the revision");
        assert_eq!(
            te.row_cache.borrow().as_ref().expect("cache filled").revision,
            te.revision,
            "the wrap cache records the post-edit revision"
        );
        let after_insert = te.layout_rows().len();
        assert!(after_insert > before, "the newline adds a row: {before} -> {after_insert}");

        assert!(te.backspace());
        assert_eq!(te.layout_rows().len(), before, "deleting the newline removes the row again");
    }

    /// I-P53: formatting after the edit point follows the characters it was applied to.
    #[test]
    fn existing_spans_move_with_an_insertion_and_deletion() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 40));
        te.set_text("hello world".to_string());
        let bold = CharFormat { bold: true, ..CharFormat::default() };
        te.set_spans(vec![TextSpan { range: 6..11, format: bold }]);
        te.set_cursor(0);

        te.insert_str("X");
        assert_eq!(te.spans()[0].range, 7..12, "the span moves past the insertion");
        assert_eq!(te.format_at(7), bold, "the styled run still starts at 'world'");

        assert!(te.backspace(), "the inserted X is removed");
        assert_eq!(te.spans()[0].range, 6..11, "the span returns with the deletion");
        assert_eq!(te.format_at(6), bold);
    }

    /// I-121: undo/redo must not mutate a read-only editor, while select-all and copy still run.
    #[test]
    fn read_only_blocks_undo_but_not_copy() {
        let mut te = editor("abc");
        te.set_cursor(3);
        te.insert_str("d");
        te.set_read_only(true);
        te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });

        te.handle_event(&Event::KeyPress { key: 90, modifiers: PRIMARY_BIT });
        assert_eq!(te.text(), "abcd", "a read-only editor refuses undo");
        te.handle_event(&Event::KeyPress { key: 65, modifiers: PRIMARY_BIT });
        assert_eq!(te.selection_range(), Some((0, 4)), "select-all still works");
    }

    /// I-120: the IME query reports the caret's row and measured column.
    #[test]
    fn input_method_query_reports_the_caret_rectangle() {
        let mut te = TextEdit::new(Rect::new(10, 20, 200, 80));
        te.set_text("abc\ndef".to_string());
        te.set_cursor(5);
        let query = te.input_method_query().expect("an editor has a caret");
        assert_eq!(query.caret_byte, Some(5));
        assert!(query.caret_rect.y >= te.geometry().y, "the query stays inside the field");
        assert!(query.caret_rect.x >= te.geometry().x);
    }
}
