// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Color dialog widget.
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// A named color-dialog feature that can be switched on independently.
///
/// These are the options that are actually honoured by the control's layout and input
/// handling; keeping them as an enum rather than one boolean per field gives a caller one
/// spelling and one setter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDialogOption {
    /// Offer an alpha rail below the picker and preserve alpha while picking.
    ShowAlphaChannel,
    /// Hide the OK/Cancel row. Enter and Escape still commit and cancel.
    NoButtons,
    /// Replace the hex readout with an editable hex field.
    ShowHexInput,
    /// Paint the built-in preset swatches in the preview row.
    ShowPresetPalette,
    /// Paint a hue rail beside the saturation/value plane.
    ShowHueRail,
}

/// The number of strips the two soft ramps are quantised into.
///
/// The renderer exposes a solid-fill primitive but not a per-context gradient fill, so a
/// multi-colour surface is composed from strips. Thirty-two is finer than the eye resolves
/// on a picker of this size and still cheap enough to repaint with the dialog.
const RAMP_STEPS: u32 = 32;

/// Paints one saturation/value plane at `hue`.
fn paint_sv_plane(context: &mut RenderContext, rect: Rect, hue: Color) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let steps = RAMP_STEPS.min(rect.width).max(1);
    for step in 0..steps {
        let t = step as f32 / (steps - 1).max(1) as f32;
        let x = rect.x + (rect.width as f32 * t) as i32;
        let next = rect.x + (rect.width as f32 * (step + 1) as f32 / steps as f32) as i32;
        let strip = Rect::new(x, rect.y, (next - x).max(1) as u32, rect.height);
        context.fill_rect(strip, Color::WHITE.blend(&hue, t));
    }
    let steps = RAMP_STEPS.min(rect.height).max(1);
    for step in 0..steps {
        let t = step as f32 / (steps - 1).max(1) as f32;
        let y = rect.y + (rect.height as f32 * t) as i32;
        let next = rect.y + (rect.height as f32 * (step + 1) as f32 / steps as f32) as i32;
        let strip = Rect::new(rect.x, y, rect.width, (next - y).max(1) as u32);
        context.fill_rect(strip, Color::BLACK.with_alpha((t * 255.0).round() as u8));
    }
}

/// Paints a transparent-to-opaque ramp of `color` for the alpha rail.
fn paint_alpha_rail(context: &mut RenderContext, rect: Rect, color: Color) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    context.fill_rect(rect, Color::WHITE);
    let steps = RAMP_STEPS.min(rect.width).max(1);
    for step in 0..steps {
        let t = step as f32 / (steps - 1).max(1) as f32;
        let x = rect.x + (rect.width as f32 * t) as i32;
        let next = rect.x + (rect.width as f32 * (step + 1) as f32 / steps as f32) as i32;
        let strip = Rect::new(x, rect.y, (next - x).max(1) as u32, rect.height);
        context.fill_rect(strip, color.with_alpha((t * 255.0).round() as u8));
    }
}

/// The saturation and value of a color, each in `0.0..=1.0`.
fn sv_of(color: Color) -> (f32, f32) {
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let saturation = if max <= f32::EPSILON { 0.0 } else { (max - min) / max };
    (saturation, max)
}

/// The built-in palette applied by [`ColorDialog::apply_preset`].
const PRESET_COLORS: [Color; 8] = [
    Color::rgb(0, 0, 0),
    Color::rgb(255, 255, 255),
    Color::rgb(220, 53, 69),
    Color::rgb(255, 140, 0),
    Color::rgb(255, 193, 7),
    Color::rgb(40, 167, 69),
    Color::rgb(0, 123, 255),
    Color::rgb(111, 66, 193),
];

/// The hue of a color in degrees, `0.0..360.0`.
fn hue_of(color: Color) -> f32 {
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta <= f32::EPSILON {
        return 0.0;
    }
    let hue = if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    if hue < 0.0 {
        hue + 360.0
    } else {
        hue
    }
}

/// A color from a hue, saturation and value, each in `0.0..=1.0`.
fn color_from_hsv(hue: f32, saturation: f32, value: f32, alpha: u8) -> Color {
    let h = hue.rem_euclid(360.0) / 60.0;
    let s = saturation.clamp(0.0, 1.0);
    let v = value.clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::rgba(
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
        alpha,
    )
}
/// Color dialog for picking RGBA colors.
///
/// The picker is a saturation/value plane at the current hue: the x axis is saturation and
/// the y axis is value (brightness), with the hue taken from the selected color. That is the
/// surface the dialog paints, so a click's position and the color it produces are the same
/// fact rather than two mappings that happen to disagree.
pub struct ColorDialog {
    base: BaseWidget,
    current_color: Color,
    options_alpha: bool,
    /// Whether the action row is hidden.
    no_buttons: bool,
    /// Whether the hex readout is editable.
    show_hex_input: bool,
    /// Whether the preset swatches are painted.
    show_presets: bool,
    /// Whether the hue rail is painted.
    show_hue_rail: bool,
    /// The hue the picker plane is drawn at, in degrees.
    hue: f32,
    /// The caller's custom colors, drawn after the built-in presets.
    custom_colors: Vec<Color>,
    /// The typed hex text while the hex field is focused.
    hex_input: String,
    /// Whether the hex field has focus.
    hex_focused: bool,
    modal: bool,
    /// Emitted with the new color on every colour change, including ones made
    /// by keyboard nudges and by clicks in the picker area. Fires during
    /// [`ColorDialog::set_current_color`] and therefore also *before* the user
    /// confirms, so a slot reacting to it sees uncommitted selections.
    pub color_selected: Signal1<Color>,
    /// Emitted by [`ColorDialog::accept`], when the user confirms with Enter.
    /// Carries no color — read [`ColorDialog::current_color`] instead.
    pub accepted: GenericSignal,
    /// Emitted by [`ColorDialog::reject`], when the user cancels with Escape.
    /// The selected color is **not** rolled back, so the caller must decide
    /// whether to keep the pre-dialog value.
    pub rejected: GenericSignal,
    /// Reused buffer for the labels this dialog translates while drawing.
    ///
    /// The draw pass used to allocate two `String`s per frame (`tr!` returns an owned
    /// string, and the preview line was a `format!`). Appending into one buffer here
    /// keeps a steady-state frame allocation-free.
    draw_text_buf: String,
}
impl ColorDialog {
    /// Distance from the dialog's top edge to the picker's top edge: the title bar's
    /// 28 px plus the 10 px margin the picker shares with the panel's left inset.
    /// Shared by [`ColorDialog::picker_rect`] and `draw` so hit-testing and painting
    /// cannot disagree about where the picker is.
    const PICKER_TOP_OFFSET: i32 = 38;

    /// Height of the preview swatch band, when the dialog is tall enough to show one.
    const PREVIEW_HEIGHT: i32 = 30;

    /// Height of the button row.
    const BUTTON_HEIGHT: i32 = 28;

    /// Gap between the picker's bottom edge and the rows below it.
    const PICKER_GAP: i32 = 10;

    /// Bottom margin below the button row.
    const BOTTOM_MARGIN: i32 = 12;

    /// Height of the alpha rail, in logical pixels.
    const ALPHA_RAIL_HEIGHT: u32 = 10;
    /// The gap between the picker and the alpha rail.
    const ALPHA_RAIL_GAP: u32 = 4;
    /// The width of the hue rail.
    const HUE_RAIL_WIDTH: u32 = 14;
    /// The gap between the picker plane and the hue rail.
    const HUE_RAIL_GAP: u32 = 6;

    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered**. Painting it verbatim drew the 240x120
    /// census cell as a 240x120 panel whose picker, preview and buttons were then stacked
    /// against its top and bottom edges with literals written for the 400x300 default size.
    /// [`ControlMetrics::painted_box`] caps each axis at the dialog's own intrinsic size and
    /// centres what is left, and it clamps up to one pixel so a squeezed dialog stays
    /// visible. Every band below — the title strip, the picker, the preview row and the
    /// button row — is derived from this one rect, so they cannot be placed from different
    /// boxes.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }

    /// The y of the button row's top edge — the line the picker's height stops at.
    ///
    /// Derived by stacking the rows below the picker upward from the dialog's bottom
    /// edge, so the reserve is a *sum of the heights actually drawn* rather than a single
    /// literal standing in for them. That is what keeps it in the same units as the
    /// picker's own top offset: the old `rect.height.saturating_sub(120)` measured a bottom
    /// reserve from a top offset, and at the 120 px box the renderer uses, the literal 120
    /// was both the dialog's designed height *and* its current one, so the subtraction was
    /// exactly 0 and the picker collapsed. Tying both edges to the elements that fix them
    /// removes the coincidence of one size.
    fn button_row_top(&self) -> i32 {
        let rect = self.frame_rect();
        rect.y + rect.height as i32 - Self::BOTTOM_MARGIN - Self::BUTTON_HEIGHT
    }

    /// The y of the preview band's top edge, or `None` when the dialog is too short to
    /// hold one between the title bar and the button row.
    ///
    /// Returning an `Option` rather than a clamped coordinate is what lets the picker
    /// take the whole space on a short dialog: a band that does not fit is dropped
    /// instead of being squeezed into a negative height or overlapping the picker.
    fn preview_row_top(&self) -> Option<i32> {
        let rect = self.frame_rect();
        let band_top = self.button_row_top() - Self::PICKER_GAP - Self::PREVIEW_HEIGHT;
        // The band must clear the title bar's bottom and the picker's own minimum.
        if band_top >= rect.y + Self::PICKER_TOP_OFFSET + Self::PICKER_GAP {
            Some(band_top)
        } else {
            None
        }
    }

    /// Creates a dialog with the color `rgb(255, 255, 255)`, alpha options off,
    /// and modality on.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 400x300, which the picker and button layout assume.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorDialog, geometry, "ColorDialog"),
            current_color: Color::rgb(255, 255, 255),
            options_alpha: false,
            no_buttons: false,
            show_hex_input: false,
            show_presets: false,
            show_hue_rail: false,
            hue: 0.0,
            custom_colors: Vec::new(),
            hex_input: String::new(),
            hex_focused: false,
            modal: true,
            color_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            draw_text_buf: String::new(),
        }
    }
    /// Returns whether the dialog is modal. Defaults to `true`.
    ///
    /// This records the intent; the enforcement is [`crate::widget::runtime::enter_modal`]
    /// / [`crate::widget::runtime::exit_modal`], which push/pop the active modal so
    /// input outside the dialog's subtree is blocked. A caller that shows a modal
    /// dialog through a handle (e.g. `MessageBoxHandle::show_modal`) gets both the
    /// show and the stack push together.
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    /// Sets the modality intent. See [`ColorDialog::is_modal`].
    pub fn set_modal(&mut self, modal: bool) {
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        // `draw` paints the page-dimming layer from this, so a write that did not repaint left the
        // modal/non-modal distinction invisible until something else forced a frame.
        self.base.request_redraw();
    }
    /// Returns the currently selected color.
    pub fn current_color(&self) -> Color {
        self.current_color
    }
    /// Returns whether alpha editing is offered. Defaults to `false`.
    ///
    /// When off, picking a color forces alpha to `255`; when on, the existing
    /// alpha is preserved across picks.
    pub fn options_alpha(&self) -> bool {
        self.options_alpha
    }
    /// Sets the current color and emits `color_selected`.
    ///
    /// A write of the value the dialog already holds is a no-op: it neither signals nor
    /// redraws. Signalling on a repeated value made a slot that called back into this setter
    /// recurse without ever making progress; the signal is named `color_selected`, so reporting
    /// a selection the caller already made would be false.
    pub fn set_current_color(&mut self, color: Color) {
        if self.current_color == color {
            return;
        }
        // A chromatic write moves the plane's hue, so the next pick starts from the hue the
        // caller just selected. A grey write carries no hue and leaves it alone.
        let (saturation, _) = sv_of(color);
        if saturation > 0.0 {
            self.hue = hue_of(color);
        }
        self.current_color = color;
        self.color_selected.emit(color);
        self.base.request_redraw();
    }
    /// Enables or disables alpha handling for picker clicks. See
    /// [`ColorDialog::options_alpha`]. Repaints because the alpha rail appears or disappears.
    pub fn set_options_alpha(&mut self, enabled: bool) {
        if self.options_alpha == enabled {
            return;
        }
        self.options_alpha = enabled;
        self.base.request_redraw();
    }

    /// Whether `option` is enabled.
    pub fn test_option(&self, option: ColorDialogOption) -> bool {
        match option {
            ColorDialogOption::ShowAlphaChannel => self.options_alpha,
            ColorDialogOption::NoButtons => self.no_buttons,
            ColorDialogOption::ShowHexInput => self.show_hex_input,
            ColorDialogOption::ShowPresetPalette => self.show_presets,
            ColorDialogOption::ShowHueRail => self.show_hue_rail,
        }
    }

    /// Enables or disables `option` and repaints when it changes.
    pub fn set_option(&mut self, option: ColorDialogOption, enabled: bool) {
        match option {
            ColorDialogOption::ShowAlphaChannel => self.set_options_alpha(enabled),
            ColorDialogOption::NoButtons => {
                if self.no_buttons == enabled {
                    return;
                }
                self.no_buttons = enabled;
            }
            ColorDialogOption::ShowHexInput => {
                if self.show_hex_input == enabled {
                    return;
                }
                self.show_hex_input = enabled;
                if !enabled {
                    self.hex_focused = false;
                }
            }
            ColorDialogOption::ShowPresetPalette => {
                if self.show_presets == enabled {
                    return;
                }
                self.show_presets = enabled;
            }
            ColorDialogOption::ShowHueRail => {
                if self.show_hue_rail == enabled {
                    return;
                }
                self.show_hue_rail = enabled;
            }
        }
        self.base.request_redraw();
    }

    /// The built-in palette, in display order.
    pub fn presets(&self) -> &[Color] {
        &PRESET_COLORS
    }

    /// Applies the preset at `index`, reporting whether the index addressed one.
    pub fn apply_preset(&mut self, index: usize) -> bool {
        let Some(color) = PRESET_COLORS.get(index).copied() else {
            return false;
        };
        let alpha = if self.options_alpha { self.current_color.a } else { 255 };
        self.set_current_color(Color::rgba(color.r, color.g, color.b, alpha));
        true
    }

    /// Parses `hex` and selects it, reporting whether the text was a valid color.
    pub fn set_hex(&mut self, hex: &str) -> bool {
        let Some(color) = Color::parse_hex(hex) else {
            return false;
        };
        let alpha = if self.options_alpha { color.a } else { 255 };
        self.set_current_color(Color::rgba(color.r, color.g, color.b, alpha));
        true
    }

    /// The caller's custom colors, in the order they were added.
    pub fn custom_colors(&self) -> &[Color] {
        &self.custom_colors
    }

    /// Replaces the custom palette and repaints.
    pub fn set_custom_colors(&mut self, colors: Vec<Color>) {
        if self.custom_colors == colors {
            return;
        }
        self.custom_colors = colors;
        self.base.request_redraw();
    }

    /// Adds a custom color and repaints.
    pub fn add_custom_color(&mut self, color: Color) {
        if self.custom_colors.contains(&color) {
            return;
        }
        self.custom_colors.push(color);
        self.base.request_redraw();
    }
    /// Emits `accepted` and hides the dialog.
    ///
    /// The signal is emitted before hiding, and nothing else happens: no
    /// validation, and no signal distinguishing this acceptance from a previous
    /// one.
    pub fn accept(&mut self) {
        self.accepted.emit();
        self.hide();
    }
    /// Emits `rejected` and hides the dialog.
    ///
    /// The color chosen so far is left untouched, so a caller that wants cancel
    /// to restore the original color must snapshot it beforehand.
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
    /// Returns the selected color. Identical to
    /// [`ColorDialog::current_color`]; kept for callers using the
    /// `get_color`/`set_current_color` pairing.
    pub fn get_color(&self) -> Color {
        self.current_color
    }

    /// The band the colour picker may occupy, between the title bar's bottom edge
    /// and the button row's top edge.
    ///
    /// The height is *stacked downward from what is already drawn* rather than
    /// subtracted from the dialog's own height. The old
    /// `rect.height.saturating_sub(120)` mixed two different units: it treated the
    /// reserved lower stack as a distance below the picker's top, but the picker
    /// does not start at the dialog's top — the title bar's 38 px sit above it. At
    /// the dialog's designed 300 px the subtraction happened to leave room; at the
    /// 120 px box the renderer actually uses the same literal 120 was both the
    /// designed height and the current one, so the difference was exactly 0 and the
    /// picker collapsed to a zero-height rectangle (`color_dialog.svg` carried two
    /// `<rect ... height="0">`). Deriving the extent from the two edges the rest
    /// of the dialog fixes is what makes the picker's height a consequence of the
    /// layout instead of a coincidence of one size.
    fn picker_rect(&self) -> Rect {
        let rect = self.frame_rect();
        let picker_top = rect.y + Self::PICKER_TOP_OFFSET;
        // The picker stops at the top of whichever row is drawn below it: the preview
        // band when the dialog can hold one, otherwise the button row itself.
        let below_top = self.preview_row_top().unwrap_or_else(|| self.button_row_top());
        let rail = if self.options_alpha {
            (Self::ALPHA_RAIL_HEIGHT + Self::ALPHA_RAIL_GAP) as i32
        } else {
            0
        };
        let height = (below_top - picker_top - Self::PICKER_GAP - rail).max(0) as u32;
        let mut width = rect.width.saturating_sub(20);
        if self.show_hue_rail {
            width = width.saturating_sub(Self::HUE_RAIL_WIDTH + Self::HUE_RAIL_GAP);
        }
        Rect::new(rect.x + 10, picker_top, width, height)
    }

    /// The hue rail beside the picker, present only when the option is on.
    fn hue_rect(&self) -> Option<Rect> {
        if !self.show_hue_rail {
            return None;
        }
        let picker = self.picker_rect();
        (picker.width > 0 && picker.height > 0).then(|| {
            Rect::new(
                picker.x + picker.width as i32 + Self::HUE_RAIL_GAP as i32,
                picker.y,
                Self::HUE_RAIL_WIDTH,
                picker.height,
            )
        })
    }

    /// Maps a point on the hue rail to a hue in degrees.
    fn hue_from_point(&self, pos: Point) -> Option<f32> {
        let rail = self.hue_rect()?;
        if !Self::point_in_rect(pos, rail) {
            return None;
        }
        let fraction = ((pos.y - rail.y) as f32 / rail.height.max(1) as f32).clamp(0.0, 1.0);
        Some(fraction * 360.0)
    }

    /// The alpha rail below the picker, present only when alpha editing is on.
    fn alpha_rect(&self) -> Option<Rect> {
        if !self.options_alpha {
            return None;
        }
        let picker = self.picker_rect();
        Some(Rect::new(
            picker.x,
            picker.y + picker.height as i32 + Self::ALPHA_RAIL_GAP as i32,
            picker.width,
            Self::ALPHA_RAIL_HEIGHT,
        ))
    }

    /// The editable hex field's rect, when the option is on and a preview row exists.
    fn hex_field_rect(&self) -> Option<Rect> {
        if !self.show_hex_input {
            return None;
        }
        let preview_y = self.preview_row_top()?;
        let rect = self.frame_rect();
        let presets = self.preset_rects().len() as u32;
        let presets_width = if presets == 0 { 0 } else { presets * 18 + 4 };
        let x = rect.x + 80 + presets_width as i32;
        let width = (rect.x + rect.width as i32 - 10 - x).max(0) as u32;
        (width > 0).then(|| Rect::new(x, preview_y + 2, width, Self::PREVIEW_HEIGHT as u32 - 4))
    }

    /// The preset swatch rects in the preview row, in palette order.
    fn preset_rects(&self) -> Vec<Rect> {
        if !self.show_presets {
            return Vec::new();
        }
        let Some(preview_y) = self.preview_row_top() else {
            return Vec::new();
        };
        let rect = self.frame_rect();
        let count = (PRESET_COLORS.len() + self.custom_colors.len()).min(8) as u32;
        let available = (rect.width.saturating_sub(96)) / 18;
        let count = count.min(available);
        (0..count)
            .map(|index| Rect::new(rect.x + 80 + index as i32 * 18, preview_y + 6, 14, 14))
            .collect()
    }

    /// The preset color at a swatch index, including custom colors after the built-ins.
    fn palette_color(&self, index: usize) -> Option<Color> {
        PRESET_COLORS
            .get(index)
            .copied()
            .or_else(|| self.custom_colors.get(index.saturating_sub(PRESET_COLORS.len())).copied())
    }

    /// The action labels this dialog paints, in row order.
    ///
    /// `draw` and [`ColorDialog::button_at`] both call this, so a press can never resolve
    /// against a different label set than the one that was measured and painted.
    fn action_labels() -> Vec<String> {
        vec![
            super::localized_label("common.button.ok", "OK"),
            super::localized_label("common.button.cancel", "Cancel"),
        ]
    }

    /// The action button rects, rebuilt from the same bands `draw` uses.
    ///
    /// The press path has no render context of its own, so it builds a measuring one — the
    /// pattern `MessageBox::action_button_at` established. The rects themselves are never
    /// cached: a cache is a second copy of the geometry that a resize can leave stale, which is
    /// exactly how a hit region drifts away from the ink it is meant to describe.
    fn action_buttons(&self) -> Vec<Rect> {
        let size = Size::new(self.base.geometry().width.max(1), self.base.geometry().height.max(1));
        let mut backend = crate::render::SoftwarePaintBackend::new(size, 1.0);
        let context = RenderContext::new(&mut backend);
        let rect = self.frame_rect();
        let band = super::message_box::inset_action_band(
            Rect::new(rect.x, self.button_row_top(), rect.width, Self::BUTTON_HEIGHT as u32),
            rect.y,
        );
        let labels = Self::action_labels();
        super::message_box::action_row_geometry(
            &context,
            effective_font(self.base.style()),
            &labels,
            band,
            true,
        )
        .buttons
    }

    /// The index of the action button under `pos`, or `None`.
    fn button_at(&self, pos: Point) -> Option<usize> {
        self.action_buttons().iter().position(|button| button.contains_point(pos))
    }

    fn point_in_rect(pos: Point, rect: Rect) -> bool {
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn pick_color_from_point(&self, pos: Point) -> Option<Color> {
        let picker = self.picker_rect();
        if !Self::point_in_rect(pos, picker) {
            return None;
        }
        let w = picker.width.max(1) as f32;
        let h = picker.height.max(1) as f32;
        let saturation = ((pos.x - picker.x) as f32 / w).clamp(0.0, 1.0);
        let value = (1.0 - (pos.y - picker.y) as f32 / h).clamp(0.0, 1.0);
        let a = if self.options_alpha { self.current_color.a } else { 255 };
        Some(color_from_hsv(self.hue, saturation, value, a))
    }

    /// Maps a point on the alpha rail to an alpha value.
    fn alpha_from_point(&self, pos: Point) -> Option<u8> {
        let rail = self.alpha_rect()?;
        if !Self::point_in_rect(pos, rail) {
            return None;
        }
        let fraction = ((pos.x - rail.x) as f32 / rail.width.max(1) as f32).clamp(0.0, 1.0);
        Some((fraction * 255.0).round() as u8)
    }

    fn nudge_rgb(&mut self, dr: i16, dg: i16, db: i16) {
        let next = Color::rgba(
            (self.current_color.r as i16 + dr).clamp(0, 255) as u8,
            (self.current_color.g as i16 + dg).clamp(0, 255) as u8,
            (self.current_color.b as i16 + db).clamp(0, 255) as u8,
            self.current_color.a,
        );
        self.set_current_color(next);
    }
}
impl Widget for ColorDialog {
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
        crate::core::Size::new(400, 300)
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
            "color_selected" => {
                Some(EventSignalRef::mapped("color_selected", &self.color_selected, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "accepted" => Some(EventSignalRef::unit("accepted", &self.accepted)),
            "rejected" => Some(EventSignalRef::unit("rejected", &self.rejected)),
            _ => None,
        }
    }
}

/// `ColorDialog`'s property contract.
///
/// # Why the dialog needed its own
///
/// Until BLUE16 phase E-6 the picker *was* the dialog's implementation, so one
/// contract served both: `color_picker_capability` carried `WidgetKind::ColorDialog`
/// and the dialog itself declared no contract at all. Splitting the kinds made that
/// gap visible — a test caught `color_dialog` as "constructible but exposing no
/// contract" — so the dialog now publishes the properties that describe *it*: the
/// colour it is editing, plus the two flags that are the dialog's own business
/// (modality and the alpha option). The picker keeps the rest.
impl WidgetProperties for ColorDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_color" => Ok(CapabilityValue::String(self.current_color.to_hex_rgba())),
            "modal" => Ok(CapabilityValue::Bool(self.modal)),
            "options_alpha" => Ok(CapabilityValue::Bool(self.options_alpha)),
            "hex_rgba" => Ok(CapabilityValue::String(self.current_color.to_hex_rgba())),
            "preset_index" => Ok(
                match self.presets().iter().position(|preset| {
                    Color::rgba(preset.r, preset.g, preset.b, self.current_color.a)
                        == self.current_color
                        || *preset == self.current_color
                }) {
                    Some(index) => CapabilityValue::UInt(index as u64),
                    None => CapabilityValue::Null,
                },
            ),
            "show_hex_input" => Ok(CapabilityValue::Bool(self.show_hex_input)),
            "show_presets" => Ok(CapabilityValue::Bool(self.show_presets)),
            "show_hue_rail" => Ok(CapabilityValue::Bool(self.show_hue_rail)),
            "custom_color_count" => Ok(CapabilityValue::UInt(self.custom_colors.len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            "options_alpha" => {
                self.set_options_alpha(expect_bool(value)?);
                Ok(())
            }
            "hex_rgba" => {
                if self.set_hex(&expect_string(value)?) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::TypeMismatch)
                }
            }
            "preset_index" => {
                let index = match value {
                    CapabilityValue::UInt(v) => v as usize,
                    CapabilityValue::Int(v) if v >= 0 => v as usize,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                if self.apply_preset(index) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "show_hex_input" => {
                self.set_option(ColorDialogOption::ShowHexInput, expect_bool(value)?);
                Ok(())
            }
            "show_presets" => {
                self.set_option(ColorDialogOption::ShowPresetPalette, expect_bool(value)?);
                Ok(())
            }
            "show_hue_rail" => {
                self.set_option(ColorDialogOption::ShowHueRail, expect_bool(value)?);
                Ok(())
            }
            "custom_color_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // The edited colour is not writable here: it changes through the picker
            // area and through `set_current_color`, both of which emit
            // `color_selected`. Making it writable through a generic property write
            // would be a second path that skips that signal.
            "current_color" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "current_color",
            "modal",
            "options_alpha",
            "hex_rgba",
            "preset_index",
            "show_hex_input",
            "show_presets",
            "show_hue_rail",
            "custom_color_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `color_dialog` publishes.
    ///
    /// `set_hex` takes the hex text and `apply_preset` takes the preset index, so neither can
    /// complete without a payload: they are refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the argument is
    /// what is missing. The payload-carrying half of the same features is the writable
    /// `hex_rgba` and `preset_index` properties, which is where the value goes.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_hex" | "apply_preset" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for ColorDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                // The buttons share the bottom band with nothing else, but the check comes
                // first anyway: a press on a button must never fall through to the picker.
                if !self.no_buttons {
                    if let Some(index) = self.button_at(*pos) {
                        if index == 0 {
                            self.accept();
                        } else {
                            self.reject();
                        }
                        return;
                    }
                }
                if self.show_presets {
                    if let Some(index) =
                        self.preset_rects().iter().position(|rect| Self::point_in_rect(*pos, *rect))
                    {
                        if let Some(color) = self.palette_color(index) {
                            let alpha = if self.options_alpha { self.current_color.a } else { 255 };
                            self.set_current_color(Color::rgba(color.r, color.g, color.b, alpha));
                        }
                        return;
                    }
                }
                if self.hex_field_rect().is_some_and(|rect| Self::point_in_rect(*pos, rect)) {
                    self.hex_focused = true;
                    // The field starts empty so the first typed character is the first
                    // character of the new value rather than an append to the old one.
                    self.hex_input.clear();
                    self.base.request_redraw();
                    return;
                }
                if let Some(alpha) = self.alpha_from_point(*pos) {
                    let color = self.current_color;
                    self.set_current_color(Color::rgba(color.r, color.g, color.b, alpha));
                    return;
                }
                if let Some(hue) = self.hue_from_point(*pos) {
                    let (saturation, value) = sv_of(self.current_color);
                    let alpha = if self.options_alpha { self.current_color.a } else { 255 };
                    self.hue = hue;
                    self.set_current_color(color_from_hsv(hue, saturation, value, alpha));
                    // A grey selection keeps the same RGB while the plane's hue changes, so the
                    // paint still has to be refreshed even when the colour write was a no-op.
                    self.base.request_redraw();
                    return;
                }
                if let Some(color) = self.pick_color_from_point(*pos) {
                    self.set_current_color(color);
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if self.hex_focused {
                    let mut candidate = self.hex_input.clone();
                    candidate.extend(text.chars().filter(|character| !character.is_control()));
                    self.hex_input = candidate.clone();
                    let _ = self.set_hex(&candidate);
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } => {
                if self.hex_focused {
                    if *key == key_codes::BACKSPACE {
                        self.hex_input.pop();
                        let candidate = self.hex_input.clone();
                        let _ = self.set_hex(&candidate);
                        self.base.request_redraw();
                    } else if *key == key_codes::ENTER || *key == key_codes::ESCAPE {
                        self.hex_focused = false;
                        self.base.request_redraw();
                    }
                    return;
                }
                if *key == key_codes::ENTER {
                    self.accept();
                } else if *key == key_codes::ESCAPE {
                    self.reject();
                } else if *key == key_codes::LEFT {
                    self.nudge_rgb(-5, 0, 0);
                } else if *key == key_codes::RIGHT {
                    self.nudge_rgb(5, 0, 0);
                } else if *key == key_codes::UP {
                    self.nudge_rgb(0, 5, 0);
                } else if *key == key_codes::DOWN {
                    self.nudge_rgb(0, -5, 0);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for ColorDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // A modal dialog dims the page behind it: that dimming is the whole visual difference
        // between "this dialog is up and the page is inert" and "a panel was drawn on the page".
        // The `modal` flag was recorded and enforced by the modal stack, and nothing painted it.
        if self.modal {
            super::draw_modal_scrim(context, self.geometry());
        }
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal,
        // so a light/dark switch left the panel, its title bar and its buttons unchanged —
        // the rendering census reported the control as theme-blind.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        // The **effective font** — the resolved theme/caller font — for the title, the hex readout
        // and the buttons (D09-STYLE-01).
        let font = effective_font(style);
        let theme = crate::style::resolved_theme_style_shared("color_dialog");
        // `color_dialog` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // panel painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from
        // it, the same distinction `Colors::input_background` draws for a field.
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
        // The title bar and the buttons are distinct bands on the panel, derived from it so
        // the three stay one visible step apart in either appearance.
        let title_bar = surface.blend(&ink, 0.08);
        // The accept button takes the theme's **accent/primary token**, and the dismiss
        // button takes the panel surface plus the border. Both used to be filled with a
        // single colour — `theme.background_color`, which for this `Surface`-role control is
        // the window fill — so the two buttons were byte-identical and only Cancel's stroke
        // told them apart. The pair is the conventional one: a filled affirmative and an
        // outlined negative. Same shape as `input_dialog.rs`.
        let accent = {
            let manager = crate::style::theme_manager();
            manager
                .current_theme()
                .map(|active| active.colors.primary)
                .unwrap_or_else(|| surface.blend(&ink, 0.12))
        };
        let accent_ink = accent.contrast_color();

        // Rounded by [`dimensions::DIALOG_RADIUS`]; the radius is clamped to the frame so a
        // box smaller than its own corner is not drawn with an inverted one.
        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.face_with_gradient(
                rect,
                surface,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
            context.draw_rect(rect, border);
        }
        // Every label below is fitted to the band it sits in. None of them was bounded, so
        // the two ends of the dialog carried absolute origins written for its 400 px size
        // hint — at the census rectangle the preview label ran to x=379 and the Cancel label
        // to x=360, both past the 240 px frame. The fit makes the label's own rectangle the
        // authority instead of the size the control was designed at.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, title_bar);
        let title_font = font;
        let title_line = context.text_line(title_bar_band, title_font);
        self.draw_text_buf.clear();
        crate::tr_into!("color_dialog.title", &mut self.draw_text_buf);
        context.draw_text_fitted_cow(
            Rect::new(
                rect.x + 8,
                title_line.y,
                rect.width.saturating_sub(16),
                title_line.height.max(1),
            ),
            &self.draw_text_buf,
            title_font,
            ink,
            HorizontalAlignment::Left,
        );
        // Color picker area (simplified)
        //
        // Both fills are left as literals: the swatch is the colour being edited and the
        // field behind it is the neutral backdrop that makes it readable. Neither is theme
        // chrome, and recolouring either from `style` would misrepresent the picked colour.
        // Drawn only when the band has real extent: a zero-height picker is an element the
        // SVG backend emits while the rasteriser skips it.
        let picker_rect = self.picker_rect();
        if picker_rect.width > 0 && picker_rect.height > 0 {
            // The plane is the one the click maps onto: a saturation/value field at the
            // current hue. The old flat grey fill was a swatch of nothing — the click mapped
            // to an RGB pair the user could not see, so the picker's appearance and its
            // meaning disagreed.
            let hue = color_from_hsv(self.hue, 1.0, 1.0, 255);
            paint_sv_plane(context, picker_rect, hue);
            context.draw_rect(picker_rect, border);
        }
        if let Some(rail) = self.hue_rect() {
            if rail.width > 0 && rail.height > 0 {
                let steps = RAMP_STEPS.min(rail.height).max(1);
                for step in 0..steps {
                    let t = step as f32 / (steps - 1).max(1) as f32;
                    let y = rail.y + (rail.height as f32 * t) as i32;
                    let next =
                        rail.y + (rail.height as f32 * (step + 1) as f32 / steps as f32) as i32;
                    let strip = Rect::new(rail.x, y, rail.width, (next - y).max(1) as u32);
                    context.fill_rect(strip, color_from_hsv(t * 360.0, 1.0, 1.0, 255));
                }
                context.draw_rect(rail, border);
                let marker_y = rail.y + (rail.height as f32 * (self.hue / 360.0)) as i32;
                context.draw_line(
                    Point::new(rail.x, marker_y),
                    Point::new(rail.x + rail.width as i32, marker_y),
                    ink,
                );
            }
        }
        // The alpha rail is the same value the picker preserves: a transparent-to-opaque
        // ramp of the selected hue, so choosing alpha is a click rather than an unexposed
        // channel.
        if let Some(rail) = self.alpha_rect() {
            if rail.width > 0 && rail.height > 0 {
                let solid = Color::rgba(
                    self.current_color.r,
                    self.current_color.g,
                    self.current_color.b,
                    255,
                );
                paint_alpha_rail(context, rail, solid);
                context.draw_rect(rail, border);
                let marker_x =
                    rail.x + (rail.width as f32 * (self.current_color.a as f32 / 255.0)) as i32;
                context.draw_line(
                    Point::new(marker_x, rail.y),
                    Point::new(marker_x, rail.y + rail.height as i32),
                    ink,
                );
            }
        }
        // Color preview. Drawn only when a band's worth of room is left between the
        // title bar and the button row; on a shorter dialog the picker takes that space
        // instead, which is the trade the fit test in `preview_row_top` makes.
        let preview_font = font;
        // One buffer for both the translated prefix and the hex readout; `write_hex_rgba`
        // appends the colour without allocating its own `String`.
        self.draw_text_buf.clear();
        crate::tr_into!("color_dialog.current_color", &mut self.draw_text_buf);
        self.draw_text_buf.push(' ');
        self.current_color.write_hex_rgba(&mut self.draw_text_buf);
        let preview_text = &self.draw_text_buf;
        if let Some(preview_y) = self.preview_row_top() {
            let preview_rect = Rect::new(rect.x + 10, preview_y, 60, Self::PREVIEW_HEIGHT as u32);
            context.fill_rect(preview_rect, self.current_color);
            context.draw_rect(preview_rect, border);
            // Preset swatches are an opt-in palette: each one is a flat fill of the colour it
            // applies, and a press on it is resolved by the same rects this paints.
            let swatches = self.preset_rects();
            for (index, swatch) in swatches.iter().enumerate() {
                if let Some(color) = self.palette_color(index) {
                    context.fill_rect(*swatch, color);
                    context.draw_rect(*swatch, border);
                }
            }
            if let Some(field) = self.hex_field_rect() {
                context.fill_rect(field, surface);
                context.draw_rect(field, if self.hex_focused { accent } else { border });
                let text = if self.hex_focused {
                    &self.hex_input
                } else {
                    &self.current_color.to_hex_rgba()
                };
                let line = context.text_line(field, preview_font);
                context.draw_text_fitted_cow(
                    Rect::new(
                        field.x + 4,
                        line.y,
                        field.width.saturating_sub(8),
                        line.height.max(1),
                    ),
                    text,
                    preview_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            } else {
                // The hex readout sits in the strip between the swatch and the panel's right
                // margin, so a long hex string truncates there rather than running under the
                // buttons. It is centred on the 30 px swatch band through the shared primitive:
                // the glyph origin is the box's top-left, so the old `preview_y + 15` put the
                // line's top edge on the swatch's middle line.
                let text_x = 80 + swatches.len() as i32 * 18;
                let band = Rect::new(
                    rect.x + text_x,
                    preview_y,
                    (rect.x + rect.width as i32 - 10 - (rect.x + text_x)).max(0) as u32,
                    Self::PREVIEW_HEIGHT as u32,
                );
                let line = context.text_line(band, preview_font);
                context.draw_text_fitted_cow(
                    Rect::new(band.x, line.y, band.width.max(1), line.height.max(1)),
                    preview_text,
                    preview_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
        // OK/Cancel buttons. The row's y comes from the same stack that fixes the preview
        // band, so the rows cannot drift; its x comes from the shared [`action_row_geometry`]
        // derivation, so this dialog and the five others that draw the same pair cannot
        // disagree about the button width, the gap between them, or where the row's left edge
        // falls. The labels are centred in their buttons and fitted to them.
        if self.no_buttons {
            return;
        }
        let band = super::message_box::inset_action_band(
            Rect::new(rect.x, self.button_row_top(), rect.width, Self::BUTTON_HEIGHT as u32),
            rect.y,
        );
        let labels = Self::action_labels();
        let row = super::message_box::action_row_geometry(context, font, &labels, band, true);
        let ok_rect = row.buttons[0];
        context.fill_rect(ok_rect, accent);
        context.draw_text_line_cow(
            ok_rect,
            &labels[0],
            font,
            accent_ink,
            HorizontalAlignment::Center,
        );
        let cancel_rect = row.buttons[1];
        context.fill_rect(cancel_rect, surface);
        context.draw_rect(cancel_rect, border);
        context.draw_text_line_cow(cancel_rect, &labels[1], font, ink, HorizontalAlignment::Center);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn mouse_pick_updates_color() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        dialog.handle_event(&Event::mouse_press(60, 80, 1));
        assert_ne!(dialog.current_color(), Color::rgb(255, 255, 255));
    }

    #[test]
    fn arrow_keys_nudge_channels() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        dialog.set_current_color(Color::rgb(100, 100, 100));
        dialog.handle_event(&Event::key_press(39, 0));
        assert_eq!(dialog.current_color().r, 105);
        dialog.handle_event(&Event::key_press(38, 0));
        assert_eq!(dialog.current_color().g, 105);
    }

    #[test]
    fn clicking_ok_accepts_and_clicking_cancel_rejects() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
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
        let ok = dialog.action_buttons()[0].center();
        dialog.handle_event(&Event::mouse_press(ok.x, ok.y, 1));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        let cancel = dialog.action_buttons()[1].center();
        dialog.handle_event(&Event::mouse_press(cancel.x, cancel.y, 1));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
        assert!(!dialog.is_visible());
    }

    #[test]
    fn set_current_color_emits_signal() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        let emitted = Arc::new(Mutex::new(Vec::<Color>::new()));
        let sink = emitted.clone();
        dialog.color_selected.connect(move |color| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*color);
            }
        });

        dialog.set_current_color(Color::rgb(1, 2, 3));

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![Color::rgb(1, 2, 3)]);
    }

    #[test]
    fn setting_the_same_color_again_is_a_no_op() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        dialog.set_current_color(Color::rgb(10, 20, 30));
        let emitted = Arc::new(Mutex::new(0usize));
        let sink = emitted.clone();
        dialog.color_selected.connect(move |_| {
            if let Ok(mut count) = sink.lock() {
                *count += 1;
            }
        });
        dialog.set_current_color(Color::rgb(10, 20, 30));
        assert_eq!(*emitted.lock().expect("lock"), 0, "a repeated write must not signal");
    }

    #[test]
    fn the_picker_maps_position_to_the_saturation_value_plane() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        let picker = dialog.picker_rect();
        // Top-left is white (saturation 0, value 1).
        dialog.handle_event(&Event::mouse_press(picker.x + 1, picker.y + 1, 1));
        let top_left = dialog.current_color();
        assert!(top_left.r > 240 && top_left.g > 240 && top_left.b > 240, "{top_left:?}");
        // Bottom-right is black (value 0).
        dialog.handle_event(&Event::mouse_press(
            picker.x + picker.width as i32 - 1,
            picker.y + picker.height as i32 - 1,
            1,
        ));
        let bottom_right = dialog.current_color();
        assert!(
            bottom_right.r < 10 && bottom_right.g < 10 && bottom_right.b < 10,
            "{bottom_right:?}"
        );
    }

    #[test]
    fn the_alpha_rail_changes_the_alpha_channel() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        dialog.set_options_alpha(true);
        let rail = dialog.alpha_rect().expect("alpha on exposes the rail");
        dialog.handle_event(&Event::mouse_press(rail.x + rail.width as i32 / 2, rail.y + 2, 1));
        let alpha = dialog.current_color().a;
        assert!((120..=135).contains(&alpha), "the midpoint is half alpha, got {alpha}");
    }

    #[test]
    fn hex_and_presets_are_published_properties() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        dialog
            .set("hex_rgba", CapabilityValue::String("#11223344".to_string()))
            .expect("hex is writable");
        assert_eq!(dialog.current_color(), Color::rgba(0x11, 0x22, 0x33, 0xFF));
        assert_eq!(dialog.get("hex_rgba"), Ok(CapabilityValue::String("#112233FF".to_string())));

        dialog.set("preset_index", CapabilityValue::UInt(6)).expect("preset is writable");
        assert_eq!(dialog.current_color(), PRESET_COLORS[6]);
        assert_eq!(
            dialog.set("preset_index", CapabilityValue::UInt(99)),
            Err(CapabilityAccessError::OutOfRange)
        );
        assert!(!dialog.set_hex("not a color"));
    }

    #[test]
    fn typing_into_the_hex_field_applies_the_color() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 400, 300));
        dialog.set_option(ColorDialogOption::ShowHexInput, true);
        let field = dialog.hex_field_rect().expect("a preview row carries the field");
        dialog.handle_event(&Event::mouse_press(field.x + 2, field.y + 2, 1));
        assert!(dialog.hex_focused);
        dialog.handle_event(&Event::TextInput { text: "#00FF00".to_string() });
        assert_eq!(dialog.current_color(), Color::rgb(0, 255, 0));
    }

    #[test]
    fn switching_options_off_restores_the_plain_row() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 400, 300));
        dialog.set_option(ColorDialogOption::ShowPresetPalette, true);
        assert!(!dialog.preset_rects().is_empty());
        dialog.set_option(ColorDialogOption::NoButtons, true);
        assert!(dialog.no_buttons);
        dialog.set_option(ColorDialogOption::ShowPresetPalette, false);
        assert!(dialog.preset_rects().is_empty());
    }

    #[test]
    fn the_hue_rail_moves_the_picker_hue() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 400, 300));
        dialog.set_current_color(Color::rgb(255, 0, 0));
        dialog.set_option(ColorDialogOption::ShowHueRail, true);
        let rail = dialog.hue_rect().expect("the option exposes the rail");
        // Two thirds of the way down is roughly 240 degrees: blue.
        dialog.handle_event(&Event::mouse_press(
            rail.x + 2,
            rail.y + (rail.height as i32 * 2) / 3,
            1,
        ));
        let color = dialog.current_color();
        assert!(
            color.b > color.r && color.b > color.g,
            "the rail must select a blue hue, got {color:?}"
        );
    }
}
