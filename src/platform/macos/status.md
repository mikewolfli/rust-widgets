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

### cocoa-legacy backend (`src/platform/macos/platform_impl.rs`)

| Area | Status | Notes |
|---|---|---|
| `Platform` contract (all `create_*`) | ✅ Implemented | real `NSWindow`/`NSButton`/`NSTextField`/`NSPopover`… on the main thread |
| Native window registered with AppKit | ✅ Verified | `NSApplication.windows.count >= 1` after `create_window` |
| Menu bar (`NSMenu` + `NSMenuItem`) | ✅ Verified | installed as `NSApplication.mainMenu` |
| Clipboard (`NSPasteboard`) | ✅ Verified | round-trip on the main thread |
| Dialogs (`NSAlert`/`NSOpenPanel`/`NSColorPanel`/`NSFontPanel`) | ✅ Verified | objects created; modal presentation left to the caller |
| Geometry / visibility / enabled | ✅ Verified | reflected on the live `NSButton.frame` |
| Off-main-thread calls | ✅ Guarded | state-only fallback (`ptr == 0`), never aborts |

### objc2 backend (`src/platform/macos_objc2/`)

| Area | Status | Notes |
|---|---|---|
| `Platform` contract (all `create_*`) | ✅ Implemented | real AppKit objects; state fallback off-main |
| `init()` bootstraps `NSApplication` | ✅ Added 2026-09-11 | `sharedApplication` + `finishLaunching` |
| Native window registered with AppKit | ✅ Verified | `NSApplication.windows.count >= 1` |
| Menu bar (`NSMenu` + `NSMenuItem`) | ✅ Added 2026-09-11 | real items/submenus + `setMainMenu` install |
| Clipboard / dialogs | ✅ Verified | `NSPasteboard`, `NSAlert`/`NSOpenPanel`/panels |
| Geometry / visibility / text | ✅ Verified | `setNative…` helpers |
| Off-main-thread calls | ✅ Guarded | `objc2::MainThreadMarker::new()` before every `create_*` |

> Fixed 2026-09-11: `native::set_native_text` used
> `performSelector:withObject:` but declared a `()` return, while the selector
> returns `id`. objc2 validates the declared signature and aborted at runtime
> ("expected return to have type code '@', but found 'v'"). It now dispatches
> typed `setStringValue:` / `setTitle:` / `setAccessibilityLabel:` messages.
> This was masked before the feature-gate fix above made the native path
> reachable from `--features macos`.

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
[PASS] visibility_roundtrip        [PASS] visibility_roundtrip
RESULT: PASS                       RESULT: PASS
```

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
