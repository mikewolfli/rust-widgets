// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! InplaceEditor widget — an in-place text editing control for table/cell editing.
//!
//! Displays text normally, and when activated (double-click), switches to an
//! edit mode with a blinking cursor. Enter/Tab accepts changes, Escape cancels.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::text_utils::byte_index_of_char;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// A bit mask of the gestures that enter edit mode.
///
/// The default is [`Self::DOUBLE_CLICK`], which is the gesture this control shipped with; the
/// other flags exist so a grid cell can opt into single-click or F2 editing without a new widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditTriggers(u8);

impl EditTriggers {
    /// No gesture starts an edit; only [`InplaceEditor::start_edit`] does.
    pub const NONE: Self = Self(0);
    /// A double click on the field starts an edit.
    pub const DOUBLE_CLICK: Self = Self(1);
    /// A single click on the field starts an edit.
    pub const SINGLE_CLICK: Self = Self(2);
    /// The F2 key starts an edit while the control is focused.
    pub const F2: Self = Self(4);
    /// The default set: a double click.
    pub const DEFAULT: Self = Self::DOUBLE_CLICK;

    /// Whether every flag in `other` is set here.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two trigger sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl Default for EditTriggers {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A text editor that switches between display mode and edit mode in-place.
///
/// In display mode, the text is drawn as plain text with a background.
/// In edit mode, it draws a text input area with a blinking cursor.
pub struct InplaceEditor {
    base: BaseWidget,
    text: String,
    is_editing: bool,
    original_text: String,
    font_size: f32,
    padding: i32,
    cursor_position: usize,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// The edit caret's blink state, advanced by [`InplaceEditor::tick`].
    ///
    /// It lives on the control because it is state that must survive between frames; the tempo and
    /// the phase logic come from [`crate::style::CursorBlink`] so this control cannot drift from
    /// every other caret in the crate.
    cursor_blink: crate::style::CursorBlink,
    /// The gestures that enter edit mode.
    edit_triggers: EditTriggers,
    /// The other end of the edit selection, as a character index, or `None`.
    selection_anchor: Option<usize>,
    /// Whether a pointer drag is extending the selection.
    dragging_selection: bool,
    /// Emitted when the edit is accepted (Enter/Tab). Carries the final text.
    pub edit_accepted: Signal1<String>,
    /// Emitted when the edit is cancelled (Escape).
    pub edit_canceled: GenericSignal,
}

impl InplaceEditor {
    /// Creates a new InplaceEditor widget with the given text and geometry.
    pub fn new(text: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::InplaceEditor, geometry, "InplaceEditor"),
            text: text.to_string(),
            is_editing: false,
            original_text: text.to_string(),
            font_size: 14.0,
            // Seeded from the shared text-field inset rather than a second literal: the field *is*
            // that inset, so a default-built editor renders exactly as it did when `draw` read the
            // constant directly, and the value the getter reports is the inset in force.
            padding: dimensions::TEXT_FIELD_PADDING_H as i32,
            cursor_position: text.len(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(text.to_string())),
            restoring_history: false,
            cursor_blink: crate::style::CursorBlink::new(),
            edit_triggers: EditTriggers::default(),
            selection_anchor: None,
            dragging_selection: false,
            edit_accepted: Signal1::new(),
            edit_canceled: GenericSignal::new(),
        }
    }

    /// Compatibility spelling of [`Self::edit_canceled`] (W-19): the same emitted signal.
    pub fn edit_cancelled(&self) -> &GenericSignal {
        &self.edit_canceled
    }

    /// Starts editing mode.
    pub fn start_edit(&mut self) {
        if !self.is_editing {
            self.is_editing = true;
            self.original_text = self.text.clone();
            self.cursor_position = self.text.chars().count();
            self.selection_anchor = None;
            // Entering edit mode is entering the state the caret blinks in, so the blink starts
            // here rather than waiting for the host to notice — a caret that only began blinking on
            // some later frame would sit frozen for however long that took.
            self.cursor_blink.start();
            self.base.request_redraw();
        }
    }

    /// Finishes editing mode. If `accept` is true, the current text is kept;
    /// otherwise it reverts to the original text.
    pub fn finish_edit(&mut self, accept: bool) {
        if !self.is_editing {
            return;
        }
        self.is_editing = false;
        // Leaving edit mode stops the caret's animation, so a host driving frames from the control
        // stops scheduling them. The caret is left visible, which is what a blurred field shows.
        self.cursor_blink.stop();
        if accept {
            self.edit_accepted.emit(self.text.clone());
        } else {
            self.text = self.original_text.clone();
            self.edit_canceled.emit();
        }
        self.base.request_redraw();
    }

    /// Advances the edit caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention, and the reason the module's docs can claim
    /// a blinking cursor: before this existed the caret was a solid line and nothing ever told the
    /// host to draw another frame. A host calls this once per frame while it reports `true`.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.is_editing {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }

    /// Returns whether the editor is in edit mode.
    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        // The old value moves out rather than being cloned for the undo record (I-P02).
        let before = core::mem::replace(&mut self.text, text);
        self.cursor_position = self.text.chars().count();
        self.publish_text_change(before);
    }

    /// Publishes a value already written to `self.text`, recording `before` as one undo step.
    ///
    /// Every edit path writes the value **in place** and hands the previous value here, instead
    /// of building a fresh `String` from a `Vec<char>` for each keystroke (I-P02).
    fn publish_text_change(&mut self, before: String) {
        if !self.restoring_history && before != self.text {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(ValueCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "Edit text",
                "inplace_editor_text",
            )));
        }
        self.base.request_redraw();
    }

    /// Steps back one committed edit and returns `true`, or `false` when there is
    /// nothing to undo.
    ///
    /// Only committed values are undoable: text edited in the box but not yet
    /// accepted is not on the stack. The cursor is moved to the end of the
    /// restored text.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Steps forward one undone edit and returns `true`, or `false` when there is
    /// nothing to redo. Behaviour matches [`InplaceEditor::undo`].
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns `true` if [`InplaceEditor::undo`] would change the value.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`InplaceEditor::redo`] would change the value.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.cursor_position = self.text.chars().count();
        self.restoring_history = false;
        self.base.request_redraw();
    }

    /// Sets the font size.
    pub fn set_font_size(&mut self, size: f32) {
        let size = size.max(4.0);
        if self.font_size == size {
            return;
        }
        self.font_size = size;
        self.base.request_redraw();
    }

    /// Returns the font size.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Sets the padding around the text.
    pub fn set_padding(&mut self, padding: i32) {
        let padding = padding.max(0);
        if self.padding == padding {
            return;
        }
        self.padding = padding;
        self.base.request_redraw();
    }

    /// Returns the padding.
    pub fn padding(&self) -> i32 {
        self.padding
    }

    /// The field the control actually paints.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// An in-place editor is a text field shown only while a cell is being edited, so it is
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] tall, full width, centred — the same shape the
    /// rest of the crate's input controls draw. Painting `geometry()` made a 240x120 census
    /// cell a 240x120 box, and the value inherited the oversized box through the
    /// `padding + font_size` anchor. Everything the control paints **and the double-click it
    /// answers** is placed from this one box.
    fn field_rect(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The box the value is drawn in: the field, inset by [`Self::padding`] on every side.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `padding` was stored, published (`set_padding` clamps it to `>= 0` and requests a redraw)
    /// and read by nothing: `draw` used the fixed `dimensions::TEXT_FIELD_PADDING_H` as its inset
    /// in both modes, and the double-click hit test used [`Self::field_rect`]. So the control's one
    /// layout knob moved no pixel and clamped an input nothing consumed.
    ///
    /// # Why the *field's* inset rather than a separate constant
    ///
    /// The default is 4, which is `TEXT_FIELD_PADDING_H` — so a default-built editor is unchanged
    /// to the pixel, and the constant stays the named default rather than a second source of truth.
    /// A caller who wants a roomier editor gets one on both axes, which is what "padding around
    /// the text" says; an inset on the horizontal axis alone would be a different field.
    fn text_rect(&self) -> Rect {
        let field = self.field_rect();
        let inset = self.padding.min(field.width as i32 / 2).min(field.height as i32 / 2).max(0);
        Rect::new(
            field.x + inset,
            field.y,
            field.width.saturating_sub(inset as u32 * 2),
            field.height,
        )
    }

    /// Inserts a character at the cursor position.
    fn insert_char(&mut self, c: char) {
        if c == '\u{7f}' {
            // Backspace, kept as a sentinel for the existing keyboard path.
            if self.delete_selection() {
                return;
            }
            if self.cursor_position > 0 {
                let caret = byte_index_of_char(&self.text, self.cursor_position);
                let previous = crate::widget::text_utils::previous_char_boundary(&self.text, caret);
                let before = self.text.clone();
                self.text.replace_range(previous..caret, "");
                self.cursor_position = self.cursor_position.saturating_sub(1);
                self.publish_text_change(before);
            }
        } else if c == '\u{ffff}' {
            // Forward delete, routed through the dedicated method so the sentinel remains only a
            // compatibility spelling for callers that still pass it.
            self.delete_forward();
        } else {
            // Typing over a selection replaces it, which is the one edit every text field owes
            // the user after a drag.
            self.delete_selection();
            let mut buffer = [0u8; 4];
            let encoded = c.encode_utf8(&mut buffer);
            let at = byte_index_of_char(&self.text, self.cursor_position);
            let before = self.text.clone();
            self.text.replace_range(at..at, encoded);
            self.cursor_position = (self.cursor_position + 1).min(self.text.chars().count());
            self.publish_text_change(before);
        }
    }

    /// Inserts committed text as **one** edit (D09-INPUT-01, I-P14).
    ///
    /// The desktop backends deliver a printable character or an IME commit as
    /// `Event::TextInput`/`ImeCommit`. Routing a phrase through the per-character path made one
    /// committed word N undo steps and N signals with a caret that walked character by
    /// character; this writes the whole string with one `replace_range`, one history entry and
    /// one caret advance. Control characters are filtered so a stray one does not enter the value.
    fn insert_committed_text(&mut self, text: &str) {
        let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
        if filtered.is_empty() {
            return;
        }
        let (start_char, end_char) =
            self.selection_range().unwrap_or((self.cursor_position, self.cursor_position));
        self.selection_anchor = None;
        let start = byte_index_of_char(&self.text, start_char);
        let end = byte_index_of_char(&self.text, end_char);
        let before = self.text.clone();
        self.text.replace_range(start..end, &filtered);
        self.cursor_position =
            (start_char + filtered.chars().count()).min(self.text.chars().count());
        self.publish_text_change(before);
    }

    /// Moves the cursor left by one character.
    /// The gestures that enter edit mode.
    pub fn edit_triggers(&self) -> EditTriggers {
        self.edit_triggers
    }

    /// Replaces the gestures that enter edit mode.
    pub fn set_edit_triggers(&mut self, triggers: EditTriggers) {
        if self.edit_triggers == triggers {
            return;
        }
        self.edit_triggers = triggers;
    }

    /// The edit selection as ordered character indices, or `None` when nothing is selected.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let char_count = self.text.chars().count();
        let caret = self.cursor_position.min(char_count);
        let anchor = anchor.min(char_count);
        if anchor == caret {
            return None;
        }
        Some(if anchor < caret { (anchor, caret) } else { (caret, anchor) })
    }

    /// The selected text, or an empty string when nothing is selected.
    pub fn selected_text(&self) -> String {
        match self.selection_range() {
            Some((start, end)) => self.text.chars().skip(start).take(end - start).collect(),
            None => String::new(),
        }
    }

    /// Selects the whole value and puts the caret at its end.
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor_position = self.text.chars().count();
        self.base.request_redraw();
    }

    /// Removes the selection, if any, returning whether anything was removed.
    fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };
        let start_byte = byte_index_of_char(&self.text, start);
        let end_byte = byte_index_of_char(&self.text, end);
        let before = self.text.clone();
        self.text.replace_range(start_byte..end_byte, "");
        self.cursor_position = start.min(self.text.chars().count());
        self.selection_anchor = None;
        self.publish_text_change(before);
        true
    }

    /// Deletes the character after the caret, or the selection when one is active.
    ///
    /// This is the forward-delete the editor previously expressed by passing a sentinel
    /// replacement character through `Self::insert_char`, which made a deletion indistinguishable
    /// from typed text in every log and test.
    pub fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let char_count = self.text.chars().count();
        if self.cursor_position >= char_count {
            return false;
        }
        let at = byte_index_of_char(&self.text, self.cursor_position);
        let next = crate::widget::text_utils::next_char_boundary(&self.text, at);
        let before = self.text.clone();
        self.text.replace_range(at..next, "");
        self.cursor_position = self.cursor_position.min(self.text.chars().count());
        self.publish_text_change(before);
        true
    }

    /// Moves the caret to `target` (a character index), extending the selection when `extend` is set.
    fn move_caret(&mut self, target: usize, extend: bool) {
        let char_count = self.text.chars().count();
        let target = target.min(char_count);
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor_position.min(char_count));
            }
        } else {
            self.selection_anchor = None;
        }
        self.cursor_position = target;
        self.base.request_redraw();
    }

    /// The character index a horizontal position points at.
    ///
    /// One pass over the value accumulating each character's advance, stopping at the character
    /// whose midpoint the pointer crossed (I-P04). It used to rebuild a prefix string for every
    /// character — n prefix allocations plus n whole-string measurements per hit test, and a
    /// drag re-ran it on every pointer step.
    fn char_index_at_x(&self, x: i32) -> usize {
        let measured_font;
        let font = match self.base.style().font.as_ref() {
            Some(font) => font,
            None => {
                measured_font = crate::core::Font::simple("sans-serif", self.font_size);
                &measured_font
            }
        };
        let target = (x - self.text_rect().x).max(0) as f32;
        let mut advance = 0f32;
        let mut ordinal = 0usize;
        let mut buffer = [0u8; 4];
        for ch in self.text.chars() {
            let width =
                crate::widget::metrics::estimate_text_width(ch.encode_utf8(&mut buffer), font, 1.0)
                    as f32;
            if target < advance + width / 2.0 {
                break;
            }
            advance += width;
            ordinal += 1;
        }
        ordinal
    }

    /// The advance, in pixels, of the first `count` characters of the value.
    ///
    /// Accumulated with the **same per-character ruler** [`Self::char_index_at_x`] uses, so the
    /// paint's caret and the pointer's caret land on the same boundary (I-P04).
    fn advance_of_prefix(&self, count: usize) -> i32 {
        let measured_font;
        let font = match self.base.style().font.as_ref() {
            Some(font) => font,
            None => {
                measured_font = crate::core::Font::simple("sans-serif", self.font_size);
                &measured_font
            }
        };
        let mut advance = 0i32;
        let mut buffer = [0u8; 4];
        for ch in self.text.chars().take(count) {
            advance = advance.saturating_add(crate::widget::metrics::estimate_text_width(
                ch.encode_utf8(&mut buffer),
                font,
                1.0,
            ) as i32);
        }
        advance
    }
}

impl Widget for InplaceEditor {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::INPLACE_EDITOR_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::INPLACE_EDITOR_DEFAULT_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    // The caret blink is driven through the trait so the animation bus reaches it.
    fn tick(&mut self, delta_ms: u32) -> bool {
        InplaceEditor::tick(self, delta_ms)
    }

    /// Whether the caret is cycling, i.e. whether a frame is owed.
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host consults it to
    /// decide whether a frame is needed **before** paying for a sweep. An editing field whose caret is
    /// mid-blink answered `false`, so such a host would stop scheduling frames between two blinks and
    /// the caret would stop cycling — the same unreachable animation the trait `tick` above exists to
    /// end, one layer up. The field's own state answers it: the caret animates exactly while the field
    /// is collecting keys.
    fn is_animating(&self) -> bool {
        self.is_editing
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
            "edit_accepted" => {
                Some(EventSignalRef::mapped("edit_accepted", &self.edit_accepted, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "edit_canceled" => Some(EventSignalRef::unit("edit_canceled", &self.edit_canceled)),
            _ => None,
        }
    }
}

/// `InplaceEditor`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `editing` maps onto the
/// widget's edit mode: writing `true` starts an edit, writing `false` cancels it,
/// matching the legacy arm.
impl WidgetProperties for InplaceEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "editing" => Ok(CapabilityValue::Bool(self.is_editing())),
            "edit_triggers" => Ok(CapabilityValue::String(triggers_token(self.edit_triggers))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "editing" => {
                if expect_bool(value)? {
                    self.start_edit();
                } else {
                    self.finish_edit(false);
                }
                Ok(())
            }
            "edit_triggers" => {
                let triggers = triggers_from_token(&expect_string(value)?)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_edit_triggers(triggers);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "editing", "edit_triggers", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `inplace_editor` publishes.
    ///
    /// The two actions enter and leave edit mode through the control's own methods, so
    /// the guard against a redundant start, the revert-on-cancel and the
    /// `edit_accepted` / `edit_canceled` signals all stay in one place. `finish_editing`
    /// accepts, which is the meaning of the name: a caller that wants the revert spells
    /// it as `set("editing", false)`, where the payload says so. `set_text` assigns the
    /// text and is answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "start_editing" => {
                self.start_edit();
                Ok(())
            }
            "finish_editing" => {
                self.finish_edit(true);
                Ok(())
            }
            "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

/// The published token for an [`EditTriggers`] set.
fn triggers_token(triggers: EditTriggers) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if triggers.contains(EditTriggers::DOUBLE_CLICK) {
        parts.push("double_click");
    }
    if triggers.contains(EditTriggers::SINGLE_CLICK) {
        parts.push("single_click");
    }
    if triggers.contains(EditTriggers::F2) {
        parts.push("f2");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(",")
    }
}

/// The [`EditTriggers`] a published token names, or `None` for an unknown token.
fn triggers_from_token(token: &str) -> Option<EditTriggers> {
    let mut triggers = EditTriggers::NONE;
    let mut saw_any = false;
    for part in token.split(',') {
        match part.trim() {
            "" | "none" => continue,
            "double_click" => {
                triggers = triggers.union(EditTriggers::DOUBLE_CLICK);
                saw_any = true;
            }
            "single_click" => {
                triggers = triggers.union(EditTriggers::SINGLE_CLICK);
                saw_any = true;
            }
            "f2" => {
                triggers = triggers.union(EditTriggers::F2);
                saw_any = true;
            }
            _ => return None,
        }
    }
    if saw_any || token.trim() == "none" || token.trim().is_empty() {
        Some(triggers)
    } else {
        None
    }
}

impl Draw for InplaceEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **field**, not the control's rectangle.
        //
        // An in-place editor is a text field: it is `TEXT_FIELD_MIN_HEIGHT` tall, full
        // width, centred in the area it is given, exactly as every other field in this crate
        // is. Painting `geometry()` made a 240x120 census cell a 240x120 box, and the text
        // and caret below inherited that — the value sat `padding + font_size` down from an
        // edge that was itself not the field's. The hit test reads the same box, so the
        // double-click target is the visible field.
        let rect = self.field_rect();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; both modes' fill, border, cursor and text used to be hardcoded
        // literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        // The field's font honours a caller/theme font when one is resolved, and otherwise uses
        // this editor's own configured size. It used to hardcode `sans-serif`, so `style.font`
        // never reached the text it renders.
        let font = style
            .font
            .clone()
            .unwrap_or_else(|| crate::core::Font::simple("sans-serif", self.font_size));
        let theme = crate::style::resolved_theme_style_shared("inplace_editor");
        // One palette read for the whole frame (I-P07): the window fill and the accent used to
        // take the theme manager's lock in two separate calls after the resolved style. The
        // palette is copied out as values so the guard is released before anything is painted.
        let (window_fill, accent) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (active.colors.background, active.colors.primary),
                None => (Color::WHITE, Color::rgb(0, 120, 255)),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(50, 50, 50));
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| surface.blend(&ink, 0.25));

        // The box the value is drawn in, which honours the caller's `padding`. It is the field
        // inset on every side, so the text and the caret share one origin in both modes.
        let text_box = self.text_rect();
        if self.is_editing {
            // Draw editing mode
            let style = self.base.style();
            context.face_with_gradient(
                rect,
                surface,
                style.background_gradient.as_ref(),
                style.surface.unwrap_or_default(),
                style.border_radius.unwrap_or(0),
                Color::BLACK,
            );
            // The focus ring follows the field's radius: a square 2 px ring left the corners bare.
            crate::widget::draw::stroke_surface_border(
                context,
                rect,
                style.border_radius.unwrap_or(0),
                accent,
                2,
            );

            // Draw text, on the field's own line box. The old anchor was
            // `rect.y + padding + font_size`, which put the glyph origin a *font size* below
            // the field's top edge — the text was drawn on the row after the one it belonged
            // to. `context.text_line` returns the field's line box, so the value is centred
            // in the field rather than positioned by two unrelated offsets.
            let text_x = text_box.x;
            let line = context.text_line(text_box, &font);
            // The visible window follows the caret: a value wider than the box is scrolled by the
            // caret's overflow instead of painting past the field (I-19). A value that fits keeps
            // `scroll == 0`, i.e. exactly the previous pixels but now elided to the box.
            let caret_advance = self.advance_of_prefix(self.cursor_position);
            let scroll = (caret_advance - text_box.width as i32).max(0);
            // The selection band is painted under the glyphs, with both edges measured by the same
            // ruler the caret uses. It used to be invisible: the control had no selection state at
            // all, so a drag or a shift-extension only moved an invisible anchor.
            if let Some((start, end)) = self.selection_range() {
                let left = text_x + self.advance_of_prefix(start) - scroll;
                let right = text_x + self.advance_of_prefix(end) - scroll;
                if right > left {
                    let highlight = ink.blend(&surface, 0.3).blend(&surface, 0.25);
                    context.fill_rect(
                        Rect::new(left, line.y, (right - left) as u32, line.height.max(1)),
                        highlight,
                    );
                }
            }
            if scroll == 0 {
                context.draw_text_fitted_cow(
                    Rect::new(text_x, line.y, text_box.width, line.height.max(1)),
                    &self.text,
                    &font,
                    ink,
                    HorizontalAlignment::Left,
                );
            } else {
                context.push_clip(text_x, line.y, text_box.width, line.height.max(1));
                context.draw_text(
                    Point::new(text_x - scroll, line.y),
                    &self.text,
                    &font,
                    ink,
                    HorizontalAlignment::Left,
                );
                context.pop_clip();
            }

            // Draw the caret, but only during the visible half of its cycle. The state comes from
            // the control's `CursorBlink`, which `tick` advances; drawing it unconditionally was
            // the "blinking cursor" that never blinked.
            if self.cursor_blink.is_visible() {
                let cursor_x = (text_x + caret_advance - scroll)
                    .min(text_x + text_box.width as i32 - 1)
                    .max(text_x);
                context.draw_line(
                    Point::new(cursor_x, line.y),
                    Point::new(cursor_x, line.y + line.height as i32),
                    ink.with_alpha_f32(0.8),
                );
            }
        } else {
            // Draw display mode
            let style = self.base.style();
            context.face_with_gradient(
                rect,
                surface,
                style.background_gradient.as_ref(),
                style.surface.unwrap_or_default(),
                style.border_radius.unwrap_or(0),
                Color::BLACK,
            );
            // Rounded like the field's face, so the outline closes at all four corners.
            crate::widget::draw::stroke_surface_border(
                context,
                rect,
                style.border_radius.unwrap_or(0),
                border,
                1,
            );

            let text_x = text_box.x;
            let line = context.text_line(text_box, &font);
            // Fitted to the field: the display value used to paint straight past the box edge
            // when it was wider than the field (I-19).
            context.draw_text_fitted_cow(
                Rect::new(text_x, line.y, text_box.width, line.height.max(1)),
                &self.text,
                &font,
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for InplaceEditor {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                // Entering edit mode needs the double-click to land on the editor.
                // Discarding the position meant a double-click anywhere in the window
                // put this control into edit mode with no way for the user to cancel it.
                //
                // The test is against the **painted field**, not the control's rectangle:
                // a double-click on the empty space below a 48 px field in a tall cell is on
                // the window background, and starting an edit there would put the user into
                // a mode they never asked for.
                if self.edit_triggers.contains(EditTriggers::DOUBLE_CLICK)
                    && self.field_rect().contains_point(*pos)
                {
                    self.start_edit();
                    let index = self.char_index_at_x(pos.x);
                    self.move_caret(index, false);
                }
            }
            // A single click enters edit mode when that trigger is enabled, and places the caret
            // when the editor is already editing. Before this the click was ignored entirely, so
            // the caret stayed wherever `start_edit` had put it (the end of the value).
            Event::MousePress { pos, button, modifiers, .. }
                if *button == 1 && self.field_rect().contains_point(*pos) =>
            {
                if !self.is_editing {
                    if self.edit_triggers.contains(EditTriggers::SINGLE_CLICK) {
                        self.start_edit();
                    } else {
                        self.base.handle_event(event);
                        return;
                    }
                }
                let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::SHIFT);
                let index = self.char_index_at_x(pos.x);
                if !shift {
                    self.selection_anchor = Some(index);
                }
                self.cursor_position = index;
                self.dragging_selection = !shift;
                self.base.request_redraw();
            }
            Event::MouseMove { pos } => {
                if self.dragging_selection {
                    self.cursor_position = self.char_index_at_x(pos.x);
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                if self.dragging_selection {
                    self.dragging_selection = false;
                    if self.selection_anchor == Some(self.cursor_position) {
                        self.selection_anchor = None;
                    }
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                if !self.is_editing {
                    if *key == crate::event::key_codes::F2
                        && self.edit_triggers.contains(EditTriggers::F2)
                    {
                        self.start_edit();
                        return;
                    }
                    self.base.handle_event(event);
                    return;
                }
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                match *key {
                    // Undo and redo are the platform primary chord: Control+Z/Y on
                    // Windows and Linux, Command+Z/Y on macOS. A shifted chord is a
                    // different binding and stays unbound here.
                    _ if primary_shortcut_pressed(*key, *modifiers, Key::Z) => {
                        let _ = self.undo();
                    }
                    _ if primary_shortcut_pressed(*key, *modifiers, Key::Y) => {
                        let _ = self.redo();
                    }
                    0x1B => {
                        // Escape - cancel
                        self.selection_anchor = None;
                        self.finish_edit(false);
                    }
                    0x0D => {
                        // Enter — accept. Tab is **not** an accept: it must reach the focus
                        // navigation path, so a user can leave the cell without committing the
                        // edit (I-18).
                        self.selection_anchor = None;
                        self.finish_edit(true);
                    }
                    0x09 => {
                        // Tab hands the event back to the base widget's focus router (I-18).
                        self.selection_anchor = None;
                        self.base.handle_event(event);
                    }
                    0x08 => {
                        // Backspace
                        self.insert_char('\u{7f}');
                    }
                    0x2E => {
                        // Delete (forward)
                        self.delete_forward();
                    }
                    0x25 => {
                        // Left arrow
                        let target = self.cursor_position.saturating_sub(1);
                        self.move_caret(target, shift);
                    }
                    0x27 => {
                        // Right arrow
                        self.move_caret(self.cursor_position + 1, shift);
                    }
                    0x24 | 0x26 => {
                        // Home (and Up in a single-line editor) — the start of the value.
                        self.move_caret(0, shift);
                    }
                    0x23 | 0x28 => {
                        // End (and Down) — the end of the value.
                        self.move_caret(self.text.chars().count(), shift);
                    }
                    _ => {
                        // Printable characters
                        if let Some(c) = char::from_u32(*key) {
                            if c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation()
                            {
                                self.insert_committed_text(&c.to_string());
                            }
                        }
                    }
                }
            }
            // Platform-committed text (D09-INPUT-01). A printable character or an IME commit reaches
            // the editor as `TextInput`/`ImeCommit`, not as a `KeyPress`, so this branch is what
            // actually receives typing. It funnels into the shared `insert_committed_text`, which
            // drives `insert_char` — the same entry point the printable key arm uses. The editor only
            // accepts text while it is in edit mode, so a `TextInput` in display mode is ignored
            // exactly as the key path ignores keys outside edit mode.
            Event::TextInput { text } | Event::ImeCommit { text } if self.is_editing => {
                self.insert_committed_text(text);
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn inplace_editor_initial_state() {
        let ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        assert_eq!(ie.text(), "Hello");
        assert!(!ie.is_editing());
        assert!((ie.font_size() - 14.0).abs() < 0.01);
        assert_eq!(ie.padding(), dimensions::TEXT_FIELD_PADDING_H as i32);
        assert_eq!(ie.kind(), WidgetKind::InplaceEditor);
    }

    /// The edit caret blinks only in edit mode.
    ///
    /// The module has always documented a "blinking cursor", but the caret was a solid line and
    /// nothing ever scheduled another frame. This pins the two halves of the lifecycle: display mode
    /// owes no frame, edit mode keeps blinking, and finishing the edit stops it.
    #[test]
    fn the_edit_caret_blinks_only_in_edit_mode() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));

        assert!(!ie.tick(10_000), "display mode has no caret to animate");

        ie.start_edit();
        assert!(ie.tick(0), "entering edit mode starts the blink");
        for _ in 0..10 {
            assert!(ie.tick(500), "a blink is periodic and never settles");
        }

        ie.finish_edit(true);
        assert!(!ie.tick(500), "leaving edit mode stops the blink");
    }

    /// `is_animating` agrees with `tick`: a field owes a frame exactly while it is editing.
    ///
    /// # The defect this pins
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host consults the
    /// answer to decide whether a frame is needed **before** paying for a sweep. An editing field
    /// whose caret was mid-blink answered `false`, so such a host would stop scheduling frames between
    /// two blinks and the caret would stop cycling — the same unreachable animation the trait `tick`
    /// was lifted for, one layer up.
    ///
    /// # Why it is asserted against `tick`
    ///
    /// The two answers describe one animation, so a version that disagreed with `tick` in either
    /// direction would be wrong whichever value it picked: `false` while ticking freezes the caret,
    /// `true` while settled pins the host at frame rate. Comparing them is what makes a drift between
    /// the two impossible to land.
    #[test]
    fn is_animating_agrees_with_tick() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        assert!(!ie.is_animating(), "display mode owes no frames");

        ie.start_edit();
        assert!(ie.is_animating(), "edit mode is animating the caret");

        ie.finish_edit(true);
        assert!(!ie.is_animating(), "leaving edit mode settles the caret");
        // And the two statements agree at rest, which is the direction that matters for the host.
        assert_eq!(ie.is_animating(), ie.tick(0));
    }

    #[test]
    fn inplace_editor_set_text() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.set_text("World");
        assert_eq!(ie.text(), "World");
    }

    #[test]
    fn inplace_editor_start_and_finish_edit_accept() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));

        let accepted = std::sync::Arc::new(std::sync::Mutex::new(None));
        let accepted_clone = accepted.clone();
        ie.edit_accepted.connect(move |text| {
            *accepted_clone.lock().unwrap() = Some((*text).clone());
        });

        ie.start_edit();
        assert!(ie.is_editing());

        ie.set_text("Hello World");
        ie.finish_edit(true);

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Hello World");
        assert_eq!(*accepted.lock().unwrap(), Some("Hello World".to_string()));
    }

    #[test]
    fn inplace_editor_start_and_finish_edit_cancel() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));

        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancelled_clone = cancelled.clone();
        ie.edit_canceled.connect(move || {
            cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        ie.start_edit();
        assert!(ie.is_editing());

        ie.set_text("Modified Text");
        ie.finish_edit(false);

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Hello"); // Should revert to original
        assert!(cancelled.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn inplace_editor_double_click_starts_edit() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.handle_event(&Event::MouseDoubleClick { pos: Point::new(50, 15), button: 1 });
        assert!(ie.is_editing());
    }

    /// The field is a full-width band one text-field height tall, centred in the control.
    ///
    /// The control painted its whole rectangle, so a 240x120 census cell drew a 240x120 box
    /// and the value was anchored from its oversized top edge.
    #[test]
    fn the_field_is_a_text_field_height_in_any_rectangle() {
        for height in [48u32, 120, 300] {
            let ie = InplaceEditor::new("Sample", Rect::new(0, 0, 240, height));
            let field = ie.field_rect();
            assert_eq!(
                field.height,
                dimensions::TEXT_FIELD_MIN_HEIGHT,
                "at control height {height}"
            );
            assert_eq!(field.width, 240, "the field spans the control's width");
            assert_eq!(field.y, (height - field.height) as i32 / 2, "at control height {height}");
        }

        let short = InplaceEditor::new("Sample", Rect::new(0, 0, 240, 20));
        assert_eq!(short.field_rect().height, 20, "a short control clamps the field");
    }

    /// A double-click below the field does not start an edit.
    ///
    /// With the field centred in a 120 px cell, a double-click inside the control's
    /// rectangle but below the drawn band is on the window background; starting an edit
    /// there puts the user into a mode they never asked for.
    #[test]
    fn a_double_click_below_the_drawn_field_does_not_start_an_edit() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 240, 120));
        let field = ie.field_rect();

        ie.handle_event(&Event::MouseDoubleClick {
            pos: Point::new(field.x + 10, field.y + field.height as i32 / 2),
            button: 1,
        });
        assert!(ie.is_editing(), "a double-click on the drawn field starts an edit");

        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 240, 120));
        ie.handle_event(&Event::MouseDoubleClick {
            pos: Point::new(field.x + 10, field.y + field.height as i32 + 40),
            button: 1,
        });
        assert!(!ie.is_editing(), "a double-click below the drawn field must not");
    }

    /// The value is drawn on the field's own line box, not a font size below its top edge.
    ///
    /// The old anchor was `rect.y + padding + font_size`, which put the glyph origin a whole
    /// font size down from the field's top — the value was drawn on the row *after* the one
    /// it belonged to, and the caret spanned a different band from the text.
    ///
    /// # Why the check is on the ink
    ///
    /// The value is no longer a `<text>` element carrying a `y`: the backend emits the same
    /// `font8x8` rectangles the software rasteriser fills, as subpaths of a single `<path>`
    /// (see `crate::widget::svg::text_ink_box`). The string is absent from the document, and
    /// the ink box is the stronger witness anyway — it is where the glyphs landed, not what an
    /// element claimed.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_value_sits_inside_the_field() {
        let mut ie = InplaceEditor::new("Sample", Rect::new(0, 0, 240, 120));
        ie.start_edit();
        let svg = crate::widget::svg::render_to_svg(&mut ie);
        let field = ie.field_rect();

        let (_, top, _, bottom) = crate::widget::svg::text_ink_box(&svg)
            .expect("an editor with text draws it as glyph geometry");
        assert!(top >= field.y, "the value starts inside the field: y={top}, field={field:?}");
        assert!(
            bottom <= field.y + field.height as i32,
            "and above the field's bottom edge: y={bottom}, field={field:?}"
        );
    }

    #[test]
    fn inplace_editor_escape_cancels_edit() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.set_text("Changed");

        ie.handle_event(&Event::KeyPress {
            key: 0x1B, // Escape
            modifiers: 0,
        });

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Hello");
    }

    #[test]
    fn inplace_editor_enter_accepts_edit() {
        let mut ie = InplaceEditor::new("Hello", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.set_text("Accepted");

        ie.handle_event(&Event::KeyPress {
            key: 0x0D, // Enter
            modifiers: 0,
        });

        assert!(!ie.is_editing());
        assert_eq!(ie.text(), "Accepted");
    }

    #[test]
    fn inplace_editor_set_font_size_and_padding() {
        let mut ie = InplaceEditor::new("Test", Rect::new(0, 0, 200, 30));

        ie.set_font_size(18.0);
        assert!((ie.font_size() - 18.0).abs() < 0.01);

        ie.set_font_size(0.0); // Should clamp
        assert!((ie.font_size() - 4.0).abs() < 0.01);

        ie.set_padding(8);
        assert_eq!(ie.padding(), 8);

        ie.set_padding(-5); // Should clamp to 0
        assert_eq!(ie.padding(), 0);
    }

    #[test]
    fn inplace_editor_insert_characters() {
        let mut ie = InplaceEditor::new("", Rect::new(0, 0, 200, 30));
        ie.start_edit();

        // Simulate typing
        ie.insert_char('A');
        ie.insert_char('B');
        ie.insert_char('C');
        assert_eq!(ie.text(), "ABC");
        assert_eq!(ie.cursor_position, 3);

        // Backspace
        ie.insert_char('\u{7f}');
        assert_eq!(ie.text(), "AB");
        assert_eq!(ie.cursor_position, 2);
    }

    #[test]
    fn a_committed_phrase_is_one_undo_step() {
        // I-P14: a phrase used to be inserted one character at a time, so one IME commit became N
        // undo steps with a caret that walked through them.
        let mut ie = InplaceEditor::new("", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.insert_committed_text("hello");
        assert_eq!(ie.text(), "hello");
        assert_eq!(ie.cursor_position, 5);
        assert!(ie.undo());
        assert_eq!(ie.text(), "");
    }

    /// `padding` moves the value: the text box is the field inset by the caller's padding.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `padding` was stored, published (`set_padding` clamps it to `>= 0` and requests a redraw)
    /// and read by nothing: `draw` used the fixed `dimensions::TEXT_FIELD_PADDING_H` in both modes.
    /// The field is unchanged; only the value's box moves, which is what "padding around the
    /// text" means.
    #[test]
    fn padding_insets_the_value_inside_the_field() {
        let mut ie = InplaceEditor::new("Test", Rect::new(0, 0, 200, 30));
        let field = ie.field_rect();

        // The default padding is the shared text-field inset, so a default editor is unchanged.
        assert_eq!(
            ie.padding(),
            dimensions::TEXT_FIELD_PADDING_H as i32,
            "the default is the crate's own field inset"
        );
        assert_eq!(
            ie.text_rect().x,
            field.x + dimensions::TEXT_FIELD_PADDING_H as i32,
            "a default editor's value starts at the pre-existing inset"
        );

        ie.set_padding(12);
        let boxed = ie.text_rect();
        assert_eq!(boxed.x, field.x + 12, "a larger padding moves the value in");
        assert_eq!(
            boxed.width,
            field.width - 24,
            "and takes it off both sides, not just the leading one"
        );
        assert_eq!(boxed.y, field.y, "the field's own vertical band is unchanged");

        // Zero padding is the value flush with the field's edge.
        ie.set_padding(0);
        assert_eq!(ie.text_rect().x, field.x, "zero padding is flush with the field");

        // A padding wider than the field cannot invert the box: the inset is clamped to half.
        ie.set_padding(10_000);
        let clamped = ie.text_rect();
        assert!(clamped.width >= 1, "a clamped box still has width: {clamped:?}");
        assert!(clamped.x >= field.x, "and is still inside the field: {clamped:?}");
    }

    // ─── D09-INPUT-01: platform-committed text reaches the value ───

    /// A `TextInput` in edit mode must enter the editor's value.
    ///
    /// # The defect this pins (D09-INPUT-01)
    ///
    /// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
    /// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
    /// test feeds `TextInput` to prove that path itself is wired.
    #[test]
    fn text_input_enters_the_value_while_editing() {
        let mut ie = InplaceEditor::new("", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.handle_event(&Event::TextInput { text: "héllo".to_string() });
        assert_eq!(ie.text(), "héllo");
    }

    /// An IME commit enters the value the same way.
    #[test]
    fn ime_commit_enters_the_value_while_editing() {
        let mut ie = InplaceEditor::new("", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.handle_event(&Event::ime_commit("你好"));
        assert_eq!(ie.text(), "你好");
    }

    /// Committed text is ignored outside edit mode, matching the key gate.
    #[test]
    fn text_input_is_ignored_outside_edit_mode() {
        let mut ie = InplaceEditor::new("base", Rect::new(0, 0, 200, 30));
        ie.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(ie.text(), "base", "display mode must not take committed text");
    }

    /// A disabled editor ignores committed text.
    #[test]
    fn disabled_ignores_text_input() {
        let mut ie = InplaceEditor::new("", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.set_enabled(false);
        ie.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(ie.text(), "");
    }

    /// Edit triggers are configurable: a single click can start an edit, and `NONE` refuses both.
    #[test]
    fn edit_triggers_are_configurable() {
        let rect = Rect::new(0, 0, 200, 30);
        let mut single = InplaceEditor::new("value", rect);
        single.set_edit_triggers(EditTriggers::SINGLE_CLICK);
        single.handle_event(&Event::MousePress {
            pos: Point::new(10, 10),
            button: 1,
            modifiers: 0,
        });
        assert!(single.is_editing(), "a single click starts an edit when configured");

        let mut never = InplaceEditor::new("value", rect);
        never.set_edit_triggers(EditTriggers::NONE);
        never.handle_event(&Event::MouseDoubleClick { pos: Point::new(10, 10), button: 1 });
        assert!(!never.is_editing(), "NONE must refuse the default gesture too");
    }

    /// A click places the caret and a drag selects between the press and the pointer.
    #[test]
    fn a_click_places_the_caret_and_a_drag_selects() {
        let mut ie = InplaceEditor::new("hello world", Rect::new(0, 0, 240, 30));
        ie.set_edit_triggers(EditTriggers::SINGLE_CLICK);
        ie.handle_event(&Event::MousePress { pos: Point::new(60, 10), button: 1, modifiers: 0 });
        assert!(ie.cursor_position > 0 && ie.cursor_position < ie.text().chars().count());
        let anchor = ie.cursor_position;
        ie.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        let (start, end) = ie.selection_range().expect("a drag must select");
        assert!(end > start);
        assert!(anchor >= start, "the press is one end of the range");
        assert_eq!(ie.selected_text(), ie.text()[start..end].to_string());
    }

    /// Home and End move the caret to the ends, extending under Shift.
    #[test]
    fn home_end_and_arrows_move_the_caret() {
        let mut ie = InplaceEditor::new("abc", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.handle_event(&Event::KeyPress { key: 0x24, modifiers: 0 });
        assert_eq!(ie.cursor_position, 0);
        ie.handle_event(&Event::KeyPress { key: 0x23, modifiers: 0 });
        assert_eq!(ie.cursor_position, 3);
        ie.handle_event(&Event::KeyPress { key: 0x24, modifiers: 0b0001 });
        assert_eq!(ie.selection_range(), Some((0, 3)), "Shift+Home extends to the start");
    }

    /// Forward delete has its own entry point and does not go through `insert_char`'s sentinel.
    #[test]
    fn delete_forward_removes_the_character_after_the_caret() {
        let mut ie = InplaceEditor::new("abc", Rect::new(0, 0, 200, 30));
        ie.start_edit();
        ie.cursor_position = 1;
        assert!(ie.delete_forward());
        assert_eq!(ie.text(), "ac");
        assert_eq!(ie.cursor_position, 1);
        ie.cursor_position = 2;
        assert!(!ie.delete_forward(), "there is nothing after the end");
    }
}
