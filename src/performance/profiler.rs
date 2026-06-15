use crate::compat::HashMap;
use crate::compat::Instant;
use core::time::Duration;
#[derive(Debug, Clone, Copy)]
pub struct ProfileEntry {
    pub start: Instant,
    pub duration: Duration,
    pub call_count: u64,
}
impl Default for ProfileEntry {
    fn default() -> Self {
        Self { start: Instant::now(), duration: Duration::ZERO, call_count: 0 }
    }
}
pub struct Profiler {
    entries: HashMap<String, ProfileEntry>,
    current: Option<(String, Instant)>,
    enabled: bool,
}
impl Profiler {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), current: None, enabled: true }
    }
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn begin(&mut self, name: &str) {
        if !self.enabled {
            return;
        }
        self.current = Some((name.to_string(), Instant::now()));
    }
    pub fn end(&mut self) {
        if !self.enabled {
            return;
        }
        if let Some((name, start)) = self.current.take() {
            let duration = start.elapsed();
            let entry = self.entries.entry(name).or_default();
            entry.duration += duration;
            entry.call_count += 1;
        }
    }
    pub fn measure<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.begin(name);
        let result = f();
        self.end();
        result
    }
    pub fn get_stats(&self, name: &str) -> Option<&ProfileEntry> {
        self.entries.get(name)
    }
    pub fn get_average_duration(&self, name: &str) -> Option<Duration> {
        self.entries.get(name).and_then(|e| {
            if e.call_count > 0 {
                Some(e.duration / e.call_count as u32)
            } else {
                None
            }
        })
    }
    pub fn get_total_duration(&self) -> Duration {
        self.entries.values().map(|e| e.duration).sum()
    }
    pub fn get_hotspots(&self, threshold: Duration) -> Vec<(&str, Duration)> {
        let mut hotspots: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| e.duration >= threshold)
            .map(|(name, e)| (name.as_str(), e.duration))
            .collect();
        hotspots.sort_by_key(|b| core::cmp::Reverse(b.1));
        hotspots
    }
    pub fn get_all_stats(&self) -> &HashMap<String, ProfileEntry> {
        &self.entries
    }
    pub fn reset(&mut self) {
        self.entries.clear();
        self.current = None;
    }
    pub fn report(&self) -> ProfileReport {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|(name, entry)| ProfileReportEntry {
                name: name.clone(),
                total_duration: entry.duration,
                call_count: entry.call_count,
                average_duration: if entry.call_count > 0 {
                    entry.duration / entry.call_count as u32
                } else {
                    Duration::ZERO
                },
            })
            .collect();
        entries.sort_by_key(|b| core::cmp::Reverse(b.total_duration));
        ProfileReport { entries, total_duration: self.get_total_duration() }
    }
}
impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone)]
pub struct ProfileReportEntry {
    pub name: String,
    pub total_duration: Duration,
    pub call_count: u64,
    pub average_duration: Duration,
}
#[derive(Debug, Clone)]
pub struct ProfileReport {
    pub entries: Vec<ProfileReportEntry>,
    pub total_duration: Duration,
}
impl ProfileReport {
    pub fn to_string_summary(&self) -> String {
        let mut result = String::new();
        result.push_str(&format!("Total: {:?}\n\n", self.total_duration));
        for entry in &self.entries {
            result.push_str(&format!(
                "{}: {:?} ({} calls, avg {:?})\n",
                entry.name, entry.total_duration, entry.call_count, entry.average_duration
            ));
        }
        result
    }
}
pub struct FrameProfiler {
    frame_times: Vec<Duration>,
    max_frames: usize,
    current_frame_start: Option<Instant>,
    sections: HashMap<String, Duration>,
    current_section: Option<(String, Instant)>,
}
impl FrameProfiler {
    pub fn new(max_frames: usize) -> Self {
        Self {
            frame_times: Vec::with_capacity(max_frames),
            max_frames,
            current_frame_start: None,
            sections: HashMap::new(),
            current_section: None,
        }
    }
    pub fn begin_frame(&mut self) {
        self.current_frame_start = Some(Instant::now());
        self.sections.clear();
    }
    pub fn end_frame(&mut self) {
        if let Some(start) = self.current_frame_start.take() {
            let duration = start.elapsed();
            if self.frame_times.len() >= self.max_frames {
                self.frame_times.remove(0);
            }
            self.frame_times.push(duration);
        }
    }
    pub fn begin_section(&mut self, name: &str) {
        self.current_section = Some((name.to_string(), Instant::now()));
    }
    pub fn end_section(&mut self) {
        if let Some((name, start)) = self.current_section.take() {
            let duration = start.elapsed();
            *self.sections.entry(name).or_default() += duration;
        }
    }
    pub fn average_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.frame_times.iter().sum();
        total / self.frame_times.len() as u32
    }
    pub fn fps(&self) -> f32 {
        let avg = self.average_frame_time();
        if avg.is_zero() {
            return 0.0;
        }
        1_000_000_000.0 / avg.as_nanos() as f32
    }
    pub fn min_frame_time(&self) -> Duration {
        self.frame_times.iter().min().copied().unwrap_or(Duration::ZERO)
    }
    pub fn max_frame_time(&self) -> Duration {
        self.frame_times.iter().max().copied().unwrap_or(Duration::ZERO)
    }
    pub fn frame_count(&self) -> usize {
        self.frame_times.len()
    }
    pub fn sections(&self) -> &HashMap<String, Duration> {
        &self.sections
    }
    pub fn clear(&mut self) {
        self.frame_times.clear();
        self.sections.clear();
    }
}
impl Default for FrameProfiler {
    fn default() -> Self {
        Self::new(60)
    }
}
pub struct PerformanceMonitor {
    profiler: Profiler,
    frame_profiler: FrameProfiler,
    enabled: bool,
}
impl PerformanceMonitor {
    pub fn new() -> Self {
        Self { profiler: Profiler::new(), frame_profiler: FrameProfiler::new(60), enabled: true }
    }
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }
    pub fn profiler_mut(&mut self) -> &mut Profiler {
        &mut self.profiler
    }
    pub fn frame_profiler(&self) -> &FrameProfiler {
        &self.frame_profiler
    }
    pub fn frame_profiler_mut(&mut self) -> &mut FrameProfiler {
        &mut self.frame_profiler
    }
    pub fn enable(&mut self) {
        self.enabled = true;
        self.profiler.enable();
    }
    pub fn disable(&mut self) {
        self.enabled = false;
        self.profiler.disable();
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn begin_frame(&mut self) {
        if self.enabled {
            self.frame_profiler.begin_frame();
        }
    }
    pub fn end_frame(&mut self) {
        if self.enabled {
            self.frame_profiler.end_frame();
        }
    }
    pub fn begin_section(&mut self, name: &str) {
        if self.enabled {
            self.frame_profiler.begin_section(name);
            self.profiler.begin(name);
        }
    }
    pub fn end_section(&mut self) {
        if self.enabled {
            self.profiler.end();
            self.frame_profiler.end_section();
        }
    }
    pub fn measure<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.begin_section(name);
        let result = f();
        self.end_section();
        result
    }
    pub fn report(&self) -> PerformanceReport {
        PerformanceReport {
            profiler_report: self.profiler.report(),
            average_frame_time: self.frame_profiler.average_frame_time(),
            fps: self.frame_profiler.fps(),
            min_frame_time: self.frame_profiler.min_frame_time(),
            max_frame_time: self.frame_profiler.max_frame_time(),
            frame_count: self.frame_profiler.frame_count(),
        }
    }
    pub fn reset(&mut self) {
        self.profiler.reset();
        self.frame_profiler.clear();
    }
}
impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub profiler_report: ProfileReport,
    pub average_frame_time: Duration,
    pub fps: f32,
    pub min_frame_time: Duration,
    pub max_frame_time: Duration,
    pub frame_count: usize,
}
impl PerformanceReport {
    pub fn to_string_summary(&self) -> String {
        format!(
            "FPS: {:.1}\nAvg Frame: {:?}\nMin Frame: {:?}\nMax Frame: {:?}\nFrames: {}\n\n{}",
            self.fps,
            self.average_frame_time,
            self.min_frame_time,
            self.max_frame_time,
            self.frame_count,
            self.profiler_report.to_string_summary()
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "mini"))]
    use std::thread::sleep;
    #[test]
    fn test_profiler() {
        let mut profiler = Profiler::new();
        profiler.begin("test");
        sleep(Duration::from_millis(1));
        profiler.end();
        let stats = profiler.get_stats("test").unwrap();
        assert_eq!(stats.call_count, 1);
        assert!(stats.duration > Duration::ZERO);
    }
    #[test]
    fn test_frame_profiler() {
        let mut profiler = FrameProfiler::new(10);
        for _ in 0..5 {
            profiler.begin_frame();
            sleep(Duration::from_millis(1));
            profiler.end_frame();
        }
        assert_eq!(profiler.frame_count(), 5);
        assert!(profiler.fps() > 0.0);
    }
}
