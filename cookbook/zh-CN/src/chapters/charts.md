# 图表与数据可视化

rust-widgets 包含一个内置的图表系统，提供五种图表类型、可插拔的绘图后端、SVG 导出和窗口部件集成——全部通过相同的渲染管道实现，无需外部图表依赖。

---

## 1. 数据类型

### `DataPoint`

```rust
use rust_widgets::chart::DataPoint;

let point = DataPoint {
    x: 1.0,
    y: 42.5,
    label: Some("Q1".into()),
};

// 无标签
let simple = DataPoint {
    x: 2.0,
    y: 58.0,
    label: None,
};
```

| 字段 | 类型 | 描述 |
|-------|------|------|
| `x` | `f64` | 数据域 x 坐标 |
| `y` | `f64` | 数据域 y 坐标 |
| `label` | `Option<String>` | 用于图例/工具提示的可选标签 |

### `ChartSeries`

```rust
use rust_widgets::chart::{ChartSeries, DataPoint};
use rust_widgets::core::Color;

let series = ChartSeries {
    name: "收入".into(),
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

| 字段 | 类型 | 描述 |
|-------|------|------|
| `name` | `String` | 系列显示名称（图例） |
| `data` | `Vec<DataPoint>` | 有序数据点 |
| `color` | `Color` | 系列绘制颜色 |
| `visible` | `bool` | 切换可见性以进行筛选 |

---

## 2. `ChartType` 枚举

```rust
pub enum ChartType {
    Line,     // 折线图
    Bar,      // 垂直柱状图
    Pie,      // 饼图
    Scatter,  // 散点图
    Area,     // 面积图
}
```

### 工厂方法

```rust
use rust_widgets::chart::ChartType;

// 从类型变体创建装箱的图表
let chart: Box<dyn Chart> = ChartType::Line.create_chart();
let bar_chart = ChartType::Bar.create_chart();
let pie_chart = ChartType::Pie.create_chart();
let scatter_chart = ChartType::Scatter.create_chart();
let area_chart = ChartType::Area.create_chart();
```

---

## 3. `Chart` 特质

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

每种图表类型都实现此特质，使它们可以互换：

```rust
use rust_widgets::chart::{Chart, ChartType, ChartSeries};
use rust_widgets::core::Rect;

let mut chart = ChartType::Line.create_chart();
chart.set_title("季度收入".into());
chart.set_x_axis_label("季度".into());
chart.set_y_axis_label("收入 ($K)".into());
chart.add_series(revenue_series);

// 绘制到任意 ChartContext
chart.draw(Rect::new(0, 0, 800, 400), &mut context);
```

---

## 4. `ChartContext` 特质（14 种绘制图元）

`ChartContext` 是可插拔的绘图后端。提供了两个实现：用于矢量输出的 `SvgChartContext` 和用于测试的 `MemoryChartContext`。

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

| 序号 | 方法 | 描述 |
|:---:|--------|------|
| 1 | `draw_line` | 从起点到终点的线段，可指定宽度 |
| 2 | `draw_rect` | 在指定矩形内填充颜色 |
| 3 | `draw_text` | 在指定位置绘制文本，可指定字号和颜色 |
| 4 | `draw_circle` | 以圆心和半径绘制填充圆 |
| 5 | `draw_polygon` | 从点列表绘制填充多边形 |
| 6 | `draw_path_segment` | 单段路径（线段） |
| 7 | `draw_arc` | 通过多边形分段近似绘制弧线 |
| 8 | `draw_path` | 多段 SVG 路径 |
| 9 | `draw_ellipse` | 填充椭圆 |
| 10 | `set_fill_color` | 设置后续绘制的填充颜色 |
| 11 | `set_stroke_color` | 设置后续绘制的描边颜色 |

---

## 5. 具体图表实现

### LineChart（折线图）

```rust
use rust_widgets::chart::{LineChart, Chart, ChartSeries, DataPoint};
use rust_widgets::core::{Color, Rect};
use rust_widgets::chart::svg::MemoryChartContext;

let mut chart = LineChart::new();
chart.set_title("温度趋势".into());
chart.set_x_axis_label("天".into());
chart.set_y_axis_label("°C".into());
chart.set_x_tick_count(7);       // x 轴 7 个刻度（限制 2-16）
chart.set_y_tick_count(5);       // y 轴 5 个刻度（限制 2-16）
chart.set_grid_enabled(true);    // 启用网格线

chart.add_series(ChartSeries {
    name: "高温".into(),
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

// 绘制到内存上下文
let mut ctx = MemoryChartContext::default();
chart.draw(Rect::new(0, 0, 800, 400), &mut ctx);
println!("命令数: {:?}", ctx.commands);
```

### BarChart（柱状图）

```rust
use rust_widgets::chart::BarChart;

let mut chart = BarChart::new();
chart.set_title("月度销售额".into());
chart.set_x_axis_label("月份".into());
chart.set_y_axis_label("数量".into());

chart.add_series(ChartSeries {
    name: "产品 A".into(),
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
    name: "产品 B".into(),
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

### PieChart（饼图）

```rust
use rust_widgets::chart::PieChart;

let mut chart = PieChart::new();
chart.set_title("市场份额".into());

chart.add_series(ChartSeries {
    name: "浏览器".into(),
    data: vec![DataPoint { x: 0.0, y: 45.0, label: Some("Chrome".into()) }],
    color: Color { r: 66, g: 133, b: 244, a: 255 },
    visible: true,
});

chart.add_series(ChartSeries {
    name: "浏览器".into(),
    data: vec![DataPoint { x: 0.0, y: 30.0, label: Some("Safari".into()) }],
    color: Color { r: 52, g: 168, b: 83, a: 255 },
    visible: true,
});

chart.add_series(ChartSeries {
    name: "浏览器".into(),
    data: vec![DataPoint { x: 0.0, y: 25.0, label: Some("Firefox".into()) }],
    color: Color { r: 251, g: 188, b: 4, a: 255 },
    visible: true,
});
```

### ScatterChart（散点图）与 AreaChart（面积图）

```rust
use rust_widgets::chart::{ScatterChart, AreaChart};

// 散点图 — 单个数据点
let mut scatter = ScatterChart::new();
scatter.set_title("用户参与度".into());
scatter.set_x_axis_label("会话数".into());
scatter.set_y_axis_label("时长 (分钟)".into());
scatter.add_series(engagement_data);

// 面积图 — 折线下方填充区域
let mut area = AreaChart::new();
area.set_title("累计下载量".into());
area.set_x_axis_label("月份".into());
area.set_y_axis_label("下载量".into());
area.add_series(download_data);
```

---

## 6. 图表功能

### 轴标签、刻度和网格

```rust
let mut chart = LineChart::new();

// 轴标签
chart.set_title("性能指标".into());
chart.set_x_axis_label("迭代次数".into());
chart.set_y_axis_label("吞吐量 (请求/秒)".into());

// 刻度配置（限制为 2-16）
chart.set_x_tick_count(10);
chart.set_y_tick_count(8);

// 网格线
chart.set_grid_enabled(true);
```

### 图例

图例从可见系列自动绘制。图例位置由 `compute_cartesian_layout()` 计算，当存在图例项时在右侧预留 170px。

```rust
// 图例自动为所有可见系列显示。
// 每个系列在图例区域中获取一条彩色线 + 名称。
// 溢出时显示为 "+N more"。
```

### 调色板

颜色由用户在每个 `ChartSeries` 上定义。一个常用的调色板：

```rust
const PALETTE: [Color; 6] = [
    Color { r: 66,  g: 133, b: 244, a: 255 },  // 蓝色
    Color { r: 234, g: 67,  b: 53,  a: 255 },  // 红色
    Color { r: 52,  g: 168, b: 83,  a: 255 },  // 绿色
    Color { r: 251, g: 188, b: 4,   a: 255 },  // 黄色
    Color { r: 171, g: 71,  b: 188, a: 255 },  // 紫色
    Color { r: 0,   g: 172, b: 193, a: 255 },  // 青色
];
```

---

## 7. SVG 图表导出

`SvgChartContext` 将图表渲染为 SVG 以用于导出、嵌入或像素级精确验证。

### 渲染为 SVG 文件

```rust
use rust_widgets::chart::{Chart, ChartType};
use rust_widgets::chart::svg::render_chart_to_svg_file;
use rust_widgets::core::Rect;

let mut chart = ChartType::Line.create_chart();
chart.set_title("导出演示".into());
chart.add_series(my_series);

// 渲染并保存到文件
render_chart_to_svg_file(
    chart.as_ref(),
    Rect::new(0, 0, 800, 400),
    "chart_output.svg",
).expect("保存 SVG 失败");
```

### 渲染为 SVG 字符串

```rust
use rust_widgets::chart::svg::SvgChartContext;
use rust_widgets::core::Rect;

let mut ctx = SvgChartContext::new(800, 400);
chart.draw(Rect::new(0, 0, 800, 400), &mut ctx);

let svg_string = ctx.to_svg_string();
println!("{}", svg_string);

// 或直接保存
ctx.save("chart.svg").unwrap();
```

### SVG 输出格式

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="400" viewBox="0 0 800 400">
  <rect x="0" y="0" width="100" height="50" fill="#4285F4" fill-opacity="1.000" stroke="none" />
  <line x1="10" y1="20" x2="100" y2="20" stroke="#EA4335" stroke-opacity="1.000" stroke-width="2" />
  <text x="8" y="16" fill="#141414" fill-opacity="1.000" font-size="14" font-family="sans-serif">标题</text>
  <!-- ... -->
</svg>
```

颜色以 `#RRGGBB` 十六进制输出，alpha 以 `fill-opacity`/`stroke-opacity` 表示。文本经过 XML 转义（`&`、`<`、`>`）。

---

## 8. `MemoryChartContext` 用于测试

`MemoryChartContext` 将绘制命令记录为字符串，无需渲染即可进行测试断言：

```rust
use rust_widgets::chart::svg::MemoryChartContext;
use rust_widgets::chart::{Chart, ChartType, ChartSeries, DataPoint};
use rust_widgets::core::{Color, Rect};

let mut chart = ChartType::Line.create_chart();
chart.add_series(ChartSeries {
    name: "测试".into(),
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

// 对记录的命令进行断言
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("rect:")));
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("text:")));
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("line:")));
```

### 记录的命令格式

| 命令 | 格式 |
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

## 9. 笛卡尔布局计算

`compute_cartesian_layout()` 从给定的边界矩形计算绘图区域、边距和图例位置：

```rust
use rust_widgets::chart::charts::compute_cartesian_layout;
use rust_widgets::core::Rect;

let rect = Rect::new(0, 0, 800, 400);
let layout = compute_cartesian_layout(rect, true, true, 3);

println!("绘图区域: ({}, {}) → {}×{}",
    layout.plot_x, layout.plot_y, layout.plot_w, layout.plot_h);
println!("图例位置: ({}, {})", layout.legend_x, layout.legend_y);

// 边距规则：
//   left:    48px（带 y 轴标签时为 64px）
//   top:     32px
//   bottom:  36px（带 x 轴标签时为 52px）
//   right:   24px（带图例项时为 170px）
```

### 轴和刻度绘制函数

```rust
use rust_widgets::chart::charts::{
    draw_cartesian_axes,
    draw_x_ticks,
    draw_y_ticks,
    draw_legend,
};

// 绘制坐标轴
draw_cartesian_axes(&mut ctx, &layout);

// 绘制刻度标记和标签
draw_x_ticks(&mut ctx, &layout, min_x, max_x, 5, true);  // 带网格
draw_y_ticks(&mut ctx, &layout, min_y, max_y, 5, true);

// 绘制图例
draw_legend(&mut ctx, &layout, &visible_series);
```

---

## 10. `ChartLayout` 用于窗口部件集成

`ChartLayout` 将图表窗口部件定位以填充容器内的可用空间：

```rust
use rust_widgets::chart::layout::ChartLayout;
use rust_widgets::layout::Layout;
use rust_widgets::core::Rect;

let mut layout = ChartLayout::new(chart_widget_id);

// 检查子部件
assert!(layout.has_child(chart_widget_id));
assert_eq!(layout.child_ids(), vec![chart_widget_id]);

// 替换子部件
layout.add_widget(new_chart_id, 0);

// 布局填充整个矩形
layout.update(Rect::new(0, 0, 800, 600), &mut |id, rect| {
    // 将 chart_widget 定位在 rect 处
    println!("将窗口部件 {} 放置在 {:?}", id, rect);
});

// 移除和清空
layout.remove_widget(chart_widget_id);
layout.clear();
```

`ChartLayout` 实现了完整的 `Layout` 特质：

| 方法 | 行为 |
|--------|------|
| `add_widget` | 存储单个子部件（替换之前的） |
| `remove_widget` | 如果匹配则移除子部件 |
| `update` | 将子部件定位以填充整个矩形 |
| `child_ids` | 返回唯一的子部件 ID |
| `has_child` | 检查给定的 ID 是否被管理 |
| `clear` | 移除所有子部件 |

---

## 11. 带真实数据的完整图表示例

```rust
use rust_widgets::chart::{
    Chart, ChartType, ChartSeries, DataPoint,
};
use rust_widgets::chart::svg::{SvgChartContext, MemoryChartContext};
use rust_widgets::core::{Color, Rect};

fn main() {
    // 股票价格数据
    let stock_data = vec![
        DataPoint { x: 1.0, y: 150.0, label: None },
        DataPoint { x: 2.0, y: 152.0, label: None },
        DataPoint { x: 3.0, y: 148.0, label: None },
        DataPoint { x: 4.0, y: 155.0, label: None },
        DataPoint { x: 5.0, y: 160.0, label: None },
        DataPoint { x: 6.0, y: 158.0, label: None },
        DataPoint { x: 7.0, y: 163.0, label: None },
    ];

    // 交易量数据（柱状）
    let volume_data = vec![
        DataPoint { x: 1.0, y: 1000.0, label: None },
        DataPoint { x: 2.0, y: 1200.0, label: None },
        DataPoint { x: 3.0, y: 800.0, label: None },
        DataPoint { x: 4.0, y: 1500.0, label: None },
        DataPoint { x: 5.0, y: 2000.0, label: None },
        DataPoint { x: 6.0, y: 1300.0, label: None },
        DataPoint { x: 7.0, y: 1800.0, label: None },
    ];

    // 创建股票价格折线图
    let mut price_chart = ChartType::Line.create_chart();
    price_chart.set_title("股票价格（7 天）".into());
    price_chart.set_x_axis_label("天数".into());
    price_chart.set_y_axis_label("价格 ($)".into());
    price_chart.set_x_tick_count(7);
    price_chart.set_y_tick_count(5);
    price_chart.set_grid_enabled(true);

    price_chart.add_series(ChartSeries {
        name: "AAPL".into(),
        data: stock_data,
        color: Color { r: 66, g: 133, b: 244, a: 255 },
        visible: true,
    });

    // 使用 MemoryChartContext 进行测试
    let mut mem_ctx = MemoryChartContext::default();
    price_chart.draw(Rect::new(0, 0, 800, 400), &mut mem_ctx);

    assert!(!mem_ctx.commands.is_empty());
    println!("生成了 {} 条绘制命令", mem_ctx.commands.len());

    // 导出为 SVG
    let mut svg_ctx = SvgChartContext::new(800, 400);
    price_chart.draw(Rect::new(0, 0, 800, 400), &mut svg_ctx);
    svg_ctx.save("stock_price.svg").expect("保存 SVG 失败");
    println!("SVG 已保存到 stock_price.svg");

    // 创建交易量柱状图
    let mut volume_chart = ChartType::Bar.create_chart();
    volume_chart.set_title("交易量（7 天）".into());
    volume_chart.set_x_axis_label("天数".into());
    volume_chart.set_y_axis_label("交易量".into());

    volume_chart.add_series(ChartSeries {
        name: "交易量".into(),
        data: volume_data,
        color: Color { r: 52, g: 168, b: 83, a: 255 },
        visible: true,
    });

    // 将交易量图表导出为 SVG
    let mut svg_ctx2 = SvgChartContext::new(800, 400);
    volume_chart.draw(Rect::new(0, 0, 800, 400), &mut svg_ctx2);
    svg_ctx2.save("trading_volume.svg").expect("保存 SVG 失败");
    println!("SVG 已保存到 trading_volume.svg");
}
```

---

## 12. 架构总结

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

| 组件 | 角色 |
|-----------|------|
| `DataPoint` | 单个 (x, y) 数据点，可带可选标签 |
| `ChartSeries` | 带颜色和可见性的命名数据点集合 |
| `ChartType` | 带工厂方法用于图表实例化的枚举 |
| `Chart` 特质 | 通用图表接口：系列管理、轴标签、绘制 |
| `ChartContext` 特质 | 包含 14 种图元的可插拔绘制后端 |
| `LineChart` | 带刻度、网格、图例的折线图 |
| `BarChart` | 支持分组系列的垂直柱状图 |
| `PieChart` | 使用弧线渲染的饼图/环形图 |
| `ScatterChart` | 基于点的散点图 |
| `AreaChart` | 折线下方的填充区域 |
| `SvgChartContext` | SVG 矢量输出上下文 |
| `MemoryChartContext` | 用于测试的内存中命令记录器 |
| `CartesianLayout` | 绘图区域、边距、图例位置计算 |
| `ChartLayout` | 将图表集成到窗口部件树中的窗口部件布局 |
| `render_chart_to_svg_file()` | 一次性 SVG 导出函数 |
