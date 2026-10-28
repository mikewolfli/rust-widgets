// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_media {
    () => {
        #[cfg(not(alloc_frugal))]
        pub(crate) const ANIMATED_IMAGE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("direction", true, true, &["forward", "reverse"]),
            PropertySchema::new("frame_cache_limit", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const HERO_ANIMATION_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("animation_progress", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const LOTTIE_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("loop_count", PropertyValueKind::Int, true, true),
            PropertySchema::new("loops_completed", PropertyValueKind::Int, true, false),
            PropertySchema::enumerated("direction", true, true, &["forward", "reverse"]),
            PropertySchema::new("speed", PropertyValueKind::Float, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RIVE_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("is_playing", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("loop_count", PropertyValueKind::Int, true, true),
            PropertySchema::new("loops_completed", PropertyValueKind::Int, true, false),
            PropertySchema::new("state", PropertyValueKind::String, true, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const VIDEO_PLAYER_PROPERTIES: &[PropertySchema] = &[
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

        /// Media chapter list (COMP-A6). `position_ms` comes from the attached player, so it is
        /// read-only here; the row metrics are the list's own.
        #[cfg(not(alloc_frugal))]
        pub(crate) const CHAPTER_LIST_PANEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("active_highlight", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_thumbnails", PropertyValueKind::Bool, true, true),
            PropertySchema::new("chapters", PropertyValueKind::UInt, true, false),
            PropertySchema::new("position_ms", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const AUDIO_VISUALIZER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("bar_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("peak_threshold", PropertyValueKind::Float, true, true),
            PropertySchema::enumerated("mode", true, true, &["waveform", "spectrum"]),
            PropertySchema::new("level_report_interval", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CAMERA_PREVIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("is_active", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("pixel_format", PropertyValueKind::String, true, true),
            PropertySchema::new("target_fps", PropertyValueKind::UInt, true, true),
            PropertySchema::new("camera_id", PropertyValueKind::UInt, true, true),
        ];
    };
}
