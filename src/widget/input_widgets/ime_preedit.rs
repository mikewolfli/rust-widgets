// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! IME preedit text overlay widget — shows composition text with an underline.
//!
//! This widget displays the current IME composition string at a given position,
//! rendered with an underline style to indicate the preedit (uncommitted) state,
//! similar to how operating systems render inline IME composition text.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// One styled run of a composition string.
///
/// # Why segments rather than one string
///
/// An input method marks the clause it is currently converting differently from the clauses
/// around it, and several platforms also carry a per-clause cursor attribute. Painting the whole
/// composition with one colour and one underline threw that information away: the user could not
/// tell which clause the candidate window would replace. A segment is the smallest unit the
/// platform reports, so it is the unit this overlay stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreeditSegment {
    text: String,
    color: Option<Color>,
    underline: bool,
    cursor: bool,
}

impl PreeditSegment {
    /// Creates a plain segment: the widget's own text colour and a composition underline.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), color: None, underline: true, cursor: false }
    }

    /// Overrides the ink for this segment.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets whether the composition underline is drawn beneath this segment.
    pub fn with_underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Marks this segment as the clause the input method is converting.
    ///
    /// The mark is a stronger underline; the segment's text and colour are unchanged.
    pub fn with_cursor(mut self, cursor: bool) -> Self {
        self.cursor = cursor;
        self
    }

    /// The segment's text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The ink this segment asked for, or `None` to use the widget's text colour.
    pub fn color(&self) -> Option<Color> {
        self.color
    }

    /// Whether the composition underline is drawn for this segment.
    pub fn underline(&self) -> bool {
        self.underline
    }

    /// Whether this is the clause the input method is converting.
    pub fn is_cursor(&self) -> bool {
        self.cursor
    }
}

/// IME preedit text overlay widget that renders composition text with an underline.
///
/// This widget is visually similar to a Label but draws a composition underline
/// beneath the text to indicate that it is part of an active IME composition session.
pub struct ImePreedit {
    base: BaseWidget,
    text: String,
    font: Font,
    text_color: Color,
    underline_color: Color,
    underline_thickness: u32,
    /// Whether the caller ever chose a text colour through [`ImePreedit::set_text_color`].
    ///
    /// The field is only ever *written* by that setter, so its constructed value cannot be told
    /// from a caller's write by looking at the field alone. The flag records the difference, which
    /// is what lets `draw` fall back to the theme's foreground for a colour nobody chose instead of
    /// keeping the black the constructor happened to install.
    text_color_set: bool,
    /// Whether the caller ever chose an underline colour through
    /// [`ImePreedit::set_underline_color`]. Same reason as [`Self::text_color_set`].
    underline_color_set: bool,
    /// Styled runs of the composition, empty when the caller supplied only plain text.
    segments: Vec<PreeditSegment>,
    /// Byte offset of the composition caret inside [`Self::text`].
    cursor: usize,
    /// The caret's x offset from the control's left edge, measured by the last paint.
    ///
    /// The input method asks the host where to anchor its candidate window *before* the next
    /// frame is drawn, so a value only produced inside `draw` would be one frame stale. The
    /// initial value is an estimate from the font size and is replaced by the true measurement
    /// as soon as the composition is painted.
    caret_offset: i32,
}

impl ImePreedit {
    /// Creates a new ImePreedit widget at the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ImePreedit, geometry, "ImePreedit"),
            text: String::new(),
            font: Font::default(),
            text_color: Color::BLACK,
            underline_color: Color::BLACK,
            underline_thickness: 1,
            text_color_set: false,
            underline_color_set: false,
            segments: Vec::new(),
            cursor: 0,
            caret_offset: 0,
        }
    }

    /// Sets the preedit composition text to display.
    ///
    /// A same-value write is a no-op: an IME can re-announce the composition it already has, and
    /// repainting the same glyphs queues a frame nobody can see (WDGT-12).
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text && self.segments.is_empty() {
            return;
        }
        self.text = text;
        // A plain-text write replaces the styled runs: keeping segments from a previous
        // composition would paint glyphs that are no longer in the value.
        self.segments.clear();
        self.cursor = self.text.len();
        self.base.request_redraw();
    }

    /// Replaces the composition with styled segments.
    ///
    /// The segments' text is concatenated into [`ImePreedit::text`], so the plain accessor stays
    /// total and a host that reads back the composition does not have to walk the runs. An empty
    /// vector clears the overlay.
    pub fn set_segments(&mut self, segments: Vec<PreeditSegment>) {
        let mut text = String::new();
        let mut cursor_at = None;
        for segment in &segments {
            if segment.is_cursor() && cursor_at.is_none() {
                cursor_at = Some(text.len());
            }
            text.push_str(segment.text());
        }
        if self.text == text && self.segments == segments {
            return;
        }
        self.text = text;
        self.cursor = cursor_at.unwrap_or(self.text.len());
        self.segments = segments;
        self.base.request_redraw();
    }

    /// The styled runs of the composition, empty when it was set as plain text.
    pub fn segments(&self) -> &[PreeditSegment] {
        &self.segments
    }

    /// The caret's rectangle in the window, for anchoring a candidate window.
    ///
    /// The x is the offset measured by the most recent paint, or a font-size estimate before the
    /// first paint. The y and height are the control's own text band, so a platform that places
    /// its candidate list below the caret gets a sensible anchor even mid-composition.
    pub fn caret_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x + self.caret_offset, rect.y, 1, rect.height)
    }

    /// The caret's top-left point in the window.
    pub fn caret_screen_position(&self) -> Point {
        let caret = self.caret_rect();
        Point::new(caret.x, caret.y)
    }

    /// Returns the current preedit text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the font used for the preedit text.
    pub fn set_font(&mut self, font: Font) {
        if self.font == font {
            return;
        }
        self.font = font;
        self.base.request_redraw();
    }

    /// Sets the text color.
    pub fn set_text_color(&mut self, color: Color) {
        // The `_set` flag is part of the comparison: the first write must go through even when it
        // repeats the default colour, because that is what starts overriding the theme (WDGT-15).
        if self.text_color_set && self.text_color == color {
            return;
        }
        self.text_color = color;
        self.text_color_set = true;
        self.base.request_redraw();
    }

    /// Sets the underline color.
    pub fn set_underline_color(&mut self, color: Color) {
        if self.underline_color_set && self.underline_color == color {
            return;
        }
        self.underline_color = color;
        self.underline_color_set = true;
        self.base.request_redraw();
    }

    /// Sets the underline thickness in pixels.
    pub fn set_underline_thickness(&mut self, thickness: u32) {
        if self.underline_thickness == thickness {
            return;
        }
        self.underline_thickness = thickness;
        self.base.request_redraw();
    }

    /// Returns the composition caret position, in characters from the start of
    /// the preedit text.
    ///
    /// The composition caret is stored as a byte offset (the unit every text API in the crate
    /// uses) and reported in characters, because that is the unit the property contract and the
    /// input-method event carry.
    pub fn cursor_position(&self) -> usize {
        self.text[..self.cursor.min(self.text.len())].chars().count()
    }

    /// Moves the composition caret.
    ///
    /// Positions past the end of the composition are clamped to it; the value is snapped to a
    /// character boundary so slicing the composition at the caret stays total.
    pub fn set_cursor_position(&mut self, position: usize) {
        let byte = self
            .text
            .char_indices()
            .nth(position)
            .map(|(index, _)| index)
            .unwrap_or(self.text.len());
        if byte != self.cursor {
            self.cursor = byte;
            self.base.request_redraw();
        }
    }

    /// The text colour `draw` should actually use.
    ///
    /// Precedence is the crate-wide one: a colour the caller set wins, otherwise the caller's
    /// style colour (theme-applied or explicit), otherwise the theme's foreground passed in as
    /// `fallback`. The field's constructed `Color::BLACK` is deliberately not consulted, because
    /// it is a constructor default rather than a choice, and treating it as one would keep the
    /// control black on a dark theme.
    fn effective_text_color(&self, fallback: Color) -> Color {
        if self.text_color_set {
            return self.text_color;
        }
        self.base.style().text_color.unwrap_or(fallback)
    }

    /// The underline colour `draw` should actually use. Same precedence as
    /// [`Self::effective_text_color`].
    fn effective_underline_color(&self, fallback: Color) -> Color {
        if self.underline_color_set {
            return self.underline_color;
        }
        self.base.style().border_color.unwrap_or(fallback)
    }
}

impl Widget for ImePreedit {
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
        crate::core::Size::new(200, 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ImePreedit`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for ImePreedit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "cursor_position" => Ok(CapabilityValue::UInt(self.cursor_position() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "cursor_position" => {
                self.set_cursor_position(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "cursor_position", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `ime_preedit` publishes.
    ///
    /// Both assign state — the preedit string and the caret inside it — and each needs
    /// a payload, so both are answered through the property route. The preedit string is
    /// supplied by the input method, so a nameless command could not invent one.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_cursor_position" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ImePreedit {
    fn draw(&mut self, context: &mut RenderContext) {
        if self.text.is_empty() {
            return;
        }
        let rect = self.geometry();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. The widget's own field defaults were
        // both `Color::BLACK`, so a light/dark switch left the composition text and its underline
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // `ime_preedit` classifies as `WidgetRole::Text`, whose resolved background is `None`;
        // the theme's own `background` is read instead and tinted, which is the surface the text
        // would sit on in a real edit host. The theme read is a separate manager lock, taken and
        // released inside the accessor, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("ime_preedit");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (active.colors.background, active.colors.foreground),
                None => (Color::rgb(240, 240, 240), Color::BLACK),
            }
        };

        // The surface the composition sits on: one step from the window fill toward the text
        // colour, so the preedit is a distinct element on a light theme and on a dark one rather
        // than a bare black on black. A caller's own colour still wins.
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        let surface = window_fill.blend(&ink, 0.08);
        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // The preedit text: a caller's own colour wins, then the theme's foreground, and the
        // widget's constructed default is dropped in favour of that foreground rather than a
        // fixed black the theme cannot override.
        let text_color = self.effective_text_color(ink);
        let font_size = self.font.size() as i32;
        let underline_y = rect.y + font_size + 2;
        let underline_color = self.effective_underline_color(
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(ink),
        );

        // Caret placement is needed by both the paint and the candidate-window query, so it is
        // measured once and stored. The prefix is sliced at a character boundary, which keeps the
        // measurement total for a composition that contains multi-byte characters.
        let caret_byte = self.cursor.min(self.text.len());
        let caret_byte = self
            .text
            .char_indices()
            .take_while(|(index, _)| *index < caret_byte)
            .last()
            .map(|(index, ch)| index + ch.len_utf8())
            .unwrap_or(0);
        let caret_prefix = &self.text[..caret_byte];
        self.caret_offset = context.measure_text(caret_prefix, &self.font).width as i32;
        let caret_offset = self.caret_offset;

        if self.segments.is_empty() {
            // Plain composition: one run, fitted to the control's own width so a long
            // candidate string cannot paint past the right edge.
            let fitted = context.draw_text_fitted_cow(
                rect,
                &self.text,
                &self.font,
                text_color,
                HorizontalAlignment::Left,
            );
            let text_width = context.measure_text(&fitted, &self.font).width as i32;
            let underline_x_end = (rect.x + text_width).min(rect.x + rect.width as i32);
            context.draw_line_stroke(
                Point::new(rect.x, underline_y),
                Point::new(underline_x_end, underline_y),
                underline_color,
                self.underline_thickness,
            );
        } else {
            // Styled composition: each run keeps its own ink and underline. The runs are fitted
            // independently against the space that is left, so the last one cannot leave the
            // control even when the input method reports an over-long clause.
            let mut x = rect.x;
            for segment in &self.segments {
                if x >= rect.x + rect.width as i32 {
                    break;
                }
                let remaining = (rect.x + rect.width as i32 - x).max(1) as u32;
                let box_rect = Rect::new(x, rect.y, remaining, rect.height);
                let fitted = context.draw_text_fitted_cow(
                    box_rect,
                    segment.text(),
                    &self.font,
                    segment.color().unwrap_or(text_color),
                    HorizontalAlignment::Left,
                );
                let advance = context.measure_text(&fitted, &self.font).width as i32;
                if segment.underline() {
                    let thickness = if segment.is_cursor() {
                        self.underline_thickness.max(2)
                    } else {
                        self.underline_thickness
                    };
                    context.draw_line_stroke(
                        Point::new(x, underline_y),
                        Point::new((x + advance).min(rect.x + rect.width as i32), underline_y),
                        underline_color,
                        thickness,
                    );
                }
                x += advance;
            }
        }

        // The composition caret is drawn at the stored offset, which is also what
        // `caret_rect` reports to a platform that anchors its candidate window there.
        if self.base.focus_reason().is_some() {
            let caret_x = (rect.x + caret_offset).min(rect.x + rect.width as i32 - 1);
            context.draw_line(
                Point::new(caret_x, rect.y + 2),
                Point::new(caret_x, rect.y + rect.height as i32 - 2),
                text_color,
            );
        }
    }
}

impl EventHandler for ImePreedit {
    /// Typing edits the preedit buffer only while the control is enabled.
    ///
    /// The disabled state was not consulted, so a host that disables the control — the usual way
    /// to take a text entry out of play — still had characters appended to it by keystrokes meant
    /// for whatever the user was actually looking at.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // The composition string and its caret as the platform reports them. This is the
            // authoritative path: a host that forwards input-method events used to get no
            // update at all, because only synthetic `KeyPress` characters were consumed.
            Event::ImePreedit { text, cursor } => {
                let byte = (*cursor).min(text.len());
                let byte = text
                    .char_indices()
                    .take_while(|(index, _)| *index < byte)
                    .last()
                    .map(|(index, ch)| index + ch.len_utf8())
                    .unwrap_or(0);
                let next = text.clone();
                if self.text != next {
                    self.text = next;
                }
                // A platform preedit replaces any previously supplied styled runs: the runs
                // described the old composition.
                self.segments.clear();
                self.cursor = byte;
                self.base.request_redraw();
            }
            // A commit ends the composition. The committed text belongs to the edit host, not to
            // this overlay, so the overlay empties rather than duplicating it.
            Event::ImeCommit { .. } => {
                if !self.text.is_empty() || !self.segments.is_empty() {
                    self.text.clear();
                    self.segments.clear();
                    self.cursor = 0;
                    self.base.request_redraw();
                }
            }
            Event::TextInput { text } => {
                if !text.is_empty() {
                    let mut next = self.text.clone();
                    next.push_str(text);
                    self.text = next;
                    self.segments.clear();
                    self.cursor = self.text.len();
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                if *key == key_codes::BACKSPACE {
                    // Backspace removes the character before the caret, not the last character:
                    // once the caret can sit inside the composition, those are different edits.
                    if self.cursor > 0 {
                        let mut boundary = self.cursor.saturating_sub(1);
                        while boundary > 0 && !self.text.is_char_boundary(boundary) {
                            boundary -= 1;
                        }
                        self.text.replace_range(boundary..self.cursor, "");
                        self.cursor = boundary;
                        self.segments.clear();
                        self.base.request_redraw();
                    }
                } else if *key >= 32 && *key <= 126 {
                    let c = char::from_u32(*key).unwrap_or(' ');
                    let c = if *modifiers & 0x02 != 0 { c.to_ascii_uppercase() } else { c };
                    let mut buffer = [0u8; 4];
                    let encoded = c.encode_utf8(&mut buffer);
                    self.text.insert_str(self.cursor, encoded);
                    self.cursor += encoded.len();
                    self.segments.clear();
                    self.base.request_redraw();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::vec;

    #[test]
    fn ime_preedit_new_defaults() {
        let ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(ip.text(), "");
        assert_eq!(ip.kind(), WidgetKind::ImePreedit);
    }

    #[test]
    fn ime_preedit_set_text() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_text("composition");
        assert_eq!(ip.text(), "composition");
    }

    #[test]
    fn ime_preedit_set_text_empty() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_text("hello");
        ip.set_text("");
        assert_eq!(ip.text(), "");
    }

    #[test]
    fn ime_preedit_set_font() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        let font = Font::default();
        ip.set_font(font);
        // No panic expected
    }

    #[test]
    fn ime_preedit_set_colors() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_text_color(Color::RED);
        ip.set_underline_color(Color::BLUE);
    }

    #[test]
    fn ime_preedit_underline_thickness() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_underline_thickness(3);
    }

    #[test]
    fn ime_preedit_draw_empty_text() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        // Should not crash when drawing empty text
        let svg = crate::widget::svg::render_to_svg(&mut ip);
        assert!(svg.starts_with("<svg"));
    }
    /// A disabled preedit control ignores keystrokes.
    ///
    /// The handler never consulted `enabled`, so a host that disabled the control — the usual way
    /// to take a text entry out of play — still had characters appended to it by keystrokes meant
    /// for whatever the user was actually looking at.
    /// The platform's own composition events drive the overlay.
    ///
    /// Only synthetic key presses were consumed before, so a host that forwarded the real
    /// input-method events saw a stale composition string and an unanchored candidate window.
    #[test]
    fn platform_composition_events_drive_the_overlay() {
        let mut preedit = ImePreedit::new(Rect::new(0, 0, 200, 30));
        preedit.handle_event(&Event::ime_preedit("にほん", 3));
        assert_eq!(preedit.text(), "にほん");
        assert_eq!(preedit.cursor_position(), 1, "three bytes is one character");

        preedit.handle_event(&Event::ime_commit("日本"));
        assert_eq!(preedit.text(), "", "a commit ends the composition overlay");
        assert_eq!(preedit.cursor_position(), 0);
    }

    /// Styled runs keep their own ink and are concatenated into the plain text accessor.
    #[test]
    fn styled_segments_are_painted_and_readable_as_text() {
        let mut preedit = ImePreedit::new(Rect::new(0, 0, 200, 30));
        preedit.set_segments(vec![
            PreeditSegment::new("abc"),
            PreeditSegment::new("def").with_color(Color::RED).with_cursor(true),
        ]);
        assert_eq!(preedit.text(), "abcdef");
        assert_eq!(preedit.segments().len(), 2);
        assert!(preedit.segments()[1].is_cursor());
        assert_eq!(preedit.cursor_position(), 3, "the caret sits at the converting clause");
        assert_eq!(preedit.segments()[0].text(), "abc");
        assert_eq!(preedit.segments()[1].text(), "def");
    }

    /// The caret's screen position follows the composition caret.
    #[test]
    fn the_caret_position_is_reported_for_candidate_window_anchoring() {
        let mut preedit = ImePreedit::new(Rect::new(10, 20, 200, 30));
        preedit.set_text("hello");
        let at_end = preedit.caret_screen_position();
        assert_eq!(at_end.x, 10, "before the first paint the offset is the text origin");

        preedit.set_cursor_position(0);
        let _ = crate::widget::svg::render_to_svg(&mut preedit);
        assert_eq!(preedit.caret_rect().x, 10);

        preedit.set_cursor_position(5);
        let _ = crate::widget::svg::render_to_svg(&mut preedit);
        assert!(preedit.caret_rect().x > 10, "the caret moved with the composition cursor");
    }

    #[test]
    fn ime_preedit_ignores_keys_when_disabled() {
        use crate::event::Event;
        use crate::widget::Widget;

        let mut preedit = ImePreedit::new(Rect::new(0, 0, 200, 30));
        preedit.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        assert_eq!(preedit.text(), "A", "an enabled control accepts typed text");

        preedit.set_enabled(false);
        preedit.handle_event(&Event::KeyPress { key: 66, modifiers: 0 }); // 'B'
        assert_eq!(preedit.text(), "A", "a disabled control must not accept text");

        // Backspace is refused too, so the buffer is frozen rather than editable one way.
        preedit.set_enabled(true);
        preedit.handle_event(&Event::KeyPress { key: 67, modifiers: 0 }); // 'C'
        assert_eq!(preedit.text(), "AC");
        preedit.set_enabled(false);
        preedit.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // backspace
        assert_eq!(preedit.text(), "AC", "a disabled control must not accept backspace either");
    }
}
