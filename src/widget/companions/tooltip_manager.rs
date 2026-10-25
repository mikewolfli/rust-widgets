//! `TooltipManager` — the host-side hover/delay/duration pipeline.

use crate::compat::{String, Vec};
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{clamp_axis, surface_ink};

// ---------------------------------------------------------------------------
// COMP-09 TooltipManager
// ---------------------------------------------------------------------------

/// A host-side tooltip pipeline: hover, delay, show, expire.
///
/// # Hover source
///
/// The manager does **not** hit-test widgets itself. The library runtime owns no hover
/// registry, so the host forwards the widget under the pointer with
/// [`Self::note_hover`], clears it with [`Self::note_leave`], and feeds the pointer position
/// with [`Self::note_pointer`]. Calling [`Self::tick`] once per frame is what turns "hovered
/// for long enough" into a shown tooltip and what expires it after `duration_ms`. This is
/// deliberately a plain host model rather than a widget callback, because only the host knows
/// which control the pointer is over.
pub struct TooltipManager {
    base: BaseWidget,
    registrations: Vec<(ObjectId, String)>,
    hovered: Option<ObjectId>,
    hover_elapsed_ms: u64,
    current: Option<ObjectId>,
    visible: bool,
    shown_at_ms: u64,
    now_ms: u64,
    pointer: Point,
    delay_ms: u32,
    duration_ms: u32,
    enabled: bool,
    follow_pointer: bool,
    max_width: u32,
    /// Emitted with the widget id when a tooltip appears.
    pub tooltip_shown: Signal1<ObjectId>,
    /// Emitted with the widget id when a tooltip disappears.
    pub tooltip_hidden: Signal1<ObjectId>,
}

impl TooltipManager {
    /// Creates a manager with a 500 ms delay and a 3 s duration.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TooltipManager, geometry, "TooltipManager"),
            registrations: Vec::new(),
            hovered: None,
            hover_elapsed_ms: 0,
            current: None,
            visible: false,
            shown_at_ms: 0,
            now_ms: 0,
            pointer: Point::new(0, 0),
            delay_ms: 500,
            duration_ms: 3000,
            enabled: true,
            follow_pointer: true,
            max_width: 260,
            tooltip_shown: Signal1::new(),
            tooltip_hidden: Signal1::new(),
        }
    }

    /// Registers (or replaces) a widget's tooltip text.
    pub fn register(&mut self, widget_id: ObjectId, text: impl Into<String>) {
        let text = text.into();
        if let Some(entry) = self.registrations.iter_mut().find(|entry| entry.0 == widget_id) {
            entry.1 = text;
        } else {
            self.registrations.push((widget_id, text));
        }
    }

    /// Removes a widget's registration, hiding its tooltip if it is showing.
    pub fn unregister(&mut self, widget_id: ObjectId) {
        self.registrations.retain(|entry| entry.0 != widget_id);
        if self.current == Some(widget_id) {
            self.hide();
        }
        if self.hovered == Some(widget_id) {
            self.hovered = None;
            self.hover_elapsed_ms = 0;
        }
    }

    /// Shows a registered tooltip immediately, bypassing the delay.
    ///
    /// Returns `false` when the manager is disabled or the widget has no registration.
    pub fn show_now(&mut self, widget_id: ObjectId) -> bool {
        if !self.enabled {
            return false;
        }
        if self.text_for(widget_id).is_none() {
            return false;
        }
        if self.visible && self.current == Some(widget_id) {
            return true;
        }
        self.hide();
        self.current = Some(widget_id);
        self.visible = true;
        self.shown_at_ms = self.now_ms;
        self.base.request_redraw();
        self.tooltip_shown.emit(widget_id);
        true
    }

    /// Hides the current tooltip, publishing the id that disappeared.
    pub fn hide(&mut self) {
        let Some(widget_id) = self.current.take() else {
            return;
        };
        self.visible = false;
        self.base.request_redraw();
        self.tooltip_hidden.emit(widget_id);
    }

    /// Advances the delay and duration clocks.
    ///
    /// `delta_ms` is the elapsed frame time (used for the hover delay); `now_ms` is the host's
    /// monotonic clock (used for the duration). Returns whether visibility changed.
    pub fn tick(&mut self, delta_ms: u64, now_ms: u64) -> bool {
        self.now_ms = self.now_ms.max(now_ms);
        let before = self.visible;
        if !self.enabled {
            self.hide();
            return before != self.visible;
        }
        if self.visible
            && self.duration_ms > 0
            && self.now_ms.saturating_sub(self.shown_at_ms) >= u64::from(self.duration_ms)
        {
            self.hide();
            // Restart the hover delay: without this the same tick would see `hovered` still
            // true, `hover_elapsed_ms` past the delay, and immediately re-show what it just
            // expired.
            self.hover_elapsed_ms = 0;
        }
        if !self.visible {
            match self.hovered {
                Some(widget_id) => {
                    self.hover_elapsed_ms = self.hover_elapsed_ms.saturating_add(delta_ms);
                    if self.hover_elapsed_ms >= u64::from(self.delay_ms) {
                        self.show_now(widget_id);
                    }
                }
                None => self.hover_elapsed_ms = 0,
            }
        }
        before != self.visible
    }

    /// Reports the widget under the pointer, restarting the delay when it changed.
    pub fn note_hover(&mut self, widget_id: ObjectId) {
        if self.hovered == Some(widget_id) {
            return;
        }
        self.hovered = Some(widget_id);
        self.hover_elapsed_ms = 0;
        if self.visible && self.current != Some(widget_id) {
            self.hide();
        }
    }

    /// Reports that the pointer left every registered widget.
    pub fn note_leave(&mut self) {
        self.hovered = None;
        self.hover_elapsed_ms = 0;
        self.hide();
    }

    /// Reports the pointer position, which anchors a follow-pointer bubble.
    pub fn note_pointer(&mut self, pointer: Point) {
        self.pointer = pointer;
        if self.visible && self.follow_pointer {
            self.base.request_redraw();
        }
    }

    /// Returns the current tooltip text, if a bubble is visible.
    pub fn tooltip(&self) -> Option<&str> {
        if !self.visible {
            return None;
        }
        self.current.and_then(|widget_id| self.text_for(widget_id))
    }

    /// Returns the widget id the visible tooltip belongs to.
    pub fn active_widget(&self) -> Option<ObjectId> {
        self.current
    }

    /// Returns a registration's text.
    pub fn text_for(&self, widget_id: ObjectId) -> Option<&str> {
        self.registrations.iter().find(|entry| entry.0 == widget_id).map(|entry| entry.1.as_str())
    }

    /// Sets the hover delay in milliseconds.
    pub fn set_delay_ms(&mut self, delay_ms: u32) {
        self.delay_ms = delay_ms;
    }
    /// Returns the hover delay.
    pub fn delay_ms(&self) -> u32 {
        self.delay_ms
    }
    /// Sets how long a tooltip stays visible (`0` means until the pointer leaves).
    pub fn set_duration_ms(&mut self, duration_ms: u32) {
        self.duration_ms = duration_ms;
    }
    /// Returns the visible duration.
    pub fn duration_ms(&self) -> u32 {
        self.duration_ms
    }
    /// Enables or disables the pipeline; disabling hides the current tooltip.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hide();
        }
    }
    /// Returns whether the pipeline is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Sets whether the bubble follows pointer motion.
    pub fn set_follow_pointer(&mut self, follow_pointer: bool) {
        self.follow_pointer = follow_pointer;
    }
    /// Returns whether the bubble follows pointer motion.
    pub fn follow_pointer(&self) -> bool {
        self.follow_pointer
    }
    /// Sets the maximum bubble width.
    pub fn set_max_width(&mut self, max_width: u32) {
        self.max_width = max_width.max(24);
        self.base.request_redraw();
    }
    /// Returns the maximum bubble width.
    pub fn max_width(&self) -> u32 {
        self.max_width
    }
}

impl EventHandler for TooltipManager {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => self.note_pointer(*pos),
            Event::MouseLeave { .. } => self.note_leave(),
            Event::KeyPress { key: 27, .. } => self.hide(),
            _ => {}
        }
    }
}

impl Draw for TooltipManager {
    fn draw(&mut self, context: &mut RenderContext) {
        let Some(text) = self.tooltip() else {
            let rect = self.geometry();
            let style = self.style();
            let (surface, ink) = surface_ink(style);
            context.fill_rect(rect, surface.with_alpha(32));
            context.draw_rect(rect, surface.blend(&ink, 0.18));
            return;
        };
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        let width = context.measure_text(text, font).width.min(self.max_width).max(24);
        let height = context.measure_text(text, font).height.max(16) + 8;
        let rect = self.geometry();
        let origin = if self.follow_pointer {
            self.pointer + (12, 20)
        } else {
            Point::new(rect.x + 8, rect.y + 8)
        };
        let max_x = rect.x + rect.width as i32 - width as i32;
        let max_y = rect.y + rect.height as i32 - height as i32;
        let bubble = Rect::new(
            clamp_axis(origin.x, rect.x, max_x),
            clamp_axis(origin.y, rect.y, max_y),
            width,
            height,
        );
        let fill = ink.blend(&surface, 0.10);
        context.fill_rect(bubble, fill);
        context.draw_rect(bubble, fill.contrast_color().with_alpha(60));
        context.draw_text_fitted(
            bubble,
            text,
            font,
            fill.contrast_color(),
            crate::core::HorizontalAlignment::Center,
        );
    }
}

impl Widget for TooltipManager {
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
        Size::new(240, 40)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "tooltip_shown" => {
                Some(EventSignalRef::mapped("tooltip_shown", &self.tooltip_shown, |value| {
                    CapabilityValue::UInt(*value)
                }))
            }
            "tooltip_hidden" => {
                Some(EventSignalRef::mapped("tooltip_hidden", &self.tooltip_hidden, |value| {
                    CapabilityValue::UInt(*value)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for TooltipManager {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "delay_ms" => Ok(CapabilityValue::UInt(u64::from(self.delay_ms))),
            "duration_ms" => Ok(CapabilityValue::UInt(u64::from(self.duration_ms))),
            "enabled" => Ok(CapabilityValue::Bool(self.enabled)),
            "follow_pointer" => Ok(CapabilityValue::Bool(self.follow_pointer)),
            "max_width" => Ok(CapabilityValue::UInt(u64::from(self.max_width))),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "delay_ms" => {
                self.set_delay_ms(expect_u32(value)?);
                Ok(())
            }
            "duration_ms" => {
                self.set_duration_ms(expect_u32(value)?);
                Ok(())
            }
            "enabled" => {
                self.set_enabled(expect_bool(value)?);
                Ok(())
            }
            "follow_pointer" => {
                self.set_follow_pointer(expect_bool(value)?);
                Ok(())
            }
            "max_width" => {
                self.set_max_width(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        // `enabled` here means "the pipeline is on", which is the property the issue names.
        // It is listed once; the shared base names keep `visible`, `tooltip` and `geometry`,
        // while the widget-level enable flag remains reachable through `Widget::set_enabled`.
        property_names_of![
            "delay_ms",
            "duration_ms",
            "enabled",
            "follow_pointer",
            "max_width",
            "visible",
            "tooltip",
            "geometry"
        ]
    }
}
