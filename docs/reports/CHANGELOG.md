# Changelog

All notable changes to this project are documented in this file.

## 2.1.0 (2026-09-17) — HarmonyOS Made Real, Error Messages Audited, Cross-Target `--all-targets` Fixed

This release makes the cross-target claims **falsifiable**. No public API changed and no
capability was added or removed. Every entry is code that already claimed to work and did
not — the recurring shape is *a gate that had never been run in the configuration it
names*, which is exactly what a green gate is supposed to rule out.

### Fixed

- **The OpenHarmony SDK was installed but its Rust targets were not**, so "HarmonyOS
  passes" had never actually been observed on this host. Installing
  `aarch64`/`armv7`/`x86_64-unknown-linux-ohos` was the missing step. All three now:

  | target | `check` | `build` + link | artifact machine | clippy `-D warnings` |
  |---|---|---|---|---|
  | `aarch64-unknown-linux-ohos` | ✅ | ✅ 275 MB | `AArch64` | ✅ |
  | `armv7-unknown-linux-ohos` | ✅ | ✅ 252 MB | `ARM` | — |
  | `x86_64-unknown-linux-ohos` | ✅ | ✅ 277 MB | `X86-64` | — |

  `loongarch64-unknown-linux-ohos` stays unbuildable — rustup ships no std for it
  (Tier 3) and the SDK ships no libc for that architecture — and `check_harmony_cross.sh`
  **pins that specific outcome** rather than reporting a pass for a target it never
  touched. Full gate: `All HarmonyOS cross-target checks passed.` (6/6).

- **`cargo check --target wasm32-unknown-unknown --all-targets` did not compile at all.**
  criterion is a host-only dev-dependency, and the bench sources carried a crate-level
  `#![cfg(not(target_arch = "wasm32"))]`. That removes the whole crate contents —
  including the `main` that `criterion_main!` generates — while the bench *targets* still
  exist in every configuration, so five benches failed with
  `E0601: main function not found in crate`. The benches are now gated **item by item**,
  with an explicit `#[cfg(target_arch = "wasm32")] fn main() {}` fallback so the crate is
  well-formed wherever it is built. `json_bench` already used this pattern, which is what
  made the other four look like a style difference rather than a defect.

- **`src/platform/os_probes.rs` did not compile outside unix/windows.**
  `print_job_waits_for_the_spooler_before_reading_back` called `stand_in_spooler`, which
  is `#[cfg(any(unix, windows))]`; the test was ungated. The property it asserts really
  is about the spooler submission path, which only exists on those hosts, so the gate
  belongs on the test.

- **An `unused_imports` warning on wasm32** for `AtomicBool` in `src/event/loop.rs`. Its
  only users are the native-pump tests, which are themselves gated on
  `not(target_arch = "wasm32")` (there is no native pump in a browser sandbox), so the
  import now carries the same gate.

- **206 error messages were not actionable.** `TODO.md` asks that "all error messages are
  user-friendly and actionable"; restated as a checkable rule, a message a caller can see
  must (a) name the specific value/path/id that failed and (b) state the expected form or
  the next step. Every reported site now does both:

  ```text
  before  PNG dimensions too large: 4096x4096
  after   PNG is 4096x4096 (16777216 pixels), which exceeds the 134217728 pixel cap;
          downscale the image before decoding

  before  Invalid JPEG signature
  after   JPEG must start with the SOI marker FF D8, but this 3-byte input starts with [50, 36, 52]

  before  Nothing to undo
  after   nothing to undo: the undo stack is empty (0 redoable command(s) pending)
  ```

  The scanner itself had a real defect worth recording: its message regex
  `["']([^"']{4,200})["']` stops at the `'` inside `"muxer '{name}' could not be
  created"`, so the finding was reported as the fragment `muxer ` — which then looked
  like a message that named nothing. Twelve of the reported messages were this artifact.
  `STRING_LITERAL` now accepts `{…}` interpolations in the body. Report:
  **206 findings → 0**. Six `expect(..)` strings were reclassified rather than rewritten:
  they follow a value the crate itself constructs from literals
  (`from_ymd_opt(1900,1,1)`, `with_day(1)`, `from_size_align(capacity, 8)`), so no caller
  input can reach them — they explain a crash trace, they are not messages to act on.

- **Two doc-lint failures blocked the build.** `#![deny(missing_docs)]` rejected
  `MacOSAccessibilityBridge::new` and `MacOsClipboard`, neither of which carried a doc
  comment. `cargo check --features desktop` failed with 2 errors before this round.

- **Four tests asserted the exact old wording** and were updated to assert the refined
  contract instead of a substring that had to change with it: the messages they pinned
  (`"cannot print empty content"`, `"JSON data is empty"`, `"Nothing to undo"`,
  `"JSON parse error"`, `"not found"`) were themselves the defect. Each test now pins the
  two facts a caller needs — e.g. the missing id **and** the list of available ids.

### Verification

```text
$ cargo test --no-default-features --features desktop --lib -q
  test result: ok. 4127 passed; 0 failed; 0 ignored
$ cargo test --no-default-features --features embedded --lib -q
  test result: ok. 1490 passed; 0 failed; 0 ignored
$ cargo test --no-default-features --features mini --lib -q
  test result: ok. 1411 passed; 0 failed; 0 ignored

$ cargo fmt --all -- --check                                  fmt OK
$ cargo clippy --all-features --all-targets -- -D warnings     clean
$ python3 tools/check_error_messages.py
  Scanned 98 error message(s) that leave the crate from src/.
  0 do not satisfy both parts of the rule.
```

Cross targets, all with 0 warnings:

```text
wasm32-unknown-unknown --features wasm --all-targets            Finished
x86_64-pc-windows-gnu --features windows --all-targets         Finished
aarch64-apple-ios          --features ios,ios-uikit-ffi          Finished
aarch64-apple-ios-sim      --features ios,ios-uikit-ffi --all-targets  Finished
aarch64/armv7/x86_64-unknown-linux-ohos (SDK sysroot, linked)   Finished
```

## 2.0.1 (2026-09-16) — Linux GTK Backend Restored, Windows/Linux Link Fixes

A corrective release for platform paths that 2.0.0's verification did not cover. No API
changed; no capability was added or removed. Four of these were **functional blockers**
on the affected platform, and all of them were invisible to the macOS-based evidence
that 2.0.0 relied on.

### Fixed

- **`desktop` did not compile on Linux.** The `canvas` call sites were gated on
  `widgets_unstripped` while the module itself also requires `gtk-native`, so
  `--features desktop` produced four `E0433`s.
- **The `gtk-native` backend had never compiled at all.** It referred to `glib`/`gdk`/
  `cairo` as bare crates (only `gtk` is a declared dependency), missed a `MutexExt`
  import, called `Fixed`-only `move_` on a child widget, and called an unsafe
  `destroy` without an `unsafe` block. None of these needed a running GTK to find.
- **A process-wide SIGSEGV in `gtk-native` test runs.** GTK binds a process to one main
  thread, but the Rust test harness runs each `#[test]` on its own thread. `init` now
  guards the check-then-initialize sequence with a process-wide lock, and `create_window`
  / `mount_surface` degrade to a state-only window off the main thread instead of
  aborting — the pattern the macOS backend already used for AppKit.
- **The Linux clipboard was never wired.** Every other backend delegated to the shared
  state record; `LinuxPlatform` inherited the trait default that returns `false`, so
  copy/paste inside the library was a silent no-op on Linux.
- **A library-created window could not carry controls** (BLUE15 Gap B). A window existed
  in two disconnected id spaces — the widget registry's and the platform's — and
  `mount_surface` can only resolve the latter. `App::new_window(..)` followed by
  `mount_widget_by_name(..)` returned "refused the mount"; the host-window link is now
  recorded at creation and resolved at mount.
- **The Wayland menu model had consumers but no producers.** `create_menu_bar` first
  checks that the parent is a `MenuBar`, but nothing ever constructed one, so
  `attach_menu_bar_to_window` and `menu_add_item` could only return `false`/`0`.
- **HarmonyOS targets did not build at all.** Every `*-unknown-linux-ohos` target reports
  `target_os = "linux"` and `target_env = "ohos"`, so the six backend-selection sites
  written as `cfg(target_os = "ohos")` never matched and `create_native_platform` had no
  definition for the target. The checks now key off `target_env`, and the Linux/Wayland
  arms exclude OpenHarmony explicitly (they share `target_os = "linux"`, so without that
  exclusion both would match). Honest note: the backend status doc's own verification
  command passed a `harmony` feature, which is a development switch — it satisfied the
  `feature = "harmony"` arm and so masked the broken target arm.
- **Harmony ignored injected widget-trigger events.** `inject_widget_trigger_event` and
  `poll_widget_trigger_event` were not wired, though the shared implementation existed
  and four sibling backends used it.
- **A `clippy::missing_const_for_thread_local` false report on the OpenHarmony target**
  (`src/widget/runtime.rs`). The lint suggests moving `RefCell::new(HashMap::new())` into
  a `const` block, but `std::collections::HashMap::new()` is not a `const fn`; the host
  toolchain can resolve `HashMap` and stays quiet, OpenHarmony's std cannot. The
  suggestion would not compile, so it is allowed at the three affected cells with the
  reason recorded.
- **`cargo test --all-features` failed** (5 cases at the start of this round). Two were
  the Wayland/Harmony defects above; three examples did not compile because
  `--all-features` enables `desktop` and `mini` simultaneously, which compiles out the
  modules they import (`required-features` cannot express this, since it is satisfied by
  any listed feature).
- **`reported_entry_point_count_matches_the_exports`** counted its own literal and the
  module documentation as JNI entry points, so it reported 9 export sites for 7 real
  `pub extern "system" fn` declarations. The declared count was correct; the measurement
  was not.

### Verification

Five feature configurations and `--all-features` build and test clean, with zero clippy
warnings under `-D warnings`, a clean `cargo doc -D warnings`, and clean cross-target
builds for `x86_64-pc-windows-msvc`, `wasm32-unknown-unknown`, `aarch64-linux-android`
and `aarch64-unknown-linux-ohos` (OpenHarmony).

New CI job `linux-gtk` compiles and tests the `gtk-native` combination, which no previous
job covered (and which `--all-features` cannot cover, because it also enables `mini`). It
re-runs the suite to catch the concurrency-dependent failure mode above.

New CI job `harmony-cross-check` builds the OpenHarmony target, which no previous job
covered. Its checks live in `tools/check_harmony_cross.sh` so that CI and a workstation
run identical commands; the gate also asserts the target-identification premise
(`target_os="linux"`, `target_env="ohos"`) that the selection logic rests on, so a future
change there fails loudly instead of silently flipping every backend arm.

Full detail, with reproduction commands and the before/after evidence for Gap B, is in
[docs/log/log-20260916-1.md](log/log-20260916-1.md). The HarmonyOS cross-target round,
including the masked-blocker analysis, is in
[docs/log/log-20260916-2.md](log/log-20260916-2.md).

## 2.0.0 (2026-09-14) — Self-Drawn Controls Everywhere (BLUE15)

A major release that completes the move to a **fully self-drawn** architecture. Every
platform backend used to build native OS controls alongside the Rust renderer; now all
ten backends build none. The library paints 100% of its own controls, so a control is
pixel-identical on every platform, needs no GUI toolkit linked, and works on targets
with no operating system at all.

The release also collapsed the property layer onto a per-control contract (deleting a
535-arm centralised dispatch), removed the duplicated platform probes and date/text
helpers, and replaced 117 hand-written `Default` impls with a macro — **net −2,288 lines
of source**, with no capability removed and four new mechanical contract gates added.

Full detail, with reproduction commands, is in
[docs/log/log-20260914-1.md](log/log-20260914-1.md).

### ⚠️ Breaking changes

#### 1. No native control construction on any platform

Every backend's native control constructor implementations were deleted. `fn create_*`
**implementations** in `src/platform/**` went from **606 → 0**, across all ten backends:

| Backend | Removed |
|---|---|
| Windows | `CreateWindowExW`-based control creation |
| macOS (cocoa + objc2) | `NSButton` / `NSTextView` / `NSComboBox` construction |
| Linux GTK | `gtk_button_new` and siblings |
| Wayland | xdg-shell control surfaces |
| iOS | `UIButton` / `UILabel` / `UITextField` |
| Android | `android.widget.*` via JNI |
| Harmony / WASM | state-backed control records |

**Migration:** controls are created through the control backend / `WidgetFactory`:

```rust
let factory = WidgetFactory::new_with_defaults();
let mut button = factory.create("button", Rect::new(10, 10, 100, 30), "OK").unwrap();
```

Controls are still addressed by id; only their construction moved.

> **Note on `Platform::create_*`.** The 42 trait *declarations* are kept, each with a
default that returns `0` and documents why. They are not removed because a backend
that genuinely owns a native primitive can still opt in deliberately, in one place,
without an API change — and because returning `0` is the honest answer, whereas a
constructor that reported success while creating nothing is the dishonesty the
defaults exist to remove. **No shipped backend overrides any of them.**

A backend now owns exactly four things: a surface, the event loop, input translation,
and platform services (IME, clipboard, accessibility, native menus, file dialogs, DPI).

> **The C ABI is unaffected.** `rw_create_button` and its 21 siblings, `rw_set_widget_text`,
> `rw_get_widget_text` and the rest of the `rw_*` surface still exist and still work —
> they are the control-backend API's C face, not the platform backends'.
> `rw_bindings_api_version` remains `8`.

#### 2. The property layer is per-control

Every control implements `WidgetProperties` in its own file. The centralised
`read_widget_property_legacy` / `write_widget_property_legacy` dispatch — 18 include
files, **535 match arms**, and the 98 imports that only it used — was deleted, because
it restated the contract for 39 controls and the two copies had already drifted.

**Migration:** read and write through `WidgetFactory::{read_property, write_property}`,
or the id-level `read_widget_property_by_id` / `write_widget_property_by_id`.
`Platform::get_*`/`set_*` control-property methods are gone.

#### 3. `Platform` de-controlified — 75 required methods → 6

The removed methods now have honest defaults reporting `UnsupportedOnWidget` / `None`
for a capability the host lacks, rather than a write that reports success without taking
effect.

#### 4. `NativeCapabilityContract` is now a type alias

`pub type NativeCapabilityContract = PlatformCapabilities;`. The two held the same five
flags and were kept in step by a field-by-field copy — the classic way such a pair
drifts. The name is unchanged; `from_platform_caps` is gone.

#### 5. Unused public modules deleted

Each had **zero** references in `src/`, `tests/`, `examples/` and `tools/`, verified
before removal:

| Removed | Was reachable as | Use instead |
|---|---|---|
| `platform::detector` | `DeviceEnvironment` (406 lines) | — |
| `platform::virtual_keyboard` | `VirtualKeyboard` (331 lines) | — |
| `render::text_cache` | `TextCache` / `GlyphCache` (318 lines) | — |
| `style::css_watcher` | `CssWatcher` (206 lines) | — |
| `util::asset_watcher` | re-export shim | `crate::asset::{AssetWatcher, AssetEvent}` |
| `widget::image` | re-export shim | `crate::image::{Image, ImageFormat}` |
| `platform::ime_stubs` | doc-comment-only file | `platform::ime_windows` / `ime_macos` |
| `widget::overlay_widgets::pull_to_refresh` | duplicate alias | `widget::PullToRefresh` |

Also deleted: four `macos_objc2` modules that contained only an SPDX header
(`clipboard_dnd`, `dialog_creation`, `menu_impl`, `widget_state`).

#### 6. Android Java binding trimmed

`bindings/java/RustWidgetsAndroid.java` no longer declares `nativeCreate*`,
`nativeSetView*`, `nativeDestroyView` or `nativeSelfTest*`, because the Rust symbols
they bound to no longer exist. A Java method whose Rust symbol is gone fails with
`UnsatisfiedLinkError` at the first call, which is worse than its absence. The class
keeps the platform handshake (`nativeInit`, `nativeAttachContext`,
`nativeDetachContext`), the diagnostics, and `nativeOpenDocument(String mimeType)`.

### Added

- **Four factory-driven property-contract gates.** These are the substantive change of
  the release: they turned "is every control's property surface complete?" from manual
  review into a machine check. On first run they found — and this release fixes:

  | Defect | Scale |
  |---|---|
  | Controls with no `WidgetProperties` contract at all | **59** |
  | Shared `WidgetKind`s silently reading *another* control's schema | **10** |
  | Published properties answering `UnknownProperty` where `ReadOnlyProperty` is right | **42** |
  | Schema tables omitting the shared `visible`/`geometry` names | **155 of 155** |

- **`surface_policy()`** — one strategy table for how each profile obtains a drawing
  surface, with monotonicity invariant tests.
- **A `portable` backend** (`platform::portable`) for targets with no OS behind them;
  it carries `mini` and host-less `embedded`, and is fully testable because nothing in
  the paint path is OS-dependent.
- **`platform::os_probes`** — the `/proc` and `/sys` probes shared by the five
  Linux-kernel backends (was 296 byte-identical lines across six files), plus the unix
  print-spooler helper.
- **`widget::text_utils::floor_char_boundary`** — shared by six text controls (was six
  copies).
- **`widget::misc_widgets::date_utils`** — month/day tables, leap-year arithmetic and an
  ISO-8601 parser shared by three date pickers (was a third copy of `MONTH_NAMES`). It
  also gains `parse_iso_date_checked`, which distinguishes "not a date" from "the day is
  out of range for the month", so `2026-02-30` is rejected instead of clamped.
- **`impl_default_via_new!`** — one macro replacing **117** identical `Default` impls.
- **New control capabilities:** `Arc` gained readable `minimum`/`maximum`/`sweep_angle`/
  `thickness`/`indeterminate` (they were writable but unpublished, i.e. undiscoverable);
  `TabBar` gained `clear_current_index()` so `current_index` can round-trip through
  `Null`; `TagInput` gained a real, settable `placeholder` that the renderer honours
  (previously a hardcoded string); `DropdownMenu` gained `selected_index` accessors.

### Changed

- **`tools/check_apple_native.sh` now asserts self-painting.** The iOS Simulator probe
  previously asserted that `rw_create_button` produced a real `UIButton` subview — it
  asserted the *absence* of the feature this release implements, and so failed on correct
  code. It now asserts the opposite via `no_backend_owned_window` and
  `self_painted_no_native_views`, which **fail if native construction ever returns**.
- **`tools/check_jni_signatures.sh`** repaired from 20 errors to 0.
- **Android feature gating widened** so `--features android` and `--features android,android-jni`
  build cleanly; fixed a real pre-existing bug where `LOGCAT_LOGGER` was referenced but
  never declared.

### Performance & size

- **Net −2,288 lines of source**, with no capability removed.
- A property read is **one direct `match`**, replacing up to nine serial category probes.
  `property_names()` returns `&'static [&'static str]` — zero allocation.
- `src/platform` shrank from **43,151 → 22,010** lines across the BLUE15 series.
- One fewer parse of `/proc/meminfo`, `/sys/class/power_supply` and `/proc/self/status`
  in five backends (the code existed five times, though only one copy ran per process).

### Verification

```text
cargo test --no-default-features --features desktop  --lib   4016 passed; 0 failed
cargo test --no-default-features --features embedded --lib   1459 passed; 0 failed
cargo test --no-default-features --features mini     --lib   1388 passed; 0 failed
cargo fmt -- --check                                         0 diff
cargo clippy --features desktop|embedded|mini --all-targets -- -D warnings
                                                             0 warnings (all three)
9 real feature combinations x --all-targets                  0 errors
16 QA gates                                                  all PASS
  (including a real iOS Simulator run)
```

> **No ABI change**: `rw_bindings_api_version` remains `8`; no exported `rw_*` symbol was
> added, removed or changed.

## 1.1.3 (2026-09-13) — Unified Native-Control Property API Release

A feature release that closes the gap between "native controls exist" and "native controls are
usable without writing per-OS code". Before it, the only OS-independent way to reach a control's
state was `set_widget_text` — lossy for anything whose payload is not its display text, and
unavailable for selection indices, ranges, busy states or window attributes. Each round of work
was verified by compilation on every target, by tests whose teeth were proven with negative
controls, and by a runtime probe on the real Cocoa backend.

### Added — one property API, every OS

`Platform` gained 36 semantic property methods, matched by 45 crate-root functions, 41
`WidgetHandle` trait methods, and implementations in every backend (macOS/AppKit, Windows/Win32,
Linux/GTK, and the state-only stub/mobile/wasm paths). Nothing at the call site branches on the OS.

| Group | Capability |
|---|---|
| Value controls | `set_widget_value` / `widget_value`, `set_widget_range` / `widget_range`, `set_widget_step` / `widget_step` |
| Selection | `set_widget_selected_index` / `widget_selected_index`, `set_widget_group` / `widget_group` |
| Checkable | `set_widget_checked` / `is_widget_checked`, `set_widget_tristate` / `is_widget_tristate` |
| Progress | `set_widget_indeterminate` / `is_widget_indeterminate` |
| Text entry | `set_widget_read_only` / `is_widget_read_only`, `set_widget_max_length` / `widget_max_length`, `set_widget_placeholder` / `widget_placeholder`, `set_widget_echo_mode` / `widget_echo_mode`, `set_widget_selection` / `widget_selection` |
| Window | `set_window_state` / `is_window_in_state`, `set_window_min_size` / `window_min_size`, `set_window_icon` / `window_icon` |
| Slider | `set_slider_orientation` / `slider_orientation` (creation-time; see below) |
| Scroll | `set_widget_scroll_position` / `widget_scroll_position` |

New public types: `WindowStateFlag` (an enum instead of five booleans, so a backend dispatches
with `match` rather than a chain of `if`/`else`), `WindowStateRecord`, and `EchoMode` moved from
`app` to `platform` so backends can name it without inverting the layering (`app` re-exports it,
so existing import paths keep working).

### Design decision — unify the call shape, not the capabilities

A control may carry a value on one OS and not on another; that is a genuine platform difference,
not a defect. The defaults return `false` / `None` — never a made-up value — so a backend that has
no such property reports it as *absent*, and the caller branches on the result at runtime if it
cares. Every refusal carries a doc comment explaining which toolkit limit causes it:

| Refused | Why |
|---|---|
| macOS placeholder / echo-mode | The macOS line edit is an `NSTextView`; `placeholderString` and `NSSecureTextField` belong to the `NSTextField` family, and changing class would rebuild the view (out of scope for an attribute write) |
| `NoEcho` on all three desktops | AppKit, Win32 and GTK each expose a mask character but no "echo nothing" mode |
| `set_widget_max_length` on macOS | `NSTextField` has no direct limit (it is enforced via a delegate) |
| Slider orientation on Win32 | `TBS_VERT` is a creation style; the full `TBM_*` message set has no orientation message |

### Changed

- **`SliderHandle::set_orientation` removed; orientation is now creation-time.** It was a setter
  that could not be honoured on two of three toolkits, which is exactly the kind of lie this release
  removes. Use `WindowHandle::new_slider_with_orientation(...)` (applies the orientation in one call
  on every OS) and read it back with `SliderHandle::orientation()`. The only in-tree caller
  (`demo/control`) was updated.
- `EchoMode` is now defined in `platform` and re-exported from `app`; `use rust_widgets::app::EchoMode`
  continues to compile.

### Fixed — macOS

- **20 `create_*` constructors registered no handle.** `list_view`, `group_box`, `frame`,
  `tab_widget`, `splitter`, `toggle_button`, `calendar`, `scroll_bar`, `double_spin_box`,
  `font_combo_box`, `context_menu`, `popup_window`, `dialog`, `input_dialog`, `progress_dialog`,
  `directory_dialog`, `date_picker`, `time_picker`, `date_time_picker` and `activity_indicator`
  called `state.create_widget` but never registered a handle, so every handle-gated property
  refused with "unknown id" — while the constructor still returned a non-zero id, and purely
  state-backed reads (text, visibility) kept working. All 20 now use `register_state_only_handle`.
- **Full-screen could not be turned off.** The guard compared the request against `styleMask`,
  which is still the pre-transition value while `toggleFullScreen:` animates, so `on == false`
  never sent the toggle.
- **`minimized` and `fullscreen` always read back stale.** Both transitions complete on the run
  loop, so an immediate read returned the old value. Reads now prefer the recorded request until
  the native state agrees, then trust the native query — so a user-initiated change is still seen.
- **`create_spin_box` registered no handle** (same class as the 20 above), so its value API
  refused.
- **`create_line_edit` builds an `NSTextView` with `setEditable: NO`**, i.e. macOS line edits start
  read-only. The state model now records that so `is_widget_read_only` reports the control's real
  state instead of the cross-platform default.

### Fixed — Windows

- `SendMessageW` argument widths corrected throughout the new code (`wparam: usize`, `lparam: isize`).
- `TBS_VERT` sourced from `commctrl` (not `winuser`); `SetScrollPos`/`GetScrollPos` take `c_int`.
- Scroll areas are created as `Static` windows carrying `WS_HSCROLL | WS_VSCROLL`, so `WM_GETMINMAXINFO`
  is routed through the real window procedure to enforce a window minimum size (Win32 has no setter).

### Fixed — Linux (GTK)

- `hadjustment()`/`vadjustment()` return an `Adjustment`, not an `Option` (GTK synthesises one);
  the first version unwrapped an `Option` that never existed.
- GTK 0.18 exposes `fullscreen()`/`unfullscreen()` but **no** `is_fullscreen()`, and no
  `is_iconified()`; those reads are served from the state model rather than inventing a
  `GdkWindow` query.

### Fixed — process

- **A regression test that could not fail was caught and fixed.** The first macOS guard for the 20
  missing handles asserted `set_widget_text` → `get_widget_text` round-trip plus `is_widget_visible`.
  A negative control (temporarily reverting one constructor) showed it still passed, because those
  two properties read the state model directly and never consult the handle table. The test now
  asserts handle registration itself, and the negative control fails loudly with the constructor's
  name.

### Tooling

- `tools/add_spdx_headers.py` — idempotent SPDX header writer (`--check` supported).
- `tools/gtk_property_check.py` — type-checks the Linux backend's GTK calls on a non-Linux host by
  lifting the property accessors into a scratch crate against the real `gtk` 0.18. It found two
  genuine defects this release (the `hadjustment` misuse and a duplicated function signature).
- `examples/control_property_uniform.rs`, `examples/macos_window_state_async_probe.rs`,
  `examples/macos_created_controls_are_usable.rs` — runtime probes for the new API, the AppKit
  transition timing, and handle registration respectively.

### Compatibility

- **No ABI change**: `rw_bindings_api_version` remains `8`; no exported `rw_*` symbol changed.
- One API shape change (slider orientation), with the replacement documented above and the sole
  in-tree caller migrated.

### Known issue (observed once, not reproduced)

During the version bump, one combined invocation reported
`test result: FAILED. 4004 passed; 1 failed` for the lib test binary. It has not reproduced in
more than ten subsequent runs, including the exact concurrency condition (a `cargo clippy`
contending for the build lock) and a forced full rebuild via `touch src/lib.rs`. No test asserts
the crate version, so the bump cannot be the cause. The failure is recorded rather than dismissed:
the suite contains tests backed by process-global state (the i18n manager, the platform singleton,
the JS-engine setter), so an order- or timing-dependent flake remains plausible. It is **not**
claimed fixed, and no test was changed to hide it.

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
