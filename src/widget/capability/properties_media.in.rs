// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

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
