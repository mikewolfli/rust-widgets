# Changelog

The canonical project changelog is maintained at [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md).

This root-level file exists for tools and release automation that expect `CHANGELOG.md` at repository root.

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
