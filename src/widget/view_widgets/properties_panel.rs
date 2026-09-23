// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PropertiesPanel widget — a categorized property editor panel.
//!
//! Similar to VS Code properties view or Qt QTreeView with property delegates.
//! Properties are grouped by category and rendered as a grid with name-value pairs.
//! Supports filter text for search, editable values (text, number, bool, color,
//! choice, file), and emits `property_changed` on edits.

use crate::core::{Color, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::ControlMetrics;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::HashMap;

/// How a property is represented and edited inside a [`PropertiesPanel`].
///
/// # Not the same as [`crate::object::PropertyValue`]
///
/// Both are called `PropertyValue` but model different things, so they are kept
/// separate (principle #49):
///
/// * this one — the editor vocabulary of a panel entry, including presentation
///   variants (`Color`, `Choice { options }`) that only make sense in a UI;
/// * [`crate::object::PropertyValue`] — the four scalar kinds an
///   object property can hold, with no presentation semantics.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// Free-form text value.
    Text(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Bool(bool),
    /// Color value.
    Color(Color),
    /// Choice from a list of options.
    Choice {
        /// Available options.
        options: Vec<String>,
        /// Index of the selected option.
        selected: usize,
    },
    /// File path string.
    File(String),
}

impl PartialEq for PropertyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => (a - b).abs() < f64::EPSILON,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Color(a), Self::Color(b)) => {
                a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a
            }
            (
                Self::Choice { options: ao, selected: as_ },
                Self::Choice { options: bo, selected: bs },
            ) => ao == bo && as_ == bs,
            (Self::File(a), Self::File(b)) => a == b,
            _ => false,
        }
    }
}

/// A single entry in the properties panel.
#[derive(Debug, Clone)]
pub struct PropertyEntry {
    /// Display name of the property.
    pub name: String,
    /// Current value.
    pub value: PropertyValue,
    /// Optional category grouping. Properties with the same category are grouped together.
    pub category: Option<String>,
    /// Optional description shown as a tooltip.
    pub description: Option<String>,
    /// Whether this property can be edited by the user.
    pub editable: bool,
}

impl PropertyEntry {
    /// Creates a new property entry.
    pub fn new(
        name: &str,
        value: PropertyValue,
        category: Option<&str>,
        description: Option<&str>,
        editable: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            value,
            category: category.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            editable,
        }
    }
}

/// Layout constants for drawing.
const ROW_HEIGHT: u32 = 26;
const CATEGORY_PADDING: i32 = 4;
const NAME_COL_LEFT: i32 = 8;
const VALUE_COL_LEFT: i32 = 148;
/// The margin the panel leaves between its own frame and its rows: 2 px on every edge.
///
/// Named once so the first category header and the rows below it are all measured from the
/// same inset. They used to start at the panel's literal `geom.y`, which pinned the first
/// header's glyph box to the frame's own stroke.
const PANEL_INSET: u32 = 2;

/// PropertiesPanel widget — a property editor with categorized grid layout.
pub struct PropertiesPanel {
    base: BaseWidget,
    /// All property entries in order.
    properties: Vec<PropertyEntry>,
    /// Current filter text for searching properties by name.
    filter_text: String,
    /// Internal scroll offset for the property list.
    scroll_offset: i32,
    /// Emitted when a property value changes, provides (name, new_value).
    pub property_changed: Signal1<(String, PropertyValue)>,
}

impl PropertiesPanel {
    /// Creates a new PropertiesPanel with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PropertiesPanel, geometry, "PropertiesPanel"),
            properties: Vec::new(),
            filter_text: String::new(),
            scroll_offset: 0,
            property_changed: Signal1::new(),
        }
    }

    // ── Property management ──

    /// Adds a property entry to the panel.
    pub fn add_property(&mut self, entry: PropertyEntry) {
        self.properties.push(entry);
        self.base.request_redraw();
    }

    /// Removes a property by name. Returns true if found and removed.
    pub fn remove_property(&mut self, name: &str) -> bool {
        let len_before = self.properties.len();
        self.properties.retain(|p| p.name != name);
        let removed = self.properties.len() < len_before;
        if removed {
            self.base.request_redraw();
        }
        removed
    }

    /// Removes all properties.
    pub fn clear_properties(&mut self) {
        self.properties.clear();
        self.base.request_redraw();
    }

    /// Returns a reference to all properties.
    pub fn properties(&self) -> &[PropertyEntry] {
        &self.properties
    }

    /// Sets the value of a property by name. Returns true if the property was found.
    pub fn set_property_value(&mut self, name: &str, value: PropertyValue) -> bool {
        if let Some(entry) = self.properties.iter_mut().find(|p| p.name == name) {
            if entry.value != value {
                entry.value = value.clone();
                self.property_changed.emit((name.to_string(), value));
                self.base.request_redraw();
            }
            true
        } else {
            false
        }
    }

    /// Gets the value of a property by name.
    pub fn get_property_value(&self, name: &str) -> Option<PropertyValue> {
        self.properties.iter().find(|p| p.name == name).map(|p| p.value.clone())
    }

    // ── Filter ──

    /// Sets the filter text. Only properties whose name contains the filter
    /// (case-insensitive) will be shown.
    pub fn set_filter(&mut self, text: &str) {
        self.filter_text = text.to_string();
        self.scroll_offset = 0;
        self.base.request_redraw();
    }

    /// Returns the current filter text.
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    /// Returns the filtered properties that match the current filter text.
    fn filtered_properties(&self) -> Vec<&PropertyEntry> {
        if self.filter_text.is_empty() {
            return self.properties.iter().collect();
        }
        let lower = self.filter_text.to_lowercase();
        self.properties.iter().filter(|p| p.name.to_lowercase().contains(&lower)).collect()
    }

    // ── Category grouping ──

    /// Returns properties grouped by category. Properties without a category
    /// are placed under "General".
    pub fn properties_by_category(&self) -> Vec<(String, Vec<&PropertyEntry>)> {
        let filtered = self.filtered_properties();
        let mut grouped: HashMap<String, Vec<&PropertyEntry>> = HashMap::new();
        for entry in filtered {
            let cat = entry.category.clone().unwrap_or_else(|| "General".to_string());
            grouped.entry(cat).or_default().push(entry);
        }
        let mut result: Vec<_> = grouped.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    // ── Scroll ──

    /// Sets the scroll offset.
    pub fn set_scroll_offset(&mut self, offset: i32) {
        self.scroll_offset = offset.max(0);
        self.base.request_redraw();
    }

    /// Returns the scroll offset.
    pub fn scroll_offset(&self) -> i32 {
        self.scroll_offset
    }

    // ── Row geometry ──

    /// The area a row may occupy: the panel's box minus the margin that keeps ink off the
    /// frame's own stroke. Both the draw pass and the hit test start from this, so a row's
    /// column stops coincide with the clickable box by construction.
    fn content_box(&self) -> Rect {
        ControlMetrics::band_inset(self.geometry(), PANEL_INSET)
    }

    /// The box of the `row`-th visual row, counted from the first category header.
    ///
    /// Row `0` is the first header; each header and each property entry take exactly one row.
    /// The box is a pure function of the index and the current scroll offset, so the row a
    /// press resolves to is exactly the row that was painted — the draw loop and
    /// [`Self::handle_event`] both read this instead of each advancing their own `y` cursor.
    fn row_rect_for(&self, row: usize) -> Rect {
        let content = self.content_box();
        let top = content.y - self.scroll_offset + row as i32 * ROW_HEIGHT as i32;
        Rect::new(content.x, top, content.width, ROW_HEIGHT)
    }

    /// The total number of visual rows the panel paints: one header plus one row per entry,
    /// over the currently filtered grouping.
    fn visual_row_count(&self) -> usize {
        self.properties_by_category().iter().map(|(_, entries)| 1 + entries.len()).sum()
    }

    /// The first visual row that is entirely above the content box, i.e. how many rows the
    /// scroll offset has pushed out of view. Rows before it are skipped rather than painted,
    /// so a deep scroll does not walk every row it has already passed.
    fn first_visible_row(&self) -> usize {
        (self.scroll_offset.max(0) as usize) / ROW_HEIGHT as usize
    }

    /// Toggles the editable entry that occupies the visual row drawn at `row_rect`.
    ///
    /// The row's index is recovered from the same box the draw pass produced, so the mapping
    /// from a painted row to a property cannot drift. Recovering it here (rather than
    /// threading an entry reference out of the hit-test loop) also avoids a second, divergent
    /// copy of the row walk.
    fn toggle_editable_row(&mut self, row_rect: Rect, pos: crate::core::Point) {
        let Some(name) = self.editable_name_at(row_rect, pos) else { return };
        // Only booleans toggle on click; every other kind needs a real editor, so the click is
        // acknowledged by a redraw but must not silently rewrite the value.
        if let Some(entry) = self.properties.iter().find(|e| e.name == name) {
            if let PropertyValue::Bool(value) = entry.value {
                self.set_property_value(&name, PropertyValue::Bool(!value));
            }
        }
        self.base.request_redraw();
    }

    /// The name of the editable property whose value column covers `pos`, when the row drawn
    /// at `row_rect` is that property's row.
    ///
    /// A click on the name column or on a category header resolves to `None`: headers are not
    /// editable, and the name column is a label, not a value editor.
    fn editable_name_at(&self, row_rect: Rect, pos: crate::core::Point) -> Option<String> {
        let content = self.content_box();
        let value_rect = Rect::new(
            content.x + VALUE_COL_LEFT,
            row_rect.y,
            content.width.saturating_sub(VALUE_COL_LEFT as u32),
            ROW_HEIGHT,
        );
        if !value_rect.contains_point(pos) {
            return None;
        }
        self.row_owner(row_rect.y - content.y + self.scroll_offset)
    }

    /// Maps a visual row's vertical offset within the scrolled content back to the name of the
    /// entry it carries, or `None` when the offset falls on a category header.
    ///
    /// This is the inverse of the row walk in `visual_row_count`: headers and entries occupy
    /// exactly one `ROW_HEIGHT` band each, in the same order the grouping yields them.
    fn row_owner(&self, offset: i32) -> Option<String> {
        if offset < 0 {
            return None;
        }
        let mut row = 0usize;
        let target = offset as usize / ROW_HEIGHT as usize;
        for (_, entries) in self.properties_by_category() {
            if row == target {
                return None; // a category header
            }
            row += 1;
            for entry in entries {
                if row == target {
                    return entry.editable.then(|| entry.name.clone());
                }
                row += 1;
            }
        }
        None
    }

    /// Returns the text representation of a property value for display.
    fn value_display_text(value: &PropertyValue) -> String {
        match value {
            PropertyValue::Text(s) => s.clone(),
            PropertyValue::Number(n) => {
                if *n == n.floor() && n.is_finite() {
                    format!("{}", *n as i64)
                } else {
                    format!("{n:.2}")
                }
            }
            PropertyValue::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            PropertyValue::Color(c) => c.to_hex_rgb(),
            PropertyValue::Choice { options, selected } => {
                if *selected < options.len() {
                    options[*selected].clone()
                } else {
                    "Invalid".to_string()
                }
            }
            PropertyValue::File(s) => s.clone(),
        }
    }
}

impl Widget for PropertiesPanel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 400)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `PropertiesPanel`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` dispatch, so callers see the same value shape as
/// before; the count now comes from the panel's real entries instead of the
/// placeholder default. `property_count` is derived from the entry list and has
/// no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for PropertiesPanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "property_count" => Ok(CapabilityValue::UInt(self.properties().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Derived from the entry list, which owns it.
            "property_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["property_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for PropertiesPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("properties_panel");
        // `properties_panel` is not a control kind in the role table, so it classifies
        // as `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The panel's own fill is therefore a step toward the
        // foreground, so it reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::BACKGROUND);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::BORDER);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::FOREGROUND);
        let background = resolved.blend(&text_color, 0.08);
        // The category header is a second step away from the panel body.
        let header_background = background.blend(&text_color, 0.16);
        // A row is raised off the panel, the value cell deeper still, so the two
        // columns stay legible as separate regions.
        let row_background = background.blend(&text_color, 0.04);
        let value_background = background.blend(&text_color, 0.12);
        // Row dividers are secondary chrome, derived from the same pair.
        let divider = background.blend(&text_color, 0.14);
        // An editable value reads as normal text; a read-only one is muted rather
        // than the previous literal grey.
        let value_color = text_color;
        let readonly_value_color = text_color.blend(&value_background, 0.5);

        // ── Background ──
        context.fill_rect(geom, background);

        let font = crate::core::Font::simple("sans-serif", 12.0);
        let categories = self.properties_by_category();
        // Rows scroll inside the panel's own content box, not from its literal top edge: the
        // first category header used to start at `geom.y`, so its glyph box began on the
        // frame's own stroke. `content_box` reserves the panel's margin and the scroll offset
        // is applied inside it, so the ink and the scroll both move together.
        let content = self.content_box();
        let bottom = content.y + content.height as i32;
        // Every row's box comes from `row_rect_for`, the same derivation the hit test reads, so
        // a drawn row and a clickable row can never drift apart. The loop used to advance its
        // own `y += ROW_HEIGHT` cursor, which made a row's box depend on how many rows happened
        // to precede it and let the columns disagree about where the panel's margin was.
        let mut row = self.first_visible_row();

        for (category, entries) in &categories {
            // ── Category header ──
            let header_rect = self.row_rect_for(row);
            row += 1;
            context.fill_rect(header_rect, header_background);
            // Centred through the shared primitive. Every row of this panel used to draw at
            // `y + ROW_HEIGHT / 2 + 4`: the `+ 4` was meant to compensate for the top-edge
            // origin but only halved the error, so a 12 px label in a 26 px row occupied
            // `17..29` and its descenders crossed the divider drawn at `y + ROW_HEIGHT - 1`.
            let header_line = context.text_line(header_rect, &font);
            context.draw_text_fitted(
                Rect {
                    x: header_rect.x + CATEGORY_PADDING,
                    y: header_line.y,
                    width: header_rect.width.saturating_sub(CATEGORY_PADDING as u32),
                    height: header_line.height,
                },
                category,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );

            for entry in entries {
                let row_rect = self.row_rect_for(row);
                row += 1;
                if row_rect.y >= bottom {
                    break;
                }
                if row_rect.y + ROW_HEIGHT as i32 <= content.y {
                    continue;
                }

                // Alternate row background
                context.fill_rect(row_rect, row_background);
                let row_line = context.text_line(row_rect, &font);

                // Property name. Guarded on the text being non-empty so a nameless entry
                // emits no `<text …></text>`.
                let name_x = content.x + NAME_COL_LEFT;
                if !entry.name.is_empty() {
                    context.draw_text_fitted(
                        Rect {
                            x: name_x,
                            y: row_line.y,
                            width: (content.x + VALUE_COL_LEFT - name_x).max(0) as u32,
                            height: row_line.height,
                        },
                        &entry.name,
                        &font,
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }

                // Property value (with label-style background)
                let value_rect = Rect::new(
                    content.x + VALUE_COL_LEFT,
                    row_rect.y,
                    content.width.saturating_sub(VALUE_COL_LEFT as u32),
                    ROW_HEIGHT,
                );
                let display = Self::value_display_text(&entry.value);
                context.fill_rect(value_rect, value_background);
                let value_line = context.text_line(value_rect, &font);
                if !display.is_empty() {
                    context.draw_text_fitted(
                        Rect {
                            x: value_rect.x + 2,
                            y: value_line.y,
                            width: value_rect.width.saturating_sub(4),
                            height: value_line.height,
                        },
                        &display,
                        &font,
                        if entry.editable { value_color } else { readonly_value_color },
                        HorizontalAlignment::Left,
                    );
                }

                // Draw bottom border line
                context.draw_rect_stroke(
                    Rect::new(row_rect.x, row_rect.y + ROW_HEIGHT as i32 - 1, row_rect.width, 1),
                    divider,
                    1,
                );
            }
        }

        // Draw border around the entire panel
        context.draw_rect_stroke(geom, border, 1);
    }
}

impl EventHandler for PropertiesPanel {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, button } => {
                // A disabled panel must not edit values. Without this gate
                // `set_enabled(false)` left the rows clickable, so a panel put into a
                // read-only state still mutated its properties.
                if !self.base.is_enabled() {
                    self.base.handle_event(event);
                    return;
                }
                if *button == 1 {
                    // The press is resolved through the same derivation the draw pass uses, so
                    // the editable box a click lands in is the box that was painted for that
                    // row. The old version re-ran its own `y` accumulator from `geom.y`
                    // (without the panel inset the draw pass applies), so the clickable area
                    // was offset from the painted one by the frame margin.
                    let rows = self.visual_row_count();
                    let mut row = self.first_visible_row();
                    while row < rows {
                        let row_rect = self.row_rect_for(row);
                        row += 1;
                        // Row 0 is the first category header; headers are separators and are
                        // not editable. The offset from a header to its entries is one row,
                        // matching `1 + entries.len()` in `visual_row_count`.
                        if row_rect.contains_point(*pos) {
                            self.toggle_editable_row(row_rect, *pos);
                            return;
                        }
                    }
                }
            }
            Event::Wheel { delta, modifiers: _ } => {
                self.scroll_offset = (self.scroll_offset - delta.y * 3).max(0);
                self.base.request_redraw();
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
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn make_text_property(name: &str, value: &str, category: Option<&str>) -> PropertyEntry {
        PropertyEntry::new(
            name,
            PropertyValue::Text(value.to_string()),
            category,
            Some("A text property"),
            true,
        )
    }

    fn make_bool_property(name: &str, value: bool, category: Option<&str>) -> PropertyEntry {
        PropertyEntry::new(name, PropertyValue::Bool(value), category, Some("A boolean"), true)
    }

    fn make_number_property(name: &str, value: f64, category: Option<&str>) -> PropertyEntry {
        PropertyEntry::new(name, PropertyValue::Number(value), category, Some("A number"), true)
    }

    fn make_choice_property(
        name: &str,
        options: Vec<String>,
        selected: usize,
        category: Option<&str>,
    ) -> PropertyEntry {
        PropertyEntry::new(
            name,
            PropertyValue::Choice { options, selected },
            category,
            Some("A choice"),
            true,
        )
    }

    #[test]
    fn properties_panel_default_creation() {
        let panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        assert_eq!(panel.kind(), WidgetKind::PropertiesPanel);
        assert_eq!(panel.properties().len(), 0);
        assert!(panel.filter_text().is_empty());
        assert!(panel.get_property_value("nothing").is_none());
    }

    #[test]
    fn properties_panel_add_remove_properties() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        assert_eq!(panel.properties().len(), 0);

        panel.add_property(make_text_property("name", "Alice", Some("General")));
        panel.add_property(make_bool_property("enabled", true, Some("Behavior")));
        panel.add_property(make_number_property("count", 42.0, Some("General")));
        assert_eq!(panel.properties().len(), 3);

        assert!(panel.remove_property("name"));
        assert_eq!(panel.properties().len(), 2);

        assert!(!panel.remove_property("nonexistent"));
        assert_eq!(panel.properties().len(), 2);

        panel.clear_properties();
        assert_eq!(panel.properties().len(), 0);
    }

    #[test]
    fn properties_panel_get_set_value() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("greeting", "Hello", None));

        let val = panel.get_property_value("greeting");
        assert!(val.is_some());
        match val.unwrap() {
            PropertyValue::Text(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected Text value"),
        }

        // Set new value
        let updated = panel.set_property_value("greeting", PropertyValue::Text("Hi".to_string()));
        assert!(updated);

        let updated_val = panel.get_property_value("greeting").unwrap();
        match updated_val {
            PropertyValue::Text(s) => assert_eq!(s, "Hi"),
            _ => panic!("Expected Text value"),
        }

        // Setting value for non-existent property returns false
        assert!(!panel.set_property_value("nonexistent", PropertyValue::Text("x".to_string())));
    }

    #[test]
    fn properties_panel_set_value_emits_signal() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("name", "Alice", None));

        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&count);
        panel.property_changed.connect(move |pair: Arc<(String, PropertyValue)>| {
            if pair.0 == "name" && matches!(pair.1, PropertyValue::Text(ref s) if s == "Bob") {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        panel.set_property_value("name", PropertyValue::Text("Bob".to_string()));
        assert_eq!(count.load(Ordering::SeqCst), 1, "property_changed signal should fire");
    }

    #[test]
    fn properties_panel_filter() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("FontSize", "12", Some("Appearance")));
        panel.add_property(make_text_property("FontFamily", "Arial", Some("Appearance")));
        panel.add_property(make_text_property("BackgroundColor", "blue", Some("Appearance")));
        panel.add_property(make_number_property("Volume", 75.0, Some("Audio")));

        // No filter — all properties
        assert_eq!(panel.filtered_properties().len(), 4);

        // Filter by "font"
        panel.set_filter("font");
        assert_eq!(panel.filtered_properties().len(), 2, "Should match FontSize and FontFamily");
        assert_eq!(panel.filter_text(), "font");

        // Filter by "volume"
        panel.set_filter("Volume");
        assert_eq!(panel.filtered_properties().len(), 1);

        // Filter clears for non-matching
        panel.set_filter("zzzzz");
        assert_eq!(panel.filtered_properties().len(), 0);

        // Clear filter
        panel.set_filter("");
        assert_eq!(panel.filtered_properties().len(), 4);
    }

    #[test]
    fn properties_panel_properties_by_category() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("Name", "Alice", Some("General")));
        panel.add_property(make_bool_property("Visible", true, Some("Behavior")));
        panel.add_property(make_text_property("Title", "My App", Some("General")));
        panel.add_property(make_number_property("Opacity", 1.0, None));
        panel.add_property(make_bool_property("Enabled", true, Some("Behavior")));

        let categories = panel.properties_by_category();
        // None-category properties are grouped under "General"
        assert_eq!(categories.len(), 2, "Expected 2 categories: Behavior, General");

        let cat_names: Vec<&str> = categories.iter().map(|(c, _)| c.as_str()).collect();
        assert!(cat_names.contains(&"Behavior"));
        assert!(cat_names.contains(&"General"));

        for (cat, entries) in &categories {
            if cat == "Behavior" {
                assert_eq!(entries.len(), 2);
            } else if cat == "General" {
                // Name, Title (explicit General) + Opacity (None -> General)
                assert_eq!(entries.len(), 3);
            }
        }
    }

    #[test]
    fn properties_panel_value_display() {
        assert_eq!(
            PropertiesPanel::value_display_text(&PropertyValue::Text("hello".to_string())),
            "hello"
        );
        assert_eq!(PropertiesPanel::value_display_text(&PropertyValue::Number(42.0)), "42");
        assert_eq!(
            PropertiesPanel::value_display_text(&PropertyValue::Number(std::f64::consts::PI)),
            "3.14"
        );
        assert_eq!(PropertiesPanel::value_display_text(&PropertyValue::Bool(true)), "true");
        assert_eq!(PropertiesPanel::value_display_text(&PropertyValue::Bool(false)), "false");
        assert_eq!(
            PropertiesPanel::value_display_text(&PropertyValue::File(
                "/path/to/file.txt".to_string()
            )),
            "/path/to/file.txt"
        );

        let choice_val = PropertyValue::Choice {
            options: vec!["Option A".to_string(), "Option B".to_string()],
            selected: 1,
        };
        assert_eq!(PropertiesPanel::value_display_text(&choice_val), "Option B");
    }

    #[test]
    fn properties_panel_bool_toggle_on_click() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_bool_property("Enabled", true, None));

        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&count);
        panel.property_changed.connect(move |_: Arc<(String, PropertyValue)>| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Click on the Enabled row (row 0, after category header).
        // The layout is: the category header (General) occupies visual row 0, so the property
        // row is visual row 1, drawn at `content.y + ROW_HEIGHT`. The press must land on the
        // *value column* of that row — the name column is a label and headers are separators,
        // neither of which is editable.
        let first_entry = panel.row_rect_for(1);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(VALUE_COL_LEFT + 4, first_entry.y + first_entry.height as i32 / 2),
            button: 1,
        });

        // The bool should have been toggled
        let val = panel.get_property_value("Enabled").unwrap();
        match val {
            PropertyValue::Bool(b) => assert!(!b, "Bool should be toggled to false"),
            _ => panic!("Expected Bool"),
        }
        assert_eq!(count.load(Ordering::SeqCst), 1, "Signal should fire on toggle");
    }

    /// A disabled panel must not edit its properties.
    ///
    /// The click handler never consulted `is_enabled()`, so a panel suspended with
    /// `set_enabled(false)` — the natural way for a caller to make it read-only — still
    /// toggled values and still emitted `property_changed`. The signal matters most:
    /// a host listening for changes would persist a value the user was not allowed to
    /// set.
    #[test]
    fn properties_panel_disabled_ignores_edits() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_bool_property("Enabled", true, None));
        panel.set_enabled(false);

        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&count);
        panel.property_changed.connect(move |_: Arc<(String, PropertyValue)>| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Aim at the box the draw pass actually produced for the entry row, so this test keeps
        // pressing the painted row even if the panel's margin or row height changes.
        let entry_row = panel.row_rect_for(1);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(VALUE_COL_LEFT + 4, entry_row.y + entry_row.height as i32 / 2),
            button: 1,
        });

        let val = panel.get_property_value("Enabled").unwrap();
        match val {
            PropertyValue::Bool(b) => assert!(b, "a disabled panel must not toggle the value"),
            _ => panic!("Expected Bool"),
        }
        assert_eq!(count.load(Ordering::SeqCst), 0, "a disabled panel must not emit changes");

        // Re-enabling restores editing, so the gate suspends rather than locks.
        panel.set_enabled(true);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(VALUE_COL_LEFT + 4, entry_row.y + entry_row.height as i32 / 2),
            button: 1,
        });
        match panel.get_property_value("Enabled").unwrap() {
            PropertyValue::Bool(b) => assert!(!b, "re-enabling must restore editing"),
            _ => panic!("Expected Bool"),
        }
    }

    /// The box a press resolves to must be the box that was painted for that row.
    ///
    /// The draw pass and the hit test used to walk two independent `y` cursors, and they
    /// disagreed about the panel's own margin: drawing started from `geom` inset by
    /// `PANEL_INSET`, hit testing from the bare `geom.y`. Every click was therefore offset
    /// from the row it appeared to hit by the frame margin — small, but enough to miss the
    /// bottom sliver of a row. Both now read `row_rect_for`, so this pins the two together.
    #[test]
    fn the_row_a_press_resolves_to_is_the_row_that_was_painted() {
        let panel = PropertiesPanel::new(Rect::new(10, 20, 300, 400));
        let content = panel.content_box();

        // The content box sits inside the panel's own box: ink must not collide with the
        // frame's stroke, and the rows inherit that margin.
        assert!(content.x > 10, "the content box must be inset from the frame");
        assert!(content.y > 20, "the content box must be inset from the frame");
        assert_eq!(content.x, panel.row_rect_for(0).x);
        assert_eq!(content.y, panel.row_rect_for(0).y);

        // Rows tile exactly, with no gap and no overlap.
        for index in 0..5 {
            let row = panel.row_rect_for(index);
            let next = panel.row_rect_for(index + 1);
            assert_eq!(row.y + ROW_HEIGHT as i32, next.y, "rows must tile without gaps");
            assert_eq!(row.height, ROW_HEIGHT);
            assert_eq!(row.width, content.width);
        }
    }

    /// A scrolled row keeps the same relation between its painted box and its hit box.
    ///
    /// `row_rect_for` is a pure function of the index and the offset, so scrolling only slides
    /// the rows; it must not change which property a given box belongs to. The old pair of
    /// cursors each applied the offset separately, which is exactly how they could drift.
    #[test]
    fn scrolling_slides_every_row_by_the_same_amount() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        let unscrolled = panel.row_rect_for(3);

        panel.set_scroll_offset(ROW_HEIGHT as i32 * 2);
        let scrolled = panel.row_rect_for(3);

        assert_eq!(
            unscrolled.y - scrolled.y,
            ROW_HEIGHT as i32 * 2,
            "the scroll offset must move a row by exactly the offset"
        );
        assert_eq!(unscrolled.x, scrolled.x, "scrolling must not move rows sideways");
        assert_eq!(unscrolled.width, scrolled.width);

        // The first visible row is the one the offset has pushed into the content box, so no
        // row that is entirely above it is ever offered for painting or hitting.
        assert_eq!(panel.first_visible_row(), 2);
    }

    #[test]
    fn properties_panel_svg_output() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 200));
        panel.add_property(make_text_property("Name", "Alice", Some("General")));
        panel.add_property(make_bool_property("Enabled", true, Some("Behavior")));
        panel.add_property(make_choice_property(
            "Theme",
            vec!["Light".to_string(), "Dark".to_string(), "System".to_string()],
            0,
            Some("Appearance"),
        ));

        let svg = render_to_svg(&mut panel);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }
}
