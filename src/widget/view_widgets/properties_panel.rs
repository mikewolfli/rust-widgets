//! PropertiesPanel widget — a categorized property editor panel.
//!
//! Similar to VS Code properties view or Qt QTreeView with property delegates.
//! Properties are grouped by category and rendered as a grid with name-value pairs.
//! Supports filter text for search, editable values (text, number, bool, color,
//! choice, file), and emits `property_changed` on edits.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::collections::HashMap;

/// Property value type for the property panel.
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

    /// Returns the text representation of a property value for display.
    fn value_display_text(value: &PropertyValue) -> String {
        match value {
            PropertyValue::Text(s) => s.clone(),
            PropertyValue::Number(n) => {
                if *n == n.floor() && n.is_finite() {
                    format!("{}", *n as i64)
                } else {
                    format!("{:.2}", n)
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
}

impl Draw for PropertiesPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();

        // ── Background ──
        context.fill_rect(geom, Color::BACKGROUND);

        let font = crate::core::Font::simple("sans-serif", 12.0);
        let categories = self.properties_by_category();
        let mut y = geom.y - self.scroll_offset;

        for (category, entries) in &categories {
            // ── Category header ──
            let header_rect = Rect::new(geom.x, y, geom.width, ROW_HEIGHT);
            context.fill_rect(header_rect, Color::LIGHT_GRAY);
            context.draw_text(
                Point::new(
                    header_rect.x + CATEGORY_PADDING,
                    header_rect.y + ROW_HEIGHT as i32 / 2 + 4,
                ),
                category,
                &font,
                Color::DARK_GRAY,
            );
            y += ROW_HEIGHT as i32;

            for entry in entries {
                if y + (ROW_HEIGHT as i32) > geom.y + (geom.height as i32) {
                    break;
                }
                if y + (ROW_HEIGHT as i32) < geom.y {
                    y += ROW_HEIGHT as i32;
                    continue;
                }

                // Alternate row background
                let row_rect = Rect::new(geom.x, y, geom.width, ROW_HEIGHT);
                context.fill_rect(row_rect, Color::WHITE);

                // Property name
                context.draw_text(
                    Point::new(geom.x + NAME_COL_LEFT, y + ROW_HEIGHT as i32 / 2 + 4),
                    &entry.name,
                    &font,
                    Color::FOREGROUND,
                );

                // Property value (with label-style background)
                let value_rect = Rect::new(
                    geom.x + VALUE_COL_LEFT,
                    y,
                    geom.width - VALUE_COL_LEFT as u32,
                    ROW_HEIGHT,
                );
                let display = Self::value_display_text(&entry.value);
                context.fill_rect(value_rect, Color::EXTRA_LIGHT_GRAY);
                context.draw_text(
                    Point::new(value_rect.x + 2, y + ROW_HEIGHT as i32 / 2 + 4),
                    &display,
                    &font,
                    if entry.editable { Color::BLACK } else { Color::GRAY },
                );

                // Draw bottom border line
                context.draw_rect_stroke(
                    Rect::new(geom.x, y + ROW_HEIGHT as i32 - 1, geom.width, 1),
                    Color::DIVIDER,
                    1,
                );

                y += ROW_HEIGHT as i32;
            }
        }

        // Draw border around the entire panel
        context.draw_rect_stroke(geom, Color::BORDER, 1);
    }
}

impl EventHandler for PropertiesPanel {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    let geom = self.geometry();
                    let categories = self.properties_by_category();
                    let mut y = geom.y - self.scroll_offset;

                    for (_category, entries) in &categories {
                        y += ROW_HEIGHT as i32; // skip header
                        for entry in entries {
                            if y + (ROW_HEIGHT as i32) > geom.y + (geom.height as i32) {
                                break;
                            }
                            let row_rect = Rect::new(geom.x, y, geom.width, ROW_HEIGHT);
                            if row_rect.contains_point(*pos) && entry.editable {
                                // Toggle bool on click
                                let prop_name = entry.name.clone();
                                if matches!(entry.value, PropertyValue::Bool(_)) {
                                    let new_val = match &entry.value {
                                        PropertyValue::Bool(b) => PropertyValue::Bool(!b),
                                        _ => unreachable!(),
                                    };
                                    self.set_property_value(&prop_name, new_val);
                                }
                                self.base.request_redraw();
                                return;
                            }
                            y += ROW_HEIGHT as i32;
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

        // Click on the Enabled row (row 0, after category header)
        // The layout is: y = geom.y - scroll_offset (0 - 0 = 0)
        // First there's a category header (General) at y=0, then the property row at y=ROW_HEIGHT
        panel.handle_event(&Event::MousePress {
            pos: Point::new(VALUE_COL_LEFT + 4, ROW_HEIGHT as i32 + ROW_HEIGHT as i32 / 2),
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
