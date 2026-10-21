// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android implementation over JNI.
//!
//! # The host attaches once
//!
//! This crate cannot discover the Activity on its own. The host calls [`attach`] with the
//! `JavaVM` and the Activity `Context`; every action then resolves what it needs from that
//! context. Without an attachment the backend answers `Unsupported`, never a fake success.
//!
//! # What is real
//!
//! Keep-awake (`Window.addFlags/clearFlags`), haptics (`Vibrator.vibrate`), permission query and
//! request (`checkSelfPermission` / `requestPermissions`), notifications (`NotificationManager`
//! with an API 26+ channel), share (`Intent.ACTION_SEND` chooser) and deep links
//! (`Intent.ACTION_VIEW`). Sensors and location are deliberately unsupported.

use crate::{
    validate_url, HapticKind, Insets, MobileServices, NotificationRequest, Orientation, Permission,
    Service, ServiceError, ShareRequest, Status,
};
use jni::objects::{GlobalRef, JObject, JObjectArray, JValue};
use jni::{JNIEnv, JavaVM};
use std::sync::{Arc, RwLock};

struct Attachment {
    vm: JavaVM,
    context: GlobalRef,
}

/// The current attachment.
///
/// A replaceable cell rather than a `OnceLock`: a configuration change recreates the Activity and
/// the new instance must be usable without restarting the process. The old `detach` was a no-op
/// that documented the limitation instead of removing it.
static ATTACHMENT: RwLock<Option<Arc<Attachment>>> = RwLock::new(None);

/// Binds the Android host's VM and Activity `Context`.
///
/// Returns `false` when the context is null. A second call replaces the previous attachment, which
/// is what a recreated Activity needs; the old global reference is dropped by the replacement.
pub fn attach(vm: JavaVM, context: GlobalRef) -> bool {
    if context.as_obj().is_null() {
        return false;
    }
    let mut slot = ATTACHMENT.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = Some(Arc::new(Attachment { vm, context }));
    true
}

/// Drops the attachment, so a recreated Activity can attach again.
pub fn detach() {
    let mut slot = ATTACHMENT.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = None;
}

fn with_env<R>(
    f: impl for<'local> FnOnce(&mut JNIEnv<'local>, &JObject<'local>) -> R,
) -> Option<R> {
    let attachment =
        ATTACHMENT.read().unwrap_or_else(|poisoned| poisoned.into_inner()).as_ref()?.clone();
    let mut env = attachment.vm.attach_current_thread().ok()?;
    let result = f(&mut env, attachment.context.as_obj());
    // A JNI call that threw leaves a pending Java exception; every subsequent JNI call on this
    // thread then fails. The closure already turned the failure into a `ServiceError`, so the
    // exception is cleared here rather than allowed to poison the next action.
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    Some(result)
}

fn service<'local>(
    env: &mut JNIEnv<'local>,
    context: &JObject<'local>,
    name: &str,
) -> Option<JObject<'local>> {
    let name = env.new_string(name).ok()?;
    let manager = env
        .call_method(
            context,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&name)],
        )
        .ok()?
        .l()
        .ok()?;
    (!manager.is_null()).then_some(manager)
}

/// `Build.VERSION.SDK_INT`, or `0` when it cannot be read.
fn sdk_int(env: &mut JNIEnv<'_>) -> i32 {
    let Ok(class) = env.find_class("android/os/Build$VERSION") else { return 0 };
    env.get_static_field(&class, "SDK_INT", "I").ok().and_then(|value| value.i().ok()).unwrap_or(0)
}

/// `NotificationManager.areNotificationsEnabled()`, or `None` when the call failed.
fn notifications_enabled<'local>(
    env: &mut JNIEnv<'local>,
    context: &JObject<'local>,
) -> Option<bool> {
    let manager = service(env, context, "notification")?;
    env.call_method(&manager, "areNotificationsEnabled", "()Z", &[]).ok()?.z().ok()
}

/// Starts the system permission dialog for `permission` on API 33+.
fn request_runtime_permission<'local>(
    env: &mut JNIEnv<'local>,
    context: &JObject<'local>,
    permission: &str,
) -> Result<(), ServiceError> {
    let name = env.new_string(permission).map_err(|_| ServiceError::Failed)?;
    let array: JObjectArray = env
        .new_object_array(1, "java/lang/String", JObject::null())
        .map_err(|_| ServiceError::Failed)?;
    env.set_object_array_element(&array, 0, &name).map_err(|_| ServiceError::Failed)?;
    env.call_method(
        context,
        "requestPermissions",
        "([Ljava/lang/String;I)V",
        &[JValue::Object(&array), JValue::Int(PERMISSION_REQUEST_CODE)],
    )
    .map(|_| ())
    .map_err(|_| ServiceError::Failed)
}

/// Request code the host sees in `onRequestPermissionsResult`.
pub const PERMISSION_REQUEST_CODE: i32 = 21080;

impl MobileServices for AndroidServices {
    fn availability(&self, service_kind: Service) -> Status {
        if ATTACHMENT.read().unwrap_or_else(|poisoned| poisoned.into_inner()).is_none() {
            return Status::Unsupported;
        }
        match service_kind {
            Service::KeepAwake | Service::Haptics | Service::Share | Service::DeepLink => {
                Status::Available
            }
            Service::Notifications => {
                let enabled =
                    with_env(|env, context| notifications_enabled(env, context)).flatten();
                match enabled {
                    Some(true) => Status::Available,
                    Some(false) => Status::Denied,
                    None => Status::Unsupported,
                }
            }
            Service::Permissions => Status::Available,
            // Both need the attached Activity's window. A null window is answered as
            // `Unsupported` by the actions themselves, so the coarse availability is
            // `Available` on a target that has a binding at all.
            Service::SafeArea | Service::Orientation => Status::Available,
            Service::Sensors | Service::Location => Status::Unsupported,
        }
    }

    fn keep_screen_on(&self, on: bool) -> Result<(), ServiceError> {
        const FLAG_KEEP_SCREEN_ON: i32 = 0x0000_0080;
        with_env(|env, context| {
            let window = env
                .call_method(context, "getWindow", "()Landroid/view/Window;", &[])
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            let method = if on { "addFlags" } else { "clearFlags" };
            env.call_method(&window, method, "(I)V", &[JValue::Int(FLAG_KEEP_SCREEN_ON)])
                .map(|_| ())
                .map_err(|_| ServiceError::Failed)
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }

    fn haptic(&self, kind: HapticKind) -> Result<(), ServiceError> {
        let millis = match kind {
            HapticKind::Light => 10,
            HapticKind::Medium => 20,
            HapticKind::Heavy => 40,
            HapticKind::Success | HapticKind::Warning | HapticKind::Error => 30,
        };
        with_env(|env, context| {
            // `Vibrator.vibrate` silently does nothing without this normal permission, so the
            // honest answer is `Denied` rather than a success that produced no haptic.
            let permission =
                env.new_string("android.permission.VIBRATE").map_err(|_| ServiceError::Failed)?;
            let granted = env
                .call_method(
                    context,
                    "checkSelfPermission",
                    "(Ljava/lang/String;)I",
                    &[JValue::Object(&permission)],
                )
                .map_err(|_| ServiceError::Failed)?
                .i()
                .map_err(|_| ServiceError::Failed)?
                == 0;
            if !granted {
                return Err(ServiceError::Denied);
            }
            let vibrator = service(env, context, "vibrator").ok_or(ServiceError::Failed)?;
            if sdk_int(env) >= 29 {
                // API 29's predefined effects are the platform's own haptic vocabulary, so a
                // success/warning/error is distinguishable rather than three identical buzzes.
                let effect_id = match kind {
                    HapticKind::Light => 3,   // EFFECT_TICK
                    HapticKind::Medium => 0,  // EFFECT_CLICK
                    HapticKind::Heavy => 2,   // EFFECT_HEAVY_CLICK
                    HapticKind::Success => 1, // EFFECT_DOUBLE_CLICK
                    HapticKind::Warning => 1,
                    HapticKind::Error => 2,
                };
                let effect_class = env
                    .find_class("android/os/VibrationEffect")
                    .map_err(|_| ServiceError::Failed)?;
                let effect = env
                    .call_static_method(
                        effect_class,
                        "createPredefined",
                        "(I)Landroid/os/VibrationEffect;",
                        &[JValue::Int(effect_id)],
                    )
                    .map_err(|_| ServiceError::Failed)?
                    .l()
                    .map_err(|_| ServiceError::Failed)?;
                env.call_method(
                    &vibrator,
                    "vibrate",
                    "(Landroid/os/VibrationEffect;)V",
                    &[JValue::Object(&effect)],
                )
                .map(|_| ())
                .map_err(|_| ServiceError::Failed)
            } else if sdk_int(env) >= 26 {
                let effect_class = env
                    .find_class("android/os/VibrationEffect")
                    .map_err(|_| ServiceError::Failed)?;
                let effect = env
                    .call_static_method(
                        effect_class,
                        "createOneShot",
                        "(JI)Landroid/os/VibrationEffect;",
                        &[JValue::Long(millis), JValue::Int(-1)],
                    )
                    .map_err(|_| ServiceError::Failed)?
                    .l()
                    .map_err(|_| ServiceError::Failed)?;
                env.call_method(
                    &vibrator,
                    "vibrate",
                    "(Landroid/os/VibrationEffect;)V",
                    &[JValue::Object(&effect)],
                )
                .map(|_| ())
                .map_err(|_| ServiceError::Failed)
            } else {
                env.call_method(&vibrator, "vibrate", "(J)V", &[JValue::Long(millis)])
                    .map(|_| ())
                    .map_err(|_| ServiceError::Failed)
            }
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }

    fn permission_status(&self, permission: Permission) -> Status {
        if permission == Permission::Notifications {
            return self.availability(Service::Notifications);
        }
        with_env(|env, context| {
            let sdk = sdk_int(env);
            let name = env.new_string(permission_name(permission, sdk)).ok()?;
            let answer = env
                .call_method(
                    context,
                    "checkSelfPermission",
                    "(Ljava/lang/String;)I",
                    &[JValue::Object(&name)],
                )
                .ok()?
                .i()
                .ok()?;
            Some(if answer == 0 { Status::Available } else { Status::Denied })
        })
        .flatten()
        .unwrap_or(Status::Unsupported)
    }

    fn request_permission(&self, permission: Permission) -> Result<(), ServiceError> {
        if permission == Permission::Notifications {
            return with_env(|env, context| {
                if notifications_enabled(env, context) == Some(true) {
                    return Ok(());
                }
                // API 33 introduced the runtime POST_NOTIFICATIONS permission; before it, a
                // disabled notification channel can only be changed in Settings, so the honest
                // answer is `Denied` rather than a dialog that will never appear.
                if sdk_int(env) < 33 {
                    return Err(ServiceError::Denied);
                }
                let sdk = sdk_int(env);
                request_runtime_permission(env, context, permission_name(permission, sdk))
            })
            .unwrap_or(Err(ServiceError::Unsupported));
        }
        with_env(|env, context| {
            let sdk = sdk_int(env);
            request_runtime_permission(env, context, permission_name(permission, sdk))
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }

    fn notify(&self, request: &NotificationRequest) -> Result<(), ServiceError> {
        with_env(|env, context| {
            let manager = service(env, context, "notification").ok_or(ServiceError::Failed)?;
            if notifications_enabled(env, context) == Some(false) {
                return Err(ServiceError::Denied);
            }
            let channel_id = env.new_string("rust-widgets").map_err(|_| ServiceError::Failed)?;
            let channel_name = env.new_string("Rust Widgets").map_err(|_| ServiceError::Failed)?;
            let sdk = sdk_int(env);
            if sdk >= 26 {
                let importance = 3; // IMPORTANCE_DEFAULT
                let channel = env
                    .new_object(
                        "android/app/NotificationChannel",
                        "(Ljava/lang/String;Ljava/lang/CharSequence;I)V",
                        &[
                            JValue::Object(&channel_id),
                            JValue::Object(&channel_name),
                            JValue::Int(importance),
                        ],
                    )
                    .map_err(|_| ServiceError::Failed)?;
                env.call_method(
                    &manager,
                    "createNotificationChannel",
                    "(Landroid/app/NotificationChannel;)V",
                    &[JValue::Object(&channel)],
                )
                .map_err(|_| ServiceError::Failed)?;
            }
            let icon = match request.icon_resource {
                Some(id) => id,
                None => {
                    let resources = env
                        .call_method(
                            context,
                            "getResources",
                            "()Landroid/content/res/Resources;",
                            &[],
                        )
                        .map_err(|_| ServiceError::Failed)?
                        .l()
                        .map_err(|_| ServiceError::Failed)?;
                    let name =
                        env.new_string("ic_dialog_info").map_err(|_| ServiceError::Failed)?;
                    let kind = env.new_string("drawable").map_err(|_| ServiceError::Failed)?;
                    let package = env.new_string("android").map_err(|_| ServiceError::Failed)?;
                    env.call_method(
                        &resources,
                        "getIdentifier",
                        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
                        &[JValue::Object(&name), JValue::Object(&kind), JValue::Object(&package)],
                    )
                    .map_err(|_| ServiceError::Failed)?
                    .i()
                    .map_err(|_| ServiceError::Failed)?
                }
            };
            let title = env.new_string(&request.title).map_err(|_| ServiceError::Failed)?;
            let body = env.new_string(&request.body).map_err(|_| ServiceError::Failed)?;
            if icon == 0 {
                return Err(ServiceError::Failed);
            }
            // The two-argument `Builder(Context, channelId)` constructor exists only on API 26+;
            // the one-argument form is what API 24/25 hosts have.
            let builder = if sdk >= 26 {
                env.new_object(
                    "android/app/Notification$Builder",
                    "(Landroid/content/Context;Ljava/lang/String;)V",
                    &[JValue::Object(context), JValue::Object(&channel_id)],
                )
                .map_err(|_| ServiceError::Failed)?
            } else {
                env.new_object(
                    "android/app/Notification$Builder",
                    "(Landroid/content/Context;)V",
                    &[JValue::Object(context)],
                )
                .map_err(|_| ServiceError::Failed)?
            };
            let _ = env.call_method(
                &builder,
                "setAutoCancel",
                "(Z)Landroid/app/Notification$Builder;",
                &[JValue::Bool(1)],
            );
            let _ = env.call_method(
                &builder,
                "setContentTitle",
                "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;",
                &[JValue::Object(&title)],
            );
            let _ = env.call_method(
                &builder,
                "setContentText",
                "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;",
                &[JValue::Object(&body)],
            );
            let _ = env.call_method(
                &builder,
                "setSmallIcon",
                "(I)Landroid/app/Notification$Builder;",
                &[JValue::Int(icon)],
            );
            let notification = env
                .call_method(&builder, "build", "()Landroid/app/Notification;", &[])
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            env.call_method(
                &manager,
                "notify",
                "(ILandroid/app/Notification;)V",
                &[JValue::Int(request.id), JValue::Object(&notification)],
            )
            .map(|_| ())
            .map_err(|_| ServiceError::Failed)
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }

    fn share(&self, request: &ShareRequest) -> Result<(), ServiceError> {
        if request.text.is_empty() && request.url.is_none() {
            return Err(ServiceError::Denied);
        }
        let mut payload = request.text.clone();
        if let Some(url) = request.url.as_deref() {
            if !validate_url(url) {
                return Err(ServiceError::Denied);
            }
            if !payload.is_empty() {
                payload.push(' ');
            }
            payload.push_str(url);
        }
        with_env(|env, context| {
            let intent_class =
                env.find_class("android/content/Intent").map_err(|_| ServiceError::Failed)?;
            let action =
                env.new_string("android.intent.action.SEND").map_err(|_| ServiceError::Failed)?;
            let intent = env
                .new_object(&intent_class, "(Ljava/lang/String;)V", &[JValue::Object(&action)])
                .map_err(|_| ServiceError::Failed)?;
            let text = env.new_string(&payload).map_err(|_| ServiceError::Failed)?;
            let extra_key =
                env.new_string("android.intent.extra.TEXT").map_err(|_| ServiceError::Failed)?;
            let _ = env.call_method(
                &intent,
                "putExtra",
                "(Ljava/lang/String;Ljava/lang/CharSequence;)Landroid/content/Intent;",
                &[JValue::Object(&extra_key), JValue::Object(&text)],
            );
            let mime = env.new_string("text/plain").map_err(|_| ServiceError::Failed)?;
            let _ = env.call_method(
                &intent,
                "setType",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&mime)],
            );
            let chooser_class =
                env.find_class("android/content/Intent").map_err(|_| ServiceError::Failed)?;
            let title = env
                .new_string(request.title.as_deref().unwrap_or("Share"))
                .map_err(|_| ServiceError::Failed)?;
            let chooser = env
                .call_static_method(
                    chooser_class,
                    "createChooser",
                    "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;",
                    &[JValue::Object(&intent), JValue::Object(&title)],
                )
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            env.call_method(
                context,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&chooser)],
            )
            .map(|_| ())
            .map_err(|_| ServiceError::Failed)
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }

    fn open_url(&self, url: &str) -> Result<(), ServiceError> {
        if !validate_url(url) {
            return Err(ServiceError::Denied);
        }
        with_env(|env, context| {
            let uri_class = env.find_class("android/net/Uri").map_err(|_| ServiceError::Failed)?;
            let value = env.new_string(url).map_err(|_| ServiceError::Failed)?;
            let uri = env
                .call_static_method(
                    uri_class,
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&value)],
                )
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            let intent_class =
                env.find_class("android/content/Intent").map_err(|_| ServiceError::Failed)?;
            let action =
                env.new_string("android.intent.action.VIEW").map_err(|_| ServiceError::Failed)?;
            let intent = env
                .new_object(&intent_class, "(Ljava/lang/String;)V", &[JValue::Object(&action)])
                .map_err(|_| ServiceError::Failed)?;
            let _ = env.call_method(
                &intent,
                "setData",
                "(Landroid/net/Uri;)Landroid/content/Intent;",
                &[JValue::Object(&uri)],
            );
            // No handler means the OS would raise `ActivityNotFoundException`; report the honest
            // `Denied` instead of a generic failure.
            let manager = env
                .call_method(
                    context,
                    "getPackageManager",
                    "()Landroid/content/pm/PackageManager;",
                    &[],
                )
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            let resolved = env
                .call_method(
                    &intent,
                    "resolveActivity",
                    "(Landroid/content/pm/PackageManager;)Landroid/content/ComponentName;",
                    &[JValue::Object(&manager)],
                )
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            if resolved.is_null() {
                return Err(ServiceError::Denied);
            }
            env.call_method(
                context,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&intent)],
            )
            .map(|_| ())
            .map_err(|_| ServiceError::Failed)
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }

    fn safe_area(&self) -> Result<Insets, ServiceError> {
        with_env(|env, context| {
            let window = env
                .call_method(context, "getWindow", "()Landroid/view/Window;", &[])
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            let decor = env
                .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            let insets = env
                .call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])
                .map_err(|_| ServiceError::Failed)?
                .l()
                .map_err(|_| ServiceError::Failed)?;
            // The framework delivers insets after the first layout pass. Zero is the honest
            // answer before then: inventing a status-bar height would crop content on a device
            // or a window that has none.
            if insets.is_null() {
                return Ok(Insets::ZERO);
            }
            Ok(Insets {
                top: system_inset(env, &insets, "getSystemWindowInsetTop")?,
                right: system_inset(env, &insets, "getSystemWindowInsetRight")?,
                bottom: system_inset(env, &insets, "getSystemWindowInsetBottom")?,
                left: system_inset(env, &insets, "getSystemWindowInsetLeft")?,
            })
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }

    fn set_orientation(&self, orientation: Orientation) -> Result<(), ServiceError> {
        // `ActivityInfo.SCREEN_ORIENTATION_*`: -1 unspecified, 1 portrait, 0 landscape. The
        // sub-variants (reverse, sensor, locked) are deliberately not exposed; see
        // [`Orientation`]'s documentation.
        let requested = match orientation {
            Orientation::Any => -1,
            Orientation::Portrait => 1,
            Orientation::Landscape => 0,
        };
        with_env(|env, context| {
            env.call_method(context, "setRequestedOrientation", "(I)V", &[JValue::Int(requested)])
                .map(|_| ())
                .map_err(|_| ServiceError::Failed)
        })
        .unwrap_or(Err(ServiceError::Unsupported))
    }
}

/// Reads one `WindowInsets.getSystemWindowInset*` value, as a non-negative pixel count.
///
/// The four getters are the API-20 spelling and are still present (deprecated) on current
/// releases. Using them avoids a `Build.VERSION` branch and a second set of JNI signatures for
/// the API-30 `getInsets(Type)` replacement; a negative value cannot mean an inset and is
/// clamped rather than passed through.
fn system_inset(
    env: &mut JNIEnv<'_>,
    insets: &JObject<'_>,
    getter: &str,
) -> Result<f32, ServiceError> {
    let value = env
        .call_method(insets, getter, "()I", &[])
        .map_err(|_| ServiceError::Failed)?
        .i()
        .map_err(|_| ServiceError::Failed)?;
    Ok(value.max(0) as f32)
}

fn permission_name(permission: Permission, sdk: i32) -> &'static str {
    match permission {
        Permission::Camera => "android.permission.CAMERA",
        Permission::Microphone => "android.permission.RECORD_AUDIO",
        // API 33 split media access into per-type permissions; older devices still use the
        // storage permission, and asking for the wrong one is a silent `Denied`.
        Permission::Photos if sdk >= 33 => "android.permission.READ_MEDIA_IMAGES",
        Permission::Photos => "android.permission.READ_EXTERNAL_STORAGE",
        Permission::Notifications => "android.permission.POST_NOTIFICATIONS",
    }
}

/// The Android backend. Construct through [`crate::services`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AndroidServices;
