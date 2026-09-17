# Financial & Market-Data Controls

rust-widgets ships a family of instrument-trading controls: a K-line pane, a volume
histogram, a depth curve, an order-book ladder, a quote board and an indicator pane,
plus the technical-analysis arithmetic they all run on.

They are **self-drawn and platform-independent**, like every other control in this
crate. There is no market-data dependency: you bring the bars, the control draws them.

---

## 1. The data model

Everything in the family is built from one series type.

### `Bar`

One OHLCV bar — the unit every financial chart is made of:

```rust
use rust_widgets::widget::special_widgets::finance::Bar;

// open, high, low, close, volume
let bar = Bar::new(100.0, 103.5, 99.2, 102.8, 1_250_000.0);
assert!(bar.is_rising());            // close >= open
assert_eq!(bar.body_height(), 2.8);  // |close - open|
assert!(bar.is_consistent());        // finite, and high >= low

// A tick-only series: one price per period, no range.
let flat = Bar::flat(101.0, 500.0);
```

`typical_price()` gives `(high + low + close) / 3`, which is what VWAP and MFI weight
by volume. `true_range(previous_close)` gives the largest of the high/low span and the
two gaps from the previous close — the definition ATR averages.

### `PriceSeries`

A series of bars plus the axis labels that go with them:

```rust
use rust_widgets::widget::special_widgets::finance::{Bar, PriceSeries};

let mut series = PriceSeries::new();
series.push(Bar::new(100.0, 103.0, 99.0, 102.0, 1_000.0));
series.push_with_label(Bar::new(102.0, 105.0, 101.0, 104.0, 1_500.0), "Day 2".into());

assert_eq!(series.len(), 2);
assert_eq!(series.label_at(1), Some("Day 2"));

// Column views for the indicators:
let closes = series.closes();
let volumes = series.volumes();

// And the price extent, for an axis:
let (low, high) = series.price_extent();
```

Bars and labels are stored together rather than as two vectors, because index
alignment is the one invariant every overlay depends on — and two vectors can fall out
of step, which shows up as a label on the wrong period rather than as a failure.

### `OrderBook` and `Quote`

```rust
use rust_widgets::widget::special_widgets::finance::{BookLevel, OrderBook, Quote};

let mut book = OrderBook::new();
book.set_bids(vec![BookLevel::new(99.9, 300.0), BookLevel::new(99.8, 500.0)]);
book.set_asks(vec![BookLevel::new(100.1, 250.0), BookLevel::new(100.2, 400.0)]);

// Both sides are sorted best-first regardless of the order you supplied.
assert_eq!(book.best_bid().unwrap().price, 99.9);
assert_eq!(book.best_ask().unwrap().price, 100.1);
assert!((book.spread().unwrap() - 0.2).abs() < 1e-9);
assert_eq!(book.cumulative_quantity(true, 2), 800.0);  // bids

let quote = Quote::new("ACME", 102.5, 100.0);
assert!((quote.change() - 2.5).abs() < 1e-9);
assert!((quote.change_percent() - 2.5).abs() < 1e-9);
assert!(quote.is_up());
```

---

## 2. Technical indicators

`finance::indicators` has the arithmetic as **pure functions over `&[f64]`**. They are
not controls: a moving average has no geometry, so as functions they are testable
without a window and the drawing stays one shared concern.

```rust
use rust_widgets::widget::special_widgets::finance::indicators::{
    sma, ema, rsi, macd, bollinger_bands, atr, vwap, stochastic, donchian_channel,
    money_flow_index, on_balance_volume,
};

let closes: Vec<f64> = (0..200).map(|i| 100.0 + (i as f64 * 0.3).sin() * 5.0).collect();

let average = sma(&closes, 20);            // simple moving average
let smoothed = ema(&closes, 12);           // exponential, seeded from the first close
let strength = rsi(&closes, 14);           // 0..=100
let (line, signal, histogram) = macd(&closes, 12, 26, 9);
let (lower, middle, upper) = bollinger_bands(&closes, 20, 2.0);
```

### The padding convention

**Every function returns a vector the same length as its input**, with positions that
are not yet computable filled with `NAN`:

```rust
# use rust_widgets::widget::special_widgets::finance::indicators::sma;
let values = [1.0, 2.0, 3.0, 4.0, 5.0];
let result = sma(&values, 3);

assert!(result[0].is_nan());  // one sample cannot fill a 3-period window
assert!(result[1].is_nan());
assert_eq!(result[2], 2.0);   // mean of 1,2,3
assert_eq!(result[4], 4.0);   // mean of 3,4,5
```

Output index `i` always corresponds to input index `i`. That is what lets an overlay be
drawn beside a price series without either being re-aligned, and `align_left` pads a
shorter derived series to a longer one when you need to stack them explicitly.

A gap in the input produces a gap in the output rather than an interpolated value: a
missing tick means the window containing it is undefined, and inventing a number for it
would look like real analysis.

### Which RSI, and which standard deviation

Two choices are worth stating because implementations differ and the difference is
visible:

- `rsi` uses **Wilder's** original form — a plain mean of the first `period` gains and
  losses, then Wilder smoothing. Many spreadsheet templates use a simple moving average
  throughout, and the two disagree by a few tenths on the same data.
- `bollinger_bands` uses the **population** standard deviation, which is Bollinger's own
  formulation. The sample form is about 3% wider at period 20.

### The volume-weighted indicators

```rust
# use rust_widgets::widget::special_widgets::finance::indicators::{vwap, money_flow_index, on_balance_volume};
# let highs = vec![101.0; 60];
# let lows = vec![99.0; 60];
# let closes = vec![100.0; 60];
# let volumes = vec![1_000.0; 60];
let average_price = vwap(&highs, &lows, &closes, &volumes, 14);
let flow = money_flow_index(&highs, &lows, &closes, &volumes, 14);  // 0..=100
let balance = on_balance_volume(&closes, &volumes);                 // cumulative
```

`vwap` weights by the typical price, not the close, because an intraday VWAP should
reflect where trade happened rather than where the bar ended. A window with zero total
volume is `NAN` rather than a carried-forward level.

---

## 3. The K-line chart

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{
    CandlestickChart, Overlay, PriceLevelKind, PriceLine, PriceSeries,
};

# let series = PriceSeries::new();
let mut chart = CandlestickChart::new(Rect::new(0, 0, 900, 420));
chart.set_series(series);

// Overlays are a list, so you can show a moving-average ribbon.
chart.add_overlay(Overlay::moving_average(20));
chart.add_overlay(Overlay::moving_average(60));
chart.add_overlay(Overlay::bollinger_bands(20, 2.0));

// And levels are annotations drawn over the candles.
chart.add_price_line(PriceLine::new(95.0, PriceLevelKind::Support));
chart.add_price_line(PriceLine::new(120.0, PriceLevelKind::Resistance));
```

| Overlay | What it draws |
|---|---|
| `Overlay::moving_average(n)` | A simple moving average of the closes |
| `Overlay::exponential_moving_average(n)` | An EMA, seeded from the first close |
| `Overlay::bollinger_bands(n, k)` | A moving average with ±k deviation envelopes |
| `Overlay::vwap(n)` | The volume-weighted average price |
| `Overlay::donchian_channel(n)` | The trailing high/low envelope |

`Overlay::warm_up_bars()` reports how many leading bars an overlay cannot cover, so you
can explain a gap at the left of the chart rather than leaving it unexplained.

### Interaction

```rust
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::CandlestickChart;
# let mut chart = CandlestickChart::new(Rect::new(0, 0, 900, 420));
chart.bar_hovered.connect(|index| { /* *index */ });
chart.bar_clicked.connect(|index| { /* *index */ });
chart.bar_unhovered.connect(|index| { /* *index */ });

// Or read it directly:
let under_pointer = chart.hovered_index();
let bar = chart.hovered_bar();
```

Signals carry the bar **index**, which is the same index the series uses — so
`series.bars()[index]` is the bar the pointer was over.

---

## 4. Stacking a price pane with its volume and an oscillator

The panes share one horizontal index axis, so bar 17's volume sits under bar 17's
candle and the oscillator lines up too. Share the series and the geometry:

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{
    CandlestickChart, IndicatorChart, IndicatorMode, PriceSeries, VolumeChart,
};

# let series = PriceSeries::new();
let price = CandlestickChart::new(Rect::new(0, 0, 900, 300));
let volume = VolumeChart::new(Rect::new(0, 300, 900, 100));
let momentum = IndicatorChart::new(Rect::new(0, 400, 900, 150));
# let _ = (price, volume, momentum);

// Give all three the same series.
# let mut price = CandlestickChart::new(Rect::new(0, 0, 900, 300));
# let mut volume = VolumeChart::new(Rect::new(0, 300, 900, 100));
# let mut momentum = IndicatorChart::new(Rect::new(0, 400, 900, 150));
price.set_series(series.clone());
volume.set_series(series.clone());
momentum.set_series(series);
momentum.set_mode(IndicatorMode::Macd);
```

The alignment is arithmetic rather than a convention: `finance::layout` owns the
index-to-x and price-to-y mappings, so two panes computing them separately cannot drift
by a pixel and silently put an overlay on the wrong bar.

### The indicator pane

```rust
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::{IndicatorChart, IndicatorMode};
# let mut momentum = IndicatorChart::new(Rect::new(0, 0, 900, 150));
momentum.set_mode(IndicatorMode::Rsi);
momentum.set_period(14);
momentum.set_show_reference_levels(true);

// The numbers behind the picture, without needing a render target:
let values = momentum.compute();
```

| Mode | Series | Axis |
|---|---|---|
| `Macd` | MACD line, signal, histogram | Scales to the data, kept symmetric about zero |
| `Rsi` | RSI | Fixed `0..=100`, reference levels 30/70 |
| `Stochastic` | `%K`, `%D` | Fixed `0..=100`, reference levels 30/70 |
| `MoneyFlowIndex` | MFI | Fixed `0..=100`, reference levels 20/80 |
| `Atr` | ATR | Scales to the data |
| `OnBalanceVolume` | OBV | Scales to the data |

The bounded modes have a **fixed** axis on purpose: if RSI's axis rescaled to each
window, the 70 line would move and "overbought" would mean something different every
time you looked. `IndicatorMode::fixed_range()` tells you which you have.

### The volume pane

```rust
use rust_widgets::widget::special_widgets::finance::VolumeColorMode;
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::VolumeChart;
# let mut volume = VolumeChart::new(Rect::new(0, 0, 900, 100));
volume.set_color_mode(VolumeColorMode::Direction);  // green up, red down
volume.set_headroom(0.92);                          // tallest bar's share of the pane
```

It takes the **price series**, not a list of volumes, so a bar's colour is read from the
same bar its volume came from and the two cannot disagree.

---

## 5. The order book and the depth curve

They show the same data and answer different questions: a ladder is read row by row,
a depth curve as one picture.

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{DepthChart, OrderBookWidget, BookSide};

# let book = rust_widgets::widget::special_widgets::finance::OrderBook::new();
let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 260));
ladder.set_book(book.clone());     // both sides sorted best-first
ladder.set_depth(5);               // five levels per side, the usual published depth
ladder.level_clicked.connect(|(side, index)| {
    let _ = (side == BookSide::Bid, *index);
});

let mut depth = DepthChart::new(Rect::new(320, 0, 480, 260));
depth.set_book(book);
depth.set_depth(20);               // keep the curve near the touch

// The plotted points, so your tooltip cannot disagree with the curve:
for point in depth.curve() {
    let _ = (point.price, point.cumulative_quantity, point.side);
}
```

The depth curve is drawn as a **step function**, not a smooth line: a level's quantity
is constant across its price and jumps at the next level, so interpolating would draw
size that does not exist at prices in between.

---

## 6. The quote board

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{Quote, QuoteBoard, QuoteColumn, QuoteSort};

# let quotes: Vec<Quote> = Vec::new();
let mut board = QuoteBoard::new(Rect::new(0, 0, 760, 300));
board.set_quotes(quotes);

board.set_columns(vec![
    QuoteColumn::Symbol,
    QuoteColumn::Last,
    QuoteColumn::Change,
    QuoteColumn::ChangePercent,
    QuoteColumn::Volume,
]);

board.set_sort(QuoteSort::ChangeMagnitude);  // the day's biggest movers first
board.quote_clicked.connect(|symbol| { /* symbol */ });
board.selection_changed.connect(|symbol| { /* Option<String> */ });
```

| Sort | Order |
|---|---|
| `QuoteSort::None` | **Your** order, restored exactly |
| `QuoteSort::Symbol` | Alphabetical |
| `QuoteSort::LastDescending` | Highest price first |
| `QuoteSort::ChangeMagnitude` | Biggest movers first, either direction |

`QuoteSort::None` restores the order you supplied rather than reversing whatever a
previous sort left behind — a watchlist's order is yours. `QuoteBoard::select` emits
`selection_changed` only on a real move, and an out-of-range index clears the selection
instead of pointing at nothing.

---

## 7. Reading and writing these controls as properties

Every control in the family implements the same property contract as the rest of the
crate, so they work from JSON, CSS and the C ABI without special cases:

```rust
use rust_widgets::widget::capability::CapabilityValue;
use rust_widgets::widget::capability::properties_trait::{widget_property_get, widget_property_set};

# let mut chart = rust_widgets::widget::special_widgets::finance::CandlestickChart::new(
#     rust_widgets::core::Rect::new(0, 0, 900, 420));
// A price series as a comma-separated list becomes flat bars.
widget_property_set(&mut chart, "series", CapabilityValue::String("100,101.5,99,102".into())).unwrap();

let count = widget_property_get(&chart, "overlay_count").unwrap();
# let _ = count;
```

| Control | Properties |
|---|---|
| `CandlestickChart` | `series`, `overlay_count`, `show_price_levels` |
| `VolumeChart` | `series`, `color_mode`, `headroom` |
| `DepthChart` | `depth`, `bid_color`, `ask_color` |
| `OrderBook` | `depth`, `decimals`, `show_spread` |
| `QuoteBoard` | `sort`, `selected_index`, `row_height` |
| `IndicatorChart` | `series`, `mode`, `period`, `show_reference_levels` |

A property that cannot be written is not declared writable: `show_spread` and
`row_height` are readable so you can confirm them, and writing them reports
`ReadOnlyProperty` rather than silently doing nothing.

---

## 8. Degenerate data

A live feed delivers empty lists, malformed ticks and halted sessions. Every control
handles them by drawing something honest rather than panicking:

- An **empty series** draws nothing. No division by a length, no panic in the paint
  callback — which on a platform backend runs inside a native window procedure.
- A **flat series** (every bar at one price) widens the price axis rather than dividing
  by a zero range, and draws a straight line through the middle. That is what a flat
  price looks like.
- A **malformed bar** — `NaN` prices, or a high below its low — is skipped by the
  drawing code and reported by `Bar::is_consistent()`, so you can audit a historical
  load rather than discovering it as a gap on screen.
- **Zero total volume** makes `vwap` `NAN` rather than carrying a level forward that no
  trade produced.
- A **zero-width range** in the stochastic puts the close at the midpoint rather than
  dividing by zero.

The general rule: a chart that refuses one bad tick from a live feed is worse than one
that draws it oddly and lets you see that something is wrong.
