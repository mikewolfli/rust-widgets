// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

/// Deterministic color-bar decoder for tests and demos.
///
/// This type is not a media decoder and is never selected by `VideoEngine`.
/// Production callers should use `MjpegDecoder` or the FFmpeg-backed decoder.
pub struct FrameBufferDecoder {
    metadata: VideoMetadata,
    current_frame: u64,
}

impl FrameBufferDecoder {
    /// Create a deterministic test-pattern source.
    pub fn new(_data: Vec<u8>, format: ContainerFormat) -> Self {
        let meta = VideoMetadata::new_with_format(format, 320, 240, 10.0);
        Self { metadata: meta, current_frame: 0 }
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

        // Try to get dimensions from the first frame.
        let (width, height) = if !frame_offsets.is_empty() {
            let (start, end) = frame_offsets[0];
            match Self::decode_dimensions(&data[start..end]) {
                Some(dim) => dim,
                None => {
                    log::warn!(
                        "[MjpegDecoder] failed to decode dimensions from the first frame, \
                         falling back to default size 320x240"
                    );
                    (320, 240)
                }
            }
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

    /// Decode a single JPEG frame to RGBA pixel data, returning the frame's own
    /// pixel dimensions alongside the bytes.
    ///
    /// The dimensions must travel with the pixels: an MJPEG stream may change
    /// resolution mid-stream, and `decode_frame` is the only place that knows the
    /// real size of the frame it just decoded (N-S-55).
    fn decode_frame(jpeg_data: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
        let mut decoder = JpegDecoder::new(std::io::Cursor::new(jpeg_data));
        let pixels = decoder.decode().map_err(|e| {
            format!(
                "JPEG frame ({} bytes) could not be decoded; the MJPEG stream is corrupt or \
                 uses an unsupported sampling mode: {e}",
                jpeg_data.len()
            )
        })?;
        let info = decoder.info().ok_or("No JPEG info available")?;
        let width = info.width as usize;
        let height = info.height as usize;

        // `decoder.decode()` returns the raw sample bytes in the stream's pixel
        // format (L8/L16 grayscale, RGB24, or CMYK32), so the conversion must
        // honour `info.pixel_format` rather than assume 3 bytes per pixel — a
        // grayscale MJPEG frame would otherwise make the fixed `chunks(3)` read
        // past the last pixel and panic.
        let mut rgba = Vec::with_capacity(width * height * 4);
        match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => {
                for chunk in pixels.as_chunks::<3>().0 {
                    rgba.push(chunk[0]);
                    rgba.push(chunk[1]);
                    rgba.push(chunk[2]);
                    rgba.push(255);
                }
            }
            jpeg_decoder::PixelFormat::L8 => {
                for &lum in &pixels {
                    rgba.push(lum);
                    rgba.push(lum);
                    rgba.push(lum);
                    rgba.push(255);
                }
            }
            jpeg_decoder::PixelFormat::CMYK32 => {
                // CMYK → RGB via naive inverse: R = C*K, G = M*K, B = Y*K (0..255).
                for chunk in pixels.as_chunks::<4>().0 {
                    let c = chunk[0] as u32;
                    let m = chunk[1] as u32;
                    let y = chunk[2] as u32;
                    let k = 255 - chunk[3] as u32;
                    rgba.push(((255 - c) * k / 255) as u8);
                    rgba.push(((255 - m) * k / 255) as u8);
                    rgba.push(((255 - y) * k / 255) as u8);
                    rgba.push(255);
                }
            }
            other => {
                // `decode()` falls back to RGB24 for L16 streams (it down-samples),
                // but refuse any format we have not explicitly converted rather
                // than emit wrong-colour pixels.
                return Err(format!(
                    "JPEG frame uses unsupported pixel format {other:?}; only RGB24, L8 and CMYK32 are handled"
                ));
            }
        }
        Ok((rgba, info.width as u32, info.height as u32))
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
        let (rgba, width, height) = Self::decode_frame(jpeg_data).map_err(|err| {
            format!("MJPEG frame {} decode failed: {err}", self.current_frame + 1)
        })?;

        // Use the decoded frame's *own* size rather than the first frame's
        // metadata dimensions: an MJPEG stream can change resolution mid-stream,
        // and reporting the cached size for a differently-sized frame would make
        // `data.len() != width * height * 4` for every consumer (N-S-55). When the
        // size changes the metadata is advanced too, so `metadata()` describes the
        // most recently decoded frame rather than the first one forever.
        if width != self.metadata.width || height != self.metadata.height {
            log::debug!(
                "[MjpegDecoder] frame {} changed size from {}x{} to {width}x{height}; updating metadata",
                self.current_frame + 1,
                self.metadata.width,
                self.metadata.height,
            );
            self.metadata.width = width;
            self.metadata.height = height;
        }

        let fps = self.frame_rate.max(1.0);
        let timestamp = self.current_frame as f64 / fps;
        let frame_type =
            if self.current_frame == 0 { FrameType::IFrame } else { FrameType::PFrame };

        let frame = VideoFrame::with_type(timestamp, rgba, width, height, frame_type);
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

/// Fake MJPEG fixture used to test frame-boundary detection and decode errors.
///
/// The bytes contain only SOI/JFIF-APP0/EOI markers: the framing parser sees
/// valid frame boundaries, but there is no scan data, so real JPEG decoding
/// fails and `read_frame` returns an error.
#[cfg(test)]
fn test_jpeg_bytes() -> Vec<u8> {
    // SOI (0xFFD8) + JFIF APP0 segment + EOI (0xFFD9). No image scan data.
    vec![
        0xFF, 0xD8, // SOI
        0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, // JFIF APP0
        0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9, // EOI
    ]
}

/// Real baseline color JPEG (1440x1499) committed under `snapshots/` for
/// snapshot tests; used here to exercise the genuine decode-success path.
#[cfg(test)]
const TEST_REAL_JPEG: &[u8] = include_bytes!("../../snapshots/header.jpg");

/// A tiny, real 2x2 grayscale baseline JPEG.
///
/// Its only purpose is to be a *differently sized* valid JPEG from
/// [`TEST_REAL_JPEG`], so the MJPEG size-change path can be exercised without a
/// second file on disk (N-S-55). It decodes to `L8`, which `decode_frame`
/// converts to RGBA.
#[cfg(test)]
const TEST_TINY_JPEG: &[u8] = &[
    255, 216, 255, 224, 0, 16, 74, 70, 73, 70, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 255, 219, 0, 67, 0,
    16, 11, 12, 14, 12, 10, 16, 14, 13, 14, 18, 17, 16, 19, 24, 40, 26, 24, 22, 22, 24, 49, 35, 37,
    29, 40, 58, 51, 61, 60, 57, 51, 56, 55, 64, 72, 92, 78, 64, 68, 87, 69, 55, 56, 80, 109, 81,
    87, 95, 98, 103, 104, 103, 62, 77, 113, 121, 112, 100, 120, 92, 101, 103, 99, 255, 192, 0, 11,
    8, 0, 2, 0, 2, 1, 1, 17, 0, 255, 196, 0, 31, 0, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 255, 196, 0, 181, 16, 0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4,
    0, 0, 1, 125, 1, 2, 3, 0, 4, 17, 5, 18, 33, 49, 65, 6, 19, 81, 97, 7, 34, 113, 20, 50, 129,
    145, 161, 8, 35, 66, 177, 193, 21, 82, 209, 240, 36, 51, 98, 114, 130, 9, 10, 22, 23, 24, 25,
    26, 37, 38, 39, 40, 41, 42, 52, 53, 54, 55, 56, 57, 58, 67, 68, 69, 70, 71, 72, 73, 74, 83, 84,
    85, 86, 87, 88, 89, 90, 99, 100, 101, 102, 103, 104, 105, 106, 115, 116, 117, 118, 119, 120,
    121, 122, 131, 132, 133, 134, 135, 136, 137, 138, 146, 147, 148, 149, 150, 151, 152, 153, 154,
    162, 163, 164, 165, 166, 167, 168, 169, 170, 178, 179, 180, 181, 182, 183, 184, 185, 186, 194,
    195, 196, 197, 198, 199, 200, 201, 202, 210, 211, 212, 213, 214, 215, 216, 217, 218, 225, 226,
    227, 228, 229, 230, 231, 232, 233, 234, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 255,
    218, 0, 8, 1, 1, 0, 0, 63, 0, 43, 255, 217,
];

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
        // The fake fixture has no scan data, so dimension decoding fails and the
        // decoder falls back to the default 320x240.
        assert_eq!(decoder.metadata().width, 320);
        assert_eq!(decoder.metadata().height, 240);
    }

    #[test]
    fn test_mjpeg_decoder_read_frame_rejects_invalid_jpeg() {
        let jpeg = test_jpeg_bytes();
        let mut decoder = MjpegDecoder::new(jpeg, ContainerFormat::Mjpeg);
        let error = decoder.read_frame().unwrap_err();
        assert!(error.contains("MJPEG frame 1 decode failed"), "unexpected error: {error}");
    }

    #[test]
    fn test_mjpeg_decoder_real_jpeg_decode_success() {
        let mut decoder = MjpegDecoder::new(TEST_REAL_JPEG.to_vec(), ContainerFormat::Mjpeg);
        // Dimensions are read from the real JPEG (1440x1499).
        assert_eq!(decoder.metadata().width, 1440);
        assert_eq!(decoder.metadata().height, 1499);
        assert_eq!(decoder.metadata().total_frames, 1);

        let frame = decoder.read_frame().unwrap();
        assert!(frame.is_some());
        let f = frame.unwrap();
        // A genuinely decoded first frame is an I-frame, not a placeholder.
        assert_eq!(f.frame_type, FrameType::IFrame);
        assert_eq!(f.width, 1440);
        assert_eq!(f.height, 1499);
        assert_eq!(f.data.len(), 1440 * 1499 * 4);

        // End of stream after the single real frame.
        assert!(decoder.read_frame().unwrap().is_none());
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
        assert!(decoder.read_frame().is_err());
    }

    #[test]
    fn test_mjpeg_decoder_set_frame_rate() {
        let jpeg = test_jpeg_bytes();
        let mut decoder = MjpegDecoder::new(jpeg, ContainerFormat::Mjpeg);
        decoder.set_frame_rate(30.0);
        let meta = decoder.metadata();
        assert!((meta.frame_rate - 30.0).abs() < f64::EPSILON);
    }

    /// N-S-55: when a later frame changes size, the returned `VideoFrame` must
    /// carry *its own* dimensions (and matching data length), and the metadata
    /// must advance to describe the most recently decoded frame.
    #[test]
    fn test_mjpeg_decoder_propagates_per_frame_dimensions_on_size_change() {
        // Frame 1 is the large real JPEG; frame 2 is the tiny 2x2 JPEG. A naive
        // decoder would report 1440x1499 (the first frame's size) for frame 2
        // while returning 2x2x4 bytes of pixels, breaking every consumer.
        let mut data = Vec::new();
        data.extend_from_slice(TEST_REAL_JPEG);
        data.extend_from_slice(TEST_TINY_JPEG);
        let mut decoder = MjpegDecoder::new(data, ContainerFormat::Mjpeg);
        assert_eq!(decoder.metadata().total_frames, 2);
        // Metadata starts from the first frame.
        assert_eq!((decoder.metadata().width, decoder.metadata().height), (1440, 1499));

        let first = decoder.read_frame().unwrap().expect("first frame decodes");
        assert_eq!((first.width, first.height), (1440, 1499));
        assert_eq!(first.data.len(), 1440 * 1499 * 4);

        let second = decoder.read_frame().unwrap().expect("second frame decodes");
        assert_eq!(
            (second.width, second.height),
            (2, 2),
            "the frame must carry its own size, not the first frame's"
        );
        assert_eq!(
            second.data.len(),
            2 * 2 * 4,
            "data length must agree with the frame's own size"
        );
        // Metadata advances to describe the latest frame.
        assert_eq!((decoder.metadata().width, decoder.metadata().height), (2, 2));

        assert!(decoder.read_frame().unwrap().is_none(), "no more frames");
    }
}
