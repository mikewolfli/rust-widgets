// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Font dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;

use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Font selection dialog.
///
/// Holds the font the user is choosing and signals the outcome. It does not
/// enumerate or preview installed fonts — the caller supplies candidate fonts
/// through [`FontDialog::set_current_font`].
pub struct FontDialog {
    base: BaseWidget,
    current_font: Font,
    modal: bool,
    /// Emitted with the new font on every change, including while the user is
    /// still browsing, and again from [`FontDialog::accept`]. A slot that reads
    /// it as "confirmed" will fire on unconfirmed selections; use `accepted` for
    /// the commit point.
    pub font_selected: Signal1<Font>,
    /// Emitted by [`FontDialog::accept`], after `font_selected`. Carries no
    /// payload.
    pub accepted: GenericSignal,
    /// Emitted by [`FontDialog::reject`]. The current font is **not** restored
    /// to its pre-dialog value, so a caller that needs cancel semantics must
    /// snapshot the original font itself.
    pub rejected: GenericSignal,
}
impl FontDialog {
    /// Creates a dialog with the default font and modality on.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 400x300.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FontDialog, geometry, "FontDialog"),
            current_font: Font::default(),
            modal: true,
            font_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    /// Returns the font currently selected in the dialog.
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    /// Replaces the selected font and emits `font_selected`.
    ///
    /// Always signals and always requests a redraw, even for an unchanged
    /// value, so a slot that calls back into this setter will recurse.
    pub fn set_current_font(&mut self, font: Font) {
        self.current_font = font.clone();
        self.font_selected.emit(font);
        self.base.request_redraw();
    }
    /// Confirms the dialog: emits `font_selected` with the current font, then
    /// `accepted`, then hides.
    ///
    /// Note the current font is reported twice in total — once from the
    /// preceding `set_current_font` calls and once here — so a slot connected
    /// to `font_selected` will see a duplicate for the final value.
    pub fn accept(&mut self) {
        self.font_selected.emit(self.current_font.clone());
        self.accepted.emit();
        self.hide();
    }
    /// Cancels the dialog: emits `rejected` and hides. The selected font is
    /// left as-is; see [`FontDialog::rejected`].
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
    /// Returns a copy of the selected font. Equivalent to cloning
    /// [`FontDialog::current_font`]; kept for callers using the
    /// `get_font`/`set_current_font` pairing.
    pub fn get_font(&self) -> Font {
        self.current_font.clone()
    }
    /// Returns whether the dialog is modal. Defaults to `true`.
    ///
    /// Records the intent; enforcement is the modal stack in
    /// [`crate::widget::runtime`] (`enter_modal` / `exit_modal`).
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    /// Sets the modality intent and requests a redraw. See
    /// [`FontDialog::is_modal`].
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
        self.base.request_redraw();
    }
}
impl Widget for FontDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 300)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `FontDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `FontDialog`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. `modal` is read-only: the old write layer
/// had no arm for this kind.
impl WidgetProperties for FontDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["modal", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `font_dialog` publishes.
    ///
    /// `accept` and `reject` are the dialog's own commit/cancel actions and map
    /// onto its real `accept` / `reject`; neither takes a payload. `set_current_font`
    /// needs a font, so a bare invocation is reported as needing one rather than
    /// being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "accept" => {
                self.accept();
                Ok(())
            }
            "reject" => {
                self.reject();
                Ok(())
            }
            "set_current_font" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for FontDialog {
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
impl Draw for FontDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
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
        let theme = crate::style::resolved_theme_style("font_dialog");
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
        // The three list columns and the preview are field interiors: one step *away* from
        // the dialog surface, so on a light theme darker and on a dark one lighter rather
        // than the forced white both used to be.
        let field = if surface.is_dark() {
            surface.blend(&Color::WHITE, 0.08)
        } else {
            surface.blend(&Color::BLACK, 0.06)
        };

        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), surface);
        context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), border);
        // Title bar: a separate region from the dialog surface, in the theme's accent
        // rather than the literal blue it carried before.
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, 28), accent);
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &tr!("dialog.font.select_font"),
            &Font::default(),
            accent_ink,
            HorizontalAlignment::Left,
        );
        let col_w = (rect.width / 3).saturating_sub(6);
        let list_y = rect.y + 38;
        let list_h = rect.height.saturating_sub(120);
        // Family, Style, Size columns
        let col_labels =
            [tr!("dialog.font.font_family"), tr!("dialog.font.style"), tr!("dialog.font.size")];
        for (i, label) in col_labels.iter().enumerate() {
            let col_x = rect.x as f32 + 4.0 + i as f32 * (col_w as f32 + 4.0);
            context.draw_text(
                Point::new(col_x as i32, list_y - 10),
                label.as_str(),
                &Font::default(),
                ink,
                HorizontalAlignment::Left,
            );
            context.fill_rect(Rect::new(col_x as i32, list_y, col_w, list_h), field);
            context.draw_rect(Rect::new(col_x as i32, list_y, col_w, list_h), border);
        }
        // Preview area
        let prev_y = list_y + list_h as i32 + 8;
        let bw = rect.width.saturating_sub(8);
        context.fill_rect(Rect::new(rect.x + 4, prev_y, bw, 36), field);
        context.draw_rect(Rect::new(rect.x + 4, prev_y, bw, 36), border);
        context.draw_text(
            Point::new(rect.x + 10, prev_y + 18),
            "AaBbYyZz 0123",
            &self.current_font,
            ink,
            HorizontalAlignment::Left,
        );
        // OK/Cancel
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        context
            .fill_rect(Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, 80, 28), accent);
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            &tr!("dialog.ok"),
            &Font::default(),
            accent_ink,
            HorizontalAlignment::Left,
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, 80, 28),
            surface.blend(&ink, 0.1),
        );
        context.draw_rect(Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, 80, 28), border);
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 48, (btn_y + 14.0) as i32),
            &tr!("dialog.cancel"),
            &Font::default(),
            ink,
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn set_current_font_emits_font_selected() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
        let seen = Arc::new(Mutex::new(Font::default()));
        let seen_clone = Arc::clone(&seen);

        dialog.font_selected.connect(move |font| {
            if let Ok(mut f) = seen_clone.lock() {
                *f = (*font).clone();
            }
        });

        let target = Font::simple("Sans", 18.0);
        dialog.set_current_font(target.clone());
        assert_eq!(dialog.current_font(), &target);
        assert_eq!(*seen.lock().expect("seen lock"), target);
    }

    #[test]
    fn enter_accepts_and_escape_rejects() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
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
