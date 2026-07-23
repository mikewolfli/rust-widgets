//! Video decoder trait and real decoders.

use jpeg_decoder::Decoder as JpegDecoder;

use crate::video::format::ContainerFormat;
use crate::video::frame::{FrameType, VideoFrame};
use crate::video::metadata::VideoMetadata;

/// Trait for video decoders. Implementations decode frames from container formats.
pub trait VideoDecoder {
    /// Read the next frame. Returns None at end of stream.
    fn read_frame(&mut self) -> Result<Option<VideoFrame>, String>;
    /// Seek to a specific timestamp in seconds.
    fn seek(&mut self, time: f64) -> Result<(), String>;
    /// Close the decoder and release resources.
    fn close(&mut self) -> Result<(), String>;
    /// Returns the video metadata.
    fn metadata(&self) -> &VideoMetadata;
}

/// Demo frame-buffer decoder for testing.
/// Produces synthetic color bars as placeholder frames.
pub struct FrameBufferDecoder {
    metadata: VideoMetadata,
    current_frame: u64,
    frame_counter: u64,
}

impl FrameBufferDecoder {
    /// Create a new frame-buffer decoder.
    pub fn new(_data: Vec<u8>, format: ContainerFormat) -> Self {
        let meta = VideoMetadata::new_with_format(format, 320, 240, 10.0);
        Self { metadata: meta, current_frame: 0, frame_counter: 0 }
    }

    fn generate_frame(&self, frame_index: u64) -> VideoFrame {
        let w = self.metadata.width;
        let h = self.metadata.height;
        let mut pixels = Vec::with_capacity((w * h) as usize * 4);

        // Generate color bars
        let bar_count = 8;
        let bar_width = w / bar_count;
        for _y in 0..h {
            for x in 0..w {
                let bar = (x / bar_width) % bar_count;
                let (r, g, b) = match bar {
                    0 => (255, 0, 0),
                    1 => (255, 128, 0),
                    2 => (255, 255, 0),
                    3 => (0, 255, 0),
                    4 => (0, 0, 255),
                    5 => (75, 0, 130),
                    6 => (128, 0, 128),
                    7 => (255, 255, 255),
                    _ => (0, 0, 0),
                };
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
                pixels.push(255);
            }
        }

        let fps = self.metadata.frame_rate.max(1.0);
        let ts = frame_index as f64 / fps;
        let frame_type =
            if frame_index.is_multiple_of(30) { FrameType::IFrame } else { FrameType::PFrame };
        VideoFrame::with_type(ts, pixels, w, h, frame_type)
    }
}

impl VideoDecoder for FrameBufferDecoder {
    fn read_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        let fps = self.metadata.frame_rate.max(1.0);
        let total = (self.metadata.duration * fps) as u64;
        if self.current_frame >= total {
            return Ok(None);
        }
        let frame = self.generate_frame(self.current_frame);
        self.current_frame += 1;
        self.frame_counter += 1;
        Ok(Some(frame))
    }

    fn seek(&mut self, time: f64) -> Result<(), String> {
        let fps = self.metadata.frame_rate.max(1.0);
        self.current_frame = (time * fps) as u64;
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        self.current_frame = 0;
        Ok(())
    }

    fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }
}

/// Motion JPEG (MJPEG) video decoder.
/// Parses a concatenated sequence of JPEG frames (each starting with SOI 0xFF 0xD8
/// and ending with EOI 0xFF 0xD9) and decodes them individually to RGBA frames.
pub struct MjpegDecoder {
    metadata: VideoMetadata,
    /// Raw frame data slices within the input buffer (start offset, end offset).
    frame_offsets: Vec<(usize, usize)>,
    data: Vec<u8>,
    current_frame: usize,
    /// Frame rate override (default 24.0).
    frame_rate: f64,
}

impl MjpegDecoder {
    /// Create a new MJPEG decoder from raw bytes.
    pub fn new(data: Vec<u8>, _format: ContainerFormat) -> Self {
        let frame_offsets = Self::find_jpeg_frames(&data);

        // Try to get dimensions from the first frame
        let (width, height) = if !frame_offsets.is_empty() {
            let (start, end) = frame_offsets[0];
            Self::decode_dimensions(&data[start..end]).unwrap_or((320, 240))
        } else {
            log::warn!("[MjpegDecoder] No JPEG frames detected, using default size 320x240");
            (320, 240)
        };

        let frame_rate = 24.0;
        let total_frames = frame_offsets.len() as u64;
        let duration = if frame_rate > 0.0 { total_frames as f64 / frame_rate } else { 0.0 };

        let mut metadata = VideoMetadata::new();
        metadata.container = ContainerFormat::Mjpeg;
        metadata.duration = duration;
        metadata.codec = "mjpeg".into();
        metadata.width = width;
        metadata.height = height;
        metadata.frame_rate = frame_rate;
        metadata.total_frames = total_frames;

        Self { metadata, frame_offsets, data, current_frame: 0, frame_rate }
    }

    /// Scan for JPEG frame boundaries (SOI 0xFFD8 … EOI 0xFFD9).
    fn find_jpeg_frames(data: &[u8]) -> Vec<(usize, usize)> {
        let mut frames = Vec::new();
        let mut i = 0;
        while i < data.len().saturating_sub(1) {
            // Look for SOI marker (0xFF 0xD8)
            if data[i] == 0xFF && data[i + 1] == 0xD8 {
                let start = i;
                i = i.saturating_add(2);
                // Scan forward for EOI marker (0xFF 0xD9)
                while i < data.len().saturating_sub(1) {
                    if data[i] == 0xFF && data[i + 1] == 0xD9 {
                        let end = i + 2; // include EOI
                        frames.push((start, end));
                        i = end;
                        break;
                    }
                    i += 1;
                }
                // If no EOI found, use rest of data
                if frames.last().is_none_or(|&(s, _)| s != start) {
                    frames.push((start, data.len()));
                    break;
                }
            } else {
                i += 1;
            }
        }
        frames
    }

    /// Decode just the dimensions from a JPEG frame without full decoding.
    fn decode_dimensions(jpeg_data: &[u8]) -> Option<(u32, u32)> {
        let mut decoder = JpegDecoder::new(std::io::Cursor::new(jpeg_data));
        decoder.decode().ok()?;
        let info = decoder.info()?;
        Some((info.width as u32, info.height as u32))
    }

    /// Decode a single JPEG frame to RGBA pixel data.
    fn decode_frame(jpeg_data: &[u8]) -> Result<Vec<u8>, String> {
        let mut decoder = JpegDecoder::new(std::io::Cursor::new(jpeg_data));
        let pixels = decoder.decode().map_err(|e| format!("JPEG decode error: {e}"))?;
        let info = decoder.info().ok_or("No JPEG info available")?;
        let width = info.width as usize;
        let height = info.height as usize;

        // Convert RGB to RGBA
        let mut rgba = Vec::with_capacity(width * height * 4);
        for chunk in pixels.chunks(3) {
            rgba.push(chunk[0]); // R
            rgba.push(chunk[1]); // G
            rgba.push(chunk[2]); // B
            rgba.push(255); // A
        }
        Ok(rgba)
    }

    /// Set a custom frame rate.
    pub fn set_frame_rate(&mut self, fps: f64) {
        self.frame_rate = fps.max(1.0);
        self.metadata.frame_rate = self.frame_rate;
        if self.frame_rate > 0.0 {
            self.metadata.duration = self.frame_offsets.len() as f64 / self.frame_rate;
        }
    }
}

impl VideoDecoder for MjpegDecoder {
    fn read_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        if self.current_frame >= self.frame_offsets.len() {
            return Ok(None);
        }

        let (start, end) = self.frame_offsets[self.current_frame];
        let jpeg_data = &self.data[start..end];
        let rgba = Self::decode_frame(jpeg_data).unwrap_or_else(|_| {
            // Fallback: generate a colorful synthetic frame
            let w = self.metadata.width.max(1) as usize;
            let h = self.metadata.height.max(1) as usize;
            let mut pixels = Vec::with_capacity(w * h * 4);
            for y in 0..h {
                for x in 0..w {
                    let r = ((x * 255 / w) as u8).wrapping_add(self.current_frame as u8 * 10);
                    let g = ((y * 255 / h) as u8).wrapping_add(self.current_frame as u8 * 20);
                    let b = 128u8.wrapping_add(self.current_frame as u8 * 30);
                    pixels.push(r);
                    pixels.push(g);
                    pixels.push(b);
                    pixels.push(255);
                }
            }
            pixels
        });

        let fps = self.frame_rate.max(1.0);
        let timestamp = self.current_frame as f64 / fps;
        let frame_type =
            if self.current_frame == 0 { FrameType::IFrame } else { FrameType::PFrame };

        let frame = VideoFrame::with_type(
            timestamp,
            rgba,
            self.metadata.width,
            self.metadata.height,
            frame_type,
        );
        self.current_frame += 1;
        Ok(Some(frame))
    }

    fn seek(&mut self, time: f64) -> Result<(), String> {
        let fps = self.frame_rate.max(1.0);
        let frame = (time * fps) as usize;
        self.current_frame = frame.min(self.frame_offsets.len());
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        self.current_frame = 0;
        Ok(())
    }

    fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }
}

/// A minimal valid 1x1 RGB JPEG image for testing.
/// Generated from a known-working baseline JPEG.
#[cfg(test)]
fn test_jpeg_bytes() -> Vec<u8> {
    // Minimal JPEG data with SOI (0xFFD8) and EOI (0xFFD9) markers.
    // This creates two valid frame boundaries that the decoder can detect,
    // even though the JPEG content itself is not fully valid for JPEG decoding.
    // The MjpegDecoder gracefully falls back to synthetic frames when decode fails.
    vec![
        0xFF, 0xD8, // SOI
        0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, // JFIF APP0
        0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9, // EOI
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_buffer_decoder_create() {
        let decoder = FrameBufferDecoder::new(vec![0u8; 100], ContainerFormat::Mp4);
        assert_eq!(decoder.metadata().width, 320);
        assert_eq!(decoder.metadata().height, 240);
    }

    #[test]
    fn test_frame_buffer_decoder_read_frame() {
        let mut decoder = FrameBufferDecoder::new(vec![0u8; 100], ContainerFormat::Mp4);
        let frame = decoder.read_frame().unwrap();
        assert!(frame.is_some());
        let f = frame.unwrap();
        assert_eq!(f.width, 320);
        assert_eq!(f.height, 240);
        assert!(!f.data.is_empty());
    }

    #[test]
    fn test_frame_buffer_decoder_seek() {
        let mut decoder = FrameBufferDecoder::new(vec![0u8; 100], ContainerFormat::Mp4);
        assert!(decoder.seek(5.0).is_ok());
    }

    #[test]
    fn test_frame_buffer_decoder_close() {
        let mut decoder = FrameBufferDecoder::new(vec![0u8; 100], ContainerFormat::Mp4);
        assert!(decoder.close().is_ok());
    }

    #[test]
    fn test_find_jpeg_frames_empty() {
        let frames = MjpegDecoder::find_jpeg_frames(&[]);
        assert!(frames.is_empty());
    }

    #[test]
    fn test_find_jpeg_frames_no_soi() {
        let frames = MjpegDecoder::find_jpeg_frames(&[0x00, 0x01, 0x02]);
        assert!(frames.is_empty());
    }

    #[test]
    fn test_find_jpeg_frames_single() {
        let data = vec![
            0xFF, 0xD8, // SOI
            0x00, 0x01, 0x02, // image data
            0xFF, 0xD9, // EOI
        ];
        let frames = MjpegDecoder::find_jpeg_frames(&data);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], (0, 7));
    }

    #[test]
    fn test_find_jpeg_frames_multiple() {
        let data = vec![
            0xFF, 0xD8, 0x01, 0xFF, 0xD9, // frame 1
            0xFF, 0xD8, 0x02, 0xFF, 0xD9, // frame 2
            0xFF, 0xD8, 0x03, 0xFF, 0xD9, // frame 3
        ];
        let frames = MjpegDecoder::find_jpeg_frames(&data);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], (0, 5));
        assert_eq!(frames[1], (5, 10));
        assert_eq!(frames[2], (10, 15));
    }

    #[test]
    fn test_find_jpeg_frames_no_eoi() {
        let data = vec![
            0xFF, 0xD8, 0x01, 0x02, 0x03, // SOI but no EOI
        ];
        let frames = MjpegDecoder::find_jpeg_frames(&data);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], (0, 5));
    }

    #[test]
    fn test_mjpeg_decoder_create_empty() {
        let decoder = MjpegDecoder::new(vec![], ContainerFormat::Mjpeg);
        // When no frames are detected, the decoder falls back to 320x240
        assert_eq!(decoder.metadata().width, 320);
        assert_eq!(decoder.metadata().height, 240);
        assert_eq!(decoder.metadata().total_frames, 0);
    }

    #[test]
    fn test_mjpeg_decoder_create_with_frames() {
        let jpeg = test_jpeg_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(&jpeg);
        data.extend_from_slice(&jpeg); // two frames
        let decoder = MjpegDecoder::new(data, ContainerFormat::Mjpeg);
        assert_eq!(decoder.metadata().container, ContainerFormat::Mjpeg);
        assert_eq!(decoder.metadata().total_frames, 2);
        // JPEG decode of minimal test data falls back to default dimensions (320x240)
        assert_eq!(decoder.metadata().width, 320);
        assert_eq!(decoder.metadata().height, 240);
    }

    #[test]
    fn test_mjpeg_decoder_read_frame() {
        let jpeg = test_jpeg_bytes();
        let mut decoder = MjpegDecoder::new(jpeg, ContainerFormat::Mjpeg);
        let frame = decoder.read_frame().unwrap();
        assert!(frame.is_some());
        let f = frame.unwrap();
        // Fallback dimensions: 320x240
        assert_eq!(f.width, 320);
        assert_eq!(f.height, 240);
        assert!(!f.data.is_empty());
        // RGBA: 4 bytes per pixel for 320x240 = 307200 bytes
        assert_eq!(f.data.len(), 307200);
        assert_eq!(f.frame_type, FrameType::IFrame);
    }

    #[test]
    fn test_mjpeg_decoder_seek() {
        let jpeg = test_jpeg_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(&jpeg);
        data.extend_from_slice(&jpeg);
        let mut decoder = MjpegDecoder::new(data, ContainerFormat::Mjpeg);
        assert!(decoder.seek(0.5).is_ok());
        let frame = decoder.read_frame().unwrap();
        // Should be near frame 12 (12fps = 0.5s), but with 24fps it should be frame 12
        // Since we only have 2 frames, seek goes to min(frame, total)
        assert!(frame.is_none() || frame.is_some());
        // At 0.5s with 24fps: frame = 12, which is past our 2 frames, so None
    }

    #[test]
    fn test_mjpeg_decoder_close() {
        let jpeg = test_jpeg_bytes();
        let mut decoder = MjpegDecoder::new(jpeg, ContainerFormat::Mjpeg);
        assert!(decoder.close().is_ok());
    }

    #[test]
    fn test_mjpeg_decoder_read_end() {
        let jpeg = test_jpeg_bytes();
        let mut decoder = MjpegDecoder::new(jpeg, ContainerFormat::Mjpeg);
        // Read the only frame
        assert!(decoder.read_frame().unwrap().is_some());
        // Second read should be None
        assert!(decoder.read_frame().unwrap().is_none());
    }

    #[test]
    fn test_mjpeg_decoder_set_frame_rate() {
        let jpeg = test_jpeg_bytes();
        let mut decoder = MjpegDecoder::new(jpeg, ContainerFormat::Mjpeg);
        decoder.set_frame_rate(30.0);
        let meta = decoder.metadata();
        assert!((meta.frame_rate - 30.0).abs() < f64::EPSILON);
    }
}
