// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Basic widget types: buttons, labels, checkboxes, etc.
pub mod button;
pub mod button_group;
pub mod checkbox;
pub mod frame;
pub mod label;
/// Shared text-measurement cache used by the base, overlay and navigation families (G-A02).
pub(crate) mod measure_cache;
pub mod radiobutton;
#[cfg(widgets_unstripped)]
pub mod toggle_button;
// Re-export widget types
pub use button::{Button, ButtonState};
pub use button_group::ButtonGroup;
pub use checkbox::{CheckBox, CheckState};
pub use frame::{Frame, FrameShadow, FrameShape};
pub use label::{InlineLink, Label, TextFormat};
pub use radiobutton::RadioButton;
#[cfg(widgets_unstripped)]
pub use toggle_button::{ToggleButton, ToggleButtonState};

/// The auto-repeat timing shared by every press-activated base control.
///
/// # Why one type rather than a field per control
///
/// [`Button`], [`CheckBox`], [`RadioButton`] and [`ToggleButton`] all answer the same question —
/// "how long has this been held, and how many repeats are due?" — and four private copies of the
/// arithmetic would drift the way every duplicated derivation in this crate has. `SpinBox` already
/// carried its own `held_key`/`repeat_elapsed` pair for the same reason; this is that mechanism
/// lifted out of one control instead of copied into four more.
///
/// # The clock
///
/// A control advances the timer from its existing `Widget::tick`, which the host's animation bus
/// already drives. There is no thread and no platform timer: a control that is not being ticked
/// simply does not repeat, which is the same contract every other animated control in the crate
/// follows.
///
/// # Semantics
///
/// The reference toolkit's `autoRepeatDelay`/`autoRepeatInterval`: a press produces its first
/// activation immediately (the control's own event path does that), then a repeat every
/// `interval_ms` once the press has been held for `delay_ms`. Releasing or disabling the control
/// ends the sequence at once.
///
/// Off by default, so a control that has never opted in behaves exactly as it did before: one
/// press, one activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoRepeat {
    enabled: bool,
    delay_ms: u32,
    interval_ms: u32,
    /// Milliseconds accumulated since the hold started; `None` while the control is not held.
    ///
    /// `Some(0)` means "the hold began on this tick", which is what keeps the frame that reported
    /// the press from contributing its own delta to the delay.
    elapsed_ms: Option<u32>,
}

impl AutoRepeat {
    /// The delay before the first repeat, in milliseconds.
    pub(crate) const DEFAULT_DELAY_MS: u32 = 500;

    /// The gap between repeats once they start, in milliseconds.
    pub(crate) const DEFAULT_INTERVAL_MS: u32 = 100;

    /// The most repeats one tick reports.
    ///
    /// A frame that arrives very late — a suspended window, a debugger pause — would otherwise
    /// replay minutes of repeats at once. Reporting a bounded number keeps a single frame's work
    /// finite, and the backlog is dropped rather than queued: a hundred activations delivered in
    /// one frame is not a user interaction.
    const MAX_REPEATS_PER_TICK: u32 = 8;

    /// A disabled, default-tempo timer: the state every control starts in.
    pub(crate) const fn new() -> Self {
        Self {
            enabled: false,
            delay_ms: Self::DEFAULT_DELAY_MS,
            interval_ms: Self::DEFAULT_INTERVAL_MS,
            elapsed_ms: None,
        }
    }

    /// Whether the control repeats while held.
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables repeating. Disabling abandons a hold that is in progress.
    pub(crate) fn set_enabled(&mut self, enabled: bool) -> bool {
        if self.enabled == enabled {
            return false;
        }
        self.enabled = enabled;
        if !enabled {
            self.elapsed_ms = None;
        }
        true
    }

    /// How long a press must be held before the first repeat.
    pub(crate) fn delay_ms(&self) -> u32 {
        self.delay_ms
    }

    /// Sets the delay before the first repeat.
    pub(crate) fn set_delay_ms(&mut self, delay_ms: u32) -> bool {
        if self.delay_ms == delay_ms {
            return false;
        }
        self.delay_ms = delay_ms;
        true
    }

    /// The gap between repeats.
    pub(crate) fn interval_ms(&self) -> u32 {
        self.interval_ms
    }

    /// Sets the gap between repeats. Zero repeats at most once per tick.
    pub(crate) fn set_interval_ms(&mut self, interval_ms: u32) -> bool {
        if self.interval_ms == interval_ms {
            return false;
        }
        self.interval_ms = interval_ms;
        true
    }

    /// Abandons a hold, so the next press starts its delay from zero.
    pub(crate) fn reset(&mut self) {
        self.elapsed_ms = None;
    }

    /// Advances the hold by `delta_ms`, returning how many repeats are due.
    ///
    /// `held` is the control's own answer to "is the pointer still down on an enabled control?".
    /// `0` means either the control is not repeating, is not held, or has not yet reached its
    /// delay.
    ///
    /// Named `advance`, not `tick`: this counts **auto-repeat** clicks, it is not an animation
    /// tick, and `check_animation_has_a_driver.sh` scans per file for an animation-shaped
    /// `tick` without a `Widget::tick` bridge. The old name made this helper file read as an
    /// undispatched animation.
    pub(crate) fn advance(&mut self, delta_ms: u32, held: bool) -> u32 {
        if !self.enabled || !held {
            self.elapsed_ms = None;
            return 0;
        }
        // Read, compute and write back rather than holding a `&mut` into the field: the field is
        // `Copy`, so this keeps the borrow out of the early-return paths entirely.
        let Some(previous) = self.elapsed_ms else {
            // The first tick observes the press. It starts the clock and reports nothing, so the
            // delay is measured from the press rather than shortened by the press frame's delta.
            self.elapsed_ms = Some(0);
            return 0;
        };
        let elapsed = previous.saturating_add(delta_ms);
        if elapsed < self.delay_ms {
            self.elapsed_ms = Some(elapsed);
            return 0;
        }
        let overdue = elapsed - self.delay_ms;
        let (due, remainder) = match self.interval_ms {
            // A zero interval means "as fast as the host ticks": one repeat per frame, and the
            // clock stays at the delay so the next frame can owe another.
            0 => (1, 0),
            interval => (
                overdue.checked_div(interval).unwrap_or(0) + 1,
                overdue.checked_rem(interval).unwrap_or(0),
            ),
        };
        // Consume the repeats that are being reported (bounded) and carry the rest of the
        // interval into the next frame, so a steady tick rate does not drift.
        self.elapsed_ms = Some(self.delay_ms.saturating_add(remainder));
        due.min(Self::MAX_REPEATS_PER_TICK)
    }
}

impl Default for AutoRepeat {
    fn default() -> Self {
        Self::new()
    }
}
