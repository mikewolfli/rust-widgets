# Performance & Quality

rust-widgets provides an integrated performance and quality management system
that automatically adapts rendering fidelity to maintain smooth frame rates
across diverse hardware — from integrated laptop GPUs to discrete desktop GPUs,
and from CPU-only software rasterizers to high-end GPUs.

## Architecture Overview

The performance-quality subsystem consists of four cooperating layers:

```
┌─────────────────────────────────────────┐
│          QualityManager                  │  ← hysteresis-based quality switching
├─────────────────────────────────────────┤
│  FrameTimeMonitor  │  GpuCapability      │  ← hardware detection + frame tracking
├─────────────────────────────────────────┤
│  Profiler  │  FrameProfiler  │  Monitor  │  ← instrumentation layer
├─────────────────────────────────────────┤
│  DirtyRegionTracker  │  UpdateBatcher    │  ← dirty-region optimization
└─────────────────────────────────────────┘
```

## Quality Levels and Adaptive Quality

### QualityLevel

The `QualityLevel` enum defines three rendering tiers:

```rust
use rust_widgets::quality::QualityLevel;

// QualityLevel::High   — full effects (anti-aliasing, shadows, complex shaders)
// QualityLevel::Medium — basic effects, simple shaders, no shadows
// QualityLevel::Low    — minimal rendering, solid fills, skip non-critical elements

assert!(QualityLevel::Low < QualityLevel::Medium);
assert!(QualityLevel::Medium < QualityLevel::High);

// Navigation helpers
assert_eq!(QualityLevel::High.lower(), Some(QualityLevel::Medium));
assert_eq!(QualityLevel::Low.higher(), Some(QualityLevel::Medium));
assert_eq!(QualityLevel::Low.lower(), None);

// Clamp within range
let clamped = QualityLevel::High.clamp(QualityLevel::Medium, QualityLevel::High);
```

### QualityConfig

The `QualityConfig` controls all hysteresis parameters:

```rust
use rust_widgets::quality::{QualityConfig, QualityLevel};

let config = QualityConfig {
    target_frame_rate: 60.0,            // Target FPS
    degrade_threshold: 1.5,             // Degrade when frame time > 1.5× target
    upgrade_threshold: 0.7,             // Upgrade when frame time < 0.7× target
    max_quality: QualityLevel::High,    // Cap at High
    min_quality: QualityLevel::Low,     // Floor at Low
    degrade_frame_count: 5,             // 5 consecutive slow frames → degrade
    upgrade_frame_count: 10,            // 10 consecutive fast frames → upgrade
};

// Computed thresholds (from target 60 FPS = ~16.67ms target):
// degrade_frame_duration() = 16.67ms × 1.5 = 25.0ms
// upgrade_frame_duration() = 16.67ms × 0.7 = 11.67ms

// Always call normalized() to clamp invalid values
let safe_config = config.normalized();
// degrade_threshold is max(1.0, 1.5) = 1.5
// upgrade_threshold is clamp(0.7, 0.1, 1.0) = 0.7
```

### FrameTimeMonitor

The `FrameTimeMonitor` maintains a 60-frame ring buffer for real-time frame time
analysis:

```rust
use rust_widgets::quality::FrameTimeMonitor;

let mut monitor = FrameTimeMonitor::new(60.0);  // target 60 FPS

// Simulate recording 60 frames at 16ms each
for _ in 0..60 {
    monitor.record_frame(0.016);  // 16ms ≈ 62.5 FPS
}

println!("Average frame time: {:.4}s", monitor.average_frame_time());
println!("Current FPS: {:.1}", monitor.current_fps());

// Check if we should degrade (5 consecutive frames > 25ms threshold)
let should_degrade = monitor.should_degrade(0.025, 5);

// Check if we should upgrade (5 consecutive frames < 11.67ms threshold)
let should_upgrade = monitor.should_upgrade(0.01167, 5);

// Reset and reconfigure
monitor.reset();
monitor.set_target_frame_rate(30.0);  // Switch to 30 FPS target
```

### QualityManager — Hysteresis-Based Adaptation

`QualityManager` wraps `FrameTimeMonitor`, `QualityConfig`, and `GpuCapability`
to provide automatic hysteresis-based quality transitions:

```rust
use rust_widgets::quality::{QualityManager, QualityConfig, QualityLevel, GpuCapability};

// Auto-detect GPU and start at recommended quality level
let mut manager = QualityManager::new();
println!("Initial quality: {:?}", manager.quality_level());
// On a discrete GPU with tier ≥ 4: starts at QualityLevel::High

// --- Integration into render loop ---
fn render_loop(manager: &mut QualityManager) {
    loop {
        let frame_start = std::time::Instant::now();

        // ... render your frame ...

        let frame_duration = frame_start.elapsed();
        manager.finish_frame(frame_duration);
        // ^ This records the frame time and evaluates degrade/upgrade conditions

        let current = manager.quality_level();
        match current {
            QualityLevel::High   => { /* full effects */ }
            QualityLevel::Medium => { /* simplified shaders */ }
            QualityLevel::Low    => { /* solid fills only */ }
        }

        println!(
            "Quality: {:?}, FPS: {:.1}, Avg frame: {:.4}s",
            current,
            manager.current_fps(),
            manager.average_frame_time()
        );
    }
}
```

**Manual quality control:**

```rust
// Bypass hysteresis and set directly
manager.set_quality_level(QualityLevel::Medium);

// Query configuration state
let config = manager.config();
println!("Degrade threshold: {} frames", config.degrade_frame_count);

// Hot-reload configuration
let new_config = QualityConfig {
    target_frame_rate: 30.0,
    ..QualityConfig::default()
};
manager.set_config(new_config);

// Reset to initial state
manager.reset();
```

**Hysteresis logic** (implemented in `update_quality_level()`):

| Current | Condition | Action |
|---------|-----------|--------|
| High | N consecutive slow frames | Degrade → Medium |
| Medium | N consecutive slow frames | Degrade → Low |
| Medium | N consecutive fast frames | Upgrade → High |
| Low | N consecutive fast frames | Upgrade → Medium |

The consecutive frame count differs for degrade (default 5) vs upgrade (default 10),
preventing "oscillation" from transient frame time spikes.

## GPU Capability Detection

### GpuCapability — 5 Performance Tiers

```rust
use rust_widgets::quality::GpuCapability;

// Manual construction
let discrete_gpu = GpuCapability {
    supports_high_quality: true,
    is_integrated: false,
    performance_tier: 5,  // 1–5 scale
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

// Recommended initial quality based on GPU tier
assert_eq!(discrete_gpu.recommended_initial_quality(), QualityLevel::High);
assert_eq!(integrated_gpu.recommended_initial_quality(), QualityLevel::Medium);
assert_eq!(cpu_fallback.recommended_initial_quality(), QualityLevel::Low);
```

**Automatic detection from wgpu adapter info:**

```rust
#[cfg(feature = "gpu-wgpu")]
async fn detect_gpu() -> GpuCapability {
    let instance = wgpu::Instance::default();
    if let Some(adapter) = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await {
        GpuCapability::from_adapter_info(&adapter.get_info())
    } else {
        GpuCapability::default_capability()  // fallback to tier 3
    }
}
```

The detection maps wgpu `DeviceType` to performance tiers:

| Device Type | Tier | Recommended Quality |
|-------------|------|---------------------|
| `DiscreteGpu` | 5 | High |
| `IntegratedGpu` | 3 | Medium |
| `VirtualGpu` | 2 | Medium |
| `Other` | 2 | Medium |
| `Cpu` | 1 | Low |

## GpuManager — Adapter Selection and Operation Modes

### GpuManager

The `GpuManager` handles GPU hardware detection, adapter selection with
multiple strategies, and adaptive performance monitoring:

```rust
use rust_widgets::gpu::{GpuManager, GpuManagerBuilder, AdapterSelectionStrategy};

// Automatic: selects the best available GPU
async fn auto_setup() -> Result<(), Box<dyn std::error::Error>> {
    let manager = GpuManager::new().await?;

    println!("Using GPU: {}", manager.adapter_info().name);
    println!("Vendor: {}", manager.adapter_info().vendor);
    println!("Backend: {}", manager.adapter_info().backend);

    if manager.is_hardware() {
        println!("Hardware-accelerated rendering active");
    } else if manager.is_software() {
        println!("Software rendering active (CPU)");
    }

    // Check if running inside a browser (WebAssembly)
    println!("Browser-forced iGPU: {:?}", manager.adapter_info().is_selected);

    Ok(())
}
```

### AdapterSelectionStrategy — 7 Selection Strategies

```rust
use rust_widgets::gpu::{
    AdapterSelectionStrategy, AdapterSelector, GpuDeviceType,
};
use std::sync::Arc;

async fn select_adapter() {
    let strategies = [
        AdapterSelectionStrategy::PreferPerformance,    // Best raw performance
        AdapterSelectionStrategy::PreferPowerEfficiency,// Battery-friendly
        AdapterSelectionStrategy::ForceDiscrete,         // Must use discrete GPU
        AdapterSelectionStrategy::ForceIntegrated,       // Must use integrated GPU
        AdapterSelectionStrategy::ForceCpu,              // Software rendering only
        AdapterSelectionStrategy::Auto,                  // Automatic selection (default)
    ];

    for strategy in strategies {
        let selector = AdapterSelector::new(strategy).allow_fallback(true);
        match selector.enumerate_adapters().await {
            Ok(adapters) => {
                println!(
                    "Strategy {:?}: {} adapters found",
                    strategy,
                    adapters.len()
                );
            }
            Err(e) => println!("Strategy {:?}: {}", strategy, e),
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
        // wgpu-based GPU rendering
        println!("Rendering on discrete/integrated GPU");
    }
    GpuOperationMode::Software => {
        // CPU rasterizer (SoftwarePaintBackend)
        println!("Rendering on CPU");
    }
    GpuOperationMode::Hybrid => {
        // GPU for composition, CPU for certain passes
        println!("Hybrid GPU/CPU mode");
    }
}
```

### GpuManagerBuilder

```rust
use rust_widgets::gpu::{GpuManagerBuilder, AdapterSelectionStrategy, QualityLevel};

let manager = GpuManagerBuilder::new()
    .strategy(AdapterSelectionStrategy::PreferPerformance)
    .allow_fallback(true)  // Fall back if discrete GPU unavailable
    .target_quality(QualityLevel::High)
    .build()
    .await?;
```

### GpuManager Frame Integration

```rust
fn render_frame(manager: &mut GpuManager) {
    manager.begin_frame();

    // ... render work ...

    manager.end_frame();
    // ^ end_frame() evaluates performance and updates quality tracker

    // Current adaptive quality based on GPU capability + frame times
    println!("Quality: {:?}", manager.current_quality());

    // Check for user-facing actions
    for action in manager.recommended_actions() {
        println!("Suggestion: {}", action.message());
        println!("  Priority: {}", action.priority());
        // GpuManagerAction variants:
        // - SuggestSwitchToCpuMode
        // - SuggestRestartOutsideBrowser
        // - SuggestCloseOtherApplications
        // - SuggestReduceResolution
        // - SuggestUpdateDrivers
    }
}
```

## Performance Monitoring and Profiling

### Profiler

The `Profiler` provides named-section instrumentation with aggregation:

```rust
use rust_widgets::performance::Profiler;
use std::time::Duration;

let mut profiler = Profiler::new();

// Named begin/end pairs
profiler.begin("layout_pass");
// ... layout computation ...
profiler.end();

profiler.begin("render_pass");
// ... rendering ...
profiler.end();

// Closure-based measurement
let result = profiler.measure("parse_json", || {
    // ... parse JSON ...
    42
});

// Query individual stats
if let Some(entry) = profiler.get_stats("layout_pass") {
    println!("layout_pass called {} times, total: {:?}",
        entry.call_count, entry.duration);
}

// Get hotspots exceeding a threshold
let hotspots = profiler.get_hotspots(Duration::from_millis(5));
for (name, duration) in &hotspots {
    println!("  {}: {:?}", name, duration);
}

// Generate human-readable report
let report = profiler.report();
println!("{}", report.to_string_summary());

// Reset for next measurement window
profiler.reset();
```

### FrameProfiler

`FrameProfiler` tracks per-frame timing to compute FPS, min/max, and per-section
breakdowns:

```rust
use rust_widgets::performance::FrameProfiler;

let mut frame_profiler = FrameProfiler::new(60);  // buffer last 60 frames

// Per-frame integration
for frame in 0..120 {
    frame_profiler.begin_frame();

    frame_profiler.begin_section("input");
    // ... process events ...
    frame_profiler.end_section();

    frame_profiler.begin_section("layout");
    // ... layout pass ...
    frame_profiler.end_section();

    frame_profiler.begin_section("draw");
    // ... rendering ...
    frame_profiler.end_section();

    frame_profiler.end_frame();
}

println!("FPS: {:.1}", frame_profiler.fps());
println!("Avg frame time: {:?}", frame_profiler.average_frame_time());
println!("Min frame time: {:?}", frame_profiler.min_frame_time());
println!("Max frame time: {:?}", frame_profiler.max_frame_time());
println!("Frames recorded: {}", frame_profiler.frame_count());

// Per-section breakdown (aggregated across frames)
for (section, duration) in frame_profiler.sections() {
    println!("  {}: {:?}", section, duration);
}
```

### PerformanceMonitor

`PerformanceMonitor` combines both profilers into a unified interface:

```rust
use rust_widgets::performance::PerformanceMonitor;

let mut monitor = PerformanceMonitor::new();

// Disable monitoring in production for zero overhead
monitor.disable();

// Enable for debugging
monitor.enable();

// Frame integration
monitor.begin_frame();

monitor.measure("expensive_operation", || {
    // ... profiled work ...
    std::thread::sleep(std::time::Duration::from_millis(10));
});

monitor.end_frame();

// Generate comprehensive report
if monitor.is_enabled() {
    let report = monitor.report();
    println!("{}", report.to_string_summary());
    // Output includes:
    //   FPS: 59.8
    //   Avg Frame: 16.723ms
    //   Min Frame: 15.201ms
    //   Max Frame: 45.332ms
    //   Frames: 120
    //   (and per-section breakdowns)
}
```

## Dirty Region Tracking

### DirtyRegionTracker

Tracks rectangular regions that need re-rendering, avoiding full-frame redraws:

```rust
use rust_widgets::performance::{DirtyRegionTracker, render_dirty_regions};
use rust_widgets::core::Rect;
use rust_widgets::render::{RenderContext, SoftwarePaintBackend, PaintBackend};
use rust_widgets::core::{Color, Size};

let mut tracker = DirtyRegionTracker::new();
// Or with custom max region limit:
// let mut tracker = DirtyRegionTracker::with_max_regions(200);

// Mark dirty regions (e.g., when widgets change)
let region_id = tracker.add(Rect::new(0, 0, 100, 100));
tracker.add(Rect::new(50, 50, 100, 100));    // Overlaps — will merge
tracker.add_with_priority(Rect::new(200, 0, 50, 50), 9);  // High priority
tracker.add_with_layer(Rect::new(0, 200, 80, 30), 1);     // Specific layer

// Merge overlapping regions
tracker.merge();
assert_eq!(tracker.len(), 3);  // 1 merged + 2 separate

// Get bounding rect of all regions
if let Some(bounding) = tracker.get_bounding_rect() {
    println!("Bounding rect: {:?}", bounding);
}

// Query regions intersecting a given rect
let overlapping = tracker.get_regions_for_rect(&Rect::new(0, 0, 50, 50));

// Clip all regions to a clip rect
tracker.clip_to(&Rect::new(0, 0, 150, 150));

// Optimize: merge + truncate if exceeding max
tracker.optimize();

// Clear after rendering
tracker.clear();
```

### WidgetDirtyState

Per-widget dirty tracking using `ObjectId`:

```rust
use rust_widgets::performance::WidgetDirtyState;
use rust_widgets::core::{ObjectId, Rect};

let mut state = WidgetDirtyState::new();

let widget_a = ObjectId::new();
let widget_b = ObjectId::new();

// Mark widgets dirty
state.mark_dirty(widget_a, Rect::new(0, 0, 100, 50));
state.mark_dirty(widget_b, Rect::new(0, 50, 100, 50));

assert!(state.is_dirty(widget_a));
assert_eq!(state.len(), 2);

// Query dirty rects
if let Some(rect) = state.get_dirty_rect(widget_a) {
    println!("Widget A dirty rect: {:?}", rect);
}

// Mark clean after rendering
state.mark_clean(widget_a);
assert!(!state.is_dirty(widget_a));

// Get all dirty rects at once
let all_rects = state.get_all_rects();
println!("{} dirty rects", all_rects.len());

// Iterate dirty widgets
for id in state.dirty_widgets() {
    println!("Widget {:?} needs repaint", id);
}

state.clear();
assert!(state.is_empty());
```

### render_dirty_regions — Optimized Render Loop

The `render_dirty_regions` function implements the complete dirty-region
rendering pipeline:

```rust
use rust_widgets::performance::{DirtyRegionTracker, render_dirty_regions};
use rust_widgets::render::{RenderContext, SoftwarePaintBackend, PaintBackend};
use rust_widgets::core::{Color, Size, Rect};

fn render_with_dirty_regions() {
    let mut backend = SoftwarePaintBackend::new(Size::new(800, 600), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);

    let mut tracker = DirtyRegionTracker::new();
    // Simulate: widgets changed in these regions
    tracker.add(Rect::new(10, 10, 200, 100));
    tracker.add(Rect::new(300, 400, 150, 50));

    render_dirty_regions(&mut tracker, &mut ctx, |ctx| {
        // This closure is called once per merged dirty region
        // with clip rects already pushed onto the context
        // Draw your full frame content inside — only dirty
        // regions will actually be rendered
        draw_all_widgets(ctx);
    });
    // tracker is automatically cleared after render

    backend.end_frame();
}

fn draw_all_widgets(_ctx: &mut RenderContext) {
    // ... draw every widget ...
}
```

**Rendering strategy** (implemented in `render_dirty_regions`):

1. **Empty** → skip entirely (no rendering)
2. **1–16 regions** → redraw each region separately with clip rects
3. **>16 regions** → fall back to full bounding-rect redraw
4. **After rendering** → tracker is cleared

## UpdateBatcher — Time+Count-Based Coalescing

`UpdateBatcher` coalesces multiple update regions into batches, flushing on
timeout or count threshold:

```rust
use rust_widgets::performance::UpdateBatcher;
use rust_widgets::core::Rect;

// 16ms batching (matches ~60 FPS refresh)
let mut batcher = UpdateBatcher::new(16);

// Collect update regions throughout the frame
batcher.add(Rect::new(10, 10, 50, 30));
batcher.add(Rect::new(100, 50, 80, 40));
batcher.add(Rect::new(200, 20, 40, 60));

assert_eq!(batcher.len(), 3);

// Check if it's time to flush (16ms elapsed or 10+ pending rects)
if batcher.should_flush() {
    // Flush returns merged rects
    let merged_rects = batcher.flush();
    println!("Flushed {} merged rects", merged_rects.len());
    assert!(batcher.is_empty());
}

// flush_clipped renders directly with dirty-region optimization
// let mut ctx = ...;
// batcher.flush_clipped(&mut ctx, |ctx| { draw_all_widgets(ctx); });

// Clear without rendering
batcher.clear();
```

**Flushing thresholds:**
- **Time-based:** `last_batch.elapsed() >= batch_timeout_ms` (default: 16ms)
- **Count-based:** `pending_updates.len() >= 10`

## Adaptive Rendering with Dynamic Quality

Combine `QualityManager`, dirty regions, and update batching for a complete
adaptive rendering pipeline:

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

        // Determine rendering strategy based on quality level
        match self.quality_manager.quality_level() {
            QualityLevel::High => {
                // Full effects: use dirty-region rendering
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.full_render(ctx),
                );
            }
            QualityLevel::Medium => {
                // Simplified: skip shadows, reduce shader complexity
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.simplified_render(ctx),
                );
            }
            QualityLevel::Low => {
                // Minimal: solid fills only, skip non-critical elements
                render_dirty_regions(
                    &mut self.dirty_tracker,
                    &mut ctx,
                    |ctx| self.minimal_render(ctx),
                );
            }
        }

        backend.end_frame();

        // Record frame time → triggers hysteresis-based quality changes
        self.quality_manager.finish_frame(frame_start.elapsed());

        // Batch dirty region updates for next frame
        if self.update_batcher.should_flush() {
            let rects = self.update_batcher.flush();
            for rect in rects {
                self.dirty_tracker.add(rect);
            }
        }
    }

    fn full_render(&self, ctx: &mut RenderContext) {
        // Draw with shadows, gradients, anti-aliasing
    }

    fn simplified_render(&self, ctx: &mut RenderContext) {
        // Draw with solid borders, no shadows
    }

    fn minimal_render(&self, ctx: &mut RenderContext) {
        // Draw with simple fills, skip decorations
    }
}
```

## Performance Trap Detection

The `PerformanceTrapDetector` identifies sustained low frame rates and provides
actionable suggestions:

```rust
use rust_widgets::gpu::{
    PerformanceTrapDetector, PerformanceTrap, AdaptivePerformanceMonitor,
    AdaptivePerformanceThresholds,
};
use rust_widgets::quality::QualityLevel;

// Configure thresholds for integrated GPU
let thresholds = AdaptivePerformanceThresholds::for_device_type(
    rust_widgets::gpu::GpuDeviceType::IntegratedGpu,
);
println!("Target FPS: {}", thresholds.target_fps);
println!("Degrade threshold: {:.2}s", thresholds.degrade_duration());

// Create adaptive performance monitor
let mut performance_monitor = AdaptivePerformanceMonitor::for_device_type(
    rust_widgets::gpu::GpuDeviceType::IntegratedGpu,
);

// Frame integration
performance_monitor.begin_frame();
// ... render ...
performance_monitor.end_frame();

// Check performance
if performance_monitor.should_degrade() {
    println!("Performance degraded — consider lowering quality");
}

if performance_monitor.should_upgrade() {
    println!("Performance recovered — consider raising quality");
}

// Performance statistics
let stats = performance_monitor.stats();
println!("Current FPS: {:.1}", stats.current_fps);
println!("Stability: {:.2} (lower = more stable)", stats.stability);
println!("Memory pressure: {}", stats.is_memory_pressure);
println!("CPU overloaded: {}", stats.is_cpu_overloaded);
```

**Performance trap detection:**

```rust
let mut trap_detector = PerformanceTrapDetector::new();

// Check each frame
if let Some(trap) = trap_detector.check(30.0) {  // threshold: 30 FPS
    match trap {
        PerformanceTrap::LowFrameRate { current_fps, threshold } => {
            eprintln!(
                "Low frame rate: {:.1} FPS (threshold: {:.1})",
                current_fps, threshold
            );
        }
        PerformanceTrap::MemoryPressure { utilization } => {
            eprintln!("Memory pressure: {:.1}% utilized", utilization);
        }
        PerformanceTrap::CpuOverload { utilization } => {
            eprintln!("CPU overload: {:.1}%", utilization);
        }
        PerformanceTrap::BrowserForcedIntegratedGpu => {
            eprintln!("Browser is forcing integrated GPU — restart outside browser");
        }
    }

    println!("{}", trap.message());
    println!("  Suggests CPU mode: {}", trap.suggests_cpu_mode());
    println!("  Suggests restart:  {}", trap.suggests_restart());
}
```

### AdaptivePerformanceThresholds

Hardware-tailored thresholds for different device types:

```rust
use rust_widgets::gpu::{AdaptivePerformanceThresholds, GpuDeviceType};

// Discrete GPU: aggressive quality upgrade, conservative degrade
let discrete = AdaptivePerformanceThresholds::discrete();
println!("Discrete GPU target: {} FPS", discrete.target_fps);
println!("  Degrade after {} slow frames", discrete.degrade_frame_count);
println!("  Upgrade after {} fast frames", discrete.upgrade_frame_count);

// Integrated GPU: balanced thresholds
let integrated = AdaptivePerformanceThresholds::integrated();

// CPU rendering: conservative, easier to degrade
let cpu = AdaptivePerformanceThresholds::cpu();

// Auto-select based on device type
let auto = AdaptivePerformanceThresholds::for_device_type(GpuDeviceType::DiscreteGpu);
```

## Summary

| Component | Purpose |
|-----------|---------|
| `QualityManager` | Adaptive quality with hysteresis (5-frame degrade, 10-frame upgrade) |
| `QualityLevel` | High / Medium / Low tier definitions |
| `QualityConfig` | Target FPS, thresholds, frame counts |
| `FrameTimeMonitor` | 60-frame ring buffer for frame time tracking |
| `GpuCapability` | 5-tier GPU detection from wgpu adapter info |
| `GpuManager` | Adapter selection (7 strategies), operation modes, buffer pools |
| `Profiler` | Named-section instrumentation with hotspot detection |
| `FrameProfiler` | Per-frame timing, FPS, min/max frame times |
| `PerformanceMonitor` | Unified Profiler + FrameProfiler with reports |
| `DirtyRegionTracker` | Region-based dirty tracking with merge/optimize |
| `WidgetDirtyState` | Per-widget dirty state using ObjectId |
| `UpdateBatcher` | 16ms time-based + 10-count-based coalescing |
| `render_dirty_regions` | Optimized dirty-region rendering pipeline |
| `PerformanceTrapDetector` | Sustained low-FPS detection with actionable suggestions |
| `AdaptivePerformanceThresholds` | Hardware-tailored degradation thresholds |
