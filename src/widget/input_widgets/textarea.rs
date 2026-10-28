// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TextArea widget — multi-line text input (BLUE13 R2.5).
#[cfg(full_widgets)]
use crate::compat::{lock, Arc, Mutex};
use crate::compat::{Box, Rc, RefCell, String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
use crate::event::wheel::WheelScroll;
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
#[cfg(full_widgets)]
use crate::signal::ConnectionScope;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::UndoStack;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
use crate::widget::companions::FindReplaceBar;
use crate::widget::input_widgets::input_companions::{ScrollOrientation, Scrollbar};
use crate::widget::metrics::{active_profile, dimensions, effective_font, profile_metrics};
use crate::widget::text_utils::{byte_index_of_char, floor_char_boundary};
use crate::widget::widget_trait::InputMethodQuery;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::Cell;
use std::collections::HashMap;

/// One logical line: `(start, content_end, end)` byte offsets.
type LineSpan = (usize, usize, usize);
/// A line table shared between readers.
type LineSpanTable = Rc<Vec<LineSpan>>;

/// The splice record the text family shares (I-001).
///
/// `TextArea` used to carry this record privately; it now lives in the undo module so `LineEdit`,
/// the combo-box editor, the search fields and the inline editor file the same shape. The alias
/// keeps the in-file call sites and tests reading naturally.
type TextDeltaCommand = crate::undo::TextSpliceCommand;

/// Multi-line text input widget.
///
/// Supports multi-line text storage with a byte-index cursor, character-level
/// insertion and deletion, placeholder display, read-only mode, and word-wrap
/// toggling. Emits a `changed` signal whenever the text content changes.
pub struct TextArea {
    base: BaseWidget,
    /// Text content (lines separated by '\n').
    text: String,
    /// Cursor position (byte index into text).
    cursor_pos: usize,
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
    selection_anchor: Option<usize>,
    /// Whether a pointer drag is currently extending a selection.
    ///
    /// Set by [`TextArea::press_at`], cleared by [`TextArea::end_drag`]. It exists because
    /// `MouseMove` is delivered for a bare hover too — without the flag, a caret the user never
    /// grabbed would follow the pointer across the control.
    dragging_selection: bool,
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
    first_visible_line: usize,
    /// Maximum text length (0 = unlimited).
    max_length: usize,
    /// Whether the widget is read-only.
    read_only: bool,
    /// Placeholder text when empty.
    placeholder: String,
    /// Whether this widget currently holds keyboard focus.
    focused: bool,
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
    line_spans_cache: RefCell<Option<LineSpanTable>>,
    /// Whether long lines wrap at word boundaries instead of being clipped.
    ///
    /// Off by default, which keeps the original one-row-per-logical-line layout pixel for pixel.
    soft_wrap: bool,
    /// Extra inset, in logical pixels, between the field's border and its text.
    ///
    /// Zero by default; a positive margin moves the text, the selection and the hit test together.
    content_margin: i32,
    /// The largest number of logical lines the value may hold, or `None` for no limit.
    ///
    /// When an insertion pushes the document past the limit, the oldest lines are dropped — the
    /// ring-buffer behaviour a log or console pane needs.
    max_block_count: Option<usize>,
    /// Horizontal scroll offset in logical pixels, for an unwrapped line wider than the field.
    ///
    /// Zero unless the caret has been driven past the field's trailing edge, so an ordinary
    /// document never scrolls sideways.
    scroll_x: i32,
    /// Bumped on every value change, so the display-row and prefix-width caches can identify the
    /// document in O(1).
    revision: u64,
    /// Memoised display rows: the soft-wrapped rows the paint, caret and hit test share.
    display_rows_cache: RefCell<Option<DisplayRowCache>>,
    /// The splice the next [`Self::display_rows`] call may patch in place (I-006).
    ///
    /// Only one splice is described at a time. A second edit before the rows are read clears the
    /// hint, which falls back to the whole-table path rather than composing two unrelated deltas.
    pending_row_edit: Cell<Option<PendingEdit>>,
    /// The splice the next [`Self::line_spans`] call may patch in place (I-P10).
    ///
    /// The line table has the same incremental rule as the display rows: an edit that inserts or
    /// removes bytes changes only the lines it touched and shifts the offsets after them. A second
    /// splice before the table is read clears the hint and falls back to a full rescan.
    pending_line_edit: Cell<Option<PendingEdit>>,
    /// Character boundaries of the row the pointer last hit-tested (I-P09).
    line_advance_cache: RefCell<Option<LineAdvanceCache>>,
    /// The renderer's line height for the current font, measured once (I-P09).
    line_height_cache: RefCell<Option<(Font, u32)>>,
    /// Measured advances for the selection band's two edges, keyed by the revision.
    ///
    /// The band used to re-shape its line prefix and its selected run on every frame, which is two
    /// full text measurements per visible line per paint. The cached values are the *same*
    /// measurements — an entry is only reused while the document is unchanged — so the band's
    /// pixels do not move.
    band_width_cache: RefCell<Option<BandWidthCache>>,
    /// The caret's blink state, advanced by [`TextArea::tick`] (I-116).
    cursor_blink: crate::style::CursorBlink,
    /// Whether the pointer is currently dragging the scrollbar thumb (I-118).
    scrollbar_drag: bool,
    /// The standard scrollbar the track, thumb and drag offset come from (I-128).
    ///
    /// The companion owns the proportional thumb geometry and the pointer-to-offset map; this
    /// control still paints it in its own ink because the companion's painter reads only palette
    /// roles and would move the field's themed scrollbar to a different colour.
    scrollbar: Scrollbar,
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

/// One display row: the byte range of the value it paints.
///
/// A logical line is one row unless soft wrap is on and the line is wider than the field, in which
/// case the same line contributes several rows. `end` excludes the line's terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisplayRow {
    /// First byte of the row's text.
    start: usize,
    /// One past the last byte of the row's text, excluding a newline.
    end: usize,
}

/// Memoised display rows, keyed by the inputs the wrapping reads.
struct DisplayRowCache {
    revision: u64,
    width: u32,
    soft_wrap: bool,
    margin: i32,
    /// The full font the rows were wrapped with, not only its size.
    ///
    /// Wrapping is a width measurement, so a face or weight change at the same point size moves
    /// every row boundary. Keying the cache on `size().to_bits()` alone left it answering from the
    /// previous face (I-07).
    font: Font,
    rows: Rc<Vec<DisplayRow>>,
    /// The logical spans the rows were built from, retained so the next edit can reuse the
    /// untouched paragraphs at either end (I-006).
    spans: Rc<Vec<LineSpan>>,
    /// One entry per logical paragraph, parallel to `spans`.
    paragraphs: Vec<ParagraphRow>,
    /// Index into `rows` of each paragraph's first row, parallel to `spans`.
    row_starts: Vec<usize>,
    /// Document byte length when the cache was built, used to validate a pending splice.
    text_len: usize,
}

/// One logical paragraph's wrapped rows, keyed by its own content revision and the style inputs.
///
/// A soft-wrap pass used to re-measure every glyph of the document on any edit. A paragraph's own
/// text, width and font decide its breaks, so an unchanged paragraph hands back its previous rows.
/// The `Rc` keeps reusing an untouched paragraph to a refcount bump.
#[derive(Clone)]
struct ParagraphRow {
    /// Fingerprint of the paragraph's visible text.
    content_revision: u64,
    /// The width the paragraph was wrapped at.
    width: u32,
    /// Whether soft wrap was on when the rows were built.
    soft_wrap: bool,
    font: Font,
    /// `(relative start, relative end)` pairs inside the paragraph's content.
    rows: Rc<Vec<(usize, usize)>>,
}

/// A byte splice that [`TextArea::display_rows`] can apply without rebuilding every paragraph.
#[derive(Clone, Copy, Debug)]
struct PendingEdit {
    /// Byte offset in the new text where the changed run begins.
    start: usize,
    /// Bytes removed at `start` from the pre-edit text.
    removed: usize,
    /// Bytes inserted at `start` in the new text.
    inserted: usize,
}

/// A cheap content hash for one line's wrap-cache key.
fn line_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The logical paragraph containing `offset`, clamped to a real span.
fn paragraph_index_at(spans: &[LineSpan], offset: usize) -> usize {
    spans.partition_point(|&(start, _, _)| start <= offset).saturating_sub(1)
}

/// Breaks one paragraph's content into `(relative start, relative end)` rows.
///
/// This is the same measurement the whole-table path always made, expressed relative to the
/// paragraph start so an unchanged row can be moved when earlier text shifts.
fn wrap_paragraph_rows(
    content: &str,
    width: u32,
    font: &Font,
    context: &RenderContext<'_>,
) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    let mut row_start = 0usize;
    let mut row_width = 0.0f32;
    let mut last_break: Option<usize> = None;
    for (offset, ch) in content.char_indices() {
        let mut glyph = [0u8; 4];
        let advance = context.measure_text(ch.encode_utf8(&mut glyph), font).width as f32;
        if row_width + advance > width as f32 && offset > row_start {
            let break_at = last_break
                .filter(|boundary| *boundary > row_start && *boundary <= offset)
                .unwrap_or(offset);
            rows.push((row_start, break_at));
            row_start = break_at;
            row_width = 0.0;
            for (_, carried) in content[row_start..offset].char_indices() {
                let mut glyph = [0u8; 4];
                row_width +=
                    context.measure_text(carried.encode_utf8(&mut glyph), font).width as f32;
            }
            last_break = None;
        }
        if ch.is_whitespace() || ch == '-' {
            last_break = Some(offset + ch.len_utf8());
        }
        row_width += advance;
    }
    rows.push((row_start, content.len()));
    rows
}

/// Memoised selection-band advances, keyed by the document revision and the full font.
struct BandWidthCache {
    revision: u64,
    font: Font,
    /// Measured `text[from..to]` advances, keyed by the byte range.
    ///
    /// A hash lookup replaces the old linear `Vec` scan (I-P61). The entry count is bounded so a
    /// long-lived document cannot grow the cache without limit; a full cache is dropped rather
    /// than evicted one entry at a time, which keeps the bookkeeping trivial.
    entries: HashMap<(usize, usize), i32>,
}

/// Per-character advances for one display row, keyed by the document revision and font (I-P09).
///
/// The pointer hit test used to shape every glyph of the row on every press and drag step. The
/// row is immutable for a given revision, so the cumulative boundaries are built once and read
/// by every following click; a caret move does not invalidate them.
struct LineAdvanceCache {
    revision: u64,
    font: Font,
    start: usize,
    end: usize,
    /// `(byte offset relative to `start`, cumulative advance)` at every character boundary.
    cuts: Vec<(usize, i32)>,
}

/// Upper bound on memoised selection-band advances (I-P61).
const BAND_WIDTH_CACHE_LIMIT: usize = 256;

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
    fn notify_caret_change(&mut self) {
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
    fn line_spans(&self) -> LineSpanTable {
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
    fn normalize_selection(&mut self) {
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
    fn replace_selection(&mut self) -> bool {
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
    fn record_edit(&mut self, before: String) {
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

    /// The first logical line the caret's row shows, i.e. how far the viewport is scrolled.
    pub fn first_visible_line(&self) -> usize {
        self.first_visible_line
    }

    /// How many whole lines fit in the control's painting area.
    ///
    /// Derived from the control's own height and the same [`LINE_H`] the paint path advances by, so
    /// "a line fits" means the same thing to the scroll arithmetic and to the rows that are drawn.
    /// Always at least one, because a control shorter than a single line still has to show something
    /// rather than dividing by zero or scrolling past everything.
    fn visible_line_count(&self) -> usize {
        let first_line_top = FIRST_LINE_INSET + (LINE_H - self.line_box_height()) / 2;
        let usable = self.geometry().height as i32 - first_line_top;
        ((usable / LINE_H).max(1)) as usize
    }

    /// The logical line a caret at `offset` sits on.
    ///
    /// Used by the tests and retained as documentation of the line index space; the paint and hit
    /// test now read [`Self::display_row_of`], which accounts for soft wrap.
    #[cfg(test)]
    fn line_index_of(&self, offset: usize) -> usize {
        let caret = floor_char_boundary(&self.text, offset.min(self.text.len()));
        let spans = self.line_spans();
        spans.partition_point(|&(start, _, _)| start <= caret).saturating_sub(1)
    }

    /// Scrolls the viewport the minimum amount that brings the caret's line into view.
    ///
    /// # The defect this fixes
    ///
    /// The paint path already stopped at the last whole line it could fit, so a value taller than
    /// the control did not overrun its own rectangle. What was missing was any way to reach the lines
    /// it skipped: typing past the bottom row wrote into a line the user could not see, and there was
    /// no key or gesture that brought it into view. That is the "invisible but editable" state the
    /// probe measured — the ink box stayed inside the field while the caret walked off the bottom.
    ///
    /// The scroll is *minimal*: the viewport moves only when the caret is above or below the window
    /// it currently shows, so reading text near the top does not make the field jump.
    fn scroll_caret_into_view(&mut self) {
        let caret_row = self.display_row_of(self.cursor_pos);
        self.first_visible_line = crate::widget::text_utils::first_visible_row_for(
            caret_row,
            self.first_visible_line,
            self.visible_line_count(),
        );
        self.ensure_caret_visible();
    }

    /// Scrolls the viewport by `lines` rows toward the end of the document.
    ///
    /// Positive values reveal later lines, negative values reveal earlier ones. The result is
    /// clamped so the first visible line can never scroll past the last window of the document,
    /// which keeps a wheel notch at either end from leaving the field showing blank rows.
    /// Returns whether the viewport actually moved, so the caller repaints only on a change.
    fn scroll_lines(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let max_first = self.max_first_visible_line();
        let next =
            (self.first_visible_line as i64 + lines as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_line {
            return false;
        }
        self.first_visible_line = next;
        true
    }

    /// The largest first-visible line that still fills the viewport, i.e. the scroll clamp.
    fn max_first_visible_line(&self) -> usize {
        self.display_rows().len().saturating_sub(self.visible_line_count())
    }

    /// Places `line` at the top of the viewport, clamped so the field never scrolls past its end.
    ///
    /// The same clamp [`Self::scroll_lines`] applies: a line past the last window is pinned to
    /// the last window, and a line that is already first is a no-op with no repaint.
    pub fn set_first_visible_line(&mut self, line: usize) {
        let next = line.min(self.max_first_visible_line());
        if next == self.first_visible_line {
            return;
        }
        self.first_visible_line = next;
        self.base.request_redraw();
    }

    /// Scrolls the viewport so `line` is the first visible line.
    ///
    /// A convenience spelling of [`Self::set_first_visible_line`] for hosts that mirror an
    /// external view: the line is clamped to the document's last window rather than being
    /// accepted as an arbitrary scroll position.
    pub fn scroll_to_line(&mut self, line: usize) {
        self.set_first_visible_line(line);
    }

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

    /// The width a display row has for text, after the field's padding and the content margin.
    fn content_width(&self) -> u32 {
        let inset = dimensions::TEXT_FIELD_PADDING_H as i32 + self.content_margin;
        self.geometry().width.saturating_sub((inset * 2).max(0) as u32)
    }

    /// The display rows the paint, the caret and the hit test all read.
    ///
    /// With soft wrap off this is exactly one row per logical line, which is what keeps the
    /// existing layout and its pixels unchanged. The cache is keyed by the mutation counter rather
    /// than the document text, so a hit is an integer comparison.
    ///
    /// When only the text changed, a pending splice lets this rebuild solely the paragraphs it
    /// touched and reuse the untouched ends (I-006). A width, wrap, margin or font change takes
    /// the whole-table path, because it moves every paragraph.
    fn display_rows(&self) -> Rc<Vec<DisplayRow>> {
        let width = self.content_width();
        let font = self.text_font().clone();
        {
            let cache = self.display_rows_cache.borrow();
            if let Some(cache) = cache.as_ref() {
                if cache.revision == self.revision
                    && cache.width == width
                    && cache.soft_wrap == self.soft_wrap
                    && cache.margin == self.content_margin
                    && cache.font == font
                {
                    return Rc::clone(&cache.rows);
                }
            }
        }
        let pending = self.pending_row_edit.take();
        let incremental = {
            let cache = self.display_rows_cache.borrow();
            match (cache.as_ref(), pending) {
                (Some(cached), Some(edit))
                    if cached.width == width
                        && cached.soft_wrap == self.soft_wrap
                        && cached.margin == self.content_margin
                        && cached.font == font
                        && edit.start <= self.text.len()
                        && edit.start + edit.inserted <= self.text.len()
                        && edit.start + edit.removed <= cached.text_len
                        && cached.text_len + edit.inserted == self.text.len() + edit.removed =>
                {
                    self.incremental_display_rows(cached, edit, width, &font)
                }
                _ => None,
            }
        };
        let next = incremental.unwrap_or_else(|| {
            let cache = self.display_rows_cache.borrow();
            self.build_display_rows(width, &font, cache.as_ref())
        });
        let rows = Rc::clone(&next.rows);
        *self.display_rows_cache.borrow_mut() = Some(next);
        rows
    }

    /// Rebuilds only the paragraphs a splice touched, reusing the untouched ends (I-006).
    ///
    /// Returns `None` when the old and new line tables do not line up — for example a splice that
    /// removes the terminator between two paragraphs and therefore merges them. The caller then
    /// takes the whole-table path, so the returned rows always match a full rebuild byte for byte.
    fn incremental_display_rows(
        &self,
        cached: &DisplayRowCache,
        edit: PendingEdit,
        width: u32,
        font: &Font,
    ) -> Option<DisplayRowCache> {
        let spans = self.line_spans();
        let old_spans = &cached.spans;
        let old_count = old_spans.len();
        let new_count = spans.len();
        let first_old = paragraph_index_at(old_spans, edit.start);
        let last_old = if edit.removed == 0 {
            first_old
        } else {
            paragraph_index_at(old_spans, edit.start + edit.removed - 1)
        };
        let prefix = first_old.min(old_count).min(new_count);
        let suffix = old_count.saturating_sub(last_old + 1);
        if prefix + suffix > new_count {
            return None;
        }
        let new_suffix_start = new_count - suffix;
        let old_suffix_start = last_old + 1;
        if new_suffix_start < prefix {
            return None;
        }
        // The untouched tail must still begin at the same paragraph boundary, shifted by the
        // splice. Removing the terminator that separated the changed run from the tail merges
        // the two paragraphs, and that is not a reusable suffix.
        if suffix > 0 {
            let delta = edit.inserted as isize - edit.removed as isize;
            let expected = old_spans[old_suffix_start].0 as isize + delta;
            if expected < 0 || spans[new_suffix_start].0 as isize != expected {
                return None;
            }
        }

        let mut rows: Vec<DisplayRow> = Vec::new();
        let mut paragraphs: Vec<ParagraphRow> = Vec::with_capacity(new_count);
        let mut row_starts: Vec<usize> = Vec::with_capacity(new_count);

        // The prefix is byte-for-byte untouched, so its absolute rows can be copied as they are.
        let prefix_row_end = cached.row_starts.get(prefix).copied().unwrap_or(cached.rows.len());
        rows.extend_from_slice(&cached.rows[..prefix_row_end.min(cached.rows.len())]);
        row_starts.extend_from_slice(cached.row_starts.get(..prefix).unwrap_or(&[]));
        paragraphs.extend(cached.paragraphs.iter().take(prefix).cloned());

        // Only the middle is measured. Every untouched paragraph keeps its cached rows.
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        for &(start, content_end, _) in spans.iter().take(new_suffix_start).skip(prefix) {
            row_starts.push(rows.len());
            let content = &self.text[start..content_end];
            let entry = self.wrap_paragraph(width, font, &context, content, line_hash(content));
            for &(rel_start, rel_end) in entry.rows.iter() {
                rows.push(DisplayRow { start: start + rel_start, end: start + rel_end });
            }
            paragraphs.push(entry);
        }

        // The tail's content is unchanged; its relative rows shift with the new span start.
        for offset in 0..suffix {
            let new_index = new_suffix_start + offset;
            let old_index = old_suffix_start + offset;
            let start = spans[new_index].0;
            row_starts.push(rows.len());
            let entry = cached.paragraphs.get(old_index)?.clone();
            for &(rel_start, rel_end) in entry.rows.iter() {
                rows.push(DisplayRow { start: start + rel_start, end: start + rel_end });
            }
            paragraphs.push(entry);
        }

        if rows.is_empty() {
            rows.push(DisplayRow { start: 0, end: 0 });
        }
        Some(DisplayRowCache {
            revision: self.revision,
            width,
            soft_wrap: self.soft_wrap,
            margin: self.content_margin,
            font: font.clone(),
            rows: Rc::new(rows),
            spans,
            paragraphs,
            row_starts,
            text_len: self.text.len(),
        })
    }

    /// Builds the whole display-row table, wrapping long paragraphs when soft wrap is on.
    ///
    /// This is the width/style-change fallback. It may reuse a paragraph whose content, width and
    /// font are unchanged from `previous`, but it visits every logical paragraph.
    fn build_display_rows(
        &self,
        width: u32,
        font: &Font,
        previous: Option<&DisplayRowCache>,
    ) -> DisplayRowCache {
        let spans = self.line_spans();
        let reusable = previous.filter(|cache| {
            cache.width == width
                && cache.soft_wrap == self.soft_wrap
                && cache.margin == self.content_margin
                && cache.font == *font
        });
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        let mut rows = Vec::new();
        let mut paragraphs = Vec::with_capacity(spans.len());
        let mut row_starts = Vec::with_capacity(spans.len());
        for (index, &(start, content_end, _)) in spans.iter().enumerate() {
            row_starts.push(rows.len());
            let content = &self.text[start..content_end];
            let content_revision = line_hash(content);
            let reused = reusable
                .and_then(|cache| cache.paragraphs.get(index))
                .filter(|entry| {
                    entry.content_revision == content_revision
                        && entry.width == width
                        && entry.soft_wrap == self.soft_wrap
                        && entry.font == *font
                })
                .cloned();
            let entry = reused.unwrap_or_else(|| {
                self.wrap_paragraph(width, font, &context, content, content_revision)
            });
            for &(rel_start, rel_end) in entry.rows.iter() {
                rows.push(DisplayRow { start: start + rel_start, end: start + rel_end });
            }
            paragraphs.push(entry);
        }
        if rows.is_empty() {
            rows.push(DisplayRow { start: 0, end: 0 });
        }
        DisplayRowCache {
            revision: self.revision,
            width,
            soft_wrap: self.soft_wrap,
            margin: self.content_margin,
            font: font.clone(),
            rows: Rc::new(rows),
            spans,
            paragraphs,
            row_starts,
            text_len: self.text.len(),
        }
    }

    /// Measures one logical paragraph, returning its relative rows and cache key.
    fn wrap_paragraph(
        &self,
        width: u32,
        font: &Font,
        context: &RenderContext<'_>,
        content: &str,
        content_revision: u64,
    ) -> ParagraphRow {
        let rows = if !self.soft_wrap || width == 0 || content.is_empty() {
            Rc::new(vec![(0usize, content.len())])
        } else {
            Rc::new(wrap_paragraph_rows(content, width, font, context))
        };
        ParagraphRow {
            content_revision,
            width,
            soft_wrap: self.soft_wrap,
            font: font.clone(),
            rows,
        }
    }

    /// The display row a caret at `offset` belongs to.
    fn display_row_of(&self, offset: usize) -> usize {
        let rows = self.display_rows();
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        rows.iter().rposition(|row| row.start <= offset).unwrap_or(0)
    }

    /// The display row a viewport row paints.
    fn display_row(&self, row: usize) -> Option<DisplayRow> {
        self.display_rows().get(self.first_visible_line + row).copied()
    }

    /// Scrolls the value horizontally so the caret is visible.
    ///
    /// Soft-wrapped rows never scroll sideways, because they have no overflow to reveal.
    fn ensure_caret_visible(&mut self) {
        if self.soft_wrap {
            self.scroll_x = 0;
            return;
        }
        let width = self.content_width() as i32;
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        let line_start = self.text[..caret].rfind('\n').map(|index| index + 1).unwrap_or(0);
        let advance = self.text_advance(&self.text[line_start..caret]);
        let room = 8.min(width / 4);
        if advance - self.scroll_x > width - room {
            self.scroll_x = (advance - width + room).max(0);
        } else if advance < self.scroll_x {
            self.scroll_x = advance;
        }
        self.scroll_x = self.scroll_x.max(0);
    }

    /// The byte offset of the character *before* the caret.
    ///
    /// Delegates to the shared [`crate::widget::text_utils::previous_char_boundary`] (WDGT-11).
    fn previous_char_boundary(&self) -> usize {
        crate::widget::text_utils::previous_char_boundary(&self.text, self.cursor_pos)
    }

    /// The byte offset one character after the caret.
    fn next_char_boundary(&self) -> usize {
        crate::widget::text_utils::next_char_boundary(&self.text, self.cursor_pos)
    }

    /// The caret's own line start, in bytes.
    ///
    /// A caret sitting exactly after a `\n` belongs to the *following* line (which may be empty),
    /// which is what makes `End` then `Home` land on the line the user sees rather than jumping to
    /// the previous one.
    fn line_start_of_caret(&self) -> usize {
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        self.text[..caret].rfind('\n').map(|i| i + 1).unwrap_or(0)
    }

    /// The offset just past the caret's own line, **excluding** that line's terminator.
    ///
    /// Excluding the newline is what makes `End` put the caret at the end of the visible line
    /// rather than at the start of the next one.
    fn line_end_of_caret(&self) -> usize {
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        match self.text[caret..].find('\n') {
            Some(offset) => caret + offset,
            None => self.text.len(),
        }
    }

    /// The caret one *logical* line up, at the same column where that column exists.
    ///
    /// The column counts characters, not bytes, so a line of multi-byte text does not pull the
    /// caret past the matching glyph. A destination line shorter than the column saturates at its
    /// end, which is the conventional behaviour: one Up from a long line onto a short one lands at
    /// the short line's end rather than refusing to move. With no line above, the caret keeps its
    /// own position, so the key is never silently swallowed.
    fn line_up(&self) -> usize {
        let start = self.line_start_of_caret();
        if start == 0 {
            return self.cursor_pos;
        }
        let column = self.column_of_caret(start);
        // `start - 1` is the newline terminating the line above, so that line ends there.
        let previous_start = self.text[..start - 1].rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.byte_index_at_column(previous_start, start - 1, column)
    }

    /// The caret one *logical* line down, at the same column where that column exists.
    ///
    /// See [`Self::line_up`] for the column convention and the saturation rule.
    fn line_down(&self) -> usize {
        let end = self.line_end_of_caret();
        if end >= self.text.len() {
            return self.cursor_pos;
        }
        let column = self.column_of_caret(self.line_start_of_caret());
        // `end` is this line's newline, so the next line starts one past it.
        let next_start = end + 1;
        let next_end = match self.text[next_start..].find('\n') {
            Some(offset) => next_start + offset,
            None => self.text.len(),
        };
        self.byte_index_at_column(next_start, next_end, column)
    }

    /// The caret's column within its line, counted in **characters**.
    ///
    /// Character-counted rather than byte-counted because the column is compared against another
    /// line's glyphs: a byte count would over-count a multi-byte line and walk the caret further
    /// than the vertical movement covers.
    fn column_of_caret(&self, line_start: usize) -> usize {
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        self.text[line_start..caret].chars().count()
    }

    /// The caret's column within its own display row, counted in characters.
    ///
    /// With soft wrap a logical line can occupy several rows, so the column has to be measured
    /// from the row's start rather than from the line's.
    fn column_in_display_row(&self, offset: usize) -> usize {
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        let row_start =
            self.display_rows().iter().rfind(|row| row.start <= offset).map_or(0, |row| row.start);
        self.text[row_start..offset].chars().count()
    }

    /// The byte offset of `column` characters into `text[line_start..line_end]`, saturating at
    /// `line_end` when the line is shorter.
    fn byte_index_at_column(&self, line_start: usize, line_end: usize, column: usize) -> usize {
        self.text[line_start..line_end]
            .char_indices()
            .nth(column)
            .map(|(offset, _)| line_start + offset)
            .unwrap_or(line_end)
    }

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

    /// The byte range of the word around `offset`, for a double-click selection.
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

    /// Routes platform-committed text into the ordinary editing path (D09-INPUT-01).
    ///
    /// The desktop backends deliver a printable character or an IME commit as
    /// `Event::TextInput`/`Event::ImeCommit`, not as a `KeyPress`. The `KeyPress` printable arm
    /// above handles exactly one character, so a whole committed string — a paste-like `TextInput`
    /// or an IME word — needs the multi-character `insert_str`, which is the same entry point that
    /// owns selection replacement, `max_length`, undo and the `changed` signal. Control characters
    /// are filtered out here rather than inside `insert_str`, because one unexpected control code
    /// must not discard the printable text committed alongside it.
    fn insert_committed_text(&mut self, text: &str) {
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

    /// Starts a pointer selection at the character `index`, extending from the anchor when `Shift`
    /// is held.
    ///
    /// # Why a press must not normalise
    ///
    /// A press landing exactly on the caret produces a zero-width anchor/caret pair, and collapsing
    /// that is right for a *result* — but this is the *start* of a gesture, and the anchor is what
    /// the following drag grows from. Normalising here would delete the anchor the press just set,
    /// so dragging back over the value would select nothing. The gesture's end normalises instead,
    /// in [`Self::end_drag`].
    pub fn press_at(&mut self, index: usize, modifiers: crate::shortcut::Modifiers) {
        let index = floor_char_boundary(&self.text, index.min(self.text.len()));
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            // Extend: keep the anchor (adopting the caret when the gesture is new) and move the
            // caret to where the pointer went down. Leaving the caret alone would make a
            // shift-press a no-op that only re-anchored, so shift-clicking to the other end of a
            // line would select nothing.
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor_pos);
            }
            self.cursor_pos = index;
        } else {
            // A plain press starts a new gesture, anchored where it landed.
            self.selection_anchor = Some(index);
            self.cursor_pos = index;
        }
        self.dragging_selection = true;
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Extends an in-progress pointer selection to the character at `index`.
    ///
    /// Only a gesture that began with [`Self::press_at`] continues here; a bare hover would
    /// otherwise move a caret the user never grabbed.
    pub fn drag_to(&mut self, index: usize) {
        if !self.dragging_selection {
            return;
        }
        self.cursor_pos = floor_char_boundary(&self.text, index.min(self.text.len()));
        self.normalize_selection();
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Ends a pointer selection, collapsing a gesture that never moved. Returns whether one was in
    /// progress.
    pub fn end_drag(&mut self) -> bool {
        let was_dragging = core::mem::replace(&mut self.dragging_selection, false);
        if was_dragging {
            self.normalize_selection();
            self.notify_caret_change();
        }
        was_dragging
    }

    /// Whether `pos` is inside the control's painted rectangle.
    ///
    /// A text area paints its whole `geometry()` — unlike a single-line field, whose content is a
    /// centred band — so the hit test is the control's own rectangle and no second box can drift
    /// from the ink.
    fn hit_test(&self, pos: Point) -> bool {
        self.geometry().contains_point(pos)
    }

    /// The byte offset the point `pos` falls on.
    ///
    /// # How a pixel becomes an offset
    ///
    /// The line is whichever [`LINE_H`] band `y` lands in — that is a layout choice, and the grid is
    /// what the paint path advances by, so the two agree. The **column** within the line is measured
    /// rather than divided: [`Self::column_at_x`] walks the line and measures each prefix, which is
    /// the only way a click can land on the character the user is pointing at when the face's glyphs
    /// have different widths.
    ///
    /// The result is clamped to the end of the *line* it resolved to, not to the end of the value:
    /// clicking to the right of a short line must put the caret at that line's end, not on a later
    /// line. The final snap to a character boundary keeps `&text[..caret]` total on multi-byte text.
    fn byte_index_at(&self, pos: Point) -> usize {
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;
        let text_top = rect.y + FIRST_LINE_INSET + (LINE_H - self.line_box_height()) / 2;

        let row = ((pos.y - text_top).max(0) / LINE_H) as usize;
        // The viewport row is translated to the display row it paints, so a click after scrolling
        // resolves to the row the user sees there rather than to the document's own nth row.
        let x = pos.x - origin_x + self.scroll_x - self.content_margin;

        // Walk the table the paint path also reads, so the answer is the same row the glyphs are
        // drawn on rather than a second model of where the newlines are.
        let Some(row) = self.display_row(row) else {
            return self.text.len();
        };
        let start = row.start;
        let content_end = row.end.min(self.text.len());
        start + self.column_at_x(start, content_end, x)
    }

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

impl Widget for TextArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Reports the caret an IME candidate window should anchor to while this area is focused.
    ///
    /// The caret's row is the one the paint path draws it on — the document line, shifted by the
    /// viewport's first visible line — and its x is the measured advance of the text before the
    /// caret on that line. The default `None` is still what every non-editable widget answers.
    fn input_method_query(&self) -> Option<InputMethodQuery> {
        Some(self.caret_query())
    }

    /// Turns a context-menu request into this area's signal; the host supplies the menu.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        self.context_menu_requested.emit(pos);
        true
    }

    /// Opts the area into the runtime's drag coordinator as a source.
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        Some(self)
    }

    /// Opts the area into the runtime's drag coordinator as a drop target.
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        Some(self)
    }

    fn size_hint(&self) -> Size {
        let line_count = if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        };
        let max_line_width = self.text.lines().map(|l| l.len() as u32).max().unwrap_or(0);
        let w = (max_line_width * 8 + 10).max(120);
        // A text area is a stack of line rows, so its content height grows with the text. The
        // *floor* under that stack is the active profile's text-field height: Material's own
        // historical minimum is 60 px (the control's three-row budget), while a compact profile
        // such as macOS lets an empty area start at its shorter field height. The profile never
        // makes the area taller than the text already asks for, so multi-line content is
        // unchanged on every profile.
        let field_height = profile_metrics(active_profile()).text_field_min_height;
        let material_field_height =
            profile_metrics(crate::widget::metrics::ControlMetricsProfile::Material)
                .text_field_min_height;
        let floor = if field_height < material_field_height { field_height } else { 60 };
        let h = (line_count as u32 * LINE_H as u32 + 10).max(floor);
        Size::new(w, h)
    }

    /// Advances the caret blink and reports whether another frame is owed (I-116).
    fn tick(&mut self, delta_ms: u32) -> bool {
        TextArea::tick(self, delta_ms)
    }

    /// A focused area keeps a blinking caret, so the host must keep scheduling frames (I-116).
    fn is_animating(&self) -> bool {
        self.focused
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "changed" => Some(EventSignalRef::unit("changed", &self.changed)),
            "selection_changed" => {
                Some(EventSignalRef::unit("selection_changed", &self.selection_changed))
            }
            "cursor_position_changed" => Some(EventSignalRef::mapped(
                "cursor_position_changed",
                &self.cursor_position_changed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "undo_available" => {
                Some(EventSignalRef::mapped("undo_available", &self.undo_available, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "redo_available" => {
                Some(EventSignalRef::mapped("redo_available", &self.redo_available, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| CapabilityValue::String(format!("{v:?}")),
            )),
            _ => None,
        }
    }
}

/// A selected run of text is the area's drag payload.
///
/// The item id and label both hold the text because [`DragPayload`] carries an opaque
/// identifier rather than a typed body; for `text/plain` the identifier *is* the content, and
/// using the same value for both fields means a target can read either without a rule about
/// which one is authoritative.
impl DragSource for TextArea {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.hit_test(pos) {
            return None;
        }
        let selected = self.selected_text();
        if selected.is_empty() {
            return None;
        }
        Some(DragPayload::new("text/plain", selected.clone()).with_label(selected).with_origin(pos))
    }

    /// A `Move` drop removes the source selection; every other effect keeps it.
    fn on_drag_finished(&mut self, effect: DropEffect) {
        if !effect.consumes_source() || self.read_only {
            return;
        }
        let before = self.text.clone();
        if self.replace_selection() {
            self.line_spans_cache.borrow_mut().take();
            self.pending_line_edit.set(None);
            self.record_edit(before);
            self.base.request_redraw();
        }
    }
}

/// The area accepts plain text and inserts it at the drop point.
impl DropTarget for TextArea {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        !self.read_only && payload.is_type("text/plain")
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) || !self.hit_test(pos) {
            return DropEffect::None;
        }
        let text =
            if payload.label.is_empty() { payload.item_id.clone() } else { payload.label.clone() };
        if text.is_empty() {
            return DropEffect::None;
        }
        self.cursor_pos = self.byte_index_at(pos);
        self.selection_anchor = None;
        self.insert_str(&text);
        self.base.request_redraw();
        DropEffect::Copy
    }
}

/// `TextArea`'s property contract.
impl WidgetProperties for TextArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "read_only" => Ok(CapabilityValue::Bool(self.is_read_only())),
            "max_length" => Ok(CapabilityValue::UInt(self.max_length() as u64)),
            "soft_wrap" => Ok(CapabilityValue::Bool(self.soft_wrap())),
            "content_margin" => Ok(CapabilityValue::UInt(self.content_margin().max(0) as u64)),
            "max_block_count" => match self.max_block_count() {
                Some(max) => Ok(CapabilityValue::UInt(max as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "scroll_x" => Ok(CapabilityValue::Int(self.scroll_x as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            "read_only" => {
                self.set_read_only(expect_bool(value)?);
                Ok(())
            }
            "max_length" => {
                self.set_max_length(expect_usize(value)?);
                Ok(())
            }
            "soft_wrap" => {
                self.set_soft_wrap(expect_bool(value)?);
                Ok(())
            }
            "content_margin" => {
                self.set_content_margin(expect_usize(value)? as i32);
                Ok(())
            }
            "max_block_count" => {
                match value {
                    CapabilityValue::Null => self.set_max_block_count(None),
                    other => self.set_max_block_count(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "scroll_x" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TEXT_AREA_PROPERTIES`.
        property_names_of![
            "text",
            "placeholder",
            "read_only",
            "max_length",
            "soft_wrap",
            "content_margin",
            "max_block_count",
            "scroll_x",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `text_area` publishes.
    ///
    /// `insert` and `delete_char` are the editing actions: they act at the caret, which
    /// the control owns, so they take no argument. On a fresh control `delete_char` is a
    /// no-op (the caret is already at position 0), but the command still ran, so it
    /// answers `Ok(())` rather than manufacturing an error for a legal invocation.
    ///
    /// `set_text`, `set_placeholder` and `set_read_only` assign state and need a
    /// payload, so they are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "insert" => {
                self.insert_char(' ');
                Ok(())
            }
            "delete_char" => {
                self.delete_char();
                Ok(())
            }
            "set_text" | "set_placeholder" | "set_read_only" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for TextArea {
    fn handle_event(&mut self, event: &Event) {
        // A queued find/replace intent is a shared write, applied before the event is routed so the
        // frame that follows the strip click already reflects it (COMP-A8).
        #[cfg(full_widgets)]
        self.poll_find_replace_bar();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained { .. } => {
                self.focused = true;
                // I-116: start the caret's blink cycle on focus.
                self.cursor_blink.start();
                self.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.cursor_blink.stop();
                self.request_redraw();
            }
            // Platform-committed text (D09-INPUT-01), routed into the ordinary insert path. A
            // read-only area is rejected here, exactly as the `KeyPress` printable arm is, and the
            // `is_enabled` guard above already ran.
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if self.read_only {
                    return;
                }
                self.insert_committed_text(text);
            }
            // Wheel: a positive `delta.y` moves the content down, so the viewport moves toward
            // the start of the document. `WheelScroll` converts the notch count to lines with the
            // desktop default of three lines per notch.
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                if self.scroll_lines(-scroll.lines_y) {
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                // The event carries the framework's wire bitmask; translate it once, here, so the
                // rest of this handler speaks `Modifiers` and nothing below re-derives a bit.
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                let primary = crate::shortcut::event_carries_primary(*modifiers);

                // I-117: select-all and copy remain available on a read-only area; undo/redo are
                // mutations and therefore sit behind the guard below.
                if primary && *key == 65 {
                    // Primary+A: select all. Handled on the control that owns the text, so a host
                    // whose window-level "select all" means something else does not swallow it.
                    self.select_all();
                    return;
                }
                if primary && *key == 67 {
                    // Primary+C: copy the selection. Allowed while read-only.
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
                    // Primary+X: copy, then delete the selection.
                    let _ = self.copy_selection_to_clipboard();
                    self.delete_char();
                    return;
                }
                if primary && *key == 86 {
                    // Primary+V: paste at the caret, replacing any selection.
                    self.paste_from_clipboard();
                    return;
                }
                match *key {
                    8 => {
                        // Backspace — one character back, or the whole selection.
                        self.delete_char();
                        self.request_redraw();
                    }
                    46 => {
                        // Delete — one character forward, or the whole selection.
                        self.delete_forward();
                        self.request_redraw();
                    }
                    13 => {
                        // Enter — insert a newline, over the selection if there is one.
                        self.insert_char('\n');
                        self.request_redraw();
                    }
                    9 => {
                        // Tab is content in a document editor: indentation is text, not a focus
                        // gesture, so it enters the value instead of being dropped as a control
                        // character.
                        self.insert_char('\t');
                        self.request_redraw();
                    }
                    33 => {
                        // Page Up — one window of display rows toward the start, keeping the
                        // caret's column within its row.
                        let rows = self.display_rows();
                        let page = self.visible_line_count().saturating_sub(1).max(1);
                        let row = self.display_row_of(self.cursor_pos).saturating_sub(page);
                        if let Some(display) = rows.get(row).copied() {
                            let target = self.byte_index_at_column(
                                display.start,
                                display.end,
                                self.column_in_display_row(self.cursor_pos),
                            );
                            self.select_with_modifiers(target, mods);
                        }
                    }
                    34 => {
                        // Page Down — the mirror of Page Up.
                        let rows = self.display_rows();
                        let page = self.visible_line_count().saturating_sub(1).max(1);
                        let row = (self.display_row_of(self.cursor_pos) + page)
                            .min(rows.len().saturating_sub(1));
                        if let Some(display) = rows.get(row).copied() {
                            let target = self.byte_index_at_column(
                                display.start,
                                display.end,
                                self.column_in_display_row(self.cursor_pos),
                            );
                            self.select_with_modifiers(target, mods);
                        }
                    }
                    37 => {
                        // Left — one character back, extending under Shift. At the start the caret
                        // cannot move, but a plain press still owes the user a deselect.
                        if self.cursor_pos > 0 {
                            let target = self.previous_char_boundary();
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.request_redraw();
                        }
                    }
                    39 => {
                        // Right — one character forward; see the Left arm.
                        if self.cursor_pos < self.text.len() {
                            let target = self.next_char_boundary();
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.request_redraw();
                        }
                    }
                    38 => {
                        // Up — one *logical* line, keeping the column. The multi-line movement this
                        // control has and a single-line field cannot: without it the only way to
                        // reach another line was Home then Left, repeated.
                        let target = self.line_up();
                        self.select_with_modifiers(target, mods);
                    }
                    40 => {
                        // Down — see the Up arm.
                        let target = self.line_down();
                        self.select_with_modifiers(target, mods);
                    }
                    36 => {
                        // Home — the start of the *line*, or of the *document* with Primary held.
                        // Plain Home jumping to the document start is what a single-line field
                        // does; in a multi-line one it makes the key useless for reaching the line
                        // the caret is already on.
                        let target = if primary { 0 } else { self.line_start_of_caret() };
                        self.select_with_modifiers(target, mods);
                    }
                    35 => {
                        // End — the end of the line, or of the document with Primary held.
                        let target =
                            if primary { self.text.len() } else { self.line_end_of_caret() };
                        self.select_with_modifiers(target, mods);
                    }
                    _ => {
                        // Character input. A control chord is not text, and neither is an unmapped
                        // non-printable key.
                        //
                        // # Why the three modifiers are tested one at a time
                        //
                        // `Modifiers::contains` compares the *value* of the mask, so the
                        // combined `contains(CTRL | ALT | META)` form is true only when all
                        // three are held at once — a single-Control chord fails it and the
                        // character goes through. That is the defect this arm exists to stop, so
                        // the combined spelling would have fixed nothing.
                        if !mods.contains(crate::shortcut::Modifiers::CTRL)
                            && !mods.contains(crate::shortcut::Modifiers::ALT)
                            && !mods.contains(crate::shortcut::Modifiers::META)
                        {
                            // Only a **printable** key is content: `char::from_u32(0)` is NUL, and
                            // 9/10/13 are Tab/LF/CR — control codes the host owns, not text.
                            if let Some(ch) = char::from_u32(*key).filter(|c| !c.is_control()) {
                                if ch.is_ascii_graphic() || ch == ' ' {
                                    self.insert_char(ch);
                                    self.request_redraw();
                                }
                            }
                        }
                    }
                }
            }
            // ── Pointer selection ──
            //
            // The press places the caret where the user aimed and anchors a selection there; a
            // following drag extends it. Before this a press only focused the control, so clicking
            // into the middle of an existing document and typing appended instead of inserting —
            // and there was no pointer selection at all.
            Event::MousePress { pos, button, modifiers, .. }
                if *button == 1 && self.hit_test(*pos) =>
            {
                // I-118: a press on the scrollbar thumb grabs it instead of moving the caret.
                if self.scrollbar_contains(*pos) {
                    self.scrollbar_drag = true;
                    self.scrollbar_scroll_to(pos.y);
                    self.request_redraw();
                    return;
                }
                self.focused = true;
                self.cursor_blink.start();
                let index = self.byte_index_at(*pos);
                self.press_at(index, crate::shortcut::Modifiers::from_event_bits(*modifiers));
            }
            Event::MouseMove { pos } => {
                if self.scrollbar_drag {
                    self.scrollbar_scroll_to(pos.y);
                    self.request_redraw();
                } else if self.dragging_selection {
                    let index = self.byte_index_at(*pos);
                    self.drag_to(index);
                }
            }
            // A double click selects the word under the pointer.
            Event::MouseDoubleClick { pos, button } if *button == 1 && self.hit_test(*pos) => {
                self.focused = true;
                let index = self.byte_index_at(*pos);
                let (start, end) = self.word_range_at(index);
                self.selection_anchor = Some(start);
                self.cursor_pos = end;
                self.dragging_selection = false;
                self.scroll_caret_into_view();
                self.notify_caret_change();
                self.base.request_redraw();
            }
            // A release anywhere ends the gesture, including outside the control: the anchor stays
            // where the press put it, so the selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.scrollbar_drag = false;
                self.end_drag();
            }
            _ => {}
        }
    }
}

/// Approximate line height in pixels for `Font::default()`.
///
/// # Why a row height is still a constant, when the character width is not
///
/// `CHAR_W` was removed: the advance of a glyph is a fact about the *face*, and multiplying a
/// character count by `8` disagreed with the renderer by up to 12 px on CJK (measured). Row height
/// is a different kind of number — it is the control's own layout decision (how far apart the rows
/// are), not a measurement of any glyph — so a single value is correct here and the paint path and
/// the hit test share it.
const LINE_H: i32 = 16;
/// The first line's inset below the control's top edge.
///
/// A named constant rather than a literal inside `draw`, because the pixel-to-offset map in
/// [`TextArea::byte_index_at`] has to place a click on the same rows the glyphs were painted on;
/// two copies of `4` is how a click and its text drift apart at the first line.
const FIRST_LINE_INSET: i32 = 4;

impl Draw for TextArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a queued find/replace intent first, so the caret and the selection this frame
        // paints are the ones the strip asked for (COMP-A8).
        #[cfg(full_widgets)]
        self.poll_find_replace_bar();
        // A text area is the one field whose height is **not** a fixed band: it is
        // multi-line, so the lines legitimately span whatever height the caller gives it.
        // `rect.height` therefore stays. Everything *inside* is measured from the field's
        // own padding rather than from a local literal, so the first line starts where a
        // single-line field's does instead of at a second, unshared `y = 4`.
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;

        // -- Background --
        let bg = self.style().background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.face_with_gradient_corners(
            rect,
            bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // -- Border --
        let border = self.style().border_color.unwrap_or(Color::rgb(200, 200, 200));
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // -- Text --
        let text_color = self.style().text_color.unwrap_or(Color::rgb(0, 0, 0));
        // The placeholder is the value's own ink damped toward the field it sits on — the same
        // derivation `line_edit` and `auto_complete_edit` use — rather than a fixed grey that
        // ignored the appearance. A literal here rendered a light-theme hint over a dark field.
        // The *disabled* ink is a different fact (see `disabled_ink_on`) and is not this colour.
        let field_bg = bg;
        let placeholder_color = text_color.blend(&field_bg, 0.55);

        // The first line's own glyph box, centred on a line-height band below the top edge.
        // The origin of a text run is the *top-left corner of its glyph box*, so the old
        // `rect.y + padding` put that corner 4 px below the border and drew the first line
        // half a line high — the same off-by-a-half-line defect the single-line fields had,
        // and the reason the first line sat at `y = 4` in `text_area.svg` while its own
        // placeholder sat beside it on a different baseline.
        //
        // The band is one line tall and starts one inset below the top border, where a
        // single-line field's content starts, so a text area's first line and a `line_edit`'s
        // value sit on the same row when the two are laid out at the same height. Deriving
        // the band from the *first line* rather than from the whole control is what keeps a
        // 120 px text area from putting its first line halfway down the control.
        let font = self.text_font();
        let first_band = Rect::new(origin_x, rect.y + FIRST_LINE_INSET, rect.width, LINE_H as u32);
        let first_line = context.text_line(first_band, font);
        let text_top = first_line.y;

        // ── Clip to the field's inner box ──
        //
        // This area has no soft wrap and no horizontal scroll, so a line longer than the field
        // was painted straight across the control's right edge and over its neighbours (the
        // runtime only clips a widget whose `children_clip()` is set, which a text area does not
        // override). The selection band and the caret followed the same overflow. Clipping keeps
        // the visible part exactly where it was — the origin does not move, so no existing
        // layout shifts — and makes the field a window onto its own content, which is the same
        // answer `code_editor` and `properties_panel` already give.
        let inner = Rect::new(
            origin_x,
            rect.y.saturating_add(1),
            rect.width.saturating_sub(2 * dimensions::TEXT_FIELD_PADDING_H),
            rect.height.saturating_sub(2),
        );
        context.push_clip(inner.x, inner.y, inner.width, inner.height);

        if self.text.is_empty() && !self.placeholder.is_empty() && !self.focused {
            // Draw placeholder in gray, on the same first-line box the value would use.
            context.draw_text(
                Point::new(origin_x, text_top),
                &self.placeholder,
                font,
                placeholder_color,
                HorizontalAlignment::Left,
            );
        } else if !self.text.is_empty() {
            // ── The selection, behind the glyphs ──
            //
            // Painted first, so the value's ink sits on top of it. Each logical line contributes
            // the part of the range that falls on it, which is what makes a multi-line selection
            // read as continuous instead of becoming one full-width bar down the whole control.
            // A band drawn *after* the text would cover the very words the user just selected,
            // which is why the order here is load-bearing rather than incidental.
            if let Some((sel_start, sel_end)) = self.selection_range() {
                let highlight = self.selection_color(&text_color, &field_bg);
                // One pass over the display rows, so a scrolled field highlights the same rows it
                // draws and a soft-wrapped line contributes one band per wrapped row.
                let mut y = text_top;
                for row in 0.. {
                    let Some(display) = self.display_row(row) else {
                        break;
                    };
                    // I-P62: stop at the bottom edge. The loop used to walk every display row of
                    // the document, doing overlap and band-width work for rows far below the
                    // viewport.
                    if y + LINE_H > rect.y + rect.height as i32 {
                        break;
                    }
                    // The overlap of `[sel_start, sel_end)` with this display row's own content.
                    let from = sel_start.max(display.start).min(display.end);
                    let to = sel_end.min(display.end).max(from);
                    if to > from {
                        // Measured, not multiplied: the band's two edges have to land on the same
                        // pixels the glyphs do, and the glyphs are laid out with the face's own
                        // advances (see `advance_of_caret_prefix` for what a fixed width costs).
                        let left = origin_x + self.content_margin - self.scroll_x
                            + self.band_advance(display.start, from);
                        let width = self.band_advance(from, to);
                        if width > 0 {
                            // `fill_rect`, not `draw_rect`: the latter is a one-pixel outline,
                            // so a selected run painted a hollow box around the glyphs (the
                            // glyphs themselves were not highlighted at all). `TextEdit` fills
                            // the same band from the same arithmetic.
                            context.fill_rect(
                                Rect::new(left, y, width as u32, LINE_H as u32),
                                highlight,
                            );
                        }
                    }
                    y += LINE_H;
                }
            }

            let mut y = text_top;
            // Rows are drawn from the scroll position, so the field is a window onto the document
            // rather than always showing its first line. `display_row` is the one lookup of
            // "which text is on row n", shared with the pixel-to-offset map below, so a click and
            // the glyphs it lands on cannot disagree about the scroll or the wrap.
            for row in 0.. {
                let Some(display) = self.display_row(row) else {
                    break;
                };
                if y + LINE_H > rect.y + rect.height as i32 {
                    break;
                }
                context.draw_text(
                    Point::new(origin_x + self.content_margin - self.scroll_x, y),
                    &self.text[display.start..display.end],
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
                y += LINE_H;
            }
        }

        // -- Cursor --
        // I-116: the caret is only painted during the visible half of its blink cycle.
        if self.focused && self.cursor_blink.is_visible() {
            let cursor_x = self.cursor_screen_x(origin_x);
            let cursor_y = self.cursor_screen_y(text_top);
            context.draw_line(
                Point::new(cursor_x, cursor_y),
                Point::new(cursor_x, cursor_y + LINE_H),
                text_color,
            );
        }
        context.pop_clip();
        self.draw_scrollbar(context);
    }
}

impl TextArea {
    /// Advances the caret blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate-wide `tick(delta_ms) -> bool` convention: a host that renders on demand keeps
    /// owning the clock, and a blurred area never asks for a frame (I-116).
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

    /// The scrollbar's track rectangle, or `None` when the value fits the viewport (I-118).
    fn scrollbar_track(&self) -> Option<(Rect, usize, usize)> {
        let total = self.line_spans().len();
        let visible = self.visible_line_count();
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

    /// Hands the standard scrollbar the track and the metrics it needs.
    ///
    /// Returns the track when the value overflows the viewport. The companion then owns the
    /// thumb rectangle, the `pos`-to-offset mapping and the visible predicate, so the paint and
    /// the drag cannot disagree about the thumb they act on (I-128).
    fn sync_scrollbar(&mut self) -> Option<Rect> {
        let (track, total, visible) = self.scrollbar_track()?;
        self.scrollbar.set_track(track);
        self.scrollbar.set_metrics(
            total.min(u32::MAX as usize) as u32,
            visible.min(u32::MAX as usize) as u32,
            self.first_visible_line.min(u32::MAX as usize) as u32,
        );
        Some(track)
    }

    /// Whether `pos` is on the scrollbar track (and therefore grabs the thumb).
    fn scrollbar_contains(&self, pos: Point) -> bool {
        self.scrollbar_track().map(|(track, _, _)| track.contains_point(pos)).unwrap_or(false)
    }

    /// Scrolls the value so the scrollbar dragged to `y` lands there (I-118).
    fn scrollbar_scroll_to(&mut self, y: i32) {
        let Some(track) = self.sync_scrollbar() else {
            return;
        };
        let next = self.scrollbar.drag_thumb(Point::new(track.x, y)) as usize;
        if next != self.first_visible_line {
            self.first_visible_line = next;
        }
    }

    /// Paints the right-edge scrollbar when the value is taller than the viewport (I-118).
    fn draw_scrollbar(&mut self, context: &mut RenderContext) {
        let Some(track) = self.sync_scrollbar() else {
            return;
        };
        if !self.scrollbar.is_visible() {
            return;
        }
        let paper = self.style().background_color.unwrap_or(Color::WHITE);
        let ink = self.style().text_color.unwrap_or(Color::BLACK);
        let track_color = ink.blend(&paper, 0.18);
        let thumb_color = ink.blend(&paper, 0.45);
        context.fill_rect(track, track_color);
        let thumb = self.scrollbar.thumb_rect(track);
        if thumb.width > 0 && thumb.height > 0 {
            context.fill_rect(thumb, thumb_color);
        }
    }

    /// The resolved body font the control paints and measures text with.
    ///
    /// Draw and the caret/hit-test helpers must read the same font, so a themed or scaled body font
    /// reaches both the ink and the arithmetic that places the caret and maps a click to an offset.
    fn text_font(&self) -> &Font {
        effective_font(self.style())
    }

    /// The line box the effective font occupies, shared by the paint grid and the hit test.
    ///
    /// The paint path centres each row's glyph box in its [`LINE_H`] band via `text_line`, which
    /// derives the box from the font's **effective** line height — so the grid arithmetic must read
    /// the same number here rather than the point size (`size_i32`), which only equals the line
    /// height for a font with no explicit leading.
    fn line_box_height(&self) -> i32 {
        self.text_font().effective_line_height().max(1.0) as i32
    }

    /// The band colour a selection is painted in.
    ///
    /// Derived from the field's own ink and fill rather than a fixed blue, so a selection reads on
    /// the light *and* the dark appearance without a second theme token to keep in step — the same
    /// derivation the placeholder uses a few lines above. A literal here would have painted a
    /// light-theme highlight over a dark field.
    fn selection_color(&self, text_color: &Color, field_bg: &Color) -> Color {
        // Two steps, not one: blending the ink toward the fill has to land the band *between* the
        // two, because a single blend of black into white is either invisible behind the glyphs or
        // dark enough to swallow them.
        text_color.blend(field_bg, 0.3).blend(field_bg, 0.25)
    }

    /// Computes the screen-space X coordinate of the cursor.
    ///
    /// The prefix is sliced through [`floor_char_boundary`] rather than at `cursor_pos` directly:
    /// the caret is a byte index that `set_cursor_pos` now keeps on a boundary, and an *undo*
    /// restore can replace the value under it, so slicing defensively here keeps the paint path
    /// total whatever the caret holds. A draw that can panic is worse than a caret in the wrong
    /// place — it takes the frame down.
    fn cursor_screen_x(&self, origin_x: i32) -> i32 {
        origin_x + self.content_margin - self.scroll_x
            + self.advance_of_caret_prefix(self.cursor_pos)
    }

    /// The advance of `text[from..to]`, memoised for the current revision and font.
    fn band_advance(&self, from: usize, to: usize) -> i32 {
        let font = self.text_font();
        {
            let cache = self.band_width_cache.borrow();
            if let Some(cache) = cache.as_ref() {
                if cache.revision == self.revision && cache.font == *font {
                    if let Some(width) = cache.entries.get(&(from, to)) {
                        return *width;
                    }
                }
            }
        }
        let width = if from < to { self.text_advance(&self.text[from..to]) } else { 0 };
        let mut cache = self.band_width_cache.borrow_mut();
        let stale = match cache.as_ref() {
            Some(entry) => entry.revision != self.revision || entry.font != *font,
            None => true,
        };
        if stale
            || cache.as_ref().is_some_and(|entry| entry.entries.len() >= BAND_WIDTH_CACHE_LIMIT)
        {
            *cache = Some(BandWidthCache {
                revision: self.revision,
                font: font.clone(),
                entries: HashMap::new(),
            });
        }
        if let Some(entry) = cache.as_mut() {
            entry.entries.insert((from, to), width);
        }
        width
    }

    /// The advance of the text on the caret's line, before `offset`, in pixels.
    ///
    /// # Why this is measured and not multiplied
    ///
    /// The paint path draws each line through the renderer's own shaper, so the ink's width is
    /// whatever the *face* says. Locating the caret by `characters * CHAR_W` was therefore a second,
    /// disagreeing model of the same text — measured, for `Font::default()` at 14 px, as:
    ///
    /// ```text
    /// "i"       renderer 4   CHAR_W model 8     (4 px out)
    /// "中文"    renderer 28  CHAR_W model 16    (12 px out)
    /// ```
    ///
    /// A caret 12 px from its character is on the wrong character, and every hit test that agreed
    /// with it put the insertion point somewhere the user did not click. Measuring through
    /// `measure_text` — the entry point the renderer itself uses — makes the two the same answer by
    /// construction rather than by a constant that has to be kept in step.
    fn advance_of_caret_prefix(&self, offset: usize) -> i32 {
        let caret = floor_char_boundary(&self.text, offset.min(self.text.len()));
        // The display row is the unit the painter advances by: without soft wrap it starts at the
        // logical line, and with it, at the wrapped segment the caret sits on.
        let rows = self.display_rows();
        let row_start = rows.iter().rfind(|row| row.start <= caret).map_or(0, |row| row.start);
        // One cached measurement source for the caret: the same revision-keyed table the selection
        // band reads, rather than a fresh shape of the same prefix every frame (I-005).
        self.band_advance(row_start, caret)
    }

    /// The advance of `text` under the renderer's own metrics.
    ///
    /// A throwaway backend is used as a **measurement surface**: `RenderContext::measure_text`
    /// writes no command, so a zero-sized surface is enough and nothing is rasterised on a hit test
    /// or a caret draw.
    fn text_advance(&self, text: &str) -> i32 {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        context.measure_text(text, self.text_font()).width as i32
    }

    /// The byte offset the horizontal coordinate `x` points at, within the display row
    /// `start..end`.
    ///
    /// The row's per-character advances are cached by [`Self::revision`] and the font, so a press
    /// or drag step reads a table instead of shaping every glyph again (I-P09). The first click
    /// after an edit builds the table once; moving the caret does not invalidate it.
    fn column_at_x(&self, start: usize, end: usize, x: i32) -> usize {
        if start >= end {
            return 0;
        }
        let font = self.text_font().clone();
        let fresh = {
            let cache = self.line_advance_cache.borrow();
            match cache.as_ref() {
                Some(entry)
                    if entry.revision == self.revision
                        && entry.font == font
                        && entry.start == start
                        && entry.end == end =>
                {
                    Some(entry.cuts.clone())
                }
                _ => None,
            }
        };
        let cuts = match fresh {
            Some(cuts) => cuts,
            None => {
                let row = &self.text[start..end];
                let mut cuts: Vec<(usize, i32)> = Vec::with_capacity(row.len() + 1);
                cuts.push((0, 0));
                Self::with_measurement(|context| {
                    let mut advance = 0i32;
                    let mut glyph = [0u8; 4];
                    for (index, ch) in row.char_indices() {
                        advance +=
                            context.measure_text(ch.encode_utf8(&mut glyph), &font).width as i32;
                        cuts.push((index + ch.len_utf8(), advance));
                    }
                });
                let cached = LineAdvanceCache {
                    revision: self.revision,
                    font: font.clone(),
                    start,
                    end,
                    cuts: cuts.clone(),
                };
                *self.line_advance_cache.borrow_mut() = Some(cached);
                cuts
            }
        };
        let target = x.max(0);
        let mut best = 0usize;
        for window in cuts.windows(2) {
            let (_, previous) = window[0];
            let (offset, cumulative) = window[1];
            let advance = cumulative - previous;
            if target < previous + advance / 2 {
                break;
            }
            best = offset;
        }
        best
    }

    /// Runs `f` against a throwaway measurement surface.
    ///
    /// `measure_text` writes no command, so a zero-sized backend is enough and nothing is
    /// rasterised while a layout is computed.
    fn with_measurement<R>(f: impl FnOnce(&RenderContext) -> R) -> R {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        f(&RenderContext::new(&mut measurement))
    }

    /// Computes the screen-space Y coordinate of the cursor (top of cursor line).
    ///
    /// The caret's own line is measured **relative to the viewport**, so a scrolled field draws the
    /// marker on the row the character is actually painted on rather than at its document position —
    /// which would put it above the field, or below it, once anything had been scrolled past.
    fn cursor_screen_y(&self, origin_y: i32) -> i32 {
        let row = self.display_row_of(self.cursor_pos).saturating_sub(self.first_visible_line);
        origin_y + row as i32 * LINE_H
    }

    /// The caret's window rectangle and byte offset, as a native IME asks for them.
    ///
    /// # Why the horizontal position is measured without a paint pass
    ///
    /// The IME query is answered on a `&self` borrow between frames, when no `RenderContext` is
    /// available to the caller. The x is the **cached** advance of the text before the caret on
    /// its own line — the same measurement source [`Self::cursor_screen_x`] reads during a paint
    /// (I-005) — so the anchor and the ink cannot disagree. The vertical position is the caret's
    /// visible row, so a scrolled area reports the row it actually draws the marker on rather
    /// than the caret's document position.
    fn caret_query(&self) -> InputMethodQuery {
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        let caret_x =
            origin_x + self.content_margin - self.scroll_x + self.advance_of_caret_prefix(caret);

        // The first line's band is the same one `draw` lays the glyphs out in: one line tall, one
        // inset below the top edge. The line height is measured once per font and cached, so an
        // IME query does not build a throwaway paint backend on every keystroke (I-P09).
        let band = Rect::new(origin_x, rect.y + FIRST_LINE_INSET, rect.width, LINE_H as u32);
        let line_height = self.measured_line_height();
        let text_top = band.y + ((band.height as i32 - line_height) / 2).max(0);
        let row = self.display_row_of(self.cursor_pos).saturating_sub(self.first_visible_line);
        InputMethodQuery {
            caret_rect: Rect::new(caret_x, text_top + row as i32 * LINE_H, 1, LINE_H as u32),
            caret_byte: Some(caret),
        }
    }

    /// The renderer's line height for the current body font, measured once per font.
    ///
    /// `RenderContext::text_line` derives its box from `measure_text("M", font)`. Repeating that
    /// measurement for every IME query built a throwaway backend per call; this caches the one
    /// number it needs (I-P09).
    fn measured_line_height(&self) -> i32 {
        let font = self.text_font().clone();
        if let Some((cached_font, height)) = self.line_height_cache.borrow().as_ref() {
            if *cached_font == font {
                return *height as i32;
            }
        }
        let height =
            Self::with_measurement(|context| context.measure_text("M", &font).height).max(1);
        *self.line_height_cache.borrow_mut() = Some((font, height));
        height as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    /// The area turns a context-menu request into its own signal at the requested point.
    #[test]
    fn a_context_menu_request_emits_the_requested_point() {
        let mut area = TextArea::new(String::new(), Rect::new(0, 0, 240, 120));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&seen);
        area.context_menu_requested.connect_ref(move |pos: &Point| {
            *sink.lock().unwrap() = Some(*pos);
        });

        assert!(area.context_menu_event(Point::new(12, 20)));
        assert_eq!(*seen.lock().unwrap(), Some(Point::new(12, 20)));
    }

    /// The bitmap face fills its glyph box, so its ink top *is* the box top. An outline face draws a
    /// real glyph whose ink is inset, so the ink top sits one or two pixels below the box top at these
    /// sizes. Pinning the two equal encoded a property of the bitmap face, not of the layout.
    ///
    /// Gated with its only consumer:
    /// `the_first_line_is_padded_consistently_with_every_other_field` renders through the SVG backend
    /// and is `#[cfg(not(alloc_frugal))]`. An ungated constant therefore became dead code under `mini`
    /// (which *is* `alloc_frugal`) and `clippy -D warnings` reported it there while every other
    /// profile stayed green.
    #[cfg(not(alloc_frugal))]
    const INK_INSET_TOLERANCE: i32 = 3;

    #[test]
    fn textarea_creation_defaults() {
        let ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert_eq!(ta.text(), "");
        assert_eq!(ta.cursor_pos(), 0);
        assert_eq!(ta.max_length, 0);
        assert!(!ta.is_read_only());
        assert!(ta.placeholder().is_empty());
        assert!(!ta.focused);
    }

    #[test]
    fn textarea_set_text() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_text("Hello\nWorld".to_string());
        assert_eq!(ta.text(), "Hello\nWorld");
        assert_eq!(ta.cursor_pos(), 11); // cursor at end
    }

    #[test]
    fn textarea_insert_char() {
        let mut ta = TextArea::new("Helo".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(3);
        ta.insert_char('l');
        assert_eq!(ta.text(), "Hello");
        assert_eq!(ta.cursor_pos(), 4);
    }

    #[test]
    fn textarea_delete_char() {
        let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(5);
        ta.delete_char();
        assert_eq!(ta.text(), "Hell");
        assert_eq!(ta.cursor_pos(), 4);
    }

    #[test]
    fn textarea_delete_char_at_start() {
        let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(0);
        ta.delete_char();
        assert_eq!(ta.text(), "Hello");
        assert_eq!(ta.cursor_pos(), 0);
    }

    #[test]
    fn textarea_delete_char_with_newline() {
        let mut ta = TextArea::new("A\nB".to_string(), Rect::new(0, 0, 300, 200));
        // "A\nB" — cursor at 3 (end), delete_char removes 'B', cursor at 2
        ta.set_cursor_pos(3);
        ta.delete_char();
        assert_eq!(ta.text(), "A\n");
        assert_eq!(ta.cursor_pos(), 2);
    }

    #[test]
    fn textarea_cursor_movement() {
        let mut ta = TextArea::new("Hi".to_string(), Rect::new(0, 0, 300, 200));
        assert_eq!(ta.cursor_pos(), 2);
        ta.set_cursor_pos(0);
        assert_eq!(ta.cursor_pos(), 0);
        ta.set_cursor_pos(5); // clamp
        assert_eq!(ta.cursor_pos(), 2);
    }

    #[test]
    fn textarea_placeholder() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert!(ta.placeholder().is_empty());
        ta.set_placeholder("Enter text...".to_string());
        assert_eq!(ta.placeholder(), "Enter text...");
    }

    #[test]
    fn textarea_read_only() {
        let mut ta = TextArea::new("Readable".to_string(), Rect::new(0, 0, 300, 200));
        assert!(!ta.is_read_only());
        ta.set_read_only(true);
        assert!(ta.is_read_only());
    }

    #[test]
    fn textarea_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(5);
        assert_eq!(ta.max_length, 5);
        // insert beyond limit
        ta.set_text("Hello World".to_string());
        assert_eq!(ta.text().len(), 5);
        assert_eq!(ta.text(), "Hello");
    }

    #[test]
    fn textarea_draw_does_not_panic() {
        use crate::render::RenderContext;
        use crate::render::SoftwarePaintBackend;

        let mut ta = TextArea::new("Line1\nLine2".to_string(), Rect::new(0, 0, 200, 100));
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 100), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        // Should not panic
        ta.draw(&mut ctx);
    }

    /// The first line is padded on the **field's** own inset from the top, not at `y = 4`.
    ///
    /// The origin of a text run is its glyph box's top-left corner, so `rect.y + 4` put that
    /// corner four pixels below the border and drew the first line half a line high — the
    /// placeholder and the value that replaced it therefore sat on different baselines, and
    /// `text_area.svg` showed a line whose top edge was `y = 4`. The first line now takes the
    /// same line box every other text-bearing control uses.
    ///
    /// The assertion is on the **ink**, not on a `x`/`y` attribute: text leaves the backend as
    /// the `font8x8` rectangles the rasteriser fills, so the document holds a picture of the run
    /// rather than the run itself. That is a stronger check than the attribute it replaced — a
    /// glyph placed a line off with a correct attribute would have passed the old form.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_first_line_is_padded_consistently_with_every_other_field() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut ta = TextArea::new("Line1".to_string(), Rect::new(0, 0, 240, 120));
        let svg = crate::widget::svg::render_to_svg(&mut ta);
        let (x, y, right, _) = crate::widget::svg::text_ink_box(&svg)
            .unwrap_or_else(|| panic!("a text path must be emitted for the first line: {svg}"));

        // The bitmap face fills its glyph box, so the run's left ink is the shared inset; an
        // outline face's ink is inset from the box, so the two agree to within that inset rather
        // than exactly — which was a property of the typeface, not of the layout.
        assert!(
            (x - dimensions::TEXT_FIELD_PADDING_H as i32).abs() <= INK_INSET_TOLERANCE,
            "the shared horizontal inset: ink left {x}, inset {}",
            dimensions::TEXT_FIELD_PADDING_H
        );
        assert!(right > x, "the first line laid down ink: {x}..{right}");
        // The first line's band is one line tall and starts at the field's top inset, the same
        // 4 px a single-line field's content starts at; a line centred in that band therefore
        // has its glyph-box top below the border but inside the first row. The two bounds are
        // what the old attribute assertion checked, and they still hold — what they could not
        // see is the *centred* origin, because the bare inset (4) is itself inside the row.
        assert!(y > 0, "the first line clears the border: {y}");
        assert!(y < LINE_H, "and stays inside the first row: {y}");
        // Pin the exact origin: the glyph box's top edge is the line box centred in that band,
        // which is `inset + (band - line) / 2`. Deriving it from the measured line height rather
        // than from a copied literal is what makes this a statement about the layout.
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
        let line_h = crate::render::RenderContext::new(&mut backend)
            .measure_text("M", &crate::core::Font::default())
            .height as i32;
        let first_band_top = 4;
        let expected = first_band_top + (LINE_H - line_h) / 2;
        // The glyph box's top edge is the centred line box; the ink top is that edge under the
        // bitmap face and one or two pixels in under an outline face, so the two agree to within
        // the inset. A line left at `y = 4` is a whole inset-plus-half-line away and fails.
        assert!(
            (y - expected).abs() <= INK_INSET_TOLERANCE,
            "the first line sits on the band's centred line box: ink top {y}, box top {expected}"
        );
    }

    #[test]
    fn textarea_set_text_truncates_on_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(3);
        ta.set_text("Hello".to_string());
        assert_eq!(ta.text(), "Hel");
        assert_eq!(ta.cursor_pos(), 3);
    }

    /// Reducing the limit truncates the value *and* invalidates the history that
    /// could restore the over-limit string.
    #[test]
    fn max_length_change_keeps_undo_within_the_limit() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_text("abcdef");
        ta.set_max_length(3);
        assert_eq!(ta.text(), "abc", "the value is truncated to the limit");

        assert!(!ta.can_undo(), "an over-limit value must not be undoable");
        assert!(!ta.undo());
        assert_eq!(ta.text(), "abc");
        assert!(!ta.redo());
        assert_eq!(ta.text(), "abc");
    }

    #[test]
    fn textarea_insert_respects_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(3);
        ta.set_text("ABC".to_string());
        ta.insert_char('D');
        // Should not insert because max_length reached
        assert_eq!(ta.text(), "ABC");
        assert_eq!(ta.cursor_pos(), 3);
    }

    #[test]
    fn textarea_empty_text_events_no_panic() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // backspace on empty
        assert_eq!(ta.text(), "");
        ta.handle_event(&Event::KeyPress { key: 37, modifiers: 0 }); // left arrow at 0
        assert_eq!(ta.cursor_pos(), 0);
        ta.handle_event(&Event::KeyPress { key: 39, modifiers: 0 }); // right arrow at 0 (no change)
        assert_eq!(ta.cursor_pos(), 0);
    }

    #[test]
    fn textarea_focus_events() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert!(!ta.focused);
        ta.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(ta.focused);
        ta.handle_event(&Event::FocusLost);
        assert!(!ta.focused);
    }

    #[test]
    fn textarea_keypress_insertion() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 72, modifiers: 0 }); // 'H'
        ta.handle_event(&Event::KeyPress { key: 105, modifiers: 0 }); // 'i'
        assert_eq!(ta.text(), "Hi");
    }

    #[test]
    fn textarea_keypress_enter_inserts_newline() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        ta.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter
        ta.handle_event(&Event::KeyPress { key: 66, modifiers: 0 }); // 'B'
        assert_eq!(ta.text(), "A\nB");
    }

    #[test]
    fn textarea_setters_request_redraw() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        ta.base.redraw_requested.connect({
            let flag = std::sync::Arc::clone(&fired);
            move || flag.store(true, std::sync::atomic::Ordering::SeqCst)
        });

        ta.set_text("hello");
        ta.set_cursor_pos(2);
        ta.set_max_length(3);
        ta.set_read_only(true);
        ta.set_placeholder("hint".to_string());

        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }

    /// The limit counts characters and truncation lands on a character boundary (I-014).
    #[test]
    fn max_length_truncation_respects_character_boundaries() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_text("你好");
        ta.set_max_length(1);
        assert!(ta.text().is_char_boundary(ta.text().len()));
        assert_eq!(ta.text(), "你", "one character is one cell of the limit");
        assert!(ta.text().is_char_boundary(ta.cursor_pos()));

        // A limit of four characters leaves a two-character value alone, which is what the
        // counter says it should do; the old byte budget cut it to one.
        let mut wide = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        wide.set_text("你好");
        wide.set_max_length(4);
        assert_eq!(wide.text(), "你好");
    }

    /// A caret set to a mid-character byte is snapped, so the draw cannot slice through one.
    ///
    /// The editor passes an arbitrary byte index to `set_cursor_pos`; every reader slices at the
    /// caret (`delete_char`, and the caret's own screen position on the **paint** path), so a
    /// mid-character value used to panic the frame rather than merely misplace the marker.
    #[test]
    fn a_mid_character_cursor_is_snapped_to_a_boundary() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_text("é");
        ta.set_cursor_pos(1); // inside the two-byte 'é'
        assert_eq!(ta.cursor_pos(), 0, "snapped down to a real character boundary");
        // The paint path and the edit path must both survive it.
        let _ = crate::widget::svg::render_to_svg(&mut ta);
        ta.delete_char();
        assert_eq!(ta.text(), "é", "nothing was deleted from position 0");
    }

    // ─── Selection: the keyboard ───

    /// The framework's Shift bit on an event mask.
    const SHIFT_BIT: u32 = 0b0001;

    fn area(text: &str) -> TextArea {
        TextArea::new(text.to_string(), Rect::new(0, 0, 300, 200))
    }

    // ─── D09-INPUT-01: platform-committed text reaches the value ───

    /// A `TextInput` from the platform must enter the area's value.
    ///
    /// # The defect this pins (D09-INPUT-01)
    ///
    /// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
    /// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
    /// test feeds `TextInput` to prove that path itself is wired.
    #[test]
    fn text_input_enters_the_value() {
        let mut ta = area("");
        ta.handle_event(&Event::TextInput { text: "héllo".to_string() });
        assert_eq!(ta.text(), "héllo");
    }

    /// An IME commit enters the value the same way.
    #[test]
    fn ime_commit_enters_the_value() {
        let mut area = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        area.handle_event(&Event::ime_commit("你好"));
        assert_eq!(area.text(), "你好");
    }

    /// The IME query reports a real caret rectangle on the caret's row, and it follows a newline.
    ///
    /// A multi-line field's insertion point is a *row and a column*, not one line: an anchor that
    /// stayed on the first row would place the candidate window beside the wrong line as soon as
    /// the user typed past the first newline.
    #[test]
    fn input_method_query_reports_the_caret_row_and_column() {
        let mut area = TextArea::new("abc\ndef".to_string(), Rect::new(10, 20, 300, 200));
        area.set_cursor_pos(0);
        let on_first =
            area.input_method_query().expect("an editable text area always has an insertion point");
        assert!(on_first.caret_rect.width > 0, "the caret must have a width: {on_first:?}");
        assert!(on_first.caret_rect.height > 0, "the caret must have a height: {on_first:?}");
        assert_eq!(on_first.caret_byte, Some(0));

        // The second line starts after the newline at byte 4; byte 6 is two characters in.
        area.set_cursor_pos(6);
        let on_second = area.input_method_query().expect("query after moving to the next line");
        assert_eq!(on_second.caret_byte, Some(6));
        assert!(
            on_second.caret_rect.y > on_first.caret_rect.y,
            "the caret must move down a row on the next line ({} -> {})",
            on_first.caret_rect.y,
            on_second.caret_rect.y
        );
        assert!(
            on_second.caret_rect.x > on_first.caret_rect.x,
            "two columns into the second line is two measured advances right"
        );
    }

    /// A `TextInput` replaces a selection rather than appending to it.
    #[test]
    fn text_input_replaces_the_selection() {
        let mut ta = area("abcdef");
        ta.select_all();
        ta.handle_event(&Event::TextInput { text: "Z".to_string() });
        assert_eq!(ta.text(), "Z", "the committed text replaced the selected range");
    }

    /// Committed text respects `max_length`.
    #[test]
    fn text_input_respects_the_max_length() {
        let mut ta = area("");
        ta.set_max_length(3);
        ta.handle_event(&Event::TextInput { text: "abcdef".to_string() });
        assert!(ta.text().len() <= 3, "the limit applies: got {:?}", ta.text());
    }

    /// A read-only area ignores committed text, just as it ignores a printable key.
    #[test]
    fn read_only_ignores_text_input() {
        let mut ta = area("locked");
        ta.set_read_only(true);
        ta.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(ta.text(), "locked");
    }

    /// A disabled area ignores committed text.
    #[test]
    fn disabled_ignores_text_input() {
        let mut ta = area("");
        ta.set_enabled(false);
        ta.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(ta.text(), "");
    }

    /// A Shift-arrow **extends**; a plain arrow replaces.
    ///
    /// # The defect this pins
    ///
    /// This control had no selection at all: `cursor_pos` was the only position it stored, and the
    /// arrows moved it. There was therefore no way to select text with the keyboard and, in turn, no
    /// way to replace a word by typing over it. The assertions are on the *range* rather than on the
    /// anchor, because a range is what the user sees: an implementation that moved the caret and
    /// forgot the anchor would still satisfy `cursor_pos` and select nothing.
    #[test]
    fn a_shift_arrow_extends_and_a_plain_arrow_replaces() {
        let mut ta = area("abcdef");
        ta.set_cursor_pos(4);

        ta.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(ta.selected_text(), "d", "the first Shift+Left selects the character behind");
        assert_eq!(ta.cursor_pos(), 3);

        ta.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(ta.selected_text(), "cd", "the second extends the same range");

        // A plain arrow drops the range but keeps the caret where the user had moved it to.
        ta.handle_event(&Event::key_press(37, 0));
        assert_eq!(ta.selection_anchor(), None, "a plain arrow is not an extend");
        assert_eq!(ta.selected_text(), "", "so nothing stays selected");
    }

    /// An extension keeps **one** anchor, so it is reversible.
    ///
    /// # The defect this pins
    ///
    /// The natural shortcut is to re-anchor at the caret before every shift-movement. That makes the
    /// gesture irreversible: `Shift+Home` on a caret at 5 leaves the anchor at 5, so the following
    /// `Shift+End` grows from the wrong end. The anchor belongs to the whole gesture, not to each
    /// key press within it.
    #[test]
    fn extending_back_and_forth_returns_to_the_anchor() {
        let mut ta = area("abcdefgh");
        ta.set_cursor_pos(5);

        ta.handle_event(&Event::key_press(36, SHIFT_BIT));
        assert_eq!(ta.selection_anchor(), Some(5), "the anchor is where the gesture began");
        assert_eq!(ta.cursor_pos(), 0, "Home walks the caret to the line start");
        assert_eq!(ta.selected_text(), "abcde");

        ta.handle_event(&Event::key_press(35, SHIFT_BIT));
        assert_eq!(ta.selection_anchor(), Some(5), "and it did not move");
        assert_eq!(ta.selected_text(), "fgh", "so the range is now the other side of it");
    }

    /// Home and End are **line** bounds here, and Primary reaches the document.
    ///
    /// # The defect this pins
    ///
    /// Home and End were not handled, so they fell through to the character-input catch-all where
    /// key 36 is `$` and 35 is `#` — pressing Home typed a dollar sign. A multi-line control whose
    /// Home jumps to the start of the document is also close to useless for reaching the line the
    /// caret is already on, which is why the plain key is the line and the chord is the document.
    #[test]
    fn home_and_end_are_line_bounds_and_primary_reaches_the_document() {
        let mut ta = area("one\ntwo\nthree");
        ta.set_cursor_pos(6); // two characters into the second line
        assert_eq!(ta.cursor_pos(), 6);

        ta.handle_event(&Event::key_press(36, 0));
        assert_eq!(ta.cursor_pos(), 4, "plain Home is the start of this line");

        ta.handle_event(&Event::key_press(35, 0));
        assert_eq!(ta.cursor_pos(), 7, "plain End is the end of this line, before its newline");

        // Bit 3 is the event's Meta/Command bit, i.e. the portable primary accelerator.
        ta.handle_event(&Event::key_press(36, 0b1000));
        assert_eq!(ta.cursor_pos(), 0, "Primary+Home is the start of the document");

        ta.handle_event(&Event::key_press(35, 0b1000));
        assert_eq!(ta.cursor_pos(), 13, "Primary+End is the end of the document");
    }

    /// Up and Down cross **logical** lines and keep the column.
    ///
    /// # The defect this pins
    ///
    /// Up and Down were unhandled, so key 38 (`&`) and 40 (`(`) were typed as text. The only way to
    /// reach another line was Home then Left, repeated — which is not a text editor. The column is
    /// counted in characters so a multi-byte line does not pull the caret past its own glyph, and a
    /// shorter destination line saturates at its end rather than refusing the move.
    #[test]
    fn up_and_down_cross_lines_keeping_the_column() {
        let mut ta = area("abcd\né\nwxyz");
        ta.set_cursor_pos(3); // column 3 of the first line

        ta.handle_event(&Event::key_press(40, 0));
        // The middle line is `é\n` over bytes 5..7, so its content ends at byte 7 — `é` is two
        // bytes, which is why the saturation point is not the byte right after the newline.
        assert_eq!(ta.cursor_pos(), 7, "down saturates at the end of the short line");

        ta.handle_event(&Event::key_press(40, 0));
        // The column is **re-derived from the caret's own line** each time rather than remembered
        // across the move. After saturating on the short line above, the caret sits at its end
        // (byte 7, i.e. column 1 of nothing), so this Down carries column 1 into `wxyz` — byte 9 —
        // and not the column 3 the gesture started with. Remembering the original column would be
        // the "sticky column" feature, which is a deliberate design choice this control has not
        // made; asserting 10 here would demand a behaviour the code does not promise.
        assert_eq!(ta.cursor_pos(), 9, "down carries the column the caret actually has");

        ta.handle_event(&Event::key_press(38, 0));
        assert_eq!(ta.cursor_pos(), 7, "up saturates on the short line again");

        ta.handle_event(&Event::key_press(38, 0));
        // Up carries column 1 into `abcd`, which is byte 1 — the caret is never left on a column
        // the destination line does not have.
        assert_eq!(ta.cursor_pos(), 1);
    }

    /// Vertical movement at the first and last line keeps the caret rather than being swallowed.
    #[test]
    fn vertical_movement_at_the_ends_keeps_the_caret() {
        let mut ta = area("one\ntwo");
        ta.set_cursor_pos(2);
        ta.handle_event(&Event::key_press(38, 0));
        assert_eq!(ta.cursor_pos(), 2, "no line above: the caret keeps its place");

        ta.set_cursor_pos(7);
        ta.handle_event(&Event::key_press(40, 0));
        assert_eq!(ta.cursor_pos(), 7, "no line below either");
    }

    // ─── Selection: editing over it ───

    /// Typing over a selection replaces it — the operation that makes selecting worth having.
    #[test]
    fn typing_over_a_selection_replaces_it() {
        let mut ta = area("hello world");
        ta.set_cursor_pos(5);
        for _ in 0..5 {
            ta.handle_event(&Event::key_press(37, SHIFT_BIT));
        }
        assert_eq!(ta.selected_text(), "hello");

        ta.handle_event(&Event::key_press(88, 0)); // 'X'
        assert_eq!(ta.text(), "X world", "the selection was consumed, not kept beside the insert");
        assert_eq!(ta.cursor_pos(), 1, "and the caret sits after what replaced it");
        assert_eq!(ta.selection_anchor(), None, "the range it described is gone");
    }

    /// `insert_char` is the canonical spelling shared with the other text fields (WDGT-14).
    #[test]
    fn insert_char_is_the_canonical_single_character_insert() {
        let mut ta = area("");
        ta.insert_char('é');
        assert_eq!(ta.text(), "é");
        assert_eq!(
            ta.cursor_pos(),
            'é'.len_utf8(),
            "the caret follows the character's byte length"
        );
    }

    /// Backspace and Delete both remove the whole selection rather than one character of it.
    #[test]
    fn backspace_and_delete_consume_the_selection() {
        for key in [8u32, 46] {
            let mut ta = area("abcdef");
            ta.set_cursor_pos(1);
            for _ in 0..3 {
                ta.handle_event(&Event::key_press(39, SHIFT_BIT));
            }
            assert_eq!(ta.selected_text(), "bcd");
            ta.handle_event(&Event::key_press(key, 0));
            assert_eq!(ta.text(), "aef", "key {key} removed the selection whole");
            assert_eq!(ta.cursor_pos(), 1, "key {key} left the caret at its start");
        }
    }

    /// Delete removes the character **after** the caret.
    ///
    /// # The defect this pins
    ///
    /// `Delete` was not handled, so key 46 travelled to the character-input catch-all where 46 is
    /// `.` — pressing Delete typed a full stop and deleted nothing. The one key whose entire job is
    /// forward deletion was the one key that inserted text.
    #[test]
    fn delete_removes_the_character_after_the_caret() {
        let mut ta = area("abc");
        ta.set_cursor_pos(0);
        ta.handle_event(&Event::key_press(46, 0));
        assert_eq!(ta.text(), "bc", "the character after the caret is gone");
        assert_eq!(ta.cursor_pos(), 0);

        // At the end there is nothing forward to remove, and the value must be untouched.
        ta.set_cursor_pos(2);
        ta.handle_event(&Event::key_press(46, 0));
        assert_eq!(ta.text(), "bc");
    }

    /// The forward delete is undoable, like every other edit.
    #[test]
    fn a_forward_delete_is_undoable() {
        let mut ta = area("abc");
        ta.set_cursor_pos(0);
        ta.handle_event(&Event::key_press(46, 0));
        assert_eq!(ta.text(), "bc");
        assert!(ta.undo(), "the deletion was recorded");
        assert_eq!(ta.text(), "abc", "and undo restores the deleted character");
    }

    /// A control chord is not text: `Primary+b` must not insert a `b`.
    ///
    /// # The defect this pins
    ///
    /// The catch-all typed every unmapped key, so `Primary+B`, `Primary+Q` and every other host
    /// accelerator became literal characters in the document.
    #[test]
    fn a_control_chord_is_not_typed_into_the_area() {
        let mut ta = area("");
        for (name, bits) in [("Ctrl", 0b0010u32), ("Alt", 0b0100), ("Primary", 0b1000)] {
            ta.handle_event(&Event::key_press(98, bits));
            assert!(ta.text().is_empty(), "{name}+b must not insert a `b`");
        }
        // A control code on its own is not content either.
        ta.handle_event(&Event::key_press(0, 0)); // NUL
        assert!(ta.text().is_empty(), "a bare control code is not content");
        ta.handle_event(&Event::key_press(98, 0));
        assert_eq!(ta.text(), "b");
    }

    /// Primary+A selects the whole document.
    #[test]
    fn primary_a_selects_the_whole_document() {
        let mut ta = area("one\ntwo");
        ta.set_cursor_pos(0);
        ta.handle_event(&Event::key_press(65, 0b1000));
        assert_eq!(ta.selected_text(), "one\ntwo");
        assert_eq!(ta.cursor_pos(), 7, "the caret is at the end, as select-all leaves it");
    }

    // ─── Selection: the programmatic surface ───

    /// `select_line` covers the line **and** its terminator, so a following delete removes the whole
    /// line rather than leaving an empty one behind.
    #[test]
    fn select_line_covers_the_line_and_its_terminator() {
        let mut ta = area("one\ntwo\nthree");
        assert!(ta.select_line(1), "line 1 exists");
        assert_eq!(ta.selected_text(), "two\n", "the terminator belongs to the line it ends");

        ta.handle_event(&Event::key_press(8, 0)); // backspace over the selection
        assert_eq!(ta.text(), "one\nthree", "the line went with its newline");

        assert!(!ta.select_line(9), "a line that does not exist changes nothing");
    }

    /// `insert_str` is one edit, so one undo press takes it all back.
    ///
    /// # The defect this pins
    ///
    /// The obvious implementation is a loop over `insert`, which files one snapshot per character:
    /// undoing a paste would then take as many presses as the paste had characters.
    #[test]
    fn insert_str_is_a_single_undo_step() {
        let mut ta = area("[]");
        ta.set_cursor_pos(1);
        ta.insert_str("hello");
        assert_eq!(ta.text(), "[hello]");
        assert_eq!(ta.cursor_pos(), 6, "the caret lands after the inserted run");
        assert!(ta.undo(), "one snapshot covers the whole insertion");
        assert_eq!(ta.text(), "[]", "and one undo takes all of it back");
    }

    /// A multi-byte value can be selected and deleted without splitting a character.
    #[test]
    fn a_multi_byte_selection_is_taken_by_character() {
        let mut ta = area("héllo 中文");
        ta.set_cursor_pos(0);
        ta.select_with_modifiers(7, crate::shortcut::Modifiers::SHIFT);
        assert_eq!(ta.selected_text(), "héllo ", "six characters, seven bytes");
        ta.handle_event(&Event::key_press(8, 0));
        assert_eq!(ta.text(), "中文", "the whole selection went, with no fragment left behind");
    }

    // ─── Selection: the pointer ───

    /// The x offset, from the text origin, at which the `n`-th character of `line` begins.
    ///
    /// Computed through the renderer's own measurement rather than `n * 8`: these tests exist to show
    /// that a click resolves to the character the user pointed at, so aiming them with a constant
    /// that disagrees with the paint would have made them pass for the wrong reason.
    fn x_of_char(line: &str, n: usize) -> i32 {
        let prefix: String = line.chars().take(n).collect();
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(0, 0));
        crate::render::RenderContext::new(&mut backend)
            .measure_text(&prefix, &crate::core::Font::default())
            .width as i32
    }

    /// The y of a point one line below the first line's text top.
    fn click_y(ta: &TextArea) -> i32 {
        ta.geometry().y + FIRST_LINE_INSET + (LINE_H - Font::default().size_i32()) / 2 + LINE_H / 2
    }

    /// The x of the text origin.
    fn pen_x(ta: &TextArea) -> i32 {
        ta.geometry().x + dimensions::TEXT_FIELD_PADDING_H as i32
    }

    /// A press puts the caret where the pointer landed and anchors a selection there.
    ///
    /// # The defect this pins
    ///
    /// A press used to only focus the control. `set_text` leaves the caret at the end, so clicking
    /// into the middle of an existing document and typing appended to it rather than inserting — and
    /// there was no pointer selection at all, because nothing computed an offset from a pixel.
    #[test]
    fn a_press_places_the_caret_where_the_pointer_landed() {
        let mut ta = area("abcdef");
        assert_eq!(ta.cursor_pos(), 6, "`set_text` leaves the caret at the end");

        // Aim at the *middle* of the second character, measured: the half-cell offset is what makes
        // this a test of "the character the pointer is over" rather than of a rounding rule.
        let line = ta.text().lines().next().unwrap_or("").to_string();
        let (x, y) = (pen_x(&ta) + x_of_char(&line, 1) + 2, click_y(&ta));
        ta.handle_event(&Event::mouse_press(x, y, 1));
        assert_eq!(ta.cursor_pos(), 1, "the caret lands on the character clicked");
        assert_eq!(ta.selection_anchor(), Some(1), "and it is the anchor for a drag");
    }

    /// A drag extends the selection; a hover that was never pressed does not move the caret.
    #[cfg(device_profile)]
    #[test]
    fn a_drag_extends_the_selection_but_a_bare_hover_does_not() {
        let mut ta = area("abcdef");
        let (origin, y) = (pen_x(&ta), click_y(&ta));

        // A hover with no press ever delivered leaves the caret exactly where it was.
        ta.handle_event(&Event::mouse_move(origin + 60, y));
        assert_eq!(ta.cursor_pos(), 6, "an unpressed hover must not move the caret");
        assert_eq!(ta.selection_anchor(), None);

        let line = ta.text().lines().next().unwrap_or("").to_string();
        let x1 = origin + x_of_char(&line, 1) + 2;
        let x4 = origin + x_of_char(&line, 4) + 2;
        ta.handle_event(&Event::mouse_press(x1, y, 1));
        ta.handle_event(&Event::mouse_move(x4, y));
        assert_eq!(ta.selected_text(), "bcd", "the drag selected the span between the points");

        // The release ends the gesture, so a later hover is inert again and the range is kept.
        ta.handle_event(&Event::mouse_release(x4, y, 1));
        ta.handle_event(&Event::mouse_move(origin + 140, y));
        assert_eq!(ta.cursor_pos(), 4, "a hover after the release does not extend");
        assert_eq!(ta.selected_text(), "bcd", "and the selection the user made is kept");
    }

    /// A click on the first glyph resolves to index 0 — the map and the paint agree on the origin.
    #[test]
    fn a_click_on_the_first_glyph_resolves_to_index_zero() {
        let mut ta = area("Sample");
        let (x, y) = (pen_x(&ta), click_y(&ta));
        ta.handle_event(&Event::mouse_press(x, y, 1));
        assert_eq!(ta.cursor_pos(), 0, "the first pixel of the value is index 0");
    }

    /// Every click is total: inside the value, and never inside a character.
    #[test]
    fn a_click_is_total_on_a_multi_line_multi_byte_value() {
        let mut ta = area("héllo\n中文");
        let rect = ta.geometry();
        for dy in (0..rect.height as i32).step_by(7) {
            for dx in (0..rect.width as i32).step_by(13) {
                ta.handle_event(&Event::mouse_press(rect.x + dx, rect.y + dy, 1));
                let caret = ta.cursor_pos();
                assert!(caret <= ta.text().len(), "the caret is inside the value at ({dx},{dy})");
                assert!(
                    ta.text().is_char_boundary(caret),
                    "the caret at ({dx},{dy}) split a character: {caret} in {:?}",
                    ta.text()
                );
            }
        }
    }

    /// A press on the second line resolves to that line, not to the first.
    ///
    /// This is the multi-line half of pixel-to-offset mapping: a single-line field never has to
    /// decide, so an implementation that ignored `y` would look correct there and be wrong here.
    #[test]
    fn a_press_on_the_second_line_resolves_to_the_second_line() {
        let mut ta = area("abc\ndef");
        let (x, y) = (pen_x(&ta), click_y(&ta));
        // One cell into the second row, measured against that row's own text.
        let second = ta.text().lines().nth(1).unwrap_or("").to_string();
        ta.handle_event(&Event::mouse_press(x + x_of_char(&second, 1) + 2, y + LINE_H, 1));
        // First line is `abc\n` (4 bytes), so column 1 of the second line is byte 5.
        assert_eq!(ta.cursor_pos(), 5, "the caret is in `def`, not in `abc`");
    }

    // ─── Selection: the paint ───

    /// The selection is painted **behind** the value's ink, not over it.
    ///
    /// # Why the assertion is on the glyph box rather than on a colour
    ///
    /// A band drawn after the text would cover the words the user just selected. Asserting that the
    /// band exists does not catch that; asserting that the *ink did not move* comes much closer,
    /// because a highlight painted over the glyphs sits in the same place but a highlight that
    /// displaced them would show up here.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_selection_is_painted_without_moving_the_value() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut ta = area("abcdef");
        let plain = crate::widget::svg::render_to_svg(&mut ta);

        // The caret starts at the end (`new` leaves it there), so it is placed explicitly — a
        // selection grown from an unstated caret would be testing the wrong range.
        ta.set_cursor_pos(0);
        ta.select_with_modifiers(3, crate::shortcut::Modifiers::SHIFT);
        assert_eq!(ta.selected_text(), "abc");
        let selected = crate::widget::svg::render_to_svg(&mut ta);

        assert_ne!(plain, selected, "a selection must be visible");
        assert_eq!(
            crate::widget::svg::text_ink_box(&selected),
            crate::widget::svg::text_ink_box(&plain),
            "and it must not move or cover the glyphs it sits behind"
        );
    }

    /// Nothing selected paints exactly as it did, so the highlight is additive.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_caret_without_a_selection_paints_no_band() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut ta = area("abcdef");
        ta.set_cursor_pos(3);
        assert_eq!(ta.selection_range(), None, "a caret alone is not a selection");
        let with_caret = crate::widget::svg::render_to_svg(&mut ta);

        ta.clear_selection();
        let after = crate::widget::svg::render_to_svg(&mut ta);
        assert_eq!(with_caret, after, "clearing a selection that was not there changed nothing");
    }

    /// A selection spanning a newline is drawn as more than one band.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_multi_line_selection_is_drawn_per_line() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut ta = area("ab\ncd");

        let unselected = crate::widget::svg::render_to_svg(&mut ta);
        ta.set_cursor_pos(0);
        ta.select_with_modifiers(4, crate::shortcut::Modifiers::SHIFT);
        assert_eq!(ta.selected_text(), "ab\nc");
        let selected = crate::widget::svg::render_to_svg(&mut ta);

        // The band is drawn as rectangles, and a two-line range needs two of them. Counting the
        // added rects is what distinguishes "per line" from "one full-height bar".
        let added = selected.matches("<rect").count() - unselected.matches("<rect").count();
        assert_eq!(added, 2, "one band per line the range touches");
    }

    /// The selection band is **filled**, not merely outlined.
    ///
    /// `draw_rect` is a one-pixel outline (see `RenderContext::draw_rect`), so the band used to
    /// paint a hollow box around the run: the glyphs were not highlighted, and the field colour
    /// inside the box was unchanged. `TextEdit` fills the same arithmetic with `fill_rect`, and
    /// this pins the text area to that shape.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_selection_band_is_filled_not_outlined() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut ta = area("abcdef");
        let plain = crate::widget::svg::render_to_svg(&mut ta);
        ta.set_cursor_pos(0);
        ta.select_with_modifiers(3, crate::shortcut::Modifiers::SHIFT);
        let selected = crate::widget::svg::render_to_svg(&mut ta);

        /// Every `<rect ... />` element, as an owned string so the two renders can be diffed.
        fn rects(svg: &str) -> crate::compat::Vec<String> {
            svg.split("<rect ")
                .skip(1)
                .filter_map(|chunk| chunk.split_once("/>"))
                .map(|(element, _)| element.to_string())
                .collect()
        }

        let before = rects(&plain);
        let added: crate::compat::Vec<String> =
            rects(&selected).into_iter().filter(|r| !before.contains(r)).collect();
        assert_eq!(added.len(), 1, "the selection adds exactly one rectangle: {added:?}");
        let band = &added[0];
        assert!(
            band.contains("fill=\"rgba") && !band.contains("fill=\"none\""),
            "the band must be filled, not an outline: {band}"
        );
        assert!(!band.contains("stroke="), "an outline would carry a stroke: {band}");
    }

    /// A long line is clipped to the field rather than painted over its neighbours.
    ///
    /// The run used to be emitted straight across the control's right edge (the runtime only
    /// clips a widget whose `children_clip()` is set, which a text area does not override). The
    /// clip rectangle is the field's **inner box**, which is also where the caret and the
    /// selection band stop, and it still bounds the run while the value is scrolled sideways.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_long_line_is_clipped_to_the_fields_inner_box() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut ta = TextArea::new("x".repeat(200), Rect::new(0, 0, 120, 60));
        let svg = crate::widget::svg::render_to_svg(&mut ta);

        let padding = crate::widget::metrics::dimensions::TEXT_FIELD_PADDING_H;
        let expected =
            format!(r#"<rect x="{padding}" y="1" width="{}" height="58" />"#, 120 - 2 * padding);
        assert!(
            svg.contains(&expected),
            "the value must be clipped to the field's inner box ({expected})"
        );
    }

    /// A long unwrapped line scrolls sideways to keep its caret visible.
    ///
    /// Without this the caret simply ran past the field's trailing edge: the user could move
    /// it there but could not see it, and the characters around it were the part of the value
    /// that disappeared. The offset is driven by the caret, so returning to the start of the
    /// line brings the leading edge back.
    #[test]
    fn a_long_unwrapped_line_scrolls_to_reveal_the_caret() {
        let mut ta = TextArea::new("x".repeat(200), Rect::new(0, 0, 120, 60));
        assert_eq!(
            ta.get("scroll_x").unwrap(),
            CapabilityValue::Int(0),
            "a freshly installed value opens at its leading edge"
        );

        ta.handle_event(&Event::KeyPress { key: 35, modifiers: 0 }); // End
        let offset = match ta.get("scroll_x").unwrap() {
            CapabilityValue::Int(offset) => offset,
            other => panic!("scroll_x must be an Int, got {other:?}"),
        };
        assert!(offset > 0, "the end of a 200 px line must be scrolled into view");

        ta.handle_event(&Event::KeyPress { key: 36, modifiers: 0 }); // Home
        assert_eq!(
            ta.get("scroll_x").unwrap(),
            CapabilityValue::Int(0),
            "returning to the start restores the leading edge"
        );
    }

    // ─── Scrolling ───

    /// A short field whose value is taller than it: the fixture every scroll test uses.
    fn tall_value(field_height: u32) -> TextArea {
        // Built through `crate::compat::Vec` rather than `std`'s, because under `mini` the crate is
        // `no_std` and this file's test module does not import a `Vec` of its own.
        let lines: crate::compat::Vec<String> = (0..20).map(|i| format!("line{i}")).collect();
        let text = lines.join("\n");
        TextArea::new(text, Rect::new(0, 0, 200, field_height))
    }

    /// Typing past the bottom row scrolls the viewport instead of writing out of sight.
    ///
    /// # The defect this pins
    ///
    /// The paint path already stopped at the last whole row it could fit, so a value taller than the
    /// field did not overrun its own rectangle — the probe measured the ink box staying inside. What
    /// was missing was any way to *reach* the rows it skipped: there was no scroll position at all, so
    /// the field always showed the document's first lines and a caret moved below them was editable
    /// but invisible.
    #[test]
    fn the_viewport_follows_the_caret_down() {
        let mut ta = tall_value(60);
        assert_eq!(ta.first_visible_line(), 0, "a fresh field starts at the top");
        let visible = 3; // 60 px / 16 px rows, less the first-line inset

        // Put the caret on a line well below the viewport.
        ta.set_cursor_pos(0);
        ta.select_with_modifiers(
            ta.text().find("line9").expect("the value has line9"),
            crate::shortcut::Modifiers::NONE,
        );
        assert!(
            ta.first_visible_line() > 0,
            "the viewport moved to keep the caret in view, got {}",
            ta.first_visible_line()
        );
        assert!(
            ta.first_visible_line() <= 9 && 9 < ta.first_visible_line() + visible,
            "line 9 is inside the visible window [{}, {})",
            ta.first_visible_line(),
            ta.first_visible_line() + visible
        );
    }

    /// The scroll is **minimal**: moving the caret to the top brings the view back, and it does not
    /// overshoot past what is needed.
    #[test]
    fn the_viewport_scrolls_back_minimally() {
        let mut ta = tall_value(60);
        let line9 = ta.text().find("line9").expect("the value has line9");

        ta.set_cursor_pos(line9);
        ta.select_with_modifiers(line9, crate::shortcut::Modifiers::NONE);
        let scrolled = ta.first_visible_line();
        assert!(scrolled > 0, "we are scrolled down");

        // Back to the very start: the viewport returns to the top, not to `scrolled - 1`.
        ta.set_cursor_pos(0);
        ta.select_with_modifiers(0, crate::shortcut::Modifiers::NONE);
        assert_eq!(ta.first_visible_line(), 0, "returning to the first line shows the first line");
    }

    /// A value that fits never scrolls, so a small document is unaffected by the feature.
    #[test]
    fn a_value_that_fits_never_scrolls() {
        let mut ta = TextArea::new("one\ntwo".to_string(), Rect::new(0, 0, 200, 200));
        ta.select_all();
        assert_eq!(ta.first_visible_line(), 0, "nothing to scroll to");
        ta.set_cursor_pos(ta.text().len());
        assert_eq!(ta.first_visible_line(), 0);
    }

    /// A click after scrolling resolves to the line **drawn** on that row.
    ///
    /// # The defect this pins
    ///
    /// The pixel-to-offset map counted rows from the top of the document, so once the viewport had
    /// scrolled, a click on the first visible row resolved to a line the user could not see — the
    /// caret landed above the field. Translating the viewport row through the same
    /// `first_visible_line` the paint uses is what keeps the click and the glyphs on one row.
    #[test]
    fn a_click_after_scrolling_resolves_to_the_drawn_line() {
        let mut ta = tall_value(60);
        let target = ta.text().find("line9").expect("the value has line9");

        // Scroll so that line 9 is on the first visible row.
        ta.set_cursor_pos(target);
        ta.select_with_modifiers(target, crate::shortcut::Modifiers::NONE);
        let first = ta.first_visible_line();
        assert!(first > 0, "the field is scrolled");

        // A click on the top row, at the leading edge, must land on the line painted there.
        let rect = ta.geometry();
        let y = rect.y + FIRST_LINE_INSET + (LINE_H - Font::default().size_i32()) / 2 + 1;
        let x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32 + 1;
        ta.handle_event(&Event::mouse_press(x, y, 1));

        let expected = ta.text().split_inclusive('\n').take(first).map(str::len).sum::<usize>();
        assert_eq!(
            ta.cursor_pos(),
            expected,
            "the click resolved to line {first}, the one drawn on the top row"
        );
    }

    /// Scrolling does not move the caret, and does not fabricate one.
    #[test]
    fn scrolling_alone_does_not_move_the_caret() {
        let mut ta = tall_value(60);
        let target = ta.text().find("line9").expect("the value has line9");
        ta.set_cursor_pos(target);
        let caret = ta.cursor_pos();
        // Scroll further, then back, without touching the caret.
        ta.select_with_modifiers(target, crate::shortcut::Modifiers::NONE);
        assert_eq!(ta.cursor_pos(), caret, "the caret is where it was put");
    }

    /// The line table is one pass over the value, and it keeps the document's own line ranges.
    ///
    /// # What this pins
    ///
    /// The paint path used to ask for "the nth line" once per visible row and rescan the document
    /// for every answer, and it counted those rows' byte offsets from the first **visible** line
    /// rather than from the document's start. [`TextArea::line_spans`] is what the ink, the
    /// selection band and the click map now index, so it has to agree with the `lines()` split the
    /// painter was written against — including the two cases that are easy to get wrong: an empty
    /// value (one empty line, which `split_inclusive` yields nothing for) and a value ending in a
    /// terminator (an empty line after it, which `line_count` counts and a caret can sit on).
    #[test]
    fn the_line_table_agrees_with_str_lines() {
        for text in ["", "a", "a\n", "a\n\nb", "alpha\r\nbeta\r\n", "你好\n世界"] {
            let ta = TextArea::new(text.to_string(), Rect::new(0, 0, 200, 60));
            let spans = ta.line_spans();

            let expected_len =
                text.lines().count() + usize::from(text.is_empty() || text.ends_with('\n'));
            assert_eq!(spans.len(), expected_len, "one span per line of {text:?}");
            assert_eq!(spans.len(), ta.line_count(), "and `line_count` reads the same table");

            for (index, &(start, content_end, end)) in spans.iter().enumerate() {
                let expected = text.lines().nth(index).unwrap_or("");
                assert_eq!(&ta.text()[start..content_end], expected, "line {index} of {text:?}");
                assert!(start <= content_end && content_end <= end, "the span is ordered");
                assert_eq!(
                    spans.get(index + 1).map(|next| next.0),
                    (index + 1 < spans.len()).then_some(end),
                    "each line ends where the next one starts"
                );
            }

            // And the index arithmetic: every byte offset lands on the line that counting newlines
            // before it would name, which is the answer the caret and the scroll offset use.
            for offset in text.char_indices().map(|(index, _)| index).chain([text.len()]) {
                let naive = text[..offset].chars().filter(|&c| c == '\n').count();
                assert_eq!(
                    ta.line_index_of(offset),
                    naive,
                    "offset {offset} in {text:?} belongs to line {naive}"
                );
            }
        }
    }

    /// A selection that begins on a scrolled line still paints its band.
    ///
    /// # The defect this pins
    ///
    /// The paint loop accumulated each row's byte offsets as it walked the **visible** rows, starting
    /// from zero. Once the field was scrolled, that offset belonged to the top of the viewport rather
    /// than to the line being painted, so the overlap with the selection came out empty (`from ==
    /// to`) and a scrolled field drew no highlight at all — the text a user dragged over was
    /// invisible. The offsets now come from the document (see [`TextArea::line_spans`]).
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_selection_on_a_scrolled_line_still_paints_its_band() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut ta = tall_value(60);
        let line9 = ta.text().find("line9").expect("the value has line9");
        ta.set_cursor_pos(line9);
        ta.select_with_modifiers(line9, crate::shortcut::Modifiers::NONE);
        assert!(ta.first_visible_line() > 0, "the field is scrolled to line 9");

        let unselected = crate::widget::svg::render_to_svg(&mut ta);
        assert!(ta.select_line(9), "line 9 exists");
        assert_eq!(ta.selected_text(), "line9\n", "the range includes the line's terminator");
        let selected = crate::widget::svg::render_to_svg(&mut ta);

        assert!(
            selected.matches("<rect").count() > unselected.matches("<rect").count(),
            "selecting a line while scrolled must paint its band"
        );
    }

    /// The wheel scrolls the viewport by lines and clamps at both ends of the document.
    ///
    /// # The defect this pins
    ///
    /// The field had no wheel arm at all, so a value taller than its box could only be scrolled
    /// with the arrow keys. This drives the same `first_visible_line` the caret path uses, so the
    /// painted rows and the scroll position cannot disagree.
    #[test]
    fn textarea_wheel_scrolls_and_clamps() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let mut ta = TextArea::new(text.join("\n"), Rect::new(0, 0, 200, 60));
        assert_eq!(ta.first_visible_line(), 0);

        // One notch toward the content's end reveals three lines.
        ta.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(ta.first_visible_line(), 3, "a downward notch moves the viewport down");

        // A notch the other way returns toward the start and cannot go below zero.
        ta.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(ta.first_visible_line(), 0);
        ta.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(ta.first_visible_line(), 0, "the viewport clamps at the first line");

        // Scrolling past the end stops at the final whole window.
        let max_first = ta.line_count() - ta.visible_line_count();
        for _ in 0..20 {
            ta.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        }
        assert_eq!(ta.first_visible_line(), max_first, "the viewport clamps at the last window");
    }

    /// The Material profile keeps the text area's historical 60 px minimum.
    #[cfg(device_profile)]
    #[test]
    fn the_material_profile_keeps_the_text_area_height_floor() {
        let _theme_guard = crate::style::theme_test_guard();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        let ta = TextArea::new(String::new(), Rect::new(0, 0, 240, 120));
        assert_eq!(ta.size_hint().height, 60, "the three-row Material budget is unchanged");
    }

    /// A Macos-profile theme lets an empty area start at the compact field height.
    ///
    /// The area still grows with its text: the profile only replaces the floor, so a value with
    /// more rows asks for the same height it always did.
    #[cfg(device_profile)]
    #[test]
    fn a_macos_profile_theme_lowers_the_text_area_height_floor() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut theme = crate::theme::Theme::default();
        theme.name = "p2-6-textarea-macos".to_string();
        theme.metrics_profile = crate::widget::metrics::ControlMetricsProfile::Macos;
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            assert!(manager.set_theme("p2-6-textarea-macos"));
        }

        let empty = TextArea::new(String::new(), Rect::new(0, 0, 240, 120));
        let one_line = TextArea::new("a\nb\nc\nd".to_string(), Rect::new(0, 0, 240, 120));
        let empty_hint = empty.size_hint();
        let tall_hint = one_line.size_hint();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        assert_eq!(empty_hint.height, LINE_H as u32 + 10, "one compact row: {empty_hint:?}");
        assert!(empty_hint.height < 60, "the compact profile shrinks the floor: {empty_hint:?}");
        assert_eq!(
            tall_hint.height,
            4 * LINE_H as u32 + 10,
            "content-driven rows are unchanged: {tall_hint:?}"
        );
    }

    /// The Cupertino profile keeps the Material height floor for a text area.
    #[cfg(device_profile)]
    #[test]
    fn the_cupertino_profile_keeps_the_text_area_height_floor() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut theme = crate::theme::Theme::default();
        theme.name = "p2-6-textarea-cupertino".to_string();
        theme.metrics_profile = crate::widget::metrics::ControlMetricsProfile::Cupertino;
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            assert!(manager.set_theme("p2-6-textarea-cupertino"));
        }

        let hint = TextArea::new(String::new(), Rect::new(0, 0, 240, 120)).size_hint();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        assert_eq!(hint.height, 60);
    }

    /// Soft wrap adds display rows and breaks long lines at word boundaries.
    #[test]
    fn soft_wrap_adds_word_boundary_rows() {
        let mut ta = TextArea::new("alpha beta gamma delta".to_string(), Rect::new(0, 0, 90, 200));
        let unwrapped = ta.display_rows().len();
        ta.set_soft_wrap(true);
        let wrapped = ta.display_rows();
        assert!(wrapped.len() > unwrapped, "wrapping must add rows: {wrapped:?}");
        let text = ta.text().to_string();
        for pair in wrapped.windows(2) {
            let boundary = pair[0].end;
            if boundary < text.len() && pair[1].start == boundary {
                assert!(
                    text[..boundary].ends_with(' ') || text[boundary..].starts_with(' '),
                    "a soft break must land at a space: {wrapped:?}"
                );
            }
        }
    }

    /// The line budget drops the oldest lines and moves the caret with the surviving text.
    #[test]
    fn a_block_budget_drops_the_oldest_lines() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_block_count(Some(2));
        ta.insert_str("one\ntwo\nthree");
        assert_eq!(ta.line_count(), 2);
        assert_eq!(ta.text(), "two\nthree");
        assert!(ta.cursor_pos() <= ta.text().len());
    }

    /// A content margin moves the ink and the hit test together.
    #[test]
    fn a_content_margin_insets_the_value() {
        use crate::widget::svg::render_to_svg;
        let mut plain = TextArea::new("margined".to_string(), Rect::new(0, 0, 200, 80));
        let plain_svg = render_to_svg(&mut plain);
        plain.set_content_margin(10);
        assert_eq!(plain.content_margin(), 10);
        assert_ne!(plain_svg, render_to_svg(&mut plain), "the margin must move the glyphs");
    }

    /// Primary+C and Primary+V move the selected text through the clipboard.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn clipboard_shortcuts_copy_and_paste() {
        let _clipboard = crate::clipboard::clipboard_test_guard();
        let mut source = TextArea::new("copy me".to_string(), Rect::new(0, 0, 200, 80));
        source
            .handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        source.select_all();
        source.handle_event(&Event::KeyPress { key: 67, modifiers: 0b1000 });
        assert_eq!(crate::get_clipboard_text(), "copy me");

        let mut target = TextArea::new(String::new(), Rect::new(0, 0, 200, 80));
        target
            .handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        target.handle_event(&Event::KeyPress { key: 86, modifiers: 0b1000 });
        assert_eq!(target.text(), "copy me");
    }

    /// A double click selects the word under the pointer.
    #[test]
    fn a_double_click_selects_a_word() {
        let mut ta = TextArea::new("alpha beta".to_string(), Rect::new(0, 0, 300, 80));
        ta.handle_event(&Event::MouseDoubleClick { pos: Point::new(8, 8), button: 1 });
        assert_eq!(ta.selected_text(), "alpha");
    }

    /// Page Down and Page Up move the caret by a window of display rows.
    #[test]
    fn page_keys_move_the_caret() {
        let mut ta = TextArea::new(
            (0..30).map(|n| format!("line {n}\n")).collect::<String>(),
            Rect::new(0, 0, 200, 80),
        );
        ta.set_cursor_pos(0);
        ta.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        ta.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        let after_down = ta.cursor_pos();
        assert!(after_down > 0);
        ta.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert!(ta.cursor_pos() < after_down);
    }

    /// Tab is content in a document editor, not a focus gesture.
    #[test]
    fn tab_inserts_a_tab_character() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 200, 80));
        ta.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
        ta.handle_event(&Event::KeyPress { key: 9, modifiers: 0 });
        assert_eq!(ta.text(), "\t");
    }

    /// The selection-band widths are memoised for the current revision.
    #[test]
    fn the_band_width_cache_is_scoped_to_the_revision() {
        let mut ta = TextArea::new("measured".to_string(), Rect::new(0, 0, 200, 80));
        let width = ta.band_advance(0, 4);
        assert_eq!(ta.band_width_cache.borrow().as_ref().map_or(0, |c| c.entries.len()), 1);
        assert_eq!(ta.band_advance(0, 4), width);
        assert_eq!(
            ta.band_width_cache.borrow().as_ref().map_or(0, |c| c.entries.len()),
            1,
            "a hit must not append"
        );
        ta.insert_str("x");
        let _ = ta.band_advance(0, 4);
        assert_eq!(
            ta.band_width_cache.borrow().as_ref().map_or(0, |c| c.entries.len()),
            1,
            "the new revision starts clean"
        );
    }

    /// A middle-paragraph edit patches the row table instead of re-wrapping the document.
    #[test]
    fn an_incremental_row_rebuild_matches_the_full_table_for_a_middle_line_edit() {
        let mut ta = TextArea::new(
            "first line\nsecond line that is long enough to wrap around\nthird line".to_string(),
            Rect::new(0, 0, 90, 200),
        );
        ta.set_soft_wrap(true);
        let _ = ta.display_rows();
        let stale_revision = ta.display_rows_cache.borrow().as_ref().map(|cache| cache.revision);
        let middle = ta.text().find("long").expect("the middle paragraph has the token");
        ta.set_cursor_pos(middle);
        ta.insert_char('X');
        // The edit's own scroll pass already folds the splice into the row table. If that pass
        // failed the guard, the cache would still be stale here and the comparison below would
        // still pass, so assert the cache was actually rebuilt for the new revision.
        assert!(ta.pending_row_edit.get().is_none(), "the splice hint was consumed");
        assert_eq!(
            ta.display_rows_cache.borrow().as_ref().map(|cache| cache.revision),
            Some(ta.revision)
        );
        assert_ne!(stale_revision, Some(ta.revision));
        let incremental = ta.display_rows();
        // Drop the cache so the next read has to take the whole-table path.
        ta.display_rows_cache.borrow_mut().take();
        ta.pending_row_edit.set(None);
        let full = ta.display_rows();
        assert_eq!(*incremental, *full);
    }

    /// The incremental helper itself must return rows identical to a from-scratch build.
    #[test]
    fn an_incremental_splice_matches_the_full_rebuild() {
        let mut ta = TextArea::new(
            "first line\nsecond line that is long enough to wrap around\nthird line".to_string(),
            Rect::new(0, 0, 90, 200),
        );
        ta.set_soft_wrap(true);
        let _ = ta.display_rows();
        let cached = ta.display_rows_cache.borrow_mut().take().expect("the rows are seeded");
        let at = ta.text.find("long").expect("the middle paragraph has the token");
        ta.text.insert(at, 'X');
        ta.line_spans_cache.borrow_mut().take();
        ta.revision = ta.revision.wrapping_add(1);
        let width = ta.content_width();
        let font = ta.text_font().clone();
        let incremental = ta
            .incremental_display_rows(
                &cached,
                PendingEdit { start: at, removed: 0, inserted: 1 },
                width,
                &font,
            )
            .expect("the middle splice is reusable");
        let full = ta.build_display_rows(width, &font, None);
        assert_eq!(*incremental.rows, *full.rows);
    }

    /// An edit that merges two paragraphs cannot reuse the tail, but the fallback still agrees.
    #[test]
    fn a_paragraph_merging_edit_matches_the_full_table() {
        let mut ta = TextArea::new("one\ntwo\nthree".to_string(), Rect::new(0, 0, 120, 120));
        ta.set_soft_wrap(true);
        let _ = ta.display_rows();
        let newline = ta.text().find('\n').expect("the value has a terminator");
        ta.set_cursor_pos(newline);
        ta.delete_forward();
        let incremental = ta.display_rows();
        ta.display_rows_cache.borrow_mut().take();
        ta.pending_row_edit.set(None);
        let full = ta.display_rows();
        assert_eq!(*incremental, *full);
    }

    /// Undo records only the changed run, not a snapshot of the whole document.
    #[test]
    fn an_edit_is_recorded_as_a_delta() {
        let target = Rc::new(RefCell::new(String::new()));
        let command =
            TextDeltaCommand::new(target, "hello", "hello!", "Edit text", "text_area_text");
        assert_eq!(command.offset, 5, "the common prefix is not part of the delta");
        assert_eq!(command.removed, "");
        assert_eq!(command.inserted, "!");
    }

    // ─── COMP-A8: FindReplaceBar wiring ───

    /// COMP-A8: attaching reports the current query's matches and queues nothing.
    #[test]
    #[cfg(full_widgets)]
    fn the_find_bar_counts_matches_when_attached() {
        let mut ta = area("one two one two one");
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        bar.set_query("one");

        ta.attach_find_replace_bar(&bar);
        assert!(ta.is_find_replace_bar_attached());
        assert_eq!(bar.match_count(), 3, "attach reports the current query's matches");
        assert!(!ta.poll_find_replace_bar(), "attaching queues no intent");
    }

    /// COMP-A8: a queued navigation intent moves the area's selection on the next poll.
    #[test]
    #[cfg(full_widgets)]
    fn a_find_intent_moves_the_selection_on_poll() {
        let mut ta = area("one two one two one");
        ta.set_cursor_pos(0);
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        bar.set_query("one");
        ta.attach_find_replace_bar(&bar);

        bar.find_next.emit();
        assert!(ta.poll_find_replace_bar(), "the queued intent is applied");
        assert_eq!(ta.selection_range(), Some((0, 3)));
        assert_eq!((bar.match_count(), bar.current_match()), (3, 0));

        bar.find_next.emit();
        assert!(ta.poll_find_replace_bar());
        assert_eq!(ta.selection_range(), Some((8, 11)));
        assert_eq!(bar.current_match(), 1);

        bar.find_previous.emit();
        assert!(ta.poll_find_replace_bar());
        assert_eq!(ta.selection_range(), Some((0, 3)), "previous wraps to the first match");
        assert!(!ta.poll_find_replace_bar(), "the queue is drained");
    }

    /// COMP-A8: replace and replace-all go through the area's own edit path.
    #[test]
    #[cfg(full_widgets)]
    fn replace_intents_go_through_the_areas_edit_path() {
        let mut ta = area("one two one");
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        bar.set_query("one");
        bar.set_replacement("1");
        ta.attach_find_replace_bar(&bar);

        bar.replace_one.emit();
        assert!(ta.poll_find_replace_bar());
        assert_eq!(ta.text(), "1 two one", "the first match is replaced");
        assert_eq!(bar.match_count(), 1, "the counter follows the new document");

        bar.replace_all.emit();
        assert!(ta.poll_find_replace_bar());
        assert_eq!(ta.text(), "1 two 1");
        assert_eq!(bar.match_count(), 0, "nothing matches after the replacement");

        bar.replace_all.emit();
        assert!(!ta.poll_find_replace_bar(), "nothing left to replace");
    }

    /// COMP-A8: detaching stops the link and keeps the last reported state.
    #[test]
    #[cfg(full_widgets)]
    fn a_detached_find_bar_is_not_polled() {
        let mut ta = area("aaa");
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        bar.set_query("a");
        ta.attach_find_replace_bar(&bar);
        assert_eq!(bar.match_count(), 3);

        ta.detach_find_replace_bar();
        assert!(!ta.is_find_replace_bar_attached());
        bar.find_next.emit();
        assert!(!ta.poll_find_replace_bar(), "a detached bar reports nothing");
        assert_eq!(bar.match_count(), 3, "detach leaves the last count");
        assert_eq!(ta.selection_range(), None, "and the selection did not move");
    }
}
