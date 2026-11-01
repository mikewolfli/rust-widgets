// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The presentation state machine: what a failure means and how many times it may repeat.
//!
//! A window can stop accepting frames in several distinct ways, and they are not interchangeable:
//! a stale configuration is fixed by reconfiguring, a lost surface by recreating it, a busy
//! compositor by skipping a frame, and a lost device by no longer using the GPU for that window.
//! Treating them as one "error" either throws away a working surface or retries something that
//! cannot succeed.
//!
//! # Why the budgets exist
//!
//! "Reconfigure and try again" is the correct answer once; repeated forever it is a spin. Every
//! recovery path here therefore has a budget ([`PRESENT_RECOVERY_BUDGET`]), after which the next
//! step is escalation (reconfigure -> recreate -> stop). A successful present resets the budgets,
//! because it proves the last recovery worked — the budget is per incident, not per process.
//!
//! # Where the failures come from
//!
//! `wgpu` 30 reports an acquisition outcome as
//! [`CurrentSurfaceTexture`](https://docs.rs/wgpu/30/wgpu/enum.CurrentSurfaceTexture) rather than
//! the `SurfaceError` of earlier versions: `Lost`, `Outdated`, `Timeout`, `Occluded`,
//! `Validation`, and two success values. [`SurfaceFailure`] is this module's own vocabulary
//! because the same states also arrive from `configure` (an unsupported configuration, an
//! allocation failure) and from the device-lost observer, which have no acquisition enum at all.
//!
//! [`SurfaceFailure::OutOfMemory`] and [`SurfaceFailure::DeviceLost`] can therefore only be
//! produced by those two sources — a host reading this module should not expect the acquire path
//! to report them.

/// Why a presentation attempt did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFailure {
    /// The surface no longer names a valid target; it must be created again.
    Lost,
    /// The target changed shape and the configuration no longer matches it.
    Outdated,
    /// The compositor had no frame available within the acquire budget.
    Timeout,
    /// The window is not visible (minimised, fully covered, or on a hidden workspace).
    ///
    /// This is not a fault: nothing should be presented until the window is visible again.
    Occluded,
    /// Allocating the frame or the configuration failed.
    OutOfMemory,
    /// The device was lost; the GPU path cannot continue for this window.
    DeviceLost,
    /// A validation error was raised while acquiring the frame.
    Validation,
    /// Any other failure, reported without a more precise cause.
    Other,
}

impl SurfaceFailure {
    /// A stable name for logs and status output.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lost => "lost",
            Self::Outdated => "outdated",
            Self::Timeout => "timeout",
            Self::Occluded => "occluded",
            Self::OutOfMemory => "out-of-memory",
            Self::DeviceLost => "device-lost",
            Self::Validation => "validation",
            Self::Other => "other",
        }
    }

    /// Whether the failure says nothing about the surface's health.
    ///
    /// A timeout or an occluded window leaves the surface usable, so it must not consume the
    /// recovery budget that a reconfigure or recreate would.
    pub const fn is_transient(self) -> bool {
        matches!(self, Self::Timeout | Self::Occluded)
    }
}

/// What the presenter does after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Nothing needed repairing; carry on with this frame.
    Continue,
    /// Drop this frame and retry on the next one.
    SkipFrame,
    /// Apply the configuration again, then retry.
    Reconfigure,
    /// Create the presentation target again, then retry.
    Recreate,
    /// Stop presenting for this window and leave the host on its default delivery path.
    Stop,
}

impl RecoveryAction {
    /// The name used in logs.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::SkipFrame => "skip-frame",
            Self::Reconfigure => "reconfigure",
            Self::Recreate => "recreate",
            Self::Stop => "stop",
        }
    }
}

/// The semantic state of the last presentation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresentationStatus {
    /// No attempt has been made yet.
    #[default]
    Idle,
    /// The last frame was presented.
    Presented,
    /// The last attempt was dropped and will be retried.
    Skipped,
    /// The last attempt reconfigured the target.
    Reconfigured,
    /// The last attempt recreated the target.
    Recreated,
    /// The presenter stopped; the host is back on its default delivery path.
    Stopped,
    /// This host cannot present at all, so nothing was attempted.
    Unsupported,
}

impl PresentationStatus {
    /// A stable name for logs and status output.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Presented => "presented",
            Self::Skipped => "skipped",
            Self::Reconfigured => "reconfigured",
            Self::Recreated => "recreated",
            Self::Stopped => "stopped",
            Self::Unsupported => "unsupported",
        }
    }

    /// Whether the presenter is still able to present.
    pub const fn is_usable(self) -> bool {
        !matches!(self, Self::Stopped | Self::Unsupported)
    }
}

/// How many consecutive transient failures are tolerated before escalating.
///
/// Three frames is one frame at 60 Hz: a compositor that is a frame late recovers within the
/// budget, while a surface that times out persistently reaches the escalation instead of
/// skipping indefinitely.
pub const PRESENT_TIMEOUT_BUDGET: u32 = 3;

/// How many repairs of one kind are attempted before escalating to the next kind.
///
/// Reconfigure is tried this many times, then recreate this many times, then the presenter stops.
/// Two is enough for the real cases (a resize storm, a display moving between GPUs) and short
/// enough that a broken target fails within a handful of frames rather than seconds.
pub const PRESENT_RECOVERY_BUDGET: u32 = 2;

/// Failure handling and counters for one presentation target.
///
/// The type is deliberately independent of `wgpu`: the counters and the escalation order are the
/// part that needs tests, and they are testable without a device.
#[derive(Debug, Clone)]
pub struct PresentHealth {
    consecutive_transient: u32,
    reconfigures: u32,
    recreates: u32,
    present_count: u64,
    failure_count: u64,
    status: PresentationStatus,
}

impl Default for PresentHealth {
    fn default() -> Self {
        Self {
            consecutive_transient: 0,
            reconfigures: 0,
            recreates: 0,
            present_count: 0,
            failure_count: 0,
            status: PresentationStatus::Idle,
        }
    }
}

impl PresentHealth {
    /// A fresh, idle health record.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a successful present and resets the per-incident budgets.
    pub fn on_present(&mut self) {
        self.present_count += 1;
        self.consecutive_transient = 0;
        self.reconfigures = 0;
        self.recreates = 0;
        self.status = PresentationStatus::Presented;
    }

    /// Records a repair that succeeded without a frame (a reconfigure or a recreate).
    ///
    /// The counters are deliberately untouched: nothing was presented, so the incident is not
    /// over; only a real frame proves the repair worked. This exists so the presenter can report
    /// `Reconfigured`/`Recreated` as the outcome of a repair rather than leaving the last failure
    /// as the status.
    pub fn on_present_status(&mut self, status: PresentationStatus) {
        self.status = status;
    }

    /// Records a failure and answers what to do about it.
    ///
    /// The escalation is: transient failures skip until [`PRESENT_TIMEOUT_BUDGET`], then
    /// reconfigure up to [`PRESENT_RECOVERY_BUDGET`] times, then recreate up to the same number,
    /// then stop. A `DeviceLost` failure stops immediately because the GPU path for the whole
    /// window is gone — no surface repair can bring it back.
    pub fn on_failure(&mut self, failure: SurfaceFailure) -> RecoveryAction {
        self.failure_count += 1;
        let action = match failure {
            SurfaceFailure::Occluded => {
                // Not a fault: retrying is legitimate and must not spend the repair budget.
                self.status = PresentationStatus::Skipped;
                return RecoveryAction::SkipFrame;
            }
            SurfaceFailure::DeviceLost => {
                self.status = PresentationStatus::Stopped;
                return RecoveryAction::Stop;
            }
            SurfaceFailure::Outdated => self.reconfigure(),
            SurfaceFailure::Lost | SurfaceFailure::OutOfMemory => self.recreate(),
            SurfaceFailure::Timeout | SurfaceFailure::Validation | SurfaceFailure::Other => {
                self.consecutive_transient += 1;
                if self.consecutive_transient < PRESENT_TIMEOUT_BUDGET {
                    RecoveryAction::SkipFrame
                } else {
                    self.consecutive_transient = 0;
                    self.reconfigure()
                }
            }
        };
        self.status = match action {
            RecoveryAction::Continue => PresentationStatus::Idle,
            RecoveryAction::SkipFrame => PresentationStatus::Skipped,
            RecoveryAction::Reconfigure => PresentationStatus::Reconfigured,
            RecoveryAction::Recreate => PresentationStatus::Recreated,
            RecoveryAction::Stop => PresentationStatus::Stopped,
        };
        action
    }

    fn reconfigure(&mut self) -> RecoveryAction {
        if self.reconfigures < PRESENT_RECOVERY_BUDGET {
            self.reconfigures += 1;
            RecoveryAction::Reconfigure
        } else {
            self.recreate()
        }
    }

    fn recreate(&mut self) -> RecoveryAction {
        if self.recreates < PRESENT_RECOVERY_BUDGET {
            self.recreates += 1;
            RecoveryAction::Recreate
        } else {
            RecoveryAction::Stop
        }
    }

    /// The state of the last attempt.
    pub fn status(&self) -> PresentationStatus {
        self.status
    }

    /// Marks the target as permanently unusable by this host, for a reason other than a failure.
    pub fn mark_unsupported(&mut self) {
        self.status = PresentationStatus::Unsupported;
    }

    /// Consecutive transient failures seen so far, reset by a present or by escalation.
    pub fn consecutive_transient(&self) -> u32 {
        self.consecutive_transient
    }

    /// Reconfigures performed since the last successful present.
    pub fn reconfigures(&self) -> u32 {
        self.reconfigures
    }

    /// Recreates performed since the last successful present.
    pub fn recreates(&self) -> u32 {
        self.recreates
    }

    /// Frames presented since the target was created.
    pub fn present_count(&self) -> u64 {
        self.present_count
    }

    /// Failures seen since the target was created.
    pub fn failure_count(&self) -> u64 {
        self.failure_count
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PresentHealth, PresentationStatus, RecoveryAction, SurfaceFailure, PRESENT_RECOVERY_BUDGET,
        PRESENT_TIMEOUT_BUDGET,
    };

    #[test]
    fn a_fresh_record_is_idle_and_usable() {
        let health = PresentHealth::new();
        assert_eq!(health.status(), PresentationStatus::Idle);
        assert!(health.status().is_usable());
        assert_eq!(health.present_count(), 0);
    }

    #[test]
    fn outdated_reconfigures_then_recreates_then_stops() {
        let mut health = PresentHealth::new();
        for expected in 0..PRESENT_RECOVERY_BUDGET {
            assert_eq!(
                health.on_failure(SurfaceFailure::Outdated),
                RecoveryAction::Reconfigure,
                "reconfigure {expected}"
            );
        }
        for expected in 0..PRESENT_RECOVERY_BUDGET {
            assert_eq!(
                health.on_failure(SurfaceFailure::Outdated),
                RecoveryAction::Recreate,
                "recreate {expected}"
            );
        }
        assert_eq!(health.on_failure(SurfaceFailure::Outdated), RecoveryAction::Stop);
        assert_eq!(health.status(), PresentationStatus::Stopped);
        assert!(!health.status().is_usable());
    }

    #[test]
    fn lost_recreates_without_reconfiguring_first() {
        let mut health = PresentHealth::new();
        assert_eq!(health.on_failure(SurfaceFailure::Lost), RecoveryAction::Recreate);
        assert_eq!(health.status(), PresentationStatus::Recreated);
        assert_eq!(health.reconfigures(), 0);
    }

    #[test]
    fn a_device_loss_stops_immediately() {
        let mut health = PresentHealth::new();
        assert_eq!(health.on_failure(SurfaceFailure::DeviceLost), RecoveryAction::Stop);
        assert_eq!(health.status(), PresentationStatus::Stopped);
    }

    #[test]
    fn a_timeout_skips_then_escalates_to_a_reconfigure() {
        let mut health = PresentHealth::new();
        for _ in 0..PRESENT_TIMEOUT_BUDGET - 1 {
            assert_eq!(health.on_failure(SurfaceFailure::Timeout), RecoveryAction::SkipFrame);
        }
        assert_eq!(health.consecutive_transient(), PRESENT_TIMEOUT_BUDGET - 1);
        assert_eq!(health.on_failure(SurfaceFailure::Timeout), RecoveryAction::Reconfigure);
        assert_eq!(health.consecutive_transient(), 0, "escalation resets the transient streak");
        assert_eq!(health.status(), PresentationStatus::Reconfigured);
    }

    #[test]
    fn an_occluded_window_never_spends_the_repair_budget() {
        let mut health = PresentHealth::new();
        for _ in 0..50 {
            assert_eq!(health.on_failure(SurfaceFailure::Occluded), RecoveryAction::SkipFrame);
        }
        assert_eq!(health.reconfigures(), 0);
        assert_eq!(health.recreates(), 0);
        assert_eq!(health.status(), PresentationStatus::Skipped);
        assert!(health.status().is_usable());
    }

    #[test]
    fn a_present_resets_the_budgets() {
        let mut health = PresentHealth::new();
        let _ = health.on_failure(SurfaceFailure::Outdated);
        let _ = health.on_failure(SurfaceFailure::Outdated);
        assert_eq!(health.reconfigures(), PRESENT_RECOVERY_BUDGET);
        health.on_present();
        assert_eq!(health.status(), PresentationStatus::Presented);
        assert_eq!(health.reconfigures(), 0);
        assert_eq!(
            health.on_failure(SurfaceFailure::Outdated),
            RecoveryAction::Reconfigure,
            "a recovered target starts a new incident"
        );
        assert_eq!(health.present_count(), 1);
    }

    #[test]
    fn every_failure_is_counted() {
        let mut health = PresentHealth::new();
        let _ = health.on_failure(SurfaceFailure::Timeout);
        let _ = health.on_failure(SurfaceFailure::Validation);
        let _ = health.on_failure(SurfaceFailure::Other);
        assert_eq!(health.failure_count(), 3);
    }

    #[test]
    fn unsupported_is_not_usable_and_is_not_a_failure() {
        let mut health = PresentHealth::new();
        health.mark_unsupported();
        assert_eq!(health.status(), PresentationStatus::Unsupported);
        assert!(!health.status().is_usable());
        assert_eq!(health.failure_count(), 0);
    }

    #[test]
    fn every_failure_and_action_has_a_distinct_label() {
        let failures = [
            SurfaceFailure::Lost,
            SurfaceFailure::Outdated,
            SurfaceFailure::Timeout,
            SurfaceFailure::Occluded,
            SurfaceFailure::OutOfMemory,
            SurfaceFailure::DeviceLost,
            SurfaceFailure::Validation,
            SurfaceFailure::Other,
        ];
        for (index, failure) in failures.iter().enumerate() {
            assert!(!failure.label().is_empty());
            assert!(!failures[..index].iter().any(|other| other.label() == failure.label()));
        }
        let actions = [
            RecoveryAction::Continue,
            RecoveryAction::SkipFrame,
            RecoveryAction::Reconfigure,
            RecoveryAction::Recreate,
            RecoveryAction::Stop,
        ];
        for (index, action) in actions.iter().enumerate() {
            assert!(!actions[..index].iter().any(|other| other.label() == action.label()));
        }
        assert!(SurfaceFailure::Timeout.is_transient());
        assert!(!SurfaceFailure::Lost.is_transient());
    }
}
