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
use crate::widget::metrics::{dimensions, ControlMetrics};
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
    ///
    /// # Why an unmasked field accepts everything
    ///
    /// With no mask there are no segment positions to validate against, and
    /// [`MaskedEdit::set_mask`] documents exactly that case: "If the mask is empty, no
    /// formatting is applied and all input is accepted". The loop below used to iterate
    /// `self.segments` unconditionally, so with no mask it ran zero times and **silently
    /// discarded the whole string** — `set_text("Sample")` left `raw_text` empty and the
    /// control painted nothing. That is why the census SVG for this control contained an
    /// empty `<text></text>`: not a rendering subtlety, but a setter that threw its
    /// argument away.
    pub fn set_text(&mut self, text: &str) {
        let before = self.raw_text.clone();
        self.raw_text = String::new();
        if self.segments.is_empty() {
            // No mask: the text is the value, verbatim.
            self.raw_text = text.to_string();
        } else {
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

    /// The field the control actually paints.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A masked field is a text field: [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is the
    /// touch-sized content floor every field shares, full width and centred in the area the
    /// caller offers. Painting `geometry()` made a 240x120 census cell a 240x120 box — a
    /// panel rather than a field — and every anchor inside it (the ink's line box, the
    /// caret) was derived from the oversized box, so a themed field's text sat on the wrong
    /// row. [`ControlMetrics::full_width_band`] is the shared derivation, and the hit test
    /// below reads it too, so the clickable area is exactly the painted one.
    fn field_rect(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
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
        // The **field**, not the control's rectangle.
        //
        // A masked field is a text field, and every other field in this crate is
        // `TEXT_FIELD_MIN_HEIGHT` tall, full width, centred in the area it is given.
        // Painting `geometry()` made a 240x120 census cell a 240x120 box, and every
        // measurement below — the fill, the border, the ink's line box and the caret —
        // inherited that. `ControlMetrics::full_width_band` is the shared derivation, and
        // it is also what the hit test uses, so the clickable area is the visible one.
        let geom = self.field_rect();
        let is_enabled = self.base.is_enabled();
        let font = Font::simple("monospace", 13.0);

        // ── Background ──
        //
        // Resolved from the style so a theme switch reaches this control; previously the three
        // states were fixed light greys and the box stayed light in a dark theme. The state
        // ladder is preserved but *derived*: focused is the resolved colour brightened toward
        // white, disabled is faded, so the states stay distinguishable and follow the theme.
        //
        // This covers the field's **fill and border only**. The five pieces of *ink* below —
        // body text, mask literals, placeholders, the caret and the glyph drawn on the caret —
        // were still fixed literals until they were derived from the same resolved pair, so a
        // themed field carried unthemed text: `rgb(33,33,33)` on the dark theme's `rgb(69,69,69)`
        // field is ~1.35:1, which is present in the SVG and unreadable to a person.
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

        // ── Ink ──
        //
        // Every piece of text ink below resolves `style.text_color` (the theme writes the
        // resolved text colour for an `Input`-role control) and then pushes that ink away from
        // the field's own fill until it is legible on it. `style.text_color` is the right source
        // because the field's interior is what the text sits on — that is exactly Qt's
        // `QPalette::Text` over `QPalette::Base` relationship — and `legible_on` is what makes
        // the result a property of the resolved pair rather than of a guessed lightness. Three
        // states are then derived from that one ink: enabled is the ink itself, disabled is
        // faded toward the fill, and a *placeholder* is a placeholder rather than a value, so it
        // is a further step toward the fill. The caret is the control's *emphasis* colour, so it
        // reads the theme's primary token rather than the text ink, and the glyph drawn inside
        // it is that colour's contrast partner.
        //
        // WCAG AA for normal text. Mask literals and placeholders are deliberately below it —
        // they are not values the user is reading — but not *arbitrarily* below: both are a fixed
        // fraction of the distance to the fill, so on any appearance they stay visibly weaker
        // than the entered text without ever collapsing into it.
        const INK_MIN_RATIO: f32 = 4.5;
        let ink = style
            .text_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
            .unwrap_or_else(|| bg_color.contrast_color())
            .legible_on(bg_color, INK_MIN_RATIO);
        let disabled_ink = ink.blend(&bg_color, 0.45);
        let mask_ink = ink.blend(&bg_color, 0.55);
        let placeholder_ink = ink.blend(&bg_color, 0.70);
        // The caret is chrome in the semantic sense — it is the control saying "the cursor is
        // here" — so it takes the accent the theme assigns emphasis (`primary`) rather than the
        // text ink, and pushes it clear of the field. The glyph inside it is read off the caret
        // itself, so a light caret cannot carry a light glyph.
        let caret_color = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .or_else(|| {
                crate::style::resolved_theme_style("button").and_then(|b| b.background_color)
            })
            .unwrap_or_else(|| bg_color.contrast_color())
            .legible_on(bg_color, INK_MIN_RATIO);
        let caret_ink = caret_color.contrast_color();

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
        // The field's horizontal content inset, from the shared table rather than a local
        // `6`: a masked field and every other field inset their content by the same amount,
        // so the value belongs in the table the others read.
        let padding = dimensions::TEXT_FIELD_PADDING_H as i32;
        let text_x = geom.x + padding;
        // The band a character may occupy. It starts below the field's top border and stops at
        // the field's bottom edge, because a character is drawn *down* from its origin: a 13 px
        // glyph placed on the vertical centre line at `geom.y + height / 2` reached
        // `geom.y + height`, which the P5 assertion reads as painting outside the control (the
        // SVG snapshot shows a `<text>` at `y = 60` in a 120 px field whose painted extent
        // reaches y = 73). Centring the *glyph box* instead of its centre line keeps the text
        // where it was while giving both branches below a bound they can be checked against.
        //
        // `context.text_line` is what places the box: it returns the field's own line box, so
        // the ink is vertically centred in the *field* rather than beginning on the point the
        // field happens to be centred on.
        let line = context.text_line(geom, &font);
        let inner_top = line.y;
        let inner_height = line.height;

        if self.mask.is_empty() {
            let text_color = if !is_enabled { disabled_ink } else { ink };
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
            // An empty value emits no text element at all. `draw_text_fitted` on an empty string
            // still reaches the backend, which emitted `<text ...></text>` — an element with no
            // content, which is neither a glyph nor a space and which nothing can consume. The
            // masked branch below already skips empty segments, so guarding here keeps the two
            // halves of the control consistent rather than leaving one of them emitting nothing
            // that looks like something.
            if !self.raw_text.is_empty() {
                context.draw_text_fitted(
                    text_bounds,
                    &self.raw_text,
                    &font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
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
                        context.draw_text(
                            Point::new(draw_x, inner_top),
                            &ch.to_string(),
                            &font,
                            mask_ink,
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
                            disabled_ink
                        } else if has_input {
                            ink
                        } else {
                            placeholder_ink
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
                                caret_color,
                            );
                            context.draw_text(
                                Point::new(draw_x, inner_top),
                                &ch.to_string(),
                                &font,
                                caret_ink,
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
            Event::FocusGained { .. } => {
                self.focused = true;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.base.request_redraw();
            }
            // A press focuses the field only when it lands on the **painted band**.
            //
            // Testing the control's rectangle is what let a user focus this field by
            // clicking the empty space below it: the field is a 48 px band centred in the
            // area the caller gave, so in a 120 px cell the bottom 36 px of the rectangle
            // are window background, not the control. Hit-testing the drawn box keeps the
            // clickable area equal to the visible one.
            Event::MousePress { pos, .. } => {
                if self.field_rect().contains_point(*pos) {
                    self.focused = true;
                    self.base.request_redraw();
                }
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

// These tests drive the **theme**, which only exists in a build with a device profile
// (see `crate::lib`: `pub mod theme` is gated on `device_profile`). Without this gate the
// `mini` and `embedded` profiles fail to compile their test targets, because the test code
// names a module that those builds compile out — the production code is profile-clean and
// only the fixture was not.
#[cfg(all(test, full_widgets))]
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

    /// The field is a full-width band one text-field height tall, centred in the control.
    ///
    /// The control painted its whole rectangle, so a 240x120 census cell drew a 240x120
    /// box — a panel rather than a field — and every anchor inside it was derived from the
    /// oversized box. This pins the shared `full_width_band` derivation, including the
    /// clamp for a control smaller than the field.
    #[test]
    fn the_field_is_a_text_field_height_in_any_rectangle() {
        for height in [48u32, 120, 300] {
            let me = MaskedEdit::new(Rect::new(0, 0, 240, height));
            let field = me.field_rect();
            assert_eq!(
                field.height,
                dimensions::TEXT_FIELD_MIN_HEIGHT,
                "at control height {height}"
            );
            assert_eq!(field.width, 240, "the field spans the control's width");
            assert_eq!(field.y, (height - field.height) as i32 / 2, "at control height {height}");
        }

        let short = MaskedEdit::new(Rect::new(0, 0, 240, 20));
        assert_eq!(short.field_rect().height, 20, "a short control clamps the field");
    }

    /// Hit-testing follows the ink: a press below the field does not focus it.
    ///
    /// With the field centred in a 120 px cell, a press inside the control's rectangle but
    /// well below the drawn band belongs to the window background. The control must not
    /// claim it. The flag is read directly because this control publishes no focus accessor;
    /// the assertion is on the state the handler sets, which is what the ring and the caret
    /// are drawn from.
    #[test]
    fn a_press_outside_the_drawn_band_does_not_focus_the_field() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
        let field = me.field_rect();

        me.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 / 2),
            button: 1,
        });
        assert!(me.focused, "a press on the drawn field focuses it");

        me.focused = false;
        me.handle_event(&Event::MousePress {
            pos: Point::new(field.x + 10, field.y + field.height as i32 + 40),
            button: 1,
        });
        assert!(!me.focused, "a press below the drawn field must not focus it");
    }

    /// One ink box per text `<path>` in the document, as `(left, top, right, bottom)`.
    ///
    /// # Why per element, not per merged run
    ///
    /// Text leaves the backend as `font8x8` **glyph geometry** — one axis-aligned subpath per
    /// set bitmap bit — so the string is not in the document in any form and a test has to
    /// locate ink by *where* it is rather than by what it says. This control draws one
    /// character per `draw_text`, so each element is exactly one glyph's ink; the boundary is
    /// therefore the element, which is what makes "every glyph" expressible.
    ///
    /// The union of a `<path>`'s subpaths is its ink. Subpaths are deliberately **not**
    /// deduplicated: a glyph box wider than the 8 bitmap columns maps two columns onto the same
    /// pixel (`gx * w / 8`), so the same rectangle is emitted twice — exactly as the rasteriser
    /// fills that pixel twice.
    fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        let mut boxes = Vec::new();
        for line in svg.lines() {
            let Some(path_at) = line.find("<path ") else { continue };
            let Some(d_at) = line[path_at..].find("d=\"") else { continue };
            let start = path_at + d_at + 3;
            let Some(end) = line[start..].find('"') else { continue };
            let mut bounds: Option<(i32, i32, i32, i32)> = None;
            for subpath in line[start..start + end].split('M').skip(1) {
                let numbers: Vec<i32> = subpath
                    .split(|c: char| !c.is_ascii_digit() && c != '-')
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse().ok())
                    .collect();
                if numbers.len() < 4 {
                    continue;
                }
                let (x, y, w, h) = (numbers[0], numbers[1], numbers[2], numbers[3]);
                let bit = (x, y, x + w, y + h);
                bounds = Some(match bounds {
                    None => bit,
                    Some((l, t, r, b)) => (l.min(bit.0), t.min(bit.1), r.max(bit.2), b.max(bit.3)),
                });
            }
            if let Some(union) = bounds {
                boxes.push(union);
            }
        }
        boxes
    }

    /// The ink is vertically centred inside the **field**, not below its middle line.
    ///
    /// The old anchor was `geom.y + (geom.height - font.size) / 2`, derived from a
    /// rectangle that was already too tall. Drawing down from that origin put a 13 px glyph
    /// near the bottom of the field. The line box is now taken from the field itself.
    ///
    /// The check is on the emitted **ink**, not on a `<text y>` attribute: the backend now
    /// paints the `font8x8` rectangles the rasteriser fills, so the document holds a picture of
    /// the run rather than the run. Measuring the ink is also the stronger statement — a glyph
    /// drawn a line low with a correct attribute would have passed the old form.
    #[test]
    fn the_ink_is_vertically_centred_in_the_field() {
        let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
        me.set_mask("000-0000");
        me.set_text("5551234");
        let svg = render_to_svg(&mut me);
        let field = me.field_rect();

        let runs = text_run_boxes(&svg);
        assert!(!runs.is_empty(), "a filled masked field draws glyphs: {svg}");
        // Every character is placed on the field's own line box, so every glyph's ink has to sit
        // inside that box — and the topmost ink has to *be* the box's top edge, because a digit's
        // bitmap lights its first row. The box top is `field.y + (height - line) / 2`, derived
        // from the measured line height rather than from a copied literal so this stays a
        // statement about the layout. The defect was an origin below the field's middle line,
        // which pushes the whole box down by at least a line and fails both halves.
        let mut font_probe = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
        let line_h = crate::render::RenderContext::new(&mut font_probe)
            .measure_text("M", &Font::simple("monospace", 13.0))
            .height as i32;
        let expected_top = field.y + (field.height as i32 - line_h) / 2;
        let mut highest = i32::MAX;
        for (left, top, right, bottom) in runs {
            assert!(right > left, "a glyph painted ink: {left}..{right}");
            assert!(top >= expected_top, "glyph ink {top}..{bottom} starts on or below the box");
            assert!(
                bottom <= expected_top + line_h,
                "glyph ink {top}..{bottom} stays inside the one-line box ending at {}",
                expected_top + line_h
            );
            assert!(top >= field.y, "ink starts inside the field: top={top}, field={field:?}");
            assert!(
                top < field.y + field.height as i32,
                "ink starts above the field's bottom edge: top={top}, field={field:?}"
            );
            highest = highest.min(top);
        }
        assert_eq!(highest, expected_top, "the line box's top edge is where the ink begins");
    }

    /// The five chrome colours were fixed literals, so a themed field carried unthemed text:
    /// `rgb(33,33,33)` on the dark theme's `rgb(69,69,69)` field is ~1.35:1.
    ///
    /// The expectation is read off the rendered drawing rather than a copied-out colour: the body
    /// ink must move with the appearance, and it must actually be legible on the field the control
    /// painted behind it. Legibility is asserted with the crate's own `contrast_ratio`, so the
    /// test states the property the fix is for rather than the numbers it happened to produce.
    ///
    /// # Why the guard is held for the whole body, and the theme restored at the end
    ///
    /// The theme is process-wide and the guard is what serialises tests that switch it. A test
    /// that takes it once per render releases it between the two appearances, and another test
    /// running at that moment then switches the active theme underneath this one — the "dark" SVG
    /// comes back drawn in the light palette and the comparison below is meaningless. Holding it
    /// once around the whole body is what makes both renders observe the appearances they asked
    /// for.
    ///
    /// This test leaves the process on the **dark** theme, and it restores the light default
    /// before releasing the guard rather than leaving the choice to whichever test runs next: a
    /// theme left behind by a new test is exactly how an unrelated control's rendering assertion
    /// starts failing for a reason that has nothing to do with it.
    #[test]
    fn masked_edit_body_ink_is_legible_on_its_own_field() {
        let _guard = crate::theme::theme_test_guard();
        // Whatever this test does, the process goes back to the light default on the way out.
        struct RestoreOnDrop;
        impl Drop for RestoreOnDrop {
            fn drop(&mut self) {
                crate::theme::global_theme_manager()
                    .set_appearance(crate::theme::AppearanceMode::Light);
            }
        }
        let _restore = RestoreOnDrop;

        fn render(appearance: crate::theme::AppearanceMode, text: &str) -> String {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
            me.set_text(text);
            render_to_svg(&mut me)
        }

        fn fills(svg: &str) -> Vec<String> {
            svg.lines()
                .filter_map(|l| {
                    l.split("fill=\"")
                        .nth(1)
                        .and_then(|rest| rest.split('"').next())
                        .map(|s| s.to_string())
                })
                .collect()
        }

        /// Parses the `rgba(r,g,b,a)` spelling the SVG backend emits.
        fn parse_rgba(value: &str) -> Option<Color> {
            let inner = value.strip_prefix("rgba(")?.strip_suffix(')')?;
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() < 3 {
                return None;
            }
            let channel = |s: &str| s.trim().parse::<f32>().ok().map(|v| v.round() as u8);
            Some(Color::rgba(
                channel(parts[0])?,
                channel(parts[1])?,
                channel(parts[2])?,
                parts
                    .get(3)
                    .and_then(|a| a.trim().parse::<f32>().ok())
                    .map_or(255, |a| (a * 255.0).round() as u8),
            ))
        }

        let dark = render(crate::theme::AppearanceMode::Dark, "Sample");
        let light = render(crate::theme::AppearanceMode::Light, "Sample");
        assert_ne!(dark, light, "the field's ink must respond to the appearance");

        // The text path's fill is the ink; the second `<rect>` fill is the field.
        //
        // Text is no longer a `<text>` element: the backend emits the same `font8x8` rectangles
        // the rasteriser fills, so the ink's colour is now the fill of a `<path>`. Only text is a
        // path in this backend — the field, the border and the caret are `<rect>`s — so the first
        // path carrying a fill is the body run.
        let ink = |svg: &str| -> Color {
            svg.lines()
                .find(|l| l.contains("<path ") && l.contains("fill="))
                .and_then(|l| l.split("fill=\"").nth(1))
                .and_then(|rest| rest.split('"').next())
                .and_then(parse_rgba)
                .expect("the body text must be drawn with a parseable fill")
        };
        let field = |svg: &str| -> Color {
            fills(svg)
                .iter()
                .skip(1)
                .find_map(|f| parse_rgba(f))
                .expect("the field background must be a parseable fill")
        };

        for (svg, name) in [(&dark, "dark"), (&light, "light")] {
            let ratio = ink(svg).contrast_ratio(field(svg));
            assert!(
                ratio >= 4.5,
                "{name}: body ink {:?} on field {:?} is only {ratio:.2}:1",
                ink(svg),
                field(svg)
            );
        }

        // A placeholder is deliberately weaker than an entered value, but never invisible.
        let with_mask = {
            crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);
            let mut me = MaskedEdit::new(Rect::new(0, 0, 240, 120));
            me.set_mask("000-0000");
            render_to_svg(&mut me)
        };
        // A placeholder must still be distinguishable from the field it sits on.
        //
        // The mask characters are separate glyph runs, so each is its own `<path>`; the ink is
        // read from the path's fill rather than from a `<text>` element, which no longer exists.
        let placeholders: Vec<Color> = with_mask
            .lines()
            .filter(|l| l.contains("<path "))
            .filter_map(|l| l.split("fill=\"").nth(1))
            .filter_map(|rest| rest.split('"').next())
            .filter_map(parse_rgba)
            .collect();
        assert!(!placeholders.is_empty(), "a masked field must render its mask characters");
        for colour in placeholders {
            let field_colour =
                fills(&with_mask).iter().skip(1).find_map(|f| parse_rgba(f)).expect("field fill");
            assert!(
                colour.contrast_ratio(field_colour) >= 1.5,
                "a mask character at {colour:?} is indistinguishable from its field"
            );
        }
    }
}
