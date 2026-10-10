// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::Layout;
use crate::compat::{Any, Box, Vec};
use crate::core::{ObjectId, Rect, Size};
use crate::widget::Widget;
use core::fmt;

/// Internal child entry that holds a widget ID and optionally the widget object.
/// Ensures `add_widget` (ID only) and `add_child` (full widget) use the same list.
struct FlowChild {
    widget_id: ObjectId,
    widget: Option<Box<dyn Widget>>,
    /// Default size used when widget is absent (added via `add_widget`).
    default_size: Size,
}

impl FlowChild {
    fn size_hint(&self) -> Size {
        self.widget.as_ref().map(|w| w.size_hint()).unwrap_or(self.default_size)
    }
}
/// Direction of child arrangement in a flow layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowDirection {
    /// Children are laid out left to right; wrapping starts a new row below.
    #[default]
    Horizontal,
    /// Children are laid out top to bottom; wrapping starts a new column to
    /// the right.
    Vertical,
}
/// Alignment strategy for items within each flow line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowAlignment {
    /// Items are packed against the start edge, honouring [`FlowLayoutConfig::spacing`].
    #[default]
    Start,
    /// Items are grouped in the middle of the content box. The offset is
    /// computed from the *total* item extent including spacing, applied
    /// identically on both axes.
    Center,
    /// Items are packed against the far edge of the content box.
    End,
    /// Free space is distributed *between* items, leaving no space at either
    /// end. With a single item this is a no-op, and if the items overflow the
    /// spacing becomes negative.
    SpaceBetween,
    /// Free space is placed before, between, and after items. Unlike
    /// [`FlowAlignment::SpaceBetween`] this also applies with a single item.
    SpaceAround,
}
/// Configuration for a flow layout: direction, alignment, spacing, padding, and wrapping.
#[derive(Debug, Clone, Copy)]
pub struct FlowLayoutConfig {
    /// Axis along which children are arranged.
    pub direction: FlowDirection,
    /// How items are positioned within each line after the flow pass.
    pub alignment: FlowAlignment,
    /// Gap in logical pixels between adjacent items, and between wrapped lines.
    /// May be negative. Defaults to `8`.
    pub spacing: i32,
    /// Inset in logical pixels applied on **all four** sides of the available
    /// rectangle before children are placed, and included in
    /// [`FlowLayout::preferred_size`]. Defaults to `8`.
    pub padding: i32,
    /// When `true`, items that would cross the far edge of the content box
    /// start a new line (or column) instead of overflowing. Defaults to `false`.
    pub wrap: bool,
}
impl Default for FlowLayoutConfig {
    fn default() -> Self {
        Self {
            direction: FlowDirection::Horizontal,
            alignment: FlowAlignment::Start,
            spacing: 8,
            padding: 8,
            wrap: false,
        }
    }
}
/// A flow layout that arranges children sequentially in a given direction.
///
/// Supports wrapping when `wrap` is enabled in the config — children
/// overflow to the next line (for horizontal) or column (for vertical).
pub struct FlowLayout {
    config: FlowLayoutConfig,
    children: Vec<FlowChild>,
}
impl fmt::Debug for FlowLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowLayout")
            .field("config", &self.config)
            .field("children", &format_args!("{} children", self.children.len()))
            .finish()
    }
}
impl FlowLayout {
    /// Creates an empty layout with the default configuration (horizontal,
    /// start-aligned, spacing 8, padding 8, wrapping off).
    pub fn new() -> Self {
        Self { config: FlowLayoutConfig::default(), children: Vec::new() }
    }
    /// Creates an empty layout with the supplied configuration.
    pub fn with_config(mut config: FlowLayoutConfig) -> Self {
        // Negative padding has no defined meaning and previously reached `2 * padding as u32`,
        // which overflowed in debug builds. Reject it here so every entry point (construction,
        // field, and device-scaled reads) agrees on a non-negative inset.
        if config.padding < 0 {
            log::warn!(
                "FlowLayout: negative padding {} is undefined; treating it as 0",
                config.padding
            );
            config.padding = 0;
        }
        Self { config, children: Vec::new() }
    }
    /// Appends a widget, taking ownership of it.
    ///
    /// The widget's current [`Widget::size_hint`] is captured as its default
    /// size and re-read from the widget on every layout pass, so later changes
    /// to the hint are picked up. Children are laid out in insertion order.
    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        let widget_id = child.id();
        let default_size = child.size_hint();
        self.children.push(FlowChild { widget_id, widget: Some(child), default_size });
    }
    /// Removes the child at `index` and returns it, or returns `None` when the
    /// index is out of range.
    ///
    /// Returns `None` for children that were registered by id only through
    /// [`Layout::add_widget`], because there is no owned widget to hand back.
    /// The entry is removed from the layout either way.
    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            self.children.remove(index).widget
        } else {
            None
        }
    }
    /// Override the default size hint for a child added via `add_widget` (no widget ref).
    ///
    /// The override only affects that default: a child holding a live widget
    /// still reports the widget's own `size_hint`, so this call has no effect
    /// on children added with [`FlowLayout::add_child`]. It is a no-op when no
    /// child has the given id.
    pub fn set_child_size(&mut self, widget_id: ObjectId, size: Size) {
        if let Some(child) = self.children.iter_mut().find(|c| c.widget_id == widget_id) {
            child.default_size = size;
        }
    }

    /// Removes every child, returning no ownership (owned widgets are dropped).
    pub fn clear_children(&mut self) {
        self.children.clear();
    }
    /// Returns the number of children currently registered.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
    /// Computes a rectangle for each child, in the order the children were
    /// added.
    ///
    /// `available_rect` is the layout's own area in parent-relative logical
    /// pixels; the returned rectangles use the same coordinate space, already
    /// inset by [`FlowLayoutConfig::padding`]. A child whose size hint exceeds
    /// the remaining space is still placed at the next slot and is allowed to
    /// overflow when `wrap` is off.
    pub fn layout(&self, available_rect: Rect) -> Vec<Rect> {
        // S-47: clamp a negative padding (which a directly-mutated config could still hold)
        // before the `* 2 as u32` so the inset arithmetic can never overflow; a huge positive
        // padding saturates the content rect to zero instead of folding the error into a
        // bogus geometry.
        let padding = self.config.padding.max(0);
        let content_rect = Rect::new(
            available_rect.x.saturating_add(padding),
            available_rect.y.saturating_add(padding),
            available_rect.width.saturating_sub(2 * padding as u32),
            available_rect.height.saturating_sub(2 * padding as u32),
        );
        match self.config.direction {
            FlowDirection::Horizontal => self.layout_horizontal(&content_rect),
            FlowDirection::Vertical => self.layout_vertical(&content_rect),
        }
    }
    fn layout_horizontal(&self, content_rect: &Rect) -> Vec<Rect> {
        let mut positions = Vec::new();
        // The index at which each flow line begins. Recorded as the lines are formed rather
        // than recovered later from coordinates: with a legal negative `spacing` a later item
        // on the *same* line sits before the content origin, so the old "major <= origin means
        // a new line" heuristic split one line into two and mis-aligned it (D08-L-01).
        let mut line_starts = vec![0usize];
        let mut current_x = content_rect.x;
        let mut current_y = content_rect.y;
        let mut row_height = 0i32;
        for child in &self.children {
            let size = child.size_hint();
            let child_width = size.width as i32;
            let child_height = size.height as i32;
            if self.config.wrap
                && current_x + child_width > content_rect.x + content_rect.width as i32
            {
                current_x = content_rect.x;
                current_y += row_height + self.config.spacing;
                row_height = 0;
                line_starts.push(positions.len());
            }
            positions.push(Rect::new(
                current_x,
                current_y,
                child_width as u32,
                child_height as u32,
            ));
            current_x += child_width + self.config.spacing;
            row_height = row_height.max(child_height);
        }
        self.apply_alignment(&mut positions, &line_starts, content_rect);
        positions
    }
    fn layout_vertical(&self, content_rect: &Rect) -> Vec<Rect> {
        let mut positions = Vec::new();
        let mut line_starts = vec![0usize];
        let mut current_x = content_rect.x;
        let mut current_y = content_rect.y;
        let mut column_width = 0i32;
        for child in &self.children {
            let size = child.size_hint();
            let child_width = size.width as i32;
            let child_height = size.height as i32;
            if self.config.wrap
                && current_y + child_height > content_rect.y + content_rect.height as i32
            {
                current_y = content_rect.y;
                current_x += column_width + self.config.spacing;
                column_width = 0;
                line_starts.push(positions.len());
            }
            positions.push(Rect::new(
                current_x,
                current_y,
                child_width as u32,
                child_height as u32,
            ));
            current_y += child_height + self.config.spacing;
            column_width = column_width.max(child_width);
        }
        self.apply_alignment(&mut positions, &line_starts, content_rect);
        positions
    }
    /// Align the items **within each flow line**, not the whole child set.
    ///
    /// # Why per line
    ///
    /// The flow pass already decided where each line breaks, so an alignment can only
    /// redistribute the free space of *that* line. The previous version summed every
    /// child's extent and moved every child by that single offset, which is only correct
    /// when nothing wrapped: with more than one line the rows were pushed by a number
    /// computed from the other rows, and `End` could drive the early rows off the left
    /// (or top) edge entirely. With wrapping off there is exactly one line, so the old
    /// behaviour is preserved exactly.
    ///
    /// The minor axis handling is per **block**: `Center`/`End` shift every item by the
    /// remaining minor extent, measured as the sum of each line's tallest member plus the
    /// gaps between lines (S-45). This is what centres a single line by its max height and a
    /// wrapped block by its real line footprint.
    fn apply_alignment(&self, positions: &mut [Rect], line_starts: &[usize], content_rect: &Rect) {
        if self.config.alignment == FlowAlignment::Start {
            return;
        }

        // Recover the line groups from the boundaries the layout pass recorded. A coordinate
        // heuristic cannot do this correctly for a negative `spacing`, because a same-line item
        // may legitimately sit before the content origin (D08-L-01).
        let major_is_x = self.config.direction == FlowDirection::Horizontal;
        let origin = if major_is_x { content_rect.x } else { content_rect.y };
        let mut lines: Vec<core::ops::Range<usize>> = Vec::with_capacity(line_starts.len());
        for (i, &start) in line_starts.iter().enumerate() {
            let end = line_starts.get(i + 1).copied().unwrap_or(positions.len());
            if start < end {
                lines.push(start..end);
            }
        }
        if lines.is_empty() && !positions.is_empty() {
            lines.push(0..positions.len());
        }

        for line in &lines {
            let items = &mut positions[line.start..line.end];
            if items.is_empty() {
                continue;
            }
            let available =
                if major_is_x { content_rect.width as i32 } else { content_rect.height as i32 };
            let sizes: i32 = if major_is_x {
                items.iter().map(|r| r.width as i32).sum()
            } else {
                items.iter().map(|r| r.height as i32).sum()
            };
            let n = items.len() as i32;

            // The gap **between** items (start and end gaps are handled separately).
            let gap = match self.config.alignment {
                FlowAlignment::Center | FlowAlignment::End => self.config.spacing,
                FlowAlignment::SpaceBetween => {
                    if n > 1 {
                        (available - sizes) / (n - 1)
                    } else {
                        self.config.spacing
                    }
                }
                FlowAlignment::SpaceAround => {
                    if n > 1 {
                        self.config.spacing
                            + (available - sizes - (n - 1) * self.config.spacing) / n
                    } else {
                        self.config.spacing
                    }
                }
                FlowAlignment::Start => self.config.spacing,
            };

            let painted = sizes + (n - 1) * gap;
            let slack = available - painted;
            let start_offset = match self.config.alignment {
                // Centred / end-aligned: the whole slack moves to one side. `SpaceBetween`
                // leaves none at either end; `SpaceAround` splits it evenly, which is what
                // makes it differ from `SpaceBetween` for a single item too.
                FlowAlignment::Center => slack.max(0) / 2,
                FlowAlignment::End => slack,
                FlowAlignment::SpaceBetween => 0,
                FlowAlignment::SpaceAround => {
                    // The leading offset is half of what the run did *not* paint. The old form
                    // subtracted `(n - 1) * (gap - spacing)` a second time, which double-counted
                    // the very gap increment `painted` had already included (S-44).
                    slack.max(0) / 2
                }
                FlowAlignment::Start => 0,
            };

            let mut cursor = origin + start_offset;
            for item in items.iter_mut() {
                if major_is_x {
                    item.x = cursor;
                    cursor += item.width as i32 + gap;
                } else {
                    item.y = cursor;
                    cursor += item.height as i32 + gap;
                }
            }
        }

        // The minor axis: `Center`/`End` shift every item by the remaining extent. The
        // extent is the **block's** footprint — each line's tallest member plus the gap
        // between lines — not the sum of every sibling's extent. Summing every sibling
        // pushed a single-line block off-centre and a wrapped block by a number computed
        // from the other lines (S-45).
        let minor_total: i32 = lines
            .iter()
            .map(|line| {
                positions[line.start..line.end]
                    .iter()
                    .map(|r| if major_is_x { r.height as i32 } else { r.width as i32 })
                    .max()
                    .unwrap_or(0)
            })
            .sum::<i32>()
            + (lines.len().saturating_sub(1) as i32) * self.config.spacing;
        let minor_available =
            if major_is_x { content_rect.height as i32 } else { content_rect.width as i32 };
        let minor_offset = match self.config.alignment {
            FlowAlignment::Center => (minor_available - minor_total) / 2,
            FlowAlignment::End => minor_available - minor_total,
            _ => 0,
        };
        if minor_offset != 0 {
            for pos in positions.iter_mut() {
                if major_is_x {
                    pos.y += minor_offset;
                } else {
                    pos.x += minor_offset;
                }
            }
        }
    }
    /// Returns the size the layout would like for its children, including
    /// [`FlowLayoutConfig::padding`] on both sides.
    ///
    /// This ignores `wrap` entirely even though [`FlowLayout::layout`] honours
    /// it, so a wrapping layout reports the size needed to keep everything on
    /// one line rather than the wrapped footprint. Gaps are counted between
    /// children only, never around them.
    pub fn preferred_size(&self) -> Size {
        // S-46: `spacing` is documented to allow negative values, so it must be summed as a
        // signed quantity. The old body cast it to `u32` first, which overflowed (debug panic)
        // or wrapped (release) for `spacing = -1`. Accumulate in `i64`, then clamp to zero.
        let count = self.children.len() as i32;
        let gap_total: i64 =
            if count > 1 { (count - 1) as i64 * self.config.spacing as i64 } else { 0 };
        // S-47: a negative padding is rejected at construction, and clamped here as a second
        // line of defence so a directly-mutated config cannot overflow the `* 2`.
        let padding = self.config.padding.max(0) as i64;
        // Clamp the accumulated extent into the representable range so a huge positive
        // padding saturates instead of wrapping around to a bogus small size (S-47).
        let clamp_extent = |v: i64| -> u32 { v.clamp(0, u32::MAX as i64) as u32 };
        match self.config.direction {
            FlowDirection::Horizontal => {
                let width: i64 =
                    self.children.iter().map(|c| c.size_hint().width as i64).sum::<i64>()
                        + gap_total;
                let height: i64 =
                    self.children.iter().map(|c| c.size_hint().height as i64).max().unwrap_or(0);
                Size::new(clamp_extent(width + 2 * padding), clamp_extent(height + 2 * padding))
            }
            FlowDirection::Vertical => {
                let height: i64 =
                    self.children.iter().map(|c| c.size_hint().height as i64).sum::<i64>()
                        + gap_total;
                let width: i64 =
                    self.children.iter().map(|c| c.size_hint().width as i64).max().unwrap_or(0);
                Size::new(clamp_extent(width + 2 * padding), clamp_extent(height + 2 * padding))
            }
        }
    }
}
crate::impl_default_via_new!(FlowLayout);

impl Layout for FlowLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        // Push to the unified children list so layout and update stay in sync.
        if !self.children.iter().any(|c| c.widget_id == widget_id) {
            // Use a reasonable default size (100x100) so layout doesn't collapse
            // children added without a widget reference. The caller can later
            // set the actual size via `set_child_size` or through the Widget trait.
            self.children.push(FlowChild {
                widget_id,
                widget: None,
                default_size: Size::new(100, 100),
            });
        }
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|c| c.widget_id != widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let positions = self.layout(rect);
        for (i, child_rect) in positions.iter().enumerate() {
            if let Some(child) = self.children.get(i) {
                widgets(child.widget_id, *child_rect);
            }
        }
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children.iter().map(|c| c.widget_id).collect()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.children.iter().any(|c| c.widget_id == id)
    }

    fn clear(&mut self) {
        self.clear_children();
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal test widget that returns a fixed size hint.
    struct TestWidget {
        id: ObjectId,
        size: Size,
    }
    impl TestWidget {
        fn new(id: ObjectId, width: u32, height: u32) -> Self {
            Self { id, size: Size::new(width, height) }
        }
    }
    impl crate::event::EventHandler for TestWidget {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Widget for TestWidget {
        fn id(&self) -> ObjectId {
            self.id
        }
        fn size_hint(&self) -> Size {
            self.size
        }
    }

    // --- Empty layout tests ---

    #[test]
    fn test_empty_layout_returns_empty() {
        let layout = FlowLayout::new();
        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert!(positions.is_empty());
    }

    #[test]
    fn test_empty_layout_center_alignment() {
        let mut layout = FlowLayout::new();
        layout.config.alignment = FlowAlignment::Center;
        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert!(positions.is_empty());
    }

    // --- Child positioning tests via add_child (with full widget) ---

    #[test]
    fn test_horizontal_positions_children_in_a_row() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 10;
        layout.config.padding = 0;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 60, 30)));

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 2);
        // First child at (0, 0), second immediately after with 10px spacing
        assert_eq!(positions[0], Rect::new(0, 0, 40, 20));
        assert_eq!(positions[1], Rect::new(50, 0, 60, 30));
    }

    #[test]
    fn test_vertical_positions_children_in_a_column() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Vertical;
        layout.config.spacing = 5;
        layout.config.padding = 0;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 30, 50)));

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], Rect::new(0, 0, 40, 20));
        assert_eq!(positions[1], Rect::new(0, 25, 30, 50));
    }

    #[test]
    fn test_horizontal_wrap_to_next_row() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.config.wrap = true;

        // Three 60px children in a 125px-wide container: row1 gets two, row2 gets one.
        layout.add_child(Box::new(TestWidget::new(1, 60, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 60, 20)));
        layout.add_child(Box::new(TestWidget::new(3, 60, 20)));

        let positions = layout.layout(Rect::new(0, 0, 125, 100));
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], Rect::new(0, 0, 60, 20)); // row 1, child 1
        assert_eq!(positions[1], Rect::new(60, 0, 60, 20)); // row 1, child 2
        assert_eq!(positions[2], Rect::new(0, 20, 60, 20)); // row 2, child 3
    }

    #[test]
    fn test_vertical_wrap_to_next_column() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Vertical;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.config.wrap = true;

        // Three 50px children in a 105px-tall container: col1 gets two, col2 gets one.
        layout.add_child(Box::new(TestWidget::new(1, 30, 50)));
        layout.add_child(Box::new(TestWidget::new(2, 30, 50)));
        layout.add_child(Box::new(TestWidget::new(3, 30, 50)));

        let positions = layout.layout(Rect::new(0, 0, 200, 105));
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], Rect::new(0, 0, 30, 50)); // col 1, child 1
        assert_eq!(positions[1], Rect::new(0, 50, 30, 50)); // col 1, child 2
        assert_eq!(positions[2], Rect::new(30, 0, 30, 50)); // col 2, child 3
    }

    #[test]
    fn test_layout_honors_padding() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 10;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));

        let positions = layout.layout(Rect::new(0, 0, 100, 50));
        assert_eq!(positions.len(), 1);
        // Content starts at (10, 10) due to padding
        assert_eq!(positions[0], Rect::new(10, 10, 40, 20));
    }

    #[test]
    fn test_horizontal_preserves_child_taller_than_container() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 0;

        // Layout preserves the child; the parent renderer owns clipping.
        layout.add_child(Box::new(TestWidget::new(1, 50, 30)));

        let positions = layout.layout(Rect::new(0, 0, 200, 20));
        assert_eq!(positions, vec![Rect::new(0, 0, 50, 30)]);
    }

    #[test]
    fn test_horizontal_preserves_children_after_height_overflow() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 0;
        layout.config.padding = 0;

        // Two children: the second exceeds height but remains addressable.
        layout.add_child(Box::new(TestWidget::new(1, 50, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 50, 30)));

        let positions = layout.layout(Rect::new(0, 0, 200, 25));
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], Rect::new(0, 0, 50, 20));
        assert_eq!(positions[1], Rect::new(50, 0, 50, 30));
    }

    #[test]
    fn test_add_widget_sets_default_size() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.padding = 0;
        layout.config.spacing = 0;

        layout.add_widget(42, 0); // no widget ref → uses default_size 100x100

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn test_set_child_size_overrides_default() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.padding = 0;
        layout.config.spacing = 0;

        layout.add_widget(42, 0);
        layout.set_child_size(42, Size::new(50, 30));

        let positions = layout.layout(Rect::new(0, 0, 300, 200));
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], Rect::new(0, 0, 50, 30));
    }

    #[test]
    fn test_preferred_size_with_children() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = 10;
        layout.config.padding = 0;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 60, 30)));

        // width = 40 + 10 + 60 = 110, height = max(20,30) = 30
        assert_eq!(layout.preferred_size(), Size::new(110, 30));
    }

    /// A wrapped flow has to align **each line on its own**. The offset used to be computed
    /// from every child's extent, so with more than one row each row was shifted by a number
    /// that had nothing to do with it — and `End` could push the early rows off the top-left.
    #[test]
    fn wrapped_rows_are_aligned_per_line_not_against_the_whole_set() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.alignment = FlowAlignment::End;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.config.wrap = true;

        // 100 wide box, 40-wide items: two per row, so two rows of two.
        for id in 1..=4 {
            layout.add_child(Box::new(TestWidget::new(id, 40, 10)));
        }
        let positions = layout.layout(Rect::new(0, 0, 100, 200));
        assert_eq!(positions.len(), 4);

        // Each row holds two items, so it must occupy exactly 80 of the 100 available and
        // be pushed right by the remaining 20 — the *same* 20 for both rows. The old
        // whole-set offset computed 100 - 160 = -60 and moved every row to x = -60, which
        // parked the first item 60px off the left edge of its own box.
        for row in 0..2 {
            let first = positions[row * 2];
            let second = positions[row * 2 + 1];
            assert_eq!(first.x, 20, "row {row} is not right-aligned: {first:?}");
            assert_eq!(second.x, 60, "row {row} is not right-aligned: {second:?}");
            assert_eq!(
                second.x + second.width as i32,
                100,
                "row {row} does not reach the right edge"
            );
        }
        // Two lines: a row is a shared `y`, and the second line is below the first.
        assert_eq!(positions[0].y, positions[1].y, "the first row is one line");
        assert_eq!(positions[2].y, positions[3].y, "the second row is one line");
        assert!(positions[2].y > positions[0].y, "the second line is below the first");
    }

    /// With wrapping off there is a single line, so the aligned result must be unchanged
    /// from the simple whole-set arithmetic callers already depend on.
    #[test]
    fn a_single_line_still_uses_the_whole_set_alignment() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.alignment = FlowAlignment::Center;
        layout.config.spacing = 10;
        layout.config.padding = 0;

        layout.add_child(Box::new(TestWidget::new(1, 40, 20)));
        layout.add_child(Box::new(TestWidget::new(2, 40, 20)));

        // 90 used in a 200 box: 55 of slack on each side.
        let positions = layout.layout(Rect::new(0, 0, 200, 100));
        assert_eq!(positions[0].x, 55);
        assert_eq!(positions[1].x, 105);
    }

    // ── S-44: SpaceAround must stay symmetric and in-bounds ──────────────────────────

    #[test]
    fn space_around_is_symmetric_and_in_bounds() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.alignment = FlowAlignment::SpaceAround;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.add_child(Box::new(TestWidget::new(1, 10, 10)));
        layout.add_child(Box::new(TestWidget::new(2, 10, 10)));

        let positions = layout.layout(Rect::new(0, 0, 100, 40));
        // 20 used by items, 80 left: a 40 gap between them, 20 before and 20 after.
        assert_eq!(positions[0], Rect::new(20, 0, 10, 10));
        assert_eq!(positions[1], Rect::new(70, 0, 10, 10));
        assert_eq!(positions[0].x, 100 - (positions[1].x + positions[1].width as i32));
    }

    #[test]
    fn space_around_pays_the_base_gap_and_handles_remainders() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.alignment = FlowAlignment::SpaceAround;
        layout.config.spacing = 10;
        layout.config.padding = 0;
        for id in [1, 2, 3] {
            layout.add_child(Box::new(TestWidget::new(id, 30, 10)));
        }
        let positions = layout.layout(Rect::new(0, 0, 160, 40));
        assert_eq!(positions.len(), 3);
        let lead = positions[0].x;
        let trail = 160 - (positions[2].x + positions[2].width as i32);
        assert_eq!(lead, trail, "the run is centred, not left-padded");
        for (i, pos) in positions.iter().enumerate() {
            assert!(
                pos.x >= 0 && pos.x + pos.width as i32 <= 160,
                "item {i} ran off the line: {pos:?}"
            );
        }
    }

    #[test]
    fn space_around_single_item_and_vertical_are_symmetric() {
        let mut horizontal = FlowLayout::new();
        horizontal.config.direction = FlowDirection::Horizontal;
        horizontal.config.alignment = FlowAlignment::SpaceAround;
        horizontal.config.spacing = 0;
        horizontal.config.padding = 0;
        horizontal.add_child(Box::new(TestWidget::new(1, 10, 10)));
        let positions = horizontal.layout(Rect::new(0, 0, 100, 40));
        assert_eq!(positions[0].x, 45, "a lone item is centred");

        let mut vertical = FlowLayout::new();
        vertical.config.direction = FlowDirection::Vertical;
        vertical.config.alignment = FlowAlignment::SpaceAround;
        vertical.config.spacing = 0;
        vertical.config.padding = 0;
        vertical.add_child(Box::new(TestWidget::new(1, 10, 10)));
        vertical.add_child(Box::new(TestWidget::new(2, 10, 10)));
        let positions = vertical.layout(Rect::new(0, 0, 40, 100));
        assert_eq!(positions[0], Rect::new(0, 20, 10, 10));
        assert_eq!(positions[1], Rect::new(0, 70, 10, 10));
    }

    // ── S-45: minor-axis centring uses the line's max extent ──────────────────────────

    #[test]
    fn minor_axis_centering_uses_the_lines_max_extent() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.alignment = FlowAlignment::Center;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.add_child(Box::new(TestWidget::new(1, 10, 10)));
        layout.add_child(Box::new(TestWidget::new(2, 10, 10)));

        // Two 10x10 children in a 100x100 box: the line's max height is 10, so the
        // vertical centre is (100 - 10) / 2 = 45, not (100 - 20) / 2 = 40.
        let positions = layout.layout(Rect::new(0, 0, 100, 100));
        assert_eq!(positions[0].y, 45);
        assert_eq!(positions[1].y, 45);
    }

    #[test]
    fn wrapped_minor_axis_centering_uses_real_line_heights() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.alignment = FlowAlignment::Center;
        layout.config.spacing = 0;
        layout.config.padding = 0;
        layout.config.wrap = true;
        // Two rows in a 100px box: row 1 is 30 tall, row 2 is 20 tall.
        layout.add_child(Box::new(TestWidget::new(1, 40, 10)));
        layout.add_child(Box::new(TestWidget::new(2, 40, 30)));
        layout.add_child(Box::new(TestWidget::new(3, 40, 20)));

        let positions = layout.layout(Rect::new(0, 0, 100, 100));
        // Block height = 30 + 20 = 50, centre = (100 - 50) / 2 = 25.
        assert_eq!(positions[0].y, 25);
        assert_eq!(positions[1].y, 25);
        assert_eq!(positions[2].y, 55);
    }

    // ── S-46: negative spacing is legal and must measure/place consistently ───────────

    #[test]
    fn negative_spacing_measures_and_places_consistently() {
        let mut layout = FlowLayout::new();
        layout.config.direction = FlowDirection::Horizontal;
        layout.config.spacing = -1;
        layout.config.padding = 0;
        layout.add_child(Box::new(TestWidget::new(1, 10, 10)));
        layout.add_child(Box::new(TestWidget::new(2, 10, 10)));

        // Width = 10 + (-1) + 10 = 19; previously this panicked on the u32 cast.
        assert_eq!(layout.preferred_size(), Size::new(19, 10));
        let positions = layout.layout(Rect::new(0, 0, 100, 40));
        assert_eq!(positions[1].x, 9, "the second child overlaps by the negative spacing");
    }

    /// D08-L-01: a legal negative spacing must not be mistaken for a line break.
    ///
    /// Horizontal, `wrap = false`, padding 0, spacing -10, `SpaceBetween`, two 10×10
    /// children in a 100×40 area. The second child's initial x (0) equals the content
    /// origin, and with alignment applied coordinates can fall behind it, so the old
    /// "major <= origin means a new line" split the single line and left both children at
    /// x = 0. The free space of one line must instead push them apart.
    #[test]
    fn negative_spacing_with_space_between_keeps_one_line() {
        let mut layout = FlowLayout::with_config(FlowLayoutConfig {
            direction: FlowDirection::Horizontal,
            alignment: FlowAlignment::SpaceBetween,
            spacing: -10,
            wrap: false,
            padding: 0,
        });
        layout.add_child(Box::new(TestWidget::new(1, 10, 10)));
        layout.add_child(Box::new(TestWidget::new(2, 10, 10)));
        let positions = layout.layout(Rect::new(0, 0, 100, 40));
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].y, positions[1].y, "both children stay on one line");
        assert_eq!(positions[0].x, 0);
        assert_eq!(positions[1].x, 90, "SpaceBetween spreads the single line's free space");
    }

    /// D08-L-01: the Vertical direction has the symmetric defect and must be fixed too.
    #[test]
    fn negative_spacing_with_space_between_keeps_one_column() {
        let mut layout = FlowLayout::with_config(FlowLayoutConfig {
            direction: FlowDirection::Vertical,
            alignment: FlowAlignment::SpaceBetween,
            spacing: -10,
            wrap: false,
            padding: 0,
        });
        layout.add_child(Box::new(TestWidget::new(1, 10, 10)));
        layout.add_child(Box::new(TestWidget::new(2, 10, 10)));
        let positions = layout.layout(Rect::new(0, 0, 40, 100));
        assert_eq!(positions[0].x, positions[1].x, "both children stay in one column");
        assert_eq!(positions[0].y, 0);
        assert_eq!(positions[1].y, 90);
    }

    /// A genuine wrap still splits into the recorded lines even with negative spacing, so the
    /// fix does not collapse two real lines into one.
    #[test]
    fn wrapping_still_splits_lines_with_negative_spacing() {
        let mut layout = FlowLayout::with_config(FlowLayoutConfig {
            direction: FlowDirection::Horizontal,
            alignment: FlowAlignment::Start,
            spacing: -5,
            wrap: true,
            padding: 0,
        });
        // Three 10-wide children in a 15-wide area with spacing -5: the first two fit on
        // the first line, the third wraps.
        for id in 1..=3 {
            layout.add_child(Box::new(TestWidget::new(id, 10, 10)));
        }
        let positions = layout.layout(Rect::new(0, 0, 15, 100));
        assert_eq!(positions[0].y, 0);
        assert_eq!(positions[1].y, 0, "the second child still fits on the first line");
        assert!(positions[2].y > 0, "the third child wraps to a new line");
    }

    // ── S-47: negative padding is rejected, not folded into geometry ──────────────────

    #[test]
    fn negative_padding_is_clamped_to_zero_at_construction() {
        let layout = FlowLayout::with_config(FlowLayoutConfig {
            direction: FlowDirection::Horizontal,
            alignment: FlowAlignment::Start,
            spacing: 0,
            padding: -1,
            wrap: false,
        });
        assert_eq!(layout.preferred_size(), Size::new(0, 0));
    }

    #[test]
    fn a_directly_mutated_negative_padding_does_not_panic() {
        let mut layout = FlowLayout::new();
        layout.config.padding = -1;
        layout.config.spacing = 0;
        layout.add_child(Box::new(TestWidget::new(1, 10, 10)));
        let positions = layout.layout(Rect::new(0, 0, 100, 50));
        assert_eq!(positions[0], Rect::new(0, 0, 10, 10));
    }

    #[test]
    fn a_huge_padding_saturates_instead_of_wrapping() {
        let mut layout = FlowLayout::new();
        layout.config.padding = i32::MAX;
        layout.config.spacing = 0;
        layout.add_child(Box::new(TestWidget::new(1, 10, 10)));
        // No panic, and the reported preferred size saturates rather than wrapping negative.
        let size = layout.preferred_size();
        assert!(size.width >= 10 && size.height >= 10);
    }

    // ── S-55: `dyn Layout::clear` must reach the concrete clear_children ──────────────

    #[test]
    fn dyn_clear_removes_flow_children() {
        let mut layout: Box<dyn Layout> = Box::new(FlowLayout::new());
        layout.add_widget(1, 0);
        assert!(layout.has_child(1));
        layout.clear();
        assert!(!layout.has_child(1));
        assert!(layout.child_ids().is_empty());
        // A repeat clear is a no-op, not a panic or a re-emission.
        layout.clear();
        assert!(layout.child_ids().is_empty());
    }
}
