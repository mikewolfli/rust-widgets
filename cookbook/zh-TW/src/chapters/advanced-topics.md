# 進階主題

本章涵蓋自訂控制項、自訂佈局管理器、繪製後端、控制後端、進階信號/槽模式、資料繫結、自訂主題、CSS 熱重載、效能分析、測試、no_std 建置、安全考量，以及貢獻指南。

## 建立自訂控制項

一個自訂控制項需要實作三個特徵（trait）：`Widget`、`EventHandler`，以及選擇性的 `Draw`（用於自訂繪製）。

### 實作 Widget + EventHandler + Draw

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
            // 從點擊位置計算數值
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

        // 背景軌道
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height, Color::LIGHT_GRAY);

        // 已填充部分
        let filled_width = (rect.width as f32 * pct) as u32;
        ctx.fill_rect(
            rect.x,
            rect.y,
            filled_width,
            rect.height,
            self.color,
        );

        // 邊框
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

### 支援信號的控制項

```rust
use rust_widgets::signal::{Signal1, GenericSignal};

struct Slider {
    base: BaseWidget,
    value: f32,
    pub value_changed: Signal1<f32>,
}

impl Draw for Slider {
    fn draw(&mut self, _ctx: &mut RenderContext) {
        // ... 自訂繪製邏輯 ...
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

## 自訂佈局管理器

實作 `Layout` 特徵來建立自訂佈局演算法：

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
// circular.update(container_rect, &mut |id, rect| { /* 套用 */ });
```

## 自訂繪製後端

實作 `PaintBackend` 特徵來加入自訂渲染後端：

```rust
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::core::{Color, Size, Rect, Point};

struct CustomPaintBackend {
    width: u32,
    height: u32,
    scale: f32,
    buffer: Vec<u32>,  // 自訂像素格式
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
        // 將顏色轉換為 u32 ABGR
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
        // 繪製四條邊 ...
    }

    fn end_frame(&mut self) {
        // 將緩衝區刷新到實際顯示器
        // platform::display_write(&self.buffer);
    }
}

fn pixel_from_color(c: Color) -> u32 {
    (c.a as u32) << 24 | (c.b as u32) << 16 | (c.g as u32) << 8 | c.r as u32
}

// 透過 RenderContext 使用：
// let mut backend = CustomPaintBackend::new(Size::new(800, 600), 1.0);
// backend.begin_frame(Color::WHITE);
// let mut ctx = RenderContext::new(&mut backend);
// ctx.fill_rect(0, 0, 100, 50, Color::RED);
// backend.end_frame();
```

## 控制後端自訂

控制後端負責管理控制項的分派策略與路由：

```rust
use rust_widgets::control_backend::Dispatcher;
use rust_widgets::core::ObjectId;

// 設定分派策略
struct CustomDispatchPolicy;

impl Dispatcher for CustomDispatchPolicy {
    fn should_dispatch_to_widget(&self, widget_id: ObjectId, action: &str) -> bool {
        // 自訂路由邏輯
        !action.contains("internal")
    }

    fn route_action(&self, widget_id: ObjectId, action: &str) -> Option<ObjectId> {
        // 將動作路由至特定處理器
        None  // 預設路由
    }
}
```

## 信號/槽進階模式

### ConnectionScope — 自動斷開連線

```rust
use rust_widgets::signal::{ConnectionScope, Signal, GenericSignal};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

// 連線範圍隸屬於擁有者生命週期
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

// 當擁有者被釋放後，連線會自動中斷
signal.emit();
assert_eq!(hits.load(Ordering::SeqCst), 1);  // 不會再增加
```

### 一次性信號

```rust
let signal = GenericSignal::new();
let hits = Arc::new(AtomicUsize::new(0));

{
    let h = Arc::clone(&hits);
    signal.connect_once(move || {
        h.fetch_add(1, Ordering::SeqCst);
    });
}

signal.emit();  // 觸發
signal.emit();  // 無作用——首次觸發後已斷開連線
assert_eq!(hits.load(Ordering::SeqCst), 1);
```

### 可重入安全性

信號發射具備可重入安全性——在發射期間連線新的槽（slot）不會造成死結：

```rust
let signal = Signal::<u32>::new();
let emitted = Arc::new(AtomicUsize::new(0));

let e1 = Arc::clone(&emitted);
let e2 = Arc::clone(&emitted);
let s2 = signal.clone();

signal.connect(move |v| {
    e1.fetch_add(1, Ordering::SeqCst);
    if *v == 1 {
        // 在發射期間連線另一個槽（可重入）
        s2.connect(move |_| {
            e2.fetch_add(1, Ordering::SeqCst);
        });
    }
});

signal.emit(1);  // 首次發射——新槽被連線
signal.emit(2);  // 第二次發射——兩個槽皆觸發

assert_eq!(emitted.load(Ordering::SeqCst), 3);
```

### 帶型別資料的信號

```rust
use rust_widgets::signal::Signal1;
use std::sync::Arc;

let signal = Signal1::<String>::new();

signal.connect(|msg: Arc<String>| {
    println!("Received: {}", msg);
});

signal.emit("Hello!".to_string());
```

## 雙向資料繫結

rust-widgets 提供了一個反應式資料繫結系統，適用於 MVVM 風格的使用者介面：

```rust
use rust_widgets::data_binding::{Binding, FnListener, Computed};

// 單一反應式數值
let mut name = Binding::new("World".to_string());

name.subscribe("log", Box::new(FnListener::new(|key| {
    println!("[{}] changed!", key);
})));

name.set("Rust".to_string());
assert_eq!(name.get(), "Rust");

// 衍生／計算值
let mut full_name = Binding::new("John Doe".to_string());
let mut greeting = Computed::new(|| {
    format!("Hello, {}!", full_name.get())
});

assert_eq!(greeting.get(), "Hello, John Doe!");
full_name.set("Jane Doe".to_string());
greeting.invalidate();
assert_eq!(greeting.get(), "Hello, Jane Doe!");
```

## 自訂圖表類型

實作圖表特徵來建立自訂視覺化元件：

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
        // 存取繪製上下文來進行繪製
        // ctx.fill_rect(...);
        for point in &self.data {
            // 繪製每個資料點
        }
    }
}
```

## 自訂主題 — 擴充 ThemeManager

```rust
use rust_widgets::theme::{ThemeManager, Theme, Colors, Fonts, Spacing, Borders, ThemeOverrides};
use rust_widgets::core::{Color, Font};
use std::collections::HashMap;

let mut theme_manager = ThemeManager::default();

// 存取目前的主題
let current = theme_manager.current_theme().unwrap();
println!("Current theme: {}", current.name);

// 建立自訂主題
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

// 使用內建的深色主題
let dark = Theme::dark();
println!("Dark theme background: {:?}", dark.colors.background);

// ThemeStyleToken 用於按類別覆寫
use rust_widgets::theme::ThemeStyleToken;
let button_override = ThemeStyleToken {
    background: Some(Color::from_hex("#0052CC").unwrap()),
    foreground: Some(Color::WHITE),
    border: None,
    border_width: None,
    radius: Some(6),
};
```

## CSS 熱重載工作流程

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
        // 檢查檔案變更
        if let Ok(metadata) = fs::metadata(css_path) {
            if let Ok(modified) = metadata.modified() {
                if Some(modified) != last_modified {
                    // 重新載入 CSS
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
        break;  // 範例：單次疊代
    }
}
```

## 使用 PerformanceMonitor 進行效能分析

```rust
use rust_widgets::performance::PerformanceMonitor;
use std::time::Duration;

// 全系統效能分析
let mut monitor = PerformanceMonitor::new();

// 畫面層級分析
monitor.begin_frame();

monitor.begin_section("layout");
// 佈局工作...
monitor.end_section();

monitor.measure("render", || {
    // 被測量的程式區塊
});

monitor.end_frame();

// 產生報告
if monitor.is_enabled() {
    let report = monitor.report();
    println!("{}", report.to_string_summary());
    // 顯示 FPS、平均/最小/最大畫面時間，以及各區段分析
}

// 用於熱點偵測的分析器
let profiler = monitor.profiler();
if let Some(avg) = profiler.get_average_duration("render") {
    if avg > Duration::from_millis(8) {
        eprintln!("Warning: render pass average >8ms ({:?})", avg);
    }
}

// 重設以進行乾淨的測量
monitor.reset();
```

## 使用 TestHarness、WidgetTester、LayoutTester 進行測試

### TestHarness

```rust
use rust_widgets::test::TestHarness;
use rust_widgets::core::{Size, Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::widget::Label;

let mut harness = TestHarness::new()
    .with_screen_size(Size::new(1024, 768));

// 發送事件
harness.send_mouse_click(100, 100, 0);  // x, y, 按鈕
harness.send_mouse_move(200, 200);
harness.send_key_press(65, 0);           // 'A'
harness.send_key_release(65, 0);

// 分派至控制項
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

// 可鏈式斷言
tester
    .assert_visible()
    .assert_enabled()
    .assert_geometry(Rect::new(0, 0, 100, 32))
    .assert_size(Size::new(100, 32))
    .assert_position(Point::new(0, 0));

// 模擬互動
tester.click(50, 16);   // 點擊按鈕中央
tester.move_mouse(10, 10);
tester.press_key(13);   // Enter 鍵

// 檢查互動後的狀態
// assert_eq!(tester.widget().text(), "Clicked!");
```

### LayoutTester

```rust
use rust_widgets::test::LayoutTester;
use rust_widgets::core::Rect;

let tester = LayoutTester::new(Rect::new(0, 0, 400, 300));

// 測試佈局函式
let positions = vec![
    Rect::new(0, 0, 100, 50),
    Rect::new(100, 0, 100, 50),
    Rect::new(200, 0, 100, 50),
];

tester.assert_fits_in_container(&positions);   // 全部在 400×300 範圍內
tester.assert_no_overlap(&positions);           // 無重疊

// 精確位置比對
tester.test_layout(
    |container| {
        // 你的佈局函式
        vec![Rect::new(0, 0, container.width / 2, container.height)]
    },
    &[Rect::new(0, 0, 200, 300)],
);
```

## 視覺回歸的快照測試

```rust
use rust_widgets::test::{SnapshotManager, SnapshotConfig};

// 設定快照測試
let mut snapshots = SnapshotManager::new();

let config = SnapshotConfig {
    tolerance: 0.01,     // 1% 逐像素容差
    update: false,       // 設為 true 以更新基準
    output_dir: "tests/snapshots/".to_string(),
};

// 比對渲染畫面與基準
fn test_widget_rendering() {
    let rendered_frame = render_widget_to_buffer();
    // snapshots.compare("button_default", &rendered_frame, &config);
}
```

## 功能旗標矩陣與組合測試

rust-widgets 使用三軸功能系統：

| 軸 | 範例旗標 | 說明 |
|------|--------------|-------------|
| 裝置設定檔 | `desktop`、`mini`、`embedded` | 功能集範圍 |
| 作業系統後端 | `linux`、`windows`、`macos`、`wasm` | 平台後端 |
| 功能模組 | `gpu-wgpu`、`chart`、`pdf`、`i18n` | 選擇性模組 |

**測試組合：**

```rust
// 測試不同的功能組合
#[test]
fn test_chart_without_gpu() {
    // 僅 chart 功能，不含 GPU
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
    // 此測試僅在 desktop/embedded 設定檔下執行，不包含 mini
}
```

## 建置為 no_std / mini 設定檔

```toml
# Cargo.toml
[dependencies]
rust_widgets = { version = "0.9", default-features = false, features = [
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

`mini` 功能提供 `no_std` 相容性：

```rust
// 在 no_std 模式下，HashMap → BTreeMap（透過 compat.rs）
// Mutex → RefCell
// Vec → MiniVec
// String → MiniString
// 所有特徵實作必須與 Send + Sync 相容

#![no_std]
extern crate alloc;

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
            // 緊急記憶體回收
        }
    }
}
```

## 安全考量

1. **JavaScript 注入：** 在顯示不受信任的內容時，務必在 `WebViewEnhanced` / `WebEngineViewEnhanced` 上呼叫 `set_javascript_enabled(false)`。

2. **混合內容：** 使用 `SecuritySettings` 來封鎖混合 HTTP/HTTPS 內容：
   ```rust
   view.security_mut().allow_insecure_content = false;
   view.security_mut().allow_mixed_content = false;
   ```

3. **外掛權限：** 使用 `PluginManager` 權限來限制外掛：
   ```rust
   manager.revoke_permission(plugin_id, PluginPermission::FileSystemAccess);
   manager.revoke_permission(plugin_id, PluginPermission::NetworkAccess);
   ```

4. **Cookie 隔離：** 使用具備嚴格網域範圍的 `CookieJar`：
   ```rust
   let cookie = Cookie::new("session", token, "app.example.com");
   cookie.http_only = true;
   cookie.secure = true;
   ```

5. **私密瀏覽：** 為敏感工作階段啟用：
   ```rust
   engine.set_private_browsing(true);
   // 結束時清除 cookies、瀏覽歷程、快取
   ```

6. **追蹤防護：** 封鎖指紋辨識與追蹤：
   ```rust
   let privacy = TrackingProtection::new(PrivacySettings::strict());
   ```

7. **記憶體安全：** 在安全關鍵路徑中使用 `ArenaAllocator` 和 `ObjectPool` 以避免堆積碎片化。

8. **功能精簡：** 停用不需要的功能：
   ```toml
   default-features = false
   features = ["mini"]  # 最小攻擊面
   ```

## 貢獻指南

### 程式碼風格

```rust
// 遵循專案的 rustfmt.toml 格式化設定
// $ cargo fmt --all

// 使用 clippy 進行檢查
// $ cargo clippy --all-features --all-targets
```

### 專案規則

- **禁止空白的 `todo!()` 佔位符** — 每個分支都必須實作實際邏輯
- **在編輯前後驗證所有符號配對**（`{}`、`()`、`[]`、`<>`）
- **模組之間不得有循環依賴** — 使用特徵／介面進行解耦
- **不得有未使用的匯入或變數** — 在開發中啟用 `#![deny(unused)]`
- **處理邊界情況** — 所有輸入都需檢查 `Option`、`Result`、邊界
- **錯誤處理** — 優先使用 `Result<T, E>` 回傳而非 panic

### 測試需求

```rust
// 每個模組都必須包含：
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // 核心正常路徑
    }

    #[test]
    fn test_edge_cases() {
        // 空輸入、零值、溢位
    }

    #[test]
    fn test_error_handling() {
        // 無效輸入、越界、空值狀態
    }
}
```

### 拉取請求流程

1. 執行 `cargo test --all-features` 並確認所有測試通過
2. 執行 `cargo clippy --all-features --all-targets` 並修正警告
3. 執行 `cargo fmt --all --check`
4. 更新 `cookbook/` 目錄中的相關文件
5. 為視覺變更新增快照測試

### 模組文件標準

每個公開模組必須包含：

```rust
//! 模組層級文件，包含：
//! 1. 用途說明（一段落）
//! 2. 架構圖或條列式功能列表
//! 3. 附有至少一個程式碼範例的 `# Examples` 區段
//! 4. 如有條件編譯，則包含 `# Feature flags` 區段
```

## 總結

| 主題 | 關鍵元件 |
|-------|---------------|
| 自訂控制項 | `Widget` + `EventHandler` + `Draw` 特徵 |
| 自訂佈局 | `Layout` 特徵，包含 `update()`、`add_widget()` |
| 自訂後端 | `PaintBackend` 特徵（fill_rect、draw_rect 等） |
| 控制後端 | `Dispatcher` 特徵用於分派策略 |
| 信號/槽 | `ConnectionScope`、`connect_once()`、可重入安全性 |
| 資料繫結 | `Binding<T>`、`Computed<T>`、`FnListener` |
| 自訂圖表 | `Chart` 特徵 + `ChartContext` |
| 自訂主題 | `ThemeManager`、`Theme`、`ThemeStyleToken` |
| CSS 熱重載 | `Widget::apply_css()` + 檔案監聽器 |
| 效能分析 | `PerformanceMonitor`、`Profiler`、`FrameProfiler` |
| 測試 | `TestHarness`、`WidgetTester`、`LayoutTester` |
| 快照測試 | `SnapshotManager`、逐像素容差 |
| 功能矩陣 | 3 軸 × 多值 = 組合測試 |
| no_std 建置 | `mini` 設定檔、`MiniVec`、`MiniString` |
| 安全 | 停用 JS、混合內容、外掛權限、Cookie 隔離 |
