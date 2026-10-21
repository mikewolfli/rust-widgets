// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Optional mobile application services (BLUE28 B28.44).
//!
//! These are **application-shell** capabilities, not GUI core: keeping the screen awake, haptics,
//! permission queries, local notifications, the share sheet, deep links, safe-area insets,
//! orientation lock, and (deliberately unsupported here) sensors/location. They live in their own
//! crate so a desktop build never links them and a mobile host opts in explicitly:
//!
//! ```toml
//! [dependencies]
//! rust-widgets-mobile-services = { path = "../rust-widgets/crates/mobile-services" }
//! ```
//!
//! # The three-state rule
//!
//! Every service answers [`Status::Available`], [`Status::Denied`] or [`Status::Unsupported`].
//! Actions return `Result<(), ServiceError>`; a failure is never reported as success. A request
//! that would show an OS permission prompt is a **separate** call
//! ([`MobileServices::request_permission`]); querying status and sending a notification never
//! trigger a prompt implicitly (principle: no hidden permission side effects).
//!
//! # Platform selection
//!
//! [`services`] returns the backend for the target: Android JNI, iOS UIKit, or the honest
//! unsupported implementation on desktop and every target without a binding. Android needs the
//! host to call [`android::attach`] with its `JavaVM` and Activity `Context`; without that
//! attachment every action answers `Unsupported` rather than a fabricated success.

mod types;

#[cfg(target_os = "android")]
pub mod android;
/// HarmonyOS services, forwarded to the ArkTS host (BLUE28 B28.44).
///
/// Compiled on every host so its contract and tests are visible; [`services`] selects it only on
/// an OpenHarmony target. A host installs its ArkTS-backed functions with [`harmony::attach`].
pub mod harmony;
#[cfg(target_os = "ios")]
pub mod ios;
mod unsupported;

pub use types::{
    validate_url, HapticKind, Insets, MobileServices, NotificationRequest, Orientation, Permission,
    Service, ServiceError, ShareRequest, Status,
};

/// The mobile-services backend for this target.
///
/// On a desktop host this is [`unsupported::UnsupportedServices`], which answers `Unsupported`
/// for every service. The function exists on every target so a host can write one call site.
pub fn services() -> Box<dyn MobileServices> {
    #[cfg(target_os = "android")]
    {
        return Box::new(android::AndroidServices);
    }
    #[cfg(target_os = "ios")]
    {
        return Box::new(ios::IosServices);
    }
    #[cfg(target_env = "ohos")]
    {
        return Box::new(harmony::HarmonyServices);
    }
    #[allow(unreachable_code)]
    Box::new(unsupported::UnsupportedServices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_desktop_host_answers_unsupported_not_a_fake_success() {
        let services = services();
        assert_eq!(services.availability(Service::KeepAwake), Status::Unsupported);
        assert_eq!(services.keep_screen_on(true), Err(ServiceError::Unsupported));
    }

    #[test]
    fn url_validation_refuses_control_characters_and_empty_targets() {
        assert!(validate_url("https://example.com/a?b=1"));
        assert!(validate_url("mailto:user@example.com"));
        assert!(!validate_url(""));
        assert!(!validate_url("javascript:alert(1)"));
        assert!(!validate_url("https://example.com/\nnext"));
    }
}
