// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One place that answers "what is the menu effect policy?".
//!
//! The menu controls and [`super::popup_reveal`] are available in every `full_widgets`
//! build, while the adaptive configuration lives behind `advanced-widgets`. This module
//! is the single `cfg` boundary between them: a build without the feature answers
//! [`MenuEffects::legacy`], which is exactly the pre-wiring rendering.

use crate::core::{Color, Rect};
use crate::render::{RenderCommand, RenderContext};
use crate::style::MenuEffects;

#[cfg(test)]
thread_local! {
    /// A per-thread effect override, so a test can exercise a policy without changing the
    /// process-wide configuration that other tests draw with in parallel.
    static TEST_EFFECTS: core::cell::Cell<Option<MenuEffects>> = const { core::cell::Cell::new(None) };
}

/// Runs `f` with a per-thread effect override installed, restoring the previous one after.
#[cfg(test)]
pub(crate) fn with_test_effects<R>(effects: MenuEffects, f: impl FnOnce() -> R) -> R {
    let previous = TEST_EFFECTS.with(|cell| cell.replace(Some(effects)));
    let result = f();
    TEST_EFFECTS.with(|cell| cell.set(previous));
    result
}

/// The effect decisions for this build and this process.
pub(crate) fn current() -> MenuEffects {
    #[cfg(test)]
    if let Some(effects) = TEST_EFFECTS.with(|cell| cell.get()) {
        return effects;
    }
    #[cfg(feature = "advanced-widgets")]
    {
        crate::menu_config::current_effects()
    }
    #[cfg(not(feature = "advanced-widgets"))]
    {
        MenuEffects::legacy()
    }
}

/// Paints a popup's elevation shadow.
///
/// Shared by `menu` and `dropdown_menu` so the shadow cannot be one set of numbers in one
/// control and another in the other. `blur` in [`MenuEffects`] is the shadow's blur radius:
/// the renderer cannot sample the backdrop, so a backdrop blur is not available (the same
/// boundary `Material::Translucent` documents). Without hardware acceleration the shadow
/// is a flat offset rect; the blurred path is the expensive one that flag gates.
pub(crate) fn paint_popup_shadow(context: &mut RenderContext, rect: Rect, effects: &MenuEffects) {
    let blur_radius = if effects.blur {
        12
    } else if effects.hardware_acceleration {
        6
    } else {
        0
    };
    context.execute_command(RenderCommand::BoxShadow {
        rect,
        // The backends halve a `BoxShadow`'s alpha, so 120 renders as ~60/255.
        color: Color::rgba(0, 0, 0, 120),
        offset_x: 0,
        offset_y: 3,
        blur_radius,
        spread: 0,
    });
}
