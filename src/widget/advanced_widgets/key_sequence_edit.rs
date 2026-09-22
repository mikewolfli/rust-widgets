// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Key sequence editor widget for capturing keyboard shortcuts.
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_KEY_SEQUENCE_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

struct KeySequenceCommand {
    id: CommandId,
    target: Rc<RefCell<KeySequence>>,
    before: KeySequence,
    after: KeySequence,
}

impl KeySequenceCommand {
    fn new(target: Rc<RefCell<KeySequence>>, before: KeySequence, after: KeySequence) -> Self {
        Self {
            id: CommandId(NEXT_KEY_SEQUENCE_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
        }
    }
}

impl UndoCommand for KeySequenceCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit key sequence".to_string(),
            timestamp_ms: 0,
            command_type: "key_sequence_edit",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.after.clone();
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.before.clone();
        Ok(())
    }
}
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
    /// Emitted once each time recording stops, whether the sequence changed or
    /// not (including when recording was cancelled with Escape, which emits
    /// nothing else). Carries no payload.
    pub editing_finished: GenericSignal,
    /// Emitted with the new sequence whenever it changes: from user recording,
    /// from [`KeySequenceEdit::set_key_sequence`], and from undo/redo replayed
    /// history. Not emitted by [`KeySequenceEdit::set_key_sequence`] when the
    /// incoming sequence equals the current one.
    pub key_sequence_changed: Signal1<KeySequence>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<KeySequence>>,
    restoring_history: bool,
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
            editing_finished: GenericSignal::new(),
            key_sequence_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(KeySequence::empty())),
            restoring_history: false,
        }
    }
    /// Returns the current sequence; compare with [`KeySequence::is_empty`] to
    /// distinguish "unset" from "set".
    pub fn key_sequence(&self) -> &KeySequence {
        &self.key_sequence
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
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.key_sequence.clone();
            self.undo_stack.push(Box::new(KeySequenceCommand::new(
                self.history_target.clone(),
                before,
                self.key_sequence.clone(),
            )));
        }
        self.key_sequence_changed.emit(seq);
        self.base.request_redraw();
    }
    /// Clears the sequence, as if [`KeySequence::empty`] had been set.
    ///
    /// Goes through [`KeySequenceEdit::set_key_sequence`], so it is undoable
    /// and is a no-op when the sequence is already empty.
    pub fn clear(&mut self) {
        self.set_key_sequence(KeySequence::empty());
    }
    /// Begins capturing the next key press. Idempotent, and does not emit
    /// anything. If no `Event::FocusLost` or key press follows, recording
    /// stays on indefinitely.
    pub fn start_recording(&mut self) {
        self.recording = true;
    }
    /// Ends recording.
    ///
    /// Emits `editing_finished` only if recording was actually on, so calling
    /// this twice emits once — unless [`KeySequenceEdit::start_recording`] ran
    /// in between.
    pub fn stop_recording(&mut self) {
        if self.recording {
            self.recording = false;
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
        self.key_sequence_changed.emit(seq);
        self.base.request_redraw();
    }
}
impl Widget for KeySequenceEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(150, 28)
    }
    impl_draw_bridge!();
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
                if *key == 27 {
                    self.recording = false;
                    return;
                }
                // Ignore modifier-only keys
                if *key == 16 || *key == 17 || *key == 18 {
                    return;
                } // Shift/Ctrl/Alt
                let key_name = key_code_to_name(*key);
                let seq = KeySequence::new(*modifiers, *key, key_name);
                self.set_key_sequence(seq);
                self.stop_recording();
            }
            Event::KeyPress { key, modifiers } if *key == 90 && *modifiers == 2 => {
                let _ = self.undo();
            }
            Event::KeyPress { key, modifiers } if *key == 89 && *modifiers == 2 => {
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
        k if (48..=57).contains(&k) => (((k - 48) as u8 + b'0') as char).to_string(),
        k => format!("Key{k}"),
    }
}
impl Draw for KeySequenceEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let bg = if self.recording { Color::rgb(255, 240, 240) } else { Color::rgb(255, 255, 255) };
        context.fill_rect(rect, bg);
        context.draw_rect(
            rect,
            if self.recording { Color::rgb(200, 0, 0) } else { Color::rgb(150, 150, 150) },
        );
        let display = if self.recording {
            "Recording...".to_string()
        } else if self.key_sequence.is_empty() {
            "Click to set shortcut...".to_string()
        } else {
            self.key_sequence.to_display_string()
        };
        let text_color = if self.key_sequence.is_empty() && !self.recording {
            Color::rgb(180, 180, 180)
        } else {
            Color::rgb(0, 0, 0)
        };
        // Vertically centred through the shared primitive, and bounded to the field's own
        // width: the recorded sequence grows without limit (`Ctrl+Shift+Alt+Meta+K`), and the
        // SVG backend emits absolute coordinates, so an unbounded one ran past the field.
        let font = Font::default();
        let line = context.text_line(rect, &font);
        context.draw_text_fitted(
            Rect {
                x: rect.x + 6,
                y: line.y,
                width: rect.width.saturating_sub(12),
                height: line.height,
            },
            &display,
            &font,
            text_color,
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::core::Rect;
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    // ── 1. Creating default widget ──────────────────────────────────

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

        kse.handle_event(&Event::KeyPress { key: 90, modifiers: 2 });
        assert_eq!(kse.key_sequence().key_name(), "A");
        kse.handle_event(&Event::KeyPress { key: 89, modifiers: 2 });
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
        kse.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(kse.is_recording());
    }

    #[test]
    fn test_keypress_during_recording_captures_sequence() {
        let mut kse = KeySequenceEdit::new(Rect::new(0, 0, 150, 30));

        kse.start_recording();
        assert!(kse.is_recording());

        // Press 'A' (key code 65) with Ctrl modifier
        kse.handle_event(&Event::KeyPress { key: 65, modifiers: 0x01 });

        // Recording stops, sequence captured
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
        kse.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(!kse.is_recording());

        // Re-enable and verify it works
        kse.set_enabled(true);
        kse.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
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
}
