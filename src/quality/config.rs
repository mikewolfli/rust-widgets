//! Quality adjustment configuration.
use super::level::QualityLevel;
/// Configuration for quality adjustment behavior.
///
/// # Examples
///
/// ```rust
/// use rust_widgets::quality::QualityConfig;
///
/// let config = QualityConfig::default();
/// assert!(config.target_frame_rate > 0.0);
/// assert!(config.degrade_threshold >= 1.0);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct QualityConfig {
    /// Target frame rate in frames per second.
    pub target_frame_rate: f32,
    /// Threshold multiplier for degrading quality (e.g., 1.5 = degrade when frame time exceeds 1.5x target).
    pub degrade_threshold: f32,
    /// Threshold multiplier for upgrading quality (e.g., 0.7 = upgrade when frame time is below 0.7x target).
    pub upgrade_threshold: f32,
    /// Maximum allowed quality level.
    pub max_quality: QualityLevel,
    /// Minimum allowed quality level.
    pub min_quality: QualityLevel,
    /// Number of consecutive frames that must exceed threshold before degrading.
    pub degrade_frame_count: usize,
    /// Number of consecutive frames that must be below threshold before upgrading.
    pub upgrade_frame_count: usize,
}
impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            target_frame_rate: 60.0,
            degrade_threshold: 1.5,
            upgrade_threshold: 0.7,
            max_quality: QualityLevel::High,
            min_quality: QualityLevel::Low,
            degrade_frame_count: 5,
            upgrade_frame_count: 10,
        }
    }
}
impl QualityConfig {
    /// Returns the target frame duration in seconds.
    pub fn target_frame_duration(&self) -> f32 {
        1.0 / self.target_frame_rate
    }
    /// Returns the frame duration threshold for degrading quality.
    pub fn degrade_frame_duration(&self) -> f32 {
        self.target_frame_duration() * self.degrade_threshold
    }
    /// Returns the frame duration threshold for upgrading quality.
    pub fn upgrade_frame_duration(&self) -> f32 {
        self.target_frame_duration() * self.upgrade_threshold
    }
    /// Creates a new config with clamped threshold values.
    pub fn normalized(self) -> Self {
        Self {
            degrade_threshold: self.degrade_threshold.max(1.0),
            upgrade_threshold: self.upgrade_threshold.clamp(0.1, 1.0),
            degrade_frame_count: self.degrade_frame_count.max(1),
            upgrade_frame_count: self.upgrade_frame_count.max(1),
            ..self
        }
    }
}
