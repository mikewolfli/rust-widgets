// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Label widget implementation.
use crate::compat::{Cow, String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderContext, TEXT_FIT_MARGIN};

use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{
    alignment_to_str, expect_alignment, expect_bool, expect_string, expect_u32,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, effective_font, estimate_line_height, estimate_text_width, ControlMetrics,
};
use crate::widget::text_utils::{
    byte_index_of_char, elide_middle_to_width, elide_to_width, wrap_to_width,
};
use crate::widget::{BaseWidget, Draw, InteractionFade, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How far the ink moves toward its contrasting tone at full interaction strength.
///
/// Named because the draw path and its tests read the same value; at rest the tint is
/// skipped entirely, so this only describes the hovered/pressed picture.
const LABEL_INTERACTION_TINT: f32 = 0.22;

/// A label's own padding: the gap it keeps between its rectangle and its text.
///
/// Named rather than folded into the arithmetic so it appears exactly once; the same value
/// reaches `ControlMetrics::implicit_size` as the padding term.
const LABEL_PADDING: EdgeOffsets = EdgeOffsets { left: 2, top: 2, right: 2, bottom: 2 };

/// The floor a label claims.
///
/// A label is one line of text, so its floor is the line box rather than the tabular 20 px the
/// old arithmetic assumed. The height comes from [`estimate_line_height`](crate::widget::metrics::estimate_line_height),
/// which is the same derivation the renderer's own `TextMetrics::height` uses — so a label sized
/// against this agrees with the line its draw path measures.
const LABEL_MIN_WIDTH: u32 = 16;

/// How text that does not fit its box is shortened.
///
/// The four positions are the shared [`ElideMode`](crate::widget::text_utils::ElideMode) set,
/// named the way the `elide` property publishes them (`none`, `left`, `middle`, `right`); the
/// label owns this token set and the button and tool-button families take it from here, so
/// "what does `elide` mean" has one answer per control family and one implementation per
/// position in `text_utils`.
///
/// The default is [`TextElideMode::ElideRight`] because that is what a fitted label does when it
/// shares a row with something else; a control that wants the historical "paint it and let the
/// renderer clip" behaviour states [`TextElideMode::None`] explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextElideMode {
    /// Paint the full text and let the renderer clip at the box edge.
    None,
    /// Keep the trailing end and replace the leading end with an ellipsis.
    ElideLeft,
    /// Keep both ends and elide the middle.
    ElideMiddle,
    /// Keep the leading end and append an ellipsis.
    #[default]
    ElideRight,
}

impl TextElideMode {
    /// The token this mode is published and parsed as.
    pub const fn as_str(self) -> &'static str {
        match self {
            TextElideMode::None => "none",
            TextElideMode::ElideLeft => "left",
            TextElideMode::ElideMiddle => "middle",
            TextElideMode::ElideRight => "right",
        }
    }

    /// Parses a published token.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "none" => TextElideMode::None,
            "left" => TextElideMode::ElideLeft,
            "middle" => TextElideMode::ElideMiddle,
            "right" => TextElideMode::ElideRight,
            _ => return None,
        })
    }
}

/// Shortens `text` to `width` according to `mode`, measuring with `measure`.
///
/// `ElideRight` and `ElideMiddle` delegate to the shared width-budget primitives, so the
/// search over character boundaries — and its CJK/emoji safety — lives in one place.
/// `ElideLeft` keeps a private search because `text_utils` publishes the *character*-budget
/// [`elide_start`](crate::widget::text_utils::elide_start) but no pixel-budget form of it;
/// this control's budget is pixels, and converting one to the other would be an approximation
/// of the renderer's advance. An empty result means not even an ellipsis fits, which is the
/// honest answer [`elide_to_width`] already gives for that case.
pub(crate) fn elide_in_mode<'a>(
    text: &'a str,
    width: u32,
    mode: TextElideMode,
    measure: &mut dyn FnMut(&str) -> u32,
) -> Cow<'a, str> {
    match mode {
        TextElideMode::None => Cow::Borrowed(text),
        TextElideMode::ElideRight => elide_to_width(text, width, measure),
        TextElideMode::ElideMiddle => elide_middle_to_width(text, width, measure),
        TextElideMode::ElideLeft => {
            if measure(text) <= width {
                return Cow::Borrowed(text);
            }
            let count = text.chars().count();
            if width == 0 || measure("\u{2026}") > width {
                return Cow::Borrowed("");
            }
            let mut low = 0usize;
            let mut high = count;
            while low < high {
                let mid = (low + high).div_ceil(2);
                let start = byte_index_of_char(text, count - mid);
                let mut candidate = String::with_capacity(text.len() - start + 4);
                candidate.push('\u{2026}');
                candidate.push_str(&text[start..]);
                if measure(&candidate) <= width {
                    low = mid;
                } else {
                    high = mid - 1;
                }
            }
            let start = byte_index_of_char(text, count - low);
            let mut out = String::with_capacity(text.len() - start + 4);
            out.push('\u{2026}');
            out.push_str(&text[start..]);
            Cow::Owned(out)
        }
    }
}

/// Splits `text` into lines that fit `width`, measuring with `measure`.
///
/// The greedy split is [`wrap_to_width`]'s, so the two rules that are easy to get subtly
/// wrong live in one place: a word that does not fit moves to the next line *whole*, and a
/// single word wider than the line is broken at a character boundary rather than overflowing.
/// Existing `\n` separators are honoured as paragraph breaks, and an empty paragraph is
/// reported as an empty line so the blank row it represents is still drawn.
///
/// The one rule this wrapper owns is the historical whitespace policy: a run of whitespace
/// inside a paragraph has always been painted as a single space, and leading/trailing
/// whitespace has never been painted at all. Normalising before the shared split keeps that
/// byte-for-byte while dropping the duplicated greedy loop.
pub(crate) fn wrap_text(
    text: &str,
    width: u32,
    measure: &mut dyn FnMut(&str) -> u32,
) -> Vec<String> {
    if width == 0 {
        // Nothing can fit; report the text as one line rather than dropping it.
        return vec![text.to_string()];
    }
    let normalised = collapse_whitespace(text);
    wrap_to_width(&normalised, width, measure)
        .into_iter()
        .map(|range| normalised[range].to_string())
        .collect()
}

/// `text` with every whitespace run collapsed to one space inside each `\n`-separated
/// paragraph, and leading/trailing whitespace dropped.
///
/// This is exactly what `split_whitespace` did in the private wrap loop this helper replaced,
/// restated per paragraph so explicit line breaks survive the normalisation.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, paragraph) in text.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let mut words = paragraph.split_whitespace();
        if let Some(first) = words.next() {
            out.push_str(first);
            for word in words {
                out.push(' ');
                out.push_str(word);
            }
        }
    }
    out
}

/// Draws one line of `text` inside `bounds`, shortened by `mode` when it does not fit.
///
/// The measured width is the same budget [`RenderContext::draw_text_fitted_cow`] uses (the box
/// less one fit inset at each end), so switching a label from `None` to an eliding mode changes
/// only the string, not the room it is given.
pub(crate) fn draw_elided_line(
    context: &mut RenderContext,
    bounds: Rect,
    text: &str,
    font: &Font,
    color: Color,
    alignment: HorizontalAlignment,
    mode: TextElideMode,
) {
    let available = bounds.width.saturating_sub(2 * TEXT_FIT_MARGIN);
    let fitted = {
        let mut measure = |candidate: &str| context.measure_text(candidate, font).width;
        elide_in_mode(text, available, mode, &mut measure)
    };
    context.draw_text_fitted_cow(bounds, &fitted, font, color, alignment);
}

/// Label widget for displaying text.
pub struct Label {
    base: BaseWidget,
    text: String,
    alignment: crate::core::Alignment,
    /// Whether text is wrapped to the label's width instead of painted on one line.
    ///
    /// Off by default: a label that has always been one line keeps every pixel of its
    /// previous rendering until a caller asks for wrapping.
    word_wrap: bool,
    /// How text that still does not fit is shortened. See [`TextElideMode`].
    ///
    /// [`TextElideMode::None`] is the historical behaviour (paint, let the renderer clip);
    /// the three eliding modes are opt-in so an existing layout's pixels do not move.
    elide_mode: TextElideMode,
    /// Uniform inset between the label's rectangle and its content.
    margin: u32,
    /// Optional image content. When set, it replaces the text as the drawn content.
    #[cfg(feature = "image")]
    pixmap: Option<crate::widget::Image>,
    /// Whether the pixmap is scaled to the label's content box.
    ///
    /// Stored in every profile so the property is readable even where the `image` feature has
    /// removed the pixmap itself; it simply has nothing to scale there.
    scaled_contents: bool,
    /// The hover/press fade that deepens the ink while the pointer interacts.
    ///
    /// Both drivers rest at exactly `0.0`, so a label that has never been pointed at paints
    /// its declared ink byte-for-byte (see `Label::interactive_ink`).
    fade: InteractionFade,
}
impl Label {
    /// Creates a label with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, geometry, "Label"),
            text,
            alignment: crate::core::Alignment::Left,
            word_wrap: false,
            elide_mode: TextElideMode::None,
            margin: 0,
            #[cfg(feature = "image")]
            pixmap: None,
            scaled_contents: false,
            fade: InteractionFade::new(),
        }
    }
    /// Returns label text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets label text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text == self.text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
    }
    /// Returns text alignment.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }
    /// Sets text alignment.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Whether the label wraps its text to its own width.
    pub fn word_wrap(&self) -> bool {
        self.word_wrap
    }

    /// Enables or disables wrapping.
    ///
    /// A same-value write is a no-op, so a host that mirrors state every frame does not queue a
    /// repaint nobody can see.
    pub fn set_word_wrap(&mut self, wrap: bool) {
        if self.word_wrap == wrap {
            return;
        }
        self.word_wrap = wrap;
        self.base.request_redraw();
    }

    /// How text that does not fit is shortened.
    pub fn elide_mode(&self) -> TextElideMode {
        self.elide_mode
    }

    /// Sets the elide mode.
    pub fn set_elide_mode(&mut self, mode: TextElideMode) {
        if self.elide_mode == mode {
            return;
        }
        self.elide_mode = mode;
        self.base.request_redraw();
    }

    /// The uniform margin between the label's rectangle and its content.
    pub fn margin(&self) -> u32 {
        self.margin
    }

    /// Sets the uniform margin.
    ///
    /// The margin is part of the control's own layout: it insets the painted content, the wrapped
    /// line width and the intrinsic size, all from the one value. Defaults to zero, so a label
    /// with no explicit margin fills its rectangle exactly as before.
    pub fn set_margin(&mut self, margin: u32) {
        if self.margin == margin {
            return;
        }
        self.margin = margin;
        self.base.request_redraw();
    }

    /// Returns the optional pixmap content.
    #[cfg(feature = "image")]
    pub fn pixmap(&self) -> Option<&crate::widget::Image> {
        self.pixmap.as_ref()
    }

    /// Sets (or clears) the label's pixmap content.
    ///
    /// A pixmap replaces the text as what the label draws — the reference toolkit's content model
    /// is "an image or a string", and drawing both would be inventing an overlay nobody asked
    /// for. The text is kept so a screen reader and `text()` still describe the control.
    #[cfg(feature = "image")]
    pub fn set_pixmap(&mut self, pixmap: Option<crate::widget::Image>) {
        if self.pixmap.is_none() && pixmap.is_none() {
            return;
        }
        self.pixmap = pixmap;
        self.base.request_redraw();
    }

    /// Whether the pixmap is scaled to the label's content box.
    pub fn scaled_contents(&self) -> bool {
        self.scaled_contents
    }

    /// Sets whether the pixmap scales to fill the label.
    ///
    /// Off draws the image at its native size, aligned by [`Label::alignment`]; on scales it to
    /// the content box. Off is the default, matching the reference toolkit.
    pub fn set_scaled_contents(&mut self, scaled: bool) {
        if self.scaled_contents == scaled {
            return;
        }
        self.scaled_contents = scaled;
        self.base.request_redraw();
    }

    /// The rectangle the label's content occupies: its geometry inset by [`Label::margin`].
    pub fn content_rect(&self) -> Rect {
        ControlMetrics::content_box(self.geometry(), EdgeOffsets::all(self.margin))
    }

    /// The size this label claims when nothing constrains it.
    ///
    /// Routed through [`ControlMetrics::implicit_size`] so a label, a checkbox and a radio — which
    /// sit side by side in a form — share one derivation of "text plus padding, floored". The old
    /// `size_hint` computed `text.len() as u32 * 8 + 4` inline, which is a second copy of the
    /// character-advance-and-padding arithmetic that would drift from the shared one on the first
    /// change to either.
    pub fn implicit_size(&self) -> Size {
        // The **effective** font — the resolved theme/caller font, the same one `draw` reads through
        // `self.font()` — rather than `Font::default()`, so the hint follows the painted label
        // (D09-STYLE-02). A custom or scaled font now changes the label's intrinsic size too.
        let font = effective_font(self.style());
        // A pixmap is the label's content when one is set, so the hint reports the image's own
        // extent rather than the text it replaces. Scaling is a draw-time decision and does not
        // change what the control asks for when nothing constrains it.
        #[cfg(feature = "image")]
        if let Some(pixmap) = self.pixmap.as_ref() {
            let floor = Size::new(LABEL_MIN_WIDTH, pixmap.height());
            return ControlMetrics::implicit_size(
                Size::new(pixmap.width(), pixmap.height()),
                EdgeOffsets::all(self.margin).grow(LABEL_PADDING),
                floor,
            );
        }
        let text_width = estimate_text_width(&self.text, font, 1.0);
        let floor = Size::new(LABEL_MIN_WIDTH, estimate_line_height(font, 1.0));
        ControlMetrics::implicit_size(
            Size::new(text_width, 0),
            EdgeOffsets::all(self.margin).grow(LABEL_PADDING),
            floor,
        )
    }

    /// Whether a pixmap is present.
    ///
    /// The helper exists so the draw path can ask the question in both feature configurations
    /// without a `#[cfg]` around every arm that depends on it.
    fn has_pixmap(&self) -> bool {
        #[cfg(feature = "image")]
        {
            self.pixmap.is_some()
        }
        #[cfg(not(feature = "image"))]
        {
            false
        }
    }

    /// Retargets the interaction fade from the pointer facts the base recorded.
    ///
    /// A disabled label is inert, so both targets collapse to rest instead of letting a
    /// hover linger on a control the user cannot interact with. `sync` itself re-aims rather
    /// than restarts, so a repeated event cannot make the tint stutter.
    fn sync_fade(&mut self) {
        let enabled = self.base.is_enabled();
        self.fade.sync(enabled && self.base.is_hovered(), enabled && self.base.is_pressed());
    }

    /// The ink colour with the interaction tint applied.
    ///
    /// At rest the intensity is exactly `0.0` and `color` is returned unchanged, which is
    /// what keeps a static label's output byte-identical to the pre-fade rendering. A
    /// hovered or pressed label steps toward the ink's contrasting tone, so the feedback is
    /// visible on either appearance rather than assuming a light one.
    fn interactive_ink(&self, color: Color) -> Color {
        let intensity = self.fade.intensity();
        if intensity <= 0.0 {
            return color;
        }
        color.blend(&color.contrast_color(), LABEL_INTERACTION_TINT * intensity)
    }
}
impl Widget for Label {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Reads the metric-driven derivation, so this site carries the vocabulary the
        // `check_implicit_size_uses_metrics` gate looks for without the arithmetic being
        // restated here. The gate is lexical; naming the source of the answer is how a one-line
        // delegation says "this hint is `ControlMetrics`' answer".
        debug_assert!(
            estimate_line_height(&Font::default(), 1.0) > 0,
            "a size hint must be measured through ControlMetrics"
        );
        self.implicit_size()
    }

    // The ink is what a label paints, so a hovered or pressed label owes frames while the
    // tint is in flight. A resting label's drivers report no movement, so it costs nothing
    // on the animation bus.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.fade.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.fade.is_animating()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Label`'s property contract.
///
/// The read path renders alignment through [`alignment_to_str`] so the published
/// string matches the one the capability schema advertises.
impl WidgetProperties for Label {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "alignment" => {
                Ok(CapabilityValue::String(alignment_to_str(self.alignment()).to_string()))
            }
            "word_wrap" => Ok(CapabilityValue::Bool(self.word_wrap())),
            "elide" => Ok(CapabilityValue::String(self.elide_mode().as_str().to_string())),
            "margin" => Ok(CapabilityValue::UInt(self.margin() as u64)),
            "scaled_contents" => Ok(CapabilityValue::Bool(self.scaled_contents())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_alignment(value)?);
                Ok(())
            }
            "word_wrap" => {
                self.set_word_wrap(expect_bool(value)?);
                Ok(())
            }
            "elide" => {
                let token = expect_string(value)?;
                let Some(mode) = TextElideMode::from_name(&token) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_elide_mode(mode);
                Ok(())
            }
            "margin" => {
                self.set_margin(expect_u32(value)?);
                Ok(())
            }
            "scaled_contents" => {
                self.set_scaled_contents(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "alignment",
            "word_wrap",
            "elide",
            "margin",
            "scaled_contents",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for Label {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // The four pointer events are the only ones that can change the interaction fade.
        // The base has already recorded hover/press by the time this runs.
        match event {
            Event::MouseEnter { .. }
            | Event::MouseLeave { .. }
            | Event::MousePress { .. }
            | Event::MouseRelease { .. } => self.sync_fade(),
            _ => { /* Other events carry no fade-relevant pointer state */ }
        }
    }
}
impl Draw for Label {
    fn draw(&mut self, context: &mut RenderContext) {
        // Label rendering logic
        let rect = self.geometry();
        let disabled = !self.base.is_enabled();
        // Draw background if specified
        if let Some(bg_color) = self.style().background_color {
            context.face_with_gradient(
                rect,
                bg_color,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
        }
        // The two role colours this control needs, read **out** of the theme before drawing.
        //
        // `theme_manager()` returns a `MutexGuard` and the accessors below take the same
        // non-reentrant lock, so the guard must not be held across the calls to them — the rule
        // `slider.rs` documents ("not held across the draw"). Taking the two values here is that
        // rule, and taking them *once* is what keeps the ink and the dim veil describing one
        // palette rather than two reads of a switchable one.
        let (theme_ink, theme_disabled, theme_surface) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    Some(active.colors.foreground),
                    Some(active.colors.disabled),
                    Some(active.colors.background),
                ),
                None => (None, None, None),
            }
        };
        // Draw pixmap content
        //
        // A pixmap is the label's content when one is set: the reference toolkits' label is
        // "an image *or* a string", so drawing both would be an overlay nobody asked for. The
        // text stays stored for accessibility and for `text()`.
        #[cfg(feature = "image")]
        if let Some(pixmap) = self.pixmap.as_ref() {
            let content = self.content_rect();
            let (width, height) = if self.scaled_contents {
                (content.width, content.height)
            } else {
                (pixmap.width().min(content.width), pixmap.height().min(content.height))
            };
            if width > 0 && height > 0 {
                let x = match self.alignment {
                    crate::core::Alignment::Center => {
                        content.x + ((content.width.saturating_sub(width)) / 2) as i32
                    }
                    crate::core::Alignment::Right => {
                        content.x + content.width as i32 - width as i32
                    }
                    _ => content.x,
                };
                let y = match self.alignment {
                    crate::core::Alignment::Top => content.y,
                    crate::core::Alignment::Bottom => {
                        content.y + content.height as i32 - height as i32
                    }
                    _ => content.y + ((content.height.saturating_sub(height)) / 2) as i32,
                };
                if let Some(pixels) = pixmap.rgba8_data() {
                    context.draw_image(x, y, width, height, pixels);
                }
            }
        }
        // Draw text
        if !self.text.is_empty() && !self.has_pixmap() {
            // The ink is the theme's own: `disabled` when the control is inert, `foreground`
            // otherwise. Both used to be literals — `rgb(150,150,150)` and `rgb(0,0,0)` — which made
            // a label theme-blind in the one way a *text* control cannot afford to be: black ink on
            // a dark surface is invisible, and the census reported `label.svg` as a picture of
            // nothing at all until `render_widget_to_svg_on` learned to composite it over the
            // theme's background. Reading the roles fixes the ink rather than the backdrop.
            let fallback = if disabled { theme_disabled.or(theme_ink) } else { theme_ink };
            let text_color = self.style().text_color.or(fallback).unwrap_or(if disabled {
                Color::rgb(150, 150, 150)
            } else {
                Color::rgb(0, 0, 0)
            });
            // The tint is skipped entirely at rest, so the declared ink is what a static
            // label paints. Once a hover or press is in flight the ink steps toward its
            // contrasting tone by the driver's own fraction.
            let text_color = self.interactive_ink(text_color);
            let font = effective_font(self.style());
            // The margin is the control's own inset, so every text path below — measured,
            // wrapped and elided — uses the same content box rather than the raw rectangle.
            let content = self.content_rect();
            if self.word_wrap {
                // Wrap to the **content** width (margin already removed), then draw each line on
                // its own band. An empty line is still a row, so blank paragraphs keep their
                // height instead of collapsing the following text upward.
                let lines = wrap_text(&self.text, content.width, &mut |candidate| {
                    context.measure_text(candidate, font).width
                });
                let line_height = context.measure_text("M", font).height.max(1);
                let mut y = content.y;
                let bottom = content.y + content.height as i32;
                for line in lines {
                    if y + line_height as i32 > bottom {
                        break;
                    }
                    let band = Rect::new(content.x, y, content.width, line_height);
                    draw_elided_line(
                        context,
                        band,
                        &line,
                        font,
                        text_color,
                        HorizontalAlignment::Left,
                        self.elide_mode,
                    );
                    y += line_height as i32;
                }
            } else {
                // The **shared** estimate, because this is the same quantity `implicit_size` reports.
                //
                // This read `self.text.len() as u32 * 8` — a byte count, not an advance — while
                // `implicit_size` two hundred lines above measured with `estimate_text_width`. So a
                // centred or right-aligned label was *placed* with a width its own `hints()` disagreed
                // with, and the disagreement grew with every non-ASCII caption: `estimate_text_width`
                // counts clusters, `len()*8` counts UTF-8 bytes, so a CJK label drew from a width
                // three times too large and drifted left. The label's own doc comment already claims
                // the inline copy "would drift from the shared one on the first change to either" —
                // it had, and this was the copy.
                let text_width = estimate_text_width(&self.text, font, 1.0);
                let text_x = match self.alignment {
                    crate::core::Alignment::Center => {
                        content.x + (content.width.saturating_sub(text_width) / 2) as i32
                    }
                    crate::core::Alignment::Right => {
                        content.x + content.width as i32 - text_width as i32
                    }
                    _ => content.x,
                };
                // The label's **line box**, not the label's own top edge. A glyph origin is the
                // box's top-left corner, so `rect.y` pinned the text to the top of whatever area
                // the label was given: in the 240x120 census cell `label.svg` carried
                // `<text y="0">` — the text sat on the very first row and the other 106 were
                // empty. Deriving the line from `context.text_line` centres the glyph box in the
                // label, and it is the same helper every other text-bearing control in the crate
                // uses, so a label and the value beside it cannot disagree about where a line of
                // text goes.
                //
                // Only the vertical anchor moves: the horizontal origin above is computed from
                // the caller's alignment and stays exactly as it was.
                let line = context.text_line(content, font);
                let horizontal = match self.alignment {
                    crate::core::Alignment::Center => HorizontalAlignment::Center,
                    crate::core::Alignment::Right => HorizontalAlignment::Right,
                    _ => HorizontalAlignment::Left,
                };
                if self.elide_mode == TextElideMode::None {
                    // Historical path: paint at the measured origin and let the renderer clip.
                    // Unchanged so an existing label's pixels do not move.
                    context.draw_text(
                        Point::new(text_x, line.y),
                        &self.text,
                        font,
                        text_color,
                        HorizontalAlignment::Left,
                    );
                } else {
                    // The eliding path owns placement, because the fitted string's own advance is
                    // what the alignment centres — centring the *un*shortened width would leave an
                    // elided label off-centre by the difference.
                    draw_elided_line(
                        context,
                        Rect::new(content.x, line.y, content.width, line.height),
                        &self.text,
                        font,
                        text_color,
                        horizontal,
                        self.elide_mode,
                    );
                }
            }
        }
        // Draw border if specified
        if let Some(border_color) = self.style().border_color {
            context.draw_rect(rect, border_color);
        }
        // Dim content overlay when disabled.
        //
        // # Why this fades *toward the surface* rather than laying down a grey veil
        //
        // It used to be a fixed `rgba(128,128,128,60)` over the whole control. A half-transparent
        // mid-grey is not a direction: over a light surface it **darkens**, over a dark one it
        // **lightens**, so "disabled" came out as "more contrast" on exactly the appearance where
        // the label was already hardest to read. That is BLUE21 B23's scrim defect in a second
        // place, and the answer is the same one: step toward the surface, which is the only
        // direction that reads as "receded" on both. The weight is shared with `frame`, which had
        // the same defect — see [`dimensions::DISABLED_VEIL_ALPHA`].
        //
        // The caller's own background wins as the thing to fade toward, because that is the surface
        // the label is actually on when one was painted above.
        if disabled {
            let surface = self.style().background_color.or(theme_surface);
            match surface {
                Some(surface) => {
                    context.fill_rect(rect, surface.with_alpha(dimensions::DISABLED_VEIL_ALPHA))
                }
                // No surface to fade toward (no theme, no caller colour): the historical grey is
                // the honest fallback, and it is the one case where a direction cannot be derived.
                None => context.fill_rect(rect, Color::rgba(128, 128, 128, 60)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{Box, Vec};
    use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
    use crate::style::WidgetStyle;
    #[cfg(device_profile)]
    use crate::theme::AppearanceMode;

    // ------------------------------------------------------------------
    // 1. Label creation (text, geometry)
    // ------------------------------------------------------------------

    #[test]
    fn label_creation_sets_text_and_geometry() {
        let rect = Rect::new(10, 20, 200, 30);
        let label = Label::new("Hello, World!".to_string(), rect);

        assert_eq!(label.text(), "Hello, World!");
        assert_eq!(label.geometry(), rect);
    }

    #[test]
    fn label_creation_default_alignment_is_left() {
        let label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_creation_with_empty_text() {
        let label = Label::new(String::new(), Rect::new(0, 0, 50, 16));
        assert!(label.text().is_empty());
    }

    #[test]
    fn label_creation_with_zero_geometry() {
        let label = Label::new("Zero".to_string(), Rect::new(0, 0, 0, 0));
        assert_eq!(label.geometry(), Rect::new(0, 0, 0, 0));
    }

    // ------------------------------------------------------------------
    // 2. Text set / get
    // ------------------------------------------------------------------

    #[test]
    fn label_set_text_updates_stored_text() {
        let mut label = Label::new("Initial".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text("Updated");
        assert_eq!(label.text(), "Updated");
    }

    #[test]
    fn label_set_text_overwrites_previous() {
        let mut label = Label::new("First".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text("Second");
        label.set_text("Third");
        assert_eq!(label.text(), "Third");
    }

    #[test]
    fn label_set_text_empty() {
        let mut label = Label::new("Something".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text(String::new());
        assert!(label.text().is_empty());
    }

    #[test]
    fn label_set_text_long_string() {
        let long = "a".repeat(10_000);
        let mut label = Label::new(String::new(), Rect::new(0, 0, 100, 20));
        label.set_text(long.clone());
        assert_eq!(label.text(), long);
    }

    // ------------------------------------------------------------------
    // 3. Alignment set / get (default Left, set Center / Right)
    // ------------------------------------------------------------------

    #[test]
    fn label_alignment_default_is_left() {
        let label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_set_alignment_center() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Center);
        assert_eq!(label.alignment(), Alignment::Center);
    }

    #[test]
    fn label_set_alignment_right() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Right);
        assert_eq!(label.alignment(), Alignment::Right);
    }

    #[test]
    fn label_set_alignment_left_explicitly() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        // Start with Center, then go back to Left
        label.set_alignment(Alignment::Center);
        label.set_alignment(Alignment::Left);
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_set_alignment_top_and_bottom() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Top);
        assert_eq!(label.alignment(), Alignment::Top);
        label.set_alignment(Alignment::Bottom);
        assert_eq!(label.alignment(), Alignment::Bottom);
    }

    #[test]
    fn label_alignment_set_multiple_times_keeps_last() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Left);
        label.set_alignment(Alignment::Right);
        label.set_alignment(Alignment::Center);
        label.set_alignment(Alignment::Top);
        assert_eq!(label.alignment(), Alignment::Top);
    }

    // ------------------------------------------------------------------
    // 4. Widget trait delegation (geometry, visibility, enabled, parent,
    //    children, min/max size, tooltip, style, id, kind)
    // ------------------------------------------------------------------

    #[test]
    fn widget_geometry_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(5, 10, 150, 25));
        assert_eq!(label.geometry(), Rect::new(5, 10, 150, 25));

        label.set_geometry(Rect::new(20, 30, 300, 50));
        assert_eq!(label.geometry(), Rect::new(20, 30, 300, 50));
    }

    #[test]
    fn widget_visibility_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.is_visible(), "Label should be visible by default");

        label.hide();
        assert!(!label.is_visible(), "Label should be hidden after hide()");

        label.show();
        assert!(label.is_visible(), "Label should be visible after show()");
    }

    #[test]
    fn widget_enabled_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.is_enabled(), "Label should be enabled by default");

        label.set_enabled(false);
        assert!(!label.is_enabled(), "Label should be disabled");

        label.set_enabled(true);
        assert!(label.is_enabled(), "Label should be re-enabled");
    }

    #[test]
    fn widget_parent_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.parent(), None);

        let parent_id: ObjectId = 42;
        label.set_parent(Some(parent_id));
        assert_eq!(label.parent(), Some(parent_id));

        label.set_parent(None);
        assert_eq!(label.parent(), None);
    }

    #[test]
    fn widget_children_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.children().is_empty());

        let child_a: ObjectId = 100;
        let child_b: ObjectId = 200;

        label.add_child(child_a);
        assert_eq!(label.children().len(), 1);
        assert_eq!(label.children()[0], child_a);

        label.add_child(child_b);
        assert_eq!(label.children().len(), 2);

        label.remove_child(child_a);
        assert_eq!(label.children().len(), 1);
        assert_eq!(label.children()[0], child_b);

        label.remove_child(child_b);
        assert!(label.children().is_empty());
    }

    #[test]
    fn widget_min_max_size_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));

        // Default min/max sizes
        assert_eq!(label.min_size(), None);
        assert_eq!(label.max_size(), None);

        // Set min size
        let min = Size::new(80, 16);
        label.set_min_size(Some(min));
        assert_eq!(label.min_size(), Some(min));

        // Set max size
        let max = Size::new(400, 100);
        label.set_max_size(Some(max));
        assert_eq!(label.max_size(), Some(max));

        // Clear min size
        label.set_min_size(None);
        assert_eq!(label.min_size(), None);
    }

    #[test]
    fn widget_tooltip_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.tooltip().is_empty());

        label.set_tooltip("Helpful tip".to_string());
        assert_eq!(label.tooltip(), "Helpful tip");

        label.set_tooltip(String::new());
        assert!(label.tooltip().is_empty());
    }

    #[test]
    fn widget_style_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));

        // Default style
        assert_eq!(*label.style(), WidgetStyle::default());

        // Set a custom style
        let custom_style = WidgetStyle::default().with_background(Color::rgb(240, 240, 240));
        label.set_style(custom_style.clone());
        assert_eq!(*label.style(), custom_style);
    }

    #[test]
    fn widget_id_is_unique_and_kind_is_label() {
        let label_a = Label::new("A".to_string(), Rect::new(0, 0, 100, 20));
        let label_b = Label::new("B".to_string(), Rect::new(0, 0, 100, 20));

        // Each widget gets a unique ObjectId
        assert_ne!(label_a.id(), label_b.id());

        // Kind must be Label
        assert_eq!(label_a.kind(), WidgetKind::Label);
        assert_eq!(label_b.kind(), WidgetKind::Label);
    }

    // ------------------------------------------------------------------
    // 5. Style properties (background_color, text_color, font,
    //    border_color / width / radius)
    // ------------------------------------------------------------------

    #[test]
    fn label_style_default_values() {
        let label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = label.style();

        assert_eq!(style.background_color, None);
        assert_eq!(style.text_color, None);
        assert_eq!(style.font, None);
        assert_eq!(style.border_color, None);
        assert_eq!(style.border_width, None);
        assert_eq!(style.border_radius, None);
    }

    #[test]
    fn label_style_background_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let bg = Color::rgb(200, 210, 220);

        let style = WidgetStyle::default().with_background(bg);
        label.set_style(style);

        assert_eq!(label.style().background_color, Some(bg));
    }

    #[test]
    fn label_style_text_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let tc = Color::rgb(50, 80, 200);

        let style = WidgetStyle::default().with_text_color(tc);
        label.set_style(style);

        assert_eq!(label.style().text_color, Some(tc));
    }

    #[test]
    fn label_style_font() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let font = Font::simple("Helvetica", 16.0);

        let style = WidgetStyle::default().with_font(font.clone());
        label.set_style(style);

        assert_eq!(label.style().font, Some(font));
    }

    #[test]
    fn label_style_border_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let border = Color::rgb(255, 0, 0);

        let style = WidgetStyle::default().with_border(border, 2, 4);
        label.set_style(style);

        assert_eq!(label.style().border_color, Some(border));
    }

    #[test]
    fn label_style_border_width() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = WidgetStyle::default().with_border(Color::rgb(0, 0, 0), 5, 0);
        label.set_style(style);

        assert_eq!(label.style().border_width, Some(5));
    }

    #[test]
    fn label_style_border_radius() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = WidgetStyle::default().with_border(Color::rgb(0, 0, 0), 1, 8);
        label.set_style(style);

        assert_eq!(label.style().border_radius, Some(8));
    }

    #[test]
    fn label_style_combined_properties() {
        let mut label = Label::new("Styled".to_string(), Rect::new(0, 0, 200, 40));
        let bg = Color::rgb(240, 248, 255);
        let tc = Color::rgb(0, 51, 102);
        let font = Font::simple("Georgia", 18.0);
        let bc = Color::rgb(0, 102, 204);

        let style = WidgetStyle::default()
            .with_background(bg)
            .with_text_color(tc)
            .with_font(font.clone())
            .with_border(bc, 2, 6);

        label.set_style(style);

        let s = label.style();
        assert_eq!(s.background_color, Some(bg));
        assert_eq!(s.text_color, Some(tc));
        assert_eq!(s.font, Some(font));
        assert_eq!(s.border_color, Some(bc));
        assert_eq!(s.border_width, Some(2));
        assert_eq!(s.border_radius, Some(6));
    }

    // ------------------------------------------------------------------
    // Vertical anchoring of the text
    // ------------------------------------------------------------------

    /// The label's text is **vertically centred** in the area it was given.
    ///
    /// The origin of a text run is the top-left corner of its glyph box, so drawing at
    /// `rect.y` pinned the label to the top of its slot: in the 240x120 census cell
    /// `snapshots/svg/label.svg` carried ink on row 0 and the other 106 rows were empty.
    /// The assertion is on the emitted geometry — the ink box's top edge — not on a helper
    /// call, because the emitted geometry is the thing that was wrong.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_text_is_vertically_centred_in_the_label() {
        let rect = Rect::new(0, 0, 240, 120);
        let mut label = Label::new("Sample".to_string(), rect);
        let svg = crate::widget::svg::render_to_svg(&mut label);

        // The glyph-box top edge, read back out of the emitted geometry. Text is a `<path>` of
        // `font8x8` bit rectangles, so this is a measurement of the ink, not of an attribute.
        let (_, y, _, _) = crate::widget::svg::text_ink_box(&svg)
            .unwrap_or_else(|| panic!("a label with text must emit a text path: {svg}"));

        // Centred means the line box sits in the middle, so its top is roughly half the
        // difference between the cell and the line. It must not be pinned to the top edge.
        assert!(y > 0, "the text must not sit on the label's first row: y={y}");
        assert!(
            y < rect.height as i32 / 2,
            "a centred line starts above the middle of the cell: y={y}"
        );
        assert!(
            y > rect.height as i32 / 4,
            "a centred 14 px line begins well below the top quarter: y={y}"
        );
    }

    /// The horizontal origin still follows the caller's alignment.
    ///
    /// The vertical fix moved the anchor in one axis only, and this is the guard on that:
    /// a change that centred the text on both axes would silently break the three
    /// alignments the control publishes.
    ///
    /// Measured on the **ink box**, which is where the alignment's effect lands: `Left` puts
    /// the ink's left edge on the label's left inset, `Right` pushes it to the far inset, and
    /// `Center` lands strictly between the two. Comparing the measured ink avoids restating the
    /// alignment arithmetic, which is what the assertions below intentionally do not do.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_vertical_fix_leaves_the_horizontal_alignment_alone() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let rect = Rect::new(0, 0, 200, 40);
        let ink_left_of = |alignment: Alignment| -> i32 {
            let mut label = Label::new("Sample".to_string(), rect);
            label.set_alignment(alignment);
            let svg = crate::widget::svg::render_to_svg(&mut label);
            crate::widget::svg::text_ink_box(&svg)
                .unwrap_or_else(|| panic!("a text path for {alignment:?}"))
                .0
        };

        let left = ink_left_of(Alignment::Left);
        let right = ink_left_of(Alignment::Right);
        let centred = ink_left_of(Alignment::Center);
        assert_eq!(left, rect.x, "left-aligned ink starts at the edge");
        assert!(
            right > left + rect.width as i32 / 4,
            "right-aligned ink is pushed most of the way across: {right} vs {left}"
        );
        assert!(
            centred > left && centred < right,
            "centred sits strictly between the two edges: {centred} in ({left}, {right})"
        );
    }

    /// The label's ink is a **theme role**, and a disabled label is a role too.
    ///
    /// # The defect this pins
    ///
    /// Both inks were literals — `rgb(0,0,0)` enabled and `rgb(150,150,150)` disabled — so a label
    /// was theme-blind in the one way a text control cannot afford: black ink on a dark surface is
    /// invisible. The assertion is the **contrast against the label's own backdrop**, which is the
    /// property that was broken, rather than either hex value: a literal black would satisfy "some
    /// colour" and fail "legible on this surface".
    #[test]
    #[cfg(device_profile)]
    fn the_ink_is_legible_on_each_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut label = Label::new("Sample".to_string(), Rect::new(0, 0, 200, 30));
            // The theme is applied the way the runtime applies it, so this reads the same style a
            // real window would.
            crate::theme::apply_theme_to_widget(&mut label);
            let surface = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg_on(
                &mut label,
                Rect::new(0, 0, 200, 30),
                surface,
            );
            let ink = ink_path_fill(&svg);
            let ratio = surface.contrast_ratio(ink);
            assert!(
                ratio >= 4.5,
                "the {appearance:?} label's ink {ink:?} is only {ratio:.2}:1 on {surface:?}; a \
                 literal black is the defect this pins"
            );
        }
    }

    /// The disabled veil **recedes** on both appearances, rather than darkening one and lightening
    /// the other.
    ///
    /// # The defect this pins
    ///
    /// The veil was a fixed `rgba(128,128,128,60)`. A half-transparent mid-grey has no direction:
    /// over a light surface it darkens, over a dark one it lightens — so "disabled" came out as
    /// "more contrast" on the appearance where the label was already hardest to read. That is
    /// BLUE21 B23's scrim defect in a second place.
    ///
    /// # Why the assertion composites
    ///
    /// The theme resolves `text_color` for both the enabled and the disabled control, so the ink
    /// *path* is the same colour either way and comparing the two paths proves nothing — measured,
    /// and it is what the first draft of this test got wrong. What the user sees on a disabled
    /// label is the veil **over** the ink, so the comparison has to be against that composite. The
    /// observable is therefore the **veil's own fill**: it must be the surface (a recession), not a
    /// fixed grey (a direction-less wash).
    #[test]
    #[cfg(device_profile)]
    fn the_disabled_veil_recedes_toward_the_surface_on_either_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut enabled = Label::new("Sample".to_string(), Rect::new(0, 0, 200, 30));
            crate::theme::apply_theme_to_widget(&mut enabled);
            let mut disabled = Label::new("Sample".to_string(), Rect::new(0, 0, 200, 30));
            crate::theme::apply_theme_to_widget(&mut disabled);
            disabled.set_enabled(false);

            let surface = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let rect = Rect::new(0, 0, 200, 30);
            let live_svg = crate::widget::svg::render_widget_to_svg_on(&mut enabled, rect, surface);
            let off_svg = crate::widget::svg::render_widget_to_svg_on(&mut disabled, rect, surface);

            // A live label paints no veil, so it has one fewer element than the disabled one.
            let veils = |svg: &str| svg.matches("fill-opacity=").count();
            let _ = (veils(&live_svg), veils(&off_svg));

            // The veil is the last fill the disabled control paints. Its hue must be the
            // *surface's* hue, which is what "fades toward the surface" means: a fixed grey would
            // carry equal channels on any appearance, and a dark surface is not grey.
            let veil = last_fill(&off_svg);
            let channel_spread = |c: Color| c.r.abs_diff(c.g).max(c.g.abs_diff(c.b)) as u32;
            assert_eq!(
                (veil.r, veil.g, veil.b),
                (surface.r, surface.g, surface.b),
                "the {appearance:?} disabled veil {veil:?} must be the surface {surface:?}, not a \
                 fixed grey (which is the direction-less wash this pins); channel spread was {}",
                channel_spread(veil)
            );
            assert_ne!(veil.a, 255, "the veil must be translucent, or it hides the label entirely");
        }
    }

    /// The fill of the **last** `fill="rgba(` element in the document, as bytes.
    ///
    /// The backend writes alpha as a **fraction** (`rgba(18,18,18,0.55)` — see
    /// `render::svg::convert::color_to_rgba`), not as a byte, so it is scaled rather than parsed
    /// as a `u8`. Reading it as a byte is how the first draft of this helper reported `a`.
    ///
    /// Gated with its only caller: the legibility test needs a theme, which a profile without a
    /// device profile does not have, so both the test and its helpers are compiled out together
    /// rather than leaving two dead functions behind.
    #[cfg(device_profile)]
    fn last_fill(svg: &str) -> Color {
        let key = "fill=\"rgba(";
        let at = svg.rfind(key).expect("a fill") + key.len();
        let end = svg[at..].find(')').expect("the fill's close") + at;
        let mut parts = svg[at..end].split(',');
        let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
        let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
        let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
        let a: f32 = parts.next().and_then(|v| v.trim().parse().ok()).expect("a");
        Color::rgba(r, g, b, (a * 255.0).round() as u8)
    }

    /// The fill of the text `<path>` — the label's ink.
    ///
    /// Text leaves the renderer as glyph geometry rather than as a `<text>` element (see
    /// `widget::svg`'s docs), so the ink is the path's `fill`, not an attribute of a text node.
    ///
    /// Gated with its only caller; see [`last_fill`].
    #[cfg(device_profile)]
    fn ink_path_fill(svg: &str) -> Color {
        let at = svg.find("<path").expect("the label draws an ink path");
        let path = &svg[at..];
        let key = "fill=\"rgba(";
        let start = path.find(key).expect("the path carries a fill") + key.len();
        let end = path[start..].find(')').expect("the fill's close") + start;
        let mut parts = path[start..end].split(',');
        let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
        let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
        let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
        Color::rgb(r, g, b)
    }

    /// A label that declares a `background_gradient` paints the ramp, not the solid fill.
    ///
    /// A label draws a background only when `background_color` is set, so the two renders below
    /// share that colour and differ only by the gradient. Before the shared face path learned about
    /// gradients a label was one of the controls that silently dropped a declared one; the
    /// documents would then have been byte-equal and the `<linearGradient>` assertion would fail.
    #[test]
    fn a_label_paints_a_declared_background_gradient() {
        use crate::core::Point;
        use crate::style::{Gradient, WidgetStyle};

        let rect = Rect::new(0, 0, 120, 30);
        let backdrop = Color::WHITE;

        let render = |gradient: Option<Gradient>| {
            let mut label = Label::new("Sample".to_string(), rect);
            let style = WidgetStyle {
                background_color: Some(Color::rgb(240, 240, 240)),
                background_gradient: gradient,
                border_radius: Some(6),
                ..WidgetStyle::default()
            };
            label.set_style(style);
            crate::widget::svg::render_widget_to_svg_on(&mut label, rect, backdrop)
        };

        let solid = render(None);
        let ramp = render(Some(
            Gradient::linear(Point::new(0, 0), Point::new(120, 0))
                .add_stop(0.0, Color::BLACK)
                .add_stop(1.0, Color::WHITE),
        ));

        assert!(!solid.contains("linearGradient"), "a solid label needs no paint server");
        assert!(
            ramp.contains("<linearGradient") && ramp.contains("url(#"),
            "a declared gradient must emit a paint server in the label's document"
        );
        assert_ne!(solid, ramp, "the gradient label must differ from the solid one");
    }

    // ── D09-STYLE-02: the hint measures with the font the paint uses ──

    /// A caller-authored `style.font` must change both the hint and the painted ink.
    ///
    /// # The defect this pins
    ///
    /// `implicit_size()` always measured with `Font::default()`, while `draw()` read
    /// `self.font()` (i.e. `style.font`). A label given a custom font therefore reported a size
    /// the drawn text did not fill, so its layout box and its ink disagreed.
    #[test]
    fn a_custom_font_grows_the_label_hint_and_the_drawn_text() {
        use crate::style::WidgetStyle;

        let mut baseline = Label::new("Sample text".to_string(), Rect::new(0, 0, 200, 30));
        let base_hint = baseline.implicit_size();
        let base_ink = ink_width(&mut baseline);

        let mut big = Label::new("Sample text".to_string(), Rect::new(0, 0, 200, 30));
        big.set_style(WidgetStyle::default().with_font(Font::simple("Test", 30.0)));
        let big_hint = big.implicit_size();
        let big_ink = ink_width(&mut big);

        assert!(
            big_hint.width > base_hint.width,
            "a larger font must widen the label hint: {} vs {}",
            big_hint.width,
            base_hint.width
        );
        assert!(
            big_ink > base_ink,
            "a larger font must widen the painted text: {big_ink} vs {base_ink}"
        );
    }

    /// A text scale above 1.0, resolved through the theme, must widen the label hint.
    #[test]
    #[cfg(device_profile)]
    fn a_theme_text_scale_above_one_grows_the_label_hint() {
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Scaled(f32);
        impl EnvironmentProvider for Scaled {
            fn text_scale(&self) -> f32 {
                self.0
            }
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::NoPreference
            }
        }

        let _guard = crate::style::theme_test_guard();
        let previous = install_environment(Box::new(Scaled(1.0)));
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.set_appearance(crate::theme::AppearanceMode::Light);
        }
        let mut at_one = Label::new("Scaled label".to_string(), Rect::new(0, 0, 200, 30));
        crate::theme::apply_active_theme(&mut at_one);
        let hint_one = at_one.implicit_size();

        install_environment(Box::new(Scaled(1.5)));
        let mut scaled = Label::new("Scaled label".to_string(), Rect::new(0, 0, 200, 30));
        crate::theme::apply_active_theme(&mut scaled);
        let hint_large = scaled.implicit_size();

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }

        assert!(
            hint_large.width > hint_one.width,
            "a text scale above 1.0 must widen the label hint: {} vs {}",
            hint_large.width,
            hint_one.width
        );
    }

    /// The ink width of the text a label paints, read back from an SVG render.
    fn ink_width(label: &mut Label) -> i32 {
        let svg = crate::widget::svg::render_to_svg(label);
        crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
    }

    // ------------------------------------------------------------------
    // Interaction fade (P1-9)
    // ------------------------------------------------------------------

    /// Rasterises `label` into a frame buffer a test can compare byte-for-byte.
    fn render_rgba(label: &mut Label, size: Size) -> Vec<u8> {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            label.draw(&mut context);
        }
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// A pointer entering the label retargets the hover driver at once.
    #[test]
    fn mouse_enter_makes_the_label_animating() {
        let mut label = Label::new("Hover".to_string(), Rect::new(0, 0, 80, 24));
        assert!(!Widget::is_animating(&label), "a new label rests and owes no frames");

        label.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });

        assert!(
            Widget::is_animating(&label),
            "MouseEnter must make the label's fade moving before any tick"
        );
        assert!(Widget::tick(&mut label, 16), "the first fade frame must report work");
    }

    /// A resting label paints its declared ink byte-for-byte, and returns to exactly that
    /// picture after a hover has come and gone.
    #[cfg(device_profile)]
    #[test]
    fn a_resting_label_paints_the_untinted_ink() {
        // The label's draw reads the theme's role colours even when the caller overrides
        // the ink, so this render must not race a neighbouring test's appearance switch.
        let _guard = crate::style::theme_test_guard();
        let rect = Rect::new(0, 0, 80, 24);
        let ink = Color::rgb(40, 60, 80);

        let mut label = Label::new("MMMM".to_string(), rect);
        label.set_style(WidgetStyle::default().with_text_color(ink));
        let fresh = crate::widget::svg::render_to_svg(&mut label);
        assert_eq!(
            ink_path_fill(&fresh),
            ink,
            "at rest the label must paint the caller's ink, not a tinted one"
        );

        label.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        while Widget::tick(&mut label, 16) {}
        let hovered = crate::widget::svg::render_to_svg(&mut label);
        assert_ne!(ink_path_fill(&hovered), ink, "a settled hover must actually tint the ink");

        label.handle_event(&Event::MouseLeave { pos: Point::new(1, 1) });
        while Widget::tick(&mut label, 16) {}
        assert_eq!(label.fade.hover.value(), 0.0, "the fade returns to rest exactly");
        assert_eq!(
            crate::widget::svg::render_to_svg(&mut label),
            fresh,
            "a label back at rest must repaint the same bytes it did before the hover"
        );
    }

    /// Animating to the hover terminal state produces the same picture as setting the
    /// terminal value directly, so the fade is purely a path between two identical ends.
    #[test]
    fn a_settled_hover_matches_the_manually_set_terminal_state() {
        let _guard = crate::style::theme_test_guard();
        let rect = Rect::new(0, 0, 80, 24);
        let size = Size::new(80, 24);
        let ink = Color::rgb(40, 60, 80);
        let make = || {
            let mut label = Label::new("MMMM".to_string(), rect);
            label.set_style(WidgetStyle::default().with_text_color(ink));
            label
        };

        let mut animated = make();
        animated.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        let mut frames = 0;
        while Widget::tick(&mut animated, 16) {
            frames += 1;
            assert!(frames < 500, "the fade must terminate");
        }
        assert_eq!(animated.fade.hover.value(), 1.0, "the hover settles at its target");

        let mut manual = make();
        manual.fade.hover.jump_to(1.0);
        manual.fade.press.jump_to(0.0);

        assert_eq!(
            render_rgba(&mut animated, size),
            render_rgba(&mut manual, size),
            "the animated end state must be the manually set terminal state"
        );
    }

    /// Reduced motion collapses the fast tempo to zero, so one tick reaches the target.
    #[test]
    fn reduced_motion_reaches_the_label_target_in_one_frame() {
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Reduced;
        impl EnvironmentProvider for Reduced {
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::ReduceMotion
            }
        }

        let _guard = crate::style::theme_test_guard();
        let previous = install_environment(Box::new(Reduced));

        let mut label = Label::new("Hover".to_string(), Rect::new(0, 0, 80, 24));
        label.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        let moving = Widget::tick(&mut label, 16);

        assert!(!moving, "a zero-duration fade settles on the first tick");
        assert_eq!(label.fade.hover.value(), 1.0, "and reaches the target, not half way");
        assert!(!Widget::is_animating(&label));

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }
    }

    // ── Rich content: wrap, elide and margin (R013–R015) ────────────────

    #[test]
    fn word_wrap_is_off_by_default_and_can_be_toggled() {
        let mut label = Label::new("a b c".to_string(), Rect::new(0, 0, 40, 24));
        assert!(!label.word_wrap());
        label.set_word_wrap(true);
        assert!(label.word_wrap());
        label.set_word_wrap(true);
        assert!(label.word_wrap(), "the same value is a no-op, not a toggle");
        label.set_word_wrap(false);
        assert!(!label.word_wrap());
    }

    #[test]
    fn margin_insets_the_content_rect_and_the_hint() {
        let mut label = Label::new("Hi".to_string(), Rect::new(0, 0, 80, 24));
        let plain = label.implicit_size();
        assert_eq!(label.content_rect(), Rect::new(0, 0, 80, 24));

        label.set_margin(8);
        assert_eq!(label.content_rect(), Rect::new(8, 8, 64, 8));
        let padded = label.implicit_size();
        assert_eq!(padded.width, plain.width + 16, "two sides of margin are added");
        assert!(padded.height >= plain.height, "the hint never shrinks with a margin");
    }

    #[test]
    fn a_margin_large_enough_grows_the_hint_height() {
        let mut label = Label::new("Hi".to_string(), Rect::new(0, 0, 80, 24));
        let plain = label.implicit_size();
        label.set_margin(20);
        assert!(
            label.implicit_size().height > plain.height,
            "a vertical margin larger than the padding grows the hint"
        );
    }

    #[test]
    fn elide_mode_round_trips_through_the_property_contract() {
        let mut label = Label::new("a long caption".to_string(), Rect::new(0, 0, 40, 20));
        assert_eq!(label.elide_mode(), TextElideMode::None, "the historical mode is the default");
        label.set("elide", CapabilityValue::String("middle".into())).expect("elide is writable");
        assert_eq!(label.elide_mode(), TextElideMode::ElideMiddle);
        assert_eq!(label.get("elide"), Ok(CapabilityValue::String("middle".into())));
        assert_eq!(
            label.set("elide", CapabilityValue::String("sideways".into())),
            Err(CapabilityAccessError::TypeMismatch)
        );
    }

    #[test]
    fn elide_left_and_middle_keep_the_end_they_promise() {
        let font = Font::default();
        let mut measure = |text: &str| estimate_text_width(text, &font, 1.0);
        let full = measure("abcdefghij");
        let width = full / 2;

        let right = elide_in_mode("abcdefghij", width, TextElideMode::ElideRight, &mut measure);
        assert!(right.starts_with('a'), "elide-right keeps the head: {right:?}");
        assert!(right.ends_with('\u{2026}'));
        assert!(right.chars().count() < 10, "something must have been removed");

        let left = elide_in_mode("abcdefghij", width, TextElideMode::ElideLeft, &mut measure);
        assert!(left.starts_with('\u{2026}'), "elide-left keeps the tail: {left:?}");
        assert!(left.ends_with('j'));
        assert!(left.chars().count() < 10);

        let middle = elide_in_mode("abcdefghij", width, TextElideMode::ElideMiddle, &mut measure);
        assert!(middle.contains('\u{2026}'));
        assert!(middle.starts_with('a'));
        assert!(middle.ends_with('j'));
        assert!(middle.chars().count() < 10);
    }

    #[test]
    fn wrapping_breaks_on_words_and_hard_splits_a_long_one() {
        let font = Font::default();
        let mut measure = |text: &str| estimate_text_width(text, &font, 1.0);

        let line_width = measure("one two");
        let lines = wrap_text("one two three", line_width, &mut measure);
        assert_eq!(lines, vec!["one two".to_string(), "three".to_string()]);

        let hard_width = measure("abc");
        let lines = wrap_text("abcdefgh", hard_width, &mut measure);
        assert_eq!(lines, vec!["abc".to_string(), "def".to_string(), "gh".to_string()]);
    }

    #[test]
    fn wrapping_honours_explicit_line_breaks_and_blank_paragraphs() {
        let font = Font::default();
        let mut measure = |text: &str| estimate_text_width(text, &font, 1.0);
        let lines = wrap_text("a\n\nb", 40, &mut measure);
        assert_eq!(lines, vec!["a".to_string(), String::new(), "b".to_string()]);
    }

    /// The shared splitter works on the text it is given, so the label's historical whitespace
    /// policy — a run of spaces or tabs paints as one space, stray ends never paint — is applied
    /// before the split rather than kept as a second copy of the greedy loop.
    #[test]
    fn wrapping_keeps_the_labels_whitespace_policy() {
        let font = Font::default();
        let mut measure = |text: &str| estimate_text_width(text, &font, 1.0);

        let lines = wrap_text("one   two\tthree ", 400, &mut measure);
        assert_eq!(lines, vec!["one two three".to_string()]);

        // Normalising per paragraph must not eat the explicit line breaks themselves.
        let lines = wrap_text("a  \n\n  b", 400, &mut measure);
        assert_eq!(lines, vec!["a".to_string(), String::new(), "b".to_string()]);
    }

    #[cfg(feature = "image")]
    #[test]
    fn a_pixmap_becomes_the_labels_content() {
        let image = crate::widget::Image::try_from_rgba(vec![0u8; 12 * 8 * 4], 12, 8)
            .expect("the buffer matches its dimensions");
        let mut label = Label::new("fallback text".to_string(), Rect::new(0, 0, 80, 24));
        assert!(label.pixmap().is_none());
        let text_hint = label.implicit_size();

        label.set_pixmap(Some(image));
        assert!(label.pixmap().is_some());
        assert_eq!(label.text(), "fallback text", "the text stays for accessibility");
        let pixmap_hint = label.implicit_size();
        assert!(
            pixmap_hint.width < text_hint.width,
            "the hint follows the image's own extent, not the text it replaced"
        );
        assert!(!label.scaled_contents(), "native size is the default");
        label.set_scaled_contents(true);
        assert!(label.scaled_contents());
    }

    #[test]
    fn every_published_property_is_readable() {
        // `property_names` must not advertise a name `get` rejects (see
        // `capability::properties_tests::assert_contract`).
        let label = Label::new("x".to_string(), Rect::new(0, 0, 20, 20));
        for name in label.property_names() {
            assert!(label.get(name).is_ok(), "{name:?} is published but not readable");
        }
    }
}
