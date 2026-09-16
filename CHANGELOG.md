# Changelog

The canonical project changelog is maintained at [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md).

This root-level file exists for tools and release automation that expect `CHANGELOG.md` at repository root.

## 2.0.1 (2026-09-16) — Linux GTK Backend Restored, HarmonyOS Target Buildable

A corrective release for platform paths that 2.0.0's verification did not cover. No API
changed.

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) for the full list and
[docs/log/log-20260916-1.md](docs/log/log-20260916-1.md) (Linux/Windows hosts) and
[docs/log/log-20260916-2.md](docs/log/log-20260916-2.md) (HarmonyOS cross target) for the
reproduction commands.

### Fixed

- **HarmonyOS targets did not build.** Every `*-unknown-linux-ohos` target reports
  `target_os = "linux"` and `target_env = "ohos"`, so the backend-selection sites written
  as `cfg(target_os = "ohos")` never matched and the target had no platform constructor.
  Selection now keys off `target_env`, and the Linux/Wayland arms exclude OpenHarmony
  explicitly (they share `target_os = "linux"`).

- **`desktop` did not compile on Linux** — the `canvas` call sites dropped the
  `gtk-native` condition the module itself is gated on.
- **The `gtk-native` backend had never compiled at all** — bare `glib`/`gdk`/`cairo`
  crate names, a missing import, a `Fixed`-only method called on a child, and an
  unsafe call without an `unsafe` block.
- **A SIGSEGV in `gtk-native` test runs**, plus the underlying single-main-thread
  constraint: GTK aborts when a second thread initializes it or builds a window.
  `init`, `create_window` and `mount_surface` now degrade honestly instead.
- **The Linux clipboard was never wired** — every other backend delegated to the
  shared state record; Linux inherited the `false` default, so copy/paste was a
  silent no-op.
- **A library-created window could not carry controls** (BLUE15 Gap B) — the widget
  registry id and the platform's window id were never linked, so
  `App::new_window(..)` + `mount_widget_by_name(..)` was refused.
- **The Wayland menu model had no producers** for the nodes it validated, making
  `attach_menu_bar_to_window` and `menu_add_item` unreachable.
- **Harmony ignored injected widget-trigger events** (the methods were not wired to
  the shared implementation four sibling backends use).
- **`cargo test --all-features` failures**, including three examples that cannot
  compile when `desktop` and `mini` are both on.

### Verification

Five feature configurations plus `--all-features` build and test clean; zero clippy
warnings under `-D warnings`; clean `cargo doc -D warnings`; clean cross-target builds
for windows-msvc, wasm32, android and OpenHarmony (`aarch64-unknown-linux-ohos`).
New CI jobs `linux-gtk` (the `gtk-native` combination) and `harmony-cross-check` (the
OpenHarmony target) cover two configurations no previous job built.

## 2.0.0 (2026-09-14) — Self-Drawn Controls Everywhere (BLUE15)

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md),
[docs/MIGRATION_GUIDE.md](docs/MIGRATION_GUIDE.md) and the nine execution logs
[docs/log/log-20260914-1.md](docs/log/log-20260914-1.md) for full details.

### ⚠️ Breaking changes

- **Native control creation is gone from all ten backends.** `Platform::create_*`
  and every platform-side control constructor were deleted — Windows `CreateWindowExW`,
  macOS `NSButton`/`NSTextView`, GTK `gtk_button_new`, Android
  `android.widget.*` via JNI, iOS `UIButton`, and the rest. `fn create_*` went from
  **606 → 0** in `src/platform/**`.

  **The library paints 100% of its own controls.** A backend now supplies only what
  genuinely belongs to the OS: a surface, the event loop, input translation, and
  platform services (IME, clipboard, accessibility, file dialogs, DPI).

  *Migration:* replace any `Platform::create_button(...)`-style call with the
  control-backend API (`control_backend::create` / `WidgetFactory`). Controls are
  still addressed by id; only their construction moved.

- **The property layer is now per-control.** Every control implements
  `WidgetProperties` in its own file (`get` / `set` / `property_names`). The
  centralised `read_widget_property_legacy` / `write_widget_property_legacy`
  dispatch — 18 include files and **535 match arms** — was deleted.

  *Migration:* `Platform::get_*`/`set_*` control-property methods are gone. Read and
  write through `WidgetFactory::{read_property, write_property}` or the id-level
  `read_widget_property_by_id` / `write_widget_property_by_id`.

- **`Platform` was de-controlified**: its required methods went from **75 → 6**
  (surface, event loop, lifecycle). The removed methods now have honest defaults that
  report `UnsupportedOnWidget` rather than pretending to act.

- **`NativeCapabilityContract` is now a type alias** for `PlatformCapabilities`.
  They held the same five flags and were kept in step by a field-by-field copy, which
  is how such pairs drift. The name is unchanged; the conversion function is gone.

- **Deleted unused public modules** (zero in-tree references, verified before
  removal): `platform::detector` (`DeviceEnvironment`), `platform::virtual_keyboard`,
  `render::text_cache`, `style::css_watcher`, `util::asset_watcher`,
  `widget::image` (a re-export shim), and `platform::ime_stubs`.

  *Migration:* use `crate::image::{Image, ImageFormat}` (was `widget::image`), and
  `crate::asset::{AssetWatcher, AssetEvent}` (was `util::asset_watcher`).

- **`bindings/java/RustWidgetsAndroid.java` no longer declares** `nativeCreate*`,
  `nativeSetView*`, `nativeDestroyView` or the `nativeSelfTest*` probes, because the
  Rust symbols they bound to no longer exist. The class keeps the platform handshake
  (`nativeInit`, `nativeAttachContext`, `nativeDetachContext`), diagnostics and
  `nativeOpenDocument(String mimeType)`.

### Added

- **Four factory-driven property-contract gates**, which turned "is every control's
  property surface complete?" from manual review into a machine check:
  `every_factory_widget_declares_a_property_contract`,
  `every_shared_kind_has_a_tie_break`,
  `no_published_property_answers_unknown_when_written`,
  `schema_and_contract_publish_the_same_names`. They found, and this release fixes:
  59 controls with no contract, 10 shared `WidgetKind`s silently reading another
  control's schema, 42 properties answering `UnknownProperty` where `ReadOnlyProperty`
  is correct, and 155/155 schema tables omitting the shared `visible`/`geometry` names.
- **`Platform::surface()` / `render_engine::surface_policy()`** — one strategy table
  describing how each profile gets a drawing surface, with invariant tests.
- **A `portable` backend** (`platform::portable`) for targets with no OS behind them;
  it carries `mini` and host-less `embedded`.
- **`platform::os_probes`** — the `/proc` and `/sys` system probes shared by the five
  Linux-kernel backends, and the unix print spooler helper.
- **`widget::text_utils::floor_char_boundary`** and
  **`widget::misc_widgets::date_utils`** — shared by six text controls and three date
  pickers respectively.
- **`impl_default_via_new!`** — one macro replacing 117 identical `Default` impls.
- **New widget capabilities**: `Arc` gained readable `minimum`/`maximum`/`sweep_angle`/
  `thickness`/`indeterminate`; `TabBar` gained `clear_current_index()` so
  `current_index` reads as `Null` and can be written back to `Null` (the round trip
  previously did not close); `TagInput` gained a real `placeholder`; `DropdownMenu`
  gained `selected_index` accessors.

### Changed

- **`check_apple_native.sh` now asserts self-painting instead of native controls.**
  The iOS Simulator probe previously asserted that `rw_create_button` produced a real
  `UIButton` subview — i.e. it asserted the *absence* of the feature this release
  implements, and failed on correct code. It now asserts the opposite:
  `no_backend_owned_window` and `self_painted_no_native_views`, which fail if native
  construction ever returns.
- **`check_jni_signatures.sh`** repaired from 20 errors to 0.
- **Android gating widened** so `--features android` and `android,android-jni` build
  cleanly; fixed a real pre-existing bug where `LOGCAT_LOGGER` was referenced but never
  declared.

### Performance & size

- **Net −2,288 lines of source** in this release, with no capability removed.
- Property reads are **one direct `match`** instead of up to nine serial category
  probes; `property_names()` returns a `&'static [&'static str]` (zero allocation).
- `src/platform` shrank from **43,151 → 22,010** lines across the BLUE15 series.

### Verification

- **4,016 tests passing** (desktop), 1,459 (embedded), 1,388 (mini); 0 failing.
- **0 clippy warnings** on `desktop`/`embedded`/`mini` with `--all-targets -D warnings`.
- **0 errors** across 9 real feature combinations × `--all-targets`.
- **16/16 QA gates pass**, including a real iOS Simulator run.
- **No ABI change**: `rw_bindings_api_version` remains `8`.

## 1.1.3 (2026-09-13) — Unified Native-Control Property API Release

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) and
[docs/log/log-20260913-1.md](docs/log/log-20260913-1.md) for full details.

### Highlights
- **Native controls now expose one property API that works unchanged on every OS.**
  Slider value/range/step, progress value and busy state, spin-box value/range/step,
  combo/list selection index, check/tri-state state, text-entry read-only/max-length/
  placeholder/echo-mode/selection, window state (maximise, minimise, full-screen,
  resizable, decorated), window minimum size and window icon are all reachable through
  `Platform` without a single `cfg(target_os)` or per-OS `if`/`else` at the call site.
- **What is unified is the call *shape*, not the capabilities.** Each backend maps a call
  onto whatever its own toolkit actually offers and reports honestly (`false` / `None`)
  when it cannot; genuine per-OS differences (AppKit has no placeholder on `NSTextView`,
  no `NoEcho` mode anywhere, no runtime slider orientation on Win32) are documented rather
  than papered over. What is forbidden is a write that reports success without taking effect.
- **Fixed a whole-family defect on macOS**: 20 `create_*` constructors
  (`list_view`, `group_box`, `frame`, `tab_widget`, `splitter`, `toggle_button`, `calendar`,
  `scroll_bar`, `double_spin_box`, `font_combo_box`, `context_menu`, `popup_window`,
  `dialog`, `input_dialog`, `progress_dialog`, `directory_dialog`, `date_picker`,
  `time_picker`, `date_time_picker`, `activity_indicator`) recorded widget state but never
  registered a handle, so **every handle-gated property refused with "unknown id"** while
  the constructor still returned a non-zero id. Purely state-backed reads (text,
  visibility) kept working, which is why it went unnoticed.
- **Fixed macOS window-state transitions**: `toggleFullScreen:` was gated on a style-mask
  read that is still stale during the transition, so turning full screen *off* never sent
  the toggle; `miniaturize:`/full-screen are now read back from the request while the
  run-loop transition is pending.
- **Corrected the API shape for slider orientation**: it is a creation-time property on
  Win32 (`TBS_VERT`, no `TBM_*` runtime message) and AppKit, so the misleading
  `SliderHandle::set_orientation` setter was replaced by
  `WindowHandle::new_slider_with_orientation` plus a read-only `orientation()`.
- **SPDX headers** (`MIT`, copyright Mike Li / Mikewolfli / Wei Li) added to all 601
  source files under `src/` via the idempotent `tools/add_spdx_headers.py`.
- **No ABI change**: `rw_bindings_api_version` remains `8`; no exported `rw_*` symbol
  was added, removed or changed.
- **4005 tests passing**, 0 failing; 0 clippy warnings; 0 errors/warnings across all 8
  checked configurations (desktop, mini, embedded, tablet, Windows, Windows+mini, wasm32,
  and a GTK type-check of 37 property functions on a non-Linux host).

## 1.1.2 (2026-09-12) — Platform Correctness & Unsafe-Surface Audit Release

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) for full details.

### Highlights
- **Apple backends were only ever exercised off-host until now.** Un-guarded AppKit calls made
  `cargo test --lib --features desktop` **SIGABRT the whole test process** on macOS; the objc2
  backend was silently degraded to state-only because it was gated on the alias feature
  `objc2-macos` (43 call sites compiled out); and `ime_macos`'s `ImeCtx` was declared as four
  distinct local types, so three native IME paths silently did nothing
- **Removed 3 unnecessary `unsafe impl Sync`**, each proven redundant by a delete-and-compile test
  (`EventHandlerContext`, `LinuxPlatform`, `TsfThreadMgr`); the one that is genuinely required
  (`AndroidPlatform`) was confirmed required by 55 errors when removed
- **Added widget destruction** (`Platform::destroy_widget` / `widget_count` / `rw_destroy_widget`,
  all 11 backends) — previously there was **no way to destroy a widget at all**
- **Fixed two temp-file leaks** on FFmpeg encode/decode error paths, a GTK clipboard **process abort**
  when called off the main thread, an i18n hot-reload miss on coarse-mtime filesystems, a data race
  in the `undo/stack` test fixture, and a `syncing` flag that a panic could wedge permanently
- **50 host-invisible tests now run** (`ime_macos` 19, `macos_objc2` 17, `android` 8, `ios` 6)
- **Corrected docs that contradicted the code**: `codemap.md` said 166 widget variants (actual 167);
  both READMEs said "80+ widgets" (actual 167 kinds); new `check_widget_kind_count.sh` gate prevents
  recurrence
- **No ABI break** (`rw_bindings_api_version` = `8`); one new symbol (`rw_destroy_widget`)
- **3854 tests passing**, 0 failing; 0 clippy warnings across all profiles and targets

## 1.1.1 (2026-09-11) — Cross-compilation & Coverage Visibility Release

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) for full details.

### Highlights
- **AVIF switched to the pure-Rust `avif` codec** (no more `dav1d-sys`), making
  `mobile`/`tablet`/`desktop` genuinely cross-compilable for Android/iOS/wasm
- **Control routing fixed**: `NativeControlBackend` was bypassing real Win32 primitives
  (`BS_AUTOCHECKBOX`, `msctls_updown32`, the scrollable child window); `SpinBox`/`ListView`/
  `ScrollArea` now route natively **on Windows only**
- **50 previously invisible tests** (`ime_macos`/`android`/`ios`/`macos_objc2`) are now compiled
  and executed on the host; fixed a missing `Debug` that made three `android` tests uncompilable
- **`cargo test --all-features` (the CI command) now compiles** — it previously failed with `E0277`
- **Two dead CI jobs repaired**: the wasm job was missing `--no-default-features`, and the Android
  job swallowed every failure
- **Version references aligned to `1.1.1`**; **no ABI break** (`rw_bindings_api_version` = `8`)
- **3853 tests passing**, 0 failing

## 1.1.0 (2026-09-09) — Version Contract Sync Release

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) for full details.

### Highlights
- **Crate version bumped `1.0.0` → `1.1.0`** (stable line; no ABI / `Version`-type API break)
- **Version references aligned across code & docs**: Cargo, Node.js `package.json`, Python `setup.py`,
  demo/control banner, CoreConfig default version contract, cookbook/README mentions
- **`CoreConfig::desktop()/embedded()/mobile()`** default version synced to `1.1.0`

## 1.0.0 (2026-09-02) — Stable Release

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) for full details.

### Highlights
- **Stable public API line**; C ABI contract version bumped to `8`
- **Zero errors/warnings across the full matrix**: all profiles, capability features, and installed targets
  (windows-msvc, android ×3, wasm ×3) — including first-time Windows/Android/tablet/mobile compilation
- **Honest implementation pass**: no fake/stub decoding (audio/image/video), PNG rewrite, PDF password leak fixed
- **WASM end-to-end**: `cargo test` on `wasm32-wasip1` = 2158 passing
- **3793 tests passing**, 0 failing

## 0.9.10 (2026-07-23) — Code Quality Release

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) for full details.

### Highlights
- **mod.rs refactoring (20/20 complete)**: all module files now re-exports only
- **Three profiles at 0 errors**: default, mini, embedded
- **0 clippy warnings**, 0 deprecated items, 0 todo!()/unimplemented!()
- **3771 tests passing**, 0 failing
