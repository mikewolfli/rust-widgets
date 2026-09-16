// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use core::hash::{Hash, Hasher};
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
    /// Width in pixels. Not stored on disk, so it is not recovered by
    /// [`Snapshot::load`].
    pub width: u32,
    /// Height in pixels. Not stored on disk, so it is not recovered by
    /// [`Snapshot::load`].
    pub height: u32,
}
impl Snapshot {
    /// Creates a snapshot from a name, raw pixel bytes, and dimensions.
    ///
    /// Nothing is validated: the length of `data` is not checked against
    /// `width * height * 4`.
    pub fn new(name: &str, data: Vec<u8>, width: u32, height: u32) -> Self {
        Self { name: name.to_string(), data, width, height }
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
    /// Writes the pixel data to `<dir>/<name>.bin`, creating `dir` if needed.
    ///
    /// Only the raw bytes are written: the dimensions are *not* persisted, so a
    /// snapshot reloaded by [`Snapshot::load`] comes back with `width` and
    /// `height` set to `0`. Errors are returned as display strings rather than a
    /// typed error.
    pub fn save(&self, dir: &str) -> Result<(), String> {
        let path = Path::new(dir).join(format!("{}.bin", self.name));
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::write(&path, &self.data).map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Reads a previously saved snapshot from `<dir>/<name>.bin`.
    ///
    /// The returned snapshot has the given `name` but zero dimensions, because
    /// only pixel bytes are stored; see [`Snapshot::save`]. This is why
    /// [`Snapshot::compare`] derives the pixel count from the data length
    /// rather than from the dimensions.
    pub fn load(name: &str, dir: &str) -> Result<Self, String> {
        let path = Path::new(dir).join(format!("{name}.bin"));
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        Ok(Self { name: name.to_string(), data, width: 0, height: 0 })
    }
    /// Compares this snapshot (the reference) against `other` (the candidate).
    ///
    /// `tolerance` is a **percentage of differing pixels**, not a per-channel
    /// colour threshold: a pixel counts as different when any of its four RGBA
    /// bytes differ, and the result is that count over the total pixel count
    /// times `100.0`. So `tolerance = 0.01` allows 0.01% of pixels to differ,
    /// while `1.0` allows one percent.
    ///
    /// The dimensions are not consulted; a length mismatch is reported as
    /// [`SnapshotComparison::Different`] with `100.0` percent rather than being
    /// compared pixel by pixel. An empty candidate is reported as
    /// [`SnapshotComparison::Identical`].
    pub fn compare(&self, other: &Snapshot, tolerance: f32) -> SnapshotComparison {
        if self.data.len() != other.data.len() {
            return SnapshotComparison::Different {
                reason: "Size mismatch".to_string(),
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
    /// compared as `|a - b| / max(a, b)` and must be at most `tolerance`, so
    /// `0.1` allows a ten percent deviation.
    ///
    /// The comparison is order-sensitive and name-sensitive: differing metric
    /// counts, or a name mismatch at any position, returns `false` regardless
    /// of the values. Pairs where both values are `<= 0.0` are treated as
    /// matching, and NaN values never compare greater than the tolerance.
    ///
    /// Note that this is a plain predicate, not a [`SnapshotComparison`]: it
    /// does not report which metric differed or by how much.
    pub fn compare(&self, other: &PerformanceSnapshot, tolerance: f64) -> bool {
        if self.metrics.len() != other.metrics.len() {
            return false;
        }
        for ((name1, value1), (name2, value2)) in self.metrics.iter().zip(other.metrics.iter()) {
            if name1 != name2 {
                return false;
            }
            let diff = (value1 - value2).abs();
            let max_val = value1.max(*value2);
            if max_val > 0.0 {
                let diff_percentage = diff / max_val;
                if diff_percentage > tolerance {
                    return false;
                }
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
}
