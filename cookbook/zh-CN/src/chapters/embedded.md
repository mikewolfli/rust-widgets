# 嵌入式支持

rust-widgets 通过 no_std 就绪配置文件（`mini`，当前在 std 上编译）支持嵌入式及资源受限目标平台，该配置包含 `AtomicBool` 全局标志、固定 DPI 模式、轻量级控件工厂、硬件输入处理以及自适应资源管理。

## 架构概述

```
┌──────────────────────────────────────────────┐
│  AtomicBool 全局标志：embedded_mode,          │
│  low_memory_mode, fixed_dpi                  │
├──────────────────────────────────────────────┤
│  EmbeddedConfig  →  ResourceManager          │
│  (屏幕, DPI,     →  (控件上限,               │
│   开关)              内存上限)               │
├──────────────────────┬───────────────────────┤
│  HardwareInput       │  LightweightWidget    │
│  (TouchPoint,        │  (最小堆内存,          │
│   手势, 过滤器)      │  简化特效)             │
├──────────────────────┼───────────────────────┤
│  DPI 管理            │  WidgetPool<T>        │
│  (固定 DPI, 缩放)    │  (对象池)             │
├──────────────────────┼───────────────────────┤
│  LightweightStyle    │  LightweightFactory   │
│  (紧凑默认值)        │  (速率限制)           │
└──────────────────────┴───────────────────────┘
```

## 嵌入式模式 — AtomicBool 全局标志

三个全局 `AtomicBool` 标志控制嵌入式子系统，无需全局上下文对象：

```rust
use rust_widgets::embedded;

// 检查当前模式
println!("Embedded: {}", embedded::is_embedded_mode());
println!("Low memory: {}", embedded::is_low_memory_mode());

// 启用嵌入式模式
embedded::set_embedded_mode(true);
assert!(embedded::is_embedded_mode());

// 启用低内存模式
embedded::set_low_memory_mode(true);
assert!(embedded::is_low_memory_mode());

// 切换回来
embedded::set_embedded_mode(false);
embedded::set_low_memory_mode(false);
```

**自适应常量**根据模式自动调整：

```rust
use rust_widgets::embedded;
use rust_widgets::core::Size;

// 推荐的缓冲区大小
embedded::set_low_memory_mode(true);
let low_size = embedded::recommended_buffer_size();
assert_eq!(low_size, Size::new(800, 600));

embedded::set_low_memory_mode(false);
let normal_size = embedded::recommended_buffer_size();
assert_eq!(normal_size, Size::new(1920, 1080));

// 纹理大小限制
embedded::set_embedded_mode(true);
assert_eq!(embedded::max_texture_size(), 1024);  // 嵌入式受限

embedded::set_embedded_mode(false);
assert_eq!(embedded::max_texture_size(), 4096);  // 桌面级

// 字体缓存大小
embedded::set_low_memory_mode(true);
assert_eq!(embedded::font_cache_size(), 256 * 1024);  // 256 KiB

embedded::set_low_memory_mode(false);
assert_eq!(embedded::font_cache_size(), 2 * 1024 * 1024);  // 2 MiB

// 事件队列大小
embedded::set_embedded_mode(true);
assert_eq!(embedded::event_queue_size(), 64);  // 受限

embedded::set_embedded_mode(false);
assert_eq!(embedded::event_queue_size(), 256);  // 标准
```

### init_embedded / init_desktop

单次调用即可初始化环境：

```rust
use rust_widgets::embedded::{init_embedded, init_desktop, EmbeddedConfig};
use rust_widgets::core::Size;

// 为嵌入式目标初始化，使用固定 DPI
let config = EmbeddedConfig::new(Size::new(1024, 768))
    .with_fixed_dpi(96)
    .low_memory();
init_embedded(config);
assert!(embedded::is_embedded_mode());
assert!(embedded::is_low_memory_mode());

// 切换回桌面模式
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
    .low_memory()                  // 启用低内存优化
    .with_max_widgets(50)          // 上限 50 个控件
    .with_touch(true)              // 启用触控输入
    .with_hardware_acceleration(false) // 软件渲染
    .with_font_scale(1.2);         // 120% 字号

println!("Screen: {}×{}", config.screen_size.width, config.screen_size.height);
println!("Fixed DPI: {:?}", config.fixed_dpi);
println!("Low memory: {}", config.low_memory_mode);
println!("Max widgets: {}", config.max_widgets);
println!("Animations: {}", config.enable_animations);
println!("Touch: {}", config.touch_enabled);
println!("Font scale: {}", config.font_scale);
```

**`.low_memory()` 内部设置的参数：**

| 设置项 | 默认值 | 调用 `.low_memory()` 后 |
|---------|---------|----------------------|
| `max_widgets` | 100 | 50 |
| `max_texture_size` | 1024 | 512 |
| `enable_animations` | true | false |
| `enable_shadows` | false | false |
| `enable_gradients` | true | false |

## ResourceManager — 控件数量限制与内存限制

```rust
use rust_widgets::embedded::{ResourceManager, ResourceConstraint};

// 约束级别决定限制值
let mut rm = ResourceManager::new(ResourceConstraint::Low);
// Low：   16 MiB 内存，50 个控件
// Medium：64 MiB 内存，200 个控件
// High： 256 MiB 内存，1000 个控件
// None：  无限制

// 内存分配
assert!(rm.can_allocate(1024));
assert!(rm.allocate(1024));
assert_eq!(rm.memory_usage(), 1024);
assert_eq!(rm.memory_percentage(), (1024.0_f32 / (16.0 * 1024.0 * 1024.0)) * 100.0);

// 释放内存
rm.deallocate(512);
assert_eq!(rm.memory_usage(), 512);

// 控件跟踪
assert!(rm.can_create_widget());
assert!(rm.register_widget());
assert_eq!(rm.widget_count(), 1);
rm.unregister_widget();
assert_eq!(rm.widget_count(), 0);

// 控件上限强制执行
for _ in 0..50 {
    assert!(rm.register_widget());
}
assert!(!rm.register_widget());  // 超过 max_widgets（50）
assert_eq!(rm.widget_count(), 50);

// 压力检测
assert!(rm.is_under_pressure());  // 50/50 个控件 = 100% > 90%
```

**集成模式：**

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

// 控件销毁时
fn destroy_widget(rm: &mut ResourceManager, memory_freed: usize) {
    rm.deallocate(memory_freed);
    rm.unregister_widget();
}
```

## DPI 管理

固定 DPI 模式使用全局 `AtomicU32` 状态，适用于显示屏 DPI 永远不会变化的环境（嵌入式面板、固定显示器）：

```rust
use rust_widgets::embedded;

// 设置固定 DPI
embedded::set_fixed_dpi(192);  // 2 倍密度面板
assert!(embedded::is_fixed_dpi());
assert_eq!(embedded::get_fixed_dpi(), Some(192));

// 缩放因子：DPI / 96
assert!((embedded::scale_factor() - 2.0).abs() < 0.01);

// 缩放函数
assert_eq!(embedded::scale(100), 200);       // 100 × 2.0 = 200
assert_eq!(embedded::scale_u32(100), 200);   // u32 版本
assert!((embedded::scale_f32(50.0) - 100.0).abs() < 0.01);

// 点 ↔ 像素转换（1pt = 1/72 英寸）
let px = embedded::points_to_pixels(12.0, 96);
assert!((px - 16.0).abs() < 0.01);  // 12pt at 96dpi = 16px

let pt = embedded::pixels_to_points(16.0, 96);
assert!((pt - 12.0).abs() < 0.01);  // 16px at 96dpi = 12pt

// 清除固定 DPI — 恢复为系统检测的 DPI
embedded::clear_fixed_dpi();
assert!(!embedded::is_fixed_dpi());
assert_eq!(embedded::get_fixed_dpi(), None);
```

### DpiScaler

`DpiScaler` 结构体提供了一个局部、栈分配的替代方案，无需使用全局固定 DPI：

```rust
use rust_widgets::embedded::DpiScaler;

let scaler = DpiScaler::new(144)              // 144 DPI（1.5 倍）
    .with_base_dpi(96);

assert!((scaler.scale_factor() - 1.5).abs() < 0.01);
assert_eq!(scaler.scale(100), 150);
assert_eq!(scaler.scale_u32(100), 150);
assert!((scaler.scale_f32(100.0) - 150.0).abs() < 0.01);

// 反向缩放（像素 → 逻辑值）
assert_eq!(scaler.unscale(150), 100);
assert_eq!(scaler.unscale_u32(150), 100);
assert!((scaler.unscale_f32(150.0) - 100.0).abs() < 0.01);
```

## 硬件输入：触控与手势

### TouchPoint

```rust
use rust_widgets::embedded::TouchPoint;

let point = TouchPoint::new(1, 100, 200)    // id=1, x=100, y=200
    .with_pressure(0.8);                     // 80% 压力

assert_eq!(point.id, 1);
assert_eq!(point.position.x, 100);
assert_eq!(point.position.y, 200);
assert!((point.pressure - 0.8).abs() < 0.01);
```

### HardwareInputManager

处理多点触控、32 个硬件按钮以及手势检测：

```rust
use rust_widgets::embedded::{
    HardwareInputManager, TouchPoint, TouchEvent,
    GestureType, InputType,
};

let mut manager = HardwareInputManager::new();

// 处理触摸按下
let point = TouchPoint::new(1, 100, 100);
manager.process_touch(TouchEvent::Down, point);
assert_eq!(manager.touch_point_count(), 1);

// 处理触摸移动
let moved = TouchPoint::new(1, 120, 100);
manager.process_touch(TouchEvent::Move, moved);

// 处理触摸抬起 — 自动检测手势
manager.process_touch(TouchEvent::Up, moved);

// 轮询检测到的手势
while let Some(gesture) = manager.get_gesture() {
    match gesture.gesture_type {
        GestureType::Tap => println!("Tap at ({}, {})", gesture.center.x, gesture.center.y),
        GestureType::SwipeRight => println!("Swipe right, velocity: {:?}", gesture.velocity),
        GestureType::LongPress => println!("Long press at ({}, {})", gesture.center.x, gesture.center.y),
        _ => println!("Gesture: {:?}", gesture.gesture_type),
    }
}

// 硬件按钮（最多 32 个）
manager.process_button(0, true);   // 按钮 0 按下
assert!(manager.is_button_pressed(0));
manager.process_button(0, false);  // 释放
assert!(!manager.is_button_pressed(0));

// 触摸取消
manager.process_touch(TouchEvent::Cancel, point);
manager.clear();
```

**手势检测阈值：**
- **轻点：** 时长 < 200ms，距离 < 50px
- **长按：** 时长 ≥ 500ms，距离 < 50px
- **滑动：** 距离 ≥ 50px — 方向由主坐标轴决定

### InputFilter

`InputFilter` 提供压力阈值过滤、死区滤波以及位置平滑：

```rust
use rust_widgets::embedded::{InputFilter, TouchPoint};

let mut filter = InputFilter::new()
    .with_dead_zone(10);  // 10px 死区

// 首次触摸直接通过（无前一次位置）
let point1 = TouchPoint::new(1, 100, 100);
let result1 = filter.filter_touch(&point1);
assert!(result1.is_some());

// 死区内的微小移动 → 被过滤掉
let point2 = TouchPoint::new(1, 105, 105);  // dx=5, dy=5 < 10
let result2 = filter.filter_touch(&point2);
assert!(result2.is_none());

// 超出死区的明显移动 → 平滑处理
let point3 = TouchPoint::new(1, 150, 150);  // dx=50, dy=50
let result3 = filter.filter_touch(&point3);
assert!(result3.is_some());
// 位置被平滑处理：100 + 0.5×(150-100) = 125

// 压力阈值：低于 min_pressure 的触摸被过滤
let weak = TouchPoint::new(2, 200, 200).with_pressure(0.05);
assert!(filter.filter_touch(&weak).is_none());  // 低于 min_pressure（0.1）

filter.reset();  // 清除状态
```

## LightweightWidget — 资源受限渲染

```rust
use rust_widgets::embedded::{
    LightweightWidget, LightweightConfig, LightweightStyle,
};
use rust_widgets::widget::Label;
use rust_widgets::core::Rect;

// 将任意 Widget 包装在轻量外壳中
let label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 30));
let lw = LightweightWidget::new(label)
    .with_config(LightweightConfig::minimal());

// 访问内部控件
println!("Inner widget kind: {:?}", lw.inner().kind());

// 解包装
let label = lw.into_inner();
```

**LightweightConfig 预设：**

```rust
// 灵活配置
let config = LightweightConfig::new()
    .with_shadows_disabled()
    .with_animations_disabled()
    .with_gradients_disabled();

// 或使用最小化预设
let minimal = LightweightConfig::minimal();
assert!(minimal.disable_shadows);
assert!(minimal.disable_animations);
assert!(minimal.disable_gradients);
assert!(minimal.simple_borders);
assert!(minimal.reduced_padding);
assert!(minimal.minimal_signals);

// 使用最小化配置创建控件
let lw = LightweightWidget::new(label)
    .with_config(LightweightConfig::minimal());
```

### LightweightStyle — 最小堆内存使用

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

## LightweightWidgetFactory — 速率限制的控件创建

```rust
use rust_widgets::embedded::{
    LightweightWidgetFactory, LightweightConfig,
};
use rust_widgets::widget::Label;
use rust_widgets::core::Rect;

let mut factory = LightweightWidgetFactory::new()
    .with_config(LightweightConfig::minimal())
    .with_max_widgets(5);

// 创建控件（超出上限时返回 None）
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

// 释放一个槽位
factory.release();
assert_eq!(factory.widget_count(), 4);
assert!(factory.can_create());  // 现在可以再创建一个
```

## WidgetPool&lt;T&gt; — 控件对象池

```rust
use rust_widgets::embedded::WidgetPool;

let mut pool: WidgetPool<i32> = WidgetPool::new(3);  // 最多 3 个池化项

// 获取项（通过工厂闭包创建）
let h1 = pool.acquire(|| 1);
assert!(h1.is_some());
assert_eq!(pool.used_count(), 1);
assert_eq!(pool.available_count(), 0);  // 所有预分配槽位已用

let h2 = pool.acquire(|| 2);
assert!(h2.is_some());
assert_eq!(pool.used_count(), 2);

// 通过句柄访问项
let h1_ref = h1.as_ref().unwrap();
assert_eq!(*pool.get(h1_ref.index()).unwrap(), 1);

// 释放句柄 → 槽位回到池中
drop(h1);
assert_eq!(pool.used_count(), 1);

// 释放的槽位可被重用
let h3 = pool.acquire(|| 3);
assert!(h3.is_some());

// 池已满 — 后续获取返回 None
let h4 = pool.acquire(|| 4);
assert!(h4.is_none());
```

## 低内存模式 — 推荐限制

启用 `low_memory_mode` 后，框架会自动调整：

| 资源 | 标准 | 低内存 |
|----------|----------|------------|
| 缓冲区大小 | 1920×1080 | 800×600 |
| 最大纹理大小 | 4096 | 1024（使用 `.low_memory()` 时为 512） |
| 字体缓存 | 2 MiB | 256 KiB |
| 事件队列 | 256 | 64 |
| 最大控件数（默认） | 100 | 50 |
| 动画 | 启用 | 禁用 |
| 阴影 | 禁用 | 禁用 |
| 渐变 | 启用 | 禁用 |

## 构建嵌入式目标

### Release-embedded 配置文件

添加到 `Cargo.toml`：

```toml
[profile.release-embedded]
inherits = "release"
opt-level = "s"           # 优化体积
lto = true                # 链接时优化
codegen-units = 1         # 单个代码生成单元以优化 LTO
strip = true              # 剥离调试符号
panic = "abort"           # 不展开（更小的二进制）
```

使用 `mini` 特性构建：

```sh
cargo build --profile release-embedded --no-default-features \
  --features "mini,embedded" --target thumbv7em-none-eabihf
```

### 特性标志配置

```toml
[dependencies]
rust_widgets = { version = "1.0", default-features = false, features = [
    "mini",          # no_std 就绪配置，heapless 支撑的 MiniVec
    "embedded",      # 嵌入式模式 + 轻量控件
] }
```

### 推荐的 `mini` 特性用法

`mini` 特性通过 `compat.rs` 将 std 类型替换为竞技场分配和无堆的替代方案：
- `HashMap` → `BTreeMap`
- `Mutex` → `RefCell`
- `Vec` → `MiniVec`
- `String` → `MiniString`

## 完整嵌入式渲染循环

一个结合了所有概念的最小嵌入式渲染循环：

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

    // 2. 设置资源管理
    let mut resources = ResourceManager::new(ResourceConstraint::Low);

    // 3. 输入处理
    let mut input = HardwareInputManager::new();
    let mut filter = InputFilter::new().with_dead_zone(8);

    // 4. 控件工厂
    let mut factory = LightweightWidgetFactory::new()
        .with_config(LightweightConfig::minimal())
        .with_max_widgets(20);

    // 5. 渲染后端
    let buf_size = embedded::recommended_buffer_size();
    let mut backend = SoftwarePaintBackend::new(buf_size, 1.0);

    // 6. 主循环
    loop {
        // --- 输入 ---
        // 从硬件读取触摸事件（平台相关）
        // 示例：在 (100, 200) 模拟一次轻点
        let raw_point = TouchPoint::new(1, 100, 200);
        if let Some(filtered) = filter.filter_touch(&raw_point) {
            input.process_touch(TouchEvent::Down, filtered);

            let mapped_point = TouchPoint::new(1, 101, 201);
            input.process_touch(TouchEvent::Move, mapped_point);

            let end_point = TouchPoint::new(1, 105, 202);
            input.process_touch(TouchEvent::Up, end_point);
        }

        while let Some(gesture) = input.get_gesture() {
            // 处理手势
            let _ = gesture;
        }

        // --- 布局 & 创建控件 ---
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
        // 绘制控件...
        backend.end_frame();

        // --- 资源检查 ---
        if resources.is_under_pressure() {
            // 修剪缓存，释放非必要的控件
            eprintln!("Memory: {:.1}%", resources.memory_percentage());
        }

        // 平台相关：睡眠到下一帧
        // std::thread::sleep(Duration::from_millis(16));
        break;  // 本例仅执行一次迭代后退出
    }
}
```

## 总结

| 组件 | 用途 |
|-----------|---------|
| `embedded::set_embedded_mode()` | 嵌入式优化的全局标志 |
| `embedded::set_low_memory_mode()` | 低内存约束的全局标志 |
| `EmbeddedConfig` | 屏幕大小、固定 DPI、特性开关 |
| `ResourceManager` | 控件数量限制、内存分配跟踪、压力检测 |
| `DpiScaler` / DPI 函数 | 固定 DPI 管理、缩放因子、点转换 |
| `TouchPoint` | 带压力和大小的多点触控点 |
| `HardwareInputManager` | 触摸处理、手势检测、32 个按钮 |
| `InputFilter` | 压力阈值过滤、死区、平滑 |
| `LightweightWidget<W>` | 用资源受限配置包装任意 Widget |
| `LightweightWidgetFactory` | 带最大数量的速率限制控件创建 |
| `WidgetPool<T>` | 基于句柄的获取/释放对象池 |
| `LightweightStyle` | 紧凑默认值的最小堆样式 |
| `LightweightConfig` | 嵌入式特性开关（阴影、动画等） |
| `init_embedded()` / `init_desktop()` | 一次性环境初始化 |
| `mini` 特性 | no_std 就绪：经 compat.rs 从 core/alloc 导入，heapless 支撑的 MiniVec |
