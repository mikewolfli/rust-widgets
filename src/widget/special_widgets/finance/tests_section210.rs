// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the §2.10 finance changes: order-book scrolling and incremental levels,
//! quote-board filtering and the sort cycle, overlay visibility, streaming bar updates,
//! wheel zoom, tooltip summaries and the `O(n)` indicator windows.

#![cfg(test)]

use super::candlestick_chart::{CandlestickChart, Overlay};
use super::indicator_chart::IndicatorChart;
use super::indicators;
use super::order_book::{BookSide, OrderBookWidget};
use super::quote_board::{QuoteBoard, QuoteSort};
use super::types::{fixtures, Bar, BookLevel, OrderBook, PriceSeries, Quote};
use super::volume_chart::VolumeChart;
use crate::core::{Point, Rect};
use crate::event::{Event, EventHandler};
use crate::widget::view_widgets::filter_expr::{FilterCondition, FilterOperator};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

fn rect() -> Rect {
    Rect::new(0, 0, 400, 300)
}

/// Compares two overlay value lists element-wise, treating `NAN` as equal to `NAN` and
/// allowing the tiny difference a rolling sum introduces against a re-sum.
fn assert_close(left: Option<Vec<&[f64]>>, right: Option<Vec<&[f64]>>) {
    let left = left.expect("left overlay");
    let right = right.expect("right overlay");
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right.iter()) {
        assert_eq!(left.len(), right.len());
        for (a, b) in left.iter().zip(right.iter()) {
            match (a.is_finite(), b.is_finite()) {
                (false, false) => {}
                (true, true) => assert!((a - b).abs() < 1e-9, "{a} vs {b}"),
                _ => panic!("finite mismatch: {a} vs {b}"),
            }
        }
    }
}

fn series_with(count: usize) -> PriceSeries {
    let mut series = PriceSeries::new();
    for index in 0..count {
        let base = 100.0 + index as f64;
        series.push(Bar::new(base, base + 1.0, base - 1.0, base + 0.5, 1_000.0 + index as f64));
    }
    series
}

// ── FIN-06: order-book scrolling ───────────────────────────────────────────

fn deep_book() -> OrderBook {
    let mut book = OrderBook::new();
    book.set_bids((0..12).map(|index| BookLevel::new(100.0 - index as f64, 10.0)).collect());
    book.set_asks((0..12).map(|index| BookLevel::new(101.0 + index as f64, 10.0)).collect());
    book
}

/// Scrolling exposes deeper levels and the reported indices stay absolute.
#[test]
fn scrolling_the_ladder_reports_absolute_level_indices() {
    let mut widget = OrderBookWidget::new(rect());
    widget.set_depth(5);
    widget.set_book(deep_book());
    assert_eq!(widget.scroll_offset(), 0);
    widget.set_scroll_offset(3);
    assert_eq!(widget.scroll_offset(), 3);
    // The window shows levels 3..8; `levels` is that window.
    let bids = widget.levels(BookSide::Bid);
    assert_eq!(bids.len(), 5);
    assert_eq!(bids[0], BookLevel::new(97.0, 10.0));
    // A click on the row touching the spread rule names the best visible bid.
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = seen.clone();
    widget.level_clicked.connect(move |row| *sink.lock().unwrap() = Some(*row));
    let row_height = (rect().height as i32 / (5 * 2)).max(1);
    let upper_rows = 5;
    let y = row_height * (upper_rows - 1) + row_height / 2;
    widget.handle_event(&Event::MousePress { pos: Point::new(20, y), button: 1, modifiers: 0 });
    assert_eq!(*seen.lock().unwrap(), Some((BookSide::Ask, 3)));
    // Scrolling cannot go past the deepest window.
    widget.set_scroll_offset(99);
    assert_eq!(widget.scroll_offset(), 7);
}

/// The wheel scrolls the ladder a level at a time.
#[test]
fn the_wheel_scrolls_the_ladder() {
    let mut widget = OrderBookWidget::new(rect());
    widget.set_depth(5);
    widget.set_book(deep_book());
    widget.handle_event(&Event::Wheel { delta: Point::new(0, 10), modifiers: 0 });
    assert_eq!(widget.scroll_offset(), 1);
    widget.handle_event(&Event::Wheel { delta: Point::new(0, -10), modifiers: 0 });
    assert_eq!(widget.scroll_offset(), 0);
}

// ── J-P13: incremental book levels ────────────────────────────────────────

/// Insert / update / remove keep the ladder sorted and the best price correct.
#[test]
fn applying_one_level_matches_a_batch_replace() {
    let mut book = OrderBook::new();
    book.set_bids(vec![BookLevel::new(100.0, 1.0), BookLevel::new(99.0, 2.0)]);
    assert!(book.apply_bid_level(99.5, 3.0));
    assert_eq!(book.bids()[1], BookLevel::new(99.5, 3.0));
    assert!(book.apply_bid_level(99.0, 5.0));
    assert_eq!(book.bids()[2], BookLevel::new(99.0, 5.0));
    assert!(book.apply_bid_level(100.0, 0.0), "a zero quantity removes the level");
    assert_eq!(book.best_bid(), Some(BookLevel::new(99.5, 3.0)));
    assert!(!book.apply_bid_level(f64::NAN, 1.0), "a non-finite price is refused");

    let mut asks = OrderBook::new();
    asks.set_asks(vec![BookLevel::new(101.0, 1.0), BookLevel::new(102.0, 1.0)]);
    assert!(asks.apply_ask_level(101.5, 4.0));
    assert_eq!(asks.asks()[1], BookLevel::new(101.5, 4.0));
    assert_eq!(asks.best_ask(), Some(BookLevel::new(101.0, 1.0)));
}

// ── FIN-07 / FIN-08: quote board sort cycle and filtering ─────────────────

fn board() -> QuoteBoard {
    let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
    let mut beta = Quote::new("B", 20.0, 10.0);
    beta.name = String::from("Beta");
    let mut alpha = Quote::new("A", 10.0, 10.0);
    alpha.name = String::from("Alpha");
    let mut gamma = Quote::new("C", 30.0, 10.0);
    gamma.name = String::from("Gamma");
    board.set_quotes(vec![beta, alpha, gamma]);
    board
}

/// Filtering keeps the matching rows, and clearing restores the caller's order.
#[test]
fn filtering_selects_rows_and_clearing_restores_them() {
    let mut board = board();
    // Column 2 is `Last` in the fixed filter column space.
    board.set_filter(&[FilterCondition::new(2, FilterOperator::GreaterOrEqual, "20")]);
    let symbols: Vec<&str> = board.quotes().iter().map(|quote| quote.symbol.as_str()).collect();
    assert_eq!(symbols, vec!["B", "C"]);
    board.clear_filter();
    let symbols: Vec<&str> = board.quotes().iter().map(|quote| quote.symbol.as_str()).collect();
    assert_eq!(symbols, vec!["B", "A", "C"]);
}

/// Filtering and sorting compose: filtering happens before the sort.
#[test]
fn filtering_and_sorting_compose() {
    let mut board = board();
    // Names all contain "a", so the filter keeps all three and the sort decides the order.
    board.set_filter(&[FilterCondition::new(1, FilterOperator::Contains, "a")]);
    board.set_sort(QuoteSort::LastDescending);
    let symbols: Vec<&str> = board.quotes().iter().map(|quote| quote.symbol.as_str()).collect();
    assert_eq!(symbols, vec!["C", "B", "A"]);
}

/// A header click cycles preferred → opposite → caller order.
#[test]
fn clicking_a_header_cycles_ascending_descending_and_none() {
    let mut board = board();
    board.set_sortable_headers(true);
    let _ = crate::widget::svg::render_to_svg(&mut board);
    // The symbol column is the first one, so a click near the left edge is on it.
    let header_y = 10;
    let press = Event::MousePress { pos: Point::new(20, header_y), button: 1, modifiers: 0 };
    board.handle_event(&press);
    assert_eq!(board.sort(), QuoteSort::SymbolAscending);
    board.handle_event(&press);
    assert_eq!(board.sort(), QuoteSort::SymbolDescending);
    board.handle_event(&press);
    assert_eq!(board.sort(), QuoteSort::None);
}

/// The property spelling round-trips and still accepts the old tokens.
#[test]
fn sort_names_round_trip_and_accept_legacy_tokens() {
    for sort in [
        QuoteSort::None,
        QuoteSort::SymbolAscending,
        QuoteSort::SymbolDescending,
        QuoteSort::LastAscending,
        QuoteSort::LastDescending,
        QuoteSort::ChangeMagnitudeAscending,
        QuoteSort::ChangeMagnitudeDescending,
    ] {
        assert_eq!(QuoteSort::from_name(sort.as_str()), Some(sort));
    }
    assert_eq!(QuoteSort::from_name("symbol"), Some(QuoteSort::SymbolAscending));
    assert_eq!(
        QuoteSort::from_name("change_magnitude"),
        Some(QuoteSort::ChangeMagnitudeDescending)
    );
}

// ── FIN-04: overlay visibility and removal ────────────────────────────────

/// Hiding an overlay keeps it in the list, removes it from the axis and re-adds it.
#[test]
fn hiding_an_overlay_changes_the_extent_and_can_be_undone() {
    let mut chart = CandlestickChart::new(rect());
    chart.set_series(series_with(10));
    chart.add_overlay(Overlay::moving_average(2));
    assert!(chart.overlay_visible(0));
    assert!(chart.set_overlay_visible(0, false));
    assert!(!chart.overlay_visible(0));
    assert_eq!(chart.overlays().len(), 1, "hiding does not remove the overlay");
    assert!(chart.set_overlay_visible(0, true));
    assert!(chart.overlay_visible(0));
}

/// Removing an overlay drops exactly that entry and its visibility flag.
#[test]
fn removing_an_overlay_drops_it_and_keeps_the_others() {
    let mut chart = CandlestickChart::new(rect());
    chart.set_series(series_with(10));
    chart.add_overlay(Overlay::moving_average(2));
    chart.add_overlay(Overlay::exponential_moving_average(3));
    assert!(chart.remove_overlay(0));
    assert_eq!(chart.overlays().len(), 1);
    assert_eq!(chart.overlays()[0], Overlay::exponential_moving_average(3));
    assert!(chart.overlay_visible(0));
    assert!(!chart.remove_overlay(9));
    let legend = chart.overlay_legend_entries();
    assert_eq!(legend.len(), 1);
    assert_eq!(legend[0].0, "exponential_moving_average");
}

// ── J-P16: streaming bar updates ──────────────────────────────────────────

/// Updating the last bar matches a full overlay rebuild.
#[test]
fn updating_the_last_bar_matches_a_full_recompute() {
    let series = series_with(20);
    let mut chart = CandlestickChart::new(rect());
    chart.set_series(series.clone());
    for overlay in [
        Overlay::moving_average(5),
        Overlay::exponential_moving_average(4),
        Overlay::bollinger_bands(5, 2.0),
        Overlay::vwap(4),
        Overlay::donchian_channel(5),
    ] {
        chart.add_overlay(overlay);
    }
    let mut updated = series.clone();
    let last = updated.len() - 1;
    let mut bar = updated.bars()[last];
    bar.close = 140.0;
    bar.high = 150.0;
    updated.set_bar(last, bar);
    assert!(chart.update_last_bar(bar));

    let mut fresh = CandlestickChart::new(rect());
    fresh.set_series(updated);
    for overlay in chart.overlays() {
        fresh.add_overlay(*overlay);
    }
    for index in 0..chart.overlays().len() {
        assert_close(chart.overlay_values(index), fresh.overlay_values(index));
    }
}

/// Appending a bar matches a full overlay rebuild.
#[test]
fn appending_a_bar_matches_a_full_recompute() {
    let series = series_with(20);
    let mut chart = CandlestickChart::new(rect());
    chart.set_series(series.clone());
    chart.add_overlay(Overlay::moving_average(5));
    chart.add_overlay(Overlay::donchian_channel(4));
    let extra = Bar::new(130.0, 132.0, 128.0, 131.0, 5_000.0);
    chart.append_bar(extra);

    let mut updated = series;
    updated.push(extra);
    let mut fresh = CandlestickChart::new(rect());
    fresh.set_series(updated);
    for overlay in chart.overlays() {
        fresh.add_overlay(*overlay);
    }
    for index in 0..chart.overlays().len() {
        assert_close(chart.overlay_values(index), fresh.overlay_values(index));
    }
}

// ── FIN-02 / FIN-03: wheel zoom ──────────────────────────────────────────

/// The volume pane zooms with the wheel and reports the new range.
#[test]
fn the_volume_pane_zooms_with_the_wheel() {
    let mut volume = VolumeChart::new(rect());
    volume.set_series(series_with(40));
    assert_eq!(volume.view_range(), None);
    let centre = Point::new(rect().width as i32 / 2, rect().height as i32 / 2);
    volume.handle_event(&Event::MouseMove { pos: centre });
    volume.handle_event(&Event::Wheel { delta: Point::new(0, -10), modifiers: 0 });
    let (start, end) = volume.view_range().expect("zoomed in");
    assert!(end - start < 40, "the span shrank");
    assert!(start <= end);
}

/// The indicator pane zooms with the wheel too.
#[test]
fn the_indicator_pane_zooms_with_the_wheel() {
    let mut indicator = IndicatorChart::new(rect());
    indicator.set_series(series_with(40));
    let centre = Point::new(rect().width as i32 / 2, rect().height as i32 / 2);
    indicator.handle_event(&Event::MouseMove { pos: centre });
    indicator.handle_event(&Event::Wheel { delta: Point::new(0, -10), modifiers: 0 });
    let (start, end) = indicator.view_range().expect("zoomed in");
    assert!(end - start < 40);
}

// ── COMP-J02: hover summaries ────────────────────────────────────────────

/// Each chart produces a tooltip body from the hover it already reports.
#[test]
fn charts_expose_hover_summaries() {
    let mut candle = CandlestickChart::new(rect());
    candle.set_series(series_with(10));
    candle.handle_event(&Event::MouseMove { pos: Point::new(200, 100) });
    let summary = candle.hovered_summary();
    assert!(!summary.is_empty());
    assert!(summary.iter().any(|line| line.text.starts_with('O')));

    let mut volume = VolumeChart::new(rect());
    volume.set_series(series_with(10));
    volume.handle_event(&Event::MouseMove { pos: Point::new(200, 100) });
    assert!(!volume.hovered_summary().is_empty());

    let mut depth = super::depth_chart::DepthChart::new(rect());
    depth.set_book(fixtures::sample_book(6));
    depth.handle_event(&Event::MouseMove { pos: Point::new(200, 100) });
    let summary = depth.hovered_summary();
    assert!(!summary.is_empty(), "a hovered depth curve reports its level");

    let mut ladder = OrderBookWidget::new(rect());
    ladder.set_book(fixtures::sample_book(6));
    ladder.handle_event(&Event::MouseMove { pos: Point::new(20, 10) });
    assert!(!ladder.hovered_summary().is_empty());
}

// ── J-P18: linear indicator windows ──────────────────────────────────────

/// The rolling windows agree with a naive window scan, including through a gap.
#[test]
fn rolling_indicators_match_a_naive_scan() {
    let closes: Vec<f64> = (0..60).map(|index| 50.0 + (index as f64 * 0.7).sin() * 5.0).collect();
    let highs: Vec<f64> = closes.iter().map(|value| value + 1.0).collect();
    let lows: Vec<f64> = closes.iter().map(|value| value - 1.0).collect();
    let volumes: Vec<f64> = (0..60).map(|index| 100.0 + index as f64).collect();
    let period = 7;

    let vwap = indicators::vwap(&highs, &lows, &closes, &volumes, period);
    for (index, value) in vwap.iter().enumerate().skip(period - 1) {
        let mut weighted = 0.0;
        let mut total = 0.0;
        for offset in index + 1 - period..=index {
            let typical = (highs[offset] + lows[offset] + closes[offset]) / 3.0;
            weighted += typical * volumes[offset];
            total += volumes[offset];
        }
        assert!((*value - weighted / total).abs() < 1e-9, "vwap at {index}");
    }

    let (stochastic, _) = indicators::stochastic(&highs, &lows, &closes, period, 1, 3);
    for index in period - 1..closes.len() {
        let highest =
            highs[index + 1 - period..=index].iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lowest = lows[index + 1 - period..=index].iter().copied().fold(f64::INFINITY, f64::min);
        let expected = (closes[index] - lowest) / (highest - lowest) * 100.0;
        assert!((stochastic[index] - expected).abs() < 1e-9, "stochastic at {index}");
    }

    let (lower, middle, upper) = indicators::bollinger_bands(&closes, period, 2.0);
    for index in period - 1..closes.len() {
        let window = &closes[index + 1 - period..=index];
        let mean: f64 = window.iter().sum::<f64>() / period as f64;
        let variance: f64 =
            window.iter().map(|value| (value - mean) * (value - mean)).sum::<f64>() / period as f64;
        let deviation = variance.sqrt();
        assert!((middle[index] - mean).abs() < 1e-9);
        assert!((upper[index] - (mean + 2.0 * deviation)).abs() < 1e-6);
        assert!((lower[index] - (mean - 2.0 * deviation)).abs() < 1e-6);
    }
}

/// A gap in the middle of the series blanks its windows and nothing after it.
#[test]
fn a_gap_only_blanks_its_own_windows() {
    let mut closes: Vec<f64> = (0..20).map(|index| 10.0 + index as f64).collect();
    let highs: Vec<f64> = closes.iter().map(|value| value + 1.0).collect();
    let lows: Vec<f64> = closes.iter().map(|value| value - 1.0).collect();
    let volumes: Vec<f64> = vec![10.0; 20];
    closes[10] = f64::NAN;
    let period = 4;
    let vwap = indicators::vwap(&highs, &lows, &closes, &volumes, period);
    for (index, value) in vwap.iter().enumerate().skip(10).take(period) {
        assert!(!value.is_finite(), "window {index} contains the gap");
    }
    assert!(vwap[10 + period].is_finite(), "the next window is clean again");
}
