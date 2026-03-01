# 鴻蒙原生橋接（ArkUI/NAPI）

本指南使用與英文版一致的分步結構，說明如何將鴻蒙 ArkUI/NAPI 回呼接入 `rust_widgets`。

## 目標

讓 ArkUI/NAPI 回呼與桌面後端共用同一套選單/控制項觸發輪詢管線。

## 步驟 1：建置與初始化

可選 feature：

```bash
cargo check --features harmony-native
```

建議啟動順序：

1. 呼叫 `rust_widgets_init()`。
2. 使用 `rust_widgets_create_*` 建立視窗與控制項。
3. 在原生層保存回傳的 `widget_id`。

## 步驟 2：綁定 ArkUI Node Handle

當 node 建立並映射到 `widget_id` 後，進行一次綁定：

- `rust_widgets_harmony_bind_node(node_handle, widget_id)`

當 node 銷毀時：

- `rust_widgets_harmony_unbind_node(node_handle)`

應用結束清理時：

- `rust_widgets_harmony_clear_node_bindings()`

## 步驟 3：轉發 ArkUI/NAPI 回呼

可於 ArkUI/NAPI 回呼中直接呼叫：

- `rust_widgets_harmony_on_menu_item(menu_item_id)`
- `rust_widgets_harmony_on_click(widget_id)`
- `rust_widgets_harmony_on_value_changed(widget_id)`
- `rust_widgets_harmony_on_widget_event(widget_id, kind_code)`

按 node handle 的回呼別名：

- `rust_widgets_harmony_on_node_menu_item(node_handle)`
- `rust_widgets_harmony_on_node_click(node_handle)`
- `rust_widgets_harmony_on_node_value_changed(node_handle)`
- `rust_widgets_harmony_on_node_widget_event(node_handle, kind_code)`

可選 lookup 介面：

- `rust_widgets_harmony_lookup_widget_id(node_handle)`

## 步驟 4：在應用迴圈中輪詢並分派

在每個 tick 輪詢事件佇列：

- `rust_widgets_poll_menu_triggered()`
- `rust_widgets_poll_widget_trigger_event(widget_id_out)`

現階段不需要額外橋接執行緒；回呼會直接入隊。

## 步驟 5：觸發類型映射與回退介面

觸發類型碼：

- `1`：clicked
- `2`：value-changed
- 其他：unknown

通用回退介面：

- `rust_widgets_inject_menu_trigger(menu_item_id)`
- `rust_widgets_inject_widget_trigger_event(widget_id, kind_code)`

## 相關檔案

參考資源：

- `examples/harmony_napi_bridge_sample.c`
- `examples/harmony_napi_bridge_flow.md`
