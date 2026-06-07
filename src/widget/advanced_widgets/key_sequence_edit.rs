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
            _ => {}
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
