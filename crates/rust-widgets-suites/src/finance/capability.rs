// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability rows, property schemas and constructors of the finance suite.
//!
//! A capability is the designer's entire view of a control: the name it is created by, the
//! property contract with a type and an accessibility flag per row, and the published events with
//! their payload kinds. All of those are declared here, next to the constructors, so the
//! registration and the documentation cannot describe different controls.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::apply_control_label;

use rust_widgets::widget::capability::types::{
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema, PropertyValueKind as K,
    RegistrationError, WidgetCapability, WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};

use super::candlestick_chart::CandlestickChart;
use super::depth_chart::DepthChart;
use super::indicator_chart::IndicatorChart;
use super::order_book::OrderBookWidget;
use super::quote_board::{QuoteBoard, QUOTE_RECORD_SCHEMA};
use super::types::BOOK_LEVEL_RECORD_SCHEMA;
use super::volume_chart::VolumeChart;

/// `CandlestickChart`'s published properties.
pub const CANDLESTICK_CHART_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
    PropertySchema::new("series", K::String, true, true),
    // `readable: true`, because the control **answers** it: `CandlestickChart::get`
    // returns `self.overlays.len()`. It was declared `false`, and that flag is not
    // advisory — a non-readable schema turns into `UnsupportedOnWidget`, so a caller
    // could `set` the overlay count and then could not read it back. "Wrote it, cannot
    // read it" is the one-directional contract the schema exists to prevent.
    PropertySchema::new("overlay_count", K::UInt, true, true),
    PropertySchema::new("show_price_levels", K::Bool, true, true),
    PropertySchema::new("show_overlay_legend", K::Bool, true, true),
];

/// `VolumeChart`'s published properties.
pub const VOLUME_CHART_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
    PropertySchema::new("series", K::String, true, true),
    PropertySchema::enumerated("color_mode", true, true, &["direction", "uniform"]),
    PropertySchema::new("headroom", K::Float, true, true),
];

/// `DepthChart`'s published properties.
pub const DEPTH_CHART_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
    PropertySchema::new("depth", K::UInt, true, true),
    // RW-106: the depth curve is derived from the two ladders, so the ladders are what a design
    // document writes. Empty lists produce the empty pane the control already draws.
    PropertySchema::records("bids", true, true, &BOOK_LEVEL_RECORD_SCHEMA),
    PropertySchema::records("asks", true, true, &BOOK_LEVEL_RECORD_SCHEMA),
    // Declared `Color`, matching what `DepthChart::set` accepts: it destructures
    // `CapabilityValue::Color` and answers `TypeMismatch` for anything else, so a
    // `String` declaration would make the property unwritable through the ABI —
    // `write_property` validates against the schema and the control then refuses it.
    PropertySchema::new("bid_color", K::Color, true, true),
    PropertySchema::new("ask_color", K::Color, true, true),
];

/// `OrderBookWidget`'s published properties.
pub const ORDER_BOOK_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
    PropertySchema::new("depth", K::UInt, true, true),
    PropertySchema::new("decimals", K::UInt, true, true),
    // RW-106: the two ladders, writable as record lists (see `BOOK_LEVEL_RECORD_SCHEMA`).
    PropertySchema::records("bids", true, true, &BOOK_LEVEL_RECORD_SCHEMA),
    PropertySchema::records("asks", true, true, &BOOK_LEVEL_RECORD_SCHEMA),
    // Read-only: `OrderBookWidget::set` destructures the value and then refuses the
    // write. The spread band is a display decision the control owns, so there is
    // nothing for a caller to assign.
    PropertySchema::new("show_spread", K::Bool, true, false),
];

/// `QuoteBoard`'s published properties.
pub const QUOTE_BOARD_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
    PropertySchema::enumerated(
        "sort",
        true,
        true,
        &[
            "none",
            "symbol_ascending",
            "symbol_descending",
            "name_ascending",
            "name_descending",
            "last_ascending",
            "last_descending",
            "change_magnitude_ascending",
            "change_magnitude_descending",
            "high_ascending",
            "high_descending",
            "low_ascending",
            "low_descending",
            "volume_ascending",
            "volume_descending",
        ],
    ),
    PropertySchema::new("selected_index", K::UInt, true, true),
    // Read-only: `QuoteBoard::set` answers `ReadOnlyProperty` because the board's row
    // height is a layout decision the control owns.
    PropertySchema::new("row_height", K::UInt, true, false),
    PropertySchema::new("scroll_offset", K::UInt, true, true),
    // RW-79: the quote rows as typed records, written through `set_quotes`.
    PropertySchema::records("quotes", true, true, &QUOTE_RECORD_SCHEMA),
];

/// `IndicatorChart`'s published properties.
pub const INDICATOR_CHART_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
    PropertySchema::new("series", K::String, true, true),
    PropertySchema::enumerated(
        "mode",
        true,
        true,
        &["macd", "rsi", "stochastic", "money_flow_index", "atr", "on_balance_volume"],
    ),
    PropertySchema::new("period", K::UInt, true, true),
    PropertySchema::new("show_reference_levels", K::Bool, true, true),
];

/// `CandlestickChart`'s published events.
const CANDLESTICK_CHART_EVENTS: &[EventSchema] = &[
    EventSchema::scalar("bar_clicked", K::UInt),
    EventSchema::scalar("bar_hovered", K::UInt),
    EventSchema::scalar("bar_unhovered", K::UInt),
    // `(start, end)`: two indices in one payload, so the declared kind is how the
    // whole value travels and the shape says it is a pair.
    EventSchema { name: "view_changed", payload: Some(K::UInt), shape: Some(S::Tuple2) },
];

/// `VolumeChart`'s published events.
const VOLUME_CHART_EVENTS: &[EventSchema] = &[
    EventSchema::scalar("bar_clicked", K::UInt),
    EventSchema::scalar("bar_hovered", K::UInt),
    EventSchema { name: "view_changed", payload: Some(K::UInt), shape: Some(S::Tuple2) },
];

/// `DepthChart`'s published events.
const DEPTH_CHART_EVENTS: &[EventSchema] = &[
    EventSchema::scalar("level_clicked", K::Float),
    EventSchema::scalar("level_hovered", K::Float),
];

/// `OrderBookWidget`'s published events.
const ORDER_BOOK_EVENTS: &[EventSchema] = &[
    // `(side, level index)`: the side is a token and the index is a `UInt`, so the payload is
    // `Mixed` — there is no single-element kind to promise. The index is absolute into
    // `bids()`/`asks()`, not a price.
    EventSchema { name: "level_clicked", payload: Some(K::String), shape: Some(S::Mixed) },
    EventSchema { name: "level_hovered", payload: Some(K::String), shape: Some(S::Mixed) },
    EventSchema { name: "level_unhovered", payload: Some(K::String), shape: Some(S::Mixed) },
];

/// `QuoteBoard`'s published events.
const QUOTE_BOARD_EVENTS: &[EventSchema] = &[
    EventSchema::scalar("quote_clicked", K::String),
    EventSchema::scalar("quote_hovered", K::String),
    // The symbol of the row the pointer left: leaving the board, moving off a row,
    // scrolling the window, or a replacement list that no longer holds the hovered index.
    EventSchema::scalar("quote_unhovered", K::String),
    // `Option<String>`: a cleared selection carries no symbol, which the shape says
    // rather than the payload kind (the kind is the element's).
    EventSchema {
        name: "selection_changed",
        payload: Some(K::String),
        shape: Some(S::OptionalScalar),
    },
];

/// `IndicatorChart`'s published events.
const INDICATOR_CHART_EVENTS: &[EventSchema] = &[
    EventSchema::scalar("bar_clicked", K::UInt),
    EventSchema::scalar("bar_hovered", K::UInt),
    EventSchema { name: "view_changed", payload: Some(K::UInt), shape: Some(S::Tuple2) },
];

/// Creates an empty `CandlestickChart`. `text` is applied as the control's label.
pub fn create_candlestick_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(CandlestickChart::new(geometry)), text)
}

/// Creates an empty `VolumeChart`. `text` is applied as the control's label.
pub fn create_volume_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(VolumeChart::new(geometry)), text)
}

/// Creates an empty `DepthChart`. `text` is applied as the control's label.
pub fn create_depth_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(DepthChart::new(geometry)), text)
}

/// Creates an empty `OrderBookWidget`. `text` is applied as the control's label.
pub fn create_order_book(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(OrderBookWidget::new(geometry)), text)
}

/// Creates an empty `QuoteBoard`. `text` is applied as the control's label.
pub fn create_quote_board(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(QuoteBoard::new(geometry)), text)
}

/// Creates an empty `IndicatorChart`. `text` is applied as the control's label.
pub fn create_indicator_chart(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(IndicatorChart::new(geometry)), text)
}

/// The `candlestick_chart` capability.
pub fn candlestick_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CandlestickChart,
        canonical_name: "candlestick_chart",
        aliases: &["candlestick", "kline", "k_line", "k_line_chart", "ohlc_chart"],
        properties: CANDLESTICK_CHART_PROPERTIES,
        events: CANDLESTICK_CHART_EVENTS,
        commands: &[
            "set_series",
            "set_overlay_count",
            "set_show_price_levels",
            "set_show_overlay_legend",
            "add_overlay",
        ],
    }
}

/// The `volume_chart` capability.
pub fn volume_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VolumeChart,
        canonical_name: "volume_chart",
        aliases: &["volume", "volume_histogram", "volume_bars"],
        properties: VOLUME_CHART_PROPERTIES,
        events: VOLUME_CHART_EVENTS,
        // RW-106: `add_overlay` is only implemented by `CandlestickChart`; naming it here
        // would promise an action that cannot be obtained (principle #18).
        commands: &["set_series", "set_color_mode", "set_headroom"],
    }
}

/// The `depth_chart` capability.
pub fn depth_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DepthChart,
        canonical_name: "depth_chart",
        aliases: &["market_depth", "depth_graph", "liquidity_chart"],
        properties: DEPTH_CHART_PROPERTIES,
        events: DEPTH_CHART_EVENTS,
        // RW-106: the two ladder setters carry their values through the property route.
        commands: &["set_bids", "set_asks"],
    }
}

/// The `order_book` capability.
pub fn order_book_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::OrderBook,
        canonical_name: "order_book",
        aliases: &["book_ladder", "market_depth_ladder"],
        properties: ORDER_BOOK_PROPERTIES,
        events: ORDER_BOOK_EVENTS,
        // RW-106: the two ladder setters carry their values through the property route, exactly
        // as every other data-bearing control publishes its `set_*` names.
        commands: &["set_bids", "set_asks", "set_depth", "set_decimals"],
    }
}

/// The `quote_board` capability.
pub fn quote_board_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QuoteBoard,
        canonical_name: "quote_board",
        aliases: &["quotes", "watchlist", "quote_table", "market_watch"],
        properties: QUOTE_BOARD_PROPERTIES,
        events: QUOTE_BOARD_EVENTS,
        // RW-106: `set_quotes` is the full-list write RW-79's property already carries. An
        // incremental add/remove needs a composite payload, which the command vocabulary cannot
        // express yet; the property route is the whole data surface this build can publish.
        commands: &["set_quotes"],
    }
}

/// The `indicator_chart` capability.
pub fn indicator_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::IndicatorChart,
        canonical_name: "indicator_chart",
        aliases: &["indicator", "technical_indicator", "oscillator", "macd_chart"],
        properties: INDICATOR_CHART_PROPERTIES,
        events: INDICATOR_CHART_EVENTS,
        commands: &["set_series", "set_mode", "set_period", "set_show_reference_levels"],
    }
}

/// A registered control: its capability row, its constructor, and its defaults provider.
type FinanceRow = (WidgetCapability, WidgetCtor, fn(&str) -> Option<CapabilityValue>);

/// Registers the finance suite in `factory`.
///
/// Returns the first name that could not be claimed. A factory that already answers one of these
/// names (a host that installed twice, or a build that ships a control of the same name) is
/// reported rather than silently shadowed.
///
/// Each row also registers the kind's declared defaults, so the standard
/// `default_property_value` a host or designer calls answers for these kinds too.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    let rows: [FinanceRow; 6] = [
        (candlestick_chart_capability(), create_candlestick_chart, candlestick_defaults),
        (volume_chart_capability(), create_volume_chart, volume_defaults),
        (depth_chart_capability(), create_depth_chart, depth_defaults),
        (order_book_capability(), create_order_book, order_book_defaults),
        (quote_board_capability(), create_quote_board, quote_board_defaults),
        (indicator_chart_capability(), create_indicator_chart, indicator_defaults),
    ];
    for (capability, ctor, defaults) in rows {
        let name = capability.canonical_name;
        factory.try_register(capability, ctor)?;
        let _ = factory.register_property_defaults(name, defaults);
    }
    Ok(())
}

/// The per-kind defaults providers, in the shape the factory calls.
fn candlestick_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("candlestick_chart", property)
}

fn volume_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("volume_chart", property)
}

fn depth_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("depth_chart", property)
}

fn order_book_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("order_book", property)
}

fn quote_board_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("quote_board", property)
}

fn indicator_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("indicator_chart", property)
}

/// A property's declared default, for the capability manifest.
///
/// Mirrors what the controls' constructors set, so a dropped binding resets to the value
/// a fresh control would have had.
pub fn default_property_value(kind: &str, name: &str) -> Option<CapabilityValue> {
    match (kind, name) {
        ("candlestick_chart", "series") => Some(CapabilityValue::String(String::new())),
        ("candlestick_chart", "overlay_count") => Some(CapabilityValue::UInt(0)),
        ("candlestick_chart", "show_price_levels") => Some(CapabilityValue::Bool(true)),
        ("candlestick_chart", "show_overlay_legend") => Some(CapabilityValue::Bool(true)),
        ("volume_chart", "series") => Some(CapabilityValue::String(String::new())),
        ("volume_chart", "color_mode") => Some(CapabilityValue::String("direction".to_string())),
        ("volume_chart", "headroom") => Some(CapabilityValue::Float(0.92)),
        ("depth_chart", "depth") => Some(CapabilityValue::UInt(0)),
        // RW-106: a fresh pane has neither ladder; the empty list of the declared schema is
        // what a dropped binding resets to.
        ("depth_chart", "bids" | "asks") => Some(CapabilityValue::Records(
            rust_widgets::widget::capability::RecordList::new(&BOOK_LEVEL_RECORD_SCHEMA),
        )),
        ("depth_chart", "bid_color") => {
            Some(CapabilityValue::Color(rust_widgets::core::Color::rgb(38, 166, 91)))
        }
        ("depth_chart", "ask_color") => {
            Some(CapabilityValue::Color(rust_widgets::core::Color::rgb(220, 68, 70)))
        }
        ("order_book", "depth") => Some(CapabilityValue::UInt(5)),
        ("order_book", "decimals") => Some(CapabilityValue::Null),
        ("order_book", "show_spread") => Some(CapabilityValue::Bool(true)),
        ("order_book", "bids" | "asks") => Some(CapabilityValue::Records(
            rust_widgets::widget::capability::RecordList::new(&BOOK_LEVEL_RECORD_SCHEMA),
        )),
        ("quote_board", "sort") => Some(CapabilityValue::String("none".to_string())),
        ("quote_board", "selected_index") => Some(CapabilityValue::Null),
        ("quote_board", "row_height") => Some(CapabilityValue::UInt(22)),
        ("quote_board", "scroll_offset") => Some(CapabilityValue::UInt(0)),
        // RW-79: a fresh board has no rows; the empty record list is what a dropped
        // binding resets to, and it is the value the declared schema accepts.
        ("quote_board", "quotes") => Some(CapabilityValue::Records(
            rust_widgets::widget::capability::RecordList::new(&QUOTE_RECORD_SCHEMA),
        )),
        ("indicator_chart", "series") => Some(CapabilityValue::String(String::new())),
        ("indicator_chart", "mode") => Some(CapabilityValue::String("macd".to_string())),
        ("indicator_chart", "period") => Some(CapabilityValue::UInt(14)),
        ("indicator_chart", "show_reference_levels") => Some(CapabilityValue::Bool(true)),
        _ => None,
    }
}

/// Every property name the finance suite publishes, for a quick completeness check.
pub fn published_property_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "candlestick_chart",
            CANDLESTICK_CHART_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        ("volume_chart", VOLUME_CHART_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("depth_chart", DEPTH_CHART_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("order_book", ORDER_BOOK_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("quote_board", QUOTE_BOARD_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("indicator_chart", INDICATOR_CHART_PROPERTIES.iter().map(|schema| schema.name).collect()),
    ]
}

/// Every event name the finance suite publishes, for a quick completeness check.
pub fn published_event_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("candlestick_chart", CANDLESTICK_CHART_EVENTS.iter().map(|schema| schema.name).collect()),
        ("volume_chart", VOLUME_CHART_EVENTS.iter().map(|schema| schema.name).collect()),
        ("depth_chart", DEPTH_CHART_EVENTS.iter().map(|schema| schema.name).collect()),
        ("order_book", ORDER_BOOK_EVENTS.iter().map(|schema| schema.name).collect()),
        ("quote_board", QUOTE_BOARD_EVENTS.iter().map(|schema| schema.name).collect()),
        ("indicator_chart", INDICATOR_CHART_EVENTS.iter().map(|schema| schema.name).collect()),
    ]
}
