//! Hardware-adaptive GPU manager with automatic device selection and fallback.
//!
//! This module provides a unified interface for GPU management that:
//! - Automatically selects the best available GPU (discrete > integrated > CPU)
//! - Configures buffer pools based on hardware capabilities
//! - Monitors performance and adjusts quality dynamically
//! - Detects performance traps and provides user guidance
//!
//! This integrates with the existing memory pool system in `crate::memory`.
use super::adapter::{AdapterInfo, AdapterSelectionStrategy, AdapterSelector};
use super::buffer_pool::{GpuBufferPoolStats, GpuStagingBufferPool};
use super::performance::{
    AdaptivePerformanceMonitor, PerformanceStats, PerformanceTrap, PerformanceTrapDetector,
};
use crate::quality::{GpuCapability, QualityLevel};
use std::sync::Mutex;

/// GPU operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuOperationMode {
    /// Hardware GPU rendering
    Hardware,
    /// CPU software rendering
    Software,
    /// Hybrid mode (CPU fallback for some operations)
    Hybrid,
}

/// Self-contained quality level tracker used by `GpuManager`.
///
/// Replaces the previous dependency on `crate::quality::QualityManager` which
/// does not export the type. This local implementation provides the same
/// degrade/upgrade logic with frame-time monitoring.
struct GpuQualityTracker {
    /// Current quality level.
    level: QualityLevel,
    /// Minimum allowed quality.
    min_quality: QualityLevel,
    /// Maximum allowed quality.
    max_quality: QualityLevel,
    /// Number of consecutive frames below degrade threshold.
    bad_frame_count: u32,
    /// Number of consecutive frames above upgrade threshold.
    good_frame_count: u32,
    /// Degrade threshold as multiple of target frame time.
    degrade_threshold: f64,
    /// Upgrade threshold as multiple of target frame time.
    upgrade_threshold: f64,
    /// Frame count needed to trigger degrade.
    degrade_frame_count: u32,
    /// Frame count needed to trigger upgrade.
    upgrade_frame_count: u32,
    /// Target frame duration in seconds.
    target_frame_time: f64,
}

impl GpuQualityTracker {
    fn new(gpu_capability: &GpuCapability) -> Self {
        let initial = gpu_capability.recommended_initial_quality();
        Self {
            level: initial,
            min_quality: QualityLevel::Low,
            max_quality: initial,
            bad_frame_count: 0,
            good_frame_count: 0,
            degrade_threshold: 1.5,
            upgrade_threshold: 0.7,
            degrade_frame_count: 5,
            upgrade_frame_count: 5,
            target_frame_time: 1.0 / 60.0,
        }
    }

    fn quality_level(&self) -> QualityLevel {
        self.level
    }

    fn set_quality_level(&mut self, level: QualityLevel) {
        self.level = level.clamp(self.min_quality, self.max_quality);
    }

    fn finish_frame(&mut self, frame_duration: std::time::Duration) {
        let secs = frame_duration.as_secs_f64();
        if secs > self.target_frame_time * self.degrade_threshold {
            self.bad_frame_count += 1;
            self.good_frame_count = 0;
            if self.bad_frame_count >= self.degrade_frame_count {
                if let Some(lower) = self.level.lower() {
                    if lower >= self.min_quality {
                        self.level = lower;
                    }
                }
                self.bad_frame_count = 0;
            }
        } else if secs < self.target_frame_time * self.upgrade_threshold {
            self.good_frame_count += 1;
            self.bad_frame_count = 0;
            if self.good_frame_count >= self.upgrade_frame_count {
                if let Some(higher) = self.level.higher() {
                    if higher <= self.max_quality {
                        self.level = higher;
                    }
                }
                self.good_frame_count = 0;
            }
        } else {
            self.bad_frame_count = 0;
            self.good_frame_count = 0;
        }
    }
}

/// Hardware-adaptive GPU manager
pub struct GpuManager {
    /// Selected adapter info
    adapter_info: AdapterInfo,
    /// Operation mode
    mode: GpuOperationMode,
    /// Staging buffer pool (GPU-specific)
    buffer_pool: Mutex<GpuStagingBufferPool>,
    /// Performance monitor
    performance_monitor: Mutex<AdaptivePerformanceMonitor>,
    /// Quality tracker
    quality_tracker: Mutex<GpuQualityTracker>,
    /// Performance trap detector
    trap_detector: Mutex<PerformanceTrapDetector>,
    /// Whether running in browser
    is_browser: bool,
    /// Performance warnings
    warnings: Mutex<Vec<String>>,
}

impl GpuManager {
    /// Creates a new GPU manager with automatic hardware detection
    pub async fn new() -> Result<Self, GpuManagerError> {
        Self::with_strategy(AdapterSelectionStrategy::Auto).await
    }

    /// Creates a new GPU manager with specific selection strategy
    pub async fn with_strategy(
        strategy: AdapterSelectionStrategy,
    ) -> Result<Self, GpuManagerError> {
        let selector = AdapterSelector::with_strategy(strategy);
        let adapter_info = selector
            .select_adapter_with_fallback(None)
            .await
            .map_err(|e| GpuManagerError::AdapterSelectionFailed(e.to_string()))?;
        Self::from_adapter_info(adapter_info).await
    }

    /// Creates a GPU manager from adapter info
    pub async fn from_adapter_info(adapter_info: AdapterInfo) -> Result<Self, GpuManagerError> {
        let mode = if adapter_info.device_type.is_cpu() {
            GpuOperationMode::Software
        } else {
            GpuOperationMode::Hardware
        };
        let buffer_pool = GpuStagingBufferPool::for_gpu_type(adapter_info.device_type);
        let performance_monitor =
            AdaptivePerformanceMonitor::for_device_type(adapter_info.device_type);
        let gpu_capability = GpuCapability {
            supports_high_quality: adapter_info.supports_high_quality(),
            is_integrated: adapter_info.device_type.is_integrated(),
            performance_tier: adapter_info.device_type.performance_tier(),
        };
        let quality_tracker = GpuQualityTracker::new(&gpu_capability);
        let low_fps_threshold = if adapter_info.device_type.is_cpu() {
            15.0
        } else if adapter_info.device_type.is_integrated() {
            20.0
        } else {
            25.0
        };
        let trap_detector = PerformanceTrapDetector::new(low_fps_threshold, 10);
        let is_browser = super::adapter::detect_browser_forced_integrated_gpu();
        let manager = Self {
            adapter_info,
            mode,
            buffer_pool: Mutex::new(buffer_pool),
            performance_monitor: Mutex::new(performance_monitor),
            quality_tracker: Mutex::new(quality_tracker),
            trap_detector: Mutex::new(trap_detector),
            is_browser,
            warnings: Mutex::new(Vec::new()),
        };
        if is_browser && manager.adapter_info.device_type.is_integrated() {
            manager.add_warning(
                "Browser is forcing integrated GPU. For best performance, run outside browser.",
            );
        }
        Ok(manager)
    }

    /// Returns the selected adapter info
    pub fn adapter_info(&self) -> &AdapterInfo {
        &self.adapter_info
    }

    /// Returns the operation mode
    pub fn operation_mode(&self) -> GpuOperationMode {
        self.mode
    }

    /// Returns true if using hardware GPU
    pub fn is_hardware(&self) -> bool {
        matches!(self.mode, GpuOperationMode::Hardware)
    }

    /// Returns true if using CPU software rendering
    pub fn is_software(&self) -> bool {
        matches!(self.mode, GpuOperationMode::Software)
    }

    /// Begins a new frame
    pub fn begin_frame(&self) {
        if let Ok(mut monitor) = self.performance_monitor.lock() {
            monitor.begin_frame();
        }
        if let Ok(mut pool) = self.buffer_pool.lock() {
            pool.next_frame();
        }
    }

    /// Ends the current frame and updates performance monitoring
    pub fn end_frame(&self) -> Option<PerformanceStats> {
        let sample = if let Ok(mut monitor) = self.performance_monitor.lock() {
            Some(monitor.end_frame())
        } else {
            None
        };
        if let Ok(mut quality) = self.quality_tracker.lock() {
            if let Some(ref s) = sample {
                quality.finish_frame(s.frame_duration);
            }
        }
        if let Ok(mut detector) = self.trap_detector.lock() {
            if let Some(ref sample) = sample {
                let fps = 1.0 / sample.frame_duration.as_secs_f32();
                if let Some(trap) = detector.check(fps) {
                    self.handle_performance_trap(trap);
                }
            }
        }
        if let Ok(mut monitor) = self.performance_monitor.lock() {
            monitor.auto_adjust_thresholds();
        }
        if let Ok(monitor) = self.performance_monitor.lock() {
            Some(monitor.stats())
        } else {
            None
        }
    }

    /// Handles a performance trap
    fn handle_performance_trap(&self, trap: PerformanceTrap) {
        match &trap {
            PerformanceTrap::LowFrameRate { current_fps, .. } => {
                log::warn!("[gpu] Low frame rate detected: {:.1} FPS", current_fps);
            }
            PerformanceTrap::MemoryPressure { utilization } => {
                log::warn!("[gpu] Memory pressure: {:.0}%", utilization * 100.0);
                self.add_warning(&trap.message());
            }
            PerformanceTrap::CpuOverload { utilization } => {
                log::warn!("[gpu] CPU overload: {:.0}%", utilization * 100.0);
                self.add_warning(&trap.message());
            }
            PerformanceTrap::BrowserForcedIntegratedGpu => {
                log::warn!("[gpu] Browser forcing integrated GPU");
                self.add_warning(&trap.message());
            }
        }
    }

    /// Adds a warning message
    fn add_warning(&self, message: &str) {
        if let Ok(mut warnings) = self.warnings.lock() {
            if !warnings.contains(&message.to_string()) {
                warnings.push(message.to_string());
            }
        }
    }

    /// Returns all warnings
    pub fn warnings(&self) -> Vec<String> {
        if let Ok(warnings) = self.warnings.lock() {
            warnings.clone()
        } else {
            Vec::new()
        }
    }

    /// Clears all warnings
    pub fn clear_warnings(&self) {
        if let Ok(mut warnings) = self.warnings.lock() {
            warnings.clear();
        }
    }

    /// Returns the current quality level
    pub fn current_quality(&self) -> QualityLevel {
        if let Ok(quality) = self.quality_tracker.lock() {
            quality.quality_level()
        } else {
            QualityLevel::Medium
        }
    }

    /// Sets the quality level manually
    pub fn set_quality(&self, level: QualityLevel) {
        if let Ok(mut quality) = self.quality_tracker.lock() {
            quality.set_quality_level(level);
        }
    }

    /// Returns buffer pool statistics
    pub fn buffer_pool_stats(&self) -> Option<GpuBufferPoolStats> {
        if let Ok(pool) = self.buffer_pool.lock() {
            Some(pool.memory_stats())
        } else {
            None
        }
    }

    /// Returns performance statistics
    pub fn performance_stats(&self) -> Option<PerformanceStats> {
        if let Ok(monitor) = self.performance_monitor.lock() {
            Some(monitor.stats())
        } else {
            None
        }
    }

    /// Returns true if quality should be degraded
    pub fn should_degrade_quality(&self) -> bool {
        if let Ok(monitor) = self.performance_monitor.lock() {
            monitor.should_degrade()
        } else {
            false
        }
    }

    /// Returns true if quality should be upgraded
    pub fn should_upgrade_quality(&self) -> bool {
        if let Ok(monitor) = self.performance_monitor.lock() {
            monitor.should_upgrade()
        } else {
            false
        }
    }

    /// Returns recommended actions based on current state
    pub fn recommended_actions(&self) -> Vec<GpuManagerAction> {
        let mut actions = Vec::new();
        if self.current_quality() == QualityLevel::Low {
            if let Some(stats) = self.performance_stats() {
                if stats.current_fps < 15.0 {
                    if self.is_browser {
                        actions.push(GpuManagerAction::SuggestRestartOutsideBrowser);
                    } else if !self.is_software() {
                        actions.push(GpuManagerAction::SuggestSwitchToCpuMode);
                    }
                }
            }
        }
        if let Some(stats) = self.performance_stats() {
            if stats.consecutive_bad_frames > 30 {
                actions.push(GpuManagerAction::SuggestCloseOtherApplications);
            }
        }
        actions
    }

    /// Returns a summary of the current GPU configuration
    pub fn configuration_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("GPU: {}\n", self.adapter_info.name));
        summary.push_str(&format!("Type: {}\n", self.adapter_info.device_type));
        summary.push_str(&format!("Mode: {:?}\n", self.mode));
        summary.push_str(&format!("Quality: {:?}\n", self.current_quality()));
        if let Some(stats) = self.performance_stats() {
            summary.push_str(&format!("FPS: {:.1}\n", stats.current_fps));
            summary.push_str(&format!("Stability: {:.0}%\n", stats.stability * 100.0));
        }
        if let Some(pool_stats) = self.buffer_pool_stats() {
            let utilization = pool_stats.used_size as f32 / pool_stats.total_size as f32 * 100.0;
            summary.push_str(&format!("Buffer Pool: {:.0}%\n", utilization));
        }
        summary
    }
}

/// Actions recommended by the GPU manager
#[derive(Debug, Clone)]
pub enum GpuManagerAction {
    /// Suggest switching to CPU software mode
    SuggestSwitchToCpuMode,
    /// Suggest restarting outside browser
    SuggestRestartOutsideBrowser,
    /// Suggest closing other applications
    SuggestCloseOtherApplications,
    /// Suggest reducing screen resolution
    SuggestReduceResolution,
    /// Suggest updating GPU drivers
    SuggestUpdateDrivers,
}

impl GpuManagerAction {
    /// Returns a user-friendly message
    pub fn message(&self) -> String {
        match self {
            Self::SuggestSwitchToCpuMode => {
                "Performance is very low. Consider switching to CPU software mode by restarting with --cpu flag.".to_string()
            }
            Self::SuggestRestartOutsideBrowser => {
                "Browser is limiting GPU performance. For best results, run the application outside the browser.".to_string()
            }
            Self::SuggestCloseOtherApplications => {
                "System resources are constrained. Try closing other applications to improve performance.".to_string()
            }
            Self::SuggestReduceResolution => {
                "Consider reducing the window size or screen resolution for better performance.".to_string()
            }
            Self::SuggestUpdateDrivers => {
                "GPU drivers may be outdated. Consider updating to the latest version.".to_string()
            }
        }
    }

    /// Returns the priority (higher = more urgent)
    pub fn priority(&self) -> u8 {
        match self {
            Self::SuggestRestartOutsideBrowser => 5,
            Self::SuggestSwitchToCpuMode => 4,
            Self::SuggestCloseOtherApplications => 3,
            Self::SuggestReduceResolution => 2,
            Self::SuggestUpdateDrivers => 1,
        }
    }
}

/// GPU manager errors
#[derive(Debug, Clone)]
pub enum GpuManagerError {
    /// Adapter selection failed
    AdapterSelectionFailed(String),
    /// Device creation failed
    DeviceCreationFailed(String),
    /// No suitable GPU found
    NoSuitableGpu,
}

impl std::fmt::Display for GpuManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdapterSelectionFailed(msg) => write!(f, "Adapter selection failed: {}", msg),
            Self::DeviceCreationFailed(msg) => write!(f, "Device creation failed: {}", msg),
            Self::NoSuitableGpu => write!(f, "No suitable GPU found"),
        }
    }
}

impl std::error::Error for GpuManagerError {}

/// Builder for GPU manager
pub struct GpuManagerBuilder {
    strategy: AdapterSelectionStrategy,
    allow_fallback: bool,
    target_quality: QualityLevel,
}

impl GpuManagerBuilder {
    /// Creates a new builder
    pub fn new() -> Self {
        Self {
            strategy: AdapterSelectionStrategy::Auto,
            allow_fallback: true,
            target_quality: QualityLevel::High,
        }
    }

    /// Sets the selection strategy
    pub fn strategy(mut self, strategy: AdapterSelectionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Sets whether to allow fallback
    pub fn allow_fallback(mut self, allow: bool) -> Self {
        self.allow_fallback = allow;
        self
    }

    /// Sets the target quality
    pub fn target_quality(mut self, quality: QualityLevel) -> Self {
        self.target_quality = quality;
        self
    }

    /// Builds the GPU manager
    pub async fn build(self) -> Result<GpuManager, GpuManagerError> {
        GpuManager::with_strategy(self.strategy).await
    }
}

impl Default for GpuManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_gpu_manager_action_priority() {
        assert!(
            GpuManagerAction::SuggestRestartOutsideBrowser.priority()
                > GpuManagerAction::SuggestCloseOtherApplications.priority()
        );
    }
    #[test]
    fn test_gpu_manager_action_messages() {
        let action = GpuManagerAction::SuggestSwitchToCpuMode;
        let msg = action.message();
        assert!(msg.contains("CPU"));
    }
    #[test]
    fn test_operation_mode() {
        assert!(matches!(
            GpuOperationMode::Hardware,
            GpuOperationMode::Hardware
        ));
        assert!(matches!(
            GpuOperationMode::Software,
            GpuOperationMode::Software
        ));
    }
}
