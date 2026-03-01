# rust_widgets C ABI Quickstart

## Purpose

This guide shows the minimal steps to call `rust_widgets` from C with the current stable C ABI.

## Provided files

- Header: `examples/rust_widgets.h`
- Auto-generated header: `examples/rust_widgets.generated.h`
- C sample: `examples/c_abi_poll_demo.c`

Regenerate header declarations from Rust C ABI exports:

```bash
python3 tools/generate_c_header.py
```

## Build library

From project root:

```bash
cargo build
```

This produces dynamic library artifacts under `target/debug`.

## Compile C sample (macOS)

```bash
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_poll_demo
```

Run:

```bash
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

## Linux runtime loader example

```bash
LD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

## Windows runtime note

Ensure the dynamic library directory is in `PATH` before running:

```bat
set PATH=target\debug;%PATH%
```

## Typed widget trigger polling

`rust_widgets_poll_widget_trigger_event(widget_id_out)` returns a trigger kind code and writes widget id via output pointer.

- `0`: none
- `1`: clicked
- `2`: value-changed

## Core control constructors (P2 coverage)

The C ABI now exposes additional core control constructors:

- `rust_widgets_create_label(parent, text, x, y, width, height)`
- `rust_widgets_create_radio_button(parent, text, x, y, width, height)`
- `rust_widgets_create_slider(parent, x, y, width, height)`
- `rust_widgets_create_progress_bar(parent, x, y, width, height)`
- `rust_widgets_create_combo_box(parent, x, y, width, height)`
- `rust_widgets_create_list_box(parent, x, y, width, height)`
- `rust_widgets_create_panel(parent, x, y, width, height)`

Backward-compatible APIs are still available:

- `rust_widgets_poll_menu_triggered`
- `rust_widgets_poll_widget_triggered`

## Trigger injection bridge

To feed external/native events into the same polling pipeline:

- `rust_widgets_inject_menu_trigger(menu_item_id)`
- `rust_widgets_inject_widget_trigger_event(widget_id, kind_code)`

`kind_code` mapping is the same as polling return values:

- `1`: clicked
- `2`: value-changed

Harmony direct callback aliases (for ArkUI/NAPI handlers):

- `rust_widgets_harmony_on_menu_item(menu_item_id)`
- `rust_widgets_harmony_on_click(widget_id)`
- `rust_widgets_harmony_on_value_changed(widget_id)`
- `rust_widgets_harmony_on_widget_event(widget_id, kind_code)`

Node-handle registry and aliases are documented in `docs/HARMONY_NATIVE_BRIDGE.md`.
