// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Form layout manager — two-column label/field row pairs.
use super::{ChildInfo, Layout};
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect};
use crate::style::EdgeOffsets;

/// How a form decides the width of its field column.
///
/// The equivalent of the reference toolkit's `form layout::FieldGrowthPolicy`. The default keeps this layout's
/// historical behaviour (the field takes whatever the label column left), so introducing the
/// enum changes no existing document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldGrowthPolicy {
    /// Every field that is not fixed takes the rest of the row width. The default.
    #[default]
    AllNonFixed,
    /// A field stays at the width it asks for; the rest of the row is left unused.
    FieldsStayAtSize,
    /// Fields grow, but only within the row they were given. Bounded by the same remainder
    /// as [`Self::AllNonFixed`] in this implementation: the crate has no per-field
    /// `SizePolicy` on form rows (unlike `BoxLayout`), so the two differ only once one
    /// exists. Documented rather than silently treated as an alias.
    ExpandingFieldsGrow,
}

/// When a form puts its field on a row of its own.
///
/// The equivalent of the reference toolkit's `form layout::RowWrapPolicy`. The default is `DontWrapRows`, which
/// is what this layout has always done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowWrapPolicy {
    /// Label and field always share a row, however narrow it is. The default.
    #[default]
    DontWrapRows,
    /// A row wraps when the label and the field's preferred widths do not fit together.
    WrapLongRows,
    /// Every field goes on its own row, below its label.
    WrapAllRows,
}

/// Two-column form layout storing `(label, field)` row pairs
/// plus standalone items added via the `Layout` trait.
/// How a form's label column places a label narrower than the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormLabelAlignment {
    /// Left-aligned in the label column (the historical behaviour).
    #[default]
    Left,
    /// Centred in the label column.
    Center,
    /// Right-aligned against the field gap.
    Right,
}

/// The role of an entry in a form row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormItemRole {
    /// The label half of a `(label, field)` row.
    Label,
    /// The field half of a `(label, field)` row.
    Field,
    /// A standalone item added through [`Layout::add_widget`].
    Standalone,
}

/// A two-column form layout with a measured label column and a flexible field column.
pub struct FormLayout {
    spacing: u32,
    margin: u32,
    /// Four-way margins. `None` means the uniform [`Self::margin`] is used on every side.
    margins: Option<EdgeOffsets>,
    growth_policy: FieldGrowthPolicy,
    row_wrap_policy: RowWrapPolicy,
    label_alignment: FormLabelAlignment,
    rows: Vec<(ObjectId, ObjectId)>,
    /// Standalone items added via the `Layout` trait (widget_id, stretch).
    items: Vec<(ObjectId, u32)>,
}

impl FormLayout {
    /// Create a two-column form layout.
    pub fn new(spacing: u32, margin: u32) -> Self {
        Self {
            spacing,
            margin,
            margins: None,
            growth_policy: FieldGrowthPolicy::default(),
            row_wrap_policy: RowWrapPolicy::default(),
            label_alignment: FormLabelAlignment::default(),
            rows: Vec::new(),
            items: Vec::new(),
        }
    }

    /// Sets how much of the row's width a field takes.
    pub fn set_field_growth_policy(&mut self, policy: FieldGrowthPolicy) {
        self.growth_policy = policy;
    }

    /// The field growth policy.
    pub fn field_growth_policy(&self) -> FieldGrowthPolicy {
        self.growth_policy
    }

    /// Sets when a field moves onto its own row.
    pub fn set_row_wrap_policy(&mut self, policy: RowWrapPolicy) {
        self.row_wrap_policy = policy;
    }

    /// The row wrap policy.
    pub fn row_wrap_policy(&self) -> RowWrapPolicy {
        self.row_wrap_policy
    }

    /// How a label narrower than its column is positioned.
    pub fn label_alignment(&self) -> FormLabelAlignment {
        self.label_alignment
    }

    /// Sets how a label narrower than its column is positioned.
    pub fn set_label_alignment(&mut self, alignment: FormLabelAlignment) {
        self.label_alignment = alignment;
    }

    /// The field paired with `label_id`, if the id names a label.
    ///
    /// This is the layout-side equivalent of a label's buddy: a caller can walk from the
    /// caption it built to the control it describes.
    pub fn field_for_label(&self, label_id: ObjectId) -> Option<ObjectId> {
        self.rows.iter().find(|(label, _)| *label == label_id).map(|(_, field)| *field)
    }

    /// The label paired with `field_id`, if the id names a field.
    pub fn label_for_field(&self, field_id: ObjectId) -> Option<ObjectId> {
        self.rows.iter().find(|(_, field)| *field == field_id).map(|(label, _)| *label)
    }

    /// The widget at `row` in `role`.
    ///
    /// Returns `None` for a row that does not exist or a role the row does not have. A
    /// standalone role indexes the standalone item list, not the pair list.
    pub fn item_at(&self, row: usize, role: FormItemRole) -> Option<ObjectId> {
        match role {
            FormItemRole::Label => self.rows.get(row).map(|(label, _)| *label),
            FormItemRole::Field => self.rows.get(row).map(|(_, field)| *field),
            FormItemRole::Standalone => self.items.get(row).map(|(id, _)| *id),
        }
    }

    /// Sets four-way margins, overriding the uniform [`Self::margin`].
    pub fn set_margins(&mut self, margins: EdgeOffsets) {
        self.margins = Some(margins);
    }

    /// The four-way margins, or `None` when the uniform [`Self::margin`] applies.
    pub fn margins(&self) -> Option<EdgeOffsets> {
        self.margins
    }

    /// Left/top/right/bottom insets for `rect`, each clamped to half its axis.
    fn insets(&self, rect: Rect) -> (u32, u32, u32, u32) {
        let limit_x = rect.width / 2;
        let limit_y = rect.height / 2;
        match self.margins {
            Some(margins) => (
                margins.left.min(limit_x),
                margins.top.min(limit_y),
                margins.right.min(limit_x),
                margins.bottom.min(limit_y),
            ),
            None => {
                let uniform = self.margin.min(limit_x.min(limit_y));
                (uniform, uniform, uniform, uniform)
            }
        }
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

    /// The hint-aware path: applies [`FieldGrowthPolicy`] and [`RowWrapPolicy`].
    ///
    /// `update` keeps the historical two-column behaviour; this method is where the policies
    /// take effect, because both of them ask "how wide does this child want to be?" — the
    /// question `ChildInfo` exists to answer (LAY-N01).
    ///
    /// Height is still shared evenly across entries, which is the model `update` uses: a form
    /// gives every row the same height, and a wrapping row costs two entries.
    fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect)) {
        if self.rows.is_empty() && self.items.is_empty() {
            return;
        }
        let (left, top, right, bottom) = self.insets(rect);
        let inner_width = rect.width.saturating_sub(left).saturating_sub(right);
        let available_height = rect.height.saturating_sub(top).saturating_sub(bottom);
        if inner_width == 0 || available_height == 0 {
            return;
        }

        // Whether each pair occupies two rows. `WrapLongRows` asks the two children how wide
        // they want to be; a missing hint counts as zero, which keeps the row unwrapped (the
        // lenient answer, matching the rest of this parser).
        let wraps: Vec<bool> = self
            .rows
            .iter()
            .map(|(label, field)| match self.row_wrap_policy {
                RowWrapPolicy::DontWrapRows => false,
                RowWrapPolicy::WrapAllRows => true,
                RowWrapPolicy::WrapLongRows => {
                    let wanted = |id: ObjectId| {
                        ChildInfo::find(children, id).map(|c| c.hints.width.pref).unwrap_or(0)
                    };
                    wanted(*label).saturating_add(self.spacing).saturating_add(wanted(*field))
                        > inner_width
                }
            })
            .collect();

        let entries = self
            .rows
            .iter()
            .zip(&wraps)
            .map(|(_, wrapped)| if *wrapped { 2 } else { 1 })
            .sum::<usize>()
            + self.items.len();
        if entries == 0 {
            return;
        }
        let spacing_total = Self::spacing_budget(entries, self.spacing);
        let entry_height = (available_height.saturating_sub(spacing_total) / entries as u32).max(1);

        // Measure the label column instead of assuming one third of the form. A form of
        // short captions wasted most of that third; a form of long ones clipped them. The
        // preferred extents from the hint channel are the only measurement available here,
        // and a form whose hints are all zero keeps the historical one-third split.
        let measured_label = self
            .rows
            .iter()
            .filter_map(|(label, _)| ChildInfo::find(children, *label))
            .map(|info| info.hints.width.pref)
            .max()
            .unwrap_or(0);
        let label_width = if measured_label > 0 {
            measured_label.min(inner_width.saturating_sub(1))
        } else {
            inner_width / 3
        };
        let gap = self.spacing.min(inner_width.saturating_sub(label_width));
        let remaining_field_width = inner_width.saturating_sub(Self::sum_budget(label_width, gap));
        let origin_x = rect.x.saturating_add(left as i32);
        let spacing = Self::clamped_i32(gap);
        let entry_height_i32 = entry_height as i32;
        let mut cursor_y = rect.y.saturating_add(top as i32);

        for ((label, field), wrapped) in self.rows.iter().zip(&wraps) {
            let row_y = cursor_y;
            if *wrapped {
                out(*label, Rect::new(origin_x, row_y, inner_width, entry_height));
                let field_y = row_y.saturating_add(entry_height_i32).saturating_add(spacing);
                out(*field, Rect::new(origin_x, field_y, inner_width, entry_height));
                cursor_y = field_y.saturating_add(entry_height_i32).saturating_add(spacing);
            } else {
                let wanted = ChildInfo::find(children, *label)
                    .map(|info| info.hints.width.pref)
                    .unwrap_or(0);
                let label_rect = if wanted > 0 && wanted < label_width {
                    let offset = match self.label_alignment {
                        FormLabelAlignment::Left => 0,
                        FormLabelAlignment::Center => (label_width - wanted) / 2,
                        FormLabelAlignment::Right => label_width - wanted,
                    };
                    Rect::new(origin_x.saturating_add(offset as i32), row_y, wanted, entry_height)
                } else {
                    Rect::new(origin_x, row_y, label_width, entry_height)
                };
                out(*label, label_rect);
                let field_width = match self.growth_policy {
                    FieldGrowthPolicy::AllNonFixed | FieldGrowthPolicy::ExpandingFieldsGrow => {
                        remaining_field_width
                    }
                    FieldGrowthPolicy::FieldsStayAtSize => ChildInfo::find(children, *field)
                        .map(|c| c.hints.width.pref.min(remaining_field_width))
                        .unwrap_or(remaining_field_width),
                };
                out(
                    *field,
                    Rect::new(
                        origin_x.saturating_add(label_width as i32).saturating_add(spacing),
                        row_y,
                        field_width,
                        entry_height,
                    ),
                );
                cursor_y = row_y.saturating_add(entry_height_i32).saturating_add(spacing);
            }
        }

        for (id, _stretch) in &self.items {
            out(*id, Rect::new(origin_x, cursor_y, inner_width, entry_height));
            cursor_y = cursor_y.saturating_add(entry_height_i32).saturating_add(spacing);
        }
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
        let (left, top, right, bottom) = self.insets(rect);
        let inner_width = rect.width.saturating_sub(left).saturating_sub(right);
        let available_height = rect.height.saturating_sub(top).saturating_sub(bottom);
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
        let margin = Self::clamped_i32(left);
        let margin_top = Self::clamped_i32(top);
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
            let y = rect.y.saturating_add(margin_top).saturating_add(index.saturating_mul(stride));
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
            let y = rect.y.saturating_add(margin_top).saturating_add(index.saturating_mul(stride));
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

    /// `WrapLongRows` puts the field on its own row when the pair does not fit.
    #[test]
    fn wrap_long_rows_moves_the_field_below_the_label() {
        let mut layout = FormLayout::new(8, 0);
        layout.add_row_pair(1, 2);
        layout.set_row_wrap_policy(RowWrapPolicy::WrapLongRows);

        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(150, 20)),
            ChildInfo::new(2, crate::layout::Hints::fixed(150, 20)),
        ];
        let mut out = Vec::new();
        layout.arrange(Rect::new(0, 0, 200, 100), &children, &mut |id, rect| out.push((id, rect)));

        let label = out.iter().find(|(id, _)| *id == 1).unwrap().1;
        let field = out.iter().find(|(id, _)| *id == 2).unwrap().1;
        assert_eq!(label.width, 200, "a wrapped label takes the whole row");
        assert_eq!(field.width, 200, "and so does the field below it");
        assert!(field.y > label.y, "the field is on the next row");
    }

    /// `FieldsStayAtSize` gives the field its preferred width instead of the remainder.
    #[test]
    fn fields_stay_at_size_uses_the_requested_width() {
        let mut layout = FormLayout::new(8, 0);
        layout.add_row_pair(1, 2);
        layout.set_field_growth_policy(FieldGrowthPolicy::FieldsStayAtSize);

        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(120, 20)),
            ChildInfo::new(2, crate::layout::Hints::fixed(60, 20)),
        ];
        let mut out = Vec::new();
        layout.arrange(Rect::new(0, 0, 200, 100), &children, &mut |id, rect| out.push((id, rect)));

        let field = out.iter().find(|(id, _)| *id == 2).unwrap().1;
        assert_eq!(field.width, 60, "the field keeps its own width");
        // The default policy would have given it the rest of the row.
        assert!(field.width < 200 - 200 / 3);
    }

    /// The label column is measured from the label's preferred width, and the label's
    /// rect honours the alignment inside that column.
    #[test]
    fn the_label_column_uses_the_measured_width() {
        let mut layout = FormLayout::new(0, 0);
        layout.add_row_pair(1, 2);
        layout.add_row_pair(3, 4);
        layout.set_label_alignment(FormLabelAlignment::Right);
        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(50, 20)),
            ChildInfo::new(2, crate::layout::Hints::fixed(40, 20)),
            ChildInfo::new(3, crate::layout::Hints::fixed(80, 20)),
            ChildInfo::new(4, crate::layout::Hints::fixed(40, 20)),
        ];
        let mut out = Vec::new();
        layout.arrange(Rect::new(0, 0, 200, 60), &children, &mut |id, rect| out.push((id, rect)));
        let label = out.iter().find(|(id, _)| *id == 1).unwrap().1;
        assert_eq!(label.width, 50, "the label rect is its measured width");
        assert_eq!(label.x, 30, "right alignment places the slack before the label");
        assert_eq!(layout.field_for_label(1), Some(2));
        assert_eq!(layout.label_for_field(2), Some(1));
        assert_eq!(layout.item_at(0, FormItemRole::Field), Some(2));
    }
}
