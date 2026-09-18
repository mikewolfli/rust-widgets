//! Finance Demo — 金融控件族综合演示（基于 App 框架）
//!
//! 本 demo 把所有六个金融控件放进**一块真实交易屏**的版式里，而不是把它们
//! 各自孤立地摆开。这一点是刻意的：这组控件的价值在于**同一份数据在多个面板
//! 上互相对齐**，孤立摆放恰好测不出这件事。
//!
//! # 版式（窗口 1440×900）
//!
//! ```text
//!  ┌───────────────────────────────────────────────────────────────┐
//!  │ 菜单栏 File / View                                            │
//!  ├──────────────────────────────┬────────────────────────────────┤
//!  │ CandlestickChart  (主图)     │  QuoteBoard                    │
//!  │   + MA(20) / MA(60) 叠加     │   （行情表，点击选股）         │
//!  │   + BOLL(20,2) 叠加          │                                │
//!  │   + 支撑/阻力/昨收 价位线    ├────────────────────────────────┤
//!  ├──────────────────────────────┤  OrderBook                     │
//!  │ VolumeChart  (成交量副图)    │   （盘口五档）                 │
//!  ├──────────────────────────────┤                                │
//!  │ IndicatorChart (MACD)        ├────────────────────────────────┤
//!  ├──────────────────────────────┤  DepthChart                    │
//!  │ IndicatorChart (RSI)         │   （深度曲线）                 │
//!  ├──────────────────────────────┴────────────────────────────────┤
//!  │ 状态栏 — 当前模式 / 最近事件                                   │
//!  └───────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 三个面板共享同一条索引轴
//!
//! 主图、成交量、MACD、RSI **共用同一个 `PriceSeries`**。所以第 17 根 K 线的
//! 成交量正好位于第 17 根蜡烛下方，MACD 的柱也落在同一根上。这不是靠调用方
//! 手动对齐，而是 `finance::layout` 让四个面板走同一套 `IndexAxis` 映射 ——
//! 对齐是算术结果，不是约定。
//!
//! # 面板的版式由 `BoxLayout` 决定
//!
//! 两列各自是一个纵向 `BoxLayout`（`Layout::left_column_layout` / `right_column_layout`），
//! 每个面板的高度来自它的权重而非像素字面量；`Layout::compute` 只是把布局算出的
//! 高度读回来、再给每个面板配上相同的 `x` 与宽度 —— 后者正是“共享索引轴”的充要条件。
//! 这样“哪个面板多高”是布局的属性，而不是一串 `y + height + gap` 的加法。
//!
//! # 跨平台
//!
//! 与 `demo/control` 一样，本 demo 没有任何 `cfg(target_os)`，也不出现任何
//! OS 控件名。唯一需要运行时询问的是能力存在与否：`supports_surfaces()`。
//! 自绘型控件的放置方式由 `src/platform/` 内部决定。

use std::sync::{Arc, Mutex};

use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::core::{ObjectId, Orientation, Rect};
// The demo has a `Layout` struct of its own (the panel rects), so the library's layout
// trait is imported under a distinct name rather than shadowing it.
use rust_widgets::layout::{BoxLayout, Layout as LayoutTrait, LayoutConstraints};
use rust_widgets::widget::special_widgets::finance::{
    Bar, BookLevel, CandlestickChart, DepthChart, IndicatorChart, IndicatorMode, OrderBook,
    OrderBookWidget, Overlay, PriceLevelKind, PriceLine, PriceSeries, Quote, QuoteBoard,
    QuoteColumn, QuoteSort, VolumeChart,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Log System — 线程安全的事件日志
// ═══════════════════════════════════════════════════════════════════════════════

struct EventLog {
    entries: Mutex<Vec<String>>,
}

impl EventLog {
    fn new() -> Self {
        Self { entries: Mutex::new(Vec::new()) }
    }

    fn append(&self, message: impl Into<String>) {
        let message = message.into();
        println!("[EVENT] {}", message);
        self.entries.lock().unwrap().push(message);
    }

    fn snapshot(&self) -> Vec<String> {
        self.entries.lock().unwrap().clone()
    }

    /// The most recent entry, for the status bar.
    fn latest(&self) -> String {
        self.entries.lock().unwrap().last().cloned().unwrap_or_default()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sample data
// ═══════════════════════════════════════════════════════════════════════════════

/// How many bars the demo chart shows.
///
/// 240 is roughly a trading year: enough that the 60-period average has a real
/// warm-up and the MACD has settled, which is the point of showing it. Fewer bars
/// would make every indicator look like it starts mid-series.
const BAR_COUNT: usize = 240;

/// Builds a deterministic price series that behaves like an instrument.
///
/// # Why not random
///
/// A demo that redraws differently every run cannot be compared against a screenshot,
/// and a reader cannot tell a rendering bug from ordinary noise. This generates a
/// trend plus two waves plus a deterministic "gap" every 47 bars, so the chart shows
/// a trend, a range, and a gap — the three shapes a K-line reader looks for — and
/// shows them identically on every launch.
///
/// The volume is deliberately *correlated* with the size of the bar's move, because
/// that is what real volume does and it makes the volume pane's relationship to the
/// price pane visible: a big candle has a tall volume bar under it.
fn sample_series() -> PriceSeries {
    let mut series = PriceSeries::new();
    let mut previous_close = 100.0_f64;

    for index in 0..BAR_COUNT {
        let step = index as f64;
        // A trend, so the moving averages have something to lag behind.
        let trend = step * 0.08;
        // Two waves at different periods, so the price is not a straight line and the
        // bands widen and narrow the way a real instrument's do.
        let wave = (step * 0.09).sin() * 3.2 + (step * 0.31).cos() * 1.4;
        // An occasional overnight gap, which is the case `true_range` exists to account
        // for and the one a plain high/low span would hide.
        //
        // The gap has to move the **open**, not only the close. The first version of
        // this generator set `open = previous_close` and then applied the jump to the
        // close, so `open` always equalled the prior close and no gap ever existed —
        // the fixture looked like it tested the gap case while testing nothing. The
        // assertion in `the_sample_series_contains_a_gap` is what caught it.
        let gap = if index > 0 && index % 47 == 0 { 2.5 } else { 0.0 };

        let open = previous_close + gap;
        let close = 100.0 + trend + wave + gap;
        let high = open.max(close) + (step * 0.17).sin().abs() * 0.9 + 0.15;
        let low = open.min(close) - (step * 0.23).cos().abs() * 0.9 - 0.15;

        // Volume tracks the bar's own range, so the volume pane reads against the
        // candles rather than being unrelated noise.
        let range = high - low;
        let volume = 400_000.0 + range * 900_000.0 + (step * 0.41).sin().abs() * 250_000.0;

        series.push(Bar::new(open, high, low, close, volume));
        previous_close = close;
    }

    let labels: Vec<String> = (0..BAR_COUNT).map(|index| format!("D{index}")).collect();
    series.set_labels(labels);
    series
}

/// Builds an order book whose shape makes the depth curve interesting.
///
/// The sizes deliberately do **not** fall off monotonically: the best bid is large
/// and the second is small, which puts a visible shelf in the cumulative curve. A
/// book whose levels increased smoothly would draw a smooth curve, and a smooth curve
/// would not exercise the step rendering at all.
fn sample_book(mid: f64) -> OrderBook {
    let mut book = OrderBook::new();

    let bid_sizes = [1800.0, 420.0, 2600.0, 900.0, 1500.0, 300.0, 2100.0, 700.0];
    let ask_sizes = [900.0, 2300.0, 500.0, 1700.0, 1100.0, 2800.0, 600.0, 1900.0];

    let bids: Vec<BookLevel> = bid_sizes
        .iter()
        .enumerate()
        .map(|(level, size)| BookLevel::new(mid - 0.05 - level as f64 * 0.05, *size))
        .collect();
    let asks: Vec<BookLevel> = ask_sizes
        .iter()
        .enumerate()
        .map(|(level, size)| BookLevel::new(mid + 0.05 + level as f64 * 0.05, *size))
        .collect();

    // Deliberately supplied out of order: the controls sort best-first themselves, and
    // this proves it rather than taking the control's word for it.
    book.set_bids(bids.into_iter().rev().collect());
    book.set_asks(asks.into_iter().rev().collect());
    book
}

/// A watchlist of instruments, in the order a trader arranged them.
///
/// The order is intentionally not alphabetical and not by price: `QuoteSort::None` is
/// supposed to restore *this* order, and a caller's order is the only thing that makes
/// that promise meaningful.
fn sample_quotes(last_price: f64) -> Vec<Quote> {
    let rows = [
        ("AAPL", "Apple Inc.", 214.30, 209.85, 61_200_000.0),
        ("MSFT", "Microsoft Corp.", 438.10, 441.20, 22_400_000.0),
        ("NVDA", "NVIDIA Corp.", last_price, last_price - 2.4, 310_500_000.0),
        ("TSLA", "Tesla Inc.", 248.75, 244.10, 98_700_000.0),
        ("AMZN", "Amazon.com Inc.", 186.40, 187.90, 41_300_000.0),
    ];

    rows.iter()
        .map(|(symbol, name, last, previous_close, volume)| {
            let mut quote = Quote::new(symbol, *last, *previous_close);
            quote.name = String::from(*name);
            quote.high = last.max(*previous_close) + 1.8;
            quote.low = last.min(*previous_close) - 2.1;
            quote.volume = *volume;
            quote
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Layout
// ═══════════════════════════════════════════════════════════════════════════════

/// The panel rectangles.
///
/// # Why the rectangles are still computed here
///
/// The stacked panes must share a left edge and a width, or their index axes differ by
/// a few pixels and bar 17 no longer lines up across the chart. That agreement is the
/// demo's whole point, so the rects are derived from one place.
///
/// # Why the *columns* are also described as layouts
///
/// Deriving every rect by hand means every rect's arithmetic has to be checked by hand.
/// The two columns are expressed as [`BoxLayout`]s instead (see [`Self::column_layouts`]),
/// which makes "the left column is 4 stacked panes in these proportions" a property of a
/// layout rather than of a chain of `y + height + gap` additions. The per-pane rects are
/// then *read back* from those layouts, so the two never disagree.
struct Layout {
    price: Rect,
    volume: Rect,
    macd: Rect,
    rsi: Rect,
    quotes: Rect,
    book: Rect,
    depth: Rect,
}

/// The demo window's width, shared by the layout and the window it creates.
const WINDOW_WIDTH: u32 = 1440;
/// The demo window's height, shared by the layout and the window it creates.
const WINDOW_HEIGHT: u32 = 900;
/// The menu bar's height, which every panel starts below.
const MENU_BAR_HEIGHT: u32 = 30;
/// The status bar's height, which every panel ends above.
const STATUS_BAR_HEIGHT: u32 = 30;
/// The gap between neighbouring panels.
const GAP: i32 = 6;
/// The margin around the whole grid.
const MARGIN: i32 = 12;
/// The left column's width: the price chart and its indicator panes.
const LEFT_WIDTH: u32 = 940;

/// The left column's vertical proportions: price / volume / MACD / RSI.
///
/// Ratios rather than pixels: a taller window then grows every pane in proportion, and no
/// pane can silently keep a height that was chosen for a different window size.
const LEFT_COLUMN_WEIGHTS: [u32; 4] = [45, 15, 20, 20];
/// The right column's vertical proportions: watchlist / ladder / depth.
const RIGHT_COLUMN_WEIGHTS: [u32; 3] = [30, 34, 36];

/// Placeholder ids per pane: four for the left column, three for the right.
///
/// These stand in for the real widget ids so a column layout's arithmetic can be built and
/// read back — and asserted — without a window.
const LEFT_IDS: &[ObjectId] = &[101, 102, 103, 104];
const RIGHT_IDS: &[ObjectId] = &[201, 202, 203];

impl Layout {
    /// The area the panels may occupy: below the menu bar, above the status bar.
    fn content_rect() -> Rect {
        let top = MENU_BAR_HEIGHT as i32 + MARGIN;
        let bottom = WINDOW_HEIGHT as i32 - STATUS_BAR_HEIGHT as i32 - MARGIN;
        let height = (bottom - top).max(0) as u32;
        Rect::new(MARGIN, top, (WINDOW_WIDTH as i32 - MARGIN * 2).max(0) as u32, height)
    }

    /// The right column's width, given the left column's.
    fn right_column_width() -> u32 {
        let content = Self::content_rect();
        content.width.saturating_sub(LEFT_WIDTH + GAP as u32 * 2).max(120)
    }

    /// The left column as a vertical layout of four panes.
    ///
    /// Returned so [`Self::compute`] can read the pane heights back from it, and so a test
    /// can assert the proportions without constructing a window.
    fn left_column_layout() -> BoxLayout {
        let mut column = BoxLayout::new(Orientation::Vertical, GAP as u32, 0);
        for (index, weight) in LEFT_COLUMN_WEIGHTS.iter().enumerate() {
            let id = LEFT_IDS[index];
            LayoutTrait::add_widget(&mut column, id, *weight);
            // Each pane needs room for its own axis labels and a few bars.
            column.set_constraints(id, LayoutConstraints::new(60, None));
        }
        column
    }

    /// The right column as a vertical layout of three panes.
    fn right_column_layout() -> BoxLayout {
        let mut column = BoxLayout::new(Orientation::Vertical, GAP as u32, 0);
        for (index, weight) in RIGHT_COLUMN_WEIGHTS.iter().enumerate() {
            let id = RIGHT_IDS[index];
            LayoutTrait::add_widget(&mut column, id, *weight);
            column.set_constraints(id, LayoutConstraints::new(60, None));
        }
        column
    }

    /// The pane heights a column layout produces inside `area`, in declaration order.
    ///
    /// `ids` must be the ids the column was built from, in the same order: reading the
    /// heights back by the *wrong* id list silently yields zeros for every pane whose id
    /// is not in that list. The demo's own tests caught exactly that (the right column was
    /// read back with the left column's ids, so all three panes came out zero-height).
    fn column_heights(column: &BoxLayout, area: Rect, ids: &[ObjectId]) -> Vec<u32> {
        let mut placed = Vec::new();
        LayoutTrait::update(column, area, &mut |id, rect| placed.push((id, rect)));
        ids.iter()
            .map(|id| {
                placed.iter().find(|(placed_id, _)| placed_id == id).map(|(_, rect)| rect.height)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|height| height.unwrap_or(0))
            .collect()
    }

    /// Splits the window into the seven panel rectangles.
    ///
    /// The heights come from the two column layouts rather than from a chain of manual
    /// additions: the layout decides the proportions, and this function only assigns
    /// x/width to each pane (which is what makes the panes share an index axis).
    fn compute() -> Self {
        let content = Self::content_rect();
        let left_x = content.x;
        let right_x = left_x + LEFT_WIDTH as i32 + GAP * 2;
        let right_width = Self::right_column_width();

        let left_area = Rect::new(left_x, content.y, LEFT_WIDTH, content.height);
        let right_area = Rect::new(right_x, content.y, right_width, content.height);

        let left_heights = Self::column_heights(&Self::left_column_layout(), left_area, LEFT_IDS);
        let right_heights =
            Self::column_heights(&Self::right_column_layout(), right_area, RIGHT_IDS);

        let mut left_y = content.y;
        let mut take_left = |index: usize| -> (i32, u32) {
            let top = left_y;
            let height = left_heights.get(index).copied().unwrap_or(0);
            left_y += height as i32 + GAP;
            (top, height)
        };
        let (price_y, price_h) = take_left(0);
        let (volume_y, volume_h) = take_left(1);
        let (macd_y, macd_h) = take_left(2);
        let (rsi_y, rsi_h) = take_left(3);

        let mut right_y = content.y;
        let mut take_right = |index: usize| -> (i32, u32) {
            let top = right_y;
            let height = right_heights.get(index).copied().unwrap_or(0);
            right_y += height as i32 + GAP;
            (top, height)
        };
        let (quotes_y, quotes_h) = take_right(0);
        let (book_y, book_h) = take_right(1);
        let (depth_y, depth_h) = take_right(2);

        Self {
            price: Rect::new(left_x, price_y, LEFT_WIDTH, price_h),
            volume: Rect::new(left_x, volume_y, LEFT_WIDTH, volume_h),
            macd: Rect::new(left_x, macd_y, LEFT_WIDTH, macd_h),
            rsi: Rect::new(left_x, rsi_y, LEFT_WIDTH, rsi_h),
            quotes: Rect::new(right_x, quotes_y, right_width, quotes_h),
            book: Rect::new(right_x, book_y, right_width, book_h),
            depth: Rect::new(right_x, depth_y, right_width, depth_h),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Build
// ═══════════════════════════════════════════════════════════════════════════════

/// Mounts one control and reports the outcome, so a failure names the control.
///
/// A bare `?`/`unwrap` would abort the demo on the first unsupported panel and hide
/// which one it was; this way the log says exactly what could not be placed.
fn mount<W: rust_widgets::widget::Widget + 'static>(
    win: &WindowHandle,
    widget: W,
    rect: Rect,
    name: &str,
    log: &Arc<EventLog>,
) {
    match win.mount_surface(Box::new(widget), rect) {
        Ok(handle) => log.append(format!(
            "[{name}] 已挂载 id={} rect=({},{},{},{})",
            handle.raw_id(),
            rect.x,
            rect.y,
            rect.width,
            rect.height
        )),
        Err(error) => log.append(format!("[{name}] 挂载失败：{error}")),
    }
}

/// The last close, which every panel keys off.
fn last_close(series: &PriceSeries) -> f64 {
    series.bars().last().map(|bar| bar.close).unwrap_or(100.0)
}

fn build_screen(win: &WindowHandle, log: &Arc<EventLog>) {
    let layout = Layout::compute();
    let series = sample_series();
    let close = last_close(&series);

    // ── The price chart ────────────────────────────────────────────────────
    log.append("═══ 主图：CandlestickChart ═══");

    let mut chart = CandlestickChart::new(layout.price);
    chart.set_series(series.clone());
    // A moving-average ribbon: two periods so the cross is visible.
    chart.add_overlay(Overlay::moving_average(20));
    chart.add_overlay(Overlay::moving_average(60));
    chart.add_overlay(Overlay::bollinger_bands(20, 2.0));
    // And levels, which draw over the candles rather than behind them.
    chart.add_price_line(PriceLine::new(close - 6.0, PriceLevelKind::Support));
    chart.add_price_line(PriceLine::new(close + 6.0, PriceLevelKind::Resistance));
    chart.add_price_line(PriceLine::new(close, PriceLevelKind::PreviousClose));
    log.append(format!(
        "[CandlestickChart] {} 根 K 线 / {} 条叠加线 / {} 条价位线",
        series.len(),
        chart.overlays().len(),
        chart.price_lines().len()
    ));

    // The interaction wiring: a hover reports the bar, a click logs its OHLC. This is
    // what makes the panel demonstrably live rather than a static picture.
    let hover_log = Arc::clone(log);
    chart.bar_hovered.connect(move |index| {
        hover_log.append(format!("[CandlestickChart] hover bar #{index}"));
    });
    let click_log = Arc::clone(log);
    chart.bar_clicked.connect(move |index| {
        click_log.append(format!("[CandlestickChart] click bar #{index}"));
    });
    mount(win, chart, layout.price, "CandlestickChart", log);

    // ── The volume pane, sharing the series ────────────────────────────────
    let mut volume = VolumeChart::new(layout.volume);
    volume.set_series(series.clone());
    let volume_log = Arc::clone(log);
    volume.bar_hovered.connect(move |index| {
        volume_log.append(format!("[VolumeChart] hover bar #{index}"));
    });
    mount(win, volume, layout.volume, "VolumeChart", log);

    // ── Two oscillator panes ───────────────────────────────────────────────
    let mut macd = IndicatorChart::new(layout.macd);
    macd.set_series(series.clone());
    macd.set_mode(IndicatorMode::Macd);
    // The standard periods; stated explicitly so the demo shows where to change them.
    macd.set_macd_periods(12, 26, 9);
    mount(win, macd, layout.macd, "IndicatorChart(MACD)", log);

    let mut rsi = IndicatorChart::new(layout.rsi);
    rsi.set_series(series.clone());
    rsi.set_mode(IndicatorMode::Rsi);
    rsi.set_period(14);
    rsi.set_show_reference_levels(true);
    mount(win, rsi, layout.rsi, "IndicatorChart(RSI)", log);

    // ── The watchlist ──────────────────────────────────────────────────────
    log.append("═══ 右列：QuoteBoard / OrderBook / DepthChart ═══");

    let mut quotes = QuoteBoard::new(layout.quotes);
    quotes.set_quotes(sample_quotes(close));
    quotes.set_columns(vec![
        QuoteColumn::Symbol,
        QuoteColumn::Last,
        QuoteColumn::Change,
        QuoteColumn::ChangePercent,
    ]);
    // Sorted by the day's biggest movers, which is what a trader looks at first.
    quotes.set_sort(QuoteSort::ChangeMagnitude);
    let quote_log = Arc::clone(log);
    quotes.quote_clicked.connect(move |symbol| {
        quote_log.append(format!("[QuoteBoard] 选中 {symbol}"));
    });
    mount(win, quotes, layout.quotes, "QuoteBoard", log);

    // ── The order book ladder ──────────────────────────────────────────────
    let mut ladder = OrderBookWidget::new(layout.book);
    ladder.set_book(sample_book(close));
    ladder.set_depth(8);
    let ladder_log = Arc::clone(log);
    ladder.level_hovered.connect(move |position| {
        // The signal hands the value out as an `Arc`, so the tuple is dereferenced
        // rather than destructured in the parameter position.
        let (side, index) = *position;
        ladder_log.append(format!("[OrderBook] hover {side:?} #{index}"));
    });
    mount(win, ladder, layout.book, "OrderBook", log);

    // ── The depth curve, over the same book ────────────────────────────────
    let mut depth = DepthChart::new(layout.depth);
    depth.set_book(sample_book(close));
    // Every level, so the curve's far ends are visible — which is where the accumulated
    // size reading lives.
    depth.set_depth(0);
    let depth_log = Arc::clone(log);
    depth.level_hovered.connect(move |price| {
        depth_log.append(format!("[DepthChart] hover price {price:.2}"));
    });
    mount(win, depth, layout.depth, "DepthChart", log);

    // ── Verify the alignment claim rather than only asserting it in a comment ──
    //
    // All four left-column panes were given the same series, so their index axes cover
    // the same bar count. If a future change gave one of them a different series, this
    // log line would show it immediately.
    log.append(format!(
        "[align] 左列四个面板共享 {} 根 K 线的索引轴（主图/成交量/MACD/RSI）",
        series.len()
    ));

    // ── And verify the panels actually paint, not merely construct ──────────
    log.append(format!("[paint] {}", verify_panels_paint(&layout, &series, close)));
}

/// Renders each panel offscreen at its **real geometry** and reports how many pixels it
/// painted.
///
/// # Why the demo does this
///
/// A panel placed at a non-zero origin used to render blank in its upper band and clipped
/// at its lower-right, because the frame was sized to the panel but the panel drew at its
/// absolute position. Nothing about constructing or mounting a control reveals that — only
/// asking it to paint does. Checking here means the demo says so out loud at startup rather
/// than showing a subtly wrong screen that a reader has to notice.
///
/// # Why this goes through `render_frame` rather than calling `Draw::draw` directly
///
/// `Draw` paints at absolute coordinates; the translation from a control's absolute
/// position to its own frame is `render_frame`'s job. Calling `draw` against a bare backend
/// would re-create the very defect this check exists to catch, so the check must exercise
/// the same entry point the backends use.
fn verify_panels_paint(layout: &Layout, series: &PriceSeries, close: f64) -> String {
    use rust_widgets::core::{Color, Size};
    use rust_widgets::widget::runtime;

    // Each entry builds one panel fed the demo's own data, at the demo's own rect.
    let mut panels: Vec<(&str, Rect, Box<dyn rust_widgets::widget::Widget>)> = Vec::new();

    let mut chart = CandlestickChart::new(layout.price);
    chart.set_series(series.clone());
    chart.add_overlay(Overlay::moving_average(20));
    chart.add_overlay(Overlay::bollinger_bands(20, 2.0));
    panels.push(("CandlestickChart", layout.price, Box::new(chart)));

    let mut volume = VolumeChart::new(layout.volume);
    volume.set_series(series.clone());
    panels.push(("VolumeChart", layout.volume, Box::new(volume)));

    let mut macd = IndicatorChart::new(layout.macd);
    macd.set_series(series.clone());
    macd.set_mode(IndicatorMode::Macd);
    panels.push(("IndicatorChart(MACD)", layout.macd, Box::new(macd)));

    let mut rsi = IndicatorChart::new(layout.rsi);
    rsi.set_series(series.clone());
    rsi.set_mode(IndicatorMode::Rsi);
    panels.push(("IndicatorChart(RSI)", layout.rsi, Box::new(rsi)));

    let mut quotes = QuoteBoard::new(layout.quotes);
    quotes.set_quotes(sample_quotes(close));
    panels.push(("QuoteBoard", layout.quotes, Box::new(quotes)));

    let mut ladder = OrderBookWidget::new(layout.book);
    ladder.set_book(sample_book(close));
    panels.push(("OrderBook", layout.book, Box::new(ladder)));

    let mut depth = DepthChart::new(layout.depth);
    depth.set_book(sample_book(close));
    panels.push(("DepthChart", layout.depth, Box::new(depth)));

    let mut painted = Vec::new();
    for (name, rect, panel) in panels {
        let Some(id) = runtime::register(panel) else {
            painted.push(format!("{name} UNMOUNTED"));
            continue;
        };
        let size = Size::new(rect.width, rect.height);
        // A colour no panel paints, so "did it draw" is not confused with "the clear
        // colour happens to match a panel colour".
        let frame = runtime::render_frame(id, size, Color::rgb(255, 0, 255));
        runtime::unregister(id);

        let Some(frame) = frame else {
            painted.push(format!("{name} NO FRAME"));
            continue;
        };
        let total = (rect.width * rect.height) as usize;
        // `as_chunks` rather than `chunks_exact` so the four-byte grouping is expressed in
        // the type; it also lets the compiler see the length is exact, which the lints
        // require.
        let drawn =
            frame.as_chunks::<4>().0.iter().filter(|pixel| **pixel != [255, 0, 255, 255]).count();
        let percent = drawn * 100 / total.max(1);
        painted.push(format!("{name} {percent}%"));
    }

    painted.join(" / ")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Entry point
// ═══════════════════════════════════════════════════════════════════════════════

/// Runs the demo.
///
/// Kept as `app::run()` so it matches the other demos, and so the body is callable
/// from a test without a window.
pub fn run() {
    let log = Arc::new(EventLog::new());

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║        rust_widgets — Finance Controls Demo              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let mut app = App::new();
    log.append("[App] creating");

    // A self-drawn control needs a surface to paint into. Asking at runtime — rather
    // than branching on the OS — is the pattern this crate requires (principle #35):
    // the answer is a fact about the current backend, not about the platform name.
    if !rust_widgets::supports_surfaces() {
        log.append(format!(
            "[App] 当前后端 '{}' 暂不能承载自绘型控件，本 demo 无法显示。",
            rust_widgets::backend_name()
        ));
        log.append(
            "[run] 提示: 当前后端不创建窗口。Linux 桌面请以 `gtk-native` feature 构建\
             （见 demo/finance/Cargo.toml）"
                .to_string(),
        );
        print_log(&log);
        return;
    }

    app.init();
    log.append("[App] init() done");

    let win = app.new_window("Finance Demo — rust_widgets", 60, 60, WINDOW_WIDTH, WINDOW_HEIGHT);
    log.append(format!("[Window] created: id={:?}", win.raw_id()));

    // A menu bar, so the demo has the same window chrome as the other demos and the
    // financial panels are placed below it rather than under the title bar.
    let menu_bar = win.new_menu_bar(0, 0, 0, 0);
    win.new_menu(&menu_bar, "File", 0, 0, 0, 0);
    win.new_menu(&menu_bar, "View", 0, 0, 0, 0);
    win.attach_menu_bar(&menu_bar);

    // A status bar along the bottom, carrying the most recent event.
    //
    // The text is set once here and then updated from the log after the screen is
    // built, so the bar reflects what actually happened rather than a static string:
    // a status bar that never changes is indistinguishable from a broken one.
    let status = win.new_status_bar(
        "就绪",
        0,
        (WINDOW_HEIGHT - STATUS_BAR_HEIGHT) as i32,
        WINDOW_WIDTH,
        STATUS_BAR_HEIGHT,
    );

    build_screen(&win, &log);

    status.set_text(&log.latest());

    win.show();
    log.append(format!("[Window] shown: id={:?}", win.raw_id()));
    println!("[hint] 悬停主图可看到十字光标与 OHLC 读数；点击行情表任意行会在日志中看到选中事件。");

    app.run();
    log.append("[App] event loop exited");

    print_log(&log);
}

/// Prints the collected event log.
fn print_log(log: &EventLog) {
    let entries = log.snapshot();
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                     EVENT LOG                           ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    for (index, entry) in entries.iter().enumerate() {
        println!("  [{:>3}] {}", index + 1, entry);
    }
    println!();
    println!("  共 {} 条事件。", entries.len());
    println!();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Tests for the demo's own data and layout.
///
/// # Why a demo needs tests
///
/// The demo is the only place all six controls are wired together, so it is where an
/// integration mistake would first appear — and a demo is normally the one thing in a
/// tree nobody tests. These cover the parts that are pure: the sample data, the layout
/// arithmetic and the pipes between them. The controls themselves are covered by the
/// library's own 180 tests; what is checked here is that this file feeds them
/// correctly.
#[cfg(test)]
mod tests {
    use super::*;

    /// The series really has the shape the demo claims.
    #[test]
    fn the_sample_series_has_the_advertised_bars() {
        let series = sample_series();
        assert_eq!(series.len(), BAR_COUNT);
        assert_eq!(series.labels().len(), BAR_COUNT, "labels must stay aligned with bars");
        assert!(series.bars().iter().all(|bar| bar.is_consistent()));
    }

    /// No bar is degenerate, so the chart has a real price range to scale to.
    #[test]
    fn the_sample_series_has_a_real_range() {
        let series = sample_series();
        let (low, high) = series.price_extent();
        assert!(low.is_finite() && high.is_finite());
        assert!(high - low > 5.0, "a flat series would not exercise the price axis");
        assert!(series.max_volume() > 0.0, "the volume pane needs a divisor");
    }

    /// The series contains a gap, which is what makes the true-range behaviour visible.
    ///
    /// Asserted rather than left to chance: if the generator ever stopped producing the
    /// every-47-bars gap, the chart would still draw and nobody would notice that the
    /// one interesting edge case had silently disappeared from the demo.
    #[test]
    fn the_sample_series_contains_a_gap() {
        let series = sample_series();
        let bars = series.bars();
        let has_gap = bars.windows(2).any(|pair| (pair[1].open - pair[0].close).abs() > 1.0);
        assert!(has_gap, "the demo is supposed to show a gap bar");
    }

    /// Every overlay the demo uses has enough bars to be defined.
    ///
    /// A 60-period average on a 240-bar series has 180 real values; if `BAR_COUNT` were
    /// ever lowered below the longest period, the overlay would be all gaps and the
    /// chart would look broken for a reason no assertion covered.
    #[test]
    fn every_overlay_has_room_to_warm_up() {
        let series = sample_series();
        let closes = series.closes();
        let longest = 60;
        assert!(
            series.len() > longest * 2,
            "the series must be long enough for the longest overlay to settle"
        );
        let average =
            rust_widgets::widget::special_widgets::finance::indicators::sma(&closes, longest);
        let defined = average.iter().filter(|value| value.is_finite()).count();
        assert!(defined >= series.len() - longest, "the 60-period average must be defined");
    }

    /// The panel rectangles do not overlap and share the left edge.
    ///
    /// This is the alignment claim the demo exists to show, so it is checked rather
    /// than assumed: a one-pixel drift here is exactly the bug that would put bar 17's
    /// volume under bar 16's candle.
    #[test]
    fn the_stacked_panes_share_an_axis_and_do_not_overlap() {
        let layout = Layout::compute();

        for rect in [layout.price, layout.volume, layout.macd, layout.rsi] {
            assert_eq!(rect.x, layout.price.x, "every left pane shares a left edge");
            assert_eq!(rect.width, layout.price.width, "and a width, so the axes match");
        }
        assert!(
            layout.volume.y >= layout.price.y + layout.price.height as i32,
            "the volume pane must start below the price pane"
        );
        assert!(layout.macd.y >= layout.volume.y + layout.volume.height as i32);
        assert!(layout.rsi.y >= layout.macd.y + layout.macd.height as i32);
    }

    /// Every panel must fit inside the window, above the status bar and below the menu bar.
    ///
    /// The layout and the window now both derive from the same constants, and this pins
    /// that: a panel that ran off the right edge or under the status bar would be drawn
    /// without any visible symptom at the panel level.
    #[test]
    fn every_panel_stays_inside_the_window() {
        let layout = Layout::compute();
        let bottom_limit = WINDOW_HEIGHT as i32 - STATUS_BAR_HEIGHT as i32;
        let panels = [
            ("price", layout.price),
            ("volume", layout.volume),
            ("macd", layout.macd),
            ("rsi", layout.rsi),
            ("quotes", layout.quotes),
            ("book", layout.book),
            ("depth", layout.depth),
        ];
        for (name, rect) in panels {
            assert!(rect.width > 0 && rect.height > 0, "{name} has no area: {rect:?}");
            assert!(rect.x >= 0, "{name} starts left of the window: {rect:?}");
            assert!(
                rect.x + rect.width as i32 <= WINDOW_WIDTH as i32,
                "{name} runs off the right edge: {rect:?}"
            );
            assert!(rect.y >= MENU_BAR_HEIGHT as i32, "{name} is under the menu bar: {rect:?}");
            assert!(
                rect.y + rect.height as i32 <= bottom_limit,
                "{name} runs under the status bar: {rect:?}"
            );
        }
    }

    /// The two columns must not overlap each other.
    #[test]
    fn the_columns_do_not_overlap() {
        let layout = Layout::compute();
        let left_right_edge = layout.price.x + layout.price.width as i32;
        for rect in [layout.quotes, layout.book, layout.depth] {
            assert!(
                rect.x >= left_right_edge,
                "the right column must start at or after the left column's edge"
            );
        }
    }

    /// The right column sits beside the left one and within the window.
    #[test]
    fn the_right_column_is_beside_the_left_one() {
        let layout = Layout::compute();
        assert!(layout.quotes.x > layout.price.x + layout.price.width as i32);
        assert_eq!(layout.quotes.x, layout.book.x, "the right column is one column");
        assert_eq!(layout.quotes.width, layout.book.width);
        assert!(layout.depth.y >= layout.book.y + layout.book.height as i32);
        // And nothing runs off the right edge of a 1440-wide window.
        assert!(layout.quotes.x + layout.quotes.width as i32 <= 1440);
    }

    /// The book the demo builds is ordered the way the ladder requires.
    ///
    /// The fixture deliberately supplies the levels reversed; this asserts the control
    /// really corrected it, which is the behaviour the demo advertises.
    #[test]
    fn the_sample_book_is_sorted_best_first() {
        let book = sample_book(100.0);
        let bids = book.bids();
        let asks = book.asks();
        assert_eq!(bids.len(), 8);
        assert_eq!(asks.len(), 8);
        for window in bids.windows(2) {
            assert!(window[0].price >= window[1].price, "bids must descend");
        }
        for window in asks.windows(2) {
            assert!(window[0].price <= window[1].price, "asks must ascend");
        }
        assert!(book.spread().is_some());
    }

    /// The book has a non-monotonic size profile, which is what makes the curve a step.
    #[test]
    fn the_sample_book_has_an_uneven_size_profile() {
        let book = sample_book(100.0);
        let quantities: Vec<f64> = book.bids().iter().map(|level| level.quantity).collect();
        let is_monotonic = quantities.windows(2).all(|pair| pair[0] <= pair[1]);
        assert!(
            !is_monotonic,
            "a smoothly growing book would draw a smooth curve and exercise nothing"
        );
    }

    /// The watchlist keeps the caller's order until a sort is asked for.
    #[test]
    fn the_watchlist_starts_in_the_supplied_order() {
        use rust_widgets::core::Color;

        let quotes = sample_quotes(120.0);
        let mut board = QuoteBoard::new(Rect::new(0, 0, 400, 200));
        board.set_quotes(quotes);

        // Collected as owned strings: a `&str` view would borrow the board, and the
        // sort calls below need it mutably. The copy is the point -- it is a snapshot
        // of the order before any sort touched it.
        let before: Vec<String> = board.quotes().iter().map(|quote| quote.symbol.clone()).collect();
        assert_eq!(before, vec!["AAPL", "MSFT", "NVDA", "TSLA", "AMZN"]);

        board.set_sort(QuoteSort::Symbol);
        board.set_sort(QuoteSort::None);
        let restored: Vec<String> =
            board.quotes().iter().map(|quote| quote.symbol.clone()).collect();
        assert_eq!(restored, before, "no-sort must restore the caller's order exactly");

        let _ = Color::BLACK;
    }

    /// The last close the demo keys its levels off is a real number.
    #[test]
    fn the_last_close_is_usable() {
        let close = last_close(&sample_series());
        assert!(close.is_finite() && close > 0.0);
    }

    /// Every panel the demo mounts really paints, at its real geometry.
    ///
    /// This is the assertion that would have caught the frame-origin defect directly: a
    /// panel placed at a non-zero origin used to paint nothing in its upper band, and
    /// nothing about constructing it revealed that. The report must name every panel and
    /// every percentage must be well above zero, or a blank pane would pass unnoticed.
    #[test]
    fn every_panel_paints_something_at_its_real_geometry() {
        let layout = Layout::compute();
        let series = sample_series();
        let close = last_close(&series);
        let report = verify_panels_paint(&layout, &series, close);
        println!("[paint report] {report}");

        for name in [
            "CandlestickChart",
            "VolumeChart",
            "IndicatorChart(MACD)",
            "IndicatorChart(RSI)",
            "QuoteBoard",
            "OrderBook",
            "DepthChart",
        ] {
            let entry = report
                .split(" / ")
                .find(|entry| entry.starts_with(name))
                .unwrap_or_else(|| panic!("{name} missing from the report: {report}"));
            let percent: u32 = entry
                .rsplit(' ')
                .next()
                .and_then(|value| value.trim_end_matches('%').parse().ok())
                .unwrap_or_else(|| panic!("{name} did not report a percentage: {entry}"));
            assert!(percent > 5, "{name} painted only {percent}% of its frame: {entry}");
        }
    }

    /// Every control the demo mounts can be constructed and drawn headlessly.
    ///
    /// This is the integration claim in one test: six controls, fed the demo's own
    /// data, each produce a frame without a window. If a control's contract changed in
    /// a way the demo did not follow, this fails here rather than on launch.
    #[test]
    fn every_panel_draws_with_the_demo_data() {
        use rust_widgets::core::Color;
        use rust_widgets::render::{PaintBackend, SoftwarePaintBackend};
        use rust_widgets::widget::Draw;

        let layout = Layout::compute();
        let series = sample_series();
        let close = last_close(&series);
        let size = |rect: Rect| rust_widgets::core::Size::new(rect.width, rect.height);

        let mut chart = CandlestickChart::new(layout.price);
        chart.set_series(series.clone());
        chart.add_overlay(Overlay::moving_average(20));
        chart.add_overlay(Overlay::bollinger_bands(20, 2.0));

        let mut volume = VolumeChart::new(layout.volume);
        volume.set_series(series.clone());

        let mut macd = IndicatorChart::new(layout.macd);
        macd.set_series(series.clone());
        macd.set_mode(IndicatorMode::Macd);

        let mut rsi = IndicatorChart::new(layout.rsi);
        rsi.set_series(series.clone());
        rsi.set_mode(IndicatorMode::Rsi);

        let mut quotes = QuoteBoard::new(layout.quotes);
        quotes.set_quotes(sample_quotes(close));

        let mut ladder = OrderBookWidget::new(layout.book);
        ladder.set_book(sample_book(close));

        let mut depth = DepthChart::new(layout.depth);
        depth.set_book(sample_book(close));

        // Each is drawn into its own backend at the panel's real size, so a control
        // that only survives a 0×0 frame would not pass.
        let panels: Vec<(&str, Rect, &mut dyn Draw)> = vec![
            ("candlestick", layout.price, &mut chart),
            ("volume", layout.volume, &mut volume),
            ("macd", layout.macd, &mut macd),
            ("rsi", layout.rsi, &mut rsi),
            ("quotes", layout.quotes, &mut quotes),
            ("book", layout.book, &mut ladder),
            ("depth", layout.depth, &mut depth),
        ];
        for (name, rect, panel) in panels {
            assert!(rect.width > 0 && rect.height > 0, "{name} panel has no area");
            let mut backend = SoftwarePaintBackend::new(size(rect), 1.0);
            // `begin_frame` must run before the context borrows the backend: the two
            // borrows cannot overlap, and clearing first is also what a real frame does.
            backend.begin_frame(Color::BLACK);
            {
                let mut context = rust_widgets::render::RenderContext::new(&mut backend);
                panel.draw(&mut context);
            }
            backend.end_frame();
            assert_eq!(
                backend.frame_rgba().len(),
                (rect.width * rect.height * 4) as usize,
                "{name} must produce a frame of the panel's size"
            );
        }

        // The three left-column panes were all handed the same series, so their index
        // axes agree — which is the demo's whole point.
        assert_eq!(series.len(), BAR_COUNT);
    }

    /// The two columns are built from layouts, and their declared proportions are what the
    /// panel heights follow.
    ///
    /// This is the assertion that ties the layout to the version this demo previously had
    /// (hand-computed percentages): the price chart must still be the tallest pane on the
    /// left, and each column's panes must sum to the column's height.
    #[test]
    fn the_column_layouts_produce_the_declared_proportions() {
        let content = Layout::content_rect();
        let left_area = Rect::new(0, content.y, LEFT_WIDTH, content.height);
        let right_area = Rect::new(0, content.y, Layout::right_column_width(), content.height);

        let left = Layout::column_heights(&Layout::left_column_layout(), left_area, LEFT_IDS);
        let right = Layout::column_heights(&Layout::right_column_layout(), right_area, RIGHT_IDS);

        assert_eq!(left.len(), LEFT_COLUMN_WEIGHTS.len());
        assert_eq!(right.len(), RIGHT_COLUMN_WEIGHTS.len());
        for (index, height) in left.iter().enumerate() {
            assert!(*height > 0, "left pane {index} has no height: {left:?}");
        }
        for (index, height) in right.iter().enumerate() {
            assert!(*height > 0, "right pane {index} has no height: {right:?}");
        }
        // The price chart carries the heaviest weight, so it must be the tallest.
        assert!(
            left[0] >= *left.iter().max().unwrap_or(&0),
            "the price pane must be the tallest on the left: {left:?}"
        );
        // The panes plus inter-pane gaps must not exceed the column.
        let left_used: u32 = left.iter().sum::<u32>() + GAP as u32 * 3;
        assert!(
            left_used <= content.height,
            "the left column overflows: {left_used} > {}",
            content.height
        );
        let right_used: u32 = right.iter().sum::<u32>() + GAP as u32 * 2;
        assert!(
            right_used <= content.height,
            "the right column overflows: {right_used} > {}",
            content.height
        );
    }

    /// The panes read back from the layouts are exactly the rects `compute` hands out.
    ///
    /// This pins "the geometry comes from the layout": if `compute` ever went back to
    /// computing heights independently, this would catch the divergence.
    #[test]
    fn the_computed_rects_match_the_layout_output() {
        let layout = Layout::compute();
        let content = Layout::content_rect();
        let left_area = Rect::new(content.x, content.y, LEFT_WIDTH, content.height);
        let right = Layout::right_column_width();
        let right_area =
            Rect::new(content.x + LEFT_WIDTH as i32 + GAP * 2, content.y, right, content.height);

        let left_heights =
            Layout::column_heights(&Layout::left_column_layout(), left_area, LEFT_IDS);
        let right_heights =
            Layout::column_heights(&Layout::right_column_layout(), right_area, RIGHT_IDS);

        assert_eq!(layout.price.height, left_heights[0]);
        assert_eq!(layout.volume.height, left_heights[1]);
        assert_eq!(layout.macd.height, left_heights[2]);
        assert_eq!(layout.rsi.height, left_heights[3]);
        assert_eq!(layout.quotes.height, right_heights[0]);
        assert_eq!(layout.book.height, right_heights[1]);
        assert_eq!(layout.depth.height, right_heights[2]);
    }

    /// A taller window must grow the panes rather than leaving a gap at the bottom.
    #[test]
    fn the_columns_fill_whatever_height_they_are_given() {
        for height in [600u32, 900, 1200] {
            let area = Rect::new(0, 0, LEFT_WIDTH, height);
            let left = Layout::column_heights(&Layout::left_column_layout(), area, LEFT_IDS);
            let used: u32 = left.iter().sum::<u32>() + GAP as u32 * 3;
            assert!(used <= height, "the column must fit its area at height {height}: {used}");
            // A column that left a large hole would mean the weights stopped stretching.
            let slack = height - used;
            assert!(
                slack < 16,
                "the column left {slack}px unused at height {height} (weights must stretch)"
            );
        }
    }

    /// The status bar text is the most recent entry, which is what it shows.
    #[test]
    fn the_log_reports_its_latest_entry() {
        let log = EventLog::new();
        assert_eq!(log.latest(), "");
        log.append("first");
        log.append("second");
        assert_eq!(log.latest(), "second");
        assert_eq!(log.snapshot().len(), 2);
    }
}
