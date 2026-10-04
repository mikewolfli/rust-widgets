// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::MiniToString;
use crate::compat::{format, String, Vec};
use core::hash::{Hash, Hasher};
// NOTE: `DefaultHasher`, `fs` and `Path` are std-only; left on `std` deliberately.
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::path::Path;
/// Snapshot for visual regression testing
///
/// Holds one rendered frame plus its expected dimensions. `data` is the raw
/// pixel buffer in the same layout the renderer produces (RGBA byte quads, four
/// bytes per pixel, tightly packed), which is what [`Snapshot::compare`] assumes
/// when it steps through the buffer four bytes at a time.
///
/// A user of the crate would normally not construct these directly: they come
/// out of the renderer's frame accessor and are handed to a [`SnapshotManager`],
/// which owns naming, storage, and tolerance.
pub struct Snapshot {
    /// Logical snapshot name; also the file stem used when saving and loading.
    pub name: String,
    /// Raw pixel bytes, four per pixel in RGBA order.
    pub data: Vec<u8>,
    /// Width in pixels. Persisted by [`Snapshot::save`] and recovered by
    /// [`Snapshot::load`], so a reloaded snapshot compares its geometry too.
    pub width: u32,
    /// Height in pixels. Persisted by [`Snapshot::save`] and recovered by
    /// [`Snapshot::load`], so a reloaded snapshot compares its geometry too.
    pub height: u32,
}

/// On-disk magic for [`Snapshot::save`] / [`Snapshot::load`].
const SNAPSHOT_MAGIC: &[u8; 8] = b"RWSNAP01";
/// Byte length of the on-disk header: magic (8) + width/height/data_len (3 x 4).
const SNAPSHOT_HEADER_LEN: usize = 8 + 12;

/// The number of RGBA bytes a `width x height` snapshot must hold.
fn expected_bytes(width: u32, height: u32) -> Result<usize, String> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("dimensions {width}x{height} overflow"))?;
    pixels
        .checked_mul(4)
        .ok_or_else(|| format!("dimensions {width}x{height} overflow the RGBA buffer"))
}
impl Snapshot {
    /// Creates a snapshot from a name, raw pixel bytes, and dimensions.
    ///
    /// This is the unchecked constructor: it accepts any buffer and dimension
    /// pair so test fixtures can be written tersely. Prefer
    /// [`Snapshot::try_new`] when the buffer must be validated against
    /// `width * height * 4`.
    pub fn new(name: &str, data: Vec<u8>, width: u32, height: u32) -> Self {
        Self { name: name.to_string(), data, width, height }
    }

    /// Creates a snapshot, validating that `data` holds exactly
    /// `width * height * 4` bytes of RGBA pixels.
    ///
    /// Returns an error describing the mismatch rather than building a snapshot
    /// that would later panic or compare incorrectly.
    pub fn try_new(name: &str, data: Vec<u8>, width: u32, height: u32) -> Result<Self, String> {
        let expected = expected_bytes(width, height)?;
        if data.len() != expected {
            return Err(format!(
                "buffer of {} bytes does not match {width}x{height} RGBA (expected {expected})",
                data.len()
            ));
        }
        Ok(Self { name: name.to_string(), data, width, height })
    }
    /// Hashes the pixel data with the standard library's default hasher.
    ///
    /// Only `data` takes part, so the name and dimensions do not affect the
    /// value. The hasher is not guaranteed to be stable across Rust releases,
    /// so treat the result as process-local rather than a durable file key.
    pub fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.data.hash(&mut hasher);
        hasher.finish()
    }
    /// Writes the snapshot to `<dir>/<name>.bin`, creating `dir` if needed.
    ///
    /// The dimensions are persisted in a small header before the pixel bytes so
    /// [`Snapshot::load`] can restore them; see [`Snapshot::load`] for the
    /// format. Saving an invalid buffer (one whose length does not match
    /// `width * height * 4`) fails rather than writing a snapshot that could
    /// never be compared.
    pub fn save(&self, dir: &str) -> Result<(), String> {
        let expected = expected_bytes(self.width, self.height)?;
        if self.data.len() != expected {
            return Err(format!(
                "refusing to save {} bytes for {w}x{h} RGBA (expected {expected})",
                self.data.len(),
                w = self.width,
                h = self.height,
            ));
        }
        let path = Path::new(dir).join(format!("{}.bin", self.name));
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut bytes = Vec::with_capacity(SNAPSHOT_HEADER_LEN + self.data.len());
        bytes.extend_from_slice(SNAPSHOT_MAGIC);
        bytes.extend_from_slice(&self.width.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&self.data);
        fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Reads a previously saved snapshot from `<dir>/<name>.bin`.
    ///
    /// The on-disk format is an 8-byte magic, then `width`, `height` and
    /// `data_len` as little-endian `u32`, then the raw RGBA bytes. A file that
    /// is truncated, carries the wrong magic, or whose recorded dimensions do
    /// not match its buffer is rejected with an error rather than yielding a
    /// snapshot that would mis-compare.
    pub fn load(name: &str, dir: &str) -> Result<Self, String> {
        let path = Path::new(dir).join(format!("{name}.bin"));
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.len() < SNAPSHOT_HEADER_LEN {
            return Err(format!(
                "corrupt snapshot '{}': {} bytes is shorter than the {SNAPSHOT_HEADER_LEN}-byte header",
                path.display(),
                bytes.len()
            ));
        }
        if &bytes[..SNAPSHOT_MAGIC.len()] != SNAPSHOT_MAGIC {
            return Err(format!("corrupt snapshot '{}': bad magic", path.display()));
        }
        let width = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let height = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let data_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        if SNAPSHOT_HEADER_LEN + data_len != bytes.len() {
            return Err(format!(
                "corrupt snapshot '{}': header declares {data_len} bytes but {} remain",
                path.display(),
                bytes.len() - SNAPSHOT_HEADER_LEN
            ));
        }
        let expected = expected_bytes(width, height)?;
        if data_len != expected {
            return Err(format!(
                "corrupt snapshot '{}': {width}x{height} expects {expected} bytes, found {data_len}",
                path.display()
            ));
        }
        Ok(Self {
            name: name.to_string(),
            data: bytes[SNAPSHOT_HEADER_LEN..].to_vec(),
            width,
            height,
        })
    }
    /// Compares this snapshot (the reference) against `other` (the candidate).
    ///
    /// `tolerance` is a **percentage of differing pixels**, not a per-channel
    /// colour threshold: a pixel counts as different when any of its four RGBA
    /// bytes differ, and the result is that count over the total pixel count
    /// times `100.0`. So `tolerance = 0.01` allows 0.01% of pixels to differ,
    /// while `1.0` allows one percent.
    ///
    /// The dimensions are consulted: `1x2` and `2x1` hold the same number of
    /// bytes but are not the same image, so a geometry difference is reported as
    /// [`SnapshotComparison::Different`] rather than compared pixel by pixel. A
    /// buffer whose length is not a whole number of RGBA quads is also reported
    /// as `Different` instead of panicking on the four-byte stride.
    pub fn compare(&self, other: &Snapshot, tolerance: f32) -> SnapshotComparison {
        // A geometry difference is a difference: `1x2` and `2x1` hold the same
        // byte count but are not the same image.
        if self.width != other.width || self.height != other.height {
            return SnapshotComparison::Different {
                reason: format!(
                    "Geometry mismatch: {0}x{1} vs {2}x{3}",
                    self.width, self.height, other.width, other.height
                ),
                diff_percentage: 100.0,
            };
        }
        if self.data.len() != other.data.len() {
            return SnapshotComparison::Different {
                reason: "Size mismatch".to_string(),
                diff_percentage: 100.0,
            };
        }
        // A buffer whose length is not a whole number of RGBA quads cannot be
        // stepped in fours; report it rather than panicking on the stride.
        if !self.data.len().is_multiple_of(4) {
            return SnapshotComparison::Different {
                reason: format!("Buffer of {} bytes is not whole RGBA pixels", self.data.len()),
                diff_percentage: 100.0,
            };
        }
        let mut diff_count = 0;
        let total_pixels = self.data.len() / 4;
        for i in (0..self.data.len()).step_by(4) {
            if self.data[i] != other.data[i]
                || self.data[i + 1] != other.data[i + 1]
                || self.data[i + 2] != other.data[i + 2]
                || self.data[i + 3] != other.data[i + 3]
            {
                diff_count += 1;
            }
        }
        let diff_percentage =
            if total_pixels > 0 { (diff_count as f32 / total_pixels as f32) * 100.0 } else { 0.0 };
        if diff_percentage == 0.0 {
            SnapshotComparison::Identical
        } else if diff_percentage <= tolerance {
            SnapshotComparison::Similar { diff_percentage }
        } else {
            SnapshotComparison::Different {
                reason: format!("Difference: {diff_percentage:.2}%"),
                diff_percentage,
            }
        }
    }
}
/// Outcome of comparing two snapshots.
///
/// Reported by [`Snapshot::compare`] and by
/// [`SnapshotManager::compare_or_create`]; callers usually only need
/// [`SnapshotComparison::is_match`].
#[derive(Debug, Clone)]
pub enum SnapshotComparison {
    /// The two snapshots have identical bytes; nothing differed.
    Identical,
    /// Some pixels differed, but within the caller's tolerance.
    Similar {
        /// Difference as a percentage of total pixels, in `0.0..=100.0`.
        diff_percentage: f32,
    },
    /// The difference exceeded the tolerance, or the comparison could not be
    /// performed at all (size mismatch, or a save/load failure).
    Different {
        /// Human-readable explanation; for a pure difference this is the
        /// formatted percentage.
        reason: String,
        /// Difference as a percentage of total pixels, in `0.0..=100.0`.
        /// Deliberately `100.0` when the comparison could not run, so that the
        /// value is never mistaken for "nearly identical".
        diff_percentage: f32,
    },
}
impl SnapshotComparison {
    /// Returns `true` for [`SnapshotComparison::Identical`] and
    /// [`SnapshotComparison::Similar`], i.e. whenever the comparison is
    /// considered a pass.
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Identical | Self::Similar { .. })
    }
    /// Returns the difference as a percentage of total pixels.
    ///
    /// [`SnapshotComparison::Identical`] reports `0.0`.
    pub fn diff_percentage(&self) -> f32 {
        match self {
            Self::Identical => 0.0,
            Self::Similar { diff_percentage } => *diff_percentage,
            Self::Different { diff_percentage, .. } => *diff_percentage,
        }
    }
}
/// Snapshot manager
///
/// Owns the snapshot directory, the comparison tolerance, and whether existing
/// baselines are overwritten. Tests call
/// [`SnapshotManager::compare_or_create`] and assert on the returned
/// [`SnapshotComparison`]; the manager does no asserting itself.
///
/// This is the type a crate user is most likely to touch, since it is what
/// wires the rendering path to a checked-in baseline directory.
pub struct SnapshotManager {
    snapshot_dir: String,
    tolerance: f32,
    update_mode: bool,
}
impl SnapshotManager {
    /// Creates a manager over `snapshot_dir` with a tolerance of `0.01` percent
    /// of pixels and update mode disabled.
    pub fn new(snapshot_dir: &str) -> Self {
        Self { snapshot_dir: snapshot_dir.to_string(), tolerance: 0.01, update_mode: false }
    }
    /// Builder-style setter for the comparison tolerance.
    ///
    /// The unit is a percentage of differing pixels, the same as
    /// [`Snapshot::compare`]; there is no lower bound, so `0.0` demands an exact
    /// match.
    pub fn with_tolerance(mut self, tolerance: f32) -> Self {
        self.tolerance = tolerance;
        self
    }
    /// Builder-style setter for update mode.
    ///
    /// When `update` is `true`, [`SnapshotManager::compare_or_create`] rewrites
    /// the baseline from the candidate instead of comparing against it, so a
    /// run in update mode silently accepts every change. Intended for
    /// regenerating baselines deliberately, not for normal test runs.
    pub fn with_update_mode(mut self, update: bool) -> Self {
        self.update_mode = update;
        self
    }
    /// Compares `snapshot` against the stored baseline, creating it when absent.
    ///
    /// The baseline path is `<snapshot_dir>/<name>.bin`, where `name` comes from
    /// the snapshot's own name field. A missing baseline, or update mode, causes
    /// the candidate to be saved and reported as
    /// [`SnapshotComparison::Identical`] — so a first run always passes and
    /// establishes the baseline. Otherwise the stored bytes are compared using
    /// the manager's tolerance.
    ///
    /// A save or load failure is reported as [`SnapshotComparison::Different`]
    /// with a message rather than being returned as an `Err`, so callers that
    /// only check `is_match` still fail the test.
    pub fn compare_or_create(&self, name: &str, snapshot: &Snapshot) -> SnapshotComparison {
        let existing_path = Path::new(&self.snapshot_dir).join(format!("{name}.bin"));
        if !existing_path.exists() || self.update_mode {
            if let Err(e) = snapshot.save(&self.snapshot_dir) {
                return SnapshotComparison::Different {
                    reason: format!("Failed to save: {e}"),
                    diff_percentage: 100.0,
                };
            }
            return SnapshotComparison::Identical;
        }
        match Snapshot::load(name, &self.snapshot_dir) {
            Ok(existing) => existing.compare(snapshot, self.tolerance),
            Err(e) => SnapshotComparison::Different {
                reason: format!("Failed to load: {e}"),
                diff_percentage: 100.0,
            },
        }
    }
    /// Returns the directory baselines are read from and written to.
    pub fn snapshot_dir(&self) -> &str {
        &self.snapshot_dir
    }
}
impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new("tests/snapshots")
    }
}
/// Performance snapshot for regression testing
///
/// A named list of scalar metrics, such as frames per second or peak memory.
/// Metrics preserve insertion order and are compared positionally, so the same
/// snapshot name must always record the same metrics in the same order for
/// comparisons to mean anything.
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    /// Logical name; also the file stem used when saving and loading.
    pub name: String,
    /// Recorded metrics as `(metric name, value)` pairs in insertion order.
    pub metrics: Vec<(String, f64)>,
}
impl PerformanceSnapshot {
    /// Creates an empty snapshot with the given name.
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), metrics: Vec::new() }
    }
    /// Appends a metric.
    ///
    /// Duplicate names are allowed and are kept as separate entries; they are
    /// not merged or de-duplicated.
    pub fn add_metric(&mut self, name: &str, value: f64) {
        self.metrics.push((name.to_string(), value));
    }
    /// Returns `true` when `other` has the same metrics within `tolerance`.
    ///
    /// `tolerance` is a **relative fraction**, not a percentage: each pair is
    /// compared as `|a - b| / max(|a|, |b|)` and must be at most `tolerance`, so
    /// `0.1` allows a ten percent deviation. Using the larger magnitude as the
    /// denominator keeps the comparison well-defined for negative metrics, where
    /// `max(a, b)` could be non-positive or miss the actual scale.
    ///
    /// The comparison is order-sensitive and name-sensitive: differing metric
    /// counts, or a name mismatch at any position, returns `false` regardless
    /// of the values. A non-finite metric (`NaN`/infinite) is a data error and
    /// never compares as matching; an invalid `tolerance` (non-finite or
    /// negative) likewise returns `false` rather than silently passing.
    ///
    /// Note that this is a plain predicate, not a [`SnapshotComparison`]: it
    /// does not report which metric differed or by how much.
    pub fn compare(&self, other: &PerformanceSnapshot, tolerance: f64) -> bool {
        // An invalid tolerance cannot define a comparison, so nothing matches.
        if !tolerance.is_finite() || tolerance < 0.0 {
            return false;
        }
        if self.metrics.len() != other.metrics.len() {
            return false;
        }
        for ((name1, value1), (name2, value2)) in self.metrics.iter().zip(other.metrics.iter()) {
            if name1 != name2 {
                return false;
            }
            // A non-finite metric is a data error, not a value to be compared.
            if !value1.is_finite() || !value2.is_finite() {
                return false;
            }
            if value1 == value2 {
                continue;
            }
            let denom = value1.abs().max(value2.abs());
            let diff_percentage = (value1 - value2).abs() / denom;
            if diff_percentage > tolerance {
                return false;
            }
        }
        true
    }
    /// Writes the metrics to `<dir>/<name>.perf`, creating `dir` if needed.
    ///
    /// # Format
    ///
    /// A UTF-8 text file with one `name=value` pair per line, values written
    /// with `f64`'s default formatting. Metric names must therefore not contain
    /// a `=` or a newline, or the round-trip through
    /// [`PerformanceSnapshot::load`] will not reproduce them.
    pub fn save(&self, dir: &str) -> Result<(), String> {
        let path = Path::new(dir).join(format!("{}.perf", self.name));
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let content = self
            .metrics
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Reads metrics from `<dir>/<name>.perf`.
    ///
    /// Lines without a `=` and values that do not parse as `f64` are skipped
    /// silently, so a corrupt file yields a snapshot with fewer metrics rather
    /// than an error; the subsequent [`PerformanceSnapshot::compare`] then fails
    /// on the count mismatch.
    pub fn load(name: &str, dir: &str) -> Result<Self, String> {
        let path = Path::new(dir).join(format!("{name}.perf"));
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut snapshot = Self::new(name);
        for line in content.lines() {
            if let Some((name, value)) = line.split_once('=') {
                if let Ok(value) = value.parse::<f64>() {
                    snapshot.add_metric(name, value);
                }
            }
        }
        Ok(snapshot)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    #[test]
    fn test_snapshot_comparison() {
        let snapshot1 = Snapshot::new("test", vec![255, 0, 0, 255], 1, 1);
        let snapshot2 = Snapshot::new("test", vec![255, 0, 0, 255], 1, 1);
        let snapshot3 = Snapshot::new("test", vec![255, 255, 0, 255], 1, 1);
        let comparison1 = snapshot1.compare(&snapshot2, 0.01);
        assert!(matches!(comparison1, SnapshotComparison::Identical));
        let comparison2 = snapshot1.compare(&snapshot3, 0.01);
        assert!(!comparison2.is_match());
    }
    #[test]
    fn test_performance_snapshot() {
        let mut snapshot1 = PerformanceSnapshot::new("perf_test");
        snapshot1.add_metric("fps", 60.0);
        snapshot1.add_metric("memory", 1024.0);
        let mut snapshot2 = PerformanceSnapshot::new("perf_test");
        snapshot2.add_metric("fps", 59.5);
        snapshot2.add_metric("memory", 1025.0);
        assert!(snapshot1.compare(&snapshot2, 0.1));
        let mut snapshot3 = PerformanceSnapshot::new("perf_test");
        snapshot3.add_metric("fps", 30.0);
        snapshot3.add_metric("memory", 1024.0);
        assert!(!snapshot1.compare(&snapshot3, 0.1));
    }

    // ── S-08: geometry validation, round-trip, and malformed-input handling ──

    #[test]
    fn try_new_validates_the_buffer_against_the_dimensions() {
        assert!(Snapshot::try_new("ok", vec![0; 4], 1, 1).is_ok());
        assert!(Snapshot::try_new("short", vec![0; 3], 1, 1).is_err());
        assert!(Snapshot::try_new("long", vec![0; 8], 1, 1).is_err());
        // 2x2 needs 16 bytes; an 8-byte buffer is 1x2's worth, not 2x2's.
        assert!(Snapshot::try_new("wrong_geometry", vec![0; 8], 2, 2).is_err());
    }

    #[test]
    fn compare_distinguishes_geometry_with_the_same_byte_count() {
        // 1x2 and 2x1 both hold 8 bytes (two pixels) but are not the same image.
        let one_by_two = Snapshot::new("a", vec![1, 2, 3, 4, 5, 6, 7, 8], 1, 2);
        let two_by_one = Snapshot::new("b", vec![1, 2, 3, 4, 5, 6, 7, 8], 2, 1);
        let comparison = one_by_two.compare(&two_by_one, 0.01);
        assert!(
            matches!(comparison, SnapshotComparison::Different { .. }),
            "a geometry difference must not compare as identical: {comparison:?}"
        );
        assert!(!comparison.is_match());
    }

    #[test]
    fn compare_does_not_panic_on_a_sub_rgba_buffer() {
        // Same length, but not a whole number of RGBA quads. The old stride-4 loop
        // indexed past the end and panicked; it must report a difference instead.
        let left = Snapshot::new("a", vec![0], 1, 1);
        let right = Snapshot::new("b", vec![0], 1, 1);
        let comparison = left.compare(&right, 0.01);
        assert!(matches!(comparison, SnapshotComparison::Different { .. }));
        assert!(!comparison.is_match());
    }

    #[test]
    fn save_and_load_round_trip_the_geometry() {
        let dir = temp_dir("round_trip");
        let original = Snapshot::new("shot", vec![1, 2, 3, 4, 5, 6, 7, 8], 1, 2);
        original.save(&dir).expect("save");
        let loaded = Snapshot::load("shot", &dir).expect("load");
        assert_eq!(loaded.width, 1);
        assert_eq!(loaded.height, 2);
        assert_eq!(loaded.data, original.data);
        assert!(matches!(loaded.compare(&original, 0.0), SnapshotComparison::Identical));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_rejects_a_buffer_that_does_not_match_the_dimensions() {
        let dir = temp_dir("bad_save");
        let bad = Snapshot::new("bad", vec![1, 2, 3], 1, 1);
        assert!(bad.save(&dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_corrupt_files_clearly() {
        let dir = temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();

        // Too short to contain the header.
        let path = Path::new(&dir).join("short.bin");
        fs::write(&path, [0u8; 4]).unwrap();
        assert!(Snapshot::load("short", &dir).is_err());

        // Right length but wrong magic.
        let path = Path::new(&dir).join("magic.bin");
        fs::write(&path, [0u8; SNAPSHOT_HEADER_LEN]).unwrap();
        assert!(Snapshot::load("magic", &dir).is_err());

        // Valid header but a declared buffer length that does not match the file.
        let path = Path::new(&dir).join("truncated.bin");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(SNAPSHOT_MAGIC);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.push(0); // one byte of a four-byte pixel is missing
        fs::write(&path, bytes).unwrap();
        assert!(Snapshot::load("truncated", &dir).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    /// A unique scratch directory under the system temp dir, removed by the caller.
    fn temp_dir(tag: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("rw_snapshot_{}_{}_{}", tag, std::process::id(), unique))
            .to_string_lossy()
            .into_owned()
    }

    // ── S-09: NaN / Inf / invalid-tolerance semantics ──

    #[test]
    fn performance_compare_rejects_non_finite_metrics() {
        let baseline = perf(&[("frame_ms", 1.0)]);
        let nan = perf(&[("frame_ms", f64::NAN)]);
        let inf = perf(&[("frame_ms", f64::INFINITY)]);
        assert!(!baseline.compare(&nan, 0.01), "a NaN metric must not pass");
        assert!(!baseline.compare(&inf, 0.01), "an infinite metric must not pass");
    }

    #[test]
    fn performance_compare_rejects_invalid_tolerances() {
        let baseline = perf(&[("frame_ms", 1.0)]);
        let candidate = perf(&[("frame_ms", 1.0)]);
        assert!(!baseline.compare(&candidate, f64::NAN));
        assert!(!baseline.compare(&candidate, f64::INFINITY));
        assert!(!baseline.compare(&candidate, -0.1));
        assert!(baseline.compare(&candidate, 0.0), "an exact match passes a zero tolerance");
    }

    #[test]
    fn performance_compare_defines_signed_metric_semantics() {
        // Negative values use the larger magnitude as the denominator, so a small
        // relative deviation still passes.
        let a = perf(&[("rate", -100.0)]);
        let b = perf(&[("rate", -101.0)]);
        assert!(a.compare(&b, 0.05), "-100 vs -101 is a 1% deviation");
        assert!(!a.compare(&b, 0.001), "and fails a tighter tolerance");
        // A sign flip is a large change and must fail, not be skipped as non-positive.
        let positive = perf(&[("rate", 100.0)]);
        let negative = perf(&[("rate", -100.0)]);
        assert!(!positive.compare(&negative, 0.1));
    }

    fn perf(metrics: &[(&str, f64)]) -> PerformanceSnapshot {
        let mut snapshot = PerformanceSnapshot::new("perf");
        for (name, value) in metrics {
            snapshot.add_metric(name, *value);
        }
        snapshot
    }
}
