# 記憶體管理

rust-widgets 提供了一套完整的記憶體管理層，專為桌面與 embedded 目標所設計。它包含池配置器、arena bump 配置器、基於堆疊的配置器、記憶體壓力監控以及專門的物件池 — 所有元件皆針對可預測的效能與低碎片化進行最佳化。

## 架構概觀

```
┌──────────────────────────────────────────┐
│              MemoryMonitor               │  ← observer pattern, pressure alerts
├──────────────────────────────────────────┤
│           PoolManager                    │  ← type-erased registry
├─────────────────────┬────────────────────┤
│  ObjectPool<T>      │ SharedPool<T>      │  ← typed per-class pools
├─────────────────────┼────────────────────┤
│  BufferPool         │ StringPool         │  ← specialized pools
├─────────────────────┼────────────────────┤
│  ArenaAllocator     │ StackAllocator     │  ← linear / marker-based
├─────────────────────┼────────────────────┤
│  VecPool<T>         │ Poolable trait     │  ← generic pool + reset contract
└─────────────────────┴────────────────────┘
```

## MemoryStats — 配置 / 釋放 / 峰值追蹤

`MemoryStats` 追蹤所有記憶體操作的生命週期，並計算池命中率：

```rust
use rust_widgets::memory::MemoryStats;

let mut stats = MemoryStats::default();

// Record allocations
stats.record_allocation(1024);   // +1024 bytes
stats.record_allocation(2048);   // +2048 bytes
assert_eq!(stats.current_usage, 3072);
assert_eq!(stats.peak_usage, 3072);

// Record deallocations
stats.record_deallocation(1024); // -1024 bytes
assert_eq!(stats.current_usage, 2048);
assert_eq!(stats.peak_usage, 3072);  // Peak is never reduced

// Record pool hits and misses
stats.record_pool_hit();
stats.record_pool_hit();
stats.record_pool_miss();
assert_eq!(stats.pool_hit_rate(), 2.0 / 3.0);  // 66.7%

println!(
    "Total allocated: {} bytes\n\
     Total freed: {} bytes\n\
     Current: {} bytes\n\
     Peak: {} bytes\n\
     Pool hits: {}, misses: {}, hit rate: {:.1}%",
    stats.total_allocated,
    stats.total_freed,
    stats.current_usage,
    stats.peak_usage,
    stats.pool_hits,
    stats.pool_misses,
    stats.pool_hit_rate() * 100.0,
);
```

## ArenaAllocator — 帶重置的 Bump 配置

`ArenaAllocator` 預先配置一塊連續的記憶體區塊，並在每次配置時將游標向前推進。它從不單獨釋放單一配置 — 整個 arena 會一次全部重置。這非常適合渲染場景中每幀的暫存資料：

```rust
use rust_widgets::memory::ArenaAllocator;

// Pre-allocate 1 MiB arena
let mut arena = ArenaAllocator::new(1024 * 1024);
assert_eq!(arena.capacity(), 1_048_576);
assert_eq!(arena.used(), 0);
assert_eq!(arena.available(), 1_048_576);

// Bump-allocate typed objects
let ptr: Option<std::ptr::NonNull<u32>> = arena.allocate::<u32>();
assert!(ptr.is_some());

let ptr2: Option<std::ptr::NonNull<f64>> = arena.allocate::<f64>();
assert!(ptr2.is_some());

println!("Used: {} bytes, Available: {} bytes", arena.used(), arena.available());

// Reset the entire arena — all allocations invalidated, offset back to 0
arena.reset();
assert_eq!(arena.used(), 0);

// After reset, the same memory can be reused
let ptr3 = arena.allocate::<u32>();
assert!(ptr3.is_some());
```

**使用場景：**
- **每幀暫存資料：** 配置暫時的矩陣、頂點緩衝區、暫時字串 — 在幀結束時重置
- **關卡／場景載入：** 使用 arena 配置場景圖節點 — 卸載時一次釋放整個場景
- **指令緩衝區：** 在 arena 中批次渲染指令 — 刷新至 GPU 後重置

## StackAllocator — 基於標記的回退

`StackAllocator` 是一個以 `Vec<u8>` 為後端支援的線性配置器，支援透過標記進行儲存／恢復。在作用域之前推入標記，在作用域內自由配置，然後彈回標記以釋放該作用域中配置的所有內容：

```rust
use rust_widgets::memory::StackAllocator;

let mut stack = StackAllocator::new(4096);  // 4 KiB stack
assert_eq!(stack.capacity(), 4096);

// Push a marker before a lexical scope
stack.push_marker();  // Marker 0

// Allocate within the scope
let ptr_a = stack.allocate(256, 8);   // 256 bytes, 8-byte aligned
assert!(ptr_a.is_some());

stack.push_marker();  // Marker 1 (nested scope)

let ptr_b = stack.allocate(512, 8);
assert!(ptr_b.is_some());

println!("Scope 2 used: {} bytes", stack.used());

// Pop to Marker 1 — ptr_b is invalidated, ptr_a still valid
stack.pop_to_marker();
println!("After pop to marker 1: {} bytes used", stack.used());

// Pop to Marker 0 — all allocations invalidated
stack.pop_to_marker();
assert_eq!(stack.used(), 0);  // Fully reset

// Clear everything
stack.clear();
assert_eq!(stack.used(), 0);
assert_eq!(stack.available(), 4096);
```

**使用場景：**
- **遞迴演算法：** 在每個遞迴層級配置暫時緩衝區
- **錯誤復原：** 在風險操作前執行 `push_marker()`，在錯誤時執行 `pop_to_marker()`
- **分層合成：** 配置圖層專屬的暫存緩衝區，在移動至下一圖層時回退

## MemoryPressure — 5 個層級

```rust
use rust_widgets::memory::MemoryPressure;

assert_eq!(MemoryPressure::from_usage(25, 100),  MemoryPressure::None);
assert_eq!(MemoryPressure::from_usage(60, 100),  MemoryPressure::Low);
assert_eq!(MemoryPressure::from_usage(80, 100),  MemoryPressure::Medium);
assert_eq!(MemoryPressure::from_usage(90, 100),  MemoryPressure::High);
assert_eq!(MemoryPressure::from_usage(98, 100),  MemoryPressure::Critical);
```

| 壓力層級 | 使用比率 | 反應措施 |
|---------------|-------------|----------|
| `None` | < 50% | 無需動作 |
| `Low` | 50–70% | 監控；開始合併小型配置 |
| `Medium` | 70–85% | 清除快取，縮小池大小 |
| `High` | 85–95% | 捨棄非關鍵池，積極執行 GC |
| `Critical` | ≥ 95% | 緊急修剪，停用功能 |

## MemoryMonitor — 基於閾值的觀察者模式警示

```rust
use rust_widgets::memory::{MemoryMonitor, MemoryPressure, MemoryPressureHandler};

// Handler that logs pressure changes
struct PressureLogger;
impl MemoryPressureHandler for PressureLogger {
    fn on_pressure(&mut self, pressure: MemoryPressure) {
        match pressure {
            MemoryPressure::None     => {},
            MemoryPressure::Low      => eprintln!("MEM: Low pressure — monitoring"),
            MemoryPressure::Medium   => eprintln!("MEM: Medium pressure — purging caches"),
            MemoryPressure::High     => eprintln!("MEM: High pressure — dropping pools"),
            MemoryPressure::Critical => eprintln!("MEM: CRITICAL — emergency mode!"),
        }
    }
}

// Create monitor with thresholds (100 MiB warning, 200 MiB critical)
let mut monitor = MemoryMonitor::new(100 * 1024 * 1024, 200 * 1024 * 1024);

// Register observers
monitor.register_handler(Box::new(PressureLogger));

// Integration point: call update() periodically with current usage
monitor.update(50 * 1024 * 1024);   // 50 MiB → None
assert_eq!(monitor.pressure(), MemoryPressure::None);

monitor.update(110 * 1024 * 1024);  // 110 MiB → High (exceeds warning)
assert_eq!(monitor.pressure(), MemoryPressure::High);

monitor.update(210 * 1024 * 1024);  // 210 MiB → Critical
assert_eq!(monitor.pressure(), MemoryPressure::Critical);

// Record individual operations
monitor.record_allocation(4096);
monitor.record_deallocation(2048);

// Query stats
let stats = monitor.stats();
println!("Current usage: {} bytes", stats.current_usage);
println!("Peak usage: {} bytes", stats.peak_usage);
```

## ObjectPool&lt;T&gt; — 帶最大容量的 Acquire／Release

`ObjectPool<T>` 維護一個預先配置的物件池，這些物件實作了 `Poolable` trait。物件在釋放時會被回收（重置），而非直接丟棄後重新配置：

```rust
use rust_widgets::memory::{ObjectPool, PoolConfig, Poolable};

#[derive(Default, Clone)]
struct Particle {
    position: (f32, f32),
    velocity: (f32, f32),
    lifetime: f32,
    color: u32,
}

impl Poolable for Particle {
    fn reset(&mut self) {
        // Restore to default state without reallocation
        self.position = (0.0, 0.0);
        self.velocity = (0.0, 0.0);
        self.lifetime = 0.0;
        self.color = 0xFFFFFFFF;
    }
}

// Pool with 16 initial objects, max 1024, 1.5× growth factor
let config = PoolConfig {
    initial_size: 16,
    max_size: 1024,
    growth_factor: 1.5,
};
let mut pool: ObjectPool<Particle> = ObjectPool::new(config);

// Acquire recycles existing objects (calls reset()) or creates new ones
let mut particle = pool.acquire();
assert_eq!(pool.allocated(), 1);
assert_eq!(pool.available(), 15);  // 16 initial - 1 acquired

// Modify particle
particle.position = (100.0, 200.0);
particle.velocity = (0.5, -0.3);
particle.lifetime = 3.0;

// Release returns object to pool (reset + push back)
pool.release(particle);
assert_eq!(pool.allocated(), 0);
assert_eq!(pool.available(), 16);  // Back to 16

// Objects released after reaching max_size are dropped (not pooled)
let mut small_pool: ObjectPool<Particle> = ObjectPool::new(
    PoolConfig { initial_size: 2, max_size: 2, growth_factor: 1.0 }
);

// Shrink to fit
pool.shrink_to_fit();

// Clear all pooled objects
pool.clear();
```

## SharedPool&lt;T&gt; — 執行緒安全的物件池

`SharedPool<T>` 將 `ObjectPool<T>` 包裝在 `Arc<Mutex<>>` 中，以實現跨多個執行緒的並行存取：

```rust
use rust_widgets::memory::{SharedPool, PoolConfig, PoolStats};

// Thread-safe pool of buffers
let shared_pool: SharedPool<Vec<u8>> = SharedPool::default();
// SharedPool::new(config) for custom sizing

// Clone shares the same underlying pool (Arc)
let pool_clone = shared_pool.clone();

// Acquire and release from any thread
let buf = shared_pool.acquire();
assert_eq!(buf.capacity(), 0); // Vec default

shared_pool.release(vec![1, 2, 3]);

// Query stats
let stats: PoolStats = shared_pool.stats();
println!(
    "Pool stats: {} available, {} allocated, capacity {}",
    stats.available, stats.allocated, stats.capacity
);
```

執行緒安全提示：`SharedPool<T>` 要求 `T: Poolable + Send`。每次 `acquire()`／`release()` 呼叫都會短暫鎖定 mutex，因此對於高頻配置，建議優先使用每個執行緒各自的 `ObjectPool<T>` 實例。

## PoolManager — 型別抹消註冊表

`PoolManager` 維護多個 `SharedPool<T>` 實例的型別抹消註冊表，實現集中化的生命週期管理：

```rust
use rust_widgets::memory::{PoolManager, SharedPool, PoolConfig};

let mut manager = PoolManager::new();

// Register pools — each returns a SharedPool handle
let particle_pool: SharedPool<Particle> = manager.register::<Particle>(
    PoolConfig { initial_size: 32, max_size: 256, growth_factor: 1.5 }
);

let buffer_pool: SharedPool<Vec<u8>> = manager.register::<Vec<u8>>(
    PoolConfig { initial_size: 8, max_size: 64, growth_factor: 1.0 }
);

// Use pools independently
let particle = particle_pool.acquire();
let buf = buffer_pool.acquire();

// Clear all pools at once (e.g., on scene change)
manager.clear_all();
```

## BufferPool、StringPool、VecPool

三種針對常見配置模式設計的專門池類型：

### BufferPool

```rust
use rust_widgets::memory::BufferPool;

// 4 KiB buffers, 4 pre-allocated, max 64 pooled
let mut pool = BufferPool::new(4096, 4, 64);
assert_eq!(pool.available(), 4);

// Acquire recycles existing buffer or allocates new
let buf1 = pool.acquire();
assert_eq!(buf1.len(), 4096);  // zero-initialized
assert_eq!(pool.available(), 3);

// Acquire with custom size (recycles if ≤ buffer_size, allocates fresh if larger)
let buf2 = pool.acquire_sized(512);
assert_eq!(buf2.len(), 512);
// buf2 reused a 4K buffer, resized to 512 bytes

let buf3 = pool.acquire_sized(8192);
assert_eq!(buf3.len(), 8192);
// buf3 was fresh-allocated (exceeds 4K buffer size)

// Release recycles if capacity matches buffer_size
pool.release(buf1);
pool.release(buf2);

println!(
    "Available: {}, Buffer size: {} bytes",
    pool.available(),
    pool.buffer_size()
);

pool.clear();
```

### StringPool

```rust
use rust_widgets::memory::StringPool;

// 64-char capacity, 8 pre-allocated, max 128 pooled
let mut pool = StringPool::new(64, 8, 128);

// Acquire a String
let mut s = pool.acquire();
s.push_str("Hello, world!");
assert_eq!(s, "Hello, world!");

// Return to pool (cleared, capacity preserved)
pool.release(s);
assert_eq!(pool.available(), 8);

// Strings exceeding default_capacity won't be pooled
let mut large = pool.acquire();
// ... build a very long string (capacity > 64) ...
// pool.release(large); // This may drop large if capacity > 64
```

### VecPool&lt;T&gt;

```rust
use rust_widgets::memory::VecPool;

// 16-element capacity, 4 pre-allocated, max 64 pooled
let mut pool: VecPool<f32> = VecPool::new(16, 4, 64);

// Acquire a Vec
let mut vec = pool.acquire();
vec.extend_from_slice(&[1.0, 2.0, 3.0]);

// Return to pool (cleared, capacity preserved)
pool.release(vec);
assert_eq!(pool.available(), 4);

// Vecs with capacity below default_capacity are dropped on release
let mut small = Vec::with_capacity(4);
small.push(1);
// pool.release(small); // Dropped — capacity 4 < 16
```

## Poolable Trait

`Poolable` trait 定義了可透過池回收之物件的契約：

```rust
use rust_widgets::memory::Poolable;

// Minimal implementation: Default + Clone + reset()
#[derive(Default, Clone)]
struct RenderCommand {
    command_type: u32,
    data: Vec<u8>,
    transform: [f32; 16],
    clip_rect: Option<(i32, i32, u32, u32)>,
}

impl Poolable for RenderCommand {
    fn reset(&mut self) {
        // Restore to default state — avoids heap allocations
        self.command_type = 0;
        self.data.clear();  // Vec retains capacity!
        self.transform = [0.0; 16];
        self.clip_rect = None;
    }
}

// Usage with ObjectPool
use rust_widgets::memory::ObjectPool;

let mut pool = ObjectPool::<RenderCommand>::default();
let mut cmd = pool.acquire();  // reset() is called automatically
// ... use cmd ...
pool.release(cmd);  // reset() is called again before storage
```

**`reset()` 的最佳實踐：**
- 對 `Vec`／`String` 欄位呼叫 `clear()`（保留容量 — 下次使用時避免重新配置）
- 將數值型欄位歸零至 `Default` 值
- 將 `Option` 欄位設為 `None`
- 不要在 `reset()` 中進行配置 — 目標是避免堆積操作

## 適合 Embedded 的記憶體最佳化模式

### 每幀 Arena 重置

```rust
use rust_widgets::memory::ArenaAllocator;

struct FrameAllocator {
    arena: ArenaAllocator,
}

impl FrameAllocator {
    fn new(capacity: usize) -> Self {
        Self { arena: ArenaAllocator::new(capacity) }
    }

    fn begin_frame(&mut self) {
        self.arena.reset();  // Reclaim entire arena in O(1)
    }

    fn alloc_scratch<T>(&mut self) -> Option<std::ptr::NonNull<T>> {
        self.arena.allocate::<T>()
    }

    fn end_frame(&mut self) {
        // Arena is not reset here — data is used during the frame
        // Reset happens at next begin_frame()
    }
}
```

### 基於池的 Widget 回收

```rust
use rust_widgets::memory::{ObjectPool, PoolConfig, Poolable};

#[derive(Default, Clone)]
struct UiRect {
    x: i32, y: i32, w: u32, h: u32,
    color: u32,
    border_width: u32,
    visible: bool,
}

impl Poolable for UiRect {
    fn reset(&mut self) {
        *self = UiRect::default();
    }
}

let mut rect_pool = ObjectPool::<UiRect>::new(
    PoolConfig { initial_size: 64, max_size: 256, growth_factor: 1.0 }
);

// In render loop: acquire, use, release
fn draw_rect(pool: &mut ObjectPool<UiRect>, x: i32, y: i32, w: u32, h: u32) {
    let mut rect = pool.acquire();
    rect.x = x; rect.y = y; rect.w = w; rect.h = h;
    // ... draw the rect ...
    pool.release(rect);
}
```

### 堆疊作用域用於錯誤復原

```rust
use rust_widgets::memory::StackAllocator;

fn process_with_rollback(alloc: &mut StackAllocator, input: &[u8]) -> Result<(), &str> {
    // Save position before potentially failing work
    alloc.push_marker();

    // Allocate temporary buffer
    let buf_ptr = alloc.allocate(input.len() * 2, 8)
        .ok_or("OOM")?;

    // Process data (may fail)
    if !process_data(buf_ptr, input) {
        // Roll back on failure — reclaims all allocations since marker
        alloc.pop_to_marker();
        return Err("Processing failed");
    }

    // Success: keep allocation, just drop the marker
    alloc.pop_to_marker();
    Ok(())
}

fn process_data(_ptr: *mut u8, _input: &[u8]) -> bool {
    true
}
```

### MemoryMonitor 整合

```rust
use rust_widgets::memory::{MemoryMonitor, MemoryPressure, MemoryPressureHandler};

struct PoolTuner {
    current_pressure: MemoryPressure,
}

impl MemoryPressureHandler for PoolTuner {
    fn on_pressure(&mut self, pressure: MemoryPressure) {
        self.current_pressure = pressure;
    }
}

fn adaptive_memory_management() {
    let mut monitor = MemoryMonitor::new(
        50 * 1024 * 1024,   // 50 MiB warning
        80 * 1024 * 1024,   // 80 MiB critical
    );

    let mut tuner = PoolTuner { current_pressure: MemoryPressure::None };
    monitor.register_handler(Box::new(tuner));

    // Periodically check with actual heap state
    // (simulated here; in production, use a global allocator hook or jemalloc stats)
    let estimated_usage = 45 * 1024 * 1024;
    monitor.update(estimated_usage);

    if monitor.pressure() >= MemoryPressure::Medium {
        // Purge non-essential caches
        // font_cache.clear();
        // image_cache.trim();
        // texture_atlas.compact();
    }

    if monitor.pressure() >= MemoryPressure::Critical {
        // Emergency: drop all pools, disable animations, switch to Low quality
    }
}
```

## 總結

| 元件 | 配置器類型 | 使用場景 |
|-----------|---------------|----------|
| `MemoryStats` | 不適用 | 追蹤配置／釋放／峰值／命中率 |
| `ArenaAllocator` | Bump（線性） | 每幀暫存、場景載入、指令緩衝區 |
| `StackAllocator` | 基於標記 | 遞迴演算法、錯誤復原、分層合成 |
| `ObjectPool<T>` | 池（回收） | 粒子系統、UI 矩形、渲染指令 |
| `SharedPool<T>` | 執行緒安全池 | 多執行緒粒子／指令池 |
| `PoolManager` | 型別抹消註冊表 | 集中化池生命週期（clear_all） |
| `BufferPool` | 專門池 | 固定大小位元組緩衝區重用 |
| `StringPool` | 專門池 | 字串配置重用 |
| `VecPool<T>` | 專門池 | Vec 配置重用 |
| `Poolable` | Trait | 定義池化物件的 reset() 契約 |
| `MemoryPressure` | 列舉 | 5 層級壓力分類 |
| `MemoryMonitor` | 觀察者 | 基於閾值的警示，含 handler 註冊 |
