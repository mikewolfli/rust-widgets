# 金融與行情控件

rust-widgets 提供一整套金融交易控件：K 线图、成交量副图、深度圖、盤口報價表、
行情表，以及它们共同依賴的技術指標計算。

与函式庫中其他控件一样，它们是**自绘的、平台无关的**。不依賴任何行情資料源：
你提供 K 线資料，控件负责繪製。

---

## 1. 資料模型

整个控件族都建構在同一个序列型別之上。

### `Bar`

一根 OHLCV K 线，是金融图表的基本单位：

```rust
use rust_widgets::widget::special_widgets::finance::Bar;

// open, high, low, close, volume
let bar = Bar::new(100.0, 103.5, 99.2, 102.8, 1_250_000.0);
assert!(bar.is_rising());            // close >= open
assert_eq!(bar.body_height(), 2.8);  // |close - open|
assert!(bar.is_consistent());        // 有限值，且 high >= low

// 只有成交价的序列：每週期一个價格，无振幅
let flat = Bar::flat(101.0, 500.0);
```

`typical_price()` 返回 `(high + low + close) / 3`，即 VWAP 与 MFI 按成交量加權的價格。
`true_range(previous_close)` 返回最高/最低振幅与「相对前收盘的两个跳空」中的最大值 ——
这正是 ATR 所平均的定义。

### `PriceSeries`

由 K 线与其座標軸標籤组成：

```rust
use rust_widgets::widget::special_widgets::finance::{Bar, PriceSeries};

let mut series = PriceSeries::new();
series.push(Bar::new(100.0, 103.0, 99.0, 102.0, 1_000.0));
series.push_with_label(Bar::new(102.0, 105.0, 101.0, 104.0, 1_500.0), "Day 2".into());

assert_eq!(series.len(), 2);
assert_eq!(series.label_at(1), Some("Day 2"));

// 供指标使用的按列视图：
let closes = series.closes();
let volumes = series.volumes();

// 以及價格范围，用于建構座標軸：
let (low, high) = series.price_extent();
```

K 线与標籤存放在同一个型別中，而非两个向量，因为「索引对齐」是所有疊加线依賴的唯一不变量 ——
两个向量会失去同步，而后果是標籤标到了錯誤的週期上，既不会报错也难以察觉。

### `OrderBook` 与 `Quote`

```rust
use rust_widgets::widget::special_widgets::finance::{BookLevel, OrderBook, Quote};

let mut book = OrderBook::new();
book.set_bids(vec![BookLevel::new(99.9, 300.0), BookLevel::new(99.8, 500.0)]);
book.set_asks(vec![BookLevel::new(100.1, 250.0), BookLevel::new(100.2, 400.0)]);

// 无论你传入的顺序如何，两侧都会按「最优价优先」排序。
assert_eq!(book.best_bid().unwrap().price, 99.9);
assert_eq!(book.best_ask().unwrap().price, 100.1);
assert!((book.spread().unwrap() - 0.2).abs() < 1e-9);
assert_eq!(book.cumulative_quantity(true, 2), 800.0);  // 买盘

let quote = Quote::new("ACME", 102.5, 100.0);
assert!((quote.change() - 2.5).abs() < 1e-9);
assert!((quote.change_percent() - 2.5).abs() < 1e-9);
assert!(quote.is_up());
```

---

## 2. 技術指標

`finance::indicators` 把指标計算實作为**作用于 `&[f64]` 的纯函式**。它们不是控件：
一条均线没有幾何、没有狀態、也没有平台呼叫。作为函式，它们無需視窗即可測試，
而繪製则保持在消費它们的图表控件中，只實作一次。

```rust
use rust_widgets::widget::special_widgets::finance::indicators::{
    sma, ema, rsi, macd, bollinger_bands, atr, vwap, stochastic, donchian_channel,
    money_flow_index, on_balance_volume,
};

let closes: Vec<f64> = (0..200).map(|i| 100.0 + (i as f64 * 0.3).sin() * 5.0).collect();

let average = sma(&closes, 20);            // 簡單移動平均
let smoothed = ema(&closes, 12);           // 指數移動平均，以首个收盘价播種
let strength = rsi(&closes, 14);           // 0..=100
let (line, signal, histogram) = macd(&closes, 12, 26, 9);
let (lower, middle, upper) = bollinger_bands(&closes, 20, 2.0);
```

### 补位約定

**每个函式返回与输入等长的向量**，尚不可計算的位置填 `NAN`：

```rust
# use rust_widgets::widget::special_widgets::finance::indicators::sma;
let values = [1.0, 2.0, 3.0, 4.0, 5.0];
let result = sma(&values, 3);

assert!(result[0].is_nan());  // 一个樣本填不满 3 週期視窗
assert!(result[1].is_nan());
assert_eq!(result[2], 2.0);   // 1、2、3 的均值
assert_eq!(result[4], 4.0);   // 3、4、5 的均值
```

输出索引 `i` 永远对应输入索引 `i`。正因如此，疊加线才能与價格序列并排繪製而無需重新对齐；
当你確實需要显式堆叠时，`align_left` 可把较短的派生序列补到较长序列的长度。

输入中的缺口会在输出中产生缺口，而不是插值：缺失的一筆意味着包含它的視窗无法定义，
为它编造一个数字会让结果看起来像真实分析。

### 是哪一个 RSI、哪一种標準差

有两个选择需要明确写出，因为不同實作并不一致，而差异是可見的：

- `rsi` 采用 **Wilder** 原始形式 —— 先取前 `period` 个涨跌幅的普通均值，之后用 Wilder 平滑。
  许多電子表格範本全程使用簡單移動平均，两者在同一資料上会相差零点几个点。
- `bollinger_bands` 使用**總體**標準差，即 Bollinger 本人的公式。樣本標準差在 20 週期时约宽 3%。

### 成交量加權指标

```rust
# use rust_widgets::widget::special_widgets::finance::indicators::{vwap, money_flow_index, on_balance_volume};
# let highs = vec![101.0; 60];
# let lows = vec![99.0; 60];
# let closes = vec![100.0; 60];
# let volumes = vec![1_000.0; 60];
let average_price = vwap(&highs, &lows, &closes, &volumes, 14);
let flow = money_flow_index(&highs, &lows, &closes, &volumes, 14);  // 0..=100
let balance = on_balance_volume(&closes, &volumes);                 // 累加值
```

`vwap` 按典型價格加權而非收盘价，因为日內 VWAP 应反映成交发生的位置，而不是 K 线結束的位置。
总成交量为零的視窗返回 `NAN`，而不是沿用前一个價位。

---

## 3. K 线图

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{
    CandlestickChart, Overlay, PriceLevelKind, PriceLine, PriceSeries,
};

# let series = PriceSeries::new();
let mut chart = CandlestickChart::new(Rect::new(0, 0, 900, 420));
chart.set_series(series);

// 疊加线是一个列表，因此可以画出均线带
chart.add_overlay(Overlay::moving_average(20));
chart.add_overlay(Overlay::moving_average(60));
chart.add_overlay(Overlay::bollinger_bands(20, 2.0));

// 價位线是繪製在蠟燭之上的标注
chart.add_price_line(PriceLine::new(95.0, PriceLevelKind::Support));
chart.add_price_line(PriceLine::new(120.0, PriceLevelKind::Resistance));
```

| 疊加线 | 繪製内容 |
|---|---|
| `Overlay::moving_average(n)` | 收盘价的簡單移動平均 |
| `Overlay::exponential_moving_average(n)` | 指數移動平均，以首个收盘价播種 |
| `Overlay::bollinger_bands(n, k)` | 移動平均及其 ±k 倍標準差包絡 |
| `Overlay::vwap(n)` | 成交量加權平均价 |
| `Overlay::donchian_channel(n)` | 移动最高/最低包絡 |

`Overlay::warm_up_bars()` 報告该疊加线无法覆盖的前導 K 线数量，
因此你可以解释图表左侧的空白，而不是让它無從解釋。

### 交互

```rust
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::CandlestickChart;
# let mut chart = CandlestickChart::new(Rect::new(0, 0, 900, 420));
chart.bar_hovered.connect(|index| { /* *index */ });
chart.bar_clicked.connect(|index| { /* *index */ });
chart.bar_unhovered.connect(|index| { /* *index */ });

// 或直接讀取：
let under_pointer = chart.hovered_index();
let bar = chart.hovered_bar();
```

訊號携带的是 K 线**索引**，与序列自身使用的索引一致 ——
因此 `series.bars()[index]` 就是指標所指的那根 K 线。

---

## 4. 把價格、成交量与振盪指标叠起来

各面板共享同一条水平索引軸，因此第 17 根 K 线的成交量位于第 17 根蠟燭下方，
振盪指标也能对齐。共享同一个序列即可：

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{
    CandlestickChart, IndicatorChart, IndicatorMode, PriceSeries, VolumeChart,
};

# let series = PriceSeries::new();
# let mut price = CandlestickChart::new(Rect::new(0, 0, 900, 300));
# let mut volume = VolumeChart::new(Rect::new(0, 300, 900, 100));
# let mut momentum = IndicatorChart::new(Rect::new(0, 400, 900, 150));
price.set_series(series.clone());
volume.set_series(series.clone());
momentum.set_series(series);
momentum.set_mode(IndicatorMode::Macd);
```

这种对齐是**算術保证**而非約定：`finance::layout` 獨佔持有「索引→x」与「價格→y」映射，
因此两个面板各自計算时不会相差一个像素，也就不会把疊加线悄悄画到錯誤的 K 线上。

### 指标面板

```rust
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::{IndicatorChart, IndicatorMode};
# let mut momentum = IndicatorChart::new(Rect::new(0, 0, 900, 150));
momentum.set_mode(IndicatorMode::Rsi);
momentum.set_period(14);
momentum.set_show_reference_levels(true);

// 讀取图形背后的数值，無需渲染目标：
let values = momentum.compute();
```

| 模式 | 序列 | 座標軸 |
|---|---|---|
| `Macd` | MACD 线、訊號线、柱状图 | 按資料缩放，并保持关于零对称 |
| `Rsi` | RSI | 固定 `0..=100`，参考线 30/70 |
| `Stochastic` | `%K`、`%D` | 固定 `0..=100`，参考线 30/70 |
| `MoneyFlowIndex` | MFI | 固定 `0..=100`，参考线 20/80 |
| `Atr` | ATR | 按資料缩放 |
| `OnBalanceVolume` | OBV | 按資料缩放 |

有界模式使用**固定**座標軸是刻意的：如果 RSI 的軸随每个視窗重新縮放，
70 这条线就会移动，「超買」的含義会在你每次查看时都不同。
`IndicatorMode::fixed_range()` 会告诉你当前是哪种。

### 成交量面板

```rust
use rust_widgets::widget::special_widgets::finance::VolumeColorMode;
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::VolumeChart;
# let mut volume = VolumeChart::new(Rect::new(0, 0, 900, 100));
volume.set_color_mode(VolumeColorMode::Direction);  // 漲綠跌紅
volume.set_headroom(0.92);                          // 最高柱占面板高度的比例
```

它接收的是**價格序列**而不是一串成交量，因此柱子颜色讀取自成交量所属的那根 K 线，
两者不可能不一致。

---

## 5. 盤口与深度圖

它们展示同一份資料，回答的却是两个不同问题：報價表按行阅读，深度圖當作一张图阅读。

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{DepthChart, OrderBookWidget, BookSide};

# let book = rust_widgets::widget::special_widgets::finance::OrderBook::new();
let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 260));
ladder.set_book(book.clone());     // 两侧均按最优价优先排序
ladder.set_depth(5);               // 每侧五档，常见的揭露檔位
ladder.level_clicked.connect(|(side, index)| {
    let _ = (side == BookSide::Bid, *index);
});

let mut depth = DepthChart::new(Rect::new(320, 0, 480, 260));
depth.set_book(book);
depth.set_depth(20);               // 让曲線集中在盤口附近

// 实际繪製的資料点，使你的提示框不会与曲線不一致：
for point in depth.curve() {
    let _ = (point.price, point.cumulative_quantity, point.side);
}
```

深度曲線繪製为**階梯函式**而非平滑曲線：某个價位的数量在该價位上是常量，
到下一档才跳變，因此插值会画出中間價位上并不存在的数量。

---

## 6. 行情表

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

board.set_sort(QuoteSort::ChangeMagnitude);  // 当日振幅最大者在前
board.quote_clicked.connect(|symbol| { /* symbol */ });
board.selection_changed.connect(|symbol| { /* Option<String> */ });
```

| 排序 | 顺序 |
|---|---|
| `QuoteSort::None` | **你提供的**顺序，精确還原 |
| `QuoteSort::Symbol` | 按字母 |
| `QuoteSort::LastDescending` | 價格由高到低 |
| `QuoteSort::ChangeMagnitude` | 振幅最大者在前，涨跌皆可 |

`QuoteSort::None` 会還原你提供的顺序，而不是把上一次排序的结果倒过来 ——
自選股的顺序屬於你。`QuoteBoard::select` 仅在真正移动时發出 `selection_changed`，
越界索引会清除選中狀態，而不是指向一个不存在的行。

---

## 7. 以屬性方式读写这些控件

该控件族的每个控件都實作与函式庫中其他控件相同的屬性契約，
因此無需特殊分支即可从 JSON、CSS 与 C ABI 使用：

```rust
use rust_widgets::widget::capability::CapabilityValue;
use rust_widgets::widget::capability::properties_trait::{widget_property_get, widget_property_set};

# let mut chart = rust_widgets::widget::special_widgets::finance::CandlestickChart::new(
#     rust_widgets::core::Rect::new(0, 0, 900, 420));
// 逗號分隔的價格序列会被轉換为平價 K 线
widget_property_set(&mut chart, "series", CapabilityValue::String("100,101.5,99,102".into())).unwrap();

let count = widget_property_get(&chart, "overlay_count").unwrap();
# let _ = count;
```

| 控件 | 屬性 |
|---|---|
| `CandlestickChart` | `series`、`overlay_count`、`show_price_levels` |
| `VolumeChart` | `series`、`color_mode`、`headroom` |
| `DepthChart` | `depth`、`bid_color`、`ask_color` |
| `OrderBook` | `depth`、`decimals`、`show_spread` |
| `QuoteBoard` | `sort`、`selected_index`、`row_height` |
| `IndicatorChart` | `series`、`mode`、`period`、`show_reference_levels` |

不可寫的屬性不会被宣告为可写：`show_spread` 与 `row_height` 可读，以便你確認其值，
而寫入它们会返回 `ReadOnlyProperty`，而不是靜默什么都不做。

---

## 8. 退化資料

即時行情会送来空列表、畸形 tick 与停牌时段。每个控件的处理方式都是繪製出誠實的结果，
而不是 panic：

- **空序列**什么都不画。不会除以长度，也不会在繪製回呼中 panic ——
  在平台后端里，那个回呼執行在原生視窗過程之内。
- **平價序列**（每根 K 线價格相同）会放宽價格軸，而不是除以零振幅，并画出穿过中部的直線。
  平價價格看起来確實就是这样。
- **畸形 K 线**（`NaN` 價格，或最高价低于最低价）会被繪製代码跳過，
  并由 `Bar::is_consistent()` 報告，因此你可以在歷史資料載入后稽核，而不是在螢幕上看出一道缺口。
- **总成交量为零**时 `vwap` 返回 `NAN`，而不是沿用没有任何成交产生的價位。
- 随机指标的**零寬度振幅**会把收盘价放在中点，而不是除以零。

总的原则：一个因为即時行情里有一筆坏 tick 就拒絕繪製的图表，
比一个把它画得有点奇怪、让你看得出有问题的图表更糟。
