// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PropertyGrid widget — a two-column property editor table (inspector-style).
//!
//! Displays key-value pairs in a two-column layout similar to VS/Unity Inspector:
//! - Name column (left): bold text on gray background
//! - Value column (right): editable text with alternating row colors
//!   Supports row selection via click and emits a `selected` signal.

use crate::compat::Rc;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::metrics::ControlMetrics;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The editor a property row uses.
///
/// [`PropertyItem`] used to carry only `name`/`value`/`editable`, so every row — including one
/// that holds a flag or a colour — opened as a plain text field. The kind is additive: a row
/// built with [`PropertyItem::new`] is [`PropertyItemKind::Text`] and behaves exactly as before.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PropertyItemKind {
    /// Free-form text.
    #[default]
    Text,
    /// A floating-point number.
    Number,
    /// A checkbox-style flag.
    Bool,
    /// A `#RRGGBB` colour spelling.
    Color,
    /// One of a fixed option list; clicking cycles to the next one.
    Choice(Vec<String>),
    /// A filesystem path.
    File,
}

/// The open editor of one grid row: which row, its buffer and the caret's character index.
#[derive(Debug, Clone)]
struct GridEdit {
    row: usize,
    buffer: String,
    cursor: usize,
}

impl GridEdit {
    fn new(row: usize, text: String) -> Self {
        let cursor = text.chars().count();
        Self { row, buffer: text, cursor }
    }

    fn byte_offset(&self) -> usize {
        self.buffer.char_indices().nth(self.cursor).map_or(self.buffer.len(), |(offset, _)| offset)
    }

    /// The buffer with a caret at the cursor, for the single-run fitted draw.
    fn with_caret(&self) -> String {
        let offset = self.byte_offset();
        let mut rendered = String::with_capacity(self.buffer.len() + 1);
        rendered.push_str(&self.buffer[..offset]);
        rendered.push('|');
        rendered.push_str(&self.buffer[offset..]);
        rendered
    }

    fn insert(&mut self, text: &str) {
        let offset = self.byte_offset();
        self.buffer.insert_str(offset, text);
        self.cursor += text.chars().count();
    }

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

    fn delete(&mut self) -> bool {
        let start = self.byte_offset();
        let Some((_, next)) = self.buffer[start..].char_indices().next() else {
            return false;
        };
        self.buffer.replace_range(start..start + next.len_utf8(), "");
        true
    }
}

/// A single property row in the grid.
#[derive(Debug, Clone)]
pub struct PropertyItem {
    /// Display name shown in the left column.
    pub name: String,
    /// Current value shown in the right column.
    pub value: String,
    /// Whether the value can be edited by the user.
    pub editable: bool,
    /// The editor this row uses.
    pub kind: PropertyItemKind,
    /// Optional category the row belongs to (V-17).
    ///
    /// Consecutive rows with the same category are painted under one non-selectable header; a
    /// `None` row starts an ungrouped section.
    pub category: Option<String>,
}

impl PropertyItem {
    /// Creates a new text property item.
    pub fn new(name: impl Into<String>, value: impl Into<String>, editable: bool) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            editable,
            kind: PropertyItemKind::Text,
            category: None,
        }
    }

    /// Replaces the row's editor kind.
    pub fn with_kind(mut self, kind: PropertyItemKind) -> Self {
        self.kind = kind;
        self
    }

    /// Assigns the row to a category (V-17).
    pub fn in_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// The published token for a row's editor kind.
fn property_item_kind_token(kind: &PropertyItemKind) -> &'static str {
    match kind {
        PropertyItemKind::Text => "text",
        PropertyItemKind::Number => "number",
        PropertyItemKind::Bool => "bool",
        PropertyItemKind::Color => "color",
        PropertyItemKind::Choice(_) => "choice",
        PropertyItemKind::File => "file",
    }
}

/// The declared shape of one property row, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `properties` property, and the control owns both ends of it: the
/// field names are [`PropertyItem`]'s own fields. `name` and `value` are required because
/// [`PropertyItem::new`] requires them; the rest are optional because a row that omits them is an
/// editable text property with no category. `kind` is the editor token and `choices` carries a
/// [`PropertyItemKind::Choice`]'s option list (the only kind with a payload).
pub const PROPERTY_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "name",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "value",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "editable",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "kind",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "choices",
                crate::widget::capability::PropertyValueKind::StringList,
            ),
            crate::widget::capability::RecordField::optional(
                "category",
                crate::widget::capability::PropertyValueKind::String,
            ),
        ],
        // A row has no separate identity field; its name is what a document spells it by, and two
        // rows may legitimately share a name across categories.
        id_field: "name",
        ids_unique: false,
        ids_required: false,
    };

/// One row of [`PROPERTY_ITEM_SCHEMA`].
fn property_item_record(item: &PropertyItem) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("name", CapabilityValue::String(item.name.clone()))
        .with("value", CapabilityValue::String(item.value.clone()))
        .with("editable", CapabilityValue::Bool(item.editable))
        .with("kind", CapabilityValue::String(property_item_kind_token(&item.kind).to_string()));
    if let PropertyItemKind::Choice(choices) = &item.kind {
        record = record.with(
            "choices",
            CapabilityValue::Tuple(
                choices.iter().map(|choice| CapabilityValue::String(choice.clone())).collect(),
            ),
        );
    }
    if let Some(category) = &item.category {
        record = record.with("category", CapabilityValue::String(category.clone()));
    }
    record
}

/// Rebuilds one [`PropertyItem`] from a row of [`PROPERTY_ITEM_SCHEMA`].
fn property_item_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<PropertyItem, CapabilityAccessError> {
    let name = match record.get("name") {
        Some(CapabilityValue::String(name)) => name.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let value = match record.get("value") {
        Some(CapabilityValue::String(value)) => value.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let editable = match record.get("editable") {
        None | Some(CapabilityValue::Null) => true,
        Some(CapabilityValue::Bool(flag)) => *flag,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let choices = match record.get("choices") {
        None | Some(CapabilityValue::Null) => Vec::new(),
        Some(CapabilityValue::Tuple(items)) => items
            .iter()
            .map(|item| match item {
                CapabilityValue::String(choice) => Ok(choice.clone()),
                _ => Err(CapabilityAccessError::TypeMismatch),
            })
            .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let kind = match record.get("kind") {
        None | Some(CapabilityValue::Null) => PropertyItemKind::Text,
        Some(CapabilityValue::String(token)) => match token.as_str() {
            "text" => PropertyItemKind::Text,
            "number" => PropertyItemKind::Number,
            "bool" => PropertyItemKind::Bool,
            "color" => PropertyItemKind::Color,
            "choice" => PropertyItemKind::Choice(choices),
            "file" => PropertyItemKind::File,
            _ => return Err(CapabilityAccessError::OutOfRange),
        },
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let category = match record.get("category") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(category)) => Some(category.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(PropertyItem { name, value, editable, kind, category })
}

/// One painted line of the property grid: a group header or a property row (V-17).
#[derive(Clone, Debug, PartialEq, Eq)]
enum PropertyRow {
    /// A non-selectable category header.
    Category(String),
    /// The property at this index.
    Item(usize),
}

/// A custom editor for one property row (V-008).
///
/// The grid's built-in editor is a text field. A date, a slider or a colour wheel needs an editor
/// the grid cannot guess at, so a host installs one per entry and the grid asks it to build and
/// commit the editor. The delegate is consulted before the plain-text path.
pub trait PropertyEditorDelegate: Send + Sync {
    /// Builds the editor widget for `entry`.
    fn make_editor(&self, entry: &PropertyItem) -> Box<dyn Widget>;
    /// Reads the committed text out of `editor`, or `None` to leave the value unchanged.
    fn commit_value(&self, entry: &PropertyItem, editor: &dyn Widget) -> Option<String>;
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

/// The margin the grid leaves between its own frame and its rows: 2 px on every edge.
///
/// Named once so the header, the property rows, the hit test and the visible-row count are
/// all measured from the same inset. They used to start at the control's literal `rect.y`,
/// which pinned the header's glyph box to the frame's own stroke.
const GRID_INSET: u32 = 2;

/// PropertyGrid widget — a two-column property editor table.
pub struct PropertyGrid {
    base: BaseWidget,
    properties: Vec<PropertyItem>,
    selected_index: Option<usize>,
    /// The property row the pointer is over, or `None`.
    ///
    /// Stored as an entry index like `selected_index`, so a scroll moves the
    /// highlight with its row instead of lighting whatever index the row used
    /// to occupy.
    hovered_row: Option<usize>,
    scroll_offset: u32,
    /// Emitted when a property row is selected via click.
    pub selected: Signal1<usize>,
    /// The row being edited, its scratch buffer and the caret.
    editing: Option<GridEdit>,
    /// Emitted when an edit commits `(row, text)`.
    pub cell_changed: Signal1<(usize, String)>,
    /// Emitted when the host should open a colour picker for `(row, current hex)` (V-16).
    pub color_picker_requested: Signal1<(usize, String)>,
    /// Emitted when the host should open a file picker for `row` (V-16).
    pub file_picker_requested: Signal1<usize>,
    /// Emitted when the context menu is requested at a point.
    pub context_menu_requested: Signal1<crate::core::Point>,
    /// The row whose choice popup is open, if any (V-16).
    choice_popup: Option<usize>,
    /// The display rows (group headers + items), cached until the structure changes (V-17).
    ///
    /// The cache holds an `Rc` so a reader shares the row set instead of cloning the whole
    /// table: five call sites (paint, geometry, hit test) used to deep-copy every row on every
    /// call (V-012).
    grouped_cache: std::cell::RefCell<Option<(u64, Rc<Vec<PropertyRow>>)>>,
    /// Bumped by every structural change; keys `grouped_cache`.
    group_revision: u64,
    /// Collapsed category headers (V-007).
    collapsed_categories: HashSet<String>,
    /// Emitted with `(category, collapsed)` when a category is toggled (V-007).
    pub on_category_toggled: Signal1<(String, bool)>,
    /// Per-entry custom editors (V-008).
    entry_delegates: HashMap<usize, Arc<dyn PropertyEditorDelegate>>,
    /// An editor a delegate built and the grid is holding for the host (V-008).
    delegate_editor: Option<(usize, Box<dyn Widget>)>,
}

impl PropertyGrid {
    /// The grid's inset content box, the one rectangle every row is measured from.
    ///
    /// `draw` lays the header and the property rows out from this box rather than from the
    /// control's literal top edge — which is what kept the header's glyph box off the frame's
    /// own stroke — so the hit test, the visible-row count and the paint path all derive from
    /// this one helper instead of each re-deriving the offset.
    fn content_rect(&self) -> Rect {
        ControlMetrics::band_inset(self.geometry(), GRID_INSET)
    }

    /// Number of property rows that fit in the current geometry.
    ///
    /// Mirrors the arithmetic in `draw` so a hit-test can reject a click below the
    /// last painted row. Without this, a click in the empty area under the rows produced
    /// an index that `selected_index < properties.len()` accepted, selecting a row the
    /// user cannot see.
    fn visible_row_count(&self) -> u32 {
        let height = self.content_rect().height;
        height.saturating_sub(FIRST_ROW_TOP) / ROW_HEIGHT
    }

    /// The painted display rows: group headers interleaved with items (V-17).
    ///
    /// Rebuilt only when the structure changes; with no categories it is exactly one `Item` per
    /// property, so every existing geometry and hit test keeps its historical answer. Returns the
    /// shared row set, so a read is a refcount bump rather than a table clone (V-012).
    fn display_rows(&self) -> Rc<Vec<PropertyRow>> {
        let mut cache = self.grouped_cache.borrow_mut();
        let stale = cache.as_ref().is_none_or(|(revision, _)| *revision != self.group_revision);
        if stale {
            *cache = Some((self.group_revision, Rc::new(self.build_display_rows())));
        }
        Rc::clone(&cache.as_ref().expect("the display rows were just built").1)
    }

    /// Builds the display rows from the property list.
    fn build_display_rows(&self) -> Vec<PropertyRow> {
        let mut rows = Vec::with_capacity(self.properties.len());
        let mut current: Option<&str> = None;
        for (index, item) in self.properties.iter().enumerate() {
            let category = item.category.as_deref().filter(|category| !category.is_empty());
            let collapsed =
                category.is_some_and(|category| self.collapsed_categories.contains(category));
            match category {
                Some(category) => {
                    if current != Some(category) {
                        rows.push(PropertyRow::Category(category.to_string()));
                        current = Some(category);
                    }
                }
                None => current = None,
            }
            // A collapsed category keeps its header so the reader can reopen it, but hides
            // every row underneath it (V-007).
            if !collapsed {
                rows.push(PropertyRow::Item(index));
            }
        }
        rows
    }

    /// The painted row position of property `index` (V-17).
    fn display_position_of(&self, index: usize) -> Option<usize> {
        self.display_rows()
            .iter()
            .position(|row| matches!(row, PropertyRow::Item(item) if *item == index))
    }

    /// The largest `scroll_offset` that still shows a row.
    ///
    /// One definition for the wheel, the arrow keys and the End key. Those three used to recompute
    /// it from a locally-defined `row_height = 24` together with `height - (row_height + 1)`, a
    /// formula that disagreed with [`visible_row_count`](Self::visible_row_count) by one row at
    /// some heights and drifted from it whenever either changed.
    fn max_scroll(&self) -> u32 {
        (self.display_rows().len() as u32).saturating_sub(self.visible_row_count())
    }

    /// The property row under `pos`, or `None` when the pointer is over the header,
    /// past the last painted row or outside the rows entirely.
    ///
    /// The exact inverse of the press arm's mapping, so a highlight cannot land on
    /// a row a click would not select.
    fn row_index_at(&self, pos: Point) -> Option<usize> {
        let content = self.content_rect();
        let click_y = pos.y - content.y;
        if click_y < FIRST_ROW_TOP as i32 {
            return None;
        }
        let row_in_view = (click_y as u32 - FIRST_ROW_TOP) / ROW_HEIGHT;
        if row_in_view >= self.visible_row_count() {
            return None;
        }
        let row_index = row_in_view + self.scroll_offset;
        match self.display_rows().get(row_index as usize) {
            Some(PropertyRow::Item(index)) => Some(*index),
            Some(PropertyRow::Category(_)) | None => None,
        }
    }
    /// Creates a new PropertyGrid with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PropertyGrid, geometry, "PropertyGrid"),
            properties: Vec::new(),
            selected_index: None,
            hovered_row: None,
            scroll_offset: 0,
            selected: Signal1::new(),
            editing: None,
            cell_changed: Signal1::new(),
            color_picker_requested: Signal1::new(),
            file_picker_requested: Signal1::new(),
            context_menu_requested: Signal1::new(),
            choice_popup: None,
            grouped_cache: std::cell::RefCell::new(None),
            group_revision: 0,
            collapsed_categories: HashSet::new(),
            on_category_toggled: Signal1::new(),
            entry_delegates: HashMap::new(),
            delegate_editor: None,
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
        self.group_revision = self.group_revision.wrapping_add(1);
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
        self.group_revision = self.group_revision.wrapping_add(1);
        self.selected_index = None;
        self.scroll_offset = 0;
        // The open editor, choice popup and delegate editor all name a row index; with no rows
        // left there is nothing for them to point at.
        self.editing = None;
        self.choice_popup = None;
        self.delegate_editor = None;
        self.base.request_redraw();
    }

    /// Removes the property at `index`, returning it when the index was valid.
    ///
    /// The selection is repaired rather than left dangling: clearing it when it named the removed
    /// row (or is now past the end), and decrementing it when it named a row after the removal.
    /// Any open edit state is cleared because it carries an index the removal just shifted.
    pub fn remove_property(&mut self, index: usize) -> Option<PropertyItem> {
        if index >= self.properties.len() {
            return None;
        }
        let removed = self.properties.remove(index);
        self.group_revision = self.group_revision.wrapping_add(1);
        self.selected_index = match self.selected_index {
            Some(selected) if selected == index => None,
            Some(selected) if selected > index => Some(selected - 1),
            other => other.filter(|&selected| selected < self.properties.len()),
        };
        self.hovered_row = None;
        self.editing = None;
        self.choice_popup = None;
        self.delegate_editor = None;
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());
        self.base.request_redraw();
        Some(removed)
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
    /// ([`clear`](Self::clear), `remove_property`) running; the index then
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

    /// The row whose editor is open, if any.
    pub fn editing_entry(&self) -> Option<usize> {
        self.editing.as_ref().map(|edit| edit.row)
    }

    /// Adds a typed property item.
    pub fn add_property_item(&mut self, item: PropertyItem) {
        self.properties.push(item);
        self.group_revision = self.group_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The editor kind of `index`, when the row exists.
    pub fn property_kind(&self, index: usize) -> Option<&PropertyItemKind> {
        self.properties.get(index).map(|item| &item.kind)
    }

    /// Replaces the editor kind of `index`, returning whether the row exists.
    pub fn set_property_kind(&mut self, index: usize, kind: PropertyItemKind) -> bool {
        let Some(item) = self.properties.get_mut(index) else {
            return false;
        };
        item.kind = kind;
        self.base.request_redraw();
        true
    }

    /// Opens the editor for `index`, dispatching on its kind.
    ///
    /// A flag or a choice is edited in place; every string-shaped kind opens the text
    /// editor seeded with its current display value.
    fn begin_grid_edit(&mut self, index: usize) -> bool {
        let Some(item) = self.properties.get(index).cloned() else {
            return false;
        };
        if !item.editable {
            return false;
        }
        // A custom editor takes precedence over every built-in kind (V-008): a date or a slider
        // would otherwise fall through to the text path and lose the host's editor.
        if let Some(delegate) = self.entry_delegates.get(&index).cloned() {
            let editor = delegate.make_editor(&item);
            self.delegate_editor = Some((index, editor));
            self.selected_index = Some(index);
            self.base.request_redraw();
            return true;
        }
        match item.kind {
            PropertyItemKind::Bool => {
                let next = if item.value.eq_ignore_ascii_case("true") { "false" } else { "true" };
                self.commit_edit_value(index, next.to_string());
                true
            }
            PropertyItemKind::Choice(options) if !options.is_empty() => {
                // A choice opens its option list rather than silently cycling (V-16).
                self.choice_popup = Some(index);
                self.selected_index = Some(index);
                self.base.request_redraw();
                true
            }
            PropertyItemKind::Color => {
                // The host owns the colour UI; the text editor stays available as the
                // fallback for a host that has none.
                self.color_picker_requested.emit((index, item.value.clone()));
                self.editing = Some(GridEdit::new(index, item.value.clone()));
                self.selected_index = Some(index);
                self.base.request_redraw();
                true
            }
            PropertyItemKind::File => {
                self.file_picker_requested.emit(index);
                self.editing = Some(GridEdit::new(index, item.value.clone()));
                self.selected_index = Some(index);
                self.base.request_redraw();
                true
            }
            _ => {
                self.editing = Some(GridEdit::new(index, item.value.clone()));
                self.selected_index = Some(index);
                self.base.request_redraw();
                true
            }
        }
    }

    /// The option rectangles of an open choice popup (V-16).
    ///
    /// Empty when the row is not a choice, is scrolled out of view, or the options do not fit
    /// under its row. The press path and the draw read this one derivation.
    fn choice_option_rects(&self, index: usize) -> Vec<(String, Rect)> {
        let Some(item) = self.properties.get(index) else {
            return Vec::new();
        };
        let PropertyItemKind::Choice(options) = &item.kind else {
            return Vec::new();
        };
        let Some(position) = self.display_position_of(index) else {
            return Vec::new();
        };
        if position < self.scroll_offset as usize {
            return Vec::new();
        }
        let content = self.content_rect();
        let name_col_width = content.width / 3;
        let local = position - self.scroll_offset as usize;
        let mut y = content.y + FIRST_ROW_TOP as i32 + (local as i32 + 1) * ROW_HEIGHT as i32;
        let x = content.x + name_col_width as i32;
        let width = content.width.saturating_sub(name_col_width);
        let bottom = content.y + content.height as i32;
        let mut rects = Vec::new();
        for option in options {
            if y + ROW_HEIGHT as i32 > bottom {
                break;
            }
            rects.push((option.clone(), Rect::new(x, y, width, ROW_HEIGHT)));
            y += ROW_HEIGHT as i32;
        }
        rects
    }

    /// Writes a committed value into row `index` and reports it.
    fn commit_edit_value(&mut self, index: usize, value: String) {
        if let Some(item) = self.properties.get_mut(index) {
            item.value = value.clone();
        }
        self.cell_changed.emit((index, value));
        self.base.request_redraw();
    }

    /// Validates a text buffer against the row's kind and returns the value to store.
    ///
    /// A buffer that cannot be parsed keeps the row's previous value rather than writing an
    /// invalid one; that is the grid's equivalent of `PropertiesPanel::commit_edit` refusing a
    /// malformed number.
    fn coerce_edit_value(&self, index: usize, text: &str) -> String {
        let Some(item) = self.properties.get(index) else {
            return text.to_string();
        };
        match &item.kind {
            PropertyItemKind::Number => text.trim().parse::<f64>().map_or_else(
                |_| item.value.clone(),
                |number| {
                    if number.fract() == 0.0 && number.is_finite() {
                        format!("{}", number as i64)
                    } else {
                        format!("{number}")
                    }
                },
            ),
            PropertyItemKind::Color => Color::parse_hex(text)
                .map_or_else(|| item.value.clone(), |color| color.to_hex_rgb()),
            PropertyItemKind::Choice(options) => {
                if options.iter().any(|option| option == text) {
                    text.to_string()
                } else {
                    item.value.clone()
                }
            }
            PropertyItemKind::Bool => {
                if text.eq_ignore_ascii_case("true") {
                    "true".to_string()
                } else if text.eq_ignore_ascii_case("false") {
                    "false".to_string()
                } else {
                    item.value.clone()
                }
            }
            PropertyItemKind::Text | PropertyItemKind::File => text.to_string(),
        }
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
    /// [`add_property`](Self::add_property) / `remove_property`, which
    /// keep the cursor in range.
    pub fn properties_mut(&mut self) -> &mut Vec<PropertyItem> {
        self.group_revision = self.group_revision.wrapping_add(1);
        &mut self.properties
    }

    // ── Category collapse (V-007) ──

    /// Whether `category`'s rows are hidden.
    pub fn is_category_collapsed(&self, category: &str) -> bool {
        self.collapsed_categories.contains(category)
    }

    /// Collapses or expands `category`, returning the new state.
    ///
    /// Collapsing hides the category's rows but keeps its header, so the gesture is reversible.
    /// The scroll offset is clamped, because the row set may now be shorter than the viewport.
    pub fn set_category_collapsed(&mut self, category: impl Into<String>, collapsed: bool) -> bool {
        let category = category.into();
        if category.is_empty() {
            return false;
        }
        let changed = if collapsed {
            self.collapsed_categories.insert(category.clone())
        } else {
            self.collapsed_categories.remove(&category)
        };
        if !changed {
            return collapsed;
        }
        self.group_revision = self.group_revision.wrapping_add(1);
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());
        self.on_category_toggled.emit((category, collapsed));
        self.base.request_layout();
        self.base.request_redraw();
        collapsed
    }

    /// Toggles `category`'s collapsed state, returning the new state.
    pub fn toggle_category(&mut self, category: impl Into<String>) -> bool {
        let category = category.into();
        let collapsed = !self.is_category_collapsed(&category);
        self.set_category_collapsed(category, collapsed)
    }

    // ── Custom editors (V-008) ──

    /// Installs a custom editor for the property at `index`.
    ///
    /// The delegate is consulted before the plain-text path: [`Self::begin_grid_edit`] asks it for
    /// an editor and holds that editor until [`Self::commit_delegate_edit`]. Returns `false` when
    /// the index does not name a property.
    pub fn set_entry_delegate(
        &mut self,
        index: usize,
        delegate: Arc<dyn PropertyEditorDelegate>,
    ) -> bool {
        if index >= self.properties.len() {
            return false;
        }
        self.entry_delegates.insert(index, delegate);
        true
    }

    /// Removes the custom editor for `index`.
    pub fn clear_entry_delegate(&mut self, index: usize) -> bool {
        self.entry_delegates.remove(&index).is_some()
    }

    /// The custom editor installed for `index`, if any.
    pub fn entry_delegate(&self, index: usize) -> Option<&Arc<dyn PropertyEditorDelegate>> {
        self.entry_delegates.get(&index)
    }

    /// Takes the editor the delegate built, so the host can mount it.
    ///
    /// Taking the editor hands its ownership to the host; [`Self::commit_delegate_edit`] can then
    /// no longer read it, so a host that takes it commits through
    /// [`PropertyEditorDelegate::commit_value`] itself.
    pub fn take_delegate_editor(&mut self) -> Option<(usize, Box<dyn Widget>)> {
        self.delegate_editor.take()
    }

    /// Reads the held delegate editor back and commits its value.
    pub fn commit_delegate_edit(&mut self) -> Option<String> {
        let (index, editor) = self.delegate_editor.take()?;
        let delegate = self.entry_delegates.get(&index)?.clone();
        let entry = self.properties.get(index)?.clone();
        let text = delegate.commit_value(&entry, editor.as_ref())?;
        let value = self.coerce_edit_value(index, &text);
        self.commit_edit_value(index, value);
        Some(text)
    }
}

impl Widget for PropertyGrid {
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
        crate::core::Size::new(300, 300)
    }
    impl_draw_bridge!();
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "category_toggled" | "on_category_toggled" => Some(EventSignalRef::mapped(
                "category_toggled",
                &self.on_category_toggled,
                |value| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(value.0.clone()),
                        CapabilityValue::Bool(value.1),
                    ]))
                },
            )),

            "selected" => Some(EventSignalRef::mapped("selected", &self.selected, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "cell_changed" => {
                Some(EventSignalRef::mapped("cell_changed", &self.cell_changed, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::String(v.1.clone()),
                    ]))
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::String(format!("{},{}", v.x, v.y)),
            )),
            "color_picker_requested" => Some(EventSignalRef::mapped(
                "color_picker_requested",
                &self.color_picker_requested,
                // `(row index, property name)` — the declared shape is `Mixed` because the two
                // components are a number and text.
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::String(v.1.clone()),
                    ]))
                },
            )),
            "file_picker_requested" => Some(EventSignalRef::mapped(
                "file_picker_requested",
                &self.file_picker_requested,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
            _ => None,
        }
    }
}

/// `PropertyGrid`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for PropertyGrid {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The whole row list as records (BLUE26 B26.09): `add_property` and `set_value` take
            // arguments the bare command channel cannot carry, so the list is the property and
            // `set_properties` is its published command form.
            "properties" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &PROPERTY_ITEM_SCHEMA,
                    self.properties.iter().map(property_item_record).collect::<Vec<_>>(),
                )))
            }
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
            // A whole-list replacement through the control's own `clear` + `add_property_item`,
            // so the grouping revision is invalidated in one place.
            "properties" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&PROPERTY_ITEM_SCHEMA).map_err(|violation| {
                    log::warn!("property_grid.properties refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("properties"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let mut items = Vec::with_capacity(list.len());
                for record in list.items() {
                    items.push(property_item_from_record(record)?);
                }
                self.clear();
                for item in items {
                    self.add_property_item(item);
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["properties", "property_count", "selected_index", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `property_grid` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every
    /// property and resets the selection, which is exactly what
    /// [`PropertyGrid::clear`] does. `add_property` takes the row to add and
    /// `set_value` an index+value; the `properties` records property is their
    /// payload channel (B26.03), and `set_properties` is its published command form.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_property" | "set_value" | "set_properties" => {
                Err(CapabilityAccessError::OutOfRange)
            }
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
        // The theme read takes and releases the global manager's lock internally, and the palette
        // comes from the shared snapshot, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("property_grid");
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
        // One palette snapshot read for the window fill and the accent below.
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the header bar and the editable-value ink follow the appearance rather than a
        // literal grey and a literal navy.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
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
        // A hovered row is pointer position, not a committed selection, so it reads the
        // accent at a **lighter** weight than the selected row — the same surface/accent
        // pair the table family uses for its row tint.
        let hovered_row_bg = surface.blend(&accent, 0.12);
        // Row separators are a subdivision of the surface, not a second literal grey.
        let row_separator = surface.blend(&ink, 0.12);
        let disabled_ink = ink.blend(&surface, 0.55);

        // Background
        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // The header row and the property rows are laid out from the control's inset content
        // box, not from its literal top edge. The header used to be the first 26 px of the
        // control and its label was drawn at `rect.y + 6`, so its glyph box began on the
        // frame's own stroke; the rows below then started at `rect.y + row_height + 1` with
        // the same flush-top origin. `band_inset` reserves the margin and `text_line` centres
        // each label on its own row.
        let content = self.content_rect();

        // Header row
        let header_font = role_font(style, 12.0).with_bold(true);
        // The name column's font is the same face as the header, so it is built once here rather
        // than once per visible row (each `role_font` allocates its family `String`).
        let name_font = role_font(style, 12.0).with_bold(true);
        let header_rect = Rect::new(content.x, content.y, content.width, row_height);
        context.fill_rect(header_rect, header_bg);
        let header_line = context.text_line(header_rect, &header_font);
        context.draw_text_fitted_cow(
            Rect::new(
                header_rect.x + 4,
                header_line.y,
                name_col_width.saturating_sub(4),
                header_line.height.max(1),
            ),
            "Property",
            &header_font,
            header_ink,
            HorizontalAlignment::Left,
        );
        context.draw_text_fitted_cow(
            Rect::new(
                header_rect.x + name_col_width as i32 + 4,
                header_line.y,
                header_rect.width.saturating_sub(name_col_width).saturating_sub(8),
                header_line.height.max(1),
            ),
            "Value",
            &header_font,
            header_ink,
            HorizontalAlignment::Left,
        );

        // Draw a separator line under header
        let separator_y = header_rect.y + row_height as i32;
        context.draw_line(
            Point::new(content.x, separator_y),
            Point::new(content.x + content.width as i32, separator_y),
            separator,
        );

        // Property rows
        let value_font = role_font(style, 12.0);
        let mut y = separator_y + 1;

        #[allow(clippy::manual_checked_ops)]
        let visible_count = if row_height > 0 {
            (content.height.saturating_sub(FIRST_ROW_TOP)) / row_height
        } else {
            0
        };

        let rows = self.display_rows();
        let start_idx = self.scroll_offset as usize;
        let end_idx = (start_idx + visible_count as usize).min(rows.len());

        for row in rows.iter().skip(start_idx).take(end_idx.saturating_sub(start_idx)) {
            let row_rect = Rect::new(content.x, y, content.width, row_height);
            let row_line = context.text_line(row_rect, &value_font);
            match row {
                PropertyRow::Category(category) => {
                    // A group header is not selectable and is painted on the name-column wash.
                    context.fill_rect(row_rect, name_column);
                    // A disclosure marker makes the collapsed state and the toggle gesture visible
                    // (V-007).
                    let collapsed = self.is_category_collapsed(category);
                    let icon = if collapsed {
                        crate::widget::IconName::ChevronRight
                    } else {
                        crate::widget::IconName::ChevronDown
                    };
                    let icon_rect = Rect::new(
                        row_rect.x + 2,
                        row_rect.y + (row_rect.height as i32 - 8) / 2,
                        8,
                        8,
                    );
                    crate::widget::draw_icon_at(context, icon_rect, ink, icon);
                    context.draw_text_fitted_cow(
                        Rect::new(
                            row_rect.x + 14,
                            row_line.y,
                            row_rect.width.saturating_sub(18),
                            row_line.height.max(1),
                        ),
                        category,
                        &name_font,
                        ink.blend(&surface, 0.2),
                        HorizontalAlignment::Left,
                    );
                }
                PropertyRow::Item(index) => {
                    let i = *index;
                    let is_selected = self.selected_index == Some(i);

                    // Alternating row background — the hover tint sits between the plain bands
                    // and the selection, so the three states stay visually ordered.
                    if is_selected {
                        context.fill_rect(row_rect, selected_row);
                    } else if Some(i) == self.hovered_row {
                        context.fill_rect(row_rect, hovered_row_bg);
                    } else if i % 2 == 0 {
                        context.fill_rect(row_rect, striped_row);
                    } else {
                        context.fill_rect(row_rect, surface);
                    }

                    // Name column background
                    let name_rect = Rect::new(content.x, y, name_col_width, row_height);
                    context.fill_rect(name_rect, name_column);

                    // Name text (bold)
                    let name_text_color = if !is_enabled { disabled_ink } else { ink };
                    context.draw_text_fitted_cow(
                        Rect::new(
                            name_rect.x + 4,
                            row_line.y,
                            name_col_width.saturating_sub(8),
                            row_line.height.max(1),
                        ),
                        &self.properties[i].name,
                        &name_font,
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
                    let value_text = match self.editing.as_ref() {
                        Some(edit) if edit.row == i => std::borrow::Cow::Owned(edit.with_caret()),
                        _ => std::borrow::Cow::Borrowed(self.properties[i].value.as_str()),
                    };
                    context.draw_text_fitted_cow(
                        Rect::new(
                            row_rect.x + name_col_width as i32 + 4,
                            row_line.y,
                            row_rect.width.saturating_sub(name_col_width).saturating_sub(8),
                            row_line.height.max(1),
                        ),
                        &value_text,
                        &value_font,
                        value_color,
                        HorizontalAlignment::Left,
                    );
                }
            }

            // Row separator
            context.draw_line(
                Point::new(rect.x, y + row_height as i32 - 1),
                Point::new(rect.x + rect.width as i32, y + row_height as i32 - 1),
                row_separator,
            );

            y += row_height as i32;
        }

        // A choice popup is drawn last so it sits above every row (V-16).
        if let Some(index) = self.choice_popup {
            for (option, option_rect) in self.choice_option_rects(index) {
                let selected = self.properties.get(index).is_some_and(|item| item.value == option);
                context.fill_rect(option_rect, if selected { selected_row } else { surface });
                context.draw_rect(option_rect, separator);
                context.draw_text_fitted_cow(
                    option_rect,
                    &option,
                    &value_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for PropertyGrid {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                if *button == 3 {
                    self.context_menu_requested.emit(*pos);
                    return;
                }
                if *button == 1 {
                    let is_press = matches!(event, Event::MousePress { .. });
                    // An open choice popup owns the next press: clicking an option commits it and
                    // clicking anywhere else dismisses the popup (V-16). A release must not
                    // dismiss the popup the press just opened.
                    if is_press {
                        if let Some(popup) = self.choice_popup {
                            let hit = self
                                .choice_option_rects(popup)
                                .into_iter()
                                .find(|(_, rect)| rect.contains_point(*pos));
                            self.choice_popup = None;
                            if let Some((option, _)) = hit {
                                let value = self.coerce_edit_value(popup, &option);
                                self.commit_edit_value(popup, value);
                            }
                            self.base.request_redraw();
                            return;
                        }
                    }
                    let content = self.content_rect();

                    // The click is measured from the content box's own top edge, so a press
                    // above the first painted row (`click_y < FIRST_ROW_TOP`) is outside the
                    // rows and must deselect rather than index one. The previous arithmetic
                    // cast `click_y` to `u32` before subtracting, which turned a negative
                    // offset into a huge row index.
                    let click_y = pos.y - content.y;
                    // The row index below is derived from `pos.y` **alone**: nothing in that
                    // arithmetic looks at `x`, and nothing tests whether `y` is inside the control.
                    // Without this guard a press one column over in a dialog -- or in the window's
                    // own margin -- selected whichever property happened to sit at that height, and
                    // emitted `clicked` for a gesture that never touched the grid.
                    //
                    // # Why the guard gates *selection* and not the whole arm
                    //
                    // `property_grid_click_above_the_widget_deselects` pins a deliberate
                    // behaviour: a press outside the rows **deselects**. Returning early for every
                    // point outside the control would silently drop that, turning an
                    // outside-the-widget press into a no-op. The two requirements are not in
                    // conflict once separated: containment decides whether a *row can be chosen*,
                    // and the click's own `y` decides whether the existing selection *stands*.
                    // A press outside therefore still clears the selection and still cannot select.
                    let over_the_grid = self.base.contains_point_with_touch_expansion(*pos);

                    if over_the_grid && click_y >= FIRST_ROW_TOP as i32 {
                        // `draw` paints display row `scroll_offset + i` at
                        // `y = content.y + FIRST_ROW_TOP + i * ROW_HEIGHT`, so this is the
                        // inverse of that mapping. `>=` (not `>`) matters: a click on the
                        // first row's top edge is a click on that row, and `>` skipped it.
                        let row_in_view = (click_y as u32 - FIRST_ROW_TOP) / ROW_HEIGHT;
                        let display_index = row_in_view as usize + self.scroll_offset as usize;
                        let display_row = self.display_rows().get(display_index).cloned();

                        // Bound above by the rows actually painted. Without this, a click
                        // in the blank area below the last row selected an invisible row
                        // that merely happened to exist in `properties`.
                        if row_in_view < self.visible_row_count() {
                            // A press on a category header toggles its rows instead of selecting
                            // a property (V-007).
                            if is_press {
                                if let Some(PropertyRow::Category(category)) = display_row.as_ref()
                                {
                                    let category = category.clone();
                                    self.toggle_category(category);
                                    return;
                                }
                            }
                            if let Some(PropertyRow::Item(row_index)) = display_row {
                                self.selected_index = Some(row_index);
                                self.selected.emit(row_index);
                                self.base.clicked.emit();
                                // A typed row commits its own gesture on a single click (V-16):
                                // a flag flips and a choice opens its list. Every other kind
                                // still needs the double-click editor, so a plain press only
                                // selects.
                                if is_press
                                    && self
                                        .properties
                                        .get(row_index)
                                        .is_some_and(|item| item.editable)
                                {
                                    match self.properties[row_index].kind {
                                        PropertyItemKind::Bool => {
                                            let _ = self.begin_grid_edit(row_index);
                                        }
                                        PropertyItemKind::Choice(_) => {
                                            self.choice_popup = Some(row_index);
                                        }
                                        _ => {}
                                    }
                                }
                                self.base.request_redraw();
                                return;
                            }
                        }
                    }

                    // Click outside rows — deselect
                    if self.selected_index.is_some() {
                        self.selected_index = None;
                        self.base.request_redraw();
                    }
                }
            }
            Event::MouseMove { pos, .. } => {
                // Hover is the row under the pointer, from the same inverse mapping the
                // press uses, so a highlight cannot land on a row a click would not select.
                // `MouseMove` fires far more often than the highlight changes, so the
                // redraw is requested only on an actual change.
                let hovered = if self.base.contains_point_with_touch_expansion(*pos) {
                    self.row_index_at(*pos)
                } else {
                    None
                };
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
                self.base.handle_event(event);
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(row) = self.row_index_at(*pos) {
                    let _ = self.begin_grid_edit(row);
                }
            }
            Event::ContextMenu { pos } => self.context_menu_requested.emit(*pos),
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
            // IME / keyboard-layout committed text, rather than raw ASCII key codes.
            Event::TextInput { text }
                if self.editing.is_some() && text.chars().all(|ch| !ch.is_control()) =>
            {
                if let Some(edit) = self.editing.as_mut() {
                    edit.insert(text);
                }
                self.base.request_redraw();
            }
            Event::KeyPress { key, .. } => {
                if self.editing.is_some() {
                    let redraw = match *key {
                        crate::event::key_codes::ENTER => {
                            let edit = self.editing.take();
                            if let Some(edit) = edit {
                                let (row, text) = (edit.row, edit.buffer);
                                let value = self.coerce_edit_value(row, &text);
                                self.commit_edit_value(row, value);
                            }
                            false
                        }
                        crate::event::key_codes::ESCAPE => {
                            self.editing = None;
                            true
                        }
                        _ => {
                            let Some(edit) = self.editing.as_mut() else { return };
                            match *key {
                                crate::event::key_codes::BACKSPACE => edit.backspace(),
                                crate::event::key_codes::DELETE => edit.delete(),
                                crate::event::key_codes::LEFT => {
                                    edit.cursor = edit.cursor.saturating_sub(1);
                                    true
                                }
                                crate::event::key_codes::RIGHT => {
                                    if edit.cursor < edit.buffer.chars().count() {
                                        edit.cursor += 1;
                                    }
                                    true
                                }
                                crate::event::key_codes::HOME => {
                                    edit.cursor = 0;
                                    true
                                }
                                crate::event::key_codes::END => {
                                    edit.cursor = edit.buffer.chars().count();
                                    true
                                }
                                _ => false,
                            }
                        }
                    };
                    if redraw {
                        self.base.request_redraw();
                    }
                    return;
                }
                let max_scroll = self.max_scroll();
                if *key == crate::event::key_codes::ESCAPE && self.choice_popup.take().is_some() {
                    self.base.request_redraw();
                    return;
                }
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
                            let new_pos = self.display_position_of(new_idx).unwrap_or(new_idx);
                            if new_pos < self.scroll_offset as usize {
                                self.scroll_offset = new_pos as u32;
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
                        let new_pos = self.display_position_of(new_idx).unwrap_or(new_idx);
                        if new_pos >= self.scroll_offset as usize + visible as usize {
                            self.scroll_offset = (new_pos as u32 + 1).saturating_sub(visible);
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
                            let last_pos = self.display_position_of(last).unwrap_or(last);
                            self.scroll_offset =
                                (last_pos as u32 + 1).saturating_sub(visible).min(max_scroll);
                            self.base.request_redraw();
                        }
                    }
                    crate::event::key_codes::PAGE_UP => {
                        let current = self.selected_index.unwrap_or(0);
                        let page = self.visible_row_count().max(1) as usize;
                        let next = current.saturating_sub(page);
                        self.selected_index = Some(next);
                        self.selected.emit(next);
                        self.scroll_offset = self.display_position_of(next).unwrap_or(next) as u32;
                        self.base.request_redraw();
                    }
                    crate::event::key_codes::PAGE_DOWN => {
                        if !self.properties.is_empty() {
                            let current = self.selected_index.unwrap_or(0);
                            let page = self.visible_row_count().max(1) as usize;
                            let next = (current + page).min(self.properties.len() - 1);
                            self.selected_index = Some(next);
                            self.selected.emit(next);
                            let visible = self.visible_row_count();
                            let next_pos = self.display_position_of(next).unwrap_or(next);
                            self.scroll_offset =
                                (next_pos as u32 + 1).saturating_sub(visible).min(max_scroll);
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
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 35), button: 1, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(0));
    }

    #[test]
    fn property_grid_mouse_click_header_does_not_select() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);

        // Click in header area (y < row_height + 1 = 25)
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert_eq!(pg.selected_index(), None);
    }

    /// Row 0 occupies `[25, 49)` **inside the control's inset content box**; its top edge is
    /// part of it.
    ///
    /// The hit-test compared `click_y > header_height` (25), so a click exactly on the
    /// first row's top edge fell into the header branch and selected nothing, while
    /// `y = 26` selected row 0. `draw` paints that row starting at 25, so the boundary
    /// case is the row's own first pixel.
    ///
    /// The coordinates below are taken from the grid's own `content_rect`, which is what
    /// this now pins: the rows are measured from the inset content box rather than from the
    /// control's literal top edge, so the first row's top edge sits at `content.y + 25`.
    /// Asserting the absolute `25` would pin the old flush-top geometry and fail once the
    /// inset moved the rows down.
    #[test]
    fn property_grid_first_row_top_edge_selects_row_zero() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);

        let top = pg.content_rect().y + FIRST_ROW_TOP as i32;
        pg.handle_event(&Event::MousePress { pos: Point::new(10, top), button: 1, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(0), "the first row's top edge is part of it");

        // The last pixel of the previous row is not.
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        let top = pg.content_rect().y + FIRST_ROW_TOP as i32;
        pg.handle_event(&Event::MousePress {
            pos: Point::new(10, top - 1),
            button: 1,
            modifiers: 0,
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
        pg.handle_event(&Event::MousePress {
            pos: Point::new(10, 50 + 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(pg.selected_index(), Some(0));

        // A click above the widget's top edge is outside it.
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 5), button: 1, modifiers: 0 });
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

        pg.handle_event(&Event::MousePress { pos: Point::new(10, 196), button: 1, modifiers: 0 });
        assert_eq!(pg.selected_index(), None, "row 7 of 10 is not painted here");

        // The last painted row is still reachable.
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 190), button: 1, modifiers: 0 });
        assert_eq!(pg.selected_index(), Some(6));
    }

    /// A click beside the grid must not select a row, however well its `y` maps.
    ///
    /// # Why the `y` axis alone was not enough
    ///
    /// The row index is derived from `pos.y` alone -- an offset from the content box's top edge
    /// divided by the row height. Nothing in that arithmetic looks at `x`, and until this test the
    /// arm did not either, so a press one column over in a dialog, or in the window's own margin,
    /// selected whichever property happened to sit at that height. The press was outside the control
    /// and the grid answered as though it were inside.
    ///
    /// `property_grid_click_above_the_widget_deselects` covers the vertical axis, which the old code
    /// *did* bound (via `click_y >= FIRST_ROW_TOP`). This one covers the horizontal axis, which had
    /// no bound at all -- so neither test subsumes the other.
    #[test]
    fn property_grid_click_beside_the_grid_does_not_select() {
        let rows_top = |pg: &PropertyGrid| pg.content_rect().y + FIRST_ROW_TOP as i32;
        // A `y` that maps onto row 0, so only the containment test can reject these points.
        let y_inside_row_zero = |pg: &PropertyGrid| rows_top(pg) + ROW_HEIGHT as i32 / 2;

        let build = || {
            let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
            pg.add_property("A", "1", true);
            pg.add_property("B", "2", true);
            pg
        };

        // The baseline: the same `y` *inside* the grid does select, so a green result below cannot
        // be explained by the point being rejected for some other reason.
        let mut inside = build();
        inside.handle_event(&Event::MousePress {
            pos: Point::new(10, y_inside_row_zero(&inside)),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(inside.selected_index(), Some(0), "the baseline press must select row 0");

        for (label, x) in [("left", -5), ("right", 305), ("far right", 5000)] {
            let mut pg = build();
            let y = y_inside_row_zero(&pg);
            pg.handle_event(&Event::MousePress { pos: Point::new(x, y), button: 1, modifiers: 0 });
            assert_eq!(
                pg.selected_index(),
                None,
                "a press {label} of the grid at x={x} (y={y} maps onto row 0) must not select"
            );
        }
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

    /// Hover is the row under the pointer, resolved through the same inverse mapping
    /// the press uses: the header and the blank area under the last row stay unlit,
    /// painted rows resolve to their entry index, and leaving the panel clears the
    /// highlight instead of freezing it in place.
    #[test]
    fn property_grid_hover_follows_the_pointer() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        for i in 0..10 {
            pg.add_property(format!("p{i}"), format!("v{i}"), true);
        }
        assert_eq!(pg.visible_row_count(), 7);

        let rows_top = pg.content_rect().y + FIRST_ROW_TOP as i32;

        // The header band is not a row.
        pg.handle_event(&Event::MouseMove { pos: Point::new(10, pg.content_rect().y + 12) });
        assert_eq!(pg.hovered_row, None);

        // Row 0 and row 1 light their own entries.
        pg.handle_event(&Event::MouseMove { pos: Point::new(10, rows_top + 12) });
        assert_eq!(pg.hovered_row, Some(0));
        pg.handle_event(&Event::MouseMove {
            pos: Point::new(10, rows_top + ROW_HEIGHT as i32 + 12),
        });
        assert_eq!(pg.hovered_row, Some(1));

        // The blank area under the last painted row stays unlit (7 of 10 fit).
        pg.handle_event(&Event::MouseMove { pos: Point::new(10, 196) });
        assert_eq!(pg.hovered_row, None);

        pg.handle_event(&Event::MouseLeave { pos: Point::new(-1, -1) });
        assert_eq!(pg.hovered_row, None);
    }

    /// `handle_event` returns early for a disabled grid, so no mouse move can arm a
    /// hover affordance on a control that also refuses to select.
    #[test]
    fn property_grid_disabled_shows_no_hover() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.set_enabled(false);

        let rows_top = pg.content_rect().y + FIRST_ROW_TOP as i32;
        pg.handle_event(&Event::MouseMove { pos: Point::new(10, rows_top + 12) });
        assert_eq!(pg.hovered_row, None);
    }

    /// Double-click editing, page keys and the change signal are wired.
    #[test]
    fn editing_paging_and_change_signal_are_wired() {
        use crate::event::EventHandler;
        let mut grid = PropertyGrid::new(Rect::new(0, 0, 320, 200));
        for index in 0..20 {
            grid.add_property(format!("p{index}"), format!("v{index}"), true);
        }
        let row = grid.content_rect().y + FIRST_ROW_TOP as i32 + 12;
        grid.handle_event(&Event::MouseDoubleClick { pos: Point::new(200, row), button: 1 });
        assert_eq!(grid.editing_entry(), Some(0));
        grid.handle_event(&Event::text_input("x"));
        let changed = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&changed);
        grid.cell_changed.connect(move |cell| {
            if let Ok(mut slot) = sink.lock() {
                *slot = Some((*cell).clone());
            }
        });
        grid.handle_event(&Event::KeyPress { key: crate::event::key_codes::ENTER, modifiers: 0 });
        assert!(
            changed.lock().map(|slot| slot.is_some()).unwrap_or(false),
            "committing emits cell_changed"
        );

        grid.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        grid.handle_event(&Event::KeyPress {
            key: crate::event::key_codes::PAGE_DOWN,
            modifiers: 0,
        });
        assert!(grid.selected_index.unwrap_or(0) > 0);
    }

    /// V-16: a flag toggles on a single click, and a choice opens its option list.
    #[test]
    fn typed_rows_toggle_and_offer_options() {
        use crate::event::EventHandler;

        let mut grid = PropertyGrid::new(Rect::new(0, 0, 320, 200));
        grid.add_property_item(
            PropertyItem::new("Enabled", "false", true).with_kind(PropertyItemKind::Bool),
        );
        grid.add_property_item(
            PropertyItem::new("Mode", "a", true)
                .with_kind(PropertyItemKind::Choice(vec!["a".into(), "b".into()])),
        );

        let rows_top = grid.content_rect().y + FIRST_ROW_TOP as i32;
        grid.handle_event(&Event::MousePress {
            pos: Point::new(200, rows_top + 12),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(grid.value(0), Some("true"), "a flag flips on one click");

        grid.handle_event(&Event::MousePress {
            pos: Point::new(200, rows_top + ROW_HEIGHT as i32 + 12),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(grid.choice_popup, Some(1), "a choice opens its option list");
        let options = grid.choice_option_rects(1);
        assert_eq!(options.len(), 2);
        let (option, rect) = options[1].clone();
        assert_eq!(option, "b");
        grid.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 2, rect.y + 2),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(grid.value(1), Some("b"), "clicking an option commits it");
        assert_eq!(grid.choice_popup, None);
    }

    /// V-16: colour and file rows ask the host for its picker instead of forcing typed text.
    #[test]
    fn colour_and_file_rows_request_their_pickers() {
        use crate::event::EventHandler;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut grid = PropertyGrid::new(Rect::new(0, 0, 320, 200));
        grid.add_property_item(
            PropertyItem::new("Background", "#ff0000", true).with_kind(PropertyItemKind::Color),
        );
        grid.add_property_item(
            PropertyItem::new("Path", "/tmp/a", true).with_kind(PropertyItemKind::File),
        );
        let colours = std::sync::Arc::new(AtomicUsize::new(0));
        let files = std::sync::Arc::new(AtomicUsize::new(0));
        let colour_sink = std::sync::Arc::clone(&colours);
        let file_sink = std::sync::Arc::clone(&files);
        grid.color_picker_requested.connect(move |_| {
            colour_sink.fetch_add(1, Ordering::Relaxed);
        });
        grid.file_picker_requested.connect(move |_| {
            file_sink.fetch_add(1, Ordering::Relaxed);
        });

        let rows_top = grid.content_rect().y + FIRST_ROW_TOP as i32;
        grid.handle_event(&Event::MouseDoubleClick {
            pos: Point::new(200, rows_top + 12),
            button: 1,
        });
        assert_eq!(colours.load(Ordering::Relaxed), 1);
        assert_eq!(grid.editing_entry(), Some(0), "the text editor stays as the fallback");
        grid.handle_event(&Event::KeyPress { key: crate::event::key_codes::ESCAPE, modifiers: 0 });

        grid.handle_event(&Event::MouseDoubleClick {
            pos: Point::new(200, rows_top + ROW_HEIGHT as i32 + 12),
            button: 1,
        });
        assert_eq!(files.load(Ordering::Relaxed), 1);
    }

    /// V-17: a category adds a non-selectable header row without shifting property indices.
    #[test]
    fn categories_insert_selectable_headers() {
        use crate::event::EventHandler;

        let mut grid = PropertyGrid::new(Rect::new(0, 0, 320, 300));
        grid.add_property_item(PropertyItem::new("A", "1", true).in_category("General"));
        grid.add_property_item(PropertyItem::new("B", "2", true).in_category("General"));
        grid.add_property_item(PropertyItem::new("C", "3", true).in_category("Layout"));

        let rows = grid.display_rows();
        assert_eq!(
            *rows,
            vec![
                PropertyRow::Category("General".into()),
                PropertyRow::Item(0),
                PropertyRow::Item(1),
                PropertyRow::Category("Layout".into()),
                PropertyRow::Item(2),
            ]
        );

        // A click on a header row selects nothing and toggles its group (V-007); the row under
        // it selects its property once the group is open again.
        let rows_top = grid.content_rect().y + FIRST_ROW_TOP as i32;
        grid.handle_event(&Event::MousePress {
            pos: Point::new(10, rows_top + 12),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(grid.selected_index(), None, "a group header is not selectable");
        assert!(grid.is_category_collapsed("General"), "a header click toggles its group");
        grid.toggle_category("General");
        grid.handle_event(&Event::MousePress {
            pos: Point::new(10, rows_top + ROW_HEIGHT as i32 + 12),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(grid.selected_index(), Some(0));
    }

    /// V-012: repeated reads share one row set instead of cloning the table.
    ///
    /// The pointer comparison is the observable form of the change: a caller must receive the
    /// cache's own allocation on a cache hit, and a new one only after the structure moves.
    #[test]
    fn display_rows_are_shared_between_reads() {
        let mut grid = PropertyGrid::new(Rect::new(0, 0, 320, 300));
        grid.add_property("A", "1", true);
        grid.add_property("B", "2", false);

        let first = grid.display_rows();
        let second = grid.display_rows();
        assert!(Rc::ptr_eq(&first, &second), "a cache hit hands out the same row set");

        grid.add_property("C", "3", true);
        let third = grid.display_rows();
        assert!(!Rc::ptr_eq(&first, &third), "a structural change rebuilds the row set");
        assert_eq!(third.len(), 3);
    }
}
