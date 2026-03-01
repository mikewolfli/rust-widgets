# Нативный мост Harmony (ArkUI/NAPI)

Используйте это руководство для подключения callback’ов ArkUI/NAPI Harmony к `rust_widgets` в той же пошаговой структуре, что и английская версия.

## Цель

Передавать события ArkUI/NAPI в тот же pipeline polling menu/widget, что используется desktop-бэкендами.

## Шаг 1: Сборка и инициализация

Опциональный feature:

```bash
cargo check --features harmony-native
```

Рекомендуемая последовательность запуска:

1. Вызвать `rust_widgets_init()`.
2. Создать окно и контролы через `rust_widgets_create_*`.
3. Сохранить возвращённые `widget_id` в нативном состоянии приложения.

## Шаг 2: Привязка ArkUI node handle

Когда node создан и сопоставлен с `widget_id`, выполните одноразовую привязку:

- `rust_widgets_harmony_bind_node(node_handle, widget_id)`

Когда node уничтожается:

- `rust_widgets_harmony_unbind_node(node_handle)`

При завершении приложения:

- `rust_widgets_harmony_clear_node_bindings()`

## Шаг 3: Проброс callback’ов ArkUI/NAPI

Вызывайте из callback’ов ArkUI/NAPI:

- `rust_widgets_harmony_on_menu_item(menu_item_id)`
- `rust_widgets_harmony_on_click(widget_id)`
- `rust_widgets_harmony_on_value_changed(widget_id)`
- `rust_widgets_harmony_on_widget_event(widget_id, kind_code)`

Alias-функции по node handle:

- `rust_widgets_harmony_on_node_menu_item(node_handle)`
- `rust_widgets_harmony_on_node_click(node_handle)`
- `rust_widgets_harmony_on_node_value_changed(node_handle)`
- `rust_widgets_harmony_on_node_widget_event(node_handle, kind_code)`

Опциональный API lookup:

- `rust_widgets_harmony_lookup_widget_id(node_handle)`

## Шаг 4: Polling и dispatch в цикле приложения

Потребляйте события очередей на каждом тике:

- `rust_widgets_poll_menu_triggered()`
- `rust_widgets_poll_widget_trigger_event(widget_id_out)`

На этом этапе дополнительный bridge-поток не нужен: callback’и сразу enqueue события.

## Шаг 5: Маппинг триггеров и fallback API

Коды триггеров:

- `1`: clicked
- `2`: value-changed
- прочее: unknown

Универсальные fallback API:

- `rust_widgets_inject_menu_trigger(menu_item_id)`
- `rust_widgets_inject_widget_trigger_event(widget_id, kind_code)`

## Связанные файлы

Референсные материалы:

- `examples/harmony_napi_bridge_sample.c`
- `examples/harmony_napi_bridge_flow.md`
