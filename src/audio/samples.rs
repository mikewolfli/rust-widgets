//! Audio buffer — multi-channel sample storage and manipulation.

use crate::audio::format::SampleFormat;

/// Buffer of audio samples with metadata.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Sample rate in Hz (e.g., 44100).
    pub sample_rate: u32,
    /// Interleaved F32 samples: L0, R0, L1, R1, ...
    pub samples: Vec<f32>,
    /// Number of channels (1=mono, 2=stereo, etc.).
    pub channels: u8,
    /// Original sample format before conversion to F32.
    pub original_format: SampleFormat,
}

impl AudioBuffer {
    /// Create a new audio buffer.
    pub fn new(sample_rate: u32, samples: Vec<f32>, channels: u8) -> Self {
        Self { sample_rate, samples, channels: channels.max(1), original_format: SampleFormat::F32 }
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

    /// Apply fade-in to the first `duration` samples.
    pub fn fade_in(&mut self, duration_samples: usize) {
        let len = duration_samples.min(self.samples.len());
        for i in 0..len {
            let gain = i as f32 / duration_samples as f32;
            self.samples[i] *= gain;
        }
    }

    /// Apply fade-out to the last `duration` samples.
    pub fn fade_out(&mut self, duration_samples: usize) {
        let len = duration_samples.min(self.samples.len());
        let start = self.samples.len() - len;
        for i in 0..len {
            let gain = (len - i) as f32 / len as f32;
            self.samples[start + i] *= gain;
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
}
