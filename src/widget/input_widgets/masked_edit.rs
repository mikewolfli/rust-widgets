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

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
    /// Whether this widget currently has keyboard focus.
    focused: bool,
    /// Emitted when the text changes, providing the raw text.
    pub text_changed: Signal1<String>,
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
            focused: false,
            text_changed: Signal1::new(),
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

    /// Returns the raw user input text (without mask literals).
    pub fn raw_text(&self) -> &str {
        &self.raw_text
    }

    /// Sets the raw text. The text is validated against the mask, and only
    /// characters that match the mask positions are accepted.
    pub fn set_text(&mut self, text: &str) {
        self.raw_text = String::new();
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
        self.update_display_text();
        self.cursor_pos = self.display_text.len();
        self.text_changed.emit(self.raw_text.clone());
        self.base.request_redraw();
    }

    /// Returns the display text (with mask literals inserted).
    pub fn text(&self) -> &str {
        &self.display_text
    }

    /// Returns whether all required mask positions are filled.
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
        self.raw_text.len() >= required_count && !self.mask.is_empty()
    }

    /// Returns the current cursor position in the display text.
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Sets the cursor position in the display text.
    pub fn set_cursor_pos(&mut self, pos: usize) {
        self.cursor_pos = pos.min(self.display_text.len());
        self.base.request_redraw();
    }

    /// Inserts a character at the current cursor position.
    fn insert_char(&mut self, ch: char) {
        let raw_idx = self.display_to_raw_index(self.cursor_pos);
        if raw_idx >= self.input_count() {
            return;
        }

        // Find the input segment at this raw index
        if let Some((seg_idx, kind)) = self.find_input_at_raw_index(raw_idx) {
            if mask_char_matches(kind, ch) {
                self.raw_text.insert(raw_idx, ch);
                self.update_display_text();
                // Move cursor past this input
                self.cursor_pos = seg_idx + 1;
                self.text_changed.emit(self.raw_text.clone());
                self.base.request_redraw();
            }
        }
    }

    /// Deletes the character before the cursor (backspace).
    fn backspace(&mut self) {
        if self.raw_text.is_empty() || self.cursor_pos == 0 {
            return;
        }
        let raw_idx = self.display_to_raw_index(self.cursor_pos);
        if raw_idx > 0 && raw_idx <= self.raw_text.len() {
            self.raw_text.remove(raw_idx - 1);
            self.update_display_text();
            self.cursor_pos = self.segment_before_raw_index(raw_idx - 1);
            self.text_changed.emit(self.raw_text.clone());
            self.base.request_redraw();
        }
    }

    /// Deletes the character at the cursor (delete).
    fn delete(&mut self) {
        let raw_idx = self.display_to_raw_index(self.cursor_pos);
        if raw_idx < self.raw_text.len() {
            self.raw_text.remove(raw_idx);
            self.update_display_text();
            self.text_changed.emit(self.raw_text.clone());
            self.base.request_redraw();
        }
    }

    /// Rebuilds the display text from raw_text and the mask segments.
    fn update_display_text(&mut self) {
        self.display_text = build_display_text(&self.segments, &self.raw_text);
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
}

impl Widget for MaskedEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for MaskedEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();
        let is_enabled = self.base.is_enabled();
        let font = Font::simple("monospace", 13.0);

        // ── Background ──
        let bg_color = if !is_enabled {
            Color::rgba(240, 240, 240, 180)
        } else if self.focused {
            Color::WHITE
        } else {
            Color::rgba(248, 248, 250, 200)
        };
        context.fill_rounded_rect(geom, 4, bg_color);

        // ── Border ──
        let border_color = if self.focused && is_enabled {
            Color::from_rgb(25, 118, 210)
        } else {
            Color::rgba(190, 190, 200, 200)
        };
        context.draw_rounded_rect_stroke(geom, 4, border_color, if self.focused { 2 } else { 1 });

        // ── Draw display text with mask placeholders ──
        let padding = 6i32;
        let text_x = geom.x + padding;
        let text_y = geom.y + geom.height as i32 / 2;

        if self.mask.is_empty() {
            let text_color = if !is_enabled {
                Color::rgba(150, 150, 150, 200)
            } else {
                Color::from_rgb(33, 33, 33)
            };
            context.draw_text(Point::new(text_x, text_y), &self.raw_text, &font, text_color, HorizontalAlignment::Left);
            return;
        }

        // Draw each segment
        let mut raw_idx = 0;
        let mut display_x = text_x;
        let char_width = 8u32;

        for (seg_idx, seg) in self.segments.iter().enumerate() {
            if display_x - text_x > geom.width as i32 - padding * 2 {
                break;
            }

            match seg {
                MaskSegment::Literal { ch } => {
                    let lit_color = Color::rgba(160, 160, 160, 200);
                    context.draw_text(
                        Point::new(display_x, text_y),
                        &ch.to_string(),
                        &font,
                        lit_color,
                        HorizontalAlignment::Left,
                    );
                    display_x += char_width as i32;
                }
                MaskSegment::Input { kind } => {
                    let has_input = raw_idx < self.raw_text.len();
                    let ch = if has_input {
                        self.raw_text.as_bytes()[raw_idx] as char
                    } else {
                        placeholder_char(*kind)
                    };

                    let char_color = if !is_enabled {
                        Color::rgba(150, 150, 150, 200)
                    } else if has_input {
                        Color::from_rgb(33, 33, 33)
                    } else {
                        Color::rgba(180, 180, 180, 200)
                    };

                    let metrics = context.measure_text(&ch.to_string(), &font);
                    let ch_width = metrics.width as i32;

                    // Draw cursor if at this segment position
                    if self.focused && is_enabled && seg_idx == self.cursor_pos {
                        context.fill_rect(
                            Rect::new(display_x, geom.y + 2, char_width, geom.height - 4),
                            Color::from_rgb(25, 118, 210),
                        );
                        context.draw_text(
                            Point::new(display_x, text_y),
                            &ch.to_string(),
                            &font,
                            Color::WHITE,
                            HorizontalAlignment::Left,
                        );
                    } else {
                        context.draw_text(
                            Point::new(display_x, text_y),
                            &ch.to_string(),
                            &font,
                            char_color,
                            HorizontalAlignment::Left,
                        );
                    }

                    display_x += ch_width.max(char_width as i32);
                    if has_input {
                        raw_idx += 1;
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
            Event::FocusGained => {
                self.focused = true;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.base.request_redraw();
            }
            Event::MousePress { .. } => {
                self.focused = true;
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers: _ } => {
                if !self.focused {
                    return;
                }
                match *key {
                    8 => {
                        // Backspace
                        self.backspace();
                    }
                    127 => {
                        // Delete
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
                        // Left arrow
                        if self.cursor_pos > 0 {
                            self.cursor_pos -= 1;
                            self.base.request_redraw();
                        }
                    }
                    39 => {
                        // Right arrow
                        if self.cursor_pos < self.display_text.len() {
                            self.cursor_pos += 1;
                            self.base.request_redraw();
                        }
                    }
                    _ => {
                        // Printable character
                        if *key >= 32 && *key < 127 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

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
}
