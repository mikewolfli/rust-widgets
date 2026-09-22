// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Color dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;

use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Color dialog for picking RGBA colors.
///
/// # Not a full picker
///
/// The "picker" area is a synthetic two-axis field, not a real palette: the x
/// axis drives the red and blue channels inversely and the y axis drives green,
/// so only a subset of RGB space is reachable by clicking. The alpha channel is
/// never changed by clicking — it is carried over from the previous color (or
/// forced to fully opaque when `options_alpha` is off).
///
/// Alpha is only ever preserved or set to `255`; there is no control for
/// choosing it.
///
pub struct ColorDialog {
    base: BaseWidget,
    current_color: Color,
    options_alpha: bool,
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
            modal: true,
            color_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
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
        self.modal = modal;
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
    /// Unlike most widgets' setters this is **not** a no-op for a repeated
    /// value — it always signals and always requests a redraw. A slot that
    /// calls back into a setter will therefore recurse.
    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
        self.color_selected.emit(color);
        self.base.request_redraw();
    }
    /// Enables or disables alpha handling for picker clicks. See
    /// [`ColorDialog::options_alpha`]. Does not request a redraw.
    pub fn set_options_alpha(&mut self, enabled: bool) {
        self.options_alpha = enabled;
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
        let height = (below_top - picker_top - Self::PICKER_GAP).max(0) as u32;
        Rect::new(rect.x + 10, picker_top, rect.width.saturating_sub(20), height)
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
        let rx = ((pos.x - picker.x) as f32 / w).clamp(0.0, 1.0);
        let ry = ((pos.y - picker.y) as f32 / h).clamp(0.0, 1.0);
        let r = (rx * 255.0).round() as u8;
        let g = ((1.0 - ry) * 255.0).round() as u8;
        let b = ((1.0 - rx) * 255.0).round() as u8;
        let a = if self.options_alpha { self.current_color.a } else { 255 };
        Some(Color::rgba(r, g, b, a))
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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            // The edited colour is not writable here: it changes through the picker
            // area and through `set_current_color`, both of which emit
            // `color_selected`. Making it writable through a generic property write
            // would be a second path that skips that signal.
            "current_color" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["current_color", "modal", "options_alpha", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `color_dialog` publishes.
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
impl EventHandler for ColorDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(color) = self.pick_color_from_point(*pos) {
                    self.set_current_color(color);
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == 13 {
                    self.accept();
                } else if *key == 27 {
                    self.reject();
                } else if *key == 37 {
                    self.nudge_rgb(-5, 0, 0);
                } else if *key == 39 {
                    self.nudge_rgb(5, 0, 0);
                } else if *key == 38 {
                    self.nudge_rgb(0, 5, 0);
                } else if *key == 40 {
                    self.nudge_rgb(0, -5, 0);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for ColorDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal,
        // so a light/dark switch left the panel, its title bar and its buttons unchanged —
        // the rendering census reported the control as theme-blind.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("color_dialog");
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
            context.fill_rect(rect, surface);
            context.draw_rect(rect, border);
        }
        // Every label below is fitted to the band it sits in. None of them was bounded, so
        // the two ends of the dialog carried absolute origins written for its 400 px size
        // hint — at the census rectangle the preview label ran to x=379 and the Cancel label
        // to x=360, both past the 240 px frame. The fit makes the label's own rectangle the
        // authority instead of the size the control was designed at.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, title_bar);
        let title_font = Font::default();
        let title_line = context.text_line(title_bar_band, &title_font);
        context.draw_text_fitted(
            Rect::new(
                rect.x + 8,
                title_line.y,
                rect.width.saturating_sub(16),
                title_line.height.max(1),
            ),
            &tr!("color_dialog.title"),
            &title_font,
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
            context.fill_rect(picker_rect, Color::rgb(200, 200, 200));
            context.draw_rect(picker_rect, border);
        }
        // Color preview. Drawn only when a band's worth of room is left between the
        // title bar and the button row; on a shorter dialog the picker takes that space
        // instead, which is the trade the fit test in `preview_row_top` makes.
        let preview_font = Font::default();
        let preview_text =
            format!("{} {}", tr!("color_dialog.current_color"), self.current_color.to_hex_rgba());
        if let Some(preview_y) = self.preview_row_top() {
            let preview_rect = Rect::new(rect.x + 10, preview_y, 60, Self::PREVIEW_HEIGHT as u32);
            context.fill_rect(preview_rect, self.current_color);
            context.draw_rect(preview_rect, border);
            // The hex readout sits in the strip between the swatch and the panel's right
            // margin, so a long hex string truncates there rather than running under the
            // buttons. It is centred on the 30 px swatch band through the shared primitive:
            // the glyph origin is the box's top-left, so the old `preview_y + 15` put the
            // line's top edge on the swatch's middle line.
            let band = Rect::new(
                rect.x + 80,
                preview_y,
                rect.width.saturating_sub(88),
                Self::PREVIEW_HEIGHT as u32,
            );
            let line = context.text_line(band, &preview_font);
            context.draw_text_fitted(
                Rect::new(band.x, line.y, band.width.max(1), line.height.max(1)),
                &preview_text,
                &preview_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // OK/Cancel buttons. The row's y comes from the same stack that fixes the preview
        // band, so the rows cannot drift; its x comes from the shared [`action_row_geometry`]
        // derivation, so this dialog and the five others that draw the same pair cannot
        // disagree about the button width, the gap between them, or where the row's left edge
        // falls. The labels are centred in their buttons and fitted to them.
        let band = Rect::new(rect.x, self.button_row_top(), rect.width, Self::BUTTON_HEIGHT as u32);
        let labels = vec![tr!("common.button.ok"), tr!("common.button.cancel")];
        let row = super::message_box::action_row_geometry(context, &labels, band, true);
        let font = Font::default();
        let ok_rect = row.buttons[0];
        context.fill_rect(ok_rect, accent);
        context.draw_text_line(ok_rect, &labels[0], &font, accent_ink, HorizontalAlignment::Center);
        let cancel_rect = row.buttons[1];
        context.fill_rect(cancel_rect, surface);
        context.draw_rect(cancel_rect, border);
        context.draw_text_line(cancel_rect, &labels[1], &font, ink, HorizontalAlignment::Center);
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
}
