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
    /// The window's safe-area insets (notch, status bar, home indicator).
    SafeArea,
    /// Lock the window to portrait or landscape.
    Orientation,
    /// Device sensors. **Deliberately unsupported by this crate**: the widget library does not
    /// hold sensor/health permissions.
    Sensors,
    /// Geographic location. **Deliberately unsupported by this crate**: location belongs to the
    /// application, which owns the permission prompt and the privacy disclosure.
    Location,
}

/// A window's safe-area insets, in the platform's own units.
///
/// Android reports pixels, iOS points and OpenHarmony vp; the values are passed through
/// unchanged rather than rescaled, because the only code that can scale them correctly is the
/// host that owns the display density. A layout uses them as "content must stay inside the
/// rectangle this much smaller than the window".
///
/// The type is `Copy` and has a `ZERO` constant, so a host can treat "no inset reported" and
/// "no inset needed" the same way without an `Option` at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    /// Inset at the top edge (status bar, notch).
    pub top: f32,
    /// Inset at the right edge (rounded corner in landscape).
    pub right: f32,
    /// Inset at the bottom edge (home indicator, navigation bar).
    pub bottom: f32,
    /// Inset at the left edge.
    pub left: f32,
}

impl Insets {
    /// No inset on any edge.
    pub const ZERO: Self = Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };

    /// `true` when every edge is zero.
    pub fn is_zero(&self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }
}

/// A requested screen orientation.
///
/// Three values, not the platform's full enum: every platform expresses "portrait" and
/// "landscape" the same way, while the sub-variants (`reversePortrait`, `sensorLandscape`, …)
/// differ per OS and would either be lossy or need a second, platform-shaped enum. A host that
/// needs a sub-variant locks the coarse axis here and refines in its manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Let the system decide (the platform's `unspecified`/`any` value).
    Any,
    /// Portrait only.
    Portrait,
    /// Landscape only.
    Landscape,
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

    /// Reads the window's current safe-area insets.
    ///
    /// Returns [`Insets::ZERO`] when the platform reports none (a desktop-sized window, or
    /// insets not yet delivered). Sealed before the first layout pass, a zero answer is the
    /// only honest one — inventing a status-bar height would crop content on a device without
    /// a status bar.
    fn safe_area(&self) -> Result<Insets, ServiceError>;

    /// Locks the window to `orientation`, or releases the lock with [`Orientation::Any`].
    fn set_orientation(&self, orientation: Orientation) -> Result<(), ServiceError>;

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
