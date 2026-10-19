# rust-widgets-mobile-services

Optional mobile **application-shell** services for `rust-widgets`, split out for BLUE28 B28.44.
They are deliberately not part of the widget library: a desktop build never links this crate, and
the root package's `Cargo.toml` does not compile it.

## Scope

| service | Android | iOS (incl. Mac Catalyst) | HarmonyOS |
|---|---|---|---|
| keep screen awake | `Window.addFlags(FLAG_KEEP_SCREEN_ON)` | `UIApplication.isIdleTimerDisabled` | native `OH_WindowManager_SetWindowKeepScreenOn(windowId)` |
| haptics | `Vibrator` (`VibrationEffect` on API 26+) | `UIImpactFeedbackGenerator` | native `OH_Vibrator_PlayVibration` |
| permission query | `checkSelfPermission` | `AVCaptureDevice.authorizationStatusForMediaType`, `PHPhotoLibrary.authorizationStatusForAccessLevel`, `UNUserNotificationCenter.getNotificationSettings` | native `OH_AT_CheckSelfPermission` |
| permission request | `requestPermissions` (API 33+ for notifications) | `requestAccessForMediaType`, `requestAuthorizationForAccessLevel`, `requestAuthorizationWithOptions` | ArkTS `abilityAccessCtrl` through the host |
| local notification | `NotificationManager` + API 26 channel | `UNMutableNotificationContent` + `UNNotificationRequest` | ArkTS `notificationManager` through the host |
| share sheet | `Intent.ACTION_SEND` chooser | `UIActivityViewController` | ArkTS `ShareKit` through the host |
| deep link | `Intent.ACTION_VIEW` | `UIApplication.openURL` | ArkTS `startAbility` through the host |
| sensors / location | **unsupported by design** | **unsupported by design** | **unsupported by design** |

Sensors and location are out of scope for a widget library: the application owns the permission
prompt, the privacy disclosure and the sensor lifecycle. Every backend answers
`Status::Unsupported` for them instead of holding a permission it should not have.

## Three-state contract

`MobileServices::availability` answers `Available`, `Denied` or `Unsupported`. Actions return
`Result<(), ServiceError>`; a failed platform call is never reported as success. Only
`request_permission` may show an OS prompt — `notify` and `share` never request permission
implicitly.

## Android attachment

```rust
use rust_widgets_mobile_services::android;

// From the host's JNI setup, once the VM and Activity Context exist:
let vm = /* jni::JavaVM */;
let context = /* jni::objects::GlobalRef */;
android::attach(vm, context);
```

Without an attachment every Android action answers `Unsupported`; the crate never invents a
Context.

## iOS attachment

No attachment is needed: the backend reads `UIApplication`, `AVCaptureDevice`, `PHPhotoLibrary`
and `UNUserNotificationCenter` directly. `share` and `keep_screen_on` require the main thread and
answer `Failed` elsewhere.

## HarmonyOS attachment

The C SDK covers keep-awake, haptics and the permission **query**. The ArkTS-only actions
(permission prompt, notification publishing, share, deep link) are installed as host functions:

```rust
use rust_widgets_mobile_services::harmony;

harmony::attach_window(window_id); // window.getWindowProperties().id
harmony::attach(harmony::HarmonyHost {
    keep_screen_on: None,       // the native call is used when a window id is bound
    haptic: None,               // ditto
    permission_status: None,    // ditto
    request_permission: Some(request_permission_from_arkts),
    notify: Some(publish_notification_from_arkts),
    share: Some(share_from_arkts),
    open_url: Some(open_url_from_arkts),
});
```

A service with neither a native binding nor an installed function answers `Unsupported`.

## Build

This crate is **not** a member of the root package. Check it explicitly:

```bash
cargo check --manifest-path crates/mobile-services/Cargo.toml
cargo test  --manifest-path crates/mobile-services/Cargo.toml

cargo check --manifest-path crates/mobile-services/Cargo.toml \
    --target aarch64-apple-ios
cargo check --manifest-path crates/mobile-services/Cargo.toml \
    --target aarch64-apple-ios-macabi
cargo check --manifest-path crates/mobile-services/Cargo.toml \
    --target x86_64-linux-android
cargo check --manifest-path crates/mobile-services/Cargo.toml \
    --target aarch64-unknown-linux-ohos
```
