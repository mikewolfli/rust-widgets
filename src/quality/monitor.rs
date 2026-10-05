// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Frame time monitor for tracking rendering performance.
use crate::compat::{vec, Vec};

/// Default number of frame samples a monitor retains.
///
/// Kept as a named constant because the history window is part of the monitor's observable
/// contract: `should_degrade`/`should_upgrade` can only ever look back over a sample count the
/// buffer can actually hold.
pub const DEFAULT_HISTORY_CAPACITY: usize = 60;

/// Frame time monitor for tracking rendering performance with lightweight statistics.
///
/// # Accepted samples
///
/// A sample is accepted only when it is **finite and non-negative**:
///
/// * a non-finite value (`NaN`, `±Infinity`) has no place on a timeline and would poison both the
///   average and the degrade/upgrade comparisons — a `NaN` sample used to satisfy *both* verdicts at
///   once, so a single bad frame could not be attributed to either policy;
/// * a negative duration is not a duration at all (it would make the average shrink when more work
///   is done).
///
/// Rejected samples are dropped without touching the ring buffer, so a caller cannot use a bad
/// sample to evict a good one. `0.0` is accepted: it is the honest value for a frame that cost no
/// measurable time, and it is what `reset` fills the buffer with.
#[derive(Debug, Clone)]
pub struct FrameTimeMonitor {
    frame_times: Vec<f32>,
    index: usize,
    count: usize,
    target_frame_time: f32,
}
impl FrameTimeMonitor {
    /// Creates a new frame time monitor with the specified target frame rate.
    ///
    /// The history window defaults to [`DEFAULT_HISTORY_CAPACITY`] samples. Use
    /// [`Self::with_capacity`] when a longer consecutive-frame window is needed.
    pub fn new(target_frame_rate: f32) -> Self {
        Self::with_capacity(target_frame_rate, DEFAULT_HISTORY_CAPACITY)
    }

    /// Creates a monitor whose history window holds `capacity` samples.
    ///
    /// `capacity` is raised to at least one so the ring buffer is never zero-length — a
    /// zero-length history could never satisfy any `consecutive_frames` request and indexing it
    /// would be a division by zero. See [`Self::with_capacity_for_counts`] to size the window from
    /// the consecutive-frame counts a config asks for.
    pub fn with_capacity(target_frame_rate: f32, capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            frame_times: vec![0.0; capacity],
            index: 0,
            count: 0,
            target_frame_time: 1.0 / target_frame_rate,
        }
    }

    /// Creates a monitor whose history window is large enough to satisfy the larger of two
    /// consecutive-frame request counts.
    ///
    /// The monitor can only ever look back over as many samples as its buffer holds, so a
    /// configuration that asks for a `degrade_frame_count` of 90 over a 60-sample window would
    /// silently never degrade. Sizing the window from the counts keeps the buffer and the policy
    /// from disagreeing.
    pub fn with_capacity_for_counts(
        target_frame_rate: f32,
        degrade_frame_count: usize,
        upgrade_frame_count: usize,
    ) -> Self {
        let required = degrade_frame_count.max(upgrade_frame_count).max(1);
        Self::with_capacity(target_frame_rate, required)
    }

    /// Number of samples the history window can hold.
    pub fn capacity(&self) -> usize {
        self.frame_times.len()
    }

    /// Records a frame duration in seconds.
    ///
    /// The sample is accepted only when it is finite and non-negative; see the type-level
    /// documentation for why. Rejected samples leave the history untouched.
    pub fn record_frame(&mut self, frame_duration: f32) {
        if !frame_duration.is_finite() || frame_duration < 0.0 {
            return;
        }
        self.frame_times[self.index] = frame_duration;
        self.index = (self.index + 1) % self.frame_times.len();
        self.count = self.count.saturating_add(1).min(self.frame_times.len());
    }

    /// Returns the average frame time over the recorded frames.
    ///
    /// Returns `0.0` when nothing has been recorded, and skips any non-finite slots that survive in
    /// the buffer (the window is pre-filled with zeros, and rejected samples never write), so the
    /// result is always finite.
    pub fn average_frame_time(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }
        let mut sum = 0.0f32;
        let mut samples = 0usize;
        for &time in self.frame_times.iter().take(self.count) {
            if time.is_finite() {
                sum += time;
                samples += 1;
            }
        }
        if samples == 0 {
            return 0.0;
        }
        sum / samples as f32
    }

    /// Returns the current frame rate based on average frame time.
    ///
    /// Returns `0.0` rather than a non-finite value when the average is unusable, so a caller can
    /// print the number without a `NaN` fanning out into every downstream calculation.
    pub fn current_fps(&self) -> f32 {
        let avg = self.average_frame_time();
        if avg.is_finite() && avg > 0.0 {
            1.0 / avg
        } else {
            0.0
        }
    }

    /// Returns the start index of the most recent `consecutive_frames` samples, or `None` when
    /// fewer samples have been recorded.
    ///
    /// The window is `None` for a zero-sized request because there is no meaningful "zero most
    /// recent frames" span to test.
    fn recent_window_start(&self, consecutive_frames: usize) -> Option<usize> {
        if consecutive_frames == 0 || self.count < consecutive_frames {
            return None;
        }
        // `index` is the write cursor: the sample *before* it is the most recent. Stepping back
        // `consecutive_frames` from there, modulo the ring, lands on the oldest sample in the
        // window. Both subtractions are guarded because `index` can be smaller than the window.
        let len = self.frame_times.len();
        let start = (self.index + len - (consecutive_frames % len)) % len;
        Some(start)
    }

    /// Checks if quality should be degraded based on frame time threshold.
    ///
    /// Returns `false` unless *every* sample in the most recent `consecutive_frames` window is
    /// finite and strictly greater than `threshold_duration`. A window that cannot be filled (the
    /// monitor holds fewer samples, or `consecutive_frames` is zero) is `false`.
    pub fn should_degrade(&self, threshold_duration: f32, consecutive_frames: usize) -> bool {
        let Some(start) = self.recent_window_start(consecutive_frames) else {
            return false;
        };
        let len = self.frame_times.len();
        for i in 0..consecutive_frames {
            let idx = (start + i) % len;
            let time = self.frame_times[idx];
            if !time.is_finite() || time <= threshold_duration {
                return false;
            }
        }
        true
    }

    /// Checks if quality should be upgraded based on frame time threshold.
    ///
    /// Returns `false` unless *every* sample in the most recent `consecutive_frames` window is
    /// finite and less than or equal to `threshold_duration`. A window that cannot be filled is
    /// `false`.
    pub fn should_upgrade(&self, threshold_duration: f32, consecutive_frames: usize) -> bool {
        let Some(start) = self.recent_window_start(consecutive_frames) else {
            return false;
        };
        let len = self.frame_times.len();
        for i in 0..consecutive_frames {
            let idx = (start + i) % len;
            let time = self.frame_times[idx];
            if !time.is_finite() || time > threshold_duration {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A single non-finite or negative sample must not satisfy either verdict, and must not skew the
    /// average. The defect being fixed is that a `NaN` sample used to make *both* `should_degrade`
    /// and `should_upgrade` return true at once — the two policies contradicting each other from one
    /// bad observation.
    #[test]
    fn an_invalid_sample_is_rejected_and_cannot_satisfy_contradictory_verdicts() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
            let mut monitor = FrameTimeMonitor::new(60.0);
            monitor.record_frame(bad);
            assert_eq!(monitor.average_frame_time(), 0.0, "{bad} must not enter the average");
            assert_eq!(monitor.count, 0, "{bad} must not be counted as a sample");
            // The window cannot be filled, so neither policy fires.
            assert!(!monitor.should_degrade(0.02, 1), "{bad} satisfied should_degrade");
            assert!(!monitor.should_upgrade(0.01, 1), "{bad} satisfied should_upgrade");
        }
    }

    /// A rejected sample must not evict a good one: the ring buffer is only written for accepted
    /// samples.
    #[test]
    fn a_rejected_sample_does_not_evict_a_recorded_one() {
        let mut monitor = FrameTimeMonitor::new(60.0);
        monitor.record_frame(0.016);
        monitor.record_frame(f32::NAN);
        monitor.record_frame(-5.0);
        assert_eq!(monitor.count, 1, "only the finite, non-negative sample was stored");
        assert!((monitor.average_frame_time() - 0.016).abs() < 1e-9);
    }

    /// A valid sample still drives both decisions the way it always did.
    #[test]
    fn valid_samples_drive_degrade_and_upgrade_normally() {
        let mut slow = FrameTimeMonitor::new(60.0);
        for _ in 0..3 {
            slow.record_frame(0.05);
        }
        assert!(slow.should_degrade(0.02, 3), "three slow frames must degrade");
        assert!(!slow.should_upgrade(0.02, 3), "slow frames must not upgrade");

        let mut fast = FrameTimeMonitor::new(60.0);
        for _ in 0..3 {
            fast.record_frame(0.005);
        }
        assert!(fast.should_upgrade(0.01, 3), "three fast frames must upgrade");
        assert!(!fast.should_degrade(0.01, 3), "fast frames must not degrade");
    }

    /// A window of 60 fills a 60-sample buffer exactly, and 61 asks for more than the buffer holds so
    /// it is refused rather than reading stale/zero slots. This pins the boundary the old fixed `60`
    /// buffer had no way to satisfy.
    #[test]
    fn a_window_larger_than_the_buffer_is_refused_rather_than_reading_stale_slots() {
        let mut monitor = FrameTimeMonitor::with_capacity(60.0, 60);
        for _ in 0..60 {
            monitor.record_frame(0.05);
        }
        assert_eq!(monitor.count, 60);
        assert!(monitor.should_degrade(0.02, 60), "a full 60-sample window must degrade");
        assert!(
            !monitor.should_degrade(0.02, 61),
            "61 samples cannot be shown when the buffer holds 60"
        );
        assert!(!monitor.should_upgrade(0.02, 61));
    }

    /// Sizing the buffer from the configured counts makes a longer window reachable: a 90-frame
    /// window over a 90-sample buffer must degrade once 90 slow frames arrive, and must keep wrapping
    /// correctly afterwards.
    #[test]
    fn a_longer_window_is_reachable_when_the_buffer_is_sized_for_it() {
        let mut monitor = FrameTimeMonitor::with_capacity_for_counts(60.0, 90, 30);
        assert_eq!(monitor.capacity(), 90);
        for _ in 0..89 {
            monitor.record_frame(0.05);
        }
        assert!(!monitor.should_degrade(0.02, 90), "89 of 90 samples is not yet a full window");
        monitor.record_frame(0.05);
        assert!(monitor.should_degrade(0.02, 90), "the 90th slow frame completes the window");
        // Wrap the ring several times; the most-recent window must still be honoured.
        for _ in 0..200 {
            monitor.record_frame(0.001);
        }
        assert!(!monitor.should_degrade(0.02, 90), "recent frames are fast, so no degrade");
        assert!(monitor.should_upgrade(0.02, 90), "recent frames are all fast");
    }

    /// The wrap-around arithmetic must look at the most recent samples, not the first ones. After
    /// more samples than the buffer holds, the old frames are gone.
    #[test]
    fn the_window_wraps_to_the_most_recent_samples() {
        let mut monitor = FrameTimeMonitor::with_capacity(60.0, 4);
        // Fill with slow frames, then overwrite with fast ones for two full cycles.
        for _ in 0..4 {
            monitor.record_frame(0.05);
        }
        assert!(monitor.should_degrade(0.02, 4));
        for _ in 0..8 {
            monitor.record_frame(0.001);
        }
        assert!(!monitor.should_degrade(0.02, 4), "the slow frames were evicted by the wrap");
        assert!(monitor.should_upgrade(0.02, 4), "the newest four frames are fast");
    }

    /// A zero-capacity request still yields a usable one-sample buffer, and a zero-sized window is
    /// refused rather than treated as vacuously true.
    #[test]
    fn a_zero_capacity_buffer_is_raised_to_one_and_a_zero_window_is_refused() {
        let mut monitor = FrameTimeMonitor::with_capacity(60.0, 0);
        assert_eq!(monitor.capacity(), 1);
        monitor.record_frame(0.05);
        assert_eq!(monitor.count, 1);
        assert!(monitor.should_degrade(0.02, 1));
        assert!(!monitor.should_degrade(0.02, 0), "zero frames is not a window");
        assert!(!monitor.should_upgrade(0.02, 0));
    }
}
