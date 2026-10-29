// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`TextEdit`]'s document model: the value, the caret, the selection, the undo history and
//! the wrap table every reader of the document goes through.
//!
//! The wrap table built here is the single break rule: the paint path, the caret, the
//! selection band and the pointer hit test all read it rather than deriving the breaks again.

// The only users are the COMP-A8 find/replace binding below: the shared queue is an
// `Arc<Mutex<..>>` so the bar and the editor can both reach it.
#[cfg(full_widgets)]
use crate::compat::{lock, Arc, Mutex};
use crate::core::{Color, Font, Point, Rect};
use crate::render::RenderContext;
#[cfg(full_widgets)]
use crate::signal::ConnectionScope;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
#[cfg(full_widgets)]
use crate::widget::companions::FindReplaceBar;
use crate::widget::metrics::effective_font;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::widget_trait::InputMethodQuery;
use crate::widget::{BaseWidget, Widget, WidgetKind};
use std::cell::RefCell;
use std::collections::HashMap;
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
    pub(crate) base: BaseWidget,
    pub(crate) text: String,
    pub(crate) placeholder_text: String,
    pub(crate) max_length: Option<usize>,
    pub(crate) read_only: bool,
    pub(crate) line_wrap: bool,
    /// The caret, as a **byte** offset into the value.
    ///
    /// # Why this control gained a caret
    ///
    /// It was append-only: there was no insertion point at all, so typing could only add to the end
    /// and `Backspace` was `text.pop()`. A document you can only edit at its end is not an editor, and
    /// the field's own name promises otherwise. The caret defaults to the **end**, so a caller that
    /// only ever appended sees the behaviour it had before.
    pub(crate) cursor: usize,
    /// The end of the selection that is **not** the caret, as a byte offset, or `None`.
    ///
    /// Stored as an anchor rather than an ordered pair so that "which end is the caret" is a fact
    /// rather than an inference — every other text control in this crate arrived at the same shape
    /// after the ordered-pair version lost that information (see `rich_edit`'s `extend_caret`).
    pub(crate) selection_anchor: Option<usize>,
    /// Whether a pointer drag is extending a selection.
    pub(crate) dragging_selection: bool,
    /// Whether this editor currently owns keyboard focus.
    ///
    /// # Why this control needed it too
    ///
    /// The `KeyPress` arm ran for every event, so an editor nobody had clicked into still consumed
    /// the keyboard — with two on a page, one keystroke edited both. The framework does deliver
    /// `FocusGained` / `FocusLost`, so the fact was available; this field is where it is recorded.
    /// Defaults to `false`, and the pointer path focuses on press, which is how a user reaches it.
    pub(crate) focused: bool,
    /// How far down the laid-out rows the view is scrolled, in **rows**.
    ///
    /// Rows rather than lines: this control wraps, so one logical line can be several rows and the
    /// viewport has to move by what is drawn, not by what the newlines say.
    pub(crate) first_visible_row: usize,
    /// How each line of the document is aligned within the editor's interior.
    ///
    /// Horizontal only: the document is laid out as rows down the interior, so a `top`/`bottom`
    /// value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    pub(crate) alignment: crate::core::Alignment,
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
    /// Emitted with the new value whenever undo availability changes.
    pub undo_available: Signal1<bool>,
    /// Emitted with the new value whenever redo availability changes.
    pub redo_available: Signal1<bool>,
    /// Emitted when a context menu is requested at this point in the document.
    ///
    /// The editor does not build a menu: the host owns the cut/copy/paste implementation and
    /// decides which items apply. The payload is the request position in window coordinates,
    /// matching [`TextArea::context_menu_requested`](super::textarea::TextArea).
    pub context_menu_requested: Signal1<Point>,
    /// Last undo/redo availability announced, so both signals are edge-triggered.
    undo_available_announced: bool,
    redo_available_announced: bool,
    /// Last caret offset and range announced, so both signals are edge-triggered.
    caret_cursor_announced: usize,
    caret_selection_announced: Option<(usize, usize)>,
    /// The last wrap table, keyed by the inputs the wrapping reads.
    ///
    /// `layout_rows` is called by the paint path **and** by every caret/hit-test answer; before
    /// this cache a single Up key re-measured the whole document several times. The key is the full
    /// text plus the interior width, wrap flag and font — the exact inputs `rows_for` reads — so a
    /// change to any of them re-wraps and nothing else can leave a stale row behind (WDGT-01).
    pub(crate) row_cache: RefCell<Option<RowCache>>,
    /// Per-line wrap results, reused when a paragraph is unchanged (I-006).
    wrapped_lines: RefCell<Vec<TextLineWrapCache>>,
    /// Whether the pointer is dragging the scrollbar thumb (I-015).
    pub(crate) scrollbar_drag: bool,
    /// Styled runs painted over the value, in caller order.
    ///
    /// Empty by default, which keeps the paint path on its original single-run drawing.
    pub(crate) spans: Vec<TextSpan>,
    /// Bumped on every value change, so the row cache can identify the document in O(1).
    pub(crate) revision: u64,
    /// Bumped whenever `spans` change, so the format-run cache invalidates independently of the
    /// text cache (I-P54).
    pub(crate) spans_revision: u64,
    /// Format runs for the current document, keyed by `(revision, spans_revision)` (I-P54).
    pub(crate) format_cache: RefCell<Option<FormatRunCache>>,
    /// Per-row prefix advances for the selection band and the caret (IP-001).
    pub(crate) marker_advance_cache: RefCell<Option<MarkerAdvanceCache>>,
    /// The caret's blink state, advanced by [`TextEdit::tick`] (I-119).
    pub(crate) cursor_blink: crate::style::CursorBlink,
    /// Horizontal scroll offset in pixels, for an unwrapped row wider than the field (I-122).
    pub(crate) scroll_x: i32,
    #[cfg(full_widgets)]
    /// The `FindReplaceBar` this editor follows, when one is attached (COMP-A8).
    find_replace: Option<FindReplaceBinding>,
}

/// Memoised output of [`TextEdit::layout_rows`].
///
/// The text is identified by the mutation counter rather than stored: comparing the whole
/// document on every read was O(bytes) per keystroke, which is the same order as re-wrapping
/// it. The counter changes only when the value changes, so a hit is an integer comparison and
/// cannot collide.
pub(crate) struct RowCache {
    pub(crate) revision: u64,
    interior_width: u32,
    line_wrap: bool,
    font: Font,
    pub(crate) rows: Rc<Vec<RowSpan>>,
}

/// One logical line's wrapped rows, relative to the line start, keyed by its content (I-006).
struct TextLineWrapCache {
    hash: u64,
    width: u32,
    font: Font,
    rows: Vec<(usize, usize)>,
}

/// FNV-1a over one line's bytes, for the wrap-cache key.
fn text_line_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Memoised per-row prefix widths for the selection band and the caret (IP-001).
///
/// The draw pass measures each visible row's selected prefix and selected run. The document is
/// unchanged between frames while a selection is being dragged, so those measurements are cached
/// under the text revision, the selection and the font. A new revision or a moved selection drops
/// the table rather than serving widths for a different document.
pub(crate) struct MarkerAdvanceCache {
    pub(crate) revision: u64,
    pub(crate) selection: Option<(usize, usize)>,
    pub(crate) font: Font,
    pub(crate) entries: HashMap<(usize, usize), u32>,
}

/// One contiguous run of a single [`CharFormat`] over the document.
///
/// Built from the span boundaries rather than by probing every character, so the paint path sees
/// one run per format change instead of one `format_at` scan per glyph (I-P54).
pub(crate) struct FormatRunCache {
    pub(crate) revision: u64,
    pub(crate) spans_revision: u64,
    pub(crate) runs: Rc<Vec<(usize, usize, CharFormat)>>,
}

/// One intent reported by an attached [`FindReplaceBar`] (COMP-A8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(full_widgets)]
enum FindReplaceIntent {
    /// Move to the next match.
    Next,
    /// Move to the previous match.
    Previous,
    /// Replace the current match.
    ReplaceOne,
    /// Replace every match.
    ReplaceAll,
}

/// The live link between a [`TextEdit`] and a [`FindReplaceBar`] (COMP-A8).
///
/// The bar reports intents with unit signals, so the link parks one intent per emission and
/// [`TextEdit::poll_find_replace_bar`] applies them. The query, replacement and match policy are
/// read from the bar's shared context at apply time, and the match counters are written back
/// through the bar's shared cell so the strip can show "3 of 7" without the host forwarding it.
#[cfg(full_widgets)]
struct FindReplaceBinding {
    /// The intents reported since the last poll.
    intents: Arc<Mutex<Vec<FindReplaceIntent>>>,
    /// `(query, replacement, match_case, use_regex)` as last written by the bar.
    context: Arc<Mutex<(String, String, bool, bool)>>,
    /// `(total, current)` match counters written back to the bar.
    matches: Arc<Mutex<(usize, usize)>>,
    /// The bar's paint request, emitted after a counter write.
    redraw: GenericSignal,
    /// The subscriptions, dropped with the binding so a detach leaves no slot behind.
    _scope: ConnectionScope,
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
            undo_available: Signal1::new(),
            redo_available: Signal1::new(),
            context_menu_requested: Signal1::new(),
            undo_available_announced: false,
            redo_available_announced: false,
            caret_cursor_announced: 0,
            caret_selection_announced: None,
            row_cache: RefCell::new(None),
            wrapped_lines: RefCell::new(Vec::new()),
            scrollbar_drag: false,
            spans: Vec::new(),
            revision: 0,
            spans_revision: 0,
            format_cache: RefCell::new(None),
            marker_advance_cache: RefCell::new(None),
            cursor_blink: crate::style::CursorBlink::new(),
            scroll_x: 0,
            #[cfg(full_widgets)]
            find_replace: None,
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
    pub(crate) fn notify_caret_change(&mut self) {
        let selection = self.selection_range();
        if self.cursor != self.caret_cursor_announced {
            self.caret_cursor_announced = self.cursor;
            self.cursor_position_changed.emit(self.cursor);
        }
        if selection != self.caret_selection_announced {
            self.caret_selection_announced = selection;
            self.selection_changed.emit();
        }
        self.refresh_edit_capabilities();
    }

    /// Emits the undo/redo availability signals when their answers have changed.
    ///
    /// The same edge-triggered pattern `TextArea` uses: a host menu can enable or disable its
    /// Undo/Redo items from the signal, and a caret move that does not change history stays quiet.
    fn refresh_edit_capabilities(&mut self) {
        let undo = self.can_undo();
        if undo != self.undo_available_announced {
            self.undo_available_announced = undo;
            self.undo_available.emit(undo);
        }
        let redo = self.can_redo();
        if redo != self.redo_available_announced {
            self.redo_available_announced = redo;
            self.redo_available.emit(redo);
        }
    }

    /// A primary press at `pos`: grabs the scrollbar when the point is on it, otherwise focuses
    /// the editor and places the caret / anchors the selection.
    ///
    /// Shared by the mouse and touch press arms, so a finger enters exactly the state machine a
    /// pointer does instead of a second copy of it.
    pub(crate) fn press_with_pointer(&mut self, pos: crate::core::Point, modifiers: u32) {
        // A press on the scrollbar grabs its thumb instead of placing the caret (I-015).
        if self.scrollbar_contains(pos) {
            self.scrollbar_drag = true;
            self.scrollbar_scroll_to(pos.y);
            self.base.request_redraw();
            return;
        }
        // A press focuses the control, which is how the user reaches it with the pointer.
        self.focused = true;
        let offset = self.offset_at_point(pos);
        self.press_at(offset, crate::shortcut::Modifiers::from_event_bits(modifiers));
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
    pub(crate) fn caret_query(&self) -> InputMethodQuery {
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
    pub(crate) fn previous_char_boundary(&self, offset: usize) -> usize {
        crate::widget::text_utils::previous_char_boundary(&self.text, offset)
    }

    /// The byte offset one character after `offset`.
    pub(crate) fn next_char_boundary(&self, offset: usize) -> usize {
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
    pub(crate) fn text_font(&self) -> &Font {
        effective_font(self.style())
    }

    /// The line box the effective font occupies, shared by the paint grid and the hit test.
    pub(crate) fn line_box_height(&self) -> i32 {
        self.text_font().effective_line_height().max(1.0) as i32
    }

    /// Runs `f` against a throwaway measurement surface.
    ///
    /// `measure_text` writes no command, so a zero-sized backend is enough and nothing is
    /// rasterised while a layout is computed. This exists so callers that measure **many** glyphs
    /// (the wrap table, the click-to-offset walk) create one backend instead of one per glyph.
    pub(crate) fn with_measurement<R>(f: impl FnOnce(&RenderContext) -> R) -> R {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        f(&RenderContext::new(&mut measurement))
    }

    /// The advance of `text` under the renderer's own metrics.
    pub(crate) fn text_advance(&self, text: &str) -> u32 {
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
        let mut rows = Vec::new();
        // A paragraph's breaks depend only on its own text, the width and the font, so an
        // unchanged paragraph reuses its last rows instead of being measured again (I-006). With
        // wrapping off no measurement is needed at all: one row per line.
        let mut cache = self.wrapped_lines.borrow_mut();
        let line_count = self.text.bytes().filter(|byte| *byte == b'\n').count() + 1;
        cache.resize_with(line_count, || TextLineWrapCache {
            hash: 0,
            width: 0,
            font: Font::default(),
            rows: Vec::new(),
        });
        let mut line_start = 0usize;
        let mut line_index = 0usize;
        for line in self.text.split('\n') {
            let line_end = line_start + line.len();
            let row_end = if line_end < self.text.len() { line_end + 1 } else { line_end };
            if !self.line_wrap || interior_width == 0 {
                rows.push(RowSpan { start: line_start, end: row_end });
                line_start = line_end + 1;
                line_index += 1;
                continue;
            }
            let hash = text_line_hash(line);
            let reusable = {
                let entry = &cache[line_index];
                entry.hash == hash && entry.width == interior_width && entry.font == *font
            };
            if reusable {
                for &(relative_start, relative_end) in &cache[line_index].rows {
                    rows.push(RowSpan {
                        start: line_start + relative_start,
                        end: line_start + relative_end,
                    });
                }
                line_start = line_end + 1;
                line_index += 1;
                continue;
            }
            let mut line_rows = Vec::new();
            let mut row_start = 0usize;
            let mut row_width = 0.0f32;
            let mut last_break: Option<usize> = None;
            for (offset, ch) in line.char_indices() {
                let advance = advance_of(ch) as f32;
                if row_width + advance > interior_width as f32 && offset > row_start {
                    let break_at = last_break
                        .filter(|boundary| *boundary > row_start && *boundary <= offset)
                        .unwrap_or(offset);
                    line_rows.push((row_start, break_at));
                    row_start = break_at;
                    row_width = 0.0;
                    for (_, carried) in line[row_start..offset].char_indices() {
                        row_width += advance_of(carried) as f32;
                    }
                    last_break = None;
                }
                if ch.is_whitespace() || ch == '-' {
                    last_break = Some(offset + ch.len_utf8());
                }
                row_width += advance;
            }
            line_rows.push((row_start, row_end - line_start));
            for &(relative_start, relative_end) in &line_rows {
                rows.push(RowSpan {
                    start: line_start + relative_start,
                    end: line_start + relative_end,
                });
            }
            let entry = &mut cache[line_index];
            entry.hash = hash;
            entry.width = interior_width;
            entry.font = font.clone();
            entry.rows = line_rows;
            line_start = line_end + 1;
            line_index += 1;
        }
        cache.truncate(line_count);
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
    pub(crate) fn row_of_offset_in(rows: &[RowSpan], text: &str, offset: usize) -> usize {
        let offset = floor_char_boundary(text, offset.min(text.len()));
        rows.iter().position(|row| offset < row.end).unwrap_or_else(|| rows.len().saturating_sub(1))
    }

    /// The first visible row, i.e. how far the viewport is scrolled.
    pub fn first_visible_row(&self) -> usize {
        self.first_visible_row
    }

    /// The top of a laid-out row within the control, in control coordinates.
    pub(crate) fn row_top(&self, row: usize) -> i32 {
        let padding = 4;
        let line_height = self.line_box_height();
        self.geometry().y + padding + row as i32 * line_height
    }

    /// How many whole rows fit in the interior.
    pub(crate) fn visible_row_count(&self) -> usize {
        let padding = 4;
        let line_height = self.line_box_height();
        let usable = self.geometry().height as i32 - padding * 2;
        // At least one, so a control shorter than a single row still shows a row rather than
        // dividing by zero or scrolling past everything.
        ((usable / line_height).max(1)) as usize
    }

    /// The scrollbar's track rectangle, or `None` when the value fits the viewport (I-015).
    ///
    /// Same geometry and colours as the text area's scrollbar, so the two multi-line editors
    /// present scrolling the same way rather than one of them being reachable only by the wheel.
    pub(crate) fn scrollbar_track(&self) -> Option<(Rect, usize, usize)> {
        let total = self.layout_rows().len();
        let visible = self.visible_row_count();
        if total <= visible {
            return None;
        }
        let rect = self.geometry();
        let width = 4u32;
        let track = Rect::new(
            rect.x + rect.width.saturating_sub(width + 2) as i32,
            rect.y.saturating_add(2),
            width,
            rect.height.saturating_sub(4),
        );
        Some((track, total, visible))
    }

    /// The thumb rectangle within `track`, proportional to the visible fraction.
    pub(crate) fn scrollbar_thumb(&self, track: Rect, total: usize, visible: usize) -> Rect {
        let height = ((visible as f32 / total as f32) * track.height as f32)
            .max(8.0)
            .min(track.height as f32) as u32;
        let max_first = total.saturating_sub(visible);
        let fraction = if max_first == 0 {
            0.0
        } else {
            self.first_visible_row.min(max_first) as f32 / max_first as f32
        };
        let y = track.y + (((track.height - height) as f32) * fraction) as i32;
        Rect::new(track.x, y, track.width, height)
    }

    /// Whether `pos` is on the scrollbar track (and therefore grabs the thumb).
    fn scrollbar_contains(&self, pos: crate::core::Point) -> bool {
        self.scrollbar_track().map(|(track, _, _)| track.contains_point(pos)).unwrap_or(false)
    }

    /// Scrolls so the row under the dragged thumb is the first visible one.
    pub(crate) fn scrollbar_scroll_to(&mut self, y: i32) {
        let Some((track, total, visible)) = self.scrollbar_track() else {
            return;
        };
        let thumb = self.scrollbar_thumb(track, total, visible);
        let travel = (track.height.saturating_sub(thumb.height)).max(1) as f32;
        let offset = (y - track.y - thumb.height as i32 / 2) as f32;
        let max_first = total.saturating_sub(visible);
        let fraction = (offset / travel).clamp(0.0, 1.0);
        let next = (fraction * max_first as f32).round() as usize;
        if next != self.first_visible_row {
            self.first_visible_row = next;
        }
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
    pub(crate) fn scroll_columns(&mut self, columns: i32) -> bool {
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
    pub(crate) fn scroll_rows(&mut self, rows: i32) -> bool {
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
    pub(crate) fn offset_in_row_with(&self, rows: &[RowSpan], span: RowSpan, from: usize) -> usize {
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
    pub(crate) fn word_range_at(&self, offset: usize) -> (usize, usize) {
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
    pub(crate) fn copy_selection_to_clipboard(&self) -> Option<String> {
        let selected = self.selected_text();
        if selected.is_empty() {
            return None;
        }
        crate::set_clipboard_text(&selected);
        Some(selected)
    }

    /// The allocation-frugal build has no clipboard integration.
    #[cfg(alloc_frugal)]
    pub(crate) fn copy_selection_to_clipboard(&self) -> Option<String> {
        None
    }

    /// Inserts the platform clipboard's text at the caret.
    #[cfg(not(alloc_frugal))]
    pub(crate) fn paste_from_clipboard(&mut self) {
        let text = crate::get_clipboard_text();
        if !text.is_empty() {
            self.insert_str(&text);
        }
    }

    /// The allocation-frugal build has no clipboard integration.
    #[cfg(alloc_frugal)]
    pub(crate) fn paste_from_clipboard(&mut self) {}

    // ── FindReplaceBar wiring (COMP-A8) ──

    /// Attaches `bar` to this editor (COMP-A8).
    ///
    /// The bar's four intent signals are subscribed into a shared queue and applied by
    /// [`Self::poll_find_replace_bar`]. Attaching also reports the current match count for whatever
    /// query the bar already holds, so the strip does not wait for the first navigation. Attaching
    /// replaces any previous bar, so calling this twice leaves one set of subscriptions.
    ///
    /// The bar exposes a `use_regex` flag, but this crate has no regular-expression engine, so a
    /// regex query is searched literally. `match_case` is honoured.
    #[cfg(full_widgets)]
    pub fn attach_find_replace_bar(&mut self, bar: &FindReplaceBar) {
        let intents = Arc::new(Mutex::new(Vec::new()));
        let scope = ConnectionScope::new();
        let redraw = self.base.redraw_requested_signal().clone();

        let sink = Arc::clone(&intents);
        let request_redraw = redraw.clone();
        bar.find_next.connect_scoped(&scope, move || {
            lock(&sink).push(FindReplaceIntent::Next);
            request_redraw.emit();
        });

        let sink = Arc::clone(&intents);
        let request_redraw = redraw.clone();
        bar.find_previous.connect_scoped(&scope, move || {
            lock(&sink).push(FindReplaceIntent::Previous);
            request_redraw.emit();
        });

        let sink = Arc::clone(&intents);
        let request_redraw = redraw.clone();
        bar.replace_one.connect_scoped(&scope, move || {
            lock(&sink).push(FindReplaceIntent::ReplaceOne);
            request_redraw.emit();
        });

        let sink = Arc::clone(&intents);
        bar.replace_all.connect_scoped(&scope, move || {
            lock(&sink).push(FindReplaceIntent::ReplaceAll);
            redraw.emit();
        });

        let (matches, bar_redraw) = bar.match_counts_handle();
        self.find_replace = Some(FindReplaceBinding {
            intents,
            context: bar.context_handle(),
            matches,
            redraw: bar_redraw,
            _scope: scope,
        });
        // Adopt the bar's current query now, so its counter starts in step with the document.
        let (query, _replacement, match_case, _use_regex) = self.find_replace_context();
        self.report_find_replace_matches(&query, match_case);
        self.base.request_redraw();
    }

    /// Stops following the attached find/replace bar; the last reported counts stay.
    #[cfg(full_widgets)]
    pub fn detach_find_replace_bar(&mut self) {
        if self.find_replace.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`FindReplaceBar`] is attached.
    #[cfg(full_widgets)]
    pub fn is_find_replace_bar_attached(&self) -> bool {
        self.find_replace.is_some()
    }

    /// Applies every find/replace intent reported since the last poll (COMP-A8).
    ///
    /// Called from `draw` and `handle_event`, so a strip click reaches the document on the next
    /// frame or interaction. Returns whether the document or the selection moved. A bar with no
    /// query reports zero matches and changes nothing.
    #[cfg(full_widgets)]
    pub fn poll_find_replace_bar(&mut self) -> bool {
        let Some(binding) = self.find_replace.as_ref() else {
            return false;
        };
        let intents: Vec<FindReplaceIntent> = {
            let mut guard = lock(&binding.intents);
            core::mem::take(&mut *guard)
        };
        if intents.is_empty() {
            return false;
        }
        let (query, replacement, match_case, _use_regex) = self.find_replace_context();
        let mut applied = false;
        for intent in intents {
            applied |= match intent {
                FindReplaceIntent::Next => self.find_replace_step(&query, match_case, true),
                FindReplaceIntent::Previous => self.find_replace_step(&query, match_case, false),
                FindReplaceIntent::ReplaceOne => {
                    self.find_replace_one(&query, &replacement, match_case)
                }
                FindReplaceIntent::ReplaceAll => {
                    self.find_replace_all(&query, &replacement, match_case)
                }
            };
        }
        applied
    }

    /// The attached bar's `(query, replacement, match_case, use_regex)`, or empty defaults.
    #[cfg(full_widgets)]
    fn find_replace_context(&self) -> (String, String, bool, bool) {
        match self.find_replace.as_ref() {
            Some(binding) => lock(&binding.context).clone(),
            None => (String::new(), String::new(), false, false),
        }
    }

    /// Moves the selection to the next or previous match, wrapping at the document ends.
    ///
    /// Returns whether the selection moved. The search starts outside the current selection, so
    /// pressing "next" twice visits two different matches rather than re-selecting the first.
    #[cfg(full_widgets)]
    fn find_replace_step(&mut self, query: &str, match_case: bool, forward: bool) -> bool {
        if query.is_empty() {
            self.write_find_replace_counts(0, 0);
            return false;
        }
        let matches = self.find_replace_matches(query, match_case);
        if matches.is_empty() {
            self.write_find_replace_counts(0, 0);
            return false;
        }
        let from = self
            .selection_range()
            .map(|(start, end)| if forward { end } else { start })
            .unwrap_or(self.cursor);
        let index = if forward {
            matches.iter().position(|&(start, _)| start >= from).unwrap_or(0)
        } else {
            matches.iter().rposition(|&(_, end)| end <= from).unwrap_or(matches.len() - 1)
        };
        let (start, end) = matches[index];
        self.select_find_match(start, end);
        self.write_find_replace_counts(matches.len(), index);
        true
    }

    /// Replaces the current match, or the next one when the selection is not a match.
    #[cfg(full_widgets)]
    fn find_replace_one(&mut self, query: &str, replacement: &str, match_case: bool) -> bool {
        if self.read_only || query.is_empty() {
            self.report_find_replace_matches(query, match_case);
            return false;
        }
        let matches = self.find_replace_matches(query, match_case);
        if matches.is_empty() {
            self.write_find_replace_counts(0, 0);
            return false;
        }
        let selection = self.selection_range();
        let target = selection.filter(|range| matches.contains(range)).or_else(|| {
            let from = selection.map_or(self.cursor, |(_, end)| end);
            matches
                .iter()
                .copied()
                .find(|&(start, _)| start >= from)
                .or_else(|| matches.first().copied())
        });
        let Some((start, end)) = target else {
            self.write_find_replace_counts(matches.len(), 0);
            return false;
        };
        self.select_find_match(start, end);
        let before = self.text.clone();
        self.insert_str(replacement);
        let changed = self.text != before;
        self.report_find_replace_matches(query, match_case);
        changed
    }

    /// Replaces every match in one edit, which is one undo step through the editor's own path.
    #[cfg(full_widgets)]
    fn find_replace_all(&mut self, query: &str, replacement: &str, match_case: bool) -> bool {
        if self.read_only || query.is_empty() {
            self.report_find_replace_matches(query, match_case);
            return false;
        }
        let matches = self.find_replace_matches(query, match_case);
        if matches.is_empty() {
            self.write_find_replace_counts(0, 0);
            return false;
        }
        let mut next = String::with_capacity(self.text.len());
        let mut consumed = 0usize;
        for &(start, end) in &matches {
            next.push_str(&self.text[consumed..start]);
            next.push_str(replacement);
            consumed = end;
        }
        next.push_str(&self.text[consumed..]);
        let before = self.text.clone();
        self.set_text(next);
        let changed = self.text != before;
        self.report_find_replace_matches(query, match_case);
        changed
    }

    /// Selects one match without going through a movement gesture.
    #[cfg(full_widgets)]
    fn select_find_match(&mut self, start: usize, end: usize) {
        self.selection_anchor = Some(start);
        self.cursor = end;
        self.scroll_caret_into_view();
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Counts the matches of `query` and writes `(total, current)` into the attached bar.
    #[cfg(full_widgets)]
    fn report_find_replace_matches(&self, query: &str, match_case: bool) {
        let matches = self.find_replace_matches(query, match_case);
        let anchor = self.selection_range().map_or(self.cursor, |(start, _)| start);
        let current = matches.iter().position(|&(start, _)| start >= anchor).unwrap_or(0);
        self.write_find_replace_counts(matches.len(), current);
    }

    /// Writes the reported counters into the attached bar's shared cell.
    #[cfg(full_widgets)]
    fn write_find_replace_counts(&self, total: usize, current: usize) {
        let Some(binding) = self.find_replace.as_ref() else {
            return;
        };
        let current = if total == 0 { 0 } else { current.min(total - 1) };
        {
            let mut guard = lock(&binding.matches);
            if *guard == (total, current) {
                return;
            }
            *guard = (total, current);
        }
        binding.redraw.emit();
    }

    /// Every non-overlapping match of `query`, as ordered `(start, end)` byte ranges.
    ///
    /// Matching is literal: the bar's `use_regex` flag is not honoured because the crate has no
    /// regular-expression engine. A case-insensitive comparison folds one character at a time, so
    /// offsets stay on character boundaries for multi-byte text.
    #[cfg(full_widgets)]
    fn find_replace_matches(&self, query: &str, match_case: bool) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();
        if query.is_empty() {
            return matches;
        }
        let mut start = 0usize;
        while start < self.text.len() {
            if let Some(end) = match_query_at(&self.text, start, query, match_case) {
                matches.push((start, end));
                start = end;
            } else {
                match self.text[start..].chars().next() {
                    Some(ch) => start += ch.len_utf8(),
                    None => break,
                }
            }
        }
        matches
    }
}

/// The end offset of `query` when it matches at `start`, or `None`.
///
/// A character-wise comparison keeps the answer on a character boundary for multi-byte text. Case
/// folding is per character, which is the honest limit of a comparison done without the Unicode
/// multi-character case tables.
#[cfg(full_widgets)]
fn match_query_at(haystack: &str, start: usize, query: &str, match_case: bool) -> Option<usize> {
    if !haystack.is_char_boundary(start) {
        return None;
    }
    let mut consumed = start;
    let mut chars = haystack[start..].chars();
    for wanted in query.chars() {
        let found = chars.next()?;
        let same = if match_case {
            found == wanted
        } else {
            found == wanted || found.to_lowercase().eq(wanted.to_lowercase())
        };
        if !same {
            return None;
        }
        consumed += found.len_utf8();
    }
    Some(consumed)
}
