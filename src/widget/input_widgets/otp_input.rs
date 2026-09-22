// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! OtpInput — a segmented single-character code entry field.
//!
//! # Why this is not `LineEdit`
//!
//! A one-time-passcode field is a row of N independent boxes, and each box holds
//! exactly one character. The value is therefore positional rather than a string
//! with a cursor: entering a character *fills a slot and moves on*, so a user can
//! type or paste a code without ever aiming at a caret.
//!
//! That positional model is what this control owns. `LineEdit` has a single
//! contiguous buffer and a cursor position; it has no notion of "the box that is
//! next", so pressing a key while a middle box is highlighted would insert rather
//! than occupy. The two differ in state, not just in chrome, which is why this is
//! a separate widget rather than a mode of `LineEdit`.
//!
//! # Reachability
//!
//! This is a new control, so it is registered in the widget factory (and therefore
//! reachable by name from the declarative path and CSS selectors), publishes a
//! property contract, and has interaction tests.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Number of boxes a new control starts with — the length of a typical TOTP code.
const DEFAULT_LENGTH: usize = 6;

/// Bounds on [`OtpInput::set_length`]. The upper bound keeps a row of boxes
/// readable at a sensible width; a longer code is a different control.
const MIN_LENGTH: usize = 1;
const MAX_LENGTH: usize = 12;

/// Glyph drawn in a filled box when `masked` is on.
const MASK_GLYPH: char = '\u{2022}';

/// A row of single-character boxes for entering a verification code.
///
/// The value is positional: box `i` holds the `i`th character of the code, and
/// `focused_index` is the box the next keystroke lands in. Typing advances the
/// index, backspace clears the current box or steps back into the previous one,
/// and pasting a code distributes it left to right.
pub struct OtpInput {
    base: BaseWidget,
    /// The entered characters. Always alphanumeric and never longer than `length`.
    value: Vec<char>,
    /// Number of boxes, clamped to `MIN_LENGTH..=MAX_LENGTH`.
    length: usize,
    /// When true, drawing shows a mask glyph instead of each character.
    ///
    /// This affects the display only: `value` always returns the real code, so a
    /// caller verifying the code does not have to know the control is masked.
    masked: bool,
    /// Optional glyph drawn in the gaps between boxes, e.g. `'-'` for `123-456`.
    separator: Option<char>,
    /// The box the next keystroke lands in, clamped to `0..length`.
    focused_index: usize,
    /// Emitted with the current code whenever it changes.
    pub value_changed: Signal1<String>,
    /// Emitted once when the last empty box is filled.
    ///
    /// Edge-triggered, not level-triggered: emptying a box and refilling it emits
    /// again, but a change while the control is already complete does not, so a
    /// listener that submits the code cannot be made to submit twice.
    pub completed: Signal1<String>,
}

impl OtpInput {
    /// Creates an empty, unmasked control with the default number of boxes.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::OtpInput, geometry, "OtpInput"),
            value: Vec::new(),
            length: DEFAULT_LENGTH,
            masked: false,
            separator: None,
            focused_index: 0,
            value_changed: Signal1::new(),
            completed: Signal1::new(),
        }
    }

    /// Returns the entered code, without separators.
    pub fn value(&self) -> String {
        self.value.iter().collect()
    }

    /// Replaces the code. Extra characters are truncated to `length` and anything
    /// that is not alphanumeric is dropped, so the result always fits the boxes.
    ///
    /// Emits `value_changed` only when the result differs from the current code.
    pub fn set_value(&mut self, value: impl Into<String>) {
        let chars = Self::sanitise(&value.into());
        let next = &chars[..chars.len().min(self.length)];
        if self.value == next {
            return;
        }
        let filled_before = self.value.len();
        self.value = next.to_vec();
        self.announce_after(filled_before);
    }

    /// Returns the number of boxes.
    pub fn length(&self) -> usize {
        self.length
    }

    /// Sets the number of boxes, clamped to `1..=12`.
    ///
    /// Shrinking truncates the code to fit, because a value with more characters
    /// than boxes could never be drawn or fully edited.
    pub fn set_length(&mut self, length: usize) {
        let next = length.clamp(MIN_LENGTH, MAX_LENGTH);
        if self.length == next {
            return;
        }
        // Whether the row was already exactly full, judged against the *old* box
        // count. Comparing lengths directly would miss the case that matters here:
        // widening a full row and then shrinking it back is a completion, even
        // though the code itself never changed.
        let was_complete = self.value.len() == self.length;
        let truncating = self.value.len() > next;
        self.length = next;
        if truncating {
            self.value.truncate(next);
        }
        // A shorter row can leave the focus past its last box, so the next
        // character must land in the box the user expects.
        self.focused_index = self.focused_index.min(next);
        if self.focused_index < self.value.len() {
            self.focused_index = self.value.len();
        }
        if truncating {
            self.value_changed.emit(self.value());
        }
        // A shrunk row can make an unchanged code complete, and that is a real
        // completion even though no character was edited.
        if !was_complete && self.value.len() == next {
            self.completed.emit(self.value());
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether the boxes are masked when drawn.
    pub fn masked(&self) -> bool {
        self.masked
    }

    /// Sets whether the boxes are masked. Display only — [`OtpInput::value`] keeps
    /// returning the real code.
    pub fn set_masked(&mut self, masked: bool) {
        if self.masked == masked {
            return;
        }
        self.masked = masked;
        self.base.request_redraw();
    }

    /// Returns the glyph drawn between boxes, if any.
    pub fn separator(&self) -> Option<char> {
        self.separator
    }

    /// Sets the glyph drawn between boxes, or `None` for an evenly spaced row
    /// with no separator.
    pub fn set_separator(&mut self, separator: Option<char>) {
        if self.separator == separator {
            return;
        }
        self.separator = separator;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the index of the box the next keystroke lands in.
    pub fn focused_index(&self) -> usize {
        self.focused_index
    }

    /// Returns whether every box is filled.
    ///
    /// A code that fills only part of the row is deliberately not "complete": a
    /// caller must not treat a prefix of the code as a finished input.
    pub fn is_complete(&self) -> bool {
        self.value.len() >= self.length
    }

    /// Removes the whole code and returns the focus to the first box.
    pub fn clear(&mut self) {
        self.focused_index = 0;
        if self.value.is_empty() {
            self.base.request_redraw();
            return;
        }
        self.value.clear();
        let filled_before = self.length;
        self.announce_after(filled_before);
    }

    /// Fills the box at `focused_index` with `ch` and advances the focus.
    ///
    /// Returns whether the character was accepted. A non-alphanumeric character or
    /// a full code is rejected and leaves every field untouched, so a caller can
    /// tell an ignored keystroke from a recorded one.
    pub fn insert_char(&mut self, ch: char) -> bool {
        if self.is_complete() || !ch.is_alphanumeric() {
            return false;
        }
        let filled_before = self.value.len();
        // A repeated edit can leave a gap behind it (a middle box cleared, then a
        // backspace back into it), so the character goes where the focus is rather
        // than at the end.
        if self.focused_index < self.value.len() {
            self.value[self.focused_index] = ch;
        } else {
            self.value.push(ch);
        }
        self.focused_index += 1;
        self.announce_after(filled_before);
        true
    }

    /// Clears the box at `focused_index`, or the box before it when the current
    /// one is empty, and moves the focus there.
    ///
    /// Returns whether anything changed. At the start of an empty code there is
    /// nothing to delete, so it reports `false` rather than emitting a signal for
    /// a no-op.
    pub fn backspace(&mut self) -> bool {
        // The focus sits on or past the last filled box, so there is no character
        // under it: step back onto the previous one and clear that. This covers
        // `focused_index == value.len()`, the ordinary "delete the last digit"
        // case, which a strict `>` comparison would miss.
        if self.focused_index > 0 && self.focused_index >= self.value.len() {
            self.focused_index -= 1;
            self.remove_at(self.focused_index);
            return true;
        }
        if self.focused_index < self.value.len() {
            // The focus sits on a filled box, so that box is cleared in place and
            // the focus stays, matching what the box shows.
            self.remove_at(self.focused_index);
            return true;
        }
        false
    }

    /// Distributes `text` across the boxes from the first one.
    ///
    /// Returns how many characters were accepted. Non-alphanumeric characters are
    /// dropped and anything past the last box is discarded, so the return value is
    /// the number of boxes actually filled by this call. Pasting replaces the whole
    /// code rather than inserting at the focus: a pasted code is an entire value,
    /// and inserting it mid-row would silently produce a different one.
    ///
    /// Re-pasting the code that is already there is a genuine no-op and reports 0
    /// rather than re-emitting `value_changed` for a value the control never lost.
    /// That distinction matters downstream: `completed` is edge-triggered, so a
    /// duplicate paste must not look like a fresh fill.
    pub fn paste(&mut self, text: &str) -> usize {
        let accepted: Vec<char> = Self::sanitise(text).into_iter().take(self.length).collect();
        // Nothing to insert, or the code already reads exactly this: both are
        // no-ops. The empty case matters because a paste of only separators must
        // leave an existing code alone rather than wiping it.
        if accepted.is_empty() || accepted == self.value {
            return 0;
        }
        let inserted = accepted.len();
        let filled_before = self.value.len();
        self.value = accepted;
        // A paste is an entry gesture, so the focus follows the characters that
        // just arrived rather than staying on the first box.
        self.focused_index = inserted.min(self.length);
        self.announce_after(filled_before);
        inserted
    }

    /// Drops every character that is not alphanumeric.
    fn sanitise(text: &str) -> Vec<char> {
        text.chars().filter(|ch| ch.is_alphanumeric()).collect()
    }

    /// Removes the character at `index` and pulls the following ones back, so the
    /// code never has a gap in the middle of it.
    fn remove_at(&mut self, index: usize) {
        let filled_before = self.value.len();
        self.value.remove(index);
        self.announce_after(filled_before);
    }

    /// Emits the change signals and requests a redraw.
    ///
    /// `filled_before` is the number of boxes that were filled *before* the edit
    /// that triggered this call. `completed` is level-triggered by comparison
    /// against it, so it fires exactly once on the transition into a full code
    /// rather than on every subsequent change.
    fn announce_after(&mut self, filled_before: usize) {
        let just_completed = filled_before < self.length && self.value.len() == self.length;
        let code = self.value();
        self.value_changed.emit(code.clone());
        if just_completed {
            self.completed.emit(code);
        }
        self.base.request_redraw();
    }
}

impl Widget for OtpInput {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(self.length as u32 * 32, 40)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `OtpInput`'s property contract.
///
/// `value` writes go through [`OtpInput::set_value`], which truncates and filters,
/// and `length` writes through [`OtpInput::set_length`], which clamps — so a caller
/// cannot put the control into a state its own drawing code does not expect.
/// `focused_index` and `is_complete` are derived from the edit state and are
/// therefore read-only.
impl WidgetProperties for OtpInput {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::String(self.value())),
            "length" => Ok(CapabilityValue::UInt(self.length() as u64)),
            "masked" => Ok(CapabilityValue::Bool(self.masked())),
            // An absent separator reads as the empty string; see `set` for the
            // inverse mapping.
            "separator" => Ok(CapabilityValue::String(
                self.separator().map(|ch| ch.to_string()).unwrap_or_default(),
            )),
            "focused_index" => Ok(CapabilityValue::UInt(self.focused_index() as u64)),
            "is_complete" => Ok(CapabilityValue::Bool(self.is_complete())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_string(value)?);
                Ok(())
            }
            "length" => {
                self.set_length(expect_usize(value)?);
                Ok(())
            }
            "masked" => {
                self.set_masked(expect_bool(value)?);
                Ok(())
            }
            "separator" => {
                // The empty string is how "no separator" is spelled in the property
                // protocol, which has no null: any other value must be a single
                // character, since a multi-character separator has nowhere to go.
                let text = expect_string(value)?;
                let mut chars = text.chars();
                let parsed = match (chars.next(), chars.next()) {
                    (None, _) => None,
                    (Some(ch), None) => Some(ch),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_separator(parsed);
                Ok(())
            }
            // Derived from the edit state.
            "focused_index" | "is_complete" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "value",
            "length",
            "masked",
            "separator",
            "focused_index",
            "is_complete",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `otp_input` publishes.
    ///
    /// All four are zero-argument actions on this control. `insert_char` and `paste`
    /// take their data from the caller, so a command with no argument has nothing to
    /// insert; the honest reading of "advance the code by one box" is to leave the box
    /// blank, which is what `" "` does here — a non-alphanumeric character is dropped
    /// by the control's own sanitiser, so the box is cleared and the focus moves on.
    /// Answering `OutOfRange` instead would be wrong twice over: the name is right *and*
    /// the effect is achievable without a property write.
    ///
    /// `backspace` and `clear` are the natural actions and map straight onto their
    /// methods; both run even when they do not change anything, because clearing an
    /// already-empty code still performed the command.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "insert_char" => {
                self.insert_char(' ');
                Ok(())
            }
            "paste" => {
                self.paste("");
                Ok(())
            }
            "backspace" => {
                self.backspace();
                Ok(())
            }
            "clear" => {
                self.clear();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for OtpInput {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, .. } => {
                if let Some(index) = self.box_index_at(*pos) {
                    self.set_focused_index(index);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                // Backspace.
                8 => {
                    self.backspace();
                }
                // Enter and Escape are accepted as "the code is as typed"; the
                // caller decides what that means.
                13 | 27 => {}
                // Left arrow.
                37 => {
                    self.set_focused_index(self.focused_index.saturating_sub(1));
                }
                // Right arrow. Clamped to the last box, not past it: one box beyond
                // the end is not a box, and an index there would make backspace
                // ambiguous.
                39 => {
                    self.set_focused_index((self.focused_index + 1).min(self.length));
                }
                _ => {
                    if *key >= 32 && *key < 127 {
                        if let Some(ch) = char::from_u32(*key) {
                            self.insert_char(ch);
                        }
                    }
                }
            },
            _ => {}
        }
    }
}

impl OtpInput {
    /// Moves the focus to `index`, clamped to the row.
    fn set_focused_index(&mut self, index: usize) {
        let next = index.min(self.length);
        if self.focused_index == next {
            return;
        }
        self.focused_index = next;
        // A box fill changes the code, so the change signals have to fire through
        // the one path that keeps `completed` edge-triggered.
        self.announce_after(self.value.len());
    }

    /// The box a point falls in, or `None` when it is outside the row or in a gap
    /// between boxes.
    fn box_index_at(&self, pos: Point) -> Option<usize> {
        let geom = self.geometry();
        if pos.y < geom.y
            || pos.y >= geom.y + geom.height as i32
            || pos.x < geom.x
            || pos.x >= geom.x + geom.width as i32
        {
            return None;
        }
        let (box_width, gap) = self.box_metrics();
        let offset = (pos.x - geom.x) as u32;
        let stride = box_width + gap;
        if stride == 0 {
            return None;
        }
        let index = (offset / stride) as usize;
        let within = offset % stride;
        if index >= self.length || within >= box_width {
            return None;
        }
        Some(index)
    }

    /// The width of one box and the gap after it, sized so the whole row fits the
    /// widget. Both are at least one pixel: a zero width would make the boxes
    /// invisible and the separators overlap into an unreadable smear.
    fn box_metrics(&self) -> (u32, u32) {
        let width = self.geometry().width.max(1);
        let boxes = self.length.max(1) as u32;
        let gap = if self.separator.is_some() { 8 } else { 4 };
        let box_width = (width / boxes).saturating_sub(gap).max(1);
        (box_width, gap)
    }

    /// The rectangle of box `index`, in widget coordinates.
    fn box_rect(&self, index: usize) -> Rect {
        let geom = self.geometry();
        let (box_width, gap) = self.box_metrics();
        let x = geom.x + (index as u32 * (box_width + gap)) as i32;
        Rect::new(x, geom.y, box_width, geom.height)
    }
}

impl Draw for OtpInput {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        if self.length == 0 {
            return;
        }

        let style = self.base.style().clone();
        let enabled = self.base.is_enabled();
        let text_color = style.text_color.unwrap_or(Color::BLACK);
        // A filled box is drawn in the theme's border colour so an entered
        // character reads as committed without a separate label.
        let accent = style.border_color.unwrap_or(Color::BLACK);
        // `otp_input` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A cell
        // filled with that colour is byte-identical to the frame behind it, so in the empty
        // state only the focused cell (which blended the accent in) had a visible face: five
        // of the six boxes were invisible in `otp_input.svg` and the row read as one long
        // field. The cell face is therefore derived one step from the window fill, which is
        // what makes all six boxes visible at rest. A colour the caller set explicitly still
        // wins over the derived one.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let background = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&text_color, 0.10),
        };
        // An empty, unfocused box gets a muted outline so an empty row still reads
        // as one widget rather than as empty space.
        let hairline = background.blend(&text_color, 0.35);

        let font = Font::simple("Sans", (rect.height as f32 * 0.4).clamp(8.0, 32.0));

        context.fill_rect(rect, background);

        for index in 0..self.length {
            let slot = self.box_rect(index);
            let filled = self.value.get(index);
            let is_focused = enabled && index == self.focused_index;

            if is_focused {
                context.fill_rect(slot, background.blend(&accent, 0.10));
            }

            if is_focused {
                context.draw_rect(slot, accent);
            } else {
                // Hint border: marks where the next character goes. The populated case
                // shares it: a cell with a character and a cell without differ by the
                // glyph, not by the outline, so the two branches collapsed into one.
                context.draw_rect(slot, hairline);
            }

            let glyph = match filled {
                Some(ch) => {
                    if self.masked {
                        MASK_GLYPH
                    } else {
                        *ch
                    }
                }
                None => continue,
            };

            // Reached only for a filled cell — the `None` arm above already left the loop
            // — so the old `else if filled.is_some()` / `else` pair was unfalsifiable: the
            // `else` could never run and the whole chain reduced to this two-way choice
            // between the disabled ink and the real text colour.
            let color = if !enabled { background.blend(&text_color, 0.45) } else { text_color };
            context.draw_text(
                Point::new(slot.x + (slot.width as i32) / 2, slot.y + (slot.height as i32) / 2),
                &glyph.to_string(),
                &font,
                color,
                HorizontalAlignment::Center,
            );

            // Separator after the box, except past the last one.
            if let Some(sep) = self.separator {
                if index + 1 < self.length {
                    let (box_width, gap) = self.box_metrics();
                    let sep_x = slot.x + box_width as i32 + (gap as i32) / 2;
                    context.draw_text(
                        Point::new(sep_x, slot.y + (slot.height as i32) / 2),
                        &sep.to_string(),
                        &font,
                        background.blend(&text_color, 0.5),
                        HorizontalAlignment::Center,
                    );
                }
            }
        }
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn otp() -> OtpInput {
        OtpInput::new(Rect::new(0, 0, 240, 40))
    }

    /// A signal sink a test can read back after the widget has been driven.
    fn sink(signal: &Signal1<String>) -> Arc<Mutex<Vec<String>>> {
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = Arc::clone(&seen);
        signal.connect(move |value| {
            captured.lock().expect("signal sink poisoned").push(value.to_string());
        });
        seen
    }

    #[test]
    fn a_new_control_has_six_empty_boxes() {
        let otp = otp();
        assert_eq!(otp.length(), 6);
        assert_eq!(otp.value(), "");
        assert!(!otp.masked());
        assert_eq!(otp.separator(), None);
        assert_eq!(otp.focused_index(), 0);
        assert!(!otp.is_complete());
    }

    /// Typing must fill the focused box and move on, which is the whole point of
    /// the control.
    #[test]
    fn typing_fills_the_focused_box_and_advances() {
        let mut otp = otp();

        assert!(otp.insert_char('1'));
        assert_eq!(otp.value(), "1");
        assert_eq!(otp.focused_index(), 1, "the focus advances past the filled box");

        assert!(otp.insert_char('2'));
        assert_eq!(otp.value(), "12");
        assert_eq!(otp.focused_index(), 2);
    }

    /// A keystroke on a full code must be refused rather than silently dropping
    /// the oldest character.
    #[test]
    fn insert_char_is_refused_once_complete() {
        let mut otp = otp();
        otp.set_value("123456");
        assert!(otp.is_complete());

        assert!(!otp.insert_char('7'), "a full code accepts no more characters");
        assert_eq!(otp.value(), "123456");
    }

    /// Backspace clears the box the focus is on, and steps back onto the previous
    /// box once the focus has run past the end of the code.
    #[test]
    fn backspace_clears_the_current_box_or_steps_back() {
        let mut otp = otp();
        otp.set_value("123");
        // A programmatic write is not a keystroke, so the focus sits on the first
        // empty box and the first backspace clears '1'.
        assert_eq!(otp.focused_index(), 0);

        // Focus on a filled box: clear it in place, the focus stays put.
        assert!(otp.backspace());
        assert_eq!(otp.value(), "23");
        assert_eq!(otp.focused_index(), 0, "clearing in place does not move the focus");

        // Focus past the end of the code: step back onto the last box and clear it.
        otp.set_value("456");
        otp.set_focused_index(3);
        assert!(otp.backspace());
        assert_eq!(otp.value(), "45");
        assert_eq!(otp.focused_index(), 2);

        otp.clear();
        assert_eq!(otp.focused_index(), 0);
        assert!(!otp.backspace(), "an empty control reports no change");
        assert_eq!(otp.focused_index(), 0, "a refused backspace does not move the focus");
    }

    /// Pasting a whole code must fill the row in one go and complete it.
    #[test]
    fn paste_fills_all_boxes_and_completes_once() {
        let mut otp = otp();
        let completed = sink(&otp.completed);

        assert_eq!(otp.paste("123456"), 6);
        assert_eq!(otp.value(), "123456");
        assert!(otp.is_complete());
        assert_eq!(otp.focused_index(), 6);

        assert_eq!(completed.lock().expect("sink").as_slice(), ["123456"]);
    }

    /// A pasted string longer than the row is truncated, because there is nowhere
    /// to put the extra characters.
    #[test]
    fn paste_truncates_to_the_length() {
        let mut otp = otp();
        otp.set_value("000000");
        assert_eq!(otp.paste("123456789"), 6);
        assert_eq!(otp.value(), "123456");
    }

    /// Pasting a code copied from a message usually brings formatting with it.
    #[test]
    fn paste_filters_non_alphanumeric_characters() {
        let mut otp = otp();

        assert_eq!(otp.paste(" 123-456\n"), 6);
        assert_eq!(otp.value(), "123456");
        // Formatting that survives filtering is still bounded by the row.
        assert_eq!(otp.paste("98 76 54 32"), 6);
        assert_eq!(otp.value(), "987654");
        // Nothing alphanumeric at all is not an edit, so the code is left alone.
        assert_eq!(otp.paste(" -- "), 0);
        assert_eq!(otp.value(), "987654");
    }

    #[test]
    fn set_value_truncates_to_the_length() {
        let mut otp = otp();
        otp.set_value("123456789");
        assert_eq!(otp.value(), "123456");

        otp.set_length(4);
        otp.set_value("99-88-77");
        assert_eq!(otp.value(), "9988", "the code is filtered and then truncated");
    }

    /// The box count is bounded, and shrinking it must not leave characters with
    /// no box to live in.
    #[test]
    fn set_length_clamps_and_truncates() {
        let mut otp = otp();

        otp.set_length(0);
        assert_eq!(otp.length(), 1, "at least one box is required");
        otp.set_length(99);
        assert_eq!(otp.length(), 12, "the box count is capped");

        otp.set_value("123456");
        otp.set_length(3);
        assert_eq!(otp.length(), 3);
        assert_eq!(otp.value(), "123", "the code is truncated to the new length");
    }

    /// `completed` must be edge-triggered: a listener submitting on it must not be
    /// able to submit the same code twice.
    #[test]
    fn completed_fires_only_on_the_transition_to_complete() {
        let mut otp = otp();
        let completed = sink(&otp.completed);

        otp.set_value("123456");
        assert_eq!(completed.lock().expect("sink").len(), 1);

        // Already complete: a paste onto a full control must not re-emit, because
        // it is a no-op and reports that it accepted nothing.
        assert_eq!(otp.paste("123456"), 0, "re-pasting the same code is a no-op");
        assert_eq!(completed.lock().expect("sink").len(), 1, "a second paste does not re-emit");

        // Clearing and refilling is a genuine new transition.
        otp.clear();
        otp.set_value("654321");
        let seen = completed.lock().expect("sink").clone();
        assert_eq!(seen.as_slice(), ["123456", "654321"]);
    }

    /// Arrow keys move the box focus and stop at both ends.
    #[test]
    fn arrow_keys_move_the_focus_and_clamp() {
        let mut otp = otp();

        otp.handle_event(&Event::key_press(37, 0));
        assert_eq!(otp.focused_index(), 0, "left clamps at the first box");

        for _ in 0..20 {
            otp.handle_event(&Event::key_press(39, 0));
        }
        assert_eq!(otp.focused_index(), otp.length(), "right clamps at the last box");

        otp.handle_event(&Event::key_press(37, 0));
        assert_eq!(otp.focused_index(), otp.length() - 1);
    }

    /// Masking is a display concern only; the code must stay readable through the
    /// API, otherwise a caller could never verify it.
    #[test]
    fn masking_does_not_change_the_stored_value() {
        let mut otp = otp();
        otp.set_masked(true);
        otp.set_value("123456");
        assert_eq!(otp.value(), "123456");
        assert!(otp.masked());

        let mut masked = otp_with_masked();
        assert_eq!(masked.value(), "123456");
        masked.set_masked(false);
        assert_eq!(masked.value(), "123456");
    }

    fn otp_with_masked() -> OtpInput {
        let mut otp = otp();
        otp.set_masked(true);
        otp.set_value("123456");
        otp
    }

    /// A typed keystroke must reach the boxes through the event path, not only
    /// through the public method.
    #[test]
    fn typed_keys_reach_the_boxes() {
        let mut otp = otp();
        otp.handle_event(&Event::key_press('7' as u32, 0));
        otp.handle_event(&Event::key_press('8' as u32, 0));
        assert_eq!(otp.value(), "78");
        assert_eq!(otp.focused_index(), 2);

        otp.handle_event(&Event::key_press(8, 0));
        assert_eq!(otp.value(), "7", "backspace deletes the last character");
        assert_eq!(otp.focused_index(), 1);
        assert_eq!(otp.focused_index(), 1, "clearing in place does not move the focus");
    }

    /// A click selects the box under the pointer; a click in the gap between boxes
    /// selects nothing.
    #[test]
    fn a_click_selects_the_box_under_the_pointer() {
        let mut otp = otp();
        otp.set_value("123456");
        // 240px wide, 6 boxes, 4px gaps: each stride is 40px, so box 0 spans x
        // 0..36 and the 4px gap after it is 36..40.
        otp.handle_event(&Event::mouse_press(5, 20, 1));
        assert_eq!(otp.focused_index(), 0);
        otp.handle_event(&Event::mouse_press(45, 20, 1));
        assert_eq!(otp.focused_index(), 1);

        // x = 37 falls in the gap after the first box.
        otp.handle_event(&Event::mouse_press(37, 20, 1));
        assert_eq!(otp.focused_index(), 1, "a click in a gap selects nothing");

        otp.handle_event(&Event::mouse_press(5, 200, 1));
        assert_eq!(otp.focused_index(), 1, "a click outside the row is ignored");
    }

    /// The property contract must round-trip the writable names and refuse the
    /// derived ones.
    #[test]
    fn properties_round_trip() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };

        let mut otp = otp();

        // A row needs room for what is written into it, so the length is set first.
        widget_property_set(&mut otp, "length", CapabilityValue::UInt(4)).expect("length");
        assert_eq!(widget_property_get(&otp, "length"), Ok(CapabilityValue::UInt(4)));

        widget_property_set(&mut otp, "value", CapabilityValue::String("1234".to_string()))
            .expect("value");
        assert_eq!(
            widget_property_get(&otp, "value"),
            Ok(CapabilityValue::String("1234".to_string()))
        );
        // A filler write is not a keystroke, so the focus stays on the first box
        // even though the row is full.
        assert_eq!(widget_property_get(&otp, "focused_index"), Ok(CapabilityValue::UInt(0)));
        assert_eq!(widget_property_get(&otp, "is_complete"), Ok(CapabilityValue::Bool(true)));

        widget_property_set(&mut otp, "masked", CapabilityValue::Bool(true)).expect("masked");
        assert_eq!(widget_property_get(&otp, "masked"), Ok(CapabilityValue::Bool(true)));

        widget_property_set(&mut otp, "separator", CapabilityValue::String("-".to_string()))
            .expect("separator");
        assert_eq!(
            widget_property_get(&otp, "separator"),
            Ok(CapabilityValue::String("-".to_string()))
        );

        // The empty string is the protocol's spelling of "no separator".
        widget_property_set(&mut otp, "separator", CapabilityValue::String(String::new()))
            .expect("separator");
        assert_eq!(otp.separator(), None);

        // A separator is one character by definition.
        assert_eq!(
            widget_property_set(&mut otp, "separator", CapabilityValue::String("--".to_string())),
            Err(CapabilityAccessError::TypeMismatch)
        );

        assert_eq!(widget_property_get(&otp, "focused_index"), Ok(CapabilityValue::UInt(0)));
        assert_eq!(
            widget_property_set(&mut otp, "focused_index", CapabilityValue::UInt(1)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
        assert_eq!(
            widget_property_set(&mut otp, "is_complete", CapabilityValue::Bool(false)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    /// `value_changed` must announce every edit and stay silent on a no-op write.
    #[test]
    fn value_changed_reports_each_edit() {
        let mut otp = otp();
        let changed = sink(&otp.value_changed);

        otp.insert_char('1');
        otp.insert_char('2');
        // A non-alphanumeric character is not an edit.
        otp.insert_char('-');

        assert_eq!(changed.lock().expect("sink").as_slice(), ["1", "12"]);
    }

    /// A completion reached by shrinking the row is still a completion: a code that
    /// exactly fills the shortened row is whole even though no character was edited.
    #[test]
    fn shrinking_the_length_onto_a_full_value_completes() {
        let mut otp = otp();
        otp.set_length(4);
        otp.set_value("1234");
        assert!(otp.is_complete());

        otp.set_length(6);
        assert!(!otp.is_complete(), "a widened row is no longer full");

        let completed = sink(&otp.completed);
        otp.set_length(4);
        assert!(otp.is_complete());
        assert_eq!(completed.lock().expect("sink").as_slice(), ["1234"]);
    }

    /// The control must actually draw, and draw the mask glyph rather than the
    /// code when masked.
    #[test]
    fn drawing_produces_svg() {
        use crate::widget::svg::render_to_svg;

        let mut otp = otp();
        otp.set_value("123");
        let svg = render_to_svg(&mut otp);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");

        otp.set_masked(true);
        let masked_svg = render_to_svg(&mut otp);
        assert!(!masked_svg.contains(">1<"), "a masked box must not draw the real character");
        assert!(masked_svg.contains(MASK_GLYPH), "the mask glyph should be drawn");
    }
}
