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
    pub fn play(&mut self) {
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

        // # Why the decoder is read only when there is no frame in hand
        //
        // A first version read one frame per tick and judged it. That is wrong twice over: it burns
        // the decoder at the loop's rate (a 1 fps stream read at 60 Hz exhausts its whole length
        // before the first second of playback has passed), and it makes `Wait` meaningless — the frame
        // it "waited" for had already been consumed and thrown away.
        //
        // Holding the frame instead means a tick during which nothing is due costs a comparison and
        // no decode, which is what makes this affordable to call every frame. The held frame is
        // re-judged each tick, so it is shown as soon as the clock reaches it.
        if self.pending.is_none() {
            match self.decoder.read_frame()? {
                Some(frame) => self.pending = Some(frame),
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
                // Not due yet. The frame stays in hand and is re-judged next tick; the caller keeps
                // the previous picture up, which is what stops playback running ahead.
                Ok(None)
            }
            FrameVerdict::Drop => {
                // Overdue. Discard it so the next one can be read, and count the skip: a silent
                // discard is indistinguishable from having rendered the frame.
                self.clock.note_dropped();
                if let Some(frame) = self.pending.take() {
                    self.current_time = frame.timestamp;
                }
                Ok(None)
            }
            FrameVerdict::Show => {
                let Some(frame) = self.pending.take() else {
                    return Ok(None);
                };
                self.current_time = frame.timestamp;
                self.on_frame.emit(frame.clone());
                Ok(Some(frame))
            }
        }
    }

    /// Frames dropped since the last call, resetting the count.
    ///
    /// See [`crate::video::clock::MediaClock::take_dropped`] for why it is taken and not read.
    pub fn take_dropped_frames(&mut self) -> u64 {
        self.clock.take_dropped()
    }

    /// Advance one frame (step forward).
    pub fn step_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        let frame = self.decoder.read_frame()?;
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
        let frame = self.decoder.read_frame()?;
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
        let data = vec![0u8; 100];
        if let Ok(mut engine) = VideoEngine::open(data) {
            engine.play();
            assert_eq!(engine.state(), PlaybackState::Playing);
            engine.pause();
            assert_eq!(engine.state(), PlaybackState::Paused);
            engine.stop();
            assert_eq!(engine.state(), PlaybackState::Stopped);
        }
    }

    #[test]
    fn test_seek_bounds() {
        let data = vec![0u8; 100];
        if let Ok(mut engine) = VideoEngine::open(data) {
            assert_eq!(engine.current_time(), 0.0);
            let _ = engine.seek(5.0);
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
}
