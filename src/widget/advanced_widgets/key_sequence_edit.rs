//! Key sequence editor widget for capturing keyboard shortcuts.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Represents a key sequence (modifier + key name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySequence {
    pub modifiers: u32, // Bit flags: 0x01=Ctrl, 0x02=Alt, 0x04=Shift, 0x08=Meta
    pub key_code: u32,
    pub key_name: String,
}
impl KeySequence {
    pub fn new(modifiers: u32, key_code: u32, key_name: impl Into<String>) -> Self {
        Self {
            modifiers,
            key_code,
            key_name: key_name.into(),
        }
    }
    pub fn empty() -> Self {
        Self {
            modifiers: 0,
            key_code: 0,
            key_name: String::new(),
        }
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
    fn start_recording(&mut self) {
        self.recording = true;
    }
    fn stop_recording(&mut self) {
        if self.recording {
            self.recording = false;
            self.editing_finished.emit();
        }
    }
}
impl Widget for KeySequenceEdit {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
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
        k if k >= 65 && k <= 90 => (k as u8 as char).to_string(),
        k if k >= 48 && k <= 57 => (((k - 48) as u8 + b'0') as char).to_string(),
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
            Point {
                x: rect.x + 6,
                y: rect.y + (rect.height as i32 / 2),
            },
            &display,
            &Font::default(),
            text_color,
        );
    }
}
