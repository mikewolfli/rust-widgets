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
| IME + accessibility metadata | ✅ Implemented | modelled state |
| Typed widget-trigger events | ✅ Implemented | delegated to the shared `BackendState` queue |
| Backend auto-selection on `ohos` target | ✅ Implemented | via `target_env`, see above |
| **Widget surfaces** (`mount_surface` + repaint queue) | ✅ Implemented | records which widgets are displayed; the ArkTS side pulls frames |
| ArkUI native view bridge | ⬜ Not implemented | needs N-API/ArkUI headers bound at runtime |
| **Input delivery into widgets** | ⬜ Not wired | the ArkTS side must forward its events; see below |

## How a widget reaches the screen

The ArkTS host owns the pixels; the library hands them over as a frame.

```text
1. ArkTS: rw_harmony_bind_node(node_handle, widget_id)
2. ArkTS: rw_mount_surface(parent, widget_id, rect)   -> true
3. library: invalidate_surface(widget_id)             -> queued (coalesced)
4. ArkTS: take the pending repaint, then render the frame
5. ArkTS: blit the RGBA into its Canvas
6. ArkTS: unmount on teardown
```

Because the queue lives in the backend, the host learns *which* widget went stale
instead of repainting everything. Pinned by
`widget_surfaces_are_advertised_and_round_trip`.

`mount_surface` makes a widget **visible**; it does not make it **interactive**.
Forwarding ArkTS touches/keys into `crate::widget::runtime::dispatch_pointer_event`
(or `dispatch_event` for a known id) is still to be wired, which is why the
`rw_harmony_on_*` entry points exist.

## Capabilities (honest contract)

`HarmonyPlatform::capabilities()` declares the flags explicitly rather than
inheriting desktop defaults:

- `dpi_scaling: true`, `ime: true`, `accessibility: true` — the state model
  tracks these.
- `native_menu: false` — the menu is an in-process tree served through an
  injectable queue, **not** an OS menu. This is asserted by
  `capabilities_are_explicit_and_honest`.

`supports_surfaces()` returns `true`: the backend records mounted surfaces and
queues repaints for the host to drain, which is everything a host needs to put a
widget on screen. It previously returned `false` because `mount_surface` was
unimplemented; that is now closed, and the earlier assertion of the gap was
rewritten into an assertion of the capability (see
`widget_surfaces_are_advertised_and_round_trip`).

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
