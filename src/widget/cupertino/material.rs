// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Material Design controls that share the Cupertino module's registration.
//!
//! These two controls are Material widgets by design language, not Cupertino ones. They used
//! to live in `core.rs`, which is the iOS collection's file — a reader looking for the iOS
//! switch found a snackbar and a navigation rail beside it. Moving the definitions here makes
//! each module's name describe its contents; the public paths are re-exported by
//! `cupertino::mod`, so no caller's import changes.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconItemError, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// ── MaterialSnackbar ─────────────────────────────────────────────────────────

/// Material Design snackbar notification.
///
/// A snackbar displays a brief message at the bottom of the screen with an
/// optional action button. It appears temporarily and can be dismissed by
/// the user or programmatically.
pub struct MaterialSnackbar {
    base: BaseWidget,
    message: String,
    action_text: String,
    /// Emitted when the action button is pressed.
    pub action_pressed: GenericSignal,
    /// Emitted when the snackbar is dismissed.
    pub dismissed: GenericSignal,
    /// How long the snackbar stays on screen, in milliseconds; `0` means it stays until dismissed.
    duration_ms: u64,
    /// Milliseconds the current presentation has been visible.
    elapsed_ms: u64,
    /// Entrance/exit slide fraction, `0.0` off screen and `1.0` fully presented.
    slide: PropertyDriver,
    /// Whether an animated dismissal is in progress.
    closing: bool,
}

impl MaterialSnackbar {
    /// Creates a new MaterialSnackbar with the given geometry.
    ///
    /// The snackbar starts hidden. Use [`show()`](Self::show) to display it.
    pub fn new(geometry: Rect) -> Self {
        let mut base = BaseWidget::new(WidgetKind::MaterialSnackbar, geometry, "MaterialSnackbar");
        base.hide();
        Self {
            base,
            message: String::new(),
            action_text: String::new(),
            action_pressed: GenericSignal::new(),
            dismissed: GenericSignal::new(),
            duration_ms: 4_000,
            elapsed_ms: 0,
            slide: PropertyDriver::at(0.0, MotionSlot::Normal),
            closing: false,
        }
    }

    /// Sets the message text displayed in the snackbar.
    pub fn set_message(&mut self, message: &str) {
        if self.message == message {
            return;
        }
        self.message = message.to_string();
        self.base.request_redraw();
    }

    /// Returns the current message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets the action button text. Pass an empty string to hide the action button.
    pub fn set_action_text(&mut self, text: &str) {
        if self.action_text == text {
            return;
        }
        self.action_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current action button text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Shows the snackbar. Does nothing if already visible.
    pub fn show(&mut self) {
        if !self.base.is_visible() {
            self.base.show();
            self.elapsed_ms = 0;
            self.closing = false;
            // Slide in from below: start off screen and animate to the presented position.
            self.slide.jump_to(0.0);
            self.slide.set_target(1.0);
            self.base.request_redraw();
        }
    }

    /// Dismisses the snackbar immediately. Emits the `dismissed` signal.
    ///
    /// `dismissed` is deliberately not gated by `enabled`: it reports the snackbar leaving
    /// the screen (a timeout or a host decision), not a user action, and a host that
    /// disabled the snackbar's interactions still needs to know it went away. Hiding the
    /// snackbar does block the signal, which is the condition `dismiss` actually depends on.
    pub fn dismiss(&mut self) {
        if self.base.is_visible() {
            self.base.hide();
            self.closing = false;
            self.slide.jump_to(0.0);
            self.dismissed.emit();
            self.base.request_redraw();
        }
    }

    /// Starts an animated dismissal: the bar slides out, then hides and emits `dismissed`.
    ///
    /// [`Self::dismiss`] stays immediate because a host that tears the control down must not be
    /// left with a visible bar waiting for a frame. This variant is for the user-visible exit.
    pub fn dismiss_animated(&mut self) {
        if self.base.is_visible() && !self.closing {
            self.closing = true;
            self.slide.set_target(0.0);
            self.base.request_redraw();
        }
    }

    /// How long the snackbar stays on screen in milliseconds; `0` means it never auto-dismisses.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Sets the auto-dismiss delay. Zero keeps the bar until the user or host dismisses it.
    pub fn set_duration_ms(&mut self, duration_ms: u64) {
        if self.duration_ms == duration_ms {
            return;
        }
        self.duration_ms = duration_ms;
        self.elapsed_ms = 0;
    }

    /// The entrance/exit slide fraction, `0.0` off screen and `1.0` presented.
    pub fn slide_progress(&self) -> f32 {
        self.slide.value()
    }

    /// Returns whether the snackbar is currently visible.
    pub fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
}

impl Widget for MaterialSnackbar {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::CUPERTINO_BAR_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::CUPERTINO_BAR_DEFAULT_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Drives the auto-dismiss countdown and the slide animation.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut changed = false;
        if self.base.is_visible() && !self.closing && self.duration_ms > 0 {
            self.elapsed_ms = self.elapsed_ms.saturating_add(u64::from(delta_ms));
            if self.elapsed_ms >= self.duration_ms {
                self.dismiss();
                return true;
            }
            changed = true;
        }
        let moving = self.slide.tick(delta_ms);
        changed |= moving;
        if self.closing && !self.slide.is_moving() && self.slide.value() <= 0.001 {
            self.base.hide();
            self.closing = false;
            self.dismissed.emit();
            changed = true;
        }
        if changed {
            self.base.request_redraw();
        }
        changed
    }

    /// A running countdown owes a timeout frame, and a slide in flight owes an animation frame.
    fn is_animating(&self) -> bool {
        self.slide.is_moving() || (self.base.is_visible() && !self.closing && self.duration_ms > 0)
    }

    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "action_pressed" => Some(EventSignalRef::unit("action_pressed", &self.action_pressed)),
            "dismissed" => Some(EventSignalRef::unit("dismissed", &self.dismissed)),
            _ => None,
        }
    }
}

/// `MaterialSnackbar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. Both properties now report
/// the snackbar's real text instead of the placeholder defaults that dispatch
/// returned.
impl WidgetProperties for MaterialSnackbar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" => Ok(CapabilityValue::String(self.message().to_string())),
            "action_text" => Ok(CapabilityValue::String(self.action_text().to_string())),
            "duration_ms" => Ok(CapabilityValue::UInt(self.duration_ms())),
            "slide_progress" => Ok(CapabilityValue::Float(self.slide_progress() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" => {
                self.set_message(&expect_string(value)?);
                Ok(())
            }
            "action_text" => {
                self.set_action_text(&expect_string(value)?);
                Ok(())
            }
            "duration_ms" => {
                let duration = match value {
                    CapabilityValue::UInt(ms) => ms,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_duration_ms(duration);
                Ok(())
            }
            "slide_progress" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "message",
            "action_text",
            "duration_ms",
            "slide_progress",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `material_snackbar` publishes.
    ///
    /// Both `show` and `dismiss` map onto the widget's real methods and take no
    /// payload. `set_message` and `set_action_text` carry the text the caller
    /// wants displayed, so a bare invocation is reported as needing one rather
    /// than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show" => {
                self.show();
                Ok(())
            }
            "dismiss" => {
                self.dismiss();
                Ok(())
            }
            "set_message" | "set_action_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for MaterialSnackbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The pill used to be a fixed near-black and the
        // action a fixed Material blue, so a light/dark switch left the snackbar unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("material_snackbar");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `material_snackbar` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and the active theme writes the window fill into `style.background_color`.
        // The pill is eased away from the window fill in both directions — darker on the light
        // theme, lighter on the dark one — so it never coincides with the frame behind it, while a
        // colour the caller set still wins.
        let pill = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.72),
        };
        // The label is drawn in whatever contrasts with the pill, rather than a fixed white that
        // only read against the old near-black bar.
        let label = pill.contrast_color();
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != pill)
            .unwrap_or_else(|| pill.blend(&secondary, 0.45));

        let bar_height = 48u32;
        // The entrance/exit slide moves the bar off its resting slot; at rest the fraction is 1.0 and
        // the bar is exactly where it always was. A hidden snackbar paints its resting slot (at
        // reduced presence, below), not the off-screen end of the slide: the census draws every
        // control inside its own box, and an off-screen pill would otherwise escape it.
        let slide = if self.base.is_visible() { self.slide.value().clamp(0.0, 1.0) } else { 1.0 };
        let bar_y = rect.y + rect.height as i32 - bar_height as i32 - 16
            + ((1.0 - slide) * (bar_height + 16) as f32) as i32;

        // ── Background pill ──
        //
        // Painted before the visibility check. It used to `return` while hidden, so a freshly
        // constructed snackbar — which starts hidden by contract — painted nothing and the census
        // reported `ink = 0`. The bar slot is shown at reduced presence while hidden and becomes the
        // opaque toast when `show` is called, which is the same shape the bottom sheet uses for its
        // own closed state.
        let bar_rect = Rect::new(rect.x + 12, bar_y, rect.width.saturating_sub(24), bar_height);
        let fill = if self.base.is_visible() { pill } else { pill.with_alpha(96) };
        context.fill_rounded_rect(bar_rect, bar_height / 2, fill);
        context.draw_rounded_rect_stroke(bar_rect, bar_height / 2, border, 1);

        if !self.base.is_visible() {
            return;
        }

        // ── Message text (left side) ──
        if !self.message.is_empty() {
            // The message takes the **effective font** at its role size, so the theme family and the
            // user's text scale reach it (D09-STYLE-01).
            let font = role_font(style, 14.0);
            let metrics = context.measure_text(&self.message, &font);

            let text_x = bar_rect.x + 16;
            // Centre on the bar. The origin is the glyph box's top edge, so the offset is half
            // the *line box*; the old ascent/descent pair claimed to centre but sat half a
            // line low.
            let text_y = bar_rect.y + (bar_height as i32 - metrics.height as i32) / 2;

            context.draw_text(
                crate::core::Point::new(text_x, text_y),
                &self.message,
                &font,
                label,
                HorizontalAlignment::Left,
            );
        }

        // ── Action text (right side, if non-empty) ──
        if !self.action_text.is_empty() {
            let action_font = role_font(style, 14.0).with_bold(true);
            let metrics = context.measure_text(&self.action_text, &action_font);

            let action_x = bar_rect.x + bar_rect.width as i32 - metrics.width as i32 - 16;
            // Same top-edge origin as the message: centre on the line box, not on the
            // ascent/descent midpoint, which sat the action half a line low.
            let action_y = bar_rect.y + (bar_height as i32 - metrics.height as i32) / 2;

            context.draw_text(
                crate::core::Point::new(action_x, action_y),
                &self.action_text,
                &action_font,
                primary,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for MaterialSnackbar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_visible() {
            return;
        }
        // A disabled control stays out of play even while visible: without this the action
        // button still fired for a snackbar the host had disabled.
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                let bar_height = 48u32;
                let bar_y = rect.y + rect.height as i32 - bar_height as i32 - 16;
                let bar_rect =
                    Rect::new(rect.x + 12, bar_y, rect.width.saturating_sub(24), bar_height);

                if !bar_rect.contains_point(*pos) {
                    return;
                }

                // Check if the click is in the action area (right side)
                if !self.action_text.is_empty() {
                    let action_font = role_font(self.style(), 14.0).with_bold(true);
                    let action_area_width =
                        context_proxy_measure_text(&self.action_text, &action_font) + 32;

                    if pos.x >= bar_rect.x + bar_rect.width as i32 - action_area_width {
                        self.action_pressed.emit();
                        self.base.request_redraw();
                        return;
                    }
                }

                // Click anywhere else on the snackbar dismisses it
                self.dismiss();
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

/// Helper to measure text width without a RenderContext reference for hit-testing.
/// Falls back to an approximate width calculation when context is unavailable.
fn context_proxy_measure_text(text: &str, font: &crate::core::Font) -> i32 {
    // Approximate width: average char width ~0.6 * font size
    let char_width = (font.size() * 0.6) as i32;
    (text.len() as i32 * char_width).max(0)
}

#[cfg(test)]
mod snackbar_tests {
    use super::*;

    #[test]
    fn a_snackbar_dismisses_after_its_duration() {
        let mut snackbar = MaterialSnackbar::new(Rect::new(0, 0, 375, 160));
        snackbar.set_message("Saved");
        assert_eq!(snackbar.duration_ms(), 4_000);
        snackbar.show();
        assert!(snackbar.is_visible());
        assert!(snackbar.is_animating(), "a visible snackbar owes its timeout frame");
        assert!((snackbar.slide_progress() - 0.0).abs() < f32::EPSILON);

        assert!(snackbar.tick(16), "the slide moved");
        assert!(snackbar.slide_progress() > 0.0);

        let dismissed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let d = dismissed.clone();
        snackbar.dismissed.connect(move || {
            d.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(snackbar.tick(5_000));
        assert!(!snackbar.is_visible(), "the timeout hides the bar");
        assert!(dismissed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn zero_duration_is_permanent_and_the_exit_slides_out() {
        let mut snackbar = MaterialSnackbar::new(Rect::new(0, 0, 375, 160));
        snackbar.set_duration_ms(0);
        snackbar.show();
        for _ in 0..100 {
            snackbar.tick(100);
        }
        assert!(snackbar.is_visible(), "duration 0 must not auto-dismiss");
        assert!((snackbar.slide_progress() - 1.0).abs() < 1e-3);

        snackbar.dismiss_animated();
        assert!(snackbar.is_visible(), "the exit starts while still on screen");
        for _ in 0..100 {
            snackbar.tick(16);
        }
        assert!(!snackbar.is_visible());
        assert_eq!(snackbar.slide_progress(), 0.0);
    }
}

// ── MaterialNavigationRail ─────────────────────────────────────────────────────

/// A single item in a MaterialNavigationRail.
#[derive(Clone)]
pub struct RailItem {
    /// Icon glyph or icon-name text. A text stand-in, not image data.
    pub icon: String,
    /// Text label shown beneath (or beside) the icon.
    pub label: String,
}

impl RailItem {
    /// Creates a new rail item with the given icon and label.
    pub fn new(icon: &str, label: &str) -> Self {
        Self { icon: icon.to_string(), label: label.to_string() }
    }
}

/// The widest the rail's own strip is drawn.
///
/// A navigation rail is a fixed-width edge strip (Material's spec is 80 dp), so the
/// control's rectangle is the *room offered*, not the rail's width. Painting the whole
/// rectangle made the rail a full-width panel and left its items against the far-left edge.
const RAIL_MAX_WIDTH: u32 = 80;
/// The vertical pitch of one rail destination.
///
/// Named once because the paint and the hit test both step by it; two literals would let
/// the click target drift from the destination it describes.
const RAIL_ITEM_HEIGHT: u32 = 72;

/// Material Design navigation rail for tablets (BLUE11 R10.22).
///
/// A vertically stacked set of destinations with one selected at a time. The
/// selected index always names an item: it defaults to `0` even while the rail
/// is empty, so it is out of range until the first item is added.
pub struct MaterialNavigationRail {
    base: BaseWidget,
    items: Vec<RailItem>,
    item_icon_sources: Vec<Option<IconSource>>,
    selected_index: usize,
    /// Emitted with the newly selected index whenever the selection changes.
    /// One-based ordering is not implied; indices are positions in
    /// `MaterialNavigationRail::items()`.
    pub selected_changed: Signal1<usize>,
}

impl MaterialNavigationRail {
    /// Creates a new MaterialNavigationRail with the given geometry.
    pub fn new(geometry: crate::core::Rect) -> Self {
        let base =
            BaseWidget::new(WidgetKind::MaterialNavigationRail, geometry, "MaterialNavigationRail");
        Self {
            base,
            items: Vec::new(),
            item_icon_sources: Vec::new(),
            selected_index: 0,
            selected_changed: Signal1::new(),
        }
    }

    /// Adds an item with the given icon and label.
    pub fn add_item(&mut self, icon: &str, label: &str) {
        self.items.push(RailItem::new(icon, label));
        self.item_icon_sources.push(None);
        self.base.request_redraw();
    }

    /// Assigns a typed icon source to an existing rail item index.
    pub fn set_item_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let len = self.items.len();
        let Some(slot) = self.item_icon_sources.get_mut(index) else {
            return Err(IconItemError::IndexOutOfBounds { index, len });
        };
        *slot = source;
        self.base.request_redraw();
        Ok(())
    }

    /// Sets the selected index. Clamped to valid range.
    ///
    /// `selected_changed` is deliberately not gated by `enabled`: selection here is driven
    /// by the host (navigation state), and the disabled control already ignores presses, so
    /// silencing the programmatic path would only desynchronise the host's state.
    pub fn set_selected(&mut self, index: usize) {
        if self.items.is_empty() {
            self.selected_index = 0;
            return;
        }
        let clamped = index.min(self.items.len() - 1);
        if clamped != self.selected_index {
            self.selected_index = clamped;
            self.selected_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected index.
    pub fn selected(&self) -> usize {
        self.selected_index
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Removes all items.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.item_icon_sources.clear();
        self.selected_index = 0;
        self.base.request_redraw();
    }
}

impl Widget for MaterialNavigationRail {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::CUPERTINO_TAB_BAR_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::CUPERTINO_TAB_BAR_DEFAULT_HEIGHT,
        )
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::MaterialNavigationRail
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "selected_changed" => {
                Some(EventSignalRef::mapped("selected_changed", &self.selected_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `MaterialNavigationRail`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch: `selected_index`
/// is the published name for the field the `selected()` / `set_selected()`
/// accessors back.
impl WidgetProperties for MaterialNavigationRail {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => Ok(CapabilityValue::UInt(self.selected() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_index", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MaterialNavigationRail {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The rail was a fixed white, its indicator and
        // labels a fixed Material blue and grey, so a light/dark switch left it unchanged — the
        // rendering census reported the control as theme-blind. It also `return`ed while empty, so
        // a freshly constructed rail reported `ink = 0`.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("material_navigation_rail");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `material_navigation_rail` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and the active theme writes the window fill into
        // `style.background_color`. A rail painted in that colour would be byte-identical to the
        // frame behind it, so a resolved surface equal to the window fill is re-derived a visible
        // step away from it, while a colour the caller set still wins.
        let rail = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != rail)
            .unwrap_or_else(|| rail.blend(&secondary, 0.45));

        let rail_width = rect.width.min(RAIL_MAX_WIDTH);

        // ── Rail background ──
        //
        // Painted before the empty check: a rail with nothing in it is still a rail, and hiding it
        // made the control invisible rather than empty. The strip is at most `RAIL_MAX_WIDTH` wide:
        // a navigation rail is a fixed-width edge strip, and painting the caller's whole rectangle
        // made it a full-width panel with the items stranded at its left edge.
        context.fill_rect(Rect::new(rect.x, rect.y, rail_width, rect.height), rail);
        context.draw_line(
            Point::new(rect.x + rail_width as i32 - 1, rect.y),
            Point::new(rect.x + rail_width as i32 - 1, rect.y + rect.height as i32),
            border,
        );

        if self.items.is_empty() {
            return;
        }

        let item_height = RAIL_ITEM_HEIGHT;
        let icon_font = role_font(style, 14.0);
        let label_font = role_font(style, 12.0);

        for (i, item) in self.items.iter().enumerate() {
            let item_y = rect.y + (i as u32 * item_height) as i32;
            let is_selected = i == self.selected_index;

            // ── Selected indicator bar ──
            if is_selected {
                context.fill_rect(
                    crate::core::Rect::new(rect.x, item_y + 8, 4, item_height.saturating_sub(16)),
                    primary,
                );
            }

            // ── Selected background tint ──
            if is_selected {
                context.fill_rounded_rect(
                    crate::core::Rect::new(
                        rect.x + 8,
                        item_y + 8,
                        rail_width.saturating_sub(16),
                        item_height.saturating_sub(16),
                    ),
                    8,
                    primary.with_alpha(25),
                );
            }

            // ── Icon ──
            //
            // A named icon (`home`, `search`, …) is resolved to the bundled outline. The text
            // path was the control's *only* icon rendering, so a rail built with icon names
            // painted the word `home` above the label `Home` — a duplicated label, not an icon.
            // The glyph fallback is kept for a caller that passes a character.
            let icon_color = if is_selected { primary } else { secondary };
            let typed_source = self.item_icon_sources.get(i).copied().flatten();
            let icon_side = 24u32;
            let icon_rect = Rect::new(
                rect.x + (rail_width as i32 - icon_side as i32) / 2,
                item_y + 16,
                icon_side,
                icon_side,
            );
            if let Some(source) = typed_source {
                if let Err(error) =
                    crate::widget::paint_icon(context, source.into(), icon_rect, icon_color)
                {
                    log::warn!("navigation rail item {i} icon failed: {error:?}");
                }
            } else if !crate::widget::draw_icon_named(context, icon_rect, icon_color, &item.icon) {
                // A character rather than a registered name: draw it as a mark in the same box.
                context.draw_text_fitted_cow(
                    icon_rect,
                    &item.icon,
                    &icon_font,
                    icon_color,
                    HorizontalAlignment::Center,
                );
            }

            // ── Label ──
            // Centred in a band whose top is the label's top edge: the origin is the glyph box,
            // so `item_y + 44 + height / 2` put every label half a line low.
            let label_color = if is_selected { primary } else { secondary };
            let label_metrics = context.measure_text(&item.label, &label_font);
            let label_band = Rect::new(
                rect.x + 4,
                item_y + 44,
                rail_width.saturating_sub(8),
                label_metrics.height.max(1),
            );
            context.draw_text_fitted_cow(
                label_band,
                &item.label,
                &label_font,
                label_color,
                HorizontalAlignment::Center,
            );
        }
    }
}

impl EventHandler for MaterialNavigationRail {
    /// Interactions are ignored while the control is disabled.
    ///
    /// The disabled state was not consulted, so a host that disabled the control — the
    /// usual way to take a control out of play — still received clicks and value changes
    /// from it.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                if self.items.is_empty() {
                    return;
                }

                let item_height = RAIL_ITEM_HEIGHT;
                for i in 0..self.items.len() {
                    let item_y = rect.y + (i as u32 * item_height) as i32;
                    let item_rect = crate::core::Rect::new(
                        rect.x,
                        item_y,
                        rect.width.min(RAIL_MAX_WIDTH),
                        item_height,
                    );
                    if item_rect.contains_point(*pos) {
                        if let Event::MouseRelease { .. } = event {
                            self.set_selected(i);
                        }
                        return;
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}
