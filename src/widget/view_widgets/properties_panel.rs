// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PropertiesPanel widget — a categorized property editor panel.
//!
//! Similar to the VS Code properties view: a categorized editor with property delegates.
//! Properties are grouped by category and rendered as a grid with name-value pairs.
//! Supports filter text for search, editable values (text, number, bool, color,
//! choice, file), and emits `property_changed` on edits.

use crate::compat::Vec;
use crate::core::{Color, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::{Ref, RefCell};
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

/// Where a property's `description` is shown.
///
/// The field has always been documented as "shown as a tooltip" and never painted. A mode rather
/// than a single behaviour because a tooltip is not always reachable — a keyboard-only user cannot
/// hover — and a panel embedded in a narrow inspector may prefer one persistent line to a floating
/// box that covers its neighbours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DescriptionMode {
    /// A floating box beside the hovered row.
    #[default]
    Tooltip,
    /// A persistent line across the panel's bottom edge.
    StatusBar,
    /// The description is not painted at all.
    Hidden,
}

impl DescriptionMode {
    /// The token this mode is spelled as in a property write.
    pub fn as_str(self) -> &'static str {
        match self {
            DescriptionMode::Tooltip => "tooltip",
            DescriptionMode::StatusBar => "status_bar",
            DescriptionMode::Hidden => "hidden",
        }
    }

    /// Parses the token back, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "tooltip" => DescriptionMode::Tooltip,
            "status_bar" => DescriptionMode::StatusBar,
            "hidden" => DescriptionMode::Hidden,
            _ => return None,
        })
    }
}

/// What gesture opens a text-style editor on a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditTrigger {
    /// A single click on the value column opens the editor.
    #[default]
    SingleClick,
    /// A double click opens the editor; a single click only focuses the row.
    DoubleClick,
}

impl EditTrigger {
    /// The token this trigger is spelled as in a property write.
    pub fn as_str(self) -> &'static str {
        match self {
            EditTrigger::SingleClick => "single_click",
            EditTrigger::DoubleClick => "double_click",
        }
    }

    /// Parses the token back, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "single_click" => EditTrigger::SingleClick,
            "double_click" => EditTrigger::DoubleClick,
            _ => return None,
        })
    }
}

/// A typed handle to the entry whose inline editor was opened.
///
/// The payload used to be only the property name, so a subscriber that wanted the entry's value
/// or index had to name-match it back out of `properties()`. This carries the index, the name and
/// a snapshot of the value at open time — the three facts a per-type editor host needs.
/// `editor_opened` publishes this handle.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyEditorHandle {
    /// Index into [`PropertiesPanel::properties`].
    pub index: usize,
    /// The entry's display name.
    pub name: String,
    /// The entry's value when the editor opened.
    pub value: PropertyValue,
}

/// An open row editor: which entry, its scratch buffer, and the caret's character position.
///
/// The caret is a **character** index rather than a byte offset, so left/right move one grapheme
/// for the scripts this panel edits, and inserting a multi-byte character cannot split a `char`.
#[derive(Debug, Clone)]
struct PropertyEdit {
    index: usize,
    buffer: String,
    cursor: usize,
}

/// The open choice drop-down: which entry owns it and which option is highlighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChoicePopup {
    index: usize,
    highlighted: usize,
}

/// The open colour swatch palette: which entry owns it and which swatch is highlighted.
///
/// A colour is edited by picking a swatch rather than by typing a hex string. The palette is the
/// active theme's semantic colours ([`color_palette`]), so the editor offers the same colours the
/// rest of the UI is built from instead of an arbitrary colour wheel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ColorPopup {
    index: usize,
    highlighted: usize,
}

/// Swatches per palette row.
const COLOR_POPUP_COLUMNS: usize = 4;
/// A swatch's cell size, in pixels.
const COLOR_SWATCH_SIZE: u32 = 24;
/// The gap between swatches, also used as the palette's inner margin.
const COLOR_SWATCH_GAP: u32 = 4;

/// The swatches an inline colour editor offers.
///
/// The theme palette when one is active, and a fixed semantic set otherwise, so a colour can be
/// edited on a build with no theme. A value already in the palette is highlighted when the editor
/// opens; one outside it starts on the first swatch.
fn color_palette() -> Vec<Color> {
    match crate::style::palette_shared() {
        Some(colors) => vec![
            colors.primary,
            colors.secondary,
            colors.accent,
            colors.success,
            colors.warning,
            colors.error,
            colors.info,
            colors.foreground,
        ],
        None => vec![
            Color::rgb(33, 150, 243),
            Color::rgb(156, 39, 176),
            Color::rgb(0, 150, 136),
            Color::rgb(76, 175, 80),
            Color::rgb(255, 152, 0),
            Color::rgb(244, 67, 54),
            Color::rgb(3, 169, 244),
            Color::rgb(33, 33, 33),
        ],
    }
}

/// The palette index under `pos`, or `None` when the pointer is over a gap or outside the grid.
fn color_swatch_at(rect: Rect, pos: crate::core::Point) -> Option<usize> {
    let stride = COLOR_SWATCH_SIZE + COLOR_SWATCH_GAP;
    let inner_x = pos.x - (rect.x + COLOR_SWATCH_GAP as i32);
    let inner_y = pos.y - (rect.y + COLOR_SWATCH_GAP as i32);
    if inner_x < 0 || inner_y < 0 {
        return None;
    }
    let column = inner_x as u32 / stride;
    let row = inner_y as u32 / stride;
    if column as usize >= COLOR_POPUP_COLUMNS || (inner_x as u32 % stride) >= COLOR_SWATCH_SIZE {
        return None;
    }
    if (inner_y as u32 % stride) >= COLOR_SWATCH_SIZE {
        return None;
    }
    let index = row as usize * COLOR_POPUP_COLUMNS + column as usize;
    if index < color_palette().len() {
        Some(index)
    } else {
        None
    }
}

impl PropertyEdit {
    fn new(index: usize, text: String) -> Self {
        let cursor = text.chars().count();
        Self { index, buffer: text, cursor }
    }

    /// The buffer with a caret marker at the cursor, for the single-run fitted draw.
    fn with_caret(&self) -> String {
        let caret = self.byte_offset();
        let mut rendered = String::with_capacity(self.buffer.len() + 1);
        rendered.push_str(&self.buffer[..caret]);
        rendered.push('|');
        rendered.push_str(&self.buffer[caret..]);
        rendered
    }

    fn byte_offset(&self) -> usize {
        self.buffer.char_indices().nth(self.cursor).map_or(self.buffer.len(), |(offset, _)| offset)
    }

    /// Inserts the committed text at the caret and advances it past the insertion.
    fn insert(&mut self, text: &str) {
        let offset = self.byte_offset();
        self.buffer.insert_str(offset, text);
        self.cursor += text.chars().count();
    }

    /// Removes the character before the caret.
    fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let end = self.byte_offset();
        let start = self.buffer.char_indices().nth(self.cursor - 1).map_or(0, |(offset, _)| offset);
        self.buffer.replace_range(start..end, "");
        self.cursor -= 1;
        true
    }

    /// Removes the character after the caret.
    fn delete(&mut self) -> bool {
        let start = self.byte_offset();
        let Some((_, next)) = self.buffer[start..].char_indices().next() else {
            return false;
        };
        self.buffer.replace_range(start..start + next.len_utf8(), "");
        true
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        if self.cursor < self.buffer.chars().count() {
            self.cursor += 1;
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.buffer.chars().count();
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

/// Height of the description status bar, in pixels.
const STATUS_BAR_HEIGHT: u32 = 22;

/// Vertical allowance of the floating description tooltip.
const TOOLTIP_LINE_HEIGHT: u32 = 18;

/// How long the pointer must rest on a row before its tooltip appears, in milliseconds.
const TOOLTIP_DELAY_MS: u32 = 450;

/// Padding around the tooltip's text.
const TOOLTIP_PADDING: i32 = 6;

/// Width of the inline swatch/checkbox column that precedes a typed value's text.
const TYPE_INDICATOR_WIDTH: u32 = 18;

/// The trailing browse affordance of a file value cell.
fn file_browse_rect(value_rect: Rect) -> Rect {
    let width = 18.min(value_rect.width);
    Rect::new(
        value_rect.x + value_rect.width as i32 - width as i32,
        value_rect.y,
        width,
        value_rect.height,
    )
}

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
    /// Category grouping as indices into `properties`, rebuilt only when the
    /// entry list or the filter changes.
    ///
    /// The draw pass and the hit test used to call `properties_by_category`,
    /// which cloned every category name and allocated a `HashMap` plus one
    /// `Vec` per category on every frame. The grouping is stable between edits,
    /// so it is cached here and rebuilt on the mutations that can change it.
    grouped_cache: RefCell<Option<Vec<(String, Vec<usize>)>>>,
    /// Display text per entry (same index as `properties`), rebuilt when a
    /// value changes.
    ///
    /// `value_display_text` allocated a `String` per visible row per frame;
    /// formatting is now done once per value change instead of once per paint.
    display_cache: RefCell<Option<Vec<String>>>,
    /// The entry index the pointer is over, or `None`.
    ///
    /// Stored as an entry index rather than a visual row so a scroll cannot
    /// leave the highlight on whatever visual row the entry used to occupy;
    /// category headers are separators and are never hovered.
    hovered_entry: Option<usize>,
    /// The entry being edited, its scratch buffer and the caret.
    editing: Option<PropertyEdit>,
    /// The entry carrying keyboard focus.
    focused_entry: Option<usize>,
    /// Where the entry's `description` is painted.
    description_mode: DescriptionMode,
    /// What gesture opens a text-style editor.
    edit_trigger: EditTrigger,
    /// The last pointer position over the panel, used to anchor the tooltip.
    hover_pos: Option<crate::core::Point>,
    /// How long the pointer has rested on `hovered_entry`, in milliseconds.
    hover_elapsed_ms: u32,
    /// Emitted with a typed handle when a row's editor opens.
    pub editor_opened: Signal1<PropertyEditorHandle>,
    /// The open choice drop-down, if any.
    choice_popup: Option<ChoicePopup>,
    /// The open colour swatch palette, if any.
    color_popup: Option<ColorPopup>,
    /// Emitted with the property name when the user asks to browse for a file value.
    ///
    /// A panel cannot open a native file dialog itself, so this is the request a host answers.
    /// The value itself is still edited inline (the path is text), which keeps the control usable
    /// without a host that implements a picker.
    ///
    /// # Capability note
    ///
    /// This signal is **not** in the published capability event list yet (the owned task forbids
    /// editing capability files). Rust hosts can subscribe directly; `connect_event` cannot reach
    /// it until the schema adds `file_picker_requested`.
    pub file_picker_requested: Signal1<String>,
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
            grouped_cache: RefCell::new(None),
            display_cache: RefCell::new(None),
            hovered_entry: None,
            editing: None,
            focused_entry: None,
            description_mode: DescriptionMode::default(),
            edit_trigger: EditTrigger::default(),
            hover_pos: None,
            hover_elapsed_ms: 0,
            editor_opened: Signal1::new(),
            choice_popup: None,
            color_popup: None,
            file_picker_requested: Signal1::new(),
        }
    }

    /// Drops the cached grouping. Called by every mutation that can change
    /// which entries are shown or which category they belong to.
    fn invalidate_grouping(&mut self) {
        *self.grouped_cache.get_mut() = None;
        // A structural/filter change can move or delete the entry the drop-down was opened on,
        // so the popup cannot outlive it. The value it would have committed may now belong to
        // another property.
        self.choice_popup = None;
        self.color_popup = None;
    }

    /// Drops the cached display texts. Called whenever a value changes.
    fn invalidate_display(&mut self) {
        *self.display_cache.get_mut() = None;
    }

    // ── Property management ──

    /// Adds a property entry to the panel.
    pub fn add_property(&mut self, entry: PropertyEntry) {
        self.properties.push(entry);
        self.invalidate_grouping();
        self.invalidate_display();
        self.base.request_redraw();
    }

    /// Removes a property by name. Returns true if found and removed.
    pub fn remove_property(&mut self, name: &str) -> bool {
        let len_before = self.properties.len();
        self.properties.retain(|p| p.name != name);
        let removed = self.properties.len() < len_before;
        if removed {
            self.invalidate_grouping();
            self.invalidate_display();
            self.base.request_redraw();
        }
        removed
    }

    /// Removes all properties.
    pub fn clear_properties(&mut self) {
        self.properties.clear();
        self.invalidate_grouping();
        self.invalidate_display();
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
                self.invalidate_display();
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
        if self.filter_text == text {
            return;
        }
        self.filter_text = text.to_string();
        self.scroll_offset = 0;
        self.invalidate_grouping();
        self.base.request_redraw();
    }

    /// Returns the current filter text.
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    /// Returns the filtered properties that match the current filter text.
    ///
    /// Test-only: the draw path and the public accessor read the cached grouping instead, so
    /// this helper's per-call `Vec` is confined to the tests that assert filter semantics.
    #[cfg(test)]
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
    ///
    /// The grouping is cached; this accessor materialises borrowed entries for
    /// callers outside the draw path. The draw pass and the hit test read
    /// [`Self::grouped_indices`] directly and allocate nothing.
    pub fn properties_by_category(&self) -> Vec<(String, Vec<&PropertyEntry>)> {
        self.grouped_indices()
            .iter()
            .map(|(category, indices)| {
                (
                    category.clone(),
                    indices.iter().filter_map(|&index| self.properties.get(index)).collect(),
                )
            })
            .collect()
    }

    /// The cached category grouping, as indices into `properties`.
    ///
    /// Rebuilt on the first read after [`Self::invalidate_grouping`]. Categories
    /// are sorted by name and entries keep their insertion order within a
    /// category, which is the same shape the previous per-frame HashMap build
    /// produced (the map was drained and sorted, and each bucket was filled in
    /// insertion order).
    fn grouped_indices(&self) -> Ref<'_, Vec<(String, Vec<usize>)>> {
        if self.grouped_cache.borrow().is_none() {
            let lower = (!self.filter_text.is_empty()).then(|| self.filter_text.to_lowercase());
            let mut grouped: HashMap<String, Vec<usize>> = HashMap::new();
            for (index, entry) in self.properties.iter().enumerate() {
                if let Some(lower) = lower.as_ref() {
                    if !entry.name.to_lowercase().contains(lower) {
                        continue;
                    }
                }
                let category = entry.category.clone().unwrap_or_else(|| "General".to_string());
                grouped.entry(category).or_default().push(index);
            }
            let mut result: Vec<_> = grouped.into_iter().collect();
            result.sort_by(|a, b| a.0.cmp(&b.0));
            *self.grouped_cache.borrow_mut() = Some(result);
        }
        Ref::map(self.grouped_cache.borrow(), |cache| {
            cache.as_ref().expect("grouping was rebuilt above")
        })
    }

    /// The cached display text for every entry, indexed like `properties`.
    fn display_texts(&self) -> Ref<'_, Vec<String>> {
        if self.display_cache.borrow().is_none() {
            let texts = self
                .properties
                .iter()
                .map(|entry| Self::value_display_text(&entry.value))
                .collect();
            *self.display_cache.borrow_mut() = Some(texts);
        }
        Ref::map(self.display_cache.borrow(), |cache| {
            cache.as_ref().expect("display texts were rebuilt above")
        })
    }

    // ── Scroll ──

    /// Sets the scroll offset.
    pub fn set_scroll_offset(&mut self, offset: i32) {
        let offset = offset.max(0);
        if self.scroll_offset == offset {
            return;
        }
        self.scroll_offset = offset;
        self.base.request_redraw();
    }

    /// Returns the scroll offset.
    pub fn scroll_offset(&self) -> i32 {
        self.scroll_offset
    }

    // ── Description and edit policy ──

    /// Where property descriptions are painted.
    pub fn description_mode(&self) -> DescriptionMode {
        self.description_mode
    }

    /// Sets where property descriptions are painted.
    pub fn set_description_mode(&mut self, mode: DescriptionMode) {
        if self.description_mode == mode {
            return;
        }
        self.description_mode = mode;
        // The status bar changes the content box, so the rows and the hit test must relayout.
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// What gesture opens a text-style editor.
    pub fn edit_trigger(&self) -> EditTrigger {
        self.edit_trigger
    }

    /// Sets what gesture opens a text-style editor.
    pub fn set_edit_trigger(&mut self, trigger: EditTrigger) {
        self.edit_trigger = trigger;
    }

    /// The description of the entry under the pointer, when it has one.
    pub fn hovered_description(&self) -> Option<&str> {
        self.description_of(self.hovered_entry.or(self.focused_entry))
    }

    /// Whether a delayed tooltip is still waiting to appear.
    fn tooltip_pending(&self) -> bool {
        self.description_mode == DescriptionMode::Tooltip
            && self.hover_elapsed_ms < TOOLTIP_DELAY_MS
            && self.description_of(self.hovered_entry).is_some()
    }

    /// The description of `entry`, when one exists.
    fn description_of(&self, entry: Option<usize>) -> Option<&str> {
        let entry = self.properties.get(entry?)?;
        entry.description.as_deref().filter(|text| !text.is_empty())
    }

    // ── Row geometry ──

    /// The area a row may occupy: the panel's box minus the margin that keeps ink off the
    /// frame's own stroke. Both the draw pass and the hit test start from this, so a row's
    /// column stops coincide with the clickable box by construction.
    fn content_box(&self) -> Rect {
        let inset = ControlMetrics::band_inset(self.geometry(), PANEL_INSET);
        let status = self.status_bar_height();
        if status == 0 {
            return inset;
        }
        Rect::new(inset.x, inset.y, inset.width, inset.height.saturating_sub(status))
    }

    /// The description status bar's own box, when that mode is active.
    fn status_bar_rect(&self) -> Option<Rect> {
        let height = self.status_bar_height();
        if height == 0 {
            return None;
        }
        let inset = ControlMetrics::band_inset(self.geometry(), PANEL_INSET);
        let y = inset.y + inset.height as i32 - height as i32;
        Some(Rect::new(inset.x, y, inset.width, height))
    }

    /// How much of a `StatusBar` description mode reserves at the panel's foot.
    fn status_bar_height(&self) -> u32 {
        match self.description_mode {
            DescriptionMode::StatusBar => STATUS_BAR_HEIGHT.min(self.geometry().height / 2),
            _ => 0,
        }
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
        self.grouped_indices().iter().map(|(_, entries)| 1 + entries.len()).sum()
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
    fn toggle_editable_row(&mut self, row_rect: Rect, pos: crate::core::Point, double_click: bool) {
        let Some(index) = self.editable_index_at(row_rect, pos) else { return };
        self.focused_entry = Some(index);
        let Some(entry) = self.properties.get(index).cloned() else { return };
        // A row that needs a text editor honours the configured trigger; the two one-click
        // controls (a boolean and a choice) stay immediate, because toggling them *is* the value
        // edit and there is no editor to open.
        let editor_allowed = match self.edit_trigger {
            EditTrigger::SingleClick => true,
            EditTrigger::DoubleClick => double_click,
        };
        match entry.value.clone() {
            PropertyValue::Bool(value) => {
                self.commit_value(index, PropertyValue::Bool(!value));
            }
            PropertyValue::Choice { options, .. } if !options.is_empty() => {
                // A choice is edited in a drop-down, not by cycling: cycling made the value an
                // implicit ordered list, so a user could not pick non-adjacent options directly.
                self.open_choice_popup(index);
            }
            // A choice with no options has nothing to cycle to; the guarded arm above does not
            // count towards exhaustiveness.
            PropertyValue::Choice { .. } => {}
            PropertyValue::Text(text) => {
                if editor_allowed {
                    self.begin_edit(index, text);
                }
            }
            PropertyValue::Number(value) => {
                if editor_allowed {
                    self.begin_edit(index, value.to_string());
                }
            }
            // A colour is a swatch: the editor is a palette of the theme's semantic colours, so
            // picking a value is a click rather than typing a hex spelling. A path keeps its text
            // editor and its trailing browse affordance.
            PropertyValue::Color(_) => {
                if editor_allowed {
                    self.open_color_popup(index);
                }
            }
            PropertyValue::File(path) => {
                // The trailing ellipsis asks the host for a picker; the rest of the cell opens
                // the path editor, so the control still works without a picker host.
                let value_rect = self.value_rect_for(row_rect);
                if file_browse_rect(value_rect).contains_point(pos) {
                    let name = self.properties[index].name.clone();
                    self.focused_entry = Some(index);
                    self.file_picker_requested.emit(name);
                } else if editor_allowed {
                    self.begin_edit(index, path);
                }
            }
        }
        self.base.request_redraw();
    }

    /// The value cell of a row, shared by the draw pass and the hit tests.
    fn value_rect_for(&self, row_rect: Rect) -> Rect {
        let content = self.content_box();
        Rect::new(
            content.x + VALUE_COL_LEFT,
            row_rect.y,
            content.width.saturating_sub(VALUE_COL_LEFT as u32),
            ROW_HEIGHT,
        )
    }

    /// Opens the choice drop-down for `index`, returning whether the entry has options.
    pub fn open_choice_popup(&mut self, index: usize) -> bool {
        let Some(entry) = self.properties.get(index) else {
            return false;
        };
        if !entry.editable {
            return false;
        }
        let selected = match &entry.value {
            PropertyValue::Choice { options, selected } if !options.is_empty() => *selected,
            _ => return false,
        };
        self.editing = None;
        self.color_popup = None;
        self.choice_popup = Some(ChoicePopup { index, highlighted: selected });
        self.focused_entry = Some(index);
        self.emit_editor_opened(index);
        self.base.request_redraw();
        true
    }

    /// Closes the choice drop-down without changing the value.
    pub fn close_choice_popup(&mut self) {
        if self.choice_popup.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// The entry whose choice drop-down is open, if any.
    pub fn choice_popup_entry(&self) -> Option<usize> {
        self.choice_popup.map(|popup| popup.index)
    }

    /// The highlighted option of the open drop-down, if any.
    pub fn choice_popup_highlighted(&self) -> Option<usize> {
        self.choice_popup.map(|popup| popup.highlighted)
    }

    /// The drop-down's box, or `None` when it is closed or the entry is no longer a choice.
    fn choice_popup_rect(&self, index: usize) -> Option<Rect> {
        let entry = self.properties.get(index)?;
        let PropertyValue::Choice { options, .. } = &entry.value else {
            return None;
        };
        if options.is_empty() {
            return None;
        }
        let row = self.visual_row_of_entry(index)?;
        let anchor = self.row_rect_for(row);
        let content = self.content_box();
        let width = content.width.saturating_sub(VALUE_COL_LEFT as u32);
        let height = ROW_HEIGHT.saturating_mul(options.len() as u32);
        let below = anchor.y + ROW_HEIGHT as i32;
        let bottom = content.y + content.height as i32;
        // Open downwards when the list fits and upwards when it would fall out of the panel.
        let y = if below + height as i32 <= bottom {
            below
        } else {
            (anchor.y - height as i32).max(content.y)
        };
        Some(Rect::new(content.x + VALUE_COL_LEFT, y, width, height))
    }

    /// The visual row that paints `index`, accounting for headers and the current filter.
    fn visual_row_of_entry(&self, index: usize) -> Option<usize> {
        let mut row = 0usize;
        for (_, indices) in self.grouped_indices().iter() {
            row += 1; // category header
            for &entry in indices {
                if entry == index {
                    return Some(row);
                }
                row += 1;
            }
        }
        None
    }

    /// Handles one key while the choice drop-down is open, returning whether a frame is owed.
    fn handle_choice_key(&mut self, key: u32) -> bool {
        let Some(mut popup) = self.choice_popup else {
            return false;
        };
        let count = match self.properties.get(popup.index).map(|entry| &entry.value) {
            Some(PropertyValue::Choice { options, .. }) if !options.is_empty() => options.len(),
            _ => {
                // The entry disappeared under the open popup (remove_property): close it.
                self.choice_popup = None;
                return true;
            }
        };
        match key {
            crate::event::key_codes::ESCAPE => {
                self.choice_popup = None;
                true
            }
            crate::event::key_codes::UP => {
                popup.highlighted = popup.highlighted.saturating_sub(1);
                self.choice_popup = Some(popup);
                true
            }
            crate::event::key_codes::DOWN => {
                popup.highlighted = popup.highlighted.saturating_add(1).min(count - 1);
                self.choice_popup = Some(popup);
                true
            }
            crate::event::key_codes::HOME => {
                popup.highlighted = 0;
                self.choice_popup = Some(popup);
                true
            }
            crate::event::key_codes::END => {
                popup.highlighted = count - 1;
                self.choice_popup = Some(popup);
                true
            }
            crate::event::key_codes::ENTER | crate::event::key_codes::SPACE => {
                self.select_choice_option(popup.index, popup.highlighted);
                true
            }
            _ => false,
        }
    }

    /// Commits `option` for the open drop-down's entry.
    fn select_choice_option(&mut self, index: usize, option: usize) {
        let Some(entry) = self.properties.get(index) else {
            return;
        };
        let PropertyValue::Choice { options, .. } = &entry.value else {
            return;
        };
        if option >= options.len() {
            return;
        }
        let options = options.clone();
        self.choice_popup = None;
        self.commit_value(index, PropertyValue::Choice { options, selected: option });
        self.base.request_redraw();
    }

    /// Opens the colour palette for `index`, returning whether the entry holds a colour.
    pub fn open_color_popup(&mut self, index: usize) -> bool {
        let Some(entry) = self.properties.get(index) else {
            return false;
        };
        if !entry.editable {
            return false;
        }
        let PropertyValue::Color(color) = &entry.value else {
            return false;
        };
        let palette = color_palette();
        let highlighted = palette.iter().position(|swatch| *swatch == *color).unwrap_or(0);
        self.editing = None;
        self.choice_popup = None;
        self.color_popup = Some(ColorPopup { index, highlighted });
        self.focused_entry = Some(index);
        self.emit_editor_opened(index);
        self.base.request_redraw();
        true
    }

    /// Closes the colour palette without changing the value.
    pub fn close_color_popup(&mut self) {
        if self.color_popup.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// The entry whose colour palette is open, if any.
    pub fn color_popup_entry(&self) -> Option<usize> {
        self.color_popup.map(|popup| popup.index)
    }

    /// The highlighted swatch of the open colour palette, if any.
    pub fn color_popup_highlighted(&self) -> Option<usize> {
        self.color_popup.map(|popup| popup.highlighted)
    }

    /// The palette's box, or `None` when it is closed or the entry is no longer a colour.
    fn color_popup_rect(&self, index: usize) -> Option<Rect> {
        let entry = self.properties.get(index)?;
        if !matches!(&entry.value, PropertyValue::Color(_)) {
            return None;
        }
        let row = self.visual_row_of_entry(index)?;
        let anchor = self.row_rect_for(row);
        let content = self.content_box();
        let width = content.width.saturating_sub(VALUE_COL_LEFT as u32);
        let palette = color_palette();
        let palette_rows = palette.len().div_ceil(COLOR_POPUP_COLUMNS) as u32;
        let height = palette_rows * (COLOR_SWATCH_SIZE + COLOR_SWATCH_GAP) + COLOR_SWATCH_GAP;
        let below = anchor.y + ROW_HEIGHT as i32;
        let bottom = content.y + content.height as i32;
        let y = if below + height as i32 <= bottom {
            below
        } else {
            (anchor.y - height as i32).max(content.y)
        };
        Some(Rect::new(content.x + VALUE_COL_LEFT, y, width, height))
    }

    /// Handles one key while the colour palette is open, returning whether a frame is owed.
    fn handle_color_key(&mut self, key: u32) -> bool {
        let Some(mut popup) = self.color_popup else {
            return false;
        };
        let count = color_palette().len();
        match key {
            crate::event::key_codes::ESCAPE => {
                self.color_popup = None;
                true
            }
            crate::event::key_codes::LEFT | crate::event::key_codes::UP => {
                popup.highlighted = popup.highlighted.saturating_sub(1);
                self.color_popup = Some(popup);
                true
            }
            crate::event::key_codes::RIGHT | crate::event::key_codes::DOWN => {
                popup.highlighted = popup.highlighted.saturating_add(1).min(count - 1);
                self.color_popup = Some(popup);
                true
            }
            crate::event::key_codes::HOME => {
                popup.highlighted = 0;
                self.color_popup = Some(popup);
                true
            }
            crate::event::key_codes::END => {
                popup.highlighted = count - 1;
                self.color_popup = Some(popup);
                true
            }
            crate::event::key_codes::ENTER | crate::event::key_codes::SPACE => {
                self.select_color(popup.index, popup.highlighted);
                true
            }
            _ => false,
        }
    }

    /// Commits palette swatch `swatch` for the open colour palette's entry.
    fn select_color(&mut self, index: usize, swatch: usize) {
        let palette = color_palette();
        let Some(color) = palette.get(swatch).copied() else {
            return;
        };
        self.color_popup = None;
        self.commit_value(index, PropertyValue::Color(color));
        self.base.request_redraw();
    }

    /// Emits `editor_opened` with the typed handle for `index`.
    fn emit_editor_opened(&self, index: usize) {
        if let Some(entry) = self.properties.get(index) {
            self.editor_opened.emit(PropertyEditorHandle {
                index,
                name: entry.name.clone(),
                value: entry.value.clone(),
            });
        }
    }

    /// The entry index whose value column covers `pos`.
    fn editable_index_at(&self, row_rect: Rect, pos: crate::core::Point) -> Option<usize> {
        let content = self.content_box();
        let value_rect = self.value_rect_for(row_rect);
        if !value_rect.contains_point(pos) {
            return None;
        }
        let offset = row_rect.y - content.y + self.scroll_offset;
        if offset < 0 {
            return None;
        }
        let target = offset as usize / ROW_HEIGHT as usize;
        let mut row = 0usize;
        for (_, indices) in self.grouped_indices().iter() {
            if row == target {
                return None;
            }
            row += 1;
            for &index in indices {
                if row == target {
                    return self
                        .properties
                        .get(index)
                        .filter(|entry| entry.editable)
                        .map(|_| index);
                }
                row += 1;
            }
        }
        None
    }

    /// Starts editing an entry with its current text.
    pub fn begin_edit(&mut self, index: usize, text: String) {
        let Some(entry) = self.properties.get(index) else { return };
        if !entry.editable {
            return;
        }
        self.choice_popup = None;
        self.color_popup = None;
        self.editing = Some(PropertyEdit::new(index, text));
        self.focused_entry = Some(index);
        self.emit_editor_opened(index);
        self.base.request_redraw();
    }

    /// Commits the open editor, parsing a number or a colour when the entry holds one.
    pub fn commit_edit(&mut self) -> bool {
        let Some(edit) = self.editing.take() else {
            return false;
        };
        let Some(entry) = self.properties.get(edit.index).cloned() else {
            return false;
        };
        let text = edit.buffer;
        let value = match entry.value {
            PropertyValue::Number(_) => match text.trim().parse::<f64>() {
                Ok(number) => PropertyValue::Number(number),
                // A value that does not parse is refused rather than coerced to zero.
                Err(_) => return false,
            },
            PropertyValue::Color(_) => match Color::parse_hex(&text) {
                Some(color) => PropertyValue::Color(color),
                // A malformed colour is refused for the same reason a malformed number is:
                // silently keeping the old value would look like the edit had applied.
                None => return false,
            },
            PropertyValue::File(_) => PropertyValue::File(text),
            _ => PropertyValue::Text(text),
        };
        self.commit_value(edit.index, value);
        true
    }

    /// Cancels the open editor, discarding its buffer.
    pub fn cancel_edit(&mut self) {
        if self.editing.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether an editor is open.
    pub fn editing_entry(&self) -> Option<usize> {
        self.editing.as_ref().map(|edit| edit.index)
    }

    /// The open editor's caret position, in characters from the start of its buffer.
    pub fn edit_cursor(&self) -> Option<usize> {
        self.editing.as_ref().map(|edit| edit.cursor)
    }

    fn commit_value(&mut self, index: usize, value: PropertyValue) {
        let Some(name) = self.properties.get(index).map(|entry| entry.name.clone()) else {
            return;
        };
        self.set_property_value(&name, value);
    }

    /// The entry index under `pos`, or `None` when the pointer is over a category
    /// header or outside the content box.
    fn entry_index_at(&self, pos: crate::core::Point) -> Option<usize> {
        let content = self.content_box();
        if !content.contains_point(pos) {
            return None;
        }
        let target = (pos.y - content.y + self.scroll_offset) as usize / ROW_HEIGHT as usize;
        let mut row = 0usize;
        for (_, indices) in self.grouped_indices().iter() {
            if row == target {
                return None; // a category header
            }
            row += 1;
            for &index in indices {
                if row == target {
                    return Some(index);
                }
                row += 1;
            }
        }
        None
    }

    /// Paints the open choice drop-down, if any.
    fn draw_choice_popup(
        &self,
        context: &mut RenderContext,
        font: &crate::core::Font,
        background: Color,
        border: Color,
        text_color: Color,
        accent: Color,
    ) {
        let Some(popup) = self.choice_popup else {
            return;
        };
        let Some(rect) = self.choice_popup_rect(popup.index) else {
            return;
        };
        let Some(entry) = self.properties.get(popup.index) else {
            return;
        };
        let PropertyValue::Choice { options, selected } = &entry.value else {
            return;
        };
        context.fill_rect(rect, background.blend(&text_color, 0.16));
        context.draw_rect_stroke(rect, border, 1);
        for (option, label) in options.iter().enumerate() {
            let row = Rect::new(
                rect.x,
                rect.y + option as i32 * ROW_HEIGHT as i32,
                rect.width,
                ROW_HEIGHT,
            );
            if option == popup.highlighted {
                context.fill_rect(row, background.blend(&accent, 0.24));
            }
            let line = context.text_line(row, font);
            context.draw_text_fitted_cow(
                Rect::new(row.x + 4, line.y, row.width.saturating_sub(8), line.height),
                label,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
            if option == *selected {
                // The current value keeps a margin marker independent of the highlight, so the
                // selection is still readable while the pointer moves over other rows.
                context.draw_text(
                    crate::core::Point::new(row.x + row.width as i32 - 14, line.y),
                    "*",
                    font,
                    accent,
                    HorizontalAlignment::Left,
                );
            }
        }
    }

    /// Paints the open colour palette over the rows it covers.
    fn draw_color_popup(
        &self,
        context: &mut RenderContext,
        background: Color,
        border: Color,
        accent: Color,
    ) {
        let Some(popup) = self.color_popup else {
            return;
        };
        let Some(rect) = self.color_popup_rect(popup.index) else {
            return;
        };
        let palette = color_palette();
        context.fill_rect(rect, background.blend(&accent, 0.16));
        context.draw_rect_stroke(rect, border, 1);
        let stride = COLOR_SWATCH_SIZE + COLOR_SWATCH_GAP;
        for (swatch, color) in palette.iter().enumerate() {
            let column = (swatch % COLOR_POPUP_COLUMNS) as i32;
            let row = (swatch / COLOR_POPUP_COLUMNS) as i32;
            let cell = Rect::new(
                rect.x + COLOR_SWATCH_GAP as i32 + column * stride as i32,
                rect.y + COLOR_SWATCH_GAP as i32 + row * stride as i32,
                COLOR_SWATCH_SIZE,
                COLOR_SWATCH_SIZE,
            );
            context.fill_rect(cell, *color);
            // The highlight is a heavier stroke rather than a recolour, so the swatch still reads
            // as its own colour while the pointer moves over it.
            let (stroke, width) =
                if swatch == popup.highlighted { (accent, 2) } else { (border, 1) };
            context.draw_rect_stroke(cell, stroke, width);
        }
    }

    /// Paints the hovered entry's description according to [`Self::description_mode`].
    ///
    /// This is the wiring the `description` field never had: the row loop and the event handler
    /// both skipped it, so the documented "shown as a tooltip" was a comment and nothing more.
    fn draw_description(
        &self,
        context: &mut RenderContext,
        geom: Rect,
        background: Color,
        border: Color,
        text_color: Color,
        font: &crate::core::Font,
    ) {
        match self.description_mode {
            DescriptionMode::Hidden => {}
            DescriptionMode::StatusBar => {
                let Some(bar) = self.status_bar_rect() else { return };
                context.fill_rect(bar, background.blend(&text_color, 0.10));
                context.draw_line(
                    crate::core::Point::new(bar.x, bar.y),
                    crate::core::Point::new(bar.x + bar.width as i32, bar.y),
                    border,
                );
                let text =
                    self.description_of(self.hovered_entry.or(self.focused_entry)).unwrap_or("");
                if !text.is_empty() {
                    context.draw_text_fitted_cow(
                        Rect::new(
                            bar.x + 6,
                            context.text_line(bar, font).y,
                            bar.width.saturating_sub(12),
                            bar.height,
                        ),
                        text,
                        font,
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }
            }
            DescriptionMode::Tooltip => {
                let Some(text) = self.description_of(self.hovered_entry) else { return };
                // A tooltip that appeared on the first hovered frame would flash while the
                // pointer crosses the panel; the delay is what makes it read as an answer to a
                // deliberate rest.
                if self.hover_elapsed_ms < TOOLTIP_DELAY_MS {
                    return;
                }
                let anchor = self.hover_pos.unwrap_or(crate::core::Point::new(geom.x, geom.y));
                let metrics = context.measure_text(text, font);
                let width = (metrics.width + TOOLTIP_PADDING as u32 * 2)
                    .min(geom.width.saturating_sub(4))
                    .max(1);
                let height = TOOLTIP_LINE_HEIGHT.min(geom.height).max(1);
                // Anchor below-right of the pointer, then pull the box back inside the panel so
                // a row near an edge cannot paint its description outside the control.
                let max_x = (geom.x + geom.width as i32 - width as i32).max(geom.x);
                let max_y = (geom.y + geom.height as i32 - height as i32).max(geom.y);
                let x = (anchor.x + 12).clamp(geom.x, max_x);
                let y = (anchor.y + 16).clamp(geom.y, max_y);
                let box_rect = Rect::new(x, y, width, height);
                context.fill_rounded_rect(box_rect, 3, background.blend(&text_color, 0.18));
                context.draw_rect_stroke(box_rect, border, 1);
                context.draw_text_fitted_cow(
                    Rect::new(
                        box_rect.x + TOOLTIP_PADDING,
                        context.text_line(box_rect, font).y,
                        box_rect.width.saturating_sub(TOOLTIP_PADDING as u32 * 2),
                        box_rect.height,
                    ),
                    text,
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
        }
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 400)
    }

    /// Advances the tooltip's hover delay; `true` while the tooltip still owes a frame.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.tooltip_pending() {
            return false;
        }
        self.hover_elapsed_ms = self.hover_elapsed_ms.saturating_add(delta_ms);
        if self.hover_elapsed_ms >= TOOLTIP_DELAY_MS {
            self.base.request_redraw();
            return false;
        }
        true
    }

    fn is_animating(&self) -> bool {
        self.tooltip_pending()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "property_changed" => {
                Some(EventSignalRef::mapped("property_changed", &self.property_changed, |v| {
                    // The declared shape is `Tuple2` of `String`, so the payload arrives as the
                    // property name paired with its value rather than its `Debug` spelling — a
                    // subscriber that read the schema can read the two components by name.
                    let (name, value) = v;
                    CapabilityValue::Tuple(Vec::from([
                        CapabilityValue::String(name.clone()),
                        property_value_to_capability(value),
                    ]))
                }))
            }
            "editor_opened" => {
                // The Rust signal carries a typed [`PropertyEditorHandle`]; the published
                // capability still declares a `String` payload, so the bridge reports the entry
                // name. A schema change to a tuple would be an event-contract edit, which this
                // file does not own.
                Some(EventSignalRef::mapped("editor_opened", &self.editor_opened, |v| {
                    CapabilityValue::String(v.name.clone())
                }))
            }
            "file_picker_requested" => Some(EventSignalRef::mapped(
                "file_picker_requested",
                &self.file_picker_requested,
                |name| CapabilityValue::String(name.clone()),
            )),
            _ => None,
        }
    }
}

/// Converts a [`PropertyValue`] to the string representation shown by the panel.
fn property_value_to_capability(value: &PropertyValue) -> CapabilityValue {
    CapabilityValue::String(PropertiesPanel::value_display_text(value))
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
            "description_mode" => {
                Ok(CapabilityValue::String(self.description_mode().as_str().to_string()))
            }
            "edit_trigger" => Ok(CapabilityValue::String(self.edit_trigger().as_str().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Derived from the entry list, which owns it.
            "property_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "description_mode" => {
                let text = match value {
                    CapabilityValue::String(text) => text,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                let mode =
                    DescriptionMode::from_name(&text).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_description_mode(mode);
                Ok(())
            }
            "edit_trigger" => {
                let text = match value {
                    CapabilityValue::String(text) => text,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                let trigger =
                    EditTrigger::from_name(&text).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_edit_trigger(trigger);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "property_count",
            "description_mode",
            "edit_trigger",
            BASE_PROPERTY_NAMES
        ]
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
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("properties_panel");
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
        // A hovered entry is pointer position, not a committed selection, so it reads the
        // accent at a **lighter** weight than the row background — the same surface/accent
        // pair the table family uses for its row tint, so the two grids hover alike.
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);
        let hovered_bg = background.blend(&accent, 0.12);

        // ── Background ──
        context.fill_rect(geom, background);

        // The panel's own effective font rather than a per-frame `Font::simple` (which also
        // allocated the family `String` and ignored the theme's typeface).
        let font = effective_font(self.style());
        // Both caches are borrowed (not rebuilt) on a steady-state frame: the grouping only
        // changes on add/remove/filter, and the display text only when a value changes.
        let grouped = self.grouped_indices();
        let displays = self.display_texts();
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
        // Rows and headers are painted inside the panel's content box. A partially scrolled row
        // used to paint its full rectangle above the box (over the margin and the border), and a
        // category header below the box still painted after the row loop broke.
        context.push_clip(content.x, content.y, content.width, content.height);

        for (category, indices) in grouped.iter() {
            // ── Category header ──
            let header_rect = self.row_rect_for(row);
            row += 1;
            if header_rect.y >= bottom {
                break;
            }
            context.fill_rect(header_rect, header_background);
            // Centred through the shared primitive. Every row of this panel used to draw at
            // `y + ROW_HEIGHT / 2 + 4`: the `+ 4` was meant to compensate for the top-edge
            // origin but only halved the error, so a 12 px label in a 26 px row occupied
            // `17..29` and its descenders crossed the divider drawn at `y + ROW_HEIGHT - 1`.
            let header_line = context.text_line(header_rect, font);
            context.draw_text_fitted_cow(
                Rect {
                    x: header_rect.x + CATEGORY_PADDING,
                    y: header_line.y,
                    width: header_rect.width.saturating_sub(CATEGORY_PADDING as u32),
                    height: header_line.height,
                },
                category,
                font,
                text_color,
                HorizontalAlignment::Left,
            );

            for &index in indices {
                let Some(entry) = self.properties.get(index) else { continue };
                let row_rect = self.row_rect_for(row);
                row += 1;
                if row_rect.y >= bottom {
                    break;
                }
                if row_rect.y + ROW_HEIGHT as i32 <= content.y {
                    continue;
                }

                // Alternate row background — or the hover tint when the pointer is over this
                // entry. The stored hover is an entry index, so it follows its row through a
                // scroll instead of lighting whatever visual row it used to occupy.
                context.fill_rect(
                    row_rect,
                    if Some(index) == self.hovered_entry { hovered_bg } else { row_background },
                );
                let row_line = context.text_line(row_rect, font);

                // Property name. Guarded on the text being non-empty so a nameless entry
                // emits no `<text …></text>`.
                let name_x = content.x + NAME_COL_LEFT;
                if !entry.name.is_empty() {
                    context.draw_text_fitted_cow(
                        Rect {
                            x: name_x,
                            y: row_line.y,
                            width: (content.x + VALUE_COL_LEFT - name_x).max(0) as u32,
                            height: row_line.height,
                        },
                        &entry.name,
                        font,
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }

                // Property value (with label-style background)
                let value_rect = self.value_rect_for(row_rect);
                let display = match self.editing.as_ref() {
                    Some(edit) if edit.index == index => std::borrow::Cow::Owned(edit.with_caret()),
                    _ => std::borrow::Cow::Borrowed(
                        displays.get(index).map(String::as_str).unwrap_or(""),
                    ),
                };
                context.fill_rect(value_rect, value_background);

                // A per-type indicator column turns the value cell into the right *kind* of
                // editor affordance: a swatch for a colour, a check box for a bool, a caret for
                // a choice and an ellipsis for a file.
                let indicator = Rect::new(
                    value_rect.x + 2,
                    value_rect.y + (ROW_HEIGHT.saturating_sub(14) / 2) as i32,
                    TYPE_INDICATOR_WIDTH.saturating_sub(4),
                    14,
                );
                let mut text_x = value_rect.x + 2;
                let mut text_width = value_rect.width.saturating_sub(4);
                match &entry.value {
                    PropertyValue::Color(color) => {
                        let swatch = Rect::new(
                            indicator.x,
                            indicator.y,
                            TYPE_INDICATOR_WIDTH.saturating_sub(4),
                            14,
                        );
                        context.fill_rect(swatch, *color);
                        context.draw_rect_stroke(swatch, border, 1);
                        text_x = swatch.x + swatch.width as i32 + 4;
                        text_width = text_width.saturating_sub(TYPE_INDICATOR_WIDTH);
                    }
                    PropertyValue::Bool(on) => {
                        context.draw_rect_stroke(indicator, border, 1);
                        if *on {
                            context.fill_rect(
                                Rect::new(
                                    indicator.x + 3,
                                    indicator.y + 3,
                                    indicator.width.saturating_sub(6),
                                    indicator.height.saturating_sub(6),
                                ),
                                accent,
                            );
                        }
                        text_x = indicator.x + indicator.width as i32 + 4;
                        text_width = text_width.saturating_sub(TYPE_INDICATOR_WIDTH);
                    }
                    PropertyValue::Choice { .. } => {
                        // A down caret on the trailing edge, matching the drop-down it opens.
                        let caret_x = value_rect.x + value_rect.width as i32 - 12;
                        let caret_y = value_rect.y + ROW_HEIGHT as i32 / 2;
                        context.draw_line(
                            crate::core::Point::new(caret_x - 4, caret_y - 2),
                            crate::core::Point::new(caret_x, caret_y + 2),
                            text_color,
                        );
                        context.draw_line(
                            crate::core::Point::new(caret_x, caret_y + 2),
                            crate::core::Point::new(caret_x + 4, caret_y - 2),
                            text_color,
                        );
                        text_width = text_width.saturating_sub(14);
                    }
                    PropertyValue::File(_) => {
                        let browse = file_browse_rect(value_rect);
                        context.draw_rect_stroke(browse, border, 1);
                        context.draw_text_fitted_cow(
                            Rect::new(browse.x + 2, browse.y, browse.width.saturating_sub(4), 18),
                            "...",
                            font,
                            text_color,
                            HorizontalAlignment::Center,
                        );
                        text_width = text_width.saturating_sub(browse.width + 2);
                    }
                    PropertyValue::Text(_) | PropertyValue::Number(_) => {}
                }

                let value_line = context.text_line(value_rect, font);
                if !display.is_empty() {
                    context.draw_text_fitted_cow(
                        Rect {
                            x: text_x,
                            y: value_line.y,
                            width: text_width,
                            height: value_line.height,
                        },
                        &display,
                        font,
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
        context.pop_clip();
        context.draw_rect_stroke(geom, border, 1);

        // The choice drop-down is painted last so it sits over the rows it covers.
        self.draw_choice_popup(context, font, background, border, text_color, accent);
        self.draw_color_popup(context, background, border, accent);

        self.draw_description(context, geom, background, border, text_color, font);
    }
}

impl EventHandler for PropertiesPanel {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, button, .. } => {
                // A disabled panel must not edit values. Without this gate
                // `set_enabled(false)` left the rows clickable, so a panel put into a
                // read-only state still mutated its properties.
                if !self.base.is_enabled() {
                    self.base.handle_event(event);
                    return;
                }
                if *button == 1 {
                    // An open popup owns the press: a cell chooses a value, anywhere else
                    // dismisses it. This runs before the row walk so a press can never open a
                    // second editor underneath the popup.
                    if let Some(popup) = self.color_popup {
                        if let Some(rect) = self.color_popup_rect(popup.index) {
                            if rect.contains_point(*pos) {
                                if let Some(swatch) = color_swatch_at(rect, *pos) {
                                    self.select_color(popup.index, swatch);
                                } else {
                                    self.close_color_popup();
                                }
                                return;
                            }
                        }
                        self.close_color_popup();
                        return;
                    }
                    if let Some(popup) = self.choice_popup {
                        if let Some(rect) = self.choice_popup_rect(popup.index) {
                            if rect.contains_point(*pos) {
                                let option =
                                    ((pos.y - rect.y).max(0) as usize) / ROW_HEIGHT as usize;
                                self.select_choice_option(popup.index, option);
                                return;
                            }
                        }
                        self.close_choice_popup();
                        return;
                    }
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
                            self.toggle_editable_row(row_rect, *pos, false);
                            return;
                        }
                    }
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                // A double click while a popup is open is two presses on the popup, not a
                // second editor request.
                if self.choice_popup.is_some() || self.color_popup.is_some() {
                    return;
                }
                // The second press of a double click is delivered separately as a `MousePress`,
                // so this arm only opens the editor; `toggle_editable_row` owns the trigger
                // decision and the by-type dispatch.
                let rows = self.visual_row_count();
                let mut row = self.first_visible_row();
                while row < rows {
                    let row_rect = self.row_rect_for(row);
                    row += 1;
                    if row_rect.contains_point(*pos) {
                        self.toggle_editable_row(row_rect, *pos, true);
                        return;
                    }
                }
            }
            Event::MouseMove { pos, .. } => {
                // A disabled panel is read-only, so it shows no hover affordance either —
                // the same gate its press arm applies.
                if self.base.is_enabled() {
                    // Hover is the entry under the pointer, from the same inverse walk the
                    // press uses, so a highlight cannot land on a row a click would not
                    // reach. `MouseMove` fires far more often than the highlight changes,
                    // so the redraw is requested only on an actual change. The position is
                    // recorded either way: the tooltip anchors to it.
                    self.hover_pos = Some(*pos);
                    let hovered = self.entry_index_at(*pos);
                    if hovered != self.hovered_entry {
                        self.hovered_entry = hovered;
                        self.hover_elapsed_ms = 0;
                        self.base.request_redraw();
                    } else if self.description_mode == DescriptionMode::Tooltip && hovered.is_some()
                    {
                        // The box follows the pointer even while it stays over one row.
                        self.base.request_redraw();
                    }
                }
                self.base.handle_event(event);
            }
            Event::MouseLeave { .. } => {
                self.hover_pos = None;
                self.hover_elapsed_ms = 0;
                let popup_closed = self.choice_popup.take().is_some();
                let color_closed = self.color_popup.take().is_some();
                if self.hovered_entry.take().is_some() || popup_closed || color_closed {
                    self.base.request_redraw();
                }
                self.base.handle_event(event);
            }
            Event::Wheel { delta, modifiers: _ } => {
                // One notch is one row: the old `* 3` had no row semantics and scrolled a
                // different distance per host.
                let step = ROW_HEIGHT as i32 * delta.y.signum();
                let max =
                    ((self.visual_row_count().saturating_sub(1)) * ROW_HEIGHT as usize) as i32;
                self.scroll_offset = (self.scroll_offset - step).clamp(0, max.max(0));
                self.base.request_redraw();
            }
            // IME / keyboard-layout committed text. The previous handler accepted raw key codes
            // in `32..=126` only, so a CJK or accented character — which never arrives as one
            // ASCII code point — was dropped on the floor.
            Event::TextInput { text }
                if self.editing.is_some() && text.chars().all(|ch| !ch.is_control()) =>
            {
                if let Some(edit) = self.editing.as_mut() {
                    edit.insert(text);
                }
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } if self.base.focus_reason().is_some() => {
                // An open drop-down owns the keyboard while it is up: arrows move the highlight,
                // Enter/Space commit it and Escape closes it without changing the value.
                if self.choice_popup.is_some() {
                    if self.handle_choice_key(*key) {
                        self.base.request_redraw();
                    }
                    return;
                }
                if self.color_popup.is_some() {
                    if self.handle_color_key(*key) {
                        self.base.request_redraw();
                    }
                    return;
                }
                if self.editing.is_some() {
                    // Commit/cancel take the editor out of the field, so they cannot run inside a
                    // borrow of it; the remaining keys mutate the buffer and report whether the
                    // frame changed, which keeps the redraw outside the borrow as well.
                    let redraw = match *key {
                        crate::event::key_codes::ENTER => {
                            let _ = self.commit_edit();
                            true
                        }
                        crate::event::key_codes::ESCAPE => {
                            self.cancel_edit();
                            false
                        }
                        _ => {
                            let Some(edit) = self.editing.as_mut() else { return };
                            match *key {
                                crate::event::key_codes::BACKSPACE => edit.backspace(),
                                crate::event::key_codes::DELETE => edit.delete(),
                                crate::event::key_codes::LEFT => {
                                    edit.move_left();
                                    true
                                }
                                crate::event::key_codes::RIGHT => {
                                    edit.move_right();
                                    true
                                }
                                crate::event::key_codes::HOME => {
                                    edit.move_home();
                                    true
                                }
                                crate::event::key_codes::END => {
                                    edit.move_end();
                                    true
                                }
                                _ => {
                                    let _ = modifiers;
                                    false
                                }
                            }
                        }
                    };
                    if redraw {
                        self.base.request_redraw();
                    }
                    return;
                }
                let visible = self.visual_row_count();
                if visible == 0 {
                    return;
                }
                let current = self.focused_entry.unwrap_or(0);
                let page = (self.content_box().height / ROW_HEIGHT).max(1) as usize;
                let next = match *key {
                    crate::event::key_codes::UP => current.saturating_sub(1),
                    crate::event::key_codes::DOWN => {
                        current.saturating_add(1).min(self.properties.len().saturating_sub(1))
                    }
                    crate::event::key_codes::HOME => 0,
                    crate::event::key_codes::END => self.properties.len().saturating_sub(1),
                    crate::event::key_codes::PAGE_UP => current.saturating_sub(page),
                    crate::event::key_codes::PAGE_DOWN => {
                        current.saturating_add(page).min(self.properties.len().saturating_sub(1))
                    }
                    crate::event::key_codes::ENTER | crate::event::key_codes::SPACE => {
                        if let Some(entry) = self.properties.get(current).cloned() {
                            if let PropertyValue::Bool(value) = entry.value {
                                self.commit_value(current, PropertyValue::Bool(!value));
                            }
                        }
                        current
                    }
                    _ => current,
                };
                self.focused_entry = Some(next);
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
    use std::sync::{Arc, Mutex};

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
        assert_eq!(
            property_value_to_capability(&choice_val),
            CapabilityValue::String("Option B".to_string())
        );
        assert_eq!(
            property_value_to_capability(&PropertyValue::Color(Color::rgb(12, 34, 56))),
            CapabilityValue::String("#0C2238".to_string())
        );
        assert_eq!(
            property_value_to_capability(&PropertyValue::Choice {
                options: vec!["Option A".to_string()],
                selected: 1,
            }),
            CapabilityValue::String("Invalid".to_string())
        );
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
            modifiers: 0,
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
            modifiers: 0,
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
            modifiers: 0,
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

    /// The grouping cache must not outlive the mutations it depends on (VIEW-N03): adding,
    /// removing and filtering each change what the next frame paints.
    #[test]
    fn grouping_cache_follows_every_structural_mutation() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("alpha", "a", Some("One")));
        assert_eq!(panel.properties_by_category().len(), 1);

        // A second category changes the grouping.
        panel.add_property(make_text_property("beta", "b", Some("Two")));
        let grouped = panel.properties_by_category();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, "One");
        assert_eq!(grouped[1].0, "Two");

        // Filtering shrinks the grouping without touching the property list.
        panel.set_filter("alp");
        let grouped = panel.properties_by_category();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].1.len(), 1);
        assert_eq!(grouped[0].1[0].name, "alpha");

        // Removing drops it from the grouping.
        panel.set_filter("");
        panel.remove_property("alpha");
        let grouped = panel.properties_by_category();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].1[0].name, "beta");
    }

    /// The display cache is keyed on the entries' values, so a value change must rebuild it.
    #[test]
    fn display_cache_reflects_a_value_change() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("greeting", "Hello", None));
        assert_eq!(panel.display_texts().as_slice(), &[String::from("Hello")]);

        panel.set_property_value("greeting", PropertyValue::Text("Hi".to_string()));
        assert_eq!(panel.display_texts().as_slice(), &[String::from("Hi")]);
    }

    /// Hover is the entry under the pointer, resolved through the same row walk the
    /// press uses: headers stay unlit, entries resolve to their index and leaving
    /// the panel clears the highlight instead of freezing it in place.
    #[test]
    fn hover_follows_the_pointer_across_headers_and_entries() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("name", "Alice", Some("General")));
        panel.add_property(make_bool_property("enabled", true, Some("General")));

        // Visual row 0 is the category header — a separator, never hovered.
        let header = panel.row_rect_for(0);
        panel.handle_event(&Event::MouseMove { pos: Point::new(header.x + 10, header.y + 13) });
        assert_eq!(panel.hovered_entry, None);

        // Rows 1 and 2 carry entries 0 and 1 respectively.
        let first = panel.row_rect_for(1);
        panel.handle_event(&Event::MouseMove { pos: Point::new(first.x + 10, first.y + 13) });
        assert_eq!(panel.hovered_entry, Some(0));

        let second = panel.row_rect_for(2);
        panel.handle_event(&Event::MouseMove { pos: Point::new(second.x + 10, second.y + 13) });
        assert_eq!(panel.hovered_entry, Some(1));

        panel.handle_event(&Event::MouseLeave { pos: Point::new(-1, -1) });
        assert_eq!(panel.hovered_entry, None);
    }

    /// A disabled panel is read-only, so it shows no hover affordance either — the
    /// same gate its press arm applies before an edit is accepted.
    #[test]
    fn a_disabled_panel_shows_no_hover() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
        panel.add_property(make_text_property("name", "Alice", Some("General")));
        panel.set_enabled(false);

        let first = panel.row_rect_for(1);
        panel.handle_event(&Event::MouseMove { pos: Point::new(first.x + 10, first.y + 13) });
        assert_eq!(panel.hovered_entry, None);
    }

    /// Text/number editors open on click, commit on Enter, and the wheel moves by rows.
    #[test]
    fn editors_wheel_rows_and_keyboard_are_wired() {
        use crate::event::EventHandler;
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 320, 200));
        panel.add_property(PropertyEntry::new(
            "count",
            PropertyValue::Number(3.0),
            Some("General"),
            None,
            true,
        ));
        let row = panel.row_rect_for(1);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(row.x + VALUE_COL_LEFT + 4, row.y + 13),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(panel.editing_entry(), Some(0), "a click opens the number editor");
        panel.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        panel.handle_event(&Event::text_input("1"));
        panel.handle_event(&Event::KeyPress { key: crate::event::key_codes::ENTER, modifiers: 0 });
        assert_eq!(panel.editing_entry(), None);
        assert!(matches!(
            panel.properties()[0].value,
            PropertyValue::Number(value) if value >= 31.0
        ));

        panel.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert!(panel.scroll_offset >= 0);
        let _ = crate::widget::svg::render_to_svg(&mut panel);
    }

    /// V-21: a choice is edited in a drop-down, and the typed handle is published on open.
    #[test]
    fn a_choice_opens_a_dropdown_and_commits_the_highlighted_option() {
        use crate::event::key_codes::{DOWN, ENTER};
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 320, 240));
        panel.add_property(make_choice_property(
            "mode",
            vec!["Alpha".to_string(), "Beta".to_string(), "Gamma".to_string()],
            0,
            Some("General"),
        ));
        let opened = Arc::new(Mutex::new(None));
        panel.editor_opened.connect({
            let opened = Arc::clone(&opened);
            move |handle: Arc<PropertyEditorHandle>| {
                *opened.lock().unwrap() = Some(handle.as_ref().clone());
            }
        });

        let row = panel.row_rect_for(1);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(row.x + VALUE_COL_LEFT + 4, row.y + 13),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(panel.choice_popup_entry(), Some(0), "a click opens the drop-down");
        let handle = opened.lock().unwrap().clone().expect("editor_opened carries a handle");
        assert_eq!(handle.index, 0);
        assert_eq!(handle.name, "mode");

        panel.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        panel.handle_event(&Event::KeyPress { key: DOWN, modifiers: 0 });
        panel.handle_event(&Event::KeyPress { key: ENTER, modifiers: 0 });
        assert_eq!(panel.choice_popup_entry(), None, "Enter closes the drop-down");
        assert!(matches!(panel.properties()[0].value, PropertyValue::Choice { selected: 1, .. }));
    }

    /// V-21: `edit_trigger` decides which gesture opens a text-style editor.
    #[test]
    fn the_edit_trigger_decides_whether_a_single_click_opens_the_editor() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 320, 240));
        panel.set_edit_trigger(EditTrigger::DoubleClick);
        panel.add_property(make_text_property("name", "ab", Some("General")));
        let row = panel.row_rect_for(1);
        let pos = Point::new(row.x + VALUE_COL_LEFT + 4, row.y + 13);

        panel.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(panel.editing_entry(), None, "a single click only focuses with this trigger");

        panel.handle_event(&Event::MouseDoubleClick { pos, button: 1 });
        assert_eq!(panel.editing_entry(), Some(0), "a double click opens the editor");
    }

    /// V-21: a colour is edited through the theme palette, not by typing a hex string.
    #[test]
    fn a_color_opens_a_swatch_palette_and_commits_the_chosen_swatch() {
        use crate::event::key_codes::{ENTER, RIGHT};
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 320, 240));
        let palette = color_palette();
        panel.add_property(PropertyEntry::new(
            "accent",
            PropertyValue::Color(palette[0]),
            Some("Appearance"),
            None,
            true,
        ));

        let row = panel.row_rect_for(1);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(row.x + VALUE_COL_LEFT + 4, row.y + 13),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(panel.color_popup_entry(), Some(0), "a click opens the palette");
        assert_eq!(panel.editing_entry(), None, "the colour is not text-edited");

        panel.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        panel.handle_event(&Event::KeyPress { key: RIGHT, modifiers: 0 });
        panel.handle_event(&Event::KeyPress { key: ENTER, modifiers: 0 });
        assert_eq!(panel.color_popup_entry(), None, "Enter closes the palette");
        assert_eq!(
            panel.properties()[0].value,
            PropertyValue::Color(palette[1]),
            "the second swatch is committed"
        );
    }

    /// V-21: the browse affordance asks the host for a picker, and the value body stays editable.
    #[test]
    fn a_file_value_requests_a_picker_or_edits_the_path_inline() {
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 320, 240));
        panel.add_property(PropertyEntry::new(
            "source",
            PropertyValue::File("/tmp/a.txt".to_string()),
            Some("General"),
            None,
            true,
        ));
        let requested = Arc::new(Mutex::new(None));
        panel.file_picker_requested.connect({
            let requested = Arc::clone(&requested);
            move |name: Arc<String>| {
                *requested.lock().unwrap() = Some(name.as_ref().clone());
            }
        });

        let row = panel.row_rect_for(1);
        let value = panel.value_rect_for(row);
        let browse = file_browse_rect(value);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(browse.x + browse.width as i32 - 2, browse.y + 13),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(requested.lock().unwrap().as_deref(), Some("source"));
        assert_eq!(panel.editing_entry(), None, "the browse button does not open the text editor");

        panel.handle_event(&Event::MousePress {
            pos: Point::new(value.x + 4, value.y + 13),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(panel.editing_entry(), Some(0), "the value body edits the path inline");
    }

    /// V-21: committed text arrives through `TextInput` (so IME input works) and the caret moves.
    #[test]
    fn an_editor_accepts_ime_text_and_moves_the_caret() {
        use crate::event::key_codes::{ENTER, LEFT, RIGHT};
        let mut panel = PropertiesPanel::new(Rect::new(0, 0, 320, 240));
        panel.add_property(make_text_property("name", "ab", Some("General")));
        let row = panel.row_rect_for(1);
        panel.handle_event(&Event::MousePress {
            pos: Point::new(row.x + VALUE_COL_LEFT + 4, row.y + 13),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(panel.edit_cursor(), Some(2), "the caret starts at the end");

        panel.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        // A CJK cluster arrives as committed `TextInput`, not as ASCII key codes.
        panel.handle_event(&Event::text_input("\u{4e2d}"));
        assert_eq!(panel.edit_cursor(), Some(3), "the caret advances past the inserted cluster");

        panel.handle_event(&Event::KeyPress { key: LEFT, modifiers: 0 });
        assert_eq!(panel.edit_cursor(), Some(2));
        panel.handle_event(&Event::KeyPress { key: RIGHT, modifiers: 0 });
        assert_eq!(panel.edit_cursor(), Some(3));

        panel.handle_event(&Event::KeyPress { key: ENTER, modifiers: 0 });
        assert_eq!(panel.properties()[0].value, PropertyValue::Text("ab\u{4e2d}".to_string()));
    }
}
