use super::*;

/// Public software render configuration for quality-related knobs.
///
/// This type is `Copy` and cheap to pass around; a [`SoftwareSurface`] inherits
/// the process-wide default returned by [`default_software_render_config`] at
/// construction time unless [`SoftwareSurface::apply_render_config`] overrides
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareRenderConfig {
    /// Number of anti-aliasing samples taken along each axis, forming an
    /// `n x n` sample grid. Higher values cost more per primitive. The valid
    /// range is `1..=8` (where `1` disables anti-aliasing); out-of-range values
    /// are silently clamped by [`SoftwareRenderConfig::normalized`] or
    /// [`SoftwareSurface::apply_render_config`]. Defaults to `4`.
    pub aa_samples_per_axis: u8,
}
impl Default for SoftwareRenderConfig {
    fn default() -> Self {
        Self { aa_samples_per_axis: 4 }
    }
}
impl SoftwareRenderConfig {
    /// Returns a copy with every value clamped into its documented range.
    ///
    /// Currently this only clamps `aa_samples_per_axis` to `1..=8`.
    pub fn normalized(self) -> Self {
        Self { aa_samples_per_axis: self.aa_samples_per_axis.clamp(1, 8) }
    }
}
pub(crate) fn global_software_render_config() -> &'static Mutex<SoftwareRenderConfig> {
    pub(crate) static CONFIG: OnceLock<Mutex<SoftwareRenderConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(SoftwareRenderConfig::default()))
}

#[cfg(test)]
pub(crate) fn software_render_config_test_lock() -> &'static Mutex<()> {
    pub(crate) static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
/// Sets the process-wide default software render configuration.
///
/// The value is normalized (clamped) before being stored. The setting is
/// global and guarded by a mutex, so it is not scoped to a thread or a window;
/// it affects surfaces created afterwards. Newly created [`SoftwareSurface`]s
/// pick it up, existing ones keep their own value until reconfigured.
///
/// # Panics
///
/// Panics if the internal config lock is poisoned by a previous panic.
///
/// ```text
/// // Illustrates the API shape; the crate's tests use an internal guard lock.
/// set_default_software_render_config(SoftwareRenderConfig { aa_samples_per_axis: 4 });
/// ```
pub fn set_default_software_render_config(config: SoftwareRenderConfig) {
    *lock(global_software_render_config()) = config.normalized();
}
/// Returns a copy of the process-wide default software render configuration.
///
/// This never fails on an unset value: it falls back to
/// [`SoftwareRenderConfig::default`] the first time it is called.
///
/// # Panics
///
/// Panics if the internal config lock is poisoned by a previous panic.
pub fn default_software_render_config() -> SoftwareRenderConfig {
    *lock(global_software_render_config())
}
