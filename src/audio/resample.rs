// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Audio resampling — sample rate conversion.

use crate::audio::samples::AudioBuffer;

/// Resample audio buffer to a new sample rate using linear interpolation.
pub fn resample(buffer: &AudioBuffer, target_sample_rate: u32) -> AudioBuffer {
    if buffer.sample_rate == target_sample_rate
        || buffer.sample_rate == 0
        || target_sample_rate == 0
    {
        return buffer.clone();
    }

    let ratio = target_sample_rate as f64 / buffer.sample_rate as f64;
    let new_frames = (buffer.frames() as f64 * ratio) as usize;
    if new_frames == 0 || buffer.frames() == 0 {
        return AudioBuffer::new(target_sample_rate, Vec::new(), buffer.channels());
    }
    let channels = buffer.channels() as usize;
    let src_frames = buffer.frames();
    let mut interleaved = Vec::with_capacity(new_frames * channels);

    // One pass over the interleaved source. The previous version de-interleaved into a fresh
    // `Vec` per channel (`AudioBuffer::channel`), resampled each, and then walked the three
    // buffers to re-interleave — two extra full-buffer allocations and an extra traversal whose
    // only purpose was to undo the split.
    for i in 0..new_frames {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        for channel in 0..channels {
            // The final interpolation point clamps to the channel's last sample, so every output
            // channel keeps exactly `new_frames` samples. A source position past the last frame
            // (possible when the ratio rounds up) takes the same fallback.
            let value = if src_idx + 1 < src_frames {
                let index = src_idx * channels + channel;
                let next = index + channels;
                buffer.samples[index] * (1.0 - frac) + buffer.samples[next] * frac
            } else {
                let last = (src_frames - 1) * channels + channel;
                buffer.samples.get(last).copied().unwrap_or(0.0)
            };
            interleaved.push(value);
        }
    }

    AudioBuffer::new(target_sample_rate, interleaved, buffer.channels())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_same_rate() {
        let buf = AudioBuffer::new(44100, vec![0.0; 1000], 1);
        let result = resample(&buf, 44100);
        assert_eq!(result.sample_rate, 44100);
        assert_eq!(result.samples.len(), 1000);
    }

    #[test]
    fn test_resample_downsample() {
        let buf = AudioBuffer::new(44100, vec![0.0; 44100], 1);
        let result = resample(&buf, 22050);
        assert_eq!(result.sample_rate, 22050);
        assert!(result.samples.len() < 44100);
        assert!(result.samples.len() > 20000);
    }

    #[test]
    fn test_resample_upsample() {
        let buf = AudioBuffer::new(22050, vec![0.0; 22050], 1);
        let result = resample(&buf, 44100);
        assert_eq!(result.sample_rate, 44100);
        assert!(result.samples.len() > 40000);
    }

    #[test]
    fn test_resample_stereo() {
        let buf = AudioBuffer::new(44100, vec![0.0; 88200], 2);
        let result = resample(&buf, 22050);
        assert_eq!(result.channels(), 2);
        assert!(result.samples.len() < 88200);
    }

    #[test]
    fn test_resample_zero_rate() {
        let buf = AudioBuffer::new(0, vec![], 1);
        let result = resample(&buf, 44100);
        assert_eq!(result.sample_rate, 0);
    }

    #[test]
    fn test_resample_zero_target_rate_preserves_input() {
        let buf = AudioBuffer::new(44100, vec![0.25, -0.5], 1);
        let result = resample(&buf, 0);
        assert_eq!(result.sample_rate, 44100);
        assert_eq!(result.samples, buf.samples);
    }

    #[test]
    fn test_resample_output_keeps_all_channel_frames() {
        let buf = AudioBuffer::new(2, vec![1.0, 2.0, 3.0], 1);
        let result = resample(&buf, 4);
        assert_eq!(result.samples.len(), 6);
        assert_eq!(result.samples.last().copied(), Some(3.0));
    }
}
