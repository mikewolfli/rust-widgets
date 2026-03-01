//! Layout managers.

use crate::core::{ObjectId, Rect};

pub trait Layout {
    /// Add widget into layout with optional stretch factor.
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    /// Remove widget from layout.
    fn remove_widget(&mut self, widget_id: ObjectId);
    /// Recompute child geometries within given rect.
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Main axis is horizontal.
    Horizontal,
    /// Main axis is vertical.
    Vertical,
}

pub struct BoxLayout {
    orientation: Orientation,
    spacing: u32,
    margin: u32,
    items: Vec<(ObjectId, u32)>,
}

impl BoxLayout {
    /// Create a box layout with orientation, spacing and margin.
    pub fn new(orientation: Orientation, spacing: u32, margin: u32) -> Self {
        Self { orientation, spacing, margin, items: Vec::new() }
    }
}

impl Layout for BoxLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.items.push((widget_id, stretch.max(1)));
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.items.retain(|(id, _)| *id != widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.items.is_empty() {
            return;
        }
        let gaps = (self.items.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width,
            Orientation::Vertical => rect.height,
        }
        .saturating_sub(self.margin * 2)
        .saturating_sub(gaps * self.spacing);

        let total_stretch: u32 = self.items.iter().map(|(_, stretch)| *stretch).sum::<u32>().max(1);
        let mut cursor_x = rect.x + self.margin as i32;
        let mut cursor_y = rect.y + self.margin as i32;

        for (widget_id, stretch) in &self.items {
            let major = primary.saturating_mul(*stretch) / total_stretch;
            let child_rect = match self.orientation {
                Orientation::Horizontal => Rect {
                    x: cursor_x,
                    y: cursor_y,
                    width: major,
                    height: rect.height.saturating_sub(self.margin * 2),
                },
                Orientation::Vertical => Rect {
                    x: cursor_x,
                    y: cursor_y,
                    width: rect.width.saturating_sub(self.margin * 2),
                    height: major,
                },
            };
            widgets(*widget_id, child_rect);
            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + self.spacing) as i32,
                Orientation::Vertical => cursor_y += (major + self.spacing) as i32,
            }
        }
    }
}

pub struct GridLayout {
    rows: u32,
    cols: u32,
    spacing: u32,
    margin: u32,
    cells: Vec<Option<ObjectId>>,
}

impl GridLayout {
    /// Create a grid layout with fixed rows/columns.
    pub fn new(rows: u32, cols: u32, spacing: u32, margin: u32) -> Self {
        let safe_rows = rows.max(1);
        let safe_cols = cols.max(1);
        Self {
            rows: safe_rows,
            cols: safe_cols,
            spacing,
            margin,
            cells: vec![None; (safe_rows * safe_cols) as usize],
        }
    }

    /// Assign widget to explicit cell.
    pub fn set_widget(&mut self, row: u32, col: u32, widget_id: ObjectId) {
        if row < self.rows && col < self.cols {
            self.cells[(row * self.cols + col) as usize] = Some(widget_id);
        }
    }
}

impl Layout for GridLayout {
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        if let Some(slot) = self.cells.iter_mut().find(|cell| cell.is_none()) {
            *slot = Some(widget_id);
        }
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        for cell in &mut self.cells {
            if *cell == Some(widget_id) {
                *cell = None;
            }
        }
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let cell_width = rect
            .width
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.cols - 1) * self.spacing)
            / self.cols;
        let cell_height = rect
            .height
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.rows - 1) * self.spacing)
            / self.rows;

        for row in 0..self.rows {
            for col in 0..self.cols {
                if let Some(widget_id) = self.cells[(row * self.cols + col) as usize] {
                    let x = rect.x + self.margin as i32 + (col * (cell_width + self.spacing)) as i32;
                    let y = rect.y + self.margin as i32 + (row * (cell_height + self.spacing)) as i32;
                    widgets(widget_id, Rect { x, y, width: cell_width, height: cell_height });
                }
            }
        }
    }
}

pub struct FormLayout {
    spacing: u32,
    margin: u32,
    rows: Vec<(ObjectId, ObjectId)>,
}

impl FormLayout {
    /// Create a two-column form layout.
    pub fn new(spacing: u32, margin: u32) -> Self {
        Self { spacing, margin, rows: Vec::new() }
    }

    /// Add one form row as `(label, field)` pair.
    pub fn add_row(&mut self, label_id: ObjectId, field_id: ObjectId) {
        self.rows.push((label_id, field_id));
    }
}

impl Layout for FormLayout {
    fn add_widget(&mut self, _widget_id: ObjectId, _stretch: u32) {}

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.rows.retain(|(label, field)| *label != widget_id && *field != widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.rows.is_empty() {
            return;
        }
        let row_height = rect
            .height
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.rows.len() as u32 - 1) * self.spacing)
            / self.rows.len() as u32;
        let label_width = rect.width / 3;
        let field_width = rect.width.saturating_sub(self.margin * 2 + label_width + self.spacing);

        for (index, (label, field)) in self.rows.iter().enumerate() {
            let y = rect.y + self.margin as i32 + index as i32 * (row_height + self.spacing) as i32;
            widgets(
                *label,
                Rect { x: rect.x + self.margin as i32, y, width: label_width, height: row_height },
            );
            widgets(
                *field,
                Rect {
                    x: rect.x + self.margin as i32 + label_width as i32 + self.spacing as i32,
                    y,
                    width: field_width,
                    height: row_height,
                },
            );
        }
    }
}

pub struct StackLayout {
    items: Vec<ObjectId>,
    current: usize,
}

impl StackLayout {
    /// Create stack layout with no pages.
    pub fn new() -> Self {
        Self { items: Vec::new(), current: 0 }
    }

    /// Select visible page by index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.items.len() {
            self.current = index;
        }
    }
}

impl Default for StackLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout for StackLayout {
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        self.items.push(widget_id);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.items.retain(|id| *id != widget_id);
        if self.current >= self.items.len() {
            self.current = self.items.len().saturating_sub(1);
        }
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if let Some(widget_id) = self.items.get(self.current) {
            widgets(*widget_id, rect);
        }
    }
}
