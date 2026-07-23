use core::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::path::Path;
/// Snapshot for visual regression testing
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub name: String,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
impl Snapshot {
    pub fn new(name: &str, data: Vec<u8>, width: u32, height: u32) -> Self {
        Self { name: name.to_string(), data, width, height }
    }
    pub fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.data.hash(&mut hasher);
        hasher.finish()
    }
    pub fn save(&self, dir: &str) -> Result<(), String> {
        let path = Path::new(dir).join(format!("{}.bin", self.name));
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::write(&path, &self.data).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn load(name: &str, dir: &str) -> Result<Self, String> {
        let path = Path::new(dir).join(format!("{name}.bin"));
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        Ok(Self { name: name.to_string(), data, width: 0, height: 0 })
    }
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
#[derive(Debug, Clone)]
pub enum SnapshotComparison {
    Identical,
    Similar { diff_percentage: f32 },
    Different { reason: String, diff_percentage: f32 },
}
impl SnapshotComparison {
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Identical | Self::Similar { .. })
    }
    pub fn diff_percentage(&self) -> f32 {
        match self {
            Self::Identical => 0.0,
            Self::Similar { diff_percentage } => *diff_percentage,
            Self::Different { diff_percentage, .. } => *diff_percentage,
        }
    }
}
/// Snapshot manager
pub struct SnapshotManager {
    snapshot_dir: String,
    tolerance: f32,
    update_mode: bool,
}
impl SnapshotManager {
    pub fn new(snapshot_dir: &str) -> Self {
        Self { snapshot_dir: snapshot_dir.to_string(), tolerance: 0.01, update_mode: false }
    }
    pub fn with_tolerance(mut self, tolerance: f32) -> Self {
        self.tolerance = tolerance;
        self
    }
    pub fn with_update_mode(mut self, update: bool) -> Self {
        self.update_mode = update;
        self
    }
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
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    pub name: String,
    pub metrics: Vec<(String, f64)>,
}
impl PerformanceSnapshot {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), metrics: Vec::new() }
    }
    pub fn add_metric(&mut self, name: &str, value: f64) {
        self.metrics.push((name.to_string(), value));
    }
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
