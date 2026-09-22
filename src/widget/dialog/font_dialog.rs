// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Font dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
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
use crate::widget::metrics::{dimensions, ControlMetrics};
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
impl FontDialog {
    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered**. Painting it verbatim drew the 240x120
    /// census cell as a 240x120 frame whose three columns and preview were stacked from
    /// literals written for the 400 px default size. [`ControlMetrics::painted_box`] caps
    /// each axis at the dialog's own intrinsic size and centres what is left; every band
    /// below — the title strip, the column header row, the columns and the button row — is
    /// derived from this one rect.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }
}

impl Draw for FontDialog {
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
        // Title bar: a separate region from the dialog surface, in the theme's accent
        // rather than the literal blue it carried before. The label is centred on the bar's
        // own band through the shared primitive and fitted to the bar, so a truncating
        // locale cannot run the title past the frame.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, accent);
        let title_font = Font::default();
        let title_label = tr!("dialog.font.select_font");
        let title_line = context.text_line(title_bar_band, &title_font);
        context.draw_text_fitted(
            Rect::new(
                rect.x + 8,
                title_line.y,
                rect.width.saturating_sub(16),
                title_line.height.max(1),
            ),
            &title_label,
            &title_font,
            accent_ink,
            HorizontalAlignment::Left,
        );
        // The columns share the space left after the button row is reserved, so a third
        // of an already-short dialog cannot reach below the buttons. `col_w` is derived
        // from that clamped height rather than from the dialog's own, which is what made
        // the last column's label leave the frame. The four tracks (three columns plus the
        // gaps between them) are laid out from the frame's own padding, and the trailing
        // column takes the exact remainder so a rounding remainder cannot push it out.
        let btn_h = dimensions::DIALOG_BUTTON_HEIGHT as i32;
        let button_band = ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT);
        let button_top = button_band.y;
        const COL_GAP: i32 = 4;
        let col_w = ((rect.width as i32 - 8 - COL_GAP * 2) / 3).max(1) as u32;
        // The header strip is sized from the font's own line box first, then the columns
        // start below it. Deriving `list_y` from the strip (instead of the strip from
        // `list_y`) is what removes the collision at the source: the old form hardcoded
        // `list_y = rect.y + 38` and then placed the header 10 px above it, which is
        // `rect.y + 28` — exactly the title bar's bottom edge, and one pixel short of the
        // 14 px line box the default font needs. The label was therefore drawn touching the
        // accent bar and hanging out of its own 12 px strip.
        //
        // A column header in Qt (`QHeaderView`), Flutter (`DataTable`) and SwiftUI
        // (`TableColumn`) sits wholly inside its own header row, and the row is at least as
        // tall as the text it holds. Sizing from `measure_text` is what makes those two
        // agree by construction rather than by a tuned pair of literals.
        let header_metrics = context.measure_text("M", &Font::default());
        let header_h = header_metrics.height.max(1);
        let list_y = rect.y + dimensions::DIALOG_TITLE_BAR_HEIGHT as i32 + 2 + header_h as i32 + 2;
        // The columns are the dialog's primary content, so they take the space left
        // between the list's real top and the rows reserved *below* it, and the preview
        // well is the band that yields when there is not room for both.
        //
        // The old form subtracted a second copy of `list_y` as the literal 46 —
        // `(button_top - 46 - list_y)` — which is the 28 px title-bar offset plus the
        // 8 px header strip plus the 2 px margins, i.e. the *same* 46 that `list_y`
        // already was. The subtrahend therefore cancelled the minuend exactly and every
        // column collapsed to `height="0"` in `font_dialog.svg` while its header still
        // painted at y=30: a reservation subtracted a second time is the defect.
        //
        // The preview band is then offered only what is genuinely left over, and only
        // if that is a whole line's worth. At the 120 px box there is no room for it and
        // it is dropped rather than squeezing the columns back to zero, which would be
        // the same defect with a different constant.
        let preview_gap = 8i32;
        let preview_metrics = context.measure_text("AaBbYyZz 0123", &self.current_font);
        let min_preview_h = preview_metrics.height as i32 + 4;
        let wanted_preview_h = 36i32;
        // Height a preview band would take if drawn, including the gap above it.
        let preview_band = wanted_preview_h + preview_gap;
        let space_below_list = (button_top - list_y).max(0);
        let (list_h, preview_h) = if space_below_list >= preview_band + min_preview_h {
            // Both the columns and the preview fit: the columns take the remainder.
            ((space_below_list - preview_band) as u32, wanted_preview_h)
        } else {
            // Not enough for both. The columns keep the whole remainder and the preview
            // is dropped; a band that cannot hold a line is not worth a column.
            (space_below_list as u32, 0)
        };
        let header_top = (list_y - 2 - header_h as i32).max(rect.y);
        // Family, Style, Size columns. Each column's box is derived from the frame's own
        // padding plus its index, so the third column cannot leave the frame; the label is
        // fitted to its column and centred on the header row's band.
        let col_labels =
            [tr!("dialog.font.font_family"), tr!("dialog.font.style"), tr!("dialog.font.size")];
        let header_band = Rect::new(rect.x, header_top, rect.width, header_h);
        let header_line = context.text_line(header_band, &Font::default());
        for (i, label) in col_labels.iter().enumerate() {
            let col_x = rect.x + 4 + i as i32 * (col_w as i32 + COL_GAP);
            context.draw_text_fitted(
                Rect::new(col_x, header_line.y, col_w, header_line.height.max(1)),
                label.as_str(),
                &Font::default(),
                ink,
                HorizontalAlignment::Left,
            );
            // The column body is drawn only when it has real extent: a zero-height column
            // is an element the SVG backend emits while the rasteriser skips it.
            if list_h > 0 {
                context.fill_rect(Rect::new(col_x, list_y, col_w, list_h), field);
                context.draw_rect(Rect::new(col_x, list_y, col_w, list_h), border);
            }
        }
        // Preview area. Placed on the same gap the columns reserved above, so the two
        // derivations cannot drift apart into an overlap or a hole between them. It has
        // zero height exactly when the layout above decided it did not fit, and is not
        // drawn at all in that case — a zero-height well would be the same
        // invisible-rectangle defect the columns had.
        let preview_font = self.current_font.clone();
        if preview_h > 0 {
            let prev_y = list_y + list_h as i32 + preview_gap;
            let bw = rect.width.saturating_sub(8);
            context.fill_rect(Rect::new(rect.x + 4, prev_y, bw, preview_h as u32), field);
            context.draw_rect(Rect::new(rect.x + 4, prev_y, bw, preview_h as u32), border);
            // The sample is fitted to the preview well, so a caller-selected font larger
            // than the well is truncated there instead of marching out of the dialog.
            // Centred via the shared primitive rather than by repeating the well's height
            // in a second subtraction, which is what the literal 36 here would have
            // become if the well ever changed size.
            let band =
                Rect::new(rect.x + 10, prev_y, rect.width.saturating_sub(20), preview_h as u32);
            let line = context.text_line(band, &preview_font);
            context.draw_text_fitted(
                Rect::new(band.x, line.y, band.width, preview_metrics.height.max(1)),
                "AaBbYyZz 0123",
                &preview_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // OK/Cancel. Right-aligned inside the frame and floored at its left edge, so a
        // control narrower than the two 80 px buttons keeps them on screen; the row is the
        // frame's bottom band and the labels are centred in their buttons and fitted to them.
        let btn_y = button_top;
        let btn_w = 80i32.min(button_band.width as i32).max(1);
        let btn_step = btn_w + 8;
        let cancel_x = (rect.x + rect.width as i32 - btn_step).max(rect.x);
        let ok_x = (cancel_x - btn_step).max(rect.x);
        let ok_rect = Rect::new(ok_x, btn_y, btn_w as u32, btn_h.max(1) as u32);
        context.fill_rect(ok_rect, accent);
        context.draw_text_line(
            ok_rect,
            &tr!("dialog.ok"),
            &Font::default(),
            accent_ink,
            HorizontalAlignment::Center,
        );
        let cancel_rect = Rect::new(cancel_x, btn_y, btn_w as u32, btn_h.max(1) as u32);
        context.fill_rect(cancel_rect, surface.blend(&ink, 0.1));
        context.draw_rect(cancel_rect, border);
        context.draw_text_line(
            cancel_rect,
            &tr!("dialog.cancel"),
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
