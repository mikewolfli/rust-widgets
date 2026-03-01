# 鸿蒙原生桥接（ArkUI/NAPI）

本指南采用与英文版一致的分步骤结构，说明如何将鸿蒙 ArkUI/NAPI 回调接入 `rust_widgets`。

## 目标

让 ArkUI/NAPI 回调与桌面后端共用同一套菜单/控件触发轮询管线。

## 步骤 1：构建与初始化

可选 feature：

```bash
cargo check --features harmony-native
```

推荐启动顺序：

1. 调用 `rust_widgets_init()`。
2. 使用 `rust_widgets_create_*` 创建窗口与控件。
3. 在原生层保存返回的 `widget_id`。

## 步骤 2：绑定 ArkUI Node Handle

当 node 创建并映射到 `widget_id` 后，执行一次绑定：

- `rust_widgets_harmony_bind_node(node_handle, widget_id)`

当 node 销毁时：

- `rust_widgets_harmony_unbind_node(node_handle)`

应用退出清理时：

- `rust_widgets_harmony_clear_node_bindings()`

## 步骤 3：转发 ArkUI/NAPI 回调

在 ArkUI/NAPI 回调里可直接调用：

- `rust_widgets_harmony_on_menu_item(menu_item_id)`
- `rust_widgets_harmony_on_click(widget_id)`
- `rust_widgets_harmony_on_value_changed(widget_id)`
- `rust_widgets_harmony_on_widget_event(widget_id, kind_code)`

按 node handle 的回调别名：

- `rust_widgets_harmony_on_node_menu_item(node_handle)`
- `rust_widgets_harmony_on_node_click(node_handle)`
- `rust_widgets_harmony_on_node_value_changed(node_handle)`
- `rust_widgets_harmony_on_node_widget_event(node_handle, kind_code)`

可选 lookup 接口：

- `rust_widgets_harmony_lookup_widget_id(node_handle)`

## 步骤 4：在应用循环中轮询并分发

每个 tick 轮询事件队列：

- `rust_widgets_poll_menu_triggered()`
- `rust_widgets_poll_widget_trigger_event(widget_id_out)`

当前阶段不需要额外桥接线程；回调会直接入队。

## 步骤 5：触发类型映射与回退接口

触发类型码：

- `1`：clicked
- `2`：value-changed
- 其他：unknown

通用回退接口：

- `rust_widgets_inject_menu_trigger(menu_item_id)`
- `rust_widgets_inject_widget_trigger_event(widget_id, kind_code)`

## 相关文件

参考资源：

- `examples/harmony_napi_bridge_sample.c`
- `examples/harmony_napi_bridge_flow.md`
