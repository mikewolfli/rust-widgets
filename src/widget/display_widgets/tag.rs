// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tag — a compact, tone-coloured label used to categorise or filter content.
//!
//! # Why a tag and not a `Chip`
//!
//! A chip is a **list** control: it owns a collection of items, a focus index and a selection
//! model in one of two modes. A tag is a single label attached to the row it sits in: it
//! carries a semantic tone (neutral / primary / success / warning / error), it can be
//! selected, and it can be closed. Modelling one tag as a one-item chip would give every tag
//! an item list, a focus index and a multi-select flag it can never use.

use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{estimate_line_height, estimate_text_width};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The semantic tone of a tag: which of the theme's status colours it is painted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagTone {
    /// The neutral grey, for ordinary labels.
    #[default]
    Neutral,
    /// The brand colour, for the primary categorisation.
    Primary,
    /// The success colour.
    Success,
    /// The warning colour.
    Warning,
    /// The error colour, for a tag the user must notice.
    Error,
}

impl TagTone {
    /// Every tone, in the order they are published.
    pub const ALL: [Self; 5] =
        [Self::Neutral, Self::Primary, Self::Success, Self::Warning, Self::Error];

    /// The token this tone is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Primary => "primary",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tone| tone.token() == token)
    }
}

/// The horizontal padding inside a tag, in logical pixels.
const TAG_PADDING: u32 = 8;
/// The gap between the label and the close glyph.
const TAG_CLOSE_GAP: u32 = 6;
/// The width of the close glyph's own box.
const TAG_CLOSE_SIDE: u32 = 8;

/// Tag — a single tone-coloured label with an optional close glyph.
///
/// The tag reports its own text as its accessible label, and the close glyph is drawn only
/// when `closable` is set — the two are separate properties so a caller can present a tag the
/// user can select but not remove.
pub struct Tag {
    base: BaseWidget,
    text: String,
    tone: TagTone,
    selected: bool,
    closable: bool,
    /// Emitted when the close glyph is activated, with the tag's own text as the payload.
    pub closed: Signal1<String>,
}

impl Tag {
    /// Creates a neutral, unselected, non-closable tag labelled `text`.
    pub fn new(text: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Tag, geometry, "Tag"),
            text: text.to_string(),
            tone: TagTone::default(),
            selected: false,
            closable: false,
            closed: Signal1::new(),
        }
    }

    /// The tag's label.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the label.
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The tag's semantic tone.
    pub fn tone(&self) -> TagTone {
        self.tone
    }

    /// Sets the tag's semantic tone.
    ///
    /// Re-resolves the theme style because the tone selects a different colour role, and a
    /// redraw alone would keep the previously resolved style.
    pub fn set_tone(&mut self, tone: TagTone) {
        if self.tone == tone {
            return;
        }
        self.tone = tone;
        crate::style::reapply_active_theme_state(self);
        self.base.request_redraw();
    }

    /// Whether the tag is selected.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Sets whether the tag is selected.
    pub fn set_selected(&mut self, selected: bool) {
        if self.selected == selected {
            return;
        }
        self.selected = selected;
        crate::style::reapply_active_theme_state(self);
        self.base.request_redraw();
    }

    /// Whether the close glyph is drawn and accepted.
    pub fn is_closable(&self) -> bool {
        self.closable
    }

    /// Sets whether the close glyph is drawn and accepted.
    pub fn set_closable(&mut self, closable: bool) {
        if self.closable == closable {
            return;
        }
        self.closable = closable;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle the close glyph occupies, or `None` when the tag is not closable.
    ///
    /// The press test and the painter both call this, so the glyph the user sees and the
    /// target that responds cannot drift apart.
    fn close_rect(&self) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        let geometry = self.geometry();
        let side = TAG_CLOSE_SIDE.min(geometry.height).min(geometry.width);
        let x = geometry
            .x
            .saturating_add(geometry.width as i32)
            .saturating_sub(TAG_PADDING as i32)
            .saturating_sub(side as i32);
        let y = geometry.y.saturating_add(((geometry.height - side) / 2) as i32);
        Some(Rect::new(x, y, side, side))
    }

    /// The tone colour this tag resolves from the theme, falling back to its own style.
    fn tone_color(&self, style: &crate::style::WidgetStyle) -> Color {
        let (primary, success, warning, error, outline) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(theme) => (
                    theme.colors.primary,
                    theme.colors.success,
                    theme.colors.warning,
                    theme.colors.error,
                    theme.colors.outline,
                ),
                None => (
                    Color::PRIMARY,
                    Color::rgb(46, 160, 67),
                    Color::rgb(210, 153, 34),
                    Color::rgb(207, 34, 46),
                    Color::rgb(160, 160, 160),
                ),
            }
        };
        match self.tone {
            TagTone::Neutral => style.border_color.unwrap_or(outline),
            TagTone::Primary => primary,
            TagTone::Success => success,
            TagTone::Warning => warning,
            TagTone::Error => error,
        }
    }
}

impl Widget for Tag {
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
        // The label's extent comes from the shared measurement primitives, not a hand-rolled
        // `chars * size`: the flat model this replaced was only correct while every cluster
        // advanced exactly one point size, and diverged the moment the shaper knew more
        // (kerning, wide glyphs, an explicit line height). The font is resolved exactly as
        // `draw` resolves it, so the hint and the painting agree on the same face.
        let font = self.base.style().font.clone().unwrap_or_else(|| Font::simple("Arial", 12.0));
        let label = estimate_text_width(&self.text, &font, 1.0);
        let close = if self.closable { TAG_CLOSE_GAP + TAG_CLOSE_SIDE } else { 0 };
        Size::new(
            label.saturating_add(2 * TAG_PADDING).saturating_add(close),
            estimate_line_height(&font, 1.0).saturating_add(8),
        )
    }

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "closed" => Some(EventSignalRef::mapped("closed", &self.closed, |value| {
                CapabilityValue::String(value.clone())
            })),
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Tag {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text.clone())),
            "tone" => Ok(CapabilityValue::String(self.tone.token().to_string())),
            "selected" => Ok(CapabilityValue::Bool(self.selected)),
            "closable" => Ok(CapabilityValue::Bool(self.closable)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "tone" => {
                let token = expect_string(value)?;
                let tone = TagTone::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_tone(tone);
                Ok(())
            }
            "selected" => {
                self.set_selected(expect_bool(value)?);
                Ok(())
            }
            "closable" => {
                self.set_closable(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "tone", "selected", "closable", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Tag {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let font = style.font.clone().unwrap_or_else(|| Font::simple("Arial", 12.0));
        let surface = style.background_color.unwrap_or_else(|| {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|theme| theme.colors.background).unwrap_or(Color::WHITE)
        });
        let tone = self.tone_color(style);
        // A selected tag is filled with its tone; an unselected one is a tint of it, so the
        // pair is distinguishable in both appearances without a second hardcoded colour.
        let (fill, ink) = if self.selected {
            (tone, tone.contrast_color())
        } else {
            // Both terms come from the theme, so the tint moves towards the surface in light
            // mode and away from it in dark mode. The ink is whatever is legible on the
            // resulting fill, which is what keeps the label readable in both appearances.
            let fill = tone.blend(&surface, 0.86);
            (fill, fill.contrast_color())
        };
        let radius = rect.height / 2;
        ctx.fill_rounded_rect(rect, radius, fill);
        ctx.draw_rounded_rect_stroke(rect, radius, tone.with_alpha_f32(0.7), 1);

        let close = self.close_rect();
        let label_right = close.map_or(rect.width, |close| {
            rect.width.saturating_sub(
                close.width.saturating_add(TAG_CLOSE_GAP).saturating_add(TAG_PADDING),
            )
        });
        let label_band =
            Rect::new(rect.x.saturating_add(TAG_PADDING as i32), rect.y, label_right, rect.height);
        // Bounded text: a long label is fitted with an ellipsis rather than running past the
        // tag (and, in the SVG backend, past the control's own rectangle).
        ctx.draw_text_line(label_band, &self.text, &font, ink, HorizontalAlignment::Left);

        if let Some(close) = close {
            // An "x" drawn as two strokes rather than a glyph: a font-dependent glyph would
            // change the control's ink the moment the theme changed typeface.
            let inset = close.width / 4;
            let from = crate::core::Point::new(
                close.x.saturating_add(inset as i32),
                close.y.saturating_add(inset as i32),
            );
            let to = crate::core::Point::new(
                close.x.saturating_add(close.width as i32).saturating_sub(inset as i32),
                close.y.saturating_add(close.height as i32).saturating_sub(inset as i32),
            );
            ctx.draw_line_stroke(from, to, ink, 1);
            ctx.draw_line_stroke(
                crate::core::Point::new(from.x, to.y),
                crate::core::Point::new(to.x, from.y),
                ink,
                1,
            );
        }
    }
}

impl EventHandler for Tag {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some(close) = self.close_rect() {
                if close.contains(*pos) {
                    self.closed.emit(self.text.clone());
                    self.base.request_redraw();
                    return;
                }
            }
            if self.geometry().contains(*pos) {
                let selected = !self.selected;
                self.set_selected(selected);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn a_tag_starts_neutral_and_unselected() {
        let tag = Tag::new("rust", Rect::new(0, 0, 60, 24));
        assert_eq!(tag.tone(), TagTone::Neutral);
        assert!(!tag.is_selected());
        assert!(!tag.is_closable());
        assert_eq!(tag.kind(), WidgetKind::Tag);
    }

    #[test]
    fn a_tag_size_hint_covers_its_label_and_close_glyph() {
        let mut tag = Tag::new("abc", Rect::new(0, 0, 10, 10));
        let without_close = tag.size_hint();
        tag.set_closable(true);
        let with_close = tag.size_hint();
        assert!(with_close.width > without_close.width, "the close glyph takes room");
        assert_eq!(with_close.width - without_close.width, TAG_CLOSE_GAP + TAG_CLOSE_SIDE);
    }

    #[test]
    fn pressing_the_close_glyph_emits_closed_with_the_label() {
        let mut tag = Tag::new("draft", Rect::new(0, 0, 80, 24));
        tag.set_closable(true);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        tag.closed.connect_ref({
            let seen = std::sync::Arc::clone(&seen);
            move |value: &String| seen.lock().expect("lock").push(value.clone())
        });
        let close = tag.close_rect().expect("a closable tag has a close glyph");
        tag.handle_event(&Event::MousePress {
            pos: Point::new(close.x + 1, close.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(seen.lock().expect("lock").as_slice(), ["draft".to_string()]);
        assert!(!tag.is_selected(), "closing must not also toggle selection");
    }

    #[test]
    fn pressing_the_body_toggles_selection() {
        let mut tag = Tag::new("rust", Rect::new(0, 0, 80, 24));
        tag.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(tag.is_selected());
        tag.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(!tag.is_selected());
    }

    #[test]
    fn the_property_route_round_trips_every_field() {
        let mut tag = Tag::new("", Rect::new(0, 0, 80, 24));
        tag.set("text", CapabilityValue::String("beta".to_string())).expect("writable");
        tag.set("tone", CapabilityValue::String("warning".to_string())).expect("writable");
        tag.set("selected", CapabilityValue::Bool(true)).expect("writable");
        tag.set("closable", CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(tag.get("text"), Ok(CapabilityValue::String("beta".to_string())));
        assert_eq!(tag.get("tone"), Ok(CapabilityValue::String("warning".to_string())));
        assert_eq!(tag.get("selected"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(tag.get("closable"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(
            tag.set("tone", CapabilityValue::String("chartreuse".to_string())),
            Err(CapabilityAccessError::OutOfRange)
        );
    }

    #[test]
    fn a_tag_paints_its_label() {
        let mut tag = Tag::new("hello", Rect::new(0, 0, 80, 24));
        let svg = crate::widget::svg::render_to_svg(&mut tag);
        assert_eq!(crate::widget::svg::text_ink_boxes(&svg).len(), 1, "{svg}");
    }
}
