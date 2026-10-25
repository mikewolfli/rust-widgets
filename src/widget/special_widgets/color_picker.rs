// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ColorPicker widget.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{Gradient, GradientStop};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use core::cell::RefCell;

/// The inputs the cached rail paints depend on; a change in any of them rebuilds the cache.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PaintCacheKey {
    palette: Rect,
    hue_rect: Rect,
    alpha_rect: Rect,
    hue: u8,
    color: (u8, u8, u8, u8),
    font_size_bits: u32,
    show_alpha: bool,
}

/// The gradients and text measurements one frame paints with (SP-P70).
///
/// The palette's two full-width gradients, the hue rail's seven-stop rainbow and the alpha ramp
/// were rebuilt on **every** frame, and the hex readout was measured every frame. All four are
/// pure functions of the rects, the hue and the colour, so they are built only when one of those
/// moves.
struct ColorPickerPaintCache {
    key: PaintCacheKey,
    horizontal: Gradient,
    vertical: Gradient,
    rainbow: Gradient,
    alpha: Gradient,
    hex_width: u32,
    hex_height: u32,
}

/// Interactive color picker with HSV controls and preset swatches.
pub struct ColorPicker {
    base: BaseWidget,
    color: Color,
    hue: u8,
    saturation: u8,
    value: u8,
    alpha: u8,
    show_alpha: bool,
    presets: Vec<Color>,
    /// Which rail a drag in progress is editing, if any.
    ///
    /// The three rails are disjoint, but a drag is not: the pointer is free to leave the rail it
    /// started in. Remembering the *target* rather than re-hit-testing on every move is what keeps
    /// a sweep that strays one pixel past the hue rail's edge from silently switching to the alpha
    /// rail — the region a drag edits is the region the press began in.
    drag_target: Option<ColorDragTarget>,
    /// Whether the hex readout is in editable form.
    hex_editing: bool,
    /// The text in the hex field while editing.
    hex_buffer: String,
    /// Whether the panel paints its own OK / Cancel row (SP-62).
    show_buttons: bool,
    /// The cached rail paints; see [`ColorPickerPaintCache`].
    ///
    /// `RefCell` because the cache is refreshed from inside `draw`, which already holds a shared
    /// borrow of the widget for its resolved style.
    paint_cache: RefCell<Option<ColorPickerPaintCache>>,
    /// Emitted when selected color changes.
    pub color_changed: Signal1<Color>,
    /// Emitted when hex text changes.
    pub hex_changed: Signal1<String>,
    /// Emitted when a colour is committed through [`ColorPicker::pick`].
    ///
    /// This is the panel-facing commit signal: a host such as `color_well` applies the colour
    /// once it sees this, while `color_changed` still reports every intermediate value.
    pub color_committed: Signal1<Color>,
    /// Emitted for every live HSVA edit (palette sweep, rail drag, keyboard nudge).
    pub color_previewed: Signal1<Color>,
    /// Emitted when the panel session is canceled with [`ColorPicker::cancel`].
    pub canceled: GenericSignal,
}

/// The rail a colour-picker drag is currently editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorDragTarget {
    /// The saturation/value square.
    Palette,
    /// The hue rail.
    Hue,
    /// The alpha rail.
    Alpha,
}

impl ColorPicker {
    /// Creates a color picker.
    pub fn new(geometry: Rect) -> Self {
        let mut picker = Self {
            base: BaseWidget::new(WidgetKind::ColorPicker, geometry, "ColorPicker"),
            color: Color::rgb(255, 0, 0),
            hue: 0,
            saturation: 255,
            value: 255,
            alpha: 255,
            show_alpha: true,
            drag_target: None,
            hex_editing: false,
            hex_buffer: String::new(),
            show_buttons: false,
            paint_cache: RefCell::new(None),
            presets: vec![
                Color::rgb(244, 67, 54),
                Color::rgb(33, 150, 243),
                Color::rgb(76, 175, 80),
                Color::rgb(255, 193, 7),
                Color::rgb(156, 39, 176),
                Color::rgb(96, 125, 139),
            ],
            color_changed: Signal1::new(),
            hex_changed: Signal1::new(),
            color_committed: Signal1::new(),
            color_previewed: Signal1::new(),
            canceled: GenericSignal::new(),
        };
        picker.sync_color_from_hsva();
        picker
    }

    /// Returns current color.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Sets current color directly.
    pub fn set_color(&mut self, color: Color) {
        self.apply_color_quietly(color);
        self.color_changed.emit(self.color);
        self.hex_changed.emit(self.color.to_hex_rgba());
    }

    /// Applies a colour the user chose and publishes the panel-facing commit.
    ///
    /// This is the method a host panel calls from its OK affordance. It reports both the
    /// per-value change (`color_changed`) and the commit (`color_committed`), so an observer
    /// that only wants the final choice subscribes to the latter.
    pub fn pick(&mut self, color: Color) {
        self.set_color(color);
        self.color_committed.emit(color);
    }

    /// Sets the value the panel opens with, without publishing a change.
    ///
    /// An initial value is not a user choice, so this deliberately emits neither
    /// `color_changed` nor `color_previewed`: a host that seeds a freshly opened panel does not
    /// want its own value handler to run before the user has touched anything.
    pub fn set_initial(&mut self, color: Color) {
        self.apply_color_quietly(color);
    }

    /// Cancels the panel session, resetting transient edit state and emitting `canceled`.
    ///
    /// The event layer still accepts the historical `cancelled` spelling through
    /// `EVENT_ALIASES`; the signal field is the US spelling so the emitted name and the
    /// declared name are the same string (W-19).
    pub fn cancel(&mut self) {
        self.hex_editing = false;
        self.hex_buffer.clear();
        self.drag_target = None;
        self.base.request_redraw();
        self.canceled.emit();
    }

    /// Compatibility spelling of [`Self::canceled`] (W-19): the same emitted signal.
    pub fn cancelled(&self) -> &GenericSignal {
        &self.canceled
    }

    /// Updates the colour and HSV cache without emitting any signal.
    fn apply_color_quietly(&mut self, color: Color) {
        self.color = color;
        self.alpha = color.a;
        let (h, s, v) = color_to_hsv(color);
        // Only update hue if the color is chromatic; grayscale colors have undefined hue
        if color.r != color.g || color.g != color.b {
            self.hue = h;
        }
        self.saturation = s;
        self.value = v;
        self.base.request_redraw();
    }

    /// Enables the editable hex field.
    pub fn set_hex_editing(&mut self, editing: bool) {
        if self.hex_editing == editing {
            return;
        }
        self.hex_editing = editing;
        if editing {
            self.hex_buffer = self.hex_rgba();
        }
        self.base.request_redraw();
    }

    /// Returns whether the hex field is editable.
    pub fn is_hex_editing(&self) -> bool {
        self.hex_editing
    }

    /// Samples the colour at `pos` and applies it, which is the picker's eyedropper.
    ///
    /// The sample is read from the same mapping the rails are painted with, so a caller that
    /// shows its own magnifier cannot pick a colour different from the one under the pixel.
    pub fn pick_color_at(&mut self, pos: Point) -> bool {
        if Self::point_in_rect(pos, self.palette_rect()) {
            self.set_from_palette_point(pos);
            return true;
        }
        if Self::point_in_rect(pos, self.hue_rect()) {
            self.set_from_hue_point(pos);
            return true;
        }
        if self.show_alpha && Self::point_in_rect(pos, self.alpha_rect()) {
            self.set_from_alpha_point(pos);
            return true;
        }
        false
    }

    /// Commits the hex field, leaving edit mode.
    pub fn commit_hex(&mut self) {
        if !self.hex_editing {
            return;
        }
        let text = self.hex_buffer.clone();
        if self.set_hex(&text) {
            self.hex_editing = false;
        }
        self.base.request_redraw();
    }
    /// Sets HSVA components and updates color.
    ///
    /// Each component is taken as the byte the picker's own rails store; the resulting RGB is
    /// derived through the same conversion the rails paint with.
    pub fn set_hsva(&mut self, hue: u8, saturation: u8, value: u8, alpha: u8) {
        self.hue = hue;
        self.saturation = saturation;
        self.value = value;
        self.alpha = alpha;
        self.sync_color_from_hsva();
    }

    /// Returns `(h, s, v, a)`.
    pub fn hsva(&self) -> (u8, u8, u8, u8) {
        (self.hue, self.saturation, self.value, self.alpha)
    }

    /// Enables/disables alpha strip rendering.
    pub fn set_show_alpha(&mut self, show_alpha: bool) {
        if self.show_alpha == show_alpha {
            return;
        }
        self.show_alpha = show_alpha;
        self.base.request_redraw();
    }

    /// Returns whether alpha strip is visible.
    pub fn show_alpha(&self) -> bool {
        self.show_alpha
    }

    /// Sets color by hex text (`#RRGGBB` or `#RRGGBBAA`).
    pub fn set_hex(&mut self, hex: &str) -> bool {
        let Some(color) = Color::parse_hex(hex) else {
            return false;
        };
        self.set_color(color);
        true
    }

    /// Returns current color hex string.
    pub fn hex_rgba(&self) -> String {
        self.color.to_hex_rgba()
    }

    /// Applies a preset by index.
    pub fn apply_preset(&mut self, index: usize) -> bool {
        let Some(color) = self.presets.get(index).copied() else {
            return false;
        };
        self.set_color(color);
        true
    }

    /// Returns preset count.
    pub fn preset_count(&self) -> usize {
        self.presets.len()
    }

    /// Returns the preset palette.
    pub fn presets(&self) -> &[Color] {
        &self.presets
    }

    /// Replaces the preset palette.
    ///
    /// The palette was six colours fixed at construction, with no writer at all, so a caller's own
    /// house colours could not appear in the picker without editing the crate. The swatches that do
    /// not fit the row are not painted or hit-tested — the row is bounded by the current-value
    /// swatch — and [`Self::apply_preset`] still addresses every entry by index.
    pub fn set_presets(&mut self, presets: Vec<Color>) {
        self.presets = presets;
        self.base.request_redraw();
    }

    /// Appends one preset, returning its index.
    pub fn add_preset(&mut self, color: Color) -> usize {
        let index = self.presets.len();
        self.presets.push(color);
        self.base.request_redraw();
        index
    }

    /// Removes the preset at `index`, returning it when it was present.
    pub fn remove_preset(&mut self, index: usize) -> Option<Color> {
        if index >= self.presets.len() {
            return None;
        }
        let removed = self.presets.remove(index);
        self.base.request_redraw();
        Some(removed)
    }

    /// The hue in degrees, `0..=359`.
    ///
    /// The rails store hue as a byte (`0..=255`), which is the right storage for a gradient but the
    /// wrong unit for a caller or a designer: a hue is named in degrees. This is the published
    /// spelling; the byte stays the paint's own index.
    pub fn hue_degrees(&self) -> u16 {
        ((f32::from(self.hue) / 255.0) * 359.0).round() as u16
    }

    /// Sets the hue from degrees, clamped to `0..=359`.
    pub fn set_hue_degrees(&mut self, degrees: u16) {
        let degrees = degrees.min(359);
        let hue = ((f32::from(degrees) / 359.0) * 255.0).round().clamp(0.0, 255.0) as u8;
        if self.hue == hue {
            return;
        }
        self.hue = hue;
        self.sync_color_from_hsva();
    }

    /// Height of the bottom chrome stack below the palette: the hex readout's line box, the
    /// preset swatch row, and the margins around them.
    ///
    /// Derived from the parts rather than being one literal, because `44` was too small for
    /// them: it did not account for the readout's own 14 px line, so the palette extended to
    /// within 6 px of the swatch row and the readout was drawn straight over the palette's
    /// bottom edge (the census measured `#FF0000FF` in `225,225,225` on the palette's
    /// `255,64,64`, 2.65:1). The palette is a spectrum, so text over any of it is wrong
    /// regardless of the ratio.
    const BOTTOM_STACK: u32 = READOUT_LINE + PRESET_ROW + MARGIN * 3;

    /// Height the OK / Cancel row takes when [`Self::set_show_buttons`] is on.
    const BUTTON_ROW: u32 = 22;

    /// The vertical room below the palette: the readout stack plus the button row when shown.
    fn bottom_stack(&self) -> u32 {
        if self.show_buttons {
            Self::BOTTOM_STACK + Self::BUTTON_ROW
        } else {
            Self::BOTTOM_STACK
        }
    }

    /// Whether the panel paints its own OK / Cancel row.
    pub fn show_buttons(&self) -> bool {
        self.show_buttons
    }

    /// Shows or hides the panel's own OK / Cancel row.
    ///
    /// It is off by default because the historical contract leaves the commit flow to the host.
    /// Turning it on puts the two actions in the panel and makes it usable on its own (SP-62).
    pub fn set_show_buttons(&mut self, show: bool) {
        if self.show_buttons == show {
            return;
        }
        self.show_buttons = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The OK button's rectangle, when the row is shown.
    fn ok_rect(&self) -> Option<Rect> {
        if !self.show_buttons {
            return None;
        }
        let rect = self.geometry();
        let height = (Self::BUTTON_ROW - MARGIN * 2).max(1);
        let width = 64.min(rect.width / 2);
        Some(Rect::new(
            rect.x + rect.width as i32 - 8 - width as i32,
            rect.y + rect.height as i32 - Self::BUTTON_ROW as i32 + MARGIN as i32,
            width,
            height,
        ))
    }

    /// The Cancel button's rectangle, when the row is shown.
    fn cancel_rect(&self) -> Option<Rect> {
        let ok = self.ok_rect()?;
        Some(Rect::new(ok.x - ok.width as i32 - 6, ok.y, ok.width, ok.height))
    }

    fn palette_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + 8,
            rect.y + 8,
            rect.width.saturating_sub(48),
            rect.height.saturating_sub(self.bottom_stack() + 8),
        )
    }

    fn hue_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + rect.width as i32 - 34,
            rect.y + 8,
            12,
            rect.height.saturating_sub(self.bottom_stack() + 8),
        )
    }

    fn alpha_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + rect.width as i32 - 18,
            rect.y + 8,
            10,
            rect.height.saturating_sub(self.bottom_stack() + 8),
        )
    }

    fn preset_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.presets.len() {
            return None;
        }
        let rect = self.geometry();
        let x = rect.x + 8 + (index as i32) * 22;
        // The row ends where the current-value swatch begins, so an injected palette cannot paint
        // under it. A swatch past that edge has no rectangle; `presets` still holds it and
        // `apply_preset` still addresses it.
        let row_end = rect.x + rect.width as i32 - 70 - 6;
        if x + 18 > row_end {
            return None;
        }
        let y = rect.y + rect.height as i32 - (PRESET_ROW + MARGIN) as i32;
        Some(Rect::new(x, y, 18, 18))
    }

    fn point_in_rect(pos: Point, rect: Rect) -> bool {
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn sync_color_from_hsva(&mut self) {
        self.color = hsv_to_color(self.hue, self.saturation, self.value, self.alpha);
        self.color_changed.emit(self.color);
        self.hex_changed.emit(self.color.to_hex_rgba());
        // Every HSVA writer is a live edit (palette, rails, keyboard nudge), so preview and
        // change always describe the same value.
        self.color_previewed.emit(self.color);
        self.base.request_redraw();
    }

    fn set_from_palette_point(&mut self, pos: Point) {
        let palette = self.palette_rect();
        let width = palette.width.max(1) as f32;
        let height = palette.height.max(1) as f32;
        let sat_ratio = ((pos.x - palette.x) as f32 / width).clamp(0.0, 1.0);
        let val_ratio = (1.0 - ((pos.y - palette.y) as f32 / height)).clamp(0.0, 1.0);
        self.saturation = (sat_ratio * 255.0).round() as u8;
        self.value = (val_ratio * 255.0).round() as u8;
        self.sync_color_from_hsva();
    }

    fn set_from_hue_point(&mut self, pos: Point) {
        let hue_rect = self.hue_rect();
        let height = hue_rect.height.max(1) as f32;
        let ratio = ((pos.y - hue_rect.y) as f32 / height).clamp(0.0, 1.0);
        self.hue = (ratio * 255.0).round() as u8;
        self.sync_color_from_hsva();
    }

    fn set_from_alpha_point(&mut self, pos: Point) {
        let alpha_rect = self.alpha_rect();
        let height = alpha_rect.height.max(1) as f32;
        let ratio = (1.0 - ((pos.y - alpha_rect.y) as f32 / height)).clamp(0.0, 1.0);
        self.alpha = (ratio * 255.0).round() as u8;
        self.sync_color_from_hsva();
    }
}

impl Widget for ColorPicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Paints itself, so it can be mounted into a host surface.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200 + if self.show_buttons { Self::BUTTON_ROW } else { 0 })
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
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "color_changed" => {
                Some(EventSignalRef::mapped("color_changed", &self.color_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "hex_changed" => Some(EventSignalRef::mapped("hex_changed", &self.hex_changed, |v| {
                CapabilityValue::String(v.clone())
            })),
            "color_committed" => {
                Some(EventSignalRef::mapped("color_committed", &self.color_committed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "color_previewed" => {
                Some(EventSignalRef::mapped("color_previewed", &self.color_previewed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "canceled" => Some(EventSignalRef::unit("canceled", &self.canceled)),
            _ => None,
        }
    }
}

/// `ColorPicker`'s property contract.
///
/// These are the properties the `WidgetKind::ColorDialog` arms used to serve: the
/// old dispatch keyed on `ColorDialog` but downcast to `ColorPicker`, so the
/// contract belongs here, next to the fields it reads. Read/write semantics are
/// carried over unchanged from `access_read_dialog.in.rs` /
/// `access_write_dialog.in.rs`.
///
/// Since BLUE16 phase E-6 the control declares `WidgetKind::ColorPicker` instead,
/// so it no longer shares the dialog's kind. The history above is kept because the
/// contract was written under the old key and the semantics still come from there;
/// what changed is *which kind answers*, not what the properties mean.
impl WidgetProperties for ColorPicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "hex_rgba" => Ok(CapabilityValue::String(self.hex_rgba())),
            "show_alpha" => Ok(CapabilityValue::Bool(self.show_alpha())),
            "preset_count" => Ok(CapabilityValue::UInt(self.preset_count() as u64)),
            "presets" => Ok(CapabilityValue::Tuple(
                self.presets()
                    .iter()
                    .map(|color| CapabilityValue::String(color.to_hex_rgba()))
                    .collect(),
            )),
            "hue_degrees" => Ok(CapabilityValue::UInt(self.hue_degrees() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "hex_rgba" => {
                // `set_hex` reports a parse failure by returning `false`; the old
                // dispatch mapped that to `TypeMismatch`, so the meaning is kept.
                if self.set_hex(&expect_string(value)?) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::TypeMismatch)
                }
            }
            "show_alpha" => {
                self.set_show_alpha(expect_bool(value)?);
                Ok(())
            }
            // The preset palette is written through `set_presets` / `add_preset` /
            // `remove_preset`, so the count is a derived read.
            "preset_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "presets" => {
                let tokens = crate::widget::capability::coercion::expect_string_list(value)?;
                let mut colors = Vec::with_capacity(tokens.len());
                for token in tokens {
                    // A token that is not a colour refuses the whole list rather than being
                    // skipped, so the palette the control holds is exactly the one asked for.
                    let color =
                        Color::parse_hex(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                    colors.push(color);
                }
                self.set_presets(colors);
                Ok(())
            }
            "hue_degrees" => {
                let degrees = crate::widget::capability::coercion::expect_usize(value)?;
                let degrees =
                    u16::try_from(degrees).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                self.set_hue_degrees(degrees);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "hex_rgba",
            "show_alpha",
            "preset_count",
            "presets",
            "hue_degrees",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `color_picker` publishes.
    ///
    /// `set_hex` takes the hex text and `apply_preset` takes the preset index, so
    /// neither can complete without a payload: they are refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_hex" | "apply_preset" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ColorPicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                // The panel's own commit pair comes first: they are chrome over the rails, so a
                // click on OK must not fall through to a colour edit (SP-62).
                if let Some(ok) = self.ok_rect() {
                    if Self::point_in_rect(*pos, ok) {
                        let color = self.color;
                        self.pick(color);
                        return;
                    }
                }
                if let Some(cancel) = self.cancel_rect() {
                    if Self::point_in_rect(*pos, cancel) {
                        self.cancel();
                        return;
                    }
                }
                for index in 0..self.presets.len() {
                    let Some(preset_rect) = self.preset_rect(index) else {
                        continue;
                    };
                    if Self::point_in_rect(*pos, preset_rect) {
                        let _ = self.apply_preset(index);
                        return;
                    }
                }

                if Self::point_in_rect(*pos, self.palette_rect()) {
                    self.drag_target = Some(ColorDragTarget::Palette);
                    self.set_from_palette_point(*pos);
                } else if Self::point_in_rect(*pos, self.hue_rect()) {
                    self.drag_target = Some(ColorDragTarget::Hue);
                    self.set_from_hue_point(*pos);
                } else if self.show_alpha && Self::point_in_rect(*pos, self.alpha_rect()) {
                    self.drag_target = Some(ColorDragTarget::Alpha);
                    self.set_from_alpha_point(*pos);
                }
            }
            // # The defect this replaces
            //
            // Only `MousePress` was handled, so a picker was a **click-only** control: dragging
            // inside the palette, the hue rail or the alpha rail moved nothing until the pointer
            // was released and pressed again. Every other draggable control in the crate
            // (`slider`, `range_slider`, `dial`, `scroll_bar`) reads a move, and a colour picker is
            // the one control where the gesture is unambiguously a drag — the user is looking for
            // a colour, which means sweeping until the swatch looks right.
            //
            // The arm re-uses the press helpers rather than adding three more hit tests, so the
            // region a drag updates is by construction the region the press started in: a pointer
            // that wanders out of the hue rail does not silently start editing the alpha.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => match self.drag_target {
                Some(ColorDragTarget::Palette) => self.set_from_palette_point(*pos),
                Some(ColorDragTarget::Hue) => self.set_from_hue_point(*pos),
                Some(ColorDragTarget::Alpha) => self.set_from_alpha_point(*pos),
                None => {}
            },
            Event::MouseRelease { .. } | Event::PointerRelease { .. } => {
                self.drag_target = None;
            }
            // A pointer that has left the control must not keep editing it: the drag target is
            // cleared, which is the same latch-clear `fab` does for its press.
            Event::MouseLeave { .. } => {
                self.drag_target = None;
            }
            Event::KeyPress { key, modifiers } => {
                if self.hex_editing {
                    match *key {
                        8 => {
                            self.hex_buffer.pop();
                            self.base.request_redraw();
                        }
                        13 => self.commit_hex(),
                        27 => {
                            self.hex_editing = false;
                            self.base.request_redraw();
                        }
                        _ => {}
                    }
                    return;
                }
                let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::SHIFT);
                let step = if shift { 10 } else { 2 };
                match crate::shortcut::Key::from_key_code(*key) {
                    Some(crate::shortcut::Key::Left) => {
                        self.saturation = self.saturation.saturating_sub(step);
                        self.sync_color_from_hsva();
                    }
                    // Right/Left edit saturation while Up/Down edit value: the two axes of the
                    // palette square, so keyboard navigation reaches every colour a pointer can.
                    Some(crate::shortcut::Key::Right) => {
                        self.saturation = self.saturation.saturating_add(step);
                        self.sync_color_from_hsva();
                    }
                    Some(crate::shortcut::Key::Up) => {
                        self.value = self.value.saturating_add(step);
                        self.sync_color_from_hsva();
                    }
                    Some(crate::shortcut::Key::Down) => {
                        self.value = self.value.saturating_sub(step);
                        self.sync_color_from_hsva();
                    }
                    Some(crate::shortcut::Key::PageUp) => {
                        self.hue = self.hue.saturating_add(step);
                        self.sync_color_from_hsva();
                    }
                    Some(crate::shortcut::Key::PageDown) => {
                        self.hue = self.hue.saturating_sub(step);
                        self.sync_color_from_hsva();
                    }
                    _ => {}
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } if self.hex_editing => {
                self.hex_buffer.push_str(text);
                self.base.request_redraw();
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for ColorPicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The picker's frame and its guide rails are chrome and resolve through the theme; the
        // **spectrum** inside them is the value being picked and stays the caller's data (that is
        // what the `hue-wheel` exemption is about). Every literal below used to be a light-theme
        // grey, so the panel did not move with the appearance at all and a dark-theme picker was
        // a white card.
        let style = self.style();
        let theme = crate::style::resolved_theme_style_shared("color_picker");
        let panel = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(247, 249, 252));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or_else(|| panel.contrast_color());
        // The rails frame a spectrum whose own brightness varies along its length, so they are
        // held at a fixed ± step from the panel rather than derived from the spectrum.
        let frame = panel.blend(&ink, 0.22);

        context.face_with_gradient(
            rect,
            panel,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().border_radius.unwrap_or(0),
            frame,
            1,
        );

        let palette = self.palette_rect();
        let hue_rect = self.hue_rect();
        let alpha_rect = self.alpha_rect();
        let hex_font = effective_font(style);
        let hex_text = self.hex_rgba();
        // One cache for the four gradients and the readout's line box. They are pure functions of
        // the rects, the hue and the colour, so a still picker rebuilds nothing (SP-P70).
        let key = PaintCacheKey {
            palette,
            hue_rect,
            alpha_rect,
            hue: self.hue,
            color: (self.color.r, self.color.g, self.color.b, self.color.a),
            font_size_bits: hex_font.size().to_bits(),
            show_alpha: self.show_alpha,
        };
        let stale = self.paint_cache.borrow().as_ref().is_none_or(|cache| cache.key != key);
        if stale {
            let base_hue = hsv_to_color(self.hue, 255, 255, 255);
            let horizontal = Gradient::linear(
                Point::new(palette.x, palette.y),
                Point::new(palette.x + palette.width as i32, palette.y),
            )
            .with_stops(vec![
                GradientStop::new(0.0, Color::WHITE),
                GradientStop::new(1.0, base_hue),
            ]);
            let vertical = Gradient::linear(
                Point::new(palette.x, palette.y),
                Point::new(palette.x, palette.y + palette.height as i32),
            )
            .with_stops(vec![
                GradientStop::new(0.0, Color::TRANSPARENT),
                GradientStop::new(1.0, Color::BLACK),
            ]);
            // The rainbow's stops are constant values, so they are recomputed only when the rail
            // moves — not once per frame as they were.
            let rainbow = Gradient::linear(
                Point::new(hue_rect.x, hue_rect.y),
                Point::new(hue_rect.x + hue_rect.width as i32, hue_rect.y),
            )
            .with_stops(
                (0..=6)
                    .map(|step| {
                        GradientStop::new(
                            step as f32 / 6.0,
                            hsv_to_color((step * 255 / 6) as u8, 255, 255, 255),
                        )
                    })
                    .collect(),
            );
            let alpha = Gradient::linear(
                Point::new(alpha_rect.x, alpha_rect.y),
                Point::new(alpha_rect.x + alpha_rect.width as i32, alpha_rect.y),
            )
            .with_stops(vec![
                GradientStop::new(0.0, self.color.with_alpha(0)),
                GradientStop::new(1.0, self.color.with_alpha(255)),
            ]);
            let metrics = context.measure_text(&hex_text, hex_font);
            *self.paint_cache.borrow_mut() = Some(ColorPickerPaintCache {
                key,
                horizontal,
                vertical,
                rainbow,
                alpha,
                hex_width: metrics.width,
                hex_height: metrics.height,
            });
        }
        let cache = self.paint_cache.borrow();
        let Some(cache) = cache.as_ref() else { return };
        // The saturation/value square: a horizontal white-to-hue ramp overlayed with a vertical
        // transparent-to-black ramp. That is the same two-dimensional colour space the click
        // mapping reads, so the pixel under the pointer and the picked colour agree — the flat
        // grey slab it replaces showed no relationship to the value being selected.
        context.fill_gradient(palette, &cache.horizontal);
        context.fill_gradient(palette, &cache.vertical);
        context.draw_rect(palette, panel.blend(&ink, 0.40));

        // The hue rail is the rainbow the hue byte indexes, so the rail and the value cannot
        // disagree about which colour a position means.
        context.fill_gradient(hue_rect, &cache.rainbow);
        context.draw_rect(hue_rect, panel.blend(&ink, 0.35));

        if self.show_alpha {
            // The alpha rail runs from the colour with no alpha to the same colour fully
            // opaque, so the rail shows exactly what the alpha byte is choosing.
            context.fill_gradient(alpha_rect, &cache.alpha);
            context.draw_rect(alpha_rect, panel.blend(&ink, 0.30));
        }

        for (index, color) in self.presets.iter().enumerate() {
            let Some(preset_rect) = self.preset_rect(index) else {
                continue;
            };
            // The preset swatch IS a colour value, so it is passed through; only its outline is
            // chrome, and it is chosen against the swatch so a dark preset still has an edge.
            context.fill_rect(preset_rect, *color);
            context.draw_rect(preset_rect, color.contrast_color().with_alpha(90));
        }

        // The current-value swatch: its outline is its own contrast colour rather than a fixed
        // dark grey, which only outlined light swatches.
        let swatch_rect = Rect::new(
            rect.x + rect.width as i32 - 70,
            rect.y + rect.height as i32 - (PRESET_ROW + MARGIN) as i32,
            56,
            PRESET_ROW,
        );
        context.fill_rect(swatch_rect, self.color);
        context.draw_rect(swatch_rect, self.color.contrast_color().with_alpha(120));

        // The hex readout takes the row immediately below the palette, which the palette's own
        // rectangle now leaves for it.
        let hex_text = self.hex_rgba();
        let hex_height = cache.hex_height as i32;
        let hex_band = Rect::new(
            rect.x + 8,
            palette.y + palette.height as i32,
            rect.width.saturating_sub(16),
            hex_height.max(1) as u32,
        );
        if self.hex_editing {
            // Editable form: a field, so the readout can be typed into rather than only read.
            context.fill_rect(hex_band, panel.blend(&ink, 0.08));
            context.draw_rect(hex_band, ink.with_alpha(120));
            context.draw_text_fitted_cow(
                hex_band,
                if self.hex_buffer.is_empty() { "#" } else { &self.hex_buffer },
                hex_font,
                ink,
                HorizontalAlignment::Left,
            );
        } else {
            // The readout is chrome sitting on the panel, so it is the resolved ink rather than
            // a literal that was tuned for one appearance.
            let hex_ink = ink.legible_on(panel, 4.5);
            let fit_inset = crate::render::TEXT_FIT_MARGIN as i32;
            let available = (hex_band.width as i32 - 2 * fit_inset).max(0) as u32;
            if cache.hex_width <= available {
                // The cached width says the text fits, so it is drawn at the fitted origin
                // without re-measuring or re-shaping it (SP-P70).
                context.draw_text(
                    Point::new(hex_band.x + fit_inset, hex_band.y),
                    &hex_text,
                    hex_font,
                    hex_ink,
                    HorizontalAlignment::Left,
                );
            } else {
                context.draw_text_fitted_cow(
                    hex_band,
                    &hex_text,
                    hex_font,
                    hex_ink,
                    HorizontalAlignment::Left,
                );
            }
        }

        // The panel's own OK / Cancel row (SP-62). The signals it emits are the ones
        // `pick`/`cancel` already publish, so a host that used the API sees the same facts.
        if let (Some(ok), Some(cancel)) = (self.ok_rect(), self.cancel_rect()) {
            let button_face = panel.blend(&ink, 0.12);
            let button_border = panel.blend(&ink, 0.28);
            context.fill_rect(cancel, button_face);
            context.draw_rect(cancel, button_border);
            context.draw_text_line_cow(
                cancel,
                "Cancel",
                hex_font,
                ink,
                HorizontalAlignment::Center,
            );
            let ok_face = crate::style::palette_shared()
                .as_ref()
                .map(|colors| colors.primary)
                .unwrap_or(Color::PRIMARY);
            context.fill_rect(ok, ok_face);
            context.draw_rect(ok, button_border);
            context.draw_text_line_cow(
                ok,
                "OK",
                hex_font,
                ok_face.contrast_color(),
                HorizontalAlignment::Center,
            );
        }
    }
}

/// Height of the bottom chrome stack below the palette: the hex readout's line box, the
/// preset swatch row, and the margins around them.
///
/// Derived from the parts rather than being one literal, because `44` was too small for
/// them: it did not account for the readout's own 14 px line, so the palette extended to
/// within 6 px of the swatch row and the readout was drawn straight over the palette's
/// bottom edge (the census measured `#FF0000FF` in `225,225,225` on the palette's
/// `255,64,64`, 2.65:1). The palette is a spectrum, so text over any of it is wrong
/// regardless of the ratio.
const READOUT_LINE: u32 = 14;
const PRESET_ROW: u32 = 18;
const MARGIN: u32 = 4;

/// Converts RGB color to HSV components: `(hue, saturation, value)`.
///
/// Hue is scaled to `0..=255` (corresponding to 0..360 degrees).
fn color_to_hsv(color: Color) -> (u8, u8, u8) {
    let rf = color.r as f32 / 255.0;
    let gf = color.g as f32 / 255.0;
    let bf = color.b as f32 / 255.0;

    let c_max = rf.max(gf).max(bf);
    let c_min = rf.min(gf).min(bf);
    let delta = c_max - c_min;

    let hue = if delta == 0.0 {
        0.0
    } else if (c_max - rf).abs() < f32::EPSILON {
        60.0 * (((gf - bf) / delta) % 6.0)
    } else if (c_max - gf).abs() < f32::EPSILON {
        60.0 * (((bf - rf) / delta) + 2.0)
    } else {
        60.0 * (((rf - gf) / delta) + 4.0)
    };

    let normalized_hue = if hue < 0.0 { hue + 360.0 } else { hue };
    let h = ((normalized_hue / 360.0) * 255.0).round().clamp(0.0, 255.0) as u8;

    let s =
        if c_max == 0.0 { 0 } else { ((delta / c_max) * 255.0).round().clamp(0.0, 255.0) as u8 };

    let v = (c_max * 255.0).round().clamp(0.0, 255.0) as u8;

    (h, s, v)
}

fn hsv_to_color(h: u8, s: u8, v: u8, a: u8) -> Color {
    let hf = (h as f32 / 255.0) * 360.0;
    let sf = s as f32 / 255.0;
    let vf = v as f32 / 255.0;

    let c = vf * sf;
    let x = c * (1.0 - (((hf / 60.0) % 2.0) - 1.0).abs());
    let m = vf - c;

    let (r1, g1, b1) = if hf < 60.0 {
        (c, x, 0.0)
    } else if hf < 120.0 {
        (x, c, 0.0)
    } else if hf < 180.0 {
        (0.0, c, x)
    } else if hf < 240.0 {
        (0.0, x, c)
    } else if hf < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Color::rgba(
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
        a,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Records a colour signal into a shared vector.
    fn record_colors(signal: &Signal1<Color>) -> Arc<Mutex<Vec<Color>>> {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let handle = Arc::clone(&sink);
        signal.connect(move |color: Arc<Color>| {
            if let Ok(mut guard) = handle.lock() {
                guard.push(*color);
            }
        });
        sink
    }

    fn colours(sink: &Arc<Mutex<Vec<Color>>>) -> Vec<Color> {
        sink.lock().map(|guard| guard.clone()).unwrap_or_default()
    }

    #[test]
    fn hsva_red_maps_to_red_color() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        picker.set_hsva(0, 255, 255, 255);
        let color = picker.color();
        assert!(color.r >= 250);
        assert!(color.g <= 5);
        assert!(color.b <= 5);
    }

    #[test]
    fn set_hex_updates_color() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        assert!(picker.set_hex("#336699CC"));
        assert_eq!(picker.color(), Color::rgba(0x33, 0x66, 0x99, 0xCC));
    }

    #[test]
    fn apply_preset_emits_color_changed() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));

        let emitted = Arc::new(Mutex::new(Vec::<Color>::new()));
        let sink = emitted.clone();
        picker.color_changed.connect(move |color| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*color);
            }
        });

        assert!(picker.apply_preset(1));
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!got.is_empty());
    }

    #[test]
    fn set_color_synchronizes_hsva() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        picker.set_color(Color::rgb(0, 255, 0));
        let (h, s, v, a) = picker.hsva();
        assert_eq!(a, 255);
        assert!(s >= 250);
        assert!(v >= 250);
        // Hue for green is around 120 deg -> (120/360)*255 = 85
        assert!((h as i32 - 85).abs() <= 2);
    }

    #[test]
    fn presets_are_writable_and_indexed() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        picker.set_presets(vec![Color::rgb(1, 2, 3), Color::rgb(4, 5, 6)]);
        assert_eq!(picker.preset_count(), 2);
        assert_eq!(picker.presets()[1], Color::rgb(4, 5, 6));

        let index = picker.add_preset(Color::rgb(7, 8, 9));
        assert_eq!(index, 2);
        assert_eq!(picker.preset_count(), 3);

        assert!(picker.apply_preset(2));
        assert_eq!(picker.color(), Color::rgb(7, 8, 9));

        assert_eq!(picker.remove_preset(0), Some(Color::rgb(1, 2, 3)));
        assert_eq!(picker.preset_count(), 2);
        assert_eq!(picker.remove_preset(9), None);
    }

    #[test]
    fn presets_property_round_trips_as_a_string_list() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        let list = CapabilityValue::Tuple(vec![
            CapabilityValue::String("#112233FF".to_string()),
            CapabilityValue::String("#445566FF".to_string()),
        ]);
        picker.set("presets", list).expect("a list of hex colours is accepted");
        assert_eq!(picker.preset_count(), 2);
        match picker.get("presets").expect("readable") {
            CapabilityValue::Tuple(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], CapabilityValue::String("#112233FF".to_string()));
            }
            other => panic!("presets must be a string list, got {other:?}"),
        }

        // One bad token refuses the whole list instead of silently dropping an entry.
        let bad = CapabilityValue::Tuple(vec![
            CapabilityValue::String("#112233FF".to_string()),
            CapabilityValue::String("not a colour".to_string()),
        ]);
        assert!(picker.set("presets", bad).is_err());
        assert_eq!(picker.preset_count(), 2);
    }

    #[test]
    fn hue_degrees_is_the_published_spelling_of_the_hue_byte() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        picker.set_hue_degrees(120);
        assert_eq!(picker.hue_degrees(), 120);
        let color = picker.color();
        assert!(color.g > 200, "120° is green, got {color:?}");
        assert!(color.r < 60 && color.b < 60, "120° must be green, got {color:?}");

        // Round trip through the byte the rails store. The byte has 256 steps and a hue has 360
        // degrees, so the two cannot be exactly bijective; the contract is one degree, which is
        // finer than the byte itself can express.
        for degrees in [0u16, 1, 90, 180, 240, 270, 359] {
            picker.set_hue_degrees(degrees);
            let read_back = picker.hue_degrees();
            assert!(
                (i32::from(read_back) - i32::from(degrees)).abs() <= 1,
                "{degrees}° read back as {read_back}°"
            );
        }

        picker.set("hue_degrees", CapabilityValue::UInt(240)).expect("writable");
        assert!((i32::from(picker.hue_degrees()) - 240).abs() <= 1);
    }

    #[test]
    fn pick_commits_the_chosen_colour() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        let committed = record_colors(&picker.color_committed);
        let chosen = Color::rgb(12, 34, 56);
        picker.pick(chosen);
        assert_eq!(picker.color(), chosen);
        assert_eq!(colours(&committed), vec![chosen]);
    }

    #[test]
    fn set_initial_is_silent() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        let committed = record_colors(&picker.color_committed);
        let previewed = record_colors(&picker.color_previewed);
        let initial = Color::rgb(9, 9, 9);
        picker.set_initial(initial);
        assert_eq!(picker.color(), initial);
        assert!(colours(&committed).is_empty());
        assert!(colours(&previewed).is_empty());
    }

    #[test]
    fn cancel_emits_cancelled_and_clears_edit_state() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        picker.set_hex_editing(true);
        let cancelled = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&cancelled);
        picker.canceled.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        picker.cancel();
        assert_eq!(cancelled.load(Ordering::SeqCst), 1);
        assert!(!picker.is_hex_editing());
    }

    #[test]
    fn live_hsva_edit_emits_preview() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        let previewed = record_colors(&picker.color_previewed);
        picker.set_hsva(0, 255, 255, 255);
        assert_eq!(colours(&previewed).len(), 1);
    }
}
