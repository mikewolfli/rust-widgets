# Changelog

All notable changes to this project are documented in this file.

## 0.9.6 (2026-06-09) — BLUE11 Release

### New Widgets (28 new controls)
- **Popular Controls**: Switch, SearchBox, Chip, Badge, SkeletonLoader, FAB, Avatar, Rating, Stepper, Divider, Carousel, EmptyState, ColorWell, QRCode
- **Mobile-First**: PullToRefresh, BottomSheet, BottomNavigationBar, NavigationDrawer, AppBar, MobileDatePicker, ContextMenu (alias)
- **Platform Styles**: CupertinoSwitch, MaterialSnackbar, AdaptiveScaffold
- **Desktop Advanced**: PropertyGrid, WizardDialog, TagInput
- **Input Support**: ImePreedit
- **Layout**: MasonryLayout

### Visual Effects (R5)
- BoxShadow, Blur, ClipPath, BlendMode (16 modes), ConicGradient render commands
- Software and SVG backend support for all new commands

### Animation System (R6)
- KeyframeAnimation with multi-keyframe interpolation
- TransitionManager for CSS-style property transitions
- SpringAnimation with physical spring dynamics
- ThemeStateManager dark/light auto mode

### Accessibility (R7)
- AccessibleRole mappings for all 30+ new widgets
- AriaProperties struct for platform API bridging
- FocusTraversalStrategy (TabOrder, RowMajor, ColumnMajor)
- HighContrastMode support (BlackOnWhite, WhiteOnBlack, Custom)
- ReducedMotionPreference detection

### Event System (R8)
- Pointer Events with pressure and tilt support
- Gamepad Events (press, release, axis, connect/disconnect)
- AsyncTask with thread-local task queue
- IdleTask with frame-threshold scheduling

### Configuration & Documentation (R4)
- Cargo.toml enhanced (authors, categories, include/exclude)
- deny.toml for cargo-deny license auditing
- ARCHITECTURE.md and TUTORIAL.md documentation
- WIDGET_GALLERY.md visual reference
- .gitignore coverage improvements

### Quality & CI (R3)
- 130+ new tests across all new widgets
- CI: cargo-deny license audit job
- CI: docs-build check job
- Full feature build verification

### Architecture (R9)
- Widget re-export normalization
- containers.rs confirmed at 849 lines (no split needed)
- missing_docs and unsafe_code lint warnings added

## [Unreleased]

### Added

- Signal-first event model migration notes for `v9`:
  - generic `Signal<T>` core with typed payload dispatch
  - `connect_once` one-shot slot semantics
  - scoped auto-disconnect via owner lifetime drop
- Expanded typed widget trigger kinds in platform/C ABI routing:
  - `3`: `selection-changed`
  - `4`: `closed`
- Geometry/style baseline primitives for `v10`:
  - `Point::new/origin`, `Size::new/is_empty`, `Rect::new/from_position_size/position/size/decompose/is_valid`
  - `Color::parse_hex` (`#RGB/#RGBA/#RRGGBB/#RRGGBBAA`), canonical hex serializers, `u32` pack/unpack
  - `Font` weight baseline (`100..=900`), shared defaults, and normalization helpers
  - `Padding`/`Margin` per-side types with `all/symmetric/normalized` constructors
  - axis-specific alignment enums and mapping helpers (`HorizontalAlignment`/`VerticalAlignment`)
- Basic widget full-class baseline for `v12`:
  - `Label`: deterministic text/alignment/image/word-wrap state with change signals
  - `LineEdit`: return-pressed signal, password-mode masking, selection/copy/cut/paste contract
  - `CheckBox`/`RadioButton`: tri-state + group-selection routing with explicit state/selected signals
  - `ComboBox`/`Slider`/`ProgressBar`: deterministic index/value range-clamped change signaling
- CI signal-first guard:
  - `tools/check_event_model_signal_first.sh` blocks wxWidgets-style event table patterns
  - validation gate wired in `.github/workflows/ci.yml`
- Layout system baseline for `v13`:
  - explicit `HBoxLayout` / `VBoxLayout` named layout types with `Layout` parity
  - deterministic `BoxLayout` major-axis allocation (remainder-aware, constraint-safe)
  - spacing/margin/item-count tuning APIs for directional layout control
  - focused layout regressions for box/grid/stack placement and auto geometry conversion
- Action system baseline for `v14`:
  - shared action routing parity across menu/button/toolbar hosts plus shortcut triggers
  - deterministic trigger contract with enabled gating and trigger result semantics
  - checkable action semantics (`checkable`, `checked`, toggle-on-trigger)
  - action state signals for signal-first routes (`triggered`, `toggled`, `enabled_changed`)
  - focused regressions for action binding/trigger/toggle behavior
- Intermediate widgets slice for `v15`:
  - `ScrollBar` full-state contract (`min/max/value/page_step/single_step`) with deterministic `value_changed`
  - `ScrollArea` baseline contract (`content_size`/`viewport_size`/`scroll_offset`) with signal-first change events
  - focused widget regressions for bounded scroll behavior and offset normalization
  - `GroupBox` title/checkable/checked deterministic contract with state change signals
  - `TabWidget` deterministic selected-index routing (`add/remove/select`) with `current_index_changed`
  - `Splitter` pane-ratio/size distribution contract with orientation/layout change signals
  - `MenuBar`/`Menu`/`ToolBar`/`StatusBar` intermediate host contracts for action routing and status/message state signals
  - dialog-family baseline contracts: `Dialog`, `MessageBox`, `FileDialog`, `ColorDialog`, `FontDialog`
    with deterministic result/state signals for accept/reject/select flows
- Model/view architecture baseline for `v16`:
  - observable model signal surface via `data_changed_signal` on `ListModel`/`TreeModel`/`TableModel`
  - in-memory observable model contracts: `VecListModel`, `VecTreeModel`, `VecTableModel`
  - auto-refresh wiring in `TreeView::set_model` and `TableWidget::set_model` (model changes trigger redraw/layout requests)
  - focused regressions for model signal propagation and tree/table view refresh behavior
- Advanced widgets kickoff for `v17`:
  - `ListView` baseline contract with `ListModel` projection, deterministic selection, and model-driven auto-refresh wiring
  - dedicated `TableView` contract wrapper with `TableWidget` parity for model/delegate/selection APIs
  - expanded tree/table/list advanced view state contracts with focused row/node state and projection-safe normalization on model rebind
  - `RichEdit` baseline contract with text/selection/read-only state and deterministic edit/cursor signals
  - container baselines: `DockPanel` pane-placement contract and `MdiArea` document/active-document state contract
  - focused regressions for `ListView`/`TableView` baseline behavior and full widget-suite compatibility
- Runtime GUI mode contract for `v18`:
  - `RuntimeGuiMode::{NativeInteractive, PreviewOrStub}` and active-backend resolvers
  - startup mode reporting in `demo_main` for explicit backend behavior visibility
  - v18 startup smoke matrix and evidence template in `docs/QA_HARNESS.md`

### Changed

- Native signal bridge routing now normalizes covered widget interactions through typed trigger routes
  (`clicked`, `value-changed`, `selection-changed`, `closed`) instead of per-kind ad-hoc paths.
- Widget interaction baseline now emits explicit selection/closed signals for covered controls
  (window, combo box, tree view, table widget).
- Representative widget/layout entry points now accept primitive geometry/style workflows:
  - widget trait helpers: `position/size`, `set_position/set_size`, `padding/margin`, `set_padding/set_margin`
  - layout trait helper: `update_from_position_size(position, size, ...)`
- XML style parsing now reuses shared color parser (`Color::parse_hex`) and supports short/alpha hex forms.
- Advanced widget runtime kinds are now disambiguated for `RichEdit`, `ListView`, `DockPanel`, and `MdiArea`
  (no longer aliased to baseline kinds like `TextEdit`/`ListBox`/`Panel`/`StackWidget`).
- Historical roadmap audit coverage for `v1~v9` is now explicitly recorded and re-validated against code/tests/scripts.
- Embedded profile behavior-matrix validation now stays green by gating `serde_json`-dependent core deserialization test behind `desktop-runtime` feature.
- Validation sweep for this audit slice is green: `check_profiles`, `check_event_model_signal_first`, `cargo test --lib`, `check_behavior_matrix`, `check_visual_regression`, and `check_abi`.
- Windows runtime lifecycle path now uses active message pumping with stable loop-alive behavior for `demo_main`.
- Non-native preview backends now emit explicit runtime diagnostics:
  - Linux (non-`gtk-native`)
  - Harmony desktop preview path
  - Android mobile preview backend
  - macOS objc2 preview backend
- Cross-platform control creation routing now uses explicit backend `create_*` implementations (Linux/Harmony/macOS-objc2/mobile + Windows overrides) with no implicit demo/C-ABI button fallback shims for slider/progress/combo paths.
- Platform default `create_*` methods now fail explicitly (`0`) when unsupported; Windows backend create failures also return `0` with runtime diagnostics instead of silent button downgrade.

### Migration Notes

- Existing `Font::new(family, size, bold, italic)` remains supported; it now derives normalized `weight`
  (`400` regular, `700` bold). Prefer `Font::with_weight(...)` for explicit typography contracts.
- Existing uniform spacing behavior is preserved (`Padding::all`, `Margin::all`), while per-side values are
  now available for forward-compatible style contracts.
- Geometry callers can incrementally adopt primitive helpers without breaking existing `Rect` call sites.

## [0.10.0] - 2026-06-10

### Added

- **WidgetKind cleanup**: Removed orphan/duplicate variants from the widget kind enum
  for a cleaner, more maintainable categorisation.
- **macOS + iOS native FFI wiring**: Full native platform FFI bridges for macOS
  (AppKit/NSAccessibility) and iOS (UIKit/UIAccessibility), enabling native control
  creation and accessibility event routing.
- **i18n system (complete)**: Fully functional internationalisation with locale
  loading, plural rules, and context-based message translation.
- **StyleSheet engine**: CSS-like declarative style system with selector matching,
  cascading, and computed property resolution.
- **New layouts**:
  - `FlexLayout` — flex-box style responsive layout
  - `WrapLayout` — flow-based wrapping layout
  - `KeyboardAwareLayout` — auto-offset layout for soft keyboard
  - `ConstraintLayout` — anchor/constraint-based positioning
  - `Center` — centred container layout
  - `AspectRatio` — fixed-ratio sizing wrapper
- **New infrastructure**:
  - `App Lifecycle` — structured application lifecycle management
  - `Undo/Redo` — full undo/redo framework with command stack and composite commands
  - `Data Binding` — reactive model-to-view automatic synchronisation
  - `Print framework` — cross-platform print and preview API
  - `PDF export` — PDF document generation with form, image, and security support
- **New widgets** (65+):
  - Tooltip, SegmentedButton, NavigationStack, ProgressCircle, Icon
  - Popover, MenuButton, DropdownMenu
  - MaskedEdit, AutoCompleteEdit, MultiSelectComboBox
  - RangeSlider, FloatingLabel
  - TabView, SearchBar
  - Cupertino* controls: CupertinoNavigationBar, CupertinoSegmentedControl,
    CupertinoDatePicker
  - SwipeToDismiss, Pager/PageView, RefreshControl
  - ModalBottomSheet
  - FindReplaceDialog, PropertiesPanel
  - Charts: LineChart, BarChart, PieChart, Sparkline
  - EditableComboBox, DateRangePicker
  - AnimatedImage — frame-based animation widget with loop control
  - HeroAnimation — shared-element transition animation widget
  - BezierCurveEditor — interactive cubic Bézier curve editor
  - ColorHistory — colour swatch history with selection/hover signals
  - FontPreview — live font preview with configurable sample text
  - ShortcutEditor — keyboard shortcut configuration editor
  - InplaceEditor — click-to-edit text field with accept/cancel signals
- **Platform backends**:
  - `android` — full Android Platform trait implementation (state-driven)
  - `wasm` — WASM platform module for web browser execution
- **IME implementations**:
  - `macOS` — NSTextInputContext-based IME bridge
  - `Windows` — TSF-based IME bridge
  - `Linux` — IBus-based IME bridge
- **A11y enhancements**:
  - `A11yRole` enum with 27 semantic roles (Button..Unknown)
  - `A11yState` / `A11yNode` / `A11yTree` for screen reader tree management
  - `A11yProvider` trait for cross-platform screen reader integration
  - NSAccessibility protocol helpers (macOS) and UIA control type helpers (Windows)
  - `DefaultA11yProvider` in-memory implementation with focus traversal
- **Text shaping & rich text**:
  - `TextShaper` / `SimpleTextShaper` — text measurement and glyph run shaping
  - `RichText` / `TextSpan` / `TextStyle` — multi-span styled text rendering
  - `TextOverflow` / `TextClamp` — ellipsis, clip, and multi-line clamp handling
  - `GraphemeCluster` / `GraphemeProcessor` — Unicode emoji, combining marks, and ZWJ grapheme support
- **Platform backend refactoring**:
  - `macOS` (objc2) — `platform_impl.rs` split into `widget_creation.rs`, `menu_impl.rs`,
    `widget_state.rs`, `clipboard_dnd.rs`, `dialog_creation.rs`, `native.rs` for maintainability
  - `Linux` — `platform_impl.rs` split into `widget_creation.rs`, `menu_impl.rs`,
    `widget_state.rs` for maintainability
- **Render pipeline refactoring**:
  - `render/pipeline/` split — `containers.rs` extracted from monolithic pipeline
    into dedicated sub-module per widget family for maintainability
- **Testing**: 3400+ test cases across all subsystems

### Changed

- Missing docs lint changed from `allow` to `warn` to surface documentation gaps.

## [0.5.19] - 2026-03-03

### Added

- v19 GPU visual parity coverage builders and regressions for covered controls:
  - base controls: `Window`/`Panel`/`Label`/`Button`/`CheckBox`/`RadioButton`/`LineEdit`
  - data/range controls: `ComboBox`/`ListBox`/`ProgressBar`/`Slider`/`ScrollBar`
  - host/navigation controls: `MenuBar`/`Menu`/`ToolBar`/`StatusBar`/`TabWidget`/`StackWidget`
- GPU parity aggregate regression tests:
  - `render::tests::gpu_parity_covered_controls_emit_non_empty_command_suite`
  - `render::tests::gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend`
- New covered-control GPU parity demo:
  - `demos/demo_wgpu_control_parity.rs` (`cargo run --features gpu-wgpu --example demo_wgpu_control_parity`)
- QA/profile gate integration for P3g parity checks:
  - `tools/check_behavior_matrix.sh`
  - `tools/check_profiles.sh`

### Changed

- Embedded v19 closure stream is now fully documented as complete (`P4a`..`P4d`) with explicit residual embedded host-control unsupported boundaries.
- Version updated from `0.5.0` to `0.5.19` in `Cargo.toml`.

### Notes

- GPU implementation mode remains the light-weight route by design for this cycle:
  CPU command rasterization + `wgpu` upload/readback through the unified auto backend selection path.
- Controls without explicit GPU parity builders remain documented as uncovered in roadmap/docs for follow-up expansion.

## [0.5.0] - 2026-03-02

### Added

- Basic widgets milestone is complete and stabilized (Button/Label/LineEdit/CheckBox/RadioButton/ComboBox/SpinBox/Slider/ProgressBar).

### Changed

- Project crate version is now `0.5.0`.

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
