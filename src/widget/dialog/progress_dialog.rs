// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Progress dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::tr;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Progress dialog widget.
/// Progress dialog widget.
///
/// A modal-style dialog that reports the progress of a long operation on an
/// integer range and offers a single cancel affordance. It owns no timer: the
/// caller drives it by calling [`ProgressDialog::set_value`], which is the only
/// way the displayed progress changes.
///
pub struct ProgressDialog {
    base: BaseWidget,
    title: String,
    label_text: String,
    value: i32,
    minimum: i32,
    maximum: i32,
    cancel_button_text: String,
    was_canceled: bool,
    auto_close: bool,
    auto_reset: bool,
    modal: bool,
    /// Signal emitted when the user cancels, either by pressing Escape while
    /// the dialog is enabled or by calling [`ProgressDialog::cancel`] directly.
    ///
    /// Carries no payload; the reason is not distinguishible from the signal
    /// alone. Connect through [`Widget::connection_scope`] so the slot is
    /// disconnected when the dialog is dropped.
    pub canceled: GenericSignal,
}
impl ProgressDialog {
    /// Creates a dialog with an empty title and label, a range of `0..=100`, a
    /// value of `0`, the translated "Cancel" button text, and `auto_close`,
    /// `auto_reset`, and `modal` all `true`.
    ///
    /// `geometry` is in parent-relative logical pixels; the default size hint
    /// is 350x120, which the caller is free to override.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressDialog, geometry, "ProgressDialog"),
            title: String::new(),
            label_text: String::new(),
            value: 0,
            minimum: 0,
            maximum: 100,
            cancel_button_text: tr!("common.button.cancel"),
            was_canceled: false,
            auto_close: true,
            auto_reset: true,
            modal: true,
            canceled: GenericSignal::new(),
        }
    }
    /// Returns the dialog title, drawn in the title bar. Empty by default.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Returns the label drawn above the progress bar. Empty by default.
    pub fn label_text(&self) -> &str {
        &self.label_text
    }
    /// Returns the current value, always within `minimum() ..= maximum()`.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Returns the lower bound of the progress range. Defaults to `0`.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Returns the upper bound of the progress range. Defaults to `100`; this
    /// is also the value at which `auto_close` hides the dialog.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Returns `true` once the user (or [`ProgressDialog::cancel`]) has
    /// cancelled. Cleared again by [`ProgressDialog::reset`].
    pub fn was_canceled(&self) -> bool {
        self.was_canceled
    }
    /// Returns whether reaching the maximum hides the dialog automatically.
    /// Defaults to `true`.
    pub fn auto_close(&self) -> bool {
        self.auto_close
    }
    /// Returns whether cancellation resets the value back to the minimum.
    /// Defaults to `true`.
    pub fn auto_reset(&self) -> bool {
        self.auto_reset
    }
    /// Returns the cancel button caption. Defaults to the translated
    /// `common.button.cancel` string.
    pub fn cancel_button_text(&self) -> &str {
        &self.cancel_button_text
    }
    /// Sets the title bar text and requests a redraw.
    pub fn set_title(&mut self, t: impl Into<String>) {
        self.title = t.into();
        self.base.request_redraw();
    }
    /// Sets the label shown above the progress bar and requests a redraw.
    pub fn set_label_text(&mut self, t: impl Into<String>) {
        self.label_text = t.into();
        self.base.request_redraw();
    }
    /// Sets the lower bound of the progress range.
    ///
    /// The new bound is **not** applied to the current value, so the value can
    /// temporarily sit outside the range until the next
    /// [`ProgressDialog::set_value`] call clamps it. A `min` above `max` makes
    /// [`ProgressDialog::progress_fraction`] report full progress.
    pub fn set_minimum(&mut self, min: i32) {
        self.minimum = min;
        self.base.request_redraw();
    }
    /// Sets the upper bound of the progress range. As with
    /// [`ProgressDialog::set_minimum`], the current value is not re-clamped
    /// until the next `set_value`.
    pub fn set_maximum(&mut self, max: i32) {
        self.maximum = max;
        self.base.request_redraw();
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.minimum = min;
        self.maximum = max;
        self.base.request_redraw();
    }
    /// Toggles automatic hiding at completion; requests a redraw. See
    /// [`ProgressDialog::auto_close`].
    pub fn set_auto_close(&mut self, v: bool) {
        self.auto_close = v;
        self.base.request_redraw();
    }
    /// Toggles automatic reset on restart; requests a redraw. See
    /// [`ProgressDialog::auto_reset`].
    ///
    /// Note the flag is stored but not consulted anywhere in this type:
    /// [`ProgressDialog::cancel`] never resets the value, and
    /// [`ProgressDialog::reset`] is an explicit call. The intended consumer is a
    /// caller coordinating restarts.
    pub fn set_auto_reset(&mut self, v: bool) {
        self.auto_reset = v;
        self.base.request_redraw();
    }
    /// Sets the caption of the cancel button and requests a redraw.
    pub fn set_cancel_button_text(&mut self, t: impl Into<String>) {
        self.cancel_button_text = t.into();
        self.base.request_redraw();
    }
    /// Returns whether the dialog is modal (blocks interaction with the widgets
    /// behind it). Defaults to `true`.
    ///
    /// Records the intent; enforcement is the modal stack in
    /// [`crate::widget::runtime`] (`enter_modal` / `exit_modal`).
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    /// Sets the modality intent. See [`ProgressDialog::is_modal`].
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
        self.base.request_redraw();
    }
    /// Moves the progress value, clamped into `minimum() ..= maximum()`.
    ///
    /// Side effects: when `auto_close` is set and the clamped value reaches the
    /// maximum, the dialog hides itself. Hiding does not emit `canceled`. The
    /// value is set before hiding, so [`ProgressDialog::value`] still reports
    /// the maximum afterwards. A redraw is requested in all cases.
    pub fn set_value(&mut self, value: i32) {
        self.value = ordered_clamp_i32(value, self.minimum, self.maximum);
        if self.auto_close && self.value >= self.maximum {
            self.hide();
        }
        self.base.request_redraw();
    }
    /// Resets the value to the minimum and clears the cancelled flag.
    ///
    /// Does not show the dialog: combine with [`Widget::show`] when reusing a
    /// hidden dialog. The `auto_reset` flag is not consulted here.
    pub fn reset(&mut self) {
        self.value = self.minimum;
        self.was_canceled = false;
    }
    /// Marks the dialog as cancelled, emits `canceled`, and hides the dialog.
    ///
    /// The signal is emitted **before** the dialog is hidden, so a slot that
    /// reads geometry still sees the visible state. Calling `cancel` twice
    /// emits the signal twice.
    pub fn cancel(&mut self) {
        self.was_canceled = true;
        self.canceled.emit();
        self.hide();
    }
    /// Returns progress as a fraction in `0.0..=1.0`, where `0.0` is the
    /// minimum and `1.0` is the maximum.
    ///
    /// An empty or inverted range (`maximum <= minimum`) reports `1.0`
    /// unconditionally, i.e. a single-valued range is treated as complete
    /// rather than as division by zero.
    pub fn progress_fraction(&self) -> f32 {
        let range = self.maximum - self.minimum;
        if range <= 0 {
            return 1.0;
        }
        (self.value - self.minimum) as f32 / range as f32
    }
}
impl Widget for ProgressDialog {
    /// Access to the shared base-widget state; all default trait behaviour
    /// delegates through this.
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    /// Mutable access to the shared base-widget state.
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(350, 120)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `ProgressDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `ProgressDialog`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. Both properties are read-only: the old
/// write layer had no arm for this kind.
impl WidgetProperties for ProgressDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "label_text" => Ok(CapabilityValue::String(self.label_text().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "label_text" => {
                self.set_label_text(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "label_text", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `progress_dialog` publishes.
    ///
    /// All four names carry a value — `set_value` and `set_range` the progress,
    /// `set_title` and `set_label_text` the text — so a payload-less invocation
    /// is reported as needing one rather than being called unknown. Cancelling
    /// is not published as a command, so it has no entry here.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_range" | "set_title" | "set_label_text" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for ProgressDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, .. } if *key == 27 => self.cancel(),
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl ProgressDialog {
    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered**. Painting it verbatim drew the 240x120
    /// census cell as a 240x120 panel whose label, bar and button were then pinned to
    /// literals (`48`, `62`, `rect.height - 40`) written for a taller default size — so the
    /// bar sat in the top third and the cancel button floated well below it.
    /// [`ControlMetrics::painted_box`] caps each axis at the dialog's own intrinsic size and
    /// centres what is left; every band below is derived from this one rect.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }
}

impl Draw for ProgressDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The dialog surface already read the style,
        // but the title bar, the progress track and the button were literals, so a light/dark
        // switch left them unchanged and the census reported the control as theme-blind.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("progress_dialog");
        // `progress_dialog` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A panel painted in that colour would be byte-identical to the frame
        // behind it, so a resolved surface equal to the window fill is re-derived a visible
        // step away from it, the same distinction `Colors::input_background` draws for a
        // field.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.45));
        // The title bar, the progress track and the button are distinct bands on the panel,
        // derived from it so they stay one visible step apart in either appearance.
        let title_bar = surface.blend(&ink, 0.08);
        let track = surface.blend(&ink, 0.12);
        let button_fill = surface.blend(&ink, 0.12);

        // The filled portion of the bar encodes "complete" rather than a surface
        // appearance, so it reads the theme's semantic `success` token instead of the
        // literal green it used to carry. `semantic_color` takes its own lock and returns an
        // owned `Color`, so no guard outlives the call.
        let progress_fill = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .unwrap_or(Color::rgb(6, 176, 37));

        // Rounded by [`dimensions::DIALOG_RADIUS`]; the radius is clamped to the frame so a
        // box smaller than its own corner is not drawn with an inverted one.
        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.fill_rect(rect, surface);
            context.draw_rect(rect, border);
        }
        // Every label is fitted to the band it belongs to and centred on that band through
        // the shared primitive: the title to the title bar, the label to its own row, and
        // the button caption to the button. None of them was bounded before, so a long
        // caption ran past the frame — the raster backends clip that away and the SVG
        // snapshot showed it as drawing outside the picture.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, title_bar);
        // Guarded on the text being non-empty so a titleless dialog emits no `<text …></text>`.
        if !self.title.is_empty() {
            let title_font = Font::default();
            let title_line = context.text_line(title_bar_band, &title_font);
            context.draw_text_fitted(
                Rect::new(
                    rect.x + 8,
                    title_line.y,
                    rect.width.saturating_sub(16),
                    title_line.height.max(1),
                ),
                &self.title,
                &title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // The body is everything the title bar leaves, and the button is the bottom band of
        // it. The label and the bar then take the space above the button, so the three rows
        // stack inside the frame instead of each being placed from the frame's top edge plus
        // a literal.
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let button_band = ControlMetrics::bottom_band(body, dimensions::DIALOG_BUTTON_HEIGHT);
        let content =
            ControlMetrics::content_above_bottom_band(body, dimensions::DIALOG_BUTTON_HEIGHT);
        let label_font = Font::default();
        let label_line_h = context.measure_text("M", &label_font).height.max(1);
        // Label: the top row of the content area, bounded by it rather than by `rect.y + 48`.
        // Guarded on the text being non-empty so a labelless dialog emits no empty `<text>`.
        if !self.label_text.is_empty() {
            let label_band = ControlMetrics::top_band(content, label_line_h);
            let label_line = context.text_line(label_band, &label_font);
            context.draw_text_fitted(
                Rect::new(
                    content.x + 10,
                    label_line.y,
                    content.width.saturating_sub(20),
                    label_line.height.max(1),
                ),
                &self.label_text,
                &label_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // Progress bar: the row below the label, centred in whatever the label row left.
        // The track is chrome and follows the theme; the filled portion is the semantic
        // `success` colour resolved above. The bar is drawn only when that row is non-empty:
        // a zero-height band is an element the SVG backend emits and the rasteriser skips.
        let bar_row = ControlMetrics::content_below_top_band(content, label_line_h);
        let bar_h = 20u32.min(bar_row.height);
        if bar_row.height > 0 && bar_row.width > 20 {
            let bar_band = ControlMetrics::centered_band(
                Rect::new(
                    bar_row.x + 10,
                    bar_row.y,
                    bar_row.width.saturating_sub(20),
                    bar_row.height,
                ),
                bar_h,
            );
            context.fill_rect(bar_band, track);
            context.draw_rect(bar_band, border);
            let fill_w = (bar_band.width as f32 * self.progress_fraction()) as i32;
            if fill_w > 0 {
                context.fill_rect(
                    Rect::new(bar_band.x, bar_band.y, fill_w as u32, bar_band.height),
                    progress_fill,
                );
            }
            // Percentage text. Centred on the bar itself through `draw_text_line`, which
            // re-derives the line box from the band and bounds the fit to it — the old
            // `bar_x + bar_w/2` origin made the caption occupy only its right half and, at
            // 100%, run up to `bar_w/2 + 42` past the bar.
            let pct = (self.progress_fraction() * 100.0) as i32;
            context.draw_text_line(
                bar_band,
                &format!("{pct}%"),
                &Font::default(),
                if track.is_dark() { Color::WHITE } else { ink },
                HorizontalAlignment::Center,
            );
        }
        // Cancel button: the frame's bottom band, centred horizontally and floored at the
        // frame's left edge so a narrow dialog keeps it on screen. The caption and its
        // button are one rectangle, with the label centred and fitted inside it.
        let btn_w = 80i32.min(button_band.width as i32).max(1);
        let btn_x = (rect.x + rect.width as i32 / 2 - btn_w / 2).max(rect.x);
        let btn_rect = Rect::new(btn_x, button_band.y, btn_w as u32, button_band.height.max(1));
        context.fill_rect(btn_rect, button_fill);
        context.draw_rect(btn_rect, border);
        context.draw_text_line(
            btn_rect,
            &self.cancel_button_text,
            &Font::default(),
            ink,
            HorizontalAlignment::Center,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn set_value_clamps_and_auto_closes_on_max() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        dialog.set_range(10, 20);

        dialog.set_value(5);
        assert_eq!(dialog.value(), 10);

        dialog.show();
        dialog.set_value(99);
        assert_eq!(dialog.value(), 20);
        assert!(!dialog.is_visible());
    }

    #[test]
    fn escape_key_cancels_and_emits_signal() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        let canceled = Arc::new(Mutex::new(0usize));
        let canceled_clone = Arc::clone(&canceled);

        dialog.canceled.connect(move || {
            if let Ok(mut n) = canceled_clone.lock() {
                *n += 1;
            }
        });

        dialog.show();
        dialog.handle_event(&Event::key_press(27, 0));
        assert!(dialog.was_canceled());
        assert_eq!(*canceled.lock().expect("canceled lock"), 1);
        assert!(!dialog.is_visible());
    }
}
