# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Added

- GitHub project governance and collaboration files:
  - `LICENSE` (MIT), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `SUPPORT.md`
  - Issue templates and PR template under `.github/`
  - CI workflow and Dependabot configuration
- Desktop shell capability parity improvements:
  - richer menu/menu item lifecycle support in desktop backends
  - typed widget trigger event model (clicked/value-changed)
- C ABI enhancements:
  - typed trigger polling API: `rust_widgets_poll_widget_trigger_event`
  - expanded core control constructors (`label`, `radio_button`, `slider`, `progress_bar`, `combo_box`, `list_box`, `panel`)
  - trigger injection APIs for host/native event sources
  - Harmony callback entrypoints for direct ArkUI/NAPI integration
  - node-handle registry APIs (`node_handle ↔ widget_id`) for Harmony integration
- New bridge and onboarding assets:
  - `docs/C_ABI_QUICKSTART.md`
  - `docs/HARMONY_NATIVE_BRIDGE.md` and localized variants
  - `examples/rust_widgets.h`
  - `examples/c_abi_poll_demo.c`
  - `examples/harmony_napi_bridge_sample.c`
- New runtime demo:
  - `demos/demo_native_events.rs`
- v2 validation tooling:
  - `tools/check_profiles.sh` for default/examples/embedded matrix checks
  - `tools/check_abi.sh` for ABI header drift + symbol and version gate checks

### Implemented

- Real print backend path in `src/print/mod.rs`:
  - system spool submission via `lpr`/`lp` on macOS/Linux
  - print-verb submission path on Windows
  - `Printer::print_with_result` for explicit backend error reporting
- Real PDF backend path in `src/pdf/mod.rs`:
  - valid minimal PDF (`%PDF-1.4`) serialization with catalog/pages/xref/trailer
  - page drawing commands mapped to PDF operators (`BT/Tj`, `m/l/S`, `re`, `f`)
  - reader supports `/Count` page parsing for round-trip loading baseline
- Real chart backend path in `src/chart/mod.rs`:
  - SVG rendering context (`SvgChartContext`) for concrete vector output
  - file export helper `render_chart_to_svg_file`
  - demo integration that exports `target/debug/demo_chart.svg`
- Embedded deep trimming path:
  - embedded builds exclude `xml`, `i18n`, `theme`, and `bindings` modules
  - `init()` uses a no-op i18n initializer under `embedded` profile
  - verified by `cargo check --no-default-features --features embedded`
- Dual-engine architecture baseline:
  - new `render_engine` module with `RenderEngine` trait
  - `NativeRenderEngine` and `EmbeddedRenderEngine` implementations
  - lifecycle APIs (`init`/`run`/`quit`) routed through default engine selection
- Object reflection/property enhancement:
  - dynamic `PropertyValue` model in `src/object/mod.rs`
  - reflective property APIs (`set_property`, `property`, `remove_property`, `property_keys`)
- Platform capability expansion:
  - `PlatformCapabilities` model and DPI scale query in `src/platform/mod.rs`
  - C ABI exposure via `rust_widgets_platform_capabilities` and `rust_widgets_platform_dpi_scale_factor`
  - C header sync in `examples/rust_widgets.h`
- ABI engineering improvements:
  - automated header generator `tools/generate_c_header.py`
  - generated artifact `examples/rust_widgets.generated.h`
  - C ABI versioning advanced to `5`

### Changed

- Linux backend now supports optional native GTK signal path under feature `gtk-native`.
- Documentation index expanded in `README.md` and localized help docs for C ABI and Harmony bridge coverage.
- C ABI version increased to `5` to reflect newly added public ABI functions.
- Lifecycle routing boundaries are profile-explicit:
  - desktop profile calls native platform lifecycle directly
  - embedded profile keeps lifecycle routed through `RenderEngine`
- Desktop-only dependencies (`serde_json`, `lazy_static`, `roxmltree`) are now optional via `desktop-runtime` feature to reduce embedded footprint.

### Notes

- Default builds remain stable and pass `cargo check` and `cargo check --examples`.
- Optional feature checks pass for `gtk-native` and `harmony-native`.
