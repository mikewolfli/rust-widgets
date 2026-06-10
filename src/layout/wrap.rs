//! Wrap layout manager — auto-wrap flow layout that places items in rows or columns,
//! breaking to the next line/column when the available space is exhausted.
use super::{Layout, LayoutContext};
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

impl WrapLayout {
    /// Create a new wrap layout with the given parameters.
    pub fn new(
        direction: WrapDirection,
        alignment: WrapAlignment,
        spacing: i32,
        padding: i32,
    ) -> Self {
        Self { direction, alignment, spacing, padding, children: Vec::new() }
    }

    /// Create a wrap layout with default settings.
    pub fn default() -> Self {
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
        let pad = self.padding;
        Rect::new(
            outer.x + pad,
            outer.y + pad,
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
                lines.push(std::mem::take(&mut current_line));
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
        let mut row_height: Vec<i32> = lines
            .iter()
            .map(|line| line.iter().map(|(_, s)| s.height as i32).max().unwrap_or(0))
            .collect();

        let total_height: i32 =
            row_height.iter().sum::<i32>() + (lines.len() as i32 - 1).max(0) * gap;

        // Apply vertical alignment to the whole content.
        let avail_h = content.height as i32;
        let start_y = content.y
            + match self.alignment {
                WrapAlignment::Start => 0,
                WrapAlignment::Center => (avail_h - total_height).max(0) / 2,
                WrapAlignment::End => (avail_h - total_height).max(0),
                _ => 0,
            };

        let mut results = Vec::new();
        let mut cur_y = start_y;

        for line in &lines {
            let line_total_w: i32 = line.iter().map(|(_, s)| s.width as i32).sum::<i32>()
                + (line.len() as i32 - 1).max(0) * gap;
            let max_h = row_height[0];
            row_height.remove(0);

            let start_x = match self.alignment {
                WrapAlignment::Start => content.x,
                WrapAlignment::Center => content.x + (avail_w - line_total_w).max(0) / 2,
                WrapAlignment::End => content.x + (avail_w - line_total_w).max(0),
                WrapAlignment::SpaceBetween => content.x,
                WrapAlignment::SpaceAround => content.x,
            };

            let spacing = if line.len() > 1 {
                match self.alignment {
                    WrapAlignment::SpaceBetween => {
                        (avail_w - line_total_w) / (line.len() as i32 - 1)
                    }
                    WrapAlignment::SpaceAround => (avail_w - line_total_w) / (line.len() as i32),
                    _ => gap,
                }
            } else {
                gap
            };

            let mut cur_x = start_x;
            for (i, (wid, sz)) in line.iter().enumerate() {
                if i > 0 {
                    cur_x += spacing;
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
                cols.push(std::mem::take(&mut current_col));
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

        let mut col_width: Vec<i32> = cols
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

        for col in &cols {
            let col_total_h: i32 = col.iter().map(|(_, s)| s.height as i32).sum::<i32>()
                + (col.len() as i32 - 1).max(0) * gap;
            let max_w = col_width[0];
            col_width.remove(0);

            let start_y = match self.alignment {
                WrapAlignment::Start => content.y,
                WrapAlignment::Center => content.y + (avail_h - col_total_h).max(0) / 2,
                WrapAlignment::End => content.y + (avail_h - col_total_h).max(0),
                WrapAlignment::SpaceBetween => content.y,
                WrapAlignment::SpaceAround => content.y,
            };

            let spacing = if col.len() > 1 {
                match self.alignment {
                    WrapAlignment::SpaceBetween => (avail_h - col_total_h) / (col.len() as i32 - 1),
                    WrapAlignment::SpaceAround => (avail_h - col_total_h) / (col.len() as i32),
                    _ => gap,
                }
            } else {
                gap
            };

            let mut cur_y = start_y;
            for (i, (wid, sz)) in col.iter().enumerate() {
                if i > 0 {
                    cur_y += spacing;
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
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
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
        let scale = context.layout_scale;
        let scaled_padding = (self.padding as f32 * scale) as i32;

        // Use scaled spacing inside the content rect by temporarily wrapping.
        let scaled = WrapLayout {
            direction: self.direction,
            alignment: self.alignment,
            spacing: (self.spacing as f32 * scale) as i32,
            padding: scaled_padding,
            children: self.children.clone(),
        };

        let content = Rect::new(
            rect.x + scaled_padding,
            rect.y + scaled_padding,
            rect.width.saturating_sub(2 * scaled_padding as u32),
            rect.height.saturating_sub(2 * scaled_padding as u32),
        );

        if content.width == 0 || content.height == 0 {
            return;
        }

        let results = scaled.compute_rects(content);
        for (wid, child_rect) in results {
            widgets(wid, child_rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
        // Padding 10: content starts at (10,10)
        layout.update(Rect::new(0, 0, 100, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.get(&1), Some(&Rect::new(10, 10, 40, 20)));
    }

    #[test]
    fn wrap_layout_vertical_single_column() {
        let mut layout = WrapLayout::new(WrapDirection::Vertical, WrapAlignment::Start, 0, 0);
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.set_child_size(1, Size::new(30, 40));
        layout.set_child_size(2, Size::new(30, 40));

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
        layout.update_with_context(Rect::new(0, 0, 200, 100), &context, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Scale=2.0: padding=8, content starts at (8,8), width=200-16=184
        // Item1 at (8,8), item2 at (8+30+16(spacing)=54, 8)
        assert_eq!(rects.get(&1), Some(&Rect::new(8, 8, 30, 20)));
        assert_eq!(rects.get(&2), Some(&Rect::new(54, 8, 30, 20)));
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
}
