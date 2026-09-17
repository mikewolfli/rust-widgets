// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The quote board — a watchlist table of instruments with price, change and volume.
//!
//! # Why it is not a `DataGrid`
//!
//! A grid draws whatever rows it is given and lets each column format itself. A quote
//! board's columns are not arbitrary: the number of decimals follows the instrument, the
//! change column is coloured by sign, and the change *value* is derived from two other
//! columns rather than stored. Encoding those as grid column formatters would mean a
//! `DataGrid` configured to know about previous closes — which is finance knowledge in a
//! generic control.
//!
//! The opposite direction is the real risk: a board built on a grid that then needs a
//! sorting key, a pinned first column and a row-colour rule usually ends up re-deciding
//! what a row is. This one decides once.

use alloc::string::String;
use alloc::vec::Vec;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::finance::types::Quote;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The columns a quote board can show.
///
/// A list rather than four booleans, because it is ordered: the order of this vector is
/// the left-to-right order of the columns, which is what a caller actually wants to
/// control and what four flags could not express.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteColumn {
    /// The ticker symbol.
    Symbol,
    /// The display name.
    Name,
    /// The last traded price.
    Last,
    /// The change, in price units. Coloured by sign.
    Change,
    /// The change, as a percentage. Coloured by sign.
    ChangePercent,
    /// The session high.
    High,
    /// The session low.
    Low,
    /// The session volume.
    Volume,
}

impl QuoteColumn {
    /// The column heading.
    pub fn title(self) -> &'static str {
        match self {
            QuoteColumn::Symbol => "Symbol",
            QuoteColumn::Name => "Name",
            QuoteColumn::Last => "Last",
            QuoteColumn::Change => "Change",
            QuoteColumn::ChangePercent => "Chg%",
            QuoteColumn::High => "High",
            QuoteColumn::Low => "Low",
            QuoteColumn::Volume => "Volume",
        }
    }

    /// Whether this column's value is signed and therefore coloured.
    pub fn is_signed(self) -> bool {
        matches!(self, QuoteColumn::Change | QuoteColumn::ChangePercent)
    }

    /// Whether this column is right-aligned, as numbers are.
    pub fn is_numeric(self) -> bool {
        !matches!(self, QuoteColumn::Symbol | QuoteColumn::Name)
    }

    /// The factory spelling of this column.
    pub fn as_str(self) -> &'static str {
        match self {
            QuoteColumn::Symbol => "symbol",
            QuoteColumn::Name => "name",
            QuoteColumn::Last => "last",
            QuoteColumn::Change => "change",
            QuoteColumn::ChangePercent => "change_percent",
            QuoteColumn::High => "high",
            QuoteColumn::Low => "low",
            QuoteColumn::Volume => "volume",
        }
    }

    /// Parses a factory spelling into a column.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "symbol" => QuoteColumn::Symbol,
            "name" => QuoteColumn::Name,
            "last" => QuoteColumn::Last,
            "change" => QuoteColumn::Change,
            "change_percent" => QuoteColumn::ChangePercent,
            "high" => QuoteColumn::High,
            "low" => QuoteColumn::Low,
            "volume" => QuoteColumn::Volume,
            _ => return None,
        })
    }
}

/// Which column the rows are sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuoteSort {
    /// The order the caller supplied, which is often a watchlist's own order.
    #[default]
    None,
    /// By symbol, ascending.
    Symbol,
    /// By last price, descending — the most active at the top.
    LastDescending,
    /// By absolute change, descending — the day's biggest movers first.
    ChangeMagnitude,
}

/// The quote board control.
pub struct QuoteBoard {
    base: BaseWidget,
    quotes: Vec<Quote>,
    /// The stable order the caller supplied, kept separately from the display order so
    /// switching the sort back to `None` restores it rather than inventing an order.
    ///
    /// A snapshot of the quotes rather than a list of indices: any sort permutes
    /// `quotes`, which invalidates every index into it. See `apply_sort`.
    original_order: Vec<Quote>,
    columns: Vec<QuoteColumn>,
    sort: QuoteSort,
    hovered: Option<usize>,
    selected: Option<usize>,
    rising_color: Color,
    falling_color: Color,
    text_color: Color,
    /// Emitted with the symbol when a row is clicked.
    pub quote_clicked: Signal1<String>,
    /// Emitted with the symbol as the pointer moves onto a row.
    pub quote_hovered: Signal1<String>,
    /// Emitted with the symbol when the selected row changes.
    pub selection_changed: Signal1<Option<String>>,
}

impl QuoteBoard {
    /// Creates an empty board.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::QuoteBoard, geometry, "QuoteBoard"),
            quotes: Vec::new(),
            original_order: Vec::new(),
            columns: alloc::vec![
                QuoteColumn::Symbol,
                QuoteColumn::Last,
                QuoteColumn::Change,
                QuoteColumn::ChangePercent,
                QuoteColumn::Volume,
            ],
            sort: QuoteSort::default(),
            hovered: None,
            selected: None,
            rising_color: Color::rgb(38, 166, 91),
            falling_color: Color::rgb(220, 68, 70),
            text_color: Color::rgb(214, 220, 228),
            quote_clicked: Signal1::new(),
            quote_hovered: Signal1::new(),
            selection_changed: Signal1::new(),
        }
    }

    /// The quotes, in display order.
    pub fn quotes(&self) -> &[Quote] {
        &self.quotes
    }

    /// Replaces the quotes and requests a repaint.
    ///
    /// The supplied order is remembered so a sort can be undone. Storing the quotes
    /// themselves — rather than indices into the live vector — is what makes
    /// [`QuoteSort::None`] meaningful: indices would be invalidated by any intervening
    /// sort, and "no sort" would then mean "restore the order that happens to be left
    /// over", which is not the caller's order.
    pub fn set_quotes(&mut self, quotes: Vec<Quote>) {
        self.quotes = quotes;
        self.original_order = self.quotes.clone();
        self.apply_sort();
        self.revalidate_selection();
        self.base.request_redraw();
    }

    /// The visible columns, left to right.
    pub fn columns(&self) -> &[QuoteColumn] {
        &self.columns
    }

    /// Replaces the visible columns.
    ///
    /// An empty list is applied as given: a caller that has hidden every column gets an
    /// empty board rather than a silently restored default, because the default is a
    /// choice the caller may be deliberately making not to have.
    pub fn set_columns(&mut self, columns: Vec<QuoteColumn>) {
        self.columns = columns;
        self.base.request_redraw();
    }

    /// The active sort.
    pub fn sort(&self) -> QuoteSort {
        self.sort
    }

    /// Sets the sort and requests a repaint.
    pub fn set_sort(&mut self, sort: QuoteSort) {
        self.sort = sort;
        self.apply_sort();
        self.revalidate_selection();
        self.base.request_redraw();
    }

    /// The index of the selected row.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// The selected quote's symbol.
    pub fn selected_symbol(&self) -> Option<&str> {
        self.selected.and_then(|index| self.quotes.get(index)).map(|quote| quote.symbol.as_str())
    }

    /// Selects a row by index, emitting `selection_changed` when it moves.
    ///
    /// An out-of-range index clears the selection rather than being ignored, so a caller
    /// that has just replaced a shorter quote list and passes its old index gets a
    /// consistent "nothing is selected" rather than a stale highlight.
    pub fn select(&mut self, index: Option<usize>) {
        let next = index.filter(|value| *value < self.quotes.len());
        if next == self.selected {
            return;
        }
        self.selected = next;
        let symbol = self.selected_symbol().map(String::from);
        self.selection_changed.emit(symbol);
        self.base.request_redraw();
    }

    /// The index of the hovered row.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered
    }

    /// The row colours.
    pub fn colors(&self) -> (Color, Color) {
        (self.rising_color, self.falling_color)
    }

    /// Sets the up and down row colours.
    pub fn set_colors(&mut self, rising: Color, falling: Color) {
        self.rising_color = rising;
        self.falling_color = falling;
        self.base.request_redraw();
    }

    /// Reorders the quotes according to the active sort.
    ///
    /// # Why the caller's order is kept as quotes, not as indices
    ///
    /// `QuoteSort::None` promises to restore *the order the caller supplied*, so the
    /// natural implementation is to remember the indices and re-read them. That is what
    /// the first version did, and it was wrong: after `QuoteSort::Symbol` the quotes are
    /// permuted, so a remembered index no longer points at the quote it named. The
    /// restore then produced a third order that was neither the caller's nor the sorted
    /// one — silent, because each step looked correct on its own.
    ///
    /// Keeping a snapshot of the quotes makes the restore independent of whatever any
    /// earlier sort did to the live vector. It costs one clone per quote per
    /// `set_quotes`, which happens on a data update rather than per frame.
    /// `app::tests::the_watchlist_starts_in_the_supplied_order` is the test that found
    /// this; it round-trips through two different sorts before checking.
    fn apply_sort(&mut self) {
        match self.sort {
            QuoteSort::None => {
                if self.original_order.len() == self.quotes.len() {
                    self.quotes.clone_from(&self.original_order);
                }
            }
            QuoteSort::Symbol => {
                self.quotes.sort_by(|left, right| left.symbol.cmp(&right.symbol));
            }
            QuoteSort::LastDescending => {
                self.quotes.sort_by(|left, right| {
                    right.last.partial_cmp(&left.last).unwrap_or(core::cmp::Ordering::Equal)
                });
            }
            QuoteSort::ChangeMagnitude => {
                self.quotes.sort_by(|left, right| {
                    let left_magnitude = left.change().abs();
                    let right_magnitude = right.change().abs();
                    right_magnitude
                        .partial_cmp(&left_magnitude)
                        .unwrap_or(core::cmp::Ordering::Equal)
                });
            }
        }
    }

    /// Drops a selection or hover that no longer points at a row.
    fn revalidate_selection(&mut self) {
        if self.selected.is_some_and(|index| index >= self.quotes.len()) {
            self.selected = None;
        }
        if self.hovered.is_some_and(|index| index >= self.quotes.len()) {
            self.hovered = None;
        }
    }

    /// The height of one row.
    fn row_height(&self) -> i32 {
        // A fixed 22 pixels rather than a share of the pane: a watchlist is scanned
        // vertically and a row height that changed with the pane size would make the same
        // list read differently in two windows.
        22
    }

    /// The y of the first data row, after the header.
    fn first_row_y(&self) -> i32 {
        self.base.geometry().y + self.row_height() + 1
    }

    /// The index of the row at a screen y, accounting for the header.
    fn row_at(&self, y: i32) -> Option<usize> {
        let offset = y - self.first_row_y();
        if offset < 0 {
            return None;
        }
        let index = (offset / self.row_height()) as usize;
        if index < self.quotes.len() {
            Some(index)
        } else {
            None
        }
    }

    /// The horizontal range of each column, as `(start, width)` pairs.
    ///
    /// Computed once per frame and shared by the header and the rows, which is what keeps
    /// the two aligned. Computing them per row would let a row drift from its heading.
    fn column_ranges(&self) -> Vec<(i32, i32)> {
        let geometry = self.base.geometry();
        let width = geometry.width as i32;
        let count = self.columns.len().max(1) as i32;
        let slot = width / count;
        (0..self.columns.len() as i32)
            .map(|index| {
                let start = geometry.x + index * slot;
                // The last column takes the remaining pixels, so rounding cannot leave a
                // strip of unpainted background at the right edge.
                let column_width =
                    if index == count - 1 { geometry.x + width - start } else { slot };
                (start, column_width)
            })
            .collect()
    }

    /// The text for one cell.
    fn cell_text(&self, quote: &Quote, column: QuoteColumn, decimals: usize) -> String {
        match column {
            QuoteColumn::Symbol => quote.symbol.clone(),
            QuoteColumn::Name => quote.name.clone(),
            QuoteColumn::Last => format_decimal(quote.last, decimals),
            QuoteColumn::Change => {
                let change = quote.change();
                if change.is_finite() {
                    let sign = if change >= 0.0 { "+" } else { "" };
                    alloc::format!("{sign}{}", format_decimal(change, decimals))
                } else {
                    String::from("—")
                }
            }
            QuoteColumn::ChangePercent => {
                let percent = quote.change_percent();
                if percent.is_finite() {
                    let sign = if percent >= 0.0 { "+" } else { "" };
                    alloc::format!("{sign}{:.2}%", percent)
                } else {
                    String::from("—")
                }
            }
            QuoteColumn::High => format_decimal(quote.high, decimals),
            QuoteColumn::Low => format_decimal(quote.low, decimals),
            QuoteColumn::Volume => format_volume(quote.volume),
        }
    }

    /// The decimals to draw prices at, inferred from the quotes.
    fn inferred_decimals(&self) -> usize {
        for quote in &self.quotes {
            if !quote.last.is_finite() {
                continue;
            }
            for decimals in 0..=4usize {
                let scaled = quote.last * 10f64.powi(decimals as i32);
                if (scaled - scaled.round()).abs() < 1e-6 {
                    return decimals;
                }
            }
        }
        2
    }
}

/// Formats a price at a fixed precision.
fn format_decimal(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return String::from("—");
    }
    let factor = 10f64.powi(decimals as i32);
    let scaled = (value * factor).round() as i64;
    let negative = scaled < 0;
    let digits = scaled.unsigned_abs().to_string();
    if decimals == 0 {
        return if negative { alloc::format!("-{digits}") } else { digits };
    }
    let padded = if digits.len() <= decimals {
        let mut padded = String::new();
        for _ in 0..(decimals - digits.len()) {
            padded.push('0');
        }
        padded.push_str(&digits);
        padded
    } else {
        digits
    };
    let split = padded.len() - decimals;
    let sign = if negative { "-" } else { "" };
    alloc::format!("{sign}{}.{}", &padded[..split], &padded[split..])
}

/// Formats a volume with a magnitude suffix.
///
/// A quote board shows volumes in the millions, where the exact digits are unreadable
/// and the magnitude is the whole message. A suffix is what every platform does, and a
/// caller wanting exact figures reads them from `Quote::volume` rather than the cell.
fn format_volume(volume: f64) -> String {
    if !volume.is_finite() {
        return String::from("—");
    }
    if volume.abs() >= 1_000_000_000.0 {
        alloc::format!("{:.2}B", volume / 1_000_000_000.0)
    } else if volume.abs() >= 1_000_000.0 {
        alloc::format!("{:.2}M", volume / 1_000_000.0)
    } else if volume.abs() >= 1_000.0 {
        alloc::format!("{:.2}K", volume / 1_000.0)
    } else {
        alloc::format!("{volume:.0}")
    }
}

impl Widget for QuoteBoard {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
    impl_widget_property_hooks!();
}

impl crate::event::EventHandler for QuoteBoard {
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::Event;
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let next = self.row_at(pos.y);
                if next != self.hovered {
                    if let Some(index) = next {
                        if let Some(quote) = self.quotes.get(index) {
                            self.quote_hovered.emit(quote.symbol.clone());
                        }
                    }
                    self.hovered = next;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, .. } | Event::PointerPress { pos, .. } => {
                if let Some(index) = self.row_at(pos.y) {
                    if let Some(quote) = self.quotes.get(index) {
                        self.quote_clicked.emit(quote.symbol.clone());
                    }
                    self.select(Some(index));
                }
            }
            _ => {}
        }
        self.base.handle_event(event);
    }
}

impl Draw for QuoteBoard {
    fn draw(&mut self, context: &mut RenderContext) {
        let geometry = self.base.geometry();
        if geometry.width == 0 || geometry.height == 0 {
            return;
        }
        context.fill_rect(geometry, Color::rgb(18, 22, 28));
        if self.columns.is_empty() {
            return;
        }

        let ranges = self.column_ranges();
        let row_height = self.row_height();
        let decimals = self.inferred_decimals();

        // The header, on a slightly lighter band so it reads as a heading rather than as
        // a first data row.
        context.fill_rect(
            Rect::new(geometry.x, geometry.y, geometry.width, row_height as u32),
            Color::rgb(30, 36, 46),
        );
        for (column, (start, width)) in self.columns.iter().zip(ranges.iter()) {
            let text_x = if column.is_numeric() { start + width - 8 } else { start + 8 };
            let alignment = if column.is_numeric() {
                HorizontalAlignment::Right
            } else {
                HorizontalAlignment::Left
            };
            let _ = alignment;
            context.draw_text(
                Point { x: text_x - estimate_width(column.title()), y: geometry.y + 5 },
                column.title(),
                &Font::simple("Sans", 11.0),
                Color::rgb(150, 160, 172),
                HorizontalAlignment::Left,
            );
        }

        // The rows, clipped to the pane by the loop bound rather than by a clip push:
        // a board taller than its frame simply does not draw the rows past the end.
        let available = ((geometry.height as i32 - row_height) / row_height).max(0) as usize;
        for (index, quote) in self.quotes.iter().enumerate().take(available) {
            let y = self.first_row_y() + index as i32 * row_height;
            if self.selected == Some(index) {
                context.fill_rect(
                    Rect::new(geometry.x, y, geometry.width, row_height as u32),
                    Color::rgb(44, 56, 74),
                );
            } else if self.hovered == Some(index) {
                context.fill_rect(
                    Rect::new(geometry.x, y, geometry.width, row_height as u32),
                    Color::rgb(32, 40, 52),
                );
            }

            for (column, (start, width)) in self.columns.iter().zip(ranges.iter()) {
                let text = self.cell_text(quote, *column, decimals);
                // A signed column is coloured by the quote's direction, not by the text's
                // own sign, so a change and its percentage can never disagree about colour.
                let color = if column.is_signed() {
                    if quote.is_up() {
                        self.rising_color
                    } else {
                        self.falling_color
                    }
                } else if *column == QuoteColumn::Symbol {
                    self.text_color
                } else {
                    Color::rgb(170, 178, 190)
                };
                let text_width = estimate_width(&text);
                let text_x =
                    if column.is_numeric() { start + width - 8 - text_width } else { start + 8 };
                context.draw_text(
                    Point { x: text_x, y: y + 5 },
                    &text,
                    &Font::simple("Sans", 11.0),
                    color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

/// Estimates the pixel width of `text` at the small font size the board uses.
///
/// An estimate rather than a measurement because `RenderContext::draw_text` positions by
/// origin and this control right-aligns numeric columns: the wrap width has to be known
/// before the call. The factor matches the fixed-pitch metrics the axis labels elsewhere
/// in this module assume, so a right-aligned column lines up with the rest of the crate's
/// text without a measurement pass.
fn estimate_width(text: &str) -> i32 {
    (text.chars().count() as i32 * 7).max(0)
}

/// Test module for the quote board.
impl WidgetProperties for QuoteBoard {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "sort" => {
                let token = match self.sort {
                    QuoteSort::None => "none",
                    QuoteSort::Symbol => "symbol",
                    QuoteSort::LastDescending => "last_descending",
                    QuoteSort::ChangeMagnitude => "change_magnitude",
                };
                Ok(CapabilityValue::String(alloc::string::String::from(token)))
            }
            "selected_index" => match self.selected {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "sort" => {
                let CapabilityValue::String(text) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let sort = match text.as_str() {
                    "none" => QuoteSort::None,
                    "symbol" => QuoteSort::Symbol,
                    "last_descending" => QuoteSort::LastDescending,
                    "change_magnitude" => QuoteSort::ChangeMagnitude,
                    _ => return Err(CapabilityAccessError::OutOfRange),
                };
                self.set_sort(sort);
                Ok(())
            }
            "selected_index" => {
                match value {
                    CapabilityValue::UInt(index) => {
                        // An out-of-range index is refused here rather than silently
                        // clearing: the property contract reports what was asked for, and
                        // "select row 99 of 3" is a caller error, not a request to
                        // deselect.
                        if index as usize >= self.quotes.len() {
                            return Err(CapabilityAccessError::OutOfRange);
                        }
                        self.select(Some(index as usize));
                    }
                    CapabilityValue::Null => self.select(None),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "row_height" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["sort", "selected_index", "row_height", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventHandler;
    use crate::widget::special_widgets::finance::types::{fixtures, Quote};
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// The board carries the quotes it was given.
    #[test]
    fn a_board_carries_its_quotes() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        assert!(board.quotes().is_empty());
        board.set_quotes(fixtures::sample_quotes(8));
        assert_eq!(board.quotes().len(), 8);
    }

    /// The default columns are the ones a watchlist is read by.
    #[test]
    fn the_default_columns_are_the_usual_set() {
        let board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        assert_eq!(
            board.columns(),
            &[
                QuoteColumn::Symbol,
                QuoteColumn::Last,
                QuoteColumn::Change,
                QuoteColumn::ChangePercent,
                QuoteColumn::Volume,
            ]
        );
    }

    /// Columns can be replaced, including with nothing.
    #[test]
    fn columns_can_be_replaced() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_columns(alloc::vec![QuoteColumn::Symbol, QuoteColumn::Name]);
        assert_eq!(board.columns().len(), 2);
        board.set_columns(alloc::vec![]);
        assert!(board.columns().is_empty(), "an empty list is honoured as given");
    }

    /// Every column spelling round-trips.
    #[test]
    fn column_names_round_trip() {
        for column in [
            QuoteColumn::Symbol,
            QuoteColumn::Name,
            QuoteColumn::Last,
            QuoteColumn::Change,
            QuoteColumn::ChangePercent,
            QuoteColumn::High,
            QuoteColumn::Low,
            QuoteColumn::Volume,
        ] {
            let name = column.as_str();
            assert_eq!(QuoteColumn::from_name(name), Some(column), "{name} must round-trip");
            assert!(!column.title().is_empty(), "every column needs a heading");
        }
        assert_eq!(QuoteColumn::from_name("nonsense"), None);
    }

    /// The change columns are the signed ones, and the text columns are the non-numeric.
    #[test]
    fn column_roles_are_classified() {
        assert!(QuoteColumn::Change.is_signed());
        assert!(QuoteColumn::ChangePercent.is_signed());
        assert!(!QuoteColumn::Last.is_signed(), "a price is not signed");
        assert!(!QuoteColumn::Symbol.is_numeric());
        assert!(!QuoteColumn::Name.is_numeric());
        assert!(QuoteColumn::Volume.is_numeric());
    }

    /// Sorting by symbol orders the rows.
    #[test]
    fn sorting_by_symbol_orders_the_rows() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(6));
        board.set_sort(QuoteSort::Symbol);
        let symbols: alloc::vec::Vec<&str> =
            board.quotes().iter().map(|quote| quote.symbol.as_str()).collect();
        let mut expected = symbols.clone();
        expected.sort_unstable();
        assert_eq!(symbols, expected);
    }

    /// Sorting by last price is descending.
    #[test]
    fn sorting_by_price_puts_the_highest_first() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(6));
        board.set_sort(QuoteSort::LastDescending);
        let prices: alloc::vec::Vec<f64> = board.quotes().iter().map(|quote| quote.last).collect();
        for window in prices.windows(2) {
            assert!(window[0] >= window[1], "the highest price must come first");
        }
    }

    /// Sorting by change magnitude puts the biggest movers first, either direction.
    #[test]
    fn sorting_by_magnitude_ignores_direction() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(6));
        board.set_sort(QuoteSort::ChangeMagnitude);
        let magnitudes: alloc::vec::Vec<f64> =
            board.quotes().iter().map(|quote| quote.change().abs()).collect();
        for window in magnitudes.windows(2) {
            assert!(window[0] >= window[1], "the biggest movers must come first");
        }
    }

    /// Returning to no sort restores the caller's own order.
    ///
    /// This is why the board stores the original order rather than reversing whatever a
    /// previous sort left behind: a watchlist's order is the caller's, and "unsorted"
    /// has to mean theirs.
    ///
    /// # Why this round-trips through *two* sorts
    ///
    /// The first version checked a single sort, which was not enough to catch the bug it
    /// was written for. The board used to remember the caller's order as **indices** into
    /// the live vector; one sort permutes that vector, so a second sort made the indices
    /// point at the wrong quotes and the restore produced a third order that was neither
    /// the caller's nor the sorted one. With one sort, the permutation happened to be
    /// the identity for this fixture and the test passed.
    ///
    /// Sorting twice with two different keys applies two different permutations, so an
    /// index-based restore cannot coincide with the correct answer by luck.
    #[test]
    fn clearing_the_sort_restores_the_supplied_order() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        // A fixture whose order is deliberately *not* any of the sorts, so each sort
        // below really permutes it. `fixtures::sample_quotes` is already symbol-ordered
        // (`SYM0`, `SYM1`, …), which would make the symbol sort a no-op and weaken this
        // test to one real permutation.
        let mut quotes = fixtures::sample_quotes(6);
        quotes.reverse();
        board.set_quotes(quotes);

        let original: alloc::vec::Vec<alloc::string::String> =
            board.quotes().iter().map(|quote| quote.symbol.clone()).collect();
        let mut by_symbol = original.clone();
        by_symbol.sort();
        assert_ne!(by_symbol, original, "the fixture must not already be symbol-sorted");

        board.set_sort(QuoteSort::LastDescending);
        board.set_sort(QuoteSort::Symbol);
        board.set_sort(QuoteSort::ChangeMagnitude);
        board.set_sort(QuoteSort::None);
        let restored: alloc::vec::Vec<alloc::string::String> =
            board.quotes().iter().map(|quote| quote.symbol.clone()).collect();
        assert_eq!(restored, original, "the caller's order must come back exactly");
    }

    /// Selecting a row emits its symbol, and selecting again does nothing.
    #[test]
    fn selection_emits_and_is_idempotent() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(4));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        board.selection_changed.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        board.select(Some(1));
        assert_eq!(count.load(Ordering::SeqCst), 1, "a real move emits once");
        assert_eq!(board.selected_symbol(), Some("SYM1"));
        board.select(Some(1));
        assert_eq!(count.load(Ordering::SeqCst), 1, "re-selecting the same row is a no-op");
        board.select(None);
        assert_eq!(count.load(Ordering::SeqCst), 2, "clearing emits");
        assert_eq!(board.selected_symbol(), None);
    }

    /// An out-of-range selection clears rather than pointing at nothing.
    #[test]
    fn an_out_of_range_selection_clears() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(3));
        board.select(Some(99));
        assert_eq!(board.selected_index(), None, "there is no row 99 to select");
    }

    /// An empty board draws without panicking.
    #[test]
    fn an_empty_board_draws_without_panicking() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(480, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
    }

    /// A board with every column selected draws without panicking.
    ///
    /// The column widths are a division of the pane by the column count, so this is the
    /// narrowest-column case.
    #[test]
    fn every_column_draws() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(4));
        board.set_columns(alloc::vec![
            QuoteColumn::Symbol,
            QuoteColumn::Name,
            QuoteColumn::Last,
            QuoteColumn::Change,
            QuoteColumn::ChangePercent,
            QuoteColumn::High,
            QuoteColumn::Low,
            QuoteColumn::Volume,
        ]);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(640, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
    }

    /// A malformed quote draws without panicking.
    #[test]
    fn a_malformed_quote_draws_without_panicking() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        let mut quote = Quote::new("BAD", f64::NAN, 0.0);
        quote.high = f64::INFINITY;
        quote.low = f64::NEG_INFINITY;
        quote.volume = f64::NAN;
        board.set_quotes(alloc::vec![quote]);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(480, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
    }

    /// Colours are stored and returned.
    #[test]
    fn colours_round_trip() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        let (up, down) = board.colors();
        assert_ne!(up, down);
        board.set_colors(crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6));
        assert_eq!(
            board.colors(),
            (crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6))
        );
    }

    /// Clicking a row emits its symbol and selects it.
    #[test]
    fn clicking_a_row_emits_and_selects() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(5));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        board.quote_clicked.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        board.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 100, y: 40 },
            button: 0,
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "a press on a row emits its symbol");
        assert!(board.selected_index().is_some(), "and selects it");
    }
}
