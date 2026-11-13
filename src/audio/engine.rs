// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Audio player engine for playback control.

#[cfg(feature = "audio-output")]
use crate::audio::AudioOutput;
use crate::audio::{decode, AudioBuffer};
use crate::signal::Signal;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;

/// Audio player engine for playback control.
pub struct AudioEngine {
    buffer: Option<AudioBuffer>,
    position: usize,
    is_playing: bool,
    volume: f32,
    /// A decode running on the background thread started by [`AudioEngine::load_async`].
    ///
    /// `None` when nothing is loading. The receiver is the completion channel: the worker sends
    /// exactly one result, and [`AudioEngine::poll_load`] / [`AudioEngine::wait_for_load`] installs
    /// it. See [`AudioEngine::load_async`] for why loading is a background operation (MEDIA-03).
    pending_load: Option<Receiver<Result<AudioBuffer, String>>>,
    /// The active device output, held for the full duration of device playback
    /// so the underlying cpal stream is not dropped the moment `play_to_device`
    /// returns.
    #[cfg(feature = "audio-output")]
    output: Option<AudioOutput>,
    /// Generation of the loaded buffer. Incremented in `install_decoded` so a
    /// prepared copy can never outlive the samples it was made from.
    #[cfg(feature = "audio-output")]
    buffer_revision: u64,
    /// `buffer` converted to one device layout, reused across `play_to_device`
    /// calls when the revision and layout both still match.
    #[cfg(feature = "audio-output")]
    prepared: Option<PreparedBufferCache>,
    /// Emitted when playback state changes.
    pub on_state_change: Signal<bool>,
}

/// A prepared copy of the loaded buffer in one device layout.
///
/// `prepare_buffer` resamples the whole track and copies every sample; for a
/// five-minute stereo source that is ≈100 MB of work per call. The engine holds
/// the result here and reuses it while the buffer revision and the device layout
/// both stay the same (W6-29).
#[cfg(feature = "audio-output")]
struct PreparedBufferCache {
    revision: u64,
    sample_rate: u32,
    channels: u8,
    samples: Arc<Vec<f32>>,
}

/// Returns `buffer` prepared for `(sample_rate, channels)`, reusing the cached
/// conversion when both the buffer revision and the layout match.
#[cfg(feature = "audio-output")]
fn prepared_samples_cached(
    cache: &mut Option<PreparedBufferCache>,
    revision: u64,
    buffer: &AudioBuffer,
    sample_rate: u32,
    channels: u8,
) -> Arc<Vec<f32>> {
    if let Some(entry) = cache.as_ref() {
        if entry.revision == revision
            && entry.sample_rate == sample_rate
            && entry.channels == channels
        {
            return Arc::clone(&entry.samples);
        }
    }
    let samples = AudioOutput::prepare_samples(buffer, sample_rate, channels);
    *cache = Some(PreparedBufferCache {
        revision,
        sample_rate,
        channels,
        samples: Arc::clone(&samples),
    });
    samples
}

impl AudioEngine {
    /// Create a new empty audio engine.
    pub fn new() -> Self {
        Self {
            buffer: None,
            position: 0,
            is_playing: false,
            volume: 1.0,
            pending_load: None,
            #[cfg(feature = "audio-output")]
            output: None,
            #[cfg(feature = "audio-output")]
            buffer_revision: 0,
            #[cfg(feature = "audio-output")]
            prepared: None,
            on_state_change: Signal::new(),
        }
    }

    /// Load audio from raw bytes, decoding on a background thread (MEDIA-03).
    ///
    /// # Why this is not inside [`AudioEngine::load`]
    ///
    /// `load` decodes the whole file into memory on the calling thread — a 5-minute stereo track
    /// is ~100 MB of PCM and hundreds of milliseconds of decode, which is a visible stall on the
    /// frame loop. This method starts the same decode on a worker and returns immediately; the
    /// caller either polls [`AudioEngine::poll_load`] once per frame or blocks once with
    /// [`AudioEngine::wait_for_load`]. `load` remains the synchronous shorthand and decodes
    /// directly on the calling thread; it is **not** implemented in terms of these two.
    ///
    /// If no thread can be spawned, the decode runs synchronously and the result is still
    /// delivered through the same channel, so the failure mode is "slower", never "nothing
    /// loads".
    pub fn load_async(&mut self, data: Vec<u8>) {
        // A new load supersedes any earlier one: dropping the old receiver makes the previous
        // worker's send fail, so it stops instead of installing a track the caller replaced.
        self.pending_load = None;

        // `Arc` rather than a `Vec` clone: the worker needs the bytes only for the length of the
        // decode, and a spawn failure must still be able to decode them here.
        let data = Arc::new(data);
        let worker_data = Arc::clone(&data);
        let (sender, receiver) = mpsc::channel();
        let worker_sender = sender.clone();
        let spawned =
            thread::Builder::new().name("rust-widgets-audio-decode".to_string()).spawn(move || {
                if worker_sender.send(decode(&worker_data)).is_err() {
                    // A superseding `load_async` drops the receiver on purpose. Visible at debug
                    // level so a missing track can be told apart from a decode failure.
                    log::debug!("[audio] decode result discarded: a newer load replaced this one");
                }
            });
        if spawned.is_err() && sender.send(decode(&data)).is_err() {
            log::debug!("[audio] synchronous decode result discarded: the load was replaced");
        }
        self.pending_load = Some(receiver);
    }

    /// Whether a background load started by [`AudioEngine::load_async`] is still running.
    pub fn is_loading(&self) -> bool {
        self.pending_load.is_some()
    }

    /// Installs a finished background load, if one is ready.
    ///
    /// Returns `None` while the decode is still running, and `Some(result)` exactly once — on the
    /// call that reports success the buffer is already installed and the previous device output
    /// released.
    pub fn poll_load(&mut self) -> Option<Result<(), String>> {
        let receiver = self.pending_load.as_ref()?;
        match receiver.try_recv() {
            Ok(Ok(buffer)) => {
                self.pending_load = None;
                self.install_decoded(buffer);
                Some(Ok(()))
            }
            Ok(Err(message)) => {
                self.pending_load = None;
                Some(Err(message))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.pending_load = None;
                Some(Err(
                    "the audio decode thread stopped before producing a result; the track could \
                     not be loaded"
                        .to_string(),
                ))
            }
        }
    }

    /// Blocks until the background load finishes and installs its buffer.
    pub fn wait_for_load(&mut self) -> Result<(), String> {
        let Some(receiver) = self.pending_load.take() else { return Ok(()) };
        match receiver.recv() {
            Ok(Ok(buffer)) => {
                self.install_decoded(buffer);
                Ok(())
            }
            Ok(Err(message)) => Err(message),
            Err(_) => Err(
                "the audio decode thread stopped before producing a result; the track could not \
                 be loaded"
                    .to_string(),
            ),
        }
    }

    /// Installs a decoded buffer, stopping any device playback of the old one first.
    fn install_decoded(&mut self, buffer: AudioBuffer) {
        // Release the previous device output before installing the new buffer: the old `output`
        // holds a live cpal stream that keeps feeding the speakers independently of this engine's
        // state.
        self.release_output();
        self.buffer = Some(buffer);
        self.position = 0;
        self.is_playing = false;
        #[cfg(feature = "audio-output")]
        {
            // A different buffer means different samples: bump the revision so a
            // cached conversion can never be reused for it.
            self.buffer_revision = self.buffer_revision.wrapping_add(1);
            self.prepared = None;
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
        self.install_decoded(buffer);
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
    ///
    /// A stream that already reached its end is replayed from the beginning: without the
    /// rewind the next [`Self::tick`] saw `advanced == 0` at the old end position and stopped
    /// again immediately, so the play button did nothing after the first play-through
    /// (W6-09, the audio side of T3-12).
    pub fn play(&mut self) {
        if let Some(buffer) = &self.buffer {
            if self.position >= buffer.samples.len() {
                self.position = 0;
            }
        }
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
    ///
    /// The prepared conversion and the device are both reused when they are still
    /// valid: pausing and resuming must not pay device setup or a full-track
    /// resample again (W6-29). A new [`AudioOutput`] is constructed only when no
    /// output is held; its failure is returned unchanged, so the existing error
    /// contract is preserved.
    #[cfg(feature = "audio-output")]
    pub fn play_to_device(&mut self) -> Result<(), String> {
        if self.buffer.is_none() {
            return Err("No audio loaded".to_string());
        }

        // A paused stream still holds its position and its samples: resuming it is
        // both the cheapest and the only behaviour that does not jump backwards.
        if let Some(output) = self.output.as_mut() {
            if output.is_paused() && output.has_stream() {
                output.resume();
                self.is_playing = true;
                self.on_state_change.emit(true);
                return Ok(());
            }
        }

        // Read the layout before taking a mutable borrow of `output`; the borrow
        // used by `and_then` must end here or the reuse call below would conflict.
        let existing_layout = self.output.as_ref().and_then(AudioOutput::prepared_layout);
        if let Some((sample_rate, channels)) = existing_layout {
            let revision = self.buffer_revision;
            let buffer = self.buffer.as_ref().ok_or("No audio loaded")?;
            let samples = prepared_samples_cached(
                &mut self.prepared,
                revision,
                buffer,
                sample_rate,
                channels,
            );
            let volume = self.volume;
            let output = self.output.as_mut().ok_or("No audio device")?;
            output.set_volume(volume);
            output.play_prepared(samples)?;
            self.is_playing = true;
            self.on_state_change.emit(true);
            return Ok(());
        }

        let output = AudioOutput::new()?;
        self.play_to_device_with(output)
    }

    /// Start device playback using an already-constructed output and hold it
    /// for the duration of playback.
    #[cfg(feature = "audio-output")]
    fn play_to_device_with(&mut self, mut output: AudioOutput) -> Result<(), String> {
        if self.buffer.is_none() {
            return Err("No audio loaded".to_string());
        }
        output.set_volume(self.volume);
        match output.prepared_layout() {
            Some((sample_rate, channels)) => {
                let revision = self.buffer_revision;
                let buffer = self.buffer.as_ref().ok_or("No audio loaded")?;
                let samples = prepared_samples_cached(
                    &mut self.prepared,
                    revision,
                    buffer,
                    sample_rate,
                    channels,
                );
                output.play_prepared(samples)?;
            }
            None => {
                // No usable layout (for example a test output with no device):
                // delegate to `play` so the explicit device/config error is the
                // same one this path has always reported.
                let buffer = self.buffer.as_ref().ok_or("No audio loaded")?;
                output.play(buffer)?;
            }
        }
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

    /// Serializes the tests that observe the process-wide prepare/construction
    /// counters.
    #[cfg(feature = "audio-output")]
    static DEVICE_COUNTER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// W6-29: an existing, layout-bearing output must be reused rather than a
    /// second device being constructed on every `play_to_device` call.
    #[test]
    #[cfg(feature = "audio-output")]
    fn play_to_device_reuses_the_existing_output() {
        let _serial = DEVICE_COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::audio::output::reset_construction_count();

        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(48_000, vec![0.0; 8], 1));
        engine.output = Some(AudioOutput::new_with_layout_for_test(48_000, 1));

        // The injected output has a layout but no device, so starting the stream
        // reports the explicit contract error; the point is that it was reused.
        let err = engine.play_to_device().unwrap_err();
        assert!(err.contains("No audio device"), "got: {err}");
        assert_eq!(
            crate::audio::output::construction_count(),
            0,
            "an existing output must be reused"
        );
        assert!(engine.output.is_some(), "the reused output stays held");
    }

    /// W6-29: a cache hit must not resample the track again, and a new buffer or
    /// a new device layout must invalidate the cached conversion.
    #[test]
    #[cfg(feature = "audio-output")]
    fn a_cached_prepared_buffer_is_not_repared_until_the_buffer_or_layout_changes() {
        let _serial = DEVICE_COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::audio::output::reset_prepare_samples_call_count();

        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(24_000, vec![0.0; 240], 1));

        let first = engine.play_to_device_with(AudioOutput::new_with_layout_for_test(48_000, 2));
        assert!(first.is_err(), "a layout-only test output cannot open a real stream");
        assert_eq!(
            crate::audio::output::prepare_samples_call_count(),
            1,
            "the first play prepares once"
        );

        let second = engine.play_to_device_with(AudioOutput::new_with_layout_for_test(48_000, 2));
        assert!(second.is_err());
        assert_eq!(
            crate::audio::output::prepare_samples_call_count(),
            1,
            "a cache hit must not resample the track again"
        );

        // A different device layout is a different cache key.
        let _ = engine.play_to_device_with(AudioOutput::new_with_layout_for_test(44_100, 1));
        assert_eq!(crate::audio::output::prepare_samples_call_count(), 2);
        // The same layout a second time is a hit again.
        let _ = engine.play_to_device_with(AudioOutput::new_with_layout_for_test(44_100, 1));
        assert_eq!(crate::audio::output::prepare_samples_call_count(), 2);

        // A new buffer has different samples, so the cache must be invalidated.
        engine.install_decoded(AudioBuffer::new(24_000, vec![0.5; 480], 1));
        let _ = engine.play_to_device_with(AudioOutput::new_with_layout_for_test(44_100, 1));
        assert_eq!(
            crate::audio::output::prepare_samples_call_count(),
            3,
            "loading a new buffer must invalidate the prepared copy"
        );
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

    /// W6-09: pressing play after the stream finished restarts it instead of stopping instantly.
    #[test]
    fn play_after_the_end_rewinds_to_the_start() {
        let mut engine = AudioEngine::new();
        engine.buffer = Some(AudioBuffer::new(44100, vec![0.0; 8], 1));
        engine.play();
        assert_eq!(engine.tick(usize::MAX), 8);
        assert!(!engine.is_playing());

        engine.play();
        assert_eq!(engine.position(), 0, "the read pointer must return to the first sample");
        assert!(engine.is_playing());
        assert_eq!(engine.tick(3), 3, "playback advances again after the replay");
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

    /// A minimal 16-bit mono WAV: 8 kHz, eight samples of a square wave.
    ///
    /// Built here rather than loaded from disk so the async-load tests have a decoder fixture the
    /// repository does not have to carry as a binary (MEDIA-03 asked for a testable load path).
    fn minimal_wav() -> Vec<u8> {
        let sample_rate: u32 = 8000;
        let samples: [i16; 8] = [0, 8000, 16000, 8000, 0, -8000, -16000, -8000];
        let data_len = (samples.len() * 2) as u32;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    /// `load_async` decodes on a worker and `poll_load` installs the buffer once.
    #[test]
    fn load_async_installs_a_decoded_wav() {
        let mut engine = AudioEngine::new();
        engine.load_async(minimal_wav());
        assert!(engine.is_loading(), "the decode starts in the background");

        let mut installed = None;
        // Bounded polling: a hung worker fails the test instead of hanging the suite.
        for _ in 0..500 {
            if let Some(result) = engine.poll_load() {
                installed = Some(result);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        installed.expect("the load must finish").expect("the WAV must decode");
        assert!(!engine.is_loading(), "a completed load clears the pending job");
        assert_eq!(engine.buffer().map(|buffer| buffer.samples.len()), Some(8));
        assert!(engine.duration_seconds() > 0.0);
        assert_eq!(engine.poll_load(), None, "the result is reported exactly once");
    }

    /// `wait_for_load` is the blocking form and reports a decode failure without installing
    /// anything.
    #[test]
    fn wait_for_load_reports_a_failure_and_keeps_the_old_track() {
        let mut engine = AudioEngine::new();
        engine.load(&minimal_wav()).expect("the fixture decodes");

        engine.load_async(vec![0u8; 16]);
        let error = engine.wait_for_load().expect_err("invalid bytes must fail to decode");
        assert!(error.contains("format") || error.contains("WAV"), "{error}");
        assert!(
            engine.buffer().is_some(),
            "a failed load must not clear the track that is still loaded"
        );
    }

    /// `load` remains the synchronous shorthand over the same install path.
    #[test]
    fn load_decodes_a_wav_synchronously() {
        let mut engine = AudioEngine::new();
        engine.load(&minimal_wav()).expect("the fixture decodes");
        assert_eq!(engine.buffer().map(|buffer| buffer.samples.len()), Some(8));
        assert!(!engine.is_loading());
    }
}
