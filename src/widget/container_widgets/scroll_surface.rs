// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The generic scroll surface contract (RW-58).
//!
//! # Why this exists
//!
//! Each scrolling container owned its offset under a different name: `ScrollArea` publishes
//! `scroll_position` / `set_scroll_position` / `ensure_visible`, while `MdiArea` exposes a
//! `scroll_offset` point with `scroll_by`. A host that restores a workspace therefore had to
//! know which concrete control it was holding and write one branch per control.
//!
//! This trait is the common read/write primitive. The per-widget methods stay exactly as they
//! were; implementing the trait is additive and only names the three operations every scrolling
//! surface already performs.
//!
//! # Coordinate space
//!
//! Positions are **content** coordinates: `(0, 0)` is the top-left of the scrollable content,
//! and a positive offset moves the content up/left under the viewport. `ensure_visible` takes a
//! rectangle in the same space and is allowed to do nothing when the rectangle is already
//! inside the viewport.

use crate::core::Rect;

/// A container that can report and move its scroll offset.
pub trait ScrollableSurface {
    /// The current scroll offset as `(x, y)` in content coordinates.
    fn scroll_position(&self) -> (i32, i32);

    /// Moves the scroll offset to `(x, y)`.
    ///
    /// Implementations clamp against their own content and viewport extents, so a caller may
    /// pass an out-of-range request and read the accepted value back from
    /// [`Self::scroll_position`].
    fn set_scroll_position(&mut self, x: i32, y: i32);

    /// Scrolls the smallest distance that makes `rect` fully visible.
    ///
    /// `rect` is in content coordinates. An empty or already visible rectangle leaves the
    /// offset unchanged.
    fn ensure_visible(&mut self, rect: Rect);
}

/// A container whose scroll position is a **row index**, not a pixel offset (RW-58).
///
/// # Why this is separate from [`ScrollableSurface`]
///
/// A table, list or tree scrolls by whole rows, and its own state already stores the first
/// visible row. Dressing that index up as a pixel offset (`y / row_height`) would make the API
/// lie: the value is not continuous, and rounding it back is not the operation the control
/// performed. The two traits therefore describe the two units a container can genuinely scroll
/// in; a container implements the one its own state uses (or both, when it has both axes).
///
/// # Units and clamping
///
/// The unit is a **row index**: `0` is the first row, the same `(0, 0)`-at-the-leading-edge rule
/// [`ScrollableSurface`] uses. Implementations clamp against their own row count, so an
/// out-of-range request is accepted up to the control's own limit and the accepted value can be
/// read back from [`Self::scroll_row`]. `(0, 0)` is the top row; a positive index moves the
/// viewport down.
///
/// `row_height` answers [`None`] when the container's rows are not uniform (`data_grid` sizes a
/// row by its open detail band). It is deliberately not an average: a number the control cannot
/// honour would be worse than the honest absence.
pub trait ScrollableRows {
    /// The first visible row index.
    fn scroll_row(&self) -> usize;

    /// Moves the first visible row to `row`, clamped by the control.
    fn scroll_to_row(&mut self, row: usize);

    /// How many whole rows fit in the viewport at the current size.
    fn visible_row_count(&self) -> usize;

    /// The height of one row in pixels, or [`None`] when rows are not uniform.
    fn row_height(&self) -> Option<u32>;

    /// Scrolls the smallest distance that makes `row` fully visible.
    ///
    /// The default uses the same nearest-window rule the list/table control already apply: a row
    /// above the window becomes the first row, a row below it lands at the bottom edge, and a row
    /// already inside does not move. Implementations with a richer own routine (a list box's
    /// `ScrollToMode`, a virtual list's `ScrollHint`) override it.
    fn ensure_row_visible(&mut self, row: usize) {
        let current = self.scroll_row();
        let visible = self.visible_row_count().max(1);
        let target = if row < current {
            row
        } else if row >= current.saturating_add(visible) {
            row.saturating_add(1).saturating_sub(visible)
        } else {
            current
        };
        self.scroll_to_row(target);
    }
}

// ── Row-unit implementations (RW-58) ───────────────────────────
//
// Each implementation bridges the control's existing row API; no method is renamed and no new
// scroll state is introduced. `visible_row_count` measures the viewport, not the document: a tree
// whose projection has 200 rows still reports the handful that fit.

/// How many whole rows of `row_height` fit in a `viewport_height` band.
fn rows_in_viewport(viewport_height: u32, row_height: u32) -> usize {
    if viewport_height == 0 {
        return 0;
    }
    (viewport_height / row_height.max(1)) as usize
}

/// Generates a [`ScrollableRows`] implementation from four field/method expressions.
///
/// The method names on the controls differ (`scroll_offset` vs `scroll_row`) and their storage
/// types differ (`usize` vs `i32` pixels), so every implementation has to bridge rather than
/// rename. This macro keeps that bridge to one line per operation and keeps the clamping in the
/// control: `set` is whatever the control already does to accept a row.
///
/// `ensure_row_visible` is not parameterised: the trait's default nearest-window rule is the same
/// for every uniform-row container.
#[macro_export]
macro_rules! impl_scrollable_rows {
    ($ty:ty,
     row = |$s:ident| $row:expr,
     set = |$s_mut:ident, $row_arg:ident| $set:expr,
     visible = |$s_vis:ident| $visible:expr,
     height = |$s_h:ident| $height:expr) => {
        impl $crate::widget::container_widgets::ScrollableRows for $ty {
            fn scroll_row(&self) -> usize {
                let $s = self;
                $row
            }

            fn scroll_to_row(&mut self, $row_arg: usize) {
                let $s_mut = self;
                $set
            }

            fn visible_row_count(&self) -> usize {
                let $s_vis = self;
                $visible
            }

            fn row_height(&self) -> Option<u32> {
                let $s_h = self;
                $height
            }
        }
    };
}

use crate::widget::input_widgets::listbox::{ListBox, ScrollToMode};
#[cfg(full_widgets)]
use crate::widget::view_widgets::data_grid::DataGrid;
#[cfg(full_widgets)]
use crate::widget::view_widgets::grid_table::GridTableWidget;
#[cfg(full_widgets)]
use crate::widget::view_widgets::list_view::{ListView, ScrollHint};
#[cfg(full_widgets)]
use crate::widget::view_widgets::table_widget::TableWidget;
#[cfg(full_widgets)]
use crate::widget::view_widgets::tree::tree_table::TreeTable;
#[cfg(full_widgets)]
use crate::widget::view_widgets::tree::tree_view::TreeView;
#[cfg(full_widgets)]
use crate::widget::view_widgets::virtual_list::VirtualList;
#[cfg(full_widgets)]
use crate::widget::view_widgets::virtual_table::VirtualTable;

/// The row window that keeps `row` inside a viewport of `visible` rows.
#[cfg(full_widgets)]
fn nearest_row(current: usize, row: usize, visible: usize) -> usize {
    let visible = visible.max(1);
    if row < current {
        row
    } else if row >= current.saturating_add(visible) {
        row.saturating_add(1).saturating_sub(visible)
    } else {
        current
    }
}

impl ScrollableRows for ListBox {
    fn scroll_row(&self) -> usize {
        ListBox::scroll_offset(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        ListBox::scroll_to_item(self, row, ScrollToMode::PositionAtTop);
    }

    fn visible_row_count(&self) -> usize {
        rows_in_viewport(
            crate::widget::Widget::geometry(self).height,
            ListBox::item_height(self).round().max(1.0) as u32,
        )
    }

    fn row_height(&self) -> Option<u32> {
        Some(ListBox::item_height(self).round().max(1.0) as u32)
    }

    fn ensure_row_visible(&mut self, row: usize) {
        ListBox::scroll_to_item(self, row, ScrollToMode::EnsureVisible);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for VirtualList {
    fn scroll_row(&self) -> usize {
        VirtualList::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        VirtualList::set_scroll_row(self, row);
    }

    fn visible_row_count(&self) -> usize {
        rows_in_viewport(
            crate::widget::Widget::geometry(self).height,
            VirtualList::row_height(self),
        )
    }

    fn row_height(&self) -> Option<u32> {
        Some(VirtualList::row_height(self))
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let _ = VirtualList::scroll_to_row(self, row, ScrollHint::Nearest);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for VirtualTable {
    fn scroll_row(&self) -> usize {
        VirtualTable::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        VirtualTable::set_scroll_row(self, row);
    }

    fn visible_row_count(&self) -> usize {
        rows_in_viewport(
            crate::widget::Widget::geometry(self).height,
            VirtualTable::row_height(self),
        )
    }

    fn row_height(&self) -> Option<u32> {
        Some(VirtualTable::row_height(self))
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let target = nearest_row(self.scroll_row(), row, self.visible_row_count());
        VirtualTable::set_scroll_row(self, target);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for DataGrid {
    fn scroll_row(&self) -> usize {
        DataGrid::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        DataGrid::set_scroll_row(self, row);
    }

    /// Counts rows by summing their real heights, because an open detail band makes them
    /// non-uniform. No average is used: a count derived from a mean would disagree with what the
    /// paint pass fits.
    fn visible_row_count(&self) -> usize {
        let viewport = crate::widget::Widget::geometry(self).height as i64;
        let rows = DataGrid::row_count(self);
        let mut used = 0i64;
        let mut count = 0usize;
        let mut row = self.scroll_row();
        while row < rows {
            let height = i64::from(DataGrid::row_height_at(self, row));
            if height <= 0 || used.saturating_add(height) > viewport {
                break;
            }
            used = used.saturating_add(height);
            count += 1;
            row += 1;
        }
        count
    }

    /// Rows are not uniform, so there is no single height to answer (`BLUE30 §2.2`).
    fn row_height(&self) -> Option<u32> {
        None
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let target = nearest_row(self.scroll_row(), row, self.visible_row_count());
        DataGrid::set_scroll_row(self, target);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for GridTableWidget {
    fn scroll_row(&self) -> usize {
        GridTableWidget::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        GridTableWidget::set_scroll_row(self, row);
    }

    fn visible_row_count(&self) -> usize {
        GridTableWidget::visible_row_count(self)
    }

    fn row_height(&self) -> Option<u32> {
        Some(GridTableWidget::row_height(self))
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let target = nearest_row(self.scroll_row(), row, self.visible_row_count());
        GridTableWidget::set_scroll_row(self, target);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for ListView {
    fn scroll_row(&self) -> usize {
        ListView::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        ListView::set_scroll_row(self, row);
    }

    fn visible_row_count(&self) -> usize {
        ListView::visible_row_count(self)
    }

    fn row_height(&self) -> Option<u32> {
        Some(ListView::row_height(self))
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let target = nearest_row(self.scroll_row(), row, self.visible_row_count());
        ListView::set_scroll_row(self, target);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for TreeView {
    fn scroll_row(&self) -> usize {
        TreeView::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        TreeView::set_scroll_row(self, row);
    }

    fn visible_row_count(&self) -> usize {
        rows_in_viewport(crate::widget::Widget::geometry(self).height, TreeView::row_height(self))
    }

    fn row_height(&self) -> Option<u32> {
        Some(TreeView::row_height(self))
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let target = nearest_row(self.scroll_row(), row, self.visible_row_count());
        TreeView::set_scroll_row(self, target);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for TreeTable {
    fn scroll_row(&self) -> usize {
        TreeTable::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        TreeTable::set_scroll_row(self, row);
    }

    fn visible_row_count(&self) -> usize {
        rows_in_viewport(crate::widget::Widget::geometry(self).height, TreeTable::row_height(self))
    }

    fn row_height(&self) -> Option<u32> {
        Some(TreeTable::row_height(self))
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let target = nearest_row(self.scroll_row(), row, self.visible_row_count());
        TreeTable::set_scroll_row(self, target);
    }
}

#[cfg(full_widgets)]
impl ScrollableRows for TableWidget {
    fn scroll_row(&self) -> usize {
        TableWidget::scroll_row(self)
    }

    fn scroll_to_row(&mut self, row: usize) {
        TableWidget::set_scroll_row(self, row);
    }

    /// The rows band, measured with each row's resolved height.
    ///
    /// A table's rows are not uniform — an override or a delegate's size hint composes them — so a
    /// single division would be a fiction; the control owns the walk because only it can resolve a
    /// row's height (override, hint, or the default constant).
    fn visible_row_count(&self) -> usize {
        TableWidget::visible_rows(self)
    }

    /// Rows carry their own heights, so a single answer would be a fiction.
    fn row_height(&self) -> Option<u32> {
        None
    }

    fn ensure_row_visible(&mut self, row: usize) {
        let target = nearest_row(self.scroll_row(), row, self.visible_row_count());
        TableWidget::set_scroll_row(self, target);
    }
}

#[cfg(widgets_unstripped)]
use super::mdiarea::MdiArea;
use super::scrollarea::ScrollArea;

impl ScrollableSurface for ScrollArea {
    fn scroll_position(&self) -> (i32, i32) {
        ScrollArea::scroll_position(self)
    }

    fn set_scroll_position(&mut self, x: i32, y: i32) {
        ScrollArea::set_scroll_position(self, x, y);
    }

    fn ensure_visible(&mut self, rect: Rect) {
        ScrollArea::ensure_visible(self, rect);
    }
}

#[cfg(widgets_unstripped)]
impl ScrollableSurface for MdiArea {
    fn scroll_position(&self) -> (i32, i32) {
        let offset = MdiArea::scroll_offset(self);
        (offset.x, offset.y)
    }

    fn set_scroll_position(&mut self, x: i32, y: i32) {
        MdiArea::set_scroll_offset(self, crate::core::Point::new(x, y));
    }

    /// Pans the work area by the smallest offset that brings `rect` into the viewport.
    ///
    /// The MDI pan is measured from the content's leading edge, and a stored sub-window frame
    /// is in content coordinates, so this is the same "subtract the offset, compare with the
    /// viewport" rule [`ScrollArea`] applies.
    fn ensure_visible(&mut self, rect: Rect) {
        let viewport = crate::widget::Widget::geometry(self);
        let current = MdiArea::scroll_offset(self);
        let mut x = current.x;
        let mut y = current.y;
        if rect.x - x < viewport.x {
            x = rect.x - viewport.x;
        }
        let right = rect.x.saturating_add(rect.width as i32) - x;
        let viewport_right = viewport.x.saturating_add(viewport.width as i32);
        if right > viewport_right {
            x = x.saturating_add(right - viewport_right);
        }
        if rect.y - y < viewport.y {
            y = rect.y - viewport.y;
        }
        let bottom = rect.y.saturating_add(rect.height as i32) - y;
        let viewport_bottom = viewport.y.saturating_add(viewport.height as i32);
        if bottom > viewport_bottom {
            y = y.saturating_add(bottom - viewport_bottom);
        }
        MdiArea::set_scroll_offset(self, crate::core::Point::new(x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RW-58: each row container answers `scroll_row` through the property route and reports the
    /// value it accepted, not the value it was asked for.
    #[cfg(full_widgets)]
    #[test]
    fn row_containers_round_trip_scroll_row() {
        use crate::widget::capability::types::CapabilityValue;
        use crate::widget::WidgetFactory;

        let factory = WidgetFactory::new_with_defaults();
        let cases: &[(&str, bool)] = &[
            ("list_box", true),
            ("list_view", true),
            ("tree_view", true),
            ("tree_table", true),
            ("table_widget", false),
            ("virtual_list", true),
            ("virtual_table", true),
            ("data_grid", false),
            ("grid_table", true),
            ("command_palette", true),
            ("context_menu", false),
            ("menu", false),
            ("dropdown_menu", true),
            ("font_combo_box", true),
            ("layer_panel", true),
            ("mac_source_list", true),
            ("mention", true),
            ("menu_button", false),
            ("menu_panel", true),
            ("multi_select_combo_box", true),
            ("notification_center", true),
            ("playlist_panel", true),
            ("properties_panel", true),
            ("property_grid", true),
            ("query_builder", true),
            ("shortcut_editor", true),
            ("timeline_widget", true),
            ("toast_stack", true),
            ("tool_box", true),
        ];

        for &(name, uniform_rows) in cases {
            let mut widget = factory
                .create(name, Rect::new(0, 0, 200, 120), "")
                .unwrap_or_else(|| panic!("`{name}` constructs"));
            let rows = widget
                .scrollable_rows()
                .unwrap_or_else(|| panic!("`{name}` implements ScrollableRows"));
            assert_eq!(
                rows.row_height().is_some(),
                uniform_rows,
                "`{name}` must answer `row_height` only when rows are uniform"
            );

            assert_eq!(
                factory.read_property(widget.as_ref(), "scroll_row"),
                Ok(CapabilityValue::UInt(0)),
                "`{name}` starts at row 0"
            );

            factory
                .write_property(widget.as_mut(), "scroll_row", CapabilityValue::UInt(9_999))
                .unwrap_or_else(|error| panic!("`{name}` accepts an out-of-range row: {error:?}"));
            let accepted = match factory.read_property(widget.as_ref(), "scroll_row") {
                Ok(CapabilityValue::UInt(row)) => row,
                other => panic!("`{name}` reports its accepted row as UInt: {other:?}"),
            };
            assert!(accepted <= 9_999);

            // The reported value is stable: writing it back does not move the control again.
            factory
                .write_property(widget.as_mut(), "scroll_row", CapabilityValue::UInt(accepted))
                .unwrap_or_else(|error| panic!("`{name}` re-accepts its own value: {error:?}"));
            assert_eq!(
                factory.read_property(widget.as_ref(), "scroll_row"),
                Ok(CapabilityValue::UInt(accepted)),
                "`{name}` keeps the value it reported"
            );

            // And row 0 is always reachable, whatever the content length was.
            factory
                .write_property(widget.as_mut(), "scroll_row", CapabilityValue::UInt(0))
                .unwrap_or_else(|error| panic!("`{name}` accepts row 0: {error:?}"));
            assert_eq!(
                factory.read_property(widget.as_ref(), "scroll_row"),
                Ok(CapabilityValue::UInt(0))
            );
        }
    }

    /// A control with no row state answers `UnknownProperty`, not a fabricated row.
    #[cfg(full_widgets)]
    #[test]
    fn a_control_without_rows_has_no_scroll_row() {
        use crate::widget::capability::types::CapabilityValue;
        use crate::widget::WidgetFactory;

        let factory = WidgetFactory::new_with_defaults();
        let mut button =
            factory.create("button", Rect::new(0, 0, 80, 24), "").expect("button constructs");
        assert!(button.scrollable_rows().is_none());
        assert_eq!(
            factory.read_property(button.as_ref(), "scroll_row"),
            Err(crate::widget::capability::CapabilityAccessError::UnknownProperty)
        );
        assert_eq!(
            factory.write_property(button.as_mut(), "scroll_row", CapabilityValue::UInt(3)),
            Err(crate::widget::capability::CapabilityAccessError::UnknownProperty)
        );
    }

    #[test]
    fn a_scroll_area_round_trips_the_trait_offset() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 100, 100));
        area.set_content_size(crate::core::Size::new(400, 400));
        area.set_viewport(Rect::new(0, 0, 100, 100));
        ScrollableSurface::set_scroll_position(&mut area, 40, 60);
        assert_eq!(ScrollableSurface::scroll_position(&area), (40, 60));
        ScrollableSurface::ensure_visible(&mut area, Rect::new(0, 0, 10, 10));
        assert_eq!(
            ScrollableSurface::scroll_position(&area),
            (0, 0),
            "the leading edge is revealed"
        );
        // The per-widget entry and the trait entry name the same state.
        assert_eq!(area.scroll_position(), ScrollableSurface::scroll_position(&area));
    }

    #[cfg(widgets_unstripped)]
    #[test]
    fn an_mdi_area_round_trips_the_trait_offset() {
        let mut area = MdiArea::new(Rect::new(0, 0, 100, 100));
        // A pan needs content larger than the viewport to be meaningful; the MDI clamp keeps
        // an empty desktop at (0, 0).
        area.add_sub_window(1, Rect::new(0, 0, 400, 400));
        ScrollableSurface::set_scroll_position(&mut area, 15, 25);
        assert_eq!(ScrollableSurface::scroll_position(&area), (15, 25));
        // The MDI content bounds clamp the pan; the accepted value is what the trait reports.
        ScrollableSurface::set_scroll_position(&mut area, 9_999, 9_999);
        assert_eq!(ScrollableSurface::scroll_position(&area), (300, 300));
    }

    /// A table without row overrides still reports how many rows its band holds.
    ///
    /// `visible_row_count` broke out of its walk on `TableWidget::row_height(row) == None`, which is
    /// every row that has no explicit override. A plain 20-row table in a 300 px viewport therefore
    /// reported zero visible rows, and `ensure_row_visible(5)` scrolled row 5 to the top although it
    /// was already on screen. The control now owns the walk over its resolved heights.
    #[cfg(full_widgets)]
    #[test]
    fn a_table_without_row_overrides_reports_its_visible_rows() {
        use crate::widget::view_widgets::table_widget::{TableModel, TableWidget};
        use crate::widget::Widget;
        use std::sync::Arc;

        struct TwentyRows;
        impl TableModel for TwentyRows {
            fn row_count(&self) -> usize {
                20
            }
            fn column_count(&self) -> usize {
                2
            }
            fn data(&self, row: usize, _column: usize) -> Option<String> {
                Some(format!("r{row}"))
            }
        }

        let mut table = TableWidget::new(Rect::new(0, 0, 200, 300));
        table.set_model(Arc::new(TwentyRows));

        // 300 px less the 2 px inset on each edge and the 20 px header leaves 276 px: 13 rows.
        let visible = table
            .scrollable_rows()
            .expect("the table implements ScrollableRows")
            .visible_row_count();
        assert_eq!(visible, 13, "a plain table must report the rows that fit");

        table.scrollable_rows_mut().expect("mutable access").ensure_row_visible(5);
        assert_eq!(table.scroll_row(), 0, "row 5 was already inside the viewport");
    }
}

/// RW-58: the suggestion popup is a companion (no capability row), but it owns a row window and
/// therefore answers the same contract every registered row container answers.
#[cfg(all(test, full_widgets))]
mod companion_row_tests {
    use super::*;
    use crate::widget::input_widgets::text_entry::{PopupItem, SuggestionPopup};
    use crate::widget::Widget;

    #[test]
    fn the_suggestion_popup_companion_implements_the_row_contract() {
        let mut popup = SuggestionPopup::new(Rect::new(0, 0, 200, 120));
        popup.set_metrics(4, 20);
        popup
            .set_popup_items((0..10).map(|index| PopupItem::new(format!("row {index}"))).collect());

        let rows = popup.scrollable_rows().expect("the companion opts into the row contract");
        assert_eq!(rows.row_height(), Some(20));
        assert_eq!(rows.scroll_row(), 0);
        assert_eq!(rows.visible_row_count(), 4);

        let rows = popup.scrollable_rows_mut().expect("mutable access");
        rows.scroll_to_row(3);
        assert_eq!(rows.scroll_row(), 3);
        rows.scroll_to_row(9_999);
        assert!(rows.scroll_row() <= 9_999);
    }
}
