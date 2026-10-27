// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mention — `@`-triggered candidate completion inside free text.
//!
//! # Why this is not an `AutoCompleteEdit` mode
//!
//! `AutoCompleteEdit` matches its suggestions against **the whole text**
//! (`auto_complete_edit.rs`: `text: String`, `filter_suggestions()` over the entire
//! field) and replaces the whole value on selection. A mention is a different
//! model:
//!
//! | | `AutoCompleteEdit` | `Mention` |
//! |---|---|---|
//! | matched against | the whole field text | **the token before the caret** |
//! | trigger | typing anything | **one trigger character** (`@`) |
//! | what a choice replaces | the entire text | **only the active token** |
//! | caret after a choice | end of the field | **after the inserted mention** |
//! | more than one | impossible | **many mentions in one field** |
//!
//! The last row is the one that makes them genuinely different types: a mention
//! field holds *several* completed references plus surrounding prose, so the value
//! is not "the text" but a text with recognised spans inside it. Trying to express
//! that through one `text: String` and a whole-field replacement would mean the
//! caller re-implementing the token logic outside the control.
//!
//! # What it shares rather than reimplements
//!
//! The suggestion list, the highlighted index, the up/down/enter/Escape keyboard
//! contract and the popup's drawing follow `AutoCompleteEdit`'s conventions, so a
//! caller switching between them does not relearn the interaction.

//! # Reachability
//!
//! Registered in the widget factory as `mention` (aliases `mention_edit`,
//! `at_mention`), so it is reachable by name from the declarative JSON path
//! (`"mention"`), from a CSS selector (`Mention`), and through the typed
//! `create_mention` on `ControlBackend`.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{expect_string, expect_u32_saturating, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// A person who can be mentioned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionCandidate {
    /// Stable identifier; what a completed mention resolves to.
    pub id: String,
    /// The text drawn in the candidate list and inserted into the field.
    pub display: String,
    /// An optional secondary line, e.g. a role or address.
    pub description: String,
}

/// The declared shape of one mention candidate, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `candidates` property, and the control owns both ends of it: the
/// field names are [`MentionCandidate`]'s own fields. `id` is required because it is what a
/// completed mention resolves to, `display` because it is what the popup draws and inserts;
/// `description` is optional because a candidate with no secondary line is the normal case.
pub const MENTION_CANDIDATE_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "display",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "description",
                crate::widget::capability::PropertyValueKind::String,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`MENTION_CANDIDATE_SCHEMA`].
fn mention_candidate_record(
    candidate: &MentionCandidate,
) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(candidate.id.clone()))
        .with("display", CapabilityValue::String(candidate.display.clone()));
    if !candidate.description.is_empty() {
        record = record.with("description", CapabilityValue::String(candidate.description.clone()));
    }
    record
}

/// Rebuilds one [`MentionCandidate`] from a row of [`MENTION_CANDIDATE_SCHEMA`].
fn mention_candidate_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<MentionCandidate, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let description = match record.get("description") {
        None | Some(CapabilityValue::Null) => String::new(),
        Some(CapabilityValue::String(description)) => description.clone(),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(MentionCandidate { id: text("id")?, display: text("display")?, description })
}

impl MentionCandidate {
    /// Creates a candidate with `id` and `display`, and no description.
    pub fn new(id: impl Into<String>, display: impl Into<String>) -> Self {
        Self { id: id.into(), display: display.into(), description: String::new() }
    }

    /// Sets the secondary line.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Whether this candidate matches the typed query, case-insensitively.
    pub fn matches(&self, query: &str) -> bool {
        self.match_rank(query).is_some()
    }

    /// How well this candidate matches `query`, lower being better (I-130).
    ///
    /// A display-prefix match ranks above a prefix on the id, which ranks above a
    /// substring anywhere. `None` means no match. Sorting by this rank keeps the
    /// declaration order among equally good matches.
    fn match_rank(&self, query: &str) -> Option<u8> {
        if query.is_empty() {
            return Some(0);
        }
        let query = query.to_lowercase();
        use crate::widget::text_utils::{
            contains_ignore_ascii_case, starts_with_ignore_ascii_case,
        };
        if starts_with_ignore_ascii_case(&self.display, &query) {
            return Some(0);
        }
        if starts_with_ignore_ascii_case(&self.id, &query) {
            return Some(1);
        }
        if contains_ignore_ascii_case(&self.display, &query)
            || contains_ignore_ascii_case(&self.id, &query)
            || contains_ignore_ascii_case(&self.description, &query)
        {
            return Some(2);
        }
        None
    }
}

/// One completed mention inside the field's text.
///
/// Recorded so a caller can read back *which* references the text contains without
/// re-parsing it — and so the control can keep the spans consistent when the text
/// is edited around them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedMention {
    /// The candidate's id.
    pub id: String,
    /// Byte offset in [`Mention::text`] where the mention starts.
    pub start: usize,
    /// Byte offset just past the mention.
    pub end: usize,
}

/// The height of one candidate row.
const ROW_HEIGHT: u32 = 24;

/// How many candidates the popup shows at once.
const MAX_VISIBLE: usize = 6;

/// The height the caret is drawn with, and its x offset inside the field.
const CARET_HEIGHT: u32 = 16;

/// Per-character advances of the field's text, memoised per revision (I-107).
///
/// `byte_offsets[i]` is the byte index where character `i` starts and `widths[i]` is the
/// advance of everything before it; the final entry is the text's end. Any byte offset is
/// therefore answered by binary search, and the expensive per-character measurement happens
/// once per edit rather than once per frame.
struct TextWidthCache {
    revision: u64,
    font: Font,
    dpi_bits: u32,
    byte_offsets: Vec<usize>,
    widths: Vec<u32>,
}

impl TextWidthCache {
    /// The advance before `offset`, clamped to the text.
    fn width_at(&self, offset: usize) -> u32 {
        let index = self.byte_offsets.partition_point(|start| *start <= offset).saturating_sub(1);
        self.widths.get(index).copied().unwrap_or(0)
    }
}

/// Mention — `@`-triggered completion inside free text.
pub struct Mention {
    base: BaseWidget,
    /// The field's text.
    text: String,
    /// The caret's byte offset into `text`.
    caret: usize,
    /// The character that starts a mention.
    trigger: char,
    /// Every candidate, in declaration order.
    candidates: Vec<MentionCandidate>,
    /// Candidate indices the popup is currently offering, in order.
    filtered: Vec<usize>,
    /// The highlighted candidate, as an index into `filtered`.
    highlighted: Option<usize>,
    /// The byte offset of the trigger that opened the current mention.
    ///
    /// Retained rather than recomputed so the replacement span is exactly what the
    /// user is looking at, even if the caret has moved within the token.
    trigger_offset: Option<usize>,
    /// The mentions currently present in `text`.
    completed: Vec<CompletedMention>,
    /// Emitted when text changes, with the new text.
    pub text_changed: Signal1<String>,
    /// Emitted when a mention is completed, with the candidate's id.
    pub mention_inserted: Signal1<String>,
    /// Emitted when the popup opens or closes, with the new state.
    pub popup_toggled: Signal1<bool>,
    /// How many candidate rows the popup shows at once.
    max_visible: usize,
    /// The height of one candidate row, in logical pixels.
    row_height: u32,
    /// The other end of the pointer selection, or `None` when nothing is selected.
    selection_anchor: Option<usize>,
    /// Whether a pointer drag is extending the selection.
    dragging_selection: bool,
    /// The first candidate row the popup paints, so candidates beyond `max_visible` are
    /// reachable (I-102).
    scroll_offset: usize,
    /// The host's available popup area, used to flip the popup upward when there is not enough
    /// room below (I-102). `None` leaves the popup below the field.
    popup_viewport: Option<Rect>,
    /// Whether the popup was flipped above the field for the current layout (I-102).
    popup_flipped: bool,
    /// The caret's blink state (I-101).
    cursor_blink: crate::style::CursorBlink,
    /// Whether the field owns focus, which starts and stops the caret blink (I-101).
    focused: bool,
    /// Bumped on every text mutation, so the width cache invalidates (I-107).
    text_revision: u64,
    /// Per-character advance cache, keyed by the text revision, font and scale (I-107).
    width_cache: RefCell<Option<TextWidthCache>>,
    /// The earliest byte offset the completed-mention table must rescan (I-106).
    ///
    /// A text edit sets it to the edit's start; a caret-only refresh sets it past the
    /// end of the text, which keeps every existing mention without scanning.
    dirty_from: usize,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl Mention {
    /// Creates an empty mention field.
    ///
    /// Defaults: empty text, caret at 0, the `@` trigger, no candidates, popup closed.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Mention, geometry, "Mention"),
            text: String::new(),
            caret: 0,
            trigger: '@',
            candidates: Vec::new(),
            filtered: Vec::new(),
            highlighted: None,
            trigger_offset: None,
            completed: Vec::new(),
            text_changed: Signal1::new(),
            mention_inserted: Signal1::new(),
            popup_toggled: Signal1::new(),
            max_visible: MAX_VISIBLE,
            row_height: ROW_HEIGHT,
            selection_anchor: None,
            dragging_selection: false,
            scroll_offset: 0,
            popup_viewport: None,
            popup_flipped: false,
            cursor_blink: crate::style::CursorBlink::new(),
            focused: false,
            text_revision: 0,
            width_cache: RefCell::new(None),
            dirty_from: 0,
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }

    /// The field's text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret's byte offset.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The trigger character.
    pub fn trigger(&self) -> char {
        self.trigger
    }

    /// Sets the trigger character.
    ///
    /// A whitespace trigger is refused: the token walker stops at whitespace, so a
    /// space could never start a token and the popup would never open — a trigger
    /// that cannot trigger is worse than a rejected call.
    pub fn set_trigger(&mut self, trigger: char) -> bool {
        if trigger.is_whitespace() {
            return false;
        }
        self.trigger = trigger;
        self.dirty_from = 0;
        self.refresh();
        self.base.request_redraw();
        true
    }

    /// Replaces the candidate list.
    ///
    /// `completed` is re-derived from the new list, so a mention whose candidate no
    /// longer exists stops being reported as a mention rather than lingering as an
    /// id nothing can resolve.
    pub fn set_candidates(&mut self, candidates: Vec<MentionCandidate>) {
        self.candidates = candidates;
        self.dirty_from = 0;
        self.refresh();
        let known: Vec<&str> = self.candidates.iter().map(|c| c.id.as_str()).collect();
        self.completed.retain(|mention| known.contains(&mention.id.as_str()));
        self.base.request_redraw();
    }

    /// The candidate list.
    pub fn candidates(&self) -> &[MentionCandidate] {
        &self.candidates
    }

    /// The mentions present in the text.
    pub fn completed_mentions(&self) -> &[CompletedMention] {
        &self.completed
    }

    /// Whether the candidate popup is showing.
    pub fn is_popup_open(&self) -> bool {
        self.trigger_offset.is_some()
    }

    /// The candidates the popup is offering.
    pub fn visible_candidates(&self) -> Vec<&MentionCandidate> {
        self.filtered.iter().filter_map(|index| self.candidates.get(*index)).collect()
    }

    /// The highlighted candidate's index within [`Self::visible_candidates`].
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// The query currently being completed, i.e. the text after the trigger.
    ///
    /// `None` when no popup is open, so a caller cannot mistake the whole field for
    /// a query.
    pub fn query(&self) -> Option<&str> {
        let start = self.trigger_offset?;
        // The token runs from just past the trigger to the caret.
        self.text.get(start + self.trigger.len_utf8()..self.caret)
    }

    /// Replaces the text and re-derives the mention state.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        let before = self.text.clone();
        self.text = text;
        self.record_edit(before);
        self.caret = self.text.len();
        self.selection_anchor = None;
        self.dirty_from = 0;
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// The maximum number of candidate rows the popup shows at once.
    pub fn max_visible(&self) -> usize {
        self.max_visible
    }

    /// Sets the visible-row count, clamped to at least one.
    pub fn set_max_visible(&mut self, rows: usize) {
        let rows = rows.max(1);
        if self.max_visible == rows {
            return;
        }
        self.max_visible = rows;
        self.base.request_redraw();
    }

    /// The height of one candidate row, in logical pixels.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the candidate row height, clamped to a drawable band.
    pub fn set_row_height(&mut self, height: u32) {
        let height = height.clamp(8, 96);
        if self.row_height == height {
            return;
        }
        self.row_height = height;
        self.base.request_redraw();
    }

    /// The selected byte range of the field's text, if any.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        if anchor == self.caret {
            return None;
        }
        let (start, end) =
            if anchor < self.caret { (anchor, self.caret) } else { (self.caret, anchor) };
        Some((start.min(self.text.len()), end.min(self.text.len())))
    }

    /// Selects the whole field's text.
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.caret = self.text.len();
        self.base.request_redraw();
    }

    /// Drops the selection, leaving the caret where it is.
    pub fn clear_selection(&mut self) {
        if self.selection_anchor.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// The byte offset a horizontal position points at.
    ///
    /// The prefix is measured with the shared metric model the paint path uses, so a click lands
    /// on the character the pointer is over rather than at a count-derived guess.
    fn byte_index_at_x(&self, x: i32) -> usize {
        let font = self.field_font();
        let target = (x - (self.geometry().x + 8)).max(0) as u32;
        // I-131: the click reads the same renderer-derived advance table the painter uses.
        let widths = Self::with_measurement(|context| {
            self.text_widths(&font, 1.0f32.to_bits(), &mut |text| {
                context.measure_text(text, &font).width
            })
        });
        let mut best = 0usize;
        for index in 0..widths.widths.len().saturating_sub(1) {
            let next_start = widths.byte_offsets[index + 1];
            let advance = widths.widths[index + 1].saturating_sub(widths.widths[index]);
            if target < widths.widths[index] + advance / 2 {
                break;
            }
            best = next_start;
        }
        best.min(self.text.len())
    }

    /// Inserts `ch` at the caret and moves the caret past it.
    pub fn insert_char(&mut self, ch: char) {
        let mut glyph = [0u8; 4];
        self.insert_str(ch.encode_utf8(&mut glyph));
    }

    /// Inserts a whole run at the caret in one refresh and one signal (I-P66).
    ///
    /// A multi-character `TextInput`/IME commit used to loop over `insert_char`, doing one
    /// `refresh`, one whole-text clone and one signal per character.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.caret > self.text.len() || !self.text.is_char_boundary(self.caret) {
            // A caret that is not on a character boundary cannot be spliced; moving it
            // to the end is the only safe repair, and it is reported rather than done
            // silently by a panic.
            self.caret = self.text.len();
        }
        let before = self.text.clone();
        let insert_at = self.caret;
        self.text.insert_str(self.caret, text);
        self.caret += text.len();
        self.selection_anchor = None;
        self.record_edit(before);
        self.dirty_from = insert_at;
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Files one undo step for a text mutation (I-101).
    fn record_edit(&mut self, before: String) {
        if self.restoring_history {
            return;
        }
        self.text_revision = self.text_revision.wrapping_add(1);
        *self.history_target.borrow_mut() = self.text.clone();
        self.undo_stack.push(Box::new(ValueCommand::new(
            self.history_target.clone(),
            before,
            self.text.clone(),
            "Edit text",
            "mention_text",
        )));
    }

    /// Reverts the latest text mutation (I-101).
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the latest undone mutation (I-101).
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.caret = self.text.len();
        self.selection_anchor = None;
        self.restoring_history = false;
        self.text_revision = self.text_revision.wrapping_add(1);
        self.dirty_from = 0;
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Copies the selected text to the platform clipboard (I-101).
    #[cfg(not(alloc_frugal))]
    fn copy_selection_to_clipboard(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let selected = self.text[start..end].to_string();
        if selected.is_empty() {
            return None;
        }
        crate::set_clipboard_text(&selected);
        Some(selected)
    }

    #[cfg(alloc_frugal)]
    fn copy_selection_to_clipboard(&self) -> Option<String> {
        None
    }

    /// Inserts the clipboard's text at the caret (I-101).
    #[cfg(not(alloc_frugal))]
    fn paste_from_clipboard(&mut self) {
        let text = crate::get_clipboard_text();
        if !text.is_empty() {
            self.insert_str(&text);
        }
    }

    #[cfg(alloc_frugal)]
    fn paste_from_clipboard(&mut self) {}

    /// Removes the selected range, if any (I-101).
    fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };
        let before = self.text.clone();
        self.text.replace_range(start..end, "");
        self.caret = start;
        self.selection_anchor = None;
        self.record_edit(before);
        self.dirty_from = start;
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
        true
    }

    /// Removes the character before the caret.
    ///
    /// Returns whether anything was removed.
    pub fn backspace(&mut self) -> bool {
        if self.caret == 0 || !self.text.is_char_boundary(self.caret) {
            return false;
        }
        let previous = self.previous_char_boundary(self.caret);
        let before = self.text.clone();
        self.text.replace_range(previous..self.caret, "");
        self.caret = previous;
        self.selection_anchor = None;
        self.record_edit(before);
        self.dirty_from = previous;
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
        true
    }

    /// The byte offset of the character boundary before `index`.
    ///
    /// Delegates to the crate's shared [`floor_char_boundary`]: the walk is identical, and this
    /// file already imports the byte/character helpers for `insert_char`. A second copy of the
    /// same primitive is a second place for it to be wrong.
    /// The byte offset of the character before `index`.
    ///
    /// Delegates to the shared [`crate::widget::text_utils::previous_char_boundary`]: the local copy
    /// stepped back a byte and floored, which agreed with the other controls' scanning version but
    /// was a fourth spelling of it (WDGT-11).
    fn previous_char_boundary(&self, index: usize) -> usize {
        crate::widget::text_utils::previous_char_boundary(&self.text, index)
    }

    /// Moves the caret one character left.
    pub fn move_caret_left(&mut self) {
        if self.caret > 0 {
            self.caret = self.previous_char_boundary(self.caret);
            self.dirty_from = usize::MAX;
            self.refresh();
            self.base.request_redraw();
        }
    }

    /// Moves the caret one character right.
    pub fn move_caret_right(&mut self) {
        if self.caret < self.text.len() {
            let mut cursor = self.caret + 1;
            while cursor < self.text.len() && !self.text.is_char_boundary(cursor) {
                cursor += 1;
            }
            self.caret = cursor;
            self.dirty_from = usize::MAX;
            self.refresh();
            self.base.request_redraw();
        }
    }

    /// Completes the mention with `candidate_index`, replacing the active token.
    ///
    /// Returns the candidate's id. When no popup is open, or the index names no
    /// visible candidate, nothing changes and `None` is returned — a completion the
    /// user did not ask for would corrupt prose.
    pub fn complete(&mut self, candidate_index: usize) -> Option<String> {
        let start = self.trigger_offset?;
        let candidate_id = {
            let index = *self.filtered.get(candidate_index)?;
            self.candidates.get(index)?.id.clone()
        };
        let display = self
            .filtered
            .get(candidate_index)
            .and_then(|index| self.candidates.get(*index))
            .map(|candidate| candidate.display.clone())?;

        // Replace from the trigger to the caret: the trigger itself is consumed, which
        // is the convention every mention UI uses (`@al` becomes `@Alice`, not
        // `@@Alice`).
        let replaced = format!("{}{}", self.trigger, display);
        let before = self.text.clone();
        self.text.replace_range(start..self.caret, &replaced);
        self.record_edit(before);
        self.selection_anchor = None;
        self.caret = start + replaced.len();
        self.trigger_offset = None;
        self.filtered.clear();
        self.highlighted = None;
        self.rebuild_completed_from(start);
        self.text_changed.emit(self.text.clone());
        self.mention_inserted.emit(candidate_id.clone());
        self.popup_toggled.emit(false);
        self.base.request_redraw();
        Some(candidate_id)
    }

    /// Opens the popup at the caret, as typing the trigger does.
    ///
    /// Returns whether it opened. A caller with a toolbar button needs the same
    /// entry point the keyboard has.
    pub fn open_popup(&mut self) -> bool {
        self.trigger_offset = None;
        // Insert the trigger, then treat it exactly as typed input would.
        self.insert_char(self.trigger);
        self.is_popup_open()
    }

    /// Closes the popup without completing.
    pub fn close_popup(&mut self) {
        if self.trigger_offset.take().is_some() {
            self.filtered.clear();
            self.highlighted = None;
            self.popup_toggled.emit(false);
            self.base.request_redraw();
        }
    }

    /// Moves the popup's highlight by `delta`, wrapping at both ends.
    fn move_highlight(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            self.highlighted = None;
            return;
        }
        let count = self.filtered.len() as i32;
        let next = match self.highlighted {
            Some(current) => (current as i32 + delta).rem_euclid(count),
            // Nothing highlighted yet: a forward move enters at the start, a backward
            // one at the end. Offsetting from an assumed 0 would skip the first row.
            None if delta >= 0 => 0,
            None => count - 1,
        };
        self.highlighted = Some(next as usize);
        // I-102: the visible window follows the highlight, so a candidate beyond `max_visible`
        // comes into view when the arrows reach it.
        self.scroll_to_highlight();
        self.base.request_redraw();
    }

    /// Re-derives the active token, the visible candidates and the completed mentions.
    ///
    /// The single place that reads the text, so the popup's contents, the
    /// replacement span and the reported mentions cannot disagree.
    ///
    /// `popup_toggled` fires only on an actual **change** of openness, so a caller
    /// can drive a show/hide animation from it. Emitting on every keystroke while
    /// the popup stayed open would make the signal a keystroke counter rather than a
    /// state-change notification.
    fn refresh(&mut self) {
        // The completed-mention table is maintained incrementally by
        // `rebuild_completed_from`; clearing it here would throw away the prefix an edit
        // did not touch (I-106).
        // I-102: a new filter starts at the top of its own candidate list.
        self.scroll_offset = 0;
        let was_open = self.trigger_offset.is_some();
        let token = self.active_token();
        match token {
            Some((trigger_offset, query_start)) => {
                self.trigger_offset = Some(trigger_offset);
                let query = self.text.get(query_start..self.caret).unwrap_or("");
                // Prefix matches rank above substring matches; the sort is stable, so
                // candidates of equal rank keep their declaration order (I-130).
                let mut matches: Vec<(usize, u8)> = self
                    .candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        candidate.match_rank(query).map(|rank| (index, rank))
                    })
                    .collect();
                matches.sort_by_key(|(_, rank)| *rank);
                self.filtered = matches.into_iter().map(|(index, _)| index).collect();
                if self.filtered.is_empty() {
                    // An empty popup reads as a broken control, so it closes — and
                    // that is a state change, reported as one.
                    self.trigger_offset = None;
                    self.highlighted = None;
                } else {
                    // The highlight resets to the first candidate on every edit, so it
                    // always points at the top of what the user currently sees.
                    self.highlighted = Some(0);
                }
            }
            None => {
                self.trigger_offset = None;
                self.filtered.clear();
                self.highlighted = None;
            }
        }
        let is_open = self.trigger_offset.is_some();
        if is_open != was_open {
            self.popup_toggled.emit(is_open);
        }
        let dirty = core::mem::replace(&mut self.dirty_from, usize::MAX);
        self.rebuild_completed_from(dirty);
    }

    /// The trigger that opens the token containing the caret, if any.
    ///
    /// Returns `(trigger_offset, query_start)`. Walks backwards from the caret to the
    /// nearest trigger, stopping at whitespace: a token cannot contain a space, so a
    /// trigger before one is a *finished* mention and must not re-open the popup.
    fn active_token(&self) -> Option<(usize, usize)> {
        let before_caret = self.text.get(..self.caret)?;
        // Find the last trigger before the caret.
        let trigger_offset = before_caret.rfind(self.trigger)?;
        let after_trigger = &before_caret[trigger_offset + self.trigger.len_utf8()..];
        // A whitespace between the trigger and the caret ends the token.
        if after_trigger.contains(char::is_whitespace) {
            return None;
        }
        // A trigger that is itself preceded by a non-space, non-trigger character
        // would make `foo@bar` a mention, which is an email address, not a mention.
        // Only a trigger at the start or after whitespace opens the popup.
        if trigger_offset > 0 {
            let preceding = before_caret[..trigger_offset].chars().next_back();
            if !preceding.is_some_and(char::is_whitespace) {
                return None;
            }
        }
        Some((trigger_offset, trigger_offset + self.trigger.len_utf8()))
    }

    /// Rebuilds the completed-mention list from the text.
    ///
    /// Scans for `trigger` + a display string that matches a known candidate, which
    /// is what keeps the reported mentions true after the text is edited around them
    /// rather than only when a completion was performed.
    /// Rebuilds the completed-mention table after an edit starting at `from` (I-106).
    ///
    /// Mentions entirely before the edit are kept; anything overlapping or after it is
    /// rescanned from the last kept mention's end. An edit in the middle of a long field
    /// therefore costs the suffix rather than the whole document.
    fn rebuild_completed_from(&mut self, from: usize) {
        let keep = self
            .completed
            .iter()
            .position(|mention| mention.end > from)
            .unwrap_or(self.completed.len());
        self.completed.truncate(keep);
        let mut search_from = self.completed.last().map_or(from, |mention| mention.end);
        if search_from >= self.text.len() {
            return;
        }
        while let Some(relative) = self.text[search_from..].find(self.trigger) {
            let start = search_from + relative;
            // Only a trigger at the start of the text or after whitespace can begin a
            // mention; this is the same rule `active_token` applies.
            if start > 0 {
                let preceding = self.text[..start].chars().next_back();
                if !preceding.is_some_and(char::is_whitespace) {
                    search_from = start + self.trigger.len_utf8();
                    continue;
                }
            }
            let after = &self.text[start + self.trigger.len_utf8()..];
            // The longest matching display wins, so two candidates whose displays
            // share a prefix resolve to the one the text actually contains.
            let mut best: Option<(&MentionCandidate, usize)> = None;
            for candidate in &self.candidates {
                if after.starts_with(&candidate.display) {
                    let length = candidate.display.len();
                    if best.is_none_or(|(_, best_length)| length > best_length) {
                        best = Some((candidate, length));
                    }
                }
            }
            match best {
                Some((candidate, length)) => {
                    let end = start + self.trigger.len_utf8() + length;
                    self.completed.push(CompletedMention { id: candidate.id.clone(), start, end });
                    search_from = end;
                }
                // A trigger with no matching candidate is ordinary text (an email, a
                // handle the caller does not know); skipping one character avoids an
                // infinite loop without pretending it is a mention.
                None => search_from = start + self.trigger.len_utf8(),
            }
        }
    }

    /// How many popup rows the visible window shows (I-102).
    fn visible_row_count(&self) -> usize {
        self.filtered.len().min(self.max_visible)
    }

    /// The popup's top edge and whether it was flipped above the field (I-102).
    ///
    /// The host may supply the available popup area through [`Self::set_popup_viewport`]. When
    /// the rows below the field would cross its bottom edge, the popup is placed above the field
    /// instead. Without a viewport there is no screen fact to measure against, so the popup stays
    /// below — the same placement this control always had.
    fn popup_top(&self, rows: usize) -> (i32, bool) {
        let rect = self.geometry();
        let height = (rows as u32 * self.row_height) as i32;
        let below = rect.y + rect.height as i32;
        let fits = self
            .popup_viewport
            .is_none_or(|viewport| below + height <= viewport.y + viewport.height as i32);
        if fits {
            (below, false)
        } else {
            (rect.y - height, true)
        }
    }

    /// Sets the host's available popup area so the popup can flip upward (I-102).
    pub fn set_popup_viewport(&mut self, viewport: Option<Rect>) {
        if self.popup_viewport == viewport {
            return;
        }
        self.popup_viewport = viewport;
        self.base.request_redraw();
    }

    /// The rectangle of the popup row at `index` **within the visible window**.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.visible_row_count() {
            return None;
        }
        let rect = self.geometry();
        let (top, _) = self.popup_top(self.visible_row_count());
        Some(Rect::new(
            rect.x,
            top + (index as u32 * self.row_height) as i32,
            rect.width,
            self.row_height,
        ))
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        let rows = self.visible_row_count();
        (0..rows).find(|index| self.row_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// Keeps the highlighted row inside the visible window (I-102).
    fn scroll_to_highlight(&mut self) {
        let Some(highlight) = self.highlighted else {
            return;
        };
        if highlight < self.scroll_offset {
            self.scroll_offset = highlight;
        } else if highlight >= self.scroll_offset + self.max_visible {
            self.scroll_offset = highlight + 1 - self.max_visible;
        }
        let max_offset = self.filtered.len().saturating_sub(self.max_visible);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// Whether the current popup was placed above the field (I-102).
    pub fn popup_flipped(&self) -> bool {
        self.popup_flipped
    }

    /// Advances the caret blink by `delta_ms` (I-101).
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

    /// The font the mention field and its popup rows draw with: the resolved theme/caller font when
    /// the style names one, otherwise the default sans-serif face (D09-STYLE-01).
    ///
    /// Named once so `caret_x`, `draw` and `draw_popup` measure and paint with the same font, which
    /// is what keeps the caret on the glyph the pointer lands on.
    fn field_font(&self) -> Font {
        effective_font(self.style()).clone()
    }

    /// Runs `f` against a throwaway measurement surface.
    fn with_measurement<R>(f: impl FnOnce(&RenderContext) -> R) -> R {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        f(&RenderContext::new(&mut measurement))
    }

    /// The per-character advances of the current text, measured once per revision (I-107).
    ///
    /// `measure` is the caller's renderer measurement, so the paint path and the click path
    /// use the same ruler — the click must not guess with a second model (I-131).
    fn text_widths(
        &self,
        font: &Font,
        dpi_bits: u32,
        measure: &mut dyn FnMut(&str) -> u32,
    ) -> core::cell::Ref<'_, TextWidthCache> {
        let stale = self.width_cache.borrow().as_ref().is_none_or(|cache| {
            cache.revision != self.text_revision
                || cache.font != *font
                || cache.dpi_bits != dpi_bits
        });
        if stale {
            let mut byte_offsets = Vec::with_capacity(self.text.chars().count() + 1);
            let mut widths = Vec::with_capacity(self.text.chars().count() + 1);
            let mut prefix = 0u32;
            let mut glyph = [0u8; 4];
            for (index, ch) in self.text.char_indices() {
                byte_offsets.push(index);
                widths.push(prefix);
                prefix = prefix.saturating_add(measure(ch.encode_utf8(&mut glyph)));
            }
            byte_offsets.push(self.text.len());
            widths.push(prefix);
            *self.width_cache.borrow_mut() = Some(TextWidthCache {
                revision: self.text_revision,
                font: font.clone(),
                dpi_bits,
                byte_offsets,
                widths,
            });
        }
        core::cell::Ref::map(self.width_cache.borrow(), |cache| {
            cache.as_ref().expect("the width cache was just filled")
        })
    }
}

impl Widget for Mention {
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
        Size::new(240, 30)
    }

    /// Advances the caret blink and reports whether another frame is owed (I-101).
    fn tick(&mut self, delta_ms: u32) -> bool {
        Mention::tick(self, delta_ms)
    }

    /// A focused field's caret is cycling, so a frame is owed (I-101).
    fn is_animating(&self) -> bool {
        self.focused && self.cursor_blink.is_running()
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
            "mention_inserted" => {
                Some(EventSignalRef::mapped("mention_inserted", &self.mention_inserted, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "popup_toggled" => {
                Some(EventSignalRef::mapped("popup_toggled", &self.popup_toggled, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `Mention`'s property contract.
///
/// The candidates and the text are written through `set_candidates` / `set_text`;
/// the property layer reports the derived counts and the editable scalars, following
/// the same convention as the other list-valued controls.
impl WidgetProperties for Mention {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The candidate list as records (BLUE26 B26.09): `set_candidates` takes a list the bare
            // command channel cannot carry, so the list is the property and `set_candidates` is
            // its published command form.
            "candidates" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &MENTION_CANDIDATE_SCHEMA,
                    self.candidates().iter().map(mention_candidate_record).collect::<Vec<_>>(),
                )))
            }
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "trigger" => Ok(CapabilityValue::String(self.trigger().to_string())),
            "candidate_count" => Ok(CapabilityValue::UInt(self.candidates().len() as u64)),
            "visible_candidate_count" => {
                Ok(CapabilityValue::UInt(self.visible_candidates().len() as u64))
            }
            "mention_count" => Ok(CapabilityValue::UInt(self.completed_mentions().len() as u64)),
            "popup_open" => Ok(CapabilityValue::Bool(self.is_popup_open())),
            "caret" => Ok(CapabilityValue::UInt(self.caret() as u64)),
            "max_visible" => Ok(CapabilityValue::UInt(self.max_visible() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                let text = expect_string(value)?;
                self.set_text(text);
                Ok(())
            }
            "trigger" => {
                let text = expect_string(value)?;
                let mut chars = text.chars();
                let (Some(trigger), None) = (chars.next(), chars.next()) else {
                    // A trigger is one character; accepting a longer string would make
                    // "which character fired" unanswerable.
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                if self.set_trigger(trigger) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::TypeMismatch)
                }
            }
            "popup_open" => {
                // Read-only in spirit: opening without a typed trigger would show a
                // popup whose query does not exist. `open_popup` is the real entry.
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            // A whole-list replacement through the control's own `set_candidates`, which
            // re-derives visibility and completion rather than leaving a second copy to go stale.
            "candidates" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&MENTION_CANDIDATE_SCHEMA).map_err(|violation| {
                    log::warn!("mention.candidates refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("candidates"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let candidates = list
                    .items()
                    .iter()
                    .map(mention_candidate_from_record)
                    .collect::<Result<Vec<_>, CapabilityAccessError>>()?;
                self.set_candidates(candidates);
                Ok(())
            }
            "max_visible" => {
                self.set_max_visible(expect_usize(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32_saturating(value)?);
                Ok(())
            }
            // Derived from the text and the candidate list.
            "candidate_count" | "visible_candidate_count" | "mention_count" | "caret" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `MENTION_PROPERTIES`.
        property_names_of![
            "candidates",
            "text",
            "trigger",
            "candidate_count",
            "visible_candidate_count",
            "mention_count",
            "popup_open",
            "caret",
            "max_visible",
            "row_height",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `mention` publishes.
    ///
    /// `open_popup` is the zero-argument action and goes through the control's own
    /// method, which inserts the trigger and runs the same path typed input would. On a
    /// fresh control that leaves the popup open, and answers `Ok(())`.
    ///
    /// `complete` picks a candidate by index, so a bare command has no candidate to
    /// commit; `set_candidates` carries its list through the `candidates` records
    /// property (B26.03), and `set_trigger` through the writable `trigger` property.
    /// Both therefore report `OutOfRange` for a bare invocation — the payload is what
    /// is missing, not the command.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "open_popup" => {
                self.open_popup();
                Ok(())
            }
            "set_candidates" | "set_trigger" => Err(CapabilityAccessError::OutOfRange),
            // `complete` needs a candidate, so the command completes the highlighted one and
            // refuses when nothing is highlighted — the same answer the control's own key
            // path gives.
            "complete" => match self.highlighted() {
                Some(index) if self.complete(index).is_some() => Ok(()),
                _ => Err(CapabilityAccessError::OutOfRange),
            },
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Mention {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let background = style.background_color.unwrap_or(Color::WHITE);
        let border = style.border_color.unwrap_or(Color::rgb(190, 192, 198));
        let text_color = style.text_color.unwrap_or(Color::rgb(40, 44, 52));

        context.fill_rounded_rect(rect, 4, background);
        context.draw_rounded_rect_stroke(rect, 4, border, 1);
        // The **effective font** — the resolved theme/caller font — for the typed text and the
        // completed-mention underlines, so the theme body font and the user's text scale apply
        // (D09-STYLE-01).
        let field_font = self.field_font();
        let field_line = context.text_line(rect, &field_font);
        // The field is an editable input: its text, the mention underlines and the caret
        // are all positioned by measurement, so clipping the group to the field (rather
        // than ellipsising the text) keeps every position honest while a long line can
        // no longer paint past the field's own edge. The popup below is drawn after the
        // clip is popped, so it is unaffected.
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        // I-P66: one measured prefix table shared by the selection edges, every completed
        // mention's underline and the caret. The table itself is memoised per text revision
        // and text scale, so an unchanged frame measures no character again (I-107).
        let dpi_bits = context.dpi_scale().to_bits();
        let widths = self.text_widths(&field_font, dpi_bits, &mut |text| {
            context.measure_text(text, &field_font).width
        });
        let width_at = |offset: usize| -> u32 { widths.width_at(offset.min(self.text.len())) };
        // The selection band sits under the glyphs, measured with the same shaper the text uses.
        if let Some((start, end)) = self.selection_range() {
            let from = start.min(self.text.len());
            let to = end.min(self.text.len());
            let x0 = rect.x + 8 + width_at(from) as i32;
            let x1 = rect.x + 8 + width_at(to) as i32;
            if x1 > x0 {
                let fill = text_color.blend(&background, 0.3).blend(&background, 0.25);
                context.fill_rect(
                    Rect::new(x0, field_line.y, (x1 - x0) as u32, field_line.height.max(1)),
                    fill,
                );
            }
        }
        context.draw_text(
            Point::new(rect.x + 8, field_line.y),
            &self.text,
            &field_font,
            text_color,
            HorizontalAlignment::Left,
        );

        // Completed mentions are underlined, so the user can see which spans the
        // control recognises — including the ones typed by hand rather than chosen.
        // The underline is the theme's `primary`, the hue a theme varies most, rather than a
        // literal blue that stayed put on both appearances.
        let mention_ink = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::rgb(66, 133, 244));
        for mention in &self.completed {
            let start = mention.start.min(self.text.len());
            let end = mention.end.min(self.text.len());
            let start_x = rect.x + 8 + width_at(start) as i32;
            let width = width_at(end).saturating_sub(width_at(start)) as i32;
            // The underline sits just under the line box, so it tracks the effective font rather
            // than the fixed `rect.y + 22` that assumed a 12 px face.
            let underline_y = field_line.y + field_line.height as i32;
            context.draw_line_stroke(
                Point::new(start_x, underline_y),
                Point::new(start_x + width, underline_y),
                mention_ink,
                1,
            );
        }

        if self.is_popup_open() {
            // I-101: the caret is painted only during the visible half of its blink cycle.
            if self.cursor_blink.is_visible() {
                let caret_x = rect.x + 8 + width_at(self.caret.min(self.text.len())) as i32;
                context.draw_line_stroke(
                    Point::new(caret_x, rect.y + 8),
                    Point::new(caret_x, rect.y + 8 + CARET_HEIGHT as i32),
                    text_color,
                    1,
                );
            }
            context.pop_clip();
            // The width table borrows the cache; release it before mutating the control.
            drop(widths);
            // I-102: remember the placement the hit test will derive from the same inputs.
            self.popup_flipped = self.popup_top(self.visible_row_count()).1;
            self.draw_popup(context);
        } else {
            context.pop_clip();
        }
    }
}

impl Mention {
    /// Draws the candidate popup below the field.
    ///
    /// # Why the popup reads the theme and the field's fallbacks do not
    ///
    /// The field's literals are *fallbacks*: `apply_active_theme` fills `style.background_color` and
    /// friends before this runs, so on a themed build the `unwrap_or` arms are unreachable. The popup
    /// is different — it painted `Color::WHITE`, a fixed grey border and two fixed inks
    /// unconditionally, so a dark build opened a white list under a dark field. It is fixed the same
    /// way `dropdown`'s list and `cascader`'s columns were: a popup is one step above the page
    /// (`surface_container`), its border is the weak separator (`outline_variant`), a highlighted row
    /// is a *selection* (the accent pair), and the secondary description is the theme's weak ink.
    fn draw_popup(&self, context: &mut RenderContext) {
        let rows = self.visible_row_count();
        if rows == 0 {
            return;
        }
        let rect = self.geometry();
        // I-102: the popup flips above the field when the host-reported viewport has no room
        // below it.
        let (top, _) = self.popup_top(rows);
        let popup = Rect::new(
            rect.x,
            top,
            rect.width,
            (rows as u32 * self.row_height).max(self.row_height),
        );
        // One guard over the theme reads, released before drawing: `theme_manager()` is a
        // non-reentrant mutex and the context's accessors cannot take it.
        let (popup_surface, separator, highlight, on_highlight, row_ink, weak_ink) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.surface_container,
                    active.colors.outline_variant,
                    active.colors.primary,
                    active.colors.primary.contrast_color(),
                    active.colors.foreground,
                    active.colors.secondary,
                ),
                None => (
                    Color::WHITE,
                    Color::rgb(190, 192, 198),
                    Color::rgb(232, 240, 254),
                    Color::rgb(40, 44, 52),
                    Color::rgb(40, 44, 52),
                    Color::rgb(140, 144, 152),
                ),
            }
        };
        context.fill_rounded_rect(popup, 4, popup_surface);
        context.draw_rounded_rect_stroke(popup, 4, separator, 1);

        for row_index in 0..rows {
            let Some(row) = self.row_rect(row_index) else {
                continue;
            };
            // The visible window is the filtered list offset by the scroll position.
            let candidate_index = self.scroll_offset + row_index;
            let Some(candidate) =
                self.filtered.get(candidate_index).and_then(|i| self.candidates.get(*i))
            else {
                continue;
            };
            let highlighted = self.highlighted == Some(candidate_index);
            if highlighted {
                context.fill_rect(row, highlight);
            }
            let ink = if highlighted { on_highlight } else { row_ink };
            let popup_font = self.field_font();
            let row_line = context.text_line(row, &popup_font);
            // The value column stops where the description starts; the description stops
            // at the row's right padding. Neither ran out of room before, so a long pair
            // overlapped or painted past the popup edge.
            let display_width = ((row.x + 112) - (row.x + 8)).max(0) as u32;
            context.draw_text_fitted_cow(
                Rect::new(row.x + 8, row_line.y, display_width.max(1), row_line.height.max(1)),
                &candidate.display,
                &popup_font,
                ink,
                HorizontalAlignment::Left,
            );
            if !candidate.description.is_empty() {
                let description_width =
                    ((row.x + row.width as i32 - 8) - (row.x + 120)).max(0) as u32;
                context.draw_text_fitted_cow(
                    Rect::new(
                        row.x + 120,
                        row_line.y,
                        description_width.max(1),
                        row_line.height.max(1),
                    ),
                    &candidate.description,
                    &popup_font,
                    // The description is a hint beside a value, so on a highlighted row it damps
                    // toward that row's fill rather than staying the page's weak ink.
                    if highlighted { on_highlight.blend(&highlight, 0.3) } else { weak_ink },
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for Mention {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained { .. } => {
                self.focused = true;
                self.cursor_blink.start();
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.cursor_blink.stop();
                self.base.request_redraw();
            }
            Event::Wheel { delta, modifiers, .. } => {
                // I-102: the candidate window scrolls without moving the highlight.
                if self.is_popup_open() {
                    let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                    let max_offset = self.filtered.len().saturating_sub(self.max_visible);
                    let next = (self.scroll_offset as i64 - scroll.lines_y as i64)
                        .clamp(0, max_offset as i64) as usize;
                    if next != self.scroll_offset {
                        self.scroll_offset = next;
                        self.base.request_redraw();
                    }
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let rect = self.geometry();
                if rect.contains_point(*pos) {
                    self.focused = true;
                    self.cursor_blink.start();
                    // The press places the caret where the user aimed and starts a selection, so
                    // clicking into the middle of the value inserts there instead of appending.
                    let index = self.byte_index_at_x(pos.x);
                    self.caret = index;
                    self.selection_anchor = Some(index);
                    self.dragging_selection = true;
                    self.refresh();
                    self.base.request_redraw();
                    return;
                }
                if self.is_popup_open() {
                    if let Some(index) = self.row_at(*pos) {
                        self.complete(index);
                    } else {
                        // A click outside the popup and outside the field dismisses,
                        // which is what every popup list does.
                        self.close_popup();
                    }
                }
            }
            // A drag extends the press's selection; a bare hover still does nothing because
            // `dragging_selection` is only raised by a press.
            Event::MouseMove { pos } => {
                if self.dragging_selection {
                    self.caret = self.byte_index_at_x(pos.x);
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.dragging_selection = false;
            }
            // Typing goes to the field, and a trigger character opens the popup as a
            // side effect of `refresh` — which is what makes the trigger model work
            // without a separate "detect @" step.
            //
            // `ImeCommit` is the CJK path: an IME commits the composed string as one
            // event, not as `TextInput`, so without this arm every composed character
            // was dropped. Both spellings share one insertion path and one guard.
            Event::TextInput { text } | Event::ImeCommit { text }
                if text.chars().all(|ch| !ch.is_control()) && !text.is_empty() =>
            {
                // I-P66: one splice, one refresh and one signal for the whole committed run.
                self.insert_str(text);
            }
            Event::KeyDown((key, _)) | Event::KeyPress { key, .. } => {
                let primary = match event {
                    Event::KeyPress { modifiers, .. } => {
                        crate::shortcut::event_carries_primary(*modifiers)
                    }
                    _ => false,
                };
                // I-101: clipboard and history, matching the crate's text fields.
                if primary && (*key == 90 || *key == 122) {
                    let _ = self.undo();
                    return;
                }
                if primary && (*key == 89 || *key == 121) {
                    let _ = self.redo();
                    return;
                }
                if primary && (*key == 67 || *key == 99) {
                    let _ = self.copy_selection_to_clipboard();
                    return;
                }
                if primary && (*key == 88 || *key == 120) {
                    let _ = self.copy_selection_to_clipboard();
                    self.delete_selection();
                    return;
                }
                if primary && (*key == 86 || *key == 118) {
                    self.paste_from_clipboard();
                    return;
                }
                if primary && (*key == 65 || *key == 97) {
                    self.select_all();
                    return;
                }
                match *key {
                    8 => {
                        self.backspace();
                    }
                    37 => self.move_caret_left(),
                    39 => self.move_caret_right(),
                    // I-101: Home/End reach the ends of the single-line value.
                    36 => {
                        self.caret = 0;
                        self.refresh();
                        self.base.request_redraw();
                    }
                    35 => {
                        self.caret = self.text.len();
                        self.refresh();
                        self.base.request_redraw();
                    }
                    // Up/Down drive the popup when it is open, and do nothing when it is
                    // not — a single-line field has no vertical caret movement, so
                    // consuming the keys would be surprising.
                    38 if self.is_popup_open() => self.move_highlight(-1),
                    40 if self.is_popup_open() => self.move_highlight(1),
                    13 if self.is_popup_open() => {
                        let index = self.highlighted.unwrap_or(0);
                        self.complete(index);
                    }
                    27 => self.close_popup(),
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
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn candidates() -> Vec<MentionCandidate> {
        vec![
            MentionCandidate::new("alice", "Alice").with_description("Engineering"),
            MentionCandidate::new("alicia", "Alicia").with_description("Design"),
            MentionCandidate::new("bob", "Bob").with_description("Support"),
        ]
    }

    fn mention() -> Mention {
        let mut m = Mention::new(Rect::new(0, 0, 240, 30));
        m.set_candidates(candidates());
        m
    }

    /// Renders and returns the RGBA frame.
    ///
    /// Gated like its consumer: the software backend and `Size` only exist behind a device
    /// profile with `software`, so an ungated helper is a dead-code warning (or a hard error in a
    /// `no_std` profile) for a build that never calls it.
    fn render(m: &mut Mention, size: Size) -> Vec<u8> {
        render_on(m, size, Color::WHITE)
    }

    /// Renders over an explicit backdrop, which is the surface the control would sit on.
    fn render_on(m: &mut Mention, size: Size, backdrop: Color) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(backdrop);
        let mut context = RenderContext::new(&mut backend);
        m.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The RGBA pixel at `(x, y)` of a `size`-wide frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn pixel(frame: &[u8], size: Size, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * size.width + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    }

    /// Types `text` one character at a time, as a keyboard would.
    fn type_text(m: &mut Mention, text: &str) {
        for ch in text.chars() {
            m.insert_char(ch);
        }
    }

    #[test]
    fn mention_creation_defaults() {
        let m = Mention::new(Rect::new(0, 0, 240, 30));
        assert_eq!(m.kind(), WidgetKind::Mention);
        assert_eq!(m.text(), "");
        assert_eq!(m.caret(), 0);
        assert_eq!(m.trigger(), '@');
        assert!(m.candidates().is_empty());
        assert!(!m.is_popup_open());
        assert!(m.completed_mentions().is_empty());
    }

    #[test]
    fn mention_trigger_opens_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@");
        assert!(m.is_popup_open(), "the trigger must open the popup");
        assert_eq!(m.query(), Some(""));
        assert_eq!(m.visible_candidates().len(), 3);
    }

    /// The candidate popup follows the appearance, not just the field.
    ///
    /// # The defect this pins
    ///
    /// `draw_popup` painted `Color::WHITE`, a fixed grey border, a fixed pale highlight and two
    /// fixed inks — none of which moved with the theme — while the field above it read `style.*`.
    /// So a dark build showed a white suggestion list hanging off a dark field, the same
    /// "half themed" shape `dropdown` had.
    ///
    /// # Why the assertion names the popup's own pixel
    ///
    /// A frame-wide comparison passes even with the literals restored, because the field and the
    /// backdrop differ between appearances too. The sample point is inside the popup band, on a row
    /// with no glyph there, so the pixel it reads is the popup surface and nothing else.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_candidate_popup_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let size = Size::new(240, 160);

        let popup_pixel = |appearance| -> [u8; 4] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let backdrop = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let mut m = mention();
            type_text(&mut m, "@");
            crate::theme::apply_theme_to_widget(&mut m);
            let frame = render_on(&mut m, size, backdrop);
            // The field is 30 tall, so the popup starts below it. Half the popup's width and a y
            // near its bottom edge are clear of any glyph — the row text is drawn from `y + 16`.
            let x = 240 / 2;
            let y = 30 + 3 * ROW_HEIGHT - 3;
            pixel(&frame, size, x, y)
        };

        let dark = popup_pixel(crate::theme::AppearanceMode::Dark);
        let light = popup_pixel(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "the candidate popup must follow the appearance; both were {dark:?}"
        );
        assert_ne!(
            dark,
            [255, 255, 255, 255],
            "the candidate popup must not be the fixed white the defect used"
        );
    }

    #[test]
    fn mention_typing_filters_by_query() {
        let mut m = mention();
        type_text(&mut m, "@al");
        assert_eq!(m.query(), Some("al"));
        let visible = m.visible_candidates();
        assert_eq!(visible.len(), 2, "Alice and Alicia both start with 'al'");
        assert_eq!(visible[0].display, "Alice");
    }

    #[test]
    fn mention_query_narrows_to_one() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        let visible = m.visible_candidates();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "bob");
    }

    #[test]
    fn mention_matches_description_as_well_as_display() {
        let mut m = mention();
        type_text(&mut m, "@Design");
        let visible = m.visible_candidates();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "alicia");
    }

    #[test]
    fn mention_with_no_match_closes_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@zzzz");
        // An empty popup reads as a broken control, so it closes instead.
        assert!(!m.is_popup_open());
        assert!(m.visible_candidates().is_empty());
    }

    #[test]
    fn mention_completion_replaces_the_token_including_the_trigger() {
        let mut m = mention();
        type_text(&mut m, "@ali");
        let id = m.complete(0).expect("a candidate is highlighted");
        assert_eq!(id, "alice");
        // `@ali` becomes `@Alice` — the trigger is consumed, not doubled.
        assert_eq!(m.text(), "@Alice");
        assert_eq!(m.caret(), "@Alice".len(), "the caret lands after the mention");
        assert!(!m.is_popup_open(), "completing closes the popup");
    }

    #[test]
    fn mention_completion_keeps_surrounding_prose() {
        let mut m = mention();
        type_text(&mut m, "hello ");
        type_text(&mut m, "@bob");
        m.complete(0);
        assert_eq!(m.text(), "hello @Bob");
    }

    #[test]
    fn mention_records_the_completed_mention() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        assert!(m.complete(0).is_some(), "a candidate must be highlighted to complete");
        // Prose after the mention does not disturb the recorded span.
        type_text(&mut m, " please review");

        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "bob");
        // The span covers the trigger and the display.
        assert_eq!(&m.text()[completed[0].start..completed[0].end], "@Bob");
        assert_eq!(m.text(), "@Bob please review");
    }

    #[test]
    fn mention_supports_several_mentions_in_one_field() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        m.complete(0);
        type_text(&mut m, " and ");
        type_text(&mut m, "@ali");
        m.complete(0);

        assert_eq!(m.text(), "@Bob and @Alice");
        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].id, "bob");
        assert_eq!(completed[1].id, "alice");
    }

    #[test]
    fn mention_does_not_open_mid_word() {
        let mut m = mention();
        // `@` inside a word makes an email address, not a mention.
        type_text(&mut m, "user@example");
        assert!(!m.is_popup_open(), "a mid-word trigger is not a mention");
    }

    #[test]
    fn mention_does_not_reopen_after_a_space() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        m.complete(0);
        type_text(&mut m, " ");
        assert!(!m.is_popup_open(), "a completed mention must not reopen");
    }

    #[test]
    fn mention_a_trigger_after_whitespace_opens() {
        let mut m = mention();
        type_text(&mut m, "hi ");
        type_text(&mut m, "@");
        assert!(m.is_popup_open());
    }

    #[test]
    fn mention_backspace_updates_the_query() {
        let mut m = mention();
        type_text(&mut m, "@ali");
        assert_eq!(m.visible_candidates().len(), 2);
        m.backspace();
        assert_eq!(m.query(), Some("al"));
        m.backspace();
        assert_eq!(m.query(), Some("a"));
    }

    #[test]
    fn mention_backspacing_the_trigger_closes_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@");
        assert!(m.is_popup_open());
        assert!(m.backspace());
        assert!(!m.is_popup_open(), "no trigger means no popup");
        assert_eq!(m.text(), "");
    }

    #[test]
    fn mention_backspace_at_zero_does_nothing() {
        let mut m = mention();
        assert!(!m.backspace());
    }

    #[test]
    fn mention_backspace_handles_multibyte_characters() {
        let mut m = mention();
        // A CJK character is three bytes; a naive `caret - 1` would split it and
        // panic on the slice.
        type_text(&mut m, "你");
        assert_eq!(m.text(), "你");
        assert_eq!(m.caret(), 3);
        assert!(m.backspace());
        assert_eq!(m.text(), "");
        assert_eq!(m.caret(), 0);
    }

    #[test]
    fn mention_highlight_moves_and_wraps() {
        let mut m = mention();
        type_text(&mut m, "@");
        assert_eq!(m.highlighted(), Some(0));

        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), Some(1));
        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), Some(2));
        // Wraps at the end.
        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), Some(0));
        m.handle_event(&Event::KeyDown((38, 0)));
        assert_eq!(m.highlighted(), Some(2));
    }

    #[test]
    fn mention_enter_chooses_the_highlighted_candidate() {
        let mut m = mention();
        type_text(&mut m, "@");
        m.handle_event(&Event::KeyDown((40, 0))); // highlight the second
        m.handle_event(&Event::KeyDown((13, 0)));
        assert_eq!(m.text(), "@Alicia");
    }

    #[test]
    fn the_complete_command_uses_the_highlight_and_refuses_without_one() {
        let mut m = mention();
        // Nothing highlighted yet: the command answers the same refusal the key path gives.
        assert_eq!(m.command("complete"), Err(CapabilityAccessError::OutOfRange));

        type_text(&mut m, "@");
        assert_eq!(m.highlighted(), Some(0));
        assert_eq!(m.command("complete"), Ok(()));
        assert_eq!(m.text(), "@Alice");
        assert!(!m.is_popup_open(), "completing closes the popup");
    }

    #[test]
    fn mention_arrows_do_not_apply_when_the_popup_is_closed() {
        let mut m = mention();
        type_text(&mut m, "plain");
        // A single-line field has no vertical caret movement, so consuming the keys
        // would be surprising.
        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), None);
        assert_eq!(m.text(), "plain");
    }

    #[test]
    fn mention_escape_closes_without_completing() {
        let mut m = mention();
        type_text(&mut m, "@ali");
        m.handle_event(&Event::KeyDown((27, 0)));
        assert!(!m.is_popup_open());
        assert_eq!(m.text(), "@ali", "cancelling must not change the text");
    }

    #[test]
    fn mention_horizontal_arrows_move_the_caret() {
        let mut m = mention();
        type_text(&mut m, "abc");
        assert_eq!(m.caret(), 3);
        m.handle_event(&Event::KeyDown((37, 0)));
        assert_eq!(m.caret(), 2);
        m.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(m.caret(), 3);
        // Clamped at both ends.
        m.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(m.caret(), 3);
    }

    #[test]
    fn mention_click_on_a_row_completes_it() {
        let mut m = mention();
        type_text(&mut m, "@");
        let row = m.row_rect(1).expect("row 1");
        m.handle_event(&Event::mouse_press(row.x + 20, row.y + 10, 1));
        assert_eq!(m.text(), "@Alicia");
    }

    #[test]
    fn mention_click_outside_closes_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@");
        m.handle_event(&Event::mouse_press(5000, 5000, 1));
        assert!(!m.is_popup_open());
    }

    #[test]
    fn mention_click_on_the_field_puts_the_caret_at_the_end() {
        let mut m = mention();
        type_text(&mut m, "abc");
        m.move_caret_left();
        assert_eq!(m.caret(), 2);
        m.handle_event(&Event::mouse_press(100, 15, 1));
        assert_eq!(m.caret(), 3);
    }

    #[test]
    fn mention_disabled_ignores_typing() {
        let mut m = mention();
        m.set_enabled(false);
        m.handle_event(&Event::TextInput { text: "@".to_string() });
        assert_eq!(m.text(), "");
    }

    #[test]
    fn mention_ime_commit_inserts_text_and_opens_the_popup() {
        let mut m = mention();
        m.handle_event(&Event::ime_commit("\u{4f60}\u{597d}"));
        assert_eq!(m.text(), "\u{4f60}\u{597d}", "IME commits are not dropped");

        // The trigger opens a token only at the start of the field or after whitespace
        // (an embedded `foo@bar` is an email address, not a mention). The IME path must
        // therefore reach the same `refresh` rule as typed input.
        m.handle_event(&Event::ime_commit(" @"));
        assert_eq!(m.text(), "\u{4f60}\u{597d} @");
        assert!(m.is_popup_open(), "the trigger character still opens the popup");
    }

    #[test]
    fn mention_open_popup_from_code_inserts_the_trigger() {
        let mut m = mention();
        // A toolbar button needs the same entry point the keyboard has.
        assert!(m.open_popup());
        assert_eq!(m.text(), "@");
        assert!(m.is_popup_open());
    }

    #[test]
    fn mention_open_popup_with_no_candidates_does_nothing() {
        let mut m = Mention::new(Rect::new(0, 0, 240, 30));
        assert!(!m.open_popup());
        assert!(!m.is_popup_open());
    }

    #[test]
    fn mention_signals_fire() {
        let mut m = mention();
        let inserted = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let toggles = std::sync::Arc::new(std::sync::Mutex::new(Vec::<bool>::new()));
        let sink = inserted.clone();
        m.mention_inserted.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*id).clone());
            }
        });
        let sink = toggles.clone();
        m.popup_toggled.connect(move |open| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*open);
            }
        });

        type_text(&mut m, "@bob");
        m.complete(0);

        assert_eq!(*inserted.lock().expect("signal lock poisoned"), vec!["bob"]);
        assert_eq!(*toggles.lock().expect("signal lock poisoned"), vec![true, false]);
    }

    #[test]
    fn mention_set_text_derives_the_state() {
        let mut m = mention();
        m.set_text("hey @ali".to_string());
        assert_eq!(m.caret(), "hey @ali".len());
        assert!(m.is_popup_open(), "set_text must derive the token state too");
        assert_eq!(m.visible_candidates().len(), 2);
    }

    #[test]
    fn mention_set_candidates_drops_vanished_mentions() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        m.complete(0);
        assert_eq!(m.completed_mentions().len(), 1);

        // A mention whose candidate is gone would report an id nothing resolves.
        m.set_candidates(vec![MentionCandidate::new("alice", "Alice")]);
        assert!(m.completed_mentions().is_empty());
    }

    #[test]
    fn mention_rebuilds_mentions_typed_by_hand() {
        let mut m = mention();
        // Typed rather than chosen: the control must still recognise it, which is what
        // makes the underline honest.
        m.set_text("cc @Bob and @Alice".to_string());
        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].id, "bob");
        assert_eq!(completed[1].id, "alice");
    }

    #[test]
    fn mention_ignores_a_trigger_with_no_matching_candidate() {
        let mut m = mention();
        // An email address is not a mention, and neither is an unknown handle.
        m.set_text("mail me at a@b.com".to_string());
        assert!(m.completed_mentions().is_empty());
    }

    #[test]
    fn mention_prefers_the_longest_matching_display() {
        let mut m = Mention::new(Rect::new(0, 0, 240, 30));
        m.set_candidates(vec![
            MentionCandidate::new("al", "Al"),
            MentionCandidate::new("alan", "Alan"),
        ]);
        // `@Alan` matches both displays as a prefix; the longer one is what the text
        // contains.
        m.set_text("@Alan".to_string());
        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "alan");
    }

    #[test]
    fn mention_set_trigger_changes_the_character() {
        let mut m = mention();
        assert!(m.set_trigger('#'));
        assert_eq!(m.trigger(), '#');
        type_text(&mut m, "#bo");
        assert!(m.is_popup_open());
        assert_eq!(m.visible_candidates().len(), 1);
    }

    #[test]
    fn mention_set_trigger_refuses_whitespace() {
        let mut m = mention();
        // The token walker stops at whitespace, so a space trigger could never fire.
        assert!(!m.set_trigger(' '));
        assert_eq!(m.trigger(), '@');
    }

    // ── Drawing ─────────────────────────────────────────────────────────────

    #[test]
    fn mention_draw_closed_paints_without_panicking() {
        let mut m = mention();
        type_text(&mut m, "hi");
        let rgba = render(&mut m, Size::new(240, 30));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn mention_popup_differs_from_the_closed_frame() {
        let mut m = mention();
        let closed = render(&mut m, Size::new(240, 120));
        type_text(&mut m, "@");
        let open = render(&mut m, Size::new(240, 120));
        assert_ne!(closed, open, "the popup must be visible");
    }

    #[test]
    fn mention_highlight_is_visible() {
        let mut m = mention();
        type_text(&mut m, "@");
        let first = render(&mut m, Size::new(240, 120));
        m.handle_event(&Event::KeyDown((40, 0)));
        let second = render(&mut m, Size::new(240, 120));
        assert_ne!(first, second, "moving the highlight must be visible");
    }

    #[test]
    fn mention_completed_underline_is_visible() {
        let mut m = mention();
        m.set_text("hi".to_string());
        let plain = render(&mut m, Size::new(240, 30));
        m.set_text("@Bob".to_string());
        let mentioned = render(&mut m, Size::new(240, 30));
        assert_ne!(plain, mentioned);
    }

    #[test]
    fn mention_draw_zero_geometry_does_not_panic() {
        let mut m = mention();
        type_text(&mut m, "@");
        let rgba = render(&mut m, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    // ── Property contract ───────────────────────────────────────────────────

    #[test]
    fn mention_text_property_round_trips() {
        let mut m = mention();
        m.set("text", CapabilityValue::String("@bob hi".to_string())).unwrap();
        assert_eq!(m.text(), "@bob hi");
        assert_eq!(m.get("text").unwrap(), CapabilityValue::String("@bob hi".to_string()));
    }

    #[test]
    fn mention_trigger_property_accepts_one_character_only() {
        let mut m = mention();
        m.set("trigger", CapabilityValue::String("#".to_string())).unwrap();
        assert_eq!(m.trigger(), '#');
        // A longer string would make "which character fired" unanswerable.
        assert!(m.set("trigger", CapabilityValue::String("##".to_string())).is_err());
        assert!(m.set("trigger", CapabilityValue::String(String::new())).is_err());
        // And a whitespace trigger is refused through the property too.
        assert!(m.set("trigger", CapabilityValue::String(" ".to_string())).is_err());
    }

    #[test]
    fn mention_derived_properties_are_read_only() {
        let mut m = mention();
        m.set_text("@bob".to_string());
        assert_eq!(m.get("candidate_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(m.get("visible_candidate_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(m.get("popup_open").unwrap(), CapabilityValue::Bool(true));

        for name in
            ["candidate_count", "visible_candidate_count", "mention_count", "caret", "popup_open"]
        {
            assert_eq!(
                m.set(name, CapabilityValue::UInt(9)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
    }

    #[test]
    fn mention_candidate_matches_id_description_and_display() {
        let candidate = MentionCandidate::new("alice", "Alice").with_description("Engineering");
        assert!(candidate.matches("alice"));
        assert!(candidate.matches("Alice"));
        assert!(candidate.matches("engineering"));
        assert!(candidate.matches(""));
        assert!(!candidate.matches("bob"));
    }

    /// A press places the caret where the pointer landed rather than at the end.
    #[test]
    fn a_click_places_the_caret_at_the_pointer() {
        let mut m = mention();
        m.set_text("hello world");
        m.handle_event(&Event::MousePress { pos: Point::new(40, 8), button: 1, modifiers: 0 });
        assert!(m.caret() > 0, "the caret must not stay at the start");
        assert!(m.caret() < m.text().len(), "the caret must not jump to the end");
    }

    /// A press followed by a drag selects the swept range.
    #[test]
    fn a_drag_selects_a_range() {
        let mut m = mention();
        m.set_text("hello world");
        m.handle_event(&Event::MousePress { pos: Point::new(9, 8), button: 1, modifiers: 0 });
        m.handle_event(&Event::MouseMove { pos: Point::new(70, 8) });
        m.handle_event(&Event::MouseRelease { pos: Point::new(70, 8), button: 1 });
        let (start, end) = m.selection_range().expect("a drag must select");
        assert!(end > start);
        assert!(m.text()[start..end].starts_with("hello"));
    }

    /// The popup's visible-row count and row height are configurable.
    #[test]
    fn the_popup_geometry_is_configurable() {
        let mut m = mention();
        m.set_text("@");
        m.set_max_visible(2);
        m.set_row_height(30);
        assert_eq!(m.max_visible(), 2);
        assert_eq!(m.row_height(), 30);
        assert!(m.row_rect(1).is_some());
        assert!(m.row_rect(2).is_none(), "the popup must not lay out rows past its limit");
        assert_eq!(m.row_rect(1).expect("second row").height, 30);

        use crate::widget::capability::WidgetProperties;
        m.set("max_visible", crate::widget::capability::CapabilityValue::UInt(4))
            .expect("`max_visible` is writable");
        assert_eq!(m.max_visible(), 4);
    }

    #[test]
    fn an_edit_rescans_only_the_touched_suffix() {
        let mut m = mention();
        m.set_text("@Alice and @Bob");
        assert_eq!(m.completed_mentions().len(), 2);
        // Deleting the last character invalidates only the second mention; the first is
        // kept without a rescan.
        assert!(m.backspace());
        assert_eq!(m.completed_mentions().len(), 1);
        assert_eq!(m.completed_mentions()[0].id, "alice");
    }

    #[test]
    fn prefix_matches_rank_above_substring_matches() {
        let mut m = Mention::new(Rect::new(0, 0, 240, 30));
        m.set_candidates(vec![
            MentionCandidate::new("alice", "Alice"),
            MentionCandidate::new("malice", "Malice"),
        ]);
        m.set_text("@ali");
        let visible: Vec<&str> =
            m.visible_candidates().iter().map(|candidate| candidate.display.as_str()).collect();
        assert_eq!(visible, ["Alice", "Malice"], "a display prefix outranks a substring");
    }

    #[test]
    fn the_advance_cache_follows_the_text_revision() {
        let mut m = mention();
        m.set_text("hello @ali");
        let font = m.field_font();
        let first = Mention::with_measurement(|context| {
            m.text_widths(&font, 1.0f32.to_bits(), &mut |text| {
                context.measure_text(text, &font).width
            })
            .revision
        });
        assert_eq!(first, 1, "the first edit advanced the revision");
        // A second frame with the same text reuses the same table.
        Mention::with_measurement(|context| {
            let widths = m.text_widths(&font, 1.0f32.to_bits(), &mut |text| {
                context.measure_text(text, &font).width
            });
            assert_eq!(widths.revision, first);
            assert_eq!(widths.widths.len(), m.text().chars().count() + 1);
        });
        m.insert_char('c');
        assert_eq!(m.text_revision, 2, "another edit advances the revision");
    }
}
