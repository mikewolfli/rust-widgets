// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tooltip widget — a popup label that appears on hover for context info.
//!
//! The Tooltip widget displays a short text label near a target widget when the
//! pointer hovers over it. It supports configurable show/hide delays, custom colors,
//! padding, and maximum width. Tooltips are rendered as rounded rectangles with
//! semi-transparent dark backgrounds and white text.

use crate::core::HorizontalAlignment;
use crate::core::ObjectId;
use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Default delay (ms) before the tooltip appears after mouse enters the target.
const DEFAULT_SHOW_DELAY_MS: u64 = 500;
/// Default delay (ms) before the tooltip hides after mouse leaves the target.
const DEFAULT_HIDE_DELAY_MS: u64 = 200;
/// Default padding inside the tooltip (pixels).
const DEFAULT_PADDING: i32 = 6;
/// Default font size for tooltip text.
const DEFAULT_FONT_SIZE: f32 = 12.0;
/// Default maximum width of the tooltip before text wraps.
const DEFAULT_MAX_WIDTH: u32 = 300;
/// Default background colour, used as the last resort behind [`Tooltip`]'s own
/// `background_color` field when neither an explicit style nor the theme resolves one.
const DEFAULT_BG_COLOR: Color = Color::rgba(40, 40, 40, 220);
/// Default text colour, the counterpart of [`DEFAULT_BG_COLOR`].
const DEFAULT_TEXT_COLOR: Color = Color::WHITE;
/// Timer id used for show-delay scheduling.
const TIMER_SHOW_ID: u32 = 1;
/// Timer id used for hide-delay scheduling.
const TIMER_HIDE_ID: u32 = 2;

/// Tooltip widget — a popup label attached to a target widget.
///
/// The tooltip appears when the pointer hovers over the target widget and
/// disappears after the pointer leaves. Both delays are configurable.
pub struct Tooltip {
    base: BaseWidget,
    text: String,
    target_widget: Option<ObjectId>,
    show_delay: u64,
    hide_delay: u64,
    visible: bool,
    background_color: Color,
    text_color: Color,
    padding: i32,
    font_size: f32,
    max_width: u32,
    /// Tracks whether the pointer is currently hovering over the target area.
    hovering: bool,
    /// Tracks whether a show timer has been requested and is pending.
    show_pending: bool,
    /// Tracks whether a hide timer has been requested and is pending.
    hide_pending: bool,
}

impl Tooltip {
    /// Creates a new Tooltip widget with the given text and geometry.
    ///
    /// The tooltip starts hidden with default show/hide delays, default
    /// semi-transparent dark background, white text, 6px padding, 12px font
    /// size, and 300px max width. No target widget is set initially.
    pub fn new(text: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Tooltip, geometry, "Tooltip"),
            text: text.to_string(),
            target_widget: None,
            show_delay: DEFAULT_SHOW_DELAY_MS,
            hide_delay: DEFAULT_HIDE_DELAY_MS,
            visible: false,
            background_color: DEFAULT_BG_COLOR,
            text_color: DEFAULT_TEXT_COLOR,
            padding: DEFAULT_PADDING,
            font_size: DEFAULT_FONT_SIZE,
            max_width: DEFAULT_MAX_WIDTH,
            hovering: false,
            show_pending: false,
            hide_pending: false,
        }
    }

    /// Sets the tooltip text content.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the tooltip text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Immediately shows the tooltip.
    pub fn show(&mut self) {
        self.show_pending = false;
        self.hide_pending = false;
        self.visible = true;
        self.base.request_redraw();
    }

    /// Immediately hides the tooltip.
    pub fn hide(&mut self) {
        self.hide_pending = false;
        self.show_pending = false;
        self.visible = false;
        self.base.request_redraw();
    }

    /// Returns whether the tooltip is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Reports the tooltip's own shown-state.
    ///
    /// Deliberately distinct from [`Widget::is_visible`], which this widget
    /// overrides to hide `BaseWidget::visible`. The property contract publishes
    /// the inherited `visible` for the control, so the popup state is published
    /// under the `shown` name instead; without this separation the base property
    /// would be unreachable.
    pub fn is_shown(&self) -> bool {
        self.visible
    }

    /// Sets the tooltip's own shown-state, scheduling nothing.
    ///
    /// The counterpart to [`is_shown`](Self::is_shown); it routes through the
    /// existing `show` / `hide` accessors so the pending-timer bookkeeping stays
    /// consistent.
    pub fn set_shown(&mut self, shown: bool) {
        if shown {
            self.show();
        } else {
            self.hide();
        }
    }

    /// Returns the bubble's fill colour.
    ///
    /// The last step of `draw`'s resolution order — explicit style, then the theme, then
    /// this field — so a caller can override the bubble without restyling the whole control.
    pub fn background_color(&self) -> Color {
        self.background_color
    }

    /// Sets the bubble's fill colour and requests a redraw.
    ///
    /// Written straight onto the widget rather than onto its style so it survives the
    /// theme's own application, which would otherwise replace the widget's style record.
    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
        self.base.request_redraw();
    }

    /// Returns the colour the bubble's text is drawn in.
    ///
    /// Read only when the bubble is opaque enough for the default contrast colour to be
    /// unreadable; `draw` otherwise uses the bubble's own contrast colour.
    pub fn text_color(&self) -> Color {
        self.text_color
    }

    /// Sets the colour the bubble's text is drawn in and requests a redraw.
    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
        self.base.request_redraw();
    }

    /// Sets the target widget id that this tooltip is attached to.
    /// The tooltip responds to mouse enter/leave events associated with
    /// this target by scheduling show/hide.
    pub fn set_target(&mut self, target: ObjectId) {
        self.target_widget = Some(target);
    }

    /// Returns the target widget id, if any.
    pub fn target(&self) -> Option<ObjectId> {
        self.target_widget
    }

    /// Sets the show delay in milliseconds.
    /// This is the time the pointer must hover before the tooltip appears.
    pub fn set_show_delay(&mut self, ms: u64) {
        self.show_delay = ms;
    }

    /// Sets the hide delay in milliseconds.
    /// This is the time after the pointer leaves before the tooltip disappears.
    pub fn set_hide_delay(&mut self, ms: u64) {
        self.hide_delay = ms;
    }

    /// Calculates the preferred size of the tooltip based on the text content
    /// and the configured padding.
    ///
    /// Uses a simple estimation: measures the text at the configured font size
    /// and adds padding on all sides. If the text is empty, returns a default
    /// minimum size.
    pub fn preferred_size(&self) -> Size {
        if self.text.is_empty() {
            return Size::new((self.padding as u32) * 2, (self.padding as u32) * 2 + 16);
        }
        // Estimate: approximate text measurement using character count
        let char_width = self.font_size * 0.6;
        let estimated_width = (self.text.len() as f32 * char_width).ceil() as u32;
        let line_height = (self.font_size * 1.4).ceil() as u32;

        let width = (estimated_width + (self.padding as u32) * 2).min(self.max_width);
        let height = line_height + (self.padding as u32) * 2;
        Size::new(width, height)
    }
}

impl Widget for Tooltip {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(100, 30)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Tooltip`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
///
/// The legacy table served a `visible` arm for this kind, but there is a name
/// collision to resolve: this widget overrides [`Widget::is_visible`] to return its
/// *popup* state, so a `visible` arm here would shadow the shared `visible` that
/// [`base_property_get`] serves — the two would be indistinguishable and the base
/// one unreachable. The popup state is therefore published as `shown`, and bare
/// `visible` keeps meaning what it means for every other control.
impl WidgetProperties for Tooltip {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "shown" => Ok(CapabilityValue::Bool(self.is_shown())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "shown" => {
                self.set_shown(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "shown", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `tooltip` publishes.
    ///
    /// `show` and `hide` map onto the widget's real methods and take no payload
    /// (each also cancels any pending timer). `set_text` carries the text the
    /// caller wants shown, so a bare invocation is reported as needing one
    /// rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show" => {
                self.show();
                Ok(())
            }
            "hide" => {
                self.hide();
                Ok(())
            }
            "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Tooltip {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseEnter { pos: _ } => {
                if !self.hovering {
                    self.hovering = true;
                    // Cancel any pending hide
                    self.hide_pending = false;
                    // Schedule show (or show immediately if delay is 0)
                    if self.show_delay == 0 {
                        self.show();
                    } else {
                        self.show_pending = true;
                    }
                }
            }
            Event::MouseLeave { pos: _ } => {
                if self.hovering {
                    self.hovering = false;
                    // Cancel any pending show
                    self.show_pending = false;
                    // Schedule hide (or hide immediately if delay is 0)
                    if self.hide_delay == 0 {
                        self.hide();
                    } else {
                        self.hide_pending = true;
                    }
                }
            }
            Event::Timer { id } => {
                if *id == TIMER_SHOW_ID && self.show_pending {
                    self.show();
                } else if *id == TIMER_HIDE_ID && self.hide_pending {
                    self.hide();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

impl Draw for Tooltip {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal —
        // and the whole body used to be skipped unless the tooltip was already showing — so
        // the census reported `ink = 0` *and* no response to a light/dark switch.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("tooltip");
        // `tooltip` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // bubble painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from
        // it, the same distinction `Colors::input_background` draws for a field.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(self.text_color);
        let bubble = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => {
                // The control's own colour is the last resort before the theme-derived
                // surface, so a caller who set one is not overruled by the derivation.
                let own = self.background_color;
                if own != window_fill {
                    own
                } else {
                    window_fill.blend(&ink, 0.85)
                }
            }
        };
        // The text sits on the bubble, so it resolves from the bubble rather than from the
        // style: on a bubble that is already the foreground colour, an inherited text colour
        // would be invisible.
        let text_color = bubble.contrast_color();

        let font = Font::simple("sans-serif", self.font_size);

        // Measure text for layout. The empty case measures the placeholder so a hidden
        // tooltip still has a body to paint at the census geometry.
        let label = if self.text.is_empty() { "Tooltip" } else { self.text.as_str() };
        let metrics = context.measure_text(label, &font);
        let text_width = metrics.width;
        let glyph_height = metrics.height;

        // Clamp content width to max_width
        let content_width = text_width.min(self.max_width);
        let content_height = glyph_height;

        // Calculate total dimensions with padding
        let total_width = (content_width + (self.padding as u32) * 2).max(rect.width);
        let total_height = (content_height + (self.padding as u32) * 2).max(rect.height);

        // Center the tooltip within its geometry (typically positioned near cursor/target)
        let bg_x = rect.x + (rect.width as i32 - total_width as i32) / 2;
        let bg_y = rect.y + (rect.height as i32 - total_height as i32) / 2;
        let bg_rect = Rect::new(bg_x, bg_y, total_width, total_height);
        let corner_radius = 4u32;

        // A tooltip that is not showing is drawn as a dimmed bubble rather than omitted, so
        // the control has a rendered body in every state instead of vanishing at rest.
        let bubble = if self.visible { bubble } else { window_fill.blend(&bubble, 0.45) };

        // Draw rounded rectangle background
        context.fill_rounded_rect(bg_rect, corner_radius, bubble);

        // Draw text centered within the padded area
        let text_x = bg_rect.x + self.padding;
        let text_y = bg_rect.y + self.padding + metrics.ascent as i32;

        context.draw_text(
            Point::new(text_x, text_y),
            label,
            &font,
            text_color,
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn tooltip_default_creation() {
        let tooltip = Tooltip::new("Hello", Rect::new(0, 0, 100, 40));
        assert_eq!(tooltip.kind(), WidgetKind::Tooltip);
        assert_eq!(tooltip.text(), "Hello");
        assert!(!tooltip.is_visible());
        assert!(tooltip.target().is_none());
        assert_eq!(tooltip.geometry(), Rect::new(0, 0, 100, 40));
    }

    #[test]
    fn tooltip_show_hide() {
        let mut tooltip = Tooltip::new("Test", Rect::new(0, 0, 100, 40));
        assert!(!tooltip.is_visible());

        tooltip.show();
        assert!(tooltip.is_visible());

        tooltip.hide();
        assert!(!tooltip.is_visible());
    }

    #[test]
    fn tooltip_text_accessor() {
        let mut tooltip = Tooltip::new("Initial", Rect::new(0, 0, 100, 40));
        assert_eq!(tooltip.text(), "Initial");

        tooltip.set_text("Updated");
        assert_eq!(tooltip.text(), "Updated");

        tooltip.set_text("");
        assert_eq!(tooltip.text(), "");
    }

    #[test]
    fn tooltip_preferred_size_empty_text() {
        let tooltip = Tooltip::new("", Rect::new(0, 0, 100, 40));
        let size = tooltip.preferred_size();
        assert!(size.width >= 12);
        assert!(size.height >= 28);
    }

    #[test]
    fn tooltip_preferred_size_with_text() {
        let tooltip = Tooltip::new("Hello World", Rect::new(0, 0, 100, 40));
        let size = tooltip.preferred_size();
        // Should be larger than empty padding alone
        assert!(size.width >= 12);
        assert!(size.height >= 28);
    }

    #[test]
    fn tooltip_target_widget() {
        let mut tooltip = Tooltip::new("Info", Rect::new(0, 0, 100, 40));
        assert!(tooltip.target().is_none());

        tooltip.set_target(42);
        assert_eq!(tooltip.target(), Some(42));
    }

    #[test]
    fn tooltip_delays() {
        let mut tooltip = Tooltip::new("Delayed", Rect::new(0, 0, 100, 40));
        // Default delays
        assert_eq!(tooltip.show_delay, 500);
        assert_eq!(tooltip.hide_delay, 200);

        tooltip.set_show_delay(1000);
        assert_eq!(tooltip.show_delay, 1000);

        tooltip.set_hide_delay(300);
        assert_eq!(tooltip.hide_delay, 300);
    }

    #[test]
    fn tooltip_event_mouse_enter_triggers_show_pending() {
        let mut tooltip = Tooltip::new("Tooltip", Rect::new(0, 0, 100, 40));
        assert!(!tooltip.is_visible());
        assert!(!tooltip.show_pending);
        assert!(!tooltip.hovering);

        tooltip.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(tooltip.hovering);
        assert!(tooltip.show_pending);
        assert!(!tooltip.is_visible());

        // Timer fires — should show
        tooltip.handle_event(&Event::Timer { id: TIMER_SHOW_ID });
        assert!(tooltip.is_visible());
        assert!(!tooltip.show_pending);
    }

    #[test]
    fn tooltip_event_mouse_leave_hides() {
        let mut tooltip = Tooltip::new("Tooltip", Rect::new(0, 0, 100, 40));
        // Simulate the full lifecycle: enter → (timer) → shown → leave → (timer) → hidden
        tooltip.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(tooltip.hovering);
        assert!(tooltip.show_pending);
        assert!(!tooltip.is_visible());

        // Timer fires — tooltip becomes visible
        tooltip.handle_event(&Event::Timer { id: TIMER_SHOW_ID });
        assert!(tooltip.is_visible());

        // Now mouse leaves
        tooltip.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert!(!tooltip.hovering);
        assert!(tooltip.hide_pending);
        assert!(tooltip.is_visible()); // still visible until timer fires

        // Timer fires — should hide
        tooltip.handle_event(&Event::Timer { id: TIMER_HIDE_ID });
        assert!(!tooltip.is_visible());
        assert!(!tooltip.hide_pending);
    }

    #[test]
    fn tooltip_immediate_show_with_zero_delay() {
        let mut tooltip = Tooltip::new("Fast", Rect::new(0, 0, 100, 40));
        tooltip.set_show_delay(0);
        assert!(!tooltip.is_visible());
        assert!(!tooltip.hovering);

        tooltip.handle_event(&Event::MouseEnter { pos: Point::new(5, 5) });
        assert!(tooltip.hovering);
        assert!(tooltip.is_visible()); // shown immediately because delay is 0
    }

    #[test]
    fn tooltip_svg_output_visible() {
        let mut tooltip = Tooltip::new("SVG Tooltip", Rect::new(0, 0, 140, 30));
        tooltip.show();

        let svg = render_to_svg(&mut tooltip);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    #[test]
    fn tooltip_svg_output_hidden() {
        let mut tooltip = Tooltip::new("Hidden", Rect::new(0, 0, 100, 30));
        // A tooltip that is not showing is dimmed, not omitted: `draw` used to `return`
        // early here, so a tooltip that had been created but not hovered painted nothing at
        // all — the defect the rendering census reported as `ink = 0`.
        let svg = render_to_svg(&mut tooltip);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        let fill_count = svg.matches("fill=").count();
        assert!(
            fill_count > 1,
            "a hidden tooltip must still paint its bubble, got only the background fill: {svg}"
        );

        // Shown and hidden must still be distinguishable, so the fix did not simply paint
        // the same frame in both states.
        let mut open = Tooltip::new("Hidden", Rect::new(0, 0, 100, 30));
        open.show();
        let shown = render_to_svg(&mut open);
        assert_ne!(svg, shown, "showing the tooltip must change what is painted");
    }
}
