// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MaskedEdit widget — a text input with mask-based formatting.
//!
//! The MaskedEdit widget provides formatted text input using a mask pattern.
//! Mask characters define which types of input are allowed at each position:
//! - `0` — required digit
//! - `9` — optional digit
//! - `A` — required letter (a-z, A-Z)
//! - `a` — optional letter
//! - `X` — required alphanumeric
//! - `x` — optional alphanumeric
//! - All other characters are treated as literal separators and appear automatically.
//!
//! Example: mask `(000) 000-0000` formats phone numbers like `(555) 123-4567`.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::coercion::{
    expect_horizontal_alignment, expect_string, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::text_utils::byte_index_of_char;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// A parsed segment in the mask — either a literal character or an input placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaskSegment {
    /// A literal character that appears as-is (e.g., `(`, `)`, `-`, space).
    Literal { ch: char },
    /// A user-input position with a mask character from `[09AaXx]`.
    Input { kind: MaskCharKind },
}

/// The kind of input allowed at a mask position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaskCharKind {
    /// `0` — required digit.
    RequiredDigit,
    /// `9` — optional digit.
    OptionalDigit,
    /// `A` — required letter.
    RequiredLetter,
    /// `a` — optional letter.
    OptionalLetter,
    /// `X` — required alphanumeric.
    RequiredAlphaNum,
    /// `x` — optional alphanumeric.
    OptionalAlphaNum,
}

/// MaskedEdit widget — formatted text input with a mask.
///
/// Provides a text field where input is constrained by a mask pattern.
/// Non-input characters appear automatically, and the cursor skips over
/// literal positions. The widget emits a `text_changed` signal whenever
/// the raw text content changes.
pub struct MaskedEdit {
    base: BaseWidget,
    /// The mask pattern string (e.g., "(000) 000-0000").
    mask: String,
    /// Parsed mask segments derived from the mask string.
    segments: Vec<MaskSegment>,
    /// Raw user input text (without literal characters).
    raw_text: String,
    /// Display text (with literals inserted per the mask).
    display_text: String,
    /// Current cursor position within the display text.
    cursor_pos: usize,
    /// The other end of the selection, as a **display** position, or `None` when nothing is
    /// selected.
    ///
    /// # Why the anchor is a display position and not a raw index
    ///
    /// This control has two coordinate spaces: the *raw* value the user typed, and the *display*
    /// string with the mask's literals inserted. The caret lives in display space, because that is
    /// what the user sees and points at, so the anchor does too — a range could not otherwise be
    /// compared with the caret to decide which end is which. The translation to raw happens once,
    /// in [`MaskedEdit::selection_raw_range`], so no other method has to know both spaces.
    selection_anchor: Option<usize>,
    /// Whether a pointer drag is currently extending a selection.
    ///
    /// Set by [`MaskedEdit::press_at`], cleared by [`MaskedEdit::end_drag`]. It exists because
    /// `MouseMove` is delivered for a bare hover too, so without it a caret the user never grabbed
    /// would follow the pointer across the field.
    dragging_selection: bool,
    /// Whether this widget currently has keyboard focus.
    focused: bool,
    /// How the field's content is aligned within its own box.
    ///
    /// Horizontal only: the field is a fixed-height band and the characters are centred vertically
    /// in it as a matter of the field's own layout, so a `top`/`bottom` value would be one this
    /// control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    /// Emitted when the text changes, providing the raw text.
    pub text_changed: Signal1<String>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl MaskedEdit {
    /// Creates a new MaskedEdit widget with the given geometry.
    ///
    /// Initially has an empty mask, so all input is accepted freely.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MaskedEdit, geometry, "MaskedEdit"),
            mask: String::new(),
            segments: Vec::new(),
            raw_text: String::new(),
            display_text: String::new(),
            cursor_pos: 0,
            selection_anchor: None,
            dragging_selection: false,
            focused: false,
            alignment: crate::core::Alignment::Left,
            text_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }

    /// Sets the input mask. The mask pattern is parsed into segments.
    ///
    /// If the mask is empty, no formatting is applied and all input is accepted.
    pub fn set_mask(&mut self, mask: &str) {
        self.mask = mask.to_string();
        self.segments = parse_mask(mask);
        self.cursor_pos = 0;
        self.update_display_text();
        self.base.request_redraw();
    }

    /// Returns the current mask pattern.
    pub fn mask(&self) -> &str {
        &self.mask
    }

    /// How the field's content is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's content is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the field's characters
    /// are centred vertically in the field band by the field's own layout — the property route
    /// refuses it through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and
    /// this setter matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Returns the raw user input text (without mask literals).
    pub fn raw_text(&self) -> &str {
        &self.raw_text
    }

    /// Sets the raw text. The text is validated against the mask, and only
    /// characters that match the mask positions are accepted.
    ///
    /// # Why an unmasked field accepts everything
    ///
    /// With no mask there are no segment positions to validate against, and
    /// [`MaskedEdit::set_mask`] documents exactly that case: "If the mask is empty, no
    /// formatting is applied and all input is accepted". The loop below used to iterate
    /// `self.segments` unconditionally, so with no mask it ran zero times and **silently
    /// discarded the whole string** — `set_text("Sample")` left `raw_text` empty and the
    /// control painted nothing. That is why the census SVG for this control contained an
    /// empty `<text></text>`: not a rendering subtlety, but a setter that threw its
    /// argument away.
    pub fn set_text(&mut self, text: &str) {
        let before = self.raw_text.clone();
        self.raw_text = String::new();
        if self.segments.is_empty() {
            // No mask: the text is the value, verbatim.
            self.raw_text = text.to_string();
        } else {
            let mut chars = text.chars();
            for seg in &self.segments {
                if let MaskSegment::Input { kind } = seg {
                    for ch in chars.by_ref() {
                        if mask_char_matches(*kind, ch) {
                            self.raw_text.push(ch);
                            break;
                        }
                    }
                }
            }
        }
        self.update_display_text();
        // The caret is a **segment** index (see [`Self::display_to_raw_index`], which iterates the
        // segments by index), not a byte offset into the display string. Assigning
        // `display_text.len()` put the caret past the last segment on any mask whose literal
        // separators are multi-byte, so the drawn caret vanished. `segments.len()` is the count of
        // display positions the caret may occupy.
        self.cursor_pos = self.segments.len();
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.raw_text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.raw_text.clone(),
                "masked_edit_text",
            )));
        }
        self.text_changed.emit(self.raw_text.clone());
        self.base.request_redraw();
    }

    /// Returns the display text (with mask literals inserted).
    pub fn text(&self) -> &str {
        &self.display_text
    }

    /// Steps back one edit and returns `true`, or `false` when there is nothing to
    /// undo.
    ///
    /// The restored value is re-applied through the mask, so it is validated and
    /// re-rendered like freshly typed input rather than pasted in verbatim.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }
    /// Steps forward one undone edit and returns `true`, or `false` when there is
    /// nothing to redo. Behaviour matches [`MaskedEdit::undo`].
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }
    /// Returns `true` if [`MaskedEdit::undo`] would change the value.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`MaskedEdit::redo`] would change the value.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }
    fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        self.restoring_history = true;
        self.set_text(&text);
        self.restoring_history = false;
    }

    /// Returns whether all required mask positions are filled.
    ///
    /// # Both sides count **characters**
    ///
    /// `required_count` counts input *segments*, and the value's length is therefore a character
    /// count. `raw_text.len()` is a byte count, so on any multi-byte raw value the test reported
    /// "valid" while the field was still short of required positions. The raw text is normally
    /// ASCII (the mask's own character classes only admit ASCII), but `set_text` stores an
    /// unmasked value verbatim, so the two frames can genuinely differ.
    pub fn is_valid(&self) -> bool {
        let required_count = self
            .segments
            .iter()
            .filter(|s| {
                matches!(
                    s,
                    MaskSegment::Input { kind: MaskCharKind::RequiredDigit }
                        | MaskSegment::Input { kind: MaskCharKind::RequiredLetter }
                        | MaskSegment::Input { kind: MaskCharKind::RequiredAlphaNum }
                )
            })
            .count();
        self.raw_text.chars().count() >= required_count && !self.mask.is_empty()
    }

    /// Returns the current cursor position in the display text.
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Sets the cursor position in the display text.
    ///
    /// The position is a **segment** index (a display position), so it is clamped to the number of
    /// mask segments rather than to the display string's byte length.
    ///
    /// This is the programmatic "put the caret here", not a movement, so it **drops any selection**.
    /// The `Shift`-aware path a *user* takes is [`Self::select_with_modifiers`], which keeps the
    /// anchor on purpose. Without the drop, a caller that set a position after a selection left the
    /// anchor behind, and the next `Shift+arrow` extended from a position belonging to a gesture the
    /// user had already finished — a range starting somewhere they never chose.
    pub fn set_cursor_pos(&mut self, pos: usize) {
        self.cursor_pos = pos.min(self.segments.len());
        self.selection_anchor = None;
        self.base.request_redraw();
    }

    /// The selected range as ordered `(start, end)` **display** positions, or `None` when nothing
    /// is selected.
    ///
    /// Display positions rather than raw indices because that is the space the caret moves in and
    /// the paint highlights; [`Self::selection_raw_range`] is the one place that translates.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let caret = self.cursor_pos.min(self.segments.len());
        if anchor == caret {
            return None;
        }
        Some(if anchor < caret { (anchor, caret) } else { (caret, anchor) })
    }

    /// The selected range as ordered `(start, end)` **raw** character indices.
    ///
    /// # Why a display range must be re-derived rather than divided
    ///
    /// The mask sits *between* the two spaces: display position 5 may already be raw character 2
    /// because three literal characters precede it. A display range translated by averaging or by
    /// subtracting the literal count would land on the wrong character, so each end is converted
    /// independently through [`Self::display_to_raw_index`] — the same function the caret and the
    /// insert path use, so all three agree about which character a position addresses.
    ///
    /// The start is converted from the display position *after* the last selected character while
    /// the end is converted from the one *before* the first, which is what makes the raw range
    /// cover exactly the characters the user highlighted and no literal-adjacent neighbour.
    pub fn selection_raw_range(&self) -> Option<(usize, usize)> {
        let (start, end) = self.selection_range()?;
        Some((self.display_to_raw_index(start), self.display_to_raw_index(end)))
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

    /// Selects the whole value: from display position 0 to the end of the mask.
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor_pos = self.segments.len();
        self.base.request_redraw();
    }

    /// Moves the caret to display position `target`, honouring the modifier keys held.
    ///
    /// # The single entry point for every keyboard movement
    ///
    /// Left, Right, Home and End all funnel through here, so "what Shift means" is written once
    /// instead of four times (principle #101):
    ///
    /// * **Shift** — extend: the caret moves to `target` while the anchor stays put, so the
    ///   selection becomes the range between them. With no anchor yet the current caret is adopted
    ///   as the anchor, so `Shift+Home` selects the prefix rather than the whole value.
    /// * **No modifier** — replace: the caret moves and any selection is dropped.
    ///
    /// The anchor belongs to the whole gesture, so a later extension does **not** re-anchor; that is
    /// what makes `Shift+End` after `Shift+Home` sweep back to the other side of the same anchor.
    ///
    /// `target` is clamped to the number of mask segments, which is the same bound
    /// [`Self::set_cursor_pos`] applies — the caret addresses display positions, not bytes.
    pub fn select_with_modifiers(&mut self, target: usize, modifiers: crate::shortcut::Modifiers) {
        let target = target.min(self.segments.len());
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor_pos.min(self.segments.len()));
            }
            self.cursor_pos = target;
        } else {
            self.selection_anchor = None;
            self.cursor_pos = target;
        }
        self.normalize_selection();
        self.base.request_redraw();
    }

    /// Collapses a zero-width anchor/caret pair to "no selection".
    fn normalize_selection(&mut self) {
        if self.selection_anchor == Some(self.cursor_pos) {
            self.selection_anchor = None;
        }
    }

    /// Starts a pointer selection at display position `index`.
    ///
    /// A press with no `Shift` begins a new gesture anchored where it landed; with `Shift` it
    /// extends from the existing anchor, moving the caret to the pressed position. The anchor is
    /// **not** normalised here — this is the start of a gesture, and a drag grows from it — so the
    /// collapse happens in [`Self::end_drag`] instead.
    pub fn press_at(&mut self, index: usize, modifiers: crate::shortcut::Modifiers) {
        let index = index.min(self.segments.len());
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor_pos.min(self.segments.len()));
            }
            self.cursor_pos = index;
        } else {
            self.selection_anchor = Some(index);
            self.cursor_pos = index;
        }
        self.dragging_selection = true;
        self.base.request_redraw();
    }

    /// Extends an in-progress pointer selection to `index`.
    pub fn drag_to(&mut self, index: usize) {
        if !self.dragging_selection {
            return;
        }
        self.cursor_pos = index.min(self.segments.len());
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

    /// Inserts a character at the current cursor position.
    fn insert_char(&mut self, ch: char) {
        // Typing over a selection replaces it: the highlighted characters go, and the new one takes
        // their place. This has to happen before the caret is read below, because the replacement
        // moves the caret to the selection's start.
        self.delete_selection();
        let raw_idx = self.display_to_raw_index(self.cursor_pos);
        if raw_idx >= self.input_count() {
            return;
        }

        // Find the input segment at this raw index
        if let Some((seg_idx, kind)) = self.find_input_at_raw_index(raw_idx) {
            if mask_char_matches(kind, ch) {
                // `raw_idx` counts input positions (characters); `String::insert` takes a byte
                // offset. Converting here — rather than inserting at the character index — keeps
                // the byte index on a character boundary, which is what `String::insert` requires.
                let byte = byte_index_of_char(&self.raw_text, raw_idx);
                self.raw_text.insert(byte, ch);
                self.update_display_text();
                // Move cursor past this input
                self.cursor_pos = seg_idx + 1;
                self.text_changed.emit(self.raw_text.clone());
                self.base.request_redraw();
            }
        }
    }

    /// Feeds platform-committed text through the mask one character at a time (D09-INPUT-01).
    ///
    /// The desktop backends deliver a printable character or an IME commit as
    /// `Event::TextInput`/`Event::ImeCommit`, not as a `KeyPress`. `insert_char` is the same entry
    /// point the `KeyPress` printable arm uses, so each committed character is validated against
    /// the mask and advances the caret exactly as a keystroke would — which is what keeps a
    /// multi-character commit filling successive slots instead of only the first.
    fn insert_committed_text(&mut self, text: &str) {
        for ch in text.chars().filter(|c| !c.is_control()) {
            self.insert_char(ch);
        }
    }

    /// Deletes the character before the cursor (backspace).
    fn backspace(&mut self) {
        // A selection is what Backspace means when there is one: the whole range goes, not just the
        // character behind the caret.
        if self.delete_selection() {
            return;
        }
        if self.raw_text.is_empty() || self.cursor_pos == 0 {
            return;
        }
        let raw_idx = self.display_to_raw_index(self.cursor_pos);
        let char_count = self.raw_text.chars().count();
        if raw_idx > 0 && raw_idx <= char_count {
            // Remove the *nth* character, found by byte offset, so a multi-byte raw value loses a
            // whole character instead of one byte of it.
            let start = byte_index_of_char(&self.raw_text, raw_idx - 1);
            let end = byte_index_of_char(&self.raw_text, raw_idx);
            self.raw_text.replace_range(start..end, "");
            self.update_display_text();
            self.cursor_pos = self.segment_before_raw_index(raw_idx - 1);
            self.text_changed.emit(self.raw_text.clone());
            self.base.request_redraw();
        }
    }

    /// Deletes the character at the cursor (delete).
    fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        let raw_idx = self.display_to_raw_index(self.cursor_pos);
        if raw_idx < self.raw_text.chars().count() {
            let start = byte_index_of_char(&self.raw_text, raw_idx);
            let end = byte_index_of_char(&self.raw_text, raw_idx + 1);
            self.raw_text.replace_range(start..end, "");
            self.update_display_text();
            self.text_changed.emit(self.raw_text.clone());
            self.base.request_redraw();
        }
    }

    /// Rebuilds the display text from raw_text and the mask segments.
    fn update_display_text(&mut self) {
        self.display_text = build_display_text(&self.segments, &self.raw_text);
    }

    /// Removes the selected raw characters, returns whether anything was removed, and leaves the
    /// caret at the start of what was selected.
    ///
    /// # Why the range goes through the raw space
    ///
    /// `raw_text` holds the user's characters with no mask literals in it, so a deletion expressed
    /// in display positions has to be translated before it can be removed — editing the display
    /// string would take the mask's own characters with it. Both ends are converted through
    /// [`Self::selection_raw_range`], which is the same translation the reader uses, so a delete and
    /// a copy can never disagree about which characters were selected.
    ///
    /// The display text is rebuilt afterwards, which re-inserts the literals the shortened raw value
    /// no longer reaches — so deleting the middle of a formatted number leaves the formatting
    /// intact rather than a hole in the mask.
    fn delete_selection(&mut self) -> bool {
        let Some((raw_start, raw_end)) = self.selection_raw_range() else {
            self.selection_anchor = None;
            return false;
        };
        if raw_start >= raw_end {
            self.selection_anchor = None;
            return false;
        }
        let start = byte_index_of_char(&self.raw_text, raw_start);
        let end = byte_index_of_char(&self.raw_text, raw_end);
        self.raw_text.replace_range(start..end, "");
        self.update_display_text();
        // The caret returns to the display position the removed run began at, which is where the
        // user expects to keep typing. `segment_before_raw_index` answers "the segment before this
        // character", and after the removal that is exactly the first position the run vacated.
        self.cursor_pos = self.segment_before_raw_index(raw_start);
        self.selection_anchor = None;
        self.text_changed.emit(self.raw_text.clone());
        self.base.request_redraw();
        true
    }

    /// Returns the number of input segments in the mask.
    fn input_count(&self) -> usize {
        self.segments.iter().filter(|s| matches!(s, MaskSegment::Input { .. })).count()
    }

    /// Finds the segment index and mask char kind for the nth input position.
    fn find_input_at_raw_index(&self, raw_idx: usize) -> Option<(usize, MaskCharKind)> {
        let mut input_count = 0;
        for (seg_idx, seg) in self.segments.iter().enumerate() {
            if let MaskSegment::Input { kind } = seg {
                if input_count == raw_idx {
                    return Some((seg_idx, *kind));
                }
                input_count += 1;
            }
        }
        None
    }

    /// Converts a display text position to the number of input characters before it.
    fn display_to_raw_index(&self, display_pos: usize) -> usize {
        let mut raw_count = 0;
        for (display_idx, seg) in self.segments.iter().enumerate() {
            if display_idx >= display_pos {
                break;
            }
            if matches!(seg, MaskSegment::Input { .. }) {
                raw_count += 1;
            }
        }
        raw_count
    }

    /// Returns the segment index just before the nth input character.
    fn segment_before_raw_index(&self, raw_idx: usize) -> usize {
        let mut input_count = 0;
        for (seg_idx, seg) in self.segments.iter().enumerate() {
            if matches!(seg, MaskSegment::Input { .. }) {
                if input_count == raw_idx {
                    return seg_idx;
                }
                input_count += 1;
            }
        }
        self.segments.len()
    }

    /// The field the control actually paints.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A masked field is a text field: [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is the
    /// touch-sized content floor every field shares, full width and centred in the area the
    /// caller offers. Painting `geometry()` made a 240x120 census cell a 240x120 box — a
    /// panel rather than a field — and every anchor inside it (the ink's line box, the
    /// caret) was derived from the oversized box, so a themed field's text sat on the wrong
    /// row. [`ControlMetrics::full_width_band`] is the shared derivation, and the hit test
    /// below reads it too, so the clickable area is exactly the painted one.
    fn field_rect(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The display position the horizontal coordinate `x` points at.
    ///
    /// # How a pixel becomes a display position
    ///
    /// The display string is drawn one character per cell at the field's font advance, so the
    /// distance from the value's origin divided by that advance is the character the pointer is on.
    /// The advance is **measured through the same font the paint path uses** rather than assumed, so
    /// a face change moves the click target with the glyphs.
    ///
    /// The result saturates at the number of mask segments, which is the space the caret addresses —
    /// clicking past the end of the value must land at its end, not past the mask. Adding half a cell
    /// before dividing is what makes a click in the *middle* of a character select that character's
    /// boundary rather than the one before it, which is what a user pointing at a glyph expects.
    fn display_position_at_x(&self, x: i32) -> usize {
        let field = self.field_rect();
        let font = Font::simple("monospace", 13.0);
        let cell = font.size().max(1.0) as i32 * 3 / 5; // the shaper's 0.6 factor, in pixels
        let cell = cell.max(1);
        let origin = field.x + dimensions::TEXT_FIELD_PADDING_H as i32;
        let offset = (x - origin).max(0);
        let index = (offset + cell / 2) / cell;
        (index as usize).min(self.segments.len())
    }

    /// The band colour a selection is painted in.
    ///
    /// Derived from the field's own ink and fill rather than a fixed blue, so a selection reads on
    /// the light *and* the dark appearance without a second theme token to keep in step — the same
    /// derivation the placeholder uses.
    fn selection_color(&self, text_color: &Color, field_bg: &Color) -> Color {
        // Two steps, not one: the band has to land *between* ink and fill, because a single blend is
        // either invisible behind the glyphs or dark enough to swallow them.
        text_color.blend(field_bg, 0.3).blend(field_bg, 0.25)
    }
}

impl Widget for MaskedEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
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

/// `MaskedEdit`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `text` reads the formatted
/// display string the widget draws; a write goes through
/// [`MaskedEdit::set_text`], which re-validates each character against the mask.
impl WidgetProperties for MaskedEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "mask" => Ok(CapabilityValue::String(self.mask().to_string())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "mask" => {
                self.set_mask(&expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "mask", "alignment", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `masked_edit` publishes.
    ///
    /// Both assign state — the contents and the mask that validates them — and each
    /// needs a payload, so the whole set is answered through the property route. The
    /// mask argument is not merely data: changing it re-validates the text, which is a
    /// decision the caller has to make rather than one a nameless command can supply.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_mask" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for MaskedEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **field**, not the control's rectangle.
        //
        // A masked field is a text field, and every other field in this crate is
        // `TEXT_FIELD_MIN_HEIGHT` tall, full width, centred in the area it is given.
        // Painting `geometry()` made a 240x120 census cell a 240x120 box, and every
        // measurement below — the fill, the border, the ink's line box and the caret —
        // inherited that. `ControlMetrics::full_width_band` is the shared derivation, and
        // it is also what the hit test uses, so the clickable area is the visible one.
        let geom = self.field_rect();
        let is_enabled = self.base.is_enabled();
        let font = Font::simple("monospace", 13.0);

        // ── Background ──
        //
        // Resolved from the style so a theme switch reaches this control; previously the three
        // states were fixed light greys and the box stayed light in a dark theme. The state
        // ladder is preserved but *derived*: focused is the resolved colour brightened toward
        // white, disabled is faded, so the states stay distinguishable and follow the theme.
        //
        // This covers the field's **fill and border only**. The five pieces of *ink* below —
        // body text, mask literals, placeholders, the caret and the glyph drawn on the caret —
        // were still fixed literals until they were derived from the same resolved pair, so a
        // themed field carried unthemed text: `rgb(33,33,33)` on the dark theme's `rgb(69,69,69)`
        // field is ~1.35:1, which is present in the SVG and unreadable to a person.
        let style = self.style().clone();
        let themed = crate::style::resolved_theme_style("masked_edit");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        // Explicit style wins, then the theme, then the original literal as a last resort so
        // an inactive theme still leaves the control with a defined appearance.
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(248, 248, 250, 200));
        // The page this field sits in, read once: a disabled fill recedes toward it.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let accent = style.border_color.or(themed_border);
        let bg_color = if !is_enabled {
            // A disabled **field** recedes toward the page it sits in. It cannot recede toward its
            // own contrast colour: that makes a fill stand out *more* against the page, so the
            // disabled field measured 8.16:1 against the window while the enabled one was at
            // 1.95:1 -- the state read backwards. Blending toward a fixed white was the same
            // mistake pointed at a different literal, and it also pointed the same way as
            // `focused` below, so the two states were distinguished only by distance travelled.
            self.base.disabled_surface_near(base_bg, window_fill)
        } else if self.focused {
            base_bg.blend(&Color::WHITE, 0.55)
        } else {
            base_bg
        };
        context.fill_rounded_rect(geom, 4, bg_color);

        // ── Border ──
        let border_color = if self.focused && is_enabled {
            accent.unwrap_or(Color::rgb(25, 118, 210))
        } else {
            style.border_color.unwrap_or(Color::rgba(190, 190, 200, 200))
        };
        context.draw_rounded_rect_stroke(geom, 4, border_color, if self.focused { 2 } else { 1 });

        // ── Ink ──
        //
        // Every piece of text ink below resolves `style.text_color` (the theme writes the
        // resolved text colour for an `Input`-role control) and then pushes that ink away from
        // the field's own fill until it is legible on it. `style.text_color` is the right source
        // because the field's interior is what the text sits on — that is exactly the
        // standard text-over-base palette relationship — and `legible_on` is what makes
        // the result a property of the resolved pair rather than of a guessed lightness. Three
        // states are then derived from that one ink: enabled is the ink itself, disabled is
        // faded toward the fill, and a *placeholder* is a placeholder rather than a value, so it
        // is a further step toward the fill. The caret is the control's *emphasis* colour, so it
        // reads the theme's primary token rather than the text ink, and the glyph drawn inside
        // it is that colour's contrast partner.
        //
        // WCAG AA for normal text. Mask literals and placeholders are deliberately below it —
        // they are not values the user is reading — but not *arbitrarily* below: both are a fixed
        // fraction of the distance to the fill, so on any appearance they stay visibly weaker
        // than the entered text without ever collapsing into it.
        const INK_MIN_RATIO: f32 = 4.5;
        let ink = style
            .text_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
            .unwrap_or_else(|| bg_color.contrast_color())
            .legible_on(bg_color, INK_MIN_RATIO);
        let disabled_ink = ink.blend(&bg_color, 0.45);
        let mask_ink = ink.blend(&bg_color, 0.55);
        let placeholder_ink = ink.blend(&bg_color, 0.70);
        // The caret is chrome in the semantic sense — it is the control saying "the cursor is
        // here" — so it takes the accent the theme assigns emphasis (`primary`) rather than the
        // text ink, and pushes it clear of the field. The glyph inside it is read off the caret
        // itself, so a light caret cannot carry a light glyph.
        let caret_color = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .or_else(|| {
                crate::style::resolved_theme_style("button").and_then(|b| b.background_color)
            })
            .unwrap_or_else(|| bg_color.contrast_color())
            .legible_on(bg_color, INK_MIN_RATIO);
        let caret_ink = caret_color.contrast_color();

        // ── Draw display text with mask placeholders ──
        //
        // Each character is positioned by advancing a fixed `char_width` rather than by the
        // font's reported advance. The advance is deliberately *not* the fitting budget here:
        // layout is computed through the SVG backend, which reports metrics at a DPI scale its
        // drawing coordinates are not expressed in — every `measure_text` under it came back
        // 1.75x the drawn size (a 13 px glyph measured 22.75). Fitting to a metric in a
        // different coordinate space than the coordinates being emitted is what let a field's
        // text be laid out from the centre line downward and leave the control. The segment
        // walk below therefore bounds each character by arithmetic in the painter's own space,
        // where its own `char_width` is the unit it advances by.
        // The field's horizontal content inset, from the shared table rather than a local
        // `6`: a masked field and every other field inset their content by the same amount,
        // so the value belongs in the table the others read.
        let padding = dimensions::TEXT_FIELD_PADDING_H as i32;
        let text_x = geom.x + padding;
        // The band a character may occupy. It starts below the field's top border and stops at
        // the field's bottom edge, because a character is drawn *down* from its origin: a 13 px
        // glyph placed on the vertical centre line at `geom.y + height / 2` reached
        // `geom.y + height`, which the P5 assertion reads as painting outside the control (the
        // SVG snapshot shows a `<text>` at `y = 60` in a 120 px field whose painted extent
        // reaches y = 73). Centring the *glyph box* instead of its centre line keeps the text
        // where it was while giving both branches below a bound they can be checked against.
        //
        // `context.text_line` is what places the box: it returns the field's own line box, so
        // the ink is vertically centred in the *field* rather than beginning on the point the
        // field happens to be centred on.
        let line = context.text_line(geom, &font);
        let inner_top = line.y;
        let inner_height = line.height;

        if self.mask.is_empty() {
            let text_color = if !is_enabled { disabled_ink } else { ink };
            // An unmasked field needs a string, so the census hands this one "Sample" — but so
            // does a real caller entering past the field's width, and the text was drawn at the
            // call site with no bound at all. Unlike the masked branch below, which walks one
            // character per segment and stops at the padding, there is no per-segment budget
            // here, so the whole string was emitted whatever the field could show. Bounding the
            // box is still worth doing for the vertical placement even though the run is laid
            // out from the painter's own geometry: the origin is the reason the text sat below
            // the field's inner area in the first place.
            let text_bounds = Rect::new(
                text_x,
                inner_top,
                geom.width.saturating_sub(padding as u32 * 2),
                inner_height,
            );
            // An empty value emits no text element at all. `draw_text_fitted` on an empty string
            // still reaches the backend, which emitted `<text ...></text>` — an element with no
            // content, which is neither a glyph nor a space and which nothing can consume. The
            // masked branch below already skips empty segments, so guarding here keeps the two
            // halves of the control consistent rather than leaving one of them emitting nothing
            // that looks like something.
            if !self.raw_text.is_empty() {
                context.draw_text_fitted(
                    text_bounds,
                    &self.raw_text,
                    &font,
                    text_color,
                    self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
                );
            }
            return;
        }

        // The horizontal alignment the field's content is painted with. Each character below is
        // positioned from its own cell, so a centred or right-aligned run needs the cells moved as
        // one block rather than per glyph: the run is measured in the painter's own space (one
        // `char_width` per segment) and the whole block is offset from the leading inset. Left is
        // the identity, so the common case is unchanged.
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        // One cell's advance in the painter's own space, which is the unit the segment walk below
        // advances by. Declared here because the alignment shift and the walk both read it.
        let char_width = 8u32;
        // The run the segments walk to the field's trailing edge, in the painter's own unit; the
        // number of character cells a mask can fill.
        let max_cells = (geom.width.saturating_sub(padding as u32 * 2)) / char_width;
        // Where the run begins. `Right`/`Center` hang the whole run off the trailing edge by the
        // cells it would fill, and a mask wider than the field is left as-is so the overflow is
        // clipped at the trailing edge rather than pushed off the leading one.
        let start_shift = if self.segments.len() as u32 <= max_cells {
            match value_align {
                HorizontalAlignment::Left => 0,
                HorizontalAlignment::Center => {
                    (max_cells - self.segments.len() as u32) as i32 / 2 * char_width as i32
                }
                HorizontalAlignment::Right => {
                    (max_cells - self.segments.len() as u32) as i32 * char_width as i32
                }
            }
        } else {
            0
        };

        // Draw each segment
        let mut raw_idx = 0;
        let mut display_x = text_x + start_shift;

        // ── The selection, behind the glyphs ──
        //
        // Painted as one band covering the *contiguous* run of segments the range spans, before the
        // segment walk writes any ink, so the characters stay legible on top of it. A band drawn per
        // segment after its glyph would cover that glyph, and a band drawn across the whole field
        // would highlight mask literals outside the range — both are visible mistakes, which is why
        // the band is placed from the range's own display positions rather than from the caret.
        let selection = self.selection_range();

        for (seg_idx, seg) in self.segments.iter().enumerate() {
            // A segment only starts inside the inner rectangle. With one character per segment
            // the cumulative test `< geom.width - padding * 2` is the whole bound.
            if display_x - text_x > geom.width as i32 - padding * 2 {
                break;
            }
            // What is left of the inner rectangle, in the painter's own coordinates. A zero or
            // negative remainder means the field is already full, and `draw_x` therefore skips
            // the segment instead of writing a character whose glyph box would begin at or past
            // the field's right edge and advance outside it.
            let remaining_w = geom.x + geom.width as i32 - padding - display_x;
            let draw_x = display_x.min(geom.x + geom.width as i32 - padding - 1);

            if remaining_w > 0 {
                // The selection band for this segment, if it is inside the range. Drawn from the same
                // `display_x`/`remaining_w` the glyph below uses, so the highlight and the character
                // it sits behind cannot drift apart — a second derivation of the cell's box is how a
                // selection ends up underlining the neighbouring character.
                if let Some((sel_start, sel_end)) = selection {
                    if seg_idx >= sel_start && seg_idx < sel_end {
                        context.fill_rect(
                            Rect::new(
                                draw_x,
                                geom.y + 2,
                                (char_width as i32).min(remaining_w).max(1) as u32,
                                geom.height.saturating_sub(4),
                            ),
                            self.selection_color(&ink, &bg_color),
                        );
                    }
                }
                match seg {
                    MaskSegment::Literal { ch } => {
                        context.draw_text(
                            Point::new(draw_x, inner_top),
                            &ch.to_string(),
                            &font,
                            mask_ink,
                            value_align,
                        );
                        display_x += char_width as i32;
                    }
                    MaskSegment::Input { kind } => {
                        // `raw_idx` counts input positions (**characters**), so it must index by
                        // character: `as_bytes()[raw_idx]` read the nth *byte* of the raw value
                        // and `as char` then produced the byte's Latin-1 glyph, so any multi-byte
                        // raw character painted as mojibake.
                        let ch = self
                            .raw_text
                            .chars()
                            .nth(raw_idx)
                            .unwrap_or_else(|| placeholder_char(*kind));
                        let has_input = raw_idx < self.raw_text.chars().count();

                        let char_color = if !is_enabled {
                            disabled_ink
                        } else if has_input {
                            ink
                        } else {
                            placeholder_ink
                        };

                        // Draw cursor if at this segment position
                        if self.focused && is_enabled && seg_idx == self.cursor_pos {
                            context.fill_rect(
                                Rect::new(
                                    draw_x,
                                    geom.y + 2,
                                    (char_width as i32).min(remaining_w) as u32,
                                    geom.height.saturating_sub(4),
                                ),
                                caret_color,
                            );
                            context.draw_text(
                                Point::new(draw_x, inner_top),
                                &ch.to_string(),
                                &font,
                                caret_ink,
                                value_align,
                            );
                        } else {
                            context.draw_text(
                                Point::new(draw_x, inner_top),
                                &ch.to_string(),
                                &font,
                                char_color,
                                value_align,
                            );
                        }

                        // `char_width` rather than the measured advance: it is the unit the
                        // layout above already advances by (the literal width this branch
                        // reserves for the cursor), and the measured advance is reported in a
                        // different coordinate space than `display_x`. Advancing by one and
                        // measuring in the other is what previously let the last cell start up
                        // to 22 px past where the layout believed it was.
                        display_x += char_width as i32;
                        if has_input {
                            raw_idx += 1;
                        }
                    }
                }
            }
        }
    }
}

impl EventHandler for MaskedEdit {
    fn handle_event(&mut self, event: &Event) {
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
            // A press focuses the field only when it lands on the **painted band**.
            //
            // Testing the control's rectangle is what let a user focus this field by
            // clicking the empty space below it: the field is a 48 px band centred in the
            // area the caller gave, so in a 120 px cell the bottom 36 px of the rectangle
            // are window background, not the control. Hit-testing the drawn box keeps the
            // clickable area equal to the visible one.
            //
            // A press also places the caret and anchors a selection, which it did not before:
            // the click only focused the field, so the caret stayed at position 0 and there was no
            // pointer selection at all.
            Event::MousePress { pos, button, modifiers, .. }
                if *button == 1 && self.field_rect().contains_point(*pos) =>
            {
                self.focused = true;
                let index = self.display_position_at_x(pos.x);
                self.press_at(index, crate::shortcut::Modifiers::from_event_bits(*modifiers));
            }
            Event::MouseMove { pos } => {
                if self.dragging_selection {
                    let index = self.display_position_at_x(pos.x);
                    self.drag_to(index);
                }
            }
            // A release anywhere ends the gesture, including outside the field: the anchor stays
            // where the press put it, so the selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.end_drag();
            }
            // Platform-committed text (D09-INPUT-01), routed through the mask entry point the
            // `KeyPress` printable arm uses. Without this branch a committed string never reached
            // the raw value. An unfocused field ignores it, matching the key path below.
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if self.focused {
                    self.insert_committed_text(text);
                }
            }
            Event::KeyPress { key, modifiers } => {
                if !self.focused {
                    return;
                }
                // The event carries the framework's wire bitmask; translate it once, here, so
                // nothing below re-derives a bit.
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                let primary = mods.contains(crate::shortcut::Modifiers::PRIMARY);

                if primary && *key == 65 {
                    // Primary+A: select all. Handled on the control that owns the text.
                    self.select_all();
                    return;
                }
                match *key {
                    8 => {
                        // Backspace — the character behind the caret, or the whole selection.
                        self.backspace();
                    }
                    127 => {
                        // Delete — the character at the caret, or the whole selection.
                        self.delete();
                    }
                    13 => {
                        // Enter — commit, no special action
                    }
                    27 => {
                        // Escape — lose focus
                        self.focused = false;
                        self.base.request_redraw();
                    }
                    37 => {
                        // Left — one display position back, extending under Shift. At position 0 the
                        // caret cannot move, but a plain press still owes the user a deselect.
                        if self.cursor_pos > 0 {
                            let target = self.cursor_pos - 1;
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                        }
                    }
                    39 => {
                        // Right — one display position forward; see the Left arm.
                        if self.cursor_pos < self.segments.len() {
                            let target = self.cursor_pos + 1;
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                        }
                    }
                    36 => {
                        // Home — the start of the field, or of the whole value with Primary held.
                        // A masked field is single-line, so both reach position 0; the distinction
                        // is kept so the chord is not silently swallowed as an unhandled key.
                        self.select_with_modifiers(0, mods);
                    }
                    35 => {
                        // End — the end of the value; see the Home arm.
                        self.select_with_modifiers(self.segments.len(), mods);
                    }
                    _ => {
                        // Printable character. A control chord is not text: `Primary+B` used to be
                        // inserted as a literal `b`, because the catch-all accepted every
                        // printable key regardless of the modifiers beside it.
                        //
                        // The three modifiers are tested one at a time on purpose:
                        // `Modifiers::contains` compares the *value* of the mask, so the combined
                        // `contains(CTRL | ALT | META)` form is true only when all three are held.
                        let chord = mods.contains(crate::shortcut::Modifiers::CTRL)
                            || mods.contains(crate::shortcut::Modifiers::ALT)
                            || mods.contains(crate::shortcut::Modifiers::META);
                        if !chord && *key >= 32 && *key < 127 {
                            if let Some(ch) = char::from_u32(*key) {
                                self.insert_char(ch);
                            }
                        }
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

// ── Private helpers ──

/// Returns the placeholder character for a mask kind when no input is present.
fn placeholder_char(kind: MaskCharKind) -> char {
    match kind {
        MaskCharKind::RequiredDigit | MaskCharKind::OptionalDigit => '_',
        MaskCharKind::RequiredLetter | MaskCharKind::OptionalLetter => '_',
        MaskCharKind::RequiredAlphaNum | MaskCharKind::OptionalAlphaNum => '_',
    }
}

/// Checks whether a character matches a mask character kind.
fn mask_char_matches(kind: MaskCharKind, ch: char) -> bool {
    match kind {
        MaskCharKind::RequiredDigit | MaskCharKind::OptionalDigit => ch.is_ascii_digit(),
        MaskCharKind::RequiredLetter | MaskCharKind::OptionalLetter => ch.is_ascii_alphabetic(),
        MaskCharKind::RequiredAlphaNum | MaskCharKind::OptionalAlphaNum => {
            ch.is_ascii_alphanumeric()
        }
    }
}

/// Parses a mask string into a vector of segments.
fn parse_mask(mask: &str) -> Vec<MaskSegment> {
    let mut segments = Vec::new();
    for ch in mask.chars() {
        let seg = match ch {
            '0' => MaskSegment::Input { kind: MaskCharKind::RequiredDigit },
            '9' => MaskSegment::Input { kind: MaskCharKind::OptionalDigit },
            'A' => MaskSegment::Input { kind: MaskCharKind::RequiredLetter },
            'a' => MaskSegment::Input { kind: MaskCharKind::OptionalLetter },
            'X' => MaskSegment::Input { kind: MaskCharKind::RequiredAlphaNum },
            'x' => MaskSegment::Input { kind: MaskCharKind::OptionalAlphaNum },
            _ => MaskSegment::Literal { ch },
        };
        segments.push(seg);
    }
    segments
}

/// Builds the display string from segments and raw text.
fn build_display_text(segments: &[MaskSegment], raw_text: &str) -> String {
    let mut result = String::new();
    let mut raw_idx = 0;
    let raw_chars: Vec<char> = raw_text.chars().collect();

    for seg in segments {
        match seg {
            MaskSegment::Literal { ch } => {
                result.push(*ch);
            }
            MaskSegment::Input { .. } => {
                if raw_idx < raw_chars.len() {
                    result.push(raw_chars[raw_idx]);
                    raw_idx += 1;
                } else {
                    result.push('_');
                }
            }
        }
    }
    result
}

// These tests drive the **theme**, which only exists in a build with a device profile
// (see `crate::lib`: `pub mod theme` is gated on `device_profile`). Without this gate the
// `mini` and `embedded` profiles fail to compile their test targets, because the test code
// names a module that those builds compile out — the production code is profile-clean and
// only the fixture was not.
#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    /// How far inside its glyph **box** a glyph's ink may begin and still count as starting on
    /// that box's own line.
    ///
    /// The bitmap face fills its whole glyph box (`font8x8` rectangles span the box), so a glyph's
    /// ink top *is* the box top, exactly. An **outline** face draws a real glyph whose ink is
    /// inset — a digit's cap does not quite reach the box's top edge — so the ink top sits one or
    /// two pixels below the box top at 13 px. Both are correct; the assertion must not encode
    /// which face drew the run, so it allows the inset and stays tight enough that a run placed a
    /// whole line low still fails.
    const INK_INSET_TOLERANCE: i32 = 3;

    #[test]
    fn masked_edit_default_creation() {
        let me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(me.mask(), "");
        assert_eq!(me.raw_text(), "");
        assert_eq!(me.text(), "");
        assert!(!me.is_valid());
        assert_eq!(me.cursor_pos(), 0);
    }

    #[test]
    fn masked_edit_set_mask() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("(000) 000-0000");
        assert_eq!(me.mask(), "(000) 000-0000");
    }

    #[test]
    fn masked_edit_set_text_validates_against_mask() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("000-0000");

        me.set_text("5551234");
        assert_eq!(me.raw_text(), "5551234");
        assert_eq!(me.text(), "555-1234");
        assert!(me.is_valid());
    }

    #[test]
    fn masked_edit_validity() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("0000");
        assert!(!me.is_valid());

        me.set_text("123");
        assert!(!me.is_valid());

        me.set_text("1234");
        assert!(me.is_valid());
    }

    #[test]
    fn masked_edit_insert_char() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("(000) 000-0000");

        me.insert_char('5');
        assert_eq!(me.raw_text(), "5");
        assert_eq!(me.text(), "(5__) ___-____");

        me.insert_char('5');
        me.insert_char('5');
        me.insert_char('1');
        me.insert_char('2');
        me.insert_char('3');
        me.insert_char('4');
        me.insert_char('5');
        me.insert_char('6');
        me.insert_char('7');
        assert_eq!(me.raw_text(), "5551234567");
        assert_eq!(me.text(), "(555) 123-4567");
    }

    #[test]
    fn masked_edit_invalid_char_rejected() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("000");

        // Letters should be rejected for digit mask
        me.insert_char('A');
        assert_eq!(me.raw_text(), "");
        assert_eq!(me.text(), "___");

        // Digits should be accepted
        me.insert_char('1');
        assert_eq!(me.raw_text(), "1");
    }

    #[test]
    fn masked_edit_cursor_position() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("000-0000");
        assert_eq!(me.cursor_pos(), 0);

        me.insert_char('1');
        // Cursor should be at segment position after the first input
        // mask "000-0000": segments at 0,1,2 (digits), 3 (-), 4,5,6,7 (digits)
        // After inserting '1' at segment 0 (first digit), cursor moves to segment 1
        // but since segment 1 is also a digit, cursor should be at segment position 1
        assert!(me.cursor_pos() > 0);

        me.set_cursor_pos(0);
        assert_eq!(me.cursor_pos(), 0);
    }

    #[test]
    fn masked_edit_text_changed_signal() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("0000");

        let captured = Arc::new(Mutex::new(None));
        let cap = captured.clone();
        me.text_changed.connect(move |val| {
            *cap.lock().unwrap() = Some(val.to_string());
        });

        me.insert_char('1');
        assert_eq!(captured.lock().unwrap().as_deref(), Some("1"));

        me.insert_char('2');
        assert_eq!(captured.lock().unwrap().as_deref(), Some("12"));
    }

    #[test]
    fn masked_edit_backspace() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("0000");
        me.set_text("1234");
        assert_eq!(me.raw_text(), "1234");

        // Backspace from end
        me.set_cursor_pos(4);
        me.backspace();
        assert_eq!(me.raw_text(), "123");
    }

    #[test]
    fn masked_edit_letter_mask() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("AAA");

        me.insert_char('H');
        me.insert_char('i');
        me.insert_char('!'); // should be rejected (not a letter)
        assert_eq!(me.raw_text(), "Hi");
    }

    #[test]
    fn masked_edit_svg_output() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("000-0000");
        me.set_text("5551234");

        let svg = render_to_svg(&mut me);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    /// The field is a full-width band one text-field height tall, centred in the control.
    ///
    /// The control painted its whole rectangle, so a 240x120 census cell drew a 240x120
    /// box — a panel rather than a field — and every anchor inside it was derived from the
    /// oversized box. This pins the shared `full_width_band` derivation, including the
    /// clamp for a control smaller than the field.
    #[test]
    fn the_field_is_a_text_field_height_in_any_rectangle() {
        for height in [48u32, 120, 300] {
            let me = MaskedEdit::new(Rect::new(0, 0, 240, height));
            let field = me.field_rect();
            assert_eq!(
                field.height,
                dimensions::TEXT_FIELD_MIN_HEIGHT,
                "at control height {height}"
            );
            assert_eq!(field.width, 240, "the field spans the control's width");
            assert_eq!(field.y, (height - field.height) as i32 / 2, "at control height {height}");
        }

        let short = MaskedEdit::new(Rect::new(0, 0, 240, 20));
        assert_eq!(short.field_rect().height, 20, "a short control clamps the field");
    }

    /// Hit-testing follows the ink: a press below the field does not focus it.
    ///
    /// With the field centred in a 120 px cell, a press inside the control's rectangle but
    /// well below the drawn band belongs to the window background. The control must not
    /// claim it. The flag is read directly because this control publishes no focus accessor;
    /// the assertion is on the state the handler sets, which is what the ring and the caret
    /// are drawn from.
    #[test]
    fn a_press_outside_the_drawn_band_does_not_focus_the_field() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
        let field = me.field_rect();

        me.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 / 2),
            button: 1,
            modifiers: 0,
        });
        assert!(me.focused, "a press on the drawn field focuses it");

        me.focused = false;
        me.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 + 40),
            button: 1,
            modifiers: 0,
        });
        assert!(!me.focused, "a press below the drawn field must not focus it");
    }

    /// One ink box per text `<path>` in the document, as `(left, top, right, bottom)`.
    ///
    /// # Why per element, not per merged run
    ///
    /// This control draws one character per `draw_text`, so each `<path>` is exactly one glyph's
    /// ink; the boundary is therefore the element, which is what makes "every glyph" expressible.
    /// The string is not in the document in any form (a run is glyph geometry), so a test locates
    /// ink by *where* it is rather than by what it says.
    ///
    /// # Why this delegates instead of parsing `d` itself
    ///
    /// The first form read the `font8x8` rectangle spelling directly (`M{x} {y}h{w}v{h}`), which
    /// is the **bitmap** path's grammar and only its grammar. An outline face writes
    /// `M18.30 59.24L...Z` with fractional vertices, so that reader took `18` as `x` and `30` as
    /// `y` — not ink, and not where the glyph is — and every box came back in the wrong place. The
    /// crate's own reader knows both grammars and the `data-text` provenance tag; one reader, so
    /// the two cannot drift apart.
    fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        crate::widget::svg::text_ink_boxes(svg)
    }

    /// The ink is vertically centred inside the **field**, not below its middle line.
    ///
    /// The old anchor was `geom.y + (geom.height - font.size) / 2`, derived from a
    /// rectangle that was already too tall. Drawing down from that origin put a 13 px glyph
    /// near the bottom of the field. The line box is now taken from the field itself.
    ///
    /// The check is on the emitted **ink**, not on a `<text y>` attribute: the backend now
    /// paints the `font8x8` rectangles the rasteriser fills, so the document holds a picture of
    /// the run rather than the run. Measuring the ink is also the stronger statement — a glyph
    /// drawn a line low with a correct attribute would have passed the old form.
    #[test]
    fn the_ink_is_vertically_centred_in_the_field() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
        me.set_mask("000-0000");
        me.set_text("5551234");
        let svg = render_to_svg(&mut me);
        let field = me.field_rect();

        let runs = text_run_boxes(&svg);
        assert!(!runs.is_empty(), "a filled masked field draws glyphs: {svg}");
        // Every character is placed on the field's own line box, so every glyph's ink has to sit
        // inside that box — and the topmost ink has to *reach* the box's top edge, because that is
        // where the glyph begins. Under the bitmap face the ink top *is* the box top; under an
        // outline face the glyph is inset a pixel or two (`INK_INSET_TOLERANCE`), so "reaches" is
        // stated as "within the inset" rather than as an equality, which is a property of the
        // layout rather than of the typeface. The box top is `field.y + (height - line) / 2`, the
        // shared line-box derivation rather than a copied literal, so this stays a statement about
        // the control. The defect was an origin below the field's middle line, which pushes the
        // whole box down by at least a whole line — far past the tolerance — and fails.
        let mut font_probe = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
        let line_h = crate::render::RenderContext::new(&mut font_probe)
            .measure_text("M", &Font::simple("monospace", 13.0))
            .height as i32;
        let expected_top = field.y + (field.height as i32 - line_h) / 2;
        let mut highest = i32::MAX;
        for (left, top, right, bottom) in runs {
            assert!(right > left, "a glyph painted ink: {left}..{right}");
            assert!(top >= expected_top, "glyph ink {top}..{bottom} starts on or below the box");
            assert!(
                bottom <= expected_top + line_h,
                "glyph ink {top}..{bottom} stays inside the one-line box ending at {}",
                expected_top + line_h
            );
            assert!(top >= field.y, "ink starts inside the field: top={top}, field={field:?}");
            assert!(
                top < field.y + field.height as i32,
                "ink starts above the field's bottom edge: top={top}, field={field:?}"
            );
            highest = highest.min(top);
        }
        assert!(
            highest - expected_top <= INK_INSET_TOLERANCE && highest >= expected_top,
            "the line box's top edge is where the ink begins: box top {expected_top}, ink top {highest}"
        );
    }

    /// The five chrome colours were fixed literals, so a themed field carried unthemed text:
    /// `rgb(33,33,33)` on the dark theme's `rgb(69,69,69)` field is ~1.35:1.
    ///
    /// The expectation is read off the rendered drawing rather than a copied-out colour: the body
    /// ink must move with the appearance, and it must actually be legible on the field the control
    /// painted behind it. Legibility is asserted with the crate's own `contrast_ratio`, so the
    /// test states the property the fix is for rather than the numbers it happened to produce.
    ///
    /// # Why the guard is held for the whole body, and the theme restored at the end
    ///
    /// The theme is process-wide and the guard is what serialises tests that switch it. A test
    /// that takes it once per render releases it between the two appearances, and another test
    /// running at that moment then switches the active theme underneath this one — the "dark" SVG
    /// comes back drawn in the light palette and the comparison below is meaningless. Holding it
    /// once around the whole body is what makes both renders observe the appearances they asked
    /// for.
    ///
    /// This test leaves the process on the **dark** theme, and it restores the light default
    /// before releasing the guard rather than leaving the choice to whichever test runs next: a
    /// theme left behind by a new test is exactly how an unrelated control's rendering assertion
    /// starts failing for a reason that has nothing to do with it.
    #[test]
    fn masked_edit_body_ink_is_legible_on_its_own_field() {
        let _guard = crate::style::theme_test_guard();
        // Whatever this test does, the process goes back to the light default on the way out.
        struct RestoreOnDrop;
        impl Drop for RestoreOnDrop {
            fn drop(&mut self) {
                crate::theme::global_theme_manager()
                    .set_appearance(crate::theme::AppearanceMode::Light);
            }
        }
        let _restore = RestoreOnDrop;

        fn render(appearance: crate::theme::AppearanceMode, text: &str) -> String {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
            me.set_text(text);
            render_to_svg(&mut me)
        }

        fn fills(svg: &str) -> Vec<String> {
            svg.lines()
                .filter_map(|l| {
                    l.split("fill=\"")
                        .nth(1)
                        .and_then(|rest| rest.split('"').next())
                        .map(|s| s.to_string())
                })
                .collect()
        }

        /// Parses the `rgba(r,g,b,a)` spelling the SVG backend emits.
        fn parse_rgba(value: &str) -> Option<Color> {
            let inner = value.strip_prefix("rgba(")?.strip_suffix(')')?;
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() < 3 {
                return None;
            }
            let channel = |s: &str| s.trim().parse::<f32>().ok().map(|v| v.round() as u8);
            Some(Color::rgba(
                channel(parts[0])?,
                channel(parts[1])?,
                channel(parts[2])?,
                parts
                    .get(3)
                    .and_then(|a| a.trim().parse::<f32>().ok())
                    .map_or(255, |a| (a * 255.0).round() as u8),
            ))
        }

        let dark = render(crate::theme::AppearanceMode::Dark, "Sample");
        let light = render(crate::theme::AppearanceMode::Light, "Sample");
        assert_ne!(dark, light, "the field's ink must respond to the appearance");

        // The text path's fill is the ink; the second `<rect>` fill is the field.
        //
        // Text is no longer a `<text>` element: the backend emits the same `font8x8` rectangles
        // the rasteriser fills, so the ink's colour is now the fill of a `<path>`. Only text is a
        // path in this backend — the field, the border and the caret are `<rect>`s — so the first
        // path carrying a fill is the body run.
        let ink = |svg: &str| -> Color {
            svg.lines()
                .find(|l| l.contains("<path ") && l.contains("fill="))
                .and_then(|l| l.split("fill=\"").nth(1))
                .and_then(|rest| rest.split('"').next())
                .and_then(parse_rgba)
                .expect("the body text must be drawn with a parseable fill")
        };
        let field = |svg: &str| -> Color {
            fills(svg)
                .iter()
                .skip(1)
                .find_map(|f| parse_rgba(f))
                .expect("the field background must be a parseable fill")
        };

        for (svg, name) in [(&dark, "dark"), (&light, "light")] {
            let ratio = ink(svg).contrast_ratio(field(svg));
            assert!(
                ratio >= 4.5,
                "{name}: body ink {:?} on field {:?} is only {ratio:.2}:1",
                ink(svg),
                field(svg)
            );
        }

        // A placeholder is deliberately weaker than an entered value, but never invisible.
        let with_mask = {
            crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);
            let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
            me.set_mask("000-0000");
            render_to_svg(&mut me)
        };
        // A placeholder must still be distinguishable from the field it sits on.
        //
        // The mask characters are separate glyph runs, so each is its own `<path>`; the ink is
        // read from the path's fill rather than from a `<text>` element, which no longer exists.
        let placeholders: Vec<Color> = with_mask
            .lines()
            .filter(|l| l.contains("<path "))
            .filter_map(|l| l.split("fill=\"").nth(1))
            .filter_map(|rest| rest.split('"').next())
            .filter_map(parse_rgba)
            .collect();
        assert!(!placeholders.is_empty(), "a masked field must render its mask characters");
        for colour in placeholders {
            let field_colour =
                fills(&with_mask).iter().skip(1).find_map(|f| parse_rgba(f)).expect("field fill");
            assert!(
                colour.contrast_ratio(field_colour) >= 1.5,
                "a mask character at {colour:?} is indistinguishable from its field"
            );
        }
    }

    // ─── Selection ───

    /// The framework's Shift bit on an event mask.
    const SHIFT_BIT: u32 = 0b0001;

    /// A focused field with a phone mask and the given raw digits typed in.
    fn phone(raw: &str) -> MaskedEdit {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 30));
        me.set_mask("(000) 000-0000");
        me.set_text(raw);
        me.focused = true;
        me
    }

    /// A Shift-arrow **extends**; a plain arrow replaces.
    ///
    /// # The defect this pins
    ///
    /// This control tracked only `cursor_pos`, so there was no way to select text at all: the
    /// arrows moved the caret and that was the whole of the keyboard model. The assertions are on
    /// the *range* rather than on the anchor, because a range is what the user sees — an
    /// implementation that moved the caret but forgot the anchor would still satisfy `cursor_pos`
    /// and select nothing.
    #[test]
    fn a_shift_arrow_extends_and_a_plain_arrow_replaces() {
        let mut me = phone("5551234567");
        me.set_cursor_pos(5);

        me.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(me.selection_range(), Some((4, 5)), "the first Shift+Left selects one cell");
        assert_eq!(me.cursor_pos(), 4);

        me.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(me.selection_range(), Some((3, 5)), "the second extends the same range");

        // A plain arrow drops the range but keeps the caret where the user moved it to.
        me.handle_event(&Event::key_press(37, 0));
        assert_eq!(me.selection_anchor(), None, "a plain arrow is not an extend");
        assert_eq!(me.selection_range(), None, "so nothing stays selected");
    }

    /// An extension keeps **one** anchor, so it is reversible.
    ///
    /// # The defect this pins
    ///
    /// Re-anchoring at the caret before every shift-movement is the tempting shortcut, and it makes
    /// the gesture irreversible: `Shift+Home` would leave the anchor at the start, so the following
    /// `Shift+End` grows from the wrong end. The anchor belongs to the whole gesture.
    #[test]
    fn extending_back_and_forth_returns_to_the_anchor() {
        let mut me = phone("5551234567");
        me.set_cursor_pos(6);

        me.handle_event(&Event::key_press(36, SHIFT_BIT));
        assert_eq!(me.selection_anchor(), Some(6), "the anchor is where the gesture began");
        assert_eq!(me.cursor_pos(), 0, "Home walks the caret to the start");
        assert_eq!(me.selection_range(), Some((0, 6)));

        me.handle_event(&Event::key_press(35, SHIFT_BIT));
        assert_eq!(me.selection_anchor(), Some(6), "and it did not move");
        // The mask is `(000) 000-0000` = 14 segments.
        assert_eq!(me.selection_range(), Some((6, 14)), "so the range is the other side of it");
    }

    /// Home and End reach the ends of the field.
    ///
    /// # The defect this pins
    ///
    /// Home and End were not handled at all, so they fell through to the printable-character arm
    /// where key 36 is `$` and 35 is `#` — pressing Home typed a dollar sign into the number.
    #[test]
    fn home_and_end_reach_the_ends_and_do_not_type() {
        let mut me = phone("5551234567");
        me.set_cursor_pos(4);
        let before = me.raw_text().to_string();

        me.handle_event(&Event::key_press(36, 0));
        assert_eq!(me.cursor_pos(), 0, "Home is the start of the field");
        me.handle_event(&Event::key_press(35, 0));
        assert_eq!(me.cursor_pos(), 14, "End is the end of the mask");
        assert_eq!(me.raw_text(), before, "and neither key typed a character");
    }

    /// Primary+A selects the whole value.
    #[test]
    fn primary_a_selects_the_whole_value() {
        let mut me = phone("5551234567");
        me.handle_event(&Event::key_press(65, 0b1000));
        assert_eq!(me.selection_range(), Some((0, 14)));
        assert_eq!(me.selection_raw_range(), Some((0, 10)), "the ten raw digits");
    }

    /// The selection translates to **raw** indices per end, not by subtracting literals.
    ///
    /// # The defect this pins
    ///
    /// A display position is not a raw index: the mask inserts `(`, `)`, ` ` and `-` between the
    /// digits, so display 5 is raw digit 2. A translation that divided or subtracted a literal count
    /// would land on the wrong digit, and the `delete`/`copy` paths would then disagree with the
    /// highlight the user is looking at.
    #[test]
    fn a_display_range_translates_to_the_matching_raw_digits() {
        let mut me = phone("5551234567");
        // Display positions 1..=3 are the three digits after `(`.
        me.set_cursor_pos(1);
        me.select_with_modifiers(4, crate::shortcut::Modifiers::SHIFT);
        assert_eq!(me.selection_range(), Some((1, 4)), "three display cells");
        assert_eq!(me.selection_raw_range(), Some((0, 3)), "which are the first three digits");

        // And a range that starts after the literals: display 6 is the first digit of the second
        // group (`(555) 1` has its `1` at display 6), so it maps to raw digit 3.
        me.set_cursor_pos(6);
        me.select_with_modifiers(9, crate::shortcut::Modifiers::SHIFT);
        assert_eq!(me.selection_raw_range(), Some((3, 6)), "the second group of three");
    }

    /// Typing over a selection replaces it — the operation that makes selecting worth having.
    #[test]
    fn typing_over_a_selection_replaces_it() {
        let mut me = phone("5551234567");
        me.set_cursor_pos(1);
        me.select_with_modifiers(4, crate::shortcut::Modifiers::SHIFT);
        assert_eq!(me.selection_raw_range(), Some((0, 3)));

        me.handle_event(&Event::key_press(57, 0)); // '9'
        assert_eq!(me.raw_text(), "91234567", "the selected digits were consumed");
        assert_eq!(me.selection_range(), None, "and the range it described is gone");
        // The mask is re-applied to the shorter raw value, so the tail reverts to placeholders.
        assert_eq!(me.text(), "(912) 345-67__", "the mask was re-applied to the shorter value");
    }

    /// Backspace and Delete both remove the whole selection rather than one character of it.
    #[test]
    fn backspace_and_delete_consume_the_selection() {
        for key in [8u32, 127] {
            let mut me = phone("5551234567");
            me.set_cursor_pos(1);
            me.select_with_modifiers(4, crate::shortcut::Modifiers::SHIFT);
            me.handle_event(&Event::key_press(key, 0));
            assert_eq!(me.raw_text(), "1234567", "key {key} removed the three selected digits");
            assert_eq!(me.selection_range(), None, "key {key} cleared the range");
        }
    }

    /// A control chord is not text: `Primary+b` must not be typed into the field.
    ///
    /// # The defect this pins
    ///
    /// The printable-character arm accepted every key in `32..127` regardless of the modifiers
    /// beside it, so `Primary+B`, `Alt+1` and any other host accelerator were typed into the value.
    #[test]
    fn a_control_chord_is_not_typed_into_the_field() {
        let mut me = phone("");
        // `set_text` leaves the caret at the end of the mask, so it is placed at the start — a chord
        // tested at a full caret would pass for the wrong reason (`insert_char` refuses there).
        me.set_cursor_pos(1);
        for (name, bits) in [("Ctrl", 0b0010u32), ("Alt", 0b0100), ("Primary", 0b1000)] {
            me.handle_event(&Event::key_press(53, bits)); // '5'
            assert!(me.raw_text().is_empty(), "{name}+5 must not be typed");
        }
        // The same key with no modifier is content, so the guard did not disable typing.
        me.handle_event(&Event::key_press(53, 0));
        assert_eq!(me.raw_text(), "5");
    }

    /// A programmatic `set_cursor_pos` drops the selection; the `Shift` path keeps it.
    #[test]
    fn set_cursor_pos_drops_the_selection() {
        let mut me = phone("5551234567");
        me.set_cursor_pos(1);
        me.select_with_modifiers(4, crate::shortcut::Modifiers::SHIFT);
        assert!(me.selection_range().is_some());

        me.set_cursor_pos(2);
        assert_eq!(me.selection_range(), None, "placing the caret is not extending a range");
    }

    /// Nothing selected paints no band, so the highlight is additive.
    #[test]
    fn a_caret_without_a_selection_paints_no_band() {
        // One field, rendered twice, so the caret's own state is identical across the two frames and
        // only the selection can account for a difference.
        let mut me = phone("5551234567");
        let _theme_guard = crate::style::theme_test_guard();
        me.set_cursor_pos(3);
        assert_eq!(me.selection_range(), None, "a caret alone is not a selection");
        let with_caret = render_to_svg(&mut me);

        me.clear_selection();
        let after = render_to_svg(&mut me);
        assert_eq!(with_caret, after, "clearing a selection that was not there changed nothing");
    }

    /// The selection is painted, and it does not move or erase the characters it covers.
    #[test]
    fn the_selection_is_painted_without_losing_the_digits() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut me = phone("5551234567");
        let plain = render_to_svg(&mut me);

        me.set_cursor_pos(1);
        me.select_with_modifiers(4, crate::shortcut::Modifiers::SHIFT);
        let selected = render_to_svg(&mut me);

        assert_ne!(plain, selected, "a selection must be visible");
        // The glyphs are still emitted: the band sits *behind* them. Losing the digits would mean the
        // band had been drawn over the ink instead of under it.
        assert_eq!(
            crate::widget::svg::text_ink_box(&selected),
            crate::widget::svg::text_ink_box(&plain),
            "and the value's ink is where it was"
        );
    }

    // ─── D09-INPUT-01: platform-committed text reaches the raw value ───

    /// A `TextInput` from the platform must be fed through the mask.
    ///
    /// # The defect this pins (D09-INPUT-01)
    ///
    /// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
    /// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
    /// test feeds `TextInput` to prove that path itself is wired.
    #[test]
    fn text_input_enters_the_raw_value() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("000-0000");
        me.focused = true;
        me.handle_event(&Event::TextInput { text: "123".to_string() });
        assert_eq!(me.raw_text(), "123", "each committed character filled a slot");
        assert_eq!(me.text(), "123-____");
    }

    /// An IME commit is fed through the mask one character at a time.
    #[test]
    fn ime_commit_enters_the_raw_value() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("0000");
        me.focused = true;
        me.handle_event(&Event::ime_commit("9876"));
        assert_eq!(me.raw_text(), "9876");
    }

    /// A committed character that does not match the mask is rejected, exactly as a keystroke is.
    #[test]
    fn text_input_rejects_characters_the_mask_forbids() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("000");
        me.focused = true;
        me.handle_event(&Event::TextInput { text: "abc".to_string() });
        assert_eq!(me.raw_text(), "", "letters do not match a digit mask");
    }

    /// An unfocused field ignores committed text, matching the key gate.
    #[test]
    fn unfocused_ignores_text_input() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("000");
        me.handle_event(&Event::TextInput { text: "1".to_string() });
        assert_eq!(me.raw_text(), "");
    }

    /// Committed text emits `text_changed` once with the raw value.
    #[test]
    fn text_input_emits_text_changed() {
        let captured = Arc::new(Mutex::new(None));
        let mut me = MaskedEdit::new(Rect::new(0, 0, 200, 30));
        me.set_mask("0000");
        me.focused = true;
        let cap = captured.clone();
        me.text_changed.connect(move |val| {
            *cap.lock().unwrap() = Some(val.to_string());
        });
        me.handle_event(&Event::TextInput { text: "12".to_string() });
        assert_eq!(*captured.lock().unwrap(), Some("12".to_string()));
    }
}
