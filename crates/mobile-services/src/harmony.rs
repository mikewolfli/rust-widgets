// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! HarmonyOS services forwarded to the ArkTS host.
//!
//! # Why the actions are function pointers
//!
//! Every service in `blue28.md`'s HarmonyOS column is an **ArkTS** object:
//! `window.setWindowKeepScreenOn`, `@ohos.vibrator`, `abilityAccessCtrl`,
//! `notificationManager`, `ShareKit`, and `startAbility`. None of them has a stable C ABI a Rust
//! `cdylib` can call directly, and inventing symbol names would produce a build that links only
//! on one SDK revision. The host therefore installs one function per service with [`attach`]; a
//! service with no function answers `Unsupported`, never a fabricated success.
//!
//! The contract is deliberately small and synchronous, matching the trait:
//!
//! | function | arguments | return |
//! |---|---|---|
//! | `keep_screen_on` | `1` on / `0` off | `1` ok, `0` denied, `-1` unsupported |
//! | `haptic` | [`HapticKind`] code (`1..=6`) | same |
//! | `permission_status` | [`Permission`] code (`1..=4`) | `1` available, `0` denied, `-1` unsupported |
//! | `request_permission` | [`Permission`] code | same |
//! | `notify` | id, title, body, icon resource | same |
//! | `share` | text, URL (may be null), chooser title (may be null) | same |
//! | `open_url` | validated URL | same |
//! | `safe_area` | four `f32` out-pointers (top, right, bottom, left) | same |
//! | `set_orientation` | `0` any / `1` portrait / `2` landscape | same |
//!
//! The returned strings are borrowed only for the duration of the call; the host copies whatever
//! it needs before returning.

use crate::{
    validate_url, HapticKind, Insets, MobileServices, NotificationRequest, Orientation, Permission,
    Service, ServiceError, ShareRequest, Status,
};
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::RwLock;

/// The host functions an ArkTS page can install.
#[derive(Clone, Copy, Debug, Default)]
pub struct HarmonyHost {
    /// Keep the display awake.
    pub keep_screen_on: Option<extern "C" fn(i32) -> i32>,
    /// Play one haptic ([`HapticKind`] code).
    pub haptic: Option<extern "C" fn(i32) -> i32>,
    /// Read a permission ([`Permission`] code).
    pub permission_status: Option<extern "C" fn(i32) -> i32>,
    /// Request a permission ([`Permission`] code).
    pub request_permission: Option<extern "C" fn(i32) -> i32>,
    /// Post a local notification.
    pub notify: Option<extern "C" fn(i32, *const c_char, *const c_char, i32) -> i32>,
    /// Show the system share sheet.
    pub share: Option<extern "C" fn(*const c_char, *const c_char, *const c_char) -> i32>,
    /// Open a URL or a custom scheme.
    pub open_url: Option<extern "C" fn(*const c_char) -> i32>,
    /// Write the window's safe-area insets through the four out-pointers, in vp.
    ///
    /// The out-pointer form rather than a packed return keeps the units explicit and leaves
    /// room for the four values to be fractional, which vp usually are. Returns the same
    /// `1`/`0`/`-1` code as every other host function.
    pub safe_area: Option<extern "C" fn(*mut f32, *mut f32, *mut f32, *mut f32) -> i32>,
    /// Lock the window's orientation: `0` any, `1` portrait, `2` landscape.
    pub set_orientation: Option<extern "C" fn(i32) -> i32>,
}

static HOST: RwLock<Option<HarmonyHost>> = RwLock::new(None);

/// The ArkTS window id `OH_WindowManager_SetWindowKeepScreenOn` needs, or `-1`.
static WINDOW_ID: AtomicI32 = AtomicI32::new(-1);

/// Binds the window id the native keep-awake call takes.
///
/// The id comes from the ArkTS `window` object (`window.getWindowProperties().id`). A negative
/// value clears it, and `keep_screen_on` then falls back to the host function or answers
/// `Unsupported`.
pub fn attach_window(window_id: i32) -> bool {
    WINDOW_ID.store(window_id, Ordering::SeqCst);
    window_id >= 0
}

/// Reports the window id currently bound, if any.
pub fn window_id() -> Option<i32> {
    let id = WINDOW_ID.load(Ordering::SeqCst);
    (id >= 0).then_some(id)
}

/// Installs (or replaces) the host functions.
///
/// Returns `false` when every function is `None`, which is the same as not attaching: a host
/// that has not implemented any service must not look available.
pub fn attach(host: HarmonyHost) -> bool {
    let any = host.keep_screen_on.is_some()
        || host.haptic.is_some()
        || host.permission_status.is_some()
        || host.request_permission.is_some()
        || host.notify.is_some()
        || host.share.is_some()
        || host.open_url.is_some()
        || host.safe_area.is_some()
        || host.set_orientation.is_some();
    if !any {
        return false;
    }
    *HOST.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(host);
    true
}

/// Drops the host functions.
pub fn detach() {
    *HOST.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

fn host() -> Option<HarmonyHost> {
    *HOST.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Maps the host's `1`/`0`/`-1` reply.
fn from_code(code: i32) -> Result<(), ServiceError> {
    match code {
        1 => Ok(()),
        0 => Err(ServiceError::Denied),
        -1 => Err(ServiceError::Unsupported),
        _ => Err(ServiceError::Failed),
    }
}

fn to_status(code: i32) -> Status {
    match code {
        1 => Status::Available,
        0 => Status::Denied,
        _ => Status::Unsupported,
    }
}

fn permission_code(permission: Permission) -> i32 {
    match permission {
        Permission::Camera => 1,
        Permission::Microphone => 2,
        Permission::Photos => 3,
        Permission::Notifications => 4,
    }
}

/// The native SDK answers for the services that have a C API; the rest are host-only.
///
/// A build that is not OpenHarmony has no such symbols, so the honest answer is `Unsupported`
/// unless the host installed a function.
fn native_availability(service: Service) -> Status {
    #[cfg(target_env = "ohos")]
    {
        return match service {
            Service::KeepAwake | Service::Permissions => Status::Available,
            Service::Haptics => Status::Available,
            Service::Notifications => {
                if native::notifications_enabled() {
                    // Publishing still needs the ArkTS `notificationManager` through the host.
                    Status::Unsupported
                } else {
                    Status::Denied
                }
            }
            Service::Share | Service::DeepLink | Service::Sensors | Service::Location => {
                Status::Unsupported
            }
            // Both are ArkTS-only APIs (`window.getWindowAvoidArea`,
            // `window.setPreferredOrientation`), so a native-only host has no binding for them.
            Service::SafeArea | Service::Orientation => Status::Unsupported,
        };
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = service;
        Status::Unsupported
    }
}

fn native_keep_screen_on(window_id: i32, on: bool) -> Option<Result<(), ServiceError>> {
    #[cfg(target_env = "ohos")]
    {
        return Some(native::keep_screen_on(window_id, on));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (window_id, on);
        None
    }
}

fn native_haptic(kind: HapticKind) -> Option<Result<(), ServiceError>> {
    #[cfg(target_env = "ohos")]
    {
        return Some(native::haptic(kind));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = kind;
        None
    }
}

fn native_permission_status(permission: Permission) -> Option<Status> {
    #[cfg(target_env = "ohos")]
    {
        return Some(native::permission_status(permission));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = permission;
        None
    }
}

/// Whether the platform allows this app to post notifications.
///
/// A non-OpenHarmony build has no native query; the host-owned path is trusted there, which is
/// what makes the trait testable on a desktop host.
fn notifications_allowed() -> bool {
    #[cfg(target_env = "ohos")]
    {
        native::notifications_enabled()
    }
    #[cfg(not(target_env = "ohos"))]
    {
        true
    }
}

/// The OpenHarmony native bindings the SDK actually ships.
///
/// Each `#[link]` names the library from `native/sysroot/usr/lib/<arch>-linux-ohos`; the
/// signatures are the SDK 15+ headers (`window_manager/oh_window.h`, `sensors/vibrator.h`,
/// `accesstoken/ability_access_control.h`, `NotificationKit/notification.h`).
#[cfg(target_env = "ohos")]
mod native {
    use super::{permission_name_ohos, HapticKind, Permission, ServiceError, Status};
    use std::ffi::CString;
    use std::os::raw::c_char;

    /// `Vibrator_Attribute` from `sensors/vibrator_type.h`: `int32_t vibratorId` followed by the
    /// `Vibrator_Usage` enum (also `int32_t`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VibratorAttribute {
        vibrator_id: i32,
        usage: i32,
    }

    #[link(name = "native_window_manager")]
    unsafe extern "C" {
        fn OH_WindowManager_SetWindowKeepScreenOn(window_id: i32, keep_on: bool) -> i32;
    }

    #[link(name = "ohvibrator.z")]
    unsafe extern "C" {
        fn OH_Vibrator_PlayVibration(duration: i32, attribute: VibratorAttribute) -> i32;
    }

    #[link(name = "ability_access_control")]
    unsafe extern "C" {
        fn OH_AT_CheckSelfPermission(permission: *const c_char) -> bool;
    }

    #[link(name = "ohnotification")]
    unsafe extern "C" {
        fn OH_Notification_IsNotificationEnabled() -> bool;
    }

    fn from_window_code(code: i32) -> Result<(), ServiceError> {
        match code {
            0 => Ok(()),
            // WINDOW_MANAGER_ERRORCODE_INVALID_PARAM / STATE_ABNORMAL / SYSTEM_ABNORMAL.
            401 | 1300002 | 1300003 => Err(ServiceError::Failed),
            _ => Err(ServiceError::Failed),
        }
    }

    fn from_vibrator_code(code: i32) -> Result<(), ServiceError> {
        match code {
            0 => Ok(()),
            201 => Err(ServiceError::Denied),
            801 => Err(ServiceError::Unsupported),
            _ => Err(ServiceError::Failed),
        }
    }

    pub(super) fn keep_screen_on(window_id: i32, on: bool) -> Result<(), ServiceError> {
        // SAFETY: the window id was bound by the ArkTS host, and the call only sets a window flag.
        from_window_code(unsafe { OH_WindowManager_SetWindowKeepScreenOn(window_id, on) })
    }

    pub(super) fn haptic(kind: HapticKind) -> Result<(), ServiceError> {
        let (duration, usage) = match kind {
            HapticKind::Light => (10, 5), // VIBRATOR_USAGE_TOUCH
            HapticKind::Medium => (20, 5),
            HapticKind::Heavy => (40, 5),
            HapticKind::Success | HapticKind::Warning | HapticKind::Error => {
                (30, 3) // VIBRATOR_USAGE_NOTIFICATION
            }
        };
        let attribute = VibratorAttribute { vibrator_id: 0, usage };
        // SAFETY: a fixed POD attribute by value; the API copies it.
        from_vibrator_code(unsafe { OH_Vibrator_PlayVibration(duration, attribute) })
    }

    pub(super) fn permission_status(permission: Permission) -> Status {
        let Ok(name) = CString::new(permission_name_ohos(permission)) else {
            return Status::Unsupported;
        };
        // SAFETY: `name` is a live NUL-terminated string for the duration of the call.
        if unsafe { OH_AT_CheckSelfPermission(name.as_ptr()) } {
            Status::Available
        } else {
            Status::Denied
        }
    }

    pub(super) fn notifications_enabled() -> bool {
        // SAFETY: a no-argument query of the application's notification setting.
        unsafe { OH_Notification_IsNotificationEnabled() }
    }
}

/// The HarmonyOS permission strings from `@ohos.abilityAccessCtrl`.
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
fn permission_name_ohos(permission: Permission) -> &'static str {
    match permission {
        Permission::Camera => "ohos.permission.CAMERA",
        Permission::Microphone => "ohos.permission.MICROPHONE",
        Permission::Photos => "ohos.permission.READ_IMAGEVIDEO",
        Permission::Notifications => "ohos.permission.NOTIFICATION_CONTROLLER",
    }
}

fn haptic_code(kind: HapticKind) -> i32 {
    match kind {
        HapticKind::Light => 1,
        HapticKind::Medium => 2,
        HapticKind::Heavy => 3,
        HapticKind::Success => 4,
        HapticKind::Warning => 5,
        HapticKind::Error => 6,
    }
}

/// The HarmonyOS backend. Construct through [`crate::services`].
#[derive(Debug, Clone, Copy, Default)]
pub struct HarmonyServices;

impl MobileServices for HarmonyServices {
    fn availability(&self, service: Service) -> Status {
        let Some(host) = host() else {
            // Keep-awake, haptics and the permission query have real native bindings; the rest
            // need the host functions.
            return native_availability(service);
        };
        let present = match service {
            Service::KeepAwake => window_id().is_some() || host.keep_screen_on.is_some(),
            Service::Haptics => true,
            Service::Permissions => {
                host.permission_status.is_some() || host.request_permission.is_some()
            }
            Service::Notifications => host.notify.is_some() && notifications_allowed(),
            Service::Share => host.share.is_some(),
            Service::DeepLink => host.open_url.is_some(),
            Service::SafeArea => host.safe_area.is_some(),
            Service::Orientation => host.set_orientation.is_some(),
            Service::Sensors | Service::Location => false,
        };
        if present {
            Status::Available
        } else {
            Status::Unsupported
        }
    }

    fn keep_screen_on(&self, on: bool) -> Result<(), ServiceError> {
        if let Some(id) = window_id() {
            if let Some(result) = native_keep_screen_on(id, on) {
                return result;
            }
        }
        if let Some(call) = host().and_then(|host| host.keep_screen_on) {
            return from_code(call(i32::from(on)));
        }
        Err(ServiceError::Unsupported)
    }

    fn haptic(&self, kind: HapticKind) -> Result<(), ServiceError> {
        if let Some(result) = native_haptic(kind) {
            return result;
        }
        let Some(call) = host().and_then(|host| host.haptic) else {
            return Err(ServiceError::Unsupported);
        };
        from_code(call(haptic_code(kind)))
    }

    fn permission_status(&self, permission: Permission) -> Status {
        if permission == Permission::Notifications {
            if !notifications_allowed() {
                return Status::Denied;
            }
            return if host().is_some_and(|host| host.notify.is_some()) {
                Status::Available
            } else {
                Status::Unsupported
            };
        }
        if let Some(status) = native_permission_status(permission) {
            return status;
        }
        let Some(call) = host().and_then(|host| host.permission_status) else {
            return Status::Unsupported;
        };
        to_status(call(permission_code(permission)))
    }

    fn request_permission(&self, permission: Permission) -> Result<(), ServiceError> {
        let Some(call) = host().and_then(|host| host.request_permission) else {
            // ArkTS `abilityAccessCtrl.requestPermissionsFromUser` is the only prompt API; the
            // native SDK exposes the query only.
            return Err(ServiceError::Unsupported);
        };
        from_code(call(permission_code(permission)))
    }

    fn notify(&self, request: &NotificationRequest) -> Result<(), ServiceError> {
        let Some(host) = host() else { return Err(ServiceError::Unsupported) };
        let Some(call) = host.notify else { return Err(ServiceError::Unsupported) };
        let title = CString::new(request.title.as_str()).map_err(|_| ServiceError::Denied)?;
        let body = CString::new(request.body.as_str()).map_err(|_| ServiceError::Denied)?;
        from_code(call(
            request.id,
            title.as_ptr(),
            body.as_ptr(),
            request.icon_resource.unwrap_or(0),
        ))
    }

    fn share(&self, request: &ShareRequest) -> Result<(), ServiceError> {
        let Some(host) = host() else { return Err(ServiceError::Unsupported) };
        let Some(call) = host.share else { return Err(ServiceError::Unsupported) };
        if request.text.is_empty() && request.url.is_none() {
            return Err(ServiceError::Denied);
        }
        if let Some(url) = request.url.as_deref() {
            if !validate_url(url) {
                return Err(ServiceError::Denied);
            }
        }
        let text = CString::new(request.text.as_str()).map_err(|_| ServiceError::Denied)?;
        let url = request
            .url
            .as_deref()
            .map(CString::new)
            .transpose()
            .map_err(|_| ServiceError::Denied)?;
        let title = request
            .title
            .as_deref()
            .map(CString::new)
            .transpose()
            .map_err(|_| ServiceError::Denied)?;
        from_code(call(
            text.as_ptr(),
            url.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
            title.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
        ))
    }

    fn open_url(&self, url: &str) -> Result<(), ServiceError> {
        if !validate_url(url) {
            return Err(ServiceError::Denied);
        }
        let Some(host) = host() else { return Err(ServiceError::Unsupported) };
        let Some(call) = host.open_url else { return Err(ServiceError::Unsupported) };
        let value = CString::new(url).map_err(|_| ServiceError::Denied)?;
        from_code(call(value.as_ptr()))
    }

    fn safe_area(&self) -> Result<Insets, ServiceError> {
        let Some(host) = host() else { return Err(ServiceError::Unsupported) };
        let Some(call) = host.safe_area else { return Err(ServiceError::Unsupported) };
        let mut insets = Insets::ZERO;
        let code = call(&mut insets.top, &mut insets.right, &mut insets.bottom, &mut insets.left);
        if code != 1 {
            return from_code(code).map(|_| Insets::ZERO);
        }
        // A negative inset cannot mean anything; clamp rather than trust a host that wrote one.
        insets.top = insets.top.max(0.0);
        insets.right = insets.right.max(0.0);
        insets.bottom = insets.bottom.max(0.0);
        insets.left = insets.left.max(0.0);
        Ok(insets)
    }

    fn set_orientation(&self, orientation: Orientation) -> Result<(), ServiceError> {
        let Some(host) = host() else { return Err(ServiceError::Unsupported) };
        let Some(call) = host.set_orientation else { return Err(ServiceError::Unsupported) };
        let code = match orientation {
            Orientation::Any => 0,
            Orientation::Portrait => 1,
            Orientation::Landscape => 2,
        };
        from_code(call(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    static LAST_HAPTIC: AtomicI32 = AtomicI32::new(0);
    static LAST_ON: AtomicI32 = AtomicI32::new(-1);
    static LAST_ORIENTATION: AtomicI32 = AtomicI32::new(-1);
    /// The host is process-wide, so the tests that install and remove it serialize.
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    extern "C" fn keep(on: i32) -> i32 {
        LAST_ON.store(on, Ordering::SeqCst);
        1
    }
    extern "C" fn haptic(code: i32) -> i32 {
        LAST_HAPTIC.store(code, Ordering::SeqCst);
        1
    }
    extern "C" fn permission(code: i32) -> i32 {
        i32::from(code == 1)
    }
    extern "C" fn request(code: i32) -> i32 {
        i32::from(code == 1)
    }
    extern "C" fn notify(_id: i32, _title: *const c_char, _body: *const c_char, _icon: i32) -> i32 {
        1
    }
    extern "C" fn share(_text: *const c_char, _url: *const c_char, _title: *const c_char) -> i32 {
        1
    }
    extern "C" fn open(url: *const c_char) -> i32 {
        // Read the borrowed URL so the test proves the pointer carries the validated value.
        // SAFETY: `open_url` passes a live `CString` for the duration of this call.
        let value = unsafe { std::ffi::CStr::from_ptr(url) };
        i32::from(value.to_bytes() == b"https://example.com")
    }

    /// Writes fixed insets through the out-pointers, the way an ArkTS host would.
    extern "C" fn safe_area(
        top: *mut f32,
        right: *mut f32,
        bottom: *mut f32,
        left: *mut f32,
    ) -> i32 {
        // SAFETY: `HarmonyServices::safe_area` passes four live `f32` locations.
        unsafe {
            *top = 44.0;
            *right = 0.0;
            *bottom = 34.0;
            *left = 0.0;
        }
        1
    }

    /// Records the requested orientation code so the test can read it back.
    extern "C" fn orientation(code: i32) -> i32 {
        LAST_ORIENTATION.store(code, Ordering::SeqCst);
        1
    }

    fn install() -> HarmonyHost {
        let host = HarmonyHost {
            keep_screen_on: Some(keep),
            haptic: Some(haptic),
            permission_status: Some(permission),
            request_permission: Some(request),
            notify: Some(notify),
            share: Some(share),
            open_url: Some(open),
            safe_area: Some(safe_area),
            set_orientation: Some(orientation),
        };
        assert!(attach(host));
        host
    }

    #[test]
    fn safe_area_returns_the_host_insets_and_orientation_uses_the_documented_codes() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        detach();
        install();
        let services = HarmonyServices;
        assert_eq!(services.availability(Service::SafeArea), Status::Available);
        assert_eq!(services.availability(Service::Orientation), Status::Available);
        assert_eq!(
            services.safe_area(),
            Ok(Insets { top: 44.0, right: 0.0, bottom: 34.0, left: 0.0 })
        );
        assert_eq!(services.set_orientation(Orientation::Landscape), Ok(()));
        assert_eq!(LAST_ORIENTATION.load(Ordering::SeqCst), 2, "landscape is code 2");
        assert_eq!(services.set_orientation(Orientation::Any), Ok(()));
        assert_eq!(LAST_ORIENTATION.load(Ordering::SeqCst), 0, "any is code 0");
        detach();
    }

    #[test]
    fn without_a_host_every_action_is_unsupported() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        detach();
        let services = HarmonyServices;
        assert_eq!(services.availability(Service::KeepAwake), Status::Unsupported);
        assert_eq!(services.keep_screen_on(true), Err(ServiceError::Unsupported));
        assert_eq!(services.sensors(), Err(ServiceError::Unsupported));
    }

    #[test]
    fn an_empty_host_is_refused() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        detach();
        assert!(!attach(HarmonyHost::default()));
        assert!(host().is_none());
    }

    #[test]
    fn every_installed_function_is_called_with_its_documented_code() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = install();
        let services = HarmonyServices;
        assert_eq!(services.availability(Service::KeepAwake), Status::Available);
        assert_eq!(services.keep_screen_on(true), Ok(()));
        assert_eq!(LAST_ON.load(Ordering::SeqCst), 1);
        assert_eq!(services.haptic(HapticKind::Error), Ok(()));
        assert_eq!(LAST_HAPTIC.load(Ordering::SeqCst), 6);
        assert_eq!(services.permission_status(Permission::Camera), Status::Available);
        assert_eq!(services.request_permission(Permission::Camera), Ok(()));
        assert_eq!(services.open_url("https://example.com"), Ok(()));
        assert_eq!(services.open_url("javascript:alert(1)"), Err(ServiceError::Denied));
        assert_eq!(
            services.share(&ShareRequest { text: "x".into(), url: None, title: None }),
            Ok(())
        );
        assert_eq!(
            services.notify(&NotificationRequest {
                id: 1,
                title: "t".into(),
                body: "b".into(),
                icon_resource: None,
            }),
            Ok(())
        );
        detach();
    }
}
