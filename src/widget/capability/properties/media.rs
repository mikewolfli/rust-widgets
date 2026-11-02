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
