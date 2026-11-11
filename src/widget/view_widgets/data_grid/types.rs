// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared data types, enums and tokens for the data grid.

use super::*;

/// The margin a data grid leaves between its own frame and its cells: 2 px on every edge.
///
/// Named once so the first cell and the frozen-column divider are both measured from the
/// same inset. The cells used to start at the control's literal `rect.y`, which put the first
/// row's border on the frame's own stroke.
pub(crate) const GRID_INSET: u32 = 2;

/// The strip a data grid reserves above its first cell for column titles: 22 px, one row.
///
/// A grid reads as one whether or not a header row is supplied, so the first cell is always
/// one header below the frame rather than pinned to the top edge.
pub(crate) const GRID_HEADER_HEIGHT: u32 = 22;

/// What a selection gesture selects in a [`DataGrid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataGridSelectionBehavior {
    /// Individual cells (the historical behaviour).
    #[default]
    Cells,
    /// Whole rows, whichever cell was clicked.
    Rows,
    /// Whole columns, whichever cell was clicked.
    Columns,
}

pub(crate) fn data_grid_selection_mode_to_str(
    mode: crate::widget::view_widgets::list_view::SelectionMode,
) -> &'static str {
    use crate::widget::view_widgets::list_view::SelectionMode;
    match mode {
        SelectionMode::Single => "single",
        SelectionMode::Multi => "multi",
        SelectionMode::Extended => "extended",
        SelectionMode::None => "none",
    }
}

/// The published spelling of a column sizing mode.
pub(crate) fn column_width_mode_to_str(mode: ColumnWidthMode) -> &'static str {
    match mode {
        ColumnWidthMode::Fixed => "fixed",
        ColumnWidthMode::Stretch => "stretch",
        ColumnWidthMode::StretchLast => "stretch_last",
        ColumnWidthMode::Interactive => "interactive",
    }
}

/// Parses the published column sizing token.
pub(crate) fn expect_column_width_mode(
    value: CapabilityValue,
) -> Result<ColumnWidthMode, CapabilityAccessError> {
    match crate::widget::capability::coercion::expect_string(value)?.to_ascii_lowercase().as_str() {
        "fixed" => Ok(ColumnWidthMode::Fixed),
        "stretch" => Ok(ColumnWidthMode::Stretch),
        "stretch_last" | "stretchlast" => Ok(ColumnWidthMode::StretchLast),
        "interactive" => Ok(ColumnWidthMode::Interactive),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

pub(crate) fn expect_data_grid_selection_mode(
    value: CapabilityValue,
) -> Result<crate::widget::view_widgets::list_view::SelectionMode, CapabilityAccessError> {
    use crate::widget::view_widgets::list_view::SelectionMode;
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "single" => Ok(SelectionMode::Single),
            "multi" => Ok(SelectionMode::Multi),
            "extended" => Ok(SelectionMode::Extended),
            "none" => Ok(SelectionMode::None),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

pub(crate) fn selection_behavior_to_str(behavior: DataGridSelectionBehavior) -> &'static str {
    match behavior {
        DataGridSelectionBehavior::Cells => "cells",
        DataGridSelectionBehavior::Rows => "rows",
        DataGridSelectionBehavior::Columns => "columns",
    }
}

pub(crate) fn expect_selection_behavior(
    value: CapabilityValue,
) -> Result<DataGridSelectionBehavior, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "cells" => Ok(DataGridSelectionBehavior::Cells),
            "rows" => Ok(DataGridSelectionBehavior::Rows),
            "columns" => Ok(DataGridSelectionBehavior::Columns),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Sort descriptor for a data grid column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortSpec {
    /// Zero-based source column index.
    pub column: usize,
    /// Descending sort when true.
    pub descending: bool,
}

/// Contains-based text filter bound to a source column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnFilter {
    /// Zero-based source column index.
    pub column: usize,
    /// Query token applied with case-insensitive contains.
    pub query: String,
}

/// How a data grid decides column widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnWidthMode {
    /// Every column uses [`DataGrid::column_width`] (the historical behaviour).
    #[default]
    Fixed,
    /// Columns divide the visible cells box equally.
    Stretch,
    /// Every column keeps its width except the last, which takes whatever room is left.
    ///
    /// This is the classic table-header mode for a list whose final column is a note or action
    /// column: stretching every column would move the one the reader is reading.
    StretchLast,
    /// Columns use the fixed width until the caller resizes one (interactive mode).
    Interactive,
}

/// Gestures that start an in-place edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditTriggers(u8);

impl EditTriggers {
    /// No gesture starts an edit.
    pub const NONE: Self = Self(0);
    /// A double-click starts an edit.
    pub const DOUBLE_CLICKED: Self = Self(1 << 0);
    /// A selected cell starts an edit when the user presses Enter.
    pub const EDIT_KEY: Self = Self(1 << 1);
    /// Any of the above.
    pub const ALL: Self = Self(0b11);

    /// Whether every flag in `other` is present.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two trigger sets.
    pub fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The data type a column holds (RW-6).
///
/// A column description used to carry presentation only (a title, a width and three gestures), so
/// a host that wanted to edit a typed column had to keep its own parallel type table keyed by
/// column index. The type is stored on [`ColumnSpec`] instead, which lets a header, a cell editor
/// and a diagnostic all read the same description.
///
/// [`ColumnDataType::Auto`] is the default and means "whatever the model already shows", which is
/// exactly the pre-existing behaviour for a column that never declared a type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ColumnDataType {
    /// No declared type; the value is shown as the model spells it.
    #[default]
    Auto,
    /// Free-form text.
    Text,
    /// A signed integral value.
    Integer,
    /// A floating-point value.
    Number,
    /// A boolean, spelled `true`/`false`.
    Bool,
    /// A calendar date.
    Date,
    /// A clock time.
    Time,
    /// A date and a clock time.
    DateTime,
    /// A `#RRGGBB` colour spelling.
    Color,
    /// A filesystem path.
    File,
    /// A host-defined type the crate does not interpret.
    Other(String),
}

impl ColumnDataType {
    /// The published token spelling used by [`ColumnSpec::to_token`].
    pub fn as_str(&self) -> &str {
        match self {
            Self::Auto => "auto",
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Bool => "bool",
            Self::Date => "date",
            Self::Time => "time",
            Self::DateTime => "datetime",
            Self::Color => "color",
            Self::File => "file",
            Self::Other(name) => name.as_str(),
        }
    }

    /// Parses a published token; anything unrecognised becomes [`Self::Other`] rather than an
    /// error, because a column type is descriptive metadata and an unknown spelling from a newer
    /// writer must not make the whole token unparsable.
    pub fn from_token(token: &str) -> Self {
        match token.trim().to_ascii_lowercase().as_str() {
            "" | "auto" | "any" => Self::Auto,
            "text" | "string" => Self::Text,
            "integer" | "int" => Self::Integer,
            "number" | "float" | "decimal" => Self::Number,
            "bool" | "boolean" => Self::Bool,
            "date" => Self::Date,
            "time" => Self::Time,
            "datetime" | "date_time" | "timestamp" => Self::DateTime,
            "color" | "colour" => Self::Color,
            "file" | "path" => Self::File,
            other => Self::Other(other.to_string()),
        }
    }
}

/// What a [`ColumnChange`] did to the column structure (RW-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnChangeKind {
    /// A column was added at the reported index.
    Inserted,
    /// A column was removed from the reported index.
    Removed,
    /// The column's title changed; the index still names the same column.
    TitleChanged,
    /// The column's declared data type changed.
    TypeChanged,
}

impl ColumnChangeKind {
    /// The token spelling the wire publishes for this change kind (RW-5).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inserted => "inserted",
            Self::Removed => "removed",
            Self::TitleChanged => "title_changed",
            Self::TypeChanged => "type_changed",
        }
    }
}

/// A structural change to a table's column list (RW-5).
///
/// One definition shared by every table implementation that owns columns, so a host that listens
/// to a `TableWidget` and a `DataGrid` reads one payload type rather than two lookalikes
/// (principle #54).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnChange {
    /// What happened.
    pub kind: ColumnChangeKind,
    /// The column the change is addressed at: the insertion point, the removed index, or the
    /// index whose title/type changed.
    pub index: usize,
}

impl ColumnChange {
    /// A change of `kind` at `index`.
    pub fn new(kind: ColumnChangeKind, index: usize) -> Self {
        Self { kind, index }
    }
}

/// One column as a header describes it.
///
/// A header is shared by more than one table implementation, so this is deliberately the whole
/// of a column's description: the title, its width, the three capabilities a header row can
/// offer, and the declared data type. Nothing here refers to "the" cell value, which stays the
/// table's own business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    /// Column title.
    pub title: String,
    /// Width in logical pixels when the header is not stretching the column.
    pub width: u32,
    /// Whether clicking the section cycles its sort order.
    pub sortable: bool,
    /// Whether the section shows a filter affordance.
    pub filterable: bool,
    /// Whether the section's right edge may be dragged.
    pub resizable: bool,
    /// How the column's values are aligned in their cells (V-001).
    ///
    /// The default is [`HorizontalAlignment::Left`], the historical paint behaviour, so a
    /// column that never asks for an alignment keeps its original pixels.
    pub alignment: HorizontalAlignment,
    /// The declared data type of the column's values (RW-6).
    pub data_type: ColumnDataType,
}

impl ColumnSpec {
    /// Creates a plain column: fixed width, no sort or filter affordance, resizable.
    pub fn new(title: impl Into<String>, width: u32) -> Self {
        Self {
            title: title.into(),
            width: width.max(1),
            sortable: false,
            filterable: false,
            resizable: true,
            alignment: HorizontalAlignment::Left,
            data_type: ColumnDataType::Auto,
        }
    }

    /// Marks the column sortable.
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Marks the column filterable.
    pub fn with_filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// Sets whether the column's edge may be dragged.
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Sets the column's cell alignment (V-001).
    pub fn with_alignment(mut self, alignment: HorizontalAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Declares the column's data type (RW-6).
    pub fn with_type(mut self, data_type: ColumnDataType) -> Self {
        self.data_type = data_type;
        self
    }

    /// The column's declared data type (RW-6).
    pub fn data_type(&self) -> &ColumnDataType {
        &self.data_type
    }

    /// The published property spelling:
    /// `title|width|sortable|filterable|resizable|alignment|type`.
    ///
    /// The trailing fields were appended one at a time, so a token written by an older build
    /// (five fields without an alignment, or six without a type) still parses: a missing
    /// alignment means `Left` and a missing type means [`ColumnDataType::Auto`].
    pub fn to_token(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.title,
            self.width,
            u8::from(self.sortable),
            u8::from(self.filterable),
            u8::from(self.resizable),
            alignment_token(self.alignment),
            self.data_type.as_str()
        )
    }

    /// Parses [`Self::to_token`]; flags are `0`/`1`.
    ///
    /// # Backward compatibility
    ///
    /// The split is capped at seven fields so the type field may not swallow anything, and each
    /// trailing field is optional: the six-field form (no type) parses exactly as before, and the
    /// five-field form (no alignment) keeps meaning `Left`. A title containing `|` was never
    /// supported by this token and still is not.
    pub fn from_token(token: &str) -> Option<Self> {
        let mut parts = token.splitn(7, '|');
        let title = parts.next()?.to_string();
        let width = parts.next()?.trim().parse::<u32>().ok()?.max(1);
        let flag = |text: Option<&str>| matches!(text.map(str::trim), Some("1" | "true"));
        Some(Self {
            title,
            width,
            sortable: flag(parts.next()),
            filterable: flag(parts.next()),
            resizable: parts.next().is_none_or(|text| !matches!(text.trim(), "0" | "false")),
            alignment: parts
                .next()
                .and_then(|text| alignment_from_token(text.trim()))
                .unwrap_or(HorizontalAlignment::Left),
            data_type: parts.next().map(ColumnDataType::from_token).unwrap_or_default(),
        })
    }
}

/// The token spelling of a `HorizontalAlignment`, used by [`ColumnSpec::to_token`].
fn alignment_token(alignment: HorizontalAlignment) -> &'static str {
    match alignment {
        HorizontalAlignment::Left => "left",
        HorizontalAlignment::Center => "center",
        HorizontalAlignment::Right => "right",
    }
}

/// Parses [`alignment_token`]'s spelling.
fn alignment_from_token(token: &str) -> Option<HorizontalAlignment> {
    match token {
        "left" | "l" => Some(HorizontalAlignment::Left),
        "center" | "centre" | "c" => Some(HorizontalAlignment::Center),
        "right" | "r" => Some(HorizontalAlignment::Right),
        _ => None,
    }
}

/// Which way a column's sort runs.
///
/// The data-source layer owns the single definition (principle #54); this path is the grid's
/// historical spelling of the same type.
pub use crate::widget::view_widgets::data_source::SortOrder;

impl SortOrder {
    /// The token spelling published in `sort_columns`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "ascending" | "asc" => Some(Self::Ascending),
            "descending" | "desc" => Some(Self::Descending),
            _ => None,
        }
    }

    /// Whether the order is descending, the flag `DataGrid::set_sort_specs` takes.
    pub fn is_descending(self) -> bool {
        matches!(self, Self::Descending)
    }

    /// The order a click on an already-sorted section advances to.
    pub fn next(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// A merged cell: its origin and how many rows and columns it covers (V-014).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSpan {
    /// The origin's projected row.
    pub row: usize,
    /// The origin's projected column.
    pub column: usize,
    /// Rows covered, at least 1.
    pub row_span: usize,
    /// Columns covered, at least 1.
    pub column_span: usize,
}

impl CellSpan {
    /// A span of at least one row and one column.
    pub fn new(row: usize, column: usize, row_span: usize, column_span: usize) -> Self {
        Self { row, column, row_span: row_span.max(1), column_span: column_span.max(1) }
    }

    /// Whether `(row, column)` is covered by this span.
    pub fn covers(&self, row: usize, column: usize) -> bool {
        row >= self.row
            && row < self.row.saturating_add(self.row_span)
            && column >= self.column
            && column < self.column.saturating_add(self.column_span)
    }
}

/// Paint hook for a data grid cell (V-014).
///
/// A delegate owns a cell's ink when `paint` returns `true`; returning `false` keeps the grid's
/// own text rendering, which is also the behaviour when no delegate is installed.
pub trait DataGridCellDelegate: Send + Sync {
    /// Paints `cell`, returning whether the delegate took over the cell.
    fn paint(
        &self,
        context: &mut RenderContext,
        cell: Rect,
        row: usize,
        column: usize,
        text: Option<&str>,
    ) -> bool {
        let _ = (context, cell, row, column, text);
        false
    }
}

/// The plain single-line editor [`DataGrid::begin_edit`] hands back (V-002).
///
/// It owns the text a cell edit starts with, so a host can place it over the cell and read the
/// committed value back through [`DataGrid::commit_edit`] without knowing which control the grid
/// chose. It draws its own frame and value, which makes it a complete widget rather than a
/// container the host has to populate.
pub struct DataGridCellEditor {
    pub(crate) base: BaseWidget,
    pub(crate) text: String,
}

impl DataGridCellEditor {
    /// Creates an editor seeded with `text` and placed at `geometry`.
    pub fn new(geometry: Rect, text: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "DataGridCellEditor"),
            text: text.into(),
        }
    }

    /// The text the editor currently holds.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the editor's text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }
}

impl Widget for DataGridCellEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    impl_draw_bridge!();
}

impl Draw for DataGridCellEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        // Resolve the interior from the same `Input` theme role a line edit uses. Filling
        // `Color::WHITE` unconditionally made this editor a white box on a dark appearance; the
        // explicit style wins, then the theme's resolved style, then the palette's derived input
        // background, and only a build with no theme falls back to the historical literal.
        let theme = crate::style::resolved_theme_style_shared("line_edit");
        let palette = crate::style::palette_shared();
        let surface = self
            .base
            .style()
            .background_color
            .or_else(|| theme.as_ref().and_then(|style| style.background_color))
            .or_else(|| palette.as_ref().map(|colors| colors.input_background()))
            .unwrap_or(Color::WHITE);
        let ink = self
            .base
            .style()
            .text_color
            .or_else(|| theme.as_ref().and_then(|style| style.text_color))
            .or_else(|| palette.as_ref().map(|colors| colors.foreground))
            .unwrap_or(Color::BLACK);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, ink);
        let font = effective_font(self.base.style());
        context.draw_text_fitted_ellipsis(
            context.text_line(rect, font),
            &self.text,
            font,
            ink,
            HorizontalAlignment::Left,
        );
    }
}

impl crate::event::EventHandler for DataGridCellEditor {
    fn handle_event(&mut self, _event: &Event) {
        // The host owns the platform text input; the grid only owns the edit lifecycle.
    }
}

/// Source of the detail text shown under an expanded data-grid row (V-009).
///
/// The provider is asked once per painted detail band, so a caller that cannot answer cheaply can
/// return `None` and keep the row collapsed.
pub type RowDetailProvider = Arc<dyn Fn(usize) -> Option<String> + Send + Sync>;

/// The height of a header strip: 28 px, one row of titles.
pub(crate) const HEADER_STRIP_HEIGHT: u32 = 28;

/// The default detail-band height an expanded row uses (V-009).
pub(crate) const DEFAULT_ROW_DETAIL_HEIGHT: u32 = 48;

/// How close to a section's right edge a press must land to start a resize.
pub(crate) const HEADER_RESIZE_GRAB: i32 = 4;

/// The side of a sort arrow or filter glyph.
pub(crate) const HEADER_ICON_SIDE: u32 = 10;

#[cfg(test)]
mod tests {
    // The only test here runs on desktop with the theme compiled in; in every other profile it is
    // cfg'd out, and an ungated glob import would then be the module's only item (unused import).
    #[cfg(all(device_profile, feature = "desktop"))]
    use super::*;

    /// The cell editor is a field, so its interior has to follow the `Input` theme role; filling
    /// `Color::WHITE` unconditionally made it a white box on a dark appearance.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_cell_editor_interior_follows_the_theme() {
        use crate::core::Size;
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::draw_bridge::draw_of;

        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);

        let mut editor = DataGridCellEditor::new(Rect::new(0, 0, 80, 24), "x");
        let size = Size::new(80, 24);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        let probe = Color { r: 255, g: 0, b: 255, a: 255 };
        backend.begin_frame(probe);
        {
            let drawable = draw_of(&mut editor).expect("the editor is a drawable widget");
            let mut context = RenderContext::new(&mut backend);
            drawable.draw(&mut context);
        }
        backend.end_frame();

        // (3, 3) is inside the fill, clear of the one-pixel border and the centred glyph.
        let frame = backend.frame_rgba();
        let index = ((3 * 80) + 3) * 4;
        let pixel = (frame[index], frame[index + 1], frame[index + 2]);
        assert_ne!(pixel, (255, 255, 255), "a dark appearance must not paint a white field");
        assert_ne!(pixel, (probe.r, probe.g, probe.b), "the fill covers the backdrop");
    }
}
