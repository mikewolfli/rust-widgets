use alloc::collections::VecDeque;
use core::time::Duration;
use crate::compat::Instant;

/// Tracks frame timestamps and computes running average FPS and frame time
/// over a configurable sliding window (default 60 frames).
pub struct FrameTimer {
    timestamps: VecDeque<Instant>,
    window_size: usize,
}

impl FrameTimer {
    /// Creates a new `FrameTimer` with a 60-frame sliding window.
    pub fn new() -> Self {
        Self { timestamps: VecDeque::with_capacity(60), window_size: 60 }
    }

    /// Creates a new `FrameTimer` with a custom window size.
    pub fn with_window_size(window_size: usize) -> Self {
        Self { timestamps: VecDeque::with_capacity(window_size), window_size }
    }

    /// Records a new frame timestamp.
    ///
    /// Returns the duration elapsed since the last recorded frame, or
    /// [`Duration::ZERO`] if this is the first frame.
    pub fn tick(&mut self) -> Duration {
        let now = Instant::now();
        let elapsed =
            self.timestamps.back().map_or(Duration::ZERO, |last| now.duration_since(*last));

        self.timestamps.push_back(now);

        // Evict oldest timestamps beyond the window limit
        while self.timestamps.len() > self.window_size {
            self.timestamps.pop_front();
        }

        elapsed
    }

    /// Returns the average FPS over the current sliding window.
    ///
    /// Returns `0.0` if fewer than 2 frames have been recorded.
    pub fn average_fps(&self) -> f64 {
        let count = self.timestamps.len();
        if count < 2 {
            return 0.0;
        }
        let first = self.timestamps.front().unwrap();
        let last = self.timestamps.back().unwrap();
        let total_secs = last.duration_since(*first).as_secs_f64();
        if total_secs <= 0.0 {
            return 0.0;
        }
        (count - 1) as f64 / total_secs
    }

    /// Returns the average frame time in milliseconds over the current
    /// sliding window.
    ///
    /// Returns `0.0` if fewer than 2 frames have been recorded.
    pub fn frame_time_ms(&self) -> f64 {
        let count = self.timestamps.len();
        if count < 2 {
            return 0.0;
        }
        let first = self.timestamps.front().unwrap();
        let last = self.timestamps.back().unwrap();
        let total_ms = last.duration_since(*first).as_secs_f64() * 1000.0;
        if total_ms <= 0.0 {
            return 0.0;
        }
        total_ms / (count - 1) as f64
    }

    /// Clears all recorded frame timestamps, resetting the timer.
    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_new_timer_is_empty() {
        let timer = FrameTimer::new();
        assert_eq!(timer.average_fps(), 0.0);
        assert_eq!(timer.frame_time_ms(), 0.0);
    }

    #[test]
    fn test_first_tick_returns_zero() {
        let mut timer = FrameTimer::new();
        let elapsed = timer.tick();
        assert_eq!(elapsed, Duration::ZERO);
    }

    #[test]
    fn test_tick_returns_elapsed() {
        let mut timer = FrameTimer::new();
        timer.tick(); // first tick, ZERO
        thread::sleep(Duration::from_millis(10));
        let elapsed = timer.tick();
        assert!(elapsed >= Duration::from_millis(10));
    }

    #[test]
    fn test_average_fps_smoke() {
        let mut timer = FrameTimer::new();
        // Record 10 frames with ~10ms spacing
        for _ in 0..10 {
            timer.tick();
            thread::sleep(Duration::from_millis(10));
        }
        let fps = timer.average_fps();
        // At ~10ms per frame, ~100 FPS, allow wide tolerance
        assert!(fps > 50.0 && fps < 200.0);
    }

    #[test]
    fn test_frame_time_ms_smoke() {
        let mut timer = FrameTimer::new();
        for _ in 0..10 {
            timer.tick();
            thread::sleep(Duration::from_millis(10));
        }
        let ft = timer.frame_time_ms();
        assert!(ft > 5.0 && ft < 20.0);
    }

    #[test]
    fn test_window_size_limit() {
        let mut timer = FrameTimer::with_window_size(5);
        for _ in 0..10 {
            timer.tick();
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(timer.timestamps.len(), 5);
    }

    #[test]
    fn test_reset_clears() {
        let mut timer = FrameTimer::new();
        timer.tick();
        thread::sleep(Duration::from_millis(5));
        timer.tick();
        assert!(timer.average_fps() > 0.0);
        timer.reset();
        assert_eq!(timer.average_fps(), 0.0);
        assert_eq!(timer.frame_time_ms(), 0.0);
    }

    #[test]
    fn test_default_equals_new() {
        let a = FrameTimer::new();
        let b = FrameTimer::default();
        assert_eq!(a.window_size, b.window_size);
    }
}
