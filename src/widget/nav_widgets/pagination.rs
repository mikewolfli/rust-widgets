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

use crate::compat::{format, String, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, mouse_button, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Ellipsis glyph used to mark a collapsed run of page numbers.
const ELLIPSIS: char = '\u{2026}';

/// Suggested width of the trailing page-size selector cell, in pixels.
const PAGE_SIZE_CELL_WIDTH: u32 = 84;

/// Suggested width of the trailing jump-to-page cell, in pixels.
const JUMP_CELL_WIDTH: u32 = 64;

/// Height of one row in the page-size popup, in pixels.
const PAGE_SIZE_ROW_HEIGHT: u32 = 24;

/// Vertical gap between the page-size cell and its popup, in pixels.
const PAGE_SIZE_POPUP_GAP: i32 = 2;

/// Most digits the jump buffer accepts, enough for any realistic page index.
const JUMP_INPUT_MAX_DIGITS: usize = 6;

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
    /// When true, a trailing jump-to-page cell is shown.
    show_jump_input: bool,
    /// When true, the jump cell holds focus and keystrokes build `jump_buffer`.
    jump_editing: bool,
    /// Digits typed into the jump cell.
    jump_buffer: String,
    /// When true, a trailing page-size selector cell is shown.
    show_page_size_selector: bool,
    /// Normalized, ascending options the selector offers.
    page_size_options: Vec<usize>,
    /// When true, the page-size popup is visible.
    page_size_menu_open: bool,
    /// Index of the option under the pointer while the popup is open.
    page_size_hover: Option<usize>,
    /// Emitted with the new zero-based page whenever it changes.
    pub page_changed: Signal1<usize>,
}

/// The bar's resolved geometry: the numbered band plus the optional trailing cells.
///
/// `draw`, `cell_at`, `size_hint` and the page-size popup all read this one value, so
/// the cells that are painted are exactly the cells that answer a click.
struct BarLayout {
    /// The band the numbered cells share.
    numbered_rect: Rect,
    /// The page-size selector cell, when it is shown and has options.
    page_size_rect: Option<Rect>,
    /// The jump-to-page cell, when it is shown.
    jump_rect: Option<Rect>,
}

impl BarLayout {
    /// Width of one numbered cell, at least 1 px so a narrow bar still divides.
    fn cell_width(&self, cell_count: usize) -> u32 {
        (self.numbered_rect.width / cell_count.max(1) as u32).max(1)
    }
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
            show_jump_input: false,
            jump_editing: false,
            jump_buffer: String::new(),
            show_page_size_selector: false,
            page_size_options: Vec::new(),
            page_size_menu_open: false,
            page_size_hover: None,
            page_changed: Signal1::new(),
        }
    }

    /// Total number of items being paged.
    pub fn total(&self) -> usize {
        self.total
    }

    /// Sets the total item count, clamping the current page into the new range.
    pub fn set_total(&mut self, total: usize) {
        if self.total == total {
            return;
        }
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
        let page_size = page_size.max(1);
        if self.page_size == page_size {
            return;
        }
        self.page_size = page_size;
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

    /// A one-line summary of the visible range, e.g. `11\u{2013}20 of 95`.
    ///
    /// Provided as data rather than painted unconditionally: a pager that already sits under a
    /// table does not need a second statement of the same range, so a host that wants the
    /// summary draws it itself. An empty set reports `0 of 0` rather than a negative range.
    pub fn summary_text(&self) -> String {
        if self.total == 0 {
            return "0 of 0".to_string();
        }
        let first = self.first_item_index() + 1;
        let last = (self.first_item_index() + self.item_count_on_page()).min(self.total);
        format!("{first}\u{2013}{last} of {}", self.total)
    }

    /// Numbered siblings shown on each side of the current page.
    pub fn sibling_count(&self) -> usize {
        self.sibling_count
    }

    /// Sets how many numbered siblings flank the current page, clamped to 0..=3
    /// so the bar cannot grow past what a single row can hold legibly.
    pub fn set_sibling_count(&mut self, sibling_count: usize) {
        let sibling_count = sibling_count.min(3);
        if self.sibling_count == sibling_count {
            return;
        }
        self.sibling_count = sibling_count;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the previous/next controls are shown.
    pub fn show_nav_buttons(&self) -> bool {
        self.show_nav_buttons
    }

    /// Sets whether the previous/next controls are shown.
    pub fn set_show_nav_buttons(&mut self, show: bool) {
        if self.show_nav_buttons == show {
            return;
        }
        self.show_nav_buttons = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the trailing jump-to-page cell is shown.
    pub fn show_jump_input(&self) -> bool {
        self.show_jump_input
    }

    /// Shows or hides the trailing jump-to-page cell.
    ///
    /// Hiding the cell also leaves any in-progress edit, so a later show starts clean.
    pub fn set_show_jump_input(&mut self, show: bool) {
        if self.show_jump_input == show {
            return;
        }
        self.show_jump_input = show;
        if !show {
            self.jump_editing = false;
            self.jump_buffer.clear();
        }
        self.base.request_redraw();
    }

    /// Whether the jump cell currently has focus and accepts digits.
    pub fn is_jump_editing(&self) -> bool {
        self.jump_editing
    }

    /// Whether the trailing page-size selector cell is shown.
    pub fn show_page_size_selector(&self) -> bool {
        self.show_page_size_selector
    }

    /// Shows or hides the trailing page-size selector cell.
    ///
    /// Hiding the cell also closes its popup, so a later show starts closed.
    pub fn set_show_page_size_selector(&mut self, show: bool) {
        if self.show_page_size_selector == show {
            return;
        }
        self.show_page_size_selector = show;
        if !show {
            self.page_size_menu_open = false;
            self.page_size_hover = None;
        }
        self.base.request_redraw();
    }

    /// Whether the page-size popup is currently open.
    pub fn is_page_size_menu_open(&self) -> bool {
        self.page_size_menu_open
    }

    /// The normalized options the page-size selector offers.
    ///
    /// Options are deduped and sorted ascending, and a zero option is dropped, so the
    /// popup's row order cannot depend on how the caller happened to build the list.
    pub fn page_size_options(&self) -> &[usize] {
        &self.page_size_options
    }

    /// Sets the options the page-size selector offers.
    ///
    /// Zero is dropped because it is not a valid page size; the current `page_size`
    /// need not appear in the list.
    pub fn set_page_size_options(&mut self, options: Vec<usize>) {
        let mut normalized: Vec<usize> = options.into_iter().filter(|size| *size > 0).collect();
        normalized.sort_unstable();
        normalized.dedup();
        if self.page_size_options == normalized {
            return;
        }
        self.page_size_options = normalized;
        if self.page_size_options.is_empty() {
            self.page_size_menu_open = false;
            self.page_size_hover = None;
        } else if self.page_size_hover.is_some_and(|index| index >= self.page_size_options.len()) {
            self.page_size_hover = None;
        }
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

    /// Width of one numbered cell, derived from the **drawn bar** and the one
    /// [`Self::layout`] derivation so the plan always fits inside the band the control
    /// actually paints.
    fn cell_width(&self) -> u32 {
        self.layout().cell_width(self.cell_plan().len())
    }

    /// The bar's one geometry derivation: the numbered band plus the optional trailing
    /// controls.
    ///
    /// # Layout order
    ///
    /// The numbered cells keep the leading band. The page-size selector takes the
    /// rightmost trailing cell and the jump cell sits immediately to its left, so the
    /// ordering left-to-right is `numbers … [Go] [N / page]`. Each trailing cell asks
    /// for its suggested width but is capped at 40% of the bar (and at whatever is left),
    /// which keeps the numbers legible on a narrow bar. With both features off the
    /// numbered band is the whole bar, so every existing measurement is unchanged.
    fn layout(&self) -> BarLayout {
        let bar = self.bar_rect();
        let left = bar.x;
        let mut right = bar.x.saturating_add(bar.width as i32);
        // Each trailing cell may claim at most two fifths of the bar.
        let trailing_cap = bar.width.saturating_mul(2) / 5;

        let mut page_size_rect = None;
        if self.show_page_size_selector && !self.page_size_options.is_empty() {
            let available = (right - left).max(0) as u32;
            let width = PAGE_SIZE_CELL_WIDTH.min(trailing_cap).min(available);
            if width > 0 {
                right = right.saturating_sub(width as i32);
                page_size_rect = Some(Rect::new(right, bar.y, width, bar.height));
            }
        }

        let mut jump_rect = None;
        if self.show_jump_input {
            let available = (right - left).max(0) as u32;
            let width = JUMP_CELL_WIDTH.min(trailing_cap).min(available);
            if width > 0 {
                right = right.saturating_sub(width as i32);
                jump_rect = Some(Rect::new(right, bar.y, width, bar.height));
            }
        }

        BarLayout {
            numbered_rect: Rect::new(left, bar.y, (right - left).max(0) as u32, bar.height),
            page_size_rect,
            jump_rect,
        }
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

    /// The cell under `pos`, or `None` when the point is outside the numbered band.
    fn cell_at(&self, pos: Point) -> Option<Cell> {
        // The **painted numbered band**, so the cells a user can see are the ones that
        // answer. Reading the whole bar would let a click on the jump or page-size cell
        // fall through to a page number.
        let layout = self.layout();
        let rect = layout.numbered_rect;
        if !rect.contains_point(pos) {
            return None;
        }
        let index = ((pos.x - rect.x) as u32 / self.cell_width()) as usize;
        self.cell_plan().get(index).copied()
    }

    /// Whether `pos` is inside the jump cell, when that cell exists.
    fn jump_cell_contains(&self, pos: Point) -> bool {
        self.layout().jump_rect.is_some_and(|rect| rect.contains_point(pos))
    }

    /// Whether `pos` is inside the page-size selector cell, when that cell exists.
    fn page_size_cell_contains(&self, pos: Point) -> bool {
        self.layout().page_size_rect.is_some_and(|rect| rect.contains_point(pos))
    }

    /// The popup rectangle, anchored to the page-size cell.
    ///
    /// It opens **above** the cell when the control's own geometry has room, otherwise
    /// **below** the bar. The choice is documented because a pager is commonly the last
    /// row in a table footer: preferring above keeps the menu off the content below, and
    /// falling back below keeps it attached to the cell when there is no room above.
    /// Horizontally it keeps the cell's columns, so it can never cover the jump cell.
    fn page_size_popup_rect(&self) -> Option<Rect> {
        if !self.page_size_menu_open || self.page_size_options.is_empty() {
            return None;
        }
        let cell = self.layout().page_size_rect?;
        let height = self.page_size_options.len() as u32 * PAGE_SIZE_ROW_HEIGHT;
        let geometry = self.geometry();
        let above = cell.y.saturating_sub(PAGE_SIZE_POPUP_GAP).saturating_sub(height as i32);
        let y = if above >= geometry.y {
            above
        } else {
            cell.y.saturating_add(cell.height as i32).saturating_add(PAGE_SIZE_POPUP_GAP)
        };
        Some(Rect::new(cell.x, y, cell.width.max(1), height.max(1)))
    }

    /// The option row at `index` inside `popup`.
    fn page_size_option_rect(&self, popup: Rect, index: usize) -> Rect {
        Rect::new(
            popup.x,
            popup.y + (index as u32 * PAGE_SIZE_ROW_HEIGHT) as i32,
            popup.width,
            PAGE_SIZE_ROW_HEIGHT,
        )
    }

    /// The option index under `pos`, or `None` when the popup is closed or the point is
    /// on a gap between/around the rows.
    fn page_size_option_at(&self, pos: Point) -> Option<usize> {
        let popup = self.page_size_popup_rect()?;
        if !popup.contains_point(pos) {
            return None;
        }
        let row = ((pos.y - popup.y) as u32 / PAGE_SIZE_ROW_HEIGHT) as usize;
        (row < self.page_size_options.len()).then_some(row)
    }

    /// Opens or closes the page-size popup.
    fn toggle_page_size_menu(&mut self) {
        if self.page_size_options.is_empty() {
            return;
        }
        self.page_size_menu_open = !self.page_size_menu_open;
        self.page_size_hover = None;
        self.base.request_redraw();
    }

    /// Closes the page-size popup, if it is open.
    fn close_page_size_menu(&mut self) {
        if !self.page_size_menu_open && self.page_size_hover.is_none() {
            return;
        }
        self.page_size_menu_open = false;
        self.page_size_hover = None;
        self.base.request_redraw();
    }

    /// Applies a primary press to the correct cell.
    fn handle_press(&mut self, pos: Point) {
        // The popup behaves like a small modal menu while it is open: an option applies,
        // the selector toggles, and any other press dismisses it before the bar handles
        // the same press normally.
        if self.page_size_menu_open {
            if let Some(index) = self.page_size_option_at(pos) {
                let option = self.page_size_options[index];
                self.page_size_menu_open = false;
                self.page_size_hover = None;
                self.set_page_size(option);
                self.base.request_redraw();
                return;
            }
            if self.page_size_cell_contains(pos) {
                self.close_page_size_menu();
                return;
            }
            self.close_page_size_menu();
        }

        // A press outside the jump cell ends the edit but still reaches the bar, so
        // clicking a page number both cancels and navigates.
        if self.jump_editing && !self.jump_cell_contains(pos) {
            self.jump_editing = false;
            self.jump_buffer.clear();
            self.base.request_redraw();
        }

        if self.jump_cell_contains(pos) {
            if !self.jump_editing {
                self.jump_editing = true;
                self.jump_buffer.clear();
                self.base.request_redraw();
            }
            return;
        }

        if self.page_size_cell_contains(pos) {
            self.toggle_page_size_menu();
            return;
        }

        if let Some(cell) = self.cell_at(pos) {
            self.activate(cell);
        }
    }

    /// Handles a key press, including the jump cell's editing keys.
    fn handle_key(&mut self, key: u32) {
        if key == key_codes::ESCAPE {
            let mut handled = false;
            if self.jump_editing {
                self.jump_editing = false;
                self.jump_buffer.clear();
                handled = true;
            }
            if self.page_size_menu_open {
                self.page_size_menu_open = false;
                self.page_size_hover = None;
                handled = true;
            }
            if handled {
                self.base.request_redraw();
                return;
            }
        }

        if self.jump_editing {
            self.handle_jump_key(key);
            return;
        }

        // An open popup owns the keyboard until it is dismissed.
        if self.page_size_menu_open {
            return;
        }

        // Logical keys rather than raw key codes: the numeric codes are QWERTY
        // positions, so on AZERTY and other layouts the arrows and Home/End landed on
        // unrelated keys (or nowhere). `Key::from_key_code` is the crate's one mapping
        // from a platform code to the logical key.
        use crate::shortcut::Key;
        match Key::from_key_code(key) {
            Some(Key::Left) | Some(Key::Up) => {
                self.previous_page();
            }
            Some(Key::Right) | Some(Key::Down) => {
                self.next_page();
            }
            Some(Key::Home) | Some(Key::PageUp) => self.set_page(0),
            Some(Key::End) | Some(Key::PageDown) => self.set_page(self.last_page()),
            _ => {}
        }
    }

    /// Handles one key while the jump cell is focused.
    fn handle_jump_key(&mut self, key: u32) {
        if key == key_codes::ENTER || key == key_codes::LINE_FEED {
            self.commit_jump();
            return;
        }
        if key == key_codes::BACKSPACE {
            if self.jump_buffer.pop().is_some() {
                self.base.request_redraw();
            }
            return;
        }
        // Digits are looked up through the crate's ASCII mapping instead of a literal
        // range, so the one key-code convention lives in `key_codes`.
        if let Some(ch) = ('0'..='9').find(|ch| key_codes::ascii_key_code(*ch) == Some(key)) {
            if self.jump_buffer.len() < JUMP_INPUT_MAX_DIGITS {
                self.jump_buffer.push(ch);
                self.base.request_redraw();
            }
        }
    }

    /// Commits the jump buffer as a 1-based page number.
    ///
    /// An empty buffer or zero is ignored and leaves the cell editing; a number past the
    /// end is clamped by `set_page`, which is the same rule every other page write uses.
    fn commit_jump(&mut self) {
        let Ok(value) = self.jump_buffer.parse::<usize>() else {
            return;
        };
        if value == 0 {
            return;
        }
        self.jump_editing = false;
        self.jump_buffer.clear();
        self.set_page(value - 1);
        self.base.request_redraw();
    }

    /// Draws the jump cell: the typed buffer while editing, otherwise a short hint.
    fn draw_jump_cell(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        font: &Font,
        color: Color,
        outline: Color,
    ) {
        context.draw_rect(rect, outline);
        let mut buffer = String::new();
        let label = if self.jump_editing {
            // The caret is a plain underscore, so the cell reads as an input without
            // needing a second focus indicator in a 32 px row.
            buffer.push_str(&self.jump_buffer);
            buffer.push('_');
            buffer.as_str()
        } else {
            "Go"
        };
        draw_centered_label(context, rect, label, font, color);
    }

    /// Draws the page-size selector cell: the current size plus a chevron affordance.
    fn draw_page_size_cell(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        font: &Font,
        color: Color,
        outline: Color,
    ) {
        context.draw_rect(rect, outline);
        let arrow_side = (rect.height / 3).clamp(6, 12);
        let arrow_box_width = (arrow_side + 4).min(rect.width);
        let arrow_rect = Rect::new(
            rect.x + rect.width.saturating_sub(arrow_box_width) as i32,
            rect.y,
            arrow_box_width,
            rect.height,
        );
        // A `ChevronDown` **icon outline** rather than the `▾` text glyph: no bundled
        // face covers U+25BE, so the glyph would draw as a bitmap block.
        crate::widget::draw_icon_centered(
            context,
            arrow_rect,
            arrow_side,
            color,
            IconName::ChevronDown,
        );
        let text_rect =
            Rect::new(rect.x, rect.y, rect.width.saturating_sub(arrow_box_width), rect.height);
        let label = format!("{} / page", self.page_size);
        draw_centered_label(context, text_rect, &label, font, color);
    }

    /// Draws the page-size popup and its option rows.
    fn draw_page_size_popup(
        &self,
        context: &mut RenderContext,
        popup: Rect,
        font: &Font,
        color: Color,
        background: Color,
        outline: Color,
    ) {
        use core::fmt::Write as _;
        context.fill_rect(popup, background);
        let mut label = String::new();
        for (index, option) in self.page_size_options.iter().enumerate() {
            let row = self.page_size_option_rect(popup, index);
            if self.page_size_hover == Some(index) {
                context.fill_rect(row, background.blend(&color, 0.12));
            }
            label.clear();
            let _ = write!(label, "{option}");
            draw_centered_label(context, row, &label, font, color);
        }
        context.draw_rect(popup, outline);
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let layout = self.layout();
        let cell_count = self.cell_plan().len().max(1);
        let width = layout.cell_width(cell_count) * cell_count as u32
            + layout.page_size_rect.map_or(0, |rect| rect.width)
            + layout.jump_rect.map_or(0, |rect| rect.width);
        Size::new(width, dimensions::PAGINATION_HEIGHT)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "page_changed" => {
                Some(EventSignalRef::mapped("page_changed", &self.page_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
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
            Event::MousePress { pos, button, .. } if *button == mouse_button::PRIMARY => {
                self.handle_press(*pos);
            }
            Event::MouseMove { pos } => {
                if self.page_size_menu_open {
                    let hover = self.page_size_option_at(*pos);
                    if self.page_size_hover != hover {
                        self.page_size_hover = hover;
                        self.base.request_redraw();
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                self.handle_key(*key);
            }
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

        let style = self.base.style();
        let background = style.background_color.unwrap_or(Color::WHITE);
        let text_color = style.text_color.unwrap_or(Color::BLACK);
        let selected_background = style.border_color.unwrap_or(Color::rgb(60, 90, 160));
        let outline = style.border_color.unwrap_or(Color::rgb(210, 210, 210));
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
        // The **effective font** — the resolved theme/caller font — when the style names one, else
        // the bar-height-derived face this control has always used, so a themed font or text scale
        // reaches the page numbers (D09-STYLE-01).
        let fallback_font = Font::simple("Sans", font_size);
        let font = effective_font(style);
        let font = if style.font.is_some() { font } else { &fallback_font };

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        let layout = self.layout();
        let plan = self.cell_plan();
        let cell_w = layout.cell_width(plan.len());
        // One reused label buffer for the whole bar. The previous per-cell
        // `to_string()`/`String::new()` allocated for every page number on every paint;
        // a pagination bar repaints whenever the table scrolls.
        use core::fmt::Write as _;
        let mut label = String::new();
        for (index, cell) in plan.iter().enumerate() {
            let x = layout.numbered_rect.x + (index as u32 * cell_w) as i32;
            let cell_rect = Rect::new(x, rect.y, cell_w, rect.height);
            let is_current = matches!(cell, Cell::Page(page) if *page == self.page);
            // The active page and the two navigation ends are the only cells that
            // do anything; drawing them identically to the rest would make the bar
            // look inert.
            let is_affordance = matches!(cell, Cell::Previous | Cell::Next | Cell::Gap { .. });

            if is_current {
                context.fill_rect(cell_rect, selected_background);
            }

            // The previous/next ends are `ChevronLeft`/`ChevronRight` **icon outlines**, not the
            // `‹`/`›` text glyphs (U+2039/U+203A are covered by no bundled face, so those drew as
            // 8x8 bitmap blocks). Page numbers and the ellipsis stay text, because they *are* text.
            let nav_icon = match cell {
                Cell::Previous => Some(IconName::ChevronLeft),
                Cell::Next => Some(IconName::ChevronRight),
                _ => None,
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
            if let Some(icon) = nav_icon {
                let line = context.text_line(cell_rect, font);
                let side = line.height.max(1);
                let icon_rect = Rect::new(
                    cell_rect.x + (cell_rect.width as i32 - side as i32) / 2,
                    line.y,
                    side,
                    side,
                );
                crate::widget::draw_icon_at(context, icon_rect, color, icon);
                continue;
            }
            let label = match cell {
                Cell::Page(page) => {
                    label.clear();
                    let _ = write!(label, "{}", page + 1);
                    label.as_str()
                }
                Cell::Gap { .. } => {
                    label.clear();
                    label.push(ELLIPSIS);
                    label.as_str()
                }
                // Handled above; unreachable, but the match must stay total.
                Cell::Previous | Cell::Next => "",
            };
            // The origin is the glyph's top-left, so the vertical centre is reached by
            // subtracting half the *measured line box* rather than by halving the cell — the
            // old `cell_rect.y + (cell_rect.height - font_size) / 2` used the font size as a
            // stand-in for the line box and put the label half a line low whenever the two
            // differed.
            let text_width = context.measure_text(label, font).width as i32;
            let line = context.text_line(cell_rect, font);
            let origin =
                Point::new(cell_rect.x + (cell_rect.width as i32 - text_width) / 2, line.y);
            context.draw_text(origin, label, font, color, HorizontalAlignment::Left);
        }

        if let Some(jump_rect) = layout.jump_rect {
            self.draw_jump_cell(context, jump_rect, font, text_color, outline);
        }
        if let Some(page_size_rect) = layout.page_size_rect {
            self.draw_page_size_cell(context, page_size_rect, font, text_color, outline);
        }
        if let Some(popup) = self.page_size_popup_rect() {
            self.draw_page_size_popup(context, popup, font, text_color, background, outline);
        }

        context.draw_rect(rect, outline);
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

/// Draws `label` horizontally centred in `rect`, using the same line-box rule the
/// numbered cells use.
fn draw_centered_label(
    context: &mut RenderContext,
    rect: Rect,
    label: &str,
    font: &Font,
    color: Color,
) {
    let text_width = context.measure_text(label, font).width as i32;
    let line = context.text_line(rect, font);
    let origin = Point::new(rect.x + (rect.width as i32 - text_width) / 2, line.y);
    context.draw_text(origin, label, font, color, HorizontalAlignment::Left);
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

    #[test]
    fn the_summary_names_the_visible_item_range() {
        let mut bar = bar();
        bar.set_total(95);
        assert_eq!(bar.summary_text(), "1\u{2013}10 of 95");
        bar.set_page(9);
        assert_eq!(bar.summary_text(), "91\u{2013}95 of 95");
    }

    #[test]
    fn page_up_and_page_down_are_logical_keys_not_positions() {
        let mut bar = bar();
        bar.set_total(95);
        bar.set_page(4);
        bar.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_eq!(bar.page(), 9, "Page Down goes to the last page");
        bar.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert_eq!(bar.page(), 0, "Page Up goes to the first page");
    }

    // ── Jump input and page-size selector ──

    /// Types one ASCII digit through the crate's key-code mapping.
    fn press_digit(bar: &mut Pagination, digit: char) {
        let key = crate::event::key_codes::ascii_key_code(digit).expect("ASCII digit");
        bar.handle_event(&Event::key_press(key, 0));
    }

    /// A primary click at a point.
    fn click(bar: &mut Pagination, pos: Point) {
        bar.handle_event(&Event::mouse_press(pos.x, pos.y, 1));
    }

    /// A point comfortably inside `rect`.
    fn inside(rect: Rect) -> Point {
        Point::new(rect.x + 2, rect.y + 2)
    }

    fn configured_bar() -> Pagination {
        let mut bar = bar();
        bar.set_total(200);
        bar.set_page_size(10);
        bar
    }

    /// Both features are off by default, so every existing measurement and label list is
    /// untouched until a caller opts in.
    #[test]
    fn the_new_cells_are_off_by_default() {
        let bar = configured_bar();

        assert!(!bar.show_jump_input());
        assert!(!bar.show_page_size_selector());
        assert!(!bar.is_jump_editing());
        assert!(!bar.is_page_size_menu_open());
        assert!(bar.page_size_options().is_empty());
        assert_eq!(
            bar.cells(),
            vec!["\u{2039}", "1", "2", "3", "\u{2026}", "20", "\u{203a}"],
            "cells() must keep its old page/gap/nav label set"
        );

        let layout = bar.layout();
        assert_eq!(layout.jump_rect, None);
        assert_eq!(layout.page_size_rect, None);
        assert_eq!(layout.numbered_rect, bar.bar_rect());
        assert_eq!(bar.cell_width(), bar.bar_rect().width / bar.cells().len() as u32);
        assert_eq!(
            bar.size_hint(),
            Size::new(bar.cell_width() * bar.cells().len() as u32, dimensions::PAGINATION_HEIGHT)
        );
    }

    /// The jump cell must live in its own trailing band, outside every numbered cell.
    #[test]
    fn the_jump_cell_is_outside_the_numbered_cells() {
        let mut bar = configured_bar();
        bar.set_show_jump_input(true);
        let layout = bar.layout();
        let jump = layout.jump_rect.expect("the jump cell exists when shown");
        let bar_rect = bar.bar_rect();

        assert!(jump.width > 0);
        assert!(jump.width <= bar_rect.width * 2 / 5, "clamped to at most 40% of the bar");
        assert_eq!(jump.x + jump.width as i32, bar_rect.x + bar_rect.width as i32);
        assert!(layout.numbered_rect.x + layout.numbered_rect.width as i32 <= jump.x);

        // A click on the jump cell focuses it and must never change the page.
        click(&mut bar, inside(jump));
        assert!(bar.is_jump_editing());
        assert_eq!(bar.page(), 0);
    }

    /// Digits plus Enter are read as a 1-based page and navigate there.
    #[test]
    fn typing_digits_and_enter_jumps() {
        let mut bar = configured_bar();
        bar.set_show_jump_input(true);
        let jump = bar.layout().jump_rect.expect("jump cell");
        click(&mut bar, inside(jump));

        press_digit(&mut bar, '1');
        press_digit(&mut bar, '2');
        assert_eq!(bar.jump_buffer, "12");
        bar.handle_event(&Event::key_press(crate::event::key_codes::ENTER, 0));

        assert!(bar.jump_buffer.is_empty(), "a committed buffer starts clean next time");
        assert!(!bar.is_jump_editing());
        assert_eq!(bar.page(), 11, "page 12 is index 11");
    }

    /// An empty buffer or zero stays editing; a number past the end is clamped, never
    /// allowed out of range.
    #[test]
    fn zero_and_empty_jumps_are_ignored_and_out_of_range_clamps() {
        let mut bar = configured_bar();
        bar.set_show_jump_input(true);
        let jump = bar.layout().jump_rect.expect("jump cell");
        click(&mut bar, inside(jump));

        bar.handle_event(&Event::key_press(crate::event::key_codes::ENTER, 0));
        assert!(bar.is_jump_editing(), "an empty buffer stays editing");
        assert_eq!(bar.page(), 0);

        press_digit(&mut bar, '0');
        bar.handle_event(&Event::key_press(crate::event::key_codes::LINE_FEED, 0));
        assert!(bar.is_jump_editing(), "zero stays editing");
        assert_eq!(bar.page(), 0);

        press_digit(&mut bar, '9');
        press_digit(&mut bar, '9');
        bar.handle_event(&Event::key_press(crate::event::key_codes::ENTER, 0));
        assert!(!bar.is_jump_editing());
        assert_eq!(bar.page(), bar.last_page(), "a page past the end clamps to the last");
    }

    /// Escape and a press outside the jump cell both abandon the edit.
    #[test]
    fn escape_and_an_outside_press_cancel_jump_editing() {
        let mut bar = configured_bar();
        bar.set_show_jump_input(true);
        let jump = bar.layout().jump_rect.expect("jump cell");

        click(&mut bar, inside(jump));
        press_digit(&mut bar, '3');
        bar.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert!(!bar.is_jump_editing());
        assert!(bar.jump_buffer.is_empty());
        assert_eq!(bar.page(), 0, "Escape must not jump");

        // A press outside the cell cancels the edit but still reaches the bar.
        click(&mut bar, inside(jump));
        press_digit(&mut bar, '3');
        let layout = bar.layout();
        let cell_w = layout.cell_width(bar.cell_plan().len());
        let page_two = Point::new(
            layout.numbered_rect.x + (2 * cell_w) as i32 + 1,
            layout.numbered_rect.y + 2,
        );
        click(&mut bar, page_two);
        assert!(!bar.is_jump_editing());
        assert!(bar.jump_buffer.is_empty());
        assert_eq!(bar.page(), 1, "the same press also selects page 2");
    }

    /// Backspace edits the buffer and it is capped so a huge paste cannot overflow the
    /// cell. (Digits only, so the six-character cap is the whole guard.)
    #[test]
    fn the_jump_buffer_accepts_digits_and_backspace() {
        let mut bar = configured_bar();
        bar.set_show_jump_input(true);
        let jump = bar.layout().jump_rect.expect("jump cell");
        click(&mut bar, inside(jump));

        for digit in ['1', '2', '3', '4', '5', '6', '7'] {
            press_digit(&mut bar, digit);
        }
        assert_eq!(bar.jump_buffer, "123456", "at most six digits");
        bar.handle_event(&Event::key_press(crate::event::key_codes::BACKSPACE, 0));
        assert_eq!(bar.jump_buffer, "12345");
    }

    /// The selector opens, applies the clicked option, and closes in one press.
    #[test]
    fn the_page_size_popup_applies_an_option_and_closes() {
        let mut bar = configured_bar();
        bar.set_page(2);
        bar.set_page_size_options(vec![50, 0, 10, 20, 10]);
        bar.set_show_page_size_selector(true);
        assert_eq!(bar.page_size_options(), [10, 20, 50].as_slice());

        let cell = bar.layout().page_size_rect.expect("page-size cell");
        click(&mut bar, inside(cell));
        assert!(bar.is_page_size_menu_open());

        let popup = bar.page_size_popup_rect().expect("popup is open");
        let option =
            bar.page_size_options().iter().position(|size| *size == 50).expect("50 is an option");
        let row = bar.page_size_option_rect(popup, option);
        click(&mut bar, inside(row));

        assert_eq!(bar.page_size(), 50);
        assert!(!bar.is_page_size_menu_open());
        assert_eq!(bar.page(), 2, "choosing a page size does not jump pages");
    }

    /// Escape and an outside press dismiss the popup without changing the size.
    #[test]
    fn escape_and_an_outside_press_close_the_page_size_popup() {
        let mut bar = configured_bar();
        bar.set_page_size_options(vec![10, 20, 50]);
        bar.set_show_page_size_selector(true);
        let cell = bar.layout().page_size_rect.expect("page-size cell");

        click(&mut bar, inside(cell));
        assert!(bar.is_page_size_menu_open());
        bar.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert!(!bar.is_page_size_menu_open());
        assert_eq!(bar.page_size(), 10);

        click(&mut bar, inside(cell));
        assert!(bar.is_page_size_menu_open());
        let layout = bar.layout();
        let numbered = Point::new(layout.numbered_rect.x + 2, layout.numbered_rect.y + 2);
        click(&mut bar, numbered);
        assert!(!bar.is_page_size_menu_open());
        assert_eq!(bar.page_size(), 10);
    }

    /// Options are normalized: zero is dropped, duplicates collapse, order is ascending.
    #[test]
    fn page_size_options_are_normalized() {
        let mut bar = bar();
        bar.set_page_size_options(vec![0, 50, 10, 50, 0, 20, 10]);
        assert_eq!(bar.page_size_options(), [10, 20, 50].as_slice());
    }

    /// The numbered band, the jump cell and the page-size cell answer different presses.
    #[test]
    fn hit_testing_never_conflates_the_cells() {
        let mut bar = configured_bar();
        bar.set_show_jump_input(true);
        bar.set_page_size_options(vec![10, 20]);
        bar.set_show_page_size_selector(true);

        let layout = bar.layout();
        let jump = layout.jump_rect.expect("jump cell");
        let selector = layout.page_size_rect.expect("page-size cell");
        let bar_rect = bar.bar_rect();
        assert_eq!(jump.x + jump.width as i32, selector.x);
        assert_eq!(
            layout.numbered_rect.width + jump.width + selector.width,
            bar_rect.width,
            "the three bands tile the bar"
        );

        // The selector opens its popup but is not a page cell and not the jump cell.
        click(&mut bar, inside(selector));
        assert!(bar.is_page_size_menu_open());
        assert!(!bar.is_jump_editing());
        assert_eq!(bar.page(), 0);

        // The jump cell focuses the input, closes the popup and does not page.
        click(&mut bar, inside(jump));
        assert!(!bar.is_page_size_menu_open());
        assert!(bar.is_jump_editing());
        assert_eq!(bar.page(), 0);

        // A numbered cell pages and does neither of the above.
        let cell_w = layout.cell_width(bar.cell_plan().len());
        let page_two = Point::new(
            layout.numbered_rect.x + (2 * cell_w) as i32 + 1,
            layout.numbered_rect.y + 2,
        );
        click(&mut bar, page_two);
        assert_eq!(bar.page(), 1);
        assert!(!bar.is_page_size_menu_open());
        assert!(!bar.is_jump_editing());
    }

    /// A theme or layout can shrink the bar; each trailing cell then takes only what is
    /// left instead of overflowing the numbers.
    #[test]
    fn trailing_cells_never_overflow_a_narrow_bar() {
        let mut bar = Pagination::new(Rect::new(0, 0, 100, 32));
        bar.set_show_jump_input(true);
        bar.set_page_size_options(vec![10, 20]);
        bar.set_show_page_size_selector(true);

        let layout = bar.layout();
        let bar_rect = bar.bar_rect();
        let jump = layout.jump_rect.expect("jump cell");
        let selector = layout.page_size_rect.expect("page-size cell");
        assert_eq!(jump.x, layout.numbered_rect.x + layout.numbered_rect.width as i32);
        assert_eq!(selector.x + selector.width as i32, bar_rect.x + bar_rect.width as i32);
        assert!(jump.width <= bar_rect.width * 2 / 5);
        assert!(selector.width <= bar_rect.width * 2 / 5);
    }
}
