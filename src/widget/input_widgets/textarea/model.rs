// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `TextArea`'s document model: the buffer, the caret and its selection anchor, the undo step and
//! the find/replace wiring.
//!
//! This module answers every question about the *document*. Turning a byte offset into pixels is
//! [`super::paint`]'s job, so the two cannot disagree about where the ink goes: the geometry reads
//! the line table and the caret this module owns.

#[cfg(full_widgets)]
use crate::compat::{lock, Arc, Mutex};
use crate::compat::{Box, Rc, RefCell, String, ToString, Vec};
use crate::core::{Font, Point, Rect};
#[cfg(full_widgets)]
use crate::signal::ConnectionScope;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::UndoStack;
#[cfg(full_widgets)]
use crate::widget::companions::FindReplaceBar;
use crate::widget::input_widgets::input_companions::{ScrollOrientation, Scrollbar};
use crate::widget::text_utils::{byte_index_of_char, floor_char_boundary};
use crate::widget::{BaseWidget, WidgetKind};
use std::cell::Cell;

use super::paint::{BandWidthCache, DisplayRowCache, LineAdvanceCache, PendingEdit};

/// One logical line: `(start, content_end, end)` byte offsets.
pub(crate) type LineSpan = (usize, usize, usize);
/// A line table shared between readers.
pub(crate) type LineSpanTable = Rc<Vec<LineSpan>>;

/// The splice record the text family shares (I-001).
///
/// `TextArea` used to carry this record privately; it now lives in the undo module so `LineEdit`,
/// the combo-box editor, the search fields and the inline editor file the same shape. The alias
/// keeps the in-file call sites and tests reading naturally.
pub(crate) type TextDeltaCommand = crate::undo::TextSpliceCommand;

/// Multi-line text input widget.
///
/// Supports multi-line text storage with a byte-index cursor, character-level
/// insertion and deletion, placeholder display, read-only mode, and word-wrap
/// toggling. Emits a `changed` signal whenever the text content changes.
pub struct TextArea {
    pub(crate) base: BaseWidget,
    /// Text content (lines separated by '\n').
    pub(crate) text: String,
    /// Cursor position (byte index into text).
    pub(crate) cursor_pos: usize,
    /// The other end of the selection, as a byte offset, or `None` when nothing is selected.
    ///
    /// # Why an anchor and not an ordered `(start, end)` pair
    ///
    /// The caret is the *live* end the user is moving; the anchor is the end that stays put.
    /// Storing the range as a pair instead would mean rewriting both numbers on every movement,
    /// and no fact would be left to answer "which end is the caret" — which is exactly what the
    /// paint path and the next edit both ask. So the anchor is stored alone and the range is
    /// derived in [`TextArea::selection_range`], the same way
    /// [`crate::widget::input_widgets::lineedit::LineEdit`] derives it, so a multi-line and a
    /// single-line field cannot disagree about what a selection is.
    pub(crate) selection_anchor: Option<usize>,
    /// Whether a pointer drag is currently extending a selection.
    ///
    /// Set by [`TextArea::press_at`], cleared by [`TextArea::end_drag`]. It exists because
    /// `MouseMove` is delivered for a bare hover too — without the flag, a caret the user never
    /// grabbed would follow the pointer across the control.
    pub(crate) dragging_selection: bool,
    /// The first logical line the field paints, i.e. how far the view is scrolled down.
    ///
    /// # Why a line index and not a pixel offset
    ///
    /// Rows are laid out on a fixed `LINE_H` grid, so a line index *is* the scroll position divided
    /// by the row height — and it stays correct when the value grows or shrinks above the viewport,
    /// where a pixel offset would have to be re-derived against the new content height. The paint
    /// path and the pixel-to-offset map both read it ([`TextArea::visible_line_index`]), so a
    /// scrolled field draws and hit-tests on the same rows.
    ///
    /// It is maintained by [`TextArea::scroll_caret_into_view`] after every caret movement, which is
    /// what makes typing at the bottom of a full field advance the view instead of writing onto a
    /// row the user cannot see.
    pub(crate) first_visible_line: usize,
    /// Maximum text length (0 = unlimited).
    pub(crate) max_length: usize,
    /// Whether the widget is read-only.
    pub(crate) read_only: bool,
    /// Placeholder text when empty.
    pub(crate) placeholder: String,
    /// Whether this widget currently holds keyboard focus.
    pub(crate) focused: bool,
    /// Signal emitted when text changes.
    pub changed: GenericSignal,
    /// Emitted when a context menu is requested at this point in the document.
    ///
    /// The host supplies the menu. The signal carries the request position in window
    /// coordinates so the host can place the menu beside the click; the control does not
    /// decide which items are meaningful, because that policy belongs to the application
    /// that owns the cut/copy/paste implementation.
    pub context_menu_requested: Signal1<Point>,
    /// Emitted when the selected range changes.
    ///
    /// The range is read back through [`Self::selection_range`], the same derivation the paint
    /// and edit paths use, so a host never has to reconstruct it from two offsets.
    pub selection_changed: GenericSignal,
    /// Emitted with the new caret byte offset whenever it moves.
    pub cursor_position_changed: Signal1<usize>,
    /// Emitted with the new value whenever undo availability changes.
    pub undo_available: Signal1<bool>,
    /// Emitted with the new value whenever redo availability changes.
    pub redo_available: Signal1<bool>,
    /// Last caret offset announced, so the signal is edge-triggered.
    caret_cursor_announced: usize,
    /// Last selected range announced, so the signal is edge-triggered.
    caret_selection_announced: Option<(usize, usize)>,
    /// Last undo/redo availability announced.
    undo_available_announced: bool,
    redo_available_announced: bool,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// Lazily rebuilt line table, shared with every reader of the current text.
    ///
    /// The paint path asks for the table twice when a selection is visible (once for the
    /// highlight bands, once for the glyphs), and a click asks again. Without a cache that was
    /// two or three O(document) scans and Vec allocations per frame. The cache is invalidated
    /// by the one funnel every text mutation passes through, so a hit is always the current
    /// text's table; the `Rc` makes a hit a refcount bump rather than a copy.
    pub(crate) line_spans_cache: RefCell<Option<LineSpanTable>>,
    /// Whether long lines wrap at word boundaries instead of being clipped.
    ///
    /// Off by default, which keeps the original one-row-per-logical-line layout pixel for pixel.
    pub(crate) soft_wrap: bool,
    /// Extra inset, in logical pixels, between the field's border and its text.
    ///
    /// Zero by default; a positive margin moves the text, the selection and the hit test together.
    pub(crate) content_margin: i32,
    /// The largest number of logical lines the value may hold, or `None` for no limit.
    ///
    /// When an insertion pushes the document past the limit, the oldest lines are dropped — the
    /// ring-buffer behaviour a log or console pane needs.
    max_block_count: Option<usize>,
    /// Horizontal scroll offset in logical pixels, for an unwrapped line wider than the field.
    ///
    /// Zero unless the caret has been driven past the field's trailing edge, so an ordinary
    /// document never scrolls sideways.
    pub(crate) scroll_x: i32,
    /// Bumped on every value change, so the display-row and prefix-width caches can identify the
    /// document in O(1).
    pub(crate) revision: u64,
    /// Memoised display rows: the soft-wrapped rows the paint, caret and hit test share.
    pub(crate) display_rows_cache: RefCell<Option<DisplayRowCache>>,
    /// The splice the next [`Self::display_rows`] call may patch in place (I-006).
    ///
    /// Only one splice is described at a time. A second edit before the rows are read clears the
    /// hint, which falls back to the whole-table path rather than composing two unrelated deltas.
    pub(crate) pending_row_edit: Cell<Option<PendingEdit>>,
    /// The splice the next [`Self::line_spans`] call may patch in place (I-P10).
    ///
    /// The line table has the same incremental rule as the display rows: an edit that inserts or
    /// removes bytes changes only the lines it touched and shifts the offsets after them. A second
    /// splice before the table is read clears the hint and falls back to a full rescan.
    pub(crate) pending_line_edit: Cell<Option<PendingEdit>>,
    /// Character boundaries of the row the pointer last hit-tested (I-P09).
    pub(crate) line_advance_cache: RefCell<Option<LineAdvanceCache>>,
    /// The renderer's line height for the current font, measured once (I-P09).
    pub(crate) line_height_cache: RefCell<Option<(Font, u32)>>,
    /// Measured advances for the selection band's two edges, keyed by the revision.
    ///
    /// The band used to re-shape its line prefix and its selected run on every frame, which is two
    /// full text measurements per visible line per paint. The cached values are the *same*
    /// measurements — an entry is only reused while the document is unchanged — so the band's
    /// pixels do not move.
    pub(crate) band_width_cache: RefCell<Option<BandWidthCache>>,
    /// The caret's blink state, advanced by [`TextArea::tick`] (I-116).
    pub(crate) cursor_blink: crate::style::CursorBlink,
    /// Whether the pointer is currently dragging the scrollbar thumb (I-118).
    pub(crate) scrollbar_drag: bool,
    /// The standard scrollbar the track, thumb and drag offset come from (I-128).
    ///
    /// The companion owns the proportional thumb geometry and the pointer-to-offset map; this
    /// control still paints it in its own ink because the companion's painter reads only palette
    /// roles and would move the field's themed scrollbar to a different colour.
    pub(crate) scrollbar: Scrollbar,
    /// The `FindReplaceBar` this area follows, when one is attached (COMP-A8).
    #[cfg(full_widgets)]
    find_replace: Option<FindReplaceBinding>,
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

/// The live link between a [`TextArea`] and a [`FindReplaceBar`] (COMP-A8).
///
/// The bar reports intents with unit signals, so the link parks one intent per emission and
/// [`TextArea::poll_find_replace_bar`] applies them. The query, replacement and match policy are
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

/// One line's visible content end, excluding a `\r\n` or `\n` terminator.
fn line_content_end(chunk: &str, end: usize) -> usize {
    if chunk.ends_with("\r\n") {
        end - 2
    } else if chunk.ends_with('\n') {
        end - 1
    } else {
        end
    }
}

/// Patches a line table with one splice, returning whether the patch applied (I-P10).
///
/// Only the lines the edit touched are rescanned; every line after the change has its offsets
/// shifted by the size difference. The full-table fallback remains for the cases this cannot
/// describe (a block-count trim or two splices before the table is read).
fn splice_line_spans(spans: &mut Vec<LineSpan>, text: &str, edit: PendingEdit) -> bool {
    let Some(&(_, _, old_len)) = spans.last() else {
        return false;
    };
    let offset = edit.start.min(old_len);
    let removed_end = offset.saturating_add(edit.removed).min(old_len);
    if offset > text.len() {
        return false;
    }
    // The line containing the edit's start. An offset exactly on a line boundary belongs to the
    // line that begins there, which is what keeps an insertion at a line start from rewriting the
    // previous terminator.
    let first = spans
        .partition_point(|&(start, _, _)| start <= offset)
        .saturating_sub(1)
        .min(spans.len() - 1);
    // The last line the removal touched. When the removed run ends exactly on a boundary the
    // following line joins the affected region, because deleting a terminator merges the two.
    let last = if removed_end > offset {
        spans
            .partition_point(|&(start, _, _)| start <= removed_end)
            .saturating_sub(1)
            .min(spans.len() - 1)
    } else {
        first
    }
    .max(first);
    let region_start = spans[first].0;
    let region_end_old = spans[last].2;
    // Bytes removed inside the region; the caret can sit after a terminator, so clamp.
    let removed_in_region = removed_end.saturating_sub(offset).min(region_end_old - region_start);
    let region_end_new =
        region_start + (region_end_old - region_start) - removed_in_region + edit.inserted;
    if region_end_new > text.len() || region_end_new < region_start {
        return false;
    }
    let mut fresh = Vec::new();
    let mut start = region_start;
    for chunk in text[region_start..region_end_new].split_inclusive('\n') {
        let end = start + chunk.len();
        fresh.push((start, line_content_end(chunk, end), end));
        start = end;
    }
    if fresh.is_empty() {
        fresh.push((region_start, region_start, region_start));
    } else if start == text.len() && text.ends_with('\n') {
        fresh.push((start, start, start));
    }
    let fresh_len = fresh.len();
    spans.splice(first..=last, fresh);
    let shift = edit.inserted as i64 - edit.removed as i64;
    if shift != 0 {
        let begin = first + fresh_len;
        for span in spans.iter_mut().skip(begin) {
            span.0 = shift_saturating(span.0, shift);
            span.1 = shift_saturating(span.1, shift);
            span.2 = shift_saturating(span.2, shift);
        }
    }
    true
}

fn shift_saturating(value: usize, shift: i64) -> usize {
    (value as i64 + shift).max(0) as usize
}

impl TextArea {
    /// Creates a new `TextArea` with the given initial text and geometry.
    pub fn new(text: String, rect: Rect) -> Self {
        let cursor_pos = text.len();
        let history_target = Rc::new(RefCell::new(text.clone()));
        Self {
            base: BaseWidget::new(WidgetKind::TextArea, rect, "TextArea"),
            text,
            cursor_pos,
            selection_anchor: None,
            dragging_selection: false,
            first_visible_line: 0,
            max_length: 0,
            read_only: false,
            placeholder: String::new(),
            focused: false,
            changed: GenericSignal::new(),
            context_menu_requested: Signal1::new(),
            selection_changed: GenericSignal::new(),
            cursor_position_changed: Signal1::new(),
            undo_available: Signal1::new(),
            redo_available: Signal1::new(),
            caret_cursor_announced: cursor_pos,
            caret_selection_announced: None,
            undo_available_announced: false,
            redo_available_announced: false,
            undo_stack: UndoStack::new(),
            history_target,
            restoring_history: false,
            line_spans_cache: RefCell::new(None),
            soft_wrap: false,
            content_margin: 0,
            max_block_count: None,
            scroll_x: 0,
            revision: 0,
            display_rows_cache: RefCell::new(None),
            pending_row_edit: Cell::new(None),
            pending_line_edit: Cell::new(None),
            line_advance_cache: RefCell::new(None),
            line_height_cache: RefCell::new(None),
            band_width_cache: RefCell::new(None),
            cursor_blink: crate::style::CursorBlink::new(),
            scrollbar_drag: false,
            scrollbar: {
                let mut bar = Scrollbar::new();
                bar.set_orientation(ScrollOrientation::Vertical, 4);
                bar.set_thumb_min_length(8);
                bar
            },
            #[cfg(full_widgets)]
            find_replace: None,
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The selected range as ordered `(start, end)` byte offsets, or `None` when nothing is
    /// selected.
    ///
    /// The single derivation every reader goes through — [`Self::selected_text`], the paint, and
    /// the deletion paths — so "is there a selection" and "which bytes are in it" cannot be
    /// answered two different ways. A zero-width anchor/caret pair is not a range: see
    /// `Self::normalize_selection`.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        let anchor = floor_char_boundary(&self.text, anchor.min(self.text.len()));
        if anchor == caret {
            return None;
        }
        Some(if anchor < caret { (anchor, caret) } else { (caret, anchor) })
    }

    /// The selected text, or an empty string when nothing is selected.
    ///
    /// Returned by value rather than as a slice because the range ends are clamped and snapped to
    /// character boundaries: a caller holding `&str` would have to repeat that arithmetic to index
    /// it safely on a multi-byte value.
    pub fn selected_text(&self) -> String {
        match self.selection_range() {
            Some((start, end)) => self.text[start..end].to_string(),
            None => String::new(),
        }
    }

    /// The anchor a `Shift`-extend grows from, if one is set.
    ///
    /// Exposed because it is the only way a caller can tell "the caret is here and nothing is
    /// selected yet" from "a drag began here" — a distinction the next `Shift`-movement acts on,
    /// and the one the selection tests assert instead of inferring from the range.
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
        self.cursor_pos = self.text.len();
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Selects all of `line` (0-based logical line), returning whether that line exists.
    ///
    /// # Why select-by-line is its own entry point
    ///
    /// "Select this line" is a *program* over the text, not a movement of the caret. Leaving it to
    /// the host would put the line arithmetic outside the control, where it cannot know the
    /// trailing-newline convention this control's `lines()` and caret placement share. The bounded
    /// answer — a line that does not exist changes nothing — is the honest one, rather than
    /// clamping to the last line and surprising the caller.
    ///
    /// The range **includes** the line's terminating newline, so `select_line` followed by a
    /// delete removes the whole line instead of leaving an empty one behind, which is what
    /// deleting a line means.
    pub fn select_line(&mut self, line: usize) -> bool {
        let Some((start, end)) = self.line_byte_range(line) else {
            return false;
        };
        self.selection_anchor = Some(start);
        self.cursor_pos = end;
        self.notify_caret_change();
        self.base.request_redraw();
        true
    }

    /// Reports a caret/selection change and the availability signals derived from it.
    ///
    /// Each signal is edge-triggered against the value last announced, so the many entry points
    /// that move the caret (keyboard, pointer, programmatic writes, history restore) can call this
    /// unconditionally without a changed position producing a second event.
    pub(crate) fn notify_caret_change(&mut self) {
        let selection = self.selection_range();
        if self.cursor_pos != self.caret_cursor_announced {
            self.caret_cursor_announced = self.cursor_pos;
            self.cursor_position_changed.emit(self.cursor_pos);
        }
        if selection != self.caret_selection_announced {
            self.caret_selection_announced = selection;
            self.selection_changed.emit();
        }
        self.refresh_edit_capabilities();
    }

    /// Emits the undo/redo availability signals when their answers have changed.
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

    /// The half-open byte range of one logical line, including its terminator, or `None` when the
    /// line does not exist.
    fn line_byte_range(&self, line: usize) -> Option<(usize, usize)> {
        self.line_spans().get(line).map(|&(start, _, end)| (start, end))
    }

    /// Every logical line's byte range, as `(start, content_end, end)`.
    ///
    /// * `start` — the line's first byte.
    /// * `content_end` — one past the line's **visible** text, excluding a `\n` (or `\r\n`)
    ///   terminator. This is the same split [`str::lines`] makes, which is what the paint path
    ///   draws.
    /// * `end` — one past the terminator, i.e. the next line's `start`.
    ///
    /// # Why a table and not "the nth line"
    ///
    /// The paint path used to ask "which line is row N" once per visible row, and every answer
    /// rescanned the document from byte 0 (`split_inclusive('\n').enumerate().nth(N)`, and
    /// `lines().nth(N)`), so one frame cost O(rows × document).
    ///
    /// It also reset the running offset at the first **visible** line instead of counting from the
    /// document's start, so a scrolled field measured its selection band from a byte the band does
    /// not belong to — the highlight drifted away from the glyphs it was meant to cover. One table
    /// answers both: the index is a lookup, and every offset is the document's own.
    ///
    /// The trailing empty line of a value that ends in a newline is part of the table, because
    /// [`Self::line_count`] counts it (a caret can sit there) even though it paints nothing.
    pub(crate) fn line_spans(&self) -> LineSpanTable {
        // A pending splice is consumed at most once. It is taken before the cache borrow so the
        // immutable read and the later write cannot overlap.
        let pending = self.pending_line_edit.take();
        let cached = self.line_spans_cache.borrow().as_ref().map(Rc::clone);
        if let Some(cached) = cached {
            if let Some(edit) = pending {
                let mut spans = (*cached).clone();
                if splice_line_spans(&mut spans, &self.text, edit) {
                    let next = Rc::new(spans);
                    *self.line_spans_cache.borrow_mut() = Some(Rc::clone(&next));
                    return next;
                }
            }
            return cached;
        }
        let mut spans = Vec::new();
        let mut start = 0usize;
        for chunk in self.text.split_inclusive('\n') {
            let end = start + chunk.len();
            let content_end = line_content_end(chunk, end);
            spans.push((start, content_end, end));
            start = end;
        }
        // `split_inclusive` yields nothing for an empty value; its one (empty) line still exists.
        if spans.is_empty() {
            spans.push((0, 0, 0));
        } else if start == self.text.len() && self.text.ends_with('\n') {
            // A value ending in a terminator has an empty line after it.
            spans.push((start, start, start));
        }
        let spans = Rc::new(spans);
        *self.line_spans_cache.borrow_mut() = Some(Rc::clone(&spans));
        spans
    }

    /// Collapses a zero-width anchor/caret pair to "no selection".
    ///
    /// A caret sitting exactly on its anchor has selected nothing, but keeping the anchor set would
    /// make a later `Shift`-movement extend from a position the user did not choose.
    pub(crate) fn normalize_selection(&mut self) {
        if self.selection_anchor == Some(self.cursor_pos) {
            self.selection_anchor = None;
        }
    }

    /// Deletes the selected range when there is one, returning whether anything was removed.
    ///
    /// Every mutating path funnels through here, so "typing over a selection replaces it" is one
    /// implementation rather than a rule [`Self::insert`], [`Self::insert_str`],
    /// [`Self::delete_char`] and [`Self::delete_forward`] each has to remember (principle #101).
    /// The caret ends at the range's start and the anchor is dropped, because the selection it
    /// described no longer exists.
    ///
    /// The undo snapshot is *not* taken here: the caller records one step per user action through
    /// [`Self::record_edit`], so a paste that replaces a selection is one undo press rather than
    /// two.
    pub(crate) fn replace_selection(&mut self) -> bool {
        let selection = self.selection_range();
        let Some(start) =
            crate::widget::text_utils::replace_selected_range(&mut self.text, selection)
        else {
            self.selection_anchor = None;
            return false;
        };
        self.cursor_pos = start;
        self.selection_anchor = None;
        true
    }

    /// Records the value `before` as one undo step and emits `changed`.
    ///
    /// Split out so the editing paths that mutate `self.text` directly — the selection replacement
    /// and the forward delete — file the same kind of snapshot and emit the same signal as
    /// `set_text` does. Without a shared step one of them would silently skip undo (so `Primary+Z`
    /// would jump past the edit) or skip `changed` (so a host bound to it would never see the
    /// edit).
    pub(crate) fn record_edit(&mut self, before: String) {
        if self.restoring_history {
            return;
        }
        // Apply the line budget before the snapshot, so the recorded "after" is the value the
        // control actually keeps. The caches are invalidated first: the budget reads the line
        // table, and a stale one would cut at an offset from the previous document.
        self.line_spans_cache.borrow_mut().take();
        self.pending_line_edit.set(None);
        self.display_rows_cache.borrow_mut().take();
        self.pending_row_edit.set(None);
        self.revision = self.revision.wrapping_add(1);
        self.enforce_max_block_count();
        *self.history_target.borrow_mut() = self.text.clone();
        self.undo_stack.push(Box::new(TextDeltaCommand::new(
            self.history_target.clone(),
            &before,
            &self.text,
            "Edit text",
            "text_area_text",
        )));
        // Every edit funnels through here, so this is the one place the viewport has to follow the
        // caret. Keeping it at the call sites would let the next edit path added forget it, and the
        // symptom (typing into a row the user cannot see) is invisible in a snapshot test.
        self.scroll_caret_into_view();
        self.changed.emit();
        self.notify_caret_change();
    }

    /// Records an in-place splice as one undo step and emits `changed` (I-P60).
    ///
    /// Unlike [`Self::record_edit`] this needs no before-text clone: the caller already knows the
    /// changed run and the inserted text, so ordinary typing costs one allocation proportional to
    /// the edit instead of a whole-document copy.
    ///
    /// When `max_block_count` is active the budget may also drop lines from the front, which a
    /// plain splice cannot describe; those callers pass `before = Some(...)` and the content diff
    /// is recorded instead.
    fn record_splice(
        &mut self,
        offset: usize,
        removed: String,
        inserted: String,
        before: Option<String>,
    ) {
        if self.restoring_history {
            return;
        }
        let edit = PendingEdit { start: offset, removed: removed.len(), inserted: inserted.len() };
        self.revision = self.revision.wrapping_add(1);
        if self.max_block_count.is_some() {
            // The line budget may drop a prefix, which no single splice describes. Keep the
            // whole-table paths for both caches.
            self.line_spans_cache.borrow_mut().take();
            self.pending_line_edit.set(None);
            self.display_rows_cache.borrow_mut().take();
            self.pending_row_edit.set(None);
        } else {
            // A second splice landed before either table was read. One hint cannot describe both,
            // so each affected cache falls back to its whole-table path rather than composing
            // unrelated deltas.
            if self.pending_line_edit.get().is_some() {
                self.line_spans_cache.borrow_mut().take();
                self.pending_line_edit.set(None);
            } else {
                self.pending_line_edit.set(Some(edit));
            }
            if self.pending_row_edit.get().is_some() {
                self.display_rows_cache.borrow_mut().take();
                self.pending_row_edit.set(None);
            } else {
                // The row cache survives: the next `display_rows` re-wraps only the paragraphs the
                // splice touched and reuses the untouched ends (I-006).
                self.pending_row_edit.set(Some(edit));
            }
        }
        self.enforce_max_block_count();
        match before {
            Some(before) => {
                // The line budget may have changed the value beyond the splice, so record the
                // content diff instead of the (now incomplete) splice description.
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.push(Box::new(TextDeltaCommand::new(
                    self.history_target.clone(),
                    &before,
                    &self.text,
                    "Edit text",
                    "text_area_text",
                )));
            }
            None => {
                // Advance the undo mirror with the same splice rather than replacing it with a
                // whole-document clone, then file the matching delta.
                {
                    let mut target = self.history_target.borrow_mut();
                    let start = offset.min(target.len());
                    let end = (start + removed.len()).min(target.len());
                    target.replace_range(start..end, &inserted);
                }
                self.undo_stack.push(Box::new(TextDeltaCommand::from_splice(
                    self.history_target.clone(),
                    offset,
                    removed,
                    inserted,
                    "Edit text",
                    "text_area_text",
                )));
            }
        }
        self.scroll_caret_into_view();
        self.changed.emit();
        self.notify_caret_change();
    }

    /// Replaces the entire text content.
    ///
    /// Emits the `changed` signal if the new text differs from the current text.
    /// The cursor is moved to the end of the new text.
    /// Replaces the entire text content.
    ///
    /// Emits the `changed` signal if the new text differs from the current text.
    /// The cursor is moved to the end of the new text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        let max = self.max_length;
        let next = if max > 0 && text.chars().count() > max {
            let boundary = byte_index_of_char(&text, max);
            text[..boundary].to_string()
        } else {
            text
        };
        let before = self.text.clone();
        self.text = next;
        self.line_spans_cache.borrow_mut().take();
        self.pending_line_edit.set(None);
        self.display_rows_cache.borrow_mut().take();
        self.pending_row_edit.set(None);
        self.revision = self.revision.wrapping_add(1);
        self.enforce_max_block_count();
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextDeltaCommand::new(
                self.history_target.clone(),
                &before,
                &self.text,
                "Edit text",
                "text_area_text",
            )));
        }
        self.cursor_pos = self.text.len();
        self.changed.emit();
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Appends `text` to the end of the document.
    ///
    /// The log-pane counterpart of [`Self::set_text`]: the existing value is kept and the new run
    /// is added at its end, as one undo step. Like `TextEdit::append` the caret ends at the end of
    /// the value, so the next keystroke continues from what was just appended.
    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // One copy into the buffer: the old path cloned the whole document into a candidate, then
        // `set_text` cloned again when it adopted it (I-007).
        self.cursor_pos = self.text.len();
        self.selection_anchor = None;
        self.insert_text_in_place(text);
    }

    /// Inserts a single character at the current cursor position.
    ///
    /// If `max_length` is greater than zero and the text already meets or exceeds
    /// that limit, the character is not inserted.
    ///
    /// The cursor is snapped to a UTF-8 char boundary before inserting (via
    /// `floor_char_boundary`), so a misplaced caret cannot split a character.
    pub fn insert_char(&mut self, ch: char) {
        if self.max_length > 0 && self.text.chars().count() >= self.max_length {
            return;
        }
        // The glyph's own UTF-8 bytes, so typing allocates no intermediate `String` (I-P60).
        let mut glyph = [0u8; 4];
        self.insert_text_in_place(ch.encode_utf8(&mut glyph));
    }

    /// Inserts `text` at the caret in place, replacing any selection.
    ///
    /// The single insertion path for both a single character and a committed/pasted run, so the
    /// two cannot disagree about selection replacement, the length limit or undo (I-P60). It
    /// mutates `self.text` directly and records the splice, so no whole-document candidate copy
    /// is built per keystroke.
    fn insert_text_in_place(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let selection = self.selection_range();
        let removed = selection.map(|(start, end)| self.text[start..end].to_string());
        let start = selection
            .map(|(start, _)| start)
            .unwrap_or_else(|| floor_char_boundary(&self.text, self.cursor_pos));
        // Only the line-budget path needs a full before-image, because it may drop lines from the
        // front and turn this splice into a larger change.
        let before = self.max_block_count.map(|_| self.text.clone());
        if let Some(removed) = removed.as_ref() {
            self.text.replace_range(start..start + removed.len(), "");
        }
        self.text.insert_str(start, text);
        if self.max_length > 0 && self.text.chars().count() > self.max_length {
            let boundary = byte_index_of_char(&self.text, self.max_length);
            self.text.truncate(boundary);
        }
        self.cursor_pos = floor_char_boundary(&self.text, start + text.len());
        self.selection_anchor = None;
        let record_start = start.min(self.text.len());
        let record_end = self.cursor_pos.max(record_start).min(self.text.len());
        let inserted = self.text[record_start..record_end].to_string();
        self.record_splice(record_start, removed.unwrap_or_default(), inserted, before);
        self.base.request_redraw();
    }

    /// Inserts a single character.
    ///
    /// # Deprecated
    ///
    /// [`Self::insert_char`] is the name shared by the other text fields (WDGT-14). This alias
    /// enters exactly the same path, so migrating is a rename with no behaviour change.
    #[deprecated(note = "use `insert_char`, the name shared by the other text fields (WDGT-14)")]
    pub fn insert(&mut self, ch: char) {
        self.insert_char(ch);
    }

    /// Deletes the character immediately before the cursor, or the selection when there is one.
    ///
    /// If the cursor is at position 0 with no selection, this is a no-op.
    pub fn delete_char(&mut self) {
        // A selection is what Backspace means when there is one: the whole range goes, not just the
        // single character behind the caret.
        if let Some((start, end)) = self.selection_range() {
            self.delete_range(start, end);
            return;
        }
        // A no-op backspace at the start must not snapshot the whole value first (I-P60).
        if self.cursor_pos == 0 {
            return;
        }
        let start = self.previous_char_boundary();
        let end = floor_char_boundary(&self.text, self.cursor_pos);
        self.delete_range(start, end);
    }

    /// Removes `start..end` as one splice and one undo step.
    fn delete_range(&mut self, start: usize, end: usize) {
        if start >= end || end > self.text.len() {
            return;
        }
        let removed = self.text[start..end].to_string();
        let before = self.max_block_count.map(|_| self.text.clone());
        self.text.replace_range(start..end, "");
        self.cursor_pos = start;
        self.selection_anchor = None;
        self.record_splice(start, removed, String::new(), before);
        self.base.request_redraw();
    }

    /// Returns the current cursor position (byte index into `text`).
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Sets the cursor position, clamped to the text length and snapped to a character boundary.
    ///
    /// # Why the snap is not optional
    ///
    /// The position is a **byte** index (that is what a slice needs), so an arbitrary value can
    /// land inside a multi-byte character. Every reader of `cursor_pos` slices at it —
    /// [`Self::delete_char`], and the caret's `cursor_screen_x`/`cursor_screen_y` on the *draw*
    /// path — so a mid-character position panicked the paint, not merely the edit. Snapping here
    /// means no reader has to defend itself, and it is the same guard [`Self::insert`] already
    /// applies.
    pub fn set_cursor_pos(&mut self, pos: usize) {
        let clamped = floor_char_boundary(&self.text, pos.min(self.text.len()));
        if self.cursor_pos != clamped {
            self.cursor_pos = clamped;
            // This is the programmatic "put the caret here", not a movement, so it drops any
            // selection. The `Shift`-aware path a *user* takes is [`Self::select_with_modifiers`],
            // which keeps the anchor on purpose.
            self.selection_anchor = None;
            self.base.request_redraw();
        } else if self.selection_anchor.take().is_some() {
            self.base.request_redraw();
        }
        self.notify_caret_change();
    }

    /// Moves the caret to `target` (a byte offset), honouring the modifier keys held.
    ///
    /// # The single entry point for every keyboard movement
    ///
    /// Left, Right, Up, Down, Home and End all funnel through here, so "what Shift means" is
    /// written once instead of six times (principle #101). The two behaviours are:
    ///
    /// * **Shift** — extend: the caret moves to `target` and the *anchor* stays put, so the
    ///   selection becomes the range between them. With no anchor yet, the current caret is
    ///   adopted as the anchor first, so `Shift+Home` on a fresh caret selects its own line prefix
    ///   rather than the whole document.
    /// * **No modifier** — replace: the caret moves and any selection is dropped.
    ///
    /// The anchor deliberately does **not** move on a later extension: it belongs to the whole
    /// gesture, so `Shift+End` after `Shift+Home` sweeps back to the other side of the same anchor
    /// instead of re-anchoring and growing from the wrong end.
    ///
    /// `target` is clamped and snapped to a character boundary, so an offset computed from a pixel
    /// can never leave the caret inside a character.
    pub fn select_with_modifiers(&mut self, target: usize, modifiers: crate::shortcut::Modifiers) {
        let target = floor_char_boundary(&self.text, target.min(self.text.len()));
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor_pos);
            }
            self.cursor_pos = target;
        } else {
            self.selection_anchor = None;
            self.cursor_pos = target;
        }
        self.normalize_selection();
        self.scroll_caret_into_view();
        self.notify_caret_change();
        self.base.request_redraw();
    }
}

impl TextArea {
    /// The number of logical lines in the value, counting a trailing newline's empty line.
    pub fn line_count(&self) -> usize {
        self.line_spans().len()
    }

    /// Whether long lines wrap at word boundaries.
    pub fn soft_wrap(&self) -> bool {
        self.soft_wrap
    }

    /// Enables or disables word-boundary soft wrapping.
    ///
    /// Off by default: a line wider than the field is clipped, which is the layout a source editor
    /// wants and the one this control shipped with. Turning it on lays each long line out over as
    /// many display rows as it needs.
    pub fn set_soft_wrap(&mut self, wrap: bool) {
        if self.soft_wrap == wrap {
            return;
        }
        self.soft_wrap = wrap;
        self.display_rows_cache.borrow_mut().take();
        self.pending_row_edit.set(None);
        self.first_visible_line = 0;
        self.base.request_redraw();
    }

    /// The extra inset between the border and the text, in logical pixels.
    pub fn content_margin(&self) -> i32 {
        self.content_margin
    }

    /// Sets the content margin. Negative values are clamped to zero.
    pub fn set_content_margin(&mut self, margin: i32) {
        let margin = margin.max(0);
        if self.content_margin == margin {
            return;
        }
        self.content_margin = margin;
        self.display_rows_cache.borrow_mut().take();
        self.pending_row_edit.set(None);
        self.base.request_redraw();
    }

    /// The maximum number of logical lines the value may hold, or `None` for no limit.
    pub fn max_block_count(&self) -> Option<usize> {
        self.max_block_count
    }

    /// Sets the maximum number of logical lines, trimming the oldest ones immediately.
    ///
    /// `None` and `Some(0)` both mean no limit; a log pane that only needs a bound passes the
    /// number of lines it wants to keep.
    pub fn set_max_block_count(&mut self, max: Option<usize>) {
        let max = max.filter(|value| *value > 0);
        if self.max_block_count == max {
            return;
        }
        self.max_block_count = max;
        if self.enforce_max_block_count() {
            self.changed.emit();
            // Dropping the oldest lines clamps the caret and the anchor, so report the move.
            self.notify_caret_change();
        }
        self.base.request_redraw();
    }

    /// Drops the oldest lines when the value is over [`Self::max_block_count`].
    ///
    /// Returns whether anything was removed. The caret and the selection anchor move with the
    /// surviving text so an edit cannot be left pointing into a line that no longer exists.
    fn enforce_max_block_count(&mut self) -> bool {
        let Some(max) = self.max_block_count else {
            return false;
        };
        let spans = self.line_spans();
        if spans.len() <= max {
            return false;
        }
        let drop_lines = spans.len() - max;
        let cut = spans.get(drop_lines).map(|span| span.0).unwrap_or(self.text.len());
        if cut == 0 {
            return false;
        }
        self.text.drain(..cut);
        self.cursor_pos = self.cursor_pos.saturating_sub(cut);
        self.selection_anchor = self.selection_anchor.map(|anchor| anchor.saturating_sub(cut));
        self.line_spans_cache.borrow_mut().take();
        self.pending_line_edit.set(None);
        self.display_rows_cache.borrow_mut().take();
        self.pending_row_edit.set(None);
        self.revision = self.revision.wrapping_add(1);
        true
    }
}

impl TextArea {
    /// Returns the maximum text length in **characters**, where `0` means unlimited.
    pub fn max_length(&self) -> usize {
        self.max_length
    }

    /// Sets the maximum text length **in characters**, truncating the value if needed.
    ///
    /// # One frame for the whole text family (I-014)
    ///
    /// This control used to count **bytes** while `LineEdit` counted characters, so the same
    /// limit value meant different things in two fields of one form. The limit is what the user
    /// sees and what a counter reports, so it is a character budget here too. Truncation goes
    /// through [`byte_index_of_char`], which keeps the cut on a character boundary — the panic the
    /// old byte-based `String::truncate` could cause is impossible by construction now.
    pub fn set_max_length(&mut self, max: usize) {
        let previous = self.max_length;
        self.max_length = max;
        if max > 0 && self.text.chars().count() > max {
            let boundary = byte_index_of_char(&self.text, max);
            self.text.truncate(boundary);
            self.line_spans_cache.borrow_mut().take();
            self.pending_line_edit.set(None);
            self.display_rows_cache.borrow_mut().take();
            self.pending_row_edit.set(None);
            self.revision = self.revision.wrapping_add(1);
            // The caret must stay on a boundary inside what is left.
            self.cursor_pos = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
            // A shrinking limit makes every longer value unreachable, so the
            // history that could restore one is cleared and the shared target is
            // re-synced to the truncated value.
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.clear();
            self.changed.emit();
            // The caret may have been clamped and the history cleared: report both.
            self.notify_caret_change();
            self.refresh_edit_capabilities();
            self.base.request_redraw();
        } else if previous != max {
            self.base.request_redraw();
        }
    }

    /// Returns whether the text area is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Sets the read-only state.
    pub fn set_read_only(&mut self, ro: bool) {
        if self.read_only != ro {
            self.read_only = ro;
            self.base.request_redraw();
        }
    }

    /// Returns the placeholder text shown when the text is empty.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text.
    pub fn set_placeholder(&mut self, text: String) {
        if self.placeholder != text {
            self.placeholder = text;
            self.base.request_redraw();
        }
    }

    /// Inserts a string at the caret, replacing the selection when there is one.
    ///
    /// # Why this is not a loop over [`Self::insert`]
    ///
    /// A per-character loop would file one undo snapshot per character, so undoing a paste would
    /// take as many presses as the paste had characters. It would also re-apply the max-length
    /// guard against a value that grows as it goes, silently truncating a paste in a way this
    /// control gives the user no clue about. One snapshot and one guard for the whole insertion is
    /// what a user means by "paste".
    pub fn insert_str(&mut self, text: &str) {
        self.insert_text_in_place(text);
    }

    /// Copies the selected text to the platform clipboard, returning it.
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

    /// The byte range of the word around `offset`, for a double-click selection.
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

    /// Routes platform-committed text into the ordinary editing path (D09-INPUT-01).
    ///
    /// The desktop backends deliver a printable character or an IME commit as
    /// `Event::TextInput`/`Event::ImeCommit`, not as a `KeyPress`. The `KeyPress` printable arm
    /// above handles exactly one character, so a whole committed string — a paste-like `TextInput`
    /// or an IME word — needs the multi-character `insert_str`, which is the same entry point that
    /// owns selection replacement, `max_length`, undo and the `changed` signal. Control characters
    /// are filtered out here rather than inside `insert_str`, because one unexpected control code
    /// must not discard the printable text committed alongside it.
    pub(crate) fn insert_committed_text(&mut self, text: &str) {
        let printable: String = text.chars().filter(|c| !c.is_control()).collect();
        if printable.is_empty() {
            return;
        }
        self.insert_str(&printable);
    }

    /// Deletes the character after the caret, or the selection when there is one.
    ///
    /// The counterpart of [`Self::delete_char`]. Without it the `Delete` key was simply not
    /// handled: it fell through to the character-input catch-all, where key code 46 is `.`, so
    /// pressing Delete typed a full stop and deleted nothing.
    pub fn delete_forward(&mut self) {
        if let Some((start, end)) = self.selection_range() {
            self.delete_range(start, end);
            return;
        }
        if self.cursor_pos >= self.text.len() {
            return;
        }
        let start = floor_char_boundary(&self.text, self.cursor_pos);
        let end = self.next_char_boundary();
        self.delete_range(start, end);
    }
}

impl TextArea {
    /// Reverts the most recent text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise emits `changed` and clamps the caret to the restored length.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the most recently undone text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise emits `changed` and clamps the caret to the restored length.
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
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.line_spans_cache.borrow_mut().take();
        self.pending_line_edit.set(None);
        self.display_rows_cache.borrow_mut().take();
        self.pending_row_edit.set(None);
        self.revision = self.revision.wrapping_add(1);
        self.cursor_pos = self.cursor_pos.min(self.text.len());
        self.restoring_history = false;
        self.changed.emit();
        self.notify_caret_change();
        self.base.request_redraw();
    }

    // ── FindReplaceBar wiring (COMP-A8) ──

    /// Attaches `bar` to this area (COMP-A8).
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
            .unwrap_or(self.cursor_pos);
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
            let from = selection.map_or(self.cursor_pos, |(_, end)| end);
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

    /// Replaces every match in one edit, which is one undo step through the area's own path.
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
        self.cursor_pos = end;
        self.scroll_caret_into_view();
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Counts the matches of `query` and writes `(total, current)` into the attached bar.
    #[cfg(full_widgets)]
    fn report_find_replace_matches(&self, query: &str, match_case: bool) {
        let matches = self.find_replace_matches(query, match_case);
        let anchor = self.selection_range().map_or(self.cursor_pos, |(start, _)| start);
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
