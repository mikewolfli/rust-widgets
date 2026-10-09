// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Audio normalization — RMS and Peak normalization.

use crate::audio::samples::AudioBuffer;

/// Normalize audio buffer to a target level.
pub enum NormalizationTarget {
    /// Peak normalization: scale so the maximum absolute value reaches this level (0.0-1.0).
    Peak(f32),
    /// RMS normalization: scale so the RMS value reaches this level (0.0-1.0).
    Rms(f32),
}

/// Why a [`normalize`] request was rejected.
///
/// The target of a normalization request must be a *finite* level inside the
/// documented `0.0..=1.0` range. A `NaN`, an infinity or an out-of-range value
/// is not a level the buffer can be driven to, so it is reported instead of
/// silently being turned into a tenfold amplification (the old
/// `.min(10.0)` behaviour) or a no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizationError {
    /// The target level was not finite (`NaN` or ±∞).
    NonFiniteTarget,
    /// The target level was finite but outside `0.0..=1.0`.
    TargetOutOfRange,
}

impl std::fmt::Display for NormalizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NormalizationError::NonFiniteTarget => {
                write!(f, "normalization target must be a finite level in 0.0..=1.0")
            }
            NormalizationError::TargetOutOfRange => {
                write!(f, "normalization target must be in 0.0..=1.0")
            }
        }
    }
}

impl std::error::Error for NormalizationError {}

/// Apply normalization to an audio buffer. Modifies in-place.
///
/// The scale factor is applied **exactly**: the request
/// `Peak(level)` drives the buffer's peak to `level` (and `Rms(level)` its RMS),
/// with no undocumented amplification ceiling. Callers that need a ceiling must
/// impose it themselves (e.g. by pre-clamping the buffer), because an implicit
/// cap silently makes the stated target unreachable — the exact defect this
/// signature change removes.
///
/// # Return value
///
/// * `Ok(())` — the buffer was normalized (or left untouched because it was
///   empty/silent, which is a well-defined no-op: silence has no level to raise).
/// * `Err(NormalizationError)` — the requested target was not a valid level and
///   the buffer was **not** modified.
///
pub fn normalize(
    buffer: &mut AudioBuffer,
    target: NormalizationTarget,
) -> Result<(), NormalizationError> {
    let level = match target {
        NormalizationTarget::Peak(level) | NormalizationTarget::Rms(level) => level,
    };
    if !level.is_finite() {
        return Err(NormalizationError::NonFiniteTarget);
    }
    if !(0.0..=1.0).contains(&level) {
        return Err(NormalizationError::TargetOutOfRange);
    }

    if buffer.samples.is_empty() {
        return Ok(());
    }

    let gain = match target {
        NormalizationTarget::Peak(_) => {
            let current_peak = compute_peak(&buffer.samples);
            if current_peak > 0.0 {
                // The target is guaranteed to be exactly reachable: the source
                // peak is strictly positive and finite for any non-silent
                // buffer, so the quotient is a valid, finite gain.
                level / current_peak
            } else {
                1.0
            }
        }
        NormalizationTarget::Rms(_) => {
            let rms = compute_rms(&buffer.samples);
            if rms > 0.0 {
                level / rms
            } else {
                1.0
            }
        }
    };

    if (gain - 1.0).abs() > f32::EPSILON {
        for sample in &mut buffer.samples {
            *sample *= gain;
        }
    }
    Ok(())
}

/// Compute the RMS (Root Mean Square) value of samples.
pub fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Compute the peak (maximum absolute) value of samples.
pub fn compute_peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peak_normalization() {
        let mut buf = AudioBuffer::new(44100, vec![0.5, -0.3, 0.8, -0.2], 1);
        normalize(&mut buf, NormalizationTarget::Peak(1.0)).unwrap();
        let peak = compute_peak(&buf.samples);
        assert!((peak - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rms_normalization() {
        let mut buf = AudioBuffer::new(44100, vec![0.5, -0.3, 0.8, -0.2], 1);
        normalize(&mut buf, NormalizationTarget::Rms(0.5)).unwrap();
        let rms = compute_rms(&buf.samples);
        assert!((rms - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_normalize_empty() {
        let mut buf = AudioBuffer::new(44100, vec![], 1);
        normalize(&mut buf, NormalizationTarget::Peak(1.0)).unwrap(); // Should not crash
        assert!(buf.samples.is_empty());
    }

    #[test]
    fn test_normalize_silence() {
        let mut buf = AudioBuffer::new(44100, vec![0.0; 100], 1);
        normalize(&mut buf, NormalizationTarget::Peak(1.0)).unwrap(); // Should not crash or NaN
        assert!(buf.samples.iter().all(|s| s.is_finite()));
    }

    /// D08-M-01: a valid, quiet request must reach the *exact* requested level,
    /// not an internally clamped tenfold amplification. `[0.01, -0.01]` asked
    /// for `Peak(0.5)` previously produced a 0.099999994 peak; it must now be
    /// 0.5.
    #[test]
    fn peak_reaches_requested_level_without_hidden_cap() {
        let mut buf = AudioBuffer::new(44100, vec![0.01, -0.01], 1);
        normalize(&mut buf, NormalizationTarget::Peak(0.5)).unwrap();
        assert!((compute_peak(&buf.samples) - 0.5).abs() < 1e-6, "{:?}", buf.samples);
    }

    /// D08-M-01: the RMS branch has the same contract and the same removed cap.
    #[test]
    fn rms_reaches_requested_level_without_hidden_cap() {
        // A constant-amplitude buffer has `rms == |amplitude|`, so seeking 0.9
        // from 0.01 needs a gain of 90 — far past the old `.min(10.0)`. The
        // result must match the request.
        let mut buf = AudioBuffer::new(44100, vec![0.01; 64], 1);
        normalize(&mut buf, NormalizationTarget::Rms(0.9)).unwrap();
        assert!((compute_rms(&buf.samples) - 0.9).abs() < 1e-6, "{:?}", compute_rms(&buf.samples));
    }

    /// D08-M-02: a non-finite target must be reported and must leave the samples
    /// untouched — previously `Peak(NaN)` on `[0.5]` produced `[5.0]`.
    #[test]
    fn non_finite_target_is_rejected_without_touching_samples() {
        for target in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let original = vec![0.5f32, -0.25];
            let mut buf = AudioBuffer::new(44100, original.clone(), 1);
            let err = normalize(&mut buf, NormalizationTarget::Peak(target)).unwrap_err();
            assert_eq!(err, NormalizationError::NonFiniteTarget);
            assert_eq!(buf.samples, original, "buffer must be untouched for target {target}");
            let mut buf = AudioBuffer::new(44100, original.clone(), 1);
            let err = normalize(&mut buf, NormalizationTarget::Rms(target)).unwrap_err();
            assert_eq!(err, NormalizationError::NonFiniteTarget);
            assert_eq!(buf.samples, original, "buffer must be untouched for target {target}");
        }
    }

    /// Out-of-range finite targets are rejected the same way in both branches.
    #[test]
    fn out_of_range_target_is_rejected() {
        for target in [-1.0f32, 1.0001, 2.0, -0.0 - 1e-6] {
            let original = vec![0.5f32];
            let mut buf = AudioBuffer::new(44100, original.clone(), 1);
            assert_eq!(
                normalize(&mut buf, NormalizationTarget::Peak(target)).unwrap_err(),
                NormalizationError::TargetOutOfRange
            );
            assert_eq!(buf.samples, original);
            let mut buf = AudioBuffer::new(44100, original.clone(), 1);
            assert_eq!(
                normalize(&mut buf, NormalizationTarget::Rms(target)).unwrap_err(),
                NormalizationError::TargetOutOfRange
            );
            assert_eq!(buf.samples, original);
        }
    }

    /// The endpoints of the documented range are valid.
    #[test]
    fn zero_and_one_targets_are_valid() {
        let mut buf = AudioBuffer::new(44100, vec![0.5, -0.5], 1);
        normalize(&mut buf, NormalizationTarget::Peak(1.0)).unwrap();
        assert!((compute_peak(&buf.samples) - 1.0).abs() < 1e-6);
        // A zero target scales the buffer toward silence exactly.
        normalize(&mut buf, NormalizationTarget::Peak(0.0)).unwrap();
        assert_eq!(compute_peak(&buf.samples), 0.0);
        // A zero target on silence is a no-op, not an error.
        let mut silent = AudioBuffer::new(44100, vec![0.0; 4], 1);
        normalize(&mut silent, NormalizationTarget::Peak(0.0)).unwrap();
        assert!(silent.samples.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn test_compute_rms() {
        let samples = vec![1.0, -1.0, 1.0, -1.0];
        let rms = compute_rms(&samples);
        assert!((rms - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_peak() {
        let samples = vec![0.1, -0.5, 0.3, -0.9, 0.2];
        let peak = compute_peak(&samples);
        assert!((peak - 0.9).abs() < f32::EPSILON);
    }
}
