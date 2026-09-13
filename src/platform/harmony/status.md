<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# HarmonyOS Backend Status

## Current Status

The HarmonyOS backend (`src/platform/harmony/`) is a **state-driven** backend.
It implements the full `Platform` contract — widget creation, geometry, text,
visibility, enablement, menu tree, list data, clipboard, drag/drop, IME and
accessibility metadata — entirely in-process through
`BackendState<HarmonyHandleKind>`.

No ArkUI object is created: a real Harmony native bridge needs the OpenHarmony
SDK and its N-API/ArkUI headers, which are not part of this repository or its
build environment.

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
| ArkUI native view bridge | ⬜ Not implemented | requires OpenHarmony SDK (N-API/ArkUI headers) |

## Capabilities (honest contract)

`HarmonyPlatform::capabilities()` declares the flags explicitly rather than
inheriting desktop defaults:

- `dpi_scaling: true`, `ime: true`, `accessibility: true` — the state model
  tracks these.
- `native_menu: false` — the menu is an in-process tree served through an
  injectable queue, **not** an OS menu. This is asserted by
  `capabilities_are_explicit_and_honest`.

## Build and test

```bash
# Host (feature-gated preview backend)
cargo test --lib --no-default-features --features "harmony" platform::harmony

# OpenHarmony target cross-compile (installs: rustup target add aarch64-unknown-linux-ohos)
cargo check --target aarch64-unknown-linux-ohos --no-default-features \
  --features "harmony controls-custom controls-native serde serde_json"
```

Verified: the `aarch64-unknown-linux-ohos` target compiles with 0 warnings, and
12 host tests cover the widget lifecycle, menu tree, dialogs, extended controls
and the capability contract.

## Next Steps

1. Obtain the OpenHarmony SDK (ArkUI N-API headers) and add an
   `ohos-native` feature that constructs real ArkUI components.
2. Wire the native window/event loop to the `ohos` lifecycle callbacks.
3. Add the Harmony target to CI once an SDK is available.
