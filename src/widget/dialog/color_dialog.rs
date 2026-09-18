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

    fn picker_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + 10,
            rect.y + 38,
            rect.width.saturating_sub(20),
            rect.height.saturating_sub(120),
        )
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
        let rect = self.geometry();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(160, 160, 160),
        );
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, 28), Color::rgb(0, 120, 215));
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &tr!("color_dialog.title"),
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        // Color picker area (simplified)
        let picker_rect = self.picker_rect();
        context.fill_rect(picker_rect, Color::rgb(200, 200, 200));
        context.draw_rect(picker_rect, Color::rgb(100, 100, 100));
        // Color preview
        let preview_y = rect.y as f32 + rect.height as f32 - 80.0;
        context.fill_rect(Rect::new(rect.x + 10, preview_y as i32, 60, 30), self.current_color);
        context.draw_rect(Rect::new(rect.x + 10, preview_y as i32, 60, 30), Color::rgb(0, 0, 0));
        context.draw_text(
            Point::new(rect.x + 80, (preview_y + 15.0) as i32),
            &format!("{} {}", tr!("color_dialog.current_color"), self.current_color.to_hex_rgba()),
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        // OK/Cancel buttons
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        let btn_w = 80;
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, btn_w, 28),
            Color::rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            &tr!("common.button.ok"),
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::rgb(100, 100, 100),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 48, (btn_y + 14.0) as i32),
            &tr!("common.button.cancel"),
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
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
