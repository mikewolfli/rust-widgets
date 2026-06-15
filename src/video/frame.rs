//! Video frame representation.

use crate::image::format::{ImageData, ImageFormat, DecodedImage};

/// Type of video frame in the compression sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// Intra-coded frame (keyframe).
    IFrame,
    /// Predicted frame.
    PFrame,
    /// Bidirectionally predicted frame.
    BFrame,
    /// Unknown frame type.
    Unknown,
}

impl Default for FrameType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// A single video frame with timestamp and pixel data.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Timestamp in seconds.
    pub timestamp: f64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel data in RGBA8 format.
    pub data: Vec<u8>,
    /// Frame type.
    pub frame_type: FrameType,
}

impl VideoFrame {
    /// Create a new video frame.
    pub fn new(timestamp: f64, data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            timestamp,
            width,
            height,
            data,
            frame_type: FrameType::Unknown,
        }
    }

    /// Create a new video frame with frame type.
    pub fn with_type(timestamp: f64, data: Vec<u8>, width: u32, height: u32, frame_type: FrameType) -> Self {
        Self {
            timestamp,
            width,
            height,
            data,
            frame_type,
        }
    }

    /// Returns the number of pixels in this frame.
    pub fn pixel_count(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Convert to a DecodedImage for use with the image module.
    pub fn to_image(&self) -> DecodedImage {
        DecodedImage::new(
            ImageFormat::Rgba8,
            ImageData::Rgba8(self.data.clone()),
            self.width,
            self.height,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_frame_creation() {
        let frame = VideoFrame::new(1.5, vec![0u8; 160 * 120 * 4], 160, 120);
        assert!((frame.timestamp - 1.5).abs() < f64::EPSILON);
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 120);
        assert_eq!(frame.pixel_count(), 19200);
    }

    #[test]
    fn test_video_frame_to_image() {
        let frame = VideoFrame::new(0.0, vec![255u8; 16], 2, 2);
        let img = frame.to_image();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
    }

    #[test]
    fn test_video_frame_with_type() {
        let frame = VideoFrame::with_type(0.0, vec![], 1, 1, FrameType::IFrame);
        assert_eq!(frame.frame_type, FrameType::IFrame);
    }
}
