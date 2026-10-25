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

/// How long the pointer must rest on a branch row before its child panel opens.
///
/// The delay is shared by the whole menu family: a `Menu`, a `DropdownMenu` and a
/// `MenuButton` that opened a branch at different speeds would read as three different
/// controls. A short delay is what distinguishes "passing over" a branch from "asking
/// for" it, which is the habit the baseline toolkit establishes.
pub(crate) const SUBMENU_HOVER_DELAY_MS: u32 = 300;

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
///
/// During a test build the process-wide policy is deliberately ignored: any test that
/// constructs an `App` runs `menu_config::init()`, which probes the host and can install a
/// different visible-item cap while another test is drawing a popup. A test that wants a
/// policy asks for it explicitly with [`with_test_effects`], so control tests stay
/// deterministic no matter what runs in parallel.
pub(crate) fn current() -> MenuEffects {
    #[cfg(test)]
    {
        TEST_EFFECTS.with(|cell| cell.get()).unwrap_or_else(MenuEffects::legacy)
    }
    #[cfg(not(test))]
    {
        #[cfg(feature = "advanced-widgets")]
        {
            crate::menu_config::current_effects()
        }
        #[cfg(not(feature = "advanced-widgets"))]
        {
            MenuEffects::legacy()
        }
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
    if !effects.shadows {
        return;
    }
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

/// Applies the configured popup material to a resolved face colour.
///
/// # What this is, and what it is not
///
/// A platform backdrop blur needs a layer the renderer can sample behind the popup, and
/// this renderer has no such primitive (the same boundary `Material::Translucent` and
/// `MenuEffects::blur` document). The material is therefore expressed the way the render
/// stack can honour it: the face keeps its colour and becomes partly translucent, so the
/// surface behind it shows through. The result is a real pixel change when the policy is
/// on, and byte-identical output when it is off.
///
/// Both the menu panel and the dropdown panel call this, so the two popups cannot end up
/// with two different alpha values for one configuration.
pub(crate) fn popup_face(face: Color, effects: &MenuEffects) -> Color {
    if effects.transparency {
        face.with_alpha(230)
    } else {
        face
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::PaintBackend;

    fn render_shadow(effects: &MenuEffects) -> String {
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(200, 200));
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        paint_popup_shadow(&mut context, Rect::new(10, 10, 50, 50), effects);
        backend.end_frame();
        backend.finish()
    }

    #[test]
    fn the_popup_material_is_opt_in() {
        let face = Color::rgb(40, 60, 80);
        let mut effects = MenuEffects::legacy();
        assert_eq!(popup_face(face, &effects), face, "the legacy policy changes no pixel");
        effects.transparency = true;
        assert_eq!(popup_face(face, &effects).a, 230, "the material alpha is applied");
    }

    #[test]
    fn a_shadow_needs_the_policy() {
        let plain = render_shadow(&MenuEffects::legacy());
        let mut effects = MenuEffects::legacy();
        effects.shadows = true;
        let shadowed = render_shadow(&effects);
        assert_ne!(plain, shadowed, "enabling shadows must paint something");
    }
}
