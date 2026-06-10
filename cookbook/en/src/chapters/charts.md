# Charts & Data Visualization

rust-widgets includes a built-in charting system with five chart types, a
pluggable drawing backend, SVG export, and widget integration — all through
the same rendering pipeline with no external charting dependency.

---

## 1. Data Types

### `DataPoint`

```rust
use rust_widgets::chart::DataPoint;

let point = DataPoint {
    x: 1.0,
    y: 42.5,
    label: Some("Q1".into()),
};

// Without label
let simple = DataPoint {
    x: 2.0,
    y: 58.0,
    label: None,
};
```

| Field | Type | Description |
|-------|------|-------------|
| `x` | `f64` | Data-domain x coordinate |
| `y` | `f64` | Data-domain y coordinate |
| `label` | `Option<String>` | Optional label for legends/tooltips |

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

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Series display name (legend) |
| `data` | `Vec<DataPoint>` | Ordered data points |
| `color` | `Color` | Series draw color |
| `visible` | `bool` | Toggle visibility for filtering |

---

## 2. `ChartType` Enum

```rust
pub enum ChartType {
    Line,     // Polyline chart
    Bar,      // Vertical bar chart
    Pie,      // Pie chart
    Scatter,  // Scatter chart
    Area,     // Area chart
}
```

### Factory Method

```rust
use rust_widgets::chart::ChartType;

// Create a boxed chart from a type variant
let chart: Box<dyn Chart> = ChartType::Line.create_chart();
let bar_chart = ChartType::Bar.create_chart();
let pie_chart = ChartType::Pie.create_chart();
let scatter_chart = ChartType::Scatter.create_chart();
let area_chart = ChartType::Area.create_chart();
```

---

## 3. The `Chart` Trait

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

Every chart type implements this trait, making them interchangeable:

```rust
use rust_widgets::chart::{Chart, ChartType, ChartSeries};
use rust_widgets::core::Rect;

let mut chart = ChartType::Line.create_chart();
chart.set_title("Quarterly Revenue".into());
chart.set_x_axis_label("Quarter".into());
chart.set_y_axis_label("Revenue ($K)".into());
chart.add_series(revenue_series);

// Draw into any ChartContext
chart.draw(Rect::new(0, 0, 800, 400), &mut context);
```

---

## 4. The `ChartContext` Trait (14 Drawing Primitives)

`ChartContext` is the pluggable drawing backend. Two implementations are provided:
`SvgChartContext` for vector output and `MemoryChartContext` for testing.

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

| # | Method | Description |
|---|--------|-------------|
| 1 | `draw_line` | Line segment from-to with width |
| 2 | `draw_rect` | Filled rectangle at rect with color |
| 3 | `draw_text` | Text at position with font size and color |
| 4 | `draw_circle` | Filled circle at center with radius |
| 5 | `draw_polygon` | Filled polygon from point list |
| 6 | `draw_path_segment` | Single path segment (line) |
| 7 | `draw_arc` | Arc approximation via polygon segments |
| 8 | `draw_path` | Multi-segment SVG path |
| 9 | `draw_ellipse` | Filled ellipse |
| 10 | `set_fill_color` | Set fill color for subsequent draws |
| 11 | `set_stroke_color` | Set stroke color for subsequent draws |

---

## 5. Concrete Chart Implementations

### LineChart

```rust
use rust_widgets::chart::{LineChart, Chart, ChartSeries, DataPoint};
use rust_widgets::core::{Color, Rect};
use rust_widgets::chart::svg::MemoryChartContext;

let mut chart = LineChart::new();
chart.set_title("Temperature Trend".into());
chart.set_x_axis_label("Day".into());
chart.set_y_axis_label("°C".into());
chart.set_x_tick_count(7);       // 7 ticks on x-axis (clamped 2-16)
chart.set_y_tick_count(5);       // 5 ticks on y-axis (clamped 2-16)
chart.set_grid_enabled(true);    // Enable grid lines

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

// Draw to memory context
let mut ctx = MemoryChartContext::default();
chart.draw(Rect::new(0, 0, 800, 400), &mut ctx);
println!("Commands: {:?}", ctx.commands);
```

### BarChart

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

### PieChart

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

### ScatterChart & AreaChart

```rust
use rust_widgets::chart::{ScatterChart, AreaChart};

// Scatter chart — individual points
let mut scatter = ScatterChart::new();
scatter.set_title("User Engagement".into());
scatter.set_x_axis_label("Sessions".into());
scatter.set_y_axis_label("Time (min)".into());
scatter.add_series(engagement_data);

// Area chart — filled region below line
let mut area = AreaChart::new();
area.set_title("Cumulative Downloads".into());
area.set_x_axis_label("Month".into());
area.set_y_axis_label("Downloads".into());
area.add_series(download_data);
```

---

## 6. Chart Features

### Axis Labels, Ticks, and Grid

```rust
let mut chart = LineChart::new();

// Axis labels
chart.set_title("Performance Metrics".into());
chart.set_x_axis_label("Iteration".into());
chart.set_y_axis_label("Throughput (req/s)".into());

// Tick configuration (clamped to 2-16)
chart.set_x_tick_count(10);
chart.set_y_tick_count(8);

// Grid lines
chart.set_grid_enabled(true);
```

### Legend

Legends are drawn automatically from visible series. The legend position is
computed by `compute_cartesian_layout()`, which reserves 170px on the right
when legend items are present.

```rust
// Legends appear automatically for all visible series.
// Each series gets a colored line + name in the legend area.
// Overflow is shown as "+N more".
```

### Color Palettes

Colors are user-defined on each `ChartSeries`. A common palette:

```rust
const PALETTE: [Color; 6] = [
    Color { r: 66,  g: 133, b: 244, a: 255 },  // Blue
    Color { r: 234, g: 67,  b: 53,  a: 255 },  // Red
    Color { r: 52,  g: 168, b: 83,  a: 255 },  // Green
    Color { r: 251, g: 188, b: 4,   a: 255 },  // Yellow
    Color { r: 171, g: 71,  b: 188, a: 255 },  // Purple
    Color { r: 0,   g: 172, b: 193, a: 255 },  // Cyan
];
```

---

## 7. SVG Chart Export

`SvgChartContext` renders charts to SVG for export, embedding, or pixel-accurate
verification.

### Render to SVG File

```rust
use rust_widgets::chart::{Chart, ChartType};
use rust_widgets::chart::svg::render_chart_to_svg_file;
use rust_widgets::core::Rect;

let mut chart = ChartType::Line.create_chart();
chart.set_title("Export Demo".into());
chart.add_series(my_series);

// Render and save to file
render_chart_to_svg_file(
    chart.as_ref(),
    Rect::new(0, 0, 800, 400),
    "chart_output.svg",
).expect("Failed to save SVG");
```

### Render to SVG String

```rust
use rust_widgets::chart::svg::SvgChartContext;
use rust_widgets::core::Rect;

let mut ctx = SvgChartContext::new(800, 400);
chart.draw(Rect::new(0, 0, 800, 400), &mut ctx);

let svg_string = ctx.to_svg_string();
println!("{}", svg_string);

// Or save directly
ctx.save("chart.svg").unwrap();
```

### SVG Output Format

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="400" viewBox="0 0 800 400">
  <rect x="0" y="0" width="100" height="50" fill="#4285F4" fill-opacity="1.000" stroke="none" />
  <line x1="10" y1="20" x2="100" y2="20" stroke="#EA4335" stroke-opacity="1.000" stroke-width="2" />
  <text x="8" y="16" fill="#141414" fill-opacity="1.000" font-size="14" font-family="sans-serif">Title</text>
  <!-- ... -->
</svg>
```

Colors are output as `#RRGGBB` hex with alpha as `fill-opacity`/`stroke-opacity`.
Text is XML-escaped (`&`, `<`, `>`).

---

## 8. `MemoryChartContext` for Testing

`MemoryChartContext` records draw commands as strings, enabling test assertions
without rendering:

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

// Assert on recorded commands
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("rect:")));
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("text:")));
assert!(ctx.commands.iter().any(|cmd| cmd.starts_with("line:")));
```

### Recorded Command Format

| Command | Format |
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

## 9. Cartesian Layout Computation

`compute_cartesian_layout()` calculates the plot area, margins, and legend
position from a given bounding rectangle:

```rust
use rust_widgets::chart::charts::compute_cartesian_layout;
use rust_widgets::core::Rect;

let rect = Rect::new(0, 0, 800, 400);
let layout = compute_cartesian_layout(rect, true, true, 3);

println!("Plot area: ({}, {}) → {}×{}",
    layout.plot_x, layout.plot_y, layout.plot_w, layout.plot_h);
println!("Legend at: ({}, {})", layout.legend_x, layout.legend_y);

// Margin rules:
//   left:    48px (64px with y-axis label)
//   top:     32px
//   bottom:  36px (52px with x-axis label)
//   right:   24px (170px with legend items)
```

### Axis and Tick Drawing Functions

```rust
use rust_widgets::chart::charts::{
    draw_cartesian_axes,
    draw_x_ticks,
    draw_y_ticks,
    draw_legend,
};

// Draw axes
draw_cartesian_axes(&mut ctx, &layout);

// Draw tick marks and labels
draw_x_ticks(&mut ctx, &layout, min_x, max_x, 5, true);  // with grid
draw_y_ticks(&mut ctx, &layout, min_y, max_y, 5, true);

// Draw legend
draw_legend(&mut ctx, &layout, &visible_series);
```

---

## 10. `ChartLayout` for Widget Integration

`ChartLayout` positions a chart widget to fill available space within a
container:

```rust
use rust_widgets::chart::layout::ChartLayout;
use rust_widgets::layout::Layout;
use rust_widgets::core::Rect;

let mut layout = ChartLayout::new(chart_widget_id);

// Check children
assert!(layout.has_child(chart_widget_id));
assert_eq!(layout.child_ids(), vec![chart_widget_id]);

// Replace child
layout.add_widget(new_chart_id, 0);

// Layout fills entire rectangle
layout.update(Rect::new(0, 0, 800, 600), &mut |id, rect| {
    // Position chart_widget at rect
    println!("Place widget {} at {:?}", id, rect);
});

// Remove and clear
layout.remove_widget(chart_widget_id);
layout.clear();
```

`ChartLayout` implements the full `Layout` trait:

| Method | Behavior |
|--------|----------|
| `add_widget` | Stores a single child (replaces previous) |
| `remove_widget` | Removes child if it matches |
| `update` | Positions child to fill the entire rect |
| `child_ids` | Returns the single child ID |
| `has_child` | Checks if the given ID is managed |
| `clear` | Removes all children |

---

## 11. Complete Chart Example with Real Data

```rust
use rust_widgets::chart::{
    Chart, ChartType, ChartSeries, DataPoint,
};
use rust_widgets::chart::svg::{SvgChartContext, MemoryChartContext};
use rust_widgets::core::{Color, Rect};

fn main() {
    // Stock price data
    let stock_data = vec![
        DataPoint { x: 1.0, y: 150.0, label: None },
        DataPoint { x: 2.0, y: 152.0, label: None },
        DataPoint { x: 3.0, y: 148.0, label: None },
        DataPoint { x: 4.0, y: 155.0, label: None },
        DataPoint { x: 5.0, y: 160.0, label: None },
        DataPoint { x: 6.0, y: 158.0, label: None },
        DataPoint { x: 7.0, y: 163.0, label: None },
    ];

    // Volume data (bars)
    let volume_data = vec![
        DataPoint { x: 1.0, y: 1000.0, label: None },
        DataPoint { x: 2.0, y: 1200.0, label: None },
        DataPoint { x: 3.0, y: 800.0, label: None },
        DataPoint { x: 4.0, y: 1500.0, label: None },
        DataPoint { x: 5.0, y: 2000.0, label: None },
        DataPoint { x: 6.0, y: 1300.0, label: None },
        DataPoint { x: 7.0, y: 1800.0, label: None },
    ];

    // Create line chart for price
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

    // Test with MemoryChartContext
    let mut mem_ctx = MemoryChartContext::default();
    price_chart.draw(Rect::new(0, 0, 800, 400), &mut mem_ctx);

    assert!(!mem_ctx.commands.is_empty());
    println!("Generated {} draw commands", mem_ctx.commands.len());

    // Export to SVG
    let mut svg_ctx = SvgChartContext::new(800, 400);
    price_chart.draw(Rect::new(0, 0, 800, 400), &mut svg_ctx);
    svg_ctx.save("stock_price.svg").expect("Failed to save SVG");
    println!("SVG saved to stock_price.svg");

    // Create bar chart for volume
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

    // Export volume chart to SVG
    let mut svg_ctx2 = SvgChartContext::new(800, 400);
    volume_chart.draw(Rect::new(0, 0, 800, 400), &mut svg_ctx2);
    svg_ctx2.save("trading_volume.svg").expect("Failed to save SVG");
    println!("SVG saved to trading_volume.svg");
}
```

---

## 12. Architecture Summary

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

| Component | Role |
|-----------|------|
| `DataPoint` | Single (x, y) data point with optional label |
| `ChartSeries` | Named collection of data points with color and visibility |
| `ChartType` | Enum with factory method for chart instantiation |
| `Chart` trait | Universal chart interface: series management, axis labels, draw |
| `ChartContext` trait | Pluggable drawing backend with 14 primitives |
| `LineChart` | Polyline chart with ticks, grid, legend |
| `BarChart` | Vertical bar chart with grouped series |
| `PieChart` | Pie/donut chart with arc rendering |
| `ScatterChart` | Point-based scatter plot |
| `AreaChart` | Filled area below a line |
| `SvgChartContext` | SVG vector output context |
| `MemoryChartContext` | In-memory command recorder for tests |
| `CartesianLayout` | Plot area, margins, legend position computation |
| `ChartLayout` | Widget layout for integrating charts into widget trees |
| `render_chart_to_svg_file()` | One-shot SVG export function |
