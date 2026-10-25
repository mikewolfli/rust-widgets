// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VideoPlayer — simulated video player widget (self-drawn UI, no media engine).
//!
//! This is a simulation/preview component: it draws a player UI (video area
//! background, transport controls, seek bar, time display) and tracks a local
//! simulated clock. It does not open, demux or decode media files. play/pause/
//! seek only move the simulated position, which advances when `tick` is called.
//! It emits signals for playback state changes and time updates.

use core::cell::Cell;
use core::fmt::Write as _;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

/// A subtitle cue for the simulated player.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleCue {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Text shown while `start <= t < end`.
    pub text: String,
}

impl SubtitleCue {
    /// A cue covering `[start, end)` with `text`.
    pub fn new(start: f64, end: f64, text: impl Into<String>) -> Self {
        Self { start, end, text: text.into() }
    }
}

/// A decoded frame supplied by the host.
///
/// The data is tightly packed RGBA, row-major, with no padding between rows. The control does
/// not demux or decode anything itself; it presents what the host hands it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Presentation timestamp in milliseconds.
    pub pts_ms: u64,
    /// RGBA bytes, `width * height * 4` long.
    pub data: Vec<u8>,
}

/// What the host-driven pipeline is currently doing.
///
/// The control owns no demuxer or decoder; this enum records which stage of a host pipeline
/// last reported activity, so a caller can observe the difference between "nothing loaded",
/// "waiting for a decoded frame" and "a frame is on screen".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoPipelineState {
    /// No source is loaded.
    #[default]
    Idle,
    /// A source was handed over and the control is waiting for the host's stream.
    Demuxing,
    /// The host is decoding; no frame has been injected yet.
    Decoding,
    /// A host frame is available for presentation.
    Presenting,
    /// Playback reached the end of the timeline.
    Ended,
}

/// One selectable audio track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTrack {
    /// Stable track identifier, as the host names it.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// BCP-47 language tag, when the container declares one.
    pub language: Option<String>,
}

impl AudioTrack {
    /// A track with an identifier and a label.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), language: None }
    }

    /// The same track with a language tag attached.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }
}

/// Metadata the host read from the container.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoMetadata {
    /// Title, when the container declares one.
    pub title: String,
    /// Author or artist, when the container declares one.
    pub artist: String,
    /// Cover art as encoded image bytes, when the container carries it.
    pub artwork: Option<Vec<u8>>,
}

/// One selectable subtitle track.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTrack {
    /// Stable track identifier, as the host names it.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// BCP-47 language tag, when the container declares one.
    pub language: Option<String>,
    /// The cues belonging to this track.
    pub cues: Vec<SubtitleCue>,
}

impl SubtitleTrack {
    /// A track with an identifier, a label and its cues.
    pub fn new(id: impl Into<String>, label: impl Into<String>, cues: Vec<SubtitleCue>) -> Self {
        Self { id: id.into(), label: label.into(), language: None, cues }
    }
}

/// The player's reported media state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaStatus {
    /// No source is loaded.
    #[default]
    Idle,
    /// The source is loading.
    Loading,
    /// Playback is stalled waiting for data.
    Buffering,
    /// Frames are being presented.
    Playing,
    /// Playback is paused.
    Paused,
    /// The end of the media was reached.
    Ended,
    /// The host reported an error.
    Error,
}

/// VideoPlayer — simulated video player widget with playback controls.
///
/// Self-drawn UI only: no media is decoded or rendered. State (play/pause,
/// position, volume) is simulated and displayed locally.
pub struct VideoPlayer {
    base: BaseWidget,
    /// Source URI or path of the video.
    source: String,
    /// Whether playback is active.
    is_playing: bool,
    /// Current playback position in seconds.
    current_time: f64,
    /// Total duration in seconds.
    duration: f64,
    /// Volume level (0.0 = silent, 1.0 = full).
    volume: f32,
    /// Whether audio is muted.
    muted: bool,
    /// Playback rate multiplier.
    playback_rate: f32,
    /// Whether the control bar is visible.
    controls_visible: bool,
    /// Emitted when playback starts.
    pub playback_started: GenericSignal,
    /// Emitted when playback is paused.
    pub playback_paused: GenericSignal,
    /// Emitted when playback reaches the end.
    pub playback_ended: GenericSignal,
    /// Emitted when the current playback time updates. Passes the new time.
    pub time_updated: Signal1<f64>,
    /// Subtitle cues, sorted by start time when set.
    subtitles: Vec<SubtitleCue>,
    /// Whether cues are painted.
    subtitles_enabled: bool,
    /// Whether the player is in fullscreen presentation.
    fullscreen: bool,
    /// Bytes or seconds buffered ahead, as reported by the host.
    buffered: f64,
    /// The reported media state.
    status: MediaStatus,
    /// Emitted when fullscreen is entered or left.
    pub fullscreen_changed: Signal1<bool>,
    /// Emitted when the reported media status changes.
    pub media_status_changed: Signal1<MediaStatus>,
    /// Emitted with the buffered amount whenever the host reports more data.
    pub buffering_progress_changed: Signal1<f64>,
    /// Emitted when the host reports a playback failure.
    pub error_occurred: Signal1<String>,
    /// Emitted when the volume changes, so a host with an audio output path can mirror it.
    pub volume_changed: Signal1<f32>,
    /// Emitted when mute changes.
    pub mute_changed: Signal1<bool>,
    /// Emitted when the playback rate changes.
    pub playback_rate_changed: Signal1<f32>,
    /// Emitted when the duration changes, with the new duration in seconds.
    pub duration_changed: Signal1<f64>,
    /// Emitted with the presentation timestamp when the host injects a decoded frame.
    pub frame_available: Signal1<u64>,
    /// Emitted with the presentation timestamp when a frame is presented on screen.
    pub frame_presented: Signal1<u64>,
    /// Emitted when the host replaces the container metadata.
    pub metadata_changed: GenericSignal,
    /// Whether playback loops at the end instead of stopping.
    looping: bool,
    /// The most recent host-supplied frame, if any.
    frame: Option<VideoFrame>,
    /// The reported pipeline stage.
    pipeline: VideoPipelineState,
    /// Host-supplied audio tracks.
    audio_tracks: Vec<AudioTrack>,
    /// Index of the selected audio track in [`Self::audio_tracks`].
    active_audio_track: Option<usize>,
    /// Host-supplied subtitle tracks.
    subtitle_tracks: Vec<SubtitleTrack>,
    /// Index of the selected subtitle track in [`Self::subtitle_tracks`].
    active_subtitle_track: Option<usize>,
    /// Container metadata reported by the host.
    metadata: VideoMetadata,
    /// Cached cue index for [`Self::subtitle_at`], so a forward-playing clock is O(1).
    subtitle_cursor: Cell<usize>,
}

impl VideoPlayer {
    /// Creates a new VideoPlayer with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::VideoPlayer, geometry, "VideoPlayer"),
            source: String::new(),
            is_playing: false,
            current_time: 0.0,
            duration: 0.0,
            volume: 0.8,
            muted: false,
            playback_rate: 1.0,
            controls_visible: true,
            playback_started: GenericSignal::new(),
            playback_paused: GenericSignal::new(),
            playback_ended: GenericSignal::new(),
            time_updated: Signal1::new(),
            subtitles: Vec::new(),
            subtitles_enabled: true,
            fullscreen: false,
            buffered: 0.0,
            status: MediaStatus::Idle,
            fullscreen_changed: Signal1::new(),
            media_status_changed: Signal1::new(),
            buffering_progress_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            volume_changed: Signal1::new(),
            mute_changed: Signal1::new(),
            playback_rate_changed: Signal1::new(),
            duration_changed: Signal1::new(),
            frame_available: Signal1::new(),
            frame_presented: Signal1::new(),
            metadata_changed: GenericSignal::new(),
            looping: false,
            frame: None,
            pipeline: VideoPipelineState::Idle,
            audio_tracks: Vec::new(),
            active_audio_track: None,
            subtitle_tracks: Vec::new(),
            active_subtitle_track: None,
            metadata: VideoMetadata::default(),
            subtitle_cursor: Cell::new(0),
        }
    }

    /// Records a video source URI or file path for display.
    ///
    /// The source is stored as a label only: it is not opened, demuxed or
    /// decoded. Loading also resets the simulated clock and pauses playback.
    pub fn load(&mut self, source: &str) {
        self.source = source.to_string();
        self.current_time = 0.0;
        self.is_playing = false;
        self.frame = None;
        self.subtitle_cursor.set(0);
        self.pipeline =
            if source.is_empty() { VideoPipelineState::Idle } else { VideoPipelineState::Demuxing };
        self.set_status(if source.is_empty() { MediaStatus::Idle } else { MediaStatus::Paused });
        self.base.request_redraw();
    }

    /// Accepts a decoded frame from the host's pipeline.
    ///
    /// The control has no demuxer or decoder, so this is the only way pixels reach it. The
    /// frame is validated (non-empty dimensions, exactly `width * height * 4` RGBA bytes) before
    /// it is stored, so a malformed buffer cannot be painted. The pipeline moves to
    /// [`VideoPipelineState::Presenting`] and `frame_available` announces the timestamp.
    pub fn set_video_frame(&mut self, frame: VideoFrame) -> Result<(), String> {
        if frame.width == 0 || frame.height == 0 {
            return Err("a video frame needs non-zero dimensions".to_string());
        }
        let expected = (frame.width as usize) * (frame.height as usize) * 4;
        if frame.data.len() != expected {
            return Err(format!(
                "a {}x{} RGBA frame needs {expected} bytes, got {}",
                frame.width,
                frame.height,
                frame.data.len()
            ));
        }
        let pts = frame.pts_ms;
        self.frame = Some(frame);
        self.pipeline = VideoPipelineState::Presenting;
        self.frame_available.emit(pts);
        self.base.request_redraw();
        Ok(())
    }

    /// Presents the most recent host frame, announcing it as `frame_presented`.
    ///
    /// Returns `false` when the host has not supplied a frame, which is a statement about the
    /// pipeline rather than an error: a player may legitimately be waiting for its first frame.
    pub fn present_frame(&mut self) -> bool {
        let Some(frame) = self.frame.as_ref() else {
            return false;
        };
        let pts = frame.pts_ms;
        self.pipeline = VideoPipelineState::Presenting;
        self.frame_presented.emit(pts);
        self.base.request_redraw();
        true
    }

    /// The most recent host frame, if any.
    pub fn video_frame(&self) -> Option<&VideoFrame> {
        self.frame.as_ref()
    }

    /// The reported pipeline stage.
    pub fn pipeline(&self) -> VideoPipelineState {
        self.pipeline
    }

    /// Whether playback loops at the end of the timeline.
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// Turns end-of-timeline looping on or off.
    pub fn set_looping(&mut self, looping: bool) {
        if self.looping == looping {
            return;
        }
        self.looping = looping;
    }

    /// Stops playback and rewinds to the start, leaving the source loaded.
    pub fn stop(&mut self) {
        let was_active = self.is_playing;
        self.is_playing = false;
        self.current_time = 0.0;
        self.subtitle_cursor.set(0);
        self.pipeline = if self.source.is_empty() {
            VideoPipelineState::Idle
        } else {
            VideoPipelineState::Demuxing
        };
        self.set_status(if self.source.is_empty() {
            MediaStatus::Idle
        } else {
            MediaStatus::Paused
        });
        if was_active {
            self.playback_paused.emit();
        }
        self.time_updated.emit(0.0);
        self.base.request_redraw();
    }

    /// Replaces the host-supplied audio track list.
    pub fn set_audio_tracks(&mut self, tracks: Vec<AudioTrack>) {
        self.audio_tracks = tracks;
        self.active_audio_track =
            self.active_audio_track.filter(|index| *index < self.audio_tracks.len());
        self.base.request_redraw();
    }

    /// The host-supplied audio tracks.
    pub fn audio_tracks(&self) -> &[AudioTrack] {
        &self.audio_tracks
    }

    /// Selects the audio track at `index`.
    ///
    /// Returns `Err` for an index the host never supplied rather than silently selecting
    /// nothing, which would make a caller believe the switch happened.
    pub fn set_audio_track(&mut self, index: usize) -> Result<(), String> {
        if index >= self.audio_tracks.len() {
            return Err(format!(
                "audio track {index} is out of range; the host supplied {} track(s)",
                self.audio_tracks.len()
            ));
        }
        if self.active_audio_track == Some(index) {
            return Ok(());
        }
        self.active_audio_track = Some(index);
        self.base.request_redraw();
        Ok(())
    }

    /// The selected audio track index, when one is selected.
    pub fn audio_track(&self) -> Option<usize> {
        self.active_audio_track
    }

    /// Replaces the host-supplied subtitle tracks, keeping the active track when it still exists.
    pub fn set_subtitle_tracks(&mut self, tracks: Vec<SubtitleTrack>) {
        self.subtitle_tracks = tracks;
        self.active_subtitle_track =
            self.active_subtitle_track.filter(|index| *index < self.subtitle_tracks.len());
        if let Some(index) = self.active_subtitle_track {
            self.set_subtitles(self.subtitle_tracks[index].cues.clone());
        }
        self.base.request_redraw();
    }

    /// The host-supplied subtitle tracks.
    pub fn subtitle_tracks(&self) -> &[SubtitleTrack] {
        &self.subtitle_tracks
    }

    /// Selects the subtitle track at `index`, replacing the active cue list with its cues.
    pub fn set_subtitle_track(&mut self, index: usize) -> Result<(), String> {
        if index >= self.subtitle_tracks.len() {
            return Err(format!(
                "subtitle track {index} is out of range; the host supplied {} track(s)",
                self.subtitle_tracks.len()
            ));
        }
        self.active_subtitle_track = Some(index);
        let cues = self.subtitle_tracks[index].cues.clone();
        self.set_subtitles(cues);
        Ok(())
    }

    /// The selected subtitle track index, when one is selected.
    pub fn subtitle_track(&self) -> Option<usize> {
        self.active_subtitle_track
    }

    /// The container metadata reported by the host.
    pub fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }

    /// Replaces the container metadata and announces it through `metadata_changed`.
    pub fn set_metadata(&mut self, metadata: VideoMetadata) {
        self.metadata = metadata;
        self.metadata_changed.emit();
        self.base.request_redraw();
    }

    /// Changes the reported media status, emitting only on a real change.
    fn set_status(&mut self, status: MediaStatus) {
        if self.status != status {
            self.status = status;
            self.media_status_changed.emit(status);
        }
    }

    /// Returns the reported media status.
    pub fn media_status(&self) -> MediaStatus {
        self.status
    }

    /// Replaces the subtitle cue list.
    ///
    /// Cues are sorted by start time here, so a host may append them in any order. Emptying the
    /// list removes the subtitle layer entirely.
    pub fn set_subtitles(&mut self, mut cues: Vec<SubtitleCue>) {
        cues.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(core::cmp::Ordering::Equal));
        self.subtitles = cues;
        self.subtitle_cursor.set(0);
        self.base.request_redraw();
    }

    /// Appends a subtitle cue.
    pub fn add_subtitle(&mut self, cue: SubtitleCue) {
        self.subtitles.push(cue);
        self.subtitles
            .sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(core::cmp::Ordering::Equal));
        self.subtitle_cursor.set(0);
        self.base.request_redraw();
    }

    /// Removes every subtitle cue.
    pub fn clear_subtitles(&mut self) {
        if self.subtitles.is_empty() {
            return;
        }
        self.subtitles.clear();
        self.subtitle_cursor.set(0);
        self.base.request_redraw();
    }

    /// Returns the cue visible at `time`, if any.
    ///
    /// # Why this is not a linear scan
    ///
    /// `draw` calls this every frame, so a full `iter().find` made the cost grow with the length
    /// of the subtitle track. The cues are sorted by start here, so a monotonically advancing
    /// clock keeps a cursor and answers in one comparison; a backwards seek binary-searches for
    /// the first cue that could contain `time` instead of walking the whole list. Overlapping
    /// cues are still handled by walking back over any earlier cue whose end is past `time`.
    pub fn subtitle_at(&self, time: f64) -> Option<&SubtitleCue> {
        if self.subtitles.is_empty() {
            return None;
        }
        let mut index = self.subtitle_cursor.get().min(self.subtitles.len() - 1);
        if index >= self.subtitles.len() || self.subtitles[index].start > time {
            // The clock moved backwards: find the last cue whose start is at or before `time`.
            index = self.subtitles.partition_point(|cue| cue.start <= time).saturating_sub(1);
        } else {
            while index + 1 < self.subtitles.len() && self.subtitles[index + 1].start <= time {
                index += 1;
            }
        }
        self.subtitle_cursor.set(index);
        // At most one cue normally contains a given time. Walking back past an earlier cue whose
        // end is still ahead of `time` keeps overlapping tracks correct without making the common
        // (non-overlapping) case anything but O(1).
        let mut probe = index + 1;
        while probe > 0 {
            probe -= 1;
            let cue = &self.subtitles[probe];
            if time >= cue.start && time < cue.end {
                return Some(cue);
            }
            if cue.end <= time {
                break;
            }
        }
        None
    }

    /// Whether subtitle cues are painted.
    pub fn subtitles_enabled(&self) -> bool {
        self.subtitles_enabled
    }

    /// Enables or disables subtitle painting.
    pub fn set_subtitles_enabled(&mut self, enabled: bool) {
        if self.subtitles_enabled == enabled {
            return;
        }
        self.subtitles_enabled = enabled;
        self.base.request_redraw();
    }

    /// Whether the player is in fullscreen presentation.
    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Enters or leaves fullscreen presentation, emitting `fullscreen_changed`.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        if self.fullscreen == fullscreen {
            return;
        }
        self.fullscreen = fullscreen;
        self.fullscreen_changed.emit(fullscreen);
        self.base.request_redraw();
    }

    /// Toggles fullscreen presentation.
    pub fn toggle_fullscreen(&mut self) {
        self.set_fullscreen(!self.fullscreen);
    }

    /// The buffered position reported by the host, in the same unit as the clock.
    pub fn buffered(&self) -> f64 {
        self.buffered
    }

    /// Reports how much of the media is buffered.
    ///
    /// The player does no network I/O, so this is host-supplied. It is clamped into the timeline
    /// and painted as a second, quieter segment of the seek bar.
    pub fn set_buffered(&mut self, buffered: f64) {
        if !buffered.is_finite() {
            return;
        }
        let buffered = buffered.clamp(0.0, self.duration.max(0.0));
        if self.buffered == buffered {
            return;
        }
        self.buffered = buffered;
        self.buffering_progress_changed.emit(buffered);
        self.base.request_redraw();
    }

    /// The buffered fraction, `0.0..=1.0`.
    pub fn buffering_progress(&self) -> f64 {
        if self.duration > 0.0 {
            (self.buffered / self.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Reports a playback failure, moving the status to [`MediaStatus::Error`].
    pub fn report_error(&mut self, message: impl Into<String>) {
        self.set_status(MediaStatus::Error);
        self.error_occurred.emit(message.into());
    }

    /// Returns the source URI.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Sets the simulated video duration in seconds used by the clock, seek bar
    /// and time display. The value comes from the caller; it is not measured
    /// from any media file. Non-finite values are ignored. If the duration is
    /// shortened below the current position, the clock is clamped and emits
    /// `time_updated` with the new position.
    pub fn set_duration(&mut self, duration: f64) {
        if !duration.is_finite() {
            return;
        }
        let duration = duration.max(0.0);
        if self.duration == duration {
            return;
        }
        self.duration = duration;
        self.duration_changed.emit(duration);
        let clamped_time = self.current_time.clamp(0.0, self.duration);
        if clamped_time != self.current_time {
            self.current_time = clamped_time;
            self.time_updated.emit(self.current_time);
        }
        self.base.request_redraw();
    }

    /// Starts simulated playback.
    ///
    /// Marks the player as playing and emits `playback_started`. No media
    /// frames are decoded; the position advances only when `tick` is called.
    pub fn play(&mut self) {
        if self.source.is_empty() {
            return;
        }
        if !self.is_playing {
            self.is_playing = true;
            self.pipeline = VideoPipelineState::Decoding;
            self.set_status(MediaStatus::Playing);
            self.playback_started.emit();
            self.base.request_redraw();
        }
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        if self.is_playing {
            self.is_playing = false;
            self.set_status(MediaStatus::Paused);
            self.playback_paused.emit();
            self.base.request_redraw();
        }
    }

    /// Toggles between play and pause.
    pub fn toggle_play(&mut self) {
        if self.is_playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Returns whether playback is active.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Seeks the simulated playback clock to the given time in seconds
    /// (clamped to 0..duration). Does not read any media data.
    ///
    /// A non-finite request (`NaN`, `±inf`) has no position on the timeline; it is ignored so it
    /// cannot poison `current_time`, which every later `progress` / `tick` / `time_updated` reads.
    pub fn seek(&mut self, time: f64) {
        if !time.is_finite() {
            return;
        }
        self.current_time = time.clamp(0.0, self.duration.max(0.0));
        self.subtitle_cursor.set(0);
        self.time_updated.emit(self.current_time);
        self.base.request_redraw();
    }

    /// Returns the current playback time in seconds.
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// Returns the total duration in seconds.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Sets the volume level (0.0 = silent, 1.0 = full).
    pub fn set_volume(&mut self, vol: f32) {
        let volume = vol.clamp(0.0, 1.0);
        if self.volume == volume {
            return;
        }
        self.volume = volume;
        self.volume_changed.emit(volume);
        self.base.request_redraw();
    }

    /// Returns the current volume level.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Sets whether audio is muted.
    pub fn set_muted(&mut self, muted: bool) {
        if self.muted == muted {
            return;
        }
        self.muted = muted;
        self.mute_changed.emit(muted);
        self.base.request_redraw();
    }

    /// Returns whether audio is muted.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Sets the playback rate multiplier.
    pub fn set_playback_rate(&mut self, rate: f32) {
        if !rate.is_finite() {
            return;
        }
        let rate = rate.max(0.1);
        if self.playback_rate == rate {
            return;
        }
        self.playback_rate = rate;
        self.playback_rate_changed.emit(rate);
        self.base.request_redraw();
    }

    /// Returns the current playback rate.
    pub fn playback_rate(&self) -> f32 {
        self.playback_rate
    }

    /// Shows the control bar overlay.
    pub fn show_controls(&mut self) {
        self.controls_visible = true;
        self.base.request_redraw();
    }

    /// Hides the control bar overlay.
    pub fn hide_controls(&mut self) {
        self.controls_visible = false;
        self.base.request_redraw();
    }

    /// Returns whether controls are visible.
    pub fn controls_visible(&self) -> bool {
        self.controls_visible
    }

    /// Advances the simulated playback clock by the given number of seconds
    /// (scaled by playback rate).
    /// This simulates video frame advancement; no media is decoded.
    /// Returns true if playback reached the end.
    ///
    /// A non-finite or negative `delta_secs` is rejected: a clock that runs on `NaN` never satisfies
    /// `current_time >= duration` (all comparisons with `NaN` are false), so the control would keep
    /// emitting `time_updated` with a `NaN` payload forever, and a negative delta would run the
    /// clock backwards past zero. Both are input errors with no timeline position, so the tick is a
    /// no-op that reports "not finished" rather than corrupting the state.
    pub fn tick(&mut self, delta_secs: f64) -> bool {
        if !self.is_playing || self.duration <= 0.0 || !delta_secs.is_finite() || delta_secs < 0.0 {
            return false;
        }

        let effective_delta = delta_secs * self.playback_rate as f64;
        // `playback_rate` is a finite `f32` clamped positive, so `effective_delta` stays finite; the
        // clamp below is the belt-and-braces that keeps `current_time` in `[0, duration]` regardless.
        self.current_time = (self.current_time + effective_delta).clamp(0.0, self.duration);

        if self.current_time >= self.duration {
            if self.looping {
                // A looping timeline wraps without reporting the end: a listener that tears down
                // its state on `playback_ended` must not be told the clip finished.
                self.current_time = 0.0;
                self.subtitle_cursor.set(0);
                self.pipeline = VideoPipelineState::Decoding;
                self.time_updated.emit(self.current_time);
                self.base.request_redraw();
                return false;
            }
            self.current_time = self.duration;
            self.is_playing = false;
            self.pipeline = VideoPipelineState::Ended;
            self.set_status(MediaStatus::Ended);
            self.playback_ended.emit();
            self.time_updated.emit(self.current_time);
            self.base.request_redraw();
            return true;
        }

        self.time_updated.emit(self.current_time);
        self.base.request_redraw();
        false
    }

    /// Returns the current playback progress as a fraction (0.0 – 1.0).
    pub fn progress(&self) -> f64 {
        if self.duration > 0.0 {
            (self.current_time / self.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl Widget for VideoPlayer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 240)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `VideoPlayer` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    // Trait contract: `u32` ms. This control's own `tick` takes seconds as `f64` because
    // playback position is a media clock rather than a frame delta; convert at the boundary.
    fn tick(&mut self, delta_ms: u32) -> bool {
        VideoPlayer::tick(self, f64::from(delta_ms) / 1000.0)
    }

    fn is_animating(&self) -> bool {
        self.is_playing()
    }
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "playback_started" => {
                Some(EventSignalRef::unit("playback_started", &self.playback_started))
            }
            "playback_paused" => {
                Some(EventSignalRef::unit("playback_paused", &self.playback_paused))
            }
            "playback_ended" => Some(EventSignalRef::unit("playback_ended", &self.playback_ended)),
            "time_updated" => {
                Some(EventSignalRef::mapped("time_updated", &self.time_updated, |v| {
                    CapabilityValue::Float(*v)
                }))
            }
            "buffering_progress_changed" => Some(EventSignalRef::mapped(
                "buffering_progress_changed",
                &self.buffering_progress_changed,
                |v| crate::widget::capability::CapabilityValue::Float(*v),
            )),
            "error_occurred" => {
                Some(EventSignalRef::mapped("error_occurred", &self.error_occurred, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "fullscreen_changed" => {
                Some(EventSignalRef::mapped("fullscreen_changed", &self.fullscreen_changed, |v| {
                    crate::widget::capability::CapabilityValue::Bool(*v)
                }))
            }
            "media_status_changed" => Some(EventSignalRef::mapped(
                "media_status_changed",
                &self.media_status_changed,
                |v| crate::widget::capability::CapabilityValue::String(format!("{v:?}")),
            )),
            "mute_changed" => {
                Some(EventSignalRef::mapped("mute_changed", &self.mute_changed, |v| {
                    crate::widget::capability::CapabilityValue::Bool(*v)
                }))
            }
            "playback_rate_changed" => Some(EventSignalRef::mapped(
                "playback_rate_changed",
                &self.playback_rate_changed,
                |v| crate::widget::capability::CapabilityValue::Float(*v as f64),
            )),
            "volume_changed" => {
                Some(EventSignalRef::mapped("volume_changed", &self.volume_changed, |v| {
                    crate::widget::capability::CapabilityValue::Float(*v as f64)
                }))
            }
            "duration_changed" => {
                Some(EventSignalRef::mapped("duration_changed", &self.duration_changed, |v| {
                    crate::widget::capability::CapabilityValue::Float(*v)
                }))
            }
            "frame_available" => {
                Some(EventSignalRef::mapped("frame_available", &self.frame_available, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v)
                }))
            }
            "frame_presented" => {
                Some(EventSignalRef::mapped("frame_presented", &self.frame_presented, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v)
                }))
            }
            "metadata_changed" => {
                Some(EventSignalRef::unit("metadata_changed", &self.metadata_changed))
            }
            _ => None,
        }
    }
}

/// `VideoPlayer`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch.
impl WidgetProperties for VideoPlayer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "is_playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            "volume" => Ok(CapabilityValue::Float(self.volume() as f64)),
            "muted" => Ok(CapabilityValue::Bool(self.is_muted())),
            "playback_rate" => Ok(CapabilityValue::Float(self.playback_rate() as f64)),
            "controls_visible" => Ok(CapabilityValue::Bool(self.controls_visible())),
            "looping" => Ok(CapabilityValue::Bool(self.is_looping())),
            "pipeline" => Ok(CapabilityValue::String(format!("{:?}", self.pipeline()))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "is_playing" => {
                if expect_bool(value)? {
                    self.play();
                } else {
                    self.pause();
                }
                Ok(())
            }
            "volume" => {
                self.set_volume(expect_f32(value)?);
                Ok(())
            }
            "muted" => {
                self.set_muted(expect_bool(value)?);
                Ok(())
            }
            "playback_rate" => {
                self.set_playback_rate(expect_f32(value)?);
                Ok(())
            }
            "controls_visible" => {
                if expect_bool(value)? {
                    self.show_controls();
                } else {
                    self.hide_controls();
                }
                Ok(())
            }
            "looping" => {
                self.set_looping(expect_bool(value)?);
                Ok(())
            }
            // The pipeline stage is reported by the host-driven lifecycle, not assigned.
            "pipeline" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "is_playing",
            "volume",
            "muted",
            "playback_rate",
            "controls_visible",
            "looping",
            "pipeline",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for VideoPlayer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // # Which of this control's colours are chrome and which are the video's own frame
        //
        // A media player deliberately draws its *transport* chrome in white on a black band:
        // the picture beneath is arbitrary, so an overlay that took the page's ink would be
        // unreadable over a light frame and disappear over a dark one. Those literals are kept,
        // and they are the reason `audit_appearance.py` still counts literals in this file.
        //
        // Two things are **not** the video's frame, and those follow the theme:
        //
        //   * the **empty state** -- there is no picture yet, so the rectangle is a placeholder
        //     panel, and a panel belongs to the page's surface scale (`surface_container`);
        //   * the **seek fill** -- a value indicator, so it is the accent, exactly as a slider's
        //     fill or a progress bar's is. It was a fixed `rgb(60,140,240)` that no theme could
        //     move.
        let (accent, placeholder, placeholder_ink, window_fill) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.primary,
                    active.colors.surface_container,
                    active.colors.secondary,
                    active.colors.background,
                ),
                None => (
                    Color::rgba(60, 140, 240, 230),
                    Color::rgba(30, 30, 30, 255),
                    Color::rgba(180, 180, 180, 220),
                    Color::WHITE,
                ),
            }
        };

        // ── Video area ──────────────────────────────────────────────────
        let video_bg = if !is_enabled {
            // A disabled player is the page's own surface, veiled, rather than a fixed
            // translucent grey that read as a light slab in a dark appearance.
            self.base.disabled_surface_near(placeholder, window_fill)
        } else if self.source.is_empty() {
            placeholder
        } else {
            Color::rgba(20, 20, 40, 255)
        };
        context.face_with_gradient(
            rect,
            video_bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect_stroke(
            rect,
            self.style().border_color.unwrap_or_else(|| video_bg.blend(&Color::WHITE, 0.2)),
            1,
        );

        // The control's chrome text (placeholder, time, rate) takes the **effective font**, so the
        // theme body font and the user's text scale reach it (D09-STYLE-01).
        let font = effective_font(self.style());

        if self.source.is_empty() {
            // Empty state. Fitted to the control's width so a narrow player truncates the
            // label instead of drawing it past its own edge.
            let text = "No video loaded";
            let metrics = context.measure_text(text, font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            context.draw_text_fitted_cow(
                line,
                text,
                font,
                placeholder_ink,
                HorizontalAlignment::Center,
            );
            return;
        }

        // ── Host-decoded frame ──
        //
        // The control owns no decoder, so the picture is exactly the buffer the host injected.
        // It is centred at its native pixel size and clipped to the player: scaling here would
        // invent a resampling policy the pipeline did not ask for.
        if let Some(frame) = self.frame.as_ref() {
            let dx = rect.x + (rect.width as i32 - frame.width as i32) / 2;
            let dy = rect.y + (rect.height as i32 - frame.height as i32) / 2;
            context.push_clip(rect.x, rect.y, rect.width, rect.height);
            context.draw_image(dx, dy, frame.width, frame.height, &frame.data);
            context.pop_clip();
        }

        // Play icon overlay in center when paused — the `Play` **icon outline**, not the `▶`
        // text glyph (U+25B6 is covered by no bundled face, so it drew as an 8x8 bitmap block).
        if !self.is_playing {
            let side = (rect.height / 3).clamp(24, 96);
            crate::widget::draw_icon_centered(
                context,
                rect,
                side,
                Color::rgba(255, 255, 255, 180),
                IconName::Play,
            );
        }

        // ── Subtitle layer ──
        let control_bar_top =
            ControlMetrics::bottom_band(rect, dimensions::VIDEO_CONTROL_BAR_HEIGHT).y;
        //
        // Drawn above the transport bar so a cue never sits under the controls. The band is
        // fitted to the player's width, and a cue outside its own time range draws nothing.
        if self.subtitles_enabled {
            if let Some(cue) = self.subtitle_at(self.current_time) {
                let band_height = (rect.height as f32 * 0.16).clamp(18.0, 40.0) as u32;
                let band = Rect::new(
                    rect.x + 8,
                    control_bar_top - band_height as i32 - 8,
                    rect.width.saturating_sub(16),
                    band_height,
                );
                if band.y > rect.y {
                    context.fill_rounded_rect(band, 4, Color::rgba(0, 0, 0, 150));
                    context.draw_text_fitted_cow(
                        context.text_line(band, font),
                        &cue.text,
                        font,
                        Color::WHITE,
                        HorizontalAlignment::Center,
                    );
                }
            }
        }

        // ── Control bar ─────────────────────────────────────────────────
        if !self.controls_visible {
            return;
        }

        // The transport bar is a fixed overlay pinned to the video surface's bottom edge; its
        // height is the player's own fact, not a fraction of the surface it covers, so it stays
        // 36 px whether the player is a 320x240 panel or a full-screen canvas.
        let control_bar_height = dimensions::VIDEO_CONTROL_BAR_HEIGHT;
        let control_bar_rect = ControlMetrics::bottom_band(rect, control_bar_height);
        let control_bar_y = control_bar_rect.y;
        context.fill_rect(control_bar_rect, Color::rgba(0, 0, 0, 160));

        // Play/Pause button: a `Play`/`Pause` **icon outline**, not the `▶`/`⏸` text glyphs
        // (U+25B6 and U+23F8 are covered by no bundled face).
        let btn_icon = if self.is_playing { IconName::Pause } else { IconName::Play };
        let btn_side = 16u32;
        let btn_box = Rect::new(rect.x + 8, control_bar_y, btn_side, control_bar_height);
        crate::widget::draw_icon_centered(context, btn_box, btn_side, Color::WHITE, btn_icon);

        // Seek bar: the shared thickness, centred on the transport bar's own middle line.
        //
        // The right-hand items are laid out from the right edge first, then the time label
        // is placed left of them, and only then is the seek bar given what remains. The old
        // fixed 80 px reservation held an 11-glyph time string (≈74 px at 14 px) and left no
        // room for a playback-rate label, which was then drawn over the fullscreen icon.
        let seek_bar_x = rect.x + 8 + btn_side as i32 + 12;
        let seek_bar_y = control_bar_y
            + (control_bar_height as i32 - dimensions::VIDEO_SEEK_BAR_HEIGHT as i32) / 2;
        let seek_bar_height = dimensions::VIDEO_SEEK_BAR_HEIGHT;
        // Both labels render in place: `format!` built a fresh `String` for the timestamp
        // and another for the rate every frame.
        let mut time_text = String::new();
        let _ = write!(
            time_text,
            "{:02}:{:02}/{:02}:{:02}",
            (self.current_time as u32) / 60,
            (self.current_time as u32) % 60,
            (self.duration as u32) / 60,
            (self.duration as u32) % 60,
        );
        let time_metrics = context.measure_text(&time_text, font);
        let time_width = time_metrics.width as i32;
        let vol_side = 16u32;
        let vol_box = Rect::new(
            rect.x + rect.width as i32 - vol_side as i32 - 8,
            control_bar_y,
            vol_side,
            control_bar_height,
        );
        let fs_side = 16u32;
        let fs_box =
            Rect::new(vol_box.x - fs_side as i32 - 24, control_bar_y, fs_side, control_bar_height);
        // The rate label is measured once and the metrics reused for both the layout and
        // the draw; the old form measured it twice and allocated the text as an `Option<String>`.
        let has_rate = (self.playback_rate - 1.0).abs() > 0.01;
        let mut rate_text = String::new();
        if has_rate {
            let _ = write!(rate_text, "{:.1}x", self.playback_rate);
        }
        let rate_metrics =
            if has_rate { Some(context.measure_text(&rate_text, font)) } else { None };
        let rate_width = rate_metrics.as_ref().map(|m| m.width as i32).unwrap_or(0);
        // The rate label sits left of the fullscreen icon; the time label sits left of the
        // whole right-hand group. Both gaps are real, so neither label can overlap an icon.
        let rate_x = fs_box.x - rate_width - 8;
        let time_x = if has_rate { rate_x } else { fs_box.x } - time_width - 8;
        let seek_bar_width = (time_x - 8 - seek_bar_x).max(0) as u32;
        let seek_bar_full = Rect::new(seek_bar_x, seek_bar_y, seek_bar_width, seek_bar_height);
        context.fill_rounded_rect(
            seek_bar_full,
            seek_bar_height / 2,
            Color::rgba(100, 100, 100, 200),
        );

        // The buffered segment sits between the track and the played fill. Its default is zero,
        // so a player whose host reports nothing paints exactly the bar it always did.
        let buffered_width = (seek_bar_full.width as f64 * self.buffering_progress()) as u32;
        if buffered_width > 0 {
            context.fill_rounded_rect(
                Rect::new(seek_bar_full.x, seek_bar_full.y, buffered_width, seek_bar_full.height),
                seek_bar_height / 2,
                Color::rgba(160, 160, 160, 220),
            );
        }

        let fill_width = (seek_bar_full.width as f64 * self.progress()) as u32;
        if fill_width > 0 {
            let seek_bar_fill =
                Rect::new(seek_bar_full.x, seek_bar_full.y, fill_width, seek_bar_full.height);
            context.fill_rounded_rect(seek_bar_fill, seek_bar_height / 2, accent);
        }

        // Time display, placed in the gap the right-hand group left for it.
        let time_y = control_bar_y + (control_bar_height as i32 - time_metrics.height as i32) / 2;
        context.draw_text(
            Point::new(time_x, time_y),
            &time_text,
            font,
            Color::rgba(220, 220, 220, 230),
            HorizontalAlignment::Left,
        );

        // Volume icon and mute button: a `VolumeOff`/`VolumeUp` **icon outline**, not the
        // `🔇`/`🔉`/`🔊` emoji (no bundled face covers them, and a colour emoji here is not what a
        // transport bar wants). Muted is the crossed speaker; otherwise the plain speaker.
        let vol_icon =
            if self.muted || self.volume < 0.01 { IconName::VolumeOff } else { IconName::VolumeUp };
        crate::widget::draw_icon_centered(context, vol_box, vol_side, Color::WHITE, vol_icon);

        // Playback rate indicator, in the slot reserved left of the fullscreen button.
        if let Some(metrics) = rate_metrics.as_ref() {
            let rate_y = control_bar_y + (control_bar_height as i32 - metrics.height as i32) / 2;
            context.draw_text(
                Point::new(rate_x, rate_y),
                &rate_text,
                font,
                Color::rgba(255, 220, 100, 230),
                HorizontalAlignment::Left,
            );
        }

        // Fullscreen button: a `Fullscreen` **icon outline**, not the `⛶` text glyph (U+26F6 is
        // covered by no bundled face).
        crate::widget::draw_icon_centered(
            context,
            fs_box,
            fs_side,
            Color::WHITE,
            IconName::Fullscreen,
        );
    }
}

impl EventHandler for VideoPlayer {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    let rect = self.geometry();
                    if !rect.contains_point(*pos) {
                        return;
                    }

                    // Check if clicked on the play/pause button area.
                    let control_bar_height = 36u32;
                    let control_bar_y = rect.y + rect.height as i32 - control_bar_height as i32;

                    if pos.y >= control_bar_y
                        && pos.y < rect.y + rect.height as i32
                        && self.controls_visible
                    {
                        // Play/pause button click area. The box matches the drawn button: a
                        // 16 px icon in a strip at `x + 8`, full transport-bar height, so the hit
                        // target and the ink are the same object (a glyph-measured box here and a
                        // fixed icon box in `draw` would disagree on where the button is).
                        let btn_rect = Rect::new(rect.x + 8, control_bar_y, 16, control_bar_height);
                        if btn_rect.contains_point(*pos) {
                            self.toggle_play();
                            return;
                        }
                        // Fullscreen button, in the slot the draw path reserves for it.
                        let fs_box = Rect::new(
                            rect.x + rect.width as i32 - 16 - 8 - 16 - 24,
                            control_bar_y,
                            16,
                            control_bar_height,
                        );
                        if fs_box.contains_point(*pos) {
                            self.toggle_fullscreen();
                            return;
                        }
                    }

                    // Click on video area toggles controls visibility or play/pause.
                    if pos.y < control_bar_y {
                        if self.controls_visible {
                            self.toggle_play();
                        } else {
                            self.show_controls();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                match (key, modifiers) {
                    // Space bar toggles play/pause.
                    (32, _) => {
                        self.toggle_play();
                    }
                    // Left arrow seeks backward 5 seconds.
                    (37, _) => {
                        self.seek(self.current_time - 5.0);
                    }
                    // Right arrow seeks forward 5 seconds.
                    (39, _) => {
                        self.seek(self.current_time + 5.0);
                    }
                    // Up arrow increases volume.
                    (38, _) => {
                        self.set_volume(self.volume + 0.1);
                    }
                    // Down arrow decreases volume.
                    (40, _) => {
                        self.set_volume(self.volume - 0.1);
                    }
                    // M key toggles mute.
                    (109, _) | (77, _) => {
                        self.set_muted(!self.muted);
                    }
                    // F key toggles fullscreen.
                    (102, _) | (70, _) => {
                        self.toggle_fullscreen();
                    }
                    _ => {}
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Renders the player over `backdrop` and returns its RGBA frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn render_on(vp: &mut VideoPlayer, size: crate::core::Size, backdrop: Color) -> Vec<u8> {
        use crate::render::{PaintBackend, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(backdrop);
        let mut context = RenderContext::new(&mut backend);
        vp.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The RGBA pixel at `(x, y)` of a `size`-wide frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn pixel(frame: &[u8], size: crate::core::Size, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * size.width + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    }

    /// The empty-state panel follows the appearance rather than a fixed near-black.
    ///
    /// # What is themed here, and what deliberately is not
    ///
    /// The transport chrome stays white on a black band on purpose: the picture beneath is
    /// arbitrary, so an overlay that took the page's ink would be unreadable over a light frame.
    /// The **empty state** is different — there is no picture, so the rectangle is a placeholder
    /// panel and belongs to the page's surface scale. It used to be a fixed `rgb(30,30,30)`, which on
    /// a light appearance was a black rectangle where every other placeholder in the crate is a
    /// surface.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_empty_state_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 320, 240);
        let size = crate::core::Size::new(320, 240);

        let centre = |appearance| -> [u8; 4] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let backdrop = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let mut vp = VideoPlayer::new(rect);
            crate::theme::apply_theme_to_widget(&mut vp);
            let frame = render_on(&mut vp, size, backdrop);
            // No source is loaded, so the whole rectangle is the empty-state panel. The sample is
            // near the top-left, clear of the centred "No video loaded" label.
            pixel(&frame, size, 5, 5)
        };

        let dark = centre(crate::theme::AppearanceMode::Dark);
        let light = centre(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "the empty-state panel must follow the appearance; both were {dark:?}"
        );
    }

    #[test]
    fn video_player_creation_defaults() {
        let vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert_eq!(vp.source(), "");
        assert!(!vp.is_playing());
        assert_eq!(vp.current_time(), 0.0);
        assert_eq!(vp.duration(), 0.0);
        assert_eq!(vp.volume(), 0.8);
        assert!(!vp.is_muted());
        assert_eq!(vp.playback_rate(), 1.0);
        assert!(vp.controls_visible());
        assert_eq!(vp.kind(), WidgetKind::VideoPlayer);
    }

    #[test]
    fn video_player_load_play_pause_toggle() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test_video.mp4");
        assert_eq!(vp.source(), "test_video.mp4");

        assert!(!vp.is_playing());
        vp.play();
        assert!(vp.is_playing());
        vp.pause();
        assert!(!vp.is_playing());

        vp.toggle_play();
        assert!(vp.is_playing());
        vp.toggle_play();
        assert!(!vp.is_playing());
    }

    #[test]
    fn video_player_seek_and_time() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(120.0);

        assert_eq!(vp.current_time(), 0.0);
        vp.seek(60.0);
        assert_eq!(vp.current_time(), 60.0);
        vp.seek(200.0); // clamped
        assert_eq!(vp.current_time(), 120.0);
        vp.seek(-10.0); // clamped
        assert_eq!(vp.current_time(), 0.0);
    }

    #[test]
    fn video_player_volume_and_mute() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert_eq!(vp.volume(), 0.8);
        assert!(!vp.is_muted());

        vp.set_volume(0.5);
        assert_eq!(vp.volume(), 0.5);

        vp.set_volume(2.0); // clamped
        assert_eq!(vp.volume(), 1.0);

        vp.set_volume(-1.0); // clamped
        assert_eq!(vp.volume(), 0.0);

        vp.set_muted(true);
        assert!(vp.is_muted());
        vp.set_muted(false);
        assert!(!vp.is_muted());
    }

    #[test]
    fn video_player_playback_rate() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert_eq!(vp.playback_rate(), 1.0);
        vp.set_playback_rate(2.0);
        assert_eq!(vp.playback_rate(), 2.0);
        vp.set_playback_rate(0.0); // clamped
        assert_eq!(vp.playback_rate(), 0.1);
        vp.set_playback_rate(f32::INFINITY);
        assert_eq!(vp.playback_rate(), 0.1, "a non-finite rate must be ignored");
    }

    #[test]
    fn video_player_controls_visibility() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert!(vp.controls_visible());
        vp.hide_controls();
        assert!(!vp.controls_visible());
        vp.show_controls();
        assert!(vp.controls_visible());
    }

    #[test]
    fn video_player_progress() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(100.0);
        assert_eq!(vp.progress(), 0.0);
        vp.seek(50.0);
        assert!((vp.progress() - 0.5).abs() < 0.001);
        vp.seek(100.0);
        assert!((vp.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn video_player_tick_advances_time() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(60.0);
        vp.play();

        assert_eq!(vp.current_time(), 0.0);
        vp.tick(10.0);
        assert!((vp.current_time() - 10.0).abs() < 0.001);
        vp.tick(20.0);
        assert!((vp.current_time() - 30.0).abs() < 0.001);
    }

    #[test]
    fn shrinking_duration_clamps_the_clock_and_emits_the_new_time() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(60.0);
        vp.seek(20.0);

        let observed = Arc::new(Mutex::new(None));
        vp.time_updated.connect({
            let observed = Arc::clone(&observed);
            move |time| {
                *observed.lock().unwrap() = Some(*time);
            }
        });

        vp.set_duration(10.0);
        assert_eq!(vp.current_time(), 10.0);
        assert_eq!(*observed.lock().unwrap(), Some(10.0));

        vp.set_duration(f64::INFINITY);
        assert_eq!(vp.duration(), 10.0, "a non-finite duration must be ignored");
    }

    #[test]
    fn video_player_tick_playback_ended() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(10.0);
        vp.play();

        let ended = Arc::new(Mutex::new(false));
        vp.playback_ended.connect({
            let ended = Arc::clone(&ended);
            move || {
                *ended.lock().unwrap() = true;
            }
        });

        let finished = vp.tick(15.0);
        assert!(finished);
        assert!(*ended.lock().unwrap());
        assert!(!vp.is_playing());
        assert_eq!(vp.current_time(), 10.0);
    }

    /// A negative or non-finite `tick` delta must not move the clock or emit a bad value.
    ///
    /// # What this pins
    ///
    /// `tick` used to add the delta unchecked, so `tick(-10)` on a clock at 5 s gave −5 s and a
    /// `NaN` delta made the clock `NaN` forever — every `current_time >= duration` comparison then
    /// being false, so the control emitted `time_updated` with a `NaN` payload on every tick and
    /// never reported the end. Both are rejected now, leaving the clock exactly where it was.
    #[test]
    fn an_invalid_tick_delta_does_not_corrupt_the_clock() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(60.0);
        vp.play();
        vp.seek(5.0);

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -10.0] {
            let finished = vp.tick(bad);
            assert!(!finished, "an invalid delta must not report playback finished");
            assert!(vp.current_time().is_finite(), "the clock must stay finite after tick({bad})");
            assert_eq!(vp.current_time(), 5.0, "the clock must not move for tick({bad})");
            assert!(vp.is_playing(), "an invalid delta must not stop playback");
        }

        // A valid delta still advances and stays within `[0, duration]`.
        vp.tick(10.0);
        assert!((vp.current_time() - 15.0).abs() < 1e-9);
    }

    /// A non-finite `seek` is ignored rather than poisoning the clock.
    #[test]
    fn a_non_finite_seek_is_ignored() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(60.0);
        vp.seek(20.0);
        let before = vp.current_time();
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            vp.seek(bad);
            assert_eq!(vp.current_time(), before, "seek({bad}) must be a no-op");
        }
        assert!(vp.progress().is_finite(), "progress must stay finite");
    }

    /// The live setters are reachable through the property channel too.
    ///
    /// `muted`, `playback_rate` and `controls_visible` were one method away on the type but
    /// absent from `get`/`set`/`property_names`/the capability table, so a host driving the
    /// player through the designer route could not mute it or change its rate.
    #[test]
    fn video_player_property_channel_covers_the_live_setters() {
        use crate::widget::capability::WidgetProperties;
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));

        vp.set("muted", CapabilityValue::Bool(true)).unwrap();
        assert!(vp.is_muted());
        assert_eq!(vp.get("muted").unwrap(), CapabilityValue::Bool(true));

        vp.set("playback_rate", CapabilityValue::Float(1.5)).unwrap();
        assert!((vp.playback_rate() - 1.5).abs() < 1e-6);

        vp.set("controls_visible", CapabilityValue::Bool(false)).unwrap();
        assert!(!vp.controls_visible());
        vp.set("controls_visible", CapabilityValue::Bool(true)).unwrap();
        assert!(vp.controls_visible());

        for name in ["muted", "playback_rate", "controls_visible"] {
            assert!(vp.property_names().contains(&name), "{name} must be published");
        }
    }

    #[test]
    fn video_player_signals() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(30.0);

        let started = Arc::new(Mutex::new(false));
        let paused = Arc::new(Mutex::new(false));
        let last_time = Arc::new(Mutex::new(0.0));

        vp.playback_started.connect({
            let started = Arc::clone(&started);
            move || {
                *started.lock().unwrap() = true;
            }
        });
        vp.playback_paused.connect({
            let paused = Arc::clone(&paused);
            move || {
                *paused.lock().unwrap() = true;
            }
        });
        vp.time_updated.connect({
            let last_time = Arc::clone(&last_time);
            move |val: Arc<f64>| {
                *last_time.lock().unwrap() = *val;
            }
        });

        vp.play();
        assert!(*started.lock().unwrap());

        vp.seek(15.0);
        assert!((*last_time.lock().unwrap() - 15.0).abs() < 0.001);

        vp.pause();
        assert!(*paused.lock().unwrap());
    }

    #[test]
    fn subtitles_are_painted_only_inside_their_own_window() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_subtitles(vec![SubtitleCue::new(1.0, 3.0, "hello")]);
        assert_eq!(vp.subtitle_at(2.0).map(|cue| cue.text.as_str()), Some("hello"));
        assert!(vp.subtitle_at(3.5).is_none());
        vp.set_subtitles_enabled(false);
        assert!(!vp.subtitles_enabled());
    }

    #[test]
    fn the_fullscreen_button_is_clickable() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("movie.mp4");
        let fs_x = 320 - 16 - 8 - 16 - 24;
        vp.handle_event(&Event::MousePress {
            pos: Point::new(fs_x + 4, 240 - 18),
            button: 1,
            modifiers: 0,
        });
        assert!(vp.is_fullscreen());
    }

    #[test]
    fn buffered_progress_and_status_are_reported() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("movie.mp4");
        vp.set_duration(100.0);
        vp.set_buffered(25.0);
        assert!((vp.buffering_progress() - 0.25).abs() < 1e-6);
        vp.play();
        assert_eq!(vp.media_status(), MediaStatus::Playing);
        vp.report_error("decode failed");
        assert_eq!(vp.media_status(), MediaStatus::Error);
    }

    #[test]
    fn audio_settings_are_announced_for_a_host_output_path() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        let volume = std::sync::Arc::new(std::sync::Mutex::new(-1.0f32));
        let v = volume.clone();
        vp.volume_changed.connect(move |value: std::sync::Arc<f32>| {
            *v.lock().unwrap() = *value;
        });
        vp.set_volume(0.4);
        assert!((*volume.lock().unwrap() - 0.4).abs() < 1e-6);
        let muted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let m = muted.clone();
        vp.mute_changed.connect(move |value: std::sync::Arc<bool>| {
            m.store(*value, std::sync::atomic::Ordering::SeqCst);
        });
        vp.set_muted(true);
        assert!(muted.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_host_frame_is_validated_announced_and_painted() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("movie.mp4");
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX));
        let s = seen.clone();
        vp.frame_available.connect(move |pts: std::sync::Arc<u64>| {
            s.store(*pts, std::sync::atomic::Ordering::SeqCst);
        });

        assert!(
            vp.set_video_frame(VideoFrame { width: 2, height: 2, pts_ms: 42, data: vec![9u8; 8] })
                .is_err(),
            "a wrong byte count must be refused"
        );
        assert!(vp.video_frame().is_none());

        vp.set_video_frame(VideoFrame { width: 2, height: 2, pts_ms: 42, data: vec![9u8; 16] })
            .expect("a matching frame is accepted");
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 42);
        assert_eq!(vp.pipeline(), VideoPipelineState::Presenting);

        let presented = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX));
        let p = presented.clone();
        vp.frame_presented.connect(move |pts: std::sync::Arc<u64>| {
            p.store(*pts, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(vp.present_frame());
        assert_eq!(presented.load(std::sync::atomic::Ordering::SeqCst), 42);
    }

    #[test]
    fn looping_wraps_instead_of_reporting_the_end() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("loop.mp4");
        vp.set_duration(10.0);
        vp.set_looping(true);
        vp.play();
        let ended = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let e = ended.clone();
        vp.playback_ended.connect(move || {
            e.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(!vp.tick(15.0), "a looping clip does not finish");
        assert_eq!(vp.current_time(), 0.0);
        assert!(vp.is_playing());
        assert!(!ended.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn stop_rewinds_and_pauses() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("clip.mp4");
        vp.set_duration(20.0);
        vp.seek(5.0);
        vp.play();
        vp.stop();
        assert!(!vp.is_playing());
        assert_eq!(vp.current_time(), 0.0);
        assert_eq!(vp.media_status(), MediaStatus::Paused);
    }

    #[test]
    fn duration_changes_are_announced_even_without_a_clock_clamp() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(0.0f64));
        let s = seen.clone();
        vp.duration_changed.connect(move |value: std::sync::Arc<f64>| {
            *s.lock().unwrap() = *value;
        });
        vp.set_duration(30.0);
        assert_eq!(*seen.lock().unwrap(), 30.0);
    }

    #[test]
    fn subtitle_lookup_handles_long_tracks_and_backwards_seeks() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        let cues: Vec<SubtitleCue> = (0..500)
            .map(|index| {
                let start = index as f64 * 2.0;
                SubtitleCue::new(start, start + 1.0, format!("cue {index}"))
            })
            .collect();
        vp.set_subtitles(cues);
        for index in [0usize, 10, 100, 499] {
            let start = index as f64 * 2.0;
            let expected = format!("cue {index}");
            assert_eq!(
                vp.subtitle_at(start + 0.5).map(|cue| cue.text.as_str()),
                Some(expected.as_str())
            );
        }
        // A backwards seek must find an earlier cue rather than staying on the cursor.
        assert_eq!(vp.subtitle_at(0.5).map(|cue| cue.text.as_str()), Some("cue 0"));
        assert!(vp.subtitle_at(1.5).is_none(), "the gap between cues holds no cue");
    }

    #[test]
    fn audio_subtitle_tracks_and_metadata_are_selectable() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_audio_tracks(vec![
            AudioTrack::new("a1", "English"),
            AudioTrack::new("a2", "Deutsch"),
        ]);
        assert_eq!(vp.audio_tracks().len(), 2);
        assert!(vp.set_audio_track(2).is_err());
        vp.set_audio_track(1).expect("the second track exists");
        assert_eq!(vp.audio_track(), Some(1));

        vp.set_subtitle_tracks(vec![
            SubtitleTrack::new("s1", "English", vec![SubtitleCue::new(0.0, 1.0, "hello")]),
            SubtitleTrack::new("s2", "Deutsch", vec![SubtitleCue::new(0.0, 1.0, "hallo")]),
        ]);
        vp.set_subtitle_track(1).expect("the second track exists");
        assert_eq!(vp.subtitle_track(), Some(1));
        assert_eq!(vp.subtitle_at(0.5).map(|cue| cue.text.as_str()), Some("hallo"));

        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let s = seen.clone();
        vp.metadata_changed.connect(move || {
            s.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        vp.set_metadata(VideoMetadata {
            title: "Arrival".to_string(),
            artist: "Villeneuve".to_string(),
            artwork: None,
        });
        assert_eq!(vp.metadata().title, "Arrival");
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn the_new_events_resolve_and_loop_is_published() {
        use crate::widget::capability::WidgetProperties;
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        for name in ["duration_changed", "frame_available", "frame_presented", "metadata_changed"] {
            assert!(vp.event_signal_dyn(name).is_some(), "{name} must resolve");
        }
        assert!(vp.property_names().contains(&"looping"));
        assert!(vp.property_names().contains(&"pipeline"));
        vp.set("looping", CapabilityValue::Bool(true)).unwrap();
        assert!(vp.is_looping());
        assert_eq!(vp.get("pipeline").unwrap(), CapabilityValue::String("Idle".to_string()));
    }
}
