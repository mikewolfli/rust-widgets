<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# iOS Backend Status

## Current Status

The iOS backend (`src/platform/ios/`) is a **state-driven** backend. It
implements the `Platform` contract — widget creation, geometry, text,
visibility, enablement, menu tree, list/combo data, clipboard, drag/drop, IME
and accessibility metadata — through `BackendState<IosHandleKind>`.

The one thing the host owes the library is a **window** to paint into. That is
the single UIKit object this backend creates (`native::create_ui_window` in
`src/platform/ios/native.rs`), and it is gated behind
`#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]`. Elsewhere the
backend runs in pure state mode.

## BLUE15: no per-kind UIKit controls

The backend used to instantiate a real `UIView` per logical widget —
`UIButton`, `UILabel`, `UISwitch`, `UITextField`, `UIPickerView`,
`UITableView`, `UIProgressView`, `UISlider`, `UIScrollView`,
`UIAlertController`, `UIStackView`, `UIActivityIndicatorView` — and mirror every
state mutation into it through `native::*` helpers (`store_native_view`,
`add_as_subview`, `wire_button_action`, `set_native_text`, `set_native_frame`,
`set_native_hidden`, `set_native_enabled`).

Under the self-drawn strategy (BLUE15 #55/#56) the library paints every
`WidgetKind`, so the host supplies a **window plus a drawing surface** and
nothing per-kind. The control creators, the view registry, the `ButtonTarget`
Objective-C class and its event queue existed only to serve that path and have
been **deleted** (#59: delete means delete).

The host attaches its view with `MobilePlatformExtension::attach_to_native_view`,
and widgets are then displayed through `Platform::mount_surface` — see below.

## What is implemented

| Area | Status | Notes |
|---|---|---|
| `Platform` contract (`create_window` + full window contract) | ✅ Implemented | state-backed, with parent/kind validation |
| `UIWindow` creation | ✅ Implemented | real `UIWindow` with a root view controller, visible |
| Per-kind UIKit controls (`UIButton` / `UILabel` / …) | ⛔ Deleted | BLUE15 #59 — the library paints them |
| **Widget surfaces** (`mount_surface` + repaint queue) | ✅ Implemented | records which widgets are displayed and returns their frames |
| Menu tree (MenuBar/Menu/MenuItem) | ✅ Implemented | in-process tree + injectable trigger queue |
| ComboBox / ListBox data paths | ✅ Implemented | shared list-data tables |
| Show / hide / geometry / text / enabled | ✅ Implemented | logical state round-trips |
| Clipboard | ✅ Implemented | in-process store |
| Drag & drop | ✅ Implemented | injectable drop-event queue |
| IME + accessibility metadata | ✅ Implemented | modelled state |
| Print facts | ✅ Implemented | honest error: `UIPrintInteractionController` is not bound |
| **Input delivery into widgets** | ⬜ Not wired | the host must forward its touch/key events; see below |

### How a widget reaches the screen

The host owns the pixels; the library hands them over as a frame.

```text
1. host:  attach_to_native_view(ui_view_handle)
2. host:  mount_surface(parent, widget_id, rect)   -> true
3. library: invalidate_surface(widget_id)          -> queued (coalesced)
4. host:  take_pending_repaint()                   -> Some(widget_id)
5. host:  render_frame(widget_id, size, clear)     -> RGBA, blit it
6. host:  unmount_surface(widget_id) on teardown
```

In step 3-4 the queue is the backend's, so the host learns *which* widget went
stale rather than repainting everything. Pinned by
`ios_hosts_widget_surfaces_and_queues_repaints`.

### Input is not yet delivered into widgets

`mount_surface` makes a widget **visible**; it does not make it **interactive**.
A UIKit host must translate its touches and keys and forward them, e.g. through
`crate::widget::runtime::dispatch_pointer_event(root, event, point)` (which also
drives focus, hover and pointer capture) or `dispatch_event(id, event)` when the
target is already known. Until that wiring exists in the host layer, this backend
paints but does not react.

## Capabilities (honest contract)

`IosMobilePlatform::capabilities()` declares the flags explicitly rather than
inheriting desktop defaults:

- `dpi_scaling: true`, `ime: true`, `accessibility: true` — the state model
  tracks these.
- `native_menu: false` — the menu is an in-process tree served through an
  injectable queue, **not** an OS menu. This is asserted by
  `ios_platform_reports_explicit_mobile_capabilities`.

## Build and test

```bash
# Host (feature-gated preview backend; UIKit code is compiled out)
cargo test --lib --no-default-features --features "mobile-api" platform::ios

# iOS target cross-compile (installs: rustup target add aarch64-apple-ios-sim)
cargo check --target aarch64-apple-ios-sim --no-default-features \
  --features "ios ios-uikit-ffi"
```

## Honest boundaries

Client-verified facts for this revision:

- The **host** test suite runs the state backend and is green on every host
  (`cargo test --no-default-features --features desktop --lib`).
- The iOS-target compile checks and the Simulator probe outputs that earlier
  revisions of this file recorded (`tools/run_ios_testapp.sh`) were taken
  **before** the BLUE15 control deletion and have **not** been re-run: the
  development host is macOS with no iOS SDK/toolchain in this workspace, so the
  `ios-uikit-ffi` build and the Simulator probe are **unverified** for this
  revision. Re-running `tools/run_ios_testapp.sh` on a Mac with Xcode is the
  required next step.
- `create_ui_window` uses `initWithFrame:` (scene-less) because the library has
  no access to a `UIWindowScene` instance; `initWithWindowScene:` would be the
  modern path once a scene is threaded through by the host app.
- The library ships no AndroidX-equivalent framework dependency; UIKit is the
  system framework, so no bundling is required.
