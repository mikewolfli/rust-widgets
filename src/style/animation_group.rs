//! Named animation group that can hold both parallel and sequential
//! animation sets together (BLUE11 R6.4).

use super::animation::{AnimationConfig, AnimationDriver, AnimationId};

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
        let parallel_done = self
            .parallel
            .iter()
            .all(|id| driver.get_progress(*id).map(|p| p >= 1.0).unwrap_or(true));
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
        let parallel_done = self
            .parallel
            .iter()
            .all(|id| driver.get_progress(*id).map(|p| p >= 1.0).unwrap_or(true));
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
        let done = self
            .current_seq_id
            .and_then(|id| driver.get_progress(id))
            .map(|p| p >= 1.0)
            .unwrap_or(true);

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
