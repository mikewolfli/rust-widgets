// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pagination — numbered page navigation for paged content.
//!
//! # Why this is not `Carousel`
//!
//! `Carousel` is a **swipeable container**: it holds the pages and changes which
//! one is visible as the user swipes. This control holds no content at all — it is
//! the *index* for content somebody else owns (typically a table). It knows the
//! total, the page size and the current page, and it shortens a long run of numbers
//! into the `1 … 4 5 6 … 20` shape every platform's table footer uses.
//!
//! The distinction matters because the two have opposite ownership: a page view
//! owns pages, a pagination bar owns only a number.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Ellipsis glyph used to mark a collapsed run of page numbers.
const ELLIPSIS: char = '\u{2026}';

/// One clickable cell in the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// A real page number.
    Page(usize),
    /// A collapsed run; clicking it jumps a screenful in that direction.
    Gap { forward: bool },
    /// The "previous page" control.
    Previous,
    /// The "next page" control.
    Next,
}

/// Numbered page navigation bar.
pub struct Pagination {
    base: BaseWidget,
    /// Total number of items being paged.
    total: usize,
    /// Items per page; always at least 1.
    page_size: usize,
    /// Current page, zero-based.
    page: usize,
    /// How many numbered siblings to show on each side of the current page.
    sibling_count: usize,
    /// When true, the previous/next controls are drawn at the ends.
    show_nav_buttons: bool,
    /// Emitted with the new zero-based page whenever it changes.
    pub page_changed: Signal1<usize>,
}

impl Pagination {
    /// Creates a bar over 0 items with a page size of 10.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Pagination, geometry, "Pagination"),
            total: 0,
            page_size: 10,
            page: 0,
            sibling_count: 1,
            show_nav_buttons: true,
            page_changed: Signal1::new(),
        }
    }

    /// Total number of items being paged.
    pub fn total(&self) -> usize {
        self.total
    }

    /// Sets the total item count, clamping the current page into the new range.
    pub fn set_total(&mut self, total: usize) {
        self.total = total;
        let last = self.last_page();
        if self.page > last {
            self.page = last;
            self.page_changed.emit(last);
        }
        self.base.request_redraw();
    }

    /// Items per page.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Sets the items per page. A size of 0 is treated as 1, because a zero page
    /// size would make the page count infinite.
    pub fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
        let last = self.last_page();
        if self.page > last {
            self.page = last;
            self.page_changed.emit(last);
        }
        self.base.request_redraw();
    }

    /// Current page, zero-based.
    pub fn page(&self) -> usize {
        self.page
    }

    /// Sets the current page, clamped to `0..=last_page()`.
    ///
    /// Emits `page_changed` only when the value actually moves, so a listener
    /// cannot be told about a page the control never displayed.
    pub fn set_page(&mut self, page: usize) {
        let clamped = page.min(self.last_page());
        if clamped == self.page {
            return;
        }
        self.page = clamped;
        self.page_changed.emit(clamped);
        self.base.request_redraw();
    }

    /// Number of pages, at least 1 so an empty set still shows a first page.
    pub fn page_count(&self) -> usize {
        if self.total == 0 {
            return 1;
        }
        self.total.div_ceil(self.page_size)
    }

    /// Index of the last page, i.e. `page_count() - 1`.
    pub fn last_page(&self) -> usize {
        self.page_count() - 1
    }

    /// Whether a previous page exists.
    pub fn has_previous(&self) -> bool {
        self.page > 0
    }

    /// Whether a next page exists.
    pub fn has_next(&self) -> bool {
        self.page < self.last_page()
    }

    /// Moves to the next page. Returns whether the page changed.
    pub fn next_page(&mut self) -> bool {
        if !self.has_next() {
            return false;
        }
        self.set_page(self.page + 1);
        true
    }

    /// Moves to the previous page. Returns whether the page changed.
    pub fn previous_page(&mut self) -> bool {
        if !self.has_previous() {
            return false;
        }
        self.set_page(self.page - 1);
        true
    }

    /// Index of the first item on the current page.
    pub fn first_item_index(&self) -> usize {
        self.page.saturating_mul(self.page_size)
    }

    /// Number of items on the current page, which is short on the last page.
    pub fn item_count_on_page(&self) -> usize {
        self.total.saturating_sub(self.first_item_index()).min(self.page_size)
    }

    /// Numbered siblings shown on each side of the current page.
    pub fn sibling_count(&self) -> usize {
        self.sibling_count
    }

    /// Sets how many numbered siblings flank the current page, clamped to 0..=3
    /// so the bar cannot grow past what a single row can hold legibly.
    pub fn set_sibling_count(&mut self, sibling_count: usize) {
        self.sibling_count = sibling_count.min(3);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the previous/next controls are shown.
    pub fn show_nav_buttons(&self) -> bool {
        self.show_nav_buttons
    }

    /// Sets whether the previous/next controls are shown.
    pub fn set_show_nav_buttons(&mut self, show: bool) {
        self.show_nav_buttons = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The cells to draw, in order, for the current state.
    ///
    /// Kept separate from drawing so the shortening logic can be tested without a
    /// render context: the `1 … 4 5 6 … 20` shape is the whole point of the
    /// control, and it is not observable from pixels.
    pub fn cells(&self) -> Vec<String> {
        self.cell_plan()
            .iter()
            .map(|cell| match cell {
                Cell::Page(index) => (index + 1).to_string(),
                Cell::Gap { .. } => ELLIPSIS.to_string(),
                Cell::Previous => "\u{2039}".to_string(),
                Cell::Next => "\u{203a}".to_string(),
            })
            .collect()
    }

    /// The typed cell plan.
    fn cell_plan(&self) -> Vec<Cell> {
        let last = self.last_page();
        let mut plan = Vec::new();
        if self.show_nav_buttons {
            plan.push(Cell::Previous);
        }
        // A window is a contiguous run of page indices that always contains the
        // current page and both ends when the total is small.
        let window = self.sibling_count * 2 + 1;
        let start =
            self.page.saturating_sub(self.sibling_count).min(last.saturating_sub(window - 1));
        let end = (start + window - 1).min(last);

        if start > 0 {
            plan.push(Cell::Page(0));
            if start > 1 {
                plan.push(Cell::Gap { forward: false });
            }
        }
        for index in start..=end {
            plan.push(Cell::Page(index));
        }
        if end < last {
            if end + 1 < last {
                plan.push(Cell::Gap { forward: true });
            }
            plan.push(Cell::Page(last));
        }
        if self.show_nav_buttons {
            plan.push(Cell::Next);
        }
        plan
    }

    /// Width of one cell, derived from the **drawn bar** so the plan always fits inside the
    /// band the control actually paints.
    fn cell_width(&self) -> u32 {
        let cells = self.cell_plan().len().max(1) as u32;
        (self.bar_rect().width / cells).max(1)
    }

    /// The single-line bar the control actually paints: a full-width band
    /// [`dimensions::PAGINATION_HEIGHT`] tall, centred in the area the control was given.
    ///
    /// # Why the bar is not the control's rectangle
    ///
    /// A pager is a **row of glyph cells**, not a panel. Drawing `geometry()` made a 240x120
    /// census cell a 120 px-tall column of 48 px glyphs — `pagination.svg` carried a 80x120
    /// filled block, a stripe rather than a bar — and the glyph size came from `height * 0.4`,
    /// so the same control showed two different type sizes at two container heights.
    /// [`ControlMetrics::full_width_band`] keeps the full width, takes the bar's own height and
    /// centres it, and this one band is what `cell_width`, the hit test and `draw` all read.
    fn bar_rect(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::PAGINATION_HEIGHT)
    }

    /// The cell under `pos`, or `None` when the point is outside the bar.
    fn cell_at(&self, pos: Point) -> Option<Cell> {
        // The **painted bar**, so the cells a user can see are the ones that answer. Testing
        // `geometry()` would let a click tens of pixels below a 32 px bar change the page.
        let rect = self.bar_rect();
        if pos.x < rect.x
            || pos.x >= rect.x + rect.width as i32
            || pos.y < rect.y
            || pos.y >= rect.y + rect.height as i32
        {
            return None;
        }
        let index = ((pos.x - rect.x) as u32 / self.cell_width()) as usize;
        self.cell_plan().get(index).copied()
    }

    /// Applies a click on `cell`.
    fn activate(&mut self, cell: Cell) {
        match cell {
            Cell::Page(index) => self.set_page(index),
            Cell::Previous => {
                self.previous_page();
            }
            Cell::Next => {
                self.next_page();
            }
            // A gap jumps a whole window, which is what a reader expects from
            // clicking the dots rather than a single page.
            Cell::Gap { forward } => {
                let window = self.sibling_count * 2 + 1;
                if forward {
                    self.set_page(self.page.saturating_add(window));
                } else {
                    self.set_page(self.page.saturating_sub(window));
                }
            }
        }
    }
}

impl Widget for Pagination {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(
            self.cell_width() * self.cell_plan().len().max(1) as u32,
            dimensions::PAGINATION_HEIGHT,
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Pagination`'s property contract.
///
/// Every number here is writable except `page_count` / `last_page`, which are
/// derived from `total` and `page_size`: exposing them as assignments would let a
/// caller set a page count that the item count contradicts.
impl WidgetProperties for Pagination {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "total" => Ok(CapabilityValue::UInt(self.total() as u64)),
            "page_size" => Ok(CapabilityValue::UInt(self.page_size() as u64)),
            "page" => Ok(CapabilityValue::UInt(self.page() as u64)),
            "page_count" => Ok(CapabilityValue::UInt(self.page_count() as u64)),
            "last_page" => Ok(CapabilityValue::UInt(self.last_page() as u64)),
            "sibling_count" => Ok(CapabilityValue::UInt(self.sibling_count() as u64)),
            "show_nav_buttons" => Ok(CapabilityValue::Bool(self.show_nav_buttons())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "total" => {
                self.set_total(expect_usize(value)?);
                Ok(())
            }
            "page_size" => {
                self.set_page_size(expect_usize(value)?);
                Ok(())
            }
            "page" => {
                self.set_page(expect_usize(value)?);
                Ok(())
            }
            "sibling_count" => {
                self.set_sibling_count(expect_usize(value)?);
                Ok(())
            }
            "show_nav_buttons" => {
                self.set_show_nav_buttons(expect_bool(value)?);
                Ok(())
            }
            // Derived from `total` / `page_size`.
            "page_count" | "last_page" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "total",
            "page_size",
            "page",
            "page_count",
            "last_page",
            "sibling_count",
            "show_nav_buttons",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `pagination` publishes.
    ///
    /// `next_page` / `previous_page` carry no payload and have a real return
    /// value, so they execute here. A `false` result means the pager is already
    /// at that end — an argument fault (there is nowhere to go), which is what
    /// `OutOfRange` reports. `set_page` / `set_total` need a number and are
    /// therefore answered through the property route (`page` / `total`).
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "next_page" => {
                if self.next_page() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "previous_page" => {
                if self.previous_page() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "set_page" | "set_total" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Pagination {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(cell) = self.cell_at(*pos) {
                    self.activate(cell);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                // Left / Up and Right / Down step one page.
                37 | 38 => {
                    self.previous_page();
                }
                39 | 40 => {
                    self.next_page();
                }
                36 => self.set_page(0),
                35 => self.set_page(self.last_page()),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for Pagination {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **bar**, not the control's rectangle: see `bar_rect`. Every measurement below —
        // the fill, the cells, the glyph size and the outline — is taken from this one band,
        // and the hit test reads the same one.
        let rect = self.bar_rect();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let style = self.base.style().clone();
        let background = style.background_color.unwrap_or(Color::WHITE);
        let text_color = style.text_color.unwrap_or(Color::BLACK);
        let selected_background = style.border_color.unwrap_or(Color::rgb(60, 90, 160));
        // The glyph size is derived from the **bar's** height, not the control's.
        //
        // The rasteriser paints a glyph **downward** from its origin, so a 48 px font in a
        // 22 px bar reserved the full 48 px: the labels overlapped and the rows below them
        // were painted (and clipped) outside the control. Deriving `height * 0.4` from a 120 px
        // census cell asked for 48 px glyphs in a bar that is now only 32 px tall, so the
        // fraction is taken from the band and capped at `height - 8`, the tallest size that
        // stays inside a page-number cell.
        let font_size =
            (rect.height as f32 * 0.4).max(8.0).min(rect.height.saturating_sub(8).max(8) as f32);
        let font = Font::simple("Sans", font_size);

        context.fill_rect(rect, background);

        let cell_w = self.cell_width();
        for (index, cell) in self.cell_plan().iter().enumerate() {
            let x = rect.x + (index as u32 * cell_w) as i32;
            let cell_rect = Rect::new(x, rect.y, cell_w, rect.height);
            let is_current = matches!(cell, Cell::Page(page) if *page == self.page);
            // The active page and the two navigation ends are the only cells that
            // do anything; drawing them identically to the rest would make the bar
            // look inert.
            let is_affordance = matches!(cell, Cell::Previous | Cell::Next | Cell::Gap { .. });

            if is_current {
                context.fill_rect(cell_rect, selected_background);
            }

            let label = match cell {
                Cell::Page(page) => (page + 1).to_string(),
                Cell::Gap { .. } => ELLIPSIS.to_string(),
                Cell::Previous => "\u{2039}".to_string(),
                Cell::Next => "\u{203a}".to_string(),
            };
            let color = if is_current {
                // The label sits on the filled cell, so it must contrast with **that**
                // fill, not with the bar behind it. Using the bar's own background was
                // only correct while the accent was darker than the surface: on the light
                // theme the accent is `rgb(158,158,158)` and the bar `rgb(240,240,240)`,
                // which measured 2.35:1 — below even the 3:1 large-text floor.
                // `contrast_color` picks whichever of black/white is legible on the fill,
                // which is the same rule the theme's own `Primary` role uses.
                selected_background.contrast_color()
            } else if is_affordance {
                background.blend(&text_color, 0.6)
            } else {
                text_color
            };
            // The origin is the glyph's top-left, so the vertical centre is reached by
            // subtracting half the *measured line box* rather than by halving the cell — the
            // old `cell_rect.y + (cell_rect.height - font_size) / 2` used the font size as a
            // stand-in for the line box and put the label half a line low whenever the two
            // differed.
            let text_width = context.measure_text(&label, &font).width as i32;
            let line = context.text_line(cell_rect, &font);
            let origin =
                Point::new(cell_rect.x + (cell_rect.width as i32 - text_width) / 2, line.y);
            context.draw_text(origin, &label, &font, color, HorizontalAlignment::Left);
        }

        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(210, 210, 210)));
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> Pagination {
        Pagination::new(Rect::new(0, 0, 300, 32))
    }

    #[test]
    fn starts_on_the_first_page() {
        let bar = bar();
        assert_eq!(bar.page(), 0);
        assert_eq!(bar.page_count(), 1, "an empty set still shows one page");
        assert!(!bar.has_previous());
        assert!(!bar.has_next());
    }

    #[test]
    fn page_count_from_total_and_size() {
        let mut bar = bar();
        bar.set_total(95);
        bar.set_page_size(10);
        assert_eq!(bar.page_count(), 10, "95 items at 10 per page is 10 pages");
        assert_eq!(bar.last_page(), 9);
    }

    #[test]
    fn a_partial_last_page_reports_only_its_own_items() {
        let mut bar = bar();
        bar.set_total(95);
        bar.set_page_size(10);
        bar.set_page(9);

        assert_eq!(bar.first_item_index(), 90);
        assert_eq!(bar.item_count_on_page(), 5, "the last page holds the remainder");
    }

    #[test]
    fn set_page_clamps_to_the_range() {
        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);

        bar.set_page(99);
        assert_eq!(bar.page(), 2);
        bar.set_page(0);
        assert_eq!(bar.page(), 0);
    }

    /// Shrinking the data must not leave the bar pointing past the end.
    #[test]
    fn shrinking_the_total_pulls_the_page_back() {
        let mut bar = bar();
        bar.set_total(100);
        bar.set_page_size(10);
        bar.set_page(9);

        bar.set_total(20);
        assert_eq!(bar.page(), 1, "the page is clamped into the new range");
        assert_eq!(bar.page_count(), 2);
    }

    #[test]
    fn a_zero_page_size_is_treated_as_one() {
        let mut bar = bar();
        bar.set_page_size(0);
        assert_eq!(bar.page_size(), 1);
    }

    #[test]
    fn next_and_previous_report_whether_they_moved() {
        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);

        assert!(!bar.previous_page(), "already on the first page");
        assert!(bar.next_page());
        assert_eq!(bar.page(), 1);
        assert!(bar.previous_page());
        assert_eq!(bar.page(), 0);

        bar.set_page(2);
        assert!(!bar.next_page(), "already on the last page");
    }

    /// A long run must collapse, otherwise 100 pages would produce 100 cells.
    #[test]
    fn a_long_run_collapses_into_gaps() {
        let mut bar = bar();
        bar.set_total(1000);
        bar.set_page_size(10);
        bar.set_page(49);

        let cells = bar.cells();
        assert!(cells.contains(&ELLIPSIS.to_string()), "the run must be shortened: {cells:?}");
        assert!(
            cells.len() < 12,
            "the bar must stay a single row, got {} cells: {cells:?}",
            cells.len()
        );
        // First and last pages stay reachable no matter how long the run is.
        assert_eq!(cells.get(1), Some(&"1".to_string()));
        assert_eq!(cells.last().map(String::as_str), Some("\u{203a}"));
    }

    /// A run short enough to fit must show every page, with no ellipsis.
    #[test]
    fn a_short_run_shows_every_page() {
        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);

        let cells = bar.cells();
        assert_eq!(cells, vec!["\u{2039}", "1", "2", "3", "\u{203a}"]);
    }

    /// The current page must always be one of the drawn cells.
    #[test]
    fn the_current_page_is_always_drawn() {
        let mut bar = bar();
        bar.set_total(1000);
        bar.set_page_size(10);

        for page in [0usize, 1, 5, 49, 98, 99] {
            bar.set_page(page);
            let expected = (bar.page() + 1).to_string();
            assert!(
                bar.cells().contains(&expected),
                "page {} must be visible in {:?}",
                bar.page() + 1,
                bar.cells()
            );
        }
    }

    /// Clicking a numbered cell must navigate to it.
    #[test]
    fn clicking_a_page_number_navigates() {
        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);

        let cell_w = bar.cell_width();
        // Cell 0 is Previous, so cell 2 is page 2 (index 1).
        let target_x = bar.geometry().x + (2 * cell_w) as i32 + 1;
        bar.handle_event(&Event::mouse_press(target_x, 10, 1));
        assert_eq!(bar.page(), 1, "clicking the second number selects page 2");
    }

    /// Clicking a gap must jump a whole window rather than one page.
    #[test]
    fn clicking_a_gap_jumps_a_window() {
        let mut bar = bar();
        bar.set_total(1000);
        bar.set_page_size(10);
        bar.set_page(50);

        let plan = bar.cell_plan();
        let gap_index = plan
            .iter()
            .position(|cell| matches!(cell, Cell::Gap { forward: true }))
            .expect("a forward gap must exist mid-run");
        let cell_w = bar.cell_width();
        let x = bar.geometry().x + (gap_index as u32 * cell_w) as i32 + 1;
        bar.handle_event(&Event::mouse_press(x, 10, 1));

        assert_eq!(bar.page(), 53, "a gap jumps past the current window");
    }

    /// The current page must announce itself exactly once per change.
    #[test]
    fn page_changed_fires_only_on_a_real_change() {
        use std::sync::{Arc, Mutex};

        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);
        let seen = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&seen);
        bar.page_changed.connect(move |page| {
            sink.lock().expect("signal sink poisoned").push(*page);
        });

        bar.set_page(1);
        bar.set_page(1);
        bar.set_page(2);

        let recorded = seen.lock().expect("signal sink poisoned").clone();
        assert_eq!(recorded, vec![1, 2], "a no-op write must not emit");
    }

    /// Arrow keys must navigate, so the bar is usable without a pointer.
    #[test]
    fn arrow_keys_navigate() {
        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);

        bar.handle_event(&Event::key_press(39, 0));
        assert_eq!(bar.page(), 1, "Right advances");
        bar.handle_event(&Event::key_press(37, 0));
        assert_eq!(bar.page(), 0, "Left retreats");

        bar.handle_event(&Event::key_press(35, 0));
        assert_eq!(bar.page(), 2, "End jumps to the last page");
        bar.handle_event(&Event::key_press(36, 0));
        assert_eq!(bar.page(), 0, "Home jumps to the first page");
    }

    /// The previous/next controls must be omittable, which is what a compact
    /// footer needs.
    #[test]
    fn nav_buttons_can_be_hidden() {
        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);
        bar.set_show_nav_buttons(false);

        let cells = bar.cells();
        assert_eq!(cells, vec!["1", "2", "3"], "no nav ends when they are hidden");
    }

    #[test]
    fn sibling_count_is_clamped() {
        let mut bar = bar();
        bar.set_sibling_count(99);
        assert_eq!(bar.sibling_count(), 3, "the bar must stay one row");

        bar.set_sibling_count(0);
        assert_eq!(bar.sibling_count(), 0);
    }

    /// A click outside the bar must do nothing.
    #[test]
    fn a_click_outside_the_bar_is_ignored() {
        let mut bar = bar();
        bar.set_total(30);
        bar.set_page_size(10);

        bar.handle_event(&Event::mouse_press(500, 500, 1));
        assert_eq!(bar.page(), 0);
        bar.handle_event(&Event::mouse_press(1, 5, 2));
        assert_eq!(bar.page(), 0, "a non-primary button must not page");
    }

    #[test]
    fn properties_round_trip() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };

        let mut bar = bar();
        widget_property_set(&mut bar, "total", CapabilityValue::UInt(95)).expect("total");
        widget_property_set(&mut bar, "page_size", CapabilityValue::UInt(10)).expect("page_size");
        widget_property_set(&mut bar, "page", CapabilityValue::UInt(3)).expect("page");

        assert_eq!(widget_property_get(&bar, "total"), Ok(CapabilityValue::UInt(95)));
        assert_eq!(widget_property_get(&bar, "page"), Ok(CapabilityValue::UInt(3)));
        assert_eq!(widget_property_get(&bar, "page_count"), Ok(CapabilityValue::UInt(10)));
        assert_eq!(widget_property_get(&bar, "last_page"), Ok(CapabilityValue::UInt(9)));

        assert_eq!(
            widget_property_set(&mut bar, "page_count", CapabilityValue::UInt(1)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }
}
