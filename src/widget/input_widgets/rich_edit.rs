// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rich text editor widget.
use crate::core::{Font, HorizontalAlignment, Rect, Size};
use crate::event::wheel::WheelScroll;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;

/// A styled run of a [`RichEdit`] document.
///
/// The range is a byte range into the value and the format applies to every character it covers.
/// Spans may overlap (a later span wins) and need not cover the whole document.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    /// The byte range the format applies to.
    pub range: core::ops::Range<usize>,
    /// The format painted over that range.
    pub format: CharFormat,
}

/// The character attributes a [`TextSpan`] can carry.
///
/// The default inherits the editor's own style; an empty span list therefore paints exactly as
/// an unstyled document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CharFormat {
    /// Paint the run in a bold face.
    pub bold: bool,
    /// Paint the run in an italic face.
    pub italic: bool,
    /// Draw a line under the run.
    pub underline: bool,
    /// Draw a line through the middle of the run.
    pub strikeout: bool,
    /// Override the run's family; `None` keeps the editor's own face.
    pub font_family: Option<String>,
    /// Override the run's point size; `None` keeps the editor's own size.
    ///
    /// Integral, so `CharFormat` keeps `Eq` (a float field would have removed that impl).
    pub font_size: Option<u32>,
    /// Override the run's ink; `None` keeps the editor's text colour.
    pub color: Option<crate::core::Color>,
}

impl CharFormat {
    /// Whether this format changes anything about the painted run.
    pub fn is_plain(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.underline
            && !self.strikeout
            && self.font_family.is_none()
            && self.font_size.is_none()
            && self.color.is_none()
    }
}

/// A splice shared between an undo entry and the typing run that extends it (I-P55).
#[derive(Debug, Default)]
struct RichDelta {
    offset: usize,
    removed: String,
    inserted: String,
}

/// One rich-edit splice, recorded as a delta instead of two whole documents (I-P55).
///
/// The delta lives in an `Rc<RefCell<_>>` so a continuous typing run can extend the *same* undo
/// entry instead of pushing one entry per keystroke: the command reads the shared run when it is
/// undone, so the last key of a word is the state the single undo step reverses.
struct RichTextDeltaCommand {
    id: CommandId,
    target: Rc<RefCell<String>>,
    delta: Rc<RefCell<RichDelta>>,
}

static NEXT_RICH_COMMAND_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);

impl RichTextDeltaCommand {
    fn new(target: Rc<RefCell<String>>, delta: Rc<RefCell<RichDelta>>) -> Self {
        Self {
            id: CommandId(NEXT_RICH_COMMAND_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed)),
            target,
            delta,
        }
    }

    fn splice(&self, removed_len: usize, inserted: &str) {
        let delta = self.delta.borrow();
        let mut text = self.target.borrow_mut();
        let start = delta.offset.min(text.len());
        let end = (start + removed_len).min(text.len());
        text.replace_range(start..end, inserted);
    }
}

/// The differing middle of a before/after pair, for programmatic replacements (I-P55).
fn diff_delta(before: &str, after: &str) -> RichDelta {
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
    RichDelta {
        offset: prefix,
        removed: before[prefix..before.len() - suffix].to_string(),
        inserted: after[prefix..after.len() - suffix].to_string(),
    }
}

impl UndoCommand for RichTextDeltaCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit text".to_string(),
            timestamp_ms: 0,
            command_type: "rich_edit_text",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        let inserted = self.delta.borrow().inserted.clone();
        let removed_len = self.delta.borrow().removed.len();
        self.splice(removed_len, &inserted);
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        let removed = self.delta.borrow().removed.clone();
        let inserted_len = self.delta.borrow().inserted.len();
        self.splice(inserted_len, &removed);
        Ok(())
    }
}

/// Rich text/code editor baseline widget contract.
///
/// Holds a plain `String` plus a byte-offset selection and a set of change
/// signals. Despite the name it stores no styling or markup: it is a text
/// buffer with editor signalling, and "rich" refers to the role it fills (the
/// editing surface that richer tooling builds on) rather than to its contents.
///
/// Offsets in the selection and cursor signals are **byte** indices into the
/// UTF-8 text, not character indices, so they are not guaranteed to land on
/// character boundaries unless the writer keeps them there.
///
pub struct RichEdit {
    base: BaseWidget,
    text: String,
    selection: Option<(usize, usize)>,
    /// Whether this editor currently owns keyboard focus.
    ///
    /// # Why a text editor has to know
    ///
    /// The `KeyPress` arm ran for **every** event, so a document that had never been clicked into
    /// still consumed the keyboard: with two editors on a page, typing into one edited both, and an
    /// unfocused editor swallowed accelerators the host meant for something else. The framework does
    /// deliver `FocusGained` / `FocusLost` (see `crate::widget::runtime`), so the fact was available —
    /// this control simply had no field to record it in.
    ///
    /// Defaults to `false`, so a control that was never focused does not consume keys; the pointer
    /// path focuses on press, which is how a user reaches it.
    focused: bool,
    /// Which end of `selection` the caret is at.
    ///
    /// The range is stored as an ordered pair, so the moving end is not recoverable from it —
    /// [`Self::cursor_position`] reports `start`, which is the wrong end for a range built
    /// leftwards. A `Shift`-extension needs to know which end must stay put, so the caret's own
    /// position is tracked here and updated by every path that moves it.
    extend_caret: usize,
    read_only: bool,
    /// Emitted with the full new text on every accepted change. Carries the
    /// whole document, not a delta.
    pub text_changed: Signal1<String>,
    /// Emitted with the new selection as byte offsets `(start, end)`, or `None`
    /// when the selection is cleared.
    pub selection_changed: Signal1<Option<(usize, usize)>>,
    /// Emitted with the new flag from [`RichEdit::set_read_only`].
    pub read_only_changed: Signal1<bool>,
    /// Emitted with the new cursor byte offset whenever the text is replaced.
    /// Note the name does not match the payload: it reports an offset, not a
    /// position struct.
    pub cursor_position_changed: Signal1<usize>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// Styled runs painted over the value; empty by default.
    spans: Vec<TextSpan>,
    /// The first document line the viewport paints.
    ///
    /// Zero until the user scrolls, so a document that fits moves exactly as it did.
    first_visible_line: usize,
    /// Whether a pointer drag is extending the selection.
    dragging_selection: bool,
    /// The byte offset where the pointer drag began.
    drag_anchor: usize,
    /// Whether long logical lines wrap at the interior's width instead of being clipped.
    ///
    /// Off by default, so a document that was clipped at the control's edge keeps that layout.
    line_wrap: bool,
    /// Text painted when the document is empty.
    placeholder_text: String,
    /// The format applied to text typed at a bare caret.
    ///
    /// [`Self::apply_char_format`] stores the format here when there is no selection; the next
    /// inserted run is then stamped with it. `None` inherits the editor's own style.
    typing_format: Option<CharFormat>,
    /// Bumped on every value change (I-P56).
    revision: u64,
    /// Bumped whenever the styled runs change (I-P56).
    spans_revision: u64,
    /// The wrapped row table, keyed by the inputs that derive it (I-P56).
    rows_cache: RefCell<Option<VisualRowsCache>>,
    /// Contiguous format runs, keyed by the text and span revisions (I-P56).
    format_cache: RefCell<Option<FormatRunsCache>>,
    /// Logical line starts, keyed by the text revision (I-P56).
    line_starts_cache: RefCell<Option<(u64, Rc<Vec<usize>>)>>,
    /// The typing run the next keystroke may extend, so a word is one undo step (I-P55).
    typing_run: Option<Rc<RefCell<RichDelta>>>,
}

struct VisualRowsCache {
    revision: u64,
    spans_revision: u64,
    width: u32,
    line_wrap: bool,
    font: Font,
    rows: Rc<Vec<(usize, usize)>>,
}

struct FormatRunsCache {
    revision: u64,
    spans_revision: u64,
    runs: Rc<Vec<(usize, usize, CharFormat)>>,
}
impl RichEdit {
    /// Creates an empty rich editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "RichEdit"),
            text: String::new(),
            selection: None,
            focused: false,
            extend_caret: 0,
            read_only: false,
            text_changed: Signal1::new(),
            selection_changed: Signal1::new(),
            read_only_changed: Signal1::new(),
            cursor_position_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            spans: Vec::new(),
            first_visible_line: 0,
            dragging_selection: false,
            drag_anchor: 0,
            line_wrap: false,
            placeholder_text: String::new(),
            typing_format: None,
            revision: 0,
            spans_revision: 0,
            rows_cache: RefCell::new(None),
            format_cache: RefCell::new(None),
            line_starts_cache: RefCell::new(None),
            typing_run: None,
        }
    }
    /// Returns current editor text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Replaces editor text and resets selection/cursor to end.
    ///
    /// Ignored entirely — no change, no signals — when the editor is read-only
    /// or when `text` equals the current content. Otherwise the previous content
    /// is pushed onto the undo stack (unless an undo/redo is being replayed),
    /// the selection is cleared, and `text_changed` then `cursor_position_changed`
    /// fire, followed by a redraw request.
    ///
    /// The cursor is reported as `text.len()`, a byte offset at the end of the
    /// document.
    pub fn set_text(&mut self, text: String) {
        if self.read_only || self.text == text {
            return;
        }
        let before = self.text.clone();
        self.text = text;
        self.revision = self.revision.wrapping_add(1);
        self.clamp_spans();
        self.typing_run = None;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            let delta = Rc::new(RefCell::new(diff_delta(&before, &self.text)));
            self.undo_stack
                .push(Box::new(RichTextDeltaCommand::new(self.history_target.clone(), delta)));
        }
        self.selection = None;
        self.first_visible_line = 0;
        self.text_changed.emit(self.text.clone());
        self.cursor_position_changed.emit(self.text.len());
        self.base.request_redraw();
    }

    /// Applies one in-place splice, records it as a delta and keeps the spans aligned (I-P55/I-P57).
    ///
    /// `mergeable` marks an ordinary typing insertion: consecutive insertions at the caret's own
    /// end extend the same undo entry, so a typed word undoes as one step. The history mirror is
    /// advanced with the same splice rather than replaced by a whole-document clone.
    fn apply_splice(&mut self, at: usize, removed_len: usize, inserted: &str, mergeable: bool) {
        if self.read_only {
            return;
        }
        let at = floor_char_boundary(&self.text, at.min(self.text.len()));
        let removed_len = removed_len.min(self.text.len().saturating_sub(at));
        let removed = self.text[at..at + removed_len].to_string();
        self.text.replace_range(at..at + removed_len, inserted);
        self.revision = self.revision.wrapping_add(1);
        self.shift_spans(at, removed_len, inserted.len());
        {
            let mut target = self.history_target.borrow_mut();
            let start = at.min(target.len());
            let end = (start + removed_len).min(target.len());
            target.replace_range(start..end, inserted);
        }
        if !self.restoring_history {
            let extended = if mergeable {
                self.typing_run.as_ref().is_some_and(|run| {
                    let run = run.borrow();
                    run.removed.is_empty() && run.offset + run.inserted.len() == at
                })
            } else {
                false
            };
            if extended {
                if let Some(run) = self.typing_run.as_ref() {
                    run.borrow_mut().inserted.push_str(inserted);
                }
            } else {
                let run = Rc::new(RefCell::new(RichDelta {
                    offset: at,
                    removed,
                    inserted: inserted.to_string(),
                }));
                self.undo_stack.push(Box::new(RichTextDeltaCommand::new(
                    self.history_target.clone(),
                    run.clone(),
                )));
                self.typing_run = mergeable.then_some(run);
            }
        }
        self.text_changed.emit(self.text.clone());
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }

    /// Shifts existing spans across one splice (I-P57).
    fn shift_spans(&mut self, at: usize, removed: usize, inserted: usize) {
        if self.spans.is_empty() {
            return;
        }
        let end = at + removed;
        let map = |pos: usize| -> usize {
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

    /// The styled runs painted over the document.
    pub fn spans(&self) -> &[TextSpan] {
        &self.spans
    }

    /// Replaces the styled runs.
    ///
    /// Ranges are clamped to the current document and empty or reversed ranges are dropped. An
    /// empty list returns the control to its unstyled paint path.
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
        self.spans_revision = self.spans_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The format that applies at `offset`, taken from the last span covering it.
    pub fn format_at(&self, offset: usize) -> CharFormat {
        self.spans
            .iter()
            .rev()
            .find(|span| span.range.contains(&offset))
            .map_or(CharFormat::default(), |span| span.format.clone())
    }

    /// Whether long lines wrap at the interior's width.
    pub fn line_wrap(&self) -> bool {
        self.line_wrap
    }

    /// Enables or disables soft wrapping of long logical lines.
    pub fn set_line_wrap(&mut self, wrap: bool) {
        if self.line_wrap == wrap {
            return;
        }
        self.line_wrap = wrap;
        self.first_visible_line = 0;
        self.rows_cache.borrow_mut().take();
        self.base.request_redraw();
    }

    /// The text painted when the document is empty.
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }

    /// Sets the placeholder text.
    pub fn set_placeholder_text(&mut self, text: String) {
        if self.placeholder_text == text {
            return;
        }
        self.placeholder_text = text;
        self.base.request_redraw();
    }

    /// Applies `format` to the selection, or to text typed next when nothing is selected.
    ///
    /// The format is stored as a span over the selected range; spans are resolved last-wins, so a
    /// later call can restyle a range it previously covered. With a bare caret the format is kept
    /// as the typing format and stamped onto the next inserted run instead of changing anything
    /// already in the document.
    pub fn apply_char_format(&mut self, format: CharFormat) {
        if self.read_only {
            return;
        }
        let Some((start, end)) = self.selection.filter(|(start, end)| start != end) else {
            self.typing_format = Some(format);
            return;
        };
        let len = self.text.len();
        let (lo, hi) = if start < end { (start, end) } else { (end, start) };
        let lo = floor_char_boundary(&self.text, lo.min(len));
        let hi = floor_char_boundary(&self.text, hi.min(len));
        if lo >= hi {
            self.typing_format = Some(format);
            return;
        }
        self.spans.push(TextSpan { range: lo..hi, format });
        self.base.request_redraw();
    }

    /// The width a display row has for text, after the field's padding.
    fn content_width(&self) -> f32 {
        self.geometry().width.saturating_sub(8).max(1) as f32
    }

    /// The face a run at `offset` is painted with, after its span's attributes are applied.
    fn face_at(&self, offset: usize) -> Font {
        let format = self.format_at(offset);
        let mut face = effective_font(self.style()).clone();
        if let Some(family) = format.font_family {
            face = face.with_family(family);
        }
        if let Some(size) = format.font_size {
            face = face.with_size(size as f32);
        }
        if format.bold {
            face = face.with_bold(true);
        }
        if format.italic {
            face = face.with_italic(true);
        }
        face
    }

    /// The byte ranges of the rows the paint and hit test share.
    ///
    /// With wrapping off this is one row per logical line; with it on, a long line contributes as
    /// many rows as it takes to fit the interior width. Deriving both readers from this one table
    /// is what keeps a click on the row it was painted on.
    fn visual_rows(&self) -> Rc<Vec<(usize, usize)>> {
        let rect = self.geometry();
        let width = rect.width;
        let font = effective_font(self.style()).clone();
        if let Some(cache) = self.rows_cache.borrow().as_ref() {
            if cache.revision == self.revision
                && cache.spans_revision == self.spans_revision
                && cache.width == width
                && cache.line_wrap == self.line_wrap
                && cache.font == font
            {
                return Rc::clone(&cache.rows);
            }
        }
        let mut rows = Vec::new();
        let mut line_start = 0usize;
        for line in self.text.split('\n') {
            let line_end = line_start + line.len();
            // A trailing carriage return (from CRLF) is invisible and was never painted by
            // `.lines()`, so excluding it keeps this table's rows the same spans the old paint used.
            let content = line.strip_suffix('\r').unwrap_or(line);
            let content_end = line_start + content.len();
            if !self.line_wrap || content.is_empty() {
                rows.push((line_start, content_end));
            } else {
                let limit = self.content_width();
                let mut row_start = line_start;
                let mut width = 0.0f32;
                let mut glyph = [0u8; 4];
                for (index, ch) in content.char_indices() {
                    let face = self.face_at(line_start + index);
                    let advance =
                        estimate_text_width(ch.encode_utf8(&mut glyph), &face, 1.0) as f32;
                    if width > 0.0 && width + advance > limit {
                        rows.push((row_start, line_start + index));
                        row_start = line_start + index;
                        width = 0.0;
                    }
                    width += advance;
                }
                rows.push((row_start, content_end));
            }
            line_start = line_end + 1;
        }
        let rows = Rc::new(rows);
        *self.rows_cache.borrow_mut() = Some(VisualRowsCache {
            revision: self.revision,
            spans_revision: self.spans_revision,
            width,
            line_wrap: self.line_wrap,
            font,
            rows: Rc::clone(&rows),
        });
        rows
    }

    /// Contiguous format runs over the document, cached by both revisions (I-P56).
    ///
    /// Built from span boundaries, so the painter sees one run per format change instead of
    /// probing every character.
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
                .map_or_else(CharFormat::default, |span| span.format.clone());
            if let Some(last) = runs.last_mut() {
                if last.1 == start && last.2 == format {
                    last.1 = end;
                    continue;
                }
            }
            runs.push((start, end, format));
        }
        let runs = Rc::new(runs);
        *self.format_cache.borrow_mut() = Some(FormatRunsCache {
            revision: self.revision,
            spans_revision: self.spans_revision,
            runs: Rc::clone(&runs),
        });
        runs
    }

    /// The byte offset of every logical line start, cached per revision (I-P56).
    fn line_starts(&self) -> Rc<Vec<usize>> {
        if let Some((revision, starts)) = self.line_starts_cache.borrow().as_ref() {
            if *revision == self.revision {
                return Rc::clone(starts);
            }
        }
        let mut starts = vec![0usize];
        for (index, ch) in self.text.char_indices() {
            if ch == '\n' {
                starts.push(index + 1);
            }
        }
        let starts = Rc::new(starts);
        *self.line_starts_cache.borrow_mut() = Some((self.revision, Rc::clone(&starts)));
        starts
    }

    /// The number of rows the current wrap layout paints.
    fn display_row_count(&self) -> usize {
        if self.line_wrap {
            self.visual_rows().len()
        } else {
            self.line_count()
        }
    }

    /// The first document line the viewport paints.
    pub fn first_visible_line(&self) -> usize {
        self.first_visible_line
    }

    /// Sets the first painted line, clamped to the document.
    pub fn set_first_visible_line(&mut self, line: usize) {
        let max = self.text.lines().count().saturating_sub(1);
        let line = line.min(max);
        if self.first_visible_line == line {
            return;
        }
        self.first_visible_line = line;
        self.base.request_redraw();
    }

    /// The document's lines, as the painter and the hit test both count them.
    fn line_count(&self) -> usize {
        self.text.lines().count().max(1)
    }

    /// How many whole lines fit in the interior at the current font.
    fn visible_line_count(&self) -> usize {
        let rect = self.geometry();
        let line_height = effective_font(self.style()).effective_line_height().max(1.0) as i32;
        let usable = (rect.height as i32 - 8).max(line_height);
        (usable / line_height).max(1) as usize
    }

    /// Scrolls by `lines` document rows toward the end of the value.
    fn scroll_lines(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let max_first = self.display_row_count().saturating_sub(self.visible_line_count());
        let next =
            (self.first_visible_line as i64 + lines as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_line {
            return false;
        }
        self.first_visible_line = next;
        true
    }

    /// Scrolls the viewport the minimum amount that brings `offset`'s line into view.
    fn scroll_caret_into_view(&mut self) {
        let row = if self.line_wrap {
            let rows = self.visual_rows();
            rows.iter()
                .position(|(_, end)| *end >= self.extend_caret)
                .unwrap_or_else(|| rows.len().saturating_sub(1))
        } else {
            match self.byte_offset_to_line_col(self.extend_caret) {
                Some((line, _)) => line,
                None => return,
            }
        };
        let visible = self.visible_line_count();
        if row < self.first_visible_line {
            self.first_visible_line = row;
        } else if row >= self.first_visible_line + visible {
            self.first_visible_line = row + 1 - visible;
        }
    }

    /// Copies the selected text to the platform clipboard, returning it.
    #[cfg(not(alloc_frugal))]
    fn copy_selection_to_clipboard(&self) -> Option<String> {
        let (start, end) = self.selection?;
        if start == end {
            return None;
        }
        let (lo, hi) = if start < end { (start, end) } else { (end, start) };
        let lo = floor_char_boundary(&self.text, lo.min(self.text.len()));
        let hi = floor_char_boundary(&self.text, hi.min(self.text.len()));
        let selected = self.text[lo..hi].to_string();
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
            self.insert_committed_text(&text);
        }
    }

    /// The allocation-frugal build has no clipboard integration.
    #[cfg(alloc_frugal)]
    fn paste_from_clipboard(&mut self) {}

    /// Inserts committed text at the caret, replacing the selection when one is active
    /// (D09-INPUT-01).
    ///
    /// This is the single editing entry point for text that came from the platform rather than a
    /// single key: `Event::TextInput`/`Event::ImeCommit` route here, and the `KeyPress` printable
    /// arm calls it too, so selection replacement, the undo push, `text_changed` and
    /// `cursor_position_changed` are implemented once. Control characters are dropped because a
    /// stray one must not enter the document as a non-printing byte.
    pub fn insert_committed_text(&mut self, text: &str) {
        if self.read_only {
            return;
        }
        let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
        if filtered.is_empty() {
            return;
        }
        // The insertion point is the caret end of the range, which is where a user typing sees the
        // next character land. A non-empty selection is replaced rather than appended to.
        let caret = self.extend_caret.min(self.text.len());
        let (start, end) = match self.selection {
            Some((a, b)) => {
                let lo = a.min(b).min(self.text.len());
                let hi = a.max(b).min(self.text.len());
                (floor_char_boundary(&self.text, lo), floor_char_boundary(&self.text, hi))
            }
            None => {
                (floor_char_boundary(&self.text, caret), floor_char_boundary(&self.text, caret))
            }
        };
        // The splice is applied in place and recorded as a delta; an ordinary typed run extends
        // the previous typing step (I-P55).
        self.apply_splice(start, end - start, &filtered, true);
        let new_caret = (start + filtered.len()).min(self.text.len());
        // Stamp the run with the format a bare caret asked for. Insertion is the only moment the
        // range is known, and the splice leaves existing spans aligned.
        if let Some(format) = self.typing_format.clone() {
            if start < new_caret {
                self.spans.push(TextSpan { range: start..new_caret, format });
                self.spans_revision = self.spans_revision.wrapping_add(1);
            }
        }
        self.selection = Some((new_caret, new_caret));
        self.extend_caret = new_caret;
        self.cursor_position_changed.emit(new_caret);
        self.base.request_redraw();
    }

    /// Steps back one text change and returns `true`, or `false` when there is
    /// nothing to undo.
    ///
    /// Replaying history emits `text_changed` but does not consult the read-only
    /// flag, so an undo can alter the text of a read-only editor. No undo entry
    /// is created for the replay.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Steps forward one undone change and returns `true`, or `false` when there
    /// is nothing to redo. Signal behaviour matches [`RichEdit::undo`], including
    /// the ignored read-only flag.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns `true` if [`RichEdit::undo`] would change the text.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`RichEdit::redo`] would change the text.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        self.restoring_history = true;
        self.text = text;
        self.revision = self.revision.wrapping_add(1);
        self.clamp_spans();
        self.typing_run = None;
        let caret = self.extend_caret.min(self.text.len());
        self.selection = Some((caret, caret));
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.cursor_position_changed.emit(caret);
        // I-P57: keep the viewport on the caret after a history replay instead of jumping to the
        // document's first line.
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }
    /// Returns current selection range.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }
    /// Sets selection range.
    ///
    /// The caret is left at `end`, so a following `Shift`-movement extends from `start` — the
    /// convention every other control in the crate has, and the one a caller writing
    /// `set_selection(0, 5)` then pressing Shift+Left expects.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        if self.read_only {
            return;
        }
        self.typing_run = None;
        let start = start.min(self.text.len());
        let end = end.min(self.text.len());
        self.extend_caret = end;
        if self.selection == Some((start, end)) {
            return;
        }
        self.selection = Some((start, end));
        self.selection_changed.emit(self.selection);
        self.base.request_redraw();
    }
    /// The byte offset a point falls on, resolved against the laid-out lines.
    ///
    /// # How a point becomes an offset
    ///
    /// The line is whichever row `y` lands in, using the same `line_height` and origin the paint path
    /// uses; the within-line offset then comes from [`Self::byte_offset_at_x`], which measures with
    /// the renderer's own width model. Deriving the row from a second layout would let a click land
    /// on a different line than the one drawn there.
    pub fn byte_offset_at_point(&self, pos: crate::core::Point) -> usize {
        let rect = self.geometry();
        let padding = 4;
        let font = effective_font(self.style());
        let line_height = font.effective_line_height().max(1.0) as i32;
        let first_line_y = rect.y + padding + line_height;
        let row =
            self.first_visible_line + ((pos.y - first_line_y).max(0) / line_height.max(1)) as usize;

        // Walk the same row table the paint path uses, so a click resolves to the row that was
        // drawn under it — including across a soft-wrapped line.
        let rows = self.visual_rows();
        if let Some((start, end)) = rows.get(row).copied() {
            return start + self.byte_offset_at_x(&self.text[start..end], pos.x - rect.x);
        }
        // Below every row the value has (or an empty value): the caret belongs at the end.
        self.text.len()
    }

    /// Clears selection.
    pub fn clear_selection(&mut self) {
        self.typing_run = None;
        if self.selection.is_none() {
            return;
        }
        self.selection = None;
        self.selection_changed.emit(None);
    }

    /// Moves the caret to `target` (a byte offset), honouring the modifier keys held.
    ///
    /// # The single entry point for every keyboard movement
    ///
    /// Left, Right, Up, Down, Home and End all funnel through here, so "what Shift means" is written
    /// once (principle #101). Each of them used to carry its own `if *modifiers == 0` guard, which
    /// meant **Shift was silently ignored**: the arm did not run, and the probe showed
    /// `Shift+Right` leaving `Some((1, 1))` — the caret had moved and nothing was selected, so the
    /// key did something visible but not what it says.
    ///
    /// The two behaviours are:
    ///
    /// * **Shift** — extend: the caret moves to `target` while the anchor stays put, so the range
    ///   grows from where the gesture began.
    /// * **No modifier** — replace: the caret moves and the range collapses to zero width there,
    ///   which is this control's existing spelling for "just a caret" (see the module docs on why
    ///   `Some((n, n))` rather than `None`).
    ///
    /// # Why the anchor cannot be read back out of the range
    ///
    /// This control stores the range as an ordered pair, so which end is the *caret* is not
    /// recorded. [`Self::cursor_position`] answers `start`, which makes a live range `(0, 1)` look
    /// as though the caret were at 0 — so an implementation that derived the anchor from the caret
    /// picked the wrong end and the range collapsed on the second extension (observed: `(1, 1)` where
    /// `(0, 2)` was due). The moving end is therefore tracked in its own field
    /// (`Self::extend_caret`), which is set whenever the caret moves for any reason.
    ///
    /// `target` is clamped to the value and snapped to a character boundary, so an offset computed
    /// from a pixel can never leave the caret inside a multi-byte character.
    pub fn select_with_modifiers(&mut self, target: usize, modifiers: crate::shortcut::Modifiers) {
        let target = floor_char_boundary(&self.text, target.min(self.text.len()));
        self.typing_run = None;
        // Where the caret is **now**, before it moves. This is the anchor for an extension that has
        // no live range yet: the field is showing a bare caret, and a `Shift`-movement must grow a
        // range *from that caret*. Reading the anchor off `selection` instead would pick the new
        // target — a zero-width range carries no information about where the caret was — and the
        // first extension would select nothing (observed: `Some((1, 1))` for the first `Shift+Right`
        // on a caret at 0).
        let previous_caret = self.extend_caret.min(self.text.len());
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            let anchor = match self.selection.filter(|(s, e)| s != e) {
                // A live range: the anchor is the end the caret is not at. `extend_caret` records
                // which end that is, which `selection` alone cannot tell (it stores an ordered pair,
                // and `cursor_position()` reports `start`).
                Some((start, end)) => {
                    if self.extend_caret == start {
                        end
                    } else {
                        start
                    }
                }
                // No live range: start one from where the caret already was.
                None => previous_caret,
            };
            let (start, end) = if anchor <= target { (anchor, target) } else { (target, anchor) };
            self.selection = Some((start, end));
        } else {
            // A plain movement collapses the range onto the new position.
            self.selection = Some((target, target));
        }
        self.extend_caret = target;
        self.cursor_position_changed.emit(target);
        self.selection_changed.emit(self.selection);
        self.scroll_caret_into_view();
        self.base.request_redraw();
    }

    /// The byte offset the horizontal coordinate `x` points at on `line`.
    ///
    /// # How a pixel becomes an offset
    ///
    /// The answer is found by walking the line's own characters and measuring the prefix, using
    /// `estimate_text_width` — the same model the
    /// paint path lays the ink out with and the caret is placed by. A division by an assumed advance
    /// would be a fourth copy of the width model, which is precisely the defect the caret's own docs
    /// record ("`col * 7`, with the comment rough char width").
    ///
    /// The result is a character boundary by construction, because it is only ever returned at one.
    pub fn byte_offset_at_x(&self, line: &str, x: i32) -> usize {
        let font = effective_font(self.style());
        let padding = 4;
        let target = (x - padding).max(0) as f32;
        // One accumulated prefix instead of re-measuring the whole prefix per character, which
        // was O(line²) for a click (I-P56).
        let mut best = 0usize;
        let mut prefix = 0.0f32;
        let mut glyph = [0u8; 4];
        for (index, ch) in line.char_indices() {
            let advance = estimate_text_width(ch.encode_utf8(&mut glyph), font, 1.0) as f32;
            if target < prefix + advance / 2.0 {
                break;
            }
            prefix += advance;
            best = index + ch.len_utf8();
        }
        best
    }
    /// Returns read-only state.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        if self.read_only == read_only {
            return;
        }
        self.read_only = read_only;
        self.read_only_changed.emit(read_only);
        self.base.request_redraw();
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.selection.map_or(0, |(start, _)| start)
    }

    /// Converts a byte offset into (line_index, col_index).
    /// Returns `None` when the offset is at end of text.
    fn byte_offset_to_line_col(&self, offset: usize) -> Option<(usize, usize)> {
        if self.text.is_empty() {
            return Some((0, 0));
        }
        let offset = floor_char_boundary(&self.text, offset.min(self.text.len()));
        let starts = self.line_starts();
        // Binary search over the cached line starts instead of scanning the document (I-P56).
        let line = match starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let start = starts.get(line).copied().unwrap_or(0).min(offset);
        Some((line, self.text[start..offset].chars().count()))
    }

    /// Converts (line_index, col_index) back to a byte offset.
    fn line_col_to_byte_offset(&self, line: usize, col: usize) -> usize {
        let starts = self.line_starts();
        let Some(&start) = starts.get(line) else {
            return self.text.len();
        };
        let end =
            starts.get(line + 1).map(|next| next.saturating_sub(1)).unwrap_or(self.text.len());
        let end = end.min(self.text.len()).max(start);
        match self.text[start..end].char_indices().nth(col) {
            Some((index, _)) => start + index,
            None => end,
        }
    }
    /// Sets cursor position.
    pub fn set_cursor_position(&mut self, position: usize) {
        if self.read_only {
            return;
        }
        self.typing_run = None;
        let position = position.min(self.text.len());
        // I-P57: the caret's own end must move with a programmatic placement, or the paint and
        // the next edit keep using the old anchor.
        self.extend_caret = position;
        if self.selection == Some((position, position)) {
            return;
        }
        self.selection = Some((position, position));
        self.cursor_position_changed.emit(position);
        self.base.request_redraw();
    }
}
impl Widget for RichEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// A document asks for the width of its widest line and the height of its rows.
    ///
    /// The editor used to fall back to whatever default the trait supplied, which ignored the
    /// value entirely. Measuring with the same effective font the paint uses keeps the hint and
    /// the ink in step, and the row count follows the wrap mode so a wrapped document asks for the
    /// height it actually paints.
    fn size_hint(&self) -> Size {
        let font = effective_font(self.style());
        let width = self
            .text
            .lines()
            .map(|line| estimate_text_width(line, font, 1.0) as f32)
            .fold(0.0f32, f32::max);
        let line_height = font.effective_line_height().max(1.0);
        let height = self.display_row_count() as f32 * line_height;
        Size::new((width + 16.0).max(120.0) as u32, (height + 12.0).max(60.0) as u32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    // The declared shape is `OptionalTuple2` of `UInt`, so a present selection is a
                    // real `(start, end)` pair of offsets and a cleared one is `Null` — not a
                    // `Debug` string that a subscriber cannot read as numbers.
                    match v {
                        Some((start, end)) => CapabilityValue::Tuple(crate::compat::Vec::from([
                            CapabilityValue::UInt(*start as u64),
                            CapabilityValue::UInt(*end as u64),
                        ])),
                        None => CapabilityValue::Null,
                    }
                }))
            }
            "cursor_position_changed" => Some(EventSignalRef::mapped(
                "cursor_position_changed",
                &self.cursor_position_changed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "read_only_changed" => {
                Some(EventSignalRef::mapped("read_only_changed", &self.read_only_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `RichEdit`'s property contract.
///
/// `WidgetKind::RichEdit` is the kind the capability layer pairs with the
/// [`CodeEditor`](crate::widget::CodeEditor) control
/// (`capability::properties::code_editor_capability`), so the `RichEdit` arm of
/// the old centralised dispatch was answered by `CodeEditor`, not by this widget.
/// The trait impl therefore lives beside `CodeEditor` in its own file; this widget
/// declares that it has no properties of its own rather than claiming another
/// control's, which is the honest answer the property layer asks for.
impl WidgetProperties for RichEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "line_count" => Ok(CapabilityValue::UInt(self.text().lines().count() as u64)),
            "read_only" => Ok(CapabilityValue::Bool(self.read_only)),
            "line_wrap" => Ok(CapabilityValue::Bool(self.line_wrap)),
            "placeholder_text" => Ok(CapabilityValue::String(self.placeholder_text.clone())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "read_only" => {
                // I-109: route through the setter, so the property and method entry points emit
                // the same `read_only_changed` signal.
                self.set_read_only(expect_bool(value)?);
                Ok(())
            }
            "line_wrap" => {
                self.set_line_wrap(expect_bool(value)?);
                Ok(())
            }
            "placeholder_text" => {
                self.set_placeholder_text(expect_string(value)?);
                Ok(())
            }
            // Derived from the document body.
            "line_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "line_count",
            "read_only",
            "line_wrap",
            "placeholder_text",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `rich_edit` publishes.
    ///
    /// `set_text` assigns the document body and needs a payload, so it is answered
    /// through the property route — the capability publishes no zero-argument action.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl Draw for RichEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the page, its border, its text and its cursor
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        // The fallback name matters: the role table is keyed on **role** names, so `rich_edit`
        // (the factory name) is not in it and would classify as `Surface`, i.e. the window fill.
        // `richedit` is, and resolves to the editable interior plus the theme's foreground.
        let theme = crate::style::resolved_theme_style_shared("rich_edit")
            .or_else(|| crate::style::resolved_theme_style_shared("richedit"));
        // The window fill, read as its own lock acquisition and copied out as a value, so the
        // guard is dropped before anything else touches the theme.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let paper_from_theme =
            theme.as_ref().and_then(|t| t.background_color).unwrap_or(Color::rgb(255, 255, 255));
        // The filter is on the **resolved** value, not only on the theme's: a control absent from
        // the role table resolves its background to the window fill, and `text_edit`/`rich_edit`
        // are exactly those controls, so letting that value through would paint the document in
        // the window's own colour — invisible on screen. A caller's own colour still wins.
        let paper = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => paper_from_theme,
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The border is one step from the page toward the ink, and a read-only document is
        // recessed a further step so the two states stay distinguishable on either appearance.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| paper.blend(&ink, 0.28));
        // The selection band is a step from the page toward the accent — visible on both appearances
        // and distinct from the caret, which is the accent at full strength.
        let selection_fill = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .unwrap_or(ink)
            .blend(&paper, 0.65);

        // Draw background
        context.face_with_gradient(
            rect,
            paper,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Draw border
        context.draw_rect(rect, if self.read_only { border.blend(&paper, 0.50) } else { border });
        // Draw text content — all lines. I-P57: the row height comes from the font the ink is
        // actually drawn with, not a hardcoded 16 that disagreed with `size_hint`.
        let font = effective_font(self.style());
        let line_height = font.effective_line_height().max(1.0) as i32;
        let padding = 2i32;
        // I-P57: the caret is the *moving* end of the range, not the range's lower bound.
        let cursor_before = self.extend_caret.min(self.text.len());

        // The selection highlight is drawn as a background band **before** the ink, so the text
        // stays legible on top of it.
        //
        // # The gap this closes (BLUE24 §12 U-4)
        //
        // The control has published `selection()` / `set_selection()` / `clear_selection()` since it
        // was written, and drew **nothing** for them: a caller could select a range and the only
        // visible feedback was a caret at one end. A selection model with no rendering is the
        // "mechanism built, port not opened" shape this whole plan is about.
        //
        // The band's two edges are measured with the same `estimate_text_width` the caret uses, so
        // highlighting a range cancels exactly to the caret at either end. Computing them `col * 7`
        // apart would have reproduced the caret defect twice more.
        //
        // The rows come from `visual_rows`, the same table the hit test reads, so a soft-wrapped
        // line is highlighted and clicked on the row it was painted on.
        let selection = self.selection.filter(|(start, end)| start != end);
        let rows = self.visual_rows();
        // I-P57: a long unwrapped row must not paint over the border or its neighbours.
        context.push_clip(
            rect.x + padding,
            rect.y + padding,
            rect.width.saturating_sub(padding as u32 * 2),
            rect.height.saturating_sub(padding as u32 * 2),
        );
        // An empty document still shows a caret and its placeholder.
        if self.text.is_empty() && !self.placeholder_text.is_empty() {
            context.draw_text(
                crate::core::Point::new(rect.x + padding, rect.y + padding + line_height),
                &self.placeholder_text,
                font,
                ink.blend(&paper, 0.45),
                HorizontalAlignment::Left,
            );
        }
        for (row_idx, (row_start, row_end)) in rows.iter().copied().enumerate() {
            // The viewport starts at `first_visible_line`; rows above it are skipped without
            // consuming a row of the paint grid, which is what makes the scroll offset real.
            if row_idx < self.first_visible_line {
                continue;
            }
            let line_y =
                rect.y + padding + line_height * ((row_idx - self.first_visible_line) as i32 + 1);
            if line_y > rect.y + rect.height as i32 {
                break;
            }
            let row_text = &self.text[row_start..row_end];
            if let Some((start, end)) = selection {
                // Clamp the selection to this row, so a range spanning several rows highlights
                // each of them only over its own part. The offsets are byte indices and
                // `set_selection` only clamps them to the value's length, so each is floored to a
                // character boundary before slicing (a mid-character slice would panic).
                let from = (start.max(row_start).min(row_end) - row_start).min(row_text.len());
                let to = (end.max(row_start).min(row_end) - row_start).min(row_text.len());
                let from = floor_char_boundary(row_text, from);
                let to = floor_char_boundary(row_text, to);
                if from < to {
                    let left =
                        rect.x + padding + estimate_text_width(&row_text[..from], font, 1.0) as i32;
                    let width = estimate_text_width(&row_text[from..to], font, 1.0);
                    context.fill_rect(
                        crate::core::Rect::new(
                            left,
                            line_y - line_height + 2,
                            width,
                            line_height as u32,
                        ),
                        selection_fill,
                    );
                }
            }
            // Draw the row's text
            if self.spans.is_empty() {
                context.draw_text(
                    crate::core::Point::new(rect.x + padding, line_y),
                    row_text,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            } else {
                self.draw_rich_line(
                    context,
                    row_text,
                    row_start,
                    rect.x + padding,
                    line_y,
                    font,
                    ink,
                );
            }
            // Draw the caret on the row that owns it. At a soft-wrap boundary the boundary offset
            // belongs to the **previous** row, so the character the caret follows stays on screen;
            // a logical line's own start is still included (the newline separates the offsets).
            let wrapped_continuation = row_idx > 0 && rows[row_idx - 1].1 == row_start;
            if !self.read_only
                && cursor_before >= row_start
                && cursor_before <= row_end
                && !(cursor_before == row_start && wrapped_continuation)
            {
                // The caret sits where the text **actually** ends, measured with the renderer's
                // own advance model, rather than at `col * 7`.
                let prefix_end = floor_char_boundary(&self.text, cursor_before);
                let prefix = &self.text[row_start..prefix_end.max(row_start)];
                let advance = estimate_text_width(prefix, font, 1.0) as i32;
                let cursor_x = rect.x + padding + advance;
                // The caret is the selection indicator, so it carries the accent rather than a
                // fixed black the user could not find on a dark page.
                let caret =
                    crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(ink);
                context.draw_line(
                    crate::core::Point::new(cursor_x, line_y - line_height + 2),
                    crate::core::Point::new(cursor_x, line_y + 2),
                    caret,
                );
            }
        }
        context.pop_clip();
    }
}

impl RichEdit {
    /// Paints one line as a sequence of formatted runs.
    ///
    /// Each run is measured with its own face, so a bold run advances further than the same text
    /// in the regular face. The unstyled path in [`Draw`] is untouched, which is what keeps a
    /// document without spans pixel-identical to its pre-span rendering.
    fn draw_rich_line(
        &self,
        context: &mut RenderContext,
        line: &str,
        line_start: usize,
        origin_x: i32,
        baseline_y: i32,
        font: &crate::core::Font,
        ink: crate::core::Color,
    ) {
        // Slice the cached document-wide run table to this line (I-P56) instead of calling
        // `format_at` — a reverse span scan — for every character painted.
        let runs = self.format_runs();
        let line_end = line_start + line.len();
        let first = runs.partition_point(|(_, end, _)| *end <= line_start);
        let mut pieces: Vec<(usize, usize, CharFormat)> = Vec::new();
        for (start, end, format) in runs.iter().skip(first) {
            if *start >= line_end {
                break;
            }
            let start = (*start).max(line_start) - line_start;
            let end = (*end).min(line_end) - line_start;
            if start < end {
                pieces.push((start, end, format.clone()));
            }
        }

        let mut pen_x = origin_x;
        for (start, end, format) in pieces {
            if start >= end {
                continue;
            }
            let mut face = font.clone();
            if let Some(family) = format.font_family.clone() {
                face = face.with_family(family);
            }
            if let Some(size) = format.font_size {
                face = face.with_size(size as f32);
            }
            if format.bold {
                face = face.with_bold(true);
            }
            if format.italic {
                face = face.with_italic(true);
            }
            let run = &line[start..end];
            let run_ink = format.color.unwrap_or(ink);
            context.draw_text(
                crate::core::Point::new(pen_x, baseline_y),
                run,
                &face,
                run_ink,
                HorizontalAlignment::Left,
            );
            let advance = estimate_text_width(run, &face, 1.0) as i32;
            if format.underline {
                let underline_y = baseline_y + 1;
                context.draw_line(
                    crate::core::Point::new(pen_x, underline_y),
                    crate::core::Point::new(pen_x + advance, underline_y),
                    run_ink,
                );
            }
            if format.strikeout {
                // Through the middle of the glyph box rather than at the baseline, so the line
                // reads as a strike rather than a second underline.
                let strike_y = baseline_y - face.effective_line_height() as i32 / 2;
                context.draw_line(
                    crate::core::Point::new(pen_x, strike_y),
                    crate::core::Point::new(pen_x + advance, strike_y),
                    run_ink,
                );
            }
            pen_x += advance;
        }
    }
}

impl crate::event::EventHandler for RichEdit {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let crate::event::Event::KeyPress { key: 90, modifiers: 2 } = event {
            let _ = self.undo();
            return;
        }
        if let crate::event::Event::KeyPress { key: 89, modifiers: 2 } = event {
            let _ = self.redo();
            return;
        }
        // Copying is the one edit gesture a read-only document still supports.
        if let crate::event::Event::KeyPress { key: 67, modifiers } = event {
            if crate::shortcut::Modifiers::from_event_bits(*modifiers)
                .contains(crate::shortcut::Modifiers::PRIMARY)
            {
                let _ = self.copy_selection_to_clipboard();
                return;
            }
        }
        if self.read_only {
            return;
        }
        match event {
            crate::event::Event::FocusGained { .. } => {
                self.focused = true;
                self.base.request_redraw();
            }
            crate::event::Event::FocusLost => {
                self.focused = false;
                self.base.request_redraw();
            }
            crate::event::Event::MousePress { pos, button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(true);
                // A press focuses the control, which is how the user reaches it with the pointer.
                self.focused = true;
                // A press places the caret where the user aimed. It used to only set the pressed
                // flag, so clicking into the document left the caret wherever it was and there was
                // no pointer selection at all — the probe read `Some((0, 0))` after a press in the
                // middle of the value.
                let index = self.byte_offset_at_point(*pos);
                self.set_selection(index, index);
                self.drag_anchor = index;
                self.dragging_selection = true;
            }
            // Moving with the button down extends the selection the press began. A bare hover
            // still does nothing, because `dragging_selection` is only set by a press.
            crate::event::Event::MouseMove { pos } => {
                if self.dragging_selection {
                    let index = self.byte_offset_at_point(*pos);
                    let (start, end) = if self.drag_anchor <= index {
                        (self.drag_anchor, index)
                    } else {
                        (index, self.drag_anchor)
                    };
                    self.set_selection(start, end);
                    // The caret is the moving end, whichever end of the ordered pair that is.
                    self.extend_caret = index;
                }
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
                self.dragging_selection = false;
            }
            // Wheel: scroll the viewport by the shared three-line desktop notch.
            crate::event::Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                if self.scroll_lines(-scroll.lines_y) {
                    self.base.request_redraw();
                }
            }
            // Platform-committed text (D09-INPUT-01). The desktop backends deliver a printable
            // character or an IME commit as `TextInput`/`ImeCommit`, so this is the branch that
            // actually receives typing; it funnels into `insert_committed_text`, the same entry
            // point the `KeyPress` printable arm below uses. `set_text`/the helper reject read-only,
            // and an unfocused editor ignores it exactly as the key path does.
            crate::event::Event::TextInput { text } | crate::event::Event::ImeCommit { text } => {
                if self.focused {
                    self.insert_committed_text(text);
                }
            }
            crate::event::Event::KeyPress { key, modifiers } => {
                // An unfocused editor owns no keys: otherwise a second editor on the page is edited
                // by the same keystroke, and a control the user never reached swallows the host's
                // accelerators. The framework delivers the focus pair, so this is a field to check
                // rather than a condition to guess.
                if !self.focused {
                    return;
                }
                // The **caret** end of the range, not `selection.start`. The two differ as soon as
                // a range is built right-to-left, and every movement below is relative to where the
                // caret is — reading `start` made `Shift+Right` extend in a loop, because `start`
                // stayed pinned at the anchor while the caret advanced.
                //
                // `text.len()` and `0` are clamped in, because `extend_caret` is a stored offset and
                // a `set_text` that shortened the value could have left it past the end.
                let cursor = self.extend_caret.min(self.text.len());
                // The event carries the framework's wire bitmask; translate it once, here.
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                let primary = mods.contains(crate::shortcut::Modifiers::PRIMARY);
                match *key {
                    8 if cursor > 0 => {
                        // Backspace — delete char before cursor, recorded as one splice (I-P55).
                        let boundary = floor_char_boundary(&self.text, cursor - 1);
                        let removed = cursor - boundary;
                        self.apply_splice(boundary, removed, "", false);
                        let new_cursor = boundary;
                        self.selection = Some((new_cursor, new_cursor));
                        self.extend_caret = new_cursor;
                        self.cursor_position_changed.emit(new_cursor);
                    }
                    127 if cursor < self.text.len() => {
                        // Delete — remove the whole *character* after the cursor.
                        //
                        // `floor_char_boundary(cursor + 1)` floors **down**, so for a multi-byte
                        // character it returns `cursor` itself; the old `if end == cursor {
                        // cursor + 1 }` fallback then invented a mid-character bound and `drain`
                        // panicked on `"é"`. The next boundary is found by taking the *end* of the
                        // first character after the caret, which is total for every value.
                        let end = self.text[cursor..]
                            .chars()
                            .next()
                            .map(|ch| cursor + ch.len_utf8())
                            .unwrap_or(self.text.len());
                        self.apply_splice(cursor, end - cursor, "", false);
                        self.selection = Some((cursor, cursor));
                        self.extend_caret = cursor;
                        self.cursor_position_changed.emit(cursor);
                    }
                    13 => {
                        // Enter — insert a newline as its own undo step.
                        self.apply_splice(cursor, 0, "\n", false);
                        let new_cursor = (cursor + 1).min(self.text.len());
                        // A typing format asked for at a bare caret applies to the newline too.
                        if let Some(format) = self.typing_format.clone() {
                            self.spans.push(TextSpan { range: cursor..new_cursor, format });
                            self.spans_revision = self.spans_revision.wrapping_add(1);
                        }
                        self.selection = Some((new_cursor, new_cursor));
                        self.extend_caret = new_cursor;
                        self.cursor_position_changed.emit(new_cursor);
                    }
                    37 if cursor > 0 => {
                        // Left arrow — one character back, extending under Shift.
                        let boundary = floor_char_boundary(&self.text, cursor - 1);
                        self.select_with_modifiers(boundary, mods);
                    }
                    39 if cursor < self.text.len() => {
                        // Right arrow — one character forward.
                        //
                        // `floor_char_boundary(cursor + 1)` floors down, so on a multi-byte
                        // character it returned the caret's own position and the key did nothing —
                        // the caret was stuck at the start of every `é`/han character. Taking the
                        // first character's length advances past it, matching the Left arm above
                        // and the Backspace/Delete arms, which are all character steps.
                        let next = self.text[cursor..]
                            .chars()
                            .next()
                            .map(|ch| cursor + ch.len_utf8())
                            .unwrap_or(self.text.len());
                        self.select_with_modifiers(next, mods);
                    }
                    36 => {
                        // Home — beginning of the current line.
                        let new_cursor = match self.byte_offset_to_line_col(cursor) {
                            Some((line, _)) => self.line_col_to_byte_offset(line, 0),
                            None => 0,
                        };
                        self.select_with_modifiers(new_cursor, mods);
                    }
                    35 => {
                        // End — end of the current line.
                        let new_cursor = match self.byte_offset_to_line_col(cursor) {
                            Some((line, _)) => {
                                let mut current_line = 0usize;
                                let mut line_end = self.text.len();
                                for (i, ch) in self.text.char_indices() {
                                    if current_line == line && ch == '\n' {
                                        line_end = i;
                                        break;
                                    }
                                    if ch == '\n' {
                                        current_line += 1;
                                    }
                                }
                                line_end
                            }
                            None => self.text.len(),
                        };
                        self.select_with_modifiers(new_cursor, mods);
                    }
                    38 => {
                        // Up — one line, keeping the column.
                        if let Some((line, col)) = self.byte_offset_to_line_col(cursor) {
                            if line > 0 {
                                let new_cursor = self.line_col_to_byte_offset(line - 1, col);
                                self.select_with_modifiers(new_cursor, mods);
                            } else if !shift {
                                // No line above: a plain press still collapses the range.
                                self.select_with_modifiers(cursor, mods);
                            }
                        }
                    }
                    40 => {
                        // Down — see the Up arm.
                        if let Some((line, col)) = self.byte_offset_to_line_col(cursor) {
                            // Only move when a later line exists; otherwise the cached line table
                            // would send the caret to the document end.
                            if line + 1 < self.line_starts().len() {
                                let new_cursor = self.line_col_to_byte_offset(line + 1, col);
                                self.select_with_modifiers(new_cursor, mods);
                            } else if !shift {
                                self.select_with_modifiers(cursor, mods);
                            }
                        }
                    }
                    65 if primary => {
                        // Primary+A: select the whole value.
                        self.set_selection(0, self.text.len());
                    }
                    88 if primary => {
                        // Primary+X: copy, then remove the selection as one splice.
                        let _ = self.copy_selection_to_clipboard();
                        if let Some((start, end)) = self.selection.filter(|(s, e)| s != e) {
                            let (lo, hi) = if start < end { (start, end) } else { (end, start) };
                            let (lo, hi) = (
                                floor_char_boundary(&self.text, lo.min(self.text.len())),
                                floor_char_boundary(&self.text, hi.min(self.text.len())),
                            );
                            self.apply_splice(lo, hi - lo, "", false);
                            self.selection = Some((lo, lo));
                            self.extend_caret = lo;
                            self.cursor_position_changed.emit(lo);
                        }
                    }
                    86 if primary => {
                        // Primary+V: paste at the caret.
                        self.paste_from_clipboard();
                    }
                    _ if *key >= 32 && *key <= 126 => {
                        // Printable ASCII — insert at the caret through the shared entry point, so
                        // the keyboard and the platform's committed-text path cannot drift.
                        let c = char::from_u32(*key).unwrap_or(' ');
                        let c = if *modifiers & 0x02 != 0 { c.to_ascii_uppercase() } else { c };
                        self.insert_committed_text(&c.to_string());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::event::EventHandler;

    #[test]
    fn richedit_creation_defaults() {
        let re = RichEdit::new(Rect::new(0, 0, 400, 300));
        assert!(re.text().is_empty());
        assert!(re.selection().is_none());
        assert!(!re.is_read_only());
        assert_eq!(re.cursor_position(), 0);
    }

    /// The caret sits at the measured end of the prefix, not at `col * 7`.
    ///
    /// # The defect this pins (BLUE24 §12 U-4's prerequisite)
    ///
    /// The caret was drawn at `rect.x + padding + col * 7`, with the comment "rough char width".
    /// Seven pixels is an average for one proportional face at one size, so on a real face the caret
    /// drifts off the character it should sit on — and on CJK, roughly doubles per character. The
    /// test asserts the **vertical line the renderer emits** against the shared measurement, so the
    /// caret and the ink cannot disagree.
    #[test]
    fn the_caret_is_placed_by_measurement_not_by_a_fixed_advance() {
        use crate::widget::metrics::estimate_text_width;
        use crate::widget::svg::render_widget_to_svg;

        let bounds = Rect::new(0, 0, 400, 300);
        let font = crate::core::Font::default();
        // A line whose characters have very different advances: `ill` is narrow, `www` is wide. A
        // fixed 7 px per character cannot be right for both.
        let text = "illwww";
        let mut re = RichEdit::new(bounds);
        re.set_text(text.to_string());
        // Put the caret after `ill` (column 3).
        re.set_cursor_position(3);
        let svg = render_widget_to_svg(&mut re, bounds);

        // The caret is a vertical `<line>`: x1 == x2. Take the last one (the caret is drawn after
        // the text).
        let caret_x = svg
            .lines()
            .filter_map(|line| {
                let rest = line.split("<line ").nth(1)?;
                let x1: i32 = rest.split("x1=\"").nth(1)?.split('"').next()?.parse().ok()?;
                let x2: i32 = rest.split("x2=\"").nth(1)?.split('"').next()?.parse().ok()?;
                (x1 == x2).then_some(x1)
            })
            .next_back()
            .expect("the caret is drawn as a vertical line");

        // The crate's padding for this control, and the measured advance of `ill`.
        let expected = 2 + estimate_text_width("ill", &font, 1.0) as i32;
        assert_eq!(
            caret_x, expected,
            "the caret must be at the measured end of `ill` ({expected}), not at 3 * 7 = 21"
        );
        assert_ne!(caret_x, 2 + 3 * 7, "the fixed-advance placement is the defect");
    }

    /// A selected range is actually painted, and its band ends where the caret would be.
    ///
    /// # The gap this pins (BLUE24 §12 U-4)
    ///
    /// The control published `selection()` / `set_selection()` / `clear_selection()` and drew
    /// **nothing** for them — a selection model with no rendering. This asserts a filled rect appears
    /// whose left edge is the measured start of the range and whose width is the measured length of
    /// it, so highlighting a range cancels to the caret at either end.
    #[test]
    fn a_selection_range_is_painted_and_measured() {
        use crate::widget::metrics::estimate_text_width;
        use crate::widget::svg::render_widget_to_svg;

        let bounds = Rect::new(0, 0, 400, 300);
        let font = crate::core::Font::default();
        let mut re = RichEdit::new(bounds);
        re.set_text("abcdef".to_string());
        re.set_selection(1, 4); // `bcd`
        let svg = render_widget_to_svg(&mut re, bounds);

        let expected_left = 2 + estimate_text_width("a", &font, 1.0) as i32;
        let expected_width = estimate_text_width("bcd", &font, 1.0);
        // The band is a filled rect at the selection's x, with the selection's width.
        let painted = svg.lines().any(|line| {
            if !line.contains("<rect ") || line.contains("fill=\"none\"") {
                return false;
            }
            let attr = |name: &str| -> Option<i32> {
                line.split(&format!("{name}=\"")).nth(1)?.split('"').next()?.parse().ok()
            };
            attr("x") == Some(expected_left) && attr("width") == Some(expected_width as i32)
        });
        assert!(
            painted,
            "a selection must be painted as a rect at x={expected_left} width={expected_width} \
             (the model existed, the rendering did not)"
        );

        // And an empty selection paints no band, so the highlight is tied to the range rather than
        // to the control always drawing one.
        re.clear_selection();
        let svg = render_widget_to_svg(&mut re, bounds);
        let still_painted = svg.lines().any(|line| {
            line.contains("<rect ")
                && !line.contains("fill=\"none\"")
                && line
                    .split("x=\"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .and_then(|x| x.parse::<i32>().ok())
                    == Some(expected_left)
        });
        assert!(!still_painted, "with no selection there is no band");
    }

    #[test]
    fn richedit_set_text() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("Hello RichEdit".to_string());
        assert_eq!(re.text(), "Hello RichEdit");
    }

    #[test]
    fn richedit_undo_redo_restores_text() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("one".to_string());
        re.set_text("two".to_string());
        assert!(re.can_undo());
        assert!(re.undo());
        assert_eq!(re.text(), "one");
        assert!(re.can_redo());
        assert!(re.redo());
        assert_eq!(re.text(), "two");
    }

    #[test]
    fn richedit_control_z_and_control_y_drive_history() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("before".to_string());
        re.set_text("after".to_string());
        re.handle_event(&crate::event::Event::key_press(90, 2));
        assert_eq!(re.text(), "before");
        re.handle_event(&crate::event::Event::key_press(89, 2));
        assert_eq!(re.text(), "after");
    }

    #[test]
    fn richedit_set_text_read_only() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_read_only(true);
        re.set_text("Should not change".to_string());
        assert!(re.text().is_empty());
    }

    #[test]
    fn richedit_read_only() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        assert!(!re.is_read_only());
        re.set_read_only(true);
        assert!(re.is_read_only());
        re.set_read_only(false);
        assert!(!re.is_read_only());
    }

    #[test]
    fn richedit_set_selection() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("Hello World".to_string());
        re.set_selection(0, 5);
        assert_eq!(re.selection(), Some((0, 5)));
    }

    #[test]
    fn richedit_clear_selection() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("Hello World".to_string());
        re.set_selection(0, 5);
        re.clear_selection();
        assert!(re.selection().is_none());
    }

    #[test]
    fn richedit_set_cursor_position() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("Hello".to_string());
        re.set_cursor_position(3);
        assert_eq!(re.cursor_position(), 3);
        re.set_cursor_position(100); // clamps
        assert_eq!(re.cursor_position(), 5);
    }

    #[test]
    fn richedit_geometry_delegation() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_geometry(Rect::new(10, 10, 500, 400));
        assert_eq!(re.geometry(), Rect::new(10, 10, 500, 400));
    }

    #[test]
    fn richedit_visibility() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        assert!(re.is_visible());
        re.hide();
        assert!(!re.is_visible());
        re.show();
        assert!(re.is_visible());
    }

    #[test]
    fn richedit_enabled() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        assert!(re.is_enabled());
        re.set_enabled(false);
        assert!(!re.is_enabled());
        re.set_enabled(true);
        assert!(re.is_enabled());
    }

    #[test]
    fn richedit_id_kind() {
        let re_a = RichEdit::new(Rect::new(0, 0, 100, 100));
        let re_b = RichEdit::new(Rect::new(0, 0, 100, 100));
        assert_ne!(re_a.id(), re_b.id());
        assert_eq!(re_a.kind(), WidgetKind::RichEdit);
        assert_eq!(re_b.kind(), WidgetKind::RichEdit);
    }

    #[test]
    fn richedit_signal_accessors() {
        let re = RichEdit::new(Rect::new(0, 0, 100, 100));
        let _ = &re.text_changed;
        let _ = &re.selection_changed;
        let _ = &re.read_only_changed;
        let _ = &re.cursor_position_changed;
    }

    /// Delete removes the whole character after the caret, whatever its byte width.
    ///
    /// `floor_char_boundary(cursor + 1)` floors **down**, so for a multi-byte character it
    /// returned the caret itself; the old "advance at least one" fallback then produced a
    /// mid-character bound and `drain` panicked. On `"é"` this took the frame down.
    #[test]
    fn delete_removes_a_whole_multibyte_character() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 100)));
        re.set_text("éa".to_string());
        re.set_selection(0, 0);
        re.handle_event(&crate::event::Event::key_press(127, 0)); // Delete
        assert_eq!(re.text(), "a", "the whole 'é' goes, not one byte of it");
    }

    /// The Right arrow advances past a multi-byte character instead of sticking on it.
    ///
    /// The same floor-down arithmetic returned the caret's own position, so the key was a no-op
    /// at the start of every `é`/han character — the caret could never leave it.
    #[test]
    fn the_right_arrow_moves_past_a_multibyte_character() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 100)));
        re.set_text("éa".to_string());
        re.set_selection(0, 0);
        let moved = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
        re.cursor_position_changed.connect({
            let flag = std::sync::Arc::clone(&moved);
            move |position| flag.store(*position, std::sync::atomic::Ordering::SeqCst)
        });
        re.handle_event(&crate::event::Event::key_press(39, 0)); // Right
        assert_eq!(
            moved.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "the caret advances by one character (two bytes for 'é')"
        );
        // And back again, one character at a time.
        re.handle_event(&crate::event::Event::key_press(37, 0)); // Left
        assert_eq!(re.selection(), Some((0, 0)));
    }

    // ─── Shift-extend and pointer placement ───

    /// Focuses an editor, because the keyboard is gated on it.
    ///
    /// Stated as one helper rather than inline at each test: a user reaches a text control by clicking
    /// it, and a fixture that skipped that would be exercising a state no user can be in. The gate
    /// itself is covered by its own test, so this is setup rather than the behaviour under test.
    fn focused(mut re: RichEdit) -> RichEdit {
        re.handle_event(&crate::event::Event::FocusGained {
            reason: crate::event::FocusReason::Programmatic,
        });
        re
    }

    /// The framework's Shift bit on an event mask.
    const SHIFT_BIT: u32 = 0b0001;

    /// A Shift-arrow **extends** the selection instead of being ignored.
    ///
    /// # The defect this pins
    ///
    /// Every movement arm carried its own `if *modifiers == 0` guard, so a `Shift` press did not run
    /// the arm at all. The probe showed what that produced:
    ///
    /// ```text
    /// Shift+Right on "hello world" with the caret at 0  ->  selection = Some((1, 1))
    /// ```
    ///
    /// The caret had moved and the range was zero width, so the key did something visible without
    /// doing what it says. A selection the user cannot grow is a selection model that is present but
    /// unreachable from the keyboard.
    #[test]
    fn a_shift_arrow_extends_instead_of_being_ignored() {
        use crate::event::Event;
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("hello world".to_string());
        re.set_selection(0, 0);

        re.handle_event(&Event::key_press(39, SHIFT_BIT));
        assert_eq!(re.selection(), Some((0, 1)), "Shift+Right selects one character");
        assert_eq!(re.cursor_position(), 0, "and the anchor stays at the start");

        re.handle_event(&Event::key_press(39, SHIFT_BIT));
        assert_eq!(re.selection(), Some((0, 2)), "the second extends the same range");

        // A plain arrow collapses the range onto its new position.
        re.handle_event(&Event::key_press(39, 0));
        let (start, end) = re.selection().expect("a plain move leaves a caret, not `None`");
        assert_eq!(start, end, "a plain arrow is not an extend");
    }

    /// Extending in the other direction moves the anchor to the far end, so the range stays ordered.
    #[test]
    fn extending_leftwards_keeps_the_range_ordered() {
        use crate::event::Event;
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("hello world".to_string());
        re.set_selection(5, 5);

        re.handle_event(&Event::key_press(37, SHIFT_BIT)); // Shift+Left
        assert_eq!(re.selection(), Some((4, 5)), "the range is ordered, and the caret end is 4");
        re.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(re.selection(), Some((3, 5)), "and it grows from the same anchor");
    }

    /// Primary+A selects the whole value.
    #[test]
    fn primary_a_selects_the_whole_value() {
        use crate::event::Event;
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("hello".to_string());
        re.set_selection(2, 2);
        re.handle_event(&Event::key_press(65, 0b1000));
        assert_eq!(re.selection(), Some((0, 5)));
    }

    /// A press puts the caret where the pointer landed.
    ///
    /// # The defect this pins
    ///
    /// The `MousePress` arm set only the pressed flag, so a click into the document left the caret
    /// where it was. The probe read `Some((0, 0))` after pressing in the middle of the value, which
    /// means a user could not put the caret anywhere with the pointer — every edit went to whatever
    /// position the last keystroke had left behind.
    #[test]
    fn a_press_places_the_caret_where_the_pointer_landed() {
        use crate::event::Event;
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("hello world".to_string());
        re.set_selection(0, 0);

        // A point well inside the value resolves to a real offset, not to 0.
        let index = re.byte_offset_at_point(crate::core::Point::new(30, 20));
        assert!(index > 0, "a point 30 px in must resolve past the first character, got {index}");

        re.handle_event(&Event::mouse_press(30, 20, 1));
        let (start, end) = re.selection().expect("a press leaves a caret");
        assert_eq!(start, end, "a press places a caret, not a range");
        assert_eq!(start, index, "and it is the offset the map computed");
    }

    /// The pixel-to-offset map is monotonic and total: moving right never moves the caret left, and
    /// no x resolves inside a multi-byte character.
    #[test]
    fn the_pixel_to_offset_map_is_monotonic_and_total() {
        let re = {
            let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
            re.set_text("héllo 中文 world".to_string());
            re
        };
        let line = re.text().to_string();
        let mut previous = 0usize;
        for x in 0..240 {
            let index = re.byte_offset_at_x(&line, x);
            assert!(index <= line.len(), "the offset is inside the value at x={x}");
            assert!(
                line.is_char_boundary(index),
                "x={x} resolved inside a character: {index} in {line:?}"
            );
            assert!(index >= previous, "x={x} moved the caret backwards ({previous} -> {index})");
            previous = index;
        }
        assert_eq!(previous, line.len(), "the far end of the line reaches the end of the value");
    }

    /// A click on a later line resolves to that line, not to the first.
    #[test]
    fn a_press_on_a_later_line_resolves_to_that_line() {
        use crate::event::Event;
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("abc\ndef".to_string());
        let second_line = re.byte_offset_at_point(crate::core::Point::new(5, 40));
        assert!(second_line >= 4, "a point below the first line is in `def`, got {second_line}");

        re.handle_event(&Event::mouse_press(5, 40, 1));
        let (start, _) = re.selection().expect("a press leaves a caret");
        assert_eq!(start, second_line);
    }

    // ─── Focus ───

    /// An unfocused editor ignores the keyboard, and a focused one accepts it.
    ///
    /// # The defect this pins
    ///
    /// The `KeyPress` arm ran for every event, so an editor nobody had clicked into still consumed the
    /// keyboard — with two editors on a page, one keystroke edited both.
    #[test]
    fn an_unfocused_editor_ignores_the_keyboard() {
        let mut re = RichEdit::new(Rect::new(0, 0, 300, 200));
        re.handle_event(&crate::event::Event::key_press(97, 0)); // 'a'
        assert!(re.text().is_empty(), "an unfocused editor must not take the keystroke");

        re.handle_event(&crate::event::Event::FocusGained {
            reason: crate::event::FocusReason::Programmatic,
        });
        re.handle_event(&crate::event::Event::key_press(97, 0));
        assert_eq!(re.text(), "a", "once focused it does");

        re.handle_event(&crate::event::Event::FocusLost);
        re.handle_event(&crate::event::Event::key_press(98, 0)); // 'b'
        assert_eq!(re.text(), "a", "and losing focus takes the keyboard away again");
    }

    /// A press focuses the editor, which is how a user reaches it with the pointer.
    #[test]
    fn a_press_focuses_the_editor() {
        let mut re = RichEdit::new(Rect::new(0, 0, 300, 200));
        re.handle_event(&crate::event::Event::mouse_press(6, 6, 1));
        re.handle_event(&crate::event::Event::key_press(97, 0)); // 'a'
        assert_eq!(re.text(), "a", "the press made the editor own the keyboard");
    }

    // ─── D09-INPUT-01: platform-committed text reaches the document ───

    /// A `TextInput` from the platform must enter the document.
    ///
    /// # The defect this pins (D09-INPUT-01)
    ///
    /// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
    /// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
    /// test feeds `TextInput` to prove that path itself is wired.
    #[test]
    fn text_input_enters_the_document() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 200)));
        re.set_selection(0, 0);
        re.handle_event(&crate::event::Event::TextInput { text: "héllo".to_string() });
        assert_eq!(re.text(), "héllo");
    }

    /// An IME commit enters the document the same way.
    #[test]
    fn ime_commit_enters_the_document() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 200)));
        re.set_selection(0, 0);
        re.handle_event(&crate::event::Event::ime_commit("你好"));
        assert_eq!(re.text(), "你好");
    }

    /// A `TextInput` replaces a selection rather than appending to it.
    #[test]
    fn text_input_replaces_the_selection() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 400, 300)));
        re.set_text("hello world".to_string());
        re.set_selection(0, 5); // `hello`
        re.handle_event(&crate::event::Event::TextInput { text: "Z".to_string() });
        assert_eq!(re.text(), "Z world", "the committed text replaced the selected range");
    }

    /// A read-only editor ignores committed text, just as it ignores a printable key.
    #[test]
    fn read_only_ignores_text_input() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 200)));
        re.set_text("locked".to_string());
        re.set_read_only(true);
        re.handle_event(&crate::event::Event::TextInput { text: "X".to_string() });
        assert_eq!(re.text(), "locked");
    }

    /// An unfocused editor ignores committed text, matching the key gate.
    #[test]
    fn unfocused_ignores_text_input() {
        let mut re = RichEdit::new(Rect::new(0, 0, 300, 200));
        re.handle_event(&crate::event::Event::TextInput { text: "X".to_string() });
        assert!(re.text().is_empty());
    }

    /// Committed text emits `text_changed` with the new document.
    #[test]
    fn text_input_emits_text_changed() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 200)));
        re.set_selection(0, 0);
        let last = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        re.text_changed.connect({
            let last = std::sync::Arc::clone(&last);
            move |text| *last.lock().unwrap() = text.to_string()
        });
        re.handle_event(&crate::event::Event::TextInput { text: "abc".to_string() });
        assert_eq!(*last.lock().unwrap(), "abc");
    }

    /// Styled spans are addressable and change the painted document.
    #[test]
    fn spans_format_the_named_run() {
        use crate::widget::svg::render_to_svg;
        let mut re = RichEdit::new(Rect::new(0, 0, 300, 200));
        re.set_text("bold and plain".to_string());
        let plain = render_to_svg(&mut re);
        re.set_spans(vec![TextSpan {
            range: 0..4,
            format: CharFormat { bold: true, ..CharFormat::default() },
        }]);
        assert!(re.format_at(0).bold);
        assert!(!re.format_at(6).bold);
        assert_ne!(plain, render_to_svg(&mut re), "a bold run must change the ink");
    }

    /// A press followed by a drag selects the swept range.
    #[test]
    fn a_drag_extends_the_selection() {
        let mut re = RichEdit::new(Rect::new(0, 0, 300, 200));
        re.set_text("alpha beta".to_string());
        re.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point::new(6, 12),
            button: 1,
            modifiers: 0,
        });
        re.handle_event(&crate::event::Event::MouseMove { pos: crate::core::Point::new(60, 12) });
        re.handle_event(&crate::event::Event::MouseRelease {
            pos: crate::core::Point::new(60, 12),
            button: 1,
        });
        let (start, end) = re.selection().expect("a drag must select");
        assert!(end > start, "the swept range must have width: {start}..{end}");
    }

    /// Primary+C and Primary+V move selected text through the clipboard.
    #[test]
    fn clipboard_shortcuts_copy_and_paste() {
        let _clipboard = crate::clipboard::clipboard_test_guard();
        let mut source = focused(RichEdit::new(Rect::new(0, 0, 300, 200)));
        source.set_text("copy me".to_string());
        source.set_selection(0, source.text().len());
        source.handle_event(&crate::event::Event::KeyPress { key: 67, modifiers: 0b1000 });
        assert_eq!(crate::get_clipboard_text(), "copy me");

        let mut target = focused(RichEdit::new(Rect::new(0, 0, 300, 200)));
        target.handle_event(&crate::event::Event::KeyPress { key: 86, modifiers: 0b1000 });
        assert_eq!(target.text(), "copy me");
    }

    /// The wheel scrolls the viewport and clamps it to the document.
    #[test]
    fn the_wheel_scrolls_and_clamps() {
        let mut re = RichEdit::new(Rect::new(0, 0, 200, 60));
        re.set_text((0..30).map(|n| format!("line {n}\n")).collect::<String>());
        assert_eq!(re.first_visible_line(), 0);
        let notch = crate::core::Point::new(0, -1);
        re.handle_event(&crate::event::Event::Wheel { delta: notch, modifiers: 0 });
        let scrolled = re.first_visible_line();
        assert!(scrolled > 0, "a wheel notch must scroll the viewport");
        for _ in 0..40 {
            re.handle_event(&crate::event::Event::Wheel { delta: notch, modifiers: 0 });
        }
        let max_first = re.line_count().saturating_sub(re.visible_line_count());
        assert_eq!(re.first_visible_line(), max_first, "the viewport clamps at the last window");
    }

    /// I-P57: formatting after the edit point follows the characters it was applied to.
    #[test]
    fn edits_shift_existing_spans() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 100)));
        re.set_text("hello world".to_string());
        re.set_spans(vec![TextSpan {
            range: 6..11,
            format: CharFormat { bold: true, ..CharFormat::default() },
        }]);
        re.set_selection(0, 0);
        re.insert_committed_text("X");
        assert_eq!(re.spans()[0].range, 7..12, "the span moves past the insertion");
        assert!(re.spans()[0].format.bold);
    }

    /// I-P55: a continuous typed run is one undo step.
    #[test]
    fn a_typing_run_merges_into_one_undo_step() {
        let mut re = focused(RichEdit::new(Rect::new(0, 0, 300, 100)));
        re.set_selection(0, 0);
        for ch in ["a", "b", "c"] {
            re.handle_event(&crate::event::Event::TextInput { text: ch.to_string() });
        }
        assert_eq!(re.text(), "abc");
        assert!(re.undo());
        assert_eq!(re.text(), "", "one undo reverses the whole typed run");
        assert!(!re.can_undo());
    }

    /// I-P57: an undo replay keeps the viewport on the caret instead of jumping to the top.
    #[test]
    fn undo_keeps_the_viewport_near_the_caret() {
        let mut re = RichEdit::new(Rect::new(0, 0, 200, 60));
        re.set_text((0..30).map(|n| format!("line {n}\n")).collect::<String>());
        re.set_cursor_position(20 * 8);
        re.set_first_visible_line(0);
        re.insert_committed_text("x");
        assert!(re.undo());
        assert!(re.first_visible_line() > 0, "the viewport follows the restored caret");
    }

    /// I-109: the property route emits `read_only_changed` like the method does.
    #[test]
    fn the_property_route_emits_read_only_changed() {
        use crate::widget::capability::types::CapabilityValue;
        use crate::widget::capability::WidgetProperties;
        let mut re = RichEdit::new(Rect::new(0, 0, 200, 60));
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        re.read_only_changed.connect({
            let seen = std::sync::Arc::clone(&seen);
            move |flag| {
                if *flag {
                    seen.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
        re.set("read_only", CapabilityValue::Bool(true)).expect("the property is writable");
        assert!(seen.load(std::sync::atomic::Ordering::SeqCst));
    }
}
