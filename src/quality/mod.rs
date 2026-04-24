//! Adaptive rendering quality management for dynamic performance optimization.
//!
//! # Example
//!
//! ```rust
//! use rust_widgets::quality::{QualityManager, QualityLevel, QualityConfig};
//!
//! // Create a quality manager with default configuration
//! let manager = QualityManager::new();
//! // Default gpu capability maps to Medium quality
//! assert_eq!(manager.quality_level(), QualityLevel::Medium);
//!
//! // Check quality level ordering
//! assert!(QualityLevel::Low < QualityLevel::Medium);
//! assert!(QualityLevel::Medium < QualityLevel::High);
//! ```
mod config;
mod gpu;
mod level;
mod manager;
mod monitor;
pub use config::QualityConfig;
pub use gpu::GpuCapability;
pub use level::QualityLevel;
pub use manager::QualityManager;
pub use monitor::FrameTimeMonitor;
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
        assert_eq!(
            QualityLevel::High.clamp(QualityLevel::Low, QualityLevel::High),
            QualityLevel::High
        );
        assert_eq!(
            QualityLevel::Medium.clamp(QualityLevel::Low, QualityLevel::High),
            QualityLevel::Medium
        );
        assert_eq!(
            QualityLevel::Low.clamp(QualityLevel::Low, QualityLevel::High),
            QualityLevel::Low
        );
        assert_eq!(
            QualityLevel::Medium.clamp(QualityLevel::Medium, QualityLevel::Medium),
            QualityLevel::Medium
        );
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
        let gpu_capability = GpuCapability {
            supports_high_quality: true,
            is_integrated: false,
            performance_tier: 5,
        };
        let mut manager = QualityManager::with_config_and_capability(config, gpu_capability);
        assert_eq!(manager.quality_level(), QualityLevel::High);
        for _ in 0..3 {
            manager.finish_frame_secs(0.030);
        }
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
        let gpu_capability = GpuCapability {
            supports_high_quality: true,
            is_integrated: true,
            performance_tier: 1,
        };
        let mut manager = QualityManager::with_config_and_capability(config, gpu_capability);
        assert_eq!(manager.quality_level(), QualityLevel::Low);
        for _ in 0..3 {
            manager.finish_frame_secs(0.010);
        }
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
        assert_eq!(
            medium_tier.recommended_initial_quality(),
            QualityLevel::Medium
        );
        let low_tier = GpuCapability {
            supports_high_quality: false,
            is_integrated: false,
            performance_tier: 1,
        };
        assert_eq!(low_tier.recommended_initial_quality(), QualityLevel::Low);
    }
}
