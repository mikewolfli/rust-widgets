# 嵌入式 / 资源受限支持

> **本章已重写。** 此前它记录的是一套 `rust_widgets::embedded` 模块，包含
> `EmbeddedConfig`、`ResourceManager`、`WidgetPool`、`DpiScaler`、
> `LightweightStyle`、`InputFilter`、`TouchPoint`、`init_embedded`、
> `init_desktop`。**crate 中不存在该模块** —— `grep -n "pub mod" src/lib.rs | grep -i embed`
> 没有任何输出，而旧章节里每一处 `use rust_widgets::embedded::…` 都无法解析，
> 约 660 行示例**无法编译**。下文描述的是真实存在的接口。

rust-widgets 通过两个**device profile** 支持资源受限目标，二者在构建期选择：

| Profile | Feature | 改变什么 |
|---|---|---|
| `mini` | `--features mini` | 分配节俭档。`mini` 会让 crate 切换到 `no_std`；控件集被裁剪，且不链接平台运行时。 |
| `embedded` | `--features embedded` | 嵌入式绘制面，控件集被裁剪。 |

这两个 profile 与 `desktop`、`tablet`、`mobile` 是**互斥**的，因此一次构建只指定一个：

```bash
cargo check --no-default-features --features mini
cargo check --no-default-features --features embedded
```

> **不要**用 `--all-features` 验证：它会同时打开 `desktop` 与 `mini`，而 `mini` 是
> `no_std`，该组合**无法编译**，不构成有效检查。

## 裁剪档究竟去掉了什么

门控名在 `build.rs` 中统一定义，模块按**意图**引用，而不是各自重推一遍条件：

| 别名 | 条件 | 含义 |
|---|---|---|
| `device_profile` | `desktop \| tablet \| mobile` | 是否设备构建？ |
| `full_widgets` | `device_profile && !(mini \| embedded)` | 完整控件集 + 设备传输层 |
| `widgets_unstripped` | `!(mini \| embedded)` | 控件集未被裁剪 |
| `alloc_frugal` | `mini` | 适用分配预算 |
| `embedded_surface` | `embedded` | 嵌入式绘制面 |

最关键的区别是**行为**上的，而非结构上的：裁剪档会**如实**报告它无法承载绘制面，
而不是先挂载再交回一个空白窗口。

```rust
// `supports_surfaces()` 是现行名称（`supports_custom_widgets` 已是带弃用警告的别名）。
// 在 `mini`/`embedded` 上它返回 `false`，宿主据此可以拒绝挂载，而不是显示空白面。
// 这是一个**运行期事实** —— 调用方代码里没有 `cfg(target_os)`，也没有 profile 分支。
if !rust_widgets::supports_surfaces() {
    eprintln!("该配置无法承载绘制面");
}
```

## 查询当前构建支持什么

不要按 profile 分支，而是直接询问你需要的能力：

```rust
use rust_widgets::platform::{backend_name, capabilities};

// 编译进的是哪个后端 —— 一个运行期答案。
println!("backend: {}", backend_name());

// 后端报告的能力集。
let caps = capabilities();
println!("ime: {}, accessibility: {}, dpi_scaling: {}",
         caps.ime, caps.accessibility, caps.dpi_scaling);
```

这是整个库遵循的模式：平台与 profile 差异都是通过 platform trait 查询的**运行期事实**，
从不在调用方代码里做编译期分支（见 `docs/ARCHITECTURE.md`）。

## 用嵌入式绘制面重绘

`embedded_surface` 提供宿主用于 blit 的绘制面。几何信息里携带显式的 `stride`，
它与 `width * 4` 分开保存 —— GPU backbuffer 往往按对齐填充，若按紧密排列做整行拷贝，
恰好会在这些宿主上产生图像错切：

```rust
use rust_widgets::platform::{FrameBuffer, SurfaceGeometry};

// 宿主从自己的显示驱动填入这些值。
let geometry = SurfaceGeometry::tight(320, 240);
let mut buffer = FrameBuffer::new();
assert!(buffer.resize(geometry));

// `frame_mut()` 只交出当前区域；超出当前几何的内容永远不会被暴露。
if let Some(frame) = buffer.frame_mut() {
    println!("待 blit 的字节数：{}", frame.len());
}
```

## `alloc_frugal` 下的内存行为

`mini` 会设置 `alloc_frugal`，从而选中节俭分配路径。它们的行为刻意为**透明**而非伪装：

> 在 `alloc_frugal` 下，arena 记账是 no-op 且报告 0 字节，其文档明确写道：
> “不要用它作为内存用量度量。”一个看起来合理的值，正是本项目禁止的那类伪造报告。

如果你在受限目标上需要真实的分配数据，请从目标自身的 allocator 取，而不要取节俭档的桩值。

## 小结

- 用 `--no-default-features --features mini` 或 `--features embedded` 构建。
- 挂载绘制面前先问 `supports_surfaces()`。
- 用 `capabilities()` 查询能力，而不是按 profile 分支。
- 在 `alloc_frugal` 下应预期被如实标注的 no-op，而不是看似合理的数字。
