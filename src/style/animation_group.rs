// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Named animation group that can hold both parallel and sequential
//! animation sets together (BLUE11 R6.4).
//!
//! # Reachability (issue §9 item #5)
//!
//! This is a **retained public capability with no in-crate control consumer**. The engine it drives
//! ([`AnimationDriver`](crate::style::AnimationDriver)) *is* on the production path — controls reach
//! it through [`PropertyDriver`](crate::style::PropertyDriver) / [`Transition`](crate::style::Transition)
//! — but a *named timeline group* is a different feature: it composes `AnimationGroup`'s parallel
//! and sequential sets by name, which no control's property transition needs. Keeping it exported
//! and tested (rather than deleting it) is the explicit decision the issue asked for: a host that
//! wants timeline choreography drives it from its own frame loop, as the example below shows.

use super::animation::{AnimationConfig, AnimationDriver, AnimationId};
use crate::compat::{String, Vec};

/// A named group of animations that can contain both parallel and sequential sets.
///
/// Parallel animations all run at the same time; sequential animations run
/// one after another. The group is considered "completed" when both its
/// parallel set is done and its sequential queue has been exhausted.
///
/// # Example
///
/// ```text
/// let mut group = AnimationGroup::new("fade-in-slide");
/// group.add_parallel(fade_id);
/// group.add_parallel(scale_id);
/// group.add_sequential(AnimationConfig::new(300));
/// group.add_sequential(AnimationConfig::new(200));
///
/// // Each frame:
/// if !group.is_completed(&driver) {
///     group.advance(&mut driver);
///     driver.advance();
/// }
/// ```
pub struct AnimationGroup {
    /// User-facing name for debugging / profiling.
    name: String,
    /// Parallel animation IDs (all run concurrently).
    parallel: Vec<AnimationId>,
    /// Sequential animation configs (run one after another).
    sequential: Vec<AnimationConfig>,
    /// Index into `sequential` for the currently running config.
    current_seq_index: usize,
    /// The animation ID of the currently running sequential animation
    /// (if one has been started in the driver).
    current_seq_id: Option<AnimationId>,
}

impl AnimationGroup {
    /// Creates a new empty animation group with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parallel: Vec::new(),
            sequential: Vec::new(),
            current_seq_index: 0,
            current_seq_id: None,
        }
    }

    /// Add an animation ID to the parallel set.
    pub fn add_parallel(&mut self, id: AnimationId) {
        self.parallel.push(id);
    }

    /// Add an animation configuration to the sequential queue.
    pub fn add_sequential(&mut self, config: AnimationConfig) {
        self.sequential.push(config);
    }

    /// Returns the group's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns `true` when all parallel animations have completed **and**
    /// all sequential animations have been consumed.
    pub fn is_completed(&self, driver: &AnimationDriver) -> bool {
        let parallel_done = self.parallel.iter().all(|id| driver.is_finished(*id));
        parallel_done && self.current_seq_index >= self.sequential.len()
    }

    /// Returns the number of parallel animation IDs.
    pub fn len_parallel(&self) -> usize {
        self.parallel.len()
    }

    /// Returns the number of sequential animation configs.
    pub fn len_sequential(&self) -> usize {
        self.sequential.len()
    }

    /// Returns the current sequential index (how many have been consumed).
    pub fn current_seq_index(&self) -> usize {
        self.current_seq_index
    }

    /// Reset the sequential index back to 0 (does not remove parallel IDs).
    pub fn reset(&mut self) {
        self.current_seq_index = 0;
        self.current_seq_id = None;
    }

    /// Advance the sequential index when the current sequential animation completes.
    ///
    /// Call this each frame while the group is running. Once parallel animations
    /// are all finished, each sequential animation config is added to the driver
    /// and run to completion before advancing to the next one. When
    /// `current_seq_index` reaches `sequential.len()`,
    /// [`is_completed`](AnimationGroup::is_completed) returns `true`.
    pub fn advance(&mut self, driver: &mut AnimationDriver) {
        if self.current_seq_index >= self.sequential.len() {
            return;
        }
        // Only advance sequential animations after parallel animations complete
        let parallel_done = self.parallel.iter().all(|id| driver.is_finished(*id));
        if !parallel_done {
            return;
        }

        // If no sequential animation is running yet, start the current one
        if self.current_seq_id.is_none() {
            let config = self.sequential[self.current_seq_index].clone();
            let id = driver.add(config, |_| {});
            self.current_seq_id = Some(id);
            return;
        }

        // Check if the current sequential animation has completed
        let done = self.current_seq_id.map(|id| driver.is_finished(id)).unwrap_or(true);

        if done {
            self.current_seq_id = None;
            self.current_seq_index += 1;

            // Start the next sequential animation if available
            if self.current_seq_index < self.sequential.len() {
                let config = self.sequential[self.current_seq_index].clone();
                let id = driver.add(config, |_| {});
                self.current_seq_id = Some(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::animation::AnimationDirection;
    use core::time::Duration;

    /// The named group's parallel half must use lifecycle completion, not `progress >= 1.0`,
    /// so a reverse child is not treated as complete at its start.
    #[test]
    fn a_reverse_parallel_child_does_not_complete_the_group() {
        let mut driver = AnimationDriver::new();
        let mut group = AnimationGroup::new("reverse");
        let id = driver.add(
            AnimationConfig::new(Duration::from_millis(100))
                .with_direction(AnimationDirection::Reverse),
            |_| {},
        );
        group.add_parallel(id);

        assert!(!group.is_completed(&driver), "a reverse child starts at 1.0 but has not finished");

        driver.advance_by(Duration::from_millis(10_000));
        assert!(group.is_completed(&driver), "once the child finishes the group completes");
    }

    /// The sequential half waits for a multi-iteration child instead of advancing early.
    #[test]
    fn the_group_waits_for_a_multi_iteration_sequential_child() {
        let mut driver = AnimationDriver::new();
        let mut group = AnimationGroup::new("multi");
        group.add_sequential(AnimationConfig::new(Duration::from_millis(100)).with_iterations(3));
        group.add_sequential(AnimationConfig::new(Duration::from_millis(100)));

        // Start the first sequential child.
        group.advance(&mut driver);
        assert_eq!(group.current_seq_index(), 0);

        // Advance past one iteration but not all three: the child is not finished.
        driver.advance_by(Duration::from_millis(150));
        group.advance(&mut driver);
        assert_eq!(
            group.current_seq_index(),
            0,
            "a 3-iteration child at 1.5 iterations must not advance the sequence"
        );

        // Finish the child, then the sequence moves on.
        driver.advance_by(Duration::from_millis(10_000));
        group.advance(&mut driver);
        assert_eq!(group.current_seq_index(), 1, "the finished child advances the sequence");
    }
}
