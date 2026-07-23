//! Test infrastructure types and utilities.

use std::time::{Duration, Instant};

/// Test configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub timeout: Duration,
    pub snapshot_dir: String,
    pub tolerance: f32,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            snapshot_dir: "tests/snapshots".to_string(),
            tolerance: 0.01,
        }
    }
}

/// Test result
#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub duration: Duration,
    pub message: Option<String>,
}

impl TestResult {
    pub fn pass(name: &str, duration: Duration) -> Self {
        Self { name: name.to_string(), passed: true, duration, message: None }
    }
    pub fn fail(name: &str, duration: Duration, message: &str) -> Self {
        Self { name: name.to_string(), passed: false, duration, message: Some(message.to_string()) }
    }
}

/// Test runner
pub struct TestRunner {
    config: TestConfig,
    results: Vec<TestResult>,
}

impl TestRunner {
    pub fn new() -> Self {
        Self { config: TestConfig::default(), results: Vec::new() }
    }
    pub fn with_config(mut self, config: TestConfig) -> Self {
        self.config = config;
        self
    }
    pub fn run<F>(&mut self, name: &str, test: F)
    where
        F: FnOnce() -> Result<(), String>,
    {
        let start = Instant::now();
        let result = test();
        let duration = start.elapsed();
        let test_result = match result {
            Ok(_) => TestResult::pass(name, duration),
            Err(msg) => TestResult::fail(name, duration, &msg),
        };
        self.results.push(test_result);
    }
    pub fn results(&self) -> &[TestResult] {
        &self.results
    }
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }
    pub fn print_summary(&self) {
        println!("\nTest Summary:");
        println!("=============");
        println!("Total: {}", self.results.len());
        println!("Passed: {}", self.passed_count());
        println!("Failed: {}", self.failed_count());
        if self.failed_count() > 0 {
            println!("\nFailed tests:");
            for result in &self.results {
                if !result.passed {
                    println!("  - {}: {}", result.name, result.message.as_ref().unwrap());
                }
            }
        }
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance benchmark
pub struct Benchmark {
    name: String,
    iterations: u32,
}

impl Benchmark {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), iterations: 1000 }
    }
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }
    pub fn run<F, T>(&self, f: F) -> BenchmarkResult
    where
        F: Fn() -> T,
    {
        let mut times = Vec::with_capacity(self.iterations as usize);
        for _ in 0..self.iterations {
            let start = Instant::now();
            let _ = f();
            times.push(start.elapsed());
        }
        let total: Duration = times.iter().sum();
        let avg = total / self.iterations;
        let min = *times.iter().min().unwrap();
        let max = *times.iter().max().unwrap();
        BenchmarkResult {
            name: self.name.clone(),
            iterations: self.iterations,
            total,
            average: avg,
            min,
            max,
        }
    }
}

/// Performance benchmark result
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u32,
    pub total: Duration,
    pub average: Duration,
    pub min: Duration,
    pub max: Duration,
}

impl BenchmarkResult {
    pub fn print(&self) {
        println!("\nBenchmark: {}", self.name);
        println!("  Iterations: {}", self.iterations);
        println!("  Total: {:?}", self.total);
        println!("  Average: {:?}", self.average);
        println!("  Min: {:?}", self.min);
        println!("  Max: {:?}", self.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner() {
        let mut runner = TestRunner::new();
        runner.run("passing_test", || Ok(()));
        runner.run("failing_test", || Err("Test failed".to_string()));
        assert_eq!(runner.passed_count(), 1);
        assert_eq!(runner.failed_count(), 1);
    }

    #[test]
    fn test_benchmark() {
        let benchmark = Benchmark::new("test_benchmark").with_iterations(100);
        let result = benchmark.run(|| {
            let _ = 1 + 1;
        });
        assert_eq!(result.iterations, 100);
        assert!(result.total > Duration::ZERO);
    }
}
