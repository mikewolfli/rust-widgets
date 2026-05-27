//! Quality manager for dynamic quality adjustment.
use super::config::QualityConfig;
use super::gpu::GpuCapability;
use super::level::QualityLevel;
use super::monitor::FrameTimeMonitor;
use std::time::Duration;
/// Quality manager for dynamic quality adjustment with hysteresis.
///
/// # Examples
///
/// ```rust
/// use rust_widgets::quality::{QualityManager, QualityLevel};
///
/// let mut manager = QualityManager::new();
/// assert_eq!(manager.quality_level(), QualityLevel::Medium);
///
/// // Record frame durations
/// manager.finish_frame_secs(0.016); // ~60 FPS
/// assert_eq!(manager.quality_level(), QualityLevel::Medium);
/// ```
#[derive(Debug, Clone)]
pub struct QualityManager {
    current_level: QualityLevel,
    config: QualityConfig,
    frame_monitor: FrameTimeMonitor,
    gpu_capability: GpuCapability,
}
impl QualityManager {
    /// Creates a new quality manager with default configuration.
    pub fn new() -> Self {
        Self::with_config_and_capability(QualityConfig::default(), GpuCapability::default())
    }
    /// Creates a new quality manager with the specified configuration.
    pub fn with_config(config: QualityConfig) -> Self {
        Self::with_config_and_capability(config, GpuCapability::default())
    }
    /// Creates a new quality manager with the specified configuration and GPU capability.
    pub fn with_config_and_capability(
        config: QualityConfig,
        gpu_capability: GpuCapability,
    ) -> Self {
        let config = config.normalized();
        let initial_quality = gpu_capability
            .recommended_initial_quality()
            .clamp(config.min_quality, config.max_quality);
        let frame_monitor = FrameTimeMonitor::new(config.target_frame_rate);
        Self {
            current_level: initial_quality,
            config,
            frame_monitor,
            gpu_capability,
        }
    }
    /// Records a frame duration and updates quality level if necessary.
    pub fn finish_frame(&mut self, frame_duration: Duration) {
        let frame_duration_secs = frame_duration.as_secs_f32();
        self.finish_frame_secs(frame_duration_secs);
    }
    /// Records a frame duration in seconds and updates quality level if necessary.
    pub fn finish_frame_secs(&mut self, frame_duration: f32) {
        self.frame_monitor.record_frame(frame_duration);
        self.update_quality_level();
    }
    fn update_quality_level(&mut self) {
        match self.current_level {
            QualityLevel::High => {
                if self.frame_monitor.should_degrade(
                    self.config.degrade_frame_duration(),
                    self.config.degrade_frame_count,
                ) {
                    if let Some(lower) = self.current_level.lower() {
                        if lower >= self.config.min_quality {
                            self.current_level = lower;
                        }
                    }
                }
            }
            QualityLevel::Medium => {
                if self.frame_monitor.should_degrade(
                    self.config.degrade_frame_duration(),
                    self.config.degrade_frame_count,
                ) {
                    if let Some(lower) = self.current_level.lower() {
                        if lower >= self.config.min_quality {
                            self.current_level = lower;
                        }
                    }
                } else if self.frame_monitor.should_upgrade(
                    self.config.upgrade_frame_duration(),
                    self.config.upgrade_frame_count,
                ) {
                    if let Some(higher) = self.current_level.higher() {
                        if higher <= self.config.max_quality {
                            self.current_level = higher;
                        }
                    }
                }
            }
            QualityLevel::Low => {
                if self.frame_monitor.should_upgrade(
                    self.config.upgrade_frame_duration(),
                    self.config.upgrade_frame_count,
                ) {
                    if let Some(higher) = self.current_level.higher() {
                        if higher <= self.config.max_quality {
                            self.current_level = higher;
                        }
                    }
                }
            }
        }
    }
    /// Returns the current quality level.
    pub fn quality_level(&self) -> QualityLevel {
        self.current_level
    }
    /// Sets the quality level manually.
    pub fn set_quality_level(&mut self, level: QualityLevel) {
        self.current_level = level.clamp(self.config.min_quality, self.config.max_quality);
    }
    /// Returns the quality configuration.
    pub fn config(&self) -> &QualityConfig {
        &self.config
    }
    /// Updates the quality configuration.
    pub fn set_config(&mut self, config: QualityConfig) {
        self.config = config.normalized();
        self.frame_monitor
            .set_target_frame_rate(self.config.target_frame_rate);
    }
    /// Returns the GPU capability.
    pub fn gpu_capability(&self) -> &GpuCapability {
        &self.gpu_capability
    }
    /// Returns the frame time monitor.
    pub fn frame_monitor(&self) -> &FrameTimeMonitor {
        &self.frame_monitor
    }
    /// Returns the current frame rate.
    pub fn current_fps(&self) -> f32 {
        self.frame_monitor.current_fps()
    }
    /// Returns the average frame time in seconds.
    pub fn average_frame_time(&self) -> f32 {
        self.frame_monitor.average_frame_time()
    }
    /// Resets the quality manager state.
    pub fn reset(&mut self) {
        self.frame_monitor.reset();
        self.current_level = self
            .gpu_capability
            .recommended_initial_quality()
            .clamp(self.config.min_quality, self.config.max_quality);
    }
}
impl Default for QualityManager {
    fn default() -> Self {
        Self::new()
    }
}
