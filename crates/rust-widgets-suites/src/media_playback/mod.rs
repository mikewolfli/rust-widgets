// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `media-playback` group: the media player family, the animation players and the previews.
//!
//! # Why these are one group
//!
//! Every control here plays or previews timed content: the player and its transport chrome, the
//! Lottie and Rive animation players, the audio visualiser and the camera preview. They share the
//! same clock model and the same host-side wiring, so a host that wants one usually wants all.
//!
//! # Adding a group later
//!
//! A group is a module under `src/` with a `capability` submodule holding its property schemas,
//! event schemas and constructors, and an `install(factory)` function. A `mod.rs` only declares
//! and re-exports (principle #8).

/// The audio visualiser.
pub mod audio_visualizer;
/// The live camera preview.
pub mod camera_preview;
mod capability;
/// The chapter list beside a player.
pub mod chapter_list_panel;
/// The Lottie animation player.
pub mod lottie_widget;
/// The media player family: the player and its transport, scrubber and subtitle chrome.
pub mod media_player;
/// The Rive animation player.
pub mod rive_widget;
#[cfg(test)]
mod tests;
/// The video player.
pub mod video_player;

pub use audio_visualizer::AudioVisualizer;
pub use camera_preview::{CameraDevice, CameraFrame, CameraPreview, PixelFormat};
pub use capability::{
    audio_visualizer_capability, camera_preview_capability, chapter_list_panel_capability,
    create_audio_visualizer, create_camera_preview, create_chapter_list_panel,
    create_lottie_widget, create_media_player, create_media_scrubber, create_media_transport_bar,
    create_rive_widget, create_subtitle_bar, create_video_player, default_property_value, install,
    lottie_widget_capability, media_player_capability, media_scrubber_capability,
    media_transport_bar_capability, published_event_names, published_property_names,
    rive_widget_capability, subtitle_bar_capability, video_player_capability,
    AUDIO_VISUALIZER_PROPERTIES, CAMERA_PREVIEW_PROPERTIES, CHAPTER_LIST_PANEL_PROPERTIES,
    LOTTIE_WIDGET_PROPERTIES, MEDIA_PLAYER_PROPERTIES, MEDIA_SCRUBBER_PROPERTIES,
    MEDIA_TRANSPORT_BAR_PROPERTIES, RIVE_WIDGET_PROPERTIES, SUBTITLE_BAR_PROPERTIES,
    VIDEO_PLAYER_PROPERTIES,
};
pub use chapter_list_panel::ChapterListPanel;
pub use lottie_widget::{
    LottieDirection, LottieFontProvider, LottieImage, LottieImageLayer, LottieImageProvider,
    LottieTextLayer, LottieWidget,
};
pub use media_player::media_scrubber::{MediaScrubber, ScrubberThumbStyle};
pub use media_player::media_transport::{MediaTargetKind, MediaTransportBar};
pub use media_player::subtitle_bar::{SubtitleBar, SubtitlePosition};
pub use media_player::{media_scrubber, media_transport, subtitle_bar, MediaPlayer};
pub use rive_widget::{RiveInput, RiveInputValue, RiveState, RiveWidget};
pub use video_player::{
    AudioTrack, SubtitleTrack, VideoChapter, VideoFrame, VideoMetadata, VideoPipelineState,
    VideoPixelFormat, VideoPlayer,
};
