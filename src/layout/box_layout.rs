// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Box layout manager — arranges items in a single row or column.
use super::{Layout, LayoutConstraints, LayoutContext, Orientation, SizePolicy};
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect};
#[derive(Debug)]
struct BoxLayoutItem {
    widget_id: Option<ObjectId>,
    stretch: u32,
    constraints: LayoutConstraints,
    policy: SizePolicy,
}
/// Linear layout that arranges items in one direction.
#[derive(Debug)]
pub struct BoxLayout {
    orientation: Orientation,
    spacing: u32,
    margin: u32,
    items: Vec<BoxLayoutItem>,
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

        let mut total_assigned: u32 = assigned.iter().sum();
        while total_assigned < primary {
            let mut grew = false;
            for (index, item) in self.items.iter().enumerate() {
                if total_assigned >= primary {
                    break;
                }
                // A `Fixed` item has already taken its exact constrained size and must not
                // absorb leftover room.
                if is_fixed(index) {
                    continue;
                }
                let max_allowed =
                    item.constraints.max.unwrap_or(u32::MAX).max(item.constraints.min);
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
            for (index, _item) in self.items.iter().enumerate().rev() {
                if total_assigned <= primary {
                    break;
                }
                let min_allowed = self.items[index].constraints.min;
                if assigned[index] > min_allowed {
                    assigned[index] = assigned[index].saturating_sub(1);
                    total_assigned = total_assigned.saturating_sub(1);
                    shrank = true;
                }
            }
            if !shrank {
                // Nothing is above its minimum and the total is still too large, which
                // can now only happen if a `max` below the summed minima was pinned above
                // its own minimum. Reducing from the largest allocation keeps the sum
                // inside `primary` instead of returning an overflowing vector.
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
        let scaled_margin = (self.margin as f32 * scale).round() as u32;
        let gaps = (self.items.len().saturating_sub(1)) as u32;
        let primary = match self.orientation {
            Orientation::Horizontal => rect.width,
            Orientation::Vertical => rect.height,
        }
        .saturating_sub(scaled_margin * 2)
        .saturating_sub(gaps * scaled_spacing);
        let majors = self.allocate_major_lengths(primary);
        let mut cursor_x = rect.x + scaled_margin as i32;
        let mut cursor_y = rect.y + scaled_margin as i32;
        for (index, item) in self.items.iter().enumerate() {
            let major = majors.get(index).copied().unwrap_or(0);
            let child_rect = match self.orientation {
                Orientation::Horizontal => Rect::new(
                    cursor_x,
                    cursor_y,
                    major,
                    rect.height.saturating_sub(scaled_margin * 2),
                ),
                Orientation::Vertical => Rect::new(
                    cursor_x,
                    cursor_y,
                    rect.width.saturating_sub(scaled_margin * 2),
                    major,
                ),
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
                Orientation::Horizontal => Rect::new(
                    cursor_x,
                    cursor_y,
                    major,
                    rect.height.saturating_sub(self.margin * 2),
                ),
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
}
