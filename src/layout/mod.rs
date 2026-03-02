//! Layout managers.

use crate::core::{ObjectId, Point, Rect, Size};

/// Space allocation preference used by layout items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizePolicy {
    /// Use fixed size defined by constraints.
    Fixed,
    /// Prefer natural size while allowing negotiation.
    Preferred,
    /// Expand to consume remaining space.
    Expanding,
}

/// Min/max limits applied during layout calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConstraints {
    /// Minimum major-axis size.
    pub min: u32,
    /// Optional maximum major-axis size.
    pub max: Option<u32>,
}

impl LayoutConstraints {
    /// Creates new layout constraints.
    pub fn new(min: u32, max: Option<u32>) -> Self {
        Self { min, max }
    }
}

/// Common interface implemented by all layout managers.
pub trait Layout {
    /// Add widget into layout with optional stretch factor.
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    /// Remove widget from layout.
    fn remove_widget(&mut self, widget_id: ObjectId);
    /// Recompute child geometries within given rect.
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
    /// Recompute child geometries from explicit position/size primitives.
    fn update_from_position_size(
        &self,
        position: Point,
        size: Size,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(Rect::from_position_size(position, size), widgets);
    }
}

/// Orientation used by directional layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Main axis is horizontal.
    Horizontal,
    /// Main axis is vertical.
    Vertical,
}

/// Linear layout that arranges items in one direction.
pub struct BoxLayout {
    orientation: Orientation,
    spacing: u32,
    margin: u32,
    items: Vec<BoxLayoutItem>,
}

struct BoxLayoutItem {
    widget_id: Option<ObjectId>,
    stretch: u32,
    constraints: LayoutConstraints,
    policy: SizePolicy,
}

impl BoxLayout {
    /// Create a box layout with orientation, spacing and margin.
    pub fn new(orientation: Orientation, spacing: u32, margin: u32) -> Self {
        Self { orientation, spacing, margin, items: Vec::new() }
    }

    /// Returns layout orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns inter-item spacing.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Updates inter-item spacing.
    pub fn set_spacing(&mut self, spacing: u32) {
        self.spacing = spacing;
    }

    /// Returns outer margin.
    pub fn margin(&self) -> u32 {
        self.margin
    }

    /// Updates outer margin.
    pub fn set_margin(&mut self, margin: u32) {
        self.margin = margin;
    }

    /// Returns number of managed items (widgets + spacers).
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Adds an empty spacer item with the provided stretch factor.
    pub fn add_spacer(&mut self, stretch: u32) {
        self.items.push(BoxLayoutItem {
            widget_id: None,
            stretch: stretch.max(1),
            constraints: LayoutConstraints::new(0, None),
            policy: SizePolicy::Expanding,
        });
    }

    /// Sets size constraints for an existing widget item.
    pub fn set_constraints(&mut self, widget_id: ObjectId, constraints: LayoutConstraints) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.widget_id == Some(widget_id))
        {
            item.constraints = constraints;
        }
    }

    /// Sets size policy for an existing widget item.
    pub fn set_size_policy(&mut self, widget_id: ObjectId, policy: SizePolicy) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.widget_id == Some(widget_id))
        {
            item.policy = policy;
        }
    }

    fn allocate_major_lengths(&self, primary: u32) -> Vec<u32> {
        if self.items.is_empty() {
            return Vec::new();
        }

        let total_stretch: u32 = self.items.iter().map(|item| item.stretch).sum::<u32>().max(1);
        let mut assigned = Vec::with_capacity(self.items.len());

        for item in &self.items {
            let mut major = if item.policy == SizePolicy::Fixed {
                item.constraints.max.unwrap_or(item.constraints.min)
            } else {
                primary.saturating_mul(item.stretch) / total_stretch
            };

            major = major.max(item.constraints.min);
            if let Some(max) = item.constraints.max {
                major = major.min(max.max(item.constraints.min));
            }
            assigned.push(major);
        }

        let mut total_assigned: u32 = assigned.iter().sum();

        while total_assigned < primary {
            let mut grew = false;
            for (index, item) in self.items.iter().enumerate() {
                if total_assigned >= primary {
                    break;
                }
                let max_allowed = item.constraints.max.unwrap_or(u32::MAX).max(item.constraints.min);
                if assigned[index] < max_allowed {
                    assigned[index] = assigned[index].saturating_add(1);
                    total_assigned = total_assigned.saturating_add(1);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }

        while total_assigned > primary {
            let mut shrank = false;
            for (index, item) in self.items.iter().enumerate().rev() {
                if total_assigned <= primary {
                    break;
                }
                let min_allowed = item.constraints.min;
                if assigned[index] > min_allowed {
                    assigned[index] = assigned[index].saturating_sub(1);
                    total_assigned = total_assigned.saturating_sub(1);
                    shrank = true;
                }
            }
            if !shrank {
                break;
            }
        }

        assigned
    }
}

impl Layout for BoxLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.items.push(BoxLayoutItem {
            widget_id: Some(widget_id),
            stretch: stretch.max(1),
            constraints: LayoutConstraints::new(0, None),
            policy: SizePolicy::Expanding,
        });
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.items.retain(|item| item.widget_id != Some(widget_id));
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
        let majors = self.allocate_major_lengths(primary);
        let mut cursor_x = rect.x + self.margin as i32;
        let mut cursor_y = rect.y + self.margin as i32;

        for (index, item) in self.items.iter().enumerate() {
            let major = majors.get(index).copied().unwrap_or(0);

            let child_rect = match self.orientation {
                Orientation::Horizontal => {
                    Rect::new(cursor_x, cursor_y, major, rect.height.saturating_sub(self.margin * 2))
                }
                Orientation::Vertical => {
                    Rect::new(cursor_x, cursor_y, rect.width.saturating_sub(self.margin * 2), major)
                }
            };
            if let Some(widget_id) = item.widget_id {
                widgets(widget_id, child_rect);
            }
            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + self.spacing) as i32,
                Orientation::Vertical => cursor_y += (major + self.spacing) as i32,
            }
        }
    }
}

/// Horizontal box layout with explicit naming parity.
pub struct HBoxLayout {
    inner: BoxLayout,
}

impl HBoxLayout {
    /// Creates a horizontal box layout.
    pub fn new(spacing: u32, margin: u32) -> Self {
        Self {
            inner: BoxLayout::new(Orientation::Horizontal, spacing, margin),
        }
    }

    pub fn add_spacer(&mut self, stretch: u32) {
        self.inner.add_spacer(stretch);
    }

    pub fn set_constraints(&mut self, widget_id: ObjectId, constraints: LayoutConstraints) {
        self.inner.set_constraints(widget_id, constraints);
    }

    pub fn set_size_policy(&mut self, widget_id: ObjectId, policy: SizePolicy) {
        self.inner.set_size_policy(widget_id, policy);
    }

    pub fn spacing(&self) -> u32 {
        self.inner.spacing()
    }

    pub fn set_spacing(&mut self, spacing: u32) {
        self.inner.set_spacing(spacing);
    }

    pub fn margin(&self) -> u32 {
        self.inner.margin()
    }

    pub fn set_margin(&mut self, margin: u32) {
        self.inner.set_margin(margin);
    }

    pub fn item_count(&self) -> usize {
        self.inner.item_count()
    }
}

impl Layout for HBoxLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.inner.add_widget(widget_id, stretch);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.inner.remove_widget(widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        self.inner.update(rect, widgets);
    }
}

/// Vertical box layout with explicit naming parity.
pub struct VBoxLayout {
    inner: BoxLayout,
}

impl VBoxLayout {
    /// Creates a vertical box layout.
    pub fn new(spacing: u32, margin: u32) -> Self {
        Self {
            inner: BoxLayout::new(Orientation::Vertical, spacing, margin),
        }
    }

    pub fn add_spacer(&mut self, stretch: u32) {
        self.inner.add_spacer(stretch);
    }

    pub fn set_constraints(&mut self, widget_id: ObjectId, constraints: LayoutConstraints) {
        self.inner.set_constraints(widget_id, constraints);
    }

    pub fn set_size_policy(&mut self, widget_id: ObjectId, policy: SizePolicy) {
        self.inner.set_size_policy(widget_id, policy);
    }

    pub fn spacing(&self) -> u32 {
        self.inner.spacing()
    }

    pub fn set_spacing(&mut self, spacing: u32) {
        self.inner.set_spacing(spacing);
    }

    pub fn margin(&self) -> u32 {
        self.inner.margin()
    }

    pub fn set_margin(&mut self, margin: u32) {
        self.inner.set_margin(margin);
    }

    pub fn item_count(&self) -> usize {
        self.inner.item_count()
    }
}

impl Layout for VBoxLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.inner.add_widget(widget_id, stretch);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.inner.remove_widget(widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        self.inner.update(rect, widgets);
    }
}

/// Fixed-grid layout manager with row/column cell placement.
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
                    widgets(widget_id, Rect::new(x, y, cell_width, cell_height));
                }
            }
        }
    }
}

/// Two-column form layout storing `(label, field)` row pairs.
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
                Rect::new(rect.x + self.margin as i32, y, label_width, row_height),
            );
            widgets(
                *field,
                Rect::new(
                    rect.x + self.margin as i32 + label_width as i32 + self.spacing as i32,
                    y,
                    field_width,
                    row_height,
                ),
            );
        }
    }
}

/// Stack layout that shows one child page at a time.
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

/// Splitter-like layout distributing space by pane ratios.
pub struct SplitterLayout {
    orientation: Orientation,
    spacing: u32,
    panes: Vec<ObjectId>,
    ratios: Vec<f32>,
}

impl SplitterLayout {
    /// Creates a splitter layout with orientation and pane spacing.
    pub fn new(orientation: Orientation, spacing: u32) -> Self {
        Self {
            orientation,
            spacing,
            panes: Vec::new(),
            ratios: Vec::new(),
        }
    }

    /// Sets relative size ratio for a pane index.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) {
        if index < self.ratios.len() {
            self.ratios[index] = ratio.max(0.01);
        }
    }
}

impl Layout for SplitterLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.panes.push(widget_id);
        self.ratios.push((stretch.max(1) as f32).max(0.01));
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        if let Some(index) = self.panes.iter().position(|id| *id == widget_id) {
            self.panes.remove(index);
            self.ratios.remove(index);
        }
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.panes.is_empty() {
            return;
        }

        let total_ratio = self.ratios.iter().copied().sum::<f32>().max(0.01);
        let gaps = (self.panes.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width.saturating_sub(gaps * self.spacing),
            Orientation::Vertical => rect.height.saturating_sub(gaps * self.spacing),
        };

        let mut cursor_x = rect.x;
        let mut cursor_y = rect.y;
        for (index, pane) in self.panes.iter().enumerate() {
            let ratio = self.ratios.get(index).copied().unwrap_or(1.0) / total_ratio;
            let major = ((primary as f32) * ratio).max(1.0) as u32;
            let pane_rect = match self.orientation {
                Orientation::Horizontal => Rect::new(cursor_x, rect.y, major, rect.height),
                Orientation::Vertical => Rect::new(rect.x, cursor_y, rect.width, major),
            };
            widgets(*pane, pane_rect);
            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + self.spacing) as i32,
                Orientation::Vertical => cursor_y += (major + self.spacing) as i32,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_layout_applies_constraints() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_constraints(1, LayoutConstraints::new(80, Some(80)));
        layout.set_size_policy(1, SizePolicy::Fixed);

        let mut rects = std::collections::HashMap::new();
        layout.update(
            Rect::new(0, 0, 200, 40),
            &mut |id, rect| {
                rects.insert(id, rect);
            },
        );

        assert_eq!(rects.get(&1).map(|rect| rect.width), Some(80));
    }

    #[test]
    fn splitter_layout_distributes_space() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 0);
        splitter.add_widget(1, 1);
        splitter.add_widget(2, 3);

        let mut rects = std::collections::HashMap::new();
        splitter.update(
            Rect::new(0, 0, 400, 40),
            &mut |id, rect| {
                rects.insert(id, rect);
            },
        );

        let left = rects.get(&1).map(|rect| rect.width).unwrap_or(0);
        let right = rects.get(&2).map(|rect| rect.width).unwrap_or(0);
        assert!(right > left);
    }

    #[test]
    fn layout_update_from_position_size_routes_through_rect_conversion() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(42, 1);

        let mut out = None;
        layout.update_from_position_size(
            Point::new(9, 11),
            Size::new(30, 12),
            &mut |id, rect| {
                if id == 42 {
                    out = Some(rect);
                }
            },
        );

        assert_eq!(out, Some(Rect::new(9, 11, 30, 12)));
    }

    #[test]
    fn hbox_and_vbox_named_types_delegate_to_box_layout_contract() {
        let mut hbox = HBoxLayout::new(3, 2);
        hbox.add_widget(1, 1);
        hbox.add_spacer(1);
        hbox.add_widget(2, 2);
        assert_eq!(hbox.spacing(), 3);
        assert_eq!(hbox.margin(), 2);
        assert_eq!(hbox.item_count(), 3);

        let mut rects = std::collections::HashMap::new();
        hbox.update(Rect::new(0, 0, 120, 20), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.len(), 2);

        let mut vbox = VBoxLayout::new(1, 0);
        vbox.add_widget(10, 1);
        vbox.add_widget(11, 1);
        let mut out = std::collections::HashMap::new();
        vbox.update(Rect::new(0, 0, 20, 40), &mut |id, rect| {
            out.insert(id, rect);
        });
        assert_eq!(out.len(), 2);
        assert!(out.get(&11).map(|r| r.y).unwrap_or_default() > out.get(&10).map(|r| r.y).unwrap_or_default());
    }

    #[test]
    fn box_layout_distribution_consumes_available_major_axis() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.add_widget(3, 1);

        let mut widths = std::collections::HashMap::new();
        layout.update(Rect::new(0, 0, 100, 10), &mut |id, rect| {
            widths.insert(id, rect.width);
        });

        let total: u32 = widths.values().copied().sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn grid_and_stack_layouts_have_deterministic_placement() {
        let mut grid = GridLayout::new(2, 2, 0, 0);
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 1, 2);
        let mut grid_rects = std::collections::HashMap::new();
        grid.update(Rect::new(0, 0, 40, 20), &mut |id, rect| {
            grid_rects.insert(id, rect);
        });

        assert_eq!(grid_rects.get(&1), Some(&Rect::new(0, 0, 20, 10)));
        assert_eq!(grid_rects.get(&2), Some(&Rect::new(20, 10, 20, 10)));

        let mut stack = StackLayout::new();
        stack.add_widget(7, 0);
        stack.add_widget(8, 0);
        stack.set_current_index(1);

        let mut shown = None;
        stack.update(Rect::new(1, 2, 30, 40), &mut |id, rect| {
            shown = Some((id, rect));
        });

        assert_eq!(shown, Some((8, Rect::new(1, 2, 30, 40))));
    }
}
