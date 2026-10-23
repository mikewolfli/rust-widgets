// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Font dialog widget.
use crate::compat::{format, String, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::platform::SystemFont;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;

use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Font selection dialog.
///
/// Holds the font the user is choosing and signals the outcome. It enumerates the host's
/// installed families through [`crate::platform::fonts`] (one process-wide snapshot), offers the
/// four style combinations and a size list, and applies a click or arrow-key selection to
/// `current_font` by emitting `font_selected`.
///
/// A host that has its own list can replace the catalogue with [`FontDialog::set_font_families`],
/// which is also what a designer preview with a synthetic list uses.
pub struct FontDialog {
    base: BaseWidget,
    current_font: Font,
    modal: bool,
    families: Vec<String>,
    sizes: Vec<u32>,
    family_index: usize,
    style_index: usize,
    size_index: usize,
    family_scroll: usize,
    style_scroll: usize,
    size_scroll: usize,
    active_column: usize,
    /// The OK button's rect from the last paint, so a press resolves to the button the user saw.
    ok_button: Rect,
    /// The Cancel button's rect from the last paint.
    cancel_button: Rect,
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

/// One entry in the Style column.
struct StyleChoice {
    label: &'static str,
    weight: u16,
    italic: bool,
}

/// The styles every backend can honour, because `Font` is family/size/weight/italic.
const STYLES: [StyleChoice; 5] = [
    StyleChoice { label: "Regular", weight: Font::REGULAR_WEIGHT, italic: false },
    StyleChoice { label: "Bold", weight: Font::BOLD_WEIGHT, italic: false },
    StyleChoice { label: "Italic", weight: Font::REGULAR_WEIGHT, italic: true },
    StyleChoice { label: "Bold Italic", weight: Font::BOLD_WEIGHT, italic: true },
    StyleChoice { label: "Light", weight: 300, italic: false },
];

/// Sizes offered when the current size is not already in the list.
const SIZES: [u32; 17] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 48, 64, 72];

/// The row height shared by drawing and hit-testing.
const ROW_HEIGHT: i32 = 20;
/// The column header band's height; derived from the font size in [`FontDialog::header_height`].
const MIN_HEADER_HEIGHT: i32 = 16;
/// The gap between the column list and the preview well.
const PREVIEW_GAP: i32 = 8;
/// The preview well's height.
const PREVIEW_HEIGHT: u32 = 36;

/// The geometry both drawing and hit-testing derive from.
#[derive(Clone, Copy)]
struct DialogLayout {
    rect: Rect,
    list_y: i32,
    list_h: u32,
    preview_h: u32,
    col_w: u32,
}
impl FontDialog {
    /// Creates a dialog with the default font and modality on.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 400x300.
    pub fn new(geometry: Rect) -> Self {
        let mut dialog = Self {
            base: BaseWidget::new(WidgetKind::FontDialog, geometry, "FontDialog"),
            current_font: Font::default(),
            modal: true,
            families: Vec::new(),
            sizes: SIZES.to_vec(),
            family_index: 0,
            style_index: 0,
            size_index: 0,
            family_scroll: 0,
            style_scroll: 0,
            size_scroll: 0,
            active_column: 0,
            ok_button: Rect::new(0, 0, 0, 0),
            cancel_button: Rect::new(0, 0, 0, 0),
            font_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        };
        dialog.reload_system_fonts();
        dialog
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
        self.sync_indices();
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
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        self.base.request_redraw();
    }

    /// Rebuilds the catalogue from the host's installed-font snapshot and resets the selection to
    /// the current font.
    ///
    /// Returns the number of families now in the catalogue. The list comes from the
    /// process-wide snapshot in [`crate::platform::fonts`], so opening several dialogs does not
    /// repeat the OS query; a backend that reports no installed fonts falls back to the families
    /// compiled into this build. A caller that has just installed a font should call
    /// [`crate::platform::fonts::refresh_system_fonts`] first.
    pub fn reload_system_fonts(&mut self) -> usize {
        self.set_font_families(crate::platform::fonts::chooser_families());
        self.families.len()
    }

    /// Replaces the catalogue from a host-supplied list.
    ///
    /// The families are sorted and de-duplicated case-insensitively; the current family is kept
    /// even when the host's list does not contain it, so an externally set font stays visible.
    pub fn set_system_fonts(&mut self, fonts: &[SystemFont]) {
        let families = fonts.iter().map(|font| font.family.clone()).collect();
        self.set_font_families(families);
    }

    /// Replaces the family column with `families`.
    pub fn set_font_families(&mut self, mut families: Vec<String>) {
        families.retain(|family| !family.trim().is_empty());
        families.sort_unstable_by_key(|family| family.to_lowercase());
        families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        self.families = families;
        self.family_scroll = 0;
        self.sync_indices();
        self.base.request_redraw();
    }

    /// The families currently offered, in display order.
    pub fn font_families(&self) -> &[String] {
        &self.families
    }

    /// The style labels currently offered.
    pub fn font_styles(&self) -> Vec<&'static str> {
        STYLES.iter().map(|style| style.label).collect()
    }

    /// The sizes currently offered, ascending.
    pub fn font_sizes(&self) -> &[u32] {
        &self.sizes
    }

    /// Recomputes the selected row in each column from `current_font`.
    fn sync_indices(&mut self) {
        let family = self.current_font.family().trim().to_string();
        if !family.is_empty()
            && !self.families.iter().any(|candidate| candidate.eq_ignore_ascii_case(&family))
        {
            self.families.push(family);
            self.families.sort_unstable_by_key(|family| family.to_lowercase());
        }
        self.family_index = self
            .families
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(self.current_font.family()))
            .unwrap_or(0);
        self.size_index =
            match self.sizes.iter().position(|size| *size == self.current_font.size_u32()) {
                Some(index) => index,
                None => {
                    let size = self.current_font.size_u32();
                    let index = self
                        .sizes
                        .iter()
                        .position(|candidate| *candidate > size)
                        .unwrap_or(self.sizes.len());
                    self.sizes.insert(index, size);
                    index
                }
            };
        let weight = self.current_font.weight();
        self.style_index = if weight >= 600 && self.current_font.is_italic() {
            3
        } else if weight >= 600 {
            1
        } else if self.current_font.is_italic() {
            2
        } else if weight < 400 {
            4
        } else {
            0
        };
    }

    /// The header band height, from the dialog chrome font's size.
    fn header_height(&self) -> i32 {
        let style = self.style();
        let font = effective_font(style);
        (font.size().ceil() as i32 + 2).max(MIN_HEADER_HEIGHT)
    }

    /// The geometry the columns and rows use.
    fn layout(&self) -> DialogLayout {
        let rect = self.frame_rect();
        let list_y =
            rect.y + dimensions::DIALOG_TITLE_BAR_HEIGHT as i32 + 2 + self.header_height() + 2;
        let button_top = ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT).y;
        let space_below_list = (button_top - list_y).max(0);
        // The preview well is offered only when the list keeps at least a row below the header
        // and the well plus its gap fit. `draw` reads the same fields, so a click can never
        // address a row the painter did not draw.
        let preview_extent = PREVIEW_HEIGHT as i32 + PREVIEW_GAP;
        let (list_h, preview_h) = if space_below_list >= preview_extent + MIN_HEADER_HEIGHT + 2 {
            ((space_below_list - preview_extent) as u32, PREVIEW_HEIGHT)
        } else {
            (space_below_list as u32, 0)
        };
        const COL_GAP: i32 = 4;
        let col_w = ((rect.width as i32 - 8 - COL_GAP * 2) / 3).max(1) as u32;
        DialogLayout { rect, list_y, list_h, preview_h, col_w }
    }

    /// How many rows fit in a column.
    fn visible_rows(&self) -> usize {
        (self.layout().list_h as i32 / ROW_HEIGHT).max(1) as usize
    }

    /// Scroll offsets, by column.
    fn scroll(&self, column: usize) -> usize {
        match column {
            0 => self.family_scroll,
            1 => self.style_scroll,
            _ => self.size_scroll,
        }
    }

    fn scroll_mut(&mut self, column: usize) -> &mut usize {
        match column {
            0 => &mut self.family_scroll,
            1 => &mut self.style_scroll,
            _ => &mut self.size_scroll,
        }
    }

    /// The selected row, by column.
    fn selected(&self, column: usize) -> usize {
        match column {
            0 => self.family_index,
            1 => self.style_index,
            _ => self.size_index,
        }
    }

    /// Applies the current row selections to `current_font` and emits `font_selected`.
    fn apply_selection(&mut self) {
        let mut font = self.current_font.clone();
        if let Some(family) = self.families.get(self.family_index) {
            // The renderer matches faces by name, so a system family only draws as itself once
            // its file is registered. Backends that report no file (`None`) fall through to the
            // compiled faces; the family name still changes so the caller sees the selection.
            #[cfg(all(feature = "runtime-fonts", not(alloc_frugal)))]
            {
                crate::platform::fonts::register_family(family);
            }
            font.set_family(family.as_str());
        }
        if let Some(style) = STYLES.get(self.style_index) {
            font = font.with_weight_value(style.weight).with_italic(style.italic);
        }
        if let Some(size) = self.sizes.get(self.size_index) {
            font.set_size(*size as f32);
        }
        self.set_current_font(font);
    }

    /// Selects `row` in `column` and scrolls it into view.
    fn select_row(&mut self, column: usize, row: usize) {
        let length = match column {
            0 => self.families.len(),
            1 => STYLES.len(),
            _ => self.sizes.len(),
        };
        if length == 0 {
            return;
        }
        let row = row.min(length - 1);
        match column {
            0 => self.family_index = row,
            1 => self.style_index = row,
            _ => self.size_index = row,
        }
        let visible = self.visible_rows();
        let start = self.scroll(column);
        if row < start {
            *self.scroll_mut(column) = row;
        } else if row >= start + visible {
            *self.scroll_mut(column) = row + 1 - visible;
        }
        self.active_column = column;
        self.apply_selection();
    }

    /// Moves the active column's selection by `delta`, clamping at both ends.
    fn move_selection(&mut self, delta: i32) {
        let column = self.active_column;
        let length = match column {
            0 => self.families.len(),
            1 => STYLES.len(),
            _ => self.sizes.len(),
        };
        if length == 0 {
            return;
        }
        let current = self.selected(column) as i32;
        let next = (current + delta).clamp(0, length as i32 - 1) as usize;
        if next != current as usize {
            self.select_row(column, next);
        }
        self.base.request_redraw();
    }

    /// The column row under `pos`, if any.
    fn row_at(&self, pos: crate::core::Point) -> Option<(usize, usize)> {
        let layout = self.layout();
        if layout.list_h == 0
            || pos.y < layout.list_y
            || pos.y >= layout.list_y + layout.list_h as i32
        {
            return None;
        }
        let row = ((pos.y - layout.list_y) / ROW_HEIGHT) as usize;
        const COL_GAP: i32 = 4;
        for column in 0..3 {
            let x = layout.rect.x + 4 + column as i32 * (layout.col_w as i32 + COL_GAP);
            if pos.x >= x && pos.x < x + layout.col_w as i32 {
                let index = self.scroll(column) + row;
                return Some((column, index));
            }
        }
        None
    }
}
impl Widget for FontDialog {
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

    /// Reports this widget as the object that paints it.
    ///
    /// `FontDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

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
            "font_selected" => {
                Some(EventSignalRef::mapped("font_selected", &self.font_selected, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "accepted" => Some(EventSignalRef::unit("accepted", &self.accepted)),
            "rejected" => Some(EventSignalRef::unit("rejected", &self.rejected)),
            _ => None,
        }
    }
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
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                // The buttons keep the rects the painter used, so a click cannot land between a
                // visible button and a separately derived one.
                if self.ok_button.contains_point(*pos) {
                    self.accept();
                    return;
                }
                if self.cancel_button.contains_point(*pos) {
                    self.reject();
                    return;
                }
                if let Some((column, row)) = self.row_at(*pos) {
                    self.select_row(column, row);
                    return;
                }
                // A press in the title, preview or button band is not a row selection, but it
                // does make the dialog's active column explicit for the next wheel/key press.
                let layout = self.layout();
                if pos.y >= layout.list_y {
                    self.active_column = 0;
                }
            }
            Event::Wheel { delta, .. } => {
                let column = self.active_column;
                let length = match column {
                    0 => self.families.len(),
                    1 => STYLES.len(),
                    _ => self.sizes.len(),
                };
                let visible = self.visible_rows();
                let max_scroll = length.saturating_sub(visible);
                let start = self.scroll(column) as i32;
                let next = (start + delta.y).clamp(0, max_scroll as i32) as usize;
                if next != self.scroll(column) {
                    *self.scroll_mut(column) = next;
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } => match *key {
                key_codes::ENTER => self.accept(),
                key_codes::ESCAPE => self.reject(),
                key_codes::TAB => {
                    self.active_column = (self.active_column + 1) % 3;
                    self.base.request_redraw();
                }
                key_codes::LEFT => {
                    self.active_column = self.active_column.saturating_sub(1);
                    self.base.request_redraw();
                }
                key_codes::RIGHT => {
                    self.active_column = (self.active_column + 1).min(2);
                    self.base.request_redraw();
                }
                key_codes::UP => self.move_selection(-1),
                key_codes::DOWN => self.move_selection(1),
                _ => {}
            },
            _ => {}
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
        // A modal dialog dims the page behind it: that dimming is the whole visual difference
        // between "this dialog is up and the page is inert" and "a panel was drawn on the page".
        // The `modal` flag was recorded and enforced by the modal stack, and nothing painted it.
        if self.modal {
            super::draw_modal_scrim(context, self.geometry());
        }
        // One geometry for both painting and hit-testing. A second derivation here is what
        // previously made a click land on a row the painter had not drawn: the two sides read
        // different constants for the header, the preview band and the column width.
        let layout = self.layout();
        let rect = layout.rect;
        let style = self.style();
        // The **effective font** — the resolved theme/caller font — for the title, the column
        // headers and the buttons. The preview sample keeps `self.current_font`, which is the
        // user's selected font and deliberately not the theme's (D09-STYLE-01).
        let font = effective_font(style);

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
        let theme = crate::style::resolved_theme_style_shared("font_dialog");
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
        // Title bar: a separate region from the dialog surface, in the theme's accent
        // rather than the literal blue it carried before. The label is centred on the bar's
        // own band through the shared primitive and fitted to the bar, so a truncating
        // locale cannot run the title past the frame.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, accent);
        let title_font = font;
        let title_label = tr!("dialog.font.select_font");
        let title_line = context.text_line(title_bar_band, title_font);
        context.draw_text_fitted_cow(
            Rect::new(
                rect.x + 8,
                title_line.y,
                rect.width.saturating_sub(16),
                title_line.height.max(1),
            ),
            &title_label,
            title_font,
            accent_ink,
            HorizontalAlignment::Left,
        );
        // The button row is reserved first; every band above it is derived from what is left,
        // through `layout()`, so the painter and `row_at()` cannot disagree.
        let button_band = super::message_box::inset_action_band(
            ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT),
            rect.y,
        );
        const COL_GAP: i32 = 4;
        let col_w = layout.col_w;
        let list_y = layout.list_y;
        let list_h = layout.list_h;
        let header_h = self.header_height();
        let header_top = (list_y - 2 - header_h).max(rect.y);
        // Family, Style, Size columns. Each column's box is derived from the frame's own
        // padding plus its index, so the third column cannot leave the frame; the label is
        // fitted to its column and centred on the header row's band.
        let col_labels =
            [tr!("dialog.font.font_family"), tr!("dialog.font.style"), tr!("dialog.font.size")];
        let header_band = Rect::new(rect.x, header_top, rect.width, header_h as u32);
        let header_line = context.text_line(header_band, font);
        for (column, label) in col_labels.iter().enumerate() {
            let col_x = rect.x + 4 + column as i32 * (col_w as i32 + COL_GAP);
            context.draw_text_fitted_cow(
                Rect::new(col_x, header_line.y, col_w, header_line.height.max(1)),
                label.as_str(),
                font,
                ink,
                HorizontalAlignment::Left,
            );
            // The column body is drawn only when it has real extent: a zero-height column
            // is an element the SVG backend emits while the rasteriser skips it.
            if list_h > 0 {
                let column_rect = Rect::new(col_x, list_y, col_w, list_h);
                context.fill_rect(column_rect, field);
                // The active column carries the accent outline, so an arrow key or a wheel
                // scroll has a visible owner.
                let outline = if column == self.active_column { accent } else { border };
                context.draw_rect(column_rect, outline);
            }
        }
        // Rows. Every column reads the same `list_y`, `list_h` and `ROW_HEIGHT` the hit test
        // reads, and is clipped to its own well so a long family name or a scrolled row list
        // cannot paint over a neighbouring column or into the preview band.
        if list_h > 0 {
            let rows = (list_h as i32 / ROW_HEIGHT).max(0) as usize;
            // One reused buffer for the size column: the labels were collected into a
            // fresh `Vec<String>` every frame for values that are one or two digits.
            use core::fmt::Write as _;
            let mut size_label = String::new();
            for column in 0..3 {
                let col_x = rect.x + 4 + column as i32 * (col_w as i32 + COL_GAP);
                let start = self.scroll(column);
                let selected = self.selected(column);
                context.push_clip(col_x, list_y, col_w, list_h);
                for row in 0..rows {
                    let index = start + row;
                    let label = match column {
                        0 => self.families.get(index).map(String::as_str),
                        1 => STYLES.get(index).map(|style| style.label),
                        _ => match self.sizes.get(index) {
                            Some(size) => {
                                size_label.clear();
                                let _ = write!(size_label, "{size}");
                                Some(size_label.as_str())
                            }
                            None => None,
                        },
                    };
                    let label = match label {
                        Some(label) => label,
                        None => break,
                    };
                    let row_rect = Rect::new(
                        col_x,
                        list_y + row as i32 * ROW_HEIGHT,
                        col_w,
                        ROW_HEIGHT as u32,
                    );
                    let row_ink = if index == selected {
                        context.fill_rect(row_rect, accent);
                        accent_ink
                    } else {
                        ink
                    };
                    context.draw_text_line_cow(
                        Rect::new(
                            row_rect.x + 4,
                            row_rect.y,
                            row_rect.width.saturating_sub(8),
                            row_rect.height,
                        ),
                        label,
                        font,
                        row_ink,
                        HorizontalAlignment::Left,
                    );
                }
                context.pop_clip();
            }
        }
        // Preview area: the sample is drawn in the user's selected font, not the theme's, and
        // the well has zero height exactly when `layout()` decided there was no room for it.
        if layout.preview_h > 0 {
            let prev_y = list_y + list_h as i32 + PREVIEW_GAP;
            let well =
                Rect::new(rect.x + 4, prev_y, rect.width.saturating_sub(8), layout.preview_h);
            context.fill_rect(well, field);
            context.draw_rect(well, border);
            // Clipped for the same reason the columns are: a caller may have selected a size
            // larger than the well, and the sample must be cut off rather than drawn over the
            // button row.
            context.push_clip(well.x, well.y, well.width, well.height);
            context.draw_text_line_cow(
                Rect::new(well.x + 6, well.y, well.width.saturating_sub(12), well.height),
                "AaBbYyZz 0123",
                &self.current_font,
                ink,
                HorizontalAlignment::Left,
            );
            context.pop_clip();
        }
        // OK/Cancel. Right-aligned inside the frame through the shared
        // [`action_row_geometry`] derivation, so this dialog and the five others that draw the
        // same pair cannot disagree about the button width, the gap between them or where the
        // row's left edge falls. The labels are centred in their buttons and fitted to them.
        let labels = vec![
            super::localized_label("dialog.ok", "OK"),
            super::localized_label("dialog.cancel", "Cancel"),
        ];
        let row =
            super::message_box::action_row_geometry(context, font, &labels, button_band, true);
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
        context.fill_rect(cancel_rect, surface.blend(&ink, 0.1));
        context.draw_rect(cancel_rect, border);
        context.draw_text_line_cow(cancel_rect, &labels[1], font, ink, HorizontalAlignment::Center);
        // Cached for hit-testing only; assigned after the last use of the borrowed style/font so
        // the read-only borrow can end instead of forcing a `WidgetStyle` clone.
        self.ok_button = ok_rect;
        self.cancel_button = cancel_rect;
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

    #[test]
    fn a_press_on_the_painted_buttons_accepts_or_rejects() {
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
        let _ = crate::widget::svg::render_to_svg(&mut dialog);
        let ok = dialog.ok_button;
        assert!(ok.width > 0 && ok.height > 0, "the accept button was painted");
        dialog.handle_event(&Event::mouse_press(
            ok.x + ok.width as i32 / 2,
            ok.y + ok.height as i32 / 2,
            1,
        ));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        let _ = crate::widget::svg::render_to_svg(&mut dialog);
        let cancel = dialog.cancel_button;
        assert!(cancel.width > 0 && cancel.height > 0, "the cancel button was painted");
        dialog.handle_event(&Event::mouse_press(
            cancel.x + cancel.width as i32 / 2,
            cancel.y + cancel.height as i32 / 2,
            1,
        ));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
        assert!(!dialog.is_visible());
    }

    #[test]
    fn font_families_are_sorted_de_duplicated_and_blank_free() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
        dialog.set_font_families(vec![
            "Beta".to_string(),
            "Alpha".to_string(),
            "beta".to_string(),
            "   ".to_string(),
        ]);

        let families = dialog.font_families();
        assert!(families.windows(2).all(|pair| pair[0].to_lowercase() <= pair[1].to_lowercase()));
        assert!(families.iter().all(|family| !family.trim().is_empty()));
        assert_eq!(families.iter().filter(|family| family.eq_ignore_ascii_case("beta")).count(), 1);
    }

    #[test]
    fn set_system_fonts_replaces_the_catalogue() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
        dialog.set_system_fonts(&[
            SystemFont {
                family: "Zed Sans".to_string(),
                style: "Regular".to_string(),
                source: None,
            },
            SystemFont { family: "Able Sans".to_string(), style: "Bold".to_string(), source: None },
        ]);

        assert_eq!(dialog.font_families().first().map(String::as_str), Some("Able Sans"));
        assert!(dialog.font_families().iter().any(|family| family == "Zed Sans"));
        assert!(!dialog.font_families().iter().any(|family| family == "Bold"));
    }

    #[test]
    fn clicking_a_family_row_applies_it() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
        dialog.set_font_families(vec!["Beta".to_string(), "Alpha".to_string()]);
        let layout = dialog.layout();
        assert!(layout.list_h >= ROW_HEIGHT as u32);

        // First row of the family column. `set_font_families` sorts, so row 0 is "Alpha".
        let x = layout.rect.x + 4 + 2;
        let y = layout.list_y + 2;
        dialog.handle_event(&Event::mouse_press(x, y, 1));

        assert_eq!(dialog.current_font().family(), "Alpha");
    }

    #[test]
    fn arrow_keys_move_within_the_active_column_and_apply_the_row() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
        dialog.set_font_families(vec!["Alpha".to_string(), "Beta".to_string()]);
        dialog.select_row(0, 0);
        assert_eq!(dialog.current_font().family(), "Alpha");

        // The default font's family ("Arial") is kept in the catalogue, sorted between
        // "Alpha" and "Beta", so one step down lands on it.
        dialog.handle_event(&Event::key_press(key_codes::DOWN, 0));
        assert_eq!(dialog.current_font().family(), "Arial");

        dialog.handle_event(&Event::key_press(key_codes::UP, 0));
        assert_eq!(dialog.current_font().family(), "Alpha");
    }

    #[test]
    fn selecting_a_size_row_updates_the_current_font() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
        let target = dialog.font_sizes().last().copied().expect("sizes are never empty");
        let index = dialog.font_sizes().len() - 1;

        dialog.select_row(2, index);

        assert_eq!(dialog.current_font().size_u32(), target);
    }
}
