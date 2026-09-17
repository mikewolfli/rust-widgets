# 金融与行情控件

rust-widgets 提供一整套金融交易控件：K 线图、成交量副图、深度图、盘口报价表、
行情表，以及它们共同依赖的技术指标计算。

与库中其他控件一样，它们是**自绘的、平台无关的**。不依赖任何行情数据源：
你提供 K 线数据，控件负责绘制。

---

## 1. 数据模型

整个控件族都构建在同一个序列类型之上。

### `Bar`

一根 OHLCV K 线，是金融图表的基本单位：

```rust
use rust_widgets::widget::special_widgets::finance::Bar;

// open, high, low, close, volume
let bar = Bar::new(100.0, 103.5, 99.2, 102.8, 1_250_000.0);
assert!(bar.is_rising());            // close >= open
assert_eq!(bar.body_height(), 2.8);  // |close - open|
assert!(bar.is_consistent());        // 有限值，且 high >= low

// 只有成交价的序列：每周期一个价格，无振幅
let flat = Bar::flat(101.0, 500.0);
```

`typical_price()` 返回 `(high + low + close) / 3`，即 VWAP 与 MFI 按成交量加权的价格。
`true_range(previous_close)` 返回最高/最低振幅与「相对前收盘的两个跳空」中的最大值 ——
这正是 ATR 所平均的定义。

### `PriceSeries`

由 K 线与其坐标轴标签组成：

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

// 以及价格范围，用于构建坐标轴：
let (low, high) = series.price_extent();
```

K 线与标签存放在同一个类型中，而非两个向量，因为「索引对齐」是所有叠加线依赖的唯一不变量 ——
两个向量会失去同步，而后果是标签标到了错误的周期上，既不会报错也难以察觉。

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

## 2. 技术指标

`finance::indicators` 把指标计算实现为**作用于 `&[f64]` 的纯函数**。它们不是控件：
一条均线没有几何、没有状态、也没有平台调用。作为函数，它们无需窗口即可测试，
而绘制则保持在消费它们的图表控件中，只实现一次。

```rust
use rust_widgets::widget::special_widgets::finance::indicators::{
    sma, ema, rsi, macd, bollinger_bands, atr, vwap, stochastic, donchian_channel,
    money_flow_index, on_balance_volume,
};

let closes: Vec<f64> = (0..200).map(|i| 100.0 + (i as f64 * 0.3).sin() * 5.0).collect();

let average = sma(&closes, 20);            // 简单移动平均
let smoothed = ema(&closes, 12);           // 指数移动平均，以首个收盘价播种
let strength = rsi(&closes, 14);           // 0..=100
let (line, signal, histogram) = macd(&closes, 12, 26, 9);
let (lower, middle, upper) = bollinger_bands(&closes, 20, 2.0);
```

### 补位约定

**每个函数返回与输入等长的向量**，尚不可计算的位置填 `NAN`：

```rust
# use rust_widgets::widget::special_widgets::finance::indicators::sma;
let values = [1.0, 2.0, 3.0, 4.0, 5.0];
let result = sma(&values, 3);

assert!(result[0].is_nan());  // 一个样本填不满 3 周期窗口
assert!(result[1].is_nan());
assert_eq!(result[2], 2.0);   // 1、2、3 的均值
assert_eq!(result[4], 4.0);   // 3、4、5 的均值
```

输出索引 `i` 永远对应输入索引 `i`。正因如此，叠加线才能与价格序列并排绘制而无需重新对齐；
当你确实需要显式堆叠时，`align_left` 可把较短的派生序列补到较长序列的长度。

输入中的缺口会在输出中产生缺口，而不是插值：缺失的一笔意味着包含它的窗口无法定义，
为它编造一个数字会让结果看起来像真实分析。

### 是哪一个 RSI、哪一种标准差

有两个选择需要明确写出，因为不同实现并不一致，而差异是可见的：

- `rsi` 采用 **Wilder** 原始形式 —— 先取前 `period` 个涨跌幅的普通均值，之后用 Wilder 平滑。
  许多电子表格模板全程使用简单移动平均，两者在同一数据上会相差零点几个点。
- `bollinger_bands` 使用**总体**标准差，即 Bollinger 本人的公式。样本标准差在 20 周期时约宽 3%。

### 成交量加权指标

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

`vwap` 按典型价格加权而非收盘价，因为日内 VWAP 应反映成交发生的位置，而不是 K 线结束的位置。
总成交量为零的窗口返回 `NAN`，而不是沿用前一个价位。

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

// 叠加线是一个列表，因此可以画出均线带
chart.add_overlay(Overlay::moving_average(20));
chart.add_overlay(Overlay::moving_average(60));
chart.add_overlay(Overlay::bollinger_bands(20, 2.0));

// 价位线是绘制在蜡烛之上的标注
chart.add_price_line(PriceLine::new(95.0, PriceLevelKind::Support));
chart.add_price_line(PriceLine::new(120.0, PriceLevelKind::Resistance));
```

| 叠加线 | 绘制内容 |
|---|---|
| `Overlay::moving_average(n)` | 收盘价的简单移动平均 |
| `Overlay::exponential_moving_average(n)` | 指数移动平均，以首个收盘价播种 |
| `Overlay::bollinger_bands(n, k)` | 移动平均及其 ±k 倍标准差包络 |
| `Overlay::vwap(n)` | 成交量加权平均价 |
| `Overlay::donchian_channel(n)` | 移动最高/最低包络 |

`Overlay::warm_up_bars()` 报告该叠加线无法覆盖的前导 K 线数量，
因此你可以解释图表左侧的空白，而不是让它无从解释。

### 交互

```rust
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::CandlestickChart;
# let mut chart = CandlestickChart::new(Rect::new(0, 0, 900, 420));
chart.bar_hovered.connect(|index| { /* *index */ });
chart.bar_clicked.connect(|index| { /* *index */ });
chart.bar_unhovered.connect(|index| { /* *index */ });

// 或直接读取：
let under_pointer = chart.hovered_index();
let bar = chart.hovered_bar();
```

信号携带的是 K 线**索引**，与序列自身使用的索引一致 ——
因此 `series.bars()[index]` 就是指针所指的那根 K 线。

---

## 4. 把价格、成交量与振荡指标叠起来

各面板共享同一条水平索引轴，因此第 17 根 K 线的成交量位于第 17 根蜡烛下方，
振荡指标也能对齐。共享同一个序列即可：

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

这种对齐是**算术保证**而非约定：`finance::layout` 独占持有「索引→x」与「价格→y」映射，
因此两个面板各自计算时不会相差一个像素，也就不会把叠加线悄悄画到错误的 K 线上。

### 指标面板

```rust
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::{IndicatorChart, IndicatorMode};
# let mut momentum = IndicatorChart::new(Rect::new(0, 0, 900, 150));
momentum.set_mode(IndicatorMode::Rsi);
momentum.set_period(14);
momentum.set_show_reference_levels(true);

// 读取图形背后的数值，无需渲染目标：
let values = momentum.compute();
```

| 模式 | 序列 | 坐标轴 |
|---|---|---|
| `Macd` | MACD 线、信号线、柱状图 | 按数据缩放，并保持关于零对称 |
| `Rsi` | RSI | 固定 `0..=100`，参考线 30/70 |
| `Stochastic` | `%K`、`%D` | 固定 `0..=100`，参考线 30/70 |
| `MoneyFlowIndex` | MFI | 固定 `0..=100`，参考线 20/80 |
| `Atr` | ATR | 按数据缩放 |
| `OnBalanceVolume` | OBV | 按数据缩放 |

有界模式使用**固定**坐标轴是刻意的：如果 RSI 的轴随每个窗口重新缩放，
70 这条线就会移动，「超买」的含义会在你每次查看时都不同。
`IndicatorMode::fixed_range()` 会告诉你当前是哪种。

### 成交量面板

```rust
use rust_widgets::widget::special_widgets::finance::VolumeColorMode;
# use rust_widgets::core::Rect;
# use rust_widgets::widget::special_widgets::finance::VolumeChart;
# let mut volume = VolumeChart::new(Rect::new(0, 0, 900, 100));
volume.set_color_mode(VolumeColorMode::Direction);  // 涨绿跌红
volume.set_headroom(0.92);                          // 最高柱占面板高度的比例
```

它接收的是**价格序列**而不是一串成交量，因此柱子颜色读取自成交量所属的那根 K 线，
两者不可能不一致。

---

## 5. 盘口与深度图

它们展示同一份数据，回答的却是两个不同问题：报价表按行阅读，深度图当作一张图阅读。

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::finance::{DepthChart, OrderBookWidget, BookSide};

# let book = rust_widgets::widget::special_widgets::finance::OrderBook::new();
let mut ladder = OrderBookWidget::new(Rect::new(0, 0, 320, 260));
ladder.set_book(book.clone());     // 两侧均按最优价优先排序
ladder.set_depth(5);               // 每侧五档，常见的披露档位
ladder.level_clicked.connect(|(side, index)| {
    let _ = (side == BookSide::Bid, *index);
});

let mut depth = DepthChart::new(Rect::new(320, 0, 480, 260));
depth.set_book(book);
depth.set_depth(20);               // 让曲线集中在盘口附近

// 实际绘制的数据点，使你的提示框不会与曲线不一致：
for point in depth.curve() {
    let _ = (point.price, point.cumulative_quantity, point.side);
}
```

深度曲线绘制为**阶梯函数**而非平滑曲线：某个价位的数量在该价位上是常量，
到下一档才跳变，因此插值会画出中间价位上并不存在的数量。

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
| `QuoteSort::None` | **你提供的**顺序，精确还原 |
| `QuoteSort::Symbol` | 按字母 |
| `QuoteSort::LastDescending` | 价格由高到低 |
| `QuoteSort::ChangeMagnitude` | 振幅最大者在前，涨跌皆可 |

`QuoteSort::None` 会还原你提供的顺序，而不是把上一次排序的结果倒过来 ——
自选股的顺序属于你。`QuoteBoard::select` 仅在真正移动时发出 `selection_changed`，
越界索引会清除选中状态，而不是指向一个不存在的行。

---

## 7. 以属性方式读写这些控件

该控件族的每个控件都实现与库中其他控件相同的属性契约，
因此无需特殊分支即可从 JSON、CSS 与 C ABI 使用：

```rust
use rust_widgets::widget::capability::CapabilityValue;
use rust_widgets::widget::capability::properties_trait::{widget_property_get, widget_property_set};

# let mut chart = rust_widgets::widget::special_widgets::finance::CandlestickChart::new(
#     rust_widgets::core::Rect::new(0, 0, 900, 420));
// 逗号分隔的价格序列会被转换为平价 K 线
widget_property_set(&mut chart, "series", CapabilityValue::String("100,101.5,99,102".into())).unwrap();

let count = widget_property_get(&chart, "overlay_count").unwrap();
# let _ = count;
```

| 控件 | 属性 |
|---|---|
| `CandlestickChart` | `series`、`overlay_count`、`show_price_levels` |
| `VolumeChart` | `series`、`color_mode`、`headroom` |
| `DepthChart` | `depth`、`bid_color`、`ask_color` |
| `OrderBook` | `depth`、`decimals`、`show_spread` |
| `QuoteBoard` | `sort`、`selected_index`、`row_height` |
| `IndicatorChart` | `series`、`mode`、`period`、`show_reference_levels` |

不可写的属性不会被声明为可写：`show_spread` 与 `row_height` 可读，以便你确认其值，
而写入它们会返回 `ReadOnlyProperty`，而不是静默什么都不做。

---

## 8. 退化数据

实时行情会送来空列表、畸形 tick 与停牌时段。每个控件的处理方式都是绘制出诚实的结果，
而不是 panic：

- **空序列**什么都不画。不会除以长度，也不会在绘制回调中 panic ——
  在平台后端里，那个回调运行在原生窗口过程之内。
- **平价序列**（每根 K 线价格相同）会放宽价格轴，而不是除以零振幅，并画出穿过中部的直线。
  平价价格看起来确实就是这样。
- **畸形 K 线**（`NaN` 价格，或最高价低于最低价）会被绘制代码跳过，
  并由 `Bar::is_consistent()` 报告，因此你可以在历史数据加载后审计，而不是在屏幕上看出一道缺口。
- **总成交量为零**时 `vwap` 返回 `NAN`，而不是沿用没有任何成交产生的价位。
- 随机指标的**零宽度振幅**会把收盘价放在中点，而不是除以零。

总的原则：一个因为实时行情里有一笔坏 tick 就拒绝绘制的图表，
比一个把它画得有点奇怪、让你看得出有问题的图表更糟。
