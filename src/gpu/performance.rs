//! Hardware-adaptive performance monitoring and dynamic quality adjustment.
//!
//! This module provides performance monitoring that adapts to different hardware types:
//! - Discrete GPU: GPU timestamp-based monitoring
//! - Integrated GPU: Frame time + memory bandwidth monitoring
//! - CPU Software: CPU frame time + thread utilization monitoring
use super::adapter::GpuDeviceType;
use alloc::collections::VecDeque;
use core::time::Duration;
use std::time::Instant;
/// Performance monitoring strategy based on hardware type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceMonitorStrategy {
    /// GPU timestamp query-based monitoring
    GpuTimestamp,
    /// Frame time-based monitoring
    FrameTime,
    /// CPU utilization-based monitoring
    CpuUtilization,
}
impl PerformanceMonitorStrategy {
    /// Returns the appropriate strategy for a GPU type
    pub fn for_device_type(device_type: GpuDeviceType) -> Self {
        match device_type {
            GpuDeviceType::DiscreteGpu => Self::GpuTimestamp,
            GpuDeviceType::IntegratedGpu => Self::FrameTime,
            _ => Self::CpuUtilization,
        }
    }
}
/// Performance thresholds that adapt to hardware capabilities
#[derive(Debug, Clone)]
pub struct AdaptivePerformanceThresholds {
    /// Target frame rate
    pub target_fps: f32,
    /// Degrade threshold multiplier (e.g., 1.5 = degrade when 1.5x target frame time)
    pub degrade_threshold: f32,
    /// Upgrade threshold multiplier
    pub upgrade_threshold: f32,
    /// Consecutive frames required for degrade
    pub degrade_frame_count: usize,
    /// Consecutive frames required for upgrade
    pub upgrade_frame_count: usize,
    /// Memory pressure threshold (0.0 - 1.0)
    pub memory_pressure_threshold: f32,
    /// CPU utilization threshold for CPU rendering
    pub cpu_utilization_threshold: f32,
}
impl AdaptivePerformanceThresholds {
    /// Creates thresholds for discrete GPU
    pub fn discrete() -> Self {
        Self {
            target_fps: 60.0,
            degrade_threshold: 1.5,  // Degrade when frame time > 1.5x target
            upgrade_threshold: 0.7,  // Upgrade when frame time < 0.7x target
            degrade_frame_count: 5,  // Need 5 bad frames
            upgrade_frame_count: 10, // Need 10 good frames
            memory_pressure_threshold: 0.9,
            cpu_utilization_threshold: 0.8,
        }
    }
    /// Creates thresholds for integrated GPU
    pub fn integrated() -> Self {
        Self {
            target_fps: 60.0,
            degrade_threshold: 1.3, // More aggressive degrade for iGPU
            upgrade_threshold: 0.75,
            degrade_frame_count: 3,          // Faster degrade response
            upgrade_frame_count: 15,         // Slower upgrade (conservative)
            memory_pressure_threshold: 0.75, // iGPU shares memory with CPU
            cpu_utilization_threshold: 0.7,
        }
    }
    /// Creates thresholds for CPU software rendering
    pub fn cpu() -> Self {
        Self {
            target_fps: 30.0,       // Lower target for CPU
            degrade_threshold: 1.2, // Very aggressive degrade
            upgrade_threshold: 0.8,
            degrade_frame_count: 2,  // Very fast degrade
            upgrade_frame_count: 20, // Very slow upgrade
            memory_pressure_threshold: 0.6,
            cpu_utilization_threshold: 0.5, // Lower CPU threshold
        }
    }
    /// Creates thresholds for a device type
    pub fn for_device_type(device_type: GpuDeviceType) -> Self {
        match device_type {
            GpuDeviceType::DiscreteGpu => Self::discrete(),
            GpuDeviceType::IntegratedGpu => Self::integrated(),
            _ => Self::cpu(),
        }
    }
    /// Returns target frame duration
    pub fn target_frame_duration(&self) -> Duration {
        Duration::from_secs_f32(1.0 / self.target_fps)
    }
    /// Returns degrade threshold duration
    pub fn degrade_duration(&self) -> Duration {
        Duration::from_secs_f32(self.target_frame_duration().as_secs_f32() * self.degrade_threshold)
    }
    /// Returns upgrade threshold duration
    pub fn upgrade_duration(&self) -> Duration {
        Duration::from_secs_f32(self.target_frame_duration().as_secs_f32() * self.upgrade_threshold)
    }
    /// Adjusts thresholds based on actual performance history
    pub fn adjust_based_on_performance(&mut self, avg_frame_time: Duration, stability: f32) {
        // If performance is unstable, make thresholds more conservative
        if stability < 0.5 {
            self.degrade_threshold *= 0.9;
            self.upgrade_threshold *= 1.1;
        }
        // If consistently missing target, lower expectations
        let avg_fps = 1.0 / avg_frame_time.as_secs_f32();
        if avg_fps < self.target_fps * 0.5 {
            self.target_fps *= 0.9;
        }
    }
}
/// Performance sample
#[derive(Debug, Clone, Copy)]
pub struct PerformanceSample {
    /// Frame index
    pub frame_index: u64,
    /// Frame duration
    pub frame_duration: Duration,
    /// GPU time (if available)
    pub gpu_time: Option<Duration>,
    /// CPU time
    pub cpu_time: Duration,
    /// Memory utilization (0.0 - 1.0)
    pub memory_utilization: f32,
    /// CPU utilization (0.0 - 1.0)
    pub cpu_utilization: f32,
    /// Timestamp
    pub timestamp: Instant,
}
/// Hardware-adaptive performance monitor
pub struct AdaptivePerformanceMonitor {
    strategy: PerformanceMonitorStrategy,
    thresholds: AdaptivePerformanceThresholds,
    samples: VecDeque<PerformanceSample>,
    max_samples: usize,
    current_frame: u64,
    frame_start: Instant,
    consecutive_bad_frames: usize,
    consecutive_good_frames: usize,
    last_quality_change: Instant,
}
impl AdaptivePerformanceMonitor {
    /// Creates a new monitor for a device type
    pub fn for_device_type(device_type: GpuDeviceType) -> Self {
        let strategy = PerformanceMonitorStrategy::for_device_type(device_type);
        let thresholds = AdaptivePerformanceThresholds::for_device_type(device_type);
        Self::new(strategy, thresholds)
    }
    /// Creates a new monitor with specific strategy and thresholds
    pub fn new(
        strategy: PerformanceMonitorStrategy,
        thresholds: AdaptivePerformanceThresholds,
    ) -> Self {
        Self {
            strategy,
            thresholds,
            samples: VecDeque::with_capacity(120),
            max_samples: 120,
            current_frame: 0,
            frame_start: Instant::now(),
            consecutive_bad_frames: 0,
            consecutive_good_frames: 0,
            last_quality_change: Instant::now() - Duration::from_secs(60),
        }
    }
    /// Starts a new frame
    pub fn begin_frame(&mut self) {
        self.frame_start = Instant::now();
        self.current_frame += 1;
    }
    /// Ends the current frame and records performance
    pub fn end_frame(&mut self) -> PerformanceSample {
        let frame_duration = self.frame_start.elapsed();
        let sample = PerformanceSample {
            frame_index: self.current_frame,
            frame_duration,
            gpu_time: self.measure_gpu_time(),
            cpu_time: frame_duration, // Simplified - in real impl, measure CPU separately
            memory_utilization: self.measure_memory_utilization(),
            cpu_utilization: self.measure_cpu_utilization(),
            timestamp: Instant::now(),
        };
        self.record_sample(sample);
        sample
    }
    /// Measures GPU time.
    ///
    /// Checks the `RUST_WIDGETS_GPU_TIME_MS` env var first (value in
    /// milliseconds, parsed as `f64`).  Without a wgpu context no real GPU
    /// timestamp query is possible, so `None` is returned when the env var
    /// is absent.
    fn measure_gpu_time(&self) -> Option<Duration> {
        // Env-var override: RUST_WIDGETS_GPU_TIME_MS (milliseconds, f64)
        if let Ok(val) = std::env::var("RUST_WIDGETS_GPU_TIME_MS") {
            if let Ok(ms) = val.trim().parse::<f64>() {
                return Some(Duration::from_secs_f64(ms / 1000.0));
            }
            log::warn!("[performance] RUST_WIDGETS_GPU_TIME_MS value '{}' is not a valid f64", val);
        }
        log::debug!(
            "[performance] measure_gpu_time: no GPU query backend available (strategy={:?})",
            self.strategy
        );
        None
    }
    /// Measures memory utilization as a fraction `[0.0, 1.0]`.
    ///
    /// Priority:
    /// 1. `RUST_WIDGETS_MEM_UTIL` env var override
    /// 2. Linux: `/proc/self/status` → `VmRSS / VmSize`
    /// 3. macOS: `ps` RSS vs estimated total memory
    /// 4. Fallback: `0.0` with diagnostic log
    fn measure_memory_utilization(&self) -> f32 {
        // 1. Env-var override
        if let Ok(val) = std::env::var("RUST_WIDGETS_MEM_UTIL") {
            if let Ok(v) = val.trim().parse::<f32>() {
                return v.clamp(0.0, 1.0);
            }
            log::warn!("[performance] RUST_WIDGETS_MEM_UTIL value '{}' is not a valid f32", val);
        }
        // 2. Linux: /proc/self/status
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                let mut vmrss_kb: u64 = 0;
                let mut vmsize_kb: u64 = 0;
                for line in status.lines() {
                    if let Some(rest) = line.strip_prefix("VmRSS:") {
                        vmrss_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
                    } else if let Some(rest) = line.strip_prefix("VmSize:") {
                        vmsize_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
                    }
                }
                if vmsize_kb > 0 {
                    let ratio = vmrss_kb as f32 / vmsize_kb as f32;
                    log::debug!(
                        "[performance] memory utilization from /proc/self/status: {ratio:.3}"
                    );
                    return ratio.clamp(0.0, 1.0);
                }
            }
        }
        // 3. macOS: ps RSS vs. estimated total
        #[cfg(target_os = "macos")]
        {
            let pid = std::process::id().to_string();
            if let Ok(output) =
                std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Ok(rss_kb) = stdout.trim().parse::<f64>() {
                        // Estimate 8 GB as reasonable total for a typical macOS system
                        let ratio = (rss_kb / (8.0 * 1024.0 * 1024.0)) as f32;
                        log::debug!("[performance] memory utilization from ps: {ratio:.3}");
                        return ratio.clamp(0.0, 1.0);
                    }
                }
            }
        }
        log::debug!("[performance] measure_memory_utilization: no backend available");
        0.0
    }
    /// Measures CPU utilization as a fraction `[0.0, 1.0]`.
    ///
    /// Priority:
    /// 1. `RUST_WIDGETS_CPU_UTIL` env var override
    /// 2. Linux: `/proc/self/status` → thread count / number of cores
    /// 3. Fallback: `0.0` with diagnostic log
    fn measure_cpu_utilization(&self) -> f32 {
        // 1. Env-var override
        if let Ok(val) = std::env::var("RUST_WIDGETS_CPU_UTIL") {
            if let Ok(v) = val.trim().parse::<f32>() {
                return v.clamp(0.0, 1.0);
            }
            log::warn!("[performance] RUST_WIDGETS_CPU_UTIL value '{}' is not a valid f32", val);
        }
        // 2. Linux: /proc/self/status → Threads:
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if let Some(rest) = line.strip_prefix("Threads:") {
                        if let Ok(threads) = rest.trim().parse::<f32>() {
                            // Rough heuristic: threads / (available_cores * 2)
                            // A well-utilized 4-core system might have ~8 active threads.
                            let cores = std::thread::available_parallelism()
                                .map(|n| n.get() as f32)
                                .unwrap_or(4.0);
                            let ratio = (threads / (cores * 2.0)).clamp(0.0, 1.0);
                            log::debug!(
                                "[performance] CPU utilization from /proc/self/status: {} threads / {} cores = {ratio:.3}",
                                threads, cores
                            );
                            return ratio;
                        }
                    }
                }
            }
        }
        log::debug!("[performance] measure_cpu_utilization: no backend available");
        0.0
    }
    /// Records a performance sample
    fn record_sample(&mut self, sample: PerformanceSample) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
        // Update consecutive frame counters
        if self.is_frame_bad(&sample) {
            self.consecutive_bad_frames += 1;
            self.consecutive_good_frames = 0;
        } else {
            self.consecutive_good_frames += 1;
            self.consecutive_bad_frames = 0;
        }
    }
    /// Checks if a frame is considered "bad"
    fn is_frame_bad(&self, sample: &PerformanceSample) -> bool {
        sample.frame_duration > self.thresholds.degrade_duration()
    }
    /// Checks if quality should be degraded
    pub fn should_degrade(&self) -> bool {
        if self.consecutive_bad_frames >= self.thresholds.degrade_frame_count {
            // Check cooldown period
            if self.last_quality_change.elapsed() > Duration::from_secs(2) {
                return true;
            }
        }
        false
    }
    /// Checks if quality should be upgraded
    pub fn should_upgrade(&self) -> bool {
        if self.consecutive_good_frames >= self.thresholds.upgrade_frame_count {
            // Check cooldown period
            if self.last_quality_change.elapsed() > Duration::from_secs(5) {
                return true;
            }
        }
        false
    }
    /// Notifies that quality has been changed
    pub fn notify_quality_changed(&mut self) {
        self.last_quality_change = Instant::now();
        self.consecutive_bad_frames = 0;
        self.consecutive_good_frames = 0;
    }
    /// Returns average frame time
    pub fn average_frame_time(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::from_secs(0);
        }
        let total: Duration = self.samples.iter().map(|s| s.frame_duration).sum();
        total / self.samples.len() as u32
    }
    /// Returns current FPS
    pub fn current_fps(&self) -> f32 {
        let avg = self.average_frame_time();
        if avg.as_secs_f32() > 0.0 {
            1.0 / avg.as_secs_f32()
        } else {
            0.0
        }
    }
    /// Returns performance stability (0.0 - 1.0)
    pub fn stability(&self) -> f32 {
        if self.samples.len() < 10 {
            return 1.0;
        }
        let avg = self.average_frame_time();
        let variance: f32 = self
            .samples
            .iter()
            .map(|s| {
                let diff = s.frame_duration.as_secs_f32() - avg.as_secs_f32();
                diff * diff
            })
            .sum::<f32>()
            / self.samples.len() as f32;
        let std_dev = variance.sqrt();
        let stability = 1.0 - (std_dev / avg.as_secs_f32()).min(1.0);
        stability.max(0.0)
    }
    /// Returns true if under memory pressure
    pub fn is_memory_pressure(&self) -> bool {
        if let Some(sample) = self.samples.back() {
            sample.memory_utilization > self.thresholds.memory_pressure_threshold
        } else {
            false
        }
    }
    /// Returns true if CPU is overloaded (for CPU rendering)
    pub fn is_cpu_overloaded(&self) -> bool {
        if let Some(sample) = self.samples.back() {
            sample.cpu_utilization > self.thresholds.cpu_utilization_threshold
        } else {
            false
        }
    }
    /// Returns performance statistics
    pub fn stats(&self) -> PerformanceStats {
        PerformanceStats {
            current_fps: self.current_fps(),
            average_frame_time: self.average_frame_time(),
            stability: self.stability(),
            consecutive_bad_frames: self.consecutive_bad_frames,
            consecutive_good_frames: self.consecutive_good_frames,
            is_memory_pressure: self.is_memory_pressure(),
            is_cpu_overloaded: self.is_cpu_overloaded(),
        }
    }
    /// Auto-adjusts thresholds based on performance history
    pub fn auto_adjust_thresholds(&mut self) {
        if self.samples.len() < 60 {
            return; // Need more data
        }
        let avg = self.average_frame_time();
        let stability = self.stability();
        self.thresholds.adjust_based_on_performance(avg, stability);
    }
    /// Returns the current thresholds
    pub fn thresholds(&self) -> &AdaptivePerformanceThresholds {
        &self.thresholds
    }
    /// Updates thresholds
    pub fn set_thresholds(&mut self, thresholds: AdaptivePerformanceThresholds) {
        self.thresholds = thresholds;
    }
}
/// Performance statistics
#[derive(Debug, Clone, Copy)]
pub struct PerformanceStats {
    /// Current FPS
    pub current_fps: f32,
    /// Average frame time
    pub average_frame_time: Duration,
    /// Performance stability (0.0 - 1.0)
    pub stability: f32,
    /// Consecutive bad frames
    pub consecutive_bad_frames: usize,
    /// Consecutive good frames
    pub consecutive_good_frames: usize,
    /// Whether under memory pressure
    pub is_memory_pressure: bool,
    /// Whether CPU is overloaded
    pub is_cpu_overloaded: bool,
}
/// Performance trap detector
pub struct PerformanceTrapDetector {
    low_fps_threshold: f32,
    sustained_low_fps_frames: usize,
    low_fps_counter: usize,
    last_warning: Instant,
}
impl PerformanceTrapDetector {
    /// Creates a new trap detector
    pub fn new(low_fps_threshold: f32, sustained_frames: usize) -> Self {
        Self {
            low_fps_threshold,
            sustained_low_fps_frames: sustained_frames,
            low_fps_counter: 0,
            last_warning: Instant::now() - Duration::from_secs(300),
        }
    }
    /// Checks for performance traps
    pub fn check(&mut self, fps: f32) -> Option<PerformanceTrap> {
        if fps < self.low_fps_threshold {
            self.low_fps_counter += 1;
            if self.low_fps_counter >= self.sustained_low_fps_frames
                && self.last_warning.elapsed() > Duration::from_secs(30)
            {
                self.last_warning = Instant::now();
                return Some(PerformanceTrap::LowFrameRate {
                    current_fps: fps,
                    threshold: self.low_fps_threshold,
                });
            }
        } else {
            self.low_fps_counter = 0;
        }
        None
    }
}
/// Performance trap types
#[derive(Debug, Clone)]
pub enum PerformanceTrap {
    /// Sustained low frame rate
    LowFrameRate { current_fps: f32, threshold: f32 },
    /// Memory pressure
    MemoryPressure { utilization: f32 },
    /// CPU overload (for CPU rendering)
    CpuOverload { utilization: f32 },
    /// Browser forcing integrated GPU
    BrowserForcedIntegratedGpu,
}
impl PerformanceTrap {
    /// Returns a user-friendly message
    pub fn message(&self) -> String {
        match self {
            Self::LowFrameRate { current_fps, threshold } => {
                format!(
                    "Performance warning: Frame rate is {:.1} FPS (below {:.1} FPS). Consider lowering graphics quality or closing other applications.",
                    current_fps, threshold
                )
            }
            Self::MemoryPressure { utilization } => {
                format!(
                    "Memory warning: GPU memory is {:.0}% full. Consider reducing texture quality or closing other applications.",
                    utilization * 100.0
                )
            }
            Self::CpuOverload { utilization } => {
                format!(
                    "CPU warning: CPU usage is {:.0}%. Software rendering is CPU-intensive. Consider using a GPU if available.",
                    utilization * 100.0
                )
            }
            Self::BrowserForcedIntegratedGpu => {
                "Browser is forcing integrated GPU. For best performance, try running outside browser".to_string()
            }
        }
    }
    /// Returns true if this trap suggests switching to CPU mode
    pub fn suggests_cpu_mode(&self) -> bool {
        matches!(self, Self::BrowserForcedIntegratedGpu)
    }
    /// Returns true if this trap suggests restarting
    pub fn suggests_restart(&self) -> bool {
        matches!(self, Self::BrowserForcedIntegratedGpu | Self::CpuOverload { .. })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_thresholds_for_device_type() {
        let discrete = AdaptivePerformanceThresholds::for_device_type(GpuDeviceType::DiscreteGpu);
        let integrated =
            AdaptivePerformanceThresholds::for_device_type(GpuDeviceType::IntegratedGpu);
        let cpu = AdaptivePerformanceThresholds::for_device_type(GpuDeviceType::Cpu);
        assert!(discrete.degrade_threshold > integrated.degrade_threshold);
        assert!(integrated.degrade_threshold > cpu.degrade_threshold);
        assert_eq!(discrete.target_fps, 60.0);
        assert_eq!(cpu.target_fps, 30.0);
    }
    #[test]
    fn test_performance_monitor() {
        let mut monitor = AdaptivePerformanceMonitor::for_device_type(GpuDeviceType::DiscreteGpu);
        monitor.begin_frame();
        std::thread::sleep(Duration::from_millis(10));
        let sample = monitor.end_frame();
        assert_eq!(sample.frame_index, 1);
        assert!(sample.frame_duration >= Duration::from_millis(10));
    }
    #[test]
    fn test_trap_detector() {
        let mut detector = PerformanceTrapDetector::new(30.0, 5);
        // Simulate low FPS
        for _ in 0..4 {
            assert!(detector.check(20.0).is_none());
        }
        // 5th low FPS should trigger
        let trap = detector.check(20.0);
        assert!(trap.is_some());
        if let Some(PerformanceTrap::LowFrameRate { current_fps, .. }) = trap {
            assert_eq!(current_fps, 20.0);
        }
    }
    #[test]
    fn test_trap_messages() {
        let trap = PerformanceTrap::LowFrameRate { current_fps: 15.0, threshold: 30.0 };
        let msg = trap.message();
        assert!(msg.contains("15.0"));
        assert!(msg.contains("30.0"));
    }
}
