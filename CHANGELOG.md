# Changelog

The canonical project changelog is maintained at [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md).

This root-level file exists for tools and release automation that expect `CHANGELOG.md` at repository root.

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
