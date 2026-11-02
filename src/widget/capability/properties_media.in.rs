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
            // RW-83: the frame scrubber the designed preview needs.
            PropertySchema::new("current_frame", PropertyValueKind::UInt, true, true),
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

    };
}
