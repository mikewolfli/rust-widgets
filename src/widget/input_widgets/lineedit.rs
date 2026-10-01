// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Single-line text edit widget.
use crate::compat::{Box, Rc, RefCell, String, ToString};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{TextSnapshotCommand, UndoStack};

use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_string, expect_usize,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::decorations::{
    DecorationLayout, DecorationMetrics, DecorationSlots, DECORATION_GAP,
};
use crate::widget::metrics::{dimensions, estimate_text_width, ControlMetrics};

/// Trailing room a text field leaves for the caret, added to the measured value.
const LINE_EDIT_CARET_ROOM: u32 = 10;
/// Narrowest a text field may be, so an empty one is still a field.
const LINE_EDIT_MIN_WIDTH: u32 = 80;
/// Height of a text field.
const LINE_EDIT_HEIGHT: u32 = 24;
use crate::widget::text_utils::{byte_index_of_char, floor_char_boundary};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Single-line text edit widget.
pub struct LineEdit {
    base: BaseWidget,
    text: String,
    placeholder_text: String,
    max_length: Option<usize>,
    echo_mode: EchoMode,
    cursor_position: usize,
    selection_start: Option<usize>,
    read_only: bool,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// Whether this field currently owns keyboard focus.
    ///
    /// Tracked here because the caret is only drawn for the focused field, and the
    /// library's focus router sends `FocusGained` / `FocusLost` (see
    /// `crate::widget::runtime::focus_widget`). Without it the field had no way to
    /// know, and the caret was never drawn at all.
    focused: bool,
    /// The five non-value strings this field shows: `prefix`/`suffix` inside it, and
    /// `helper`/`error`/`counter` on the row below.
    ///
    /// Held as one record rather than five fields so a caller assembling a form from a document can
    /// replace all five in one step, and so the painted state is one value that can be compared.
    decorations: DecorationSlots,
    /// The caret's blink state, advanced by [`LineEdit::tick`].
    ///
    /// Borrowed from [`crate::style::CursorBlink`] rather than reimplemented, so this field's caret
    /// keeps the tempo and phase logic of every other caret in the crate.
    cursor_blink: crate::style::CursorBlink,
    /// The uncommitted input-method composition, or `None` when no IME session is active.
    ///
    /// # Why the preedit is a field of the *field*, and not an overlay widget
    ///
    /// [`crate::widget::input_widgets::ime_preedit::ImePreedit`] draws a composition string with an
    /// underline, and that is the right thing to draw — but on its own it is an *overlay*: the string
    /// it shows never reaches any text model, so committing it had nowhere to go and the "preedit"
    /// could not be edited, cancelled or committed. That is BLUE24 §12 U-5's gap exactly
    /// ("how composition strings enter the text model").
    ///
    /// The three methods a real IME needs are [`LineEdit::set_composition`],
    /// [`LineEdit::commit_composition`] and [`LineEdit::cancel_composition`], and they are correct
    /// **because** the composition is held apart from `text`: while a session is active the field's
    /// value must not change (a `text_changed` per keystroke would fire a form's validation on a word
    /// the user has not finished typing), and cancelling must restore the text exactly as it was.
    composition: Option<String>,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field band as a matter of the
    /// field's own layout, so a `top`/`bottom` value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    /// Whether a pointer drag is currently extending a selection.
    ///
    /// Set by [`LineEdit::press_at`] and cleared by [`LineEdit::end_drag`]. It exists so a
    /// `MouseMove` that is *not* part of a gesture — a bare hover, which arrives on every
    /// pointer step over the field — cannot move a caret the user never grabbed.
    dragging_selection: bool,
    /// Emitted after the widget's text changes: on edit commits, and after an
    /// undo/redo restores a snapshot. Not emitted when a programmatic
    /// `set_text` is given the text the field already holds.
    pub text_changed: Signal1<String>,
    /// Emitted when editing ends — the field loses focus or Enter is pressed.
    /// Carries no payload.
    pub editing_finished: GenericSignal,
    /// Emitted when Enter is pressed in the field, after the edit is committed.
    pub return_pressed: GenericSignal,
}
/// Text echo mode for a line edit.
///
/// Re-exported from [`crate::platform::EchoMode`] — the widget layer and the
/// platform layer name the **same three modes**, so a mode read back from a
/// native control (`widget_echo_mode()`) can be handed straight to this widget
/// with no conversion, and neither copy can drift from the other
/// (principle #54).
///
/// The former local variant `PasswordEchoOnEdit` was removed with this change: it
/// had no consumer outside this file and its "implementation" was a placeholder
/// that behaved exactly like `Password` (principle #5). Adding a real one back
/// means adding it to the canonical enum and to every backend that must honour
/// it.
pub use crate::platform::EchoMode;

impl LineEdit {
    /// Creates an empty line edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "LineEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            echo_mode: EchoMode::Normal,
            cursor_position: 0,
            selection_start: None,
            read_only: false,
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            focused: false,
            decorations: DecorationSlots::default(),
            cursor_blink: crate::style::CursorBlink::new(),
            composition: None,
            alignment: crate::core::Alignment::Left,
            dragging_selection: false,
            text_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
            return_pressed: GenericSignal::new(),
        }
    }
    /// Returns current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How the field's value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the value is centred
    /// vertically in the field band by the field's own layout — the property route refuses it
    /// through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }
    /// Returns whether this field currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }
    /// Sets the focus flag directly.
    ///
    /// The runtime normally drives this through `FocusGained` / `FocusLost`, the same
    /// way the other input controls are driven; this setter exists for hosts and tests
    /// that need to place focus without an event round-trip.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        // The caret blinks exactly while the field holds focus, so the blink state is driven from
        // the same flag `draw` reads rather than from a parallel notion of "active".
        if focused {
            self.cursor_blink.start();
        } else {
            self.cursor_blink.stop();
        }
        self.base.request_redraw();
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention: `true` while the caret is still cycling, so
    /// a host schedules the next frame only for a field that is actually blinking. A blurred field
    /// or a read-only one returns `false` immediately — a steady caret has no next frame.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.focused || self.read_only {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }
    /// Sets text and emits text_changed signal if different.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        // `max_length` is enforced here as well as in `insert_text`.
        //
        // # Why the two paths have to agree
        //
        // `insert_text` clamped to the limit and this did not, so a programmatic `set_text` could leave
        // the field holding a longer value than its own limit — and once the `counter` is derived from
        // the value and that limit, the two disagreed visibly (`8/5`). A limit the control enforces only
        // for typing is not a limit, it is a hint. The truncation goes through `floor_char_boundary`
        // because `max_length` counts **characters** and the slice is in bytes.
        let text = match self.max_length {
            Some(max) if text.chars().count() > max => {
                let byte = byte_index_of_char(&text, max);
                text[..byte].to_string()
            }
            _ => text,
        };
        if self.text == text {
            return;
        }
        self.text = text;
        if !self.restoring_history {
            let before = self.history_target.borrow().clone();
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "line_edit_text",
            )));
        }
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
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

    /// Returns whether a text mutation can be undone.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether a text mutation can be redone.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        self.restoring_history = true;
        self.text = text;
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
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
    /// Returns maximum text length **in characters**.
    pub fn max_length(&self) -> Option<usize> {
        self.max_length
    }
    /// Sets maximum text length, **in characters**, truncating the value if needed.
    ///
    /// # `max_length` counts characters, so this cannot truncate by byte
    ///
    /// The limit is a character budget: it is what the user typing sees and what
    /// [`Self::counter_text`] reports. An earlier revision truncated with
    /// `self.text.truncate(max)`, which is a **byte** index — so `"héllo"` (5 chars, 6 bytes)
    /// under a limit of 3 was cut to `"hé"` (2 chars) rather than `"hél"`, and a limit that
    /// happened to fall inside a multi-byte character would have split it. The truncation
    /// therefore goes through [`byte_index_of_char`], the same conversion `insert_text` and
    /// `set_text` use, so all three agree about what the limit means.
    pub fn set_max_length(&mut self, max_length: Option<usize>) {
        self.max_length = max_length;
        // Truncate if needed
        if let Some(max) = max_length {
            if self.text.chars().count() > max {
                let byte = byte_index_of_char(&self.text, max);
                self.text.truncate(byte);
                self.clamp_caret();
                self.text_changed.emit(self.text.clone());
            }
        }
        self.base.request_redraw();
    }

    /// The current value's length **in characters**.
    ///
    /// The frame `max_length` and the counter are expressed in, so a limit test never has to
    /// compare a character budget against a byte length.
    fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Snaps the caret and the selection anchor to a real character boundary inside the value.
    ///
    /// The caret is a **byte** offset (that is what a slice needs), while every move the user makes
    /// — Left, Right, Backspace, Delete — is a **character** step. Rounding the stored offset down to
    /// a boundary after any of those keeps `&text[..cursor]` total without making the caret's own
    /// arithmetic guess where a character begins, which is what a bare `+= 1` did: on `"é"` a single
    /// Right put the caret *inside* the character, and the next insert then snapped to the end
    /// instead of at the position the user had moved to.
    fn clamp_caret(&mut self) {
        let len = self.text.len();
        self.cursor_position = floor_char_boundary(&self.text, self.cursor_position.min(len));
        if let Some(start) = self.selection_start {
            self.selection_start = Some(floor_char_boundary(&self.text, start.min(len)));
        }
    }
    /// Returns echo mode.
    pub fn echo_mode(&self) -> EchoMode {
        self.echo_mode
    }
    /// Sets echo mode.
    pub fn set_echo_mode(&mut self, mode: EchoMode) {
        self.echo_mode = mode;
        self.base.request_redraw();
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }
    /// Sets cursor position (a byte offset into the value), snapped to a character boundary.
    ///
    /// The offset is a byte index because that is what a slice needs, but a caller passing an
    /// arbitrary number must not be able to leave the caret inside a character — otherwise the next
    /// insert would silently round to the end of the value instead of landing where it was put.
    pub fn set_cursor_position(&mut self, position: usize) {
        self.cursor_position = floor_char_boundary(&self.text, position.min(self.text.len()));
        self.selection_start = None;
        self.base.request_redraw();
    }
    /// Returns selection start position.
    pub fn selection_start(&self) -> Option<usize> {
        self.selection_start
    }
    /// Returns selected text.
    ///
    /// The two offsets are snapped to character boundaries before slicing, so a caret left inside a
    /// character (by a caller's `set`, or by an older `+= 1` step) reads the characters it actually
    /// covers instead of panicking.
    pub fn selected_text(&self) -> String {
        if let Some(start) = self.selection_start {
            let start = floor_char_boundary(&self.text, start.min(self.text.len()));
            let end = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            self.text[start..end].to_string()
        } else {
            String::new()
        }
    }
    /// Selects all text.
    pub fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor_position = self.text.len();
    }
    /// Clears selection.
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
    }

    /// Selects `target` (a byte offset) honouring the modifier keys held at the time.
    ///
    /// # What this is the single entry point for
    ///
    /// Every keyboard movement in this field — Left, Right, Home, End — and every pointer
    /// gesture funnels through here, so the anchor rule is written once. Before it existed,
    /// each of the four movements carried its own copy of the shift test, and a press could
    /// not extend at all. The two behaviours are:
    ///
    /// * **Shift** — extend: the caret moves to `target` and the *anchor* stays put, so the
    ///   selection becomes the range between them. With no anchor yet, the current caret is
    ///   adopted as the anchor first (`Home` on a fresh field then selects back to the start).
    /// * **No modifier** — replace: the caret moves and any selection is dropped.
    ///
    /// # Why Home/End must not re-anchor while extending
    ///
    /// The first cut of this helper re-anchored at the caret before moving, for every key. That
    /// made an extension **irreversible**: `Shift+Home` on a caret at 5 selected `0..=5` and left
    /// the anchor at 5, so the following `Shift+End` selected `5..=len` rather than sweeping to
    /// the end. The anchor belongs to the whole gesture — it is where the gesture *began* — so
    /// only a press with no Shift (which starts one) may move it.
    ///
    /// `target` is clamped and snapped to a character boundary, so a caller passing a byte
    /// offset computed from a pixel can never put the caret inside a character.
    pub fn select_with_modifiers(&mut self, target: usize, modifiers: crate::shortcut::Modifiers) {
        let target = floor_char_boundary(&self.text, target.min(self.text.len()));
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            // Adopt the caret as the anchor only when this extension is the *first* one; a
            // gesture already in flight keeps the anchor it started with.
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor_position);
            }
            self.cursor_position = target;
        } else {
            self.selection_start = None;
            self.cursor_position = target;
        }
        self.normalize_selection();
        self.base.request_redraw();
    }

    /// Places the caret at `pos` and starts a pointer selection.
    ///
    /// A press with no modifier begins a *new* selection anchored at the pressed character:
    /// the anchor is where the pointer went down, so dragging away from it selects the span
    /// between them (the behaviour every text field has). Holding Shift extends from the
    /// existing anchor instead, which is the pointer's version of the same modifier rule
    /// [`Self::select_with_modifiers`] applies to the keyboard.
    ///
    /// # Why the anchor is a **character index**, not a position
    ///
    /// `index` is a byte offset and is converted once, here, by [`Self::byte_index_at_x`].
    /// Pressing past the end of the value is not a no-op: the press anchors at the end and
    /// the following drag then selects backwards, which is how a user selects the tail of a
    /// value shorter than the field.
    pub fn press_at(&mut self, index: usize, modifiers: crate::shortcut::Modifiers) {
        let index = floor_char_boundary(&self.text, index.min(self.text.len()));
        let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT);
        if !shift {
            // A press with no Shift **starts** a gesture, so it re-anchors where it landed.
            self.selection_start = Some(index);
            self.cursor_position = index;
        } else {
            // A Shift-press **extends**: the anchor is kept (adopted from the caret when the
            // gesture is new) and the caret moves to where the pointer went down. Leaving the
            // caret alone here — an earlier revision did — made a shift-press a no-op that only
            // re-anchored, so `Shift`-clicking to the start of a value selected nothing and a
            // following drag grew from a point the user had not pressed.
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor_position);
            }
            self.cursor_position = index;
        }
        self.dragging_selection = true;
        // Deliberately **not** passed through `normalize_selection`.
        //
        // A press that lands exactly on the caret produces a zero-width anchor/caret pair, and
        // collapsing it to "no selection" is right for a *result* — but this is the *start* of a
        // gesture, and the anchor is the whole point: the drag that follows grows the range from
        // it. Normalising here deleted the anchor the press had just set, so a drag backwards over
        // the value selected nothing; the end of the gesture normalises instead.
        self.base.request_redraw();
    }

    /// Extends an in-progress pointer selection to the character at `index`.
    ///
    /// Only a gesture that began with [`Self::press_at`] continues here, so a bare hover —
    /// this control is not animated and a `MouseMove` arrives on every pointer step — cannot
    /// move a caret the user never grabbed. The anchor is left where the press put it.
    pub fn drag_to(&mut self, index: usize) {
        if !self.dragging_selection {
            return;
        }
        self.cursor_position = floor_char_boundary(&self.text, index.min(self.text.len()));
        self.normalize_selection();
        self.base.request_redraw();
    }

    /// Ends a pointer selection. Returns whether one was in progress.
    ///
    /// This is where a zero-width gesture is collapsed: a press that never moved leaves the caret
    /// exactly on its anchor, and a highlight of no width is not a selection. The collapse happens
    /// here, at the end, rather than in [`Self::press_at`] — see the note there, and
    /// [`Self::normalize_selection`].
    pub fn end_drag(&mut self) -> bool {
        let was_dragging = core::mem::replace(&mut self.dragging_selection, false);
        if was_dragging {
            self.normalize_selection();
        }
        was_dragging
    }

    /// Collapses a zero-width anchor/caret pair to "no selection".
    ///
    /// A caret that sits exactly on its anchor has selected nothing, but keeping the anchor
    /// set would make a later Shift-press extend from a position the user did not choose and
    /// leave `selected_text` reporting an empty range. Dropping it here keeps "there is a
    /// selection" and "`selected_text` is non-empty" the same statement.
    fn normalize_selection(&mut self) {
        if self.selection_start == Some(self.cursor_position) {
            self.selection_start = None;
        }
    }

    /// The byte offset of the character *before* the caret.
    fn previous_char_boundary(&self) -> usize {
        if self.cursor_position == 0 {
            return 0;
        }
        let caret = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
        self.text[..caret].char_indices().next_back().map(|(index, _)| index).unwrap_or(0)
    }

    /// The byte offset of the character *after* the caret.
    fn next_char_boundary(&self) -> usize {
        if self.cursor_position >= self.text.len() {
            return self.text.len();
        }
        let caret = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
        self.text[caret..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| caret + index)
            .unwrap_or(self.text.len())
    }

    /// The byte offset a horizontal pixel `x` points at.
    ///
    /// # How the pixel is turned into an index
    ///
    /// The value's own advance is measured with the renderer's shaper
    /// ([`RenderContext::shape_text`]) — the same call [`crate::render::fitted_origin`] uses to
    /// place the ink — and used as a **scale**, not as a character count:
    ///
    /// * when the value's clusters are all one advance wide (the crate's shipped face is
    ///   monospaced), `chars / advance == 1 / cell`, so `round(clicked_chars) - 1` is the
    ///   exact character the pointer is on, including out past the end of a short value;
    /// * when they are not, the round trip still yields `clicked_chars` — an advance-weighted
    ///   character index — which rounds to the nearest cluster the pointer falls on.
    ///
    /// Both cases come out of one expression, so there is no separate "is this face
    /// monospaced?" branch to keep in step with the paint path. The value is clipped to the
    /// same [`Self::field_rect`] box the paint and the press hit-test use, and the result is
    /// snapped to a character boundary, which is what keeps `&self.text[..caret]` total when
    /// the value contains multi-byte characters.
    fn byte_index_at_x(&self, x: i32) -> usize {
        if self.text.is_empty() {
            return 0;
        }
        let style = self.base.style();
        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        // The box the value is laid out in, derived the only way this file derives it: from the
        // decoration layout, so a prefix (`$`) and the value's own origin cannot disagree with
        // the paint (see the `layout` binding in `draw`, which comes from this same call).
        //
        // The zero-sized throwaway backend is a **measurement surface**: `decoration_layout`
        // only ever calls `measure_text`, and `shape_text` only ever reads the font, so neither
        // writes a command into it. It is zero-sized because a hit test must not rasterise a
        // frame, and constructing a full-size surface here would allocate one on every click.
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let mut context = RenderContext::new(&mut measurement);
        let layout = self.decoration_layout(&mut context);
        // The pointer's distance from the value's origin, clamped to the value's own box so a
        // press on the field's padding (or past the end of a short value) reads as "the
        // nearest end" rather than as a negative index.
        let dx = (x - layout.value.x).clamp(0, layout.value.width as i32) as f32;
        let advance = context.shape_text(&self.text, font).advance().max(1.0);
        let chars = self.text.chars().count().max(1) as f32;
        // `saturating_sub` is the whole point: this is a **boundary** count (a caret sits
        // *before* the character it is nearest), and a click past the last character must
        // saturate at the end of the value rather than index one past it.
        let boundary = ((dx * chars / advance).round() as usize).min(chars as usize);
        let boundary = boundary.saturating_sub(1);
        self.text.char_indices().nth(boundary).map(|(index, _)| index).unwrap_or(self.text.len())
    }
    /// Inserts text at cursor position.
    ///
    /// # The limit is in characters, so the room left is too
    ///
    /// `max_length` is a **character** budget (it is what the user sees and what the counter
    /// reports), but this compared it against `self.text.len()`, a byte length. On `"héllo"` — five
    /// characters, six bytes — a limit of six therefore computed `6 - 6 == 0` bytes of room and
    /// refused a perfectly legal insert. Counting both sides in characters makes the field accept
    /// exactly the value its own counter says it is allowed to hold.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Check max length and truncate if needed
        // The slice is taken through `byte_index_of_char` because the budget is in characters and
        // the slice is in bytes, so cutting at the budget's byte offset could split a character.
        let effective_text = if let Some(max) = self.max_length {
            let available = max.saturating_sub(self.char_count());
            if available == 0 {
                return;
            }
            let incoming = text.chars().count();
            if incoming > available {
                let boundary = byte_index_of_char(text, available);
                &text[..boundary]
            } else {
                text
            }
        } else {
            text
        };
        // Handle selection
        let mut new_text = self.text.clone();
        if let Some(start) = self.selection_start {
            let start = floor_char_boundary(&new_text, start.min(new_text.len()));
            let end = floor_char_boundary(&new_text, self.cursor_position.min(new_text.len()));
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            new_text.replace_range(start..end, effective_text);
            self.cursor_position = start + effective_text.len();
        } else {
            let at = floor_char_boundary(&new_text, self.cursor_position.min(new_text.len()));
            new_text.insert_str(at, effective_text);
            self.cursor_position = at + effective_text.len();
        }
        self.selection_start = None;
        self.set_text(new_text);
        // `set_text` may have clamped the value to `max_length`, which can move the caret it
        // recomputed; snap it back onto a boundary inside whatever the field now holds.
        self.clamp_caret();
    }
    /// Starts or updates an input-method composition without changing the field's value.
    ///
    /// # The three methods an IME session needs, and why they are a set (BLUE24 §12 U-5)
    ///
    /// A platform input method reports a **preedit** (the uncommitted string it is building) and then
    /// either commits it or cancels. Mapping that onto a text field takes exactly these three
    /// operations, and none of them is expressible as `insert_text`:
    ///
    /// * `set_composition` — the user is still typing. The field's **value must not change**: firing
    ///   `text_changed` on every keystroke of an unfinished word would run a form's validation
    ///   against text the user has not committed, which is the defect this separation exists to
    ///   prevent.
    /// * `commit_composition` — the input method accepted its candidate. Now the string enters the
    ///   model, through the ordinary edit path (so `max_length`, the selection replacement and
    ///   `text_changed` all apply as they do for any other insert).
    /// * `cancel_composition` — the user pressed Escape. The composition is dropped and the value is
    ///   exactly what it was, because it was never touched.
    ///
    /// An empty string clears the composition without committing it, which is what an input method
    /// sends when it withdraws.
    pub fn set_composition(&mut self, preedit: &str) {
        if preedit.is_empty() {
            self.composition = None;
        } else {
            self.composition = Some(preedit.to_string());
        }
        self.base.request_redraw();
    }

    /// The uncommitted composition, or `None` when no IME session is active.
    pub fn composition(&self) -> Option<&str> {
        self.composition.as_deref()
    }

    /// Whether an input-method composition is in progress.
    pub fn has_composition(&self) -> bool {
        self.composition.is_some()
    }

    /// Accepts the composition: the preedit becomes text, through the ordinary insert path.
    ///
    /// Returns `true` when something was committed. A commit with no active composition is a no-op
    /// rather than an error: an input method may commit and clear in either order, and a field that
    /// refused the second call would report a failure for a normal sequence.
    pub fn commit_composition(&mut self) -> bool {
        let Some(preedit) = self.composition.take() else {
            return false;
        };
        // Through `insert_text`, so `max_length`, the selection replacement and `text_changed`
        // behave exactly as they do for any other insertion — a committed composition is text.
        self.insert_text(&preedit);
        self.base.request_redraw();
        true
    }

    /// Drops the composition without changing the field's value.
    ///
    /// Returns `true` when there was one. The text is untouched **by construction**, not by
    /// restoring a snapshot: the composition was never written into it.
    pub fn cancel_composition(&mut self) -> bool {
        let had = self.composition.take().is_some();
        if had {
            self.base.request_redraw();
        }
        had
    }

    /// Deletes selected text or the character before the caret.
    ///
    /// # The deletion is one *character*, not one byte
    ///
    /// `new_text.remove(self.cursor_position - 1)` removes the byte before the caret, which on a
    /// multi-byte character deletes a fragment of it — and panics outright when the caret sits
    /// immediately after a multi-byte character, because `cursor_position - 1` is then inside that
    /// character and `String::remove` requires a boundary. Stepping back to the previous character's
    /// start deletes the whole thing, which is what a user means by “backspace”.
    pub fn backspace(&mut self) {
        if let Some(start) = self.selection_start {
            // Delete selection
            let start = floor_char_boundary(&self.text, start.min(self.text.len()));
            let end = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            if start == end {
                // An empty selection is not a deletion; fall through to the character case.
                self.selection_start = None;
                self.backspace();
                return;
            }
            let mut new_text = self.text.clone();
            new_text.replace_range(start..end, "");
            self.cursor_position = start;
            self.selection_start = None;
            self.set_text(new_text);
        } else if self.cursor_position > 0 {
            // Delete the character before the caret, from its own start to the caret.
            let caret = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let previous =
                self.text[..caret].char_indices().next_back().map(|(index, _)| index).unwrap_or(0);
            let mut new_text = self.text.clone();
            new_text.replace_range(previous..caret, "");
            self.cursor_position = previous;
            self.set_text(new_text);
        }
    }
    /// Deletes selected text or the character after the caret.
    pub fn delete(&mut self) {
        if let Some(_start) = self.selection_start {
            // Delete selection
            self.backspace(); // Same logic
        } else if self.cursor_position < self.text.len() {
            // Delete the *character* after the caret, not the single byte it starts on.
            let caret = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let next = self.text[caret..]
                .char_indices()
                .nth(1)
                .map(|(index, _)| caret + index)
                .unwrap_or(self.text.len());
            let mut new_text = self.text.clone();
            new_text.replace_range(caret..next, "");
            self.set_text(new_text);
        }
    }
    /// Returns whether the line edit is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Sets the read-only state.
    ///
    /// Repaints because the state is draw-visible: a read-only field suppresses the caret (see
    /// [`Draw`]), so toggling it without a redraw left the marker on a field that had just become
    /// uneditable — or, going the other way, left the user with no caret in a field they could now
    /// type into.
    pub fn set_read_only(&mut self, ro: bool) {
        if self.read_only == ro {
            return;
        }
        self.read_only = ro;
        self.base.request_redraw();
    }

    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }
    /// Copy the current selection to the platform clipboard (no-op when empty).
    #[cfg(not(alloc_frugal))]
    fn copy_selection_to_clipboard(&self) {
        let selection = self.selected_text();
        if !selection.is_empty() {
            crate::set_clipboard_text(&selection);
        }
    }
    /// Returns display text based on echo mode.
    ///
    /// `EchoMode` has exactly three modes, so this match is exhaustive without a
    /// catch-all: adding a mode to the canonical enum forces every renderer to
    /// decide what it looks like, instead of silently inheriting one.
    fn display_text(&self) -> String {
        match self.echo_mode {
            EchoMode::Normal => self.text.clone(),
            EchoMode::Password => "*".repeat(self.text.len()),
            EchoMode::NoEcho => String::new(),
        }
    }

    /// The field the control actually paints.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A text field is a *fixed-height band*: [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is the
    /// touch-sized content floor every field in this crate shares. Painting `rect` made a
    /// 240x120 census cell a 240x120 white slab — a rectangle pretending to be a field —
    /// and it also made `size_hint`'s 24 disagree with the ink by a factor of five.
    /// [`ControlMetrics::full_width_band`] keeps the full width, takes the field's own
    /// height and centres it, which is exactly what stops a 48 px field from drawing as a
    /// 120 px panel.
    ///
    /// Everything the control paints **and everything it hit-tests** is placed from this
    /// one box, so the clickable area cannot drift away from the ink.
    fn field_rect(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    // ---------------------------------------------------------------------------
    // Decoration slots
    // ---------------------------------------------------------------------------

    /// Returns the five decoration strings this field shows.
    pub fn decorations(&self) -> &DecorationSlots {
        &self.decorations
    }

    /// Replaces the whole decoration set at once.
    ///
    /// Prefer the individual setters when only one slot changes; this exists because a caller
    /// assembling a form from a document has all five at once and five separate calls would repaint
    /// five times.
    pub fn set_decorations(&mut self, decorations: DecorationSlots) {
        let changed = self.decorations != decorations;
        self.decorations = decorations;
        if changed {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Returns the unit marker written before the value.
    pub fn prefix(&self) -> &str {
        &self.decorations.prefix
    }

    /// Sets the unit marker written before the value.
    ///
    /// # Why this does not touch `text`
    ///
    /// The prefix is **chrome**, not content: it is drawn in its own box, so the caret can still sit at
    /// the start of what the user is editing and a select-all copies the value alone. Folding it into
    /// `text` would make all three of those wrong while looking the same on screen.
    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.prefix = prefix.into();
        self.set_decorations(next);
    }

    /// Returns the unit marker written after the value.
    pub fn suffix(&self) -> &str {
        &self.decorations.suffix
    }

    /// Sets the unit marker written after the value, anchored to the field's trailing edge.
    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.suffix = suffix.into();
        self.set_decorations(next);
    }

    /// Returns the quiet hint shown below the field.
    pub fn helper_text(&self) -> &str {
        &self.decorations.helper
    }

    /// Sets the quiet hint shown below the field, displaced by any error.
    pub fn set_helper_text(&mut self, helper: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.helper = helper.into();
        self.set_decorations(next);
    }

    /// Returns the refusal message shown below the field.
    pub fn error_text(&self) -> &str {
        &self.decorations.error
    }

    /// Sets the refusal message, which displaces the helper and paints in the theme's error colour.
    ///
    /// Pass an empty string to clear it. This does not *validate* anything — it is the caller's report
    /// — but [`Self::set_max_length`] does drive the counter and the over-limit state on its own,
    /// because the field knows both numbers itself.
    pub fn set_error_text(&mut self, error: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.error = error.into();
        self.set_decorations(next);
    }

    /// Returns the usage counter shown at the field's trailing lower edge, if any.
    ///
    /// Derived from the value's length and `max_length` rather than stored, so it cannot go stale: a
    /// counter the caller has to keep in step with the text is a counter that will disagree with it.
    pub fn counter_text(&self) -> Option<String> {
        DecorationLayout::counter_text(self.text.chars().count(), self.max_length)
    }

    /// Returns whether the current value exceeds `max_length`.
    ///
    /// The field's own answer, so an over-long value is *reported* rather than silently truncated.
    pub fn is_over_limit(&self) -> bool {
        DecorationLayout::over_limit(self.text.chars().count(), self.max_length) > 0
    }

    /// The boxes the field's five regions occupy, measured for the current font.
    ///
    /// # Why the layout is computed from `field_rect()` and not from `geometry()`
    ///
    /// The decoration slots belong to the **field**, which on a tall control is a band centred inside
    /// it. Measuring from the control's rectangle would put the helper row under the control instead of
    /// under the field, and the support row would be as wide as the cell rather than as the input.
    fn decoration_layout(&self, context: &mut RenderContext) -> DecorationLayout {
        let field = self.field_rect();
        let style = self.base.style().clone();
        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        let metrics = DecorationMetrics::measure(&self.decorations, |text| {
            context.measure_text(text, font).width
        });
        let counter_width =
            self.counter_text().map(|text| context.measure_text(&text, font).width).unwrap_or(0);
        let line_height = font.effective_line_height().max(1.0) as u32;
        DecorationLayout::compute(
            field,
            dimensions::TEXT_FIELD_PADDING_H,
            line_height,
            DECORATION_GAP,
            metrics,
            counter_width,
            &self.decorations,
        )
    }
}
// Implement Widget trait
impl Widget for LineEdit {
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
        // The shared estimate plus the caret's own trailing room, rather than `len() * 8 + 10`.
        // The 10 px was making the same point — a text field leaves the caret somewhere to be —
        // but the multiplier was a byte count, so any non-ASCII value asked for three times the
        // width it would draw.
        let text_w = estimate_text_width(self.text(), &crate::core::Font::default(), 1.0)
            + LINE_EDIT_CARET_ROOM;
        Size::new(text_w.max(LINE_EDIT_MIN_WIDTH), LINE_EDIT_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    // The caret blink is the whole animation: without a trait-visible `tick` a host holding
    // `&mut dyn Widget` could not advance it, so the caret never blinked on screen.
    fn tick(&mut self, delta_ms: u32) -> bool {
        LineEdit::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.focused && !self.read_only
    }

    /// Reports the **interaction** state: `Disabled` when inert, else pressed / hovered / focused.
    ///
    /// # Why the refusal is no longer reported here
    ///
    /// This used to return `Error` while the field carried a refusal message. That made the
    /// refusal and the interaction compete for one value, and the interaction won whenever the
    /// pointer was over the field — so `"line_edit:error"` stopped applying exactly when the user
    /// moved the pointer onto the field to see what was wrong. The meaning moved to
    /// [`Self::semantic_state`], which is orthogonal and so holds at the same time as `Hover`.
    ///
    /// `Disabled` still outranks everything: an inert field cannot be interacted with, so the
    /// interaction channel is what describes it, and the meaning channel reports the refusal
    /// separately.
    fn widget_state(&self) -> crate::style::WidgetState {
        use crate::style::WidgetState;
        if !self.base.is_enabled() {
            return WidgetState::Disabled;
        }
        if self.base.is_pressed() {
            WidgetState::Pressed
        } else if self.base.is_hovered() {
            WidgetState::Hover
        } else if self.base.draws_focus_ring() {
            WidgetState::Focused
        } else {
            WidgetState::Normal
        }
    }

    /// Reports the field's **meaning**: `Error` while it carries a refusal or is over its limit.
    ///
    /// The other half of the split [`Widget::widget_state`] documents. The two are independent, so
    /// a hovered refusal keeps the hover fill **and** the error border — which is what the plan's
    /// criterion asks for and what a single-valued chain could not express.
    fn semantic_state(&self) -> crate::style::SemanticState {
        if !self.decorations.error.is_empty() || self.is_over_limit() {
            return crate::style::SemanticState::Error;
        }
        crate::style::SemanticState::None
    }
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
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |_| {
                    CapabilityValue::Null
                }))
            }
            "editing_finished" => {
                Some(EventSignalRef::unit("editing_finished", &self.editing_finished))
            }
            "return_pressed" => Some(EventSignalRef::unit("return_pressed", &self.return_pressed)),
            _ => None,
        }
    }
}

/// `LineEdit`'s property contract.
///
/// # The decoration slots
///
/// `prefix`, `suffix`, `helper`, `error` and `counter` are all published here. They are the five strings
/// a text entry shows that are **not** its value, and before they existed the control could only be
/// announced and driven by its text and placeholder.
///
/// `counter` is published read-only: it is derived from the value's own length and `max_length`, so a
/// caller that could write it would be able to make the count disagree with the text it counts.
///
/// `echo_mode` is intentionally absent: the centralised layer never exposed it,
/// so publishing it here would add a property rather than preserve one.
impl WidgetProperties for LineEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder_text" => Ok(CapabilityValue::String(self.placeholder_text().to_string())),
            "max_length" => match self.max_length() {
                Some(len) => Ok(CapabilityValue::UInt(len as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "read_only" => Ok(CapabilityValue::Bool(self.is_read_only())),
            "cursor_position" => Ok(CapabilityValue::UInt(self.cursor_position() as u64)),
            "prefix" => Ok(CapabilityValue::String(self.prefix().to_string())),
            "suffix" => Ok(CapabilityValue::String(self.suffix().to_string())),
            "helper" => Ok(CapabilityValue::String(self.helper_text().to_string())),
            "error" => Ok(CapabilityValue::String(self.error_text().to_string())),
            "counter" => match self.counter_text() {
                Some(text) => Ok(CapabilityValue::String(text)),
                None => Ok(CapabilityValue::Null),
            },
            "over_limit" => Ok(CapabilityValue::Bool(self.is_over_limit())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder_text" => {
                self.set_placeholder_text(expect_string(value)?);
                Ok(())
            }
            "max_length" => {
                match value {
                    CapabilityValue::Null => self.set_max_length(None),
                    other => self.set_max_length(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "read_only" => {
                self.set_read_only(expect_bool(value)?);
                Ok(())
            }
            "prefix" => {
                self.set_prefix(expect_string(value)?);
                Ok(())
            }
            "suffix" => {
                self.set_suffix(expect_string(value)?);
                Ok(())
            }
            "helper" => {
                self.set_helper_text(expect_string(value)?);
                Ok(())
            }
            "error" => {
                self.set_error_text(expect_string(value)?);
                Ok(())
            }
            // Derived from the value and the limit: a writer would be a second way to say what the
            // text already determines, and one of the two would be able to disagree.
            "counter" | "over_limit" => Err(CapabilityAccessError::ReadOnlyProperty),
            "cursor_position" => {
                self.set_cursor_position(expect_usize(value)?);
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
        // Mirrors `LINE_EDIT_PROPERTIES`.
        property_names_of![
            "text",
            "placeholder_text",
            "max_length",
            "read_only",
            "cursor_position",
            "prefix",
            "suffix",
            "helper",
            "error",
            "counter",
            "over_limit",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `line_edit` publishes.
    ///
    /// Both are zero-argument actions with a direct method on the control, so both
    /// run rather than being routed through the property path: `clear` assigns the
    /// empty text and `select_all` selects without changing it.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "select_all" => {
                self.select_all();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl LineEdit {
    /// Routes a key event through the shared selection entry point instead of mutating
    /// `selection_start` directly.
    ///
    /// # Why the dispatch was pulled out of `handle_event`
    ///
    /// The four arrows and Home/End each carried their own copy of the shift test — six
    /// copies of one rule, each free to drift from the others. They now all call
    /// [`LineEdit::select_with_modifiers`] (the same entry point the pointer gesture uses),
    /// so what Shift means is written once (principle #101: one concept, one path).
    ///
    /// The helper is inherent rather than a trait method because `EventHandler` declares
    /// only `handle_event`; the trait impl below is a thin façade over it.
    ///
    /// Returns whether the key was consumed. A key this field does not handle leaves the
    /// event to the rest of the chain rather than being silently swallowed.
    fn handle_key(&mut self, key: u32, modifiers: crate::shortcut::Modifiers) -> bool {
        match key {
            8 => {
                // Backspace
                self.backspace();
            }
            46 => {
                // Delete
                self.delete();
            }
            13 => {
                // Enter/Return
                self.return_pressed.emit();
                self.editing_finished.emit();
            }
            27 => {
                // Escape
                self.editing_finished.emit();
            }
            37 => {
                // Left arrow. A plain press deselects and steps one character back; a
                // Shift press extends from the anchor rather than replacing it, so the
                // range grows instead of collapsing to a caret (see `select_with_modifiers`).
                if self.cursor_position > 0 {
                    let target = self.previous_char_boundary();
                    self.select_with_modifiers(target, modifiers);
                } else if !modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
                    // Already at the start, and no Shift: the key cannot move the caret, but it
                    // still owes the user the deselect every other plain press performs. It used
                    // to be dropped entirely, which left a stale range alive after an arrow press
                    // that plainly meant "just move". A Shift press here has nothing to extend to.
                    self.clear_selection();
                    self.base.request_redraw();
                }
            }
            39 => {
                // Right arrow. Same contract as Left, stepping forward.
                if self.cursor_position < self.text.len() {
                    let target = self.next_char_boundary();
                    self.select_with_modifiers(target, modifiers);
                } else if !modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
                    // Already at the end: see the Left arm — the deselect still applies.
                    self.clear_selection();
                    self.base.request_redraw();
                }
            }
            36 => {
                // Home
                self.select_with_modifiers(0, modifiers);
            }
            35 => {
                // End
                self.select_with_modifiers(self.text.len(), modifiers);
            }
            65 if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) => {
                // Primary+A: Select all
                self.select_all();
            }
            86 if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) => {
                // Primary+V: Paste from the platform clipboard.
                #[cfg(not(alloc_frugal))]
                {
                    let pasted = crate::get_clipboard_text();
                    if !pasted.is_empty() {
                        self.insert_text(&pasted);
                    }
                }
                #[cfg(alloc_frugal)]
                {
                    // Clipboard integration is unavailable in the mini profile.
                    self.base.redraw_requested.emit();
                }
            }
            67 if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) => {
                // Primary+C: Copy selection to the platform clipboard.
                #[cfg(not(alloc_frugal))]
                {
                    self.copy_selection_to_clipboard();
                }
                #[cfg(alloc_frugal)]
                {
                    // Clipboard integration is unavailable in the mini profile.
                    self.base.redraw_requested.emit();
                }
            }
            88 if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) => {
                // Primary+X: Copy selection, then delete it.
                #[cfg(not(alloc_frugal))]
                {
                    let selection = self.selected_text();
                    if !selection.is_empty() {
                        crate::set_clipboard_text(&selection);
                        self.backspace(); // removes the selection
                    }
                }
                #[cfg(alloc_frugal)]
                {
                    // Clipboard integration is unavailable in the mini profile.
                    self.base.redraw_requested.emit();
                }
            }
            90 if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) => {
                let _ = self.undo();
            }
            89 if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) => {
                let _ = self.redo();
            }
            _ => {
                // Character input. A control chord must not be swallowed as text, nor may an
                // unmapped **non-printable** key be: both used to be typed as their bare
                // character, so `Ctrl+B` inserted `b` and `Ctrl+Z` inserted `z`.
                //
                // The two chords this field swaps the meaning of are the right ones for the
                // host: undo/redo are `Primary+Z`/`Primary+Y` (handled above), and the
                // accelerator convention is `from_event_bits_primary_is_ctrl`.
                // Character input. A control chord must not be swallowed as text, nor may an
                // unmapped **non-printable** key be: both used to be typed as their bare
                // character, so `Ctrl+B` inserted `b` and `Ctrl+Z` inserted `z`.
                //
                // # Why the three modifiers are tested one at a time
                //
                // Combining them with `|` and asking `contains` is not the predicate it reads
                // as: `Modifiers::contains` compares the *value* of the mask, so
                // `contains(CTRL | ALT | META)` is true only when **all three** are held at once.
                // A `Ctrl+B` carries CTRL alone and therefore failed the combined test, which is
                // exactly the defect this arm exists to stop. Three separate `contains` calls say
                // what is meant: none of the three is held.
                let chord_held = modifiers.contains(crate::shortcut::Modifiers::CTRL)
                    || modifiers.contains(crate::shortcut::Modifiers::ALT)
                    || modifiers.contains(crate::shortcut::Modifiers::META);
                if !chord_held {
                    // Only a **printable** key is content. `char::from_u32(0)` is `\0`, and 9, 10
                    // and 13 are Tab/Enter/CR — none of them is something a user typed into a
                    // text field, and all of them are control codes the host owns.
                    if let Some(ch) = char::from_u32(key).filter(|c| !c.is_control()) {
                        if ch.is_ascii_graphic() || ch == ' ' {
                            self.insert_text(&ch.to_string());
                        }
                    }
                }
            }
        }
        true
    }
}

impl EventHandler for LineEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.read_only {
            // Read-only fields still allow copying the current selection.
            #[cfg(not(alloc_frugal))]
            if let Event::KeyPress { key: 67, modifiers: 2 } = event {
                self.copy_selection_to_clipboard();
            }
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => {
                self.handle_key(*key, crate::shortcut::Modifiers::from_event_bits(*modifiers));
            }
            Event::FocusLost => {
                self.set_focused(false);
                self.editing_finished.emit();
            }
            Event::FocusGained { .. } => {
                self.set_focused(true);
            }
            // A press focuses the field only when it lands on the **painted band**, not on
            // the control's rectangle. The two used to be the same box, so the hit test
            // silently agreed with the ink by accident; once the field became a centred
            // 48 px band in a 120 px cell, testing `geometry()` would let a user focus the
            // field by clicking 60 px below it — on the window background, nowhere near
            // any ink. The test is against `field_rect()` for exactly that reason.
            Event::MousePress { pos, button, modifiers, .. }
                if *button == 1 && self.field_rect().contains_point(*pos) =>
            {
                self.set_focused(true);
                // A press places the caret where the user aimed and starts a selection.
                //
                // Previously a press only focused the field, so the caret stayed wherever it
                // happened to be — `set_text` leaves it at the end, so clicking into the middle
                // of an existing value and typing appended instead of inserting, and there was
                // no way at all to select with the pointer. `press_at` is the pointer's
                // counterpart to `select_with_modifiers`: it anchors where the press landed,
                // and a following drag extends without a key being held, because a drag *is*
                // the gesture that means "extend".
                let index = self.byte_index_at_x(pos.x);
                self.press_at(index, crate::shortcut::Modifiers::from_event_bits(*modifiers));
            }
            // A drag extends the selection the press began, and only then — see `drag_to`.
            Event::MouseMove { pos } => {
                if self.dragging_selection {
                    let index = self.byte_index_at_x(pos.x);
                    self.drag_to(index);
                }
            }
            // A release anywhere ends the gesture, including outside the field: the anchor
            // stays where the press put it so the selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.end_drag();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for LineEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        //
        // The **field**, not the control's rectangle: see `field_rect`. Every measurement
        // below — the fill, the border, the text's line box and the caret's top and bottom
        // — is taken from this one box, so they cannot disagree about where the field is.
        let rect = self.field_rect();
        let style = self.style();
        // ── The decorated layout ──
        //
        // Every box below comes from **one** derivation, measured against the renderer's own font. The
        // value's origin, the caret, the two slots and the support row cannot disagree, because they
        // are the same answer read for different purposes. The value's origin used to be
        // `rect.x + padding` regardless of any slot, so a `$` would have been drawn *over* the value
        // it marks; and the caret measured from that same fixed inset, so it sat at the wrong end of
        // a centred or right-aligned field.
        let layout = self.decoration_layout(context);
        // Draw background
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.face(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            bg,
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Draw border
        //
        // The **border** answers to the meaning channel, the fill above to the interaction one.
        // A refusal therefore keeps its outline while the pointer is over the field, which is the
        // one moment the outline is what the user is looking for. When the control carries no
        // meaning the border falls back to the interaction style's own colour, so a control with
        // `semantic_state() == None` draws exactly as it did before the two channels were split.
        let semantic_border =
            crate::style::resolved_semantic_border("line_edit", self.semantic_state());
        if let Some(border_color) = semantic_border.or(style.border_color) {
            let bw = style.border_width.unwrap_or(0);
            if bw > 0 {
                context.draw_rect_stroke(
                    Rect::new(rect.x, rect.y, rect.width, rect.height),
                    border_color,
                    bw,
                );
            } else {
                context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), border_color);
            }
        }
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.display_text()
        };
        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        let value_line = context.text_line(rect, font);
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        // The box the value is laid out in, and the alignment it uses within that box.
        //
        // Resolved **once**, outside the `if` that draws the string, because the caret below needs
        // the exact same two facts: it rides with the glyphs, so it must be placed from the origin
        // the glyphs were placed from. Computing them in two places is how the caret and the value
        // drifted apart in the first place.
        //
        // `draw_text_fitted` insets its box by `TEXT_FIT_MARGIN` at each end, which is right for
        // a label that must not touch its frame but wrong for a field value: left-aligned ink
        // would then begin `TEXT_FIT_MARGIN` past the field's own padding. Handing it a box
        // widened by that same inset on both sides puts the left edge back on the padding, so
        // `Left` is byte-identical to the pre-alignment `draw_text` and the other two alignments
        // measure from the same true box.
        let fit = crate::render::TEXT_FIT_MARGIN as i32;
        let value_box = Rect::new(
            layout.value.x - fit,
            value_line.y,
            layout.value.width + (fit * 2) as u32,
            value_line.height,
        );
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        if !display_text.is_empty() {
            // The field's own line box. A glyph origin is the box's top-left edge, so the
            // previous `rect.y + rect.height / 2` placed that edge on the field's middle line
            // and drew the value half a line low.
            context.draw_text_fitted(value_box, display_text, font, text_color, value_align);
        }

        // ── The in-field slots ──
        //
        // Drawn in their **own** boxes, in a muted tone of the field's ink so they read as unit marks
        // rather than as content. They are deliberately not folded into `display_text`: the caret below
        // is measured against `self.text` alone, so a prefix in the string would put `cursor_position
        // == 0` after the `$` and make the start of the value unreachable.
        let slot_color = text_color.blend(&bg, 0.35);
        if let Some(prefix_box) = layout.prefix {
            context.draw_text(
                Point::new(prefix_box.x, value_line.y),
                &self.decorations.prefix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }
        if let Some(suffix_box) = layout.suffix {
            context.draw_text(
                Point::new(suffix_box.x, value_line.y),
                &self.decorations.suffix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }

        // ── The support row ──
        //
        // Message on the leading edge, counter on the trailing one. The error takes the theme's error
        // colour, because a refusal that is painted in the same ink as a hint is a refusal the user has
        // to read to notice.
        if let Some(row) = layout.support {
            let message = self.decorations.support_message();
            if !message.is_empty() {
                let message_color = if self.decorations.has_error() {
                    crate::style::resolved_theme_style("line_edit")
                        .and_then(|theme| theme.border_color)
                        .map(|border| border.blend(&Color::rgb(220, 40, 40), 0.6))
                        .unwrap_or(Color::rgb(200, 40, 40))
                } else {
                    slot_color
                };
                let row_line = context.text_line(row, font);
                context.draw_text(
                    Point::new(row.x, row_line.y),
                    message,
                    font,
                    message_color,
                    HorizontalAlignment::Left,
                );
            }
            if let Some(counter_box) = layout.counter {
                let counter_line = context.text_line(counter_box, font);
                // The counter is a *budget* reading, so it turns to the error colour once the value
                // exceeds the limit — the one moment it has something to warn about.
                let counter_color =
                    if self.is_over_limit() { Color::rgb(200, 40, 40) } else { slot_color };
                if let Some(counter) = self.counter_text() {
                    context.draw_text(
                        Point::new(counter_box.x, counter_line.y),
                        &counter,
                        font,
                        counter_color,
                        HorizontalAlignment::Right,
                    );
                }
            }
        }
        // Draw the caret for whichever field owns keyboard focus.
        if self.focused && !self.read_only && self.cursor_blink.is_visible() {
            // The caret sits at **`cursor_position`**, not at the end of the value.
            //
            // This measured the whole string, so the marker was always drawn after the last
            // glyph however the control was positioned: with `text == "Sample"` and
            // `cursor_position == 0`, the caret rendered past the `e`. The field answers
            // `cursor_position`, `set_cursor_position` clamps it, and `backspace`/`delete`
            // both act on it — so the cursor was the one thing in the field that ignored the
            // very field that defines it, and every editing affordance appeared to work at the
            // wrong end of the text.
            //
            // The slice is taken through `floor_char_boundary` because `cursor_position` indexes
            // bytes and the value may be multi-byte; slicing mid-character would panic, which is
            // why the import for it was already present in this file.
            //
            // # Why the origin is the same `fitted_origin` the value was drawn with
            //
            // A centred or right-aligned value does **not** start at `text_x`: the fit path lays it
            // out from `fitted_origin(value_box, advance, alignment)`. Measuring the caret from
            // `text_x` therefore put the marker at the left edge of a right-aligned field while its
            // own text sat at the right edge. Rather than duplicate that arithmetic here — the two
            // would drift exactly as the value's origin and the caret's once did — the whole value's
            // advance is measured and run through the same origin function, so the caret rides with
            // the glyphs it belongs to.
            let caret_byte = floor_char_boundary(self.text.as_str(), self.cursor_position);
            let prefix = &self.text[..caret_byte];
            // The value's own advance, run through the same `fitted_origin` `draw_text_fitted`
            // uses, so a centred or right-aligned value keeps its caret on the glyphs instead of at
            // the field's leading edge.
            let value_advance = context.measure_text(display_text, font).width as i32;
            let value_origin = crate::render::fitted_origin(value_box, value_advance, value_align);
            let prefix_advance = context.measure_text(prefix, font).width as i32;
            let caret_x = value_origin.x + prefix_advance;
            // The marker is clipped to the **value's** box. A caret beyond the visible text (a value
            // wider than the room it has) belongs at the last pixel a user can see, not outside the
            // control and not under the suffix. Clipping to the whole field instead would let the caret
            // sit on top of a `%` and read as if the unit were part of the value.
            let caret_x = caret_x.min(layout.value.x + layout.value.width as i32);
            let caret_x = caret_x.max(layout.value.x);
            // The caret spans the field's own content band, inset so it does not touch the
            // border. The band is the field's, not the control's: with the field centred in
            // a tall cell, `rect` alone would have drawn a caret taller than the field it
            // belongs to.
            let caret_top = rect.y + 2;
            let caret_bottom = rect.y + rect.height as i32 - 2;
            let caret_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
            context.draw_line(
                Point::new(caret_x, caret_top),
                Point::new(caret_x, caret_bottom),
                caret_color,
            );

            // The uncommitted composition is drawn **after the caret**, where it will land when it is
            // committed, with an underline marking it as preedit.
            //
            // # Why the field draws this and not only `ImePreedit`
            //
            // `ImePreedit` is an overlay: a host that mounts one gets a composition string with an
            // underline, in a box of its own. But a user typing into a field expects the composition
            // **inline**, at the caret, growing as they type — that is what makes an input method
            // legible. Drawing it here is possible only because the preedit is now part of the
            // field's model (`composition`); the overlay could never place it, because it does not
            // know where this field's caret is.
            //
            // The advance is measured from the **same** `prefix` the caret uses, so the underline
            // begins exactly at the caret and cannot drift from it.
            if let Some(preedit) = self.composition.as_deref() {
                let preedit_x = caret_x;
                let preedit_width = context.measure_text(preedit, font).width as i32;
                context.draw_text(
                    Point::new(preedit_x, value_line.y),
                    preedit,
                    font,
                    caret_color,
                    crate::core::HorizontalAlignment::Left,
                );
                // The underline is one pixel below the glyph box, over the preedit's own width.
                let underline_y = value_line.y + context.measure_text("M", font).height as i32 + 1;
                context.draw_line(
                    Point::new(preedit_x, underline_y),
                    Point::new(preedit_x + preedit_width, underline_y),
                    caret_color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{MiniToString, Vec};
    use crate::core::Rect;

    /// The bitmap face fills its glyph box, so its ink left *is* the box left. An outline face draws
    /// a real glyph whose ink is inset, so the ink's left edge sits one or two pixels right of the
    /// box edge at these sizes. Pinning the two equal encoded a property of the bitmap face, not of
    /// the layout.
    const INK_INSET_TOLERANCE: i32 = 3;

    #[test]
    fn lineedit_creation_defaults() {
        let le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.text().is_empty());
        assert!(le.placeholder_text().is_empty());
        assert_eq!(le.max_length(), None);
        assert_eq!(le.echo_mode(), EchoMode::Normal);
        assert_eq!(le.cursor_position(), 0);
        assert!(le.selection_start().is_none());
        assert!(!le.is_focused(), "a fresh field is not focused");
    }

    /// The caret blinks only while the field holds focus, and it stops asking for frames when it
    /// does not.
    ///
    /// The caret was a solid line for the whole life of the field, so `tick` did not exist and a
    /// host had no signal about whether more frames were owed. This pins the contract in both
    /// directions: a blurred field is inert, and a focused one keeps reporting work until it
    /// loses focus again.
    #[test]
    fn the_caret_blinks_while_focused_and_is_inert_while_blurred() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

        // Blurred: no frames owed, whatever the delta.
        assert!(!le.tick(10_000), "a blurred field owes no frame");

        // Focused: it reports work, and keeps doing so because a blink never settles.
        le.set_focused(true);
        assert!(le.tick(0), "a focused field is blinking");
        for _ in 0..10 {
            assert!(le.tick(500), "a blink is periodic, so it never reports settled");
        }

        // A read-only field shows no caret, so it must not animate one either.
        le.set_focused(false);
        assert!(!le.tick(500));
    }

    /// The field must learn about focus from the events the runtime sends, since
    /// that is what gates the caret. Before this the field had no focus state, so
    /// `FocusGained` was ignored and the caret was never drawn.
    #[test]
    fn lineedit_tracks_focus_from_events() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

        le.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(le.is_focused(), "FocusGained must mark the field focused");

        le.handle_event(&Event::FocusLost);
        assert!(!le.is_focused(), "FocusLost must clear it");
    }

    /// Setting focus directly must be idempotent in effect: the flag is a boolean,
    /// so a repeated set cannot leave it in a contradictory state.
    #[test]
    fn lineedit_set_focused_round_trips() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_focused(true);
        assert!(le.is_focused());
        le.set_focused(true);
        assert!(le.is_focused());
        le.set_focused(false);
        assert!(!le.is_focused());
    }

    /// A focused field must actually paint a caret. The previous code drew nothing,
    /// and the comment said why — there was no focus state to read.
    #[test]
    fn focused_lineedit_draws_a_caret() {
        let rect = Rect::new(0, 0, 200, 24);
        let mut focused = LineEdit::new(rect);
        focused.set_text("ab");
        focused.set_focused(true);

        let mut unfocused = LineEdit::new(rect);
        unfocused.set_text("ab");

        let focused_frame = render(&mut focused, rect);
        let unfocused_frame = render(&mut unfocused, rect);
        assert_ne!(
            focused_frame, unfocused_frame,
            "focusing must change what is painted, because it draws the caret"
        );
    }

    /// Renders a widget into an RGBA frame for comparison.
    fn render(widget: &mut LineEdit, rect: Rect) -> Vec<u8> {
        use crate::core::{Color, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::Draw;

        let mut surface = SoftwarePaintBackend::new(Size::new(rect.width, rect.height), 1.0);
        surface.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut surface);
            widget.draw(&mut context);
        }
        surface.end_frame();
        surface.frame_rgba().to_vec()
    }

    #[test]
    fn lineedit_set_text() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        assert_eq!(le.text(), "Hello");
        assert_eq!(le.cursor_position(), 5);
    }

    #[test]
    fn lineedit_set_text_empty() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.set_text(String::new());
        assert!(le.text().is_empty());
        assert_eq!(le.cursor_position(), 0);
    }

    #[test]
    fn lineedit_undo_redo_restores_text() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("one");
        le.set_text("two");
        assert!(le.can_undo());
        assert!(le.undo());
        assert_eq!(le.text(), "one");
        assert!(le.can_redo());
        assert!(le.redo());
        assert_eq!(le.text(), "two");
    }

    #[test]
    fn lineedit_control_z_and_control_y_drive_history() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("before");
        le.set_text("after");
        // Bit 3 is the event's Meta/Command bit, i.e. the portable "primary accelerator"
        // (`Modifiers::from_event_bits`). The physical-Control bit is bit 1 and means Control.
        le.handle_event(&Event::key_press(90, 0b1000));
        assert_eq!(le.text(), "before");
        le.handle_event(&Event::key_press(89, 0b1000));
        assert_eq!(le.text(), "after");
    }

    #[test]
    fn lineedit_placeholder() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.placeholder_text().is_empty());
        le.set_placeholder_text("Enter name".to_string());
        assert_eq!(le.placeholder_text(), "Enter name");
        le.set_placeholder_text(String::new());
        assert!(le.placeholder_text().is_empty());
    }

    #[test]
    fn lineedit_max_length() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert_eq!(le.max_length(), None);
        le.set_max_length(Some(5));
        assert_eq!(le.max_length(), Some(5));
        le.set_max_length(None);
        assert_eq!(le.max_length(), None);
    }

    #[test]
    fn lineedit_max_length_truncates() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello World".to_string());
        le.set_max_length(Some(5));
        assert_eq!(le.text(), "Hello");
    }

    #[test]
    fn lineedit_cursor_position() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.set_cursor_position(3);
        assert_eq!(le.cursor_position(), 3);
        le.set_cursor_position(100); // clamps to text len
        assert_eq!(le.cursor_position(), 5);
    }

    #[test]
    fn lineedit_echo_mode() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert_eq!(le.echo_mode(), EchoMode::Normal);
        le.set_echo_mode(EchoMode::Password);
        assert_eq!(le.echo_mode(), EchoMode::Password);
        le.set_echo_mode(EchoMode::NoEcho);
        assert_eq!(le.echo_mode(), EchoMode::NoEcho);
        le.set_echo_mode(EchoMode::Normal);
        assert_eq!(le.echo_mode(), EchoMode::Normal);
    }

    /// The widget layer and the platform layer must name the **same** echo-mode
    /// enum, so a mode read back from a native control can be handed straight to
    /// this widget. Distinct types would make the assignment below ill-typed.
    #[test]
    fn lineedit_shares_the_canonical_echo_mode_type() {
        let canonical: crate::platform::EchoMode = crate::platform::EchoMode::Password;
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_echo_mode(canonical);
        assert_eq!(le.echo_mode(), canonical);
    }

    #[test]
    fn lineedit_select_all() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello World".to_string());
        le.select_all();
        assert_eq!(le.selection_start(), Some(0));
        assert_eq!(le.cursor_position(), 11);
        assert_eq!(le.selected_text(), "Hello World");
    }

    #[test]
    fn lineedit_clear_selection() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.select_all();
        le.clear_selection();
        assert!(le.selection_start().is_none());
    }

    // ─── Shift-arrow selection (BLUE24 §12 U-6) ───

    /// A Shift-arrow **extends** the selection; a plain arrow replaces it.
    ///
    /// # The defect this pins
    ///
    /// The anchor rule was written out once per arrow key, six copies of one decision. Beyond the
    /// duplication, the copies meant "Shift extends" lived only in the event loop — nothing else
    /// could reach it, so the pointer gesture and the entry point a host calls had no way to
    /// express a range. The assertions are on the *range*, not on the anchor, because a range is
    /// what the user sees selected: an implementation that moved the caret and forgot the anchor
    /// would still satisfy `cursor_position` and silently select nothing.
    ///
    /// The modifiers are spelled as the **event bitmask** (see
    /// [`crate::shortcut::Modifiers`]), not as `Modifiers` bits: an `Event` carries the framework's
    /// wire convention, and the field translates it with `from_event_bits`.
    #[test]
    fn a_shift_arrow_extends_and_a_plain_arrow_replaces() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("abcdef".to_string()); // caret at the end

        // Shift+Left twice grows a two-character selection, without collapsing to a caret.
        le.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(le.selected_text(), "f", "the first Shift+Left selects the character behind");
        le.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(le.selected_text(), "ef", "the second extends the same range");

        // A plain Left keeps the caret where it is and drops the selection entirely.
        le.handle_event(&Event::key_press(37, 0));
        assert_eq!(le.selection_start(), None, "a plain arrow is not an extend");
        assert_eq!(le.cursor_position(), 3, "and the caret is unmoved by the deselect");
    }

    /// Shift+Home and Shift+End select from the caret to the ends of the value.
    #[test]
    fn a_shift_home_and_end_select_to_the_ends_of_the_value() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("abcdef".to_string());
        le.set_cursor_position(2);

        le.handle_event(&Event::key_press(35, SHIFT_BIT));
        assert_eq!(le.selected_text(), "cdef", "Shift+End runs from the anchor to the end");

        // Shift+Home extends from the *same* anchor at 2, back past the caret to the start: the
        // range becomes 0..=2, the other side of the same anchor rather than a rewrite of it.
        le.handle_event(&Event::key_press(36, SHIFT_BIT));
        assert_eq!(le.cursor_position(), 0, "the caret walks to the start");
        assert_eq!(le.selection_start(), Some(2), "the anchor is unchanged by the second key");
        assert_eq!(le.selected_text(), "ab", "so the selection flips to the other side of it");

        // Home with no modifier collapses the range, the same as any plain movement.
        le.handle_event(&Event::key_press(36, 0));
        assert_eq!(le.selection_start(), None);
        assert_eq!(le.cursor_position(), 0);
    }

    /// Shift+Home on a field that has never been selected anchors at the live caret.
    ///
    /// The anchor is adopted from the caret when there is none, rather than assumed to be 0. A
    /// field whose caret sits at the end would otherwise select the whole value for a gesture the
    /// user aimed at the start of the line they were on.
    #[test]
    fn a_shift_move_with_no_anchor_anchors_at_the_caret() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("abcdef".to_string());
        le.set_cursor_position(3);
        assert_eq!(le.selection_start(), None, "a fresh caret is not an anchor");

        le.handle_event(&Event::key_press(36, SHIFT_BIT));
        assert_eq!(le.cursor_position(), 0, "Shift+Home walks the caret to the start");
        assert_eq!(le.selection_start(), Some(3), "adopting the caret as the anchor, not 0");
        assert_eq!(le.selected_text(), "abc", "so the range is the caret's own line prefix");
    }

    /// A Shift-arrow does not move the anchor once a selection exists, so the range grows from a
    /// fixed point instead of sliding along with the caret.
    #[test]
    fn an_extended_range_grows_from_a_fixed_anchor() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("abcdef".to_string());
        le.set_cursor_position(3);
        // The caret steps back one *character per press*, so the anchor stays at 3 while the
        // caret walks 3 -> 2 -> 1 and the range grows `"c"`, `"bc"`.
        le.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(le.cursor_position(), 2, "one press is one character");
        assert_eq!(le.selection_start(), Some(3), "the anchor is where the gesture began");
        assert_eq!(le.selected_text(), "c", "so the first step selects one character");
        le.handle_event(&Event::key_press(37, SHIFT_BIT));
        assert_eq!(le.selection_start(), Some(3), "and it does not slide with the caret");
        assert_eq!(le.selected_text(), "bc", "so the range is anchor-to-caret");
        assert_eq!(le.cursor_position(), 1);
    }

    /// An extension is **reversible**: Shift+Home then Shift+End returns the range to the anchor.
    ///
    /// # The defect this pins
    ///
    /// Home and End re-anchored at the caret before moving, so a second extension grew from the
    /// wrong end: Shift+Home on a caret at 5 selected 0..=5 and moved the anchor to 5; the following
    /// Shift+End then selected 5..=len instead of 0..=len. The anchor belongs to the *gesture*, and
    /// only a press that begins one may move it.
    #[test]
    fn extending_back_and_forth_returns_to_the_anchor() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("abcdefgh".to_string());
        le.set_cursor_position(5);

        le.handle_event(&Event::key_press(36, SHIFT_BIT));
        assert_eq!(le.cursor_position(), 0, "Shift+Home walks the caret to the start");
        assert_eq!(le.selected_text(), "abcde", "the range is anchor(5)-to-caret(0)");
        assert_eq!(le.selection_start(), Some(5), "the anchor is still where it began");

        // The gesture is reversible: Shift+End **extends** from the same anchor, so the range
        // flips to the other side of it rather than being replaced with 5..=len. This is the
        // assertion that fails if Home/End copy the old `selection_start = Some(cursor)` rule.
        le.handle_event(&Event::key_press(35, SHIFT_BIT));
        assert_eq!(le.cursor_position(), 8, "Shift+End walks the caret to the end");
        assert_eq!(le.selection_start(), Some(5), "the anchor never moved");
        assert_eq!(le.selected_text(), "fgh", "so the range is anchor(5)-to-caret(8)");
    }

    /// Movement saturates rather than running off either end of the value.
    ///
    /// # The defect this pins
    ///
    /// The arrows used to be guarded by `if cursor > 0` / `if cursor < len`, so at an end the key was
    /// *dropped* — including the deselect a plain press owes the user. A ragged selection then
    /// survived an arrow press that plainly meant "just move". Saturation keeps the caret in range
    /// and keeps the plain-press contract intact.
    #[test]
    fn movement_at_an_end_still_drops_the_selection() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("abc".to_string());

        // Caret already at the end: Right cannot move, but it still ends the selection.
        le.select_all();
        assert_eq!(le.selection_start(), Some(0));
        le.handle_event(&Event::key_press(39, 0));
        assert_eq!(le.cursor_position(), 3, "the caret stays inside the value");
        assert_eq!(le.selection_start(), None, "a plain press at the end still deselects");

        // Caret already at the start with a range straddling it: Left cannot move, but the same
        // deselect is owed. The range is built the way a shift-click does — press at the end,
        // then shift-press at the start — so the caret lands on 0 with the anchor at 3.
        le.select_all();
        le.press_at(3, crate::shortcut::Modifiers::NONE);
        le.press_at(0, crate::shortcut::Modifiers::SHIFT);
        assert_eq!(le.cursor_position(), 0);
        assert_eq!(le.selection_start(), Some(3), "a real range straddling the start");
        assert_eq!(le.selected_text(), "abc");

        le.handle_event(&Event::key_press(37, 0));
        assert_eq!(le.cursor_position(), 0, "Left cannot move past the start");
        assert_eq!(le.selection_start(), None, "but it still drops the stale range");
        assert_eq!(le.selected_text(), "", "so nothing is left selected");
    }

    /// A control chord is not text: `Ctrl+B` must not insert a literal `b`.
    ///
    /// # The defect this pins
    ///
    /// The catch-all arm treated every unmapped key as a character and only excluded the chords it
    /// happened to list (copy, paste, cut, undo, redo). So Ctrl+B, Ctrl+Q, Alt+1 and every other
    /// shortcut the host may own were typed into the field as their bare key. A field that swallows
    /// the application's own accelerators is unusable as a text input in a real form.
    ///
    /// The comment describes Ctrl/Alt/Meta, and the event convention says the Meta bit is the
    /// primary accelerator — so those are the bits asserted, not `Modifiers` bits.
    #[test]
    fn a_control_chord_is_not_typed_into_the_field() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

        // Bit 1 is physical Control, bit 2 is Alt, bit 3 is the primary accelerator (Command on
        // macOS). `from_event_bits` folds bit 3 into *both* PRIMARY and CTRL, and `contains`
        // compares the mask's value rather than testing a bit, so the guard must ask about the
        // three modifiers separately — a combined `contains(CTRL|ALT|META)` would only fire when
        // all three were held, letting a plain Ctrl chord through.
        for (name, bits) in [("Ctrl", 0b0010u32), ("Alt", 0b0100), ("Primary", 0b1000)] {
            le.handle_event(&Event::key_press(98, bits));
            assert!(le.text().is_empty(), "{name}+B must not insert a `b`");
        }
        // And a control code with no modifier at all is not content either: `key: 9` is Tab.
        le.handle_event(&Event::key_press(9, 0));
        assert!(le.text().is_empty(), "a bare Tab is not text");

        // The unmodified key is still text, so the guard did not disable typing. The key codes
        // are ASCII, so 66 is the uppercase `B` and 98 the lowercase `b`.
        le.handle_event(&Event::key_press(98, 0));
        assert_eq!(le.text(), "b", "a plain `b` is content");
    }

    /// The framework's Shift bit on an event mask.
    const SHIFT_BIT: u32 = 0b0001;

    // ─── Pointer selection ───

    /// A press puts the caret where the user aimed and anchors a selection there.
    ///
    /// # The defect this pins
    ///
    /// A press used to *only* focus the field. `set_text` leaves the caret at the end, so clicking
    /// into the middle of an existing value and typing appended to it rather than inserting at the
    /// click — and there was no pointer selection at all, because nothing computed an index from `x`.
    /// This bites the common case first: opening a form, clicking a prefilled field to correct one
    /// word, and finding every keystroke going to the end of the line.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_press_places_the_caret_where_the_pointer_landed() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("abcdefgh".to_string());
        assert_eq!(le.cursor_position(), 8, "`set_text` leaves the caret at the end");
        let field = le.field_rect();

        // One character cell past the value's origin is the boundary after character 1.
        let cell = cell_width();
        let origin = value_origin(&mut le);
        le.handle_event(&Event::mouse_press(
            origin + cell + cell / 2,
            field.y + field.height as i32 / 2,
            1,
        ));
        assert_eq!(le.cursor_position(), 1, "the caret lands on the character clicked");
        assert_eq!(le.selection_start(), Some(1), "and it is the anchor for a drag");
    }

    /// A drag extends that selection; a hover that was never pressed does not move the caret.
    ///
    /// Both halves matter. The first is the gesture: press on character 1, drag to character 4,
    /// release leaves `"bcd"` selected. The second is the guard — `MouseMove` is delivered on every
    /// pointer step over the control, so a field that acted on it unconditionally would have its
    /// caret follow the mouse whenever the pointer crossed it, with no button pressed at all.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_drag_extends_the_selection_but_a_bare_hover_does_not() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("abcdefgh".to_string());
        let field = le.field_rect();
        let mid = field.y + field.height as i32 / 2;
        let cell = cell_width();
        let origin = value_origin(&mut le);

        // A hover with no press ever delivered leaves the caret exactly where it was.
        le.handle_event(&Event::mouse_move(field.x + 4 * cell, mid));
        assert_eq!(le.cursor_position(), 8, "an unpressed hover must not move the caret");
        assert_eq!(le.selection_start(), None);

        // Press on character 1, drag to character 4: `"bcd"` is selected.
        le.handle_event(&Event::mouse_press(origin + cell + cell / 2, mid, 1));
        assert_eq!(le.selection_start(), Some(1), "the press anchors where it landed");
        le.handle_event(&Event::mouse_move(origin + 4 * cell + cell / 2, mid));
        assert_eq!(le.selected_text(), "bcd", "the drag selected the span between the two points");

        // The release ends the gesture, so a later hover is inert again.
        le.handle_event(&Event::mouse_release(origin + 4 * cell, mid, 1));
        le.handle_event(&Event::mouse_move(field.x + 7 * cell, mid));
        assert_eq!(le.cursor_position(), 4, "a hover after the release does not extend");
        assert_eq!(le.selected_text(), "bcd", "and the selection the user made is kept");
    }

    /// A press past the end of the value anchors at the end and readies a backward drag.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_press_beyond_the_value_anchors_at_its_end() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("ab".to_string());
        let field = le.field_rect();
        let mid = field.y + field.height as i32 / 2;

        // Well past the two characters that exist: the caret saturates at the end. The press must
        // land **inside the painted band** to be delivered, so it is taken from `field`, not from
        // the control's rectangle — on a tall cell the two are different boxes.
        le.handle_event(&Event::mouse_press(field.x + field.width as i32 - 2, mid, 1));
        assert_eq!(
            le.cursor_position(),
            1,
            "a press past the two-character value clamps to its own box, i.e. onto the last one"
        );
        assert_eq!(le.selection_start(), Some(1), "and that clamped character is the anchor");

        // Dragging back over the value selects it, which is the gesture this enables.
        let origin = value_origin(&mut le);
        le.handle_event(&Event::mouse_move(origin, mid));
        assert_eq!(le.cursor_position(), 0, "the drag reaches the value's leading edge");
        assert_eq!(le.selected_text(), "a", "so the range is anchor(1)-to-caret(0)");
    }

    /// The click-to-index map and the painted value agree about where character zero is.
    ///
    /// Read off the ink rather than off the same helper the hit test calls: a test that asked
    /// `decoration_layout` where the value starts would agree with a bug in it. The pen that draws
    /// the string is at `field.x + TEXT_FIELD_PADDING_H` (see `the_value_starts_at_the_fields_horizontal_padding`),
    /// so clicking that pixel must resolve to index 0.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_click_on_the_first_glyph_resolves_to_index_zero() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("Sample".to_string());
        let field = le.field_rect();
        let pen = field.x + dimensions::TEXT_FIELD_PADDING_H as i32;

        le.handle_event(&Event::mouse_press(pen, field.y + field.height as i32 / 2, 1));
        assert_eq!(le.cursor_position(), 0, "the first pixel of the value is index 0");
    }

    /// The pixel-to-index map is total on multi-byte values: no click can split a character.
    ///
    /// A byte-counting map lands inside `é` or a CJK glyph and the next `&text[..caret]` panics.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_click_never_lands_inside_a_multi_byte_character() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("héllo wörld 中文".to_string());
        let field = le.field_rect();
        let mid = field.y + field.height as i32 / 2;

        for dx in 0..field.width as i32 {
            le.handle_event(&Event::mouse_press(field.x + dx, mid, 1));
            let caret = le.cursor_position();
            assert!(caret <= le.text().len(), "the caret is inside the value at +{dx}");
            assert!(
                le.text().is_char_boundary(caret),
                "the caret at +{dx} split a character: {caret} in {:?}",
                le.text()
            );
        }
    }

    /// The width of one character cell in the field's shipped face, as the shaper computes it.
    ///
    /// Kept in the test module rather than as a private method on the control: it is the fixture's
    /// model of the metrics, and a control method for it would be dead code the gate would flag.
    ///
    /// Gated with its only consumers, which are all `#[cfg(not(alloc_frugal))]` because they drive
    /// the pointer path: an ungated helper became dead code under `mini` (which *is* `alloc_frugal`),
    /// where `clippy -D warnings` reported it while every other profile stayed green.
    #[cfg(not(alloc_frugal))]
    fn cell_width() -> i32 {
        (crate::core::Font::default().size() * 0.6).round().max(1.0) as i32
    }

    /// The x the value's glyphs are laid out from, read from the field's own layout.
    #[cfg(not(alloc_frugal))]
    fn value_origin(le: &mut LineEdit) -> i32 {
        let mut measurement =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
        let mut context = RenderContext::new(&mut measurement);
        le.decoration_layout(&mut context).value.x
    }

    #[test]
    fn lineedit_insert_text() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.insert_text("Hello");
        assert_eq!(le.text(), "Hello");
    }

    #[test]
    fn lineedit_backspace() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.backspace();
        assert_eq!(le.text(), "Hell");
    }

    #[test]
    fn lineedit_delete() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.set_cursor_position(0);
        le.delete();
        assert_eq!(le.text(), "ello");
    }

    #[test]
    fn lineedit_clear() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("Hello".to_string());
        le.clear();
        assert!(le.text().is_empty());
    }

    #[test]
    fn lineedit_geometry_delegation() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_geometry(Rect::new(10, 10, 150, 30));
        assert_eq!(le.geometry(), Rect::new(10, 10, 150, 30));
    }

    /// The field is a full-width band one text-field height tall, centred in the control.
    ///
    /// The control used to paint its whole rectangle, so a 240x120 census cell drew a
    /// 240x120 white slab — a panel, not a field — and the drawn height disagreed with
    /// `size_hint`'s 24 by a factor of five. This pins the shared `full_width_band`
    /// derivation in both directions: the height is the table's constant whatever the
    /// cell, and a control smaller than the field clamps rather than painting outside.
    #[test]
    fn the_field_is_a_text_field_height_in_any_rectangle() {
        for height in [48u32, 120, 300] {
            let le = LineEdit::new(Rect::new(0, 0, 240, height));
            let field = le.field_rect();
            assert_eq!(
                field.height,
                dimensions::TEXT_FIELD_MIN_HEIGHT,
                "at control height {height}"
            );
            assert_eq!(field.width, 240, "the field spans the control's width");
            assert_eq!(field.y, (height - field.height) as i32 / 2, "at control height {height}");
        }

        let short = LineEdit::new(Rect::new(0, 0, 240, 20));
        assert_eq!(short.field_rect().height, 20, "a short control clamps the field");
    }

    /// Hit-testing follows the ink: a press below the field does not focus it.
    ///
    /// With the field centred in a 120 px cell, a press on the control's rectangle but
    /// 60 px below the drawn band belongs to the window background. The control must not
    /// claim it — the clickable area has to be the one a user can see.
    #[test]
    fn a_press_outside_the_drawn_band_does_not_focus_the_field() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        let field = le.field_rect();

        // Inside the band focuses.
        le.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 / 2),
            button: 1,
            modifiers: 0,
        });
        assert!(le.is_focused(), "a press on the drawn field focuses it");

        // Below the band, inside the control's rectangle, does not.
        le.set_focused(false);
        le.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 + 40),
            button: 1,
            modifiers: 0,
        });
        assert!(!le.is_focused(), "a press below the drawn field must not focus it");
    }

    /// The value is drawn on the field's own middle line.
    ///
    /// The origin of a text run is its glyph box's top-left corner, so the old
    /// `rect.y + rect.height / 2` put that corner on the field's middle line and drew the
    /// value half a line low. Pinning the drawn line box — not the origin — to the centre is
    /// what keeps the two from agreeing by accident.
    ///
    /// # Why the check reads the ink
    ///
    /// The value is no longer a `<text>` element carrying a `y`: the backend emits the same
    /// `font8x8` rectangles the software rasteriser fills, as subpaths of one `<path>` (see
    /// `crate::widget::svg::text_ink_box`). The ink box is also the better witness, because it
    /// is where the glyphs actually landed rather than what an element claimed.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_value_sits_on_the_fields_middle_line() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("Sample");
        let svg = crate::widget::svg::render_to_svg(&mut le);
        let field = le.field_rect();

        let (_, top, _, bottom) = crate::widget::svg::text_ink_box(&svg)
            .expect("a field with a value draws it as glyph geometry");
        assert!(top >= field.y, "the value starts inside the field: y={top}");
        assert!(
            bottom <= field.y + field.height as i32,
            "and above its bottom edge: y={bottom}, field={field:?}"
        );
        // And it is *centred*, not merely contained: the field's own line box is the reference
        // the value was moved onto, so its ink shares that box's middle line. A value left on
        // the old `rect.y + rect.height / 2` anchor sits a whole half line below it.
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
        let context = RenderContext::new(&mut backend);
        let font = crate::core::Font::default();
        let line = context.text_line(field, &font);
        assert_eq!(
            top + bottom,
            line.y * 2 + line.height as i32,
            "the value hangs on the field's own line box middle line"
        );
    }

    /// The value's horizontal origin is the field's own padding.
    ///
    /// A field's text is inset from its edge by [`dimensions::TEXT_FIELD_PADDING_H`]; the
    /// origin was a local literal `4`, which is a different fact written in a second place.
    ///
    /// # Why the edge is read off the ink
    ///
    /// There is no `x` attribute to read any more: the backend emits the string as `font8x8`
    /// glyph rectangles inside one `<path>` (see `crate::widget::svg::text_ink_box`), so the
    /// run's left edge is where the pen actually put its first set bit. `context.text_line`
    /// centres the line box's *height*, not each cluster, so the pen itself is `field.x +
    /// TEXT_FIELD_PADDING_H` exactly; what is left over is the first glyph's own blank lead
    /// column, which the `font8x8` table gives as 0 for `S`. The assertion is still a real
    /// constraint on the padding: the pen is what the padding places, and the test would read
    /// `padding + 20` (or any other literal) instead of `padding` if the draw site stopped
    /// reading [`dimensions::TEXT_FIELD_PADDING_H`].
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_value_starts_at_the_fields_horizontal_padding() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("Sample");
        let svg = crate::widget::svg::render_to_svg(&mut le);
        // The fixture paints only the field's own chrome — a fill and a border — so what is
        // left is the value's ink, which no other string in this control can supply.
        assert_eq!(svg.matches("<rect").count(), 2, "only the field's fill and border are rects");
        let field = le.field_rect();

        let (left, _, _, _) = crate::widget::svg::text_ink_box(&svg)
            .expect("a field with a value draws it as glyph geometry");
        assert_eq!(
            left,
            field.x + dimensions::TEXT_FIELD_PADDING_H as i32,
            "the value starts at the field's own padding: field={field:?}"
        );
    }

    // ─── Decoration slots (F-12) ───

    /// A field that never sets a slot must paint exactly as it did, and reserve nothing extra.
    ///
    /// This is the non-change guarantee: the slots are additive, so every existing caller keeps its
    /// pixels and its height.
    #[test]
    fn a_field_with_no_slots_is_unchanged() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("Sample");
        assert!(le.decorations().is_empty());
        assert_eq!(le.get("prefix").unwrap().as_str(), Some(""));
        assert_eq!(le.get("counter").unwrap(), CapabilityValue::Null, "no limit means no counter");

        let svg = crate::widget::svg::render_to_svg(&mut le);
        // The value is still the only ink, and it still starts at the field's padding.
        let (left, _, _, _) = crate::widget::svg::text_ink_box(&svg).expect("the value is drawn");
        assert_eq!(left, le.field_rect().x + dimensions::TEXT_FIELD_PADDING_H as i32);
    }

    /// A prefix is drawn **before** the value, and the value moves right to make room.
    ///
    /// # The defect this pins
    ///
    /// The shortcut is to concatenate `prefix + value` into one string. That draws nearly the same
    /// glyphs in nearly the same place, so it looks right — while it also (a) makes `cursor_position ==
    /// 0` render after the `$`, and (b) makes a select-all copy the `$`. The distinguishing fact is that
    /// the two are **separate ink runs** with the prefix to the left of the value, so the test reads all
    /// runs and orders them rather than trusting which one was painted first.
    #[test]
    fn a_prefix_is_drawn_before_the_value_and_shifts_it() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut plain = LineEdit::new(Rect::new(0, 0, 240, 120));
        plain.set_text("12");
        let plain_svg = crate::widget::svg::render_to_svg(&mut plain);
        let plain_runs = crate::widget::svg::text_ink_boxes(&plain_svg);
        assert_eq!(plain_runs.len(), 1, "a plain field draws its value as one run");
        let (plain_left, _, plain_right, _) = plain_runs[0];

        let mut prefixed = LineEdit::new(Rect::new(0, 0, 240, 120));
        prefixed.set_text("12");
        prefixed.set_prefix("$");
        let prefixed_svg = crate::widget::svg::render_to_svg(&mut prefixed);
        let mut runs = crate::widget::svg::text_ink_boxes(&prefixed_svg);
        assert_eq!(
            runs.len(),
            2,
            "the prefix and the value are two runs, not one concatenated string"
        );
        runs.sort_by_key(|b| b.0);
        let (prefix_left, prefix_right) = (runs[0].0, runs[0].2);
        let (value_left, value_right) = (runs[1].0, runs[1].2);

        // The `$` takes the field's leading padding. The run's left edge is the *ink*, and neither
        // face's ink starts exactly at the origin — the `$` bitmap has no set bit in its leftmost
        // column, and an outline `$` is inset by its own shape — so the assertion allows the ink
        // inset and nothing more. It is still a comparison against where a *value with no prefix*
        // starts, so a prefix that ignored the padding and began on the border (or after the
        // value) is a whole glyph away and fails.
        assert!(
            (prefix_left - plain_left).abs() <= INK_INSET_TOLERANCE,
            "the prefix starts where a value with no prefix starts: it begins at {prefix_left}, \
             the field's leading padding puts a run at {plain_left}"
        );
        // …and the value is pushed clear of it rather than starting there too.
        assert!(
            value_left > prefix_right,
            "the value must begin past the prefix: prefix ends at {prefix_right}, value at {value_left}"
        );
        assert!(
            value_left > plain_left,
            "and further right than it sat without a prefix ({plain_left})"
        );
        // The value's own ink is the same width as before — only its origin moved, so the prefix did not
        // disturb the glyphs it marks.
        assert_eq!(
            value_right - value_left,
            plain_right - plain_left,
            "the prefix shifted the value without reshaping it"
        );

        assert_eq!(prefixed.get("prefix").unwrap().as_str(), Some("$"));
        assert!(
            plain_left >= plain.field_rect().x + dimensions::TEXT_FIELD_PADDING_H as i32,
            "the value is inset by the field's padding, not on the border"
        );
    }

    /// The caret is measured against the **value**, so at `cursor_position == 0` it sits at the value's
    /// origin — after a prefix, not after the `$`'s own box.
    ///
    /// This is the property that makes keeping the slots separate worth the trouble: with the prefix
    /// folded into the text, `cursor_position == 0` would place the caret to the right of the `$` and
    /// the start of the value would be unreachable.
    #[test]
    fn the_caret_at_position_zero_sits_at_the_values_origin_not_after_the_prefix() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_prefix("$");
        le.set_text("12");
        le.set_cursor_position(0);
        le.set_focused(true);

        // The layout is computed against the **real** SVG backend's metrics — the same backend the
        // painter uses — and then read for the two facts that matter: the prefix has a box, and the
        // value begins past it.
        let mut backend = crate::render::svg::SvgPaintBackend::new(le.geometry().size());
        let layout = {
            let mut context = crate::render::RenderContext::new(&mut backend);
            le.decoration_layout(&mut context)
        };
        let prefix = layout.prefix.expect("the prefix has a box of its own");
        assert!(
            layout.value.x >= prefix.right(),
            "the value must begin at or past the prefix's trailing edge: {layout:?}"
        );
        assert!(layout.value.width > 0, "and it must keep some room: {layout:?}");
    }

    /// An error displaces the helper and paints on its own row; the counter shares that row and is
    /// derived from the value and the limit.
    #[test]
    fn the_support_row_shows_the_error_and_a_derived_counter() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_max_length(Some(5));
        le.set_text("abc");
        assert_eq!(le.get("counter").unwrap().as_str(), Some("3/5"));
        assert_eq!(le.get("over_limit").unwrap().as_bool(), Some(false));

        // The helper shows while there is no error…
        le.set_helper_text("Up to five characters");
        assert_eq!(le.decorations().support_message(), "Up to five characters");
        assert!(le.decorations().needs_support_row());

        // …and the error displaces it rather than joining it.
        le.set_error_text("Too long");
        assert_eq!(le.decorations().support_message(), "Too long");
        assert!(le.decorations().has_error());
        assert_eq!(le.get("error").unwrap().as_str(), Some("Too long"));
        assert_eq!(
            le.get("helper").unwrap().as_str(),
            Some("Up to five characters"),
            "the helper is displaced, not deleted"
        );

        // Over the limit is reported, and the counter says how far. `set_text` enforces the limit too —
        // a limit the control honours only while typing is a hint, not a limit — so the value is clamped
        // and the pair stays consistent.
        le.set_text("abcdefgh");
        assert_eq!(le.text(), "abcde", "`set_text` must honour `max_length` like typing does");
        assert_eq!(le.get("counter").unwrap().as_str(), Some("5/5"));
        assert_eq!(
            le.get("over_limit").unwrap().as_bool(),
            Some(false),
            "the stored value cannot exceed a limit it was clamped to"
        );

        // Clearing the error restores the helper without the caller having to re-set it.
        le.set_error_text("");
        assert_eq!(le.decorations().support_message(), "Up to five characters");
    }

    /// The counter is read-only, and the two in-field slots round-trip through the contract.
    #[test]
    fn the_slots_round_trip_through_the_property_api() {
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set("prefix", CapabilityValue::String("$".to_string())).unwrap();
        le.set("suffix", CapabilityValue::String("%".to_string())).unwrap();
        le.set("helper", CapabilityValue::String("Hint".to_string())).unwrap();
        le.set("error", CapabilityValue::String("Bad".to_string())).unwrap();

        assert_eq!(le.get("prefix").unwrap().as_str(), Some("$"));
        assert_eq!(le.get("suffix").unwrap().as_str(), Some("%"));
        assert_eq!(le.get("helper").unwrap().as_str(), Some("Hint"));
        assert_eq!(le.get("error").unwrap().as_str(), Some("Bad"));

        // The two derived names are refused, so there is no second writer to disagree with.
        assert!(le.set("counter", CapabilityValue::String("1/1".to_string())).is_err());
        assert!(le.set("over_limit", CapabilityValue::Bool(true)).is_err());
    }

    /// A field too narrow for its slots still draws its field chrome and does not panic.
    #[test]
    fn a_field_squeezed_by_its_slots_still_paints() {
        let mut le = LineEdit::new(Rect::new(0, 0, 40, 120));
        le.set_prefix("verylongprefix");
        le.set_suffix("verylongsuffix");
        le.set_text("value");
        let svg = crate::widget::svg::render_to_svg(&mut le);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn lineedit_visibility() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.is_visible());
        le.hide();
        assert!(!le.is_visible());
        le.show();
        assert!(le.is_visible());
    }

    #[test]
    fn lineedit_enabled() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(le.is_enabled());
        le.set_enabled(false);
        assert!(!le.is_enabled());
        le.set_enabled(true);
        assert!(le.is_enabled());
    }

    #[test]
    fn lineedit_id_kind() {
        let le_a = LineEdit::new(Rect::new(0, 0, 100, 24));
        let le_b = LineEdit::new(Rect::new(0, 0, 100, 24));
        assert_ne!(le_a.id(), le_b.id());
        assert_eq!(le_a.kind(), WidgetKind::LineEdit);
        assert_eq!(le_b.kind(), WidgetKind::LineEdit);
    }

    #[test]
    fn lineedit_signal_accessors() {
        let le = LineEdit::new(Rect::new(0, 0, 100, 24));
        let _text_changed = &le.text_changed;
        let _editing_finished = &le.editing_finished;
        let _return_pressed = &le.return_pressed;
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_caret_is_drawn_at_cursor_position_not_at_the_end() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        // The caret used to be measured against the **whole** value, so it was always drawn
        // after the last glyph however the control was positioned: with `cursor_position = 0` it
        // still sat past the `e` of "Sample". The field answers `cursor_position`, clamps it on
        // every write, and acts on it in `backspace`/`delete`, so the marker was the one part of
        // the field ignoring the field's own cursor.
        //
        // The assertion reads the drawn geometry out of the SVG, because the defect is entirely
        // about where the ink lands — asserting the stored index would have passed before the fix
        // too, which is why it survived.
        fn caret_x(cursor_position: usize) -> i32 {
            let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
            field.set_text("Sample".to_string());
            // `focus()` places the caret at the end for the autofocus case, so the requested
            // position is applied *after* it — the same order a caller that positions a caret
            // uses.
            field.set_focused(true);
            field.set_cursor_position(cursor_position);
            let svg = crate::widget::svg::render_to_svg(&mut field);
            // The caret is the only vertical line the control draws, emitted as a `<line>` with
            // `x1 == x2`.
            svg.lines()
                .filter(|line| line.contains("<line"))
                .find_map(|line| {
                    let x1 = line.split("x1=\"").nth(1)?.split('"').next()?;
                    let x2 = line.split("x2=\"").nth(1)?.split('"').next()?;
                    if x1 != x2 {
                        return None;
                    }
                    x1.parse::<i32>().ok()
                })
                .expect("a focused, editable field must draw its caret as a vertical line")
        }

        let at_start = caret_x(0);
        let at_three = caret_x(3);
        let at_end = caret_x(6);
        // This assertion used to pin the literal `4`, which was a local padding constant
        // written at the draw site. The origin is now [`dimensions::TEXT_FIELD_PADDING_H`],
        // the same table value every other field insets its content by, so what this pins is
        // "the caret starts at the field's own horizontal padding" — the fact the old
        // literal was an unshared spelling of, not a different fact.
        assert_eq!(
            at_start,
            dimensions::TEXT_FIELD_PADDING_H as i32,
            "a caret at position 0 sits at the text origin (the field's padding inset)"
        );
        assert!(
            at_start < at_three && at_three < at_end,
            "the caret must advance with the cursor position, not jump to the end \
             ({at_start}, {at_three}, {at_end})"
        );
    }

    /// `max_length` is a **character** budget, so truncating at it must count characters.
    ///
    /// The limit is what the user sees and what `counter_text` reports, but an earlier revision
    /// truncated with `String::truncate(max)` — a **byte** index. On `"héllo"` (five characters,
    /// six bytes) a limit of three cut to `"hé"`, dropping a legal character; and a limit that
    /// landed inside a multi-byte character split it, which `String::truncate` panics on rather
    /// than permitting.
    #[test]
    fn max_length_truncates_whole_characters_not_bytes() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("héllo");
        assert_eq!(le.text().chars().count(), 5);
        le.set_max_length(Some(3));
        assert_eq!(le.text(), "hél", "three *characters* are kept, not three bytes");
        assert!(le.text().is_char_boundary(le.cursor_position()));

        // A limit that falls inside a character must not panic and must not split it.
        let mut wide = LineEdit::new(Rect::new(0, 0, 200, 24));
        wide.set_text("éééé");
        wide.set_max_length(Some(2));
        assert_eq!(wide.text(), "éé");
    }

    /// The limit is compared against the value in the *same* frame it is expressed in.
    ///
    /// `insert_text` computed its remaining room as `max - self.text.len()`, mixing a character
    /// budget with a byte length: on `"héllo"` a limit of eight left `8 - 6 == 2` characters of
    /// room instead of three, and a limit of six — which the counter shows as "5/6", i.e. one
    /// character free — left **zero**, so the field refused every further keystroke.
    #[test]
    fn insert_text_measures_the_remaining_room_in_characters() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("héllo");
        le.set_max_length(Some(6));
        le.insert_text("z");
        assert_eq!(le.text(), "hélloz", "the counter said 5/6, so one more character fits");
        assert_eq!(le.text().chars().count(), 6);

        // A multi-character insert is clipped to the room that remains, by character.
        let mut clipped = LineEdit::new(Rect::new(0, 0, 200, 24));
        clipped.set_text("éé");
        clipped.set_max_length(Some(4));
        clipped.insert_text("wxyz");
        assert_eq!(clipped.text(), "ééwx");
    }

    /// The caret and the edit keys step by **character**, not by byte.
    ///
    /// A bare `cursor_position += 1` landed inside a multi-byte character, so `insert_text`'s
    /// `floor_char_boundary` then rounded forward to the end of the value: on `"éé"` a single
    /// Right followed by a keystroke appended instead of inserting one character in. The same
    /// arithmetic made `backspace`/`delete` slice mid-character.
    #[test]
    fn caret_and_edit_keys_step_by_character() {
        let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
        le.set_text("éé");
        le.set_cursor_position(0);
        le.handle_event(&Event::key_press(39, 0)); // Right
        assert_eq!(le.cursor_position(), 2, "one character forward is two bytes for 'é'");
        le.insert_text("x");
        assert_eq!(le.text(), "éxé", "the insert lands at the caret, not at the end");

        // Backspace removes the whole character before the caret.
        let mut back = LineEdit::new(Rect::new(0, 0, 200, 24));
        back.set_text("aé");
        back.set_cursor_position(back.text().len());
        back.backspace();
        assert_eq!(back.text(), "a");

        // Delete removes the whole character after the caret.
        let mut fwd = LineEdit::new(Rect::new(0, 0, 200, 24));
        fwd.set_text("éa");
        fwd.set_cursor_position(0);
        fwd.delete();
        assert_eq!(fwd.text(), "a");
    }

    /// The caret rides with the glyphs of a centred or right-aligned value.
    ///
    /// The value is laid out from `fitted_origin(value_box, advance, alignment)`; the caret was
    /// measured from the field's fixed leading inset instead, so on a right-aligned field the
    /// marker sat at the left edge while its own text sat at the right edge. The two now read the
    /// same origin function, which is what keeps them together.
    #[test]
    fn the_caret_follows_a_non_left_aligned_value() {
        let _theme_guard = crate::style::theme_test_guard();
        fn caret_x(alignment: crate::core::Alignment) -> i32 {
            let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
            field.set_text("Sample".to_string());
            field.set_alignment(alignment);
            field.set_focused(true);
            field.set_cursor_position(0);
            let svg = crate::widget::svg::render_to_svg(&mut field);
            svg.lines()
                .filter(|line| line.contains("<line"))
                .find_map(|line| {
                    let x1 = line.split("x1=\"").nth(1)?.split('"').next()?;
                    let x2 = line.split("x2=\"").nth(1)?.split('"').next()?;
                    (x1 == x2).then(|| x1.parse::<i32>().ok())?
                })
                .expect("a focused, editable field must draw its caret as a vertical line")
        }

        let left = caret_x(crate::core::Alignment::Left);
        let centre = caret_x(crate::core::Alignment::Center);
        let right = caret_x(crate::core::Alignment::Right);
        assert!(
            left < centre && centre < right,
            "a caret at position 0 must follow the value's own alignment \
             (left={left}, centre={centre}, right={right})"
        );
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn lineedit_clipboard_copy_paste_cut() {
        use crate::event::Event::KeyPress;
        // The clipboard is process-wide, so this test must not run concurrently with any
        // other test that copies or pastes. Without the guard it failed intermittently
        // under the parallel harness while passing on its own.
        let _clipboard = crate::clipboard::clipboard_test_guard();

        let mut source = LineEdit::new(Rect::new(0, 0, 200, 24));
        source.set_text("hello world");
        source.select_all();
        // Ctrl+C copies the selection to the platform clipboard.
        source.handle_event(&KeyPress { key: 67, modifiers: 0b1000 });
        assert_eq!(crate::get_clipboard_text(), "hello world");

        // Ctrl+V pastes into another field.
        let mut target = LineEdit::new(Rect::new(0, 0, 200, 24));
        target.handle_event(&KeyPress { key: 86, modifiers: 0b1000 });
        assert_eq!(target.text(), "hello world");

        // Ctrl+X on a selected field copies then removes the selection.
        source.select_all();
        source.handle_event(&KeyPress { key: 88, modifiers: 0b1000 });
        assert_eq!(source.text(), "");
        assert_eq!(crate::get_clipboard_text(), "hello world");

        // Read-only fields still copy via Ctrl+C.
        let mut read_only = LineEdit::new(Rect::new(0, 0, 200, 24));
        read_only.set_text("secret");
        read_only.set_read_only(true);
        read_only.select_all();
        read_only.handle_event(&KeyPress { key: 67, modifiers: 2 });
        assert_eq!(crate::get_clipboard_text(), "secret");
        // …but editing stays blocked.
        assert_eq!(read_only.text(), "secret");
    }

    /// A refusal travels the **meaning** channel, so it never competes with the interaction one.
    ///
    /// # The defect this pins
    ///
    /// The preset themes declare ten `"<kind>:error"` overrides, and the meaning used to be
    /// reported from `widget_state()` — a single-valued chain the interaction also wanted. The
    /// interaction won whenever the pointer was over the field, so `"line_edit:error"` stopped
    /// applying exactly when the user moved onto the field to read the refusal. The meaning is now
    /// orthogonal (`semantic_state`), so the first half of this test proves the control still asks
    /// the question, and the second proves the theme's answer arrives on a border.
    ///
    /// # Why this is gated on `full_widgets`
    ///
    /// It installs the preset appearances through `widget::census`, which is the module that owns
    /// them and exists only where the widget set is unstripped. On a stripped profile the theme is
    /// absent entirely, so there is no preset to install and no `:error` override to read — the
    /// subject of the test does not exist there, which is what a profile gate has to say.
    #[cfg(full_widgets)]
    #[test]
    fn a_refused_field_reports_the_error_meaning_and_takes_the_override() {
        use crate::style::{SemanticState, WidgetState};

        let _theme_guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));

        assert_eq!(field.widget_state(), WidgetState::Normal, "a fresh field is at rest");
        assert_eq!(field.semantic_state(), SemanticState::None, "and carries no meaning");

        field.set_error_text("Too long");
        assert_eq!(
            field.semantic_state(),
            SemanticState::Error,
            "a field with a refusal message must say so, or the `:error` override is unreachable"
        );
        assert_eq!(
            field.widget_state(),
            WidgetState::Normal,
            "but the interaction channel stays at rest: the refusal is not an interaction"
        );

        // The theme resolves a border for that meaning — the whole point of reporting it.
        let border = crate::style::resolved_semantic_border("line_edit", field.semantic_state());
        assert!(border.is_some(), "the preset must define a border on `line_edit:error`");

        // Clearing the message returns the field to no meaning, so it is not latched.
        field.set_error_text("");
        assert_eq!(
            field.semantic_state(),
            SemanticState::None,
            "clearing the error restores no meaning"
        );
    }

    /// BLUE24 §3.3 criterion 1: a hovered refusal keeps **both** channels at once.
    ///
    /// This is the assertion the single-valued chain could not satisfy: `widget_state()` was the
    /// only way to say "error", so a hovered field could report either the hover or the error but
    /// never both. With the channels split, the fill follows the hover and the border follows the
    /// meaning in the same frame.
    #[cfg(full_widgets)]
    #[test]
    fn a_hovered_refusal_keeps_hover_fill_and_error_border() {
        use crate::event::Event;
        use crate::style::{SemanticState, WidgetState};

        let _theme_guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_error_text("Too long");

        field.handle_event(&Event::MouseEnter { pos: crate::core::Point::new(10, 10) });

        assert_eq!(
            field.widget_state(),
            WidgetState::Hover,
            "the fill answers to the interaction, so a hovered field reports hover"
        );
        assert_eq!(
            field.semantic_state(),
            SemanticState::Error,
            "and the meaning survives the hover, which is the defect being pinned"
        );

        // Both channels reach the theme in the same frame: the hover fill and the error border.
        let hover_style =
            crate::theme::resolved_theme_style_for_state("line_edit", WidgetState::Hover);
        assert!(hover_style.is_some(), "the preset defines `line_edit:hover`");
        assert!(
            crate::style::resolved_semantic_border("line_edit", field.semantic_state()).is_some(),
            "and a border for `line_edit:error` at the same time"
        );
    }

    /// The interaction channel still reports `Disabled` for an inert field, refusal or not.
    #[test]
    fn a_disabled_field_reports_disabled_even_with_a_refusal_message() {
        use crate::style::{SemanticState, WidgetState};

        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_error_text("Too long");
        field.set_enabled(false);
        assert_eq!(
            field.widget_state(),
            WidgetState::Disabled,
            "a disabled field cannot be acted on, so its state must say disabled"
        );
        assert_eq!(
            field.semantic_state(),
            SemanticState::Error,
            "while the meaning is still reported on its own channel"
        );
    }
}

#[cfg(test)]
mod composition_tests {
    use super::*;

    fn field() -> LineEdit {
        LineEdit::new(Rect::new(0, 0, 200, 24))
    }

    /// A composition does **not** change the field's value.
    ///
    /// # The defect this pins (BLUE24 §12 U-5)
    ///
    /// A platform input method reports the preedit one keystroke at a time. If that went through
    /// `insert_text`, the field's value would change on every partial character and `text_changed`
    /// would fire for a word the user has not finished — so a form's validation would run against
    /// text nobody committed. Holding the composition apart is what makes the value stable until the
    /// commit.
    #[test]
    fn a_composition_does_not_change_the_value() {
        let mut field = field();
        field.set_text("ab");
        field.set_composition("し");
        assert_eq!(field.text(), "ab", "the value is untouched while composing");
        assert_eq!(field.composition(), Some("し"));
        assert!(field.has_composition());
    }

    /// Committing puts the preedit into the model, at the caret, through the ordinary insert path.
    #[test]
    fn committing_inserts_the_preedit_as_text() {
        let mut field = field();
        field.set_text("ab");
        field.set_cursor_position(1);
        field.set_composition("X");
        assert!(field.commit_composition());
        assert_eq!(field.text(), "aXb", "a commit is an insert, not a replacement");
        assert!(!field.has_composition(), "and the session is over");
        assert!(!field.commit_composition(), "a second commit has nothing to accept");
    }

    /// Cancelling restores nothing, because nothing was changed.
    #[test]
    fn cancelling_leaves_the_value_exactly_as_it_was() {
        let mut field = field();
        field.set_text("keep");
        field.set_composition("discard me");
        assert!(field.cancel_composition());
        assert_eq!(field.text(), "keep");
        assert!(!field.has_composition());
        assert!(!field.cancel_composition(), "there was nothing left to cancel");
    }

    /// Committing a composition respects `max_length`, exactly as typing does.
    ///
    /// This is the reason the commit goes through `insert_text` rather than writing `text` directly:
    /// a shortcut would have been the one edit path in the control that ignores the limit.
    #[test]
    fn committing_respects_the_max_length() {
        let mut field = field();
        field.set_max_length(Some(4));
        field.set_text("ab");
        field.set_composition("cdef");
        field.commit_composition();
        assert!(field.text().len() <= 4, "the limit applies: got {:?}", field.text());
    }

    /// A composition can replace a selection, like any other insertion.
    #[test]
    fn committing_replaces_a_selection() {
        let mut field = field();
        field.set_text("abc");
        field.select_all();
        field.set_composition("Z");
        field.commit_composition();
        assert_eq!(field.text(), "Z", "the commit replaced the selected range");
    }

    /// An empty preedit withdraws the session without committing anything.
    #[test]
    fn an_empty_preedit_withdraws_the_session() {
        let mut field = field();
        field.set_text("ab");
        field.set_composition("x");
        field.set_composition("");
        assert!(!field.has_composition());
        assert_eq!(field.text(), "ab", "withdrawing is not committing");
    }

    /// The preedit is drawn **inline at the caret**, with an underline.
    ///
    /// # The gap this pins (BLUE24 §12 U-5)
    ///
    /// `ImePreedit` is an overlay in a box of its own; it cannot place a composition at a field's
    /// caret, because it does not know where that is. The field can, and this asserts it does: the
    /// preedit appears after the value's own ink and a horizontal rule is drawn beneath it.
    #[test]
    fn the_preedit_is_painted_at_the_caret_with_an_underline() {
        let bounds = Rect::new(0, 0, 200, 24);
        let mut field = LineEdit::new(bounds);
        field.set_text("ab");
        field.set_cursor_position(2);
        field.set_focused(true);

        let without = crate::widget::svg::render_widget_to_svg(&mut field, bounds);
        field.set_composition("し");
        let with = crate::widget::svg::render_widget_to_svg(&mut field, bounds);

        assert_ne!(with, without, "an active composition must change what is painted");
        // A horizontal rule is a `<line>` with `y1 == y2`; the caret is a vertical one. The underline
        // is the only horizontal line this change adds.
        let horizontal_lines = |svg: &str| {
            svg.lines()
                .filter(|line| line.contains("<line "))
                .filter(|line| {
                    let get = |name: &str| -> Option<i32> {
                        line.split(&format!("{name}=\"")).nth(1)?.split('"').next()?.parse().ok()
                    };
                    get("y1").is_some() && get("y1") == get("y2")
                })
                .count()
        };
        assert!(
            horizontal_lines(&with) > horizontal_lines(&without),
            "the preedit must be underlined, or a user cannot see it is uncommitted"
        );

        // And cancelling removes it again, so the picture is tied to the session.
        field.cancel_composition();
        let after = crate::widget::svg::render_widget_to_svg(&mut field, bounds);
        assert_eq!(horizontal_lines(&after), horizontal_lines(&without));
    }
}
