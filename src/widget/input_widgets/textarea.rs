// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TextArea widget — multi-line text input (BLUE13 R2.5).
use crate::compat::{Box, Rc, RefCell, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
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
            undo_stack: UndoStack::new(),
            history_target,
            restoring_history: false,
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
    /// [`Self::normalize_selection`].
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
            self.base.request_redraw();
        }
    }

    /// Selects the whole value and puts the caret at its end.
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor_pos = self.text.len();
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
        self.base.request_redraw();
        true
    }

    /// The half-open byte range of one logical line, including its terminator, or `None` when the
    /// line does not exist.
    fn line_byte_range(&self, line: usize) -> Option<(usize, usize)> {
        let mut start = 0usize;
        for (current, chunk) in self.text.split_inclusive('\n').enumerate() {
            let end = start + chunk.len();
            if current == line {
                return Some((start, end));
            }
            start = end;
        }
        None
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
        let Some((start, end)) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };
        self.text.replace_range(start..end, "");
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
        *self.history_target.borrow_mut() = self.text.clone();
        self.undo_stack.push(Box::new(TextSnapshotCommand::new(
            self.history_target.clone(),
            before,
            self.text.clone(),
            "text_area_text",
        )));
        // Every edit funnels through here, so this is the one place the viewport has to follow the
        // caret. Keeping it at the call sites would let the next edit path added forget it, and the
        // symptom (typing into a row the user cannot see) is invisible in a snapshot test.
        self.scroll_caret_into_view();
        self.changed.emit();
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
        let next = if max > 0 && text.len() > max {
            // Use floor_char_boundary to avoid splitting a multi-byte UTF-8 char
            let boundary = floor_char_boundary(&text, max);
            text[..boundary].to_string()
        } else {
            text
        };
        let before = self.text.clone();
        self.text = next;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "text_area_text",
            )));
        }
        self.cursor_pos = self.text.len();
        self.changed.emit();
        self.base.request_redraw();
    }

    /// Inserts a single character at the current cursor position.
    ///
    /// If `max_length` is greater than zero and the text already meets or exceeds
    /// that limit, the character is not inserted.
    ///
    /// SAFETY: Ensures `cursor_pos` is on a valid UTF-8 char boundary before
    /// inserting. If not, snaps to the nearest char boundary via `floor_char_boundary`.
    /// Inserts a single character at the current cursor position.
    ///
    /// If `max_length` is greater than zero and the text already meets or exceeds
    /// that limit, the character is not inserted.
    ///
    /// SAFETY: Ensures `cursor_pos` is on a valid UTF-8 char boundary before
    /// inserting. If not, snaps to the nearest char boundary via `floor_char_boundary`.
    pub fn insert(&mut self, ch: char) {
        if self.max_length > 0 && self.text.len() >= self.max_length {
            return;
        }
        // Typing over a selection **replaces** it. This is the one thing every text field does and
        // the only way to fix a word you just selected, so it happens before the boundary arithmetic
        // below — the caret has to be at the range's start, not where it used to be.
        self.replace_selection();
        let boundary = floor_char_boundary(&self.text, self.cursor_pos);
        self.cursor_pos = boundary;
        let mut next = self.text.clone();
        next.insert(self.cursor_pos, ch);
        let new_cursor_pos = self.cursor_pos + ch.len_utf8();
        self.set_text(next);
        self.cursor_pos = new_cursor_pos.min(self.text.len());
    }

    /// Deletes the character immediately before the cursor, or the selection when there is one.
    ///
    /// If the cursor is at position 0 with no selection, this is a no-op.
    pub fn delete_char(&mut self) {
        // A selection is what Backspace means when there is one: the whole range goes, not just the
        // single character behind the caret.
        let before = self.text.clone();
        if self.replace_selection() {
            self.record_edit(before);
            self.base.request_redraw();
            return;
        }
        if self.cursor_pos == 0 {
            return;
        }
        let prev = self.text[..self.cursor_pos].char_indices().last().map(|(i, c)| {
            if c == '\n' {
                (i, 1)
            } else {
                (i, c.len_utf8())
            }
        });
        if let Some((start, len)) = prev {
            let mut next = self.text.clone();
            next.replace_range(start..start + len, "");
            self.set_text(next);
            self.cursor_pos = start;
        }
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
        let first_line_top = FIRST_LINE_INSET + (LINE_H - Font::default().size_i32()) / 2;
        let usable = self.geometry().height as i32 - first_line_top;
        ((usable / LINE_H).max(1)) as usize
    }

    /// The logical line a caret at `offset` sits on.
    fn line_index_of(&self, offset: usize) -> usize {
        let caret = floor_char_boundary(&self.text, offset.min(self.text.len()));
        self.text[..caret].chars().filter(|&c| c == '\n').count()
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
        let caret_line = self.line_index_of(self.cursor_pos);
        let visible = self.visible_line_count();
        if caret_line < self.first_visible_line {
            self.first_visible_line = caret_line;
        } else if caret_line >= self.first_visible_line + visible {
            // Put the caret's line on the last visible row, so the row the user is typing on is the
            // one they can see.
            self.first_visible_line = caret_line + 1 - visible;
        }
    }

    /// The logical line that a viewport row paints, or `None` for this control's own layout.
    ///
    /// Returned as an owned index rather than a slice so the paint path can iterate rows without
    /// re-deriving the scroll each time, and so `None` unambiguously means "no line here" — an empty
    /// row below the last line of the value, which must paint nothing.
    fn visible_line_index(&self, row: usize) -> Option<usize> {
        let index = self.first_visible_line + row;
        (index < self.line_count()).then_some(index)
    }

    /// The number of logical lines in the value, counting a trailing newline's empty line.
    pub fn line_count(&self) -> usize {
        self.text.chars().filter(|&c| c == '\n').count() + 1
    }

    /// The byte offset of the character *before* the caret.
    fn previous_char_boundary(&self) -> usize {
        if self.cursor_pos == 0 {
            return 0;
        }
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        self.text[..caret].char_indices().next_back().map(|(index, _)| index).unwrap_or(0)
    }

    /// The byte offset one character after the caret.
    fn next_char_boundary(&self) -> usize {
        if self.cursor_pos >= self.text.len() {
            return self.text.len();
        }
        let caret = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        self.text[caret..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| caret + index)
            .unwrap_or(self.text.len())
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

    /// The byte offset of `column` characters into `text[line_start..line_end]`, saturating at
    /// `line_end` when the line is shorter.
    fn byte_index_at_column(&self, line_start: usize, line_end: usize, column: usize) -> usize {
        self.text[line_start..line_end]
            .char_indices()
            .nth(column)
            .map(|(offset, _)| line_start + offset)
            .unwrap_or(line_end)
    }

    /// Sets the maximum text length, truncating the value if it exceeds the new limit.
    ///
    /// # The limit is a **byte** budget, and the truncation must respect that
    ///
    /// Unlike `LineEdit`, whose limit counts characters, this control counts bytes — the value is
    /// a multi-line document and the limit is a buffer bound. The truncation therefore has to
    /// floor to a character boundary: `String::truncate` panics outright when the index falls
    /// inside a character, so `set_max_length` on `"你好"` (six bytes) with a limit of four took
    /// the process down. `set_text` already guarded its own truncation with
    /// [`floor_char_boundary`]; this path did not.
    pub fn set_max_length(&mut self, max: usize) {
        let previous = self.max_length;
        self.max_length = max;
        if max > 0 && self.text.len() > max {
            let boundary = floor_char_boundary(&self.text, max);
            self.text.truncate(boundary);
            // The caret must stay on a boundary inside what is left.
            self.cursor_pos = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
            self.changed.emit();
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
        if text.is_empty() {
            return;
        }
        let before = self.text.clone();
        self.replace_selection();
        let at = floor_char_boundary(&self.text, self.cursor_pos.min(self.text.len()));
        let mut next = self.text.clone();
        next.insert_str(at, text);
        self.text = next;
        // Applied after the insertion so the limit bounds the final value. The boundary snap is
        // not optional: `String::truncate` panics on a non-boundary index (see `set_max_length`).
        if self.max_length > 0 && self.text.len() > self.max_length {
            let boundary = floor_char_boundary(&self.text, self.max_length);
            self.text.truncate(boundary);
        }
        self.cursor_pos = floor_char_boundary(&self.text, at + text.len());
        self.selection_anchor = None;
        self.record_edit(before);
        self.base.request_redraw();
    }

    /// Deletes the character after the caret, or the selection when there is one.
    ///
    /// The counterpart of [`Self::delete_char`]. Without it the `Delete` key was simply not
    /// handled: it fell through to the character-input catch-all, where key code 46 is `.`, so
    /// pressing Delete typed a full stop and deleted nothing.
    pub fn delete_forward(&mut self) {
        let before = self.text.clone();
        if self.replace_selection() {
            self.record_edit(before);
            self.base.request_redraw();
            return;
        }
        if self.cursor_pos >= self.text.len() {
            return;
        }
        let start = floor_char_boundary(&self.text, self.cursor_pos);
        let end = self.next_char_boundary();
        self.text.replace_range(start..end, "");
        self.cursor_pos = start;
        self.record_edit(before);
        self.base.request_redraw();
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
        self.base.request_redraw();
    }

    /// Ends a pointer selection, collapsing a gesture that never moved. Returns whether one was in
    /// progress.
    pub fn end_drag(&mut self) -> bool {
        let was_dragging = core::mem::replace(&mut self.dragging_selection, false);
        if was_dragging {
            self.normalize_selection();
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
        let text_top = rect.y + FIRST_LINE_INSET + (LINE_H - Font::default().size_i32()) / 2;

        let row = ((pos.y - text_top).max(0) / LINE_H) as usize;
        // The viewport row is translated to the logical line it paints, so a click after scrolling
        // resolves to the line the user sees there rather than to the document's own nth line.
        let line_index = self.first_visible_line + row;
        let x = pos.x - origin_x;

        // Walk the value's own line splitting, so the answer is derived from the text rather than
        // rebuilt from a second model of where the newlines are.
        let mut line_start = 0usize;
        for (current, chunk) in self.text.split_inclusive('\n').enumerate() {
            let line = chunk.trim_end_matches('\n');
            if current == line_index {
                return line_start + self.column_at_x(line, x);
            }
            line_start += chunk.len();
        }
        // The point is below every line the value has, or the value is empty: the caret belongs at
        // the end.
        self.text.len()
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
        self.cursor_pos = self.cursor_pos.min(self.text.len());
        self.restoring_history = false;
        self.changed.emit();
        self.base.request_redraw();
    }
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

    fn size_hint(&self) -> Size {
        let line_count = if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        };
        let max_line_width = self.text.lines().map(|l| l.len() as u32).max().unwrap_or(0);
        let w = (max_line_width * 8 + 10).max(120);
        let h = (line_count as u32 * 16 + 10).max(60);
        Size::new(w, h)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TextArea`'s property contract.
impl WidgetProperties for TextArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "read_only" => Ok(CapabilityValue::Bool(self.is_read_only())),
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TEXT_AREA_PROPERTIES`.
        property_names_of!["text", "placeholder", "read_only", BASE_PROPERTY_NAMES]
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
                self.insert(' ');
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
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained { .. } => {
                self.focused = true;
                self.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.request_redraw();
            }
            Event::KeyPress { key, modifiers } => {
                // The event carries the framework's wire bitmask; translate it once, here, so the
                // rest of this handler speaks `Modifiers` and nothing below re-derives a bit.
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
                    // Primary+A: select all. Handled on the control that owns the text, so a host
                    // whose window-level "select all" means something else does not swallow it.
                    self.select_all();
                    return;
                }
                if self.read_only {
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
                        self.insert('\n');
                        self.request_redraw();
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
                                    self.insert(ch);
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
                self.focused = true;
                let index = self.byte_index_at(*pos);
                self.press_at(index, crate::shortcut::Modifiers::from_event_bits(*modifiers));
            }
            Event::MouseMove { pos } => {
                if self.dragging_selection {
                    let index = self.byte_index_at(*pos);
                    self.drag_to(index);
                }
            }
            // A release anywhere ends the gesture, including outside the control: the anchor stays
            // where the press put it, so the selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
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
        // A text area is the one field whose height is **not** a fixed band: it is
        // multi-line, so the lines legitimately span whatever height the caller gives it.
        // `rect.height` therefore stays. Everything *inside* is measured from the field's
        // own padding rather than from a local literal, so the first line starts where a
        // single-line field's does instead of at a second, unshared `y = 4`.
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;

        // -- Background --
        let bg = self.style().background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.face(
            rect,
            bg,
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // -- Border --
        let border = self.style().border_color.unwrap_or(Color::rgb(200, 200, 200));
        context.draw_rect(rect, border);

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
        let first_band = Rect::new(origin_x, rect.y + FIRST_LINE_INSET, rect.width, LINE_H as u32);
        let first_line = context.text_line(first_band, &Font::default());
        let text_top = first_line.y;

        if self.text.is_empty() && !self.placeholder.is_empty() && !self.focused {
            // Draw placeholder in gray, on the same first-line box the value would use.
            context.draw_text(
                Point::new(origin_x, text_top),
                &self.placeholder,
                &Font::default(),
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
                let mut line_start = 0usize;
                let mut y = text_top;
                let mut row = 0usize;
                while let Some(line_index) = self.visible_line_index(row) {
                    let Some((_, line)) =
                        self.text.split_inclusive('\n').enumerate().nth(line_index)
                    else {
                        break;
                    };
                    let line = line.trim_end_matches('\n');
                    let line_end = line_start + line.len();
                    let _ = line_index;
                    // The overlap of `[sel_start, sel_end)` with this line's own content.
                    let from = sel_start.max(line_start).min(line_end);
                    let to = sel_end.min(line_end).max(from);
                    if to > from {
                        // Measured, not multiplied: the band's two edges have to land on the same
                        // pixels the glyphs do, and the glyphs are laid out with the face's own
                        // advances (see `advance_of_caret_prefix` for what a fixed width costs).
                        let left = origin_x + self.text_advance(&self.text[line_start..from]);
                        let width = self.text_advance(&self.text[from..to]);
                        if width > 0 {
                            context.draw_rect(
                                Rect::new(left, y, width as u32, LINE_H as u32),
                                highlight,
                            );
                        }
                    }
                    line_start += line.len() + 1;
                    y += LINE_H;
                    row += 1;
                }
            }

            let mut y = text_top;
            // Rows are drawn from the scroll position, so the field is a window onto the document
            // rather than always showing its first line. `visible_line_index` is the one lookup of
            // "which logical line is on row n", shared with the pixel-to-offset map below, so a
            // click and the glyphs it lands on cannot disagree about the scroll.
            let mut row = 0usize;
            while let Some(line_index) = self.visible_line_index(row) {
                let Some(line) = self.text.lines().nth(line_index) else { break };
                if y + LINE_H > rect.y + rect.height as i32 {
                    break;
                }
                context.draw_text(
                    Point::new(origin_x, y),
                    line,
                    &Font::default(),
                    text_color,
                    HorizontalAlignment::Left,
                );
                y += LINE_H;
                row += 1;
            }
        }

        // -- Cursor --
        if self.focused {
            let cursor_x = self.cursor_screen_x(origin_x);
            let cursor_y = self.cursor_screen_y(text_top);
            context.draw_line(
                Point::new(cursor_x, cursor_y),
                Point::new(cursor_x, cursor_y + LINE_H),
                text_color,
            );
        }
    }
}

impl TextArea {
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
        origin_x + self.advance_of_caret_prefix(self.cursor_pos)
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
        let line_start = self.text[..caret].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let prefix = &self.text[line_start..caret];
        if prefix.is_empty() {
            return 0;
        }
        self.text_advance(prefix)
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
        context.measure_text(text, &Font::default()).width as i32
    }

    /// The byte offset the horizontal coordinate `x` points at, **within `line`**.
    ///
    /// Found by walking the characters and measuring the prefix, so the answer is the character the
    /// pointer is actually over rather than the one a fixed advance would guess. The result is a
    /// character boundary by construction.
    fn column_at_x(&self, line: &str, x: i32) -> usize {
        let target = x.max(0) as f32;
        let font = Font::default();
        let mut best = 0usize;
        let mut prefix_width = 0.0f32;
        for (index, ch) in line.char_indices() {
            let advance = self.char_advance_at(index, line);
            // The midpoint decides which side of a character's centre the pointer is on, which is the
            // conventional rule and the one that makes clicking the left half of a glyph put the
            // caret before it.
            if target < prefix_width + advance / 2.0 {
                let _ = (font, ch);
                break;
            }
            prefix_width += advance;
            best = index + ch.len_utf8();
        }
        best
    }

    /// The advance of the character at `byte_index` in `line`.
    fn char_advance_at(&self, byte_index: usize, line: &str) -> f32 {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let context = RenderContext::new(&mut measurement);
        let ch = line[byte_index..].chars().next().unwrap_or(' ');
        context.measure_text(&ch.to_string(), &Font::default()).width as f32
    }

    /// Computes the screen-space Y coordinate of the cursor (top of cursor line).
    ///
    /// The caret's own line is measured **relative to the viewport**, so a scrolled field draws the
    /// marker on the row the character is actually painted on rather than at its document position —
    /// which would put it above the field, or below it, once anything had been scrolled past.
    fn cursor_screen_y(&self, origin_y: i32) -> i32 {
        let line = self.line_index_of(self.cursor_pos);
        let row = line.saturating_sub(self.first_visible_line);
        origin_y + row as i32 * LINE_H
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

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
        ta.insert('l');
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

    #[test]
    fn textarea_insert_respects_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(3);
        ta.set_text("ABC".to_string());
        ta.insert('D');
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

    /// `set_max_length` must not truncate inside a multi-byte character.
    ///
    /// The limit is a byte budget on this control (a document's buffer bound), but
    /// `String::truncate` **panics** when the byte index is not a character boundary — so
    /// `set_max_length(4)` on `"你好"` (six bytes) took the process down instead of trimming it.
    #[test]
    fn max_length_truncation_respects_character_boundaries() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_text("你好");
        ta.set_max_length(4);
        assert!(ta.text().is_char_boundary(ta.text().len()));
        assert_eq!(ta.text(), "你", "truncated to the last boundary at or before 4 bytes");
        assert!(ta.text().is_char_boundary(ta.cursor_pos()));
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
        ta.handle_event(&Event::key_press(9, 0)); // Tab
        assert!(ta.text().is_empty(), "a bare Tab is a control code, not content");
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
}
