// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Input dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::numeric::{ordered_clamp_f64, ordered_clamp_i64};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Input dialog input mode.
///
/// The mode selects which of the dialog's parallel value slots the input field
/// displays and edits; the other slots keep their values but are not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Free text, edited through [`InputDialog::text_value`].
    Text,
    /// A whole number, clamped to the `int_min`/`int_max` range set by
    /// [`InputDialog::get_int`] and stepped by `int_step`.
    Integer,
    /// A real number, clamped to the `double_min`/`double_max` range and shown
    /// rounded to `double_decimals` places.
    Double,
    /// A choice from [`InputDialog::items`], navigated by
    /// [`InputDialog::current_item`].
    Item,
}

/// The stable property spelling of [`InputMode`].
///
/// The capability schema for `mode` is `PropertySchema::enumerated(..)` over
/// `["text", "integer", "double", "item"]`, and those are the exact strings
/// returned here. Keeping one function as the single source of truth means the
/// read route, the write route and the schema cannot drift apart: a value the
/// schema advertises but this function does not produce would be a property the
/// caller can see listed and never read.
fn mode_name(mode: InputMode) -> &'static str {
    match mode {
        InputMode::Text => "text",
        InputMode::Integer => "integer",
        InputMode::Double => "double",
        InputMode::Item => "item",
    }
}

/// Parses the stable property spelling of [`InputMode`].
///
/// Returns [`CapabilityAccessError::OutOfRange`] rather than silently falling back
/// to a default: a caller that misspells the mode has asked for something this
/// control cannot do, and the schema already told it which four strings are valid.
fn mode_from_name(name: &str) -> Result<InputMode, CapabilityAccessError> {
    match name {
        "text" => Ok(InputMode::Text),
        "integer" => Ok(InputMode::Integer),
        "double" => Ok(InputMode::Double),
        "item" => Ok(InputMode::Item),
        _ => Err(CapabilityAccessError::OutOfRange),
    }
}
/// Input dialog for simple user input.
pub struct InputDialog {
    base: BaseWidget,
    modal: bool,
    title: String,
    label_text: String,
    mode: InputMode,
    text_value: String,
    int_value: i64,
    double_value: f64,
    items: Vec<String>,
    current_item: usize,
    int_min: i64,
    int_max: i64,
    int_step: i64,
    double_min: f64,
    double_max: f64,
    _double_step: f64,
    double_decimals: u8,
    /// Emitted when the text value changes. Nothing in this widget emits it
    /// yet — editing happens elsewhere and calls [`InputDialog::set_text_value`]
    /// — so it is for a host that drives the field.
    pub text_value_changed: Signal1<String>,
    /// Emitted when the integer value changes. Not emitted by this widget yet;
    /// see [`InputDialog::text_value_changed`].
    pub int_value_changed: Signal1<i64>,
    /// Emitted when the floating-point value changes. Not emitted by this widget
    /// yet; see [`InputDialog::text_value_changed`].
    pub double_value_changed: Signal1<f64>,
    /// Emitted by [`InputDialog::accept`].
    pub accepted: GenericSignal,
    /// Emitted by [`InputDialog::reject`].
    pub rejected: GenericSignal,
}
impl InputDialog {
    /// Creates a modal, empty dialog in [`InputMode::Text`].
    ///
    /// The title and label are empty, the item list is empty, and the numeric
    /// ranges are left wide open (the full `i64`/`f64` ranges, step 1). Set what
    /// you need afterwards, or use one of the configured constructors.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::InputDialog, geometry, "InputDialog"),
            modal: true,
            title: String::new(),
            label_text: String::new(),
            mode: InputMode::Text,
            text_value: String::new(),
            int_value: 0,
            double_value: 0.0,
            items: Vec::new(),
            current_item: 0,
            int_min: i64::MIN,
            int_max: i64::MAX,
            int_step: 1,
            double_min: f64::MIN,
            double_max: f64::MAX,
            _double_step: 1.0,
            double_decimals: 1,
            text_value_changed: Signal1::new(),
            int_value_changed: Signal1::new(),
            double_value_changed: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    /// Creates a text-input dialog with its title, label and initial text
    /// preset, in [`InputMode::Text`].
    ///
    /// `default` seeds [`InputDialog::text_value`]; it is not a placeholder — an
    /// empty `default` means the field opens genuinely empty.
    pub fn get_text(
        geometry: Rect,
        title: impl Into<String>,
        label: impl Into<String>,
        default: impl Into<String>,
    ) -> Self {
        let mut d = Self::new(geometry);
        d.title = title.into();
        d.label_text = label.into();
        d.text_value = default.into();
        d.mode = InputMode::Text;
        d
    }
    /// Creates a whole-number dialog preset to `value`, bounded by `min` and
    /// `max`, in [`InputMode::Integer`].
    ///
    /// `value` is clamped into the inclusive range as the dialog is built, so a
    /// request outside it is silently brought inside; read
    /// [`InputDialog::int_value`] to see what was actually taken. `step` is stored
    /// but nothing in this widget applies increments, so it does not affect the
    /// value.
    ///
    /// # Crossed bounds
    ///
    /// The `min`/`max` arguments are *inputs*, so `get_int(.., 500, 100, 0, ..)`
    /// is a call a caller can make, and `i64::clamp` **panics** on `min > max`.
    /// The two bounds are therefore ordered once here and the ordered pair is both
    /// stored and clamped against, so a descending range yields the same answer as
    /// its ascending twin instead of taking the process down. `value` is `i64`, so
    /// `i64::MIN`/`i64::MAX` remain usable as open bounds.
    pub fn get_int(
        geometry: Rect,
        title: impl Into<String>,
        label: impl Into<String>,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
    ) -> Self {
        let mut d = Self::new(geometry);
        d.title = title.into();
        d.label_text = label.into();
        d.int_min = min.min(max);
        d.int_max = min.max(max);
        d.int_value = value.clamp(d.int_min, d.int_max);
        d.int_step = step;
        d.mode = InputMode::Integer;
        d
    }
    /// The dialog's title, drawn in its header bar.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// The text drawn beside the input field to say what is being asked for.
    pub fn label_text(&self) -> &str {
        &self.label_text
    }
    /// Which value the input field currently presents.
    pub fn mode(&self) -> InputMode {
        self.mode
    }
    /// The free-text value. This is the value the caller cares about after an
    /// [`InputMode::Text`] dialog is accepted; it exists in every mode but is
    /// only displayed in `Text`.
    pub fn text_value(&self) -> &str {
        &self.text_value
    }
    /// The whole-number value, clamped to the `int_min`/`int_max` range.
    ///
    /// Read this after an [`InputMode::Integer`] dialog is accepted. It holds a
    /// meaningful value in every mode, but only the `Integer` mode displays it.
    pub fn int_value(&self) -> i64 {
        self.int_value
    }
    /// The floating-point value, clamped to the `double_min`/`double_max` range.
    ///
    /// Stays `0.0` unless set: no constructor here seeds it, and it is only
    /// displayed in [`InputMode::Double`].
    pub fn double_value(&self) -> f64 {
        self.double_value
    }
    /// The index of the selected item, or `0` when there are no items.
    ///
    /// Always an index, never an optional — check [`InputDialog::items`] to tell
    /// "nothing to choose from" from "the first item is chosen".
    pub fn current_item(&self) -> usize {
        self.current_item
    }
    /// The choices offered in [`InputMode::Item`]. Empty unless
    /// [`InputDialog::set_items`] was called.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// The selected item's text, or `None` when the list is empty or the index
    /// no longer addresses an entry.
    pub fn current_item_text(&self) -> Option<&str> {
        self.items.get(self.current_item).map(|s| s.as_str())
    }
    /// Sets the title and repaints.
    pub fn set_title(&mut self, t: impl Into<String>) {
        self.title = t.into();
        self.base.request_redraw();
    }
    /// Sets the label drawn beside the input field and repaints.
    pub fn set_label_text(&mut self, t: impl Into<String>) {
        self.label_text = t.into();
        self.base.request_redraw();
    }
    /// Switches which value the input field presents, and repaints.
    ///
    /// The other values are unaffected: switching away from a mode does not
    /// clear what it held, so switching back shows it again.
    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
        self.base.request_redraw();
    }
    /// Sets the free-text value and repaints. Overwrites rather than appends, so
    /// it cannot be used for incremental typing.
    ///
    /// Emits [`Self::text_value_changed`] when the value actually differs. The
    /// signal is named `_changed`, so emitting on a no-op write would be a lie: a
    /// caller using it to drive an expensive downstream update would run that
    /// update for a write that changed nothing.
    pub fn set_text_value(&mut self, v: impl Into<String>) {
        let value = v.into();
        if value == self.text_value {
            return;
        }
        self.text_value = value;
        self.base.request_redraw();
        self.text_value_changed.emit(self.text_value.clone());
    }
    /// Replaces the item list and repaints.
    ///
    /// Resets the selection to index 0, so a previously chosen item is lost even
    /// if it is still present in the new list.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.current_item = 0;
        self.base.request_redraw();
    }
    /// Sets the whole-number value, clamped into the current
    /// `int_min`/`int_max` range, and repaints.
    ///
    /// Emits [`Self::int_value_changed`] when the *stored* value changes, so a
    /// request that was clamped back to what was already there is not reported as
    /// a change.
    pub fn set_int_value(&mut self, v: i64) {
        let clamped = ordered_clamp_i64(v, self.int_min, self.int_max);
        if clamped == self.int_value {
            return;
        }
        self.int_value = clamped;
        self.base.request_redraw();
        self.int_value_changed.emit(self.int_value);
    }

    /// Sets the floating-point value, clamped into the current
    /// `double_min`/`double_max` range, and repaints.
    ///
    /// A `NaN` is rejected without touching the stored value: it cannot be clamped
    /// into a range (every comparison is false), so accepting it would leave the
    /// control outside its own declared bounds. Emits
    /// [`Self::double_value_changed`] on a real change.
    pub fn set_double_value(&mut self, v: f64) {
        if v.is_nan() {
            return;
        }
        let clamped = ordered_clamp_f64(v, self.double_min, self.double_max);
        if clamped == self.double_value {
            return;
        }
        self.double_value = clamped;
        self.base.request_redraw();
        self.double_value_changed.emit(self.double_value);
    }
    /// Whether the dialog blocks interaction with its owner while open.
    ///
    /// Records the intent, on by default; enforcement is the modal stack in
    /// [`crate::widget::runtime`] (`enter_modal` / `exit_modal`).
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    /// Sets the modality flag and repaints. See [`InputDialog::is_modal`].
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
        self.base.request_redraw();
    }
    /// Confirms the dialog: emits `accepted`, then hides it.
    ///
    /// Unlike the file dialog this does not emit the value: the caller reads
    /// [`InputDialog::text_value`], [`InputDialog::int_value`],
    /// [`InputDialog::double_value`] or [`InputDialog::current_item`] according
    /// to [`InputDialog::mode`]. Pressing Enter (key code 13) on an enabled,
    /// visible dialog does the same.
    pub fn accept(&mut self) {
        self.accepted.emit();
        self.hide();
    }
    /// Cancels the dialog and hides it, emitting `rejected`.
    ///
    /// The values are **not** cleared, so a cancelled dialog still reports what
    /// was in its fields. Pressing Escape (key code 27) has the same effect.
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
}
impl Widget for InputDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(350, 150)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `InputDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `InputDialog`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. Both properties are read-only: the old
/// write layer had no arm for this kind.
impl WidgetProperties for InputDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "label_text" => Ok(CapabilityValue::String(self.label_text().to_string())),
            // The four typed value properties. The schema in
            // `INPUT_DIALOG_PROPERTIES` has always declared them, but neither the
            // read nor the write arm existed, so the capability table published
            // three commands (`set_mode`, `set_text_value`, `set_int_value`,
            // `set_double_value`) against properties that answered
            // `UnknownProperty` for a control that really does support them.
            "mode" => Ok(CapabilityValue::String(mode_name(self.mode()).to_string())),
            "text_value" => Ok(CapabilityValue::String(self.text_value().to_string())),
            "int_value" => Ok(CapabilityValue::Int(self.int_value())),
            "double_value" => Ok(CapabilityValue::Float(self.double_value())),
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
            "mode" => {
                let requested = expect_string(value)?;
                let mode = mode_from_name(&requested)?;
                self.set_mode(mode);
                Ok(())
            }
            "text_value" => {
                let text = expect_string(value)?;
                self.set_text_value(text);
                Ok(())
            }
            "int_value" => {
                let number = match value {
                    CapabilityValue::Int(v) => v,
                    CapabilityValue::Float(v) => v as i64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_int_value(number);
                Ok(())
            }
            "double_value" => {
                let number = match value {
                    CapabilityValue::Float(v) => v,
                    CapabilityValue::Int(v) => v as f64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_double_value(number);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "label_text",
            "mode",
            "text_value",
            "int_value",
            "double_value",
            BASE_PROPERTY_NAMES
        ]
    }
}
impl EventHandler for InputDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
            if *key == 13 {
                self.accept();
            } else if *key == 27 {
                self.reject();
            }
        }
    }
}
impl InputDialog {
    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered**. Painting it verbatim drew the 240x120
    /// census cell as a 240x120 frame whose title bar, label, field and buttons were then
    /// pinned to literals (`48`, `60`, `rect.height - 40`) written for the 400 px default
    /// size — so the rows sat in the top half and the buttons floated away from them.
    /// [`ControlMetrics::painted_box`] caps each axis at the dialog's own intrinsic size
    /// and centres what is left; every band below is derived from this one rect.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }
}

impl Draw for InputDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        let style = self.style().clone();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. The style step alone
        // was not enough: `WidgetStyle` carries no title-bar or accent field, so the two
        // most visible pixels of the dialog — the title band and the accept button —
        // stayed a hardcoded blue in either appearance, and the render census reported
        // the whole control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let theme = crate::style::resolved_theme_style("input_dialog");
        // The window fill and the accent are read as their own lock acquisition and copied
        // out as values, so the guard is dropped before anything else touches the theme —
        // the global manager's mutex is not re-entrant.
        let (window_fill, accent) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (active.colors.background, active.colors.primary),
                None => (Color::WHITE, Color::rgb(0, 120, 215)),
            }
        };
        let accent_ink = accent.contrast_color();

        // A dialog is a `Surface`-role control and `Surface` resolves to the window's own
        // fill, which would leave the frame invisible against the window. A resolved
        // surface equal to the window fill is therefore re-derived one step toward the
        // ink, the same distinction `Colors::input_background` draws for a field.
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
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
            .unwrap_or(Color::rgb(160, 160, 160));
        // The entry field is the editable interior: one step *away* from the dialog
        // surface, so on a light theme darker and on a dark one lighter rather than the
        // forced white it used to be.
        let field = if surface.is_dark() {
            surface.blend(&Color::WHITE, 0.08)
        } else {
            surface.blend(&Color::BLACK, 0.06)
        };

        // Rounded by [`dimensions::DIALOG_RADIUS`] so the dialog reads as the same class of
        // object as its neighbours; the radius is clamped to the frame so a box smaller
        // than its own corner is not drawn with an inverted one.
        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.fill_rect(rect, surface);
            context.draw_rect(rect, border);
        }
        // Title bar: a separate region from the dialog surface, in the theme's accent
        // rather than the literal blue it carried before. The label is fitted to the bar,
        // so a long title truncates at the bar's edge instead of running past the frame, and
        // it is centred on the bar's own band through the shared primitive rather than at a
        // literal `y`. The strip comes from `top_band`, which keeps its thickness.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, accent);
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
                accent_ink,
                HorizontalAlignment::Left,
            );
        }
        // The body is everything the title bar leaves. Its rows are derived from the body
        // rather than from the frame's top edge plus a literal, so the label sits above the
        // field it names at any frame size and the two cannot drift apart.
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let label_font = Font::default();
        let label_line_h = context.measure_text("M", &label_font).height.max(1);
        // Label. Its box is the row above the entry field, which is what bounds a label
        // longer than the dialog instead of the dialog's own width. Guarded on the text
        // being non-empty so a labelless dialog emits no `<text …></text>`.
        let label_band = ControlMetrics::top_band(body, label_line_h);
        if !self.label_text.is_empty() {
            let label_line = context.text_line(label_band, &label_font);
            context.draw_text_fitted(
                Rect::new(
                    body.x + 10,
                    label_line.y,
                    body.width.saturating_sub(20),
                    label_line.height.max(1),
                ),
                &self.label_text,
                &label_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // Input field: the band below the label, its height the field's own rather than a
        // literal `26` written at the frame's top plus `60`.
        let field_band = ControlMetrics::content_below_top_band(body, label_line_h);
        let input_band = ControlMetrics::top_band(field_band, field_band.height.min(26));
        context.fill_rect(input_band, field);
        context.draw_rect(input_band, border);
        let display_text = match self.mode {
            InputMode::Text => self.text_value.clone(),
            InputMode::Integer => self.int_value.to_string(),
            InputMode::Double => {
                format!("{:.prec$}", self.double_value, prec = self.double_decimals as usize)
            }
            InputMode::Item => self.current_item_text().unwrap_or("").to_string(),
        };
        // Guarded: an unguarded draw of an empty value emits `<text …></text>`, an element
        // the rasteriser never produces. The line box is centred on the field through the
        // shared primitive, which the old `input_y + ((26 - h) / 2)` re-derived by hand.
        if !display_text.is_empty() {
            let input_font = Font::default();
            let input_line = context.text_line(input_band, &input_font);
            context.draw_text_fitted(
                Rect::new(
                    input_band.x + 4,
                    input_line.y,
                    input_band.width.saturating_sub(8),
                    input_line.height.max(1),
                ),
                &display_text,
                &input_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // OK/Cancel. The pair is the frame's bottom band, right-aligned inside it through the
        // shared [`action_row_geometry`] derivation — the same function `message_box`,
        // `file_dialog`, `font_dialog`, `color_dialog` and `progress_dialog` draw their pair
        // with, so the six cannot disagree about the button width, the gap or the row's left
        // edge. Taking the row from `bottom_band` is what stops the buttons floating up at a
        // tall frame (the old `rect.height - 40`) or clipping at a short one.
        let button_band = ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT);
        let labels = vec![tr!("common.button.ok"), tr!("common.button.cancel")];
        let row = super::message_box::action_row_geometry(context, &labels, button_band, true);
        let font = Font::default();
        let ok_rect = row.buttons[0];
        context.fill_rect(ok_rect, accent);
        context.draw_text_line(ok_rect, &labels[0], &font, accent_ink, HorizontalAlignment::Center);
        let cancel_rect = row.buttons[1];
        context.fill_rect(cancel_rect, surface.blend(&ink, 0.1));
        context.draw_rect(cancel_rect, border);
        context.draw_text_line(cancel_rect, &labels[1], &font, ink, HorizontalAlignment::Center);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn get_int_clamps_and_sets_integer_mode() {
        let dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 500, 0, 100, 5);
        assert_eq!(dialog.mode(), InputMode::Integer);
        assert_eq!(dialog.int_value(), 100);
    }

    /// Regression: `min`/`max` are `get_int` *parameters*, so a caller can cross
    /// them. `i64::clamp` panics on `min > max`, and it did so here during
    /// construction — before the dialog existed to be inspected. The pair is now
    /// ordered as it is stored, so a descending range behaves like its ascending
    /// twin.
    #[test]
    fn get_int_with_crossed_bounds_does_not_panic() {
        let dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 500, 100, 0, 5);
        assert_eq!(dialog.int_value(), 100, "500 clamps into the ordered span [0, 100]");
        assert_eq!(dialog.mode(), InputMode::Integer);

        // A value below both bounds clamps up, in either written order.
        let dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", -50, 100, 0, 5);
        assert_eq!(dialog.int_value(), 0);
    }

    /// The same defect through the mutator, which clamps against whatever the
    /// dialog is currently holding.
    #[test]
    fn set_int_value_survives_a_collapsed_range() {
        let mut dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 0, 0, 0, 1);
        dialog.set_int_value(42);
        assert_eq!(dialog.int_value(), 0, "the collapsed span [0, 0] pulls the value to 0");

        let mut dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 0, 10, 10, 1);
        dialog.set_int_value(-5);
        assert_eq!(dialog.int_value(), 10, "the collapsed span [10, 10] pulls the value to 10");
    }

    /// `set_double_value` rejects `NaN` outright rather than clamping it. Keep
    /// that pinned: `NaN` compares false against every bound, so "clamping" it
    /// would store a value the control cannot place inside its own range.
    #[test]
    fn set_double_value_rejects_nan_and_clamps_otherwise() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        dialog.set_double_value(2.5);
        assert_eq!(dialog.double_value(), 2.5);
        dialog.set_double_value(f64::NAN);
        assert_eq!(dialog.double_value(), 2.5, "NaN must leave the stored value untouched");
    }

    #[test]
    fn enter_and_escape_emit_accept_reject() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        let accepted = Arc::new(Mutex::new(0usize));
        let rejected = Arc::new(Mutex::new(0usize));

        let a = Arc::clone(&accepted);
        dialog.accepted.connect(move || {
            if let Ok(mut n) = a.lock() {
                *n += 1;
            }
        });

        let r = Arc::clone(&rejected);
        dialog.rejected.connect(move || {
            if let Ok(mut n) = r.lock() {
                *n += 1;
            }
        });

        dialog.show();
        dialog.handle_event(&Event::key_press(13, 0));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        dialog.handle_event(&Event::key_press(27, 0));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
        assert!(!dialog.is_visible());
    }
}
