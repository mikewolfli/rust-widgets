// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wrap layout manager — auto-wrap flow layout that places items in rows or columns,
//! breaking to the next line/column when the available space is exhausted.
//!
//! # One line breaker
//!
//! [`break_lines`] and [`place_lines`] are the crate's only implementation of "where does a
//! wrapped line end and where do its items sit". [`WrapLayout`] calls them with the sizes it was
//! told; [`FlowLayout`](crate::layout::FlowLayout) calls them with the sizes its children report.
//! The two layouts keep their own size sources and configuration surfaces, but not two
//! algorithms — see LAY-N08.
use super::{ChildInfo, Layout, LayoutContext};
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect, Size};

/// Direction in which items flow before wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapDirection {
    /// Items flow left-to-right, wrapping to new rows.
    #[default]
    Horizontal,
    /// Items flow top-to-bottom, wrapping to new columns.
    Vertical,
}

/// Alignment of items within each wrap line/column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapAlignment {
    /// Align to the start of the line/column.
    #[default]
    Start,
    /// Centre items within the line/column.
    Center,
    /// Align to the end of the line/column.
    End,
    /// Distribute with equal space between items.
    SpaceBetween,
    /// Distribute with equal space around each item.
    SpaceAround,
}

/// Tracks size and id for a single wrapped child.
#[derive(Debug, Clone, Copy)]
struct WrapChild {
    widget_id: ObjectId,
    size: Size,
}

/// Auto-wrap flow layout that arranges items in rows or columns,
/// wrapping when the available extent is exceeded.
#[derive(Debug)]
pub struct WrapLayout {
    /// Flow direction before wrapping.
    direction: WrapDirection,
    /// Alignment within each wrap line/column.
    alignment: WrapAlignment,
    /// Spacing between items and between lines.
    spacing: i32,
    /// Spacing between wrap lines, when it differs from [`Self::spacing`].
    ///
    /// Flutter's `Wrap` separates `spacing` (within a run) from `runSpacing` (between runs);
    /// the two were one value here, so a row of buttons could not be tight horizontally and
    /// airy between rows.
    run_spacing: Option<i32>,
    /// Outer padding.
    padding: i32,
    /// Managed children with size tracking.
    children: Vec<WrapChild>,
}

impl Default for WrapLayout {
    fn default() -> Self {
        Self::new(WrapDirection::Horizontal, WrapAlignment::Start, 8, 8)
    }
}

/// The gap `SpaceBetween` / `SpaceAround` leave between two adjacent items, and the
/// leading/trailing gap `SpaceAround` leaves at the ends.
///
/// # The model
///
/// Both alignments distribute only what is left **after** the base gap, which is what
/// keeps the wrap decision and the drawing in agreement ([`break_lines`] charges each
/// pair the configured spacing when it decides where a line breaks).
///
/// * `SpaceBetween::gap` = base + leftover / (n - 1), no leading gap.
/// * `SpaceAround::gap` = base + leftover / n, with a leading gap of
///   `gap - base` divided by two, i.e. half a slot at each end.
///   With `base == 0` this is the textbook spelling: `leftover / n` between items and
///   `leftover / (2n)` before the first and after the last.
///
/// A one-item line has no pair to separate and no slot to distribute, so both alignments
/// fall back to the base gap; without that the leading gap would swallow the whole
/// leftover and shove the lone item across the line.
struct Spacing {
    /// Between two adjacent items.
    gap: i32,
}

fn spacing_for(
    alignment: WrapAlignment,
    available: i32,
    sizes_sum: i32,
    item_count: usize,
    base_gap: i32,
) -> Spacing {
    if item_count < 2 {
        return Spacing { gap: base_gap };
    }
    let used = sizes_sum + (item_count as i32 - 1) * base_gap;
    let leftover = available - used;
    match alignment {
        WrapAlignment::SpaceBetween => {
            Spacing { gap: base_gap + leftover / (item_count as i32 - 1) }
        }
        WrapAlignment::SpaceAround => Spacing { gap: base_gap + leftover / item_count as i32 },
        _ => Spacing { gap: base_gap },
    }
}

/// The extent a child occupies on the axis its line runs along.
fn main_extent(size: Size, direction: WrapDirection) -> i32 {
    match direction {
        WrapDirection::Horizontal => size.width as i32,
        WrapDirection::Vertical => size.height as i32,
    }
}

/// The extent a child occupies on the axis lines are stacked across.
fn cross_extent(size: Size, direction: WrapDirection) -> i32 {
    match direction {
        WrapDirection::Horizontal => size.height as i32,
        WrapDirection::Vertical => size.width as i32,
    }
}

/// Breaks `sizes` into lines along `direction`.
///
/// # Why this is a free function
///
/// Two layouts break lines: [`WrapLayout`] and
/// [`FlowLayout`](crate::layout::FlowLayout). They differ in where a child's size comes from
/// (a `set_child_size` table, or the child widget's own `size_hint`), not in where a line ends.
/// Keeping the arithmetic in one place is what stops the two from disagreeing about which
/// children share a line — the previous copies had already drifted in whether the gap was
/// counted in the wrap decision.
///
/// `wrap == false` keeps every child on one line, which is `FlowLayoutConfig::wrap`'s
/// contract; running that case through the same arithmetic keeps a no-wrap layout and a
/// wrapping one in agreement about the line they would have formed.
///
/// An empty line never wraps: a single child wider than the box is placed where it is and
/// allowed to overflow, which is the documented behaviour of both layouts.
pub(crate) fn break_lines(
    sizes: &[Size],
    direction: WrapDirection,
    spacing: i32,
    available: i32,
    wrap: bool,
) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = Vec::new();
    // The running extent of the current line, kept instead of re-summing the line for every
    // child: the previous form was O(n²) in the length of a run (each candidate re-walked the
    // items already on the line), which is the common shape of a wrapped toolbar.
    let mut used = 0i32;
    let mut count = 0usize;
    for (index, size) in sizes.iter().enumerate() {
        let candidate = main_extent(*size, direction);
        // `count > 0` already keeps the first item on its line; the removed `used > 0` guard made a
        // line of zero-extent items unbreakable, so the next oversized item shared it and landed on
        // a different cross position than the same run without the invisible items.
        let needs_wrap = wrap && count > 0 && used + spacing + candidate > available;
        if needs_wrap {
            lines.push(Vec::new());
            used = 0;
            count = 0;
        }
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        lines
            .last_mut()
            .expect("a line was just pushed above, so the list is non-empty")
            .push(index);
        used += candidate + if count > 0 { spacing } else { 0 };
        count += 1;
    }
    lines
}

/// Places every child on the lines [`break_lines`] produced.
///
/// The returned vector is indexed by the child's position in `sizes`, not by paint order, so a
/// caller can zip it with its own child list. Only the caller knows whether a line's extent is
/// the one the wrap decision assumed; both layouts charge `spacing` when breaking and this
/// function charges the same `spacing`, with `SpaceBetween`/`SpaceAround` adding only what is
/// left over.
pub(crate) fn place_lines(
    sizes: &[Size],
    lines: &[Vec<usize>],
    direction: WrapDirection,
    alignment: WrapAlignment,
    spacing: i32,
    run_spacing: i32,
    content: Rect,
) -> Vec<Rect> {
    let mut rects = vec![Rect::new(content.x, content.y, 0, 0); sizes.len()];
    if sizes.is_empty() {
        return rects;
    }
    let horizontal = direction == WrapDirection::Horizontal;
    let (available_main, available_cross) = if horizontal {
        (content.width as i32, content.height as i32)
    } else {
        (content.height as i32, content.width as i32)
    };
    let line_cross: Vec<i32> = lines
        .iter()
        .map(|line| {
            line.iter().map(|&index| cross_extent(sizes[index], direction)).max().unwrap_or(0)
        })
        .collect();

    // The block's own footprint on the cross axis: every line's tallest member plus the gap
    // between lines. A single short line in a tall box leaves most of the box empty, not "the
    // block is as tall as the box", so the alignment offsets are measured from this.
    let used_cross: i32 =
        line_cross.iter().sum::<i32>() + (lines.len() as i32 - 1).max(0) * run_spacing;

    if horizontal {
        // Clamping the block to the box keeps the centred case inside it; the end-aligned case
        // still lands flush with the trailing edge when the content overflows.
        let total_cross = used_cross.min(available_cross);
        let mut cursor_cross = content.y
            + match alignment {
                WrapAlignment::Start => 0,
                WrapAlignment::Center => (available_cross - total_cross).max(0) / 2,
                WrapAlignment::End => (available_cross - total_cross).max(0),
                _ => 0,
            };

        for (line_index, line) in lines.iter().enumerate() {
            let sizes_sum: i32 = line.iter().map(|&index| sizes[index].width as i32).sum::<i32>();
            let line_total = sizes_sum + (line.len() as i32 - 1).max(0) * spacing;
            let max_cross = line_cross[line_index];
            let spacing = spacing_for(alignment, available_main, sizes_sum, line.len(), spacing);
            let start_main = match alignment {
                WrapAlignment::Start => content.x,
                WrapAlignment::Center => content.x + (available_main - line_total).max(0) / 2,
                WrapAlignment::End => content.x + (available_main - line_total).max(0),
                WrapAlignment::SpaceBetween => content.x,
                // `SpaceAround` gives every item an equal share of the leftover space, so half
                // a slot belongs before the first one. The slack is measured from what will be
                // painted (the per-item gaps included), so truncating the gap cannot push the
                // run off centre.
                WrapAlignment::SpaceAround => {
                    let painted = sizes_sum + (line.len() as i32 - 1) * spacing.gap;
                    content.x + (available_main - painted).max(0) / 2
                }
            };

            let mut cursor_main = start_main;
            for (slot, &index) in line.iter().enumerate() {
                if slot > 0 {
                    cursor_main += spacing.gap;
                }
                let size = sizes[index];
                rects[index] = Rect::new(cursor_main, cursor_cross, size.width, size.height);
                cursor_main += size.width as i32;
            }
            cursor_cross += max_cross + run_spacing;
        }
    } else {
        let total_cross = used_cross;
        let mut cursor_cross = content.x
            + match alignment {
                WrapAlignment::Start => 0,
                WrapAlignment::Center => (available_cross - total_cross).max(0) / 2,
                WrapAlignment::End => (available_cross - total_cross).max(0),
                _ => 0,
            };

        for (line_index, line) in lines.iter().enumerate() {
            let sizes_sum: i32 = line.iter().map(|&index| sizes[index].height as i32).sum::<i32>();
            let line_total = sizes_sum + (line.len() as i32 - 1).max(0) * spacing;
            let max_cross = line_cross[line_index];
            let spacing = spacing_for(alignment, available_main, sizes_sum, line.len(), spacing);
            let start_main = match alignment {
                WrapAlignment::Start => content.y,
                WrapAlignment::Center => content.y + (available_main - line_total).max(0) / 2,
                WrapAlignment::End => content.y + (available_main - line_total).max(0),
                WrapAlignment::SpaceBetween => content.y,
                WrapAlignment::SpaceAround => {
                    let painted = sizes_sum + (line.len() as i32 - 1) * spacing.gap;
                    content.y + (available_main - painted).max(0) / 2
                }
            };

            let mut cursor_main = start_main;
            for (slot, &index) in line.iter().enumerate() {
                if slot > 0 {
                    cursor_main += spacing.gap;
                }
                let size = sizes[index];
                rects[index] = Rect::new(cursor_cross, cursor_main, size.width, size.height);
                cursor_main += size.height as i32;
            }
            cursor_cross += max_cross + run_spacing;
        }
    }

    rects
}

impl WrapLayout {
    /// Create a new wrap layout with the given parameters.
    pub fn new(
        direction: WrapDirection,
        alignment: WrapAlignment,
        spacing: i32,
        padding: i32,
    ) -> Self {
        // S-47: negative padding is undefined, so reject it at construction. Without this
        // `content_rect` reached `2 * padding as u32`, which overflowed in debug builds.
        let padding = padding.max(0);
        Self { direction, alignment, spacing, run_spacing: None, padding, children: Vec::new() }
    }

    /// Sets the gap between wrap lines, independently of the gap between items.
    ///
    /// `None` (the default) means the item spacing is used on both axes, which is the
    /// behaviour this layout had before the two were separable.
    pub fn set_run_spacing(&mut self, spacing: Option<i32>) {
        self.run_spacing = spacing;
    }

    /// The gap between wrap lines: the configured run spacing, or the item spacing.
    pub fn run_spacing(&self) -> i32 {
        self.run_spacing.unwrap_or(self.spacing)
    }

    /// Create a wrap layout with default settings.
    pub fn new_default() -> Self {
        Self::new(WrapDirection::Horizontal, WrapAlignment::Start, 8, 8)
    }

    /// Returns the number of children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Returns the direction.
    pub fn direction(&self) -> WrapDirection {
        self.direction
    }

    /// Returns the alignment.
    pub fn alignment(&self) -> WrapAlignment {
        self.alignment
    }

    /// Returns the spacing.
    pub fn spacing(&self) -> i32 {
        self.spacing
    }

    /// Returns the padding.
    pub fn padding(&self) -> i32 {
        self.padding
    }

    /// Set the size hint for a child widget (call before update).
    pub fn set_child_size(&mut self, widget_id: ObjectId, size: Size) {
        if let Some(child) = self.children.iter_mut().find(|c| c.widget_id == widget_id) {
            child.size = size;
        }
    }

    fn content_rect(&self, outer: Rect) -> Rect {
        let pad = self.padding.max(0);
        Rect::new(
            outer.x.saturating_add(pad),
            outer.y.saturating_add(pad),
            outer.width.saturating_sub(2 * pad as u32),
            outer.height.saturating_sub(2 * pad as u32),
        )
    }

    /// Compute rects for all children within the content area.
    ///
    /// The line breaking and placement live in [`break_lines`] and [`place_lines`], shared with
    /// [`FlowLayout`](crate::layout::FlowLayout); this method only converts the layout's child
    /// table into the size slice those functions take and zips the ids back on.
    fn compute_rects(&self, content: Rect) -> Vec<(ObjectId, Rect)> {
        let sizes: Vec<Size> = self.children.iter().map(|child| child.size).collect();
        let available_main = match self.direction {
            WrapDirection::Horizontal => content.width,
            WrapDirection::Vertical => content.height,
        } as i32;
        let lines = break_lines(&sizes, self.direction, self.spacing, available_main, true);
        let rects = place_lines(
            &sizes,
            &lines,
            self.direction,
            self.alignment,
            self.spacing,
            self.run_spacing(),
            content,
        );
        self.children.iter().zip(rects).map(|(child, rect)| (child.widget_id, rect)).collect()
    }
}

impl Layout for WrapLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        self.children.push(WrapChild { widget_id, size: Size::new(0, 0) });
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|c| c.widget_id != widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children.iter().map(|c| c.widget_id).collect()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.children.iter().any(|c| c.widget_id == id)
    }

    fn clear(&mut self) {
        self.children.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let content = self.content_rect(rect);
        if content.width == 0 || content.height == 0 {
            return;
        }
        let results = self.compute_rects(content);
        for (wid, child_rect) in results {
            widgets(wid, child_rect);
        }
    }

    /// Lays the children out from their own hints (LAY-N01).
    ///
    /// [`update`](Self::update) can only be *told* a child's size through
    /// [`set_child_size`](Self::set_child_size); this path asks each child instead. The fallback
    /// is the size the layout was already given, so a caller that has migrated only some of its
    /// children keeps the geometry of the ones it has not described (see
    /// [`ChildInfo::resolved_size`](crate::layout::ChildInfo::resolved_size)).
    ///
    /// A hint's preferred extent is the size the child would like, and this layout places each
    /// child at that size — a *wrapping* container does not stretch its items to the line, which
    /// is `Wrap`'s contract in Flutter and the behaviour every existing caller of `update` gets.
    fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect)) {
        let sized: Vec<WrapChild> = self
            .children
            .iter()
            .enumerate()
            .map(|(index, child)| WrapChild {
                widget_id: child.widget_id,
                size: match ChildInfo::find_at(children, index, child.widget_id) {
                    Some(info) => info.resolved_size(child.size),
                    None => child.size,
                },
            })
            .collect();
        // Reuse `compute_rects` rather than a second solver: scaling, alignment and the line
        // arithmetic must stay in one place (see the module's `break_lines` note).
        let sized = WrapLayout {
            direction: self.direction,
            alignment: self.alignment,
            spacing: self.spacing,
            run_spacing: self.run_spacing,
            padding: self.padding,
            children: sized,
        };
        let content = sized.content_rect(rect);
        if content.width == 0 || content.height == 0 {
            return;
        }
        for (wid, child_rect) in sized.compute_rects(content) {
            out(wid, child_rect);
        }
    }

    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
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
        // Rounded, not truncated: `FlexLayout`, `BoxLayout`, `BorderLayout` and `FlowLayout` all
        // round the same `max(layout_scale, font_scale)` product. Truncating made a 3 px gap 5 px
        // in a flex row and 4 px in a wrap row at 1.5x, so the two containers disagreed about the
        // same device by one pixel per gap.
        let scaled_padding = (self.padding.max(0) as f32 * scale).max(0.0).round() as i32;

        // Use scaled spacing inside the content rect by temporarily wrapping.
        let scaled = WrapLayout {
            direction: self.direction,
            alignment: self.alignment,
            spacing: (self.spacing as f32 * scale).round() as i32,
            run_spacing: self.run_spacing.map(|spacing| (spacing as f32 * scale).round() as i32),
            padding: scaled_padding,
            children: self.children.clone(),
        };

        let content = Rect::new(
            rect.x.saturating_add(scaled_padding),
            rect.y.saturating_add(scaled_padding),
            rect.width.saturating_sub(2 * scaled_padding as u32),
            rect.height.saturating_sub(2 * scaled_padding as u32),
        );

        if content.width == 0 || content.height == 0 {
            return;
        }

        let results = scaled.compute_rects(content);
        for (wid, child_rect) in results {
            // Same minimum-touch growth the flex and box layouts apply, so a wrapped row and a
            // flowing row address their controls identically.
            widgets(
                wid,
                crate::layout::types::grow_to_min_touch_size(child_rect, context.min_touch_size),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;
    use crate::layout::Hints;

    #[test]
    fn wrap_layout_default_creates_empty() {
        let layout = WrapLayout::default();
        assert_eq!(layout.child_count(), 0);
    }

    #[test]
    fn wrap_layout_add_and_remove() {
        let mut layout = WrapLayout::default();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        assert_eq!(layout.child_count(), 2);
        assert!(layout.has_child(1));

        layout.remove_widget(1);
        assert_eq!(layout.child_count(), 1);
        assert!(!layout.has_child(1));
        assert!(layout.has_child(2));
    }

    #[test]
    fn wrap_layout_child_ids() {
        let mut layout = WrapLayout::default();
        layout.add_widget(10, 0);
        layout.add_widget(20, 0);
        let ids = layout.child_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&10));
        assert!(ids.contains(&20));
    }

    #[test]
    fn wrap_layout_clear() {
        let mut layout = WrapLayout::default();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        assert_eq!(layout.child_count(), 2);
        layout.clear();
        assert_eq!(layout.child_count(), 0);
    }

    #[test]
    fn wrap_layout_horizontal_single_row() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 0, 0);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_size(1, Size::new(40, 20));
        layout.set_child_size(2, Size::new(40, 20));

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Single row: item1 at (0,0), item2 at (40,0)
        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 40, 20)));
        assert_eq!(rects.get(&2), Some(&Rect::new(40, 0, 40, 20)));
    }

    #[test]
    fn wrap_layout_horizontal_wraps_to_next_row() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 4, 0);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_widget(3, 0);
        layout.set_child_size(1, Size::new(60, 20));
        layout.set_child_size(2, Size::new(60, 20));
        layout.set_child_size(3, Size::new(60, 20));

        let mut rects = HashMap::new();
        // Width 100: item1(60) fits, item2(60+4=64) doesn't => wraps
        // Row1: item1 at (0,0)
        // Row2: item2 at (0,24), item3 at (60+4=64) doesn't fit => wraps
        // Row3: item3 at (0,48)
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 60, 20)));
        assert_eq!(rects.get(&2), Some(&Rect::new(0, 24, 60, 20)));
        assert_eq!(rects.get(&3), Some(&Rect::new(0, 48, 60, 20)));
    }

    #[test]
    fn wrap_layout_horizontal_two_per_row() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 0, 0);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_widget(3, 0);
        layout.add_widget(4, 0);
        layout.set_child_size(1, Size::new(40, 20));
        layout.set_child_size(2, Size::new(40, 20));
        layout.set_child_size(3, Size::new(40, 20));
        layout.set_child_size(4, Size::new(40, 20));

        let mut rects = HashMap::new();
        // Width 80: two items of 40 fit per row
        // Row1: (0,0) and (40,0)
        // Row2: (0,20) and (40,20)
        layout.update(Rect::new(0, 0, 80, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 40, 20)));
        assert_eq!(rects.get(&2), Some(&Rect::new(40, 0, 40, 20)));
        assert_eq!(rects.get(&3), Some(&Rect::new(0, 20, 40, 20)));
        assert_eq!(rects.get(&4), Some(&Rect::new(40, 20, 40, 20)));
    }

    #[test]
    fn wrap_layout_horizontal_center_alignment() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Center, 0, 0);
        layout.add_widget(1, 0);
        layout.set_child_size(1, Size::new(40, 20));

        let mut rects = HashMap::new();
        // 40px item in 80x50 container => centered at x=20, y=15 (vertical center too)
        layout.update(Rect::new(0, 0, 80, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(20, 15, 40, 20)));
    }

    #[test]
    fn wrap_layout_horizontal_end_alignment() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::End, 0, 0);
        layout.add_widget(1, 0);
        layout.set_child_size(1, Size::new(40, 20));

        let mut rects = HashMap::new();
        // 40px item in 80x50 container => right-aligned at x=40, y=30 (vertical end too)
        layout.update(Rect::new(0, 0, 80, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(40, 30, 40, 20)));
    }

    #[test]
    fn wrap_layout_padding_applied() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 0, 10);
        layout.add_widget(1, 0);
        layout.set_child_size(1, Size::new(40, 20));

        let mut rects = HashMap::new();
        // Padding 10: content starts at (10,10)
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(10, 10, 40, 20)));
    }

    /// S-47: a negative padding is rejected at construction, so `content_rect` never reaches
    /// `2 * padding as u32` with a negative value (which overflowed in debug builds).
    #[test]
    fn wrap_layout_rejects_negative_padding() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 0, -1);
        layout.add_widget(1, 0);
        layout.set_child_size(1, Size::new(40, 20));

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Padding clamped to 0: the child starts at the content origin.
        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 40, 20)));
    }

    #[test]
    fn wrap_layout_vertical_single_column() {
        let mut layout = WrapLayout::new(WrapDirection::Vertical, WrapAlignment::Start, 0, 0);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_size(1, Size::new(30, 40));
        layout.set_child_size(2, Size::new(30, 40));

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 150), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Single column: item1 at (0,0), item2 at (0,40)
        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 30, 40)));
        assert_eq!(rects.get(&2), Some(&Rect::new(0, 40, 30, 40)));
    }

    #[test]
    fn wrap_layout_vertical_wraps_to_next_column() {
        let mut layout = WrapLayout::new(WrapDirection::Vertical, WrapAlignment::Start, 4, 0);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_widget(3, 0);
        layout.set_child_size(1, Size::new(30, 50));
        layout.set_child_size(2, Size::new(30, 50));
        layout.set_child_size(3, Size::new(30, 50));

        let mut rects = HashMap::new();
        // Height 60: item1(50) fits, item2(50+4=54) doesn't => wraps
        // Col1: item1 at (0,0)
        // Col2: item2 at (34,0), item3 at (34,54) doesn't fit => wraps
        // Col3: item3 at (68,0)
        layout.update(Rect::new(0, 0, 200, 60), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 30, 50)));
        assert_eq!(rects.get(&2), Some(&Rect::new(34, 0, 30, 50)));
        assert_eq!(rects.get(&3), Some(&Rect::new(68, 0, 30, 50)));
    }

    #[test]
    fn wrap_layout_update_with_context_scales_spacing() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 8, 4);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_size(1, Size::new(30, 20));
        layout.set_child_size(2, Size::new(30, 20));

        let context = LayoutContext { layout_scale: 2.0, ..LayoutContext::default() };

        let mut rects = HashMap::new();
        layout.update_with_context(Rect::new(0, 0, 200, 100), &context, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Scale=2.0: padding=8, content starts at (8,8), width=200-16=184, and the items are
        // laid at (8,8) and (54,8) at 30x20 — then each is grown to the context's 32x32 minimum
        // touch target, centred on the slot it was given. Asserting the un-grown size would mean
        // the layout ignoring `min_touch_size`, which no layout should.
        let min = context.min_touch_size;
        let first = rects.get(&1).copied().expect("item 1 was laid out");
        let second = rects.get(&2).copied().expect("item 2 was laid out");
        assert_eq!(first.width, min.width.max(30));
        assert_eq!(first.height, min.height.max(20));
        assert_eq!(first.x, 8 - (first.width as i32 - 30) / 2);
        assert_eq!(first.y, 8 - (first.height as i32 - 20) / 2);
        // The sibling keeps its own slot: growing one child must not shift the others.
        assert_eq!(second.x, 54 - (second.width as i32 - 30) / 2);
    }

    /// A wrap layout scales its gap the same way a flex row does: rounding, not truncation.
    ///
    /// `FlexLayout`, `BoxLayout`, `BorderLayout` and `FlowLayout` all round the
    /// `max(layout_scale, font_scale)` product; `WrapLayout` truncated, so at 1.5× a 3 px gap was
    /// 5 px in a flex row and 4 px in a wrap row on the same device.
    #[test]
    fn wrap_layout_rounds_its_scaled_gap_like_a_flex_row() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 3, 3);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_size(1, Size::new(10, 10));
        layout.set_child_size(2, Size::new(10, 10));

        let context =
            LayoutContext { layout_scale: 1.5, font_scale: 1.0, min_touch_size: Size::new(0, 0) };
        let mut rects = HashMap::new();
        layout.update_with_context(Rect::new(0, 0, 200, 100), &context, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // padding 3 * 1.5 = 4.5 -> 5 (truncation gave 4); gap 3 * 1.5 = 4.5 -> 5 (truncation
        // gave 4), so the second child starts at 5 + 10 + 5 = 20.
        assert_eq!(rects.get(&1).expect("item 1 was laid out").x, 5);
        assert_eq!(rects.get(&2).expect("item 2 was laid out").x, 20);
    }

    #[test]
    fn wrap_layout_empty_rect_yields_no_children() {
        let mut layout = WrapLayout::default();
        layout.add_widget(1, 0);
        layout.set_child_size(1, Size::new(100, 100));

        let mut called = false;
        layout.update(Rect::new(0, 0, 0, 0), &mut |_id, _rect| {
            called = true;
        });

        assert!(!called);
    }

    /// `SpaceAround` must leave **half** a slot before the first item and after the last,
    /// unlike `SpaceBetween` which starts flush. They used to render identically (minus
    /// one gap), which made the alignment's name a lie.
    #[test]
    fn space_around_leaves_a_half_gap_at_each_end() {
        /// The alignment arithmetic is asserted on the raw slots, not on the rects that
        /// `update` finally emits: `update` grows every child to the device's minimum touch
        /// target, which would move the x coordinates and hide the very gap being tested.
        fn x_positions(alignment: WrapAlignment) -> Vec<i32> {
            let mut layout = WrapLayout::new(WrapDirection::Horizontal, alignment, 0, 0);
            for id in [1, 2, 3] {
                layout.add_widget(id, 0);
                layout.set_child_size(id, Size::new(20, 10));
            }
            layout
                .compute_rects(layout.content_rect(Rect::new(0, 0, 160, 40)))
                .into_iter()
                .map(|(_, rect)| rect.x)
                .collect()
        }

        let around = x_positions(WrapAlignment::SpaceAround);
        let between = x_positions(WrapAlignment::SpaceBetween);

        // 160 wide, 3 items of 20, no configured spacing: 60 is used by the items and the
        // remaining 100 is what each alignment distributes.
        //   SpaceBetween: 2 shares of 50 -> the items sit at 0 / 70 / 140 (20 + 50).
        //   SpaceAround:  3 slots of 33 -> a 33 gap between items (so a 53 pitch); the run
        //                 measures 20*3 + 33*2 = 126 and is centred in the 160 box.
        assert_eq!(between, vec![0, 70, 140], "SpaceBetween starts flush");
        let pitch = around[1] - around[0];
        assert_eq!(pitch, 53, "a whole slot between items");
        assert_eq!(around[2] - around[1], pitch);
        // Centred: the same space before the run as after it.
        let lead = around[0];
        let trail = 160 - (around[2] + 20);
        assert_eq!(lead, trail, "the run is centred, not left-padded");
        assert!(lead > 0, "SpaceAround leaves a gap before the first item");
        assert_ne!(around, between, "SpaceAround and SpaceBetween are not the same alignment");
    }

    /// A `SpaceBetween`/`SpaceAround` run must stay **inside** its box: the wrap decision
    /// charges every line the configured spacing, so the painted gap has to start from
    /// that same base rather than redistributing the whole leftover.
    #[test]
    fn spacing_alignments_never_overflow_the_line() {
        for alignment in [WrapAlignment::SpaceBetween, WrapAlignment::SpaceAround] {
            // `spacing = 10`, `padding = 0`: the wrap decision charges each of the two
            // gaps 10, leaving 80 to distribute, and the painted gap must agree.
            let mut layout = WrapLayout::new(WrapDirection::Horizontal, alignment, 10, 0);
            for id in [1, 2, 3] {
                layout.add_widget(id, 0);
                layout.set_child_size(id, Size::new(30, 10));
            }
            let out = layout.compute_rects(layout.content_rect(Rect::new(0, 0, 160, 40)));
            assert_eq!(
                out.iter().map(|(_, r)| r.width).collect::<Vec<_>>(),
                vec![30, 30, 30],
                "the child sizes are what the test set"
            );
            assert_eq!(out.len(), 3, "160 must hold three 30px items plus two 10px gaps");
            let last = out.last().expect("three items").1;
            assert!(
                last.x >= 0 && last.x + last.width as i32 <= 160,
                "{alignment:?} ran off the line: last item at {}..{}",
                last.x,
                last.x + last.width as i32
            );
        }
    }

    /// A one-item line has no pair to separate and no slot to distribute, so the item
    /// keeps its normal slot; only `SpaceAround` centres it, which is what its name
    /// promises. Critically it must not be **pushed** by a gap computed as if there were
    /// two items, which is what a naive `leftover / (n + 1)` leading gap did.
    #[test]
    fn a_single_item_line_is_only_centred_not_pushed() {
        for alignment in [WrapAlignment::SpaceBetween, WrapAlignment::SpaceAround] {
            let mut layout = WrapLayout::new(WrapDirection::Horizontal, alignment, 10, 0);
            layout.add_widget(1, 0);
            layout.set_child_size(1, Size::new(30, 10));
            let out = layout.compute_rects(layout.content_rect(Rect::new(0, 0, 160, 40)));
            let x = out[0].1.x;
            let expected = if alignment == WrapAlignment::SpaceAround { (160 - 30) / 2 } else { 0 };
            assert_eq!(x, expected, "{alignment:?} placed a lone item at x={x}");
        }
    }

    /// The vertical path mirrors the horizontal one: `SpaceAround` is symmetric around
    /// the run in both axes.
    #[test]
    fn space_around_is_symmetric_in_the_vertical_direction() {
        let mut layout = WrapLayout::new(WrapDirection::Vertical, WrapAlignment::SpaceAround, 0, 0);
        for id in [1, 2, 3] {
            layout.add_widget(id, 0);
            layout.set_child_size(id, Size::new(10, 20));
        }
        let out = layout.compute_rects(layout.content_rect(Rect::new(0, 0, 40, 160)));

        // The run is 3 items of 20 separated by 33, so 126 of the 160 is painted and the
        // 34 that is left is split evenly: 17 above and 17 below.
        assert_eq!(out[0].1.y, 17, "the run is centred vertically");
        assert_eq!(160 - (out[2].1.y + 20), 17, "and symmetrically so");
    }

    /// `run_spacing` separates wrapped rows without changing the gap inside a row.
    #[test]
    fn run_spacing_separates_rows_without_widening_the_items() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 0, 0);
        for id in 1..=4 {
            layout.add_widget(id, 0);
            layout.set_child_size(id, Size::new(40, 10));
        }
        layout.set_run_spacing(Some(20));
        assert_eq!(layout.run_spacing(), 20);

        let mut placed = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            placed.insert(id, rect);
        });

        // 40 + 40 fits; the third item wraps, so rows are {1,2} and {3,4}.
        assert_eq!(placed[&1].y, 0);
        assert_eq!(placed[&2].y, 0);
        assert_eq!(placed[&2].x, 40, "the in-row gap is still the item spacing (0)");
        assert_eq!(placed[&3].y, 30, "10 px row height plus 20 px run spacing");
        assert_eq!(placed[&4].y, 30);
    }

    /// LAY-N01: the hint-aware path places children at the size they ask for, and wraps on
    /// that size — the question `update` can only answer when told through `set_child_size`.
    #[test]
    fn arrange_lays_children_out_from_their_hints() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 0, 0);
        for id in 1..=3 {
            layout.add_widget(id, 0);
        }
        let children: Vec<ChildInfo> =
            (1..=3).map(|id| ChildInfo::new(id, Hints::fixed(40, 10))).collect();

        let mut placed = HashMap::new();
        layout.arrange(Rect::new(0, 0, 90, 100), &children, &mut |id, rect| {
            placed.insert(id, rect);
        });

        // 40 + 40 fits in 90; the third child wraps to the next row.
        assert_eq!(placed[&1], Rect::new(0, 0, 40, 10));
        assert_eq!(placed[&2], Rect::new(40, 0, 40, 10));
        assert_eq!(placed[&3], Rect::new(0, 10, 40, 10));
    }

    /// A partially-migrated caller must not lose the children it has not described: an
    /// unconstrained hint means "no opinion" and falls back to the registered size.
    #[test]
    fn arrange_falls_back_to_the_registered_size() {
        let mut layout = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 0, 0);
        layout.add_widget(1, 0);
        layout.set_child_size(1, Size::new(30, 12));
        let children = [ChildInfo::new(1, Hints::default())];

        let mut placed = HashMap::new();
        layout.arrange(Rect::new(0, 0, 100, 50), &children, &mut |id, rect| {
            placed.insert(id, rect);
        });

        assert_eq!(placed[&1], Rect::new(0, 0, 30, 12));
    }
}
