//! Adaptive rendering quality management for dynamic performance optimization.

use std::time::Duration;

/// Rendering quality levels for adaptive performance control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QualityLevel {
    /// High quality: full effects including anti-aliasing, shadows, complex shaders.
    #[default]
    High,
    /// Medium quality: basic effects with simple shaders, no shadows.
    Medium,
    /// Low quality: minimal rendering with solid fills, no textures, may skip non-critical elements.
    Low,
}

impl PartialOrd for QualityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QualityLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (QualityLevel::High, QualityLevel::High) => std::cmp::Ordering::Equal,
            (QualityLevel::High, _) => std::cmp::Ordering::Greater,
            (_, QualityLevel::High) => std::cmp::Ordering::Less,
            (QualityLevel::Medium, QualityLevel::Medium) => std::cmp::Ordering::Equal,
            (QualityLevel::Medium, QualityLevel::Low) => std::cmp::Ordering::Greater,
            (QualityLevel::Low, QualityLevel::Medium) => std::cmp::Ordering::Less,
            (QualityLevel::Low, QualityLevel::Low) => std::cmp::Ordering::Equal,
        }
    }
}

impl QualityLevel {
    /// Returns the next lower quality level, if any.
    pub fn lower(&self) -> Option<Self> {
        match self {
            Self::High => Some(Self::Medium),
            Self::Medium => Some(Self::Low),
            Self::Low => None,
        }
    }

    /// Returns the next higher quality level, if any.
    pub fn higher(&self) -> Option<Self> {
        match self {
            Self::Low => Some(Self::Medium),
            Self::Medium => Some(Self::High),
            Self::High => None,
        }
    }
}

/// Configuration for quality adjustment behavior.
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

/// GPU capability detection based on adapter information.
#[derive(Debug, Clone, Copy)]
pub struct GpuCapability {
    /// Whether the GPU supports high-quality rendering.
    pub supports_high_quality: bool,
    /// Whether the GPU is integrated (vs discrete).
    pub is_integrated: bool,
    /// Estimated performance tier (1-5, higher is better).
    pub performance_tier: u8,
}

impl Default for GpuCapability {
    fn default() -> Self {
        Self {
            supports_high_quality: true,
            is_integrated: false,
            performance_tier: 3,
        }
    }
}

impl GpuCapability {
    /// Creates GPU capability from adapter information.
    // GPU capability detection is currently disabled due to missing feature flag.
    #[cfg(feature = "gpu-wgpu")]
    pub fn from_adapter_info(adapter_info: &wgpu::AdapterInfo) -> Self {
        let supports_high_quality = matches!(
            adapter_info.device_type,
            wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::IntegratedGpu
        );

        let is_integrated = matches!(adapter_info.device_type, wgpu::DeviceType::IntegratedGpu);

        let performance_tier = match adapter_info.device_type {
            wgpu::DeviceType::DiscreteGpu => 5,
            wgpu::DeviceType::IntegratedGpu => 3,
            wgpu::DeviceType::Other => 2,
            wgpu::DeviceType::VirtualGpu => 2,
            wgpu::DeviceType::Cpu => 1,
        };

        Self {
            supports_high_quality,
            is_integrated,
            performance_tier,
        }
    }

    /// Creates a default capability when GPU info is unavailable.
    pub fn default_capability() -> Self {
        Self::default()
    }

    /// Returns the recommended initial quality level.
    pub fn recommended_initial_quality(&self) -> QualityLevel {
        if self.supports_high_quality && self.performance_tier >= 4 {
            QualityLevel::High
        } else if self.supports_high_quality && self.performance_tier >= 2 {
            QualityLevel::Medium
        } else {
            QualityLevel::Low
        }
    }
}

/// Frame time monitor for tracking rendering performance with lightweight statistics.
#[derive(Debug, Clone)]
pub struct FrameTimeMonitor {
    frame_times: Vec<f32>,
    index: usize,
    count: usize,
    target_frame_time: f32,
}

impl FrameTimeMonitor {
    /// Creates a new frame time monitor with the specified target frame rate.
    pub fn new(target_frame_rate: f32) -> Self {
        Self {
            frame_times: vec![0.0; 60], // 60-frame sliding window
            index: 0,
            count: 0,
            target_frame_time: 1.0 / target_frame_rate,
        }
    }

    /// Records a frame duration in seconds.
    pub fn record_frame(&mut self, frame_duration: f32) {
        self.frame_times[self.index] = frame_duration;
        self.index = (self.index + 1) % self.frame_times.len();
        self.count = self.count.saturating_add(1).min(self.frame_times.len());
    }

    /// Returns the average frame time over the recorded frames.
    /// Uses simple moving average for lightweight statistics.
    pub fn average_frame_time(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }

        let sum: f32 = self.frame_times[..self.count].iter().sum();
        sum / self.count as f32
    }

    /// Returns the current frame rate based on average frame time.
    pub fn current_fps(&self) -> f32 {
        let avg = self.average_frame_time();
        if avg > 0.0 {
            1.0 / avg
        } else {
            0.0
        }
    }

    /// Checks if quality should be degraded based on frame time threshold.
    /// Uses hysteresis by requiring consecutive frames to exceed threshold.
    pub fn should_degrade(&self, threshold_duration: f32, consecutive_frames: usize) -> bool {
        if self.count < consecutive_frames {
            return false;
        }

        let start = if self.index >= consecutive_frames {
            self.index - consecutive_frames
        } else {
            self.frame_times.len() - (consecutive_frames - self.index)
        };

        for i in 0..consecutive_frames {
            let idx = (start + i) % self.frame_times.len();
            if self.frame_times[idx] <= threshold_duration {
                return false;
            }
        }

        true
    }

    /// Checks if quality should be upgraded based on frame time threshold.
    /// Uses hysteresis by requiring consecutive frames to be below threshold.
    pub fn should_upgrade(&self, threshold_duration: f32, consecutive_frames: usize) -> bool {
        if self.count < consecutive_frames {
            return false;
        }

        let start = if self.index >= consecutive_frames {
            self.index - consecutive_frames
        } else {
            self.frame_times.len() - (consecutive_frames - self.index)
        };

        for i in 0..consecutive_frames {
            let idx = (start + i) % self.frame_times.len();
            if self.frame_times[idx] > threshold_duration {
                return false;
            }
        }

        true
    }

    /// Resets the monitor state.
    pub fn reset(&mut self) {
        self.index = 0;
        self.count = 0;
        self.frame_times.fill(0.0);
    }

    /// Updates the target frame time.
    pub fn set_target_frame_rate(&mut self, frame_rate: f32) {
        self.target_frame_time = 1.0 / frame_rate;
    }

    /// Returns the target frame time.
    pub fn target_frame_time(&self) -> f32 {
        self.target_frame_time
    }
}

impl Default for FrameTimeMonitor {
    fn default() -> Self {
        Self::new(60.0)
    }
}

/// Quality manager for dynamic quality adjustment with hysteresis.
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
    pub fn with_config_and_capability(config: QualityConfig, gpu_capability: GpuCapability) -> Self {
        let config = config.normalized();
        let initial_quality = gpu_capability.recommended_initial_quality().clamp(
            config.min_quality,
            config.max_quality,
        );

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

    /// Updates the quality level based on frame time monitoring with hysteresis.
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
        self.frame_monitor.set_target_frame_rate(self.config.target_frame_rate);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_level_ordering() {
        assert!(QualityLevel::Low < QualityLevel::Medium);
        assert!(QualityLevel::Medium < QualityLevel::High);
    }

    #[test]
    fn quality_level_clamp() {
        // Test clamp behavior with valid min <= max
        assert_eq!(QualityLevel::High.clamp(QualityLevel::Low, QualityLevel::High), QualityLevel::High);
        assert_eq!(QualityLevel::Medium.clamp(QualityLevel::Low, QualityLevel::High), QualityLevel::Medium);
        assert_eq!(QualityLevel::Low.clamp(QualityLevel::Low, QualityLevel::High), QualityLevel::Low);
        
        // Test clamp with same min and max
        assert_eq!(QualityLevel::Medium.clamp(QualityLevel::Medium, QualityLevel::Medium), QualityLevel::Medium);
    }

    #[test]
    fn quality_level_navigation() {
        assert_eq!(QualityLevel::High.lower(), Some(QualityLevel::Medium));
        assert_eq!(QualityLevel::Medium.lower(), Some(QualityLevel::Low));
        assert_eq!(QualityLevel::Low.lower(), None);

        assert_eq!(QualityLevel::Low.higher(), Some(QualityLevel::Medium));
        assert_eq!(QualityLevel::Medium.higher(), Some(QualityLevel::High));
        assert_eq!(QualityLevel::High.higher(), None);
    }

    #[test]
    fn quality_config_normalization() {
        let config = QualityConfig {
            degrade_threshold: 0.5,
            upgrade_threshold: 1.5,
            degrade_frame_count: 0,
            upgrade_frame_count: 0,
            ..Default::default()
        };
        let normalized = config.normalized();

        assert!(normalized.degrade_threshold >= 1.0);
        assert!(normalized.upgrade_threshold <= 1.0 && normalized.upgrade_threshold >= 0.1);
        assert!(normalized.degrade_frame_count >= 1);
        assert!(normalized.upgrade_frame_count >= 1);
    }

    #[test]
    fn frame_time_monitor_average() {
        let mut monitor = FrameTimeMonitor::new(60.0);

        for _ in 0..10 {
            monitor.record_frame(0.016);
        }

        let avg = monitor.average_frame_time();
        assert!((avg - 0.016).abs() < 0.001);
    }

    #[test]
    fn frame_time_monitor_degrade() {
        let mut monitor = FrameTimeMonitor::new(60.0);

        for _ in 0..10 {
            monitor.record_frame(0.030);
        }

        assert!(monitor.should_degrade(0.020, 5));
    }

    #[test]
    fn frame_time_monitor_upgrade() {
        let mut monitor = FrameTimeMonitor::new(60.0);

        for _ in 0..10 {
            monitor.record_frame(0.010);
        }

        assert!(monitor.should_upgrade(0.020, 5));
    }

    #[test]
    fn quality_manager_degrades_on_slow_frames() {
        let config = QualityConfig {
            target_frame_rate: 60.0,
            degrade_threshold: 1.5,
            upgrade_threshold: 0.7,
            max_quality: QualityLevel::High,
            min_quality: QualityLevel::Low,
            degrade_frame_count: 3,
            upgrade_frame_count: 5,
        };

        // Use a high-tier GPU capability to ensure initial quality is High
        let gpu_capability = GpuCapability {
            supports_high_quality: true,
            is_integrated: false,
            performance_tier: 5,
        };
        let mut manager = QualityManager::with_config_and_capability(config, gpu_capability);

        assert_eq!(manager.quality_level(), QualityLevel::High);

        // Record exactly degrade_frame_count slow frames to trigger one degradation
        for _ in 0..3 {
            manager.finish_frame_secs(0.030);
        }

        // Quality should have degraded from High to Medium
        assert_eq!(manager.quality_level(), QualityLevel::Medium);
    }

    #[test]
    fn quality_manager_upgrades_on_fast_frames() {
        let config = QualityConfig {
            target_frame_rate: 60.0,
            degrade_threshold: 1.5,
            upgrade_threshold: 0.7,
            max_quality: QualityLevel::High,
            min_quality: QualityLevel::Low,
            degrade_frame_count: 5,
            upgrade_frame_count: 3,
        };

        // Use a low-tier GPU capability to ensure initial quality is Low
        let gpu_capability = GpuCapability {
            supports_high_quality: true,
            is_integrated: true,
            performance_tier: 1,
        };
        let mut manager = QualityManager::with_config_and_capability(config, gpu_capability);

        assert_eq!(manager.quality_level(), QualityLevel::Low);

        // Record exactly upgrade_frame_count fast frames to trigger one upgrade
        for _ in 0..3 {
            manager.finish_frame_secs(0.010);
        }

        // Quality should have upgraded from Low to Medium
        assert_eq!(manager.quality_level(), QualityLevel::Medium);
    }

    #[test]
    fn gpu_capability_recommended_quality() {
        let high_tier = GpuCapability {
            supports_high_quality: true,
            is_integrated: false,
            performance_tier: 5,
        };
        assert_eq!(high_tier.recommended_initial_quality(), QualityLevel::High);

        let medium_tier = GpuCapability {
            supports_high_quality: true,
            is_integrated: true,
            performance_tier: 3,
        };
        assert_eq!(medium_tier.recommended_initial_quality(), QualityLevel::Medium);

        let low_tier = GpuCapability {
            supports_high_quality: false,
            is_integrated: false,
            performance_tier: 1,
        };
        assert_eq!(low_tier.recommended_initial_quality(), QualityLevel::Low);
    }
}
