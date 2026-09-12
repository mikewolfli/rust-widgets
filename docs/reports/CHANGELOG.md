# Changelog

All notable changes to this project are documented in this file.

## 1.1.2 (2026-09-12) — Platform Correctness & Unsafe-Surface Audit Release

An audit-driven release. Work began as "complete the Apple-related items in `blue14.md`" and
continued through seven rotated audit directions (FFI soundness, error-path leaks, concurrency/panic
safety, memory/long-run, API-contract consistency, doc/code consistency, and unsafe-impl necessity).
Every fix below was reproduced first, then closed with a regression test and **negative verification**
(revert the fix → the test must fail).

### Fixed — Apple (host-visible only on macOS, so latent for the project's whole history)
- **`MacOSPlatform` (cocoa-legacy) made unguarded AppKit calls from any thread.** Creating a window
  or touching any of ~30 other call sites from a worker thread made AppKit raise a foreign
  Objective-C exception, which Rust cannot catch, so the **entire test process died with SIGABRT**
  (`cargo test --lib --features desktop` aborted in `c_abi_widget_lifecycle_roundtrip`).
  Added an `is_main_thread()` guard plus a state-only fallback handle to every AppKit entry point,
  and made `add_to_parent_window`/`sync_list_box_native` skip nil receivers (the `cocoa` crate
  dereferences null and aborts).
- **`macos_objc2` native FFI was gated on the alias feature `objc2-macos` instead of `macos`.**
  Because feature aliases are one-way, `--features macos` left **43 native call sites silently
  compiled out**, degrading the backend to state-only with no error. Now gated on the canonical
  feature.
- **`set_native_text` declared `performSelector:withObject:` as returning `()`**, but it returns
  `id`; the mismatch aborted at runtime. Return type corrected.
- **`ime_macos` declared `ImeCtx` as four identically-named *local* structs.** `Any::downcast_ref`
  matches on `TypeId`, not memory layout, so every downcast silently returned `None` and **three
  native IME paths were no-ops**. Types hoisted to module level.

### Fixed — FFI soundness
- **Removed three unnecessary `unsafe impl Sync`, each proven redundant by a delete-and-compile
  test**: `EventHandlerContext` (`json/events.rs`), `LinuxPlatform` (`linux/types.rs`), and
  `TsfThreadMgr` (`ime_windows.rs`). Each removal compiled clean across desktop, `--all-features`,
  every profile, and `x86_64-pc-windows-gnu`. `EventHandlerContext` was the sharpest case: it exposes
  both `user_data<T>() -> &T` and `user_data_mut<T>() -> &mut T` from the same unowned pointer, so a
  `Sync` impl would have legalized a genuine data race. `AndroidPlatform`'s `Sync` was **kept** —
  removing it produces 55 `E0277` errors, i.e. real code depends on it. The crate now has exactly
  one `unsafe impl Sync`.

### Fixed — resource leaks on error paths
- **`ffmpeg_encoder` leaked its temp file on any encode failure.** `format::output_as` creates the
  file before many `?` early-returns, so failures after that point left the file behind. Now guarded
  by RAII (`TempFileGuard`).
- **`ffmpeg_decoder` leaked on write failure** — same class, fixed the same way.

### Fixed — concurrency, panic safety, timing
- **i18n hot-reload trusted `mtime` alone**, so on coarse-granularity filesystems a change could be
  missed entirely. Replaced with a content fingerprint.
- **`undo/stack` test fixture used `static mut`**, a real data race between concurrent tests. Now atomic.
- **Data-binding's `syncing` re-entrancy flag stayed stuck forever if a callback panicked**, silently
  disabling all future two-way sync. Now an RAII guard, so a panic unwinds it.

### Fixed — API contracts and lifecycle
- **`StubPlatform` contradicted itself**: 21 `create_*` methods ignored their `parent` argument while
  19 sibling methods validated it. All 21 now validate, so tests built on the stub are a valid
  contract baseline. `create_menu_bar` requires a `Window` parent; `create_menu` requires
  `MenuBar | Menu`.
- **`macos`/cocoa's `create_spin_box`/`create_list_view`/`create_scroll_area` ignored `parent`**
  while the `create_group_box` directly beneath them validated it. Fixed.
- **Added `Platform::destroy_widget()` and `widget_count()`** (plus `rw_destroy_widget` in the C ABI,
  all 11 backends, and the `ControlBackend` trait). Previously there was **no way to destroy a
  widget at all**, so dynamic UIs accumulated registry entries without bound. Cleanup is real, not
  cosmetic: objc2 releases derived submenu ids and cascades menu children; cocoa now calls the
  previously-never-called `a11y_bridge.unregister_handle`; iOS releases its `ButtonTarget`.
- **GTK clipboard panicked off the main thread.** `gtk_clipboard()` called
  `gdk::Display::default()` directly, and GDK panics ("may only be used from the main thread") on a
  non-GTK thread, aborting the process; the function's own doc comment claimed it returned `None`
  there. Now guarded by `gtk::is_initialized_main_thread()` with the documented mirror fallback.
- **`libvorbis` was hard-coded as an external encoder.** This had been misattributed as an
  "environment failure"; `brew deps ffmpeg` / `otool -L` proved Homebrew's ffmpeg ships no vorbis.
  Now falls back to ffmpeg's native Vorbis encoder.

### Fixed — test coverage that was not actually running
- **50 logic-only tests were excluded from the build** by module-level `#[cfg(target_os = ...)]`:
  `ime_macos` (19), `macos_objc2` (17), `android` (8), `ios` (6). All now execute on the host.
  Un-gating them exposed a previously-uncompilable `assert_eq!` in the `android` tests
  (`AndroidHandleKind` lacked `Debug`).
- Earlier rounds likewise recovered `ime_windows` (15) and `windows_notify` (11) tests that were
  trapped behind `target_os` gates despite being pure state-machine logic.
- Added `platform::contract_tests` (11 tests, incl. a 17-method parent-rejection matrix) and
  `platform::teardown_tests` (3 tests asserting the library's own registry sizes).

### Added
- `tools/check_widget_kind_count.sh` — mechanically parses `src/widget/kind.rs` and fails if any
  document's stated widget-kind count has drifted. Wired into the CI `validation-gates` job.
- `tools/check_apple_native.sh`, `tools/check_apple_thread_safety.sh`, `tools/build_ios_testapp.sh`,
  `tools/run_ios_testapp.sh` — a real iOS `.app` (staticlib + ObjC host, no Xcode project) that
  boots an iOS simulator, installs, launches, and asserts `RESULT: PASS`; plus AppKit probes for
  both macOS backends.
- `examples/apple_appkit_probe_objc2.rs` — a separate probe for the objc2 backend (the two macOS
  backends use different Objective-C crates).
- An `apple-native` CI job covering both macOS backends and the iOS simulator run.

### Changed
- Crate version bumped `1.1.1` → `1.1.2`. **No ABI change**: `rw_bindings_api_version` remains `8`.
  `rw_destroy_widget` is a new symbol in the generated header (106 declarations, up from 105).
- Version references aligned to `1.1.2` in `Cargo.toml`, the `CoreConfig` runtime version contract,
  Node.js/Python package metadata, the demo banner, and the en/zh-CN/zh-TW cookbooks.
- **Docs corrected where they contradicted the code**: `codemap.md` claimed 166 `WidgetKind`
  variants (actual 167), and both READMEs advertised "80+ widgets" (actual 167 kinds).

### Verification
- `cargo test --lib`: **3854 passed**, 0 failed (desktop) / **3946** (full) / **1869**
  (`--all-features`); mobile 3662, tablet 3653, wasm 2263, harmony 2268 — all 0 failed.
- `clippy --all-features --all-targets -- -D warnings`: **0 warnings**; `cargo fmt --check` clean.
- `cargo check --all-targets` across desktop/full/mini/embedded/mobile/tablet/wasm/harmony: 0 errors.
- Eight CI gates pass: profiles, ABI, widget-kind count, platform capability matrix, capability
  truthfulness, control route matrix, platform impl matrix, Apple thread safety.
- Both macOS AppKit probes report `RESULT: PASS` (12 named checks each) against live AppKit objects.

### Notes on what was deliberately *not* changed
- **Dialog parent validation stays as-is.** `create_message_box` / `create_file_dialog` /
  `create_color_dialog` / `create_font_dialog` ignore the parent on 7 of 11 backends and validate on
  4. Ignoring is the majority convention and is semantically right (dialogs are top-level modals,
  not children of a widget); forcing uniformity would break the 7 backends behaving correctly. The
  contract is now pinned by a test so future audits do not re-flag it.
- **`accessibility/windows` stays gated.** It references `winapi::um::winuser::EVENT_*`
  unconditionally, so it genuinely cannot compile off Windows, and un-gating it would only produce
  an empty assertion body (false coverage).
- **An RSS-based leak claim was retracted as wrong.** A draft report asserted a leak from
  "8000 widgets → +49 MB RSS". Three independent checks disproved it: macOS `leaks --atExit`
  reported `0 leaks for 0 total leaked bytes`, malloc node counts did not grow under churn, and a
  pure-AppKit control (no rust-widgets code) reproduced the same RSS curve. The growth is AppKit's
  own caching. Regression tests now assert registry state, not RSS.

## 1.1.1 (2026-09-11) — Cross-compilation & Coverage Visibility Release

### Fixed — cross-compilation
- **AVIF now uses the pure-Rust `avif` codec instead of `avif-native`.**
  `avif-native` pulled in `dav1d-sys`, a system C library that fails to cross-compile for
  Android/iOS/wasm unless a pkg-config sysroot is configured by hand. This made the
  `mobile`/`tablet`/`desktop` profiles impossible to build for any foreign target.
  Trade-off: slower AVIF decode and ~15 extra build-time crates (`rav1e` et al.), in exchange
  for genuinely cross-compilable builds. Verified: Android (2 ABIs × 5 profiles), iOS,
  Windows (msvc/gnullvm), and wasm32 all compile clean.
- **Cross-target cfg-gating mismatches fixed** in `platform/runtime.rs`, `platform/android/types.rs`,
  `platform/ios/types.rs`, `gpu/adapter.rs`, and `compat.rs`. Several imports and helpers were gated on
  `target_os` alone while their users were also gated on `not(feature = "mini")` /
  `not(feature = "embedded")`, producing `E0432`/`E0433`/`E0599` on the affected combinations.
- **`compat::OnceLock` gained `set()`** so the `mini` profile matches the `std::sync::OnceLock` API
  surface (the Windows notify path calls it).

### Fixed — control routing
- **`NativeControlBackend` delegated to the wrong platform primitives**, discarding real Win32
  implementations:
  - `create_checkbox` → `create_toggle_button` (bypassed `BS_AUTOCHECKBOX`)
  - `create_spin_box` → `create_double_spin_box` (bypassed `msctls_updown32`)
  - `create_scroll_area` → `create_panel` (bypassed the scrollable child window)
  The two reverse delegations (`create_toggle_button`, `create_double_spin_box`) were dead code and
  now preserve widget identity.
- **Per-OS routing for `SpinBox`/`ListView`/`ScrollArea`.** They have real Win32 implementations but
  were listed in the global `CustomRequired` arm, so the native path was unreachable on Windows while
  other platforms (which have no such primitive) genuinely need the custom backend. Routing now
  promotes them to `NativePreferred` **only under `cfg(target_os = "windows")`**, pinned by two
  complementary tests.

### Fixed — test coverage visibility
- **50 pure-logic tests that never compiled or ran on any reachable path** are now part of the host
  build, following the BLUE14 rule that "not in the build" must be distinguished from "failed" and
  "ignored": `ime_macos` (19), `android` (8), `ios` (6), `macos_objc2` (17). Each was verified by
  mechanical scan to touch no OS API.
- **`AndroidHandleKind` was missing `Debug`**, which made three of its own `assert_eq!` tests
  impossible to compile. Never surfaced because the module was excluded from the host build.
- **`cargo test --all-features` (the actual CI command) did not compile** (`E0277`): `serialize_state`
  was gated more loosely than the `BackendState` `Serialize` derive it depends on.
- **`tools/generate_platform_capability_matrix.py`** now derives the degradation table by mechanically
  parsing `native.rs`; the capability-matrix doc is gated against drift in CI. Its prose is generated,
  so hand-editing the markdown is no longer possible without the freshness check failing.

### CI
- **`wasm-check` job was permanently broken**: it ran `--features wasm` without
  `--no-default-features`, which pulled in the whole `desktop` profile (audio, JS engine) for a wasm32
  target. Now pinned to `--no-default-features --features wasm`.
- **`android-cross-check` job was a no-op**: it passed `-Zlinker-features`/`-C linker` (rustc flags,
  invalid for `cargo check`) and swallowed every failure with `|| echo skipped`. It now exports the
  NDK linker/compiler via `CARGO_TARGET_*`/`CC_*` and checks 2 ABIs × 5 profiles with no failure mask.

### Changed
- Crate version bumped `1.1.0` → `1.1.1`. **No ABI change**: `rw_bindings_api_version` remains `8`
  and no exported symbols were added or removed.
- Version references aligned to `1.1.1` across `Cargo.toml`, the `CoreConfig` runtime version contract,
  Node.js/Python package metadata, the demo banner, and the en/zh-CN/zh-TW cookbooks.

### Documentation
- README (en + zh-CN) refreshed: corrected the stale test-count badge (`3700+`/`3400+` → `3850+`),
  documented the AVIF codec switch and its trade-off, added the CI cross-compile commands, and
  filled in zh-CN sections that had drifted (Web widget list, Special widgets, `performance`/`memory`
  core modules, Performance section).
- `docs/plans/blue14.md` updated with the round-8 audit record (E-class), including the one item
  deliberately left gated (`accessibility/windows`) because un-gating it would only produce an
  empty assertion body.

### Verification
- `cargo test --lib`: **3853 passed**, 0 failed (was 3820).
- `cargo test --all-features` (CI command): 0 failed (previously failed to compile).
- `clippy --all-targets -D warnings` and `clippy --all-features --all-targets -D warnings`: 0 warnings.
- All profiles (desktop/mobile/tablet/mini/embedded/full) and cross-targets
  (iOS / wasm32 / Windows gnullvm) build with 0 errors, 0 warnings.
- Android JNI test APK builds and signs via `tools/build_android_testapp.sh`.

## 1.1.0 (2026-09-09) — Version Contract Sync Release

### Changed
- Crate version bumped `1.0.0` → `1.1.0` (stable line; **not** an ABI break).

### Bindings & docs
- Version references aligned to `1.1.0` across code, demo banner, and documentation:
  `Cargo.toml`, package metadata (Node.js `package.json`, Python `setup.py`), demo/control banner,
  CoreConfig default version contract, and all cookbook/README version mentions.

### Connectivity
- `CoreConfig::desktop()/embedded()/mobile()` default version synced to `1.1.0`
  (runtime `Version` contract mirrors the crate semantic version; existing `Version`-type API
  examples and ABI contract `rw_bindings_api_version` = `8` are unchanged).

## 1.0.0 (2026-09-02) — Stable Release

### Stability & Quality
- **Stable public API line declared**; `rw_bindings_api_version` bumped to `8` for the 1.0 ABI contract.
- **Full compile matrix at zero errors/warnings**: every profile (desktop/tablet/mobile/mini/embedded/bare),
  every capability feature and cross-combination, plus all installed targets
  (windows-msvc, android aarch64/x86_64/armv7, wasm32-unknown-unknown/wasip1/wasip1-threads) verified clean.
- **Platform code compiled for the first time**: Windows (TSF `Send` safety, winapi pointer types) and
  Android backends now build and lint cleanly.
- **WASM end-to-end**: real `cargo test` on `wasm32-wasip1` (2158 tests passing via wasmtime);
  browser wasm (`wasm32-unknown-unknown`) builds/lints clean.
- **Tablet/mobile profiles compile for the first time**: dependency/feature gaps fixed
  (i18n/video feature cohesion, `tr!` macro gating, runtime stub cfg), 3509/3514 tests passing.

### Honest implementation pass (no fake/stub behavior)
- Audio: removed synthetic FLAC/OGG/AAC/Opus decoding and PCM-impersonating encoders; missing codec
  features now return explicit `Err`. Real FFmpeg/symphonia paths verified (2195 tests).
- Image: PNG decoder rewritten (scanline filters, RGBA stride, 16-bit sampling, bounds safety);
  GIF/WebP/TIFF/AVIF/ICO/SVG decoders now refuse instead of fabricating pixels.
- Video MJPEG failures are explicit (`FrameType::Synthetic` + diagnostics); web-engine JS evaluation
  wired to a real engine; PDF security markers no longer leak passwords.
- Line-edit clipboard shortcuts wired; undo zero-capacity panic fixed; software text rastering
  handles Unicode instead of truncating to `u8`.
- Dead code removed (`misc_widgets/chip.rs`, `image/svg_utils.rs`); dead test file mounted (`pdf/tests.rs`).

### Bindings & docs
- FFI bindings complete: C++ header and Python wrapper cover all 105 exported symbols (105/105);
  `rw_bindings_api_version` = 8 for the 1.0 ABI line.
- Capability matrix regenerated at 167 rows matching the 167 `WidgetKind` variants, with honest
  degradation notes; `unexpected_cfgs` lint restored to `warn`.
- README/docs aligned with code; version bumped 0.9.10 → 1.0.0 everywhere (Cargo, demo banner,
  Node.js package, migration guide).

### Full release backlog (folded in from the former `[Unreleased]` section)
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

## 0.9.10 (2026-07-23) — Code Quality Release

### Infrastructure
- **mod.rs refactoring (20/20 complete)**: All module declaration files now contain only `pub mod` declarations and `pub use` re-exports; implementation code moved to dedicated files (custom_paint, init, legacy_types, a11y_wiring, arc_helpers, gpu_types, backend, test_types)
- **Three profiles at 0 errors**: `default`, `mini`, and `embedded` all pass `cargo check` with zero errors
- **0 clippy warnings**: All unnecessary casts and manual range checks eliminated
- **0 `#[deprecated]` items**: All 28 deprecated annotations removed
- **0 `todo!()` / `unimplemented!()`** across the entire codebase

### Bug Fixes
- PdfPage naming conflict: struct renamed to `ExportPage`, trait retained as `PdfPage`
- Control backend dispatcher: fixed overlapping `#[cfg]` conditions causing duplicate function definitions in mini profile
- Embedded profile: 70 errors fixed (chrono/serde dependency gating, advanced widget module gating)

### Dependency Management
- `serde::Serialize`/`Deserialize` derives made conditional: `#[cfg_attr(feature = "serde", derive(...))]` across core types (Color, Font, WidgetRecord, etc.)
- `chrono` imports gated behind `not(mini)` / `not(embedded)` in capability layer
- `serde_json::Value` usage gated behind `not(embedded)` for json module
- Asset, advanced_widgets, media_widgets modules gated behind `cfg` for embedded profile

### Test
- Total: 3771 tests passing (3676 lib, 70 integration, 25 doctests)

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
