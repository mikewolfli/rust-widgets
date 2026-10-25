//! `ChartTooltip` — a floating, chart-positioned tooltip bubble.

use crate::compat::{String, Vec};
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal};
use crate::widget::capability::coercion::{expect_bool, expect_string_list};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{clamp_axis, expect_point, point_value, surface_ink};

// ---------------------------------------------------------------------------
// COMP-08 ChartTooltip
// ---------------------------------------------------------------------------

/// One line of chart tooltip content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooltipLine {
    /// The text drawn on this line.
    pub text: String,
    /// Optional per-line colour (a series colour). `None` uses the bubble's ink.
    pub color: Option<Color>,
}

impl TooltipLine {
    /// Creates an ink-coloured line.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), color: None }
    }
    /// Creates a line painted in `color`.
    pub fn colored(text: impl Into<String>, color: Color) -> Self {
        Self { text: text.into(), color: Some(color) }
    }
}

/// A free-floating tooltip that a chart positions at a data point.
///
/// The hover source is the chart: it calls [`Self::show_at`] from its own
/// `point_hover_changed`/`point_hovered` handler, supplying the anchor and the complete
/// series/value/time text. With [`Self::set_follow_pointer`] enabled the bubble also follows
/// pointer motion between hovers, which is what a dense line chart needs.
pub struct ChartTooltip {
    base: BaseWidget,
    anchor: Point,
    offset: Point,
    follow_pointer: bool,
    content: Vec<TooltipLine>,
    visible: bool,
    /// Emitted when the bubble becomes visible.
    pub shown: GenericSignal,
    /// Emitted when the bubble is hidden.
    pub hidden: GenericSignal,
}

impl ChartTooltip {
    /// Creates a hidden tooltip.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ChartTooltip, geometry, "ChartTooltip"),
            anchor: Point::new(0, 0),
            offset: Point::new(12, 18),
            follow_pointer: true,
            content: Vec::new(),
            visible: false,
            shown: GenericSignal::new(),
            hidden: GenericSignal::new(),
        }
    }

    /// Shows the bubble at `anchor` with `content`.
    pub fn show_at(&mut self, anchor: Point, content: Vec<TooltipLine>) {
        self.anchor = anchor;
        self.content = content;
        let was_visible = self.visible;
        self.visible = true;
        self.base.request_redraw();
        if !was_visible {
            self.shown.emit();
        }
    }

    /// Hides the bubble.
    pub fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.base.request_redraw();
        self.hidden.emit();
    }

    /// Returns whether the bubble is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns the anchor point.
    pub fn anchor(&self) -> Point {
        self.anchor
    }

    /// Moves the anchor without changing the content or visibility.
    pub fn set_anchor(&mut self, anchor: Point) {
        if self.anchor != anchor {
            self.anchor = anchor;
            self.base.request_redraw();
        }
    }

    /// Returns the offset applied to the anchor.
    pub fn offset(&self) -> Point {
        self.offset
    }

    /// Sets the offset applied to the anchor.
    pub fn set_offset(&mut self, offset: Point) {
        if self.offset != offset {
            self.offset = offset;
            self.base.request_redraw();
        }
    }

    /// Enables or disables pointer following.
    pub fn set_follow_pointer(&mut self, follow_pointer: bool) {
        self.follow_pointer = follow_pointer;
    }

    /// Returns whether the bubble follows the pointer.
    pub fn follow_pointer(&self) -> bool {
        self.follow_pointer
    }

    /// Replaces the content of a visible (or hidden) bubble.
    pub fn set_content(&mut self, content: Vec<TooltipLine>) {
        self.content = content;
        self.base.request_redraw();
    }

    /// Returns the current content.
    pub fn content(&self) -> &[TooltipLine] {
        &self.content
    }

    /// The bubble's rectangle, derived from measured text.
    fn bubble_rect(&self, context: &RenderContext, font: &crate::core::Font) -> Rect {
        let mut width = 40;
        let mut height = 8;
        for line in &self.content {
            let metrics = context.measure_text(&line.text, font);
            width = width.max(metrics.width + 16);
            height += metrics.height.max(1) + 2;
        }
        let rect = self.geometry();
        let origin = self.anchor + (self.offset.x, self.offset.y);
        let max_x = rect.x + rect.width as i32 - width as i32;
        let max_y = rect.y + rect.height as i32 - height as i32;
        Rect::new(
            clamp_axis(origin.x, rect.x, max_x),
            clamp_axis(origin.y, rect.y, max_y),
            width,
            height,
        )
    }
}

impl EventHandler for ChartTooltip {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                if self.visible && self.follow_pointer {
                    self.set_anchor(*pos);
                }
            }
            Event::MouseLeave { .. } => {
                if self.follow_pointer {
                    self.hide();
                }
            }
            Event::KeyPress { key: 27, .. } => self.hide(),
            _ => {}
        }
    }
}

impl Draw for ChartTooltip {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible || self.content.is_empty() {
            // A fresh tooltip paints its resting chip, so the rendering census can see the
            // control and its theme response before anything is shown.
            let rect = self.geometry();
            let style = self.style();
            let (surface, ink) = surface_ink(style);
            context.fill_rect(rect, surface.with_alpha(40));
            context.draw_rect(rect, surface.blend(&ink, 0.20));
            return;
        }
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        let bubble = self.bubble_rect(context, font);
        // A tooltip is deliberately the inverse of the panel it floats over, so it reads as an
        // overlay in both appearances without a second literal palette.
        let fill = ink.blend(&surface, 0.10);
        let shadow = Rect::new(bubble.x + 2, bubble.y + 2, bubble.width, bubble.height);
        context.fill_rect(shadow, Color::BLACK.with_alpha(40));
        context.fill_rect(bubble, fill);
        context.draw_rect(bubble, fill.contrast_color().with_alpha(50));
        let line_height = bubble.height.div_ceil(self.content.len() as u32);
        for (index, line) in self.content.iter().enumerate() {
            let row = Rect::new(
                bubble.x + 8,
                bubble.y + 4 + index as u32 as i32 * line_height as i32,
                bubble.width.saturating_sub(16),
                line_height,
            );
            let color = line.color.unwrap_or_else(|| fill.contrast_color());
            context.draw_text_fitted(
                row,
                &line.text,
                font,
                color,
                crate::core::HorizontalAlignment::Left,
            );
        }
    }
}

impl Widget for ChartTooltip {
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
        Size::new(220, 64)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "shown" => Some(EventSignalRef::unit("shown", &self.shown)),
            "hidden" => Some(EventSignalRef::unit("hidden", &self.hidden)),
            _ => None,
        }
    }
}

impl WidgetProperties for ChartTooltip {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "anchor" => Ok(point_value(self.anchor)),
            "offset" => Ok(point_value(self.offset)),
            "follow_pointer" => Ok(CapabilityValue::Bool(self.follow_pointer)),
            "content" => Ok(CapabilityValue::Tuple(
                self.content
                    .iter()
                    .map(|line| CapabilityValue::String(line.text.clone()))
                    .collect(),
            )),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "anchor" => {
                self.set_anchor(expect_point(value)?);
                Ok(())
            }
            "offset" => {
                self.set_offset(expect_point(value)?);
                Ok(())
            }
            "follow_pointer" => {
                self.set_follow_pointer(expect_bool(value)?);
                Ok(())
            }
            "content" => {
                // The property form carries text only; per-line colours stay available through
                // `TooltipLine::colored` and `show_at`.
                let texts = expect_string_list(value)?;
                self.set_content(texts.into_iter().map(TooltipLine::new).collect());
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["anchor", "offset", "follow_pointer", "content", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showing_a_bubble_publishes_it_and_hiding_clears_it() {
        let mut tooltip = ChartTooltip::new(Rect::new(0, 0, 240, 160));
        assert!(!tooltip.is_visible());
        tooltip.show_at(
            Point::new(20, 30),
            crate::compat::Vec::from([TooltipLine::new("AAPL 182.10")]),
        );
        assert!(tooltip.is_visible());
        assert_eq!(tooltip.anchor(), Point::new(20, 30));
        assert_eq!(tooltip.content().len(), 1);
        assert_eq!(tooltip.content()[0].text, "AAPL 182.10");
        tooltip.hide();
        assert!(!tooltip.is_visible());
    }
}
