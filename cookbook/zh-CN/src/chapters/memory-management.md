# 内存管理

rust-widgets **不提供**手写分配器层。分配遵循 Rust 的所有权模型（原则 #30）：控件拥有自己的状态，
缓存是带明确淘汰规则的有界集合，公开 API 中没有任何 `alloc`/`free` 式的池化入口。

> **2026-11-11 变更（issue-20261111 R-24）。** 早期版本带有 `rust_widgets::memory` 模块
> （`ArenaAllocator`、`StackAllocator`、`BufferPool`、`StringPool`、`VecPool`、`ObjectPool`、
> `SharedPool`、`PoolManager`、`MemoryMonitor`、`MemoryStats`）。它在库、demo、绑定与测试中
> **零消费者**——模块文档自己写着 "Reserved public API"——因此被删除，而不是保留为第二套分配词汇。
> 曾引用它的代码请改用标准库（`Vec`、`Box`、`Arc`）或下文列出的有界缓存。
> 删除正是"不要构建无人使用的东西"的诚实形态。

## 真正管理内存的设施

| 关注点 | 位置 | 上界 |
|---|---|---|
| 解码图像复用 | `src/image/cache.rs` | 可配置字节预算，真 LRU |
| 控件光栅层 | `src/widget/runtime/cache.rs` | `MAX_CACHED_WIDGET_LAYERS`（128）+ LRU-clock |
| 文本整形 / 度量 / 覆盖 | `src/render/text/` | 有界表，按字体代次失效 |
| CSS 解析 | `src/style/` | 四级有界缓存（G-161） |
| GPU 上传 | `src/wgpu_backend/renderer/resources.rs` | LRU 纹理与覆盖缓存 |
| GPU 暂存缓冲 | `src/gpu/` | 只增 arena，跨帧复用 |

以上每一项都是**有明确上界与淘汰规则**的缓存，而不是通用对象池。缓存允许随时丢弃条目，
因此库从不发放"池化对象"，也从不要求宿主归还。

## 编写低分配控件

- 复用每帧暂存缓冲（`RenderContext` 自带），只在几何真正变化时分配。
- 像素载荷优先使用 `Arc<[u8]>`：记录图像与图层路径可以共享同一份分配，而不是拷贝。
- 每控件状态放在控件自身，而不是全局表；运行时注册表已经拥有控件，第二张表就是第二个事实来源
  （原则 #101）。
- 嵌入式 profile 通过 `alloc_frugal`/`mini` 门控**整体编译掉**依赖分配器的子系统，而不是模拟它们。

## 嵌入式 profile

`mini`/`embedded` 构建会编译掉控件注册表、主题以及所有需要分配器的缓存。那里的控件由宿主直接驱动，
没有池需要管理，也没有后台保留。profile 门控见 `docs/plans/principle.md` #47。

## 小结

- 没有手写内存池 API；使用所有权 + 有界缓存。
- 原 `memory` 模块已按证据删除（R-24）。
- 每个缓存的上界与淘汰规则都写在实现它的代码旁边。
