// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::HashMap;
use crate::compat::Instant;
use core::time::Duration;
#[derive(Debug, Clone, Copy)]
/// Accumulated timing statistics for a single named section.
pub struct ProfileEntry {
    /// Instant at which the most recent `begin` call for this section happened.
    /// Only the last start is retained; elapsed time is measured against it.
    pub start: Instant,
    /// Total time spent inside the section, summed across every completed call.
    pub duration: Duration,
    /// Number of times the section was started *and* finished.
    pub call_count: u64,
}
impl Default for ProfileEntry {
    fn default() -> Self {
        Self { start: Instant::now(), duration: Duration::ZERO, call_count: 0 }
    }
}
/// Named-section profiler that accumulates per-section call counts and durations.
///
/// Only one section can be open at a time: `begin` overwrites any section that
/// was started but never ended, and that abandoned interval is not recorded.
/// Disabled instances make `begin`/`end`/`measure` no-ops.
pub struct Profiler {
    entries: HashMap<String, ProfileEntry>,
    current: Option<(String, Instant)>,
    enabled: bool,
}
impl Profiler {
    /// Creates a profiler with no recorded sections, already enabled.
    pub fn new() -> Self {
        Self { entries: HashMap::new(), current: None, enabled: true }
    }
    /// Resumes recording. Has no effect on sections already accumulated.
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    /// Suspends recording. `begin`/`end` become no-ops while disabled, so
    /// measurements taken in that window are silently dropped.
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    /// Returns whether recording is currently active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Starts timing a section under `name`, replacing any section still open.
    /// No-op when the profiler is disabled.
    pub fn begin(&mut self, name: &str) {
        if !self.enabled {
            return;
        }
        self.current = Some((name.to_string(), Instant::now()));
    }
    /// Stops timing the open section and adds the elapsed wall-clock time to it.
    /// Does nothing if no section is open or the profiler is disabled.
    pub fn end(&mut self) {
        if !self.enabled {
            return;
        }
        if let Some((name, start)) = self.current.take() {
            let duration = start.elapsed();
            let entry = self.entries.entry(name).or_default();
            entry.duration += duration;
            entry.call_count += 1;
        }
    }
    /// Times a single call to `f` as section `name` and returns its result.
    /// The call is only recorded while the profiler is enabled; `f` itself always runs.
    pub fn measure<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.begin(name);
        let result = f();
        self.end();
        result
    }
    /// Returns the accumulated entry for `name`, or `None` if it was never measured.
    pub fn get_stats(&self, name: &str) -> Option<&ProfileEntry> {
        self.entries.get(name)
    }
    /// Returns the mean wall-clock duration per completed call of `name`.
    /// `None` if the section is unknown or has never been completed.
    pub fn get_average_duration(&self, name: &str) -> Option<Duration> {
        self.entries.get(name).and_then(|e| {
            if e.call_count > 0 {
                Some(e.duration / e.call_count as u32)
            } else {
                None
            }
        })
    }
    /// Returns the sum of the accumulated durations of every recorded section,
    /// not the elapsed wall-clock time of the process. Zero when nothing was measured.
    pub fn get_total_duration(&self) -> Duration {
        self.entries.values().map(|e| e.duration).sum()
    }
    /// Returns every section whose accumulated duration is at least `threshold`,
    /// sorted from slowest to fastest. The threshold is inclusive and compares
    /// against totals, not per-call averages.
    pub fn get_hotspots(&self, threshold: Duration) -> Vec<(&str, Duration)> {
        let mut hotspots: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| e.duration >= threshold)
            .map(|(name, e)| (name.as_str(), e.duration))
            .collect();
        hotspots.sort_by_key(|b| core::cmp::Reverse(b.1));
        hotspots
    }
    /// Borrows the raw section table, keyed by the name passed to `begin`.
    pub fn get_all_stats(&self) -> &HashMap<String, ProfileEntry> {
        &self.entries
    }
    /// Drops all accumulated statistics and any section still open.
    pub fn reset(&mut self) {
        self.entries.clear();
        self.current = None;
    }
    /// Builds a snapshot report with one entry per recorded section, sorted by
    /// descending total duration. The report is detached from later measurements.
    pub fn report(&self) -> ProfileReport {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|(name, entry)| ProfileReportEntry {
                name: name.clone(),
                total_duration: entry.duration,
                call_count: entry.call_count,
                average_duration: if entry.call_count > 0 {
                    entry.duration / entry.call_count as u32
                } else {
                    Duration::ZERO
                },
            })
            .collect();
        entries.sort_by_key(|b| core::cmp::Reverse(b.total_duration));
        ProfileReport { entries, total_duration: self.get_total_duration() }
    }
}
crate::impl_default_via_new!(Profiler);
#[derive(Debug, Clone)]
/// One row of a [`ProfileReport`]: totals for a single named section.
pub struct ProfileReportEntry {
    /// Section name exactly as passed to `Profiler::begin`.
    pub name: String,
    /// Total wall-clock time accumulated by the section.
    pub total_duration: Duration,
    /// Number of completed calls that contributed to `total_duration`.
    pub call_count: u64,
    /// `total_duration` divided by `call_count`; `Duration::ZERO` when
    /// `call_count` is zero.
    pub average_duration: Duration,
}
#[derive(Debug, Clone)]
/// Immutable snapshot of a [`Profiler`] at the time `report` was called.
pub struct ProfileReport {
    /// Per-section rows, sorted by descending `total_duration`.
    pub entries: Vec<ProfileReportEntry>,
    /// Sum of all section totals in this snapshot.
    pub total_duration: Duration,
}
impl ProfileReport {
    /// Renders a multi-line plain-text summary: the overall total, then one
    /// line per section with its name, total, call count and average.
    pub fn to_string_summary(&self) -> String {
        let mut result = String::new();
        result.push_str(&format!("Total: {:?}\n\n", self.total_duration));
        for entry in &self.entries {
            result.push_str(&format!(
                "{}: {:?} ({} calls, avg {:?})\n",
                entry.name, entry.total_duration, entry.call_count, entry.average_duration
            ));
        }
        result
    }
}
/// Per-frame timing profiler with a rolling window of recent frame durations
/// and per-section timings for the frame currently in progress.
///
/// `begin_frame` clears the section table, so section timings always refer to the
/// most recently started frame; `frame_times` keeps at most `max_frames` samples
/// (the oldest is dropped on overflow).
pub struct FrameProfiler {
    frame_times: Vec<Duration>,
    max_frames: usize,
    current_frame_start: Option<Instant>,
    sections: HashMap<String, Duration>,
    current_section: Option<(String, Instant)>,
}
impl FrameProfiler {
    /// Creates a profiler that retains up to `max_frames` frame durations.
    /// With `max_frames` of zero, `end_frame` never stores a sample.
    pub fn new(max_frames: usize) -> Self {
        Self {
            frame_times: Vec::with_capacity(max_frames),
            max_frames,
            current_frame_start: None,
            sections: HashMap::new(),
            current_section: None,
        }
    }
    /// Marks the start of a frame and discards all section timings from the
    /// previous frame. Replaces any frame start that was not closed.
    pub fn begin_frame(&mut self) {
        self.current_frame_start = Some(Instant::now());
        self.sections.clear();
    }
    /// Closes the current frame and records its duration, evicting the oldest
    /// sample when the window is full. No-op if no frame is open.
    pub fn end_frame(&mut self) {
        if let Some(start) = self.current_frame_start.take() {
            let duration = start.elapsed();
            if self.frame_times.len() >= self.max_frames {
                self.frame_times.remove(0);
            }
            self.frame_times.push(duration);
        }
    }
    /// Starts timing a named section of the current frame, replacing any section
    /// still open. Section names are independent of frame boundaries.
    pub fn begin_section(&mut self, name: &str) {
        self.current_section = Some((name.to_string(), Instant::now()));
    }
    /// Stops the open section and adds the elapsed time to its running total for
    /// the current frame. Does nothing if no section is open.
    pub fn end_section(&mut self) {
        if let Some((name, start)) = self.current_section.take() {
            let duration = start.elapsed();
            *self.sections.entry(name).or_default() += duration;
        }
    }
    /// Returns the mean of the recorded frame durations, or `Duration::ZERO`
    /// when no frames have been recorded.
    pub fn average_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.frame_times.iter().sum();
        total / self.frame_times.len() as u32
    }
    /// Returns frames per second derived from `average_frame_time`
    /// (1e9 / average nanoseconds). Returns `0.0` when no frame has been timed.
    pub fn fps(&self) -> f32 {
        let avg = self.average_frame_time();
        if avg.is_zero() {
            return 0.0;
        }
        1_000_000_000.0 / avg.as_nanos() as f32
    }
    /// Returns the fastest recorded frame duration, or `Duration::ZERO` when no
    /// frames have been recorded.
    pub fn min_frame_time(&self) -> Duration {
        self.frame_times.iter().min().copied().unwrap_or(Duration::ZERO)
    }
    /// Returns the slowest recorded frame duration, or `Duration::ZERO` when no
    /// frames have been recorded.
    pub fn max_frame_time(&self) -> Duration {
        self.frame_times.iter().max().copied().unwrap_or(Duration::ZERO)
    }
    /// Returns how many frame durations are currently held, capped at the
    /// `max_frames` value given to `new`.
    pub fn frame_count(&self) -> usize {
        self.frame_times.len()
    }
    /// Borrows the per-section totals for the frame in progress. The map is
    /// emptied by `begin_frame` and is not cleared by `end_frame`.
    pub fn sections(&self) -> &HashMap<String, Duration> {
        &self.sections
    }
    /// Drops the recorded frame durations and all section timings. Does not
    /// close a frame that is currently open.
    pub fn clear(&mut self) {
        self.frame_times.clear();
        self.sections.clear();
    }
}
impl Default for FrameProfiler {
    fn default() -> Self {
        Self::new(60)
    }
}
/// Combines a [`Profiler`] for named sections with a [`FrameProfiler`] for frame
/// timing, so section and frame data come from one set of `begin`/`end` calls.
///
/// While disabled, every measurement entry point is skipped, but the accessors
/// still return whatever was collected before the monitor was disabled.
pub struct PerformanceMonitor {
    profiler: Profiler,
    frame_profiler: FrameProfiler,
    enabled: bool,
}
impl PerformanceMonitor {
    /// Creates a monitor with an empty section profiler, a frame profiler
    /// keeping the last 60 frame durations, and measurement enabled.
    pub fn new() -> Self {
        Self { profiler: Profiler::new(), frame_profiler: FrameProfiler::new(60), enabled: true }
    }
    /// Borrows the underlying section profiler.
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }
    /// Mutably borrows the underlying section profiler, for direct control over
    /// its enable state and measurements.
    pub fn profiler_mut(&mut self) -> &mut Profiler {
        &mut self.profiler
    }
    /// Borrows the underlying frame profiler.
    pub fn frame_profiler(&self) -> &FrameProfiler {
        &self.frame_profiler
    }
    /// Mutably borrows the underlying frame profiler.
    pub fn frame_profiler_mut(&mut self) -> &mut FrameProfiler {
        &mut self.frame_profiler
    }
    /// Resumes measurement and re-enables the section profiler.
    pub fn enable(&mut self) {
        self.enabled = true;
        self.profiler.enable();
    }
    /// Suspends measurement and disables the section profiler. Already-collected
    /// statistics remain readable through the accessors.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.profiler.disable();
    }
    /// Returns whether measurement is currently active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Opens a frame on the frame profiler; no-op while disabled.
    pub fn begin_frame(&mut self) {
        if self.enabled {
            self.frame_profiler.begin_frame();
        }
    }
    /// Closes the open frame; no-op while disabled.
    pub fn end_frame(&mut self) {
        if self.enabled {
            self.frame_profiler.end_frame();
        }
    }
    /// Opens a section in both profilers; no-op while disabled. Because the
    /// frame profiler tracks one section at a time, this replaces any section
    /// that was opened but not closed.
    pub fn begin_section(&mut self, name: &str) {
        if self.enabled {
            self.frame_profiler.begin_section(name);
            self.profiler.begin(name);
        }
    }
    /// Closes the open section in both profilers; no-op while disabled.
    pub fn end_section(&mut self) {
        if self.enabled {
            self.profiler.end();
            self.frame_profiler.end_section();
        }
    }
    /// Times one call to `f` as a section and returns its result.
    /// `f` always runs, even while the monitor is disabled.
    pub fn measure<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.begin_section(name);
        let result = f();
        self.end_section();
        result
    }
    /// Snapshots both profilers into a [`PerformanceReport`].
    pub fn report(&self) -> PerformanceReport {
        PerformanceReport {
            profiler_report: self.profiler.report(),
            average_frame_time: self.frame_profiler.average_frame_time(),
            fps: self.frame_profiler.fps(),
            min_frame_time: self.frame_profiler.min_frame_time(),
            max_frame_time: self.frame_profiler.max_frame_time(),
            frame_count: self.frame_profiler.frame_count(),
        }
    }
    /// Clears all section statistics and all recorded frame timings.
    pub fn reset(&mut self) {
        self.profiler.reset();
        self.frame_profiler.clear();
    }
}
crate::impl_default_via_new!(PerformanceMonitor);
#[derive(Debug, Clone)]
/// Combined snapshot of section timings and frame timings.
pub struct PerformanceReport {
    /// Named-section statistics, sorted by descending total duration.
    pub profiler_report: ProfileReport,
    /// Mean frame duration across the retained frame window; `Duration::ZERO`
    /// when no frame has been recorded.
    pub average_frame_time: Duration,
    /// Frames per second derived from `average_frame_time`; `0.0` when no frame
    /// has been recorded.
    pub fps: f32,
    /// Fastest retained frame duration; `Duration::ZERO` when no frame has been recorded.
    pub min_frame_time: Duration,
    /// Slowest retained frame duration; `Duration::ZERO` when no frame has been recorded.
    pub max_frame_time: Duration,
    /// Number of frame durations in the retained window.
    pub frame_count: usize,
}
impl PerformanceReport {
    /// Renders a multi-line plain-text summary: FPS and frame statistics first,
    /// then the section summary from [`ProfileReport::to_string_summary`].
    pub fn to_string_summary(&self) -> String {
        format!(
            "FPS: {:.1}\nAvg Frame: {:?}\nMin Frame: {:?}\nMax Frame: {:?}\nFrames: {}\n\n{}",
            self.fps,
            self.average_frame_time,
            self.min_frame_time,
            self.max_frame_time,
            self.frame_count,
            self.profiler_report.to_string_summary()
        )
    }
}
#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;
    use std::thread::sleep;
    #[test]
    fn test_profiler() {
        let mut profiler = Profiler::new();
        profiler.begin("test");
        sleep(Duration::from_millis(1));
        profiler.end();
        let stats = profiler.get_stats("test").unwrap();
        assert_eq!(stats.call_count, 1);
        assert!(stats.duration > Duration::ZERO);
    }
    #[test]
    fn test_frame_profiler() {
        let mut profiler = FrameProfiler::new(10);
        for _ in 0..5 {
            profiler.begin_frame();
            sleep(Duration::from_millis(1));
            profiler.end_frame();
        }
        assert_eq!(profiler.frame_count(), 5);
        assert!(profiler.fps() > 0.0);
    }
}
