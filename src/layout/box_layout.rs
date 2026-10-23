// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Box layout manager — arranges items in a single row or column.
use super::{ChildInfo, Layout, LayoutConstraints, LayoutContext, Orientation, SizePolicy};
use crate::compat::{Any, Vec};
use crate::core::Alignment;
use crate::core::{ObjectId, Rect};
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
    /// layout has always done and what a CSS/Qt box does by default. A value shrinks the
    /// item to the cross size it *asks for* (from [`ChildInfo::hints`]) and places it
    /// within the cross extent — the same split Flutter's `crossAxisAlignment` makes.
    align: Option<Alignment>,
    /// The cross-axis size used when `align` is set and the hint-aware path has no child info.
    ///
    /// `update_with_context` does not receive [`ChildInfo`] (its signature is the trait's), so an
    /// aligned item used to be stretched there while `arrange` honoured the hint — the same layout
    /// answered differently through its two entry points. This is the explicit cross size the
    /// caller can supply for that entry point; `arrange` prefers it over the hint so both agree.
    cross_size: Option<u32>,
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
        });
    }

    /// Adds an empty spacer that is exactly `size` pixels on the major axis.
    ///
    /// [`Self::add_spacer`] is elastic by construction (`stretch.max(1)` with an
    /// `Expanding` policy), so it cannot express Qt's `QSpacerItem(w, h, Fixed)` or wx's
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
            let mut major = if item.policy == SizePolicy::Fixed {
                item.constraints.max.unwrap_or(item.constraints.min)
            } else {
                (primary as u64 * item.stretch as u64 / total_stretch) as u32
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
        // was filled to 100. A `Fixed` item may still be *scaled down* in the shortfall pass
        // below, which is why only this pass skips it.
        let is_fixed = |index: usize| self.items[index].policy == SizePolicy::Fixed;

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
            if is_fixed(index) {
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
                    if is_fixed(index) {
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
                        if is_fixed(index) {
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
                // Every item is now at its own minimum and the total is still too large — which can
                // only happen if a `max` below the summed minima pinned an item above its minimum.
                // Reducing from the largest allocation restores invariant 1 without an unbounded
                // loop; the `min` floor is the only thing that had to yield here already.
                while total_assigned > primary_u64 {
                    let Some((largest_index, _)) = assigned
                        .iter()
                        .enumerate()
                        .filter(|(_, value)| **value > 0)
                        .max_by_key(|(_, value)| **value)
                    else {
                        break;
                    };
                    assigned[largest_index] = assigned[largest_index].saturating_sub(1);
                    total_assigned = total_assigned.saturating_sub(1);
                }
            }
        }
        assigned
    }
}
impl Layout for BoxLayout {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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
        if self.items.is_empty() {
            return;
        }
        let (left, top, right, bottom) = self.scaled_insets(1.0);
        let gaps = (self.items.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width.saturating_sub(left).saturating_sub(right),
            Orientation::Vertical => rect.height.saturating_sub(top).saturating_sub(bottom),
        }
        .saturating_sub(gaps * self.spacing);
        let cross_extent = match self.orientation {
            Orientation::Horizontal => rect.height.saturating_sub(top).saturating_sub(bottom),
            Orientation::Vertical => rect.width.saturating_sub(left).saturating_sub(right),
        };
        let majors = self.allocate_major_lengths(primary);
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
                Orientation::Horizontal => cursor_x += (major + self.spacing) as i32,
                Orientation::Vertical => cursor_y += (major + self.spacing) as i32,
            }
        }
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
}
