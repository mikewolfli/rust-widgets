// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! FloatingLabel widget — a text input with a floating label (Material Design style).
//!
//! The FloatingLabel widget combines a text input field with a label that
//! animates from inside the field to above it when the field is focused or
//! contains text. It also supports placeholder text that is shown when the
//! field is empty and unfocused.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// When the label floats above the field instead of resting inside it.
///
/// The three variants are Material's [`FloatingLabelBehavior`], and each one names a
/// genuinely different *drawing* rule rather than a preference:
///
/// * [`Always`](FloatingLabelBehavior::Always) — Material's outlined/filled field with a
///   permanently visible caption. The label is above the field even while it is empty and
///   unfocused, so the field always says what it wants.
/// * [`Never`](FloatingLabelBehavior::Never) — the label is a plain inline placeholder and
///   never moves. This is what a caller who wants `<input placeholder>` semantics asks for.
/// * [`Auto`](FloatingLabelBehavior::Auto) — the classic behaviour: float once the field is
///   focused or holds text.
///
/// # Why this is a control property rather than three constructors
///
/// The behaviour changes *where the same label is drawn*, not what the control is, so a
/// single control with a switch keeps the widget identity (and every id, signal and
/// property binding) stable across the change — the same shape `TabWidget::tab_position`
/// and `FloatingLabel::focused` already use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FloatingLabelBehavior {
    /// Float the label only while the field is focused or non-empty.
    #[default]
    Auto,
    /// Always float the label above the field, even when unfocused and empty.
    Always,
    /// Never float the label; it always rests inside the field.
    Never,
}

impl FloatingLabelBehavior {
    /// The token this behaviour is carried as across the property boundary.
    ///
    /// The spelling matches the entries in `FLOATING_LABEL_PROPERTIES`'s `floating_label_behavior`
    /// row, so the token a caller reads is the token a caller may write back.
    pub fn to_token(self) -> &'static str {
        match self {
            FloatingLabelBehavior::Auto => "auto",
            FloatingLabelBehavior::Always => "always",
            FloatingLabelBehavior::Never => "never",
        }
    }

    /// Parses a property token, accepting the same spellings [`Self::to_token`] returns.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "auto" => Some(FloatingLabelBehavior::Auto),
            "always" => Some(FloatingLabelBehavior::Always),
            "never" => Some(FloatingLabelBehavior::Never),
            _ => None,
        }
    }
}

/// A text input with a floating label (Material Design TextInputLayout style).
///
/// The label floats above the text field when the field is focused or contains
/// text. When the field is empty and unfocused, the label appears inside the
/// field (or a separate placeholder text is shown). The transition between these
/// two states is animated: [`FloatingLabel::tick`] advances an interpolation
/// value (`0.0` = label inside, `1.0` = label fully above) that [`FloatingLabel`]
/// consumes when drawing, so the label smoothly rises rather than teleporting.
///
/// [`FloatingLabel::behavior`] selects which of the three policies decides whether the
/// label floats; see [`FloatingLabelBehavior`].
pub struct FloatingLabel {
    base: BaseWidget,
    text: String,
    label: String,
    placeholder: String,
    is_focused: bool,
    /// Whether the label currently floats, as resolved from `behavior`, focus and the
    /// text content. Derived state: every write that can change one of those three
    /// re-derives it through [`FloatingLabel::update_label_state`], so it cannot be
    /// written independently of them.
    show_label_above: bool,
    /// The policy that decides when the label floats. See [`FloatingLabelBehavior`].
    behavior: FloatingLabelBehavior,
    /// Interpolated float position, advanced toward `target_progress` by
    /// [`FloatingLabel::tick`] and consumed by the draw pass. `0.0` draws the
    /// label inline; `1.0` draws it fully above the field.
    ///
    /// Held as the shared [`Transition`](crate::style::Transition) rather than as a bare
    /// `f32`: the travel needs a *duration* and an *easing curve*, and both are theme
    /// decisions this control used to hardcode (see `tick`).
    travel: crate::style::Transition,
    /// The value the travel moves toward — `1.0` when the label should
    /// float, `0.0` when it should rest inline.
    target_progress: f32,
    /// Emitted when the text content changes.
    pub text_changed: Signal1<String>,
}

impl FloatingLabel {
    /// Creates a new FloatingLabel widget with the given geometry.
    ///
    /// By default, no text, empty label, empty placeholder, unfocused, and
    /// [`FloatingLabelBehavior::Auto`].
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FloatingLabel, geometry, "FloatingLabel"),
            text: String::new(),
            label: String::new(),
            placeholder: String::new(),
            is_focused: false,
            show_label_above: false,
            behavior: FloatingLabelBehavior::Auto,
            // The label rising out of a field is a direct reaction to focus, which is
            // what the theme's `fast` token describes.
            travel: crate::style::Transition::with_tempo(crate::style::TransitionTempo::Fast),
            target_progress: 0.0,
            text_changed: Signal1::new(),
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content and emits `text_changed` signal.
    /// Also updates the floating label state based on whether text is non-empty.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.update_label_state();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Returns the label text.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Sets the label text (shown inside field or floating above).
    pub fn set_label(&mut self, label: String) {
        self.label = label;
        self.update_label_state();
        self.base.request_redraw();
    }

    /// Returns the placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text (shown when empty and unfocused).
    pub fn set_placeholder(&mut self, placeholder: String) {
        self.placeholder = placeholder;
        self.base.request_redraw();
    }

    /// Returns whether the input field is currently focused.
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Returns whether the label is currently drawn floating above the field.
    ///
    /// Resolved from the [`Self::behavior`] policy together with focus and content, so it
    /// is the answer the draw pass acts on rather than a second copy of the policy.
    pub fn show_label_above(&self) -> bool {
        self.show_label_above
    }

    /// Returns the floating-label policy.
    pub fn behavior(&self) -> FloatingLabelBehavior {
        self.behavior
    }

    /// Sets the floating-label policy and re-derives whether the label floats.
    pub fn set_behavior(&mut self, behavior: FloatingLabelBehavior) {
        if self.behavior != behavior {
            self.behavior = behavior;
            self.update_label_state();
            self.base.request_redraw();
        }
    }

    /// Returns whether the text content is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Sets the focused state. When focused, the label floats above.
    pub fn set_focused(&mut self, focused: bool) {
        if self.is_focused != focused {
            self.is_focused = focused;
            self.update_label_state();
            self.base.request_redraw();
        }
    }

    /// Updates whether the label should float above, from the policy, focus and content.
    ///
    /// The policy is consulted first because both of the fixed behaviours override the
    /// focus/content rule entirely: `Never` pins the label inline, `Always` pins it above.
    /// Only `Auto` falls through to the focus/content test.
    fn update_label_state(&mut self) {
        let should_float = match self.behavior {
            FloatingLabelBehavior::Always => true,
            FloatingLabelBehavior::Never => false,
            FloatingLabelBehavior::Auto => self.is_focused || !self.text.is_empty(),
        };
        if should_float != self.show_label_above {
            self.show_label_above = should_float;
            // Retarget, rather than jump: `tick` then interpolates the visible
            // position toward this target across subsequent frames.
            self.target_progress = if should_float { 1.0 } else { 0.0 };
        }
    }

    /// Advances the floating-label animation toward its target.
    ///
    /// `delta_ms` is the elapsed time since the previous frame. Returns `true` when
    /// another frame is needed, so the caller can schedule one only while the label is
    /// still moving — the contract every animated control in this crate follows.
    ///
    /// # Why this drives the shared `Transition`
    ///
    /// The travel used to be `progress += (target - progress) * delta / 150`. That is not
    /// a 150 ms animation: it is an **exponential approach** that only reaches the target
    /// asymptotically, so the label never quite arrived and the control kept asking for
    /// frames. It also hardcoded its duration, which meant the theme's `Motion` tokens
    /// could not re-price it — the one animated control in the crate that ignored them.
    ///
    /// `Transition` supplies both halves correctly: the engine's eased interpolation over
    /// a real duration, and the duration read from `theme.motion` (the `fast` token, since
    /// a label rising out of a field is a direct reaction to focus).
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.travel.tick(self.target_progress, delta_ms) {
            // Settle exactly on the target: `Transition` snaps when it arrives, but a
            // caller reading `animation_progress()` must never see a value a hair short.
            self.travel.reset_to(self.target_progress);
            return false;
        }
        self.base.request_redraw();
        true
    }

    /// Returns the current interpolation of the label's float position, in
    /// `0.0 ..= 1.0`. Exposed for tests and animation-aware hosts; the draw pass
    /// consumes the same value to place the label.
    pub fn animation_progress(&self) -> f32 {
        self.travel.progress()
    }

    /// The resolved field fill.
    ///
    /// Explicit style first, then the theme's resolved style for this control, then the
    /// input role's, then a literal. The label's muted colour is derived from this fill, so
    /// it has to be reachable from `draw_label` as well as from `draw` — a second copy of
    /// the four-step chain would let the two disagree and the label blend toward a colour
    /// the field does not have.
    fn field_background_color(&self) -> Color {
        self.base
            .style()
            .background_color
            .or_else(|| {
                crate::style::resolved_theme_style("floating_label")
                    .and_then(|t| t.background_color)
            })
            .or_else(|| {
                // The label kind classifies as plain text, so the theme leaves its
                // background unset; a floating *label* decorates an editable field, so the
                // field interior is read from the input role, which resolves a colour in
                // every appearance.
                crate::style::resolved_theme_style("line_edit")
                    .and_then(|input| input.background_color)
            })
            .unwrap_or(Color::rgba(255, 255, 255, 255))
    }

    /// The label colour for the current state.
    ///
    /// Shared by both label placements in `draw_label` so the inline and floating forms of
    /// the same label cannot drift apart in colour as the state changes around them.
    fn label_color(&self, ink: Color, field_background: Color, is_enabled: bool) -> Color {
        if self.is_focused {
            ink
        } else if is_enabled {
            ink.blend(&field_background, 0.3)
        } else {
            Color::rgba(180, 180, 180, 255)
        }
    }

    /// Draws the label, either floating above the field or resting inside it.
    ///
    /// # Why the placement is derived rather than branched at the call site
    ///
    /// The label has exactly two positions and `animation_progress` interpolates between
    /// them, so every case — floating, in transit, inline, inline-as-placeholder — is one
    /// expression of (font, y) rather than four separate draw calls that could disagree
    /// about the geometry. It is also what makes `FloatingLabelBehavior::Never` correct by
    /// construction: that policy pins `show_label_above` false and the animation at `0.0`,
    /// so this draws at the inline position and there is no path that raises the label.
    fn draw_label(&self, context: &mut RenderContext, rect: Rect, ink: Color) {
        if self.label.is_empty() {
            return;
        }
        let field_background = self.field_background_color();
        let is_enabled = self.base.is_enabled();
        let input_font = Font::simple("sans-serif", INPUT_FONT_SIZE);
        let label_font = Font::simple("sans-serif", LABEL_FONT_SIZE);
        let label_x = rect.x + LABEL_PADDING;
        // Both text origins come from `text_line`, which centres the line box in the band.
        // The inline origin sits on the band the input text occupies, the floating one on
        // the caption band above it; deriving them from the same bands the rest of the
        // layout uses is what keeps the label on the input line rather than beside it.
        let inline_band = Rect::new(rect.x, rect.y + INLINE_BAND_TOP, 1, FIELD_LINE_HEIGHT);
        let inline_line = context.text_line(inline_band, &input_font);
        let floating_band = Rect::new(rect.x, rect.y + LABEL_TOP_MARGIN, 1, LABEL_LINE_HEIGHT);
        let floating_line = context.text_line(floating_band, &label_font);

        if self.show_label_above || self.travel.progress() > 0.0 {
            // Float: interpolate the origin from the inline line up to the caption line. The
            // caption font is used only once the label has fully risen, so the glyphs do not
            // change size mid-flight.
            let font = if self.travel.progress() >= 1.0 { &label_font } else { &input_font };
            let float_y = inline_line.y
                + ((floating_line.y - inline_line.y) as f32 * self.travel.progress()) as i32;
            context.draw_text(
                Point::new(label_x, float_y),
                &self.label,
                font,
                self.label_color(ink, field_background, is_enabled),
                HorizontalAlignment::Left,
            );
        } else {
            // Inline: the label sits on the input line and blends into the field the way a
            // placeholder does, which is what distinguishes it from the input text itself.
            let inline_ink = if is_enabled {
                ink.blend(&field_background, 0.45)
            } else {
                Color::rgba(180, 180, 180, 255)
            };
            context.draw_text(
                Point::new(label_x, inline_line.y),
                &self.label,
                &input_font,
                inline_ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl Widget for FloatingLabel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 40)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `FloatingLabel`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
///
/// # Why `label` is published separately from `text`
///
/// `text` is the *content the user typed or the host set*; `label` is the caption that
/// floats above it. They are two different things and this control is the one place where
/// conflating them is visibly wrong: without a `label` name the widget's own label was
/// unreachable, so a floating-label field had nothing to float. The shared constructor
/// helper writes a label into `text` (the first name in `LABEL_PROPERTY_NAMES`) and
/// [`crate::widget::capability::widget_label_property_name`] now prefers `label` for this
/// kind, so the constructor's string lands where the caption is drawn.
impl WidgetProperties for FloatingLabel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "label" => Ok(CapabilityValue::String(self.label().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "focused" => Ok(CapabilityValue::Bool(self.is_focused())),
            "floating_label_behavior" => {
                Ok(CapabilityValue::String(self.behavior().to_token().to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "label" => {
                self.set_label(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            "focused" => {
                self.set_focused(expect_bool(value)?);
                Ok(())
            }
            // An unknown token is a parse failure rather than a silently different
            // behaviour: a caller that wrote `"Always"` must be told the spelling is
            // `"always"` instead of having the write accepted and ignored. The accepted
            // spellings are the schema row's, so a caller reading the schema can write
            // back exactly what `get` returned.
            "floating_label_behavior" => {
                let token = expect_string(value)?;
                let behavior = FloatingLabelBehavior::from_token(&token)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_behavior(behavior);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "label",
            "placeholder",
            "focused",
            "floating_label_behavior",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for FloatingLabel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the field fill and
        // its underline were previously hardcoded.
        //
        // The theme reads are separate manager locks, each taken and released inside
        // `resolved_theme_style`, so none is held across the draw or across another
        // accessor — the global manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let field_background = self.field_background_color();
        // The label is a `Text` role, so its resolved ink is the theme's foreground; the
        // border colour carries the underline and the focused accent.
        let ink = style
            .text_color
            .or_else(|| {
                crate::style::resolved_theme_style("floating_label").and_then(|t| t.text_color)
            })
            .unwrap_or(Color::BLACK);
        let border_color = style
            .border_color
            .or_else(|| {
                crate::style::resolved_theme_style("floating_label").and_then(|t| t.border_color)
            })
            .or_else(|| {
                crate::style::resolved_theme_style("floating_label").and_then(|t| t.text_color)
            })
            .unwrap_or_else(|| ink.blend(&field_background, 0.55));

        // Draw the text field background
        let bg_color = if is_enabled { field_background } else { Color::rgba(240, 240, 240, 255) };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Draw the underline/border. Focused is the resolved ink, undamped so it reads as
        // active; resting is the same ink damped toward the field it sits on.
        let underline_color = if self.is_focused { ink } else { border_color };
        let underline_y = rect.y + rect.height as i32 - 2;
        let underline_rect = Rect::new(rect.x + 2, underline_y, rect.width.saturating_sub(4), 2);
        context.fill_rounded_rect(underline_rect, 1, underline_color);

        // The band the input text occupies. Which band that is depends only on whether the
        // caption is taking the top of the field, so the input line is resolved once here
        // and handed to every string that shares it. Deriving the origin from `text_line`
        // (rather than a literal y) is what keeps "centred in its band" true when the
        // constants change.
        let has_label = !self.label.is_empty();
        let floats = has_label && self.show_label_above;
        let input_font = Font::simple("sans-serif", INPUT_FONT_SIZE);
        let input_band = Rect::new(
            rect.x,
            rect.y + if floats { FLOATED_BAND_TOP } else { INLINE_BAND_TOP },
            1,
            if floats { INLINE_LINE_HEIGHT } else { FIELD_LINE_HEIGHT },
        );
        let input_line = context.text_line(input_band, &input_font);

        // Draw the label (floating above, or inline acting as the placeholder).
        self.draw_label(context, rect, ink);

        // Show placeholder when empty, unfocused, and the label is not occupying the input
        // line itself — otherwise the two strings would be drawn on top of each other.
        let show_placeholder =
            self.text.is_empty() && !self.is_focused && (!has_label || self.show_label_above);
        if show_placeholder && !self.placeholder.is_empty() {
            context.draw_text(
                Point::new(rect.x + LABEL_PADDING, input_line.y),
                &self.placeholder,
                &input_font,
                ink.blend(&field_background, 0.55),
                HorizontalAlignment::Left,
            );
        }

        // Draw input text
        if !self.text.is_empty() {
            let text_color = if is_enabled { ink } else { Color::rgba(160, 160, 160, 255) };
            context.draw_text(
                Point::new(rect.x + LABEL_PADDING, input_line.y),
                &self.text,
                &input_font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

/// Padding from the field's left edge to the label and the input text.
const LABEL_PADDING: i32 = 8;

/// Font size of the label once it has floated above the field.
const LABEL_FONT_SIZE: f32 = 11.0;

/// Font size of the input text, and of the label while it is inline.
const INPUT_FONT_SIZE: f32 = 14.0;

/// Space between the field's top edge and the floated caption's line box.
const LABEL_TOP_MARGIN: i32 = 4;

/// Height of the floated caption's line box.
///
/// Pinned rather than measured: this band is what reserves room for the caption, so it has
/// to be known before anything is drawn in it, and a measured height would make the input
/// line's position depend on the font backend.
const LABEL_LINE_HEIGHT: u32 = 12;

/// Space between the field's top edge and the input line's box when nothing floats.
const INLINE_BAND_TOP: i32 = 6;

/// Height of the input line's box when nothing floats above it.
const FIELD_LINE_HEIGHT: u32 = 28;

/// Space between the floated caption and the input line's box.
///
/// The floated layout splits the field into a caption band and an input band; this constant
/// and [`INLINE_LINE_HEIGHT`] are the second band, and together with [`LABEL_TOP_MARGIN`]
/// and [`LABEL_LINE_HEIGHT`] they are the whole vertical layout of the control.
const FLOATED_BAND_TOP: i32 = 20;

/// Height of the input line's box below a floated caption.
const INLINE_LINE_HEIGHT: u32 = 16;

const KEYCODE_TAB: u32 = 9;
const KEYCODE_ENTER: u32 = 13;
const KEYCODE_BACKSPACE: u32 = 8;

impl EventHandler for FloatingLabel {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();
                if rect.contains_point(*pos) {
                    self.set_focused(true);
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == KEYCODE_TAB {
                    // Tab — lose focus
                    self.set_focused(false);
                } else if *key == KEYCODE_ENTER {
                    // Enter — lose focus
                    self.set_focused(false);
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII — append to text
                    let c = char::from_u32(*key).unwrap_or(' ');
                    if self.is_focused {
                        let mut new_text = self.text.clone();
                        new_text.push(c);
                        self.set_text(new_text);
                    }
                } else if *key == KEYCODE_BACKSPACE {
                    // Backspace
                    if self.is_focused && !self.text.is_empty() {
                        let mut new_text = self.text.clone();
                        new_text.pop();
                        self.set_text(new_text);
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    /// One ink box `(left, top, right, bottom)` per text `<path>`, in document order.
    ///
    /// # Why the runs are recovered geometrically
    ///
    /// A text run leaves the backend as `font8x8` glyph geometry — one axis-aligned subpath per
    /// set bitmap bit, the same rectangles the software rasteriser fills — so the document holds
    /// a picture of the string rather than the string (see `crate::widget::svg::text_ink_box`).
    /// A test that needs a run other than the first must therefore find it by *where it is*.
    ///
    /// Subpaths are not deduplicated: a glyph box wider than 8 px maps two bitmap columns onto
    /// one pixel column and the backend emits that rectangle twice, exactly as the rasteriser
    /// fills it twice. The union is unaffected either way.
    #[cfg(not(alloc_frugal))]
    fn ink_runs(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        let mut runs = Vec::new();
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
                bounds = Some(match bounds {
                    None => (x, y, x + w, y + h),
                    Some((left, top, right, bottom)) => {
                        (left.min(x), top.min(y), right.max(x + w), bottom.max(y + h))
                    }
                });
            }
            if let Some(bounds) = bounds {
                runs.push(bounds);
            }
        }
        runs
    }

    #[test]
    fn floating_label_default_creation() {
        let fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        assert_eq!(fl.kind(), WidgetKind::FloatingLabel);
        assert!(fl.text().is_empty());
        assert!(fl.label().is_empty());
        assert!(fl.placeholder().is_empty());
        assert!(!fl.is_focused());
        assert!(fl.is_empty());
    }

    #[test]
    fn floating_label_set_text_and_label() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Username".to_string());
        assert_eq!(fl.label(), "Username");

        fl.set_text("hello".to_string());
        assert_eq!(fl.text(), "hello");
        assert!(!fl.is_empty());

        // Label should float above since text is non-empty; the target is 1.0 and
        // the interpolation animates toward it rather than jumping instantly.
        assert!(fl.show_label_above);
        assert_eq!(fl.animation_progress(), 0.0);
        assert!(fl.tick(16));
        assert!(fl.animation_progress() > 0.0);
    }

    #[test]
    fn floating_label_animation_reaches_target() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Email".to_string());
        fl.set_focused(true);
        assert!(fl.show_label_above);

        // A single large step lands **exactly** on the target. The return value is `false`
        // because there is no further work — the same settled signal every `tick` in this
        // crate reports. The old implementation returned `true` here, which was the visible
        // symptom of it being an asymptotic approach rather than a timed transition: it could
        // never report "arrived".
        assert!(!fl.tick(1000), "one long frame both arrives and reports settled");
        assert_eq!(fl.animation_progress(), 1.0);
        assert!(!fl.tick(1000)); // already at target — no further work

        // Losing focus retargets back to inline.
        fl.set_focused(false);
        assert!(!fl.show_label_above);
        assert!(!fl.tick(1000));
        assert_eq!(fl.animation_progress(), 0.0);

        // A short frame does *not* arrive, and does report more work — so the two answers are
        // actually distinguishable, which a transition that never settled could not express.
        fl.set_focused(true);
        assert!(fl.tick(1), "one millisecond cannot cross the travel");
        assert!(fl.animation_progress() > 0.0 && fl.animation_progress() < 1.0);
    }

    #[test]
    fn floating_label_focus_toggle() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Email".to_string());
        assert!(!fl.is_focused());
        assert!(!fl.show_label_above);

        fl.set_focused(true);
        assert!(fl.is_focused());
        assert!(fl.show_label_above);

        fl.set_focused(false);
        assert!(!fl.is_focused());
        // Should still float since text is empty? No, empty + not focused = not floating
        assert!(!fl.show_label_above);
    }

    #[test]
    fn floating_label_text_changed_signal() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        let captured = Arc::new(Mutex::new(None::<String>));
        fl.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        fl.set_text("World".to_string());
        assert_eq!(captured.lock().unwrap().as_deref(), Some("World"));
    }

    #[test]
    fn floating_label_placeholder() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_placeholder("Enter text here...".to_string());
        assert_eq!(fl.placeholder(), "Enter text here...");
    }

    #[test]
    fn floating_label_focus_on_click() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        assert!(!fl.is_focused());

        fl.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(fl.is_focused());
    }

    #[test]
    fn floating_label_keyboard_input() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_focused(true);

        // Type 'A'
        fl.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
        assert_eq!(fl.text(), "A");

        // Type 'B'
        fl.handle_event(&Event::KeyPress { key: 66, modifiers: 0 });
        assert_eq!(fl.text(), "AB");

        // Backspace
        fl.handle_event(&Event::KeyPress { key: 8, modifiers: 0 });
        assert_eq!(fl.text(), "A");
    }

    #[test]
    fn floating_label_svg_output() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Name".to_string());
        fl.set_placeholder("Enter name".to_string());
        fl.set_text("John".to_string());
        let svg = render_to_svg(&mut fl);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn floating_label_publishes_its_label_as_a_property() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };

        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        // The defect this pins: `label` was absent from the contract, so the caption the
        // control exists to float was unreachable through the property API.
        assert!(fl.property_names().contains(&"label"));
        assert!(fl.property_names().contains(&"floating_label_behavior"));

        widget_property_set(&mut fl, "label", CapabilityValue::String("Username".into()))
            .expect("`label` must be writable");
        assert_eq!(fl.label(), "Username");
        assert_eq!(
            widget_property_get(&fl, "label"),
            Ok(CapabilityValue::String("Username".to_string()))
        );
        // `label` and `text` are separate: writing the caption must not become content.
        assert!(fl.text().is_empty());
    }

    #[test]
    fn floating_label_behavior_round_trips_as_tokens() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };

        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        for behavior in [
            FloatingLabelBehavior::Auto,
            FloatingLabelBehavior::Always,
            FloatingLabelBehavior::Never,
        ] {
            let token = behavior.to_token();
            widget_property_set(
                &mut fl,
                "floating_label_behavior",
                CapabilityValue::String(token.to_string()),
            )
            .unwrap_or_else(|error| panic!("token {token:?} must be accepted, got {error:?}"));
            assert_eq!(fl.behavior(), behavior);
            // The read must return the same token, so a caller can write back what it read.
            assert_eq!(
                widget_property_get(&fl, "floating_label_behavior"),
                Ok(CapabilityValue::String(token.to_string()))
            );
        }
    }

    #[test]
    fn floating_label_behavior_rejects_an_unknown_token() {
        use crate::widget::capability::properties_trait::widget_property_set;

        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        // Accepting `"Always"` and silently doing nothing would leave the caller believing
        // the behaviour changed, and it is the spelling the schema does not publish.
        assert!(widget_property_set(
            &mut fl,
            "floating_label_behavior",
            CapabilityValue::String("Always".to_string())
        )
        .is_err());
        assert_eq!(fl.behavior(), FloatingLabelBehavior::Auto);
    }

    #[test]
    fn floating_label_always_floats_even_when_empty_and_unfocused() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Email".to_string());
        fl.set_behavior(FloatingLabelBehavior::Always);
        assert!(!fl.is_focused());
        assert!(fl.text().is_empty());
        assert!(fl.show_label_above);
        // The float is a real state change, not just a flag: the animation crosses the whole
        // travel and reports that it has no more work once it arrives.
        assert!(!fl.tick(1000), "a long frame completes the travel in one step");
        assert_eq!(fl.animation_progress(), 1.0);
    }

    #[test]
    fn floating_label_never_floats_even_when_focused_with_text() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Email".to_string());
        fl.set_behavior(FloatingLabelBehavior::Never);
        fl.set_text("not-an-email".to_string());
        fl.set_focused(true);
        assert!(fl.is_focused());
        assert!(!fl.text().is_empty());
        assert!(!fl.show_label_above);
        // Nothing is pending, so the label is pinned inline and `tick` has no work to do.
        assert!(!fl.tick(1000));
        assert_eq!(fl.animation_progress(), 0.0);
    }

    /// The three behaviours must produce three *different* drawings, which is the whole
    /// point of publishing the property: a flag that does not reach the paint pass is the
    /// declaration-without-behaviour defect this control already had once.
    ///
    /// # Why the label is located as ink
    ///
    /// The label is no longer a `<text>` element with a `y` attribute to read: the backend emits
    /// the `font8x8` rectangles the software rasteriser fills, one subpath per set bitmap bit,
    /// in a single `<path>` (see `crate::widget::svg::text_ink_box`). The string is absent from
    /// the document, so the run is found by *where it is* — the float position is above the
    /// input line, which is the only difference the policies make.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn floating_label_behavior_changes_the_drawn_label_position() {
        /// The ink box `(left, top, right, bottom)` of the caption when the policy allows it
        /// above the input line, and of the label resting **inline** otherwise.
        ///
        /// The field draws its label (or a placeholder) on the input line and the value on the
        /// same line, so the inline case has more than one run. The caption is the run the
        /// float moves, and the question the test asks is whether *any* run sits above the
        /// inline line, so the topmost run is the right witness in both cases.
        fn label_ink(behavior: FloatingLabelBehavior, focused: bool) -> (i32, i32, i32, i32) {
            let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
            fl.set_label("Email".to_string());
            fl.set_behavior(behavior);
            fl.set_focused(focused);
            // Settle the animation at whatever target the policy chose.
            fl.tick(1000);
            let svg = render_to_svg(&mut fl);
            let mut runs = ink_runs(&svg);
            assert!(!runs.is_empty(), "the label must be drawn as ink: {svg}");
            runs.sort_by_key(|run| run.1);
            runs[0]
        }

        let auto_unfocused = label_ink(FloatingLabelBehavior::Auto, false);
        let auto_focused = label_ink(FloatingLabelBehavior::Auto, true);
        let always_unfocused = label_ink(FloatingLabelBehavior::Always, false);
        let never_focused = label_ink(FloatingLabelBehavior::Never, true);

        // Floating means a smaller y (higher on the field) than resting inline.
        assert!(
            auto_focused.1 < auto_unfocused.1,
            "auto must float on focus: {} -> {}",
            auto_unfocused.1,
            auto_focused.1
        );
        assert_eq!(
            always_unfocused, auto_focused,
            "`always` must float a field that `auto` would leave inline"
        );
        assert_eq!(
            never_focused, auto_unfocused,
            "`never` must keep the label where an unfocused `auto` field draws it"
        );
    }
}
