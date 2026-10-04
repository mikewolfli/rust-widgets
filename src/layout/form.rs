// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Form layout manager — two-column label/field row pairs.
use super::Layout;
use crate::compat::{Any, Vec};
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

    /// Convenience method that adds a standalone widget,
    /// delegating to [`Layout::add_widget`]. Returns the index of the added item.
    pub fn add_row(&mut self, widget_id: ObjectId) -> usize {
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
    ///
    /// # What this removes, and what it must not
    ///
    /// `rows` (label/field pairs, from [`Self::add_row_pair`]) and `items` (standalone
    /// widgets, from [`Self::add_row`] / [`Layout::add_widget`]) are **two separate stores**, so
    /// only `rows` holds the pair being removed. The body used to run
    /// `items.retain(|(id, _)| *id != label && *id != field)` as well, which could therefore
    /// never remove the row it had just taken out — its only effect was to *also* delete a
    /// standalone item whose id happened to equal the departed label's or field's. Since
    /// `ObjectId`s are unique that coincidence needs a caller re-using an id, but the
    /// statement expressed a mixed-up model of the two stores, so it is gone rather than kept
    /// "just in case".
    ///
    /// A field widget that the caller also registered standalone must be removed through
    /// [`Layout::remove_widget`], which is the store that actually holds it.
    pub fn remove_row(&mut self, index: usize) -> bool {
        if index >= self.rows.len() {
            return false;
        }
        self.rows.remove(index);
        true
    }
}

impl Layout for FormLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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
        // Keep every entry addressable when the form is smaller than the
        // minimum spacing budget; the parent clip/scroll container owns the
        // visual overflow rather than receiving zero-height child rects.
        let entry_height =
            (available_height.saturating_sub(spacing_total) / total_entries as u32).max(1);

        // Layout rows: each row has a label (1/3 width) and a field (2/3 width), measured
        // against the same inner box the standalone items use — the parent minus *both*
        // margins. Allocating the full width and then adding the left margin pushed the field
        // past the inner right edge (S-57).
        let inner_width = rect.width.saturating_sub(self.margin * 2);
        let label_width = inner_width / 3;
        let field_width = inner_width.saturating_sub(label_width + self.spacing);
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

        // Layout standalone items: each gets the full inner width, sharing the same box
        // the label/field pairs use.
        for (id, _stretch) in &self.items {
            let y = rect.y + margin + index * (entry_height_i32 + spacing);
            widgets(*id, Rect::new(rect.x + margin, y, inner_width, entry_height));
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;

    #[test]
    fn form_layout_preserves_nonzero_height_when_entries_overflow() {
        let mut layout = FormLayout::new(8, 0);
        layout.add_row_pair(1, 2);
        layout.add_row_pair(3, 4);

        let mut geometries = Vec::new();
        layout.update(Rect::new(0, 0, 200, 4), &mut |id, rect| geometries.push((id, rect)));

        assert_eq!(geometries.len(), 4);
        assert!(geometries.iter().all(|(_, rect)| rect.height >= 1));
    }

    #[test]
    fn form_layout_empty_form_emits_no_children() {
        let layout = FormLayout::new(8, 4);
        let mut count = 0;
        layout.update(Rect::new(0, 0, 200, 100), &mut |_, _| count += 1);
        assert_eq!(count, 0);
    }

    /// S-57: a label/field pair shares the standalone items' inner box (parent minus both
    /// margins), so the field never overflows the right edge.
    #[test]
    fn form_layout_pair_stays_inside_the_inner_box() {
        let mut layout = FormLayout::new(10, 10);
        layout.add_row_pair(1, 2);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 300, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        let inner_right = (300 - 10) as i32;
        let label = rects.get(&1).copied().expect("label placed");
        let field = rects.get(&2).copied().expect("field placed");
        assert_eq!(label.x, 10, "the label starts at the left margin");
        assert!(label.x + label.width as i32 <= inner_right, "label escapes: {label:?}");
        assert!(field.x + field.width as i32 <= inner_right, "field escapes: {field:?}");
        assert_eq!(field.x, label.x + label.width as i32 + 10, "the gap is paid once");
    }

    /// S-57 (non-zero origin): the same inner-box arithmetic holds away from the origin.
    #[test]
    fn form_layout_pair_respects_a_nonzero_parent_origin() {
        let mut layout = FormLayout::new(0, 5);
        layout.add_row_pair(1, 2);

        let mut rects = HashMap::new();
        layout.update(Rect::new(100, 50, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        let inner_left = 100 + 5;
        let inner_right = (100 + 200 - 5) as i32;
        let field = rects.get(&2).copied().expect("field placed");
        assert_eq!(field.x + field.width as i32, inner_right, "field reaches the inner right edge");
        let label = rects.get(&1).copied().expect("label placed");
        assert_eq!(label.x, inner_left, "label starts at the origin plus the margin");
    }
}
