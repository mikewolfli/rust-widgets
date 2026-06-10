# 圖表與資料視覺化 (Charts & Data Visualization)

rust-widgets 包含一個內建的圖表系統，提供五種圖表類型、可插拔的繪圖後端、SVG 匯出以及 widget 整合——全部透過同一個渲染管線，無需外部圖表相依套件。

---

## 1. 資料型別

### `DataPoint`

```rust
use rust_widgets::chart::DataPoint;

let point = DataPoint {
    x: 1.0,
    y: 42.5,
    label: Some("Q1".into()),
};

// 不含標籤
let simple = DataPoint {
    x: 2.0,
    y: 58.0,
    label: None,
};
```

| 欄位 | 型別 | 說明 |
|-------|------|-------------|
| `x` | `f64` | 資料域的 x 座標 |
| `y` | `f64` | 資料域的 y 座標 |
| `label` | `Option<String>` | 用於圖例/工具提示的可選標籤 |

### `ChartSeries`

```rust
use rust_widgets::chart::{ChartSeries, DataPoint};
use rust_widgets::core::Color;

let series = ChartSeries {
    name: "Revenue".into(),
    data: vec![
        DataPoint { x: 2020.0, y: 100.0, label: None },
        DataPoint { x: 2021.0, y: 150.0, label: None },
        DataPoint { x: 2022.0, y: 200.0, label: None },
        DataPoint { x: 2023.0, y: 180.0, label: None },
        DataPoint { x: 2024.0, y: 250.0, label: None },
    ],
    color: Color { r: 66, g: 133, b: 244, a: 255 },
    visible: true,
};
```

| 欄位 | 型別 | 說明 |
|-------|------|-------------|
| `name` | `String` | 數列顯示名稱 (圖例) |
| `data` | `Vec<DataPoint>` | 有序的資料點 |
| `color` | `Color` | 數列繪圖顏色 |
| `visible` | `bool` | 切換顯示以進行篩選 |

---

## 2. `ChartType` 列舉

```rust
pub enum ChartType {
    Line,     // 折線圖
    Bar,      // 垂直長條圖
    Pie,      // 圓餅圖
    Scatter,  // 散佈圖
    Area,     // 區域圖
}
```

### 工廠方法

```rust
use rust_widgets::chart::ChartType;

// 從型別變體建立 boxed chart
let chart: Box<dyn Chart> = ChartType::Line.create_chart();
let bar_chart = ChartType::Bar.create_chart();
let pie_chart = ChartType::Pie.create_chart();
let scatter_chart = ChartType::Scatter.create_chart();
let area_chart = ChartType::Area.create_chart();
```

---

## 3. `Chart` 特徵 (Trait)

```rust
pub trait Chart {
    fn add_series(&mut self, series: ChartSeries);
    fn remove_series(&mut self, name: &str);
    fn clear_series(&mut self);
    fn set_title(&mut self, title: String);
    fn set_x_axis_label(&mut self, label: String);
    fn set_y_axis_label(&mut self, label: String);
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext);
}
```

每種圖表型別都實作了此特徵，使它們可以互換使用：

```rust
use rust_widgets::chart::{Chart, ChartType, ChartSeries};
use rust_widgets::core::Rect;

let mut chart = ChartType::Line.create_chart();
chart.set_title("Quarterly Revenue".into());
chart.set_x_axis_label("Quarter".into());
chart.set_y_axis_label("Revenue ($K)".into());
chart.add_series(revenue_series);

// 繪製到任何 ChartContext
chart.draw(Rect::new(0, 0, 800, 400), &mut context);
```

---

## 4. `ChartContext` 特徵 (14 個繪圖基本單元)

`ChartContext` 是可插拔的繪圖後端。提供了兩種實作：`SvgChartContext` 用於向量輸出，以及 `MemoryChartContext` 用於測試。

```rust
pub trait ChartContext {
    fn draw_line(&mut self, from: Point, to: Point, width: f32, color: Color);
    fn draw_rect(&mut self, rect: Rect, color: Color);
    fn draw_text(&mut self, text: &str, pos: Point, font_size: f32, color: Color);
    fn draw_circle(&mut self, center: Point, radius: f32, color: Color);
    fn draw_polygon(&mut self, points: &[Point], color: Color);
    fn draw_path_segment(&mut self, start: Point, end: Point, width: f32, color: Color);
    fn draw_arc(&mut self, center: Point, radius: f32, start_angle: f64, end_angle: f64, color: Color);
    fn draw_path(&mut self, points: &[Point], width: f32, color: Color);
    fn draw_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color);
    fn set_fill_color(&mut self, color: Color);
    fn set_stroke_color(&mut self, color: Color);
}
```

| # | 方法 | 說明 |
|---|--------|-------------|
| 1 | `draw_line` | 從起點到終點的線段，含寬度 |
| 2 | `draw_rect` | 在指定矩形內填滿顏色 |
| 3 | `draw_text` | 在指定位置繪製文字，含字型大小與顏色 |
| 4 | `draw_circle` | 以中心點與半徑繪製填滿圓形 |
| 5 | `draw_polygon` | 從點清單繪製填滿多邊形 |
| 6 | `draw_path_segment` | 單一路徑段 (線條) |
| 7 | `draw_arc` | 透過多邊形段逼近的弧形 |
| 8 | `draw_path` | 多段 SVG 路徑 |
| 9 | `draw_ellipse` | 填滿橢圓形 |
| 10 | `set_fill_color` | 設定後續繪圖的填滿顏色 |
| 11 | `set_stroke_color` | 設定後續繪圖的筆畫顏色 |

---

## 5. 具體圖表實作

### 折線圖 (LineChart)

```rust
use rust_widgets::chart::{LineChart, Chart, ChartSeries, DataPoint};
use rust_widgets::core::{Color, Rect};
use rust_widgets::chart::svg::MemoryChartContext;

let mut chart = LineChart::new();
chart.set_title("Temperature Trend".into());
chart.set_x_axis_label("Day".into());
chart.set_y_axis_label("°C".into());
chart.set_x_tick_count(7);       // x 軸 7 個刻度 (限制在 2-16)
chart.set_y_tick_count(5);       // y 軸 5 個刻度 (限制在 2-16)
chart.set_grid_enabled(true);    // 啟用格線

chart.add_series(ChartSeries {
    name: "High".into(),
    data: vec![
        DataPoint { x: 1.0, y: 22.0, label: None },
        DataPoint { x: 2.0, y: 24.0, label: None },
        DataPoint { x: 3.0, y: 19.0, label: None },
        DataPoint { x: 4.0, y: 26.0, label: None },
        DataPoint { x: 5.0, y: 28.0, label: None },
    ],
    color: Color { r: 234, g: 67, b: 53, a: 255 },
    visible: true,
});

// 繪製到記憶體上下文
let mut ctx = MemoryChartContext::default();
chart.draw(Rect::new(0, 0, 800, 400), &mut ctx);
println!("Commands: {:?}", ctx.commands);
```

### 長條圖 (BarChart)

```rust
use rust_widgets::chart::BarChart;

let mut chart = BarChart::new();
chart.set_title("Monthly Sales".into());
chart.set_x_axis_label("Month".into());
chart.set_y_axis_label("Units".into());

chart.add_series(ChartSeries {
    name: "Product A".into(),
    data: vec![
        DataPoint { x: 1.0, y: 120.0, label: None },
        DataPoint { x: 2.0, y: 90.0, label: None },
        DataPoint { x: 3.0, y: 150.0, label: None },
        DataPoint { x: 4.0, y: 80.0, label: None },
        DataPoint { x: 5.0, y: 200.0, label: None },
    ],
    color: Color { r: 52, g: 168, b: 83, a: 255 },
    visible: true,
});

chart.add_series(ChartSeries {
    name: "Product B".into(),
    data: vec![
        DataPoint { x: 1.0, y: 80.0, label: None },
        DataPoint { x: 2.0, y: 110.0, label: None },
        DataPoint { x: 3.0, y: 70.0, label: None },
        DataPoint { x: 4.0, y: 140.0, label: None },
        DataPoint { x: 5.0, y: 95.0, label: None },
    ],
    color: Color { r: 251, g: 188, b: 4, a: 255 },
    visible: true,
});
```

### 圓餅圖 (PieChart)

```rust
use rust_widgets::chart::PieChart;

let mut chart = PieChart::new();
chart.set_title("Market Share".into());

chart.add_series(ChartSeries {
    name: "Browser".into(),
    data: vec![DataPoint { x: 0.0, y: 45.0, label: Some("Chrome".into()) }],
    color: Color { r: 66, g: 133, b: 244, a: 255 },
    visible: true,
});

chart.add_series(ChartSeries {
    name: "Browser".into(),
    data: vec![DataPoint { x: 0.0, y: 30.0, label: Some("Safari".into()) }],
    color: Color { r: 52, g: 168, b: 83, a: 255 },
    visible: true,
});

chart.add_series(ChartSeries {
    name: "Browser".into(),
    data: vec![DataPoint { x: 0.0, y: 25.0, label: Some("Firefox".into()) }],
    color: Color { r: 251, g: 188, b: 4, a: 255 },
    visible: true,
});
```

### 散佈圖與區域圖 (ScatterChart & AreaChart)

```rust
use rust_widgets::chart::{ScatterChart, AreaChart};

// 散佈圖 — 個別點
let mut scatter = ScatterChart::new();
scatter.set_title("User Engagement".into());
scatter.set_x_axis_label("Sessions".into());
scatter.set_y_axis_label("Time (min)".into());
scatter.add_series(engagement_data);

// 區域圖 — 線條下方的填滿區域
let mut area = AreaChart::new();
area.set_title("Cumulative Downloads".into());
area.set_x_axis_label("Month".into());
area.set_y_axis_label("Downloads".into());
area.add_series(download_data);
```

---

## 6. 圖表功能

### 軸標籤、刻度與格線

```rust
let mut chart = LineChart::new();

// 軸標籤
chart.set_title("Performance Metrics".into());
chart.set_x_axis_label("Iteration".into());
chart.set_y_axis_label("Throughput (req/s)".into());

// 刻度設定 (限制在 2-16)
chart.set_x_tick_count(10);
chart.set_y_tick_count(8);

// 格線
chart.set_grid_enabled(true);
```

### 圖例

圖例會根據可見的數列自動繪製。圖例位置由 `compute_cartesian_layout()` 計算，當有圖例項目時會在右側保留 170px。

```rust
// 所有可見數列會自動顯示圖例。
// 每個數列會獲得一條彩色線條 + 圖例區域中的名稱。
// 超出顯示範圍時會顯示 "+N more"。
```

### 顏色調色板

顏色由每個 `ChartSeries` 使用者定義。一個常見的調色板：

```rust
const PALETTE: [Color; 6] = [
    Color { r: 66,  g: 133, b: 244, a: 255 },  // 藍色
    Color { r: 234, g: 67,  b: 53,  a: 255 },  // 紅色
    Color { r: 52,  g: 168, b: 83,  a: 255 },  // 綠色
    Color { r: 251, g: 188, b: 4,   a: 255 },  // 黃色
    Color { r: 171, g: 71,  b: 188, a: 255 },  // 紫色
    Color { r: 0,   g: 172, b: 193, a: 255 },  // 青色
];
```

---

## 7. SVG 圖表匯出

`SvgChartContext` 將圖表渲染為 SVG，用於匯出、嵌入或像素精確的驗證。

### 渲染為 SVG 檔案

```rust
use rust_widgets::chart::{Chart, ChartType};
use rust_widgets::chart::svg::render_chart_to_svg_file;
use rust_widgets::core::Rect;

let mut chart = ChartType::Line.create_chart();
chart.set_title("Export Demo".into());
chart.add_series(my_series);

// 渲染並儲存為檔案
render_chart_to_svg_file(
    chart.as_ref(),
    Rect::new(0, 0, 800, 400),
    "chart_output.svg",
).expect("Failed to save SVG");
```

### 渲染為 SVG 字串

```rust
use rust_widgets::chart::svg::SvgChartContext;
use rust_widgets::core::Rect;

let mut ctx = SvgChartContext::new(800, 400);
chart.draw(Rect::new(0, 0, 800, 400), &mut ctx);

let svg_string = ctx.to_svg_string();
println!("{}", svg_string);

// 或直接儲存
ctx.save("chart.svg").unwrap();
```

### SVG 輸出格式

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="400" viewBox="0 0 800 400">
  <rect x="0" y="0" width="100" height="50" fill="#4285F4" fill-opacity="1.000" stroke="none" />
  <line x1="10" y1="20" x2="100" y2="20" stroke="#EA4335" stroke-opacity="1.000" stroke-width="2" />
  <text x="8" y="16" fill="#141414" fill-opacity="1.000" font-size="14" font-family="sans-serif">Title</text>
  <!-- ... -->
</svg>
```

顏色以 `#RRGGBB` 十六進位格式輸出，透明度以 `fill-opacity`/`stroke-opacity` 表示。文字會進行 XML 跳脫（`&`、`<`、`>`）。

---

## 8. `MemoryChartContext` 用於測試

`MemoryChartContext` 將繪圖指令記錄為字串，無需實際渲染即可進行測試斷言：

```rust
use rust_widgets::chart::svg::MemoryChartContext;
use rust_widgets::chart::{Chart, ChartType, ChartSeries, DataPoint};
use rust_widgets::core::{Color, Rect};

let mut chart = ChartType::Line.create_chart();
chart.add_series(ChartSeries {
    name: "Test".into(),
    data: vec![
        DataPoint { x: 0.0, y: 0.0, label: None },
        DataPoint { x: 1.0, y: 10.0, label: None },
        DataPoint { x: 2.0, y: 20.0, label: None },
    ],
    color: Color { r: 255, g: 0, b: 0, a: 255 },
    visible: true,
});

let mut ctx = MemoryChartContext::default();
chart.draw(Rect::new(0, 0, 400, 300), &mut ctx);

// 對記錄的指令進行斷言
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("rect:")));
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("text:")));
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("line:")));
```

### 記錄的指令格式

| 指令 | 格式 |
|---------|--------|
| `rect` | `rect:x,y,width,height` |
| `line` | `line:x1,y1->x2,y2:width` |
| `text` | `text:content@x,y:font_size` |
| `circle` | `circle:cx,cy:radius` |
| `polygon` | `polygon:[x1,y1 x2,y2 ...]` |
| `path` | `path:[x1,y1 x2,y2 ...]:width` |
| `arc` | `arc:cx,cy:radius:start°->end°` |
| `ellipse` | `ellipse:cx,cy:rx×ry` |
| `set_fill` | `set_fill_color:#RRGGBB` |
| `set_stroke` | `set_stroke_color:#RRGGBB` |

---

## 9. 笛卡兒佈局計算

`compute_cartesian_layout()` 從給定的邊界矩形計算繪圖區域、邊距和圖例位置：

```rust
use rust_widgets::chart::charts::compute_cartesian_layout;
use rust_widgets::core::Rect;

let rect = Rect::new(0, 0, 800, 400);
let layout = compute_cartesian_layout(rect, true, true, 3);

println!("Plot area: ({}, {}) → {}×{}",
    layout.plot_x, layout.plot_y, layout.plot_w, layout.plot_h);
println!("Legend at: ({}, {})", layout.legend_x, layout.legend_y);

// 邊距規則：
//   左：    48px (有 y 軸標籤時為 64px)
//   上：    32px
//   下：    36px (有 x 軸標籤時為 52px)
//   右：    24px (有圖例項目時為 170px)
```

### 軸與刻度繪製函式

```rust
use rust_widgets::chart::charts::{
    draw_cartesian_axes,
    draw_x_ticks,
    draw_y_ticks,
    draw_legend,
};

// 繪製軸
draw_cartesian_axes(&mut ctx, &layout);

// 繪製刻度標記與標籤
draw_x_ticks(&mut ctx, &layout, min_x, max_x, 5, true);  // 含格線
draw_y_ticks(&mut ctx, &layout, min_y, max_y, 5, true);

// 繪製圖例
draw_legend(&mut ctx, &layout, &visible_series);
```

---

## 10. `ChartLayout` 用於 Widget 整合

`ChartLayout` 將圖表 widget 定位以填滿容器中的可用空間：

```rust
use rust_widgets::chart::layout::ChartLayout;
use rust_widgets::layout::Layout;
use rust_widgets::core::Rect;

let mut layout = ChartLayout::new(chart_widget_id);

// 檢查子元件
assert!(layout.has_child(chart_widget_id));
assert_eq!(layout.child_ids(), vec![chart_widget_id]);

// 替換子元件
layout.add_widget(new_chart_id, 0);

// 佈局填滿整個矩形
layout.update(Rect::new(0, 0, 800, 600), &mut |id, rect| {
    // 將 chart_widget 定位到 rect
    println!("Place widget {} at {:?}", id, rect);
});

// 移除與清除
layout.remove_widget(chart_widget_id);
layout.clear();
```

`ChartLayout` 實作了完整的 `Layout` 特徵：

| 方法 | 行為 |
|--------|----------|
| `add_widget` | 儲存單一子元件 (取代前一個) |
| `remove_widget` | 如果符合則移除子元件 |
| `update` | 將子元件定位以填滿整個矩形 |
| `child_ids` | 回傳單一子元件的 ID |
| `has_child` | 檢查給定 ID 是否被管理 |
| `clear` | 移除所有子元件 |

---

## 11. 完整的圖表範例與真實資料

```rust
use rust_widgets::chart::{
    Chart, ChartType, ChartSeries, DataPoint,
};
use rust_widgets::chart::svg::{SvgChartContext, MemoryChartContext};
use rust_widgets::core::{Color, Rect};

fn main() {
    // 股價資料
    let stock_data = vec![
        DataPoint { x: 1.0, y: 150.0, label: None },
        DataPoint { x: 2.0, y: 152.0, label: None },
        DataPoint { x: 3.0, y: 148.0, label: None },
        DataPoint { x: 4.0, y: 155.0, label: None },
        DataPoint { x: 5.0, y: 160.0, label: None },
        DataPoint { x: 6.0, y: 158.0, label: None },
        DataPoint { x: 7.0, y: 163.0, label: None },
    ];

    // 成交量資料 (長條)
    let volume_data = vec![
        DataPoint { x: 1.0, y: 1000.0, label: None },
        DataPoint { x: 2.0, y: 1200.0, label: None },
        DataPoint { x: 3.0, y: 800.0, label: None },
        DataPoint { x: 4.0, y: 1500.0, label: None },
        DataPoint { x: 5.0, y: 2000.0, label: None },
        DataPoint { x: 6.0, y: 1300.0, label: None },
        DataPoint { x: 7.0, y: 1800.0, label: None },
    ];

    // 為價格建立折線圖
    let mut price_chart = ChartType::Line.create_chart();
    price_chart.set_title("Stock Price (7-Day)".into());
    price_chart.set_x_axis_label("Day".into());
    price_chart.set_y_axis_label("Price ($)".into());
    price_chart.set_x_tick_count(7);
    price_chart.set_y_tick_count(5);
    price_chart.set_grid_enabled(true);

    price_chart.add_series(ChartSeries {
        name: "AAPL".into(),
        data: stock_data,
        color: Color { r: 66, g: 133, b: 244, a: 255 },
        visible: true,
    });

    // 使用 MemoryChartContext 測試
    let mut mem_ctx = MemoryChartContext::default();
    price_chart.draw(Rect::new(0, 0, 800, 400), &mut mem_ctx);

    assert!(!mem_ctx.commands.is_empty());
    println!("Generated {} draw commands", mem_ctx.commands.len());

    // 匯出為 SVG
    let mut svg_ctx = SvgChartContext::new(800, 400);
    price_chart.draw(Rect::new(0, 0, 800, 400), &mut svg_ctx);
    svg_ctx.save("stock_price.svg").expect("Failed to save SVG");
    println!("SVG saved to stock_price.svg");

    // 為成交量建立長條圖
    let mut volume_chart = ChartType::Bar.create_chart();
    volume_chart.set_title("Trading Volume (7-Day)".into());
    volume_chart.set_x_axis_label("Day".into());
    volume_chart.set_y_axis_label("Volume".into());

    volume_chart.add_series(ChartSeries {
        name: "Volume".into(),
        data: volume_data,
        color: Color { r: 52, g: 168, b: 83, a: 255 },
        visible: true,
    });

    // 匯出成交量圖表為 SVG
    let mut svg_ctx2 = SvgChartContext::new(800, 400);
    volume_chart.draw(Rect::new(0, 0, 800, 400), &mut svg_ctx2);
    svg_ctx2.save("trading_volume.svg").expect("Failed to save SVG");
    println!("SVG saved to trading_volume.svg");
}
```

---

## 12. 架構摘要

```
┌─────────────────────────────────────────┐
│              ChartType Enum              │
│  Line | Bar | Pie | Scatter | Area       │
└──────────────────┬──────────────────────┘
                   │ create_chart()
┌──────────────────▼──────────────────────┐
│            Chart Trait                   │
│  add_series / draw / set_title / ...    │
└──────────────────┬──────────────────────┘
                   │
     ┌─────────────┼─────────────┐
     ▼             ▼             ▼
┌─────────┐ ┌─────────┐ ┌──────────┐
│LineChart│ │BarChart │ │PieChart  │  ...
└────┬────┘ └────┬────┘ └────┬─────┘
     │           │           │
     └───────────┼───────────┘
                 │ draw(rect, context)
     ┌───────────▼───────────┐
     │   ChartContext Trait   │
     │  14 drawing primitives │
     └───────────┬───────────┘
                 │
     ┌───────────┼───────────┐
     ▼                       ▼
┌──────────────┐    ┌─────────────────┐
│SvgChartContext│   │MemoryChartContext│
│ (SVG export)  │   │  (testing only)  │
└──────────────┘    └─────────────────┘
```

| 元件 | 角色 |
|-----------|------|
| `DataPoint` | 單一 (x, y) 資料點，含可選標籤 |
| `ChartSeries` | 具名資料點集合，含顏色與可見性 |
| `ChartType` | 列舉，含用於圖表實例化的工廠方法 |
| `Chart` 特徵 | 通用圖表介面：數列管理、軸標籤、繪圖 |
| `ChartContext` 特徵 | 可插拔繪圖後端，含 14 個基本單元 |
| `LineChart` | 含刻度、格線、圖例的折線圖 |
| `BarChart` | 含分組數列的垂直長條圖 |
| `PieChart` | 含弧形渲染的圓餅圖/環圈圖 |
| `ScatterChart` | 基於點的散佈圖 |
| `AreaChart` | 線條下方的填滿區域 |
| `SvgChartContext` | SVG 向量輸出上下文 |
| `MemoryChartContext` | 用於測試的記憶體內指令記錄器 |
| `CartesianLayout` | 繪圖區域、邊距、圖例位置計算 |
| `ChartLayout` | 用於將圖表整合到 widget 樹中的 widget 佈局 |
| `render_chart_to_svg_file()` | 一次性 SVG 匯出函式 |
