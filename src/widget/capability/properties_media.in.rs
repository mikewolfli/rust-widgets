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
        pub(crate) const ANIMATED_IMAGE_PROPERTIES: &[PropertySchema] = &[PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
        PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
        PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
        PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
        PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const HERO_ANIMATION_PROPERTIES: &[PropertySchema] = &[PropertySchema::new("animation_progress", PropertyValueKind::Float, true, true),
        PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
        PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
        PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
        PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const LOTTIE_WIDGET_PROPERTIES: &[PropertySchema] = &[PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
        PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
        PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
        PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
        PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RIVE_WIDGET_PROPERTIES: &[PropertySchema] = &[PropertySchema::new("is_playing", PropertyValueKind::Bool, true, true),
        PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
        PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
        PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
        PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const VIDEO_PLAYER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("is_playing", PropertyValueKind::Bool, true, true),
            PropertySchema::new("volume", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const AUDIO_VISUALIZER_PROPERTIES: &[PropertySchema] = &[PropertySchema::new("bar_count", PropertyValueKind::UInt, true, true),
        PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
        PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
        PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
        PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CAMERA_PREVIEW_PROPERTIES: &[PropertySchema] = &[PropertySchema::new("is_active", PropertyValueKind::Bool, true, true),
        PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
        PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
        PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
        PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
