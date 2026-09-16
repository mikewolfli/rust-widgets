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
(`src/widget/`), so a native ArkUI control has no role; what the backend still
cannot do is present that painting on screen, because the ArkUI `Canvas` bridge
is not bound (see below).

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
| ArkUI native view bridge | ⬜ Not implemented | needs N-API/ArkUI headers bound at runtime |

## Capabilities (honest contract)

`HarmonyPlatform::capabilities()` declares the flags explicitly rather than
inheriting desktop defaults:

- `dpi_scaling: true`, `ime: true`, `accessibility: true` — the state model
  tracks these.
- `native_menu: false` — the menu is an in-process tree served through an
  injectable queue, **not** an OS menu. This is asserted by
  `capabilities_are_explicit_and_honest`.

`supports_surfaces()` returns `false`: there is no ArkUI `Canvas` to paint into
yet, so a host that asks is told the truth rather than shown an empty window.
This is pinned by `widget_surface_support_is_refused_until_the_arkui_bridge_exists`.

## Build and test

### Host (feature-gated preview backend)

```bash
cargo test --lib --no-default-features --features "harmony" platform::harmony
```

### OpenHarmony cross-compile

Requires the OpenHarmony SDK's native toolchain (clang + sysroot). Point
`OHOS_SDK` at the SDK's host directory (the one containing `native/`):

```bash
export OHOS_SDK=/path/to/ohos-sdk/linux
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER=\
"$OHOS_SDK/native/llvm/bin/aarch64-unknown-linux-ohos-clang"

# Backend auto-selected from the target — no `harmony` feature needed.
cargo check --target aarch64-unknown-linux-ohos \
  --no-default-features --features "desktop,touch,i18n,serde,serde_json"

# Preview feature on a non-OpenHarmony host, for the same backend in tests.
cargo check --target aarch64-unknown-linux-ohos \
  --no-default-features --features "desktop,harmony"
```

Verified: the `aarch64-unknown-linux-ohos` target compiles (0 warnings,
0 errors) both with the backend auto-selected and with the `harmony` feature;
`cargo clippy --all-targets -- -D warnings` is clean on that target; and 10 host
tests cover the widget lifecycle, menu tree, dialogs, extended controls,
trigger events and the capability contract.

Without the SDK linker configured the build fails at the link step with
`Relocations in generic ELF (EM: 183)` / `file in wrong format` — the host
`ld` cannot read OpenHarmony objects. That is a toolchain configuration error,
not a source error.

## Next Steps

1. Bind the ArkUI `Canvas` through N-API and implement `mount_surface` /
   `invalidate_surface` / `unmount_surface`, then flip `supports_surfaces()` to
   report the real capability.
2. Wire the native window/event loop to the `ohos` lifecycle callbacks; the
   ArkTS side drives the loop and polls `rw_poll_*` (see
   `docs/plans/harmony_integration.md`).
3. The cross-compile job in `.github/workflows/ci.yml`
   (`harmony-cross-check`) keeps the backend-selection contract from regressing.
