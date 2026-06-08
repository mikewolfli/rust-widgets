//! Key sequence editor widget for capturing keyboard shortcuts.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Represents a key sequence (modifier + key name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySequence {
    modifiers: u32, // Bit flags: 0x01=Ctrl, 0x02=Alt, 0x04=Shift, 0x08=Meta
    key_code: u32,
    key_name: String,
}
impl KeySequence {
    pub fn new(modifiers: u32, key_code: u32, key_name: impl Into<String>) -> Self {
        Self { modifiers, key_code, key_name: key_name.into() }
    }
    pub fn empty() -> Self {
        Self { modifiers: 0, key_code: 0, key_name: String::new() }
    }
    pub fn modifiers(&self) -> u32 {
        self.modifiers
    }
    pub fn key_code(&self) -> u32 {
        self.key_code
    }
    pub fn key_name(&self) -> &str {
        &self.key_name
    }
    pub fn set_modifiers(&mut self, modifiers: u32) {
        self.modifiers = modifiers;
    }
    pub fn set_key_code(&mut self, key_code: u32) {
        self.key_code = key_code;
    }
    pub fn set_key_name(&mut self, key_name: impl Into<String>) {
        self.key_name = key_name.into();
    }
    pub fn is_empty(&self) -> bool {
        self.key_code == 0
    }
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
pub struct KeySequenceEdit {
    base: BaseWidget,
    key_sequence: KeySequence,
    recording: bool,
    pub editing_finished: GenericSignal,
    pub key_sequence_changed: Signal1<KeySequence>,
}
impl KeySequenceEdit {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "KeySequenceEdit"),
            key_sequence: KeySequence::empty(),
            recording: false,
            editing_finished: GenericSignal::new(),
            key_sequence_changed: Signal1::new(),
        }
    }
    pub fn key_sequence(&self) -> &KeySequence {
        &self.key_sequence
    }
    pub fn is_recording(&self) -> bool {
        self.recording
    }
    pub fn set_key_sequence(&mut self, seq: KeySequence) {
        self.key_sequence = seq.clone();
        self.key_sequence_changed.emit(seq);
    }
    pub fn clear(&mut self) {
        self.set_key_sequence(KeySequence::empty());
    }
    pub fn start_recording(&mut self) {
        self.recording = true;
    }
    pub fn stop_recording(&mut self) {
        if self.recording {
            self.recording = false;
            self.editing_finished.emit();
        }
    }
}
impl Widget for KeySequenceEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
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
                self.key_sequence = seq.clone();
                self.key_sequence_changed.emit(seq);
                self.stop_recording();
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
        k => format!("Key{}", k),
    }
}
impl Draw for KeySequenceEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let bg = if self.recording {
            Color::from_rgb(255, 240, 240)
        } else {
            Color::from_rgb(255, 255, 255)
        };
        context.fill_rect(rect, bg);
        context.draw_rect(
            rect,
            if self.recording {
                Color::from_rgb(200, 0, 0)
            } else {
                Color::from_rgb(150, 150, 150)
            },
        );
        let display = if self.recording {
            "Recording...".to_string()
        } else if self.key_sequence.is_empty() {
            "Click to set shortcut...".to_string()
        } else {
            self.key_sequence.to_display_string()
        };
        let text_color = if self.key_sequence.is_empty() && !self.recording {
            Color::from_rgb(180, 180, 180)
        } else {
            Color::from_rgb(0, 0, 0)
        };
        context.draw_text(
            Point { x: rect.x + 6, y: rect.y + (rect.height as i32 / 2) },
            &display,
            &Font::default(),
            text_color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(kse.rect(), Rect::new(0, 0, 200, 32));
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
