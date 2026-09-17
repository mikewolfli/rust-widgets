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

#[cfg(widgets_unstripped)]
pub mod candlestick_chart;
#[cfg(widgets_unstripped)]
pub mod depth_chart;
#[cfg(widgets_unstripped)]
pub mod indicator_chart;
#[cfg(widgets_unstripped)]
pub mod indicators;
#[cfg(widgets_unstripped)]
pub mod layout;
#[cfg(widgets_unstripped)]
pub mod order_book;
#[cfg(widgets_unstripped)]
pub mod quote_board;
#[cfg(widgets_unstripped)]
pub mod types;
#[cfg(widgets_unstripped)]
pub mod volume_chart;

#[cfg(widgets_unstripped)]
pub use candlestick_chart::{CandlestickChart, Overlay};
#[cfg(widgets_unstripped)]
pub use depth_chart::{DepthChart, DepthPoint, DepthSide};
#[cfg(widgets_unstripped)]
pub use indicator_chart::{IndicatorChart, IndicatorMode};
#[cfg(widgets_unstripped)]
pub use indicators::{
    align_left, atr, bollinger_bands, donchian_channel, ema, has_drawable_values, macd,
    money_flow_index, on_balance_volume, rsi, series_extent, sma, stochastic, trailing_extremes,
    vwap, wilder_smooth,
};
#[cfg(widgets_unstripped)]
pub use layout::{IndexAxis, PlotArea, PriceAxis};
#[cfg(widgets_unstripped)]
pub use order_book::{BookSide, OrderBookWidget, DEFAULT_BOOK_DEPTH};
#[cfg(widgets_unstripped)]
pub use quote_board::{QuoteBoard, QuoteColumn, QuoteSort};
#[cfg(widgets_unstripped)]
pub use types::{Bar, BookLevel, OrderBook, PriceLevelKind, PriceLine, PriceSeries, Quote};
#[cfg(widgets_unstripped)]
pub use volume_chart::{VolumeChart, VolumeColorMode};
