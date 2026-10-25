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
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};

#[cfg(feature = "video-codecs")]
use crate::video::ffmpeg_decoder::FfmpegDecoder;

/// A command from the engine to the decode thread.
///
/// `generation` is the engine's epoch token. The worker stamps every event it produces with it,
/// so a frame decoded before a seek can be recognised and discarded instead of shown at the new
/// position (MEDIA-03).
enum DecodeCommand {
    /// Seek to `time` and answer with [`DecodeEvent::Seeked`].
    Seek { time: f64, generation: u64 },
    /// Release the decoder and stop the thread.
    Close,
}

/// One event from the decode thread, tagged with the generation that produced it.
enum DecodeEvent {
    Frame { generation: u64, frame: VideoFrame },
    End { generation: u64 },
    Failed { generation: u64, message: String },
    Seeked { generation: u64, result: Result<(), String> },
}

/// Depth of the decoded-frame queue.
///
/// Small on purpose: the queue is the read-ahead window, and a decoded 1080p RGBA frame is ~8 MiB.
/// Eight frames is enough to absorb a stall of a few display frames without letting a fast
/// decoder fill memory with a whole film.
const DECODE_QUEUE_DEPTH: usize = 8;

/// Owns the decode thread and the two channels to it.
///
/// # Why the decoder lives on its own thread (MEDIA-03)
///
/// `VideoEngine::tick_frame` used to call `decoder.read_frame()` on the frame loop's thread, so a
/// software H.264 decode (10–30 ms for 1080p) blocked the UI for every due frame. The decoder is
/// `Send`, so it now runs on a worker that reads ahead into a bounded queue; the engine pulls from
/// the queue and never blocks once the window is primed.
///
/// The blocking contract is deliberately conservative: an empty queue still blocks the *first*
/// read of a tick, which is exactly the old behaviour. That keeps start-up and a decoder that
/// cannot keep up no worse than before, while steady-state playback costs a channel receive.
struct DecodeWorker {
    commands: Sender<DecodeCommand>,
    events: Option<Receiver<DecodeEvent>>,
    handle: Option<JoinHandle<()>>,
}

impl DecodeWorker {
    fn spawn(mut decoder: Box<dyn VideoDecoder + Send>) -> Self {
        let (commands, command_rx) = mpsc::channel::<DecodeCommand>();
        let (event_tx, events) = mpsc::sync_channel::<DecodeEvent>(DECODE_QUEUE_DEPTH);
        let handle = thread::Builder::new()
            .name("rust-widgets-video-decode".to_string())
            .spawn(move || decode_loop(decoder.as_mut(), &command_rx, &event_tx));
        match handle {
            Ok(handle) => Self { commands, events: Some(events), handle: Some(handle) },
            // A host that cannot spawn a thread gets the old behaviour: the error surfaces on the
            // first read rather than a decoder that silently produces nothing.
            Err(_) => Self { commands, events: Some(events), handle: None },
        }
    }

    /// Blocks until the next event, or `None` when the worker is gone.
    fn recv(&mut self) -> Option<DecodeEvent> {
        self.events.as_ref()?.recv().ok()
    }

    /// Takes an event only if one is already queued.
    fn try_recv(&mut self) -> Result<DecodeEvent, TryRecvError> {
        match self.events.as_ref() {
            Some(events) => events.try_recv(),
            None => Err(TryRecvError::Disconnected),
        }
    }

    fn send(&self, command: DecodeCommand) -> Result<(), String> {
        self.commands.send(command).map_err(|_| {
            "the video decode thread has stopped; the source can no longer be read".to_string()
        })
    }
}

impl Drop for DecodeWorker {
    fn drop(&mut self) {
        // Dropping the receiver first is what unblocks a worker parked in `send` on a full
        // queue; the `Close` then reaches it (or fails, if it already noticed the disconnect).
        self.events.take();
        let _ = self.commands.send(DecodeCommand::Close);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The decode thread.
///
/// It reads frames continuously, tagging each with the generation that was current when the read
/// began. Commands are checked before each read, so a seek takes effect at the next frame
/// boundary; the engine ignores events from older generations until the new one arrives.
fn decode_loop(
    decoder: &mut (dyn VideoDecoder + Send),
    commands: &Receiver<DecodeCommand>,
    events: &SyncSender<DecodeEvent>,
) {
    let mut generation = 0u64;
    loop {
        // Commands first: a seek must not be starved by a long read-ahead.
        loop {
            match commands.try_recv() {
                Ok(DecodeCommand::Seek { time, generation: requested }) => {
                    generation = requested;
                    let result = decoder.seek(time);
                    if events.send(DecodeEvent::Seeked { generation, result }).is_err() {
                        return;
                    }
                }
                Ok(DecodeCommand::Close) => {
                    let _ = decoder.close();
                    return;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        match decoder.read_frame() {
            Ok(Some(frame)) => {
                if events.send(DecodeEvent::Frame { generation, frame }).is_err() {
                    return;
                }
            }
            Ok(None) => {
                if events.send(DecodeEvent::End { generation }).is_err() {
                    return;
                }
                // Park until a seek or close arrives; spinning here would burn a core while the
                // caller holds the last frame on screen.
                match commands.recv() {
                    Ok(DecodeCommand::Seek { time, generation: requested }) => {
                        generation = requested;
                        let result = decoder.seek(time);
                        if events.send(DecodeEvent::Seeked { generation, result }).is_err() {
                            return;
                        }
                    }
                    Ok(DecodeCommand::Close) | Err(_) => {
                        let _ = decoder.close();
                        return;
                    }
                }
            }
            Err(message) => {
                if events.send(DecodeEvent::Failed { generation, message }).is_err() {
                    return;
                }
                // Park like the end-of-stream path: a failed read does not recover by retrying.
                match commands.recv() {
                    Ok(DecodeCommand::Seek { time, generation: requested }) => {
                        generation = requested;
                        let result = decoder.seek(time);
                        if events.send(DecodeEvent::Seeked { generation, result }).is_err() {
                            return;
                        }
                    }
                    Ok(DecodeCommand::Close) | Err(_) => {
                        let _ = decoder.close();
                        return;
                    }
                }
            }
        }
    }
}

/// Video player engine that wraps a decoder and exposes playback control.
///
/// # Who consumes it
///
/// This engine is a **standalone, host-driven component**: it is not mounted into the widget
/// tree, and no in-crate control subscribes to its signals. [`Self::on_frame`] and
/// [`Self::on_state_change`] are the host's hooks — a host that owns an engine connects them
/// to its own presentation path, exactly as it drives `tick_frame`. `MediaPlayer` deliberately
/// does not own one (it has no decoder and shares only the [`MediaClock`] *type*, not an
/// instance), so there is no second, silent wiring path.
pub struct VideoEngine {
    /// The decode thread and the channels to it.
    worker: DecodeWorker,
    metadata: VideoMetadata,
    /// The generation of the current position. Events from an older generation are stale.
    generation: u64,
    /// True once the current generation has reported end of stream.
    ended: bool,
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
            // A finished stream is replayed from its first frame: the clock rewinding alone is not
            // enough, because the decoder is still parked at end-of-stream and the next read would
            // report `Ended` again immediately. `seek` restarts the decode thread's generation and
            // moves the clock with it, which is the one path that keeps the two in step.
            if let Err(e) = self.seek(0.0) {
                log::error!("[video-engine] failed to rewind decoder on replay: {e}");
                return;
            }
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
        // resume from where the decoder left off rather than from the first frame. Routing through
        // `seek` also restarts the decode generation, so frames already queued for the old position
        // cannot be shown after the stop (MEDIA-03).
        if let Err(e) = self.seek(0.0) {
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
        let (generation, time) = self.request_seek(time)?;
        // Wait for the decoder's answer. Frames from older generations are drained here — they are
        // exactly the frames the seek makes obsolete, and leaving them in the bounded queue would
        // keep the worker blocked on a full channel.
        let result =
            loop {
                match self.worker.recv() {
                    Some(DecodeEvent::Seeked { generation: answered, result })
                        if answered == generation =>
                    {
                        break result
                    }
                    Some(_) => continue,
                    None => return Err(
                        "the video decode thread stopped while seeking; the source can no longer \
                         be read"
                            .to_string(),
                    ),
                }
            };
        result?;
        // Only a successful seek moves the reported position: a caller that got an error must not
        // observe a clock it never reached.
        self.apply_seek_state(time);
        Ok(())
    }

    /// Requests a seek and returns without waiting for the decoder.
    ///
    /// # Why this exists
    ///
    /// [`Self::seek`] blocks until the decode thread answers, and that answer cannot arrive before
    /// the `read_frame` already in flight finishes — 10–30 ms for 1080p, plus the codec's own
    /// keyframe search. A progress bar that calls `seek` on every pointer move therefore
    /// serialises the drag on the decoder (MEDIA-N05). Scrubbing calls this instead; the answer is
    /// consumed by the ordinary frame path, where a stale generation is dropped anyway.
    ///
    /// The optimistic state here is what makes the drag feel immediate: the clock and the reported
    /// position move at once, and a frame decoded for the previous position has already been
    /// invalidated by the new generation, so it can never be shown at the new time. The cost of
    /// that optimism is that a decoder-side seek failure is not reported by this call; it resurfaces
    /// as a decode failure on the next frame request.
    pub fn seek_async(&mut self, time: f64) -> Result<(), String> {
        let (_, time) = self.request_seek(time)?;
        self.apply_seek_state(time);
        Ok(())
    }

    /// Sends the seek command and returns `(generation, clamped time)`.
    ///
    /// Deliberately does not touch the clock or `current_time`: the synchronous path must not show
    /// a position the decoder refused, and the asynchronous path applies it itself.
    fn request_seek(&mut self, time: f64) -> Result<(u64, f64), String> {
        let time = ordered_clamp_f64(time, 0.0, self.metadata.duration);
        // Every seek starts a new generation, so frames already decoded for the previous position
        // are recognisable as stale and are dropped rather than shown at the new one.
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.worker.send(DecodeCommand::Seek { time, generation })?;
        Ok((generation, time))
    }

    /// Moves the reported position and the clock to a seek target.
    ///
    /// A frame decoded before the seek is a frame from where playback *was*; showing it after the
    /// seek would put one stale picture in the middle of the jump, so the parked frame is dropped.
    /// The clock moves with the decoder: if they disagreed, the first frame after a seek would be
    /// judged against the pre-seek position and dropped as "overdue" — a seek that stutters.
    fn apply_seek_state(&mut self, time: f64) {
        self.ended = false;
        self.current_time = time;
        self.pending = None;
        self.clock.seek(time);
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
            worker: DecodeWorker::spawn(decoder),
            metadata,
            generation: 0,
            ended: false,
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
                // Pull from the decode thread's read-ahead queue. A queued frame is a
                // non-blocking receive; an empty queue blocks for the next frame, bounded by the
                // catch-up budget above. Blocking is the old synchronous contract, so a decoder
                // that cannot keep up behaves exactly as it did (MEDIA-03).
                let next = match self.try_decoded() {
                    Some(result) => result?,
                    None => self.next_decoded()?,
                };
                match next {
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
    /// See `crate::video::clock::MediaClock::take_dropped` for why it is taken and not read.
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
        self.next_decoded()
    }

    /// Blocks until the decode thread produces a frame for the current generation.
    ///
    /// Stale events — frames and end markers from before a seek — are discarded here; they belong
    /// to the position playback left.
    fn next_decoded(&mut self) -> Result<Option<VideoFrame>, String> {
        if self.ended {
            return Ok(None);
        }
        loop {
            let Some(event) = self.worker.recv() else {
                return Err(
                    "the video decode thread stopped unexpectedly; the source can no longer be read"
                        .to_string(),
                );
            };
            match event {
                DecodeEvent::Frame { generation, frame } => {
                    if generation == self.generation {
                        return Ok(Some(frame));
                    }
                }
                DecodeEvent::End { generation } => {
                    if generation == self.generation {
                        self.ended = true;
                        return Ok(None);
                    }
                }
                DecodeEvent::Failed { generation, message } => {
                    if generation == self.generation {
                        return Err(message);
                    }
                }
                // `seek` is the only caller that waits for this event.
                DecodeEvent::Seeked { .. } => {}
            }
        }
    }

    /// Takes a decoded frame only if one is already queued.
    ///
    /// `None` means "the decoder has not got there yet", which the frame loop treats as a normal
    /// no-frame tick rather than as a reason to wait.
    fn try_decoded(&mut self) -> Option<Result<Option<VideoFrame>, String>> {
        if self.ended {
            return Some(Ok(None));
        }
        loop {
            match self.worker.try_recv() {
                Ok(DecodeEvent::Frame { generation, frame }) => {
                    if generation == self.generation {
                        return Some(Ok(Some(frame)));
                    }
                }
                Ok(DecodeEvent::End { generation }) => {
                    if generation == self.generation {
                        self.ended = true;
                        return Some(Ok(None));
                    }
                }
                Ok(DecodeEvent::Failed { generation, message }) => {
                    if generation == self.generation {
                        return Some(Err(message));
                    }
                }
                Ok(DecodeEvent::Seeked { .. }) => {}
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(Err(
                        "the video decode thread stopped unexpectedly; the source can no longer \
                         be read"
                            .to_string(),
                    ))
                }
            }
        }
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

    /// A scrub must not wait for the decoder: `seek_async` returns after the send, and the
    /// reported position/clock move immediately.
    ///
    /// The decoder here sleeps in `seek`, which is the shape of the real cost (an in-flight
    /// `read_frame` plus the codec's keyframe search). If `seek_async` ever regressed to waiting,
    /// this call would take ~400 ms instead of returning at once.
    #[test]
    fn seek_async_returns_before_the_decoder_answers() {
        struct SlowSeekDecoder {
            metadata: VideoMetadata,
        }

        impl VideoDecoder for SlowSeekDecoder {
            fn read_frame(&mut self) -> Result<Option<VideoFrame>, String> {
                Ok(None)
            }

            fn seek(&mut self, _time: f64) -> Result<(), String> {
                std::thread::sleep(std::time::Duration::from_millis(400));
                Ok(())
            }

            fn close(&mut self) -> Result<(), String> {
                Ok(())
            }

            fn metadata(&self) -> &VideoMetadata {
                &self.metadata
            }
        }

        let metadata = VideoMetadata::new_with_format(ContainerFormat::Mjpeg, 320, 240, 10.0);
        let mut engine = VideoEngine::with_decoder(Box::new(SlowSeekDecoder { metadata }));

        let start = std::time::Instant::now();
        engine.seek_async(4.0).expect("the request is accepted");
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_millis(150),
            "seek_async waited for the decoder ({elapsed:?}); a scrub would serialise on it"
        );
        assert_eq!(engine.current_time(), 4.0, "the reported position moves immediately");
        assert!(
            (engine.clock().position() - 4.0).abs() < 1e-9,
            "and the clock moves with it, so the first frame is not judged overdue"
        );
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

    /// Decoding runs on the engine's worker thread, not on the caller's.
    ///
    /// This is the observable half of MEDIA-03: `tick_frame` used to call `read_frame` inline, so
    /// a slow codec blocked the frame loop. The decoder now records the thread it ran on, and the
    /// assertion is that it is never the thread that called `next_frame`.
    #[test]
    fn frames_are_decoded_off_the_calling_thread() {
        use std::sync::{Arc, Mutex};

        struct ThreadRecordingDecoder {
            metadata: VideoMetadata,
            threads: Arc<Mutex<Vec<std::thread::ThreadId>>>,
            read: u64,
        }

        impl VideoDecoder for ThreadRecordingDecoder {
            fn read_frame(&mut self) -> Result<Option<VideoFrame>, String> {
                self.threads.lock().unwrap().push(std::thread::current().id());
                if self.read >= 4 {
                    return Ok(None);
                }
                self.read += 1;
                Ok(Some(VideoFrame::new(self.read as f64 / 10.0, vec![0u8; 4], 1, 1)))
            }

            fn seek(&mut self, _time: f64) -> Result<(), String> {
                Ok(())
            }

            fn close(&mut self) -> Result<(), String> {
                Ok(())
            }

            fn metadata(&self) -> &VideoMetadata {
                &self.metadata
            }
        }

        let threads = Arc::new(Mutex::new(Vec::new()));
        let decoder = ThreadRecordingDecoder {
            metadata: VideoMetadata::new_with_format(ContainerFormat::Mjpeg, 1, 1, 10.0),
            threads: Arc::clone(&threads),
            read: 0,
        };

        let caller = std::thread::current().id();
        let mut engine = VideoEngine::with_decoder(Box::new(decoder));
        engine.play();
        assert!(engine.next_frame().expect("no error").is_some(), "a frame is produced");

        let seen = threads.lock().unwrap();
        assert!(!seen.is_empty(), "the decoder was read");
        assert!(
            seen.iter().all(|thread| *thread != caller),
            "every read must happen on the decode thread, not the caller's: {seen:?}"
        );
    }

    /// A seek discards frames already decoded for the old position.
    ///
    /// The worker reads ahead, so without the generation check the first frame after a seek could
    /// be one from before it — the stale-picture bug the generation token exists to prevent.
    #[test]
    fn a_seek_drops_frames_decoded_for_the_old_position() {
        let mut engine = engine_with_pattern();
        engine.play();
        // Prime the read-ahead window before moving.
        assert!(engine.tick_frame(0, 0.05).expect("no error").is_some());

        engine.seek(5.0).expect("seek");
        let frame = engine.next_frame().expect("no error").expect("a frame after the seek");
        assert!(
            frame.timestamp >= 4.9,
            "the first frame after seeking to 5.0 s must not be an earlier one: got {}",
            frame.timestamp
        );
    }
}
