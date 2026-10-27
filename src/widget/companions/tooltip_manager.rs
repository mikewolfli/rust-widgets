//! `TooltipManager` — the host-side hover/delay/duration pipeline.
//!
//! # The hover bridge
//!
//! The manager is the library's **one** tooltip presenter. A control does not draw its own
//! bubble: the runtime already knows which widget the pointer is over (`runtime::hovered_widget`),
//! so the runtime's hover transition forwards every change here through
//! [`TooltipManager::note_runtime_hover`], registering the hovered control's `tooltip` property
//! and unregistering the one the pointer left. A host therefore does not have to call
//! [`TooltipManager::register`] for a control it wired with `set_tooltip` — the registration is
//! part of the pointer path. `register` / `unregister` remain public for a host that wants to
//! stage text ahead of time or present a tooltip for a control outside the tree.
//!
//! # The bubble renderer
//!
//! [`crate::widget::dialog::tooltip::Tooltip`] is the library's one tooltip **renderer** (text
//! runs, wrap, padding, colours, fade). This manager is the library's one tooltip **pipeline**
//! (hover, delay, duration, pointer anchoring). They used to be two independent presenters that
//! each painted a bubble; the manager now owns a `Tooltip` and paints through it, so a change to
//! how a bubble looks cannot land in one path and miss the other (gap 9).
//!
//! `Tooltip` lives in the `dialog` module, which only exists with `full_widgets`, so the
//! delegation is gated: a build without that module keeps the manager's own simple bubble. That
//! is one renderer **per build**, not two at once — the two arms never compile together.

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
/// Only the fallback bubble measures text itself; with `full_widgets` the renderer does.
#[cfg(not(full_widgets))]
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{clamp_axis, surface_ink};

// ---------------------------------------------------------------------------
// COMP-09 TooltipManager
// ---------------------------------------------------------------------------

/// The hover registration the runtime bridge currently owns.
#[derive(Debug, Clone)]
struct BridgeHover {
    /// The widget the bridge registered.
    widget: ObjectId,
    /// The text the host had registered for this widget before the bridge adopted it, or `None`
    /// when the bridge created the entry. On release the host's text is written back, so a hover
    /// passing over a host registration cannot destroy it.
    previous: Option<String>,
}

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
    /// The hover registration the runtime's bridge currently owns, if any.
    ///
    /// It records the host's text when the bridge adopted an existing registration, so a tooltip
    /// the host staged survives the pointer leaving. Without it, the first hover would destroy a
    /// host's registration and a later `show_now` would silently stop working.
    bridge_hover: Option<BridgeHover>,
    /// The bubble itself: text, runs, wrap, padding, colours and fade (gap 9).
    ///
    /// See the module docs for why this is gated: the renderer lives in `dialog`, which is a
    /// `full_widgets` module.
    #[cfg(full_widgets)]
    bubble: crate::widget::dialog::tooltip::Tooltip,
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
            bridge_hover: None,
            #[cfg(full_widgets)]
            bubble: crate::widget::dialog::tooltip::Tooltip::new("", geometry),
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
        // A text change for the tooltip that is on screen must reach the renderer at once, or the
        // bubble would keep drawing the string it was handed when it appeared.
        if self.current == Some(widget_id) {
            self.sync_bubble_text();
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
        self.sync_bubble_text();
        self.base.request_redraw();
        self.tooltip_shown.emit(widget_id);
        true
    }

    /// Copies the presentable text into the renderer (gap 9).
    ///
    /// The manager owns **which** tooltip is shown; the `Tooltip` owns what it looks like. Keeping
    /// the copy in one place means every path that changes the current registration — `show_now`,
    /// a re-`register` of the visible control, a hover bridge sync — goes through the same line.
    #[cfg(full_widgets)]
    fn sync_bubble_text(&mut self) {
        let text = self.current.and_then(|id| self.text_for(id)).unwrap_or("").to_string();
        self.bubble.set_text(text);
    }

    /// [`Self::sync_bubble_text`] for a build without the `dialog` module, which has no renderer to
    /// copy into: the manager's own bubble reads the registration table directly.
    #[cfg(not(full_widgets))]
    fn sync_bubble_text(&mut self) {}

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

    /// Applies a hover change forwarded by the runtime's global hover registry.
    ///
    /// `hovered` is the widget under the pointer and `text` is that widget's `tooltip` property
    /// — the same value `properties_dyn().get("tooltip")` answers, because
    /// `base_property_get` reads `Widget::tooltip`. An absent widget or an empty/whitespace-only
    /// tooltip means "nothing to present", which releases the previous registration and hides
    /// any bubble.
    ///
    /// # Why this is not just `register` + `note_hover`
    ///
    /// The bridge must not destroy a registration the **host** made. It therefore saves the
    /// text it adopted and writes it back when the pointer leaves; an entry the bridge created
    /// itself is removed instead. A widget that stays hovered while its tooltip text changes is
    /// re-registered on each sync, so the manager never presents a stale string.
    pub fn note_runtime_hover(&mut self, hovered: Option<ObjectId>, text: Option<&str>) {
        let text = text.map(str::trim).filter(|text| !text.is_empty());
        let (Some(widget), Some(text)) = (hovered, text) else {
            self.release_bridge_hover();
            return;
        };
        if self.bridge_hover.as_ref().is_some_and(|bridge| bridge.widget == widget) {
            self.register(widget, text);
            self.note_hover(widget);
            // The control stayed hovered but rewrote its tooltip: a visible bubble must show
            // the new string on the next frame rather than the one it was painted with.
            if self.visible && self.current == Some(widget) {
                self.base.request_redraw();
            }
            return;
        }
        self.release_bridge_hover();
        let previous = self.text_for(widget).map(String::from);
        self.register(widget, text);
        self.bridge_hover = Some(BridgeHover { widget, previous });
        self.note_hover(widget);
    }

    /// Releases the registration the bridge owns, restoring a host's text when it adopted one.
    fn release_bridge_hover(&mut self) {
        let Some(bridge) = self.bridge_hover.take() else {
            return;
        };
        match bridge.previous {
            Some(previous) => self.register(bridge.widget, previous),
            None => self.unregister(bridge.widget),
        }
        // Even an adopted entry loses the hover: the pointer is no longer over the widget, so
        // the delay must restart and a bubble that belongs to it must disappear.
        self.note_leave();
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
        if self.tooltip().is_none() {
            let rect = self.geometry();
            let style = self.style();
            let (surface, ink) = surface_ink(style);
            context.fill_rect(rect, surface.with_alpha(32));
            context.draw_rect(rect, surface.blend(&ink, 0.18));
            return;
        }
        let rect = self.geometry();
        // The bubble box is decided here — the manager owns the pointer/anchor geometry — and the
        // **renderer** decides what goes inside it (gap 9). The fallback below is the same box
        // painted by the manager itself for a build without the `dialog` module.
        #[cfg(full_widgets)]
        {
            // One measurement for both axes: `preferred_size` walks the text layout, and a
            // per-frame draw has no reason to walk it twice.
            let preferred = self.bubble.preferred_size();
            let width = preferred.width.min(self.max_width).max(24);
            let height = preferred.height.max(18);
            let origin = if self.follow_pointer {
                self.pointer + (12, 20)
            } else {
                Point::new(rect.x + 8, rect.y + 8)
            };
            let bubble = Rect::new(
                clamp_axis(origin.x, rect.x, rect.x + rect.width as i32 - width as i32),
                clamp_axis(origin.y, rect.y, rect.y + rect.height as i32 - height as i32),
                width,
                height,
            );
            self.bubble.set_geometry(bubble);
            self.bubble.set_shown(self.visible);
            self.bubble.draw(context);
        }
        #[cfg(not(full_widgets))]
        {
            let text = self.tooltip().unwrap_or_default();
            let style = self.style();
            let (surface, ink) = surface_ink(style);
            let font = effective_font(style);
            let width = context.measure_text(text, font).width.min(self.max_width).max(24);
            let height = context.measure_text(text, font).height.max(16) + 8;
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
            context.draw_text_fitted_cow(
                bubble,
                text,
                font,
                fill.contrast_color(),
                crate::core::HorizontalAlignment::Center,
            );
        }
    }
}

impl TooltipManager {
    /// The bubble renderer this manager paints through (gap 9).
    ///
    /// A host that wants rich-text runs, wrapping or explicit bubble colours configures the
    /// renderer here and then shows the bubble through the manager's own pipeline
    /// ([`Self::show_now`] / hover); the manager keeps owning *which* tooltip is shown. Editing the
    /// renderer and painting it directly would bypass the delay/duration pipeline, so this is the
    /// only hand-off between the two.
    #[cfg(full_widgets)]
    pub fn renderer(&mut self) -> &mut crate::widget::dialog::tooltip::Tooltip {
        &mut self.bubble
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
    /// Advances the host's tooltip clocks from the frame bus.
    ///
    /// [`Self::tick`] takes the host's monotonic clock because a tooltip's *duration* is a
    /// wall-clock fact; the bus only knows how much time the frame covers. Accumulating the
    /// delta reproduces the same clock for a host that drives every frame (the same choice
    /// `segmented_button` makes for the tooltips it owns).
    fn tick(&mut self, delta_ms: u32) -> bool {
        let now_ms = self.now_ms.saturating_add(u64::from(delta_ms));
        Self::tick(self, u64::from(delta_ms), now_ms)
    }
    /// A tooltip owes frames while a hover delay is counting down or one is on screen.
    ///
    /// Without this the bus would stop sweeping after the first frame and a tooltip shown at
    /// the end of its delay would never expire.
    fn is_animating(&self) -> bool {
        self.hovered.is_some() || self.visible
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_registered_widget_can_show_a_tooltip() {
        let mut manager = TooltipManager::new(Rect::new(0, 0, 320, 200));
        manager.register(11, "Save");
        assert_eq!(manager.text_for(11), Some("Save"));
        assert!(manager.show_now(11));
        assert_eq!(manager.tooltip(), Some("Save"));
        manager.hide();
        assert!(manager.tooltip().is_none());
        assert!(!manager.show_now(12));
    }

    /// The bridge registers what the pointer is over and unregisters what it left.
    #[test]
    fn a_forwarded_hover_registers_and_releases_its_own_entry() {
        let mut manager = TooltipManager::new(Rect::new(0, 0, 320, 200));
        manager.note_runtime_hover(Some(7), Some("  Bold  "));
        assert_eq!(manager.text_for(7), Some("Bold"), "the tooltip text is registered trimmed");
        assert!(manager.tick(600, 600), "a registered hover shows after the delay");
        assert_eq!(manager.active_widget(), Some(7));

        manager.note_runtime_hover(None, None);
        assert_eq!(manager.text_for(7), None, "the bridge releases its own registration");
        assert!(manager.tooltip().is_none(), "leaving hides the bubble");
    }

    /// A host's own registration survives a hover passing over it.
    #[test]
    fn the_bridge_leaves_a_host_registration_in_place() {
        let mut manager = TooltipManager::new(Rect::new(0, 0, 320, 200));
        manager.register(9, "Staged by the host");
        manager.note_runtime_hover(Some(9), Some("Hovered"));
        assert_eq!(manager.text_for(9), Some("Hovered"));
        manager.note_runtime_hover(None, None);
        assert_eq!(
            manager.text_for(9),
            Some("Staged by the host"),
            "an adopted host entry is restored, not deleted"
        );
        assert!(manager.show_now(9), "the host can still present it later");
    }

    /// An empty tooltip property means "nothing to present".
    #[test]
    fn a_widget_without_tooltip_text_clears_the_hover() {
        let mut manager = TooltipManager::new(Rect::new(0, 0, 320, 200));
        manager.note_runtime_hover(Some(5), Some("tip"));
        manager.note_runtime_hover(Some(5), Some("   "));
        assert_eq!(manager.text_for(5), None);
        assert!(manager.active_widget().is_none());
    }

    /// The manager paints through the `Tooltip` renderer, so a bubble feature configured on the
    /// renderer reaches the bubble the manager presents (gap 9).
    ///
    /// The picture, not the field: if the manager still had its own bubble painter, setting a run
    /// on the renderer would change nothing on screen.
    #[cfg(full_widgets)]
    #[test]
    fn the_manager_paints_through_the_tooltip_renderer() {
        use crate::widget::dialog::tooltip::TooltipSpan;
        use crate::widget::svg::render_to_svg;

        let mut manager = TooltipManager::new(Rect::new(0, 0, 320, 200));
        manager.register(7, "Alpha");
        assert!(manager.show_now(7), "the registration is presentable");
        let plain = render_to_svg(&mut manager);

        let mut spans = Vec::new();
        // A colour, not a weight: the bundled face has no bold cut, so a bold run can paint
        // identically to the plain label and the test would pass for the wrong reason.
        spans.push(TooltipSpan::new("Alpha").with_color(crate::core::Color::rgb(255, 0, 0)));
        manager.renderer().set_spans(spans);
        assert_eq!(manager.renderer().spans().len(), 1, "the run reached the renderer");
        let rich = render_to_svg(&mut manager);

        assert_ne!(
            plain, rich,
            "a run set on the renderer must change the bubble the manager paints"
        );
    }
}
