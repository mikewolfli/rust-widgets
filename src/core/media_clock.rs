// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The **master clock** a media timeline is measured against, and the state it is in.
//!
//! # Why this lives in `core` and not in `video`
//!
//! It was written in `src/video/` and that was a mistake with a concrete cost: `src/video/` is gated
//! on the `video` feature (a real codec dependency), so a **core** control could not use the clock
//! without pulling a decoder in. But nothing here touches a codec — it is arithmetic over a delta and
//! four thresholds, and `PlaybackState` is a handful of variants. Moving both here is what lets
//! `MediaPlayer` (always built) and `VideoEngine` (behind `video`) share one timeline instead of each
//! carrying its own.
//!
//! # The gap this closes (BLUE24 §12 U-2)
//!
//! The crate shipped two media engines with **two unrelated timelines**:
//!
//! ```text
//! src/video/engine.rs   VideoEngine::next_frame()   pulls a frame, current_time = frame.timestamp
//!                                                    ↑ the *frame's own* presentation time
//! src/audio/engine.rs   AudioEngine::tick(samples)   pushes samples, position += samples
//!                                                    ↑ a count of *samples played*
//! media_player.rs       MediaPlayer::position_ms     advanced by nothing at all
//! ```
//!
//! None is wrong on its own. What was missing is the thing that relates them: a question none could
//! answer — *what time is it now, and is this frame early, due, or late?* Without it a player can
//! only "play frames as fast as they decode" (running ahead on a fast machine and behind on a slow
//! one) and audio can only "play samples as fast as the device consumes them". The two drift, and
//! nothing measures the drift because there is no shared reference to measure against.
//!
//! # Why it is driven by `drive_frame`
//!
//! A clock needs a time source, and the frame loop is the one the crate already owns:
//! [`crate::drive_frame`] hands every consumer the same `delta_ms` on every platform. So the clock is
//! **not** a wall-clock reader — it advances by the same delta the frame advanced animations by, which
//! means a paused window's still frames cost the clock nothing and a test can drive it by hand
//! (`tick(16)` sixty times is 0.96 s, deterministically, with no sleeping).
//!
//! # What it is not
//!
//! It does not decode, does not own a frame, and does not touch a device. It answers two questions —
//! *where are we* ([`MediaClock::position`]) and *what should happen to this frame*
//! ([`MediaClock::verdict_for`]) — and the engines act on the answers. Keeping the arithmetic here
//! and the I/O in the engines is what makes the sync policy testable without a video file or a sound
//! card.

/// Playback state for media engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// No media loaded or stopped.
    #[default]
    Stopped,
    /// Currently playing.
    Playing,
    /// Paused.
    Paused,
    /// Buffering/waiting for data.
    Buffering,
    /// Playback finished.
    Ended,
}

impl PlaybackState {
    /// Returns true if the player is actively playing.
    pub fn is_active(&self) -> bool {
        matches!(self, PlaybackState::Playing | PlaybackState::Buffering)
    }

    /// Returns true if playback can be resumed.
    pub fn can_resume(&self) -> bool {
        matches!(self, PlaybackState::Paused)
    }

    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            PlaybackState::Stopped => "Stopped",
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Buffering => "Buffering",
            PlaybackState::Ended => "Ended",
        }
    }
}

/// What should happen to a frame at the clock's current position.
///
/// # Why a verdict and not a boolean
///
/// "Should I show this?" has three answers, not two, and collapsing the third into either of the
/// others is what makes a player misbehave in a way that looks like a performance problem:
///
/// * returning `Show` for a **late** frame makes playback fall behind and never recover — the
///   classic "video slows down and the audio overtakes it" failure;
/// * returning `Wait` for a late frame is the same failure spelled differently, because waiting for a
///   frame that is already overdue is waiting for nothing;
/// * returning `Show` for an **early** frame makes playback run as fast as the decoder can go, which
///   is the failure mode a fast machine exhibits and a slow one hides.
///
/// Naming all three is what lets the loop state the policy it implements rather than infer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameVerdict {
    /// The frame is due now: hand it to the renderer.
    Show,
    /// The frame is not due yet: keep showing the previous one and ask again later.
    Wait,
    /// The frame is too far past due: discharge it without showing it.
    ///
    /// Dropping is not an optimisation — it is the *only* way to catch up after a stall. A player
    /// that never drops can only ever fall further behind, because the media's timeline is fixed and
    /// its own is not.
    Drop,
}

/// The master clock: one timeline, advanced by the frame loop.
///
/// See the module docs for why this exists and what it deliberately does not do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaClock {
    /// Where playback is, in seconds. Always in `0.0..=duration`.
    position: f64,
    /// How long the media is, in seconds. `0.0` means "unknown", which clamps nothing.
    duration: f64,
    /// Playback rate: `1.0` is real time, `2.0` is double speed.
    ///
    /// A non-finite or non-positive rate is refused by [`MediaClock::set_rate`] rather than stored,
    /// because a rate of `0` is a pause expressed as a rate and a negative rate would make
    /// [`MediaClock::position`] decrease while [`MediaClock::verdict_for`]'s window moves the wrong
    /// way — two mechanisms for one idea (the state has a `Paused` already).
    rate: f64,
    /// Whether the clock is running.
    state: PlaybackState,
    /// Frames dropped by [`MediaClock::verdict_for`] since the last [`MediaClock::take_dropped`].
    ///
    /// Counted rather than merely dropped so "is playback keeping up?" is answerable. A player that
    /// silently discards frames looks identical to one that is rendering all of them.
    dropped: u64,
}

impl Default for MediaClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaClock {
    /// A stopped clock at position `0`, with no known duration.
    pub fn new() -> Self {
        Self { position: 0.0, duration: 0.0, rate: 1.0, state: PlaybackState::Stopped, dropped: 0 }
    }

    /// States how long the media is, so [`MediaClock::position`] stops at the end.
    ///
    /// A non-finite or negative duration is treated as "unknown" rather than stored: a container
    /// that failed to parse yields `NaN`, and a clock that clamped against `NaN` would panic on every
    /// tick (the same hazard `VideoEngine::seek` documents for its own bound).
    ///
    /// Setting a duration also re-clamps the current position into `[0, duration]`, because the
    /// invariant "position is never beyond the duration" must hold as soon as the bound is known —
    /// otherwise a clock that played past a newly-discovered short duration would report a position
    /// the media does not have. Landing exactly on the new end reports `Ended`, for the same reason
    /// [`Self::seek`] does. Setting the duration to "unknown" (`0`) leaves the position alone, since
    /// there is no bound to clamp against.
    pub fn set_duration(&mut self, duration_secs: f64) {
        self.duration =
            if duration_secs.is_finite() && duration_secs > 0.0 { duration_secs } else { 0.0 };
        if self.duration > 0.0 {
            self.position = self.position.clamp(0.0, self.duration);
            if self.position >= self.duration {
                self.state = PlaybackState::Ended;
            }
        }
    }

    /// The media duration this clock was told about, or `0.0` when unknown.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// How fast the clock runs. `1.0` is real time.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Sets the playback rate, ignoring a value that is not finite and positive.
    ///
    /// Returns the rate actually in effect, so a caller that passed something unusable can see that
    /// nothing changed rather than assuming it did.
    pub fn set_rate(&mut self, rate: f64) -> f64 {
        if rate.is_finite() && rate > 0.0 {
            self.rate = rate;
        }
        self.rate
    }

    /// The current playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Starts the clock, or resumes it from a pause.
    ///
    /// Starting a clock whose media has ended rewinds it first: "play" on a finished track means
    /// "play it again", and leaving the position at the end would make the first tick immediately
    /// report `Ended` again — a play button that does nothing.
    pub fn play(&mut self) {
        if self.state == PlaybackState::Ended {
            self.position = 0.0;
            self.dropped = 0;
        }
        self.state = PlaybackState::Playing;
    }

    /// Pauses the clock where it is.
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    /// Stops the clock and rewinds it.
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.position = 0.0;
        self.dropped = 0;
    }

    /// Moves the position, clamped to the known range.
    ///
    /// The state is **not** changed: seeking while paused stays paused and seeking while playing
    /// stays playing, which is what a scrubber needs. Landing exactly on the end reports `Ended`,
    /// because that is what the position means.
    pub fn seek(&mut self, position_secs: f64) {
        if !position_secs.is_finite() {
            return;
        }
        let upper = if self.duration > 0.0 { self.duration } else { f64::MAX };
        self.position = position_secs.clamp(0.0, upper);
        if self.duration > 0.0 && self.position >= self.duration {
            self.state = PlaybackState::Ended;
        }
    }

    /// Advances the clock by one frame's delta, in milliseconds.
    ///
    /// Returns the position after advancing. A clock that is not `Playing` does not move: that is the
    /// whole of "pausing costs nothing", and it is also why a still window's frames do not make media
    /// time run on.
    ///
    /// Reaching the end sets [`PlaybackState::Ended`] and **stops** the position at the duration.
    /// `Ended` rather than a wrap: repeating needs a repeat mode the crate does not have, and
    /// inventing one here would be a second mechanism for a question the caller has not asked.
    ///
    /// With no known duration the position must still stay **finite**: a very large but accepted rate
    /// multiplied by a delta cannot be allowed to overflow to `Infinity`, because every later frame's
    /// [`Self::verdict_for`] would then be judged against an infinite clock and be dropped. The
    /// advance is computed in finite steps and torn down to `f64::MAX` if it would overflow.
    pub fn tick(&mut self, delta_ms: u32) -> f64 {
        if self.state != PlaybackState::Playing {
            return self.position;
        }
        let delta_secs = delta_ms as f64 / 1000.0;
        // `rate` is finite and positive (set_rate refuses anything else), but `delta_secs * rate` can
        // still overflow to +Infinity. Clamping the product into the finite f64 range keeps the
        // position arithmetic finite and preserves the ordering between advances.
        let advance = (delta_secs * self.rate).min(f64::MAX);
        self.position = (self.position + advance).min(f64::MAX);
        if self.duration > 0.0 && self.position >= self.duration {
            self.position = self.duration;
            self.state = PlaybackState::Ended;
        }
        self.position
    }

    /// The current position, in seconds.
    pub fn position(&self) -> f64 {
        self.position
    }

    /// Decides what should happen to a frame whose presentation time is `frame_pts`.
    ///
    /// # The policy, and why the tolerance is a parameter
    ///
    /// * `frame_pts` more than `tolerance` **ahead** of the clock → [`FrameVerdict::Wait`];
    /// * `frame_pts` more than `tolerance` **behind** the clock → [`FrameVerdict::Drop`];
    /// * otherwise → [`FrameVerdict::Show`].
    ///
    /// `tolerance` is the caller's, not a constant here, because it is the difference between a
    /// display that refreshes at 60 Hz (16 ms of slop is invisible) and one at 24 Hz (it is not), and
    /// because a test needs it to be small enough to be exact. A caller with no opinion passes half a
    /// frame's duration, which is the largest error that cannot be seen.
    pub fn verdict_for(&self, frame_pts: f64, tolerance: f64) -> FrameVerdict {
        if !frame_pts.is_finite() {
            // A frame with no usable timestamp cannot be placed on the timeline. Showing it would put
            // it wherever the loop happened to be; dropping it loses one frame of a broken stream.
            // Dropping is the honest choice — it is what the stream's own defect produces.
            return FrameVerdict::Drop;
        }
        let tolerance = if tolerance.is_finite() && tolerance >= 0.0 { tolerance } else { 0.0 };
        let lead = frame_pts - self.position;
        if lead > tolerance {
            FrameVerdict::Wait
        } else if -lead > tolerance {
            FrameVerdict::Drop
        } else {
            FrameVerdict::Show
        }
    }

    /// Records that a frame was dropped, so "is playback keeping up?" is answerable.
    pub fn note_dropped(&mut self) {
        self.dropped = self.dropped.saturating_add(1);
    }

    /// Returns the frames dropped since the last call, resetting the count.
    ///
    /// Taken rather than read, because the number a caller wants is *this period's* drops — and a
    /// counter that only grows makes "the last second dropped three frames" impossible to state.
    pub fn take_dropped(&mut self) -> u64 {
        core::mem::take(&mut self.dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock at 60 Hz for one second is exactly sixty ticks — the whole point of driving the clock
    /// from the frame loop instead of a wall clock: this is deterministic and needs no sleeping.
    #[test]
    fn the_clock_advances_by_the_frames_delta() {
        let mut clock = MediaClock::new();
        clock.set_duration(10.0);
        clock.play();
        for _ in 0..60 {
            clock.tick(16);
        }
        // 60 x 16 ms = 0.96 s, not 1.0 — the assertion is exact because the arithmetic is.
        assert!((clock.position() - 0.96).abs() < 1e-9, "position was {}", clock.position());
    }

    /// A paused clock does not move, whatever the frame loop does.
    #[test]
    fn a_paused_clock_does_not_advance() {
        let mut clock = MediaClock::new();
        clock.set_duration(10.0);
        clock.play();
        clock.tick(16);
        let paused_at = clock.position();
        clock.pause();
        for _ in 0..60 {
            clock.tick(16);
        }
        assert_eq!(clock.position(), paused_at, "a still frame must not run media time on");
        assert_eq!(clock.state(), PlaybackState::Paused);
    }

    /// Reaching the end stops at the duration and reports `Ended`.
    #[test]
    fn the_clock_stops_at_the_end_and_reports_ended() {
        let mut clock = MediaClock::new();
        clock.set_duration(1.0);
        clock.play();
        // A single long frame overshoots; the position must clamp, not run past the end.
        clock.tick(5_000);
        assert_eq!(clock.position(), 1.0, "the position clamps at the duration");
        assert_eq!(clock.state(), PlaybackState::Ended);
        // And a further tick is a no-op rather than a wrap.
        clock.tick(16);
        assert_eq!(clock.position(), 1.0);
    }

    /// Pressing play on a finished track starts it again.
    ///
    /// Without the rewind, `play()` after `Ended` leaves the position at the end, so the next tick
    /// immediately re-reports `Ended` — a play button that visibly does nothing.
    #[test]
    fn play_after_the_end_rewinds() {
        let mut clock = MediaClock::new();
        clock.set_duration(1.0);
        clock.play();
        clock.tick(2_000);
        assert_eq!(clock.state(), PlaybackState::Ended);
        clock.play();
        assert_eq!(clock.position(), 0.0, "play on a finished track means play it again");
        assert_eq!(clock.state(), PlaybackState::Playing);
    }

    /// The three verdicts, at the boundaries.
    #[test]
    fn a_frame_is_waited_for_shown_or_dropped_by_how_far_it_is_from_now() {
        let mut clock = MediaClock::new();
        clock.set_duration(10.0);
        clock.play();
        for _ in 0..25 {
            clock.tick(16); // 0.4 s in
        }
        assert!((clock.position() - 0.4).abs() < 1e-9);

        // Due now, and anything inside the tolerance either way.
        assert_eq!(clock.verdict_for(0.4, 0.008), FrameVerdict::Show);
        assert_eq!(clock.verdict_for(0.405, 0.008), FrameVerdict::Show);
        assert_eq!(clock.verdict_for(0.395, 0.008), FrameVerdict::Show);
        // Ahead of the clock: not due yet.
        assert_eq!(clock.verdict_for(0.5, 0.008), FrameVerdict::Wait);
        // Behind it: overdue, and waiting for it would be waiting for nothing.
        assert_eq!(clock.verdict_for(0.1, 0.008), FrameVerdict::Drop);
    }

    /// A frame with no usable timestamp is dropped rather than placed arbitrarily.
    #[test]
    fn an_unusable_timestamp_is_dropped() {
        let clock = MediaClock::new();
        assert_eq!(clock.verdict_for(f64::NAN, 0.008), FrameVerdict::Drop);
        assert_eq!(clock.verdict_for(f64::INFINITY, 0.008), FrameVerdict::Drop);
    }

    /// The rate scales the advance, and an unusable rate is refused rather than stored.
    #[test]
    fn the_rate_scales_the_advance_and_refuses_zero() {
        let mut clock = MediaClock::new();
        clock.set_duration(100.0);
        clock.play();
        assert_eq!(clock.set_rate(2.0), 2.0);
        clock.tick(1000);
        assert!((clock.position() - 2.0).abs() < 1e-9, "double speed is two seconds per second");

        // A rate of zero is a pause spelled as a rate, and a negative one would run the media
        // backwards; both are refused, and the caller is told what is in effect.
        assert_eq!(clock.set_rate(0.0), 2.0);
        assert_eq!(clock.set_rate(-1.0), 2.0);
        assert_eq!(clock.set_rate(f64::NAN), 2.0);
    }

    /// A duration that failed to parse must not poison the clock.
    #[test]
    fn an_unusable_duration_clamps_nothing_and_does_not_panic() {
        let mut clock = MediaClock::new();
        clock.set_duration(f64::NAN);
        assert_eq!(clock.duration(), 0.0, "an unknown duration is not a NaN bound");
        clock.play();
        clock.tick(1000);
        assert!(
            (clock.position() - 1.0).abs() < 1e-9,
            "with no duration the clock runs on rather than clamping against a NaN"
        );
    }

    /// Dropped frames are counted, and the count is taken rather than read.
    #[test]
    fn drops_are_counted_and_taken() {
        let mut clock = MediaClock::new();
        clock.note_dropped();
        clock.note_dropped();
        assert_eq!(clock.take_dropped(), 2, "this period dropped two");
        assert_eq!(clock.take_dropped(), 0, "and the count was taken, not read");
    }

    /// Seeking is clamped, keeps the state, and landing on the end reports `Ended`.
    #[test]
    fn seek_is_clamped_and_keeps_the_state() {
        let mut clock = MediaClock::new();
        clock.set_duration(10.0);
        clock.play();
        clock.seek(5.0);
        assert_eq!(clock.position(), 5.0);
        assert_eq!(clock.state(), PlaybackState::Playing, "seeking does not pause");

        clock.seek(-3.0);
        assert_eq!(clock.position(), 0.0, "clamped to the start");
        clock.seek(99.0);
        assert_eq!(clock.position(), 10.0, "clamped to the end");
        assert_eq!(clock.state(), PlaybackState::Ended, "landing on the end is the end");

        // A NaN seek is ignored rather than poisoning the position.
        clock.play();
        clock.seek(4.0);
        clock.seek(f64::NAN);
        assert_eq!(clock.position(), 4.0);
    }

    /// Discovering a shorter duration than the position must clamp the position into `[0, duration]`,
    /// so the invariant `position <= duration` holds from the moment the bound is known — including
    /// while paused, where no tick would otherwise fix it up.
    #[test]
    fn setting_a_duration_clamps_the_position_into_range() {
        // Learn the duration only after playing past it: the classic "parse finished late" case.
        let mut clock = MediaClock::new();
        clock.play();
        clock.tick(10_000);
        assert_eq!(clock.position(), 10.0, "unknown duration lets the clock run on");
        clock.pause();
        clock.set_duration(2.0);
        assert_eq!(clock.position(), 2.0, "the position must clamp to the newly-known duration");
        assert!(clock.position() <= clock.duration(), "position <= duration is the invariant");
        assert_eq!(clock.state(), PlaybackState::Ended, "landing on the end is the end");

        // The paused tick that previously returned the stale out-of-range position now returns the
        // clamped one.
        assert_eq!(clock.tick(16), 2.0);

        // A longer duration than the position leaves it untouched, in any state.
        let mut running = MediaClock::new();
        running.set_duration(10.0);
        running.play();
        running.tick(1_000);
        running.set_duration(30.0);
        assert_eq!(running.position(), 1.0);
        assert_eq!(running.state(), PlaybackState::Playing, "a longer duration does not end it");

        // A stopped clock that discovers a short duration is also clamped.
        let mut stopped = MediaClock::new();
        stopped.set_duration(5.0);
        stopped.play();
        stopped.tick(4_000);
        stopped.stop();
        stopped.set_duration(1.0);
        assert_eq!(stopped.position(), 0.0, "stop already rewound, so nothing to clamp");
    }

    /// A large but accepted rate must not make an unknown-duration clock reach `Infinity`: every
    /// downstream verdict would then judge a finite frame as overdue and drop it. The position must
    /// stay finite across repeated ticks.
    #[test]
    fn an_extreme_rate_keeps_an_unknown_duration_clock_finite() {
        let mut clock = MediaClock::new();
        clock.play();
        assert_eq!(clock.set_rate(f64::MAX), f64::MAX);
        clock.tick(2_000);
        assert!(clock.position().is_finite(), "position went non-finite: {}", clock.position());
        for _ in 0..1000 {
            clock.tick(2_000);
        }
        assert!(clock.position().is_finite(), "repeated ticks overflowed to {}", clock.position());
        // And a finite frame near the (finite) clock is still shown rather than dropped.
        assert_eq!(clock.verdict_for(clock.position(), 0.008), FrameVerdict::Show);
    }
}
