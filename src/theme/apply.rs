// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Theme application — the single point where a created control picks up the
//! active theme.
//!
//! # Why this is a separate module
//!
//! Every widget reaches the runtime through `crate::widget::runtime::register`, and
//! there are exactly two callers that construct before registering:
//! `crate::mount_widget_object` (the crate-root creation path that the C ABI and the
//! window API use) and
//! `CustomPaintControlBackend::mount_widget_of_kind`. Both must apply the theme, or a
//! control created one way is styled and a control created the other way is not.
//!
//! Before this existed, the theme was applied **only** by the JSON loader — which has
//! no production callers. So a theme switch did nothing for the widgets an
//! application actually creates through the C ABI: the one path with real users was
//! the one path without theming.
//!
//! # Precedence
//!
//! The theme is merged **under** whatever style the widget already carries
//! (`WidgetStyle::merge` only fills unset fields), so:
//!
//! 1. an explicit style the caller set (or the constructor chose) wins;
//! 2. otherwise the active theme supplies the value;
//! 3. otherwise the widget's own default stands.
//!
//! That ordering is what lets a caller override one property of one control without
//! restating the theme, and is the same precedence the JSON path documents.
//!
//! # Gating
//!
//! The real `apply_active_theme` needs the capability registry (it classifies a
//! widget by its factory name), so it is gated `not(alloc_frugal)` **and**
//! `widgets_unstripped`. Two no-op arms cover the builds where it cannot work:
//!
//! * `alloc_frugal` (`mini`) — no theme module to apply at all;
//! * `device_profile` with stripped widgets — a theme module exists but there is no
//!   registry to resolve a role from, so a control keeps its own defaults rather
//!   than being styled by a guess.
//!
//! Both arms return without styling, which is the truthful answer in each case.
//! Keeping them here — rather than at a call site — is what stops a call site from
//! growing its own `cfg` that then drifts out of step with the module's gate.

#[cfg(all(not(alloc_frugal), widgets_unstripped))]
use crate::style::WidgetStyle;
#[cfg(all(not(alloc_frugal), widgets_unstripped))]
use crate::widget::Widget;

/// Applies the active theme to a widget that is about to be registered.
///
/// Classifies the widget by its **kind name** via
/// `WidgetRole::for_kind_name`, which is the same classification the JSON path
/// uses, so a control looks the same whichever way it was created.
///
/// Does nothing when no theme is active. The global manager always has a theme
/// registered, so in practice this always applies; the `None` case exists for a
/// caller that replaced the manager with an empty one.
///
/// Gated on `widgets_unstripped` because `factory_name_for_kind` is: the lookup
/// needs the capability registry, which a stripped build does not compile. The
/// no-op arm below covers that case.
#[cfg(all(not(alloc_frugal), widgets_unstripped))]
pub(crate) fn apply_active_theme(widget: &mut dyn Widget) {
    let kind_name = crate::widget::capability::factory_name_for_kind(widget.kind());
    if kind_name.is_empty() {
        // A kind with no resolvable name cannot be classified into a role, and
        // guessing one would style the control as something it is not. The honest
        // answer is to leave its own defaults in place.
        log::debug!(
            "theme: {:?} has no resolvable factory name; leaving its own style in place",
            widget.kind()
        );
        return;
    }

    let Some(theme_style) = crate::theme::resolved_theme_style(kind_name) else {
        return;
    };

    // `merge_theme`, not `merge`: a control that already carries the *previous* theme's
    // colours must be re-rendered in the new one. A plain `merge` fills only `None`
    // fields, so after light → dark every already-styled control kept the light palette
    // — the switch appeared to do nothing. A field the caller set is still preserved;
    // see `WidgetStyle::merge_theme` for how the two cases are told apart.
    //
    // A style the theme authored is marked as such, which is what lets the *next*
    // application replace these values. A style the caller has also touched stays
    // unmarked, so its caller-set fields survive every later switch.
    let mut style = widget.style().clone();
    let caller_authored = !style.theme_derived && style != WidgetStyle::default();
    style.merge_theme(&theme_style);
    if !caller_authored {
        style.theme_derived = true;
    }
    widget.set_style(style);
}

/// Applies the active theme on a device build whose widget registry is stripped.
///
/// `crate::theme` is gated `device_profile`, which is *wider* than
/// `widgets_unstripped`: a build that overlaps two axis-1 features (e.g.
/// `--features "tablet,embedded"`) has a theme module but no capability registry,
/// so `factory_name_for_kind` does not exist. Without this arm such a build failed
/// to compile with `cannot find function factory_name_for_kind`.
///
/// Classification needs the factory name, so there is no honest way to resolve a
/// role here. Doing nothing is the truthful answer: a control keeps its own
/// defaults rather than being styled by a guess, which is the same rule the
/// unresolvable-kind branch above follows.
#[cfg(all(device_profile, not(widgets_unstripped)))]
pub(crate) fn apply_active_theme(_widget: &mut dyn crate::widget::Widget) {}

/// No-op for the allocation-frugal profile, which has no theme module.
#[cfg(alloc_frugal)]
pub(crate) fn apply_active_theme(_widget: &mut dyn crate::widget::Widget) {}

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;
    use crate::core::{Color, Rect};
    use crate::theme::{self, global_theme_manager, AppearanceMode};

    /// Serialises the tests that switch the process-wide theme.
    fn guard() -> std::sync::MutexGuard<'static, ()> {
        theme::theme_test_guard()
    }

    #[test]
    fn a_widget_with_no_style_gets_the_theme() {
        let _guard = guard();
        let mut manager = global_theme_manager();
        assert!(manager.set_appearance(AppearanceMode::Light));
        drop(manager);

        let primary = global_theme_manager().current_theme().expect("active").colors.primary;
        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        assert_eq!(button.style().background_color, None, "before: unset");

        apply_active_theme(&mut button);

        assert_eq!(
            button.style().background_color,
            Some(primary),
            "a button's fill must come from the theme's primary token"
        );
    }

    /// An explicit style wins over the theme, which is what lets a caller override
    /// one control without restating the palette.
    #[test]
    fn an_explicit_style_is_not_overwritten() {
        let _guard = guard();
        let explicit = Color::rgb(1, 2, 3);
        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        button.set_background_color(Some(explicit));

        apply_active_theme(&mut button);

        assert_eq!(
            button.style().background_color,
            Some(explicit),
            "the theme must not replace a colour the caller set"
        );
    }

    /// A theme switch changes what a newly created control receives, so the switch
    /// is observable through this path rather than only through the JSON loader.
    #[test]
    fn switching_appearance_changes_what_a_new_control_gets() {
        let _guard = guard();

        {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Light));
        }
        let mut light = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        apply_active_theme(&mut light);

        {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Dark));
        }
        let mut dark = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        apply_active_theme(&mut dark);

        assert_ne!(
            light.style().background_color,
            dark.style().background_color,
            "a light and a dark theme must fill a button differently"
        );

        global_theme_manager().set_appearance(AppearanceMode::Light);
    }

    /// Re-applying a theme after a switch really changes an already-styled control.
    ///
    /// # The defect this pins
    ///
    /// [`WidgetStyle::merge`] fills only fields that are `None`, which is what makes a
    /// caller-set colour survive the theme. That rule alone cannot express "replace what
    /// the **previous** theme put here": once a control had been styled, its fields were no
    /// longer `None`, so switching light → dark had nowhere to write and every control on
    /// screen kept the light palette. The switch appeared to do nothing.
    ///
    /// This applies a theme, switches, and applies again — the sequence
    /// `crate::reapply_active_theme` performs — and asserts the control actually changed.
    #[test]
    fn reapplying_after_a_switch_replaces_the_previous_themes_colours() {
        let _guard = guard();

        global_theme_manager().set_appearance(AppearanceMode::Light);
        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        apply_active_theme(&mut button);
        let light_fill = button.style().background_color;
        assert!(light_fill.is_some(), "the first application must style the button");

        global_theme_manager().set_appearance(AppearanceMode::Dark);
        apply_active_theme(&mut button);
        let dark_fill = button.style().background_color;

        assert_ne!(
            light_fill, dark_fill,
            "re-applying after a switch must replace the previous theme's colour, not be
             silently refused because the field is already set"
        );

        global_theme_manager().set_appearance(AppearanceMode::Light);
    }

    /// A caller's explicit colour survives every later theme application.
    ///
    /// The counterpart of the test above: making the theme able to replace *its own*
    /// previous values must not let it start overwriting a caller's. Both directions
    /// matter, and a fix that satisfied only one would trade a stale palette for a lost
    /// override.
    #[test]
    fn a_callers_colour_survives_repeated_theme_applications() {
        let _guard = guard();
        let explicit = Color::rgb(1, 2, 3);

        global_theme_manager().set_appearance(AppearanceMode::Light);
        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        button.set_background_color(Some(explicit));

        // Two applications through a switch: the case that would overwrite if the theme
        // treated every set field as its own.
        apply_active_theme(&mut button);
        global_theme_manager().set_appearance(AppearanceMode::Dark);
        apply_active_theme(&mut button);

        assert_eq!(
            button.style().background_color,
            Some(explicit),
            "a colour the caller set must survive a theme switch"
        );

        global_theme_manager().set_appearance(AppearanceMode::Light);
    }

    /// The high-contrast override reaches controls created through this path, not
    /// only through the JSON loader.
    #[test]
    fn a_high_contrast_override_reaches_a_new_control() {
        use crate::style::HighContrastMode;
        let _guard = guard();

        global_theme_manager().set_high_contrast(HighContrastMode::WhiteOnBlack);
        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        apply_active_theme(&mut button);

        assert_eq!(button.style().background_color, Some(Color::BLACK));
        assert_eq!(button.style().text_color, Some(Color::WHITE));

        global_theme_manager().set_high_contrast(HighContrastMode::None);
    }

    /// Distinct kinds receive distinct treatments, so the classification is really
    /// happening rather than every control getting the same defaults.
    #[test]
    fn different_kinds_get_different_treatments() {
        let _guard = guard();
        {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Light));
        }

        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        let mut label = crate::widget::Label::new("hi".to_string(), Rect::new(0, 0, 10, 10));
        apply_active_theme(&mut button);
        apply_active_theme(&mut label);

        assert_ne!(
            button.style().background_color,
            label.style().background_color,
            "a filled action and a plain label must not share a background"
        );
    }
}
