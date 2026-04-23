use std::fmt::Debug;
use super::geometry::{Point, Rect, Size};
use super::alignment::Alignment;
use super::font::Font;

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
/// Platform family classification for backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFamily {
    /// Traditional desktop runtime targets.
    Desktop,
    /// Embedded and constrained runtime targets.
    Embedded,
    /// Mobile runtime targets.
    Mobile,
}
/// Common trait implemented by id-addressable core objects.
pub trait CoreObject: Debug + Send + Sync {
    /// Get stable object id.
    fn id(&self) -> ObjectId;
    /// Set stable object id (used by object system adapters).
    fn set_id(&mut self, id: ObjectId);
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
impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            Self::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}
impl std::error::Error for CoreError {}
/// Generic result type with default error.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;
/// Version information for compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
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
    /// Converts version to u32 (major.minor.patch packed).
    pub const fn to_u32(&self) -> u32 {
        ((self.major as u32) << 16) | ((self.minor as u32) << 8) | (self.patch as u32)
    }
    /// Creates version from string (e.g., "1.2.3").
    pub fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(CoreError::InvalidArgument(format!(
                "Invalid version format: {}",
                s
            )));
        }
        let major = parts[0]
            .parse()
            .map_err(|_| CoreError::InvalidArgument(format!("Invalid major version: {}", parts[0])))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| CoreError::InvalidArgument(format!("Invalid minor version: {}", parts[1])))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| CoreError::InvalidArgument(format!("Invalid patch version: {}", parts[2])))?;
        Ok(Self::new(major, minor, patch))
    }
    /// Converts version to string.
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
    /// Checks if this version is compatible with another (same major version).
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
}
/// Platform capabilities descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub has_gpu: bool,
    pub has_touch: bool,
    pub has_keyboard: bool,
    pub has_mouse: bool,
    pub screen_width: u32,
    pub screen_height: u32,
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
    pub profile: RuntimeProfile,
    pub platform: PlatformFamily,
    pub capabilities: PlatformCapabilities,
    pub version: Version,
}
impl CoreConfig {
    /// Creates default desktop configuration.
    pub fn desktop() -> Self {
        Self {
            profile: RuntimeProfile::Full,
            platform: PlatformFamily::Desktop,
            capabilities: PlatformCapabilities::desktop(),
            version: Version::new(0, 6, 1),
        }
    }
    /// Creates default embedded configuration.
    pub fn embedded() -> Self {
        Self {
            profile: RuntimeProfile::Embedded,
            platform: PlatformFamily::Embedded,
            capabilities: PlatformCapabilities::embedded(),
            version: Version::new(0, 6, 1),
        }
    }
    /// Creates default mobile configuration.
    pub fn mobile() -> Self {
        Self {
            profile: RuntimeProfile::Full,
            platform: PlatformFamily::Mobile,
            capabilities: PlatformCapabilities::mobile(),
            version: Version::new(0, 6, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_version_from_str() {
        let v = Version::from_str("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_from_str_invalid() {
        assert!(Version::from_str("1.2").is_err());
        assert!(Version::from_str("1.2.3.4").is_err());
        assert!(Version::from_str("a.b.c").is_err());
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
        assert_eq!(desktop.version.major, 0);
        assert_eq!(desktop.version.minor, 6);
        assert_eq!(desktop.version.patch, 1);

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
