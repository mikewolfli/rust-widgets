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

use crate::compat::{Arc, Mutex, Vec};
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
use crate::widget::companions::PlaylistPanel;
use crate::widget::media_widgets::subtitle_bar::SubtitleBar;
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

/// A chapter marker on the playback timeline (MW-002).
#[derive(Debug, Clone, PartialEq)]
pub struct VideoChapter {
    /// The chapter's title, shown by a chapter list or the scrubber's readout.
    pub title: String,
    /// Where the chapter starts, in seconds from the beginning of the media.
    pub start_seconds: f64,
}

impl VideoChapter {
    /// A chapter titled `title` starting at `start_seconds`.
    pub fn new(title: impl Into<String>, start_seconds: f64) -> Self {
        Self { title: title.into(), start_seconds }
    }
}

/// An intent a transport bar queues for its player (MW-001).
///
/// A widget cannot hold a borrow of another, so a paired [`MediaTransportBar`] writes intents into
/// the player's shared queue and the player drains them on its next paint or interaction. Each
/// variant goes through the same public method a host would call, so the bar cannot bypass the
/// player's own validation.
///
/// [`MediaTransportBar`]: crate::widget::media_widgets::media_transport::MediaTransportBar
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportRequest {
    /// Start playback.
    Play,
    /// Stop playback.
    Pause,
    /// Flip the current playback state.
    TogglePlay,
    /// Seek to `fraction` of the duration, clamped to `0.0..=1.0`.
    Seek(f32),
    /// Set the output volume, clamped by the player.
    Volume(f32),
    /// Load the next queue entry.
    NextTrack,
    /// Load the previous queue entry.
    PreviousTrack,
}

/// The pixel layout of a decoded video frame.
///
/// Only tightly packed 4-byte layouts are accepted by [`VideoPlayer::set_video_frame`]; the
/// control is a presenter, not a converter, so a layout it cannot blit is refused rather than
/// drawn as a garbled image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoPixelFormat {
    /// Tightly packed RGBA, 4 bytes per pixel.
    #[default]
    Rgba8888,
    /// Tightly packed BGRA, 4 bytes per pixel.
    Bgra8888,
}

/// A decoded frame supplied by the host.
///
/// The data is row-major RGBA (or BGRA, see [`Self::format`]) with `stride` bytes per row. A
/// stride larger than `width * 4` is allowed, so a decoder that pads rows to 64 or 256 bytes can
/// hand over its buffer without a repack. The control does not demux or decode anything itself;
/// it presents what the host hands it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Presentation timestamp in milliseconds.
    pub pts_ms: u64,
    /// Pixel layout of `data`.
    pub format: VideoPixelFormat,
    /// Bytes per row, including any padding. Must be at least `width * 4`.
    pub stride: u32,
    /// Pixel bytes, `stride * height` long.
    ///
    /// The buffer is shared so a decoder can hand over a buffer it already owns without the
    /// player copying it on injection, and so the double-buffer swap moves handles rather than
    /// pixels.
    pub data: Arc<[u8]>,
}

impl VideoFrame {
    /// A tightly packed RGBA frame with `stride = width * 4`.
    ///
    /// This is the common construction: a decoder that already produces tightly packed RGBA
    /// passes its buffer here, and a decoder with padded rows builds the struct with an explicit
    /// `stride` and `format`.
    pub fn new(width: u32, height: u32, pts_ms: u64, data: Vec<u8>) -> Self {
        Self::from_shared(
            width,
            height,
            pts_ms,
            VideoPixelFormat::Rgba8888,
            width.saturating_mul(4),
            Arc::from(data),
        )
    }

    /// A frame over an already-shared pixel buffer, with no copy of the pixels.
    ///
    /// This is the decoder-thread entry point: a host that owns an `Arc<[u8]>` buffer hands the
    /// handle over and the frame keeps it. The caller still declares the layout, since a padded
    /// row stride and a BGRA order cannot be inferred from a byte count.
    pub fn from_shared(
        width: u32,
        height: u32,
        pts_ms: u64,
        format: VideoPixelFormat,
        stride: u32,
        data: Arc<[u8]>,
    ) -> Self {
        Self { width, height, pts_ms, format, stride, data }
    }

    /// The byte length this frame requires for `height` rows at its declared stride.
    pub fn expected_len(&self) -> usize {
        self.stride as usize * self.height as usize
    }
}

/// A tightly packed copy of a padded frame, reused between draws.
struct PackedVideoFrame {
    ptr: usize,
    len: usize,
    width: u32,
    height: u32,
    stride: u32,
    data: Vec<u8>,
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
    /// The playback queue. The player owns the model; a `PlaylistPanel` attached with
    /// [`Self::attach_playlist_panel`] is the chrome that lists it (MW-015).
    playlist: Vec<String>,
    /// The queue entry currently loaded, when the source came from the queue.
    playlist_index: Option<usize>,
    /// An optional queue panel kept in step with the model.
    playlist_panel: Option<PlaylistPanel>,
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
    /// Chapter markers, sorted by start time when set (MW-002).
    chapters: Vec<VideoChapter>,
    /// The chapter the playback clock is currently inside, when any.
    current_chapter: Option<usize>,
    /// Seek requests queued by an attached chapter list (COMP-A6).
    ///
    /// A widget cannot hold a borrow of another, so the list writes a chapter start time into
    /// this shared queue and wakes the player; [`Self::apply_pending_chapter_seeks`] drains it
    /// on the next paint or interaction, mirroring the `ColorWell`/`ColorHistory` pairing.
    pending_chapter_seeks: Option<Arc<Mutex<Vec<f64>>>>,
    /// Intents queued by an attached transport bar (MW-001). Same contract as the chapter seeks
    /// above: the bar writes, this player drains in its paint and event paths.
    pending_transport_requests: Option<Arc<Mutex<Vec<TransportRequest>>>>,
    /// A scrubber kept in step with the chapter model and the playback clock.
    scrubber: Option<crate::widget::media_widgets::media_scrubber::MediaScrubber>,
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
    /// Emitted with the presented timestamp **in seconds** when a frame is swapped on screen.
    ///
    /// Seconds rather than milliseconds so it shares a unit with [`Self::time_updated`] and the
    /// rest of the playback clock; a subscriber that needs the frame's own `pts_ms` reads it from
    /// [`Self::video_frame`].
    pub frame_presented: Signal1<f64>,
    /// Emitted with the chapter index when playback crosses into a new chapter (MW-002).
    pub on_chapter_entered: Signal1<usize>,
    /// Emitted when the host replaces the container metadata.
    pub metadata_changed: GenericSignal,
    /// Whether playback loops at the end instead of stopping.
    looping: bool,
    /// The most recent frame the host supplied, waiting for [`Self::present_frame`].
    ///
    /// The control keeps **two** slots: a host writes the back slot and `present_frame` swaps it
    /// into the front. A host that decodes on a worker thread therefore never mutates the pixels
    /// the painter is reading, and a burst of frames between presents costs one handle swap rather
    /// than a copy.
    back: Option<Arc<VideoFrame>>,
    /// The frame currently on screen.
    front: Option<Arc<VideoFrame>>,
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
    /// A tightly packed copy of the presented frame, used when the host's frame has a row
    /// stride larger than `width * 4` (MW-08). Keyed by the shared buffer's identity.
    packed_frame: Option<PackedVideoFrame>,
    /// How long the controls stay visible after the last pointer activity; `0` keeps them up.
    auto_hide_ms: u64,
    /// Milliseconds since the last pointer activity over the player.
    idle_ms: u64,
    /// Whether the seek bar is being dragged.
    scrubbing: bool,
    /// The time a drag is currently previewing, if any.
    scrub_time: Option<f64>,
    /// The seek bar's rectangle from the last paint, so a drag maps to the same span the user sees.
    seek_bar_rect: Option<Rect>,
    /// The transport buttons' rectangles from the last paint.
    play_rect: Option<Rect>,
    volume_rect: Option<Rect>,
    fullscreen_rect: Option<Rect>,
    /// The formatted timecode label, reused between paints (MW-001).
    time_label: String,
    /// The whole-second position and duration the cached label was built from.
    time_label_key: (u32, u32),
    /// The formatted playback-rate label, reused between paints (MW-001).
    rate_label: String,
    /// The tenths of the rate the cached label was built from.
    rate_label_key: Option<u32>,
    /// Cached subtitle line ranges and the inputs they were wrapped for (MW-002).
    subtitle_wrap: core::cell::RefCell<Option<SubtitleWrap>>,
    /// Emitted with the selected audio track index (MW-004).
    pub audio_track_selected: Signal1<usize>,
    /// Emitted with the selected subtitle track index (MW-004).
    pub subtitle_track_selected: Signal1<usize>,
    /// An attached caption companion, which follows this player's clock (MW-016).
    subtitle_companion: Option<Box<SubtitleBar>>,
}

/// One cached subtitle wrap, keyed by the caption and the band it was fitted to.
struct SubtitleWrap {
    text: String,
    width: u32,
    line_height: u32,
    ranges: Vec<(usize, usize)>,
}

impl VideoPlayer {
    /// Creates a new VideoPlayer with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::VideoPlayer, geometry, "VideoPlayer"),
            source: String::new(),
            playlist: Vec::new(),
            playlist_index: None,
            playlist_panel: None,
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
            chapters: Vec::new(),
            current_chapter: None,
            pending_chapter_seeks: None,
            pending_transport_requests: None,
            scrubber: None,
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
            on_chapter_entered: Signal1::new(),
            metadata_changed: GenericSignal::new(),
            looping: false,
            back: None,
            front: None,
            pipeline: VideoPipelineState::Idle,
            audio_tracks: Vec::new(),
            active_audio_track: None,
            subtitle_tracks: Vec::new(),
            active_subtitle_track: None,
            metadata: VideoMetadata::default(),
            subtitle_cursor: Cell::new(0),
            packed_frame: None,
            auto_hide_ms: 3000,
            idle_ms: 0,
            scrubbing: false,
            scrub_time: None,
            seek_bar_rect: None,
            play_rect: None,
            volume_rect: None,
            fullscreen_rect: None,
            time_label: String::new(),
            time_label_key: (u32::MAX, u32::MAX),
            rate_label: String::new(),
            rate_label_key: None,
            subtitle_wrap: core::cell::RefCell::new(None),
            audio_track_selected: Signal1::new(),
            subtitle_track_selected: Signal1::new(),
            subtitle_companion: None,
        }
    }

    /// Records a video source URI or file path for display.
    ///
    /// The source is stored as a label only: it is not opened, demuxed or
    /// decoded. Loading also resets the simulated clock and pauses playback.
    pub fn load(&mut self, source: &str) {
        self.source = source.to_string();
        // A direct load of a source that is in the queue keeps the "where am I" answer, the same
        // rule `set_playlist` applies in the other direction (MW-015).
        self.playlist_index = self.playlist.iter().position(|item| item == source);
        self.sync_playlist_panel();
        self.current_time = 0.0;
        self.is_playing = false;
        self.back = None;
        self.front = None;
        self.subtitle_cursor.set(0);
        self.pipeline =
            if source.is_empty() { VideoPipelineState::Idle } else { VideoPipelineState::Demuxing };
        self.set_status(if source.is_empty() { MediaStatus::Idle } else { MediaStatus::Paused });
        self.base.request_redraw();
    }

    /// Replaces the playback queue.
    ///
    /// The model is the player's; the attached [`PlaylistPanel`] is chrome that lists it. A
    /// source that names a queue entry keeps its position, so a host that sets the list after
    /// loading does not lose the "where am I" answer (the same rule `MediaPlayer` applies).
    pub fn set_playlist(&mut self, playlist: Vec<String>) {
        self.playlist = playlist;
        self.playlist_index = self.playlist.iter().position(|item| item == &self.source);
        self.sync_playlist_panel();
        self.base.request_redraw();
    }

    /// Returns the playback queue.
    pub fn playlist(&self) -> &[String] {
        &self.playlist
    }

    /// The queue entry currently loaded, if the source came from the queue.
    pub fn playlist_index(&self) -> Option<usize> {
        self.playlist_index.filter(|index| *index < self.playlist.len())
    }

    /// Loads queue entry `index`.
    pub fn select_track(&mut self, index: usize) -> bool {
        let Some(source) = self.playlist.get(index).cloned() else {
            return false;
        };
        self.load(&source);
        // `load` matched the source back to its first occurrence; a duplicate entry has to take
        // the position that was asked for, so the index is written again after the load.
        self.playlist_index = Some(index);
        self.sync_playlist_panel();
        true
    }

    /// Loads the next queue entry.
    pub fn next_track(&mut self) -> bool {
        if self.playlist.is_empty() {
            return false;
        }
        let next = match self.playlist_index() {
            Some(index) => index + 1,
            None => 0,
        };
        self.select_track(next)
    }

    /// Loads the previous queue entry.
    pub fn previous_track(&mut self) -> bool {
        let Some(index) = self.playlist_index() else {
            return false;
        };
        if index == 0 {
            return false;
        }
        self.select_track(index - 1)
    }

    /// Attaches a queue panel and fills it with the current model.
    ///
    /// Returns whether a panel is attached afterwards; `false` detaches the current one.
    pub fn attach_playlist_panel(&mut self, panel: Option<PlaylistPanel>) -> bool {
        self.playlist_panel = panel;
        self.sync_playlist_panel();
        self.playlist_panel.is_some()
    }

    /// The attached queue panel, if any.
    pub fn playlist_panel(&self) -> Option<&PlaylistPanel> {
        self.playlist_panel.as_ref()
    }

    /// The attached queue panel for editing, if any.
    pub fn playlist_panel_mut(&mut self) -> Option<&mut PlaylistPanel> {
        self.playlist_panel.as_mut()
    }

    /// Pushes the model into the attached panel.
    ///
    /// Called from every model write, so the panel cannot show a queue the player is not
    /// playing from.
    fn sync_playlist_panel(&mut self) {
        // Snapshot both model facts before borrowing the panel mutably: the panel lives in
        // `self`, so reading `self.playlist` inside the borrow is a second borrow of the same
        // value.
        let tracks = self.playlist.clone();
        let current = self.playlist_index();
        if let Some(panel) = self.playlist_panel.as_mut() {
            panel.set_tracks(tracks);
            panel.set_current_index(current);
        }
    }

    /// Accepts a decoded frame from the host's pipeline.
    ///
    /// The control has no demuxer or decoder, so this is the only way pixels reach it. The
    /// frame is validated (non-empty dimensions, exactly `width * height * 4` RGBA bytes) before
    /// it is stored, so a malformed buffer cannot be painted. The pipeline moves to
    /// [`VideoPipelineState::Presenting`] and `frame_available` announces the timestamp.
    pub fn set_video_frame(&mut self, frame: VideoFrame) -> Result<(), String> {
        self.set_video_frame_shared(Arc::new(frame))
    }

    /// Accepts a frame already owned through an `Arc`, with no `VideoFrame` copy.
    ///
    /// A worker thread that owns its decoded frames can share the handle with the UI thread and
    /// hand it here; the control validates and stores that same allocation. This is the zero-copy
    /// sibling of [`Self::set_video_frame`].
    pub fn set_video_frame_shared(&mut self, frame: Arc<VideoFrame>) -> Result<(), String> {
        if frame.width == 0 || frame.height == 0 {
            return Err("a video frame needs non-zero dimensions".to_string());
        }
        if frame.format != VideoPixelFormat::Rgba8888 {
            return Err(format!(
                "a {:?} video frame cannot be painted; this control accepts RGBA8888",
                frame.format
            ));
        }
        let minimum = (frame.width as usize) * (frame.height as usize) * 4;
        let expected = frame.expected_len();
        if (frame.stride as usize) < frame.width as usize * 4 {
            return Err(format!(
                "a {}x{} frame needs a stride of at least {} bytes, got {}",
                frame.width,
                frame.height,
                frame.width as usize * 4,
                frame.stride
            ));
        }
        if frame.data.len() != expected {
            return Err(format!(
                "a {}x{} RGBA frame at stride {} needs {expected} bytes, got {}",
                frame.width,
                frame.height,
                frame.stride,
                frame.data.len()
            ));
        }
        debug_assert!(expected >= minimum);
        let pts = frame.pts_ms;
        // The back slot, not the front: the painter must never see a half-published frame. The
        // next `present_frame` swaps it in.
        self.back = Some(frame);
        self.pipeline = VideoPipelineState::Presenting;
        self.frame_available.emit(pts);
        self.base.request_redraw();
        Ok(())
    }

    /// Swaps the waiting frame into the front slot and announces it as `frame_presented`.
    ///
    /// This is the explicit double-buffer swap: `set_video_frame` writes the back slot, this moves
    /// it to the front, and the previous front becomes the new back (dropping it). Returns `false`
    /// when the host has not supplied a new frame, which is a statement about the pipeline rather
    /// than an error: a player may legitimately be waiting for its first frame, and re-presenting
    /// the same frame would emit a timestamp that did not happen.
    pub fn present_frame(&mut self) -> bool {
        let Some(frame) = self.back.take() else {
            return false;
        };
        let pts = frame.pts_ms;
        self.front = Some(frame);
        self.pipeline = VideoPipelineState::Presenting;
        self.frame_presented.emit(pts as f64 / 1000.0);
        self.base.request_redraw();
        true
    }

    /// The frame currently on screen, if any.
    ///
    /// Returns the **presented** frame, not a frame waiting for [`Self::present_frame`]; use
    /// [`Self::pending_frame`] to inspect the back slot.
    pub fn video_frame(&self) -> Option<&VideoFrame> {
        self.front.as_deref()
    }

    /// The frame waiting for the next [`Self::present_frame`], if any.
    pub fn pending_frame(&self) -> Option<&VideoFrame> {
        self.back.as_deref()
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
        // The transport bar may show a loop indicator derived from this flag; without the
        // repaint that indicator stayed stale (MW-09).
        self.base.request_redraw();
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
        // A track switch is an observable state change, so it is announced rather than only
        // painted (MW-004). The name follows the existing `*_selected` selection signals.
        self.audio_track_selected.emit(index);
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
        let changed = self.active_subtitle_track != Some(index);
        self.active_subtitle_track = Some(index);
        let cues = self.subtitle_tracks[index].cues.clone();
        self.set_subtitles(cues);
        if changed {
            // Selection is announced only on a real switch (MW-004).
            self.subtitle_track_selected.emit(index);
        }
        Ok(())
    }

    /// The selected subtitle track index, when one is selected.
    pub fn subtitle_track(&self) -> Option<usize> {
        self.active_subtitle_track
    }

    /// Attaches a caption companion to this player.
    ///
    /// The companion follows this player's clock (it subscribes in [`SubtitleBar::follow_video_player`])
    /// and paints the caption band in place of the player's own cue layer, so a host gets the
    /// multi-line band without positioning it. The default (no companion) leaves the existing
    /// paint path untouched (MW-016).
    pub fn attach_subtitle_bar(&mut self, mut bar: SubtitleBar) {
        bar.follow_video_player(self);
        self.subtitle_companion = Some(Box::new(bar));
        self.base.request_redraw();
    }

    /// Replaces the chapter markers (MW-002).
    ///
    /// Non-finite or negative starts are dropped, and the rest are sorted, so
    /// [`Self::chapter_at`] can search the timeline with one reverse scan. An attached scrubber
    /// receives the same markers as fractions of the duration.
    pub fn set_chapters(&mut self, chapters: Vec<VideoChapter>) {
        let mut chapters: Vec<VideoChapter> = chapters
            .into_iter()
            .filter(|chapter| chapter.start_seconds.is_finite() && chapter.start_seconds >= 0.0)
            .collect();
        chapters.sort_by(|left, right| left.start_seconds.total_cmp(&right.start_seconds));
        self.chapters = chapters;
        self.sync_scrubber_chapters();
        self.current_chapter = None;
        self.sync_chapter();
        self.base.request_redraw();
    }

    /// The chapter markers currently installed.
    pub fn chapters(&self) -> &[VideoChapter] {
        &self.chapters
    }

    /// Accepts the seek stream of a paired chapter list (COMP-A6).
    ///
    /// The list keeps the write end; this player drains it through
    /// [`Self::apply_pending_chapter_seeks`], which the paint and event paths already call.
    /// The returned signal is this player's own redraw request, so a queued seek repaints even
    /// when it did not originate from a player gesture.
    pub(crate) fn accept_chapter_seeks(&mut self, seeks: Arc<Mutex<Vec<f64>>>) -> GenericSignal {
        self.pending_chapter_seeks = Some(seeks);
        self.base.request_redraw();
        self.base.redraw_requested_signal().clone()
    }

    /// Accepts a transport bar's request queue and returns the redraw signal that wakes this
    /// player when a request is queued.
    pub(crate) fn accept_transport_requests(
        &mut self,
        requests: Arc<Mutex<Vec<TransportRequest>>>,
    ) -> GenericSignal {
        self.pending_transport_requests = Some(requests);
        self.base.request_redraw();
        self.base.redraw_requested_signal().clone()
    }

    /// Applies every transport request a paired bar has queued since the last call.
    ///
    /// Returns the number of requests consumed. Each one runs through the public method a host
    /// would call ([`Self::play`], [`Self::seek`], [`Self::set_volume`], [`Self::next_track`],
    /// [`Self::previous_track`]), so the bar cannot bypass this player's clamping and queue rules.
    pub fn apply_pending_transport_requests(&mut self) -> usize {
        let Some(queue) = self.pending_transport_requests.as_ref() else {
            return 0;
        };
        let requests: Vec<TransportRequest> = {
            let mut guard = crate::compat::lock(queue);
            core::mem::take(&mut *guard)
        };
        let count = requests.len();
        for request in requests {
            match request {
                TransportRequest::Play => self.play(),
                TransportRequest::Pause => self.pause(),
                TransportRequest::TogglePlay => {
                    if self.is_playing() {
                        self.pause();
                    } else {
                        self.play();
                    }
                }
                TransportRequest::Seek(fraction) => {
                    let target = f64::from(fraction.clamp(0.0, 1.0)) * self.duration;
                    self.seek(target);
                }
                TransportRequest::Volume(volume) => self.set_volume(volume),
                TransportRequest::NextTrack => {
                    self.next_track();
                }
                TransportRequest::PreviousTrack => {
                    self.previous_track();
                }
            }
        }
        count
    }

    /// Applies every seek a paired chapter list has queued since the last call.
    ///
    /// Returns the number of requests consumed. A request is a chapter start time in seconds,
    /// so [`Self::seek`] still owns the rules (clamping, non-finite rejection and chapter-entry
    /// reporting).
    pub fn apply_pending_chapter_seeks(&mut self) -> usize {
        let Some(queue) = self.pending_chapter_seeks.as_ref() else {
            return 0;
        };
        let seeks: Vec<f64> = {
            let mut guard = crate::compat::lock(queue);
            core::mem::take(&mut *guard)
        };
        let count = seeks.len();
        for time in seeks {
            self.seek(time);
        }
        count
    }

    /// The chapter containing `time`, or `None` before the first marker.
    pub fn chapter_at(&self, time: f64) -> Option<usize> {
        if !time.is_finite() {
            return None;
        }
        self.chapters.iter().rposition(|chapter| chapter.start_seconds <= time)
    }

    /// The chapter the playback clock is currently inside.
    pub fn current_chapter(&self) -> Option<usize> {
        self.current_chapter
    }

    /// Attaches a scrubber, pushing the chapter markers into it (MW-002).
    pub fn attach_scrubber(
        &mut self,
        scrubber: Option<crate::widget::media_widgets::media_scrubber::MediaScrubber>,
    ) -> bool {
        self.scrubber = scrubber;
        self.sync_scrubber_chapters();
        self.scrubber.is_some()
    }

    /// The attached scrubber, if any.
    pub fn scrubber(&self) -> Option<&crate::widget::media_widgets::media_scrubber::MediaScrubber> {
        self.scrubber.as_ref()
    }

    /// Emits `on_chapter_entered` when the clock crosses a marker.
    fn sync_chapter(&mut self) {
        let next = self.chapter_at(self.current_time);
        if next == self.current_chapter {
            return;
        }
        self.current_chapter = next;
        if let Some(index) = next {
            self.on_chapter_entered.emit(index);
        }
    }

    /// Pushes the chapter model to the attached scrubber as duration fractions.
    fn sync_scrubber_chapters(&mut self) {
        let Some(scrubber) = self.scrubber.as_mut() else {
            return;
        };
        let duration = self.duration.max(f64::EPSILON);
        scrubber.set_chapters(
            self.chapters
                .iter()
                .map(|chapter| (chapter.start_seconds / duration).clamp(0.0, 1.0))
                .collect(),
        );
    }

    /// The attached caption companion, if any.
    pub fn subtitle_bar(&self) -> Option<&SubtitleBar> {
        self.subtitle_companion.as_deref()
    }

    /// The attached caption companion, mutably, if any.
    pub fn subtitle_bar_mut(&mut self) -> Option<&mut SubtitleBar> {
        self.subtitle_companion.as_deref_mut()
    }

    /// Detaches the caption companion and returns to the built-in cue layer.
    pub fn detach_subtitle_bar(&mut self) -> Option<SubtitleBar> {
        let bar = self.subtitle_companion.take().map(|bar| *bar);
        if bar.is_some() {
            self.base.request_redraw();
        }
        bar
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

    /// Reports that playback is stalled waiting for data, or that it has resumed.
    ///
    /// The control performs no I/O, so only the host knows when a buffer underrun starts and
    /// ends. `true` moves the reported status to [`MediaStatus::Buffering`] (without changing
    /// `is_playing`, which records the user's intent); `false` restores `Playing` or `Paused`
    /// according to that intent. The simulated clock does not advance while buffering, so a
    /// stalled stream does not appear to play through the gap (MW-003).
    pub fn set_buffering(&mut self, buffering: bool) {
        if self.source.is_empty() {
            return;
        }
        if buffering {
            if self.status != MediaStatus::Buffering {
                self.set_status(MediaStatus::Buffering);
                self.base.request_redraw();
            }
        } else if self.status == MediaStatus::Buffering {
            self.set_status(if self.is_playing {
                MediaStatus::Playing
            } else {
                MediaStatus::Paused
            });
            self.base.request_redraw();
        }
    }

    /// Whether the host has reported that playback is waiting for data.
    pub fn is_buffering(&self) -> bool {
        self.status == MediaStatus::Buffering
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
            // A host-reported buffer stall outranks the user's play request: playback is intended,
            // but the clock must stay still until the host says data is available (MW-003).
            if self.status != MediaStatus::Buffering {
                self.set_status(MediaStatus::Playing);
            }
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
        self.sync_chapter();
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

    /// How long the controls stay visible after the last pointer activity, in milliseconds.
    pub fn auto_hide_ms(&self) -> u64 {
        self.auto_hide_ms
    }

    /// Sets the control-bar auto-hide delay; `0` keeps the controls visible permanently.
    pub fn set_auto_hide_ms(&mut self, ms: u64) {
        if self.auto_hide_ms == ms {
            return;
        }
        self.auto_hide_ms = ms;
        self.idle_ms = 0;
        if ms == 0 {
            self.controls_visible = true;
        }
        self.base.request_redraw();
    }

    /// Resets the auto-hide countdown and shows the controls.
    ///
    /// A host that owns its own pointer pipeline can call this from its hover handler; the
    /// control's own `MouseMove` handler calls it when the pointer enters the control bar.
    pub fn notify_pointer_activity(&mut self) {
        self.idle_ms = 0;
        if !self.controls_visible {
            self.controls_visible = true;
            self.base.request_redraw();
        }
    }

    /// Maps a seek-bar x coordinate to a time on the timeline.
    fn time_from_seek_x(&self, x: i32) -> f64 {
        let Some(bar) = self.seek_bar_rect else {
            return self.current_time;
        };
        if bar.width == 0 || self.duration <= 0.0 {
            return 0.0;
        }
        let fraction = ((x - bar.x) as f64 / bar.width as f64).clamp(0.0, 1.0);
        fraction * self.duration
    }

    /// Repacks a padded frame into a tightly packed buffer, reusing the previous result.
    ///
    /// Returns whether [`Self::packed_frame`] holds the bytes to paint. A tightly packed frame
    /// needs no buffer at all, which is the common case (MW-08 / L-P06).
    fn ensure_packed_frame(&mut self, frame: &VideoFrame) -> bool {
        let tight = frame.width as usize * 4;
        if frame.stride as usize == tight {
            return false;
        }
        let ptr = frame.data.as_ptr() as usize;
        let len = frame.data.len();
        let reusable = matches!(
            self.packed_frame.as_ref(),
            Some(cached)
                if cached.ptr == ptr
                    && cached.len == len
                    && cached.width == frame.width
                    && cached.height == frame.height
                    && cached.stride == frame.stride
        );
        if reusable {
            return true;
        }
        let mut data = crate::compat::vec![0u8; tight * frame.height as usize];
        for row in 0..frame.height as usize {
            let start = row * frame.stride as usize;
            let Some(pixels) = frame.data.get(start..start + tight) else {
                break;
            };
            let destination = row * tight;
            data[destination..destination + tight].copy_from_slice(pixels);
        }
        self.packed_frame = Some(PackedVideoFrame {
            ptr,
            len,
            width: frame.width,
            height: frame.height,
            stride: frame.stride,
            data,
        });
        true
    }

    /// Rebuilds the timecode label only when the displayed whole seconds change (MW-001).
    ///
    /// `format!` built a fresh `String` on every paint. The buffer is cleared and rewritten
    /// instead, so after the first frame its capacity is reused and a steady clock allocates
    /// nothing; the key is exactly the pair of values the label is derived from.
    fn update_time_label(&mut self, shown_time: f64) {
        let key = (shown_time.max(0.0) as u32, self.duration.max(0.0) as u32);
        if self.time_label_key == key {
            return;
        }
        self.time_label.clear();
        let _ = write!(
            self.time_label,
            "{:02}:{:02}/{:02}:{:02}",
            key.0 / 60,
            key.0 % 60,
            key.1 / 60,
            key.1 % 60,
        );
        self.time_label_key = key;
    }

    /// Whether the playback-rate label is shown at all.
    fn has_rate_label(&self) -> bool {
        (self.playback_rate - 1.0).abs() > 0.01
    }

    /// Rebuilds the playback-rate label only when the shown tenths change (MW-001).
    fn update_rate_label(&mut self) {
        if !self.has_rate_label() {
            return;
        }
        let key = (self.playback_rate * 10.0).round() as u32;
        if self.rate_label_key == Some(key) {
            return;
        }
        self.rate_label.clear();
        let _ = write!(self.rate_label, "{:.1}x", self.playback_rate);
        self.rate_label_key = Some(key);
    }

    /// The wrapped subtitle ranges for `text`, reusing the previous result while it still fits.
    ///
    /// Wrapping calls the text measurer once per candidate break, which is the expensive part of
    /// painting a cue; doing it on every frame made a long caption re-wrap at frame rate (MW-002).
    /// The cache is keyed by the caption itself and the band it is fitted to, so a new cue, a
    /// resized player or a changed font height re-wraps exactly once.
    fn subtitle_lines<'a>(
        &'a self,
        text: &str,
        width: u32,
        line_height: u32,
        measure: &mut dyn FnMut(&str) -> u32,
    ) -> core::cell::Ref<'a, Vec<(usize, usize)>> {
        let stale = match &*self.subtitle_wrap.borrow() {
            Some(cached) => {
                cached.text != text || cached.width != width || cached.line_height != line_height
            }
            None => true,
        };
        if stale {
            let ranges = crate::widget::text_utils::wrap_to_width(text, width, measure)
                .into_iter()
                .map(|range| (range.start, range.end))
                .collect();
            *self.subtitle_wrap.borrow_mut() =
                Some(SubtitleWrap { text: text.to_string(), width, line_height, ranges });
        }
        core::cell::Ref::map(self.subtitle_wrap.borrow(), |slot| {
            &slot.as_ref().expect("the cache was filled above").ranges
        })
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
        if !self.is_playing
            || self.status == MediaStatus::Buffering
            || self.duration <= 0.0
            || !delta_secs.is_finite()
            || delta_secs < 0.0
        {
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
                self.sync_chapter();
                self.time_updated.emit(self.current_time);
                self.base.request_redraw();
                return false;
            }
            self.current_time = self.duration;
            self.is_playing = false;
            self.pipeline = VideoPipelineState::Ended;
            self.set_status(MediaStatus::Ended);
            self.playback_ended.emit();
            self.sync_chapter();
            self.time_updated.emit(self.current_time);
            self.base.request_redraw();
            return true;
        }

        self.sync_chapter();
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
        let finished = VideoPlayer::tick(self, f64::from(delta_ms) / 1000.0);
        // The control bar hides itself after a period without pointer activity (MW-07). A
        // paused or stopped player keeps its controls up: hiding them would leave the only way
        // to resume invisible.
        if self.auto_hide_ms > 0 && self.controls_visible && self.is_playing && !self.scrubbing {
            self.idle_ms = self.idle_ms.saturating_add(u64::from(delta_ms));
            if self.idle_ms >= self.auto_hide_ms {
                self.controls_visible = false;
                self.base.request_redraw();
            }
        }
        finished
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
            "on_chapter_entered" => Some(EventSignalRef::mapped(
                "on_chapter_entered",
                &self.on_chapter_entered,
                |index| CapabilityValue::UInt(*index as u64),
            )),
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
                    crate::widget::capability::CapabilityValue::Float(*v)
                }))
            }
            "metadata_changed" => {
                Some(EventSignalRef::unit("metadata_changed", &self.metadata_changed))
            }
            "audio_track_selected" => Some(EventSignalRef::mapped(
                "audio_track_selected",
                &self.audio_track_selected,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
            "subtitle_track_selected" => Some(EventSignalRef::mapped(
                "subtitle_track_selected",
                &self.subtitle_track_selected,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
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
            // MW-003: the host's data-starvation report, so the state machine's `Buffering` arm
            // has a production entry instead of being reachable only from tests.
            "buffering" => Ok(CapabilityValue::Bool(self.is_buffering())),
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
            "buffering" => {
                self.set_buffering(expect_bool(value)?);
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
            "buffering",
            "pipeline",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for VideoPlayer {
    fn draw(&mut self, context: &mut RenderContext) {
        // A chapter-list press queues its seek and wakes this player; the paint is the first
        // point after that wake where the clock can be moved safely (COMP-A6).
        self.apply_pending_chapter_seeks();
        self.apply_pending_transport_requests();
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
            match crate::style::palette_shared() {
                Some(colors) => {
                    (colors.primary, colors.surface_container, colors.secondary, colors.background)
                }
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
        context.face_with_gradient_corners(
            rect,
            video_bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        // The outline follows the face's radius: a square 1 px stroke over the rounded video
        // surface left the four corners outside the drawn face.
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            self.style().border_color.unwrap_or_else(|| video_bg.blend(&Color::WHITE, 0.2)),
            1,
        );

        // The control's chrome text (placeholder, time, rate) takes the **effective font**, so the
        // theme body font and the user's text scale reach it (D09-STYLE-01). Cloned once because
        // the frame-repack cache below mutates the control during this frame.
        let font = effective_font(self.style()).clone();
        let font = &font;

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
        if let Some(frame) = self.front.clone() {
            let dx = rect.x + (rect.width as i32 - frame.width as i32) / 2;
            let dy = rect.y + (rect.height as i32 - frame.height as i32) / 2;
            context.push_clip(rect.x, rect.y, rect.width, rect.height);
            // A host may hand over a frame with padded rows; the image primitive is tightly
            // packed, so such a frame is repacked once per buffer rather than drawn skewed
            // (MW-08).
            let needs_packing = self.ensure_packed_frame(&frame);
            let pixels: &[u8] = match self.packed_frame.as_ref() {
                Some(packed) if needs_packing => &packed.data,
                _ => frame.data.as_ref(),
            };
            context.draw_image(dx, dy, frame.width, frame.height, pixels);
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
        if let Some(companion) = self.subtitle_companion.as_mut() {
            // The companion owns its own band; it is given the video area (above the transport
            // bar) so its bottom/top placement lands where the built-in band would (MW-016).
            let area_height = (control_bar_top - rect.y).max(0) as u32;
            companion.set_geometry(Rect::new(rect.x, rect.y, rect.width, area_height));
            if let Some(drawable) = crate::widget::draw_bridge::draw_of(companion.as_mut()) {
                drawable.draw(context);
            }
        } else if self.subtitles_enabled {
            if let Some(cue) = self.subtitle_at(self.current_time) {
                // A cue is wrapped to the player's width and the band grows with the line
                // count, capped at three lines; a long sentence used to be truncated to one
                // strip (MW-09). The wrap itself is cached by caption and band width (MW-002).
                let band_width = rect.width.saturating_sub(16).max(1);
                let line_height = context.measure_text("M", font).height.max(1);
                let ranges = self.subtitle_lines(
                    &cue.text,
                    band_width.saturating_sub(12),
                    line_height,
                    &mut |text: &str| context.measure_text(text, font).width,
                );
                let lines = ranges.len().clamp(1, 3);
                let band_height =
                    (lines as u32 * line_height + 6).min((rect.height / 2).max(line_height + 6));
                let band = Rect::new(
                    rect.x + 8,
                    control_bar_top - band_height as i32 - 8,
                    band_width,
                    band_height,
                );
                if band.y > rect.y {
                    context.fill_rounded_rect(band, 4, Color::rgba(0, 0, 0, 150));
                    for (line, (start, end)) in ranges.iter().take(lines).enumerate() {
                        let Some(text) = cue.text.get(*start..*end) else {
                            continue;
                        };
                        let row = Rect::new(
                            band.x + 6,
                            band.y + 3 + line as i32 * line_height as i32,
                            band.width.saturating_sub(12),
                            line_height,
                        );
                        context.draw_text_fitted_cow(
                            row,
                            text,
                            font,
                            Color::WHITE,
                            HorizontalAlignment::Center,
                        );
                    }
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
        // and another for the rate every frame. A drag previews the position it would seek to.
        // The buffers are kept on the control and rebuilt only when their value changes (MW-001).
        let shown_time = self.scrub_time.unwrap_or(self.current_time);
        self.update_time_label(shown_time);
        self.update_rate_label();
        let has_rate = self.has_rate_label();
        let time_text = &self.time_label;
        let rate_text = &self.rate_label;
        let time_metrics = context.measure_text(time_text, font);
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
        let rate_metrics =
            if has_rate { Some(context.measure_text(rate_text, font)) } else { None };
        let rate_width = rate_metrics.as_ref().map(|m| m.width as i32).unwrap_or(0);
        // The rate label sits left of the fullscreen icon; the time label sits left of the
        // whole right-hand group. Both gaps are real, so neither label can overlap an icon.
        let rate_x = fs_box.x - rate_width - 8;
        let time_x = if has_rate { rate_x } else { fs_box.x } - time_width - 8;
        let seek_bar_width = (time_x - 8 - seek_bar_x).max(0) as u32;
        let seek_bar_full = Rect::new(seek_bar_x, seek_bar_y, seek_bar_width, seek_bar_height);
        // The transport rectangles are cached for the hit test, so a drag maps to exactly the
        // span that was painted and the volume icon is clickable where its ink is (MW-07).
        self.seek_bar_rect = Some(seek_bar_full);
        self.play_rect = Some(Rect::new(rect.x + 8, control_bar_y, btn_side, control_bar_height));
        self.volume_rect = Some(vol_box);
        self.fullscreen_rect = Some(fs_box);
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
            time_text,
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
                rate_text,
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
        self.apply_pending_chapter_seeks();
        self.apply_pending_transport_requests();
        match event {
            Event::MouseMove { pos } => {
                let bar_top = self.geometry().bottom()
                    - crate::widget::metrics::dimensions::VIDEO_CONTROL_BAR_HEIGHT as i32;
                if pos.y >= bar_top && self.geometry().contains_point(*pos) && self.controls_visible
                {
                    self.notify_pointer_activity();
                }
            }
            Event::Drag { pos, .. } => {
                if self.scrubbing {
                    let time = self.time_from_seek_x(pos.x);
                    self.scrub_time = Some(time);
                    self.seek(time);
                }
            }
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    let rect = self.geometry();
                    if !rect.contains_point(*pos) {
                        return;
                    }

                    let control_bar_height = 36u32;
                    let control_bar_y = rect.y + rect.height as i32 - control_bar_height as i32;

                    if pos.y >= control_bar_y
                        && pos.y < rect.y + rect.height as i32
                        && self.controls_visible
                    {
                        // The painted rectangles are the hit regions. The fallbacks match the
                        // draw path's constants, so a press that arrives before the first
                        // paint still addresses the button it looks like.
                        let play_rect = self.play_rect.unwrap_or(Rect::new(
                            rect.x + 8,
                            control_bar_y,
                            16,
                            control_bar_height,
                        ));
                        if play_rect.contains_point(*pos) {
                            self.toggle_play();
                            return;
                        }
                        let fs_box = self.fullscreen_rect.unwrap_or(Rect::new(
                            rect.x + rect.width as i32 - 16 - 8 - 16 - 24,
                            control_bar_y,
                            16,
                            control_bar_height,
                        ));
                        if fs_box.contains_point(*pos) {
                            self.toggle_fullscreen();
                            return;
                        }
                        // The volume icon cycles mute, which is the only action a speaker can
                        // carry (MW-07).
                        if self.volume_rect.is_some_and(|box_| box_.contains_point(*pos)) {
                            self.set_muted(!self.muted);
                            self.notify_pointer_activity();
                            return;
                        }
                        // The seek bar starts a drag. The press also seeks, so a tap jumps in
                        // one gesture while a drag keeps updating.
                        if self.seek_bar_rect.is_some_and(|bar| bar.contains_point(*pos)) {
                            let time = self.time_from_seek_x(pos.x);
                            self.scrubbing = true;
                            self.scrub_time = Some(time);
                            self.seek(time);
                            self.notify_pointer_activity();
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
            // A scrub ends on release and commits the position already previewed.
            Event::MouseRelease { pos, button } if self.scrubbing => {
                if *button == 1 {
                    let time = self.time_from_seek_x(pos.x);
                    self.seek(time);
                    self.scrubbing = false;
                    self.scrub_time = None;
                    self.notify_pointer_activity();
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
            let backdrop = crate::style::palette_shared()
                .map(|colors| colors.background)
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
            vp.set_video_frame(VideoFrame {
                width: 2,
                height: 2,
                pts_ms: 42,
                format: VideoPixelFormat::Rgba8888,
                stride: 8,
                data: Arc::from(vec![9u8; 8]),
            })
            .is_err(),
            "a wrong byte count must be refused"
        );
        assert!(vp.video_frame().is_none());

        vp.set_video_frame(VideoFrame {
            width: 2,
            height: 2,
            pts_ms: 42,
            format: VideoPixelFormat::Rgba8888,
            stride: 8,
            data: Arc::from(vec![9u8; 16]),
        })
        .expect("a matching frame is accepted");
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 42);
        assert_eq!(vp.pipeline(), VideoPipelineState::Presenting);
        assert!(vp.video_frame().is_none(), "a set frame waits for an explicit present");
        assert!(vp.pending_frame().is_some());

        let presented = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX));
        let p = presented.clone();
        vp.frame_presented.connect(move |pts: std::sync::Arc<f64>| {
            p.store((*pts * 1000.0) as u64, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(vp.present_frame());
        assert_eq!(presented.load(std::sync::atomic::Ordering::SeqCst), 42);
        assert!(vp.video_frame().is_some());
        assert!(!vp.present_frame(), "presenting twice without a new frame does nothing");
    }

    #[test]
    fn a_shared_video_frame_is_swapped_without_repacking() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("movie.mp4");
        let pixels: std::sync::Arc<[u8]> = std::sync::Arc::from(vec![5u8; 4 * 4 * 4]);
        let frame = std::sync::Arc::new(VideoFrame::from_shared(
            4,
            4,
            80,
            VideoPixelFormat::Rgba8888,
            16,
            std::sync::Arc::clone(&pixels),
        ));
        let handle = std::sync::Arc::clone(&frame);
        vp.set_video_frame_shared(handle).expect("a well-formed shared frame is adopted");
        assert_eq!(
            std::sync::Arc::strong_count(&frame),
            2,
            "the back slot keeps the caller's frame handle"
        );
        assert!(vp.present_frame());
        let stored = vp.video_frame().expect("the frame is on screen");
        assert!(std::sync::Arc::ptr_eq(&stored.data, &pixels));
        assert_eq!((stored.width, stored.height, stored.stride), (4, 4, 16));
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

    #[test]
    fn the_timecode_label_is_rebuilt_only_when_its_value_changes() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(125.0);
        vp.update_time_label(65.0);
        assert_eq!(vp.time_label, "01:05/02:05");
        let allocated = vp.time_label.as_ptr();
        // The same displayed second must reuse the buffer, not rewrite it.
        vp.update_time_label(65.9);
        assert_eq!(vp.time_label, "01:05/02:05");
        assert_eq!(vp.time_label.as_ptr(), allocated, "an unchanged label reuses its buffer");
        // A new second rewrites in place: the capacity acquired above still fits.
        vp.update_time_label(66.0);
        assert_eq!(vp.time_label, "01:06/02:05");
        assert_eq!(vp.time_label.as_ptr(), allocated, "a changed label reuses its capacity");
    }

    #[test]
    fn the_rate_label_is_rebuilt_only_when_the_shown_tenths_change() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_playback_rate(1.5);
        vp.update_rate_label();
        assert_eq!(vp.rate_label, "1.5x");
        let allocated = vp.rate_label.as_ptr();
        vp.update_rate_label();
        assert_eq!(vp.rate_label.as_ptr(), allocated);
        vp.set_playback_rate(2.0);
        vp.update_rate_label();
        assert_eq!(vp.rate_label, "2.0x");
    }

    #[test]
    fn the_subtitle_wrap_is_cached_until_the_caption_or_band_changes() {
        let vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        let measurements = std::cell::Cell::new(0usize);
        {
            let ranges = vp.subtitle_lines("a caption that wraps", 48, 14, &mut |_| {
                measurements.set(measurements.get() + 1);
                8
            });
            assert!(!ranges.is_empty());
        }
        let after_first = measurements.get();
        assert!(after_first > 0);
        {
            let _ = vp.subtitle_lines("a caption that wraps", 48, 14, &mut |_| {
                measurements.set(measurements.get() + 1);
                8
            });
        }
        assert_eq!(measurements.get(), after_first, "an unchanged caption must not re-wrap");
        {
            let _ = vp.subtitle_lines("a caption that wraps", 24, 14, &mut |_| {
                measurements.set(measurements.get() + 1);
                8
            });
        }
        assert!(measurements.get() > after_first, "a narrower band must re-wrap");
    }

    #[test]
    fn reported_buffering_stalls_the_clock_and_resumes_it() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("stream.m3u8");
        vp.set_duration(60.0);
        vp.play();
        vp.tick(1.0);
        assert!((vp.current_time() - 1.0).abs() < 1e-9);

        vp.set_buffering(true);
        assert!(vp.is_buffering());
        assert_eq!(vp.media_status(), MediaStatus::Buffering);
        assert!(!vp.tick(1.0), "a stalled clock does not finish playback");
        assert!((vp.current_time() - 1.0).abs() < 1e-9, "the clock must not advance");
        assert!(vp.is_playing(), "buffering does not change the user's play intent");

        vp.set_buffering(false);
        assert_eq!(vp.media_status(), MediaStatus::Playing);
        vp.tick(1.0);
        assert!((vp.current_time() - 2.0).abs() < 1e-9);
    }

    /// MW-015: the queue is the player's model and the panel is its chrome; both stay in step.
    #[test]
    fn the_playlist_model_drives_an_attached_panel() {
        use crate::widget::companions::PlaylistPanel;

        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_playlist(vec!["a.mp4".to_string(), "b.mp4".to_string(), "c.mp4".to_string()]);
        let panel = PlaylistPanel::new(Rect::new(0, 0, 160, 120));
        assert!(vp.attach_playlist_panel(Some(panel)), "the panel is attached");
        assert_eq!(vp.playlist_panel().expect("attached").tracks().len(), 3);
        assert_eq!(vp.playlist_panel().expect("attached").current_index(), None);

        assert!(vp.select_track(1));
        assert_eq!(vp.playlist_index(), Some(1));
        assert_eq!(vp.playlist_panel().expect("attached").current_index(), Some(1));
        assert_eq!(vp.source(), "b.mp4");

        assert!(vp.next_track());
        assert_eq!(vp.playlist_index(), Some(2));
        assert!(!vp.next_track(), "the queue has no wrap-around");
        assert!(vp.previous_track());
        assert_eq!(vp.playlist_index(), Some(1));
        assert_eq!(vp.playlist_panel().expect("attached").current_index(), Some(1));

        assert!(!vp.attach_playlist_panel(None));
        assert!(vp.playlist_panel().is_none());
    }

    /// MW-003: the declarative property route reaches the same `Buffering` arm as the method.
    #[test]
    fn the_buffering_property_reaches_the_state_machine() {
        use crate::widget::capability::types::CapabilityValue;
        use crate::widget::capability::WidgetProperties;

        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("stream.m3u8");
        vp.play();
        assert_eq!(vp.get("buffering"), Ok(CapabilityValue::Bool(false)));

        vp.set("buffering", CapabilityValue::Bool(true)).expect("the property is writable");
        assert_eq!(vp.media_status(), MediaStatus::Buffering);
        assert_eq!(vp.get("buffering"), Ok(CapabilityValue::Bool(true)));

        vp.set("buffering", CapabilityValue::Bool(false)).expect("the property is writable");
        assert_eq!(vp.media_status(), MediaStatus::Playing);
    }

    #[test]
    fn switching_tracks_announces_the_selected_index() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        let audio = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let a = audio.clone();
        vp.audio_track_selected.connect(move |index: std::sync::Arc<usize>| {
            a.lock().unwrap().push(*index);
        });
        let subtitle = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let s = subtitle.clone();
        vp.subtitle_track_selected.connect(move |index: std::sync::Arc<usize>| {
            s.lock().unwrap().push(*index);
        });

        vp.set_audio_tracks(vec![AudioTrack::new("a1", "One"), AudioTrack::new("a2", "Two")]);
        vp.set_audio_track(1).expect("the second track exists");
        vp.set_audio_track(1).expect("re-selecting is a no-op");
        assert_eq!(*audio.lock().unwrap(), vec![1], "only a real switch is announced");

        vp.set_subtitle_tracks(vec![
            SubtitleTrack::new("s1", "One", vec![SubtitleCue::new(0.0, 1.0, "a")]),
            SubtitleTrack::new("s2", "Two", vec![SubtitleCue::new(0.0, 1.0, "b")]),
        ]);
        vp.set_subtitle_track(1).expect("the second track exists");
        vp.set_subtitle_track(1).expect("re-selecting is a no-op");
        assert_eq!(*subtitle.lock().unwrap(), vec![1]);
    }

    /// An attached companion follows the player rather than taking a one-time snapshot (MW-016).
    #[test]
    fn an_attached_subtitle_bar_follows_the_player_clock() {
        use crate::widget::media_widgets::subtitle_bar::SubtitleBar;
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(10.0);
        vp.set_subtitle_tracks(vec![SubtitleTrack::new(
            "s1",
            "English",
            vec![SubtitleCue::new(0.0, 2.0, "hello"), SubtitleCue::new(2.0, 4.0, "world")],
        )]);
        vp.set_subtitle_track(0).expect("the track exists");
        vp.attach_subtitle_bar(SubtitleBar::new(Rect::new(0, 0, 320, 48)));
        assert!(vp.subtitle_bar().is_some_and(SubtitleBar::is_following_player));
        vp.seek(3.0);
        assert_eq!(vp.subtitle_bar().map(SubtitleBar::current_caption), Some("world".to_string()));
        let detached = vp.detach_subtitle_bar();
        assert!(detached.is_some());
        assert!(vp.subtitle_bar().is_none());
    }
}
