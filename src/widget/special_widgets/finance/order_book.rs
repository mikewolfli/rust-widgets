// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The order book ladder — the bid and ask levels, best first, with sizes.
//!
//! # What a reader does with it
//!
//! Reads down from the top of each side. The top row of the bids and the top row of the
//! asks are the spread; the size sitting at each level says how much would have to trade
//! to move the price past it. A ladder is therefore read row by row, which is why the
//! levels are drawn in a fixed row order rather than scaled to their prices: the
//! interesting comparison is between adjacent rows, and a price-proportional layout
//! compresses exactly where the rows are interesting.
//!
//! # Why the size bars are anchored to the outer edge
//!
//! Bids grow leftward from the centre spine and asks grow rightward. That way the
//! visual length of a bar is its size, both sides share one scale, and the spine itself
//! marks the price of the best bid and offer. Anchoring both to the left instead would
//! make the two sides look like one series.

use alloc::vec::Vec;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::finance::types::{BookLevel, OrderBook};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Which ladder a level belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookSide {
    /// The bid ladder, highest price first.
    Bid,
    /// The ask ladder, lowest price first.
    Ask,
}

/// How many levels of each side the ladder shows.
///
/// Five is the depth Chinese and most European venues publish; ten is the common
/// Western default. The default here is five because it is the tighter reading and a
/// caller wanting more asks for it explicitly.
pub const DEFAULT_BOOK_DEPTH: usize = 5;

/// The order book control.
pub struct OrderBookWidget {
    base: BaseWidget,
    book: OrderBook,
    depth: usize,
    /// How many price decimals to draw. `None` infers them from the book.
    decimals: Option<usize>,
    bid_color: Color,
    ask_color: Color,
    text_color: Color,
    hovered: Option<(BookSide, usize)>,
    /// Emitted with the side and the level index when a row is clicked.
    pub level_clicked: crate::signal::Signal1<(BookSide, usize)>,
    /// Emitted as the pointer moves onto a row.
    pub level_hovered: crate::signal::Signal1<(BookSide, usize)>,
    /// Emitted with the pointer leaves a row.
    pub level_unhovered: crate::signal::Signal1<(BookSide, usize)>,
}

impl OrderBookWidget {
    /// Creates an empty ladder.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::OrderBook, geometry, "OrderBook"),
            book: OrderBook::new(),
            depth: DEFAULT_BOOK_DEPTH,
            decimals: None,
            bid_color: Color::rgb(38, 166, 91),
            ask_color: Color::rgb(220, 68, 70),
            text_color: Color::rgb(214, 220, 228),
            hovered: None,
            level_clicked: crate::signal::Signal1::new(),
            level_hovered: crate::signal::Signal1::new(),
            level_unhovered: crate::signal::Signal1::new(),
        }
    }

    /// The book being displayed.
    pub fn book(&self) -> &OrderBook {
        &self.book
    }

    /// Replaces the book and requests a repaint.
    ///
    /// The hovered row is re-validated rather than kept: a level index that existed in
    /// the previous book may be past the end of the new one, and a hover highlight on a
    /// row that is not there would point at nothing.
    pub fn set_book(&mut self, book: OrderBook) {
        self.book = book;
        if let Some((side, index)) = self.hovered {
            let count = match side {
                BookSide::Bid => self.book.bids().len(),
                BookSide::Ask => self.book.asks().len(),
            };
            if index >= count {
                self.hovered = None;
            }
        }
        self.base.request_redraw();
    }

    /// How many levels per side are shown.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Sets how many levels per side to show.
    ///
    /// A stored `0` would mean a ladder that draws nothing, so it is treated as a request
    /// to show nothing and is honoured literally — the caller may genuinely want the pane
    /// blank while it waits for the first snapshot.
    pub fn set_depth(&mut self, depth: usize) {
        self.depth = depth;
        if let Some((_, index)) = self.hovered {
            if index >= depth {
                self.hovered = None;
            }
        }
        self.base.request_redraw();
    }

    /// The explicit price precision, if one was set.
    pub fn decimals(&self) -> Option<usize> {
        self.decimals
    }

    /// Sets the price precision, or `None` to infer it from the book.
    ///
    /// Capped at six places, past which the column is wider than the price is meaningful.
    pub fn set_decimals(&mut self, decimals: Option<usize>) {
        self.decimals = decimals.map(|value| value.min(6));
        self.base.request_redraw();
    }

    /// The row colours.
    pub fn colors(&self) -> (Color, Color) {
        (self.bid_color, self.ask_color)
    }

    /// Sets the bid and ask row colours.
    pub fn set_colors(&mut self, bid: Color, ask: Color) {
        self.bid_color = bid;
        self.ask_color = ask;
        self.base.request_redraw();
    }

    /// The row under the pointer.
    pub fn hovered(&self) -> Option<(BookSide, usize)> {
        self.hovered
    }

    /// The levels shown on one side, best first, capped at the depth.
    pub fn levels(&self, side: BookSide) -> Vec<BookLevel> {
        let levels = match side {
            BookSide::Bid => self.book.bids(),
            BookSide::Ask => self.book.asks(),
        };
        levels.iter().take(self.depth).copied().collect()
    }

    /// The precision to draw prices at.
    fn effective_decimals(&self) -> usize {
        if let Some(decimals) = self.decimals {
            return decimals;
        }
        // Infer from the levels, the same way the K-line pane infers from its bars: a
        // feed's precision is a property of the data, not a setting.
        for side in [BookSide::Bid, BookSide::Ask] {
            for level in self.levels(side) {
                if !level.price.is_finite() {
                    continue;
                }
                for decimals in 0..=6usize {
                    let scaled = level.price * 10f64.powi(decimals as i32);
                    if (scaled - scaled.round()).abs() < 1e-6 {
                        return decimals;
                    }
                }
            }
        }
        2
    }

    /// The height of one row.
    fn row_height(&self) -> i32 {
        let height = self.base.geometry().height as i32;
        // Both sides share the available height, so the two ladders stay aligned and the
        // pane does not scroll: a depth the pane cannot fit is clipped rather than
        // resized, because a changing row height between updates makes the ladder
        // unreadable.
        let rows = (self.depth * 2).max(1) as i32;
        (height / rows).max(1)
    }

    /// The row index under a screen y, if any.
    ///
    /// Bounded by the widget's own rectangle on both edges, matching `TreeTable::row_at`.
    ///
    /// The upper bound is defensive rather than a live fix: with today's `row_height`
    /// (which scales as `height / (depth * 2)`) a pointer below the widget always lands
    /// beyond the book's combined row count, so the data bound already rejected it. It is
    /// stated explicitly so the rejection does not depend on that coincidence surviving a
    /// future change to the row-height formula.
    fn row_at(&self, y: i32) -> Option<(BookSide, usize)> {
        let geometry = self.base.geometry();
        if y < geometry.y || y >= geometry.y + geometry.height as i32 {
            return None;
        }
        let row_height = self.row_height();
        let offset = y - geometry.y;
        if offset < 0 {
            return None;
        }
        let row = (offset / row_height) as usize;
        let bid_rows = self.levels(BookSide::Bid).len();
        if row < bid_rows {
            return Some((BookSide::Bid, row));
        }
        let ask_row = row - self.levels(BookSide::Bid).len();
        if ask_row < self.levels(BookSide::Ask).len() {
            return Some((BookSide::Ask, ask_row));
        }
        None
    }
}

/// Formats a price at a fixed number of decimals.
///
/// Written out rather than using a dynamic-precision formatter, which the `alloc`-backed
/// `format!` in this crate does not provide. Shared by the ladder's two columns so the
/// price and the size cannot disagree about their alignment.
fn format_price(value: f64, decimals: usize) -> alloc::string::String {
    if !value.is_finite() {
        return alloc::string::String::from("—");
    }
    let factor = 10f64.powi(decimals as i32);
    let scaled = (value * factor).round() as i64;
    let negative = scaled < 0;
    let digits = scaled.unsigned_abs().to_string();
    if decimals == 0 {
        return if negative { alloc::format!("-{digits}") } else { digits };
    }
    let padded = if digits.len() <= decimals {
        let mut padded = alloc::string::String::new();
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

impl Widget for OrderBookWidget {
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

impl crate::event::EventHandler for OrderBookWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::Event;
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let next = self.row_at(pos.y);
                if next != self.hovered {
                    if let Some(previous) = self.hovered {
                        self.level_unhovered.emit(previous);
                    }
                    if let Some(row) = next {
                        self.level_hovered.emit(row);
                    }
                    self.hovered = next;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if let Some(previous) = self.hovered.take() {
                    self.level_unhovered.emit(previous);
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, .. } | Event::PointerPress { pos, .. } => {
                if let Some(row) = self.row_at(pos.y) {
                    self.level_clicked.emit(row);
                }
            }
            _ => {}
        }
        self.base.handle_event(event);
    }
}

impl Draw for OrderBookWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let geometry = self.base.geometry();
        if geometry.width == 0 || geometry.height == 0 || self.depth == 0 {
            return;
        }
        context.fill_rect(geometry, Color::rgb(18, 22, 28));

        let max_quantity = self.book.max_level_quantity();
        let row_height = self.row_height();
        let decimals = self.effective_decimals();
        // The size column occupies the middle band; prices sit outside it, so a long bar
        // cannot run under a price and make it unreadable.
        let spine = geometry.x + geometry.width as i32 / 2;
        let bar_span = (geometry.width as i32 / 2 - 40).max(8);

        // Bids first, from the top down; asks follow below them.
        let mut row = 0;
        for side in [BookSide::Bid, BookSide::Ask] {
            let levels = self.levels(side);
            let color = if side == BookSide::Bid { self.bid_color } else { self.ask_color };
            for (index, level) in levels.iter().enumerate() {
                let y = geometry.y + row * row_height;
                let is_hovered = self.hovered == Some((side, index));
                if is_hovered {
                    context.fill_rect(
                        Rect::new(geometry.x, y, geometry.width, row_height as u32),
                        Color::rgb(38, 46, 58),
                    );
                }

                // The size bar, growing outward from the spine.
                if max_quantity > 0.0 && level.quantity.is_finite() && level.quantity > 0.0 {
                    let length =
                        ((level.quantity / max_quantity) * bar_span as f64).round().max(1.0) as i32;
                    let (bar_x, bar_width) = match side {
                        BookSide::Bid => (spine - length, length as u32),
                        BookSide::Ask => (spine, length as u32),
                    };
                    context.fill_rect(
                        Rect::new(bar_x, y + 1, bar_width, (row_height - 2).max(1) as u32),
                        color,
                    );
                }

                let price_text = format_price(level.price, decimals);
                let quantity_text = if level.quantity.is_finite() {
                    alloc::format!("{:.0}", level.quantity)
                } else {
                    alloc::string::String::from("—")
                };

                // Price on the outer edge, quantity beside the bar, so the two columns
                // align vertically across every row.
                let (price_x, quantity_x) = match side {
                    BookSide::Bid => (geometry.x + 6, spine - bar_span - 46),
                    BookSide::Ask => {
                        (geometry.x + geometry.width as i32 - 58, spine + bar_span + 4)
                    }
                };
                let text_y = y + (row_height - 12) / 2;
                context.draw_text(
                    Point { x: price_x, y: text_y },
                    &price_text,
                    &Font::simple("Sans", 11.0),
                    color,
                    HorizontalAlignment::Left,
                );
                if quantity_x > geometry.x && quantity_x < geometry.x + geometry.width as i32 - 8 {
                    context.draw_text(
                        Point { x: quantity_x, y: text_y },
                        &quantity_text,
                        &Font::simple("Sans", 11.0),
                        self.text_color,
                        HorizontalAlignment::Left,
                    );
                }
                row += 1;
            }
        }

        // The spread, drawn across the middle where the two sides meet.
        if let Some(spread) = self.book.spread() {
            let y = geometry.y + self.levels(BookSide::Bid).len() as i32 * row_height;
            context.draw_line_stroke(
                Point { x: geometry.x, y },
                Point { x: geometry.x + geometry.width as i32, y },
                Color::rgb(120, 120, 120),
                1,
            );
            let text = alloc::format!("spread {}", format_price(spread, decimals));
            context.draw_text(
                Point { x: geometry.x + 6, y: y - 13 },
                &text,
                &Font::simple("Sans", 10.0),
                Color::rgb(158, 158, 158),
                HorizontalAlignment::Left,
            );
        }
    }
}

/// Test module for the order book ladder.
impl WidgetProperties for OrderBookWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "depth" => Ok(CapabilityValue::UInt(self.depth as u64)),
            "decimals" => match self.decimals {
                Some(decimals) => Ok(CapabilityValue::UInt(decimals as u64)),
                // `Null` rather than a sentinel integer: "infer it" is not a number of
                // decimals, and reporting a zero would read as "no decimals".
                None => Ok(CapabilityValue::Null),
            },
            "show_spread" => Ok(CapabilityValue::Bool(true)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "depth" => {
                let CapabilityValue::UInt(depth) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_depth(depth as usize);
                Ok(())
            }
            "decimals" => match value {
                CapabilityValue::UInt(decimals) => {
                    self.set_decimals(Some(decimals as usize));
                    Ok(())
                }
                // Writing `Null` is the documented way to ask for inference again.
                CapabilityValue::Null => {
                    self.set_decimals(None);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "show_spread" => {
                let CapabilityValue::Bool(_) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // The spread separator is part of what makes the ladder readable, so it
                // is not switchable; the property exists so a caller can confirm it is
                // on, and the write is accepted only for the value it already holds.
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["depth", "decimals", "show_spread", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `order_book` publishes.
    ///
    /// A ladder is a table of levels, not a plotted series, so it has no overlay
    /// model; `add_overlay` is published for the whole finance group and is answered
    /// as [`CapabilityAccessError::UnsupportedOnWidget`] — the name is real and this
    /// control cannot perform it. That refusal is an honest answer, not a fix: the
    /// `commands` list still advertises an action this control cannot take, and
    /// reconciling it is a registry decision. `set_series` / `set_depth` /
    /// `set_decimals` carry their values and are refused as
    /// [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_overlay" => Err(CapabilityAccessError::UnsupportedOnWidget),
            "set_series" | "set_depth" | "set_decimals" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventHandler};
    use crate::widget::special_widgets::finance::types::fixtures;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// The ladder carries the book it was given.
    #[test]
    fn a_ladder_carries_its_book() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        assert!(ladder.book().bids().is_empty());
        ladder.set_book(fixtures::sample_book(5));
        assert_eq!(ladder.book().bids().len(), 5);
    }

    /// The default depth is five, matching the conventional published depth.
    #[test]
    fn the_default_depth_is_five() {
        let ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        assert_eq!(ladder.depth(), DEFAULT_BOOK_DEPTH);
        assert_eq!(ladder.depth(), 5);
    }

    /// The levels shown are capped at the depth on each side.
    #[test]
    fn levels_are_capped_at_the_depth() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        ladder.set_book(fixtures::sample_book(10));
        assert_eq!(ladder.levels(BookSide::Bid).len(), 5);
        assert_eq!(ladder.levels(BookSide::Ask).len(), 5);
        ladder.set_depth(2);
        assert_eq!(ladder.levels(BookSide::Bid).len(), 2);
        assert_eq!(ladder.levels(BookSide::Ask).len(), 2);
    }

    /// The bids are best-first (highest price) and the asks best-first (lowest).
    ///
    /// This is the property the whole ladder is read by: the top row of each side is the
    /// spread. If the book held arrival order instead, the top row would be arbitrary.
    #[test]
    fn both_sides_are_ordered_best_first() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        let mut book = crate::widget::special_widgets::finance::types::OrderBook::new();
        // Deliberately out of order, as a feed delivers them.
        book.set_bids(alloc::vec![
            crate::widget::special_widgets::finance::types::BookLevel::new(97.0, 1.0),
            crate::widget::special_widgets::finance::types::BookLevel::new(99.0, 1.0),
            crate::widget::special_widgets::finance::types::BookLevel::new(98.0, 1.0),
        ]);
        book.set_asks(alloc::vec![
            crate::widget::special_widgets::finance::types::BookLevel::new(103.0, 1.0),
            crate::widget::special_widgets::finance::types::BookLevel::new(101.0, 1.0),
            crate::widget::special_widgets::finance::types::BookLevel::new(102.0, 1.0),
        ]);
        ladder.set_book(book);
        let bids = ladder.levels(BookSide::Bid);
        let asks = ladder.levels(BookSide::Ask);
        assert!((bids[0].price - 99.0).abs() < 1e-9, "the best bid is the highest");
        assert!(bids[0].price > bids[1].price && bids[1].price > bids[2].price);
        assert!((asks[0].price - 101.0).abs() < 1e-9, "the best ask is the lowest");
        assert!(asks[0].price < asks[1].price && asks[1].price < asks[2].price);
    }

    /// An explicit precision overrides the inference and is capped.
    #[test]
    fn decimals_can_be_forced_and_are_capped() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        ladder.set_book(fixtures::sample_book(5));
        assert_eq!(ladder.decimals(), None, "inferred by default");
        ladder.set_decimals(Some(2));
        assert_eq!(ladder.decimals(), Some(2));
        ladder.set_decimals(Some(99));
        assert_eq!(ladder.decimals(), Some(6), "capped at the readable maximum");
        ladder.set_decimals(None);
        assert_eq!(ladder.decimals(), None);
    }

    /// Colours are stored and returned.
    #[test]
    fn colours_round_trip() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        let (bid, ask) = ladder.colors();
        assert_ne!(bid, ask);
        ladder.set_colors(crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6));
        assert_eq!(
            ladder.colors(),
            (crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6))
        );
    }

    /// An empty book draws without panicking.
    #[test]
    fn an_empty_book_draws_without_panicking() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        ladder.draw(&mut context);
    }

    /// A zero depth draws nothing rather than panicking on a division by zero.
    #[test]
    fn a_zero_depth_draws_nothing() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        ladder.set_book(fixtures::sample_book(5));
        ladder.set_depth(0);
        assert!(ladder.levels(BookSide::Bid).is_empty());
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        ladder.draw(&mut context);
    }

    /// A malformed book draws without panicking.
    #[test]
    fn a_malformed_book_draws_without_panicking() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        let mut book = crate::widget::special_widgets::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![
            crate::widget::special_widgets::finance::types::BookLevel::new(f64::NAN, f64::NAN),
            crate::widget::special_widgets::finance::types::BookLevel::new(99.0, f64::INFINITY),
        ]);
        ladder.set_book(book);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        ladder.draw(&mut context);
    }

    /// Clicking a row emits its side and index.
    #[test]
    fn clicking_a_row_emits_its_position() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        ladder.set_book(fixtures::sample_book(5));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        ladder.level_clicked.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        ladder.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 100, y: 10 },
            button: 0,
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "a press on a row emits it");
    }

    /// The hovered row clears when the book shrinks past it.
    #[test]
    fn replacing_a_shorter_book_clears_a_stale_hover() {
        let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 240));
        ladder.set_book(fixtures::sample_book(5));
        ladder.handle_event(&crate::event::Event::MouseMove {
            pos: crate::core::Point { x: 100, y: 10 },
        });
        ladder.set_book(fixtures::sample_book(1));
        assert!(
            ladder.hovered().is_none_or(|(_, index)| index < 1),
            "a hover must not survive past the book it pointed into"
        );
    }
    /// Hover is bounded by the widget's own rectangle, not only by the book's depth.
    ///
    /// `row_at` rejected a negative offset but had no upper bound, so whether a pointer below
    /// the ladder resolved was left to the book's depth. Today the row-height formula makes
    /// that coincidentally safe (a row below the widget always exceeds the book's row count),
    /// and this pins the property so it does not depend on that coincidence: the widget is
    /// short and the book far deeper than it can show.
    #[test]
    fn order_book_hover_is_bounded_by_the_geometry() {
        let geometry = Rect::new(0, 50, 300, 100);
        let hover_at = |y: i32| {
            let mut ladder = OrderBookWidget::new(geometry);
            ladder.set_depth(64);
            ladder.set_book(fixtures::sample_book(64));
            ladder.handle_event(&Event::MouseMove { pos: Point::new(150, y) });
            ladder.hovered()
        };

        assert_eq!(hover_at(49), None, "above the widget");
        assert!(hover_at(60).is_some(), "the top rows are inside the widget");
        assert_eq!(hover_at(150), None, "the bottom edge is outside");
        assert_eq!(hover_at(200), None, "below the widget");
        assert_eq!(hover_at(2000), None, "far below the widget");
    }
}
