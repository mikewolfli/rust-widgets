// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The honest backend for targets with no mobile service binding.

use crate::{
    HapticKind, Insets, MobileServices, NotificationRequest, Orientation, Permission, Service,
    ServiceError, ShareRequest, Status,
};

/// Answers `Unsupported` for every service.
///
/// # Why this type is public
///
/// A desktop host that compiles the crate still needs a value with the full trait so its own code
/// can be target-independent. Answering `Unsupported` is the trait's documented honest absence;
/// returning `Ok(())` would report an action that never happened.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedServices;

impl MobileServices for UnsupportedServices {
    fn availability(&self, _service: Service) -> Status {
        Status::Unsupported
    }

    fn keep_screen_on(&self, _on: bool) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }

    fn haptic(&self, _kind: HapticKind) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }

    fn permission_status(&self, _permission: Permission) -> Status {
        Status::Unsupported
    }

    fn request_permission(&self, _permission: Permission) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }

    fn notify(&self, _request: &NotificationRequest) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }

    fn share(&self, _request: &ShareRequest) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }

    fn open_url(&self, _url: &str) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }

    fn safe_area(&self) -> Result<Insets, ServiceError> {
        Err(ServiceError::Unsupported)
    }

    fn set_orientation(&self, _orientation: Orientation) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_is_an_explicit_unsupported() {
        let services = UnsupportedServices;
        assert_eq!(services.keep_screen_on(true), Err(ServiceError::Unsupported));
        assert_eq!(services.haptic(HapticKind::Light), Err(ServiceError::Unsupported));
        assert_eq!(services.request_permission(Permission::Camera), Err(ServiceError::Unsupported));
        assert_eq!(services.safe_area(), Err(ServiceError::Unsupported));
        assert_eq!(services.set_orientation(Orientation::Portrait), Err(ServiceError::Unsupported));
        assert_eq!(services.availability(Service::SafeArea), Status::Unsupported);
        assert_eq!(services.sensors(), Err(ServiceError::Unsupported));
        assert_eq!(services.location(), Err(ServiceError::Unsupported));
    }
}
