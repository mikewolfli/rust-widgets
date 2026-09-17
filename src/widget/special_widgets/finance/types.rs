// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The data model shared by the financial controls.
//!
//! # Why one model rather than a per-control shape
//!
//! A K-line pane, a volume pane and an order book all describe the same instrument at
//! the same moments, and they must agree about both. If each control owned its own
//! representation, a caller holding one OHLC series would have to convert it three ways,
//! and the three copies could disagree about which index is the newest.
//!
//! There is also a hard rendering constraint behind it: several panes are drawn in one
//! viewport and must share a **horizontal index range**, or an indicator overlay lands at
//! the wrong bar. That agreement needs the range to be one value in one place, not three
//! values that happen to be kept equal.
//!
//! # Why the bar is `f64` rather than a decimal type
//!
//! Prices arrive from a network feed as JSON numbers, which are already IEEE-754. Keeping
//! them as `f64` means no conversion at the boundary and no rounding policy to agree on.
//! The trade-off is real — `0.1 + 0.2` is not `0.3` — so anything that must be exact
//! (position accounting, order matching) belongs in the caller's ledger, not in a chart.
//! These controls display prices; they do not settle them.

use alloc::string::String;
use alloc::vec::Vec;

/// One OHLCV bar: the unit every financial chart is built from.
///
/// The five fields are the standard set. `open`, `high`, `low` and `close` are prices;
/// `volume` is in whatever unit the feed reports, since the control only ever needs it
/// for relative comparison — a volume bar's height and a VWAP's weight.
///
/// A bar with `high < low` is malformed but not rejected: the drawing code clamps rather
/// than panicking, because a chart that refuses one bad tick from a live feed is worse
/// than one that draws it oddly. `is_consistent` lets a caller audit a series before
/// trusting it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bar {
    /// Price at the start of the period.
    pub open: f64,
    /// Highest price traded during the period.
    pub high: f64,
    /// Lowest price traded during the period.
    pub low: f64,
    /// Price at the end of the period.
    pub close: f64,
    /// Traded quantity, in the feed's own unit.
    pub volume: f64,
}

impl Bar {
    /// Creates a bar from its five values.
    pub fn new(open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self { open, high, low, close, volume }
    }

    /// A bar whose open, high, low and close are all `price`, with the given volume.
    ///
    /// The common case for a synthetic or a tick-only series, where only one price is
    /// known per period: a doji with no range. Writing it out at each call site invites
    /// the slip of setting only some of the four.
    pub fn flat(price: f64, volume: f64) -> Self {
        Self { open: price, high: price, low: price, close: price, volume }
    }

    /// Whether the bar rose: the close is at or above the open.
    ///
    /// An unchanged bar reads as rising, which is the convention on every platform — a
    /// doji is drawn in the up colour rather than a third neutral colour, because a
    /// second colour for "unchanged" is noise on a chart.
    pub fn is_rising(&self) -> bool {
        self.close >= self.open
    }

    /// The bar's body height as an absolute price difference.
    ///
    /// Absolute because the direction is `is_rising`'s job: the drawing code needs the
    /// height and the colour separately, and a signed height would have to be un-signed
    /// at exactly the place a mistake would be invisible.
    pub fn body_height(&self) -> f64 {
        (self.close - self.open).abs()
    }

    /// The typical price: `(high + low + close) / 3`.
    ///
    /// The price VWAP and MFI weight by volume. Not the close, because the point of those
    /// indicators is to reflect where trade happened during the bar rather than where the
    /// bar happened to end.
    pub fn typical_price(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }

    /// The true range: the largest of the high/low span and the two gaps from the
    /// previous close.
    ///
    /// Takes the previous close as an argument rather than reading it from a series,
    /// because a single bar cannot know it. `None` is the first bar of a series and is
    /// also what a gap in the data produces; in both cases the fallback is the plain
    /// high/low span, which is the only part of the definition still knowable.
    pub fn true_range(&self, previous_close: Option<f64>) -> f64 {
        let span = self.high - self.low;
        let Some(previous) = previous_close.filter(|value| value.is_finite()) else {
            return span;
        };
        span.max((self.high - previous).abs()).max((self.low - previous).abs())
    }

    /// Whether every field is a finite number and the high/low bracket the open and close.
    ///
    /// `false` means the bar cannot be a real observation: a `NaN` price, or a high below
    /// the low. Exposed so a caller loading historical data can report which bars were
    /// rejected, rather than discovering it as a gap on screen.
    pub fn is_consistent(&self) -> bool {
        let finite = self.open.is_finite()
            && self.high.is_finite()
            && self.low.is_finite()
            && self.close.is_finite()
            && self.volume.is_finite();
        finite && self.high >= self.low
    }
}

/// A time series of [`Bar`]s plus the metadata a chart needs to label them.
///
/// # Why the label is a `String` and not a date type
///
/// Because a market series is not always dated. Intraday charts label bars by clock time,
/// daily charts by date, and some feeds label by session or by bar sequence number. A
/// `chrono::NaiveDateTime` would force every caller to have one, and this crate has no
/// date dependency; a label string accepts all three without a conversion, and the
/// control only ever draws it.
///
/// # Why this is not `Vec<Bar>` plus a separate `Vec<String>`
///
/// Index alignment is the one invariant the overlays depend on. Two vectors can fall out
/// of step — a push to one and not the other — and the failure is an off-by-one in the
/// label, which is invisible until someone misreads a price. One struct with one `push`
/// cannot drift.
#[derive(Debug, Clone, Default)]
pub struct PriceSeries {
    bars: Vec<Bar>,
    labels: Vec<String>,
}

impl PriceSeries {
    /// Creates an empty series.
    pub fn new() -> Self {
        Self { bars: Vec::new(), labels: Vec::new() }
    }

    /// Creates a series from bars with no labels.
    ///
    /// The common case for a programmatically generated series, where an axis label
    /// would be a bar index anyway — which the control already knows how to draw.
    pub fn from_bars(bars: Vec<Bar>) -> Self {
        Self { bars, labels: Vec::new() }
    }

    /// Appends one bar, keeping the label list aligned.
    ///
    /// A bar appended without a label leaves every existing label at its own index, so
    /// no label can end up attached to the wrong bar — the new bar simply has none.
    pub fn push(&mut self, bar: Bar) {
        self.bars.push(bar);
    }

    /// Appends one bar with an axis label.
    pub fn push_with_label(&mut self, bar: Bar, label: String) {
        self.bars.push(bar);
        self.labels.push(label);
    }

    /// Replaces the bars, discarding the labels.
    ///
    /// Labels are dropped rather than kept because their length would no longer be able
    /// to match, and a stale label is worse than no label: it names the wrong period.
    pub fn set_bars(&mut self, bars: Vec<Bar>) {
        self.bars = bars;
        self.labels.clear();
    }

    /// Replaces the labels.
    ///
    /// Ignored when the length disagrees with the bars, because a mismatched label list
    /// is exactly the off-by-one this type exists to prevent. A caller with a differently
    /// sized list has a bug worth not silently accepting.
    pub fn set_labels(&mut self, labels: Vec<String>) {
        if labels.len() == self.bars.len() {
            self.labels = labels;
        }
    }

    /// The bars.
    pub fn bars(&self) -> &[Bar] {
        &self.bars
    }

    /// The axis labels, empty when none were supplied.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// The label for bar `index`, if one was supplied.
    pub fn label_at(&self, index: usize) -> Option<&str> {
        self.labels.get(index).map(String::as_str)
    }

    /// How many bars the series holds.
    pub fn len(&self) -> usize {
        self.bars.len()
    }

    /// Whether the series holds no bars.
    pub fn is_empty(&self) -> bool {
        self.bars.is_empty()
    }

    /// The closes, which is what the overlay indicators are computed over.
    ///
    /// Returned as an owned vector rather than a view because an indicator needs
    /// `&[f64]` and the bars are a struct-of-arrays. The allocation is one per indicator
    /// call, not one per frame, because the caller caches the result.
    pub fn closes(&self) -> Vec<f64> {
        self.bars.iter().map(|bar| bar.close).collect()
    }

    /// The opens.
    pub fn opens(&self) -> Vec<f64> {
        self.bars.iter().map(|bar| bar.open).collect()
    }

    /// The highs.
    pub fn highs(&self) -> Vec<f64> {
        self.bars.iter().map(|bar| bar.high).collect()
    }

    /// The lows.
    pub fn lows(&self) -> Vec<f64> {
        self.bars.iter().map(|bar| bar.low).collect()
    }

    /// The volumes.
    pub fn volumes(&self) -> Vec<f64> {
        self.bars.iter().map(|bar| bar.volume).collect()
    }

    /// The lowest low and highest high across the series, for a price axis.
    ///
    /// An empty or all-malformed series returns `(NAN, NAN)`, which the drawing code
    /// reads as "nothing to draw" rather than as a zero-height axis.
    pub fn price_extent(&self) -> (f64, f64) {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for bar in &self.bars {
            if bar.low.is_finite() {
                low = low.min(bar.low);
            }
            if bar.high.is_finite() {
                high = high.max(bar.high);
            }
        }
        if low > high {
            (f64::NAN, f64::NAN)
        } else {
            (low, high)
        }
    }

    /// The largest volume in the series, or `0.0` when there is none.
    ///
    /// Zero rather than `NAN` because it is used as a divisor for bar heights: a caller
    /// that guards on `> 0.0` gets the "nothing to draw" path, while a `NAN` would have
    /// to be checked for separately at every use.
    pub fn max_volume(&self) -> f64 {
        self.bars
            .iter()
            .map(|bar| bar.volume)
            .filter(|value| value.is_finite())
            .fold(0.0_f64, f64::max)
    }
}

/// One side of an order book: a price and the quantity resting at it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BookLevel {
    /// The limit price of this level.
    pub price: f64,
    /// Total quantity resting at that price.
    pub quantity: f64,
}

impl BookLevel {
    /// Creates a level.
    pub fn new(price: f64, quantity: f64) -> Self {
        Self { price, quantity }
    }

    /// The notional value resting at this level: `price * quantity`.
    ///
    /// What a depth chart's area represents, and what "how much would it cost to clear
    /// this level" means. Zero when either field is unusable, so a caller summing notionals
    /// does not have to filter first.
    pub fn notional(&self) -> f64 {
        if self.price.is_finite() && self.quantity.is_finite() {
            self.price * self.quantity
        } else {
            0.0
        }
    }
}

/// A live order book: the bid and ask ladders around the spread.
///
/// # Ordering is part of the type, not a caller convention
///
/// Bids are stored best-first (descending price) and asks best-first (ascending price).
/// That is the order every exchange publishes and every trader reads in, and enforcing it
/// in [`Self::set_bids`] / [`Self::set_asks`] means the drawing code can index from zero
/// to get "the best bid" without first deciding what "best" means. A book stored in
/// arrival order would put the top-of-book decision in the renderer, where it would be
/// duplicated at each of the several places that need it.
#[derive(Debug, Clone, Default)]
pub struct OrderBook {
    bids: Vec<BookLevel>,
    asks: Vec<BookLevel>,
}

impl OrderBook {
    /// Creates an empty book.
    pub fn new() -> Self {
        Self { bids: Vec::new(), asks: Vec::new() }
    }

    /// Replaces the bids, sorting them best-first (highest price first).
    ///
    /// Sorting rather than trusting the caller's order: a feed that delivers levels out
    /// of order is normal, and a book whose top row is not the best bid is worse than
    /// useless — it is actively misleading about the spread.
    pub fn set_bids(&mut self, mut bids: Vec<BookLevel>) {
        bids.sort_by(|left, right| {
            right.price.partial_cmp(&left.price).unwrap_or(core::cmp::Ordering::Equal)
        });
        self.bids = bids;
    }

    /// Replaces the asks, sorting them best-first (lowest price first).
    pub fn set_asks(&mut self, mut asks: Vec<BookLevel>) {
        asks.sort_by(|left, right| {
            left.price.partial_cmp(&right.price).unwrap_or(core::cmp::Ordering::Equal)
        });
        self.asks = asks;
    }

    /// The bids, best first.
    pub fn bids(&self) -> &[BookLevel] {
        &self.bids
    }

    /// The asks, best first.
    pub fn asks(&self) -> &[BookLevel] {
        &self.asks
    }

    /// The best bid: the highest price anyone will buy at.
    pub fn best_bid(&self) -> Option<BookLevel> {
        self.bids.first().copied()
    }

    /// The best ask: the lowest price anyone will sell at.
    pub fn best_ask(&self) -> Option<BookLevel> {
        self.asks.first().copied()
    }

    /// The spread: `best_ask - best_bid`.
    ///
    /// `None` when either side is empty. A crossed book — a spread below zero, which
    /// happens when one side is stale — is reported as it is rather than clamped, because
    /// a negative spread is a real signal about the data that clamping would hide.
    pub fn spread(&self) -> Option<f64> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        Some(ask.price - bid.price)
    }

    /// The mid price: the midpoint of the spread.
    pub fn mid_price(&self) -> Option<f64> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        Some((bid.price + ask.price) / 2.0)
    }

    /// The cumulative quantity from the top of the book down to `depth` levels.
    ///
    /// The y-value of a depth chart's step function: how much is available within that
    /// many levels of the best price on that side.
    pub fn cumulative_quantity(&self, side_is_bid: bool, depth: usize) -> f64 {
        let levels = if side_is_bid { &self.bids } else { &self.asks };
        levels
            .iter()
            .take(depth)
            .map(|level| if level.quantity.is_finite() { level.quantity } else { 0.0 })
            .sum()
    }

    /// The largest quantity resting at any single level on either side.
    ///
    /// The scale a depth bar is drawn against, so the deepest level fills its row.
    pub fn max_level_quantity(&self) -> f64 {
        self.bids
            .iter()
            .chain(self.asks.iter())
            .map(|level| level.quantity)
            .filter(|value| value.is_finite())
            .fold(0.0_f64, f64::max)
    }
}

/// One instrument's row in a quote board.
#[derive(Debug, Clone, Default)]
pub struct Quote {
    /// The ticker symbol, drawn in the first column.
    pub symbol: String,
    /// The display name, drawn as secondary text.
    pub name: String,
    /// The last traded price.
    pub last: f64,
    /// The previous close, which the day's change is measured against.
    pub previous_close: f64,
    /// The highest price of the session.
    pub high: f64,
    /// The lowest price of the session.
    pub low: f64,
    /// Traded volume for the session.
    pub volume: f64,
}

impl Quote {
    /// Creates a quote from the fields a board actually displays.
    pub fn new(symbol: &str, last: f64, previous_close: f64) -> Self {
        Self {
            symbol: symbol.into(),
            name: String::new(),
            last,
            previous_close,
            high: last,
            low: last,
            volume: 0.0,
        }
    }

    /// The absolute change from the previous close.
    pub fn change(&self) -> f64 {
        if self.last.is_finite() && self.previous_close.is_finite() {
            self.last - self.previous_close
        } else {
            f64::NAN
        }
    }

    /// The percentage change from the previous close.
    ///
    /// `NAN` when the previous close is missing or zero — a percentage against zero is
    /// undefined, and reporting `0%` or `inf%` would both be wrong in a way a trader would
    /// act on.
    pub fn change_percent(&self) -> f64 {
        let change = self.change();
        if !change.is_finite() || !self.previous_close.is_finite() || self.previous_close == 0.0 {
            return f64::NAN;
        }
        change / self.previous_close * 100.0
    }

    /// Whether the quote is up on the day.
    ///
    /// An unchanged quote reads as up, matching [`Bar::is_rising`] and every trading
    /// platform's colour convention.
    pub fn is_up(&self) -> bool {
        let change = self.change();
        !change.is_finite() || change >= 0.0
    }
}

/// A price level drawn as a horizontal line, with the reason it matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceLevelKind {
    /// Support: a floor the price has repeatedly failed to break below.
    Support,
    /// Resistance: a ceiling the price has repeatedly failed to break above.
    Resistance,
    /// The previous session's close.
    PreviousClose,
}

impl PriceLevelKind {
    /// The factory spelling of this variant, matching the control's property contract.
    pub fn as_str(self) -> &'static str {
        match self {
            PriceLevelKind::Support => "support",
            PriceLevelKind::Resistance => "resistance",
            PriceLevelKind::PreviousClose => "previous_close",
        }
    }

    /// Parses a factory spelling into a variant.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "support" => PriceLevelKind::Support,
            "resistance" => PriceLevelKind::Resistance,
            "previous_close" => PriceLevelKind::PreviousClose,
            _ => return None,
        })
    }
}

/// A horizontal level marked on a price chart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceLine {
    /// The price the line sits at.
    pub price: f64,
    /// What the line represents, which selects its colour.
    pub kind: PriceLevelKind,
}

impl PriceLine {
    /// Creates a price line.
    pub fn new(price: f64, kind: PriceLevelKind) -> Self {
        Self { price, kind }
    }
}

/// Test fixtures shared by the financial control tests.
///
/// A synthetic series is built here rather than in each test because the controls all
/// need one of a known shape — deterministic, non-flat, with a gap big enough to be
/// visible — and six local copies would eventually differ.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    /// A deterministic series of `count` bars, rising on average with a repeating wave.
    ///
    /// Deliberately not random: a test that fails intermittently teaches nothing, and a
    /// shape that can be reasoned about is worth more here than realism. The wave makes
    /// highs and lows differ from the closes so the OHLC geometry is exercised, and the
    /// drift makes `is_rising` mostly true so both colours appear.
    pub fn sample_bars(count: usize) -> Vec<Bar> {
        (0..count)
            .map(|index| {
                let base = 100.0 + (index as f64) * 0.5 + (index as f64 * 0.4).sin() * 4.0;
                let open = base;
                let close = base + (index as f64 * 0.7).cos() * 1.5;
                let high = open.max(close) + 0.8;
                let low = open.min(close) - 0.8;
                Bar::new(open, high, low, close, 1000.0 + (index % 11) as f64 * 137.0)
            })
            .collect()
    }

    /// A [`PriceSeries`] of `count` labelled bars.
    pub fn sample_series(count: usize) -> PriceSeries {
        let mut series = PriceSeries::from_bars(sample_bars(count));
        let labels: Vec<String> = (0..count).map(|index| alloc::format!("D{index}")).collect();
        series.set_labels(labels);
        series
    }

    /// A book with `depth` levels on each side, symmetric around 100.0.
    pub fn sample_book(depth: usize) -> OrderBook {
        let mut book = OrderBook::new();
        let bids: Vec<BookLevel> = (0..depth)
            .map(|index| BookLevel::new(99.9 - index as f64 * 0.1, 100.0 * (index + 1) as f64))
            .collect();
        let asks: Vec<BookLevel> = (0..depth)
            .map(|index| BookLevel::new(100.1 + index as f64 * 0.1, 90.0 * (index + 1) as f64))
            .collect();
        book.set_bids(bids);
        book.set_asks(asks);
        book
    }

    /// A quote board of `count` symbols, alternating up and down.
    pub fn sample_quotes(count: usize) -> Vec<Quote> {
        (0..count)
            .map(|index| {
                let previous_close = 100.0 + index as f64;
                let direction = if index % 2 == 0 { 1.0 } else { -1.0 };
                let mut quote = Quote::new(
                    &alloc::format!("SYM{index}"),
                    previous_close + direction * 2.5,
                    previous_close,
                );
                quote.name = alloc::format!("Sample {index}");
                quote.high = previous_close + 3.0;
                quote.low = previous_close - 3.0;
                quote.volume = 1_000_000.0 + index as f64 * 1000.0;
                quote
            })
            .collect()
    }
}
