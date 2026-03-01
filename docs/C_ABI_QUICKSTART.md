# rust_widgets C ABI Quickstart

## Purpose

This guide shows the minimal steps to call `rust_widgets` from C with the current stable C ABI.

## Provided files

- Header: `examples/rust_widgets.h`
- Auto-generated header: `examples/rust_widgets.generated.h`
- C sample: `examples/c_abi_poll_demo.c`
- Embedded engine sample: `examples/c_abi_embedded_engine_demo.c`

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

Compile embedded engine sample (macOS):

```bash
clang -Iexamples examples/c_abi_embedded_engine_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_embedded_engine_demo
```

Run:

```bash
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_embedded_engine_demo
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

## Render AA quality controls

The stable C ABI now exposes process-wide default anti-aliasing sample controls:

- `rust_widgets_set_render_aa_samples_per_axis(samples)`
- `rust_widgets_get_render_aa_samples_per_axis()`

Behavior:

- Value is clamped to `1..=8`
- Value affects new software render surfaces created afterwards
- Current binding API version is `7`

Minimal C usage:

```c
unsigned int before = rust_widgets_get_render_aa_samples_per_axis();
unsigned int applied = rust_widgets_set_render_aa_samples_per_axis(4);
printf("AA samples: before=%u applied=%u\n", before, applied);
```

Wrapper mapping:

- C++ (`examples/cpp/rust_widgets.hpp`)
	- `RustWidgets::setRenderAASamplesPerAxis(samples)`
	- `RustWidgets::renderAASamplesPerAxis()`
- Python (`examples/python/rust_widgets.py`)
	- `set_render_aa_samples_per_axis(samples)`
	- `render_aa_samples_per_axis()`
- Java JNI skeleton (`examples/java/RustWidgets.java`)
	- `setRenderAASamplesPerAxis(samples)`
	- `getRenderAASamplesPerAxis()`

## Embedded engine controls and stats

The C ABI also exposes embedded runtime controls and diagnostics:

- `rust_widgets_set_embedded_target_fps(fps)` / `rust_widgets_get_embedded_target_fps()`
- `rust_widgets_submit_embedded_noop_task(label)`
- `rust_widgets_embedded_engine_is_initialized()`
- `rust_widgets_embedded_engine_is_running()`
- `rust_widgets_embedded_engine_frame_count()`
- `rust_widgets_embedded_engine_pending_task_count()`
- `rust_widgets_embedded_engine_window_count()`
- `rust_widgets_embedded_engine_button_count()`

Minimal C usage:

```c
unsigned int fps = rust_widgets_set_embedded_target_fps(90); /* clamped to 1..=240 */
uint64_t task_id = rust_widgets_submit_embedded_noop_task("c-abi-noop");
printf("embedded fps=%u task=%llu\n", fps, (unsigned long long)task_id);

printf("init=%d running=%d frames=%llu pending=%llu windows=%llu buttons=%llu\n",
       rust_widgets_embedded_engine_is_initialized(),
       rust_widgets_embedded_engine_is_running(),
       (unsigned long long)rust_widgets_embedded_engine_frame_count(),
       (unsigned long long)rust_widgets_embedded_engine_pending_task_count(),
       (unsigned long long)rust_widgets_embedded_engine_window_count(),
       (unsigned long long)rust_widgets_embedded_engine_button_count());
```

Wrapper mapping:

- C++ (`examples/cpp/rust_widgets.hpp`)
	- `setEmbeddedTargetFps(fps)` / `embeddedTargetFps()`
	- `submitEmbeddedNoopTask(label)`
	- `embeddedEngineInitialized()` / `embeddedEngineRunning()`
	- `embeddedEngineFrameCount()` / `embeddedEnginePendingTaskCount()`
	- `embeddedEngineWindowCount()` / `embeddedEngineButtonCount()`
- Python (`examples/python/rust_widgets.py`)
	- `set_embedded_target_fps(fps)` / `embedded_target_fps()`
	- `submit_embedded_noop_task(label)`
	- `embedded_engine_is_initialized()` / `embedded_engine_is_running()`
	- `embedded_engine_frame_count()` / `embedded_engine_pending_task_count()`
	- `embedded_engine_window_count()` / `embedded_engine_button_count()`
	- runnable demo: `examples/python/demo_embedded_engine.py`
- Java JNI skeleton (`examples/java/RustWidgets.java`)
	- `setEmbeddedTargetFps(fps)` / `getEmbeddedTargetFps()`
	- `submitEmbeddedNoopTask(label)`
	- `isEmbeddedEngineInitialized()` / `isEmbeddedEngineRunning()`
	- `embeddedEngineFrameCount()` / `embeddedEnginePendingTaskCount()`
	- `embeddedEngineWindowCount()` / `embeddedEngineButtonCount()`
	- runnable demo: `examples/java/RustWidgetsEmbeddedEngineDemo.java`

### Standardized demo output schema

The C / Python / Java embedded demos use the same `KEY=VALUE` output format and order:

1. `DEMO_PROFILE`
2. `ABI_VERSION`
3. `TARGET_FPS`
4. `APPLIED_FPS`
5. `TASK_ID`
6. `WINDOW_ID`
7. `BUTTON_ID`
8. `ENGINE_INITIALIZED`
9. `ENGINE_RUNNING`
10. `FRAME_COUNT`
11. `PENDING_TASK_COUNT`
12. `WINDOW_COUNT`
13. `BUTTON_COUNT`

This allows straightforward cross-language log diffing in CI and integration tests.
