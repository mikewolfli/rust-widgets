# rust_widgets

Pure Rust cross-platform native GUI architecture.

## Quick Start

```bash
cargo check
cargo check --examples
cargo run --example demo_main
```

### One-click Build Script

Use the provided one-click build script to compile all targets, examples, and documentation:

```bash
# Linux/macOS
./tools/build_all.sh

# Windows
./tools/build_all.bat
```

This script will:
- Compile the library and all examples
- Build the documentation
- Run tests
- Check code quality with clippy

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

## Documentation

Comprehensive documentation is available in multiple languages:

- **English**: [docs/book/en/index.html](docs/book/en/index.html)
- **简体中文**: [docs/book/zh-cn/index.html](docs/book/zh-cn/index.html)
- **繁體中文**: [docs/book/zh-tw/index.html](docs/book/zh-tw/index.html)
- **Français**: [docs/book/fr/index.html](docs/book/fr/index.html)

### Documentation Index

- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Future constrained work: [FUTURE.md](FUTURE.md)
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- ABI policy: [docs/ABI_POLICY.md](docs/ABI_POLICY.md)
- QA harness: [docs/QA_HARNESS.md](docs/QA_HARNESS.md)
- v3 handoff template: [docs/V3_HANDOFF_TEMPLATE.md](docs/V3_HANDOFF_TEMPLATE.md)
- Commenting Guidelines: [docs/COMMENTING_GUIDELINES.md](docs/COMMENTING_GUIDELINES.md)
- Demo catalog: [demos/README.md](demos/README.md)
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

## Demos

All demos have been updated and verified to work with the latest API and platform features. Each demo showcases a core widget or subsystem:

- `demo_basic`: Basic controls and event logging
- `demo_treeview`: Tree view widget
- `demo_tabwidget`: Tab widget
- `demo_tableview`: Table view widget
- `demo_splitter`: Splitter control
- `demo_mdiarea`: MDI area (multi-document interface)
- `demo_listview`: List view widget
- `demo_dockpanel`: Dock panel widget

Run any demo with:

```bash
cargo run --example demo_basic
cargo run --example demo_treeview
cargo run --example demo_tabwidget
cargo run --example demo_tableview
cargo run --example demo_splitter
cargo run --example demo_mdiarea
cargo run --example demo_listview
cargo run --example demo_dockpanel
```

Refer to `demos/README.md` for a full categorized demo list and details.

All demos are recommended as reference implementations for custom development.

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

## Widget Implementation Statistics

The following table provides a comprehensive overview of all widget implementations in the rust-widgets library, including their implementation type (Native/Self-painted) and completion status.

### Implementation Types

- **Native**: Uses platform-specific native controls (Win32, Cocoa, GTK)
- **Self-painted**: Uses custom rendering through the render module (software/GPU rendering)

> Note: Many basic controls support both implementations - native for platform-native look, and self-painted for custom styling via the render module.

### Core Widgets

| Widget Name | Native | Self-painted | Completion | Notes |
|-------------|:------:|:------------:|:----------:|-------|
| Window | ✓ | ✓ | 100% | Full geometry, state, event handling |
| Dialog | ✓ | - | 100% | Modal/non-modal, result handling |
| MessageBox | ✓ | - | 100% | Icon support, result handling |
| FileDialog | ✓ | - | 90% | File selection, filtering |
| ColorDialog | ✓ | - | 85% | Color picker UI |
| FontDialog | ✓ | - | 85% | Font selection UI |
| PopupWindow | ✓ | - | 90% | Positioning and behavior |
| Button | ✓ | ✓ | 100% | Pressed state, signals, event handling |
| CheckBox | ✓ | ✓ | 100% | Tri-state support, signals |
| RadioButton | ✓ | ✓ | 100% | Group support, signals |
| Label | ✓ | ✓ | 100% | Text, alignment, word wrap |
| LineEdit | ✓ | ✓ | 100% | Text input, selection, password mode |
| TextEdit | - | ✓ | 80% | Multi-line text editing |
| RichEdit | - | ✓ | 85% | Rich text support, selection |
| ComboBox | ✓ | ✓ | 90% | Dropdown, selection |
| SpinBox | ✓ | - | 100% | Range, step, value handling |
| ListBox | ✓ | ✓ | 100% | Item management, selection |
| ListView | ✓ | - | 95% | Model binding, selection modes |
| TreeView | ✓ | - | 90% | Model binding, node expansion |
| ProgressBar | ✓ | ✓ | 100% | Range, value, indeterminate mode |
| Slider | ✓ | ✓ | 100% | Range, value, tick marks |
| ScrollBar | ✓ | ✓ | 100% | Range, page/step values |
| ScrollArea | - | ✓ | 90% | Viewport, content management |
| Panel | - | ✓ | 100% | Basic container |
| DockPanel | - | ✓ | 90% | Dock area management |
| GroupBox | - | ✓ | 95% | Title, checkable state |
| TabWidget | - | ✓ | 95% | Tab management, selection |
| Splitter | - | ✓ | 95% | Pane management, sizing |
| StackWidget | - | ✓ | 100% | Stack-based page management |
| MdiArea | - | ✓ | 85% | Document management |
| MenuBar | ✓ | ✓ | 100% | Menu hosting |
| Menu | ✓ | - | 95% | Action management |
| ToolBar | ✓ | - | 95% | Action hosting |
| StatusBar | - | ✓ | 100% | Message display |
| Canvas | - | ✓ | 90% | Custom drawing surface |
| Table | - | ✓ | 90% | Model binding, selection |
| Grid | - | ✓ | 100% | Grid layout container |
| Chart | - | ✓ | 75% | Basic charting support |

### Extended Widgets

| Widget Name | Native | Self-painted | Completion | Notes |
|-------------|:------:|:------------:|:----------:|-------|
| ToggleButton | - | ✓ | 95% | Checked state, auto-exclusive |
| CheckListBox | - | ✓ | 90% | Item checkboxes, selection |
| DoubleSpinBox | - | ✓ | 95% | Decimal support, prefix/suffix |
| Dial | - | ✓ | 90% | Rotary control, notches |
| Wizard | - | ✓ | 85% | Page management, navigation |
| DatePicker | - | ✓ | 85% | Date selection, calendar popup |
| TimePicker | - | ✓ | 85% | Time selection, format options |
| DateTimePicker | - | ✓ | 85% | Combined date/time selection |
| DirectoryPicker | ✓ | - | 85% | Directory selection |
| DataView | - | ✓ | 75% | Data visualization, columns |
| PropertyGrid | - | ✓ | 75% | Property editing interface |
| Toolbox | - | ✓ | 80% | Tool palette |
| StackedWidget | - | ✓ | 95% | Stacked page management |
| CollapsiblePane | - | ✓ | 80% | Expandable sections |
| DockWidget | - | ✓ | 80% | Dockable widget |
| WebView | - | ✓ | 75% | Web content display |
| ActivityIndicator | - | ✓ | 90% | Loading animation |
| Calendar | - | ✓ | 85% | Calendar display/selection |
| ColumnView | - | ✓ | 80% | Column-based list |
| UndoView | - | ✓ | 75% | Undo history display |
| CommandLink | - | ✓ | 80% | Command link button |
| LCDNumber | - | ✓ | 85% | LCD-style number display |
| FontComboBox | - | ✓ | 80% | Font selection combo |

### WebEngine Widgets

| Widget Name | Native | Self-painted | Completion | Notes |
|-------------|:------:|:------------:|:----------:|-------|
| WebEngineView | ✓ | - | 75% | Web content rendering |
| WebEnginePage | ✓ | - | 70% | Page management |
| WebEngineSettings | ✓ | - | 75% | Configuration options |
| WebEngineDownloadItem | ✓ | - | 70% | Download management |
| WebEngineCookieStore | ✓ | - | 65% | Cookie management |
| WebEngineWebChannel | ✓ | - | 60% | JS communication |
| WebEngineFindTextResult | ✓ | - | 60% | Search result handling |
| WebEngineNotification | ✓ | - | 60% | Web notifications |
| WebEngineScriptDialog | ✓ | - | 60% | JS dialog handling |
| WebEngineContextMenuRequest | ✓ | - | 60% | Context menu handling |

### Summary Statistics

- **Total Widgets**: 60+
- **Native-only**: ~15 widgets
- **Self-painted-only**: ~35 widgets
- **Dual Implementation (Native + Self-painted)**: ~10 widgets (basic controls)
- **100% Complete**: ~25 widgets
- **90%+ Complete**: ~20 widgets
- **75-90% Complete**: ~10 widgets
- **Below 75%**: ~5 widgets (WebEngine components)

### Implementation Notes

1. **Native widgets** use platform-specific controls through the `control_backend` abstraction, providing native look and feel.
2. **Self-painted widgets** use custom rendering through the `render` module, offering full customization.
3. **Dual implementation**: Basic controls (Window, Button, CheckBox, RadioButton, Label, LineEdit, ComboBox, ListBox, ProgressBar, Slider, ScrollBar, MenuBar, StatusBar) support both native platform controls and self-painted rendering.
4. The library follows a consistent pattern using `BaseWidget` for common functionality.
5. Most widgets implement the full `Widget` and `EventHandler` traits with proper signal definitions.
6. Model-View architecture is supported through traits like `ListModel`, `TreeModel`, and `TableModel`.

Windows (MSYS2/MinGW style): use `set PATH=target\\debug;%PATH%` before running the executable.
