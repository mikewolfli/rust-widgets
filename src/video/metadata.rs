//! Video metadata — codec, resolution, frame rate, bitrate, duration.

use crate::video::format::ContainerFormat;

/// Metadata describing a video stream.
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    /// Container format.
    pub container: ContainerFormat,
    /// Duration in seconds.
    pub duration: f64,
    /// Video codec name (e.g., "h264", "vp9").
    pub codec: String,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frame rate in frames per second.
    pub frame_rate: f64,
    /// Bitrate in bits per second.
    pub bitrate: u64,
    /// Whether the video has an audio track.
    pub has_audio: bool,
    /// Audio codec name if audio is present.
    pub audio_codec: String,
    /// Total number of frames (estimated).
    pub total_frames: u64,
}

impl VideoMetadata {
    /// Create default/empty metadata.
    pub fn new() -> Self {
        Self {
            container: ContainerFormat::Unknown,
            duration: 0.0,
            codec: String::new(),
            width: 0,
            height: 0,
            frame_rate: 0.0,
            bitrate: 0,
            has_audio: false,
            audio_codec: String::new(),
            total_frames: 0,
        }
    }

    /// Create metadata for a specific container/dimensions.
    pub fn new_with_format(
        container: ContainerFormat,
        width: u32,
        height: u32,
        duration: f64,
    ) -> Self {
        Self {
            container,
            duration,
            codec: String::new(),
            width,
            height,
            frame_rate: 0.0,
            bitrate: 0,
            has_audio: false,
            audio_codec: String::new(),
            total_frames: (duration * 30.0) as u64, // Estimate at 30fps
        }
    }

    /// Aspect ratio as a float (width/height).
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Returns true if the metadata has valid dimensions.
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0 && self.duration > 0.0
    }
}

impl Default for VideoMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_default() {
        let meta = VideoMetadata::new();
        assert_eq!(meta.width, 0);
        assert_eq!(meta.height, 0);
        assert!(!meta.is_valid());
    }

    #[test]
    fn test_metadata_aspect_ratio() {
        let meta = VideoMetadata::new_with_format(ContainerFormat::Mp4, 1920, 1080, 120.0);
        assert_eq!(meta.width, 1920);
        assert_eq!(meta.height, 1080);
        assert!((meta.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);
        assert!(meta.is_valid());
    }

    #[test]
    fn test_metadata_zero_height_ratio() {
        let meta = VideoMetadata::new_with_format(ContainerFormat::Mp4, 1920, 0, 0.0);
        assert!((meta.aspect_ratio() - 1.0).abs() < f32::EPSILON);
    }
}
