# 内存管理

rust-widgets 提供了一套全面的内存管理层，专为桌面和嵌入式目标设计。它包括池分配器、竞技场碰撞分配器、基于栈的分配器、内存压力监控以及专门的对象池——所有这些都针对可预测的性能和低碎片进行了优化。

## 架构概述

```
┌──────────────────────────────────────────┐
│              MemoryMonitor               │  ← 观察者模式，压力警报
├──────────────────────────────────────────┤
│           PoolManager                    │  ← 类型擦除注册表
├─────────────────────┬────────────────────┤
│  ObjectPool<T>      │ SharedPool<T>      │  ← 按类划分的类型化池
├─────────────────────┼────────────────────┤
│  BufferPool         │ StringPool         │  ← 专用池
├─────────────────────┼────────────────────┤
│  ArenaAllocator     │ StackAllocator     │  ← 线性 / 基于标记
├─────────────────────┼────────────────────┤
│  VecPool<T>         │ Poolable trait     │  ← 通用池 + 重置契约
└─────────────────────┴────────────────────┘
```

## MemoryStats — 分配/释放/峰值跟踪

`MemoryStats` 跟踪所有内存操作的生命周期并计算池命中率：

```rust
use rust_widgets::memory::MemoryStats;

let mut stats = MemoryStats::default();

// 记录分配
stats.record_allocation(1024);   // +1024 字节
stats.record_allocation(2048);   // +2048 字节
assert_eq!(stats.current_usage, 3072);
assert_eq!(stats.peak_usage, 3072);

// 记录释放
stats.record_deallocation(1024); // -1024 字节
assert_eq!(stats.current_usage, 2048);
assert_eq!(stats.peak_usage, 3072);  // 峰值永不降低

// 记录池命中和未命中
stats.record_pool_hit();
stats.record_pool_hit();
stats.record_pool_miss();
assert_eq!(stats.pool_hit_rate(), 2.0 / 3.0);  // 66.7%

println!(
    "总计分配: {} 字节\n\
     总计释放: {} 字节\n\
     当前: {} 字节\n\
     峰值: {} 字节\n\
     池命中: {}, 未命中: {}, 命中率: {:.1}%",
    stats.total_allocated,
    stats.total_freed,
    stats.current_usage,
    stats.peak_usage,
    stats.pool_hits,
    stats.pool_misses,
    stats.pool_hit_rate() * 100.0,
);
```

## ArenaAllocator — 带重置的碰撞分配

`ArenaAllocator` 预分配一个连续的内存块，每次分配将光标向前移动。它从不释放单个分配——整个竞技场一次性重置。这对于渲染中的每帧临时数据非常理想：

```rust
use rust_widgets::memory::ArenaAllocator;

// 预分配 1 MiB 竞技场
let mut arena = ArenaAllocator::new(1024 * 1024);
assert_eq!(arena.capacity(), 1_048_576);
assert_eq!(arena.used(), 0);
assert_eq!(arena.available(), 1_048_576);

// 碰撞分配类型化对象
let ptr: Option<std::ptr::NonNull<u32>> = arena.allocate::<u32>();
assert!(ptr.is_some());

let ptr2: Option<std::ptr::NonNull<f64>> = arena.allocate::<f64>();
assert!(ptr2.is_some());

println!("已用: {} 字节, 可用: {} 字节", arena.used(), arena.available());

// 重置整个竞技场 — 所有分配失效，偏移量回到 0
arena.reset();
assert_eq!(arena.used(), 0);

// 重置后，相同的内存可以重用
let ptr3 = arena.allocate::<u32>();
assert!(ptr3.is_some());
```

**使用场景：**
- **每帧临时数据：** 分配临时矩阵、顶点缓冲区、临时字符串——在帧结束时重置
- **关卡/场景加载：** 使用竞技场分配场景图节点——卸载时一次性释放整个场景
- **命令缓冲区：** 在竞技场中批量渲染命令——刷新到 GPU 并重置

## StackAllocator — 基于标记的回滚

`StackAllocator` 是一个基于 `Vec<u8>` 的线性分配器，支持通过标记进行保存/恢复。在作用域之前推入标记，在其中自由分配，然后弹出回标记以释放该作用域中分配的所有内容：

```rust
use rust_widgets::memory::StackAllocator;

let mut stack = StackAllocator::new(4096);  // 4 KiB 栈
assert_eq!(stack.capacity(), 4096);

// 在词法作用域之前推入标记
stack.push_marker();  // 标记 0

// 在作用域内分配
let ptr_a = stack.allocate(256, 8);   // 256 字节，8 字节对齐
assert!(ptr_a.is_some());

stack.push_marker();  // 标记 1（嵌套作用域）

let ptr_b = stack.allocate(512, 8);
assert!(ptr_b.is_some());

println!("作用域 2 已用: {} 字节", stack.used());

// 弹出到标记 1 — ptr_b 失效，ptr_a 仍然有效
stack.pop_to_marker();
println!("弹出到标记 1 后: {} 字节已用", stack.used());

// 弹出到标记 0 — 所有分配失效
stack.pop_to_marker();
assert_eq!(stack.used(), 0);  // 完全重置

// 清除所有
stack.clear();
assert_eq!(stack.used(), 0);
assert_eq!(stack.available(), 4096);
```

**使用场景：**
- **递归算法：** 在每个递归级别分配临时缓冲区
- **错误恢复：** 在危险操作前 `push_marker()`，出错时 `pop_to_marker()`
- **分层合成：** 分配层特定临时缓冲区，移动到下一层时回滚

## MemoryPressure — 5 个级别

```rust
use rust_widgets::memory::MemoryPressure;

assert_eq!(MemoryPressure::from_usage(25, 100),  MemoryPressure::None);
assert_eq!(MemoryPressure::from_usage(60, 100),  MemoryPressure::Low);
assert_eq!(MemoryPressure::from_usage(80, 100),  MemoryPressure::Medium);
assert_eq!(MemoryPressure::from_usage(90, 100),  MemoryPressure::High);
assert_eq!(MemoryPressure::from_usage(98, 100),  MemoryPressure::Critical);
```

| 压力级别 | 使用率 | 响应 |
|---------------|-------------|----------|
| `None` | < 50% | 无需操作 |
| `Low` | 50–70% | 监控；开始合并小分配 |
| `Medium` | 70–85% | 清除缓存，减小池大小 |
| `High` | 85–95% | 丢弃非关键池，积极 GC |
| `Critical` | ≥ 95% | 紧急裁剪，禁用功能 |

## MemoryMonitor — 带阈值警报的观察者模式

```rust
use rust_widgets::memory::{MemoryMonitor, MemoryPressure, MemoryPressureHandler};

// 记录压力变化的处理器
struct PressureLogger;
impl MemoryPressureHandler for PressureLogger {
    fn on_pressure(&mut self, pressure: MemoryPressure) {
        match pressure {
            MemoryPressure::None     => {},
            MemoryPressure::Low      => eprintln!("MEM: 低压力 — 监控中"),
            MemoryPressure::Medium   => eprintln!("MEM: 中等压力 — 清除缓存"),
            MemoryPressure::High     => eprintln!("MEM: 高压力 — 丢弃池"),
            MemoryPressure::Critical => eprintln!("MEM: 严重 — 紧急模式！"),
        }
    }
}

// 创建带阈值的监控器（100 MiB 警告，200 MiB 严重）
let mut monitor = MemoryMonitor::new(100 * 1024 * 1024, 200 * 1024 * 1024);

// 注册观察者
monitor.register_handler(Box::new(PressureLogger));

// 集成点：定期使用当前使用量调用 update()
monitor.update(50 * 1024 * 1024);   // 50 MiB → None
assert_eq!(monitor.pressure(), MemoryPressure::None);

monitor.update(110 * 1024 * 1024);  // 110 MiB → High（超过警告）
assert_eq!(monitor.pressure(), MemoryPressure::High);

monitor.update(210 * 1024 * 1024);  // 210 MiB → Critical
assert_eq!(monitor.pressure(), MemoryPressure::Critical);

// 记录单个操作
monitor.record_allocation(4096);
monitor.record_deallocation(2048);

// 查询统计信息
let stats = monitor.stats();
println!("当前使用量: {} 字节", stats.current_usage);
println!("峰值使用量: {} 字节", stats.peak_usage);
```

## ObjectPool&lt;T&gt; — 带最大容量的获取/释放

`ObjectPool<T>` 维护一个预分配的对象池，对象实现 `Poolable` trait。对象在释放时被回收（重置）而不是丢弃和重新分配：

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
        // 恢复到默认状态，不重新分配
        self.position = (0.0, 0.0);
        self.velocity = (0.0, 0.0);
        self.lifetime = 0.0;
        self.color = 0xFFFFFFFF;
    }
}

// 池：16 个初始对象，最大 1024，1.5× 增长因子
let config = PoolConfig {
    initial_size: 16,
    max_size: 1024,
    growth_factor: 1.5,
};
let mut pool: ObjectPool<Particle> = ObjectPool::new(config);

// 获取：回收现有对象（调用 reset()）或创建新对象
let mut particle = pool.acquire();
assert_eq!(pool.allocated(), 1);
assert_eq!(pool.available(), 15);  // 16 初始 - 1 已获取

// 修改粒子
particle.position = (100.0, 200.0);
particle.velocity = (0.5, -0.3);
particle.lifetime = 3.0;

// 释放：将对象返回池（重置 + 推回）
pool.release(particle);
assert_eq!(pool.allocated(), 0);
assert_eq!(pool.available(), 16);  // 回到 16

// 达到 max_size 后释放的对象会被丢弃（不入池）
let mut small_pool: ObjectPool<Particle> = ObjectPool::new(
    PoolConfig { initial_size: 2, max_size: 2, growth_factor: 1.0 }
);

// 收缩到合适大小
pool.shrink_to_fit();

// 清除所有池中对象
pool.clear();
```

## SharedPool&lt;T&gt; — 线程安全对象池

`SharedPool<T>` 将 `ObjectPool<T>` 包装在 `Arc<Mutex<>>` 中，用于跨多个线程的并发访问：

```rust
use rust_widgets::memory::{SharedPool, PoolConfig, PoolStats};

// 线程安全的缓冲区池
let shared_pool: SharedPool<Vec<u8>> = SharedPool::default();
// SharedPool::new(config) 用于自定义大小

// 克隆共享同一个底层池（Arc）
let pool_clone = shared_pool.clone();

// 在任何线程中获取和释放
let buf = shared_pool.acquire();
assert_eq!(buf.capacity(), 0); // Vec 默认

shared_pool.release(vec![1, 2, 3]);

// 查询统计信息
let stats: PoolStats = shared_pool.stats();
println!(
    "池统计: {} 可用, {} 已分配, 容量 {}",
    stats.available, stats.allocated, stats.capacity
);
```

线程安全说明：`SharedPool<T>` 要求 `T: Poolable + Send`。每次 `acquire()`/`release()` 调用会短暂锁定互斥锁，因此对于高频分配，建议使用每线程 `ObjectPool<T>` 实例。

## PoolManager — 类型擦除注册表

`PoolManager` 维护多个 `SharedPool<T>` 实例的类型擦除注册表，实现集中式生命周期管理：

```rust
use rust_widgets::memory::{PoolManager, SharedPool, PoolConfig};

let mut manager = PoolManager::new();

// 注册池 — 每个返回一个 SharedPool 句柄
let particle_pool: SharedPool<Particle> = manager.register::<Particle>(
    PoolConfig { initial_size: 32, max_size: 256, growth_factor: 1.5 }
);

let buffer_pool: SharedPool<Vec<u8>> = manager.register::<Vec<u8>>(
    PoolConfig { initial_size: 8, max_size: 64, growth_factor: 1.0 }
);

// 独立使用池
let particle = particle_pool.acquire();
let buf = buffer_pool.acquire();

// 一次性清除所有池（例如，场景切换时）
manager.clear_all();
```

## BufferPool, StringPool, VecPool

三种针对常见分配模式的专用池类型：

### BufferPool

```rust
use rust_widgets::memory::BufferPool;

// 4 KiB 缓冲区，4 个预分配，最多 64 个入池
let mut pool = BufferPool::new(4096, 4, 64);
assert_eq!(pool.available(), 4);

// 获取：回收现有缓冲区或分配新缓冲区
let buf1 = pool.acquire();
assert_eq!(buf1.len(), 4096);  // 零初始化
assert_eq!(pool.available(), 3);

// 获取自定义大小（如果 ≤ buffer_size 则回收，如果更大则新鲜分配）
let buf2 = pool.acquire_sized(512);
assert_eq!(buf2.len(), 512);
// buf2 重用了 4K 缓冲区，调整为 512 字节

let buf3 = pool.acquire_sized(8192);
assert_eq!(buf3.len(), 8192);
// buf3 是新鲜分配的（超过了 4K 缓冲区大小）

// 释放：如果容量匹配 buffer_size 则回收
pool.release(buf1);
pool.release(buf2);

println!(
    "可用: {}, 缓冲区大小: {} 字节",
    pool.available(),
    pool.buffer_size()
);

pool.clear();
```

### StringPool

```rust
use rust_widgets::memory::StringPool;

// 64 字符容量，8 个预分配，最多 128 个入池
let mut pool = StringPool::new(64, 8, 128);

// 获取一个 String
let mut s = pool.acquire();
s.push_str("Hello, world!");
assert_eq!(s, "Hello, world!");

// 返回池（清除，容量保留）
pool.release(s);
assert_eq!(pool.available(), 8);

// 超过 default_capacity 的字符串不会被入池
let mut large = pool.acquire();
// ... 构建一个非常长的字符串（容量 > 64）...
// pool.release(large); // 如果容量 > 64，可能被丢弃
```

### VecPool&lt;T&gt;

```rust
use rust_widgets::memory::VecPool;

// 16 元素容量，4 个预分配，最多 64 个入池
let mut pool: VecPool<f32> = VecPool::new(16, 4, 64);

// 获取一个 Vec
let mut vec = pool.acquire();
vec.extend_from_slice(&[1.0, 2.0, 3.0]);

// 返回池（清除，容量保留）
pool.release(vec);
assert_eq!(pool.available(), 4);

// 容量低于 default_capacity 的 Vec 在释放时会被丢弃
let mut small = Vec::with_capacity(4);
small.push(1);
// pool.release(small); // 丢弃 — 容量 4 < 16
```

## Poolable Trait

`Poolable` trait 定义了可通过池回收的对象的契约：

```rust
use rust_widgets::memory::Poolable;

// 最小实现：Default + Clone + reset()
#[derive(Default, Clone)]
struct RenderCommand {
    command_type: u32,
    data: Vec<u8>,
    transform: [f32; 16],
    clip_rect: Option<(i32, i32, u32, u32)>,
}

impl Poolable for RenderCommand {
    fn reset(&mut self) {
        // 恢复到默认状态 — 避免堆分配
        self.command_type = 0;
        self.data.clear();  // Vec 保留容量！
        self.transform = [0.0; 16];
        self.clip_rect = None;
    }
}

// 与 ObjectPool 一起使用
use rust_widgets::memory::ObjectPool;

let mut pool = ObjectPool::<RenderCommand>::default();
let mut cmd = pool.acquire();  // reset() 自动调用
// ... 使用 cmd ...
pool.release(cmd);  // reset() 在存储前再次调用
```

**`reset()` 的最佳实践：**
- 在 `Vec`/`String` 字段上调用 `clear()`（保留容量——下次使用避免重新分配）
- 将数字字段归零为 `Default` 值
- 将 `Option` 字段设置为 `None`
- 不要在 `reset()` 中分配——目标是避免堆操作

## 嵌入式内存优化模式

### 每帧竞技场重置

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
        self.arena.reset();  // 在 O(1) 时间内回收整个竞技场
    }

    fn alloc_scratch<T>(&mut self) -> Option<std::ptr::NonNull<T>> {
        self.arena.allocate::<T>()
    }

    fn end_frame(&mut self) {
        // 此处不重置竞技场 — 数据在帧内使用
        // 重置发生在下次 begin_frame() 时
    }
}
```

### 基于池的 Widget 回收

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

// 在渲染循环中：获取、使用、释放
fn draw_rect(pool: &mut ObjectPool<UiRect>, x: i32, y: i32, w: u32, h: u32) {
    let mut rect = pool.acquire();
    rect.x = x; rect.y = y; rect.w = w; rect.h = h;
    // ... 绘制矩形 ...
    pool.release(rect);
}
```

### 用于错误恢复的栈作用域

```rust
use rust_widgets::memory::StackAllocator;

fn process_with_rollback(alloc: &mut StackAllocator, input: &[u8]) -> Result<(), &str> {
    // 在可能失败的工作前保存位置
    alloc.push_marker();

    // 分配临时缓冲区
    let buf_ptr = alloc.allocate(input.len() * 2, 8)
        .ok_or("内存不足")?;

    // 处理数据（可能失败）
    if !process_data(buf_ptr, input) {
        // 失败时回滚 — 回收标记以来的所有分配
        alloc.pop_to_marker();
        return Err("处理失败");
    }

    // 成功：保留分配，仅丢弃标记
    alloc.pop_to_marker();
    Ok(())
}

fn process_data(_ptr: *mut u8, _input: &[u8]) -> bool {
    true
}
```

### MemoryMonitor 集成

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
        50 * 1024 * 1024,   // 50 MiB 警告
        80 * 1024 * 1024,   // 80 MiB 严重
    );

    let mut tuner = PoolTuner { current_pressure: MemoryPressure::None };
    monitor.register_handler(Box::new(tuner));

    // 定期检查实际堆状态
    // （此处为模拟；生产环境中使用全局分配器钩子或 jemalloc 统计）
    let estimated_usage = 45 * 1024 * 1024;
    monitor.update(estimated_usage);

    if monitor.pressure() >= MemoryPressure::Medium {
        // 清除非必要缓存
        // font_cache.clear();
        // image_cache.trim();
        // texture_atlas.compact();
    }

    if monitor.pressure() >= MemoryPressure::Critical {
        // 紧急：丢弃所有池，禁用动画，切换到低质量
    }
}
```

## 总结

| 组件 | 分配器类型 | 使用场景 |
|-----------|---------------|----------|
| `MemoryStats` | 不适用 | 跟踪分配/释放/峰值/命中率 |
| `ArenaAllocator` | 碰撞（线性） | 每帧临时数据、场景加载、命令缓冲区 |
| `StackAllocator` | 基于标记 | 递归算法、错误恢复、分层合成 |
| `ObjectPool<T>` | 池（回收） | 粒子系统、UI 矩形、渲染命令 |
| `SharedPool<T>` | 线程安全池 | 多线程粒子/命令池 |
| `PoolManager` | 类型擦除注册表 | 集中式池生命周期管理（clear_all） |
| `BufferPool` | 专用池 | 固定大小字节缓冲区重用 |
| `StringPool` | 专用池 | 字符串分配重用 |
| `VecPool<T>` | 专用池 | Vec 分配重用 |
| `Poolable` | Trait | 定义池对象的 reset() 契约 |
| `MemoryPressure` | 枚举 | 5 级压力分类 |
| `MemoryMonitor` | 观察者 | 基于阈值的警报，带处理器注册 |
