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
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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
    /// Whether this widget currently has keyboard focus.
    focused: bool,
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
            focused: false,
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

    /// Returns the raw user input text (without mask literals).
    pub fn raw_text(&self) -> &str {
        &self.raw_text
    }

    /// Sets the raw text. The text is validated against the mask, and only
    /// characters that match the mask positions are accepted.
    pub fn set_text(&mut self, text: &str) {
        let before = self.raw_text.clone();
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "mask", BASE_PROPERTY_NAMES]
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
        let geom = self.geometry();
        let is_enabled = self.base.is_enabled();
        let font = Font::simple("monospace", 13.0);

        // ── Background ──
        //
        // Resolved from the style so a theme switch reaches this control; previously the three
        // states were fixed light greys and the box stayed light in a dark theme. The state
        // ladder is preserved but *derived*: focused is the resolved colour brightened toward
        // white, disabled is faded, so the states stay distinguishable and follow the theme.
        let style = self.style().clone();
        let themed = crate::style::resolved_theme_style("masked_edit");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        // Explicit style wins, then the theme, then the original literal as a last resort so
        // an inactive theme still leaves the control with a defined appearance.
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(248, 248, 250, 200));
        let accent = style.border_color.or(themed_border);
        let bg_color = if !is_enabled {
            base_bg.blend(&Color::WHITE, 0.35)
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
        let padding = 6i32;
        let text_x = geom.x + padding;
        // The band a character may occupy. It starts below the field's top border and stops at
        // the field's bottom edge, because a character is drawn *down* from its origin: a 13 px
        // glyph placed on the vertical centre line at `geom.y + height / 2` reached
        // `geom.y + height`, which the P5 assertion reads as painting outside the control (the
        // SVG snapshot shows a `<text>` at `y = 60` in a 120 px field whose painted extent
        // reaches y = 73). Centring the *glyph box* instead of its centre line keeps the text
        // where it was while giving both branches below a bound they can be checked against.
        let inner_top = geom.y + (geom.height.saturating_sub(font.size() as u32) / 2) as i32;
        let inner_height = (geom.height as i32 - padding).max(0) as u32;

        if self.mask.is_empty() {
            let text_color =
                if !is_enabled { Color::rgba(150, 150, 150, 200) } else { Color::rgb(33, 33, 33) };
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
            context.draw_text_fitted(
                text_bounds,
                &self.raw_text,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
            return;
        }

        // Draw each segment
        let mut raw_idx = 0;
        let mut display_x = text_x;
        let char_width = 8u32;

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
                match seg {
                    MaskSegment::Literal { ch } => {
                        let lit_color = Color::rgba(160, 160, 160, 200);
                        context.draw_text(
                            Point::new(draw_x, inner_top),
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
                            Color::rgb(33, 33, 33)
                        } else {
                            Color::rgba(180, 180, 180, 200)
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
                                Color::rgb(25, 118, 210),
                            );
                            context.draw_text(
                                Point::new(draw_x, inner_top),
                                &ch.to_string(),
                                &font,
                                Color::WHITE,
                                HorizontalAlignment::Left,
                            );
                        } else {
                            context.draw_text(
                                Point::new(draw_x, inner_top),
                                &ch.to_string(),
                                &font,
                                char_color,
                                HorizontalAlignment::Left,
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
