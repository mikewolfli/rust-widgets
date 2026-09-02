use crate::signal::Signal;
#[cfg(not(feature = "video-codecs"))]
use crate::video::decoder::FrameBufferDecoder;
use crate::video::decoder::{MjpegDecoder, VideoDecoder};
use crate::video::format::{self, ContainerFormat};
use crate::video::frame::VideoFrame;
use crate::video::metadata::VideoMetadata;
use crate::video::player::PlaybackState;

#[cfg(feature = "video-codecs")]
use crate::video::ffmpeg_decoder::FfmpegDecoder;

/// Video player engine that wraps a decoder and exposes playback control.
pub struct VideoEngine {
    decoder: Box<dyn VideoDecoder + Send>,
    metadata: VideoMetadata,
    state: PlaybackState,
    current_time: f64,
    /// Emitted when a new frame is decoded.
    pub on_frame: Signal<VideoFrame>,
    /// Emitted when playback state changes.
    pub on_state_change: Signal<PlaybackState>,
}

impl VideoEngine {
    /// Open a video file from raw bytes. Detects container format automatically.
    ///
    /// When the `video-codecs` feature is enabled, non-MJPEG formats are
    /// decoded via FFmpeg (real decoding).  With the feature disabled, the
    /// synthetic `FrameBufferDecoder` fallback is used.
    pub fn open(data: Vec<u8>) -> Result<Self, String> {
        let format = format::detect_container_format(&data);
        let decoder: Box<dyn VideoDecoder + Send> = match format {
            ContainerFormat::Unknown => {
                return Err("Unknown video container format".into());
            }
            ContainerFormat::Mjpeg => Box::new(MjpegDecoder::new(data, format)),
            #[cfg(feature = "video-codecs")]
            _ => Box::new(FfmpegDecoder::new(data)?),
            #[cfg(not(feature = "video-codecs"))]
            _ => Box::new(FrameBufferDecoder::new(data, format)),
        };
        let metadata = decoder.metadata().clone();
        Ok(Self {
            decoder,
            metadata,
            state: PlaybackState::Stopped,
            current_time: 0.0,
            on_frame: Signal::new(),
            on_state_change: Signal::new(),
        })
    }

    /// Returns the video metadata.
    pub fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }

    /// Returns the current playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Returns the current playback time in seconds.
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// Start or resume playback.
    pub fn play(&mut self) {
        self.state = PlaybackState::Playing;
        self.on_state_change.emit(self.state);
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
        self.on_state_change.emit(self.state);
    }

    /// Stop playback and reset to beginning.
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.current_time = 0.0;
        self.on_state_change.emit(self.state);
    }

    /// Seek to a specific time in seconds.
    pub fn seek(&mut self, time: f64) -> Result<(), String> {
        let time = time.clamp(0.0, self.metadata.duration);
        self.decoder.seek(time)?;
        self.current_time = time;
        Ok(())
    }

    /// Advance one frame (step forward).
    pub fn step_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        let frame = self.decoder.read_frame()?;
        if let Some(ref f) = frame {
            self.current_time = f.timestamp;
            self.on_frame.emit(f.clone());
        }
        Ok(frame)
    }

    /// Decode the next available frame. Returns None at end of stream.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        if self.state != PlaybackState::Playing {
            return Ok(None);
        }
        let frame = self.decoder.read_frame()?;
        if let Some(ref f) = frame {
            self.current_time = f.timestamp;
            self.on_frame.emit(f.clone());
        } else {
            // End of stream
            self.state = PlaybackState::Stopped;
            self.on_state_change.emit(self.state);
        }
        Ok(frame)
    }

    /// Returns the duration of the video in seconds.
    pub fn duration(&self) -> f64 {
        self.metadata.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_engine_open() {
        let data = vec![0u8; 100]; // Not a real video
        let result = VideoEngine::open(data);
        assert!(result.is_err()); // Unknown format
    }

    #[test]
    fn test_playback_states() {
        let data = vec![0u8; 100];
        if let Ok(mut engine) = VideoEngine::open(data) {
            engine.play();
            assert_eq!(engine.state(), PlaybackState::Playing);
            engine.pause();
            assert_eq!(engine.state(), PlaybackState::Paused);
            engine.stop();
            assert_eq!(engine.state(), PlaybackState::Stopped);
        }
    }

    #[test]
    fn test_seek_bounds() {
        let data = vec![0u8; 100];
        if let Ok(mut engine) = VideoEngine::open(data) {
            assert_eq!(engine.current_time(), 0.0);
            let _ = engine.seek(5.0);
        }
    }
}
