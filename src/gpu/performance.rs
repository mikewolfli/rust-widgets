// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Hardware-adaptive performance monitoring and dynamic quality adjustment.
//!
//! This module provides performance monitoring that adapts to different hardware types:
//! - Discrete GPU: GPU timestamp-based monitoring
//! - Integrated GPU: Frame time + memory bandwidth monitoring
//! - CPU Software: CPU frame time + thread utilization monitoring
//!
//! # Relationship to `crate::performance` (G-214)
//!
//! The two timing systems have separate, complementary jobs and are meant to be
//! read together:
//!
//! * **This module** owns per-frame classification against adaptive thresholds
//!   (good/neutral/bad streaks, memory/CPU pressure, trap detection) and the
//!   quality decision those drive. [`PerformanceStats`] is its single statistics
//!   endpoint; nothing else here produces a second table for the same metric.
//! * [`crate::performance::profiler`] owns per-section attribution
//!   ([`FrameProfiler`](crate::performance::profiler::FrameProfiler) and
//!   [`Profiler`](crate::performance::profiler::Profiler)): which named stage of
//!   a frame spent the time. It does not decide quality.
//!
//! Neither monitors `crate::wgpu_backend` GPU timestamps today: `measure_gpu_time`
//! reports `None` unless the `RUST_WIDGETS_GPU_TIME_MS` override is set, because
//! this module has no device handle to issue a timestamp query on.
use super::adapter::GpuDeviceType;
use crate::compat::{format, Instant, MiniToString, OnceLock, String};
use alloc::collections::VecDeque;
use core::time::Duration;

/// The largest number of seconds `Duration::from_secs_f32` accepts without
/// panicking. `Duration` stores whole seconds in a `u64`, and `u64::MAX as f32`
/// rounds *up* past `u64::MAX`, so the bound is a comfortably smaller value that
/// is exactly representable and leaves room for the nanosecond conversion.
const MAX_DURATION_SECS_F32: f32 = 1.0e18;

/// Converts a caller-supplied frame rate to a *safe* frame duration in seconds.
///
/// `fps` is a public field, so it can be `0`, negative, `NaN` or infinite. The
/// result is clamped into `0.0 < secs <= MAX_DURATION_SECS_F32` so that passing it
/// to `Duration::from_secs_f32` can never panic; a non-positive or non-finite
/// rate falls back to 30 fps (N-S-61).
fn secs_from_fps(fps: f32) -> f32 {
    if !fps.is_finite() || fps <= 0.0 {
        // 1/30 is a sane, non-panicking default for a rate the caller did not
        // give us a usable value for.
        return 1.0 / 30.0;
    }
    let secs = 1.0 / fps;
    safe_duration_secs(secs)
}

/// Clamps `secs` into the range `Duration::from_secs_f32` accepts.
///
/// A non-finite or negative value becomes `0.0`; an over-large value clamps to
/// the representable maximum; and a tiny positive value is floored to one
/// nanosecond so the resulting `Duration` is strictly positive (a duration of
/// exactly zero would make `1.0 / duration` infinite downstream) (N-S-61).
fn safe_duration_secs(secs: f32) -> f32 {
    const MIN_POSITIVE_SECS: f32 = 1e-9;
    if !secs.is_finite() || secs < 0.0 {
        0.0
    } else {
        secs.clamp(MIN_POSITIVE_SECS, MAX_DURATION_SECS_F32)
    }
}

/// How many frames must be recorded between automatic threshold adjustments.
///
/// `GpuManager::end_frame` calls `auto_adjust_thresholds` every frame; without a
/// cadence a burst of unstable frames would push the multipliers to their bounds
/// almost immediately (N-S-62). 60 frames is roughly one second at 60 fps, which
/// is long enough to reflect sustained behaviour rather than a stutter.
const ADJUST_INTERVAL_FRAMES: usize = 60;

/// Classification of a single frame against the adaptivity thresholds.
///
/// A frame's duration is compared with both [`AdaptivePerformanceThresholds::upgrade_duration`]
/// and [`AdaptivePerformanceThresholds::degrade_duration`], which leaves three
/// outcomes rather than two: clearly good, clearly bad, and the neutral middle
/// band where the frame meets its target but does not earn an upgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameQuality {
    /// At or below the upgrade threshold.
    Good,
    /// Above the degrade threshold.
    Bad,
    /// Between the two thresholds — neither evidence for upgrade nor degrade.
    Neutral,
}
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
    /// Returns target frame duration.
    ///
    /// `target_fps` is a public, caller-settable `f32`, and `Duration::from_secs_f32`
    /// panics on `NaN`, infinity, negative values and values too large to
    /// represent. The value is sanitized to a small positive duration first, so
    /// this accessor is total rather than a panic waiting on a caller mistake
    /// (N-S-61).
    pub fn target_frame_duration(&self) -> Duration {
        Duration::from_secs_f32(secs_from_fps(self.target_fps))
    }
    /// Returns degrade threshold duration.
    ///
    /// Sanitized like [`Self::target_frame_duration`]: `degrade_threshold` is a
    /// public multiplier and an unvalidated `0.0`/`NaN`/negative value would make
    /// `from_secs_f32` panic (N-S-61).
    pub fn degrade_duration(&self) -> Duration {
        Duration::from_secs_f32(safe_duration_secs(
            self.target_frame_duration().as_secs_f32() * self.degrade_threshold,
        ))
    }
    /// Returns upgrade threshold duration.
    ///
    /// Sanitized like [`Self::degrade_duration`] (N-S-61).
    pub fn upgrade_duration(&self) -> Duration {
        Duration::from_secs_f32(safe_duration_secs(
            self.target_frame_duration().as_secs_f32() * self.upgrade_threshold,
        ))
    }
    /// Adjusts thresholds based on actual performance history.
    ///
    /// Two safety bounds keep repeated calls from running away (N-S-62):
    ///
    /// * The degrade/upgrade multipliers are clamped to a sane band. Repeated
    ///   `degrade *= 0.9` / `upgrade *= 1.1` previously drove them toward `0` and
    ///   `inf` and *inverted* them (upgrade above degrade), which made the good/bad
    ///   classification meaningless.
    /// * `target_fps` is clamped to a sane floor so repeated `*= 0.9` cannot reach
    ///   `0` (which would make `target_frame_duration` infinite).
    ///
    /// The bounds preserve the normal response: an unstable first call still
    /// widens the degrade window and narrows the upgrade window.
    pub fn adjust_based_on_performance(&mut self, avg_frame_time: Duration, stability: f32) {
        // Keep the multipliers apart even after many adjustments: the degrade
        // threshold is the *larger* one (a frame over it is bad) and the upgrade
        // threshold the smaller (a frame at or under it is good). `degrade` stays
        // at least `DEGRADE_FLOOR` and `upgrade` at most `UPGRADE_CEILING`, with
        // `DEGRADE_FLOOR > UPGRADE_CEILING`, so the two thresholds can never
        // invert (N-S-62).
        const DEGRADE_FLOOR: f32 = 1.1;
        const UPGRADE_CEILING: f32 = 0.9;
        // `target_fps` must stay positive; 10 fps is a low but workable floor.
        const TARGET_FPS_FLOOR: f32 = 10.0;

        // If performance is unstable, make thresholds more conservative.
        if stability < 0.5 {
            self.degrade_threshold = (self.degrade_threshold * 0.9).max(DEGRADE_FLOOR);
            self.upgrade_threshold = (self.upgrade_threshold * 1.1).min(UPGRADE_CEILING);
        }
        // If consistently missing target, lower expectations.
        let avg_fps = 1.0 / avg_frame_time.as_secs_f32();
        if avg_fps < self.target_fps * 0.5 {
            self.target_fps = (self.target_fps * 0.9).max(TARGET_FPS_FLOOR);
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
    /// CPU-side time.
    ///
    /// # Current semantics: this is the frame duration, not a CPU measurement
    ///
    /// This monitor has no submit boundary to bracket, and the platform layer
    /// exposes CPU *utilization* rather than a CPU-time counter, so no honest
    /// CPU-time interval can be measured here yet (G-55). The field therefore
    /// carries [`Self::frame_duration`], which includes any blocking or waiting
    /// the frame did; do **not** treat it as "time spent on the CPU" or use it to
    /// decide that a frame is CPU-bound. Use [`Self::gpu_time`] (when present) and
    /// [`Self::cpu_utilization`] for that question. A real value requires a
    /// submit hook that starts a timer before submission and stops it after the
    /// CPU side is done; that hook does not exist in this module.
    pub cpu_time: Duration,
    /// Memory utilization (0.0 - 1.0), or `None` when no reliable measurement exists.
    ///
    /// # Why this is optional
    ///
    /// A backend that cannot measure memory used to be recorded as `0.0`, which made "no
    /// measurement" indistinguishable from "idle". A consumer then read `0.0` as "plenty free",
    /// so an unmeasured platform was silently treated as unloaded. `None` carries the missing
    /// measurement as its own fact, so a quality decision can decline to judge rather than judge on
    /// a fabricated zero.
    pub memory_utilization: Option<f32>,
    /// CPU utilization (0.0 - 1.0), or `None` when no reliable measurement exists.
    ///
    /// See [`Self::memory_utilization`] for why the absence of a measurement is its own value.
    pub cpu_utilization: Option<f32>,
    /// Timestamp
    pub timestamp: Instant,
}

/// Environment inputs that are fixed for the process (G-51).
///
/// The overrides are environment variables, so they cannot change under a
/// running process. They used to be re-read on every frame: three
/// `std::env::var` calls per `end_frame`. Capturing them once removes that work
/// from the frame path.
///
/// The *measurements* are deliberately not captured: memory and CPU utilization
/// change over time, and freezing them would turn live pressure detection into a
/// single stale sample. Only the override values are fixed here;
/// `process_memory_utilization`/`process_cpu_utilization` are still called per
/// frame (the platform layer caches its own expensive probes).
#[derive(Debug, Clone, Copy, Default)]
struct EnvironmentSample {
    /// Parsed `RUST_WIDGETS_GPU_TIME_MS`, when the override is present and valid.
    gpu_time: Option<Duration>,
    /// Parsed `RUST_WIDGETS_MEM_UTIL`, when the override is present and valid.
    memory_utilization: Option<f32>,
    /// Parsed `RUST_WIDGETS_CPU_UTIL`, when the override is present and valid.
    cpu_utilization: Option<f32>,
}

impl EnvironmentSample {
    /// Reads every override once for the whole process (G-51).
    ///
    /// A `OnceLock` rather than a per-monitor read: the values are process-level
    /// facts, and the point of the cache is that the first frame pays for them and
    /// no later frame does, even if a second monitor is created.
    fn cached() -> Self {
        static SAMPLE: OnceLock<EnvironmentSample> = OnceLock::new();
        *SAMPLE.get_or_init(Self::capture)
    }

    /// Reads every override once.
    fn capture() -> Self {
        Self {
            gpu_time: std::env::var("RUST_WIDGETS_GPU_TIME_MS")
                .ok()
                .and_then(|raw| parse_gpu_time_override(&raw)),
            memory_utilization: std::env::var("RUST_WIDGETS_MEM_UTIL")
                .ok()
                .and_then(|raw| parse_ratio_override("RUST_WIDGETS_MEM_UTIL", &raw)),
            cpu_utilization: std::env::var("RUST_WIDGETS_CPU_UTIL")
                .ok()
                .and_then(|raw| parse_ratio_override("RUST_WIDGETS_CPU_UTIL", &raw)),
        }
    }
}

/// Parses the `RUST_WIDGETS_GPU_TIME_MS` override into a `Duration`.
///
/// `Duration::from_secs_f64` panics on negative/NaN/infinite or unrepresentably
/// large inputs, so a hostile or fat-fingered environment value must not crash the
/// renderer (N-S-61). An invalid value is reported and treated as "no override".
fn parse_gpu_time_override(raw: &str) -> Option<Duration> {
    match raw.trim().parse::<f64>() {
        Ok(ms) => {
            let secs = ms / 1000.0;
            if secs.is_finite() && secs >= 0.0 && secs <= u64::MAX as f64 {
                Some(Duration::from_secs_f64(secs))
            } else {
                log::warn!(
                    "[performance] RUST_WIDGETS_GPU_TIME_MS value '{raw}' is out of range for a \
                     Duration; ignoring it"
                );
                None
            }
        }
        Err(_) => {
            log::warn!("[performance] RUST_WIDGETS_GPU_TIME_MS value '{raw}' is not a valid f64");
            None
        }
    }
}

/// Parses a `[0.0, 1.0]` utilization override, clamping a valid number.
fn parse_ratio_override(name: &str, raw: &str) -> Option<f32> {
    match raw.trim().parse::<f32>() {
        Ok(value) => Some(value.clamp(0.0, 1.0)),
        Err(_) => {
            log::warn!("[performance] {name} value '{raw}' is not a valid f32");
            None
        }
    }
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
    /// Frames recorded since the last automatic threshold adjustment, used to
    /// throttle [`Self::auto_adjust_thresholds`] to a sane cadence.
    frames_since_adjust: usize,
    /// Environment overrides taken from the process-wide snapshot (G-51).
    environment: EnvironmentSample,
    /// Platform handle captured once at construction (G-51).
    ///
    /// `platform_facts()` is a process-wide singleton lookup, but `end_frame`
    /// used to perform it twice per frame. Holding the `&'static` handle makes
    /// the frame path free of the lookup. Consequence: a platform override
    /// installed *after* this monitor was created is not observed by it; the
    /// monitor's lifetime is the renderer's lifetime, so this is the intended
    /// boundary.
    platform: &'static dyn crate::platform::Platform,
    /// Running sum of the durations in `samples`, maintained on push/pop so
    /// [`Self::average_frame_time`] and [`Self::stability`] never rescan the
    /// window (G-52).
    total_frame_time: Duration,
    /// Running sum of squared frame durations, in seconds². Maintained alongside
    /// `total_frame_time` so the variance in [`Self::stability`] is O(1).
    sum_sq_frame_seconds: f64,
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
            // Backdated cooldown: the field is only ever read as
            // `last_quality_change.elapsed() > 2s / 5s` (see `should_degrade` /
            // `should_upgrade`), so it is seeded 60s in the past. That makes the
            // first adjustment of a freshly built monitor *allowed* rather than held
            // back by the cooldown, which is the initial state a new monitor is
            // specified to have. Seeding it with `Instant::now()` would instead make
            // `elapsed()` zero and suppress that first adjustment for 2s/5s.
            last_quality_change: Instant::now() - Duration::from_secs(60),
            frames_since_adjust: 0,
            environment: EnvironmentSample::cached(),
            platform: crate::platform::platform_facts(),
            total_frame_time: Duration::ZERO,
            sum_sq_frame_seconds: 0.0,
        }
    }
    #[cfg(test)]
    /// Builds a monitor with an explicit environment snapshot.
    ///
    /// Tests must not depend on (or mutate) process environment variables: the
    /// test harness runs cases in parallel, so a `set_var` in one case can leak
    /// into another. This constructor lets a test state the overrides directly.
    fn with_environment(environment: EnvironmentSample) -> Self {
        let mut monitor = Self::for_device_type(GpuDeviceType::DiscreteGpu);
        monitor.environment = environment;
        monitor
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
            // No CPU-side submit boundary exists to measure, so this carries the
            // frame duration and says so in the field docs (G-55).
            cpu_time: frame_duration,
            memory_utilization: self.measure_memory_utilization(),
            cpu_utilization: self.measure_cpu_utilization(),
            timestamp: Instant::now(),
        };
        self.record_sample(sample);
        sample
    }
    /// Measures GPU time.
    ///
    /// Returns the cached `RUST_WIDGETS_GPU_TIME_MS` override (milliseconds,
    /// parsed as `f64`) when one was present at construction. Without a wgpu
    /// context no real GPU timestamp query is possible, so `None` is returned
    /// otherwise. The override is read once (G-51) rather than per frame.
    fn measure_gpu_time(&self) -> Option<Duration> {
        if let Some(gpu_time) = self.environment.gpu_time {
            return Some(gpu_time);
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
    /// 1. cached `RUST_WIDGETS_MEM_UTIL` override (read once at construction)
    /// 2. Active platform backend (`Platform::process_memory_utilization`)
    /// 3. `None` (no reliable measurement available)
    ///
    /// The OS-specific probes (`/proc/self/status`, `ps`) live inside the
    /// platform backends, not here — see principle #36. The platform *handle* is
    /// cached, but the measurement itself is still taken per frame so pressure
    /// detection stays live (G-51).
    ///
    /// Returns `None` rather than `0.0` when no backend can measure: a missing measurement must not
    /// be recorded as "no pressure", or an unmeasured platform would sail past every pressure
    /// threshold (rule #37 — the absence of a capability is reported, never fabricated).
    fn measure_memory_utilization(&self) -> Option<f32> {
        // 1. Cached env-var override
        if let Some(value) = self.environment.memory_utilization {
            return Some(value);
        }
        // 2. Platform backend owns the OS probe
        if let Some(ratio) = self.platform.process_memory_utilization() {
            log::debug!("[performance] memory utilization from platform backend: {ratio:.3}");
            return Some(ratio.clamp(0.0, 1.0));
        }
        // 3. No reliable source: report the absence rather than a fabricated `0.0`.
        log::debug!("[performance] measure_memory_utilization: no backend available");
        None
    }
    /// Measures CPU utilization as a fraction `[0.0, 1.0]`.
    ///
    /// Priority:
    /// 1. cached `RUST_WIDGETS_CPU_UTIL` override (read once at construction)
    /// 2. Active platform backend (`Platform::process_cpu_utilization`)
    /// 3. `None` (no reliable measurement available)
    ///
    /// As with memory, the OS-specific sampling lives in the backend, a missing measurement is
    /// `None` rather than a fabricated `0.0`, and the measurement is taken per frame while only
    /// the override and the platform handle are cached (G-51).
    fn measure_cpu_utilization(&self) -> Option<f32> {
        // 1. Cached env-var override
        if let Some(value) = self.environment.cpu_utilization {
            return Some(value);
        }
        // 2. Platform backend owns the OS probe
        if let Some(ratio) = self.platform.process_cpu_utilization() {
            log::debug!("[performance] CPU utilization from platform backend: {ratio:.3}");
            return Some(ratio.clamp(0.0, 1.0));
        }
        log::debug!("[performance] measure_cpu_utilization: no backend available");
        None
    }
    /// Records a performance sample
    fn record_sample(&mut self, sample: PerformanceSample) {
        // Keep the running sums in step with the sliding window: subtract the
        // evicted sample before adding the new one, so `average_frame_time` and
        // `stability` never rescan the whole window (G-52).
        if self.samples.len() >= self.max_samples {
            if let Some(evicted) = self.samples.pop_front() {
                self.total_frame_time =
                    self.total_frame_time.saturating_sub(evicted.frame_duration);
                let secs = evicted.frame_duration.as_secs_f64();
                self.sum_sq_frame_seconds = (self.sum_sq_frame_seconds - secs * secs).max(0.0);
            }
        }
        let secs = sample.frame_duration.as_secs_f64();
        self.total_frame_time = self.total_frame_time.saturating_add(sample.frame_duration);
        self.sum_sq_frame_seconds += secs * secs;
        self.samples.push_back(sample);
        // Update consecutive frame counters. A frame is classified against
        // *both* thresholds: at or below `upgrade_duration` it is good, above
        // `degrade_duration` it is bad, and anything in between is neutral. The
        // old code put every frame at or below `degrade_duration` in the "good"
        // bucket, so a frame that missed the upgrade target still counted toward
        // the upgrade streak (N-S-60).
        match self.classify_frame(&sample) {
            FrameQuality::Bad => {
                self.consecutive_bad_frames += 1;
                self.consecutive_good_frames = 0;
            }
            FrameQuality::Good => {
                self.consecutive_good_frames += 1;
                self.consecutive_bad_frames = 0;
            }
            FrameQuality::Neutral => {
                // Neither streak continues: a frame that is neither clearly good
                // nor clearly bad must not count as evidence for either
                // adjustment.
                self.consecutive_bad_frames = 0;
                self.consecutive_good_frames = 0;
            }
        }
    }
    /// Classifies one frame against the degrade/upgrade thresholds.
    fn classify_frame(&self, sample: &PerformanceSample) -> FrameQuality {
        if sample.frame_duration > self.thresholds.degrade_duration() {
            FrameQuality::Bad
        } else if sample.frame_duration <= self.thresholds.upgrade_duration() {
            FrameQuality::Good
        } else {
            FrameQuality::Neutral
        }
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
    ///
    /// O(1): the total is maintained as samples enter and leave the window
    /// (G-52), so this does not rescan up to 120 samples per call.
    pub fn average_frame_time(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::from_secs(0);
        }
        self.total_frame_time / self.samples.len() as u32
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
    ///
    /// O(1): the sum of squared frame durations is maintained alongside the
    /// total (G-52). The variance is `E[x²] − E[x]²`, clamped at zero because
    /// floating-point rounding can make the algebraic form slightly negative for
    /// a perfectly uniform window.
    pub fn stability(&self) -> f32 {
        if self.samples.len() < 10 {
            return 1.0;
        }
        let avg = self.average_frame_time();
        let avg_secs = avg.as_secs_f32();
        if avg_secs <= 0.0 {
            // A zero mean would divide by zero; a window of zero-duration frames
            // is perfectly stable by definition.
            return 1.0;
        }
        let count = self.samples.len() as f64;
        let mean_square = self.sum_sq_frame_seconds / count;
        let variance = (mean_square - (avg_secs as f64) * (avg_secs as f64)).max(0.0);
        let std_dev = variance.sqrt() as f32;
        let stability = 1.0 - (std_dev / avg_secs).min(1.0);
        stability.max(0.0)
    }
    /// Returns true if under memory pressure.
    ///
    /// A sample with **no** memory measurement (`None`) reports no pressure: an unmeasured platform
    /// must not be judged as either loaded or idle, and the caller asking "is there pressure?" is
    /// answered `false` only because nothing said there was — never because a missing value was read
    /// as zero.
    pub fn is_memory_pressure(&self) -> bool {
        if let Some(sample) = self.samples.back() {
            sample
                .memory_utilization
                .is_some_and(|util| util > self.thresholds.memory_pressure_threshold)
        } else {
            false
        }
    }
    /// Returns true if CPU is overloaded (for CPU rendering).
    ///
    /// As with [`Self::is_memory_pressure`], a `None` CPU measurement is not a `0.0` and does not
    /// count as overloaded.
    pub fn is_cpu_overloaded(&self) -> bool {
        if let Some(sample) = self.samples.back() {
            sample
                .cpu_utilization
                .is_some_and(|util| util > self.thresholds.cpu_utilization_threshold)
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
    /// Auto-adjusts thresholds based on performance history.
    ///
    /// Runs at most once per `ADJUST_INTERVAL_FRAMES` frames, and only once at
    /// least 60 samples exist. Without a cadence this was called from
    /// `GpuManager::end_frame` on *every* frame, so a run of unstable frames made
    /// the thresholds converge to their bounds within a handful of frames instead
    /// of responding to sustained behaviour (N-S-62).
    pub fn auto_adjust_thresholds(&mut self) {
        // Only judge on a full history window.
        if self.samples.len() < 60 {
            return; // Need more data
        }
        // Cadence: adjust at most every `ADJUST_INTERVAL_FRAMES` recorded frames.
        self.frames_since_adjust = self.frames_since_adjust.saturating_add(1);
        if self.frames_since_adjust < ADJUST_INTERVAL_FRAMES {
            return;
        }
        self.frames_since_adjust = 0;
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
            // Backdated cooldown, same rationale as
            // `AdaptivePerformanceMonitor::last_quality_change`: the field is only
            // read as `last_warning.elapsed() > 30s`, so it is seeded 300s in the
            // past and the first trap fires as soon as the sustained-low-FPS
            // threshold is met, with no 30s warm-up on a fresh detector.
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
///
/// Conditions the performance monitor has recognised as worth reporting. Each
/// variant is a diagnosis, not a measurement: the monitor decides the threshold,
/// so the same raw metric can appear or not depending on configuration.
#[derive(Debug, Clone)]
pub enum PerformanceTrap {
    /// Sustained low frame rate.
    LowFrameRate {
        /// The measured frame rate, in frames per second.
        current_fps: f32,
        /// The rate below which the trap fires, in frames per second. Carried
        /// alongside the measurement so a report is self-explanatory.
        threshold: f32,
    },
    /// Memory pressure.
    MemoryPressure {
        /// Fraction of the memory budget in use, `0.0 ..= 1.0`.
        utilization: f32,
    },
    /// CPU overload (for CPU rendering).
    CpuOverload {
        /// Fraction of CPU capacity in use, `0.0 ..= 1.0`.
        utilization: f32,
    },
    /// Browser forcing integrated GPU.
    ///
    /// Has no payload: the condition is a fact about the environment rather than
    /// a measurement, and carries no actionable magnitude.
    BrowserForcedIntegratedGpu,
}
impl PerformanceTrap {
    /// Returns a user-friendly message
    pub fn message(&self) -> String {
        match self {
            Self::LowFrameRate { current_fps, threshold } => {
                format!(
                    "Performance warning: Frame rate is {current_fps:.1} FPS (below {threshold:.1} FPS). Consider lowering graphics quality or closing other applications."
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

    /// Build a sample with an explicit frame duration and no optional metrics.
    fn sample_of(frame_duration: Duration) -> PerformanceSample {
        PerformanceSample {
            frame_index: 0,
            frame_duration,
            gpu_time: None,
            cpu_time: frame_duration,
            memory_utilization: None,
            cpu_utilization: None,
            timestamp: Instant::now(),
        }
    }

    fn discrete_monitor() -> AdaptivePerformanceMonitor {
        AdaptivePerformanceMonitor::for_device_type(GpuDeviceType::DiscreteGpu)
    }

    /// N-S-60: `record_sample` must classify frames into three buckets using both
    /// thresholds, not treat everything under `degrade_duration` as good.
    #[test]
    fn record_sample_classifies_good_neutral_and_bad() {
        let monitor = discrete_monitor();
        let t = monitor.thresholds();
        let upgrade = t.upgrade_duration();
        let degrade = t.degrade_duration();
        assert!(upgrade < degrade, "the two thresholds must be ordered");

        assert_eq!(monitor.classify_frame(&sample_of(upgrade)), FrameQuality::Good);
        assert_eq!(monitor.classify_frame(&sample_of(upgrade / 2)), FrameQuality::Good);
        assert_eq!(monitor.classify_frame(&sample_of(degrade)), FrameQuality::Neutral);
        // Strictly between the two bounds is neutral.
        let mid = upgrade + (degrade - upgrade) / 2;
        assert_eq!(monitor.classify_frame(&sample_of(mid)), FrameQuality::Neutral);
        assert_eq!(
            monitor.classify_frame(&sample_of(degrade + Duration::from_millis(1))),
            FrameQuality::Bad
        );
    }

    /// A neutral frame must break both streaks, so it cannot silently accrue
    /// progress toward an upgrade as the old code allowed.
    #[test]
    fn neutral_frame_resets_both_streaks() {
        let mut monitor = discrete_monitor();
        let t = monitor.thresholds();
        let good = t.upgrade_duration();
        let degrade = t.degrade_duration();
        let neutral = t.upgrade_duration() + (degrade - t.upgrade_duration()) / 2;

        monitor.record_sample(sample_of(good));
        monitor.record_sample(sample_of(good));
        assert_eq!(monitor.stats().consecutive_good_frames, 2);

        monitor.record_sample(sample_of(neutral));
        let stats = monitor.stats();
        assert_eq!(stats.consecutive_good_frames, 0, "a neutral frame is not good evidence");
        assert_eq!(stats.consecutive_bad_frames, 0, "a neutral frame is not bad evidence");
    }

    /// N-S-60: `should_upgrade` must fire only after enough frames at or below
    /// the *upgrade* threshold, and respect the cooldown.
    #[test]
    fn should_upgrade_uses_upgrade_threshold_and_cooldown() {
        let mut monitor = discrete_monitor();
        // Force the cooldown to be satisfied.
        monitor.last_quality_change = Instant::now() - Duration::from_secs(10);
        let good = monitor.thresholds().upgrade_duration();
        let needed = monitor.thresholds().upgrade_frame_count;

        for _ in 0..needed {
            monitor.record_sample(sample_of(good));
        }
        assert!(monitor.should_upgrade(), "enough consecutive good frames must allow an upgrade");

        // A single frame just above the upgrade threshold (but below degrade) breaks
        // the streak and must suppress the upgrade.
        let neutral = monitor.thresholds().upgrade_duration() + Duration::from_micros(1);
        monitor.record_sample(sample_of(neutral));
        assert!(!monitor.should_upgrade(), "a non-good frame must reset the upgrade streak");

        // The cooldown still holds the upgrade back even with a full streak.
        monitor.notify_quality_changed();
        for _ in 0..needed {
            monitor.record_sample(sample_of(good));
        }
        assert!(!monitor.should_upgrade(), "the cooldown must still apply after a change");
    }

    /// N-S-61: pathological `target_fps` and multipliers must not panic; the
    /// accessors clamp to a representable, positive duration.
    #[test]
    fn thresholds_survive_pathological_values() {
        let mut t = AdaptivePerformanceThresholds::discrete();
        for fps in [0.0f32, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
            t.target_fps = fps;
            // Must not panic.
            let d = t.target_frame_duration();
            assert!(d > Duration::ZERO, "fps {fps} must yield a positive duration");
        }
        // A negative/NaN multiplier must not turn the duration accessors into a panic.
        for mult in [-1.0f32, f32::NAN, f32::INFINITY, f32::MAX] {
            t.target_fps = 60.0;
            t.degrade_threshold = mult;
            t.upgrade_threshold = mult;
            let _ = t.degrade_duration();
            let _ = t.upgrade_duration();
        }
    }

    /// The env-var override for GPU time must ignore out-of-range values rather
    /// than panicking inside `Duration::from_secs_f64` (N-S-61), and the override
    /// must be read once and reused (G-51).
    ///
    /// The parser is exercised directly, and the monitor gets an explicit
    /// snapshot: mutating process environment from a test would race the other
    /// cases the harness runs in parallel.
    #[test]
    fn gpu_time_env_override_is_range_checked() {
        // A negative millisecond value would make `from_secs_f64` panic.
        assert_eq!(parse_gpu_time_override("-5"), None, "a negative duration is rejected");
        assert_eq!(parse_gpu_time_override("nan"), None, "NaN is rejected");
        assert_eq!(parse_gpu_time_override("inf"), None, "infinity is rejected");
        assert_eq!(
            parse_gpu_time_override("12.5"),
            Some(Duration::from_secs_f64(0.0125)),
            "a normal value is converted"
        );

        // The cached snapshot is what `end_frame` reads, so a monitor keeps the
        // same override for its whole lifetime.
        let monitor = AdaptivePerformanceMonitor::with_environment(EnvironmentSample {
            gpu_time: Some(Duration::from_secs_f64(0.0125)),
            ..EnvironmentSample::default()
        });
        assert_eq!(monitor.measure_gpu_time(), Some(Duration::from_secs_f64(0.0125)));
        assert_eq!(monitor.measure_gpu_time(), Some(Duration::from_secs_f64(0.0125)));

        // No snapshot means no override, regardless of what the environment holds.
        let monitor = AdaptivePerformanceMonitor::with_environment(EnvironmentSample::default());
        assert_eq!(monitor.measure_gpu_time(), None);
    }

    /// G-52: the incremental sums must stay equal to a direct scan of the window,
    /// including after samples are evicted from the front.
    #[test]
    fn incremental_frame_sums_match_a_full_scan() {
        let mut monitor = discrete_monitor();
        for i in 0..(monitor.max_samples + 7) {
            let ms = 8 + (i as u64 % 11);
            monitor.record_sample(sample_of(Duration::from_millis(ms)));

            let (sum, sum_sq, count) = monitor.samples.iter().fold(
                (Duration::ZERO, 0.0_f64, 0_u32),
                |(sum, sum_sq, count), sample| {
                    let secs = sample.frame_duration.as_secs_f64();
                    (sum + sample.frame_duration, sum_sq + secs * secs, count + 1)
                },
            );
            assert_eq!(monitor.total_frame_time, sum, "running total drifted");
            assert!(
                (monitor.sum_sq_frame_seconds - sum_sq).abs() < 1e-9,
                "running square sum drifted"
            );
            assert_eq!(monitor.samples.len(), count as usize);
        }
    }

    /// N-S-62: a long run of unstable frames must keep the thresholds bounded and
    /// never let the degrade/upgrade multipliers invert.
    #[test]
    fn repeated_adjustment_stays_bounded_and_ordered() {
        let mut t = AdaptivePerformanceThresholds::discrete();
        // Far more adjustments than any real run of frames.
        for _ in 0..1000 {
            t.adjust_based_on_performance(Duration::from_millis(200), 0.0);
        }
        assert!(
            t.degrade_threshold >= 1.1,
            "degrade must not collapse to the upgrade side: {}",
            t.degrade_threshold
        );
        assert!(t.upgrade_threshold <= 0.9, "upgrade must stay bounded: {}", t.upgrade_threshold);
        assert!(
            t.degrade_threshold > t.upgrade_threshold,
            "the thresholds must never invert (degrade {} vs upgrade {})",
            t.degrade_threshold,
            t.upgrade_threshold
        );
        assert!(t.target_fps >= 10.0, "target_fps must stay usable: {}", t.target_fps);
        assert!(t.target_frame_duration() > Duration::ZERO);
        assert!(t.degrade_duration() > t.upgrade_duration());
    }

    /// N-S-62: the adjustment cadence must throttle repeated `auto_adjust_thresholds`
    /// calls so a burst of unstable frames cannot converge the thresholds at once;
    /// a normal (stable) sequence must still leave them at their defaults.
    #[test]
    fn auto_adjust_thresholds_is_rate_limited() {
        let mut monitor = discrete_monitor();
        let before = monitor.thresholds().degrade_threshold;

        // Feed 60 samples with a wide spread so `stability()` is well below 0.5.
        // The threshold is only adjusted once the cadence is crossed, so the first
        // `ADJUST_INTERVAL_FRAMES - 1` calls must not change anything.
        for i in 0..60 {
            let ms = if i % 2 == 0 { 4 } else { 200 };
            monitor.record_sample(sample_of(Duration::from_millis(ms)));
        }
        assert!(monitor.stability() < 0.5, "the fixture must be unstable");
        for _ in 0..(ADJUST_INTERVAL_FRAMES - 1) {
            monitor.auto_adjust_thresholds();
        }
        assert_eq!(
            monitor.thresholds().degrade_threshold,
            before,
            "an adjustment before the cadence elapses must not change the thresholds"
        );

        // The next call crosses the cadence and applies the (conservative) change.
        monitor.auto_adjust_thresholds();
        assert!(
            monitor.thresholds().degrade_threshold < before,
            "once the cadence elapses, an unstable run widens the degrade window"
        );
        let wider = monitor.thresholds().degrade_threshold;
        // Immediate successor calls must not keep adjusting.
        monitor.auto_adjust_thresholds();
        assert_eq!(monitor.thresholds().degrade_threshold, wider, "the cadence must hold");
    }
}
