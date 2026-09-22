// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Layout managers.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/capability/constructors.rs:1` and `src/json/loader.rs:1` (both resolve layout kinds through `layout::`).
/// Positions each child at explicit absolute coordinates, ignoring the parent
/// rect apart from the origin it offsets from.
pub mod absolute;
/// Derives a child's missing dimension from a fixed width:height ratio.
pub mod aspect_ratio;
/// Single-axis linear layout with per-child stretch factors, spacing, and
/// margins; also exposed as `HBox` / `VBox`.
pub mod box_layout;
/// Centers one child in the available rect.
pub mod center;
/// Min/max size limits and expansion weights attached to layout items.
pub mod constraint;
/// Runtime storage and application for declarative layouts, shared by the JSON
/// loader and the C ABI. See the module docs for why it is separate from the
/// `serde_json`-facing translation in `json::layout`.
pub mod declarative;
/// CSS-Flexbox-style layout combining direction, wrap, and flex factors.
pub mod flex;
/// Wraps children into rows or columns, moving to the next line when full.
pub mod flow;
/// Two-column label/field rows, with an optional alignment for the field
/// column.
pub mod form;
/// Fixed grid layout addressed by `(row, column)` with optional spans.
pub mod grid;
/// Size hints — the channel that lets a layout ask a child how big it wants to be.
pub mod hints;
/// Diagnostic tooling for inspecting the geometry a layout produced.
pub mod inspector;
/// Adjusts a rect to avoid the on-screen software keyboard.
pub mod keyboard_aware;
/// Divisible panes with draggable handles; see
/// `crate::widget::Splitter` for the widget front end.
pub mod splitter;
/// Shows one child at a time, filling the whole rect.
pub mod stack;
pub mod types;
/// Uniform grid where every cell has the same size.
pub mod uniform_grid;
/// Line-breaking layout for a sequence of fixed-size items.
pub mod wrap;
pub use crate::core::Orientation;
pub use absolute::*;
pub use aspect_ratio::*;
pub use box_layout::*;
pub use center::*;
pub use constraint::*;
pub use flex::*;
pub use flow::*;
pub use form::*;
pub use grid::*;
pub use hints::*;
pub use inspector::*;
pub use keyboard_aware::*;
pub use splitter::*;
pub use stack::*;
pub use types::*;
pub use uniform_grid::*;
pub use wrap::*;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;
    use crate::core::{Point, Rect, Size};
    #[test]
    fn box_layout_applies_constraints() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.set_constraints(1, LayoutConstraints::new(80, Some(80)));
        layout.set_size_policy(1, SizePolicy::Fixed);
        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 40), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.get(&1).map(|rect| rect.width), Some(80));
    }
    #[test]
    fn splitter_layout_distributes_space() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 0);
        splitter.add_widget(1, 1);
        splitter.add_widget(2, 3);
        let mut rects = HashMap::new();
        splitter.update(Rect::new(0, 0, 400, 40), &mut |id, rect| {
            rects.insert(id, rect);
        });
        let left = rects.get(&1).map(|rect| rect.width).unwrap_or(0);
        let right = rects.get(&2).map(|rect| rect.width).unwrap_or(0);
        assert!(right > left);
    }
    #[test]
    fn layout_update_from_position_size_routes_through_rect_conversion() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(42, 1);
        let mut out = None;
        layout.update_from_position_size(Point::new(9, 11), Size::new(30, 12), &mut |id, rect| {
            if id == 42 {
                out = Some(rect);
            }
        });
        assert_eq!(out, Some(Rect::new(9, 11, 30, 12)));
    }
    #[test]
    fn hbox_and_vbox_named_types_delegate_to_box_layout_contract() {
        let mut hbox = BoxLayout::new(Orientation::Horizontal, 3, 2);
        hbox.add_widget(1, 1);
        hbox.add_spacer(1);
        hbox.add_widget(2, 2);
        assert_eq!(hbox.spacing(), 3);
        assert_eq!(hbox.margin(), 2);
        assert_eq!(hbox.item_count(), 3);
        let mut rects = HashMap::new();
        hbox.update(Rect::new(0, 0, 120, 20), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.len(), 2);
        let mut vbox = BoxLayout::new(Orientation::Vertical, 1, 0);
        vbox.add_widget(10, 1);
        vbox.add_widget(11, 1);
        let mut out = HashMap::new();
        vbox.update(Rect::new(0, 0, 20, 40), &mut |id, rect| {
            out.insert(id, rect);
        });
        assert_eq!(out.len(), 2);
        assert!(
            out.get(&11).map(|r| r.y).unwrap_or_default()
                > out.get(&10).map(|r| r.y).unwrap_or_default()
        );
    }
    #[test]
    fn box_layout_distribution_consumes_available_major_axis() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        layout.add_widget(3, 1);
        let mut widths = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 10), &mut |id, rect| {
            widths.insert(id, rect.width);
        });
        let total: u32 = widths.values().copied().sum();
        assert_eq!(total, 100);
    }
    #[test]
    fn grid_and_stack_layouts_have_deterministic_placement() {
        let mut grid = GridLayout::new(2, 2, 0, 0);
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 1, 2);
        let mut grid_rects = HashMap::new();
        grid.update(Rect::new(0, 0, 40, 20), &mut |id, rect| {
            grid_rects.insert(id, rect);
        });
        assert_eq!(grid_rects.get(&1), Some(&Rect::new(0, 0, 20, 10)));
        assert_eq!(grid_rects.get(&2), Some(&Rect::new(20, 10, 20, 10)));
        let mut stack = StackLayout::new();
        stack.add_widget(7, 0);
        stack.add_widget(8, 0);
        stack.set_current_index(1);
        let mut shown = None;
        stack.update(Rect::new(1, 2, 30, 40), &mut |id, rect| {
            shown = Some((id, rect));
        });
        assert_eq!(shown, Some((8, Rect::new(1, 2, 30, 40))));
    }

    /// Children must never be placed outside the rect the layout was given.
    ///
    /// Regression: when the sum of the items' minima exceeded the available length,
    /// `BoxLayout` returned allocations whose total was larger than the parent, so two
    /// 80-pixel minima in a 100-pixel row were drawn at `x = 0` and `x = 80` — running to
    /// 160 and painting the second control over whatever sat beside the layout. There is no
    /// assignment that honours both minima there, so the shortfall is now split evenly.
    #[test]
    fn box_layout_children_stay_inside_a_parent_that_cannot_pay_every_minimum() {
        let mut layout = BoxLayout::new(Orientation::Horizontal, 0, 0);
        layout.add_widget(1, 1);
        layout.add_widget(2, 1);
        for id in [1, 2] {
            layout.set_constraints(id, LayoutConstraints::new(80, None));
            layout.set_size_policy(id, SizePolicy::Fixed);
        }
        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 20), &mut |id, rect| {
            rects.insert(id, rect);
        });
        let total: u32 = rects.values().map(|rect| rect.width).sum();
        assert!(total <= 100, "the children must fit the parent, got {total}");
        for (id, rect) in &rects {
            assert!(
                rect.x >= 0 && rect.x + rect.width as i32 <= 100,
                "item {id} escapes: {rect:?}"
            );
        }
        // The shortfall is shared, not applied to one item by iteration order.
        let first = rects.get(&1).map(|rect| rect.width).unwrap_or(0);
        let second = rects.get(&2).map(|rect| rect.width).unwrap_or(0);
        assert!(first.abs_diff(second) <= 1, "the shortfall must be shared evenly");
    }

    /// A grid whose rect cannot hold its own margins and spacing must not overflow.
    ///
    /// Regression: `available_width` saturated to zero while the offset walk still stepped
    /// by the full configured spacing, so later columns were placed at coordinates the
    /// parent had never allocated.
    #[test]
    fn grid_cells_stay_inside_a_parent_too_small_for_its_spacing() {
        let mut grid = GridLayout::new(2, 2, 8, 2);
        grid.set_widget(0, 0, 1);
        grid.set_widget(0, 1, 2);
        grid.set_widget(1, 0, 3);
        grid.set_widget(1, 1, 4);
        let mut rects = HashMap::new();
        grid.update(Rect::new(0, 0, 8, 4), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.len(), 4, "every cell is still addressed");
        for (id, rect) in &rects {
            assert!(
                rect.x >= 0
                    && rect.y >= 0
                    && rect.x + rect.width as i32 <= 8
                    && rect.y + rect.height as i32 <= 4,
                "cell {id} escapes the 8x4 parent: {rect:?}"
            );
        }
    }

    /// The splitter's weights and its normalised ratios must not disagree in meaning.
    ///
    /// Regression: `add_pane` stored a raw weight without the `0.01` floor that
    /// `add_widget` applied, so the two entry points populated the same field with
    /// different units. The weights are relative (normalised on release), and the
    /// documented accessor now says so.
    #[test]
    fn splitter_add_pane_matches_add_widget_and_normalises_on_release() {
        let mut splitter = SplitterLayout::new(Orientation::Horizontal, 0);
        splitter.add_pane(1, 1);
        splitter.add_pane(2, 3);
        assert_eq!(splitter.pane_count(), 2);
        // Relative weights, not fractions, until normalised.
        let weight_sum: f32 = splitter.ratios().iter().sum();
        assert!((weight_sum - 4.0).abs() < 1e-6, "weights are relative: {weight_sum}");
        splitter.normalize_ratios();
        let fraction_sum: f32 = splitter.ratios().iter().sum();
        assert!((fraction_sum - 1.0).abs() < 1e-6, "normalising yields fractions");
        assert!((splitter.ratio(1).unwrap_or(0.0) - 0.75).abs() < 1e-6);
    }
}
