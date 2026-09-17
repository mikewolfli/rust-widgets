# Changelog

The canonical project changelog is maintained at [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md).

This root-level file exists for tools and release automation that expect `CHANGELOG.md` at repository root.
When the two disagree, this file is the one that ships; `tools/check_changelog_sync.sh` keeps them identical.

## 2.3.0 (2026-09-17) — Declarative-Retained View Layer, and One Paged Control Instead of Three

Backward compatible for the supported API. **One deliberate removal**: the two redundant
paged-view controls `PagerPageView` and `TileView` are gone, along with their
`WidgetKind` variants (`WidgetKind`: 171 → **169**). See *Removed* below for why this is
the intent rather than a regression.

The theme of this release is **the declarative half of the architecture**. The library
has always been retained-mode (a control is a long-lived object with an `ObjectId`), and
it now also has a declarative way to describe a tree — without giving that up. React,
Flutter and SwiftUI are all declarative *and* retained; the two are orthogonal axes, and
this release is about having both.

### Added

- **Declarative-retained view layer** (`rust_widgets::view`):
  - `Node` — a tree described as a value: widget name, optional `key`, properties, children.
  - `diff` — a pure function over two trees, producing `Patch`es (`SetProperty`, `Insert`,
    `Remove`, `Move`, `Replace`). It reports `positional_matches` so a missing `key` is
    **visible** rather than a silent identity drift.
  - `apply` — the only place that mutates the retained tree, through the same property
    contract the JSON loader uses.
  - `View` + `ViewEngine` — `mount` builds a tree, `update` diffs and applies only the
    differences, so untouched controls keep their focus, scroll offset and internal state.
  - **Additive**: no control, no `WidgetKind`, no factory and no property contract changed.
    `add_child` still works exactly as before.
- **`ReactiveHost`** — the production wiring from reactive state to the view layer.
  `BindingListener` must be `Send` and `ViewEngine` is `!Send`, so a listener cannot hold
  the engine; the listener records the change on a `Send`-safe queue and the UI thread's
  `pump()` does the engine work. A `Binding::set` on a worker thread now reaches a live
  control.
- **Carousel capability completion.** The three controls that each held a third of
  "one page at a time" are now one: mount real controls on a page
  (`set_page_content`), swipe with **distance and velocity** (a short flick pages, a slow
  short drag does not), autoplay with hover/press/disabled pausing, wrap-around, keyboard
  navigation, and an indicator that can be dots, bars, a `current/total` counter, or hidden,
  on any of the four edges.
- **Chart variants** (`ChartWidget`): `Area`, `Waterfall`, `Funnel`, `Candlestick` and
  `BoxPlot` (4 → 9 types), over a multi-series data model. `Vec<f64>` remains the common
  case; `set_series` is the general one.
- **`Meter` additions**: threshold colour bands, tick value labels, and a unit suffix.
- **Six new controls**: `KanbanBoard`, `RadarChart`, `Cascader`, `QueryBuilder`,
  `EmojiPicker` and `Mention`.
- **Drag-and-drop infrastructure** (`rust_widgets::event::dnd`): `DragPayload`,
  `DropEffect`, `DropTarget` and `DragSession`, replacing the three-event drag loop that
  28 files each implemented by hand.
- **`ScrollArea` sticky headers** (`add_sticky_region`).
- **Two new gates**: `check_view_platform_gate.sh` and `check_view_keys_are_unique.sh`
  (gates: 28 → **30**).
- `tools/check_changelog_sync.sh` — keeps this file and the canonical copy identical.

### Changed

- **`json` module documentation** now names the real reload path (`ViewEngine::mount` +
  `update`) instead of claiming hot-reload without an implementation behind it.
- **`check_profiles.sh`** verifies the declarative layer's platform gate as step `[8/9]`
  (8 → 9 steps).
- **Platform gates are now single names, not hand-written conjunctions.** `build.rs`
  derives three more cfg aliases — `device_profile`, `desktop_surface`,
  `declarative_view` — so `crate::json`, `crate::app`, `crate::theme` and `crate::view`
  each state their gate by intent:

  | Before | After |
  |---|---|
  | `all(any(feature = "desktop", feature = "tablet", feature = "mobile"), widgets_unstripped)` ×2 | `full_widgets` |
  | `any(feature = "desktop", feature = "tablet", feature = "mobile")` ×4 | `device_profile` |
  | `all(not(embedded_surface), feature = "desktop")` and its negation ×5 each | `desktop_surface` / `not(desktop_surface)` |

  The last one was the worst: the same fifteen-token conjunction appeared **ten times**
  as an if/else pair choosing a `WidgetKind`. No behaviour changed — every profile builds
  identically — but a gate can no longer drift from the others that mean the same thing.

### Features

- **`no-declarative-view`** (opt-out) — `rust_widgets::view` is compiled on `desktop`,
  `tablet` and `mobile` **by default**, as before. This feature removes it from such a
  build for callers that do not want its cost:

  ```bash
  cargo build --no-default-features --features desktop,no-declarative-view
  ```

  It is opt-**out** rather than opt-in deliberately: a Cargo feature list cannot express
  "on unless named", so an opt-in spelling would force every existing caller to add a
  feature to keep working. `mini`/`embedded` are unaffected — they never had the layer,
  and that is a property of those profiles rather than a choice.

  `tools/check_view_platform_gate.sh` now asserts all four states: present by default,
  absent with the opt-out (on each of the three device profiles), absent on a stripped
  profile, and absent on a build with no device profile.

### Removed

- **`PagerPageView` and `TileView`** (with their `WidgetKind` variants, factory names,
  property schemas, CSS selectors, JSON tokens and accessibility roles).

  These were **not** removed for size. Three controls described "one page at a time with
  dot indicators", and each held one third of the capability:

  | | content slots | swipe | keyboard | autoplay |
  |---|---|---|---|---|
  | `Carousel` | ❌ `{title, color}` only | ❌ | ❌ | ❌ |
  | `PagerPageView` | ✅ | ❌ | ✅ | ❌ |
  | `TileView` | ❌ (`page_count: u32`) | ❌ | ✅ | ❌ |

  A caller wanting a real carousel had to pick which third to go without, and no single
  control could host content *and* be navigated. `Carousel` is the name the `WidgetKind`,
  the factory and the CSS selector already used, so it absorbed the capability and the
  other two were deleted outright. Keeping them as deprecated shells would have preserved
  exactly the divergence the merge exists to remove. **`Carousel` is the replacement for
  both.**

- **Partial repaint now happens on its own** — and the reason it never did before was a
  defect, not a missing feature:
  - **Two id spaces.** `runtime::register` allocates a registry id and keys the widget
    table by it, while a widget's `BaseWidget::id()` comes from its own `Object` counter.
    They are different numbers (a fresh control reports `1`; the registry handed out
    `0x5345_4C46_0000_0001`). Damage filed under the widget's own id was therefore
    **silently dropped**. `register` now records the mapping and `unregister` removes it.
  - **No producer.** `mark_dirty_rect` had no production caller, so the tracker, the
    platform invalidation and `render_frame_incremental` were complete but unreachable
    outside tests. `BaseWidget::request_redraw` now records damage — the single point
    every appearance change converges on (~1000 call sites), which makes partial repaint
    correct *by construction* rather than dependent on a list of mutation sites.
  - **Two spellings of one operation.** `Widget::request_redraw`'s trait default emitted
    the signal itself instead of routing through `BaseWidget::request_redraw`, so whether
    damage was recorded depended on which spelling a call site used. Both emit the same
    signal, so no test could tell them apart; the default now delegates.
  - **The library decides, at mount time.** `register` asks `should_track_damage` and
    enables `RepaintMode::Adaptive` where regioning pays off. The judgement is two-sided:
    it refuses a surface below `AUTO_REPAINT_MIN_PIXELS` (250 000, ≈500×500) and one whose
    control has no children (one rect *is* the whole surface), so a small or single-rect
    widget keeps `Full` and pays no bookkeeping. A control that has never asked to be
    repainted is refused too — enabling a surface nothing redraws is pure cost.
  - `Adaptive` rather than `Dirty` is what makes an automatic *yes* safe: a frame whose
    damage covers the surface falls back to a whole paint *for that frame*, and a run of
    such frames stops measuring until the damage shrinks. A wrong yes costs bounded
    bookkeeping, never a wrong frame — `an_incremental_repaint_matches_a_full_repaint`
    compares the two byte for byte.
  - The decision is observable rather than a black box: `adaptive_large_damage_run(id)`,
    `should_track_damage(id)`, `enable_damage_tracking_if_useful(id)`,
    `set_repaint_mode_adaptive(id)`, `registry_id_of(own_id)`.
  - A control that asks to be repainted during construction is recorded too — the flag
    lives on the widget (`BaseWidget::has_ever_requested_redraw`), because a pre-mount
    request runs before there is any registry id to file it against.
  - `15` new tests (11 unit + 4 end-to-end), each shown able to fail by reverse injection.

### Fixed

- A duplicate `current_page` concept and two divergent indicator implementations are gone
  with the two removed controls.
- `tools/check_view_keys_are_unique.sh` — `src/view/node.rs` referenced this gate by name
  before it existed; now it exists, scans 130 builder chains, and fails on a real
  duplicate sibling key.
- `src/platform/windows/types.rs`'s window procedure was named `rw_wnd_proc`, borrowing
  the **C ABI** prefix for an internal Win32 callback. Renamed to `wnd_proc`.
- Added `tools/check_rw_prefix_is_abi_only.sh`: `rw_` belongs to the ABI boundary
  (`src/bindings/`), where a flat C namespace makes a prefix necessary. Rust does not need
  it — a module path already namespaces — so the gate fails on any new `rw_*`
  **definition** outside that directory, and on any ABI export outside `src/bindings/`
  and the JNI bridge. One documented exception: `RwError`/`RwResult`, which are settled
  public API (documented in `api-reference.md`, 35 call sites); renaming them would be a
  breaking change for a naming preference.

  The check is deliberately narrowed to *definition position* rather than every token.
  A "no `rw_` anywhere" rule produced 38 hits of which most were **correct** — doc comments
  naming ABI entry points, and test temp-path prefixes like `/tmp/rw_spool_probe_*` whose
  purpose is to be unlikely to collide. A gate with a 34-item allowlist gets allowlisted
  into uselessness.

### Verified in this release

`4770` lib tests pass on `desktop` (`+14` for the repaint auto-decision), `1538` on
`embedded`, `1477` on `mini`; `clippy --all-targets -- -D warnings` and
`cargo doc --no-deps` are both clean; all 5 profiles build; 30 gates run,
with 4 host-gated or pre-existing (documented in `docs/log/log-20260917-4.md` §8.3).

Every new assertion in this release was checked by **reverse injection** — the change it
guards was removed and the test observed to fail — because a gate that cannot fail is not
a gate. See `docs/log/log-20260917-4.md` §8.4.

## 2.2.0 (2026-09-17) — Reachability: Everything Implemented Is Now Constructible

Backward compatible. Twelve C ABI functions were added (106 → 118); no existing signature
changed. Eleven controls that were implemented but unconstructible by name are now
registered, and every language binding reaches the whole published ABI.

The theme of this release is **reachability**: controls, properties and modules that were
fully implemented but had no way to be reached — by name, over the C ABI, or by any
caller at all. Each gap was invisible because nothing failed; the code simply was not
there for the people it was written for.

### Added

- **Twelve C ABI entry points** (106 → 118 functions):
  - Generic creation and reflection: `rw_create_widget_of_kind`, `rw_widget_kind_names`,
    `rw_get_widget_property`, `rw_set_widget_property`, `rw_widget_property_names`.
  - Collections, which have no settable count: `rw_widget_list_add`,
    `rw_widget_list_clear`, `rw_widget_list_count`.
  - Scrolling, which is an action rather than an assignment:
    `rw_widget_set_scroll_position`, `rw_widget_scroll_to`.
  - Theme: `rw_set_theme`, `rw_theme_names`, `rw_set_high_contrast`.
- **Eleven newly registered controls.** Seven already existed with complete
  implementations (`timeline_widget`, `command_palette`, `notification_center`,
  `diff_viewer`, `markdown_editor`, `toast_stack`, `grid_table`) and were merely never
  wired into the factory. Four are new: `number_picker`, `otp_input`, `banner`,
  `pagination`. Factory names: 155 → **166**. `WidgetKind`: 167 → **171**.

### Fixed

- `include/rw_generated.h` was four functions behind the ABI, including the only
  destructor `rw_destroy_widget`.
- Every language binding was missing `rw_destroy_widget`.
- Eighteen schema-declared properties were answered by no contract; `canvas` was serving
  the *map view* schema, and `chart` a marker concept it never had.
- `rw_errors.h` and `rw_generated.h` disagreed about `rw_error_message`'s return type, so
  a translation unit including both could not compile.
- `src/embedded` (1,861 lines) was a forwarding layer over `platform::profile`, which is
  the layer with real consumers; the layer itself had none.

### Gates

Four new checks, each verified to fail before being trusted:
`check_binding_symbol_coverage.sh`, `check_widget_registration_fidelity.sh`,
`check_module_reachability.sh`, and a bidirectional schema↔contract test. The last of
those is what found the eighteen phantom properties.

## 2.1.0 (2026-09-17) — HarmonyOS Made Real, Error Messages Audited, Cross-Target `--all-targets` Fixed

Makes the cross-target claims falsifiable. No public API changed and no capability was
added or removed: every entry below is code that already claimed to work and did not.

See [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md) for the full list.

### Fixed

- **The OpenHarmony SDK is on this host but the targets were never installed**, so
  "HarmonyOS passes" had never been observed. `rustup target add` for
  `aarch64`/`armv7`/`x86_64-unknown-linux-ohos` was the missing step; all three now
  build **and link** against the SDK sysroot, with the produced `.so` machine type
  verified (`AArch64`, `ARM`, `X86-64`), and clippy is clean under `-D warnings`.
  `loongarch64` remains unbuildable (Tier 3, no prebuilt std, no libc for that arch in
  the SDK) and the gate pins that specific outcome rather than pretending either way.

- **`cargo check --target wasm32-unknown-unknown --all-targets` did not compile.**
  A bench target has no wasm build (criterion is a host-only dev-dependency), but the
  bench *targets* still existed, so five benches failed with `E0601` (`main` function
  not found) — `wasm32 --all-targets` had never been run. Each bench is now gated by
  item instead of by a crate-level `#![cfg]`, which is what had been removing the
  `criterion_main!`-generated `main` along with everything else.

- **`src/platform/os_probes.rs` did not compile off unix/windows.** The test
  `print_job_waits_for_the_spooler_before_reading_back` called `stand_in_spooler`,
  which is `#[cfg(any(unix, windows))]`; the test itself was ungated. The gate now sits
  on the test, where the property it asserts is also the gate for the helper.

- **`wasm32 --all-targets` also emitted an `unused_imports` warning** for `AtomicBool`
  in `src/event/loop.rs`: its only uses are the native-pump tests, which are themselves
  wasm-gated.

- **206 error messages were not actionable** (`tools/check_error_messages.py`, the
  `TODO.md` item "all error messages are user-friendly and actionable"). Each now names
  the specific input that failed **and** states the expected form — e.g.
  `PNG is 4096x4096 (16777216 pixels), which exceeds the 134217728 pixel cap; downscale
  the image before decoding`. The scanner itself had a real defect: its string-literal
  regex stopped at the `'` inside `"muxer '{name}' could not be created"`, so the message
  was reported as the fragment `muxer ` — which then looked like it named nothing.
  Fixing both took the report from **206 findings to 0**.

- **Two doc-lint failures blocked the build** (`missing documentation` for
  `MacOSAccessibilityBridge::new` and `MacOsClipboard`), reintroduced at some point after
  the `#![deny(missing_docs)]` pass.

### Verification

`cargo test`: **desktop 4127 · embedded 1490 · mini 1411, 0 failed**. `cargo fmt --check`
clean; `cargo clippy --all-features --all-targets -- -D warnings` clean. Cross targets
build with 0 warnings: `wasm32-unknown-unknown --all-targets`,
`x86_64-pc-windows-gnu --all-targets`, `aarch64-apple-ios`, `aarch64-apple-ios-sim
--all-targets`, and the three linkable OpenHarmony triples.

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
