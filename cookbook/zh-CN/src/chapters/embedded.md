# 嵌入式 / 资源受限支持

本章是面向受限目标的**实操指南**：如何选择 profile、运行期能拿到什么、在没有 OS 的情况下
如何驱动帧循环，以及如何把像素送到你自己的屏上。

下面每个示例都有一个**可运行**的对应文件 ——
[`examples/embedded_host.rs`](../../examples/embedded_host.rs) —— 你可以直接跑，而不用只凭信任：

```bash
cargo run --no-default-features --features embedded --example embedded_host
cargo run --no-default-features --features mini     --example embedded_host
```

> **本章已重写。** 此前它记录的是一套 `rust_widgets::embedded` 模块
> （`EmbeddedConfig`、`ResourceManager`、`WidgetPool`、`DpiScaler`、
> `LightweightStyle`、`InputFilter`、`TouchPoint`、`init_embedded`、`init_desktop`），
> 该模块在 crate 中**不存在**，约 660 行示例无法编译。本章所有内容都对照真实 API 核验过。
> 另见 [`docs/MIGRATION_GUIDE.md`](../../docs/MIGRATION_GUIDE.md)。

---

## 1. 选择 profile

有两个面向受限硬件的 profile。它们与 `desktop`、`tablet`、`mobile` **互斥**，
因此一次构建只指定一个。

| Profile | Feature | 你得到什么 | 代价 |
|---|---|---|---|
| `mini` | `--features mini` | 软件光栅、约 15 个核心控件、`heapless`/`bumpalo`/`spin` 分配器、完全不链接 OS 运行时。 | crate 变为 `no_std`；平台运行时不被链接。 |
| `embedded` | `--features embedded` | 软件光栅、无 GPU、无触摸、控件集被裁剪。 | 无 GPU 后端、无触摸。 |

```toml
# 你自己二进制的 Cargo.toml
[dependencies]
rust_widgets = { version = "2.7.0", default-features = false, features = ["embedded"] }
```

适合设备的 release profile —— 优先体积而非速度，并用 `panic = "abort"`，因为没有 unwinder：

```toml
[profile.release-embedded]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

> **不要用 `--all-features` 验证。** 它会同时打开 `desktop` 与 `mini`，而 `mini` 的 `no_std`
> 会移除 crate 大部分代码解析 `String`/`Vec` 所依赖的 `alloc` prelude，因此该组合无法编译。
> 它不是更强的检查，而是**根本不是检查**。

## 2. 询问当前构建是什么

宿主第一件该做的事是**询问**，而不是假设。每个 profile 事实都有运行期访问器
（`platform::profile`），因此同一份调用方代码在各目标上原样运行：

```rust
use rust_widgets::platform::profile;

println!("profile: {}", profile::profile_name());          // "mini" / "embedded" / "desktop"
println!("os runtime: {}", profile::has_os_runtime());      // mini 上为 false
println!("full widgets: {}", profile::full_widget_set());   // mini/embedded 上为 false
println!("alloc frugal: {}", profile::is_alloc_frugal());   // mini 上为 true
println!("surface usable: {}", rust_widgets::supports_surfaces());
```

各 profile 的实测输出：

| 查询 | `mini` | `embedded` | `desktop` |
|---|---|---|---|
| `profile_name()` | `mini` | `embedded` | `desktop` |
| `has_os_runtime()` | `false` | `false` | `true` |
| `full_widget_set()` | `false` | `false` | `true` |
| `is_alloc_frugal()` | **`true`** | `false` | `false` |
| `supports_surfaces()` | `false` | `false` | `true` |

这些门控名在 `build.rs` 中统一定义，模块按**意图**引用，而不是各自重推一遍合取式：

| 别名 | 条件 | 含义 |
|---|---|---|
| `device_profile` | `desktop \| tablet \| mobile` | 是否设备构建？ |
| `full_widgets` | `device_profile && !(mini \| embedded)` | 完整控件集 + 设备传输层 |
| `widgets_unstripped` | `!(mini \| embedded)` | 控件集未被裁剪 |
| `alloc_frugal` | `mini` | 适用分配预算 |
| `embedded_surface` | `embedded` | 嵌入式绘制面 |

### 什么时候 `cfg` 仍然是对的

本库的规则是：**调用方代码对"行为"用运行期查询分支**。但有一种情况编译期门控是正确的
——当差异是符号的**存在性**而非行为时。平台运行时
（`backend_name`、`capabilities`、`get_platform`、`quit`）不会被链接进 `mini`，因此：

```rust
use rust_widgets::platform::profile;

// 始终可用 —— 这些是对 profile 事实的 const fn。
println!("{}", profile::profile_name());

// 仅当链接了平台运行时才存在。
#[cfg(not(alloc_frugal))]
println!("backend: {}", rust_widgets::platform::backend_name());
```

这是**能力存在性**门控，不是行为分支。这个区分值得记清：`#[cfg]` 决定*代码是否参与编译*，
查询决定*代码做什么*。

## 3. 没有 OS 的帧循环

受限目标通常没有窗口系统来给你帧回调，因此本库提供**按帧节流的任务队列**：
你提交工作，它在下一次帧运行一次，并收到该帧的序号。

```rust
use rust_widgets::render_engine;

// 目标帧率被钳到 1..=240，并返回*实际生效*的值 ——
// 因此请求 999 的调用方会知道自己真正拿到了什么。
let applied = render_engine::set_embedded_target_fps(60);
assert!((1..=240).contains(&applied));
println!("target fps: {}", render_engine::embedded_target_fps());

// 把工作排到下一帧。
let task_id = render_engine::submit_embedded_task("tick", |frame_index| {
    // 在 `frame_index` 这一帧运行一次。
    let _ = frame_index;
});
println!("queued task {task_id}");
```

### 节流机制，以及为什么它不是细节

两个 profile 的节流方式不同，而这个差异在电池上很重要：

- **非 `mini`**（带线程宿主上的 `embedded`）：循环在 condvar 上停泊到下一帧截止时间，
  因此帧间 CPU 会睡眠。
- **`mini`**：没有第二个线程能唤醒循环，于是它**在同一个单调时钟上按同一份帧预算忙等**。
  没有这份预算，循环会全速空转、任务运行频率远超目标帧率要求 —— 这在设备上是发热与耗电问题，
  而不是外观问题。

无论哪种方式，循环都会在等待内部重新检查运行标志，因此从任务中发出的 `quit()`
会被及时观察到，而不是等到下一个截止时间。

## 4. 读取运行期状态

`embedded_engine_stats()` 是诊断入口 —— 看门狗或启动自检断言的就是这些数字：

```rust
use rust_widgets::render_engine;

let stats = render_engine::embedded_engine_stats();
println!("initialised: {}", stats.initialized);
println!("running:     {}", stats.running);
println!("frames:      {}", stats.frame_count);
println!("queued:      {}", stats.pending_task_count);
println!("windows:     {}", stats.window_count);
println!("buttons:     {}", stats.button_count);
println!("target fps:  {}", stats.target_fps);
```

`EmbeddedEngineStats` 派生 `Clone + Debug + PartialEq + Eq`，因此可以在测试里直接比较，
而不必逐字段判断。

引擎是**进程级单例**（一份绘制预算、一份注册表），因此两个会改动它的测试不能交错执行。
crate 内部的测试为此共享一把守卫锁串行化；宿主通常只读。

## 5. 把像素送到你的屏上

这是与桌面构建真正不同的地方：没有窗口、没有合成器，**缓冲区归你所有**。

### 分配绘制面

```rust
use rust_widgets::platform::{FrameBuffer, SurfaceGeometry};

// 注意显式的 `stride` —— 为什么它不等于 `width * 4`，见下。
let geometry = SurfaceGeometry { width: 320, height: 240, stride: 320 * 4 + 16 };
let mut frame = FrameBuffer::new();

if !frame.resize(geometry) {
    // stride 比一个紧密排列的行还窄时无法描述一帧，调用会拒绝，
    // 而不是产出一帧错切的数据。
    return;
}
println!("{} bytes", frame.frame().len());
```

当你的硬件没有填充时，用 `SurfaceGeometry::tight(w, h)` 构造紧密排列的几何。

### 为什么 `stride` 与 `width * 4` 分开

真实面板常按对齐填充，GPU backbuffer 往往也是。若代码假定第 `n` 行从 `n * width * 4` 开始，
就会**恰好**在这些宿主上产生图像错切。把 `stride` 显式化，意味着正确做法可直接表达，
而错误做法必须被刻意写出来。

### 重排外部缓冲区

当驱动布局不同（有填充，或原点自下而上）时，不应强迫每个控件去学那套布局。
`copy_rows` 把这一次转换收敛在一处：

```rust
use rust_widgets::platform::portable::copy_rows;
use rust_widgets::platform::SurfaceGeometry;

let geometry = SurfaceGeometry { width: 2, height: 2, stride: 12 }; // 4 字节填充
let src = [1u8; 24];
let mut dst = [0u8; 24];

assert!(copy_rows(&src, &mut dst, geometry));
// 填充字节保持不动，而不会被写入陈旧数据。
assert_eq!(&dst[8..12], &[0u8; 4]);
```

它在**两个方向**都做边界检查，并且宁可拒绝也不裁剪：放不下的区域是错误，
因为静默裁剪出来的帧是**会把自己藏起来的**渲染 bug。零尺寸绘制面被视为合法的空操作，
而不是失败。

### 没有 OS 在背后的宿主

对 `mini` —— 以及对没有后端模块的目标 —— 可移植宿主就是那个 `Platform` 实现：

```rust
use rust_widgets::platform::portable;

let host = portable::instance();     // 一个 StubPlatform：内存态、无 OS
assert_eq!(host.backend_name(), portable::BACKEND_NAME); // "portable"
assert_eq!(portable::FAMILY, rust_widgets::core::PlatformFamily::Embedded);
```

它刻意报告 `PlatformFamily::Embedded`：报告 `Desktop` 会让自适应代码为一个
没有窗口管理器的宿主挑选它无法兑现的默认值。

## 6. `alloc_frugal` 下的内存行为

`mini` 会设置 `alloc_frugal`，从而选中节俭分配路径。它们的行为刻意**透明而非看似合理**：

- arena 记账是 no-op，报告 `0` 字节，其文档明确写了这一点 ——
  *「不要用它作为内存用量度量。」*
- `Condvar` 桩无法停泊调用方，两个 notify 方法也都是空操作。

伪造一个数字正是本项目禁止的那类报告，因此桩选择如实作答。
**如果你在受限目标上需要真实分配数据，请从目标自身的 allocator 取**，而不要取这些桩值。

## 7. 裁剪档究竟去掉了什么

除了内存策略，裁剪档还会失去：

- **声明式视图层。** `crate::view` / `crate::json` / `crate::app` 门控在
  `declarative_view`（`device_profile && !stripped && !no-declarative-view`）上，
  因此 `mini`/`embedded` 上你能用的是命令式 `add_child` API。理由是分配预算，
  以及这些目标上没有调用方会逐帧重新求值一个视图。
- **自绘控件绘制面**，而且它会说出来：`supports_surfaces()` 返回 `false`，
  而不是先挂载再交回一个空白窗口。
- **平台运行时**（见 §2）。

菜单与快捷键**刻意不受影响** —— 它们的代码不带 `mini` 门控 —— 因此 `mini` 构建最准确的描述是
「没有自绘控件绘制面，但菜单完全可用」。

## 8. 检查清单

- [ ] 选定**一个** profile；用 `--no-default-features --features <profile>` 验证。
- [ ] 也跑 `cargo check --no-default-features --features mini --all-targets` 看警告。
- [ ] 挂载任何东西之前先问 `profile::*` 与 `supports_surfaces()`。
- [ ] 设置目标帧率，并读回**实际生效**的值。
- [ ] 提供绘制面，且 `stride` 必须准确。
- [ ] 填充缓冲区用 `copy_rows`，不要手写整行拷贝。
- [ ] 在 `alloc_frugal` 下预期被如实标注的 no-op；真实内存数字从你自己的 allocator 取。
- [ ] 在启动自检里对 `embedded_engine_stats()` 做断言。
