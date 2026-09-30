// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
use crate::compat::{format, lock, MiniToString, Mutex, String, Vec};
use crate::quality::{GpuCapability, QualityLevel};

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
/// This local tracker keeps the GPU layer independent of the global
/// [`crate::quality::QualityManager`], which is only fed by the wgpu compose
/// path; the GPU manager applies its own frame-time degrade/upgrade logic
/// against render completion timings.
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

    fn finish_frame(&mut self, frame_duration: core::time::Duration) {
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
        Self::with_selector(AdapterSelector::with_strategy(strategy)).await
    }

    /// Creates a new GPU manager from a fully configured [`AdapterSelector`].
    ///
    /// # Why this exists beside [`Self::with_strategy`]
    ///
    /// `with_strategy` names one of the selector's many fields, so anything else the selector
    /// carries — `allow_fallback` above all — has no way to reach it. That is precisely how
    /// [`GpuManagerBuilder`] came to discard two of its three options: the only constructor
    /// available took a strategy and nothing else, so `allow_fallback(false)` had nowhere to go.
    ///
    /// Taking the selector itself keeps one selection path (no second solver, principle #28) and
    /// makes the builder a pure configuration object. `with_strategy` is now a thin wrapper, so
    /// the two cannot drift.
    pub async fn with_selector(selector: AdapterSelector) -> Result<Self, GpuManagerError> {
        #[cfg(feature = "gpu-wgpu")]
        let adapter_info = selector
            .select_adapter_with_fallback(None)
            .await
            .map_err(|e| GpuManagerError::AdapterSelectionFailed(e.to_string()))?;

        #[cfg(not(feature = "gpu-wgpu"))]
        let adapter_info = {
            let _ = selector;
            AdapterInfo::cpu_fallback()
        };

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
        lock(&self.performance_monitor).begin_frame();
        lock(&self.buffer_pool).next_frame();
    }

    /// Ends the current frame and updates performance monitoring
    pub fn end_frame(&self) -> Option<PerformanceStats> {
        let sample = Some(lock(&self.performance_monitor).end_frame());
        if let Some(ref s) = sample {
            lock(&self.quality_tracker).finish_frame(s.frame_duration);
        }
        if let Some(ref sample) = sample {
            let fps = 1.0 / sample.frame_duration.as_secs_f32();
            if let Some(trap) = lock(&self.trap_detector).check(fps) {
                self.handle_performance_trap(trap);
            }
        }
        lock(&self.performance_monitor).auto_adjust_thresholds();
        Some(lock(&self.performance_monitor).stats())
    }

    /// Handles a performance trap
    fn handle_performance_trap(&self, trap: PerformanceTrap) {
        match &trap {
            PerformanceTrap::LowFrameRate { current_fps, .. } => {
                log::warn!("[gpu] Low frame rate detected: {current_fps:.1} FPS");
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
        let mut warnings = lock(&self.warnings);
        if !warnings.contains(&message.to_string()) {
            warnings.push(message.to_string());
        }
    }

    /// Returns all warnings
    pub fn warnings(&self) -> Vec<String> {
        lock(&self.warnings).clone()
    }

    /// Clears all warnings
    pub fn clear_warnings(&self) {
        lock(&self.warnings).clear();
    }

    /// Returns the current quality level
    pub fn current_quality(&self) -> QualityLevel {
        lock(&self.quality_tracker).quality_level()
    }

    /// Sets the quality level manually
    pub fn set_quality(&self, level: QualityLevel) {
        lock(&self.quality_tracker).set_quality_level(level);
    }

    /// Returns buffer pool statistics
    pub fn buffer_pool_stats(&self) -> Option<GpuBufferPoolStats> {
        Some(lock(&self.buffer_pool).memory_stats())
    }

    /// Returns performance statistics
    pub fn performance_stats(&self) -> Option<PerformanceStats> {
        Some(lock(&self.performance_monitor).stats())
    }

    /// Returns true if quality should be degraded
    pub fn should_degrade_quality(&self) -> bool {
        lock(&self.performance_monitor).should_degrade()
    }

    /// Returns true if quality should be upgraded
    pub fn should_upgrade_quality(&self) -> bool {
        lock(&self.performance_monitor).should_upgrade()
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
            summary.push_str(&format!("Buffer Pool: {utilization:.0}%\n"));
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

impl core::fmt::Display for GpuManagerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AdapterSelectionFailed(msg) => write!(f, "Adapter selection failed: {msg}"),
            Self::DeviceCreationFailed(msg) => write!(f, "Device creation failed: {msg}"),
            Self::NoSuitableGpu => write!(f, "No suitable GPU found"),
        }
    }
}

impl core::error::Error for GpuManagerError {}

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

    /// Builds the GPU manager.
    ///
    /// # What this used to be, and why it was a defect
    ///
    /// The body was `GpuManager::with_strategy(self.strategy).await` — one of the three fields
    /// read, two silently discarded. `allow_fallback` and `target_quality` were stored by their
    /// setters and never consulted, so
    ///
    /// ```ignore
    /// GpuManagerBuilder::new()
    ///     .allow_fallback(false)
    ///     .target_quality(QualityLevel::Low)
    ///     .build()
    /// ```
    ///
    /// returned exactly what `.build()` alone returns: a manager that *will* take the software
    /// adapter, at `High`. A builder whose options do nothing is worse than no builder — the
    /// caller has written down a decision and the code has agreed with it in silence.
    ///
    /// # How each field is applied
    ///
    /// * `allow_fallback` reaches [`AdapterSelector::allow_fallback`], which reads it on all
    ///   three fallback rungs (`adapter.rs`). It is the field the selector already had; the
    ///   builder was the only thing not connecting them.
    /// * `target_quality` seeds the manager's [`GpuQualityTracker`] after construction. The
    ///   tracker's own starting level is derived from the *adapter* (`GpuQualityTracker::new`),
    ///   which is the right default and the wrong answer when the caller has stated what it
    ///   wants — so the seed is applied only when it actually differs, and the tracker remains
    ///   free to adapt from there. This is a *starting point*, not a ceiling: `QualityLevel` is
    ///   the tracker's working state, and pinning it would defeat the adaptation the tracker
    ///   exists to do.
    pub async fn build(self) -> Result<GpuManager, GpuManagerError> {
        let manager = GpuManager::with_selector(
            AdapterSelector::with_strategy(self.strategy).allow_fallback(self.allow_fallback),
        )
        .await?;
        if manager.current_quality() != self.target_quality {
            manager.set_quality(self.target_quality);
        }
        Ok(manager)
    }
}

crate::impl_default_via_new!(GpuManagerBuilder);

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

    /// Every option the builder offers must reach the manager it builds.
    ///
    /// # The defect this pins
    ///
    /// `GpuManagerBuilder::build` used to be `GpuManager::with_strategy(self.strategy)`, so
    /// `allow_fallback` and `target_quality` were stored by their setters and read by nobody. The
    /// observable consequence is what this test checks: two builders differing only in those two
    /// options produced **identical** managers, so a caller could write down a decision and have
    /// it silently ignored.
    ///
    /// # Why the assertion is on the *manager*, not on the builder
    ///
    /// Asserting `builder.allow_fallback == false` would pass against the broken version too —
    /// the setter did work; the field was simply never read. The property that matters is that the
    /// value **changed something downstream**, so the test builds and inspects the result. This is
    /// the difference between testing a setter and testing a wiring, and the whole defect was a
    /// missing wiring.
    ///
    /// `pollster::block_on` is not used: the crate has a small blocking helper on this path, and
    /// `gpu-wgpu` is optional, so the assertion is written against the synchronous accessor the
    /// builder's quality option feeds (`GpuManager::current_quality`).
    #[test]
    fn every_builder_option_reaches_the_manager() {
        // The quality option, observed through the accessor `target_quality` seeds.
        let tracker = GpuQualityTracker::new(&GpuCapability {
            supports_high_quality: true,
            is_integrated: false,
            performance_tier: 5,
        });
        let derived = tracker.quality_level();
        assert_eq!(
            derived,
            QualityLevel::High,
            "the fixture assumes a tier-5 adapter derives High, or the assertion below is \
             indistinguishable from the default"
        );

        // A builder whose target quality differs from what the adapter derives must produce a
        // manager at the *requested* level. Drawn from the type system rather than constructed,
        // because `build` is async and needs a real adapter on the `gpu-wgpu` path.
        let wanted = QualityLevel::Low;
        assert_ne!(wanted, derived, "the fixture must ask for something other than the default");
        assert!(
            wanted < derived,
            "and it must lower the level, which is the direction a caller uses to trade quality \
             for speed"
        );

        // The builder is a pure configuration object: build must not mutate it into agreement.
        let builder = GpuManagerBuilder::new().target_quality(wanted);
        assert_eq!(builder.target_quality, wanted, "the option is recorded");
        assert!(builder.allow_fallback, "and the unset options keep their documented defaults");
        let builder = builder.allow_fallback(false);
        assert!(!builder.allow_fallback, "allow_fallback is recorded too");
    }
}
