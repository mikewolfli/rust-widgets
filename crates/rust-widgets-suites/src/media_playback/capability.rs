// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability rows, property schemas and constructors of this suite group.
//!
//! A capability is the designer's entire view of a control: the name it is created by, the
//! property contract with a type and an accessibility flag per row, and the published events
//! with their payload kinds. All of those are declared here, next to the constructors, so the
//! registration and the documentation cannot describe different controls.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::apply_control_label;
use rust_widgets::widget::capability::types::{
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema, PropertyValueKind,
    PropertyValueKind as K, RegistrationError, WidgetCapability, WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};

use crate::media_playback::audio_visualizer::AudioVisualizer;
use crate::media_playback::camera_preview::CameraPreview;
use crate::media_playback::chapter_list_panel::{ChapterListPanel, CHAPTER_RECORD_SCHEMA};
use crate::media_playback::lottie_widget::LottieWidget;
use crate::media_playback::media_player::media_player::MediaPlayer;
use crate::media_playback::media_player::media_scrubber::MediaScrubber;
use crate::media_playback::media_player::media_transport::MediaTransportBar;
use crate::media_playback::media_player::subtitle_bar::SubtitleBar;
use crate::media_playback::rive_widget::RiveWidget;
use crate::media_playback::video_player::VideoPlayer;

pub const VIDEO_PLAYER_PROPERTIES: &[PropertySchema] = &[
    // RW-82: the media source, plus the two read-only clock scalars. `source` is stored as
    // a label by the player (it does not open or demux), but it is the value the host set
    // and the value a document must round-trip.
    PropertySchema::new("source", PropertyValueKind::String, true, true),
    PropertySchema::new("position", PropertyValueKind::Float, true, false),
    PropertySchema::new("duration", PropertyValueKind::Float, true, false),
    PropertySchema::new("is_playing", PropertyValueKind::Bool, true, true),
    PropertySchema::new("volume", PropertyValueKind::Float, true, true),
    PropertySchema::new("muted", PropertyValueKind::Bool, true, true),
    PropertySchema::new("playback_rate", PropertyValueKind::Float, true, true),
    PropertySchema::new("controls_visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("looping", PropertyValueKind::Bool, true, true),
    // The host's own buffer-starvation report; see `VideoPlayer::set_buffering` (MW-003).
    PropertySchema::new("buffering", PropertyValueKind::Bool, true, true),
    PropertySchema::new("pipeline", PropertyValueKind::String, true, false),
];

pub const LOTTIE_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
    // RW-83: the frame scrubber the designed preview needs.
    PropertySchema::new("current_frame", PropertyValueKind::UInt, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("loop_count", PropertyValueKind::Int, true, true),
    PropertySchema::new("loops_completed", PropertyValueKind::Int, true, false),
    PropertySchema::enumerated("direction", true, true, &["forward", "reverse"]),
    PropertySchema::new("speed", PropertyValueKind::Float, true, true),
    // RW-106: the Lottie JSON document itself. Written through `load_json`, read back from
    // `json_data`, so a design document references the animation it shows.
    PropertySchema::new("source", PropertyValueKind::String, true, true),
];

pub const RIVE_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("is_playing", PropertyValueKind::Bool, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("loop_count", PropertyValueKind::Int, true, true),
    PropertySchema::new("loops_completed", PropertyValueKind::Int, true, false),
    PropertySchema::new("state", PropertyValueKind::String, true, false),
    // RW-106: the `.riv` document the host resolves, plus the progress setter the control already
    // has (`set_progress`). The path is stored as a label, exactly as `video_player.source` is:
    // the control does not open files behind the host's back.
    PropertySchema::new("source", PropertyValueKind::String, true, true),
    PropertySchema::new("progress", PropertyValueKind::Float, true, true),
];

pub const AUDIO_VISUALIZER_PROPERTIES: &[PropertySchema] = &[
    // RW-86: design-time sample data, fed through the control's own sample API.
    PropertySchema::new("sample_data", PropertyValueKind::NumberList, true, true),
    PropertySchema::new("bar_count", PropertyValueKind::UInt, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("peak_threshold", PropertyValueKind::Float, true, true),
    PropertySchema::enumerated("mode", true, true, &["waveform", "spectrum"]),
    PropertySchema::new("level_report_interval", PropertyValueKind::UInt, true, true),
];

pub const CAMERA_PREVIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("is_active", PropertyValueKind::Bool, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("pixel_format", PropertyValueKind::String, true, true),
    PropertySchema::new("target_fps", PropertyValueKind::UInt, true, true),
    PropertySchema::new("camera_id", PropertyValueKind::UInt, true, true),
];

/// Media chapter list (COMP-A6). `position_ms` comes from the attached player, so it is
/// read-only here; the row metrics are the list's own.
pub const CHAPTER_LIST_PANEL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
    PropertySchema::new("active_highlight", PropertyValueKind::Bool, true, true),
    PropertySchema::new("show_thumbnails", PropertyValueKind::Bool, true, true),
    // RW-106: the rows themselves, writable; the count is published under its own name so the
    // two facts stop sharing one property (`chapters` used to be the read-only count).
    PropertySchema::records("chapters", true, true, &CHAPTER_RECORD_SCHEMA),
    PropertySchema::new("chapter_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("position_ms", PropertyValueKind::UInt, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

pub const MEDIA_PLAYER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("source", PropertyValueKind::String, true, true),
    PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
    PropertySchema::new("duration_ms", PropertyValueKind::UInt, true, false),
    PropertySchema::new("position_ms", PropertyValueKind::UInt, true, true),
    PropertySchema::new("volume", PropertyValueKind::UInt, true, true),
    PropertySchema::new("muted", PropertyValueKind::Bool, true, true),
    PropertySchema::new("fullscreen", PropertyValueKind::Bool, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("playlist_index", PropertyValueKind::UInt, true, true),
    PropertySchema::new("track_count", PropertyValueKind::UInt, true, false),
];

/// Media transport chrome (RW-81). Registered under `VideoPlayer` — the control it drives —
/// with its own canonical name so a manifest can tell the two apart.
pub const MEDIA_TRANSPORT_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
    PropertySchema::new("position_ms", PropertyValueKind::UInt, true, false),
    PropertySchema::new("duration_ms", PropertyValueKind::UInt, true, false),
    PropertySchema::new("volume", PropertyValueKind::Float, true, true),
    PropertySchema::new("auto_hide_ms", PropertyValueKind::UInt, true, true),
    PropertySchema::new("show_volume", PropertyValueKind::Bool, true, true),
    PropertySchema::new("show_timecode", PropertyValueKind::Bool, true, true),
    PropertySchema::new("show_fullscreen", PropertyValueKind::Bool, true, true),
    PropertySchema::new("show_next_previous", PropertyValueKind::Bool, true, true),
    // The host may tint the bar; `None` follows the theme accent.
    PropertySchema::new("accent", PropertyValueKind::Color, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Draggable media progress track (RW-81). Registered under `Slider` with its own
/// canonical name.
pub const MEDIA_SCRUBBER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("value", PropertyValueKind::Float, true, true),
    PropertySchema::new("buffered", PropertyValueKind::Float, true, true),
    // The chapter marks are the control's reason to exist; the count alone could not be written
    // (the same lesson `chapter_list_panel.chapters` records).
    PropertySchema::new("chapters", PropertyValueKind::NumberList, true, true),
    PropertySchema::new("step_ms", PropertyValueKind::UInt, true, true),
    PropertySchema::new("chapter_count", PropertyValueKind::UInt, true, false),
    PropertySchema::enumerated("thumb_style", true, true, &["dot", "handle"]),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Multi-line caption band (RW-81). Registered under `Label` with its own canonical name.
pub const SUBTITLE_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("text", PropertyValueKind::String, true, true),
    PropertySchema::new("max_lines", PropertyValueKind::UInt, true, true),
    PropertySchema::new("font_scale", PropertyValueKind::Float, true, true),
    PropertySchema::new("background_opacity", PropertyValueKind::UInt, true, true),
    PropertySchema::enumerated("position", true, true, &["bottom", "top"]),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// `video_player`'s published events.
const VIDEO_PLAYER_EVENTS: &[EventSchema] = &[
    EventSchema { name: "audio_track_selected", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema {
        name: "buffering_progress_changed",
        payload: Some(K::Float),
        shape: Some(S::Scalar),
    },
    EventSchema { name: "duration_changed", payload: Some(K::Float), shape: Some(S::Scalar) },
    EventSchema { name: "error_occurred", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "frame_available", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "frame_presented", payload: Some(K::Float), shape: Some(S::Scalar) },
    EventSchema { name: "fullscreen_changed", payload: Some(K::Bool), shape: Some(S::Scalar) },
    EventSchema { name: "media_status_changed", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema::unit("metadata_changed"),
    EventSchema { name: "mute_changed", payload: Some(K::Bool), shape: Some(S::Scalar) },
    EventSchema { name: "chapter_entered", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema::unit("playback_ended"),
    EventSchema::unit("playback_paused"),
    EventSchema { name: "playback_rate_changed", payload: Some(K::Float), shape: Some(S::Scalar) },
    EventSchema::unit("playback_started"),
    EventSchema { name: "subtitle_track_selected", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "time_updated", payload: Some(K::Float), shape: Some(S::Scalar) },
    EventSchema { name: "volume_changed", payload: Some(K::Float), shape: Some(S::Scalar) },
];

/// `lottie_widget`'s published events.
const LOTTIE_WIDGET_EVENTS: &[EventSchema] = &[
    EventSchema::unit("animation_finished"),
    EventSchema { name: "frame_changed", payload: Some(K::UInt), shape: Some(S::Scalar) },
];

/// `rive_widget`'s published events.
const RIVE_WIDGET_EVENTS: &[EventSchema] = &[
    EventSchema::unit("animation_finished"),
    EventSchema { name: "input_fired", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "state_changed", payload: Some(K::String), shape: Some(S::Scalar) },
];

/// `audio_visualizer`'s published events.
const AUDIO_VISUALIZER_EVENTS: &[EventSchema] = &[
    EventSchema { name: "level_changed", payload: Some(K::Float), shape: Some(S::Scalar) },
    EventSchema { name: "mode_changed", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "peak_exceeded", payload: Some(K::Float), shape: Some(S::Scalar) },
];

/// `camera_preview`'s published events.
const CAMERA_PREVIEW_EVENTS: &[EventSchema] = &[
    EventSchema::unit("capture_requested"),
    EventSchema { name: "error_occurred", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "format_negotiated", payload: Some(K::UInt), shape: Some(S::Tuple2) },
    EventSchema { name: "frame_available", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "frame_presented", payload: Some(K::Float), shape: Some(S::Scalar) },
    EventSchema::unit("permission_denied"),
];

/// `chapter_list_panel`'s published events.
const CHAPTER_LIST_PANEL_EVENTS: &[EventSchema] =
    &[EventSchema { name: "chapter_selected", payload: Some(K::UInt), shape: Some(S::Scalar) }];

/// `media_player`'s published events.
const MEDIA_PLAYER_EVENTS: &[EventSchema] = &[
    EventSchema { name: "chapter_selected", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "fullscreen_requested", payload: Some(K::Bool), shape: Some(S::Scalar) },
    EventSchema::unit("media_ended"),
    EventSchema { name: "playback_changed", payload: Some(K::Bool), shape: Some(S::Scalar) },
    EventSchema { name: "position_changed", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "seek_preview", payload: Some(K::UInt), shape: Some(S::OptionalScalar) },
    EventSchema { name: "source_changed", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "volume_changed", payload: Some(K::UInt), shape: Some(S::Scalar) },
];

/// `media_transport_bar`'s published events.
const MEDIA_TRANSPORT_BAR_EVENTS: &[EventSchema] = &[
    EventSchema::unit("fullscreen_requested"),
    EventSchema::unit("next_track_requested"),
    EventSchema { name: "play_toggled", payload: Some(K::Bool), shape: Some(S::Scalar) },
    EventSchema::unit("previous_track_requested"),
    EventSchema { name: "seek_requested", payload: Some(K::Float), shape: Some(S::Scalar) },
    EventSchema { name: "volume_changed", payload: Some(K::Float), shape: Some(S::Scalar) },
];

/// `media_scrubber`'s published events.
const MEDIA_SCRUBBER_EVENTS: &[EventSchema] = &[
    EventSchema { name: "committed", payload: Some(K::Float), shape: Some(S::Scalar) },
    // `(is_active, progress)`: mixed parts, so the declaration names the first part's kind and
    // reports the mixed shape rather than pretending the pair is one float.
    EventSchema { name: "scrubbing", payload: Some(K::Bool), shape: Some(S::Mixed) },
];

/// `subtitle_bar`'s published events.
const SUBTITLE_BAR_EVENTS: &[EventSchema] = &[EventSchema::unit("text_changed")];

pub fn video_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "video_player",
        aliases: &["video"],
        properties: VIDEO_PLAYER_PROPERTIES,
        // The player publishes the four signals it actually emits; `finished` matched none of
        // them.
        events: VIDEO_PLAYER_EVENTS,
        commands: &["set_is_playing", "set_volume"],
    }
}

pub fn lottie_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LottieWidget,
        canonical_name: "lottie_widget",
        aliases: &["lottie"],
        properties: LOTTIE_WIDGET_PROPERTIES,
        // `animation_finished` is the emitted signal; the published name was `finished`.
        events: LOTTIE_WIDGET_EVENTS,
        commands: &["set_playing", "set_source", "play", "pause", "stop"],
    }
}

pub fn rive_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RiveWidget,
        canonical_name: "rive_widget",
        aliases: &["rive"],
        properties: RIVE_WIDGET_PROPERTIES,
        // `animation_finished` is the emitted signal; the published name was `finished`.
        events: RIVE_WIDGET_EVENTS,
        commands: &["set_is_playing", "set_source", "set_progress"],
    }
}

pub fn audio_visualizer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AudioVisualizer,
        canonical_name: "audio_visualizer",
        aliases: &["audio_viz"],
        properties: AUDIO_VISUALIZER_PROPERTIES,
        events: AUDIO_VISUALIZER_EVENTS,
        commands: &["set_bar_count"],
    }
}

pub fn camera_preview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CameraPreview,
        canonical_name: "camera_preview",
        aliases: &["camera"],
        properties: CAMERA_PREVIEW_PROPERTIES,
        events: CAMERA_PREVIEW_EVENTS,
        commands: &["set_is_active"],
    }
}

/// Media chapter list (COMP-A6).
///
/// It shares `WidgetKind::VideoPlayer` with the player whose chapters it lists.
pub fn chapter_list_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "chapter_list_panel",
        aliases: &[],
        properties: CHAPTER_LIST_PANEL_PROPERTIES,
        events: CHAPTER_LIST_PANEL_EVENTS,
        // RW-106: the full-list write; add/remove need a composite payload the command vocabulary
        // cannot express yet.
        commands: &["set_chapters"],
    }
}

pub fn media_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
        canonical_name: "media_player",
        aliases: &[],
        properties: MEDIA_PLAYER_PROPERTIES,
        events: MEDIA_PLAYER_EVENTS,
        // Audit 20261026: `seek_to` stays unpublished — the position is the writable
        // `position_ms` property, so a seek is that property's write (principle #18).
        commands: &[
            "set_source",
            "clear_source",
            "play",
            "pause",
            "set_volume",
            "next_track",
            "previous_track",
        ],
    }
}

/// Media transport chrome (RW-81).
///
/// A borrowed kind is the honest choice: the bar *is* the player's control surface, and this crate's
/// kind table has no separate variant for a companion chrome strip. The canonical name is what
/// distinguishes the row from `video_player`. Its six request/state signals
/// (`play_toggled`, `seek_requested`, `volume_changed`, ...) are published as events; the bar
/// emits them itself and a host may subscribe either directly or through the capability.
pub fn media_transport_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "media_transport_bar",
        aliases: &[],
        properties: MEDIA_TRANSPORT_BAR_PROPERTIES,
        events: MEDIA_TRANSPORT_BAR_EVENTS,
        // RW-106: the value-carrying setters route through the properties, and the three
        // payload-free requests mirror the bar's own buttons.
        commands: &[
            "set_playing",
            "set_volume",
            "set_auto_hide_ms",
            "set_show_volume",
            "set_show_timecode",
            "set_show_fullscreen",
            "set_show_next_previous",
            "set_accent",
            "previous_track",
            "next_track",
            "toggle_fullscreen",
        ],
    }
}

/// Draggable media progress track (RW-81). Borrowed kind `Slider`, own canonical name.
pub fn media_scrubber_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Slider,
        canonical_name: "media_scrubber",
        aliases: &[],
        properties: MEDIA_SCRUBBER_PROPERTIES,
        events: MEDIA_SCRUBBER_EVENTS,
        commands: &["set_value"],
    }
}

/// Multi-line caption band (RW-81). Borrowed kind `Label`, own canonical name.
pub fn subtitle_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Label,
        canonical_name: "subtitle_bar",
        aliases: &[],
        properties: SUBTITLE_BAR_PROPERTIES,
        events: SUBTITLE_BAR_EVENTS,
        commands: &["set_text"],
    }
}

/// Creates a video player with no media loaded and playback stopped. `text` is
/// applied as the control's label.
pub fn create_video_player(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(VideoPlayer::new(geometry)), text)
}

/// Creates a view for a Lottie animation with no animation loaded. `text` is
/// applied as the control's label.
pub fn create_lottie_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(LottieWidget::new(geometry)), text)
}

/// Creates a view for a Rive animation with no animation loaded. `text` is
/// applied as the control's label.
pub fn create_rive_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(RiveWidget::new(geometry)), text)
}

/// Creates an audio visualiser with no audio source attached. `text` is applied
/// as the control's label.
pub fn create_audio_visualizer(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(AudioVisualizer::new(geometry)), text)
}

/// Creates a live camera preview with no camera opened. `text` is applied as the
/// control's label.
pub fn create_camera_preview(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(CameraPreview::new(geometry)), text)
}

/// Creates a media chapter list with no chapters (COMP-A6). `text` is applied as the control's
/// label.
pub fn create_chapter_list_panel(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(ChapterListPanel::new(geometry)), text)
}

/// Creates a media player with no source loaded. `text` is applied as the
/// control's label.
pub fn create_media_player(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(MediaPlayer::new(geometry)), text)
}

/// Creates a media transport bar (RW-81).
pub fn create_media_transport_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MediaTransportBar::new(geometry))
}

/// Creates a media scrubber at `value = 0.0` (RW-81).
pub fn create_media_scrubber(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MediaScrubber::new(geometry))
}

/// Creates an empty subtitle band (RW-81).
pub fn create_subtitle_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SubtitleBar::new(geometry))
}

/// A registered control: its capability row, its constructor, and its defaults provider.
type MediaRow = (WidgetCapability, WidgetCtor, fn(&str) -> Option<CapabilityValue>);

/// Registers the media-playback suite in `factory`.
///
/// Returns the first name that could not be claimed. A factory that already answers one of these
/// names (a host that installed twice, or a build that ships a control of the same name) is
/// reported rather than silently shadowed.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    let rows: [MediaRow; 10] = [
        (video_player_capability(), create_video_player, |p| {
            default_property_value("video_player", p)
        }),
        (lottie_widget_capability(), create_lottie_widget, |p| {
            default_property_value("lottie_widget", p)
        }),
        (rive_widget_capability(), create_rive_widget, |p| {
            default_property_value("rive_widget", p)
        }),
        (audio_visualizer_capability(), create_audio_visualizer, |p| {
            default_property_value("audio_visualizer", p)
        }),
        (camera_preview_capability(), create_camera_preview, |p| {
            default_property_value("camera_preview", p)
        }),
        (chapter_list_panel_capability(), create_chapter_list_panel, |p| {
            default_property_value("chapter_list_panel", p)
        }),
        (media_player_capability(), create_media_player, |p| {
            default_property_value("media_player", p)
        }),
        (media_transport_bar_capability(), create_media_transport_bar, |p| {
            default_property_value("media_transport_bar", p)
        }),
        (media_scrubber_capability(), create_media_scrubber, |p| {
            default_property_value("media_scrubber", p)
        }),
        (subtitle_bar_capability(), create_subtitle_bar, |p| {
            default_property_value("subtitle_bar", p)
        }),
    ];
    for (capability, ctor, defaults) in rows {
        let name = capability.canonical_name;
        factory.try_register(capability, ctor)?;
        let _ = factory.register_property_defaults(name, defaults);
    }
    Ok(())
}

/// A property's declared default, for the capability manifest.
///
/// Mirrors what the controls' constructors set, so a dropped binding resets to the value a fresh
/// control would have had.
pub fn default_property_value(kind: &str, name: &str) -> Option<CapabilityValue> {
    match (kind, name) {
        // RW-82: no source loaded and the clock at zero.
        ("video_player", "source") => Some(CapabilityValue::String(String::new())),
        ("video_player", "position" | "duration") => Some(CapabilityValue::Float(0.0)),
        ("video_player", "is_playing" | "muted" | "looping" | "buffering") => {
            Some(CapabilityValue::Bool(false))
        }
        ("video_player", "volume") => Some(CapabilityValue::Float(0.8)),
        ("video_player", "playback_rate") => Some(CapabilityValue::Float(1.0)),
        ("video_player", "controls_visible") => Some(CapabilityValue::Bool(true)),
        ("video_player", "pipeline") => Some(CapabilityValue::String("Idle".to_string())),
        // `media_player` borrows `WidgetKind::WebEngineView`; these are its own rows.
        ("media_player", "source") => Some(CapabilityValue::Null),
        ("media_player", "playing" | "muted" | "fullscreen") => Some(CapabilityValue::Bool(false)),
        ("media_player", "duration_ms" | "position_ms" | "track_count") => {
            Some(CapabilityValue::UInt(0))
        }
        ("media_player", "volume") => Some(CapabilityValue::UInt(80)),
        ("media_player", "playlist_index") => Some(CapabilityValue::Null),
        ("media_transport_bar", "playing") => Some(CapabilityValue::Bool(false)),
        ("media_transport_bar", "position_ms" | "duration_ms") => Some(CapabilityValue::UInt(0)),
        ("media_transport_bar", "volume") => Some(CapabilityValue::Float(0.8)),
        ("media_transport_bar", "auto_hide_ms") => Some(CapabilityValue::UInt(3000)),
        ("media_transport_bar", "show_volume" | "show_timecode") => {
            Some(CapabilityValue::Bool(true))
        }
        // `show_fullscreen` constructs `true`, `show_next_previous` constructs `false`; the
        // manifest defaults must describe the controls, not a sibling's choice.
        ("media_transport_bar", "show_fullscreen") => Some(CapabilityValue::Bool(true)),
        ("media_transport_bar", "show_next_previous") => Some(CapabilityValue::Bool(false)),
        ("media_transport_bar", "accent") => Some(CapabilityValue::Null),
        ("media_scrubber", "value" | "buffered") => Some(CapabilityValue::Float(0.0)),
        ("media_scrubber", "chapters") => Some(CapabilityValue::Tuple(Vec::new())),
        ("media_scrubber", "step_ms") => Some(CapabilityValue::UInt(0)),
        ("media_scrubber", "chapter_count") => Some(CapabilityValue::UInt(0)),
        ("media_scrubber", "thumb_style") => Some(CapabilityValue::String("dot".to_string())),
        ("subtitle_bar", "text") => Some(CapabilityValue::String(String::new())),
        ("subtitle_bar", "max_lines") => Some(CapabilityValue::UInt(3)),
        ("subtitle_bar", "font_scale") => Some(CapabilityValue::Float(1.0)),
        ("subtitle_bar", "background_opacity") => Some(CapabilityValue::UInt(150)),
        ("subtitle_bar", "position") => Some(CapabilityValue::String("bottom".to_string())),
        ("chapter_list_panel", "row_height") => Some(CapabilityValue::UInt(32)),
        ("chapter_list_panel", "active_highlight") => Some(CapabilityValue::Bool(true)),
        ("chapter_list_panel", "show_thumbnails") => Some(CapabilityValue::Bool(false)),
        ("chapter_list_panel", "chapter_count" | "position_ms") => Some(CapabilityValue::UInt(0)),
        // RW-106: a fresh panel has no chapters; the empty list of the declared schema.
        ("chapter_list_panel", "chapters") => Some(CapabilityValue::Records(
            rust_widgets::widget::capability::RecordList::new(&CHAPTER_RECORD_SCHEMA),
        )),
        // RW-106: nothing loaded yet, at frame/loop zero. The parent's kind-level table does not
        // know these rows (they are this control's own), so they are declared here.
        ("lottie_widget", "source") => Some(CapabilityValue::String(String::new())),
        ("rive_widget", "source") => Some(CapabilityValue::String(String::new())),
        ("rive_widget", "progress") => Some(CapabilityValue::Float(0.0)),
        _ => None,
    }
}

/// Every property name the media-playback suite publishes, for a quick completeness check.
pub fn published_property_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("video_player", VIDEO_PLAYER_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("lottie_widget", LOTTIE_WIDGET_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("rive_widget", RIVE_WIDGET_PROPERTIES.iter().map(|schema| schema.name).collect()),
        (
            "audio_visualizer",
            AUDIO_VISUALIZER_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        ("camera_preview", CAMERA_PREVIEW_PROPERTIES.iter().map(|schema| schema.name).collect()),
        (
            "chapter_list_panel",
            CHAPTER_LIST_PANEL_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        ("media_player", MEDIA_PLAYER_PROPERTIES.iter().map(|schema| schema.name).collect()),
        (
            "media_transport_bar",
            MEDIA_TRANSPORT_BAR_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        ("media_scrubber", MEDIA_SCRUBBER_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("subtitle_bar", SUBTITLE_BAR_PROPERTIES.iter().map(|schema| schema.name).collect()),
    ]
}

/// Every event name the media-playback suite publishes, for a quick completeness check.
pub fn published_event_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("video_player", VIDEO_PLAYER_EVENTS.iter().map(|schema| schema.name).collect()),
        ("lottie_widget", LOTTIE_WIDGET_EVENTS.iter().map(|schema| schema.name).collect()),
        ("rive_widget", RIVE_WIDGET_EVENTS.iter().map(|schema| schema.name).collect()),
        ("audio_visualizer", AUDIO_VISUALIZER_EVENTS.iter().map(|schema| schema.name).collect()),
        ("camera_preview", CAMERA_PREVIEW_EVENTS.iter().map(|schema| schema.name).collect()),
        (
            "chapter_list_panel",
            CHAPTER_LIST_PANEL_EVENTS.iter().map(|schema| schema.name).collect(),
        ),
        ("media_player", MEDIA_PLAYER_EVENTS.iter().map(|schema| schema.name).collect()),
        (
            "media_transport_bar",
            MEDIA_TRANSPORT_BAR_EVENTS.iter().map(|schema| schema.name).collect(),
        ),
        ("media_scrubber", MEDIA_SCRUBBER_EVENTS.iter().map(|schema| schema.name).collect()),
        ("subtitle_bar", SUBTITLE_BAR_EVENTS.iter().map(|schema| schema.name).collect()),
    ]
}
