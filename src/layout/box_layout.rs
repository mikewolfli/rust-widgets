// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Box layout manager — arranges items in a single row or column.
use super::{ChildInfo, Layout, LayoutConstraints, LayoutContext, Orientation, SizePolicy};
use crate::compat::{Any, Vec};
use crate::core::Alignment;
use crate::core::{ObjectId, Rect, Size};
use crate::style::EdgeOffsets;
#[derive(Debug)]
struct BoxLayoutItem {
    widget_id: Option<ObjectId>,
    stretch: u32,
    constraints: LayoutConstraints,
    policy: SizePolicy,
    /// Cross-axis alignment for this item.
    ///
    /// `None` means the item is stretched to the full cross extent, which is what the
    /// layout has always done and what a CSS/参考工具包 box does by default. A value shrinks the
    /// item to the cross size it *asks for* (from [`ChildInfo::hints`]) and places it
    /// within the cross extent — the same split Flutter's `crossAxisAlignment` makes.
    /// cross_size: None,
    /// }
    /// ```
    align: Option<Alignment>,
    /// The cross-axis size used when `align` is set and the hint-aware path has no child info.
    ///
    /// `update_with_context` does not receive [`ChildInfo`] (its signature is the trait's), so an
    /// aligned item used to be stretched there while `arrange` honoured the hint — the same layout
    /// answered differently through its two entry points. This is the explicit cross size the
    /// caller can supply for that entry point; `arrange` prefers it over the hint so both agree.
    cross_size: Option<u32>,
    /// Distance from the item's top edge to its text baseline, for [`Alignment::Baseline`]
    /// (LAY-004).
    ///
    /// `None` means "baseline at the item's bottom edge", the same default the flex row
    /// applies: a row of items with no baseline information aligns their bottoms. This is a
    /// real measurement supplied by the caller (a text control knows its ascent), not a
    /// stand-in for the start edge.
    baseline: Option<u32>,
}
/// Linear layout that arranges items in one direction.
#[derive(Debug)]
pub struct BoxLayout {
    orientation: Orientation,
    spacing: u32,
    margin: u32,
    /// Four-way margins. `None` means the uniform [`Self::margin`] is used on every side.
    margins: Option<EdgeOffsets>,
    items: Vec<BoxLayoutItem>,
}
impl BoxLayout {
    /// Create a box layout with orientation, spacing and margin.
    pub fn new(orientation: Orientation, spacing: u32, margin: u32) -> Self {
        Self { orientation, spacing, margin, margins: None, items: Vec::new() }
    }

    /// Sets four-way margins, overriding the uniform [`Self::margin`].
    ///
    /// `margin` stays the fallback so a JSON document that only knows the uniform key keeps
    /// working. Radii are scaled by the device context on the same path as the uniform value.
    pub fn set_margins(&mut self, margins: EdgeOffsets) {
        self.margins = Some(margins);
    }

    /// The four-way margins, or `None` when the uniform [`Self::margin`] applies.
    pub fn margins(&self) -> Option<EdgeOffsets> {
        self.margins
    }

    /// Left/top/right/bottom insets, scaled by `scale`.
    fn scaled_insets(&self, scale: f32) -> (u32, u32, u32, u32) {
        match self.margins {
            Some(margins) => (
                (margins.left as f32 * scale).round() as u32,
                (margins.top as f32 * scale).round() as u32,
                (margins.right as f32 * scale).round() as u32,
                (margins.bottom as f32 * scale).round() as u32,
            ),
            None => {
                let uniform = (self.margin as f32 * scale).round() as u32;
                (uniform, uniform, uniform, uniform)
            }
        }
    }
    /// Returns layout orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Changes the orientation at runtime, without reordering the items.
    ///
    /// The existing stretch factors and constraints are axis-relative, so the same items
    /// are reused; only the axis they are laid out on changes.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
    }

    /// Inserts a widget at `index`, shifting the items at and after it.
    ///
    /// Returns `false` when `index` is past the append position. The stretch factor is
    /// treated exactly like [`Layout::add_widget`]'s (`0` becomes `1`).
    pub fn insert_widget_at(&mut self, index: usize, widget_id: ObjectId, stretch: u32) -> bool {
        if index > self.items.len() {
            return false;
        }
        self.items.insert(
            index,
            BoxLayoutItem {
                widget_id: Some(widget_id),
                stretch: stretch.max(1),
                constraints: LayoutConstraints::new(0, None),
                policy: SizePolicy::Expanding,
                align: None,
                cross_size: None,
                baseline: None,
            },
        );
        true
    }

    /// Inserts an elastic spacer at `index`, pushing later items back.
    ///
    /// Returns `false` when `index` is past the append position.
    pub fn insert_stretch(&mut self, index: usize, stretch: u32) -> bool {
        if index > self.items.len() {
            return false;
        }
        self.items.insert(
            index,
            BoxLayoutItem {
                widget_id: None,
                stretch: stretch.max(1),
                constraints: LayoutConstraints::new(0, None),
                policy: SizePolicy::Expanding,
                align: None,
                cross_size: None,
                baseline: None,
            },
        );
        true
    }
    /// Returns inter-item spacing.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }
    /// Sets or clears the cross-axis alignment of `widget_id`.
    ///
    /// Returns `false` when `widget_id` is not an item of this layout. `None` restores the
    /// default stretch. The item is placed at the cross size it asks for through
    /// [`ChildInfo::hints`] only when it is aligned; a stretched item ignores the hint on
    /// that axis, exactly as a CSS box's `align-self: stretch` does.
    pub fn set_item_alignment(&mut self, widget_id: ObjectId, align: Option<Alignment>) -> bool {
        match self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            Some(item) => {
                item.align = align;
                true
            }
            None => false,
        }
    }

    /// The cross-axis alignment of `widget_id`, or `None` when it stretches.
    pub fn item_alignment(&self, widget_id: ObjectId) -> Option<Alignment> {
        self.items.iter().find(|item| item.widget_id == Some(widget_id)).and_then(|item| item.align)
    }

    /// The explicit cross-axis size of `widget_id`, or `None` when it uses its hint/stretch.
    pub fn item_cross_size(&self, widget_id: ObjectId) -> Option<u32> {
        self.items
            .iter()
            .find(|item| item.widget_id == Some(widget_id))
            .and_then(|item| item.cross_size)
    }

    /// Sets (or clears, with `None`) the cross size used by an aligned item.
    ///
    /// Returns `false` when `widget_id` is not an item of this layout. This is the counterpart of
    /// [`Self::set_item_alignment`] for [`Layout::update_with_context`], which has no [`ChildInfo`]
    /// to read a preferred size from. A value larger than the band is clamped, so an aligned item
    /// can never be placed outside its parent.
    pub fn set_item_cross_size(&mut self, widget_id: ObjectId, size: Option<u32>) -> bool {
        match self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            Some(item) => {
                item.cross_size = size;
                true
            }
            None => false,
        }
    }

    /// The baseline measurement of `widget_id`, or `None` when it aligns at its bottom edge.
    pub fn item_baseline(&self, widget_id: ObjectId) -> Option<u32> {
        self.items
            .iter()
            .find(|item| item.widget_id == Some(widget_id))
            .and_then(|item| item.baseline)
    }

    /// Sets (or clears, with `None`) the baseline measurement of an item (LAY-004).
    ///
    /// Returns `false` when `widget_id` is not an item of this layout. The value is the
    /// distance from the item's top edge to its text baseline — a text control knows its
    /// ascent. `None` (the default) means "baseline at the item's bottom edge", so a row of
    /// items with no measurements aligns their bottoms, exactly as the flex row does.
    pub fn set_item_baseline(&mut self, widget_id: ObjectId, baseline: Option<u32>) -> bool {
        match self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            Some(item) => {
                item.baseline = baseline;
                true
            }
            None => false,
        }
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
            align: None,
            cross_size: None,
            baseline: None,
        });
    }

    /// Adds an empty spacer that is exactly `size` pixels on the major axis.
    ///
    /// [`Self::add_spacer`] is elastic by construction (`stretch.max(1)` with an
    /// `Expanding` policy), so it cannot express the reference toolkit's `spacer(w, h, Fixed)` or wx's
    /// `AddSpacer(size)`. A fixed spacer is the "push these two items `size` apart and
    /// give the leftover to the others" shape, which is common in toolbars and dialog
    /// button rows. The minor axis is unconstrained; a spacer paints nothing.
    ///
    /// The cap is expressed through the item's own constraints, so the allocator is the
    /// one that enforces it (the same path that honours `set_constraints`).
    pub fn add_fixed_spacer(&mut self, size: u32) {
        self.items.push(BoxLayoutItem {
            widget_id: None,
            stretch: 1,
            constraints: LayoutConstraints::new(size, Some(size)),
            policy: SizePolicy::Fixed,
            align: None,
            cross_size: None,
            baseline: None,
        });
    }
    /// Sets size constraints for an existing widget item.
    pub fn set_constraints(&mut self, widget_id: ObjectId, constraints: LayoutConstraints) {
        if let Some(item) = self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            item.constraints = constraints;
        }
    }
    /// Sets size policy for an existing widget item.
    pub fn set_size_policy(&mut self, widget_id: ObjectId, policy: SizePolicy) {
        if let Some(item) = self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            item.policy = policy;
        }
    }

    /// The stretch factor of the item at `index`, or `None` when `index` is out of range.
    ///
    /// This addresses the item list, including spacers; [`child_ids`](Layout::child_ids) skips
    /// spacers, so use [`index_of`](Layout::index_of) for a widget-id index.
    pub fn stretch_factor(&self, index: usize) -> Option<u32> {
        self.items.get(index).map(|item| item.stretch)
    }

    /// Replaces the stretch factor of the item at `index`.
    ///
    /// `0` becomes `1`, matching every insertion path (a zero weight would make an item
    /// unaddressable by the allocator). Returns `false` when `index` is out of range.
    pub fn set_stretch_factor(&mut self, index: usize, stretch: u32) -> bool {
        match self.items.get_mut(index) {
            Some(item) => {
                item.stretch = stretch.max(1);
                true
            }
            None => false,
        }
    }

    /// The stretch factor of `widget_id`, or `None` when it is not a widget item.
    pub fn widget_stretch_factor(&self, widget_id: ObjectId) -> Option<u32> {
        self.items.iter().find(|item| item.widget_id == Some(widget_id)).map(|item| item.stretch)
    }

    /// Replaces the stretch factor of `widget_id`.
    ///
    /// `0` becomes `1`. Returns `false` when `widget_id` is not an item of this layout. Spacers
    /// have no id and are addressed through [`Self::set_stretch_factor`].
    pub fn set_widget_stretch_factor(&mut self, widget_id: ObjectId, stretch: u32) -> bool {
        match self.items.iter_mut().find(|item| item.widget_id == Some(widget_id)) {
            Some(item) => {
                item.stretch = stretch.max(1);
                true
            }
            None => false,
        }
    }
    /// Splits `primary` pixels across the items, honouring each item's constraints.
    ///
    /// # The two invariants this must not break
    ///
    /// 1. `sum(assigned) <= primary` — children that together need more than the parent
    ///    must not be placed partly outside it. Overflow here is visible as a control
    ///    painted over its neighbour, and it is reachable from the public
    ///    `set_constraints` API, so it cannot be left to the caller to avoid.
    /// 2. Each item's `min` is honoured *when the space can satisfy all of them*. When it
    ///    cannot — two 80px minima in a 100px row — no assignment satisfies both, so the
    ///    shortfall is distributed proportionally to the minima instead of being applied
    ///    inconsistently (the previous single-pass shrink loop reduced some items below
    ///    their minimum while leaving others at it, so the result depended on item order).
    fn allocate_major_lengths(&self, primary: u32) -> Vec<u32> {
        if self.items.is_empty() {
            return Vec::new();
        }
        // The stretch total is summed in `u64`: two spacers of weight `u32::MAX` must not overflow
        // a `u32` (a debug panic, and a wrapped total in release). Computing each share in `u64`
        // also keeps the weighted distribution exact instead of saturating the product.
        let total_stretch: u64 =
            self.items.iter().map(|item| item.stretch as u64).sum::<u64>().max(1);
        let mut assigned = Vec::with_capacity(self.items.len());
        for item in &self.items {
            let mut major = match item.policy {
                SizePolicy::Fixed => item.constraints.max.unwrap_or(item.constraints.min),
                SizePolicy::Maximum => {
                    let share = (primary as u64 * item.stretch as u64 / total_stretch) as u32;
                    item.constraints.max.unwrap_or(share).min(share)
                }
                SizePolicy::Ignored => {
                    (primary as u64 * item.stretch as u64 / total_stretch) as u32
                }
                SizePolicy::Minimum | SizePolicy::Expanding | SizePolicy::MinimumExpanding => {
                    (primary as u64 * item.stretch as u64 / total_stretch) as u32
                }
                // LAY-002: `Preferred` is no longer synonymous with `Expanding`. It strictly
                // hugs the size its constraints state — the adapter's `min` is both the floor
                // and the preferred value (see `LayoutConstraints::to_axis_hints`) — so it
                // starts there and never joins the leftover-space distribution below. Only
                // the strategies that ask for room (`Expanding`, `MinimumExpanding`, and the
                // unweighted `Minimum`/`Ignored`) take a stretch-weighted share.
                SizePolicy::Preferred => item.constraints.min,
            };
            major = major.max(item.constraints.min);
            if let Some(max) = item.constraints.max {
                major = major.min(max.max(item.constraints.min));
            }
            assigned.push(major);
        }

        // A `Fixed` item's size is decided by its constraints alone, so the grow pass must
        // not stretch it toward the container. Treating a missing `max` as `u32::MAX` did
        // exactly that: a single `Fixed` item with `min = 20` and no `max` in a 100px parent
        // was filled to 100. A `Maximum` item is capped the same way, and — since LAY-002 —
        // a `Preferred` item hugs the size its constraints state, so the leftover space is
        // distributed only among the items that ask for it. Any of these may still be
        // *scaled down* in the shortfall pass below, which is why only this pass skips them.
        let skips_grow = |index: usize| {
            matches!(
                self.items[index].policy,
                SizePolicy::Fixed | SizePolicy::Maximum | SizePolicy::Preferred
            )
        };

        // `min` is a hard floor only while the parent can pay for every floor. When the
        // floors alone exceed `primary`, they are scaled down proportionally: every item
        // then falls short by the same fraction, which is the only order-independent
        // answer, and invariant 1 is restored before the grow/shrink passes run.
        //
        // `total_min` and each numerator are computed in `u64`: summing several large
        // minima in `u32` overflowed, and the proportional share used `saturating_mul` in
        // `u32`, which saturated the numerator and biased the split heavily toward the last
        // item. Widening makes both the sum and the products exact.
        let total_min: u64 = self.items.iter().map(|item| item.constraints.min as u64).sum();
        if total_min > primary as u64 {
            let budget = primary;
            let mut scaled = Vec::with_capacity(self.items.len());
            let mut consumed = 0u32;
            for (index, item) in self.items.iter().enumerate() {
                // The last item takes the remainder rather than its own rounded share, so
                // the pieces always add up to exactly `budget`.
                let share = if index + 1 == self.items.len() {
                    budget.saturating_sub(consumed)
                } else {
                    (budget as u64 * item.constraints.min as u64 / total_min.max(1)) as u32
                };
                let share = share.min(budget.saturating_sub(consumed));
                consumed = consumed.saturating_add(share);
                scaled.push(share);
            }
            return scaled;
        }

        // Grow/shrink are *batch* passes, not one-pixel loops. The difference is not cosmetic:
        // a legal constraint such as `LayoutConstraints::new(0, Some(u32::MAX))` on a `Fixed` item
        // makes the initial allocation `u32::MAX`, so the old `while total > primary { ... -1 }`
        // loop needed ~4.29e9 iterations (minutes of a stalled layout thread) on an item count of
        // one. Each iteration below moves a whole batch of pixels, so the complexity is bounded by
        // the number of items, never by the pixel deficit.
        let mut total_assigned = assigned.iter().fold(0u64, |acc, &v| acc + v as u64);
        let primary_u64 = primary as u64;
        // Largest distance any item can still grow. A `Fixed` item has already taken its exact
        // constrained size, so it contributes 0 — the grow pass must not stretch it toward the
        // container (the previous per-pixel loop skipped the same items).
        let mut grow_room: u64 = 0;
        for (index, item) in self.items.iter().enumerate() {
            if skips_grow(index) {
                continue;
            }
            let max_allowed = item.constraints.max.unwrap_or(u32::MAX).max(item.constraints.min);
            grow_room += max_allowed.saturating_sub(assigned[index]) as u64;
        }
        if total_assigned < primary_u64 {
            let deficit = primary_u64 - total_assigned;
            if grow_room > 0 {
                let to_grow = deficit.min(grow_room);
                let mut remainder = to_grow;
                for (index, item) in self.items.iter().enumerate() {
                    if remainder == 0 {
                        break;
                    }
                    if skips_grow(index) {
                        continue;
                    }
                    let max_allowed =
                        item.constraints.max.unwrap_or(u32::MAX).max(item.constraints.min);
                    let room = max_allowed.saturating_sub(assigned[index]) as u64;
                    if room == 0 {
                        continue;
                    }
                    // Give each item its proportional share (in u64, so the product is exact)
                    // capped at its remaining room; `remainder` tracks what is still unassigned.
                    let share = ((to_grow as u128 * room as u128) / grow_room as u128) as u64;
                    let give = share.min(room).min(remainder);
                    assigned[index] = assigned[index].saturating_add(give as u32);
                    remainder -= give;
                }
                // Proportional rounding can leave a few pixels unassigned. Hand them to any item
                // that still has room so the parent is filled exactly (never partially).
                if remainder > 0 {
                    for (index, item) in self.items.iter().enumerate() {
                        if remainder == 0 {
                            break;
                        }
                        if skips_grow(index) {
                            continue;
                        }
                        let max_allowed =
                            item.constraints.max.unwrap_or(u32::MAX).max(item.constraints.min);
                        let room = max_allowed.saturating_sub(assigned[index]) as u64;
                        let give = room.min(remainder);
                        assigned[index] = assigned[index].saturating_add(give as u32);
                        remainder -= give;
                    }
                }
                let _ = to_grow - remainder;
            }
        } else if total_assigned > primary_u64 {
            let excess = total_assigned - primary_u64;
            // Largest distance any item can still shrink down to its own minimum. A `Fixed` item
            // has no `max`-driven slack, but it may still be scaled down here, exactly as before.
            let shrink_room: u64 = self
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| assigned[index].saturating_sub(item.constraints.min) as u64)
                .sum();
            if shrink_room > 0 {
                let to_shrink = excess.min(shrink_room);
                let mut remainder = to_shrink;
                for (index, item) in self.items.iter().enumerate().rev() {
                    if remainder == 0 {
                        break;
                    }
                    let room = assigned[index].saturating_sub(item.constraints.min) as u64;
                    if room == 0 {
                        continue;
                    }
                    let share = ((to_shrink as u128 * room as u128) / shrink_room as u128) as u64;
                    let take = share.min(room).min(remainder);
                    assigned[index] = assigned[index].saturating_sub(take as u32);
                    remainder -= take;
                }
                total_assigned -= to_shrink - remainder;
            }
            if total_assigned > primary_u64 {
                // Every item is now at its own minimum and the total is still too large.
                // Rather than shaving one pixel at a time until the budget is met — which
                // is O(excess) — compute each item's proportional removal in one batch and
                // hand the rounding remainder to the leading items.
                let excess = (total_assigned - primary_u64) as u32;
                let sum: u32 = assigned.iter().copied().sum();
                if excess > 0 && sum > 0 {
                    let mut removed = 0u32;
                    for value in &mut assigned {
                        let take = ((excess as u64) * (*value as u64) / sum as u64) as u32;
                        let take = take.min(*value);
                        *value -= take;
                        removed = removed.saturating_add(take);
                    }
                    let mut left = excess.saturating_sub(removed);
                    for value in &mut assigned {
                        if left == 0 {
                            break;
                        }
                        let take = (*value).min(left);
                        *value -= take;
                        left -= take;
                    }
                }
            }
        }
        assigned
    }

    /// The hint-aware arrangement at an explicit scale (LAY-003).
    ///
    /// The spacing between items and the margins around them follow the device context the
    /// way [`Layout::update_with_context`](Self::update_with_context) already does; the
    /// scaled values shrink the primary budget before the allocator runs, so a scaled gap
    /// costs the children room exactly as a nominal gap does.
    fn arrange_with_scale(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        scale: f32,
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        if self.items.is_empty() {
            return;
        }
        let scaled_spacing = (self.spacing as f32 * scale).round() as u32;
        let (left, top, right, bottom) = self.scaled_insets(scale);
        let gaps = (self.items.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width.saturating_sub(left).saturating_sub(right),
            Orientation::Vertical => rect.height.saturating_sub(top).saturating_sub(bottom),
        }
        .saturating_sub(gaps * scaled_spacing);
        let cross_extent = match self.orientation {
            Orientation::Horizontal => rect.height.saturating_sub(top).saturating_sub(bottom),
            Orientation::Vertical => rect.width.saturating_sub(left).saturating_sub(right),
        };
        let majors = self.allocate_major_lengths(primary);
        // LAY-004: the shared baseline is decided before placement, from every
        // `Baseline`-aligned item's placed cross extent (the explicit size, else the hint's
        // preferred extent, clamped to the band).
        let shared_baseline = self.shared_baseline(|index| {
            let item = &self.items[index];
            let info = item.widget_id.and_then(|id| ChildInfo::find_at(children, index, id));
            item.cross_size
                .or_else(|| info.map(|info| crate::layout::hints::axis(&info.hints, true).pref))
                .map(|requested| requested.min(cross_extent))
        });
        let mut cursor_x = rect.x + left as i32;
        let mut cursor_y = rect.y + top as i32;
        for (index, item) in self.items.iter().enumerate() {
            let cross_virtual = self.orientation == Orientation::Horizontal;
            let info = item.widget_id.and_then(|id| ChildInfo::find_at(children, index, id));
            let mut major = majors.get(index).copied().unwrap_or(0);
            // N03: the hint's main-axis bounds are the converged form of
            // `LayoutConstraints`/`SizePolicy`. Applying them here means the hint-aware path
            // has one source for "how much room does this child want", not two that can
            // disagree; the struct fields stay as the adapter used when no hint is supplied.
            if let Some(info) = info {
                let main = crate::layout::hints::axis(&info.hints, !cross_virtual);
                major = major.max(main.min);
                if main.max > 0 {
                    major = major.min(main.max.max(main.min));
                }
            }
            let mut cross = cross_extent;
            let mut cross_offset = 0;
            if let Some(align) = item.align {
                // An explicit cross size wins over the hint: the caller stated the size both
                // entry points should use, and the hint is the fallback for the one that can
                // read it.
                let requested = item.cross_size.or_else(|| {
                    info.map(|info| crate::layout::hints::axis(&info.hints, cross_virtual).pref)
                });
                if let Some(requested) = requested {
                    // A hint larger than the box is clamped: the box cannot give what it
                    // does not have, and overflowing the cross axis would paint outside
                    // the parent.
                    cross = requested.min(cross_extent);
                    cross_offset = match align {
                        Alignment::Center => (cross_extent - cross) / 2,
                        Alignment::Right | Alignment::Bottom => cross_extent - cross,
                        // LAY-004: baselines align at one shared cross coordinate — the
                        // tallest baseline decides the origin, so a shorter item is pushed
                        // down by exactly the difference. With no baseline information the
                        // offset is 0, the start alignment an unspecified arm always had.
                        Alignment::Baseline => {
                            let own = item.baseline.unwrap_or(cross);
                            shared_baseline.map_or(0, |shared| shared.saturating_sub(own))
                        }
                        // Left/Top are the start of either cross axis; so is any
                        // unspecified alignment.
                        _ => 0,
                    };
                }
            }
            let child_rect = match self.orientation {
                Orientation::Horizontal => {
                    Rect::new(cursor_x, cursor_y.saturating_add(cross_offset as i32), major, cross)
                }
                Orientation::Vertical => {
                    Rect::new(cursor_x.saturating_add(cross_offset as i32), cursor_y, cross, major)
                }
            };
            if let Some(widget_id) = item.widget_id {
                out(widget_id, child_rect);
            }
            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + scaled_spacing) as i32,
                Orientation::Vertical => cursor_y += (major + scaled_spacing) as i32,
            }
        }
    }

    /// The shared baseline of the [`Alignment::Baseline`]-aligned items (LAY-004), or `None`
    /// when the row has no usable baseline.
    ///
    /// The tallest baseline decides the row's cross origin, exactly as the flex row does: an
    /// item's baseline is the distance from its top edge to its text baseline, defaulting to
    /// the item's *placed* cross extent (its bottom edge) when the caller supplied no
    /// measurement. `placed_cross` yields each item's placed cross extent — `None` for an
    /// item whose cross size the calling entry cannot know, which then cannot state a
    /// bottom-edge baseline and is skipped. A vertical box (whose cross axis is horizontal)
    /// has no baselines across it, so it always answers `None` and every `Baseline` item
    /// keeps start alignment — the flex column's reading.
    fn shared_baseline(&self, placed_cross: impl Fn(usize) -> Option<u32>) -> Option<u32> {
        if self.orientation != Orientation::Horizontal {
            return None;
        }
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.align == Some(Alignment::Baseline))
            .filter_map(|(index, item)| {
                placed_cross(index).map(|cross| item.baseline.unwrap_or(cross))
            })
            .max()
    }
}
impl Layout for BoxLayout {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /// The box's minimum is the sum of the items' floors plus the gaps and margins.
    fn minimum_size(&self) -> Size {
        let (left, top, right, bottom) = self.scaled_insets(1.0);
        let gaps = self.spacing.saturating_mul(self.items.len().saturating_sub(1) as u32);
        let mut main: u32 = 0;
        let mut cross: u32 = 0;
        for item in &self.items {
            let item_cross = item.cross_size.unwrap_or(item.constraints.min);
            main = main.saturating_add(item.constraints.min);
            cross = cross.max(item_cross);
        }
        main = main.saturating_add(gaps);
        match self.orientation {
            Orientation::Horizontal => Size::new(
                main.saturating_add(left).saturating_add(right),
                cross.saturating_add(top).saturating_add(bottom),
            ),
            Orientation::Vertical => Size::new(
                cross.saturating_add(left).saturating_add(right),
                main.saturating_add(top).saturating_add(bottom),
            ),
        }
    }

    fn expanding_directions(&self) -> (bool, bool) {
        let (main_expands, cross_expands) = self.items.iter().fold((false, false), |acc, item| {
            (
                acc.0
                    || matches!(
                        item.policy,
                        SizePolicy::Expanding | SizePolicy::MinimumExpanding | SizePolicy::Ignored
                    ),
                acc.1
                    || (item.cross_size.is_none() && matches!(item.policy, SizePolicy::Expanding)),
            )
        });
        match self.orientation {
            Orientation::Horizontal => (main_expands, cross_expands),
            Orientation::Vertical => (cross_expands, main_expands),
        }
    }

    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        if self.items.is_empty() {
            return;
        }
        // Spacing follows the **larger** of the layout scale and the text scale.
        //
        // `LayoutContext::font_scale` is the device's text-size preference, and the two are
        // separate facts: a HiDPI screen needs more logical spacing, and a device whose text is set
        // larger needs more room between controls even at the same DPI. Taking the maximum is the
        // conservative reading — a control whose font grew but whose padding did not would have its
        // text touching its own border, which is the defect the field exists to let a layout avoid.
        //
        // The field had no reader at all before this, so a 2x text preference grew the glyphs (via
        // the theme's font token) and left every gap at its nominal size.
        let scale = context.layout_scale.max(context.font_scale);
        let scaled_spacing = (self.spacing as f32 * scale).round() as u32;
        let (left, top, right, bottom) = self.scaled_insets(scale);
        let gaps = (self.items.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width.saturating_sub(left).saturating_sub(right),
            Orientation::Vertical => rect.height.saturating_sub(top).saturating_sub(bottom),
        }
        .saturating_sub(gaps * scaled_spacing);
        let cross = match self.orientation {
            Orientation::Horizontal => rect.height.saturating_sub(top).saturating_sub(bottom),
            Orientation::Vertical => rect.width.saturating_sub(left).saturating_sub(right),
        };
        let majors = self.allocate_major_lengths(primary);
        // LAY-004: the shared baseline among the `Baseline`-aligned items, from their explicit
        // cross sizes (this entry has no `ChildInfo` to fall back to).
        let shared_baseline =
            self.shared_baseline(|index| self.items[index].cross_size.map(|size| size.min(cross)));
        let mut cursor_x = rect.x + left as i32;
        let mut cursor_y = rect.y + top as i32;
        for (index, item) in self.items.iter().enumerate() {
            let major = majors.get(index).copied().unwrap_or(0);
            // An aligned item shrinks to its explicit cross size and is placed within the band;
            // with no explicit size it keeps stretching, because this entry point has no
            // `ChildInfo` to ask. The hint-aware `arrange` prefers `cross_size` too, so both
            // entries agree whenever the caller supplied one.
            let mut cross = cross;
            let mut cross_offset = 0u32;
            if let Some(align) = item.align {
                if let Some(requested) = item.cross_size {
                    let band = cross;
                    cross = requested.min(band);
                    cross_offset = match align {
                        Alignment::Center => (band - cross) / 2,
                        Alignment::Right | Alignment::Bottom => band - cross,
                        // LAY-004: baselines align at one shared cross coordinate; with no
                        // baseline information the offset is 0, the start alignment an
                        // unspecified arm always had.
                        Alignment::Baseline => {
                            let own = item.baseline.unwrap_or(cross);
                            shared_baseline.map_or(0, |shared| shared.saturating_sub(own))
                        }
                        _ => 0,
                    };
                }
            }
            let child_rect = match self.orientation {
                Orientation::Horizontal => {
                    Rect::new(cursor_x, cursor_y.saturating_add(cross_offset as i32), major, cross)
                }
                Orientation::Vertical => {
                    Rect::new(cursor_x.saturating_add(cross_offset as i32), cursor_y, cross, major)
                }
            };
            if let Some(widget_id) = item.widget_id {
                // Grown to the device class's minimum touch area, as the flex layout does — the two
                // must agree or the same controls would be addressable in one container and not the
                // other. The cursor advances by the *allocated* major length either way, so growing
                // a child cannot push its siblings around.
                widgets(
                    widget_id,
                    crate::layout::types::grow_to_min_touch_size(
                        child_rect,
                        context.min_touch_size,
                    ),
                );
            }
            match self.orientation {
                Orientation::Horizontal => cursor_x += (major + scaled_spacing) as i32,
                Orientation::Vertical => cursor_y += (major + scaled_spacing) as i32,
            }
        }
    }

    /// The hint-aware path: an aligned item is placed at the cross size it asks for.
    ///
    /// This is the first layout to consume `ChildInfo` (LAY-N01). The main axis keeps its
    /// stretch allocation — a box divides the axis it owns — while the cross axis becomes
    /// the child's own answer when the caller asked for an alignment. A layout that ignored
    /// `children` would have had to be told each child's size in advance, which is the
    /// workaround `FlexLayout::set_child_sizes` exists for.
    fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect)) {
        // The nominal path stays at scale 1.0 so every existing caller's geometry is
        // byte-identical. `LayoutContext::default()` is deliberately not used here: it reads
        // the platform's text scale, so it is not the nominal answer (the same distinction
        // `FlexLayout::arrange` draws).
        self.arrange_with_scale(rect, children, 1.0, out);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn child_ids(&self) -> Vec<ObjectId> {
        self.items.iter().filter_map(|item| item.widget_id).collect()
    }
    fn has_child(&self, id: ObjectId) -> bool {
        self.items.iter().any(|item| item.widget_id == Some(id))
    }
    fn widget_count(&self) -> usize {
        self.items.iter().filter(|item| item.widget_id.is_some()).count()
    }
    fn index_of(&self, id: ObjectId) -> Option<usize> {
        let mut index = 0usize;
        for item in &self.items {
            if let Some(widget_id) = item.widget_id {
                if widget_id == id {
                    return Some(index);
                }
                index += 1;
            }
        }
        None
    }
    fn child_rect(&self, id: ObjectId, rect: Rect) -> Option<Rect> {
        if !self.has_child(id) {
            return None;
        }
        let mut found = None;
        self.update(rect, &mut |child, child_rect| {
            if child == id && found.is_none() {
                found = Some(child_rect);
            }
        });
        found
    }
    fn clear(&mut self) {
        self.items.clear();
    }
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.items.push(BoxLayoutItem {
            widget_id: Some(widget_id),
            stretch: stretch.max(1),
            constraints: LayoutConstraints::new(0, None),
            policy: SizePolicy::Expanding,
            align: None,
            cross_size: None,
            baseline: None,
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
        let (left, top, right, bottom) = self.scaled_insets(1.0);
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width.saturating_sub(left).saturating_sub(right),
            Orientation::Vertical => rect.height.saturating_sub(top).saturating_sub(bottom),
        }
        .saturating_sub(gaps * self.spacing);
        let cross = match self.orientation {
            Orientation::Horizontal => rect.height.saturating_sub(top).saturating_sub(bottom),
            Orientation::Vertical => rect.width.saturating_sub(left).saturating_sub(right),
        };
        let majors = self.allocate_major_lengths(primary);
        // LAY-004: the shared baseline among the `Baseline`-aligned items, from their explicit
        // cross sizes (this entry has no `ChildInfo` either).
        let shared_baseline =
            self.shared_baseline(|index| self.items[index].cross_size.map(|size| size.min(cross)));
        let mut cursor_x = rect.x + left as i32;
        let mut cursor_y = rect.y + top as i32;
        for (index, item) in self.items.iter().enumerate() {
            let major = majors.get(index).copied().unwrap_or(0);
            // The explicit cross size is honoured here too: this entry point has no `ChildInfo`
            // either, but a caller that stated a size expects both entries to use it.
            let mut cross = cross;
            let mut cross_offset = 0u32;
            if let Some(align) = item.align {
                if let Some(requested) = item.cross_size {
                    let band = cross;
                    cross = requested.min(band);
                    cross_offset = match align {
                        Alignment::Center => (band - cross) / 2,
                        Alignment::Right | Alignment::Bottom => band - cross,
                        // LAY-004: baselines align at one shared cross coordinate; with no
                        // baseline information the offset is 0, the start alignment an
                        // unspecified arm always had.
                        Alignment::Baseline => {
                            let own = item.baseline.unwrap_or(cross);
                            shared_baseline.map_or(0, |shared| shared.saturating_sub(own))
                        }
                        _ => 0,
                    };
                }
            }
            let child_rect = match self.orientation {
                Orientation::Horizontal => {
                    Rect::new(cursor_x, cursor_y.saturating_add(cross_offset as i32), major, cross)
                }
                Orientation::Vertical => {
                    Rect::new(cursor_x.saturating_add(cross_offset as i32), cursor_y, cross, major)
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
    // G-403: the trait no longer defaults these entries; each forward reproduces the previous fallback behaviour.
    fn arrange_with_context(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        context: &LayoutContext,
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        // LAY-003: the context is consumed, not dropped. Spacing and margins follow the
        // **larger** of the layout scale and the text scale — the same conservative reading
        // `update_with_context` applies, because a device whose text is set larger needs more
        // room between controls even at the same DPI. The context's remaining field,
        // `min_touch_size`, has no meaningful use on this entry: the hint-aware path is a
        // geometry query callers pin against exact rects, so growing a child here would break
        // that contract — the touch floor stays an update-path concern.
        let scale = context.layout_scale.max(context.font_scale);
        self.arrange_with_scale(rect, children, scale, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    /// Collects the rects a horizontal box layout produces, keyed by widget id.
    fn placed(layout: &BoxLayout, rect: Rect) -> Vec<(ObjectId, Rect)> {
        let mut out = Vec::new();
        layout.update(rect, &mut |id, r| out.push((id, r)));
        out
    }

    fn rect_of(out: &[(ObjectId, Rect)], id: ObjectId) -> Rect {
        out.iter().find(|(i, _)| *i == id).map(|(_, r)| *r).expect("the id was placed")
    }

    /// The explicit cross size is honoured by every entry point, including the one that receives
    /// no `ChildInfo`.
    ///
    /// `update_with_context` used to stretch an aligned item while `arrange` shrank it to the
    /// hint, so the same layout produced two different rects depending on how the caller drove
    /// it. `set_item_cross_size` is the size both entries read.
    #[test]
    fn an_explicit_cross_size_is_honoured_by_both_entry_points() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        assert!(layout.set_item_alignment(1, Some(Alignment::Center)));
        assert!(layout.set_item_cross_size(1, Some(20)));
        assert_eq!(layout.item_cross_size(1), Some(20));

        let rect = Rect::new(0, 0, 100, 100);
        let plain = placed(&layout, rect);
        let from_update = rect_of(&plain, 1);
        assert_eq!(from_update.height, 20, "the explicit size replaces the 100 px band");
        assert_eq!(from_update.y, 40, "and the item is centred in that band");
        assert_eq!(from_update.width, 100, "the main axis still fills the box");

        let context = LayoutContext {
            layout_scale: 1.0,
            font_scale: 1.0,
            min_touch_size: crate::core::Size::new(0, 0),
        };
        let mut out = Vec::new();
        layout.update_with_context(rect, &context, &mut |id, r| out.push((id, r)));
        assert_eq!(
            rect_of(&out, 1),
            from_update,
            "both entry points must place the item the same way"
        );

        // The hint-aware path prefers the explicit size over a conflicting hint.
        let children = [ChildInfo::new(
            1,
            crate::layout::Hints {
                width: crate::layout::AxisHints::unconstrained(),
                height: crate::layout::AxisHints::fixed(80),
            },
        )];
        let mut arranged = Vec::new();
        layout.arrange(rect, &children, &mut |id, r| arranged.push((id, r)));
        assert_eq!(rect_of(&arranged, 1).height, 20, "explicit size outranks the hint");

        assert!(layout.set_item_cross_size(1, None));
        assert_eq!(layout.item_cross_size(1), None);
    }

    /// An aligned item without an explicit size still stretches through the context entry, which
    /// cannot read a hint. This is the documented fallback, not a silent guess.
    #[test]
    fn an_aligned_item_without_a_cross_size_keeps_the_band_in_update_with_context() {
        let mut layout = BoxLayout::new(Orientation::Vertical, 0, 0);
        layout.add_widget(1, 1);
        assert!(layout.set_item_alignment(1, Some(Alignment::Center)));

        let context = LayoutContext {
            layout_scale: 1.0,
            font_scale: 1.0,
            min_touch_size: crate::core::Size::new(0, 0),
        };
        let mut out = Vec::new();
        layout.update_with_context(Rect::new(0, 0, 100, 100), &context, &mut |id, r| {
            out.push((id, r))
        });
        assert_eq!(rect_of(&out, 1).width, 100, "no size to shrink to, so it stretches");
    }

    /// Two items of weight `u32::MAX` must not overflow the stretch sum.
    ///
    /// The total was a `u32`, so this case panicked in debug and, in release, wrapped the total
    /// and mis-shared the container. Both items now split the container evenly and stay inside it.
    #[test]
    fn max_box_stretches_do_not_overflow_or_overrun_the_container() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, u32::MAX);
        layout.add_widget(2, u32::MAX);

        let rect = Rect::new(0, 0, 100, 50);
        let out = placed(&layout, rect);

        let a = rect_of(&out, 1);
        let b = rect_of(&out, 2);
        assert_eq!(a.width, 50, "equal max weights split the container evenly");
        assert_eq!(b.width, 50, "equal max weights split the container evenly");
        assert_eq!(a.width + b.width, 100, "the items must not exceed the container");
        assert!(b.x + b.width as i32 <= rect.x + rect.width as i32, "right item: {b:?}");
    }

    /// Mixed max and minimal weights must share the container without overrunning it.
    #[test]
    fn mixed_box_stretches_share_the_container_without_overrunning() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, u32::MAX);
        layout.add_widget(2, 1);

        let rect = Rect::new(0, 0, 100, 50);
        let out = placed(&layout, rect);

        let a = rect_of(&out, 1);
        let b = rect_of(&out, 2);
        assert!(a.width >= b.width, "the max-weight item gets the larger share");
        assert_eq!(a.width + b.width, 100, "the items must not exceed the container");
        assert!(b.x + b.width as i32 <= rect.x + rect.width as i32, "right item: {b:?}");
    }

    /// The defect this pins: a `Fixed` item with no `max` was treated as having `u32::MAX`
    /// room, so the grow pass stretched it to the whole container.
    ///
    /// A `Fixed` item is fixed by definition — its size is `max.unwrap_or(min)` — and must
    /// keep it no matter how much room is left over. Both axes are checked, with and without
    /// an explicit `max`.
    #[test]
    fn a_fixed_item_without_a_max_keeps_its_size() {
        // Horizontal: min 20, no max, in a 100px parent.
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.set_constraints(1, LayoutConstraints::new(20, None));
        layout.set_size_policy(1, SizePolicy::Fixed);
        let out = placed(&layout, Rect::new(0, 0, 100, 50));
        assert_eq!(rect_of(&out, 1).width, 20, "a Fixed item is not filled to the parent width");

        // Horizontal: min 20, explicit max 30, in a 100px parent.
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.set_constraints(1, LayoutConstraints::new(20, Some(30)));
        layout.set_size_policy(1, SizePolicy::Fixed);
        let out = placed(&layout, Rect::new(0, 0, 100, 50));
        assert_eq!(rect_of(&out, 1).width, 30, "a Fixed item keeps its explicit max");

        // Vertical: min 20, no max, in a 100px parent.
        let mut layout = BoxLayout::new(Orientation::Vertical, 0, 0);
        layout.add_widget(1, 1);
        layout.set_constraints(1, LayoutConstraints::new(20, None));
        layout.set_size_policy(1, SizePolicy::Fixed);
        let out = placed(&layout, Rect::new(0, 0, 50, 100));
        assert_eq!(rect_of(&out, 1).height, 20, "a Fixed item is not filled to the parent height");
    }

    /// A `Fixed` sibling must not absorb leftover room that belongs to the expanding items,
    /// and the two together must still sum to the container.
    #[test]
    fn leftover_room_goes_to_expanding_items_not_to_fixed_ones() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1); // becomes Fixed via policy below
        layout.add_widget(2, 1); // stays Expanding
        layout.set_constraints(1, LayoutConstraints::new(20, None));
        layout.set_size_policy(1, SizePolicy::Fixed);

        let out = placed(&layout, Rect::new(0, 0, 100, 50));
        assert_eq!(rect_of(&out, 1).width, 20, "the Fixed item keeps its size");
        assert_eq!(rect_of(&out, 2).width, 80, "the expanding item takes the leftover");
        assert_eq!(rect_of(&out, 1).width + rect_of(&out, 2).width, 100);
    }

    /// The defect this pins: the shortfall split summed the minima in `u32` and computed each
    /// numerator with `u32::saturating_mul`, so two large equal minima produced a wildly
    /// unequal split (the first share saturated, the last took almost the whole budget).
    #[test]
    fn a_large_shortfall_is_split_unbiased() {
        let min = 2_000_000_000u32;
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_constraints(1, LayoutConstraints::new(min, None));
        layout.set_constraints(2, LayoutConstraints::new(min, None));

        let out = placed(&layout, Rect::new(0, 0, 2_000_000_000, 50));
        let a = rect_of(&out, 1);
        let b = rect_of(&out, 2);
        // Equal minima must receive equal shares; the remainder lands on one of them.
        assert!(
            a.width.abs_diff(b.width) <= 1,
            "equal minima must split the budget near-evenly, got {} and {}",
            a.width,
            b.width
        );
        assert_eq!(a.width + b.width, 2_000_000_000, "the pieces sum to the budget exactly");
    }

    /// The summed minima can exceed `u32::MAX`; the split must still work and sum to the
    /// budget instead of overflowing the running total.
    #[test]
    fn a_shortfall_whose_minima_sum_overflows_u32_still_sums_to_the_budget() {
        let min = 3_000_000_000u32; // 2 * this > u32::MAX
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_constraints(1, LayoutConstraints::new(min, None));
        layout.set_constraints(2, LayoutConstraints::new(min, None));

        let out = placed(&layout, Rect::new(0, 0, 1_000_000_000, 50));
        let a = rect_of(&out, 1);
        let b = rect_of(&out, 2);
        assert_eq!(a.width + b.width, 1_000_000_000, "the pieces sum to the budget exactly");
        assert!(a.width.abs_diff(b.width) <= 1, "an overflowing min sum still splits evenly");
    }

    /// A legal `Fixed` constraint with `max = u32::MAX` must not stall the layout. The old
    /// per-pixel shrink loop needed ~4.29e9 iterations to bring the initial `u32::MAX`
    /// allocation down to the 100px parent; the batch pass converges in one traversal.
    ///
    /// The assertions also pin the resulting geometry: the sum must land inside the parent.
    #[test]
    fn a_max_fixed_constraint_is_allocated_in_one_batch_not_by_pixel_iteration() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.set_constraints(1, LayoutConstraints::new(0, Some(u32::MAX)));
        layout.set_size_policy(1, SizePolicy::Fixed);

        let rect = Rect::new(0, 0, 100, 50);
        let out = placed(&layout, rect);
        let a = rect_of(&out, 1);
        assert!(a.width <= 100, "the allocation must be inside the parent, got {}", a.width);
        assert!(a.x + a.width as i32 <= rect.x + rect.width as i32, "right edge: {a:?}");
    }

    /// A huge grow deficit (an expanding item with room far beyond the parent) must be satisfied
    /// by bounded proportional batches, and the total must still equal the parent exactly.
    #[test]
    fn a_large_grow_deficit_is_shared_without_per_pixel_iteration() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        // Both start at 0 (min 0) and have effectively unbounded room.
        layout.set_constraints(1, LayoutConstraints::new(0, Some(u32::MAX)));
        layout.set_constraints(2, LayoutConstraints::new(0, Some(u32::MAX)));

        let out = placed(&layout, Rect::new(0, 0, 100, 50));
        let a = rect_of(&out, 1);
        let b = rect_of(&out, 2);
        assert_eq!(a.width + b.width, 100, "the whole parent is filled exactly");
        assert!(a.width.abs_diff(b.width) <= 1, "equal rooms split near-evenly");
    }

    /// Growing must respect an item's `max`: the surplus that the capped item cannot take is
    /// absorbed by a neighbour that still has room, and the total stays inside the parent.
    #[test]
    fn grow_batches_respect_max_and_pass_leftover_to_a_roomy_neighbour() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_constraints(1, LayoutConstraints::new(0, Some(30)));
        layout.set_constraints(2, LayoutConstraints::new(0, Some(u32::MAX)));

        let out = placed(&layout, Rect::new(0, 0, 100, 50));
        let a = rect_of(&out, 1);
        let b = rect_of(&out, 2);
        assert_eq!(a.width, 30, "the capped item stops at its max");
        assert_eq!(b.width, 70, "the roomy neighbour takes the leftover");
        assert_eq!(a.width + b.width, 100, "the parent is exactly filled");
    }

    /// Four-way margins inset each side independently; the uniform margin stays the fallback.
    #[test]
    fn four_way_margins_inset_each_side_independently() {
        let mut layout = BoxLayout::new(Orientation::Vertical, 0, 0);
        layout.add_widget(1, 1);
        layout.set_margins(crate::style::EdgeOffsets { top: 5, right: 20, bottom: 0, left: 10 });

        let out = placed(&layout, Rect::new(0, 0, 100, 100));
        let child = rect_of(&out, 1);
        assert_eq!(child.x, 10, "left inset");
        assert_eq!(child.y, 5, "top inset");
        assert_eq!(child.width, 70, "width pays both horizontal insets");
        assert_eq!(child.height, 95, "height pays both vertical insets");

        // Clearing back to the uniform margin restores symmetric insets.
        let mut uniform = BoxLayout::new(Orientation::Vertical, 0, 4);
        uniform.add_widget(1, 1);
        let out = placed(&uniform, Rect::new(0, 0, 100, 100));
        assert_eq!(rect_of(&out, 1), Rect::new(4, 4, 92, 92));
    }

    /// A fixed spacer takes exactly its size; the elastic items share what is left.
    #[test]
    fn a_fixed_spacer_does_not_stretch() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_fixed_spacer(40);
        layout.add_widget(2, 1);

        let out = placed(&layout, Rect::new(0, 0, 200, 10));
        let first = rect_of(&out, 1);
        let second = rect_of(&out, 2);
        assert_eq!(second.x - (first.x + first.width as i32), 40, "the gap is the fixed size");
        assert_eq!(first.width, 80, "the elastic items split the remainder");
        assert_eq!(second.width, 80);
    }

    /// An aligned item takes the cross size it asks for and is placed within the box.
    #[test]
    fn an_aligned_item_is_placed_at_its_requested_cross_size() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        assert!(layout.set_item_alignment(1, Some(Alignment::Center)));
        assert_eq!(layout.item_alignment(1), Some(Alignment::Center));

        let children = [ChildInfo::new(
            1,
            crate::layout::Hints {
                width: crate::layout::AxisHints::unconstrained(),
                height: crate::layout::AxisHints::fixed(20),
            },
        )];
        let mut placed = Vec::new();
        layout.arrange(Rect::new(0, 0, 100, 100), &children, &mut |id, rect| {
            placed.push((id, rect));
        });

        let (id, rect) = placed[0];
        assert_eq!(id, 1);
        assert_eq!(rect.height, 20, "the hint's cross size, not the box's");
        assert_eq!(rect.y, 40, "centred in the 100 px cross extent");
        assert_eq!(rect.width, 100, "the main axis is still the box's allocation");
    }

    /// A stretched item (no alignment) keeps filling the cross axis, as before.
    #[test]
    fn an_unaligned_item_still_stretches() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        let children = [ChildInfo::new(
            1,
            crate::layout::Hints {
                width: crate::layout::AxisHints::unconstrained(),
                height: crate::layout::AxisHints::fixed(20),
            },
        )];
        let mut placed = Vec::new();
        layout.arrange(Rect::new(0, 0, 100, 100), &children, &mut |_, rect| placed.push(rect));
        assert_eq!(placed[0], Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn inserting_and_reorienting_keeps_the_same_items() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(3, 1);
        assert!(layout.insert_widget_at(1, 2, 1));
        assert!(layout.insert_stretch(3, 0));
        assert!(!layout.insert_widget_at(99, 4, 1));
        assert_eq!(layout.child_ids(), vec![1, 2, 3]);

        layout.set_orientation(Orientation::Vertical);
        assert_eq!(layout.orientation(), Orientation::Vertical);
    }

    /// The four added policies are all accepted, and the maximum policy refuses extra room.
    #[test]
    fn the_extended_size_policies_are_honoured() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_constraints(1, LayoutConstraints::new(10, Some(10)));
        layout.set_size_policy(1, SizePolicy::Maximum);
        layout.set_size_policy(2, SizePolicy::MinimumExpanding);
        let placed = placed(&layout, Rect::new(0, 0, 100, 20));
        assert_eq!(rect_of(&placed, 1).width, 10);
        assert_eq!(rect_of(&placed, 2).width, 90);
    }

    // ── LAY-002: `Preferred` hugs its constraints, `Expanding` distributes the leftover ──

    /// A `Preferred` item keeps exactly the extent its constraints state while an
    /// `Expanding` sibling absorbs the whole leftover. Before the divergence both policies
    /// took the same stretch-weighted share, so the two strategies were synonyms.
    #[test]
    fn a_preferred_item_hugs_its_constraint_while_an_expanding_sibling_takes_the_leftover() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_size_policy(1, SizePolicy::Preferred);
        layout.set_constraints(1, LayoutConstraints::new(40, None));
        let placed = placed(&layout, Rect::new(0, 0, 300, 50));
        assert_eq!(rect_of(&placed, 1), Rect::new(0, 0, 40, 50), "Preferred hugs its hint");
        assert_eq!(rect_of(&placed, 2), Rect::new(40, 0, 260, 50), "Expanding takes the leftover");
    }

    /// A lone `Preferred` item must not be stretched toward the container even when there
    /// is nothing else to take the room — the same rule a `Fixed` item already followed.
    #[test]
    fn a_preferred_item_hugs_even_when_it_is_the_only_item() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.set_size_policy(1, SizePolicy::Preferred);
        layout.set_constraints(1, LayoutConstraints::new(40, None));
        let placed = placed(&layout, Rect::new(0, 0, 300, 50));
        assert_eq!(rect_of(&placed, 1), Rect::new(0, 0, 40, 50));
    }

    /// Hugging is not immunity to the shortfall pass: when the minima together exceed the
    /// parent, a `Preferred` item scales down proportionally like any other.
    #[test]
    fn a_preferred_item_is_scaled_down_when_the_parent_cannot_pay_the_floors() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_size_policy(1, SizePolicy::Preferred);
        layout.set_size_policy(2, SizePolicy::Preferred);
        layout.set_constraints(1, LayoutConstraints::new(100, None));
        layout.set_constraints(2, LayoutConstraints::new(100, None));
        let placed = placed(&layout, Rect::new(0, 0, 150, 50));
        assert_eq!(rect_of(&placed, 1).width, 75);
        assert_eq!(rect_of(&placed, 2).width, 75);
    }

    /// The pre-existing default (`Expanding` on every insertion path) must produce exactly
    /// the geometry it always did — this pins the byte-identical default path required by
    /// the LAY-002 divergence.
    #[test]
    fn the_default_policy_path_output_is_unchanged() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 10, 5);
        layout.add_widget(1, 1);
        layout.add_widget(2, 2);
        layout.add_widget(3, 1);
        // 300 - 2*5 margins - 2*10 gaps = 270 to split by stretch 1:2:1 → 67/135/67,
        // and the one-pixel rounding remainder goes to the first item with room, exactly
        // as the allocator always handed it out.
        let placed = placed(&layout, Rect::new(0, 0, 300, 40));
        assert_eq!(rect_of(&placed, 1), Rect::new(5, 5, 68, 30));
        assert_eq!(rect_of(&placed, 2), Rect::new(83, 5, 135, 30));
        assert_eq!(rect_of(&placed, 3), Rect::new(228, 5, 67, 30));
    }

    /// LAY-003: `arrange_with_context` consumes the context — a 2x device scale doubles the
    /// gaps and the margins on the hint-aware path, while the nominal `arrange` stays put.
    #[test]
    fn arrange_with_context_scales_the_gaps_and_margins() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 10, 5);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(50, 20)),
            ChildInfo::new(2, crate::layout::Hints::fixed(50, 20)),
        ];
        let rect = Rect::new(0, 0, 200, 100);
        let mut nominal = Vec::new();
        layout.arrange(rect, &children, &mut |id, r| nominal.push((id, r)));
        assert_eq!(rect_of(&nominal, 1), Rect::new(5, 5, 50, 90));
        assert_eq!(rect_of(&nominal, 2), Rect::new(65, 5, 50, 90));

        let context =
            LayoutContext { layout_scale: 2.0, font_scale: 1.0, min_touch_size: Size::new(0, 0) };
        let mut scaled = Vec::new();
        layout.arrange_with_context(rect, &children, &context, &mut |id, r| scaled.push((id, r)));
        assert_eq!(rect_of(&scaled, 1), Rect::new(10, 10, 50, 80));
        assert_eq!(rect_of(&scaled, 2), Rect::new(80, 10, 50, 80), "the gap doubled to 20");
    }

    // ── LAY-004: baseline alignment ──

    /// Baselines align at one shared cross coordinate — the tallest baseline decides the
    /// origin, so the shorter item is pushed down by exactly the difference and the two
    /// supplied baselines coincide. Both entry points agree.
    #[test]
    fn baseline_alignment_aligns_the_supplied_baselines() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        assert!(layout.set_item_alignment(1, Some(Alignment::Baseline)));
        assert!(layout.set_item_alignment(2, Some(Alignment::Baseline)));
        assert!(layout.set_item_cross_size(1, Some(40)));
        assert!(layout.set_item_cross_size(2, Some(70)));
        assert!(layout.set_item_baseline(1, Some(10)));
        assert!(layout.set_item_baseline(2, Some(30)));
        assert_eq!(layout.item_baseline(1), Some(10));

        let rect = Rect::new(0, 0, 200, 100);
        let mut placed = Vec::new();
        layout.update(rect, &mut |id, r| placed.push((id, r)));
        assert_eq!(rect_of(&placed, 1), Rect::new(0, 20, 100, 40));
        assert_eq!(rect_of(&placed, 2), Rect::new(100, 0, 100, 70));
        let a = rect_of(&placed, 1);
        let b = rect_of(&placed, 2);
        assert_eq!(a.y + 10, b.y + 30, "the two supplied baselines coincide");

        // The hint-aware path agrees when the caller supplies the same sizes as hints.
        let children = [
            ChildInfo::new(1, crate::layout::Hints::fixed(50, 40)),
            ChildInfo::new(2, crate::layout::Hints::fixed(50, 70)),
        ];
        let mut arranged = Vec::new();
        layout.arrange(rect, &children, &mut |id, r| arranged.push((id, r)));
        let a = rect_of(&arranged, 1);
        let b = rect_of(&arranged, 2);
        assert_eq!(a.y + 10, b.y + 30, "the hint-aware path aligns the same baselines");
    }

    /// With no baseline measurements, a `Baseline` row aligns bottoms — the same default the
    /// flex row applies to items with no ascent information.
    #[test]
    fn baseline_without_measurements_aligns_bottoms() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        assert!(layout.set_item_alignment(1, Some(Alignment::Baseline)));
        assert!(layout.set_item_alignment(2, Some(Alignment::Baseline)));
        assert!(layout.set_item_cross_size(1, Some(40)));
        assert!(layout.set_item_cross_size(2, Some(70)));
        let placed = placed(&layout, Rect::new(0, 0, 200, 100));
        assert_eq!(rect_of(&placed, 1), Rect::new(0, 30, 100, 40));
        assert_eq!(rect_of(&placed, 2), Rect::new(100, 0, 100, 70));
        let a = rect_of(&placed, 1);
        let b = rect_of(&placed, 2);
        assert_eq!(a.y + a.height as i32, b.y + b.height as i32, "bottoms coincide");
    }

    /// A vertical box has no baselines across its horizontal cross axis, so `Baseline` keeps
    /// the start alignment — the flex column's reading.
    #[test]
    fn a_vertical_box_treats_baseline_as_the_start() {
        let mut layout = BoxLayout::new(Orientation::Vertical, 0, 0);
        layout.add_widget(1, 1);
        assert!(layout.set_item_alignment(1, Some(Alignment::Baseline)));
        assert!(layout.set_item_cross_size(1, Some(40)));
        let placed = placed(&layout, Rect::new(0, 0, 100, 200));
        assert_eq!(rect_of(&placed, 1), Rect::new(0, 0, 40, 200));
    }

    /// A `Baseline` item the entry cannot size (no explicit cross size on the update paths)
    /// keeps the band and the start alignment — the documented fallback, not a guess.
    #[test]
    fn a_baseline_item_without_a_cross_size_keeps_the_band() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        assert!(layout.set_item_alignment(1, Some(Alignment::Baseline)));
        let placed = placed(&layout, Rect::new(0, 0, 200, 100));
        assert_eq!(rect_of(&placed, 1), Rect::new(0, 0, 200, 100));
    }
}
