// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dropdown widget — standalone dropdown list selector (BLUE13 R2.4).
//!
//! A button-like label showing the current selection. When clicked, it
//! expands to show a scrollable list of options. Selecting an item emits
//! a `changed` signal and collapses the list.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Height of each item row in the expanded dropdown list (pixels).
const ITEM_HEIGHT: u32 = 20;
/// Padding between the button text and the widget edges.
const PADDING: i32 = 4;

/// Width of the indicator cell at the field's trailing edge.
///
/// It was the literal `12` at the draw site while the value was written at `geo.x + PADDING`
/// with no bound, so the two were independent derivations from the same field width and a
/// long selection ran under the arrow. Naming it is what lets the value's box be derived
/// from it instead of from a second numeral.
const INDICATOR_WIDTH: i32 = 12;

/// Space between the indicator's leading edge and the end of the value's box: 2.
///
/// Half of [`PADDING`], written as a literal because `Ord::max` is not available in a
/// `const` expression on this toolchain; the relation is documented rather than computed so
/// the constant table stays where a reader can find it.
const INDICATOR_LEADING_GAP: i32 = 2;

/// The indicator's box and the box the value may occupy, derived together.
///
/// # Why the two are computed together
///
/// A `▼` and the label beside it are one row: the label must *yield* to the indicator rather
/// than be clipped by an inset that was chosen independently of it. The shared relation reads
/// `rightPadding: padding + indicator.width`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FieldGeometry {
    /// The indicator cell at the field's trailing edge.
    indicator_box: Rect,
    /// The rectangle the selected label may occupy.
    label_box: Rect,
}

impl FieldGeometry {
    /// Derives both boxes from the band the control paints and `line_height`.
    fn for_band(band: Rect, line_height: u32) -> Self {
        let indicator_width = INDICATOR_WIDTH.min(band.width as i32);
        let height = line_height.min(band.height);
        let indicator_x = band.x + band.width as i32 - PADDING - indicator_width;
        // Keep the cell inside the band when the field is narrower than its own padding: a
        // field with no room for its chrome shows no indicator rather than one outside itself.
        let indicator_x =
            indicator_x.min(band.x + band.width.saturating_sub(indicator_width as u32) as i32);
        let indicator_box = Rect::new(
            indicator_x.max(band.x),
            band.y + (band.height.saturating_sub(height) / 2) as i32,
            indicator_width.max(0) as u32,
            height,
        );
        let label_right = indicator_box.x.saturating_sub(INDICATOR_LEADING_GAP);
        let label_left = (band.x + PADDING).min(label_right);
        let label_box = Rect::new(
            label_left,
            band.y,
            label_right.saturating_sub(label_left) as u32,
            band.height,
        );
        Self { indicator_box, label_box }
    }
}

/// Dropdown widget for selecting from a list of options.
///
/// # Behaviour
/// - Displays the currently selected item (or a placeholder) inside a button-like
///   rectangle with a ▼ arrow.
/// - On click the list expands below the button; clicking an item selects it and
///   collapses the list.
/// - Losing focus also collapses the list.
pub struct Dropdown {
    base: BaseWidget,
    /// List of option strings.
    items: Vec<String>,
    /// Currently selected index. Defaults to `0` when items is non-empty.
    selected_index: usize,
    /// Whether the dropdown list is expanded.
    expanded: bool,
    /// Signal emitted when the selection changes.
    pub changed: GenericSignal,
}

impl Dropdown {
    /// Creates a new `Dropdown` with the given items and geometry.
    ///
    /// The first item is automatically selected when `items` is non-empty.
    /// The widget's height should accommodate the collapsed button area.
    pub fn new(items: Vec<String>, rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dropdown, rect, "Dropdown"),
            selected_index: 0,
            expanded: false,
            changed: GenericSignal::new(),
            items,
        }
    }

    /// Returns the full list of option strings.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Replaces all option strings and resets the selection to the first item
    /// (or index 0 if the list is empty).
    pub fn set_items(&mut self, new_items: Vec<String>) {
        self.items = new_items;
        self.selected_index = 0;
        self.changed.emit();
        self.base.request_redraw();
    }

    /// Returns the currently-selected index (may be out of range if the list
    /// was shrunk after selection).
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Sets the selected index, clamping it to the list bounds.
    ///
    /// Emits the `changed` signal whenever the index actually changes or is
    /// clamped to a different value.
    pub fn set_selected_index(&mut self, index: usize) {
        let clamped =
            if self.items.is_empty() { 0 } else { index.min(self.items.len().saturating_sub(1)) };
        if self.selected_index != clamped {
            self.selected_index = clamped;
            self.changed.emit();
            self.base.request_redraw();
        }
    }

    /// Returns the text of the currently-selected item, or `None` if the list
    /// is empty.
    pub fn selected_text(&self) -> Option<&str> {
        if self.selected_index < self.items.len() {
            Some(self.items[self.selected_index].as_str())
        } else {
            None
        }
    }

    /// Returns `true` when the dropdown list is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands or collapses the dropdown list.
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
        self.base.request_redraw();
    }

    /// Toggles the expanded / collapsed state.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        self.base.request_redraw();
    }

    // ─── helpers ────────────────────────────────────────────────────────

    /// The band the collapsed field actually occupies: full width, one field tall, centred.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A dropdown is a text field with an indicator in it, and
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is what every field in this crate occupies. The
    /// 240x120 census cell drew a 240x120 slab, so the collapsed field and the `line_edit`
    /// beside it were different objects. The band is the single derivation the fill, the
    /// border, the label and the indicator all read — and the expanded list still hangs from
    /// the control's own bottom edge, because the list is not part of the field.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's cell and the label's box, derived from the band and one line height.
    fn field_geometry(&self, line_height: u32) -> FieldGeometry {
        FieldGeometry::for_band(self.field_band(), line_height)
    }

    /// Compute the bounding `Rect` of the n-th list item in screen coordinates.
    /// Only meaningful when `expanded == true`.
    fn item_rect(&self, index: usize) -> Rect {
        let geo = self.geometry();
        let item_y = geo.y + geo.height as i32 + (index as u32 * ITEM_HEIGHT) as i32;
        Rect::new(geo.x, item_y, geo.width, ITEM_HEIGHT)
    }

    /// Determine which list item (if any) a point falls on.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }
        (0..self.items.len()).position(|i| self.item_rect(i).contains_point(pos))
    }
}

impl Widget for Dropdown {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // Derived through the same metric the band is drawn from, so the reported size and the
        // painted box cannot describe two different controls. The width was the bare expression
        // `max_text_w * 8 + 30` and the height a flat `24`, while `draw` painted the whole given
        // rectangle — 120 px tall in the census cell.
        let widest = self.items.iter().map(|s| s.len() as u32).max().unwrap_or(6) * 8;
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing = (PADDING.max(0) as u32)
            + INDICATOR_WIDTH.max(0) as u32
            + INDICATOR_LEADING_GAP.max(0) as u32;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: PADDING.max(0) as u32,
        };
        let floor = Size::new(
            PADDING.max(0) as u32 + INDICATOR_WIDTH.max(0) as u32 + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(Size::new(widest, 0), padding, floor);
        // The height is the band's, not a second numeral: this is what makes "the reported size
        // and the drawn box agree" checkable rather than merely intended. A `select` with no
        // items is still a field, so the height does not depend on the list.
        Size::new(hint.width, self.field_band().height.max(floor.height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Dropdown`'s property contract.
///
/// `text` is the selected item's label and follows the old arm's shape: an empty
/// string (not `Null`) when nothing is selected, so callers do not need a
/// two-way `Option` check for a display-only value.
impl WidgetProperties for Dropdown {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => match self.selected_text() {
                Some(text) => Ok(CapabilityValue::String(text.to_string())),
                None => Ok(CapabilityValue::String(String::new())),
            },
            "selected_index" => Ok(CapabilityValue::UInt(self.selected_index() as u64)),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected_index(expect_usize(value)?);
                Ok(())
            }
            "expanded" => {
                self.set_expanded(expect_bool(value)?);
                Ok(())
            }
            // `text` follows the selection and `item_count` the item list.
            "text" | "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `DROPDOWN_PROPERTIES`.
        property_names_of!["text", "selected_index", "item_count", "expanded", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `dropdown` publishes.
    ///
    /// `toggle` flips the expanded state through the control's own `toggle()`, so the
    /// flag and the redraw request stay in one place. `set_items`,
    /// `set_selected_index` and `set_expanded` assign state and need a payload, so they
    /// are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_items" | "set_selected_index" | "set_expanded" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Dropdown {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if self.expanded {
                    // Clicked on one of the list items?
                    if let Some(idx) = self.hit_test_item(*pos) {
                        self.set_selected_index(idx);
                        self.expanded = false;
                        return;
                    }

                    // Clicked back on the button header — collapse
                    let geo = self.geometry();
                    if geo.contains_point(*pos) {
                        self.expanded = false;
                        return;
                    }

                    // Clicked outside the entire widget while expanded – collapse
                    self.expanded = false;
                } else {
                    // Collapsed: toggle expand
                    let geo = self.geometry();
                    if geo.contains_point(*pos) {
                        self.expanded = true;
                    }
                }
            }

            Event::FocusLost => {
                self.expanded = false;
            }

            Event::KeyPress { key, modifiers: _ } => {
                if !self.expanded {
                    // Up / Down arrow keys open the list
                    if (*key == 40 || *key == 38) && !self.items.is_empty() {
                        self.expanded = true;
                    }
                    return;
                }

                match *key {
                    38 if self.selected_index > 0 => {
                        // Up arrow — previous item
                        self.set_selected_index(self.selected_index - 1);
                    }
                    40 if self.selected_index + 1 < self.items.len() => {
                        // Down arrow — next item
                        self.set_selected_index(self.selected_index + 1);
                    }
                    13 => {
                        // Enter — commit selection, collapse
                        self.expanded = false;
                    }
                    27 => {
                        // Escape — cancel, collapse
                        self.expanded = false;
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }
}

impl Draw for Dropdown {
    fn draw(&mut self, context: &mut RenderContext) {
        // ── The collapsed field actually painted ──
        //
        // `geometry()` is the area the control was *given*; the **button** part of a dropdown
        // is a text field and a field is a fixed-height band. The expanded list below it is a
        // popup rather than part of the field, so it still hangs from the control's own bottom
        // edge, which is a separate derivation and is left as one.
        let geo = self.field_band();
        if geo.width == 0 || geo.height == 0 {
            return;
        }

        // ── style-derived colours ───────────────────────────────────────
        //
        // # The five literals that used to be here
        //
        // `bg`/`border`/`text_color` already read `style`, but the other five were constants:
        // `placeholder_color`, `highlight_bg`, `highlight_text`, `list_border`, and the list row's
        // own `rgb(248,248,248)`. So the *collapsed field* followed the appearance while everything
        // **inside the popup** did not — the list kept a pale blue highlight and a near-white row on
        // a dark palette, which is exactly the "drawn but unreachable by the theme" shape the census
        // reports. They now read roles:
        //
        //   placeholder  -> the theme's weak ink, so it recedes on both appearances
        //   highlight    -> the accent pair (an emphasised row is a *selection*, not a lighter grey)
        //   list border  -> `outline_variant`, the weak separator, so it is visibly weaker than the
        //                   field's own focus ring
        //   list row     -> `surface_container`, one step above the page, which is what a popup *is*
        //
        // The guard is released before drawing: `theme_manager()` is a non-reentrant mutex and the
        // accessors below take the same one — the rule `slider.rs` documents.
        let (weak_ink, accent, on_accent, separator, popup_surface) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    let accent = active.colors.primary;
                    (
                        Some(active.colors.secondary),
                        Some(accent),
                        Some(accent.contrast_color()),
                        Some(active.colors.outline_variant),
                        Some(active.colors.surface_container),
                    )
                }
                None => (None, None, None, None, None),
            }
        };
        let bg = self.style().background_color.unwrap_or(Color::rgb(255, 255, 255));
        let border = self.style().border_color.unwrap_or(Color::rgb(180, 180, 180));
        let text_color = self.style().text_color.unwrap_or(Color::rgb(0, 0, 0));
        let placeholder_color = weak_ink.unwrap_or(Color::rgb(160, 160, 160));
        let highlight_bg = accent.unwrap_or(Color::rgb(200, 220, 255));
        // The highlighted row's ink is the accent's contrast colour rather than the field's ink: a
        // selected row is filled *with the accent*, so the ink has to be legible on that, not on the
        // page. Reading `text_color` here was legible only while the fill happened to be pale.
        let highlight_text = on_accent.unwrap_or(Color::rgb(0, 0, 0));
        let list_border = separator.unwrap_or(Color::rgb(150, 150, 150));
        let list_row = popup_surface.unwrap_or(Color::rgb(248, 248, 248));

        // ── Collapsed / button area ─────────────────────────────────────
        // Background
        context.fill_rect(geo, bg);
        // Border
        context.draw_rect(geo, border);

        // The indicator's cell and the label's box come from one derivation, so the selection
        // yields to the indicator instead of being written at an unconstrained offset.
        // Both labels on the collapsed row sit on the field's own line box: a glyph origin is
        // the box's top-left edge, so the old `geo.y + geo.height / 2` put that edge on the
        // field's middle line and drew the value and the indicator half a line low.
        let field_line = context.text_line(geo, &Font::default());
        let geometry = self.field_geometry(field_line.height);
        let indicator = "▼";
        let label_line = context.text_line(geometry.label_box, &Font::default());
        let indicator_line = context.text_line(geometry.indicator_box, &Font::default());

        let (label, color) = match self.selected_text() {
            Some(text) => (text, text_color),
            None => ("(Select)", placeholder_color),
        };
        // Fitted, so a selection longer than the field is elided rather than drawn under the
        // indicator — the box it is given is the one the indicator left.
        context.draw_text_fitted(
            Rect::new(
                geometry.label_box.x,
                label_line.y,
                geometry.label_box.width,
                label_line.height,
            ),
            label,
            &Font::default(),
            color,
            HorizontalAlignment::Left,
        );
        context.draw_text_fitted(
            Rect::new(
                geometry.indicator_box.x,
                indicator_line.y,
                geometry.indicator_box.width,
                indicator_line.height,
            ),
            indicator,
            &Font::default(),
            color,
            HorizontalAlignment::Left,
        );

        // ── Expanded list ───────────────────────────────────────────────
        if !self.expanded || self.items.is_empty() {
            return;
        }

        for i in 0..self.items.len() {
            let item_geo = self.item_rect(i);

            // Background
            let is_selected = i == self.selected_index;
            if is_selected {
                context.fill_rect(item_geo, highlight_bg);
            } else {
                context.fill_rect(item_geo, list_row);
            }

            // Border (bottom line)
            context.draw_rect(item_geo, list_border);

            // Item text
            let item_color = if is_selected { highlight_text } else { text_color };
            let item_x = item_geo.x + PADDING;
            // The row's own line box, for the same reason as the collapsed field above: a
            // halved height here places the glyph box's top edge on the row's middle line.
            let item_line = context.text_line(item_geo, &Font::default());
            context.draw_text(
                Point::new(item_x, item_line.y),
                &self.items[i],
                &Font::default(),
                item_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    fn make_dropdown() -> Dropdown {
        Dropdown::new(
            vec!["Option A".to_string(), "Option B".to_string(), "Option C".to_string()],
            Rect::new(10, 10, 160, 24),
        )
    }

    #[test]
    fn dropdown_creation_defaults() {
        let dd = make_dropdown();
        assert_eq!(dd.items().len(), 3);
        assert_eq!(dd.items()[0], "Option A");
        assert_eq!(dd.items()[1], "Option B");
        assert_eq!(dd.items()[2], "Option C");
        assert_eq!(dd.selected_index(), 0);
        assert_eq!(dd.selected_text(), Some("Option A"));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_set_items() {
        let mut dd = make_dropdown();
        dd.set_items(vec!["X".to_string(), "Y".to_string()]);
        assert_eq!(dd.items().len(), 2);
        assert_eq!(dd.items()[0], "X");
        assert_eq!(dd.items()[1], "Y");
        assert_eq!(dd.selected_index(), 0);
    }

    #[test]
    fn dropdown_set_items_empty_resets_selection() {
        let mut dd = make_dropdown();
        dd.set_selected_index(2);
        dd.set_items(Vec::new());
        assert!(dd.items().is_empty());
        assert_eq!(dd.selected_index(), 0);
        assert!(dd.selected_text().is_none());
    }

    #[test]
    fn dropdown_select_item() {
        let mut dd = make_dropdown();
        dd.set_selected_index(1);
        assert_eq!(dd.selected_index(), 1);
        assert_eq!(dd.selected_text(), Some("Option B"));
    }

    #[test]
    fn dropdown_select_item_clamps_out_of_range() {
        let mut dd = make_dropdown();
        dd.set_selected_index(100);
        assert_eq!(dd.selected_index(), 2);
        assert_eq!(dd.selected_text(), Some("Option C"));
    }

    #[test]
    fn dropdown_toggle_expand() {
        let mut dd = make_dropdown();
        assert!(!dd.is_expanded());
        dd.toggle();
        assert!(dd.is_expanded());
        dd.toggle();
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_set_expanded() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        assert!(dd.is_expanded());
        dd.set_expanded(false);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_focus_lost_collapses() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        dd.handle_event(&Event::FocusLost);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_press_toggles() {
        let mut dd = make_dropdown();
        // Click inside the widget geometry to expand
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(dd.is_expanded());

        // Click inside the widget geometry again to collapse
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_press_outside_collapses_when_expanded() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        // Click far outside
        dd.handle_event(&Event::mouse_press(999, 999, 1));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_select_item() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);

        // The first item is at y = 10 + 24 + 0*20 = 34
        dd.handle_event(&Event::mouse_press(15, 35, 1));
        assert_eq!(dd.selected_index(), 0);
        assert!(!dd.is_expanded()); // selection collapses

        // Expand again and select the 3rd item (index 2)
        dd.set_expanded(true);
        dd.handle_event(&Event::mouse_press(15, 35 + 2 * 20, 1));
        assert_eq!(dd.selected_index(), 2);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_keyboard_navigation() {
        let mut dd = make_dropdown();
        // Down arrow opens the expanded list
        dd.handle_event(&Event::key_press(40, 0));
        assert!(dd.is_expanded());

        // Down arrow moves to next item
        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.selected_index(), 1);

        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.selected_index(), 2);

        // Already at last — should stay
        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.selected_index(), 2);

        // Up arrow
        dd.handle_event(&Event::key_press(38, 0));
        assert_eq!(dd.selected_index(), 1);

        dd.handle_event(&Event::key_press(38, 0));
        assert_eq!(dd.selected_index(), 0);

        // Already at first — should stay
        dd.handle_event(&Event::key_press(38, 0));
        assert_eq!(dd.selected_index(), 0);

        // Enter commits and collapses
        dd.handle_event(&Event::key_press(13, 0));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_escape_collapses() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        dd.handle_event(&Event::key_press(27, 0));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_disabled_ignores_events() {
        let mut dd = make_dropdown();
        dd.set_expanded(false);
        dd.set_enabled(false);
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(!dd.is_expanded(), "disabled widget should not expand");
    }

    #[test]
    fn dropdown_draw_does_not_panic() {
        // Minimal smoke test — we only verify that the draw method runs
        // without panicking. A real render backend would be needed for
        // meaningful pixel tests.
        let mut dd = make_dropdown();

        // Create a minimal software render backend for testing
        use crate::render::PaintBackend;
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(200, 200), 1.0);
        backend.begin_frame(crate::core::Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        // Collapsed draw
        dd.draw(&mut ctx);

        // Expanded draw
        dd.set_expanded(true);
        dd.draw(&mut ctx);
    }

    #[test]
    fn dropdown_geometry_delegation() {
        let mut dd = make_dropdown();
        dd.set_geometry(Rect::new(0, 0, 200, 30));
        assert_eq!(dd.geometry(), Rect::new(0, 0, 200, 30));
    }

    #[test]
    fn dropdown_visibility() {
        let mut dd = make_dropdown();
        assert!(dd.is_visible());
        dd.hide();
        assert!(!dd.is_visible());
        dd.show();
        assert!(dd.is_visible());
    }

    #[test]
    fn dropdown_enabled() {
        let mut dd = make_dropdown();
        assert!(dd.is_enabled());
        dd.set_enabled(false);
        assert!(!dd.is_enabled());
        dd.set_enabled(true);
        assert!(dd.is_enabled());
    }

    #[test]
    fn dropdown_id_kind() {
        let dd_a = make_dropdown();
        let dd_b = make_dropdown();
        assert_ne!(dd_a.id(), dd_b.id());
        assert_eq!(dd_a.kind(), WidgetKind::Dropdown);
        assert_eq!(dd_b.kind(), WidgetKind::Dropdown);
    }

    #[test]
    fn dropdown_signal_accessors() {
        let dd = make_dropdown();
        let _ = &dd.changed;
        let _ = dd.changed_signal();
        let _ = dd.clicked_signal();
    }

    #[test]
    fn dropdown_item_rect_computation() {
        let dd = make_dropdown(); // Rect(10, 10, 160, 24)
        let r = dd.item_rect(0);
        assert_eq!(r, Rect::new(10, 34, 160, 20));
        let r = dd.item_rect(2);
        assert_eq!(r, Rect::new(10, 74, 160, 20));
    }

    #[test]
    fn dropdown_selected_text_none_when_empty() {
        let dd = Dropdown::new(Vec::new(), Rect::new(0, 0, 100, 24));
        assert!(dd.selected_text().is_none());
    }

    /// The label's box ends where the indicator's cell begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the selection
    /// *yields* to the indicator. The label used to be written at `geo.x + PADDING` with no
    /// upper bound while the indicator sat at `geo.x + geo.width - PADDING - 12`, so a long
    /// selection ran underneath the arrow.
    #[test]
    fn the_label_box_ends_where_the_indicator_begins() {
        let trailing = PADDING.max(0) as u32
            + INDICATOR_WIDTH.max(0) as u32
            + INDICATOR_LEADING_GAP.max(0) as u32;
        for width in [0u32, 20, 64, 240, 400] {
            let dd = Dropdown::new(vec!["Sample".to_string()], Rect::new(0, 0, width, 120));
            let geometry = dd.field_geometry(14);
            let band = dd.field_band();
            assert_eq!(
                geometry.label_box.x + geometry.label_box.width as i32 + INDICATOR_LEADING_GAP,
                geometry.indicator_box.x,
                "the label must stop one gap short of the indicator at width {width}"
            );
            assert!(
                geometry.indicator_box.x + geometry.indicator_box.width as i32
                    <= band.x + band.width as i32,
                "the indicator must stay inside the band at width {width}"
            );
            if width < trailing {
                assert_eq!(
                    geometry.label_box.width, 0,
                    "a band too narrow for its own chrome holds no label at width {width}"
                );
            }
        }
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let dd = Dropdown::new(Vec::new(), Rect::new(0, 0, 240, 120));
        let band = dd.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(dd.size_hint().height, band.height);
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }

    /// The **expanded list** follows the appearance, not just the collapsed field.
    ///
    /// # The defect this pins
    ///
    /// `bg` / `border` / `text_color` read `style`, but the five colours *inside the popup* were
    /// constants: a pale-blue highlight, a near-white row, a grey placeholder and two greys for the
    /// list's border. So the field tracked the theme while the thing it opened did not — a dark
    /// build showed a near-white list under a dark field.
    ///
    /// # Why the assertion is about one specific row, and not the whole document
    ///
    /// A first version compared the *set of every fill* in the two documents. It passed even with
    /// the literals restored — measured — because the field's own background still differs between
    /// appearances, so a document-wide comparison is satisfied by a colour this test is not about.
    /// The assertion has to name the element: the popup's **first unselected row** is the
    /// `surface_container` fill, and that is the one that must move.
    #[test]
    #[cfg(device_profile)]
    fn the_expanded_list_follows_the_appearance() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 200, 60);

        let sample = |appearance| -> (String, String) {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut dd = Dropdown::new(vec!["One".to_string(), "Two".to_string()], rect);
            dd.set_expanded(true);
            crate::theme::apply_theme_to_widget(&mut dd);
            let backdrop = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg_on(&mut dd, rect, backdrop);
            // The rows live below the field's own band, so a fill whose `y` is past it belongs to
            // the popup. Reading the geometry rather than counting elements is what keeps this from
            // depending on how many surfaces the field happens to paint.
            let field_bottom = crate::widget::metrics::dimensions::TEXT_FIELD_MIN_HEIGHT as i32;
            let rows = row_fill(&svg, field_bottom);
            (rows[0].clone(), rows[1].clone())
        };

        // `sample` returns (first row, second row). The first row is the **selected** one — a fresh
        // dropdown with items selects index 0 — so the pair is (selected, unselected) rather than
        // the other way round. Reading them by position is what the helper documents; getting the
        // order wrong is what the first draft of this test did, and what the panic message made
        // obvious.
        let (dark_sel, dark_row) = sample(crate::theme::AppearanceMode::Dark);
        let (light_sel, light_row) = sample(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark_row, light_row,
            "an unselected popup row must follow the appearance; both were {dark_row}"
        );
        assert_ne!(
            dark_sel, light_sel,
            "a selected popup row must follow the appearance; both were {dark_sel}"
        );
        // And the highlighted row must differ from the plain one in each appearance, or the
        // selection is not visible at all.
        assert_ne!(dark_sel, dark_row, "the selected row must stand out on the dark appearance");
        assert_ne!(light_sel, light_row, "the selected row must stand out on the light one");
    }

    /// The fills of the first two popup rows below `field_bottom`, in document order.
    ///
    /// Rows are emitted in order and the first one is the configured selection, so the pair is
    /// `(selected, unselected)`. Walking the rects rather than guessing an index is what keeps this
    /// independent of how many surfaces the collapsed field paints — the count differs between the
    /// two appearances, which is exactly the kind of thing an index would silently track.
    #[cfg(device_profile)]
    fn row_fill(svg: &str, field_bottom: i32) -> Vec<String> {
        let mut rows = Vec::new();
        let mut rest = svg;
        while let Some(rect_at) = rest.find("<rect ") {
            let rect = &rest[rect_at..];
            let end = rect.find("/>").map(|e| e + 2).unwrap_or(rect.len());
            let element = &rect[..end];
            let attr = |name: &str| -> Option<i32> {
                let key = format!(" {name}=\"");
                let at = element.find(&key)? + key.len();
                let to = element[at..].find('"')? + at;
                element[at..to].parse().ok()
            };
            if let (Some(y), Some(key)) = (
                attr("y"),
                element.find("fill=\"rgba(").map(|i| i + "fill=\"rgba(".len()),
            ) {
                if y >= field_bottom {
                    let to = element[key..].find(')').map(|e| e + key).unwrap_or(key);
                    rows.push(element[key..to].to_string());
                    if rows.len() == 2 {
                        return rows;
                    }
                }
            }
            rest = &rect[end..];
        }
        panic!("fewer than two popup rows at or below y={field_bottom}");
    }
}
