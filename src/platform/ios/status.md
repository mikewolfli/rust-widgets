<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# iOS Backend Status

> Last verified: 2026-09-11 on a real iOS 26.2 Simulator (Xcode 26.2, arm64
> simulator on an Apple Silicon Mac).
> Evidence: `docs/log/log-20260911-2.md`, gate `tools/check_apple_native.sh`,
> host app `bindings/ios/main.m`.

## Architecture

`IosMobilePlatform` (`src/platform/ios/platform_impl.rs`) implements the full
`Platform` contract. The widget state machine is platform-independent and
compiles on every host (so its unit tests are always executable). The real
UIKit FFI lives in `src/platform/ios/native.rs` and is gated behind
`#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]`; elsewhere the
backend runs in pure state mode.

## What is implemented

| Area | Status | Notes |
|---|---|---|
| `Platform` contract (all `create_*`) | ✅ Implemented | state-backed with UIKit side effects when the FFI feature is on |
| `UIWindow` creation | ✅ Verified on Simulator | real `UIWindow` with a root view controller, visible |
| `UIButton` / `UILabel` / `UITextField` | ✅ Verified on Simulator | real subviews of the window's content view |
| `UISwitch`/`UISlider`/`UIProgressView`/`UIPickerView`/`UITableView`/`UIScrollView` | ✅ Implemented | created via `objc2-ui-kit` |
| Text round-trip | ✅ Verified on Simulator | `rw_set_widget_text` / `rw_get_widget_text` |
| Geometry / visibility / enabled | ✅ Verified on Simulator | `setFrame`/`setHidden`/`setEnabled` on the live view |
| `UIAlertController` message box | ✅ Implemented | presented on the root view controller when available |
| IME + accessibility metadata | ✅ Implemented | modelled state |

## Verification (real iOS Simulator, 2026-09-11)

`tools/build_ios_testapp.sh` builds a real `.app` (staticlib + Objective-C host,
no Xcode project), and `tools/run_ios_testapp.sh` installs, launches, and
asserts the probe's `RESULT: PASS`:

```
[PASS] ui_application: UIApplication.sharedApplication = 0x10b606df0
[PASS] create_window_id: rw_create_window = 1
[PASS] native_uikit_window: app.windows=2 rustWindow=0x10e006c90 rootVC=0x10b611ed0
[PASS] native_view_ids: button=2 label=3 line_edit=4
[PASS] native_uikit_subviews: UIButton=0x10e007030 UILabel=0x10e008e40 UITextField=0x10d02f000
[PASS] text_roundtrip: rw_get_widget_text = Tapped
[PASS] native_text_applied: UIButton title = Tapped
[PASS] visibility_geometry: hidden_reported=1 shown_reported=1
[PASS] native_frame_applied: UIButton.frame = {{20, 60}, {140, 44}}
```

> Fixed 2026-09-11: `native::set_native_text` probed `setTitle:forState:` with a
> one-argument `respondsToSelector:`, which always returns `false` for a
> two-argument selector. The loop therefore fell through to
> `setAccessibilityLabel:`, so `rw_set_widget_text` updated the Rust state but
> **never changed the visible `UIButton` title** — a silent no-op, not a crash.
> It now dispatches typed `setTitle:forState:` / `setText:` /
> `setAccessibilityLabel:` messages. The `native_text_applied` check
> (`UIButton.titleForState == "Tapped"`) was added to make this class of
> "state-only fake fix" impossible to miss.

Reproduce:

```bash
rustup target add aarch64-apple-ios-sim
xcrun simctl boot <device-udid>          # or use tools/run_ios_testapp.sh
bash tools/run_ios_testapp.sh
```

Cross-target compile checks (0 errors, 0 warnings):

| Target / features | Result |
|---|---|
| `aarch64-apple-ios` (state backend) | ✅ 0 errors |
| `aarch64-apple-ios` + `ios-uikit-ffi` | ✅ 0 errors |
| `aarch64-apple-ios-sim` + `ios-uikit-ffi` | ✅ 0 errors |
| `cargo clippy --target aarch64-apple-ios-sim ... -D warnings` | ✅ 0 warnings |

## Honest boundaries

- The probe runs on the **Simulator**, not a physical iPhone. A physical-device
  run (signing + provisioning) is not part of this gate.
- `create_ui_window` uses `initWithFrame:` (scene-less) because the library has
  no access to a `UIWindowScene` instance; `initWithWindowScene:` would be the
  modern path once a scene is threaded through by the host app.
- The library ships no AndroidX-equivalent framework dependency; UIKit is the
  system framework, so no bundling is required (unlike the Android Toolbar case).
