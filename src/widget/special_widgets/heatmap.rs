// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Heatmap — a matrix of values drawn as a grid of coloured cells.
//!
//! # Why this is a control of its own
//!
//! Rule #80 permits a new `WidgetKind` when the data model *and* the interaction differ.
//! Both do here:
//!
//! | | `ChartWidget` | `Heatmap` |
//! |---|---|---|
//! | datum | one value per **index** | one value per **(row, column)** |
//! | axes | one ordered value axis | **two categorical** axes, each labelled |
//! | geometry | a function of `(index, value)` | a function of the cell's coordinate pair |
//! | colour | series identity | a **colour scale applied to the value** |
//! | pointer | nearest index | **the cell under the pointer** |
//!
//! The last two rows are what make it a distinct control rather than a style: a heat map's
//! whole purpose is that the colour *is* the reading, and pointing at it selects a
//! coordinate rather than a position in a sequence.
//!
//! # Why the colour scale is a data colour (rule #108, class ③)
//!
//! The ramp from low to high is the *encoding*. A caller reads a cell's value from its
//! colour, so remapping the ramp on a theme switch would change the data being displayed —
//! exactly the mistake class ③ exists to prevent. It is therefore built from constant
//! endpoints and **does not** read `style.background_color` / `theme.colors.*`. The frame
//! around it (background, border, labels) *is* chrome and does read the style, which is what
//! makes the control respond to a theme switch without lying about its data.
//!
//! # Reachability
//!
//! Registered in the widget factory as `heatmap` (aliases `heat_map`, `heatmap_chart`), so
//! it is reachable by name from the declarative JSON path (`"heatmap"`), from a CSS
//! selector (`Heatmap`), and through the typed `create_heatmap` on `ControlBackend`.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Low end of the value ramp.
///
/// A constant, not a theme colour: this is the *datum's* encoding (rule #108 ③). See the
/// module docs for why.
const RAMP_LOW: Color = Color { r: 33, g: 68, b: 121, a: 255 };

/// Midpoint of the value ramp. Three stops rather than two so a mid-range value reads as
/// mid-range rather than as "almost low"; a two-stop ramp compresses every interesting
/// value into the top half of the colour range.
const RAMP_MID: Color = Color { r: 84, g: 172, b: 158, a: 255 };

/// High end of the value ramp. See [`RAMP_LOW`].
const RAMP_HIGH: Color = Color { r: 249, g: 216, b: 122, a: 255 };

/// Cell inset on each side, in pixels, so adjacent cells are separable without a per-cell
/// stroke (which would leave the grid looking like a table rather than a field).
const CELL_INSET: i32 = 1;

/// One value in the matrix.
///
/// A missing cell is `None` rather than `0.0`: on a heat map "no data" and "the minimum"
/// are different readings, and drawing them identically would invent a datum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatmapCell {
    /// The cell's value, or `None` when the coordinate has no datum.
    pub value: Option<f64>,
}

impl HeatmapCell {
    /// A cell holding `value`.
    pub fn new(value: f64) -> Self {
        // A non-finite value is not a reading. Storing it would place the cell outside the
        // ramp and produce a colour with no meaning, so it becomes "no data" — the same
        // answer the chart gives for a coordinate it was not told about.
        Self { value: if value.is_finite() { Some(value) } else { None } }
    }

    /// A cell with no datum.
    pub fn empty() -> Self {
        Self { value: None }
    }
}

/// A heat map: a matrix of values rendered as a grid of coloured cells.
///
/// # Data model
///
/// `row_labels[r]` names row `r`, `column_labels[c]` names column `c`, and
/// `values[r][c]` is the datum at their intersection. A row shorter than
/// `column_labels` leaves the trailing columns empty — the honest reading of "no value
/// supplied" at a coordinate the grid still has a cell for.
pub struct Heatmap {
    base: BaseWidget,
    row_labels: Vec<String>,
    column_labels: Vec<String>,
    values: Vec<Vec<HeatmapCell>>,
    /// The value mapped to [`RAMP_LOW`]; `None` means "derive from the data".
    scale_minimum: Option<f64>,
    /// The value mapped to [`RAMP_HIGH`]; `None` means "derive from the data".
    scale_maximum: Option<f64>,
    /// Whether row and column labels are drawn.
    show_labels: bool,
    /// Whether the legend strip is drawn.
    show_legend: bool,
    /// The cell under the pointer, as `(row, column)`, or `None` when the pointer is away.
    hovered_cell: Option<(usize, usize)>,
    /// Emitted when the pointer's cell changes, carrying `(row, column)`.
    pub cell_hovered: Signal1<(usize, usize)>,
    /// Emitted when a cell is clicked, carrying `(row, column)`.
    pub cell_clicked: Signal1<(usize, usize)>,
}

impl Heatmap {
    /// Creates a heat map with no rows, no columns and no values.
    ///
    /// Defaults: labels on, legend on, colour scale derived from the data.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Heatmap, geometry, "Heatmap"),
            row_labels: Vec::new(),
            column_labels: Vec::new(),
            values: Vec::new(),
            scale_minimum: None,
            scale_maximum: None,
            show_labels: true,
            show_legend: true,
            hovered_cell: None,
            cell_hovered: Signal1::new(),
            cell_clicked: Signal1::new(),
        }
    }

    /// Replaces the row and column labels and the value matrix together.
    ///
    /// They are set in one call because the three determine each other's shape: a matrix
    /// whose rows do not line up with the row labels is not a heat map, it is a rendering
    /// fault, and letting a caller set them separately makes that state reachable. Rows
    /// shorter than `column_labels` are padded with empty cells; extra rows beyond
    /// `row_labels` are dropped, so the drawn grid always matches the labels it is
    /// labelled with.
    pub fn set_data(
        &mut self,
        row_labels: Vec<String>,
        column_labels: Vec<String>,
        values: Vec<Vec<HeatmapCell>>,
    ) {
        let columns = column_labels.len();
        let rows = row_labels.len();
        self.values = values.into_iter().take(rows).map(|row| pad_row(row, columns)).collect();
        // A matrix with fewer rows than labels leaves the missing rows empty rather than
        // shifting the present ones up, which would mislabel every row after the gap.
        while self.values.len() < rows {
            self.values.push(pad_row(Vec::new(), columns));
        }
        self.row_labels = row_labels;
        self.column_labels = column_labels;
        self.hovered_cell = None;
        self.base.request_redraw();
    }

    /// Returns the row labels.
    pub fn row_labels(&self) -> &[String] {
        &self.row_labels
    }

    /// Returns the column labels.
    pub fn column_labels(&self) -> &[String] {
        &self.column_labels
    }

    /// Returns the number of rows.
    pub fn row_count(&self) -> usize {
        self.row_labels.len()
    }

    /// Returns the number of columns.
    pub fn column_count(&self) -> usize {
        self.column_labels.len()
    }

    /// Returns the cell at `(row, column)`, or `None` when the coordinate is outside the grid.
    ///
    /// A coordinate outside the grid and an empty cell are different answers on purpose:
    /// the first is a caller mistake, the second is a datum the grid knows is absent.
    pub fn cell(&self, row: usize, column: usize) -> Option<HeatmapCell> {
        self.values.get(row)?.get(column).copied()
    }

    /// Sets one cell's value, returning `false` when the coordinate is outside the grid.
    ///
    /// A `NaN` or infinite value clears the cell (see [`HeatmapCell::new`]) rather than
    /// being stored, so the ramp can never be driven outside its range by one bad write.
    pub fn set_cell(&mut self, row: usize, column: usize, value: f64) -> bool {
        let Some(cells) = self.values.get_mut(row) else {
            return false;
        };
        let Some(cell) = cells.get_mut(column) else {
            return false;
        };
        *cell = HeatmapCell::new(value);
        self.base.request_redraw();
        true
    }

    /// Clears one cell, returning `false` when the coordinate is outside the grid.
    pub fn clear_cell(&mut self, row: usize, column: usize) -> bool {
        let Some(cells) = self.values.get_mut(row) else {
            return false;
        };
        let Some(cell) = cells.get_mut(column) else {
            return false;
        };
        *cell = HeatmapCell::empty();
        self.base.request_redraw();
        true
    }

    /// Returns the explicit colour-scale minimum, or `None` when it is derived from the data.
    pub fn scale_minimum(&self) -> Option<f64> {
        self.scale_minimum
    }

    /// Pins the value that maps to the low end of the ramp.
    ///
    /// Pinning matters when several heat maps are compared side by side: with a derived
    /// scale, the same colour means a different value on each, and the comparison is
    /// meaningless. Passing `None` restores the derived scale.
    pub fn set_scale_minimum(&mut self, minimum: Option<f64>) {
        self.scale_minimum = minimum.filter(|value| value.is_finite());
        self.base.request_redraw();
    }

    /// Returns the explicit colour-scale maximum, or `None` when it is derived from the data.
    pub fn scale_maximum(&self) -> Option<f64> {
        self.scale_maximum
    }

    /// Pins the value that maps to the high end of the ramp. See [`Self::set_scale_minimum`].
    pub fn set_scale_maximum(&mut self, maximum: Option<f64>) {
        self.scale_maximum = maximum.filter(|value| value.is_finite());
        self.base.request_redraw();
    }

    /// Returns whether labels are drawn.
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    /// Sets whether labels are drawn.
    pub fn set_show_labels(&mut self, show: bool) {
        self.show_labels = show;
        self.base.request_redraw();
    }

    /// Returns whether the legend is drawn.
    pub fn show_legend(&self) -> bool {
        self.show_legend
    }

    /// Sets whether the legend is drawn.
    pub fn set_show_legend(&mut self, show: bool) {
        self.show_legend = show;
        self.base.request_redraw();
    }

    /// The lowest and highest values the ramp must span.
    ///
    /// Returns `None` when there is no datum at all: an all-empty grid has no range, and
    /// inventing `0.0..1.0` for it would give every cell a colour meaning "the middle",
    /// which is a reading the data does not support.
    ///
    /// A pinned bound alone (only one of the two set) is honoured, with the other end taken
    /// from the data, so a caller can fix one end without having to know the other.
    pub fn scale_range(&self) -> Option<(f64, f64)> {
        let derived = self.data_range();
        let low = self.scale_minimum.or(derived.map(|(low, _)| low));
        let high = self.scale_maximum.or(derived.map(|(_, high)| high));
        match (low, high) {
            (Some(low), Some(high)) => Some((low, high)),
            // One end pinned and no data to supply the other: there is no range.
            _ => None,
        }
    }

    /// The lowest and highest present values in the matrix, ignoring empty cells.
    fn data_range(&self) -> Option<(f64, f64)> {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        let mut seen = false;
        for row in &self.values {
            for cell in row {
                if let Some(value) = cell.value {
                    low = low.min(value);
                    high = high.max(value);
                    seen = true;
                }
            }
        }
        if seen {
            Some((low, high))
        } else {
            None
        }
    }

    /// The colour for `value` under the active scale.
    ///
    /// A pinned range that does not contain the value still produces an endpoint colour
    /// rather than no colour: clamping is the honest answer for "this is at least as
    /// extreme as the top of the scale", and the alternative (a hole in the grid) would
    /// read as missing data.
    fn color_for_value(&self, value: f64) -> Color {
        let Some((low, high)) = self.scale_range() else {
            // The caller pinned a bound but supplied no values to interpolate against.
            // The midpoint is the only non-arbitrary answer.
            return RAMP_MID;
        };
        let span = high - low;
        if span <= 0.0 {
            // A single-valued or fully pinned-degenerate range: every cell is the top of
            // the scale, because every cell is the maximum.
            return RAMP_HIGH;
        }
        let fraction = ((value - low) / span).clamp(0.0, 1.0) as f32;
        interpolate_ramp(fraction)
    }

    /// The rectangle the value grid occupies, excluding the label margins and the legend.
    fn grid_rect(&self) -> Rect {
        let full = self.geometry();
        let left = if self.show_labels { LABEL_GUTTER } else { 0 };
        let top = if self.show_labels { LABEL_GUTTER } else { 0 };
        let bottom = if self.show_legend { LEGEND_HEIGHT } else { 0 };
        // A margin larger than the control must not produce a negative width: the grid
        // collapses to zero, which draws nothing instead of panicking or mirroring.
        let width = (full.width as i32 - left).max(0) as u32;
        let height = (full.height as i32 - top - bottom).max(0) as u32;
        Rect { x: full.x + left, y: full.y + top, width, height }
    }

    /// The grid cell under `pos`, or `None` when the pointer is outside the grid.
    fn cell_at(&self, pos: Point) -> Option<(usize, usize)> {
        let rows = self.row_count();
        let columns = self.column_count();
        if rows == 0 || columns == 0 {
            return None;
        }
        let grid = self.grid_rect();
        if grid.width == 0 || grid.height == 0 {
            return None;
        }
        let dx = pos.x - grid.x;
        let dy = pos.y - grid.y;
        if dx < 0 || dy < 0 || dx >= grid.width as i32 || dy >= grid.height as i32 {
            return None;
        }
        // Integer arithmetic, and the column is floored by the same expression the drawing
        // uses, so a click lands in the cell it looks like it landed in.
        let column = (dx as u64 * columns as u64 / grid.width as u64) as usize;
        let row = (dy as u64 * rows as u64 / grid.height as u64) as usize;
        Some((row.min(rows - 1), column.min(columns - 1)))
    }
}

/// Left/top gutter reserved for the row and column labels, in pixels.
const LABEL_GUTTER: i32 = 56;

/// Height of the legend band, in pixels: the colour ramp **plus** the row of endpoint
/// numbers below it.
///
/// The band has to hold two stacked things. The ramp is `RAMP_HEIGHT` tall, then `RAMP_GAP`,
/// then the endpoint numbers occupy their own line box. The line box is measured at draw
/// time (`Heatmap::legend_label_band`), so this constant is the *reservation* the layout
/// makes and `LEGEND_LABEL_HEIGHT` is the minimum line box it assumes for it.
///
/// The two used to be equated while the labels are drawn at 14 px, so the numbers' glyph box
/// was 4 px taller than the room reserved for it and its top 3 px landed on the ramp. The
/// drawing code now places the numbers from the ramp's bottom edge rather than from the
/// band's, so the two can no longer overlap; this value stays the layout's own reservation.
const LEGEND_HEIGHT: i32 = RAMP_HEIGHT + RAMP_GAP + LEGEND_LABEL_HEIGHT;

/// Height of the legend's colour ramp, in pixels.
const RAMP_HEIGHT: i32 = 8;
/// Gap between the ramp and the endpoint numbers below it.
const RAMP_GAP: i32 = 2;
/// Fallback height of the endpoint-number line box.
///
/// The numbers are drawn in the control's own font, whose default line box is 14 px, so the
/// reservation is 14 and not 10. Reserving 10 for a 14 px glyph box is what let the higher
/// endpoint's top rows land on the ramp. The drawing code derives the row's position from
/// the ramp's bottom edge, so this value only sizes the *reservation* the grid leaves for the
/// legend, never the placement of the numbers themselves.
const LEGEND_LABEL_HEIGHT: i32 = 14;

/// Pads `row` to `columns` cells with empty cells.
fn pad_row(mut row: Vec<HeatmapCell>, columns: usize) -> Vec<HeatmapCell> {
    row.truncate(columns);
    while row.len() < columns {
        row.push(HeatmapCell::empty());
    }
    row
}

/// Blends the three-stop ramp at `fraction` in `[0, 1]`.
///
/// Piecewise-linear in RGB, which is what every heat-map ramp in circulation does: a
/// perceptual-space blend would be more accurate about brightness but would no longer match
/// the colours a caller chose at the stops.
///
/// The fraction is clamped here rather than at the callers. `color_for_value` already clamps
/// (a cell outside a pinned range must read as the endpoint), but the legend walks the
/// fraction arithmetically and a caller added later would not necessarily remember — and an
/// unclamped extrapolation produces a colour *outside* the ramp, which is a value the scale
/// cannot express. Clamping at the single definition removes the class.
fn interpolate_ramp(fraction: f32) -> Color {
    // A NaN fraction compares false against both bounds, so it is normalised explicitly;
    // otherwise it would reach the blend and produce a NaN colour component.
    let fraction = if fraction.is_nan() { 0.0 } else { fraction.clamp(0.0, 1.0) };
    let (from, to, local) = if fraction <= 0.5 {
        (RAMP_LOW, RAMP_MID, fraction * 2.0)
    } else {
        (RAMP_MID, RAMP_HIGH, (fraction - 0.5) * 2.0)
    };
    let blend = |a: u8, b: u8| -> u8 {
        let value = a as f32 + (b as f32 - a as f32) * local;
        // `round` rather than `as`, so the endpoints are exact and the middle is not
        // systematically one unit low.
        value.round().clamp(0.0, 255.0) as u8
    };
    Color { r: blend(from.r, to.r), g: blend(from.g, to.g), b: blend(from.b, to.b), a: 255 }
}

impl Widget for Heatmap {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Every cell needs room to be a cell; below ~12px a grid reads as noise. The floors
        // apply only to the *empty* case: a grid with data asks for its own shape, even when
        // that is narrower than the default, because the caller has told the control what
        // the grid is and the useful hint is the truth about it.
        let columns = if self.column_count() == 0 { DEFAULT_COLUMNS } else { self.column_count() };
        let rows = if self.row_count() == 0 { DEFAULT_ROWS } else { self.row_count() };
        Size::new(
            columns as u32 * MIN_CELL_SIZE + LABEL_GUTTER as u32,
            rows as u32 * MIN_CELL_SIZE + LABEL_GUTTER as u32 + LEGEND_HEIGHT as u32,
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// Columns assumed before any data is set, for [`Heatmap::size_hint`].
const DEFAULT_COLUMNS: usize = 4;

/// Rows assumed before any data is set, for [`Heatmap::size_hint`].
const DEFAULT_ROWS: usize = 3;

/// The smallest cell dimension that still reads as a cell, in pixels.
const MIN_CELL_SIZE: u32 = 12;

/// `Heatmap`'s property contract.
impl WidgetProperties for Heatmap {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "scale_minimum" => Ok(match self.scale_minimum() {
                Some(value) => CapabilityValue::Float(value),
                None => CapabilityValue::Null,
            }),
            "scale_maximum" => Ok(match self.scale_maximum() {
                Some(value) => CapabilityValue::Float(value),
                None => CapabilityValue::Null,
            }),
            "resolved_minimum" => Ok(match self.scale_range() {
                Some((low, _)) => CapabilityValue::Float(low),
                None => CapabilityValue::Null,
            }),
            "resolved_maximum" => Ok(match self.scale_range() {
                Some((_, high)) => CapabilityValue::Float(high),
                None => CapabilityValue::Null,
            }),
            "show_labels" => Ok(CapabilityValue::Bool(self.show_labels())),
            "show_legend" => Ok(CapabilityValue::Bool(self.show_legend())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scale_minimum" => {
                match value {
                    CapabilityValue::Null => self.set_scale_minimum(None),
                    other => self.set_scale_minimum(Some(expect_float(other)?)),
                }
                Ok(())
            }
            "scale_maximum" => {
                match value {
                    CapabilityValue::Null => self.set_scale_maximum(None),
                    other => self.set_scale_maximum(Some(expect_float(other)?)),
                }
                Ok(())
            }
            "show_labels" => {
                self.set_show_labels(expect_bool(value)?);
                Ok(())
            }
            "show_legend" => {
                self.set_show_legend(expect_bool(value)?);
                Ok(())
            }
            // The three data-bearing properties are written through `set_data`; a scalar
            // write cannot express a matrix, and accepting one would have to guess at
            // which cell it meant.
            "row_count" | "column_count" | "resolved_minimum" | "resolved_maximum" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `HEATMAP_PROPERTIES`.
        property_names_of![
            "row_count",
            "column_count",
            "scale_minimum",
            "scale_maximum",
            "resolved_minimum",
            "resolved_maximum",
            "show_labels",
            "show_legend",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `heatmap` publishes.
    ///
    /// `clear` empties every cell: it is the one mutating action with no payload and no
    /// ambiguity, which is the shape a published command has to have. `set_data` and
    /// `set_cell` need a payload, so they are answered through the property route — and
    /// refused here rather than guessed at.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                for row in &mut self.values {
                    for cell in row {
                        *cell = HeatmapCell::empty();
                    }
                }
                self.hovered_cell = None;
                self.base.request_redraw();
                Ok(())
            }
            "set_data" | "set_cell" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

/// Accepts either numeric carrier for a `Float`-declared property, so a JSON `3` and a
/// JSON `3.5` both work and neither is a type error.
fn expect_float(value: CapabilityValue) -> Result<f64, CapabilityAccessError> {
    match value {
        CapabilityValue::Float(float) => Ok(float),
        CapabilityValue::Int(int) => Ok(int as f64),
        CapabilityValue::UInt(uint) => Ok(uint as f64),
        CapabilityValue::String(text) => {
            // A pinned scale bound is a number; an unparsable string is reported as the
            // type mismatch it is rather than silently ignored.
            text.parse::<f64>().map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

impl Draw for Heatmap {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        // Chrome reads the style; the ramp does not (rule #108 ③). See the module docs.
        let background = style.background_color.unwrap_or(Color::rgb(30, 30, 30));
        let border = style.border_color.unwrap_or(Color::rgb(130, 130, 130));
        let text_color = style.text_color.unwrap_or(Color::rgb(225, 225, 225));
        let default_font = Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);

        context.fill_rect(rect, background);
        if let Some(border_color) = style.border_color {
            context.draw_rect(rect, border_color);
        }

        let rows = self.row_count();
        let columns = self.column_count();
        let grid = self.grid_rect();
        if rows == 0 || columns == 0 || grid.width == 0 || grid.height == 0 {
            // Nothing to plot. The frame above still painted, so the control is visible
            // (P1/P2) and a caller sees where it will go — rather than the census having
            // to excuse a control that draws nothing at all.
            context.draw_text(
                Point::new(rect.x + 6, rect.y + rect.height as i32 / 2),
                "Heatmap",
                font,
                text_color,
                HorizontalAlignment::Left,
            );
            return;
        }

        let cell_width = grid.width as i32 / columns as i32;
        let cell_height = grid.height as i32 / rows as i32;
        if cell_width <= 0 || cell_height <= 0 {
            return;
        }

        // Cells are laid out from the grid origin with integer arithmetic, and the last
        // cell is extended to the grid's edge so rounding does not leave a seam. The
        // occupancy test uses the same expression as `cell_at`, so a click lands in the
        // cell it looks like it landed in.
        for row in 0..rows {
            let y = grid.y + row as i32 * cell_height;
            for column in 0..columns {
                let x = grid.x + column as i32 * cell_width;
                let width = if column + 1 == columns {
                    grid.width as i32 - column as i32 * cell_width
                } else {
                    cell_width
                };
                let height = if row + 1 == rows {
                    grid.height as i32 - row as i32 * cell_height
                } else {
                    cell_height
                };
                let cell_rect = Rect {
                    x: x + CELL_INSET,
                    y: y + CELL_INSET,
                    width: (width - CELL_INSET * 2).max(1) as u32,
                    height: (height - CELL_INSET * 2).max(1) as u32,
                };
                let value = self.cell(row, column).and_then(|cell| cell.value);
                let fill = match value {
                    Some(value) => self.color_for_value(value),
                    // "No datum" is drawn as the frame's own colour, which is a hole rather
                    // than a colour on the ramp — the two must not be confusable.
                    None => background,
                };
                context.fill_rect(cell_rect, fill);
                if self.hovered_cell == Some((row, column)) {
                    // The hover ring is chrome: it marks where the pointer is, not what the
                    // value is, so it uses the border colour.
                    context.draw_rect(cell_rect, border);
                }
            }
        }

        if self.show_labels {
            self.draw_labels(context, grid, font, text_color, cell_height);
        }
        if self.show_legend {
            self.draw_legend(context, rect, grid, font, text_color);
        }
    }
}

impl Heatmap {
    /// Draws the row labels down the left gutter and the column labels across the top.
    fn draw_labels(
        &self,
        context: &mut RenderContext,
        grid: Rect,
        font: &Font,
        text_color: Color,
        cell_height: i32,
    ) {
        let rows = self.row_count();
        let columns = self.column_count();
        let cell_width = grid.width as i32 / columns.max(1) as i32;
        let font_height = context.measure_text("M", font).height as i32;
        for row in 0..rows {
            let y = grid.y + row as i32 * cell_height + cell_height / 2;
            // The row gutter is exactly `LABEL_GUTTER` wide and the label starts at its
            // left edge, so the room available is the gutter itself — not the distance to
            // the grid, which is the same number and would drift if the two ever stopped
            // being equal.
            context.draw_text_fitted(
                Rect::new(grid.x - LABEL_GUTTER, y, LABEL_GUTTER as u32, font_height.max(1) as u32),
                &self.row_labels[row],
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
        let full = self.geometry();
        for column in 0..columns {
            // Centred on the cell so the label cannot be read as belonging to its
            // neighbour, which is what left-aligning in a narrow cell does. The box is the
            // cell itself, so a label wider than its column is fitted to the column rather
            // than centring on the cell and running into (or past) the one beside it.
            let cell_x = grid.x + column as i32 * cell_width;
            let width = if column + 1 == columns {
                grid.width as i32 - column as i32 * cell_width
            } else {
                cell_width
            };
            // The labels sit in the gutter above the grid, whose top is `grid.y`; the box
            // is clamped to the control so the first row of text cannot start above it.
            let top = (grid.y - font_height).max(full.y);
            let height = (grid.y - top).max(1) as u32;
            context.draw_text_fitted(
                Rect::new(cell_x, top, width.max(1) as u32, height),
                &self.column_labels[column],
                font,
                text_color,
                HorizontalAlignment::Center,
            );
        }
    }

    /// Draws the ramp strip and its end values along the bottom edge.
    ///
    /// The legend is what makes the colour readable as a value, so the two end numbers are
    /// part of the control rather than decoration: without them a cell's colour says only
    /// "higher" or "lower", not "how much".
    fn draw_legend(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        grid: Rect,
        font: &Font,
        text_color: Color,
    ) {
        let strip_height = RAMP_HEIGHT;
        // The legend band is stacked from its own top edge in the order it is read: ramp,
        // then gap, then the endpoint numbers. Every offset is derived from the band's top
        // plus the sizes of the parts above it, so the stack cannot drift out of the band
        // when a part's size changes.
        let band_top = rect.y + rect.height as i32 - LEGEND_HEIGHT;
        let y = band_top;
        let strip_x = grid.x;
        let strip_width = grid.width as i32;
        if strip_width <= 0 {
            return;
        }
        // One segment per pixel of the strip: an exact ramp rather than a sampled one, so
        // the legend cannot disagree with the colours the cells use.
        let steps = strip_width.min(512);
        let segment = (strip_width as f32 / steps as f32).max(1.0);
        for step in 0..steps {
            let fraction = step as f32 / (steps - 1).max(1) as f32;
            let x = strip_x + (step as f32 * segment).round() as i32;
            let width = segment.round().max(1.0) as u32;
            context.fill_rect(
                Rect { x, y, width, height: strip_height as u32 },
                interpolate_ramp(fraction),
            );
        }
        let Some((low, high)) = self.scale_range() else {
            return;
        };
        // The two endpoint numbers take the row below the ramp.
        //
        // Two shapes of this were wrong before. Anchoring at `strip_bottom + 1` laid the
        // row *past* the reservation so it ran off the control. Anchoring at
        // `band_bottom - label_height` tied it to the band's bottom, and since the labels are
        // drawn at 14 px while the reservation assumed 10, the origin moved up into the ramp:
        // the higher endpoint sat on the ramp colour beneath it and measured 1.15:1. Placing
        // the row after the ramp's own bottom edge makes the two adjacent by construction,
        // and `LEGEND_HEIGHT` reserves the line box the font actually has.
        let label_height = context.measure_text("0", font).height;
        let label_top = y + strip_height + RAMP_GAP;
        let label_band_height = label_height.max(1);
        context.draw_text_fitted(
            Rect::new(strip_x, label_top, strip_width.max(1) as u32, label_band_height),
            &format_endpoint(low),
            font,
            text_color,
            HorizontalAlignment::Left,
        );
        let high_text = format_endpoint(high);
        context.draw_text_fitted(
            Rect::new(strip_x, label_top, strip_width.max(1) as u32, label_band_height),
            &high_text,
            font,
            text_color,
            HorizontalAlignment::Right,
        );
    }
}

/// Formats a scale endpoint for the legend, trimming a trailing `.0` so a whole-numbered
/// range reads as `10` rather than `10.0`.
fn format_endpoint(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1.0e15 {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

impl EventHandler for Heatmap {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos, .. } => {
                let cell = self.cell_at(*pos);
                if cell != self.hovered_cell {
                    self.hovered_cell = cell;
                    if let Some((row, column)) = cell {
                        self.cell_hovered.emit((row, column));
                    }
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button } if *button == 1 => {
                if let Some((row, column)) = self.cell_at(*pos) {
                    self.cell_clicked.emit((row, column));
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                if let Some((row, column)) = self.cell_at(*pos) {
                    self.cell_clicked.emit((row, column));
                }
            }
            // Leaving the control clears the readout: a stale highlight would claim the
            // pointer is somewhere it is not.
            Event::MouseLeave { .. } if self.hovered_cell.take().is_some() => {
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    /// A 3x4 grid with one value per coordinate, and a known range `1.0..=12.0`.
    fn sample() -> Heatmap {
        let mut heatmap = Heatmap::new(Rect::new(0, 0, 240, 140));
        let rows = vec!["R1".to_string(), "R2".to_string(), "R3".to_string()];
        let columns = vec!["C1".to_string(), "C2".to_string(), "C3".to_string(), "C4".to_string()];
        let values = vec![
            vec![
                HeatmapCell::new(1.0),
                HeatmapCell::new(2.0),
                HeatmapCell::new(3.0),
                HeatmapCell::new(4.0),
            ],
            vec![
                HeatmapCell::new(5.0),
                HeatmapCell::new(6.0),
                HeatmapCell::new(7.0),
                HeatmapCell::new(8.0),
            ],
            vec![
                HeatmapCell::new(9.0),
                HeatmapCell::new(10.0),
                HeatmapCell::new(11.0),
                HeatmapCell::new(12.0),
            ],
        ];
        heatmap.set_data(rows, columns, values);
        heatmap
    }

    /// Renders the control and returns the pixels that differ from `fill`.
    fn ink(widget: &mut Heatmap, fill: Color) -> Vec<(u8, u8, u8)> {
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 140), 1.0);
        backend.begin_frame(fill);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        widget.draw(&mut ctx);
        backend.end_frame();
        backend
            .frame_rgba()
            .chunks_exact(4)
            .filter(|px| (px[0], px[1], px[2]) != (fill.r, fill.g, fill.b))
            .map(|px| (px[0], px[1], px[2]))
            .collect()
    }

    /// Replaces the control's style with one that only sets a background, so the themed
    /// chrome is what differs between two renders.
    fn set_background(widget: &mut Heatmap, background: Color) {
        let style =
            crate::style::WidgetStyle { background_color: Some(background), ..Default::default() };
        widget.base_mut().set_style(style);
    }

    /// Q2 / P1: the `draw` body must actually call into the context. An empty `Draw`
    /// (which rule #5 forbids) would render nothing but the probe fill.
    #[test]
    fn heatmap_draw_paints_cells() {
        let mut heatmap = sample();
        let painted = ink(&mut heatmap, Color::rgb(255, 0, 255));
        assert!(
            painted.len() > 1000,
            "a filled 3x4 grid must paint far more than a frame; got {} pixels",
            painted.len()
        );
        // Distinct ramp colours, not one flat fill: the whole point of a heat map is that
        // the colour varies with the value.
        let mut distinct: Vec<(u8, u8, u8)> = painted.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert!(
            distinct.len() >= 6,
            "a 12-cell ramp over a 12-value range must produce several colours, got {}",
            distinct.len()
        );
    }

    /// Rule #108 ③: the cell colours are DATA and must not be remapped by the theme. The
    /// same data must produce the same cell colours under both appearances, while the
    /// chrome (background/border/text) *does* follow the style.
    #[test]
    fn heatmap_ramp_is_data_not_chrome() {
        // The ramp's endpoints are constants of the control, so the same value must map to
        // the same colour regardless of the style in force.
        let mut styled = sample();
        set_background(&mut styled, Color::rgb(250, 250, 250));
        let mut other = sample();
        set_background(&mut other, Color::rgb(10, 10, 10));

        // The cells: identical, because the value's colour is the datum.
        for value in [1.0, 3.0, 7.0, 12.0] {
            assert_eq!(
                styled.color_for_value(value),
                other.color_for_value(value),
                "the ramp must not move with the theme — it encodes the value"
            );
        }
        assert_eq!(styled.color_for_value(1.0), RAMP_LOW);
        assert_eq!(styled.color_for_value(12.0), RAMP_HIGH);

        // The chrome: different, because the frame is the control's appearance. The two
        // renders must therefore differ *as images*, which is what a theme switch means.
        let light = ink(&mut styled, Color::rgb(255, 0, 255));
        let dark = ink(&mut other, Color::rgb(255, 0, 255));
        assert_ne!(
            light, dark,
            "the themed frame must change with the style even though the cells do not"
        );
        // And a specific chrome pixel (the top-left corner, inside the label gutter rather
        // than inside any cell) is the background that was set.
        assert!(light.contains(&(250, 250, 250)), "the light frame must paint its background");
        assert!(dark.contains(&(10, 10, 10)), "the dark frame must paint its background");
    }

    /// The two axes are categorical and independent: a cell's identity is its coordinate
    /// pair, not a position in a flattened sequence.
    #[test]
    fn heatmap_cell_is_addressed_by_coordinate_pair() {
        let heatmap = sample();
        assert_eq!(heatmap.cell(0, 0).unwrap().value, Some(1.0));
        assert_eq!(heatmap.cell(2, 3).unwrap().value, Some(12.0));
        assert_eq!(heatmap.cell(1, 2).unwrap().value, Some(7.0));
        // Out of range is a different answer from "no datum".
        assert_eq!(heatmap.cell(3, 0), None, "past the last row");
        assert_eq!(heatmap.cell(0, 4), None, "past the last column");
    }

    /// "No datum" and "the minimum" must not be the same pixel, or the control invents a
    /// reading. An empty cell is painted as the background (a hole), not as the ramp floor.
    #[test]
    fn heatmap_distinguishes_no_data_from_the_minimum() {
        let mut heatmap = Heatmap::new(Rect::new(0, 0, 240, 140));
        set_background(&mut heatmap, Color::rgb(30, 30, 30));
        heatmap.set_data(
            vec!["R1".to_string()],
            vec!["C1".to_string(), "C2".to_string()],
            vec![vec![HeatmapCell::new(0.0), HeatmapCell::empty()]],
        );
        assert_eq!(heatmap.cell(0, 0).unwrap().value, Some(0.0));
        assert_eq!(heatmap.cell(0, 1).unwrap().value, None);
        // With one value the range is degenerate, so the single present cell is the top of
        // the ramp and the empty one is the background. They must not coincide.
        let present = heatmap.color_for_value(0.0);
        let background = Color::rgb(30, 30, 30);
        assert_ne!(present, background, "a present value must be visible against the frame");
    }

    /// A non-finite value is not a reading. Storing it would drive the ramp outside its
    /// range and paint a colour with no meaning.
    #[test]
    fn heatmap_rejects_non_finite_values() {
        assert_eq!(HeatmapCell::new(f64::NAN).value, None);
        assert_eq!(HeatmapCell::new(f64::INFINITY).value, None);
        assert_eq!(HeatmapCell::new(f64::NEG_INFINITY).value, None);
        let mut heatmap = sample();
        assert!(heatmap.set_cell(0, 0, f64::NAN));
        assert_eq!(heatmap.cell(0, 0).unwrap().value, None, "NaN must clear, not store");
        assert!(!heatmap.set_cell(9, 9, 1.0), "an out-of-range write reports failure");
        assert!(!heatmap.clear_cell(9, 9), "and so does an out-of-range clear");
    }

    /// Pinning the scale is what makes two heat maps comparable: the same colour must mean
    /// the same value on both. A derived scale would silently reshuffle the colours.
    #[test]
    fn heatmap_pinned_scale_maps_equal_values_to_equal_colours() {
        let mut narrow = Heatmap::new(Rect::new(0, 0, 240, 140));
        narrow.set_data(
            vec!["R1".to_string()],
            vec!["C1".to_string(), "C2".to_string()],
            vec![vec![HeatmapCell::new(4.0), HeatmapCell::new(6.0)]],
        );
        let mut wide = Heatmap::new(Rect::new(0, 0, 240, 140));
        wide.set_data(
            vec!["R1".to_string()],
            vec!["C1".to_string(), "C2".to_string()],
            vec![vec![HeatmapCell::new(0.0), HeatmapCell::new(10.0)]],
        );
        // Derived: the two ranges place 5.0 at different fractions. In the narrow range
        // (4..6) it is the midpoint; in the wide one (0..10) it is also the midpoint — so
        // this pair is deliberately chosen to *agree* at 5.0 and to disagree elsewhere.
        // The disagreement is what the pinned scale removes.
        assert_ne!(
            narrow.color_for_value(4.5),
            wide.color_for_value(4.5),
            "the same value must sit at different fractions of two derived ranges"
        );
        // Pinned to a shared scale: they agree everywhere, not only at the midpoint.
        narrow.set_scale_minimum(Some(0.0));
        narrow.set_scale_maximum(Some(10.0));
        wide.set_scale_minimum(Some(0.0));
        wide.set_scale_maximum(Some(10.0));
        for value in [0.0, 2.5, 4.5, 5.0, 7.5, 10.0] {
            assert_eq!(
                narrow.color_for_value(value),
                wide.color_for_value(value),
                "on a shared scale {value} must be the same colour on both"
            );
        }
        assert_eq!(narrow.scale_range(), Some((0.0, 10.0)));
        assert_eq!(narrow.color_for_value(5.0), RAMP_MID, "the midpoint of the ramp");
        assert_eq!(narrow.color_for_value(0.0), RAMP_LOW);
        assert_eq!(narrow.color_for_value(10.0), RAMP_HIGH);
    }

    /// An all-empty grid has no range, and inventing one would give every absent cell a
    /// colour that means "the middle".
    #[test]
    fn heatmap_empty_grid_has_no_scale() {
        let heatmap = Heatmap::new(Rect::new(0, 0, 240, 140));
        assert_eq!(heatmap.scale_range(), None);
        let mut with_labels = Heatmap::new(Rect::new(0, 0, 240, 140));
        with_labels.set_data(
            vec!["R1".to_string()],
            vec!["C1".to_string()],
            vec![vec![HeatmapCell::empty()]],
        );
        assert_eq!(with_labels.scale_range(), None, "labels alone are not a range");
    }

    /// A pinned range that does not contain a value clamps to the endpoint rather than
    /// producing no colour: "at least as extreme as the top" is a reading, a hole is not.
    #[test]
    fn heatmap_values_outside_a_pinned_range_clamp() {
        let mut heatmap = sample();
        heatmap.set_scale_minimum(Some(4.0));
        heatmap.set_scale_maximum(Some(8.0));
        assert_eq!(heatmap.color_for_value(1.0), RAMP_LOW, "below the floor clamps to low");
        assert_eq!(heatmap.color_for_value(12.0), RAMP_HIGH, "above the ceiling clamps to high");
    }

    /// The pointer must land in the cell it looks like it landed in, and clicking must
    /// report that same coordinate.
    #[test]
    fn heatmap_pointer_maps_to_the_cell_under_it() {
        let heatmap = sample();
        let grid = heatmap.grid_rect();
        // The centre of each cell, computed from the same geometry the drawing uses.
        for row in 0..3usize {
            for column in 0..4usize {
                let x = grid.x + (column as u32 * grid.width / 4) as i32 + 2;
                let y = grid.y + (row as u32 * grid.height / 3) as i32 + 2;
                assert_eq!(
                    heatmap.cell_at(Point::new(x, y)),
                    Some((row, column)),
                    "the centre of cell ({row}, {column}) must resolve to it"
                );
            }
        }
        // Outside the grid is `None`, not a clamped coordinate.
        assert_eq!(heatmap.cell_at(Point::new(grid.x - 5, grid.y + 5)), None);
        assert_eq!(heatmap.cell_at(Point::new(grid.x + 5, grid.y - 5)), None);
        assert_eq!(heatmap.cell_at(Point::new(grid.x + grid.width as i32 + 5, grid.y + 5)), None);
    }

    /// A hover must emit its coordinate, and leaving must clear the readout: a stale
    /// highlight claims the pointer is somewhere it is not.
    #[test]
    fn heatmap_hover_emits_and_leave_clears() {
        use crate::compat::Arc;
        use core::sync::atomic::{AtomicI64, Ordering};
        let mut heatmap = sample();
        let seen = Arc::new(AtomicI64::new(-1));
        let sink = seen.clone();
        heatmap.cell_hovered.connect(move |payload| {
            let (row, column) = *payload;
            sink.store((row * 16 + column) as i64, Ordering::SeqCst);
        });
        let grid = heatmap.grid_rect();
        heatmap.handle_event(&Event::MouseMove { pos: Point::new(grid.x + 3, grid.y + 3) });
        assert_eq!(seen.load(Ordering::SeqCst), 0, "the first cell is (0, 0)");
        // Moving within the same cell must not re-emit.
        heatmap.handle_event(&Event::MouseMove { pos: Point::new(grid.x + 4, grid.y + 4) });
        assert_eq!(seen.load(Ordering::SeqCst), 0);
        heatmap.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert_eq!(heatmap.hovered_cell, None, "leaving clears the highlight");
    }

    /// A click reports the coordinate, and a click outside the grid reports nothing.
    #[test]
    fn heatmap_click_reports_the_cell() {
        use crate::compat::Arc;
        use core::sync::atomic::{AtomicI64, Ordering};
        let mut heatmap = sample();
        let seen = Arc::new(AtomicI64::new(-1));
        let sink = seen.clone();
        heatmap.cell_clicked.connect(move |payload| {
            let (row, column) = *payload;
            sink.store((row * 16 + column) as i64, Ordering::SeqCst);
        });
        let grid = heatmap.grid_rect();
        // The last cell: row 2, column 3.
        let x = grid.x + grid.width as i32 - 3;
        let y = grid.y + grid.height as i32 - 3;
        heatmap.handle_event(&Event::MousePress { pos: Point::new(x, y), button: 1 });
        assert_eq!(seen.load(Ordering::SeqCst), 2 * 16 + 3);
        seen.store(-1, Ordering::SeqCst);
        heatmap.handle_event(&Event::MousePress {
            pos: Point::new(grid.x - 20, grid.y - 20),
            button: 1,
        });
        assert_eq!(seen.load(Ordering::SeqCst), -1, "a click outside the grid emits nothing");
    }

    /// A disabled control must not report interaction, which is the contract every other
    /// control honours.
    #[test]
    fn heatmap_disabled_ignores_pointer_events() {
        use crate::compat::Arc;
        use core::sync::atomic::{AtomicBool, Ordering};
        let mut heatmap = sample();
        heatmap.base_mut().set_enabled(false);
        let fired = Arc::new(AtomicBool::new(false));
        let flag = fired.clone();
        heatmap.cell_clicked.connect(move |_| flag.store(true, Ordering::SeqCst));
        let grid = heatmap.grid_rect();
        heatmap.handle_event(&Event::MousePress {
            pos: Point::new(grid.x + 3, grid.y + 3),
            button: 1,
        });
        assert!(!fired.load(Ordering::SeqCst), "a disabled heat map must not emit");
    }

    /// The grid must collapse rather than underflow when the margins exceed the control,
    /// which is the state a designer reaches by dragging a control to a sliver.
    #[test]
    fn heatmap_tiny_geometry_does_not_panic() {
        let mut heatmap = Heatmap::new(Rect::new(0, 0, 4, 4));
        heatmap.set_data(
            vec!["R1".to_string()],
            vec!["C1".to_string()],
            vec![vec![HeatmapCell::new(1.0)]],
        );
        let grid = heatmap.grid_rect();
        assert_eq!(grid.width, 0, "a control narrower than its margins has no grid");
        assert_eq!(heatmap.cell_at(Point::new(1, 1)), None);
        let _ = ink(&mut heatmap, Color::rgb(255, 0, 255));
        // Zero-sized geometry too, which is a control that has not been laid out yet.
        let mut zero = Heatmap::new(Rect::new(0, 0, 0, 0));
        zero.set_data(
            vec!["R1".to_string()],
            vec!["C1".to_string()],
            vec![vec![HeatmapCell::new(1.0)]],
        );
        let _ = ink(&mut zero, Color::rgb(255, 0, 255));
    }

    /// `set_data` is the only writer of the three shapes together, so it must leave the
    /// grid square with its labels: a short row is padded, an extra row is dropped.
    #[test]
    fn heatmap_set_data_keeps_the_grid_square_with_its_labels() {
        let mut heatmap = Heatmap::new(Rect::new(0, 0, 240, 140));
        heatmap.set_data(
            vec!["R1".to_string(), "R2".to_string()],
            vec!["C1".to_string(), "C2".to_string(), "C3".to_string()],
            vec![
                // Short row: the trailing column must be empty, not absent.
                vec![HeatmapCell::new(1.0)],
                // Extra row: dropped, so the grid cannot be labelled wrongly.
                vec![HeatmapCell::new(2.0), HeatmapCell::new(3.0), HeatmapCell::new(4.0)],
                vec![HeatmapCell::new(9.0)],
            ],
        );
        assert_eq!(heatmap.row_count(), 2);
        assert_eq!(heatmap.column_count(), 3);
        assert_eq!(heatmap.cell(0, 0).unwrap().value, Some(1.0));
        assert_eq!(heatmap.cell(0, 1).unwrap().value, None, "a short row is padded with empty");
        assert_eq!(heatmap.cell(0, 2).unwrap().value, None);
        assert_eq!(heatmap.cell(1, 2).unwrap().value, Some(4.0));
        assert_eq!(heatmap.values.len(), 2, "the extra third row is dropped");
    }

    /// More labels than rows must leave the missing rows empty rather than shifting the
    /// present ones up, which would mislabel every row after the gap.
    #[test]
    fn heatmap_missing_rows_stay_empty() {
        let mut heatmap = Heatmap::new(Rect::new(0, 0, 240, 140));
        heatmap.set_data(
            vec!["R1".to_string(), "R2".to_string(), "R3".to_string()],
            vec!["C1".to_string()],
            vec![vec![HeatmapCell::new(5.0)]],
        );
        assert_eq!(heatmap.row_count(), 3);
        assert_eq!(heatmap.cell(0, 0).unwrap().value, Some(5.0));
        assert_eq!(heatmap.cell(1, 0).unwrap().value, None);
        assert_eq!(heatmap.cell(2, 0).unwrap().value, None);
    }

    /// The published properties must cover what the accessors expose, and the read-only
    /// ones must refuse a write rather than silently accepting one.
    #[test]
    fn heatmap_published_properties_round_trip() {
        use crate::widget::capability::WidgetProperties;
        let mut heatmap = sample();
        assert_eq!(heatmap.get("row_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(heatmap.get("column_count").unwrap(), CapabilityValue::UInt(4));
        assert_eq!(heatmap.get("show_labels").unwrap(), CapabilityValue::Bool(true));
        assert_eq!(heatmap.get("show_legend").unwrap(), CapabilityValue::Bool(true));
        // A derived scale is reported as null through the writable pair, and as the derived
        // number through the resolved pair — that is what makes it inspectable.
        assert_eq!(heatmap.get("scale_minimum").unwrap(), CapabilityValue::Null);
        assert_eq!(heatmap.get("resolved_minimum").unwrap(), CapabilityValue::Float(1.0));
        assert_eq!(heatmap.get("resolved_maximum").unwrap(), CapabilityValue::Float(12.0));

        assert_eq!(heatmap.set("show_labels", CapabilityValue::Bool(false)), Ok(()));
        assert_eq!(heatmap.get("show_labels").unwrap(), CapabilityValue::Bool(false));
        assert_eq!(heatmap.set("scale_minimum", CapabilityValue::Float(0.5)), Ok(()));
        assert_eq!(heatmap.get("scale_minimum").unwrap(), CapabilityValue::Float(0.5));
        assert_eq!(heatmap.set("scale_minimum", CapabilityValue::Null), Ok(()));
        assert_eq!(heatmap.get("scale_minimum").unwrap(), CapabilityValue::Null);
        // An integer is a legitimate spelling of a float bound, as it is for `spin_box`.
        assert_eq!(heatmap.set("scale_maximum", CapabilityValue::Int(20)), Ok(()));
        assert_eq!(heatmap.get("scale_maximum").unwrap(), CapabilityValue::Float(20.0));

        for read_only in ["row_count", "column_count", "resolved_minimum", "resolved_maximum"] {
            assert_eq!(
                heatmap.set(read_only, CapabilityValue::UInt(1)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{read_only} is derived and must refuse a write"
            );
        }
        // A string that is not a number is a type mismatch, not a silent zero.
        assert_eq!(
            heatmap.set("scale_minimum", CapabilityValue::String("abc".to_string())),
            Err(CapabilityAccessError::TypeMismatch)
        );
    }

    /// Every name in `property_names` must be answerable by `get`, or the control publishes
    /// a contract it cannot serve (BLUE20 layer 2, Q1).
    #[test]
    fn heatmap_every_published_name_is_readable() {
        use crate::widget::capability::WidgetProperties;
        let heatmap = sample();
        for name in heatmap.property_names() {
            assert!(heatmap.get(name).is_ok(), "`{name}` is published but cannot be read");
        }
    }

    /// The published commands must be recognised: `clear` runs, and the payload-carrying
    /// names say "supply a value" rather than pretending to have run.
    #[test]
    fn heatmap_commands_are_recognised() {
        use crate::widget::capability::WidgetProperties;
        let mut heatmap = sample();
        assert_eq!(heatmap.command("clear"), Ok(()));
        assert_eq!(heatmap.cell(0, 0).unwrap().value, None, "clear empties every cell");
        assert_eq!(heatmap.scale_range(), None, "an emptied grid has no range");
        assert_eq!(heatmap.command("set_data"), Err(CapabilityAccessError::OutOfRange));
        assert_eq!(heatmap.command("set_cell"), Err(CapabilityAccessError::OutOfRange));
        assert_eq!(
            heatmap.command("definitely_not_a_command"),
            Err(CapabilityAccessError::UnknownCommand)
        );
    }

    /// The width must follow the formatted text: a wider label needs a wider gutter, or it
    /// overflows into the cells it is labelling.
    #[test]
    fn heatmap_size_hint_follows_the_grid_shape() {
        let empty = Heatmap::new(Rect::new(0, 0, 100, 100));
        // An empty control asks for the default placeholder shape, so a designer can place
        // it before it has data.
        assert_eq!(empty.size_hint(), empty_default_hint());
        // A populated one asks for its own shape, which is the honest hint: the caller has
        // said what the grid is.
        let filled = sample();
        assert_eq!(
            filled.size_hint().width,
            4 * MIN_CELL_SIZE + LABEL_GUTTER as u32,
            "4 columns must be sized as 4 cells plus the label gutter"
        );
        assert_eq!(
            filled.size_hint().height,
            3 * MIN_CELL_SIZE + LABEL_GUTTER as u32 + LEGEND_HEIGHT as u32,
            "3 rows must be sized as 3 cells plus the gutter and the legend strip"
        );
        // A one-cell grid is narrower than the empty placeholder: the hint tells the truth
        // about the grid rather than padding it up to a floor.
        let mut single = Heatmap::new(Rect::new(0, 0, 100, 100));
        single.set_data(
            vec!["R1".to_string()],
            vec!["C1".to_string()],
            vec![vec![HeatmapCell::new(1.0)]],
        );
        assert!(single.size_hint().width < empty.size_hint().width);
    }

    /// The placeholder shape an empty heat map asks for.
    fn empty_default_hint() -> Size {
        Size::new(
            DEFAULT_COLUMNS as u32 * MIN_CELL_SIZE + LABEL_GUTTER as u32,
            DEFAULT_ROWS as u32 * MIN_CELL_SIZE + LABEL_GUTTER as u32 + LEGEND_HEIGHT as u32,
        )
    }

    /// The geometry delegation must be the base's, so the control participates in layout
    /// like every other one.
    #[test]
    fn heatmap_geometry_and_kind_delegate_to_the_base() {
        let heatmap = Heatmap::new(Rect::new(5, 6, 70, 80));
        assert_eq!(heatmap.geometry(), Rect::new(5, 6, 70, 80));
        assert_eq!(heatmap.kind(), WidgetKind::Heatmap);
        let other = Heatmap::new(Rect::new(0, 0, 10, 10));
        assert_ne!(heatmap.id(), other.id());
    }

    /// The legend must be skippable, and turning it off must give the cells the space back
    /// rather than leaving a gap where it was.
    #[test]
    fn heatmap_hidden_legend_returns_its_space_to_the_grid() {
        let mut heatmap = sample();
        let with_legend = heatmap.grid_rect();
        heatmap.set_show_legend(false);
        let without = heatmap.grid_rect();
        assert!(
            without.height > with_legend.height,
            "hiding the legend must give its strip back to the grid"
        );
    }

    /// The legend's endpoint labels must read as whole numbers where they are whole.
    #[test]
    fn heatmap_endpoint_labels_trim_whole_numbers() {
        assert_eq!(format_endpoint(10.0), "10");
        assert_eq!(format_endpoint(0.0), "0");
        assert_eq!(format_endpoint(-3.0), "-3");
        assert_eq!(format_endpoint(1.5), "1.50");
    }

    /// The ramp must be monotone in lightness so a higher value never looks like a lower
    /// one, and its endpoints must be exactly the declared constants.
    #[test]
    fn heatmap_ramp_is_monotone_and_anchored() {
        assert_eq!(interpolate_ramp(0.0), RAMP_LOW);
        assert_eq!(interpolate_ramp(0.5), RAMP_MID);
        assert_eq!(interpolate_ramp(1.0), RAMP_HIGH);
        let mut previous = interpolate_ramp(0.0);
        for step in 1..=20 {
            let current = interpolate_ramp(step as f32 / 20.0);
            let before = previous.r as u32 + previous.g as u32 + previous.b as u32;
            let after = current.r as u32 + current.g as u32 + current.b as u32;
            assert!(after >= before, "the ramp must not darken as the value rises");
            previous = current;
        }
        // Out-of-range fractions clamp rather than extrapolating outside the ramp.
        assert_eq!(interpolate_ramp(-1.0), RAMP_LOW);
        assert_eq!(interpolate_ramp(2.0), RAMP_HIGH);
    }
}
