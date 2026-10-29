// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `media` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_media.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

#[cfg(not(alloc_frugal))]
pub(crate) fn animated_image_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AnimatedImage,
        canonical_name: "animated_image",
        aliases: &["anim_image"],
        properties: ANIMATED_IMAGE_PROPERTIES,
        // `animation_finished` and `frame_changed` are the signals the control emits; the
        // published name was `finished`, which nothing ever fired. See
        // `auto_complete_edit` for why a name that does not match its signal is a live-looking
        // subscription rather than a visible error.
        events: events_of!("animated_image"),
        commands: &["set_playing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn hero_animation_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::HeroAnimation,
        canonical_name: "hero_animation",
        aliases: &["hero"],
        properties: HERO_ANIMATION_PROPERTIES,
        events: events_of!("hero_animation"),
        commands: &["set_animation_progress"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn lottie_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LottieWidget,
        canonical_name: "lottie_widget",
        aliases: &["lottie"],
        properties: LOTTIE_WIDGET_PROPERTIES,
        // `animation_finished` is the emitted signal; the published name was `finished`.
        events: events_of!("lottie_widget"),
        commands: &["set_playing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn rive_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RiveWidget,
        canonical_name: "rive_widget",
        aliases: &["rive"],
        properties: RIVE_WIDGET_PROPERTIES,
        // `animation_finished` is the emitted signal; the published name was `finished`.
        events: events_of!("rive_widget"),
        commands: &["set_is_playing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn video_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "video_player",
        aliases: &["video"],
        properties: VIDEO_PLAYER_PROPERTIES,
        // The player publishes the four signals it actually emits; `finished` matched none of
        // them.
        events: events_of!("video_player"),
        commands: &["set_is_playing", "set_volume"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn audio_visualizer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AudioVisualizer,
        canonical_name: "audio_visualizer",
        aliases: &["audio_viz"],
        properties: AUDIO_VISUALIZER_PROPERTIES,
        events: events_of!("audio_visualizer"),
        commands: &["set_bar_count"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn camera_preview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CameraPreview,
        canonical_name: "camera_preview",
        aliases: &["camera"],
        properties: CAMERA_PREVIEW_PROPERTIES,
        events: events_of!("camera_preview"),
        commands: &["set_is_active"],
    }
}

/// Media chapter list (COMP-A6).
///
/// It shares `WidgetKind::VideoPlayer` with the player whose chapters it lists.
#[cfg(not(alloc_frugal))]
pub(crate) fn chapter_list_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "chapter_list_panel",
        aliases: &[],
        properties: CHAPTER_LIST_PANEL_PROPERTIES,
        events: events_of!("chapter_list_panel"),
        commands: &[],
    }
}
