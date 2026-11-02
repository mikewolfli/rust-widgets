// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Financial and market-data controls.
//!
//! This module holds the instrument-trading widget family: price charts, volume
//! panes, order books, quote boards and the technical indicators they overlay.
//!
//! # Why these are one module rather than scattered kinds
//!
//! The controls here share a data model — a series of bars with open/high/low/close
//! and volume — and a drawing concern: several panes must agree on a horizontal
//! index range and a price extent, or an overlay lands at the wrong bar. Splitting
//! them across the existing categories would put that agreement in no module at all.
//!
//! # Where the arithmetic lives
//!
//! [`indicators`] holds the technical analysis as pure functions over `&[f64]`.
//! It is deliberately not a set of controls: a moving average has no geometry, no
//! state and no platform call, so making it a widget would buy nothing and cost
//! testability (principle #24).
//!
//! # Why `indicators` is compiled more widely than the controls
//!
//! It has no dependencies at all — not `BaseWidget`, not the runtime, not a backend —
//! so gating it on the full widget set would be a gate with no purpose. A caller on a
//! reduced profile that computes an indicator and draws it through its own path should
//! not have to reimplement the arithmetic to do so.

pub mod candlestick_chart;
mod capability;
pub mod depth_chart;
pub mod indicator_chart;
pub mod indicators;
pub mod layout;
pub mod order_book;
pub mod quote_board;
pub mod types;
pub mod volume_chart;

pub use candlestick_chart::{CandlestickChart, Overlay};
pub use capability::{
    candlestick_chart_capability, create_candlestick_chart, create_depth_chart,
    create_indicator_chart, create_order_book, create_quote_board, create_volume_chart,
    default_property_value, depth_chart_capability, indicator_chart_capability, install,
    order_book_capability, published_event_names, published_property_names, quote_board_capability,
    volume_chart_capability, CANDLESTICK_CHART_PROPERTIES, DEPTH_CHART_PROPERTIES,
    INDICATOR_CHART_PROPERTIES, ORDER_BOOK_PROPERTIES, QUOTE_BOARD_PROPERTIES,
    VOLUME_CHART_PROPERTIES,
};
pub use depth_chart::{DepthChart, DepthPoint, DepthSide};
pub use indicator_chart::{IndicatorChart, IndicatorMode};
pub use indicators::{
    align_left, atr, bollinger_bands, donchian_channel, ema, has_drawable_values, macd,
    money_flow_index, on_balance_volume, rsi, series_extent, sma, stochastic, trailing_extremes,
    vwap, wilder_smooth,
};
pub use layout::{IndexAxis, PlotArea, PriceAxis};
pub use order_book::{BookSide, OrderBookWidget, DEFAULT_BOOK_DEPTH};
pub use quote_board::{QuoteBoard, QuoteColumn, QuoteSort, QUOTE_RECORD_SCHEMA};
pub use types::{Bar, BookLevel, OrderBook, PriceLevelKind, PriceLine, PriceSeries, Quote};
pub use volume_chart::{VolumeChart, VolumeColorMode};

#[cfg(test)]
mod tests_section210;

/// Copies the K-line pane's horizontal view into its stacked siblings.
///
/// The four panes are separate controls, so agreeing on a horizontal range is a host
/// responsibility; this helper is that responsibility spelled once. The range is copied,
/// not shared, because each pane owns its own `view_changed` stream and a host may still
/// drive one pane independently.
pub fn sync_view_range(
    candle: &candlestick_chart::CandlestickChart,
    volume: &mut volume_chart::VolumeChart,
    indicator: &mut indicator_chart::IndicatorChart,
) {
    let range = candle.view_range();
    volume.set_view_range(range);
    indicator.set_view_range(range);
}
