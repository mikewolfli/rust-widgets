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

    /// The total pixel cost of paying `value` on **both** sides of an axis, saturated into a
    /// `u32` pixel budget.
    ///
    /// D09-LAYOUT-02: the inner box is the parent minus two margins, and the multiplication has
    /// to reach `u64` before it is subtracted; a doubled `u32` margin that does not fit means
    /// the margin alone consumes the axis, so this returns [`u32::MAX`] and the caller's
    /// `saturating_sub` yields an empty inner box rather than a wrapped one.
    fn pixel_budget(value: u32) -> u32 {
        let doubled = value as u64 * 2;
        if doubled > u32::MAX as u64 {
            u32::MAX
        } else {
            doubled as u32
        }
    }

    /// The total spacing consumed between `entries` addressable slots, saturated into a `u32`.
    ///
    /// D09-LAYOUT-02: `(entries - 1) * spacing` is computed in `u64` so a large spacing cannot
    /// wrap in `u32`; the result saturates at [`u32::MAX`], which the caller clamps out of the
    /// available height — a spacing budget larger than the box leaves the entries the minimum
    /// 1-pixel height instead of a wrapped, generously-sized value.
    fn spacing_budget(entries: usize, spacing: u32) -> u32 {
        if entries <= 1 {
            return 0;
        }
        let product = (entries as u64 - 1) * spacing as u64;
        product.min(u32::MAX as u64) as u32
    }

    /// A `u32` layout metric as `i32`, saturating instead of wrapping.
    ///
    /// D09-LAYOUT-02: the layout paints in `i32` coordinates, so a margin/spacing above
    /// `i32::MAX` has to become `i32::MAX`; a plain `as i32` cast wraps it negative and would
    /// move a child *before* the parent's origin.
    fn clamped_i32(value: u32) -> i32 {
        value.min(i32::MAX as u32) as i32
    }

    /// The sum of two `u32` pixel budgets, saturated at [`u32::MAX`] (D09-LAYOUT-02).
    ///
    /// Used for the label column plus the gap the field column is measured against, which is
    /// the one remaining `u32 + u32` in the inner-box arithmetic.
    fn sum_budget(a: u32, b: u32) -> u32 {
        (a as u64 + b as u64).min(u32::MAX as u64) as u32
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
        // D09-LAYOUT-02: `total_entries` counts child slots, so the doubling can only overflow
        // `usize` on a build where `usize` is wider than 32 bits *and* a form holds more than
        // 2^63 label/field rows — unreachable before memory is exhausted. Saturating keeps the
        // arithmetic total on every target instead of relying on that argument.
        let total_entries = total_rows.saturating_mul(2).saturating_add(total_items);
        if total_entries == 0 {
            return;
        }

        // D09-LAYOUT-02: `margin` and `spacing` are public `u32` values, so neither
        // `margin * 2` nor `(n - 1) * spacing` may be evaluated in `u32`: the first panics in a
        // build with overflow checks and wraps otherwise once `margin > u32::MAX / 2`, and the
        // second does the same once the spacing budget exceeds `u32::MAX`. Both products are
        // computed in `u64` and saturated back into `u32`, which defines an explicit bounded
        // policy for huge values rather than letting the constructor's arguments escape the
        // layout arithmetic:
        //
        // * a margin that cannot be paid twice consumes the whole axis, so the inner box is
        //   zero-width/height and the entries are clipped by the parent (see the `== 0` early
        //   return below) instead of receiving a negative/wrapped box;
        // * a spacing budget that does not fit `u32` is saturated at the maximum, so it can
        //   never wrap into a small number and hand entries a larger height than they earned.
        let inner_width = rect.width.saturating_sub(Self::pixel_budget(self.margin));
        let available_height = rect.height.saturating_sub(Self::pixel_budget(self.margin));
        if available_height == 0 || inner_width == 0 {
            return;
        }
        let spacing_total = Self::spacing_budget(total_entries, self.spacing);
        // Keep every entry addressable when the form is smaller than the
        // minimum spacing budget; the parent clip/scroll container owns the
        // visual overflow rather than receiving zero-height child rects.
        let entry_height =
            (available_height.saturating_sub(spacing_total) / total_entries as u32).max(1);

        // Layout rows: each row has a label (1/3 width) and a field (2/3 width), measured
        // against the same inner box the standalone items use — the parent minus *both*
        // margins. Allocating the full width and then adding the left margin pushed the field
        // past the inner right edge (S-57).
        let label_width = inner_width / 3;
        // The gap between the label and the field is paid from the inner width, so it must be
        // clamped to what is left after the label column: a spacing larger than that would
        // otherwise push the field out of the parent's right edge (D09-LAYOUT-02, same policy as
        // the margin above). Clamping to the *remaining* budget, not just the inner width, is what
        // keeps `origin_x + label_width + gap` at or before the inner right edge.
        let gap = self.spacing.min(inner_width.saturating_sub(label_width));
        // `label_width + gap` is `u32 + u32` too, so it takes the same widened treatment as the
        // other products here (D09-LAYOUT-02).
        let field_width = inner_width.saturating_sub(Self::sum_budget(label_width, gap));
        let margin = Self::clamped_i32(self.margin);
        // `spacing` positions the field and advances the rows. Clamped to the inner box so a huge
        // value cannot wrap the painted `x`/`y` (D09-LAYOUT-02).
        let spacing = Self::clamped_i32(gap);
        let entry_height_i32 = entry_height as i32;
        // The per-entry stride is `entry_height + spacing`; both are already `i32`-bounded, but
        // their sum can still overflow, so saturate it once and reuse it for every row. The
        // cumulative `y` likewise saturates rather than wrapping (D09-LAYOUT-02).
        let stride = entry_height_i32.saturating_add(spacing);
        let origin_x = rect.x.saturating_add(margin);

        let mut index: i32 = 0;
        for (label, field) in &self.rows {
            let y = rect.y.saturating_add(margin).saturating_add(index.saturating_mul(stride));
            widgets(*label, Rect::new(origin_x, y, label_width, entry_height));
            widgets(
                *field,
                Rect::new(
                    origin_x.saturating_add(label_width as i32).saturating_add(spacing),
                    y,
                    field_width,
                    entry_height,
                ),
            );
            index = index.saturating_add(2);
        }

        // Layout standalone items: each gets the full inner width, sharing the same box
        // the label/field pairs use.
        for (id, _stretch) in &self.items {
            let y = rect.y.saturating_add(margin).saturating_add(index.saturating_mul(stride));
            widgets(*id, Rect::new(origin_x, y, inner_width, entry_height));
            index = index.saturating_add(1);
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

        let inner_right = 300 - 10;
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
        let inner_right = 100 + 200 - 5;
        let field = rects.get(&2).copied().expect("field placed");
        assert_eq!(field.x + field.width as i32, inner_right, "field reaches the inner right edge");
        let label = rects.get(&1).copied().expect("label placed");
        assert_eq!(label.x, inner_left, "label starts at the origin plus the margin");
    }

    // ── D09-LAYOUT-02: public `u32` margin/spacing multiplication must not overflow ──

    /// The `margin * 2` inner-box computation previously overflowed for `margin > u32::MAX / 2`
    /// (panic under overflow checks, wrap otherwise). It must now stay silent with no children
    /// emitted, because the margin consumes the whole axis and the inner box is empty under the
    /// bounded policy.
    #[test]
    fn form_layout_max_margin_does_not_panic_and_emits_nothing() {
        let mut layout = FormLayout::new(0, u32::MAX);
        layout.add_row_pair(1, 2);

        let mut count = 0;
        layout.update(Rect::new(0, 0, 200, 100), &mut |_, _| count += 1);
        assert_eq!(count, 0, "a margin that cannot be paid twice leaves no addressable inner box");
    }

    /// The boundary just past half of `u32::MAX` is where `margin * 2` stops fitting `u32`; the
    /// layout must still not panic and must not place children in a wrapped box.
    #[test]
    fn form_layout_boundary_margin_doubling_does_not_wrap() {
        let mut layout = FormLayout::new(0, u32::MAX / 2 + 1);
        layout.add_row_pair(1, 2);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });
        // Doubled margin exceeds the parent's height/width, so the inner box is empty and the
        // entries are clipped by the parent rather than handed a large wrapped rect.
        assert!(rects.is_empty(), "no child may be placed in a wrapped inner box: {rects:?}");
    }

    /// A multi-row form with a huge spacing exercises `(total_entries - 1) * spacing`, which used
    /// to overflow before the `saturating_sub`. Every placed entry must stay inside the parent
    /// and keep at least one pixel of height (D09-LAYOUT-02).
    #[test]
    fn form_layout_huge_spacing_keeps_children_bounded() {
        let mut layout = FormLayout::new(u32::MAX, 4);
        layout.add_row_pair(1, 2);
        layout.add_row_pair(3, 4);
        layout.add_widget(5, 0);

        let parent = Rect::new(10, 20, 300, 200);
        let mut rects = HashMap::new();
        layout.update(parent, &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.len(), 5, "every entry is still addressable");
        for (id, rect) in &rects {
            assert!(rect.height >= 1, "{id} collapsed: {rect:?}");
            assert!(rect.x >= parent.x, "{id} starts before the parent: {rect:?}");
            assert!(rect.y >= parent.y, "{id} starts above the parent: {rect:?}");
            assert!(
                rect.x + rect.width as i32 <= parent.x + parent.width as i32,
                "{id} escapes the parent's right edge: {rect:?}"
            );
        }
    }

    /// A single row has no `(n - 1) * spacing` term; the huge spacing must still not overflow the
    /// arithmetic or the placed rect.
    #[test]
    fn form_layout_single_row_with_huge_spacing_is_bounded() {
        let mut layout = FormLayout::new(u32::MAX, 0);
        layout.add_row_pair(1, 2);

        let parent = Rect::new(0, 0, 400, 300);
        let mut rects = HashMap::new();
        layout.update(parent, &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.len(), 2);
        for (id, rect) in &rects {
            assert!(rect.height >= 1, "{id} collapsed: {rect:?}");
            assert!(rect.x >= parent.x && rect.y >= parent.y, "{id} misplaced: {rect:?}");
            assert!(
                rect.x + rect.width as i32 <= parent.x + parent.width as i32,
                "{id} escapes the parent's right edge: {rect:?}"
            );
        }
    }
}
