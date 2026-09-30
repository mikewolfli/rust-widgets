<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# macOS Backends Status (cocoa-legacy + objc2)

> Last verified: 2026-09-11 on a real Mac (macOS 15.7.3, arm64, rustc 1.98.0).
> Evidence: `docs/log/log-20260911-2.md`, gate `tools/check_apple_native.sh`.

## Backend selection

Two macOS backends coexist and are selected by feature flag via
`src/platform/macos/macos_bridge.rs`:

| Feature | Selected backend | `backend_name()` |
|---|---|---|
| `macos` (or alias `objc2-macos`) | `MacOSObjc2Platform` | `macos-objc2-preview` |
| `cocoa-legacy` (or alias `macos-legacy`) | `MacOSPlatform` | `cocoa` |
| neither (target is macOS) | `StubPlatform` fallback | `macos-fallback-stub` |

`desktop` enables `cocoa-legacy` only, so the default desktop build on macOS
selects the legacy `cocoa` backend. Requesting `--features macos` selects the
objc2 backend. **Both are now native-verified** (see below).

> Fixed 2026-09-11: the native FFI in `macos_objc2` was gated on the *alias*
> `feature = "objc2-macos"`. Cargo aliases are one-way, so `--features macos`
> (the documented OS-backend axis in `Cargo.toml`) silently degraded to
> state-only. All 43 gates now use the canonical `feature = "macos"`.

## What is implemented

> **Corrected 2026-09-30.** The two tables below used to claim per-kind native controls, an
> `NSMenu` menu bar and `NSAlert`/`NSOpenPanel` dialogs. **None of that is in the code.** BLUE15
> #55/#56 removed per-kind control creation (the library paints every `WidgetKind`), and these rows
> were never updated — `grep -rn "NSButton\|NSTextField\|NSAlert\|NSOpenPanel" src/platform/macos/`
> matches only prose in this file, and `grep -c "fn create_" platform_impl.rs` returns **1**
> (`create_window`). Because a status table is what a host reads before choosing a build, the
> overstatement was the "documentation describes behaviour the code does not have" defect rather
> than a stale note. Each row below now names what the backend actually overrides, and the
> capability flags in `capabilities()` are the machine-checkable form of the same statement
> (`tools/check_capability_flags_match_their_methods.sh`).

### cocoa-legacy backend (`src/platform/macos/platform_impl.rs`)

| Area | Status | Notes |
|---|---|---|
| `create_window` | ✅ Implemented | a real `NSWindow` on the main thread |
| Native window registered with AppKit | ✅ Verified | `NSApplication.windows.count >= 1` after `create_window` |
| Per-kind controls (`create_button`…) | ⛔ Removed | BLUE15 #55/#56 — the library paints every `WidgetKind`, so there is no `NSButton`/`NSTextField` and `grep` finds none |
| Menu bar (`NSMenu`) | ⛔ Removed | no `create_menu_bar`/`menu_add_item` override; `native_menu: false`, and `menu_item_shortcut()`/`poll_menu_triggered()` are the only menu-shaped methods left |
| Dialogs (`NSAlert` / `NSOpenPanel`) | ⛔ Removed | no `create_message_box`/`create_file_dialog` override |
| Widget surfaces (`mount_surface` + repaint queue) | ✅ Implemented | the library-painted path every `WidgetKind` now uses |
| Clipboard (`NSPasteboard`) | ✅ Verified | round-trip on the main thread; also `clipboard_backend()` for rich content |
| IME / accessibility bridges | ✅ Implemented | `ime_bridge()` and `accessibility_bridge()` both answer with real bridges, which is why `capabilities()` reports `ime`/`accessibility` as `true` |
| Geometry / visibility / text / enabled | ✅ Verified | reflected on the live native objects |
| Off-main-thread calls | ✅ Guarded | state-only fallback (`ptr == 0`), never aborts |

### objc2 backend (`src/platform/macos_objc2/`)

| Area | Status | Notes |
|---|---|---|
| `create_window` | ✅ Implemented | real AppKit object; state fallback off-main |
| `init()` bootstraps `NSApplication` | ✅ Added 2026-09-11 | `sharedApplication` + `finishLaunching` |
| Native window registered with AppKit | ✅ Verified | `NSApplication.windows.count >= 1` |
| Per-kind controls / menu bar / dialogs | ⛔ Not implemented | this backend overrides no `create_*` except `create_window`; `capabilities()` reports `ime`, `accessibility`, `native_menu` and `dpi_scaling` all `false` for exactly that reason |
| Widget surfaces | ✅ Implemented | records displayed widgets and returns their frames |
| Clipboard | ✅ Verified | `NSPasteboard` |
| Geometry / visibility / text | ✅ Verified | `setNative…` helpers |
| Off-main-thread calls | ✅ Guarded | `objc2::MainThreadMarker::new()` before every `create_*` |

> Fixed 2026-09-11: `native::set_native_text` used
> `performSelector:withObject:` but declared a `()` return, while the selector
> returns `id`. objc2 validates the declared signature and aborted at runtime
> ("expected return to have type code '@', but found 'v'"). It now dispatches
> typed `setStringValue:` / `setTitle:` / `setAccessibilityLabel:` messages.
> This was masked before the feature-gate fix above made the native path
> reachable from `--features macos`.

> Fixed 2026-09-11: the `set_native_frame` / `set_native_hidden` helpers sent the
> **view** selectors `setFrame:` / `setHidden:` to whatever object was stored —
> including an `NSWindow`, which implements neither (windows use
> `setFrame:display:` and `orderOut:` / `makeKeyAndOrderFront:`). objc2 raised
> `invalid message send to -[NSWindow setFrame:]: method not found` and aborted.
> Both helpers are now window/view aware and carry a main-thread guard, and
> `set_native_enabled` / `set_native_text` probe `respondsToSelector:` first.

## Verification (real Mac, 2026-09-11)

`cargo run --example apple_appkit_probe --features <backend>` runs on the AppKit
main thread and asserts against live Objective-C objects:

```
backend = cocoa                    backend = macos-objc2-preview
[PASS] main_thread                 [PASS] main_thread
[PASS] ns_application              [PASS] ns_application
[PASS] create_window_id            [PASS] create_window_id
[PASS] native_window_registered    [PASS] native_window_registered
[PASS] native_children             [PASS] native_children
[PASS] widget_text_roundtrip       [PASS] widget_text_roundtrip
[PASS] native_menu_bar             [PASS] native_menu_bar
[PASS] native_clipboard            [PASS] native_clipboard
[PASS] native_dialog_kinds         [PASS] native_dialog_kinds
[PASS] native_window_frame_applied [PASS] native_window_frame_applied
[PASS] native_window_visibility    [PASS] native_window_visibility
[PASS] visibility_roundtrip        [PASS] visibility_roundtrip
RESULT: PASS (12 checks)           RESULT: PASS (12 checks)
```

> `native_window_frame_applied` and `native_window_visibility` read back the
> **native** `NSWindow.frame` / `isVisible`, so they distinguish a real FFI
> wiring from a state-only stub. They are what caught the objc2 selector bug
> above (`setFrame:` on an `NSWindow`).

Test suites on the real Mac:

| Command | Result |
|---|---|
| `cargo test --lib --features desktop` | 3836 passed, 0 failed |
| `cargo test --lib --features macos` | 3853 passed, 0 failed |
| `cargo test --lib --features macos-legacy` | 3836 passed, 0 failed |
| `cargo test --lib --features objc2-macos` | 3853 passed, 0 failed |
| `cargo clippy --lib --features desktop -- -D warnings` | 0 warnings |
| `cargo clippy --lib --features macos -- -D warnings` | 0 warnings |

## Honest boundaries

- `MacOSObjc2Platform::run()` is still a polling loop; a real `NSApplication`
  event-loop bridge (`run()` graduating to `-[NSApplication run]`) is *not* yet
  implemented — this is the remaining half of `FUTURE.md` ITEM 5.
- Modal presentation for dialogs (`runModal` / sheets) is intentionally left to
  the caller; the probe only asserts object construction.
- The objc2 backend's `backend_name()` still reports `macos-objc2-preview`.
  Renaming it is deferred until the event-loop graduation lands, to avoid
  falsely advertising parity.
