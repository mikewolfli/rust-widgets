// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Test infrastructure types and utilities.

use std::time::{Duration, Instant};

/// Test configuration
///
/// Bundles the settings a [`TestRunner`] uses. The struct is plain data with
/// public fields, so a caller can adjust one value with struct-update syntax
/// (`TestConfig { tolerance: 0.0, ..Default::default() }`) and keep the rest of
/// the defaults.
///
/// These are conventions for the crate's own test helpers, not something the
/// build system enforces: `timeout` in particular is *not* applied by
/// [`TestRunner::run`], which never interrupts a test that overruns.
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Wall-clock budget a single test is expected to stay within. Defaults to
    /// five seconds; currently advisory only, since nothing enforces it.
    pub timeout: Duration,
    /// Directory image baselines are stored in, conventionally relative to the
    /// crate root. Defaults to `"tests/snapshots"`, matching
    /// [`SnapshotManager::default`].
    pub snapshot_dir: String,
    /// Allowed image difference, in percent of differing pixels. Defaults to
    /// `0.01`, matching [`SnapshotManager::new`].
    pub tolerance: f32,
}

/// Defaults matching the rest of the test utilities: a five-second timeout, the
/// `tests/snapshots` baseline directory, and a `0.01` percent pixel tolerance.
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
///
/// One record per test that a [`TestRunner`] has run, capturing the outcome,
/// how long it took, and the failure message when there is one. Ordering in
/// [`TestRunner::results`] is the order the tests were run.
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Name the test was run under, as passed to [`TestRunner::run`].
    pub name: String,
    /// Whether the test returned `Ok`.
    pub passed: bool,
    /// Measured wall-clock duration of the test body, excluding reporting.
    pub duration: Duration,
    /// Failure message; `None` when the test passed.
    pub message: Option<String>,
}

impl TestResult {
    /// Builds a passing result with no message.
    pub fn pass(name: &str, duration: Duration) -> Self {
        Self { name: name.to_string(), passed: true, duration, message: None }
    }
    /// Builds a failing result carrying `message` for reporting.
    pub fn fail(name: &str, duration: Duration, message: &str) -> Self {
        Self { name: name.to_string(), passed: false, duration, message: Some(message.to_string()) }
    }
}

/// Test runner
///
/// A small, self-contained runner meant for the crate's own test binaries and
/// examples rather than a replacement for `#[test]`: it collects results in
/// memory and prints a summary, but performs no filtering, parallel execution,
/// or isolation. Its main use is producing a readable report from a set of
/// closure-based checks.
#[derive(Debug, Clone)]
pub struct TestRunner {
    config: TestConfig,
    results: Vec<TestResult>,
}

impl TestRunner {
    /// Creates a runner with no results and the default [`TestConfig`].
    pub fn new() -> Self {
        Self { config: TestConfig::default(), results: Vec::new() }
    }
    /// Builder-style setter for the runner's configuration.
    ///
    /// The config is stored for inspection; note that neither `timeout` nor the
    /// snapshot settings are currently consulted by [`TestRunner::run`].
    pub fn with_config(mut self, config: TestConfig) -> Self {
        self.config = config;
        self
    }
    /// Runs `test`, timing it, and records the outcome under `name`.
    ///
    /// The test signals success by returning `Ok(())` and failure by returning
    /// `Err(message)`; a panic is not caught and aborts the run. The closure
    /// runs exactly once and its result is appended to
    /// [`TestRunner::results`].
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
    /// Returns every recorded result, in execution order.
    pub fn results(&self) -> &[TestResult] {
        &self.results
    }
    /// Returns how many recorded results passed.
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }
    /// Returns how many recorded results failed.
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }
    /// Prints a human-readable summary to standard output.
    ///
    /// The summary lists totals, and when anything failed it also lists each
    /// failure with its message. This is intended for a test binary's final
    /// report; nothing is returned and no exit status is set, so the caller is
    /// responsible for failing the process itself.
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

crate::impl_default_via_new!(TestRunner);

/// Performance benchmark
///
/// Times a closure over a fixed number of iterations and summarises the
/// distribution. It is a coarse measurement tool for the crate's own checks;
/// there is no warm-up, no statistical analysis, and no protection against the
/// optimiser removing the work the closure performs.
#[derive(Debug, Clone)]
pub struct Benchmark {
    name: String,
    iterations: u32,
}

impl Benchmark {
    /// Creates a benchmark named `name` that will run 1000 iterations.
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), iterations: 1000 }
    }
    /// Builder-style setter for the iteration count.
    ///
    /// Setting this to `0` is not rejected here, but it will divide by zero
    /// when the benchmark is run.
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }
    /// Times `f` once per iteration and returns the aggregate statistics.
    ///
    /// `f` takes no arguments and its return value is discarded, so it can be
    /// any closure, typically one that repeatedly produces the value under test.
    /// The closure is called [`Benchmark::with_iterations`] times, without warm
    /// up and on the calling thread.
    ///
    /// # Panics
    ///
    /// Panics with a divide-by-zero if the iteration count is `0`, and on the
    /// `min`/`max` aggregation if it is `0` (there would be no samples to
    /// aggregate).
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
///
/// Per-iteration timing statistics for a single [`Benchmark::run`], reported as
/// [`Duration`] values in the clock's native resolution. The `average` is the
/// mean duration (`total / iterations`), not a median, so a single slow
/// iteration skews it.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Name copied from the originating benchmark.
    pub name: String,
    /// Number of iterations actually timed.
    pub iterations: u32,
    /// Sum of all iteration durations.
    pub total: Duration,
    /// Mean iteration duration, i.e. `total / iterations`.
    pub average: Duration,
    /// Shortest iteration duration observed.
    pub min: Duration,
    /// Longest iteration duration observed.
    pub max: Duration,
}

impl BenchmarkResult {
    /// Prints the statistics to standard output.
    ///
    /// Durations are formatted with their `Debug` representation, which retains
    /// the sub-unit breakdown.
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
