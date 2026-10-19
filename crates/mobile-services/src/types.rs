// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Service vocabulary shared by every platform backend.

/// One optional application service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    /// Keep the display awake while the app is foreground.
    KeepAwake,
    /// Short haptic feedback.
    Haptics,
    /// A permission query/request (the camera, microphone, …).
    Permissions,
    /// Local notifications.
    Notifications,
    /// The system share sheet.
    Share,
    /// Open a URL or a custom scheme (deep link).
    DeepLink,
    /// Device sensors. **Deliberately unsupported by this crate**: the widget library does not
    /// hold sensor/health permissions.
    Sensors,
    /// Geographic location. **Deliberately unsupported by this crate**: location belongs to the
    /// application, which owns the permission prompt and the privacy disclosure.
    Location,
}

/// The three states an application service can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// The platform exposes the API and the app is allowed to use it.
    Available,
    /// The platform exposes the API, but the user (or the system) denied it.
    Denied,
    /// This target has no binding for the service, or the service is out of scope for this crate.
    Unsupported,
}

/// Why an action did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    /// The user or the system denied the operation.
    Denied,
    /// This target has no binding, or the Android host has not attached its Context.
    Unsupported,
    /// The platform call failed (a JNI exception, a missing view controller, …).
    Failed,
}

/// The strength of a haptic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HapticKind {
    /// A light tap.
    Light,
    /// A medium impact.
    Medium,
    /// A heavy impact.
    Heavy,
    /// A success notification.
    Success,
    /// A warning notification.
    Warning,
    /// An error notification.
    Error,
}

/// A permission this crate can ask about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Camera access.
    Camera,
    /// Microphone access.
    Microphone,
    /// Photo library access.
    Photos,
    /// Local notification authorization.
    Notifications,
}

/// The share sheet's payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRequest {
    /// Text to share.
    pub text: String,
    /// Optional URL shared alongside the text.
    pub url: Option<String>,
    /// Optional chooser title.
    pub title: Option<String>,
}

/// A local notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationRequest {
    /// Stable id; a second notification with the same id replaces the first.
    pub id: i32,
    /// Title line.
    pub title: String,
    /// Body text.
    pub body: String,
    /// Android drawable resource id for the small icon. `None` makes Android use its own
    /// `android.R.drawable.ic_dialog_info`, which every device has.
    pub icon_resource: Option<i32>,
}

/// The optional services a mobile host can ask for.
///
/// Every method is fallible and every failure is named. A backend with no binding implements
/// every action as `Err(ServiceError::Unsupported)` rather than silently doing nothing.
pub trait MobileServices: Send + Sync {
    /// The current state of one service.
    fn availability(&self, service: Service) -> Status;

    /// Turns the keep-awake flag on or off.
    fn keep_screen_on(&self, on: bool) -> Result<(), ServiceError>;

    /// Plays one haptic.
    fn haptic(&self, kind: HapticKind) -> Result<(), ServiceError>;

    /// Reads a permission without prompting.
    fn permission_status(&self, permission: Permission) -> Status;

    /// Requests a permission. **This is the only call that may show an OS prompt.**
    fn request_permission(&self, permission: Permission) -> Result<(), ServiceError>;

    /// Posts a local notification. Does **not** request notification permission.
    fn notify(&self, request: &NotificationRequest) -> Result<(), ServiceError>;

    /// Shows the system share sheet.
    fn share(&self, request: &ShareRequest) -> Result<(), ServiceError>;

    /// Opens an `http`/`https`/`mailto`/`tel` URL, or a custom scheme.
    ///
    /// `javascript:` and `data:` are refused: they execute content rather than open a handler.
    fn open_url(&self, url: &str) -> Result<(), ServiceError>;

    /// Sensor access. Always `Err(Unsupported)` in this crate; see [`Service::Sensors`].
    fn sensors(&self) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }

    /// Location access. Always `Err(Unsupported)` in this crate; see [`Service::Location`].
    fn location(&self) -> Result<(), ServiceError> {
        Err(ServiceError::Unsupported)
    }
}

/// Whether `url` is a target the OS handler may be given.
///
/// Accepts a scheme followed by `:` (`https:`, `mailto:`, `tel:`, a custom app scheme) and
/// refuses empty input, control characters, and the two schemes that execute content
/// (`javascript:`, `data:`). The rule is shared by every backend so iOS and Android cannot drift.
pub fn validate_url(url: &str) -> bool {
    if url.is_empty() || url.chars().any(char::is_control) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    if rest.is_empty() || !scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    if !scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return false;
    }
    !matches!(scheme.to_ascii_lowercase().as_str(), "javascript" | "data")
}
