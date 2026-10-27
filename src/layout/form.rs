// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Form layout manager — two-column label/field row pairs.
use super::{ChildInfo, Layout, LayoutContext};
use super::types::ChildPlacements;
use crate::compat::{Any, HashMap, RefCell, Vec};
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

/// How a form divides the available height between its rows.
///
/// The default is [`Self::EqualSplit`], which is the behaviour this layout had before the
/// choice existed: every entry receives the same height. [`Self::FromHints`] instead gives
/// each row the largest preferred height its members state, so a multi-line field can be
/// taller than the row next to it. When the hints do not fit the available height, the form
/// falls back to the even split — the alternative would starve the later rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowHeightPolicy {
    /// Every row receives the same share of the available height. The default.
    #[default]
    EqualSplit,
    /// Each row takes the largest `hints.height.pref` of its members (or its own entry for a
    /// standalone item), with the leftover shared evenly. A missing hint falls back to the
    /// even split's value.
    FromHints,
}

/// Two-column form layout storing `(label, field)` row pairs
/// plus standalone items added via the `Layout` trait.
/// How a form's label column places a label narrower than the column.
///
/// This is [`crate::core::HorizontalAlignment`], not a second spelling of the same three
/// variants (principle #54): the offset computation in `arrange` is the one horizontal
/// alignment implementation, and the public name is kept as an alias so existing
/// `form::FormLabelAlignment` paths keep working.
pub use crate::core::HorizontalAlignment as FormLabelAlignment;

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
    row_height_policy: RowHeightPolicy,
    label_alignment: FormLabelAlignment,
    rows: Vec<(ObjectId, ObjectId)>,
    /// Standalone items added via the `Layout` trait (widget_id, stretch).
    items: Vec<(ObjectId, u32)>,
    /// The last nominal layout, keyed by the rect it was computed from (LAY-005).
    ///
    /// `child_rect` answers a one-child query from this cache instead of re-running the row
    /// solve (G-158). Only [`Layout::update`] — the nominal path — populates it, so a cached
    /// answer is byte-identical to the trait default's update-and-capture; a rect mismatch
    /// falls back to that same capture. Every mutator clears it.
    last_layout: RefCell<Option<(Rect, ChildPlacements)>>,
}

impl FormLayout {
    /// Drops the cached nominal layout; every mutator calls it (LAY-005).
    fn invalidate_last_layout(&mut self) {
        self.last_layout.borrow_mut().take();
    }

    /// Create a two-column form layout.
    pub fn new(spacing: u32, margin: u32) -> Self {
        Self {
            spacing,
            margin,
            margins: None,
            growth_policy: FieldGrowthPolicy::default(),
            row_wrap_policy: RowWrapPolicy::default(),
            row_height_policy: RowHeightPolicy::default(),
            label_alignment: FormLabelAlignment::default(),
            rows: Vec::new(),
            items: Vec::new(),
            last_layout: RefCell::new(None),
        }
    }

    /// Sets how much of the row's width a field takes.
    pub fn set_field_growth_policy(&mut self, policy: FieldGrowthPolicy) {
        self.invalidate_last_layout();
        self.growth_policy = policy;
    }

    /// The field growth policy.
    pub fn field_growth_policy(&self) -> FieldGrowthPolicy {
        self.growth_policy
    }

    /// Sets when a field moves onto its own row.
    pub fn set_row_wrap_policy(&mut self, policy: RowWrapPolicy) {
        self.invalidate_last_layout();
        self.row_wrap_policy = policy;
    }

    /// The row wrap policy.
    pub fn row_wrap_policy(&self) -> RowWrapPolicy {
        self.row_wrap_policy
    }

    /// Sets how the available height is divided between rows.
    pub fn set_row_height_policy(&mut self, policy: RowHeightPolicy) {
        self.invalidate_last_layout();
        self.row_height_policy = policy;
    }

    /// The row height policy.
    pub fn row_height_policy(&self) -> RowHeightPolicy {
        self.row_height_policy
    }

    /// How a label narrower than its column is positioned.
    pub fn label_alignment(&self) -> FormLabelAlignment {
        self.label_alignment
    }

    /// Sets how a label narrower than its column is positioned.
    pub fn set_label_alignment(&mut self, alignment: FormLabelAlignment) {
        self.invalidate_last_layout();
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
        self.invalidate_last_layout();
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
        self.invalidate_last_layout();
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

    /// The height of each of `entries` slots under the configured [`RowHeightPolicy`].
    ///
    /// `desired[i]` is the row's preferred height (`0` means "no hint"); `update` passes an
    /// empty slice because it has no hint channel, and the empty slice reads as "all zero".
    /// Both entry points therefore share one height rule instead of two copies.
    fn entry_heights(&self, entries: usize, budget: u32, desired: &[u32]) -> Vec<u32> {
        if entries == 0 {
            return Vec::new();
        }
        let equal = (budget / entries as u32).max(1);
        if self.row_height_policy != RowHeightPolicy::FromHints {
            return vec![equal; entries];
        }
        let resolved: Vec<u32> = (0..entries)
            .map(|index| match desired.get(index).copied().unwrap_or(0) {
                0 => equal,
                height => height,
            })
            .collect();
        let total: u64 = resolved.iter().map(|height| *height as u64).sum();
        if total > budget as u64 {
            // The stated heights do not fit. Starving the later rows would silently hide
            // controls, so the even split is the bounded answer.
            return vec![equal; entries];
        }
        let surplus = budget - total as u32;
        let share = surplus / entries as u32;
        let mut remainder = surplus % entries as u32;
        let mut heights = resolved;
        for height in &mut heights {
            *height += share;
            if remainder > 0 {
                *height += 1;
                remainder -= 1;
            }
        }
        heights
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
        self.invalidate_last_layout();
        self.rows.remove(index);
        true
    }

    /// The nominal row solve, shared by [`Layout::update`] (which caches its emissions for
    /// `child_rect`) — the historical two-column model lives here and nowhere else.
    fn layout_entries(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
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
        //
        // `update` has no hint channel, so the desired list is empty and a `FromHints`
        // form falls back to the same even split it has always produced (LAY-02).
        let heights =
            self.entry_heights(total_entries, available_height.saturating_sub(spacing_total), &[]);

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
        let origin_x = rect.x.saturating_add(margin);

        // `update` gives a label and its field one row each in the entry budget, but paints
        // them on the same line (the historical shape). The cursor therefore advances by both
        // heights plus the two gaps that budget already reserved.
        let mut cursor_y = rect.y.saturating_add(margin_top);
        let mut index: usize = 0;
        for (label, field) in &self.rows {
            let label_height = heights.get(index).copied().unwrap_or(1);
            let field_height = heights.get(index + 1).copied().unwrap_or(1);
            widgets(*label, Rect::new(origin_x, cursor_y, label_width, label_height));
            widgets(
                *field,
                Rect::new(
                    origin_x.saturating_add(label_width as i32).saturating_add(spacing),
                    cursor_y,
                    field_width,
                    field_height,
                ),
            );
            cursor_y = cursor_y
                .saturating_add(label_height as i32)
                .saturating_add(field_height as i32)
                .saturating_add(spacing.saturating_mul(2));
            index = index.saturating_add(2);
        }

        // Layout standalone items: each gets the full inner width, sharing the same box
        // the label/field pairs use.
        for (id, _stretch) in &self.items {
            let entry_height = heights.get(index).copied().unwrap_or(1);
            widgets(*id, Rect::new(origin_x, cursor_y, inner_width, entry_height));
            cursor_y = cursor_y.saturating_add(entry_height as i32).saturating_add(spacing);
            index = index.saturating_add(1);
        }
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
        self.invalidate_last_layout();
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

        // One pass over the hint channel, so the wrap decision, the label measurement and
        // the per-row height below do not each re-scan the child slice. A later duplicate is
        // ignored: `ChildInfo::find` reports the first entry, and so does this index (G-160).
        let mut hint_index: HashMap<ObjectId, &ChildInfo> = HashMap::new();
        for info in children {
            hint_index.entry(info.id).or_insert(info);
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
                    let wanted =
                        |id: ObjectId| hint_index.get(&id).map(|c| c.hints.width.pref).unwrap_or(0);
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
        let height_budget = available_height.saturating_sub(spacing_total);

        // The preferred height of each slot, in emission order. A non-wrapped pair shares one
        // slot, so it asks for the taller of its two members; a wrapped pair costs one slot
        // per member; a standalone item owns its slot.
        let desired: Vec<u32> = {
            let mut desired = Vec::with_capacity(entries);
            for ((label, field), wrapped) in self.rows.iter().zip(&wraps) {
                let height_of =
                    |id: ObjectId| hint_index.get(&id).map(|c| c.hints.height.pref).unwrap_or(0);
                if *wrapped {
                    desired.push(height_of(*label));
                    desired.push(height_of(*field));
                } else {
                    desired.push(height_of(*label).max(height_of(*field)));
                }
            }
            for (id, _) in &self.items {
                desired.push(hint_index.get(id).map(|c| c.hints.height.pref).unwrap_or(0));
            }
            desired
        };
        let heights = self.entry_heights(entries, height_budget, &desired);

        // Measure the label column instead of assuming one third of the form. A form of
        // short captions wasted most of that third; a form of long ones clipped them. The
        // preferred extents from the hint channel are the only measurement available here,
        // and a form whose hints are all zero keeps the historical one-third split.
        let measured_label = self
            .rows
            .iter()
            .filter_map(|(label, _)| hint_index.get(label).copied())
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
        let mut cursor_y = rect.y.saturating_add(top as i32);
        let mut slot = 0usize;

        for ((label, field), wrapped) in self.rows.iter().zip(&wraps) {
            let row_y = cursor_y;
            if *wrapped {
                let label_height = heights.get(slot).copied().unwrap_or(1);
                let field_height = heights.get(slot + 1).copied().unwrap_or(1);
                out(*label, Rect::new(origin_x, row_y, inner_width, label_height));
                let field_y = row_y.saturating_add(label_height as i32).saturating_add(spacing);
                out(*field, Rect::new(origin_x, field_y, inner_width, field_height));
                cursor_y = field_y.saturating_add(field_height as i32).saturating_add(spacing);
                slot += 2;
            } else {
                let entry_height = heights.get(slot).copied().unwrap_or(1);
                let wanted = hint_index.get(label).map(|info| info.hints.width.pref).unwrap_or(0);
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
                    FieldGrowthPolicy::FieldsStayAtSize => hint_index
                        .get(field)
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
                cursor_y = row_y.saturating_add(entry_height as i32).saturating_add(spacing);
                slot += 1;
            }
        }

        for (id, _stretch) in &self.items {
            let entry_height = heights.get(slot).copied().unwrap_or(1);
            out(*id, Rect::new(origin_x, cursor_y, inner_width, entry_height));
            cursor_y = cursor_y.saturating_add(entry_height as i32).saturating_add(spacing);
            slot += 1;
        }
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.invalidate_last_layout();
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
        self.invalidate_last_layout();
        self.rows.clear();
        self.items.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        // The nominal emissions are captured so `child_rect` can answer a one-child query
        // from the cache (LAY-005) instead of re-running the row solve.
        let mut entries: Vec<(ObjectId, Rect)> = Vec::new();
        self.layout_entries(rect, &mut |id, r| {
            entries.push((id, r));
            widgets(id, r);
        });
        *self.last_layout.borrow_mut() = Some((rect, entries));
    }

    // G-403: the trait no longer defaults these entries; each forward reproduces the previous fallback behaviour.
    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        let _ = context;
        self.update(rect, widgets);
    }

    fn arrange_with_context(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        context: &LayoutContext,
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        let _ = context;
        self.arrange(rect, children, out);
    }

    /// The one-child query, answered from the cached nominal layout when it matches
    /// (LAY-005): a hit test must not re-run the row solve. A rect mismatch (or a first
    /// query before any `update`) falls back to the trait default's update-and-capture,
    /// which is also what repopulates the cache.
    fn child_rect(&self, id: ObjectId, rect: Rect) -> Option<Rect> {
        if !self.has_child(id) {
            return None;
        }
        if let Some((cached_rect, entries)) = self.last_layout.borrow().as_ref() {
            if *cached_rect == rect {
                return entries.iter().find(|(child, _)| *child == id).map(|(_, r)| *r);
            }
        }
        let mut found = None;
        self.update(rect, &mut |child, child_rect| {
            if child == id && found.is_none() {
                found = Some(child_rect);
            }
        });
        found
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

    // ── LAY-02: row heights from hints ────────────────────────────────────────────

    #[test]
    fn the_default_row_height_policy_is_equal_split() {
        let layout = FormLayout::new(0, 0);
        assert_eq!(layout.row_height_policy(), RowHeightPolicy::EqualSplit);
    }

    #[test]
    fn from_hints_gives_each_row_its_tallest_member_and_fills_the_height() {
        let mut layout = FormLayout::new(0, 0);
        layout.add_row_pair(1, 2);
        layout.add_row_pair(3, 4);
        layout.set_row_height_policy(RowHeightPolicy::FromHints);
        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(50, 20)),
            ChildInfo::new(2, crate::layout::Hints::fixed(40, 60)),
            ChildInfo::new(3, crate::layout::Hints::fixed(50, 20)),
            ChildInfo::new(4, crate::layout::Hints::fixed(40, 20)),
        ];
        let mut out = Vec::new();
        layout.arrange(Rect::new(0, 0, 200, 100), &children, &mut |id, rect| out.push((id, rect)));
        let rect_of = |id: ObjectId| out.iter().find(|(i, _)| *i == id).unwrap().1;

        // Desired 60 + 20 = 80 in a 100 px box: the 20 px surplus is split evenly.
        assert_eq!(rect_of(1).height, 70, "row 1 takes its taller member's hint plus its share");
        assert_eq!(rect_of(2).height, 70);
        assert_eq!(rect_of(3).height, 30, "row 2 starts from its own hint");
        assert_eq!(rect_of(3).y, 70, "the second row starts after the first");
        assert_eq!(rect_of(4).y, 70);
    }

    #[test]
    fn from_hints_makes_a_tall_field_taller_than_its_row_neighbour() {
        let mut layout = FormLayout::new(0, 0);
        layout.add_row_pair(1, 2);
        layout.add_row_pair(3, 4);
        layout.set_row_height_policy(RowHeightPolicy::FromHints);
        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(50, 20)),
            ChildInfo::new(2, crate::layout::Hints::fixed(40, 80)),
            ChildInfo::new(3, crate::layout::Hints::fixed(50, 20)),
            ChildInfo::new(4, crate::layout::Hints::fixed(40, 20)),
        ];
        let mut out = Vec::new();
        layout.arrange(Rect::new(0, 0, 200, 100), &children, &mut |id, rect| out.push((id, rect)));
        let rect_of = |id: ObjectId| out.iter().find(|(i, _)| *i == id).unwrap().1;
        assert_eq!(rect_of(2).height, 80, "the multi-line field keeps its height");
        assert_eq!(rect_of(4).height, 20, "the short row is not stretched to match");
    }

    #[test]
    fn from_hints_falls_back_to_the_even_split_when_the_hints_do_not_fit() {
        let mut layout = FormLayout::new(0, 0);
        layout.add_row_pair(1, 2);
        layout.add_row_pair(3, 4);
        layout.set_row_height_policy(RowHeightPolicy::FromHints);
        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(50, 120)),
            ChildInfo::new(2, crate::layout::Hints::fixed(40, 120)),
            ChildInfo::new(3, crate::layout::Hints::fixed(50, 120)),
            ChildInfo::new(4, crate::layout::Hints::fixed(40, 120)),
        ];
        let mut out = Vec::new();
        layout.arrange(Rect::new(0, 0, 200, 100), &children, &mut |id, rect| out.push((id, rect)));
        let rect_of = |id: ObjectId| out.iter().find(|(i, _)| *i == id).unwrap().1;
        assert_eq!(rect_of(1).height, 50, "the even split is the bounded answer");
        assert_eq!(rect_of(3).height, 50);
    }

    #[test]
    fn update_is_unchanged_by_the_row_height_policy() {
        let mut equal = FormLayout::new(0, 0);
        equal.add_row_pair(1, 2);
        equal.add_row_pair(3, 4);
        let mut from_hints = FormLayout::new(0, 0);
        from_hints.add_row_pair(1, 2);
        from_hints.add_row_pair(3, 4);
        from_hints.set_row_height_policy(RowHeightPolicy::FromHints);

        let mut a = Vec::new();
        equal.update(Rect::new(0, 0, 200, 100), &mut |id, rect| a.push((id, rect)));
        let mut b = Vec::new();
        from_hints.update(Rect::new(0, 0, 200, 100), &mut |id, rect| b.push((id, rect)));
        assert_eq!(a, b, "update has no hints, so both policies agree");
    }

    /// LAY-005: the one-child query answers from the cached nominal layout — equal to the
    /// full update pass — and a mutation refreshes it, so the answer never goes stale.
    #[test]
    fn child_rect_answers_from_the_cached_layout_and_tracks_mutations() {
        let mut layout = FormLayout::new(0, 0);
        layout.add_row_pair(1, 2);
        let rect = Rect::new(0, 0, 300, 100);
        // The fallback solve answers even before any update populated the cache; one row of
        // two entries shares the height evenly, the label column is a third of the width.
        assert_eq!(layout.child_rect(1, rect), Some(Rect::new(0, 0, 100, 50)));
        assert_eq!(layout.child_rect(2, rect), Some(Rect::new(100, 0, 200, 50)));
        assert_eq!(layout.child_rect(9, rect), None, "a foreign id is not placed");
        // A mutation invalidates the cache: four entries share the height instead of two.
        layout.add_row_pair(3, 4);
        assert_eq!(layout.child_rect(1, rect), Some(Rect::new(0, 0, 100, 25)));
    }
}
