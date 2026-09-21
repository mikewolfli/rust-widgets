// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PropertyGrid widget — a two-column property editor table (inspector-style).
//!
//! Displays key-value pairs in a two-column layout similar to VS/Unity Inspector:
//! - Name column (left): bold text on gray background
//! - Value column (right): editable text with alternating row colors
//!   Supports row selection via click and emits a `selected` signal.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A single property row in the grid.
#[derive(Debug, Clone)]
pub struct PropertyItem {
    /// Display name shown in the left column.
    pub name: String,
    /// Current value shown in the right column.
    pub value: String,
    /// Whether the value can be edited by the user.
    pub editable: bool,
}

impl PropertyItem {
    /// Creates a new property item.
    pub fn new(name: impl Into<String>, value: impl Into<String>, editable: bool) -> Self {
        Self { name: name.into(), value: value.into(), editable }
    }
}

/// Height of one property row, in pixels.
///
/// Shared between `draw` and `handle_event`: the two must agree on where a row
/// starts, or a click selects a different row than the one under the pointer.
const ROW_HEIGHT: u32 = 24;

/// Distance from the top of the widget to the first property row.
///
/// The header occupies `[0, ROW_HEIGHT)` and a one-pixel separator line sits just
/// below it, so the first row begins at `ROW_HEIGHT + 1`.
const FIRST_ROW_TOP: u32 = ROW_HEIGHT + 1;

/// PropertyGrid widget — a two-column property editor table.
pub struct PropertyGrid {
    base: BaseWidget,
    properties: Vec<PropertyItem>,
    selected_index: Option<usize>,
    scroll_offset: u32,
    /// Emitted when a property row is selected via click.
    pub selected: Signal1<usize>,
}

impl PropertyGrid {
    /// Number of property rows that fit in the current geometry.
    ///
    /// Mirrors the arithmetic in `draw` so a hit-test can reject a click below the
    /// last painted row. Without this, a click in the empty area under the rows produced
    /// an index that `selected_index < properties.len()` accepted, selecting a row the
    /// user cannot see.
    fn visible_row_count(&self) -> u32 {
        let height = self.geometry().height;
        height.saturating_sub(FIRST_ROW_TOP) / ROW_HEIGHT
    }

    /// The largest `scroll_offset` that still shows a row.
    ///
    /// One definition for the wheel, the arrow keys and the End key. Those three used to recompute
    /// it from a locally-defined `row_height = 24` together with `height - (row_height + 1)`, a
    /// formula that disagreed with [`visible_row_count`](Self::visible_row_count) by one row at
    /// some heights and drifted from it whenever either changed.
    fn max_scroll(&self) -> u32 {
        (self.properties.len() as u32).saturating_sub(self.visible_row_count())
    }
    /// Creates a new PropertyGrid with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PropertyGrid, geometry, "PropertyGrid"),
            properties: Vec::new(),
            selected_index: None,
            scroll_offset: 0,
            selected: Signal1::new(),
        }
    }

    /// Adds a property item to the grid.
    pub fn add_property(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        editable: bool,
    ) {
        self.properties.push(PropertyItem::new(name, value, editable));
        self.base.request_redraw();
    }

    /// Sets the value of the property at the given index.
    /// Returns `true` if the index was valid and the value was updated.
    pub fn set_value(&mut self, index: usize, value: impl Into<String>) -> bool {
        if let Some(item) = self.properties.get_mut(index) {
            item.value = value.into();
            self.base.changed.emit();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Returns a reference to the value of the property at the given index, or `None`.
    pub fn value(&self, index: usize) -> Option<&str> {
        self.properties.get(index).map(|item| item.value.as_str())
    }

    /// Removes all properties from the grid.
    pub fn clear(&mut self) {
        self.properties.clear();
        self.selected_index = None;
        self.scroll_offset = 0;
        self.base.request_redraw();
    }

    /// Returns the number of properties in the grid.
    pub fn property_count(&self) -> usize {
        self.properties.len()
    }

    /// Returns the currently selected index, if any.
    ///
    /// A selection past the end of the list is reported as `None` rather than as a stale index.
    /// [`properties_mut`](Self::properties_mut) hands out `&mut Vec<PropertyItem>`, so a caller can
    /// shrink the list without either of the methods that maintain this invariant
    /// ([`clear`](Self::clear), [`remove_property`](Self::remove_property)) running; the index then
    /// named a row that does not exist, and `draw` highlighted nothing. Filtering here is what makes
    /// the accessor honest regardless of how the list changed.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|&index| index < self.properties.len())
    }

    /// Sets the selected row index. Clamps to valid range.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        let clamped = index.filter(|&i| i < self.properties.len());
        if self.selected_index != clamped {
            self.selected_index = clamped;
            if let Some(idx) = clamped {
                self.selected.emit(idx);
            }
            self.base.request_redraw();
        }
    }

    /// Returns a reference to the properties slice.
    pub fn properties(&self) -> &[PropertyItem] {
        &self.properties
    }

    /// Returns a mutable reference to the properties slice.
    ///
    /// # The selection invariant
    ///
    /// This bypasses the selectors above, so shrinking the list here can leave
    /// [`selected_index`](Self::selected_index) naming a row that no longer exists. That is
    /// tolerated rather than prevented — an accessor returning `&mut Vec` cannot police its
    /// borrow — and [`selected_index`](Self::selected_index) filters the stale value out on read,
    /// so no caller observes a selection the grid cannot paint. Prefer
    /// [`add_property`](Self::add_property) / [`remove_property`](Self::remove_property), which
    /// keep the cursor in range.
    pub fn properties_mut(&mut self) -> &mut Vec<PropertyItem> {
        &mut self.properties
    }
}

impl Widget for PropertyGrid {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `PropertyGrid`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for PropertyGrid {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "property_count" => Ok(CapabilityValue::UInt(self.property_count() as u64)),
            "selected_index" => match self.selected_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                match value {
                    CapabilityValue::Null => self.set_selected_index(None),
                    other => self.set_selected_index(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "property_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["property_count", "selected_index", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `property_grid` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every
    /// property and resets the selection, which is exactly what
    /// [`PropertyGrid::clear`] does. `add_property` takes the row to add, so a
    /// payload-less call is refused as [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_property" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for PropertyGrid {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let row_height = ROW_HEIGHT;
        let name_col_width = rect.width / 3;
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the header bar, the zebra rows, the name column and
        // all three text colours used to be hardcoded literals, so light and dark rendered
        // identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("property_grid");
        let mut surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `property_grid` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // panel painted in that colour would be byte-identical to the frame behind it (which
        // is what the census measured: the whole rect in the window fill), so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the
        // same distinction `Colors::input_background` draws for a field.
        let window_fill = crate::theme::global_theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the header bar and the editable-value ink follow the appearance rather than a
        // literal grey and a literal navy.
        let accent = crate::theme::global_theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        // The header is the panel's own surface driven toward the accent, so it stays legible
        // against whatever the theme resolves rather than being a fixed dark grey.
        let header_bg = surface.blend(&accent, 0.75);
        let header_ink = header_bg.contrast_color();
        let separator = surface.blend(&ink, 0.25);
        // The zebra bands are one step into the surface, so the rows still alternate in either
        // appearance instead of being forced to two fixed light greys.
        let striped_row = surface.blend(&ink, 0.04);
        // Selection and the name column are distinct regions of the same surface, so both are
        // derived from it rather than picked as further literals.
        let selected_row = surface.blend(&accent, 0.30);
        let name_column = surface.blend(&ink, 0.10);
        // Row separators are a subdivision of the surface, not a second literal grey.
        let row_separator = surface.blend(&ink, 0.12);
        let disabled_ink = ink.blend(&surface, 0.55);

        // Background
        context.fill_rect(rect, surface);

        // Header row
        let header_font = Font::bold("Arial", 12.0);
        let header_rect = Rect::new(rect.x, rect.y, rect.width, row_height);
        context.fill_rect(header_rect, header_bg);
        context.draw_text(
            Point::new(rect.x + 4, rect.y + 6),
            "Property",
            &header_font,
            header_ink,
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(rect.x + name_col_width as i32 + 4, rect.y + 6),
            "Value",
            &header_font,
            header_ink,
            HorizontalAlignment::Left,
        );

        // Draw a separator line under header
        let separator_y = rect.y + row_height as i32;
        context.draw_line(
            Point::new(rect.x, separator_y),
            Point::new(rect.x + rect.width as i32, separator_y),
            separator,
        );

        // Property rows
        let value_font = Font::new("Arial", 12.0, false, false);
        let mut y = separator_y + 1;

        #[allow(clippy::manual_checked_ops)]
        let visible_count = if row_height > 0 {
            (rect.height.saturating_sub(FIRST_ROW_TOP)) / row_height
        } else {
            0
        };

        let start_idx = self.scroll_offset as usize;
        let end_idx = (start_idx + visible_count as usize).min(self.properties.len());

        for i in start_idx..end_idx {
            let row_rect = Rect::new(rect.x, y, rect.width, row_height);
            let is_selected = self.selected_index == Some(i);

            // Alternating row background
            if is_selected {
                context.fill_rect(row_rect, selected_row);
            } else if i % 2 == 0 {
                context.fill_rect(row_rect, striped_row);
            } else {
                context.fill_rect(row_rect, surface);
            }

            // Name column background
            let name_rect = Rect::new(rect.x, y, name_col_width, row_height);
            context.fill_rect(name_rect, name_column);

            // Name text (bold)
            let name_text_color = if !is_enabled { disabled_ink } else { ink };
            context.draw_text(
                Point::new(rect.x + 4, y + 6),
                &self.properties[i].name,
                &Font::bold("Arial", 12.0),
                name_text_color,
                HorizontalAlignment::Left,
            );

            // Value text
            // An editable value is a link-like affordance, so it reads the theme's accent
            // instead of a literal navy that disappears on a dark surface.
            let value_color = if !is_enabled {
                disabled_ink
            } else if self.properties[i].editable {
                accent
            } else {
                ink
            };
            context.draw_text(
                Point::new(rect.x + name_col_width as i32 + 4, y + 6),
                &self.properties[i].value,
                &value_font,
                value_color,
                HorizontalAlignment::Left,
            );

            // Row separator
            context.draw_line(
                Point::new(rect.x, y + row_height as i32 - 1),
                Point::new(rect.x + rect.width as i32, y + row_height as i32 - 1),
                row_separator,
            );

            y += row_height as i32;
        }
    }
}

impl EventHandler for PropertyGrid {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();

                    // The click is measured from the widget's own top edge, so a press
                    // above the widget (`click_y < 0`) is a click outside it and must
                    // deselect rather than index a row. The previous arithmetic cast
                    // `click_y` to `u32` before subtracting, which turned a negative
                    // offset into a huge row index.
                    let click_y = pos.y - rect.y;

                    if click_y >= FIRST_ROW_TOP as i32 {
                        // `draw` paints row `scroll_offset + i` at
                        // `y = rect.y + FIRST_ROW_TOP + i * ROW_HEIGHT`, so this is the
                        // inverse of that mapping. `>=` (not `>`) matters: a click on the
                        // first row's top edge is a click on that row, and `>` skipped it.
                        let row_in_view = (click_y as u32 - FIRST_ROW_TOP) / ROW_HEIGHT;
                        let row_index = row_in_view + self.scroll_offset;

                        // Bound above by the rows actually painted. Without this, a click
                        // in the blank area below the last row selected an invisible row
                        // that merely happened to exist in `properties`.
                        if row_in_view < self.visible_row_count()
                            && (row_index as usize) < self.properties.len()
                        {
                            self.selected_index = Some(row_index as usize);
                            self.selected.emit(row_index as usize);
                            self.base.clicked.emit();
                            self.base.request_redraw();
                            return;
                        }
                    }

                    // Click outside rows — deselect
                    if self.selected_index.is_some() {
                        self.selected_index = None;
                        self.base.request_redraw();
                    }
                }
            }
            Event::Wheel { delta, .. } => {
                // `max_scroll` and the visible count both come from the shared helpers, so the
                // wheel can never park the view past what `draw` paints. This arm used to recompute
                // the count from a locally-defined `row_height = 24` and `height - (row_height + 1)`
                // — a third copy of the geometry, which is how the three formulas drifted apart.
                let max_scroll = self.max_scroll();
                if delta.y > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                } else if delta.y < 0 {
                    self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
                }
                // Clamp unconditionally: shrinking the row count or the geometry while scrolled
                // must not leave the offset past the end.
                self.scroll_offset = self.scroll_offset.min(max_scroll);
                self.base.request_redraw();
            }
            Event::KeyPress { key, .. } => {
                let max_scroll = self.max_scroll();
                // Keyboard navigation is refused when no row is painted. `draw` renders
                // `[scroll_offset, scroll_offset + visible_row_count)`, and at a short geometry
                // (`height < FIRST_ROW_TOP + ROW_HEIGHT`, i.e. under 49 px) that range is empty.
                // The arms below used `self.selected_index.unwrap_or(0)`, so with nothing selected
                // — and nothing visible — Down still committed `Some(0)`: a selection the grid can
                // never paint. Refusing up front keeps the cursor consistent with what is on screen.
                let visible = self.visible_row_count();
                if visible == 0 {
                    self.base.handle_event(event);
                    return;
                }
                match *key {
                    38 => {
                        // Up arrow — move selection up
                        let current = self.selected_index.unwrap_or(0);
                        if current > 0 {
                            let new_idx = current - 1;
                            self.selected_index = Some(new_idx);
                            self.selected.emit(new_idx);
                            // Scroll if selection moved above viewport
                            if new_idx < self.scroll_offset as usize {
                                self.scroll_offset = new_idx as u32;
                            }
                            self.base.request_redraw();
                        }
                    }
                    40 => {
                        // Down arrow — move selection down
                        let new_idx = match self.selected_index {
                            None => {
                                if self.properties.is_empty() {
                                    return;
                                }
                                0 // select first if nothing selected
                            }
                            Some(current) if current + 1 < self.properties.len() => current + 1,
                            _ => return, // already at last row
                        };
                        self.selected_index = Some(new_idx);
                        self.selected.emit(new_idx);
                        let visible = self.visible_row_count();
                        // Scroll if selection moved below viewport
                        if new_idx >= self.scroll_offset as usize + visible as usize {
                            self.scroll_offset = (new_idx as u32 + 1).saturating_sub(visible);
                        }
                        self.base.request_redraw();
                    }
                    36 => {
                        // Home — jump to first row
                        if !self.properties.is_empty() {
                            self.selected_index = Some(0);
                            self.selected.emit(0);
                            self.scroll_offset = 0;
                            self.base.request_redraw();
                        }
                    }
                    35 => {
                        // End — jump to last row
                        if !self.properties.is_empty() {
                            let last = self.properties.len() - 1;
                            self.selected_index = Some(last);
                            self.selected.emit(last);
                            let visible = self.visible_row_count();
                            self.scroll_offset =
                                (last as u32 + 1).saturating_sub(visible).min(max_scroll);
                            self.base.request_redraw();
                        }
                    }
                    _ => {
                        self.base.handle_event(event);
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    #[test]
    fn property_grid_new_is_empty() {
        let pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        assert_eq!(pg.property_count(), 0);
        assert_eq!(pg.selected_index(), None);
        assert_eq!(pg.kind(), WidgetKind::PropertyGrid);
    }

    #[test]
    fn property_grid_add_property() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("Name", "John Doe", true);
        pg.add_property("Age", "30", false);
        assert_eq!(pg.property_count(), 2);
        assert_eq!(pg.value(0), Some("John Doe"));
        assert_eq!(pg.value(1), Some("30"));
    }

    #[test]
    fn property_grid_set_value() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("Name", "", true);
        assert!(pg.set_value(0, "Alice"));
        assert_eq!(pg.value(0), Some("Alice"));
    }

    #[test]
    fn property_grid_set_value_invalid_index() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        assert!(!pg.set_value(0, "test"));
    }

    #[test]
    fn property_grid_clear() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.set_selected_index(Some(0));
        pg.clear();
        assert_eq!(pg.property_count(), 0);
        assert_eq!(pg.selected_index(), None);
    }

    #[test]
    fn property_grid_selected_signal() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("X", "10", true);
        pg.add_property("Y", "20", true);

        let captured = Arc::new(Mutex::new(None));
        pg.selected.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        pg.set_selected_index(Some(1));
        assert_eq!(*captured.lock().unwrap(), Some(1));
        assert_eq!(pg.selected_index(), Some(1));
    }

    #[test]
    fn property_grid_mouse_click_selects_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.add_property("C", "3", true);

        // First row is at y=25 (header at 0..24, then y=25+1=26 is row 0 start, so
        // click on row 0: y=26..49, clicking at y=30 should select index 0
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 35), button: 1 });
        assert_eq!(pg.selected_index(), Some(0));
    }

    #[test]
    fn property_grid_mouse_click_header_does_not_select() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);

        // Click in header area (y < row_height + 1 = 25)
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(pg.selected_index(), None);
    }

    /// Row 0 occupies `[25, 49)`; its top edge is part of it.
    ///
    /// The hit-test compared `click_y > header_height` (25), so a click exactly on the
    /// first row's top edge fell into the header branch and selected nothing, while
    /// `y = 26` selected row 0. `draw` paints that row starting at 25, so the boundary
    /// case is the row's own first pixel.
    #[test]
    fn property_grid_first_row_top_edge_selects_row_zero() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);

        pg.handle_event(&Event::MousePress {
            pos: Point::new(10, FIRST_ROW_TOP as i32),
            button: 1,
        });
        assert_eq!(pg.selected_index(), Some(0), "the first row's top edge is part of it");

        // The last pixel of the previous row is not.
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.handle_event(&Event::MousePress {
            pos: Point::new(10, FIRST_ROW_TOP as i32 - 1),
            button: 1,
        });
        assert_eq!(pg.selected_index(), None, "the separator line is not a row");
    }

    /// Rows are addressed relative to the widget's own origin.
    ///
    /// The arithmetic cast `click_y as u32` *before* subtracting the header, so a click
    /// above the widget wrapped to a near-`u32::MAX` row index instead of being treated
    /// as a click outside.
    #[test]
    fn property_grid_click_above_the_widget_deselects() {
        let mut pg = PropertyGrid::new(Rect::new(0, 50, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);

        // Select row 1 first, so "deselect" is observable.
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 50 + 30), button: 1 });
        assert_eq!(pg.selected_index(), Some(0));

        // A click above the widget's top edge is outside it.
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 5), button: 1 });
        assert_eq!(pg.selected_index(), None, "a click above the widget must deselect");
    }

    /// A click below the last painted row must not select an unpainted row.
    ///
    /// The hit-test only bounded the index by `properties.len()`, so clicking the blank
    /// area under seven visible rows selected a tenth property the user cannot see.
    #[test]
    fn property_grid_click_below_the_last_row_does_not_select() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        for i in 0..10 {
            pg.add_property(format!("p{i}"), format!("v{i}"), true);
        }

        // (200 - 25) / 24 = 7 rows are painted, occupying [25, 193).
        assert_eq!(pg.visible_row_count(), 7);

        pg.handle_event(&Event::MousePress { pos: Point::new(10, 196), button: 1 });
        assert_eq!(pg.selected_index(), None, "row 7 of 10 is not painted here");

        // The last painted row is still reachable.
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 190), button: 1 });
        assert_eq!(pg.selected_index(), Some(6));
    }

    #[test]
    fn property_grid_property_item_accessors() {
        let item = PropertyItem::new("Name", "value", true);
        assert_eq!(item.name, "Name");
        assert_eq!(item.value, "value");
        assert!(item.editable);
    }

    #[test]
    fn property_grid_svg_output() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("Name", "Alice", true);
        pg.add_property("Age", "30", false);
        let svg = crate::widget::svg::render_to_svg(&mut pg);
        assert!(svg.starts_with("<svg"));
    }

    // ── Keyboard navigation tests ──

    #[test]
    fn property_grid_key_down_selects_next_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.add_property("C", "3", true);
        pg.set_selected_index(Some(0));

        pg.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(1));

        pg.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(2));
    }

    #[test]
    fn property_grid_key_up_selects_prev_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.set_selected_index(Some(1));

        pg.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(0));
    }

    #[test]
    fn property_grid_key_up_stops_at_first_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.set_selected_index(Some(0));

        pg.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(0), "should stay at first row");
    }

    #[test]
    fn property_grid_key_down_stops_at_last_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.set_selected_index(Some(1));

        pg.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(1), "should stay at last row");
    }

    #[test]
    fn property_grid_key_home_jumps_to_first_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.add_property("C", "3", true);
        pg.set_selected_index(Some(2));

        pg.handle_event(&Event::KeyPress { key: 36, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(0));
    }

    #[test]
    fn property_grid_key_end_jumps_to_last_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.add_property("C", "3", true);
        pg.set_selected_index(Some(0));

        pg.handle_event(&Event::KeyPress { key: 35, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(2));
    }

    #[test]
    fn property_grid_key_down_from_unselected_selects_first() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);

        // No selection yet; down arrow should select index 0
        pg.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(0));
    }

    #[test]
    fn property_grid_key_navigation_emits_selected_signal() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.add_property("C", "3", true);
        pg.set_selected_index(Some(0));

        let captured = Arc::new(Mutex::new(None));
        pg.selected.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        pg.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(*captured.lock().unwrap(), Some(1));
    }
    /// Keyboard navigation is refused when the geometry paints no rows.
    ///
    /// `draw` renders `[scroll_offset, scroll_offset + visible_row_count)`. At `height = 48` that
    /// range is empty (48 - 25 = 23, / 24 = 0 rows). The arrow arms used
    /// `self.selected_index.unwrap_or(0)`, so with nothing selected Down still committed `Some(0)`:
    /// a row the grid never paints. The click path already refused it via `visible_row_count()`.
    #[test]
    fn property_grid_keyboard_navigation_refuses_when_no_row_is_visible() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 48));
        for i in 0..5 {
            pg.add_property(format!("p{i}"), format!("v{i}"), true);
        }
        assert_eq!(pg.visible_row_count(), 0, "the geometry paints no rows at all");

        // Down arrow
        pg.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(pg.selected_index(), None, "Down must not select an unpainted row");
        // Up arrow
        pg.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(pg.selected_index(), None);
        // End must not jump to the last row either.
        pg.handle_event(&Event::KeyPress { key: 35, modifiers: 0 });
        assert_eq!(pg.selected_index(), None);
        // Home is the one harmless case; it selects 0, which is also unpainted.
        pg.handle_event(&Event::KeyPress { key: 36, modifiers: 0 });
        assert_eq!(pg.selected_index(), None, "no row is visible, so none can be selected");

        // A tall-enough geometry still navigates.
        let mut tall = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        for i in 0..5 {
            tall.add_property(format!("p{i}"), format!("v{i}"), true);
        }
        tall.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(tall.selected_index(), Some(0));
    }

    /// A selection left dangling by `properties_mut` reads as `None`, not as a stale index.
    ///
    /// `properties_mut` returns `&mut Vec<PropertyItem>`, so a caller can shrink the list without
    /// `clear` or `remove_property` running. The accessor then reported an index naming no row, and
    /// `draw` highlighted nothing — the selection silently vanished while still being reported.
    #[test]
    fn property_grid_selection_does_not_outlive_its_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        for i in 0..5 {
            pg.add_property(format!("p{i}"), format!("v{i}"), true);
        }
        pg.set_selected_index(Some(4));
        assert_eq!(pg.selected_index(), Some(4));

        pg.properties_mut().truncate(2);
        assert_eq!(pg.property_count(), 2);
        assert_eq!(pg.selected_index(), None, "an index past the end must read as no selection");

        // Re-growing the list must not resurrect the stale index.
        pg.add_property("new", "v", true);
        assert_eq!(pg.selected_index(), None);
    }

    /// The wheel and the keys cannot park the view past the last row.
    ///
    /// `max_scroll` used to be recomputed inline from a locally-defined `row_height = 24` and
    /// `height - (row_height + 1)` — a second and third copy of the geometry that disagreed with
    /// `visible_row_count()` by one row at some heights.
    #[test]
    fn property_grid_scroll_offset_stays_within_range() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        for i in 0..5 {
            pg.add_property(format!("p{i}"), format!("v{i}"), true);
        }
        // 5 rows, 7 fit, so nothing to scroll.
        assert_eq!(pg.max_scroll(), 0);

        // Wheel down repeatedly.
        for _ in 0..10 {
            pg.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        }
        assert!(pg.scroll_offset <= pg.max_scroll(), "wheel must not scroll past the end");

        // End key with a list longer than the viewport.
        let mut long = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        for i in 0..50 {
            long.add_property(format!("p{i}"), format!("v{i}"), true);
        }
        long.handle_event(&Event::KeyPress { key: 35, modifiers: 0 });
        assert!(long.scroll_offset <= long.max_scroll());
        assert_eq!(long.selected_index(), Some(49));
    }
}
