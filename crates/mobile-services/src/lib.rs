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
//!
//! # Binding-layer boundary (principle #40)
//!
//! This crate **is** a language-/OS-binding layer, the one category principle #40 exempts from the
//! "no OS knowledge outside `src/platform/`" rule: every service it offers is an OS SDK call that
//! has no GUI-core meaning (haptics, local notifications, the share sheet, safe-area insets), and
//! the SDK types that carry those calls cannot cross into the platform trait without leaking native
//! types into the core. The exemption is bounded, and the bounds are enforced by the test below:
//!
//! * backend selection is the only place a target is named, and it answers at runtime through
//!   [`services`]; no caller ever picks a backend;
//! * the crate performs **no OS probing** — it reads no process table or sysfs, and it invokes
//!   no external command — so it can never fabricate a capability answer from the host;
//! * everything it reports is a three-state [`Status`], so an absent binding answers `Unsupported`
//!   rather than a plausible default.

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

    /// The binding-layer exemption is bounded as the module documentation claims: this crate
    /// reaches its host through SDK calls only, never by reading OS state or running a command.
    ///
    /// The needles are assembled at run time so the scan does not match this test's own body; a
    /// real occurrence anywhere in the crate still fails the test.
    #[test]
    fn the_binding_crate_contains_no_os_probing() {
        let sources = [
            include_str!("lib.rs"),
            include_str!("types.rs"),
            include_str!("unsupported.rs"),
            include_str!("android.rs"),
            include_str!("ios.rs"),
            include_str!("harmony.rs"),
        ];
        let needles = [
            ["/pro", "c"].concat(),
            ["/sy", "s/"].concat(),
            ["process::", "Command"].concat(),
            ["Command::", "new"].concat(),
        ];
        for (index, source) in sources.iter().enumerate() {
            for needle in &needles {
                assert!(
                    !source.contains(needle.as_str()),
                    "source #{index} probes the host with '{needle}', which the binding-layer \
                     exemption does not allow"
                );
            }
        }
    }
}
