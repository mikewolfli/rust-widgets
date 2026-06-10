# 性能与质量

rust-widgets 提供了一套集成的性能与质量管理系统，能够自动调整渲染保真度，以在多种硬件上保持流畅的帧率——从集成笔记本 GPU 到独立桌面 GPU，从纯 CPU 软件光栅化器到高端 GPU。

## 架构概述

性能-质量子系统由四个协作层组成：

```
┌─────────────────────────────────────────┐
│          QualityManager                  │  ← 基于滞后的质量切换
├─────────────────────────────────────────┤
│  FrameTimeMonitor  │  GpuCapability      │  ← 硬件检测 + 帧时间跟踪
├─────────────────────────────────────────┤
│  Profiler  │  FrameProfiler  │  Monitor  │  ← 仪器化层
├─────────────────────────────────────────┤
│  DirtyRegionTracker  │  UpdateBatcher    │  ← 脏区域优化
└─────────────────────────────────────────┘
```

## 质量等级与自适应质量

### QualityLevel

`QualityLevel` 枚举定义了三个渲染等级：

```rust
use rust_widgets::quality::QualityLevel;

// QualityLevel::High   — 全部效果（抗锯齿、阴影、复杂着色器）
// QualityLevel::Medium — 基本效果、简单着色器、无阴影
// QualityLevel::Low    — 最小化渲染、纯色填充、跳过非关键元素

assert!(QualityLevel::Low < QualityLevel::Medium);
assert!(QualityLevel::Medium < QualityLevel::High);

// 导航辅助
assert_eq!(QualityLevel::High.lower(), Some(QualityLevel::Medium));
assert_eq!(QualityLevel::Low.higher(), Some(QualityLevel::Medium));
assert_eq!(QualityLevel::Low.lower(), None);

// 在范围内限制
let clamped = QualityLevel::High.clamp(QualityLevel::Medium, QualityLevel::High);
```

### QualityConfig

`QualityConfig` 控制所有滞后参数：

```rust
use rust_widgets::quality::{QualityConfig, QualityLevel};

let config = QualityConfig {
    target_frame_rate: 60.0,            // 目标帧率
    degrade_threshold: 1.5,             // 帧时间 > 目标值 × 1.5 时降级
    upgrade_threshold: 0.7,             // 帧时间 < 目标值 × 0.7 时升级
    max_quality: QualityLevel::High,    // 上限为 High
    min_quality: QualityLevel::Low,     // 下限为 Low
    degrade_frame_count: 5,             // 连续 5 帧慢帧 → 降级
    upgrade_frame_count: 10,            // 连续 10 帧快帧 → 升级
};

// 计算阈值（目标 60 FPS ≈ ~16.67ms）：
// degrade_frame_duration() = 16.67ms × 1.5 = 25.0ms
// upgrade_frame_duration() = 16.67ms × 0.7 = 11.67ms

// 始终调用 normalized() 来限制无效值
let safe_config = config.normalized();
// degrade_threshold 为 max(1.0, 1.5) = 1.5
// upgrade_threshold 为 clamp(0.7, 0.1, 1.0) = 0.7
```

### FrameTimeMonitor

`FrameTimeMonitor` 维护一个 60 帧的环形缓冲区，用于实时帧时间分析：

```rust
use rust_widgets::quality::FrameTimeMonitor;

let mut monitor = FrameTimeMonitor::new(60.0);  // 目标 60 FPS

// 模拟记录 60 帧，每帧 16ms
for _ in 0..60 {
    monitor.record_frame(0.016);  // 16ms ≈ 62.5 FPS
}

println!("平均帧时间: {:.4}s", monitor.average_frame_time());
println!("当前帧率: {:.1}", monitor.current_fps());

// 检查是否需要降级（连续 5 帧超过 25ms 阈值）
let should_degrade = monitor.should_degrade(0.025, 5);

// 检查是否需要升级（连续 5 帧低于 11.67ms 阈值）
let should_upgrade = monitor.should_upgrade(0.01167, 5);

// 重置并重新配置
monitor.reset();
monitor.set_target_frame_rate(30.0);  // 切换到 30 FPS 目标
```

### QualityManager — 基于滞后的自适应

`QualityManager` 封装了 `FrameTimeMonitor`、`QualityConfig` 和 `GpuCapability`，提供自动的滞后式质量转换：

```rust
use rust_widgets::quality::{QualityManager, QualityConfig, QualityLevel, GpuCapability};

// 自动检测 GPU 并从推荐质量等级开始
let mut manager = QualityManager::new();
println!("初始质量: {:?}", manager.quality_level());
// 在 tier ≥ 4 的独立 GPU 上：从 QualityLevel::High 开始

// --- 集成到渲染循环中 ---
fn render_loop(manager: &mut QualityManager) {
    loop {
        let frame_start = std::time::Instant::now();

        // ... 渲染您的帧 ...

        let frame_duration = frame_start.elapsed();
        manager.finish_frame(frame_duration);
        // ^ 这会记录帧时间并评估降级/升级条件

        let current = manager.quality_level();
        match current {
            QualityLevel::High   => { /* 全部效果 */ }
            QualityLevel::Medium => { /* 简化着色器 */ }
            QualityLevel::Low    => { /* 仅纯色填充 */ }
        }

        println!(
            "质量: {:?}, 帧率: {:.1}, 平均帧: {:.4}s",
            current,
            manager.current_fps(),
            manager.average_frame_time()
        );
    }
}
```

**手动质量控制：**

```rust
// 绕过滞后，直接设置
manager.set_quality_level(QualityLevel::Medium);

// 查询配置状态
let config = manager.config();
println!("降级帧数阈值: {}", config.degrade_frame_count);

// 热重载配置
let new_config = QualityConfig {
    target_frame_rate: 30.0,
    ..QualityConfig::default()
};
manager.set_config(new_config);

// 重置到初始状态
manager.reset();
```

**滞后逻辑**（在 `update_quality_level()` 中实现）：

| 当前 | 条件 | 动作 |
|---------|-----------|--------|
| High | N 个连续慢帧 | 降级 → Medium |
| Medium | N 个连续慢帧 | 降级 → Low |
| Medium | N 个连续快帧 | 升级 → High |
| Low | N 个连续快帧 | 升级 → Medium |

降级和升级的连续帧数不同（降级默认 5 帧，升级默认 10 帧），防止瞬时的帧时间尖峰导致"振荡"。

## GPU 能力检测

### GpuCapability — 5 个性能等级

```rust
use rust_widgets::quality::GpuCapability;

// 手动构造
let discrete_gpu = GpuCapability {
    supports_high_quality: true,
    is_integrated: false,
    performance_tier: 5,  // 1–5 级
};
let integrated_gpu = GpuCapability {
    supports_high_quality: true,
    is_integrated: true,
    performance_tier: 3,
};
let cpu_fallback = GpuCapability {
    supports_high_quality: false,
    is_integrated: false,
    performance_tier: 1,
};

// 基于 GPU 等级的推荐初始质量
assert_eq!(discrete_gpu.recommended_initial_quality(), QualityLevel::High);
assert_eq!(integrated_gpu.recommended_initial_quality(), QualityLevel::Medium);
assert_eq!(cpu_fallback.recommended_initial_quality(), QualityLevel::Low);
```

**从 wgpu 适配器信息自动检测：**

```rust
#[cfg(feature = "gpu-wgpu")]
async fn detect_gpu() -> GpuCapability {
    let instance = wgpu::Instance::default();
    if let Some(adapter) = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await {
        GpuCapability::from_adapter_info(&adapter.get_info())
    } else {
        GpuCapability::default_capability()  // 回退到 tier 3
    }
}
```

检测将 wgpu `DeviceType` 映射到性能等级：

| 设备类型 | 等级 | 推荐质量 |
|-------------|------|---------------------|
| `DiscreteGpu` | 5 | High |
| `IntegratedGpu` | 3 | Medium |
| `VirtualGpu` | 2 | Medium |
| `Other` | 2 | Medium |
| `Cpu` | 1 | Low |

## GpuManager — 适配器选择与操作模式

### GpuManager

`GpuManager` 处理 GPU 硬件检测、适配器选择（多种策略）以及自适应性能监控：

```rust
use rust_widgets::gpu::{GpuManager, GpuManagerBuilder, AdapterSelectionStrategy};

// 自动选择：选择最佳可用 GPU
async fn auto_setup() -> Result<(), Box<dyn std::error::Error>> {
    let manager = GpuManager::new().await?;

    println!("使用 GPU: {}", manager.adapter_info().name);
    println!("厂商: {}", manager.adapter_info().vendor);
    println!("后端: {}", manager.adapter_info().backend);

    if manager.is_hardware() {
        println!("硬件加速渲染已激活");
    } else if manager.is_software() {
        println!("软件渲染已激活（CPU）");
    }

    // 检查是否在浏览器中运行（WebAssembly）
    println!("浏览器强制 iGPU: {:?}", manager.adapter_info().is_selected);

    Ok(())
}
```

### AdapterSelectionStrategy — 7 种选择策略

```rust
use rust_widgets::gpu::{
    AdapterSelectionStrategy, AdapterSelector, GpuDeviceType,
};
use std::sync::Arc;

async fn select_adapter() {
    let strategies = [
        AdapterSelectionStrategy::PreferPerformance,    // 最佳原始性能
        AdapterSelectionStrategy::PreferPowerEfficiency,// 省电优先
        AdapterSelectionStrategy::ForceDiscrete,         // 强制使用独立 GPU
        AdapterSelectionStrategy::ForceIntegrated,       // 强制使用集成 GPU
        AdapterSelectionStrategy::ForceCpu,              // 仅软件渲染
        AdapterSelectionStrategy::Auto,                  // 自动选择（默认）
    ];

    for strategy in strategies {
        let selector = AdapterSelector::new(strategy).allow_fallback(true);
        match selector.enumerate_adapters().await {
            Ok(adapters) => {
                println!(
                    "策略 {:?}: 找到 {} 个适配器",
                    strategy,
                    adapters.len()
                );
            }
            Err(e) => println!("策略 {:?}: {}", strategy, e),
        }
    }
}
```

### GpuOperationMode

```rust
use rust_widgets::gpu::GpuOperationMode;

let manager = GpuManager::new().await.unwrap();

match manager.operation_mode() {
    GpuOperationMode::Hardware => {
        // 基于 wgpu 的 GPU 渲染
        println!("在独立/集成 GPU 上渲染");
    }
    GpuOperationMode::Software => {
        // CPU 光栅化器（SoftwarePaintBackend）
        println!("在 CPU 上渲染");
    }
    GpuOperationMode::Hybrid => {
        // GPU 负责合成，CPU 负责某些通道
        println("混合 GPU/CPU 模式");
    }
}
```

### GpuManagerBuilder

```rust
use rust_widgets::gpu::{GpuManagerBuilder, AdapterSelectionStrategy, QualityLevel};

let manager = GpuManagerBuilder::new()
    .strategy(AdapterSelectionStrategy::PreferPerformance)
    .allow_fallback(true)  // 独立 GPU 不可用时回退
    .target_quality(QualityLevel::High)
    .build()
    .await?;
```

### GpuManager 帧集成

```rust
fn render_frame(manager: &mut GpuManager) {
    manager.begin_frame();

    // ... 渲染工作 ...

    manager.end_frame();
    // ^ end_frame() 评估性能并更新质量跟踪器

    // 基于 GPU 能力和帧时间的当前自适应质量
    println!("质量: {:?}", manager.current_quality());

    // 检查面向用户的建议
    for action in manager.recommended_actions() {
        println!("建议: {}", action.message());
        println!("  优先级: {}", action.priority());
        // GpuManagerAction 变体：
        // - SuggestSwitchToCpuMode
        // - SuggestRestartOutsideBrowser
        // - SuggestCloseOtherApplications
        // - SuggestReduceResolution
        // - SuggestUpdateDrivers
    }
}
```

## 性能监控与分析

### Profiler

`Profiler` 提供带有聚合功能的命名区间仪器化：

```rust
use rust_widgets::performance::Profiler;
use std::time::Duration;

let mut profiler = Profiler::new();

// 命名 begin/end 对
profiler.begin("layout_pass");
// ... 布局计算 ...
profiler.end();

profiler.begin("render_pass");
// ... 渲染 ...
profiler.end();

// 基于闭包的测量
let result = profiler.measure("parse_json", || {
    // ... 解析 JSON ...
    42
});

// 查询单个统计信息
if let Some(entry) = profiler.get_stats("layout_pass") {
    println!("layout_pass 被调用 {} 次，总计: {:?}",
        entry.call_count, entry.duration);
}

// 获取超过阈值的热点
let hotspots = profiler.get_hotspots(Duration::from_millis(5));
for (name, duration) in &hotspots {
    println!("  {}: {:?}", name, duration);
}

// 生成人类可读的报告
let report = profiler.report();
println!("{}", report.to_string_summary());

// 为下一个测量窗口重置
profiler.reset();
```

### FrameProfiler

`FrameProfiler` 跟踪每帧时序，计算帧率、最小/最大值和各区间细分：

```rust
use rust_widgets::performance::FrameProfiler;

let mut frame_profiler = FrameProfiler::new(60);  // 缓冲最近 60 帧

// 每帧集成
for frame in 0..120 {
    frame_profiler.begin_frame();

    frame_profiler.begin_section("input");
    // ... 处理事件 ...
    frame_profiler.end_section();

    frame_profiler.begin_section("layout");
    // ... 布局处理 ...
    frame_profiler.end_section();

    frame_profiler.begin_section("draw");
    // ... 渲染 ...
    frame_profiler.end_section();

    frame_profiler.end_frame();
}

println!("帧率: {:.1}", frame_profiler.fps());
println!("平均帧时间: {:?}", frame_profiler.average_frame_time());
println!("最小帧时间: {:?}", frame_profiler.min_frame_time());
println!("最大帧时间: {:?}", frame_profiler.max_frame_time());
println!("已记录帧数: {}", frame_profiler.frame_count());

// 各区间细分（跨帧聚合）
for (section, duration) in frame_profiler.sections() {
    println!("  {}: {:?}", section, duration);
}
```

### PerformanceMonitor

`PerformanceMonitor` 将两个分析器合并为一个统一接口：

```rust
use rust_widgets::performance::PerformanceMonitor;

let mut monitor = PerformanceMonitor::new();

// 在生产环境中禁用监控以零开销
monitor.disable();

// 调试时启用
monitor.enable();

// 帧集成
monitor.begin_frame();

monitor.measure("expensive_operation", || {
    // ... 被分析的工作 ...
    std::thread::sleep(std::time::Duration::from_millis(10));
});

monitor.end_frame();

// 生成综合报告
if monitor.is_enabled() {
    let report = monitor.report();
    println!("{}", report.to_string_summary());
    // 输出包括：
    //   帧率: 59.8
    //   平均帧: 16.723ms
    //   最小帧: 15.201ms
    //   最大帧: 45.332ms
    //   帧数: 120
    //   （以及各区间细分）
}
```

## 脏区域跟踪

### DirtyRegionTracker

跟踪需要重新渲染的矩形区域，避免全帧重绘：

```rust
use rust_widgets::performance::{DirtyRegionTracker, render_dirty_regions};
use rust_widgets::core::Rect;
use rust_widgets::render::{RenderContext, SoftwarePaintBackend, PaintBackend};
use rust_widgets::core::{Color, Size};

let mut tracker = DirtyRegionTracker::new();
// 或使用自定义最大区域数限制：
// let mut tracker = DirtyRegionTracker::with_max_regions(200);

// 标记脏区域（例如，当 widget 发生变化时）
let region_id = tracker.add(Rect::new(0, 0, 100, 100));
tracker.add(Rect::new(50, 50, 100, 100));    // 重叠 — 将合并
tracker.add_with_priority(Rect::new(200, 0, 50, 50), 9);  // 高优先级
tracker.add_with_layer(Rect::new(0, 200, 80, 30), 1);     // 特定层

// 合并重叠区域
tracker.merge();
assert_eq!(tracker.len(), 3);  // 1 个合并 + 2 个独立

// 获取所有区域的边界矩形
if let Some(bounding) = tracker.get_bounding_rect() {
    println!("边界矩形: {:?}", bounding);
}

// 查询与给定矩形相交的区域
let overlapping = tracker.get_regions_for_rect(&Rect::new(0, 0, 50, 50));

// 将所有区域裁剪到指定矩形
tracker.clip_to(&Rect::new(0, 0, 150, 150));

// 优化：合并 + 如果超过最大值则截断
tracker.optimize();

// 渲染后清除
tracker.clear();
```

### WidgetDirtyState

使用 `ObjectId` 进行逐 widget 的脏状态跟踪：

```rust
use rust_widgets::performance::WidgetDirtyState;
use rust_widgets::core::{ObjectId, Rect};

let mut state = WidgetDirtyState::new();

let widget_a = ObjectId::new();
let widget_b = ObjectId::new();

// 标记 widget 为脏
state.mark_dirty(widget_a, Rect::new(0, 0, 100, 50));
state.mark_dirty(widget_b, Rect::new(0, 50, 100, 50));

assert!(state.is_dirty(widget_a));
assert_eq!(state.len(), 2);

// 查询脏矩形
if let Some(rect) = state.get_dirty_rect(widget_a) {
    println!("Widget A 脏矩形: {:?}", rect);
}

// 渲染后标记为干净
state.mark_clean(widget_a);
assert!(!state.is_dirty(widget_a));

// 一次性获取所有脏矩形
let all_rects = state.get_all_rects();
println!("{} 个脏矩形", all_rects.len());

// 遍历脏 widget
for id in state.dirty_widgets() {
    println!("Widget {:?} 需要重绘", id);
}

state.clear();
assert!(state.is_empty());
```

### render_dirty_regions — 优化渲染循环

`render_dirty_regions` 函数实现了完整的脏区域渲染管线：

```rust
use rust_widgets::performance::{DirtyRegionTracker, render_dirty_regions};
use rust_widgets::render::{RenderContext, SoftwarePaintBackend, PaintBackend};
use rust_widgets::core::{Color, Size, Rect};

fn render_with_dirty_regions() {
    let mut backend = SoftwarePaintBackend::new(Size::new(800, 600), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);

    let mut tracker = DirtyRegionTracker::new();
    // 模拟：这些区域的 widget 发生了变化
    tracker.add(Rect::new(10, 10, 200, 100));
    tracker.add(Rect::new(300, 400, 150, 50));

    render_dirty_regions(&mut tracker, &mut ctx, |ctx| {
        // 此闭包为每个合并的脏区域调用一次
        // 裁剪矩形已推入上下文中
        // 内部绘制完整的帧内容——只有脏
        // 区域会被实际渲染
        draw_all_widgets(ctx);
    });
    // tracker 在渲染后自动清除

    backend.end_frame();
}

fn draw_all_widgets(_ctx: &mut RenderContext) {
    // ... 绘制每个 widget ...
}
```

**渲染策略**（在 `render_dirty_regions` 中实现）：

1. **空** → 完全跳过（不渲染）
2. **1–16 个区域** → 分别使用裁剪矩形重新绘制每个区域
3. **>16 个区域** → 回退到全边界矩形重绘
4. **渲染后** → 跟踪器被清除

## UpdateBatcher — 基于时间和计数的合并

`UpdateBatcher` 将多个更新区域合并为批次，在超时或达到计数阈值时刷新：

```rust
use rust_widgets::performance::UpdateBatcher;
use rust_widgets::core::Rect;

// 16ms 批处理（匹配 ~60 FPS 刷新率）
let mut batcher = UpdateBatcher::new(16);

// 在帧内收集更新区域
batcher.add(Rect::new(10, 10, 50, 30));
batcher.add(Rect::new(100, 50, 80, 40));
batcher.add(Rect::new(200, 20, 40, 60));

assert_eq!(batcher.len(), 3);

// 检查是否到了刷新时间（16ms 已过或 10+ 个待处理矩形）
if batcher.should_flush() {
    // 刷新返回合并后的矩形
    let merged_rects = batcher.flush();
    println!("刷新了 {} 个合并矩形", merged_rects.len());
    assert!(batcher.is_empty());
}

// flush_clipped 直接使用脏区域优化进行渲染
// let mut ctx = ...;
// batcher.flush_clipped(&mut ctx, |ctx| { draw_all_widgets(ctx); });

// 不清除直接清除
batcher.clear();
```

**刷新阈值：**
- **基于时间：** `last_batch.elapsed() >= batch_timeout_ms`（默认：16ms）
- **基于计数：** `pending_updates.len() >= 10`

## 自适应渲染与动态质量

结合 `QualityManager`、脏区域和更新批处理，构建完整的自适应渲染管线：

```rust
use rust_widgets::quality::{QualityManager, QualityLevel, QualityConfig};
use rust_widgets::performance::{DirtyRegionTracker, UpdateBatcher, render_dirty_regions, WidgetDirtyState};
use rust_widgets::render::{RenderContext, SoftwarePaintBackend, PaintBackend};
use rust_widgets::core::{Color, Size, Rect};
use std::time::Instant;

struct AdaptiveRenderer {
    quality_manager: QualityManager,
    dirty_tracker: DirtyRegionTracker,
    update_batcher: UpdateBatcher,
    widget_dirty_state: WidgetDirtyState,
}

impl AdaptiveRenderer {
    fn new() -> Self {
        Self {
            quality_manager: QualityManager::new(),
            dirty_tracker: DirtyRegionTracker::new(),
            update_batcher: UpdateBatcher::new(16),  // 16ms = ~60 FPS
            widget_dirty_state: WidgetDirtyState::new(),
        }
    }

    fn render_frame(
        &mut self,
        backend: &mut SoftwarePaintBackend,
    ) {
        let frame_start = Instant::now();

        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(backend);

        // 根据质量等级确定渲染策略
        match self.quality_manager.quality_level() {
            QualityLevel::High => {
                // 全部效果：使用脏区域渲染
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.full_render(ctx),
                );
            }
            QualityLevel::Medium => {
                // 简化：跳过阴影，降低着色器复杂度
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.simplified_render(ctx),
                );
            }
            QualityLevel::Low => {
                // 最小化：仅纯色填充，跳过非关键元素
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.minimal_render(ctx),
                );
            }
        }

        backend.end_frame();

        // 记录帧时间 → 触发基于滞后的质量变化
        self.quality_manager.finish_frame(frame_start.elapsed());

        // 为下一帧批量处理脏区域更新
        if self.update_batcher.should_flush() {
            let rects = self.update_batcher.flush();
            for rect in rects {
                self.dirty_tracker.add(rect);
            }
        }
    }

    fn full_render(&self, ctx: &mut RenderContext) {
        // 绘制阴影、渐变、抗锯齿
    }

    fn simplified_render(&self, ctx: &mut RenderContext) {
        // 使用实心边框绘制，无阴影
    }

    fn minimal_render(&self, ctx: &mut RenderContext) {
        // 使用简单填充绘制，跳过装饰
    }
}
```

## 性能陷阱检测

`PerformanceTrapDetector` 识别持续的低帧率并提供可操作的建议：

```rust
use rust_widgets::gpu::{
    PerformanceTrapDetector, PerformanceTrap, AdaptivePerformanceMonitor,
    AdaptivePerformanceThresholds,
};
use rust_widgets::quality::QualityLevel;

// 为集成 GPU 配置阈值
let thresholds = AdaptivePerformanceThresholds::for_device_type(
    rust_widgets::gpu::GpuDeviceType::IntegratedGpu,
);
println!("目标帧率: {}", thresholds.target_fps);
println!("降级阈值: {:.2}s", thresholds.degrade_duration());

// 创建自适应性能监控器
let mut performance_monitor = AdaptivePerformanceMonitor::for_device_type(
    rust_widgets::gpu::GpuDeviceType::IntegratedGpu,
);

// 帧集成
performance_monitor.begin_frame();
// ... 渲染 ...
performance_monitor.end_frame();

// 检查性能
if performance_monitor.should_degrade() {
    println!("性能已降级 — 考虑降低质量");
}

if performance_monitor.should_upgrade() {
    println!("性能已恢复 — 考虑提高质量");
}

// 性能统计
let stats = performance_monitor.stats();
println!("当前帧率: {:.1}", stats.current_fps);
println!("稳定性: {:.2}（越低越稳定）", stats.stability);
println!("内存压力: {}", stats.is_memory_pressure);
println!("CPU 过载: {}", stats.is_cpu_overloaded);
```

**性能陷阱检测：**

```rust
let mut trap_detector = PerformanceTrapDetector::new();

// 每帧检查
if let Some(trap) = trap_detector.check(30.0) {  // 阈值：30 FPS
    match trap {
        PerformanceTrap::LowFrameRate { current_fps, threshold } => {
            eprintln!(
                "低帧率: {:.1} 帧/秒（阈值: {:.1}）",
                current_fps, threshold
            );
        }
        PerformanceTrap::MemoryPressure { utilization } => {
            eprintln!("内存压力: {:.1}% 已使用", utilization);
        }
        PerformanceTrap::CpuOverload { utilization } => {
            eprintln!("CPU 过载: {:.1}%", utilization);
        }
        PerformanceTrap::BrowserForcedIntegratedGpu => {
            eprintln!("浏览器强制使用集成 GPU — 请在浏览器外重启");
        }
    }

    println!("{}", trap.message());
    println!("  建议 CPU 模式: {}", trap.suggests_cpu_mode());
    println!("  建议重启:     {}", trap.suggests_restart());
}
```

### AdaptivePerformanceThresholds

针对不同设备类型的硬件定制阈值：

```rust
use rust_widgets::gpu::{AdaptivePerformanceThresholds, GpuDeviceType};

// 独立 GPU：积极升级，保守降级
let discrete = AdaptivePerformanceThresholds::discrete();
println!("独立 GPU 目标: {} 帧/秒", discrete.target_fps);
println!("  降级于 {} 个慢帧后", discrete.degrade_frame_count);
println!("  升级于 {} 个快帧后", discrete.upgrade_frame_count);

// 集成 GPU：平衡阈值
let integrated = AdaptivePerformanceThresholds::integrated();

// CPU 渲染：保守，更易降级
let cpu = AdaptivePerformanceThresholds::cpu();

// 根据设备类型自动选择
let auto = AdaptivePerformanceThresholds::for_device_type(GpuDeviceType::DiscreteGpu);
```

## 总结

| 组件 | 用途 |
|-----------|---------|
| `QualityManager` | 带滞后的自适应质量（5 帧降级，10 帧升级） |
| `QualityLevel` | High / Medium / Low 等级定义 |
| `QualityConfig` | 目标帧率、阈值、帧数 |
| `FrameTimeMonitor` | 60 帧环形缓冲区，用于帧时间跟踪 |
| `GpuCapability` | 从 wgpu 适配器信息进行 5 级 GPU 检测 |
| `GpuManager` | 适配器选择（7 种策略）、操作模式、缓冲池 |
| `Profiler` | 带热点检测的命名区间仪器化 |
| `FrameProfiler` | 每帧时序、帧率、最小/最大帧时间 |
| `PerformanceMonitor` | 统一的 Profiler + FrameProfiler，带报告 |
| `DirtyRegionTracker` | 基于区域的脏跟踪，支持合并/优化 |
| `WidgetDirtyState` | 使用 ObjectId 的逐 widget 脏状态 |
| `UpdateBatcher` | 16ms 时间基 + 10 计数基合并 |
| `render_dirty_regions` | 优化的脏区域渲染管线 |
| `PerformanceTrapDetector` | 持续低帧率检测，带可操作建议 |
| `AdaptivePerformanceThresholds` | 硬件定制的降级阈值 |
