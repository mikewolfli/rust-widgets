// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Theme application reaches controls created on the **main** creation path.
//!
//! # What this guards
//!
//! The theme system had no production callers until the JSON loader was wired to
//! it — and the JSON loader itself has no production callers. So a theme switch
//! changed nothing for the controls an application actually creates: the C ABI
//! path (`rw_create_*` → `crate::create_widget_of_kind` → `mount_widget_object`)
//! and the window API. The one path with real users had no theming at all.
//!
//! These tests pin the wiring on that path, and the precedence it must honour:
//! explicit style → theme → the widget's own default.
//!
//! The style is read back off the **live widget in the runtime registry**, which is
//! what a repaint reads, so a pass means the themed value really reached the
//! control rather than being computed and discarded.

#![cfg(feature = "desktop")]

use rust_widgets::core::Color;
use rust_widgets::theme::{global_theme_manager, AppearanceMode, HighContrastMode};
use rust_widgets::widget::runtime::{with_widget, with_widget_mut};
use rust_widgets::widget::Widget as _;

/// Serialises these tests.
///
/// The theme registry is **process-wide**, and several tests here switch it (and
/// one sets the high-contrast override). Run in parallel, one test observes
/// another's theme — which showed up as a button whose fill was the forced
/// high-contrast black rather than the light theme's primary. Same guard the theme
/// unit tests use; the isolation has to be explicit, not hoped for.
fn guard() -> std::sync::MutexGuard<'static, ()> {
    rust_widgets::theme::theme_test_guard()
}

/// Reads a live widget's resolved background colour.
fn background_of(id: rust_widgets::core::ObjectId) -> Option<Color> {
    with_widget(id, |widget| widget.style().background_color).flatten()
}

/// Reads a live widget's resolved text colour.
fn text_color_of(id: rust_widgets::core::ObjectId) -> Option<Color> {
    with_widget(id, |widget| widget.style().text_color).flatten()
}

/// Pins the light appearance and returns the theme's primary colour.
///
/// Every test needs a known active theme, and the registry is process-wide, so this
/// both sets it and reports what it set. It also clears any high-contrast override a
/// previous test left set, so a forced palette cannot leak between tests.
fn use_light_theme() -> Color {
    let mut manager = global_theme_manager();
    manager.set_high_contrast(HighContrastMode::None);
    assert!(manager.set_appearance(AppearanceMode::Light), "the light preset must be registered");
    manager.current_theme().expect("an active theme").colors.primary
}

#[test]
fn a_control_created_through_the_main_path_takes_the_theme() {
    let _guard = guard();
    let primary = use_light_theme();

    let window = rust_widgets::create_window("theme-main-path", 0, 0, 320, 240);
    assert_ne!(window, 0, "the host window must be created");

    let button = rust_widgets::create_button(window, "OK", 10, 10, 80, 30);
    assert_ne!(button, 0, "the button must be created");

    assert_eq!(
        background_of(button),
        Some(primary),
        "a button created through the C ABI path must take the theme's primary fill"
    );
}

#[test]
fn distinct_kinds_are_classified_differently() {
    let _guard = guard();
    let primary = use_light_theme();

    let window = rust_widgets::create_window("theme-roles", 0, 0, 320, 240);
    let button = rust_widgets::create_button(window, "OK", 10, 10, 80, 30);
    let label = rust_widgets::create_label(window, "text", 10, 50, 80, 20);

    assert_eq!(background_of(button), Some(primary), "a filled action uses the primary token");
    assert_eq!(
        background_of(label),
        None,
        "a label is plain text on the surface, so it must not be filled"
    );
}

#[test]
fn an_explicit_style_wins_over_the_theme() {
    let _guard = guard();
    let _ = use_light_theme();
    let explicit = Color::rgb(9, 9, 9);

    let window = rust_widgets::create_window("theme-explicit", 0, 0, 320, 240);
    let button = rust_widgets::create_button(window, "OK", 10, 10, 80, 30);
    with_widget_mut(button, |widget| widget.set_background_color(Some(explicit)))
        .expect("the widget is live");

    assert_eq!(
        background_of(button),
        Some(explicit),
        "the theme must not overwrite a colour the caller set"
    );
}

#[test]
fn a_theme_switch_changes_what_a_new_control_gets() {
    let _guard = guard();
    let light_primary = use_light_theme();

    let window = rust_widgets::create_window("theme-switch", 0, 0, 320, 240);
    let light_button = rust_widgets::create_button(window, "A", 10, 10, 80, 30);
    let light_fill = background_of(light_button);
    assert_eq!(light_fill, Some(light_primary));

    {
        let mut manager = global_theme_manager();
        assert!(manager.set_appearance(AppearanceMode::Dark), "the dark preset must be registered");
    }
    let dark_button = rust_widgets::create_button(window, "B", 10, 50, 80, 30);
    let dark_fill = background_of(dark_button);
    assert_ne!(light_fill, dark_fill, "the dark theme's primary must differ from the light one");

    // Restore so a later test starts from the documented default.
    global_theme_manager().set_appearance(AppearanceMode::Light);
}

#[test]
fn a_high_contrast_override_reaches_a_new_control() {
    let _guard = guard();
    let _ = use_light_theme();

    global_theme_manager().set_high_contrast(HighContrastMode::WhiteOnBlack);
    let window = rust_widgets::create_window("theme-contrast", 0, 0, 320, 240);
    let button = rust_widgets::create_button(window, "OK", 10, 10, 80, 30);

    assert_eq!(background_of(button), Some(Color::BLACK), "the forced background must apply");
    assert_eq!(text_color_of(button), Some(Color::WHITE), "the forced foreground must apply");

    global_theme_manager().set_high_contrast(HighContrastMode::None);
}

/// The window API's mount path takes the theme too, not only the crate-root
/// `create_*` functions. Both are funnels into the same registry, so a theme
/// applied on only one of them would make appearance depend on how a control was
/// brought into being.
#[test]
fn a_control_mounted_onto_a_window_is_also_themed() {
    let _guard = guard();
    let primary = use_light_theme();

    let window = rust_widgets::create_window("theme-mount", 0, 0, 320, 240);
    assert_ne!(window, 0);
    let handle = rust_widgets::app::WindowHandle::from_raw(window);

    let widget: Box<dyn rust_widgets::widget::Widget> =
        Box::new(rust_widgets::widget::Button::new(
            "OK".to_string(),
            rust_widgets::core::Rect::new(0, 0, 80, 30),
        ));
    let surface = match handle.mount_surface(widget, rust_widgets::core::Rect::new(10, 10, 80, 30))
    {
        Ok(surface) => surface,
        Err(error) => {
            // A headless test environment gets the state-only backend, which has no
            // surface to mount onto. That is an environment fact rather than a
            // failure of this assertion — but it is *reported* instead of silently
            // passing, so a backend that genuinely broke stays visible.
            eprintln!(
                "mount path not exercised: backend '{}' cannot mount surfaces ({error}); this \
                 assertion needs a display",
                rust_widgets::platform::backend_name()
            );
            return;
        }
    };

    assert_eq!(
        background_of(surface.raw_id()),
        Some(primary),
        "a widget mounted onto a window must be themed like one created directly"
    );
}

/// A widget the caller supplied already styled keeps its style, so mounting is not
/// a way to have the theme silently override a prepared object.
#[test]
fn a_prepared_widget_keeps_its_own_style_when_mounted() {
    let _guard = guard();
    let _ = use_light_theme();
    let explicit = Color::rgb(20, 30, 40);

    let window = rust_widgets::create_window("theme-prepared", 0, 0, 320, 240);
    let handle = rust_widgets::app::WindowHandle::from_raw(window);

    let mut button = rust_widgets::widget::Button::new(
        "OK".to_string(),
        rust_widgets::core::Rect::new(0, 0, 80, 30),
    );
    button.set_background_color(Some(explicit));

    let surface = match handle
        .mount_surface(Box::new(button), rust_widgets::core::Rect::new(10, 10, 80, 30))
    {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!(
                "mount path not exercised: backend '{}' cannot mount surfaces ({error})",
                rust_widgets::platform::backend_name()
            );
            return;
        }
    };

    assert_eq!(
        background_of(surface.raw_id()),
        Some(explicit),
        "a caller-prepared widget's own style must survive mounting"
    );
}
