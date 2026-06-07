//! Form layout manager — two-column label/field row pairs.
use super::Layout;
use crate::core::{ObjectId, Rect};

/// Two-column form layout storing `(label, field)` row pairs
/// plus standalone items added via the `Layout` trait.
pub struct FormLayout {
    spacing: u32,
    margin: u32,
    rows: Vec<(ObjectId, ObjectId)>,
    /// Standalone items added via the `Layout` trait (widget_id, stretch).
    items: Vec<(ObjectId, u32)>,
}

impl FormLayout {
    /// Create a two-column form layout.
    pub fn new(spacing: u32, margin: u32) -> Self {
        Self { spacing, margin, rows: Vec::new(), items: Vec::new() }
    }

    /// Add one form row as `(label, field)` pair.
    pub fn add_row_pair(&mut self, label_id: ObjectId, field_id: ObjectId) {
        self.rows.push((label_id, field_id));
    }

    /// Returns the number of rows in the form.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Convenience method that adds a standalone widget with a label string,
    /// delegating to [`Layout::add_widget`]. Returns the index of the added item.
    pub fn add_row(&mut self, _label: &str, widget_id: ObjectId) -> usize {
        self.add_widget(widget_id, 0);
        self.items.len().saturating_sub(1)
    }

    /// Returns the number of standalone items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns the spacing between form entries.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Returns the outer margin.
    pub fn margin(&self) -> u32 {
        self.margin
    }

    /// Remove a row by its index. Returns false if index is out of bounds.
    pub fn remove_row(&mut self, index: usize) -> bool {
        if index >= self.rows.len() {
            return false;
        }
        let (label, field) = self.rows.remove(index);
        self.items.retain(|(id, _)| *id != label && *id != field);
        true
    }
}

impl Layout for FormLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.items.push((widget_id, stretch));
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.rows.retain(|(label, field)| *label != widget_id && *field != widget_id);
        self.items.retain(|(id, _)| *id != widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        let mut ids = Vec::new();
        for (label, field) in &self.rows {
            ids.push(*label);
            ids.push(*field);
        }
        for (id, _) in &self.items {
            ids.push(*id);
        }
        ids
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.rows.iter().any(|(label, field)| *label == id || *field == id)
            || self.items.iter().any(|(widget_id, _)| *widget_id == id)
    }

    fn clear(&mut self) {
        self.rows.clear();
        self.items.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let total_rows = self.rows.len();
        let total_items = self.items.len();
        let total_entries = total_rows * 2 + total_items;
        if total_entries == 0 {
            return;
        }

        // Compute available height for all entries.
        let available_height = rect.height.saturating_sub(self.margin * 2);
        if available_height == 0 {
            return;
        }
        let spacing_total =
            if total_entries > 1 { (total_entries as u32 - 1) * self.spacing } else { 0 };
        let entry_height = available_height.saturating_sub(spacing_total) / total_entries as u32;

        // Layout rows: each row has a label (1/3 width) and a field (2/3 width).
        let label_width = rect.width / 3;
        let field_width = rect.width.saturating_sub(label_width + self.spacing);
        let margin = self.margin as i32;
        let spacing = self.spacing as i32;
        let entry_height_i32 = entry_height as i32;

        let mut index = 0;
        for (label, field) in &self.rows {
            let y = rect.y + margin + index * (entry_height_i32 + spacing);
            widgets(*label, Rect::new(rect.x + margin, y, label_width, entry_height));
            widgets(
                *field,
                Rect::new(
                    rect.x + margin + label_width as i32 + spacing,
                    y,
                    field_width,
                    entry_height,
                ),
            );
            index += 2;
        }

        // Layout standalone items: each gets full width.
        for (id, _stretch) in &self.items {
            let y = rect.y + margin + index * (entry_height_i32 + spacing);
            widgets(
                *id,
                Rect::new(
                    rect.x + margin,
                    y,
                    rect.width.saturating_sub(self.margin * 2),
                    entry_height,
                ),
            );
            index += 1;
        }
    }
}
