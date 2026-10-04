// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::{FrameVerdict, MediaClock};
use crate::signal::Signal;
use crate::video::decoder::{MjpegDecoder, VideoDecoder};
use crate::video::format::{self, ContainerFormat};
use crate::video::frame::VideoFrame;
use crate::video::metadata::VideoMetadata;
use crate::video::player::PlaybackState;
use crate::widget::numeric::ordered_clamp_f64;

#[cfg(feature = "video-codecs")]
use crate::video::ffmpeg_decoder::FfmpegDecoder;

/// Video player engine that wraps a decoder and exposes playback control.
pub struct VideoEngine {
    decoder: Box<dyn VideoDecoder + Send>,
    metadata: VideoMetadata,
    state: PlaybackState,
    current_time: f64,
    /// The master timeline frames are judged against (BLUE24 §12 U-2).
    ///
    /// # Why the engine owns one rather than reading `current_time`
    ///
    /// `current_time` is where the *last decoded frame* was, which is a fact about the decoder. A
    /// clock is where *playback* is, which is a fact about time passing. They differ by exactly the
    /// amount a player is early or late, and an engine that had only the first could not tell the
    /// two apart — every frame was "on time" by definition, because nothing was measuring.
    clock: MediaClock,
    /// The frame that has been decoded but is not due to be shown yet.
    ///
    /// Held rather than discarded so a tick during which nothing is due costs a comparison instead of
    /// a decode. See [`VideoEngine::tick_frame`] for why reading one frame per tick is wrong.
    pending: Option<VideoFrame>,
    /// Emitted when a new frame is decoded.
    pub on_frame: Signal<VideoFrame>,
    /// Emitted when playback state changes.
    pub on_state_change: Signal<PlaybackState>,
}

impl VideoEngine {
    /// Open a video file from raw bytes. Detects container format automatically.
    ///
    /// When the `video-codecs` feature is enabled, non-MJPEG formats are
    /// decoded via FFmpeg. Without the feature, non-MJPEG input returns an
    /// explicit error instead of producing synthetic frames.
    pub fn open(data: Vec<u8>) -> Result<Self, String> {
        let format = format::detect_container_format(&data);
        let decoder: Box<dyn VideoDecoder + Send> = match format {
            ContainerFormat::Unknown => {
                return Err(format!(
                    "unknown video container format: the {} byte header matches no known \
                     signature (MJPEG and the ffmpeg container set were checked); pass a \
                     supported MP4/MOV/AVI/MKV/MJPEG stream",
                    data.len()
                ));
            }
            ContainerFormat::Mjpeg => Box::new(MjpegDecoder::new(data, format)),
            #[cfg(feature = "video-codecs")]
            _ => Box::new(FfmpegDecoder::new(data)?),
            #[cfg(not(feature = "video-codecs"))]
            _ => {
                return Err(format!("decoding {:?} requires the `video-codecs` feature", format));
            }
        };
        Ok(Self::with_decoder(decoder))
    }

    /// Returns the video metadata.
    pub fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }

    /// Returns the current playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Returns the current playback time in seconds.
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// Start or resume playback.
    ///
    /// A finished stream is replayed from its first frame: the clock rewinding alone is not
    /// enough, because the decoder is still parked at end-of-stream and the next tick would
    /// report `Ended` again immediately.
    pub fn play(&mut self) {
        if self.state == PlaybackState::Ended {
            if let Err(e) = self.decoder.seek(0.0) {
                log::error!("[video-engine] failed to rewind decoder on replay: {e}");
                return;
            }
            self.current_time = 0.0;
            self.pending = None;
            // The clock may be `Paused` (end-of-stream reached through `read_frame`) rather than
            // `Ended` (reached through the duration), and [`MediaClock::play`] only rewinds the
            // latter. Reset the position explicitly so replay starts at the first frame either way.
            self.clock.seek(0.0);
        }
        self.state = PlaybackState::Playing;
        self.clock.play();
        self.on_state_change.emit(self.state);
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
        self.clock.pause();
        self.on_state_change.emit(self.state);
    }

    /// Stop playback and reset to beginning.
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.current_time = 0.0;
        self.pending = None;
        self.clock.stop();
        // The clock rewound but the decoder did not; without this a stop followed by play would
        // resume from where the decoder left off rather than from the first frame.
        if let Err(e) = self.decoder.seek(0.0) {
            log::error!("[video-engine] failed to rewind decoder on stop: {e}");
        }
        self.on_state_change.emit(self.state);
    }

    /// Seek to a specific time in seconds.
    ///
    /// The clamp uses [`ordered_clamp_f64`] rather than `f64::clamp` because
    /// `metadata.duration` is a public field a host fills from its own container
    /// parser. A `NaN` duration — what a malformed or truncated container yields —
    /// makes `time.clamp(0.0, duration)` **panic**, which would abort the host
    /// process on a `seek` call rather than reporting a decode failure. The helper
    /// ignores the unusable bound and clamps only against the floor.
    pub fn seek(&mut self, time: f64) -> Result<(), String> {
        let time = ordered_clamp_f64(time, 0.0, self.metadata.duration);
        self.decoder.seek(time)?;
        self.current_time = time;
        // A frame decoded before the seek is a frame from where playback *was*; showing it after the
        // seek would put one stale picture in the middle of the jump.
        self.pending = None;
        // The clock moves with the decoder. If they disagreed, the first frame after a seek would be
        // judged against the pre-seek position and dropped as "overdue" — a seek that stutters.
        self.clock.seek(time);
        Ok(())
    }

    /// The master timeline, so a caller can read the position or state the rate.
    pub fn clock(&self) -> &MediaClock {
        &self.clock
    }

    /// Wraps an already-constructed decoder.
    ///
    /// # Why this is public and not test-only
    ///
    /// [`VideoEngine::open`] picks a decoder from the container's signature, which is the right
    /// production policy and also makes the engine **untestable without a real video file**. This
    /// constructor is the seam: a caller with its own decoder — an app's hardware decoder, or the
    /// crate's own [`crate::video::decoder::FrameBufferDecoder`] — gets the same engine, and the sync
    /// policy can be tested against a source that emits exact presentation times.
    ///
    /// Before it existed, `FrameBufferDecoder` had **no caller anywhere in the crate**: a
    /// deterministic decoder written for tests that no test could reach, because the only way into the
    /// engine was a container signature it does not produce.
    pub fn with_decoder(decoder: Box<dyn VideoDecoder + Send>) -> Self {
        let metadata = decoder.metadata().clone();
        let mut clock = MediaClock::new();
        clock.set_duration(metadata.duration);
        Self {
            decoder,
            metadata,
            state: PlaybackState::Stopped,
            current_time: 0.0,
            clock,
            pending: None,
            on_frame: Signal::new(),
            on_state_change: Signal::new(),
        }
    }

    /// The master timeline, mutably — for the rate and for seeking it directly.
    pub fn clock_mut(&mut self) -> &mut MediaClock {
        &mut self.clock
    }

    /// Advances the master clock by one frame's delta, then returns the frame that should be shown.
    ///
    /// # What this replaces (BLUE24 §12 U-2)
    ///
    /// [`VideoEngine::next_frame`] decodes and hands back whatever came out, so playback runs as fast
    /// as the decoder can go and neither late nor early frames are noticed. This is the synchronous
    /// form:
    ///
    /// * the clock advances by the frame loop's delta (so a paused window costs nothing);
    /// * a frame **not yet due** is put back and the previous one stays on screen
    ///   ([`FrameVerdict::Wait`]);
    /// * a frame **too far past due** is discharged without showing it, which is the only way to
    ///   catch up after a stall ([`FrameVerdict::Drop`]);
    /// * only a due frame is returned.
    ///
    /// `tolerance` is the caller's window, in seconds — half a display frame is the usual choice.
    /// `None` means "no frame is due this tick", which is a normal outcome and not an error: a
    /// 24 fps stream shown on a 60 Hz loop has frames due on only some ticks.
    pub fn tick_frame(
        &mut self,
        delta_ms: u32,
        tolerance: f64,
    ) -> Result<Option<VideoFrame>, String> {
        self.clock.tick(delta_ms);
        if self.clock.state() == PlaybackState::Ended && self.state == PlaybackState::Playing {
            self.state = PlaybackState::Ended;
            self.on_state_change.emit(self.state);
        }
        if self.state != PlaybackState::Playing {
            return Ok(None);
        }

        // # One frame-in-hand, shared by every read path
        //
        // `pending` is the single frame the engine has decoded but not yet shown. `tick_frame`
        // re-judges it each tick, and `step_frame`/`next_frame` read through the same slot, so no
        // read path can bypass a frame another path parked there and make the picture jump.
        //
        // # Why a tick may consume several frames
        //
        // A first version read one frame per tick and judged it. That is wrong twice over: it burns
        // the decoder at the loop's rate, and it makes `Wait` meaningless — the frame it "waited" for
        // had already been consumed and thrown away. Holding the frame fixes the burn, but it also
        // capped catching up at one drop per tick: after a stall, or whenever the source's frame rate
        // is no faster than the loop, the decoder falls behind one frame per tick and can never close
        // the gap. The loop below therefore keeps reading — dropping overdue frames and counting each
        // one — until a frame is due, a frame is in the future (parked for the next tick), or the
        // stream ends, all under a bounded read budget so a corrupt clock cannot read unboundedly.
        const CATCH_UP_BUDGET: usize = 32;
        let mut reads = 0usize;
        loop {
            if self.pending.is_none() {
                if reads >= CATCH_UP_BUDGET {
                    // Still not caught up within the budget; resume on the next tick rather than
                    // spending the whole loop here.
                    return Ok(None);
                }
                match self.decoder.read_frame()? {
                    Some(frame) => {
                        self.pending = Some(frame);
                        reads += 1;
                    }
                    None => {
                        // End of stream.
                        self.state = PlaybackState::Ended;
                        self.clock.pause();
                        self.on_state_change.emit(self.state);
                        return Ok(None);
                    }
                }
            }

            let Some(frame) = self.pending.as_ref() else {
                return Ok(None);
            };

            match self.clock.verdict_for(frame.timestamp, tolerance) {
                FrameVerdict::Wait => {
                    // Not due yet. The frame stays in hand and is re-judged next tick; the caller
                    // keeps the previous picture up, which is what stops playback running ahead.
                    return Ok(None);
                }
                FrameVerdict::Drop => {
                    // Overdue. Discard it so the next one can be read, and count the skip: a silent
                    // discard is indistinguishable from having rendered the frame.
                    self.clock.note_dropped();
                    if let Some(frame) = self.pending.take() {
                        self.current_time = frame.timestamp;
                    }
                }
                FrameVerdict::Show => {
                    let Some(frame) = self.pending.take() else {
                        return Ok(None);
                    };
                    self.current_time = frame.timestamp;
                    self.on_frame.emit(frame.clone());
                    return Ok(Some(frame));
                }
            }
        }
    }

    /// Frames dropped since the last call, resetting the count.
    ///
    /// See [`crate::video::clock::MediaClock::take_dropped`] for why it is taken and not read.
    pub fn take_dropped_frames(&mut self) -> u64 {
        self.clock.take_dropped()
    }

    /// Returns the next frame to show, honouring the frame already held in `pending`.
    ///
    /// Both [`VideoEngine::step_frame`] and [`VideoEngine::next_frame`] read through here so a frame
    /// parked by [`VideoEngine::tick_frame`] is not bypassed: reading the decoder directly would show
    /// a later frame while the parked one is still in hand, and the next tick would show it again —
    /// the picture jumping backwards.
    fn take_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        if let Some(frame) = self.pending.take() {
            return Ok(Some(frame));
        }
        self.decoder.read_frame()
    }

    /// Advance one frame (step forward).
    pub fn step_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        let frame = self.take_frame()?;
        if let Some(ref f) = frame {
            self.current_time = f.timestamp;
            self.on_frame.emit(f.clone());
        }
        Ok(frame)
    }

    /// Decode the next available frame. Returns None at end of stream.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        if self.state != PlaybackState::Playing {
            return Ok(None);
        }
        let frame = self.take_frame()?;
        if let Some(ref f) = frame {
            self.current_time = f.timestamp;
            self.on_frame.emit(f.clone());
        } else {
            // End of stream
            self.state = PlaybackState::Stopped;
            self.on_state_change.emit(self.state);
        }
        Ok(frame)
    }

    /// Returns the duration of the video in seconds.
    pub fn duration(&self) -> f64 {
        self.metadata.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_engine_open() {
        let data = vec![0u8; 100]; // Not a real video
        let result = VideoEngine::open(data);
        assert!(result.is_err()); // Unknown format
    }

    #[test]
    fn test_playback_states() {
        let mut engine = engine_with_pattern();
        engine.play();
        assert_eq!(engine.state(), PlaybackState::Playing);
        engine.pause();
        assert_eq!(engine.state(), PlaybackState::Paused);
        engine.stop();
        assert_eq!(engine.state(), PlaybackState::Stopped);
    }

    #[test]
    fn test_seek_bounds() {
        let mut engine = engine_with_pattern();
        assert_eq!(engine.current_time(), 0.0);

        engine.seek(5.0).expect("a valid seek succeeds");
        assert!((engine.current_time() - 5.0).abs() < 1e-9, "seek moves current time");
        assert!((engine.clock().position() - 5.0).abs() < 1e-9, "seek moves the clock");

        engine.seek(-3.0).expect("a negative seek clamps rather than errors");
        assert_eq!(engine.current_time(), 0.0, "the floor clamps to the start");

        let duration = engine.duration();
        engine.seek(duration + 10.0).expect("a seek past the end clamps rather than errors");
        assert_eq!(engine.current_time(), duration, "the ceiling clamps to the duration");

        // A decoder that cannot seek must report the failure and leave the clock untouched.
        let mut failing = VideoEngine::with_decoder(Box::new(FailingSeekDecoder::new()));
        assert!(failing.seek(1.0).is_err(), "a failing decoder seek is reported, not swallowed");
        assert_eq!(failing.current_time(), 0.0, "a failed seek does not move current time");
        assert_eq!(failing.clock().position(), 0.0, "a failed seek does not move the clock");
    }

    /// A decoder that always fails to seek, used to assert the engine reports seek
    /// failures instead of leaving its clock and current time inconsistent.
    struct FailingSeekDecoder {
        metadata: VideoMetadata,
    }

    impl FailingSeekDecoder {
        fn new() -> Self {
            Self {
                metadata: VideoMetadata::new_with_format(ContainerFormat::Mjpeg, 320, 240, 10.0),
            }
        }
    }

    impl VideoDecoder for FailingSeekDecoder {
        fn read_frame(&mut self) -> Result<Option<VideoFrame>, String> {
            Ok(None)
        }

        fn seek(&mut self, _time: f64) -> Result<(), String> {
            Err("seek is not supported by this decoder".to_string())
        }

        fn close(&mut self) -> Result<(), String> {
            Ok(())
        }

        fn metadata(&self) -> &VideoMetadata {
            &self.metadata
        }
    }

    // ── The master clock and the sync policy (BLUE24 §12 U-2) ───────────────

    /// An engine over [`FrameBufferDecoder`], which emits exact presentation times.
    ///
    /// This is the seam [`VideoEngine::with_decoder`] exists for: the sync policy can be tested
    /// against a source with known timestamps instead of a real file, and it gives
    /// `FrameBufferDecoder` — previously unreachable from anywhere — its first caller.
    fn engine_with_pattern() -> VideoEngine {
        VideoEngine::with_decoder(Box::new(crate::video::decoder::FrameBufferDecoder::new(
            Vec::new(),
            ContainerFormat::Mjpeg,
        )))
    }

    /// A paused engine does not advance its clock, however many frames the loop delivers.
    #[test]
    fn a_paused_engine_does_not_advance_the_clock() {
        let mut engine = engine_with_pattern();
        engine.play();
        let _ = engine.tick_frame(16, 0.008);
        let paused_at = engine.clock().position();
        engine.pause();
        for _ in 0..60 {
            let _ = engine.tick_frame(16, 0.008);
        }
        assert_eq!(engine.clock().position(), paused_at, "a paused frame advances no media time");
    }

    /// The clock advances with the frame loop's delta, in the frame loop's units.
    ///
    /// Driven on the **clock**, not through `tick_frame`: the engine stops advancing once the
    /// pattern source is exhausted, and a test that conflates "the loop ticked" with "the media had
    /// frames left" would be measuring the source's length rather than the clock's arithmetic.
    /// `tick_frame`'s own behaviour is covered by the verdict tests below.
    #[test]
    fn the_engine_clock_advances_by_the_loop_delta() {
        let mut engine = engine_with_pattern();
        engine.play();
        for _ in 0..60 {
            engine.clock_mut().tick(16);
        }
        assert!(
            (engine.clock().position() - 0.96).abs() < 1e-9,
            "60 frames of 16 ms is 0.96 s, got {}",
            engine.clock().position()
        );
    }

    /// The engine's own clock moves with the loop, and stops at the end of the media.
    #[test]
    fn ticking_the_engine_advances_its_clock() {
        let mut engine = engine_with_pattern();
        engine.play();
        let mut ticks = 0;
        while engine.state() == PlaybackState::Playing && ticks < 1_000 {
            let _ = engine.tick_frame(16, 0.008);
            ticks += 1;
        }
        assert!(ticks < 1_000, "playback must reach the end rather than tick for ever");
        assert!(engine.clock().position() > 0.0, "and the clock advanced on the way");
    }

    /// A frame that is not due yet is not shown — that is what stops playback running ahead.
    ///
    /// The pattern source runs at 10 fps, so its first frame is at 0.0 and the second at 0.1 s. On a
    /// 60 Hz loop the clock has only reached 0.016 s when the second frame arrives, which is 0.084 s
    /// early — far outside an 8 ms tolerance. A player without a clock hands it straight over and
    /// runs at whatever rate the decoder manages.
    #[test]
    fn an_early_frame_is_not_shown() {
        let mut engine = engine_with_pattern();
        engine.play();

        // The first frame is due (clock at 0.016, frame at 0.0, inside tolerance after one tick).
        let first = engine.tick_frame(16, 0.02).expect("no error");
        assert!(first.is_some(), "the frame at 0.0 is due");

        // The next frame is at 0.1 s. One more 16 ms tick puts the clock at 0.032 — still early.
        let second = engine.tick_frame(16, 0.008).expect("no error");
        assert!(second.is_none(), "a frame 68 ms in the future is not due yet");
    }

    /// A frame the clock has passed is **discharged without being shown**, and the skip is counted.
    ///
    /// # The defect this pins
    ///
    /// A player that never drops can only fall further behind: the media timeline is fixed and its own
    /// is not. So an overdue frame must be discarded rather than shown late, and the discard must be
    /// **counted** — a silent skip is indistinguishable from having rendered the frame.
    ///
    /// # The timestamps are the decoder's, not an assumption
    ///
    /// `FrameBufferDecoder` reports `frame_rate: 0.0`, which its own generator treats as 1 fps
    /// (`frame_rate.max(1.0)`), so its frames are one **second** apart. A first version of this test
    /// assumed 10 fps and failed; the failure was the test's, and the arithmetic below is written
    /// against the decoder's measured timestamps.
    #[test]
    fn an_overdue_frame_is_discharged_and_counted() {
        let mut engine = engine_with_pattern();
        engine.play();

        // Time passed while the loop was not reading anything — a stall. The clock is moved directly
        // because that is exactly what a stall is: the clock kept running, the reader did not.
        engine.clock_mut().tick(500);
        assert!((engine.clock().position() - 0.5).abs() < 1e-9);

        // The first frame is at 0.0 s: half a second overdue, so it is discharged rather than shown.
        let overdue = engine.tick_frame(0, 0.05).expect("no error");
        assert!(overdue.is_none(), "a frame half a second past due is not shown");
        assert_eq!(engine.take_dropped_frames(), 1, "and the skip is counted, not silent");

        // The next frame is at 1.0 s, which is ahead of the clock: a wait is not a drop.
        let early = engine.tick_frame(0, 0.05).expect("no error");
        assert!(early.is_none(), "a frame 0.5 s in the future is not shown");
        assert_eq!(engine.take_dropped_frames(), 0, "waiting is not dropping");
    }

    /// A frame the clock has reached is shown, and the clock is what decides that.
    #[test]
    fn a_frame_the_clock_has_reached_is_shown() {
        let mut engine = engine_with_pattern();
        engine.play();

        // At t = 0 the frame at 0.0 s is due (its lead is zero, inside any tolerance).
        let first = engine.tick_frame(0, 0.05).expect("no error");
        assert!(first.is_some(), "the frame at the clock's position is due");

        // Drive the clock forward in small steps until the frame at 1.0 s becomes due. The source is
        // read only when it is; reading it every tick would exhaust a 1 fps stream long before its
        // own first second, which is a property of the fixture rather than of the policy.
        let mut shown = 0;
        for _ in 0..200 {
            if engine.tick_frame(16, 0.05).expect("no error").is_some() {
                shown += 1;
                break;
            }
        }
        assert_eq!(shown, 1, "the frame the clock reached is shown exactly once");
    }

    /// A seek moves the clock with the decoder, so the first frame after it is not judged overdue.
    #[test]
    fn a_seek_moves_the_clock_with_the_decoder() {
        let mut engine = engine_with_pattern();
        engine.play();
        engine.seek(3.0).expect("the pattern source seeks");
        assert!((engine.clock().position() - 3.0).abs() < 1e-9, "the clock followed the decoder");

        // The frame the decoder returns after seeking to 3.0 is at 3.0, which is due — not "3 s
        // overdue", which is what it would look like if the clock had stayed at 0.
        let frame = engine.tick_frame(0, 0.05).expect("no error");
        assert!(frame.is_some(), "the frame at the seek position is due");
    }

    /// Reaching the end of the stream reports `Ended` through the one signal that carries state.
    #[test]
    fn the_end_of_the_stream_reports_ended() {
        let mut engine = engine_with_pattern();
        // The pattern is 10 s at 10 fps. Seek just before the end and play past it.
        engine.seek(9.95).expect("seeks");
        engine.play();
        let mut ended = false;
        for _ in 0..20 {
            let _ = engine.tick_frame(16, 0.05);
            if engine.state() == PlaybackState::Ended {
                ended = true;
                break;
            }
        }
        assert!(ended, "playback must reach the end rather than run for ever");
    }

    /// Stopping and playing again must resume from the first frame, not from where the
    /// decoder happened to be when it was stopped.
    #[test]
    fn stop_then_play_restarts_at_the_first_frame() {
        let mut engine = engine_with_pattern();
        engine.play();

        // Show the first frame, which advances the decoder past it.
        let first = engine.tick_frame(0, 0.05).expect("no error");
        assert!(first.is_some(), "the frame at 0.0 is due");
        assert_eq!(engine.current_time(), 0.0);

        engine.stop();
        engine.play();

        // The first frame after a stop/play is 0.0 again, not the decoder's old position.
        let restarted = engine.tick_frame(0, 0.05).expect("no error");
        assert!(restarted.is_some(), "playback resumes from the first frame");
        assert_eq!(engine.current_time(), 0.0, "the decoder restarted, not resumed");
    }

    /// Replaying a stream that reached its end must rewind the decoder, or the next tick
    /// reports `Ended` again immediately.
    #[test]
    fn replay_after_the_end_starts_from_the_first_frame() {
        let mut engine = engine_with_pattern();
        engine.play();

        let mut ended = false;
        for _ in 0..1_000 {
            let _ = engine.tick_frame(16, 0.05);
            if engine.state() == PlaybackState::Ended {
                ended = true;
                break;
            }
        }
        assert!(ended, "playback reaches the end");

        engine.play();
        assert_eq!(engine.state(), PlaybackState::Playing, "play leaves Ended");
        let restarted = engine.tick_frame(0, 0.05).expect("no error");
        assert!(restarted.is_some(), "replay starts from the first frame, not EOF");
        assert_eq!(engine.current_time(), 0.0, "the first frame is at 0.0");
    }

    /// Seeking to the end, ticking, and playing again must start over rather than re-end.
    #[test]
    fn play_after_seeking_to_the_end_does_not_re_end_immediately() {
        let mut engine = engine_with_pattern();
        engine.play();

        engine.seek(engine.duration()).expect("seek to the end");
        let _ = engine.tick_frame(0, 0.05);
        assert_eq!(engine.state(), PlaybackState::Ended, "a tick after the end reports Ended");

        engine.play();
        assert_eq!(engine.state(), PlaybackState::Playing, "play leaves Ended");
        let first = engine.tick_frame(0, 0.05).expect("no error");
        assert!(first.is_some(), "replay starts from the first frame instead of re-ending");
    }

    /// A manual step reads the frame `tick_frame` parked, rather than bypassing it and
    /// reading the decoder directly.
    #[test]
    fn step_and_tick_share_one_pending_frame() {
        let mut engine = engine_with_pattern();
        engine.play();

        // Show frame 0, then park frame 1 (at 1.0 s) as not-yet-due.
        assert!(engine.tick_frame(0, 0.05).expect("no error").is_some(), "frame 0 is shown");
        assert!(engine.tick_frame(0, 0.05).expect("no error").is_none(), "frame 1 is parked");

        // A manual step must hand back the parked frame 1, not read ahead to frame 2.
        let stepped = engine.step_frame().expect("no error");
        let stepped = stepped.expect("step_frame returns the parked frame");
        assert_eq!(stepped.timestamp, 1.0, "and it is the frame tick parked");
        assert_eq!(engine.current_time(), 1.0, "current time follows the shown frame");
    }

    /// One tick drops every overdue frame (within the budget) instead of at most one, so a
    /// source running no faster than the loop can catch up after a stall.
    #[test]
    fn a_tick_catches_up_across_multiple_overdue_frames() {
        let mut engine = engine_with_pattern();
        engine.play();

        // A stall: the clock runs 3.5 s ahead while nothing is read.
        engine.clock_mut().tick(3500);

        let result = engine.tick_frame(0, 0.05).expect("no error");
        assert!(result.is_none(), "the overdue frames are dropped, not shown");
        let dropped = engine.take_dropped_frames();
        assert!(
            dropped >= 3,
            "several overdue frames are dropped in one tick, not one: got {dropped}"
        );
    }
}
