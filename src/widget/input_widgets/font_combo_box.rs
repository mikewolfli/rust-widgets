// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Font combo box — a text field whose trailing indicator opens a family list.
//!
//! # Why the indicator drives the value's right inset
//!
//! The indicator is the field's trailing chrome, and the value's box must *yield* to it.
//! The shared relation writes this as `rightPadding: padding + indicator.width`, and the
//! reason is that a fixed text inset and a fixed indicator inset are two unrelated
//! derivations from the *same* edge. The value used to be written at `g.x + 4` with no bound
//! while the indicator was placed from `g.width - 14`, so a long family name — `Noto Sans
//! CJK JP` in a narrow format bar — was painted underneath the arrow.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};

use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_i64};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The field's own horizontal content inset: 4.
///
/// The value was written at `g.x + 4`, which is this number — named so the hint and the paint
/// read the same fact rather than one of them carrying a bare literal.
const FONT_FIELD_PADDING: i32 = 4;

/// Space between the indicator's leading edge and the end of the value's box: 2.
const INDICATOR_LEADING_GAP: u32 = 2;

/// The indicator's box and the box the value may occupy, derived together.
///
/// # Why the two are computed together
///
/// The `▼` and the family name beside it are one row, and the name must *yield* to the
/// indicator rather than be clipped by an inset chosen independently of it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FontFieldGeometry {
    /// The indicator's bounding box at the field's trailing edge.
    indicator_box: Rect,
    /// The rectangle the family name may occupy.
    value_box: Rect,
}

impl FontFieldGeometry {
    /// Derives both boxes from the band the control paints and `line_height`.
    fn for_band(band: Rect, line_height: u32) -> Self {
        let indicator_width = dimensions::BUTTON_ICON_SIZE.min(band.width);
        let height = line_height.min(band.height);
        let indicator_x = band.x
            + band.width.saturating_sub(indicator_width + dimensions::TEXT_FIELD_PADDING_H) as i32;
        let indicator_box = Rect::new(
            indicator_x,
            band.y + (band.height.saturating_sub(height) / 2) as i32,
            indicator_width,
            height,
        );
        // The value's box stops one gap before the indicator. `min` with the leading inset keeps
        // a squeezed field from describing an inverted rectangle: a field with no room for a name
        // draws none rather than one outside itself.
        let value_right = indicator_box.x.saturating_sub(INDICATOR_LEADING_GAP as i32);
        let value_left = (band.x + FONT_FIELD_PADDING).min(value_right);
        let value_box = Rect::new(
            value_left,
            band.y,
            value_right.saturating_sub(value_left) as u32,
            band.height,
        );
        Self { indicator_box, value_box }
    }
}
/// Font combo box widget for font selection.
///
/// Holds a list of font family names and an index into it. The index uses `-1`
/// as an explicit "nothing selected" sentinel, which is why selection is an
/// `i32` rather than a `usize`.
///
/// The widget does not enumerate installed fonts: the caller populates the list
/// with [`FontComboBox::add_font`].
///
pub struct FontComboBox {
    base: BaseWidget,
    current_font: Font,
    fonts: Vec<String>,
    current_index: i32,
    editable: bool,
    /// In-progress typed text while the box is editable.
    ///
    /// `None` means "not editing": the field shows the current font instead. It is
    /// distinct from `Some(String::new())`, which is an edit in progress whose text
    /// the user has erased — the two render differently, and collapsing them would
    /// make a cleared field look like an idle one.
    edit_buffer: Option<String>,
    max_visible_items: i32,
    expanded: bool,
    /// Emitted with the new font whenever [`FontComboBox::set_current_font`]
    /// actually changes it — including as a side effect of changing the index.
    pub current_font_changed: Signal1<Font>,
    /// Emitted with the new index (possibly `-1`) whenever
    /// [`FontComboBox::set_current_index`] actually changes it.
    pub current_index_changed: Signal1<i32>,
    /// Emitted with the index that was activated by the user. Currently only
    /// fired from the mouse-release path, with the same index that was just
    /// passed to `set_current_index`.
    pub activated: Signal1<i32>,
    /// Emitted when the user commits typed text while the combo box is editable.
    ///
    /// The payload is the committed family name. "Committed" rather than "every
    /// keystroke": a font family is only meaningful once the caller acts on it, and
    /// firing per keystroke would make a handler that loads a font reload it on
    /// every character. Enter and the popup's own selection both commit.
    pub text_edited: Signal1<String>,
    /// Emitted when the popup list is opened.
    pub popup_shown: GenericSignal,
    /// Emitted when the popup list is closed.
    pub popup_hidden: GenericSignal,
}
impl FontComboBox {
    /// The band the control actually paints: full width, one field tall, centred.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A font combo box is a text field with an indicator in it, and
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is what every field in this crate occupies. The
    /// 240x120 census cell drew a 240x120 slab, so the font chooser and the `line_edit` beside
    /// it in a format bar were different objects. The band is the single derivation the fill,
    /// the border, the indicator and the value's box all read.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's box and the value's box, derived from the band and one line height.
    fn field_geometry(&self, line_height: u32) -> FontFieldGeometry {
        FontFieldGeometry::for_band(self.field_band(), line_height)
    }

    /// Creates an empty, non-editable combo box with no selection
    /// ([`FontComboBox::current_index`] `-1`), the default font, and a maximum
    /// of 10 visible popup items (the popup starts closed).
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 200x28.
    pub fn new(geometry: Rect) -> Self {
        let default_font = Font::default();
        Self {
            base: BaseWidget::new(WidgetKind::FontComboBox, geometry, "FontComboBox"),
            current_font: default_font.clone(),
            fonts: Vec::new(),
            current_index: -1,
            editable: false,
            edit_buffer: None,
            max_visible_items: 10,
            expanded: false,
            current_font_changed: Signal1::new(),
            current_index_changed: Signal1::new(),
            activated: Signal1::new(),
            text_edited: Signal1::new(),
            popup_shown: GenericSignal::new(),
            popup_hidden: GenericSignal::new(),
        }
    }
    /// Returns the font family and size currently in effect. This is a
    /// [`Font`] value, which carries the size; note that selecting an index
    /// keeps the size but resets the bold/italic flags to `false`.
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    /// Returns the list of selectable family names, in display order.
    pub fn fonts(&self) -> &[String] {
        &self.fonts
    }
    /// Returns the selected index, or `-1` when nothing is selected.
    pub fn current_index(&self) -> i32 {
        self.current_index
    }
    /// Returns whether the combo box is marked editable. Defaults to `false`.
    ///
    /// When editable, typed characters accumulate in [`Self::edit_buffer`], Enter or
    /// [`Self::commit_edit`] emits [`Self::text_edited`], and Escape discards.
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    /// Returns how many popup entries are shown before the list is truncated.
    /// Always at least `1`; defaults to `10`.
    pub fn max_visible_items(&self) -> i32 {
        self.max_visible_items
    }
    /// Returns the number of fonts in the list; equivalent to `fonts().len()`
    /// narrowed to `i32`.
    pub fn count(&self) -> i32 {
        self.fonts.len() as i32
    }
    /// Replaces the effective font.
    ///
    /// A no-op when the value is unchanged: no signal, no redraw. This does
    /// **not** update [`FontComboBox::current_index`], so the index and the font
    /// can disagree until the index is set. Emits `current_font_changed`.
    pub fn set_current_font(&mut self, font: Font) {
        if self.current_font != font {
            self.current_font = font.clone();
            self.current_font_changed.emit(font);
            self.base.request_redraw();
        }
    }
    /// Selects an entry, clamped to `-1 ..= count()-1`.
    ///
    /// A no-op when the clamped index is unchanged. Otherwise emits
    /// `current_index_changed`, and for a non-negative index also derives the
    /// font from the selected name — **keeping the current point size but
    /// clearing the bold and italic flags** — which may in turn emit
    /// `current_font_changed`. Index `-1`, or an empty list (where the clamp is
    /// also `-1`), leaves the font untouched.
    pub fn set_current_index(&mut self, index: i32) {
        let clamped = index.clamp(-1, self.fonts.len() as i32 - 1);
        if self.current_index != clamped {
            self.current_index = clamped;
            self.current_index_changed.emit(clamped);
            if clamped >= 0 && clamped < self.fonts.len() as i32 {
                // Update current font based on selection
                if let Some(font_name) = self.fonts.get(clamped as usize) {
                    let new_font = Font::new(font_name, self.current_font.size(), false, false);
                    self.set_current_font(new_font);
                }
            }
            self.base.request_redraw();
        }
    }
    /// Sets the editable flag. See [`FontComboBox::is_editable`] — it currently
    /// has no behavioural effect beyond being reported. Requests a redraw.
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
        if !editable {
            // Turning the feature off abandons any in-progress edit, so the field
            // cannot keep showing text a non-editable box would not accept.
            self.edit_buffer = None;
        }
        self.base.request_redraw();
    }
    /// The in-progress typed text, or `None` when no edit is under way.
    ///
    /// `Some("")` is a real state: an edit the user has erased. See the field's own
    /// comment for why the two are not collapsed.
    pub fn edit_buffer(&self) -> Option<&str> {
        self.edit_buffer.as_deref()
    }
    /// Commits the typed text, emitting `text_edited`.
    ///
    /// Committing an empty edit is refused rather than emitting an empty family
    /// name: an empty string is not a font, and a caller that loads what the signal
    /// names would have nothing to load. Returns `true` when a commit happened.
    pub fn commit_edit(&mut self) -> bool {
        let Some(text) = self.edit_buffer.take() else {
            return false;
        };
        self.base.request_redraw();
        if text.trim().is_empty() {
            return false;
        }
        self.text_edited.emit(text.clone());
        // A typed family that matches a listed font also moves the selection, so the
        // typed path and the picker path cannot end up describing different fonts.
        if let Some(index) = self.fonts.iter().position(|family| family == &text) {
            self.set_current_index(index as i32);
        }
        true
    }
    /// Sets how many popup entries are shown, floored at `1` so the popup is
    /// never zero-height. Does not request a redraw (unlike the other setters),
    /// so an already-open popup may not repaint until the next redraw.
    pub fn set_max_visible_items(&mut self, max_items: i32) {
        self.max_visible_items = max_items.max(1);
    }
    /// Appends a family name to the end of the list and requests a redraw.
    ///
    /// Duplicates are not filtered: adding the same name twice yields two
    /// identical, independently selectable entries.
    pub fn add_font(&mut self, font_name: String) {
        self.fonts.push(font_name);
        self.base.request_redraw();
    }
    /// Removes the font at `index`; out-of-range indices are ignored.
    ///
    /// If the removed entry was selected, the selection is cleared to `-1`;
    /// otherwise an index after the removal point is shifted down by one so it
    /// keeps referring to the same entry. Note that only the raw index field is
    /// adjusted in that case — `current_index_changed` is **not** emitted —
    /// though the selection still points at the same font.
    pub fn remove_font(&mut self, index: i32) {
        if index >= 0 && index < self.fonts.len() as i32 {
            self.fonts.remove(index as usize);
            if self.current_index == index {
                self.set_current_index(-1);
            } else if self.current_index > index {
                self.current_index -= 1;
            }
            self.base.request_redraw();
        }
    }
    /// Empties the font list and clears the selection via
    /// [`FontComboBox::set_current_index`], so `current_index_changed` fires if
    /// an entry had been selected.
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.set_current_index(-1);
        self.base.request_redraw();
    }
    /// Opens the popup list, emits `popup_shown`, and requests a redraw.
    ///
    /// Not idempotent: calling it while already open emits again. The widget
    /// does not close the popup on outside clicks.
    pub fn show_popup(&mut self) {
        self.expanded = true;
        self.popup_shown.emit();
        self.base.request_redraw();
    }
    /// Closes the popup list, emits `popup_hidden`, and requests a redraw.
    /// As with [`FontComboBox::show_popup`], calling it while already closed
    /// still emits.
    pub fn hide_popup(&mut self) {
        self.expanded = false;
        self.popup_hidden.emit();
        self.base.request_redraw();
    }
    /// Returns the family name of the selected entry, or an empty string when
    /// nothing is selected ([`FontComboBox::current_index`] `-1`).
    pub fn current_text(&self) -> String {
        if self.current_index >= 0 && self.current_index < self.fonts.len() as i32 {
            self.fonts[self.current_index as usize].clone()
        } else {
            String::new()
        }
    }
}
impl Widget for FontComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // Derived through the same metric the band is drawn from, so the reported size and the
        // painted box cannot describe two different controls. The width was a bare `200` and the
        // height `28` while `draw` painted the whole given rectangle — 120 px tall in the
        // census cell, i.e. four times the height this reports.
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing =
            (FONT_FIELD_PADDING as u32) + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: FONT_FIELD_PADDING as u32,
        };
        let floor = Size::new(
            FONT_FIELD_PADDING as u32 + dimensions::BUTTON_ICON_SIZE + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(Size::new(0, 0), padding, floor);
        Size::new(hint.width, self.field_band().height.max(floor.height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `FontComboBox`'s property contract.
///
/// This control uses `i32` indices (the `-1` sentinel means "no font"), which is
/// why its index properties are `Int` rather than `UInt` — the split
/// `FONT_COMBO_BOX_PROPERTIES` records and the old dispatch preserved.
impl WidgetProperties for FontComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_font_family" => Ok(CapabilityValue::String(self.current_text())),
            "item_count" => Ok(CapabilityValue::Int(self.count() as i64)),
            "current_index" => Ok(CapabilityValue::Int(self.current_index() as i64)),
            "editable" => Ok(CapabilityValue::Bool(self.is_editable())),
            "max_visible_items" => Ok(CapabilityValue::Int(self.max_visible_items() as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_i64(value)? as i32);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "max_visible_items" => {
                self.set_max_visible_items(expect_i64(value)? as i32);
                Ok(())
            }
            // Derived from the font list.
            "current_font_family" | "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `FONT_COMBO_BOX_PROPERTIES`.
        property_names_of![
            "current_font_family",
            "item_count",
            "current_index",
            "editable",
            "max_visible_items",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `font_combo_box` publishes.
    ///
    /// `show_popup` and `hide_popup` are the zero-argument actions and go through the
    /// control's own methods, so the `popup_shown` / `popup_hidden` signals are emitted
    /// exactly as they are for a mouse-driven open. They are idempotent in the sense that
    /// matters here: opening an open popup still performed the command, so it answers
    /// `Ok(())` rather than reporting a failure the caller did not cause.
    ///
    /// `set_current_index`, `set_editable` and `set_max_visible_items` assign state and
    /// need a payload, so they are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show_popup" => {
                self.show_popup();
                Ok(())
            }
            "hide_popup" => {
                self.hide_popup();
                Ok(())
            }
            "set_current_index" | "set_editable" | "set_max_visible_items" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

use crate::render::RenderContext;
use crate::widget::Draw;

impl EventHandler for FontComboBox {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            self.base.handle_event(event);
            return;
        }
        // The gesture is resolved before the base records the event: the base clears
        // `pressed` on the release, so the commit guard must read the value the press set.
        match event {
            Event::KeyPress { key, modifiers: _ } if self.editable => {
                // Mirrors `EditableComboBox`'s key convention: 8 is backspace, 13 is
                // Enter, 27 is Escape, and any other printable code is a character.
                match *key {
                    8 => {
                        let mut buffer = self
                            .edit_buffer
                            .take()
                            .unwrap_or_else(|| self.current_font().family().to_string());
                        buffer.pop();
                        self.edit_buffer = Some(buffer);
                        self.base.request_redraw();
                    }
                    13 => {
                        self.commit_edit();
                    }
                    27 => {
                        // Escape discards the edit rather than committing it, which is
                        // the only difference between the two exit paths and therefore
                        // the whole reason both exist.
                        self.edit_buffer = None;
                        self.base.request_redraw();
                    }
                    _ => {
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                let mut buffer = self.edit_buffer.take().unwrap_or_default();
                                buffer.push(ch);
                                self.edit_buffer = Some(buffer);
                                self.base.request_redraw();
                            }
                        }
                    }
                }
            }
            Event::MousePress { pos, button } if button == &1 => {
                // Arm only for a press on the control; a press outside must not leave the
                // cycle-on-release latch armed.
                self.base.set_pressed(self.geometry().contains_point(*pos));
                if self.base.is_pressed() {
                    // Show the dropdown list
                    self.show_popup();
                    self.base.clicked.emit();
                }
            }
            Event::MouseRelease { pos, button }
                if button == &1
                    // Cycle to next font on release, but only for a completed press on this
                    // control. The position was previously discarded (`pos: _`) and no press was
                    // required, so **any** left release reached here and cycled the font — a
                    // release the host routed from a drag that began on another control, or one
                    // with no preceding press at all.
                    && self.base.is_pressed() =>
            {
                self.base.set_pressed(false);
                // A release off the control cancels, matching `Button`/`Switch`.
                if self.geometry().contains_point(*pos) && !self.fonts.is_empty() {
                    let next = (self.current_index + 1) % self.fonts.len() as i32;
                    self.set_current_index(next);
                    self.activated.emit(next);
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.base.set_pressed(false);
            }
            // Losing focus abandons a held press, so the latch cannot survive a window switch.
            Event::FocusLost => {
                self.base.set_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
        // Record the primitive facts after the gesture.
        self.base.handle_event(event);
    }
}

impl Draw for FontComboBox {
    fn draw(&mut self, ctx: &mut RenderContext) {
        // ── The field actually painted ──
        //
        // `geometry()` is the area the control was *given*; the **field** of a font combo box is
        // a text field, and a field is a fixed-height band. The expanded popup below it is a
        // menu rather than part of the field, so it still hangs from the control's own bottom
        // edge, which is a separate derivation and is left as one.
        let g = self.field_band();
        if g.width == 0 || g.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the field, its arrow, its text and its popup
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        // `font_combo_box` classifies as `WidgetRole::Input` (the role table lists it as
        // `fontcombobox`), whose resolved background is the field interior — lighter on a light
        // theme, darker on a dark one — and whose ink is the theme's foreground.
        let theme = crate::style::resolved_theme_style("font_combo_box");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, primary, secondary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.secondary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The field: a caller's own colour wins, then the theme's resolved background. The window
        // fill is filtered out because a control can carry it after a theme application, and
        // painting it would make the field indistinguishable from the window. A caller's own
        // colour still wins.
        let field = style
            .background_color
            .filter(|resolved| *resolved != window_fill)
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .filter(|resolved| *resolved != window_fill)
            .unwrap_or_else(|| window_fill.blend(&ink, 0.10));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| field.blend(&secondary, 0.45));
        // The drop-down arrow is an affordance, de-emphasised from the field's own ink so it is
        // legible on either appearance rather than being a fixed grey.
        let arrow_color = ink.blend(&field, 0.35);

        ctx.fill_rect(g, field);
        ctx.draw_rect(g, border);
        // The indicator's box and the value's box come from **one** derivation, so the family
        // name yields to the indicator instead of being written at an unconstrained offset.
        let value_font = Font::default_ui();
        let field_line = ctx.text_line(g, &value_font);
        let geometry = self.field_geometry(field_line.height);
        // Draw drop-down arrow indicator inside its own derived box, vertically centred on the
        // value's own line box rather than on the field's raw midpoint.
        let arrow_indicator_line = ctx.text_line(geometry.indicator_box, &value_font);
        let arrow_center_y = arrow_indicator_line.y + arrow_indicator_line.height as i32 / 2;
        let arrow_center_x = geometry.indicator_box.x + geometry.indicator_box.width as i32 / 2;
        ctx.draw_line(
            Point::new(arrow_center_x - 3, arrow_center_y - 2),
            Point::new(arrow_center_x, arrow_center_y + 2),
            arrow_color,
        );
        ctx.draw_line(
            Point::new(arrow_center_x, arrow_center_y + 2),
            Point::new(arrow_center_x + 3, arrow_center_y - 2),
            arrow_color,
        );
        let font_name = self.current_font().family().to_string();
        // The field's own line box. A glyph origin is the box's top-left edge, so the old
        // `g.y + g.height / 2 + 5` put that edge on the field's middle line and drew the
        // selected family half a line low.
        let value_line = ctx.text_line(geometry.value_box, &value_font);
        // Fitted, so a long family name is elided rather than drawn under the indicator — the box
        // it is given is the one the indicator left.
        ctx.draw_text_fitted(
            Rect::new(
                geometry.value_box.x,
                value_line.y,
                geometry.value_box.width,
                value_line.height,
            ),
            &font_name,
            &value_font,
            ink,
            HorizontalAlignment::Left,
        );
        // Draw popup list when expanded
        if self.expanded && !self.fonts.is_empty() {
            let popup_item_height = 22i32;
            let visible_count = self.fonts.len().min(self.max_visible_items as usize);
            let popup_height = (visible_count as i32) * popup_item_height;
            let popup_rect = Rect::new(g.x, g.y + g.height as i32, g.width, popup_height as u32);
            // The popup is a raised step out of the field, so it reads as a separate surface on
            // either appearance.
            let popup_fill = field.blend(&ink, 0.06);
            ctx.fill_rect(popup_rect, popup_fill);
            ctx.draw_rect(popup_rect, border);
            // The highlighted item is the control's selection, so it carries the theme's primary
            // rather than a fixed blue.
            let highlight = primary.blend(&field, 0.15);
            let display_font = Font::default_ui();
            for i in 0..visible_count {
                let item_y = popup_rect.y + (i as i32) * popup_item_height;
                let item_rect =
                    Rect::new(popup_rect.x, item_y, popup_rect.width, popup_item_height as u32);
                // One line box per row, shared by the highlighted and plain arms so the two
                // cannot drift by half a line while the row also changes colour.
                let item_line = ctx.text_line(item_rect, &display_font);
                if i as i32 == self.current_index {
                    ctx.fill_rect(item_rect, highlight);
                    if let Some(name) = self.fonts.get(i) {
                        // The label contrasts with the highlight it sits on, so it stays legible
                        // whatever the theme's primary is.
                        ctx.draw_text(
                            Point::new(g.x + 4, item_line.y),
                            name,
                            &display_font,
                            highlight.contrast_color(),
                            HorizontalAlignment::Left,
                        );
                    }
                } else if let Some(name) = self.fonts.get(i) {
                    ctx.draw_text(
                        Point::new(g.x + 4, item_line.y),
                        name,
                        &display_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn fontcombobox_creation_defaults() {
        let fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.fonts().is_empty());
        assert_eq!(fcb.count(), 0);
        assert_eq!(fcb.current_index(), -1);
        assert!(fcb.current_text().is_empty());
        assert!(!fcb.is_editable());
        assert_eq!(fcb.max_visible_items(), 10);
    }

    #[test]
    fn fontcombobox_add_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        assert_eq!(fcb.count(), 2);
        assert_eq!(fcb.fonts()[0], "Arial");
        assert_eq!(fcb.fonts()[1], "Helvetica");
    }

    #[test]
    fn fontcombobox_remove_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.remove_font(0);
        assert_eq!(fcb.count(), 1);
        assert_eq!(fcb.fonts()[0], "Helvetica");
    }

    #[test]
    fn fontcombobox_clear() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.set_current_index(0);
        fcb.clear();
        assert_eq!(fcb.count(), 0);
        assert_eq!(fcb.current_index(), -1);
    }

    #[test]
    fn fontcombobox_set_current_index() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.set_current_index(0);
        assert_eq!(fcb.current_index(), 0);
        assert_eq!(fcb.current_text(), "Arial");
    }

    #[test]
    fn fontcombobox_set_current_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        let font = Font::new("Arial", 12.0, false, false);
        fcb.set_current_font(font.clone());
        assert_eq!(fcb.current_font().family(), "Arial");
    }

    #[test]
    fn fontcombobox_editable() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(!fcb.is_editable());
        fcb.set_editable(true);
        assert!(fcb.is_editable());
        fcb.set_editable(false);
        assert!(!fcb.is_editable());
    }

    #[test]
    fn fontcombobox_max_visible_items() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert_eq!(fcb.max_visible_items(), 10);
        fcb.set_max_visible_items(5);
        assert_eq!(fcb.max_visible_items(), 5);
        fcb.set_max_visible_items(0); // floors at 1
        assert_eq!(fcb.max_visible_items(), 1);
    }

    #[test]
    fn fontcombobox_show_hide_popup() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.show_popup();
        fcb.hide_popup();
        // Should not panic
    }

    #[test]
    fn fontcombobox_geometry_delegation() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.set_geometry(Rect::new(10, 10, 300, 30));
        assert_eq!(fcb.geometry(), Rect::new(10, 10, 300, 30));
    }

    #[test]
    fn fontcombobox_visibility() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.is_visible());
        fcb.hide();
        assert!(!fcb.is_visible());
        fcb.show();
        assert!(fcb.is_visible());
    }

    #[test]
    fn fontcombobox_enabled() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.is_enabled());
        fcb.set_enabled(false);
        assert!(!fcb.is_enabled());
        fcb.set_enabled(true);
        assert!(fcb.is_enabled());
    }

    #[test]
    fn fontcombobox_id_kind() {
        let fcb_a = FontComboBox::new(Rect::new(0, 0, 100, 24));
        let fcb_b = FontComboBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(fcb_a.id(), fcb_b.id());
        assert_eq!(fcb_a.kind(), WidgetKind::FontComboBox);
        assert_eq!(fcb_b.kind(), WidgetKind::FontComboBox);
    }

    #[test]
    fn fontcombobox_signal_accessors() {
        let fcb = FontComboBox::new(Rect::new(0, 0, 100, 24));
        let _ = &fcb.current_font_changed;
        let _ = &fcb.current_index_changed;
        let _ = &fcb.activated;
        let _ = &fcb.text_edited;
        let _ = &fcb.popup_shown;
        let _ = &fcb.popup_hidden;
    }

    #[test]
    fn font_combo_box_draw_produces_output() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        let svg = crate::widget::svg::render_to_svg(&mut fcb);
        assert!(svg.starts_with("<svg"));
    }
    /// Only a completed press on the control cycles the font.
    ///
    /// `handle_event` cycled on `Event::MouseRelease` with the position discarded (`pos: _`) and
    /// no press-arm, so **any** left release reached the arm: a release the host routed from a drag
    /// that began on another control, or one with no preceding press at all. This is the same
    /// defect class as `Switch`; `Button`, `CheckBox` and `Switch` all gate on a press-provided
    /// flag.
    #[test]
    fn font_combo_box_cycles_only_on_a_completed_press() {
        let inside = Point::new(50, 15);
        let outside = Point::new(9000, 9000);
        let build = || {
            let mut c = FontComboBox::new(Rect::new(0, 0, 200, 30));
            c.add_font("Arial".to_string());
            c.add_font("Helvetica".to_string());
            c.add_font("Menlo".to_string());
            c
        };
        let run = |events: &[Event]| {
            let mut c = build();
            let before = c.current_index();
            for event in events {
                c.handle_event(event);
            }
            (before, c.current_index())
        };

        // A release with no press must not cycle, wherever it lands.
        assert_eq!(run(&[Event::MouseRelease { pos: inside, button: 1 }]), (-1, -1));
        assert_eq!(run(&[Event::MouseRelease { pos: outside, button: 1 }]), (-1, -1));

        // A press outside does not arm the latch.
        assert_eq!(
            run(&[
                Event::MousePress { pos: outside, button: 1 },
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            (-1, -1),
            "a drag that began outside must not commit"
        );

        // Pressing inside and releasing outside cancels.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1 },
                Event::MouseRelease { pos: outside, button: 1 },
            ]),
            (-1, -1)
        );

        // A completed activation advances exactly one step.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1 },
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            (-1, 0)
        );

        // Focus loss abandons a held press.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1 },
                Event::FocusLost,
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            (-1, -1)
        );
    }

    /// A right-button release is ignored, armed or not.
    #[test]
    fn font_combo_box_ignores_non_primary_releases() {
        let inside = Point::new(50, 15);
        let mut c = FontComboBox::new(Rect::new(0, 0, 200, 30));
        c.add_font("Arial".to_string());
        c.add_font("Helvetica".to_string());
        c.handle_event(&Event::MouseRelease { pos: inside, button: 3 });
        assert_eq!(c.current_index(), -1);
    }

    /// The family name's box ends where the indicator's box begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the name *yields*
    /// to the indicator. The name used to be written at `g.x + 4` with no upper bound while
    /// the indicator sat at `g.x + g.width - 14`, so a long family such as
    /// `Noto Sans CJK JP` in a narrow format bar was drawn underneath the arrow.
    #[test]
    fn the_family_name_box_ends_where_the_indicator_begins() {
        let trailing =
            FONT_FIELD_PADDING as u32 + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        for width in [0u32, 20, 64, 240, 400] {
            let c = FontComboBox::new(Rect::new(0, 0, width, 120));
            let geometry = c.field_geometry(14);
            let band = c.field_band();
            assert_eq!(
                geometry.value_box.x
                    + geometry.value_box.width as i32
                    + INDICATOR_LEADING_GAP as i32,
                geometry.indicator_box.x,
                "the name must stop one gap short of the indicator at width {width}"
            );
            assert!(
                geometry.indicator_box.x + geometry.indicator_box.width as i32
                    <= band.x + band.width as i32,
                "the indicator must stay inside the band at width {width}"
            );
            if width < trailing {
                assert_eq!(
                    geometry.value_box.width, 0,
                    "a band too narrow for its own chrome holds no name at width {width}"
                );
            }
        }
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let c = FontComboBox::new(Rect::new(0, 0, 240, 120));
        let band = c.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(c.size_hint().height, band.height);
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }
}
