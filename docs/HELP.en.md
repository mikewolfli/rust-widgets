# rust_widgets Help (English)

## Related docs

- Architecture: [ARCHITECTURE.md](ARCHITECTURE.md)
- Demo catalog: [../demos/README.md](../demos/README.md)
- Chinese (Simplified): [HELP.zh-CN.md](HELP.zh-CN.md)
- Chinese (Traditional): [HELP.zh-TW.md](HELP.zh-TW.md)
- French: [HELP.fr.md](HELP.fr.md)
- Russian: [HELP.ru.md](HELP.ru.md)
- C ABI Quickstart: [C_ABI_QUICKSTART.md](C_ABI_QUICKSTART.md)
- Harmony Native Bridge: [HARMONY_NATIVE_BRIDGE.md](HARMONY_NATIVE_BRIDGE.md)

## Summary

- Pure Rust cross-platform native GUI architecture.
- Desktop targets: Windows, macOS, Linux, Harmony desktop.
- Embedded-lite profile for minimal footprint.
- Reserved unified API for mobile targets (Android / iOS / Harmony mobile).
- Includes event queue, signal-slot, theme/style, layout, XML, i18n, print, PDF, and chart modules.

## Build profiles

- Full profile: `default` + `full` features.
- Embedded profile: `embedded` feature with non-core modules disabled.
- Mobile reservation: `mobile-api` feature for unified mobile extension points.

## Commands

```bash
cargo check
cargo check --examples
cargo run --example demo_main
```

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

## v2 Hardening Checks

- Lifecycle routing is explicit by profile:
    - desktop builds route directly to native platform backends
    - embedded builds route through `RenderEngine`
- To trace active runtime route during `init/run/quit`:

```bash
RUST_WIDGETS_TRACE_RUNTIME=1 cargo run --example demo_main
```

- Validation gates:

```bash
tools/check_profiles.sh
tools/check_abi.sh
```

## v3 Release Workflow

```bash
# demo smoke (default + embedded)
tools/smoke_demos.sh

# package validation without upload
cargo publish --dry-run
```

## Demos

- Full categorized demo list: see `demos/README.md`.
- Main and architecture demos: `demo_main`, `demo_layout`, `demo_xml`, `demo_i18n`.
- Control demos include window/dialog/popup, basic input controls, data-view controls,
  containers, menu/tool/status controls, plus table/grid/chart/canvas controls.

## Binding note

C ABI is implemented in `src/bindings/mod.rs` with reserved API entry points for Python/C++/Java bindings.
It also exposes polling APIs for native triggers: `rust_widgets_poll_menu_triggered` and `rust_widgets_poll_widget_triggered`.
For typed widget triggers use `rust_widgets_poll_widget_trigger_event(widget_id_out)`, which returns kind code (`0` none, `1` clicked, `2` value-changed).
Render quality is configurable via C ABI with `rust_widgets_set_render_aa_samples_per_axis` / `rust_widgets_get_render_aa_samples_per_axis` (clamped `1..=8`).
Ready-to-use C sample assets are available at `examples/rust_widgets.h` and `examples/c_abi_poll_demo.c`.
For complete build/run commands, see `docs/C_ABI_QUICKSTART.md`.
For direct ArkUI/NAPI callback wiring on Harmony, see `docs/HARMONY_NATIVE_BRIDGE.md`.

Quick build/run (project root):

```bash
# Build library
cargo build

# Compile C sample on macOS
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_poll_demo

# Run on macOS
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

Linux runtime loader example:

```bash
LD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```
