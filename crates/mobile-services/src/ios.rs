// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS implementation over UIKit, AVFoundation, Photos and UserNotifications.
//!
//! # What each service maps to
//!
//! | service | API |
//! |---|---|
//! | keep-awake | `UIApplication.idleTimerDisabled` |
//! | haptics | `UIImpactFeedbackGenerator` |
//! | camera/microphone permission | `AVCaptureDevice.authorizationStatusForMediaType` + `requestAccess…` |
//! | photos permission | `PHPhotoLibrary.authorizationStatusForAccessLevel` + `requestAuthorization…` |
//! | notifications | `UNUserNotificationCenter` (authorization + local request) |
//! | share | `UIActivityViewController` |
//! | deep link | `UIApplication.openURL` |
//!
//! Sensors and location stay `Unsupported`: the table in `blue28.md` gives them to the
//! application, which owns the privacy disclosure and the permission prompt.
//!
//! # Threading
//!
//! UIKit presentation and the feedback generators run on the main thread; a call from another
//! thread answers `Failed` rather than raising. AVFoundation/Photos/UserNotifications accept calls
//! from any thread, and their asynchronous completion handlers update a process-wide cache, so
//! [`permission_status`] never has to block except for its first notification query.

use crate::{
    validate_url, HapticKind, Insets, MobileServices, NotificationRequest, Orientation, Permission,
    Service, ServiceError, ShareRequest, Status,
};
use block2::RcBlock;
use core::ptr::NonNull;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::MainThreadMarker;
use objc2_av_foundation::{
    AVAuthorizationStatus, AVCaptureDevice, AVMediaType, AVMediaTypeAudio, AVMediaTypeVideo,
};
use objc2_foundation::{NSArray, NSError, NSString, NSURL};
use objc2_photos::{PHAccessLevel, PHAuthorizationStatus, PHPhotoLibrary};
use objc2_ui_kit::{
    UIActivityViewController, UIApplication, UIEdgeInsets, UIImpactFeedbackGenerator,
    UIImpactFeedbackStyle, UINotificationFeedbackGenerator, UINotificationFeedbackType,
};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
    UNNotificationRequest, UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The iOS backend. Construct through [`crate::services`].
#[derive(Debug, Clone, Copy, Default)]
pub struct IosServices;

/// How long a notification-authorization answer is reused.
///
/// The query crosses to the notification centre and its callback may run on any queue.
/// Without a cache every call repeats that hop, and the first call on a busy main thread
/// pays the full 250 ms timeout. Authorization changes only when the user edits settings or
/// the app prompts, which is not a per-frame event, so 30 seconds is short enough to notice
/// a change and long enough that a burst of layout queries does not re-query.
const NOTIFICATION_STATUS_TTL: Duration = Duration::from_secs(30);

/// Last notification-authorization answer and when it was recorded.
///
/// [`Status::Unknown`] is cached like any other answer: a timed-out query is a fact about
/// that call, not a refusal, and re-blocking every caller for another timeout would make an
/// already-busy main thread worse.
static NOTIFICATION_STATUS: Mutex<Option<(Status, Instant)>> = Mutex::new(None);

/// Lock the cache, recovering a poisoned guard.
///
/// A panic while holding this lock cannot leave the cached pair half-written (the value is
/// replaced as a whole), so poison recovery is the same rule the other process-wide caches
/// in this crate follow (`android::ATTACHMENT`, `harmony::HOST`).
fn notification_cache() -> std::sync::MutexGuard<'static, Option<(Status, Instant)>> {
    NOTIFICATION_STATUS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn store_notification_status(status: Status) {
    *notification_cache() = Some((status, Instant::now()));
}

fn cached_notification_status(now: Instant) -> Option<Status> {
    match *notification_cache() {
        Some((status, recorded)) if status_is_fresh(recorded, now) => Some(status),
        _ => None,
    }
}

/// Whether an answer recorded at `recorded` is still inside [`NOTIFICATION_STATUS_TTL`].
fn status_is_fresh(recorded: Instant, now: Instant) -> bool {
    // `saturating_duration_since` answers zero when `now` precedes `recorded`, so a
    // non-monotonic clock is treated as "just recorded" instead of panicking.
    now.saturating_duration_since(recorded) < NOTIFICATION_STATUS_TTL
}

fn map_av_status(status: AVAuthorizationStatus) -> Status {
    if status == AVAuthorizationStatus::Authorized {
        Status::Available
    } else {
        Status::Denied
    }
}

fn map_photo_status(status: PHAuthorizationStatus) -> Status {
    if status == PHAuthorizationStatus::Authorized || status == PHAuthorizationStatus::Limited {
        Status::Available
    } else {
        Status::Denied
    }
}

fn map_notification_status(status: UNAuthorizationStatus) -> Status {
    if status == UNAuthorizationStatus::Authorized
        || status == UNAuthorizationStatus::Provisional
        || status == UNAuthorizationStatus::Ephemeral
    {
        Status::Available
    } else {
        Status::Denied
    }
}

/// The AVFoundation media type for a permission, if it has one.
fn media_type(permission: Permission) -> Option<&'static AVMediaType> {
    // SAFETY: both statics are AppKit/AVFoundation's immutable media-type constants, and the
    // optional they are declared as is always `Some` on a device with the framework loaded.
    unsafe {
        match permission {
            Permission::Camera => AVMediaTypeVideo,
            Permission::Microphone => AVMediaTypeAudio,
            Permission::Photos | Permission::Notifications => None,
        }
    }
}

#[allow(deprecated)] // The scene-aware replacements need a UIScene this optional crate does not own.
impl MobileServices for IosServices {
    fn availability(&self, service: Service) -> Status {
        match service {
            Service::KeepAwake
            | Service::Haptics
            | Service::Permissions
            | Service::Notifications
            | Service::Share
            | Service::DeepLink => Status::Available,
            // Reading insets is a plain property read on the key window.
            Service::SafeArea => Status::Available,
            // Locking the axis on iOS is declared by the host view controller's
            // `supportedInterfaceOrientations`; this crate does not own that controller, and
            // `UIDevice.setValue(_:forKey:"orientation")` is a private API. Reporting
            // `Unsupported` is the only honest answer.
            Service::Orientation => Status::Unsupported,
            Service::Sensors | Service::Location => Status::Unsupported,
        }
    }

    fn keep_screen_on(&self, on: bool) -> Result<(), ServiceError> {
        let Some(mtm) = MainThreadMarker::new() else {
            return Err(ServiceError::Failed);
        };
        UIApplication::sharedApplication(mtm).setIdleTimerDisabled(on);
        Ok(())
    }

    fn haptic(&self, kind: HapticKind) -> Result<(), ServiceError> {
        let Some(mtm) = MainThreadMarker::new() else {
            return Err(ServiceError::Failed);
        };
        let style = match kind {
            HapticKind::Light => UIImpactFeedbackStyle::Light,
            HapticKind::Medium => UIImpactFeedbackStyle::Medium,
            HapticKind::Heavy => UIImpactFeedbackStyle::Heavy,
            HapticKind::Success | HapticKind::Warning | HapticKind::Error => {
                // Notification feedback has its own generator; folding it into an impact made
                // success, warning and error indistinguishable.
                let kind = match kind {
                    HapticKind::Success => UINotificationFeedbackType::Success,
                    HapticKind::Warning => UINotificationFeedbackType::Warning,
                    _ => UINotificationFeedbackType::Error,
                };
                let generator = UINotificationFeedbackGenerator::new(mtm);
                generator.prepare();
                generator.notificationOccurred(kind);
                return Ok(());
            }
        };
        let generator = UIImpactFeedbackGenerator::initWithStyle(mtm.alloc(), style);
        generator.prepare();
        generator.impactOccurred();
        Ok(())
    }

    fn permission_status(&self, permission: Permission) -> Status {
        match permission {
            Permission::Camera | Permission::Microphone => {
                let Some(media) = media_type(permission) else { return Status::Unsupported };
                // SAFETY: `media` is one of AVFoundation's immutable media-type constants.
                map_av_status(unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) })
            }
            Permission::Photos => {
                // SAFETY: a class method with no arguments; it only reads the process's
                // authorization record.
                map_photo_status(unsafe {
                    PHPhotoLibrary::authorizationStatusForAccessLevel(PHAccessLevel::ReadWrite)
                })
            }
            Permission::Notifications => {
                if let Some(status) = cached_notification_status(Instant::now()) {
                    return status;
                }
                // The settings callback runs on a queue of the framework's choosing. A 250 ms
                // wait is enough for a local query; a timeout is **not** a refusal, so it is
                // reported as `Unknown` and cached under the TTL above instead of being
                // permanently recorded as `Denied` (W6-31).
                let (sender, receiver) = std::sync::mpsc::channel();
                let handler = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
                    // SAFETY: UserNotifications hands a live settings object to this callback for
                    // the duration of the call.
                    let status = unsafe { settings.as_ref() }.authorizationStatus();
                    let mapped = map_notification_status(status);
                    // Store from the callback as well, so a late answer still refreshes the
                    // cache even if the caller below already timed out.
                    store_notification_status(mapped);
                    let _ = sender.send(mapped);
                });
                UNUserNotificationCenter::currentNotificationCenter()
                    .getNotificationSettingsWithCompletionHandler(&handler);
                match receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(status) => status,
                    Err(_) => {
                        // The callback may have stored a real answer between the timeout
                        // firing and this point; prefer it over inventing `Unknown`, and
                        // otherwise record the unknown outcome under the same TTL so the
                        // next caller does not block for another timeout.
                        let now = Instant::now();
                        let mut cache = notification_cache();
                        if let Some((status, recorded)) = *cache {
                            if status_is_fresh(recorded, now) {
                                return status;
                            }
                        }
                        *cache = Some((Status::Unknown, now));
                        Status::Unknown
                    }
                }
            }
        }
    }

    fn request_permission(&self, permission: Permission) -> Result<(), ServiceError> {
        match permission {
            Permission::Camera | Permission::Microphone => {
                let Some(media) = media_type(permission) else {
                    return Err(ServiceError::Unsupported);
                };
                // The completion only updates state; the call itself reports that the prompt was
                // scheduled, not that the user granted it.
                let handler = RcBlock::new(|_granted: Bool| {});
                // SAFETY: `media` is an AVFoundation media-type constant, and the framework copies
                // the block for the duration of the request.
                unsafe {
                    AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &handler);
                }
                Ok(())
            }
            Permission::Photos => {
                let handler = RcBlock::new(|status: PHAuthorizationStatus| {
                    let _ = status;
                });
                // SAFETY: the framework copies the block; `ReadWrite` is the documented access
                // level for an app that both reads and writes the library.
                unsafe {
                    PHPhotoLibrary::requestAuthorizationForAccessLevel_handler(
                        PHAccessLevel::ReadWrite,
                        &handler,
                    );
                }
                Ok(())
            }
            Permission::Notifications => {
                let options = UNAuthorizationOptions::Alert
                    | UNAuthorizationOptions::Sound
                    | UNAuthorizationOptions::Badge;
                let handler = RcBlock::new(|granted: Bool, _error: *mut NSError| {
                    store_notification_status(if granted.as_bool() {
                        Status::Available
                    } else {
                        Status::Denied
                    });
                });
                UNUserNotificationCenter::currentNotificationCenter()
                    .requestAuthorizationWithOptions_completionHandler(options, &handler);
                Ok(())
            }
        }
    }

    fn notify(&self, request: &NotificationRequest) -> Result<(), ServiceError> {
        if request.title.trim().is_empty() && request.body.trim().is_empty() {
            // Invalid caller input, not a refusal: `Denied` would send the caller down a
            // permission-recovery path that cannot help.
            return Err(ServiceError::InvalidInput);
        }
        // Only a known refusal blocks the post; an unknown status is not evidence that
        // notifications are disabled.
        if cached_notification_status(Instant::now()) == Some(Status::Denied) {
            return Err(ServiceError::Denied);
        }
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&request.title));
        content.setBody(&NSString::from_str(&request.body));
        // A local notification without a sound is silent; the default sound is what an app that
        // did not ask for a custom one expects.
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        let identifier = NSString::from_str(&format!("rust-widgets-{}", request.id));
        let notification = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &identifier,
            &content,
            None,
        );
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&notification, None);
        Ok(())
    }

    fn share(&self, request: &ShareRequest) -> Result<(), ServiceError> {
        if request.text.trim().is_empty() && request.url.is_none() {
            // Invalid caller input, not a refusal (see `notify`).
            return Err(ServiceError::InvalidInput);
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return Err(ServiceError::Failed);
        };
        let app = UIApplication::sharedApplication(mtm);
        let Some(window) = app.keyWindow() else {
            return Err(ServiceError::Failed);
        };
        let Some(presenter) = window.rootViewController() else {
            return Err(ServiceError::Failed);
        };
        let mut text = request.text.clone();
        if let Some(url) = request.url.as_deref() {
            if !validate_url(url) {
                return Err(ServiceError::Denied);
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(url);
        }
        let item = NSString::from_str(&text);
        let items: Retained<NSArray<AnyObject>> = NSArray::from_slice(&[&*item as &AnyObject]);
        // SAFETY: the activity controller is initialised on the main thread and presented by the
        // key window's root view controller; `applicationActivities: nil` is the documented
        // "standard activities only" form.
        let controller = unsafe {
            UIActivityViewController::initWithActivityItems_applicationActivities(
                mtm.alloc(),
                &items,
                None,
            )
        };
        presenter.presentViewController_animated_completion(&controller, true, None);
        Ok(())
    }

    fn open_url(&self, url: &str) -> Result<(), ServiceError> {
        if !validate_url(url) {
            return Err(ServiceError::Denied);
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return Err(ServiceError::Failed);
        };
        let Some(url) = NSURL::URLWithString(&NSString::from_str(url)) else {
            return Err(ServiceError::Denied);
        };
        if UIApplication::sharedApplication(mtm).openURL(&url) {
            Ok(())
        } else {
            Err(ServiceError::Failed)
        }
    }

    fn safe_area(&self) -> Result<Insets, ServiceError> {
        let Some(mtm) = MainThreadMarker::new() else {
            return Err(ServiceError::Failed);
        };
        let Some(window) = UIApplication::sharedApplication(mtm).keyWindow() else {
            // No key window yet (the app is launching): zero is the honest answer, see the
            // trait documentation.
            return Ok(Insets::ZERO);
        };
        let insets: UIEdgeInsets = window.safeAreaInsets();
        Ok(Insets {
            top: insets.top as f32,
            right: insets.right as f32,
            bottom: insets.bottom as f32,
            left: insets.left as f32,
        })
    }

    fn set_orientation(&self, _orientation: Orientation) -> Result<(), ServiceError> {
        // See `availability`: the supported set belongs to the host's view controller. The call
        // is accepted by the trait so a cross-platform host has one call site, and it answers
        // `Unsupported` instead of silently doing nothing.
        Err(ServiceError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_mappers_do_not_turn_denied_into_available() {
        assert_eq!(map_av_status(AVAuthorizationStatus::Authorized), Status::Available);
        assert_eq!(map_av_status(AVAuthorizationStatus::Denied), Status::Denied);
        assert_eq!(map_av_status(AVAuthorizationStatus::NotDetermined), Status::Denied);
        assert_eq!(map_photo_status(PHAuthorizationStatus::Authorized), Status::Available);
        assert_eq!(map_photo_status(PHAuthorizationStatus::Limited), Status::Available);
        assert_eq!(map_photo_status(PHAuthorizationStatus::Restricted), Status::Denied);
        assert_eq!(map_notification_status(UNAuthorizationStatus::Provisional), Status::Available);
        assert_eq!(map_notification_status(UNAuthorizationStatus::Denied), Status::Denied);
    }

    /// W6-31: a cached answer must expire after the documented TTL, so a change the
    /// user makes in Settings is observed without a manual restart.
    #[test]
    fn a_notification_answer_expires_after_the_documented_ttl() {
        let recorded = Instant::now();
        assert!(status_is_fresh(recorded, recorded));
        assert!(status_is_fresh(
            recorded,
            recorded + NOTIFICATION_STATUS_TTL - Duration::from_millis(1)
        ));
        assert!(!status_is_fresh(recorded, recorded + NOTIFICATION_STATUS_TTL));
        assert!(!status_is_fresh(recorded, recorded + NOTIFICATION_STATUS_TTL * 2));
    }

    #[test]
    fn every_service_has_an_explicit_answer() {
        let services = IosServices;
        assert_eq!(services.availability(Service::Sensors), Status::Unsupported);
        assert_eq!(services.availability(Service::Location), Status::Unsupported);
        assert_eq!(services.availability(Service::Notifications), Status::Available);
    }
}
