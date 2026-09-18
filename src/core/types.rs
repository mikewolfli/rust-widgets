// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::geometry::{Rect, Size};
use crate::compat::fmt::{Debug, Display};
use crate::compat::{format, String, Vec};

/// Stable numeric identifier used for widgets and objects.
pub type ObjectId = u64;
/// Runtime profile controlling feature and backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    /// Full desktop-oriented profile with optional advanced modules.
    Full,
    /// Reduced profile intended for constrained environments.
    Embedded,
}
/// Device form-factor classification used for touch target sizing and layout adaptation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// Desktop PC: large screen, mouse+keyboard, optionally touch.
    Desktop,
    /// Tablet: medium screen, touch-first.
    Tablet,
    /// Mobile phone: small screen, touch-first.
    Mobile,
    /// Embedded: constrained display, limited input.
    Embedded,
    /// Projector/projection: large read-only display, remote control input.
    Projector,
}

/// Platform family classification for backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFamily {
    /// Traditional desktop runtime targets.
    Desktop,
    /// Embedded and constrained runtime targets.
    Embedded,
    /// Mobile runtime targets.
    Mobile,
    /// Tablet runtime targets.
    Tablet,
    /// Projector/presentation runtime targets.
    Projector,
}
/// Common trait implemented by id-addressable core objects.
pub trait CoreObject: Debug + Send + Sync {
    /// Get stable object id.
    fn id(&self) -> ObjectId;
    /// Set stable object id (used by object system adapters).
    fn set_id(&mut self, id: ObjectId);
    /// Returns a human-readable type name for the core object.
    fn type_name(&self) -> &'static str;
}
/// Result type for core operations.
pub type CoreResult<T> = Result<T, CoreError>;
/// Error type for core operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// Invalid parameter or argument.
    InvalidArgument(String),
    /// Operation not supported.
    NotSupported(String),
    /// Resource not found.
    NotFound(String),
    /// Internal error.
    Internal(String),
}
impl crate::compat::fmt::Display for CoreError {
    fn fmt(&self, f: &mut crate::compat::fmt::Formatter<'_>) -> crate::compat::fmt::Result {
        match self {
            Self::InvalidArgument(msg) => write!(f, "Invalid argument: {msg}"),
            Self::NotSupported(msg) => write!(f, "Not supported: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}
impl core::error::Error for CoreError {}

impl From<crate::error::RwError> for CoreError {
    fn from(err: crate::error::RwError) -> Self {
        use crate::error::ErrorId;
        match err.id {
            ErrorId::INVALID_ARGUMENT => CoreError::InvalidArgument(err.message),
            ErrorId::UNSUPPORTED_OPERATION => CoreError::NotSupported(err.message),
            ErrorId::NOT_IMPLEMENTED => CoreError::NotSupported(err.message),
            ErrorId::FILE_NOT_FOUND => CoreError::NotFound(err.message),
            _ => CoreError::Internal(err.message),
        }
    }
}

/// Generic result type with default error.
pub type Result<T, E = CoreError> = core::result::Result<T, E>;
/// Version information for compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// Major version; incremented for incompatible API changes. Callers must
    /// not mix major versions — see [`Version::is_compatible_with`].
    pub major: u16,
    /// Minor version; incremented for backwards-compatible additions.
    pub minor: u16,
    /// Patch version; incremented for backwards-compatible fixes.
    pub patch: u16,
}
impl Version {
    /// Creates a new version.
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }
    /// Creates version from u32 (major.minor.patch packed).
    pub const fn from_u32(value: u32) -> Self {
        Self {
            major: ((value >> 16) & 0xFFFF) as u16,
            minor: ((value >> 8) & 0xFF) as u16,
            patch: (value & 0xFF) as u16,
        }
    }
    /// Converts version to u32 (`major:16 | minor:8 | patch:8` packed).
    ///
    /// Minor and patch values above 255 are saturated because the packed
    /// representation reserves only eight bits for each component.
    pub const fn to_u32(&self) -> u32 {
        ((self.major as u32) << 16)
            | (((if self.minor > u8::MAX as u16 { u8::MAX as u16 } else { self.minor }) as u32)
                << 8)
            | (if self.patch > u8::MAX as u16 { u8::MAX as u16 } else { self.patch }) as u32
    }
    /// Returns `true` if both versions share a major component.
    ///
    /// Compatibility is major-only in both directions: `1.0.0` and `1.5.0` are
    /// compatible, `1.0.0` and `2.0.0` are not. Minor and patch are ignored, so
    /// this does not tell you whether `other` is newer or older.
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major
    }
    /// Checks if this version is newer than another.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        (self.major > other.major)
            || (self.major == other.major && self.minor > other.minor)
            || (self.major == other.major && self.minor == other.minor && self.patch > other.patch)
    }
    /// Checks if this version is older than another.
    pub fn is_older_than(&self, other: &Self) -> bool {
        other.is_newer_than(self)
    }

    /// Parses a version string in "major.minor.patch" format.
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid version format: '{s}'. Expected 'major.minor.patch'"));
        }
        let major = parts[0].parse::<u16>().map_err(|e| format!("Invalid major version: {e}"))?;
        let minor = parts[1].parse::<u16>().map_err(|e| format!("Invalid minor version: {e}"))?;
        let patch = parts[2].parse::<u16>().map_err(|e| format!("Invalid patch version: {e}"))?;
        Ok(Self { major, minor, patch })
    }
}
impl core::str::FromStr for Version {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_str(s)
    }
}
impl Display for Version {
    fn fmt(&self, f: &mut crate::compat::fmt::Formatter<'_>) -> crate::compat::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
/// Hardware facts about the host, probed once at startup.
///
/// # Not the same as [`crate::platform::PlatformCapabilities`]
///
/// Both are called `PlatformCapabilities` but describe different things
/// (principle #49):
///
/// * this one — **what the machine has** (GPU, touch, screen size, DPI);
/// * [`crate::platform::PlatformCapabilities`] — **what the active backend can
///   do** (IME, accessibility bridge, native menus, typed trigger events).
#[derive(Debug, Clone, PartialEq)]
pub struct PlatformCapabilities {
    /// Whether a GPU (any accelerated rasteriser) is present. Gates the
    /// hardware-accelerated render backends; a `false` value means software
    /// rasterisation only.
    pub has_gpu: bool,
    /// Whether the primary input device supports touch. Drives touch target
    /// sizing (see [`crate::core::geometry::Rect::expand_to_touch_target`]).
    pub has_touch: bool,
    /// Whether a physical or on-screen keyboard is available; a `false` value
    /// means text-entry widgets cannot be focused usefully.
    pub has_keyboard: bool,
    /// Whether a pointing device (mouse, trackpad, or stylus with hover) is
    /// available for hover and fine-grained hit testing.
    pub has_mouse: bool,
    /// Primary screen width in physical pixels (not logical/device-independent
    /// units); see `dpi_scale` for the conversion factor.
    pub screen_width: u32,
    /// Primary screen height in physical pixels (not logical units).
    pub screen_height: u32,
    /// Ratio of physical pixels to logical pixels (1.0 = 96 DPI baseline,
    /// 2.0 = HiDPI). Multiply logical sizes by this to obtain physical pixels.
    pub dpi_scale: f32,
}
impl PlatformCapabilities {
    /// Creates default desktop capabilities.
    pub fn desktop() -> Self {
        Self {
            has_gpu: true,
            has_touch: false,
            has_keyboard: true,
            has_mouse: true,
            screen_width: 1920,
            screen_height: 1080,
            dpi_scale: 1.0,
        }
    }
    /// Creates default embedded capabilities.
    pub fn embedded() -> Self {
        Self {
            has_gpu: false,
            has_touch: true,
            has_keyboard: false,
            has_mouse: false,
            screen_width: 800,
            screen_height: 480,
            dpi_scale: 1.0,
        }
    }
    /// Creates default mobile capabilities.
    pub fn mobile() -> Self {
        Self {
            has_gpu: true,
            has_touch: true,
            has_keyboard: false,
            has_mouse: false,
            screen_width: 1080,
            screen_height: 1920,
            dpi_scale: 2.0,
        }
    }
    /// Returns screen size as Size.
    pub fn screen_size(&self) -> Size {
        Size::new(self.screen_width, self.screen_height)
    }
    /// Returns screen rectangle.
    pub fn screen_rect(&self) -> Rect {
        Rect::new(0, 0, self.screen_width, self.screen_height)
    }
}
/// Configuration for core initialization.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreConfig {
    /// Feature set to enable at runtime. [`RuntimeProfile::Embedded`] skips the
    /// optional advanced modules that a constrained device cannot afford.
    pub profile: RuntimeProfile,
    /// Which platform family the runtime is being initialised for; selects the
    /// default backend implementations when none are injected.
    pub platform: PlatformFamily,
    /// Probed hardware facts for this host. Must agree with `platform`, but is
    /// not validated — callers are responsible for consistency.
    pub capabilities: PlatformCapabilities,
    /// Core library version the caller was compiled against, used for the
    /// major-only compatibility check in [`Version::is_compatible_with`].
    ///
    /// Populated from [`CoreConfig::library_version`], which reads the crate's own
    /// `CARGO_PKG_VERSION` so this cannot drift from the released version.
    pub version: Version,
}
impl CoreConfig {
    /// The library version this build was compiled from.
    ///
    /// Derived from `CARGO_PKG_VERSION` rather than written by hand, so the
    /// compatibility handshake cannot report a version the crate no longer is. A
    /// hand-written literal here went stale for four minor releases while three
    /// cookbook translations documented the output as the current version.
    pub fn library_version() -> Version {
        Version::parse_str(env!("CARGO_PKG_VERSION"))
            .expect("CARGO_PKG_VERSION is always a valid major.minor.patch version")
    }

    /// Creates default desktop configuration.
    pub fn desktop() -> Self {
        Self {
            profile: RuntimeProfile::Full,
            platform: PlatformFamily::Desktop,
            capabilities: PlatformCapabilities::desktop(),
            version: Self::library_version(),
        }
    }
    /// Creates default embedded configuration.
    pub fn embedded() -> Self {
        Self {
            profile: RuntimeProfile::Embedded,
            platform: PlatformFamily::Embedded,
            capabilities: PlatformCapabilities::embedded(),
            version: Self::library_version(),
        }
    }
    /// Creates default mobile configuration.
    pub fn mobile() -> Self {
        Self {
            profile: RuntimeProfile::Full,
            platform: PlatformFamily::Mobile,
            capabilities: PlatformCapabilities::mobile(),
            version: Self::library_version(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::MiniToString;

    #[test]
    fn test_version_creation() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_from_u32() {
        let v = Version::from_u32(0x010203);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_to_u32() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_u32(), 0x010203);
    }

    #[test]
    fn test_version_packing_saturates_narrow_components() {
        let v = Version::new(1, u16::MAX, u16::MAX);
        assert_eq!(v.to_u32(), 0x01FFFF);
        assert_eq!(Version::from_u32(v.to_u32()), Version::new(1, 255, 255));
    }

    #[test]
    fn test_version_parse_str() {
        let v = Version::parse_str("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_parse_str_invalid() {
        assert!(Version::parse_str("1.2").is_err());
        assert!(Version::parse_str("1.2.3.4").is_err());
        assert!(Version::parse_str("a.b.c").is_err());
    }

    #[test]
    fn test_version_compatibility() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 5, 0);
        let v3 = Version::new(2, 0, 0);

        assert!(v1.is_compatible_with(&v2));
        assert!(!v1.is_compatible_with(&v3));
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(1, 1, 1);

        assert!(v2.is_newer_than(&v1));
        assert!(v3.is_newer_than(&v2));
        assert!(v1.is_older_than(&v2));
    }

    #[test]
    fn test_platform_capabilities() {
        let desktop = PlatformCapabilities::desktop();
        assert!(desktop.has_gpu);
        assert!(desktop.has_keyboard);
        assert!(desktop.has_mouse);
        assert!(!desktop.has_touch);
        assert_eq!(desktop.screen_width, 1920);
        assert_eq!(desktop.screen_height, 1080);
        assert_eq!(desktop.dpi_scale, 1.0);

        let embedded = PlatformCapabilities::embedded();
        assert!(!embedded.has_gpu);
        assert!(embedded.has_touch);
        assert!(!embedded.has_keyboard);
        assert!(!embedded.has_mouse);
        assert_eq!(embedded.screen_width, 800);
        assert_eq!(embedded.screen_height, 480);
        assert_eq!(embedded.dpi_scale, 1.0);

        let mobile = PlatformCapabilities::mobile();
        assert!(mobile.has_gpu);
        assert!(mobile.has_touch);
        assert!(!mobile.has_keyboard);
        assert!(!mobile.has_mouse);
        assert_eq!(mobile.screen_width, 1080);
        assert_eq!(mobile.screen_height, 1920);
        assert_eq!(mobile.dpi_scale, 2.0);
    }

    #[test]
    fn test_platform_capabilities_screen_size() {
        let caps = PlatformCapabilities::desktop();
        let size = caps.screen_size();
        assert_eq!(size.width, 1920);
        assert_eq!(size.height, 1080);

        let rect = caps.screen_rect();
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 1920);
        assert_eq!(rect.height, 1080);
    }

    #[test]
    fn test_core_config() {
        let desktop = CoreConfig::desktop();
        assert_eq!(desktop.profile, RuntimeProfile::Full);
        assert_eq!(desktop.platform, PlatformFamily::Desktop);
        // Must equal the crate's own version, not a literal that goes stale
        // independently of `Cargo.toml`.
        assert_eq!(desktop.version, CoreConfig::library_version());
        assert_eq!(desktop.version.major, env!("CARGO_PKG_VERSION_MAJOR").parse::<u16>().unwrap());

        let embedded = CoreConfig::embedded();
        assert_eq!(embedded.profile, RuntimeProfile::Embedded);
        assert_eq!(embedded.platform, PlatformFamily::Embedded);

        let mobile = CoreConfig::mobile();
        assert_eq!(mobile.profile, RuntimeProfile::Full);
        assert_eq!(mobile.platform, PlatformFamily::Mobile);
    }

    #[test]
    fn test_core_error_display() {
        let err = CoreError::InvalidArgument("test".to_string());
        assert_eq!(format!("{}", err), "Invalid argument: test");

        let err = CoreError::NotSupported("test".to_string());
        assert_eq!(format!("{}", err), "Not supported: test");

        let err = CoreError::NotFound("test".to_string());
        assert_eq!(format!("{}", err), "Not found: test");

        let err = CoreError::Internal("test".to_string());
        assert_eq!(format!("{}", err), "Internal error: test");
    }

    #[test]
    fn test_runtime_profile() {
        assert_eq!(RuntimeProfile::Full, RuntimeProfile::Full);
        assert_eq!(RuntimeProfile::Embedded, RuntimeProfile::Embedded);
        assert_ne!(RuntimeProfile::Full, RuntimeProfile::Embedded);
    }

    #[test]
    fn test_platform_family() {
        assert_eq!(PlatformFamily::Desktop, PlatformFamily::Desktop);
        assert_eq!(PlatformFamily::Embedded, PlatformFamily::Embedded);
        assert_eq!(PlatformFamily::Mobile, PlatformFamily::Mobile);
        assert_ne!(PlatformFamily::Desktop, PlatformFamily::Embedded);
    }
}
