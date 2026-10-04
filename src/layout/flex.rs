// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Flex layout manager — CSS Flexbox-style layout with grow, shrink, and alignment.
use super::{Layout, LayoutContext};
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect, Size};
use crate::layout::hints::ChildInfo;

/// Main-axis direction for flex layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    /// Items placed left-to-right.
    #[default]
    Row,
    /// Items placed right-to-left.
    RowReverse,
    /// Items placed top-to-bottom.
    Column,
    /// Items placed bottom-to-top.
    ColumnReverse,
}

/// Wrapping behaviour when items overflow the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    /// No wrapping; items may overflow.
    #[default]
    NoWrap,
    /// Wrap to next line/column.
    Wrap,
    /// Wrap in reverse direction.
    WrapReverse,
}

/// How items are distributed along the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    /// Pack items at the start.
    #[default]
    FlexStart,
    /// Pack items at the end.
    FlexEnd,
    /// Pack items in the centre.
    Center,
    /// Distribute with equal space between items.
    SpaceBetween,
    /// Distribute with equal space around each item.
    SpaceAround,
    /// Distribute with equal space between items and edges.
    SpaceEvenly,
}

/// How items are aligned along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    /// Stretch items to fill the cross axis.
    #[default]
    Stretch,
    /// Align to start of cross axis.
    FlexStart,
    /// Align to end of cross axis.
    FlexEnd,
    /// Align to centre of cross axis.
    Center,
    /// Align baselines (treated as FlexStart for now).
    Baseline,
}

/// A single item managed by the flex layout.
#[derive(Debug, Clone)]
pub struct FlexItem {
    /// Widget identifier, if any (None = spacer).
    pub widget_id: Option<ObjectId>,
    /// Proportion of remaining space this item claims.
    pub flex_grow: f32,
    /// Rate at which this item shrinks when space is tight.
    pub flex_shrink: f32,
    /// Per-item cross-axis override.
    pub align_self: Option<AlignItems>,
    /// Minimum size constraint.
    pub min_size: Size,
    /// Maximum size constraint (0 = no limit).
    pub max_size: Size,
}

impl Default for FlexItem {
    fn default() -> Self {
        Self {
            widget_id: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            align_self: None,
            min_size: Size::new(0, 0),
            max_size: Size::new(0, 0),
        }
    }
}

/// CSS Flexbox-style layout manager.
#[derive(Debug, Clone)]
pub struct FlexLayout {
    /// Main-axis direction.
    pub direction: FlexDirection,
    /// Wrapping behaviour.
    pub wrap: FlexWrap,
    /// Main-axis distribution.
    pub justify_content: JustifyContent,
    /// Cross-axis alignment.
    pub align_items: AlignItems,
    /// Gap between items in pixels.
    pub gap: i32,
    /// Outer padding in pixels.
    pub padding: i32,
    /// Managed items.
    items: Vec<FlexItem>,
    /// Size hints indexed by position within items (set before update).
    child_sizes: Vec<Size>,
}

impl FlexLayout {
    /// Create a flex layout with default settings.
    pub fn new() -> Self {
        Self {
            direction: FlexDirection::default(),
            wrap: FlexWrap::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            gap: 0,
            padding: 0,
            items: Vec::new(),
            child_sizes: Vec::new(),
        }
    }

    /// Create a flex layout with all parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn with_params(
        direction: FlexDirection,
        wrap: FlexWrap,
        justify_content: JustifyContent,
        align_items: AlignItems,
        gap: i32,
        padding: i32,
    ) -> Self {
        Self {
            direction,
            wrap,
            justify_content,
            align_items,
            gap,
            // S-47: negative padding is undefined, so reject it at construction.
            padding: padding.max(0),
            items: Vec::new(),
            child_sizes: Vec::new(),
        }
    }

    /// Returns a reference to the items vector.
    pub fn items(&self) -> &[FlexItem] {
        &self.items
    }

    /// Returns a mutable reference to the items vector.
    pub fn items_mut(&mut self) -> &mut Vec<FlexItem> {
        &mut self.items
    }

    /// Set the child size hints (call before update for proper sizing).
    pub fn set_child_sizes(&mut self, sizes: Vec<Size>) {
        self.child_sizes = sizes;
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    fn is_row(&self) -> bool {
        matches!(self.direction, FlexDirection::Row | FlexDirection::RowReverse)
    }

    fn is_reverse(&self) -> bool {
        matches!(self.direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse)
    }

    /// Compute resolved sizes for items and return (main_sizes, total_flex_grow, total_main).
    fn compute_main_sizes(&self, available_main: i32, gap: i32) -> (Vec<i32>, f32, i32) {
        let count = self.items.len();
        if count == 0 {
            return (Vec::new(), 0.0, 0);
        }

        // The main-axis bounds of an item, resolved once so every branch shares them.
        // `min` is applied before `max`, so an item whose minimum exceeds its maximum keeps
        // the maximum — the same resolution the grow branch has always used.
        let main_max = |item: &FlexItem| -> i32 {
            let max = if self.is_row() { item.max_size.width } else { item.max_size.height };
            if max > 0 {
                max as i32
            } else {
                i32::MAX
            }
        };
        let main_min = |item: &FlexItem| -> i32 {
            if self.is_row() {
                item.min_size.width as i32
            } else {
                item.min_size.height as i32
            }
        };

        // Sum child intrinsic sizes, floored at the minimum and capped at the maximum. The cap
        // used to be applied only inside the grow branch, so a non-growing child whose intrinsic
        // exceeded its max was laid out past its own ceiling (S-50).
        let mut intrinsic_main: Vec<i32> = Vec::with_capacity(count);
        let mut total_flex_grow: f32 = 0.0;

        for (i, item) in self.items.iter().enumerate() {
            let sz = self.child_sizes.get(i).copied().unwrap_or(Size::new(0, 0));
            let main = if self.is_row() { sz.width as i32 } else { sz.height as i32 };
            intrinsic_main.push(main.max(main_min(item)).min(main_max(item)));
            total_flex_grow += item.flex_grow;
        }

        let gaps = (count.saturating_sub(1)) as i32 * gap;
        let remaining = available_main - intrinsic_main.iter().sum::<i32>() - gaps;

        let mut main_sizes: Vec<i32> = Vec::with_capacity(count);

        if remaining > 0 && total_flex_grow > 0.0 {
            // Distribute the surplus among growable items, never past an item's max. When an
            // item's max refuses part of the surplus, that room is offered to the *other*
            // growable items instead of being dumped onto the last one — which is what let a
            // 20px-capped item grow to 80px while its sibling also refused the room (S-50).
            // Any room nobody can absorb is left for `justify_positions` to distribute.
            main_sizes = intrinsic_main.clone();
            let mut leftover = remaining;
            let mut progress = true;
            while leftover > 0 && progress {
                progress = false;
                let room_grow: f32 = self
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(i, item)| item.flex_grow > 0.0 && main_sizes[*i] < main_max(item))
                    .map(|(_, item)| item.flex_grow)
                    .sum();
                if room_grow <= 0.0 {
                    break;
                }
                // Distribute this pass's leftover against a fixed budget. Decrementing
                // `leftover` in place made each later item's share smaller than the one
                // before it, favouring the leading items; a fixed `pass_leftover` keeps the
                // shares proportional and only the integer remainder is shared out at the end.
                let pass_leftover = leftover;
                let mut allocated = 0i32;
                for (i, item) in self.items.iter().enumerate() {
                    if item.flex_grow <= 0.0 {
                        continue;
                    }
                    let room = main_max(item) - main_sizes[i];
                    if room <= 0 {
                        continue;
                    }
                    let share =
                        ((pass_leftover as f32) * (item.flex_grow / room_grow)).round() as i32;
                    let take = share.min(room).min(pass_leftover - allocated);
                    if take > 0 {
                        main_sizes[i] += take;
                        allocated += take;
                        progress = true;
                    }
                }
                leftover -= allocated;
                // Integer rounding can leave a remainder smaller than the number of givers; hand
                // it to the first growable item that still has room so the loop cannot spin.
                if leftover > 0 && progress {
                    if let Some(i) = (0..count).find(|&i| {
                        self.items[i].flex_grow > 0.0 && main_sizes[i] < main_max(&self.items[i])
                    }) {
                        let take = leftover.min(main_max(&self.items[i]) - main_sizes[i]);
                        main_sizes[i] += take;
                        leftover -= take;
                    }
                }
            }
        } else if remaining < 0 {
            // Shrink items proportionally to flex-shrink, then spend whatever room the floors
            // would not give up by shrinking the children that *can* still give.
            //
            // # Why one pass is not enough
            //
            // The proportional pass alone leaves the row wider than its band whenever a child's
            // floor is above its proportional share, and the overhang is not distributed — it is
            // simply left after the last child, because the positions are packed from the leading
            // edge. A row of a 110 px label and a 22 px column in a 48 px band therefore came back
            // as `x = 0, width = 110` and `x = 110, width = 22`: the second column was painted at
            // x = 110, i.e. 62 px outside the control it belongs to, and the SVG backend emits
            // absolute coordinates, so it left the picture entirely. A sub-part outside its own
            // control is a worse outcome than a compressed one: it is invisible, and the failure is
            // silent.
            //
            // #6 rule 9 already says the child declares its floor, so the second pass may not
            // cross one; the children that *can* shrink are the ones whose floor is still below
            // their size. If the floors between them leave nothing to give, the row genuinely does
            // not fit and the children stay at their floors — the same refusal the comment below
            // describes. What changes here is only that the room a floor *does* release is used,
            // so "the smallest the children may be" is a reachable layout rather than an
            // aspiration the packing pass never consults.
            let deficit = -remaining;
            let total_flex_shrink: f32 = self.items.iter().map(|i| i.flex_shrink).sum();
            let mut floors: Vec<i32> = Vec::with_capacity(count);
            for (i, item) in self.items.iter().enumerate() {
                let shrink = if total_flex_shrink > 0.0 {
                    ((deficit as f32) * (item.flex_shrink / total_flex_shrink)).round() as i32
                } else {
                    deficit / count as i32
                };
                let min_main = if self.is_row() {
                    item.min_size.width as i32
                } else {
                    item.min_size.height as i32
                };
                let size = (intrinsic_main[i] - shrink).max(min_main);
                main_sizes.push(size);
                floors.push(min_main);
            }
            let mut leftover = -(available_main - gaps - main_sizes.iter().sum::<i32>());
            // Repeated rounds rather than one, because releasing one child's floor can itself
            // expose another's: a child that was already at its floor in the first pass is
            // untouched here, but the *others* can now give more than they did when the deficit was
            // shared between all of them.
            let mut progress = true;
            while leftover > 0 && progress {
                progress = false;
                let give: Vec<i32> = main_sizes
                    .iter()
                    .zip(floors.iter())
                    .map(|(size, floor)| (size - floor).max(0))
                    .collect();
                let total_give: i32 = give.iter().sum();
                if total_give <= 0 {
                    break;
                }
                for (index, room) in give.iter().enumerate() {
                    if *room <= 0 {
                        continue;
                    }
                    let share = ((leftover as i64 * *room as i64) / total_give as i64) as i32;
                    let take = share.min(*room).min(leftover);
                    main_sizes[index] -= take;
                    leftover -= take;
                    if take > 0 {
                        progress = true;
                    }
                }
                // The integer division above always rounds *down*, for every child, so a deficit
                // smaller than the number of givers releases nothing at all and the loop would spin
                // on `leftover` forever. The last giver whose floor allows it takes the remainder.
                if leftover > 0 && progress {
                    if let Some(index) = (0..main_sizes.len())
                        .rev()
                        .find(|index| main_sizes[*index] > floors[*index])
                    {
                        let take = leftover.min(main_sizes[index] - floors[index]);
                        main_sizes[index] -= take;
                        leftover -= take;
                    }
                }
            }
            // # Why the deficit is *not* pushed past the minimum
            //
            // This block used to keep cutting the children until the row fit — "if we couldn't
            // shrink enough, cap at available" — which defeated the `max(min_main)` above it: the
            // floor was applied and then immediately overridden, down to zero. Two 100 px buttons in
            // a 120 px row came back 57 px each, i.e. narrower than their own labels, and a button
            // narrower than its label is a button whose label elides.
            //
            // The floor is a statement about what the *child* can survive, and a layout that
            // ignores it to satisfy its own extent trades a visible overflow for an unreadable
            // control. CSS flexbox makes the same choice (`min-width: auto` item floors win over
            // `flex-shrink`), and the shared `implicitMinimumWidth` is likewise a hard bound.
            //
            // # What happens when even the floors do not fit (the G-1 resolution)
            //
            // The floors are a *declaration*, and a declaration can be unsatisfiable: two 100 px
            // buttons cannot be laid out in a 120 px band. Returning them at 100 px each is the
            // right refusal — the alternative is a 20 px button — but it left the **positions** to
            // be packed from the leading edge, so the 80 px that did not fit was paid entirely by
            // the trailing child: it was painted at `x = 100` in a 120 px band, i.e. 80 px past its
            // own control's edge. Because the SVG backend emits absolute coordinates and nothing
            // clips at this layer, such a child is not "overflowing", it is **absent** from the
            // picture — a silent loss, which is strictly worse than a squeezed control.
            //
            // So the sizes stay, and the *overhang is shared*: every child is scaled by
            // `available / floors` so the run as a whole fits, which keeps each child inside the
            // band and keeps their relative proportions (the tallest child stays the tallest). It
            // is deliberately the smallest possible deviation from the floors — in the common case
            // where the floors *do* fit, this branch is not reached at all — and it trades an
            // invisible child for every child being uniformly narrower than its own declared floor.
            //
            // # Why scaling beats capping the last child
            //
            // The obvious repair is "give each child at most the room that is left". It was
            // implemented and **reverted**: a row of two 100 px buttons in a 120 px band came back
            // `100 + 20`, so the *second* button was drawn 20 px wide — still below its floor, now
            // with the loss concentrated in one child instead of spread over two. It does not
            // resolve the contradiction (nothing can), it only moves which child pays for it,
            // and it does so unevenly. Scaling is the same contradiction acknowledged honestly.
            let floors_sum: i32 = floors.iter().sum();
            if leftover > 0 && floors_sum > 0 {
                // Integer arithmetic throughout, with the remainder distributed one pixel at a
                // time from the leading edge, so the scaled sizes sum to *exactly* `main_sizes`'
                // total rather than to a rounded approximation that could re-introduce an overhang.
                let budget: i32 = main_sizes.iter().sum::<i32>() - leftover;
                let scaled: Vec<i32> = main_sizes
                    .iter()
                    .map(|size| ((*size as i64 * budget as i64) / floors_sum as i64) as i32)
                    .collect();
                let mut short = budget - scaled.iter().sum::<i32>();
                main_sizes = scaled;
                for size in main_sizes.iter_mut() {
                    if short <= 0 {
                        break;
                    }
                    *size += 1;
                    short -= 1;
                }
            }
        } else {
            main_sizes = intrinsic_main;
        }

        let total_main: i32 = main_sizes.iter().sum::<i32>() + gaps;
        (main_sizes, total_flex_grow, total_main)
    }

    /// Apply main-axis justification and produce positions.
    fn justify_positions(
        &self,
        main_sizes: &[i32],
        total_used: i32,
        available_main: i32,
        start_main: i32,
        gap: i32,
    ) -> Vec<i32> {
        let count = main_sizes.len();
        if count == 0 {
            return Vec::new();
        }

        let leftover = available_main - total_used;
        let sizes_sum: i32 = main_sizes.iter().sum();
        let mut positions = Vec::with_capacity(count);

        // The base gap is already subtracted inside `total_used`, so the space-* alignments
        // must distribute `leftover` **on top of** the declared gap rather than in place of it.
        // Replacing `gap` with `leftover / n` discarded the gap entirely: a two-item row with a
        // 10px gap never reached its trailing edge, and a row whose growth consumed everything
        // (leftover 0) lost its gap too (S-51).
        let (first_gap, inter_gap) = match self.justify_content {
            JustifyContent::FlexStart | JustifyContent::FlexEnd | JustifyContent::Center => {
                let offset = match self.justify_content {
                    JustifyContent::FlexStart => 0,
                    JustifyContent::FlexEnd => leftover,
                    JustifyContent::Center => leftover / 2,
                    _ => 0,
                };
                (offset, gap)
            }
            JustifyContent::SpaceBetween => {
                let inter = if count > 1 { gap + leftover / (count as i32 - 1) } else { gap };
                (0, inter)
            }
            JustifyContent::SpaceAround => {
                let inter = if count > 0 { gap + leftover / count as i32 } else { gap };
                let painted = sizes_sum + (count as i32 - 1) * inter;
                let lead = ((available_main - painted) / 2).max(0);
                (lead, inter)
            }
            JustifyContent::SpaceEvenly => {
                let edge = if count > 0 { leftover / (count as i32 + 1) } else { 0 };
                (edge, gap + edge)
            }
        };

        let end = start_main + available_main;

        if self.is_reverse() {
            // Reverse direction: pack from the end (right/bottom) towards start.
            let mut cursor = end - first_gap;
            for &size in main_sizes[..count].iter() {
                let pos = cursor - size;
                positions.push(pos);
                cursor = pos - inter_gap;
            }
        } else {
            let mut cursor = start_main + first_gap;
            for &size in main_sizes[..count].iter() {
                positions.push(cursor);
                cursor += size + inter_gap;
            }
        }

        positions
    }

    /// Compute cross-axis sizes and positions for each item.
    fn compute_cross_positions(&self, main_sizes: &[i32], cross_size: i32) -> Vec<(i32, i32)> {
        let count = main_sizes.len();
        if count == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(count);
        for (i, item) in self.items.iter().enumerate() {
            let sz = self.child_sizes.get(i).copied().unwrap_or(Size::new(0, 0));
            let child_cross = if self.is_row() { sz.height as i32 } else { sz.width as i32 };

            // S-53: the cross-axis bounds of the `FlexItem` were never applied. Every branch
            // must honour them: `Stretch` is capped at the maximum, `FlexStart`/`Center`/
            // `FlexEnd`/`Baseline` floor the child at its minimum and cap it at its maximum.
            let min_cross = if self.is_row() { item.min_size.height } else { item.min_size.width };
            let max_cross = if self.is_row() { item.max_size.height } else { item.max_size.width };
            let clamp_cross = |v: i32| -> i32 {
                let v = v.max(min_cross as i32);
                if max_cross > 0 {
                    v.min(max_cross as i32)
                } else {
                    v
                }
            };
            let child_cross = clamp_cross(child_cross);

            let align = item.align_self.unwrap_or(self.align_items);

            let (cross_start, cross_len) = match align {
                AlignItems::Stretch => (0, clamp_cross(cross_size)),
                AlignItems::FlexStart => (0, child_cross),
                AlignItems::FlexEnd => (cross_size - child_cross, child_cross),
                AlignItems::Center => ((cross_size - child_cross) / 2, child_cross),
                AlignItems::Baseline => (0, child_cross),
            };

            result.push((cross_start, cross_len));
        }

        result
    }

    /// Compute the rects for all items within the content area.
    /// `scaled_gap` overrides `self.gap` when Some (used for HiDPI/context-aware scaling).
    fn compute_rects(
        &self,
        content_rect: Rect,
        scaled_gap: Option<i32>,
    ) -> Vec<(Option<ObjectId>, Rect)> {
        if self.items.is_empty() {
            return Vec::new();
        }

        let gap = scaled_gap.unwrap_or(self.gap);

        if !matches!(self.wrap, FlexWrap::NoWrap) {
            return self.compute_wrapped_rects(content_rect, gap);
        }

        let (available_main, start_main, available_cross, cross_origin) = if self.is_row() {
            (content_rect.width as i32, content_rect.x, content_rect.height as i32, content_rect.y)
        } else {
            (content_rect.height as i32, content_rect.y, content_rect.width as i32, content_rect.x)
        };

        if available_main <= 0 || available_cross <= 0 {
            return Vec::new();
        }

        let (main_sizes, _total_flex_grow, total_used) =
            self.compute_main_sizes(available_main, gap);

        let main_positions =
            self.justify_positions(&main_sizes, total_used, available_main, start_main, gap);
        let cross_positions = self.compute_cross_positions(&main_sizes, available_cross);

        let mut results = Vec::with_capacity(self.items.len());

        for (i, item) in self.items.iter().enumerate() {
            let main_pos = *main_positions.get(i).unwrap_or(&0);
            let (cross_pos, cross_len) =
                cross_positions.get(i).copied().unwrap_or((0, available_cross));
            let main_len = *main_sizes.get(i).unwrap_or(&0);

            let rect = if self.is_row() {
                Rect::new(main_pos, cross_origin + cross_pos, main_len as u32, cross_len as u32)
            } else {
                Rect::new(cross_origin + cross_pos, main_pos, cross_len as u32, main_len as u32)
            };

            results.push((item.widget_id, rect));
        }

        results
    }

    fn compute_wrapped_rects(&self, content_rect: Rect, gap: i32) -> Vec<(Option<ObjectId>, Rect)> {
        let is_row = self.is_row();
        let available_main =
            if is_row { content_rect.width as i32 } else { content_rect.height as i32 };
        let available_cross =
            if is_row { content_rect.height as i32 } else { content_rect.width as i32 };
        if available_main <= 0 || available_cross <= 0 {
            return Vec::new();
        }

        let item_main = |index: usize| {
            let size = self.child_sizes.get(index).copied().unwrap_or(Size::new(0, 0));
            let intrinsic = if is_row { size.width } else { size.height };
            let minimum = if is_row {
                self.items[index].min_size.width
            } else {
                self.items[index].min_size.height
            };
            intrinsic.max(minimum) as i32
        };
        let item_cross = |index: usize| {
            let size = self.child_sizes.get(index).copied().unwrap_or(Size::new(0, 0));
            let intrinsic = if is_row { size.height } else { size.width } as i32;
            let minimum = if is_row {
                self.items[index].min_size.height
            } else {
                self.items[index].min_size.width
            } as i32;
            let maximum = if is_row {
                self.items[index].max_size.height
            } else {
                self.items[index].max_size.width
            } as i32;
            // S-53: the wrap path applied only the minimum; the maximum is now honoured too.
            let mut cross = intrinsic.max(minimum);
            if maximum > 0 {
                cross = cross.min(maximum);
            }
            cross
        };

        let mut lines: Vec<Vec<usize>> = Vec::new();
        for index in 0..self.items.len() {
            let candidate = item_main(index);
            let needs_wrap = lines.last().is_some_and(|line| {
                let used = line.iter().map(|&item| item_main(item)).sum::<i32>()
                    + gap * line.len().saturating_sub(1) as i32;
                used > 0 && used + gap + candidate > available_main
            });
            if needs_wrap {
                lines.push(Vec::new());
            }
            if lines.is_empty() {
                lines.push(Vec::new());
            }
            lines
                .last_mut()
                .expect("a line was just pushed above, so the list is non-empty")
                .push(index);
        }

        let mut line_cross_sizes = Vec::with_capacity(lines.len());
        for line in &lines {
            line_cross_sizes.push(line.iter().map(|&index| item_cross(index)).max().unwrap_or(0));
        }
        let cross_origin = if is_row { content_rect.y } else { content_rect.x };
        let mut results = Vec::with_capacity(self.items.len());
        let mut cross_cursor = if self.wrap == FlexWrap::WrapReverse {
            cross_origin + available_cross
        } else {
            cross_origin
        };

        for (line_index, line) in lines.iter().enumerate() {
            let intrinsic_total = line.iter().map(|&index| item_main(index)).sum::<i32>();
            let gaps = gap * line.len().saturating_sub(1) as i32;
            let remaining = available_main - intrinsic_total - gaps;
            let total_grow =
                line.iter().map(|&index| self.items[index].flex_grow.max(0.0)).sum::<f32>();
            let mut sizes: Vec<i32> = line.iter().map(|&index| item_main(index)).collect();
            if remaining > 0 && total_grow > 0.0 {
                for (slot, &index) in line.iter().enumerate() {
                    let extra = (remaining as f32 * self.items[index].flex_grow / total_grow)
                        .round() as i32;
                    let max_main = if is_row {
                        self.items[index].max_size.width
                    } else {
                        self.items[index].max_size.height
                    };
                    sizes[slot] = if max_main > 0 {
                        (sizes[slot] + extra).min(max_main as i32)
                    } else {
                        sizes[slot] + extra
                    };
                }
            }
            let used = sizes.iter().sum::<i32>() + gaps;
            let positions = self.justify_positions(
                &sizes,
                used,
                available_main,
                if is_row { content_rect.x } else { content_rect.y },
                gap,
            );
            let line_cross = line_cross_sizes[line_index];
            if self.wrap == FlexWrap::WrapReverse {
                cross_cursor -= line_cross;
            }
            for (slot, &index) in line.iter().enumerate() {
                let align = self.items[index].align_self.unwrap_or(self.align_items);
                let child_cross = item_cross(index);
                // S-53: `Stretch` must respect the item's cross maximum too; the other branches
                // already use the clamped `child_cross`.
                let min_cross = if is_row {
                    self.items[index].min_size.height
                } else {
                    self.items[index].min_size.width
                } as i32;
                let max_cross = if is_row {
                    self.items[index].max_size.height
                } else {
                    self.items[index].max_size.width
                } as i32;
                let clamp_cross = |v: i32| -> i32 {
                    let v = v.max(min_cross);
                    if max_cross > 0 {
                        v.min(max_cross)
                    } else {
                        v
                    }
                };
                let (cross_offset, cross_len) = match align {
                    AlignItems::Stretch => (0, clamp_cross(line_cross)),
                    AlignItems::FlexStart | AlignItems::Baseline => (0, child_cross),
                    AlignItems::FlexEnd => (line_cross - child_cross, child_cross),
                    AlignItems::Center => ((line_cross - child_cross) / 2, child_cross),
                };
                let main_pos = positions[slot];
                let cross_pos = cross_cursor + cross_offset;
                let child_rect = if is_row {
                    Rect::new(
                        main_pos,
                        cross_pos,
                        sizes[slot].max(0) as u32,
                        cross_len.max(0) as u32,
                    )
                } else {
                    Rect::new(
                        cross_pos,
                        main_pos,
                        cross_len.max(0) as u32,
                        sizes[slot].max(0) as u32,
                    )
                };
                results.push((self.items[index].widget_id, child_rect));
            }
            if self.wrap == FlexWrap::WrapReverse {
                cross_cursor -= gap;
            } else {
                cross_cursor += line_cross + gap;
            }
        }
        results
    }
}

crate::impl_default_via_new!(FlexLayout);

impl Layout for FlexLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.items.push(FlexItem {
            widget_id: Some(widget_id),
            flex_grow: stretch as f32,
            ..FlexItem::default()
        });
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        // S-52: `child_sizes` is index-aligned with `items`, so removing an item must remove its
        // size too. The old `retain` kept only the items, and the remaining children inherited the
        // deleted sibling's size hint. Walking both lists together keeps identity and size in lock
        // step, including when the same id was registered more than once (all matches are removed).
        let mut new_items = Vec::with_capacity(self.items.len());
        let mut new_sizes = Vec::with_capacity(self.child_sizes.len());
        for (index, item) in self.items.iter().enumerate() {
            if item.widget_id == Some(widget_id) {
                continue;
            }
            new_items.push(item.clone());
            if index < self.child_sizes.len() {
                new_sizes.push(self.child_sizes[index]);
            }
        }
        self.items = new_items;
        self.child_sizes = new_sizes;
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.items.iter().filter_map(|item| item.widget_id).collect()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.items.iter().any(|item| item.widget_id == Some(id))
    }

    fn clear(&mut self) {
        self.items.clear();
        self.child_sizes.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        // S-47: `padding` is `pub`, so clamp a directly-mutated negative value here too.
        let padding = self.padding.max(0);
        let content_rect = Rect::new(
            rect.x.saturating_add(padding),
            rect.y.saturating_add(padding),
            rect.width.saturating_sub(2 * padding as u32),
            rect.height.saturating_sub(2 * padding as u32),
        );

        let results = self.compute_rects(content_rect, None);
        for (widget_id, child_rect) in results {
            if let Some(wid) = widget_id {
                widgets(wid, child_rect);
            }
        }
    }

    /// Lays the items out from the children's own hints.
    ///
    /// # What this replaces
    ///
    /// The old path required the caller to call
    /// [`set_child_sizes`](FlexLayout::set_child_sizes) with the children's sizes *before*
    /// asking for a layout — the layout was told the answer instead of asking the
    /// question. Here the sizes arrive with the children, so a caller that has the widgets
    /// (and therefore their hints) needs no second call, and one that does not cannot
    /// silently lay everything out at zero.
    ///
    /// Each child's *preferred* extent is used as the intrinsic size, and its `fill` flag
    /// as the flex-grow weight — which is the same division the shared model draws: `preferredWidth`
    /// says how big it wants to be, `fillWidth` says whether it may absorb the leftover.
    /// A child that declares neither is laid out at its preferred size and no more, which
    /// is the behaviour a caller reading only `size_hint` expects.
    ///
    /// # Why the boxes are placed here rather than by a second packing pass
    ///
    /// The order the children were handed over **is** the order they were built in: a caller
    /// that says `[ok, cancel]` means `ok` then `cancel`, left to right. The hint channel is
    /// the one path a caller can take, so it is also the one path where that intent is known,
    /// and it must not be laundered through a temporary layout that rebuilds its item list and
    /// loses which child was which.
    ///
    /// Margins are paid out of the room a child may occupy, on the child's own sides: its
    /// leading margin separates it from whatever is before it and its trailing margin from
    /// whatever follows. That makes an inter-element gap the caller's own declaration — the
    /// `padding`/`spacing` split — instead of something every call site has to re-derive, and
    /// it is why this path can honour `justify_content` as well: the leftover room is what
    /// remains after the margins, not after the content alone.
    fn arrange_with_context(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        context: &LayoutContext,
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        // A real device context is in hand, so the minimum-touch growth applies.
        self.arrange_body(rect, children, context, true, out)
    }

    fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect)) {
        // No device context, so no touch growth — exactly what [`Layout::arrange`]'s default
        // promises: a layout that has not been taught about the device lays out by its nominal
        // numbers. `the_hint_channel_and_the_legacy_path_agree_on_the_same_sizes` is the test that
        // pins this: `arrange` and `update` must return the same geometry for the same sizes.
        self.arrange_body(rect, children, &LayoutContext::default(), false, out)
    }
}

impl FlexLayout {
    /// The shared body of [`Layout::arrange`] and [`Layout::arrange_with_context`].
    ///
    /// # Why the context is threaded rather than defaulted
    ///
    /// `arrange` reuses `update`'s own solver instead of re-deriving sizes (see the note below),
    /// and `update_with_context` is where `layout_scale`/`font_scale` widen the gaps. A context-free
    /// `arrange` therefore returned nominal gaps where `update_with_context` returned scaled ones —
    /// two answers to one question, on one layout and one rect. One body makes the pair agree by
    /// construction; the solver is told the scale, so the geometry it produces carries it.
    fn arrange_body(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        context: &LayoutContext,
        apply_touch_floor: bool,
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        if self.items.is_empty() {
            return;
        }
        // # Why the scale reaches the solver through the content rect
        //
        // `update_with_context` scales `padding` and `gap` and hands the result to
        // `compute_rects`. Scaling the content box here means this body runs the *same* solver on
        // the *same* input, so the two entry points cannot produce different geometry. The
        // alternative — re-deriving `compute_rects` with a scaled gap — would be a second solver,
        // which is the `set_child_sizes` shape this method's compatibility contract forbids.
        let scale = context.layout_scale.max(context.font_scale);
        // S-47: clamp a negative `padding` before the device-scale cast so it can never reach
        // `2 * padding as u32` below.
        let scaled_padding = (self.padding.max(0) as f32 * scale).max(0.0).round() as i32;
        // Which entry belongs to which item, resolved once so the two are never indexed by
        // position into two lists that could disagree. An item with no widget id (a spacer) or
        // with no `ChildInfo` gets `None` and keeps whatever size the caller last handed in,
        // so a partially-migrated caller does not lose the children it has not converted yet.
        let described: Vec<Option<ChildInfo>> = self
            .items
            .iter()
            .map(|item| item.widget_id.and_then(|id| ChildInfo::find(children, id).copied()))
            .collect();
        // The room each child *wants including its own margins* — the same sum
        // [`FlexLayout::update`] would have been handed through `set_child_sizes`.
        let sizes: Vec<Size> = described
            .iter()
            .enumerate()
            .map(|(index, info)| match info {
                // The stored size already carries the margins this path adds back below, so it
                // is added only for a child the caller described.
                Some(info) => info.bounds(),
                None => self.child_sizes.get(index).copied().unwrap_or(Size::new(0, 0)),
            })
            .collect();

        // The gap between children is scaled with the device, matching `update_with_context`'s
        // reading (the larger of the two scales). Both entry points run through this body, so the
        // reading is stated once.
        let scaled_gap = (self.gap as f32 * scale).round() as i32;

        let content_rect = Rect::new(
            rect.x.saturating_add(scaled_padding),
            rect.y.saturating_add(scaled_padding),
            rect.width.saturating_sub(2 * scaled_padding as u32),
            rect.height.saturating_sub(2 * scaled_padding as u32),
        );
        if content_rect.width == 0 || content_rect.height == 0 {
            return;
        }

        let is_row = self.is_row();
        let (available_main, origin_main, cross_origin, available_cross) = if is_row {
            (content_rect.width as i32, content_rect.x, content_rect.y, content_rect.height as i32)
        } else {
            (content_rect.height as i32, content_rect.y, content_rect.x, content_rect.width as i32)
        };

        // The leftover room is what the justification distributes.
        //
        // The sizes themselves come from `update`'s own solver, **not** from the hints directly.
        // That is deliberate and it is the whole of this method's compatibility contract: a
        // caller that adopts the hint channel must get the geometry it already had, and the
        // solver is where `fill`/`stretch` grow, where `flex_shrink` compresses, and where the
        // minimum is floored. Re-deriving sizes here would make the two entry points two
        // algorithms — which is exactly what the "the channel is not a second layout" test
        // exists to forbid, and how adopting the channel would silently change every existing
        // layout.
        let inset = |index: usize| -> (i32, i32, i32, i32) {
            match described[index] {
                Some(info) => (
                    info.params.margins.left as i32,
                    info.params.margins.top as i32,
                    info.params.margins.right as i32,
                    info.params.margins.bottom as i32,
                ),
                None => (0, 0, 0, 0),
            }
        };
        let outer_cross = |index: usize| -> i32 {
            let size = sizes[index];
            if is_row {
                size.height as i32
            } else {
                size.width as i32
            }
        };
        // The solver is driven by the outer sizes (each child's box plus its margins), and it
        // Whether the layout is being *told* a size for this child rather than deriving it.
        //
        // `Hints::fixed` (`min == max`) is exactly the shape `CompositeBuilder::add_sized` and
        // `add_flexible` register, and it is what "the composite stated the size" means — the
        // exemption from the touch floor that
        // `a_size_the_composite_stated_is_not_inflated_by_the_touch_floor` pins. A child added
        // through `add` carries its own loose `hints()` (`min 0`), which is a wish rather than a
        // statement, so the floor still applies to it
        // (`a_plain_child_is_grown_to_the_profiles_touch_floor`).
        let size_is_stated = |index: usize| -> bool {
            described
                .get(index)
                .and_then(|info| info.as_ref())
                .is_some_and(|info| info.hints.width.is_fixed() && info.hints.height.is_fixed())
        };
        // The solver is driven by the outer sizes (each child's box plus its margins), and it
        // reports what each *item*'s box may be. The margins come back out below, so a margin is
        // room the child's box never takes: it is the declaration that the box sits a gap away
        // from its neighbour, not that the box is wider.
        //
        // The interior gaps are the caller's margins, and the solver's own `gap` is `self.gap`,
        // so a caller that also sets the layout's `gap` gets both — the same double spacing the
        // crate has always allowed, and the reason the assembly rules put inter-element space on
        // one of the two and not both.
        let mut solver = FlexLayout { child_sizes: sizes.clone(), ..self.clone() };
        // Each child's *own floor* is what it may be squeezed to, and the hints channel is the
        // only place that number arrives — `add_widget(id, stretch)` carries no size at all, so
        // `FlexItem::min_size` defaults to zero and the solver's `flex_shrink` step would
        // happily compress a child below the minimum it stated. That is the defect this line
        // fixes: two 100 px buttons in a 120 px row came back 57 px each, i.e. narrower than
        // their own labels, and a button narrower than its label is a button whose label elides
        // (BLUE22 §B.10, the "shrink rather than shrink to nothing" risk).
        //
        // The floor is written into the solver's own items rather than passed alongside them
        // because the solver reads `item.min_size` — and doing it here keeps `add_widget`'s
        // signature unchanged, so the fifteen layouts that never call `arrange` are unaffected
        // (principle #21).
        for (index, info) in described.iter().enumerate() {
            if let (Some(item), Some(info)) = (solver.items.get_mut(index), info) {
                // The solver works in **outer** sizes (a child's box plus its margins — see
                // `sizes` above), while `hints.width.min` describes the child's *box*. The floor
                // therefore has to carry the margins too, or the drawn extent would come out
                // `min - margins`: a button whose floor is 64 px with a 6 px leading margin was
                // laid out at an outer 64 and drawn 58 wide, i.e. below the minimum it stated,
                // which is the very thing the floor was added to prevent. Adding the margins back
                // is what makes the two units agree.
                item.min_size = Size::new(
                    info.hints.width.min.saturating_add(info.params.margins.horizontal_total()),
                    info.hints.height.min.saturating_add(info.params.margins.vertical_total()),
                );
            }
        }
        let (solved_main, _total_grow, _total_main) =
            solver.compute_main_sizes(available_main, scaled_gap);
        // # A known defect this pass does **not** repair (BLUE22 · logged as G-1)
        //
        // The solver may return a box that exceeds the room, and the positions below are packed
        // from the leading edge, so the *last* children of an over-full row are placed past the
        // band's far edge. Nothing clips at this layer, so such a child is painted outside its own
        // control — and, because the SVG backend emits absolute coordinates, it leaves the picture
        // entirely: `split_button` at 48 px wide put its 22 px arrow column at `x = 48` in a 48 px
        // face, i.e. a sub-part that is not there rather than one that is compressed.
        //
        // The obvious repair — cap each child at the room actually left — was implemented and
        // **reverted**, because it does not fix the row, it moves the failure onto a different
        // child: a row of two 100 px buttons in a 120 px band then came back `100 + 20` instead of
        // `100 + 100`, so the second button was drawn 20 px wide, below its own stated floor. Two
        // hundred-pixel buttons in a hundred-and-twenty-pixel band *cannot* be laid out, and the
        // shrink pass is right to say so by overhanging: `a_child_is_never_squeezed_below_its_own_minimum`
        // pins that, and a silent 20 px button is exactly the defect it was written to prevent.
        //
        // The honest statement is therefore that **the row's own extent and its children's floors
        // can disagree**, and that the disagreement is currently surfaced as an overhang rather
        // than resolved. Resolving it needs a decision this pass does not own — whether the row
        // clips, elides, or pushes back on the caller for more room — so it is left as it was
        // found rather than traded for a quieter but worse failure.
        // The leftover is what the justification distributes, and it is measured as the room the
        // band has minus the room the children **occupy** — their boxes *plus* the margins that
        // produced the gaps.
        //
        // # Why the leading margin is not added a second time here
        //
        // The solver is driven by the outer sizes (`child.bounds()` = preferred extent + margins)
        // and returns the same outer sizes when nothing grows, so each `solved_main[i]` already
        // *includes* that child's margins. The previous form added `inset(index).0` on top for
        // every child but the first, which double-counted every gap: a three-button row wanted
        // 220 px in a 240 px band, so the true leftover was 20 px, but the repeated margin made
        // `consumed` read 232 and the leftover read 8 — and for a wider margin it read zero, at
        // which point `justify_content` had nothing to distribute at all. That is the reason
        // `FlexEnd` appeared to do nothing and the row sat flush left.
        //
        // The solver's own `gap` is a separate term and is *not* part of `solved_main`, so it is
        // still counted once here.
        let consumed: i32 = solved_main.iter().sum::<i32>()
            + scaled_gap * (solved_main.len().saturating_sub(1)) as i32;
        let leftover = (available_main - consumed).max(0);
        let first_offset = match self.justify_content {
            JustifyContent::FlexStart => 0,
            JustifyContent::FlexEnd => leftover,
            JustifyContent::Center => leftover / 2,
            // `SpaceBetween` puts the leftover *between* the children and none at the edges; the
            // two "space around" variants add half a unit at each edge and a full unit between,
            // which is why their inter-item term is twice their edge term.
            JustifyContent::SpaceBetween => 0,
            JustifyContent::SpaceAround => leftover / (2 * solved_main.len().max(1) as i32),
            JustifyContent::SpaceEvenly => leftover / (solved_main.len().max(1) as i32 + 1),
        };
        let inter_extra = match self.justify_content {
            JustifyContent::SpaceBetween if solved_main.len() > 1 => {
                leftover / (solved_main.len() as i32 - 1)
            }
            JustifyContent::SpaceAround if !solved_main.is_empty() => {
                leftover / solved_main.len() as i32
            }
            JustifyContent::SpaceEvenly if !solved_main.is_empty() => {
                leftover / (solved_main.len() as i32 + 1)
            }
            _ => 0,
        };

        // ── The reverse axis packs from the far edge ──
        //
        // `arrange` used to ignore `is_reverse()` entirely: the loop below always packed from
        // `origin_main` upward, so a `RowReverse`/`ColumnReverse` layout was placed exactly as if it
        // were forward. `justify_positions` (the other entry point) has always handled it, so the two
        // paths disagreed about what a reverse direction means -- and `arrange` is the path
        // `CompositeBuilder` takes, which is why every assembled row in the crate was silently
        // forward-only. This is the same form `justify_positions` uses: the cursor starts at the far
        // edge and each item is placed by *subtracting* its extent, so the first child ends at the
        // band's end and the last child lands nearest the start.
        //
        // The first child's `first_offset` is mirrored too: `FlexEnd` puts its gutter at the
        // *leading* edge on a reversed axis, which is what keeps the justification meaning "from the
        // end the children are packed toward" rather than "from the left, always".
        let reverse = self.is_reverse();
        let mut cursor = if reverse {
            origin_main + available_main - first_offset
        } else {
            origin_main + first_offset
        };
        for index in 0..sizes.len() {
            let (left, top, right, bottom) = inset(index);
            // The solver reports the *whole* box, margins included, so the child's drawn extent
            // is that minus its own margins. Nothing grows here: a child that should absorb room
            // said so through `fill`, and `update`'s solver already paid it.
            let solved = solved_main.get(index).copied().unwrap_or(0);
            let main_len = (solved - left - right).max(0);
            let cross_len = (outer_cross(index) - top - bottom).min(available_cross).max(0);
            // Cross-axis alignment inside the child's own inset box.
            //
            // # Why `Stretch` is "grow to the band, unless the child asked for less"
            //
            // `Stretch` is the crate's default alignment and was unconditionally "the full band",
            // which is right for a child that has no opinion (a row of labels, a `fill` column) and
            // wrong for one that declares its own cross extent: a 2 px-inset toolbar item wants a
            // 52 px row inside a 56 px strip, and stretching it to 56 drew a hover fill that bled
            // over the inset the strip reserves. `outer_cross` is `hints.height.pref` (plus
            // margins) — the child's own statement of what it needs — so honouring a value *below*
            // the band is the same rule every other alignment already follows, while a child that
            // asked for more than the band gets the band (nothing clips at this layer, so painting
            // outside the parent would be a layout violation rather than a graceful degradation).
            //
            // A child with no opinion reports `0` here, which is also what it did before: the
            // `min(available_cross)` collapses it to the band exactly as the old line did.
            let declared_cross = (outer_cross(index) - top - bottom).min(available_cross).max(0);
            let stretch_cross = if declared_cross == 0 { available_cross } else { declared_cross };
            let (cross_start, cross_len) =
                match self.items[index].align_self.unwrap_or(self.align_items) {
                    AlignItems::Stretch => (0, stretch_cross),
                    AlignItems::FlexStart => (0, outer_cross(index) - top - bottom),
                    AlignItems::FlexEnd => {
                        (available_cross - cross_len, outer_cross(index) - top - bottom)
                    }
                    AlignItems::Center => {
                        ((available_cross - cross_len) / 2, outer_cross(index) - top - bottom)
                    }
                    AlignItems::Baseline => (0, outer_cross(index) - top - bottom),
                };
            let cross_start = cross_start.max(0);

            // The child's box sits at its cursor plus its own leading margin, so a margin is
            // space the child does not draw in — which is what makes it a *gap* rather than a
            // padding of the child's own chrome.
            // The cursor is the trailing edge in a reverse layout, so the child is placed *behind*
            // it by its own extent plus the margin on that side. `main_len + left + right` is the
            // solved box, so subtracting it and adding back the leading margin gives the child's
            // drawn origin -- the mirror of `cursor + left` below.
            let main_start = if reverse { cursor - solved + left } else { cursor + left };
            let child_rect = if is_row {
                Rect::new(
                    main_start,
                    cross_origin + cross_start + top,
                    main_len.max(0) as u32,
                    cross_len.max(0) as u32,
                )
            } else {
                Rect::new(
                    cross_origin + cross_start + left,
                    main_start,
                    cross_len.max(0) as u32,
                    main_len.max(0) as u32,
                )
            };
            if let Some(widget_id) = self.items[index].widget_id {
                // Every child gets at least the device class's minimum touch area, exactly as
                // `update_with_context` does. A flex row of small controls is the case this matters
                // most for: the layout would otherwise place a 20 px control in a 20 px slot on a
                // phone, where the neighbouring control's own expanded hit area overlaps it.
                //
                // Applying it here — in the one body both entry points run — is what makes the
                // context-free `update` and the context-aware entry agree; leaving it in
                // `update_with_context` alone is how a composite's children came to be the one
                // place the floor did not reach.
                //
                // # Why the grown box is clamped into the band only when the band has room
                //
                // The floor grows a child toward the device class's minimum touch target, centred
                // on the rectangle the layout produced, because a touch radius is symmetric. When
                // the allocation is smaller than the floor — a 16 px label wish inside a 32 px
                // target — the centred box begins *before* the allocation: a child laid out at the
                // content edge `x = 10` grew to `x = 2`, and one at `y = 6` grew to `y = -3`.
                //
                // Placement belongs to the layout (§B.6 rule 2, pinned by
                // `the_layout_owns_placement_and_padding_is_removed_first`), and nothing clips at
                // this layer, so a child placed before the content origin is drawn over whatever
                // is behind it.
                //
                // Sliding it back is only meaningful when the band is *larger* than the grown
                // box, because then there is somewhere to slide to. When the slot already spans
                // the band — a full-height row child, the ordinary case — there is no room to
                // slide into and the symmetric overhang is the intended answer
                // (`flex_layout_update_with_context_scales_gap_and_padding` pins exactly that), so
                // it is left as the centring produced it.
                let floored = if !apply_touch_floor || size_is_stated(index) {
                    child_rect
                } else {
                    let grown = crate::layout::types::grow_to_min_touch_size(
                        child_rect,
                        context.min_touch_size,
                    );
                    let slide = |origin: i32,
                                 extent: u32,
                                 slot_extent: u32,
                                 band_origin: i32,
                                 band_extent: u32| {
                        // No room to slide into: the slot already owns the band.
                        if slot_extent >= band_extent {
                            return origin;
                        }
                        let latest = band_origin + band_extent as i32 - extent as i32;
                        if latest < band_origin {
                            band_origin
                        } else {
                            origin.clamp(band_origin, latest)
                        }
                    };
                    // # The grown box is clamped only on the axis whose slot does *not* span
                    //
                    // `slide` returns `origin` unchanged when `slot_extent >= band_extent`,
                    // on the reasoning in the comment above: "the slot already owns the band,
                    // there is nowhere to slide to, so the symmetric overhang is intended".
                    //
                    // That reasoning is right about the **main axis** and was wrong once
                    // `grow_to_min_touch_size` grew **both**. On the main axis, a slot that spans
                    // the band is a child the layout gave the whole run to, and the overhang is
                    // the documented answer (see the `#[test]` note further up: "the growth is
                    // centred on the space the layout allocated"). On the **cross** axis,
                    // however, a `Stretch` child's slot is *also* the whole band — `stretch_cross`
                    // — so `slide` no-ops there too, and the re-centring that `Stretch` had already
                    // resolved was applied a second time. Measured on `TabView`'s 90 px strip: a tab
                    // laid out at `x = 0, width = 87, height = 40` came back `x = -5, width = 97` —
                    // drawn 5 px before its own control's content origin.
                    //
                    // So the clamp is conditional on the axis: it applies exactly where `slide`
                    // declined to act *and* the axis is not the one the layout actually allocated.
                    // `Slide` on the main axis keeps its documented overhang, which is what
                    // `flex_layout_update_with_context_scales_gap_and_padding` pins.
                    //
                    // Only the **origin** is clamped, never the extent: a child whose floor
                    // legitimately exceeds its room keeps that overhang on the far side. Shrinking
                    // it would re-create the contradicted-floor case G-1 resolved by scaling rather
                    // than clamping, and the near side is the one that loses content, because
                    // content before the content origin is drawn over whatever is behind it.
                    // Slide the grown box back inside the band, then clamp its **origin** into
                    // the band on the axes where the box is *smaller* than the band.
                    //
                    // # The two axes are not symmetric here, and that is the whole point
                    //
                    // `slide` already clamps an origin into the band whenever the box fits inside
                    // it. It deliberately does **not** clamp when `extent >= band_extent`, on the
                    // reasoning quoted above: "the slot already owns the band, there is nowhere to
                    // slide to, so the symmetric overhang is intended".
                    //
                    // That reasoning is right, but it was only ever *reached* on the axis the
                    // layout allocates — and `grow_to_min_touch_size` grows **both** axes. Two
                    // distinct situations came out of the single `slot_extent >= band_extent`
                    // test that `slide` saw:
                    //
                    // * **The box really is at least as large as the band** (a child the layout
                    //   gave the whole run, grown by the touch floor). The overhang is intended;
                    //   `flex_layout_update_with_context_scales_gap_and_padding` pins it.
                    // * **The slot spans the band but the box does not** — the `Stretch` case, or
                    //   any child whose slot is the full run on one axis while its box is
                    //   narrower. Here `slide` no-ops because of the *slot*, and the re-centring
                    //   `Stretch` had already resolved is applied a second time. Measured on
                    //   `TabView`'s 90 px strip before this fix: a tab laid out at
                    //   `x = 62, width = 28` grew to `x = 60, width = 32`, i.e. 2 px past the
                    //   strip's trailing edge — and 5 px *before* the leading edge in the first
                    //   version of the fixture, because `grow_to_min_touch_size` centres.
                    //
                    // The first case is recognised by comparing the **grown box** against the
                    // band, not the slot; the second is not, and so gets clamped. Comparing the
                    // box is also what keeps `G-1`'s contradicted-floor case intact: a child whose
                    // floor genuinely exceeds the band still reports `extent >= band_extent` and
                    // keeps its overhang rather than being shrunk.
                    //
                    // Only the **origin** moves; the extent is never touched. The near side is
                    // the one that loses content, because content before the content origin is
                    // drawn over whatever is behind it.
                    let realize = |origin: i32,
                                   box_extent: u32,
                                   slot_extent: u32,
                                   band_origin: i32,
                                   band_extent: u32|
                     -> i32 {
                        if box_extent >= band_extent {
                            // The box owns the band: keep the documented centring overhang.
                            return origin;
                        }
                        // The box fits, so it must not be pushed out of the band — either by a
                        // slot that spans it (`slide` declines) or by the origin clamp.
                        let latest = band_origin + band_extent as i32 - box_extent as i32;
                        origin.clamp(band_origin, latest.max(band_origin)).max(slide(
                            origin,
                            box_extent,
                            slot_extent,
                            band_origin,
                            band_extent,
                        ))
                    };
                    let clamped_x = realize(
                        grown.x,
                        grown.width,
                        child_rect.width,
                        content_rect.x,
                        content_rect.width,
                    );
                    let clamped_y = realize(
                        grown.y,
                        grown.height,
                        child_rect.height,
                        content_rect.y,
                        content_rect.height,
                    );
                    Rect::new(clamped_x, clamped_y, grown.width, grown.height)
                };
                out(widget_id, floored);
            }
            // Backward on a reversed axis, forward otherwise -- so the next child is always placed
            // against the one just emitted.
            if reverse {
                cursor -= solved + scaled_gap + inter_extra;
            } else {
                cursor += solved + scaled_gap + inter_extra;
            }
        }
    }

    /// Updates child geometry with the device context.
    ///
    /// # Why this now forwards to the hints-aware path
    ///
    /// This method cannot carry the `children` hints — its signature is the trait's and takes ids
    /// — so it is not the entry the composite builder calls. Its body therefore used to be a second
    /// implementation of the same arrangement: `compute_rects(content, Some(scaled_gap))` plus the
    /// touch floor. Once `arrange_body` grew the same two steps (so `arrange` and
    /// `arrange_with_context` would stop disagreeing), keeping this copy meant two solvers for one
    /// question — and the floor existed in only one of them, which is how a composite's children
    /// became the one place it did not apply.
    ///
    /// Forwarding with an empty hint list is what `Layout::arrange`'s own default does for the
    /// reverse direction: a caller of this method has no hints to give, and `arrange_body` already
    /// falls back to `child_sizes` — the same pre-hints channel this method's callers use — for
    /// exactly that case.
    ///
    /// # Why `allow(dead_code)`
    ///
    /// rustc's dead-code pass sees only the *static* call graph, and this override is reached
    /// through a trait object: [`KeyboardAwareLayout::update_with_context`](crate::layout::KeyboardAwareLayout)
    /// forwards to its `Box<dyn Layout>` inner, which is a `FlexLayout` whenever the keyboard-aware
    /// wrapper is built around one. Removing the override to silence the lint would silently change
    /// behaviour on that path — the trait default forwards to [`Layout::update`], which by design
    /// does **not** apply the touch floor — so the allow is the honest answer, and it is scoped to
    /// this one method rather than to the module.
    ///
    /// The two tests that call it directly (`flex_layout_update_with_context_scales_gap_and_padding`,
    /// `wrap_layout_update_with_context_scales_spacing`) live in `#[cfg(test)]` modules, which are a
    /// separate compilation unit and so do not count as production callers for the lint either.
    #[allow(dead_code)]
    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.arrange_body(rect, &[], context, true, widgets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;
    use crate::layout::{AxisHints, ChildInfo, Hints, LayoutParams};
    use crate::style::EdgeOffsets;

    #[test]
    fn flex_layout_default_creates_empty() {
        let layout = FlexLayout::new();
        assert_eq!(layout.item_count(), 0);
    }

    #[test]
    fn flex_layout_add_and_remove_widget() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 1);
        layout.add_widget(2, 2);
        assert_eq!(layout.item_count(), 2);
        assert!(layout.has_child(1));
        assert!(layout.has_child(2));

        layout.remove_widget(1);
        assert_eq!(layout.item_count(), 1);
        assert!(!layout.has_child(1));
        assert!(layout.has_child(2));
    }

    #[test]
    fn flex_layout_child_ids() {
        let mut layout = FlexLayout::new();
        layout.add_widget(10, 0);
        layout.add_widget(20, 0);
        let ids = layout.child_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&10));
        assert!(ids.contains(&20));
    }

    #[test]
    fn flex_layout_clear() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        assert_eq!(layout.item_count(), 2);
        layout.clear();
        assert_eq!(layout.item_count(), 0);
    }

    #[test]
    fn flex_layout_distributes_evenly_with_equal_grow() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0), Size::new(0, 0)]);
        layout.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Two equal flex-grow items in 200px: each gets 100px.
        assert_eq!(rects.get(&1).map(|r| r.width), Some(100));
        assert_eq!(rects.get(&2).map(|r| r.width), Some(100));
        assert_eq!(rects.get(&1).map(|r| r.height), Some(50));
        assert_eq!(rects.get(&2).map(|r| r.height), Some(50));
    }

    #[test]
    fn flex_layout_uneven_grow() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 1);
        layout.add_widget(2, 3);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0), Size::new(0, 0)]);
        layout.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Item 1 gets 1/4 (50px), item 2 gets 3/4 (150px).
        assert_eq!(rects.get(&1).map(|r| r.width), Some(50));
        assert_eq!(rects.get(&2).map(|r| r.width), Some(150));
    }

    #[test]
    fn flex_layout_column_direction() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Column,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0), Size::new(0, 0)]);
        layout.update(Rect::new(0, 0, 100, 200), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Column: each gets 100px height.
        assert_eq!(rects.get(&1).map(|r| r.height), Some(100));
        assert_eq!(rects.get(&2).map(|r| r.height), Some(100));
        assert_eq!(rects.get(&1).map(|r| r.width), Some(100));
        assert_eq!(rects.get(&2).map(|r| r.width), Some(100));
    }

    #[test]
    fn flex_layout_justify_center() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::Center,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(30, 20), Size::new(30, 20)]);
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Two fixed-size items (30+30=60) in 100px: leftover 40, centered offset 20.
        let r1 = rects.get(&1).unwrap();
        let r2 = rects.get(&2).unwrap();
        assert_eq!(r1.x, 20);
        assert_eq!(r2.x, 50);
    }

    #[test]
    fn flex_layout_justify_space_between() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::SpaceBetween,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(20, 10), Size::new(20, 10)]);
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Items width=20 each, total=40, leftover=60, gap=60/1=60
        // Positions: item1 at 0, item2 at 80
        assert_eq!(rects.get(&1).map(|r| r.x), Some(0));
        assert_eq!(rects.get(&2).map(|r| r.x), Some(80));
    }

    #[test]
    fn flex_layout_padding_applied() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            10,
        );
        layout.add_widget(1, 1);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0)]);
        layout.update(Rect::new(0, 0, 200, 60), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Padding 10 on each side: content width = 200-20=180, height = 60-20=40.
        // Item starts at (10, 10).
        assert_eq!(rects.get(&1).map(|r| r.x), Some(10));
        assert_eq!(rects.get(&1).map(|r| r.y), Some(10));
        assert_eq!(rects.get(&1).map(|r| r.width), Some(180));
        assert_eq!(rects.get(&1).map(|r| r.height), Some(40));
    }

    #[test]
    fn flex_layout_gap_between_items() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            10,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(30, 20), Size::new(30, 20)]);
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Fixed items 30+30=60, gap=10 => total=70
        // Item1 at 0, item2 at 30+10=40
        assert_eq!(rects.get(&1).map(|r| r.x), Some(0));
        assert_eq!(rects.get(&2).map(|r| r.x), Some(40));
    }

    #[test]
    fn flex_layout_wraps_rows_when_main_axis_overflows() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::Wrap,
            JustifyContent::FlexStart,
            AlignItems::FlexStart,
            5,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_widget(3, 0);
        layout.set_child_sizes(vec![Size::new(60, 20), Size::new(60, 30), Size::new(40, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 125, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 60, 20)));
        assert_eq!(rects.get(&2), Some(&Rect::new(65, 0, 60, 30)));
        assert_eq!(rects.get(&3), Some(&Rect::new(0, 35, 40, 10)));
    }

    #[test]
    fn flex_layout_wrap_reverse_starts_lines_at_cross_end() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::WrapReverse,
            JustifyContent::FlexStart,
            AlignItems::FlexStart,
            5,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_widget(3, 0);
        layout.set_child_sizes(vec![Size::new(60, 20), Size::new(60, 20), Size::new(40, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 125, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1).map(|rect| rect.y), Some(80));
        assert_eq!(rects.get(&2).map(|rect| rect.y), Some(80));
        assert_eq!(rects.get(&3).map(|rect| rect.y), Some(65));
    }

    #[test]
    fn flex_layout_wraps_columns_for_column_direction() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Column,
            FlexWrap::Wrap,
            JustifyContent::FlexStart,
            AlignItems::FlexStart,
            5,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_widget(3, 0);
        layout.set_child_sizes(vec![Size::new(20, 60), Size::new(30, 60), Size::new(10, 40)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 125), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 20, 60)));
        assert_eq!(rects.get(&2), Some(&Rect::new(0, 65, 30, 60)));
        assert_eq!(rects.get(&3), Some(&Rect::new(35, 0, 10, 40)));
    }

    #[test]
    fn flex_layout_min_size_constraint() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 0);
        if let Some(item) = layout.items_mut().last_mut() {
            item.min_size = Size::new(50, 0);
        }

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(10, 20)]);
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Min width is 50, intrinsic is 10, so width should be at least 50.
        assert_eq!(rects.get(&1).map(|r| r.width), Some(50));
    }

    #[test]
    fn flex_layout_align_items_flex_end() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::FlexEnd,
            0,
            0,
        );
        layout.add_widget(1, 0);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(30, 20)]);
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // FlexEnd: item should be at bottom (y = 100-20 = 80).
        assert_eq!(rects.get(&1).map(|r| r.y), Some(80));
    }

    #[test]
    fn flex_layout_align_items_center() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Center,
            0,
            0,
        );
        layout.add_widget(1, 0);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(30, 20)]);
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Center: item should be vertically centered (y = (100-20)/2 = 40).
        assert_eq!(rects.get(&1).map(|r| r.y), Some(40));
    }

    #[test]
    fn flex_layout_align_self_overrides_align_items() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::FlexStart,
            0,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        if let Some(item) = layout.items_mut().get_mut(1) {
            item.align_self = Some(AlignItems::Center);
        }

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(30, 20), Size::new(30, 20)]);
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Item 1: FlexStart => y = 0
        // Item 2: align_self = Center => y = (100-20)/2 = 40
        assert_eq!(rects.get(&1).map(|r| r.y), Some(0));
        assert_eq!(rects.get(&2).map(|r| r.y), Some(40));
    }

    #[test]
    fn flex_layout_update_with_context_scales_gap_and_padding() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            10,
            10,
        );
        layout.add_widget(1, 1);

        let context = LayoutContext { layout_scale: 2.0, ..LayoutContext::default() };

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0)]);
        layout.update_with_context(Rect::new(0, 0, 200, 60), &context, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // With scale=2.0: padding=20, content area = (20,20) to (180,40), so the flex item is
        // 160x20. It is then grown to the context's minimum touch height, because a 20 px target is
        // smaller than the desktop class's 32 px minimum — that is what `min_touch_size` is for, and
        // this test previously asserted the un-grown height, which is why the field had no reader.
        assert_eq!(rects.get(&1).map(|r| r.x), Some(20));
        assert_eq!(rects.get(&1).map(|r| r.width), Some(160));
        let grown = rects.get(&1).copied().expect("the flex item was laid out");
        assert_eq!(grown.height, context.min_touch_size.height.max(20));
        // The growth is centred on the space the layout allocated, not anchored at its top.
        assert_eq!(
            grown.y,
            20 - (grown.height as i32 - 20) / 2,
            "a grown child must stay centred on its allocated slot"
        );
    }

    #[test]
    fn flex_layout_row_reverse() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::RowReverse,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);

        let mut rects = HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0), Size::new(0, 0)]);
        layout.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // RowReverse: item1 at x=100, item2 at x=0 (reversed order)
        assert_eq!(rects.get(&1).map(|r| r.x), Some(100));
        assert_eq!(rects.get(&2).map(|r| r.x), Some(0));
    }

    /// `arrange` — the path `CompositeBuilder` takes — honours the reverse axis too.
    ///
    /// # The defect this pins
    ///
    /// `update` and `arrange` are two entry points onto one layout, and only `update` routed the
    /// main-axis positions through `justify_positions`, which is where `is_reverse()` is handled.
    /// `arrange` packed from `origin_main` upward unconditionally, so a `RowReverse` layout came
    /// back **identical to a forward one**: the direction was read by one path and ignored by the
    /// other. The test above could not catch it because it calls `update`.
    ///
    /// The assertion is the *mirror* relation, not two hardcoded numbers: in a reverse row the first
    /// child ends where the last child would have ended forward, and the children appear in the
    /// opposite order, so the two layouts' x-positions are a reflection of one another. Stating it
    /// that way means a change to the band, the gaps or the sizes cannot make the test agree with a
    /// bug.
    #[test]
    fn arrange_honours_the_reverse_axis() {
        let rect_of = |direction: FlexDirection| -> HashMap<u64, Rect> {
            let mut layout = FlexLayout::with_params(
                direction,
                FlexWrap::NoWrap,
                JustifyContent::FlexStart,
                AlignItems::Stretch,
                0,
                0,
            );
            layout.add_widget(1, 1);
            layout.add_widget(2, 1);
            let children = vec![
                ChildInfo { id: 1, hints: Hints::default(), params: LayoutParams::default() },
                ChildInfo { id: 2, hints: Hints::default(), params: LayoutParams::default() },
            ];
            let mut rects = HashMap::new();
            // Both children ask for half the band, so the forward and reverse placements are exact
            // reflections and no rounding can make a wrong answer look right.
            layout.set_child_sizes(vec![Size::new(100, 50), Size::new(100, 50)]);
            layout.arrange(Rect::new(0, 0, 200, 50), &children, &mut |id, rect| {
                rects.insert(id, rect);
            });
            rects
        };

        let forward = rect_of(FlexDirection::Row);
        let reverse = rect_of(FlexDirection::RowReverse);

        assert_eq!(forward.get(&1).map(|r| r.x), Some(0), "forward packs from the leading edge");
        assert_eq!(forward.get(&2).map(|r| r.x), Some(100), "and the second follows it");
        assert_eq!(
            reverse.get(&1).map(|r| r.x),
            Some(100),
            "reversed, the first child is packed against the far edge"
        );
        assert_eq!(reverse.get(&2).map(|r| r.x), Some(0), "and the second lands beside it");
        // Spelled as the reflection, so the assertion is the property rather than the four numbers.
        for id in [1u64, 2] {
            assert_eq!(
                forward.get(&id).map(|r| r.x),
                reverse.get(&id).map(|r| 200 - r.x - r.width as i32),
                "the reverse placement of child {id} is the forward one reflected about the band's \
                 centre"
            );
        }
    }

    // ── The hints channel (BLUE22 §B.5.2) ───────────────────────────────

    #[test]
    fn justify_content_distributes_the_leftover_it_is_given() {
        // The leftover-absorbing branch used to dump the whole surplus on the *last* child,
        // so a row of fixed-width children could never leave anything for the justification.
        // `FlexEnd` was consequently unreachable: a three-button row in a 240 px band with
        // 20 px of spare room came back flush left with a 20 px wider last button.
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexEnd,
            AlignItems::Stretch,
            0,
            0,
        );
        for id in [1u64, 2, 3] {
            layout.add_widget(id, 0);
        }
        let children = vec![
            ChildInfo::new(1, Hints::fixed(60, 30)),
            ChildInfo::new(2, Hints::fixed(60, 30)),
            ChildInfo::new(3, Hints::fixed(60, 30)),
        ];
        let mut rects = HashMap::new();
        layout.arrange(Rect::new(0, 0, 240, 40), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // The row is 180 px in a 240 px band: 60 px spare, all of it *before* the children.
        assert_eq!(
            rects.get(&1).map(|r| r.x),
            Some(60),
            "FlexEnd pins the row to the trailing edge"
        );
        // The row is 180 px in a 240 px band: 60 px spare, all of it *before* the children.
        assert_eq!(
            rects.get(&1).map(|r| r.x),
            Some(60),
            "FlexEnd pins the row to the trailing edge"
        );
        assert_eq!(rects.get(&3).map(|r| r.x), Some(180));
        assert_eq!(rects.get(&3).map(|r| r.x + r.width as i32), Some(240));
        for id in [1u64, 2, 3] {
            assert_eq!(
                rects.get(&id).map(|r| r.width),
                Some(60),
                "a non-growing child must keep its own width, not absorb the leftover"
            );
        }
    }

    #[test]
    fn justification_measures_the_leftover_including_the_gaps() {
        // The leftover is `band - occupied`, and "occupied" must count each gap **once**. The
        // previous form added each child's leading margin on top of a size that already
        // included it, so every gap was counted twice: 20 px of genuine spare room read as 8,
        // and with a slightly wider gap it read as zero and the justification had nothing to
        // distribute at all. A row of three buttons 6 px apart is the shape this has to hold
        // for, because that is what a dialog's action row is.
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexEnd,
            AlignItems::Stretch,
            0,
            0,
        );
        for id in [1u64, 2, 3] {
            layout.add_widget(id, 0);
        }
        let gap = 6u32;
        let children: Vec<ChildInfo> = [1u64, 2, 3]
            .iter()
            .enumerate()
            .map(|(index, id)| {
                ChildInfo::new(*id, Hints::fixed(60, 30)).with_params(
                    LayoutParams::new().with_margins(EdgeOffsets::new(
                        0,
                        0,
                        0,
                        if index == 0 { 0 } else { gap },
                    )),
                )
            })
            .collect();
        let mut rects = HashMap::new();
        layout.arrange(Rect::new(0, 0, 240, 40), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Occupied = 3 x 60 buttons + 2 x 6 gaps = 192, so 48 px of spare room, and FlexEnd
        // puts all of it before the first button.
        assert_eq!(
            rects.get(&1).map(|r| r.x),
            Some(48),
            "the leftover must be measured net of each gap exactly once"
        );
        assert_eq!(rects.get(&3).map(|r| r.x + r.width as i32), Some(240));
        for id in [1u64, 2, 3] {
            assert_eq!(rects.get(&id).map(|r| r.width), Some(60));
        }
    }

    #[test]
    fn a_child_is_never_squeezed_below_its_own_minimum() {
        // The shrink branch applied `max(item.min_size)` and then a "if we couldn't shrink
        // enough, cap at available" pass that cut straight through it, down to zero. Two
        // 100 px children in a 120 px band therefore came back 57 px each — narrower than the
        // labels they were about to draw.
        //
        // # What this pins now that G-1 is resolved
        //
        // The floors are unsatisfiable here (200 px of floor in a 120 px band), so *some* child
        // must end up below its floor — nothing can change that. What the layout owes the caller is
        // that the loss is **shared** rather than paid by whoever happens to be last, so this
        // asserts the two children stay equal and that the run fits its band. Before G-1 was
        // resolved the band was ignored: child 1 got its full 100 px and child 2 was packed at
        // `x = 100` in a 120 px band, i.e. 80 px outside the control it belongs to.
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        let children = vec![
            ChildInfo::new(
                1,
                Hints { width: AxisHints::new(100, 100, 200), height: AxisHints::new(30, 30, 30) },
            ),
            ChildInfo::new(
                2,
                Hints { width: AxisHints::new(100, 100, 200), height: AxisHints::new(30, 30, 30) },
            ),
        ];
        let mut rects = HashMap::new();
        layout.arrange(Rect::new(0, 0, 120, 40), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });

        let widths: Vec<u32> = [1u64, 2]
            .iter()
            .map(|id| rects.get(id).map(|r| r.width).expect("both children were placed"))
            .collect();
        assert_eq!(
            widths[0], widths[1],
            "two children with the same floor must be treated the same: {widths:?}"
        );
        assert_eq!(
            widths.iter().sum::<u32>(),
            120,
            "the run must fit its band, whatever the floors say: {widths:?}"
        );
        // Each child keeps a majority of its floor rather than being reduced to nothing: the loss
        // is proportional, so no child collapses while another keeps everything.
        for (index, width) in widths.iter().enumerate() {
            assert!(
                *width >= 60,
                "child {} was reduced to {width}, far past share of the shortfall",
                index + 1
            );
        }
    }

    #[test]
    fn a_margin_is_a_gap_and_not_a_reduction_of_the_childs_own_size() {
        // `min_size` is expressed in the solver's *outer* units, which include the child's
        // margins. A 64 px floor on a child with a 6 px leading margin was previously
        // floored to an outer 64 and then drawn 58 wide — below its own minimum — because
        // the margin was subtracted after the floor was applied.
        //
        // # What this pins now that G-1 is resolved
        //
        // The band below is too narrow for both floors, so the *proportional* pass scales them
        // (see `compute_main_sizes`). What must survive that is the relation this test is named
        // for: the margin is a **gap between two boxes**, not part of either box. Before G-1 was
        // resolved the overhang hid a second reading of it — the trailing child was pushed out of
        // the picture, so its gap could not be checked at all.
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        let children = vec![
            ChildInfo::new(1, Hints::at_least(64, 30)),
            ChildInfo::new(2, Hints::at_least(64, 30))
                .with_params(LayoutParams::new().with_margins(EdgeOffsets::new(0, 0, 0, 6))),
        ];
        let rects = {
            let mut rects = HashMap::new();
            // A band too narrow for both floors: the children are scaled, not overhung.
            layout.arrange(Rect::new(0, 0, 120, 40), &children, &mut |id, rect| {
                rects.insert(id, rect);
            });
            rects
        };

        let first = rects.get(&1).copied().expect("the first child was placed");
        let second = rects.get(&2).copied().expect("the second child was placed");
        assert_eq!(
            second.x - (first.x + first.width as i32),
            6,
            "the margin must remain the gap between the two boxes: first {first:?}, second {second:?}"
        );
        assert_eq!(
            second.x + second.width as i32,
            120,
            "and the run must fit its band, so the trailing child is inside the control"
        );
        // The margin is room the child does not draw in, so the two boxes *plus the gap* are the
        // band — the margin is neither extra width nor stolen width.
        assert_eq!(
            first.width + 6 + second.width,
            120,
            "the two boxes and the one gap between them must account for the band"
        );
    }

    /// A child that *can* still shrink gives up the room a floored sibling cannot.
    ///
    /// # What this pins
    ///
    /// The proportional shrink pass alone stops as soon as any child reaches its floor, and the
    /// room that child would not release is then simply never taken from anyone else — so a row of
    /// a floored child and a freely-shrinkable one stayed wider than its band for no reason: the
    /// second child was at its comfortable size while the row overhung.
    ///
    /// This is the case a composite hits constantly: a split button's face is a *text-driven*
    /// trigger (plenty of room above its floor) beside a *fixed* arrow column (no room above its
    /// floor at all). Narrowing the face has to compress the trigger, and before the second pass it
    /// did not — the arrow column was pushed out of the control instead.
    ///
    /// # Why this case is not the G-1 case
    ///
    /// Here the floors *do* fit — `60 + 100 = 160` is more than the 140 px band, but the first
    /// child only needs 60 of the room it is claiming, so the second pass can resolve the deficit
    /// without anyone crossing a floor. The proportional scaling that resolves G-1 therefore does
    /// not run: `leftover` reaches zero first. The assertion that the first child keeps its 60 px
    /// *is* the distinction between the two mechanisms.
    #[test]
    fn a_child_that_can_shrink_gives_up_the_room_a_floored_sibling_cannot() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        let children = vec![
            // 100 px wide with a 60 px floor: it can give up 40.
            ChildInfo::new(1, Hints { width: AxisHints::new(60, 100, 200), ..Hints::default() }),
            // 100 px wide with a 100 px floor: it can give up nothing.
            ChildInfo::new(2, Hints::fixed(100, 30)),
        ];
        let mut rects = HashMap::new();
        // A 160 px band: 40 px of deficit, all of which the first child can release and none of
        // which the second can.
        layout.arrange(Rect::new(0, 0, 160, 40), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });

        let first = rects.get(&1).copied().expect("the first child was placed");
        let second = rects.get(&2).copied().expect("the second child was placed");
        assert_eq!(second.width, 100, "the floored child keeps its size");
        assert_eq!(
            first.width, 60,
            "the child with room above its floor gives up exactly what the row needs"
        );
        assert_eq!(first.width + second.width, 160, "and the two of them are the band");
    }

    /// The run never leaves its band, even when the children's floors cannot all be satisfied.
    ///
    /// # The defect this closes (BLUE22 · G-1)
    ///
    /// Two 100 px floors cannot both be honoured in a 140 px band — that is arithmetic, not a bug,
    /// and the shrink pass is explicit that a child is never squeezed below its own floor because a
    /// button narrower than its label is a button whose label elides.
    ///
    /// What *was* a bug is where the 60 px that did not fit went. The positions are packed from the
    /// leading edge, so the whole shortfall was paid by whichever child came last: the second child
    /// was placed at `x = 100` in a 140 px band and, because the SVG backend emits absolute
    /// coordinates and nothing clips at this layer, it was **absent from the picture** rather than
    /// overflowing it. A silently missing control is worse than a compressed one.
    ///
    /// # The resolution, and why not the obvious one
    ///
    /// The sizes are scaled by `available / floors`, so the run fits and every child stays inside
    /// the band while keeping its proportions. The obvious alternative — "give each child at most
    /// the room that is left" — was implemented and **reverted**: the same row came back
    /// `100 + 40`, i.e. the shortfall moved into the *second* child as a 40 px button. That does not
    /// resolve the contradiction either, it just concentrates the loss in one child instead of
    /// spreading it, and 40 px is below the 100 px floor it was supposed to respect.
    ///
    /// Scaling cannot make an unsatisfiable layout satisfiable. What it can do is make the loss
    /// **shared, bounded and contained**, which is the part the layout actually owns.
    ///
    /// # Why the scale is deliberately tiny rather than "identical sizes"
    ///
    /// A child with a larger floor keeps a larger box here, which is what keeps the relation
    /// between a wide and a narrow control readable when a form is squeezed. Forcing every child to
    /// `available / count` would make a 100 px button and a 10 px icon the same width.
    #[test]
    fn a_run_that_cannot_fit_its_floors_still_stays_inside_its_band() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        let children = vec![
            ChildInfo::new(1, Hints::fixed(100, 30)),
            ChildInfo::new(2, Hints::fixed(100, 30)),
        ];
        let mut rects = HashMap::new();
        layout.arrange(Rect::new(0, 0, 140, 40), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });
        let first = rects.get(&1).copied().expect("the first child was placed");
        let second = rects.get(&2).copied().expect("the second child was placed");

        // The property that matters, stated as containment rather than as a number: every child is
        // inside the band it was offered.
        for (label, rect) in [("first", first), ("second", second)] {
            assert!(
                rect.x >= 0 && rect.x + rect.width as i32 <= 140,
                "the {label} child must stay inside the band, got {rect:?}"
            );
        }
        // And the run is the band: the shortfall is spread, not deferred.
        assert_eq!(
            first.width + second.width,
            140,
            "the two children must account for the band: first {first:?}, second {second:?}"
        );
        // Equal floors get equal boxes, so the loss is not concentrated anywhere.
        assert_eq!(first.width, second.width, "the shortfall must be shared, not deferred");
        assert_eq!(second.x, 70, "and the second child begins where the first ends");
    }

    /// A child whose floor is larger keeps a larger box when the band cannot hold both.
    ///
    /// The companion to the test above: the scaling preserves *proportions*, so a form that is
    /// squeezed still reads as the same form rather than as a row of equal slabs. It is also the
    /// property that distinguishes scaling from "give everyone `available / count`".
    #[test]
    fn a_squeezed_run_keeps_its_childrens_proportions() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        let children =
            vec![ChildInfo::new(1, Hints::fixed(120, 30)), ChildInfo::new(2, Hints::fixed(40, 30))];
        let mut rects = HashMap::new();
        // 80 px for 160 px of floor: a half-scale band.
        layout.arrange(Rect::new(0, 0, 80, 40), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });
        let first = rects.get(&1).copied().expect("the first child was placed");
        let second = rects.get(&2).copied().expect("the second child was placed");
        assert_eq!(first.width + second.width, 80, "the run fits its band");
        assert!(
            first.width > second.width,
            "the wider control keeps the wider box: {} vs {}",
            first.width,
            second.width
        );
        // 120:40 is 3:1, and the scale is exact for a band that divides evenly.
        assert_eq!(first.width, 60);
        assert_eq!(second.width, 20);
    }

    /// A child that declares its own cross extent is not stretched past it.
    ///
    /// # What this pins
    ///
    /// `AlignItems::Stretch` used to be "the full band", unconditionally. That is right for a child
    /// with no opinion — a row of labels, a `fill` column — and wrong for a child that states its
    /// own cross size: a toolbar item reserves 2 px of inset at each end of its strip, so a 56 px
    /// strip holds 52 px rows. Stretching the item to the full 56 drew its hover fill over the
    /// inset the strip had reserved.
    ///
    /// The fix reads the child's own `hints.pref` on the cross axis, so "I want to fill the cross
    /// axis" is still expressed by declaring no preference (which reports `0` and is then expanded
    /// to the band, exactly as before).
    #[test]
    fn a_child_that_declares_its_cross_size_is_not_stretched_past_it() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        let children = vec![ChildInfo::new(1, Hints::fixed(40, 30))];
        let mut rects = HashMap::new();
        // A 60 px tall band: the child asked for 30 and the default alignment is `Stretch`.
        layout.arrange(Rect::new(0, 0, 200, 60), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });
        let placed = rects.get(&1).copied().expect("the child was placed");
        assert_eq!(placed.height, 30, "the child's own cross size wins over the band: {placed:?}");
    }

    /// A child with no cross opinion still fills the band.
    ///
    /// The companion to the test above: the crate's default alignment must keep doing what it did
    /// for the overwhelming majority of children, which declare a size on one axis only.
    #[test]
    fn a_child_with_no_cross_opinion_still_fills_the_band() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        let children =
            vec![ChildInfo::new(1, Hints { width: AxisHints::fixed(60), ..Default::default() })];
        let mut rects = HashMap::new();
        layout.arrange(Rect::new(0, 0, 200, 60), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });
        let placed = rects.get(&1).copied().expect("the child was placed");
        assert_eq!(
            placed.height, 60,
            "a child that declared no cross size is stretched to the band: {placed:?}"
        );
    }

    #[test]
    fn a_layout_can_size_its_children_without_being_told_in_advance() {
        // BLUE22 §B.10 judgment 2: "a layout that does not know the sizes can lay out from
        // `&[ChildInfo]` alone". This is the whole point of the channel — the old path
        // required `set_child_sizes` *before* `update`, so a caller who forgot got a row of
        // zero-width cells rather than an error.
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);

        // Deliberately no `set_child_sizes` call.
        let children = vec![
            ChildInfo::new(1, Hints::at_least(60, 30)),
            ChildInfo::new(2, Hints::at_least(40, 30)),
        ];
        let mut rects = HashMap::new();
        layout.arrange(Rect::new(0, 0, 200, 50), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1).map(|r| r.width), Some(60), "child 1 gets its own hint");
        assert_eq!(rects.get(&2).map(|r| r.width), Some(40), "child 2 gets its own hint");
        // And they are actually placed, not stacked at the origin.
        assert_eq!(rects.get(&1).map(|r| r.x), Some(0));
        assert_eq!(rects.get(&2).map(|r| r.x), Some(60));
    }

    #[test]
    fn the_hint_channel_and_the_legacy_path_agree_on_the_same_sizes() {
        // The channel must not be a *different* layout algorithm: given the same sizes, the
        // two entry points have to produce the same geometry, or adopting the channel would
        // silently change every existing layout.
        let mut legacy = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::FlexStart,
            8,
            0,
        );
        legacy.add_widget(1, 0);
        legacy.add_widget(2, 0);
        legacy.set_child_sizes(vec![Size::new(60, 30), Size::new(40, 30)]);

        let mut direct = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::FlexStart,
            8,
            0,
        );
        direct.add_widget(1, 0);
        direct.add_widget(2, 0);

        let children = vec![
            ChildInfo::new(1, Hints::at_least(60, 30)),
            ChildInfo::new(2, Hints::at_least(40, 30)),
        ];

        let mut from_legacy = HashMap::new();
        legacy.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            from_legacy.insert(id, rect);
        });
        let mut from_hints = HashMap::new();
        direct.arrange(Rect::new(0, 0, 200, 50), &children, &mut |id, rect| {
            from_hints.insert(id, rect);
        });

        assert_eq!(from_legacy, from_hints, "both entry points must agree");
    }

    #[test]
    fn a_child_the_caller_did_not_describe_falls_back_to_its_stored_size() {
        // A caller migrating one child at a time must not lose the children it has not
        // converted yet — the same "incremental migration" property `arrange`'s default
        // gives the fifteen layouts.
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_sizes(vec![Size::new(0, 0), Size::new(70, 30)]);

        // Only child 1 is described.
        let children = vec![ChildInfo::new(1, Hints::at_least(30, 30))];
        let mut rects = HashMap::new();
        layout.arrange(Rect::new(0, 0, 200, 50), &children, &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1).map(|r| r.width), Some(30));
        assert_eq!(rects.get(&2).map(|r| r.width), Some(70), "the stored size still applies");
    }

    // ── S-50: main-axis max must hold in every branch ──────────────────────────────

    #[test]
    fn main_axis_max_holds_in_the_grow_branch() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.items_mut()[0].max_size = Size::new(20, 0);
        layout.items_mut()[1].max_size = Size::new(20, 0);
        layout.set_child_sizes(vec![Size::new(10, 10), Size::new(10, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Both items are capped at 20; the refused room is left for justification, not dumped
        // onto the last child (which used to make the second item 80 wide).
        assert_eq!(rects.get(&1).map(|r| r.width), Some(20));
        assert_eq!(rects.get(&2).map(|r| r.width), Some(20));
        assert_eq!(rects.get(&2).map(|r| r.x + r.width as i32), Some(40));
    }

    #[test]
    fn main_axis_max_holds_in_the_non_grow_branch() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 0);
        layout.items_mut()[0].max_size = Size::new(20, 0);
        layout.set_child_sizes(vec![Size::new(100, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1).map(|r| r.width), Some(20), "a non-growing child is still capped");
    }

    // ── S-51: the declared gap is paid once in space-* alignments ───────────────────

    #[test]
    fn space_between_pays_the_declared_gap_and_reaches_the_edge() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::SpaceBetween,
            AlignItems::Stretch,
            10,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_sizes(vec![Size::new(10, 10), Size::new(10, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1).map(|r| r.x), Some(0));
        assert_eq!(
            rects.get(&2).map(|r| r.x + r.width as i32),
            Some(100),
            "the second child reaches the trailing edge"
        );
    }

    #[test]
    fn space_distribution_preserves_the_gap_when_leftover_is_zero() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::SpaceBetween,
            AlignItems::Stretch,
            10,
            0,
        );
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_child_sizes(vec![Size::new(0, 0), Size::new(0, 0)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        let first = rects.get(&1).copied().expect("first child placed");
        let second = rects.get(&2).copied().expect("second child placed");
        assert_eq!(
            first.x + first.width as i32 + 10,
            second.x,
            "the declared gap survives a zero leftover"
        );
    }

    #[test]
    fn space_around_pays_the_declared_gap_and_stays_roughly_symmetric() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::SpaceAround,
            AlignItems::Stretch,
            10,
            0,
        );
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_sizes(vec![Size::new(10, 10), Size::new(10, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        let first = rects.get(&1).copied().expect("first child placed");
        let second = rects.get(&2).copied().expect("second child placed");
        let lead = first.x;
        let trail = 100 - (second.x + second.width as i32);
        assert!(
            (lead - trail).abs() <= 1,
            "SpaceAround is symmetric up to rounding: {lead} vs {trail}"
        );
        assert!(second.x - (first.x + first.width as i32) >= 10, "the declared gap is still paid");
    }

    // ── S-52: removal keeps identity ↔ size correspondence ──────────────────────────

    #[test]
    fn removing_a_widget_does_not_reassign_its_size() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_sizes(vec![Size::new(10, 10), Size::new(30, 30)]);
        layout.remove_widget(1);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&2).map(|r| r.width), Some(30), "the survivor keeps its own size");
    }

    #[test]
    fn removing_from_the_middle_keeps_remaining_sizes_aligned() {
        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_widget(3, 0);
        layout.set_child_sizes(vec![Size::new(10, 10), Size::new(20, 10), Size::new(30, 10)]);
        layout.remove_widget(2);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1).map(|r| r.width), Some(10));
        assert_eq!(rects.get(&3).map(|r| r.width), Some(30));
    }

    // ── S-53: cross-axis bounds are honoured by every alignment ────────────────────

    #[test]
    fn cross_axis_max_is_applied_to_stretch() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            0,
        );
        layout.add_widget(1, 0);
        layout.items_mut()[0].max_size = Size::new(0, 20);
        layout.set_child_sizes(vec![Size::new(10, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1).map(|r| r.height), Some(20), "Stretch is capped at the cross max");
    }

    #[test]
    fn cross_axis_min_is_applied_to_flex_start() {
        let mut layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::FlexStart,
            0,
            0,
        );
        layout.add_widget(1, 0);
        layout.items_mut()[0].min_size = Size::new(0, 40);
        layout.set_child_sizes(vec![Size::new(10, 10)]);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(
            rects.get(&1).map(|r| r.height),
            Some(40),
            "FlexStart is floored at the cross min"
        );
    }

    // ── S-47: negative padding is rejected, not folded into geometry ───────────────

    #[test]
    fn negative_padding_is_rejected_at_construction_and_read_sites() {
        let layout = FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            0,
            -1,
        );
        assert_eq!(layout.padding, 0, "with_params rejects a negative padding");

        let mut layout = FlexLayout::new();
        layout.add_widget(1, 0);
        layout.padding = -1; // `padding` is `pub`, so a caller can mutate it directly.
        layout.set_child_sizes(vec![Size::new(10, 10)]);
        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.get(&1).map(|r| r.x), Some(0));
        assert_eq!(rects.get(&1).map(|r| r.y), Some(0));
    }
}
