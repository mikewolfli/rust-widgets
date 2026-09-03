# 高级主题

本章涵盖创建自定义控件、布局管理器、绘制后端、控制后端、高级信号/槽模式、数据绑定、自定义主题、CSS 热重载、性能分析、测试、no_std 构建、安全注意事项以及贡献指南。

## 创建自定义控件

自定义控件需实现三个 trait：`Widget`、`EventHandler`，以及可选地实现 `Draw` 以进行自定义渲染。

### 实现 Widget + EventHandler + Draw

```rust
use rust_widgets::widget::{Widget, BaseWidget, WidgetKind, Draw};
use rust_widgets::core::{Rect, Size, Color, ObjectId, Point};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::style::WidgetStyle;

#[derive(Debug)]
struct Gauge {
    base: BaseWidget,
    value: f32,           // 0.0 – 1.0
    min: f32,
    max: f32,
    color: Color,
}

impl Gauge {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Custom("Gauge"), geometry),
            value: 0.5,
            min: 0.0,
            max: 1.0,
            color: Color::BLUE,
        }
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
        self.request_redraw();
    }
}

impl EventHandler for Gauge {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::MousePress { pos, .. } = event {
            // 根据点击位置计算值
            let rect = self.geometry();
            let pct = ((pos.x - rect.x) as f32 / rect.width as f32).clamp(0.0, 1.0);
            self.set_value(self.min + (self.max - self.min) * pct);
        }
    }
}

impl Widget for Gauge {
    fn base(&self) -> &BaseWidget { &self.base }
    fn base_mut(&mut self) -> &mut BaseWidget { &mut self.base }
}

impl Draw for Gauge {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let rect = self.geometry();
        let pct = (self.value - self.min) / (self.max - self.min);

        // 背景轨道
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height, Color::LIGHT_GRAY);

        // 填充部分
        let filled_width = (rect.width as f32 * pct) as u32;
        ctx.fill_rect(
            rect.x,
            rect.y,
            filled_width,
            rect.height,
            self.color,
        );

        // 边框
        ctx.draw_rect(rect.x, rect.y, rect.width, rect.height, Color::DARK_GRAY, 1);
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

// 使用方式
let gauge = Gauge::new(Rect::new(10, 10, 200, 24));
let _id = gauge.id();
println!("Custom widget kind: {:?}", gauge.kind());
```

### 带信号的控件

```rust
use rust_widgets::signal::{Signal1, GenericSignal};

struct Slider {
    base: BaseWidget,
    value: f32,
    pub value_changed: Signal1<f32>,
}

impl Draw for Slider {
    fn draw(&mut self, _ctx: &mut RenderContext) {
        // ... 自定义渲染 ...
    }
}

impl EventHandler for Slider {
    fn handle_event(&mut self, event: &Event) {
        if let Event::MousePress { pos, .. } = event {
            let rect = self.geometry();
            let new_value = ((pos.x - rect.x) as f32 / rect.width as f32).clamp(0.0, 1.0);
            self.value = new_value;
            self.value_changed.emit(new_value);
        }
    }
}
```

## 自定义布局管理器

实现 `Layout` trait 以创建自定义布局算法：

```rust
use rust_widgets::core::{ObjectId, Rect, Point, Size};
use rust_widgets::layout::{Layout, LayoutContext, LayoutConstraints, SizePolicy};

struct CircularLayout {
    children: Vec<(ObjectId, u32)>,  // (id, stretch)
    radius: u32,
    center_x: u32,
    center_y: u32,
    start_angle: f32,
}

impl CircularLayout {
    pub fn new(radius: u32, cx: u32, cy: u32) -> Self {
        Self {
            children: Vec::new(),
            radius,
            center_x: cx,
            center_y: cy,
            start_angle: 0.0,
        }
    }
}

impl Layout for CircularLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.children.push((widget_id, stretch));
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|(id, _)| *id != widget_id);
    }

    fn update(&self, _rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let count = self.children.len() as u32;
        for (i, (id, _)) in self.children.iter().enumerate() {
            let angle = self.start_angle + (i as f32 / count as f32) * std::f32::consts::TAU;
            let x = self.center_x as f32 + self.radius as f32 * angle.cos();
            let y = self.center_y as f32 + self.radius as f32 * angle.sin();
            widgets(*id, Rect::new(x as i32, y as i32, 40, 40));
        }
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children.iter().map(|(id, _)| *id).collect()
    }

    fn clear(&mut self) {
        self.children.clear();
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// 使用方式
// let mut circular = CircularLayout::new(100, 200, 200);
// circular.add_widget(widget_a.id(), 1);
// circular.add_widget(widget_b.id(), 1);
// circular.update(container_rect, &mut |id, rect| { /* 应用 */ });
```

## 自定义绘制后端

实现 `PaintBackend` trait 以添加自定义渲染后端：

```rust
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::core::{Color, Size, Rect, Point};

struct CustomPaintBackend {
    width: u32,
    height: u32,
    scale: f32,
    buffer: Vec<u32>,  // 自定义像素格式
}

impl CustomPaintBackend {
    pub fn new(size: Size, scale: f32) -> Self {
        let pixel_count = (size.width as usize) * (size.height as usize);
        Self {
            width: size.width,
            height: size.height,
            scale,
            buffer: vec![0xFFFFFFFF; pixel_count],
        }
    }
}

impl PaintBackend for CustomPaintBackend {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    fn scale(&self) -> f32 {
        self.scale
    }

    fn begin_frame(&mut self, _background: Color) {}

    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        // 将颜色转换为 u32 ABGR
        let pixel = (color.a as u32) << 24
            | (color.b as u32) << 16
            | (color.g as u32) << 8
            | color.r as u32;

        let y_start = y.max(0) as u32;
        let y_end = (y + h as i32).min(self.height as i32) as u32;
        let x_start = x.max(0) as u32;
        let x_end = (x + w as i32).min(self.width as i32) as u32;

        for row in y_start..y_end {
            let row_start = row as usize * self.width as usize;
            for col in x_start..x_end {
                self.buffer[row_start + col as usize] = pixel;
            }
        }
    }

    fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color, _width: u32) {
        let pixel = pixel_from_color(color);
        // 绘制四条边 ...
    }

    fn end_frame(&mut self) {
        // 将缓冲区刷新到实际显示器
        // platform::display_write(&self.buffer);
    }
}

fn pixel_from_color(c: Color) -> u32 {
    (c.a as u32) << 24 | (c.b as u32) << 16 | (c.g as u32) << 8 | c.r as u32
}

// 通过 RenderContext 使用：
// let mut backend = CustomPaintBackend::new(Size::new(800, 600), 1.0);
// backend.begin_frame(Color::WHITE);
// let mut ctx = RenderContext::new(&mut backend);
// ctx.fill_rect(0, 0, 100, 50, Color::RED);
// backend.end_frame();
```

## 控制后端定制

控制后端管理控件调度策略和路由：

```rust
use rust_widgets::control_backend::Dispatcher;
use rust_widgets::core::ObjectId;

// 配置调度策略
struct CustomDispatchPolicy;

impl Dispatcher for CustomDispatchPolicy {
    fn should_dispatch_to_widget(&self, widget_id: ObjectId, action: &str) -> bool {
        // 自定义路由逻辑
        !action.contains("internal")
    }

    fn route_action(&self, widget_id: ObjectId, action: &str) -> Option<ObjectId> {
        // 将动作路由到特定处理器
        None  // 默认路由
    }
}
```

## 信号/槽高级模式

### ConnectionScope — 自动断开连接

```rust
use rust_widgets::signal::{ConnectionScope, Signal, GenericSignal};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

// 连接限定在宿主生命周期内
let signal = GenericSignal::new();
let hits = Arc::new(AtomicUsize::new(0));

{
    let owner = ConnectionScope::new();
    let h = Arc::clone(&hits);

    signal.connect_scoped(&owner, move || {
        h.fetch_add(1, Ordering::SeqCst);
    });

    signal.emit();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

// 宿主释放后，连接自动断开
signal.emit();
assert_eq!(hits.load(Ordering::SeqCst), 1);  // 不再增加
```

### 一次性信号

```rust
let signal = GenericSignal::new();
let hits = Arc::new(AtomicUsize::new(0));

{
    let h = Arc::clone(&hits);
    signal.connect_once(move || {
        h.fetch_add(1, Ordering::SeqCst);
    });
}

signal.emit();  // 触发
signal.emit();  // 无操作 — 第一次触发后已断开
assert_eq!(hits.load(Ordering::SeqCst), 1);
```

### 重入安全性

信号发射是重入安全的——在发射过程中连接新的槽不会导致死锁：

```rust
let signal = Signal::<u32>::new();
let emitted = Arc::new(AtomicUsize::new(0));

let e1 = Arc::clone(&emitted);
let e2 = Arc::clone(&emitted);
let s2 = signal.clone();

signal.connect(move |v| {
    e1.fetch_add(1, Ordering::SeqCst);
    if *v == 1 {
        // 在发射过程中连接另一个槽（重入）
        s2.connect(move |_| {
            e2.fetch_add(1, Ordering::SeqCst);
        });
    }
});

signal.emit(1);  // 第一次发射 — 新槽被连接
signal.emit(2);  // 第二次发射 — 两个槽都被触发

assert_eq!(emitted.load(Ordering::SeqCst), 3);
```

### 带类型数据的信号

```rust
use rust_widgets::signal::Signal1;
use std::sync::Arc;

let signal = Signal1::<String>::new();

signal.connect(|msg: Arc<String>| {
    println!("Received: {}", msg);
});

signal.emit("Hello!".to_string());
```

## 双向数据绑定

rust-widgets 提供了响应式数据绑定系统，适用于 MVVM 风格的用户界面：

```rust
use rust_widgets::data_binding::{Binding, FnListener, Computed};

// 单个响应式值
let mut name = Binding::new("World".to_string());

name.subscribe("log", Box::new(FnListener::new(|key| {
    println!("[{}] changed!", key);
})));

name.set("Rust".to_string());
assert_eq!(name.get(), "Rust");

// 派生/计算值
let mut full_name = Binding::new("John Doe".to_string());
let mut greeting = Computed::new(|| {
    format!("Hello, {}!", full_name.get())
});

assert_eq!(greeting.get(), "Hello, John Doe!");
full_name.set("Jane Doe".to_string());
greeting.invalidate();
assert_eq!(greeting.get(), "Hello, Jane Doe!");
```

## 自定义图表类型

实现图表 trait 以创建自定义可视化：

```rust
use rust_widgets::chart::{
    ChartContext, Chart, ChartData,
};

struct ScatterPoint {
    x: f64,
    y: f64,
    label: String,
}

struct ScatterChart {
    data: Vec<ScatterPoint>,
    show_grid: bool,
}

impl ScatterChart {
    pub fn new() -> Self {
        Self { data: Vec::new(), show_grid: true }
    }

    pub fn add_point(&mut self, x: f64, y: f64, label: &str) {
        self.data.push(ScatterPoint {
            x, y,
            label: label.to_string(),
        });
    }
}

impl Chart for ScatterChart {
    fn data(&self) -> ChartData { ChartData::default() }
    fn data_mut(&mut self) -> &mut ChartData { unimplemented!() }
    fn render(&self, ctx: &mut ChartContext) {
        // 访问渲染上下文进行绘制
        // ctx.fill_rect(...);
        for point in &self.data {
            // 绘制每个点
        }
    }
}
```

## 自定义主题 — 扩展 ThemeManager

```rust
use rust_widgets::theme::{ThemeManager, Theme, Colors, Fonts, Spacing, Borders, ThemeOverrides};
use rust_widgets::core::{Color, Font};
use std::collections::HashMap;

let mut theme_manager = ThemeManager::default();

// 访问当前主题
let current = theme_manager.current_theme().unwrap();
println!("Current theme: {}", current.name);

// 创建自定义主题
let mut custom_theme = Theme {
    name: "corporate-blue".to_string(),
    colors: Colors {
        background: Color::from_hex("#FFFFFF").unwrap(),
        foreground: Color::from_hex("#1A1A1A").unwrap(),
        primary: Color::from_hex("#0052CC").unwrap(),
        secondary: Color::from_hex("#6B778C").unwrap(),
        accent: Color::from_hex("#00B8D9").unwrap(),
        error: Color::from_hex("#DE350B").unwrap(),
        warning: Color::from_hex("#FF991F").unwrap(),
        success: Color::from_hex("#36B37E").unwrap(),
        disabled: Color::from_hex("#A5ADBA").unwrap(),
        info: Color::from_hex("#0065FF").unwrap(),
    },
    fonts: Fonts::default(),
    spacing: Spacing {
        small: 4, medium: 8, large: 16, extra_large: 32,
    },
    borders: Borders { width: 1, radius: 4, shadow: true },
    overrides: ThemeOverrides {
        styles: HashMap::new(),
    },
};

// 使用内置暗色主题
let dark = Theme::dark();
println!("Dark theme background: {:?}", dark.colors.background);

// ThemeStyleToken 用于按类覆盖
use rust_widgets::theme::ThemeStyleToken;
let button_override = ThemeStyleToken {
    background: Some(Color::from_hex("#0052CC").unwrap()),
    foreground: Some(Color::WHITE),
    border: None,
    border_width: None,
    radius: Some(6),
};
```

## CSS 热重载工作流

```rust
use rust_widgets::widget::Widget;
use rust_widgets::core::Rect;
use rust_widgets::widget::Button;
use std::fs;
use std::time::{Duration, Instant};

fn css_hot_reload() {
    let mut button = Button::new("Styled Button".to_string(), Rect::new(0, 0, 200, 40));
    let css_path = "styles/button.css";
    let mut last_modified = fs::metadata(css_path)
        .and_then(|m| m.modified())
        .ok();

    loop {
        // 检查文件更改
        if let Ok(metadata) = fs::metadata(css_path) {
            if let Ok(modified) = metadata.modified() {
                if Some(modified) != last_modified {
                    // 重新加载 CSS
                    let css = fs::read_to_string(css_path).unwrap_or_default();
                    if !css.is_empty() {
                        let _ = button.apply_css(&css, Some("custom-button"));
                        println!("CSS hot-reloaded");
                    }
                    last_modified = Some(modified);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
        break;  // 示例：单次迭代
    }
}
```

## 使用 PerformanceMonitor 进行性能分析

```rust
use rust_widgets::performance::PerformanceMonitor;
use std::time::Duration;

// 全系统分析
let mut monitor = PerformanceMonitor::new();

// 帧级分析
monitor.begin_frame();

monitor.begin_section("layout");
// 布局工作...
monitor.end_section();

monitor.measure("render", || {
    // 被测量的代码块
});

monitor.end_frame();

// 生成报告
if monitor.is_enabled() {
    let report = monitor.report();
    println!("{}", report.to_string_summary());
    // 显示 FPS、平均/最小/最大帧时间以及各部分细分
}

// 用于热点检测的分析器
let profiler = monitor.profiler();
if let Some(avg) = profiler.get_average_duration("render") {
    if avg > Duration::from_millis(8) {
        eprintln!("警告：渲染流程平均时间 >8ms ({:?})", avg);
    }
}

// 重置以进行干净的测量
monitor.reset();
```

## 使用 TestHarness、WidgetTester、LayoutTester 进行测试

### TestHarness

```rust
use rust_widgets::test::TestHarness;
use rust_widgets::core::{Size, Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::widget::Label;

let mut harness = TestHarness::new()
    .with_screen_size(Size::new(1024, 768));

// 发送事件
harness.send_mouse_click(100, 100, 0);  // x, y, button
harness.send_mouse_move(200, 200);
harness.send_key_press(65, 0);           // 'A'
harness.send_key_release(65, 0);

// 分发给控件
let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 30));
let handled = harness.dispatch_to(&mut label);
println!("{} events dispatched", handled);
```

### WidgetTester

```rust
use rust_widgets::test::WidgetTester;
use rust_widgets::core::{Rect, Size, Point};
use rust_widgets::widget::Button;

let mut tester = WidgetTester::new(
    Button::new("Click Me".to_string(), Rect::new(0, 0, 100, 32))
);

// 链式断言
tester
    .assert_visible()
    .assert_enabled()
    .assert_geometry(Rect::new(0, 0, 100, 32))
    .assert_size(Size::new(100, 32))
    .assert_position(Point::new(0, 0));

// 模拟交互
tester.click(50, 16);   // 点击按钮中心
tester.move_mouse(10, 10);
tester.press_key(13);   // Enter 键

// 交互后检查状态
// assert_eq!(tester.widget().text(), "Clicked!");
```

### LayoutTester

```rust
use rust_widgets::test::LayoutTester;
use rust_widgets::core::Rect;

let tester = LayoutTester::new(Rect::new(0, 0, 400, 300));

// 测试布局函数
let positions = vec![
    Rect::new(0, 0, 100, 50),
    Rect::new(100, 0, 100, 50),
    Rect::new(200, 0, 100, 50),
];

tester.assert_fits_in_container(&positions);   // 所有都在 400×300 内
tester.assert_no_overlap(&positions);           // 无重叠

// 精确位置匹配
tester.test_layout(
    |container| {
        // 您的布局函数
        vec![Rect::new(0, 0, container.width / 2, container.height)]
    },
    &[Rect::new(0, 0, 200, 300)],
);
```

## 视觉回归的快照测试

```rust
use rust_widgets::test::{SnapshotManager, SnapshotConfig};

// 配置快照测试
let mut snapshots = SnapshotManager::new();

let config = SnapshotConfig {
    tolerance: 0.01,     // 每像素 1% 容差
    update: false,       // 设置为 true 以更新基线
    output_dir: "tests/snapshots/".to_string(),
};

// 对比渲染帧与基线
fn test_widget_rendering() {
    let rendered_frame = render_widget_to_buffer();
    // snapshots.compare("button_default", &rendered_frame, &config);
}
```

## 特性标志矩阵与组合测试

rust-widgets 使用三轴特性系统：

| 轴 | 示例标志 | 描述 |
|------|--------------|-------------|
| 设备配置 | `desktop`、`mini`、`embedded` | 功能集范围 |
| 操作系统后端 | `linux`、`windows`、`macos`、`wasm` | 平台后端 |
| 能力模块 | `gpu-wgpu`、`chart`、`pdf`、`i18n` | 可选模块 |

**测试组合：**

```rust
// 使用不同特性组合进行测试
#[test]
fn test_chart_without_gpu() {
    // 仅 chart 特性，不含 GPU
    // cargo test --features "chart"
}

#[test]
fn test_chart_with_gpu() {
    // 完整 chart + GPU
    // cargo test --features "chart,gpu-wgpu"
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_std_only_feature() {
    // 此测试仅在 desktop/embedded 配置下运行，不在 mini 下运行
}
```

## 为 no_std / mini Profile 构建

```toml
# Cargo.toml
[dependencies]
rust_widgets = { version = "1.0", default-features = false, features = [
    "mini",
] }

[profile.mini]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

`mini` 特性提供 `no_std` 就绪基础——所有模块经 `compat.rs` 从 `core`/`alloc` 导入共享类型，
启用 `#![no_std]` 是已跟踪的后续步骤。**注意**：当前 `mini` profile 在 std 上编译；
下面的属性是设计意图，尚未启用：

```rust
// 在 no_std 模式下，HashMap → BTreeMap（通过 compat.rs）
// Mutex → RefCell
// Vec → MiniVec
// String → MiniString
// 所有 trait 实现必须兼容 Send + Sync

// 真正的 no_std 构建设计意图；mini 当前在 std 上编译。

use rust_widgets::embedded::{
    EmbeddedConfig, ResourceManager, ResourceConstraint,
    LightweightWidget, LightweightConfig,
};
use rust_widgets::render::SoftwarePaintBackend;
use rust_widgets::core::{Size, Color};

fn mini_main() {
    let config = EmbeddedConfig::new(Size::new(320, 240))
        .low_memory();
    rust_widgets::embedded::init_embedded(config);

    let mut resources = ResourceManager::new(ResourceConstraint::Low);
    let mut backend = SoftwarePaintBackend::new(Size::new(320, 240), 1.0);

    loop {
        backend.begin_frame(Color::WHITE);
        // ... 渲染最小化 UI ...
        backend.end_frame();

        if resources.is_under_pressure() {
            // 紧急内存恢复
        }
    }
}
```

## 安全注意事项

1. **JavaScript 注入：** 显示不受信任的内容时，始终在 `WebViewEnhanced` / `WebEngineViewEnhanced` 上调用 `set_javascript_enabled(false)`。

2. **混合内容：** 使用 `SecuritySettings` 阻止混合 HTTP/HTTPS 内容：
   ```rust
   view.security_mut().allow_insecure_content = false;
   view.security_mut().allow_mixed_content = false;
   ```

3. **插件权限：** 使用 `PluginManager` 权限限制插件：
   ```rust
   manager.revoke_permission(plugin_id, PluginPermission::FileSystemAccess);
   manager.revoke_permission(plugin_id, PluginPermission::NetworkAccess);
   ```

4. **Cookie 隔离：** 使用带有严格域名范围的 `CookieJar`：
   ```rust
   let cookie = Cookie::new("session", token, "app.example.com");
   cookie.http_only = true;
   cookie.secure = true;
   ```

5. **隐私浏览：** 为敏感会话启用：
   ```rust
   engine.set_private_browsing(true);
   // 退出时清除 cookies、历史记录、缓存
   ```

6. **跟踪保护：** 阻止指纹识别和跟踪：
   ```rust
   let privacy = TrackingProtection::new(PrivacySettings::strict());
   ```

7. **内存安全：** 在安全关键路径中使用 `ArenaAllocator` 和 `ObjectPool` 以避免堆碎片。

8. **功能精简：** 禁用未使用的特性：
   ```toml
   default-features = false
   features = ["mini"]  # 最小攻击面
   ```

## 贡献指南

### 代码风格

```rust
// 遵循通过 rustfmt.toml 配置的项目格式化规则
// $ cargo fmt --all

// 使用 clippy 检查
// $ cargo clippy --all-features --all-targets
```

### 项目规则

- **不允许出现空的 `todo!()` 占位符** — 每个分支必须实现实际逻辑
- **验证所有符号配对**（`{}`、`()`、`[]`、`<>`）— 编辑前后均需验证
- **模块间无循环依赖** — 使用 trait/接口进行解耦
- **无未使用的导入或变量** — 在开发中启用 `#![deny(unused)]`
- **处理边界情况** — 对所有输入进行 `Option`、`Result`、边界检查
- **错误处理** — 优先使用 `Result<T, E>` 返回值而非直接 panic

### 测试要求

```rust
// 每个模块必须包含：
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // 核心正常路径
    }

    #[test]
    fn test_edge_cases() {
        // 空输入、零值、溢出
    }

    #[test]
    fn test_error_handling() {
        // 无效输入、越界、空状态
    }
}
```

### Pull Request 流程

1. 运行 `cargo test --all-features` 并验证所有测试通过
2. 运行 `cargo clippy --all-features --all-targets` 并修复警告
3. 运行 `cargo fmt --all --check`
4. 更新 `cookbook/` 目录中的相关文档
5. 为视觉变更添加快照测试

### 模块文档标准

每个公共模块必须包含：

```rust
//! 模块级文档，包含：
//! 1. 目的说明（一段话）
//! 2. 架构图或功能要点列表
//! 3. `# Examples` 部分，至少包含一个代码示例
//! 4. 如果受特性标志控制，则需要 `# Feature flags` 部分
```

## 总结

| 主题 | 关键组件 |
|-------|---------------|
| 自定义控件 | `Widget` + `EventHandler` + `Draw` trait |
| 自定义布局 | `Layout` trait，包含 `update()`、`add_widget()` |
| 自定义后端 | `PaintBackend` trait（fill_rect、draw_rect 等） |
| 控制后端 | `Dispatcher` trait 用于调度策略 |
| 信号/槽 | `ConnectionScope`、`connect_once()`、重入安全性 |
| 数据绑定 | `Binding<T>`、`Computed<T>`、`FnListener` |
| 自定义图表 | `Chart` trait + `ChartContext` |
| 自定义主题 | `ThemeManager`、`Theme`、`ThemeStyleToken` |
| CSS 热重载 | `Widget::apply_css()` + 文件监听器 |
| 性能分析 | `PerformanceMonitor`、`Profiler`、`FrameProfiler` |
| 测试 | `TestHarness`、`WidgetTester`、`LayoutTester` |
| 快照测试 | `SnapshotManager`、逐像素容差 |
| 特性矩阵 | 3 个轴 × 多个值 = 组合测试 |
| no_std 构建 | `mini` 配置、`MiniVec`、`MiniString` |
| 安全 | 禁用 JS、混合内容、插件权限、Cookie 隔离 |
