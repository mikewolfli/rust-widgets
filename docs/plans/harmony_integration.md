# 鸿蒙（HarmonyOS / OpenHarmony）接入方式

> 结论先行：**接入方式和桌面三平台不一样，但对 Rust 侧是"同一套 API"。**
> 桌面平台由 `rust_widgets` 自己驱动事件循环；鸿蒙**必须由 ArkTS 侧驱动**，
> Rust 只提供一个可轮询的队列。这就是为什么 `harmony_napi_bridge_flow.md` 里
> 全是 `rw_poll_*`。

## 1. 为什么不一样

| | 桌面（macOS/Windows/Linux） | 鸿蒙 |
|---|---|---|
| 事件循环归属 | `rust_widgets`（`crate::run()` 阻塞） | **ArkUI 运行时**，Rust 无法抢占 |
| 原生控件 | 自己 `NSView` / `HWND` / `gtk::Widget` | ArkUI 声明式组件，由 ArkTS 创建 |
| 回调方向 | OS → Rust（`msg_send` / `WM_*` / GTK signal） | **ArkTS → Rust**（NAPI），需要显式转发 |
| Rust 的角色 | 框架 | **被 ArkTS 调用的库** |

关键差异：桌面平台 Rust 持有主线程并跑事件循环；鸿蒙上 Rust 是 `cdylib`，主线程在
ArkUI 手里。所以 Rust 侧不能"等事件"，只能"被调用 + 提供队列让 ArkTS 来取"。

当前 `src/platform/harmony/` 是 **state-only 后端**作为默认形态：完整的 `Platform` 契约、菜单树、
剪贴板、拖放、IME 元数据都在进程内实现。

**但 `feature = "xcomponent"` 时不再只是 state-only**：`src/platform/harmony/xcomponent.rs`
绑定 `OH_NativeXComponent`，拿到真实的 surface 与 touch / mouse / key / focus 回调。
启用方式：

```bash
export OHOS_SDK_NATIVE=<sdk>/linux/native
cargo ohos build -t aarch64 --no-default-features \
  --features "harmony xcomponent desktop-runtime controls-custom"
```

ArkTS 侧的接入点只有两个新函数（已发布在 `include/rw_generated.h`）：

```text
XComponent({ id: 'rw', type: 'surface' })
  .onLoad((ctx) => rw_harmony_bind_xcomponent(ctx.xcomponentId ? getComponentPtr(ctx) : 0))
```

具体地，`onLoad` 回调里的 `OH_NativeXComponent*` 交给 `rw_harmony_bind_xcomponent`，
之后 surface 尺寸、touch、mouse、key、focus 全部自动进入控件树，**不需要宿主逐控件转发**。

未启用 `xcomponent` 时（默认）仍是 state-only 后端：完整 `Platform` 契约、菜单树、
剪贴板、拖放、IME 元数据都在进程内实现，但**不创建任何 ArkUI 对象**——因为没有
绑定 XComponent，也就没有原生 surface 可绘。

## 2. 标准接入五步

以下流程与 `examples/harmony_napi_bridge_flow.md` 一致。

### 步骤 1：Rust 侧初始化并建控件

```text
rw_init()
rw_create_window("...", ...)
rw_create_button(window, "OK", ...)   → 得到 widget_id
```

`widget_id` 必须保存到 ArkTS 侧，它是后续所有操作的句柄。

### 步骤 2：ArkTS 创建 ArkUI 节点，绑定到 widget_id

```text
rw_harmony_bind_node(node_handle, widget_id)
```

节点销毁时 `rw_harmony_unbind_node(node_handle)`；退出时
`rw_harmony_clear_node_bindings()`。

### 步骤 3：ArkTS 事件回调转发进 Rust

```text
onTap(node)         → rw_harmony_on_node_click(node)
onChange(node)      → rw_harmony_on_node_value_changed(node)
onMenuItem(node)    → rw_harmony_on_node_menu_item(node)
```

已知 `widget_id` 时可直接调 `rw_harmony_on_click(widget_id)` 等。

### 步骤 4：ArkTS 每个 tick 轮询队列

```text
menuId = rw_poll_menu_triggered()
kind   = rw_poll_widget_trigger_event(&widgetId)   // 1=clicked, 2=value-changed, 0=无
```

**这一步是鸿蒙独有的**，也是与桌面最大的区别。

### 步骤 5：分派

ArkTS 拿到 `menuId` / `widgetId` / `kind` 后，自己决定调用哪些业务逻辑。

## 3. 对自绘型控件（CodeEditor）意味着什么

这是本轮新增 `mount_custom_widget` 之后，鸿蒙需要额外做的部分：

`CodeEditor` 这类自绘型控件**没有 ArkUI 组件对应**，ArkTS 侧无法"转发点击给它",
因为点击本来就该由 Rust 自己处理。正确做法是：

1. ArkTS 建一个 `XComponent`（`type: 'surface'`）；
2. 在其 `onLoad` 里调 `rw_harmony_bind_xcomponent(component_ptr)` —— **一次调用**，
   之后 surface 尺寸、touch、mouse、key、focus 全部自动进控件树；
3. 每帧轮询 `rw_take_pending_repaint()` 得知哪个控件变脏，再调
   `rw_render_surface_frame(widget_id, w, h, ...)` 拿到 RGBA 写进画面，
   最后用 `rw_free_bytes(ptr, len)` 释放；
4. 不需要逐控件转发输入 —— 桥接的回调已接管（见 `src/platform/harmony/xcomponent.rs`）。

这七个 surface 函数（`rw_mount_surface` / `rw_resize_surface` / `rw_unmount_surface` /
`rw_invalidate_surface` / `rw_supports_surfaces` / `rw_take_pending_repaint` /
`rw_render_surface_frame`）**已经在 `src/bindings/binding_impl.rs` 实现并导出**，
发布在 `include/rw_generated.h`。它们对应 `Platform` trait 上的同名方法，底层是共享的
`mount_surface_record` + 重绘队列，所以 Android / iOS / macos_objc2 用同一套调用即可驱动。

XComponent 桥接本身在 `feature = "xcomponent"` 下启用，需要
`OHOS_SDK_NATIVE` 与一个 `*-unknown-linux-ohos` 目标。

另需 `rw_report_window_resize(window_id, w, h)`：启用了 `xcomponent` 时
`on_surface_changed` 会自动入队，但宿主自己处理尺寸变化（例如嵌套布局）时仍可直接调用。

## 4. 现在能做什么 / 不能做什么

| 能力 | 状态 |
|---|---|
| `Platform` 全契约（状态层） | ✅ 已实现 |
| 菜单树、剪贴板、拖放、IME 元数据 | ✅ 已实现 |
| 表面 ABI（`rw_mount_surface` 等 7 个 + `rw_report_window_resize`） | ✅ 已实现，发布在 `include/rw_generated.h` |
| **ArkUI XComponent 桥接**（`feature = "xcomponent"`） | ✅ 已实现，已用真实 SDK 验证编译 + 链接 |
| **输入投递进控件**（touch / mouse / key / focus） | ✅ 已接线，由桥接回调直接路由 |
| 在无 SDK 环境下开发 demo | ✅ 走 state-only 后端；`supports_surfaces()` 仍报 `true`（队列是真的），但只有启用 `xcomponent` 才可交互 |
| 真机 / 模拟器运行验证 | ⬜ 需 HarmonyOS 设备（`cargo ohos build` 只证明编译与链接） |

## 5. 相关文件

- 桥接流程：`examples/harmony_napi_bridge_flow.md`
- C 示例：`examples/harmony_napi_bridge_sample.c`
- 后端状态：`src/platform/harmony/status.md`
- C ABI 头文件：`include/rw_generated.h`（由 `tools/generate_c_header.py` 生成，
  `tools/check_abi.sh` 校验）
