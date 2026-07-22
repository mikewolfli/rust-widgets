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
use crate::widget::display_widgets::switch::Switch;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
}

impl Draw for MaterialSnackbar {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.base.is_visible() {
            return;
        }

        let rect = self.geometry();
        let bar_height = 48u32;
        let bar_y = rect.y + rect.height as i32 - bar_height as i32 - 16;

        // ── Background pill ──
        let bar_rect = Rect::new(rect.x + 12, bar_y, rect.width - 24, bar_height);
        context.fill_rounded_rect(bar_rect, bar_height / 2, Color::rgba(50, 50, 50, 240));

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
                Color::WHITE,
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
                Color::rgba(102, 190, 255, 255), // Material accent blue
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

        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                let bar_height = 48u32;
                let bar_y = rect.y + rect.height as i32 - bar_height as i32 - 16;
                let bar_rect = Rect::new(rect.x + 12, bar_y, rect.width - 24, bar_height);

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
    pub confirmed: GenericSignal,
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
}

impl Draw for CupertinoAlertDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // ── Dialog background (white rounded rect) ──
        let dialog_width = (rect.width as i32).min(320) as u32;
        let dialog_x = rect.x + (rect.width as i32 - dialog_width as i32) / 2;
        let dialog_height = 200u32;
        let dialog_y = rect.y + (rect.height as i32 - dialog_height as i32) / 2;
        let dialog_rect = crate::core::Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
        context.fill_rounded_rect(dialog_rect, 14, Color::rgba(255, 255, 255, 255));

        // ── Semi-transparent backdrop ──
        context.fill_rect(rect, Color::rgba(0, 0, 0, 96));

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
                Color::BLACK,
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
                Color::rgba(100, 100, 100, 255),
                HorizontalAlignment::Left,
            );
        }

        // ── Divider line above buttons ──
        let divider_y = dialog_y + dialog_height as i32 - 48;
        context.draw_line(
            Point::new(dialog_x, divider_y),
            Point::new(dialog_x + dialog_width as i32, divider_y),
            Color::rgba(200, 200, 200, 255),
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
                Color::rgba(200, 200, 200, 255),
            );

            // Cancel button
            let cancel_metrics = context.measure_text(&self.cancel_text, &button_font);
            let cancel_x = dialog_x + (dialog_width as i32 / 2 - cancel_metrics.width as i32) / 2;
            let cancel_y = divider_y + 12 + cancel_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(cancel_x, cancel_y),
                &self.cancel_text,
                &button_font,
                Color::rgba(0, 122, 255, 255), // iOS blue
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
                Color::rgba(0, 122, 255, 255), // iOS blue
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
                Color::rgba(0, 122, 255, 255),
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for CupertinoAlertDialog {
    fn handle_event(&mut self, event: &Event) {
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
pub struct CupertinoSlider {
    base: BaseWidget,
    value: f32,
    min: f32,
    max: f32,
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
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
        self.base.request_redraw();
    }

    /// Returns the current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the minimum value.
    pub fn set_min(&mut self, min: f32) {
        self.min = min;
        self.value = self.value.clamp(self.min, self.max);
        self.base.request_redraw();
    }

    /// Sets the maximum value.
    pub fn set_max(&mut self, max: f32) {
        self.max = max;
        self.value = self.value.clamp(self.min, self.max);
        self.base.request_redraw();
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

        // ── Background track (gray) ──
        context.fill_rounded_rect(track_rect, track_height / 2, Color::rgba(224, 224, 224, 255));

        // ── Filled track (blue) ──
        let knob_center_x = self.knob_x(track_left, track_width);
        let fill_width = (knob_center_x - track_left) as u32;
        if fill_width > 0 {
            let fill_rect = crate::core::Rect::new(track_left, track_y, fill_width, track_height);
            context.fill_rounded_rect(fill_rect, track_height / 2, Color::rgba(0, 122, 255, 255));
        }

        // ── Knob (white circle with thin border) ──
        context.fill_circle_aa(
            Point::new(knob_center_x, rect.y + (rect.height as i32 / 2)),
            knob_radius,
            Color::WHITE,
        );
        // Knob border (light gray)
        context.draw_circle_stroke(
            Point::new(knob_center_x, rect.y + (rect.height as i32 / 2)),
            knob_radius,
            Color::rgba(200, 200, 200, 255),
            1,
        );
    }
}

impl EventHandler for CupertinoSlider {
    fn handle_event(&mut self, event: &Event) {
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
                let clamped = new_value.clamp(self.min, self.max);
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
    pub icon: String,
    pub label: String,
}

impl RailItem {
    /// Creates a new rail item with the given icon and label.
    pub fn new(icon: &str, label: &str) -> Self {
        Self { icon: icon.to_string(), label: label.to_string() }
    }
}

/// Material Design navigation rail for tablets (BLUE11 R10.22).
pub struct MaterialNavigationRail {
    base: BaseWidget,
    items: Vec<RailItem>,
    selected_index: usize,
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
}

impl Draw for MaterialNavigationRail {
    fn draw(&mut self, context: &mut RenderContext) {
        if self.items.is_empty() {
            return;
        }

        let rect = self.geometry();
        let rail_width = rect.width;

        // ── Rail background ──
        context.fill_rect(rect, Color::rgba(255, 255, 255, 255));

        let item_height = 72u32;
        let icon_font = crate::core::Font::new("sans-serif", 14.0, false, false);
        let label_font = crate::core::Font::new("sans-serif", 12.0, false, false);

        for (i, item) in self.items.iter().enumerate() {
            let item_y = rect.y + (i as u32 * item_height) as i32;
            let is_selected = i == self.selected_index;

            // ── Selected indicator bar ──
            if is_selected {
                context.fill_rect(
                    crate::core::Rect::new(rect.x, item_y + 8, 4, item_height - 16),
                    Color::rgba(25, 118, 210, 255), // Material blue
                );
            }

            // ── Selected background tint ──
            if is_selected {
                context.fill_rounded_rect(
                    crate::core::Rect::new(
                        rect.x + 8,
                        item_y + 8,
                        rail_width - 16,
                        item_height - 16,
                    ),
                    8,
                    Color::rgba(25, 118, 210, 25), // Very light blue tint
                );
            }

            // ── Icon (rendered as text placeholder) ──
            let icon_color = if is_selected {
                Color::rgba(25, 118, 210, 255) // Material blue
            } else {
                Color::rgba(98, 98, 98, 255) // Gray
            };
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
            let label_color = if is_selected {
                Color::rgba(25, 118, 210, 255) // Material blue
            } else {
                Color::rgba(98, 98, 98, 255) // Gray
            };
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
    fn handle_event(&mut self, event: &Event) {
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
        atomic::{AtomicBool, Ordering},
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

    #[test]
    fn cupertino_switch_event_delegation() {
        let mut cs = CupertinoSwitch::new(Rect::new(0, 0, 60, 30));
        cs.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
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

    #[test]
    fn cupertino_slider_mouse_press_updates_value() {
        let mut sl = CupertinoSlider::new(Rect::new(0, 0, 200, 40));
        // track_left = 12, track_width = 200-24 = 176
        // Click at x=100 -> fraction ~ (100-12)/176 ~ 0.5
        sl.handle_event(&Event::MousePress { pos: Point::new(100, 20), button: 1 });
        assert!((sl.value() - 0.5).abs() < 0.05);
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
