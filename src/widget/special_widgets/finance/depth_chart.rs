// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The depth chart — the cumulative supply and demand curve around the spread.
//!
//! # What the curve means
//!
//! Each point answers "how much can be bought (or sold) before the price reaches here".
//! The bid curve descends to the left — as the price falls, more resting buy orders are
//! within reach — and the ask curve rises to the right. The two meet at the spread, and
//! the shape of the V around that meeting point is the liquidity reading: a shallow V
//! means size is available close to the touch, a steep one means the book is thin.
//!
//! # Why it is separate from the order book
//!
//! They show the same data and answer different questions. A ladder shows *levels* —
//! which price, how much, in order — and is read row by row. A depth curve shows the
//! *accumulated* shape and is read as a single picture. One cannot be derived from the
//! other's drawing, and a control that tried to be both would need a mode switch that
//! changes what every pixel means.

use alloc::vec::Vec;

use crate::core::{Color, Rect};
use crate::render::RenderContext;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::finance::layout::PlotArea;
use crate::widget::special_widgets::finance::types::{BookLevel, OrderBook};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Which side of the book a point on the depth curve belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthSide {
    /// Resting buy orders, drawn to the left of the mid price.
    Bid,
    /// Resting sell orders, drawn to the right.
    Ask,
}

/// One point on the depth curve.
///
/// Exposed rather than kept private because a caller drawing its own tooltip needs the
/// exact values behind the curve, and recomputing them would be a second implementation
/// of the accumulation that could disagree with the drawn one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthPoint {
    /// The price level.
    pub price: f64,
    /// Total quantity resting at this price or better.
    pub cumulative_quantity: f64,
    /// Which side of the spread this point is on.
    pub side: DepthSide,
}

/// The depth chart control.
pub struct DepthChart {
    base: BaseWidget,
    book: OrderBook,
    /// How many levels of each side to plot. Zero means every level.
    depth: usize,
    /// The bid curve colour.
    bid_color: Color,
    /// The ask curve colour.
    ask_color: Color,
    /// Emitted with the price level under the pointer.
    pub level_hovered: crate::signal::Signal1<f64>,
}

impl DepthChart {
    /// Creates an empty depth chart.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DepthChart, geometry, "DepthChart"),
            book: OrderBook::new(),
            depth: 0,
            bid_color: Color::rgb(38, 166, 91),
            ask_color: Color::rgb(220, 68, 70),
            level_hovered: crate::signal::Signal1::new(),
        }
    }

    /// The book being plotted.
    pub fn book(&self) -> &OrderBook {
        &self.book
    }

    /// Replaces the book and requests a repaint.
    pub fn set_book(&mut self, book: OrderBook) {
        self.book = book;
        self.base.request_redraw();
    }

    /// How many levels per side are plotted; zero means all of them.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Sets how many levels per side to plot.
    ///
    /// Zero (the default) plots everything the book holds. A positive value keeps the
    /// curve near the touch, which is where the reading usually matters — a book with
    /// hundreds of levels plotted whole puts the interesting part in a few pixels.
    pub fn set_depth(&mut self, depth: usize) {
        self.depth = depth;
        self.base.request_redraw();
    }

    /// The two curve colours.
    pub fn colors(&self) -> (Color, Color) {
        (self.bid_color, self.ask_color)
    }

    /// Sets the bid and ask curve colours.
    pub fn set_colors(&mut self, bid: Color, ask: Color) {
        self.bid_color = bid;
        self.ask_color = ask;
        self.base.request_redraw();
    }

    /// The cumulative curve, as plotted.
    ///
    /// Bids are returned best-first as the book holds them, with the cumulative total
    /// growing as the price moves away from the touch. Asks follow the same rule. The
    /// caller drawing its own annotation gets the same numbers the curve was drawn from.
    pub fn curve(&self) -> Vec<DepthPoint> {
        let mut points = Vec::new();
        let mut running = 0.0;
        for level in self.levels(DepthSide::Bid) {
            running += level.quantity.max(0.0);
            points.push(DepthPoint {
                price: level.price,
                cumulative_quantity: running,
                side: DepthSide::Bid,
            });
        }
        let mut running = 0.0;
        for level in self.levels(DepthSide::Ask) {
            running += level.quantity.max(0.0);
            points.push(DepthPoint {
                price: level.price,
                cumulative_quantity: running,
                side: DepthSide::Ask,
            });
        }
        points
    }

    /// The levels to plot on one side, honouring the depth limit.
    fn levels(&self, side: DepthSide) -> Vec<BookLevel> {
        let levels = match side {
            DepthSide::Bid => self.book.bids(),
            DepthSide::Ask => self.book.asks(),
        };
        if self.depth == 0 {
            levels.to_vec()
        } else {
            levels.iter().take(self.depth).copied().collect()
        }
    }

    /// The quantity the plot scales against: the largest cumulative total on either side.
    ///
    /// Deliberately the *cumulative* maximum rather than the largest single level, because
    /// the y axis carries the accumulation. Scaling against a single level would push the
    /// curve off the top of the pane at its far end.
    fn max_cumulative(&self) -> f64 {
        let mut best = 0.0_f64;
        for side in [DepthSide::Bid, DepthSide::Ask] {
            let mut running = 0.0;
            for level in self.levels(side) {
                running += level.quantity.max(0.0);
            }
            best = best.max(running);
        }
        best
    }

    /// Draws one side's step curve.
    ///
    /// A **step** rather than a smooth line: each level is a price at which a fixed
    /// quantity sits, so the cumulative total is constant across that price and jumps at
    /// the next level. A smooth interpolation would draw quantity that does not exist at
    /// prices between levels.
    fn draw_step_curve(
        &self,
        context: &mut RenderContext,
        area: &PlotArea,
        side: DepthSide,
        color: Color,
        max_cumulative: f64,
    ) {
        let levels = self.levels(side);
        if levels.is_empty() || max_cumulative <= 0.0 {
            return;
        }
        let (low_price, high_price) = self.price_bounds();
        if !low_price.is_finite() || !high_price.is_finite() || high_price <= low_price {
            return;
        }

        let to_x = |price: f64| -> i32 {
            let fraction = (price - low_price) / (high_price - low_price);
            area.rect.x + (fraction * area.rect.width as f64).round() as i32
        };
        let to_y = |quantity: f64| -> i32 {
            let fraction = quantity / max_cumulative;
            area.bottom() - (fraction * area.rect.height as f64).round() as i32
        };

        let mut previous: Option<(i32, i32)> = None;
        let mut running = 0.0;
        // Walk away from the touch, so the curve starts at the spread and grows outward.
        for level in &levels {
            if !level.price.is_finite() || !level.quantity.is_finite() || level.quantity <= 0.0 {
                continue;
            }
            running += level.quantity;
            let x = to_x(level.price);
            let y = to_y(running);
            if let Some((previous_x, previous_y)) = previous {
                // The horizontal segment carries the previous total across to this level,
                // which is what makes it a step rather than a staircase of diagonals.
                context.draw_line_stroke(
                    crate::core::Point { x: previous_x, y: previous_y },
                    crate::core::Point { x, y: previous_y },
                    color,
                    1,
                );
                context.draw_line_stroke(
                    crate::core::Point { x, y: previous_y },
                    crate::core::Point { x, y },
                    color,
                    1,
                );
            }
            previous = Some((x, y));
        }

        // A filled area under the curve, so the two sides read as volumes of available
        // size rather than as two lines crossing.
        let mut running = 0.0;
        let mut previous_x = None;
        for level in &levels {
            if !level.price.is_finite() || !level.quantity.is_finite() || level.quantity <= 0.0 {
                continue;
            }
            running += level.quantity;
            let x = to_x(level.price);
            let y = to_y(running);
            if let Some(from) = previous_x {
                context.fill_rect(
                    Rect::new(
                        from,
                        y,
                        (x - from).unsigned_abs().max(1),
                        (area.bottom() - y).max(0) as u32,
                    ),
                    // A translucent wash would need alpha compositing; a dark shade of the
                    // same hue reads the same way and costs one fill.
                    if color == self.bid_color {
                        Color::rgb(14, 48, 30)
                    } else {
                        Color::rgb(52, 20, 22)
                    },
                );
            }
            previous_x = Some(x);
        }
    }

    /// The price range the plot spans, including the spread's midpoint.
    fn price_bounds(&self) -> (f64, f64) {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for side in [DepthSide::Bid, DepthSide::Ask] {
            for level in self.levels(side) {
                if level.price.is_finite() {
                    low = low.min(level.price);
                    high = high.max(level.price);
                }
            }
        }
        if low > high {
            return (f64::NAN, f64::NAN);
        }
        (low, high)
    }
}

impl Widget for DepthChart {
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

impl crate::event::EventHandler for DepthChart {
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::Event;
        if let Event::MouseMove { pos } | Event::PointerMove { pos, .. } = event {
            let area = PlotArea::with_margins(self.base.geometry(), 48, 8, 8, 20);
            let (low, high) = self.price_bounds();
            if high > low && area.rect.width > 0 {
                let fraction = (pos.x - area.rect.x) as f64 / area.rect.width as f64;
                if (0.0..=1.0).contains(&fraction) {
                    self.level_hovered.emit(low + fraction * (high - low));
                }
            }
        }
        self.base.handle_event(event);
    }
}

impl Draw for DepthChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let area = PlotArea::with_margins(self.base.geometry(), 48, 8, 8, 20);
        if area.rect.width == 0 || area.rect.height == 0 {
            return;
        }
        context.fill_rect(area.rect, Color::rgb(18, 22, 28));

        let max_cumulative = self.max_cumulative();
        if max_cumulative <= 0.0 {
            return;
        }
        // Bids first so the asks, drawn second, are on top where they overlap near the
        // spread — the ask side is what a buyer reads, so it should not be occluded.
        self.draw_step_curve(context, &area, DepthSide::Bid, self.bid_color, max_cumulative);
        self.draw_step_curve(context, &area, DepthSide::Ask, self.ask_color, max_cumulative);

        // The mid price, marked so the spread is readable without a separate label.
        if let Some(mid) = self.book.mid_price() {
            let (low, high) = self.price_bounds();
            if high > low {
                let fraction = (mid - low) / (high - low);
                let x = area.rect.x + (fraction * area.rect.width as f64).round() as i32;
                context.draw_line_stroke(
                    crate::core::Point { x, y: area.rect.y },
                    crate::core::Point { x, y: area.bottom() },
                    Color::rgb(120, 120, 120),
                    1,
                );
            }
        }
    }
}

/// Test module for the depth chart.
impl WidgetProperties for DepthChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "depth" => Ok(CapabilityValue::UInt(self.depth as u64)),
            "bid_color" => Ok(CapabilityValue::Color(self.bid_color)),
            "ask_color" => Ok(CapabilityValue::Color(self.ask_color)),
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
            "bid_color" | "ask_color" => {
                let CapabilityValue::Color(color) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let (bid, ask) = if name == "bid_color" {
                    (color, self.ask_color)
                } else {
                    (self.bid_color, color)
                };
                self.set_colors(bid, ask);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["depth", "bid_color", "ask_color", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::special_widgets::finance::types::fixtures;

    /// The chart carries the book it was given.
    #[test]
    fn a_depth_chart_carries_its_book() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 480, 240));
        assert!(chart.book().bids().is_empty());
        chart.set_book(fixtures::sample_book(5));
        assert_eq!(chart.book().bids().len(), 5);
        assert_eq!(chart.book().asks().len(), 5);
    }

    /// The curve accumulates, and each side accumulates independently.
    ///
    /// This is what the pane draws, so an accumulation that ran across both sides would
    /// put the ask curve at twice its real height — a chart that is wrong by a factor
    /// nobody would notice by eye.
    #[test]
    fn the_curve_accumulates_per_side() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 480, 240));
        let mut book = crate::widget::special_widgets::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![
            crate::widget::special_widgets::finance::types::BookLevel::new(99.0, 10.0),
            crate::widget::special_widgets::finance::types::BookLevel::new(98.0, 20.0),
        ]);
        book.set_asks(alloc::vec![
            crate::widget::special_widgets::finance::types::BookLevel::new(101.0, 5.0),
            crate::widget::special_widgets::finance::types::BookLevel::new(102.0, 5.0),
        ]);
        chart.set_book(book);

        let curve = chart.curve();
        let bids: alloc::vec::Vec<_> =
            curve.iter().filter(|point| point.side == DepthSide::Bid).collect();
        let asks: alloc::vec::Vec<_> =
            curve.iter().filter(|point| point.side == DepthSide::Ask).collect();
        assert_eq!(bids.len(), 2);
        assert_eq!(asks.len(), 2);
        assert!((bids[0].cumulative_quantity - 10.0).abs() < 1e-9);
        assert!((bids[1].cumulative_quantity - 30.0).abs() < 1e-9, "bids keep accumulating");
        assert!((asks[0].cumulative_quantity - 5.0).abs() < 1e-9);
        assert!(
            (asks[1].cumulative_quantity - 10.0).abs() < 1e-9,
            "the ask total must restart, not continue from the bids"
        );
    }

    /// The depth limit trims both sides.
    #[test]
    fn the_depth_limit_trims_the_curve() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 480, 240));
        chart.set_book(fixtures::sample_book(10));
        assert_eq!(chart.curve().len(), 20, "all levels by default");
        chart.set_depth(3);
        assert_eq!(chart.depth(), 3);
        assert_eq!(chart.curve().len(), 6, "three levels per side");
    }

    /// Colours are stored and returned.
    #[test]
    fn colours_round_trip() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 480, 240));
        let (bid, ask) = chart.colors();
        assert_ne!(bid, ask, "the two sides must be distinguishable by default");
        chart.set_colors(crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6));
        assert_eq!(
            chart.colors(),
            (crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6))
        );
    }

    /// An empty book draws without panicking.
    #[test]
    fn an_empty_book_draws_without_panicking() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 320, 200));
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 200), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A one-sided book draws without panicking.
    ///
    /// This is what a book looks like for a moment after a snapshot arrives with only
    /// one side, and the mid price has no value.
    #[test]
    fn a_one_sided_book_draws_without_panicking() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 320, 200));
        let mut book = crate::widget::special_widgets::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![crate::widget::special_widgets::finance::types::BookLevel::new(
            99.0, 10.0,
        )]);
        chart.set_book(book);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 200), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A book of malformed levels draws without panicking.
    #[test]
    fn a_malformed_book_draws_without_panicking() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 320, 200));
        let mut book = crate::widget::special_widgets::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![
            crate::widget::special_widgets::finance::types::BookLevel::new(f64::NAN, f64::NAN),
            crate::widget::special_widgets::finance::types::BookLevel::new(99.0, -5.0),
        ]);
        book.set_asks(alloc::vec![crate::widget::special_widgets::finance::types::BookLevel::new(
            f64::INFINITY,
            1.0,
        )]);
        chart.set_book(book);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 200), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A flat price scale — every level at one price — draws without dividing by zero.
    #[test]
    fn a_flat_price_scale_draws_without_panicking() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 320, 200));
        let mut book = crate::widget::special_widgets::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![crate::widget::special_widgets::finance::types::BookLevel::new(
            100.0, 10.0,
        )]);
        book.set_asks(alloc::vec![crate::widget::special_widgets::finance::types::BookLevel::new(
            100.0, 10.0,
        )]);
        chart.set_book(book);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 200), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }
}
