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
- `embedded`: minimal runtime for embedded targets (excludes `xml`, `i18n`, `theme`, and `bindings` modules at compile time).
- `mobile-api`: reserved unified extension points for mobile targets.

## Feature Toggle Examples

```bash
# Full profile (default)
cargo check

# Embedded-lite profile
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

## Platform Scope

- Desktop: Windows (Win32), macOS (Cocoa), Linux (GTK), Harmony Desktop.
- Embedded: embedded Linux / embedded Harmony (lite profile path).
- Mobile: Android / iOS / Harmony mobile reserved API (architecture-ready, implementation to be expanded later).

## Core Modules

- `core`, `object`, `event`, `signal`, `widget`, `layout`, `xml`, `i18n`
- `platform`, `theme`, `style`, `bindings`
- `print`, `pdf`, `chart` (feature-gated)

## Documentation Index

- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
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
