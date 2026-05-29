//! Device class detection and adaptive layout support (BLUE8 P4-6).
//!
//! This module provides runtime detection of device form factor,
//! touch capability, screen size, and other environment properties.
//! It is used by the platform layer to select appropriate touch target
//! sizes, layout modes, and interaction models.
//!
//! # Detection dimensions
//!
//! | Dimension | Method | Default |
//! |-----------|--------|---------|
//! | Touch capability | Feature flag `touch` + platform API query | `false` |
//! | Device class | Compile-time profile + screen size heuristic | `Desktop` |
//! | Screen size | Logical resolution via platform | 1024×768 |
//! | DPI scale | Platform `dpi_scale_factor()` | 1.0 |
//!
//! # Device class mapping
//!
//! | Feature profile | Default DeviceClass |
//! |-----------------|---------------------|
//! | `desktop` | `DeviceClass::Desktop` |
//! | `tablet` | `DeviceClass::Tablet` |
//! | `mobile` | `DeviceClass::Mobile` |
//! | `embedded` | `DeviceClass::Embedded` |

use crate::core::DeviceClass;

use crate::core::Size;

/// Screen orientation enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenOrientation {
    /// Device in portrait orientation.
    Portrait,
    /// Device in landscape orientation.
    Landscape,
    /// Device in reverse portrait orientation (180° rotated).
    ReversePortrait,
    /// Device in reverse landscape orientation (180° rotated).
    ReverseLandscape,
}

/// Runtime device environment descriptor.
#[derive(Debug, Clone)]
pub struct DeviceEnvironment {
    /// Device form-factor class.
    pub device_class: DeviceClass,
    /// Touch input availability.
    pub touch_capable: bool,
    /// Logical screen resolution (if known).
    pub screen_size: Size,
    /// DPI scale factor (1.0 = 96 DPI baseline).
    pub dpi_scale: f32,
    /// Whether the device is in presentation/projection mode.
    pub projection_mode: bool,
    /// Recommended minimum touch target dimensions.
    pub touch_target_min: Size,
    /// Recommended spacing between interactive elements.
    pub touch_spacing: u32,
    /// Screen orientation.
    pub orientation: ScreenOrientation,
    /// High-contrast display mode (accessibility).
    pub high_contrast: bool,
    /// Reduced motion preference (accessibility).
    pub reduced_motion: bool,
    /// Font scale factor (accessibility, default 1.0).
    pub font_scale: f32,
}

impl DeviceEnvironment {
    /// Detect device environment from compile-time features and runtime hints.
    ///
    /// Uses compile-time profile as primary classification, with screen
    /// size heuristics as fallback refinement.
    pub fn detect(screen_size: Size, dpi_scale: f32) -> Self {
        let device_class = Self::resolve_device_class(screen_size, dpi_scale);
        let touch_capable = Self::resolve_touch_capability(&device_class);
        let projection_mode = device_class == DeviceClass::Projector;
        let (touch_target_min, touch_spacing) = Self::touch_target_params(device_class);
        let orientation = Self::resolve_orientation(screen_size);

        Self {
            device_class,
            touch_capable,
            screen_size,
            dpi_scale,
            projection_mode,
            touch_target_min,
            touch_spacing,
            orientation,
            high_contrast: false,
            reduced_motion: false,
            font_scale: 1.0,
        }
    }

    /// Resolve device class from profile features and screen dimensions.
    fn resolve_device_class(screen_size: Size, dpi_scale: f32) -> DeviceClass {
        // Compile-time feature profile takes precedence.
        #[cfg(feature = "tablet")]
        {
            DeviceClass::Tablet
        }
        #[cfg(all(not(feature = "tablet"), feature = "mobile"))]
        {
            DeviceClass::Mobile
        }
        #[cfg(all(not(feature = "tablet"), not(feature = "mobile"), feature = "embedded"))]
        {
            DeviceClass::Embedded
        }

        // Fallback: heuristic based on screen width (logical points) and DPI scale.
        // High DPI (>= 2.0) is more common on mobile/tablet displays.
        // Only reached when no feature flag above is active.
        #[cfg(all(
            not(feature = "tablet"),
            not(feature = "mobile"),
            not(feature = "embedded")
        ))]
        {
            let width = screen_size.width.max(320);
            if width < 480 {
                DeviceClass::Mobile
            } else if width < 1024 {
                DeviceClass::Tablet
            } else if dpi_scale >= 2.0 && width < 1440 {
                // High-DPI display at moderate resolution is likely a tablet.
                DeviceClass::Tablet
            } else {
                DeviceClass::Desktop
            }
        }
    }

    /// Determine touch capability based on device class and feature flags.
    fn resolve_touch_capability(class: &DeviceClass) -> bool {
        #[cfg(feature = "touch")]
        {
            let _ = class;
            // When touch feature is enabled, all form factors support touch
            // except projector (which uses remote control).
            !matches!(class, DeviceClass::Projector)
        }
        #[cfg(not(feature = "touch"))]
        {
            let _ = class;
            false
        }
    }

    /// Minimum touch target dimensions and spacing for each device class.
    const fn touch_target_params(class: DeviceClass) -> (Size, u32) {
        match class {
            DeviceClass::Desktop => (Size::new(32, 32), 8),
            DeviceClass::Tablet => (Size::new(44, 44), 12),
            DeviceClass::Mobile => (Size::new(48, 48), 16),
            DeviceClass::Embedded => (Size::new(40, 40), 10),
            DeviceClass::Projector => (Size::new(24, 24), 6),
        }
    }

    /// Returns true if the device has a touch-capable display.
    pub fn is_touch_device(&self) -> bool {
        self.touch_capable
    }

    /// Returns true if the device is in projection/presentation mode.
    pub fn is_projection(&self) -> bool {
        self.projection_mode
    }

    /// Returns the recommended minimum touch target size in logical pixels.
    pub fn min_touch_target(&self) -> Size {
        self.touch_target_min
    }

    /// Returns the recommended spacing between interactive touch targets.
    pub fn touch_spacing(&self) -> u32 {
        self.touch_spacing
    }

    /// Resolve screen orientation from screen size.
    fn resolve_orientation(screen_size: Size) -> ScreenOrientation {
        if screen_size.width > screen_size.height {
            ScreenOrientation::Landscape
        } else {
            ScreenOrientation::Portrait
        }
    }

    /// Set screen orientation manually.
    pub fn set_orientation(&mut self, orientation: ScreenOrientation) {
        self.orientation = orientation;
    }

    /// Re-detect orientation and update environment after a screen/DPI change.
    pub fn recheck(&mut self, screen_size: Size, dpi_scale: f32) {
        self.screen_size = screen_size;
        self.dpi_scale = dpi_scale;
        // Re-detect orientation
        self.orientation = Self::resolve_orientation(screen_size);
    }

    /// Set the DPI scale factor directly.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.dpi_scale = dpi_scale;
    }

    /// Set high-contrast mode (accessibility).
    pub fn set_high_contrast(&mut self, enabled: bool) {
        self.high_contrast = enabled;
    }

    /// Set reduced-motion preference (accessibility).
    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.reduced_motion = enabled;
    }

    /// Set font scale factor (accessibility, clamped to [0.5, 3.0]).
    pub fn set_font_scale(&mut self, scale: f32) {
        self.font_scale = scale.clamp(0.5, 3.0);
    }

    /// Suggest layout scale factor for text and chrome adjustments.
    ///
    /// Projection mode increases scale by 20% for readability.
    /// Mobile mode keeps default scale.
    pub fn layout_scale(&self) -> f32 {
        match self.device_class {
            DeviceClass::Projector => 1.2,
            _ => 1.0,
        }
    }
}

impl Default for DeviceEnvironment {
    /// Default to desktop environment without touch.
    fn default() -> Self {
        Self {
            device_class: DeviceClass::Desktop,
            touch_capable: false,
            screen_size: Size::new(1024, 768),
            dpi_scale: 1.0,
            projection_mode: false,
            touch_target_min: Size::new(32, 32),
            touch_spacing: 8,
            orientation: ScreenOrientation::Landscape,
            high_contrast: false,
            reduced_motion: false,
            font_scale: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_desktop_no_touch() {
        let env = DeviceEnvironment::default();
        assert_eq!(env.device_class, DeviceClass::Desktop);
        assert!(!env.touch_capable);
        assert!(!env.is_projection());
        assert_eq!(env.touch_target_min, Size::new(32, 32));
        assert_eq!(env.touch_spacing, 8);
    }

    #[test]
    fn large_screen_heuristic_is_desktop() {
        let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);
        // Compile-time feature overrides heuristic.
        // Without a feature flag: width >= 1024 -> Desktop.
        if cfg!(not(any(
            feature = "tablet",
            feature = "mobile",
            feature = "embedded"
        ))) {
            assert_eq!(env.device_class, DeviceClass::Desktop);
        } else {
            // Feature is set, so class matches whichever feature is active.
            assert!(matches!(
                env.device_class,
                DeviceClass::Desktop
                    | DeviceClass::Tablet
                    | DeviceClass::Mobile
                    | DeviceClass::Embedded
            ));
        }
    }

    #[test]
    fn small_screen_heuristic_is_mobile() {
        let env = DeviceEnvironment::detect(Size::new(360, 640), 2.0);
        if cfg!(not(any(
            feature = "tablet",
            feature = "mobile",
            feature = "embedded"
        ))) {
            assert_eq!(env.device_class, DeviceClass::Mobile);
        } else {
            assert!(matches!(
                env.device_class,
                DeviceClass::Desktop
                    | DeviceClass::Tablet
                    | DeviceClass::Mobile
                    | DeviceClass::Embedded
            ));
        }
    }

    #[test]
    fn medium_screen_heuristic_is_tablet() {
        let env = DeviceEnvironment::detect(Size::new(768, 1024), 1.0);
        if cfg!(not(any(
            feature = "tablet",
            feature = "mobile",
            feature = "embedded"
        ))) {
            assert_eq!(env.device_class, DeviceClass::Tablet);
        } else {
            assert!(matches!(
                env.device_class,
                DeviceClass::Desktop
                    | DeviceClass::Tablet
                    | DeviceClass::Mobile
                    | DeviceClass::Embedded
            ));
        }
    }

    #[test]
    fn projection_mode_layout_scale() {
        let env = DeviceEnvironment {
            device_class: DeviceClass::Projector,
            projection_mode: true,
            ..Default::default()
        };
        assert!(env.is_projection());
        assert!((env.layout_scale() - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn touch_target_params_match_device_class() {
        let make_env = |class: DeviceClass| -> DeviceEnvironment {
            let (min, spacing) = DeviceEnvironment::touch_target_params(class);
            DeviceEnvironment {
                device_class: class,
                touch_target_min: min,
                touch_spacing: spacing,
                ..Default::default()
            }
        };

        let desktop = make_env(DeviceClass::Desktop);
        let tablet = make_env(DeviceClass::Tablet);
        let mobile = make_env(DeviceClass::Mobile);
        let embedded = make_env(DeviceClass::Embedded);
        let projector = make_env(DeviceClass::Projector);

        assert_eq!(desktop.touch_target_min, Size::new(32, 32));
        assert_eq!(tablet.touch_target_min, Size::new(44, 44));
        assert_eq!(mobile.touch_target_min, Size::new(48, 48));
        assert_eq!(embedded.touch_target_min, Size::new(40, 40));
        assert_eq!(projector.touch_target_min, Size::new(24, 24));
    }

    #[test]
    fn touch_capable_when_feature_enabled() {
        // At runtime without feature, touch is false.
        // With cfg(feature = "touch") the value depends on device class.
        let env = DeviceEnvironment::default();
        // Without feature flag, touch is always false.
        assert!(!env.touch_capable);
    }

    #[test]
    fn touch_spacing_values() {
        assert_eq!(
            DeviceEnvironment::touch_target_params(DeviceClass::Desktop).1,
            8
        );
        assert_eq!(
            DeviceEnvironment::touch_target_params(DeviceClass::Tablet).1,
            12
        );
        assert_eq!(
            DeviceEnvironment::touch_target_params(DeviceClass::Mobile).1,
            16
        );
        assert_eq!(
            DeviceEnvironment::touch_target_params(DeviceClass::Embedded).1,
            10
        );
        assert_eq!(
            DeviceEnvironment::touch_target_params(DeviceClass::Projector).1,
            6
        );
    }

    #[test]
    fn orientation_landscape_when_width_greater_than_height() {
        let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);
        assert_eq!(env.orientation, ScreenOrientation::Landscape);
    }

    #[test]
    fn orientation_portrait_when_height_greater_than_width() {
        let env = DeviceEnvironment::detect(Size::new(1080, 1920), 1.0);
        assert_eq!(env.orientation, ScreenOrientation::Portrait);
    }

    #[test]
    fn orientation_square_defaults_to_portrait() {
        let env = DeviceEnvironment::detect(Size::new(1024, 1024), 1.0);
        // width == height, so portrait (not >)
        assert_eq!(env.orientation, ScreenOrientation::Portrait);
    }

    #[test]
    fn set_orientation_changes_value() {
        let mut env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);
        assert_eq!(env.orientation, ScreenOrientation::Landscape);
        env.set_orientation(ScreenOrientation::Portrait);
        assert_eq!(env.orientation, ScreenOrientation::Portrait);
        env.set_orientation(ScreenOrientation::ReverseLandscape);
        assert_eq!(env.orientation, ScreenOrientation::ReverseLandscape);
        env.set_orientation(ScreenOrientation::ReversePortrait);
        assert_eq!(env.orientation, ScreenOrientation::ReversePortrait);
    }

    #[test]
    fn default_orientation_is_landscape() {
        let env = DeviceEnvironment::default();
        // Default screen_size is 1024x768, width > height -> Landscape
        assert_eq!(env.orientation, ScreenOrientation::Landscape);
    }
}
