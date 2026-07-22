// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_media {
    () => {
        #[cfg(not(feature = "mini"))]
        pub(crate) const ANIMATED_IMAGE_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "playing",
            value_kind: PropertyValueKind::Bool,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const HERO_ANIMATION_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "animation_progress",
            value_kind: PropertyValueKind::Float,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const LOTTIE_WIDGET_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "playing",
            value_kind: PropertyValueKind::Bool,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const RIVE_WIDGET_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "is_playing",
            value_kind: PropertyValueKind::Bool,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const VIDEO_PLAYER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "is_playing",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "volume",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const AUDIO_VISUALIZER_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "bar_count",
            value_kind: PropertyValueKind::UInt,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const CAMERA_PREVIEW_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "is_active",
            value_kind: PropertyValueKind::Bool,
            readable: true,
            writable: true,
        }];
    };
}
