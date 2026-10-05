// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Audio player engine for playback control.

#[cfg(feature = "audio-output")]
use crate::audio::AudioOutput;
use crate::audio::{decode, AudioBuffer};
use crate::signal::Signal;

/// Audio player engine for playback control.
pub struct AudioEngine {
    buffer: Option<AudioBuffer>,
    position: usize,
    is_playing: bool,
    volume: f32,
    /// The active device output, held for the full duration of device playback
    /// so the underlying cpal stream is not dropped the moment `play_to_device`
    /// returns.
    #[cfg(feature = "audio-output")]
    output: Option<AudioOutput>,
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
            #[cfg(feature = "audio-output")]
            output: None,
            on_state_change: Signal::new(),
        }
    }

    /// Load audio from raw bytes. Detects format automatically.
    ///
    /// Any device playback already in progress is stopped and released first, so
    /// loading a new track cannot leave the old one audible: the old `output`
    /// holds a live cpal stream that keeps feeding the speakers independently of
    /// this engine's state, so replacing the buffer without dropping it would let
    /// stale audio keep playing while `is_playing()` reports `false` (N-S-48).
    pub fn load(&mut self, data: &[u8]) -> Result<(), String> {
        let buffer = decode(data)?;
        // Release the previous device output before installing the new buffer.
        // Deferred until after `decode` succeeds so a failed load does not stop
        // playback of the track that is still loaded.
        self.release_output();
        self.buffer = Some(buffer);
        self.position = 0;
        self.is_playing = false;
        Ok(())
    }

    /// Stops and drops any held device output.
    ///
    /// Centralises the "no stale audio" invariant: `stop`, `load`, and a device
    /// failure all need the old stream stopped *and* released, and doing it in one
    /// place keeps them from drifting apart.
    #[cfg(feature = "audio-output")]
    fn release_output(&mut self) {
        if let Some(output) = &mut self.output {
            output.stop();
        }
        self.output = None;
    }

    /// No-op when the `audio-output` feature is disabled (there is never a device
    /// output to release).
    #[cfg(not(feature = "audio-output"))]
    fn release_output(&mut self) {}

    /// Start or resume playback.
    pub fn play(&mut self) {
        self.is_playing = true;
        #[cfg(feature = "audio-output")]
        if let Some(output) = &mut self.output {
            output.resume();
        }
        self.on_state_change.emit(true);
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.is_playing = false;
        #[cfg(feature = "audio-output")]
        if let Some(output) = &mut self.output {
            output.pause();
        }
        self.on_state_change.emit(false);
    }

    /// Stop and reset to beginning.
    pub fn stop(&mut self) {
        self.is_playing = false;
        self.position = 0;
        self.release_output();
        self.on_state_change.emit(false);
    }

    /// Returns true if currently playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Set volume (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        #[cfg(feature = "audio-output")]
        if let Some(output) = &mut self.output {
            output.set_volume(self.volume);
        }
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
    ///
    /// The advance is computed with saturating arithmetic and then clamped to
    /// the buffer length, so a caller passing a very large `samples` (up to
    /// `usize::MAX`) reaches the end instead of overflowing `position + samples`
    /// and wrapping to a small value (N-S-49).
    pub fn tick(&mut self, samples: usize) -> usize {
        if !self.is_playing {
            return 0;
        }
        let Some(ref buffer) = self.buffer else { return 0 };
        let total = buffer.samples.len();
        // `saturating_add` bounds the sum at `usize::MAX`, so the subsequent
        // `min` genuinely clamps rather than seeing a wrapped small value.
        let new_pos = self.position.saturating_add(samples).min(total);
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
        let output = AudioOutput::new()?;
        self.play_to_device_with(output)
    }

    /// Start device playback using an already-constructed output and hold it
    /// for the duration of playback.
    #[cfg(feature = "audio-output")]
    fn play_to_device_with(&mut self, mut output: AudioOutput) -> Result<(), String> {
        let buffer = self.buffer.as_ref().ok_or("No audio loaded")?;
        output.set_volume(self.volume);
        output.play(buffer)?;
        self.output = Some(output);
        self.is_playing = true;
        self.on_state_change.emit(true);
        Ok(())
    }

    /// Reconcile engine state with the device output's end-of-buffer flag.
    ///
    /// The cpal audio callback runs on a separate thread and cannot hold a
    /// handle back to this engine, so it reports exhaustion through a shared
    /// atomic flag. Call this periodically to observe that notification and
    /// flip the engine's playing state when playback finishes.
    #[cfg(feature = "audio-output")]
    pub fn update(&mut self) {
        let Some(output) = &self.output else { return };
        if output.is_finished() {
            self.output = None;
            self.is_playing = false;
            self.on_state_change.emit(false);
        }
    }

    /// Get interleaved samples for the current playback window, scaled by volume.
    ///
    /// The window is `[position, position + count)` clamped to the buffer, and the
    /// upper bound uses saturating arithmetic so a huge `count` cannot wrap the
    /// sum into a small value and return the wrong slice (N-S-49).
    pub fn current_samples(&self, count: usize) -> Vec<f32> {
        let Some(ref buffer) = self.buffer else { return vec![] };
        let total = buffer.samples.len();
        let end = self.position.saturating_add(count).min(total);
        if self.position >= end {
            return vec![];
        }
        buffer.samples[self.position..end].iter().map(|s| s * self.volume).collect()
    }
}

crate::impl_default_via_new!(AudioEngine);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_engine_new() {
        let engine = AudioEngine::new();
        assert!(!engine.is_playing());
        assert_eq!(engine.volume(), 1.0);
    }
    #[test]
    fn audio_engine_play_pause() {
        let mut engine = AudioEngine::new();
        engine.play();
        assert!(engine.is_playing());
        engine.pause();
        assert!(!engine.is_playing());
    }
    #[test]
    fn audio_engine_stop_resets_position() {
        let mut engine = AudioEngine::new();
        engine.play();
        engine.stop();
        assert!(!engine.is_playing());
        assert_eq!(engine.position(), 0);
    }
    #[test]
    fn audio_engine_default_volume() {
        let engine = AudioEngine::default();
        assert_eq!(engine.volume(), 1.0);
    }
    #[test]
    fn audio_engine_set_volume() {
        let mut engine = AudioEngine::new();
        engine.set_volume(0.5);
        assert!((engine.volume() - 0.5).abs() < 1e-6);
    }
    #[test]
    fn audio_engine_set_volume_clamps() {
        let mut engine = AudioEngine::new();
        engine.set_volume(1.5);
        assert!((engine.volume() - 1.0).abs() < 1e-6);
        engine.set_volume(-0.5);
        assert!((engine.volume() - 0.0).abs() < 1e-6);
    }
    #[test]
    fn audio_engine_tick_without_buffer_returns_zero() {
        let mut engine = AudioEngine::new();
        engine.play();
        assert_eq!(engine.tick(100), 0);
    }

    #[cfg(feature = "audio-output")]
    fn engine_with_output() -> AudioEngine {
        let mut engine = AudioEngine::new();
        engine.output = Some(AudioOutput::new_without_device());
        engine
    }

    #[test]
    #[cfg(feature = "audio-output")]
    fn play_to_device_without_loaded_buffer_fails() {
        let mut engine = AudioEngine::new();
        let err = engine.play_to_device().unwrap_err();
        assert!(err.contains("No audio loaded"), "got: {err}");
    }

    #[test]
    #[cfg(feature = "audio-output")]
    fn play_to_device_without_device_fails_and_holds_no_output() {
        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(44100, vec![0.0; 4], 1));
        let err = engine.play_to_device_with(AudioOutput::new_without_device()).unwrap_err();
        assert!(err.contains("No audio device"), "got: {err}");
        assert!(engine.output.is_none());
        assert!(!engine.is_playing());
    }

    #[test]
    #[cfg(feature = "audio-output")]
    fn pause_forwards_to_device_output() {
        let mut engine = engine_with_output();
        engine.play();
        engine.pause();
        assert!(!engine.is_playing());
        assert!(engine.output.as_ref().unwrap().is_paused());
    }

    #[test]
    #[cfg(feature = "audio-output")]
    fn stop_drops_device_output() {
        let mut engine = engine_with_output();
        engine.play();
        engine.stop();
        assert!(engine.output.is_none());
        assert!(!engine.is_playing());
    }

    #[test]
    #[cfg(feature = "audio-output")]
    fn set_volume_forwards_to_device_output() {
        let mut engine = engine_with_output();
        engine.set_volume(0.25);
        assert!((engine.output.as_ref().unwrap().volume() - 0.25).abs() < 1e-6);
    }

    #[test]
    #[cfg(feature = "audio-output")]
    fn update_notifies_when_output_finished() {
        let mut engine = engine_with_output();
        engine.play();
        engine.output.as_ref().unwrap().mark_finished_for_test();
        engine.update();
        assert!(!engine.is_playing());
        assert!(engine.output.is_none());
    }

    /// N-S-48: loading a new track must stop and drop any held device output so
    /// the previous stream cannot keep playing after the engine reports the new
    /// buffer as not-playing.
    #[test]
    #[cfg(feature = "audio-output")]
    fn load_releases_held_device_output() {
        let mut engine = engine_with_output();
        engine.play();
        assert!(engine.output.is_some(), "precondition: an output is held");
        assert!(engine.is_playing());

        // A minimal valid WAV (PCM I16, mono, 44.1 kHz) so `load` reaches the
        // point where it releases the old output.
        let wav = minimal_pcm16_wav(&[0i16, 1000, -1000]);
        engine.load(&wav).expect("valid WAV loads");

        assert!(engine.output.is_none(), "load must drop the stale device output");
        assert!(!engine.is_playing());
        assert_eq!(engine.position(), 0);
    }

    /// A failed `load` must not tear down the output that is still playing the
    /// current track — the release happens only after the new data decodes.
    #[test]
    #[cfg(feature = "audio-output")]
    fn failed_load_keeps_existing_output() {
        let mut engine = engine_with_output();
        engine.play();
        assert!(engine.load(b"this is not audio").is_err());
        assert!(engine.output.is_some(), "a failed load must leave the old output alone");
    }

    /// Build a minimal valid mono 16-bit PCM WAV around `samples`.
    #[cfg(feature = "audio-output")]
    fn minimal_pcm16_wav(samples: &[i16]) -> Vec<u8> {
        let data_size = (samples.len() * 2) as u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&44100u32.to_le_bytes());
        wav.extend_from_slice(&(44100u32 * 2).to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for s in samples {
            wav.extend_from_slice(&s.to_le_bytes());
        }
        wav
    }

    /// N-S-49: `tick` and `current_samples` must not overflow on `position +
    /// samples`; a huge count is clamped to the buffer instead of wrapping.
    #[test]
    fn tick_does_not_overflow_with_huge_sample_counts() {
        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(44100, vec![0.0; 8], 1));
        engine.play();
        // `position + samples` would wrap in the old arithmetic; it must clamp
        // at the buffer length and advance only the 8 available samples.
        assert_eq!(engine.tick(usize::MAX), 8);
        assert_eq!(engine.position(), 8);
        assert!(!engine.is_playing(), "reaching the end stops playback");
    }

    /// A large count while already partway through must still clamp correctly
    /// (the old code wrapped to a small `new_pos` and reported a bogus advance).
    #[test]
    fn tick_large_count_from_mid_position_is_clamped() {
        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(44100, vec![0.0; 10], 1));
        engine.play();
        assert_eq!(engine.tick(4), 4);
        assert_eq!(engine.tick(usize::MAX), 6);
        assert_eq!(engine.position(), 10);
    }

    /// N-S-49: `current_samples` with a huge count returns the remainder of the
    /// buffer rather than a wrapped, wrong-length slice.
    #[test]
    fn current_samples_does_not_overflow_with_huge_count() {
        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(44100, vec![1.0; 5], 1));
        engine.position = 3;
        let samples = engine.current_samples(usize::MAX);
        assert_eq!(samples.len(), 2, "only the two remaining samples are in the window");
        assert_eq!(samples, vec![1.0, 1.0]);
    }
}
