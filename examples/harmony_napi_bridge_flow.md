# Harmony NAPI Bridge Flow

Use this guide to wire ArkUI/NAPI callbacks into `rust_widgets` with a predictable event flow.

## Step 1: Initialize and Create Widgets

Run this sequence in your app startup:

1. `rw_init()`
2. Create window and controls with `rw_create_*`
3. Store returned `widget_id` values in your native layer

## Step 2: Bind ArkUI Node Handles

When a UI node is created and mapped to a `widget_id`, register it once:

- `rw_harmony_bind_node(node_handle, widget_id)`

When a node is destroyed:

- `rw_harmony_unbind_node(node_handle)`

During app teardown:

- `rw_harmony_clear_node_bindings()`

## Step 3: Forward ArkUI/NAPI Callbacks

Forward native callbacks into `rust_widgets`:

- Click: `rw_harmony_on_node_click(node_handle)`
- Value change: `rw_harmony_on_node_value_changed(node_handle)`
- Menu item: `rw_harmony_on_node_menu_item(node_handle)`
- Generic: `rw_harmony_on_node_widget_event(node_handle, kind_code)`

If you already have a `widget_id`, use direct APIs:

- `rw_harmony_on_click(widget_id)`
- `rw_harmony_on_value_changed(widget_id)`
- `rw_harmony_on_menu_item(menu_item_id)`
- `rw_harmony_on_widget_event(widget_id, kind_code)`

## Step 4: Poll and Dispatch in the App Loop

Consume queued events on each runtime tick:

- `rw_poll_menu_triggered()`
- `rw_poll_widget_trigger_event(widget_id_out)`

Trigger kind mapping:

- `1` = clicked
- `2` = value-changed
- `0` = no event

## Step 5: Minimal Callback Pseudo-Flow

```text
onNodeReady(node, widgetId):
  rw_harmony_bind_node(node, widgetId)

onTap(node):
  rw_harmony_on_node_click(node)

onTextChange(node):
  rw_harmony_on_node_value_changed(node)

appTick():
  menuId = rw_poll_menu_triggered()
  kind = rw_poll_widget_trigger_event(&widgetId)
  dispatch(menuId, widgetId, kind)

onNodeDispose(node):
  rw_harmony_unbind_node(node)
```

## Related Files

- Header: `examples/rust_widgets.h`
- C bridge sample: `examples/harmony_napi_bridge_sample.c`
- Bridge overview: `docs/HARMONY_NATIVE_BRIDGE.md`
