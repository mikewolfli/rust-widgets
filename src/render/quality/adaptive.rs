//! Adaptive rendering quality optimization.
use crate::quality::QualityLevel;
/// Adaptive rendering optimizer that adjusts quality based on performance.
pub struct AdaptiveRenderer {
    current_quality: QualityLevel,
    frame_times: Vec<f32>,
    target_fps: f32,
    degrade_count: usize,
    upgrade_count: usize,
}
impl AdaptiveRenderer {
    /// Creates a new adaptive renderer with target FPS.
    pub fn new(target_fps: f32) -> Self {
        Self {
            current_quality: QualityLevel::High,
            frame_times: Vec::with_capacity(120), // Keep 2 seconds of frame times
            target_fps,
            degrade_count: 0,
            upgrade_count: 0,
        }
    }
    /// Updates frame time and adjusts quality if needed.
    pub fn update_frame_time(&mut self, frame_time: f32) -> QualityLevel {
        self.frame_times.push(frame_time);
        // Keep only recent frame times
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }
        // Calculate average frame time over last 60 frames (1 second)
        let recent_frames = if self.frame_times.len() >= 60 {
            &self.frame_times[self.frame_times.len() - 60..]
        } else {
            &self.frame_times
        };
        let avg_frame_time = if !recent_frames.is_empty() {
            recent_frames.iter().sum::<f32>() / recent_frames.len() as f32
        } else {
            frame_time
        };
        let target_frame_time = 1.0 / self.target_fps;
        // Check if we need to degrade quality
        if avg_frame_time > target_frame_time * 1.5 {
            self.degrade_count += 1;
            self.upgrade_count = 0;
            // Degrade after 5 consecutive bad frames
            if self.degrade_count >= 5 {
                if let Some(lower) = self.current_quality.lower() {
                    self.current_quality = lower;
                    self.degrade_count = 0;
                    self.log_quality_change("Degraded", lower);
                }
            }
        }
        // Check if we can upgrade quality
        else if avg_frame_time < target_frame_time * 0.7 {
            self.upgrade_count += 1;
            self.degrade_count = 0;
            // Upgrade after 10 consecutive good frames (more conservative)
            if self.upgrade_count >= 10 {
                if let Some(higher) = self.current_quality.higher() {
                    self.current_quality = higher;
                    self.upgrade_count = 0;
                    self.log_quality_change("Upgraded", higher);
                }
            }
        }
        // Reset counters if frame time is within acceptable range
        else {
            self.degrade_count = 0;
            self.upgrade_count = 0;
        }
        self.current_quality
    }
    /// Returns current quality level.
    pub fn current_quality(&self) -> QualityLevel {
        self.current_quality
    }
    /// Forces a specific quality level.
    pub fn set_quality(&mut self, quality: QualityLevel) {
        if self.current_quality != quality {
            self.current_quality = quality;
            self.degrade_count = 0;
            self.upgrade_count = 0;
            self.log_quality_change("Forced", quality);
        }
    }
    /// Returns current frames per second.
    pub fn current_fps(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let total_time: f32 = self.frame_times.iter().sum();
        if total_time > 0.0 {
            self.frame_times.len() as f32 / total_time
        } else {
            0.0
        }
    }
    /// Returns average frame time over the last second.
    pub fn average_frame_time(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let recent_frames = if self.frame_times.len() >= 60 {
            &self.frame_times[self.frame_times.len() - 60..]
        } else {
            &self.frame_times
        };
        recent_frames.iter().sum::<f32>() / recent_frames.len() as f32
    }
    /// Returns performance metrics.
    pub fn metrics(&self) -> AdaptiveMetrics {
        AdaptiveMetrics {
            current_quality: self.current_quality,
            current_fps: self.current_fps(),
            average_frame_time: self.average_frame_time(),
            target_fps: self.target_fps,
            frame_count: self.frame_times.len(),
            degrade_count: self.degrade_count,
            upgrade_count: self.upgrade_count,
        }
    }
    /// Logs quality change (in a real implementation, this would use a logging framework).
    fn log_quality_change(&self, action: &str, new_quality: QualityLevel) {
        let fps = self.current_fps();
        let avg_time = self.average_frame_time();
        println!(
            "{} quality to {:?} (FPS: {:.1}, Avg frame time: {:.3}ms)",
            action,
            new_quality,
            fps,
            avg_time * 1000.0
        );
    }
}
/// Performance metrics for adaptive rendering.
#[derive(Debug, Clone)]
pub struct AdaptiveMetrics {
    pub current_quality: QualityLevel,
    pub current_fps: f32,
    pub average_frame_time: f32,
    pub target_fps: f32,
    pub frame_count: usize,
    pub degrade_count: usize,
    pub upgrade_count: usize,
}
impl AdaptiveMetrics {
    /// Returns true if performance is meeting target.
    pub fn is_meeting_target(&self) -> bool {
        self.current_fps >= self.target_fps * 0.9
    }
    /// Returns performance score (0.0 to 1.0).
    pub fn performance_score(&self) -> f32 {
        let target_frame_time = 1.0 / self.target_fps;
        let ratio = target_frame_time / self.average_frame_time;
        // Clamp between 0.0 and 1.0
        ratio.clamp(0.0, 1.0)
    }
}
/// Quality-aware rendering context extension.
pub trait QualityAwareRender {
    /// Returns current quality level.
    fn quality_level(&self) -> QualityLevel;
    /// Sets quality level.
    fn set_quality_level(&mut self, quality: QualityLevel);
    /// Updates frame time and adjusts quality.
    fn update_frame_time(&mut self, frame_time: f32) -> QualityLevel;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_adaptive_renderer_initialization() {
        let renderer = AdaptiveRenderer::new(60.0);
        assert_eq!(renderer.current_quality(), QualityLevel::High);
        assert_eq!(renderer.target_fps, 60.0);
    }
    #[test]
    fn test_quality_degradation() {
        let mut renderer = AdaptiveRenderer::new(60.0);
        let target_frame_time = 1.0 / 60.0;
        // Simulate 10 consecutive slow frames
        for _ in 0..10 {
            let quality = renderer.update_frame_time(target_frame_time * 2.0); // 2x slower
            println!("Quality after slow frame: {:?}", quality);
        }
        // After 10 slow frames: High→Medium(5 frames)→Low(5 more frames)
        // Two full degradation steps
        assert_eq!(renderer.current_quality(), QualityLevel::Low);
    }
    #[test]
    fn test_quality_upgrade() {
        let mut renderer = AdaptiveRenderer::new(60.0);
        // Start at Medium quality
        renderer.set_quality(QualityLevel::Medium);
        let target_frame_time = 1.0 / 60.0;
        // Simulate 15 consecutive fast frames
        for _ in 0..15 {
            let quality = renderer.update_frame_time(target_frame_time * 0.5); // 2x faster
            println!("Quality after fast frame: {:?}", quality);
        }
        // Should upgrade to High after 10 fast frames
        assert_eq!(renderer.current_quality(), QualityLevel::High);
    }
    #[test]
    fn test_metrics_calculation() {
        let mut renderer = AdaptiveRenderer::new(60.0);
        // Add some frame times
        renderer.update_frame_time(0.016); // ~60 FPS
        renderer.update_frame_time(0.017);
        renderer.update_frame_time(0.015);
        let metrics = renderer.metrics();
        assert!(metrics.current_fps > 50.0 && metrics.current_fps < 70.0);
        assert!(metrics.average_frame_time > 0.015 && metrics.average_frame_time < 0.017);
        assert_eq!(metrics.target_fps, 60.0);
    }
}
