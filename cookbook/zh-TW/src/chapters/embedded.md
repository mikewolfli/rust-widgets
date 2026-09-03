# 嵌入式支援

rust-widgets 透過 no_std 就緒設定檔（`mini`，目前在 std 上編譯）支援嵌入式及資源受限目標平臺，該設定檔包含 `AtomicBool` 全域旗標、固定 DPI 模式、輕量級控制項工廠、硬體輸入處理以及自適應資源管理。

## 架構概覽

```
┌──────────────────────────────────────────────┐
│  AtomicBool 全域旗標：embedded_mode,          │
│  low_memory_mode, fixed_dpi                  │
├──────────────────────────────────────────────┤
│  EmbeddedConfig  →  ResourceManager          │
│  (螢幕, DPI,     →  (控制項上限,            │
│   開關)              記憶體上限)              │
├──────────────────────┬───────────────────────┤
│  HardwareInput       │  LightweightWidget    │
│  (TouchPoint,        │  (最小堆積記憶體,      │
│   手勢, 篩選器)      │  簡化特效)            │
├──────────────────────┼───────────────────────┤
│  DPI 管理            │  WidgetPool<T>        │
│  (固定 DPI, 縮放)    │  (物件池)             │
├──────────────────────┼───────────────────────┤
│  LightweightStyle    │  LightweightFactory   │
│  (緊湊預設值)        │  (速率限制)           │
└──────────────────────┴───────────────────────┘
```

## 嵌入式模式 — AtomicBool 全域旗標

三個全域 `AtomicBool` 旗標控制嵌入式子系統，無需全域上下文物件：

```rust
use rust_widgets::embedded;

// 檢查目前模式
println!("Embedded: {}", embedded::is_embedded_mode());
println!("Low memory: {}", embedded::is_low_memory_mode());

// 啟用嵌入式模式
embedded::set_embedded_mode(true);
assert!(embedded::is_embedded_mode());

// 啟用低記憶體模式
embedded::set_low_memory_mode(true);
assert!(embedded::is_low_memory_mode());

// 切換回來
embedded::set_embedded_mode(false);
embedded::set_low_memory_mode(false);
```

**自適應常數**根據模式自動調整：

```rust
use rust_widgets::embedded;
use rust_widgets::core::Size;

// 建議的緩衝區大小
embedded::set_low_memory_mode(true);
let low_size = embedded::recommended_buffer_size();
assert_eq!(low_size, Size::new(800, 600));

embedded::set_low_memory_mode(false);
let normal_size = embedded::recommended_buffer_size();
assert_eq!(normal_size, Size::new(1920, 1080));

// 紋理大小限制
embedded::set_embedded_mode(true);
assert_eq!(embedded::max_texture_size(), 1024);  // 嵌入式受限

embedded::set_embedded_mode(false);
assert_eq!(embedded::max_texture_size(), 4096);  // 桌面級

// 字型快取大小
embedded::set_low_memory_mode(true);
assert_eq!(embedded::font_cache_size(), 256 * 1024);  // 256 KiB

embedded::set_low_memory_mode(false);
assert_eq!(embedded::font_cache_size(), 2 * 1024 * 1024);  // 2 MiB

// 事件佇列大小
embedded::set_embedded_mode(true);
assert_eq!(embedded::event_queue_size(), 64);  // 受限

embedded::set_embedded_mode(false);
assert_eq!(embedded::event_queue_size(), 256);  // 標準
```

### init_embedded / init_desktop

單次呼叫即可初始化環境：

```rust
use rust_widgets::embedded::{init_embedded, init_desktop, EmbeddedConfig};
use rust_widgets::core::Size;

// 為嵌入式目標初始化，使用固定 DPI
let config = EmbeddedConfig::new(Size::new(1024, 768))
    .with_fixed_dpi(96)
    .low_memory();
init_embedded(config);
assert!(embedded::is_embedded_mode());
assert!(embedded::is_low_memory_mode());

// 切換回桌面模式
init_desktop();
assert!(!embedded::is_embedded_mode());
assert!(!embedded::is_low_memory_mode());
```

## EmbeddedConfig

```rust
use rust_widgets::embedded::EmbeddedConfig;
use rust_widgets::core::Size;

// 建造者模式
let config = EmbeddedConfig::new(Size::new(800, 600))
    .with_fixed_dpi(160)          // 160 DPI（2 倍密度）
    .low_memory()                  // 啟用低記憶體最佳化
    .with_max_widgets(50)          // 上限 50 個控制項
    .with_touch(true)              // 啟用觸控輸入
    .with_hardware_acceleration(false) // 軟體渲染
    .with_font_scale(1.2);         // 120% 字型大小

println!("Screen: {}×{}", config.screen_size.width, config.screen_size.height);
println!("Fixed DPI: {:?}", config.fixed_dpi);
println!("Low memory: {}", config.low_memory_mode);
println!("Max widgets: {}", config.max_widgets);
println!("Animations: {}", config.enable_animations);
println!("Touch: {}", config.touch_enabled);
println!("Font scale: {}", config.font_scale);
```

**`.low_memory()` 內部設定的參數：**

| 設定項 | 預設值 | 呼叫 `.low_memory()` 後 |
|---------|---------|----------------------|
| `max_widgets` | 100 | 50 |
| `max_texture_size` | 1024 | 512 |
| `enable_animations` | true | false |
| `enable_shadows` | false | false |
| `enable_gradients` | true | false |

## ResourceManager — 控制項數量限制與記憶體限制

```rust
use rust_widgets::embedded::{ResourceManager, ResourceConstraint};

// 約束級別決定限制值
let mut rm = ResourceManager::new(ResourceConstraint::Low);
// Low：   16 MiB 記憶體，50 個控制項
// Medium：64 MiB 記憶體，200 個控制項
// High： 256 MiB 記憶體，1000 個控制項
// None：  無限制

// 記憶體分配
assert!(rm.can_allocate(1024));
assert!(rm.allocate(1024));
assert_eq!(rm.memory_usage(), 1024);
assert_eq!(rm.memory_percentage(), (1024.0_f32 / (16.0 * 1024.0 * 1024.0)) * 100.0);

// 釋放記憶體
rm.deallocate(512);
assert_eq!(rm.memory_usage(), 512);

// 控制項追蹤
assert!(rm.can_create_widget());
assert!(rm.register_widget());
assert_eq!(rm.widget_count(), 1);
rm.unregister_widget();
assert_eq!(rm.widget_count(), 0);

// 控制項上限強制執行
for _ in 0..50 {
    assert!(rm.register_widget());
}
assert!(!rm.register_widget());  // 超過 max_widgets（50）
assert_eq!(rm.widget_count(), 50);

// 壓力檢測
assert!(rm.is_under_pressure());  // 50/50 個控制項 = 100% > 90%
```

**整合模式：**

```rust
fn create_widget(rm: &mut ResourceManager, memory_needed: usize) -> Option<WidgetHandle> {
    if !rm.can_create_widget() {
        eprintln!("Widget limit reached ({}/{})", rm.widget_count(), rm.widget_count());
        return None;
    }
    if !rm.allocate(memory_needed) {
        eprintln!(
            "Memory limit exceeded ({:.1}% used)",
            rm.memory_percentage()
        );
        return None;
    }
    rm.register_widget();
    Some(WidgetHandle::new())
}

// 控制項銷毀時
fn destroy_widget(rm: &mut ResourceManager, memory_freed: usize) {
    rm.deallocate(memory_freed);
    rm.unregister_widget();
}
```

## DPI 管理

固定 DPI 模式使用全域 `AtomicU32` 狀態，適用於顯示螢幕 DPI 永遠不會變化的環境（嵌入式面板、固定顯示器）：

```rust
use rust_widgets::embedded;

// 設定固定 DPI
embedded::set_fixed_dpi(192);  // 2 倍密度面板
assert!(embedded::is_fixed_dpi());
assert_eq!(embedded::get_fixed_dpi(), Some(192));

// 縮放因子：DPI / 96
assert!((embedded::scale_factor() - 2.0).abs() < 0.01);

// 縮放函式
assert_eq!(embedded::scale(100), 200);       // 100 × 2.0 = 200
assert_eq!(embedded::scale_u32(100), 200);   // u32 版本
assert!((embedded::scale_f32(50.0) - 100.0).abs() < 0.01);

// 點 ↔ 像素轉換（1pt = 1/72 英寸）
let px = embedded::points_to_pixels(12.0, 96);
assert!((px - 16.0).abs() < 0.01);  // 12pt at 96dpi = 16px

let pt = embedded::pixels_to_points(16.0, 96);
assert!((pt - 12.0).abs() < 0.01);  // 16px at 96dpi = 12pt

// 清除固定 DPI — 恢復為系統偵測的 DPI
embedded::clear_fixed_dpi();
assert!(!embedded::is_fixed_dpi());
assert_eq!(embedded::get_fixed_dpi(), None);
```

### DpiScaler

`DpiScaler` 結構體提供了一個區域性、堆疊分配的替代方案，無需使用全域固定 DPI：

```rust
use rust_widgets::embedded::DpiScaler;

let scaler = DpiScaler::new(144)              // 144 DPI（1.5 倍）
    .with_base_dpi(96);

assert!((scaler.scale_factor() - 1.5).abs() < 0.01);
assert_eq!(scaler.scale(100), 150);
assert_eq!(scaler.scale_u32(100), 150);
assert!((scaler.scale_f32(100.0) - 150.0).abs() < 0.01);

// 反向縮放（像素 → 邏輯值）
assert_eq!(scaler.unscale(150), 100);
assert_eq!(scaler.unscale_u32(150), 100);
assert!((scaler.unscale_f32(150.0) - 100.0).abs() < 0.01);
```

## 硬體輸入：觸控與手勢

### TouchPoint

```rust
use rust_widgets::embedded::TouchPoint;

let point = TouchPoint::new(1, 100, 200)    // id=1, x=100, y=200
    .with_pressure(0.8);                     // 80% 壓力

assert_eq!(point.id, 1);
assert_eq!(point.position.x, 100);
assert_eq!(point.position.y, 200);
assert!((point.pressure - 0.8).abs() < 0.01);
```

### HardwareInputManager

處理多點觸控、32 個硬體按鈕以及手勢偵測：

```rust
use rust_widgets::embedded::{
    HardwareInputManager, TouchPoint, TouchEvent,
    GestureType, InputType,
};

let mut manager = HardwareInputManager::new();

// 處理觸控按下
let point = TouchPoint::new(1, 100, 100);
manager.process_touch(TouchEvent::Down, point);
assert_eq!(manager.touch_point_count(), 1);

// 處理觸控移動
let moved = TouchPoint::new(1, 120, 100);
manager.process_touch(TouchEvent::Move, moved);

// 處理觸控抬起 — 自動偵測手勢
manager.process_touch(TouchEvent::Up, moved);

// 輪詢偵測到手勢
while let Some(gesture) = manager.get_gesture() {
    match gesture.gesture_type {
        GestureType::Tap => println!("Tap at ({}, {})", gesture.center.x, gesture.center.y),
        GestureType::SwipeRight => println!("Swipe right, velocity: {:?}", gesture.velocity),
        GestureType::LongPress => println!("Long press at ({}, {})", gesture.center.x, gesture.center.y),
        _ => println!("Gesture: {:?}", gesture.gesture_type),
    }
}

// 硬體按鈕（最多 32 個）
manager.process_button(0, true);   // 按鈕 0 按下
assert!(manager.is_button_pressed(0));
manager.process_button(0, false);  // 釋放
assert!(!manager.is_button_pressed(0));

// 觸控取消
manager.process_touch(TouchEvent::Cancel, point);
manager.clear();
```

**手勢偵測閾值：**
- **輕點：** 時長 < 200ms，距離 < 50px
- **長按：** 時長 ≥ 500ms，距離 < 50px
- **滑動：** 距離 ≥ 50px — 方向由主座標軸決定

### InputFilter

`InputFilter` 提供壓力閾值篩選、死區濾波以及位置平滑：

```rust
use rust_widgets::embedded::{InputFilter, TouchPoint};

let mut filter = InputFilter::new()
    .with_dead_zone(10);  // 10px 死區

// 首次觸控直接通過（無前一次位置）
let point1 = TouchPoint::new(1, 100, 100);
let result1 = filter.filter_touch(&point1);
assert!(result1.is_some());

// 死區內的微小移動 → 被篩選掉
let point2 = TouchPoint::new(1, 105, 105);  // dx=5, dy=5 < 10
let result2 = filter.filter_touch(&point2);
assert!(result2.is_none());

// 超出死區的明顯移動 → 平滑處理
let point3 = TouchPoint::new(1, 150, 150);  // dx=50, dy=50
let result3 = filter.filter_touch(&point3);
assert!(result3.is_some());
// 位置被平滑處理：100 + 0.5×(150-100) = 125

// 壓力閾值：低於 min_pressure 的觸控被篩選
let weak = TouchPoint::new(2, 200, 200).with_pressure(0.05);
assert!(filter.filter_touch(&weak).is_none());  // 低於 min_pressure（0.1）

filter.reset();  // 清除狀態
```

## LightweightWidget — 資源受限渲染

```rust
use rust_widgets::embedded::{
    LightweightWidget, LightweightConfig, LightweightStyle,
};
use rust_widgets::widget::Label;
use rust_widgets::core::Rect;

// 將任意 Widget 包裝在輕量外殼中
let label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 30));
let lw = LightweightWidget::new(label)
    .with_config(LightweightConfig::minimal());

// 存取內部控制項
println!("Inner widget kind: {:?}", lw.inner().kind());

// 解包裝
let label = lw.into_inner();
```

**LightweightConfig 預設：**

```rust
// 彈性設定
let config = LightweightConfig::new()
    .with_shadows_disabled()
    .with_animations_disabled()
    .with_gradients_disabled();

// 或使用最小化預設
let minimal = LightweightConfig::minimal();
assert!(minimal.disable_shadows);
assert!(minimal.disable_animations);
assert!(minimal.disable_gradients);
assert!(minimal.simple_borders);
assert!(minimal.reduced_padding);
assert!(minimal.minimal_signals);

// 使用最小化設定建立控制項
let lw = LightweightWidget::new(label)
    .with_config(LightweightConfig::minimal());
```

### LightweightStyle — 最小堆積記憶體使用

```rust
use rust_widgets::embedded::LightweightStyle;

let style = LightweightStyle::new();
assert_eq!(style.padding, 4);
assert_eq!(style.font_size, 12);

let compact = LightweightStyle::compact();
assert_eq!(compact.padding, 2);
assert_eq!(compact.font_size, 10);
assert_eq!(compact.border_width, 1);
assert_eq!(compact.text_color, Some(0x000000));
assert_eq!(compact.border_color, Some(0x808080));
```

## LightweightWidgetFactory — 速率限制的控制項建立

```rust
use rust_widgets::embedded::{
    LightweightWidgetFactory, LightweightConfig,
};
use rust_widgets::widget::Label;
use rust_widgets::core::Rect;

let mut factory = LightweightWidgetFactory::new()
    .with_config(LightweightConfig::minimal())
    .with_max_widgets(5);

// 建立控制項（超出上限時回傳 None）
for i in 0..10 {
    if let Some(widget) = factory.create(|| {
        Label::new(format!("Widget {}", i), Rect::new(0, 0, 100, 30))
    }) {
        println!("Created widget {}", i);
    } else {
        println!("Widget limit reached at {}", i);
    }
}

assert_eq!(factory.widget_count(), 5);

// 釋放一個插槽
factory.release();
assert_eq!(factory.widget_count(), 4);
assert!(factory.can_create());  // 現在可以再建立一個
```

## WidgetPool&lt;T&gt; — 控制項物件池

```rust
use rust_widgets::embedded::WidgetPool;

let mut pool: WidgetPool<i32> = WidgetPool::new(3);  // 最多 3 個池化項目

// 取得項目（透過工廠閉包建立）
let h1 = pool.acquire(|| 1);
assert!(h1.is_some());
assert_eq!(pool.used_count(), 1);
assert_eq!(pool.available_count(), 0);  // 所有預分配插槽已用

let h2 = pool.acquire(|| 2);
assert!(h2.is_some());
assert_eq!(pool.used_count(), 2);

// 透過控制代碼存取項目
let h1_ref = h1.as_ref().unwrap();
assert_eq!(*pool.get(h1_ref.index()).unwrap(), 1);

// 釋放控制代碼 → 插槽回到池中
drop(h1);
assert_eq!(pool.used_count(), 1);

// 釋放的插槽可被重用
let h3 = pool.acquire(|| 3);
assert!(h3.is_some());

// 池已滿 — 後續取得回傳 None
let h4 = pool.acquire(|| 4);
assert!(h4.is_none());
```

## 低記憶體模式 — 建議限制

啟用 `low_memory_mode` 後，框架會自動調整：

| 資源 | 標準 | 低記憶體 |
|----------|----------|------------|
| 緩衝區大小 | 1920×1080 | 800×600 |
| 最大紋理大小 | 4096 | 1024（使用 `.low_memory()` 時為 512） |
| 字型快取 | 2 MiB | 256 KiB |
| 事件佇列 | 256 | 64 |
| 最大控制項數（預設） | 100 | 50 |
| 動畫 | 啟用 | 停用 |
| 陰影 | 停用 | 停用 |
| 漸層 | 啟用 | 停用 |

## 建置嵌入式目標

### Release-embedded 設定檔

新增到 `Cargo.toml`：

```toml
[profile.release-embedded]
inherits = "release"
opt-level = "s"           # 最佳化體積
lto = true                # 連結時最佳化
codegen-units = 1         # 單個程式碼生成單元以最佳化 LTO
strip = true              # 剝離除錯符號
panic = "abort"           # 不展開（更小的二進位檔）
```

使用 `mini` 功能建置：

```sh
cargo build --profile release-embedded --no-default-features \
  --features "mini,embedded" --target thumbv7em-none-eabihf
```

### 功能旗標設定

```toml
[dependencies]
rust_widgets = { version = "1.0", default-features = false, features = [
    "mini",          # no_std 就緒設定，heapless 支撐的 MiniVec
    "embedded",      # 嵌入式模式 + 輕量控制項
] }
```

### 建議的 `mini` 功能用法

`mini` 功能透過 `compat.rs` 將 std 型別替換為競技場分配和無堆積的替代方案：
- `HashMap` → `BTreeMap`
- `Mutex` → `RefCell`
- `Vec` → `MiniVec`
- `String` → `MiniString`

## 完整嵌入式渲染迴圈

一個結合了所有概念的最小嵌入式渲染迴圈：

```rust
use rust_widgets::embedded::{
    self, EmbeddedConfig, ResourceManager, ResourceConstraint,
    HardwareInputManager, TouchPoint, TouchEvent,
    LightweightWidgetFactory, LightweightConfig,
    InputFilter,
};
use rust_widgets::render::{SoftwarePaintBackend, PaintBackend, RenderContext};
use rust_widgets::core::{Size, Color, Rect, Point};
use rust_widgets::widget::Label;

fn embedded_main() {
    // 1. 初始化嵌入式模式
    let config = EmbeddedConfig::new(Size::new(800, 480))
        .with_fixed_dpi(120)
        .low_memory();
    embedded::init_embedded(config);

    // 2. 設定資源管理
    let mut resources = ResourceManager::new(ResourceConstraint::Low);

    // 3. 輸入處理
    let mut input = HardwareInputManager::new();
    let mut filter = InputFilter::new().with_dead_zone(8);

    // 4. 控制項工廠
    let mut factory = LightweightWidgetFactory::new()
        .with_config(LightweightConfig::minimal())
        .with_max_widgets(20);

    // 5. 渲染後端
    let buf_size = embedded::recommended_buffer_size();
    let mut backend = SoftwarePaintBackend::new(buf_size, 1.0);

    // 6. 主迴圈
    loop {
        // --- 輸入 ---
        // 從硬體讀取觸控事件（平臺相關）
        // 範例：在 (100, 200) 模擬一次輕點
        let raw_point = TouchPoint::new(1, 100, 200);
        if let Some(filtered) = filter.filter_touch(&raw_point) {
            input.process_touch(TouchEvent::Down, filtered);

            let mapped_point = TouchPoint::new(1, 101, 201);
            input.process_touch(TouchEvent::Move, mapped_point);

            let end_point = TouchPoint::new(1, 105, 202);
            input.process_touch(TouchEvent::Up, end_point);
        }

        while let Some(gesture) = input.get_gesture() {
            // 處理手勢
            let _ = gesture;
        }

        // --- 佈局 & 建立控制項 ---
        if factory.can_create() && resources.can_create_widget() {
            if resources.allocate(256) {
                let _widget = factory.create(|| {
                    Label::new(
                        "Embedded Label".to_string(),
                        Rect::new(10, 10, 200, 30),
                    )
                });
                resources.register_widget();
            }
        }

        // --- 渲染 ---
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        // 繪製控制項...
        backend.end_frame();

        // --- 資源檢查 ---
        if resources.is_under_pressure() {
            // 修剪快取，釋放非必要的控制項
            eprintln!("Memory: {:.1}%", resources.memory_percentage());
        }

        // 平臺相關：睡眠到下一幀
        // std::thread::sleep(Duration::from_millis(16));
        break;  // 本例僅執行一次迭代後退出
    }
}
```

## 總結

| 元件 | 用途 |
|-----------|---------|
| `embedded::set_embedded_mode()` | 嵌入式最佳化的全域旗標 |
| `embedded::set_low_memory_mode()` | 低記憶體限制的全域旗標 |
| `EmbeddedConfig` | 螢幕大小、固定 DPI、功能開關 |
| `ResourceManager` | 控制項數量限制、記憶體分配追蹤、壓力偵測 |
| `DpiScaler` / DPI 函式 | 固定 DPI 管理、縮放因子、點轉換 |
| `TouchPoint` | 帶壓力和大小的多點觸控點 |
| `HardwareInputManager` | 觸控處理、手勢偵測、32 個按鈕 |
| `InputFilter` | 壓力閾值篩選、死區、平滑 |
| `LightweightWidget<W>` | 用資源受限設定包裝任意 Widget |
| `LightweightWidgetFactory` | 帶最大數量的速率限制控制項建立 |
| `WidgetPool<T>` | 基於控制代碼的取得/釋放物件池 |
| `LightweightStyle` | 緊湊預設值的最小堆積樣式 |
| `LightweightConfig` | 嵌入式功能開關（陰影、動畫等） |
| `init_embedded()` / `init_desktop()` | 一次性環境初始化 |
| `mini` 功能 | no_std 就緒：經 compat.rs 從 core/alloc 匯入，heapless 支撐的 MiniVec |
