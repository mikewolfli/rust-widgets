// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wrap layout manager — auto-wrap flow layout that places items in rows or columns,
//! breaking to the next line/column when the available space is exhausted.
use super::{Layout, LayoutContext};
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
/// keeps the wrap decision and the drawing in agreement (`layout_horizontal` charges each
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
        Self { direction, alignment, spacing, padding, children: Vec::new() }
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

    /// Layout items horizontally, wrapping to new rows.
    fn layout_horizontal(&self, content: Rect) -> Vec<(ObjectId, Rect)> {
        if self.children.is_empty() {
            return Vec::new();
        }

        let gap = self.spacing;
        let avail_w = content.width as i32;

        // Group children into lines.
        let mut lines: Vec<Vec<(ObjectId, Size)>> = Vec::new();
        let mut current_line: Vec<(ObjectId, Size)> = Vec::new();
        let mut line_width = 0i32;

        for child in &self.children {
            let cw = child.size.width as i32;
            let need = if current_line.is_empty() { cw } else { cw + gap };
            if !current_line.is_empty() && line_width + need > avail_w {
                // Wrap to next line.
                lines.push(core::mem::take(&mut current_line));
                current_line.push((child.widget_id, child.size));
                line_width = cw;
            } else {
                current_line.push((child.widget_id, child.size));
                line_width += need;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // Compute row heights and total used height.
        let row_height: Vec<i32> = lines
            .iter()
            .map(|line| line.iter().map(|(_, s)| s.height as i32).max().unwrap_or(0))
            .collect();

        // The sum of the row heights is the block's height only while it still fits; a
        // single row of ten-pixel items in a two-hundred-pixel box leaves most of the box
        // empty, not "the block is two hundred tall". Clamping keeps the centred case
        // inside the box and leaves the end-aligned case pinned to the bottom.
        let avail_h = content.height as i32;
        let content_used_h: i32 =
            row_height.iter().sum::<i32>() + (lines.len() as i32 - 1).max(0) * gap;
        let total_height: i32 = content_used_h.min(avail_h);

        // Apply vertical alignment to the whole content.
        let start_y = content.y
            + match self.alignment {
                WrapAlignment::Start => 0,
                WrapAlignment::Center => (avail_h - total_height).max(0) / 2,
                WrapAlignment::End => (avail_h - total_height).max(0),
                _ => 0,
            };

        let mut results = Vec::new();
        let mut cur_y = start_y;

        for (line_idx, line) in lines.iter().enumerate() {
            // The wrap decision above charged every line the **configured** spacing, so
            // the drawing must too, with the alignment adding whatever is left over -- see
            // `spacing_between`. Deriving the gap from the alignment alone (the old shape)
            // made `SpaceAround` paint a wider gap than the one the wrap assumed, so the
            // items ran off the trailing edge.
            let sizes_sum_w: i32 = line.iter().map(|(_, s)| s.width as i32).sum::<i32>();
            let line_total_w = sizes_sum_w + (line.len() as i32 - 1).max(0) * gap;
            let max_h = row_height[line_idx];

            let spacing = spacing_for(self.alignment, avail_w, sizes_sum_w, line.len(), gap);
            let start_x = match self.alignment {
                WrapAlignment::Start => content.x,
                WrapAlignment::Center => content.x + (avail_w - line_total_w).max(0) / 2,
                WrapAlignment::End => content.x + (avail_w - line_total_w).max(0),
                WrapAlignment::SpaceBetween => content.x,
                // `SpaceAround` gives every item an equal share of the leftover space, so
                // the leading half-gap is the first thing it must paint. Without this it
                // started flush at `content.x` and was indistinguishable from
                // `SpaceBetween` with fewer gaps -- the items were spread, but nothing was
                // "around" the first one, so the line was not centred in its run.
                //
                // The slack is measured from what will actually be painted (the pieces
                // between the items included), not from `available - used`, so truncating
                // the per-item gap cannot push the run off centre.
                WrapAlignment::SpaceAround => {
                    let painted = sizes_sum_w + (line.len() as i32 - 1) * spacing.gap;
                    content.x + (avail_w - painted).max(0) / 2
                }
            };

            let mut cur_x = start_x;
            for (i, (wid, sz)) in line.iter().enumerate() {
                if i > 0 {
                    cur_x += spacing.gap;
                }
                results.push((*wid, Rect::new(cur_x, cur_y, sz.width, sz.height)));
                cur_x += sz.width as i32;
            }

            cur_y += max_h + gap;
        }

        results
    }

    /// Layout items vertically, wrapping to new columns.
    fn layout_vertical(&self, content: Rect) -> Vec<(ObjectId, Rect)> {
        if self.children.is_empty() {
            return Vec::new();
        }

        let gap = self.spacing;
        let avail_h = content.height as i32;

        // Group children into columns.
        let mut cols: Vec<Vec<(ObjectId, Size)>> = Vec::new();
        let mut current_col: Vec<(ObjectId, Size)> = Vec::new();
        let mut col_height = 0i32;

        for child in &self.children {
            let ch = child.size.height as i32;
            let need = if current_col.is_empty() { ch } else { ch + gap };
            if !current_col.is_empty() && col_height + need > avail_h {
                cols.push(core::mem::take(&mut current_col));
                current_col.push((child.widget_id, child.size));
                col_height = ch;
            } else {
                current_col.push((child.widget_id, child.size));
                col_height += need;
            }
        }
        if !current_col.is_empty() {
            cols.push(current_col);
        }

        let col_width: Vec<i32> = cols
            .iter()
            .map(|col| col.iter().map(|(_, s)| s.width as i32).max().unwrap_or(0))
            .collect();

        let total_width: i32 = col_width.iter().sum::<i32>() + (cols.len() as i32 - 1).max(0) * gap;

        let avail_w = content.width as i32;
        let start_x = content.x
            + match self.alignment {
                WrapAlignment::Start => 0,
                WrapAlignment::Center => (avail_w - total_width).max(0) / 2,
                WrapAlignment::End => (avail_w - total_width).max(0),
                _ => 0,
            };

        let mut results = Vec::new();
        let mut cur_x = start_x;

        for (col_idx, col) in cols.iter().enumerate() {
            // Same shape as the horizontal path: the column is charged the configured
            // spacing when the wrap decision is taken, so the gap painted here is that
            // spacing plus whatever the alignment distributes on top.
            let sizes_sum_h: i32 = col.iter().map(|(_, s)| s.height as i32).sum::<i32>();
            let col_total_h = sizes_sum_h + (col.len() as i32 - 1).max(0) * gap;
            let max_w = col_width[col_idx];

            let spacing = spacing_for(self.alignment, avail_h, sizes_sum_h, col.len(), gap);
            let start_y = match self.alignment {
                WrapAlignment::Start => content.y,
                WrapAlignment::Center => content.y + (avail_h - col_total_h).max(0) / 2,
                WrapAlignment::End => content.y + (avail_h - col_total_h).max(0),
                WrapAlignment::SpaceBetween => content.y,
                // See the horizontal path: `SpaceAround` also centres its run.
                WrapAlignment::SpaceAround => {
                    let painted = sizes_sum_h + (col.len() as i32 - 1) * spacing.gap;
                    content.y + (avail_h - painted).max(0) / 2
                }
            };

            let mut cur_y = start_y;
            for (i, (wid, sz)) in col.iter().enumerate() {
                if i > 0 {
                    cur_y += spacing.gap;
                }
                results.push((*wid, Rect::new(cur_x, cur_y, sz.width, sz.height)));
                cur_y += sz.height as i32;
            }

            cur_x += max_w + gap;
        }

        results
    }

    /// Compute rects for all children within the content area.
    fn compute_rects(&self, content: Rect) -> Vec<(ObjectId, Rect)> {
        match self.direction {
            WrapDirection::Horizontal => self.layout_horizontal(content),
            WrapDirection::Vertical => self.layout_vertical(content),
        }
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
        let scaled_padding = (self.padding.max(0) as f32 * scale).max(0.0) as i32;

        // Use scaled spacing inside the content rect by temporarily wrapping.
        let scaled = WrapLayout {
            direction: self.direction,
            alignment: self.alignment,
            spacing: (self.spacing as f32 * scale) as i32,
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
}
