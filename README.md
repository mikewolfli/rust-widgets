# rust_widgets

Pure Rust cross-platform native GUI architecture.

## Quick Start

```bash
cargo check
cargo check --examples
cargo run --example demo_main
```

## Runtime Profiles

- `default + full`: complete desktop-oriented stack.
- `embedded`: full-weight embedded runtime path for lifecycle/render/supported-control behavior; module surface is intentionally trimmed at compile time (`xml`, `i18n`, `theme`, and `bindings` excluded).
- `mobile-api`: reserved unified extension points for mobile targets.

## Feature Toggle Examples

```bash
# Full profile (default)
cargo check

# Embedded profile
cargo check --no-default-features --features embedded

# Full profile + mobile API reservation
cargo check --features "full,mobile-api"

# Embedded profile + mobile API reservation
cargo check --no-default-features --features "embedded,mobile-api"
```

## v2 Runtime and Validation Workflow

- Lifecycle routing is now profile-explicit:
    - desktop (`not embedded`) routes `init/run/quit` directly to native platform backends
    - embedded routes lifecycle through `RenderEngine`
- Runtime route diagnostics can be enabled with:

```bash
RUST_WIDGETS_TRACE_RUNTIME=1 cargo run --example demo_main
```

- Unified validation scripts:

```bash
# default + examples + embedded profile matrix
tools/check_profiles.sh

# ABI consistency gate (version + symbols + generated header drift)
tools/check_abi.sh
```

## v3 Release Workflow

```bash
# demo smoke (default + embedded)
tools/smoke_demos.sh

# package validation without upload
cargo publish --dry-run
```

## QA Harness

```bash
# cross-profile behavior matrix
tools/check_behavior_matrix.sh

# deterministic visual snapshot regression checks
tools/check_visual_regression.sh
```

## Platform Scope

- Desktop: Windows (Win32), macOS (Cocoa), Linux (GTK), Harmony Desktop.
- Embedded: embedded Linux / embedded Harmony (full-weight runtime path under `embedded` feature).
- Mobile: Android / iOS / Harmony mobile reserved API (architecture-ready, implementation to be expanded later).

## Runtime GUI Mode Contract (v18)

- `NativeInteractive`: backend is expected to create visible native windows and run an interactive event loop.
- `PreviewOrStub`: backend may run state-model/poll loop behavior without visible native windows.
- `demo_main` prints the resolved mode at startup so "no response" cases are explicit.
- Unsupported widget creation paths return `0` (invalid object id) explicitly; platform backends should not silently downgrade to unrelated controls.

## Control Implementation Contract

- For supported backends, each control must be implemented end-to-end: native creation, state synchronization, event path, and data/model path where applicable.
- Minimal placeholder implementations are not accepted as complete control support.
- Missing backend capability must be explicit (`unsupported`/`0`) with diagnostics where available and tracked in roadmap docs.

## V19 Status Snapshot (2026-03-03)

- Completed in this cycle:
    - Windows typed trigger routing for native controls is wired through `WM_COMMAND/WM_NOTIFY` into `poll_widget_trigger_event`.
    - Windows ComboBox event-path parity is in place (user selection and programmatic index-change sync emit deterministic typed triggers).
    - ListBox full data-path API contract is implemented at platform level with Windows + stub backend coverage.
    - Preview backends (Linux non-gtk-native, Harmony, macOS objc2 preview, mobile preview) now expose explicit unsupported diagnostics for ComboBox/ListBox data APIs (no silent fallback semantics).
    - Focused ComboBox/ListBox regression tests are added and passing.
    - GPU visual parity builders and regressions are completed for covered controls: `Window`/`Panel`/`Label`/`Button`/`CheckBox`/`RadioButton`/`LineEdit`/`ComboBox`/`ListBox`/`ProgressBar`/`Slider`/`ScrollBar`/`MenuBar`/`Menu`/`ToolBar`/`StatusBar`/`TabWidget`/`StackWidget`.
    - GPU parity aggregate regression suite and end-to-end covered-control demo (`demo_wgpu_control_parity`) are added.
- Remaining explicit gaps tracked in roadmap:
    - GPU visual parity not yet covered for: `Dialog`/`MessageBox`/`FileDialog`/`ColorDialog`/`FontDialog`/`PopupWindow`/`TextEdit`/`RichEdit`/`SpinBox`/`ListView`/`TreeView`/`Table`/`Grid`/`Canvas`/`GroupBox`/`Splitter`/`DockPanel`/`MdiArea`/`ScrollArea`.
    - Embedded host-control support expansion (`MenuBar`/`Menu`/`ToolBar`/`StatusBar`) remains explicit unsupported in current embedded profile (`0`/`false` + diagnostics).

## Core Modules

- `core`, `object`, `event`, `signal`, `widget`, `layout`, `xml`, `i18n`
- `platform`, `theme`, `style`, `bindings`
- `print`, `pdf`, `chart` (feature-gated)

## Documentation Index

- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Future constrained work: [FUTURE.md](FUTURE.md)
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- ABI policy: [docs/ABI_POLICY.md](docs/ABI_POLICY.md)
- QA harness: [docs/QA_HARNESS.md](docs/QA_HARNESS.md)
- v3 handoff template: [docs/V3_HANDOFF_TEMPLATE.md](docs/V3_HANDOFF_TEMPLATE.md)
- Commenting Guidelines: [docs/COMMENTING_GUIDELINES.md](docs/COMMENTING_GUIDELINES.md)
- Demo catalog: [demos/README.md](demos/README.md)
- Help (English): [docs/HELP.en.md](docs/HELP.en.md)
- 帮助（简体中文）: [docs/HELP.zh-CN.md](docs/HELP.zh-CN.md)
- 幫助（繁體中文）: [docs/HELP.zh-TW.md](docs/HELP.zh-TW.md)
- Aide (Français): [docs/HELP.fr.md](docs/HELP.fr.md)
- Справка (Русский): [docs/HELP.ru.md](docs/HELP.ru.md)
- C ABI Quickstart: [docs/C_ABI_QUICKSTART.md](docs/C_ABI_QUICKSTART.md)
- Harmony Native Bridge: [docs/HARMONY_NATIVE_BRIDGE.md](docs/HARMONY_NATIVE_BRIDGE.md)
- 鸿蒙原生桥接（简体中文）: [docs/HARMONY_NATIVE_BRIDGE.zh-CN.md](docs/HARMONY_NATIVE_BRIDGE.zh-CN.md)
- 鴻蒙原生橋接（繁體中文）: [docs/HARMONY_NATIVE_BRIDGE.zh-TW.md](docs/HARMONY_NATIVE_BRIDGE.zh-TW.md)
- Pont natif Harmony (Français): [docs/HARMONY_NATIVE_BRIDGE.fr.md](docs/HARMONY_NATIVE_BRIDGE.fr.md)
- Нативный мост Harmony (Русский): [docs/HARMONY_NATIVE_BRIDGE.ru.md](docs/HARMONY_NATIVE_BRIDGE.ru.md)

## Demo Highlights

- Main and architecture demos: `demo_main`, `demo_layout`, `demo_xml`, `demo_i18n`
- UI control demos: window/dialog/popup, input controls, data-view controls,
  containers, menu/tool/status controls, table/grid/chart/canvas

For the complete categorized list and command set, open [demos/README.md](demos/README.md).

## C ABI Samples

- Header: [examples/rust_widgets.h](examples/rust_widgets.h)
- Typed trigger polling demo: [examples/c_abi_poll_demo.c](examples/c_abi_poll_demo.c)
- Harmony NAPI bridge sample: [examples/harmony_napi_bridge_sample.c](examples/harmony_napi_bridge_sample.c)
- Harmony NAPI bridge flow: [examples/harmony_napi_bridge_flow.md](examples/harmony_napi_bridge_flow.md)
- Python ctypes adapter: [examples/python/rust_widgets.py](examples/python/rust_widgets.py)
- Python basic demo: [examples/python/demo_basic.py](examples/python/demo_basic.py)
- C++ wrapper skeleton: [examples/cpp/rust_widgets.hpp](examples/cpp/rust_widgets.hpp)
- C++ basic demo: [examples/cpp/demo_basic.cpp](examples/cpp/demo_basic.cpp)
- Java native-method skeleton: [examples/java/RustWidgets.java](examples/java/RustWidgets.java)
- JNI bridge skeleton: [examples/java/rust_widgets_jni_bridge.c](examples/java/rust_widgets_jni_bridge.c)
- Full build/run guide: [docs/C_ABI_QUICKSTART.md](docs/C_ABI_QUICKSTART.md)
- Harmony direct callback bridge: [docs/HARMONY_NATIVE_BRIDGE.md](docs/HARMONY_NATIVE_BRIDGE.md)

Build and run (from project root):

```bash
# 1) Build dynamic library
cargo build

# 2) Compile C sample (macOS)
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_poll_demo

# 3) Run (macOS)
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

Linux:

```bash
LD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

Windows (MSYS2/MinGW style): use `set PATH=target\\debug;%PATH%` before running the executable.
