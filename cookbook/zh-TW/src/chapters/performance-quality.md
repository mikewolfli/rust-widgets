# 效能與品質

rust-widgets 提供一套整合的效能與品質管理系統，可自動調整渲染品質以維持流暢的幀率，
適用於多種硬體——從筆記型電腦整合型 GPU 到桌上型獨立 GPU，
以及從純 CPU 軟體光柵器到高階 GPU。

## 架構概述

效能-品質子系統由四個協作層組成：

```
┌─────────────────────────────────────────┐
│          QualityManager                  │  ← 滯迴圈式品質切換
├─────────────────────────────────────────┤
│  FrameTimeMonitor  │  GpuCapability      │  ← 硬體偵測 + 幀時間追蹤
├─────────────────────────────────────────┤
│  Profiler  │  FrameProfiler  │  Monitor  │  ← 儀器層
├─────────────────────────────────────────┤
│  DirtyRegionTracker  │  UpdateBatcher    │  ← 髒區域優化
└─────────────────────────────────────────┘
```

## 品質等級與適應性品質

### QualityLevel

`QualityLevel` 列舉定義了三種渲染等級：

```rust
use rust_widgets::quality::QualityLevel;

// QualityLevel::High   — 完整特效（抗鋸齒、陰影、複雜著色器）
// QualityLevel::Medium — 基本特效、簡單著色器、無陰影
// QualityLevel::Low    — 最小化渲染、純色填滿、跳過非關鍵元素

assert!(QualityLevel::Low < QualityLevel::Medium);
assert!(QualityLevel::Medium < QualityLevel::High);

// 導航輔助
assert_eq!(QualityLevel::High.lower(), Some(QualityLevel::Medium));
assert_eq!(QualityLevel::Low.higher(), Some(QualityLevel::Medium));
assert_eq!(QualityLevel::Low.lower(), None);

// 在範圍內夾取
let clamped = QualityLevel::High.clamp(QualityLevel::Medium, QualityLevel::High);
```

### QualityConfig

`QualityConfig` 控制所有滯迴圈參數：

```rust
use rust_widgets::quality::{QualityConfig, QualityLevel};

let config = QualityConfig {
    target_frame_rate: 60.0,            // 目標 FPS
    degrade_threshold: 1.5,             // 當幀時間 > 目標 1.5 倍時降級
    upgrade_threshold: 0.7,             // 當幀時間 < 目標 0.7 倍時升級
    max_quality: QualityLevel::High,    // 上限為 High
    min_quality: QualityLevel::Low,     // 下限為 Low
    degrade_frame_count: 5,             // 連續 5 幀緩慢 → 降級
    upgrade_frame_count: 10,            // 連續 10 幀快速 → 升級
};

// 計算閾值（目標 60 FPS ≈ 16.67ms 目標）：
// degrade_frame_duration() = 16.67ms × 1.5 = 25.0ms
// upgrade_frame_duration() = 16.67ms × 0.7 = 11.67ms

// 務必呼叫 normalized() 來夾取無效值
let safe_config = config.normalized();
// degrade_threshold 為 max(1.0, 1.5) = 1.5
// upgrade_threshold 為 clamp(0.7, 0.1, 1.0) = 0.7
```

### FrameTimeMonitor

`FrameTimeMonitor` 維護一個 60 幀的環形緩衝區，用於即時幀時間分析：

```rust
use rust_widgets::quality::FrameTimeMonitor;

let mut monitor = FrameTimeMonitor::new(60.0);  // 目標 60 FPS

// 模擬記錄 60 幀，每幀 16ms
for _ in 0..60 {
    monitor.record_frame(0.016);  // 16ms ≈ 62.5 FPS
}

println!("平均幀時間: {:.4}s", monitor.average_frame_time());
println!("目前 FPS: {:.1}", monitor.current_fps());

// 檢查是否應降級（連續 5 幀 > 25ms 閾值）
let should_degrade = monitor.should_degrade(0.025, 5);

// 檢查是否應升級（連續 5 幀 < 11.67ms 閾值）
let should_upgrade = monitor.should_upgrade(0.01167, 5);

// 重設並重新設定
monitor.reset();
monitor.set_target_frame_rate(30.0);  // 切換至 30 FPS 目標
```

### QualityManager — 滯迴圈式適應

`QualityManager` 封裝了 `FrameTimeMonitor`、`QualityConfig` 和 `GpuCapability`，
以提供自動的滯迴圈式品質轉換：

```rust
use rust_widgets::quality::{QualityManager, QualityConfig, QualityLevel, GpuCapability};

// 自動偵測 GPU 並從建議品質等級開始
let mut manager = QualityManager::new();
println!("初始品質: {:?}", manager.quality_level());
// 在等級 ≥ 4 的獨立 GPU 上：從 QualityLevel::High 開始

// --- 整合至渲染迴圈 ---
fn render_loop(manager: &mut QualityManager) {
    loop {
        let frame_start = std::time::Instant::now();

        // ... 渲染您的幀畫面 ...

        let frame_duration = frame_start.elapsed();
        manager.finish_frame(frame_duration);
        // ^ 此動作記錄幀時間並評估降級/升級條件

        let current = manager.quality_level();
        match current {
            QualityLevel::High   => { /* 完整特效 */ }
            QualityLevel::Medium => { /* 簡化著色器 */ }
            QualityLevel::Low    => { /* 僅純色填滿 */ }
        }

        println!(
            "品質: {:?}, FPS: {:.1}, 平均幀: {:.4}s",
            current,
            manager.current_fps(),
            manager.average_frame_time()
        );
    }
}
```

**手動品質控制：**

```rust
// 繞過滯迴圈直接設定
manager.set_quality_level(QualityLevel::Medium);

// 查詢設定狀態
let config = manager.config();
println!("降級幀數: {} frames", config.degrade_frame_count);

// 熱載入設定
let new_config = QualityConfig {
    target_frame_rate: 30.0,
    ..QualityConfig::default()
};
manager.set_config(new_config);

// 重設至初始狀態
manager.reset();
```

**滯迴圈邏輯**（實作於 `update_quality_level()`）：

| 目前 | 條件 | 動作 |
|---------|-----------|--------|
| High | N 個連續慢幀 | 降級 → Medium |
| Medium | N 個連續慢幀 | 降級 → Low |
| Medium | N 個連續快幀 | 升級 → High |
| Low | N 個連續快幀 | 升級 → Medium |

連續幀數在降級（預設 5 幀）與升級（預設 10 幀）時有所不同，
以防止因瞬態幀時間突波而造成的「震盪」。

## GPU 能力偵測

### GpuCapability — 5 個效能等級

```rust
use rust_widgets::quality::GpuCapability;

// 手動建構
let discrete_gpu = GpuCapability {
    supports_high_quality: true,
    is_integrated: false,
    performance_tier: 5,  // 1–5 級
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

// 根據 GPU 等級建議初始品質
assert_eq!(discrete_gpu.recommended_initial_quality(), QualityLevel::High);
assert_eq!(integrated_gpu.recommended_initial_quality(), QualityLevel::Medium);
assert_eq!(cpu_fallback.recommended_initial_quality(), QualityLevel::Low);
```

**從 wgpu 配接器資訊自動偵測：**

```rust
#[cfg(feature = "gpu-wgpu")]
async fn detect_gpu() -> GpuCapability {
    let instance = wgpu::Instance::default();
    if let Some(adapter) = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await {
        GpuCapability::from_adapter_info(&adapter.get_info())
    } else {
        GpuCapability::default_capability()  // 退回至等級 3
    }
}
```

偵測機制將 wgpu `DeviceType` 對應至效能等級：

| 裝置類型 | 等級 | 建議品質 |
|-------------|------|---------------------|
| `DiscreteGpu` | 5 | High |
| `IntegratedGpu` | 3 | Medium |
| `VirtualGpu` | 2 | Medium |
| `Other` | 2 | Medium |
| `Cpu` | 1 | Low |

## GpuManager — 配接器選取與操作模式

### GpuManager

`GpuManager` 處理 GPU 硬體偵測、多策略配接器選取以及適應性效能監控：

```rust
use rust_widgets::gpu::{GpuManager, GpuManagerBuilder, AdapterSelectionStrategy};

// 自動：選取最佳可用 GPU
async fn auto_setup() -> Result<(), Box<dyn std::error::Error>> {
    let manager = GpuManager::new().await?;

    println!("使用 GPU: {}", manager.adapter_info().name);
    println!("供應商: {}", manager.adapter_info().vendor);
    println("後端: {}", manager.adapter_info().backend);

    if manager.is_hardware() {
        println!("硬體加速渲染已啟用");
    } else if manager.is_software() {
        println!("軟體渲染已啟用（CPU）");
    }

    // 檢查是否在瀏覽器中執行（WebAssembly）
    println!("瀏覽器強制 iGPU: {:?}", manager.adapter_info().is_selected);

    Ok(())
}
```

### AdapterSelectionStrategy — 7 種選取策略

```rust
use rust_widgets::gpu::{
    AdapterSelectionStrategy, AdapterSelector, GpuDeviceType,
};
use std::sync::Arc;

async fn select_adapter() {
    let strategies = [
        AdapterSelectionStrategy::PreferPerformance,    // 最佳裸機效能
        AdapterSelectionStrategy::PreferPowerEfficiency,// 省電優先
        AdapterSelectionStrategy::ForceDiscrete,         // 必須使用獨立 GPU
        AdapterSelectionStrategy::ForceIntegrated,       // 必須使用整合型 GPU
        AdapterSelectionStrategy::ForceCpu,              // 僅軟體渲染
        AdapterSelectionStrategy::Auto,                  // 自動選取（預設）
    ];

    for strategy in strategies {
        let selector = AdapterSelector::new(strategy).allow_fallback(true);
        match selector.enumerate_adapters().await {
            Ok(adapters) => {
                println!(
                    "策略 {:?}: 找到 {} 個配接器",
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
        // wgpu 為基礎的 GPU 渲染
        println!("在獨立/整合型 GPU 上渲染");
    }
    GpuOperationMode::Software => {
        // CPU 光柵器（SoftwarePaintBackend）
        println!("在 CPU 上渲染");
    }
    GpuOperationMode::Hybrid => {
        // GPU 負責合成，CPU 負責特定階段
        println!("混合 GPU/CPU 模式");
    }
}
```

### GpuManagerBuilder

```rust
use rust_widgets::gpu::{GpuManagerBuilder, AdapterSelectionStrategy, QualityLevel};

let manager = GpuManagerBuilder::new()
    .strategy(AdapterSelectionStrategy::PreferPerformance)
    .allow_fallback(true)  // 若獨立 GPU 不可用則退回
    .target_quality(QualityLevel::High)
    .build()
    .await?;
```

### GpuManager 幀整合

```rust
fn render_frame(manager: &mut GpuManager) {
    manager.begin_frame();

    // ... 渲染工作 ...

    manager.end_frame();
    // ^ end_frame() 評估效能並更新品質追蹤器

    // 根據 GPU 能力 + 幀時間的目前適應性品質
    println!("品質: {:?}", manager.current_quality());

    // 檢查對使用者可見的建議行動
    for action in manager.recommended_actions() {
        println!("建議: {}", action.message());
        println!("  優先級: {}", action.priority());
        // GpuManagerAction 變體：
        // - SuggestSwitchToCpuMode
        // - SuggestRestartOutsideBrowser
        // - SuggestCloseOtherApplications
        // - SuggestReduceResolution
        // - SuggestUpdateDrivers
    }
}
```

## 效能監控與分析

### Profiler

`Profiler` 提供具聚合功能的命名區段儀器：

```rust
use rust_widgets::performance::Profiler;
use std::time::Duration;

let mut profiler = Profiler::new();

// 成對的命名 begin/end
profiler.begin("layout_pass");
// ... 佈局計算 ...
profiler.end();

profiler.begin("render_pass");
// ... 渲染 ...
profiler.end();

// 基於閉包的測量
let result = profiler.measure("parse_json", || {
    // ... 解析 JSON ...
    42
});

// 查詢個別統計
if let Some(entry) = profiler.get_stats("layout_pass") {
    println!("layout_pass 被呼叫 {} 次，總計: {:?}",
        entry.call_count, entry.duration);
}

// 獲取超過閾值的熱點
let hotspots = profiler.get_hotspots(Duration::from_millis(5));
for (name, duration) in &hotspots {
    println!("  {}: {:?}", name, duration);
}

// 產生人類可讀報告
let report = profiler.report();
println!("{}", report.to_string_summary());

// 重設以進入下一個測量視窗
profiler.reset();
```

### FrameProfiler

`FrameProfiler` 追蹤每幀時間以計算 FPS、最小/最大值及分區段明細：

```rust
use rust_widgets::performance::FrameProfiler;

let mut frame_profiler = FrameProfiler::new(60);  // 緩衝最近 60 幀

// 每幀整合
for frame in 0..120 {
    frame_profiler.begin_frame();

    frame_profiler.begin_section("input");
    // ... 處理事件 ...
    frame_profiler.end_section();

    frame_profiler.begin_section("layout");
    // ... 佈局階段 ...
    frame_profiler.end_section();

    frame_profiler.begin_section("draw");
    // ... 渲染 ...
    frame_profiler.end_section();

    frame_profiler.end_frame();
}

println!("FPS: {:.1}", frame_profiler.fps());
println!("平均幀時間: {:?}", frame_profiler.average_frame_time());
println!("最小幀時間: {:?}", frame_profiler.min_frame_time());
println!("最大幀時間: {:?}", frame_profiler.max_frame_time());
println!("已記錄幀數: {}", frame_profiler.frame_count());

// 分區段明細（跨幀聚合）
for (section, duration) in frame_profiler.sections() {
    println!("  {}: {:?}", section, duration);
}
```

### PerformanceMonitor

`PerformanceMonitor` 將兩個分析器合併為統一的介面：

```rust
use rust_widgets::performance::PerformanceMonitor;

let mut monitor = PerformanceMonitor::new();

// 在生產環境中停用以達到零開銷
monitor.disable();

// 除錯時啟用
monitor.enable();

// 幀整合
monitor.begin_frame();

monitor.measure("expensive_operation", || {
    // ... 被分析的任務 ...
    std::thread::sleep(std::time::Duration::from_millis(10));
});

monitor.end_frame();

// 產生綜合報告
if monitor.is_enabled() {
    let report = monitor.report();
    println!("{}", report.to_string_summary());
    // 輸出包含：
    //   FPS: 59.8
    //   平均幀: 16.723ms
    //   最小幀: 15.201ms
    //   最大幀: 45.332ms
    //   幀數: 120
    //   （以及分區段明細）
}
```

## 髒區域追蹤

### DirtyRegionTracker

追蹤需要重新渲染的矩形區域，避免全幀重繪：

```rust
use rust_widgets::performance::{DirtyRegionTracker, render_dirty_regions};
use rust_widgets::core::Rect;
use rust_widgets::render::{RenderContext, SoftwarePaintBackend, PaintBackend};
use rust_widgets::core::{Color, Size};

let mut tracker = DirtyRegionTracker::new();
// 或使用自訂最大區域限制：
// let mut tracker = DirtyRegionTracker::with_max_regions(200);

// 標記髒區域（例如當控制項變更時）
let region_id = tracker.add(Rect::new(0, 0, 100, 100));
tracker.add(Rect::new(50, 50, 100, 100));    // 重疊 — 將會合併
tracker.add_with_priority(Rect::new(200, 0, 50, 50), 9);  // 高優先級
tracker.add_with_layer(Rect::new(0, 200, 80, 30), 1);     // 特定圖層

// 合併重疊區域
tracker.merge();
assert_eq!(tracker.len(), 3);  // 1 個合併 + 2 個獨立

// 取得所有區域的邊界矩形
if let Some(bounding) = tracker.get_bounding_rect() {
    println!("邊界矩形: {:?}", bounding);
}

// 查詢與指定矩形相交的區域
let overlapping = tracker.get_regions_for_rect(&Rect::new(0, 0, 50, 50));

// 裁剪所有區域至裁切矩形
tracker.clip_to(&Rect::new(0, 0, 150, 150));

// 優化：合併 + 若超出上限則截斷
tracker.optimize();

// 渲染後清除
tracker.clear();
```

### WidgetDirtyState

使用 `ObjectId` 進行每個控制項的髒狀態追蹤：

```rust
use rust_widgets::performance::WidgetDirtyState;
use rust_widgets::core::{ObjectId, Rect};

let mut state = WidgetDirtyState::new();

let widget_a = ObjectId::new();
let widget_b = ObjectId::new();

// 標記控制項為髒
state.mark_dirty(widget_a, Rect::new(0, 0, 100, 50));
state.mark_dirty(widget_b, Rect::new(0, 50, 100, 50));

assert!(state.is_dirty(widget_a));
assert_eq!(state.len(), 2);

// 查詢髒矩形
if let Some(rect) = state.get_dirty_rect(widget_a) {
    println!("控制項 A 的髒矩形: {:?}", rect);
}

// 渲染後標記為乾淨
state.mark_clean(widget_a);
assert!(!state.is_dirty(widget_a));

// 一次取得所有髒矩形
let all_rects = state.get_all_rects();
println!("{} 個髒矩形", all_rects.len());

// 迭代髒控制項
for id in state.dirty_widgets() {
    println!("控制項 {:?} 需要重繪", id);
}

state.clear();
assert!(state.is_empty());
```

### render_dirty_regions — 優化渲染迴圈

`render_dirty_regions` 函式實作完整的髒區域渲染管線：

```rust
use rust_widgets::performance::{DirtyRegionTracker, render_dirty_regions};
use rust_widgets::render::{RenderContext, SoftwarePaintBackend, PaintBackend};
use rust_widgets::core::{Color, Size, Rect};

fn render_with_dirty_regions() {
    let mut backend = SoftwarePaintBackend::new(Size::new(800, 600), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);

    let mut tracker = DirtyRegionTracker::new();
    // 模擬：這些區域中的控制項已變更
    tracker.add(Rect::new(10, 10, 200, 100));
    tracker.add(Rect::new(300, 400, 150, 50));

    render_dirty_regions(&mut tracker, &mut ctx, |ctx| {
        // 此閉包對每個合併後的髒區域呼叫一次
        // 裁切矩形已推入上下文中
        // 在內部繪製完整幀內容——只有髒
        // 區域會被實際渲染
        draw_all_widgets(ctx);
    });
    // tracker 在渲染後自動清除

    backend.end_frame();
}

fn draw_all_widgets(_ctx: &mut RenderContext) {
    // ... 繪製每個控制項 ...
}
```

**渲染策略**（實作於 `render_dirty_regions`）：

1. **空** → 完全跳過（不渲染）
2. **1–16 個區域** → 使用裁切矩形分別重繪每個區域
3. **>16 個區域** → 退回至完整邊界矩形重繪
4. **渲染後** → 清除 tracker

## UpdateBatcher — 時間+計數合併

`UpdateBatcher` 將多個更新區域合併為批次，在逾時或計數閾值時排空：

```rust
use rust_widgets::performance::UpdateBatcher;
use rust_widgets::core::Rect;

// 16ms 批次（與 ~60 FPS 重新整理率匹配）
let mut batcher = UpdateBatcher::new(16);

// 在整個幀中收集更新區域
batcher.add(Rect::new(10, 10, 50, 30));
batcher.add(Rect::new(100, 50, 80, 40));
batcher.add(Rect::new(200, 20, 40, 60));

assert_eq!(batcher.len(), 3);

// 檢查是否該排空了（16ms 已過或 10+ 個待處理矩形）
if batcher.should_flush() {
    // 排空並傳回合併後的矩形
    let merged_rects = batcher.flush();
    println!("已排空 {} 個合併矩形", merged_rects.len());
    assert!(batcher.is_empty());
}

// flush_clipped 直接使用髒區域優化渲染
// let mut ctx = ...;
// batcher.flush_clipped(&mut ctx, |ctx| { draw_all_widgets(ctx); });

// 不清除直接清除
batcher.clear();
```

**排空閾值：**
- **基於時間：** `last_batch.elapsed() >= batch_timeout_ms`（預設：16ms）
- **基於計數：** `pending_updates.len() >= 10`

## 適應性渲染與動態品質

結合 `QualityManager`、髒區域與更新批次處理，形成完整的適應性渲染管線：

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

        // 根據品質等級決定渲染策略
        match self.quality_manager.quality_level() {
            QualityLevel::High => {
                // 完整特效：使用髒區域渲染
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.full_render(ctx),
                );
            }
            QualityLevel::Medium => {
                // 簡化：跳過陰影，降低著色器複雜度
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.simplified_render(ctx),
                );
            }
            QualityLevel::Low => {
                // 最小化：僅純色填滿，跳過非關鍵元素
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.minimal_render(ctx),
                );
            }
        }

        backend.end_frame();

        // 記錄幀時間 → 觸發滯迴圈式品質變更
        self.quality_manager.finish_frame(frame_start.elapsed());

        // 為下一幀批次處理髒區域更新
        if self.update_batcher.should_flush() {
            let rects = self.update_batcher.flush();
            for rect in rects {
                self.dirty_tracker.add(rect);
            }
        }
    }

    fn full_render(&self, ctx: &mut RenderContext) {
        // 繪製含陰影、漸層、抗鋸齒
    }

    fn simplified_render(&self, ctx: &mut RenderContext) {
        // 繪製含實心邊框，無陰影
    }

    fn minimal_render(&self, ctx: &mut RenderContext) {
        // 繪製含簡單填滿，跳過裝飾
    }
}
```

## 效能陷阱偵測

`PerformanceTrapDetector` 辨識持續的低幀率並提供可行的建議：

```rust
use rust_widgets::gpu::{
    PerformanceTrapDetector, PerformanceTrap, AdaptivePerformanceMonitor,
    AdaptivePerformanceThresholds,
};
use rust_widgets::quality::QualityLevel;

// 為整合型 GPU 設定閾值
let thresholds = AdaptivePerformanceThresholds::for_device_type(
    rust_widgets::gpu::GpuDeviceType::IntegratedGpu,
);
println!("目標 FPS: {}", thresholds.target_fps);
println!("降級閾值: {:.2}s", thresholds.degrade_duration());

// 建立適應性效能監控器
let mut performance_monitor = AdaptivePerformanceMonitor::for_device_type(
    rust_widgets::gpu::GpuDeviceType::IntegratedGpu,
);

// 幀整合
performance_monitor.begin_frame();
// ... 渲染 ...
performance_monitor.end_frame();

// 檢查效能
if performance_monitor.should_degrade() {
    println!("效能已下降 — 考慮降低品質");
}

if performance_monitor.should_upgrade() {
    println!("效能已恢復 — 考慮提高品質");
}

// 效能統計
let stats = performance_monitor.stats();
println!("目前 FPS: {:.1}", stats.current_fps);
println!("穩定性: {:.2}（越低越穩定）", stats.stability);
println!("記憶體壓力: {}", stats.is_memory_pressure);
println!("CPU 超載: {}", stats.is_cpu_overloaded);
```

**效能陷阱偵測：**

```rust
let mut trap_detector = PerformanceTrapDetector::new();

// 每幀檢查
if let Some(trap) = trap_detector.check(30.0) {  // 閾值：30 FPS
    match trap {
        PerformanceTrap::LowFrameRate { current_fps, threshold } => {
            eprintln!(
                "低幀率: {:.1} FPS（閾值: {:.1}）",
                current_fps, threshold
            );
        }
        PerformanceTrap::MemoryPressure { utilization } => {
            eprintln!("記憶體壓力: {:.1}% 已使用", utilization);
        }
        PerformanceTrap::CpuOverload { utilization } => {
            eprintln!("CPU 超載: {:.1}%", utilization);
        }
        PerformanceTrap::BrowserForcedIntegratedGpu => {
            eprintln!("瀏覽器強制使用整合型 GPU — 請在瀏覽器外重新啟動");
        }
    }

    println!("{}", trap.message());
    println!("  建議使用 CPU 模式: {}", trap.suggests_cpu_mode());
    println!("  建議重新啟動:  {}", trap.suggests_restart());
}
```

### AdaptivePerformanceThresholds

針對不同裝置類型量身打造的硬體閾值：

```rust
use rust_widgets::gpu::{AdaptivePerformanceThresholds, GpuDeviceType};

// 獨立 GPU：積極升級品質，保守降級
let discrete = AdaptivePerformanceThresholds::discrete();
println!("獨立 GPU 目標: {} FPS", discrete.target_fps);
println!("  降級延遲幀數: {}", discrete.degrade_frame_count);
println!("  升級延遲幀數: {}", discrete.upgrade_frame_count);

// 整合型 GPU：平衡閾值
let integrated = AdaptivePerformanceThresholds::integrated();

// CPU 渲染：保守，較易降級
let cpu = AdaptivePerformanceThresholds::cpu();

// 根據裝置類型自動選取
let auto = AdaptivePerformanceThresholds::for_device_type(GpuDeviceType::DiscreteGpu);
```

## 總結

| 元件 | 用途 |
|-----------|---------|
| `QualityManager` | 適應性品質含滯迴圈（5 幀降級、10 幀升級） |
| `QualityLevel` | High / Medium / Low 三級定義 |
| `QualityConfig` | 目標 FPS、閾值、幀數計數 |
| `FrameTimeMonitor` | 用於幀時間追蹤的 60 幀環形緩衝區 |
| `GpuCapability` | 從 wgpu 配接器資訊進行的 5 級 GPU 偵測 |
| `GpuManager` | 配接器選取（7 種策略）、操作模式、緩衝區池 |
| `Profiler` | 含熱點偵測的命名區段儀器 |
| `FrameProfiler` | 每幀時間、FPS、最小/最大幀時間 |
| `PerformanceMonitor` | 統一的 Profiler + FrameProfiler，含報告功能 |
| `DirtyRegionTracker` | 基於區域的髒追蹤，含合併/優化 |
| `WidgetDirtyState` | 使用 ObjectId 的每個控制項髒狀態 |
| `UpdateBatcher` | 16ms 時間基礎 + 10 計數基礎合併 |
| `render_dirty_regions` | 優化的髒區域渲染管線 |
| `PerformanceTrapDetector` | 持續低 FPS 偵測，附可行建議 |
| `AdaptivePerformanceThresholds` | 硬體量身打造的降級閾值 |
