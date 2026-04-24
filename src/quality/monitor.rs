//! Frame time monitor for tracking rendering performance.
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
            frame_times: vec![0.0; 60],
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
