# Harmony Native Bridge (ArkUI/NAPI)

Use this guide to connect Harmony ArkUI/NAPI callbacks to `rust_widgets` with the same step model as the flow document.

## Goal

Allow ArkUI/NAPI callbacks to feed menu and widget trigger events directly into the same polling pipeline used by desktop backends.

## Step 1: Build and Initialize

Optional feature flag:

```bash
cargo check --features harmony-native
```

Runtime startup sequence:

1. Call `rust_widgets_init()`.
2. Create window and controls with `rust_widgets_create_*` APIs.
3. Keep returned `widget_id` values in native runtime state.

## Step 2: Bind ArkUI Node Handles

When a node is created and mapped to a `widget_id`, bind once:

- `rust_widgets_harmony_bind_node(node_handle, widget_id)`

When node is disposed:

- `rust_widgets_harmony_unbind_node(node_handle)`

At teardown:

- `rust_widgets_harmony_clear_node_bindings()`

## Step 3: Forward ArkUI/NAPI Callbacks

Use these functions from ArkUI/NAPI callback handlers:

- `rust_widgets_harmony_on_menu_item(menu_item_id)`
- `rust_widgets_harmony_on_click(widget_id)`
- `rust_widgets_harmony_on_value_changed(widget_id)`
- `rust_widgets_harmony_on_widget_event(widget_id, kind_code)`

Node-handle callback aliases:

- `rust_widgets_harmony_on_node_menu_item(node_handle)`
- `rust_widgets_harmony_on_node_click(node_handle)`
- `rust_widgets_harmony_on_node_value_changed(node_handle)`
- `rust_widgets_harmony_on_node_widget_event(node_handle, kind_code)`

Helper lookup API (optional):

- `rust_widgets_harmony_lookup_widget_id(node_handle)`

## Step 4: Poll and Dispatch in App Loop

Consume queued events on each app tick:

- `rust_widgets_poll_menu_triggered()`
- `rust_widgets_poll_widget_trigger_event(widget_id_out)`

No extra bridge thread is required at this stage; callbacks enqueue events directly.

## Step 5: Trigger Mapping and Fallback APIs

Trigger kind mapping:

- `1`: clicked
- `2`: value-changed
- `3`: selection-changed
- `4`: closed
- others: unknown

Generic fallback APIs:

- `rust_widgets_inject_menu_trigger(menu_item_id)`
- `rust_widgets_inject_widget_trigger_event(widget_id, kind_code)`

## Related Files

Reference assets:

- `examples/harmony_napi_bridge_sample.c`
- `examples/harmony_napi_bridge_flow.md`
