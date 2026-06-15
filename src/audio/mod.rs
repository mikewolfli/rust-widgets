//! Audio module — format detection, decoding, encoding, sample processing, and normalization.

pub mod decoder;
pub mod encoder;
pub mod format;
pub mod normalize;
pub mod resample;
pub mod samples;

#[cfg(feature = "video-codecs")]
pub mod ffmpeg_encoder;

#[cfg(feature = "audio-output")]
pub mod output;

pub use decoder::{decode, detect_audio_format};
pub use encoder::encode;
pub use format::AudioFormat;
pub use normalize::normalize;
pub use resample::resample;
pub use samples::AudioBuffer;

#[cfg(feature = "video-codecs")]
pub use ffmpeg_encoder::ffmpeg_encode;

#[cfg(feature = "audio-output")]
pub use output::AudioOutput;

use crate::signal::Signal;

/// Audio player engine for playback control.
pub struct AudioEngine {
    buffer: Option<AudioBuffer>,
    position: usize,
    is_playing: bool,
    volume: f32,
    /// Emitted when playback state changes.
    pub on_state_change: Signal<bool>,
}

impl AudioEngine {
    /// Create a new empty audio engine.
    pub fn new() -> Self {
        Self {
            buffer: None,
            position: 0,
            is_playing: false,
            volume: 1.0,
            on_state_change: Signal::new(),
        }
    }

    /// Load audio from raw bytes. Detects format automatically.
    pub fn load(&mut self, data: &[u8]) -> Result<(), String> {
        let buffer = decode(data)?;
        self.buffer = Some(buffer);
        self.position = 0;
        self.is_playing = false;
        Ok(())
    }

    /// Start or resume playback.
    pub fn play(&mut self) {
        self.is_playing = true;
        self.on_state_change.emit(true);
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.is_playing = false;
        self.on_state_change.emit(false);
    }

    /// Stop and reset to beginning.
    pub fn stop(&mut self) {
        self.is_playing = false;
        self.position = 0;
        self.on_state_change.emit(false);
    }

    /// Returns true if currently playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Set volume (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Get current volume.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Returns the current playback position in samples.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns the total duration in seconds, or 0 if no audio loaded.
    pub fn duration_seconds(&self) -> f64 {
        self.buffer.as_ref().map(|b| b.duration_seconds()).unwrap_or(0.0)
    }

    /// Returns a reference to the audio buffer.
    pub fn buffer(&self) -> Option<&AudioBuffer> {
        self.buffer.as_ref()
    }

    /// Advance playback by `samples` samples. Returns the number of samples actually advanced.
    pub fn tick(&mut self, samples: usize) -> usize {
        if !self.is_playing {
            return 0;
        }
        let Some(ref buffer) = self.buffer else { return 0 };
        let total = buffer.samples.len();
        let new_pos = (self.position + samples).min(total);
        let advanced = new_pos - self.position;
        self.position = new_pos;
        if self.position >= total {
            self.is_playing = false;
            self.on_state_change.emit(false);
        }
        advanced
    }

    /// Play the loaded audio through the system's default audio output device.
    #[cfg(feature = "audio-output")]
    pub fn play_to_device(&mut self) -> Result<(), String> {
        let buffer = self.buffer.as_ref().ok_or("No audio loaded")?;
        let mut output = AudioOutput::new()?;
        output.play(buffer)?;
        self.is_playing = true;
        self.on_state_change.emit(true);
        // Keep the output alive for playback duration
        // In a real implementation, this would spawn a thread
        Ok(())
    }

    /// Get interleaved samples for the current playback window, scaled by volume.
    pub fn current_samples(&self, count: usize) -> Vec<f32> {
        let Some(ref buffer) = self.buffer else { return vec![] };
        let end = (self.position + count).min(buffer.samples.len());
        if self.position >= end {
            return vec![];
        }
        buffer.samples[self.position..end].iter().map(|s| s * self.volume).collect()
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_engine_empty() {
        let engine = AudioEngine::new();
        assert!(!engine.is_playing());
        assert!((engine.volume() - 1.0).abs() < f32::EPSILON);
        assert_eq!(engine.duration_seconds(), 0.0);
    }

    #[test]
    fn test_audio_engine_play_pause() {
        let mut engine = AudioEngine::new();
        engine.play();
        assert!(engine.is_playing());
        engine.pause();
        assert!(!engine.is_playing());
    }

    #[test]
    fn test_audio_engine_volume() {
        let mut engine = AudioEngine::new();
        engine.set_volume(0.5);
        assert!((engine.volume() - 0.5).abs() < f32::EPSILON);
        engine.set_volume(2.0);
        assert!((engine.volume() - 1.0).abs() < f32::EPSILON); // Clamped
    }

    #[test]
    fn test_audio_engine_tick() {
        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(44100, vec![0.0f32; 44100], 1));
        engine.play();
        let adv = engine.tick(1000);
        assert_eq!(adv, 1000);
        assert_eq!(engine.position(), 1000);
    }

    #[test]
    fn test_audio_engine_stop() {
        let mut engine = AudioEngine::new();
        engine.play();
        engine.stop();
        assert!(!engine.is_playing());
        assert_eq!(engine.position(), 0);
    }
}
