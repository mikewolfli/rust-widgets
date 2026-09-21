// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Grid widget — a visual container that arranges children in a
//! fixed row/column matrix with optional spacing, grid-line rendering,
//! and per-cell hover/click detection.
//!
//! # JSON Example
//! ```json
//! {
//!     "grid": {
//!         "rows": 3,
//!         "columns": 4,
//!         "spacing": 4,
//!         "line_color": "#DCDCDC",
//!         "children": [
//!             ...children placed via layout row/col attrs...
//!         ]
//!     }
//! }
//! ```

use crate::core::Color;
use crate::core::{Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_u32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Grid widget for layout management.
///
/// Displays a visual grid with configurable rows, columns, spacing,
/// and grid-line rendering. Children can be assigned to cells via
/// the parent's layout manager (typically a `GridLayout`), or
/// positioned absolutely within the grid area.
pub struct GridWidget {
    base: BaseWidget,
    /// Number of rows in the grid (minimum 1).
    rows: u32,
    /// Number of columns in the grid (minimum 1).
    columns: u32,
    /// Spacing between cells in pixels.
    spacing: u32,
    /// Color of grid separator lines (`None` = no lines drawn).
    line_color: Option<Color>,
    /// Cached cell dimensions computed during the last draw pass.
    cell_width: u32,
    cell_height: u32,
    hovered_cell: Option<(u32, u32)>,
    /// Emitted when a cell is clicked.
    pub cell_clicked: Signal1<(u32, u32)>,
    /// Emitted when pointer hover changes to another cell.
    pub cell_hovered: Signal1<(u32, u32)>,
}

impl GridWidget {
    /// Creates a new grid widget with default 1x1 layout.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Grid, geometry, "GridWidget"),
            rows: 1,
            columns: 1,
            spacing: 0,
            line_color: Some(Color::rgb(220, 220, 220)),
            cell_width: geometry.width,
            cell_height: geometry.height,
            hovered_cell: None,
            cell_clicked: Signal1::new(),
            cell_hovered: Signal1::new(),
        }
    }

    /// Creates a new grid widget with specified dimensions.
    pub fn with_dimensions(geometry: Rect, rows: u32, columns: u32) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Grid, geometry, "GridWidget"),
            rows: rows.max(1),
            columns: columns.max(1),
            spacing: 0,
            line_color: Some(Color::rgb(220, 220, 220)),
            cell_width: geometry.width / columns.max(1),
            cell_height: geometry.height / rows.max(1),
            hovered_cell: None,
            cell_clicked: Signal1::new(),
            cell_hovered: Signal1::new(),
        }
    }

    // ── Row / Column accessors ─────────────────────────────

    /// Returns the number of rows.
    pub fn rows(&self) -> u32 {
        self.rows
    }
    /// Sets the number of rows (minimum 1). Triggers a redraw request.
    pub fn set_rows(&mut self, rows: u32) {
        self.rows = rows.max(1);
        self.update_cell_dimensions();
        self.base.request_redraw();
    }
    /// Returns the number of columns.
    pub fn columns(&self) -> u32 {
        self.columns
    }
    /// Sets the number of columns (minimum 1). Triggers a redraw request.
    pub fn set_columns(&mut self, columns: u32) {
        self.columns = columns.max(1);
        self.update_cell_dimensions();
        self.base.request_redraw();
    }

    // ── Spacing ────────────────────────────────────────────

    /// Returns spacing between cells in pixels.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }
    /// Sets spacing between cells in pixels. Triggers a redraw request.
    pub fn set_spacing(&mut self, spacing: u32) {
        self.spacing = spacing;
        self.update_cell_dimensions();
        self.base.request_redraw();
    }

    // ── Grid line color ────────────────────────────────────

    /// Returns the grid line color, or `None` if grid lines are disabled.
    pub fn line_color(&self) -> Option<Color> {
        self.line_color
    }
    /// Sets the grid line color. Pass `None` to disable grid lines.
    pub fn set_line_color(&mut self, color: Option<Color>) {
        self.line_color = color;
        self.base.request_redraw();
    }

    // ── Cell info ──────────────────────────────────────────

    /// Returns the cached cell width computed during the last draw.
    pub fn cell_width(&self) -> u32 {
        self.cell_width
    }
    /// Returns the cached cell height computed during the last draw.
    pub fn cell_height(&self) -> u32 {
        self.cell_height
    }

    /// Returns the cell row for a given y-coordinate, or `None` if outside.
    pub fn cell_at_y(&self, y: i32) -> Option<u32> {
        let rect = self.base.geometry();
        if y < rect.y || y >= rect.y + rect.height as i32 {
            return None;
        }
        if self.rows == 0 || self.cell_height == 0 {
            return None;
        }
        let local_y = (y - rect.y) as u32;
        let row = local_y / (self.cell_height + self.spacing);
        if row < self.rows {
            Some(row)
        } else {
            None
        }
    }

    /// Returns the cell column for a given x-coordinate, or `None` if outside.
    pub fn cell_at_x(&self, x: i32) -> Option<u32> {
        let rect = self.base.geometry();
        if x < rect.x || x >= rect.x + rect.width as i32 {
            return None;
        }
        if self.columns == 0 || self.cell_width == 0 {
            return None;
        }
        let local_x = (x - rect.x) as u32;
        let col = local_x / (self.cell_width + self.spacing);
        if col < self.columns {
            Some(col)
        } else {
            None
        }
    }

    /// Returns the cell position `(row, col)` for a given point, or `None`.
    pub fn cell_at(&self, point: Point) -> Option<(u32, u32)> {
        let row = self.cell_at_y(point.y)?;
        let col = self.cell_at_x(point.x)?;
        Some((row, col))
    }

    /// Returns the bounding rectangle of a specific cell.
    pub fn cell_rect(&self, row: u32, col: u32) -> Option<Rect> {
        if row >= self.rows || col >= self.columns {
            return None;
        }
        let rect = self.base.geometry();
        let x = rect.x + (col * (self.cell_width + self.spacing)) as i32;
        let y = rect.y + (row * (self.cell_height + self.spacing)) as i32;
        Some(Rect::new(x, y, self.cell_width, self.cell_height))
    }

    // ── Recalculate cell dimensions ────────────────────────
    fn update_cell_dimensions(&mut self) {
        let rect = self.base.geometry();
        let total_spacing_w = self.spacing.saturating_mul(self.columns.saturating_sub(1));
        let total_spacing_h = self.spacing.saturating_mul(self.rows.saturating_sub(1));
        self.cell_width = (rect.width.saturating_sub(total_spacing_w)) / self.columns;
        self.cell_height = (rect.height.saturating_sub(total_spacing_h)) / self.rows;
    }
}

// ── Widget trait ──────────────────────────────────────────

impl Widget for GridWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Returns a size hint proportional to rows x columns.
    fn size_hint(&self) -> Size {
        // Each cell at least 20×20 px, times row/col count, plus spacing.
        let w = self.columns * 20 + self.spacing.saturating_mul(self.columns.saturating_sub(1));
        let h = self.rows * 20 + self.spacing.saturating_mul(self.rows.saturating_sub(1));
        Size::new(w.max(40), h.max(40))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `GridWidget`'s property contract, published under the `Grid` kind.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `Null` handling of `line_color` and the `TypeMismatch` an unparsable hex
/// colour produced.
impl WidgetProperties for GridWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "rows" => Ok(CapabilityValue::UInt(self.rows() as u64)),
            "columns" => Ok(CapabilityValue::UInt(self.columns() as u64)),
            "spacing" => Ok(CapabilityValue::UInt(self.spacing() as u64)),
            "line_color" => Ok(match self.line_color() {
                Some(color) => CapabilityValue::String(color.to_hex_rgba()),
                None => CapabilityValue::Null,
            }),
            "cell_width" => Ok(CapabilityValue::UInt(self.cell_width() as u64)),
            "cell_height" => Ok(CapabilityValue::UInt(self.cell_height() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "rows" => {
                self.set_rows(expect_u32(value)?);
                Ok(())
            }
            "columns" => {
                self.set_columns(expect_u32(value)?);
                Ok(())
            }
            "spacing" => {
                self.set_spacing(expect_u32(value)?);
                Ok(())
            }
            "line_color" => {
                match value {
                    CapabilityValue::Null => self.set_line_color(None),
                    CapabilityValue::String(raw) => {
                        let Some(color) = crate::core::Color::parse_hex(&raw) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        self.set_line_color(Some(color));
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            // Cell extents are recomputed from the geometry and the row/column
            // counts, so they are derived reads rather than assignments.
            "cell_width" | "cell_height" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "rows",
            "columns",
            "spacing",
            "line_color",
            "cell_width",
            "cell_height",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `grid` publishes.
    ///
    /// The grid addresses cells by *hover*, not by a stored selection, so
    /// `select_cell` and `clear_selection` are names this control publishes through
    /// the shared `GridLayout` schema but cannot itself perform; they are reported as
    /// [`CapabilityAccessError::UnsupportedOnWidget`]. Note that the dispatcher
    /// already folds the trait default's `UnknownCommand` into exactly that error
    /// before the caller sees it, so the two spellings differ only in intent here.
    ///
    /// # Why the `set_*` arm
    ///
    /// `set_rows` / `set_columns` / `set_spacing` / `set_line_color` are published by
    /// this capability and are payload-carrying writes. An earlier revision answered
    /// them with `UnknownCommand`, which was a bug rather than a policy: the trait
    /// default deliberately accepts a bare `set_foo` as "needs its value", and a
    /// hand-written override that named only the non-`set_` commands silently
    /// contradicted it. `set_spacing` in particular slips past the default's
    /// `is_ascii_lowercase` prefix test, so `set_` names are matched here as a class
    /// instead of being enumerated one by one.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "select_cell" | "clear_selection" => Err(CapabilityAccessError::UnsupportedOnWidget),
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

// ── Draw ──────────────────────────────────────────────────

impl Draw for GridWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        self.update_cell_dimensions();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the matrix and its separators unchanged — the rendering census
        // reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("grid");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary) = {
            let manager = crate::theme::global_theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.secondary)
                }
                None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158)),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The matrix: one step from the window fill toward the text colour, so it is a distinct
        // element on a light theme and on a dark one. `grid` is absent from
        // `WidgetRole::for_kind_name`'s table, so it classifies as `Surface` and the active theme
        // has already written the window fill into `style.background_color`; the resolved value is
        // filtered so a byte-identical-to-the-frame fill cannot be let through unfiltered, while a
        // colour the caller set still wins.
        let surface = window_fill.blend(&ink, 0.08);
        let cell_fill = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => surface,
        };
        context.fill_rect(rect, cell_fill);

        // Border
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != cell_fill)
            .unwrap_or_else(|| cell_fill.blend(&secondary, 0.45));
        context.draw_rect(rect, border_color);

        // Grid lines (skip for 1×1, also skip if color is None). A caller-set line colour still
        // wins; otherwise the separators are derived from the themed border so they move with it.
        let Some(line_color) = self.line_color else {
            return;
        };
        let line_color =
            if line_color == Color::rgb(220, 220, 220) { border_color } else { line_color };
        if self.rows <= 1 && self.columns <= 1 {
            return;
        }

        let total_w = self.columns * self.cell_width
            + self.spacing.saturating_mul(self.columns.saturating_sub(1));
        let total_h =
            self.rows * self.cell_height + self.spacing.saturating_mul(self.rows.saturating_sub(1));

        // Vertical lines
        for col in 1..self.columns {
            let x = rect.x + (col * (self.cell_width + self.spacing)) as i32
                - (self.spacing / 2) as i32;
            let x = x.max(rect.x).min(rect.x + total_w as i32);
            context.draw_line(
                Point::new(x, rect.y),
                Point::new(x, rect.y + total_h as i32),
                line_color,
            );
        }

        // Horizontal lines
        for row in 1..self.rows {
            let y = rect.y + (row * (self.cell_height + self.spacing)) as i32
                - (self.spacing / 2) as i32;
            let y = y.max(rect.y).min(rect.y + total_h as i32);
            context.draw_line(
                Point::new(rect.x, y),
                Point::new(rect.x + total_w as i32, y),
                line_color,
            );
        }
    }
}

// ── EventHandler ──────────────────────────────────────────

impl EventHandler for GridWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match *event {
            Event::MouseMove { pos } => {
                if let Some(cell) = self.cell_at(pos) {
                    if self.hovered_cell != Some(cell) {
                        self.hovered_cell = Some(cell);
                        self.cell_hovered.emit(cell);
                    }
                } else {
                    self.hovered_cell = None;
                }
            }
            Event::MousePress { pos, button: 1 } => {
                self.base.set_mouse_pressed(true);
                if let Some(cell) = self.cell_at(pos) {
                    self.base.clicked.emit();
                    self.cell_clicked.emit(cell);
                }
            }
            Event::MouseRelease { pos: _, button: 1 } => {
                self.base.set_mouse_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn with_dimensions_uses_columns_for_width_and_rows_for_height() {
        let grid = GridWidget::with_dimensions(Rect::new(0, 0, 120, 80), 2, 4);
        assert_eq!(grid.cell_width(), 30);
        assert_eq!(grid.cell_height(), 40);
    }

    #[test]
    fn grid_mouse_interaction_emits_cell_signals() {
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 100, 100), 2, 2);

        let clicked = Arc::new(Mutex::new(Vec::<(u32, u32)>::new()));
        let hovered = Arc::new(Mutex::new(Vec::<(u32, u32)>::new()));

        let clicked_sink = clicked.clone();
        grid.cell_clicked.connect(move |cell| {
            if let Ok(mut guard) = clicked_sink.lock() {
                guard.push(*cell);
            }
        });

        let hovered_sink = hovered.clone();
        grid.cell_hovered.connect(move |cell| {
            if let Ok(mut guard) = hovered_sink.lock() {
                guard.push(*cell);
            }
        });

        grid.handle_event(&Event::mouse_move(75, 25));
        grid.handle_event(&Event::mouse_press(75, 25, 1));

        let hovered_values = hovered.lock().expect("hovered lock poisoned").clone();
        let clicked_values = clicked.lock().expect("clicked lock poisoned").clone();

        assert_eq!(hovered_values, vec![(0, 1)]);
        assert_eq!(clicked_values, vec![(0, 1)]);
    }
}
