//! Flex layout manager — CSS Flexbox-style layout with grow, shrink, and alignment.
use super::{Layout, LayoutContext};
use crate::core::{ObjectId, Rect, Size};

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
#[derive(Debug)]
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
            padding,
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
    fn compute_main_sizes(&self, available_main: i32) -> (Vec<i32>, f32, i32) {
        let count = self.items.len();
        if count == 0 {
            return (Vec::new(), 0.0, 0);
        }

        // Sum child intrinsic sizes and flex grow factors.
        let mut intrinsic_main: Vec<i32> = Vec::with_capacity(count);
        let mut total_flex_grow: f32 = 0.0;
        let mut total_intrinsic: i32 = 0;

        for (i, item) in self.items.iter().enumerate() {
            let sz = self.child_sizes.get(i).copied().unwrap_or(Size::new(0, 0));
            let main = if self.is_row() { sz.width as i32 } else { sz.height as i32 };
            let main = main.max(if self.is_row() {
                item.min_size.width as i32
            } else {
                item.min_size.height as i32
            });
            intrinsic_main.push(main);
            total_flex_grow += item.flex_grow;
            total_intrinsic += main;
        }

        let gaps = (count.saturating_sub(1)) as i32 * self.gap;
        let remaining = available_main - total_intrinsic - gaps;

        let mut main_sizes: Vec<i32> = Vec::with_capacity(count);

        if remaining > 0 && total_flex_grow > 0.0 {
            // Distribute surplus according to flex-grow.
            let mut distributed = 0i32;
            for (i, item) in self.items.iter().enumerate() {
                let extra = if total_flex_grow > 0.0 {
                    ((remaining as f32) * (item.flex_grow / total_flex_grow)).round() as i32
                } else {
                    0
                };
                let size = intrinsic_main[i] + extra;
                let max_main = if self.is_row() {
                    if item.max_size.width > 0 {
                        item.max_size.width as i32
                    } else {
                        i32::MAX
                    }
                } else {
                    if item.max_size.height > 0 {
                        item.max_size.height as i32
                    } else {
                        i32::MAX
                    }
                };
                let size = size.min(max_main);
                main_sizes.push(size);
                distributed += size - intrinsic_main[i];
            }
            // Adjust if rounding caused leftover.
            let leftover = remaining - distributed;
            if leftover > 0 && !main_sizes.is_empty() {
                main_sizes[count - 1] += leftover;
            }
        } else if remaining < 0 {
            // Shrink items proportionally to flex-shrink.
            let deficit = -remaining;
            let total_flex_shrink: f32 = self.items.iter().map(|i| i.flex_shrink).sum();
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
            }
            // If we couldn't shrink enough, cap at available
            let actual_shrunk = total_intrinsic - main_sizes.iter().sum::<i32>() - gaps;
            if actual_shrunk < deficit {
                // Distribute the remaining deficit
                let remaining_deficit = deficit - actual_shrunk;
                for s in main_sizes.iter_mut().rev() {
                    if remaining_deficit <= 0 {
                        break;
                    }
                    let possible = *s;
                    let cut = possible.min(remaining_deficit);
                    *s -= cut;
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
    ) -> Vec<i32> {
        let count = main_sizes.len();
        if count == 0 {
            return Vec::new();
        }

        let leftover = available_main - total_used;
        let mut positions = Vec::with_capacity(count);

        let (first_gap, inter_gap) = match self.justify_content {
            JustifyContent::FlexStart | JustifyContent::FlexEnd | JustifyContent::Center => {
                let offset = match self.justify_content {
                    JustifyContent::FlexStart => 0,
                    JustifyContent::FlexEnd => leftover,
                    JustifyContent::Center => leftover / 2,
                    _ => 0,
                };
                (offset, self.gap)
            }
            JustifyContent::SpaceBetween => {
                let gap = if count > 1 { leftover / (count as i32 - 1) } else { 0 };
                (0, gap)
            }
            JustifyContent::SpaceAround => {
                let gap = if count > 0 { leftover / (count as i32) } else { 0 };
                (gap / 2, gap)
            }
            JustifyContent::SpaceEvenly => {
                let gap = if count > 0 { leftover / (count as i32 + 1) } else { 0 };
                (gap, gap)
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
        for (i, _item) in self.items.iter().enumerate() {
            let sz = self.child_sizes.get(i).copied().unwrap_or(Size::new(0, 0));
            let child_cross = if self.is_row() { sz.height as i32 } else { sz.width as i32 };

            let align = self.items[i].align_self.unwrap_or(self.align_items);

            let (cross_start, cross_len) = match align {
                AlignItems::Stretch => (0, cross_size),
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
    fn compute_rects(&self, content_rect: Rect) -> Vec<(Option<ObjectId>, Rect)> {
        if self.items.is_empty() {
            return Vec::new();
        }

        let (available_main, start_main, available_cross, cross_origin) = if self.is_row() {
            (content_rect.width as i32, content_rect.x, content_rect.height as i32, content_rect.y)
        } else {
            (content_rect.height as i32, content_rect.y, content_rect.width as i32, content_rect.x)
        };

        if available_main <= 0 || available_cross <= 0 {
            return Vec::new();
        }

        let (main_sizes, _total_flex_grow, total_used) = self.compute_main_sizes(available_main);

        let main_positions =
            self.justify_positions(&main_sizes, total_used, available_main, start_main);
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
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout for FlexLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
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
        self.items.retain(|item| item.widget_id != Some(widget_id));
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
        let content_rect = Rect::new(
            rect.x + self.padding,
            rect.y + self.padding,
            rect.width.saturating_sub(2 * self.padding as u32),
            rect.height.saturating_sub(2 * self.padding as u32),
        );

        let results = self.compute_rects(content_rect);
        for (widget_id, child_rect) in results {
            if let Some(wid) = widget_id {
                widgets(wid, child_rect);
            }
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

        let content_rect = Rect::new(
            rect.x + scaled_padding,
            rect.y + scaled_padding,
            rect.width.saturating_sub(2 * scaled_padding as u32),
            rect.height.saturating_sub(2 * scaled_padding as u32),
        );

        let results = self.compute_rects(content_rect);
        for (widget_id, child_rect) in results {
            if let Some(wid) = widget_id {
                widgets(wid, child_rect);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0)]);
        layout.update_with_context(Rect::new(0, 0, 200, 60), &context, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // With scale=2.0: padding=20, content area = (20,20) to (180,40)
        // Single flex item fills: x=20, y=20, w=160, h=20
        assert_eq!(rects.get(&1).map(|r| r.x), Some(20));
        assert_eq!(rects.get(&1).map(|r| r.y), Some(20));
        assert_eq!(rects.get(&1).map(|r| r.width), Some(160));
        assert_eq!(rects.get(&1).map(|r| r.height), Some(20));
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

        let mut rects = std::collections::HashMap::new();
        layout.set_child_sizes(vec![Size::new(0, 0), Size::new(0, 0)]);
        layout.update(Rect::new(0, 0, 200, 50), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // RowReverse: item1 at x=100, item2 at x=0 (reversed order)
        assert_eq!(rects.get(&1).map(|r| r.x), Some(100));
        assert_eq!(rects.get(&2).map(|r| r.x), Some(0));
    }
}
