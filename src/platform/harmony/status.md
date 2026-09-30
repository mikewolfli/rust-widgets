<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# HarmonyOS Backend Status

## Current Status

The HarmonyOS backend (`src/platform/harmony/`) is a **state-driven** backend.
It implements the full `Platform` contract — widget creation, geometry, text,
visibility, enablement, menu tree, list data, clipboard, drag/drop, IME and
accessibility metadata — entirely in-process through
`BackendState<HarmonyHandleKind>`.

No ArkUI object is created. The library paints every `WidgetKind` itself
(`src/widget/`), so a native ArkUI control has no role. What the backend owes a
widget is a surface, and it supplies one: `mount_surface` records that the widget
is being displayed and queues a repaint the ArkTS host drains to fetch the frame.
The host is what puts pixels on screen; what is still missing is the reverse
direction — ArkTS input events are not yet forwarded into the widgets (see below).

## How the backend is selected

This is the part that is easy to get wrong, and was wrong before the OpenHarmony
SDK made it testable.

| | Discriminator | Value |
|---|---|---|
| OpenHarmony target | `target_env` | `"ohos"` |
| OpenHarmony target | `target_os` | **`"linux"`** |
| OpenHarmony target | `target_family` | `"unix"` |

Every `*-unknown-linux-ohos` target reports `target_os = "linux"`. A backend
selected with `cfg(target_os = "ohos")` therefore **never** matches, and the
build silently fails to select any backend (or, worse, picks the GTK-only
`LinuxPlatform`, which cannot exist on OpenHarmony).

The single source of truth is
[`crate::platform::profile::is_openharmony_target`](../profile.rs), and the
selection in `src/platform/runtime.rs` uses `target_env = "ohos"`. The `harmony`
Cargo feature remains the way to select the backend on a *non*-OpenHarmony host
for development and testing.

## What is implemented

| Area | Status | Notes |
|---|---|---|
| `Platform` contract (all `create_*`) | ✅ Implemented | state-backed, with parent/kind validation |
| Menu tree (MenuBar/Menu/MenuItem) | ✅ Implemented | in-process tree + injectable trigger queue |
| ComboBox / ListBox data paths | ✅ Implemented | shared list-data table |
| Show / hide / geometry / text / enabled | ✅ Implemented | logical state round-trips |
| Clipboard | ✅ Implemented | in-process store |
| Drag & drop | ✅ Implemented | injectable drop-event queue |
| IME / accessibility **metadata** (per-widget flags and names) | ✅ Implemented | `BackendState` fields that round-trip a boolean and a string |
| IME / accessibility **bridges** | ⛔ Not implemented | neither `ime_bridge()` nor `accessibility_bridge()` is overridden, so both answer `None`; this is what `capabilities()` reports as `ime: false` / `accessibility: false`. The row above is not a counter-argument — a flag store is not an input-method client, and ArkUI's real IME arrives through the XComponent below, which is not bound |
| Typed widget-trigger events | ✅ Implemented | delegated to the shared `BackendState` queue |
| Backend auto-selection on `ohos` target | ✅ Implemented | via `target_env`, see above |
| **Widget surfaces** (`mount_surface` + repaint queue) | ✅ Implemented | records which widgets are displayed; the ArkTS side pulls frames |
| **ArkUI XComponent bridge** (`feature = "xcomponent"`) | ✅ Implemented | binds `OH_NativeXComponent`; see below |
| **Input delivery into widgets** | ✅ Implemented via the bridge | touch / mouse / key / focus callbacks route into the widget tree |

## The XComponent bridge

`src/platform/harmony/xcomponent.rs` closes the two gaps this table used to record as
"⬜ Not implemented" and "⬜ Not wired" — they were one gap, because without an
`OH_NativeXComponent` the backend could neither receive a surface nor an event.

ArkUI's `XComponent` hands its native side an `OH_NativeXComponent*`; the ArkTS `onLoad` passes
that pointer to `rw_harmony_bind_xcomponent`, and from there the bridge registers:

| Native entry point | Callback | What the library gains |
|---|---|---|
| `OH_NativeXComponent_RegisterCallback` | `OnSurfaceCreated` / `Changed` / `Destroyed` | a surface to draw into, and its size and offset |
| the same block, `DispatchTouchEvent` | `OH_NativeXComponent_GetTouchEvent` | multi-contact input → `Event::Touch*` |
| `OH_NativeXComponent_RegisterMouseEventCallback` | `DispatchMouseEvent` / `DispatchHoverEvent` | click, move, hover → `Event::Mouse*` |
| `OH_NativeXComponent_RegisterKeyEventCallback` | key down | typing → `Event::KeyPress` |
| `OH_NativeXComponent_RegisterFocusEventCallback` / `Blur` | focus / blur | the library's focus model |

Design points worth knowing before editing it:

- **Hand-written `extern "C"`, not bindgen.** The declarations are a few dozen lines and this
  crate has no bindgen dependency. Each function records the header's own `@since`, and the
  module documents the SDK version it was transcribed from
  (OpenHarmony 6.0.0.46 Beta1).
- **Enums are mapped explicitly, never `transmute`d.** `TouchEventType` and
  `MouseEventAction` are `repr(C)` over SDK enums, so an out-of-range value reaching a
  `transmute` would be undefined behaviour. `from_raw` keeps an `Unknown`/`None` arm for it.
- **`touch` is orthogonal.** `Event::Touch*` is itself gated on `feature = "touch"`, so the
  bridge's touch arms carry the same gate and drop contacts at `debug` when it is off —
  identical to how the Windows canvas treats `WM_TOUCH`.
- **A thread guard refuses foreign callbacks.** The widget runtime is thread-local and ArkUI
  calls back on its own main thread; a callback from elsewhere would dispatch into a
  *different* registry and the events would vanish with no error, so it is refused and logged.
- **`on_surface_changed` does not run layout.** It queues a resize trigger and lets the
  message loop re-run the layout — re-entering a control tree's layout from inside ArkUI's own
  layout pass is how a resize becomes a stall.
- **Off by default.** `xcomponent` links `ace_ndk` and needs the SDK's headers, so a build
  without the SDK must still compile; that is principle #37's "honest absence" applied to a
  build configuration. `rw_harmony_bind_xcomponent` returns `false` with a logged reason in
  such a build, rather than failing to link.

Verified with the SDK's real headers:

```bash
OHOS_SDK_NATIVE=<sdk>/linux/native cargo ohos check -t aarch64 --lib \
  --no-default-features --features "harmony xcomponent desktop-runtime controls-custom"
OHOS_SDK_NATIVE=<sdk>/linux/native cargo ohos check -t aarch64 --lib \
  --no-default-features --features "harmony xcomponent touch i18n controls-native desktop-runtime"
```

Both pass, and `cargo check --lib --features "harmony xcomponent desktop-runtime"` on the host
is warning-free.

## How a widget reaches the screen

The ArkTS host owns the pixels; the library hands them over as a frame.

```text
1. ArkTS: rw_harmony_bind_node(node_handle, widget_id)
2. ArkTS: rw_mount_surface(parent, widget_id, x, y, w, h)   -> true
3. library: invalidate_surface(widget_id)                    -> queued (coalesced)
4. ArkTS: id = rw_take_pending_repaint()                     -> the stale widget
5. ArkTS: rw_render_surface_frame(id, w, h, ...)             -> RGBA bytes
6. ArkTS: blit the RGBA into its Canvas, then rw_free_bytes(...)
7. ArkTS: rw_unmount_surface(widget_id) on teardown
```

Because the queue lives in the backend, the host learns *which* widget went stale
instead of repainting everything. Pinned by
`widget_surfaces_are_advertised_and_round_trip`.

Steps 2 and 4–6 cross the C ABI, and **before this round they had no entry point**:
`Platform::mount_surface`/`invalidate_surface` are Rust trait methods, and no
function produced frame pixels at all, so a host holding only the C ABI could not
display anything while this backend reported `supports_surfaces() == true`. The
surface functions now exist (`rw_mount_surface`, `rw_resize_surface`,
`rw_unmount_surface`, `rw_invalidate_surface`, `rw_supports_surfaces`,
`rw_take_pending_repaint`, `rw_render_surface_frame`) and are published in
`include/rw_generated.h`, so the flow above is executable rather than aspirational.

`rw_report_window_resize(window_id, w, h)` is the seventh piece: OpenHarmony
delivers a size change to the ArkTS component's `onAreaChange`, not to this backend,
so without forwarding it here the window's layout never re-runs after a resize.

`mount_surface` makes a widget **visible**; it does not make it **interactive**.
Forwarding ArkTS touches/keys into `crate::widget::runtime::dispatch_pointer_event`
(or `dispatch_event` for a known id) is still to be wired, which is why the
`rw_harmony_on_*` entry points exist.

## Capabilities (honest contract)

`HarmonyPlatform::capabilities()` declares the flags explicitly rather than
inheriting desktop defaults:

- `dpi_scaling: false`, `ime: false`, `accessibility: false` — **not** because
  OpenHarmony lacks these, but because this backend implements none of the methods
  behind them. Each flag names a specific method (`dpi_scale_factor()`,
  `ime_bridge()`, `accessibility_bridge()`), and `grep` over `src/platform/harmony/`
  finds all three absent, so each would fall through to the trait default (`1.0`,
  `None`, `None`). A `true` here promised a method that cannot answer. ArkUI *does*
  expose display density, IME and an accessibility tree — they become reachable when
  the XComponent bridge below is bound, and these three flags flip to `true` at that
  point, not before.
- `native_menu: false` — the menu is an in-process tree served through an injectable
  queue, **not** an OS menu. This is asserted by `capabilities_are_explicit_and_honest`.
- `typed_widget_trigger: true` — implemented by this backend
  (`inject_widget_trigger_event`, `poll_widget_trigger_event`) over the shared queue,
  so it cannot be absent.

`supports_surfaces()` returns `true`: the backend records mounted surfaces and
queues repaints for the host to drain. That claim is now reachable from outside the
process, which it was not before — see "How a widget reaches the screen".

## Build and test

### Host (feature-gated preview backend)

```bash
cargo test --lib --no-default-features --features "harmony" platform::harmony
```

### OpenHarmony cross-compile

Requires the OpenHarmony SDK and [`cargo-ohos`](https://crates.io/crates/cargo-ohos),
which computes the cross environment (sysroot, per-target clang, cc-rs / bindgen /
pkg-config flags, `-D__MUSL__`) so that **C dependencies compile against the
OpenHarmony sysroot instead of the host's headers**.

```bash
cargo install cargo-ohos --locked
export OHOS_SDK_NATIVE=/path/to/ohos-sdk/linux/native   # the SDK's `native` dir

# Backend auto-selected from the target — no `harmony` feature needed.
cargo ohos build -t aarch64 --no-default-features \
  --features "desktop,touch,i18n,serde,serde_json"
```

The `-t` short names are `aarch64`, `armv7`, `x86_64` (and `loongarch64`, see
below); the full triples are also accepted.

Verified: `aarch64`, `armv7` and `x86_64` each **build and link** a real
`librust_widgets.so` whose ELF machine type matches the triple, with 0 warnings;
`cargo ohos clippy --all-targets -- -D warnings` is clean; and the host test
suite covers the widget lifecycle, menu tree, dialogs, extended controls, trigger
events and the capability contract.

**Two failure modes worth knowing**, both of which cost real debugging time:

1. **`cargo check` never links.** It type-checks only, so it cannot catch a
   missing sysroot — the C dependencies in the graph (`minimp3-sys`, …) only fail
   when they are actually compiled. Use `cargo ohos build`.
2. **Setting just `CARGO_TARGET_*_LINKER` is not enough.** That leaves C
   compilations using the *host* headers (no sysroot, no `-D__MUSL__`), and the
   resulting `--target` mismatch surfaces as
   `Relocations in generic ELF (EM: 183)` / `file in wrong format`, or as
   `bits/alltypes.h: file not found`. Let `cargo ohos` set the whole environment.

### Target support

| Triple | Status |
|---|---|
| `aarch64-unknown-linux-ohos` | ✅ builds and links (Tier 2) |
| `armv7-unknown-linux-ohos` | ✅ builds and links (Tier 2) |
| `x86_64-unknown-linux-ohos` | ✅ builds and links (Tier 2) |
| `loongarch64-unknown-linux-ohos` | ❌ cannot build with this SDK |

`loongarch64` is blocked for two independent reasons, neither of them this
crate's: rustup ships **no prebuilt std** (it is a Tier 3 target — “official
builds are not available”), and the SDK sysroot ships **no loongarch64 libc**, so
its C headers are incomplete (`bits/alltypes.h` is absent). Even past that, the
link would fail for want of `crti.o` / `-lc`. `cargo-ohos` documents the same
limitation independently: *“`loongarch64` is not supported in the latest SDK
(CMake) at the time of writing.”* `tools/check_harmony_cross.sh` asserts this
state positively, so if upstream ever fixes it the gate asks to be updated.

## Next Steps

1. Bind the ArkUI `Canvas` through N-API so the ArkTS side can act on the repaint
   queue directly, and deliver its touches/keys into
   `widget::runtime::dispatch_pointer_event` / `dispatch_event`.
2. Wire the native window/event loop to the `ohos` lifecycle callbacks; the
   ArkTS side drives the loop and polls `rw_poll_*` (see
   `docs/plans/harmony_integration.md`).
3. The cross-compile job in `.github/workflows/ci.yml`
   (`harmony-cross-check`) keeps the backend-selection contract from regressing.
