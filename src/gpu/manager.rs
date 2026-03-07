//! Hardware-adaptive GPU manager with automatic device selection and fallback.
//!
//! This module provides a unified interface for GPU management that:
//! - Automatically selects the best available GPU (discrete > integrated > CPU)
//! - Configures buffer pools based on hardware capabilities
//! - Monitors performance and adjusts quality dynamically
//! - Detects performance traps and provides user guidance
//!
//! This integrates with the existing memory pool system in `crate::memory`.

use std::sync::Mutex;

use super::adapter::{AdapterInfo, AdapterSelectionStrategy, AdapterSelector};
use super::buffer_pool::{GpuStagingBufferPool, GpuBufferPoolStats};
use super::performance::{AdaptivePerformanceMonitor, PerformanceStats, PerformanceTrap, PerformanceTrapDetector};
use crate::quality::{QualityLevel, QualityManager};

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
    /// Quality manager
    quality_manager: Mutex<QualityManager>,
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
    pub async fn with_strategy(strategy: AdapterSelectionStrategy) -> Result<Self, GpuManagerError> {
        let selector = AdapterSelector::with_strategy(strategy);
        
        // Try to select adapter
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

        // Create buffer pool based on device type
        let buffer_pool = GpuStagingBufferPool::for_gpu_type(adapter_info.device_type);

        // Create performance monitor
        let performance_monitor = AdaptivePerformanceMonitor::for_device_type(adapter_info.device_type);

        // Create quality manager with hardware-aware initial quality
        let gpu_capability = crate::quality::GpuCapability {
            supports_high_quality: adapter_info.supports_high_quality(),
            is_integrated: adapter_info.device_type.is_integrated(),
            performance_tier: adapter_info.device_type.performance_tier(),
        };
        let quality_manager = QualityManager::with_config_and_capability(
            crate::quality::QualityConfig::default(),
            gpu_capability,
        );

        // Create performance trap detector
        let low_fps_threshold = if adapter_info.device_type.is_cpu() {
            15.0 // Lower threshold for CPU
        } else if adapter_info.device_type.is_integrated() {
            20.0
        } else {
            25.0
        };
        let trap_detector = PerformanceTrapDetector::new(low_fps_threshold, 10);

        // Detect browser environment
        let is_browser = super::adapter::detect_browser_forced_integrated_gpu();

        let manager = Self {
            adapter_info,
            mode,
            buffer_pool: Mutex::new(buffer_pool),
            performance_monitor: Mutex::new(performance_monitor),
            quality_manager: Mutex::new(quality_manager),
            trap_detector: Mutex::new(trap_detector),
            is_browser,
            warnings: Mutex::new(Vec::new()),
        };

        // Check for browser forced iGPU
        if is_browser && manager.adapter_info.device_type.is_integrated() {
            manager.add_warning(
                "Browser is forcing integrated GPU. For best performance, run outside browser."
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

        // Advance buffer pool
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

        // Update quality manager
        if let Ok(mut quality) = self.quality_manager.lock() {
            if let Some(ref s) = sample {
                quality.finish_frame(s.frame_duration);
            }
        }

        // Check for performance traps
        if let Ok(mut detector) = self.trap_detector.lock() {
            if let Some(sample) = sample {
                let fps = 1.0 / sample.frame_duration.as_secs_f32();
                if let Some(trap) = detector.check(fps) {
                    self.handle_performance_trap(trap);
                }
            }
        }

        // Auto-adjust thresholds periodically
        if let Ok(mut monitor) = self.performance_monitor.lock() {
            monitor.auto_adjust_thresholds();
        }

        // Return stats
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
                // Already handled by quality manager
                eprintln!("[gpu] Low frame rate detected: {:.1} FPS", current_fps);
            }
            PerformanceTrap::MemoryPressure { utilization } => {
                eprintln!("[gpu] Memory pressure: {:.0}%", utilization * 100.0);
                self.add_warning(&trap.message());
            }
            PerformanceTrap::CpuOverload { utilization } => {
                eprintln!("[gpu] CPU overload: {:.0}%", utilization * 100.0);
                self.add_warning(&trap.message());
            }
            PerformanceTrap::BrowserForcedIntegratedGpu => {
                eprintln!("[gpu] Browser forcing integrated GPU");
                self.add_warning(&trap.message());
            }
        }
    }

    /// Adds a warning message
    fn add_warning(&self, message: &str) {
        if let Ok(mut warnings) = self.warnings.lock() {
            // Avoid duplicate warnings
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
        if let Ok(quality) = self.quality_manager.lock() {
            quality.quality_level()
        } else {
            QualityLevel::Medium
        }
    }

    /// Sets the quality level manually
    pub fn set_quality(&self, level: QualityLevel) {
        if let Ok(mut quality) = self.quality_manager.lock() {
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

    /// Returns true if should degrade quality
    pub fn should_degrade_quality(&self) -> bool {
        if let Ok(monitor) = self.performance_monitor.lock() {
            monitor.should_degrade()
        } else {
            false
        }
    }

    /// Returns true if should upgrade quality
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

        // Check if quality is at minimum and performance is still bad
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

        // Check for sustained low performance
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
        assert!(matches!(GpuOperationMode::Hardware, GpuOperationMode::Hardware));
        assert!(matches!(GpuOperationMode::Software, GpuOperationMode::Software));
    }
}
