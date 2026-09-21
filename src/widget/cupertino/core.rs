// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cupertino (iOS-style) widget collection.
//!
//! This module provides iOS-style wrappers around existing widgets. These
//! style aliases apply Cupertino design language (colors, typography, and
//! interaction patterns) while delegating all widget mechanics to the
//! underlying control.

use crate::core::HorizontalAlignment;
use crate::core::Point;
use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_f32, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::display_widgets::switch::Switch;
use crate::widget::numeric::ordered_clamp;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// ── CupertinoSwitch ──────────────────────────────────────────────────────────

/// iOS-style switch (alias for [`Switch`] with iOS coloring).
///
/// `CupertinoSwitch` wraps a [`Switch`] and configures it with iOS design
/// language — green track when checked with `rgb(52, 199, 89)`. All widget
/// behavior (toggling, signals, event handling) is delegated to the inner
/// switch.
pub struct CupertinoSwitch(pub Switch);

impl CupertinoSwitch {
    /// Creates a new Cupertino-styled switch with the given geometry.
    ///
    /// The underlying [`Switch`] is initialized with iOS green
    /// (`Color::rgba(52, 199, 89, 255)`) as the active track color.
    pub fn new(geometry: Rect) -> Self {
        let sw = Switch::new(geometry);
        // Set iOS green via the switch's drawing — the Switch already
        // uses Color::rgba(52, 199, 89, 200) internally when checked.
        // This wrapper ensures the Cupertino branding is explicit.
        Self(sw)
    }

    /// Returns a shared reference to the inner [`Switch`].
    pub fn inner(&self) -> &Switch {
        &self.0
    }

    /// Returns a mutable reference to the inner [`Switch`].
    pub fn inner_mut(&mut self) -> &mut Switch {
        &mut self.0
    }
}

impl Widget for CupertinoSwitch {
    fn base(&self) -> &BaseWidget {
        self.0.base()
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        self.0.base_mut()
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(50, 30)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::CupertinoSwitch
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CupertinoSwitch`'s property contract.
///
/// The control is a `Switch` restricted to the iOS design language, so it
/// forwards every name to the inner switch and only overrides the *kind* it
/// reports. That indirection is the whole difference: `switch` and
/// `cupertino_switch` are two registered names for two `WidgetKind` variants that
/// share one implementation.
impl WidgetProperties for CupertinoSwitch {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        self.0.get(name)
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        self.0.set(name, value)
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SWITCH_PROPERTIES`, which `cupertino_switch_capability` reuses.
        self.0.property_names()
    }

    /// Runs one of the commands `cupertino_switch` publishes.
    ///
    /// The segment's `commands` list is the shared `SWITCH_PROPERTIES` contract, so
    /// the dispatch is forwarded to the inner `Switch` rather than re-derived here:
    /// `switch` and `cupertino_switch` are two names for one control, and letting
    /// them answer differently is the drift this forward exists to prevent. A
    /// wholesale forward — including the `Unknown` fallback — is used so a name
    /// added to the inner contract cannot become silently unreachable through this
    /// spelling.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        self.0.command(name)
    }
}

impl Draw for CupertinoSwitch {
    fn draw(&mut self, context: &mut RenderContext) {
        self.0.draw(context);
    }
}

impl EventHandler for CupertinoSwitch {
    fn handle_event(&mut self, event: &Event) {
        self.0.handle_event(event);
    }
}

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
        }
    }

    /// Sets the message text displayed in the snackbar.
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.base.request_redraw();
    }

    /// Returns the current message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets the action button text. Pass an empty string to hide the action button.
    pub fn set_action_text(&mut self, text: &str) {
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
            self.base.request_redraw();
        }
    }

    /// Dismisses the snackbar. Emits the `dismissed` signal.
    ///
    /// `dismissed` is deliberately not gated by `enabled`: it reports the snackbar leaving
    /// the screen (a timeout or a host decision), not a user action, and a host that
    /// disabled the snackbar's interactions still needs to know it went away. Hiding the
    /// snackbar does block the signal, which is the condition `dismiss` actually depends on.
    pub fn dismiss(&mut self) {
        if self.base.is_visible() {
            self.base.hide();
            self.dismissed.emit();
            self.base.request_redraw();
        }
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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 48)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["message", "action_text", BASE_PROPERTY_NAMES]
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
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("material_snackbar");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::theme::global_theme_manager();
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
        let bar_y = rect.y + rect.height as i32 - bar_height as i32 - 16;

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
            let font = crate::core::Font::new("sans-serif", 14.0, false, false);
            let metrics = context.measure_text(&self.message, &font);

            let text_x = bar_rect.x + 16;
            let text_y = bar_rect.y + (bar_height as i32 / 2) + (metrics.ascent as i32 / 2)
                - (metrics.descent as i32 / 2);

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
            let action_font = crate::core::Font::new("sans-serif", 14.0, true, false);
            let metrics = context.measure_text(&self.action_text, &action_font);

            let action_x = bar_rect.x + bar_rect.width as i32 - metrics.width as i32 - 16;
            let action_y = bar_rect.y + (bar_height as i32 / 2) + (metrics.ascent as i32 / 2)
                - (metrics.descent as i32 / 2);

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
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
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
                    let action_font = crate::core::Font::new("sans-serif", 14.0, true, false);
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

// ── CupertinoAlertDialog ──────────────────────────────────────────────────────

/// iOS-style alert dialog (BLUE11 R10.21).
pub struct CupertinoAlertDialog {
    base: BaseWidget,
    title: String,
    message: String,
    /// Primary action button text (e.g. "OK").
    confirm_text: String,
    /// Cancel button text (e.g. "Cancel"). Empty = no cancel.
    cancel_text: String,
    /// Emitted with no payload when the user confirms the dialog. Read
    /// `CupertinoAlertDialog::confirm_text()` to know which action was shown.
    pub confirmed: GenericSignal,
    /// Emitted with no payload when the user cancels. A dialog constructed with
    /// an empty `cancel_text` still declares this signal but has no cancel
    /// affordance to emit it.
    pub cancelled: GenericSignal,
}

impl CupertinoAlertDialog {
    /// Creates a new CupertinoAlertDialog with the given geometry.
    pub fn new(geometry: crate::core::Rect) -> Self {
        let base =
            BaseWidget::new(WidgetKind::CupertinoAlertDialog, geometry, "CupertinoAlertDialog");
        Self {
            base,
            title: String::new(),
            message: String::new(),
            confirm_text: "OK".to_string(),
            cancel_text: "Cancel".to_string(),
            confirmed: GenericSignal::new(),
            cancelled: GenericSignal::new(),
        }
    }

    /// Sets the title text.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the message text.
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.base.request_redraw();
    }

    /// Returns the message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets the confirm button text.
    pub fn set_confirm_text(&mut self, text: &str) {
        self.confirm_text = text.to_string();
        self.base.request_redraw();
    }

    /// Sets the cancel button text. Empty string hides the cancel button.
    pub fn set_cancel_text(&mut self, text: &str) {
        self.cancel_text = text.to_string();
        self.base.request_redraw();
    }
}

impl Widget for CupertinoAlertDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(270, 150)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::CupertinoAlertDialog
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CupertinoAlertDialog`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. Both properties now report
/// the dialog's real text instead of the placeholder defaults that dispatch
/// returned.
impl WidgetProperties for CupertinoAlertDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "message" => Ok(CapabilityValue::String(self.message().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(&expect_string(value)?);
                Ok(())
            }
            "message" => {
                self.set_message(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "message", BASE_PROPERTY_NAMES]
    }
}

impl Draw for CupertinoAlertDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be
        // a literal, so a light/dark switch left the panel, its title, its message, its
        // separators and its buttons unchanged — the rendering census reported the control
        // as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("cupertino_alert_dialog");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme. The dialog is not in the role table, so it
        // classifies as `Surface` and its resolved background is the window fill itself; the
        // panel below therefore derives its own distinct surface rather than painting the
        // window's. The confirm/cancel actions are the dialog's accent-coloured affordances,
        // so they read the theme's primary token rather than iOS blue.
        let (window_fill, foreground, primary, background) = {
            let manager = crate::theme::global_theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.background,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(0, 122, 255),
                    Color::rgb(240, 240, 240),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The panel is a raised surface over the scrim below, so it is one step from the
        // window fill toward the text colour: a lighter card on a light theme, a darker one
        // on a dark theme. The filter is on the **resolved** value, not only on the theme's:
        // the active theme is applied to every control before it is drawn, so
        // `style.background_color` already holds `Surface`'s window fill and letting it
        // through unfiltered is exactly the invisible-panel defect this guards against. A
        // caller's own colour still wins.
        let panel_from_theme = window_fill.blend(&ink, 0.10);
        let panel = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => panel_from_theme,
        };
        // The separator has no theme token of its own (a `Surface` role resolves only
        // background/text/border), so it is derived one visible step from the panel.
        let separator = theme
            .as_ref()
            .and_then(|t| t.border_color)
            .filter(|resolved| *resolved != panel)
            .unwrap_or_else(|| panel.blend(&ink, 0.22));
        // The action colour: the theme's primary, contrast-checked so the label stays legible
        // on it. A caller's explicit text colour still wins.
        let action = style.text_color.map(|_| ink).unwrap_or_else(|| primary.contrast_color());
        // The scrim is the page's own colour seen through a dark wash, mixed here rather than
        // pushed as a translucent fill. A translucent fill cannot dim anything on this surface:
        // `fill_rect` writes raw pixels, so `rgba(0, 0, 0, 96)` replaced the page with black at
        // alpha 96 and the *panel* — which is painted first — became fully transparent (the
        // census reported the control's dominant colour as `0,0,0`). Mixing the two colours
        // produces the same dimming with an opaque result, and it tracks the appearance instead
        // of being a fixed black wash.
        let scrim = panel.blend(&background, 0.62);

        // ── Dialog background ──
        let dialog_width = (rect.width as i32).min(320) as u32;
        let dialog_x = rect.x + (rect.width as i32 - dialog_width as i32) / 2;
        let dialog_height = 200u32;
        let dialog_y = rect.y + (rect.height as i32 - dialog_height as i32) / 2;
        let dialog_rect = crate::core::Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        // ── Backdrop, then panel ──
        context.fill_rect(rect, scrim);
        context.fill_rounded_rect(dialog_rect, 14, panel);

        // ── Title ──
        let title_font = crate::core::Font::new("sans-serif", 17.0, true, false);
        if !self.title.is_empty() {
            let title_metrics = context.measure_text(&self.title, &title_font);
            let title_x = dialog_x + (dialog_width as i32 - title_metrics.width as i32) / 2;
            let title_y = dialog_y + 24 + title_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(title_x, title_y),
                &self.title,
                &title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // ── Message ──
        let msg_font = crate::core::Font::new("sans-serif", 14.0, false, false);
        if !self.message.is_empty() {
            let msg_metrics = context.measure_text(&self.message, &msg_font);
            let msg_x = dialog_x + (dialog_width as i32 - msg_metrics.width as i32) / 2;
            let msg_y = dialog_y + 56 + msg_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(msg_x, msg_y),
                &self.message,
                &msg_font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // ── Divider line above buttons ──
        let divider_y = dialog_y + dialog_height as i32 - 48;
        context.draw_line(
            Point::new(dialog_x, divider_y),
            Point::new(dialog_x + dialog_width as i32, divider_y),
            separator,
        );

        // ── Buttons ──
        let button_font = crate::core::Font::new("sans-serif", 17.0, false, false);
        let has_cancel = !self.cancel_text.is_empty();

        if has_cancel {
            // Two buttons: cancel on left, confirm on right
            // Vertical divider between them
            let mid_x = dialog_x + dialog_width as i32 / 2;
            context.draw_line(
                Point::new(mid_x, divider_y),
                Point::new(mid_x, dialog_y + dialog_height as i32),
                separator,
            );

            // Cancel button
            let cancel_metrics = context.measure_text(&self.cancel_text, &button_font);
            let cancel_x = dialog_x + (dialog_width as i32 / 2 - cancel_metrics.width as i32) / 2;
            let cancel_y = divider_y + 12 + cancel_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(cancel_x, cancel_y),
                &self.cancel_text,
                &button_font,
                action,
                HorizontalAlignment::Left,
            );

            // Confirm button
            let confirm_metrics = context.measure_text(&self.confirm_text, &button_font);
            let confirm_x = dialog_x
                + dialog_width as i32 / 2
                + (dialog_width as i32 / 2 - confirm_metrics.width as i32) / 2;
            let confirm_y = divider_y + 12 + confirm_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(confirm_x, confirm_y),
                &self.confirm_text,
                &button_font,
                action,
                HorizontalAlignment::Left,
            );
        } else {
            // Single confirm button centered
            let confirm_metrics = context.measure_text(&self.confirm_text, &button_font);
            let confirm_x = dialog_x + (dialog_width as i32 - confirm_metrics.width as i32) / 2;
            let confirm_y = divider_y + 12 + confirm_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(confirm_x, confirm_y),
                &self.confirm_text,
                &button_font,
                action,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for CupertinoAlertDialog {
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
            Event::MouseRelease { pos, button } | Event::MousePress { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                let dialog_width = (rect.width as i32).min(320) as u32;
                let dialog_x = rect.x + (rect.width as i32 - dialog_width as i32) / 2;
                let dialog_height = 200u32;
                let dialog_y = rect.y + (rect.height as i32 - dialog_height as i32) / 2;
                let divider_y = dialog_y + dialog_height as i32 - 48;

                // Check if click is in button area
                if pos.y < divider_y || pos.y > dialog_y + dialog_height as i32 {
                    return;
                }
                if pos.x < dialog_x || pos.x > dialog_x + dialog_width as i32 {
                    return;
                }

                let has_cancel = !self.cancel_text.is_empty();
                if has_cancel {
                    let mid_x = dialog_x + dialog_width as i32 / 2;
                    if pos.x < mid_x {
                        // Cancel clicked
                        if let Event::MouseRelease { .. } = event {
                            self.cancelled.emit();
                        }
                    } else {
                        // Confirm clicked
                        if let Event::MouseRelease { .. } = event {
                            self.confirmed.emit();
                        }
                    }
                } else {
                    // Confirm clicked
                    if let Event::MouseRelease { .. } = event {
                        self.confirmed.emit();
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

// ── CupertinoSlider ───────────────────────────────────────────────────────────

/// iOS-style slider (BLUE11 R10.21).
///
/// A range slider over a floating-point value. The range and value are plain
/// `f32`s; nothing here requires `min <= max`, and the current value is not
/// re-clamped when the bounds change.
pub struct CupertinoSlider {
    base: BaseWidget,
    value: f32,
    min: f32,
    max: f32,
    /// Emitted with the new value whenever it changes — both on a programmatic
    /// [`CupertinoSlider::set_value`] that moves the knob and on a user drag.
    /// The widget does not handle continuous drag-move, only press-to-jump; a
    /// caller needing live drag updates drives [`CupertinoSlider::set_value`]
    /// itself, which emits here too, so this signal is the single notification
    /// channel out regardless of who changed the value.
    pub value_changed: Signal1<f32>,
}

impl CupertinoSlider {
    /// Creates a new CupertinoSlider with the given geometry.
    /// Default range: 0.0 to 1.0, value at 0.0.
    pub fn new(geometry: crate::core::Rect) -> Self {
        let base = BaseWidget::new(WidgetKind::CupertinoSlider, geometry, "CupertinoSlider");
        Self { base, value: 0.0, min: 0.0, max: 1.0, value_changed: Signal1::new() }
    }

    /// Sets the current value, clamped to [min, max].
    ///
    /// Emits `value_changed` with the clamped value when it actually changes;
    /// re-applying the same clamped value is a no-op.
    ///
    /// `value_changed` is deliberately not gated by `enabled`: the pointer path over this
    /// slider is gated in `handle_event` (the round-52 contract), so a disabled slider
    /// already ignores the user; this writer is the *programmatic* path, and a host that
    /// two-way-binds the value needs the signal even while the control is disabled.
    pub fn set_value(&mut self, value: f32) {
        let clamped = ordered_clamp(value, self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(clamped);
        }
        self.base.request_redraw();
    }

    /// Returns the current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the minimum value, re-applying the clamp to the current value so an
    /// out-of-range value is pulled into range (and `value_changed` is emitted
    /// when it moves).
    pub fn set_min(&mut self, min: f32) {
        self.min = min;
        self.set_value(self.value);
    }

    /// Sets the maximum value, re-applying the clamp to the current value so an
    /// out-of-range value is pulled into range (and `value_changed` is emitted
    /// when it moves).
    pub fn set_max(&mut self, max: f32) {
        self.max = max;
        self.set_value(self.value);
    }

    /// Returns the minimum value.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Returns the maximum value.
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Computes the knob center x position based on current value.
    fn knob_x(&self, track_left: i32, track_width: i32) -> i32 {
        if self.max <= self.min {
            return track_left;
        }
        let fraction = (self.value - self.min) / (self.max - self.min);
        track_left + (fraction * track_width as f32) as i32
    }

    /// Computes value from a pixel x position.
    fn value_from_x(&self, x: i32, track_left: i32, track_width: i32) -> f32 {
        if track_width <= 0 || self.max <= self.min {
            return self.min;
        }
        let fraction = ((x - track_left) as f32 / track_width as f32).clamp(0.0, 1.0);
        self.min + fraction * (self.max - self.min)
    }
}

impl Widget for CupertinoSlider {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::CupertinoSlider
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CupertinoSlider`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `f32` → `f64` widening on read.
impl WidgetProperties for CupertinoSlider {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Float(self.value().into())),
            "min" => Ok(CapabilityValue::Float(self.min().into())),
            "max" => Ok(CapabilityValue::Float(self.max().into())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_f32(value)?);
                Ok(())
            }
            "min" => {
                self.set_min(expect_f32(value)?);
                Ok(())
            }
            "max" => {
                self.set_max(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["value", "min", "max", BASE_PROPERTY_NAMES]
    }
}

impl Draw for CupertinoSlider {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        let track_height = 4u32;
        let knob_radius = 12u32;
        let track_y = rect.y + (rect.height as i32 / 2) - (track_height as i32 / 2);
        let track_left = rect.x + knob_radius as i32;
        let track_width = rect.width as i32 - 2 * knob_radius as i32;

        if track_width <= 0 {
            return;
        }

        let track_rect =
            crate::core::Rect::new(track_left, track_y, track_width as u32, track_height);

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Without the theme step a
        // light/dark switch changed nothing on screen — the empty track and the knob were
        // hardcoded, which the rendering census reported as theme-blind. The knob is the
        // largest area the slider paints, so it is what the census measures as dominant.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("cupertino_slider");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme. `cupertino_slider` is not in the role
        // table, so it classifies as `Surface` and its resolved background is the window
        // fill itself; the surface below therefore derives its own distinct colour rather
        // than painting the window's. The filled run of the track is a value indicator, the
        // same role `Slider`'s own fill plays, so it reads the theme's accent.
        let (window_fill, foreground, accent, muted) = {
            let manager = crate::theme::global_theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.accent,
                    active.colors.secondary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The empty run of the track, one step from the window fill toward the text colour,
        // so it is visible on either appearance rather than being the window's own colour.
        let empty_track = window_fill.blend(&ink, 0.14);
        // The filter is on the **resolved** value, not only on the theme's: the active theme
        // is applied to every control before it is drawn, so `style.background_color` already
        // holds `Surface`'s window fill and letting it through unfiltered is exactly the
        // invisible-control defect this guards against. A caller's own colour still wins.
        let track_color = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => empty_track,
        };

        // ── Background track ──
        context.fill_rounded_rect(track_rect, track_height / 2, track_color);

        // ── Filled track ──
        let knob_center_x = self.knob_x(track_left, track_width);
        let fill_width = (knob_center_x - track_left) as u32;
        if fill_width > 0 {
            let fill_rect = crate::core::Rect::new(track_left, track_y, fill_width, track_height);
            context.fill_rounded_rect(fill_rect, track_height / 2, accent);
        }

        // ── Knob (circle with a thin border) ──
        // The two-step surface `Input` uses: a light knob on a light theme, a dark one on a
        // dark theme, with the luminance inverted from the surface so the disc stays visible.
        let knob_fill = if track_color.is_dark() {
            track_color.blend(&Color::WHITE, 0.30)
        } else {
            track_color.blend(&Color::WHITE, 0.85)
        };
        context.fill_circle_aa(
            Point::new(knob_center_x, rect.y + (rect.height as i32 / 2)),
            knob_radius,
            knob_fill,
        );
        // Knob border, one visible step from the knob itself.
        let knob_border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != knob_fill)
            .unwrap_or_else(|| knob_fill.blend(&muted, 0.40));
        context.draw_circle_stroke(
            Point::new(knob_center_x, rect.y + (rect.height as i32 / 2)),
            knob_radius,
            knob_border,
            1,
        );
    }
}

impl EventHandler for CupertinoSlider {
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
            Event::MousePress { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                let knob_radius = 12u32;
                let track_left = rect.x + knob_radius as i32;
                let track_width = rect.width as i32 - 2 * knob_radius as i32;

                if track_width <= 0 {
                    return;
                }

                let new_value = self.value_from_x(pos.x, track_left, track_width);
                let clamped = ordered_clamp(new_value, self.min, self.max);
                if (clamped - self.value).abs() > f32::EPSILON {
                    self.value = clamped;
                    self.value_changed.emit(clamped);
                    self.base.request_redraw();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
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

/// Material Design navigation rail for tablets (BLUE11 R10.22).
///
/// A vertically stacked set of destinations with one selected at a time. The
/// selected index always names an item: it defaults to `0` even while the rail
/// is empty, so it is out of range until the first item is added.
pub struct MaterialNavigationRail {
    base: BaseWidget,
    items: Vec<RailItem>,
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
        Self { base, items: Vec::new(), selected_index: 0, selected_changed: Signal1::new() }
    }

    /// Adds an item with the given icon and label.
    pub fn add_item(&mut self, icon: &str, label: &str) {
        self.items.push(RailItem::new(icon, label));
        self.base.request_redraw();
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
        self.selected_index = 0;
        self.base.request_redraw();
    }
}

impl Widget for MaterialNavigationRail {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(72, 400)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::MaterialNavigationRail
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("material_navigation_rail");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::theme::global_theme_manager();
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

        let rail_width = rect.width;

        // ── Rail background ──
        //
        // Painted before the empty check: a rail with nothing in it is still a rail, and hiding it
        // made the control invisible rather than empty.
        context.fill_rect(rect, rail);
        context.draw_line(
            Point::new(rect.x + rail_width as i32 - 1, rect.y),
            Point::new(rect.x + rail_width as i32 - 1, rect.y + rect.height as i32),
            border,
        );

        if self.items.is_empty() {
            return;
        }

        let item_height = 72u32;
        let icon_font = crate::core::Font::new("sans-serif", 14.0, false, false);
        let label_font = crate::core::Font::new("sans-serif", 12.0, false, false);

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

            // ── Icon (rendered as text placeholder) ──
            let icon_color = if is_selected { primary } else { secondary };
            let icon_metrics = context.measure_text(&item.icon, &icon_font);
            let icon_x = rect.x + (rail_width as i32 - icon_metrics.width as i32) / 2;
            let icon_y = item_y + 20 + icon_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(icon_x, icon_y),
                &item.icon,
                &icon_font,
                icon_color,
                HorizontalAlignment::Left,
            );

            // ── Label ──
            let label_color = if is_selected { primary } else { secondary };
            let label_metrics = context.measure_text(&item.label, &label_font);
            let label_x = rect.x + (rail_width as i32 - label_metrics.width as i32) / 2;
            let label_y = item_y + 44 + label_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(label_x, label_y),
                &item.label,
                &label_font,
                label_color,
                HorizontalAlignment::Left,
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
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                if self.items.is_empty() {
                    return;
                }

                let item_height = 72u32;
                for i in 0..self.items.len() {
                    let item_y = rect.y + (i as u32 * item_height) as i32;
                    let item_rect = crate::core::Rect::new(rect.x, item_y, rect.width, item_height);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    // ── CupertinoSwitch tests ──

    #[test]
    fn cupertino_switch_new_defaults() {
        let cs = CupertinoSwitch::new(Rect::new(0, 0, 60, 30));
        assert_eq!(cs.inner().kind(), WidgetKind::Switch);
        assert_eq!(cs.kind(), WidgetKind::CupertinoSwitch);
        assert!(!cs.inner().is_checked());
    }

    #[test]
    fn cupertino_switch_toggle_delegates() {
        let mut cs = CupertinoSwitch::new(Rect::new(0, 0, 60, 30));
        assert!(!cs.inner().is_checked());
        cs.inner_mut().toggle();
        assert!(cs.inner().is_checked());
    }

    /// A completed pointer activation on the wrapper toggles the inner switch.
    ///
    /// The bare `MouseRelease` this used to dispatch asserted the defect (see
    /// `Switch::handle_event`); the delegation intent is unchanged.
    #[test]
    fn cupertino_switch_event_delegation() {
        let mut cs = CupertinoSwitch::new(Rect::new(0, 0, 60, 30));
        let p = Point::new(10, 10);
        cs.handle_event(&Event::MousePress { pos: p, button: 1 });
        cs.handle_event(&Event::MouseRelease { pos: p, button: 1 });
        assert!(cs.inner().is_checked());
    }

    #[test]
    fn cupertino_switch_svg_output() {
        let mut cs = CupertinoSwitch::new(Rect::new(0, 0, 60, 30));
        let svg = render_to_svg(&mut cs);
        assert!(svg.starts_with("<svg"));
    }

    // ── MaterialSnackbar tests ──

    #[test]
    fn material_snackbar_creation() {
        let sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        assert_eq!(sb.kind(), WidgetKind::MaterialSnackbar);
        assert!(!sb.is_visible());
        assert!(sb.message().is_empty());
        assert!(sb.action_text().is_empty());
    }

    #[test]
    fn material_snackbar_message_accessors() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        sb.set_message("File saved");
        assert_eq!(sb.message(), "File saved");
    }

    #[test]
    fn material_snackbar_action_text_accessors() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        sb.set_action_text("Undo");
        assert_eq!(sb.action_text(), "Undo");
    }

    #[test]
    fn material_snackbar_show_hide() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        assert!(!sb.is_visible());

        sb.show();
        assert!(sb.is_visible());

        sb.dismiss();
        assert!(!sb.is_visible());
    }

    #[test]
    fn material_snackbar_dismiss_emits_signal() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        sb.set_message("Hello");
        sb.show();

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        sb.dismissed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Click on snackbar to dismiss
        sb.handle_event(&Event::MousePress { pos: Point::new(200, 80), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
        assert!(!sb.is_visible());
    }

    #[test]
    fn material_snackbar_action_pressed_emits_signal() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        sb.set_message("File deleted");
        sb.set_action_text("Undo");
        sb.show();

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        sb.action_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Click on action area (right side of bar)
        // Bar width = 375 - 24 = 351, action area is last ~metrics_width+32 px
        // Approximate action area at x > 200
        sb.handle_event(&Event::MousePress { pos: Point::new(340, 80), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn material_snackbar_hidden_blocks_events() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        sb.set_message("Test");

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        sb.dismissed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Should not fire since snackbar is hidden
        sb.handle_event(&Event::MousePress { pos: Point::new(200, 80), button: 1 });
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn material_snackbar_svg_output() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        sb.set_message("Hello");
        sb.set_action_text("Action");
        sb.show();
        let svg = render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn material_snackbar_svg_output_hidden() {
        let mut sb = MaterialSnackbar::new(Rect::new(0, 0, 375, 100));
        // Hidden — should still produce valid SVG
        let svg = render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
    }

    // ── CupertinoAlertDialog tests ──

    #[test]
    fn cupertino_alert_dialog_creation() {
        let dlg = CupertinoAlertDialog::new(Rect::new(0, 0, 400, 300));
        assert_eq!(dlg.kind(), WidgetKind::CupertinoAlertDialog);
        assert!(dlg.title().is_empty());
        assert!(dlg.message().is_empty());
    }

    #[test]
    fn cupertino_alert_dialog_title_accessors() {
        let mut dlg = CupertinoAlertDialog::new(Rect::new(0, 0, 400, 300));
        dlg.set_title("Delete File");
        assert_eq!(dlg.title(), "Delete File");
    }

    #[test]
    fn cupertino_alert_dialog_message_accessors() {
        let mut dlg = CupertinoAlertDialog::new(Rect::new(0, 0, 400, 300));
        dlg.set_message("Are you sure?");
        assert_eq!(dlg.message(), "Are you sure?");
    }

    #[test]
    fn cupertino_alert_dialog_confirm_signal() {
        let mut dlg = CupertinoAlertDialog::new(Rect::new(0, 0, 400, 300));
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        dlg.confirmed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Click on confirm button area (right half of button row)
        // Dialog x = (400-320)/2 = 40, dialog_width = 320, dialog_y = (300-200)/2 = 50
        // divider_y = 50+200-48 = 202
        // Button area: y 202..250, x 40..360
        // Confirm is right half: x 200..360
        dlg.handle_event(&Event::MouseRelease { pos: Point::new(300, 225), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn cupertino_alert_dialog_cancel_signal() {
        let mut dlg = CupertinoAlertDialog::new(Rect::new(0, 0, 400, 300));
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        dlg.cancelled.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Click on cancel button area (left half of button row)
        dlg.handle_event(&Event::MouseRelease { pos: Point::new(100, 225), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn cupertino_alert_dialog_svg_output() {
        let mut dlg = CupertinoAlertDialog::new(Rect::new(0, 0, 400, 300));
        dlg.set_title("Notice");
        dlg.set_message("Hello world");
        let svg = render_to_svg(&mut dlg);
        assert!(svg.starts_with("<svg"));
    }

    // ── CupertinoSlider tests ──

    #[test]
    fn cupertino_slider_creation() {
        let sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        assert_eq!(sl.kind(), WidgetKind::CupertinoSlider);
        assert_eq!(sl.value(), 0.0);
        assert_eq!(sl.min(), 0.0);
        assert_eq!(sl.max(), 1.0);
    }

    #[test]
    fn cupertino_slider_value_get_set() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        sl.set_value(0.5);
        assert!((sl.value() - 0.5).abs() < 1e-6);

        // Clamping
        sl.set_value(2.0);
        assert!((sl.value() - 1.0).abs() < 1e-6);

        sl.set_value(-1.0);
        assert!((sl.value() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn cupertino_slider_min_max_accessors() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        sl.set_max(100.0);
        sl.set_min(10.0);
        assert!((sl.min() - 10.0).abs() < 1e-6);
        assert!((sl.max() - 100.0).abs() < 1e-6);

        // Value should be clamped
        sl.set_value(5.0);
        assert!((sl.value() - 10.0).abs() < 1e-6);
    }

    /// Setting `min` above the current `max` must clamp, not abort the process.
    ///
    /// A slider is created with `(min, max) = (0.0, 1.0)`. Writing `min = 3.5`
    /// through the public setter — which is precisely what the capability layer's
    /// `write_property(w, "min", …)` does, and therefore what a JSON document or a
    /// C-ABI caller does — used to reach `value.clamp(3.5, 1.0)`. `f32::clamp`
    /// panics when `min > max`, so a caller who set two numbers in the "wrong"
    /// order killed the host. Setter order is not something a GUI library may put
    /// a precondition on.
    #[test]
    fn cupertino_slider_min_above_max_clamps_instead_of_panicking() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        sl.set_min(3.5);
        // The bounds may now be crossed; the value must still be a number inside
        // the range the caller named, and reading it must not panic either.
        let value = sl.value();
        assert!(value.is_finite(), "value must remain finite after crossed bounds, got {value}");
        assert!(
            (1.0..=3.5).contains(&value),
            "value {value} is outside the ordered range [1.0, 3.5]"
        );

        // The mirrored case: `max` below the current `min`.
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        sl.set_min(5.0);
        sl.set_max(2.0);
        let value = sl.value();
        assert!(value.is_finite(), "value must remain finite after crossed bounds, got {value}");
    }

    /// A `NaN` bound must be ignored rather than poison the value.
    #[test]
    fn cupertino_slider_nan_bound_does_not_poison_the_value() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        sl.set_value(0.5);
        sl.set_min(f32::NAN);
        let value = sl.value();
        assert!(!value.is_nan(), "a NaN bound must not make the value NaN; got {value}");
    }

    #[test]
    fn cupertino_slider_mouse_press_updates_value() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        // track_left = 12, track_width = 200-24 = 176
        // Click at x=100 -> fraction ~ (100-12)/176 ~ 0.5
        sl.handle_event(&Event::MousePress { pos: Point::new(100, 20), button: 1 });
        assert!((sl.value() - 0.5).abs() < 0.05);
    }

    #[test]
    fn cupertino_slider_set_value_emits_changed_only_on_real_change() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        let seen = Arc::new(AtomicUsize::new(0));
        {
            let c = seen.clone();
            sl.value_changed.connect(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        sl.set_value(0.5);
        assert_eq!(seen.load(Ordering::SeqCst), 1);
        sl.set_value(0.5); // no-op: same clamped value
        assert_eq!(seen.load(Ordering::SeqCst), 1);
        sl.set_value(0.9);
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn cupertino_slider_mouse_press_different_position() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        // Press at a different position
        sl.handle_event(&Event::MousePress { pos: Point::new(150, 20), button: 1 });
        assert!((sl.value() - 0.78).abs() < 0.05);
    }

    #[test]
    fn cupertino_slider_svg_output() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        let svg = render_to_svg(&mut sl);
        assert!(svg.starts_with("<svg"));
    }

    // ── MaterialNavigationRail tests ──

    #[test]
    fn material_navigation_rail_creation() {
        let rail = MaterialNavigationRail::new(Rect::new(0, 0, 80, 400));
        assert_eq!(rail.kind(), WidgetKind::MaterialNavigationRail);
        assert_eq!(rail.item_count(), 0);
        assert_eq!(rail.selected(), 0);
    }

    #[test]
    fn material_navigation_rail_add_items() {
        let mut rail = MaterialNavigationRail::new(Rect::new(0, 0, 80, 400));
        rail.add_item("\u{1f3e0}", "Home");
        rail.add_item("\u{2b50}", "Favorites");
        rail.add_item("\u{2699}", "Settings");
        assert_eq!(rail.item_count(), 3);
    }

    #[test]
    fn material_navigation_rail_selection() {
        let mut rail = MaterialNavigationRail::new(Rect::new(0, 0, 80, 400));
        rail.add_item("A", "Item A");
        rail.add_item("B", "Item B");
        rail.add_item("C", "Item C");

        assert_eq!(rail.selected(), 0);
        rail.set_selected(1);
        assert_eq!(rail.selected(), 1);
        rail.set_selected(5); // Clamped
        assert_eq!(rail.selected(), 2);
    }

    #[test]
    fn material_navigation_rail_clear_items() {
        let mut rail = MaterialNavigationRail::new(Rect::new(0, 0, 80, 400));
        rail.add_item("A", "Item A");
        rail.add_item("B", "Item B");
        assert_eq!(rail.item_count(), 2);
        rail.clear_items();
        assert_eq!(rail.item_count(), 0);
        assert_eq!(rail.selected(), 0);
    }

    #[test]
    fn material_navigation_rail_svg_output() {
        let mut rail = MaterialNavigationRail::new(Rect::new(0, 0, 80, 400));
        rail.add_item("\u{1f3e0}", "Home");
        rail.add_item("\u{2b50}", "Stars");
        let svg = render_to_svg(&mut rail);
        assert!(svg.starts_with("<svg"));
    }
}
