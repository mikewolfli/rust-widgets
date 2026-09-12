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

当前 `src/platform/harmony/` 是 **state-only 后端**：完整的 `Platform` 契约、菜单树、
剪贴板、拖放、IME 元数据都在进程内实现，但**不创建任何 ArkUI 对象**——因为没有
OpenHarmony SDK 的 N-API/ArkUI 头文件（见 `src/platform/harmony/status.md`）。

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

## 3. 对自绘控件（CodeEditor）意味着什么

这是本轮新增 `mount_self_drawn` 之后，鸿蒙需要额外做的部分：

`CodeEditor` 这类自绘控件**没有 ArkUI 组件对应**，ArkTS 侧无法"转发点击给它",
因为点击本来就该由 Rust 自己处理。正确做法是：

1. ArkTS 建一个 `Canvas` 组件作为宿主；
2. 调用 `rw_mount_self_drawn(canvas_widget_id, x, y, w, h)`（**待实现**）；
3. ArkTS 侧拿到 `Canvas` 的 `onDraw` 回调 → 调 `rw_render_frame(pixel_buffer)` 把
   Rust 渲染的 RGBA 写进去；
4. ArkTS 的触摸/按键回调 → `rw_dispatch_self_drawn_event(id, event_code, x, y)`。

也就是说，**鸿蒙需要实现 `Platform::mount_self_drawn` 等四个方法**，把"画一帧"和
"转发输入"映射到 ArkUI `Canvas` 上。这与其他三平台是同一套 trait，只是底层调用不同。
装好 SDK 后按 `docs/plans/self_drawn_mounting.md` 的接口逐个实现即可。

## 4. 现在能做什么 / 不能做什么

| 能力 | 状态 |
|---|---|
| `Platform` 全契约（状态层） | ✅ 已实现 |
| 菜单树、剪贴板、拖放、IME 元数据 | ✅ 已实现 |
| ArkUI 原生控件创建 | ⬜ 需 SDK |
| N-API 桥接（`rw_harmony_*`） | ⬜ 需 SDK（C 头文件已备好） |
| `mount_self_drawn`（自绘控件） | ⬜ 需 SDK |
| 在无 SDK 环境下开发 demo | ✅ 走 state-only 后端，`supports_self_drawn()` 返回 `false`，demo 如实报错 |

## 5. 相关文件

- 桥接流程：`examples/harmony_napi_bridge_flow.md`
- C 示例：`examples/harmony_napi_bridge_sample.c`
- 后端状态：`src/platform/harmony/status.md`
- C ABI 总览：`docs/HARMONY_NATIVE_BRIDGE.md`
- 自绘挂载设计：`docs/plans/self_drawn_mounting.md`
