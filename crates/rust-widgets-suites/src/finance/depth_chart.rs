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

use crate::finance::layout::{panel_colors, PanelColors, PlotArea};
use crate::finance::types::{
    book_levels_from_records, book_levels_to_records, BookLevel, OrderBook,
};
use crate::finance::volume_chart::draw_empty_pane;
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect};
use rust_widgets::render::RenderContext;
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

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
    pub level_hovered: rust_widgets::signal::Signal1<f64>,
    /// Emitted with the price level when a row is clicked.
    pub level_clicked: rust_widgets::signal::Signal1<f64>,
    /// The price of the level the pointer is nearest, once snapped.
    hovered_level: Option<f64>,
    /// Which side of the book that level belongs to.
    ///
    /// A price can hold a bid and an ask at the same time (a crossed book, or one level on
    /// either side of the mid at the same tick). The price alone cannot say which curve
    /// point the pointer meant, so the read-out used to resolve such a level to the bid —
    /// whichever the cache happened to hold first — even when the pointer was over the ask.
    hovered_side: Option<DepthSide>,
    /// The usable levels and the accumulated curve, rebuilt when the book or depth changes.
    bid_levels: Vec<BookLevel>,
    ask_levels: Vec<BookLevel>,
    curve_cache: Vec<DepthPoint>,
    max_cumulative_cache: f64,
    price_bounds_cache: (f64, f64),
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
            level_hovered: rust_widgets::signal::Signal1::new(),
            level_clicked: rust_widgets::signal::Signal1::new(),
            hovered_level: None,
            hovered_side: None,
            bid_levels: Vec::new(),
            ask_levels: Vec::new(),
            curve_cache: Vec::new(),
            max_cumulative_cache: 0.0,
            price_bounds_cache: (f64::NAN, f64::NAN),
        }
    }

    /// Rebuilds the filtered levels, the accumulated curve and the two scales.
    ///
    /// The two level vectors are **reused**: they are cleared and refilled in place, so a
    /// tick's rebuild allocates nothing. The previous form built a fresh `Vec` per side (and
    /// a second one to filter it), which made every order-book push three allocations
    /// (J-P17).
    fn rebuild(&mut self) {
        Self::fill_plottable_into(&mut self.bid_levels, self.book.bids(), self.depth);
        Self::fill_plottable_into(&mut self.ask_levels, self.book.asks(), self.depth);
        self.curve_cache.clear();
        let mut running = 0.0;
        for level in &self.bid_levels {
            running += level.quantity;
            self.curve_cache.push(DepthPoint {
                price: level.price,
                cumulative_quantity: running,
                side: DepthSide::Bid,
            });
        }
        let mut running = 0.0;
        for level in &self.ask_levels {
            running += level.quantity;
            self.curve_cache.push(DepthPoint {
                price: level.price,
                cumulative_quantity: running,
                side: DepthSide::Ask,
            });
        }
        let peak = |levels: &[BookLevel]| levels.iter().map(|level| level.quantity).sum::<f64>();
        self.max_cumulative_cache = peak(&self.bid_levels).max(peak(&self.ask_levels));
        self.price_bounds_cache = Self::compute_price_bounds(&self.bid_levels, &self.ask_levels);
        if let Some(price) = self.hovered_level {
            let side = self.hovered_side;
            if !self
                .curve_cache
                .iter()
                .any(|point| point.price == price && side.is_none_or(|side| point.side == side))
            {
                self.hovered_level = None;
                self.hovered_side = None;
            }
        }
    }

    fn compute_price_bounds(bids: &[BookLevel], asks: &[BookLevel]) -> (f64, f64) {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for level in bids.iter().chain(asks.iter()) {
            low = low.min(level.price);
            high = high.max(level.price);
        }
        if low > high {
            return (f64::NAN, f64::NAN);
        }
        if high <= low {
            let magnitude = low.abs().max(1.0) * 0.001;
            return (low - magnitude, high + magnitude);
        }
        (low, high)
    }

    /// The hovered level's anchor and suggested tooltip content, for an external read-out.
    ///
    /// The anchor is the curve point under the pointer, computed by the same mapping the painter
    /// and hit test use (gap 10).
    pub fn hover_anchor(
        &self,
    ) -> Option<(Point, Vec<rust_widgets::widget::companions::TooltipLine>)> {
        let point = self.hovered_point()?;
        let area = self.plot_area();
        if area.rect.width == 0 || area.rect.height == 0 {
            return None;
        }
        let (low, high) = self.price_bounds();
        if !low.is_finite() || !high.is_finite() || high <= low {
            return None;
        }
        let max = self.max_cumulative();
        if max <= 0.0 {
            return None;
        }
        let fraction = ((point.price - low) / (high - low)).clamp(0.0, 1.0);
        let x = area.rect.x + (fraction * area.rect.width as f64).round() as i32;
        let y = area.bottom()
            - ((point.cumulative_quantity / max).clamp(0.0, 1.0) * area.rect.height as f64).round()
                as i32;
        Some((Point::new(x, y), self.hovered_summary()))
    }

    /// The plot area for this control's geometry.
    ///
    /// Shared by `draw` and `handle_event` rather than built twice: the pointer-to-price
    /// mapping is only correct while the rectangle it inverts is the rectangle the curve was
    /// drawn in, and two copies of the margin list are two chances for those to disagree.
    fn plot_area(&self) -> PlotArea {
        PlotArea::price_pane(self.base.geometry())
    }

    /// The pane's chrome, resolved from this control's own style first and the theme second.
    fn chrome(&self) -> PanelColors {
        panel_colors(Some(self.base.style()))
    }

    /// The book being plotted.
    pub fn book(&self) -> &OrderBook {
        &self.book
    }

    /// Replaces the book and requests a repaint.
    pub fn set_book(&mut self, book: OrderBook) {
        self.book = book;
        self.rebuild();
        self.base.request_redraw();
    }

    /// Applies one bid level edit, returning whether the **drawn curve** changed.
    ///
    /// The incremental counterpart of [`Self::set_book`]. The plotted levels and the cumulative
    /// curve are rebuilt from the edited book, so the drawn picture matches the model without a
    /// full snapshot being handed in.
    ///
    /// The answer is the curve's, not the book's: with a bounded [`Self::depth`], an edit outside
    /// the plotted window changes the book while every pixel stays identical, and a caller that
    /// trusts the answer to refresh its own read-out would do that work for nothing.
    pub fn apply_bid_level(&mut self, price: f64, quantity: f64) -> bool {
        let before = self.curve_snapshot();
        if !self.book.apply_bid_level(price, quantity) {
            return false;
        }
        self.rebuild_after_edit(before)
    }

    /// Applies one ask level edit, returning whether the **drawn curve** changed.
    pub fn apply_ask_level(&mut self, price: f64, quantity: f64) -> bool {
        let before = self.curve_snapshot();
        if !self.book.apply_ask_level(price, quantity) {
            return false;
        }
        self.rebuild_after_edit(before)
    }

    /// The plotted levels, whose equality is exactly "the curve is the same picture".
    ///
    /// `BookLevel` is `Copy`, so this is a short clone of at most `depth` entries per side.
    fn curve_snapshot(&self) -> (Vec<BookLevel>, Vec<BookLevel>) {
        (self.bid_levels.clone(), self.ask_levels.clone())
    }

    /// Rebuilds after an incremental edit and reports whether the curve changed.
    fn rebuild_after_edit(&mut self, before: (Vec<BookLevel>, Vec<BookLevel>)) -> bool {
        let hover_before = (self.hovered_level, self.hovered_side);
        self.rebuild();
        let curve_changed = before.0 != self.bid_levels || before.1 != self.ask_levels;
        // `rebuild` drops a hover whose level left the window; that is a pixel change too.
        let hover_changed = hover_before != (self.hovered_level, self.hovered_side);
        if curve_changed || hover_changed {
            self.base.request_redraw();
        }
        curve_changed
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
        if self.depth == depth {
            return;
        }
        self.depth = depth;
        self.rebuild();
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
        self.curve_cache.clone()
    }

    /// The price of the level the pointer is currently nearest, if any.
    pub fn hovered_level(&self) -> Option<f64> {
        self.hovered_level
    }

    /// The hovered point's readings, ready for a `ChartTooltip`.
    ///
    /// The price, the cumulative quantity at it and which side of the book it belongs to,
    /// taken from the same `curve_cache` the step curve is drawn from (COMP-J02).
    pub fn hovered_summary(&self) -> Vec<rust_widgets::widget::companions::TooltipLine> {
        use rust_widgets::widget::companions::TooltipLine;
        let Some(point) = self.hovered_point() else { return Vec::new() };
        let price = point.price;
        let color = match point.side {
            DepthSide::Bid => self.bid_color,
            DepthSide::Ask => self.ask_color,
        };
        alloc::vec![
            TooltipLine::new(alloc::format!("{price:.2}")),
            TooltipLine::new(alloc::format!("cum {:.0}", point.cumulative_quantity)),
            TooltipLine::colored(
                match point.side {
                    DepthSide::Bid => "bid",
                    DepthSide::Ask => "ask",
                }
                .to_string(),
                color,
            ),
        ]
    }

    /// The curve point the pointer is on, honouring the side it was snapped to.
    ///
    /// A price may appear on both sides of a crossed book, so a price-only lookup could
    /// report the bid's cumulative total while the pointer was over the ask's point.
    fn hovered_point(&self) -> Option<DepthPoint> {
        let price = self.hovered_level?;
        let side = self.hovered_side;
        self.curve_cache
            .iter()
            .copied()
            .find(|point| point.price == price && side.is_none_or(|side| point.side == side))
    }

    /// The level price nearest `price`, across both sides of the book.
    ///
    /// The chart's hover used to report the interpolated pointer position, which is not a
    /// price any order rests at: a caller highlighting the reported level had to search the
    /// book for an approximate match. Snapping answers the question the signal asks.
    ///
    /// The side travels with the price so the read-out can pick the same curve point the
    /// pointer is nearest when both sides hold a level at that price. A crossed book puts a
    /// bid and an ask at the same x, so the pointer's height is the tie-break that says
    /// which of the two curve points it is on.
    fn nearest_level(&self, price: f64, pos_y: i32) -> Option<(f64, DepthSide)> {
        let area = self.plot_area();
        let max = self.max_cumulative();
        let to_y = |cumulative: f64| -> i32 {
            if max <= 0.0 {
                area.bottom()
            } else {
                let fraction = (cumulative / max).clamp(0.0, 1.0);
                area.bottom() - (fraction * area.rect.height as f64).round() as i32
            }
        };
        let mut best: Option<(DepthPoint, f64, f64)> = None;
        for point in &self.curve_cache {
            let price_distance = (point.price - price).abs();
            let pixel_distance = (to_y(point.cumulative_quantity) - pos_y).abs() as f64;
            let better = match best {
                None => true,
                Some((_, best_price, best_pixel)) => {
                    if (price_distance - best_price).abs() > f64::EPSILON {
                        price_distance < best_price
                    } else {
                        pixel_distance < best_pixel
                    }
                }
            };
            if better {
                best = Some((*point, price_distance, pixel_distance));
            }
        }
        best.map(|(point, _, _)| (point.price, point.side))
    }

    /// The spread between the best bid and best ask that the curve plots, if both sides have one.
    ///
    /// Both come from the *plotted* levels: the bounds, scale and segments are built from them,
    /// and reading the raw top of book instead let a zero-size level name a spread the picture
    /// does not contain.
    fn spread(&self) -> Option<(f64, f64)> {
        let bid = self.bid_levels.first()?.price;
        let ask = self.ask_levels.first()?.price;
        (ask > bid).then_some((bid, ask))
    }

    /// Fills `out` with the plottable levels of one side, reusing its allocation.
    ///
    /// # Why the filter lives here rather than inside the draw loop
    ///
    /// A book routinely contains levels the curve cannot use — an exchange publishes a
    /// zero-size level for every price in range, and a malformed tick can carry a `NAN`.
    /// The scale (`max_cumulative`), the price bounds (`price_bounds`) and the segments
    /// must all be computed from the *same* set of levels, or the axis is stretched by
    /// data that is never drawn and a skipped level silently removes a segment.
    /// Skipping inside the loop while the bounds still counted the skipped level was
    /// exactly that bug: one zero-size level could erase the whole curve.
    ///
    /// The filter runs **before** the depth cut, so `depth` counts levels that are actually
    /// plotted: cutting first let zero-size levels at the top of the book consume the
    /// budget and keep real size out of the curve.
    fn fill_plottable_into(out: &mut Vec<BookLevel>, levels: &[BookLevel], depth: usize) {
        out.clear();
        out.extend(levels.iter().copied().filter(|level| {
            level.price.is_finite() && level.quantity.is_finite() && level.quantity > 0.0
        }));
        if depth > 0 {
            out.truncate(depth);
        }
    }

    /// The quantity the plot scales against: the largest cumulative total on either side.
    ///
    /// Deliberately the *cumulative* maximum rather than the largest single level, because
    /// the y axis carries the accumulation. Scaling against a single level would push the
    /// curve off the top of the pane at its far end.
    fn max_cumulative(&self) -> f64 {
        self.max_cumulative_cache
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
        let levels = match side {
            DepthSide::Bid => &self.bid_levels,
            DepthSide::Ask => &self.ask_levels,
        };
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

        // A filled area under the curve, so the two sides read as volumes of available
        // size rather than as two lines crossing.
        //
        // The wash is a step from the side's own colour toward the pane's surface, so it
        // follows the appearance instead of being one of two near-black slabs. It is
        // painted **before** the stroke: the fill is near-opaque, so painting it afterwards
        // covered the very line it is meant to sit under.
        //
        // Each band spans the prices between two levels and hangs from the *previous*
        // cumulative total — the value the step carries across that span. Anchoring it at
        // the current level's total made every band one level taller than the curve.
        let wash = color.blend(&self.chrome().surface, 0.80);
        let mut running = 0.0;
        let mut previous: Option<(i32, i32)> = None;
        for level in levels {
            running += level.quantity;
            let x = to_x(level.price);
            let y = to_y(running);
            if let Some((from_x, from_y)) = previous {
                // `from_x.min(x)`: bids are ordered best-first, so `x` decreases as the loop
                // advances. Anchoring at `from` with an unsigned width painted the band one
                // step toward the spread instead of the span it belongs to.
                context.fill_rect(
                    Rect::new(
                        from_x.min(x),
                        from_y,
                        (x - from_x).unsigned_abs().max(1),
                        (area.bottom() - from_y).max(0) as u32,
                    ),
                    wash,
                );
            }
            previous = Some((x, y));
        }

        let mut previous: Option<(i32, i32)> = None;
        let mut running = 0.0;
        // Walk away from the touch, so the curve starts at the spread and grows outward.
        for level in levels {
            running += level.quantity;
            let x = to_x(level.price);
            let y = to_y(running);
            match previous {
                Some((previous_x, previous_y)) => {
                    // The horizontal segment carries the previous total across to this level,
                    // which is what makes it a step rather than a staircase of diagonals.
                    context.draw_line_stroke(
                        rust_widgets::core::Point { x: previous_x, y: previous_y },
                        rust_widgets::core::Point { x, y: previous_y },
                        color,
                        1,
                    );
                    context.draw_line_stroke(
                        rust_widgets::core::Point { x, y: previous_y },
                        rust_widgets::core::Point { x, y },
                        color,
                        1,
                    );
                }
                // A lone level has no run to draw a step on, so it is drawn as the vertical
                // from the baseline to its own total. Without this the whole curve is absent
                // for a side that holds exactly one usable level — which is the normal state
                // of a one-level book, not an edge case.
                None => {
                    context.draw_line_stroke(
                        rust_widgets::core::Point { x, y: area.bottom() },
                        rust_widgets::core::Point { x, y },
                        color,
                        1,
                    );
                }
            }
            previous = Some((x, y));
        }
    }

    /// The price range the plot spans.
    ///
    /// # Why a degenerate range is widened rather than rejected
    ///
    /// A book can legitimately have every usable level at one price — a one-level book, or
    /// a side whose other levels are all zero-size. Returning a zero-width range would make
    /// `high_price <= low_price` true and every caller bail out, so the pane would go blank
    /// for data that is perfectly drawable. The range is therefore widened by a nominal
    /// amount around the single price, which is the same degradation `PriceAxis::new` uses,
    /// and the lone level then draws through the middle of the pane.
    fn price_bounds(&self) -> (f64, f64) {
        self.price_bounds_cache
    }
}

impl Widget for DepthChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("depth_chart")
    }
    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
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
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "level_hovered" => {
                Some(EventSignalRef::mapped("level_hovered", &self.level_hovered, |v| {
                    CapabilityValue::Float(*v)
                }))
            }
            "level_clicked" => {
                Some(EventSignalRef::mapped("level_clicked", &self.level_clicked, |v| {
                    CapabilityValue::Float(*v)
                }))
            }
            _ => None,
        }
    }
}

impl rust_widgets::event::EventHandler for DepthChart {
    fn handle_event(&mut self, event: &rust_widgets::event::Event) {
        use rust_widgets::event::Event;
        // A disabled curve must not emit `level_hovered`/`level_clicked` for a picture that is
        // drawn as disabled; the same guard the K-line and volume panes carry (FIN-8).
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let area = self.plot_area();
                let (low, high) = self.price_bounds();
                // A pointer outside the plot (or over a curve with no usable level) has no
                // snapped level, and the previous hover must be dropped rather than left
                // marking a level the pointer has left. The pane's bottom strip is its price
                // axis, so a y test is part of "inside": testing only x let the axis row hover
                // (and click) a level.
                let inside_y = pos.y >= area.rect.y && pos.y < area.bottom();
                let mut next = None;
                if inside_y && high > low && area.rect.width > 0 {
                    let fraction = (pos.x - area.rect.x) as f64 / area.rect.width as f64;
                    if (0.0..=1.0).contains(&fraction) {
                        let pointer = low + fraction * (high - low);
                        next = self.nearest_level(pointer, pos.y);
                    }
                }
                let next_price = next.map(|(price, _)| price);
                let next_side = next.map(|(_, side)| side);
                if next_price != self.hovered_level || next_side != self.hovered_side {
                    self.hovered_level = next_price;
                    self.hovered_side = next_side;
                    if let Some(level) = next_price {
                        self.level_hovered.emit(level);
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                let had_level = self.hovered_level.take().is_some();
                self.hovered_side = None;
                if had_level {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, .. } | Event::PointerPress { pos, .. } => {
                let area = self.plot_area();
                let (low, high) = self.price_bounds();
                let inside_y = pos.y >= area.rect.y && pos.y < area.bottom();
                if inside_y && high > low && area.rect.width > 0 {
                    let fraction = (pos.x - area.rect.x) as f64 / area.rect.width as f64;
                    if (0.0..=1.0).contains(&fraction) {
                        let pointer = low + fraction * (high - low);
                        if let Some((level, side)) = self.nearest_level(pointer, pos.y) {
                            // A press between two moves changes the hover too, so the
                            // `level_hovered` contract holds for clicks as well rather than
                            // the press silently moving the mark the signal did not report.
                            if self.hovered_level != Some(level) || self.hovered_side != Some(side)
                            {
                                self.hovered_level = Some(level);
                                self.hovered_side = Some(side);
                                self.level_hovered.emit(level);
                            }
                            self.level_clicked.emit(level);
                            self.base.request_redraw();
                        }
                    }
                }
            }
            _ => {}
        }
        self.base.handle_event(event);
    }
}

impl Draw for DepthChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let area = self.plot_area();
        if area.rect.width == 0 || area.rect.height == 0 {
            return;
        }
        // The pane surface resolves the caller's style first and the theme second. The bid
        // and ask colours below are which side of the book a point belongs to: they are data,
        // and `set_colors` is the caller's way to change them.
        let chrome = self.chrome();
        context.fill_rect(area.rect, chrome.surface);

        let max_cumulative = self.max_cumulative();
        // No usable level: the book is empty or every level is zero-size, and the curve
        // would be two invisible hairlines on the baseline. The pane draws its frame and a
        // message instead of leaving the slab this used to return after.
        if max_cumulative <= 0.0 {
            draw_empty_pane(context, &area, chrome, &role_font(self.style(), 12.0));
            return;
        }

        // A price/quantity scale: the curve's whole reading is "how much is available by
        // here", which is a comparison against the baseline, so the gridlines are what turn
        // it from a shape into a quantity.
        let axis_font = role_font(self.style(), 10.0);
        let label_left = area.rect.x - area.left_margin;
        let label_width = (area.label_x() - label_left).max(0) as u32;
        for step in 0..=3 {
            let y = area.rect.y + area.rect.height as i32 * step / 3;
            context.draw_line(
                rust_widgets::core::Point { x: area.rect.x, y },
                rust_widgets::core::Point { x: area.right(), y },
                chrome.grid,
            );
            // The quantity this gridline stands for, so a curve's height is readable.
            let fraction = 1.0 - step as f64 / 3.0;
            let value = max_cumulative * fraction;
            super::volume_chart::with_axis_label(|text| {
                let _ = core::fmt::Write::write_fmt(&mut *text, format_args!("{value:.0}"));
                let line_height = context.measure_text(text, &axis_font).height.max(1);
                context.draw_text_fitted_ellipsis(
                    Rect::new(label_left, y - line_height as i32 / 2, label_width, line_height),
                    text,
                    &axis_font,
                    chrome.ink.with_alpha(190),
                    HorizontalAlignment::Right,
                );
            });
        }

        // Bids first so the asks, drawn second, are on top where they overlap near the
        // spread — the ask side is what a buyer reads, so it should not be occluded.
        self.draw_step_curve(context, &area, DepthSide::Bid, self.bid_color, max_cumulative);
        self.draw_step_curve(context, &area, DepthSide::Ask, self.ask_color, max_cumulative);

        // The mid price, marked so the spread is readable without a separate label. Derived from
        // the plotted top-of-book: the scale, bounds and segments all come from that set, so a
        // raw mid could be painted outside the curve (and the control).
        let mid = match (self.bid_levels.first(), self.ask_levels.first()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / 2.0),
            _ => None,
        };
        if let Some(mid) = mid {
            let (low, high) = self.price_bounds();
            if high > low {
                let fraction = (mid - low) / (high - low);
                let x = (area.rect.x + (fraction * area.rect.width as f64).round() as i32)
                    .clamp(area.rect.x, area.right());
                context.draw_line_stroke(
                    rust_widgets::core::Point { x, y: area.rect.y },
                    rust_widgets::core::Point { x, y: area.bottom() },
                    chrome.crosshair,
                    1,
                );
                // The spread annotation: the bid, the ask and their distance, on the mid line.
                // A mid line alone says where the price is but not how wide the market is.
                if let Some((bid, ask)) = self.spread() {
                    let font = role_font(self.style(), 10.0);
                    // The annotation is written into the finance panes' shared label buffer:
                    // this site used to allocate a fresh `String` every frame (FIN-10).
                    super::volume_chart::with_axis_label(|text| {
                        let _ = core::fmt::Write::write_fmt(
                            &mut *text,
                            format_args!("{bid:.2} / {ask:.2}  spread {:.2}", ask - bid),
                        );
                        let width = context.measure_text(text, &font).width as i32;
                        let x_text = (x + 4).min(area.right() - width).max(area.rect.x);
                        context.draw_text(
                            rust_widgets::core::Point { x: x_text, y: area.rect.y + 2 },
                            text,
                            &font,
                            chrome.ink,
                            HorizontalAlignment::Left,
                        );
                    });
                }
            }
        }

        // The price axis under the curve. Without it the picture answered "what shape"
        // but not "at what price"; the hover readout was the only way to recover a level
        // (FIN-05).
        let (low, high) = self.price_bounds();
        if high > low && area.bottom_margin >= 14 {
            let font = role_font(self.style(), 10.0);
            const TICKS: i32 = 5;
            let band = ((area.rect.width as i32 / TICKS).max(40)) as u32;
            let y = area.bottom() + 2;
            for step in 0..=TICKS {
                let fraction = step as f64 / TICKS as f64;
                let price = low + fraction * (high - low);
                let x = area.rect.x + (fraction * area.rect.width as f64).round() as i32;
                // Slide the label's band inward rather than letting it hang over the edge: a band
                // centred on the last tick ran past the pane, and the painter's clip turned
                // `121.30` into `1213` — a wrong number, not a cropped one.
                let band_left = (x - band as i32 / 2)
                    .clamp(area.rect.x, (area.right() - band as i32).max(area.rect.x));
                // Six ticks per frame, each formatted into the shared buffer instead of a
                // fresh `String` (FIN-10).
                super::volume_chart::with_axis_label(|text| {
                    let _ = core::fmt::Write::write_fmt(&mut *text, format_args!("{price:.2}"));
                    context.draw_text_fitted_ellipsis(
                        Rect::new(band_left, y, band, 12),
                        text,
                        &font,
                        chrome.ink.with_alpha(190),
                        HorizontalAlignment::Center,
                    );
                });
            }
        }

        // The hovered level gets a visible mark and its quantity, so the already-emitted
        // `level_hovered` signal is not information the picture withholds.
        if let Some(point) = self.hovered_point() {
            let level = point.price;
            let quantity = point.cumulative_quantity;
            let (low, high) = self.price_bounds();
            if high > low {
                let fraction = (level - low) / (high - low);
                let x = area.rect.x + (fraction * area.rect.width as f64).round() as i32;
                let fraction_y = (quantity / max_cumulative).clamp(0.0, 1.0);
                let y = area.bottom() - (fraction_y * area.rect.height as f64).round() as i32;
                context.fill_circle(rust_widgets::core::Point { x, y }, 3, chrome.crosshair);
                let font = role_font(self.style(), 10.0);
                // The hovered level's readout reuses the shared buffer, so a live hover does
                // not allocate a label per frame (FIN-10).
                super::volume_chart::with_axis_label(|text| {
                    let _ = core::fmt::Write::write_fmt(
                        &mut *text,
                        format_args!("{level:.2} · {quantity:.0}"),
                    );
                    let width = context.measure_text(text, &font).width as i32;
                    let x_text = (x + 6).min(area.right() - width).max(area.rect.x);
                    context.draw_text(
                        rust_widgets::core::Point { x: x_text, y: (y - 14).max(area.rect.y) },
                        text,
                        &font,
                        chrome.ink,
                        HorizontalAlignment::Left,
                    );
                });
            }
        }
        crate::veil_if_disabled(&self.base, context);
    }
}

/// Test module for the depth chart.
impl WidgetProperties for DepthChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // RW-106: the curve is derived from these two ladders, so they are what a document
            // writes; the read half reports exactly what was written.
            "bids" => Ok(CapabilityValue::Records(book_levels_to_records(self.book.bids()))),
            "asks" => Ok(CapabilityValue::Records(book_levels_to_records(self.book.asks()))),
            "depth" => Ok(CapabilityValue::UInt(self.depth as u64)),
            "bid_color" => Ok(CapabilityValue::Color(self.bid_color)),
            "ask_color" => Ok(CapabilityValue::Color(self.ask_color)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "bids" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let levels = book_levels_from_records(&list, "bids")?;
                let mut book = self.book.clone();
                book.set_bids(levels);
                self.set_book(book);
                Ok(())
            }
            "asks" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let levels = book_levels_from_records(&list, "asks")?;
                let mut book = self.book.clone();
                book.set_asks(levels);
                self.set_book(book);
                Ok(())
            }
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
        property_names_of!["bids", "asks", "depth", "bid_color", "ask_color", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `depth_chart` publishes.
    ///
    /// Both published names are value-carrying `set_*` commands, so the convention answers
    /// [`CapabilityAccessError::OutOfRange`] for a bare invocation and the caller writes the
    /// property instead. There is no payload-free verb on this control.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        self.default_command(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finance::types::fixtures;
    use rust_widgets::event::EventHandler;

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
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![
            crate::finance::types::BookLevel::new(99.0, 10.0),
            crate::finance::types::BookLevel::new(98.0, 20.0),
        ]);
        book.set_asks(alloc::vec![
            crate::finance::types::BookLevel::new(101.0, 5.0),
            crate::finance::types::BookLevel::new(102.0, 5.0),
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

    /// An incremental edit answers for the **drawn curve**, not for the book.
    ///
    /// With a bounded depth, editing a level outside the plotted window changes the book while
    /// every plotted point stays identical; the method used to return `true` (and ask for a
    /// repaint) for that no-op picture.
    #[test]
    fn an_edit_outside_the_plotted_depth_reports_no_change() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 480, 240));
        chart.set_book(fixtures::sample_book(10));
        chart.set_depth(2);

        // 99.7 is the third bid; the top two are 99.9 and 99.8.
        assert!(!chart.apply_bid_level(99.7, 55.0), "the plotted curve is unchanged");
        assert!(chart.book().bids().iter().any(|level| (level.price - 99.7).abs() < 1e-9));

        assert!(chart.apply_bid_level(99.9, 120.0), "the top level is plotted, so it changed");
        assert!(!chart.apply_bid_level(99.9, 120.0), "an idempotent write changes nothing");
    }

    /// Colours are stored and returned.
    #[test]
    fn colours_round_trip() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 480, 240));
        let (bid, ask) = chart.colors();
        assert_ne!(bid, ask, "the two sides must be distinguishable by default");
        chart.set_colors(
            rust_widgets::core::Color::rgb(1, 2, 3),
            rust_widgets::core::Color::rgb(4, 5, 6),
        );
        assert_eq!(
            chart.colors(),
            (rust_widgets::core::Color::rgb(1, 2, 3), rust_widgets::core::Color::rgb(4, 5, 6))
        );
    }

    /// An empty book draws without panicking.
    #[test]
    fn an_empty_book_draws_without_panicking() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 320, 200));
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(320, 200),
            1.0,
        );
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
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![crate::finance::types::BookLevel::new(99.0, 10.0,)]);
        chart.set_book(book);
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(320, 200),
            1.0,
        );
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A book of malformed levels draws without panicking.
    #[test]
    fn a_malformed_book_draws_without_panicking() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 320, 200));
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![
            crate::finance::types::BookLevel::new(f64::NAN, f64::NAN),
            crate::finance::types::BookLevel::new(99.0, -5.0),
        ]);
        book.set_asks(alloc::vec![crate::finance::types::BookLevel::new(f64::INFINITY, 1.0,)]);
        chart.set_book(book);
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(320, 200),
            1.0,
        );
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A flat price scale — every level at one price — draws without dividing by zero.
    #[test]
    fn a_flat_price_scale_draws_without_panicking() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 320, 200));
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![crate::finance::types::BookLevel::new(100.0, 10.0,)]);
        book.set_asks(alloc::vec![crate::finance::types::BookLevel::new(100.0, 10.0,)]);
        chart.set_book(book);
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(320, 200),
            1.0,
        );
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// The pane must paint a curve, not merely survive, when a level it cannot use is
    /// present.
    ///
    /// Regression: the draw loop `continue`d past a zero-size or non-finite level while
    /// the scale and the price bounds still counted it, so one such level could leave the
    /// pane completely blank — the earlier test only asserted "does not panic", which the
    /// blank pane satisfied. A zero-size level is a normal live-book state, so this is a
    /// real failure mode rather than a contrived input.
    #[test]
    fn a_zero_size_level_does_not_erase_the_curve() {
        let geometry = Rect::new(0, 0, 400, 200);
        let mut book = crate::finance::types::OrderBook::new();
        // The zero-size level is first, so it is the one the old loop skipped first.
        book.set_bids(alloc::vec![
            crate::finance::types::BookLevel::new(100.0, 0.0),
            crate::finance::types::BookLevel::new(99.0, 100.0),
        ]);
        book.set_asks(alloc::vec![crate::finance::types::BookLevel::new(101.0, 100.0,)]);
        let mut chart = DepthChart::new(geometry);
        chart.set_book(book);

        assert!(
            pane_contains_bid_colour(&mut chart, geometry),
            "a zero-size level must not erase the depth curve"
        );
        assert_eq!(chart.curve().len(), 2, "one bid and one ask are plottable");
    }

    /// A book whose only bid price is non-finite must still draw its ask side.
    ///
    /// Regression: `max_cumulative` and `price_bounds` both folded the `NAN`-priced level
    /// into their arithmetic, so the whole pane went blank. A `NAN` price is now dropped
    /// at the book boundary, and the bounds widen a single-price side so it stays drawable.
    #[test]
    fn a_non_finite_price_does_not_erase_the_curve() {
        let geometry = Rect::new(0, 0, 400, 200);
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![crate::finance::types::BookLevel::new(f64::NAN, 1000.0,)]);
        book.set_asks(alloc::vec![crate::finance::types::BookLevel::new(101.0, 100.0,)]);
        let mut chart = DepthChart::new(geometry);
        chart.set_book(book);

        assert!(
            pane_contains_ask_colour(&mut chart, geometry),
            "the ask curve must survive a NAN-priced bid"
        );
    }

    /// Whether any pixel of the pane carries the bid curve's colour.
    fn pane_contains_bid_colour(chart: &mut DepthChart, geometry: Rect) -> bool {
        use rust_widgets::render::PaintBackend as _;
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(geometry.width, geometry.height),
            1.0,
        );
        backend.begin_frame(rust_widgets::core::Color::BLACK);
        {
            let mut context = RenderContext::new(&mut backend);
            chart.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let target = [38u8, 166, 91, 255];
        rgba.as_chunks::<4>().0.contains(&target)
    }

    /// Whether any pixel of the pane carries the ask curve's colour.
    fn pane_contains_ask_colour(chart: &mut DepthChart, geometry: Rect) -> bool {
        use rust_widgets::render::PaintBackend as _;
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(geometry.width, geometry.height),
            1.0,
        );
        backend.begin_frame(rust_widgets::core::Color::BLACK);
        {
            let mut context = RenderContext::new(&mut backend);
            chart.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let target = [220u8, 68, 70, 255];
        rgba.as_chunks::<4>().0.contains(&target)
    }

    /// A pointer that leaves the plot drops its hovered level.
    ///
    /// Regression: the move handler only acted when the pointer was inside the plot, so a
    /// pointer that moved into the price axis kept the previous level marked and its readout
    /// on screen.
    #[test]
    fn moving_the_pointer_off_the_plot_clears_the_hover() {
        use rust_widgets::event::EventHandler as _;
        let mut chart = DepthChart::new(Rect::new(0, 0, 400, 200));
        chart.set_book(fixtures::sample_book(4));

        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x: 200, y: 100 },
        });
        assert!(chart.hovered_level().is_some(), "the pointer is over the curve");

        // The plot starts at x = 48 (the shared left margin), so x = 4 is in the price axis.
        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x: 4, y: 100 },
        });
        assert_eq!(chart.hovered_level(), None, "a pointer outside the plot has no level");
        assert!(chart.hovered_summary().is_empty());
    }

    /// RW-80: a book tick can edit one level without a full snapshot.
    #[test]
    fn levels_can_be_applied_incrementally() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 480, 240));
        chart.set_book(fixtures::sample_book(2));

        assert!(chart.apply_bid_level(100.5, 42.0));
        assert_eq!(chart.book().bids().len(), 3);
        assert_eq!(chart.book().bids()[0].price, 100.5);
        assert_eq!(chart.curve().iter().filter(|point| point.price == 100.5).count(), 1);

        assert!(chart.apply_ask_level(100.1, 0.0));
        assert_eq!(chart.book().asks().len(), 1, "a zero quantity removes the level");

        assert!(!chart.apply_bid_level(f64::NAN, 1.0), "a non-finite price is refused");
    }

    /// The price-axis row is not a hover target.
    ///
    /// Only `pos.x` was tested, so a pointer on the tick strip at the bottom of the pane snapped
    /// to a level the curve does not cover — and a press there emitted `level_clicked` for it.
    #[test]
    fn a_pointer_on_the_price_tick_row_has_no_level() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 400, 200));
        chart.set_book(fixtures::sample_book(4));
        let area = chart.plot_area();
        let inside = area.rect.y + area.rect.height as i32 / 2;
        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x: 200, y: inside },
        });
        assert!(chart.hovered_level().is_some(), "the curve's own row hovers a level");

        let clicked = std::sync::Arc::new(std::sync::Mutex::new(Vec::<f64>::new()));
        let sink = clicked.clone();
        chart.level_clicked.connect(move |level: std::sync::Arc<f64>| {
            sink.lock().unwrap().push(*level);
        });
        let on_axis = rust_widgets::core::Point { x: 200, y: area.bottom() + 2 };
        chart.handle_event(&rust_widgets::event::Event::MouseMove { pos: on_axis });
        assert_eq!(chart.hovered_level(), None, "the price axis is not the curve");
        chart.handle_event(&rust_widgets::event::Event::MousePress {
            pos: on_axis,
            button: 1,
            modifiers: 0,
        });
        assert!(clicked.lock().unwrap().is_empty(), "a press there clicks no level");
    }

    /// The mid and the spread come from the levels the curve plots.
    ///
    /// The curve filters zero-size levels; reading the raw top of book for the mid could put the
    /// line at a price outside the painted bounds (and outside the control).
    #[test]
    fn the_mid_line_uses_the_levels_the_curve_plots() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 400, 200));
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![
            crate::finance::types::BookLevel::new(105.0, 0.0),
            crate::finance::types::BookLevel::new(99.0, 10.0),
        ]);
        book.set_asks(alloc::vec![
            crate::finance::types::BookLevel::new(106.0, 0.0),
            crate::finance::types::BookLevel::new(101.0, 10.0),
        ]);
        chart.set_book(book);

        assert_eq!(
            chart.spread(),
            Some((99.0, 101.0)),
            "the zero-size top levels are not in the picture"
        );
        let (low, high) = chart.price_bounds();
        let mid = (chart.bid_levels[0].price + chart.ask_levels[0].price) / 2.0;
        assert!(low <= mid && mid <= high, "the mid must sit inside the painted bounds");
    }

    /// The depth cut counts levels the curve actually plots.
    ///
    /// Regression: the cut ran before the zero-size filter, so an exchange's empty levels at
    /// the top of the book consumed the budget and the curve could be blank at `depth = 2`
    /// even with real size resting further out.
    #[test]
    fn a_zero_size_level_does_not_consume_the_depth_budget() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 400, 200));
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![
            crate::finance::types::BookLevel::new(100.5, 0.0),
            crate::finance::types::BookLevel::new(100.4, 0.0),
            crate::finance::types::BookLevel::new(100.3, 4.0),
            crate::finance::types::BookLevel::new(100.2, 5.0),
        ]);
        book.set_asks(alloc::vec![crate::finance::types::BookLevel::new(100.6, 4.0)]);
        chart.set_book(book);
        chart.set_depth(2);

        let bids: Vec<f64> = chart
            .curve()
            .iter()
            .filter(|point| point.side == DepthSide::Bid)
            .map(|point| point.price)
            .collect();
        assert_eq!(
            bids,
            alloc::vec![100.3, 100.2],
            "the two plotted levels are the first two with size, not the two empty ones"
        );
    }

    /// A price that holds both a bid and an ask resolves to the point the pointer is on.
    ///
    /// Regression: the hover carried only a price, and the read-out resolved it by price
    /// alone, so it reported the bid's cumulative total for a pointer over the ask's point.
    #[test]
    fn a_crossed_level_reports_the_point_the_pointer_is_on() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 400, 200));
        let mut book = crate::finance::types::OrderBook::new();
        book.set_bids(alloc::vec![crate::finance::types::BookLevel::new(100.0, 5.0)]);
        book.set_asks(alloc::vec![crate::finance::types::BookLevel::new(100.0, 7.0)]);
        chart.set_book(book);

        let area = chart.plot_area();
        let (low, high) = chart.price_bounds();
        let fraction = (100.0 - low) / (high - low);
        let x = area.rect.x + (fraction * area.rect.width as f64).round() as i32;

        // The ask's cumulative total is the taller point, so a pointer near the top of the
        // pane is on it; the bid's point sits lower.
        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x, y: area.rect.y + 2 },
        });
        assert_eq!(chart.hovered_level(), Some(100.0));
        assert_eq!(chart.hovered_side, Some(DepthSide::Ask));
        assert_eq!(chart.hovered_point().map(|point| point.side), Some(DepthSide::Ask));
        assert_eq!(
            chart.hovered_point().map(|point| point.cumulative_quantity),
            Some(7.0),
            "the read-out reports the ask's total, not the bid's"
        );

        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x, y: area.bottom() - 2 },
        });
        assert_eq!(chart.hovered_side, Some(DepthSide::Bid));
        assert_eq!(chart.hovered_point().map(|point| point.cumulative_quantity), Some(5.0));
    }

    /// A press that creates the hover reports it on `level_hovered` too.
    ///
    /// Regression: the press path assigned `hovered_level` directly, so a click without a
    /// preceding move moved the mark while the hover signal stayed silent.
    #[test]
    fn a_press_without_a_move_emits_the_hover_it_creates() {
        let mut chart = DepthChart::new(Rect::new(0, 0, 400, 200));
        chart.set_book(fixtures::sample_book(4));
        let area = chart.plot_area();
        let x = area.rect.x + area.rect.width as i32 / 2;

        let hovered = std::sync::Arc::new(std::sync::Mutex::new(Vec::<f64>::new()));
        let sink = hovered.clone();
        chart.level_hovered.connect(move |level: std::sync::Arc<f64>| {
            sink.lock().unwrap().push(*level);
        });
        chart.handle_event(&rust_widgets::event::Event::MousePress {
            pos: rust_widgets::core::Point { x, y: area.rect.y + 40 },
            button: 1,
            modifiers: 0,
        });
        assert_eq!(chart.hovered_level(), hovered.lock().unwrap().last().copied());
        assert_eq!(hovered.lock().unwrap().len(), 1, "the press reported the hover once");
    }
}
