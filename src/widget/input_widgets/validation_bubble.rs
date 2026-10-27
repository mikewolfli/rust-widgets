// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ValidationBubble` — a validation message anchored beside the field it accuses (COMP-B02).
//!
//! # Why not a toast or a tooltip
//!
//! A toast lives in a corner; a tooltip follows the pointer. Neither says "**this** field is
//! wrong", which is the one thing a validation message has to say. The input family's feedback
//! had no such carrier: `line_edit` has an error row, `masked_edit` reports a rejected cell index
//! with no text, and `editable_combo_box` reports that editing finished with nothing to show.
//! This companion is the field-anchored form, and it is what
//! [`Validator::message_for`](crate::widget::input_widgets::input_validator::Validator::message_for)
//! gives the host something specific to put in it.
//!
//! Like the other companions it is mounted beside the control it serves and is not in the
//! capability table.

use crate::compat::{String, ToString};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal};
use crate::widget::capability::coercion::{expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::popup_support::{clamp_rect_into, expect_rect, rect_value};
use crate::widget::metrics::effective_font;
use crate::widget::text_utils::wrap_char_ranges;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// How far the bubble floats above its anchor, leaving room for the pointer triangle.
const BUBBLE_GAP: i32 = 8;
/// The pointer triangle's half-width.
const POINTER_HALF: i32 = 5;
/// Text inset inside the bubble.
const BUBBLE_INSET: i32 = 8;
/// Vertical padding inside the bubble.
const BUBBLE_PAD_V: i32 = 5;

/// How serious a validation message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidationSeverity {
    /// A neutral note about the value.
    Info,
    /// The value is worth a second look but is accepted.
    Warning,
    /// The value is refused.
    #[default]
    Error,
}

impl ValidationSeverity {
    /// The published token for this severity.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// The severity a published token names, or `None` for an unknown token.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    /// The semantic colour of this severity, following the toast host's rule.
    fn ink(self) -> Color {
        use crate::style::{semantic_color, SemanticColor};
        match self {
            Self::Info => semantic_color(SemanticColor::Info).unwrap_or(Color::PRIMARY),
            Self::Warning => semantic_color(SemanticColor::Warning).unwrap_or(Color::WARNING),
            Self::Error => semantic_color(SemanticColor::Error).unwrap_or(Color::ERROR),
        }
    }
}

/// A message bubble anchored to a field.
pub struct ValidationBubble {
    base: BaseWidget,
    message: String,
    severity: ValidationSeverity,
    anchor: Option<Rect>,
    /// The host's available area; when set, the bubble is clamped into it on all four
    /// sides (I-125).
    viewport: Option<Rect>,
    /// `None` keeps the bubble up until it is replaced or hidden.
    show_duration_ms: Option<u32>,
    /// Milliseconds left on the current message, or `None` for a persistent one.
    remaining_ms: Option<u32>,
    visible: bool,
    /// Emitted when the bubble goes away: its timer expired, or the host hid it.
    pub bubble_dismissed: GenericSignal,
}

impl ValidationBubble {
    /// Creates a hidden bubble.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "ValidationBubble"),
            message: String::new(),
            severity: ValidationSeverity::Error,
            anchor: None,
            viewport: None,
            show_duration_ms: None,
            remaining_ms: None,
            visible: false,
            bubble_dismissed: GenericSignal::new(),
        }
    }

    /// Shows `message` at `severity`, restarting any countdown.
    pub fn show_message(&mut self, message: &str, severity: ValidationSeverity) {
        self.message.clear();
        self.message.push_str(message);
        self.severity = severity;
        self.remaining_ms = self.show_duration_ms;
        self.visible = !self.message.is_empty();
        self.base.request_redraw();
    }

    /// The message being shown.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The severity being shown.
    pub fn severity(&self) -> ValidationSeverity {
        self.severity
    }

    /// Anchors the bubble to the field it accuses.
    pub fn set_anchor(&mut self, anchor: Rect) {
        if self.anchor == Some(anchor) {
            return;
        }
        self.anchor = Some(anchor);
        self.base.request_redraw();
    }

    /// The field band the bubble points at.
    pub fn anchor(&self) -> Option<Rect> {
        self.anchor
    }

    /// Supplies the host's available area.
    pub fn set_viewport(&mut self, viewport: Option<Rect>) {
        if self.viewport == viewport {
            return;
        }
        self.viewport = viewport;
        self.base.request_redraw();
    }

    /// The host's available area, if one was supplied.
    pub fn viewport(&self) -> Option<Rect> {
        self.viewport
    }

    /// Shows the current message.
    pub fn show(&mut self) {
        if self.visible || self.message.is_empty() {
            return;
        }
        self.visible = true;
        self.remaining_ms = self.show_duration_ms;
        self.base.request_redraw();
    }

    /// Hides the bubble and reports the dismissal when it was showing.
    pub fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.remaining_ms = None;
        self.base.request_redraw();
        self.bubble_dismissed.emit();
    }

    /// Whether the bubble is showing.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Sets (or clears, with `None`) the auto-hide delay.
    pub fn set_show_duration(&mut self, duration_ms: Option<u32>) {
        if self.show_duration_ms == duration_ms {
            return;
        }
        self.show_duration_ms = duration_ms;
        // A message already on screen picks the new budget up immediately, so the host does not
        // have to re-show it to change how long it lasts.
        if self.visible {
            self.remaining_ms = duration_ms;
        }
    }

    /// The auto-hide delay, or `None` for a persistent bubble.
    pub fn show_duration(&self) -> Option<u32> {
        self.show_duration_ms
    }

    /// Advances the countdown, returning whether another frame is due.
    ///
    /// A bubble with no delay never owes a frame: a persistent message does not animate.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.visible {
            return false;
        }
        let Some(remaining) = self.remaining_ms else {
            return false;
        };
        if delta_ms >= remaining {
            self.remaining_ms = None;
            self.hide();
            return false;
        }
        self.remaining_ms = Some(remaining - delta_ms);
        true
    }

    /// The bubble's own box: wide enough for the anchor, tall enough for the wrapped message.
    fn bubble_rect(&self) -> Rect {
        let font = effective_font(self.base.style());
        let line_height = font.effective_line_height().max(1.0) as u32;
        let geometry = self.geometry();
        let width = self.anchor.map(|anchor| anchor.width).unwrap_or(geometry.width);
        let lines = self.wrapped_lines(font, width).len().max(1) as u32;
        let height = line_height.saturating_mul(lines).saturating_add(BUBBLE_PAD_V as u32 * 2);
        let (x, y) = match self.anchor {
            Some(anchor) => {
                let above = anchor.y - height as i32 - BUBBLE_GAP;
                if self.viewport.is_none_or(|viewport| above >= viewport.y) {
                    (anchor.x, above)
                } else {
                    (anchor.x, anchor.y + anchor.height as i32 + BUBBLE_GAP)
                }
            }
            None => (geometry.x, geometry.y),
        };
        let rect = Rect::new(x, y, width, height);
        // The four-way clamp is what keeps a bubble anchored near the right edge or the
        // bottom of the window inside the visible area (I-125).
        match self.viewport {
            Some(viewport) => clamp_rect_into(rect, viewport),
            None => rect,
        }
    }

    /// The message split into visual lines for `width`.
    ///
    /// The split uses [`wrap_char_ranges`], which needs no renderer, so the layout box,
    /// the hit test and the paint all measure the same lines.
    fn wrapped_lines(
        &self,
        font: &crate::core::Font,
        width: u32,
    ) -> crate::compat::Vec<core::ops::Range<usize>> {
        let inner = width.saturating_sub(BUBBLE_INSET as u32 * 2);
        let char_width = (font.size().max(8.0) * 0.5).max(1.0);
        let max_chars = ((inner as f32 / char_width) as usize).max(1);
        wrap_char_ranges(&self.message, max_chars)
    }
}

impl EventHandler for ValidationBubble {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        // Any press on the bubble acknowledges it: a message the user has read should not sit over
        // the field they are trying to correct.
        if let Event::MousePress { pos, button: 1, .. } = event {
            if self.bubble_rect().contains(*pos) {
                self.hide();
            }
        }
    }
}

impl Draw for ValidationBubble {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible || self.message.is_empty() {
            return;
        }
        let bubble = self.bubble_rect();
        if bubble.width == 0 || bubble.height == 0 {
            return;
        }
        let style = self.style();
        let font = effective_font(style);
        let tone = self.severity.ink();
        // The bubble is a raised surface tinted by the severity, so the message reads on the same
        // ink the rest of the theme uses.
        let surface = style.background_color.unwrap_or_else(|| tone.blend(&Color::WHITE, 0.88));
        let ink = style.text_color.unwrap_or_else(|| surface.contrast_color());

        context.fill_rounded_rect(bubble, 4, surface);
        context.draw_rounded_rect_stroke(bubble, 4, tone, 1);

        let line = context.text_line(bubble, font);
        let lines = self.wrapped_lines(font, bubble.width);
        let inner_width = bubble.width.saturating_sub(BUBBLE_INSET as u32 * 2);
        for (row, range) in lines.iter().enumerate() {
            let y = bubble.y + BUBBLE_PAD_V + row as i32 * line.height.max(1) as i32;
            let Some(text) = self.message.get(range.clone()) else {
                continue;
            };
            context.draw_text_fitted_cow(
                Rect::new(bubble.x + BUBBLE_INSET, y, inner_width, line.height.max(1)),
                text,
                font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // The pointer: two strokes from the bubble's edge toward the anchor, drawn only when the
        // bubble floats above it (below the field the triangle would point away from the ink).
        if let Some(anchor) = self.anchor {
            if bubble.y + bubble.height as i32 <= anchor.y {
                let cx = (anchor.x + anchor.width as i32 / 2)
                    .clamp(bubble.x + POINTER_HALF, bubble.x + bubble.width as i32 - POINTER_HALF);
                let tip_y = bubble.y + bubble.height as i32;
                context.draw_line_stroke(
                    Point::new(cx - POINTER_HALF, tip_y - 1),
                    Point::new(cx, tip_y + POINTER_HALF),
                    tone,
                    1,
                );
                context.draw_line_stroke(
                    Point::new(cx, tip_y + POINTER_HALF),
                    Point::new(cx + POINTER_HALF, tip_y - 1),
                    tone,
                    1,
                );
            }
        }
    }
}

impl Widget for ValidationBubble {
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
        let bubble = self.bubble_rect();
        Size::new(bubble.width, bubble.height)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn tick(&mut self, delta_ms: u32) -> bool {
        ValidationBubble::tick(self, delta_ms)
    }
    fn is_animating(&self) -> bool {
        self.visible && self.remaining_ms.is_some()
    }
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "bubble_dismissed" => {
                Some(EventSignalRef::unit("bubble_dismissed", &self.bubble_dismissed))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ValidationBubble {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" => Ok(CapabilityValue::String(self.message.clone())),
            "severity" => Ok(CapabilityValue::String(self.severity.as_token().to_string())),
            "anchor" => match self.anchor {
                Some(anchor) => Ok(rect_value(anchor)),
                None => Ok(CapabilityValue::Null),
            },
            "show_duration_ms" => match self.show_duration_ms {
                Some(duration) => Ok(CapabilityValue::UInt(u64::from(duration))),
                None => Ok(CapabilityValue::Null),
            },
            "visible" => Ok(CapabilityValue::Bool(self.visible)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" => {
                let message = expect_string(value)?;
                let severity = self.severity;
                self.show_message(&message, severity);
                Ok(())
            }
            "severity" => {
                let token = expect_string(value)?;
                let severity = ValidationSeverity::from_token(&token)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.severity = severity;
                self.base.request_redraw();
                Ok(())
            }
            "anchor" => match value {
                CapabilityValue::Null => {
                    self.anchor = None;
                    self.base.request_redraw();
                    Ok(())
                }
                other => {
                    self.set_anchor(expect_rect(other)?);
                    Ok(())
                }
            },
            "show_duration_ms" => match value {
                CapabilityValue::Null => {
                    self.set_show_duration(None);
                    Ok(())
                }
                other => {
                    self.set_show_duration(Some(expect_u32(other)?));
                    Ok(())
                }
            },
            "visible" => {
                if matches!(value, CapabilityValue::Bool(true)) {
                    self.show();
                } else {
                    self.hide();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "message",
            "severity",
            "anchor",
            "show_duration_ms",
            "visible",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_shows_and_reports_its_severity() {
        let mut bubble = ValidationBubble::new(Rect::new(0, 0, 160, 1));
        bubble.show_message("3 is outside 1..=2", ValidationSeverity::Error);
        assert!(bubble.is_visible());
        assert_eq!(bubble.message(), "3 is outside 1..=2");
        assert_eq!(bubble.severity(), ValidationSeverity::Error);
        bubble.hide();
        assert!(!bubble.is_visible());
    }

    #[test]
    fn a_timed_message_hides_itself() {
        let mut bubble = ValidationBubble::new(Rect::new(0, 0, 160, 1));
        bubble.set_show_duration(Some(100));
        bubble.show_message("try again", ValidationSeverity::Warning);
        assert!(bubble.tick(60));
        assert!(bubble.is_visible());
        assert!(!bubble.tick(60));
        assert!(!bubble.is_visible());
    }

    #[test]
    fn a_persistent_message_never_expires() {
        let mut bubble = ValidationBubble::new(Rect::new(0, 0, 160, 1));
        bubble.show_message("still wrong", ValidationSeverity::Info);
        assert!(!bubble.tick(10_000));
        assert!(bubble.is_visible());
    }

    #[test]
    fn the_severity_tokens_round_trip() {
        for severity in
            [ValidationSeverity::Info, ValidationSeverity::Warning, ValidationSeverity::Error]
        {
            assert_eq!(ValidationSeverity::from_token(severity.as_token()), Some(severity));
        }
        assert_eq!(ValidationSeverity::default(), ValidationSeverity::Error);
    }

    #[test]
    fn the_bubble_sits_above_its_anchor_when_there_is_room() {
        let mut bubble = ValidationBubble::new(Rect::new(0, 0, 160, 1));
        bubble.set_anchor(Rect::new(10, 80, 120, 24));
        let rect = bubble.bubble_rect();
        assert!(rect.y + rect.height as i32 <= 80);
    }

    #[test]
    fn a_long_message_wraps_and_uses_more_than_one_line() {
        let mut short = ValidationBubble::new(Rect::new(0, 0, 120, 20));
        short.show_message("bad", ValidationSeverity::Error);
        let short_height = short.size_hint().height;

        let mut long = ValidationBubble::new(Rect::new(0, 0, 120, 20));
        long.show_message(
            "this message is long enough to need several wrapped lines in a narrow bubble",
            ValidationSeverity::Error,
        );
        assert!(long.size_hint().height > short_height, "wrapping must grow the bubble");
    }

    #[test]
    fn the_bubble_is_clamped_into_the_viewport_on_all_four_sides() {
        let mut bubble = ValidationBubble::new(Rect::new(0, 0, 120, 20));
        bubble.set_viewport(Some(Rect::new(0, 0, 100, 100)));
        bubble.set_anchor(Rect::new(90, 90, 80, 20));
        bubble.show_message("bad value", ValidationSeverity::Error);
        let rect = bubble.bubble_rect();
        assert!(rect.x >= 0 && rect.y >= 0);
        assert!(rect.x + rect.width as i32 <= 100, "the right edge is clamped");
        assert!(rect.y + rect.height as i32 <= 100, "the bottom edge is clamped");
    }
}
