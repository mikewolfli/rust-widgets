# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Added

- Signal-first event model migration notes for `v9`:
  - generic `Signal<T>` core with typed payload dispatch
  - `connect_once` one-shot slot semantics
  - scoped auto-disconnect via owner lifetime drop
- Expanded typed widget trigger kinds in platform/C ABI routing:
  - `3`: `selection-changed`
  - `4`: `closed`

### Changed

- Native signal bridge routing now normalizes covered widget interactions through typed trigger routes
  (`clicked`, `value-changed`, `selection-changed`, `closed`) instead of per-kind ad-hoc paths.
- Widget interaction baseline now emits explicit selection/closed signals for covered controls
  (window, combo box, tree view, table widget).

## [0.1.0] - 2026-03-01

### Added

- CI validation gate job in `.github/workflows/ci.yml`:
  - profile matrix gate via `tools/check_profiles.sh`
  - ABI gate via `tools/check_abi.sh`
- New C ABI profile-aware capability contract query:
  - `rust_widgets_platform_capability_contract(profile_code)`
- Demo smoke script:
  - `tools/smoke_demos.sh` for `default` (`demo_main`) and `embedded` (`demo_button`) checks.
- First Python binding adapter path:
  - `examples/python/rust_widgets.py` (ctypes adapter)
  - `examples/python/demo_basic.py` (basic usage demo)
- Feature-completeness CI artifact pipeline:
  - `feature-completeness-matrix` job in `.github/workflows/ci.yml`
  - artifact upload for `target/qa/feature_completeness_matrix.md`
- Allowlist-aware matrix auditing inputs:
  - `tools/feature_completeness_allowlist.toml`

### Implemented

- PDF form serialization baseline in `src/pdf/mod.rs`:
  - `PdfPage` form APIs now emit `/AcroForm` and page `/Annots` widget objects
  - text/checkbox/button widgets are serialized into the object graph
- PDF security persistence diagnostics path in `src/pdf/mod.rs`:
  - `PdfSecurity` settings are persisted via explicit unsupported-encryption diagnostic entries
  - reader path restores those diagnostics on round-trip load
- PDF image deterministic encoding route in `src/pdf/mod.rs`:
  - image normalization routes (`exact-rgb`, `exact-rgba-drop-alpha`, `exact-gray-expand`, `raw-truncate-pad`)
  - removed synthetic payload-tiling behavior and added stream route metadata comments
- PDF regression expansion:
  - focused and combined tests now cover forms + security + image pipelines and reader round-trip behavior

### Changed

- Release preparation baseline for `0.1.0` (metadata hardening + publish dry-run workflow).
- Runtime diagnostics output is now structured as:
  - `[rust_widgets.runtime] stage=<...> profile=<...> backend=<...> route=<...>`
- Feature-completeness report format now includes:
  - raw/effective/suppressed signal counts
  - allowlist suppression reasons by file/category

## [0.0.2] - 2026-03-01

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
