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
    // A kind with no resolvable name cannot be classified into a role, and guessing one
    // would style the control as something it is not. The honest answer is to leave its
    // own defaults in place. This is `None` rather than the `""` the lookup used to
    // return, so the case is stated once rather than as an empty-string convention every
    // caller had to remember.
    let Some(kind_name) = crate::widget::capability::factory_name_for_kind(widget.kind()) else {
        log::debug!(
            "theme: {:?} has no resolvable factory name; leaving its own style in place",
            widget.kind()
        );
        return;
    };

    // The control's own interaction state, so a theme can describe `"button:hover"` and have it
    // take effect. `resolve_style_for_state` was already implemented and already keyed on
    // `"{kind}:{state}"`, but nothing supplied the state — the one argument in the middle was
    // always `None`, so every state override a theme author could write was unreachable.
    //
    // Asking the control is what makes the chain complete: the state is a fact the control owns,
    // and the theme is the consumer of it (the same division the touch target uses).
    let state = widget.widget_state();
    let Some(theme_style) = crate::theme::resolved_theme_style_for_state(kind_name, state) else {
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
    refresh_state_style(&mut style, &theme_style, caller_authored);
    widget.set_style(style);
}

/// Re-resolves the active theme's override for a widget's **current** state and writes it onto
/// the widget, so a state that changes after creation is painted rather than only reported.
///
/// # The defect this exists to fix
///
/// [`apply_active_theme`] runs inside the two creation funnels, so a control is themed exactly
/// **once**, in the state its constructor produced. Every latching control in the crate changes
/// its state later, through a setter (`set_checked`, `set_selected_index`, …) that fires its
/// signals and asks for a redraw — and a redraw re-runs `draw`, not the theme application. The
/// style therefore kept the fill of the creation state, and because a control's `draw` reads
/// `style.background_color` **before** anything it derives itself, that stale colour won and the
/// new state painted nothing.
///
/// Measured (probe, `ToggleButton`): themed at `Normal`, then `set_checked(true)`.
///
/// ```text
/// widget_state()                 = Checked                    ← the control tells the truth
/// style.background_color         = Some(rgb(33,150,243))      ← the *unchecked* base
/// re-rendered SVG == the unchecked SVG                       ← a visible no-op
/// ```
///
/// The snapshot set could not see it: the exporter latches its `_checked` appearances *before*
/// theming them, which is the one order that works.
///
/// # Why this is the theme layer's job
///
/// Resolving `"<kind>:<state>"` needs the factory name (to classify the kind into a role) and
/// the store's resolved style — the same two facts [`apply_active_theme`] needs, and the two
/// reasons it lives behind the `widgets_unstripped` gate. A control cannot answer either question,
/// so the setter cannot do this itself; the call has to come from the theme side.
///
/// # Why a state change and not a paint
///
/// This is deliberately **not** wired into `request_redraw`, which would re-resolve on every
/// frame for every animating control. A state change is the only thing that can make the answer
/// differ, so it is the only thing that asks.
///
/// # Gating
///
/// [`apply_active_theme`] and this function carry the **same** gate,
/// `all(not(alloc_frugal), widgets_unstripped)`. That is deliberate and load-bearing: the two
/// were previously gated differently (`full_widgets` here, the wider gate there), so a build
/// with an OS backend but no device profile — `--features windows`, which `build.rs` marks
/// `widgets_unstripped` but **not** `full_widgets` — compiled the real theme application and a
/// **no-op** state re-resolution. A state change then silently did nothing while the creation
/// path styled the control, which is the gate drift rule #47 exists to prevent.
#[cfg(all(not(alloc_frugal), widgets_unstripped))]
pub fn reapply_active_theme_state(widget: &mut dyn crate::widget::Widget) {
    apply_active_theme(widget);
}

/// The no-theme arm, for the profiles that compile `crate::theme` out.
///
/// A separate definition rather than a `cfg` inside one body, for the same reason
/// [`apply_theme_to_widget`] has one: the widget layer still names this symbol in every profile,
/// and the truthful answer where there is no theme is that nothing was applied. It carries the
/// exact complement of the gate above, so the two agree in every configuration.
#[cfg(any(alloc_frugal, not(widgets_unstripped)))]
pub fn reapply_active_theme_state(_widget: &mut dyn crate::widget::Widget) {}

/// Writes the theme's resolution for the control's **current** state into an already-themed
/// style.
///
/// # The defect this exists to fix
///
/// A control carries its style as a value that a **later** state change does not touch. The
/// theme is re-resolved only inside the two creation funnels, so a control whose state changes
/// *after* it was themed keeps the fill of the state it was created in — and `draw` reads
/// `style.background_color` *before* whatever it derives itself, so the stale colour wins and
/// the state never appears.
///
/// Measured (probe, `ToggleButton`): theme applied, then `set_checked(true)`.
///
/// ```text
/// widget_state = Checked                                  ← the control tells the truth
/// style.background_color = Some(rgb(33,150,243))          ← the *unchecked* base
/// re-rendered SVG identical to the unchecked one = true   ← nothing a user can see changed
/// ```
///
/// Every latching control has this shape, and it is the same defect each time: a state the
/// control **declares** (it reports it from `widget_state`, and the theme names a key for it)
/// that is **unobservable** because the style was resolved before the state existed. The
/// snapshot set only ever caught the `ToggleButton` instance of it, because its extra
/// appearance happens to latch before theming.
///
/// # Why `merge_theme` and not `merge`
///
/// The style going in has already been merged once, so `Some` does **not** mean "the caller set
/// this" — it means "some theme resolution set this", which is precisely the value that has to
/// be replaced. `merge_theme` makes that distinction already; reusing it keeps one rule instead
/// of two.
///
/// # Why the caller still wins
///
/// `merge_theme`'s non-theme-derived arm is a fill-only `merge`, so a colour the **caller** set
/// after the first application survives: it is `Some` and the incoming value cannot overwrite
/// it. Nothing here re-owns a caller's field.
///
/// # Why the normal state is left alone
///
/// A resting control has no `"<kind>:normal"` key in the shipped presets, so `theme_style` is
/// empty — every field is `None` — and merging it is the identity. That is what keeps this
/// from re-resolving every control on every theme application, and it is the same reasoning by
/// which [`crate::theme::resolved_theme_style`] answers a resting state exactly as
/// [`crate::theme::resolved_theme_style_for_state`] does.
#[cfg(all(not(alloc_frugal), widgets_unstripped))]
fn refresh_state_style(style: &mut WidgetStyle, theme_style: &WidgetStyle, caller_authored: bool) {
    style.merge_theme(theme_style);
    if !caller_authored {
        style.theme_derived = true;
    }
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

    /// A theme's `"<kind>:<state>"` override reaches a **live control**.
    ///
    /// # Why this test exists
    ///
    /// `ThemeManager::resolve_style_for_state` and the `"{kind}:{state}"` key format were both
    /// implemented and both already tested at the manager level — and nothing in the control layer
    /// could reach them, because every caller passed `None` for the state. A theme author could
    /// write `"button:hover"`, the manager would resolve it correctly when asked directly, and no
    /// control would ever be affected.
    ///
    /// So this test goes through the path a real control takes: it installs a theme carrying a
    /// state override, disables a control, and asserts the override's colour arrives on the
    /// control's own style. Asserting against the manager would have passed before the fix.
    #[test]
    fn a_state_override_reaches_the_control_itself() {
        let _guard = guard();

        // A theme whose only special feature is a `disabled` override for buttons.
        let mut manager = global_theme_manager();
        let mut theme = crate::theme::Theme::default();
        theme.overrides.styles.insert(
            "button:disabled".to_string(),
            crate::theme::ThemeStyleToken {
                background: Some(Color::rgba(7, 8, 9, 255)),
                ..Default::default()
            },
        );
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));
        drop(manager);

        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        button.set_enabled(false);

        apply_active_theme(&mut button);

        assert_eq!(
            button.style().background_color,
            Some(Color::rgba(7, 8, 9, 255)),
            "the `button:disabled` override must reach the control, which is the state it reports"
        );

        // The reverse direction: an *enabled* control is in the resting state, so the disabled
        // override must not reach it. Without this the test would pass on an implementation that
        // applied every state override unconditionally.
        let mut enabled = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        apply_active_theme(&mut enabled);
        assert_ne!(
            enabled.style().background_color,
            Some(Color::rgba(7, 8, 9, 255)),
            "an enabled button is not in the disabled state, so that override must not apply"
        );
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

    /// BLUE23 §2.4, judgement 2 — the preset's state overrides reach a live control.
    ///
    /// A theme could describe `"button:hover"` and the manager would resolve it
    /// correctly when asked directly, yet nothing on screen changed: the presets shipped
    /// `overrides.styles` **empty**. This goes through the path a real control takes and
    /// asserts that a hovered button (which is exactly `widget_state() == Hover` since
    /// the state source was elevated) is filled differently from a resting one.
    #[test]
    fn a_hovered_button_is_filled_differently_from_a_resting_one() {
        let _guard = guard();
        let mut manager = global_theme_manager();
        assert!(manager.set_theme("default"), "the default preset must be registered");
        drop(manager);

        let mut resting = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        apply_active_theme(&mut resting);

        // `MouseEnter` is what the runtime synthesises for the control under the pointer,
        // and the base records it — so this is the same path a real hover takes.
        let mut hovered = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        crate::event::EventHandler::handle_event(
            &mut hovered,
            &crate::event::Event::MouseEnter { pos: crate::core::Point::new(5, 5) },
        );
        assert_eq!(
            hovered.widget_state(),
            crate::style::WidgetState::Hover,
            "the control must actually report the state the override is keyed on"
        );
        apply_active_theme(&mut hovered);

        assert_ne!(
            hovered.style().background_color,
            resting.style().background_color,
            "`button:hover` in the preset must change the painted fill"
        );
    }

    /// A state change that happens **after** the theme was applied still repaints.
    ///
    /// # The defect this pins
    ///
    /// `apply_active_theme` resolves the theme exactly once, at creation. Every latching control
    /// (`ToggleButton`, `CheckBox`, `Switch`, `RadioButton`, `SegmentedButton`) changes its state
    /// through a *setter* — `set_checked` / `set_selected_index` — which fires signals and asks for
    /// a redraw but does **not** re-theme. The style therefore kept the fill of the state the control
    /// was created in, and because `draw` reads `style.background_color` *before* whatever it derives
    /// itself, the stale colour won and the new state painted nothing.
    ///
    /// So `set_checked` on a live control was a **visible no-op**: measured on `ToggleButton`, the
    /// control reported `widget_state() == Checked` while its re-rendered SVG was byte-identical to
    /// the unchecked one. The snapshot set could not see this, because the exporter latches its
    /// `_checked` appearances *before* theming them.
    ///
    /// # Why the assertion is on the re-rendered geometry
    ///
    /// `is_checked()`, `widget_state()` and `set_checked`'s own return all passed throughout — the
    /// model was never wrong, only the painting. Only the emitted geometry can catch it.
    #[test]
    fn a_latch_that_changes_after_the_theme_was_applied_still_repaints() {
        use crate::widget::ToggleButton;
        let _guard = guard();
        let mut manager = global_theme_manager();
        assert!(manager.set_theme("default"), "the default preset must be registered");
        drop(manager);

        let bounds = Rect::new(0, 0, 120, 40);
        let mut off = ToggleButton::new("On".to_string(), bounds);
        apply_active_theme(&mut off);
        let resting = crate::widget::svg::render_widget_to_svg(&mut off, bounds);

        // Themed **first**, latched second — the order a running application produces, and the one
        // the exporter never exercises.
        let mut on = ToggleButton::new("On".to_string(), bounds);
        apply_active_theme(&mut on);
        on.set_checked(true);
        assert_eq!(on.widget_state(), crate::style::WidgetState::Checked);
        let latched = crate::widget::svg::render_widget_to_svg(&mut on, bounds);

        assert_ne!(
            resting, latched,
            "a latch applied after theming must change the painted fill, not only the model"
        );
    }

    /// The same defect in the **other five** state-reporting controls, each pinned separately.
    ///
    /// # Why one test and not five assertions elsewhere
    ///
    /// These controls fail for **different** reasons, and only a per-control assertion can tell
    /// them apart:
    ///
    /// * `CheckBox` and `RadioButton` already *painted* correctly — their `draw` reads a colour it
    ///   derives rather than `style.background_color`, so the creation-state style may not be the
    ///   field that paints. They are asserted here so a future change to either `draw` cannot
    ///   silently introduce the defect.
    /// * `Switch` is the interesting one: its track colour is `off_track.blend(&on_track,
    ///   travel.value())`, and `travel` only advances in `tick`. So a latch with no frame in
    ///   between **cannot** change the pixels, by design — the animation *is* the visible change.
    ///   What must change immediately is the *style*, because that is what a caller reads and what
    ///   a later re-theme merges onto, and it is what the `switch:checked` key exists to carry.
    /// * `Chip` reports `Selected`, which is a **different** state from `Checked` and therefore a
    ///   different key — so its assertion names the resolution rather than only comparing a field.
    ///   Before the fix a selected chip still resolved plain `chip`, and `chip:selected`'s
    ///   `rgb(33,150,243)` reached nothing.
    ///
    /// # What is deliberately **not** here
    ///
    /// `LineEdit` reports `Focused` from `widget_state`, but the preset declares no
    /// `"line_edit:focused"` key, so re-resolving changes nothing and an assertion would be
    /// vacuous. Its one semantic key, `line_edit:error`, travels a **separate** channel by design
    /// — `resolved_semantic_border` is resolved at draw time from `semantic_state`, precisely so
    /// the border channel and the interaction-fill channel cannot overwrite each other. A
    /// `SegmentedButton` is also out of scope and correctly so: it overrides neither
    /// `widget_state` nor any theme state key, so it has no state for the theme to key on.
    ///
    /// The assertion is on the style rather than the geometry, because the geometry legitimately
    /// lags for an animated control.
    #[test]
    fn the_other_state_reporting_controls_re_resolve_after_themming() {
        use crate::widget::special_widgets::chip::{Chip, ChipItem};
        use crate::widget::{CheckBox, RadioButton, Switch};
        let _guard = guard();
        let mut manager = global_theme_manager();
        assert!(manager.set_theme("default"), "the default preset must be registered");
        drop(manager);
        let bounds = Rect::new(0, 0, 120, 40);

        let mut check_box = CheckBox::new(bounds);
        check_box.set_checked(false);
        apply_active_theme(&mut check_box);
        let unchecked = check_box.style().background_color;
        check_box.set_checked(true);
        assert_ne!(
            unchecked,
            check_box.style().background_color,
            "`check_box:checked` must reach a box that latched after it was themed"
        );

        let mut radio = RadioButton::new(bounds);
        apply_active_theme(&mut radio);
        let unselected = radio.style().background_color;
        radio.set_checked(true);
        assert_ne!(
            unselected,
            radio.style().background_color,
            "`radio_button:checked` must reach a radio selected after it was themed"
        );

        let mut switch = Switch::new(bounds);
        apply_active_theme(&mut switch);
        let off = switch.style().background_color;
        switch.set_checked(true);
        assert_ne!(
            off,
            switch.style().background_color,
            "`switch:checked` must reach a switch that latched after it was themed"
        );

        // `Chip` selects through `toggle_index`, not a `set_checked`. The expected value is read
        // from the resolver, so the assertion says "the selected key was reached" rather than
        // pinning a colour that a preset change would legitimately move.
        let mut chip = Chip::new(bounds);
        chip.set_items(vec![ChipItem::new("a", "A"), ChipItem::new("b", "B")]);
        apply_active_theme(&mut chip);
        assert_eq!(chip.widget_state(), crate::style::WidgetState::Normal);
        chip.toggle_index(0);
        assert_eq!(chip.widget_state(), crate::style::WidgetState::Selected);
        assert_eq!(
            chip.style().background_color,
            crate::theme::resolved_theme_style_for_state(
                "chip",
                crate::style::WidgetState::Selected
            )
            .and_then(|resolved| resolved.background_color),
            "a chip selected after theming must resolve `chip:selected`, not the unselected `chip`"
        );
    }

    /// A-1: disabling / hovering / pressing a control **after** it was themed re-resolves it.
    ///
    /// # The defect this pins
    ///
    /// `apply_active_theme` used to run only inside the two creation funnels, and the three
    /// momentary inputs of [`crate::widget::Widget::widget_state`] — `enabled`, `hovered`,
    /// `pressed` — were plain field writes. So `button:disabled` / `button:hover` / `button:pressed`
    /// (and their siblings on `toggle_button` / `tool_button` / `split_button`) were reachable
    /// **only** when the control happened to be created in that state. Disabling a live button is a
    /// published, writable property (`properties_base.in.rs` lists `enabled` for all of them), so
    /// this was user-visible: a form that disabled its submit button changed nothing on screen.
    ///
    /// # Why the assertion names the resolved value
    ///
    /// The expected fill is read back from the resolver rather than pinned, so the test says "the
    /// `button:disabled` key was reached" instead of asserting a colour a preset change would
    /// legitimately move. The size of the change is not the contract; reaching the key is.
    #[test]
    fn a_state_change_after_theming_re_resolves_the_state_key() {
        use crate::widget::{Button, ToolButton, Widget};
        let _guard = guard();
        let mut manager = global_theme_manager();
        assert!(manager.set_theme("default"), "the default preset must be registered");
        drop(manager);
        let bounds = Rect::new(0, 0, 120, 40);

        // `enabled` is a published writable property, so this is the JSON / designer path too.
        let mut button = Button::new("ok".to_string(), bounds);
        apply_active_theme(&mut button);
        let enabled_fill = button.style().background_color;
        button.set_enabled(false);
        assert_eq!(
            button.style().background_color,
            crate::theme::resolved_theme_style_for_state(
                "button",
                crate::style::WidgetState::Disabled
            )
            .and_then(|resolved| resolved.background_color)
            .or(enabled_fill),
            "disabling a themed button must re-resolve `button:disabled`"
        );

        // The same key family covers `tool_button`; the trait default is what makes it free.
        let mut tool = ToolButton::new("go".to_string(), bounds);
        apply_active_theme(&mut tool);
        let tool_enabled = tool.style().background_color;
        tool.set_enabled(false);
        assert_ne!(
            tool.style().background_color,
            tool_enabled,
            "`tool_button:disabled` must reach a tool button disabled after theming"
        );

        // Hover and press travel the same path, through their own shared defaults.
        let mut hovered = Button::new("ok".to_string(), bounds);
        apply_active_theme(&mut hovered);
        let resting = hovered.style().background_color;
        hovered.set_hovered(true);
        assert_eq!(
            hovered.style().background_color,
            crate::theme::resolved_theme_style_for_state(
                "button",
                crate::style::WidgetState::Hover
            )
            .and_then(|resolved| resolved.background_color)
            .or(resting),
            "hovering a themed button must re-resolve `button:hover`"
        );
        hovered.set_pressed(true);
        assert_eq!(
            hovered.style().background_color,
            crate::theme::resolved_theme_style_for_state(
                "button",
                crate::style::WidgetState::Pressed
            )
            .and_then(|resolved| resolved.background_color)
            .or(resting),
            "pressing a themed button must re-resolve `button:pressed`"
        );
    }

    /// B-1: the theme's `spacing` tokens reach a control's `padding` and `margin`.
    ///
    /// # The defect this pins
    ///
    /// `role_base_style` writes `padding: Padding::all(theme.spacing.medium)` and
    /// `margin: Margin::all(theme.spacing.small)`, but neither `merge_theme` nor `merge` copied
    /// those two fields — the resolution produced a value and then dropped it. Every themed
    /// control kept `Padding::all(0)`, `Widget::padding()` always answered zero, and
    /// `command_link`'s draw read a constant. Nothing asserted otherwise, which is why a defect
    /// affecting all 188 controls had no test.
    #[test]
    fn the_themes_spacing_tokens_reach_a_controls_padding_and_margin() {
        let _guard = guard();
        let mut manager = global_theme_manager();
        assert!(manager.set_theme("default"), "the default preset must be registered");
        let spacing = manager.current_theme().expect("the default theme is active").spacing.medium;
        let small = manager.current_theme().expect("the default theme is active").spacing.small;
        drop(manager);

        let mut button = crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 120, 40));
        assert_eq!(
            button.style().padding,
            crate::style::Padding::default(),
            "the control must start with no padding, so the assertion below is not vacuous"
        );
        apply_active_theme(&mut button);

        assert_eq!(
            button.style().padding,
            crate::style::Padding::all(spacing),
            "`theme.spacing.medium` must reach the control's padding"
        );
        assert_eq!(
            button.style().margin,
            crate::style::Margin::all(small),
            "`theme.spacing.small` must reach the control's margin"
        );
        // The accessor the draw paths read must see it too, not only the raw field.
        assert_eq!(
            *crate::widget::Widget::padding(&button),
            crate::style::Padding::all(spacing),
            "`Widget::padding()` must agree with the style it reads"
        );
    }
}
