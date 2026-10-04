// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Audio buffer — multi-channel sample storage and manipulation.

use crate::audio::format::SampleFormat;

/// Buffer of audio samples with metadata.
///
/// `channels` is private and guaranteed `>= 1` (see [`AudioBuffer::channels`]):
/// an interleaved buffer with zero channels is meaningless and would make every
/// frame/offset computation a division by zero.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Sample rate in Hz (e.g., 44100).
    pub sample_rate: u32,
    /// Interleaved F32 samples: L0, R0, L1, R1, ...
    pub samples: Vec<f32>,
    /// Number of channels (1=mono, 2=stereo, etc.). Always `>= 1`.
    channels: u8,
    /// Original sample format before conversion to F32.
    pub original_format: SampleFormat,
}

impl AudioBuffer {
    /// Create a new audio buffer.
    ///
    /// `channels` is clamped to at least 1 so the buffer is always well-formed.
    pub fn new(sample_rate: u32, samples: Vec<f32>, channels: u8) -> Self {
        Self { sample_rate, samples, channels: channels.max(1), original_format: SampleFormat::F32 }
    }

    /// Number of channels. Always `>= 1`.
    pub fn channels(&self) -> u8 {
        self.channels
    }

    /// Duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 || self.samples.is_empty() {
            return 0.0;
        }
        let frames = self.samples.len() / self.channels as usize;
        frames as f64 / self.sample_rate as f64
    }

    /// Number of frames (sample groups across all channels).
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    /// Returns true if the buffer contains no samples.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Get a single channel's samples (de-interleave).
    pub fn channel(&self, channel: usize) -> Vec<f32> {
        if channel >= self.channels as usize {
            return vec![];
        }
        let count = self.frames();
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            out.push(self.samples[i * self.channels as usize + channel]);
        }
        out
    }

    /// Mix down to mono by averaging all channels.
    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels == 1 {
            return self.samples.clone();
        }
        let count = self.frames();
        let mut mono = Vec::with_capacity(count);
        for i in 0..count {
            let start = i * self.channels as usize;
            let end = start + self.channels as usize;
            let sum: f32 = self.samples[start..end].iter().sum();
            mono.push(sum / self.channels as f32);
        }
        mono
    }

    /// Apply fade-in over the first `duration_frames` frames.
    ///
    /// Every channel of a frame receives the same envelope value, so a stereo
    /// or multichannel signal fades uniformly in time instead of having a
    /// different gain per interleaved sample.
    pub fn fade_in(&mut self, duration_frames: usize) {
        let channels = self.channels as usize;
        let total_frames = self.samples.len() / channels;
        let frames = duration_frames.min(total_frames);
        if frames == 0 {
            return;
        }
        for frame in 0..frames {
            let gain = frame as f32 / duration_frames as f32;
            let base = frame * channels;
            for ch in 0..channels {
                self.samples[base + ch] *= gain;
            }
        }
    }

    /// Apply fade-out over the last `duration_frames` frames.
    ///
    /// Every channel of a frame receives the same envelope value (see
    /// [`AudioBuffer::fade_in`]).
    pub fn fade_out(&mut self, duration_frames: usize) {
        let channels = self.channels as usize;
        let total_frames = self.samples.len() / channels;
        let frames = duration_frames.min(total_frames);
        if frames == 0 {
            return;
        }
        let start = total_frames - frames;
        for i in 0..frames {
            let gain = (frames - i) as f32 / frames as f32;
            let base = (start + i) * channels;
            for ch in 0..channels {
                self.samples[base + ch] *= gain;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_duration() {
        let buf = AudioBuffer::new(44100, vec![0.0f32; 44100], 1);
        assert!((buf.duration_seconds() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_buffer_empty() {
        let buf = AudioBuffer::new(44100, vec![], 2);
        assert!(buf.is_empty());
        assert_eq!(buf.duration_seconds(), 0.0);
    }

    #[test]
    fn test_audio_buffer_channel_deinterleave() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // L0,R0, L1,R1, L2,R2
        let buf = AudioBuffer::new(44100, samples, 2);
        let left = buf.channel(0);
        let right = buf.channel(1);
        assert_eq!(left, vec![1.0, 3.0, 5.0]);
        assert_eq!(right, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_audio_buffer_to_mono() {
        let buf = AudioBuffer::new(44100, vec![0.5, 0.3, 1.0, 1.0], 2);
        let mono = buf.to_mono();
        assert!((mono[0] - 0.4).abs() < 0.001);
        assert!((mono[1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_buffer_fade() {
        let mut buf = AudioBuffer::new(44100, vec![1.0; 100], 1);
        buf.fade_in(10);
        assert!(buf.samples[0] < 0.5);
        assert!(buf.samples[9] > 0.5);
        assert!((buf.samples[99] - 1.0).abs() < 0.01);
        buf.fade_out(10);
        assert!(buf.samples[99] < 0.5);
    }

    #[test]
    fn test_audio_buffer_frames() {
        let buf = AudioBuffer::new(44100, vec![0.0; 8], 2);
        assert_eq!(buf.frames(), 4);
    }

    #[test]
    fn fade_in_applies_the_same_gain_to_all_channels_of_a_frame() {
        // Stereo with two frames, fading in over four frames: frame gains are
        // [0.0, 0.25], applied equally to the left and right channel.
        let mut buf = AudioBuffer::new(44100, vec![1.0, 1.0, 1.0, 1.0], 2);
        buf.fade_in(4);
        assert_eq!(buf.samples, vec![0.0, 0.0, 0.25, 0.25]);
    }

    #[test]
    fn fade_out_applies_the_same_gain_to_all_channels_of_a_frame() {
        // Stereo with three frames, fading out over two frames: the last two
        // frames get gains [1.0, 0.5], equal across channels.
        let mut buf = AudioBuffer::new(44100, vec![1.0; 6], 2);
        buf.fade_out(2);
        assert_eq!(buf.samples, vec![1.0, 1.0, 1.0, 1.0, 0.5, 0.5]);
    }

    #[test]
    fn fade_handles_multichannel_and_ignores_incomplete_trailing_frame() {
        // Four channels, two complete frames plus one orphan trailing sample.
        let mut buf = AudioBuffer::new(44100, vec![1.0; 9], 4);
        buf.fade_in(2);
        // Frame 0 -> gain 0.0, frame 1 -> gain 0.5; the orphan sample is untouched.
        assert_eq!(&buf.samples[0..4], &[0.0, 0.0, 0.0, 0.0]);
        assert_eq!(&buf.samples[4..8], &[0.5, 0.5, 0.5, 0.5]);
        assert_eq!(buf.samples[8], 1.0);
    }

    #[test]
    fn zero_channels_are_clamped_and_never_divide_by_zero() {
        // Pins the defect: `channels` was `pub` and `new` clamped it, but a
        // struct literal (or deserialized buffer) could set 0, making
        // `frames()` / `duration_seconds()` / `channel()` / `to_mono()` panic
        // with a division by zero. The field is now private with an accessor
        // guaranteed `>= 1`, so the only way in is `new`, which clamps.
        let buf = AudioBuffer::new(44100, vec![0.0; 8], 0);
        assert_eq!(buf.channels(), 1);
        assert_eq!(buf.frames(), 8);
        assert!((buf.duration_seconds() - 8.0 / 44100.0).abs() < 1e-9);
        assert_eq!(buf.channel(0).len(), 8);
        assert_eq!(buf.to_mono().len(), 8);
    }
}
