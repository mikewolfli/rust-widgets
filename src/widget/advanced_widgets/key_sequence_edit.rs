// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Key sequence editor widget for capturing keyboard shortcuts.
use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// Represents a key sequence (modifier + key name).
///
/// A sequence is a single chord, not a multi-step sequence: one modifier
/// bitmask plus one key. Used by [`KeySequenceEdit`] to describe a shortcut.
///
/// The `key_name` is a display-only, non-localised English rendering of
/// `key_code` ("Ctrl+S", "F5"); it is never parsed back into a code and is not
/// validated against it, so the two can disagree if a caller sets them
/// independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySequence {
    modifiers: u32, // Bit flags: 0x01=Ctrl, 0x02=Alt, 0x04=Shift, 0x08=Meta
    key_code: u32,
    key_name: String,
}
impl KeySequence {
    /// Creates a fully specified sequence.
    ///
    /// `modifiers` is a bitmask: `0x01` Ctrl, `0x02` Alt, `0x04` Shift, `0x08`
    /// Meta; other bits are ignored by [`KeySequence::to_display_string`].
    /// `key_code` is the backend virtual-key code, `0` meaning "no key".
    /// `key_name` is the display text, passed through verbatim.
    pub fn new(modifiers: u32, key_code: u32, key_name: impl Into<String>) -> Self {
        Self { modifiers, key_code, key_name: key_name.into() }
    }
    /// Returns a sequence with no modifiers, key code `0`, and no key name.
    ///
    /// All-zero is the sentinel for "unset": see [`KeySequence::is_empty`].
    pub fn empty() -> Self {
        Self { modifiers: 0, key_code: 0, key_name: String::new() }
    }
    /// Returns the modifier bitmask (`0x01` Ctrl, `0x02` Alt, `0x04` Shift,
    /// `0x08` Meta).
    pub fn modifiers(&self) -> u32 {
        self.modifiers
    }
    /// Returns the backend virtual-key code, or `0` when no key is set.
    pub fn key_code(&self) -> u32 {
        self.key_code
    }
    /// Returns the display name of the key (for example `"S"`, `"F5"`,
    /// `"Space"`). Empty when no key is set.
    pub fn key_name(&self) -> &str {
        &self.key_name
    }
    /// Replaces the modifier bitmask; see [`KeySequence::modifiers`] for the bit
    /// assignments. Does not update `key_name`.
    pub fn set_modifiers(&mut self, modifiers: u32) {
        self.modifiers = modifiers;
    }
    /// Replaces the virtual-key code. Does not update `key_name`, so the
    /// displayed text can disagree with the stored code until the name is set
    /// too.
    pub fn set_key_code(&mut self, key_code: u32) {
        self.key_code = key_code;
    }
    /// Replaces the display name of the key.
    pub fn set_key_name(&mut self, key_name: impl Into<String>) {
        self.key_name = key_name.into();
    }
    /// Returns `true` when `key_code` is `0`, i.e. no key has been captured.
    ///
    /// Cleared sequences are the widget's "unset shortcut" state; the widget
    /// shows placeholder text for them.
    pub fn is_empty(&self) -> bool {
        self.key_code == 0
    }
    /// Renders the sequence for display, joining recognised modifiers and the
    /// key name with `+` (for example `"Ctrl+Shift+S"`).
    ///
    /// Modifier order is fixed as Ctrl, Shift, Alt, Meta — it does not follow
    /// the `modifiers` bit order or platform convention. Unrecognised modifier
    /// bits are silently dropped, and an empty `key_name` yields only the
    /// modifier prefix (or an empty string when there are none).
    pub fn to_display_string(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers & 0x01 != 0 {
            parts.push("Ctrl");
        }
        if self.modifiers & 0x04 != 0 {
            parts.push("Shift");
        }
        if self.modifiers & 0x02 != 0 {
            parts.push("Alt");
        }
        if self.modifiers & 0x08 != 0 {
            parts.push("Meta");
        }
        if !self.key_name.is_empty() {
            parts.push(&self.key_name);
        }
        parts.join("+")
    }
}
impl std::fmt::Display for KeySequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}
/// Key sequence editor widget.
///
/// Clicking the widget starts recording; the next key press (with its
/// modifiers) becomes the sequence and ends recording. Recording can also be
/// driven programmatically with [`KeySequenceEdit::start_recording`].
///
/// Edits made through [`KeySequenceEdit::set_key_sequence`] — including those
/// from recording — are pushed onto an internal undo stack, so `undo` / `redo`
/// (and Ctrl+Z / Ctrl+Y) step through the edit history.
///
pub struct KeySequenceEdit {
    base: BaseWidget,
    key_sequence: KeySequence,
    recording: bool,
    /// The chords recorded so far. One chord unless multi-chord recording is enabled.
    chords: Vec<KeySequence>,
    /// How many chords one recording may collect (`1..=4`).
    max_chords: usize,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field by the field's own layout,
    /// so a `top`/`bottom` value is one this control could never honour — the setter refuses it
    /// rather than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// Emitted once each time recording stops, whether the sequence changed or
    /// not (including when recording was cancelled with Escape, which emits
    /// nothing else). Carries no payload.
    pub editing_finished: GenericSignal,
    /// Emitted with the new sequence whenever it changes: from user recording,
    /// from [`KeySequenceEdit::set_key_sequence`], and from undo/redo replayed
    /// history. Not emitted by [`KeySequenceEdit::set_key_sequence`] when the
    /// incoming sequence equals the current one.
    pub key_sequence_changed: Signal1<KeySequence>,
    /// Sequences already bound elsewhere; a recorded/assigned sequence matching one emits
    /// [`Self::conflict_detected`].
    conflicting_sequences: Vec<KeySequence>,
    /// Emitted with the sequence that collides with [`Self::conflicting_sequences`].
    pub conflict_detected: Signal1<KeySequence>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<KeySequence>>,
    restoring_history: bool,
    /// The text [`Draw`] paints, kept between frames.
    ///
    /// The draw path used to `collect` + `join` the chord list and format the modifier prefix on
    /// every frame, so an idle field allocated two or three strings per paint for a value that had
    /// not changed. Every write that can change the text marks this cache dirty through
    /// [`KeySequenceEdit::invalidate_display`], and `draw` rebuilds only when it is. The rendered
    /// string is byte-identical to what the per-frame form produced.
    display_text: String,
    /// Whether [`KeySequenceEdit::display_text`] still describes the current state.
    display_dirty: bool,
}
impl KeySequenceEdit {
    /// Creates an editor with no sequence set and recording stopped.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 150x28.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "KeySequenceEdit"),
            key_sequence: KeySequence::empty(),
            recording: false,
            chords: Vec::new(),
            max_chords: 1,
            alignment: crate::core::Alignment::Left,
            editing_finished: GenericSignal::new(),
            key_sequence_changed: Signal1::new(),
            conflicting_sequences: Vec::new(),
            conflict_detected: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(KeySequence::empty())),
            restoring_history: false,
            display_text: String::new(),
            display_dirty: true,
        }
    }
    /// Returns the current sequence; compare with [`KeySequence::is_empty`] to
    /// distinguish "unset" from "set".
    pub fn key_sequence(&self) -> &KeySequence {
        &self.key_sequence
    }

    /// How the field's value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the value is centred
    /// vertically in the field by the field's own layout, so such a value is one this setter
    /// cannot honour and it returns without writing it.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }
    /// Returns whether the widget is currently capturing the next key press.
    pub fn is_recording(&self) -> bool {
        self.recording
    }
    /// Replaces the sequence and requests a redraw.
    ///
    /// Setting a value equal to the current one is a no-op: no undo entry, no
    /// signal, and no redraw. Otherwise the previous value is pushed onto the
    /// undo stack and `key_sequence_changed` is emitted with `seq`.
    ///
    /// This does **not** stop an in-progress recording; recording continues and
    /// the next captured key press will overwrite this value.
    pub fn set_key_sequence(&mut self, seq: KeySequence) {
        if self.key_sequence == seq {
            return;
        }
        let before = self.key_sequence.clone();
        self.key_sequence = seq.clone();
        self.chords = if seq.is_empty() { Vec::new() } else { vec![seq.clone()] };
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.key_sequence.clone();
            self.undo_stack.push(Box::new(ValueCommand::new(
                self.history_target.clone(),
                before,
                self.key_sequence.clone(),
                "Edit key sequence",
                "key_sequence_edit",
            )));
        }
        self.key_sequence_changed.emit(seq.clone());
        if !seq.is_empty() && self.conflicting_sequences.contains(&seq) {
            self.conflict_detected.emit(seq);
        }
        self.invalidate_display();
        self.base.request_redraw();
    }

    /// Replaces the set of already-bound sequences used for conflict detection.
    pub fn set_conflicting_sequences(&mut self, sequences: Vec<KeySequence>) {
        self.conflicting_sequences = sequences;
    }

    /// The sequences treated as already bound.
    pub fn conflicting_sequences(&self) -> &[KeySequence] {
        &self.conflicting_sequences
    }
    /// Clears the sequence, as if [`KeySequence::empty`] had been set.
    ///
    /// Goes through [`KeySequenceEdit::set_key_sequence`], so it is undoable
    /// and is a no-op when the sequence is already empty.
    pub fn clear(&mut self) {
        self.set_key_sequence(KeySequence::empty());
    }

    /// The chords the recorded sequence contains, in order.
    pub fn chords(&self) -> &[KeySequence] {
        &self.chords
    }

    /// How many chords are recorded.
    pub fn sequence_len(&self) -> usize {
        self.chords.len()
    }

    /// How many chords one recording may collect (`1..=4`).
    pub fn max_chords(&self) -> usize {
        self.max_chords
    }

    /// Sets the multi-chord limit, clamped to `1..=4`.
    ///
    /// The default of `1` keeps the historical single-chord recording: the first key
    /// press ends the capture. A larger limit lets a user enter a sequence such as
    /// `Ctrl+X, Ctrl+C`, and recording then ends on Escape, focus loss, or the
    /// [`KeySequenceEdit::commit_recording`] call.
    pub fn set_max_chords(&mut self, max_chords: usize) {
        let max_chords = max_chords.clamp(1, 4);
        if self.max_chords == max_chords {
            return;
        }
        self.max_chords = max_chords;
        self.chords.truncate(max_chords);
        if let Some(last) = self.chords.last().cloned() {
            self.key_sequence = last;
        }
        self.invalidate_display();
        self.base.request_redraw();
    }

    /// Replaces the chord list.
    ///
    /// The last chord becomes [`KeySequenceEdit::key_sequence`], so the single-chord
    /// accessors keep answering the effective shortcut. An empty list clears the
    /// sequence.
    pub fn set_chords(&mut self, chords: Vec<KeySequence>) {
        let mut chords: Vec<KeySequence> =
            chords.into_iter().filter(|chord| !chord.is_empty()).collect();
        chords.truncate(4);
        match chords.last().cloned() {
            Some(last) => {
                self.set_key_sequence(last);
                self.chords = chords;
                self.base.request_redraw();
            }
            None => self.clear(),
        }
    }

    /// Ends a multi-chord recording and keeps what was captured.
    ///
    /// With no captured chord this only stops recording, which is the same answer
    /// Escape gives.
    pub fn commit_recording(&mut self) {
        self.stop_recording();
    }

    /// Whether any chord in `self` has the same chord in `other`.
    ///
    /// A host uses this to refuse a duplicate binding before it is saved; the control
    /// itself cannot know the other shortcuts in the application.
    pub fn conflicts_with(&self, other: &KeySequenceEdit) -> bool {
        self.chords.iter().any(|mine| other.chords.iter().any(|theirs| mine == theirs))
    }

    /// Translates an event modifier bitmask into this type's bitmask.
    ///
    /// The event convention is shift `1`, ctrl `2`, alt `4`, meta `8`; the sequence
    /// convention is ctrl `0x01`, alt `0x02`, shift `0x04`, meta `0x08`. Keeping the
    /// translation here removes the raw bit tests the recording path used.
    pub fn modifiers_from_event(bits: u32) -> u32 {
        let mut modifiers = 0;
        if bits & 0b0001 != 0 {
            modifiers |= 0x04; // shift
        }
        if bits & 0b0010 != 0 {
            modifiers |= 0x01; // ctrl
        }
        if bits & 0b0100 != 0 {
            modifiers |= 0x02; // alt
        }
        if bits & 0b1000 != 0 {
            modifiers |= 0x08; // meta
        }
        modifiers
    }
    /// Begins capturing the next key press. Idempotent, and does not emit
    /// anything. If no `Event::FocusLost` or key press follows, recording
    /// stays on indefinitely.
    pub fn start_recording(&mut self) {
        if !self.recording {
            self.recording = true;
            self.invalidate_display();
        }
    }
    /// Ends recording.
    ///
    /// Emits `editing_finished` only if recording was actually on, so calling
    /// this twice emits once — unless [`KeySequenceEdit::start_recording`] ran
    /// in between.
    pub fn stop_recording(&mut self) {
        if self.recording {
            self.recording = false;
            self.invalidate_display();
            self.editing_finished.emit();
        }
    }
    /// Steps back one edit and returns `true`, or returns `false` when there is
    /// nothing to undo.
    ///
    /// Emits `key_sequence_changed` with the restored sequence, but **not**
    /// `editing_finished`, even though the restored value may differ from what
    /// the user last confirmed.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_sequence();
        true
    }
    /// Steps forward one undone edit and returns `true`, or returns `false`
    /// when there is nothing to redo. Signals behave as in
    /// [`KeySequenceEdit::undo`].
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_sequence();
        true
    }
    /// Returns `true` if [`KeySequenceEdit::undo`] would change the sequence.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`KeySequenceEdit::redo`] would change the sequence.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }
    fn restore_history_sequence(&mut self) {
        self.restoring_history = true;
        let seq = self.history_target.borrow().clone();
        self.key_sequence = seq.clone();
        self.restoring_history = false;
        self.invalidate_display();
        self.key_sequence_changed.emit(seq);
        self.base.request_redraw();
    }

    /// Marks the cached render text stale; the next `draw` rebuilds it.
    ///
    /// Every path that writes `key_sequence`, `chords` or `recording` calls this. The cache exists
    /// for the *idle* frame, which is the overwhelming majority, so correctness is worth one call
    /// per write and those write paths are all in this file.
    fn invalidate_display(&mut self) {
        self.display_dirty = true;
    }

    /// Rebuilds [`Self::display_text`] when an input changed.
    ///
    /// The three shapes are the ones the draw path used to build inline: the recording prompt with
    /// the chords captured so far, the placeholder for an unset shortcut, and the set value (a
    /// multi-chord list joined with `, `). The strings are identical, including the space after
    /// `Recording...`.
    fn refresh_display_text(&mut self) {
        if !self.display_dirty {
            return;
        }
        self.display_text.clear();
        if self.recording {
            self.display_text.push_str("Recording...");
            if !self.chords.is_empty() {
                self.display_text.push(' ');
                for (index, chord) in self.chords.iter().enumerate() {
                    if index > 0 {
                        self.display_text.push_str(", ");
                    }
                    self.display_text.push_str(&chord.to_display_string());
                }
            }
        } else if self.key_sequence.is_empty() {
            self.display_text.push_str("Click to set shortcut...");
        } else if self.chords.len() > 1 {
            for (index, chord) in self.chords.iter().enumerate() {
                if index > 0 {
                    self.display_text.push_str(", ");
                }
                self.display_text.push_str(&chord.to_display_string());
            }
        } else {
            self.display_text.push_str(&self.key_sequence.to_display_string());
        }
        self.display_dirty = false;
    }
}
impl Widget for KeySequenceEdit {
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
        crate::core::Size::new(150, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves the control's published events to its signals.
    ///
    /// `sequence_changed` is the capability spelling of the control's `key_sequence_changed`;
    /// both name the same event so a document and a Rust caller cannot subscribe to different
    /// things.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "sequence_changed" | "key_sequence_changed" => {
                Some(EventSignalRef::mapped("sequence_changed", &self.key_sequence_changed, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([CapabilityValue::String(
                        v.to_display_string(),
                    )]))
                }))
            }
            "conflict_detected" => {
                Some(EventSignalRef::mapped("conflict_detected", &self.conflict_detected, |v| {
                    CapabilityValue::String(v.to_display_string())
                }))
            }
            "editing_finished" => {
                Some(EventSignalRef::unit("editing_finished", &self.editing_finished))
            }
            _ => None,
        }
    }
}
impl EventHandler for KeySequenceEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button, .. } if *button == 1 => {
                self.start_recording();
            }
            Event::FocusLost => {
                self.stop_recording();
            }
            Event::KeyPress { key, modifiers } if self.recording => {
                // Escape clears recording without saving
                if *key == key_codes::ESCAPE {
                    if self.recording {
                        self.recording = false;
                        self.invalidate_display();
                    }
                    return;
                }
                // Ignore modifier-only keys. The set matches the modifier bits
                // `modifiers_from_event` maps (including Meta/Super), so a lone modifier press is
                // not recorded as a shortcut key while a modified chord still is.
                if is_modifier_key(*key) {
                    return;
                }
                let key_name = key_code_to_name(*key);
                let modifiers = Self::modifiers_from_event(*modifiers);
                let chord = KeySequence::new(modifiers, *key, key_name);
                if self.max_chords <= 1 {
                    self.set_key_sequence(chord);
                    self.stop_recording();
                } else {
                    if self.chords.len() < self.max_chords {
                        self.chords.push(chord.clone());
                    }
                    self.key_sequence = chord;
                    self.invalidate_display();
                    self.base.request_redraw();
                    if self.chords.len() >= self.max_chords {
                        self.stop_recording();
                    }
                }
            }
            // Undo and redo are the platform primary chord: Control+Z/Y on
            // Windows and Linux, Command+Z/Y on macOS. A shifted chord is a
            // different binding and stays unbound here.
            Event::KeyPress { key, modifiers }
                if primary_shortcut_pressed(*key, *modifiers, Key::Z) =>
            {
                let _ = self.undo();
            }
            Event::KeyPress { key, modifiers }
                if primary_shortcut_pressed(*key, *modifiers, Key::Y) =>
            {
                let _ = self.redo();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
fn key_code_to_name(key: u32) -> String {
    match key {
        8 => "Backspace".into(),
        9 => "Tab".into(),
        13 => "Return".into(),
        27 => "Escape".into(),
        32 => "Space".into(),
        33 => "PageUp".into(),
        34 => "PageDown".into(),
        35 => "End".into(),
        36 => "Home".into(),
        37 => "Left".into(),
        38 => "Up".into(),
        39 => "Right".into(),
        40 => "Down".into(),
        46 => "Delete".into(),
        112 => "F1".into(),
        113 => "F2".into(),
        114 => "F3".into(),
        115 => "F4".into(),
        116 => "F5".into(),
        117 => "F6".into(),
        118 => "F7".into(),
        119 => "F8".into(),
        120 => "F9".into(),
        121 => "F10".into(),
        122 => "F11".into(),
        123 => "F12".into(),
        k if (65..=90).contains(&k) => (k as u8 as char).to_string(),
        // `k` in 48..=57 is the ASCII digit itself, so this is `(k as u8 as char)`, matching the
        // letter arm above. The previous `((k - 48) as u8 + b'0')` round-tripped through zero to
        // arrive back at `k`, which read as if it did something and invited a wrong "fix".
        k if (48..=57).contains(&k) => (k as u8 as char).to_string(),
        k => format!("Key{k}"),
    }
}

/// Whether `key` is one of the modifier keys themselves.
///
/// The five coded modifiers are the ones [`KeySequenceEdit::modifiers_from_event`] translates:
/// Shift (16), Control (17), Alt (18) and Meta/Super (91 left, 92 right, and 224, which some hosts
/// send for either). Recording ignores every one of them so a bare modifier press cannot become a
/// shortcut; a chord that *includes* one still records it through the event's modifier bits.
fn is_modifier_key(key: u32) -> bool {
    matches!(key, 16 | 17 | 18 | 91 | 92 | 224)
}

/// The virtual-key code for a display name this widget can produce.
///
/// The exact inverse of [`key_code_to_name`] over the names that function emits, plus the spellings
/// [`crate::shortcut::Key::from_string`] accepts. `None` for a name no key produces; the
/// `key_sequence` property treats that as an unset key (code `0`) rather than storing a sequence
/// whose display text and code disagree.
fn key_code_from_name(name: &str) -> Option<u32> {
    let upper = name.trim().to_ascii_uppercase();
    let code = match upper.as_str() {
        "BACKSPACE" | "BACK" => 8,
        "TAB" => 9,
        "RETURN" | "ENTER" => 13,
        "ESCAPE" | "ESC" => 27,
        "SPACE" => 32,
        "PAGEUP" | "PAGE_UP" => 33,
        "PAGEDOWN" | "PAGE_DOWN" => 34,
        "END" => 35,
        "HOME" => 36,
        "LEFT" => 37,
        "UP" => 38,
        "RIGHT" => 39,
        "DOWN" => 40,
        "DELETE" | "DEL" | "INSERT" | "INS" => 46,
        other => {
            if other.len() == 1 {
                let ch = other.as_bytes()[0];
                if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
                    return Some(u32::from(ch));
                }
                return None;
            }
            let number: u32 = other.strip_prefix('F')?.parse().ok()?;
            if (1..=12).contains(&number) {
                return Some(111 + number);
            }
            return None;
        }
    };
    Some(code)
}

/// Parses a `Ctrl+Shift+S` style token into a sequence.
///
/// Modifier names are case-insensitive and may appear in any order; the last segment is the key.
/// An empty token yields the empty sequence, which is how the property expresses "unset".
fn sequence_from_token(token: &str) -> KeySequence {
    let token = token.trim();
    if token.is_empty() {
        return KeySequence::empty();
    }
    let mut modifiers = 0u32;
    let mut key_part = None;
    for segment in token.split('+').map(str::trim).filter(|part| !part.is_empty()) {
        match segment.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= 0x01,
            "alt" | "option" => modifiers |= 0x02,
            "shift" => modifiers |= 0x04,
            "meta" | "cmd" | "command" | "super" | "win" => modifiers |= 0x08,
            _ => key_part = Some(segment),
        }
    }
    let Some(name) = key_part else {
        return KeySequence::empty();
    };
    let key_code = key_code_from_name(name).unwrap_or(0);
    KeySequence::new(modifiers, key_code, name)
}
impl Draw for KeySequenceEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // Rebuild the cached render text before anything borrows `self`, so an idle frame reuses the
        // string instead of collecting and joining the chords again (K-P06).
        self.refresh_display_text();
        let is_empty = self.key_sequence.is_empty();
        let recording = self.recording;
        // # Why every colour here is resolved rather than literal
        //
        // The field was painted from four literals and read the theme nowhere: a
        // `rgb(255,255,255)` surface, a `rgb(150,150,150)` frame, a `rgb(180,180,180)` hint and a
        // `rgb(0,0,0)` value, plus a fixed pale red for the recording state. On the dark appearance
        // that is a **white field with black ink**, and the recording state is a fixed wash rather
        // than the semantic colour the appearance defines. All of them now come from the active
        // theme, with the literals kept only as the rung for a build that has no theme.
        let themed = crate::style::palette_shared().map(|active| {
            (
                active.surface_container,
                active.outline_variant,
                active.foreground,
                active.secondary,
                active.error,
            )
        });
        let (surface, divider, ink, muted, error) = themed.unwrap_or((
            Color::rgb(255, 255, 255),
            Color::rgb(150, 150, 150),
            Color::rgb(0, 0, 0),
            Color::rgb(180, 180, 180),
            Color::rgb(200, 0, 0),
        ));
        let style = self.base.style();
        let surface = style.background_color.unwrap_or(surface);
        let ink = style.text_color.unwrap_or(ink);
        let divider = style.border_color.unwrap_or(divider);

        // Recording is a *state*, so it is expressed as a tint of the surface toward the semantic
        // error colour rather than as a second literal — the same derivation the preset theme's
        // state overlays use, and what keeps the two appearances in step when the palette moves.
        let bg = if recording { surface.blend(&error, 0.12) } else { surface };
        context.face_with_gradient(
            rect,
            bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Rounded to the face's radius: a square outline left the four corners unpainted.
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().border_radius.unwrap_or(0),
            if recording { error } else { divider },
            1,
        );
        // The hint is the weak ink; a recorded value is the field's own ink.
        let text_color = if is_empty && !recording { muted } else { ink };
        // Vertically centred through the shared primitive, and bounded to the field's own
        // width: the recorded sequence grows without limit (`Ctrl+Shift+Alt+Meta+K`), and the
        // SVG backend emits absolute coordinates, so an unbounded one ran past the field.
        // The **effective font** — the resolved theme/caller font — so the recorded sequence
        // honours the theme body font and the user's text scale (D09-STYLE-01).
        let font = effective_font(style);
        let line = context.text_line(rect, font);
        let display = self.display_text.as_str();
        context.draw_text_fitted_cow(
            Rect {
                x: rect.x + 6,
                y: line.y,
                width: rect.width.saturating_sub(12),
                height: line.height,
            },
            display,
            font,
            text_color,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );
    }
}

/// `KeySequenceEdit`'s property contract.
///
/// The four keys below are the control's own state, not just the shared base set: without them a
/// designer or a document could not read or set the shortcut it exists for, could not start
/// recording, and could not see whether history had an entry to step to. Each delegates to the
/// existing method so the capability path and the Rust path cannot disagree.
impl WidgetProperties for KeySequenceEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "key_sequence" => Ok(CapabilityValue::String(self.key_sequence.to_display_string())),
            "recording" => Ok(CapabilityValue::Bool(self.recording)),
            "can_undo" => Ok(CapabilityValue::Bool(self.can_undo())),
            "can_redo" => Ok(CapabilityValue::Bool(self.can_redo())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "key_sequence" => {
                let token = crate::widget::capability::coercion::expect_string(value)?;
                self.set_key_sequence(sequence_from_token(&token));
                Ok(())
            }
            "recording" => {
                let recording = crate::widget::capability::coercion::expect_bool(value)?;
                if recording {
                    self.start_recording();
                } else {
                    self.stop_recording();
                }
                self.base.request_redraw();
                Ok(())
            }
            "can_undo" | "can_redo" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["key_sequence", "recording", "can_undo", "can_redo", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::core::Rect;
    use crate::event::Event;
    use crate::widget::input_widgets::shortcut_editor::primary_modifier_event_bits;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    // ── 1. Creating default widget ──────────────────────────────────

    /// The field's surface and ink follow the appearance.
    ///
    /// # The defect this pins
    ///
    /// `Draw` read no style at all: four literals plus a fixed pale red for the recording state, so
    /// the field painted **white with black ink** on the dark appearance and its recording state was a
    /// wash rather than the semantic colour the theme defines. `audit_appearance.py` counted it among
    /// the Draw files reading no style colour.
    ///
    /// The assertion reads the surface's own fill out of the document, so it names the element under
    /// test rather than comparing two whole documents — which any other changed colour would satisfy.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_field_surface_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        let surface_fill = |appearance| -> String {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 200, 30));
            crate::theme::apply_theme_to_widget(&mut kse);
            let svg = render_to_svg(&mut kse);
            // The **last** `<rect>` whose fill is a `fill=` (not `fill="none"`, which is the frame
            // stroke) is the field's own surface. Taking the first was tried and rejected: the
            // document opens with the renderer's backdrop rectangle, so the first match is white in
            // both appearances and the assertion passed with the literals restored (measured).
            let mut surface = None;
            for element in svg.split("/>") {
                let Some(open) = element.find("<rect ") else { continue };
                let element = &element[open + "<rect ".len()..];
                let Some(from) = element.find("fill=\"rgba(") else { continue };
                let from = from + "fill=\"rgba(".len();
                let Some(to) = element[from..].find(')') else { continue };
                surface = Some(format!("rgba({})", &element[from..from + to]));
            }
            surface.expect("the field fills a surface")
        };

        let dark = surface_fill(crate::theme::AppearanceMode::Dark);
        let light = surface_fill(crate::theme::AppearanceMode::Light);
        assert_ne!(dark, light, "the field surface must follow the appearance; both were {dark}");
        assert_ne!(dark, "rgba(255,255,255,1)", "the surface must not be the fixed white");
    }

    #[test]
    fn test_default_creation() {
        let kse = KeySequenceEdit::new(Rect::new(10, 20, 150, 30));

        assert_eq!(kse.kind(), WidgetKind::LineEdit);
        assert_eq!(kse.geometry(), Rect::new(10, 20, 150, 30));
        assert!(kse.key_sequence().is_empty());
        assert!(!kse.is_recording());
        assert!(kse.is_visible());
        assert!(kse.is_enabled());
        assert_eq!(kse.key_sequence().key_code(), 0);
        assert_eq!(kse.key_sequence().modifiers(), 0);
        assert!(kse.key_sequence().key_name().is_empty());
    }

    // ── 2. Setting/getting key sequence ─────────────────────────────

    #[test]
    fn test_set_and_get_key_sequence() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        let seq = KeySequence::new(0x01, 67, "C"); // Ctrl+C
        kse.set_key_sequence(seq.clone());

        assert_eq!(kse.key_sequence(), &seq);
        assert_eq!(kse.key_sequence().modifiers(), 0x01);
        assert_eq!(kse.key_sequence().key_code(), 67);
        assert_eq!(kse.key_sequence().key_name(), "C");
        assert_eq!(kse.key_sequence().to_display_string(), "Ctrl+C");
        assert!(!kse.key_sequence().is_empty());
    }

    // ── 3. Recording mode ───────────────────────────────────────────

    #[test]
    fn test_recording_mode() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        assert!(!kse.is_recording());

        kse.start_recording();
        assert!(kse.is_recording());

        kse.stop_recording();
        assert!(!kse.is_recording());
    }

    // ── 4. Clear sequence ──────────────────────────────────────────

    #[test]
    fn test_clear_sequence() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        let seq = KeySequence::new(0x04, 83, "S"); // Shift+S
        kse.set_key_sequence(seq);
        assert!(!kse.key_sequence().is_empty());

        kse.clear();
        assert!(kse.key_sequence().is_empty());
        assert_eq!(kse.key_sequence().key_code(), 0);
    }

    #[test]
    fn test_undo_redo_restores_key_sequence() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        kse.set_key_sequence(KeySequence::new(0x01, 65, "A"));
        kse.set_key_sequence(KeySequence::new(0x01, 66, "B"));

        assert!(kse.can_undo());
        assert!(kse.undo());
        assert_eq!(kse.key_sequence().key_name(), "A");
        assert!(kse.can_redo());
        assert!(kse.redo());
        assert_eq!(kse.key_sequence().key_name(), "B");
    }

    #[test]
    fn test_keyboard_shortcuts_drive_sequence_history() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        kse.set_key_sequence(KeySequence::new(0x01, 65, "A"));
        kse.set_key_sequence(KeySequence::new(0x01, 66, "B"));

        kse.handle_event(&Event::KeyPress { key: 90, modifiers: primary_modifier_event_bits() });
        assert_eq!(kse.key_sequence().key_name(), "A");
        kse.handle_event(&Event::KeyPress { key: 89, modifiers: primary_modifier_event_bits() });
        assert_eq!(kse.key_sequence().key_name(), "B");
    }

    // ── 5. Signal accessor (sequence_changed) ──────────────────────

    #[test]
    fn test_sequence_changed_signal() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        let captured = Arc::new(Mutex::new(None::<KeySequence>));
        kse.key_sequence_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<KeySequence>| {
                *captured.lock().unwrap() = Some(val.as_ref().clone());
            }
        });

        let seq = KeySequence::new(0x01, 65, "A");
        kse.set_key_sequence(seq.clone());

        let got = captured.lock().unwrap().take();
        assert_eq!(got, Some(seq));
    }

    // ── 6. editing_finished signal ──────────────────────────────────

    #[test]
    fn test_editing_finished_signal() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        let fired = Arc::new(Mutex::new(false));
        kse.editing_finished.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        kse.start_recording();
        kse.stop_recording();
        assert!(*fired.lock().unwrap());
    }

    // ── 7. Minimum/maximum sequence length ──────────────────────────

    #[test]
    fn test_sequence_length() {
        // KeySequence supports arbitrary key_names, including multi-char
        let seq1 = KeySequence::new(0, 65, "A");
        assert_eq!(seq1.key_name(), "A");

        let seq2 = KeySequence::new(0x04 | 0x01, 90, "Z");
        assert_eq!(seq2.to_display_string(), "Ctrl+Shift+Z");
        assert!(seq2.modifiers() & 0x01 != 0);
        assert!(seq2.modifiers() & 0x04 != 0);

        // Empty sequence has length 0
        let empty = KeySequence::empty();
        assert_eq!(empty.to_display_string(), "");
    }

    // ── 8. Allowed modifier keys ───────────────────────────────────

    #[test]
    fn test_allowed_modifier_keys() {
        // Test all four modifier flags in display

        let ctrl = KeySequence::new(0x01, 65, "A");
        assert_eq!(ctrl.to_display_string(), "Ctrl+A");

        let alt = KeySequence::new(0x02, 65, "A");
        assert_eq!(alt.to_display_string(), "Alt+A");

        let shift = KeySequence::new(0x04, 65, "A");
        assert_eq!(shift.to_display_string(), "Shift+A");

        let meta = KeySequence::new(0x08, 65, "A");
        assert_eq!(meta.to_display_string(), "Meta+A");

        // Combined modifiers
        let combos = KeySequence::new(0x01 | 0x02 | 0x04 | 0x08, 88, "X");
        let display = combos.to_display_string();
        assert!(display.contains("Ctrl"));
        assert!(display.contains("Shift"));
        assert!(display.contains("Alt"));
        assert!(display.contains("Meta"));
        assert!(display.contains("X"));
    }

    // ── 9. Mouse/keyboard interaction ───────────────────────────────

    #[test]
    fn test_mouse_press_starts_recording() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        assert!(!kse.is_recording());
        kse.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(kse.is_recording());
    }

    #[test]
    fn test_keypress_during_recording_captures_sequence() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        kse.start_recording();
        assert!(kse.is_recording());

        // Press 'A' (key code 65) with Ctrl modifier
        kse.handle_event(&Event::KeyPress { key: 65, modifiers: 0x02 });

        // Recording stops, sequence captured. The event convention puts Control on bit 1,
        // and the sequence's own mask puts it on 0x01.
        assert!(!kse.is_recording());
        assert_eq!(kse.key_sequence().key_code(), 65);
        assert_eq!(kse.key_sequence().modifiers(), 0x01);
        assert_eq!(kse.key_sequence().key_name(), "A");
    }

    #[test]
    fn test_escape_during_recording_clears_without_saving() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        let seq = KeySequence::new(0x04, 83, "S");
        kse.set_key_sequence(seq);

        kse.start_recording();
        // Escape key (27) during recording aborts
        kse.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });

        assert!(!kse.is_recording());
        // Original sequence should be preserved (escape doesn't clear it)
        assert_eq!(kse.key_sequence().key_name(), "S");
    }

    #[test]
    fn test_focus_lost_stops_recording() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        kse.start_recording();
        assert!(kse.is_recording());

        kse.handle_event(&Event::FocusLost);
        assert!(!kse.is_recording());
    }

    #[test]
    fn test_modifier_only_keys_ignored_during_recording() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        kse.start_recording();

        // Shift (16), Ctrl (17), Alt (18) are ignored
        kse.handle_event(&Event::KeyPress { key: 16, modifiers: 0x04 });
        assert!(kse.is_recording()); // Still recording

        kse.handle_event(&Event::KeyPress { key: 17, modifiers: 0x01 });
        assert!(kse.is_recording()); // Still recording

        kse.handle_event(&Event::KeyPress { key: 18, modifiers: 0x02 });
        assert!(kse.is_recording()); // Still recording
    }

    // ── 10. Geometry delegation ────────────────────────────────────

    #[test]
    fn test_geometry_delegation() {
        let mut kse = KeySequenceEdit::new(Rect::new(10, 20, 150, 30));

        assert_eq!(kse.geometry(), Rect::new(10, 20, 150, 30));

        kse.set_geometry(Rect::new(0, 0, 200, 32));
        assert_eq!(kse.geometry(), Rect::new(0, 0, 200, 32));
        assert_eq!(kse.geometry(), Rect::new(0, 0, 200, 32));
        assert_eq!(kse.position(), Point::new(0, 0));
        assert_eq!(kse.size(), crate::core::Size::new(200, 32));
    }

    // ── 11. Widget ID and kind ─────────────────────────────────────

    #[test]
    fn test_widget_id_and_kind() {
        let kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        assert_eq!(kse.kind(), WidgetKind::LineEdit);
        assert_ne!(kse.id(), 0);

        let kse2 = KeySequenceEdit::new(Rect::new(0, 0, 100, 20));
        assert_ne!(kse.id(), kse2.id());
    }

    // ── 12. SVG output ─────────────────────────────────────────────

    #[test]
    fn test_svg_output() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        let svg = render_to_svg(&mut kse);

        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"150\""));
        assert!(svg.contains("height=\"30\""));

        // With sequence set, it should render differently
        let mut kse2 = KeySequenceEdit::new(Rect::new(0, 0, 200, 30));
        kse2.set_key_sequence(KeySequence::new(0x01, 67, "C"));
        let svg2 = render_to_svg(&mut kse2);
        assert!(svg2.starts_with("<svg"));
    }

    // ── 13. Disabled state blocking ────────────────────────────────

    #[test]
    fn test_disabled_state_blocks_recording() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        kse.set_enabled(false);
        assert!(!kse.is_enabled());

        // Mouse press should not start recording when disabled
        kse.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(!kse.is_recording());

        // Re-enable and verify it works
        kse.set_enabled(true);
        kse.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(kse.is_recording());
    }

    // ── 14. KeySequence to_display_string formatting ───────────────

    #[test]
    fn test_key_sequence_display_formatting() {
        // No modifiers, just a key
        let seq = KeySequence::new(0, 65, "A");
        assert_eq!(seq.to_string(), "A");

        // Empty sequence
        let empty = KeySequence::empty();
        assert_eq!(empty.to_string(), "");

        // Named keys
        let enter = KeySequence::new(0, 13, "Return");
        assert_eq!(enter.to_string(), "Return");

        let f5 = KeySequence::new(0, 117, "F5");
        assert_eq!(f5.to_string(), "F5");
    }

    // ── 15. KeySequence setters ────────────────────────────────────

    #[test]
    fn test_key_sequence_mutators() {
        let mut seq = KeySequence::empty();
        assert!(seq.is_empty());

        seq.set_modifiers(0x01);
        assert_eq!(seq.modifiers(), 0x01);

        seq.set_key_code(90);
        assert_eq!(seq.key_code(), 90);

        seq.set_key_name("Z");
        assert_eq!(seq.key_name(), "Z");
        assert!(!seq.is_empty());
    }

    /// Multi-chord recording collects up to the configured limit.
    #[test]
    fn multi_chord_recording_collects_a_sequence() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        kse.set_max_chords(3);
        assert_eq!(kse.max_chords(), 3);
        kse.start_recording();

        // Ctrl+X, then Ctrl+C.
        kse.handle_event(&Event::KeyPress { key: 88, modifiers: 0x02 });
        assert!(kse.is_recording(), "a multi-chord capture keeps recording");
        kse.handle_event(&Event::KeyPress { key: 67, modifiers: 0x02 });
        assert_eq!(kse.sequence_len(), 2);
        assert_eq!(kse.chords()[0].key_name(), "X");
        assert_eq!(kse.chords()[1].key_name(), "C");
        assert_eq!(kse.key_sequence().key_name(), "C", "the last chord is the effective one");
        assert!(kse.is_recording());

        kse.handle_event(&Event::KeyPress { key: 86, modifiers: 0x02 });
        assert_eq!(kse.sequence_len(), 3);
        assert!(!kse.is_recording(), "the limit ends the capture");

        // The limit is clamped, and a redundant write is a no-op.
        kse.set_max_chords(99);
        assert_eq!(kse.max_chords(), 4);
        kse.set_max_chords(0);
        assert_eq!(kse.max_chords(), 1);
        assert_eq!(kse.sequence_len(), 1, "lowering the limit truncates the list");
    }

    /// Event modifier bits are translated into the sequence's own mask.
    #[test]
    fn event_modifiers_are_translated() {
        assert_eq!(KeySequenceEdit::modifiers_from_event(0b0001), 0x04, "shift");
        assert_eq!(KeySequenceEdit::modifiers_from_event(0b0010), 0x01, "control");
        assert_eq!(KeySequenceEdit::modifiers_from_event(0b0100), 0x02, "alt");
        assert_eq!(KeySequenceEdit::modifiers_from_event(0b1000), 0x08, "meta");
        assert_eq!(KeySequenceEdit::modifiers_from_event(0), 0);
    }

    /// Two editors that share a chord report the conflict.
    #[test]
    fn conflicting_sequences_are_reported() {
        let mut first = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        let mut second = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        first.set_chords(vec![KeySequence::new(0x01, 83, "S"), KeySequence::new(0x01, 67, "C")]);
        second.set_chords(vec![KeySequence::new(0x01, 67, "C")]);
        assert!(first.conflicts_with(&second));

        second.set_chords(vec![KeySequence::new(0x01, 86, "V")]);
        assert!(!first.conflicts_with(&second));
        assert_eq!(first.sequence_len(), 2, "set_chords keeps every chord");
    }

    #[test]
    fn sequence_changed_and_conflict_detected_are_routable() {
        use crate::widget::Widget;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut editor = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        let taken = KeySequence::new(0x01, 83, "S");
        editor.set_conflicting_sequences(vec![taken.clone()]);
        let fires = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&fires);
        editor.conflict_detected.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        editor.set_key_sequence(taken);
        assert_eq!(fires.load(Ordering::SeqCst), 1);
        assert!(editor.event_signal_dyn("sequence_changed").is_some());
        assert!(editor.event_signal_dyn("conflict_detected").is_some());
        assert!(editor.event_signal_dyn("nope").is_none());
    }

    /// A lone Meta/Super press is a modifier, not a shortcut key (ADV-06).
    #[test]
    fn a_bare_meta_press_is_not_recorded_as_a_key() {
        let mut editor = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        editor.start_recording();
        for key in [91u32, 92, 224, 16, 17, 18] {
            editor.handle_event(&Event::key_press(key, 0));
        }
        assert!(editor.key_sequence().is_empty(), "a modifier press must not become a chord");
        assert!(editor.is_recording(), "the capture is still waiting for a real key");

        editor.handle_event(&Event::key_press(83, 0b1000)); // Meta+S
        assert_eq!(editor.key_sequence().key_name(), "S");
        assert_eq!(editor.key_sequence().modifiers(), 0x08, "the meta bit travels with the chord");
    }

    /// The capability contract carries the shortcut, the recording flag and the history flags.
    #[test]
    fn the_property_contract_covers_the_shortcut_state() {
        let mut editor = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));
        assert_eq!(editor.get("key_sequence").unwrap(), CapabilityValue::String(String::new()));
        editor.set("key_sequence", CapabilityValue::String("Ctrl+Shift+S".to_string())).unwrap();
        assert_eq!(editor.key_sequence().modifiers(), 0x01 | 0x04);
        assert_eq!(editor.key_sequence().key_name(), "S");
        assert_eq!(
            editor.get("key_sequence").unwrap(),
            CapabilityValue::String("Ctrl+Shift+S".to_string())
        );
        assert_eq!(editor.get("recording").unwrap(), CapabilityValue::Bool(false));
        editor.set("recording", CapabilityValue::Bool(true)).unwrap();
        assert!(editor.is_recording());
        assert_eq!(editor.get("can_undo").unwrap(), CapabilityValue::Bool(true));
        assert_eq!(
            editor.set("can_redo", CapabilityValue::Bool(true)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }
}
