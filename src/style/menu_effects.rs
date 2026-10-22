// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The menu subsystem's effect budget.
//!
//! # Why this type lives in `style`
//!
//! `MenuConfig` owns the **policy** (what hardware was detected and what the user asked
//! for). The menu controls own the **painting**. This type is the snapshot that crosses
//! between them, and it is deliberately free of both dependencies: `menu_config` can
//! build one, `widget::menu_toolbar` can consume one, and a build with the
//! `advanced-widgets` feature off still has the type available for
//! [`MenuEffects::legacy`].
//!
//! # Why `legacy` is the default
//!
//! Wiring an existing control to a configuration source must not change a single pixel
//! until the host opts in. [`MenuEffects::legacy`] is exactly the rendering the menu
//! family produced before this type existed — popups animate at the theme's duration, no
//! translucency, no elevation shadow, and each control keeps its own historical visible
//! item cap. `MenuConfig::enabled == false` maps to it, which is why the snapshot set is
//! unchanged by the wiring.
//!
//! # What `blur` means here
//!
//! This renderer cannot sample the backdrop, so a true backdrop blur is not available
//! (the same limitation [`crate::render::Material::Translucent`] documents). `blur`
//! therefore controls the **elevation shadow's blur radius** on popup faces. It is a real
//! pixel change, not a no-op; the boundary is stated rather than implied.

/// One draw pass's menu-effect decisions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuEffects {
    /// Whether the adaptive configuration is active at all.
    ///
    /// `false` means every other field is irrelevant: the controls use
    /// [`MenuEffects::legacy`] verbatim.
    pub enabled: bool,
    /// Whether popup open/close animates. `false` makes the reveal jump to its target.
    pub animations: bool,
    /// Multiplier on the animation clock. `1.0` is the theme's own duration.
    pub animation_speed: f32,
    /// Whether popup faces carry the render layer's translucent alpha.
    pub transparency: bool,
    /// Whether popups cast their elevation shadow.
    pub shadows: bool,
    /// Whether that shadow is blurred. See the module docs for the boundary.
    pub blur: bool,
    /// Maximum visible entries, or `None` for the control's own historical cap.
    pub max_visible_items: Option<u32>,
    /// Whether the accelerated (blurred) shadow path is allowed.
    ///
    /// Without it a shadow is still drawn, but as a flat offset rect: the software and
    /// SVG backends both blur a shadow, and that blur is the expensive part.
    pub hardware_acceleration: bool,
    /// Whether per-widget text measurements may be reused across frames.
    pub caching: bool,
}

impl MenuEffects {
    /// The pre-wiring rendering, byte-for-byte.
    pub const fn legacy() -> Self {
        Self {
            enabled: false,
            animations: true,
            animation_speed: 1.0,
            transparency: false,
            shadows: false,
            blur: false,
            max_visible_items: None,
            hardware_acceleration: false,
            caching: false,
        }
    }

    /// The effective visible-item cap, where `fallback` is the control's own constant.
    pub const fn visible_items_or(&self, fallback: u32) -> u32 {
        match self.max_visible_items {
            Some(value) => value,
            None => fallback,
        }
    }
}

impl Default for MenuEffects {
    fn default() -> Self {
        Self::legacy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_is_pixel_neutral() {
        let legacy = MenuEffects::legacy();
        assert!(!legacy.enabled);
        assert!(legacy.animations, "popups animated before the wiring");
        assert_eq!(legacy.animation_speed, 1.0);
        assert!(!legacy.transparency);
        assert!(!legacy.shadows);
        assert!(!legacy.blur);
        assert!(!legacy.caching);
        assert_eq!(legacy.max_visible_items, None, "no cap beyond each control's own");
    }

    #[test]
    fn visible_items_or_prefers_the_configured_cap() {
        let mut effects = MenuEffects::legacy();
        assert_eq!(effects.visible_items_or(8), 8);
        effects.max_visible_items = Some(20);
        assert_eq!(effects.visible_items_or(8), 20);
    }
}
