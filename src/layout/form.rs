//! Form layout manager — two-column label/field row pairs.
use crate::core::{ObjectId, Rect};
use super::Layout;
/// Two-column form layout storing `(label, field)` row pairs.
pub struct FormLayout {
    spacing: u32,
    margin: u32,
    rows: Vec<(ObjectId, ObjectId)>,
}
impl FormLayout {
    /// Create a two-column form layout.
    pub fn new(spacing: u32, margin: u32) -> Self { Self { spacing, margin, rows: Vec::new() } }
    /// Add one form row as `(label, field)` pair.
    pub fn add_row(&mut self, label_id: ObjectId, field_id: ObjectId) { self.rows.push((label_id, field_id)); }
}
impl Layout for FormLayout {
    fn add_widget(&mut self, _widget_id: ObjectId, _stretch: u32) {}
    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.rows.retain(|(label, field)| *label != widget_id && *field != widget_id);
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.rows.is_empty() { return; }
        let row_height = rect.height
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.rows.len() as u32 - 1) * self.spacing) / self.rows.len() as u32;
        let label_width = rect.width / 3;
        let field_width = rect.width.saturating_sub(self.margin * 2 + label_width + self.spacing);
        for (index, (label, field)) in self.rows.iter().enumerate() {
            let y = rect.y + self.margin as i32 + index as i32 * (row_height + self.spacing) as i32;
            widgets(*label, Rect::new(rect.x + self.margin as i32, y, label_width, row_height));
            widgets(*field, Rect::new(
                rect.x + self.margin as i32 + label_width as i32 + self.spacing as i32,
                y, field_width, row_height,
            ));
        }
    }
}
