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

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
            line_color: Some(Color::from_rgb(220, 220, 220)),
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
            line_color: Some(Color::from_rgb(220, 220, 220)),
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
}

// ── Draw ──────────────────────────────────────────────────

impl Draw for GridWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        self.update_cell_dimensions();

        // Background fill
        context.fill_rect(rect, Color::from_rgb(250, 250, 252));

        // Border
        context.draw_rect(rect, Color::from_rgb(180, 185, 195));

        // Grid lines (skip for 1×1, also skip if color is None)
        let Some(line_color) = self.line_color else {
            return;
        };
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
            _ => {}
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
